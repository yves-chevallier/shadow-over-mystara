//! Démonstrations de l'attract : les deux interpréteurs de scripts de scènes de la machine à modes
//! (sous-modes 4 et 14 : `0x1f32e` ; sous-mode 10 : `0x20926`).
//!
//! Un *script* (dans la ROM, pointeur long à `FF0EA2`) est une suite d'entrées de 32 octets :
//!
//! | Octet | Contenu |
//! |---|---|
//! | `+0` | opération : `0x80..` fin du script, `0` scène, `2` commande son puis scène (2 octets de plus), autre : présentation d'un personnage (3 octets) |
//! | `+1` | numéro de scène (`A5+0x34`) |
//! | `+2` | durée en images (mot) |
//! | `+4` | quatre joueurs de 6 octets : personnage (négatif : absent), variante, trois paramètres facultatifs |
//! | `+28` | un long recopié en `A5-0x5aa8` |
//!
//! Une scène de démonstration fait tourner le jeu lui-même : à chaque image l'interpréteur appelle le
//! gestionnaire de la phase courante (8 : entrée de scène ; 12 : le jeu, mené par l'IA) jusqu'à la fin de la
//! durée. Étapes de l'écran (`FF0E72`) : 0 début de script, 2 scène en cours, 6 présentation d'un personnage.

use cps2::Cps2Bus;
use m68k::Cpu;

use super::{mode, Block, Flow, Sequence};
use crate::effects_aids::decor_6;
use crate::families::creators::set_15_pur;
use crate::objects::Obj;
use crate::video::{effacer_couche, Couche};
use crate::mem::{rd16, rd32, rd8, set_low_word, wr16, wr32, wr8, A5};
use crate::objects::pools;
use crate::scene::Ctx;
use crate::Native;

/// Champs du bloc d'état de la machine à modes.
mod f {
    pub const SUB: u32 = 1;
    pub const STEP: u32 = 2;
    /// Sous-état de la présentation d'un personnage (octet, pas de 2).
    pub const SUBSTEP: u32 = 3;
    pub const TIMER: u32 = 8;
    pub const PRESENT_9: u32 = 9;
    /// Lien (mot) vers l'objet du portrait.
    pub const LINK: u32 = 0x10;
    /// Pointeur de script (long).
    pub const SCRIPT: u32 = 0x32;
    /// Fin de script demandée (interpréteur B).
    pub const ENDING: u32 = 0x3a;
    pub const PRESENT_KIND: u32 = 0x3c;
    pub const PRESENT_CHARACTER: u32 = 0x3d;
    /// Alterne entre les deux scripts (0 / 4).
    pub const ALTERNATE: u32 = 0x3e;
}

/// Variables de la RAM de travail (adresses absolues).
mod var {
    use crate::mem::A5;
    pub const SCREEN_WAIT: u32 = A5 - 0x7155;
    pub const YIELDED: u32 = A5 - 0x660e;
    pub const SKIP_YIELD: u32 = A5 - 0x65d8;
    pub const SAVED_TIME: u32 = A5 - 0x65ea;
    pub const SAVED_FRAMES: u32 = A5 - 0x65e8;
    pub const SAVED_RANK: u32 = A5 - 0x65e6;
    pub const SCRIPT_TAIL: u32 = A5 - 0x5aa8;
    pub const PLAYERS_PRESENT: u32 = A5 + 0x35;
    pub const SCENE: u32 = A5 + 0x34;
    pub const PHASE: u32 = A5 + 0x2c;
    pub const STEP: u32 = A5 + 0x2d;
    pub const VIDEO: u32 = A5 + 0x14;
    pub const VIDEO_FLAGS_B: u32 = A5 + 0x13;
    pub const FRAME_COUNTER: u32 = A5 + 0x6e;
    pub const DEMO_TIME: u32 = A5 + 0x6c;
    pub const RANK: u32 = A5 + 0x7f92;
    pub const FADE_STEPS: u32 = A5 + 0x48f;
    pub const FADE_PAGES: u32 = A5 + 0x48e;
    pub const FADE_DELAY: u32 = A5 + 0x490;
    /// Partie en cours ; mot de réglages.
    pub const GAME: u32 = A5 + 0x30;
    pub const DSW: u32 = A5 + 0x7fd0;
}

mod ext {
    /// Tâche de l'interface des joueurs, et tâche de fondu au noir.
    pub const PLAYER_UI: u32 = 0x150ac;
    pub const FADE_TASK_OUT: u32 = 0x14d0;
}

/// Tables en ROM (données, ou adresses lues par le code d'origine relativement au PC).
mod table {
    /// Pointeurs des deux scripts de départ, alternés (longs, lus par le PC).
    pub const SCRIPTS: u32 = 0x1f348;
    /// Routines des commandes son (longs, index en octets).
    pub const COMMANDS: u32 = 0x1f3aa;
    /// Gestionnaires de phase de la boucle A et de la boucle B.
    pub const PHASES_A: u32 = 0x1f990;
    pub const PHASES_B: u32 = 0x20a9c;
    /// Script de l'interpréteur B.
    pub const SCRIPT_B: u32 = 0x1f756;
    /// Tuiles de la présentation (deux mots par personnage), cadre, textes.
    pub const TILES: u32 = 0x1fd04;
    pub const PRESENT_DATA: u32 = 0x1febe;
    pub const FRAME_ATTR: u32 = 0x1ffa2;
    pub const BOX_ATTR: u32 = 0x1ffd0;
    pub const TEXT_1: u32 = 0x20070;
    pub const TEXT_2: u32 = 0x200ca;
}

/// Adresses de reprise et de retour de l'original.
mod at {
    pub const RETURN_A: u32 = 0x1f33a;
    pub const LOOP_A: u32 = 0x1f932;
    pub const AFTER_YIELD_A: u32 = 0x1f962;
    pub const AFTER_PHASE_A: u32 = 0x1f98c;
    pub const RETURN_B: u32 = 0x20932;
    pub const LOOP_B: u32 = 0x209d4;
    pub const AFTER_YIELD_B: u32 = 0x20a04;
    pub const AFTER_PHASE_B: u32 = 0x20a98;
}

