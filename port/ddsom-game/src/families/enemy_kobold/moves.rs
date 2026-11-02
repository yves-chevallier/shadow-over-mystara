//! Type 0, substates 6 (lying-in-wait moves), 8 (fallback moves) and 10 (stroll without a player).

use crate::families::cx::Bench;
use super::attacks::{begin_wait, hold, paw_begin};
use super::*;
use crate::objects::Current;
use crate::objects::Obj;

/// End of a substate 6 move: back to lying in wait (0x5634a).
fn ready_after_anim<R: Bench>(x: &mut Cx<'_, R>) {
    if x.anim_step() {
        set_state_long(x, STANCE);
        x.set8(LOOK, 0);
        mv(x, 0);
        ready(x);
    }
}

/// 0x555c2: the Kobold lies in wait (random test, then range boxes or charge).
fn ready<R: Bench>(x: &mut Cx<'_, R>) {
    if !lib::maybe_draw_6e(x) {
        stance::reach(x);
    } else {
        x.set16(f::SUB2, 0x400);
    }
}

// --- substate 6 --------------------------------------------------------------------------------------------

pub(super) fn run_b<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => return begin_wait(x),
        2 => return hold(x, 0x0206_0000),
        _ => {}
    }
    match x.u8(f::SUB2) {
        0 => b_roar(x),
        2 => b_stretch(x),
        4 => b_somersault(x),
        6 => b_hop(x, 0x80, 0x2c0, false),
        8 => b_hop_strike(x),
        10 => b_hop(x, 0x280, 0x2c0, true),
        _ => {}
    }
}

/// +6 = 0: roar or breath (random draw).
fn b_roar<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        4 => {
            let r = rand_and(x, 1);
            if r != 0 {
                set_state_long(x, 0x0202_0002);
                x.set8(LOOK, 0xc);
                mv(x, 0xc);
                return ready(x);
            }
            x.add8(f::SUB3, 4);
            x.set8(VULNERABLE, 1);
            x.set8(LOOK, 7);
            let r = rand_and(x, 0x1f);
            let n = x.data8(0x55de6 + r);
            x.set8(REPEAT, n);
            mv(x, 7);
        }
        6 => ready_after_anim(x),
        8 => attacks::breath_tick(x),
        _ => {}
    }
}

fn b_stretch<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        4 => {
            x.add8(f::SUB3, 2);
            x.set8(VULNERABLE, 1);
            x.set8(LOOK, 0xf);
            mv(x, 0xf);
        }
        6 => ready_after_anim(x),
        _ => {}
    }
}

/// +6 = 4: sideways somersault.
fn b_somersault<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        4 => {
            x.add8(f::SUB3, 2);
            x.set16(Obj::VY, 0x100);
            let r = rand_and(x, 1);
            if r != 0 {
                x.set16(Obj::VY, -0x100i16 as u16);
            }
            x.set8(VULNERABLE, 1);
            look(x, 6);
            x.set16(Obj::AY, 0);
            x.set16(Obj::VZ, 0x400);
            x.set16(Obj::AZ, -0x38i16 as u16);
            mv(x, 6);
        }
        6 => attacks_wait_key(x),
        8 => land_yz(x),
        10 => ready_after_anim(x),
        _ => {}
    }
}

/// 0x55148: waits for the animation key.
fn attacks_wait_key<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::KEY_FRAME) != 0 {
        x.add8(f::SUB3, 2);
        x.set8(VULNERABLE, 0);
    }
    x.anim_step();
}

/// 0x5515e: landing of the somersault.
fn land_yz<R: Bench>(x: &mut Cx<'_, R>) {
    if x.move_yz().gt() {
        x.anim_step();
        return;
    }
    x.set8(Enemy::ACTIVE, 1);
    x.set32(Obj::Z, 0);
    x.add8(f::SUB3, 2);
    x.set8(VULNERABLE, 1);
    look(x, 0x18);
    mv(x, 0x18);
}

/// +6 = 6 and 10: sideways leap; `strike`: attack animation 7 instead of walking animation 0x1c.
fn b_hop<R: Bench>(x: &mut Cx<'_, R>, vx: i16, vz: u16, strike_anim: bool) {
    match x.u8(f::SUB3) {
        4 => {
            x.add8(f::SUB3, 2);
            x.set8(VULNERABLE, 1);
            look(x, 0x1c);
            x.set16(Obj::VX, vx as u16);
            if x.u8(f::FLIP) != 0 {
                negate(x, Obj::VX);
            }
            x.set16(Obj::AX, 0);
            x.set16(Obj::VZ, vz);
            x.set16(Obj::AZ, -0x38i16 as u16);
            if strike_anim {
                strike(x, 7);
            } else {
                mv(x, 0x1c);
            }
        }
        6 => attacks_wait_key_clear(x),
        8 => land(x),
        10 => ready_after_anim(x),
        _ => {}
    }
}

