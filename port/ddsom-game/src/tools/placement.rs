//! Tools: the natives of `crate::world::placement` for the original code that calls them (arguments and results in the registers
//! of the original).

use crate::world::World;
use m68k::Cpu;

use crate::world::placement::*;
use crate::tools::frames::*;
use crate::{regs, Native};
use crate::hw::mem::{rd16, set_low_word};
use crate::world::objects::Obj;

/// The search of `place_on_free_ground` replayed (it reads the terrain only): the last terrain examined and the
/// `dbf` counters D5 (shifts in x, from 4) and D6 (shifts in depth, from 9) at its end; a counter whose loop ran out
/// has a low word of -1.
fn search_end(b: &mut World, o: Obj) -> (crate::world::terrain::Terrain, u32, u32) {
    let x0 = start_x(b, o);
    let mut last = None;
    for i in 0..5u16 {
        let mut x = if i == 0 { x0 } else { x0.wrapping_add(16) };
        let y0 = start_y(b, o);
        for j in 0..10u16 {
            let t = crate::world::terrain::terrain(b, x, if j == 0 { y0 } else { y0.wrapping_add(16) });
            if is_free(&t) {
                return (t, (4 - i) as u32, (9 - j) as u32);
            }
            x = t.x;
            last = Some(t);
        }
    }
    (last.expect("at least one attempt"), 0xffff, 0xffff)
}

/// Native of 0x3668 (calls by address): leaves the registers of the original (D0, D1: height and depth
/// found, D2-D4, A1: last terrain examined, D5, D6: search counters).
fn place_on_free_ground_native(c: &mut Cpu, b: &mut World) {
    let o = Obj(c.a[0]);
    let (last, d5, d6) = search_end(b, o);
    place_on_free_ground(b, o);
    crate::world::terrain::terrain_regs(c, &last);
    c.d[5] = d5;
    c.d[6] = d6;
    set_low_word(&mut c.d[1], rd16(b, o.0 + 0x10));
    set_low_word(&mut c.d[0], rd16(b, o.0 + 0x0c));
    rts(c, b);
}

/// `load_effect_palette` with the registers of the original: D0-D6 and A3 hold the eight longs copied, A1 and A2 the
/// destination and the next source, like the `movem` of the original.
pub(crate) fn load_effect_palette_regs(c: &mut Cpu, b: &mut World, offset: u16) {
    let (longs, next) = load_effect_palette(b, offset);
    c.d[..7].copy_from_slice(&longs[..7]);
    c.a[3] = longs[7];
    c.a[1] = EFFECT_PALETTE_RAM.0;
    c.a[2] = next;
}

fn effect_palette_native(c: &mut Cpu, b: &mut World, offset: u16) {
    load_effect_palette_regs(c, b, offset);
    rts(c, b);
}

const fn palette(entry: u32, name: &'static str, run: fn(&mut Cpu, &mut World)) -> Native {
    Native { entry, name, run: crate::Body::WithCpu(run), touches_sound: false, outputs: REGS_EFFECT_PALETTE }
}

pub(crate) const NATIVES: &[Native] = &[
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
