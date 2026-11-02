//! Type 6, substates 4 to 12.
//!
//! * 4: attacks (`+6` = 0 run and bite, 2 and 4 bite in place, 6 paw strike, 8 leap and bite,
//!   10 to 14 short paw strike, 16 fire breath);
//! * 6: lying-in-wait moves (`+6` = 0 to 12);
//! * 8: fallback moves (`+6` = 0, 2, 4, 6);
//! * 10: stroll as long as no player is alive;
//! * 12: bite of a downed player.

use crate::families::cx::Bench;
use super::arrival::stand_when_done;
use super::*;
use crate::objects::Current;
use crate::objects::Obj;

/// ROM table: duration and shot key of the breath flames.
const FLAME_SCRIPT: u32 = 0x62284;

/// Effect of a bite or an attack (0x9eba6 then 0xebd30); the animation key is reset to zero.
fn bite_effect<R: Bench>(x: &mut Cx<'_, R>, kind: u32) {
    x.set8(f::KEY_FRAME, 0);
    crate::families::eshl_creators::shot_6(&mut crate::families::effects_creators::leftovers::Pure, x.b, x.o, kind as u8);
    lib::sound(x, 0xebd30);
}

/// 0x61c88: prepares the wait (look 0, animation 0).
fn begin_wait<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB3, 2);
    x.set8(VULNERABLE, 1);
    look(x, 0);
    mv(x, 0);
}

/// 0x61fa0 / 0x61f94: pursuit in place until counter `+0x77` ends, with a turn-around if the target changes side;
/// `resume` is the state to resume after the turn-around.
fn hold<R: Bench>(x: &mut Cx<'_, R>, resume: u32) {
    x.set32(RESUME_STATE, resume);
    let n = x.u8(AI_LUNGE).wrapping_sub(1);
    x.set8(AI_LUNGE, n);
    if (n as i8) < 0 {
        return x.add8(f::SUB3, 2);
    }
    if must_turn(x) {
        let sub2 = x.u8(f::SUB2);
        x.set8(RESUME_SUB2, sub2);
        set_state_long(x, 0x0202_0200);
    }
    x.anim_step();
}

/// 0x61eb0: end of a hit animation → next (0x63790).
fn step_then_after<R: Bench>(x: &mut Cx<'_, R>) {
    if x.anim_step() {
        after_strike(x);
    }
}

/// End of an attack animation: walking animation, then continuation (0x63790).
fn done_then_after<R: Bench>(x: &mut Cx<'_, R>) {
    if x.anim_step() {
        mv(x, 0);
        after_strike(x);
    }
}

/// Waits for the animation key then moves on to the next step, without resetting the key to zero (0x61ec0 resets
/// `+0x33` to zero, not 0x6244a).
fn wait_key_clear<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::KEY_FRAME) != 0 {
        x.set8(f::KEY_FRAME, 0);
        x.add8(f::SUB3, 2);
        x.set8(VULNERABLE, 0);
    }
    x.anim_step();
}

/// 0x61eda: mid-leap, the key triggers the bite; on the ground, dust and landing animation.
fn bite_land<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::KEY_FRAME) != 0 {
        let Some(player) = lib::attack_hits_a_player(x, 4) else { return step_then_after(x) };
        x.set32(Obj::Z, 0);
        return lib::seize_player(x, player, 2);
    }
    if x.move_xz().gt() {
        x.anim_step();
        return;
    }
    x.set32(Obj::Z, 0);
    x.add8(f::SUB3, 2);
    x.set16(0x30, 1);
    x.set8(VULNERABLE, 1);
    mv(x, 0xc);
    let vy = if x.u8(f::FLIP) != 0 { -0x1c } else { 0x1c };
    x.set16(Obj::AX, vy as u16);
    x.anim_step();
}

