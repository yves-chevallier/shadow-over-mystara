//! Reactions to hits: player hit (`+5` = 4), player on the ground (`+4` = 4), dead (P4).
//!
//! When a hit strikes a player (`combat`), they enter the "hit" state (`0x02040000`, see
//! `states`) or, if they have no hit points left, "on the ground" (`0x0400xxxx`). Byte `+6` then selects the
//! *reaction* (recoil, throw, fall...), each being a small automaton whose step is `+7`.
//! Reactions and steps are routines of the original code, reached through offset tables;
//! many steps are shared (airborne throw, bounce, getting up, invulnerability blinking).
//!
//! The `HANDLERS` table gives, for an original address, the Rust version (all the reachable ones are rewritten).
//! The original tables remain the source of truth for the dispatch.

use crate::machine::Machine;
use super::actions::to_stand;
use super::walk::{clamp_to_screen, move_x, move_xyz, move_xz};
use super::{animation, ram, Ctx, Player};
use crate::mem::{wr16, wr8};
use crate::objects::anim::{load_entry, script_of};
use crate::objects::lists::ACTORS;
use crate::objects::Obj;
use crate::player::tables::{player_data, pc16, Aligned};

mod throws;
pub use throws::*;
mod seizures;
use seizures::*;

type Handler = fn(&mut Ctx, Player);

/// Identifiers (original addresses) of the routines reached (sounds).
mod rom {
    pub const BEFORE_HURT_A: crate::sound::Sound = crate::sound::CMD_A3_BY_CHARACTER;
    /// Death effects (sound, points).
    pub const DEATH_EFFECT: crate::sound::Sound = crate::sound::CMD_A4_BY_CHARACTER;
    /// Get-up sounds.
    pub const GET_UP_SOUND_A: crate::sound::Sound = crate::sound::GET_UP_A;
    pub const GET_UP_SOUND_B: crate::sound::Sound = crate::sound::GET_UP_B;
}

/// Step tables (words: offsets relative to the table, indexed by `+7`).
mod steps {
    pub const R6: u32 = 0x3a4d2;
    pub const R14: u32 = 0x3a858;
    pub const R26: u32 = 0x3aa9e;
    pub const R32: u32 = 0x3ad3a;
    pub const DOWN_6: u32 = 0x3b86e;
    pub const DOWN_A: u32 = 0x3b890;
}

/// Dispatch tables per action (`+6`).
const HURT_ACTIONS: u32 = 0x3a0f4;
const DOWN_ACTIONS: u32 = 0x3b820;


