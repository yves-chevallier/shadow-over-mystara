//! Boss 16, active movement states: wake-up (`+5` = 0), combat (2), leap toward a point (6), wait before an
//! attack (8), pause (10), grabbed player (12).

use crate::families::cx::Bench;
use super::*;
use crate::objects::Current;

// --- group 0: wake-up --------------------------------------------------------------------------

/// 0x93454: variants 0 and 1 wait for a player to come close; the others go straight to combat.
pub(super) fn wake<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::VARIANT) {
        0 => wake_low(x),
        1 => wake_high(x),
        _ => fight_from_wake(x),
    }
}

/// 0x93606: straight into combat (group 2 runs in the same frame).
fn fight_from_wake<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Enemy::ACTIVE, 1);
    lib::set_immunities(x);
    x.set32(f::STATE, state::WALK);
    fight(x);
}

/// 0x9351a: proximity level (0 to 7) of the nearest player, from the horizontal distance (the original's vertical
/// test compares the player's position with itself: it is always zero).
fn proximity<R: Bench>(x: &mut Cx<'_, R>) -> u8 {
    let mut best: u16 = 0x7fff;
    for k in 0..4u32 {
        let p = crate::ram::PLAYERS.nth(k);
        if p.u8(x.b, Obj::ACTIVE) == 0 {
            continue;
        }
        let mut dx = p.u16(x.b, Obj::X).wrapping_sub(x.u16(Obj::X));
        if (dx as i16) < 0 {
            dx = dx.wrapping_neg();
        }
        if Fl::sub16(dx, best).le() {
            best = dx;
        }
    }
    if (best as i16) >= 0x1ff {
        return 0;
    }
    byte(x, "boss_16/proximity", (best >> 4) as u32)
}

/// 0x93470: variant 0, crouched: turns around when a player approaches, and attacks when the wake-up animation
/// is finished.
fn wake_low<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Boss16::FALLS, 1);
    match x.u8(f::SUB2) {
        0 => {
            next6(x);
            x.set8(Enemy::SUB_STEP, 0);
            x.set8(Boss16::NEAR, 0);
            x.set8(f::FLIP, 0x20);
            x.set8(Boss16::TIMER, 0x7f);
            anim_a(x, 0);
        }
        2 => {
            let level = proximity(x);
            let mut near = x.u8(Boss16::NEAR);
            let mut changed = false;
            if near == 0 {
                if sub8(x, Boss16::TIMER, 1).gt() {
                    // not yet
                } else {
                    near = 2;
                    x.set8(Boss16::NEAR, near);
                    changed = true;
                }
            }
            if !changed && (level as i8) > near as i8 {
                x.set8(Boss16::NEAR, level);
                changed = true;
            }
            if changed && x.u16(Boss16::ANIM_SET) != 7 {
                    let side = x.u8(f::FLIP) ^ 0x20;
                    x.set8(f::FLIP, side);
                    return anim_a(x, 7);
                }
            if lib::step(x) {
                x.set8(Enemy::ACTIVE, 1);
                lib::set_immunities(x);
                return x.set32(f::STATE, 0x0204_0800);
            }
            let i = x.u8(Boss16::NEAR) as u32;
            let v = byte(x, "boss_16/near_limits", i);
            if Fl::sub8(v, x.u8(Boss16::NEAR_MAX)).ge() {
                return;
            }
            x.set8(Boss16::NEAR_MAX, v);
        }
        _ => {}
    }
}

/// 0x93582: variant 1, up high: starts palette script 0x10298 in time with the animation, then combat.
fn wake_high<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            next6(x);
            x.set8(Enemy::SUB_STEP, 0);
            x.set8(Boss16::TIMER, 0x7f);
            anim_a(x, 8);
            lib::palette_script(x, 0);
            wake_high_wait(x)
        }
        2 => wake_high_wait(x),
        _ => {}
    }
}

/// 0x935b0.
fn wake_high_wait<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Enemy::SUB_STEP) == 0 {
        if proximity(x) == 0 {
            return;
        }
        next7(x);
    }
    x.set8(Boss16::FALLS, 1);
    if x.anim_step() {
        lib::palette_script(x, 7);
        return fight_from_wake(x);
    }
    let event = x.u8(Enemy::STRIKE_KEY);
    if event == 8 {
        var::SHAKE.set(x.b, 2);
        crate::palette::set_flash_color(x.b, crate::palette::FLASH_COLOR_RED);
    }
    let event = x.u8(Enemy::STRIKE_KEY);
    if event == 0 {
        return;
    }
    x.set8(Enemy::STRIKE_KEY, 0);
    lib::palette_script(x, (event - 1) as u16);
}

// --- group 2: combat -------------------------------------------------------------------------

