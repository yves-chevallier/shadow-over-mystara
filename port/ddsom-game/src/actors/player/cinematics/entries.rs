//! Scene entries of the players (state `+4` = 0, `+5` = 4): common tools and routines 0x23ac8 to 0x253a0.

use crate::world::objects::Obj;
use crate::world::objects::player::Player;
use super::*;
use crate::actors::player::tables::{pc16, pc32};

// =================================================================================================
// Scene entries (`+5` = 0, table 0x23826 indexed by scene)
//
// The player appears (often off screen to the left), waits, walks to the right, strikes the pose
// and receives the controls when the game enters the play phase (`ram::PHASE` = 12). `ram::SUB_STEP`
// (`A5+0x2e`) is the step of the scene's entry script. The caller (`scripted::update`) reinserts the
// player into the actor list.
// =================================================================================================

/// Play phase: the players receive the controls.
pub(super) const PLAYING_PHASE: u8 = 12;
/// Second timer of the entries.
pub(super) const TIMER_B: u32 = 0x1e2;
/// Timer of the entries (word).
pub(super) const WAIT: u32 = 0x1e0;
/// Weapon in hand (0: 0x40 offset of the animations).
pub(super) const ARMED: u32 = 0x68;

/// Rewritten entries: original address -> Rust routine.
pub const INTRO_HANDLERS: &[(u32, Handler)] = &[
    (0x23ac8, |g, p| dispatch(g, p, 0x23ad6, INTRO_23AC8)),
    (0x23b32, intro_23b32),
    (0x24fe2, |g, p| dispatch(g, p, 0x24ff0, INTRO_24FE2)),
    (0x2522e, |g, p| dispatch(g, p, 0x2523c, INTRO_2522E)),
    (0x252ac, |g, p| dispatch(g, p, 0x252ba, INTRO_252AC)),
    (0x253a0, |g, p| dispatch(g, p, 0x253ae, INTRO_253A0)),
    (0x2541a, |g, p| dispatch(g, p, 0x25428, INTRO_2541A)),
    (0x25504, |g, p| dispatch(g, p, 0x25512, INTRO_25504)),
    (0x255ea, |g, p| dispatch(g, p, 0x255f8, INTRO_255EA)),
    (0x2564a, |g, p| dispatch(g, p, 0x25658, INTRO_2564A)),
    (0x256c2, |g, p| dispatch(g, p, 0x256d0, INTRO_256C2)),
    (0x257a4, |g, p| dispatch(g, p, 0x257b2, INTRO_257A4)),
    (0x25814, |g, p| dispatch(g, p, 0x25822, INTRO_25814)),
    (0x258de, |g, p| dispatch(g, p, 0x258ec, INTRO_258DE)),
    (0x25954, |g, p| dispatch(g, p, 0x25962, INTRO_25954)),
    (0x25a24, intro_25a24),
    (0x25aa2, |g, p| dispatch(g, p, 0x25ab0, INTRO_25AA2)),
    (0x25b3e, |g, p| dispatch(g, p, 0x25b4c, INTRO_25B3E)),
    (0x25ba0, |g, p| dispatch(g, p, 0x25bae, INTRO_25BA0)),
    (0x25c9e, |g, p| dispatch(g, p, 0x25cac, INTRO_25C9E)),
    (0x25d40, |g, p| dispatch(g, p, 0x25d4e, INTRO_25D40)),
    (0x25db4, |g, p| dispatch(g, p, 0x25dc2, INTRO_25DB4)),
    (0x25e28, |g, p| dispatch(g, p, 0x25e36, INTRO_25E28)),
    (0x25efc, intro_25efc),
    (0x25f7e, |g, p| dispatch(g, p, 0x25f8c, INTRO_25F7E)),
];

// ---- entry tools -------------------------------------------------------------------------

/// Appearance: the player becomes active; `visible` also sets `+1`; `armed`: weapon in hand.
pub(super) fn appear(g: &mut Ctx, p: Player, visible: bool, armed: bool) {
    next(g, p);
    p.set_u8(g, Player::ACTIVE, 1);
    if visible {
        p.set_u8(g, Player::FLAGS, 1);
    }
    p.set_u8(g, ARMED, armed as u8);
}

