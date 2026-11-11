//! What the type 2 boss (0x7c5a8) has of its own: creation (it spawns type 22 and, later, the ending stage), arrival
//! by a big leap, and two attacks (crushing leap `+6` = 10, spin and roar `+6` = 12).

use crate::actors::families::cx::Bench;
use super::moves::step_anim;
use super::*;
use crate::world::objects::pools;
use crate::world::objects::Current;

/// 0x7c5b8: when the `+0xbc` countdown has elapsed, creates the SET 2 stage controller (variant 0x3e), once.
pub(super) fn spawn_scene_controller<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Boss22::CONTROLLER) != 0 || !sub16(x, Boss22::SPOT_TIMER, 1).mi() {
        return;
    }
    let Some(set) = x.alloc(&pools::SET) else { return };
    set.set_u8(x.b, Obj::ACTIVE, 1);
    set.set_u8(x.b, Obj::VARIANT, 0x3e);
    set.set_u8(x.b, Obj::KIND, 2);
    x.set8(Boss22::CONTROLLER, 1);
}

/// 0x7c600: creation. Depending on the number of players and the difficulty, a type 22 boss is linked to it (`+0xac`).
pub(super) fn init<R: Bench>(x: &mut Cx<'_, R>) {
    var::BOSS_FIGHT.set(x.b, 1);
    var::STAGE_CLOCK.set(x.b, 0x0459_5900);
    lib::clear_popup(x);
    lib::empty_popups(x);
    lib::clear_all_popups(x);
    lib::sound(x, crate::hw::sound::CMD_20);
    lib::create_set_15(x);
    let r = x.random() as u32;
    x.set8(Boss22::EFFECT_COUNT, r as u8);
    x.set8(f::BOX_SET, 0x22);
    lib::initial_hp_boss(x);
    lib::create_set_14(x);
    for (off, v) in [(Enemy::AI_STANCE_ROW, 0), (Enemy::EVENT_KIND, 0x23), (Enemy::EVENT_LOW, 0), (Enemy::AI_ROW_8E, 3), (Enemy::AI_SHOT, 0xff), (Enemy::AI_68, 8), (Enemy::AI_69, 8), (Enemy::AI_WANDER_ROW, 3), (Enemy::AI_WAIT_MASKS, 1), (Enemy::AI_WAIT_ROW, 8)] {
        x.set8(off, v);
    }
    let speed = if x.u16(f::TARGET) != 0 && var::PLAYERS_ALIVE.get(x.b) != 1 { 7 } else { 6 };
    x.set8(Enemy::AI_DECISION_A, speed);
    x.set8(Enemy::AI_DECISION_B, speed);
    for (off, v) in [(Enemy::AI_BOX_CHANCE, 5), (Enemy::AI_BOX_ANIM, 0x3c), (Enemy::AI_BYTE_ROW, 6), (Enemy::LOOT, 0x12), (Enemy::LOOT_ROW, 0x17)] {
        x.set8(off, v);
    }
    x.set16(Boss22::GUARD, 0xb4);
    x.set16(Boss22::GUARD_TIMER, 0xf0);
    var::SCENE_LEVEL.set(x.b, 0);
    x.set32(f::IMMUNITY, 0xffff_ffff);
    pick_target(x);
    lib::draw_stance(x);
    lib::draw_6b(x);
    x.set8(Boss22::RAGE, 0xff);
    x.set8(Boss22::SECOND_FX, 0);
    x.set16(Boss22::SPOT_TIMER, 0x12c);
    x.set8(Boss::DEATH_CREDIT, 0);
    x.set8(Boss22::PARTNERED, 0);
    let players = var::PLAYERS_ALIVE.get(x.b);
    let alone = players == 1 || (players == 2 && (var::RANK.get(x.b) as i16) < 0x17);
    if !alone {
        let Some(part) = x.alloc(&pools::BOSS) else { return };
        part.set_u8(x.b, Obj::ACTIVE, 1);
        part.set_u8(x.b, Obj::KIND, 0x16);
        x.set16(Boss22::HOST, part.0 as u16);
        part.set_u16(x.b, Boss22::HOST, x.o.0 as u16);
        x.set8(Boss22::PARTNERED, 1);
    }
    if x.u8(Boss22::PARTNERED) == 0 {
        x.set8(Boss::DEATH_CREDIT, 1);
        x.set8(Boss::DEATH_VALUE, 0xff);
    }
    x.add8(f::STATE, 2);
    // `jmp 0x1a32`: the registers it leaves to the following handler are re-read by no one (`exit` probe)
    x.anim_start(TWO_KIT.set_a.0, 0);
}

