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
//! their end (`common::begin_spell` / `end_spell`). The original routines that create shots, effects and
//! sounds are still called as they are.

use crate::machine::Machine;
use super::common::{self, field, next_step};
use super::items;
use super::super::spawn;
use super::super::actions::{start_stand, to_stand};
use super::super::combos::clear_recognizers;
use super::super::init::init_weapon_stats;
use super::super::walk::move_xz;
use super::super::{animation, ram, Ctx, Player};
use crate::mem::{wr8, A5};
use crate::objects::pools::{BOSS, ENEMY};
use crate::objects::Obj;
use crate::player::external as ext;

/// Original routines called as they are: player effects, sounds and palettes of the cleric.
mod rom {
    /// Sounds of the strikes.
    pub const SOUND_STRIKE_B: u32 = 0xe8156;
    /// Sounds of the spinning jump.
    pub const SOUND_LEAP_LAND: u32 = 0xe2760;
    pub const SOUND_SPIN: u32 = 0xe81c8;
    /// Prayers: player effects (PEFF pool) created at the gesture, with their sounds.
    pub const PRAYER_EVENT_SOUND: u32 = 0xe2aba;
    pub const PRAYER_AURA_SOUND: u32 = 0xe96e0;
    pub const PRAYER_AURA_B_SOUND: u32 = 0xe9030;
    pub const PRAYER_STORM_SOUND: u32 = 0xe2afa;
    pub const PRAYER_SWARM_START_SOUND: u32 = 0xe2bba;
    pub const SOUND_PRAYER: u32 = 0xe2c7a;
    pub const PRAYER_BANISH_SOUND: u32 = 0xe9e34;
    pub const PRAYER_BANISH_START_SOUND: u32 = 0xe2cba;
    pub const PRAYER_HOLY_SOUND: u32 = 0xe2cfa;
    pub const PRAYER_BLESS_SOUND: u32 = 0xe8e1e;
    pub const PRAYER_BLESS_SOUND_AREA: u32 = 0xe285c;
    pub const PRAYER_BLESS_SOUND_SELF: u32 = 0xe2b3a;
    pub const PRAYER_PROTECT_SOUND: u32 = 0xe2c3a;
    pub const PRAYER_HEAL_SOUND_SELF_A: u32 = 0xe2b7a;
    pub const PRAYER_HEAL_SOUND_SELF_B: u32 = 0xe2bfa;
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
    let mut anim = if p.u8(g, 0x67) == 2 { 0xc } else { 9 };
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
            p.set_u8(g, 0x5b, 0);
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
            p.set_u8(g, 0x5b, 0);
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
                let strokes = p.u8(g, 0x1e8) + 1;
                p.set_u8(g, 0x1e8, strokes);
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
    p.set_u8(g, 0x5b, 0);
    p.set_u8(g, field::MOVE_STATE, 1);
    p.set_u8(g, field::MOVE_KIND, 1);
    p.set_u8(g, 0x1e8, 0);
    animation::start_plain(g, p, 0x19);
}

/// Table (code bytes, 0x4054e): two bytes per level, number of undead turned away by the
/// prayer (monsters of type 1 and type 5).
const UNDEAD_COUNTS: u32 = 0x4054e;

