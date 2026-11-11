//! Type 19, parts of the beast (substate `+5 = 0`) and creation of the variants.
//!
//! Variants 0 to 3 are the beast and its attached limbs; variants 4 to 13 are released parts (projectiles, leaps,
//! shadow): at creation each receives a `+6` that chooses its behaviour in `run`.

use crate::actors::families::cx::Bench;
use crate::actors::families::eshl_creators;
use super::*;
use crate::world::objects::Current;

/// Creation according to variant `+3` (0x71f86).
pub(super) fn spawn<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::VARIANT) {
        0 => {
            x.set8(Enemy::ACTIVE, 2);
            x.set32(f::IMMUNITY, 0xffff_ffff);
            x.set8(f::SUB, 0);
            x.set8(Enemy::COUNTER, 0);
            set_hp(x);
            x.set16(f::SUB2, 0);
            follow_offset(x);
            play(x, 0x14);
        }
        1 => {
            x.set8(f::SUB, 0);
            x.set8(f::SUB2, 2);
            set_hp(x);
            follow_body(x);
            x.set32(f::IMMUNITY, 0x0021_8000);
            let mut n = 0x14;
            if x.u8(DisplacerBeast::PART) == 6 {
                x.set32(f::IMMUNITY, 0xffff_ffff);
                x.set8(Enemy::ACTIVE, 2);
                x.set8(DisplacerBeast::SHOTS, 7);
                n = 0x4b;
            }
            play(x, n);
        }
        2 => {
            let Some(p) = parent(x) else { return orphan(x) };
            x.set8(f::SUB, 0);
            x.set8(f::SUB2, 4);
            x.set8(Enemy::SUB_STEP, 0);
            set_hp(x);
            x.set32(f::IMMUNITY, 0xffff_ffff);
            let rank = (x.u8(DisplacerBeast::PART) as u16) << 3;
            let flip = p.u8(x.b, f::FLIP) != 0;
            let (cam_x, cam_y) = camera(x);
            let px = if flip { 0xfff0u16.wrapping_sub(rank) } else { 0x190 + rank };
            x.set16(Obj::X, px.wrapping_add(cam_x));
            x.set16(Obj::Y, (0x20u16 + rank).wrapping_add(cam_y));
            play(x, 0x14);
        }
        3 => {
            set_state_long(x, ACTIVE_STANCE);
            x.set8(DisplacerBeast::WANT_ANIM, 0xc);
            x.set8(DisplacerBeast::CUR_ANIM, 0xc);
            x.set8(Enemy::VULNERABLE, 1);
            let Some(p) = parent(x) else { return orphan(x) };
            let flip = p.u8(x.b, f::FLIP);
            x.set8(f::FLIP, flip);
            let (cam_x, cam_y) = camera(x);
            let px = if flip != 0 { 0xfff0u16 } else { 0x190 };
            x.set16(Obj::X, px.wrapping_add(cam_x));
            let py = if x.u8(DisplacerBeast::PART) == 7 { 0x30 } else { 0x60 };
            x.set16(Obj::Y, cam_y.wrapping_add(py));
            play(x, 8);
        }
        // released parts: "active" state, step chosen by `+6`
        4 => lone(x, 0x0202_0000, 8),
        5 => lone(x, 0x0200_0800, 1),
        6 => lone(x, 0x0200_0600, 0xe),
        7 => lone(x, 0x0200_1200, 0xd),
        8 => detached(x, 0x0200_0a00, false),
        9 => detached(x, 0x0200_0c00, true),
        10 => detached(x, 0x0200_0e00, true),
        11 => detached(x, 0x0200_1000, false),
        12 => detached(x, 0x0200_1400, false),
        13 => detached(x, 0x0200_1600, false),
        _ => {}
    }
}

/// Hit points of the parts (1): they die on the first hit.
fn set_hp<R: Bench>(x: &mut Cx<'_, R>) {
    let e = Enemy(x.o);
    e.set_hp_max(x.b, 1);
    e.set_hp(x.b, 1);
    e.set_hp_last(x.b, 1);
}

/// Released part that immediately plays an animation (variants 4 to 7).
fn lone<R: Bench>(x: &mut Cx<'_, R>, state: u32, anim: u8) {
    set_state_long(x, state);
    x.set8(DisplacerBeast::PART, 0xff);
    x.set16(Obj::Z, 0);
    play(x, anim);
}

