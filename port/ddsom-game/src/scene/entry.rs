//! Phase 8 (`0x20c38`) : entrée de scène.
//!
//! Étape 0 : prépare la scène (variables, palettes, caméras, joueurs remis à neuf et placés d'après
//! les tables de la scène). Étape 2 : à chaque image, joue la *séquence d'ouverture* de la scène
//! (un fondu, parfois un carton de titre, un dialogue ou un boss qui entre), fait tourner le lecteur
//! de script d'apparition (`script`), puis ferme la phase : la séquence passe la phase à 12 (le jeu)
//! quand elle est finie.
//!
//! Les séquences sont des machines à états : la sous-étape (`A5+0x2e`, pas de 2) indique où l'on en
//! est, des minuteries et des pointeurs d'objets sont gardés dans des variables temporaires
//! (`scratch`). Une scène sans séquence propre (la majorité) ne fait qu'un fondu et passe tout de suite
//! à la phase 12. Voir `docs/scenes.md` pour la table des scènes.

use cps2::Cps2Bus;
use m68k::Cpu;

use super::{player, scratch, script, sub, var, Ctx, PLAYERS, PLAYER_STRIDE};
use crate::game;
use crate::mem::{rd8, rts, wr16, wr32, wr8, A5};
use crate::objects::Obj;
use crate::Native;
use crate::objects::pools;

/// Commandes son appelées par les séquences (wrappers de `sound_table.rs`).
mod sfx {
    /// Musique de la scène (commande « fin de cinématique » des scènes).
    pub const SCENE_MUSIC: u32 = 0xe0c5c;
    pub const COMMAND_FF00: u32 = 0xe0882;
    pub const COMMAND_21: u32 = 0xe1370;
    pub const COMMAND_2D: u32 = 0xe1768;
    pub const COMMAND_E17A4: u32 = 0xe17a4;
    pub const COMMAND_E62FC: u32 = 0xe62fc;
    pub const COMMAND_E0A2E: u32 = 0xe0a2e;
    pub const COMMAND_EAAF4: u32 = 0xeaaf4;
    pub const COMMAND_EA6E2: u32 = 0xea6e2;
    pub const COMMAND_EADD4: u32 = 0xeadd4;
}

/// Tables en ROM (données), indexées par le numéro de scène.
mod table {
    /// Octet par scène copié dans `A5+0x48` / `A5+0x49` (paramètres de palette de la scène).
    pub const PALETTE_A: u32 = 0x21b38;
    pub const PALETTE_B: u32 = 0x21bb0;
    /// Un octet par scène (copié en `A5+0x3e` et `A5+0x40`), index ×2.
    pub const SCENE_BYTE: u32 = 0x21156;
    /// Position d'entrée des joueurs : 4 couples (X, Y) de mots par scène.
    pub const START_POSITIONS: u32 = 0x2123c;
    /// Octet par joueur et par scène (`+0x2c` du joueur).
    pub const START_STATES: u32 = 0x2196c;
}

/// Sous-routines d'initialisation de scène appelées à l'étape 0.
mod init {
    pub const SPRITES_RESET: u32 = 0x1de7c;
    pub const SCENE_LOAD: u32 = 0x33aa6;
    pub const POOLS_INIT: u32 = 0x2062;
    pub const PALETTES: u32 = 0x1a296;
    pub const HOOKS: [u32; 5] = [0x19aa2, 0x19abc, 0x19db4, 0x19e10, 0xf3130];
}

/// Variables de RAM de travail hors de la page A5 ± 0x7fff, remises à zéro à l'entrée.
mod ram {
    pub const BYTE_A: u32 = 0xff_2472;
    pub const BYTE_B: u32 = 0xff_2474;
    pub const BYTE_C: u32 = 0xff_2042;
    pub const WORD_D: u32 = 0xff_2046;
    pub const WORD_E: u32 = 0xff_2048;
    pub const WORD_F: u32 = 0xff_204a;
    pub const WORD_G: u32 = 0xff_204c;
    pub const WORD_H: u32 = 0xff_2040;
}

const STEP_INIT: u8 = 0;
const STEP_SEQUENCE: u8 = 2;

/// Scènes où le monde est en mode cinématique (`A5+0x44`) : 7, 8 et 9 (entrées du boss de la caverne).
const CINEMATIC_SCENES: [u8; 3] = [7, 8, 9];

/// Champs d'un joueur remis à zéro à l'entrée de scène (octets, mots, longs).
const PLAYER_CLEAR_BYTES: [u32; 8] = [0x16a, 0x15c, 0x8a, 0x10f, 0x18a, 0x188, 0x189, 0x53];
const PLAYER_CLEAR_WORDS: [u32; 3] = [0x8c, 0x8e, 0x8a];
const PLAYER_CLEAR_LONGS: [u32; 2] = [0x86, 0x24];
const PLAYER_SPELL_SLOTS: std::ops::RangeInclusive<u32> = 0x1a0..=0x1d8;

// ---------------------------------------------------------------------------------------------
// Étape 0 : préparation de la scène