/// Rewritten reactions and steps: original address -> Rust routine.
const HANDLERS: &[(u32, Handler)] = &[
    (0x3a296, |g, p| {
        p.set_u8(g, Player::SLOW, 0);
        hurt_animation(g, p, 0xe);
        launch_common(g, p);
    }),
    (0x3a59a, |g, p| launch_high(g, p, 6)),
    (0x3a5a4, |g, p| launch_high(g, p, 0xa)),
    (0x3aa16, |g, p| {
        if is_dead(g, p) {
            p.set_u16(g, Player::MAIN, 0x0400);
            p.set_u8(g, Player::ACTIVE, 2);
            g.sound(rom::DEATH_EFFECT, p);
        }
        launch_slow(g, p);
    }),
    (0x3aa2e, launch_slow),
    (0x3aa5e, |g, p| {
        if animation::step(g, p) {
            next_step(g, p);
            p.set_u8(g, Player::TIMER_6F, 0x14);
            p.set_u16(g, Obj::VX, 0x440);
            p.set_u16(g, Obj::AX, (-0x40i16) as u16);
            if p.faces_left(g) {
                let v = p.u32(g, Obj::VX).wrapping_neg();
                p.set_u32(g, Obj::VX, v);
            }
            hurt_animation(g, p, 0xf);
        }
    }),
    (0x3aff8, slide_to_rest),
    (0x3b592, stars_start),
    (0x3b5b2, stars_shake),
    (0x3b656, stars_end),
    (0x3b740, kneel_start),
    (0x3b766, kneel_stars),
    (0x3b78a, kneel_fall),
    (0x3b7a4, |g, p| {
        animation::step(g, p);
    }),
    (0x3b7aa, |g, p| {
        animation::step(g, p);
        let left = p.u8(g, Player::TIMER_6F).wrapping_sub(1);
        p.set_u8(g, Player::TIMER_6F, left);
        if left == 0 {
            recover(g, p);
        }
    }),
    (0x3b7ca, |g, p| {
        next_step(g, p);
        // Original glitch: the setting of the stars is D1.b, the offset of the step left by the dispatch
        let setting = g.jumped.offset as u8;
        crate::families::effects_creators::for_players::blow(&mut g.leftovers, g.b, p.obj(), 1, setting);
        p.set_u8(g, Player::TIMER_6F, 0x50);
        hurt_animation(g, p, 0x11);
    }),
    (0x3b7e2, |g, p| {
        animation::step(g, p);
        let left = p.u8(g, Player::TIMER_6F).wrapping_sub(1);
        p.set_u8(g, Player::TIMER_6F, left);
        if left == 0 {
            recover(g, p);
        }
    }),
    (0x3b8d6, |g, p| {
        next_step(g, p);
        hurt_animation(g, p, 0x1d);
    }),
    (0x3b8e0, |g, p| {
        if animation::step(g, p) {
            next_step(g, p);
            p.set_u8(g, Player::TIMER_6F, 0x14);
        }
    }),
    (0x3a136, reaction_0),
    (0x3a79e, reaction_10),
    (0x3a4c6, |g, p| jump(g, p, steps::R6)),
    (0x3a84c, |g, p| jump(g, p, steps::R14)),
    (0x3aa92, |g, p| jump(g, p, steps::R26)),
    (0x3ad2e, |g, p| jump(g, p, steps::R32)),
    (0x3b862, |g, p| jump(g, p, steps::DOWN_6)),
    (0x3b884, |g, p| jump(g, p, steps::DOWN_A)),
    (0x3a4e8, |g, p| launch_with_anim(g, p, 6)),
    (0x3a4f2, |g, p| launch_with_anim(g, p, 0xa)),
    (0x3a2de, |g, p| next_animation(g, p, 7)),
    (0x3a2e4, |g, p| next_animation(g, p, 0xb)),
    (0x3a2f8, |g, p| fall_to_apex(g, p, 8)),
    (0x3a2fe, |g, p| fall_to_apex(g, p, 0xc)),
    (0x3a328, |g, p| land(g, p, 9)),
    (0x3a32e, |g, p| land(g, p, 0xd)),
    (0x3a522, bounce_up),
    (0x3a362, bounce_up_small),
    (0x3a384, land_with_effect),
    (0x3a544, slide_start),
    (0x3a3ae, slide_start_short),
    (0x3a3e2, slide),
    (0x3a424, get_up),
    (0x3a466, finish_get_up),
    (0x3a472, invulnerable_blink),
    (0x3a2ae, launch_a),
    (0x3aaac, launch_b),
    (0x3aae8, fall_b),
    (0x3ad4c, launch_c),
    (0x3ad66, fall_c),
    (0x3adbe, shock_start),
    (0x3ae0e, shock_fall),
    (0x3ae32, shock_struggle),
    (0x3ae7a, shock_release),
    (0x3aeaa, dizzy_start),
    (0x3aeca, dizzy),
    (0x3b1fa, sit_start),
    (0x3b264, sit_fall),
    (0x3b27e, |g, p| {
        if animation::step(g, p) {
            sit_down(g, p);
        }
    }),
    (0x3b28a, sit_struggle),
    (0x3b44c, held),
    (0x3b326, frozen_start),
    (0x3b3aa, frozen_fall),
    (0x3b3d8, |g, p| frozen_wait(g, p, 20)),
    (0x3b3de, |g, p| frozen_wait(g, p, 21)),
    (0x3b420, frozen_end),
    (0x3b684, fallen_start),
    (0x3b6da, fallen_drop),
    (0x3b6f4, fallen_lie),
    (0x3b700, fallen_stars),
    (0x3a204, |g, p| {
        if is_dead(g, p) {
            knock_down(g, p);
            return downed(g, p);
        }
        next_step(g, p);
        set_launch(g, p, 0x200, 0x200);
        face_attacker(g, p);
        hurt_animation(g, p, 3);
    }),
    (0x3a238, |g, p| {
        animation::step(g, p);
        if p.u8(g, Player::ANIM_EVENT) != 0 {
            next_step(g, p);
        }
    }),
    (0x3a24c, |g, p| {
        move_xz(g, p);
        if altitude(g, p) < 0 {
            p.set_u32(g, Obj::Z, 0);
            p.set_u16(g, Obj::ANIM_TIMER, 1);
            next_step(g, p);
        }
    }),
    (0x3a268, finish_get_up),
    (0x3a2a4, |g, p| {
        hurt_animation(g, p, 6);
        launch_common(g, p);
    }),
    (0x3b040, |g, p| {
        let left = p.u8(g, Player::TIMER_6F).wrapping_sub(1);
        p.set_u8(g, Player::TIMER_6F, left);
        if left == 0 {
            next_step(g, p);
            hurt_animation(g, p, 16);
        }
    }),
    (0x3b074, |g, p| {
        next_step(g, p);
        hurt_animation(g, p, 30);
    }),
    (0x3b07e, |g, p| {
        if animation::step(g, p) {
            p.set_u8(g, Player::ACTIVE, 1);
            p.set_u8(g, Player::INVULN, 0);
            next_step(g, p);
            p.set_u8(g, Player::TIMER_6F, 96);
        }
    }),
    (0x3b0a0, |g, p| {
        if struggle_out(g, p) {
            next_step(g, p);
            hurt_animation(g, p, 32);
        }
    }),
    (0x3b054, |g, p| {
        let ended = animation::step(g, p);
        if ended || (p.u8(g, Player::SLOW) != 0 && p.u8(g, Player::ANIM_EVENT) != 0) {
            next_step(g, p);
        }
    }),
    (0x3a656, |g, p| hurl(g, p, 0x200, 6)),
    (0x3b4e0, |g, p| {
        next_step(g, p);
        p.set_u32(g, Obj::VX, 0);
        p.set_u8(g, Player::INVULN, 50);
        p.set_u16(g, Obj::VZ, 0x200);
        p.set_u16(g, Obj::AZ, (-64i16) as u16);
        hurt_animation(g, p, 22);
        knock_down_if_dead(g, p);
    }),
    (0x3b504, crash_landing),
    (0x3a68a, |g, p| hurled(g, p, 8, 16)),
    (0x3a706, |g, p| hurled_landing(g, p, 9, false)),
    (0x3a75a, |g, p| hurled_slide(g, p, 26)),
    (0x3ab7c, |g, p| hurl(g, p, 0x100, 24)),
    (0x3a8de, |g, p| hurl(g, p, 0x200, 10)),
    (0x3a912, |g, p| hurled(g, p, 12, 16)),
    (0x3abb0, |g, p| hurled(g, p, 12, 14)),
    (0x3ac2c, |g, p| hurled_landing(g, p, 13, true)),
    (0x3ac86, |g, p| start_slide(g, p, 0x400)),
    (0x3acba, |g, p| hurled_slide(g, p, 24)),
    (0x3acfe, |g, p| {
        if p.u8(g, Player::ANIM_EVENT) == 0 {
            move_xz(g, p);
            if altitude(g, p) <= 0 {
                p.set_u32(g, Obj::Z, 0);
                next_step(g, p);
                hurt_animation(g, p, 9);
                g.sound(crate::sound::LANDING_EFFECT, p);
                return;
            }
        }
        animation::step(g, p);
    }),
    (0x3b150, |g, p| {
        if animation::step(g, p) {
            next_step(g, p);
        }
    }),
    (0x3a98e, hurt_slow),
    (0x3b2c2, hurt_short),
    (0x3a80a, |g, p| {
        if is_dead(g, p) {
            p.set_u16(g, Player::MAIN, 0x0400);
            p.set_u8(g, Player::ACTIVE, 2);
            return run_at(g, p, 0x3b884);
        }
        next_step(g, p);
        set_launch(g, p, 0x200, 0x200);
        face_attacker(g, p);
        hurt_animation(g, p, 4);
    }),
    (0x3ab34, |g, p| {
        next_step(g, p);
        set_launch(g, p, 0x180, 0x400);
        face_attacker(g, p);
        let anim = 0x16 + (ram::FRAME_COUNTER.get(g.b) & 1) as u16;
        hurt_animation(g, p, anim);
        knock_down_if_dead(g, p);
    }),
    (0x3af68, |g, p| {
        if rehit(g, p, 4, 0x3a274) {
            return;
        }
        if p.u16(g, Obj::Z) as i16 > 0 {
            next_step(g, p);
            set_launch_keep(g, p, 0x180, 0x400);
            let anim = 0x16 + (ram::FRAME_COUNTER.get(g.b) & 1) as u16;
            hurt_animation(g, p, anim);
            return face_attacker(g, p);
        }
        p.set_u8(g, Player::STEP, 0x10);
        let freeze = p.u8(g, Player::HIT_FREEZE);
        p.set_u8(g, Player::HITSTOP, freeze);
        face_attacker(g, p);
        let anim = if p.u8(g, Player::SLOW) != 0 { 2 } else { 0 };
        hurt_animation(g, p, anim);
    }),
    (0x3b0e8, |g, p| {
        if rehit(g, p, 4, 0x3a274) {
            return;
        }
        next_step(g, p);
        set_launch_keep(g, p, 0x200, 0x200);
        let anim = 3 + (ram::FRAME_COUNTER.get(g.b) & 1) as u16;
        hurt_animation(g, p, anim);
        face_attacker(g, p);
    }),
    (0x3b188, |g, p| {
        if rehit(g, p, 8, 0x3a578) {
            return;
        }
        next_step(g, p);
        set_launch_keep(g, p, 0x280, 0x500);
        hurt_animation(g, p, 6);
        face_attacker(g, p);
    }),
    (0x3a5d4, |g, p| {
        if animation::step(g, p) {
            next_step(g, p);
            p.set_u16(g, Obj::VZ, 0x280);
            p.set_u16(g, Obj::AZ, (-0x40i16) as u16);
            hurt_animation(g, p, 8);
        }
    }),
    (0x3a5f6, |g, p| {
        if animation::step(g, p) {
            next_step(g, p);
            p.set_u8(g, Player::TIMER_6F, 0x14);
            p.set_u16(g, Obj::VX, 0x400);
            p.set_u16(g, Obj::AX, (-0x40i16) as u16);
            if p.faces_left(g) {
                let v = p.u32(g, Obj::VX).wrapping_neg();
                p.set_u32(g, Obj::VX, v);
            }
            hurt_animation(g, p, 0xf);
        }
    }),
];

