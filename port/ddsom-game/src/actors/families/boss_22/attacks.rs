//! Boss 22, group 4: the attacks (`+6` = 0 to 22). Each ends with `next_attack` (0x9d680).

use crate::actors::families::cx::Bench;
use super::moves::step_anim;
use super::*;
use crate::world::objects::Current;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    let step = x.u8(f::SUB2);
    match (two(x), step) {
        (_, 0 | 6) => shot(x),
        (_, 2 | 8) => breath(x),
        (_, 4) => bite(x),
        (true, 10) => two::slam(x),
        (true, 12) => two::spin_roar(x),
        (true, 16) | (false, 10 | 16) => second_form_strike(x),
        (true, 18) | (false, 12 | 18) => charge_strike(x),
        (_, 14) => stomp(x),
        (_, 20) => dive(x),
        (_, 22) => dive_down(x),
        _ => {}
    }
}

/// 0x9b752: key frame of the attack animation (sound 0xebfdc), then end when the animation is finished.
fn finish_attack<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::KEY_FRAME) != 0 {
        lib::sound(x, crate::services::sound::CMD_361);
        x.set8(f::KEY_FRAME, 0);
    }
    if x.anim_step() {
        next_attack(x);
    }
}

/// 0x9b712: shot.
fn shot<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            next7(x);
            x.set8(Enemy::VULNERABLE, 1);
            x.set8(Boss22::ANIM_ID, 0xa);
            sounds(x, 0x28, 5);
            x.set8(Boss22::ANIM_LAST, 0xa);
            start(x, Set::B, 0);
        }
        2 => finish_attack(x),
        _ => {}
    }
}

/// 0x9b774: breath.
fn breath<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            next7(x);
            if two(x) {
                x.set8(Enemy::VULNERABLE, 1);
            }
            x.set8(Boss22::ANIM_ID, 0xe);
            sounds(x, 0x29, 6);
            x.set8(Boss22::ANIM_LAST, 0xe);
            start(x, Set::B, 4);
        }
        2 => finish_attack(x),
        _ => {}
    }
}

/// 0x9bfb4: key frame of the bite: grabs a player within range (0xfc44, 0xfd64), otherwise end of the animation.
fn grab_on_key<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::KEY_FRAME) != 0 {
        if let Some(player) = lib::attack_hits_a_player(x, 5) {
            return lib::seize_player(x, player, 0);
        }
    }
    if x.anim_step() {
        next_attack(x);
    }
}

/// 0x9bf80: bite.
fn bite<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            next7(x);
            x.set8(Enemy::VULNERABLE, 1);
            x.set8(Boss22::ANIM_ID, 0xc);
            x.set8(Boss22::ANIM_LAST, 0xc);
            start(x, Set::B, 3);
        }
        2 => grab_on_key(x),
        _ => {}
    }
}

/// End of the run of a second-form attack: the boss turns toward its target if it has its back to it.
fn facing_wrong<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if x.u16(f::TARGET) == 0 {
        return false;
    }
    let t = x.linked(f::TARGET);
    let wanted = if (x.u16(Obj::X).wrapping_sub(t.u16(x.b, Obj::X)) as i16) < 0 { 0x20 } else { 0 };
    wanted != x.u8(f::FLIP)
}

