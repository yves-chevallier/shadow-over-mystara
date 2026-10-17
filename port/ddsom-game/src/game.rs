//! Boucle de partie (W1) : mise à jour du monde (`0x1c86e`), fin d'image (`0x1c4c4`) et phase 0
//! (`0x1c522`, initialisation de la partie). Voir `docs/boucle.md`.
//!
//! La tâche de partie (`0x1c45a`) boucle à l'infini : une image = un appel de la phase courante
//! (`A5+0x2c`), puis la remise à vide des files de collision, puis une cession (`0xa40`). Les phases
//! ne cèdent pas elles-mêmes : chacune fait un pas par image et revient. Seule la boucle principale
//! (qui cède) reste en 68000 ; les routines ci-dessous sont celles qui s'exécutent d'une traite.
//!
//! Durée : ces routines appellent des sous-routines par `call` (qui ajoutent la leur) ; leur durée
//! propre est celle du code d'origine le long du chemin pris, retrouvée avec `Chemin`.

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::Cpu;

use crate::chemin::Chemin;
use crate::dispatch::{self, Class};
use crate::garde::Garde;
use crate::mem::{rd16, rd8, rts, wr16, wr8, A5};
use crate::objects::pools::{self, Queue};
use crate::objects::Obj;
use crate::timing::path_cycles;
use crate::{call, Native};

/// Variables de la RAM de travail (offsets depuis A5).
mod ram {
    /// Phase de partie (octet, pas de 4), étape, sous-étape.
    pub const PHASE: u32 = 0x2c;
    pub const STEP: u32 = 0x2d;
    /// Mode « continue » : la phase 0 saute à sa dernière étape.
    pub const CONTINUE: u32 = 0x25;
    /// Cinématique en cours (≠ 0).
    pub const CINEMATIC: u32 = 0x44;
    /// Numéro de scène.
    pub const SCENE: u32 = 0x34;
    /// Compte à rebours de l'étape (mot).
    pub const TIMER: u32 = 0x38;
    /// Partie active.
    pub const ACTIVE: u32 = 0x30;
    /// Décalage vertical du plan 2 (mot), utilisé pendant l'initialisation.
    pub const SCROLL2_Y: u32 = 0x4ba;
}

/// Sous-routines non réécrites ou déjà natives, appelées par `call`.
mod ext {
    /// Jauge de charge : D1 = marqueur écrit dans la première ligne du plan 2.
    pub const GAUGE: u32 = 0xb3e;
    /// Mise à jour des quatre joueurs.
    pub const PLAYERS: u32 = 0x35292;
    /// Sous-objets des joueurs (armes avant et arrière).
    pub const PLAYER_PARTS: u32 = 0x392e4;
    /// Caméras et défilement.
    pub const CAMERAS: u32 = 0x33e20;
    /// Construction de la liste de sprites.
    pub const SPRITES: u32 = 0x1de96;
    pub const SPRITES_RESET: u32 = 0x1de7c;
    pub const POOLS_INIT_SET: u32 = 0x2134;
    pub const POOLS_INIT_SOBJ: u32 = 0x21bc;
    pub const ALLOC_SET: u32 = 0x2348;
    pub const PLAYERS_PREPARE_A: u32 = 0x35288;
    pub const PLAYERS_PREPARE_B: u32 = 0x3525c;
    /// Lance un script d'animation de palette (A4 = script).
    pub const PALETTE_SCRIPT: u32 = 0x1324;
    pub const PHASE0_END_FADE: u32 = 0x1cf3c;
    pub const RANK_LOAD: u32 = 0x1f1b8;
    pub const CLEAR_STATUS: u32 = 0xdabba;
    pub const TASK_CREATE: u32 = 0x9b6;
}

// ---------------------------------------------------------------------------------------------
// Mise à jour du monde (0x1c86e)