/// 0x61f3c: the dog slides, blowing up dust, until it stops.
fn slide<R: Bench>(x: &mut Cx<'_, R>) {
    puff(x, 3, 1, 1, -0x18, -1);
    if x.u8(Obj::VX) == 0 {
        x.add8(f::SUB3, 2);
        x.set32(Obj::VX, 0);
    } else {
        x.move_x();
    }
}

/// Start of an attack leap: horizontal speed `vx` (inverted if the enemy looks left).
fn begin_jump<R: Bench>(x: &mut Cx<'_, R>, vx: i16, speed_z: u16, anim: u32, set: u32) {
    x.set16(Obj::VX, vx as u16);
    if x.u8(f::FLIP) != 0 {
        negate(x, Obj::VX);
    }
    x.set16(Obj::AX, 0);
    x.set16(Obj::VZ, speed_z);
    x.set16(Obj::AZ, -0x38i16 as u16);
    x.anim_start(set, anim as u16);
}

// --- substate 4 ----------------------------------------------------------------------------------------

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => lunge(x),
        2 | 4 => bite(x),
        6 => paw(x),
        8 => jump_bite(x),
        10 | 12 | 14 => short_paw(x),
        16 => breath(x),
        _ => {}
    }
}

/// `+6` = 0: run, possible jump then bite.
fn lunge<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => begin_wait(x),
        2 => hold(x, 0x0204_0000),
        4 => {
            let r = rand_and(x, 1);
            if r == 0 {
                x.add8(f::SUB3, 2);
                strike(x, 0);
            } else {
                x.add8(f::SUB3, 4);
                x.set8(VULNERABLE, 1);
                x.set8(LOOK, 8);
                begin_jump(x, -0x280, 0x4c0, 7, SET_STRIKE);
            }
        }
        6 => {
            if x.u8(f::KEY_FRAME) != 0 {
                lib::sound(x, 0xe361a);
                if let Some(player) = lib::attack_hits_a_player(x, 3) {
                    return lib::seize_player(x, player, 0);
                }
            }
            step_then_after(x);
        }
        8 => wait_key_clear(x),
        10 => bite_land(x),
        12 => slide(x),
        14 => step_then_after(x),
        _ => {}
    }
}

/// `+6` = 2 and 4: bite in place.
fn bite<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => begin_wait(x),
        2 => hold(x, 0x0204_0000),
        4 => {
            x.add8(f::SUB3, 2);
            strike(x, 3);
        }
        6 => {
            if x.u8(f::KEY_FRAME) != 0 {
                bite_effect(x, 0);
            }
            done_then_after(x);
        }
        _ => {}
    }
}

/// `+6` = 6: paw strike, with a run-up or while running (two variants drawn at random).
fn paw<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            x.set8(VULNERABLE, 1);
            let r = rand_and(x, 1);
            if r == 0 {
                x.add8(f::SUB3, 4);
                look(x, 0xff);
                x.set8(STRIKE_KIND, 0x2b);
                return begin_jump(x, 0x180, 0x3c0, 5, SET_MOVE);
            }
            x.add8(f::SUB3, 2);
            let mut angle = 0x18;
            if x.u16(f::TARGET) != 0 {
                let target = x.linked(f::TARGET);
                let d = target.u16(x.b, Obj::X).wrapping_sub(x.u16(Obj::X)) as i16;
                if d >= 0 {
                    angle = 8;
                }
            }
            x.set8(Obj::SPEED_INDEX, angle);
            look(x, 0x1d);
            x.set8(TIMER, 0x40);
            mv(x, 0xa);
            set_speed(x, 0xd);
        }
        2 => {
            let n = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, n);
            if (n as i8) >= 0 {
                x.move_by_table_xy();
                x.anim_step();
            } else {
                back_on_guard(x);
            }
        }
        4 => wait_key(x),
        6 => land(x),
        8 => {
            if x.anim_step() {
                back_on_guard(x);
            }
        }
        _ => {}
    }
}