fn init_scene(x: &mut Ctx) {
    wr8(x.b, A5 + var::STEP, STEP_SEQUENCE);
    for addr in [ram::BYTE_A, ram::BYTE_B, ram::BYTE_C] {
        wr8(x.b, addr, 0);
    }
    for addr in [ram::WORD_D, ram::WORD_E, ram::WORD_F] {
        wr16(x.b, addr, 0);
    }
    wr16(x.b, ram::WORD_G, 5);
    wr16(x.b, ram::WORD_H, 9);
    x.set_v8(0x5f, 1);
    x.set_v8(var::INPUT_LOCK, 0xff);
    x.set_v8(0x3c, 1);
    x.set_v8(0x3d, 0);
    x.set_v8(var::HOLD, 0);
    x.set_v16(0x54, 0);
    x.set_v16(0x52, 0);
    x.set_v8(0x58, 0);
    x.set_v8(0x51, 0);
    let scene = x.v8(var::SCENE) as u32;
    x.c.d[0] = scene;
    let (pa, pb) = (x.rd8(table::PALETTE_A + scene), x.rd8(table::PALETTE_B + scene));
    x.set_v8(0x48, pa);
    x.set_v8(0x49, pb);
    crate::modes::demo::reset_recorder(x.c, x.b);
    for routine in [init::SPRITES_RESET, init::SCENE_LOAD, script::RESET_ENTRY, init::POOLS_INIT, init::PALETTES]
        .into_iter()
        .chain(init::HOOKS)
    {
        x.call(routine);
    }
    crate::hud::forget_displayed(x.b);
    for k in 0..4 {
        clear_player(x, PLAYERS + k * PLAYER_STRIDE);
    }
    let cinematic = CINEMATIC_SCENES.contains(&(scene as u8));
    x.set_v8(0x44, cinematic as u8);
    let marker = x.rd8(table::SCENE_BYTE + 2 * scene);
    x.set_v8(0x3e, marker);
    x.set_v8(0x40, marker);
    let mut alive = 0u8;
    for k in 0..4 {
        let p = PLAYERS + k * PLAYER_STRIDE;
        if x.rd8(p + player::ACTIVE) != 0 && x.rd16(p + player::HEALTH) != 0 {
            alive += 1;
            wr32(x.b, p + 4, 0);
        }
    }
    x.c.d[0] = alive as u32;
    x.set_v8(var::PLAYERS, alive);
    place_players(x);
}

fn clear_player(x: &mut Ctx, p: u32) {
    for off in PLAYER_CLEAR_BYTES {
        wr8(x.b, p + off, 0);
    }
    for off in PLAYER_CLEAR_WORDS {
        wr16(x.b, p + off, 0);
    }
    for off in PLAYER_CLEAR_LONGS {
        wr32(x.b, p + off, 0);
    }
    for off in PLAYER_SPELL_SLOTS.step_by(4) {
        wr32(x.b, p + off, 0);
    }
}

/// Place chaque joueur en jeu devant la caméra, d'après les tables de la scène (consommées dans
/// l'ordre des joueurs en jeu).
pub fn place_players(x: &mut Ctx) {
    let scene = x.v8(var::SCENE) as u32;
    let mut positions = table::START_POSITIONS + (scene << 4);
    let mut states = table::START_STATES + (scene << 2);
    let camera = if x.v8(var::USE_ALT_CAMERA) != 0 { 0x556 } else { 0x506 };
    // registres de l'original : D0/D1 = caméra (ou scène décalée), D2 = rang, D6 = -1, A1-A3 = fins
    let (mut d0, mut d1) = (scene << 4, scene << 2);
    let mut rank = 0u8;
    for k in 0..4 {
        let p = PLAYERS + k * PLAYER_STRIDE;
        if x.rd8(p + player::ACTIVE) == 0 || x.rd16(p + player::HEALTH) == 0 {
            continue;
        }
        let (dx, dy) = (x.v16(camera), x.v16(camera + 4));
        (d0, d1) = (dx as u32, dy as u32);
        let (px, py) = (x.rd16(positions), x.rd16(positions + 2));
        positions += 4;
        wr16(x.b, p + 8, px.wrapping_add(dx));
        wr16(x.b, p + 0x10, py.wrapping_add(dy));
        wr32(x.b, p + 0xc, 0);
        let state = x.rd8(states);
        wr8(x.b, p + 0x2c, state);
        states += 1;
        wr8(x.b, p + 0x7d, rank);
        rank += 1;
    }
    x.c.d[0] = d0;
    x.c.d[1] = d1;
    x.c.d[2] = rank as u32;
    x.c.d[6] = 0xffff;
    x.c.a[1] = PLAYERS + 4 * PLAYER_STRIDE;
    x.c.a[2] = positions;
    x.c.a[3] = states;
}

/// 0x20de6 appelée par le code d'origine (sortie de scène) : `place_players`.
pub const PLACE_PLAYERS: Native = Native {
    entry: 0x20de6,
    name: "scene_place_joueurs",
    run: |c, b| {
        place_players(&mut Ctx::new(c, b));
        crate::mem::rts(c, b);
    },
    touches_sound: false,
    outputs: crate::regs::ALL,
};

// ---------------------------------------------------------------------------------------------
// Étape 2 : séquences d'ouverture

/// Une séquence d'ouverture : un pas par image (le monde est mis à jour à la fin).
type Sequence = fn(&mut Ctx);