/// Turning away the undead (0x40506): the cleric prays, the screen flashes, then the undead on
/// screen (types 1 and 5) flee, in a number limited by the level, unless the cleric has a talisman
/// (`+0x165` = 3, no cap).
fn turn_undead(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            let level = 2 * p.u8(g, Player::LEVEL) as u32;
            let (first, second) = (g.prog8(UNDEAD_COUNTS + level), g.prog8(UNDEAD_COUNTS + level + 1));
            let (first, second) = if p.u8(g, 0x165) == 3 { (0xff, 0xff) } else { (first, second) };
            p.set_u8(g, 0x1f0, first);
            p.set_u8(g, 0x1f1, second);
            animation::start_plain(g, p, 0x21);
        }
        2 => {
            if p.u8(g, field::EVENT) != 0 {
                next_step(g, p);
                p.set_u8(g, Player::ACTIVE, 2);
                if ram::INPUT_MASK.get(g.b) & 0x40 != 0 {
                    ram::INPUT_MASK.set(g.b, 0x7f);
                }
                p.set_u8(g, 0x1ea, 4);
                p.set_u8(g, 0x1eb, 3);
                p.set_u8(g, 0x1ec, 1);
                ram::SCROLL1_Y.set(g.b, 0x100);
                // 0x4b518, 0x3b0c, 0x1ac86, 0x51bb6, 0xe27da, with the registers they leave
                crate::families::creators::for_players::effect_8_68k(g.c, g.b, p.obj());
                g.c.a[0] = p.0;
                crate::placement::load_effect_palette_regs(g.c, g.b, crate::placement::effect_palette::P380);
                crate::palette::set_flash_color(g.b, crate::palette::FLASH_COLOR_WHITE_VALUE);
                g.c.flags_logic(crate::palette::FLASH_COLOR_WHITE_VALUE as u32, m68k::decode::Size::Word);
                crate::families::creators::for_players::effect_2f_68k(g.c, g.b, p.obj());
                g.sound(0xe27da, p);
            }
            animation::step(g, p);
        }
        4 => {
            let blink = p.u8(g, 0x1ea).wrapping_sub(1);
            p.set_u8(g, 0x1ea, blink);
            if blink != 0 {
                return;
            }
            p.set_u8(g, 0x1ea, 4);
            ram::SCROLL1_Y.set(g.b, 0);
            let lit = p.u8(g, 0x1ec) ^ 1;
            p.set_u8(g, 0x1ec, lit);
            if lit != 0 {
                ram::SCROLL1_Y.set(g.b, 0x100);
            }
            let flashes = p.u8(g, 0x1eb).wrapping_sub(1);
            p.set_u8(g, 0x1eb, flashes);
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

// ---- special attack (0x406d4) --------------------------------------------------------------

/// The cleric's animations for cast spells (see `items::Style`).
const STYLE: items::Style = items::Style { cast_anim: 0x20, bolt_anim: 0x1f, spark: 1, ..items::Style::CLASSIC };

/// 0x406d4: the chosen item or prayer (`field::SPECIAL_KIND`). Numbers 0..5, 11 and 21..35
/// do not exist for the cleric: return to the standing state.
pub fn special(g: &mut Ctx, p: Player) {
    cov!();
    let which = p.u8(g, field::SPECIAL_KIND);
    ram::SPELL_ROW.set(g.b, which);
    match which {
        6 => items::throw_item(g, p, 0x1e, spawn::Thrown::F),
        7 => items::throw_item(g, p, 0x1e, spawn::Thrown::G),
        8 => items::throw_item(g, p, 0x1d, spawn::Thrown::H),
        12 => swap_equipment(g, p),
        0x24 => prayer_hold(g, p),
        0x25 => prayer_storm(g, p),
        0x26 => prayer_swarm(g, p),
        0x27 => prayer_beam(g, p),
        0x28 => prayer_banish(g, p),
        0x29 => prayer_holy_light(g, p),
        0x2a => prayer_protection(g, p),
        0x2b => prayer_blessing(g, p),
        0x2c => prayer_heal(g, p, 0),
        0x2d => prayer_heal(g, p, 1),
        _ => {
            if !items::run_shared(g, p, which, STYLE) {
                to_stand(g, p);
            }
        }
    }
}

/// Equipment swap (0x40944): the picked-up item replaces the weapon; the old weapon takes its place
/// in the item list.
fn swap_equipment(g: &mut Ctx, p: Player) {
    cov!();
    if p.u8(g, Player::STEP) == 0 {
        next_step(g, p);
        animation::start(g, p, 0x32);
        return;
    }
    if animation::step(g, p) {
        to_stand(g, p);
        return;
    }
    if p.u8(g, field::EVENT) != 0 {
        p.set_u8(g, field::EVENT, 0);
        let new = p.u8(g, field::ITEM_NEW).wrapping_sub(0x2e);
        let held = p.u8(g, Player::WEAPON_A);
        p.set_u8(g, field::ITEM, held.wrapping_add(0x2e));
        p.set_u8(g, Player::WEAPON_A, new);
        init_weapon_stats(g, p);
    }
}

// ---- turning away the undead: effect on the monsters (0x40648) -----------------------------------

/// The first 23 slots of the enemy area (the 22 enemies then the first boss): a monster
/// of type 1 in play, or a slot of type 5, receives the mark `+0x8f`: 1 as long as the turn-away
/// counter (`field` 0x1f0 for type 1, 0x1f1 for type 5) is not exhausted (or with the talisman),
/// otherwise 2 (3 from level 10).
fn scare_undead(g: &mut Ctx, p: Player) {
    cov!();
    let level = p.u8(g, Player::LEVEL);
    let talisman = p.u8(g, 0x165) == 3;
    for k in 0..23 {
        let o = Obj(A5 + 0x299e + 0xc0 * k);
        let in_play = o.u8(g.b, 0) != 0 && o.u8(g.b, 1) & 1 != 0;
        let kind = o.u8(g.b, Obj::KIND);
        let counter = if in_play && kind == 1 {
            0x1f0
        } else if kind == 5 {
            0x1f1
        } else {
            continue;
        };
        let left = p.u8(g, counter);
        let mark = if talisman {
            1
        } else if left != 0 {
            p.set_u8(g, counter, left - 1);
            1
        } else if level >= 0xa {
            3
        } else {
            2
        };
        o.set_u8(g.b, 0x8f, mark);
    }
}

// ---- prayers -----------------------------------------------------------------------------------

/// Common start of the prayers: next step, pause, spell indicator, duration, one charge of the reserve
/// (`reserve`: byte of `+0x152..+0x157`), orientation, freezing of the other players.
fn begin_prayer(g: &mut Ctx, p: Player, timer: Option<u16>, reserve: u32) {
    cov!();
    common::begin_spell(g, p, timer);
    let left = p.u8(g, reserve).wrapping_sub(1);
    p.set_u8(g, reserve, left);
    common::face_held_direction(g, p);
    common::freeze_others(g, p);
}

/// One frame of countdown: `field::TIMER` decreases; true when it reaches zero.
fn countdown(g: &mut Ctx, p: Player) -> bool {
    let left = p.u16(g, field::TIMER).wrapping_sub(1);
    p.set_u16(g, field::TIMER, left);
    left == 0
}

/// Prayer cancelled (a protection, `SPELL_CANCELLED`): the countdown goes to 16 frames and the failure
/// spark of kind `spark` is played.
fn cancel_prayer(g: &mut Ctx, p: Player, spark: u16) {
    cov!();
    p.set_u16(g, field::TIMER, 0x10);
    common::item_spark(g, p, spark);
}

/// Table (code bytes): maximum number of monsters hit per level, per prayer.
mod counts {
    pub const HOLD: u32 = 0x4131e;
    pub const CURE: u32 = 0x40ece;
    pub const BANISH: u32 = 0x41afe;
}

/// Table (code words, 0x410a6): duration of the blessing bonus per level, and (0x41e4c) duration of
/// the protection aura.
mod durations {
    pub const BLESSING: u32 = 0x410a6;
    pub const PROTECTION: u32 = 0x41e4c;
}

/// 0x4119c, no. 0x24: holy aura that immobilizes monsters. Animation 0x2a prepares, 0x2b prays; at the
/// gesture the aura is created, then the first monsters designated by `hold_foes` are frozen.
fn prayer_hold(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            begin_prayer(g, p, Some(0x40), 0x152);
            common::dim_screen(g, p);
            animation::start_plain(g, p, 0x2a);
        }
        2 => {
            if countdown(g, p) {
                next_step(g, p);
                p.set_u16(g, field::TIMER, 0x40);
                animation::start_plain(g, p, 0x2b);
                return;
            }
            if p.u8(g, field::EVENT) != 0 {
                p.set_u8(g, field::EVENT, 0);
                spawn::aura(g, p, 0);
                crate::placement::load_effect_palette(g.b, crate::placement::effect_palette::LIGHT);
                g.sound(rom::PRAYER_AURA_SOUND, p);
            } else {
                animation::step(g, p);
            }
        }
        4 => {
            if countdown(g, p) {
                next_step(g, p);
                p.set_u16(g, field::TIMER, 1);
            }
            if p.u8(g, field::EVENT) != 0 {
                g.sound(rom::PRAYER_EVENT_SOUND, p);
                if p.u8(g, field::SPELL_CANCELLED) != 0 {
                    next_step(g, p);
                    cancel_prayer(g, p, 0x12);
                    return;
                }
                p.set_u8(g, field::EVENT, 0);
                spawn::aura(g, p, 1);
                g.sound(rom::PRAYER_AURA_B_SOUND, p);
                hold_foes(g, p);
            }
            animation::step(g, p);
        }
        6 => {
            if countdown(g, p) {
                common::end_spell(g, p);
            }
        }
        _ => {}
    }
}

