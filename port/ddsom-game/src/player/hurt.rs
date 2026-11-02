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

/// Reaction animations (sets of long entries per character, read as data).
const HURT_ANIMATIONS: u32 = 0x3bb52;

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
        if p.u8(g, 0x33) != 0 {
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
        if ended || (p.u8(g, Player::SLOW) != 0 && p.u8(g, 0x33) != 0) {
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
        if p.u8(g, 0x33) == 0 {
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
        let freeze = p.u8(g, 0x57);
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
    let freeze = p.u8(g, 0x57);
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
    let freeze = p.u8(g, 0x57);
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
    let offset = g.prog16(base + index as u32);
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
    let set = g.data32(HURT_ANIMATIONS + 4 * character);
    let script = script_of(g.b, set, n);
    load_entry(g.b, p.obj(), script, true);
}

/// 0x3b93c: the player turns to face the aggressor (`+0x56` = aggressor's orientation,
/// inverted) and their horizontal speed inverts accordingly.
fn face_attacker(g: &mut Ctx, p: Player) {
    cov!();
    let facing = p.u8(g, 0x56) ^ 0x20;
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
    let freeze = p.u8(g, 0x57);
    p.set_u8(g, Player::HITSTOP, freeze);
}

fn next_step(g: &mut Ctx, p: Player) {
    cov!();
    let step = p.u8(g, Player::STEP).wrapping_add(2);
    p.set_u8(g, Player::STEP, step);
}

// ---- simple reactions ------------------------------------------------------------------------

/// 0x3a17e: recoil slide. As long as `+0x6f` lasts, the player trembles by 2 pixels; then
/// the animation plays and the horizontal speed makes them slide.
fn flinch(g: &mut Ctx, p: Player) {
    cov!();
    let left = p.u8(g, Player::TIMER_6F);
    if left != 0 {
        let mut shift = 2i16;
        if p.faces_left(g) {
            shift = -shift;
        }
        p.set_u8(g, Player::TIMER_6F, left - 1);
        if (left - 1) & 1 == 0 {
            shift = -shift;
        }
        let x = p.u16(g, 8).wrapping_add(shift as u16);
        p.set_u16(g, 8, x);
        return;
    }
    if animation::step(g, p) {
        return recover(g, p);
    }
    if p.u8(g, Obj::VX) != 0 {
        move_x(g, p);
    }
}

/// Common start of reactions 0 and 10: recoil with animation `anim`.
fn start_flinch(g: &mut Ctx, p: Player, anim: u16) {
    cov!();
    next_step(g, p);
    p.set_u16(g, Obj::VX, 0x280);
    p.set_u16(g, Obj::AX, (-0x50i16) as u16);
    let freeze = p.u8(g, 0x57);
    p.set_u8(g, Player::HITSTOP, freeze);
    p.set_u8(g, Player::TIMER_6F, 8);
    face_attacker(g, p);
    hurt_animation(g, p, anim);
}

/// 0x3a136: simple recoil.
fn reaction_0(g: &mut Ctx, p: Player) {
    cov!();
    if p.u8(g, Player::STEP) != 0 {
        return flinch(g, p);
    }
    if p.u8(g, Player::SLOW) != 0 {
        p.set_u8(g, Player::ACTION, 0x14);
        return hurt_slow(g, p);
    }
    if is_dead(g, p) {
        // 0x3b8f6: on the ground right away, and the "on the ground" handler runs immediately after
        knock_down(g, p);
        return downed(g, p);
    }
    start_flinch(g, p, 0);
}

/// 0x3a79e: recoil with animation 1.
fn reaction_10(g: &mut Ctx, p: Player) {
    cov!();
    if p.u8(g, Player::STEP) != 0 {
        return flinch(g, p);
    }
    if p.u8(g, Player::SLOW) != 0 {
        p.set_u8(g, Player::ACTION, 0x14);
        return hurt_slow(g, p);
    }
    if is_dead(g, p) {
        p.set_u16(g, Player::MAIN, 0x0400);
        p.set_u8(g, Player::ACTIVE, 2);
        return jump(g, p, steps::DOWN_A);
    }
    start_flinch(g, p, 1);
}

// ---- airborne throws (shared steps) --------------------------------------------------------

/// Step 0 of the throws: animation, then speed (vx 0x200, vz 0x480).
fn launch_with_anim(g: &mut Ctx, p: Player, anim: u16) {
    cov!();
    hurt_animation(g, p, anim);
    next_step(g, p);
    set_launch(g, p, 0x200, 0x480);
    face_attacker(g, p);
    knock_down_if_dead(g, p);
}

/// 0x3a2ae: throw (vx 0x180, vz 0x400), animation 0xa.
fn launch_a(g: &mut Ctx, p: Player) {
    cov!();
    hurt_animation(g, p, 0xa);
    launch_common(g, p);
}

// ---- steps reached by no scenario (reactions 0x3a578, 0x3a890, 0x3a9da, 0x3a9f8, 0x3af44, 0x3b570, 0x3b72c, 0x3b7ba,
// on the ground 0x3b8a6 and 0x3b8c4): transcribed from the original, validated by `fuzz_couv` (players_update) ----

/// 0x3a59a, 0x3a5a4: high throw (vx 0x280, vz 0x500) with animation `anim`; with no HP, to the ground.
fn launch_high(g: &mut Ctx, p: Player, anim: u16) {
    cov!();
    hurt_animation(g, p, anim);
    next_step(g, p);
    set_launch(g, p, 0x280, 0x500);
    face_attacker(g, p);
    knock_down_if_dead(g, p);
}

/// 0x3aa2e: the slow walk ends, throw (vx 0x200, vz 0x480), animation 0xe.
fn launch_slow(g: &mut Ctx, p: Player) {
    cov!();
    p.set_u8(g, Player::SLOW, 0);
    hurt_animation(g, p, 0xe);
    next_step(g, p);
    set_launch(g, p, 0x200, 0x480);
    face_attacker(g, p);
}

/// 0x3aff8: slide on the ground (dust every other frame), stopped by an obstacle; at rest, next step and 10 frames.
fn slide_to_rest(g: &mut Ctx, p: Player) {
    cov!();
    move_x(g, p);
    if p.u16(g, Obj::VX) != 0 && ram::FRAME_COUNTER.get(g.b) & 1 != 0 {
        shock_dust(g, p);
    }
    stop_at_wall(g, p);
    if p.u8(g, Obj::VX) == 0 {
        next_step(g, p);
        p.set_u8(g, Player::TIMER_6F, 0xa);
    }
}

/// 0x3b592: stars (0xb378a, variant 0, setting 0), 20 frames of shaking (animation 0).
fn stars_start(g: &mut Ctx, p: Player) {
    cov!();
    crate::families::effects_creators::for_players::blow(&mut g.leftovers, g.b, p.obj(), 0, 0);
    next_step(g, p);
    p.set_u8(g, Player::TIMER_6F, 0x14);
    p.set_u8(g, Player::HITSTOP, 1);
    hurt_animation(g, p, 0);
}

/// 0x3b5b2: the player shakes one pixel left and right; at the end of the count they lose 16 HP and are thrown
/// (dead: to the ground), or, without altitude and still alive, slide backward (vx 0x280, deceleration 0x50).
fn stars_shake(g: &mut Ctx, p: Player) {
    cov!();
    if p.u8(g, Player::TIMER_6F) == 0 {
        return stars_end(g, p);
    }
    let left = p.u8(g, Player::TIMER_6F).wrapping_sub(1);
    p.set_u8(g, Player::TIMER_6F, left);
    if (left as i8) > 0 {
        let mut dx: u16 = 1;
        if left & 1 != 0 {
            dx = dx.wrapping_neg();
        }
        if p.faces_left(g) {
            dx = dx.wrapping_neg();
        }
        let x = p.u16(g, Obj::X).wrapping_add(dx);
        p.set_u16(g, Obj::X, x);
        return;
    }
    next_step(g, p);
    let hp = p.u16(g, Player::HP).wrapping_sub(0x10);
    p.set_u16(g, Player::HP, hp);
    if (hp as i16) <= 0 {
        p.set_u16(g, Player::HP, 0);
        g.sound(rom::DEATH_EFFECT, p);
        let alive = ram::PLAYERS_ALIVE.get(g.b).wrapping_sub(1);
        ram::PLAYERS_ALIVE.set(g.b, alive);
        p.set_u8(g, Player::ACTIVE, 2);
        p.set_u16(g, Player::MAIN, 0x0400);
        return stars_thrown(g, p);
    }
    if (p.u16(g, Obj::Z) as i16) > 0 {
        return stars_thrown(g, p);
    }
    p.set_u16(g, Obj::VX, 0x280);
    p.set_u16(g, Obj::AX, (-0x50i16) as u16);
    reverse_if_facing_left(g, p);
}

/// 0x3b5e8: throw (vx 0x200, vz 0x480), animation 0x16 or 0x17 every other frame.
fn stars_thrown(g: &mut Ctx, p: Player) {
    cov!();
    next_step(g, p);
    p.set_u8(g, Player::HITSTOP, 1);
    p.set_u16(g, Obj::VX, 0x200);
    p.set_u16(g, Obj::AX, 0);
    p.set_u16(g, Obj::VZ, 0x480);
    p.set_u16(g, Obj::AZ, (-0x40i16) as u16);
    p.set_u32(g, Obj::VY, 0);
    let anim = if ram::FRAME_COUNTER.get(g.b) & 1 != 0 { 0x17 } else { 0x16 };
    hurt_animation(g, p, anim);
    reverse_if_facing_left(g, p);
}

/// 0x3b656: end of the animation: back to the standing state; otherwise the slide goes on while there is speed.
fn stars_end(g: &mut Ctx, p: Player) {
    cov!();
    if animation::step(g, p) {
        return recover(g, p);
    }
    if p.u8(g, Obj::VX) != 0 {
        move_x(g, p);
    }
}

/// 0x3b952: the horizontal speed (and acceleration) reversed for a player facing left.
fn reverse_if_facing_left(g: &mut Ctx, p: Player) {
    cov!();
    if p.faces_left(g) {
        let v = p.u32(g, Obj::VX).wrapping_neg();
        p.set_u32(g, Obj::VX, v);
    }
}

/// 0x3b740: 240 frames; airborne, the player falls straight down (animation 3), on the ground they kneel at once.
fn kneel_start(g: &mut Ctx, p: Player) {
    cov!();
    next_step(g, p);
    p.set_u8(g, Player::TIMER_6F, 0xf0);
    if p.u16(g, Obj::Z) == 0 {
        return kneel_stars(g, p);
    }
    p.set_u32(g, Obj::VX, 0);
    p.set_u16(g, Obj::VZ, 0);
    p.set_u16(g, Obj::AZ, (-0x40i16) as u16);
    hurt_animation(g, p, 3);
}

/// 0x3b766: kneeling (animation 0x11) under stars: up to six (0xb378a, variant 2, settings 5 down to 0); a full pool
/// skips a step.
fn kneel_stars(g: &mut Ctx, p: Player) {
    cov!();
    next_step(g, p);
    hurt_animation(g, p, 0x11);
    for setting in (0..=5).rev() {
        if crate::families::effects_creators::for_players::blow(&mut g.leftovers, g.b, p.obj(), 2, setting).is_none() {
            next_step(g, p);
            return;
        }
    }
}

/// 0x3b78a: falling straight down; on the ground, kneeling (`kneel_stars`).
fn kneel_fall(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    move_xz(g, p);
    if (p.u16(g, Obj::Z) as i16) > 0 {
        return;
    }
    p.set_u16(g, Obj::Z, 0);
    kneel_stars(g, p);
}

/// 0x3a2b4: throw (vx 0x180, vz 0x400).
fn launch_common(g: &mut Ctx, p: Player) {
    cov!();
    next_step(g, p);
    set_launch(g, p, 0x180, 0x400);
    face_attacker(g, p);
    knock_down_if_dead(g, p);
}

/// 0x3aaac: throw (vx 0x200, vz 0x200), animation 0x16 or 0x17 every other frame.
fn launch_b(g: &mut Ctx, p: Player) {
    cov!();
    next_step(g, p);
    set_launch(g, p, 0x200, 0x200);
    face_attacker(g, p);
    let anim = if ram::FRAME_COUNTER.get(g.b) & 1 != 0 { 0x17 } else { 0x16 };
    hurt_animation(g, p, anim);
    knock_down_if_dead(g, p);
}

/// 0x3ad4c: throw like `launch_common`, then depth speed (`VY` 0x80), animation 0x12.
fn launch_c(g: &mut Ctx, p: Player) {
    cov!();
    launch_common(g, p);
    p.set_u16(g, Obj::VY, 0x80);
    p.set_u16(g, Obj::AY, 0);
    p.set_u8(g, Player::HITSTOP, 1);
    hurt_animation(g, p, 0x12);
}

/// Contact with an obstacle (`+0x29`): the horizontal speed is cancelled.
fn stop_at_wall(g: &mut Ctx, p: Player) {
    cov!();
    if p.u8(g, Player::BLOCKED) != 0 {
        p.set_u32(g, Obj::VX, 0);
    }
}

/// Altitude (high word of `Z`, signed).
fn altitude(g: &mut Ctx, p: Player) -> i16 {
    cov!();
    p.u16(g, Obj::Z) as i16
}

/// 0x3a2e6: waits for the end of the animation, then chains animation `anim`.
fn next_animation(g: &mut Ctx, p: Player, anim: u16) {
    cov!();
    if animation::step(g, p) {
        next_step(g, p);
        hurt_animation(g, p, anim);
    }
}

/// 0x3a300: airborne, up to the peak (vertical speed turned negative).
fn fall_to_apex(g: &mut Ctx, p: Player, anim: u16) {
    cov!();
    animation::step(g, p);
    move_xz(g, p);
    stop_at_wall(g, p);
    if (p.u16(g, Obj::VZ) as i16) >= 0 {
        return;
    }
    next_step(g, p);
    hurt_animation(g, p, anim);
}

/// 0x3a330: falls back to the ground.
fn land(g: &mut Ctx, p: Player, anim: u16) {
    cov!();
    animation::step(g, p);
    move_xz(g, p);
    stop_at_wall(g, p);
    if altitude(g, p) >= 0 {
        return;
    }
    p.set_u32(g, Obj::Z, 0);
    next_step(g, p);
    hurt_animation(g, p, anim);
    g.sound(crate::sound::LANDING_EFFECT, p);
}

/// 0x3a522: end of the fall, small bounce (vz 0x240).
fn bounce_up(g: &mut Ctx, p: Player) {
    cov!();
    if animation::step(g, p) {
        next_step(g, p);
        p.set_u16(g, Obj::VZ, 0x240);
        p.set_u16(g, Obj::AZ, (-0x40i16) as u16);
        hurt_animation(g, p, 8);
    }
}

/// 0x3a362: higher bounce (vz 0x1c0).
fn bounce_up_small(g: &mut Ctx, p: Player) {
    cov!();
    if animation::step(g, p) {
        next_step(g, p);
        p.set_u16(g, Obj::VZ, 0x1c0);
        p.set_u16(g, Obj::AZ, (-0x40i16) as u16);
        hurt_animation(g, p, 8);
    }
}

/// 0x3a384: falls back from the bounce.
fn land_with_effect(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    move_xz(g, p);
    if altitude(g, p) >= 0 {
        return;
    }
    p.set_u32(g, Obj::Z, 0);
    next_step(g, p);
    hurt_animation(g, p, 9);
    g.sound(crate::sound::LANDING_EFFECT, p);
}

/// 0x3a544: start of the slide on the ground (vx 0x300 in the recoil direction).
fn slide_start(g: &mut Ctx, p: Player) {
    cov!();
    start_slide(g, p, 0x300);
}

/// 0x3a3ae: same with vx 0x280.
fn slide_start_short(g: &mut Ctx, p: Player) {
    cov!();
    start_slide(g, p, 0x280);
}

fn start_slide(g: &mut Ctx, p: Player, vx: u16) {
    cov!();
    if !animation::step(g, p) {
        return;
    }
    next_step(g, p);
    p.set_u8(g, Player::TIMER_6F, 0x14);
    p.set_u16(g, Obj::VX, vx);
    p.set_u16(g, Obj::AX, (-0x40i16) as u16);
    if p.faces_left(g) {
        // `neg.l` on the (speed, acceleration) pair: both words are negated together
        let both = p.u32(g, Obj::VX).wrapping_neg();
        p.set_u32(g, Obj::VX, both);
    }
    hurt_animation(g, p, 0xf);
}

/// 0x3a3e2: slide; dust every other frame; stops when the speed drops to zero.
fn slide(g: &mut Ctx, p: Player) {
    cov!();
    move_x(g, p);
    if p.u16(g, Obj::VX) != 0 && ram::FRAME_COUNTER.get(g.b) & 1 != 0 {
        shock_dust(g, p);
    }
    stop_at_wall(g, p);
    if p.u8(g, Obj::VX) == 0 {
        next_step(g, p);
    }
}

/// 0x3b960: scripted fall (cinematic 0x2447e): the player is thrown without losing life (HP kept in
/// `+0x64`, brought back to 1), rises, falls back, bounces, lands then slides; at rest, next action (`+6`).
/// The odd variants of the character (`+2`) take the animations of the second series (+4).
pub fn fall(g: &mut Ctx, p: Player) {
    cov!();
    let second = p.u8(g, Player::INDEX) & 1 != 0;
    let anim = |n: u16| if second { n + 4 } else { n };
    match p.u8(g, Player::STEP) {
        0 => {
            hurt_animation(g, p, anim(6));
            let hp = p.u16(g, Player::HP);
            p.set_u16(g, Player::HP_PREV, hp);
            p.set_u16(g, Player::HP, 1);
            next_step(g, p);
            for (field, v) in [(Obj::VX, 0x180), (Obj::AX, 0), (Obj::VZ, 0x400), (Obj::AZ, 0xffc0), (Obj::VY, 0xffc0), (Obj::AY, 0)] {
                p.set_u16(g, field, v);
            }
            p.set_u8(g, 0x53, 0);
            if p.faces_left(g) {
                let both = p.u32(g, Obj::VX).wrapping_neg();
                p.set_u32(g, Obj::VX, both);
            }
            g.sound(rom::BEFORE_HURT_A, p);
        }
        2 => next_animation(g, p, anim(7)),
        4 => fall_to_apex_xyz(g, p, anim(8)),
        6 => fall_to_apex_xyz(g, p, anim(9)),
        8 => {
            if animation::step(g, p) {
                next_step(g, p);
                p.set_u16(g, Obj::VZ, 0x1c0);
                p.set_u16(g, Obj::AZ, 0xffc0);
                hurt_animation(g, p, 8);
            }
        }
        10 => {
            animation::step(g, p);
            move_xyz(g, p);
            if altitude(g, p) < 0 {
                p.set_u32(g, Obj::Z, 0);
                next_step(g, p);
                hurt_animation(g, p, 9);
                g.sound(crate::sound::LANDING_EFFECT, p);
            }
        }
        12 => {
            if animation::step(g, p) {
                next_step(g, p);
                p.set_u16(g, Obj::VX, 0x280);
                p.set_u16(g, Obj::AX, 0xffc0);
                if p.faces_left(g) {
                    let both = p.u32(g, Obj::VX).wrapping_neg();
                    p.set_u32(g, Obj::VX, both);
                }
                hurt_animation(g, p, 15);
            }
        }
        _ => {
            move_x(g, p);
            if p.u16(g, Obj::VX) != 0 && ram::FRAME_COUNTER.get(g.b) & 1 != 0 {
                shock_dust(g, p);
            }
            stop_xy_at_wall(g, p);
            if p.u8(g, Obj::VX) == 0 {
                let action = p.u8(g, Player::ACTION).wrapping_add(2);
                p.set_u8(g, Player::ACTION, action);
                p.set_u8(g, Player::STEP, 0);
            }
        }
    }
}

/// 0x3b9ec: airborne (movement on the three axes), up to the peak.
fn fall_to_apex_xyz(g: &mut Ctx, p: Player, anim: u16) {
    cov!();
    animation::step(g, p);
    move_xyz(g, p);
    stop_xy_at_wall(g, p);
    if (p.u16(g, Obj::VZ) as i16) < 0 {
        next_step(g, p);
        hurt_animation(g, p, anim);
    }
}

/// Against a wall (`+0x29`), no more speed in x or in depth.
fn stop_xy_at_wall(g: &mut Ctx, p: Player) {
    if p.u8(g, Player::BLOCKED) != 0 {
        p.set_u32(g, Obj::VX, 0);
        p.set_u32(g, Obj::VY, 0);
    }
}

/// 0x3a424: waits `+0x6f` frames on the ground then gets up (animation 0x10 and sound).
fn get_up(g: &mut Ctx, p: Player) {
    cov!();
    if p.u8(g, Player::MAIN) == 4 {
        // death: the player stays on the ground for a while, then the resurrection blinking begins
        let step = p.u8(g, Player::STEP).wrapping_add(4);
        p.set_u8(g, Player::STEP, step);
        p.set_u8(g, Player::TIMER_6F, 0x3c);
        return invulnerable_blink(g, p);
    }
    let left = p.u8(g, Player::TIMER_6F).wrapping_sub(1);
    p.set_u8(g, Player::TIMER_6F, left);
    if left != 0 {
        return;
    }
    next_step(g, p);
    hurt_animation(g, p, 0x10);
    let sound = if ram::FRAME_COUNTER.get(g.b) & 2 != 0 { rom::GET_UP_SOUND_B } else { rom::GET_UP_SOUND_A };
    g.sound(sound, p);
}

/// 0x3a466: end of the get-up animation: return to the standing state.
fn finish_get_up(g: &mut Ctx, p: Player) {
    cov!();
    if animation::step(g, p) {
        recover(g, p);
    }
}

/// 0x3a472: invulnerability blinking for `+0x6f` frames (the player is hidden one
/// frame out of four in the last 40); at the end, the status effects are cleared.
fn invulnerable_blink(g: &mut Ctx, p: Player) {
    cov!();
    let flags = p.u8(g, Player::FLAGS) & !2;
    p.set_u8(g, Player::FLAGS, flags);
    let left = p.u8(g, Player::TIMER_6F).wrapping_sub(1);
    p.set_u8(g, Player::TIMER_6F, left);
    if left == 0 {
        let sub = p.u8(g, Player::SUB).wrapping_add(2);
        p.set_u8(g, Player::SUB, sub);
        p.set_u16(g, Player::ACTION, 0);
        for field in [Player::AURA, Player::AURA_B, Player::AURA_C, Player::POWERUP] {
            p.set_u8(g, field, 0);
        }
        p.set_u16(g, Player::AURA_TIMER, 0);
        p.set_u8(g, Player::SLOW, 0);
        return;
    }
    if (left as i8) > 0x28 || left & 3 == 0 {
        return;
    }
    let flags = p.u8(g, Player::FLAGS) | 2;
    p.set_u8(g, Player::FLAGS, flags);
}

/// 0x3ad66: throw with slide in depth.
fn fall_c(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    move_xyz(g, p);
    stop_at_wall(g, p);
    if altitude(g, p) > 0 {
        return;
    }
    p.set_u32(g, Obj::Z, 0);
    next_step(g, p);
    hurt_animation(g, p, 0xd);
}

/// 0x3aae8: throw (reaction 26): falls back.
fn fall_b(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    move_xz(g, p);
    stop_at_wall(g, p);
    if altitude(g, p) > 0 {
        return;
    }
    p.set_u32(g, Obj::Z, 0);
    next_step(g, p);
    hurt_animation(g, p, 9);
}

// ---- reaction 0x34 (0x3b4c4): violent fall ------------------------------------------------------

/// 0x3b504: on landing, the screen shakes (0x2824, D0 = 10) and the player loses `+0x6f` HP (outside
/// cinematics); at zero they die.
fn crash_landing(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    move_xz(g, p);
    stop_at_wall(g, p);
    if altitude(g, p) > 0 {
        return;
    }
    crate::camera_modes::start_shake(g.b, crate::camera_modes::shake::A, 10);
    p.set_u32(g, Obj::Z, 0);
    if ram::FROZEN.get(g.b) == 0 {
        let hp = p.u16(g, Player::HP).wrapping_sub(p.u8(g, Player::TIMER_6F) as u16);
        p.set_u16(g, Player::HP, hp);
        if (hp as i16) <= 0 {
            p.set_u8(g, Player::ACTIVE, 2);
            p.set_u16(g, Player::HP, 0);
            p.set_u16(g, Player::MAIN, 0x0400);
            g.sound(rom::DEATH_EFFECT, p);
            let alive = ram::PLAYERS_ALIVE.get(g.b).wrapping_sub(1);
            ram::PLAYERS_ALIVE.set(g.b, alive);
        }
    }
    next_step(g, p);
    hurt_animation(g, p, 9);
    g.sound(crate::sound::LANDING_EFFECT, p);
}

// ---- reaction 0x3e (0x3a62a): thrown far away -------------------------------------------------------

/// The thrown player has left the screen area (more than 320 pixels after the left edge + 32) or
/// hit an obstacle.
fn hurled_out(g: &mut Ctx, p: Player) -> bool {
    cov!();
    if p.u8(g, Player::BLOCKED) != 0 {
        return true;
    }
    let camera = crate::objects::cull::camera(g.b);
    let gap = p.u16(g, Obj::X).wrapping_sub(crate::mem::rd16(g.b, camera + 8)).wrapping_sub(32);
    gap > 320
}

/// 0x3a656, 0x3ab7c: throw along the ground (vx 0x800, vertical speed `vz`, 18 frames), animation `anim`.
fn hurl(g: &mut Ctx, p: Player, vz: u16, anim: u16) {
    cov!();
    next_step(g, p);
    p.set_u16(g, Obj::VX, 0x800);
    p.set_u16(g, Obj::AX, 0);
    p.set_u16(g, Obj::VZ, vz);
    p.set_u16(g, Obj::AZ, (-4i16) as u16);
    let freeze = p.u8(g, 0x57);
    p.set_u8(g, Player::HITSTOP, freeze);
    face_attacker(g, p);
    p.set_u8(g, Player::TIMER_6F, 18);
    hurt_animation(g, p, anim);
    knock_down_if_dead(g, p);
}

/// 0x3a68a, 0x3abb0: low flight (18 frames), then braking (animation `brake`); an obstacle or the edge of
/// the screen stops the flight dead (fall at step `out`, animation 25).
fn hurled(g: &mut Ctx, p: Player, brake: u16, out: u8) {
    cov!();
    animation::step(g, p);
    move_xz(g, p);
    let left = p.u8(g, Player::TIMER_6F).wrapping_sub(1);
    p.set_u8(g, Player::TIMER_6F, left);
    if left == 0 {
        next_step(g, p);
        p.set_u16(g, Obj::AZ, (-64i16) as u16);
        let ax: u16 = if p.faces_left(g) { 32 } else { (-32i16) as u16 };
        p.set_u16(g, Obj::AX, ax);
        return hurt_animation(g, p, brake);
    }
    if !hurled_out(g, p) {
        return;
    }
    p.set_u32(g, Obj::VX, 0);
    p.set_u16(g, Obj::VZ, 0);
    p.set_u16(g, Obj::AZ, (-64i16) as u16);
    p.set_u8(g, Player::STEP, out);
    hurt_animation(g, p, 25);
}

/// 0x3a706, 0x3ac2c: falls back; on the ground, animation `anim` (and dust if `effect`); an obstacle or the edge
/// stops the slide.
fn hurled_landing(g: &mut Ctx, p: Player, anim: u16, effect: bool) {
    cov!();
    animation::step(g, p);
    move_xz(g, p);
    if altitude(g, p) <= 0 {
        p.set_u32(g, Obj::Z, 0);
        next_step(g, p);
        hurt_animation(g, p, anim);
        if effect {
            g.sound(crate::sound::LANDING_EFFECT, p);
        }
        return;
    }
    if hurled_out(g, p) {
        p.set_u32(g, Obj::VX, 0);
    }
}

/// 0x3a75a, 0x3acba: slide on the ground (dust every other frame); at rest, step `stop`.
fn hurled_slide(g: &mut Ctx, p: Player, stop: u8) {
    cov!();
    move_x(g, p);
    if p.u16(g, Obj::VX) != 0 && ram::FRAME_COUNTER.get(g.b) & 1 != 0 {
        shock_dust(g, p);
    }
    stop_at_wall(g, p);
    if p.u8(g, Obj::VX) == 0 {
        p.set_u8(g, Player::STEP, stop);
    }
}

// ---- reaction 0x22 (0x3ad96): grabbed (h) — thrown, then struggles --------------------------------

/// Change of the held directions since the previous frame (the player "struggles").
fn struggling(g: &mut Ctx, p: Player) -> bool {
    p.u8(g, Player::KEYS_HELD) & 15 != p.u8(g, Player::KEYS_PREV) & 15
}

/// Duration of the hold, counted down by the jolts.
const HOLD: u32 = 0x1f0;

/// 0x3adbe: small throw, palette 1/19, 30 frames; if no HP left, on the ground.
fn shock_start(g: &mut Ctx, p: Player) {
    cov!();
    if is_dead(g, p) {
        knock_down(g, p);
        return downed(g, p);
    }
    next_step(g, p);
    p.set_u8(g, HOLD, 8);
    p.set_u16(g, Obj::VX, 0x200);
    p.set_u16(g, Obj::AX, 0);
    p.set_u16(g, Obj::VZ, 0x400);
    p.set_u16(g, Obj::AZ, (-128i16) as u16);
    face_attacker(g, p);
    p.set_u8(g, Player::PALETTE_GROUP, 1);
    p.set_u8(g, Player::PALETTE, 19);
    p.set_u8(g, Player::TIMER_6F, 30);
    p.set_u8(g, Player::HITSTOP, 1);
    hurt_animation(g, p, 27);
    g.sound(crate::sound::CMD_31E_41E_B, p);
}

/// 0x3ae0e: falls back (without animation).
fn shock_fall(g: &mut Ctx, p: Player) {
    cov!();
    move_xz(g, p);
    stop_at_wall(g, p);
    if altitude(g, p) > 0 {
        return;
    }
    p.set_u32(g, Obj::Z, 0);
    next_step(g, p);
}

/// 0x3ae32: each jolt advances the animation and wears down the hold; hold worn out: released. After
/// `+0x6f` frames, what follows (`shock_release`).
fn shock_struggle(g: &mut Ctx, p: Player) {
    cov!();
    if struggling(g, p) {
        animation::step(g, p);
        let left = p.u8(g, HOLD).wrapping_sub(1);
        p.set_u8(g, HOLD, left);
        if left == 0 {
            crate::families::effects_creators::for_players::debris(&mut g.leftovers, g.b, p.obj(), 1);
            p.set_u16(g, Player::PALETTE_GROUP, 0);
            return recover(g, p);
        }
    }
    let left = p.u8(g, Player::TIMER_6F).wrapping_sub(1);
    p.set_u8(g, Player::TIMER_6F, left);
    if left != 0 {
        return;
    }
    next_step(g, p);
    crate::families::effects_creators::for_players::debris(&mut g.leftovers, g.b, p.obj(), 1);
}

/// 0x3ae7a: palette restored, throw.
fn shock_release(g: &mut Ctx, p: Player) {
    cov!();
    p.set_u16(g, Player::PALETTE_GROUP, 0);
    launch_a(g, p);
    p.set_u8(g, Player::HITSTOP, 1);
}

// ---- reaction 0x24 (0x3ae8a): stunned -----------------------------------------------------------

/// 0x3aeaa: stars, 40 frames of stun.
fn dizzy_start(g: &mut Ctx, p: Player) {
    cov!();
    crate::families::effects_creators::for_players::blow(&mut g.leftovers, g.b, p.obj(), 0, 0);
    next_step(g, p);
    p.set_u8(g, Player::TIMER_6F, 40);
    p.set_u8(g, Player::HITSTOP, 1);
    hurt_animation(g, p, 0);
}

/// 0x3aeca: the player staggers one pixel left and right (sometimes new stars), then falls
/// backward (animation 22 or 23 every other frame).
fn dizzy(g: &mut Ctx, p: Player) {
    cov!();
    let left = p.u8(g, Player::TIMER_6F).wrapping_sub(1);
    p.set_u8(g, Player::TIMER_6F, left);
    if left != 0 {
        if g.random() & 127 == 0 {
            crate::families::effects_creators::for_players::blow(&mut g.leftovers, g.b, p.obj(), 0, 1);
        }
        let mut dx: u16 = 1;
        if left & 1 != 0 {
            dx = dx.wrapping_neg();
        }
        if p.faces_left(g) {
            dx = dx.wrapping_neg();
        }
        let x = p.u16(g, Obj::X).wrapping_add(dx);
        p.set_u16(g, Obj::X, x);
        return;
    }
    next_step(g, p);
    p.set_u16(g, Obj::VX, 0x180);
    p.set_u16(g, Obj::AX, 0);
    p.set_u32(g, Obj::VY, 0);
    p.set_u16(g, Obj::VZ, 0x400);
    p.set_u16(g, Obj::AZ, (-64i16) as u16);
    if p.faces_left(g) {
        // 0x3b952: `neg.l` of the (speed, acceleration) pair
        let pair = p.u32(g, Obj::VX).wrapping_neg();
        p.set_u32(g, Obj::VX, pair);
    }
    let anim = if ram::FRAME_COUNTER.get(g.b) & 1 != 0 { 23 } else { 22 };
    hurt_animation(g, p, anim);
    p.set_u8(g, Player::HITSTOP, 0);
    knock_down_if_dead(g, p);
}

// ---- reaction 0x38 (0x3b66e): knocked over, stars, then gets up ---------------------------------------

/// 0x3b684: with no HP, on the ground right away (0x3b8f6). Otherwise 240 frames of stars (`+0x6f`); airborne, the
/// player falls back (animation 3), on the ground they lie down at once.
fn fallen_start(g: &mut Ctx, p: Player) {
    cov!();
    if is_dead(g, p) {
        knock_down(g, p);
        return downed(g, p);
    }
    next_step(g, p);
    p.set_u8(g, Player::TIMER_6F, 0xf0);
    if p.u16(g, Obj::Z) == 0 {
        return fallen_lie_down(g, p);
    }
    p.set_u32(g, Obj::VX, 0);
    p.set_u16(g, Obj::VZ, 0);
    p.set_u16(g, Obj::AZ, (-0x40i16) as u16);
    hurt_animation(g, p, 3);
}

/// 0x3b6b2: lying down (animation 0x23); slowed (`+0x66`), they go straight to the stars (0x3b6c4).
fn fallen_lie_down(g: &mut Ctx, p: Player) {
    cov!();
    next_step(g, p);
    if p.u8(g, Player::SLOW) != 0 {
        return fallen_stars_start(g, p);
    }
    hurt_animation(g, p, 0x23);
}

/// 0x3b6c4: stars above the lying player (animation 0x1c).
fn fallen_stars_start(g: &mut Ctx, p: Player) {
    cov!();
    p.set_u8(g, Player::ACTIVE, 1);
    p.set_u8(g, Player::INVULN, 0);
    next_step(g, p);
    hurt_animation(g, p, 0x1c);
}

/// 0x3b6da: the fall down to the ground.
fn fallen_drop(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    move_xz(g, p);
    if p.u16(g, Obj::Z) as i16 > 0 {
        return;
    }
    p.set_u16(g, Obj::Z, 0);
    fallen_lie_down(g, p);
}

/// 0x3b6f4: waits for the end of the lying player's animation.
fn fallen_lie(g: &mut Ctx, p: Player) {
    cov!();
    if animation::step(g, p) {
        fallen_stars_start(g, p);
    }
}

/// 0x3b700: new stars every 16 frames (0xb378a, D0 = 3; D1, the setting, is the offset left
/// by the dispatch); at the end of the count, the player gets up (animation 0x21).
fn fallen_stars(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    let left = p.u8(g, Player::TIMER_6F);
    if left & 0xf == 0 {
        // Original glitch: the setting is D1.b, the offset of the step left by the dispatch
        let setting = g.jumped.offset as u8;
        crate::families::effects_creators::for_players::blow(&mut g.leftovers, g.b, p.obj(), 3, setting);
    }
    let left = p.u8(g, Player::TIMER_6F).wrapping_sub(1);
    p.set_u8(g, Player::TIMER_6F, left);
    if left != 0 {
        return;
    }
    next_step(g, p);
    hurt_animation(g, p, 0x21);
}

// ---- reaction 0x28 (0x3b1e4): sitting on the ground, struggles to get up ------------------------------

/// 0x3b1fa.
fn sit_start(g: &mut Ctx, p: Player) {
    cov!();
    if is_dead(g, p) {
        p.set_u16(g, Player::MAIN, 0x0400);
        p.set_u8(g, Player::ACTIVE, 2);
        p.set_u8(g, Player::HITSTOP, 1);
        return jump(g, p, steps::DOWN_6);
    }
    p.set_u8(g, Player::TIMER_6F, 120);
    next_step(g, p);
    if p.u16(g, Obj::Z) != 0 {
        p.set_u32(g, Obj::VX, 0);
        p.set_u16(g, Obj::VZ, 0);
        p.set_u16(g, Obj::AZ, (-64i16) as u16);
        return hurt_animation(g, p, 3);
    }
    sit(g, p);
}

/// 0x3b23c: sitting (animation 35), or in slow walk directly up (0x3b24e).
fn sit(g: &mut Ctx, p: Player) {
    cov!();
    next_step(g, p);
    if p.u8(g, Player::SLOW) == 0 {
        return hurt_animation(g, p, 35);
    }
    sit_down(g, p);
}

/// 0x3b24e: the struggle begins (animation 34), no more invulnerability.
fn sit_down(g: &mut Ctx, p: Player) {
    cov!();
    p.set_u8(g, Player::ACTIVE, 1);
    p.set_u8(g, Player::INVULN, 0);
    next_step(g, p);
    hurt_animation(g, p, 34);
}

/// 0x3b264: falls back, then sits down.
fn sit_fall(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    move_xz(g, p);
    if altitude(g, p) > 0 {
        return;
    }
    p.set_u16(g, Obj::Z, 0);
    sit(g, p);
}

/// 0x3b28a: each jolt uses two frames of `+0x6f` (one otherwise); at the end, they get up (animation 32,
/// 48 frames of invulnerability).
fn sit_struggle(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    if !struggle_out(g, p) {
        return;
    }
    next_step(g, p);
    p.set_u8(g, Player::INVULN, 48);
    hurt_animation(g, p, 32);
}

/// `+0x6f` drops by one frame, by two if the player struggles; true when exhausted.
fn struggle_out(g: &mut Ctx, p: Player) -> bool {
    cov!();
    let mut left = p.u8(g, Player::TIMER_6F);
    if struggling(g, p) {
        left = left.wrapping_sub(1);
    }
    left = left.wrapping_sub(1);
    p.set_u8(g, Player::TIMER_6F, left);
    (left as i8) <= 0
}

// ---- stepped reaction 0x3b30e (0x3b326..0x3b44a) -------------------------------------------------------

/// Sounds of the reaction: 0xe21a0 (death), 0xe7b3c (hit); 0xb6ba8 (`effects_creators::for_players::type_57`)
/// creates the step-end effect.
const FROZEN_DEATH_SOUND: crate::sound::Sound = crate::sound::CMD_A4_BY_CHARACTER;
const FROZEN_SOUND: crate::sound::Sound = crate::sound::CMD_13E_23E;
/// Comparison bytes of the wait steps (`+0x6c`, `+0x6d`) and animation counter (`+0x1f0`).
const WAIT_LEFT: u32 = 0x6c;
const WAIT_RIGHT: u32 = 0x6d;
const WAIT_ANIMATIONS: u32 = 0x1f0;

/// Zero speeds (X and Z) and vertical acceleration of -0x80: common end of the entry.
fn frozen_drift(g: &mut Ctx, p: Player) {
    p.set_u16(g, Obj::VZ, 0);
    p.set_u16(g, Obj::AZ, (-0x80i16) as u16);
    p.set_u32(g, Obj::VX, 0);
}

/// 0x3b326: step 0. With hit points, normal entry (animation 19, 60 frames); otherwise death on the spot
/// (animation 21) with a sound.
fn frozen_start(g: &mut Ctx, p: Player) {
    cov!();
    if (p.u16(g, Player::HP) as i16) > 0 {
        next_step(g, p);
        p.set_u8(g, Player::TIMER_6F, 60);
        let index = crate::families::small::player_table_byte(g.b, p.obj());
        hurt_animation(g, p, 19);
        g.sound(FROZEN_SOUND, p);
        if p.u16(g, Obj::Z) == 0 {
            next_step(g, p);
            // Original glitch: variant and setting are the D0.b and D1.b left by the code before: the index of the
            // table 0x3baf8 and the offset of the step left by the dispatch
            let (variant, setting) = (index as u8, g.jumped.offset as u8);
            crate::families::effects_creators::for_players::type_57(&mut g.leftovers, g.b, p.obj(), variant, setting);
        } else {
            frozen_drift(g, p);
        }
        return;
    }
    p.set_state(g, 0x0400_2c0a);
    p.set_u8(g, 0x53, 1);
    g.sound(FROZEN_DEATH_SOUND, p);
    p.set_u8(g, Player::TIMER_6F, 60);
    hurt_animation(g, p, 21);
    if (p.u16(g, Obj::Z) as i16) <= 0 {
        return;
    }
    p.set_state(g, 0x0400_2c02);
    frozen_drift(g, p);
}

/// 0x3b3aa: step 2, fall; on the ground, step 10 if the player is on the ground (main state 4), otherwise next step
/// and effect.
fn frozen_fall(g: &mut Ctx, p: Player) {
    cov!();
    move_xz(g, p);
    if (p.u16(g, Obj::Z) as i16) > 0 {
        return;
    }
    p.set_u32(g, Obj::Z, 0);
    if p.u8(g, Player::MAIN) == 4 {
        p.set_u8(g, Player::STEP, 10);
        return;
    }
    next_step(g, p);
    // Original glitch: the creator takes the variant from D0.b, the one left by the movement 0x1da6 (`swap` then
    // `asr.l #8` of the vertical speed), whose low byte is zero, and the setting from D1.b, the offset of the step
    // left by the dispatch
    let variant = ((p.u16(g, Obj::VZ) as i16 as i32) << 8) as u8;
    let setting = g.jumped.offset as u8;
    crate::families::effects_creators::for_players::type_57(&mut g.leftovers, g.b, p.obj(), variant, setting);
}

/// 0x3b3d8 / 0x3b3de: step 4 / 6, wait. As long as the two comparison bytes differ, the animation
/// advances and counts down `+0x1f0` (at zero: end of the reaction); then 60 frames of pause before the animation
/// `next` of the following step.
fn frozen_wait(g: &mut Ctx, p: Player, next: u16) {
    cov!();
    if p.u8(g, WAIT_LEFT) & 15 != p.u8(g, WAIT_RIGHT) & 15 {
        animation::step(g, p);
        let left = p.u8(g, WAIT_ANIMATIONS).wrapping_sub(1);
        p.set_u8(g, WAIT_ANIMATIONS, left);
        if left == 0 {
            return recover(g, p);
        }
    }
    let left = p.u8(g, Player::TIMER_6F).wrapping_sub(1);
    p.set_u8(g, Player::TIMER_6F, left);
    if left == 0 {
        next_step(g, p);
        p.set_u8(g, Player::TIMER_6F, 60);
        hurt_animation(g, p, next);
    }
}

/// 0x3b420: step 8. Game stopped (`A5+0x42`): end of the reaction; otherwise a counter (`A5+0x35`) drops and
/// the player goes to main state 4.
fn frozen_end(g: &mut Ctx, p: Player) {
    cov!();
    if ram::FROZEN.get(g.b) != 0 {
        p.set_u8(g, 0x7e, 0x20);
        return recover(g, p);
    }
    let left = ram::PLAYERS_ALIVE.get(g.b).wrapping_sub(1);
    ram::PLAYERS_ALIVE.set(g.b, left);
    p.set_u16(g, Player::MAIN, 0x0400);
    p.set_u8(g, Player::TIMER_6F, 60);
    next_step(g, p);
    p.set_u16(g, Player::HP, 0);
}

// ---- reaction 0x32 (0x3b44c): held by an enemy ----------------------------------------------------

/// Enemy holding the player (16-bit pointer) and "held" flag.
const HOLDER: u32 = 0x186;
const HELD: u32 = 0x188;
/// State of the enemy holding its grip.
const HOLDING: u16 = 0x020c;

/// 0x3b44c: the held player struggles: 20 new directions free them; they are also freed if
/// the enemy lets go (its state is no longer 0x020c).
fn held(g: &mut Ctx, p: Player) {
    cov!();
    if p.u8(g, Player::STEP) == 0 {
        next_step(g, p);
        p.set_u8(g, Player::TIMER_6F, 20);
        return hurt_animation(g, p, 31);
    }
    animation::step(g, p);
    let pressed = p.u8(g, Player::KEYS_HELD) & 15 & !(p.u8(g, Player::KEYS_PREV) & 15);
    if pressed != 0 {
        let left = p.u8(g, Player::TIMER_6F).wrapping_sub(1);
        p.set_u8(g, Player::TIMER_6F, left);
        if left == 0 {
            return break_free(g, p);
        }
    }
    let holder = p.u16(g, HOLDER) as i16 as i32 as u32;
    if crate::mem::rd16(g.b, holder + 4) != HOLDING {
        break_free(g, p);
    }
}

/// Freed: reaction 0x34 (released).
fn break_free(g: &mut Ctx, p: Player) {
    cov!();
    p.set_u8(g, HELD, 0);
    p.set_u8(g, Player::ACTIVE, 1);
    p.set_state(g, 0x0204_3400);
    p.set_u8(g, Player::TIMER_6F, 0);
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
    p.set_u8(g, 0x192, 0);
    p.set_u16(g, Player::DYING_TIMER, 0);
    let action = p.u8(g, Player::ACTION);
    jump_table(g, p, DOWN_ACTIONS, action);
    crate::terrain::push(g.b, p.obj());
    if p.u8(g, Player::ACTIVE) != 0 {
        ACTORS.reinsert(g.b, p.obj());
    }
    clamp_to_screen(g.b, p);
}