/// Once the move is over, the dog resumes the approach (state 2, `+6` = 0, step 2) and re-checks the ranges (0x61aa4).
fn back_on_guard<R: Bench>(x: &mut Cx<'_, R>) {
    set_state_long(x, 0x0202_0002);
    mv(x, 2);
    stance::reach_boxes(x);
}

/// `+6` = 8: leap forward then bite.
fn jump_bite<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => begin_wait(x),
        2 => hold(x, 0x0204_0000),
        4 => {
            x.add8(f::SUB3, 2);
            x.set8(VULNERABLE, 1);
            x.set8(LOOK, 8);
            begin_jump(x, -0x380, 0x4c0, 7, SET_STRIKE);
        }
        6 => wait_key_clear(x),
        8 => bite_land(x),
        10 => slide(x),
        12 => step_then_after(x),
        _ => {}
    }
}

/// `+6` = 10, 12, 14 : coup de patte court.
fn short_paw<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => begin_wait(x),
        2 => hold(x, 0x0204_0000),
        4 => {
            x.add8(f::SUB3, 2);
            strike(x, 9);
        }
        6 => {
            if x.u8(f::KEY_FRAME) != 0 {
                bite_effect(x, 1);
            }
            done_then_after(x);
        }
        _ => {}
    }
}

/// `+6` = 16: fire breath (animation 8 then a series of shots according to the table).
fn breath<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => begin_wait(x),
        2 => hold(x, 0x0204_0000),
        4 => {
            x.add8(f::SUB3, 2);
            strike(x, 8);
        }
        6 => flame_start(x),
        8 => {
            flame_tick(x, true);
            done_then_after(x);
        }
        _ => {}
    }
}

/// 0x6220e: the animation key starts the series of flames.
fn flame_start<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::KEY_FRAME) != 0 {
        x.set8(TIMER, 1);
        x.set8(SCRIPT, 0);
        x.add8(f::SUB3, 2);
    } else {
        x.anim_step();
    }
}

/// Countdown of the series of flames; at zero, draws the next duration from the table; `both`: two effects per shot
/// (substate 4) instead of just one (substate 6). Returns false when the series is over (0x62266 in the original).
fn flame_tick<R: Bench>(x: &mut Cx<'_, R>, both: bool) -> bool {
    let n = x.u8(TIMER).wrapping_sub(1);
    x.set8(TIMER, n);
    if n != 0 {
        return true;
    }
    let idx = 2 * x.u8(SCRIPT) as u32;
    let t = x.data8(FLAME_SCRIPT + idx);
    x.set8(TIMER, t);
    let v = x.data8(FLAME_SCRIPT + idx + 1);
    x.set8(0x43, v);
    x.add8(SCRIPT, 1);
    if (x.u8(SCRIPT) as i8) > 0xc {
        return false;
    }
    if both {
        lib::sound(x, 0xed398);
    }
    crate::families::eshl_creators::sweeping_8(&mut crate::families::effects_creators::leftovers::Pure, x.b, x.o);
    true
}

// --- substate 6 -------------------------------------------------------------------------------------------

pub(super) fn run_b<R: Bench>(x: &mut Cx<'_, R>) {
    // common steps: 0 wait, 2 pursuit in place (back to state 6)
    match x.u8(f::SUB3) {
        0 => return begin_wait(x),
        2 => return hold(x, 0x0206_0000),
        _ => {}
    }
    match x.u8(f::SUB2) {
        0 => b_charge(x),
        2 => b_stretch(x),
        4 => b_hop(x),
        6 => b_lunge(x),
        8 => b_slash(x),
        10 => b_claw(x),
        12 => b_flames(x),
        _ => {}
    }
}

/// 0x61a8c: the dog lies in wait (random test, then range boxes or charge).
fn ready<R: Bench>(x: &mut Cx<'_, R>) {
    if !lib::maybe_draw_6e(x) {
        stance::reach(x);
    } else {
        x.set16(f::SUB2, 0x400);
    }
}

