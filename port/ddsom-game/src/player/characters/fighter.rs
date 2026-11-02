//! The fighter (characters 0 and 1: both variants have the same code).
//!
//! Four actions, reached through `characters::run`:
//! - `jump` (0x3e578): three-phase jump (crouch, rise, descent) with aerial attack;
//! - `attack` (0x3e7e4): button 1; `Player::SUBACTION` selects the strike (see `kind`);
//! - `special` (0x3f0ae): the chosen item or spell (`field::SPECIAL_KIND`);
//! - `dash` (0x3f9bc): action 0x10, charge then slide.
//!
//! Each action is an automaton whose step is `Player::STEP` (even). The shots, effects, sounds
//! and projectiles are created by Rust calls (`spawn`, `families`; the sounds by their identifier, `rom` constants).

use super::common::{self, field, next_step, set_forward_motion};
use super::items;
use crate::mem::rd32;
use super::super::spawn;
use super::super::actions::to_stand;
use super::super::checks::item_curse;
use super::super::combos::clear_recognizers;
use super::super::init::init_weapon_stats;
use super::super::walk::{move_xz, move_z};
use super::super::{animation, Ctx, Player};
use crate::objects::Obj;
use crate::player::ram;
use crate::player::external as ext;

/// Identifiers (original addresses) of the routines reached: player effects and sounds of the fighter.
mod rom {
    /// Sounds.
    pub const SOUND_SPIN_B: crate::sound::Sound = crate::sound::CMD_140_240;
}

/// Fighter table (ROM data): jump speeds (four words per jump type).
const JUMP_SPEEDS: u32 = 0x10_44fe;

/// Sub-actions of button 1 (`Player::SUBACTION`), all even.
mod kind {
    /// Ground strike: chain of increasingly long strikes.
    pub const COMBO: u8 = 0x00;
    /// Thrust (an enemy is within range of the first attack box).
    pub const THRUST_HIGH: u8 = 0x02;
    /// Low strike (second attack box).
    pub const THRUST_LOW: u8 = 0x04;
    /// Strike given while turning.
    pub const TURN_STRIKE: u8 = 0x06;
    /// Strike given in slow walk.
    pub const SLOW_STRIKE: u8 = 0x08;
    /// Aerial strike (landing), follow-up of a jump.
    pub const AFTER_JUMP: u8 = 0x0a;
    /// Charge: run-up of 12 pixels per frame followed by two gestures.
    pub const CHARGE: u8 = 0x0c;
    /// Strike given while running.
    pub const RUN_STRIKE: u8 = 0x0e;
    pub const LOW_BLOW: u8 = 0x10;
    /// Backward jump.
    pub const BACK_HOP: u8 = 0x12;
    pub const GUARD_BREAK: u8 = 0x14;
    /// Great spinning jump (the fighter's special attack).
    pub const LEAP: u8 = 0x16;
    /// Whirlwind (sacrifices hit points).
    pub const WHIRL: u8 = 0x18;
    /// Forward gesture without a weapon.
    pub const POSE_A: u8 = 0x1a;
    pub const POSE_B: u8 = 0x1c;
}

// ---- jump (0x3e578) --------------------------------------------------------------------------

/// The fighter's jump: takeoff speeds in the table 0x1044fe, own aerial strike, return
/// of the cursed weapon and creation of the pending player effect.
const JUMP_KIT: common::JumpKit = common::JumpKit {
    speeds: JUMP_SPEEDS,
    air_attack,
    after_recover: common::curse_check,
    finish: common::flush_pending_effect,
    ..common::JumpKit::PLAIN
};

/// 0x3e578: jump.
pub fn jump(g: &mut Ctx, p: Player) {
    cov!();
    common::jump(g, p, &JUMP_KIT);
}

/// 0x3e754: button 1 in the air gives a strike (once per jump) whose animation depends on the jump
/// type, the weapon and the direction (down: diving strike).
fn air_attack(g: &mut Ctx, p: Player) {
    cov!();
    if !common::air_attack_pressed(g, p) {
        return;
    }
    p.set_u8(g, field::MOVE_STATE, 0);
    p.set_u8(g, field::AIR_ATTACK_DONE, 1);
    let heavy = common::holds_heavy_second(g, p);
    let mut anim = if heavy { 9 } else { 0x2a };
    if p.u8(g, 0x67) == 2 {
        anim = if heavy { 0xc } else { 0x2b };
    }
    if common::holds_special_weapon(g, p) {
        anim = 0x32;
    }
    if p.u8(g, Player::KEYS_HELD) & 4 != 0 {
        anim = 0xb;
    }
    common::start_with_voice(g, p, anim);
    item_curse(g, p);
    common::aura_hit_countdown(g, p);
}

