//! Boucle de partie (W1) : mise à jour du monde (`0x1c86e`), fin d'image (`0x1c4c4`), phase 0
//! (`0x1c522`, initialisation de la partie) et phase 12 (`0x1c6a0`, le jeu). Voir `docs/boucle.md`.
//!
//! La tâche de partie (`0x1c45a`) boucle à l'infini : une image = un appel de la phase courante
//! (`A5+0x2c`), puis la remise à vide des files de collision, puis une cession (`0xa40`). Les phases
//! ne cèdent pas elles-mêmes : chacune fait un pas par image et revient. Seule la boucle principale
//! (qui cède) reste en 68000.

use cps2::Cps2Bus;
use m68k::Cpu;

use crate::dispatch::{self, Class};
use crate::mem::{rd16, rd8, rts, wr16, wr32, wr8, A5};
use crate::objects::pools::{self, Queue};
use crate::objects::Obj;
use crate::{call, call_at, regs, Native};

/// Variables de la RAM de travail (offsets depuis A5).
mod ram {
    /// Phase de partie (octet, pas de 4), étape, sous-étape.
    pub const PHASE: u32 = 0x2c;
    pub const STEP: u32 = 0x2d;
    pub const SUBSTEP: u32 = 0x2e;
    /// Mode « continue » : la phase 0 saute à sa dernière étape.
    pub const CONTINUE: u32 = 0x25;
    /// Partie active.
    pub const ACTIVE: u32 = 0x30;
    /// Numéro de scène (octet signé).
    pub const SCENE: u32 = 0x34;
    /// Cinématique en cours (≠ 0).
    pub const CINEMATIC: u32 = 0x44;
    /// Compte à rebours de l'étape (mot) ; en jeu, horloge BCD sur trois octets.
    pub const TIMER: u32 = 0x38;
    /// Défilement vertical du plan 2 (mot).
    pub const SCROLL2_Y: u32 = 0x4ba;
}

/// Sous-routines 68000 (ou déjà natives) appelées par `call`.
mod ext {
    /// Jauge de charge : D1 = marqueur écrit dans la première ligne du plan 2.
    pub const GAUGE: u32 = 0xb3e;
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
    pub const INPUT_HOOK: u32 = 0x2aec;
    pub const COLLISIONS: u32 = 0x260ee;
    pub const WORLD_EXTRA: u32 = 0x19e20;
    pub const WORLD_EXTRA_2: u32 = 0xf2e5c;
    pub const RANK_TICK: u32 = 0x1f27c;
    pub const END_OF_FRAME: u32 = 0x28d8;
}

// ---------------------------------------------------------------------------------------------
// Mise à jour du monde (0x1c86e)