/// Une étape de la mise à jour du monde.
#[derive(Clone, Copy)]
enum Step {
    /// Jauge de charge, avec son marqueur (D1).
    Gauge(u32),
    /// Les quatre joueurs, puis leurs sous-objets.
    Players,
    PlayerParts,
    /// Dispatch d'une classe d'objets.
    Objs(&'static Class),
    /// Caméras et défilement.
    Cameras,
}
use Step::{Cameras, Gauge, Objs, PlayerParts, Players};

const G_PLAYER: u32 = 0x50_0001;
const G_ENEMY: u32 = 0x45_0002;
const G_DECOR: u32 = 0x53_0003;

/// Ordre normal : joueurs et leurs tirs, puis boss, ennemis, tirs ennemis, effets, puis décors,
/// caisses, objets, caméras, objets système.
const ORDER_NORMAL: &[Step] = &[
    Gauge(G_PLAYER), Players, PlayerParts, Objs(&dispatch::PSHL), Objs(&dispatch::PEFF),
    Gauge(G_ENEMY), Objs(&dispatch::BOSS), Objs(&dispatch::ENEMY), Objs(&dispatch::ESHL), Objs(&dispatch::EFFCT),
    Gauge(G_DECOR), Objs(&dispatch::SET), Objs(&dispatch::BOX), Objs(&dispatch::ITEM), Cameras,
    Objs(&dispatch::SOBJ),
];

/// Cinématique (`A5+0x44` ≠ 0) : les effets passent en premier.
const ORDER_CINEMATIC: &[Step] = &[
    Gauge(G_ENEMY), Objs(&dispatch::EFFCT),
    Gauge(G_PLAYER), Players, PlayerParts, Objs(&dispatch::PSHL), Objs(&dispatch::PEFF),
    Gauge(G_ENEMY), Objs(&dispatch::BOSS), Objs(&dispatch::ENEMY), Objs(&dispatch::ESHL),
    Gauge(G_DECOR), Objs(&dispatch::SET), Objs(&dispatch::BOX), Objs(&dispatch::ITEM), Cameras,
    Objs(&dispatch::SOBJ),
];

/// Scènes 0x45 à 0x52 : ni joueurs, ni ennemis ; effets, décors, caméras, effets joueur, objets système.
const ORDER_REDUCED: &[Step] = &[
    Gauge(G_ENEMY), Objs(&dispatch::EFFCT), Objs(&dispatch::SET), Cameras, Objs(&dispatch::PEFF),
    Objs(&dispatch::SOBJ),
];

/// Scène 8 : comme l'ordre normal, mais les effets passent après les caméras.
const ORDER_SCENE_8: &[Step] = &[
    Gauge(G_PLAYER), Players, PlayerParts, Objs(&dispatch::PSHL), Objs(&dispatch::PEFF),
    Gauge(G_ENEMY), Objs(&dispatch::BOSS), Objs(&dispatch::ENEMY), Objs(&dispatch::ESHL),
    Gauge(G_DECOR), Objs(&dispatch::SET), Objs(&dispatch::BOX), Objs(&dispatch::ITEM), Cameras,
    Objs(&dispatch::EFFCT), Objs(&dispatch::SOBJ),
];

/// Choix de l'ordre ; `taken` note les décisions des branchements de l'original, dans l'ordre.
fn world_order(b: &mut Cps2Bus, mut taken: impl FnMut(bool) -> bool) -> &'static [Step] {
    // les comparaisons de l'original sont signées sur un octet
    let scene = rd8(b, A5 + ram::SCENE) as i8;
    if taken(rd8(b, A5 + ram::CINEMATIC) != 0) {
        ORDER_CINEMATIC
    } else if taken(scene >= 0x53) {
        ORDER_NORMAL
    } else if taken(scene >= 0x45) {
        ORDER_REDUCED
    } else if taken(scene == 8) {
        ORDER_SCENE_8
    } else {
        ORDER_NORMAL
    }
}