/// 0x93618: combat; with no player alive, the boss waits.
pub(super) fn fight<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => stand(x),
        2 => turn(x),
        4 => walk(x, 0x2f),
        6 => leap(x),
        8 => walk(x, 0x30),
        _ => {}
    }
    if var::PLAYERS_ALIVE.get(x.b) == 0 {
        x.set32(f::STATE, state::IDLE);
    }
}

/// 0x93642: standing, waits `+0x6e` frames, turns around if the target is behind.
fn stand<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Boss16::FALLS, 1);
    if x.u8(Enemy::SUB_STEP) == 0 {
        next7(x);
        x.set8(Enemy::VULNERABLE, 1);
        return anim_a(x, 0);
    }
    if sub8(x, Boss::IDLE_COUNT, 1).le() {
        return x.set32(f::STATE, state::WALK);
    }
    if target_behind(x) {
        x.set32(Boss16::SAVED_STATE, state::WALK);
        x.set32(f::STATE, state::TURN);
    }
    lib::step(x);
}

/// 0x9368e: about-face.
fn turn<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Boss16::FALLS, 1);
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            next7(x);
            let side = x.u8(f::FLIP) ^ 0x20;
            x.set8(f::FLIP, side);
            x.set8(Enemy::VULNERABLE, 1);
            x.set8(Enemy::HEADING, if side != 0 { 8 } else { 0x18 });
            anim_a(x, 1);
        }
        2 => {
            if lib::step(x) {
                let s = x.u32(Boss16::SAVED_STATE);
                x.set32(f::STATE, s);
            }
        }
        _ => {}
    }
}

/// 0x936ee (kind 0x2f) and 0x936e6 (kind 0x30): walk toward the target; in range, it becomes an attack.
fn walk<R: Bench>(x: &mut Cx<'_, R>, kind: u8) {
    x.set8(Boss::SHOT, kind);
    x.set8(Boss16::FALLS, 1);
    if x.u8(Enemy::SUB_STEP) == 0 {
        next7(x);
        x.set8(Enemy::VULNERABLE, 1);
        anim_a(x, 0);
        speed(x, 3);
        if !pick_target(x) {
            return idle(x);
        }
        lib::draw_stance(x);
        return lib::draw_6b(x);
    }
    if (x.u16(Obj::Z) as i16) <= 0 {
        lib::approach_target(x);
    }
    if sub8(x, Boss::WALK_DELAY, 1).mi() {
        lib::draw_stance(x);
        let r = rand_and(x, 0xf);
        if 0xffc0u32 >> r & 1 != 0 {
            if !pick_target(x) {
                return idle(x);
            }
        } else {
            if !lib::random_player(x) {
                return idle(x);
            }
        }
        if sub8(x, Boss::WALK_STEPS, 1).ne() {
            // the walk continues
        } else {
            lib::draw_6b(x);
            if lib::maybe_draw_6e(x) {
                x.set16(f::SUB2, 0);
                return;
            }
        }
    }
    if close_target(x) || edge_target(x, -0x50, 0x20, 0x10) {
        return attack_choice(x);
    }
    if edge_target(x, -0xc0, 0x30, 0x10) {
        return attack_choice(x);
    }
    if target_far(x) {
        return x.set32(f::STATE, state::LEAP);
    }
    lib::step(x);
    x.move_by_table_xy();
    if target_behind(x) {
        x.set32(Boss16::SAVED_STATE, state::STAND);
        x.set32(f::STATE, state::TURN);
    }
}

/// 0x949b2 and 0x949d4: a target in range; on a die roll the boss charges (0x0202_0800), otherwise it chooses an
/// attack from table 0x94a24 (the original's second table, 0x94c24, is overwritten before being read).
fn attack_choice<R: Bench>(x: &mut Cx<'_, R>) {
    let r = rand_and(x, 0xf);
    if 0xffc0u32 >> r & 1 != 0 {
        return x.set32(f::STATE, state::WALK_B);
    }
    let r = rand_and(x, 0x1f);
    let step = byte(x, "boss_16/attack_picks", r);
    x.set8(f::SUB2, step);
    x.set16(f::STATE, 0x0204);
    x.set8(Enemy::SUB_STEP, 0);
    lib::pick_byte(x);
}

/// 0x937e2: leap toward the target (the boss is far from it).
fn leap<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Boss16::FALLS, 1);
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            next7(x);
            x.set16(Boss16::TIMER, 0x1e);
            x.set8(Enemy::VULNERABLE, 1);
            speed(x, 8);
            anim_a(x, 2);
        }
        2 => {
            if sub16(x, Boss16::TIMER, 1).mi() {
                return x.set32(f::STATE, state::WALK);
            }
            x.set8(Boss::SHOT, 0x2f);
            lib::approach_target(x);
            if target_behind(x) {
                x.set32(Boss16::SAVED_STATE, state::STAND);
                x.set32(f::STATE, state::TURN);
            }
            x.move_by_table_xy();
            lib::step(x);
        }
        _ => {}
    }
}