/// 0x412a8: designates the first monsters (bosses then enemies) that can be hit: in play,
/// alive, unprotected (bit 15 of `+0x94`), not already frozen (`+0x49` = 0x9c). Their maximum number is read
/// per level from a code table. They go to reaction 0x9c, assigned to the cleric.
fn hold_foes(g: &mut Ctx, p: Player) {
    cov!();
    let level = p.u8(g, Player::LEVEL) as u32;
    let mut left = g.prog8(counts::HOLD + level);
    let (index, character) = (p.u8(g, Player::INDEX), p.u8(g, Player::CHARACTER));
    for pool in [&BOSS, &ENEMY] {
        for o in pool.iter() {
            if o.u8(g.b, 0) == 0
                || o.u8(g.b, 1) & 1 == 0
                || o.u16(g.b, 0x62) == 0
                || o.u8(g.b, 0x40) != 0
                || o.u8(g.b, 0x49) == 0x9c
                || o.u32(g.b, 0x94) >> 15 & 1 != 0
            {
                continue;
            }
            o.set_u32(g.b, 4, 0x0400_0000);
            o.set_u8(g.b, 0x49, 0x9c);
            o.set_u8(g.b, 0x48, index);
            o.set_u8(g.b, 0x4b, character);
            o.set_u16(g.b, 0x54, p.0 as u16);
            left = left.wrapping_sub(1);
            if left == 0 {
                break;
            }
        }
    }
}