/// Les sous-routines appelées par `Step`, pour ignorer leur durée dans le parcours du code d'origine.
const WORLD_EXTERNALS: [u32; 14] = [
    ext::GAUGE, ext::PLAYERS, ext::PLAYER_PARTS, ext::CAMERAS,
    0x398e2, 0x3996c, 0x39a52, 0x39aec, 0x39b70, 0x39cba, 0x39e50, 0x39e86, 0x39f34, 0x39f82,
];

/// Met à jour tous les objets : joueurs, puis dix classes d'objets dans l'ordre de la scène.
fn world_update(c: &mut Cpu, b: &mut Cps2Bus) {
    c.cycles += world_own_cycles(b);
    for &step in world_order(b, |t| t) {
        match step {
            Gauge(marker) => {
                c.d[1] = marker;
                call(c, b, ext::GAUGE);
            }
            Players => call(c, b, ext::PLAYERS),
            PlayerParts => call(c, b, ext::PLAYER_PARTS),
            Objs(class) => dispatch::run_class(c, b, class),
            Cameras => call(c, b, ext::CAMERAS),
        }
    }
    rts(c, b);
}

/// Durée propre de la routine d'origine (choix de l'ordre et appels, sans leur contenu).
fn world_own_cycles(b: &mut Cps2Bus) -> u64 {
    let mut chemin = Chemin::default();
    world_order(b, |t| {
        chemin.pris(t);
        t
    });
    chemin.duree_hors(b, 0x1c86e, None, &WORLD_EXTERNALS)
}

static GARDE_WORLD: Garde = Garde::new();

pub const WORLD_UPDATE: Native = Native {
    entry: 0x1c86e,
    name: "world_update",
    cost: |c, b| GARDE_WORLD.cout(c, b, world_update),
    run: |c, b| GARDE_WORLD.executer(c, b, world_update),
    touches_sound: false,
    outputs: 0, // les appelants enchaînent sur d'autres appels
    exact_cost: true,
};

// ---------------------------------------------------------------------------------------------
// Fin d'image (0x1c4c4)

/// Files de candidats aux collisions, dans l'ordre où l'original les remet à vide.
const QUEUES: [&Queue; 5] =
    [&pools::QUEUE_PSHL, &pools::QUEUE_ESHL, &pools::QUEUE_ENEMY_BOSS, &pools::QUEUE_BOX, &pools::QUEUE_ITEM];

/// Fin de l'image, quand la phase revient : les files de collision repartent à vide. L'adresse de
/// retour dépilée par le `rts` est celle du début de la boucle de partie (`0x1c468`).
fn frame_end(c: &mut Cpu, b: &mut Cps2Bus) {
    for q in QUEUES {
        q.reset(b);
    }
    c.a[0] = A5 + pools::QUEUE_ITEM.array;
    rts(c, b);
}

pub const FRAME_END: Native = Native {
    entry: 0x1c4c4,
    name: "game_frame_end",
    cost: |_, b| path_cycles(b, 0x1c4c4, 0, |_| false),
    run: frame_end,
    touches_sound: false,
    outputs: crate::regs::a(0),
    exact_cost: true,
};

// ---------------------------------------------------------------------------------------------
// Phase 0 : initialisation de la partie (0x1c522)

/// Scripts d'animation de palette passés à `0x1324` (dans le code, après la table des étapes).
const PALETTE_SCRIPTS: [u32; 3] = [0x1c546, 0x1c54a, 0x1c54e];

/// Variante des trois objets SET créés à l'étape 0, et leur indice d'animation de palette (`+0x2f`).
const SET_PALETTES: [(u8, u8); 3] = [(0x3e, 0), (0x3f, 1), (0x40, 2)];
const SET_PALETTE_INDEX: u32 = 0x2f;

/// Octets de la RAM hors de la page A5+-0x7fff, remis à zéro par la phase.
const CLEARED_A: u32 = 0xff_2472; // A5 - 0x5b8e
const CLEARED_B: u32 = 0xff_19f0; // A5 - 0x6610

/// Table des étapes de la phase 0 : un `bra.w` de 4 octets par étape.
const PHASE0_STEPS: u32 = 0x1c532;

