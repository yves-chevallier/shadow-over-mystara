//! Type 2, attacks. `+5 = 4`: `+6` = 0, 2, 4 combinations of attacks, 6 double attack, 8, 10 and 18 leap attack, 12
//! repeated thrust, 14 throw, 16 run or leap, 20 special attack, 22 announcement, 24 turn-around, 26 long move.
//! `+5 = 6`: guards and leaps. `+5 = 8`: fallback attacks. `+7` is the step.

use crate::families::cx::Bench;
use super::*;
use crate::objects::Current;
use crate::objects::Obj;

/// Unknown mental push from the ROM: read of the attack table of a leap, by difficulty.
fn pick_jump_move<R: Bench>(x: &mut Cx<'_, R>) {
    let row = difficulty_row(x);
    *x.c.d2() = *x.c.d2() & 0xffff_0000 | row;
    let mv = pick_byte(x, JUMP_MOVES);
    x.set8(f::SUB2, mv);
    x.set8(f::SUB3, 0);
}

// --- substate 4 --------------------------------------------------------------------------------

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(BLINK_PHASE) != 0 && (x.u8(BLINK_PHASE) as i8) > 0 {
        let n = x.u16(BLINK_COUNT).wrapping_sub(1);
        x.set16(BLINK_COUNT, n);
        if (n as i16) < 0 {
            x.set8(BLINK_ON, 0);
            x.set8(BLINK_WAIT, 0);
            x.set8(BLINK_PHASE, 0xff);
            x.set_bit(1, 1, false);
        }
    }
    match x.u8(f::SUB2) {
        id @ (0 | 2 | 4) => combo(x, id),
        6 => double(x),
        id @ (8 | 10 | 18) => leap_strike(x, id),
        12 => stab(x),
        14 => throw(x),
        16 => dash(x),
        20 => special(x),
        22 => approach(x),
        24 => turn_around(x),
        26 => long_gesture(x),
        _ => {}
    }
}

/// 0, 2 or 4: a series of attacks with animation 15fabe (number of repetitions `+0xa2`).
fn combo<R: Bench>(x: &mut Cx<'_, R>, id: u8) {
    if x.u8(f::SUB3) != 0 {
        if step(x) {
            let n = x.u8(REPEAT).wrapping_sub(1);
            x.set8(REPEAT, n);
            if (n as i8) < 0 {
                return end_attack(x);
            }
            strike(x, 0);
        }
        return;
    }
    x.add8(f::SUB3, 2);
    let look = x.u8(LOOK);
    x.set8(LOOK_PREV, look);
    x.set8(REPEAT, 0);
    let mut d3 = 0;
    x.set8(LOOK, 0xff);
    let mut anim = id as u32;
    match id {
        2 => {}
        0 => {
            x.set8(LOOK, 0xfe);
        }
        _ => {
            x.set8(AI_KIND, 3);
            x.set8(LOOK, 0xfd);
            d3 = 2;
            let r = rand_and(x, 1);
            d3 += r;
            anim = r;
        }
    }
    x.set8(STRIKE_KIND, 0x15);
    x.set8(REPEAT, d3 as u8);
    x.set8(STRIKE_POWER, 5);
    strike(x, anim);
}

/// 6 : double coup.
fn double<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(AI_KIND, 3);
        x.set8(REPEAT, 1);
        let r = rand_and(x, 1);
        x.add8(REPEAT, r as u8);
        x.set8(STRIKE_KIND, 0x15);
        x.set8(STRIKE_POWER, 5);
        return strike(x, 0);
    }
    if step(x) {
        let n = x.u8(REPEAT).wrapping_sub(1);
        x.set8(REPEAT, n);
        if (n as i8) < 0 {
            return end_attack(x);
        }
        x.set8(STRIKE_KIND, 0x16);
        x.set8(STRIKE_POWER, 5);
        strike(x, 1);
    }
}

