//! Boss 3, active state other than attacks: arrival (`+5` = 0), combat (2), patrol (6), takeoff (8), wait (10),
//! player carried off (12), counterattack (14).

use crate::actors::families::cx::Bench;
use crate::world::objects::player::Player;
use super::*;
use crate::world::objects::Current;

// --- group 0: arrival ------------------------------------------------------------------------

/// 0x7f5b8: variant 0 descends from the top of the screen (in cutscene); the others fight right away.
pub(super) fn intro<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Enemy::VARIANT) != 0 {
        return enter_fight(x);
    }
    match x.u8(f::SUB2) {
        0 => {
            next6(x);
            let cam = camera(x);
            let z = cam.u16(x.b, Obj::Z).wrapping_add(0x80);
            x.set16(Obj::Y, z);
            x.set16(Obj::Z, (-0x80i16) as u16);
            stop(x);
            x.set16(Enemy::VZ, 0x380);
            body_anim(x, 0xc);
        }
        2 => {
            if var::PAUSE.get(x.b) != 0 {
                return hidden(x);
            }
            let cam = camera(x);
            let px = cam.u16(x.b, Obj::X).wrapping_add(0x100);
            x.set16(Obj::X, px);
            lib::step(x);
            x.move_z();
            if (x.u16(Obj::Z) as i16) < 0x80 {
                return hidden(x);
            }
            next6(x);
            let z = cam.u16(x.b, Obj::Z).wrapping_add(0x40);
            x.set16(Obj::Y, z);
            x.set16(Obj::Z, 0xc0);
            stop(x);
            x.set16(Enemy::VZ, (-0x600i16) as u16);
            x.set16(Enemy::GRAVITY, (-0x60i16) as u16);
            x.set8(Boss3::TIMER, 0x20);
            body_anim(x, 0xd);
            x.unlink();
            end_frame(x, 0x50);
        }
        4 => {
            x.set_bit(Enemy::DISPLAY, 1, true);
            if sub8(x, Boss3::TIMER, 1).pl() {
                return end_frame(x, 0x50);
            }
            next6(x);
            lib::sound(x, crate::services::sound::CMD_21);
            x.set_bit(Enemy::DISPLAY, 1, false);
            descend(x);
        }
        6 => descend(x),
        _ => {
            if var::PAUSE.get(x.b) == 0 && lib::step(x) {
                return enter_fight(x);
            }
            end_frame(x, 0x50);
        }
    }
}

/// 0x7f67a: still off screen: held back, put back in the foreground list.
fn hidden<R: Bench>(x: &mut Cx<'_, R>) {
    lib::keep_on_screen(x, 0x50);
    x.reinsert_foreground();
}

/// 0x7f6a8: descent to the ground, then cry.
fn descend<R: Bench>(x: &mut Cx<'_, R>) {
    if var::PAUSE.get(x.b) == 0 {
        lib::step(x);
        x.move_z();
        if (x.u16(Obj::Z) as i16) <= 0 {
            x.set16(Obj::Z, 0);
            next6(x);
            stop(x);
            crate::actors::families::effects_creators::scenery_0(x.b, x.o);
            lib::shake_54(x, 0x30);
            body_anim(x, 0xe);
        }
    }
    end_frame(x, 0x50);
}

/// 0x7f712: the type's immunities, then the combat begins in the same frame (approach).
fn enter_fight<R: Bench>(x: &mut Cx<'_, R>) {
    lib::set_immunities(x);
    x.set32(f::STATE, 0x0202_0400);
    run(x);
}

// --- groupe 2 : combat -------------------------------------------------------------------------

/// 0x7f720: one step of the combat; with no player alive, wait (or rise if the HP threshold changes).
pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => hover(x),
        2 => turn(x),
        4 | 8 => approach(x),
        6 => sweep(x),
        10 => climb(x),
        12 => hold_high(x),
        14 => land(x),
        _ => patrol(x),
    }
    if var::PLAYERS_ALIVE.get(x.b) != 0 {
        return;
    }
    idle_alone(x);
    if wound_changed(x) {
        set_steps(x, 0x0c00);
    }
}