/// Tâche d'interface des joueurs créée à la fin de la phase.
const PLAYER_UI_SLOT: u32 = 0x30;
const PLAYER_UI_TASK: u32 = 0x150ac;

/// Octets de `A5` remis à zéro par la sortie de phase : drapeau, score.
const EXIT_CLEARED_BYTES: [u32; 2] = [0x37, 0x5d];

const PHASE0_EXTERNALS: [u32; 14] = [
    ext::POOLS_INIT_SET, ext::ALLOC_SET, ext::PLAYERS_PREPARE_A, ext::PLAYERS_PREPARE_B, ext::PALETTE_SCRIPT,
    0x39f82, 0x39e86, ext::SPRITES, ext::PHASE0_END_FADE, ext::SPRITES_RESET, ext::POOLS_INIT_SOBJ,
    ext::RANK_LOAD, ext::CLEAR_STATUS, ext::TASK_CREATE,
];

fn next_step(b: &mut Cps2Bus) {
    let step = rd8(b, A5 + ram::STEP).wrapping_add(4);
    wr8(b, A5 + ram::STEP, step);
}

fn run_script(c: &mut Cpu, b: &mut Cps2Bus, script: u32) {
    c.a[4] = script;
    call(c, b, ext::PALETTE_SCRIPT);
}

/// Étape 0 : trois objets SET (animations de palette), prépare les joueurs.
fn create_palette_objects(c: &mut Cpu, b: &mut Cps2Bus) {
    next_step(b);
    wr16(b, A5 + ram::TIMER, 0x40);
    wr8(b, A5 + ram::ACTIVE, 1);
    wr8(b, CLEARED_A, 0);
    call(c, b, ext::POOLS_INIT_SET);
    for (variant, index) in SET_PALETTES {
        // l'original n'examine pas la retenue : le pool vient d'être vidé
        call(c, b, ext::ALLOC_SET);
        let set = Obj(c.a[1]);
        set.set_u8(b, Obj::ACTIVE, 1);
        set.set_u8(b, Obj::KIND, 0);
        set.set_u8(b, Obj::VARIANT, variant);
        set.set_u8(b, SET_PALETTE_INDEX, index);
    }
    call(c, b, ext::PLAYERS_PREPARE_A);
    call(c, b, ext::PLAYERS_PREPARE_B);
    let y = rd16(b, A5 + ram::SCROLL2_Y).wrapping_add(0x100);
    wr16(b, A5 + ram::SCROLL2_Y, y);
}

/// Étape 4 : lance les deux premières animations de palette (puis tombe dans l'étape 8).
fn start_palette_scripts(c: &mut Cpu, b: &mut Cps2Bus) {
    next_step(b);
    run_script(c, b, PALETTE_SCRIPTS[0]);
    run_script(c, b, PALETTE_SCRIPTS[1]);
    let y = rd16(b, A5 + ram::SCROLL2_Y).wrapping_sub(0x100);
    wr16(b, A5 + ram::SCROLL2_Y, y);
}

/// Début des étapes 8 et 12 : objets système et décors animés, sprites, puis décompte (mot `TIMER`).
/// Renvoie vrai quand le décompte est écoulé ; sinon l'original revient aussitôt (`bpl`).
fn animate(c: &mut Cpu, b: &mut Cps2Bus, chemin: &mut Chemin) -> bool {
    dispatch::run_class(c, b, &dispatch::SOBJ);
    dispatch::run_class(c, b, &dispatch::SET);
    call(c, b, ext::SPRITES);
    let left = rd16(b, A5 + ram::TIMER).wrapping_sub(1);
    wr16(b, A5 + ram::TIMER, left);
    let running = left as i16 >= 0;
    chemin.pris(running);
    !running
}