/// 0x417c2, no. 0x25: storm. A single gesture; the effect (0x4ec66) is born at the first event.
fn prayer_storm(g: &mut Ctx, p: Player) {
    cov!();
    if p.u8(g, Player::STEP) == 0 {
        begin_prayer(g, p, Some(0x60), 0x153);
        common::dim_screen(g, p);
        animation::start_plain(g, p, 0x28);
        return;
    }
    if countdown(g, p) {
        let flags = p.u8(g, Player::FLAGS) & !2;
        p.set_u8(g, Player::FLAGS, flags);
        ram::SCROLL1_Y.set(g.b, 0);
        common::end_spell(g, p);
        return;
    }
    animation::step(g, p);
    if p.u8(g, field::EVENT) == 1 {
        p.set_u8(g, field::EVENT, 0);
        if p.u8(g, field::SPELL_CANCELLED) != 0 {
            cancel_prayer(g, p, 0x16);
            return;
        }
        spawn::storm(g, p, 0);
    }
    if p.u16(g, field::TIMER) == 0x20 {
        g.sound(rom::PRAYER_STORM_SOUND, p);
    }
}

/// 0x40e2e, no. 0x26: swarm. Gesture 0x2c prepares, 0x2d launches as many effects as the level allows
/// (0x4f272); at the first event a healing effect is created.
fn prayer_swarm(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            begin_prayer(g, p, Some(0x40), 0x154);
            common::dim_screen(g, p);
            animation::start_plain(g, p, 0x2c);
        }
        2 => {
            if countdown(g, p) {
                next_step(g, p);
                if p.u8(g, field::SPELL_CANCELLED) == 0 {
                    let level = p.u8(g, Player::LEVEL) as u32;
                    let count = g.prog8(counts::CURE + level).wrapping_sub(1);
                    spawn::swarm(g, p, count as u32 + 1);
                }
                animation::start_plain(g, p, 0x2d);
                g.sound(rom::PRAYER_SWARM_START_SOUND, p);
                return;
            }
            if p.u8(g, field::EVENT) != 0 {
                p.set_u8(g, field::EVENT, 0);
                spawn::beam(g, p, false);
                crate::placement::load_effect_palette(g.b, crate::placement::effect_palette::NORMAL);
                g.sound(ext::sound::PRAYER_QUAKE, p);
            }
            animation::step(g, p);
        }
        4 => {
            if !animation::step(g, p) {
                return;
            }
            if p.u8(g, field::SPELL_CANCELLED) != 0 {
                next_step(g, p);
                cancel_prayer(g, p, 0xf);
                prayer_finish_timer(g, p);
            } else {
                common::end_spell_keep_cancel(g, p);
            }
        }
        6 => prayer_finish_timer(g, p),
        _ => {}
    }
}

/// Common final step: at the end of the countdown, the prayer ends.
fn prayer_finish_timer(g: &mut Ctx, p: Player) {
    cov!();
    if countdown(g, p) {
        common::end_spell(g, p);
    }
}

