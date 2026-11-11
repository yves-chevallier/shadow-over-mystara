//! The cleric (characters 2 and 3: both variants have the same code).
//!
//! Four actions, reached through `characters::run`:
//! - `jump` (0x3fd08): the common jump (`common::jump`) with the cleric's aerial strike;
//! - `attack` (0x3ff2a): button 1; `Player::SUBACTION` selects the strike (see `kind`);
//! - `special` (0x406d4): the chosen item or prayer (`field::SPECIAL_KIND`): the items and spells
//!   shared with the fighter (`items`) and the cleric's ten prayers (numbers 0x24..0x2d);
//! - action 0x10 (0x41f22) is the same slide as the fighter's (`common::dash`).
//!
//! The prayers darken the screen, freeze the other players and pause the game until
//! their end (`common::begin_spell` / `end_spell`). The shots, effects and
//! sounds are created by Rust calls.

use crate::world::machine::Machine;
use super::common::{self, field, next_step};
use super::items;
use super::super::spawn;
use super::super::actions::{start_stand, to_stand};
use super::super::combos::clear_recognizers;
use super::super::init::init_weapon_stats;
use super::super::walk::move_xz;
use super::super::{animation, ram, Ctx, Player};
use crate::hw::mem::wr8;
use crate::world::objects::pools::{BOSS, ENEMY};
use crate::world::objects::Obj;
use crate::actors::player::external as ext;
use crate::actors::player::tables::{players, Read};

mod turning;
pub use turning::*;
mod prayers;
use prayers::*;

/// Identifiers (original addresses) of the routines reached: player effects, sounds and palettes of the cleric.
mod rom {
    /// Sounds of the strikes.
    pub const SOUND_STRIKE_B: crate::services::sound::Sound = crate::services::sound::CMD_14D_24D;
    /// Sounds of the spinning jump.
    pub const SOUND_LEAP_LAND: crate::services::sound::Sound = crate::services::sound::CMD_CC_BY_PLAYER;
    pub const SOUND_SPIN: crate::services::sound::Sound = crate::services::sound::CMD_14C_24C;
    /// Prayers: player effects (PEFF pool) created at the gesture, with their sounds.
    pub const PRAYER_EVENT_SOUND: crate::services::sound::Sound = crate::services::sound::CMD_50;
    pub const PRAYER_AURA_SOUND: crate::services::sound::Sound = crate::services::sound::CMD_17F;
    pub const PRAYER_AURA_B_SOUND: crate::services::sound::Sound = crate::services::sound::CMD_18D;
    pub const PRAYER_STORM_SOUND: crate::services::sound::Sound = crate::services::sound::CMD_51;
    pub const PRAYER_SWARM_START_SOUND: crate::services::sound::Sound = crate::services::sound::CMD_54;
    pub const SOUND_PRAYER: crate::services::sound::Sound = crate::services::sound::CMD_57;
    pub const PRAYER_BANISH_SOUND: crate::services::sound::Sound = crate::services::sound::CMD_17A_C;
    pub const PRAYER_BANISH_START_SOUND: crate::services::sound::Sound = crate::services::sound::CMD_58;
    pub const PRAYER_HOLY_SOUND: crate::services::sound::Sound = crate::services::sound::CMD_59;
    pub const PRAYER_BLESS_SOUND: crate::services::sound::Sound = crate::services::sound::CMD_171;
    pub const PRAYER_BLESS_SOUND_AREA: crate::services::sound::Sound = crate::services::sound::PRAYER;
    pub const PRAYER_BLESS_SOUND_SELF: crate::services::sound::Sound = crate::services::sound::CMD_52;
    pub const PRAYER_PROTECT_SOUND: crate::services::sound::Sound = crate::services::sound::CMD_56;
    pub const PRAYER_HEAL_SOUND_SELF_A: crate::services::sound::Sound = crate::services::sound::CMD_53;
    pub const PRAYER_HEAL_SOUND_SELF_B: crate::services::sound::Sound = crate::services::sound::CMD_55;
}

/// Cleric table (ROM data): jump speeds (four words per jump type).
const JUMP_SPEEDS: u32 = 0x10_451e;

/// Sub-actions of button 1 (`Player::SUBACTION`), all even.
mod kind {
    pub const COMBO: u8 = 0x00;
    pub const THRUST_HIGH: u8 = 0x02;
    pub const THRUST_LOW: u8 = 0x04;
    pub const TURN_STRIKE: u8 = 0x06;
    pub const SLOW_STRIKE: u8 = 0x08;
    pub const AFTER_JUMP: u8 = 0x0a;
    pub const CHARGE: u8 = 0x0c;
    pub const RUN_STRIKE: u8 = 0x0e;
    pub const LOW_BLOW: u8 = 0x10;
    pub const BACK_HOP: u8 = 0x12;
    pub const GUARD_BREAK: u8 = 0x14;
    /// Spinning jump (special attack 0x2d).
    pub const LEAP: u8 = 0x16;
    /// Prayer for turning away the undead.
    pub const TURN_UNDEAD: u8 = 0x18;
    pub const POSE_A: u8 = 0x1a;
    pub const POSE_B: u8 = 0x1c;
}

