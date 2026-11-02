//! The thief (characters 10 and 11: both variants have the same code).
//!
//! Four actions, reached through `characters::run`:
//! - `jump` (0x488bc): agile jump (double jump, jump against the screen edge, aerial strike, knife
//!   throw);
//! - `attack` (0x48d48): button 1; `Player::SUBACTION` selects the strike (see `kind`); the thief
//!   has neither strike continuation nor fallback to the basic strike;
//! - `special` (0x49498): the chosen item or spell (`field::SPECIAL_KIND`, 0..20), those of the
//!   fighter (`items`) plus a thrown knife (no. 11);
//! - `dash` (0x49d46): action 0x10, a slide close to `common::dash`.
//!
//! The original routines that create shots, effects and sounds are still called as they are.

use super::common::{self, field, next_step, set_forward_motion};
use super::items;
use super::super::spawn;
use super::super::actions::to_stand;
use super::super::checks::item_curse;
use super::super::combos::clear_recognizers;
use super::super::walk::{move_x, move_xz, move_z};
use super::super::{animation, Ctx, Player};
use crate::mem::{rd16, rd8, wr8};
use crate::objects::Obj;
use crate::player::ram;
use crate::machine::Machine;
use crate::player::external as ext;

/// Original routines called as they are: player effects, created objects and sounds of the thief.
mod rom {
    pub const KNIFE_SOUND: u32 = 0xeb8bc;
    /// Sounds.
    pub const SOUND_SHADOW: u32 = 0xe2578;
    pub const SOUND_STAB_A: u32 = 0xe25f2;
    pub const SOUND_STAB_B: u32 = 0xe7304;
}

/// Thief jump speed table (ROM data): a row of four words (vx, ax, vz, az)
/// per jump type (`0x67`).
const JUMP_SPEEDS: u32 = 0x10_45be;

/// ROM table (PC-relative): offset (x, z) of the thief behind the target of the backstab,
/// per type of linked object.
const BACKSTAB_OFFSETS: u32 = 0x4935a;

/// Record bytes specific to the thief.
mod own {
    /// Last screen edge touched (1 left, 2 right), set by `walk::clamp_to_screen`.
    pub const EDGE_HIT: u32 = 0x5a;
    /// The double jump has already been used during this jump.
    pub const DOUBLE_JUMP_USED: u32 = 0x1ef;
    /// Action byte with no known use (the jump sets it to 4).
    pub const JUMP_ROW: u32 = 0x1e8;
    /// Slide counter.
    pub const COUNTER: u32 = 0x1e8;
    /// Object linked to the action (word: address): the backstab shadow.
    pub const LINKED: u32 = 0x50;
    /// Counter of the animation family (word).
    pub const ANIM_COUNT: u32 = 0x190;
}

/// Sub-actions of button 1 (`Player::SUBACTION`), all even.
mod kind {
    pub const COMBO: u8 = 0x00;
    pub const STAB_A: u8 = 0x02;
    pub const STAB_B: u8 = 0x04;
    pub const TURN_STRIKE: u8 = 0x06;
    pub const SLOW_STRIKE: u8 = 0x08;
    pub const AFTER_JUMP: u8 = 0x0a;
    pub const CHARGE: u8 = 0x0c;
    pub const RUN_STRIKE: u8 = 0x0e;
    pub const LOW_BLOW: u8 = 0x10;
    pub const BACK_HOP: u8 = 0x12;
    pub const GUARD_BREAK: u8 = 0x14;
    /// Jumping kick.
    pub const LEAP_KICK: u8 = 0x16;
    /// Backward leap with kick.
    pub const BACK_KICK: u8 = 0x18;
    /// Backstab: the shadow darts forward, the thief joins it behind the target.
    pub const BACKSTAB: u8 = 0x1a;
    pub const POSE: u8 = 0x1c;
    /// Thrown knife (item 0x2c).
    pub const KNIFE: u8 = 0x1e;
}

// ---- jump (0x488bc) --------------------------------------------------------------------------