/// 8, 10 and 18: leap onto the target with an attack (8 and 18 chain the next attack, 10 replays a leap).
fn leap_strike<R: Bench>(x: &mut Cx<'_, R>, id: u8) {
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            x.set16(Obj::VX, 0);
            let target = load_target(x);
            if target.u8(x.b, 0) == 0 {
                set_state_long(x, IDLE);
                return rest_begin(x);
            }
            let look = x.u8(LOOK);
            x.set8(LOOK_PREV, look);
            x.set8(LOOK, 0xf0);
            x.set16(Obj::VZ, 0x680);
            x.set16(Obj::AZ, -0x38i16 as u16);
            x.set16(Obj::AX, 0);
            let dx = target.u16(x.b, Obj::X).wrapping_sub(x.u16(Obj::X));
            x.set16(Obj::VX, dx << 2);
            x.set8(STRIKE_KIND, 0x17);
            let mut away = x.u16(Obj::X).wrapping_sub(target.u16(x.b, Obj::X));
            if (away as i16) < 0 {
                away = away.wrapping_neg();
            }
            if away > 0x70 {
                x.set8(STRIKE_KIND, 0x18);
            }
            x.set8(STRIKE_POWER, 5);
            strike(x, 2);
        }
        2 => land_with(x),
        4 => {
            if !step(x) {
                return;
            }
            if id != 10 {
                return end_attack(x);
            }
            let r = rand_and(x, 2);
            let look = x.u8(LOOK);
            x.set8(LOOK_PREV, look);
            x.set8(f::SUB2, r as u8);
            x.set8(f::SUB3, 0);
        }
        _ => {}
    }
}

/// Climb and fall of a leap, then landing animation (0xc).
fn land_with<R: Bench>(x: &mut Cx<'_, R>) {
    x.move_xz();
    if step(x) && (x.u16(Obj::Z) as i16) < 0 {
        x.add8(f::SUB3, 2);
        x.set16(Obj::Z, 0);
        body(x, 0xc);
    }
}

/// 12: thrust repeated `+0xa2` times (animation 5).
fn stab<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(AI_KIND, 2);
        let row = difficulty_row(x);
        let n = rand_and(x, 0x1f);
        let v = x.data8(REPEATS_TABLE + row + n);
        x.set8(REPEAT, v);
        return stab_begin(x);
    }
    if step(x) {
        let n = x.u8(REPEAT).wrapping_sub(1);
        x.set8(REPEAT, n);
        if (n as i8) >= 0 {
            return stab_begin(x);
        }
        end_attack(x);
    }
}

fn stab_begin<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(STRIKE_KIND, 0x1a);
    x.set8(STRIKE_POWER, 5);
    strike(x, 5);
}

/// 14: throw (animation 8 repeated), with routine 0x9e194 which can interrupt the move.
fn throw<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        if crate::families::eshl_creators::four_3(&mut crate::families::effects_creators::leftovers::Pure, x.b, x.o) {
            x.set8(REPEAT, 6);
            return body(x, 8);
        }
        set_state_long(x, IDLE);
        return rest_begin(x);
    }
    if step(x) {
        let n = x.u8(REPEAT).wrapping_sub(1);
        x.set8(REPEAT, n);
        if (n as i8) >= 0 {
            body(x, 8);
        } else {
            pick_jump_move(x);
        }
    }
}

/// 16: run toward the target (animation 2) or leap.
fn dash<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            if rand_and(x, 1) == 0 {
                return long_jump(x);
            }
            x.add8(f::SUB3, 4);
            x.set8(LOOK_PREV, 0);
            x.set8(REPEAT, 0x20);
            walk_speed(x, 4);
            set_angle(x, 0x18, 8);
            body(x, 2);
        }
        2 => long_jump(x),
        4 => {
            move_xy(x);
            step(x);
            let n = x.u8(REPEAT).wrapping_sub(1);
            x.set8(REPEAT, n);
            if (n as i8) < 0 {
                set_state_long(x, STANCE);
            }
        }
        6 => land_with(x),
        8 => {
            if step(x) {
                set_state_long(x, STANCE);
            }
        }
        _ => {}
    }
}

/// 0x5a5a4: big sideways leap.
fn long_jump<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB3, 6);
    x.set8(LOOK_PREV, 0xcc);
    x.set16(Obj::AZ, -0x38i16 as u16);
    x.set16(Obj::VZ, 0x250);
    x.set16(Obj::VX, 0x300);
    x.set16(Obj::AX, 0);
    x.set32(Obj::VY, 0);
    x.set8(Obj::SPEED_INDEX, 8);
    if x.u8(f::FLIP) != 0 {
        x.set8(Obj::SPEED_INDEX, 0x18);
        negate(x, Obj::VX);
    }
    body(x, 0xa);
}

/// 20: special move (animation 3), interrupted by 0x9e2a0.
fn special<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(AI_KIND, 2);
        return strike(x, 3);
    }
    if step(x) {
        return end_attack(x);
    }
    if x.u8(f::KEY_FRAME) != 0
        && crate::families::eshl_creators::above_4(&mut crate::families::effects_creators::leftovers::Pure, x.b, x.o).is_none() {
            set_state_long(x, IDLE);
        }
}

