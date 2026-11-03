//! Boss 0, active state: stage entrances (`+5` = 0), combat (2), attacks (4), wait (6 to 10).

use crate::families::cx::Bench;
use super::*;
use crate::objects::pools;
use crate::objects::Current;
use crate::objects::Obj;

// --- aides communes -----------------------------------------------------------------------------

/// Follow-up to a hit, a run or a wait: a new start if a player is alive, otherwise the wait.
fn restart_or_idle<R: Bench>(x: &mut Cx<'_, R>) {
    if lib::players_alive(x) {
        run_start(x)
    } else {
        idle_alone(x)
    }
}

/// End of a hit animation (0x79e88): the run resumes when it is finished.
fn end_of_strike<R: Bench>(x: &mut Cx<'_, R>) {
    if lib::step(x) {
        restart_or_idle(x);
    }
}

/// Has the boss reached the position it is aiming for? `limit`: target X position (otherwise the camera offset by
/// 0xa0 to the right, 0xe0 to the left). The limit must have been reached or passed in the direction of movement.
fn reached<R: Bench>(x: &mut Cx<'_, R>, limit: Option<u16>) -> bool {
    let moving_left = x.u16(Enemy::VX) as i16 >= 0;
    let bound = match limit {
        Some(l) => l,
        None => {
            let reach = if moving_left { 0xa0 } else { 0xe0 };
            camera_x(x).wrapping_add(reach)
        }
    };
    let px = x.u16(Obj::X);
    let (d0, d1) = if moving_left { (bound, px) } else { (px, bound) };
    Fl::sub16(d1, d0).ge()
}

/// Forward run step at horizontal speed `+0x14`: animation, movement, dust at mid-step.
fn gallop_step<R: Bench>(x: &mut Cx<'_, R>) {
    x.anim_step();
    x.move_x();
    if x.u8(f::KEY_FRAME) == 2 {
        x.set8(f::KEY_FRAME, 0);
        lib::debris(x, 1);
        lib::shake_54(x, 0x10);
        lib::sound(x, crate::sound::CMD_376_476);
    }
}

/// End of the entrance run: starts the slide (speed `v` in `+0x16`, oriented according to the direction).
fn start_slide<R: Bench>(x: &mut Cx<'_, R>, v: u16, extra: bool) {
    next7(x);
    x.set16(Boss::TIMER, 0x1e);
    x.set16(Boss::TIMER2, 0);
    let v = if x.u8(f::FLIP) != 0 { v.wrapping_neg() } else { v };
    x.set16(Enemy::AX, v);
    if extra {
        lib::sound(x, crate::sound::CMD_22);
    }
    lib::sound(x, crate::sound::CMD_376_476);
    lib::sound(x, crate::sound::CMD_329_429);
    body(x, 9);
}

/// Slide (0x79750): a puff of dust every 4 frames, then the end of the slide.
fn slide<R: Bench>(x: &mut Cx<'_, R>) {
    if !sub16(x, Boss::TIMER2, 1).gt() {
        x.set16(Boss::TIMER2, 4);
        lib::debris(x, 0);
    }
    if expired(x, Boss::TIMER) {
        next7(x);
        body(x, 0xa);
    } else {
        x.move_x();
        lib::step(x);
    }
}

/// The target (`+0x50`) becomes the aimed position (`+0xbc`); D0 = charge angle.
fn mark_target<R: Bench>(x: &mut Cx<'_, R>) {
    let t = x.linked(f::TARGET);
    let tx = t.u16(x.b, Obj::X);
    x.set16(Boss::MARK_X, tx);
}

// --- group 0: stage entrances --------------------------------------------------------------------

pub(super) fn intro<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => arrival(x),
        2 => transformation(x),
        4 => partner_arrival(x),
        _ => {}
    }
}

/// 0x7964c: the main half arrives running from an edge of the screen.
fn arrival<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            next7(x);
            let cam = camera_x(x).wrapping_add(0xc0);
            x.set8(f::FLIP, 0);
            let mut angle = 0x18;
            if Fl::sub16(cam, x.u16(Obj::X)).gt() {
                x.set8(f::FLIP, 0x20);
                angle = 8;
            }
            x.set8(Obj::SPEED_INDEX, angle as u8);
            speed(x, 0x21);
            lib::sound(x, crate::sound::CMD_376_476);
            anim_kind(x, 0x30);
            entry(x, 3);
        }
        2 => {
            gallop_step(x);
            if reached(x, None) {
                start_slide(x, 0x20, true);
            }
        }
        4 => slide(x),
        6 => {
            if lib::step(x) {
                var::STAGE_CLOCK.set(x.b, 0x0459_5900);
                x.set8(Boss::INVULNERABLE, 0);
                lib::set_immunities(x);
                lib::create_set_14(x);
                run_start(x);
            }
        }
        _ => {}
    }
}