// ---- jump (0x3fd08) --------------------------------------------------------------------------

/// The cleric's jump: takeoff speeds in the table 0x10451e, aerial strike without a cursed weapon.
const JUMP_KIT: common::JumpKit = common::JumpKit {
    speeds: JUMP_SPEEDS,
    air_attack,
    ..common::JumpKit::PLAIN
};

/// 0x3fd08: jump.
pub fn jump(g: &mut Ctx, p: Player) {
    cov!();
    common::jump(g, p, &JUMP_KIT);
}

/// 0x3feca: button 1 in the air gives a strike (once per jump); down: diving strike.
fn air_attack(g: &mut Ctx, p: Player) {
    cov!();
    if !common::air_attack_pressed(g, p) {
        return;
    }
    p.set_u8(g, field::MOVE_STATE, 0);
    p.set_u8(g, field::AIR_ATTACK_DONE, 1);
    let mut anim = if p.u8(g, Player::JUMP_KIND) == 2 { 0xc } else { 9 };
    if p.u8(g, Player::KEYS_HELD) & 4 != 0 {
        anim = 0xb;
    }
    common::start_with_voice(g, p, anim);
    common::aura_hit_countdown(g, p);
}

// ---- attack (0x3ff2a) -----------------------------------------------------------------------

/// 0x3ff2a: button 1 attack. Ground strikes are followed by a continuation check.
pub fn attack(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::SUBACTION) {
        kind::COMBO => {
            combo(g, p);
            common::follow_up(g, p);
        }
        kind::THRUST_HIGH => {
            thrust(g, p, 1 << 1, 5);
            common::follow_up(g, p);
        }
        kind::THRUST_LOW => {
            thrust(g, p, 1 << 2, 7);
            common::follow_up(g, p);
        }
        kind::TURN_STRIKE => {
            turn_strike(g, p);
            common::follow_up(g, p);
        }
        kind::SLOW_STRIKE => slow_strike(g, p),
        kind::AFTER_JUMP => {
            after_jump(g, p);
            common::follow_up(g, p);
        }
        kind::CHARGE => charge(g, p),
        kind::RUN_STRIKE => run_strike(g, p),
        kind::LOW_BLOW => {
            common::single_strike(g, p, 0x11);
            common::follow_up(g, p);
        }
        kind::BACK_HOP => common::back_hop(g, p, 0x1c, true),
        kind::GUARD_BREAK => guard_break(g, p),
        kind::LEAP => leap(g, p),
        kind::TURN_UNDEAD => turn_undead(g, p),
        kind::POSE_A | kind::POSE_B => {
            common::pose(g, p, 0x17);
            common::follow_up(g, p);
        }
        _ => {}
    }
}

/// Basic strike (0x3ff8c): up to five chained strikes, the animation follows the rank.
fn combo(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, field::MOVE_STATE, 0);
            p.set_u8(g, field::COMBO_RELEASED, 0);
            p.set_u8(g, field::COMBO_WINDOW, 6);
            let rank = common::chain_rank(g, p, 1, 4);
            common::start_with_voice(g, p, rank as u16);
        }
        2 => {
            common::combo_follow(g, p, 0x17);
        }
        4 => common::wait_then_stand(g, p),
        _ => {}
    }
}

/// High (0x4003c) and low (0x4008e) thrusts: one gesture, of rank 0 or 1.
fn thrust(g: &mut Ctx, p: Player, hit_bit: u8, base_anim: u16) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, field::MOVE_STATE, 0);
            let rank = common::chain_rank(g, p, hit_bit, 1);
            common::start_with_voice(g, p, base_anim + rank as u16);
        }
        2 => common::wait_then_stand(g, p),
        _ => {}
    }
}

/// Strike given while turning (0x400e0): step 2 is step 0 of a per-state entry (combos).
fn turn_strike(g: &mut Ctx, p: Player) {
    cov!();
    let step = p.u8(g, Player::STEP);
    match step {
        0 | 2 => {
            if step == 0 {
                p.set_u8(g, field::MOVE_STATE, 0);
            }
            p.set_u8(g, Player::STEP, 4);
            common::start_with_voice(g, p, if step == 0 { 0x10 } else { 0x1b });
        }
        4 => common::wait_then_stand(g, p),
        _ => {}
    }
}