// --- group 6: run to a point on the screen ---------------------------------------------------

/// 0x93cfa: run (`+6` = 0, 2) then jump (4) toward a point near the camera; with no player, wait.
pub(super) fn jump_to_spot<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 | 2 => run_to_spot(x),
        4 => spot_jump(x),
        _ => {}
    }
    if var::PLAYERS_ALIVE.get(x.b) == 0 {
        x.set32(f::STATE, state::IDLE);
    }
}

/// 0x9514a: chooses the arrival point (`+0xae`, `+0xb0`), the rotation direction (`+0xb3`) and the angle (`+0x2d`)
/// from the boss's position relative to the camera (four corners, table 0x951b0: X, Z, angle, direction).
fn choose_spot<R: Bench>(x: &mut Cx<'_, R>) {
    let (cam_x, cam_y) = camera(x);
    let dx = x.u16(Obj::X).wrapping_sub(cam_x);
    let dz = x.u16(Obj::Y).wrapping_sub(cam_y);
    let row = if Fl::sub16(dx, 0xc0).gt() {
        if Fl::sub16(dz, 0x80).gt() { 0x12 } else { 0xc }
    } else if Fl::sub16(dz, 0x80).gt() {
        6
    } else {
        0
    };
    let sx = word(x, SPOTS, row).wrapping_add(cam_x);
    x.set16(Boss16::SPOT_X, sx);
    let sz = word(x, SPOTS, row + 2).wrapping_add(cam_y);
    x.set16(Boss16::SPOT_Z, sz);
    let angle = byte(x, SPOTS, row + 4);
    x.set8(Enemy::HEADING, angle);
    let spin = byte(x, SPOTS, row + 5);
    x.set8(Boss16::SPIN, spin);
}

/// Corners of the screen to run to (6-byte rows: X and Z from the camera, heading, direction of the spin).
const SPOTS: &str = "boss_16/spots";

/// 0x93d20.
fn run_to_spot<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            next7(x);
            x.set16(Boss16::TIMER, 6);
            choose_spot(x);
            anim_a(x, 2);
            speed(x, 8);
            x.move_by_table_xy();
            lib::step(x);
        }
        2 => {
            let on_target = lib::target_in_box(x, 0x80, 0x100, 0x10);
            let mut arrived = on_target;
            if !arrived {
                let dx = (x.u16(Boss16::SPOT_X).wrapping_sub(x.u16(Obj::X)).wrapping_add(8)) as i16;
                let dz = (x.u16(Boss16::SPOT_Z).wrapping_sub(x.u16(Obj::Y)).wrapping_add(8)) as i16;
                arrived = dx <= 0x10 && dz <= 0x10;
            }
            if arrived {
                return x.set32(f::STATE, state::WALK);
            }
            if sub16(x, Boss16::TIMER, 1).mi() {
                let a = x.u8(Enemy::HEADING);
                let a = if x.u8(Boss16::SPIN) != 0 { a.wrapping_sub(1) } else { a.wrapping_add(1) };
                x.set8(Enemy::HEADING, a & 0x1f);
                x.set16(Boss16::TIMER, 6);
            }
            x.move_by_table_xy();
            lib::step(x);
        }
        _ => {}
    }
}

/// 0x93dd4: jump in place.
fn spot_jump<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            next7(x);
            let angle = if x.u8(f::FLIP) != 0 { 0x18 } else { 8 };
            x.set8(Enemy::HEADING, angle);
            stop(x);
            x.set16(Enemy::VZ, 0x600);
            x.set16(Enemy::GRAVITY, (-0x80i16) as u16);
            speed(x, 8);
            anim_a(x, 0);
        }
        2 => {
            lib::step(x);
            x.move_by_table_xy();
            x.move_z();
            if (x.u16(Obj::Z) as i16) >= 0 {
                return;
            }
            x.set16(Obj::Z, 0);
            x.set32(Enemy::VZ_LONG, 0);
            next7(x);
            anim_a(x, 0);
        }
        4 => x.set32(f::STATE, state::WALK),
        _ => {}
    }
}

// --- group 8: wait before an attack ---------------------------------------------------------

/// 0x93e52.
pub(super) fn before_attack<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => wait_then_turn(x),
        2 => spot_jump(x),
        4 => run_to_spot(x),
        _ => {}
    }
}

