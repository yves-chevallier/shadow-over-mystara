//! Tools: the natives of `crate::placement` for the original code that calls them (arguments and results in the registers
//! of the original).

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::{Cpu, SR_C, SR_N, SR_V, SR_X, SR_Z};

use crate::placement::*;
use crate::tools::frames::*;
use crate::{regs, Body, Native};
use crate::mem::{prog16, rd16, rd32, rd8, set_low_word, wr16, wr32, A5};
use crate::objects::Obj;

/// Native of 0x3668 (calls by address): leaves the registers of the original (D0, D1: height and depth
/// found, D2-D4, A1: last terrain examined, D5, D6: search counters).
fn place_on_free_ground_native(c: &mut Cpu, b: &mut Cps2Bus) {
    let r = search(b, Obj(c.a[0]));
    crate::video::terrain_regs(c, &r.last);
    c.d[5] = r.d5;
    c.d[6] = r.d6;
    set_low_word(&mut c.d[1], r.y);
    set_low_word(&mut c.d[0], r.height);
    rts(c, b);
}

/// `load_effect_palette` with the registers of the original: D0-D6 and A3 hold the eight longs copied, A1 and A2 the
/// destination and the next source, like the `movem` of the original.
pub(crate) fn load_effect_palette_regs(c: &mut Cpu, b: &mut Cps2Bus, offset: u16) {
    let (longs, next) = load_effect_palette(b, offset);
    c.d[..7].copy_from_slice(&longs[..7]);
    c.a[3] = longs[7];
    c.a[1] = EFFECT_PALETTE_RAM;
    c.a[2] = next;
}

fn effect_palette_native(c: &mut Cpu, b: &mut Cps2Bus, offset: u16) {
    load_effect_palette_regs(c, b, offset);
    rts(c, b);
}

const fn palette(entry: u32, name: &'static str, run: fn(&mut Cpu, &mut Cps2Bus)) -> Native {
    Native { entry, name, run: crate::Body::WithCpu(run), touches_sound: false, outputs: REGS_EFFECT_PALETTE }
}

pub const NATIVES: &[Native] = &[
    Native {
        entry: 0x3668,
        name: "player_placed_on_ground",
        run: crate::Body::WithCpu(place_on_free_ground_native),
        touches_sound: false,
        outputs: regs::d(0) | regs::d(1) | regs::d(2) | regs::d(5) | regs::d(6),
    },
    palette(0x3af8, "palette_effect_20", |c, b| effect_palette_native(c, b, effect_palette::LIGHT)),
    palette(0x3afe, "palette_effect_0", |c, b| effect_palette_native(c, b, effect_palette::NORMAL)),
    palette(0x3b04, "palette_effect_360", |c, b| effect_palette_native(c, b, effect_palette::P360)),
    palette(0x3b0c, "palette_effect_380", |c, b| effect_palette_native(c, b, effect_palette::P380)),
    palette(0x3b14, "palette_effect_3a0", |c, b| effect_palette_native(c, b, effect_palette::RED)),
];

/// Registers left by 0x3b1c (output of the natives; the Rust callers of the players do not read them back:
/// `DDSOM_POISON=placement`, batch D).
const REGS_EFFECT_PALETTE: u32 = regs::d(0) | regs::d(1) | regs::d(2) | regs::d(3) | regs::d(4) | regs::d(5) | regs::d(6) | regs::a(1) | regs::a(2) | regs::a(3);