/// 0x62332: standing, look 0, then lying in wait.
fn stand_and_ready<R: Bench>(x: &mut Cx<'_, R>) {
    set_state_long(x, STANCE);
    x.set8(LOOK, 0);
    mv(x, 0);
    ready(x);
}

/// 0x6231e: once the animation is over, back to standing and lying in wait.
fn b_end<R: Bench>(x: &mut Cx<'_, R>) {
    if x.anim_step() {
        stand_and_ready(x);
    }
}

fn b_charge<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        4 => {
            let r = rand_and(x, 1);
            if r != 0 {
                set_state_long(x, 0x0202_0002);
                x.set8(LOOK, 2);
                mv(x, 2);
                return ready(x);
            }
            x.add8(f::SUB3, 2);
            x.set8(TIMER, 0x2a);
            x.set8(VULNERABLE, 1);
            x.set8(LOOK, 0xa);
            mv(x, 0xa);
        }
        6 => {
            let n = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, n);
            if n == 0 {
                stand_and_ready(x);
            }
        }
        _ => {}
    }
}

fn b_stretch<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        4 => {
            x.add8(f::SUB3, 2);
            x.set8(VULNERABLE, 1);
            x.set8(LOOK, 0xb);
            mv(x, 0xb);
        }
        6 => b_end(x),
        _ => {}
    }
}

/// Side jump (0x6239e): speed in depth drawn at random.
fn b_hop<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        4 => {
            x.add8(f::SUB3, 2);
            x.set16(Obj::VY, 0x100);
            let r = rand_and(x, 1);
            if r != 0 {
                x.set16(Obj::VY, -0x100i16 as u16);
            }
            x.set8(VULNERABLE, 1);
            look(x, 5);
            x.set16(Obj::AY, 0);
            x.set16(Obj::VZ, 0x400);
            x.set16(Obj::AZ, -0x38i16 as u16);
            mv(x, 5);
        }
        6 => wait_key(x),
        8 => {
            if x.move_yz().gt() {
                x.anim_step();
            } else {
                land_dust(x);
            }
        }
        10 => b_end(x),
        _ => {}
    }
}

/// `+6` = 6: leap forward, landing, back to lying in wait.
fn b_lunge<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        4 => jump_forward(x),
        6 => wait_key(x),
        8 => land(x),
        10 => b_end(x),
        _ => {}
    }
}

/// `+6` = 8: leap, landing then tail strike.
fn b_slash<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        4 => jump_forward(x),
        6 => wait_key(x),
        8 => land(x),
        10 => {
            if x.anim_step() {
                x.add8(f::SUB3, 2);
                strike(x, 0);
            }
        }
        12 => b_end(x),
        _ => {}
    }
}

/// 0x62404: run-up of the leap (animation 6).
fn jump_forward<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB3, 2);
    x.set8(VULNERABLE, 1);
    look(x, 6);
    x.set16(Obj::VX, 0x300);
    if x.u8(f::FLIP) != 0 {
        negate(x, Obj::VX);
    }
    x.set16(Obj::AX, 0);
    x.set16(Obj::VZ, 0x380);
    x.set16(Obj::AZ, -0x38i16 as u16);
    mv(x, 6);
}

fn b_claw<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        4 => jump_forward(x),
        6 => wait_key(x),
        8 => land(x),
        10 => {
            if x.anim_step() {
                x.add8(f::SUB3, 2);
                x.set8(STRIKE_POWER, 5);
                strike(x, 9);
            }
        }
        12 => {
            if x.u8(f::KEY_FRAME) != 0 {
                x.set8(f::KEY_FRAME, 0);
                crate::families::eshl_creators::shot_6(&mut crate::families::effects_creators::leftovers::Pure, x.b, x.o, 1);
            }
            if x.anim_step() {
                stand_and_ready(x);
            }
        }
        _ => {}
    }
}