/// 0x4187e, no. 0x27: holy ray. Gesture 0x30 prays; at the gesture a healing effect is born, then the ray
/// (0x4b5ac, D0 = 1) aims at a monster, stored in `SPELL_TARGET`, which receives the flag `+0x5e` at the end.
fn prayer_beam(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            begin_prayer(g, p, Some(0x40), 0x155);
            common::dim_screen(g, p);
            animation::start_plain(g, p, 0x30);
        }
        2 => {
            animation::step(g, p);
            if p.u8(g, field::EVENT) != 0 {
                p.set_u8(g, field::EVENT, 0);
                spawn::beam(g, p, false);
                crate::placement::load_effect_palette(g.b, crate::placement::effect_palette::NORMAL);
                g.sound(ext::sound::PRAYER_QUAKE, p);
            }
            if !countdown(g, p) {
                return;
            }
            next_step(g, p);
            if p.u8(g, field::SPELL_CANCELLED) == 0 {
                spawn::beam(g, p, true);
                p.set_u16(g, field::SPELL_TARGET, g.c.a[1] as u16);
            }
        }
        4 => {
            next_step(g, p);
            animation::start_plain(g, p, 0x31);
            g.sound(rom::SOUND_PRAYER, p);
        }
        6 => {
            if !animation::step(g, p) {
                return;
            }
            next_step(g, p);
            if p.u8(g, field::SPELL_CANCELLED) != 0 {
                next_step(g, p);
                cancel_prayer(g, p, 0x17);
            } else {
                p.set_u16(g, field::TIMER, 0);
                let beam = p.u16(g, field::SPELL_TARGET) as i16 as i32 as u32 & 0xff_ffff;
                wr8(g.b, beam + 0x5e, 1);
            }
        }
        8 => {
            if p.u16(g, field::TIMER) != 0 {
                common::end_spell_keep_cancel(g, p);
            }
        }
        0xa => prayer_finish_timer(g, p),
        _ => {}
    }
}

/// 0x419b8, no. 0x28: banishment. At the gesture (0x2e) an effect is created; at the end of the countdown the
/// first monsters designated by `banish_foes` are driven away (0x4c26c); without a target the spell is cut short.
fn prayer_banish(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            begin_prayer(g, p, Some(0x28), 0x157);
            common::dim_screen(g, p);
            animation::start_plain(g, p, 0x2e);
        }
        2 => {
            if !countdown(g, p) {
                if p.u8(g, field::EVENT) != 0 {
                    p.set_u8(g, field::EVENT, 0);
                    spawn::banish(g, p, None);
                    crate::placement::load_effect_palette(g.b, crate::placement::effect_palette::LIGHT);
                    g.sound(rom::PRAYER_BANISH_SOUND, p);
                }
                animation::step(g, p);
                return;
            }
            next_step(g, p);
            if p.u8(g, field::SPELL_CANCELLED) != 0 {
                p.set_u16(g, field::TIMER, 0x10);
                common::item_spark(g, p, 0x18);
                common::item_spark(g, p, 0x19);
                return;
            }
            p.set_u16(g, field::TIMER, 0x80);
            if banish_foes(g, p) == 0 {
                p.set_u16(g, field::TIMER, 0x1e);
            }
            animation::start_plain(g, p, 0x2f);
            g.sound(rom::PRAYER_BANISH_START_SOUND, p);
        }
        4 => prayer_finish_timer(g, p),
        _ => {}
    }
}

/// 0x41aa4: designates the first living unprotected monsters (bit 19 of `+0x94`) and applies
/// effect 0x4c26c to them; their maximum number depends on the level. Returns the number of monsters hit. Like
/// the original, the cap only stops the current area (the enemies are always scanned).
fn banish_foes(g: &mut Ctx, p: Player) -> u8 {
    cov!();
    let mut found = 0u8;
    let level = p.u8(g, Player::LEVEL) as u32;
    let mut left = g.prog8(counts::BANISH + level);
    for pool in [&BOSS, &ENEMY] {
        for o in pool.iter() {
            if o.u8(g.b, 0) == 0
                || o.u8(g.b, 1) & 1 == 0
                || o.u16(g.b, 0x62) == 0
                || o.u8(g.b, 0x40) != 0
                || o.u32(g.b, 0x94) >> 19 & 1 != 0
            {
                continue;
            }
            spawn::banish(g, p, Some(o.0));
            found = found.wrapping_add(1);
            left = left.wrapping_sub(1);
            if left == 0 {
                break;
            }
        }
    }
    found
}