/// 0x488bc: jump. Step 0 crouch, 2 rise, 4 peak, 6 descent, 8 recovery, 0xa aerial strike, 0xc aerial
/// knife, 0xe jump against the screen edge.
pub fn jump(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => takeoff(g, p),
        2 => rise(g, p),
        4 => apex(g, p),
        6 => fall(g, p),
        8 => recover(g, p),
        0xa => air_strike(g, p),
        0xc => air_knife(g, p),
        0xe => wall_kick(g, p),
        _ => {}
    }
    common::flush_pending_effect(g, p);
}

/// Step 0: at the end of the crouch the jump starts.
fn takeoff(g: &mut Ctx, p: Player) {
    cov!();
    if !animation::step(g, p) {
        return;
    }
    next_step(g, p);
    p.set_u8(g, field::AIR_STEERED, 0);
    p.set_u8(g, field::AIR_ATTACK_DONE, 0);
    p.set_u8(g, own::DOUBLE_JUMP_USED, 0);
    p.set_u8(g, own::EDGE_HIT, 0);
    p.set_u8(g, own::JUMP_ROW, 4);
    let x = p.u16(g, Obj::X);
    p.set_u16(g, field::TAKEOFF_X, x);
    let anim = common::by_jump_kind(g, p, 0x15, 0x10, 0xb);
    animation::start(g, p, anim);
    g.sound(ext::sound::JUMP_VOICE, p);
    let kind = p.u8(g, 0x67);
    common::launch_jump(g, p, JUMP_SPEEDS, kind);
}

/// The horizontal speed is capped at 0x300 (the rest of the momentum is lost).
fn limit_speed(g: &mut Ctx, p: Player) {
    cov!();
    let speed = p.u16(g, Obj::VX) as i16;
    if speed.unsigned_abs() > 0x300 {
        let limit = if p.u8(g, Player::FACING) == 0 { -0x300i16 } else { 0x300 };
        p.set_u16(g, Obj::AX, 0);
        p.set_u16(g, Obj::VX, limit as u16);
    }
}

/// Step 2: rise.
fn rise(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    move_z(g, p);
    if p.u8(g, 0x67) != 0 {
        move_x(g, p);
        limit_speed(g, p);
    }
    common::air_steer(g, p, 0x30);
    if air_options(g, p) {
        return;
    }
    air_attack(g, p);
    if p.u8(g, field::AIR_ATTACK_DONE) != 0 || p.u16(g, Obj::VZ) > 0x180 {
        return;
    }
    next_step(g, p);
    let anim = common::by_jump_kind(g, p, 0x16, 0x11, 0xc);
    animation::start(g, p, anim);
}

/// Step 4: peak.
fn apex(g: &mut Ctx, p: Player) {
    cov!();
    move_z(g, p);
    if p.u8(g, 0x67) != 0 {
        move_x(g, p);
        limit_speed(g, p);
    }
    common::air_steer(g, p, 0x30);
    if air_options(g, p) {
        return;
    }
    air_attack(g, p);
    if p.u8(g, field::AIR_ATTACK_DONE) != 0 {
        return;
    }
    if animation::step(g, p) {
        next_step(g, p);
        let anim = common::by_jump_kind(g, p, 0x17, 0x12, 0xd);
        animation::start(g, p, anim);
    }
}

/// Step 6: descent; on the ground the recovery begins.
fn fall(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    move_xz(g, p);
    if air_options(g, p) {
        return;
    }
    air_attack(g, p);
    if p.u8(g, field::AIR_ATTACK_DONE) != 0 {
        return;
    }
    limit_speed(g, p);
    if (p.u16(g, Obj::Z) as i16) > 0 {
        return;
    }
    next_step(g, p);
    land(g, p);
}

/// Landing: on the ground (`Z` = 0), recovery animation and sound.
fn land(g: &mut Ctx, p: Player) {
    cov!();
    p.set_u32(g, Obj::Z, 0);
    let anim = common::by_jump_kind(g, p, 0x18, 0x13, 0xe);
    animation::start(g, p, anim);
    g.sound(ext::sound::LANDING, p);
}

/// Step 8: recovery, then return to the standing state (and fall of the cursed weapon).
fn recover(g: &mut Ctx, p: Player) {
    cov!();
    if animation::step(g, p) {
        to_stand(g, p);
        common::curse_check(g, p);
    }
}

