//! Tools: the natives of `crate::ai_aids` for the original code that calls them (arguments and results in the registers
//! of the original).

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::{Cpu, SR_C, SR_N, SR_V, SR_X, SR_Z};

use crate::ai_aids::*;
use crate::tools::frames::*;
use crate::{regs, Body, Native};
use crate::mem::{rd16, rd32, rd8, wr16, wr32, wr8, A5};
use crate::tools::ops::logic;
use crate::objects::Obj;
use crate::rng;
use crate::objects::pools;

/// `moveq #v,D0`: D0 and the flags N, Z (V and C zero).
fn moveq(c: &mut Cpu, v: i32) {
    c.d[0] = v as u32;
    logic(c, v as u32, Size::Long);
}

/// 0xb5e0 (`ai::target_in_front`) with the registers of the original: A1 = the target, D0 = 0/1/2 and flags of
/// `moveq`, D1-D3 left by the box tests. For the callers whose continuation rereads these leftovers (boss 11:
/// D2, original glitch).
pub fn target_in_front_68k(c: &mut Cpu, b: &mut Cps2Bus, o: Obj, hitbox: Hitbox) -> InHitbox {
    let pointer = rd16(b, o.0 + f::TARGET);
    let mut leftovers = Leftovers::default();
    let found = if pointer == 0 {
        InHitbox::No
    } else {
        c.a[1] = object(pointer);
        box_reach(b, o.0, object(pointer), hitbox, &mut leftovers)
    };
    leftovers.set(c);
    moveq(c, found as i32);
    found
}

/// 0x7f19c (`box_around`) with the registers of the original: D0 = 0/1/2 and flags of `moveq`, D1-D3 left by
/// the box tests.
pub fn box_around_68k(c: &mut Cpu, b: &mut Cps2Bus, me: Obj, other: Obj, hitbox: Hitbox) -> InHitbox {
    let mut leftovers = Leftovers::default();
    let found = box_reach(b, me.0, other.0, hitbox, &mut leftovers);
    leftovers.set(c);
    moveq(c, found as i32);
    found
}

/// 0x7f0c / 0x7ef4 (`in_front`) with the registers of the original: D0 (1 / 0) and flags of `moveq`, D1-D4, A1
/// and A4 left as in the original.
pub fn in_front_68k(c: &mut Cpu, b: &mut Cps2Bus, o: Obj, strict: bool, hitbox: Hitbox) -> bool {
    let (yes, leftovers, other, table, draw) = in_front_leftovers(b, o.0, strict, hitbox);
    if let Some(other) = other {
        c.a[1] = other;
    }
    leftovers.set(c);
    if let Some(table) = table {
        c.a[4] = table;
    }
    if let Some((mask, roll)) = draw {
        c.d[2] = mask;
        c.d[0] = roll;
    }
    moveq(c, yes as i32);
    yes
}

impl Leftovers {
    fn set(&self, c: &mut Cpu) {
        // in the original's order: bytes then words, each in its place
        if let Some(v) = self.d3_byte {
            set_b(&mut c.d[3], v);
        }
        if let Some(v) = self.d4_byte {
            set_b(&mut c.d[4], v);
        }
        for (r, v) in [(0, self.d0), (1, self.d1), (2, self.d2), (3, self.d3)] {
            if let Some(v) = v {
                set_w(&mut c.d[r], v);
            }
        }
        if let Some(v) = self.d2_byte {
            set_b(&mut c.d[2], v);
        }
    }
}