/// Step 0 of reactions 0x26, 0x2e and 0x30: hit again during the reaction (`+0x185`), reaction
/// `action` restarts from its beginning (true); otherwise the mark is set, and with no HP the player falls (true).
fn rehit(g: &mut Ctx, p: Player, action: u8, reaction: u32) -> bool {
    if p.u8(g, REHIT) != 0 {
        p.set_u8(g, REHIT, 0);
        p.set_u8(g, Player::ACTION, action);
        run_at(g, p, reaction);
        return true;
    }
    p.set_u8(g, REHIT, 1);
    if is_dead(g, p) {
        p.set_u16(g, Player::MAIN, 0x0400);
        p.set_u8(g, Player::ACTIVE, 2);
        p.set_u8(g, Player::HITSTOP, 1);
        run_at(g, p, 0x3b862);
        return true;
    }
    false
}

/// Dust of a slide (0xb10c8, variant and setting 0, speeds 0 and -1): the speeds stay in D2 and D3.w, leftovers of the
/// original.
fn shock_dust(g: &mut Ctx, p: Player) {
    g.leftovers.d2 = 0;
    crate::mem::set_low_word(&mut g.leftovers.d3, 0xffff);
    g.shock(p, 0, 0, 0, 0xffff);
}

/// "Hit during the reaction" mark (cleared at the end of the reaction, `recover`).
const REHIT: u32 = 0x185;