/// 0x41b26, no. 0x29: holy light. Gesture 0x32 prays; the effect (0x4aeb2) is born at the gesture; when the
/// countdown (set by the effect) is non-zero, `holy_damage` strikes the monsters.
fn prayer_holy_light(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            begin_prayer(g, p, None, 0x157);
            common::dim_screen(g, p);
            animation::start_plain(g, p, 0x32);
        }
        2 => {
            if animation::step(g, p) {
                if p.u8(g, field::SPELL_CANCELLED) != 0 {
                    p.set_u8(g, Player::STEP, 6);
                    p.set_u16(g, field::TIMER, 0x10);
                    common::item_spark(g, p, 0x1a);
                    common::item_spark(g, p, 0x1b);
                } else {
                    next_step(g, p);
                    p.set_u16(g, field::TIMER, 0);
                    animation::start_plain(g, p, 0x33);
                    g.sound(rom::PRAYER_HOLY_SOUND, p);
                }
            } else if p.u8(g, field::EVENT) != 0 {
                p.set_u8(g, field::EVENT, 0);
                spawn::holy_light(g, p);
                crate::placement::load_effect_palette(g.b, crate::placement::effect_palette::LIGHT);
            }
        }
        4 => {
            if p.u16(g, field::TIMER) != 0 {
                holy_damage(g, p);
                common::end_spell_keep_cancel(g, p);
            } else {
                animation::step(g, p);
            }
        }
        6 => prayer_finish_timer(g, p),
        _ => {}
    }
}

/// Tables (ROM bytes, data): holy light damage per level, for bosses and for
/// enemies.
mod damage {
    pub const BOSS: u32 = 0x10_433c;
    pub const ENEMY: u32 = 0x10_431e;
}

/// 0x41c30: the holy light hurts all unprotected monsters in play on the ground (bit 20 of `+0x94`):
/// they go to reaction 0x10 (assigned to the cleric), lose the hit points from the table (+25% with
/// equipment `+0x166` = 4), non-boss monsters of type 0 or 0x13 die on the spot. The life bar
/// is updated.
fn holy_damage(g: &mut Ctx, p: Player) {
    cov!();
    let level = p.u8(g, Player::LEVEL) as u32;
    let (index, character) = (p.u8(g, Player::INDEX), p.u8(g, Player::CHARACTER));
    for (pool, table) in [(&BOSS, damage::BOSS), (&ENEMY, damage::ENEMY)] {
        for o in pool.iter() {
            if o.u8(g.b, 0) == 0
                || o.u8(g.b, 1) & 1 == 0
                || o.u16(g.b, 0x62) == 0
                || o.u8(g.b, 0x40) != 0
                || o.u16(g.b, 0x0c) != 0
                || o.u32(g.b, 0x94) >> 20 & 1 != 0
            {
                continue;
            }
            o.set_u32(g.b, 4, 0x0400_0000);
            o.set_u8(g.b, 1, 1);
            o.set_u8(g.b, 0x48, index);
            o.set_u8(g.b, 0x49, 0x10);
            o.set_u8(g.b, 0x4b, character);
            o.set_u16(g.b, 0x54, p.0 as u16);
            o.set_u8(g.b, 0x2c, 0);
            if g.random() & 1 != 0 {
                o.set_u8(g.b, 0x2c, 0x20);
            }
            let facing = o.u8(g.b, 0x2c);
            o.set_u8(g.b, 0x4a, facing);
            let counter = p.u16(g, 0x190).wrapping_add(2);
            p.set_u16(g, 0x190, counter);
            let hits = o.u8(g.b, 0x9c).wrapping_add(1);
            o.set_u8(g.b, 0x9c, hits);
            let mut damage = g.data8(table + level) as u16;
            if p.u8(g, 0x166) == 4 {
                damage = damage.wrapping_add(damage >> 2);
            }
            let hp = (o.u16(g.b, 0x62) as i16).wrapping_sub(damage as i16);
            o.set_u16(g.b, 0x62, if hp < 0 { 0 } else { hp as u16 });
            if o.u8(g.b, 0x28) != 4 && matches!(o.u8(g.b, Obj::KIND), 0 | 0x13) {
                o.set_u16(g.b, 0x62, 0);
            }
            if o.u16(g.b, 0x62) == 0 && o.u8(g.b, 0x93) != 0 {
                p.set_u8(g, 0x18a, 1);
                let loot = o.u8(g.b, 0x9a);
                ram::KILL_FLAG.set(g.b, loot);
            }
            let bar = (index as u32) << 8 | o.u8(g.b, Obj::KIND) as u32;
            let (max, hp, class) = (o.u16(g.b, 0x60) as u32, o.u16(g.b, 0x62) as u32, o.u8(g.b, 0x28) as u32);
            crate::events::create_popup(g.b, crate::events::Popup { header: bar as u16, max: max as u16, value: hp as u16, raw: class as u8 == 2 });
        }
    }
}