/// Séquence d'ouverture de la scène `scene`.
fn sequence(scene: u8) -> Sequence {
    match scene {
        1 => stage_1,
        2 => stage_2,
        4 => scene_4,
        5 | 20 => wait_players_10,
        7 | 9 => boss_cave,
        8 => scene_8,
        10 => scene_10,
        11 => scene_11,
        14 => title_card,
        24 => scene_24,
        25 | 106..=108 => quick_with_music,
        26 => scene_26,
        27 => scene_27,
        28 => scene_28,
        32 => scene_32,
        33 | 36 => plain_fade,
        34 => scene_34,
        35 => title_card_music,
        38 => scene_38,
        39 | 41 | 43 | 45 => wait_players_6,
        40 => scene_40,
        42 => scene_42,
        44 | 111 => scene_44,
        47 => scene_47,
        54 => scene_54,
        58 => scene_58,
        59 => title_card_timed,
        64 => scene_64,
        65 => scene_65,
        66 => scene_66,
        67 => scene_67,
        90..=104 => white_start,
        105 => scene_105,
        109 | 112..=114 => quick_with_sobj,
        110 => quick_with_sobj_flag,
        _ => quick,
    }
}

/// Phase 8 : une image.
pub fn phase_8(c: &mut Cpu, b: &mut Cps2Bus) {
    let mut x = Ctx::new(c, b);
    match x.v8(var::STEP) {
        STEP_INIT => init_scene(&mut x),
        STEP_SEQUENCE => {
            let scene = x.v8(var::SCENE);
            x.c.d[0] = (scene as u32) << 2;
            sequence(scene)(&mut x);
            x.call(script::SPAWN_ENTRY);
            game::debug_scene_skip(x.b);
        }
        _ => {}
    }
}

pub const PHASE_8: Native = Native {
    entry: 0x20c38,
    name: "scene_phase_8",
    run: |c, b| {
        phase_8(c, b);
        rts(c, b);
    },
    touches_sound: false,
    outputs: 0,
};

// --- séquences sans état -----------------------------------------------------------------------

/// Scène sans séquence propre : un fondu d'ouverture, la musique, et le jeu commence.
fn quick(x: &mut Ctx) {
    x.leave_phase();
    x.set_v8(var::PLAYERS_FROZEN, 0);
    x.fade_in();
    x.call(sfx::SCENE_MUSIC);
    x.world();
}

/// Scènes 25, 106-108 : comme `quick`, la musique ne démarre qu'une fois (`A5+0x67`).
fn quick_with_music(x: &mut Ctx) {
    x.leave_phase();
    x.set_v8(var::PLAYERS_FROZEN, 0);
    x.fade_in();
    if x.v8(0x67) == 0 {
        x.set_v8(0x67, 1);
        x.call(sfx::SCENE_MUSIC);
    }
    x.world();
}

/// Scènes 90-104 (démonstrations de l'attract) : fondu depuis le blanc et objet système 6/23.
fn white_start(x: &mut Ctx) {
    x.leave_phase();
    x.set_v8(var::PLAYERS_FROZEN, 0);
    x.set_v8(var::FADE_PAGES, 15);
    x.call(sub::PALETTES_WHITE);
    x.c.d[0] = 23;
    x.call(sub::SOBJ_TYPE_6);
    x.world();
}

/// Scènes 109, 112-114 : fondu, objet système 6/23, musique.
fn quick_with_sobj(x: &mut Ctx) {
    x.leave_phase();
    x.set_v8(var::PLAYERS_FROZEN, 0);
    x.fade_in();
    x.c.d[0] = 23;
    x.call(sub::SOBJ_TYPE_6);
    x.call(sfx::SCENE_MUSIC);
    x.world();
}

/// Scène 110 : lève `A5+0x59` puis comme 109.
fn quick_with_sobj_flag(x: &mut Ctx) {
    x.set_v8(0x59, 1);
    quick_with_sobj(x);
}

/// Scène 67 : saute directement à la phase suivante, entrées verrouillées.
fn scene_67(x: &mut Ctx) {
    x.leave_phase();
    x.set_v8(var::HOLD, 0);
    x.set_v8(var::INPUT_LOCK, 0x0f);
    x.set_v8(var::PLAYERS_FROZEN, 1);
    x.fade_in();
    x.call(sub::TITLE_CARD);
    x.call(sub::INPUTS);
    x.world();
}

// --- séquences à états ---------------------------------------------------------------------------

/// Début habituel : sous-étape suivante, scène et joueurs verrouillés, fondu d'ouverture.
fn begin(x: &mut Ctx) {
    x.next_sub();
    x.lock();
}

/// Scène 1 (niveau 1) : fondu, carton, premier dialogue, puis trois ennemis entrent par la droite.
fn stage_1(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a32);
    const MESSAGE: u32 = scratch(0x9a34);
    match x.sub() {
        0 => {
            begin(x);
            x.wr16(TIMER, 0x140);
            x.fade_in();
            x.call(0xabb70);
            x.call(sub::TITLE_CARD);
            x.call(sub::RANK_RESET);
            x.call(sfx::SCENE_MUSIC);
        }
        2 => {
            if x.tick(TIMER) {
                x.next_sub();
                x.c.d[0] = 0;
                x.call(sub::MESSAGE);
                let a1 = x.c.a[1];
                wr32(x.b, MESSAGE, a1);
            }
        }
        4 => {
            let message = Obj(crate::mem::rd32(x.b, MESSAGE));
            if message.u8(x.b, 0x22) != 0 {
                x.next_sub();
                x.wr16(TIMER, 0xb0);
            }
        }
        6 => {
            if x.tick(TIMER) {
                x.next_sub();
                x.wr16(TIMER, 0x60);
            }
        }
        8 => {
            if x.tick(TIMER) {
                x.leave_phase();
                x.set_v8(var::HOLD, 0);
                x.set_v8(var::PLAYERS_FROZEN, 0);
            } else if x.rd16(TIMER) == 0x50 {
                for (px, py) in [(0x180, 0x210), (0x230, 0x220), (0x2d0, 0x200)] {
                    let o = x.spawn(&pools::ENEMY, 0x14);
                    o.set_u8(x.b, Obj::ACTIVE, 2);
                    o.set_u8(x.b, Obj::VARIANT, 1);
                    o.set_u16(x.b, Obj::X, px);
                    o.set_u16(x.b, Obj::Y, py);
                    o.set_u16(x.b, 0x14, 0xfb80);
                }
            }
        }
        _ => {}
    }
    x.world();
}

