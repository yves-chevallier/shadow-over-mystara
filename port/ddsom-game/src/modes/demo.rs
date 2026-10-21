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

use super::{mode, Block, Flow, Sequence};
use crate::mem::{rd16, rd32, rd8, set_low_word, wr16, wr32, wr8, A5};
use crate::objects::pools;
use crate::resume;
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
    /// Arrête (et lance) la tâche du texte « INSERT COIN » clignotant (`taches/invite.rs`).
    pub const INVITE_STOP: u32 = 0xe03c6;
    pub const INVITE_START: u32 = 0xe03e4;
    pub const SOUND_E05D6: u32 = 0xe05d6;
    pub const SOUND_E0A82: u32 = 0xe0a82;
    pub const SOUND_E0A10: u32 = 0xe0a10;
    pub const KILL_TASK: u32 = 0xa14;
    pub const RESET_PLAYERS: u32 = 0x1f184;
    pub const SPRITES_RESET: u32 = 0x1de7c;
    pub const SPRITES: u32 = 0x1de96;
    pub const CLEAR_SPRITES: u32 = 0xc58;
    pub const CLEAR_SCROLL_1: u32 = 0xca8;
    pub const CLEAR_SCROLL_2: u32 = 0xcc8;
    pub const CLEAR_SCROLL_3: u32 = 0xce8;
    pub const POOLS: u32 = 0x2062;
    pub const SERVICE_2D9E: u32 = 0x2d9e;
    pub const SCENE_START_HOOK: u32 = 0x19a80;
    pub const TASK_CREATE: u32 = 0x9b6;
    pub const PLAYER_UI: u32 = 0x150ac;
    pub const PLAYER_PREPARE: u32 = 0x3525c;
    pub const PALETTE_SET: u32 = 0x1a67e;
    pub const PALETTE_FADE: u32 = 0x1a8b2;
    pub const BRIGHTNESS_WHITE: u32 = 0x1686;
    pub const SET: u32 = 0x39e86;
    pub const SOBJ: u32 = 0x39f82;
    pub const TILE_BLOCK: u32 = 0x26d4;
    pub const SET_VARIANT: u32 = 0xce00a;
    pub const SOBJ_VARIANT: u32 = 0xddeaa;
    pub const FADE_TASK_OUT: u32 = 0x14d0;
    /// Gestionnaire de la phase 12 de la démonstration, et ses étapes.
    pub const INPUTS: u32 = 0x2aec;
    pub const WORLD_UPDATE: u32 = 0x1c86e;
    pub const GAUGE: u32 = 0xb3e;
    pub const COLLISIONS: u32 = 0x260ee;
    pub const STEP_19E20: u32 = 0x19e20;
    pub const SCRIPT_READER: u32 = 0xf2e5c;
    pub const RANK_TICK: u32 = 0x1f27c;
    pub const END_FRAME: u32 = 0x28d8;
}

/// Tables en ROM (données, ou adresses lues par le code d'origine relativement au PC).
mod table {
    /// Pointeurs des deux scripts de départ, alternés (longs, lus par le PC).
    pub const SCRIPTS: u32 = 0x1f348;
    /// Routines des commandes son (longs, index en octets).
    pub const COMMANDS: u32 = 0x1f3aa;
    /// Routines d'initialisation des quatre types de joueur.
    pub const PLAYER_INIT: u32 = 0x1f8a8;
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
    x.push_return(at::RETURN_A);
    match step {
        0 => start_a(x),
        2 => {
            x.call(ext::INVITE_START);
            loop_a(x)
        }
        6 => presentation_step(x),
        _ => x.jump_to(0x1fb20),
    }
}
resume!(R_RETURN_A, at::RETURN_A, |x| {
    x.c.a[0] = mode::BASE;
    Flow::Done
});