fn b_flames<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        4 => jump_forward(x),
        6 => wait_key(x),
        8 => land(x),
        10 => {
            if x.anim_step() {
                x.add8(f::SUB3, 2);
                x.set8(STRIKE_POWER, 5);
                strike(x, 8);
            }
        }
        12 => flame_start(x),
        14 => {
            if !flame_tick(x, false) {
                // series over: end of move of substate 4
                return done_then_after(x);
            }
            if x.anim_step() {
                // unlike the other moves, look `+0xa1` is not reset to zero
                set_state_long(x, STANCE);
                mv(x, 0);
                ready(x);
            }
        }
        _ => {}
    }
}

// --- substate 8 ----------------------------------------------------------------------------------------------

pub(super) fn run_c<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 | 6 => c_wait(x),
        2 => c_leap(x),
        4 => c_side(x),
        _ => {}
    }
}

fn c_wait<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        4 => {
            x.add8(f::SUB3, 0xfe);
            c_begin(x);
        }
        0 => {
            x.add8(f::SUB3, 2);
            x.set8(TIMER, 0x10);
            c_begin(x);
        }
        2 => {
            let n = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, n);
            if (n as i8) < 0 {
                return set_state_long(x, STANCE);
            }
            if must_turn(x) {
                x.set32(RESUME_STATE, 0x0208_0004);
                let sub2 = x.u8(f::SUB2);
                x.set8(RESUME_SUB2, sub2);
                return set_state_long(x, 0x0202_0200);
            }
            x.anim_step();
        }
        _ => {}
    }
}

fn c_begin<R: Bench>(x: &mut Cx<'_, R>) {
    look(x, 0);
    x.set8(VULNERABLE, 1);
    mv(x, 0);
}

/// `+6` = 2: attack leap, landing, back to standing (0x616d8).
fn c_leap<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => jump_forward(x),
        2 => wait_key(x),
        4 => land(x),
        6 => stand_when_done(x),
        _ => {}
    }
}

/// `+6` = 4: side step (running) or side jump.
fn c_side<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            x.set8(VULNERABLE, 1);
            let r = rand_and(x, 1);
            let mut above = false;
            if x.u16(f::TARGET) != 0 {
                let target = x.linked(f::TARGET);
                above = (target.u16(x.b, Obj::Y).wrapping_sub(x.u16(Obj::Y)) as i16) >= 0;
            }
            if r == 0 {
                x.set8(f::SUB3, 4);
                x.set16(Obj::VY, -0x80i16 as u16);
                if x.u16(f::TARGET) != 0 && !above {
                    negate(x, Obj::VY);
                }
                x.set16(Obj::AY, 0);
                x.set16(Obj::VZ, 0x3c0);
                x.set16(Obj::AZ, -0x38i16 as u16);
                look(x, 6);
                return mv(x, 5);
            }
            x.add8(f::SUB3, 2);
            x.set8(Obj::SPEED_INDEX, if above { 0x10 } else { 0 });
            look(x, 0xc);
            x.set8(TIMER, 0x24);
            mv(x, 2);
            set_speed(x, 0x11);
        }
        2 => {
            let n = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, n);
            if (n as i8) < 0 {
                return set_state_long(x, STANCE);
            }
            x.move_xy();
            x.anim_step();
        }
        4 => wait_key(x),
        6 => {
            if x.move_yz().gt() {
                x.anim_step();
            } else {
                land_dust(x);
            }
        }
        8 => stand_when_done(x),
        _ => {}
    }
}

// --- substate 10: stroll ----------------------------------------------------------------------------