const PLAYERS: u32 = A5 + 0x5de;
const PLAYER_STRIDE: u32 = 0x200;

/// Écran du sous-mode `sub`, s'il est réécrit ici.
pub fn screen(x: &mut Ctx, sub: u8) -> Option<Flow> {
    match sub {
        4 | 14 => Some(interpreter_a(x)),
        10 => Some(interpreter_b(x)),
        _ => None,
    }
}

fn prog32(x: &mut Ctx, addr: u32) -> u32 {
    (x.prog16(addr) as u32) << 16 | x.prog16(addr + 2) as u32
}

fn rd_state(x: &mut Ctx, off: u32) -> u8 {
    rd8(x.b, mode::BASE + off)
}

// ---------------------------------------------------------------------------------------------
// Interpréteur A (`0x1f32e`)

fn interpreter_a(x: &mut Ctx) -> Flow {
    let step = rd_state(x, f::STEP);
    x.c.d[0] = step as u32;
    x.push_return(Etape::ReturnA);
    match step {
        0 => start_a(x),
        2 => {
            crate::taches::invite::lancer(x.b);
            loop_a(x)
        }
        6 => presentation_step(x),
        // étape 4 (0x1fb20, fin de scène) : jamais écrite, la boucle y mène directement (`scene_end_a`) ; au-delà :
        // hors de la table 0x1f340 (code-mort.md, lot « sortie du Cpu »)
        _ => unreachable!("démonstration A : étape {step:#x} jamais produite"),
    }
}
crate::etapes! {
    /// Étapes (points de reprise) de la machine à états.
    pub enum Etape {
        ReturnA = at::RETURN_A => |x| {
            x.c.a[0] = mode::BASE;
            Flow::Done
        },
        LoopA = at::LOOP_A => loop_a,
        AfterYieldA = at::AFTER_YIELD_A => after_yield_a,
        SceneEndClearA = SCENE_END_CLEAR_A => |x| scene_end_clear(x, Etape::SceneEndNextA),
        SceneEndNextA = SCENE_END_NEXT_A => next_operation,
        EndClear = END_CLEAR => |x| {
            let v = rd16(x.b, var::VIDEO) & !0xe;
            wr16(x.b, var::VIDEO, v);
            clear_layers(x, &[Couche::Un, Couche::Deux, Couche::Trois]);
            clear_players_and_pools(x);
            x.wait(4, Etape::EndShow)
        },
        EndShow = END_SHOW => |x| {
            let v = rd16(x.b, var::VIDEO) | 0xe;
            wr16(x.b, var::VIDEO, v);
            Flow::Done
        },
        /// Le saut de `0x1fce4` à `0x1fc68` laisse l'adresse du `bsr` sur la pile : le décor est rendu deux fois.
        PresentAgain = 0x1fdba => p_render,
        P4Hide = P4_HIDE => |x| {
            let v = rd16(x.b, var::VIDEO) & !0xe;
            wr16(x.b, var::VIDEO, v);
            x.wait(1, Etape::P4Clear)
        },
        P4Clear = P4_CLEAR => |x| {
            clear_layers(x, &[Couche::Un, Couche::Deux, Couche::Trois]);
            next_operation(x)
        },
        ReturnB = at::RETURN_B => |x| {
            x.c.a[0] = mode::BASE;
            Flow::Done
        },
        LoopB = at::LOOP_B => loop_b,
        AfterYieldB = at::AFTER_YIELD_B => after_yield_b,
        SceneNextClearB = SCENE_NEXT_CLEAR_B => |x| scene_end_clear(x, Etape::SceneNextNextB),
        SceneNextNextB = SCENE_NEXT_NEXT_B => next_operation_b,
        EndingReset = ENDING_RESET => |x| {
            clear_lists_and_objects(x);
            x.wait(1, Etape::EndingClear)
        },
        EndingClear = ENDING_CLEAR => |x| {
            let v = rd16(x.b, var::VIDEO) & !0xc;
            wr16(x.b, var::VIDEO, v);
            clear_layers(x, &[Couche::Deux, Couche::Trois]);
            clear_players_and_pools(x);
            x.wait(4, Etape::EndingShow)
        },
        EndingShow = ENDING_SHOW => |x| {
            let v = rd16(x.b, var::VIDEO) | 0xc;
            wr16(x.b, var::VIDEO, v);
            crate::sound::service(x.b, crate::sound::Service::Ff05);
            Flow::Done
        },
        /// Retours des phases (`jsr (A0)` des deux boucles) : une phase peut céder la main, puis y revenir.
        AfterPhaseA = at::AFTER_PHASE_A => after_phase,
        AfterPhaseB = at::AFTER_PHASE_B => after_phase,
        /// Retour de la mise à jour du monde dans la phase 12 de la démonstration (`jsr 0x1c86e` en `0x1fa44`), quand
        /// un gestionnaire d'objet a fait céder la main à la tâche.
        ApresMonde = 0x1fa4a => demo_after_world,
    }
}

/// Étape 0 : choisit l'un des deux scripts (alternativement), garde l'horloge de démonstration.
/// Couches effacées (0xca8, 0xcc8, 0xce8).
fn clear_layers(x: &mut Ctx, couches: &[Couche]) {
    for &couche in couches {
        effacer_couche(x.b, couche);
    }
}

/// Joueurs effacés (0x2d9e) et pools remis à zéro (0x2062).
fn clear_players_and_pools(x: &mut Ctx) {
    crate::player::init::effacer_joueurs(x.c, x.b);
    for pool in pools::POOLS {
        pool.reset(x.b);
    }
}

/// Listes défaites et vidées, objets effacés (0x1f184, 0x1de7c, 0xc58).
fn clear_lists_and_objects(x: &mut Ctx) {
    crate::objects::lists::unlink_all(x.b);
    crate::sprites::vider_listes(x.b);
    crate::video::vider_objets(x.b);
}