/// 0x9c0ac: second-form attack (roar that triggers it, run, hit).
fn second_form_strike<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            if second_form(x) {
                return x.add8(Enemy::SUB_STEP, 4);
            }
            x.set8(Enemy::VULNERABLE, 1);
            next7(x);
            x.set8(Boss22::ANIM_ID, 0x11);
            x.set8(Boss22::ANIM_LAST, 0x11);
            sounds(x, 0x27, 5);
            start(x, Set::B, 7);
        }
        2 => {
            if x.u8(f::KEY_FRAME) != 0 {
                x.set8(f::KEY_FRAME, 0);
                eshl_wave_1f(x);
                x.set8(Boss22::SECOND, 1);
                x.set8(Boss22::SECOND_FX, 1);
            }
            if x.anim_step() {
                next7(x);
            }
        }
        4 => {
            if !second_form(x) {
                x.set8(Boss22::SECOND_FX, 0);
                return next_attack(x);
            }
            next7(x);
            x.set8(Enemy::VULNERABLE, 1);
            x.set8(Boss22::ANIM_ID, 5);
            if x.u8(Boss22::ANIM_ID) != x.u8(Boss22::ANIM_LAST) {
                x.set8(Boss22::ANIM_LAST, 5);
                start(x, Set::A, 0xc);
            }
            speed(x, 0x15);
        }
        6 => {
            rubble(x, 0, 0x201, 0x20, 0x20);
            if !second_form(x) {
                x.set8(Boss22::SECOND_FX, 0);
                return next_attack(x);
            }
            x.set8(Boss::SHOT, 0x23);
            lib::approach_target(x);
            if sub8(x, Boss::WALK_DELAY, 1).mi() {
                lib::draw_stance(x);
                let r = rand_and(x, 0xf);
                if 0xffc0u32 >> r & 1 != 0 {
                    lib::pick_target(x);
                } else {
                    lib::random_player(x);
                }
                if sub8(x, Boss::WALK_STEPS, 1).eq() {
                    lib::draw_6b(x);
                }
            }
            step_anim(x);
            x.move_by_table_xy();
            if facing_wrong(x) {
                next7(x);
            }
        }
        8 => {
            if !second_form(x) {
                x.set8(Boss22::SECOND_FX, 0);
                return next_attack(x);
            }
            next7(x);
            x.set8(Enemy::VULNERABLE, 1);
            x.set8(Boss22::ANIM_ID, 5);
            let angle = if x.u8(f::FLIP) != 0 { 8 } else { 0x18 };
            x.set8(Enemy::HEADING, angle);
            if x.u8(Boss22::ANIM_ID) != x.u8(Boss22::ANIM_LAST) {
                x.set8(Boss22::ANIM_LAST, 5);
                start(x, Set::A, 0xc);
            }
            speed(x, 0x15);
        }
        10 => {
            if !second_form(x) {
                x.set8(Boss22::SECOND_FX, 0);
                return next_attack(x);
            }
            if x.u16(f::TARGET) != 0 && still_close(x) {
                step_anim(x);
                x.move_by_table_xy();
                return;
            }
            next7(x);
            x.set8(Enemy::VULNERABLE, 1);
            start(x, Set::A, 0xf);
        }
        12 => {
            if !second_form(x) {
                x.set8(Boss22::SECOND_FX, 0);
                return next_attack(x);
            }
            if x.anim_step() {
                x.set8(Enemy::SUB_STEP, 4);
                let side = x.u8(f::FLIP) ^ 0x20;
                x.set8(f::FLIP, side);
                x.set8(Boss22::ANIM_ID, 5);
                x.set8(Boss22::ANIM_LAST, 5);
                start(x, Set::A, 0xc);
            }
        }
        _ => {}
    }
}

/// 0x9c29a: the boss stays within the screen and the target is close enough in front of it (as at the about-face of
/// the combat).
fn still_close<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let cam = var::SCROLL_X.get(x.b);
    if x.u8(f::FLIP) != 0 && (cam.wrapping_add(0x160).wrapping_sub(x.u16(Obj::X)) as i16) < 0 {
        return false;
    }
    if (cam.wrapping_add(0x20).wrapping_sub(x.u16(Obj::X)) as i16) >= 0 {
        return false;
    }
    let t = x.linked(f::TARGET);
    let d = t.u16(x.b, Obj::X).wrapping_sub(x.u16(Obj::X));
    d <= 0xe0 && x.u8(Enemy::BLOCKED) == 0
}

/// 0x9b7ae: charge (run toward the target, hit in range).
fn charge_strike<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            next7(x);
            x.set16(Boss22::TIMER, 0x32);
            charge_start(x);
        }
        6 => {
            x.set8(Enemy::SUB_STEP, 2);
            charge_start(x);
        }
        2 => {
            rubble(x, 0, 0x201, 0x20, 0x20);
            x.set8(Boss::SHOT, 0x25);
            lib::approach_target(x);
            if lib::target_in_box(x, -0x50, 0x30, 0x10) {
                next7(x);
                sounds(x, 0x29, 6);
                return start(x, Set::B, 4);
            }
            if lib::target_in_box(x, -0x10, 0x10, 0x10) || sub16(x, Boss22::TIMER, 1).mi() {
                return next_attack(x);
            }
            if needs_turn(x) {
                x.set32(Boss22::SAVED_STATE, 0x0204_1206);
                let step = x.u8(f::SUB2);
                x.set8(Boss22::SAVED_STEP, step);
                x.set32(f::STATE, state::TURN);
            }
            step_anim(x);
            x.move_by_table_xy();
        }
        4 => finish_attack(x),
        _ => {}
    }
}

fn charge_start<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Enemy::VULNERABLE, 1);
    x.set8(Boss22::ANIM_ID, 0xf);
    x.set8(Boss22::ANIM_LAST, 0xf);
    sounds(x, 0x27, 5);
    start(x, Set::A, 2);
    speed(x, 0x15);
}