/// Released part without a starting animation (variants 8 to 13); `active`: it is active.
fn detached<R: Bench>(x: &mut Cx<'_, R>, state: u32, active: bool) {
    set_state_long(x, state);
    x.set8(DisplacerBeast::PART, 0xff);
    if active {
        x.set8(Enemy::ACTIVE, 2);
    }
}

// --- substate 0 --------------------------------------------------------------------------------

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => attached(x),
        2 => arm(x),
        4 => reach(x),
        6 => spit(x),
        8 => spit_once(x),
        10 => bound(x),
        12 => bound_x(x),
        14 => bound_back(x),
        16 => drop_in(x),
        18 => slide(x),
        20 => sneak(x),
        22 => sneak_b(x),
        _ => {}
    }
}

/// +6 = 0 (0x7245e): attached limb; it detaches when the beast is far from the camera.
fn attached<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Enemy::SUB_STEP) == 0 {
        let mut cam = var::SCROLL_X.get(x.b);
        if var::CAMERA_SELECT.get(x.b) != 0 {
            cam = var::CAMERA_ALT_X.get(x.b);
        }
        let d = x.u16(Obj::X).wrapping_sub(cam) as i16;
        if d <= 0x140 {
            x.add8(Enemy::SUB_STEP, 2);
            let p = x.linked(Enemy::CREATOR);
            if p.u8(x.b, Obj::VARIANT) != 1 {
                x.set8(Enemy::ACTIVE, 1);
                x.set32(f::IMMUNITY, 0);
            }
        }
    }
    if x.u8(Enemy::COUNTER) != 0 {
        x.add8(f::SUB, 2);
        x.set16(f::SUB2, 0);
        x.set8(DisplacerBeast::WANT_ANIM, 0xff);
        x.set8(Enemy::ACTIVE, 1);
        x.set16(f::IMMUNITY, 0);
        return;
    }
    follow_offset(x);
    step(x);
}

/// +6 = 2 (0x724cc): arm of the beast: follows the parent, plays the animation of its state and strikes a nearby
/// player.
fn arm<R: Bench>(x: &mut Cx<'_, R>) {
    follow_body(x);
    if x.u8(DisplacerBeast::DONE) == 0 {
        let mut n = 0x4b;
        if x.u8(DisplacerBeast::PART) != 6 {
            let Some(p) = parent(x) else { return orphan(x) };
            let idx = p.u8(x.b, DisplacerBeast::ARM_STATE) as u32;
            n = tables::table("dbeast/arm_anims").at8(x.b, idx);
            if (p.u32(x.b, f::STATE) as i32) <= 0x0200_0600 {
                n = 0;
            }
        }
        play_once(x, n);
    }
    if x.u8(DisplacerBeast::PART) == 6 {
        let t = x.u8(DisplacerBeast::TIMER).wrapping_sub(1);
        x.set8(DisplacerBeast::TIMER, t);
        if (t as i8) < 0 {
            x.set8(DisplacerBeast::TIMER, 0);
            if player_near(x) {
                x.set8(DisplacerBeast::DONE, 1);
                play_once(x, 0x4e);
                if (var::PLAYERS_ALIVE.get(x.b) as i8) > 1 && x.u8(f::KEY_FRAME) != 0 {
                    x.set8(f::KEY_FRAME, 0);
                    let variant = x.u8(DisplacerBeast::SHOTS);
                    eshl_thrown_by_beast(x, variant);
                    let s = x.u8(DisplacerBeast::SHOTS).wrapping_sub(1);
                    x.set8(DisplacerBeast::SHOTS, s);
                    if (s as i8) < 0 {
                        x.set8(DisplacerBeast::TIMER, 0x28);
                        arm_rest(x);
                    }
                }
            } else {
                arm_rest(x);
            }
        }
    }
    step(x);
    let p = x.linked(Enemy::CREATOR);
    if (p.u32(x.b, f::STATE) as i32) >= 0x0402_0c00 {
        x.set8(DisplacerBeast::LAYER, 3);
    }
}

/// 0x725a0: the arm rests.
fn arm_rest<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(DisplacerBeast::DONE, 0);
    x.set8(DisplacerBeast::SHOTS, 7);
}