/// 0x797ae: the transformation (HP at half): the main half straightens up, creates the second one and waits until
/// it has finished its entrance.
fn transformation<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            next7(x);
            x.set16(Boss::TIMER, 0x3c);
            lib::sound(x, crate::sound::CMD_30B_40B);
            anim_kind(x, 0x31);
            body(x, 0xe);
        }
        2 => {
            if !expired(x, Boss::TIMER) {
                x.anim_step();
                return;
            }
            next7(x);
            let Some(partner) = alloc_retry(x, &pools::BOSS) else { return };
            partner.set_u8(x.b, Enemy::ACTIVE, 2);
            partner.set_u8(x.b, Enemy::KIND, 0);
            partner.set_u8(x.b, Enemy::VARIANT, 1);
            let side = x.u8(f::FLIP);
            partner.set_u8(x.b, f::FLIP, side);
            partner.set_u32(x.b, f::IMMUNITY, 0xffff_ffff);
            partner.set_u16(x.b, Boss0::PARTNER, x.o.0 as u16);
            x.set16(Boss0::PARTNER, partner.0 as u16);
            place_partner(x, partner);
        }
        4 => {
            let partner = x.linked(Boss0::PARTNER);
            if partner.u32(x.b, f::STATE) == 0x0200_0408 {
                next7(x);
                body(x, 0xf);
            } else {
                x.anim_step();
            }
        }
        6 => {
            let partner = x.linked(Boss0::PARTNER);
            if partner.u32(x.b, f::STATE) == 0x0200_040a {
                next7(x);
                x.set16(Boss::TIMER, 0x5a);
                lib::sound(x, crate::sound::CMD_309_409);
                body(x, 0x10);
            } else {
                x.anim_step();
            }
        }
        8 => {
            if expired(x, Boss::TIMER) {
                x.set8(Enemy::ACTIVE, 1);
                x.set8(Boss0::ROLE, 0);
                lib::set_immunities(x);
                var::CAMERA_MODE.set(x.b, 0);
                run_start(x);
            } else {
                x.anim_step();
            }
        }
        _ => {}
    }
}

/// 0x7ac7a: places the second half at the screen edge, on the side the main half faces: at
/// `camera + 0x1c0` (or `camera - 0x40` if it faces left), at a height depending on the camera rank.
fn place_partner<R: Bench>(x: &mut Cx<'_, R>, partner: crate::objects::Obj) {
    let side = if x.u8(f::FLIP) != 0 { (-0x40i16) as u16 } else { 0x1c0 };
    let px = camera_x(x).wrapping_add(side);
    partner.set_u16(x.b, Obj::X, px);
    let cy = camera_y(x);
    let reach = if Fl::sub16(cy.wrapping_add(0x43), x.u16(Obj::Y)).ge() { 0x60 } else { 0x28 };
    partner.set_u16(x.b, Obj::Y, cy.wrapping_add(reach));
}

/// 0x798ac: the second half enters running, slides, then waits for the end of its host's cutscene.
fn partner_arrival<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            next7(x);
            x.set16(Enemy::AX, 0);
            let angle = if x.u8(f::FLIP) != 0 { 8 } else { 0x18 };
            x.set8(Obj::SPEED_INDEX, angle as u8);
            speed(x, 0x21);
            lib::sound(x, crate::sound::CMD_376_476);
            anim_kind(x, 0x32);
            entry(x, 3);
        }
        2 => {
            gallop_step(x);
            if reached(x, None) {
                start_slide(x, 0x20, false);
            }
        }
        4 => slide(x),
        6 => {
            if lib::step(x) {
                next7(x);
                x.set16(Boss::TIMER, 0x14);
                body(x, 4);
            }
        }
        8 => {
            if expired(x, Boss::TIMER) {
                next7(x);
                x.set16(Boss::TIMER, 0x5a);
                lib::sound(x, crate::sound::CMD_309_409);
                body(x, 0xd);
            } else {
                x.anim_step();
            }
        }
        10 => {
            if expired(x, Boss::TIMER) {
                x.set8(Enemy::ACTIVE, 1);
                x.set8(Boss0::ROLE, 0);
                lib::set_immunities(x);
                lib::create_life_bar(x);
                run_start(x);
            } else {
                x.anim_step();
            }
        }
        _ => {}
    }
}

// --- groupe 2 : combat -------------------------------------------------------------------------