/// Étape 8 : quand le décompte est écoulé, un objet SET (type 0xc) apparaît et l'étape suivante commence.
fn wait_then_spawn(c: &mut Cpu, b: &mut Cps2Bus, chemin: &mut Chemin) {
    if !animate(c, b, chemin) {
        return;
    }
    call(c, b, ext::ALLOC_SET);
    let failed = c.sr & 1 != 0;
    chemin.pris(failed);
    if failed {
        return;
    }
    let set = Obj(c.a[1]);
    set.set_u8(b, Obj::ACTIVE, 1);
    set.set_u8(b, Obj::KIND, 0xc);
    next_step(b);
    wr16(b, A5 + ram::TIMER, 0x20);
}

/// Étape 12 : quand le décompte est écoulé, fondu de fin et troisième animation de palette, puis
/// l'étape 16.
fn wait_then_fade(c: &mut Cpu, b: &mut Cps2Bus, chemin: &mut Chemin) {
    if !animate(c, b, chemin) {
        return;
    }
    call(c, b, ext::PHASE0_END_FADE);
    run_script(c, b, PALETTE_SCRIPTS[2]);
    reset_pools_and_leave(c, b);
}

/// Étape 16 : remise à neuf des pools, puis sortie de la phase.
fn reset_pools_and_leave(c: &mut Cpu, b: &mut Cps2Bus) {
    wr8(b, A5 + ram::STEP, 0);
    call(c, b, ext::SPRITES_RESET);
    call(c, b, ext::POOLS_INIT_SOBJ);
    call(c, b, ext::POOLS_INIT_SET);
    leave(c, b);
}

/// Sortie de la phase 0 (aussi en mode continue) : phase suivante, premier niveau, tâche d'interface.
fn leave(c: &mut Cpu, b: &mut Cps2Bus) {
    let phase = rd8(b, A5 + ram::PHASE).wrapping_add(4);
    wr8(b, A5 + ram::PHASE, phase);
    wr8(b, CLEARED_B, 0);
    wr8(b, A5 + ram::ACTIVE, 1);
    call(c, b, ext::RANK_LOAD);
    wr8(b, A5 + ram::SCENE, 1);
    wr8(b, A5 + EXIT_CLEARED_BYTES[0], 0);
    wr16(b, A5 + ram::TIMER, 0);
    wr16(b, A5 + ram::TIMER + 2, 0);
    wr8(b, A5 + EXIT_CLEARED_BYTES[1], 0);
    call(c, b, ext::CLEAR_STATUS);
    // move.w #slot,D0w : les drapeaux, sauvegardés par la création de tâche (trap), en dépendent
    c.d[0] = c.d[0] & 0xffff_0000 | PLAYER_UI_SLOT;
    c.flags_logic(PLAYER_UI_SLOT, Size::Word);
    c.a[0] = PLAYER_UI_TASK;
    call(c, b, ext::TASK_CREATE);
}

/// Phase 0 : un pas par image. L'étape (`A5+0x2d` : 0, 4, 8, 12, 16) choisit la suite ; les étapes
/// 4, 12 et 16 tombent dans la suivante.
fn phase0(c: &mut Cpu, b: &mut Cps2Bus) {
    // première partie du code d'origine : de l'entrée au saut de table ; seconde : de l'étape choisie au retour
    let (mut entry, mut body) = (Chemin::default(), Chemin::default());
    let continued = rd8(b, A5 + ram::CONTINUE) != 0;
    entry.pris(continued);
    let mut step_address = None;
    if continued {
        leave(c, b);
    } else {
        let step = rd8(b, A5 + ram::STEP);
        c.d[0] = c.d[0] & 0xffff_ff00 | step as u32;
        step_address = Some(PHASE0_STEPS + step as u32);
        match step {
            0 => create_palette_objects(c, b),
            4 => {
                start_palette_scripts(c, b);
                wait_then_spawn(c, b, &mut body);
            }
            8 => wait_then_spawn(c, b, &mut body),
            12 => wait_then_fade(c, b, &mut body),
            16 => reset_pools_and_leave(c, b),
            _ => panic!("phase 0 : étape {step} inconnue"),
        }
    }
    c.cycles += entry.duree_hors(b, 0x1c522, None, &PHASE0_EXTERNALS);
    if let Some(address) = step_address {
        c.cycles += body.duree_hors(b, address, None, &PHASE0_EXTERNALS);
    }
    rts(c, b);
}

