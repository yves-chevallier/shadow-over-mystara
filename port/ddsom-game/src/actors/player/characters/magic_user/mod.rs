//! The magic-user (characters 8 and 9: both variants have the same code).
//!
//! Four actions, reached through `characters::run`:
//! - `jump` (0x45a78): the common jump (`common::jump`) with a staff strike in the air;
//! - `attack` (0x45cfc): button 1; `Player::SUBACTION` selects the strike (see `kind`);
//! - `special` (0x4619e): the chosen item or spell (`field::SPECIAL_KIND`, 0..0x24);
//! - `dash` (0x4879a): action 0x10, slide specific to the magic-user.
//!
//! The magic-user's weapon is in `WEAPON_B` (`+0x10e`): a staff or one of the four rods (0x24..0x27)
//! that shoot at the carried gesture (`rod_shot`). The thrown items, lightning, the wall and spells 13..20 are
//! those of the other characters (`items`); spells 0x18..0x24 are its own. The shots, effects and
//! sounds are created by Rust calls.

use crate::world::machine::Machine;
use super::common::{self, countdown, field, is_cancelled, next_step, take_event};
use super::foes::{self, Boost, Damage, Live, Strike};
use super::items;
use super::super::spawn;
use super::super::actions::to_stand;
use super::super::combos::clear_recognizers;
use super::super::walk::{move_x, move_xz, move_z};
use super::super::{animation, Ctx, Player};
use crate::world::objects::Obj;
use crate::actors::player::ram;
use crate::actors::player::external as ext;

mod spells;
pub use spells::*;
mod conjurations;
use conjurations::*;

/// Magic-user table (ROM data): jump speeds (four words per jump type).
const JUMP_SPEEDS: u32 = 0x10_459e;

/// Sub-actions of button 1 (`Player::SUBACTION`), all even.
mod kind {
    /// Staff strike, possibly followed by a lunge.
    pub const STAFF: [u8; 8] = [0x00, 0x02, 0x04, 0x0c, 0x14, 0x16, 0x18, 0x1a];
    pub const TURN_STRIKE: u8 = 0x06;
    pub const SLOW_STRIKE: u8 = 0x08;
    pub const AFTER_JUMP: u8 = 0x0a;
    pub const RUN_STRIKE: u8 = 0x0e;
    pub const LOW_BLOW: u8 = 0x10;
    pub const BACK_HOP: u8 = 0x12;
    /// Lunge alone.
    pub const LUNGE: u8 = 0x1c;
}

// ---- jump (0x45a78) --------------------------------------------------------------------------

/// The magic-user's jump: speed table 0x10459e, lower gravity after a run, aerial gesture
/// at step 0xa during which the jump is suspended.
const JUMP_KIT: common::JumpKit = common::JumpKit {
    speeds: JUMP_SPEEDS,
    air_attack,
    busy: |g, p| p.u8(g, field::AIR_ATTACK_DONE) != 0,
    apex_gravity: Some((-0x10, -0x38)),
    extra_step,
    ..common::JumpKit::PLAIN
};

/// 0x45a78: jump.
pub fn jump(g: &mut Ctx, p: Player) {
    cov!();
    common::jump(g, p, &JUMP_KIT);
}

/// 0x45c96: in the air, button 1 gives a staff strike (once per jump, step 0xa).
fn air_attack(g: &mut Ctx, p: Player) {
    cov!();
    if !common::air_attack_pressed(g, p) {
        return;
    }
    p.set_u8(g, Player::STEP, 0xa);
    p.set_u8(g, field::MOVE_STATE, 0);
    p.set_u8(g, field::AIR_ATTACK_DONE, 1);
    let mut anim = if p.u8(g, Player::JUMP_KIND) == 2 { 4 } else { 1 };
    if p.u8(g, Player::KEYS_HELD) & 4 != 0 {
        anim = 3;
    }
    animation::start_plain(g, p, anim);
    common::weapon_voice_b(g, p);
    common::aura_hit_countdown(g, p);
}

/// Step 0xa of the jump (0x45c12): the aerial staff strike; on landing the jump resumes at the
/// recovery (step 8).
fn extra_step(g: &mut Ctx, p: Player, step: u8) {
    cov!();
    if step != 0xa {
        return;
    }
    animation::step(g, p);
    move_xz(g, p);
    if (p.u16(g, Obj::Z) as i16) < 0 {
        p.set_u8(g, Player::STEP, 8);
        common::land(g, p);
    } else {
        rod_shot(g, p);
    }
}