/// Étape 0 : choisit l'un des deux scripts (alternativement), garde l'horloge de démonstration.
fn start_a(x: &mut Ctx) -> Flow {
    x.call(ext::INVITE_STOP);
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
    x.call(ext::SCENE_START_HOOK);
    let mut a0 = PLAYERS;
    x.c.d[6] = 3;
    for d6 in (0..4u32).rev() {
        if rd8(x.b, a1) & 0x80 == 0 {
            let character = rd8(x.b, a1);
            wr8(x.b, a0 + 3, character);
            let variant = rd8(x.b, a1 + 1);
            wr8(x.b, a0 + 0x15d, variant);
            let routine = prog32(x, table::PLAYER_INIT + 4 * d6);
            // movem.l d6/a0-a1 : la routine voit D6 multiplié par 4
            let saved = (x.c.d[6], x.c.a[0], x.c.a[1]);
            x.c.d[6] = 4 * d6;
            x.c.a[0] = a0;
            x.c.a[1] = routine;
            x.call(routine);
            (x.c.d[6], x.c.a[0], x.c.a[1]) = saved;
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
    x.call(ext::TASK_CREATE);
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
        return x.wait(1, at::AFTER_YIELD_A);
    }
    wr8(x.b, var::SKIP_YIELD, skip.wrapping_add(3) as u8);
    after_yield_a(x)
}
resume!(R_LOOP_A, at::LOOP_A, loop_a);
resume!(R_AFTER_YIELD_A, at::AFTER_YIELD_A, after_yield_a);

fn after_yield_a(x: &mut Ctx) -> Flow {
    x.c.a[0] = mode::BASE;
    if rd8(x.b, var::PHASE) == 0xc {
        let left = rd16(x.b, mode::BASE + f::TIMER).wrapping_sub(1);
        wr16(x.b, mode::BASE + f::TIMER, left);
        if left == 0 {
            return scene_end_a(x);
        }
    }
    run_phase(x, table::PHASES_A, at::LOOP_A, at::AFTER_PHASE_A)
}

/// Appelle le gestionnaire de la phase courante (`jsr (A0)` précédé du `pea` de la boucle).
fn run_phase(x: &mut Ctx, phases: u32, loop_top: u32, after: u32) -> Flow {
    wr8(x.b, var::YIELDED, 0xff);
    x.push_return(loop_top);
    let phase = rd8(x.b, var::PHASE) as u32;
    let handler = prog32(x, phases + phase);
    x.c.d[0] = phase;
    x.c.a[0] = handler;
    crate::call_at(x.c, x.b, handler, after);
    after_phase(x)
}
resume!(R_AFTER_PHASE_A, at::AFTER_PHASE_A, after_phase);
resume!(R_AFTER_PHASE_B, at::AFTER_PHASE_B, after_phase);

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
    crate::mem::rts(x.c, x.b);
    Flow::Yield
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
    scene_end_common(x, SCENE_END_CLEAR_A)
}
/// Enchaîne le nettoyage commun puis la cession d'une image dont la reprise est `resume`.
fn scene_end_common(x: &mut Ctx, resume: u32) -> Flow {
    x.call(ext::SOUND_E05D6);
    wr8(x.b, var::PHASE, 0);
    wr8(x.b, var::PLAYERS_PRESENT, 0);
    set_low_word(&mut x.c.d[0], 0x30);
    x.call(ext::KILL_TASK);
    for f in [ext::RESET_PLAYERS, ext::SPRITES_RESET, ext::CLEAR_SPRITES] {
        x.call(f);
    }
    x.wait(1, resume)
}

/// Après la cession : vide les plans et les files, cède encore, puis lit l'opération suivante.
fn scene_end_clear(x: &mut Ctx, resume: u32) -> Flow {
    let v = rd16(x.b, var::VIDEO) & !0xe;
    wr16(x.b, var::VIDEO, v);
    for f in [ext::CLEAR_SCROLL_1, ext::CLEAR_SCROLL_2, ext::CLEAR_SCROLL_3, ext::SERVICE_2D9E, ext::POOLS] {
        x.call(f);
    }
    reset_queues(x);
    x.wait(1, resume)
}
const SCENE_END_CLEAR_A: u32 = 0x1fb6a;
const SCENE_END_NEXT_A: u32 = 0x1fbd2;
resume!(R_SCENE_END_CLEAR_A, SCENE_END_CLEAR_A, |x| scene_end_clear(x, SCENE_END_NEXT_A));
resume!(R_SCENE_END_NEXT_A, SCENE_END_NEXT_A, next_operation);

