//! Type 4, hit state (`+4 = 4`, 0x5ea0c). `+5`: 0 and 6 common reaction by table (`+0x49`), 2 recoil, 4 fall, 8 death
//! or flight.

use crate::families::cx::Bench;
use super::stance::next_move;
use super::*;
use crate::objects::Current;

/// Tables of the common reaction, indexed by reaction code `+0x49`.
const REACT_FIRST: u32 = 0x5ea5c;
const REACT_SECOND: u32 = 0x5ecb8;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    hit_loop_at(x, NO_ZONE, |x| match x.u8(f::SUB) {
        0 => {
            x.set8(CUR_ANIM, 0x7f);
            react(x, REACT_FIRST);
        }
        2 => {
            if !paused(x) {
                recoil(x);
            }
        }
        4 => fall(x),
        6 => react(x, REACT_SECOND),
        8 => {
            if !paused(x) {
                flee(x);
            }
        }
        _ => {}
    });
}

// --- substate 2: recoil -------------------------------------------------------------------------

fn recoil<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(WANT_ANIM, 8);
            x.set8(CUR_ANIM, 8);
            let flip = x.u8(0x4a) ^ 0x20;
            x.set8(f::FLIP, flip);
            let flipped = flip != 0;
            x.set16(0x14, if flipped { -0x500i16 } else { 0x500 } as u16);
            x.set16(0x16, if flipped { 0x28 } else { -0x28i16 } as u16);
            body(x, 8);
        }
        2 => {
            x.add8(f::SUB2, 2);
            lib::sound(x, 0xecdba);
        }
        4 => {
            let low = x.a5_u16(0x6e) & 3;
            x.set_a5_u16(0x6e, low);
            if low == 0 {
                lib::shock(x, 1, 2, 0x18, 0xffff);
            }
            if x.u8(0x14) != 0 {
                x.move_x();
                return;
            }
            if x.anim_step() {
                x.set32(0x58, 0);
                x.set32(0x14, 0);
                next_move(x);
            }
        }
        _ => {}
    }
}

// --- substate 4: fall -------------------------------------------------------------------------

fn fall<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            body(x, 0xe);
        }
        2 => {
            if paused(x) {
                return;
            }
            let t = x.u16(0x84).wrapping_sub(1);
            x.set16(0x84, t);
            if (t as i16) > 0 {
                return;
            }
            x.add8(f::SUB2, 2);
            x.set8(0, 2);
            x.set8(0x86, 0);
            x.set8(0x49, 0);
        }
        4 => {
            if paused(x) {
                return;
            }
            if x.anim_step() {
                x.set8(0, 1);
                x.set8(VULNERABLE, 1);
                set_state_long(x, ACTIVE_STANCE);
                body(x, 0);
            }
        }
        _ => {}
    }
}

// --- substate 8: death or flight -----------------------------------------------------------------

fn flee<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.set8(NO_ZONE, 1);
            let flipped = x.u8(f::FLIP) != 0;
            let run_now = if !lib::in_right_half(x) { !flipped } else { flipped };
            if run_now {
                x.add8(f::SUB2, 4);
                run_away(x);
            } else {
                x.add8(f::SUB2, 2);
                x.set8(VULNERABLE, 1);
                x.set8(WANT_ANIM, 0x7f);
                x.set8(CUR_ANIM, 0x7f);
                body(x, 1);
            }
        }
        2 => {
            if x.anim_step() {
                x.add8(f::SUB2, 2);
                let flip = x.u8(f::FLIP) ^ 0x20;
                x.set8(f::FLIP, flip);
                run_away(x);
            }
        }
        4 => {
            step(x);
            move_xy(x);
            if x.in_arena_default() {
                return;
            }
            set_state_long(x, 0x0600_0000);
            detach_from_host(x);
        }
        _ => {}
    }
}

/// 0x5ede8: runs away toward the edge of the screen.
fn run_away<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(VULNERABLE, 1);
    set_angle(x, 8, 0x18);
    body(x, 5);
    speed(x, 0x1a);
}
