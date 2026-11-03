//! Scene entries of the players, continued: routines 0x2541a to 0x25f7e (walk-ins, jumps, falls).

use crate::objects::Obj;
use crate::player::Player;
use super::*;
use crate::player::tables::{pc8, pc16};

// ---- 0x2541a, 0x25504, 0x255ea, 0x2564a, 0x256c2, 0x257a4 -------------------------------------------

/// Wait of `WAIT` frames (without animation), then `WAIT` = 80.
pub(super) fn wait_then_walk(g: &mut Ctx, p: Player) {
    if countdown(g, p, WAIT) {
        next(g, p);
        p.set_u16(g, WAIT, 80);
    }
}

pub(super) const INTRO_2541A: Steps = &[
    (0x25434, |g, p| {
        appear(g, p, true, false);
        p.set_u8(g, Player::SPECIAL_ACTIVE, 0);
        p.set_u16(g, WAIT, 224);
        animation::start(g, p, 2);
    }),
    (0x25458, wait_then_walk),
    (0x2546a, |g, p| {
        walk_right(g, p, WAIT, |g, p| {
            let index = p.u8(g, Player::INDEX) as u32;
            let wait = pc16(g.b, 0x25492 + 2 * index);
            p.set_u16(g, WAIT, wait);
            start_stand(g, p);
        })
    }),
    (0x2549a, draw_weapon_after_wait),
    (0x254b8, pose_with_shout),
    (0x254ee, take_control),
];

pub(super) const INTRO_25504: Steps = &[
    (0x25520, |g, p| {
        appear(g, p, false, false);
        p.set_u16(g, WAIT, 224);
        animation::start(g, p, 2);
    }),
    (0x2553a, wait_then_walk),
    (0x2554c, |g, p| walk_right(g, p, WAIT, anim_0)),
    (0x25568, |g, p| {
        if ram::SUB_STEP.get(g.b) == 8 {
            next(g, p);
            let wait = per_rank16(g, p, 0x2558e);
            p.set_u16(g, WAIT, wait);
            crate::families::effects_creators::for_players::halo(&mut g.leftovers, g.b, p.obj());
        }
        anim_step(g, p);
    }),
    (0x25596, |g, p| {
        if ram::SUB_STEP.get(g.b) >= 10 {
            draw_weapon_after_wait(g, p);
        } else {
            anim_step(g, p);
        }
    }),
    (0x255be, |g, p| then_anim(g, p, 0)),
    (0x255d4, take_control),
];

pub(super) const INTRO_255EA: Steps = &[
    (0x255fe, |g, p| {
        appear(g, p, false, false);
        anim_0(g, p);
    }),
    (0x25612, |g, p| {
        if ram::SUB_STEP.get(g.b) == 6 {
            next(g, p);
            p.set_u8(g, ARMED, 1);
            animation::start(g, p, 51);
        } else {
            anim_step(g, p);
        }
    }),
    (0x25634, take_control),
];

pub(super) const INTRO_2564A: Steps = &[
    (0x2565e, |g, p| {
        appear(g, p, false, true);
        super::super::hurt::hurt_animation(g, p, 16);
    }),
    (0x25674, |g, p| {
        if ram::PHASE.get(g.b) == PLAYING_PHASE {
            next(g, p);
            let wait = per_rank16(g, p, 0x25690);
            p.set_u16(g, WAIT, wait);
        }
    }),
    (0x25698, |g, p| {
        if waiting(g, p, WAIT) {
            return;
        }
        if anim_step(g, p) {
            p.set_state(g, 0x0200_0000);
            anim_0(g, p);
        }
    }),
];