static GARDE_PHASE0: Garde = Garde::new();

pub const PHASE_0: Native = Native {
    entry: 0x1c522,
    name: "game_phase0",
    cost: |c, b| GARDE_PHASE0.cout(c, b, phase0),
    run: |c, b| GARDE_PHASE0.executer(c, b, phase0),
    touches_sound: false,
    outputs: 0,
    exact_cost: true,
};

// ---------------------------------------------------------------------------------------------
// Phase 12 : le jeu proprement dit (0x1c6a0)

/// Compte à rebours de la partie en BCD : minutes (`+0x38`), secondes, images (0 à 59).
const CLOCK: u32 = 0x38;
/// Compteur de la partie (mot, plafonné à 0xffff).
const GAME_FRAMES: u32 = 0x54;
/// Boutons pressés à cette image (+1) et à la précédente (+7), pour le saut de niveau de débogage.
const BUTTONS_NOW: u32 = 0x01;
const BUTTONS_BEFORE: u32 = 0x07;
/// Octets joueur remis à zéro lors d'un saut de niveau : `+2` du statut de chaque joueur et de chaque
/// arme (deux mots par joueur).
const PLAYER_RESET: [u32; 8] = [0x5e2, 0x7e2, 0x9e2, 0xbe2, 0x602, 0x802, 0xa02, 0xc02];
const BIT_X: u16 = 0x10;
/// Mot de la ROM dont le bit 15 active le saut de niveau de débogage.
const DEBUG_SWITCH: u32 = 0xc0;
/// Dernier numéro de scène.
const LAST_SCENE: i8 = 0x6c;

const PHASE12_EXTERNALS: [u32; 9] = [
    0x2aec, 0x1c86e, ext::GAUGE, ext::SPRITES, 0x260ee, 0x19e20, 0xf2e5c, 0x1f27c, 0x28d8,
];

/// `d - s - X` en BCD sur un octet : résultat et retenue (celle de `sbcd`).
fn sbcd(d: u8, s: u8, x: bool) -> (u8, bool) {
    let (d, s, x) = (d as i32, s as i32, x as i32);
    let unadj = d - s - x;
    let mut adj = 0;
    if (d & 0x0f) - (s & 0x0f) - x < 0 {
        adj |= 0x06;
    }
    let borrow = unadj < 0 || unadj - adj < 0;
    if unadj < 0 {
        adj |= 0x60;
    }
    ((unadj - adj) as u8, borrow)
}

/// Retire une image au compte à rebours (secondes et minutes en cascade, de 59 à 00) : la
/// cascade s'arrête dès qu'un octet reste positif.
fn tick_clock(c: &mut Cpu, b: &mut Cps2Bus, chemin: &mut Chemin) {
    let mut x = c.sr & BIT_X != 0;
    for (i, off) in [CLOCK + 2, CLOCK + 1, CLOCK].into_iter().enumerate() {
        let (value, borrow) = sbcd(rd8(b, A5 + off), 1, x);
        x = borrow;
        c.sr = c.sr & !BIT_X | if x { BIT_X } else { 0 };
        wr8(b, A5 + off, value);
        let positive = value as i8 >= 0;
        chemin.pris(positive);
        if positive {
            return;
        }
        if i < 2 {
            wr8(b, A5 + off, 0x59);
        } else {
            wr16(b, A5 + CLOCK, 0);
            wr16(b, A5 + CLOCK + 2, 0);
        }
    }
}