/// 0x9bfe0: stomping (run toward a nearby player, then bite).
fn stomp<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            next7(x);
            x.set16(Boss22::TIMER, 0x3c);
            stomp_start(x);
        }
        6 => {
            x.set8(Enemy::SUB_STEP, 2);
            stomp_start(x);
        }
        2 => {
            rubble(x, 0, 0x201, 0x20, 0x20);
            x.set8(Boss::SHOT, 0x25);
            lib::approach_target(x);
            if player_within_reach(x) {
                next7(x);
                return start(x, Set::B, 3);
            }
            if sub16(x, Boss22::TIMER, 1).mi() {
                return next_attack(x);
            }
            if needs_turn(x) {
                x.set32(Boss22::SAVED_STATE, 0x0204_0e06);
                let step = x.u8(f::SUB2);
                x.set8(Boss22::SAVED_STEP, step);
                x.set32(f::STATE, state::TURN);
            }
            step_anim(x);
            x.move_by_table_xy();
        }
        4 => grab_on_key(x),
        _ => {}
    }
}

fn stomp_start<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Enemy::VULNERABLE, 1);
    x.set8(Boss22::ANIM_ID, 0xd);
    x.set8(Boss22::ANIM_LAST, 0xd);
    start(x, Set::A, 9);
    speed(x, 0x15);
}

/// 0x9c354: dive onto the target.
fn dive<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            if x.u16(f::TARGET) != 0 {
                next7(x);
                x.set8(Enemy::VULNERABLE, 1);
                let t = x.linked(f::TARGET);
                let (tx, tz) = (t.u16(x.b, Obj::X), t.u16(x.b, Obj::Y));
                // D2 = the target's depth (leftover)
                *x.c.d2() = tz as u32;
                let angle = lib::aim_from(x, (tx, tz));
                x.set8(Enemy::HEADING, angle);
                let v = if x.u8(f::FLIP) != 0 { 0x500u16 } else { (-0x500i16) as u16 };
                x.set16(Enemy::VX, v);
                x.set16(Enemy::VZ, 0x900);
                x.set16(Enemy::GRAVITY, (-0x50i16) as u16);
            }
            x.set8(Boss22::ANIM_ID, 0x11);
            x.set8(Boss22::ANIM_LAST, 0x11);
            sounds(x, 0x2a, 6);
            start(x, Set::B, 8);
        }
        2 => rise_wait(x),
        4 => climb(x, true),
        6 => climb(x, false),
        8 => land(x, false),
        10 => {
            if x.anim_step() {
                x.set8(Enemy::VULNERABLE, 1);
                next_attack(x);
            }
        }
        _ => {}
    }
}

fn rise_wait<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::KEY_FRAME) != 0 {
        next7(x);
        x.set8(Enemy::VULNERABLE, 0);
    }
    step_anim(x);
}

fn climb<R: Bench>(x: &mut Cx<'_, R>, up: bool) {
    x.move_xz();
    let vz = x.u16(Enemy::VZ);
    let reached = if up { Fl::sub16(vz, 0x100).gt() } else { Fl::sub16(vz, 0xff00).gt() };
    if !reached {
        next7(x);
        x.set16(Obj::ANIM_TIMER, 1);
    }
    step_anim(x);
}

/// Landing of the dive: sound, dust in a line; `down` (0x9c4a2) adds five bursts and does not make the boss
/// vulnerable.
fn land<R: Bench>(x: &mut Cx<'_, R>, down: bool) {
    x.move_xz();
    if (x.u32(Obj::Z) as i32) > 0 {
        return step_anim(x);
    }
    lib::shake_54(x, 0x1e);
    lib::sound(x, if down { crate::services::sound::CMD_371 } else { crate::services::sound::CMD_372 });
    x.set32(Obj::Z, 0);
    dust_line(x);
    next7(x);
    x.set16(Obj::ANIM_TIMER, 1);
    if down {
        scatter(x);
    } else {
        x.set8(Enemy::VULNERABLE, 1);
    }
    start(x, Set::A, 6);
}

/// 0x9c3ee: jump in place with a landing in a puff of dust.
fn dive_down<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            next7(x);
            x.set8(Enemy::VULNERABLE, 1);
            x.set16(Enemy::VZ, 0xb00);
            x.set16(Enemy::GRAVITY, (-0x70i16) as u16);
            x.set16(Enemy::VX, 0);
            x.set16(Enemy::AX, 0);
            sounds(x, 0x2a, 5);
            x.set8(Boss22::ANIM_ID, 0x13);
            x.set8(Boss22::ANIM_LAST, 0x13);
            start(x, Set::B, 0xa);
        }
        2 => {
            if x.u8(f::KEY_FRAME) != 0 {
                next7(x);
                x.set8(Enemy::VULNERABLE, 0);
            }
            step_anim(x);
        }
        4 => climb(x, true),
        6 => climb(x, false),
        8 => land(x, true),
        10 => {
            if x.anim_step() {
                x.set8(Enemy::ACTIVE, 1);
                x.set8(Enemy::VULNERABLE, 1);
                next_attack(x);
            }
        }
        _ => {}
    }
}