/// Strike given in slow walk (0x4012c): without continuation.
fn slow_strike(g: &mut Ctx, p: Player) {
    cov!();
    common::single_strike(g, p, 0x13);
}

/// Landing strike (0x40166): the mace (weapon 0x1e) has its own gesture.
fn after_jump(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, field::MOVE_STATE, 0);
            if p.u8(g, Player::WEAPON_A) == 0x1e {
                animation::start_plain(g, p, 0x18);
                clear_recognizers(g, p);
                g.sound(rom::SOUND_STRIKE_B, p);
            } else {
                animation::start_plain(g, p, 0xd);
                clear_recognizers(g, p);
                common::weapon_voice(g, p);
            }
            g.sound(ext::sound::STRIKE_LIGHT, p);
        }
        2 => common::wait_then_stand(g, p),
        _ => {}
    }
}

/// Charge (0x401c4): run-up of 12 pixels per frame, gesture, two effects, slide.
fn charge(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, Player::STRIKE_HITS, 0);
            p.set_u8(g, field::MOVE_STATE, 1);
            p.set_u8(g, field::MOVE_KIND, 1);
            common::set_forward_motion(g, p, 0xc00, (-0x98i16) as u16);
            animation::start_plain(g, p, 0x14);
            clear_recognizers(g, p);
        }
        2 => {
            animation::step(g, p);
            if p.u8(g, field::EVENT) != 0 {
                next_step(g, p);
                p.set_u8(g, field::EVENT, 0);
                g.sound(ext::sound::CHARGE_A, p);
                common::weapon_voice(g, p);
                g.sound(ext::sound::CHARGE_B, p);
            }
        }
        4 => common::charge_slide(g, p, |g, p| {
            if p.u8(g, field::EVENT) != 0 {
                p.set_u8(g, field::EVENT, 0);
                spawn::slash(g, p, Some(0));
                spawn::slash(g, p, None);
            }
        }),
        _ => {}
    }
}

/// Strike given while running (0x4028a).
fn run_strike(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, field::MOVE_STATE, 0);
            common::set_forward_motion(g, p, 0x500, (-0x28i16) as u16);
            animation::start_plain(g, p, 0xe);
            clear_recognizers(g, p);
            common::weapon_voice(g, p);
            g.sound(ext::sound::CHARGE_A, p);
        }
        2 => {
            common::run_strike_slide(g, p, 0xf);
        }
        4 => common::wait_then_stand(g, p),
        _ => {}
    }
}

/// Guard strike (0x4039e).
fn guard_break(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, field::MOVE_STATE, 0);
            animation::start_plain(g, p, 0x12);
        }
        2 => common::wait_then_stand(g, p),
        _ => {}
    }
}

/// Spinning jump (0x403be): leap, spin at the top, landing. With the mace (weapon 0x1e)
/// it is a series of five strikes in place (steps 8 to 0xa).
fn leap(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            if p.u8(g, Player::WEAPON_A) == 0x1e {
                leap_mace_begin(g, p);
                return;
            }
            next_step(g, p);
            p.set_u8(g, Player::STRIKE_HITS, 0);
            p.set_u8(g, field::MOVE_STATE, 1);
            p.set_u8(g, field::MOVE_KIND, 1);
            p.set_u32(g, Obj::VX, 0x0080_0000);
            p.set_u16(g, Obj::VZ, 0x560);
            p.set_u16(g, Obj::AZ, (-0x38i16) as u16);
            if p.u8(g, Player::FACING) == 0 {
                common::negate_horizontal_speed(g, p);
            }
            common::start_with_voice(g, p, 0x15);
            clear_recognizers(g, p);
        }
        2 => {
            if animation::step(g, p) {
                next_step(g, p);
                animation::start_plain(g, p, 0x16);
                return;
            }
            if p.u8(g, field::EVENT) != 0 {
                p.set_u8(g, field::EVENT, 0);
                g.sound(ext::sound::LEAP, p);
            }
        }
        4 => {
            move_xz(g, p);
            animation::step(g, p);
            if (p.u16(g, Obj::VZ) as i16) < 0 && (p.u16(g, Obj::Z) as i16) < 0 {
                p.set_u32(g, Obj::Z, 0);
                p.set_u8(g, Player::SLOW, 0);
                to_stand(g, p);
                return;
            }
            if p.u8(g, field::EVENT) != 0 {
                p.set_u8(g, field::EVENT, 0);
                g.sound(rom::SOUND_LEAP_LAND, p);
                common::weapon_voice(g, p);
            }
        }
        6 => leap_mace_begin(g, p),
        8 => {
            animation::step(g, p);
            if p.u8(g, field::EVENT) == 1 {
                p.set_u8(g, field::EVENT, 0);
                g.sound(rom::SOUND_SPIN, p);
            }
            if p.u8(g, field::EVENT) == 2 {
                p.set_u8(g, field::EVENT, 0);
                let strokes = p.u8(g, Player::ACTION_COUNT) + 1;
                p.set_u8(g, Player::ACTION_COUNT, strokes);
                if strokes == 5 {
                    next_step(g, p);
                    animation::start_plain(g, p, 0x1a);
                }
            }
        }
        0xa => {
            if animation::step(g, p) {
                p.set_u8(g, Player::SLOW, 0);
                to_stand(g, p);
            }
        }
        _ => {}
    }
}

