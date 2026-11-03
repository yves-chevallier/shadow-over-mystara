//! Type 3, substate 0: arrival according to variant `+3` (even values). 0 and 2 walk in (faster and faster), 4 fall
//! from above, 6 appear in place (grabbed on the first frame), 8 run.

use crate::families::cx::Bench;
use super::stance::stance_begin;
use super::*;
use crate::objects::Current;
use crate::objects::Obj;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::VARIANT) {
        0 => walk_in(x, 0x40, 0xd),
        2 => walk_in(x, 0x20, 0x17),
        4 => fall_in(x),
        6 => appear(x),
        8 => run_in(x),
        _ => {}
    }
}

/// End of a walk-in: when timer `+0xa4` ends, stand and make the first choice (0x5c17e).
fn end_of_walk<R: Bench>(x: &mut Cx<'_, R>) {
    let n = x.u8(Bow::TIMER).wrapping_sub(1);
    x.set8(Bow::TIMER, n);
    if n == 0 {
        x.set8(f::SUB, 2);
        x.set16(f::SUB2, 0);
        return stance_begin(x);
    }
    x.move_by_table_xy();
    x.anim_step();
}

/// Variants 0 (0x5ca68) and 2 (0x5c9ca): walks `timer` frames at speed `speed`. Also called by substate 4 when the Bow
/// Gnoll leaves the screen (variant 2).
pub(super) fn walk_in<R: Bench>(x: &mut Cx<'_, R>, timer: u8, speed: u32) {
    if x.u8(f::SUB2) != 0 {
        return end_of_walk(x);
    }
    x.add8(f::SUB2, 2);
    x.set8(Bow::TIMER, timer);
    walk_angle(x);
    set_speed(x, speed);
    mv(x, 2);
}

/// Variant 4: fall from above (`+0x18` = −4 added to the height every frame).
fn fall_in<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB2) == 0 {
        x.add8(f::SUB2, 2);
        x.set16(Obj::VZ, -4i16 as u16);
        x.set8(f::FLIP, 0);
        return mv(x, 0xf);
    }
    x.anim_step();
    let h = x.u16(Obj::Z).wrapping_add(x.u16(Obj::VZ));
    x.set16(Obj::Z, h);
    if (h as i16) < 0 {
        x.set8(f::SUB, 2);
        x.set16(f::SUB2, 0);
        x.set16(Obj::Z, 0);
    }
}

/// Variant 6: appear in place, waits for a player in the box (0, 0x80, 0x20).
fn appear<R: Bench>(x: &mut Cx<'_, R>) {
    if grabbed(x) {
        set_state_long(x, HELD);
        return;
    }
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            mv(x, 0xd);
        }
        2 => {
            x.anim_step();
            if !pick_target(x) {
                return;
            }
            if lib::target_in_box(x, 0, 0x80, 0x20) {
                x.add8(f::SUB2, 2);
            }
        }
        4 => {
            x.add8(f::SUB2, 2);
            mv(x, 0xc);
        }
        6 => {
            if x.anim_step() {
                set_state_long(x, STANCE);
            }
        }
        _ => {}
    }
}

/// Variant 8: run in.
fn run_in<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB2) == 0 {
        x.add8(f::SUB2, 2);
        x.set8(Bow::TIMER, 0x20);
        x.set8(Obj::SPEED_INDEX, 0xf);
        set_speed(x, 0xd);
        return mv(x, 5);
    }
    end_of_walk(x);
}
