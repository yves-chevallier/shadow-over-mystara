//! Type 19, hit state (`+4 = 4`, 0x7407a): reactions to hits. `+5`: 0 common reaction by table (`+0x49`), 2 recoil of
//! the parts, 4 fall, 6 second reaction table, 8 death or flight.

use crate::families::cx::Bench;
use crate::data::RomAddr;
use super::stance::next_move;
use super::*;
use crate::objects::Current;

/// Tables of the common reaction (indexed by reaction code `+0x49`).
const REACT_PART: RomAddr = RomAddr(0x740c0);
const REACT_BEAST: RomAddr = RomAddr(0x742fa);

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    let freeze = x.u8(f::FREEZE) as i8;
    if freeze > 0 {
        x.set8(f::FREEZE, (freeze - 1) as u8);
        return x.reinsert();
    }
    match x.u8(f::SUB) {
        0 => {
            x.set8(DisplacerBeast::CUR_ANIM, 0x7f);
            react(x, REACT_PART.0);
            zone_and_shadow(x);
        }
        2 => {
            if !paused(x) {
                recoil(x);
                zone_and_shadow(x);
            }
        }
        4 => {
            fall(x);
            zone_and_shadow(x);
        }
        6 => {
            react(x, REACT_BEAST.0);
            zone_and_shadow(x);
        }
        8 => {
            if !paused(x) {
                flee(x);
            }
        }
        _ => {}
    }
    x.reinsert();
}

fn zone_and_shadow<R: Bench>(x: &mut Cx<'_, R>) {
    lib::keep_in_zone(x);
    lib::collide_terrain(x);
}

// --- substate 2: recoil of a part --------------------------------------------------------------

fn recoil<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(DisplacerBeast::WANT_ANIM, 0x1e);
            let want = x.u8(DisplacerBeast::WANT_ANIM);
            x.set8(DisplacerBeast::CUR_ANIM, want);
            lib::sound(x, crate::sound::CMD_350_450);
            x.set16(Enemy::VX, 0x300);
            x.set16(Enemy::AX, (-0x28i16) as u16);
            let flip = x.u8(Enemy::ATTACKER_FLIP) ^ 0x20;
            x.set8(f::FLIP, flip);
            if flip != 0 {
                x.set16(Enemy::VX, 0x300u16.wrapping_neg());
                x.set16(Enemy::AX, 0x28);
            }
            play(x, 0x13);
        }
        2 => {
            let low = var::FRAME_COUNTER.get(x.b) & 3;
            var::FRAME_COUNTER.set(x.b, low);
            if low == 0 {
                lib::shock(x, 0, 2, 8, 0xffff);
            }
            if x.u8(Enemy::VX) != 0 {
                x.move_x();
                return;
            }
            if x.anim_step() {
                x.set32(Enemy::HIT_LOCK, 0);
                x.set32(Enemy::VX_LONG, 0);
                next_move(x);
            }
        }
        _ => {}
    }
}

// --- substate 4: fall --------------------------------------------------------------------------

fn fall<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set32(Enemy::VX_LONG, 0);
            x.set16(Enemy::VZ, 0x180);
            x.set16(Enemy::GRAVITY, (-0x30i16) as u16);
            play(x, 0xc);
        }
        2 => {
            let t = x.u16(Enemy::RECOVER).wrapping_sub(1);
            x.set16(Enemy::RECOVER, t);
            if (t as i16) > 0 {
                return;
            }
            x.add8(f::SUB2, 2);
            x.set8(Enemy::ACTIVE, 2);
            x.set16(Enemy::RECOVER, 0);
            x.set8(Enemy::DOWNED, 0);
            x.set8(Enemy::REACTION, 0);
        }
        4 => {
            if x.u8(f::KEY_FRAME) != 0 {
                x.add8(f::SUB2, 2);
                x.set8(Enemy::VULNERABLE, 0);
            }
            step(x);
        }
        6 => {
            if !x.move_xz().gt() {
                x.set32(Obj::Z, 0);
            }
            if x.anim_step() {
                x.set8(Enemy::ACTIVE, 1);
                x.set32(Enemy::VX_LONG, 0);
                x.set32(Enemy::VZ_LONG, 0);
                x.set8(Enemy::VULNERABLE, 1);
                set_state_long(x, ACTIVE_STANCE);
                play(x, 0);
            }
        }
        _ => {}
    }
}

// --- substate 8: death or flight ------------------------------------------------------------------

fn flee<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            let wall = var::SCENE.get(x.b);
            let flipped = x.u8(f::FLIP) != 0;
            let turn_back = if wall == 2 {
                flipped
            } else {
                if !lib::in_right_half(x) { flipped } else { !flipped }
            };
            if turn_back {
                x.add8(f::SUB2, 2);
                x.set8(Enemy::VULNERABLE, 1);
                x.set8(DisplacerBeast::WANT_ANIM, 5);
                x.set8(DisplacerBeast::CUR_ANIM, 5);
                play(x, 5);
            } else {
                x.add8(f::SUB2, 4);
                run_away(x);
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
            if var::SCENE.get(x.b) == 4 {
                return x.add8(f::SUB2, 2);
            }
            leave_screen(x);
        }
        6 => {
            if x.u8(Enemy::BLOCKED) != 0 {
                x.add8(f::SUB2, 2);
            }
            let t = crate::video::terrain_of(x.b, x.o);
                let terrain = t.number as u8;
            x.set8(Enemy::BLOCKED, terrain);
            x.move_by_table_xy();
            if x.in_arena_default() {
                step(x);
            } else {
                detach_from_host(x);
            }
        }
        8 => {
            x.add8(f::SUB2, 2);
            x.set8(Enemy::VULNERABLE, 1);
            x.set8(DisplacerBeast::WANT_ANIM, 6);
            x.set8(DisplacerBeast::CUR_ANIM, 6);
            let vx = if x.u8(f::FLIP) != 0 { 0x480 } else { -0x480 };
            set_velocity(x, vx, 0x4c0, -0x38);
            play(x, 6);
        }
        10 => {
            if x.u8(f::KEY_FRAME) != 0 {
                x.add8(f::SUB2, 2);
                x.set8(Enemy::VULNERABLE, 0);
            }
            step(x);
        }
        12 => {
            if x.move_xz().gt() {
                return step(x);
            }
            x.set32(Obj::Z, 0);
            x.add8(f::SUB2, 2);
            x.set8(Enemy::VULNERABLE, 1);
            x.set8(DisplacerBeast::WANT_ANIM, 0x18);
            x.set8(DisplacerBeast::CUR_ANIM, 0x18);
            play(x, 0xf);
        }
        14 => leave_screen(x),
        _ => {}
    }
}

/// 0x7444e: runs away toward the edge of the screen.
fn run_away<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Enemy::VULNERABLE, 1);
    x.set8(DisplacerBeast::WANT_ANIM, 0x16);
    x.set8(Obj::SPEED_INDEX, 8);
    if x.u8(f::FLIP) == 0 {
        x.set8(Obj::SPEED_INDEX, 0x18);
    }
    play(x, 0xd);
    speed(x, 0x21);
}

/// 0x7449e: keeps going until it leaves the screen, then disappears.
fn leave_screen<R: Bench>(x: &mut Cx<'_, R>) {
    x.move_by_table_xy();
    if x.in_arena_default() {
        return step(x);
    }
    set_state_long(x, 0x0600_0000);
    if x.u8(DisplacerBeast::PARENT) != 0 {
        var::FROZEN.set(x.b, 1);
    }
    detach_from_host(x);
}
