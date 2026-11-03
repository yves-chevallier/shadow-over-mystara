//! Type 6, substate 2: standing. `+6`: 0 and 8 approach and choice of an attack, 2 turn-around, 4 wait, 6 fire spit
//! (the dog backs off while breathing), 10 side jump.

use crate::families::cx::Bench;
use super::*;
use crate::objects::Current;
use crate::objects::Obj;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB2) != 10 && grabbed(x) {
        return set_state_long(x, HELD);
    }
    match x.u8(f::SUB2) {
        0 | 8 => approach(x),
        2 => turn(x),
        4 => wait(x),
        6 => spit(x),
        10 => hop(x),
        _ => {}
    }
    if x.u16(Obj::Z) == 0 && var::PLAYERS_ALIVE.get(x.b) == 0 {
        set_state_long(x, IDLE);
    }
}

// --- +6 = 0 and 8: approach -----------------------------------------------------------------------------

fn approach<R: Bench>(x: &mut Cx<'_, R>) {
    let dx = x.u16(Obj::X).wrapping_sub(var::SCROLL_X.get(x.b));
    if dx >= 0x180 {
        // far from the camera: turns toward the target then draws its move at random
        let mut flip = 0;
        if x.u16(f::TARGET) != 0 {
            let target = x.linked(f::TARGET);
            if (x.u16(Obj::X).wrapping_sub(target.u16(x.b, Obj::X)) as i16) < 0 {
                flip = 0x20;
            }
        }
        x.set8(f::FLIP, flip);
        let mask = tables::table("hellhound/turn_mask").at32(x.b, 0);
        let r = rand_and(x, 0x1f);
        let next = if mask >> r & 1 != 0 { 0x0202_0600 } else { 0x0202_0a00 };
        return set_state_long(x, next);
    }
    if target_far_in_line(x) {
        return set_state_long(x, 0x0202_0600);
    }
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(Enemy::VULNERABLE, 1);
        x.set8(Hellhound::LOOK, 0xc);
        mv(x, 2);
        set_speed(x, 0x11);
        return lib::draw_stance(x);
    }
    let n = x.u8(Enemy::AI_STANCE).wrapping_sub(1);
    x.set8(Enemy::AI_STANCE, n);
    if (n as i8) < 0 {
        lib::draw_stance(x);
        pick_target(x);
        let kind = if x.u8(f::SUB2) == 0 { 0x11 } else { 0x12 };
        x.set8(Enemy::AI_POINT_SET, kind);
        lib::approach_target(x);
        let w = x.u8(Enemy::AI_WANDER).wrapping_sub(1);
        x.set8(Enemy::AI_WANDER, w);
        if w == 0 {
            lib::draw_6b(x);
            if lib::maybe_draw_6e(x) {
                return x.set16(f::SUB2, 0x400);
            }
        }
    }
    reach(x);
}

/// 0x61a9e: the dog is on guard; a player in range? Immediate attack, otherwise walking step.
pub(super) fn reach<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Hellhound::GUARD, 2);
    reach_boxes(x);
}

/// 0x61aa4: successive range boxes (immediate attack, frontal attack, far attack, announcement, walk).
pub(super) fn reach_boxes<R: Bench>(x: &mut Cx<'_, R>) {
    if lib::target_in_box(x, -0x28, 0x28, 0xc) {
        x.set16(f::STATE, 0x0204);
        lib::pick_byte(x);
        return pick_attack(x, 0);
    }
    if front_attack(x) || far_attack(x) {
        return;
    }
    if lib::guard_in_front_strict(x, -0x40, 0x40, 0x10) {
        return lib::pick_byte(x);
    }
    x.anim_step();
    x.move_by_table_xy();
    if x.u16(f::TARGET) == 0 {
        return;
    }
    let target = x.linked(f::TARGET);
    let side = if (x.u16(Obj::X).wrapping_sub(target.u16(x.b, Obj::X)) as i16) < 0 { 0x20 } else { 0 };
    if side != x.u8(f::FLIP) {
        x.set32(Hellhound::RESUME_STATE, STANCE);
        x.set8(Hellhound::RESUME_SUB2, 0);
        set_state_long(x, 0x0202_0200);
    }
}