/// Last step: in the play phase, the player receives the controls.
pub(super) fn take_control(g: &mut Ctx, p: Player) {
    if ram::PHASE.get(g.b) == PLAYING_PHASE {
        p.set_state(g, 0x0200_0000);
    }
    anim_step(g, p);
}

/// Countdown of `WAIT`; true when it reaches zero.
pub(super) fn countdown(g: &mut Ctx, p: Player, field: u32) -> bool {
    let left = p.u16(g, field).wrapping_sub(1);
    p.set_u16(g, field, left);
    left == 0
}

/// Wait without animation: true as long as `field` is not exhausted (it is counted down).
pub(super) fn waiting(g: &mut Ctx, p: Player, field: u32) -> bool {
    let left = p.u16(g, field);
    if left != 0 {
        p.set_u16(g, field, left - 1);
    }
    left != 0
}

/// Advances by `dx` pixels.
pub(super) fn step_x(g: &mut Ctx, p: Player, dx: u16) {
    let x = p.u16(g, Obj::X).wrapping_add(dx);
    p.set_u16(g, Obj::X, x);
}

/// Walks to the right (2 pixels per frame) for `field` frames, then `done`.
pub(super) fn walk_right(g: &mut Ctx, p: Player, field: u32, done: fn(&mut Ctx, Player)) {
    step_x(g, p, 2);
    if countdown(g, p, field) {
        next(g, p);
        done(g, p);
    } else {
        anim_step(g, p);
    }
}

pub(super) fn anim_0(g: &mut Ctx, p: Player) {
    animation::start(g, p, 0);
}

/// Word of the table `table` for the player's rank (index `2 × rank`, byte).
pub(super) fn per_rank16(g: &mut Ctx, p: Player, table: u32) -> u16 {
    let index = (p.u8(g, RANK).wrapping_mul(2)) as u32;
    pc16(g.b, table + index)
}

/// End of the animation: still pose; in the meantime, the hero's cry at the gesture (fighter and elf).
pub(super) fn pose_with_shout(g: &mut Ctx, p: Player) {
    if anim_step(g, p) {
        next(g, p);
        start_stand(g, p);
        return;
    }
    if p.u8(g, Player::ANIM_EVENT) == 0 {
        return;
    }
    p.set_u8(g, Player::ANIM_EVENT, 0);
    if matches!(p.class(g), 0 | 2) {
        super::super::spawn::swap(g, p, 0);
    }
}

/// End of the animation: next step and animation `n`; nothing else otherwise.
pub(super) fn then_anim(g: &mut Ctx, p: Player, n: u16) {
    if anim_step(g, p) {
        next(g, p);
        animation::start(g, p, n);
    }
}

/// Countdown of `WAIT`, then the weapon in hand and pose 51.
pub(super) fn draw_weapon_after_wait(g: &mut Ctx, p: Player) {
    if countdown(g, p, WAIT) {
        next(g, p);
        p.set_u8(g, ARMED, 1);
        animation::start(g, p, 51);
    } else {
        anim_step(g, p);
    }
}

/// The player stands in front of the object followed by the camera (`A5+0x46`), shifted by its number (`+2`);
/// `table`: x offsets (words).
pub(crate) fn place_by_focus(g: &mut Ctx, p: Player, table: u32) -> u32 {
    let focus = ram::CAMERA_FOCUS.get(g.b) as i16 as i32 as u32;
    let (x, y) = (rd16(g.b, focus + 8), rd16(g.b, focus + 0x10));
    p.set_u16(g, Obj::X, x);
    p.set_u16(g, Obj::Y, y);
    p.set_u16(g, Obj::Z, 0);
    let index = 2 * p.u8(g, Player::INDEX) as u32;
    let dx = pc16(g.b, table + index);
    p.set_u16(g, Obj::X, x.wrapping_add(dx));
    focus
}

// ---- 0x23ac8, 0x23b32 ---------------------------------------------------------------------------

pub(super) const INTRO_23AC8: Steps = &[
    (0x23adc, |g, p| {
        appear(g, p, true, true);
        p.set_u8(g, Player::SPECIAL_ACTIVE, 0);
        p.set_u16(g, WAIT, 80);
        animation::start(g, p, 2);
    }),
    (0x23b02, |g, p| walk_right(g, p, WAIT, start_stand)),
    (0x23b1c, take_control),
];