/// 0x54fae: waits for the animation key (the key is not reset to zero).
fn attacks_wait_key_clear<R: Bench>(x: &mut Cx<'_, R>) {
    wait_key(x);
}

/// 0x54fc4: landing of a jump.
fn land<R: Bench>(x: &mut Cx<'_, R>) {
    if x.move_xz().gt() {
        x.anim_step();
        return;
    }
    x.set32(Obj::Z, 0);
    x.add8(f::SUB3, 2);
    x.set8(VULNERABLE, 1);
    look(x, 0x18);
    mv(x, 0x18);
}

/// +6 = 8: sideways leap then strike.
fn b_hop_strike<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        4 => b_hop(x, 0x80, 0x2c0, false),
        6 => attacks_wait_key_clear(x),
        8 => land(x),
        10 => {
            if x.anim_step() {
                x.add8(f::SUB3, 2);
                x.set8(STRIKE_POWER, 5);
                x.set8(LOOK, 0x28);
                strike(x, 0);
            }
        }
        12 => ready_after_anim(x),
        _ => {}
    }
}

// --- substate 8 ----------------------------------------------------------------------------------------------

pub(super) fn run_c<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 | 6 => c_wait(x),
        2 => c_paw(x),
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
            x.set16(TIMER, 0x10);
            c_begin(x);
        }
        2 => {
            let n = x.u16(TIMER).wrapping_sub(1);
            x.set16(TIMER, n);
            if (n as i16) < 0 {
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

/// +6 = 2 : coup de patte.
fn c_paw<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => paw_begin(x),
        2 => {
            let n = x.u16(TIMER).wrapping_sub(1);
            x.set16(TIMER, n);
            if (n as i16) >= 0 {
                x.move_by_table_xy();
                x.anim_step();
            } else {
                set_state_long(x, STANCE);
            }
        }
        4 => wait_key(x),
        6 => land(x),
        8 => stand_when_done(x),
        _ => {}
    }
}

/// +6 = 4: side step or side jump.
fn c_side<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            x.set8(VULNERABLE, 1);
            let r = rand_and(x, 1);
            let target = x.u16(f::TARGET) != 0;
            let mut above = false;
            if target {
                let t = x.linked(f::TARGET);
                above = (t.u16(x.b, Obj::Y).wrapping_sub(x.u16(Obj::Y)) as i16) >= 0;
            }
            if r == 0 {
                x.set8(f::SUB3, 4);
                x.set16(Obj::VY, -0x80i16 as u16);
                if target && !above {
                    negate(x, Obj::VY);
                }
                x.set16(Obj::AY, 0);
                x.set16(Obj::VZ, 0x3c0);
                x.set16(Obj::AZ, -0x38i16 as u16);
                look(x, 6);
                return mv(x, 6);
            }
            x.add8(f::SUB3, 2);
            x.set8(Obj::SPEED_INDEX, if above { 0x10 } else { 0 });
            look(x, 0xc);
            x.set16(TIMER, 0x24);
            mv(x, 0xc);
            set_speed(x, 0xb);
        }
        2 => {
            let n = x.u16(TIMER).wrapping_sub(1);
            x.set16(TIMER, n);
            if (n as i16) < 0 {
                return set_state_long(x, STANCE);
            }
            x.move_xy();
            x.anim_step();
        }
        4 => attacks_wait_key(x),
        6 => land_yz(x),
        8 => stand_when_done(x),
        _ => {}
    }
}

// --- substate 10: stroll -----------------------------------------------------------------------------------

pub(super) fn idle<R: Bench>(x: &mut Cx<'_, R>) {
    if var::PLAYERS_ALIVE.get(x.b) != 0 {
        set_state_long(x, STANCE);
        x.set8(LOOK, 0x7f);
        pick_target(x);
        return;
    }
    if x.u8(f::SUB2) == 0 {
        x.add8(f::SUB2, 2);
        let r = rand_and(x, 3);
        mv(x, 8 + r);
        return;
    }
    x.anim_step();
}