/// 22: announcement of the attack, then switch to the planned attack `+0xa3` (or turn-around if the target is behind).
fn approach<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(LOOK_PREV, 1);
        lib::pick_byte(x);
        if x.u8(COOLDOWN) == 0 {
            x.set8(f::SUB3, 0);
            let next = x.u8(RESUME);
            x.set8(f::SUB2, next);
        } else {
            x.set8(LOOK_PREV, 1);
        }
        return body(x, 0);
    }
    step(x);
    let n = x.u8(COOLDOWN).wrapping_sub(1);
    x.set8(COOLDOWN, n);
    if (n as i8) < 0 {
        x.set8(f::SUB3, 0);
        if needs_turn_around(x) {
            x.add8(f::SUB2, 2);
        } else {
            let next = x.u8(RESUME);
            x.set8(f::SUB2, next);
        }
    }
}

/// 24: turn-around, then resumes the planned attack.
fn turn_around<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(LOOK_PREV, 0xe);
        let flip = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
        return body(x, 1);
    }
    if step(x) {
        let next = x.u8(RESUME);
        x.set8(f::SUB2, next);
        x.set8(f::SUB3, 0);
    }
}

/// 26: long move (animation 8 repeated `+0xa2` times).
fn long_gesture<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(REPEAT, 6);
        x.set8(LOOK_PREV, 0xee);
        return body(x, 8);
    }
    if step(x) {
        let n = x.u8(REPEAT).wrapping_sub(1);
        x.set8(REPEAT, n);
        if (n as i8) >= 0 {
            body(x, 8);
        } else {
            end_attack(x);
        }
    }
}

// --- substate 6: guards and leaps --------------------------------------------------------------

pub(super) fn run_b<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 | 16 | 18 | 20 => hold(x),
        2 => guard(x),
        4 => side_hop(x),
        6 | 8 | 10 => forward_hop(x),
        12 => hop_strike(x),
        14 => throw(x),
        22 => approach(x),
        24 => turn_around(x),
        _ => {}
    }
}

/// Guard in place (animation 8) until the end of the animation, then back to standing.
fn hold<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(LOOK_PREV, 0xee);
        x.set8(VULNERABLE, 1);
        return body(x, 8);
    }
    if step(x) {
        back_to_stance(x);
    }
}

/// 2 : garde en posture 9 pendant `0x3c` images.
fn guard<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(POSTURE, 9);
        x.set8(LOOK, 0xdf);
        x.set8(REPEAT, 0x3c);
        return body(x, 0xd);
    }
    let n = x.u8(REPEAT).wrapping_sub(1);
    x.set8(REPEAT, n);
    if n == 0 {
        x.set8(POSTURE, 4);
        back_to_stance(x);
    }
}

/// 4: side leap toward the depth of the target.
fn side_hop<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(LOOK_PREV, 0xb1);
        x.set16(Obj::VZ, 0x600);
        x.set16(Obj::AZ, -0x38i16 as u16);
        x.set16(Obj::AY, 0);
        x.set16(Obj::VY, 0x80);
        let target = load_target(x);
        if target.u8(x.b, 0) == 0 {
            set_state_long(x, IDLE);
            return rest_begin(x);
        }
        let dy = target.u16(x.b, Obj::Y).wrapping_sub(x.u16(Obj::Y));
        if (dy as i16) < 0 {
            negate(x, Obj::VY);
        }
        return body(x, 0xa);
    }
    x.move_yz();
    if step(x) && (x.u16(Obj::Z) as i16) < 0 {
        x.set16(Obj::Z, 0);
        back_to_stance(x);
    }
}

/// 6, 8 and 10: run forward (animation 3); at the end, 6 goes back to standing, 8 and 10 chain an attack.
fn forward_hop<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        return forward_hop_begin(x);
    }
    move_xy(x);
    if !step(x) {
        return;
    }
    match x.u8(f::SUB2) {
        8 => set_state_long(x, 0x0204_0200),
        s if s > 8 => set_state_long(x, 0x0204_1400),
        _ => back_to_stance(x),
    }
}

/// 0x5a8c8.
pub(super) fn forward_hop_begin<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB3, 2);
    x.set8(LOOK_PREV, 0xb0);
    x.set8(Obj::SPEED_INDEX, 8);
    if x.u8(f::FLIP) != 0 {
        x.add8(Obj::SPEED_INDEX, 0x10);
    }
    walk_speed(x, 2);
    body(x, 3);
}

