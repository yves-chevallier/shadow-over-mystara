//! Boss 15 / 20, group `+5` = 0: arrival. The dragon falls from the top of the screen, 0xc0 pixels to the right of
//! the camera, stops at altitude 0x40, then the combat begins.
//!
//! | `+6` | Role |
//! |---|---|
//! | 0 | placement above the screen, delay (0xa0 frames; 0x10 for the small dragon) |
//! | 2 | waits, then starts descending |
//! | 4 | descends to altitude 0x40 |

use crate::families::cx::Bench;
use super::*;
use crate::objects::Current;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => place(x),
        2 => wait_then_fall(x),
        4 => fall(x),
        _ => {}
    }
}

fn at_camera<R: Bench>(x: &mut Cx<'_, R>) {
    let cx = var::SCROLL_X.get(x.b).wrapping_add(0xc0);
    x.set16(0x8, cx);
}

fn place<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB2, 2);
    at_camera(x);
    x.set16(0x10, 0x1a0);
    x.set16(0xc, 0xc0);
    x.set32(0x14, 0);
    x.set32(0x1c, 0);
    x.set16(0x18, (-0x400i16) as u16);
    x.set16(0x1a, 0);
    let wait = if x.u8(TYPE) == TYPE_SMALL { 0x10 } else { 0xa0 };
    x.set16(TIMER, wait);
    rest(x);
}

fn wait_then_fall<R: Bench>(x: &mut Cx<'_, R>) {
    if (x.u16(TIMER) as i16) > 0 {
        let t = x.u16(TIMER) - 1;
        x.set16(TIMER, t);
        return;
    }
    at_camera(x);
    x.move_z();
    if (x.u16(0xc) as i16) > 0x40 {
        return;
    }
    x.add8(f::SUB2, 2);
    x.set16(0x1a, 0x80);
    fall(x);
}

fn fall<R: Bench>(x: &mut Cx<'_, R>) {
    at_camera(x);
    x.move_z();
    if (x.u16(0xc) as i16) < 0x40 {
        return;
    }
    x.set16(0xc, 0x40);
    x.add8(f::SUB, 2);
    x.set16(f::SUB2, 0);
    if x.u8(TYPE) != TYPE_BIG {
        lib::sound(x, crate::sound::CMD_23);
    }
    x.set8(0, 1);
    lib::set_immunities(x);
    rest(x);
    // 0xb3fb0: the setting is D1.b, which `set_anim` (0x93182) has just set to the animation set number (0)
    crate::families::effects_creators::scenery_5(x.b, x.o, 0);
    // the arrival over, the combat begins on the same frame
    super::fight::run(x);
}