pub(super) const INTRO_256C2: Steps = &[
    (0x256dc, |g, p| {
        appear(g, p, false, false);
        p.set_u16(g, WAIT, 80);
        animation::start(g, p, 2);
    }),
    (0x256f6, |g, p| {
        if ram::SCRIPTED_B.get(g.b) != 0 || ram::SUB_STEP.get(g.b) == 6 {
            next(g, p);
        }
    }),
    (0x2570a, |g, p| {
        walk_right(g, p, WAIT, |g, p| {
            let wait = per_rank16(g, p, 0x25732);
            p.set_u16(g, WAIT, wait);
            start_stand(g, p);
        })
    }),
    (0x2573a, draw_weapon_after_wait),
    (0x25758, pose_with_shout),
    (0x2578e, take_control),
];

pub(super) const INTRO_257A4: Steps = &[
    (0x257ba, |g, p| {
        appear(g, p, false, true);
        animation::start(g, p, 2);
    }),
    (0x257d0, |g, p| {
        if ram::SUB_STEP.get(g.b) == 6 {
            next(g, p);
            p.set_u16(g, WAIT, 80);
        }
    }),
    (0x257e4, |g, p| walk_right(g, p, WAIT, start_stand)),
    (0x257fe, take_control),
];

// ---- 0x25814, 0x258de, 0x25954, 0x25a24: jumps, slow walk, falls ------------------------------

pub(super) const INTRO_25814: Steps = &[
    (0x25828, |g, p| {
        appear(g, p, false, true);
        let wait = per_rank16(g, p, 0x2584c);
        p.set_u16(g, WAIT, wait);
        animation::start(g, p, 2);
    }),
    (0x25854, |g, p| {
        walk_right(g, p, WAIT, |g, p| {
            let at = 0x2589a + 8 * p.u8(g, RANK) as u32;
            let (vx, vz, az, height) = (pc16(g.b, at), pc16(g.b, at + 2), pc16(g.b, at + 4), pc16(g.b, at + 6));
            p.set_u16(g, Obj::VX, vx);
            p.set_u16(g, Obj::AX, 0);
            p.set_u16(g, Obj::VZ, vz);
            p.set_u16(g, Obj::AZ, az);
            p.set_u16(g, Obj::Z, height);
            let y = p.u16(g, Obj::Y).wrapping_sub(height);
            p.set_u16(g, Obj::Y, y);
            anim_c(g, p, 2);
        })
    }),
    (0x258ba, |g, p| {
        super::super::walk::move_xz(g, p);
        if (p.u16(g, Obj::Z) as i16) <= 0 {
            p.set_state(g, 0x0200_0000);
            p.set_u32(g, Obj::Z, 0);
            start_stand(g, p);
        } else {
            anim_step(g, p);
        }
    }),
];

pub(super) const INTRO_258DE: Steps = &[
    (0x258f0, |g, p| {
        appear(g, p, false, false);
        let slow = ram::SLOW_MODE.get(g.b) != 0;
        p.set_u16(g, WAIT, if slow { 128 } else { 80 });
        p.set_u16(g, Obj::VX, if slow { 1 } else { 2 });
        animation::start(g, p, if slow { 53 } else { 2 });
    }),
    (0x25924, |g, p| {
        let dx = p.u16(g, Obj::VX);
        step_x(g, p, dx);
        if !countdown(g, p, WAIT) {
            anim_step(g, p);
            return;
        }
        p.set_state(g, 0x0200_0000);
        if ram::SLOW_MODE.get(g.b) == 0 {
            start_stand(g, p);
        } else {
            animation::start(g, p, 53);
        }
    }),
];

/// Fall from the top of the screen: true on landing (animation 14); `+0x3b` is cleared as long as the
/// player is higher than `high`.
pub(super) fn drop_in(g: &mut Ctx, p: Player, high: u16, clear_z: bool) {
    move_z(g, p);
    if (p.u16(g, Obj::Z) as i16) <= 0 {
        next(g, p);
        if clear_z {
            p.set_u32(g, Obj::Z, 0);
        }
        animation::start(g, p, 14);
        return;
    }
    anim_step(g, p);
    if p.u16(g, Obj::Z) > high {
        p.set_u8(g, Player::GROUND_MARK, 0);
    }
}