/// Step 0xa: strike given in the air; on the ground the recovery begins.
fn air_strike(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    move_xz(g, p);
    if air_options(g, p) || (p.u16(g, Obj::Z) as i16) > 0 {
        return;
    }
    p.set_u8(g, Player::STEP, 8);
    land(g, p);
}

/// Step 0xc: knife thrown in the air (item 0x2c); on the ground the recovery begins.
fn air_knife(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    move_xz(g, p);
    if air_options(g, p) {
        return;
    }
    if (p.u16(g, Obj::Z) as i16) <= 0 {
        p.set_u8(g, Player::STEP, 8);
        land(g, p);
    } else if common::take_event(g, p) {
        spawn::knife(g, p);
        g.sound(rom::KNIFE_SOUND, p);
    }
}

/// Step 0xe: the thief clings to the screen edge, then sets off in the opposite direction.
fn wall_kick(g: &mut Ctx, p: Player) {
    cov!();
    if !animation::step(g, p) {
        return;
    }
    p.set_u8(g, Player::STEP, 2);
    let facing = if p.u8(g, own::EDGE_HIT) == 1 { 0x20 } else { 0 };
    p.set_u8(g, Player::FACING, facing);
    p.set_u8(g, field::AIR_ATTACK_DONE, 0);
    p.set_u8(g, own::EDGE_HIT, 0);
    p.set_u16(g, Obj::VZ, 0x600);
    p.set_u16(g, Obj::AZ, (-0x38i16) as u16);
    let vx = if facing == 0 { -0x200i16 } else { 0x200 };
    p.set_u16(g, Obj::VX, vx as u16);
    p.set_u16(g, Obj::AX, 0);
    animation::start(g, p, 0x10);
    g.sound(ext::sound::JUMP_VOICE, p);
}

/// 0x48ca0: in the air, a screen edge touched with the jump button held makes the thief cling to the edge
/// (step 0xe); otherwise the jump button pressed again gives a double jump (once per jump).
/// No effect while the thief is still rising near the ground. Returns true if either one started.
fn air_options(g: &mut Ctx, p: Player) -> bool {
    cov!();
    if (p.u16(g, Obj::VZ) as i16) < 0 && p.u16(g, Obj::Z) <= 0x10 {
        return false;
    }
    if grab_edge(g, p) {
        return true;
    }
    if p.u8(g, own::DOUBLE_JUMP_USED) != 0 || p.u8(g, Player::KEYS_NEW) & 0x20 == 0 {
        return false;
    }
    p.set_u8(g, Player::STEP, 2);
    p.set_u8(g, field::AIR_ATTACK_DONE, 0);
    p.set_u8(g, 0x67, 1);
    p.set_u8(g, own::DOUBLE_JUMP_USED, 1);
    p.set_u16(g, Obj::VZ, 0x600);
    p.set_u16(g, Obj::AZ, (-0x38i16) as u16);
    p.set_u32(g, Obj::VX, 0);
    let keys = p.u8(g, Player::KEYS_HELD);
    p.set_u16(g, Obj::VX, 0x200);
    if keys & 1 == 0 {
        p.set_u16(g, Obj::VX, (-0x200i16) as u16);
        if keys & 2 == 0 {
            p.set_u8(g, 0x67, 0);
            p.set_u32(g, Obj::VX, 0);
        }
    }
    animation::start(g, p, 3);
    g.sound(ext::sound::JUMP_VOICE, p);
    true
}

/// 0x48d22: a screen edge has just been touched and the jump button is held.
fn grab_edge(g: &mut Ctx, p: Player) -> bool {
    cov!();
    if p.u8(g, own::EDGE_HIT) == 0 || p.u8(g, Player::KEYS_HELD) & 0x20 == 0 {
        return false;
    }
    p.set_u8(g, Player::STEP, 0xe);
    animation::start(g, p, 0x20);
    true
}