/// Attack decision according to the relative orientation of the target (0xa4c2 same direction, 0xa4b4 otherwise).
fn gate<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let target = x.linked(f::TARGET);
    let theirs = target.u8(x.b, f::FLIP);
    let same = theirs == x.u8(f::FLIP);
    lib::decision(x, same)
}

/// 0x61b3a: a player in front, close enough: tail strike or move.
fn front_attack<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if !lib::target_in_box(x, -0x40, 0x40, 0x10) || x.u16(f::TARGET) == 0 || !gate(x) {
        return false;
    }
    let r = rand_and(x, 0xf);
    if (0xffc0u32 >> r) & 1 != 0 {
        set_state_long(x, 0x0202_0800);
    } else {
        set_state_long(x, 0x0204_0000);
        lib::pick_byte(x);
        pick_attack(x, 0);
    }
    true
}

/// 0x61bb4: a player far in front: move from the far series.
fn far_attack<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if !lib::target_in_box(x, -0xc0, 0x40, 0x10) || x.u16(f::TARGET) == 0 || !gate(x) {
        return false;
    }
    let r = rand_and(x, 2);
    if r != 0 {
        set_state_long(x, 0x0202_0800);
    } else {
        set_state_long(x, 0x0204_0000);
        lib::pick_byte(x);
        pick_attack(x, 8);
    }
    true
}

// --- +6 = 2 : demi-tour ------------------------------------------------------------------------------------

fn turn<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(Enemy::VULNERABLE, 1);
        look(x, 4);
        return mv(x, 4);
    }
    if !x.anim_step() {
        return;
    }
    let s = x.u32(Hellhound::RESUME_STATE);
    x.set32(f::STATE, s);
    let sub2 = x.u8(Hellhound::RESUME_SUB2);
    x.set8(f::SUB2, sub2);
    let flip = x.u8(f::FLIP) ^ 0x20;
    x.set8(f::FLIP, flip);
    mv(x, 0);
}

// --- +6 = 4 : attente -----------------------------------------------------------------------------------------

fn wait<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(Enemy::VULNERABLE, 1);
        look(x, 0);
        return mv(x, 0);
    }
    let n = x.u8(Enemy::AI_WAIT).wrapping_sub(1);
    x.set8(Enemy::AI_WAIT, n);
    if (n as i8) < 0 {
        return set_state_long(x, STANCE);
    }
    if must_turn(x) {
        x.set32(Hellhound::RESUME_STATE, STANCE);
        x.set8(Hellhound::RESUME_SUB2, 4);
        set_state_long(x, 0x0202_0200);
    }
    x.anim_step();
}

// --- +6 = 6 : crachat ---------------------------------------------------------------------------------------

fn spit<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        4 => {
            x.add8(f::SUB3, 0xfe);
            spit_begin(x);
        }
        0 => {
            x.add8(f::SUB3, 2);
            x.set8(Hellhound::TIMER, 0x34);
            spit_begin(x);
        }
        2 => {
            puff(x, 7, 0, 0x101, 0x20, 4);
            let n = x.u8(Hellhound::TIMER).wrapping_sub(1);
            x.set8(Hellhound::TIMER, n);
            if (n as i8) < 0 {
                set_state_long(x, STANCE);
            } else {
                x.anim_step();
                x.move_by_table_xy();
            }
        }
        _ => {}
    }
}

/// 0x61d08: start of the spit: the dog backs off while breathing.
fn spit_begin<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Enemy::VULNERABLE, 1);
    x.set8(Hellhound::LOOK, 3);
    x.set8(Enemy::STRIKE_KIND, 0x2c);
    arrival::set_angle_facing(x, 0x18, 8);
    mv(x, 3);
    set_speed(x, 0x25);
}

// --- +6 = 10 : saut ---------------------------------------------------------------------------------------------

fn hop<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            x.set8(Enemy::VULNERABLE, 1);
            look(x, 6);
            x.set8(Enemy::STRIKE_KIND, 0x2b);
            x.set16(Obj::VX, -0x380i16 as u16);
            if x.u8(f::FLIP) != 0 {
                negate(x, Obj::VX);
            }
            x.set16(Obj::AX, 0);
            x.set16(Obj::VZ, 0x4c0);
            x.set16(Obj::AZ, -0x38i16 as u16);
            mv(x, 5);
        }
        2 => wait_key(x),
        4 => land(x),
        6 => {
            if x.anim_step() {
                set_state_long(x, STANCE);
            }
        }
        _ => {}
    }
}