/// Speeds of a throw without touching the hit freeze (`+0x53`).
fn set_launch_keep(g: &mut Ctx, p: Player, vx: u16, vz: u16) {
    p.set_u16(g, Obj::VX, vx);
    p.set_u16(g, Obj::AX, 0);
    p.set_u16(g, Obj::VZ, vz);
    p.set_u16(g, Obj::AZ, (-0x40i16) as u16);
}

/// 0x3a98e (reaction 0x14, also taken by reactions 0 and 10 in slow walk): trembling recoil
/// (`flinch`); with no HP, fall (0x3b8a6).
fn hurt_slow(g: &mut Ctx, p: Player) {
    if p.u8(g, Player::STEP) != 0 {
        return flinch(g, p);
    }
    if is_dead(g, p) {
        p.set_u16(g, Player::MAIN, 0x0400);
        p.set_u8(g, Player::ACTIVE, 2);
        g.sound(rom::DEATH_EFFECT, p);
        return run_at(g, p, 0x3b8a6);
    }
    next_step(g, p);
    p.set_u16(g, Obj::VX, 0x280);
    p.set_u16(g, Obj::AX, (-0x50i16) as u16);
    let freeze = p.u8(g, Player::HIT_FREEZE);
    p.set_u8(g, Player::HITSTOP, freeze);
    p.set_u8(g, Player::TIMER_6F, 8);
    face_attacker(g, p);
    hurt_animation(g, p, 2);
}