/// 0x48bf0: in the air, button 1 gives a strike (step 0xa), key 4 with the knife (item 0x2c)
/// throws it (step 0xc); once per jump.
fn air_attack(g: &mut Ctx, p: Player) {
    cov!();
    if p.u8(g, field::AIR_ATTACK_DONE) != 0 {
        return;
    }
    let keys_new = p.u8(g, Player::KEYS_NEW);
    if keys_new & 0x10 != 0 {
        p.set_u8(g, Player::STEP, 0xa);
        p.set_u8(g, field::MOVE_STATE, 0);
        p.set_u8(g, field::AIR_ATTACK_DONE, 1);
        let mut anim = if p.u8(g, 0x67) == 2 { 8 } else { 5 };
        if p.u8(g, Player::KEYS_HELD) & 4 != 0 {
            anim = 7;
        }
        common::start_with_voice(g, p, anim);
        item_curse(g, p);
        common::aura_hit_countdown(g, p);
    } else if keys_new & 0x80 != 0 && p.u8(g, field::ITEM) == 0x2c {
        p.set_u8(g, Player::STEP, 0xc);
        p.set_u8(g, field::AIR_ATTACK_DONE, 2);
        animation::start_plain(g, p, 0x1f);
        common::aura_hit_countdown(g, p);
    }
}

// ---- attack (0x48d48) -----------------------------------------------------------------------

/// 0x48d48: button 1 attack; `kind` selects the strike.
pub fn attack(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::SUBACTION) {
        kind::COMBO => combo(g, p),
        kind::STAB_A => common::single_strike(g, p, 3),
        kind::STAB_B => common::single_strike(g, p, 4),
        kind::TURN_STRIKE => turn_strike(g, p),
        kind::SLOW_STRIKE => common::single_strike(g, p, 0xf),
        kind::AFTER_JUMP => after_jump(g, p),
        kind::CHARGE => charge(g, p),
        kind::RUN_STRIKE => run_strike(g, p),
        kind::LOW_BLOW => common::single_strike(g, p, 0xd),
        kind::BACK_HOP => common::back_hop(g, p, 0x17, false),
        kind::GUARD_BREAK => common::single_strike(g, p, 0xe),
        kind::LEAP_KICK => leap_kick(g, p),
        kind::BACK_KICK => back_kick(g, p),
        kind::BACKSTAB => backstab(g, p),
        kind::POSE => common::pose(g, p, 0x12),
        kind::KNIFE => throw_knife(g, p, 0x20),
        _ => {}
    }
    common::flush_pending_effect(g, p);
}

/// Basic strike (0x48dba): three chained strikes if the previous ones landed.
fn combo(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, field::MOVE_STATE, 0);
            p.set_u8(g, field::COMBO_RELEASED, 0);
            p.set_u8(g, field::COMBO_WINDOW, 6);
            let rank = common::chain_rank(g, p, 1, 2);
            common::start_with_voice(g, p, rank as u16);
        }
        2 => {
            common::combo_follow(g, p, 0x12);
        }
        4 => common::wait_then_stand(g, p),
        _ => {}
    }
}

/// Strike given while turning (0x48eb8): step 2 is step 0 of a per-state entry (combos).
fn turn_strike(g: &mut Ctx, p: Player) {
    cov!();
    let step = p.u8(g, Player::STEP);
    match step {
        0 | 2 => {
            if step == 0 {
                p.set_u8(g, field::MOVE_STATE, 0);
            }
            p.set_u8(g, Player::STEP, 4);
            common::start_with_voice(g, p, if step == 0 { 9 } else { 0x16 });
        }
        4 => common::wait_then_stand(g, p),
        _ => {}
    }
}

/// Landing strike (0x48f38).
fn after_jump(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, field::MOVE_STATE, 0);
            animation::start_plain(g, p, 9);
            clear_recognizers(g, p);
            common::weapon_voice(g, p);
            g.sound(ext::sound::LEAP, p);
        }
        2 => common::wait_then_stand(g, p),
        _ => {}
    }
}

/// Charge (0x48f6e): run-up, gesture, slide with two effects.
fn charge(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, 0x5b, 0);
            p.set_u8(g, field::MOVE_STATE, 1);
            p.set_u8(g, field::MOVE_KIND, 1);
            set_forward_motion(g, p, 0xc00, (-0x98i16) as u16);
            animation::start_plain(g, p, 0x10);
            clear_recognizers(g, p);
        }
        2 => {
            animation::step(g, p);
            if common::take_event(g, p) {
                next_step(g, p);
                g.sound(ext::sound::CHARGE_A, p);
            }
        }
        4 => common::charge_slide(g, p, |g, p| {
            if p.u8(g, field::EVENT) == 1 {
                p.set_u8(g, field::EVENT, 0);
                spawn::slash(g, p, Some(0));
                spawn::slash(g, p, None);
                g.sound(ext::sound::CHARGE_B, p);
                common::weapon_voice(g, p);
            }
        }),
        _ => {}
    }
}

