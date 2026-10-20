//! Aides de création d'effets (EFFCT) appelées par les handlers des joueurs et des décors.

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::{Cpu, SR_C};

use crate::call;
use crate::mem::{rd16, rd32, rd8, rts, wr16, wr32, wr8, A5};
use crate::ops::logique;
use crate::{regs, Native};

/// Allocation d'un effet : A1 = emplacement, retenue levée si le pool est plein.
const ALLOC_EFFECT: u32 = 0x2318;
/// Scène dont les effets de choc ne sont pas créés.
const SCENE: u32 = 0x34;
const NO_SHOCK_SCENE: u8 = 41;

/// Type des effets créés ici.
const SHOCK_KIND: u8 = 25;
const SPARK_KIND: u8 = 4;

/// 0xb10c8 : crée un effet de choc à la position de l'objet courant (A0) : D0 = variante,
/// D1 = deux octets de réglage (bas : `+0x2d`, haut : `+0x2f`), D2 et D3 = vitesses (`+0x14`, `+0x1c`).
fn shock(c: &mut Cpu, b: &mut Cps2Bus) {
    if rd8(b, A5 + SCENE) != NO_SHOCK_SCENE {
        call(c, b, ALLOC_EFFECT);
        if c.sr & SR_C == 0 {
            let (me, fx) = (c.a[0], c.a[1]);
            wr8(b, fx, 1);
            wr8(b, fx + 2, SHOCK_KIND);
            for field in [0x08, 0x10, 0x0c] {
                let v = rd32(b, me + field);
                wr32(b, fx + field, v);
            }
            wr16(b, fx + 0x14, c.d[2] as u16);
            wr16(b, fx + 0x1c, c.d[3] as u16);
            let facing = rd8(b, me + 0x2c);
            wr8(b, fx + 0x2c, facing);
            wr8(b, fx + 3, c.d[0] as u8);
            wr8(b, fx + 0x2d, c.d[1] as u8);
            let high = (c.d[1] as u16) >> 8;
            crate::mem::set_low_word(&mut c.d[1], high);
            wr8(b, fx + 0x2f, high as u8);
            wr16(b, fx + 0x42, 0);
            logique(c, 0, Size::Word);
        }
    }
    rts(c, b);
}

/// 0xaa96c : crée une étincelle (type 4) liée à l'objet courant, sauf pour un joueur dans certains
/// états. D1 = variante. A1 à A4 sont rendus.
fn spark(c: &mut Cpu, b: &mut Cps2Bus) {
    let saved = [c.a[1], c.a[2], c.a[3], c.a[4]];
    let me = c.a[0];
    let skip = (rd8(b, me + 0x3c) as i8) < 2 && rd8(b, me + 0x10d).wrapping_sub(16) <= 3;
    if !skip {
        c.a[2] = c.a[1];
        call(c, b, ALLOC_EFFECT);
        if c.sr & SR_C == 0 {
            let fx = c.a[1];
            wr8(b, fx, 1);
            wr8(b, fx + 2, SPARK_KIND);
            wr8(b, fx + 3, c.d[1] as u8);
            let facing = rd8(b, me + 0x2c) ^ 32;
            wr8(b, fx + 0x2c, facing);
            wr16(b, fx + 0x50, me as u16);
            wr16(b, fx + 0x54, c.a[2] as u16);
            call(c, b, 0xab2b4);
        }
    }
    [c.a[1], c.a[2], c.a[3], c.a[4]] = saved;
    rts(c, b);
}

/// Allocation d'un objet de décor (SOBJ) et d'un effet de joueur (PEFF).
const ALLOC_SOBJ: u32 = 0x2408;
const ALLOC_PEFF: u32 = 0x228e;

/// Crée un objet de type `kind` dans le pool alloué par `alloc` : variante D0. Rend l'objet dans A1 ;
/// la retenue levée signale un pool plein.
fn spawn(c: &mut Cpu, b: &mut Cps2Bus, alloc: u32, kind: u8) -> Option<u32> {
    call(c, b, alloc);
    if c.sr & SR_C != 0 {
        return None;
    }
    let object = c.a[1];
    wr8(b, object, 1);
    wr8(b, object + 2, kind);
    wr8(b, object + 3, c.d[0] as u8);
    logique(c, c.d[0] & 0xff, Size::Byte);
    Some(object)
}