/// 0x41d48, no. 0x2a: protection. A protection aura (duration per level) is placed on all
/// living players at the end of the spell.
fn prayer_protection(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            begin_prayer(g, p, Some(0x58), 0x152);
            common::dim_screen(g, p);
            start_stand(g, p);
        }
        2 => {
            next_step(g, p);
            if p.u8(g, field::SPELL_CANCELLED) != 0 {
                p.set_u8(g, Player::STEP, 6);
                p.set_u16(g, field::TIMER, 0x20);
            }
            animation::start_plain(g, p, 0x26);
        }
        4 => {
            if countdown(g, p) {
                protect_players(g, p);
                return;
            }
            animation::step(g, p);
            if p.u8(g, field::EVENT) == 1 {
                p.set_u8(g, field::EVENT, 0);
                spawn::blessing(g, p, 2);
                crate::placement::load_effect_palette(g.b, crate::placement::effect_palette::LIGHT);
                g.sound(rom::PRAYER_BLESS_SOUND, p);
                g.sound(rom::PRAYER_PROTECT_SOUND, p);
            }
        }
        6 => {
            if countdown(g, p) {
                common::end_spell(g, p);
            } else if p.u16(g, field::TIMER) == 0x10 {
                common::item_spark(g, p, 0x13);
                common::item_spark(g, p, 0x14);
            }
        }
        _ => {}
    }
}

/// 0x41de4: end of the protection: the aura (`AURA_TIMER` = duration per level, `AURA_C` = 1) is placed on
/// each living player who is not blocked by an object.
fn protect_players(g: &mut Ctx, p: Player) {
    cov!();
    common::clear_spell_state(g);
    p.set_u8(g, field::SPELL_CANCELLED, 0);
    let level = p.u8(g, Player::LEVEL) as u32;
    let duration = g.prog16(durations::PROTECTION + 2 * level);
    for q in Player::all() {
        if q.u8(g, Player::ACTIVE) != 0 && q.u16(g, Player::HP) != 0 && q.u8(g, Player::POWER_10F) == 0 {
            q.set_u16(g, Player::AURA_TIMER, duration);
            q.set_u8(g, Player::AURA_C, 1);
            q.set_u8(g, Player::AURA_B, 0);
            q.set_u8(g, Player::AURA, 0);
        }
    }
    common::undim_screen(g, p);
    to_stand(g, p);
}

/// 0x40f6c, no. 0x2b: blessing. The cleric (or the first ally in range) receives a temporary power
/// bonus (`POWERUP_TIMER`, duration per level); without an ally the cleric blesses themselves.
fn prayer_blessing(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            begin_prayer(g, p, Some(0x58), 0x153);
            common::dim_screen(g, p);
            start_stand(g, p);
        }
        2 => {
            next_step(g, p);
            bless_ally(g, p);
            if p.u8(g, field::SPELL_CANCELLED) != 0 {
                p.set_u8(g, Player::STEP, 6);
            }
            let area = p.u8(g, field::AREA) != 0;
            animation::start_plain(g, p, if area { 0x23 } else { 0x22 });
        }
        4 => {
            if countdown(g, p) {
                end_blessing(g, p);
                return;
            }
            animation::step(g, p);
            if p.u8(g, field::EVENT) == 1 {
                let area = p.u8(g, field::AREA);
                spawn::blessing(g, p, area);
                crate::placement::load_effect_palette(g.b, crate::placement::effect_palette::LIGHT);
                g.sound(rom::PRAYER_BLESS_SOUND, p);
                g.sound(if area != 0 { rom::PRAYER_BLESS_SOUND_AREA } else { rom::PRAYER_BLESS_SOUND_SELF }, p);
            }
        }
        6 => {
            if animation::step(g, p) {
                next_step(g, p);
                p.set_u16(g, field::TIMER, 0x10);
                let spark = if p.u8(g, field::AREA) != 0 { 0x11 } else { 0x10 };
                common::item_spark(g, p, spark);
            }
        }
        8 => prayer_finish_timer(g, p),
        _ => {}
    }
}