/// Strike given while running (0x49036): run-up of 5 pixels per frame, then a dead stop.
fn run_strike(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, field::MOVE_STATE, 0);
            set_forward_motion(g, p, 0x500, (-0x28i16) as u16);
            animation::start_plain(g, p, 0xa);
            clear_recognizers(g, p);
            common::weapon_voice(g, p);
            g.sound(ext::sound::CHARGE_A, p);
        }
        2 => {
            animation::step(g, p);
            if common::momentum_reversed(g, p) {
                p.set_u32(g, Obj::VX, 0);
                p.set_u8(g, Player::SLOW, 0);
                to_stand(g, p);
            } else {
                common::dust_run(g, p);
                move_x(g, p);
            }
        }
        4 => common::wait_then_stand(g, p),
        _ => {}
    }
}

/// Jumping kick (0x49166): leap of 1 pixel per frame, one gesture (effect), a second (shadow).
fn leap_kick(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, 0x5b, 0);
            p.set_u8(g, field::MOVE_STATE, 1);
            p.set_u8(g, field::MOVE_KIND, 1);
            p.set_u16(g, Obj::VZ, 0x600);
            p.set_u16(g, Obj::AZ, (-0x40i16) as u16);
            set_forward_motion(g, p, 0x100, 0);
            animation::start_plain(g, p, 0x11);
            clear_recognizers(g, p);
        }
        2 => {
            animation::step(g, p);
            // the original tests "event absent": the next step therefore comes right away
            if p.u8(g, field::EVENT) == 0 {
                next_step(g, p);
                g.sound(ext::sound::LEAP_END, p);
                common::weapon_voice(g, p);
                g.sound(ext::sound::CHARGE_B, p);
            }
        }
        4 => {
            animation::step(g, p);
            move_xz(g, p);
            if (p.u16(g, Obj::Z) as i16) < 0 {
                p.set_u8(g, Player::SLOW, 0);
                p.set_u32(g, Obj::Z, 0);
                to_stand(g, p);
                return;
            }
            if p.u8(g, field::EVENT) == 1 {
                p.set_u8(g, field::EVENT, 0);
                spawn::leap(g, p, 0);
            }
            if p.u8(g, field::EVENT) == 2 {
                p.set_u8(g, field::EVENT, 0);
                g.sound(rom::SOUND_SHADOW, p);
                spawn::shadow(g, p);
            }
        }
        _ => {}
    }
}

/// Backward leap with kick (0x49244).
fn back_kick(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, 0x5b, 0);
            p.set_u8(g, 0x1ec, 0);
            p.set_u8(g, 0x67, 0);
            p.set_u16(g, Obj::VZ, 0x380);
            p.set_u16(g, Obj::AZ, (-0x38i16) as u16);
            let vx = if p.u8(g, Player::FACING) != 0 { 0xfd00 } else { 0x300 };
            p.set_u16(g, Obj::VX, vx);
            p.set_u16(g, Obj::AX, 0);
            animation::start_plain(g, p, 0x13);
            g.sound(ext::sound::LEAP_END, p);
            clear_recognizers(g, p);
        }
        2 => {
            if p.u8(g, field::EVENT) != 0 {
                p.set_u32(g, Obj::VX, 0);
                p.set_u8(g, field::EVENT, 0);
                spawn::shadows(g, p);
                g.sound(ext::sound::SPIN, p);
            }
            move_xz(g, p);
            if (p.u16(g, Obj::Z) as i16) < 0 {
                p.set_u8(g, Player::SLOW, 0);
                p.set_u32(g, Obj::Z, 0);
                to_stand(g, p);
            } else {
                animation::step(g, p);
            }
        }
        _ => {}
    }
}

/// The object attached to the thief (word `+0x50`, sign-extended address).
fn linked(g: &mut Ctx, p: Player) -> u32 {
    p.u16(g, own::LINKED) as i16 as i32 as u32 & 0xff_ffff
}