fn start_a(x: &mut Ctx) -> Flow {
    crate::taches::invite::arreter(x.b);
    x.next_step();
    let which = rd_state(x, f::ALTERNATE);
    wr8(x.b, mode::BASE + f::ALTERNATE, which ^ 4);
    let script = prog32(x, table::SCRIPTS + which as u32);
    wr32(x.b, mode::BASE + f::SCRIPT, script);
    let t = rd16(x.b, var::DEMO_TIME);
    wr16(x.b, var::SAVED_TIME, t);
    let t = rd16(x.b, var::FRAME_COUNTER);
    wr16(x.b, var::SAVED_FRAMES, t);
    next_operation(x)
}

/// `0x1f376` : lit l'opération suivante du script.
fn next_operation(x: &mut Ctx) -> Flow {
    x.c.a[0] = mode::BASE;
    wr8(x.b, mode::BASE + 3, 0);
    let a1 = rd32(x.b, mode::BASE + f::SCRIPT);
    let op = rd8(x.b, a1);
    x.c.a[1] = a1 + 1;
    x.c.d[0] = (x.c.d[0] & !0xff) | op as u32;
    if op & 0x80 != 0 {
        return end_of_script(x);
    }
    match op {
        0 => scene_start(x),
        2 => {
            // commande son : l'octet suivant indexe la table des routines
            let index = rd8(x.b, a1 + 1) as u32;
            x.c.a[1] = a1 + 2;
            x.c.d[0] = index;
            let routine = prog32(x, table::COMMANDS + index);
            x.c.a[6] = routine;
            x.call(routine);
            x.c.a[1] += 1;
            scene_start(x)
        }
        _ => presentation(x),
    }
}

/// `0x1f818` : début d'une scène de démonstration d'après le script (A1 après l'opération).
fn scene_start(x: &mut Ctx) -> Flow {
    let mut a1 = x.c.a[1];
    wr8(x.b, mode::BASE + f::STEP, 2);
    let scene = rd8(x.b, a1);
    wr8(x.b, var::SCENE, scene);
    let duration = rd16(x.b, a1 + 1);
    wr16(x.b, mode::BASE + f::TIMER, duration);
    a1 += 3;
    wr16(x.b, var::FRAME_COUNTER, 0);
    wr16(x.b, var::DEMO_TIME, 0);
    let rank = rd16(x.b, var::RANK);
    wr16(x.b, var::SAVED_RANK, rank);
    wr16(x.b, var::RANK, 0xe);
    wr8(x.b, var::PLAYERS_PRESENT, 0);
    wr8(x.b, var::PHASE, 8);
    wr8(x.b, var::STEP, 0);
    crate::rng::seed(x.b);
    let mut a0 = PLAYERS;
    x.c.d[6] = 3;
    for d6 in (0..4u32).rev() {
        if rd8(x.b, a1) & 0x80 == 0 {
            let character = rd8(x.b, a1);
            wr8(x.b, a0 + 3, character);
            let variant = rd8(x.b, a1 + 1);
            wr8(x.b, a0 + 0x15d, variant);
            // la routine de la table 0x1f8a8 (indexée par le compteur, à rebours) est l'initialisation du joueur
            // de ce numéro (0x2e58... : `player::init`), D6/A0/A1 gardés autour de l'appel (`movem.l`)
            crate::player::init::initialiser(x.c, x.b, 3 - d6);
            apply_record(x, a0, a1);
        }
        a1 += 6;
        a0 += PLAYER_STRIDE;
    }
    x.c.d[6] = 0xffff;
    let v = rd32(x.b, a1);
    wr32(x.b, var::SCRIPT_TAIL, v);
    a1 += 4;
    x.c.a[1] = a1;
    wr32(x.b, mode::BASE + f::SCRIPT, a1);
    wr8(x.b, A5 + 0x31, 0);
    wr8(x.b, var::SCREEN_WAIT, 3);
    x.c.a[0] = ext::PLAYER_UI;
    set_low_word(&mut x.c.d[0], 0x30);
    crate::kernel::creer_tache(x.b, 0x30, ext::PLAYER_UI);
    Flow::Done
}

/// `0x1f8b8` : paramètres facultatifs d'un joueur de démonstration (octet négatif : non fourni).
fn apply_record(x: &mut Ctx, a0: u32, a1: u32) {
    let b2 = rd8(x.b, a1 + 2);
    if b2 & 0x80 == 0 {
        wr8(x.b, a0 + 0x10d, b2);
    }
    let b3 = rd8(x.b, a1 + 3);
    if b3 & 0x80 == 0 {
        wr8(x.b, a0 + 0x10e, b3);
    }
    let b4 = rd8(x.b, a1 + 4);
    if b4 & 0x80 == 0 {
        let slot = rd8(x.b, a1 + 1) as u32;
        wr8(x.b, a0 + 0x120 + ((slot << 4) & 0xffff), b4);
        for i in 0..10 {
            wr8(x.b, a0 + 0x150 + i, 9);
        }
    }
}

// --- la scène tourne ---------------------------------------------------------------------------

/// `0x1f932` : une image de la démonstration. Le jeu tourne deux pas par image : un pas sans céder
/// quand l'image vient de commencer (`SKIP_YIELD` ≤ 1), un pas après la cession.
fn loop_a(x: &mut Ctx) -> Flow {
    let n = rd16(x.b, var::FRAME_COUNTER).wrapping_add(1);
    wr16(x.b, var::FRAME_COUNTER, n);
    let skip = rd8(x.b, var::SKIP_YIELD) as i8;
    if rd8(x.b, var::SCREEN_WAIT) != 0 || rd8(x.b, var::YIELDED) != 0 || skip > 1 {
        wr8(x.b, var::SKIP_YIELD, 0);
        x.c.d[0] = 1;
        return x.wait(1, Etape::AfterYieldA);
    }
    wr8(x.b, var::SKIP_YIELD, skip.wrapping_add(3) as u8);
    after_yield_a(x)
}

fn after_yield_a(x: &mut Ctx) -> Flow {
    x.c.a[0] = mode::BASE;
    if rd8(x.b, var::PHASE) == 0xc {
        let left = rd16(x.b, mode::BASE + f::TIMER).wrapping_sub(1);
        wr16(x.b, mode::BASE + f::TIMER, left);
        if left == 0 {
            return scene_end_a(x);
        }
    }
    run_phase(x, table::PHASES_A, Etape::LoopA, Etape::AfterPhaseA)
}