pub(super) const INTRO_25954: Steps = &[
    (0x25968, |g, p| {
        appear(g, p, false, true);
        p.set_u16(g, Obj::VZ, (-256i16) as u16);
        p.set_u16(g, Obj::AZ, (-64i16) as u16);
        let at = 0x259b0 + 8 * p.u8(g, RANK) as u32;
        let (x, z, y, wait) = (pc16(g.b, at), pc16(g.b, at + 2), pc16(g.b, at + 4), pc16(g.b, at + 6));
        p.set_u16(g, Obj::X, x);
        p.set_u16(g, Obj::Z, z);
        p.set_u16(g, Obj::Y, y);
        p.set_u16(g, WAIT, wait);
        animation::start(g, p, 56);
        p.set_u8(g, Player::GROUND_MARK, 0);
    }),
    (0x259d0, |g, p| {
        if !waiting(g, p, WAIT) {
            drop_in(g, p, 80, false);
        }
    }),
    (0x25a08, |g, p| {
        if anim_step(g, p) {
            p.set_state(g, 0x0200_0000);
            p.set_u32(g, Obj::Z, 0);
            start_stand(g, p);
        }
    }),
];

/// 0x25a24: appearance at a fixed spot (table per rank: x, height, depth, direction), then action 0x2a
/// (jump in place) after 48 frames.
pub(super) fn intro_25a24(g: &mut Ctx, p: Player) {
    if p.u8(g, Player::ACTION) != 0 {
        if countdown(g, p, WAIT) {
            p.set_state(g, 0x0200_2a00);
            p.set_u8(g, Player::GROUND_MARK, 1);
        }
        return;
    }
    next(g, p);
    p.set_u16(g, WAIT, 48);
    p.set_u8(g, Player::ACTIVE, 1);
    p.set_u8(g, ARMED, 1);
    let at = 0x25a6c + 8 * p.u8(g, RANK) as u32;
    let (x, z, y) = (pc16(g.b, at), pc16(g.b, at + 2), pc16(g.b, at + 4));
    let facing = pc8(g.b, at + 7);
    p.set_u16(g, Obj::X, x);
    p.set_u16(g, Obj::Z, z);
    p.set_u16(g, Obj::Y, y);
    p.set_u8(g, Player::FACING, facing);
    animation::start(g, p, 13);
    p.set_u8(g, Player::GROUND_MARK, 0);
}

// ---- 0x25aa2, 0x25b3e, 0x25ba0, 0x25c9e, 0x25d40, 0x25db4: walk-ins ----------------------

/// 0x25b20: at scene 41, one frame out of sixteen (according to the counter and the byte `A5+2`, like the original),
/// footstep effect in water (h).
pub(super) fn splash(g: &mut Ctx, p: Player) {
    if ram::SCENE.get(g.b) == 41 && ram::FRAME_COUNTER.get(g.b).wrapping_add(ram::INPUTS_2.get(g.b)) & 15 == 0 {
        crate::families::effects_creators::for_players::wisp(&mut g.leftovers, g.b, p.obj());
    }
}

pub(super) const INTRO_25AA2: Steps = &[
    (0x25ab8, |g, p| {
        appear(g, p, false, true);
        p.set_u16(g, WAIT, 224);
        animation::start(g, p, 2);
    }),
    (0x25ad4, wait_then_walk),
    (0x25ae6, |g, p| {
        step_x(g, p, 2);
        if countdown(g, p, WAIT) {
            next(g, p);
            anim_0(g, p);
        }
        splash(g, p);
        anim_step(g, p);
    }),
    (0x25b06, |g, p| {
        if ram::PHASE.get(g.b) == PLAYING_PHASE {
            p.set_state(g, 0x0200_0000);
        }
        splash(g, p);
        anim_step(g, p);
    }),
];