/// Backstab (0x492d0): the shadow (created by the jumping kick) is joined behind its
/// target, the thief strikes then lands.
fn backstab(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, field::MOVE_STATE, 0);
            p.set_u8(g, 0x67, 0);
            animation::start_plain(g, p, 0x14);
            clear_recognizers(g, p);
        }
        2 => {
            animation::step(g, p);
            if p.u8(g, field::EVENT) == 1 {
                next_step(g, p);
                appear_behind_shadow(g, p);
            }
        }
        4 => {
            if animation::step(g, p) {
                next_step(g, p);
                let count = p.u16(g, own::ANIM_COUNT).wrapping_add(2);
                p.set_u16(g, own::ANIM_COUNT, count);
                animation::start_plain(g, p, 0x15);
            } else if (p.u8(g, field::EVENT) as i8) < 0 {
                g.sound(rom::SOUND_STAB_A, p);
                g.sound(rom::SOUND_STAB_B, p);
            }
        }
        6 => {
            animation::step(g, p);
            if common::take_event(g, p) {
                next_step(g, p);
                p.set_u16(g, Obj::VZ, 0x300);
                p.set_u16(g, Obj::AZ, (-0x40i16) as u16);
                let vx = if p.u8(g, Player::FACING) != 0 { -0x100i16 } else { 0x100 };
                p.set_u16(g, Obj::VX, vx as u16);
                p.set_u16(g, Obj::AX, 0);
                let shadow = linked(g, p);
                if rd8(g.b, shadow + 0x49) == 0x64 {
                    wr8(g.b, shadow, 1);
                    wr8(g.b, shadow + 6, 4);
                }
            }
        }
        8 => {
            if animation::step(g, p) {
                p.set_u8(g, Player::SLOW, 0);
                p.set_u32(g, Obj::Z, 0);
                to_stand(g, p);
                return;
            }
            move_xz(g, p);
            if (p.u16(g, Obj::Z) as i16) < 0 {
                p.set_u32(g, Obj::Z, 0);
                p.set_u32(g, Obj::VX, 0);
                p.set_u32(g, Obj::VZ, 0);
            }
        }
        _ => {}
    }
}

/// The thief takes the place of its shadow, on the other side: same position and same inverted
/// orientation, offset by the value of the table `BACKSTAB_OFFSETS` according to the kind of the shadow.
fn appear_behind_shadow(g: &mut Ctx, p: Player) {
    cov!();
    let shadow = linked(g, p);
    let v = rd16(g.b, shadow + Obj::X);
    p.set_u16(g, Obj::X, v);
    let v = rd16(g.b, shadow + Obj::Z);
    p.set_u16(g, Obj::Z, v);
    let v = rd16(g.b, shadow + Obj::Y);
    p.set_u16(g, Obj::Y, v);
    let v = rd16(g.b, shadow + Player::FACING);
    p.set_u16(g, Player::FACING, v);
    p.flip(g);
    let row = BACKSTAB_OFFSETS + 4 * rd8(g.b, shadow + 2) as u32;
    let mut dx = g.prog16(row) as i16;
    let dz = g.prog16(row + 2);
    if rd8(g.b, shadow + Player::FACING) == 0 {
        dx = dx.wrapping_neg();
    }
    let x = p.u16(g, Obj::X).wrapping_add(dx as u16);
    p.set_u16(g, Obj::X, x);
    let z = p.u16(g, Obj::Z).wrapping_add(dz);
    p.set_u16(g, Obj::Z, z);
    let y = p.u16(g, Obj::Y).wrapping_add(1);
    p.set_u16(g, Obj::Y, y);
}

// ---- special attack (0x49498) --------------------------------------------------------------

/// The thief's animations for cast spells (see `items::Style`).
const STYLE: items::Style = items::Style { cast_anim: 0x1d, bolt_anim: 0x1c, spark: 5, ..items::Style::CLASSIC };