/// Start of the five mace strikes in place (0x40492).
fn leap_mace_begin(g: &mut Ctx, p: Player) {
    cov!();
    p.set_u8(g, Player::STEP, 8);
    p.set_u8(g, Player::STRIKE_HITS, 0);
    p.set_u8(g, field::MOVE_STATE, 1);
    p.set_u8(g, field::MOVE_KIND, 1);
    p.set_u8(g, Player::ACTION_COUNT, 0);
    animation::start_plain(g, p, 0x19);
}


/// Turning away the undead (0x40506): the cleric prays, the screen flashes, then the undead on
/// screen (types 1 and 5) flee, in a number limited by the level, unless the cleric has a talisman
/// (`+0x165` = 3, no cap).
fn turn_undead(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            let level = 2 * p.u8(g, Player::LEVEL) as u32;
            let (first, second) = (players("undead_counts").at8(g.b, level), players("undead_counts").at8(g.b, level + 1));
            let (first, second) = if p.u8(g, Player::TALISMAN) == 3 { (0xff, 0xff) } else { (first, second) };
            p.set_u8(g, Player::UNDEAD_LEFT, first);
            p.set_u8(g, Player::UNDEAD_LEFT_B, second);
            animation::start_plain(g, p, 0x21);
        }
        2 => {
            if p.u8(g, field::EVENT) != 0 {
                next_step(g, p);
                p.set_u8(g, Player::ACTIVE, 2);
                if ram::INPUT_MASK.get(g.b) & 0x40 != 0 {
                    ram::INPUT_MASK.set(g.b, 0x7f);
                }
                p.set_u8(g, Player::SPELL_BLINKS, 4);
                p.set_u8(g, Player::SPELL_PHASE, 3);
                p.set_u8(g, Player::STRIKE_LANDED, 1);
                ram::SCROLL1_Y.set(g.b, 0x100);
                // 0x4b518, 0x3b0c, 0x1ac86, 0x51bb6, 0xe27da; the palette copy leaves its longs 2 to 5 in D2-D5
                crate::actors::families::creators::for_players::effect_8(&mut g.leftovers, g.b, p.obj());
                let (longs, _) = crate::world::placement::load_effect_palette(g.b, crate::world::placement::effect_palette::P380);
                (g.leftovers.d2, g.leftovers.d3, g.leftovers.d4, g.leftovers.d5) = (longs[2], longs[3], longs[4], longs[5]);
                crate::services::palette::set_flash_color(g.b, crate::services::palette::FLASH_COLOR_WHITE_VALUE);
                crate::actors::families::creators::for_players::effect_2f(&mut g.leftovers, g.b);
                g.sound(crate::services::sound::BY_KIND, p);
            }
            animation::step(g, p);
        }
        4 => {
            let blink = p.u8(g, Player::SPELL_BLINKS).wrapping_sub(1);
            p.set_u8(g, Player::SPELL_BLINKS, blink);
            if blink != 0 {
                return;
            }
            p.set_u8(g, Player::SPELL_BLINKS, 4);
            ram::SCROLL1_Y.set(g.b, 0);
            let lit = p.u8(g, Player::STRIKE_LANDED) ^ 1;
            p.set_u8(g, Player::STRIKE_LANDED, lit);
            if lit != 0 {
                ram::SCROLL1_Y.set(g.b, 0x100);
            }
            let flashes = p.u8(g, Player::SPELL_PHASE).wrapping_sub(1);
            p.set_u8(g, Player::SPELL_PHASE, flashes);
            if flashes != 0 {
                return;
            }
            next_step(g, p);
            p.set_u8(g, Player::ACTIVE, 1);
            if ram::INPUT_MASK.get(g.b) & 0x40 != 0 {
                ram::INPUT_MASK.set(g.b, 0xff);
            }
            ram::SCROLL1_Y.set(g.b, 0);
            scare_undead(g, p);
        }
        6 => common::wait_then_stand(g, p),
        _ => {}
    }
}