/// Une étape de la mise à jour du monde.
#[derive(Clone, Copy)]
enum Step {
    /// Jauge de charge, avec son marqueur (D1).
    Gauge(u32),
    Players,
    PlayerParts,
    /// Dispatch d'une classe d'objets.
    Objs(&'static Class),
    Cameras,
}
use Step::{Cameras, Gauge, Objs, PlayerParts, Players};

const G_PLAYER: u32 = 0x50_0001;
const G_ENEMY: u32 = 0x45_0002;
const G_DECOR: u32 = 0x53_0003;

/// Une étape et l'adresse de son appel dans le code d'origine (`jsr`, 6 octets ; le dernier appel de
/// chaque ordre est un `jmp`). Les appels Rust empilent les mêmes adresses de retour que l'original
/// (voir `call_at`).
type Order = &'static [(u32, Step)];

/// Ordre normal (0x1c894) : joueurs et leurs tirs, puis boss, ennemis, tirs ennemis, effets, puis
/// décors, caisses, objets, caméras, objets système.
const ORDER_NORMAL: Order = &[
    (0x1c89a, Gauge(G_PLAYER)), (0x1c8a0, Players), (0x1c8a6, PlayerParts),
    (0x1c8ac, Objs(&dispatch::PSHL)), (0x1c8b2, Objs(&dispatch::PEFF)),
    (0x1c8be, Gauge(G_ENEMY)), (0x1c8c4, Objs(&dispatch::BOSS)), (0x1c8ca, Objs(&dispatch::ENEMY)),
    (0x1c8d0, Objs(&dispatch::ESHL)), (0x1c8d6, Objs(&dispatch::EFFCT)),
    (0x1c8e2, Gauge(G_DECOR)), (0x1c8e8, Objs(&dispatch::SET)), (0x1c8ee, Objs(&dispatch::BOX)),
    (0x1c8f4, Objs(&dispatch::ITEM)), (0x1c8fa, Cameras),
    (0x1c900, Objs(&dispatch::SOBJ)),
];

/// Cinématique (`A5+0x44` ≠ 0, 0x1c906) : les effets passent en premier.
const ORDER_CINEMATIC: Order = &[
    (0x1c90c, Gauge(G_ENEMY)), (0x1c912, Objs(&dispatch::EFFCT)),
    (0x1c91e, Gauge(G_PLAYER)), (0x1c924, Players), (0x1c92a, PlayerParts),
    (0x1c930, Objs(&dispatch::PSHL)), (0x1c936, Objs(&dispatch::PEFF)),
    (0x1c942, Gauge(G_ENEMY)), (0x1c948, Objs(&dispatch::BOSS)), (0x1c94e, Objs(&dispatch::ENEMY)),
    (0x1c954, Objs(&dispatch::ESHL)),
    (0x1c960, Gauge(G_DECOR)), (0x1c966, Objs(&dispatch::SET)), (0x1c96c, Objs(&dispatch::BOX)),
    (0x1c972, Objs(&dispatch::ITEM)), (0x1c978, Cameras),
    (0x1c97e, Objs(&dispatch::SOBJ)),
];

/// Scènes 0x45 à 0x52 (0x1c984) : ni joueurs, ni ennemis.
const ORDER_REDUCED: Order = &[
    (0x1c98a, Gauge(G_ENEMY)), (0x1c990, Objs(&dispatch::EFFCT)), (0x1c996, Objs(&dispatch::SET)),
    (0x1c99c, Cameras), (0x1c9a2, Objs(&dispatch::PEFF)),
    (0x1c9a8, Objs(&dispatch::SOBJ)),
];

/// Scène 8 (0x1c9ae) : comme l'ordre normal, mais les effets passent après les caméras.
const ORDER_SCENE_8: Order = &[
    (0x1c9b4, Gauge(G_PLAYER)), (0x1c9ba, Players), (0x1c9c0, PlayerParts),
    (0x1c9c6, Objs(&dispatch::PSHL)), (0x1c9cc, Objs(&dispatch::PEFF)),
    (0x1c9d8, Gauge(G_ENEMY)), (0x1c9de, Objs(&dispatch::BOSS)), (0x1c9e4, Objs(&dispatch::ENEMY)),
    (0x1c9ea, Objs(&dispatch::ESHL)),
    (0x1c9f6, Gauge(G_DECOR)), (0x1c9fc, Objs(&dispatch::SET)), (0x1ca02, Objs(&dispatch::BOX)),
    (0x1ca08, Objs(&dispatch::ITEM)), (0x1ca0e, Cameras), (0x1ca14, Objs(&dispatch::EFFCT)),
    (0x1ca1a, Objs(&dispatch::SOBJ)),
];

/// Ordre de mise à jour selon la cinématique et la scène (comparaisons signées sur un octet).
fn world_order(b: &mut Cps2Bus) -> Order {
    let scene = rd8(b, A5 + ram::SCENE) as i8;
    if rd8(b, A5 + ram::CINEMATIC) != 0 {
        ORDER_CINEMATIC
    } else if scene >= 0x53 {
        ORDER_NORMAL
    } else if scene >= 0x45 {
        ORDER_REDUCED
    } else if scene == 8 {
        ORDER_SCENE_8
    } else {
        ORDER_NORMAL
    }
}

/// Met à jour tous les objets : joueurs, puis les dix classes dans l'ordre de la scène.
pub fn update_world(c: &mut Cpu, b: &mut Cps2Bus) {
    let order = world_order(b);
    for (i, &(site, step)) in order.iter().enumerate() {
        let ret = site + JSR;
        match step {
            Gauge(marker) => gauge_at(c, b, marker, ret),
            Players => call_at(c, b, ext::PLAYERS, ret),
            PlayerParts => call_at(c, b, ext::PLAYER_PARTS, ret),
            // dernier appel : un `jmp`, l'adresse de retour de l'appelant est déjà sur la pile
            Objs(class) => class.update(c, b, (i + 1 < order.len()).then_some(ret)),
            Cameras => call_at(c, b, ext::CAMERAS, ret),
        }
    }
}

/// Longueur d'un `jsr` absolu : l'adresse de retour est celle du site d'appel + 6.
const JSR: u32 = 6;

/// Jauge de charge appelée depuis un site d'origine connu (`ret` : adresse de retour).
fn gauge_at(c: &mut Cpu, b: &mut Cps2Bus, marker: u32, ret: u32) {
    c.d[1] = marker;
    call_at(c, b, ext::GAUGE, ret);
}

pub const WORLD_UPDATE: Native = Native {
    entry: 0x1c86e,
    name: "world_update",
    cost: |_, _| 100,
    run: |c, b| {
        update_world(c, b);
        rts(c, b);
    },
    touches_sound: false,
    outputs: 0, // les appelants enchaînent sur d'autres appels
    exact_cost: false,
};

// ---------------------------------------------------------------------------------------------
// Fin d'image (0x1c4c4)

/// Files de candidats aux collisions, remises à vide à chaque image.
const QUEUES: [&Queue; 5] =
    [&pools::QUEUE_PSHL, &pools::QUEUE_ESHL, &pools::QUEUE_ENEMY_BOSS, &pools::QUEUE_BOX, &pools::QUEUE_ITEM];

/// Fin de l'image, quand la phase revient : les files de collision repartent à vide. Le `rts`
/// dépile l'adresse du début de la boucle de partie (`0x1c468`).
pub const FRAME_END: Native = Native {
    entry: 0x1c4c4,
    name: "game_frame_end",
    cost: |_, _| 100,
    run: |c, b| {
        for queue in QUEUES {
            queue.reset(b);
        }
        // l'original laisse A0 sur la dernière file (le jeu en hérite à l'image suivante)
        c.a[0] = A5 + pools::QUEUE_ITEM.array;
        rts(c, b);
    },
    touches_sound: false,
    outputs: regs::a(0),
    exact_cost: false,
};

// ---------------------------------------------------------------------------------------------
// Phase 0 : initialisation de la partie (0x1c522)

/// Scripts d'animation de palette passés à `0x1324` (dans le code, après la table des étapes).
const PALETTE_SCRIPTS: [u32; 3] = [0x1c546, 0x1c54a, 0x1c54e];

/// Variante des trois objets SET créés à l'étape 0, et leur indice d'animation de palette (`+0x2f`).
const SET_PALETTES: [(u8, u8); 3] = [(0x3e, 0), (0x3f, 1), (0x40, 2)];
const SET_PALETTE_INDEX: u32 = 0x2f;

/// Octets de la RAM hors de la page A5±0x7fff, remis à zéro par la phase.
const CLEARED_A: u32 = 0xff_2472; // A5 - 0x5b8e
const CLEARED_B: u32 = 0xff_19f0; // A5 - 0x6610

/// Tâche d'interface des joueurs créée à la fin de la phase.
const PLAYER_UI_SLOT: u32 = 0x30;
const PLAYER_UI_TASK: u32 = 0x150ac;

/// Octets de `A5` remis à zéro en sortie : drapeau et score.
const EXIT_CLEARED_BYTES: [u32; 2] = [0x37, 0x5d];

fn next_step(b: &mut Cps2Bus) {
    let step = rd8(b, A5 + ram::STEP).wrapping_add(4);
    wr8(b, A5 + ram::STEP, step);
}

fn run_script(c: &mut Cpu, b: &mut Cps2Bus, script: u32) {
    c.a[4] = script;
    call(c, b, ext::PALETTE_SCRIPT);
}

/// Alloue un objet SET ; `None` si le pool est plein (retenue positionnée par l'allocation).
fn alloc_set(c: &mut Cpu, b: &mut Cps2Bus) -> Option<Obj> {
    call(c, b, ext::ALLOC_SET);
    (c.sr & 1 == 0).then(|| Obj(c.a[1]))
}

/// Étape 0 : trois objets SET (animations de palette), prépare les joueurs.
fn create_palette_objects(c: &mut Cpu, b: &mut Cps2Bus) {
    next_step(b);
    wr16(b, A5 + ram::TIMER, 0x40);
    wr8(b, A5 + ram::ACTIVE, 1);
    wr8(b, CLEARED_A, 0);
    call(c, b, ext::POOLS_INIT_SET);
    for (variant, index) in SET_PALETTES {
        // le pool vient d'être vidé : l'allocation réussit
        let set = alloc_set(c, b).expect("pool SET vide");
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

/// Étape 4 : lance les deux premières animations de palette (puis passe à l'étape 8).
fn start_palette_scripts(c: &mut Cpu, b: &mut Cps2Bus) {
    next_step(b);
    run_script(c, b, PALETTE_SCRIPTS[0]);
    run_script(c, b, PALETTE_SCRIPTS[1]);
    let y = rd16(b, A5 + ram::SCROLL2_Y).wrapping_sub(0x100);
    wr16(b, A5 + ram::SCROLL2_Y, y);
}

/// Début des étapes 8 et 12 : objets système et décors animés, sprites, puis décompte.
/// `site` : adresse du premier des trois `jsr` d'origine (retours empilés comme l'original).
/// Renvoie vrai quand le décompte est écoulé.
fn animate(c: &mut Cpu, b: &mut Cps2Bus, site: u32) -> bool {
    dispatch::SOBJ.update(c, b, Some(site + JSR));
    dispatch::SET.update(c, b, Some(site + 2 * JSR));
    call_at(c, b, ext::SPRITES, site + 3 * JSR);
    let left = rd16(b, A5 + ram::TIMER).wrapping_sub(1);
    wr16(b, A5 + ram::TIMER, left);
    (left as i16) < 0
}

/// Étape 8 : décompte écoulé, un objet SET (type 0xc) apparaît et l'étape 12 commence.
fn wait_then_spawn(c: &mut Cpu, b: &mut Cps2Bus) {
    if !animate(c, b, 0x1c5ea) {
        return;
    }
    let Some(set) = alloc_set(c, b) else { return };
    set.set_u8(b, Obj::ACTIVE, 1);
    set.set_u8(b, Obj::KIND, 0xc);
    next_step(b);
    wr16(b, A5 + ram::TIMER, 0x20);
}

/// Étape 12 : décompte écoulé, fondu de fin et troisième animation de palette, puis étape 16.
fn wait_then_fade(c: &mut Cpu, b: &mut Cps2Bus) {
    if !animate(c, b, 0x1c624) {
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
    c.d[0] = c.d[0] & 0xffff_0000 | PLAYER_UI_SLOT;
    c.a[0] = PLAYER_UI_TASK;
    call(c, b, ext::TASK_CREATE);
}

/// Phase 0 : un pas par image. L'étape (`A5+0x2d` : 0, 4, 8, 12, 16) choisit la suite.
pub fn phase0(c: &mut Cpu, b: &mut Cps2Bus) {
    if rd8(b, A5 + ram::CONTINUE) != 0 {
        return leave(c, b);
    }
    match rd8(b, A5 + ram::STEP) {
        0 => create_palette_objects(c, b),
        4 => {
            start_palette_scripts(c, b);
            wait_then_spawn(c, b);
        }
        8 => wait_then_spawn(c, b),
        12 => wait_then_fade(c, b),
        16 => reset_pools_and_leave(c, b),
        step => panic!("phase 0 : étape {step} inconnue"),
    }
}

pub const PHASE_0: Native = Native {
    entry: 0x1c522,
    name: "game_phase0",
    cost: |_, _| 100,
    run: |c, b| {
        phase0(c, b);
        rts(c, b);
    },
    touches_sound: false,
    outputs: 0,
    exact_cost: false,
};

// ---------------------------------------------------------------------------------------------
// Phase 12 : le jeu proprement dit (0x1c6a0)

/// Horloge de la partie en BCD : minutes (`+0x38`), secondes, images (0 à 59).
const CLOCK: u32 = 0x38;
/// Compteur d'images de la partie (mot, plafonné à 0xffff).
const GAME_FRAMES: u32 = 0x54;
/// Boutons pressés à cette image (+1) et à la précédente (+7), pour le saut de niveau de débogage.
const BUTTONS_NOW: u32 = 0x01;
const BUTTONS_BEFORE: u32 = 0x07;
/// Mots remis à zéro par un saut de niveau (statut de chaque joueur et de ses armes).
const PLAYER_RESET: [u32; 8] = [0x5e2, 0x7e2, 0x9e2, 0xbe2, 0x602, 0x802, 0xa02, 0xc02];
/// Indicateur X du registre d'état (retenue de `sbcd`).
const SR_X: u16 = 0x10;
/// Mot de la ROM dont le bit 15 active le saut de niveau de débogage.
const DEBUG_SWITCH: u32 = 0xc0;
/// Dernier numéro de scène.
const LAST_SCENE: i8 = 0x6c;
/// Boutons du saut de niveau : scène suivante, scène précédente.
const BUTTONS_NEXT: u8 = 0x90;
const BUTTONS_PREVIOUS: u8 = 0x60;

/// `d - s - X` en BCD sur un octet : résultat et retenue (celle de `sbcd`).
pub(crate) fn sbcd(d: u8, s: u8, x: bool) -> (u8, bool) {
    let (d, s, x) = (d as i32, s as i32, x as i32);
    let unadjusted = d - s - x;
    let mut adjust = 0;
    if (d & 0x0f) - (s & 0x0f) - x < 0 {
        adjust |= 0x06;
    }
    let borrow = unadjusted < 0 || unadjusted - adjust < 0;
    if unadjusted < 0 {
        adjust |= 0x60;
    }
    ((unadjusted - adjust) as u8, borrow)
}

/// Retire une image à l'horloge (images, secondes, minutes en cascade, de 59 à 00) : la cascade
/// s'arrête dès qu'un octet reste positif. La retenue part de l'indicateur X laissé par le code
/// précédent, d'où la lecture de `SR`.
fn tick_clock(c: &mut Cpu, b: &mut Cps2Bus) {
    let mut borrow = c.sr & SR_X != 0;
    for (i, offset) in [CLOCK + 2, CLOCK + 1, CLOCK].into_iter().enumerate() {
        let (value, next_borrow) = sbcd(rd8(b, A5 + offset), 1, borrow);
        borrow = next_borrow;
        c.sr = c.sr & !SR_X | if borrow { SR_X } else { 0 };
        wr8(b, A5 + offset, value);
        if value as i8 >= 0 {
            return;
        }
        if i < 2 {
            wr8(b, A5 + offset, 0x59);
        } else {
            // temps écoulé : horloge à zéro
            wr16(b, A5 + CLOCK, 0);
            wr16(b, A5 + CLOCK + 2, 0);
        }
    }
}

/// Saut de niveau de débogage : joueurs remis à zéro, retour à la phase 8 (entrée de scène).
fn change_scene(b: &mut Cps2Bus, forward: bool) {
    for offset in PLAYER_RESET {
        wr16(b, A5 + offset, 0);
        wr16(b, A5 + offset + 2, 0);
    }
    wr8(b, A5 + 0x50, 0);
    let scene = rd8(b, A5 + ram::SCENE);
    let scene = if forward {
        let next = scene.wrapping_add(1);
        if next as i8 <= LAST_SCENE { next } else { 1 }
    } else {
        let previous = scene.wrapping_sub(1);
        if previous as i8 > 0 { previous } else { LAST_SCENE as u8 }
    };
    wr8(b, A5 + ram::SCENE, scene);
    wr8(b, A5 + ram::PHASE, 8);
    wr8(b, A5 + ram::STEP, 0);
    wr16(b, A5 + ram::SUBSTEP, 0);
}

/// Phase 12 : une image de jeu.
pub fn phase12(c: &mut Cpu, b: &mut Cps2Bus) {
    if rd16(b, A5 + GAME_FRAMES) != 0xffff {
        let frames = rd16(b, A5 + GAME_FRAMES) + 1;
        wr16(b, A5 + GAME_FRAMES, frames);
        c.sr &= !SR_X; // l'addition n'a pas de retenue
    }
    if rd16(b, A5 + CLOCK) != 0 {
        tick_clock(c, b);
    }
    // sites des appels d'origine (0x1c6f6...) : retours empilés comme l'original (voir `call_at`)
    call_at(c, b, ext::INPUT_HOOK, 0x1c6f6 + JSR);
    c.a[7] = c.a[7].wrapping_sub(4);
    wr32(b, c.a[7], 0x1c6fc + JSR); // retour de `jsr 0x1c86e`
    update_world(c, b);
    c.a[7] = c.a[7].wrapping_add(4);
    for (gauge_site, marker, routine) in
        [(0x1c708, 0x54_0000, ext::SPRITES), (0x1c71a, 0x48_0002, ext::COLLISIONS), (0x1c72c, 0x4f_001e, ext::WORLD_EXTRA)]
    {
        gauge_at(c, b, marker, gauge_site + JSR);
        call_at(c, b, routine, gauge_site + 2 * JSR);
    }
    for (site, routine) in [(0x1c738, ext::WORLD_EXTRA_2), (0x1c73e, ext::RANK_TICK), (0x1c744, ext::END_OF_FRAME)] {
        call_at(c, b, routine, site + JSR);
    }
    debug_scene_skip(b);
}

/// Saut de niveau de débogage (fin de la phase 12 et de la phase 8, `0x1c74a`) : inactif dans le jeu de
/// production (le bit 15 du mot ROM `0xc0` est nul).
pub fn debug_scene_skip(b: &mut Cps2Bus) {
    if rd16(b, DEBUG_SWITCH) & 0x8000 != 0 {
        let pressed = !rd8(b, A5 + BUTTONS_BEFORE) & rd8(b, A5 + BUTTONS_NOW);
        if pressed & BUTTONS_NEXT == BUTTONS_NEXT {
            change_scene(b, true);
        } else if pressed & BUTTONS_PREVIOUS == BUTTONS_PREVIOUS {
            change_scene(b, false);
        }
    }
}

pub const PHASE_12: Native = Native {
    entry: 0x1c6a0,
    name: "game_phase12",
    cost: |_, _| 100,
    run: |c, b| {
        phase12(c, b);
        rts(c, b);
    },
    touches_sound: false,
    outputs: 0,
    exact_cost: false,
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
                order.iter().filter_map(|(_, s)| if let Objs(class) = s { Some(class.entry) } else { None }).collect();
            entries.sort();
            entries.dedup();
            assert_eq!(entries.len(), 10);
        }
    }

    /// Les sites d'appel se suivent : un `jsr` (6 octets), ou un `move.l #,D1` (6 octets) puis le
    /// `jsr` de la jauge.
    #[test]
    fn sites_croissants() {
        for order in [ORDER_NORMAL, ORDER_CINEMATIC, ORDER_REDUCED, ORDER_SCENE_8] {
            for w in order.windows(2) {
                let ecart = w[1].0 - w[0].0;
                assert!(ecart == JSR || ecart == 2 * JSR && matches!(w[1].1, Gauge(_)), "{:x}", w[1].0);
            }
        }
    }
}