/// 12: leap with an attack (animation 6 of the attack series).
fn hop_strike<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => hop_strike_begin(x),
        2 => {
            x.move_xz();
            step(x);
            let h = x.u16(Obj::Z) as i16;
            if h < 0 {
                x.add8(f::SUB3, 2);
                x.set16(Obj::Z, 0);
                body(x, 0xc);
            }
        }
        _ => {
            if step(x) {
                back_to_stance(x);
            }
        }
    }
}

/// 0x5a934.
pub(super) fn hop_strike_begin<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB3, 2);
    x.set8(LOOK_PREV, 0xb2);
    x.set16(Obj::VZ, 0x680);
    x.set16(Obj::AZ, -0x38i16 as u16);
    x.set16(Obj::AX, 0);
    x.set16(Obj::VX, 0x200);
    if x.u8(f::FLIP) != 0 {
        negate(x, Obj::VX);
    }
    x.set8(STRIKE_KIND, 0x16);
    x.set8(STRIKE_POWER, 5);
    strike(x, 6);
}

// --- substate 8: fallback attacks -----------------------------------------------------------------

pub(super) fn run_fallback<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            if x.u8(f::SUB3) == 0 {
                x.add8(f::SUB3, 2);
                x.set8(VULNERABLE, 1);
                x.set8(REPEAT, 0x10);
                x.set8(LOOK_PREV, 1);
                x.set8(LOOK, 1);
                return body(x, 0);
            }
            step(x);
            let n = x.u8(REPEAT).wrapping_sub(1);
            x.set8(REPEAT, n);
            if (n as i8) < 0 {
                set_state_long(x, STANCE);
            }
        }
        2 => {
            x.set8(f::SUB, 6);
            if x.u8(SERIES) == 0 {
                x.set8(f::SUB2, 6);
                forward_hop_begin(x);
            } else {
                x.set8(f::SUB2, 0xc);
                hop_strike_begin(x);
            }
        }
        4 => sidestep(x),
        6 => choose(x),
        _ => {}
    }
}

/// `+6` = 4: side step, either a run (step 0) or a leap in depth.
fn sidestep<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            if rand_and(x, 1) == 0 {
                x.set8(REPEAT, 0x10);
                x.set8(Obj::SPEED_INDEX, 0);
                if x.u8(f::FLIP) == 0 {
                    x.set8(Obj::SPEED_INDEX, 0x10);
                }
                walk_speed(x, 4);
                return body(x, 2);
            }
            let target = load_target(x);
            let mut d0 = 0;
            if target.u8(x.b, 0) == 0 {
                set_state_long(x, IDLE);
            }
            let dy = target.u16(x.b, Obj::Y).wrapping_sub(x.u16(Obj::Y));
            if (dy as i16) < 0 {
                d0 += 2;
            }
            x.add8(f::SUB2, 2);
            x.set16(Obj::VZ, 0x600);
            x.set16(Obj::AZ, -0x38i16 as u16);
            x.set16(Obj::VY, 0x80);
            x.set16(Obj::AY, 0);
            if d0 != 0 {
                negate(x, Obj::VY);
            }
            body(x, 0xa);
        }
        2 => {
            move_xy(x);
            step(x);
            let n = x.u8(REPEAT).wrapping_sub(1);
            x.set8(REPEAT, n);
            if (n as i8) < 0 {
                set_state_long(x, STANCE);
            }
        }
        4 => {
            x.move_yz();
            if step(x) && (x.u16(Obj::Z) as i16) < 0 {
                x.set8(LOOK_PREV, 0xe0);
                x.set16(Obj::Z, 0);
                set_state_long(x, STANCE);
            }
        }
        6 => {
            if step(x) {
                set_state_long(x, STANCE);
            }
        }
        _ => {}
    }
}

/// `+6` = 6: chooses the next attack according to the target.
fn choose<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(LOOK_PREV, 1);
    body(x, 0);
    if lib::target_in_box(x, -0x20, 0x20, 8) {
        set_state_long(x, 0x0204_1600);
        return x.set8(RESUME, 6);
    }
    if front_box(x) {
        return super::stance::pick_attack_close(x);
    }
    if side_box(x) {
        return super::stance::pick_attack_far(x);
    }
    set_state_long(x, STANCE);
}