/// Heads for the part's rally point (0x721a2); true when it is there.
/// Original glitch: without a parent, the part becomes an orphan (jump to 0x725c8) and the caller tests the Z flag left
/// by the animation start (0x1a32: `move.l A4,(0x20,A0)`, non-null pointer): "arrived".
fn rally<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let Some(p) = parent(x) else {
        orphan(x);
        return true;
    };
    let flip = p.u8(x.b, f::FLIP);
    x.set8(f::FLIP, flip);
    let row = (x.u8(DisplacerBeast::PART).wrapping_mul(4)) as u32;
    let mut dx = tables::table("dbeast/rally_offsets").at16(x.b, row);
    let dy = tables::table("dbeast/rally_offsets").at16(x.b, row + 2);
    if flip != 0 {
        dx = dx.wrapping_neg();
    }
    let ax = dx.wrapping_add(p.u16(x.b, Obj::X));
    let ay = dy.wrapping_add(p.u16(x.b, Obj::Y));
    x.set16(DisplacerBeast::AIM_X, ax);
    x.set16(DisplacerBeast::AIM_Y, ay);
    // D2 (low word) = aimed Y, left as a leftover (the original leaves the computation of 0x137e there, which
    // `aim_from` does not return).
    *x.c.d2() = *x.c.d2() & 0xffff_0000 | ay as u32;
    let angle = lib::aim_from(x, (ax, ay));
    x.set8(Obj::SPEED_INDEX, angle);
    move_xy(x);
    let near = |a: u16, b: u16| a.wrapping_sub(b).wrapping_add(8) <= 0x10;
    near(x.u16(DisplacerBeast::AIM_X), x.u16(Obj::X)) && near(x.u16(DisplacerBeast::AIM_Y), x.u16(Obj::Y))
}

/// +6 = 4 (0x725e2): the part gets up, waits, then rejoins its parent.
fn reach<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            x.add8(Enemy::SUB_STEP, 2);
            x.set8(Enemy::ACTIVE, 2);
            x.set8(DisplacerBeast::TIMER, 0x28);
            play(x, 0xd);
            speed(x, 0x17);
            step(x);
        }
        2 => {
            let t = x.u8(DisplacerBeast::TIMER).wrapping_sub(1);
            x.set8(DisplacerBeast::TIMER, t);
            if (t as i8) < 0 {
                x.add8(Enemy::SUB_STEP, 2);
            }
        }
        4 => {
            if rally(x) {
                let Some(p) = parent(x) else { return orphan(x) };
                let n = p.u8(x.b, DisplacerBeast::RALLYING).wrapping_sub(1);
                p.set_u8(x.b, DisplacerBeast::RALLYING, n);
                x.set32(f::IMMUNITY, 0x0021_8000);
                if n == 0xff {
                    p.set_u8(x.b, DisplacerBeast::RALLYING, 0);
                }
                if (p.u32(x.b, f::STATE) as i32) < 0x0402_0000 {
                    x.set8(Enemy::ACTIVE, 1);
                }
                x.set8(f::SUB2, 2);
                x.set8(Enemy::SUB_STEP, 0);
            }
            step(x);
        }
        _ => {}
    }
}

/// +6 = 6 (0x7267c): spits as soon as a player is in front.
fn spit<R: Bench>(x: &mut Cx<'_, R>) {
    if grabbed(x) {
        return set_state_long(x, KNOCKED);
    }
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            if !pick_target(x) {
                return;
            }
            step(x);
            if !lib::target_in_box(x, 0, 0x80, 0x20) {
                return;
            }
            x.add8(Enemy::SUB_STEP, 2);
            x.set8(Enemy::VULNERABLE, 1);
            play(x, 3);
        }
        2 => finish_to_stance(x),
        _ => {}
    }
}

/// End of a move: when the animation is over, back to the standing beast (0x726e6).
pub(super) fn finish_to_stance<R: Bench>(x: &mut Cx<'_, R>) {
    if x.anim_step() {
        set_state_long(x, ACTIVE_STANCE);
        play(x, 0);
    }
}

