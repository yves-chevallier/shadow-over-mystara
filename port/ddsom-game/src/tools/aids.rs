//! Tools: the natives of `crate::aids` for the original code that calls them (arguments and results in the registers
//! of the original).

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::Cpu;

use crate::aids::*;
use crate::tools::frames::*;
use crate::{regs, Native};
use crate::mem::{rd16, rd8};

/// Native of 0x1386: work registers as the original leaves them (D1 to D4 and D6), D0 = the direction.
fn direction_native(c: &mut Cpu, b: &mut Cps2Bus) {
    let r = compute(b, c.d[1], c.d[2], c.d[3], c.d[4]);
    if let Some(d6) = r.d6 {
        c.d[6] = d6;
    }
    c.d[1] = r.d1;
    c.d[2] = r.d2;
    c.d[3] = r.d3;
    // `add.l #0x8000,D4` (carry in the high word), then the unit in D4.w
    c.d[4] = c.d[4].wrapping_add(0x8000);
    set_low_word(&mut c.d[4], r.unit);
    c.d[0] = r.direction as u32;
    crate::tools::ops::logic(c, r.direction as u32, Size::Byte);
    rts(c, b);
}

/// 0x137e: direction from object A0 (D3, D4 = its x and z) to (D1, D2).
fn direction_from_object(c: &mut Cpu, b: &mut Cps2Bus) {
    let (x, z) = (rd16(b, c.a[0] + 8), rd16(b, c.a[0] + 0x10));
    set_low_word(&mut c.d[3], x);
    set_low_word(&mut c.d[4], z);
    direction_native(c, b);
}

/// 0x1376: direction from object A0 to object A1.
fn direction_to_object(c: &mut Cpu, b: &mut Cps2Bus) {
    let (x, z) = (rd16(b, c.a[1] + 8), rd16(b, c.a[1] + 0x10));
    set_low_word(&mut c.d[1], x);
    set_low_word(&mut c.d[2], z);
    direction_from_object(c, b);
}

pub(crate) const DIRECTION_FROM_OBJECT: Native = Native {
    entry: 0x137e,
    name: "direction_object",
    run: crate::Body::WithCpu(direction_from_object),
    touches_sound: false,
    outputs: regs::d(0) | regs::CCR,
};

pub(crate) const DIRECTION_TO_OBJECT: Native = Native {
    entry: 0x1376,
    name: "direction_to_object",
    run: crate::Body::WithCpu(direction_to_object),
    touches_sound: false,
    outputs: regs::d(0) | regs::CCR,
};

pub(crate) const DIRECTION: Native = Native {
    entry: 0x1386,
    name: "direction",
    run: crate::Body::WithCpu(direction_native),
    touches_sound: false,
    outputs: regs::d(0) | regs::CCR,
};

/// Native: D0.w = the word `+0x46`, A1 = the player (word sign-extended), flags of the last test.
fn grabbed_native(c: &mut Cpu, b: &mut Cps2Bus) {
    if grabbed(b, crate::objects::Obj(c.a[0])) == NameEntry::Frozen {
        crate::tools::ops::logic(c, 1, Size::Byte);
    } else {
        let holder = rd16(b, c.a[0] + 0x46);
        set_low_word(&mut c.d[0], holder);
        crate::tools::ops::logic(c, holder as u32, Size::Word);
        if holder != 0 {
            c.a[1] = holder as i16 as i32 as u32;
            let state = rd8(b, c.a[1] + 0x2c);
            crate::tools::ops::logic(c, state as u32, Size::Byte);
        }
    }
    rts(c, b);
}

pub(crate) const GRABBED: Native = Native {
    entry: 0x2810,
    name: "object_seized",
    run: crate::Body::WithCpu(grabbed_native),
    touches_sound: false,
    outputs: regs::d(0) | regs::a(1) | regs::CCR,
};

fn game_over_native(c: &mut Cpu, b: &mut Cps2Bus) {
    game_over(b);
    rts(c, b);
}

pub(crate) const GAME_OVER: Native = Native {
    entry: 0x28d8,
    name: "game_over_check",
    run: crate::Body::WithCpu(game_over_native),
    touches_sound: false,
    outputs: 0,
};

/// D0 / D1: quotient in D0, remainder in D2; D3 = 0xffff (loop counter).
fn divide_native(c: &mut Cpu, b: &mut Cps2Bus) {
    let (q, r) = divide(c.d[0], c.d[1]);
    c.d[0] = q;
    c.d[2] = r;
    c.d[3] = 0xffff;
    rts(c, b);
}

pub(crate) const DIVIDE: Native = Native {
    entry: 0x17ea,
    name: "division",
    run: crate::Body::WithCpu(divide_native),
    touches_sound: false,
    outputs: regs::d(0) | regs::d(2) | regs::d(3),
};