/// 0x3b2c2 (reaction 0x2a): short shake in place, then return to the pose; with no HP, fall (0x3b8c4).
fn hurt_short(g: &mut Ctx, p: Player) {
    if p.u8(g, Player::STEP) != 0 {
        if animation::step(g, p) {
            recover(g, p);
        }
        return;
    }
    if is_dead(g, p) {
        p.set_u16(g, Player::MAIN, 0x0400);
        p.set_u8(g, Player::ACTIVE, 2);
        p.set_u8(g, Player::HITSTOP, 1);
        return run_at(g, p, 0x3b8c4);
    }
    next_step(g, p);
    let freeze = p.u8(g, Player::HIT_FREEZE);
    p.set_u8(g, Player::HITSTOP, freeze);
    p.set_u8(g, Player::TIMER_6F, 8);
    face_attacker(g, p);
    hurt_animation(g, p, 0);
}

/// Reactions that dispatch their steps through a table (`+7` -> offset relative to the table): address of the
/// reaction, address of its table. The steps shared between reactions are thus reached through `HANDLERS`.
const STEP_TABLES: &[(u32, u32)] = &[
    (0x3a1f0, 0x3a1fc),
    (0x3a274, 0x3a280),
    (0x3a578, 0x3a584),
    (0x3a62a, 0x3a636),
    (0x3a7f6, 0x3a802),
    (0x3a86e, 0x3a87a),
    (0x3a890, 0x3a89c),
    (0x3a8b2, 0x3a8be),
    (0x3a9da, 0x3a9e6),
    (0x3a9f8, 0x3aa04),
    (0x3ab16, 0x3ab22),
    (0x3ab52, 0x3ab5e),
    (0x3ad96, 0x3ada2),
    (0x3ae8a, 0x3ae96),
    (0x3af44, 0x3af50),
    (0x3b0cc, 0x3b0d8),
    (0x3b160, 0x3b16c),
    (0x3b1e4, 0x3b1f0),
    (0x3b30e, 0x3b31a),
    (0x3b4c4, 0x3b4d0),
    (0x3b570, 0x3b57c),
    (0x3b66e, 0x3b67a),
    (0x3b72c, 0x3b738),
    (0x3b7ba, 0x3b7c6),
    (0x3b8a6, 0x3b8b2),
    (0x3b8c4, 0x3b8d0),
];

/// Empty step (a simple RTS), shared by several reactions.
const IDLE_STEP: u32 = 0x3a1a6;

/// Runs the original routine at `addr`, rewritten if possible.
pub fn run_at(g: &mut Ctx, p: Player, addr: u32) {
    cov!();
    if let Some(&(_, f)) = HANDLERS.iter().find(|h| h.0 == addr) {
        f(g, p);
    } else if let Some(&(_, table)) = STEP_TABLES.iter().find(|t| t.0 == addr) {
        jump(g, p, table);
    } else if addr != IDLE_STEP {
        // no native at the addresses of these tables: the game without 68000 already stopped there
        // (`docs/code-mort.md`, "Sortie du Cpu, partie 6")
        unreachable!("reaction step {addr:06x} not rewritten (tables 0x3a0f4, 0x3b820 and their step tables)");
    }
}

/// Jump by offset table: entry `index` of table `base` gives the address of what follows. Like
/// the original, what follows finds `index` and the offset (`Ctx::jumped`, D0.b and D1.w).
fn jump_table(g: &mut Ctx, p: Player, base: u32, index: u8) {
    cov!();
    let offset = pc16(g.b, base + index as u32);
    g.jumped = super::Jumped { index, offset };
    run_at(g, p, base.wrapping_add(offset as i16 as i32 as u32));
}

/// Stepped reaction: dispatches according to `+7`.
fn jump(g: &mut Ctx, p: Player, base: u32) {
    cov!();
    let step = p.u8(g, Player::STEP);
    jump_table(g, p, base, step);
}