/// 0x46044: at the carried gesture, the held rod (`WEAPON_B` 0x24..0x27) shoots. A used-up rod
/// (reserve flag `+0x18b..+0x191` at zero) becomes a staff again (0x22) and disappears from the item list.
fn rod_shot(g: &mut Ctx, p: Player) {
    cov!();
    if !take_event(g, p) {
        return;
    }
    // (rod, shot: 0x5336a, 0x53478, 0x535f2, reserve flag, interface marker, interface sound)
    const RODS: [(u8, Option<spawn::RodShot>, u32, u32, u32); 4] = [
        (0x24, Some(spawn::RodShot::Flame), 0x18c, 0, 0xa),
        (0x25, Some(spawn::RodShot::Ball), 0x18b, 2, 8),
        (0x26, Some(spawn::RodShot::Star), 0x18d, 4, 9),
        (0x27, None, 0x191, 6, 0xb),
    ];
    for (weapon, shot, flag, tag, sound) in RODS {
        if p.u8(g, Player::WEAPON_B) != weapon {
            continue;
        }
        if let Some(shot) = shot {
            spawn::rod_shot(g, p, shot);
        }
        if p.u8(g, flag) != 0 {
            continue;
        }
        p.set_u8(g, Player::WEAPON_B, 0x22);
        for i in 0..6u32 {
            if p.u8(g, Player::INVENTORY + i) == 0x50 {
                p.set_u8(g, Player::INVENTORY + i, 0x56);
                p.set_u8(g, Player::QUANTITIES + i, 0);
                crate::actors::families::effects_creators::for_players::thrown_shard(&mut g.leftovers, g.b, p.obj(), tag as u8);
                crate::actors::families::creators::for_players::set_a(&mut g.leftovers, g.b, p.obj(), sound as u8);
                return;
            }
        }
        return;
    }
}

// ---- attack (0x45cfc) -----------------------------------------------------------------------

/// 0x45cfc: button 1 attack. No continuation or pending effect; most strikes
/// end with `rod_shot`.
pub fn attack(g: &mut Ctx, p: Player) {
    cov!();
    let sub = p.u8(g, Player::SUBACTION);
    match sub {
        s if kind::STAFF.contains(&s) => staff(g, p),
        kind::TURN_STRIKE => turn_strike(g, p),
        kind::SLOW_STRIKE => simple_strike(g, p, 9, true),
        kind::AFTER_JUMP => after_jump(g, p),
        kind::RUN_STRIKE => run_strike(g, p),
        kind::LOW_BLOW => simple_strike(g, p, 8, false),
        kind::BACK_HOP => common::back_hop(g, p, 0xb, false),
        kind::LUNGE => lunge(g, p),
        _ => {}
    }
}

/// Speed of the lunge: 4 pixels per frame decreasing, forward.
fn start_lunge(g: &mut Ctx, p: Player) {
    cov!();
    common::set_forward_motion(g, p, 0x400, (-0x38i16) as u16);
    animation::start_plain(g, p, 0xd);
    g.sound(ext::sound::STRIKE_LIGHT, p);
}

/// Staff strike (0x45d88): one strike, then, if the button stays pressed and the strike landed, a lunge.
fn staff(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, field::MOVE_STATE, 0);
            p.set_u8(g, field::COMBO_RELEASED, 0);
            p.set_u8(g, field::COMBO_WINDOW, 6);
            animation::start_plain(g, p, 0);
            common::weapon_voice_b(g, p);
        }
        2 => {
            common::combo_window(g, p);
            if !animation::step(g, p) {
                return rod_shot(g, p);
            }
            if p.u8(g, field::COMBO_RELEASED) == 0 && p.u8(g, field::HITS) != 0 {
                next_step(g, p);
                start_lunge(g, p);
            } else {
                to_stand(g, p);
            }
        }
        4 => lunge_slide(g, p),
        _ => {}
    }
}

/// Lunge (0x45d28): forward run-up, until stopping.
fn lunge(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, field::MOVE_STATE, 0);
            start_lunge(g, p);
        }
        2 => lunge_slide(g, p),
        _ => {}
    }
}