// ---- attack (0x3e7e4) -----------------------------------------------------------------------

/// 0x3e7e4: button 1 attack. `kind` (see above) selects the strike; ground strikes are followed
/// by a continuation check (`common::follow_up`).
pub fn attack(g: &mut Ctx, p: Player) {
    cov!();
    let sub = p.u8(g, Player::SUBACTION);
    match sub {
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
        kind::BACK_HOP => common::back_hop(g, p, 0x1f, false),
        kind::GUARD_BREAK => guard_break(g, p),
        kind::LEAP => leap(g, p),
        kind::WHIRL => whirl(g, p),
        kind::POSE_A | kind::POSE_B => {
            common::pose(g, p, 0x1a);
            common::follow_up(g, p);
        }
        _ => {}
    }
    common::flush_pending_effect(g, p);
}

/// With the particular weapon (8, 0x18) all strikes become the basic strike.
fn fall_back_to_combo(g: &mut Ctx, p: Player) -> bool {
    cov!();
    if !common::holds_special_weapon(g, p) {
        return false;
    }
    p.set_u8(g, Player::SUBACTION, kind::COMBO);
    true
}

/// Start of a basic strike (0x3e86e): the chain rank advances if the previous strike landed
/// (two strikes, four with heavy equipment).
fn combo_begin(g: &mut Ctx, p: Player) {
    cov!();
    next_step(g, p);
    p.set_u8(g, field::MOVE_STATE, 0);
    p.set_u8(g, field::COMBO_RELEASED, 0);
    p.set_u8(g, field::COMBO_WINDOW, 6);
    let heavy = common::holds_heavy_second(g, p);
    let rank = common::chain_rank(g, p, 1, if heavy { 3 } else { 1 });
    let anim = if common::holds_special_weapon(g, p) { 0x30 } else { (if heavy { 0 } else { 0x26 }) + rank as u16 };
    common::start_with_voice(g, p, anim);
}

/// Basic strike (0x3e858): three steps.
fn combo(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => combo_begin(g, p),
        2 => {
            common::combo_follow(g, p, 0x1a);
        }
        4 => common::wait_then_stand(g, p),
        _ => {}
    }
}


/// Thrusts (0x3e930 high, 0x3e998 low): a single gesture, of rank 0 or 1 depending on the previous strike.
fn thrust(g: &mut Ctx, p: Player, hit_bit: u8, base_anim: u16) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            p.set_u8(g, field::MOVE_STATE, 0);
            if fall_back_to_combo(g, p) {
                return combo_begin(g, p);
            }
            next_step(g, p);
            let rank = common::chain_rank(g, p, hit_bit, 1);
            common::start_with_voice(g, p, base_anim + rank as u16);
        }
        2 => common::wait_then_stand(g, p),
        _ => {}
    }
}

/// Strike given while turning (0x3ea00): step 2 is step 0 of a per-state entry (combos).
fn turn_strike(g: &mut Ctx, p: Player) {
    cov!();
    let step = p.u8(g, Player::STEP);
    match step {
        0 | 2 => {
            if step == 0 {
                p.set_u8(g, field::MOVE_STATE, 0);
            }
            if fall_back_to_combo(g, p) {
                p.set_u8(g, field::HITS, 0);
                return combo_begin(g, p);
            }
            p.set_u8(g, Player::STEP, 4);
            common::start_with_voice(g, p, if step == 0 { 0x10 } else { 0x1e });
        }
        4 => common::wait_then_stand(g, p),
        _ => {}
    }
}

/// Strike given in slow walk (0x3ea82): without continuation.
fn slow_strike(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            p.set_u8(g, field::MOVE_STATE, 0);
            if fall_back_to_combo(g, p) {
                p.set_u8(g, field::HITS, 0);
                p.set_u8(g, Player::SLOW, 0);
                return combo_begin(g, p);
            }
            next_step(g, p);
            let anim = if common::holds_heavy_second(g, p) { 0x13 } else { 0x29 };
            common::start_with_voice(g, p, anim);
        }
        2 => common::wait_then_stand(g, p),
        _ => {}
    }
}

