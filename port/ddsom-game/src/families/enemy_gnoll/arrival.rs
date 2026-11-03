//! Type 13, substate 0: arrival according to variant `+3` (even values). 0 walk-in, 2 and 6 fall from above, 4 side
//! jump, 8 armed arrival (puts down the axe), 10 armed run-in followed by the grab.

use crate::families::cx::Bench;
use super::*;
use crate::objects::Current;
use crate::objects::Obj;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::VARIANT) {
        0 => walk_in(x),
        2 => drop_in(x, true),
        4 => side_jump(x),
        6 => drop_in(x, false),
        8 => armed_in(x),
        10 => armed_run_in(x),
        _ => {}
    }
}

/// Variant 0: walks toward the screen for `+0xa2` frames.
fn walk_in<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB2) != 0 {
        x.move_by_table_xy();
        x.anim_step();
        let n = x.u8(Gnoll::TIMER).wrapping_sub(1);
        x.set8(Gnoll::TIMER, n);
        if (n as i8) < 0 {
            set_state_long(x, STANCE);
        }
        return;
    }
    x.add8(f::SUB2, 2);
    x.set8(Gnoll::TIMER, 0x14);
    let angle = if x.u8(f::FLIP) != 0 { 8 } else { 0x18 };
    x.set8(Obj::SPEED_INDEX, angle);
    set_speed(x, 0x12);
    mv(x, 1);
}

/// Variants 2 and 6: jump from above. `gravity`: variant 2 waits for the landing (`+0xc`) before getting up, variant 6
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

/// Variant 4: leap in from the side (horizontal speed −0x280), mid-air attack, landing.
fn side_jump<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set16(Obj::VZ, 0);
            x.set16(Obj::AZ, -0x38i16 as u16);
            x.set16(Obj::VX, -0x280i16 as u16);
            x.set16(Obj::AX, 0);
            if x.u8(f::FLIP) != 0 {
                negate(x, Obj::VX);
            }
            mv(x, 3);
        }
        2 => {
            if x.anim_step() {
                x.add8(f::SUB2, 2);
            }
        }
        4 => {
            x.add8(f::SUB2, 2);
            strike(x, 1);
        }
        6 => {
            x.move_xz();
            if (x.u16(Obj::Z) as i16) <= 0 {
                x.add8(f::SUB2, 2);
                x.set32(Obj::Z, 0);
                return mv(x, 5);
            }
            if (x.u16(Obj::Z) as i16) > 0x30 {
                return;
            }
            x.anim_step();
        }
        8 => {
            if x.anim_step() {
                set_state_long(x, STANCE);
            }
        }
        _ => {}
    }
}

/// The weapon is put down (key 9) or taken back (key 0xb) during the armed arrival animations.
fn weapon_key_drop<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::KEY_FRAME) == 9 {
        x.set8(f::KEY_FRAME, 0);
        x.set8(Gnoll::WEAPON, 0);
        lib::eshl_12(x);
    }
}

/// Variant 8: the Gnoll arrives running with its axe, drops it (animation 9) then takes it back.
fn armed_in<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(Gnoll::WEAPON, 1);
            x.set8(Gnoll::ARMED, 1);
            x.set8(Enemy::VULNERABLE, 0);
            x.set8(Gnoll::TIMER, 0x1a);
            set_speed(x, 0x17);
            let angle = if x.u8(f::FLIP) != 0 { 8 } else { 0x18 };
            x.set8(Obj::SPEED_INDEX, angle);
            strike(x, 4);
        }
        2 => armed_run(x),
        4 => {
            if x.anim_step() {
                x.add8(f::SUB2, 2);
                return mv(x, 9);
            }
            weapon_key_drop(x);
        }
        6 => {
            if x.anim_step() {
                return set_state_long(x, STANCE);
            }
            if x.u8(f::KEY_FRAME) == 0xb {
                lib::eshl_16(x, 1);
                x.set8(f::KEY_FRAME, 0);
                x.set8(Gnoll::ARMED, 0);
            }
        }
        _ => {}
    }
}

/// 0x6ae42: armed run for `+0xa2` frames, then animation 5.
fn armed_run<R: Bench>(x: &mut Cx<'_, R>) {
    x.move_by_table_xy();
    x.anim_step();
    let n = x.u8(Gnoll::TIMER).wrapping_sub(1);
    x.set8(Gnoll::TIMER, n);
    if (n as i8) >= 0 {
        return;
    }
    x.add8(f::SUB2, 2);
    strike(x, 5);
}

/// Variant 10: the Gnoll enters from the edge of the screen with its axe; grabbed as soon as the run is over.
fn armed_run_in<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(Gnoll::WEAPON, 1);
            x.set8(Gnoll::ARMED, 1);
            x.set8(Enemy::VULNERABLE, 0);
            x.set8(Gnoll::TIMER, 0x1a);
            set_speed(x, 0x17);
            let (angle, shift) = if x.u8(f::FLIP) != 0 { (8, -0x20i16) } else { (0x18, 0x1a0) };
            x.set8(Obj::SPEED_INDEX, angle);
            let cam = if var::CAMERA_SELECT.get(x.b) != 0 { var::CAMERA_ALT_X.get(x.b) } else { var::SCROLL_X.get(x.b) };
            x.set16(Obj::X, cam.wrapping_add(shift as u16));
            strike(x, 4);
        }
        2 => armed_run(x),
        4 => {
            if x.anim_step() {
                x.set16(f::STATE, 0x0408);
                x.set16(f::SUB2, 0);
                return grab(x);
            }
            weapon_key_drop(x);
        }
        _ => {}
    }
}
