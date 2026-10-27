//! Tâche de démarrage (`0x11b16`) : initialisation du jeu, deux écrans d'avertissement selon la région,
//! création des tâches des crédits et du gel, puis la machine à modes.
//!
//! Chaque avertissement reste à l'écran deux ou trois secondes ; un bouton (bit 4 du port `IN1`) accélère
//! le décompte. Le texte dépend de la région (octet 0xc4 du programme et réglage `A5+0x70`).

use super::Sequence;
use crate::mem::{rd16, rd32, rd8, set_low_word, wr16, wr32, wr8, A5};
use crate::scene::Ctx;
use crate::Native;

mod ext {
    pub const CLEAR_VIDEO: u32 = 0xc32;
    pub const PALETTES: u32 = 0x1a296;
    pub const BRIGHTNESS_WHITE: u32 = 0x1686;
    pub const SERVICE_CHECK: u32 = 0x2d30a;
    pub const INIT_STEPS_A: [u32; 6] = [0x19aa2, 0x19abc, 0x19db4, 0xee044, 0x2d9e, 0x2df8];
    pub const INIT_RNG: u32 = 0x19a80;
    pub const INIT_STEP_LAST: u32 = 0x19038;
    pub const CLEAR_STATUS: u32 = 0xdabba;
    pub const PRINT: u32 = 0xd5e;
    pub const CLEAR_SCROLL_1: u32 = 0xca8;
    pub const CONFIG_2DC9A: u32 = 0x2dc9a;
    pub const TASK_CREATE: u32 = 0x9b6;
    pub const CREDITS_TASK: u32 = 0x13c0a;
    pub const FREEZE_TASK: u32 = 0xee080;
    pub const MODE_LOOP: u32 = 0x133fe;
}

/// Palette de départ : huit longs de la ROM copiés dans la table des palettes.
const START_PALETTE_SRC: u32 = 0x133ba;
const START_PALETTE_DST: u32 = 0x91c580;
/// Région du jeu (octet du programme) et tables de textes (une par région, indexées par `A5+0x70`).
const REGION_BYTE: u32 = 0xc4;
const WARNING_TABLES: [u32; 3] = [0x11cba, 0x11cd2, 0x11cea];
const NOTICE_TABLE: u32 = 0x11d02;
/// Port des boutons (bit 4 : un bouton pressé accélère le décompte).
const PORT_IN1: u32 = 0x804001;
/// Décompte des écrans d'avertissement : mot à FF0E6E.
const COUNTDOWN: u32 = A5 - 0x7192;
const WARNING_FRAMES: u16 = 0x78;
const NOTICE_FRAMES: u16 = 0xb4;
const CPS_REG_VIDEO: u32 = 0x804122;
const CPS_REG_VIDEO_B: u32 = 0x804166;
const SLOT_CREDITS: u16 = 0x20;
const SLOT_FREEZE: u16 = 0xe0;

pub const BOOT: Native = Native {
    entry: 0x11b16,
    name: "boot_task",
    run: |c, b| {
        let mut x = Ctx::new(c, b);
        x.c.a[5] = A5;
        x.tail(ext::CLEAR_VIDEO, Etape::Init);
    },
    touches_sound: false,
    outputs: 0,
};