/// +6 = 8 (0x7270c): like `spit` but without waiting for the end of the move.
fn spit_once<R: Bench>(x: &mut Cx<'_, R>) {
    if grabbed(x) {
        return set_state_long(x, KNOCKED);
    }
    if x.u8(Enemy::SUB_STEP) == 0 {
        if !pick_target(x) {
            return;
        }
        step(x);
        if !lib::target_in_box(x, 0, 0x80, 0x20) {
            return;
        }
        x.add8(Enemy::SUB_STEP, 2);
        x.set8(Enemy::VULNERABLE, 1);
        set_state_long(x, ACTIVE_STANCE);
        play(x, 0);
    }
}

/// Waits for the end of the airborne leap phase (key frame), 0x727da.
pub(super) fn bound_wait<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::KEY_FRAME) != 0 {
        x.add8(Enemy::SUB_STEP, 2);
        x.set8(Enemy::VULNERABLE, 0);
    }
    step(x);
}

/// Landing of a leap in X and Z (0x727f0).
pub(super) fn bound_land<R: Bench>(x: &mut Cx<'_, R>) {
    if x.move_xz().gt() {
        return step(x);
    }
    x.set32(Obj::Z, 0);
    x.add8(Enemy::SUB_STEP, 2);
    x.set8(Enemy::VULNERABLE, 1);
    x.set8(DisplacerBeast::WANT_ANIM, 0x18);
    x.set8(DisplacerBeast::CUR_ANIM, 0x18);
    play(x, 0xf);
}

/// +6 = 10 (0x7277c) : bond en X.
fn bound<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            x.add8(Enemy::SUB_STEP, 2);
            x.set8(Enemy::VULNERABLE, 1);
            x.set8(DisplacerBeast::WANT_ANIM, 6);
            x.set8(DisplacerBeast::CUR_ANIM, 6);
            let vx = if x.u8(f::FLIP) != 0 { 0x3c0 } else { -0x3c0 };
            set_velocity(x, vx, 0x4c0, -0x38);
            play(x, 6);
        }
        2 => bound_wait(x),
        4 => bound_land(x),
        6 => finish_to_stance(x),
        _ => {}
    }
}

/// Start of a leap in depth (variants 12 and 14): speed in Y, high leap.
fn bound_y_start<R: Bench>(x: &mut Cx<'_, R>, vy: i16, anim_marker: u8, anim: u8) {
    x.set16(Enemy::VY, vy as u16);
    x.add8(Enemy::SUB_STEP, 2);
    x.set8(Enemy::VULNERABLE, 1);
    x.set8(DisplacerBeast::WANT_ANIM, anim_marker);
    x.set8(DisplacerBeast::CUR_ANIM, anim_marker);
    x.set16(Enemy::AY, 0);
    x.set16(Enemy::VZ, 0x500);
    x.set16(Enemy::GRAVITY, (-0x38i16) as u16);
    play(x, anim);
}

/// Landing of a leap in depth (0x728d4 / 0x728f0).
pub(super) fn bound_y_wait<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::KEY_FRAME) != 0 {
        lib::sound(x, crate::hw::sound::CMD_36F_46F);
        x.add8(Enemy::SUB_STEP, 2);
        x.set8(Enemy::VULNERABLE, 0);
    }
    step(x);
}

pub(super) fn bound_y_land<R: Bench>(x: &mut Cx<'_, R>) {
    if x.move_yz().gt() {
        return step(x);
    }
    x.set32(Obj::Z, 0);
    x.add8(Enemy::SUB_STEP, 2);
    x.set8(Enemy::VULNERABLE, 1);
    x.set8(Enemy::ACTIVE, 1);
    x.set8(DisplacerBeast::WANT_ANIM, 0x18);
    x.set8(DisplacerBeast::CUR_ANIM, 0x18);
    play(x, 0xf);
}

/// +6 = 12 (0x7282c): leap forward in depth.
fn bound_x<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            x.set16(Enemy::VY, 0x140);
            x.add8(Enemy::SUB_STEP, 2);
            x.set8(Enemy::ACTIVE, 2);
            x.set8(Enemy::VULNERABLE, 1);
            x.set8(DisplacerBeast::WANT_ANIM, 0xd);
            x.set8(DisplacerBeast::CUR_ANIM, 0xd);
            x.set16(Enemy::AY, 0);
            x.set16(Enemy::VZ, 0x500);
            x.set16(Enemy::GRAVITY, (-0x38i16) as u16);
            play(x, 9);
        }
        2 => bound_y_wait(x),
        4 => bound_y_land(x),
        6 => finish_to_stance(x),
        _ => {}
    }
}

