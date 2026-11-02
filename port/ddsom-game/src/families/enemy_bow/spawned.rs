//! Type 3 created by another object (`+0x42` non-zero): it does not follow the AI. It springs up (`+5` = 0), then
//! shoots in place until its lifetime `+0xa8` elapses (`+0xa7`: grabbed), after which it leaves the screen on foot. No
//! zone or shadow: only the routines' re-queuing (`jmp 0x1ee1a`) is replayed.

use crate::families::cx::Bench;
use super::*;
use crate::objects::Current;
use crate::objects::Obj;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(NO_ZONE) != 0 {
        return set_state_long(x, HELD);
    }
    match x.u8(f::SUB) {
        0 => begin(x),
        2 => active(x),
        _ => {}
    }
}

/// 0x5befc: springs up: hit points set to 1 (words `+0x60`, `+0x62`, `+0x64`), walks in place.
fn begin<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB, 2);
    x.set16(Enemy::HP_MAX, 1);
    x.set16(f::HP, 1);
    x.set16(Enemy::HP_LAST, 1);
    set_speed(x, 0x18);
    x.set8(TIMER, 0x10);
    walk_angle(x);
    mv(x, 2);
}

/// 0x5bf40 : tire en place.
fn active<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(EXPIRED) != 0 || grabbed(x) {
        x.set16(f::STATE, 0x0408);
        x.set16(f::SUB2, 0);
        return hit::grab(x);
    }
    match x.u8(f::SUB2) {
        0 => walk(x),
        2 => leave(x),
        4 => shoot(x),
        _ => {}
    }
}

/// +6 = 0: walks in place for `+0xa4` frames, then the shots.
fn walk<R: Bench>(x: &mut Cx<'_, R>) {
    x.move_by_table_xy();
    x.anim_step();
    let n = x.u8(TIMER).wrapping_sub(1);
    x.set8(TIMER, n);
    if (n as i8) < 0 {
        x.add8(f::SUB2, 4);
    }
    x.reinsert();
}

/// +6 = 2: the lifetime has elapsed: the Bow Gnoll moves away from the screen, then disappears.
fn leave<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            let mut want = 0x20u8;
            x.set8(Obj::SPEED_INDEX, 8);
            let base = if var::CAMERA_SELECT.get(x.b) != 0 { 0x55e } else { 0x50e };
            let d = x.u16(Obj::X).wrapping_sub(x.a5_u16(base));
            if (d as i16) <= 0xc0 {
                x.set8(Obj::SPEED_INDEX, 0x18);
                want = 0;
            }
            let anim = if want != x.u8(f::FLIP) {
                x.add8(f::SUB3, 2);
                3
            } else {
                x.set8(EXPIRED, 1);
                set_speed(x, 0x21);
                2
            };
            mv(x, anim);
            x.reinsert();
        }
        2 => {
            x.anim_step();
            x.move_by_table_xy();
            if x.in_arena_default() {
                return;
            }
            if (x.u8(TIMED) as i8) < 0 && x.u16(f::HOST) != 0 {
                let host = x.linked(f::HOST);
                host.set_u8(x.b, Obj::SPEED_INDEX, 1);
            }
            set_state_long(x, st::DEAD_LONG);
            detach_from_host(x);
        }
        4 => {
            if x.anim_step() {
                x.set8(EXPIRED, 1);
                x.set8(f::SUB3, 0);
                let flip = x.u8(f::FLIP) ^ 0x20;
                x.set8(f::FLIP, flip);
            }
            x.reinsert();
        }
        _ => {}
    }
}

/// +6 = 4: shoots (number of shots drawn from the row), stays in the shooting loop.
fn shoot<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        let n = rank_byte(x, SHOT_REPEATS);
        x.set8(REPEAT, n);
        shoot_anim(x);
        return x.reinsert();
    }
    if x.anim_step() {
        let n = x.u8(REPEAT).wrapping_sub(1);
        x.set8(REPEAT, n);
        if n != 0 {
            shoot_anim(x);
            return x.reinsert();
        }
        x.add8(f::SUB2, 0xfe);
        x.set8(f::SUB3, 0);
    }
    if x.u8(f::KEY_FRAME) != 0 {
        x.set8(f::KEY_FRAME, 0);
        crate::families::eshl_creators::shot_0(&mut crate::families::effects_creators::leftovers::Pure, x.b, x.o);
    }
    x.reinsert();
}

/// 0x5c04a: shot animation (0 or 5 according to profile `+0x2f`).
fn shoot_anim<R: Bench>(x: &mut Cx<'_, R>) {
    let anim = if x.u8(GROUP) != 0 { 5 } else { 0 };
    shot(x, anim);
}