/// Appelle le gestionnaire de la phase courante (`jsr (A0)` précédé du `pea` de la boucle) : il revient à `after`
/// (`after_phase`), qui revient en haut de la boucle ; une phase peut aussi revenir directement en haut de la boucle,
/// en dépilant le retour de son appel.
fn run_phase(x: &mut Ctx, phases: u32, loop_top: Etape, after: Etape) -> Flow {
    wr8(x.b, var::YIELDED, 0xff);
    x.push_return(loop_top);
    x.push_return(after);
    let phase = rd8(x.b, var::PHASE) as u32;
    let handler = prog32(x, phases + phase);
    x.c.d[0] = phase;
    x.c.a[0] = handler;
    // les deux tables n'ont que les phases 8 (entrée de scène) et 12 (le jeu) ; les autres entrées sont nulles
    // (code-mort.md, lot « sortie du Cpu »)
    match phase {
        8 => {
            crate::scene::entry::phase_8(x.c, x.b);
            Flow::Done
        }
        12 => demo_phase_12(x),
        _ => unreachable!("démonstration : phase {phase:#x} sans gestionnaire (table {phases:#x})"),
    }
}

/// `0x1f9b0` : après la phase, gère l'écran de chargement et vide les files de collision ; le `rts`
/// repart en haut de la boucle.
fn after_phase(x: &mut Ctx) -> Flow {
    let wait = rd8(x.b, var::SCREEN_WAIT);
    if wait == 0 {
        let v = rd16(x.b, var::VIDEO) | 0xe;
        wr16(x.b, var::VIDEO, v);
    } else {
        let left = wait - 1;
        wr8(x.b, var::SCREEN_WAIT, left);
        if left != 0 {
            let v = rd16(x.b, var::VIDEO) & !0xe;
            wr16(x.b, var::VIDEO, v);
            let longs = [0, 0x1800_0000, 0, 0x1800_0000, 0x8000_8000, 0];
            for (i, l) in longs.into_iter().enumerate() {
                wr32(x.b, 0x708000 + 4 * i as u32, l);
            }
            x.c.a[4] = 0x708018;
        }
    }
    reset_queues(x);
    Flow::Done
}

/// Les files de collision repartent à vide (comme `game::FRAME_END`).
fn reset_queues(x: &mut Ctx) {
    for queue in [&pools::QUEUE_PSHL, &pools::QUEUE_ESHL, &pools::QUEUE_ENEMY_BOSS, &pools::QUEUE_BOX, &pools::QUEUE_ITEM] {
        queue.reset(x.b);
    }
    x.c.a[0] = A5 + pools::QUEUE_ITEM.array;
}

// --- fin d'une scène, fin du script -------------------------------------------------------------

/// Remise à zéro commune à la fin d'une scène ou d'un script.
fn scene_cleanup_1(x: &mut Ctx) {
    wr8(x.b, A5 + 0x3d, 0);
    let v = rd8(x.b, var::VIDEO_FLAGS_B) & !1;
    wr8(x.b, var::VIDEO_FLAGS_B, v);
    wr8(x.b, A5 + 0x53c, 0);
}
/// `0x1fb20` : durée écoulée : tout est vidé, puis l'opération suivante.
fn scene_end_a(x: &mut Ctx) -> Flow {
    let rank = rd16(x.b, var::SAVED_RANK);
    wr16(x.b, var::RANK, rank);
    scene_cleanup_1(x);
    wr8(x.b, A5 + 0x50, 0);
    scene_end_common(x, Etape::SceneEndClearA)
}
/// Enchaîne le nettoyage commun puis la cession d'une image dont la reprise est `resume`.
fn scene_end_common(x: &mut Ctx, resume: Etape) -> Flow {
    stop_recorder(x.b);
    wr8(x.b, var::PHASE, 0);
    wr8(x.b, var::PLAYERS_PRESENT, 0);
    set_low_word(&mut x.c.d[0], 0x30);
    crate::kernel::tuer_tache(x.b, 0x30);
    clear_lists_and_objects(x);
    x.wait(1, resume)
}

/// Après la cession : vide les plans et les files, cède encore, puis lit l'opération suivante.
fn scene_end_clear(x: &mut Ctx, resume: Etape) -> Flow {
    let v = rd16(x.b, var::VIDEO) & !0xe;
    wr16(x.b, var::VIDEO, v);
    clear_layers(x, &[Couche::Un, Couche::Deux, Couche::Trois]);
    clear_players_and_pools(x);
    reset_queues(x);
    x.wait(1, resume)
}
const SCENE_END_CLEAR_A: u32 = 0x1fb6a;
const SCENE_END_NEXT_A: u32 = 0x1fbd2;

/// `0x1fa92` : fin du script : retour de l'horloge de démonstration, tout est vidé, écran suivant.
fn end_of_script(x: &mut Ctx) -> Flow {
    let s = rd8(x.b, mode::BASE + f::SUB).wrapping_add(2);
    wr8(x.b, mode::BASE + f::SUB, s);
    restore_clock(x);
    scene_cleanup_1(x);
    stop_recorder(x.b);
    wr8(x.b, var::PHASE, 0);
    wr16(x.b, mode::BASE + f::STEP, 0);
    wr8(x.b, var::PLAYERS_PRESENT, 0);
    set_low_word(&mut x.c.d[0], 0x30);
    crate::kernel::tuer_tache(x.b, 0x30);
    clear_lists_and_objects(x);
    x.wait(1, Etape::EndClear)
}
fn restore_clock(x: &mut Ctx) {
    let t = rd16(x.b, var::SAVED_TIME);
    wr16(x.b, var::DEMO_TIME, t);
    let t = rd16(x.b, var::SAVED_FRAMES);
    wr16(x.b, var::FRAME_COUNTER, t);
    let rank = rd16(x.b, var::SAVED_RANK);
    wr16(x.b, var::RANK, rank);
}
const END_CLEAR: u32 = 0x1faec;
const END_SHOW: u32 = 0x1fb18;