/// Landing strike (0x3eae6).
fn after_jump(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, field::MOVE_STATE, 0);
            let anim = if common::holds_special_weapon(g, p) { 0x31 } else { 0xd };
            common::start_with_voice(g, p, anim);
            g.sound(ext::sound::STRIKE_HEAVY, p);
        }
        2 => common::wait_then_stand(g, p),
        _ => {}
    }
}

/// Charge (0x3eb3c): run-up, first gesture (landed hit, effect), second gesture then slide.
fn charge(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, 0x5b, 0);
            p.set_u8(g, field::MOVE_STATE, 1);
            p.set_u8(g, field::MOVE_KIND, 1);
            set_forward_motion(g, p, 0xc00, (-0x98i16) as u16);
            let anim = if common::holds_heavy_second(g, p) { 0x14 } else { 0x28 };
            animation::start_plain(g, p, anim);
            clear_recognizers(g, p);
        }
        2 => {
            animation::step(g, p);
            if p.u8(g, field::EVENT) != 0 {
                next_step(g, p);
                p.set_u8(g, field::EVENT, 0);
                common::weapon_voice(g, p);
                g.sound(ext::sound::CHARGE_A, p);
                g.sound(ext::sound::CHARGE_B, p);
            }
        }
        4 => common::charge_slide(g, p, |g, p| match p.u8(g, field::EVENT) {
            1 => {
                p.set_u8(g, field::EVENT, 0);
                spawn::slash(g, p, None);
            }
            2 => {
                p.set_u8(g, field::EVENT, 0);
                spawn::slash(g, p, Some(0));
                if !common::holds_heavy_second(g, p) {
                    spawn::slash(g, p, Some(1));
                }
            }
            _ => {}
        }),
        _ => {}
    }
}


/// Strike given while running (0x3ec2c): run-up of 5 pixels per frame, then stop.
fn run_strike(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, field::MOVE_STATE, 0);
            set_forward_motion(g, p, 0x500, (-0x28i16) as u16);
            let anim = if common::holds_heavy_second(g, p) { 0xe } else { 0x2c };
            animation::start_plain(g, p, anim);
            common::weapon_voice(g, p);
            g.sound(ext::sound::CHARGE_A, p);
            clear_recognizers(g, p);
        }
        2 => {
            common::run_strike_slide(g, p, 0xf);
        }
        4 => common::wait_then_stand(g, p),
        _ => {}
    }
}



/// Guard strike (0x3ed40).
fn guard_break(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            p.set_u8(g, field::MOVE_STATE, 0);
            if fall_back_to_combo(g, p) {
                p.set_u8(g, field::HITS, 0);
                return combo_begin(g, p);
            }
            next_step(g, p);
            common::start_with_voice(g, p, 0x12);
        }
        2 => common::wait_then_stand(g, p),
        _ => {}
    }
}

/// Great spinning jump (0x3ed86), special attack 0x2d: leap, vertical whirlwind, landing.
/// With the light weapon (`WEAPON_B` < 0x28), simplified jump that starts at step 0xe.
fn leap(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            clear_recognizers(g, p);
            if common::holds_heavy_second(g, p) {
                next_step(g, p);
                p.set_u8(g, 0x5b, 0);
                p.set_u8(g, field::MOVE_STATE, 1);
                p.set_u8(g, field::MOVE_KIND, 1);
                p.set_u16(g, Obj::VX, 0x200);
                p.set_u16(g, Obj::AX, 0x20);
                p.set_u16(g, Obj::VZ, 0x600);
                p.set_u16(g, Obj::AZ, (-0x60i16) as u16);
                p.set_u16(g, Obj::AY, 0);
                if p.u8(g, Player::FACING) == 0 {
                    common::negate_horizontal_speed(g, p);
                }
                animation::start_plain(g, p, 0x15);
            } else {
                leap_light_begin(g, p);
            }
        }
        2 => leap_spin(g, p),
        4 | 8 => leap_descend(g, p),
        6 => leap_fall(g, p),
        0xa => {
            if animation::step(g, p) {
                p.set_u8(g, Player::SLOW, 0);
                to_stand(g, p);
            }
        }
        0xc => leap_light_begin(g, p),
        0xe => leap_light_wait(g, p),
        0x10 => leap_light_land(g, p),
        0x12 => {
            if animation::step(g, p) {
                p.set_u8(g, Player::SLOW, 0);
                to_stand(g, p);
            }
        }
        _ => {}
    }
}