/// 0x93e64.
fn wait_then_turn<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            next7(x);
            x.set16(Boss16::TIMER, 0x14);
            x.set8(Enemy::VULNERABLE, 1);
            anim_a(x, 0);
        }
        4 => {
            x.add8(Enemy::SUB_STEP, 0xfe);
            x.set8(Enemy::VULNERABLE, 1);
            anim_a(x, 0);
        }
        2 => {
            if sub16(x, Boss16::TIMER, 1).le() {
                return x.set32(f::STATE, state::WALK);
            }
            if target_behind(x) {
                x.set32(Boss16::SAVED_STATE, 0x0208_0004);
                x.set32(f::STATE, state::TURN);
            } else {
                lib::step(x);
            }
        }
        _ => {}
    }
}

// --- group 10: pause ------------------------------------------------------------------------------

/// 0x93ece.
pub(super) fn paused<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB2) == 0 {
        if var::PLAYERS_ALIVE.get(x.b) != 0 {
            return x.set32(f::STATE, state::WALK);
        }
        x.set8(Boss16::FALLS, 1);
        if x.u8(Enemy::SUB_STEP) == 0 {
            next7(x);
            return anim_a(x, 0);
        }
        lib::step(x);
        return;
    }
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            next7(x);
            stop(x);
            anim_a(x, 6);
        }
        2 => {
            if var::PAUSE.get(x.b) == 0 {
                return x.set32(f::STATE, state::STAND);
            }
            lib::step(x);
        }
        _ => {}
    }
}

// --- group 12: seized player ---------------------------------------------------------------------------

/// 0x9402a: no prey left, the boss moves away with a leap.
fn drop_prey<R: Bench>(x: &mut Cx<'_, R>) {
    x.set32(f::STATE, state::WALK);
    x.set16(Enemy::VZ, 0x400);
    x.set16(Enemy::GRAVITY, (-0x40i16) as u16);
    let v = if x.u8(f::FLIP) == 0 { 0x300u16 } else { (-0x300i16) as u16 };
    x.set16(Enemy::VX, v);
}

/// 0x93f30: the boss holds a player (`+0x50`): bites that take 0x10 HP from them, then 1, released at 1 HP.
pub(super) fn holding<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(f::TARGET) as i16 >= 0 {
        return drop_prey(x);
    }
    let prey = x.linked(f::TARGET);
    if prey.u8(x.b, Obj::ACTIVE) == 0 || prey.u8(x.b, Player::HELD) == 0 {
        return drop_prey(x);
    }
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            next7(x);
            let v = (x.u16(Enemy::VX) as i16 >> 2) as u16;
            x.set16(Enemy::VX, v);
            x.set16(Enemy::AX, 0);
            x.set8(Boss16::TIMER, 0x20);
            let hp = prey.u16(x.b, f::HP).wrapping_sub(0x10);
            prey.set_u16(x.b, f::HP, if (hp as i16) > 0 { hp } else { 1 });
            anim_b(x, 2);
            lib::hold_seized(x);
        }
        2 => {
            if tst8(x, Boss16::TIMER).gt() {
                if sub8(x, Boss16::TIMER, 1).gt() {
                    // the fall continues
                } else {
                    next7(x);
                    x.set8(Boss16::TIMER, 0x10);
                    x.set32(Enemy::VX_LONG, 0);
                    x.set32(Enemy::VY_LONG, 0);
                }
                x.move_xz();
                if (x.u16(Obj::Z) as i16) > 0 {
                    x.set16(Obj::Z, 0);
                }
            }
            hold_bite(x, prey);
        }
        4 => hold_bite(x, prey),
        _ => {}
    }
}

/// 0x93fbc: bite animation; at its end, the player loses 1 HP (never the last one) then the boss releases them
/// when the countdown has elapsed.
fn hold_bite<R: Bench>(x: &mut Cx<'_, R>, prey: crate::objects::Obj) {
    if !lib::step(x) {
        return lib::hold_seized(x);
    }
    x.set16(Boss16::ANIM_SET, 0xffff);
    anim_b(x, 3);
    if sub8(x, Boss16::TIMER, 1).le() || prey.u16(x.b, f::HP) as i16 <= 1 {
        prey.set_u8(x.b, Player::TIMER_6F, 1);
        return lib::release_player(x, prey);
    }
    add16(x, prey, f::HP, 0xffff);
    lib::sound(x, crate::sound::CMD_341_441);
    let mut dx = 0x18u16;
    if x.u8(f::FLIP) == 0 {
        dx = dx.wrapping_neg();
    }
    let pos = (dx.wrapping_add(x.u16(Obj::X)), x.u16(Obj::Z).wrapping_add(8), x.u16(Obj::Y).wrapping_sub(2));
    (*x.c.d2(), *x.c.d3(), *x.c.d4()) = (pos.0 as u32, pos.1 as u32, pos.2 as u32);
    crate::families::effects_creators::spark_at(x.b, x.o, prey.0, 2, pos);
    lib::hold_seized(x);
}