pub(super) const INTRO_25B3E: Steps = &[
    (0x25b52, |g, p| {
        appear(g, p, false, true);
        p.set_u16(g, WAIT, 80);
        animation::start(g, p, 2);
    }),
    (0x25b6e, |g, p| walk_right(g, p, WAIT, anim_0)),
    (0x25b8a, take_control),
];

pub(super) const INTRO_25BA0: Steps = &[
    (0x25bba, |g, p| {
        appear(g, p, false, false);
        let x = ram::CAMERA_X.get(g.b);
        let at = 0x25bf6 + 4 * p.u8(g, RANK) as u32;
        let (dx, dy) = (pc16(g.b, at), pc16(g.b, at + 2));
        p.set_u16(g, Obj::X, x.wrapping_add(dx));
        p.set_u16(g, Obj::Y, 280u16.wrapping_add(dy));
        p.set_u32(g, Obj::Z, 0);
        animation::start(g, p, 2);
    }),
    (0x25c06, |g, p| {
        if ram::SUB_STEP.get(g.b) == 6 {
            next(g, p);
            p.set_u16(g, TIMER_B, 80);
        }
    }),
    (0x25c1a, |g, p| {
        walk_right(g, p, TIMER_B, |g, p| {
            let wait = per_rank16(g, p, 0x25c4a);
            p.set_u16(g, WAIT, wait);
            crate::families::effects_creators::for_players::halo(&mut g.leftovers, g.b, p.obj());
            anim_0(g, p);
        })
    }),
    (0x25c52, draw_weapon_after_wait),
    (0x25c72, |g, p| then_anim(g, p, 0)),
    (0x25c88, take_control),
];

/// Walk after a wait: `WAIT` frames of waiting (without animation), then walk of `TIMER_B` frames.
pub(super) fn delayed_walk(g: &mut Ctx, p: Player) {
    if waiting(g, p, WAIT) {
        return;
    }
    walk_right(g, p, TIMER_B, anim_0);
}

pub(super) const INTRO_25C9E: Steps = &[
    (0x25cb6, |g, p| {
        appear(g, p, false, true);
        p.set_u16(g, WAIT, 80);
        animation::start(g, p, 2);
        if ram::NO_WALK_IN.get(g.b) == 0 {
            p.set_u8(g, Player::ACTION, 6);
            p.set_u16(g, TIMER_B, 480);
        }
    }),
    (0x25ce6, |g, p| walk_right(g, p, WAIT, anim_0)),
    (0x25d02, take_control),
    (0x25d18, |g, p| {
        if waiting(g, p, TIMER_B) {
            return;
        }
        walk_right(g, p, WAIT, anim_0);
    }),
];

pub(super) const INTRO_25D40: Steps = &[
    (0x25d54, |g, p| intro_wait_walk(g, p, 72)),
    (0x25d76, delayed_walk),
    (0x25d9e, take_control),
];

pub(super) const INTRO_25DB4: Steps = &[
    (0x25dc8, |g, p| intro_wait_walk(g, p, 64)),
    (0x25dea, delayed_walk),
    (0x25e12, take_control),
];

/// Appearance: wait of 224 frames, then walk of `walk` frames.
pub(super) fn intro_wait_walk(g: &mut Ctx, p: Player, walk: u16) {
    appear(g, p, false, true);
    p.set_u16(g, WAIT, 224);
    p.set_u16(g, TIMER_B, walk);
    animation::start(g, p, 2);
}

// ---- 0x25e28, 0x25efc, 0x25f7e -----------------------------------------------------------------------