pub(super) fn idle<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            set_angle(x);
            set_speed(x, 0xd);
            x.set8(0x90, 0x50);
            mv(x, 2);
        }
        2 => {
            x.move_by_table_xy();
            x.anim_step();
            let n = x.u8(0x90).wrapping_sub(1);
            x.set8(0x90, n);
            if n == 0 {
                x.add8(f::SUB2, 2);
                mv(x, 4);
            }
        }
        4 => {
            if x.anim_step() {
                let flip = x.u8(f::FLIP) ^ 0x20;
                x.set8(f::FLIP, flip);
                x.add8(Obj::SPEED_INDEX, 0x10);
                let a = x.u8(Obj::SPEED_INDEX) & 0x1f;
                x.set8(Obj::SPEED_INDEX, a);
                x.set8(f::SUB2, 2);
                x.set8(0x90, 0x50);
                mv(x, 2);
            }
        }
        _ => {}
    }
    if var::PLAYERS_ALIVE.get(x.b) != 0 {
        set_state_long(x, STANCE);
        pick_target(x);
    }
}

// --- substate 12: bite of a downed player ------------------------------------------------------------

pub(super) fn grab<R: Bench>(x: &mut Cx<'_, R>) {
    let which = x.u8(f::SUB2);
    if (x.u16(f::TARGET) as i16) >= 0 {
        return set_state_long(x, STANCE);
    }
    let victim = x.linked(f::TARGET);
    if victim.u8(x.b, 0) == 0 || victim.u8(x.b, 0x188) == 0 {
        return set_state_long(x, STANCE);
    }
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            let y = victim.u16(x.b, Obj::Y);
            x.set16(Obj::Y, y);
            if victim.u16(x.b, Obj::Z) != 0 {
                return lib::release_player(x, victim);
            }
            x.set16(Obj::Z, 0);
            x.set8(BITES, 8);
            strike(x, if which == 0 { 1 } else { 2 });
            lib::hold_seized(x);
        }
        2 => bite_victim(x, which),
        _ => {}
    }
}

/// 0x6293e: each bite (key `+0x33`) draws life from the player; at the last one it is released.
fn bite_victim<R: Bench>(x: &mut Cx<'_, R>, which: u8) {
    x.anim_step();
    if x.u8(f::KEY_FRAME) == 0 {
        return lib::hold_seized(x);
    }
    x.set8(f::KEY_FRAME, 0);
    let n = x.u8(BITES).wrapping_sub(1);
    x.set8(BITES, n);
    if (n as i8) <= 0 {
        return release(x);
    }
    x.set8(f::FREEZE, 8);
    if (x.u16(f::TARGET) as i16) >= 0 {
        return set_state_long(x, STANCE);
    }
    let reach = if x.u8(f::FLIP) != 0 { 0x38i16 } else { -0x38 };
    let d3 = if which == 0 { 0x1c } else { 0x2c };
    let pos = (x.u16(Obj::X).wrapping_add(reach as u16), x.u16(Obj::Z).wrapping_add(d3), x.u16(Obj::Y).wrapping_sub(1));
    (*x.c.d2(), *x.c.d3(), *x.c.d4()) = (pos.0 as u32, pos.1 as u32, pos.2 as u32);
    // the other object of the spark is A1, the victim (set by `grab`)
    let victim = x.linked(f::TARGET);
    crate::families::effects_creators::spark_at(x.b, x.o, victim.0, 2, pos);
    if (x.u16(f::TARGET) as i16) >= 0 {
        return set_state_long(x, STANCE);
    }
    if (victim.u16(x.b, f::HP) as i16) <= 1 {
        return release(x);
    }
    lib::sound(x, 0xedd96);
    let hp = victim.u16(x.b, f::HP).wrapping_sub(3);
    victim.set_u16(x.b, f::HP, hp);
    if (hp as i16) <= 0 {
        victim.set_u16(x.b, f::HP, 1);
        return release(x);
    }
    lib::hold_seized(x);
}

fn release<R: Bench>(x: &mut Cx<'_, R>) {
    let victim = x.linked(f::TARGET);
    victim.set_u8(x.b, 0x6f, 1);
    lib::release_player(x, victim);
}