/// 0x7f75e: hovering at ground level; in the air or wounded, it climbs back up.
fn hover<R: Bench>(x: &mut Cx<'_, R>) {
    if (x.u16(Obj::Z) as i16) > 0 || wound_changed(x) {
        x.set8(Boss3::INVULNERABLE, 1);
        return set_steps(x, 0x0a00);
    }
    if x.u8(Enemy::SUB_STEP) == 0 {
        next7(x);
        x.set8(Boss3::FLYING, 0);
        x.set8(Enemy::VULNERABLE, 1);
        return body_anim(x, 0);
    }
    if !sub8(x, Boss::IDLE_COUNT, 1).gt() {
        return x.set32(f::STATE, 0x0202_0400);
    }
    if target_behind(x) {
        x.set32(Boss3::SAVED_STATE, 0x0202_0400);
        x.set32(f::STATE, 0x0202_0200);
    }
    lib::step(x);
}

/// 0x7f7ca: about-face (in flight: `+0xa6` frames at most), then approach.
fn turn<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Enemy::SUB_STEP) == 0 {
        next7(x);
        let side = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, side);
        x.set8(Boss3::FLYING, 0);
        x.set8(Enemy::VULNERABLE, 1);
        x.set8(Obj::SPEED_INDEX, 0x18);
        if x.u8(f::FLIP) != 0 {
            x.set8(Boss3::TIMER, 0x20);
            x.set8(Obj::SPEED_INDEX, 8);
        }
        return body_anim(x, 2);
    }
    if !lib::step(x) && sub8(x, Boss3::TIMER, 1).gt() {
        return;
    }
    set_steps(x, 0x0400);
    body_anim(x, 0);
}

/// Every `+0x71` frames, new target (0xa470, then 0x424e or 0x44ee at random); after `+0x6b` choices, the step
/// count (0x788c, 0x79ce) may send it back to hovering. False if there is no target left (wait), or if the
/// step has changed.
fn retarget<R: Bench>(x: &mut Cx<'_, R>, hover_steps: u16) -> Option<bool> {
    if sub8(x, Boss::WALK_DELAY, 1).pl() {
        return Some(true);
    }
    lib::draw_stance(x);
    let r = rand_and(x, 0xf);
    let ok = if 0xffc0u32 >> r & 1 != 0 { pick_target(x) } else {
        lib::random_player(x)
    };
    if !ok {
        idle_alone(x);
        return None;
    }
    if sub8(x, Boss::WALK_STEPS, 1).ne() {
        return Some(true);
    }
    lib::draw_6b(x);
    if !lib::maybe_draw_6e(x) {
        return Some(true);
    }
    set_steps(x, hover_steps);
    Some(false)
}

/// 0x7f842: approach to the target on the ground; depending on the boxes in front of it, it attacks.
fn approach<R: Bench>(x: &mut Cx<'_, R>) {
    if wound_changed(x) {
        return set_steps(x, 0x0a00);
    }
    if !pick_target(x) {
        return idle_alone(x);
    }
    if target_far(x) {
        return x.set32(f::STATE, 0x0202_0600);
    }
    if x.u8(Enemy::SUB_STEP) == 0 {
        next7(x);
        x.set8(Boss3::FLYING, 0);
        x.set8(Enemy::VULNERABLE, 1);
        body_anim(x, 3);
        return speed(x, 0x15);
    }
    let aim = if x.u8(f::SUB2) == 0 { 0x2f } else { 0x30 };
    x.set8(Boss3::AIM, aim);
    lib::approach_target(x);
    match retarget(x, 0) {
        None | Some(false) => return,
        Some(true) => {}
    }
    x.set8(Boss3::APPROACHED, 2);
    if lib::target_in_box(x, -0x20, 0x20, 0x10) {
        lib::pick_byte(x);
        return choose_attack(x, false);
    }
    if lunge_box(x, -0x50, 0x20, false) || lunge_box(x, -0xc0, 0x30, true) {
        return;
    }
    if lib::guard_in_front_strict(x, -0x60, 0x60, 0x10) {
        return lib::pick_byte(x);
    }
    lib::step(x);
    x.move_by_table_xy();
    if target_behind(x) {
        x.set32(Boss3::SAVED_STATE, 0x0202_0000);
        x.set32(f::STATE, 0x0202_0200);
    }
}

/// 0x7f984 / 0x7f9fe: target in the box (and on the right side: 0xa4c2 if it faces like the boss, 0xa4b4
/// otherwise): patrol (6 chances in 16) or attack. True if the step changed.
fn lunge_box<R: Bench>(x: &mut Cx<'_, R>, d0: i16, d1: u16, far: bool) -> bool {
    if !lib::target_in_box(x, d0, d1, 0x10) {
        return false;
    }
    if x.u16(f::TARGET) == 0 {
        idle_alone(x);
        return true;
    }
    let t = load_target(x);
    let side = t.u8(x.b, f::FLIP);
    let same_side = side == x.u8(f::FLIP);
    if !lib::decision(x, same_side) {
        return false;
    }
    let r = rand_and(x, 0xf);
    if 0xffc0u32 >> r & 1 != 0 {
        x.set32(f::STATE, 0x0202_0800);
        return true;
    }
    x.set32(f::STATE, 0x0204_0000);
    lib::pick_byte(x);
    choose_attack(x, far);
    true
}