/// 0x23b32: still appearance, then the controls in the play phase.
pub(super) fn intro_23b32(g: &mut Ctx, p: Player) {
    if p.u8(g, Player::ACTION) == 0 {
        appear(g, p, true, false);
        p.set_u8(g, Player::SPECIAL_ACTIVE, 0);
        p.set_u8(g, ARMED, 1);
        start_stand(g, p);
    } else if ram::PHASE.get(g.b) == PLAYING_PHASE {
        p.set_state(g, 0x0200_0000);
    }
}

// ---- 0x24fe2: walk-in, two turns, then a jump toward the bottom of the screen ---------

pub(super) const INTRO_24FE2: Steps = &[
    (0x25006, |g, p| {
        next(g, p);
        p.set_u8(g, Player::ACTIVE, 1);
        p.set_u8(g, Player::SPECIAL_ACTIVE, 0);
        p.set_u8(g, ARMED, 0);
        p.set_u16(g, WAIT, 136);
        p.set_u32(g, Obj::VX, 0x0200_0000);
        p.set_u32(g, Obj::VZ, 0);
        p.set_u32(g, Obj::VY, 0);
        animation::start(g, p, 2);
    }),
    (0x25034, |g, p| {
        if countdown(g, p, WAIT) {
            next(g, p);
            let wait = per_rank16(g, p, 0x25066);
            p.set_u16(g, WAIT, wait);
            crate::actors::families::effects_creators::for_players::halo(&mut g.leftovers, g.b, p.obj());
            anim_0(g, p);
        } else {
            anim_step(g, p);
            super::super::walk::move_x(g, p);
        }
    }),
    (0x2506e, |g, p| {
        if !countdown(g, p, WAIT) {
            return;
        }
        next(g, p);
        p.set_u8(g, ARMED, 1);
        animation::start(g, p, 51);
        if p.u8(g, RANK) == 0 {
            g.sound(crate::services::sound::CMD_90, p);
            g.sound(crate::services::sound::SCENE_MUSIC, p);
        }
    }),
    (0x2509a, |g, p| {
        if anim_step(g, p) {
            next(g, p);
            let wait = per_rank16(g, p, 0x250be);
            p.set_u16(g, WAIT, wait);
            anim_0(g, p);
        }
    }),
    (0x250c6, turn_after_wait),
    (0x250e0, |g, p| {
        if anim_step(g, p) {
            next(g, p);
            p.set_u16(g, WAIT, 16);
            anim_0(g, p);
        }
    }),
    (0x250fc, turn_after_wait),
    (0x25116, |g, p| {
        if !anim_step(g, p) {
            return;
        }
        next(g, p);
        p.set_u16(g, WAIT, 320);
        let at = 0x25154 + 8 * p.u8(g, RANK) as u32;
        let (vx, vy, delay) = (pc16(g.b, at), pc16(g.b, at + 2), pc16(g.b, at + 4));
        p.set_u16(g, Obj::VX, vx);
        p.set_u16(g, Obj::VY, vy);
        p.set_u16(g, TIMER_B, delay);
        p.set_u16(g, Obj::AX, 0);
        p.set_u16(g, Obj::AY, 0);
        anim_c(g, p, 1);
    }),
    (0x25174, |g, p| {
        if !waiting(g, p, TIMER_B) {
            super::super::walk::move_xy(g, p);
        }
        anim_step(g, p);
        if rd16(g.b, A5 - 0x65c4) != 0 {
            step_x(g, p, 0xfffe);
        }
        if countdown(g, p, WAIT) {
            next(g, p);
            p.set_u16(g, WAIT, 6);
            let k = p.u8(g, RANK) as u32;
            let drift = pc32(g.b, 0x251ce + 4 * k);
            p.set_u32(g, Obj::VY, drift);
            p.set_u16(g, Obj::VZ, 0x580);
            p.set_u16(g, Obj::AZ, (-56i16) as u16);
            p.set_u32(g, Obj::VX, 0);
            anim_c(g, p, 2);
        }
    }),
    (0x251de, |g, p| {
        super::super::walk::move_xz(g, p);
        step_x(g, p, 0xffff);
        if waiting(g, p, WAIT) {
            return;
        }
        let y = p.u32(g, Obj::Y).wrapping_add(p.u32(g, Obj::VY));
        p.set_u32(g, Obj::Y, y);
        if (p.u16(g, Obj::Z) as i16) < 0 {
            next(g, p);
            p.set_u32(g, Obj::Z, 0);
            anim_0(g, p);
        } else {
            anim_step(g, p);
        }
    }),
    (0x25218, take_control),
];