// --- présentation d'un personnage (opération autre que 0 et 2) -----------------------------------

/// `0x1fbd6` : mémorise les trois paramètres de la présentation et prépare l'affichage.
fn presentation(x: &mut Ctx) -> Flow {
    let mut a1 = x.c.a[1];
    crate::taches::invite::arreter(x.b);
    wr8(x.b, mode::BASE + f::STEP, 6);
    wr16(x.b, mode::BASE + f::TIMER, 0);
    let b = rd8(x.b, a1);
    wr8(x.b, mode::BASE + f::PRESENT_9, b);
    let b = rd8(x.b, a1 + 1);
    wr8(x.b, mode::BASE + f::PRESENT_KIND, b);
    let b = rd8(x.b, a1 + 2);
    wr8(x.b, mode::BASE + f::PRESENT_CHARACTER, b);
    a1 += 3;
    wr32(x.b, mode::BASE + f::SCRIPT, a1);
    x.c.a[1] = a1;
    wr8(x.b, A5 + 0x53c, 0);
    wr8(x.b, A5 + 0x36, 0);
    wr16(x.b, A5 + 0x506, 0);
    wr16(x.b, A5 + 0x50a, 512);
    wr16(x.b, A5 + 0x506, 0);
    wr16(x.b, A5 + 0x55a, 512);
    wr16(x.b, A5 + 0x4b6, 0);
    wr16(x.b, A5 + 0x4ba, 0);
    let v = rd8(x.b, var::VIDEO_FLAGS_B) & !1;
    wr8(x.b, var::VIDEO_FLAGS_B, v);
    wr16(x.b, var::VIDEO, 0x12c0);
    crate::palette::charger_page(x.b, 2, 84);
    crate::palette::charger_page(x.b, 3, 84);
    wr8(x.b, var::FADE_PAGES, 12);
    crate::palette::full_brightness(x.b);
    x.c.d[0] = 1;
    crate::taches::attendre_puis_revenir(x)
}

fn presentation_step(x: &mut Ctx) -> Flow {
    let substep = rd_state(x, f::SUBSTEP);
    x.c.d[0] = substep as u32;
    match substep {
        0 => p0_images(x),
        2 => p2_show(x),
        4 => p4_end(x),
        6 => p6_text(x),
        // la table 0x1fc72 a quatre entrées, toutes réécrites (code-mort.md, lot « sortie du Cpu »)
        _ => unreachable!("présentation : sous-étape {substep:#x} hors de la table 0x1fc72"),
    }
}

/// `0x1fc68` : fin commune de chaque sous-état : décor (SET, SOBJ) et sprites (`jmp 0x1de96`).
fn p_render(x: &mut Ctx) -> Flow {
    crate::dispatch::SET.update_sans_suite(x.c, x.b);
    crate::dispatch::SOBJ.update_sans_suite(x.c, x.b);
    crate::sprites::construire_68k(x.c, x.b);
    Flow::Done
}

fn next_substep(x: &mut Ctx) {
    let s = rd_state(x, f::SUBSTEP).wrapping_add(2);
    wr8(x.b, mode::BASE + f::SUBSTEP, s);
}

/// Sous-état 0 : charge les deux blocs de tuiles du personnage, crée les objets du cadre et du portrait.
fn p0_images(x: &mut Ctx) -> Flow {
    next_substep(x);
    let character = rd_state(x, f::PRESENT_CHARACTER) as u32 * 4;
    let (premier, second) = (x.prog16(table::TILES + character), x.prog16(table::TILES + 2 + character));
    let fin = crate::video::charger_grille(x.b, crate::video::Grille::Couche2, premier, 0x915000);
    crate::video::charger_grille(x.b, crate::video::Grille::Couche2, second, fin.destination);
    if rd_state(x, f::PRESENT_KIND) != 0 {
        crate::sound::service(x.b, crate::sound::Service::Ff07 { verrou: 0, ligne: 0 });
        if let Ok(Obj(set)) = set_15_pur(x.b, 4) {
            let character = rd_state(x, f::PRESENT_CHARACTER);
            wr8(x.b, set + 0x2f, character);
            wr16(x.b, mode::BASE + f::TIMER, 30);
        }
        return p_render(x);
    }
    let Ok(Obj(set)) = set_15_pur(x.b, 4) else {
        return p_render(x);
    };
    let character = rd_state(x, f::PRESENT_CHARACTER);
    wr8(x.b, set + 0x2f, character);
    portrait_object(x)
}

/// `0x1fce4` : objet du portrait ; en cas d'échec ou de succès l'original *saute* à `0x1fc68`.
fn portrait_object(x: &mut Ctx) -> Flow {
    if let Ok(Obj(o)) = decor_6(x.b, 15) {
        let character = rd_state(x, f::PRESENT_CHARACTER) >> 1;
        wr8(x.b, o + 0x2f, character);
        wr16(x.b, mode::BASE + f::LINK, o as u16);
    }
    p_render(x)
}

/// Sous-état 2 : allume les plans ; sous-état suivant 6 (texte) si la présentation a un texte, sinon 4.
fn p2_show(x: &mut Ctx) -> Flow {
    let v = rd16(x.b, var::VIDEO) | 0xe;
    wr16(x.b, var::VIDEO, v);
    let next = if rd_state(x, f::PRESENT_KIND) != 0 { 6 } else { 4 };
    wr8(x.b, mode::BASE + f::SUBSTEP, next);
    p_render(x)
}