/// 0xddeaa : décor de type 6.
fn spawn_scenery_6(c: &mut Cpu, b: &mut Cps2Bus) {
    spawn(c, b, ALLOC_SOBJ, 6);
}

/// 0xdfa4a : décor de type 11, orienté selon D1.
fn spawn_scenery_11(c: &mut Cpu, b: &mut Cps2Bus) {
    if let Some(object) = spawn(c, b, ALLOC_SOBJ, 11) {
        wr8(b, object + 0x2c, c.d[1] as u8);
    }
}

/// 0x517ba : effet de joueur de type 44 ; son réglage `+0x2f` est l'octet suivant dans la table A4.
fn spawn_player_effect_44(c: &mut Cpu, b: &mut Cps2Bus) {
    if let Some(object) = spawn(c, b, ALLOC_PEFF, 44) {
        let setting = rd8(b, c.a[4]);
        c.a[4] = c.a[4].wrapping_add(1);
        wr8(b, object + 0x2f, setting);
    }
}

/// 0xcc18e : toutes les 32 images, compte les joueurs vivants dans `A5+0x35`.
fn count_living_players(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = c.a[0];
    let timer = rd16(b, me + 0x20).wrapping_sub(1);
    wr16(b, me + 0x20, timer);
    if (timer as i16) >= 0 {
        return;
    }
    wr16(b, me + 0x20, 32);
    let alive = (0..4).filter(|k| rd8(b, A5 + 0x5de + k * 0x200) != 0 && rd16(b, A5 + 0x5de + k * 0x200 + 0x62) != 0).count();
    wr8(b, A5 + 0x35, alive as u8);
    c.d[0] = alive as u32;
}

/// 0x25260 : un joueur scripté se place devant l'objet (`A5+0x46`), décalé selon son numéro (`+2`).
fn place_scripted_player(c: &mut Cpu, b: &mut Cps2Bus) {
    const OFFSETS: [u16; 4] = [0x0040, 0x0010, 0xffe8, 0xffbc];
    let me = c.a[0];
    let focus = rd16(b, A5 + 0x46) as i16 as i32 as u32;
    c.a[1] = focus;
    let (x, depth) = (rd16(b, focus + 0x08), rd16(b, focus + 0x10));
    wr16(b, me + 0x08, x);
    wr16(b, me + 0x10, depth);
    wr16(b, me + 0x0c, 0);
    let shift = OFFSETS[rd8(b, me + 2) as usize & 3];
    wr16(b, me + 0x08, x.wrapping_add(shift));
    wr16(b, me + 0x10, depth.wrapping_sub(18));
}

const fn aid(entry: u32, name: &'static str, run: fn(&mut Cpu, &mut Cps2Bus), outputs: u32) -> Native {
    Native { entry, name, cost: |_, _| 0, run, touches_sound: false, outputs, exact_cost: false }
}

macro_rules! leaf {
    ($entry:literal, $name:literal, $body:path, $outputs:expr) => {
        aid($entry, $name, |c, b| {
            $body(c, b);
            rts(c, b);
        }, $outputs)
    };
}

pub const NATIVES: &[Native] = &[
    leaf!(0xddeaa, "decor_type_6", spawn_scenery_6, regs::a(1) | regs::CCR),
    leaf!(0xdfa4a, "decor_type_11", spawn_scenery_11, regs::a(1) | regs::CCR),
    leaf!(0x517ba, "effet_joueur_44", spawn_player_effect_44, regs::a(1) | regs::a(4) | regs::CCR),
    leaf!(0xcc18e, "decompte_joueurs_vivants", count_living_players, regs::d(0)),
    leaf!(0x25260, "joueur_scripte_place", place_scripted_player, regs::a(1)),
    aid(0xb10c8, "effet_choc", shock, regs::a(1) | regs::d(1) | regs::CCR),
    aid(0xaa96c, "effet_etincelle", spark, regs::a(1) | regs::a(2) | regs::a(3) | regs::a(4)),
];