pub(super) const INTRO_25E28: Steps = &[
    (0x25e3e, |g, p| {
        appear(g, p, false, true);
        p.set_u16(g, WAIT, 192);
        p.set_u16(g, Obj::VZ, (-256i16) as u16);
        p.set_u16(g, Obj::AZ, (-64i16) as u16);
        p.set_u16(g, Obj::Z, 256);
        let k = (p.u8(g, RANK) & 3) as u32;
        let delay = pc16(g.b, 0x25e82 + 2 * k);
        p.set_u16(g, TIMER_B, delay);
        animation::start(g, p, 56);
        p.set_u8(g, Player::GROUND_MARK, 0);
    }),
    (0x25e8a, |g, p| {
        if waiting(g, p, WAIT) || waiting(g, p, TIMER_B) {
            return;
        }
        drop_in(g, p, 112, true);
    }),
    (0x25ed2, stand_after_anim),
    (0x25ee6, take_control),
];

/// 0x25efc: without `A5+0x5c`, wait for the signal `A5+0x51e`, then wait and walk.
pub(super) fn intro_25efc(g: &mut Ctx, p: Player) {
    if ram::NO_VICTORY.get(g.b) == 0 {
        dispatch(g, p, 0x25f12, INTRO_25EFC);
    }
}

pub(super) const INTRO_25EFC: Steps = &[
    (0x25f18, |g, p| {
        if ram::BOSS_SIGNAL.get(g.b) != 0 {
            appear(g, p, false, true);
            p.set_u16(g, WAIT, 192);
            p.set_u16(g, TIMER_B, 80);
        }
        animation::start(g, p, 2);
    }),
    (0x25f40, delayed_walk),
    (0x25f68, take_control),
];

pub(super) const INTRO_25F7E: Steps = &[
    (0x25f9e, |g, p| {
        next(g, p);
        p.set_u8(g, ARMED, 0);
        if ram::NO_VICTORY.get(g.b) == 0 {
            p.set_u8(g, Player::ACTION, 16);
            p.set_u8(g, ARMED, 1);
        }
        p.set_u8(g, Player::ACTIVE, 1);
        p.set_u16(g, WAIT, 32);
        let (x, y) = (ram::CAMERA_X.get(g.b), ram::CAMERA_Y.get(g.b));
        let k = (p.u8(g, RANK) & 3) as u32;
        let (dx, dy) = (pc16(g.b, 0x26004 + 4 * k), pc16(g.b, 0x26006 + 4 * k));
        p.set_u16(g, Obj::X, x.wrapping_add(dx));
        p.set_u16(g, Obj::Y, y.wrapping_add(dy));
        p.set_u8(g, Player::FACING, if k & 1 != 0 { 0 } else { 0x20 });
        anim_0(g, p);
    }),
    (0x26014, |g, p| {
        if ram::SUB_STEP.get(g.b) == 8 && countdown(g, p, WAIT) {
            next(g, p);
            let k = (p.u8(g, RANK) & 3) as u32;
            let wait = pc16(g.b, 0x26040 + 2 * k);
            p.set_u16(g, WAIT, wait);
            animation::start(g, p, 2);
        }
    }),
    (0x26048, |g, p| {
        let dx: u16 = if p.u8(g, Player::FACING) != 0 { 2 } else { 0xfffe };
        step_x(g, p, dx);
        if countdown(g, p, WAIT) {
            next(g, p);
            animation::start(g, p, 8);
        } else {
            anim_step(g, p);
        }
    }),
    (0x26070, |g, p| {
        if anim_step(g, p) {
            next(g, p);
        }
    }),
    (0x2607e, |g, p| {
        if ram::SUB_STEP.get(g.b) == 12 {
            next(g, p);
            p.set_u16(g, WAIT, 64);
        }
    }),
    (0x26092, |g, p| {
        if countdown(g, p, WAIT) {
            next(g, p);
            animation::start(g, p, 9);
        }
    }),
    (0x260a6, |g, p| {
        if anim_step(g, p) {
            next(g, p);
            p.set_u8(g, ARMED, 1);
            animation::start(g, p, 51);
        }
    }),
    (0x260c2, |g, p| then_anim(g, p, 0)),
    (0x260d8, take_control),
];
