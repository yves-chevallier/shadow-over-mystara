//! Type 15, substate 0: arrival according to variant `+3` (even values). 0 walk-in, 2 fall from above, 4 fall already
//! under way, 6 run from the edge of the screen followed by series of strikes.

use crate::families::cx::Bench;
use super::*;
use crate::objects::Current;
use crate::objects::Obj;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::VARIANT) {
        0 => walk_in(x),
        2 => drop_in(x, true),
        4 => drop_in(x, false),
        6 => run_in(x),
        _ => {}
    }
}

/// Variant 0: walks toward the screen for `+0xa2` frames.
fn walk_in<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB2) != 0 {
        x.move_by_table_xy();
        x.anim_step();
        let n = x.u8(Goblin::TIMER).wrapping_sub(1);
        x.set8(Goblin::TIMER, n);
        if (n as i8) < 0 {
            set_state_long(x, STANCE);
        }
        return;
    }
    x.add8(f::SUB2, 2);
    x.set8(Goblin::TIMER, 0x14);
    let angle = if x.u8(f::FLIP) != 0 { 8 } else { 0x18 };
    x.set8(Obj::SPEED_INDEX, angle);
    set_speed(x, 0x12);
    mv(x, 1);
}

/// Variants 2 and 4: jump from above. `gravity`: variant 2 waits for the landing (`+0xc`) before getting up, variant 4
/// gets up right away.
fn drop_in<R: Bench>(x: &mut Cx<'_, R>, gravity: bool) {
    match x.u8(f::SUB2) {
        0 => {
            x.set16(Obj::VZ, 0);
            x.set16(Obj::AZ, -0x40i16 as u16);
            x.add8(f::SUB2, 2);
            mv(x, 3);
        }
        2 => {
            if x.anim_step() {
                x.add8(f::SUB2, 2);
            }
        }
        4 => {
            if gravity {
                x.move_z();
                if (x.u16(Obj::Z) as i16) > 0 {
                    return;
                }
            }
            x.add8(f::SUB2, 2);
            x.set32(Obj::Z, 0);
            mv(x, 5);
        }
        6 => {
            if x.anim_step() {
                set_state_long(x, STANCE);
            }
        }
        _ => {}
    }
}

/// Variant 6: the Goblin runs in from the edge (position set from the camera), then strikes in series until grabbed.
fn run_in<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(Goblin::TIMER, 0x28);
            x.set8(Goblin::NO_ZONE, 1);
            let (angle, shift) = if x.u8(f::FLIP) != 0 { (8, -0x20i16) } else { (0x18, 0x1a0) };
            x.set8(Obj::SPEED_INDEX, angle);
            let cam = if var::CAMERA_SELECT.get(x.b) != 0 { var::CAMERA_ALT_X.get(x.b) } else { var::SCROLL_X.get(x.b) };
            x.set16(Obj::X, cam.wrapping_add(shift as u16));
            set_speed(x, 0x12);
            mv(x, 1);
        }
        2 => {
            x.anim_step();
            if x.u8(f::KEY_FRAME) == 0 {
                x.move_by_table_xy();
            }
            let n = x.u8(Goblin::TIMER).wrapping_sub(1);
            x.set8(Goblin::TIMER, n);
            if n != 0 {
                return;
            }
            x.add8(f::SUB2, 2);
            let t = rank_byte(x, SERIES_TABLE);
            x.set8(Goblin::TIMER, t);
            strike(x, 0);
        }
        4 => {
            if !x.anim_step() {
                if x.u8(f::KEY_FRAME) != 0 {
                    x.set8(f::KEY_FRAME, 0);
                    crate::families::eshl_creators::shot_0(&mut crate::families::effects_creators::leftovers::Pure, x.b, x.o);
                }
                return;
            }
            let n = x.u8(Goblin::TIMER).wrapping_sub(1);
            x.set8(Goblin::TIMER, n);
            if n != 0 {
                return strike(x, 0);
            }
            set_state_long(x, HELD);
            grab(x);
        }
        _ => {}
    }
}