/// Scène 2 : fondu, carton de titre ; attend un instant.
fn stage_2(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a38);
    match x.sub() {
        0 => {
            begin(x);
            x.wr16(TIMER, 1);
            x.fade_in();
            x.call(sub::TITLE_CARD);
            x.call(sub::RANK_RESET);
            x.call(sfx::COMMAND_EAAF4);
            x.call(sfx::SCENE_MUSIC);
        }
        2 => {
            if x.tick(TIMER) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scène 4 : titre, puis deux effets de scène à intervalles.
fn scene_4(x: &mut Ctx) {
    const SHAKE: u32 = scratch(0x9a3c);
    const TIMER: u32 = scratch(0x9a3a);
    match x.sub() {
        0 => {
            begin(x);
            x.wr16(SHAKE, 0);
            x.wr16(TIMER, 0x158);
            x.fade_in();
            x.call(sub::TITLE_CARD);
            x.call(sub::RANK_RESET);
        }
        2 => {
            if x.tick(TIMER) {
                x.next_sub();
                x.wr16(TIMER, 0x40);
                x.set_v8(0x54f, 8);
            }
        }
        4 => {
            if x.tick(TIMER) {
                x.next_sub();
                x.wr16(SHAKE, 1);
                x.wr16(TIMER, 0x130);
                x.set_v8(0x500, 8);
            }
        }
        6 => {
            if x.tick(TIMER) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scènes 7 et 9 (grotte du boss) : un effet, un boss et un décor entrent ; la 7 remet le rang à zéro.
fn boss_cave(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a3a);
    match x.sub() {
        0 => {
            begin(x);
            x.wr16(TIMER, 0x90);
            x.fade_in();
            let scene = x.v8(var::SCENE);
            if scene != 9 {
                x.call(sub::TITLE_CARD);
            }
            let effect = x.spawn(&pools::EFFCT, 13);
            let camera_x = x.v16(0x506);
            let camera_y = x.v16(0x50a);
            effect.set_u16(x.b, Obj::X, camera_x.wrapping_sub(0x100));
            effect.set_u16(x.b, Obj::Y, camera_y.wrapping_add(0x50));
            x.set_v16(0x46, effect.0 as u16);
            x.spawn(&pools::BOSS, 13);
            let set = x.spawn(&pools::SET, 14);
            if scene == 9 {
                set.set_u8(x.b, Obj::VARIANT, 1);
            }
            if scene == 7 {
                x.call(sub::RANK_RESET);
            }
            x.call(sfx::SCENE_MUSIC);
        }
        2 => {
            if x.tick(TIMER) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scène 8 : un effet 0x22 près de la caméra, une commande son, puis l'effet 0xb8956.
fn scene_8(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a3a);
    match x.sub() {
        0 => {
            begin(x);
            x.wr16(TIMER, 0x100);
            x.fade_in();
            let effect = x.spawn(&pools::EFFCT, 0x22);
            let camera_x = x.v16(0x506);
            let camera_y = x.v16(0x50a);
            effect.set_u16(x.b, Obj::X, camera_x.wrapping_sub(0x60));
            effect.set_u16(x.b, Obj::Y, camera_y.wrapping_add(0x50));
            x.set_v16(0x46, effect.0 as u16);
            x.call(sfx::COMMAND_FF00);
            x.c.d[1] = 1;
            x.call(0xb8956);
        }
        2 => {
            if x.tick(TIMER) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scène 10 : transition imposée (`A5+0x51e` = 4), un effet 0x13 et un décor.
fn scene_10(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a3a);
    match x.sub() {
        0 => {
            x.next_sub();
            x.set_v16(0x51e, 4);
            x.lock();
            x.wr16(TIMER, 0xc0);
            x.fade_in();
            let effect = x.spawn(&pools::EFFCT, 0x13);
            x.set_v16(0x46, effect.0 as u16);
            let set = x.spawn(&pools::SET, 14);
            set.set_u8(x.b, Obj::VARIANT, 1);
            x.call(sfx::SCENE_MUSIC);
        }
        2 => {
            if x.tick(TIMER) {
                x.leave_phase();
                x.set_v16(0x51e, 0);
                x.set_v8(var::HOLD, 0);
                x.set_v8(var::PLAYERS_FROZEN, 0);
            }
        }
        _ => {}
    }
    x.world();
}

/// Scène 11 : carton, deux dialogues (95), un ennemi (21), un défilement forcé.
fn scene_11(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a3e);
    const MESSAGE: u32 = scratch(0x9a40);
    const ENEMY: u32 = scratch(0x9a42);
    match x.sub() {
        0 => {
            begin(x);
            x.wr16(TIMER, 0x78);
            x.fade_in();
            x.call(sub::TITLE_CARD);
            x.call(0xb17ba);
            x.call(sub::RANK_RESET);
            x.call(sfx::COMMAND_EA6E2);
            x.call(sfx::SCENE_MUSIC);
        }
        2 => {
            if x.tick(TIMER) {
                x.next_sub();
                x.wr16(TIMER, 0xa0);
                x.call(0xb1764);
            }
        }
        4 => {
            if x.tick(TIMER) {
                x.next_sub();
                x.message(95, MESSAGE);
            } else if x.rd16(TIMER) == 0x40 {
                x.call(0xb1764);
            }
        }
        6 => {
            if x.message_done(MESSAGE) {
                x.next_sub();
                x.wr16(TIMER, 0x10);
            }
        }
        8 => {
            if x.tick(TIMER) {
                x.next_sub();
                x.wr16(TIMER, 0xe0);
                let o = x.spawn(&pools::ENEMY, 0x15);
                o.set_u8(x.b, Obj::VARIANT, 8);
                o.set_u16(x.b, 0x42, 0x401);
                x.store_obj(ENEMY, o);
            }
        }
        10 => {
            if x.tick(TIMER) {
                x.next_sub();
                x.set_v16(0x4ba, 0x100);
                x.wr16(TIMER, 12);
                x.call(0x1ac86);
                x.c.d[0] = 60;
                x.call(0x2860);
            }
        }
        12 => {
            if x.tick(TIMER) {
                x.next_sub();
                x.set_v16(0x4ba, 0);
                x.wr16(TIMER, 0x100);
                let o = x.load_obj(ENEMY);
                crate::mem::wr32(x.b, o.0 + 4, 0x0206_0000);
            }
        }
        14 => {
            if x.tick(TIMER) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scènes 5 et 20 : attend que les joueurs en jeu soient tous dans l'état 10.
fn wait_players_10(x: &mut Ctx) {
    match x.sub() {
        0 => {
            x.next_sub();
            x.set_v8(0x67, 0);
            x.lock();
            x.fade_in();
            x.call(sub::TITLE_CARD);
            x.call(sub::RANK_RESET);
            x.call(sfx::SCENE_MUSIC);
        }
        2 => {
            if x.players_in_state(10) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scène 14 et variantes : fondu, carton de titre, une image d'attente.
fn title_card(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a44);
    match x.sub() {
        0 => {
            begin(x);
            x.wr16(TIMER, 1);
            x.fade_in();
            x.call(sub::TITLE_CARD);
            x.call(sub::RANK_RESET);
        }
        2 => {
            if x.tick(TIMER) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scène 24 (Manticore) : deux dialogues (31, puis 32 si un joueur est dans l'état 4), un boss.
fn scene_24(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a46);
    const MESSAGE: u32 = scratch(0x9a48);
    const BOSS: u32 = scratch(0x9a4a);
    match x.sub() {
        0 => {
            x.next_sub();
            x.set_v8(0x67, 0);
            x.lock();
            x.wr16(TIMER, 0xb4);
            x.fade_in();
            x.call(sub::RANK_RESET);
            x.call(sub::TITLE_CARD);
            x.call(sfx::SCENE_MUSIC);
        }
        2 => {
            if x.players_in_state(6) {
                x.next_sub();
                x.message(31, MESSAGE);
            }
        }
        4 => {
            if x.message_done(MESSAGE) {
                x.next_sub();
                let mut found = false;
                for k in 0..4 {
                    let p = PLAYERS + k * PLAYER_STRIDE;
                    if rd8(x.b, p) != 0 {
                        let character = rd8(x.b, p + 3) as u16 >> 1;
                        found |= character as u8 == 2;
                    }
                }
                x.c.d[1] = found as u32;
                if found {
                    x.message(32, MESSAGE);
                } else {
                    x.next_sub();
                    x.wr16(TIMER, 60);
                }
            }
        }
        6 => {
            if x.message_done(MESSAGE) {
                x.next_sub();
                x.wr16(TIMER, 90);
            }
        }
        8 => {
            if x.tick(TIMER) {
                x.next_sub();
                x.wr16(TIMER, 0xa0);
                let boss = x.spawn(&pools::BOSS, 6);
                x.store_obj(BOSS, boss);
            }
        }
        10 => {
            if x.tick(TIMER) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scène 26 (Lich) : fondu, dialogue 34, puis une commande son.
fn scene_26(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a46);
    const MESSAGE: u32 = scratch(0x9a48);
    match x.sub() {
        0 => {
            x.next_sub();
            x.set_v8(0x67, 0);
            x.lock();
            x.wr16(TIMER, 0x60);
            x.fade_in();
            x.call(sub::RANK_RESET);
            x.call(sfx::SCENE_MUSIC);
        }
        2 => {
            if x.tick(TIMER) {
                x.next_sub();
                x.message(34, MESSAGE);
            }
        }
        4 => {
            if x.message_done(MESSAGE) {
                x.next_sub();
                x.wr16(TIMER, 30);
                x.call(0xa8a42);
            }
        }
        6 => {
            if x.tick(TIMER) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scène 27 : carton, dialogue 37.
fn scene_27(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a4c);
    const MESSAGE: u32 = scratch(0x9a4e);
    match x.sub() {
        0 => {
            begin(x);
            x.wr16(TIMER, 0x100);
            x.fade_in();
            x.call(sub::TITLE_CARD);
            x.call(sfx::SCENE_MUSIC);
        }
        2 => {
            if x.tick(TIMER) {
                x.next_sub();
                x.message(37, MESSAGE);
            }
        }
        4 => {
            if x.message_done(MESSAGE) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scène 28 : carton, dialogue 140 ; un déclencheur extérieur (`A5+0x4e`) saute à la sous-étape 6.
fn scene_28(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a50);
    const MESSAGE: u32 = scratch(0x9a52);
    const MUSIC_STATE: u32 = scratch(0x9a54);
    match x.sub() {
        0 => {
            begin(x);
            x.wr16(TIMER, 0x100);
            x.fade_in();
            x.call(sub::TITLE_CARD);
            if x.rd16(MUSIC_STATE) == 0 {
                x.wr16(MUSIC_STATE, 1);
                x.call(sfx::SCENE_MUSIC);
            }
            if x.v8(0x4e) != 0 {
                x.set_sub(6);
                x.wr16(TIMER, 1);
                if x.rd16(MUSIC_STATE) != 2 {
                    x.wr16(MUSIC_STATE, 2);
                    x.c.d[4] = 24;
                    x.call(sfx::COMMAND_E17A4);
                    x.world();
                    return;
                }
            }
            x.set_v32(0x38, 0x0459_5900);
        }
        2 => {
            if x.tick(TIMER) {
                x.next_sub();
                x.message(140, MESSAGE);
            }
        }
        4 => {
            if x.message_done(MESSAGE) {
                x.next_sub();
                x.wr16(TIMER, 80);
            }
        }
        6 => {
            if x.tick(TIMER) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scène 32 (dragon noir) : carton ; un boss d'ouverture (type 0x13) mène la suite ; fin quand les
/// joueurs ont fini (état 6).
fn scene_32(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a50);
    match x.sub() {
        0 => {
            begin(x);
            x.wr16(TIMER, 0xe0);
            x.fade_in();
            x.call(sub::TITLE_CARD);
            x.call(sfx::SCENE_MUSIC);
        }
        2 => {
            if x.tick(TIMER) {
                x.next_sub();
                x.spawn(&pools::BOSS, 0x13);
            }
        }
        4 => {} // le boss d'ouverture passe lui-même à la sous-étape 6
        6 => {
            if x.players_in_state(6) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scène 34 : verrou d'entrées partiel (0x0f), commande son de ouverture.
fn scene_34(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a50);
    match x.sub() {
        0 => {
            begin(x);
            x.set_v8(var::INPUT_LOCK, 0x0f);
            x.wr16(TIMER, 1);
            x.fade_in();
            if x.v8(0x59) != 0 {
                x.call(0xb7f14);
            }
            x.call(sfx::COMMAND_FF00);
            x.call(sfx::SCENE_MUSIC);
            x.c.d[0] = 0;
            x.c.d[1] = 144;
            x.call(sfx::COMMAND_E0A2E);
        }
        2 => {
            if x.tick(TIMER) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scènes 33 et 36 : fondu simple, une image d'attente.
fn plain_fade(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a50);
    match x.sub() {
        0 => {
            begin(x);
            x.wr16(TIMER, 1);
            x.fade_in();
            x.call(sfx::SCENE_MUSIC);
        }
        2 => {
            if x.tick(TIMER) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scène 35 : carton, musique, une image d'attente.
fn title_card_music(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a50);
    match x.sub() {
        0 => {
            begin(x);
            x.wr16(TIMER, 1);
            x.fade_in();
            x.call(sub::TITLE_CARD);
            x.call(sfx::SCENE_MUSIC);
        }
        2 => {
            if x.tick(TIMER) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scène 59 : carton, musique, une image d'attente.
fn title_card_timed(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a5a);
    match x.sub() {
        0 => {
            begin(x);
            x.wr16(TIMER, 1);
            x.fade_in();
            x.call(sub::TITLE_CARD);
            x.call(sfx::SCENE_MUSIC);
        }
        2 => {
            if x.tick(TIMER) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scène 38 : le nombre de joueurs règle la durée ; dialogue 68 ; la fin laisse `A5+0x4a` en place.
fn scene_38(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a50);
    const MESSAGE: u32 = scratch(0x9a52);
    const DURATIONS: u32 = 0x22c28;
    match x.sub() {
        0 => {
            x.next_sub();
            x.set_v8(var::PLAYERS_FROZEN, 1);
            x.set_v8(var::INPUT_LOCK, 0);
            let players = x.v8(var::PLAYERS) as u32;
            let duration = x.prog16(DURATIONS + 2 * players);
            x.c.d[0] = 2 * players;
            x.wr16(TIMER, duration);
            x.set_v16(var::FADE_DELAY, 2);
            x.set_v8(var::FADE_PAGES, 15);
            x.call(sub::PALETTES_BLACK);
            x.c.a[0] = sub::FADE_TASK;
            x.c.d[0] = 0x10;
            crate::kernel::creer_tache(x.b, x.c.d[0], x.c.a[0]);
        }
        2 => {
            if x.tick(TIMER) {
                x.next_sub();
                x.message(68, MESSAGE);
            }
        }
        4 => {
            if x.message_done(MESSAGE) {
                x.next_sub();
                x.wr16(TIMER, 60);
            }
        }
        6 => {
            if x.tick(TIMER) {
                x.leave_phase();
                x.set_v8(0x42, 1);
                x.set_v8(var::HOLD, 0);
            }
        }
        _ => {}
    }
    x.call(sub::INPUTS);
    x.world();
}

/// Scènes 39, 41, 43, 45 : attend les joueurs dans l'état 6 ; la 41 lance en plus 0xdfa4a.
fn wait_players_6(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a56);
    match x.sub() {
        0 => {
            begin(x);
            x.wr16(TIMER, 1);
            x.fade_in();
            x.call(sub::TITLE_CARD);
            x.call(sfx::SCENE_MUSIC);
        }
        2 => {
            if x.players_in_state(6) {
                if x.v8(var::SCENE) == 41 {
                    x.c.d[0] = 0;
                    x.c.d[1] = 0;
                    x.call(0xdfa4a);
                }
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scène 40 : joueurs dans l'état 4, puis un boss (type 12) apparaît pendant le compte à rebours.
fn scene_40(x: &mut Ctx) {
    boss_gate(x, 0x1ab56, 0x4cfbc, 0x60, 12, 0x90)
}

/// Scène 42 : comme la 40 avec d'autres appels et le boss de type 11.
fn scene_42(x: &mut Ctx) {
    boss_gate(x, 0x1aaba, 0xb9942, 0x40, 11, 0x80)
}

/// Sous-programme commun des scènes 40 et 42.
fn boss_gate(x: &mut Ctx, prepare: u32, tail: u32, spawn_at: u16, boss: u8, wait: u16) {
    const TIMER: u32 = scratch(0x9a56);
    match x.sub() {
        0 => {
            begin(x);
            x.fade_in();
            x.call(sfx::SCENE_MUSIC);
        }
        2 => {
            if x.players_in_state(4) {
                x.next_sub();
                x.wr16(TIMER, wait);
                x.call(prepare);
                x.call(tail);
            }
        }
        4 => {
            if x.tick(TIMER) {
                x.end_scene();
                x.call(0x1abe0);
            } else if x.rd16(TIMER) == spawn_at {
                x.spawn(&pools::BOSS, boss);
            }
        }
        _ => {}
    }
    x.world();
}

/// Scènes 44 et 111 : boss de type 0x11, attend l'état 10, dialogue 43.
fn scene_44(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a56);
    const MESSAGE: u32 = scratch(0x9a58);
    match x.sub() {
        0 => {
            begin(x);
            x.wr16(TIMER, 1);
            x.fade_in();
            if x.v8(0x30) == 0 {
                x.c.d[0] = 23;
                x.call(sub::SOBJ_TYPE_6);
            }
            x.call(sfx::SCENE_MUSIC);
        }
        2 => {
            x.next_sub();
            let boss = x.spawn(&pools::BOSS, 0x11);
            boss.set_u8(x.b, Obj::VARIANT, 1);
        }
        4 => {}
        6 => {
            if x.players_in_state(10) {
                x.next_sub();
                x.wr16(TIMER, 48);
            }
        }
        8 => {
            if x.tick(TIMER) {
                x.next_sub();
                x.message(43, MESSAGE);
            }
        }
        10 => {
            if x.message_done(MESSAGE) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scène 47 : fondu à un niveau par pas ; la fin attend la fin du fondu.
fn scene_47(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a5a);
    match x.sub() {
        0 => {
            begin(x);
            x.wr16(TIMER, 1);
            x.fade_in_stepped();
            x.call(sub::TITLE_CARD);
            x.call(sfx::SCENE_MUSIC);
        }
        2 => {
            if x.v8(var::FADE_STEPS) == 0 {
                x.end_scene();
                x.call(0x3525c);
            }
        }
        _ => {}
    }
    x.world();
}

/// Scène 54 (Tel'Arin) : joueurs dans les états 4 puis 8, dialogue 43 ; un boss de type 0x11 si `A5+0x66`.
fn scene_54(x: &mut Ctx) {
    const MESSAGE: u32 = scratch(0x9a5c);
    match x.sub() {
        0 => {
            begin(x);
            x.wr16(scratch(0x9a5a), 1);
            x.fade_in();
            x.call(sub::TITLE_CARD);
            if x.v8(0x66) == 0 {
                x.set_sub(4);
                let boss = x.spawn(&pools::BOSS, 0x11);
                boss.set_u8(x.b, Obj::VARIANT, 2);
            }
            x.call(sfx::SCENE_MUSIC);
        }
        2 => {
            if x.players_in_state(4) {
                x.end_scene();
            }
        }
        4 => {
            if x.players_in_state(8) {
                x.next_sub();
                x.message(43, MESSAGE);
            }
        }
        6 => {
            if x.message_done(MESSAGE) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scène 58 : un boss de type 5, carton ; joueurs dans l'état 4, dialogue 75.
fn scene_58(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a5a);
    const MESSAGE: u32 = scratch(0x9a5c);
    match x.sub() {
        0 => {
            begin(x);
            x.fade_in();
            x.spawn(&pools::BOSS, 5);
            x.call(sub::TITLE_CARD);
            x.call(sfx::SCENE_MUSIC);
        }
        2 => {
            if x.players_in_state(4) {
                x.next_sub();
                x.wr16(TIMER, 48);
            }
        }
        4 => {
            if x.tick(TIMER) {
                x.next_sub();
                x.message(75, MESSAGE);
            }
        }
        6 => {
            if x.message_done(MESSAGE) {
                x.next_sub();
                x.wr16(TIMER, 8);
            }
        }
        8 => {
            if x.tick(TIMER) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scène 64 : effet 75, carton ; joueurs dans l'état 4, dialogue 77.
fn scene_64(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a5a);
    const MESSAGE: u32 = scratch(0x9a5c);
    match x.sub() {
        0 => {
            begin(x);
            x.fade_in();
            x.spawn(&pools::EFFCT, 75);
            x.call(sub::TITLE_CARD);
            x.call(sfx::SCENE_MUSIC);
        }
        2 => {
            if x.players_in_state(4) {
                x.next_sub();
                x.message(77, MESSAGE);
            }
        }
        4 => {
            if x.message_done(MESSAGE) {
                x.next_sub();
                x.wr16(TIMER, 0x100);
            }
        }
        6 => {
            if x.tick(TIMER) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scène 65 : joueurs dans l'état 6, dialogue 81 ; la fin ouvre la transition 0xeadd4.
fn scene_65(x: &mut Ctx) {
    const MESSAGE: u32 = scratch(0x9a5c);
    match x.sub() {
        0 => {
            begin(x);
            x.fade_in();
            x.call(sub::TITLE_CARD);
            x.call(sfx::SCENE_MUSIC);
        }
        2 => {
            if x.players_in_state(6) {
                x.next_sub();
                x.message(81, MESSAGE);
            }
        }
        4 => {
            if x.message_done(MESSAGE) {
                x.leave_phase();
                x.set_v8(0x42, 1);
                x.set_v8(var::HOLD, 0);
                x.call(sfx::COMMAND_EADD4);
            }
        }
        _ => {}
    }
    x.world();
}

/// Scène 66 (dernier niveau) : enchaîne trois dialogues (82, 83, 84) autour d'un défilement forcé et de
/// quatre effets 0x50 ; `A5+0x5c` saute directement à l'étape des effets.
fn scene_66(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a5a);
    const MESSAGE: u32 = scratch(0x9a5c);
    match x.sub() {
        0 => {
            if x.v8(0x5c) != 0 {
                x.set_sub(8);
                final_effects(x);
            } else {
                x.next_sub();
                x.wr16(scratch(0x9a60), 0);
                x.lock();
                x.fade_in_stepped();
                x.spawn(&pools::BOSS, 10);
            }
        }
        2 => {
            if x.players_in_state(4) {
                x.next_sub();
                x.message(82, MESSAGE);
                x.call(0x3525c);
            }
        }
        4 => {
            if x.message_done(MESSAGE) {
                x.next_sub();
                x.wr16(TIMER, 0x100);
            }
        }
        6 => {
            if x.tick(TIMER) {
                x.end_scene();
            }
            if x.rd16(TIMER) == 0x80 {
                x.call(sfx::COMMAND_21);
            }
        }
        8 => final_effects(x),
        10 => {
            if x.v16(0x51e) != 0 {
                x.next_sub();
                x.wr16(TIMER, 48);
                x.call(sfx::COMMAND_2D);
            }
        }
        12 => {
            if x.tick(TIMER) {
                x.next_sub();
                x.message(83, MESSAGE);
                x.call(0x3525c);
            }
        }
        14 => {
            if x.message_done(MESSAGE) {
                x.next_sub();
                x.message(84, MESSAGE);
            }
        }
        16 => {
            if x.message_done(MESSAGE) {
                x.next_sub();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scène 66, sous-étape 8 : fondu à un niveau par pas et quatre effets 0x50 (variantes 0 à 3).
fn final_effects(x: &mut Ctx) {
    x.next_sub();
    x.lock();
    x.fade_in_stepped();
    for variant in 0..4 {
        let effect = x.spawn(&pools::EFFCT, 0x50);
        effect.set_u8(x.b, Obj::VARIANT, variant);
    }
    x.call(sfx::SCENE_MUSIC);
}

/// Scène 105 : un effet 0x24, dialogues 86 et 87, deux fins possibles (selon `A5+0x5c`), dialogue 42.
fn scene_105(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a5a);
    const MESSAGE: u32 = scratch(0x9a5c);
    const BOSS: u32 = scratch(0x9a5e);
    match x.sub() {
        0 => {
            begin(x);
            x.wr16(TIMER, 0x80);
            x.fade_in();
            let effect = x.spawn(&pools::EFFCT, 0x24);
            effect.set_u8(x.b, Obj::VARIANT, 2);
            x.call(sfx::SCENE_MUSIC);
        }
        2 => {
            if x.tick(TIMER) {
                x.next_sub();
                x.message(86, MESSAGE);
            }
        }
        4 => {
            if x.message_done(MESSAGE) {
                if x.v8(0x5c) == 0 {
                    x.set_sub(12);
                    x.wr16(TIMER, 0x40);
                    x.call(0xb9aa0);
                    let a1 = x.c.a[1];
                    x.wr16(BOSS, a1 as u16);
                    x.call(sfx::COMMAND_E62FC);
                } else {
                    x.next_sub();
                    x.message(87, MESSAGE);
                    let effect = x.spawn(&pools::EFFCT, 0x50);
                    effect.set_u8(x.b, Obj::VARIANT, 6);
                }
            }
        }
        6 => {
            if x.message_done(MESSAGE) {
                x.next_sub();
                x.call(sfx::COMMAND_E62FC);
            }
        }
        8 => {
            if x.players_in_state(8) {
                x.next_sub();
                x.message(42, MESSAGE);
            }
        }
        10 => {
            if x.message_done(MESSAGE) {
                x.next_sub();
                x.call(0xb9aa0);
                let a1 = x.c.a[1];
                x.wr16(BOSS, a1 as u16);
            }
        }
        12 | 14 => {
            let boss = x.load_obj(BOSS);
            if boss.u8(x.b, 0x2f) != 0 {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Toutes les scènes ont une séquence d'ouverture.
    #[test]
    fn une_sequence_par_scene() {
        for scene in 0..=114u8 {
            let _ = sequence(scene);
        }
    }
}
