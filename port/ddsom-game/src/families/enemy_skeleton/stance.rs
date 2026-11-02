//! Type 1, standing skeleton (`+5 = 2`). `+6`: 0 and 6 start of the idea (target, gaze), 2 walk, 4 turn-around. A
//! lasting idea (`+7 != 0`) walks toward the target and decides to attack.

use crate::families::cx::Bench;
use super::*;
use crate::objects::Obj;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if special_hit(x) {
        return;
    }
    if grabbed(x) || x.u8(CAUGHT) != 0 {
        return set_state_long(x, KNOCKED);
    }
    match x.u8(f::SUB2) {
        0 => {
            if x.u8(f::SUB3) != 0 {
                walk(x);
            } else {
                pick_target(x);
                begin_idle(x);
            }
        }
        2 => wait(x),
        4 => turn(x),
        6 => {
            if x.u8(f::SUB3) != 0 {
                walk(x);
            } else {
                x.add8(f::SUB3, 2);
                x.set8(VULNERABLE, 1);
                x.set8(AI_SHOT, 0xc);
                body(x, 0);
                x.set32(MOVE_TABLE, 0x1c74b6);
                pick_target(x);
                lib::draw_stance(x);
                lib::choose_near_walk_point(x);
                lib::draw_6b(x);
            }
        }
        _ => {}
    }
    if var::PLAYERS_ALIVE.get(x.b) == 0 {
        set_state_long(x, IDLE);
    }
    
    if zone_and_terrain(x) {
        lib::choose_walk_point(x);
    }
}

/// 0x580ae: the idea begins, the skeleton turns toward its target.
fn begin_idle<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB3, 2);
    x.set8(VULNERABLE, 1);
    x.set8(AI_SHOT, 0xb);
    body(x, 0);
    x.set32(MOVE_TABLE, 0x1c74b6);
    lib::draw_stance(x);
    lib::draw_6b(x);
}

/// `+6 = 2`: waits for the end of the animation while walking, then draws the next idea.
fn wait<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        if x.u8(f::KEY_FRAME) != 0xff {
            move_xy(x);
            step(x);
            return;
        }
        x.add8(f::SUB3, 2);
    }
    let t = x.u8(AI_WAIT).wrapping_sub(1);
    x.set8(AI_WAIT, t);
    if t != 0 {
        return;
    }
    set_state_long(x, STANCE);
    let r = (x.random() as u32) & 0xf;
    if 0x1111u32 >> r & 1 != 0 {
        pick_target(x);
    }
    begin_idle(x);
}

/// `+6 = 4`: turn-around (animation 1 then orientation inversion).
fn turn<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        if x.u16(ANIM_STATE) != 0xff {
            move_xy(x);
            step(x);
            return;
        }
        x.add8(f::SUB3, 2);
        return body(x, 1);
    }
    if (x.u8(ANIM_STATE) as i8) < 0 {
        let flip = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
        set_state_long(x, AFTER_TURN);
        body(x, 0);
    } else {
        step(x);
    }
}

/// 0x58126: walks toward the target; decides to turn around, wait or attack.
fn walk<R: Bench>(x: &mut Cx<'_, R>) {
    lib::walk_point_reached(x);
    let (y, z) = (x.u16(WALK_ARGS_Y), x.u16(WALK_ARGS_Z));
    lib::turn_toward(x, y, z);
    let n = x.u8(AI_WALK).wrapping_sub(1);
    x.set8(AI_WALK, n);
    if (n as i8) <= 0 {
        lib::draw_stance(x);
        if x.u16(f::TARGET) == 0 {
            return;
        }
        let target = load_target(x);
        let mine = x.u16(Obj::X);
        let want = if mine as i16 > target.u16(x.b, Obj::X) as i16 { 0 } else { 0x20 };
        if want != x.u8(f::FLIP) as u32 {
            return x.set16(f::SUB2, 0x400);
        }
    }
    let n = x.u8(AI_CHOICE).wrapping_sub(1);
    x.set8(AI_CHOICE, n);
    if (n as i8) <= 0 {
        lib::draw_6b(x);
        if lib::maybe_draw_6e(x) {
            return x.set16(f::SUB2, 0x200);
        }
    }
    attack_choice(x);
}

/// 0x58194: a player in range? If so, attacks; otherwise advances by one animation step.
fn attack_choice<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(SERIES) != 0 && lib::target_in_box(x, -0x20, 0x20, 0xc) {
        set_state_long(x, DASH);
        return x.set16(COUNTER, 0);
    }
    let side = lib::target_in_front(x, -0x20, 0x20, 0x10);
    if side.found() {
        x.set8(SIDE, side as u8);
        x.set16(f::STATE, 0x204);
        lib::pick_byte(x);
        return pick_move(x, CLOSE_ATTACKS, 0x204);
    }
    if front_attack(x) || far_attack(x) || lib::guard_in_front_strict(x, -8, 0x68, 8) {
        return lib::pick_byte(x);
    }
    step(x);
    if (x.u8(f::KEY_FRAME) as i8) >= 0 {
        move_xy(x);
    }
}

/// 0x59254: target in front; the attack is launched if the mask decision wants it.
fn front_attack<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let side = lib::target_in_front(x, -8, 0x68, 8);
    if !side.found() {
        return false;
    }
    x.set8(SIDE, side as u8);
    if !decision(x) {
        return false;
    }
    pick_move(x, CLOSE_ATTACKS, 0x204);
    true
}

/// 0x592a4: target further away (wide box); same decision, long-range attack.
fn far_attack<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let side = lib::target_in_front(x, -0xa0, 0x30, 8);
    if !side.found() {
        return false;
    }
    x.set8(SIDE, side as u8);
    if !decision(x) {
        return false;
    }
    pick_move(x, FAR_ATTACKS, 0x204);
    true
}

/// Decision by mask (0xa4c2 if target and skeleton look the same way, 0xa4b4 otherwise).
fn decision<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if x.u16(f::TARGET) == 0 {
        return false;
    }
    let target = load_target(x);
    let same = target.u8(x.b, f::FLIP) == x.u8(f::FLIP);
    lib::decision(x, same)
}