/// 0x7fa78: in low flight (`+0xa2`), approach at height 0x30; any target in a box triggers an attack.
fn patrol<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Boss3::HOVER, 0);
    if wound_changed(x) {
        return set_steps(x, 0x0e00);
    }
    if !pick_target(x) {
        return idle_alone(x);
    }
    if x.u8(Enemy::SUB_STEP) == 0 {
        next7(x);
        x.set8(Boss3::FLYING, 1);
        x.set8(Enemy::VULNERABLE, 0);
        speed(x, 0x17);
        return body_anim(x, 1);
    }
    x.set8(Boss3::AIM, 0x23);
    lib::approach_target(x);
    match retarget(x, 0x0c00) {
        None | Some(false) => return,
        Some(true) => {}
    }
    lib::step(x);
    let h = x.u16(Obj::Z) as i16;
    if h != 0x30 {
        let d = if h < 0x30 { 1 } else { 0xffff };
        x.add16(Obj::Z, d);
    }
    if lib::target_in_box(x, -0x18, 0x30, 0x18) || lib::target_in_box(x, -0xc0, 0x30, 0x10) || lib::target_in_box(x, -0x50, 0x20, 0x10) {
        lib::pick_byte(x);
        return choose_attack(x, false);
    }
    x.move_by_table_xy();
    if target_behind(x) {
        let side = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, side);
    }
}

/// 0x7fb94: holds altitude for `+0x6e` frames, then lands.
fn hold_high<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Boss3::HOVER, 0);
    if wound_changed(x) {
        return set_steps(x, 0x0e00);
    }
    if !sub8(x, Boss::IDLE_COUNT, 1).gt() {
        return set_steps(x, 0x1000);
    }
    if target_behind(x) {
        let side = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, side);
    }
    lib::step(x);
}

/// 0x7fbca: rises up to altitude 0x30.
fn climb<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Boss3::HOVER, 0);
    if x.u8(Enemy::SUB_STEP) == 0 {
        next7(x);
        x.set8(Boss3::FLYING, 1);
        x.set8(Boss3::INVULNERABLE, 1);
        stop(x);
        x.set16(Enemy::VZ, 0x400);
        return body_anim(x, 1);
    }
    lib::step(x);
    x.move_z();
    if (x.u16(Obj::Z) as i16) < 0x30 {
        return;
    }
    x.set8(Boss3::INVULNERABLE, 1);
    x.set16(Obj::Z, 0x30);
    set_steps(x, 0x0c00);
}

/// 0x7fc34: descends to the ground, then hovers.
fn land<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Boss3::HOVER, 0);
    if x.u8(Enemy::SUB_STEP) == 0 {
        next7(x);
        stop(x);
        x.set16(Enemy::VZ, (-0x400i16) as u16);
    }
    lib::step(x);
    x.move_z();
    if (x.u16(Obj::Z) as i16) > 0 {
        return;
    }
    x.set16(Obj::Z, 0);
    set_steps(x, 0);
}

/// 0x7fc6e: straight-line flight (`+0xa6` frames) toward its target, then approach.
fn sweep<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Boss3::HOVER, 0);
    if x.u8(Enemy::SUB_STEP) == 0 {
        next7(x);
        x.set16(Boss3::TIMER, 0x1e);
        x.set8(Boss3::FLYING, 0);
        x.set8(Enemy::VULNERABLE, 1);
        speed(x, 0x1f);
        return body_anim(x, 1);
    }
    if sub16(x, Boss3::TIMER, 1).mi() {
        return x.set32(f::STATE, 0x0202_0400);
    }
    x.set8(Boss3::AIM, 0x2f);
    lib::approach_target(x);
    if target_behind(x) {
        x.set32(Boss3::SAVED_STATE, 0x0202_0400);
        x.set32(f::STATE, 0x0202_0200);
    }
    x.move_by_table_xy();
    lib::step(x);
}

// --- group 6: round, group 8: takeoff -------------------------------------------------------------

/// Waypoints of the patrol, per quarter of the screen (words X, Z, bytes angle and direction).
const ROUND_POINTS: &str = "boss_3/round_points";