const INIT: u32 = 0x11b22;
crate::etapes! {
    /// Étapes (points de reprise) de la machine à états.
    pub enum Etape {
        Init = INIT => |x| {
            x.call(ext::PALETTES);
            for i in 0..8 {
                let v = rd32(x.b, START_PALETTE_SRC + 4 * i);
                wr32(x.b, START_PALETTE_DST + 4 * i, v);
            }
            wr8(x.b, A5 + 0x48e, 0xf);
            x.call(ext::BRIGHTNESS_WHITE);
            x.call(ext::SERVICE_CHECK);
            crate::credits::initialiser(x.c, x.b);
            for f in ext::INIT_STEPS_A {
                x.call(f);
            }
            clear_mode_state(x);
            x.call(ext::INIT_RNG);
            default_high_scores(x);
            x.call(ext::INIT_STEP_LAST);
            x.call(ext::CLEAR_STATUS);
            crate::taches::partie::effacer_variables(x.c, x.b);
            wr8(x.b, A5 - 0x713c, 0);
            set_low_word(&mut x.c.d[0], 0xe);
            wr16(x.b, CPS_REG_VIDEO, 0xe);
            wr16(x.b, A5 + 0x12, 0xe);
            set_low_word(&mut x.c.d[0], 0x12ce);
            wr16(x.b, CPS_REG_VIDEO_B, 0x12ce);
            wr16(x.b, A5 + 0x14, 0x12ce);
            let region = rd8(x.b, REGION_BYTE);
            let table = match region {
                0 => WARNING_TABLES[0],
                1 => WARNING_TABLES[1],
                _ => WARNING_TABLES[2],
            };
            print_region_text(x, table);
            wr16(x.b, COUNTDOWN, WARNING_FRAMES);
            x.wait(1, Etape::WarningTick)
        },
        WarningTick = WARNING_TICK => |x| {
            if countdown_tick(x) {
                return x.wait(1, Etape::WarningTick);
            }
            x.call(ext::CLEAR_SCROLL_1);
            print_region_text(x, NOTICE_TABLE);
            wr16(x.b, COUNTDOWN, NOTICE_FRAMES);
            x.wait(1, Etape::NoticeTick)
        },
        NoticeTick = NOTICE_TICK => |x| {
            if countdown_tick(x) {
                return x.wait(1, Etape::NoticeTick);
            }
            x.call(ext::CLEAR_SCROLL_1);
            wr16(x.b, A5 + 0x7fc6, 0);
            if rd8(x.b, A5 + 4) & 1 != 0 {
                wr16(x.b, A5 + 0x7fc6, 1);
            }
            x.call(ext::CONFIG_2DC9A);
            x.call(ext::SERVICE_CHECK);
            x.c.a[0] = ext::CREDITS_TASK;
            set_low_word(&mut x.c.d[0], SLOT_CREDITS);
            x.call(ext::TASK_CREATE);
            x.c.a[0] = ext::FREEZE_TASK;
            set_low_word(&mut x.c.d[0], SLOT_FREEZE);
            x.call(ext::TASK_CREATE);
            x.jump_to(ext::MODE_LOOP)
        },
    }
}

/// État de la machine à modes (16 longs en `A5-0x7190`) et `A5-0x54f0..` (4 longs) à zéro (`0x133dc`, puis
/// `0xfc562`).
fn clear_mode_state(x: &mut Ctx) {
    const MODES: u32 = A5 - 0x7190;
    const SCENE_SCRIPT: u32 = A5 - 0x54f0;
    for i in 0..16 {
        wr32(x.b, MODES + 4 * i, 0);
    }
    for i in 0..4 {
        wr32(x.b, SCENE_SCRIPT + 4 * i, 0);
    }
    x.c.d[0] = 0;
    x.c.d[6] |= 0xffff;
    x.c.a[6] = MODES + 0x40;
}

/// Tableau des meilleurs scores par défaut (`0x18082`) : dix entrées de 14 octets copiées de la ROM vers
/// FF0FD0 ; une autre table pour la version à palette alternative (`A5+0x70`).
fn default_high_scores(x: &mut Ctx) {
    const TABLES: [u32; 2] = [0x18f1e, 0x18faa];
    const SCORES: u32 = 0xff0fd0;
    const SIZE: u32 = 10 * 14;
    let src = TABLES[(rd16(x.b, A5 + 0x70) != 0) as usize];
    for i in (0..SIZE).step_by(2) {
        let w = rd16(x.b, src + i);
        wr16(x.b, SCORES + i, w);
    }
    x.c.a[1] = src + SIZE;
    x.c.a[2] = SCORES + SIZE;
    x.c.d[6] |= 0xffff;
}

/// Affiche le texte de la région courante : mot `A5+0x70` (modulo 16) indexe la table de pointeurs.
fn print_region_text(x: &mut Ctx, table: u32) {
    let region = (rd16(x.b, A5 + 0x70) & 0xf) as u32;
    x.c.a[4] = rd32(x.b, table + 2 * region);
    x.call(ext::PRINT);
}

/// Un tour du décompte : un bouton pressé le raccourcit de neuf images.
fn countdown_tick(x: &mut Ctx) -> bool {
    let mut left = rd16(x.b, COUNTDOWN) as i16;
    if rd8(x.b, PORT_IN1) & 0x10 == 0 {
        left = left.wrapping_sub(8);
    }
    left = left.wrapping_sub(1);
    wr16(x.b, COUNTDOWN, left as u16);
    left > 0
}

const WARNING_TICK: u32 = 0x11c06;

const NOTICE_TICK: u32 = 0x11c4a;

pub const NATIVES: &[Native] = &[BOOT];