// --- group 0: arrival -----------------------------------------------------------------------------

/// 0x7c814: the boss arrives by a big leap from the right, lands in a puff of dust and goes into combat.
pub(super) fn arrive<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            next7(x);
            x.set8(Enemy::VULNERABLE, 0);
            let px = x.u16(Obj::X).wrapping_add(0x1b0).wrapping_add(var::SCROLL_X.get(x.b));
            let pz = x.u16(Obj::Y).wrapping_add(0x80).wrapping_add(var::SCROLL_Y.get(x.b));
            x.set16(Obj::X, px);
            x.set16(Obj::Y, pz);
            x.set8(f::FLIP, 0);
            x.set16(Enemy::VX, (-0x700i16) as u16);
            x.set16(Enemy::VZ, 0x600);
            x.set16(Enemy::GRAVITY, (-0x50i16) as u16);
            x.set16(Enemy::AX, 0);
            x.set8(Enemy::STRIKE_KIND, 0x2a);
            x.set8(Boss22::ANIM_ID, 7);
            x.set8(Boss22::ANIM_LAST, 7);
            start(x, Set::A, 0x1c);
        }
        2 => rise_wait(x),
        4 => climb(x, true),
        6 => climb(x, false),
        8 => {
            x.move_xz();
            if (x.u32(Obj::Z) as i32) > 0 {
                return step_anim(x);
            }
            lib::shake_5c(x, 0x1e);
            lib::set_immunities(x);
            x.set32(Obj::Z, 0);
            dust_line(x);
            next7(x);
            x.set16(Obj::ANIM_TIMER, 1);
            x.set8(Enemy::VULNERABLE, 0);
            start(x, Set::A, 6);
        }
        10 => {
            if x.anim_step() {
                x.set8(f::SUB, 2);
                x.set8(Enemy::SUB_STEP, 0);
                x.set8(Enemy::VULNERABLE, 1);
            }
        }
        _ => {}
    }
}

/// 0x7d0bc: waits for the key frame of the jump animation.
pub(super) fn rise_wait<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::KEY_FRAME) != 0 {
        next7(x);
        x.set8(Enemy::VULNERABLE, 0);
    }
    step_anim(x);
}

/// 0x7d0d2 (rise) and 0x7d0f2 (descent): the vertical speed crosses ±0x100.
pub(super) fn climb<R: Bench>(x: &mut Cx<'_, R>, up: bool) {
    x.move_xz();
    let vz = x.u16(Enemy::VZ);
    let reached = if up { Fl::sub16(vz, 0x100).gt() } else { Fl::sub16(vz, 0xff00).gt() };
    if !reached {
        next7(x);
        x.set16(Obj::ANIM_TIMER, 1);
    }
    step_anim(x);
}

/// 0x7d112: landing from the jump (sound, dust in a line, vulnerable).
pub(super) fn land<R: Bench>(x: &mut Cx<'_, R>) {
    x.move_xz();
    if (x.u32(Obj::Z) as i32) > 0 {
        return step_anim(x);
    }
    lib::shake_54(x, 0x1e);
    lib::sound(x, crate::hw::sound::CMD_372);
    x.set32(Obj::Z, 0);
    dust_line(x);
    next7(x);
    x.set16(Obj::ANIM_TIMER, 1);
    x.set8(Enemy::VULNERABLE, 1);
    start(x, Set::A, 6);
}