/// Sous-état 6 : après la minuterie (mot `+8`, décompté par les objets), affiche le texte du personnage.
fn p6_text(x: &mut Ctx) -> Flow {
    let left = rd16(x.b, mode::BASE + f::TIMER).wrapping_sub(1);
    wr16(x.b, mode::BASE + f::TIMER, left);
    if left != 0 {
        return p_render(x);
    }
    wr8(x.b, mode::BASE + f::SUBSTEP, 4);
    wr16(x.b, mode::BASE + f::TIMER, 240);
    let entry = table::PRESENT_DATA + ((rd_state(x, f::PRESENT_CHARACTER) as u32) << 3);
    // choix par nombre de joueurs (A5+0x72 : 2, 4, 6...) d'une adresse de donnée (plain)
    let players = rd16(x.b, A5 + 0x72).wrapping_sub(2).wrapping_mul(2) as u32;
    let mut a1 = rd32(x.b, entry + players);
    let a4 = entry + 12;
    let (cx, cy) = (rd16(x.b, a1), rd16(x.b, a1 + 2));
    wr16(x.b, A5 + 0x5ca, cx);
    wr16(x.b, A5 + 0x5cc, cy);
    a1 += 4;
    wr32(x.b, A5 + 0x5c6, a1);
    let offset = rd16(x.b, a4) as i16 as i32 as u32;
    let src = 0x1fe3e_u32.wrapping_add(offset);
    for i in 0..8 {
        let v = rd32(x.b, src + 4 * i);
        wr32(x.b, 0x91c1e0 + 4 * i, v);
    }
    x.c.a[1] = 0x91c1e0;
    x.c.a[2] = src + 32;
    x.c.a[4] = a4 + 2;
    x.push_return(Etape::PresentAgain);
    text_screen(x);
    portrait_object(x)
}

/// Sous-état 4 : après la minuterie, ferme la présentation et lit l'opération suivante.
fn p4_end(x: &mut Ctx) -> Flow {
    let left = rd16(x.b, mode::BASE + f::TIMER).wrapping_sub(1);
    wr16(x.b, mode::BASE + f::TIMER, left);
    if left != 0 {
        return p_render(x);
    }
    let link = rd16(x.b, mode::BASE + f::LINK) as i16 as i32 as u32 & 0xff_ffff;
    x.c.a[1] = link;
    wr32(x.b, link + 4, 0x0600_0000);
    wr8(x.b, mode::BASE + f::STEP, 2);
    wr16(x.b, mode::BASE + 0x30, 300);
    crate::objects::lists::unlink_all(x.b);
    crate::sprites::vider_listes(x.b);
    for pool in pools::POOLS {
        pool.reset(x.b);
    }
    crate::video::vider_objets(x.b);
    x.wait(1, Etape::P4Hide)
}
const P4_HIDE: u32 = 0x1ff5e;
const P4_CLEAR: u32 = 0x1ff6c;

/// `0x1ffae` : cadre et textes de la présentation, écrits dans le plan 1 (GFX RAM `0x912000`).
fn text_screen(x: &mut Ctx) {
    let character = rd_state(x, f::PRESENT_CHARACTER) as u32;
    let frame = x.prog16(table::FRAME_ATTR + character) as i16 as i32 as u32;
    let a4 = 0x912000u32.wrapping_add(frame);
    let attr = x.prog16(table::BOX_ATTR + character);
    frame_box(x, a4, attr);
    text_lines(x, a4 + 128, table::TEXT_1, attr);
    let which = (character & 0xffff) * 2;
    let text = prog32(x, table::TEXT_2 + which);
    text_lines(x, a4 + 0x700, text, attr);
}

/// `0x1ffdc` : le cadre (huit tuiles de bord et de coin).
fn frame_box(x: &mut Ctx, a4: u32, attr: u16) {
    let put = |x: &mut Ctx, a: &mut u32, tile: u16| {
        wr16(x.b, *a, tile);
        wr16(x.b, *a + 2, attr);
        *a += 4;
    };
    let mut a1 = a4;
    put(x, &mut a1, 0x145);
    for _ in 0..10 {
        put(x, &mut a1, 0x14b);
    }
    put(x, &mut a1, 0x147);
    a1 += 124;
    for _ in 0..16 {
        put(x, &mut a1, 0x14a);
        a1 += 124;
    }
    a1 = a4 + 128;
    for _ in 0..16 {
        put(x, &mut a1, 0x149);
        a1 += 124;
    }
    put(x, &mut a1, 0x146);
    for _ in 0..10 {
        put(x, &mut a1, 0x14c);
    }
    put(x, &mut a1, 0x148);
}