/// 0x806b4: patrol (`+6` = 0, 2), takeoff jump (4); with no player alive, wait.
pub(super) fn round<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 | 2 => circling(x),
        4 => take_off(x),
        _ => {
            x.set8(Boss3::INVULNERABLE, 1);
            x.set32(f::STATE, 0x0202_0a00);
            circling(x)
        }
    }
    if var::PLAYERS_ALIVE.get(x.b) == 0 {
        idle_alone(x);
    }
}

/// 0x81810: waypoint opposite to the quarter of the screen where the boss is; Z = zero direction.
fn pick_round_point<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let cam = camera(x);
    let dx = x.u16(Obj::X).wrapping_sub(cam.u16(x.b, Obj::X)) as i16;
    let dz = x.u16(Obj::Y).wrapping_sub(cam.u16(x.b, Obj::Z)) as i16;
    let quarter: u32 = match (dx > 0xc0, dz > 0x80) {
        (false, false) => 0,
        (false, true) => 6,
        (true, false) => 0xc,
        (true, true) => 0x12,
    };
    let gx = word(x, ROUND_POINTS, quarter).wrapping_add(cam.u16(x.b, Obj::X));
    x.set16(Boss3::GOAL_X, gx);
    let gz = word(x, ROUND_POINTS, 2 + quarter).wrapping_add(cam.u16(x.b, Obj::Z));
    x.set16(Boss3::GOAL_Z, gz);
    let angle = byte(x, ROUND_POINTS, 4 + quarter);
    x.set8(Obj::SPEED_INDEX, angle);
    let turn = byte(x, ROUND_POINTS, 5 + quarter);
    x.set8(Boss3::GOAL_TURN, turn);
    turn != 0
}

/// 0x806ea: flies toward the waypoint while turning; on arrival (or a target far ahead): approach.
fn circling<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Enemy::SUB_STEP) == 0 {
        next7(x);
        x.set16(Boss3::TIMER, 6);
        if pick_round_point(x) {
            body_anim(x, 3);
        }
        speed(x, 0x15);
        x.move_by_table_xy();
        lib::step(x);
        return;
    }
    let arrived = lib::target_in_box(x, 0x80, 0x100, 0x10) || {
        let dx = x.u16(Boss3::GOAL_X).wrapping_sub(x.u16(Obj::X)).wrapping_add(8);
        dx <= 0x10 && {
            let dz = x.u16(Boss3::GOAL_Z).wrapping_sub(x.u16(Obj::Y)).wrapping_add(8);
            dz <= 0x10
        }
    };
    if arrived {
        return x.set32(f::STATE, 0x0202_0400);
    }
    if sub16(x, Boss3::TIMER, 1).mi() {
        let turn = if x.u8(Boss3::GOAL_TURN) != 0 { 0xff } else { 1 };
        let a = x.u8(Obj::SPEED_INDEX).wrapping_add(turn) & 0x1f;
        x.set8(Obj::SPEED_INDEX, a);
        x.set16(Boss3::TIMER, 6);
    }
    x.move_by_table_xy();
    lib::step(x);
}

/// 0x807aa: takeoff jump (forward), landing, then approach.
fn take_off<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Boss3::INVULNERABLE, 1);
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            next7(x);
            let angle = if x.u8(f::FLIP) != 0 { 0x18 } else { 8 };
            x.set8(Obj::SPEED_INDEX, angle);
            stop(x);
            x.set16(Enemy::VZ, 0x300);
            x.set16(Enemy::GRAVITY, (-0x60i16) as u16);
            speed(x, 0x15);
            body_anim(x, 0xa);
        }
        2 => {
            lib::step(x);
            if x.u8(f::KEY_FRAME) == 0 {
                x.move_by_table_xy();
                x.move_z();
            }
            if (x.u16(Obj::Z) as i16) >= 0 {
                return;
            }
            x.set16(Obj::Z, 0);
            x.set32(Enemy::VZ_LONG, 0);
            next7(x);
            body_anim(x, 0xb);
        }
        _ => x.set32(f::STATE, 0x0202_0400),
    }
}

/// 0x80844: takeoff: standing for a while (`+0xa6`), then approach; or jump (`+6` = 2), or patrol (4).
pub(super) fn rise<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => match x.u8(Enemy::SUB_STEP) {
            0 => {
                next7(x);
                x.set16(Boss3::TIMER, 0x14);
                x.set8(Enemy::VULNERABLE, 1);
                body_anim(x, 0);
            }
            2 => {
                if !sub16(x, Boss3::TIMER, 1).gt() {
                    return x.set32(f::STATE, 0x0202_0000);
                }
                if target_behind(x) {
                    x.set32(Boss3::SAVED_STATE, 0x0208_0004);
                    return x.set32(f::STATE, 0x0202_0200);
                }
                lib::step(x);
            }
            _ => {
                x.add8(Enemy::SUB_STEP, 0xfe);
                x.set8(Enemy::VULNERABLE, 1);
                body_anim(x, 0);
            }
        },
        2 => take_off(x),
        4 => circling(x),
        _ => x.set32(f::STATE, 0x0202_0a00),
    }
}