// --- attack 10: crushing leap ---------------------------------------------------------------------

/// 0x7f10a: leap speeds according to the distance to the camera (four 8-byte rows at 0x7f14c: X, height,
/// gravity, depth).
fn jump_speeds<R: Bench>(x: &mut Cx<'_, R>) {
    let d = x.u16(Obj::X).wrapping_sub(var::SCROLL_X.get(x.b));
    let row = if Fl::sub16(d, 0x60).le() {
        0
    } else if Fl::sub16(d, 0xc0).le() {
        8
    } else if Fl::sub16(d, 0x120).le() {
        0x10
    } else {
        0x18
    };
    for (k, off) in [Enemy::VX, Enemy::VZ, Enemy::GRAVITY, Enemy::AX].into_iter().enumerate() {
        let v = word(x, "boss_2/leap_speeds", row + 2 * k as u32);
        x.set16(off, v);
    }
}

/// 0x7d05c: crushing leap. Steps `+7`: 0 jump, 2 to 8 leap (as on arrival), 10 landing and hit, 12 about-face,
/// 14 double hit, 16 run, 18 run to a point, 20 turn, 22 to 30 end.
pub(super) fn slam<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            jump_speeds(x);
            next7(x);
            x.set8(Enemy::VULNERABLE, 1);
            x.set8(Enemy::STRIKE_KIND, 0x2a);
            x.set8(Enemy::STRIKE_POWER, 5);
            x.set8(Boss22::ANIM_ID, 0xb);
            x.set8(Boss22::ANIM_LAST, 0xb);
            start(x, Set::A, 5);
        }
        2 => rise_wait(x),
        4 => climb(x, true),
        6 => climb(x, false),
        8 => land(x),
        10 => {
            if !x.anim_step() {
                return;
            }
            x.set8(Enemy::ACTIVE, 1);
            if needs_turn(x) {
                next7(x);
                x.set8(Enemy::VULNERABLE, 1);
                sounds(x, 0x27, 5);
                start(x, Set::A, 1);
            } else {
                x.add8(Enemy::SUB_STEP, 4);
                start(x, Set::B, 1);
            }
        }
        12 => {
            if x.anim_step() {
                next7(x);
                let side = x.u8(f::FLIP) ^ 0x20;
                x.set8(f::FLIP, side);
                start(x, Set::B, 1);
            }
        }
        14 => {
            if x.u8(f::KEY_FRAME) == 1 {
                var::CAMERA_MODE.set(x.b, 4);
                x.set8(f::KEY_FRAME, 0);
                eshl_sweep_1d(x, 2);
            }
            if x.u8(f::KEY_FRAME) == 2 {
                x.set8(f::KEY_FRAME, 0);
                eshl_sweep_1d(x, 0);
            }
            if !x.anim_step() {
                return;
            }
            next7(x);
            x.set16(Boss22::TIMER2, 0x12c);
            x.set8(Enemy::HEADING, 0);
            let r = x.random() as u32;
            if r & 1 == 0 {
                x.set8(Enemy::HEADING, 0x10);
            }
            start(x, Set::A, 8);
            speed(x, 0x1f);
        }
        16 => {
            rubble(x, 0, 0x201, 0x20, 0x20);
            if sub16(x, Boss22::TIMER2, 1).mi() {
                return next7(x);
            }
            if player_within_reach(x) {
                return x.set8(Enemy::SUB_STEP, 0x1a);
            }
            if lib::target_in_box(x, 0, 0x180, 0x10) {
                return next7(x);
            }
            if x.u8(Enemy::BLOCKED) != 0 {
                let a = x.u8(Enemy::HEADING);
                x.set8(Enemy::HEADING, if a != 0 { 0 } else { 0x10 });
            }
            x.move_by_table_xy();
            step_anim(x);
        }
        18 => {
            if player_within_reach(x) {
                return x.set8(Enemy::SUB_STEP, 0x1a);
            }
            next7(x);
            x.set16(Boss22::TIMER2, 0x12c);
            x.set16(Boss22::TIMER, 6);
            choose_spot(x);
            start(x, Set::A, 0xa);
            speed(x, 0x1f);
            x.move_by_table_xy();
            step_anim(x);
        }
        20 => {
            rubble(x, 0, 0x201, 0x20, 0x20);
            if sub16(x, Boss22::TIMER2, 1).mi() || player_within_reach(x) {
                return x.set8(Enemy::SUB_STEP, 0x1a);
            }
            if lib::target_in_box(x, 0xc0, 0xc0, 0x10) || lib::target_in_box(x, -0xc0, 0xc0, 0x10) {
                return x.set8(Enemy::SUB_STEP, 0x1a);
            }
            let dx = x.u16(Boss22::SPOT_X).wrapping_sub(x.u16(Obj::X)).wrapping_add(8);
            let dz = x.u16(Boss22::SPOT_Z).wrapping_sub(x.u16(Obj::Y)).wrapping_add(8);
            if dx <= 0x10 && dz <= 0x10 {
                return x.set8(Enemy::SUB_STEP, 0x1a);
            }
            if sub16(x, Boss22::TIMER, 1).mi() {
                let a = x.u8(Enemy::HEADING);
                let a = if x.u8(Boss22::SPIN) != 0 { a.wrapping_sub(1) } else { a.wrapping_add(1) };
                x.set8(Enemy::HEADING, a & 0x1f);
                x.set16(Boss22::TIMER, 6);
            }
            if needs_turn(x) {
                return next7(x);
            }
            x.move_by_table_xy();
            step_anim(x);
        }
        22 => {
            next7(x);
            start(x, Set::A, 0xb);
        }
        24 => {
            rubble(x, 0, 0x201, 0x20, 0x20);
            if sub16(x, Boss22::TIMER, 1).mi() {
                let a = x.u8(Enemy::HEADING);
                let a = if x.u8(Boss22::SPIN) != 0 { a.wrapping_sub(1) } else { a.wrapping_add(1) };
                x.set8(Enemy::HEADING, a & 0x1f);
                x.set16(Boss22::TIMER, 6);
            }
            if x.anim_step() {
                x.add8(Enemy::SUB_STEP, 0xfc);
                let side = x.u8(f::FLIP) ^ 0x20;
                x.set8(f::FLIP, side);
                return start(x, Set::A, 0x15);
            }
            x.move_by_table_xy();
        }
        26 => {
            next7(x);
            x.set8(Boss22::ARM_TIMER, 0x1e);
            start(x, Set::B, 2);
        }
        28 => {
            if x.u8(f::KEY_FRAME) != 0 {
                next7(x);
                x.set8(Boss22::BUSY, 1);
            }
            step_anim(x);
        }
        30 => {
            if x.u8(Boss22::BUSY) == 0 {
                var::CAMERA_MODE.set(x.b, 0);
                return next_attack(x);
            }
            step_anim(x);
        }
        _ => {}
    }
}