/// 0x410d8: chooses the blessed ally (see `common::find_ally`, range 0x4c): they go to the blessing
/// pose (state 0x02001e00) and receive the bonus duration; without an ally the cleric receives the bonus.
fn bless_ally(g: &mut Ctx, p: Player) {
    cov!();
    let level = p.u8(g, Player::LEVEL) as u32;
    let duration = g.prog16(durations::BLESSING + 2 * level);
    let ally = common::find_ally(g, p, 0x4c, |_, _| ());
    if let Some(q) = ally {
        if p.u8(g, field::SPELL_CANCELLED) == 0 {
            q.set_state(g, 0x0200_1e00);
            q.set_u16(g, Player::POWERUP_TIMER, duration);
            q.set_u8(g, field::SPELL_FROZEN, 0);
            q.set_u8(g, Player::FACING, 0);
            if p.u16(g, Obj::X) > q.u16(g, Obj::X) {
                q.set_u8(g, Player::FACING, 0x20);
            }
        }
    }
    p.set_u8(g, field::AREA, ally.is_some() as u8);
    if ally.is_none() {
        p.set_u16(g, Player::POWERUP_TIMER, duration);
    }
}

/// 0x4101a: end of the blessing: pause and freeze lifted; players who have a bonus duration
/// receive the bonus flag (`POWERUP`).
fn end_blessing(g: &mut Ctx, p: Player) {
    cov!();
    ram::SPELL_FLAG.set(g.b, 0);
    ram::PAUSE.set(g.b, 0);
    for q in Player::all() {
        q.set_u8(g, field::SPELL_FROZEN, 0);
        if q.u16(g, Player::POWERUP_TIMER) != 0 {
            q.set_u8(g, Player::POWERUP, 1);
        }
    }
    common::undim_screen(g, p);
    to_stand(g, p);
}

/// 0x41346 (no. 0x2c) and 0x4149c (no. 0x2d): healing. `variant` 0 or 1. The cleric (or the ally in range, see
/// `common::heal_scan`) is healed by `common::apply_heal` at the end.
fn prayer_heal(g: &mut Ctx, p: Player, variant: u32) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            begin_prayer(g, p, Some(0x60), 0x154 + variant);
            p.set_u8(g, 0x1ed, 0);
            common::dim_screen(g, p);
            start_stand(g, p);
        }
        2 => {
            next_step(g, p);
            common::heal_scan(g, p);
            if p.u8(g, field::SPELL_CANCELLED) != 0 {
                p.set_u8(g, Player::STEP, 6);
                p.set_u16(g, field::TIMER, 0x20);
            }
            let area = p.u8(g, field::AREA) != 0;
            animation::start_plain(g, p, if area { 0x25 } else { 0x24 });
        }
        4 => {
            if countdown(g, p) {
                common::apply_heal(g, p);
                let flags = p.u8(g, Player::FLAGS) & !2;
                p.set_u8(g, Player::FLAGS, flags);
                common::end_spell(g, p);
                return;
            }
            animation::step(g, p);
            common::heal_flicker(g, p);
            if p.u8(g, field::EVENT) == 1 {
                p.set_u8(g, field::EVENT, 0);
                spawn::cure(g, p, variant as u8);
                crate::placement::load_effect_palette(g.b, crate::placement::effect_palette::NORMAL);
                let sound = if p.u8(g, field::AREA) != 0 {
                    rom::PRAYER_BLESS_SOUND_AREA
                } else if variant == 0 {
                    rom::PRAYER_HEAL_SOUND_SELF_A
                } else {
                    rom::PRAYER_HEAL_SOUND_SELF_B
                };
                g.sound(sound, p);
            }
        }
        6 => {
            if countdown(g, p) {
                common::end_spell(g, p);
            } else if p.u8(g, field::AREA) == 0 {
                if p.u16(g, field::TIMER) == 0x10 {
                    common::item_spark(g, p, 0x13);
                    common::item_spark(g, p, 0x14);
                }
            } else {
                if p.u16(g, field::TIMER) == 0x10 {
                    common::item_spark(g, p, 0x15);
                }
                animation::step(g, p);
            }
        }
        _ => {}
    }
}

action_native!(JUMP, 0x3fd08, "cleric_jump", jump);
action_native!(ATTACK, 0x3ff2a, "cleric_attack", attack);
action_native!(SPECIAL, 0x406d4, "cleric_special", special);
action_native!(DASH, 0x41f22, "cleric_dash", common::dash);