/// Saut de niveau de débogage : tous les joueurs remis à zéro, retour à la phase 8 (entrée de scène).
fn change_scene(b: &mut Cps2Bus, next: bool, chemin: &mut Chemin) {
    for off in PLAYER_RESET {
        wr16(b, A5 + off, 0);
        wr16(b, A5 + off + 2, 0);
    }
    wr8(b, A5 + 0x50, 0);
    let scene = rd8(b, A5 + ram::SCENE);
    let scene = if next {
        let scene = scene.wrapping_add(1);
        chemin.pris(scene as i8 <= LAST_SCENE);
        if scene as i8 <= LAST_SCENE { scene } else { 1 }
    } else {
        let scene = scene.wrapping_sub(1);
        chemin.pris(scene as i8 > 0);
        if scene as i8 > 0 { scene } else { LAST_SCENE as u8 }
    };
    wr8(b, A5 + ram::SCENE, scene);
    wr8(b, A5 + ram::PHASE, 8);
    wr8(b, A5 + ram::STEP, 0);
    wr16(b, A5 + 0x2e, 0);
}

fn phase12(c: &mut Cpu, b: &mut Cps2Bus) {
    let mut chemin = Chemin::default();
    // compteur de la partie, jusqu'à 0xffff
    let saturated = rd16(b, A5 + GAME_FRAMES) == 0xffff;
    chemin.pris(saturated);
    if !saturated {
        let frames = rd16(b, A5 + GAME_FRAMES) + 1;
        wr16(b, A5 + GAME_FRAMES, frames);
        c.sr &= !BIT_X; // addq.w : pas de retenue
    }
    // compte à rebours
    let running = rd16(b, A5 + CLOCK) != 0;
    chemin.pris(!running);
    if running {
        tick_clock(c, b, &mut chemin);
    }
    // le monde
    call(c, b, 0x2aec);
    call(c, b, 0x1c86e);
    for (marker, routine) in [(0x54_0000, ext::SPRITES), (0x48_0002, 0x260ee), (0x4f_001e, 0x19e20)] {
        c.d[1] = marker;
        call(c, b, ext::GAUGE);
        call(c, b, routine);
    }
    for routine in [0xf2e5c, 0x1f27c, 0x28d8] {
        call(c, b, routine);
    }
    // saut de niveau de débogage : actif si le bit 15 d'un mot de la ROM est levé
    let debug = rd16(b, DEBUG_SWITCH) & 0x8000 != 0;
    chemin.pris(!debug);
    if debug {
        let pressed = !rd8(b, A5 + BUTTONS_BEFORE) & rd8(b, A5 + BUTTONS_NOW);
        let next = pressed & 0x90 == 0x90;
        chemin.pris(!next);
        if next {
            change_scene(b, true, &mut chemin);
        } else {
            let previous = pressed & 0x60 == 0x60;
            chemin.pris(!previous);
            if previous {
                change_scene(b, false, &mut chemin);
            }
        }
    }
    c.cycles += chemin.duree_hors(b, 0x1c6a0, None, &PHASE12_EXTERNALS);
    rts(c, b);
}

static GARDE_PHASE12: Garde = Garde::new();

pub const PHASE_12: Native = Native {
    entry: 0x1c6a0,
    name: "game_phase12",
    cost: |c, b| GARDE_PHASE12.cout(c, b, phase12),
    run: |c, b| GARDE_PHASE12.executer(c, b, phase12),
    touches_sound: false,
    outputs: 0,
    exact_cost: true,
};

/// Routines natives de la boucle de partie.
pub const NATIVES: &[Native] = &[WORLD_UPDATE, FRAME_END, PHASE_0, PHASE_12];

#[cfg(test)]
mod tests {
    use super::*;

    /// Chaque classe est mise à jour une seule fois par image, dans tous les ordres complets.
    #[test]
    fn ordres_complets_sans_doublon() {
        for order in [ORDER_NORMAL, ORDER_CINEMATIC, ORDER_SCENE_8] {
            let mut entries: Vec<u32> =
                order.iter().filter_map(|s| if let Objs(class) = s { Some(class.entry) } else { None }).collect();
            entries.sort();
            entries.dedup();
            assert_eq!(entries.len(), 10);
        }
    }
}