// --- group 10: wait ------------------------------------------------------------------------------

/// 0x808d4: a player comes back: low flight or approach; otherwise the boss stays in its pose (on the ground or in the
/// air).
pub(super) fn wait_for_players<R: Bench>(x: &mut Cx<'_, R>) {
    if var::PLAYERS_ALIVE.get(x.b) != 0 && pick_target(x) {
        let s = if x.u8(Boss3::FLYING) != 0 { 0x0202_0a00 } else { 0x0202_0400 };
        return x.set32(f::STATE, s);
    }
    let anim = if x.u8(Boss3::FLYING) == 0 && (x.u16(Obj::Z) as i16) <= 0 {
        0
    } else {
        x.set8(Boss3::HOVER, 0);
        1
    };
    if x.u8(Enemy::SUB_STEP) == 0 {
        next7(x);
        return body_anim(x, anim);
    }
    lib::step(x);
}

// --- group 12: a player carried off --------------------------------------------------------------

/// 0x80948: the grabbed player (0xfd64) is carried into the air then dropped from high (0xfd8c).
pub(super) fn carrying<R: Bench>(x: &mut Cx<'_, R>) {
    fn held<R: Bench>(x: &mut Cx<'_, R>) -> Option<Obj> {
        let t = x.u16(f::TARGET);
        if (t as i16) >= 0 {
            return None;
        }
        let p = load_target(x);
        (p.u8(x.b, Enemy::ACTIVE) != 0 && p.u8(x.b, Player::HELD) != 0).then_some(p)
    }
    if held(x).is_none() {
        return x.set32(f::STATE, 0x0202_0c00);
    }
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            next7(x);
            let v = ((x.u16(Enemy::VX) as i16) >> 2) as u16;
            x.set16(Enemy::VX, v);
            x.set16(Enemy::AX, 0);
            x.set32(Enemy::VZ_LONG, 0);
            let t = x.u16(f::TARGET);
            if (t as i16) >= 0 {
                return x.set32(f::STATE, 0x0202_0c00);
            }
            let p = load_target(x);
            let (px, pz, ph) = (p.u16(x.b, Obj::X), p.u16(x.b, Obj::Y), p.u16(x.b, Obj::Z));
            x.set16(Obj::X, px);
            x.set16(Obj::Y, pz);
            x.set16(Obj::Z, ph.wrapping_add(0x30));
            x.set8(Boss3::TIMER, 0x10);
            attack_anim(x, 0x12);
            lib::hold_seized(x);
        }
        2 => {
            lib::step(x);
            x.move_xz();
            if (x.u16(Obj::Z) as i16) <= 0x30 {
                x.set16(Obj::Z, 0x30);
            }
            if !sub8(x, Boss3::TIMER, 1).gt() {
                next7(x);
                stop(x);
                let mut v = 0x200u16;
                if x.u8(f::FLIP) == 0 {
                    v = v.wrapping_neg();
                }
                x.set16(Enemy::VX, v);
                x.set16(Enemy::VZ, 0x200);
                x.set16(Enemy::GRAVITY, 0x20);
                attack_anim(x, 0x13);
            }
            lib::hold_seized(x);
        }
        _ => {
            lib::step(x);
            x.move_xz();
            if (x.u16(Obj::Z) as i16) <= 0x70 {
                return lib::hold_seized(x);
            }
            let p = load_target(x);
            p.set_u8(x.b, Player::TIMER_6F, 0x10);
            lib::release_player(x, p);
            x.set32(f::STATE, 0x0202_0c00);
        }
    }
}

// --- group 14: riposte ----------------------------------------------------------------------------

/// 0x80a5c: counterattack drawn according to difficulty (0xffda): attack 16 (diving rush) or 20 (low sweep).
pub(super) fn counter<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Enemy::DOWNED, 0);
    let s = if !lib::roll_ffda(x) { 0x0204_1000 } else { 0x0204_1400 };
    x.set8(Enemy::ACTIVE, 1);
    x.set8(Boss3::INVULNERABLE, 6);
    x.set32(f::STATE, s);
}