/// `0x1fa92` : fin du script : retour de l'horloge de démonstration, tout est vidé, écran suivant.
fn end_of_script(x: &mut Ctx) -> Flow {
    let s = rd8(x.b, mode::BASE + f::SUB).wrapping_add(2);
    wr8(x.b, mode::BASE + f::SUB, s);
    restore_clock(x);
    scene_cleanup_1(x);
    x.call(ext::SOUND_E05D6);
    wr8(x.b, var::PHASE, 0);
    wr16(x.b, mode::BASE + f::STEP, 0);
    wr8(x.b, var::PLAYERS_PRESENT, 0);
    set_low_word(&mut x.c.d[0], 0x30);
    x.call(ext::KILL_TASK);
    for f in [ext::RESET_PLAYERS, ext::SPRITES_RESET, ext::CLEAR_SPRITES] {
        x.call(f);
    }
    x.wait(1, END_CLEAR)
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
resume!(R_END_CLEAR, END_CLEAR, |x| {
    let v = rd16(x.b, var::VIDEO) & !0xe;
    wr16(x.b, var::VIDEO, v);
    for f in [ext::CLEAR_SCROLL_1, ext::CLEAR_SCROLL_2, ext::CLEAR_SCROLL_3, ext::SERVICE_2D9E, ext::POOLS] {
        x.call(f);
    }
    x.wait(4, END_SHOW)
});
const END_SHOW: u32 = 0x1fb18;
resume!(R_END_SHOW, END_SHOW, |x| {
    let v = rd16(x.b, var::VIDEO) | 0xe;
    wr16(x.b, var::VIDEO, v);
    Flow::Done
});

// --- présentation d'un personnage (opération autre que 0 et 2) -----------------------------------

/// `0x1fbd6` : mémorise les trois paramètres de la présentation et prépare l'affichage.
fn presentation(x: &mut Ctx) -> Flow {
    let mut a1 = x.c.a[1];
    x.call(ext::INVITE_STOP);
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
    set_low_word(&mut x.c.d[0], 84);
    x.call(ext::PALETTE_SET);
    set_low_word(&mut x.c.d[0], 84);
    x.call(ext::PALETTE_FADE);
    wr8(x.b, var::FADE_PAGES, 12);
    x.call(ext::BRIGHTNESS_WHITE);
    x.c.d[0] = 1;
    x.jump_to(super::sub::WAIT)
}

fn presentation_step(x: &mut Ctx) -> Flow {
    let substep = rd_state(x, f::SUBSTEP);
    x.c.d[0] = substep as u32;
    match substep {
        0 => p0_images(x),
        2 => p2_show(x),
        4 => p4_end(x),
        6 => p6_text(x),
        _ => x.jump_to(0x1fc5a),
    }
}

/// `0x1fc68` : fin commune de chaque sous-état : décor (SET, SOBJ) et sprites.
fn p_render(x: &mut Ctx) -> Flow {
    x.call(ext::SET);
    x.call(ext::SOBJ);
    x.jump_to(ext::SPRITES)
}

fn next_substep(x: &mut Ctx) {
    let s = rd_state(x, f::SUBSTEP).wrapping_add(2);
    wr8(x.b, mode::BASE + f::SUBSTEP, s);
}

/// Sous-état 0 : charge les deux blocs de tuiles du personnage, crée les objets du cadre et du portrait.
fn p0_images(x: &mut Ctx) -> Flow {
    next_substep(x);
    let character = rd_state(x, f::PRESENT_CHARACTER) as u32 * 4;
    x.c.d[0] = x.prog16(table::TILES + character) as u32;
    x.c.a[1] = 0x915000;
    x.call(ext::TILE_BLOCK);
    x.c.d[0] = x.prog16(table::TILES + 2 + character) as u32;
    x.call(ext::TILE_BLOCK);
    if rd_state(x, f::PRESENT_KIND) != 0 {
        x.call_d0(ext::SOUND_E0A82, 0);
        x.call_d0(ext::SET_VARIANT, 4);
        if x.ok() {
            let character = rd_state(x, f::PRESENT_CHARACTER);
            wr8(x.b, x.c.a[1] + 0x2f, character);
            wr16(x.b, mode::BASE + f::TIMER, 30);
        }
        return p_render(x);
    }
    x.call_d0(ext::SET_VARIANT, 4);
    if !x.ok() {
        return p_render(x);
    }
    let character = rd_state(x, f::PRESENT_CHARACTER);
    wr8(x.b, x.c.a[1] + 0x2f, character);
    portrait_object(x)
}

/// `0x1fce4` : objet du portrait ; en cas d'échec ou de succès l'original *saute* à `0x1fc68`.
fn portrait_object(x: &mut Ctx) -> Flow {
    x.call_d0(ext::SOBJ_VARIANT, 15);
    if x.ok() {
        let character = rd_state(x, f::PRESENT_CHARACTER) >> 1;
        let o = x.c.a[1];
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
    x.push_return(0x1fdba);
    text_screen(x);
    portrait_object(x)
}
// Le saut de `0x1fce4` à `0x1fc68` laisse l'adresse du `bsr` sur la pile : le décor est rendu deux fois.
resume!(R_PRESENT_AGAIN, 0x1fdba, p_render);

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
    for f in [ext::RESET_PLAYERS, ext::SPRITES_RESET, ext::POOLS, ext::CLEAR_SPRITES] {
        x.call(f);
    }
    x.wait(1, P4_HIDE)
}
const P4_HIDE: u32 = 0x1ff5e;
resume!(R_P4_HIDE, P4_HIDE, |x| {
    let v = rd16(x.b, var::VIDEO) & !0xe;
    wr16(x.b, var::VIDEO, v);
    x.wait(1, P4_CLEAR)
});
const P4_CLEAR: u32 = 0x1ff6c;
resume!(R_P4_CLEAR, P4_CLEAR, |x| {
    for f in [ext::CLEAR_SCROLL_1, ext::CLEAR_SCROLL_2, ext::CLEAR_SCROLL_3] {
        x.call(f);
    }
    next_operation(x)
});

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
    x.push_return(at::RETURN_B);
    match step {
        0 => {
            x.call(ext::INVITE_STOP);
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
            x.call(ext::INVITE_START);
            loop_b(x)
        }
        4 => scene_next_b(x),
        _ => Flow::Done,
    }
}
resume!(R_RETURN_B, at::RETURN_B, |x| {
    x.c.a[0] = mode::BASE;
    Flow::Done
});

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
        return x.wait(1, at::AFTER_YIELD_B);
    }
    wr8(x.b, var::SKIP_YIELD, skip.wrapping_add(3) as u8);
    after_yield_b(x)
}
resume!(R_LOOP_B, at::LOOP_B, loop_b);
resume!(R_AFTER_YIELD_B, at::AFTER_YIELD_B, after_yield_b);

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
                    x.call_d0(ext::SOBJ_VARIANT, 8);
                } else {
                    x.c.a[0] = ext::FADE_TASK_OUT;
                    wr16(x.b, var::FADE_DELAY, 2);
                    wr8(x.b, var::FADE_PAGES, 15);
                    x.c.d[0] = 0x10;
                    x.call(ext::TASK_CREATE);
                    x.call_d0(ext::SOUND_E0A82, 0);
                }
            }
        }
    }
    run_phase(x, table::PHASES_B, at::LOOP_B, at::AFTER_PHASE_B)
}