pub(super) fn fight<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Enemy::VARIANT) == 0 && x.u8(Boss0::TRANSFORMED) == 0 && Fl::sub16(x.u16(Enemy::HP_MAX) >> 1, x.u16(f::HP)).gt() {
        return transform(x);
    }
    match x.u8(f::SUB2) {
        0 => idle(x),
        2 => turn_around(x),
        4 => run_phase(x, 2),
        6 => run_phase(x, 3),
        8 => charge_phase(x),
        _ => {}
    }
}

/// 0x79a70: standing, waits a drawn time then sets off again.
fn idle<R: Bench>(x: &mut Cx<'_, R>) {
    if !lib::players_alive(x) {
        return idle_alone(x);
    }
    if x.u8(Enemy::SUB_STEP) == 0 {
        next7(x);
        lib::draw_6e(x);
        x.set8(Boss::ANIM_KIND, 0);
        if x.u8(Boss::ANIM_LAST) != 0 {
            x.set8(Boss::ANIM_LAST, 0);
            body(x, 4);
        }
        return;
    }
    if lib::outside_band(x, -0x30, 0x1b0) {
        return charge(x, false);
    }
    if sub8(x, Boss::IDLE_COUNT, 1).le() {
        return run_start(x);
    }
    if !lib::facing_target(x) {
        return turn(x);
    }
    lib::step(x);
}

/// 0x79ad4: about-face, then the saved state is resumed.
fn turn_around<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Enemy::SUB_STEP) == 0 {
        next7(x);
        let side = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, side);
        x.set8(Boss::ANIM_KIND, 1);
        return body(x, 5);
    }
    if !lib::step(x) {
        return;
    }
    if !lib::players_alive(x) {
        return idle_alone(x);
    }
    let saved = x.u32(Boss::SAVED_STATE);
    x.set32(f::STATE, saved);
    let last = x.u8(Boss::ANIM_LAST);
    if last == 2 || last == 3 {
        x.set8(Boss::ANIM_KIND, last);
        run_animation(x);
    } else {
        body(x, 4);
    }
}

/// 0x79bb8: forward run animation (6) or backward (7) depending on the direction of the target.
fn run_animation<R: Bench>(x: &mut Cx<'_, R>) {
    let n = if lib::moving_forward(x) { 6 } else { 7 };
    body(x, n);
}

/// 0x79c92: picks, in the run animation table, the entry for the current gesture.
fn select_run<R: Bench>(x: &mut Cx<'_, R>) {
    let set = if lib::moving_forward(x) { SET_RUN_BACK.0 } else { SET_RUN_FORWARD.0 };
    lib::switch_keeping_time(x, set);
}

/// 0x79b3a (kind 2) and 0x79b5c (kind 3): run; it starts by drawing the number of steps and their delay.
fn run_phase<R: Bench>(x: &mut Cx<'_, R>, kind: u8) {
    if !lib::players_alive(x) {
        return idle_alone(x);
    }
    if x.u8(Enemy::SUB_STEP) != 0 {
        return keep_running(x);
    }
    draw_steps(x, kind);
    walk_delay(x, kind);
    x.set8(Boss::ANIM_KIND, kind);
    next7(x);
    x.set16(Enemy::AX, 0);
    x.set16(Enemy::AY, 0);
    if !pick_target(x) {
        return idle_alone(x);
    }
    lib::approach_target(x);
    speed(x, 8);
    let last = x.u8(Boss::ANIM_LAST);
    if last == 2 || last == 3 {
        let kind = x.u8(Boss::ANIM_KIND);
        x.set8(Boss::ANIM_LAST, kind);
        select_run(x);
    } else {
        let kind = x.u8(Boss::ANIM_KIND);
        x.set8(Boss::ANIM_LAST, kind);
        run_animation(x);
    }
}

/// 0x79bf0: one run step; every `+0x71` frames the target is reconsidered and the possible attacks are
/// tried in order of range.
fn keep_running<R: Bench>(x: &mut Cx<'_, R>) {
    if sub8(x, Boss::WALK_DELAY, 1).mi() {
        if sub8(x, Boss::WALK_STEPS, 1).le() {
            return run_end(x);
        }
        lib::approach_target(x);
        speed(x, 8);
        let kind = x.u8(Boss::ANIM_KIND);
        walk_delay(x, kind);
        return select_run(x);
    }
    if lib::outside_band(x, -0x30, 0x1b0) {
        return charge(x, false);
    }
    if x.u8(Boss::ANIM_KIND) == 3 {
        if let Some(g) = close_gesture(x) {
            return from_near_gesture(x, g);
        }
    }
    if let Some(g) = far_gesture(x) {
        return from_far_gesture(x, g);
    }
    if lib::target_in_box(x, -0x10, 0x10, 8) {
        return from_front(x);
    }
    if lib::guard_in_front(x, -0x30, 0x18, 8) {
        return from_close(x);
    }
    if !lib::facing_target(x) {
        return turn(x);
    }
    if (x.u8(f::KEY_FRAME) as i8) >= 0 {
        x.move_by_table_xy();
    }
    lib::step(x);
}