/// +6 = 14 (0x72840): leap in depth backward.
fn bound_back<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => bound_y_start(x, -0x140, 0xe, 0xa),
        2 => bound_y_wait(x),
        4 => bound_y_land(x),
        6 => finish_to_stance(x),
        _ => {}
    }
}

/// +6 = 16 (0x72930): falls from the top of the screen.
fn drop_in<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            x.add8(Enemy::SUB_STEP, 2);
            x.set8(Enemy::VULNERABLE, 1);
            x.set8(DisplacerBeast::WANT_ANIM, 0x19);
            x.set16(Enemy::VZ, (-0x300i16) as u16);
            x.set16(Enemy::GRAVITY, (-0x10i16) as u16);
            let h = var::SCROLL_Y.get(x.b).wrapping_add(0x100).wrapping_sub(x.u16(Obj::Y));
            x.set16(Obj::Z, h);
            play(x, 0x10);
        }
        2 => {
            x.move_z();
            if (x.u16(Obj::Z) as i16) < 0 {
                x.add8(Enemy::SUB_STEP, 2);
                x.set32(Obj::Z, 0);
                play(x, 0xf);
            }
        }
        4 => finish_to_stance(x),
        _ => {}
    }
}

/// Forward slide with the speed from the table (0x729bc / 0x72a2a).
fn slide_start<R: Bench>(x: &mut Cx<'_, R>, anim: u8, delay: u16) {
    x.add8(Enemy::SUB_STEP, 2);
    x.set16(DisplacerBeast::TIMER, delay);
    x.set8(Obj::SPEED_INDEX, 8);
    if x.u8(f::FLIP) == 0 {
        x.set8(Obj::SPEED_INDEX, 0x18);
    }
    play(x, anim);
    speed(x, 0x17);
}

/// +6 = 18 (0x729ac) : glisse 30 images vers l'avant.
fn slide<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            x.set8(Enemy::VULNERABLE, 1);
            slide_start(x, 0xd, 0x1e);
        }
        2 => {
            let t = x.u16(DisplacerBeast::TIMER).wrapping_sub(1);
            x.set16(DisplacerBeast::TIMER, t);
            if (t as i16) < 0 {
                set_state_long(x, ACTIVE_STANCE);
            }
            step(x);
            move_xy(x);
        }
        _ => {}
    }
}

/// Crawl: random duration, then the beast springs up and strikes (+6 = 20 and 22, 0x72a16 / 0x72b16).
fn sneak_gen<R: Bench>(x: &mut Cx<'_, R>, throw: fn(&mut crate::world::World, crate::world::objects::Obj)) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            let r = rand_and(x, 0xf);
            let delay = tables::table("dbeast/crawl_delays").at16(x.b, 2 * r);
            slide_start(x, 8, delay);
        }
        2 => {
            let t = x.u16(DisplacerBeast::TIMER).wrapping_sub(1);
            x.set16(DisplacerBeast::TIMER, t);
            if (t as i16) >= 0 {
                step(x);
                return move_xy(x);
            }
            x.add8(Enemy::SUB_STEP, 2);
            play_b(x, 4);
        }
        4 => {
            if x.u8(f::KEY_FRAME) != 0 {
                x.set8(f::KEY_FRAME, 0);
                throw(x.b, x.o);
            }
            if x.anim_step() {
                x.add8(Enemy::SUB_STEP, 2);
                play(x, 5);
            }
        }
        6 => {
            if x.anim_step() {
                set_state_long(x, KNOCKED);
                let flip = x.u8(f::FLIP) ^ 0x20;
                x.set8(f::FLIP, flip);
                play(x, 0);
            }
        }
        _ => {}
    }
}

fn sneak<R: Bench>(x: &mut Cx<'_, R>) {
    sneak_gen(x, eshl_creators::throw_1);
}

fn sneak_b<R: Bench>(x: &mut Cx<'_, R>) {
    sneak_gen(x, eshl_creators::throw_2);
}