/// `0x20abc` : scène suivante : comme `0x1fb20` mais efface aussi `A5+0x59`.
fn scene_next_b(x: &mut Ctx) -> Flow {
    let rank = rd16(x.b, var::SAVED_RANK);
    wr16(x.b, var::RANK, rank);
    scene_cleanup_1(x);
    wr8(x.b, A5 + 0x59, 0);
    wr8(x.b, A5 + 0x50, 0);
    scene_end_common(x, SCENE_NEXT_CLEAR_B)
}
const SCENE_NEXT_CLEAR_B: u32 = 0x20b0a;
const SCENE_NEXT_NEXT_B: u32 = 0x20b72;
resume!(R_SCENE_NEXT_CLEAR_B, SCENE_NEXT_CLEAR_B, |x| scene_end_clear(x, SCENE_NEXT_NEXT_B));
resume!(R_SCENE_NEXT_NEXT_B, SCENE_NEXT_NEXT_B, next_operation_b);

/// `0x20b86` : fin de la démonstration de jeu : écran de fin.
fn ending_b(x: &mut Ctx) -> Flow {
    let s = rd8(x.b, mode::BASE + f::SUB).wrapping_add(2);
    wr8(x.b, mode::BASE + f::SUB, s);
    restore_clock(x);
    scene_cleanup_1(x);
    x.call(ext::SOUND_E05D6);
    wr8(x.b, var::PHASE, 0);
    wr16(x.b, mode::BASE + f::STEP, 0);
    wr8(x.b, var::PLAYERS_PRESENT, 0);
    set_low_word(&mut x.c.d[0], 0x30);
    x.call(ext::KILL_TASK);
    x.call(ext::CLEAR_SCROLL_1);
    x.call(ext::PLAYER_PREPARE);
    wr16(x.b, 0x91c41c, 0xf112);
    wr16(x.b, A5 + 0x4ba, 256);
    wr16(x.b, A5 + 0x4b6, 0);
    x.wait(1, ENDING_RESET)
}
const ENDING_RESET: u32 = 0x20bec;
resume!(R_ENDING_RESET, ENDING_RESET, |x| {
    for f in [ext::RESET_PLAYERS, ext::SPRITES_RESET, ext::CLEAR_SPRITES] {
        x.call(f);
    }
    x.wait(1, ENDING_CLEAR)
});
const ENDING_CLEAR: u32 = 0x20c06;
resume!(R_ENDING_CLEAR, ENDING_CLEAR, |x| {
    let v = rd16(x.b, var::VIDEO) & !0xc;
    wr16(x.b, var::VIDEO, v);
    for f in [ext::CLEAR_SCROLL_2, ext::CLEAR_SCROLL_3, ext::SERVICE_2D9E, ext::POOLS] {
        x.call(f);
    }
    x.wait(4, ENDING_SHOW)
});
const ENDING_SHOW: u32 = 0x20c2c;
resume!(R_ENDING_SHOW, ENDING_SHOW, |x| {
    let v = rd16(x.b, var::VIDEO) | 0xc;
    wr16(x.b, var::VIDEO, v);
    x.jump_to(ext::SOUND_E0A10)
});

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
    cost: |_, _| 100,
    run: |c, b| {
        let mut x = Ctx::new(c, b);
        x.c.a[5] = A5;
        x.call(ext::INPUTS);
        demo_inputs(&mut x);
        x.call(ext::WORLD_UPDATE);
        x.c.d[1] = 0x54_0000;
        x.call(ext::GAUGE);
        x.call(ext::SPRITES);
        x.c.d[1] = 0x48_0002;
        x.call(ext::GAUGE);
        x.call(ext::COLLISIONS);
        x.c.d[1] = 0x4f_001e;
        x.call(ext::GAUGE);
        for f in [ext::STEP_19E20, ext::SCRIPT_READER, ext::RANK_TICK] {
            x.call(f);
        }
        x.jump_to(ext::END_FRAME);
    },
    touches_sound: false,
    outputs: 0,
    exact_cost: false,
};

pub const NATIVES: &[Native] = &[
    DEMO_PHASE_12,
    R_RETURN_A,
    R_LOOP_A,
    R_AFTER_YIELD_A,
    R_AFTER_PHASE_A,
    R_SCENE_END_CLEAR_A,
    R_SCENE_END_NEXT_A,
    R_END_CLEAR,
    R_END_SHOW,
    R_PRESENT_AGAIN,
    R_P4_HIDE,
    R_P4_CLEAR,
    R_RETURN_B,
    R_LOOP_B,
    R_AFTER_YIELD_B,
    R_AFTER_PHASE_B,
    R_SCENE_NEXT_CLEAR_B,
    R_SCENE_NEXT_NEXT_B,
    R_ENDING_RESET,
    R_ENDING_CLEAR,
    R_ENDING_SHOW,
];