/// The lunge slides until the run-up stops.
fn lunge_slide(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    move_x(g, p);
    if common::momentum_reversed(g, p) {
        to_stand(g, p);
    }
}

/// Strike given while turning (0x45e44): step 2 is step 0 of a per-state entry (combos).
fn turn_strike(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 | 2 => {
            let first = p.u8(g, Player::STEP) == 0;
            p.set_u8(g, Player::STEP, 4);
            if first {
                p.set_u8(g, field::MOVE_STATE, 0);
            }
            common::start_with_voice_b(g, p, if first { 7 } else { 0xa });
        }
        4 => wait_then_rod(g, p),
        _ => {}
    }
}

/// A one-gesture strike: animation `anim` with the voice; at step 2 the gesture ends (with the rod
/// shot if `rod`).
fn simple_strike(g: &mut Ctx, p: Player, anim: u16, rod: bool) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, field::MOVE_STATE, 0);
            common::start_with_voice_b(g, p, anim);
        }
        2 => {
            if rod {
                wait_then_rod(g, p);
            } else {
                common::wait_then_stand(g, p);
            }
        }
        _ => {}
    }
}

/// Waits for the end of the gesture then returns to the standing state; in the meantime the rod may shoot.
fn wait_then_rod(g: &mut Ctx, p: Player) {
    cov!();
    if animation::step(g, p) {
        to_stand(g, p);
    } else {
        rod_shot(g, p);
    }
}

/// Landing strike (0x45ed8).
fn after_jump(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, field::MOVE_STATE, 1);
            p.set_u8(g, field::MOVE_KIND, 1);
            animation::start_plain(g, p, 0xc);
            g.sound(ext::sound::STRIKE_HEAVY, p);
            clear_recognizers(g, p);
        }
        2 => common::wait_then_stand(g, p),
        _ => {}
    }
}

/// Strike given while running (0x45f1c): run-up of 5 pixels per frame, then stop.
fn run_strike(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, field::MOVE_STATE, 0);
            common::set_forward_motion(g, p, 0x500, (-0x28i16) as u16);
            animation::start_plain(g, p, 5);
            clear_recognizers(g, p);
            common::weapon_voice_b(g, p);
        }
        2 => {
            if common::run_strike_slide(g, p, 6) {
                rod_shot(g, p);
            }
        }
        4 => wait_then_rod(g, p),
        _ => {}
    }
}

// ---- action 0x10: run-up and slide (0x4879a) ---------------------------------------------------

/// 0x4879a: run-up, brief leap (as long as the jump button is held), slide in two stages.
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
                p.set_u8(g, Player::ACTION_COUNT, 8);
                animation::start(g, p, 0x1d);
                g.sound(ext::sound::SLIDE, p);
            }
        }
        4 => {
            let left = p.u8(g, Player::ACTION_COUNT).wrapping_sub(1);
            p.set_u8(g, Player::ACTION_COUNT, left);
            if left == 0 || p.u8(g, Player::KEYS_HELD) & 0x20 == 0 {
                next_step(g, p);
                p.set_u8(g, Player::ACTION_COUNT, 0xc);
                let brake = if p.u8(g, Player::FACING) != 0 { -0xc0i16 } else { 0xc0 };
                p.set_u16(g, Obj::AX, brake as u16);
            }
            common::dust_jump(g, p);
            move_x(g, p);
        }
        6 => {
            move_x(g, p);
            animation::step(g, p);
            let vx = p.u16(g, Obj::VX) as i16;
            let along = if p.u8(g, Player::FACING) == 0 { vx.wrapping_neg() } else { vx };
            if along as u16 > 0x500 {
                let left = p.u8(g, Player::ACTION_COUNT).wrapping_sub(1);
                p.set_u8(g, Player::ACTION_COUNT, left);
                if (left as i8) >= 0 {
                    common::dust_jump(g, p);
                }
            } else {
                next_step(g, p);
                animation::start(g, p, 0x1e);
            }
        }
        8 => {
            move_x(g, p);
            animation::step(g, p);
            if common::momentum_reversed(g, p) {
                to_stand(g, p);
            }
        }
        _ => {}
    }
}


action_native!(JUMP, 0x45a78, "mage_jump", jump);
action_native!(ATTACK, 0x45cfc, "mage_attack", attack);
action_native!(SPECIAL, 0x4619e, "mage_special", special);
action_native!(DASH, 0x4879a, "mage_dash", dash);