// ---- common tools -------------------------------------------------------------------------

/// 0x3bb16: starts the character's reaction animation `n`.
pub fn hurt_animation(g: &mut Ctx, p: Player, n: u16) {
    cov!();
    let character = p.u8(g, Player::CHARACTER) as u32;
    let set = player_data().hurt_animations.long_at(g.b, 4 * character);
    let script = script_of(g.b, set, n);
    load_entry(g.b, p.obj(), script, true);
}

/// 0x3b93c: the player turns to face the aggressor (`+0x56` = aggressor's orientation,
/// inverted) and their horizontal speed inverts accordingly.
fn face_attacker(g: &mut Ctx, p: Player) {
    cov!();
    let facing = p.u8(g, Player::ATTACKER_FACING) ^ 0x20;
    p.set_u8(g, Player::FACING, facing);
    if facing != 0 {
        let vx = p.u32(g, Obj::VX).wrapping_neg();
        p.set_u32(g, Obj::VX, vx);
    }
}

/// Has the player no hit points left?
fn is_dead(g: &mut Ctx, p: Player) -> bool {
    cov!();
    (p.u16(g, Player::HP) as i16) <= 0
}

/// 0x3b91c: if they have no hit points left, the player goes to the ground (main state 4).
fn knock_down_if_dead(g: &mut Ctx, p: Player) {
    cov!();
    if is_dead(g, p) {
        knock_down(g, p);
    }
}

fn knock_down(g: &mut Ctx, p: Player) {
    cov!();
    p.set_u16(g, Player::MAIN, 0x0400);
    p.set_u8(g, Player::HITSTOP, 1);
    p.set_u8(g, Player::ACTIVE, 2);
    g.sound(rom::DEATH_EFFECT, p);
}

/// 0x3a1e2: end of the reaction, return to the standing state.
fn recover(g: &mut Ctx, p: Player) {
    cov!();
    p.set_u8(g, Player::ACTIVE, 1);
    p.set_u8(g, REHIT, 0);
    to_stand(g, p);
}

/// Sets the speeds of a throw (vx, vz, accelerations), stores the hit freeze (`+0x57`).
fn set_launch(g: &mut Ctx, p: Player, vx: u16, vz: u16) {
    cov!();
    p.set_u16(g, Obj::VX, vx);
    p.set_u16(g, Obj::AX, 0);
    p.set_u16(g, Obj::VZ, vz);
    p.set_u16(g, Obj::AZ, (-0x40i16) as u16);
    let freeze = p.u8(g, Player::HIT_FREEZE);
    p.set_u8(g, Player::HITSTOP, freeze);
}

fn next_step(g: &mut Ctx, p: Player) {
    cov!();
    let step = p.u8(g, Player::STEP).wrapping_add(2);
    p.set_u8(g, Player::STEP, step);
}

// ---- handlers by sub-state ---------------------------------------------------------------

/// 0x3a0a8: player hit (`+5` = 4).
pub fn hurt(g: &mut Ctx, p: Player) {
    cov!();
    if p.u8(g, Player::STEP) == 0 {
        g.sound(rom::BEFORE_HURT_A, p);
        crate::families::items::fall_equipment(&mut g.leftovers, g.b, p.obj());
        if p.u8(g, Player::SLOW_8A) != 0 {
            p.set_u8(g, Player::SLOW_8A, 0);
            let held = p.u32(g, Player::LINK);
            wr8(g.b, held + 5, 8);
            wr16(g.b, held + 6, 0);
        }
    }
    let action = p.u8(g, Player::ACTION);
    jump_table(g, p, HURT_ACTIONS, action);
    if ram::INPUT_RAW.get(g.b) == 0 {
        crate::terrain::push(g.b, p.obj());
    }
    clamp_to_screen(g.b, p);
}

/// 0x3b7f4: player on the ground (`+4` = 4).
pub fn downed(g: &mut Ctx, p: Player) {
    cov!();
    p.set_u8(g, Player::GET_UP_MARK, 0);
    p.set_u16(g, Player::DYING_TIMER, 0);
    let action = p.u8(g, Player::ACTION);
    jump_table(g, p, DOWN_ACTIONS, action);
    crate::terrain::push(g.b, p.obj());
    if p.u8(g, Player::ACTIVE) != 0 {
        ACTORS.reinsert(g.b, p.obj());
    }
    clamp_to_screen(g.b, p);
}