/// Light variant of the leap (0x3ef04): arrival at step 0xe.
fn leap_light_begin(g: &mut Ctx, p: Player) {
    cov!();
    p.set_u8(g, Player::STEP, 0xe);
    p.set_u8(g, 0x5b, 0);
    p.set_u8(g, field::MOVE_STATE, 1);
    p.set_u8(g, field::MOVE_KIND, 1);
    p.set_u16(g, Obj::VX, 0x200);
    p.set_u16(g, Obj::AX, 0);
    p.set_u16(g, Obj::VZ, 0x600);
    p.set_u16(g, Obj::AZ, (-0x40i16) as u16);
    p.set_u16(g, Obj::AY, 0);
    if p.u8(g, Player::FACING) == 0 {
        common::negate_horizontal_speed(g, p);
    }
    animation::start_plain(g, p, 0x2e);
}

/// The light leap waits for the gesture (event) then continues the animation (0x3ef44).
fn leap_light_wait(g: &mut Ctx, p: Player) {
    cov!();
    if p.u8(g, field::EVENT) != 0 {
        next_step(g, p);
        p.set_u8(g, field::EVENT, 0);
        spawn::leap(g, p, 0);
        g.sound(ext::sound::LEAP, p);
    }
    animation::step(g, p);
}

/// Landing of the light leap (0x3ef66).
fn leap_light_land(g: &mut Ctx, p: Player) {
    cov!();
    move_xz(g, p);
    if (p.u16(g, Obj::Z) as i16) < 0 {
        next_step(g, p);
        p.set_u32(g, Obj::Z, 0);
        animation::start_plain(g, p, 0x2f);
        g.sound(ext::sound::LANDING, p);
        return;
    }
    animation::step(g, p);
}

/// Step 2 of the heavy leap (0x3ee00): at the peak frame, holding button 1 triggers the descending
/// whirlwind; the carried gesture creates an effect and a voice.
fn leap_spin(g: &mut Ctx, p: Player) {
    cov!();
    let apex_frame = if p.u8(g, Obj::VARIANT) == 0 { 0x2b_8d5e } else { 0x2b_fa50 };
    if rd32(g.b, p.at(Obj::FRAME)) == apex_frame {
        if p.u8(g, Player::KEYS_HELD) & 0x10 != 0 {
            p.set_u16(g, Obj::AY, 1);
        }
        move_xz(g, p);
        if (p.u16(g, Obj::VZ) as i16) < 0 {
            if p.u16(g, Obj::AY) != 0 {
                p.set_u8(g, Player::STEP, 6);
                p.set_u16(g, Obj::AY, 0);
                p.set_u16(g, Obj::VZ, 0x600);
                animation::start_plain(g, p, 0x17);
            } else {
                next_step(g, p);
                animation::start_plain(g, p, 0x16);
            }
            return;
        }
    }
    if p.u8(g, field::EVENT) != 0 {
        p.set_u8(g, field::EVENT, 0);
        spawn::leap(g, p, 0);
        g.sound(ext::sound::LEAP, p);
        common::weapon_voice(g, p);
        g.sound(ext::sound::LEAP_END, p);
    }
    animation::step(g, p);
}

/// Steps 4 and 8 of the heavy leap (0x3ee8c): vertical descent, landing on the ground.
fn leap_descend(g: &mut Ctx, p: Player) {
    cov!();
    move_z(g, p);
    if (p.u16(g, Obj::Z) as i16) < 0 {
        p.set_u8(g, Player::STEP, 0xa);
        p.set_u32(g, Obj::Z, 0);
        animation::start_plain(g, p, 0x19);
        g.sound(ext::sound::LANDING, p);
        return;
    }
    animation::step(g, p);
}

/// Step 6 of the heavy leap (0x3eeb6): second whirlwind.
fn leap_fall(g: &mut Ctx, p: Player) {
    cov!();
    move_z(g, p);
    if (p.u16(g, Obj::VZ) as i16) < 0 {
        next_step(g, p);
        animation::start_plain(g, p, 0x18);
        return;
    }
    if p.u8(g, field::EVENT) != 0 {
        p.set_u8(g, field::EVENT, 0);
        spawn::leap(g, p, 0);
        common::weapon_voice(g, p);
    }
    animation::step(g, p);
}