/// 0x79cac: charge toward the camera or toward the saved target position.
fn charge_phase<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            next7(x);
            x.set16(Enemy::AX, 0);
            let mut angle = 0x18;
            if x.u8(Boss0::CHARGE_AIMED) != 0 {
                mark_target(x);
                if x.u8(f::FLIP) != 0 {
                    angle = 8;
                }
            } else {
                let cam = camera_x(x);
                x.set8(f::FLIP, 0);
                if !Fl::sub16(x.u16(Obj::X), cam).gt() {
                    x.set8(f::FLIP, 0x20);
                    angle = 8;
                }
            }
            x.set8(Obj::SPEED_INDEX, angle as u8);
            speed(x, 0x21);
            lib::debris(x, 1);
            lib::sound(x, crate::sound::CMD_376_476);
            anim_kind(x, 4);
            body(x, 8);
        }
        2 => {
            gallop_step(x);
            let limit = if x.u8(Boss0::CHARGE_AIMED) != 0 { Some(x.u16(Boss::MARK_X)) } else { None };
            if reached(x, limit) && x.u8(f::KEY_FRAME) != 0 {
                start_slide(x, 0x20, false);
            }
        }
        4 => slide(x),
        6 => {
            if lib::step(x) {
                if x.u8(Boss0::ROLE) == 1 {
                    return transform(x);
                }
                restart_or_idle(x);
            }
        }
        _ => {}
    }
}

// --- groupe 4 : attaques -------------------------------------------------------------------------

pub(super) fn attacks<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => simple_strike(x, 0x10, 0),
        2 => simple_strike(x, 0x11, 1),
        4 => simple_strike(x, 0x12, 2),
        6 => leap_strike(x),
        8 => arc_jump(x),
        10 => triple_strike(x),
        12 => spit(x),
        _ => {}
    }
}

/// 0x79e5a, 0x79ea0 and 0x79ece: one hit of an animation (`n` of the attacks set).
fn simple_strike<R: Bench>(x: &mut Cx<'_, R>, kind: u8, n: u32) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            next7(x);
            anim_kind(x, kind);
            entry(x, n);
        }
        2 => end_of_strike(x),
        _ => {}
    }
}

/// 0x79efc: leap onto the target followed by a hit.
fn leap_strike<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            next7(x);
            x.set16(Enemy::AX, 0);
            mark_target(x);
            let angle = if x.u8(f::FLIP) != 0 { 8 } else { 0x18 };
            x.set8(Obj::SPEED_INDEX, angle as u8);
            speed(x, 0x2b);
            lib::debris(x, 1);
            lib::sound(x, crate::sound::CMD_376_476);
            anim_kind(x, 0x13);
            entry(x, 3);
        }
        2 => {
            gallop_step(x);
            let mark = x.u16(Boss::MARK_X);
            if reached(x, Some(mark)) && x.u8(f::KEY_FRAME) != 0 {
                x.set8(Boss::INVULNERABLE, 0);
                start_slide(x, 0x30, false);
            }
        }
        4 => slide(x),
        6 => {
            if lib::step(x) {
                restart_or_idle(x);
            }
        }
        _ => {}
    }
}