/// 0x49498: the chosen item or spell (`field::SPECIAL_KIND`, 0..20), like the fighter, with the
/// thrown knife (no. 11); number 3 does nothing. 0..8: thrown items; 12: weapon swap;
/// 9, 10 and 13..20: `items`.
pub fn special(g: &mut Ctx, p: Player) {
    cov!();
    let which = p.u8(g, field::SPECIAL_KIND);
    ram::SPELL_ROW.set(g.b, which);
    match which {
        0 => items::throw_item(g, p, 0x1a, spawn::Thrown::A),
        1 => items::throw_item(g, p, 0x1a, spawn::Thrown::B),
        2 => items::throw_item(g, p, 0x1b, spawn::Thrown::C),
        4 => items::throw_item(g, p, 0x1b, spawn::Thrown::D),
        5 => items::throw_item(g, p, 0x1b, spawn::Thrown::E),
        6 => items::throw_item(g, p, 0x19, spawn::Thrown::F),
        7 => items::throw_item(g, p, 0x19, spawn::Thrown::G),
        8 => items::throw_item(g, p, 0x18, spawn::Thrown::H),
        11 => throw_knife(g, p, 0x1e),
        12 => common::swap_weapon(g, p, Player::WEAPON_A, false),
        _ => {
            items::run_shared(g, p, which, STYLE);
        }
    }
}

/// Thrown knife (special attack no. 11, 0x49636, and strike 0x1e, 0x4945e: gesture `anim`): at the gesture, the shot is
/// created (and its sound); no charge consumed.
fn throw_knife(g: &mut Ctx, p: Player, anim: u16) {
    cov!();
    if p.u8(g, Player::STEP) == 0 {
        next_step(g, p);
        animation::start_plain(g, p, anim);
        return;
    }
    if animation::step(g, p) {
        to_stand(g, p);
        return;
    }
    if common::take_event(g, p) {
        if spawn::knife(g, p) {
            g.sound(rom::KNIFE_SOUND, p);
        }
    }
}

// ---- slide (0x49d46) ----------------------------------------------------------------------

/// Action 0x10: run-up, brief leap, slide braked without dust (see `common::dash`).
pub fn dash(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            common::set_forward_motion(g, p, 0xb00, 0);
            animation::start(g, p, 0x1c);
            clear_recognizers(g, p);
        }
        2 => {
            if animation::step(g, p) {
                next_step(g, p);
                p.set_u8(g, own::COUNTER, 8);
                animation::start(g, p, 0x1d);
                g.sound(ext::sound::SLIDE, p);
            }
        }
        4 => {
            let left = p.u8(g, own::COUNTER).wrapping_sub(1);
            p.set_u8(g, own::COUNTER, left);
            if left != 0 && p.u8(g, Player::KEYS_HELD) & 0x20 != 0 {
                common::dust_jump(g, p);
                move_x(g, p);
                return;
            }
            next_step(g, p);
            p.set_u8(g, own::COUNTER, 8);
            let brake = if p.u8(g, Player::FACING) != 0 { -0xa0i16 } else { 0xa0 };
            p.set_u16(g, Obj::AX, brake as u16);
            slide(g, p);
        }
        6 => slide(g, p),
        _ => {}
    }
}

/// Slide braked to a stop.
fn slide(g: &mut Ctx, p: Player) {
    cov!();
    move_x(g, p);
    animation::step(g, p);
    if common::momentum_reversed(g, p) {
        to_stand(g, p);
    }
}

// ---- action 0x12 (0x35b50) -------------------------------------------------------------------

/// Thief roll: run-up of 6 pixels per frame; the movement only happens on the frames where the animation
/// script signals an event (which is not cleared), then return to the standing state.
pub fn roll(g: &mut Ctx, p: Player) {
    cov!();
    if p.u8(g, Player::STEP) == 0 {
        next_step(g, p);
        common::roll_start(g, p, 0x600);
        g.sound(0xe266c, p);
        return;
    }
    if animation::step(g, p) {
        to_stand(g, p);
        return;
    }
    if p.u8(g, field::EVENT) != 0 {
        move_x(g, p);
    }
}

action_native!(JUMP, 0x488bc, "thief_jump", jump);
action_native!(ATTACK, 0x48d48, "thief_attack", attack);
action_native!(SPECIAL, 0x49498, "thief_special", special);
action_native!(DASH, 0x49d46, "thief_dash", dash);
action_native!(ROLL, 0x35b50, "thief_roll", roll);