/// Whirlwind (0x3efa2): costs 12 hit points at the first contact.
fn whirl(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, 0x5b, 0);
            p.set_u8(g, field::MOVE_STATE, 1);
            p.set_u8(g, field::MOVE_KIND, 2);
            p.set_u8(g, 0x58, 0);
            p.set_u8(g, 0x1ec, 0);
            let anim = if p.u16(g, Obj::Z) != 0 { 0x1c } else { 0x1b };
            animation::start_plain(g, p, anim);
            g.sound(ext::sound::SPIN, p);
            g.sound(rom::SOUND_SPIN_B, p);
            clear_recognizers(g, p);
        }
        2 => {
            if animation::step(g, p) {
                p.set_u8(g, Player::SLOW, 0);
                to_stand(g, p);
                return;
            }
            if p.u8(g, field::EVENT) != 0 {
                p.set_u8(g, field::EVENT, 0);
                spawn::blood(g, p);
            }
            if p.u8(g, 0x1ec) == 0 && p.u8(g, 0x58) & 1 != 0 {
                p.set_u8(g, 0x1ec, 1);
                let hp = (p.u16(g, Player::HP) as i16).wrapping_sub(0xc);
                p.set_u16(g, Player::HP, if hp > 0 { hp as u16 } else { 1 });
            }
        }
        _ => {}
    }
}


// ---- special attack (0x3f0ae) --------------------------------------------------------------

/// The fighter's animations for cast spells (see `items::Style`).
const STYLE: items::Style = items::Style { cast_anim: 0x25, bolt_anim: 0x24, spark: 0, ..items::Style::CLASSIC };

/// 0x3f0ae: the chosen item or spell (`field::SPECIAL_KIND`, 0..20); numbers 3 and 11 do
/// nothing. 0..8: thrown items; 12: equipment swap; 9, 10 and 13..20: see `items`.
pub fn special(g: &mut Ctx, p: Player) {
    cov!();
    let which = p.u8(g, field::SPECIAL_KIND);
    ram::SPELL_ROW.set(g.b, which);
    match which {
        0 => items::throw_item(g, p, 0x22, spawn::Thrown::A),
        1 => items::throw_item(g, p, 0x22, spawn::Thrown::B),
        2 => items::throw_item(g, p, 0x23, spawn::Thrown::C),
        4 => items::throw_item(g, p, 0x23, spawn::Thrown::D),
        5 => items::throw_item(g, p, 0x23, spawn::Thrown::E),
        6 => items::throw_item(g, p, 0x21, spawn::Thrown::F),
        7 => items::throw_item(g, p, 0x21, spawn::Thrown::G),
        8 => items::throw_item(g, p, 0x20, spawn::Thrown::H),
        12 => swap_equipment(g, p),
        _ => {
            items::run_shared(g, p, which, STYLE);
        }
    }
}

/// Equipment swap (0x3f39a): the picked-up item (`field::ITEM_NEW`) replaces the weapon or the shield
/// and the old equipment takes its place in the item list (`field::ITEM`).
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
    match p.u8(g, field::EVENT) {
        1 => {
            p.set_u8(g, field::EVENT, 0);
            let new = p.u8(g, field::ITEM_NEW).wrapping_sub(0x2e);
            let second = p.u8(g, Player::WEAPON_B);
            if second <= 7 && second & 4 != 0 {
                // the shield in hand is replaced by the heavy equipment 0x28 + protection level
                let protection = p.u8(g, 0x164);
                p.set_u8(g, Player::WEAPON_B, 0x28u8.wrapping_add(protection));
                p.set_u8(g, field::ITEM, second.wrapping_add(0x2e));
                p.set_u8(g, Player::WEAPON_A, new);
            } else if new <= 7 && new & 4 != 0 {
                // a shield is put on: the held weapon becomes the list item
                let held = p.u8(g, Player::WEAPON_A);
                p.set_u8(g, field::ITEM, held.wrapping_add(0x2e));
                p.set_u8(g, Player::WEAPON_B, new);
                p.set_u8(g, Player::WEAPON_A, new.wrapping_sub(4));
            } else {
                let held = p.u8(g, Player::WEAPON_A);
                p.set_u8(g, field::ITEM, held.wrapping_add(0x2e));
                p.set_u8(g, Player::WEAPON_A, new);
            }
            init_weapon_stats(g, p);
        }
        2 => {
            p.set_u8(g, field::EVENT, 0);
            spawn::swap(g, p, 0);
        }
        _ => {}
    }
}

action_native!(JUMP, 0x3e578, "fighter_jump", jump);
action_native!(ATTACK, 0x3e7e4, "fighter_attack", attack);
action_native!(SPECIAL, 0x3f0ae, "fighter_special", special);
action_native!(DASH, 0x3f9bc, "fighter_dash", common::dash);