/// 0x7a034: big arcing jump toward the target, landing and roar.
fn arc_jump<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            next7(x);
            x.set16(Enemy::AX, 0);
            let angle = if x.u8(f::FLIP) != 0 { 8 } else { 0x18 };
            x.set8(Obj::SPEED_INDEX, angle as u8);
            speed(x, 0x17);
            let t = x.linked(f::TARGET);
            let mut gap = x.u32(Obj::X).wrapping_sub(t.u32(x.b, Obj::X)) as i32;
            if gap < 0 {
                gap = gap.wrapping_neg();
            }
            let per_frame = ((x.u16(Enemy::VX) as i16 as i32) << 8).wrapping_abs();
            let mut lift: u16 = 0x400;
            // the vertical speed grows by 10 per flight duration (in frames) of the horizontal gap
            if per_frame != 0 {
                loop {
                    let f = Fl::sub32(gap as u32, per_frame as u32);
                    gap = gap.wrapping_sub(per_frame);
                    if f.le() {
                        break;
                    }
                    lift = lift.wrapping_add(10);
                }
            }
            x.set16(Enemy::VZ, lift);
            x.set16(Enemy::GRAVITY, (-0x28i16) as u16);
            anim_kind(x, 0x14);
            entry(x, 4);
        }
        2 => {
            lib::step(x);
            if x.u8(f::KEY_FRAME) != 0 {
                return;
            }
            x.move_xz();
            if Fl::sub16(x.u16(Enemy::VZ), 0x100).le() {
                next7(x);
                entry(x, 5);
            }
        }
        4 => {
            x.move_xz();
            if Fl::sub16(x.u16(Enemy::VZ), (-0x100i16) as u16).le() {
                next7(x);
                x.set16(Enemy::GRAVITY, (-0x40i16) as u16);
                entry(x, 6);
            }
        }
        6 => {
            x.move_xz();
            if (x.u16(Obj::Z) as i16) <= 0 {
                next7(x);
                x.set16(Obj::Z, 0);
                x.set16(Boss::TIMER, 0x14);
                lib::shake_58(x, 0x14);
                lib::debris(x, 2);
                lib::sound(x, crate::sound::CMD_327_427_C);
                entry(x, 7);
            }
        }
        8 => {
            if expired(x, Boss::TIMER) {
                restart_or_idle(x);
            }
        }
        _ => {}
    }
}

/// 0x7a166: three chained hits (number drawn from table 0x7a1ca according to difficulty and number of players).
fn triple_strike<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            next7(x);
            anim_kind(x, 0x15);
            entry(x, 8);
        }
        2 => {
            lib::step(x);
            if x.u8(f::KEY_FRAME) == 0 {
                return;
            }
            next7(x);
            x.set8(Boss::INVULNERABLE, 0);
            let level = (var::RANK.get(x.b) & 0x1e) * 2;
            let mut players = var::PLAYERS_ALIVE.get(x.b);
            players = players.saturating_sub(1);
            let i = level.wrapping_add(players as u16);
            let n = byte(x, "boss_0/combo_counts", i as u32);
            x.set8(Boss0::COMBO, n);
        }
        4 => {
            lib::step(x);
            if x.u8(f::KEY_FRAME) == 0 {
                return;
            }
            if sub8(x, Boss0::COMBO, 1).le() {
                next7(x);
                entry(x, 9);
            }
        }
        6 => {
            if lib::step(x) {
                if lib::players_alive(x) {
                    stand(x);
                } else {
                    idle_alone(x);
                }
            }
        }
        _ => {}
    }
}

/// 0x7a248: spits a projectile (ESHL 0x43) in front of it.
fn spit<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            next7(x);
            anim_kind(x, 0x16);
            entry(x, 0xa);
        }
        2 => {
            if lib::step(x) {
                next7(x);
                x.set16(Boss::TIMER, 0x14);
                x.set8(Enemy::VULNERABLE, 1);
                return body(x, 4);
            }
            if x.u8(f::KEY_FRAME) == 0 {
                return;
            }
            x.set8(f::KEY_FRAME, 0);
            let Some(shot) = x.alloc(&pools::ESHL) else { return };
            shot.set_u8(x.b, Enemy::ACTIVE, 1);
            shot.set_u8(x.b, Enemy::KIND, 0x43);
            for off in [0x8, 0xc, 0x10] {
                let v = x.u16(off);
                shot.set_u16(x.b, off, v);
            }
            let side = x.u8(f::FLIP);
            shot.set_u8(x.b, f::FLIP, side);
            let owner = x.u16(f::OWNER);
            shot.set_u16(x.b, f::OWNER, owner);
            add16(x, shot, 0xc, 0x30);
            let dx = if side != 0 { 0x60u16 } else { (-0x60i16) as u16 };
            add16(x, shot, 0x8, dx);
        }
        4 => {
            if expired(x, Boss::TIMER) {
                restart_or_idle(x);
            } else {
                lib::step(x);
            }
        }
        _ => {}
    }
}

// --- groups 6 to 10: wait --------------------------------------------------------------------

/// 0x7a30c: while no player is alive, the boss waits in its pose; as soon as a player comes back, it sets off again.
pub(super) fn wait_for_players<R: Bench>(x: &mut Cx<'_, R>) {
    if (var::PLAYERS_ALIVE.get(x.b) as i8) > 0 {
        return run_start(x);
    }
    match x.u8(f::SUB2) {
        0 => {
            next6(x);
            x.set8(Boss::ANIM_LAST, 0xff);
            body(x, 0x11);
        }
        2 => {
            lib::step(x);
        }
        _ => {}
    }
}