// --- attack 12: spin and roar ------------------------------------------------------------------

/// Aimed point on the screen edge, on the side the boss faces (`+0xb0`, `+0xb2`).
fn edge_spot<R: Bench>(x: &mut Cx<'_, R>) {
    let off = if x.u8(f::FLIP) != 0 { 0x160 } else { 0x20 };
    let sx = var::SCROLL_X.get(x.b).wrapping_add(off);
    x.set16(Boss22::SPOT_X, sx);
    let sz = var::SCROLL_Y.get(x.b).wrapping_add(0x60);
    x.set16(Boss22::SPOT_Z, sz);
}

/// 0x7d4a0: the boss spins around a point on the screen edge, turns around, roars (effects, cutscene) then resumes.
pub(super) fn spin_roar<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            next7(x);
            x.set8(Enemy::VULNERABLE, 1);
            x.set8(Boss22::ANIM_ID, 3);
            sounds(x, 0x27, 5);
            x.set16(Boss22::TIMER, 0);
            edge_spot(x);
            x.set8(Boss22::SWAY, 4);
            if x.u8(Boss22::ANIM_ID) != x.u8(Boss22::ANIM_LAST) {
                x.set8(Boss22::ANIM_LAST, 3);
                start(x, Set::A, 4);
            }
            speed(x, 0x26);
        }
        2 => {
            rubble(x, 0, 0x201, 0x20, 0x20);
            edge_spot(x);
            if sub16(x, Boss22::TIMER, 1).mi() {
                let r = rand_and(x, 7);
                let t = word(x, "boss_2/run_times", 2 * r);
                x.set16(Boss22::TIMER, t);
                let b6 = x.u8(Boss22::SWAY).wrapping_neg();
                x.set8(Boss22::SWAY, b6);
                let (sx, sz) = (x.u16(Boss22::SPOT_X), x.u16(Boss22::SPOT_Z));
                *x.c.d2() = *x.c.d2() & 0xffff_0000 | sz as u32;
                let angle = lib::aim_from(x, (sx, sz)).wrapping_add(b6);
                x.set8(Enemy::HEADING, angle);
            }
            let cam = var::SCROLL_X.get(x.b);
            if x.u8(f::FLIP) != 0 {
                if (cam.wrapping_add(0x130).wrapping_sub(x.u16(Obj::X)) as i16) < 0 {
                    return next7(x);
                }
            } else if (cam.wrapping_add(0x50).wrapping_sub(x.u16(Obj::X)) as i16) >= 0 {
                return next7(x);
            }
            x.move_by_table_xy();
            step_anim(x);
        }
        4 => {
            next7(x);
            start(x, Set::A, 1);
        }
        6 => {
            if x.anim_step() {
                next7(x);
                let side = x.u8(f::FLIP) ^ 0x20;
                x.set8(f::FLIP, side);
                start(x, Set::B, 5);
            }
        }
        8 => {
            next7(x);
            var::CAMERA_MODE.set(x.b, 4);
            x.set8(Boss22::ARM_TIMER, 0x32);
            var::INPUT_MASK.set(x.b, 0x7f);
            x.set8(Enemy::VULNERABLE, 1);
            x.set8(Boss22::ANIM_ID, 0x10);
            x.set8(Boss22::ANIM_LAST, 0x10);
            start(x, Set::B, 5);
        }
        10 => {
            if x.u8(f::KEY_FRAME) == 1 {
                x.set8(f::KEY_FRAME, 0);
                eshl_sweep_1d(x, 2);
            }
            if x.u8(f::KEY_FRAME) == 2 {
                x.set8(f::KEY_FRAME, 0);
                eshl_wave_1e(x);
                crate::services::palette::set_flash_color(x.b, crate::actors::families::small::LIGHTNING_YELLOW);
                x.set8(Boss22::SHAKE_TIME, 4);
            }
            var::SCROLL_Y_PLANE1.set(x.b, 0);
            if x.u8(Boss22::SHAKE_TIME) != 0 {
                var::SCROLL_Y_PLANE1.set(x.b, 0x100);
                sub8(x, Boss22::SHAKE_TIME, 1);
            }
            if x.anim_step() {
                next7(x);
                x.set8(Boss22::FLASH_MARK, 0);
                x.set8(Boss22::BUSY, 1);
                start(x, Set::B, 6);
            }
        }
        12 => {
            if x.u8(Boss22::BUSY) == 0 {
                x.set8(Enemy::VULNERABLE, 0);
                var::INPUT_MASK.set(x.b, 0xff);
                var::CAMERA_MODE.set(x.b, 0);
                return next_attack(x);
            }
            step_anim(x);
        }
        _ => {}
    }
}