/// `0x20044` : écrit un texte vertical (une tuile par ligne) ; `0x2f` passe à la colonne suivante,
/// `0x40` termine.
fn text_lines(x: &mut Ctx, start: u32, text: u32, attr: u16) {
    let mut a3 = start;
    let mut a2 = text;
    let mut a1 = a3 + 4;
    a3 = a1;
    loop {
        let c = rd8(x.b, a2);
        a2 += 1;
        match c {
            0x2f => {
                a1 = a3 + 4;
                a3 = a1;
            }
            0x40 => return,
            _ => {
                wr16(x.b, a1, c as u16);
                wr16(x.b, a1 + 2, attr);
                a1 += 4 + 124;
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Interpréteur B (`0x20926`)

fn interpreter_b(x: &mut Ctx) -> Flow {
    let step = rd_state(x, f::STEP);
    x.c.d[0] = step as u32;
    x.push_return(Etape::ReturnB);
    match step {
        0 => {
            crate::taches::invite::arreter(x.b);
            x.next_step();
            wr32(x.b, mode::BASE + f::SCRIPT, table::SCRIPT_B);
            let t = rd16(x.b, var::DEMO_TIME);
            wr16(x.b, var::SAVED_TIME, t);
            let t = rd16(x.b, var::FRAME_COUNTER);
            wr16(x.b, var::SAVED_FRAMES, t);
            wr8(x.b, mode::BASE + f::ENDING, 0);
            next_operation_b(x)
        }
        2 => {
            crate::taches::invite::lancer(x.b);
            loop_b(x)
        }
        4 => scene_next_b(x),
        _ => Flow::Done,
    }
}

/// `0x20960` : opération suivante : fin du script (octet négatif) ou scène.
fn next_operation_b(x: &mut Ctx) -> Flow {
    x.c.a[0] = mode::BASE;
    wr8(x.b, mode::BASE + 3, 0);
    let a1 = rd32(x.b, mode::BASE + f::SCRIPT);
    let op = rd8(x.b, a1);
    x.c.a[1] = a1 + 1;
    x.c.d[0] = (x.c.d[0] & !0xff) | op as u32;
    if op & 0x80 != 0 {
        return end_of_script(x);
    }
    scene_start(x)
}

fn loop_b(x: &mut Ctx) -> Flow {
    let n = rd16(x.b, var::FRAME_COUNTER).wrapping_add(1);
    wr16(x.b, var::FRAME_COUNTER, n);
    let skip = rd8(x.b, var::SKIP_YIELD) as i8;
    if rd8(x.b, var::SCREEN_WAIT) != 0 || rd8(x.b, var::YIELDED) != 0 || skip > 1 {
        wr8(x.b, var::SKIP_YIELD, 0);
        x.c.d[0] = 1;
        return x.wait(1, Etape::AfterYieldB);
    }
    wr8(x.b, var::SKIP_YIELD, skip.wrapping_add(3) as u8);
    after_yield_b(x)
}

fn after_yield_b(x: &mut Ctx) -> Flow {
    x.c.a[0] = mode::BASE;
    if rd8(x.b, var::SCENE) == 0x6f || rd8(x.b, var::PHASE) == 0xc {
        let timer = rd16(x.b, mode::BASE + f::TIMER);
        if timer == 0 {
            let ending = rd_state(x, f::ENDING);
            if ending & 0x80 != 0 {
                return ending_b(x);
            }
            if ending == 0 && rd8(x.b, var::FADE_STEPS) == 0 {
                return scene_next_b(x);
            }
        } else {
            let left = timer - 1;
            wr16(x.b, mode::BASE + f::TIMER, left);
            if left == 0 {
                let script = rd32(x.b, mode::BASE + f::SCRIPT);
                if rd16(x.b, script) == 0xffff {
                    wr8(x.b, mode::BASE + f::ENDING, 1);
                    let _ = decor_6(x.b, 8);
                } else {
                    x.c.a[0] = ext::FADE_TASK_OUT;
                    wr16(x.b, var::FADE_DELAY, 2);
                    wr8(x.b, var::FADE_PAGES, 15);
                    x.c.d[0] = 0x10;
                    crate::kernel::creer_tache(x.b, x.c.d[0], x.c.a[0]);
                    crate::sound::service(x.b, crate::sound::Service::Ff07 { verrou: 0, ligne: 0 });
                }
            }
        }
    }
    run_phase(x, table::PHASES_B, Etape::LoopB, Etape::AfterPhaseB)
}

/// `0x20abc` : scène suivante : comme `0x1fb20` mais efface aussi `A5+0x59`.
fn scene_next_b(x: &mut Ctx) -> Flow {
    let rank = rd16(x.b, var::SAVED_RANK);
    wr16(x.b, var::RANK, rank);
    scene_cleanup_1(x);
    wr8(x.b, A5 + 0x59, 0);
    wr8(x.b, A5 + 0x50, 0);
    scene_end_common(x, Etape::SceneNextClearB)
}
const SCENE_NEXT_CLEAR_B: u32 = 0x20b0a;
const SCENE_NEXT_NEXT_B: u32 = 0x20b72;

/// `0x20b86` : fin de la démonstration de jeu : écran de fin.
fn ending_b(x: &mut Ctx) -> Flow {
    let s = rd8(x.b, mode::BASE + f::SUB).wrapping_add(2);
    wr8(x.b, mode::BASE + f::SUB, s);
    restore_clock(x);
    scene_cleanup_1(x);
    stop_recorder(x.b);
    wr8(x.b, var::PHASE, 0);
    wr16(x.b, mode::BASE + f::STEP, 0);
    wr8(x.b, var::PLAYERS_PRESENT, 0);
    set_low_word(&mut x.c.d[0], 0x30);
    crate::kernel::tuer_tache(x.b, 0x30);
    effacer_couche(x.b, Couche::Un);
    crate::video::effacer_debut_couche1_68k(x.c, x.b);
    wr16(x.b, 0x91c41c, 0xf112);
    wr16(x.b, A5 + 0x4ba, 256);
    wr16(x.b, A5 + 0x4b6, 0);
    x.wait(1, Etape::EndingReset)
}
const ENDING_RESET: u32 = 0x20bec;
const ENDING_CLEAR: u32 = 0x20c06;
const ENDING_SHOW: u32 = 0x20c2c;

// ---------------------------------------------------------------------------------------------
// Entrées enregistrées (`0xe05ec`)

/// Enregistreur d'entrées des démonstrations (A5 - 0x5ab2...) : pointeur vers l'entrée courante (long), images
/// restantes de l'entrée (mot), et trois drapeaux : enregistrement, relecture, actif.
mod rec {
    use crate::mem::A5;
    pub const POINTER: u32 = A5 - 0x5ab2;
    pub const LEFT: u32 = A5 - 0x5aae;
    pub const PLAYING: u32 = A5 - 0x5aac;
    pub const RECORDING: u32 = A5 - 0x5aab;
    pub const ACTIVE: u32 = A5 - 0x5aaa;
    /// Une entrée : durée (mot), puis les trois octets d'entrée de chaque joueur.
    pub const ENTRY: u32 = 0xe;
    /// Dernière entrée possible.
    pub const LAST: u32 = 0x93fff2;
}

/// Les douze octets d'entrée des joueurs (`A5 + 0x64a + 0x200 k`, trois octets chacun), dans l'ordre d'une entrée.
fn input_byte(i: u32) -> u32 {
    A5 + 0x64a + 0x200 * (i / 3) + i % 3
}

/// `0xe05ec` : enregistre les entrées des joueurs (une entrée par changement, avec sa durée) ou les relit (une
/// partie en cours avec le bit 15 des réglages suspend la relecture).
fn demo_inputs(x: &mut Ctx) {
    if rd8(x.b, rec::ACTIVE) == 0 {
        return;
    }
    if rd8(x.b, rec::RECORDING) != 0 {
        record_inputs(x);
    } else if rd8(x.b, rec::PLAYING) != 0 {
        if rd8(x.b, var::GAME) != 0 && rd16(x.b, var::DSW) & 0x8000 != 0 {
            return;
        }
        let at = rd32(x.b, rec::POINTER);
        if at > rec::LAST {
            wr8(x.b, rec::PLAYING, 0);
            return;
        }
        for i in 0..12 {
            let v = rd8(x.b, at + 2 + i);
            wr8(x.b, input_byte(i), v);
        }
        let left = rd16(x.b, rec::LEFT);
        if left != 0 {
            wr16(x.b, rec::LEFT, left - 1);
        } else {
            let next = at + rec::ENTRY;
            wr32(x.b, rec::POINTER, next);
            let length = rd16(x.b, next);
            wr16(x.b, rec::LEFT, length);
        }
    }
}

/// `0xe0538` (entrée de scène, hors partie) : prépare l'enregistreur. Réglage bit 13 : graine du hasard et
/// compteurs d'images à zéro, puis relecture active ; bit 15 : enregistrement en 0x930000 ; sinon relecture
/// (inactive) de la démonstration pointée par `A5-0x5aa8`.
pub fn reset_recorder(c: &mut Cpu, b: &mut Cps2Bus) {
    const DEMO: u32 = A5 - 0x5aa8;
    const RECORD_AREA: u32 = 0x93_0000;
    if rd8(b, var::GAME) != 0 {
        return;
    }
    wr16(b, rec::LEFT, 0);
    wr8(b, rec::RECORDING, 0);
    wr8(b, rec::PLAYING, 0);
    wr8(b, rec::ACTIVE, 0);
    let dsw = rd16(b, var::DSW);
    c.d[5] = c.d[5] & 0xffff_0000 | dsw as u32;
    if dsw & 0x2000 != 0 {
        crate::rng::seed(b);
        wr16(b, A5 + 0x6c, 0);
        wr16(b, A5 + 0x6e, 0);
        wr16(b, A5 + 0x7f92, 0xe);
        wr8(b, rec::ACTIVE, 1);
    } else if dsw & 0x8000 != 0 {
        wr32(b, rec::POINTER, RECORD_AREA);
        for off in [0, 4, 8] {
            wr32(b, RECORD_AREA + off, 0xffff_ffff);
        }
        wr16(b, RECORD_AREA + 0xc, 0xffff);
        wr8(b, rec::RECORDING, 1);
        return;
    }
    let demo = rd32(b, DEMO);
    c.a[0] = demo;
    wr32(b, rec::POINTER, demo);
    let length = rd16(b, demo);
    wr16(b, rec::LEFT, length);
    wr8(b, rec::PLAYING, 1);
}

/// `0xe05d6` : arrête l'enregistreur.
pub fn stop_recorder(b: &mut Cps2Bus) {
    wr16(b, rec::LEFT, 0);
    for flag in [rec::RECORDING, rec::PLAYING, rec::ACTIVE] {
        wr8(b, flag, 0);
    }
    wr32(b, A5 - 0x5aa8, 0);
}

pub const RECORDER_RESET: Native = Native {
    entry: 0xe0538,
    name: "demo_enregistreur_depart",
    run: crate::Corps::Processeur(|c, b| {
        reset_recorder(c, b);
        crate::mem::rts(c, b);
    }),
    touches_sound: false,
    outputs: crate::regs::d(5) | crate::regs::a(0),
};

pub const RECORDER_STOP: Native = Native {
    entry: 0xe05d6,
    name: "demo_enregistreur_arret",
    run: crate::Corps::Processeur(|c, b| {
        stop_recorder(b);
        crate::mem::rts(c, b);
    }),
    touches_sound: false,
    outputs: 0,
};

/// Enregistrement : l'entrée courante dure une image de plus tant que les entrées ne changent pas (et que sa durée
/// tient sur 16 bits) ; sinon sa durée est écrite et une nouvelle entrée commence.
fn record_inputs(x: &mut Ctx) {
    let length = rd16(x.b, rec::LEFT);
    let at = rd32(x.b, rec::POINTER);
    if length != 0xffff && (0..12).all(|i| rd8(x.b, input_byte(i)) == rd8(x.b, at + 2 + i)) {
        wr16(x.b, rec::LEFT, length + 1);
        return;
    }
    wr16(x.b, at, length);
    wr16(x.b, rec::LEFT, 0);
    let next = at + rec::ENTRY;
    wr32(x.b, rec::POINTER, next);
    if next > rec::LAST {
        wr8(x.b, rec::RECORDING, 0);
        return;
    }
    for i in 0..12 {
        let v = rd8(x.b, input_byte(i));
        wr8(x.b, next + 2 + i, v);
    }
}

// ---------------------------------------------------------------------------------------------
// Phase 12 de la démonstration (`0x1fa38`)

/// Le jeu de la démonstration : entrées de l'IA, monde, jauges, collisions, script d'apparition.
pub const DEMO_PHASE_12: Native = Native {
    entry: 0x1fa38,
    name: "demo_phase_12",
    run: crate::Corps::Processeur(|c, b| {
        let mut x = Ctx::new(c, b);
        if demo_phase_12(&mut x) == Flow::Done {
            crate::mem::rts(x.c, x.b);
        }
    }),
    touches_sound: false,
    outputs: 0,
};

/// `0x1fa38` : entrées de l'IA, puis la mise à jour du monde (un gestionnaire d'objet peut y faire céder la main à la
/// tâche : elle reprendra à [`Etape::ApresMonde`]).
fn demo_phase_12(x: &mut Ctx) -> Flow {
    x.c.a[5] = A5;
    // Glitch d'origine : D0-D4 de la copie des entrées (0x2aec) sont relus par des gestionnaires (voir game.rs)
    crate::inputs::recopier_68k(x.c, x.b);
    demo_inputs(x);
    x.push_return(Etape::ApresMonde);
    if crate::game::update_world(x.c, x.b) == Flow::Yield {
        return Flow::Yield;
    }
    x.drop_return();
    demo_after_world(x)
}

/// `0x1fa4a` : comme la phase 12 du jeu (`game::apres_le_monde`) ; l'original finit par `jmp 0x28d8` (la fin de
/// partie, puis son `rts`).
fn demo_after_world(x: &mut Ctx) -> Flow {
    crate::game::apres_le_monde(x.c, x.b);
    Flow::Done
}

pub const NATIVES: &[Native] = &[
    DEMO_PHASE_12,
    RECORDER_RESET,
    RECORDER_STOP,
];