/// Countdown of `WAIT`, then turn (animation 5).
pub(super) fn turn_after_wait(g: &mut Ctx, p: Player) {
    if countdown(g, p, WAIT) {
        next(g, p);
        p.flip(g);
        animation::start(g, p, 5);
    }
}

// ---- 0x2522e, 0x252ac: the player follows the camera object (mount, raft...) -----------------

pub(super) const INTRO_2522E: Steps = &[
    (0x25240, |g, p| {
        appear(g, p, false, true);
        p.set_u8(g, Player::SPECIAL_ACTIVE, 0);
        p.set_u16(g, WAIT, 80);
        anim_0(g, p);
        follow_focus(g, p);
    }),
    (0x25294, |g, p| {
        if ram::PHASE.get(g.b) == PLAYING_PHASE {
            p.set_state(g, 0x0200_0000);
        }
        follow_focus(g, p);
        anim_step(g, p);
    }),
];

/// 0x25260: in front of the camera object, 18 pixels closer.
pub(super) fn follow_focus(g: &mut Ctx, p: Player) {
    place_by_focus(g, p, 0x2528c);
    let y = p.u16(g, Obj::Y).wrapping_sub(18);
    p.set_u16(g, Obj::Y, y);
}

pub(super) const INTRO_252AC: Steps = &[
    (0x252c2, |g, p| {
        appear(g, p, false, true);
        p.set_u8(g, Player::SPECIAL_ACTIVE, 0);
        p.set_u16(g, WAIT, 80);
        anim_0(g, p);
    }),
    (0x252e2, |g, p| {
        let focus = ram::CAMERA_FOCUS.get(g.b) as i16 as i32 as u32;
        let (x, y) = (rd16(g.b, focus + 8), rd16(g.b, focus + 0x10));
        p.set_u16(g, Obj::X, x);
        p.set_u16(g, Obj::Y, y.wrapping_sub(17));
        p.set_u16(g, Obj::Z, 0);
        let index = 2 * p.u8(g, Player::INDEX) as u32;
        let dx = pc16(g.b, 0x25348 + index);
        p.set_u16(g, Obj::X, x.wrapping_add(dx));
        if crate::hw::mem::rd8(g.b, focus + 5) == 0 {
            anim_step(g, p);
            return;
        }
        next(g, p);
        p.set_u16(g, Obj::VZ, 0x500);
        p.set_u16(g, Obj::AZ, (-56i16) as u16);
        p.set_u32(g, Obj::VX, 0);
        let drift = pc32(g.b, 0x25350 + 2 * index);
        p.set_u32(g, Obj::VY, drift);
        anim_c(g, p, 0);
    }),
    (0x25360, |g, p| {
        super::super::walk::move_xz(g, p);
        let y = p.u32(g, Obj::Y).wrapping_add(p.u32(g, Obj::VY));
        p.set_u32(g, Obj::Y, y);
        if (p.u16(g, Obj::Z) as i16) < 0 {
            next(g, p);
            p.set_u32(g, Obj::Z, 0);
            anim_0(g, p);
        } else {
            anim_step(g, p);
        }
    }),
    (0x2538a, take_control),
];

// ---- 0x253a0 ------------------------------------------------------------------------------------

pub(super) fn intro_253a0_start(g: &mut Ctx, p: Player) {
    appear(g, p, false, true);
    p.set_u16(g, WAIT, 160);
    animation::start(g, p, 7);
}

pub(super) const INTRO_253A0: Steps = &[
    (0x253b4, |g, p| {
        if ram::SUB_STEP.get(g.b) == 14 {
            intro_253a0_start(g, p);
        }
    }),
    (0x253bc, intro_253a0_start),
    (0x253da, |g, p| {
        if countdown(g, p, WAIT) {
            next(g, p);
            p.set_u16(g, WAIT, 96);
            anim_0(g, p);
        } else if p.u16(g, WAIT) <= 40 {
            step_x(g, p, 4);
            anim_step(g, p);
        }
    }),
    (0x25406, |g, p| {
        if countdown(g, p, WAIT) {
            p.set_state(g, 0x0200_0000);
        }
        anim_step(g, p);
    }),
];
