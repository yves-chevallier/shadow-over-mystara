//! The elf (characters 4 and 5: both variants have the same code).
//!
//! Four actions, reached through `characters::run`:
//! - `jump` (0x42012): the common jump (`common::jump`) with two aerial gestures (strike, shot);
//! - `attack` (0x4241c): button 1; `Player::SUBACTION` selects the strike (see `kind`);
//! - `special` (0x42b0c): the chosen item or spell (`field::SPECIAL_KIND`, 0..0x1c);
//! - action 0x10 (0x444ca) is the fighter's slide (`common::dash`).
//!
//! The thrown items, lightning, the wall and spells 13..20 are the fighter's (`items`) with the
//! elf's animations and a final pose; spells 0x15..0x1c are its own. The shots, effects
//! and sounds are created by Rust calls.

use super::common::{self, countdown, field, is_cancelled, next_step, take_event, wind_down};
use super::foes::{self, Boost, Damage, Live, Strike};
use super::items;
use super::super::spawn;
use super::super::actions::to_stand;
use super::super::checks::item_curse;
use super::super::combos::clear_recognizers;
use super::super::walk::{move_x, move_xz};
use super::super::{animation, Ctx, Player};
use crate::objects::Obj;
use crate::player::ram;
use crate::machine::Machine;
use crate::player::external as ext;

/// Identifiers (original addresses) of the routines reached: player effects, sounds, palettes and screen effects.
mod rom {
    /// Whirl sounds (the effect: `creators::for_players::whirl`).
    pub const WHIRL_SOUND_A: crate::sound::Sound = crate::sound::CMD_141_241;
    /// Sounds of the spells, in the call order of each spell.
    pub const WARD_SOUNDS: [crate::sound::Sound; 2] = [crate::sound::CMD_E8, crate::sound::CMD_17F];
    pub const WARD_END_SOUNDS: [crate::sound::Sound; 2] = [crate::sound::CMD_64, crate::sound::CMD_172];
    pub const SHIELD_SOUNDS: [crate::sound::Sound; 2] = [crate::sound::CMD_E8, crate::sound::CMD_171];
    pub const SHIELD_SECOND_SOUNDS: [crate::sound::Sound; 2] = [crate::sound::CMD_65, crate::sound::CMD_172];
    pub const BEAM_SOUND: crate::sound::Sound = crate::sound::CMD_16F;
    pub const BEAM_SECOND_SOUND: crate::sound::Sound = crate::sound::CMD_66;
    pub const SPARK_SOUND: crate::sound::Sound = crate::sound::CMD_160;
    pub const SPARK_END_SOUND: crate::sound::Sound = crate::sound::CMD_60;
    pub const BOLT_SOUND: crate::sound::Sound = crate::sound::CMD_E8;
    pub const BOLT_SECOND_SOUND: crate::sound::Sound = crate::sound::CMD_167;
    pub const BOLT_END_SOUNDS: [crate::sound::Sound; 2] = [crate::sound::CMD_168, crate::sound::CMD_61];
    pub const FROST_SOUNDS: [crate::sound::Sound; 2] = [crate::sound::CMD_E8, crate::sound::CMD_163];
    pub const FROST_START_SOUND: crate::sound::Sound = crate::sound::CMD_62;
    pub const QUAKE_START_SOUND: crate::sound::Sound = crate::sound::CMD_63;
    pub const QUAKE_SECOND_SOUND: crate::sound::Sound = crate::sound::CMD_16B;
    pub const MOON_START_SOUNDS: [crate::sound::Sound; 2] = [crate::sound::CMD_177, crate::sound::MOON];
    pub const MOON_SOUND: crate::sound::Sound = crate::sound::CMD_67;
    pub const MOON_SOUND_B: crate::sound::Sound = crate::sound::CMD_18B;
}

/// Field that keeps the ally blessed by spell 0x15 (word: address of its record).
const ALLY: u32 = 0x1ec;

/// Elf table (ROM data): jump speeds (four words per jump type).
const JUMP_SPEEDS: u32 = 0x10_453e;

/// Sub-actions of button 1 (`Player::SUBACTION`), all even.
mod kind {
    /// Chain of seven strikes.
    pub const COMBO: u8 = 0x00;
    pub const THRUST_HIGH: u8 = 0x02;
    pub const THRUST_LOW: u8 = 0x04;
    pub const TURN_STRIKE: u8 = 0x06;
    pub const SLOW_STRIKE: u8 = 0x08;
    pub const AFTER_JUMP: u8 = 0x0a;
    /// Charge: run-up of 12 pixels per frame followed by two effects.
    pub const CHARGE: u8 = 0x0c;
    pub const RUN_STRIKE: u8 = 0x0e;
    pub const LOW_BLOW: u8 = 0x10;
    pub const BACK_HOP: u8 = 0x12;
    pub const GUARD_BREAK: u8 = 0x14;
    /// Shot while backing away.
    pub const RETREAT_SHOT: u8 = 0x16;
    /// Whirlwind (sacrifices hit points).
    pub const WHIRL: u8 = 0x18;
    /// Strike with a small backward leap.
    pub const HOP_STRIKE_A: u8 = 0x1a;
    pub const HOP_STRIKE_B: u8 = 0x1c;
    /// Volley of shots.
    pub const VOLLEY: u8 = 0x1e;
}

// ---- jump (0x42012) --------------------------------------------------------------------------

/// The elf's jump: speed table 0x10453e, push of 0x20 in the air, two aerial gestures (step
/// 0xa: strike, 0xc: item shot) during which the jump is suspended.
const JUMP_KIT: common::JumpKit = common::JumpKit {
    speeds: JUMP_SPEEDS,
    steer_push: 0x20,
    air_attack,
    after_recover: common::curse_check,
    finish: common::flush_pending_effect,
    takeoff: |g, p| {
        p.set_u8(g, 0x59, 0);
        p.set_u8(g, 0x58, 0);
        p.set_u8(g, 0x1e8, 0);
    },
    busy: |g, p| p.u8(g, field::AIR_ATTACK_DONE) == 2,
    apex_gravity: None,
    extra_step,
};

/// 0x42012: jump.
pub fn jump(g: &mut Ctx, p: Player) {
    cov!();
    common::jump(g, p, &JUMP_KIT);
}

/// 0x4233a: in the air, button 1 gives a strike (`AIR_ATTACK_DONE` = 1, step 0xa); key 4 with
/// item 0x23..0x26 in row 2 launches a shot (`AIR_ATTACK_DONE` = 2, step 0xc). Once per jump.
fn air_attack(g: &mut Ctx, p: Player) {
    cov!();
    if p.u8(g, field::AIR_ATTACK_DONE) != 0 {
        return;
    }
    let keys = p.u8(g, Player::KEYS_NEW);
    if keys & 0x10 != 0 {
        p.set_u8(g, Player::STEP, 0xa);
        p.set_u8(g, field::MOVE_STATE, 0);
        p.set_u8(g, field::AIR_ATTACK_DONE, 1);
        let (mut anim, mut style) = (0xb, 1);
        if p.u8(g, 0x67) == 2 {
            (anim, style) = (0xe, 2);
        }
        if p.u8(g, Player::KEYS_HELD) & 4 != 0 {
            (anim, style) = (0xd, 3);
        }
        p.set_u8(g, 0x59, style);
        common::start_with_voice(g, p, anim);
        item_curse(g, p);
        common::aura_hit_countdown(g, p);
    } else if keys & 0x80 != 0 && p.u8(g, 0x15d) == 2 && (0x23..=0x26).contains(&p.u8(g, field::ITEM)) {
        p.set_u8(g, Player::STEP, 0xc);
        p.set_u8(g, field::AIR_ATTACK_DONE, 2);
        animation::start_plain(g, p, 0x22);
        common::aura_hit_countdown(g, p);
    }
}

/// Steps 0xa and 0xc of the jump: the aerial strike (bounce possible off a monster) and the item shot.
fn extra_step(g: &mut Ctx, p: Player, step: u8) {
    cov!();
    match step {
        0xa => {
            if p.u8(g, 0x59) == 3 && p.u8(g, 0x58) & 1 != 0 && p.u8(g, 0x1e8) <= 4 {
                bounce(g, p);
            } else {
                air_fall(g, p);
            }
        }
        0xc => {
            if !air_fall(g, p) && p.u8(g, field::AIR_ATTACK_DONE) == 2 && take_event(g, p) {
                match p.u8(g, field::ITEM) {
                    0x24 => spawn::thrown(g, p, spawn::Thrown::C),
                    0x25 => spawn::thrown(g, p, spawn::Thrown::D),
                    0x26 => spawn::thrown(g, p, spawn::Thrown::E),
                    _ => false,
                };
            }
        }
        _ => {}
    }
}

/// Bounce: the diving strike landed, the elf goes back up (at most five times).
fn bounce(g: &mut Ctx, p: Player) {
    cov!();
    p.set_u8(g, 0x58, 0);
    p.set_u8(g, field::AIR_STEERED, 0);
    let n = p.u8(g, 0x1e8) + 1;
    p.set_u8(g, 0x1e8, n);
    p.set_u16(g, Obj::VZ, 0x600);
    p.set_u16(g, Obj::AZ, (-0x48i16) as u16);
    common::set_forward_motion(g, p, 0x180, 0);
    animation::start_plain(g, p, 0x41);
    let tag = (p.u8(g, Player::INDEX) as u32) << 8;
    crate::events::enqueue_points(g.b, tag as u16);
}

/// One flight step during an aerial gesture; on landing the jump resumes at the recovery (step
/// 8). Returns true if the elf has landed.
fn air_fall(g: &mut Ctx, p: Player) -> bool {
    cov!();
    animation::step(g, p);
    move_xz(g, p);
    common::air_steer(g, p, 0x20);
    if (p.u16(g, Obj::Z) as i16) < 0 {
        p.set_u8(g, Player::STEP, 8);
        common::land(g, p);
        return true;
    }
    false
}

// ---- attack (0x4241c) -----------------------------------------------------------------------

/// 0x4241c: button 1 attack. `kind` (see above) selects the strike; most strikes are
/// followed by a continuation check (`common::follow_up`).
pub fn attack(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::SUBACTION) {
        kind::COMBO => {
            combo(g, p);
            common::follow_up(g, p);
        }
        kind::THRUST_HIGH => {
            thrust(g, p, 1 << 1, 7);
            common::follow_up(g, p);
        }
        kind::THRUST_LOW => {
            thrust(g, p, 1 << 2, 9);
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
            common::single_strike(g, p, 0x13);
            common::follow_up(g, p);
        }
        kind::BACK_HOP => common::back_hop(g, p, 0x1d, false),
        kind::GUARD_BREAK => common::single_strike(g, p, 0x14),
        kind::RETREAT_SHOT => retreat_shot(g, p),
        kind::WHIRL => whirl(g, p),
        kind::HOP_STRIKE_A | kind::HOP_STRIKE_B => {
            hop_strike(g, p);
            common::follow_up(g, p);
        }
        kind::VOLLEY => volley(g, p),
        _ => {}
    }
    common::flush_pending_effect(g, p);
}

/// Basic strike (0x424ee): seven chained gestures as long as the strikes land; the last may end with
/// a small backward leap.
fn combo(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, field::MOVE_STATE, 0);
            p.set_u8(g, field::COMBO_RELEASED, 0);
            p.set_u8(g, field::COMBO_WINDOW, 6);
            let rank = common::chain_rank(g, p, 1, 6);
            common::start_with_voice(g, p, rank as u16);
        }
        2 => {
            if common::combo_follow(g, p, 0x1a) {
                g.sound(ext::sound::STRIKE_LIGHT, p);
            }
        }
        4 => hop_wait(g, p),
        6 => hop_flight(g, p),
        _ => {}
    }
}

/// Thrusts (0x425f8 high, 0x4265a low): a single gesture, of rank 0 or 1 depending on the previous strike.
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

/// Strike given while turning (0x426ac): step 2 is step 0 of a per-state entry (combos).
fn turn_strike(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            p.set_u8(g, Player::STEP, 4);
            p.set_u8(g, field::MOVE_STATE, 0);
            animation::start_plain(g, p, 0x12);
        }
        2 => {
            p.set_u8(g, Player::STEP, 4);
            common::start_with_voice(g, p, 0x1c);
        }
        4 => common::wait_then_stand(g, p),
        _ => {}
    }
}

/// Strike given in slow walk (0x426ee): without continuation.
fn slow_strike(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, field::MOVE_STATE, 0);
            common::start_with_voice(g, p, 0x15);
        }
        2 => common::wait_then_stand(g, p),
        _ => {}
    }
}

/// Landing strike (0x42728).
fn after_jump(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, field::MOVE_STATE, 0);
            animation::start_plain(g, p, 0xf);
            clear_recognizers(g, p);
            g.sound(ext::sound::STRIKE_HEAVY, p);
        }
        2 => common::wait_then_stand(g, p),
        _ => {}
    }
}

/// Charge (0x4275a): run-up, first gesture, second gesture with two effects, slide.
fn charge(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, 0x5b, 0);
            p.set_u8(g, field::MOVE_STATE, 1);
            p.set_u8(g, field::MOVE_KIND, 1);
            common::set_forward_motion(g, p, 0xc00, (-0x98i16) as u16);
            animation::start_plain(g, p, 0x16);
            clear_recognizers(g, p);
        }
        2 => {
            animation::step(g, p);
            if p.u8(g, field::EVENT) != 0 {
                next_step(g, p);
                p.set_u8(g, field::EVENT, 0);
            }
        }
        4 => common::charge_slide(g, p, |g, p| {
            if take_event(g, p) {
                spawn::slash(g, p, Some(0));
                spawn::slash(g, p, None);
                g.sound(ext::sound::CHARGE_B, p);
            }
        }),
        _ => {}
    }
}

/// Strike given while running (0x42810): run-up of 5 pixels per frame, then stop.
fn run_strike(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, field::MOVE_STATE, 0);
            common::set_forward_motion(g, p, 0x500, (-0x28i16) as u16);
            animation::start_plain(g, p, 0x10);
            clear_recognizers(g, p);
            common::weapon_voice(g, p);
            g.sound(ext::sound::CHARGE_A, p);
        }
        2 => {
            common::run_strike_slide(g, p, 0x11);
        }
        4 => common::wait_then_stand(g, p),
        _ => {}
    }
}

/// Shot while backing away (0x42948): gesture, forward run-up with two effects, backward leap, landing.
fn retreat_shot(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, 0x5b, 0);
            p.set_u8(g, field::MOVE_STATE, 1);
            p.set_u8(g, field::MOVE_KIND, 1);
            animation::start_plain(g, p, 0x17);
            clear_recognizers(g, p);
        }
        2 => {
            animation::step(g, p);
            if p.u8(g, field::EVENT) != 0 {
                next_step(g, p);
                p.set_u8(g, field::EVENT, 0);
                common::set_forward_motion(g, p, 0xa80, (-0x98i16) as u16);
                move_x(g, p);
                spawn::slash(g, p, Some(0));
                spawn::slash(g, p, None);
            }
        }
        4 => {
            if animation::step(g, p) {
                next_step(g, p);
                p.set_u16(g, Obj::VX, 0x180);
                p.set_u16(g, Obj::AX, 0);
                p.set_u16(g, Obj::VZ, 0x600);
                p.set_u16(g, Obj::AZ, (-0x38i16) as u16);
                if p.u8(g, Player::FACING) != 0 {
                    common::negate_horizontal_speed(g, p);
                }
                animation::start_plain(g, p, 0x18);
                g.sound(ext::sound::LEAP, p);
            } else {
                move_x(g, p);
            }
        }
        6 => {
            animation::step(g, p);
            move_xz(g, p);
            if (p.u16(g, Obj::Z) as i16) < 0 {
                next_step(g, p);
                p.set_u32(g, Obj::Z, 0);
                p.set_u8(g, Player::SLOW, 0);
                animation::start_plain(g, p, 0x19);
            }
        }
        8 => common::wait_then_stand(g, p),
        _ => {}
    }
}

/// Whirlwind (0x42a2e): costs 12 hit points at the first contact.
fn whirl(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, 0x5b, 0);
            p.set_u8(g, field::MOVE_STATE, 1);
            p.set_u8(g, field::MOVE_KIND, 2);
            p.set_u8(g, 0x58, 0);
            p.set_u8(g, 0x1ef, 0);
            p.set_u8(g, 0x1e8, 0x80);
            animation::start_plain(g, p, 0x1b);
            clear_recognizers(g, p);
        }
        2 => {
            animation::step(g, p);
            if take_event(g, p) {
                crate::families::creators::for_players::whirl(&mut g.leftovers, g.b, p.obj());
                g.sound(rom::WHIRL_SOUND_A, p);
                g.sound(ext::sound::SPIN, p);
            }
            let left = p.u8(g, 0x1e8).wrapping_sub(1);
            p.set_u8(g, 0x1e8, left);
            if left == 0 {
                to_stand(g, p);
                return;
            }
            if p.u8(g, 0x1ef) == 0 && p.u8(g, 0x58) & 1 != 0 {
                p.set_u8(g, 0x1ef, 1);
                let hp = (p.u16(g, Player::HP) as i16).wrapping_sub(0xc);
                p.set_u16(g, Player::HP, if hp > 0 { hp as u16 } else { 1 });
            }
        }
        _ => {}
    }
}

/// Strike with small leap (sub-actions 0x1a and 0x1c, and end of the basic strike): gesture, then, at the carried
/// gesture,
/// backward leap.
fn hop_strike(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, field::MOVE_STATE, 0);
            animation::start_plain(g, p, 0x1a);
            g.sound(ext::sound::STRIKE_LIGHT, p);
        }
        2 => hop_wait(g, p),
        4 => hop_flight(g, p),
        _ => {}
    }
}

/// The gesture plays; at the carried gesture (`EVENT`) the elf leaps back 16 pixels (0x4248c).
fn hop_wait(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    if p.u8(g, field::EVENT) == 0 {
        return;
    }
    next_step(g, p);
    p.set_u16(g, Obj::VZ, 0x300);
    p.set_u16(g, Obj::AZ, (-0x40i16) as u16);
    let (mut vx, mut shift) = (0x200i16, -0x10i16);
    if p.u8(g, Player::FACING) != 0 {
        (vx, shift) = (-vx, -shift);
    }
    p.set_u16(g, Obj::VX, vx as u16);
    p.set_u16(g, Obj::AX, 0);
    let x = p.u16(g, Obj::X).wrapping_add(shift as u16);
    p.set_u16(g, Obj::X, x);
}

/// Flight of the leap (0x424c8): as long as the gesture is not finished, the elf advances; at the end it is back
/// on the ground.
fn hop_flight(g: &mut Ctx, p: Player) {
    cov!();
    if p.u8(g, field::EVENT) == 0 {
        p.set_u32(g, Obj::Z, 0);
    } else {
        move_xz(g, p);
    }
    if animation::step(g, p) {
        p.set_u32(g, Obj::Z, 0);
        to_stand(g, p);
    }
}

/// Volley (0x42ac0): 144 frames of item shots at the carried gesture.
fn volley(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u16(g, field::TIMER, 0x90);
            animation::start_plain(g, p, 0x42);
        }
        2 => {
            animation::step(g, p);
            if countdown(g, p) {
                to_stand(g, p);
                clear_recognizers(g, p);
            } else if take_event(g, p) {
                spawn::thrown(g, p, spawn::Thrown::C);
            }
        }
        _ => {}
    }
}

// ---- special attack (0x42b0c) --------------------------------------------------------------

/// The elf's animations for cast spells (see `items::Style`): the target of lightning and of the
/// wall is kept at `+0x1ec`, the monsters are hurt on the last frame of the lightning.
const STYLE: items::Style = items::Style {
    cast_anim: 0x24,
    bolt_anim: 0x23,
    spark: 2,
    target: 0x1ec,
    thunder_at: 1,
    finish_anim: Some(0x25),
    d0_is_step: true,
    undim_mode: 2,
};

/// The elf's red ring (no. 16): the aura is only placed at the end of the countdown.
const CAST_16: items::Cast = items::Cast {
    fire: |g, p, _| items::ring_fire(g, p),
    on_end: |g, p| items::red_aura(g, p, 0x708),
    ..items::CAST_13
};

/// 0x42b0c: the chosen item or spell (`field::SPECIAL_KIND`).
pub fn special(g: &mut Ctx, p: Player) {
    cov!();
    let which = p.u8(g, field::SPECIAL_KIND);
    ram::SPELL_ROW.set(g.b, which);
    match which {
        0 => items::throw_item(g, p, 0x20, spawn::Thrown::A),
        1 | 0xb => items::throw_item(g, p, 0x20, spawn::Thrown::B),
        2..=5 => timed_throw(g, p),
        6 => items::throw_item(g, p, 0x1f, spawn::Thrown::F),
        7 => items::throw_item(g, p, 0x1f, spawn::Thrown::G),
        8 => items::throw_item(g, p, 0x1e, spawn::Thrown::H),
        0xc => common::swap_weapon(g, p, Player::WEAPON_A, true),
        16 => items::cast(g, p, &CAST_16, STYLE),
        0x15 => ward_ally(g, p),
        0x16 => ward_party(g, p),
        0x17 => beam(g, p),
        0x18 => sparks(g, p),
        0x19 => bolt(g, p),
        0x1a => frost(g, p),
        0x1b => quake(g, p),
        0x1c => moonlight(g, p),
        _ => {
            items::run_shared(g, p, which, STYLE);
        }
    }
}

/// Items 2..5 (0x42bb0): a 48-frame pose, a shot at the carried gesture; number 3 does not consume a
/// charge.
fn timed_throw(g: &mut Ctx, p: Player) {
    cov!();
    if p.u8(g, Player::STEP) == 0 {
        next_step(g, p);
        p.set_u16(g, field::TIMER, 0x30);
        animation::start_plain(g, p, 0x21);
        return;
    }
    animation::step(g, p);
    if countdown(g, p) {
        to_stand(g, p);
        return;
    }
    if !take_event(g, p) {
        return;
    }
    let which = p.u8(g, field::SPECIAL_KIND);
    let launcher = match which {
        2 | 3 => spawn::Thrown::C,
        4 => spawn::Thrown::D,
        _ => spawn::Thrown::E,
    };
    if spawn::thrown(g, p, launcher) && which != 3 {
        common::consume_charge(g, p);
    }
}


/// Plays ROM sounds.
fn sounds(g: &mut Ctx, p: Player, list: &[crate::sound::Sound]) {
    for &sound in list {
        g.sound(sound, p);
    }
}

/// End of the spells that first go through a final animation step (without restoring the cancellation flag).
fn end_when_done(g: &mut Ctx, p: Player) {
    cov!();
    if animation::step(g, p) {
        common::end_spell_keep_cancel(g, p);
    }
}

/// ROM word (code) indexed by the caster's level.
fn by_level(g: &mut Ctx, p: Player, table: u32) -> u16 {
    let level = p.u8(g, Player::LEVEL) as u32;
    g.prog16(table + 2 * level)
}

/// Aura for `target`: duration `duration` (`AURA_TIMER`), power 1, without the other flags.
fn guard_aura(g: &mut Ctx, target: Player, duration: u16) {
    target.set_u16(g, Player::AURA_TIMER, duration);
    target.set_u8(g, Player::AURA_B, 1);
    target.set_u8(g, Player::AURA, 0);
    target.set_u8(g, Player::AURA_C, 0);
}

/// 0x43b04, no. 0x15 (h): red ring on the ally in front of the elf (within 76 pixels) or on themselves.
fn ward_ally(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => common::start_cast(g, p, 0x20, Some(0x152), 0x3b),
        2 => {
            animation::step(g, p);
            if take_event(g, p) {
                spawn::ring(g, p, 0);
                crate::placement::load_effect_palette(g.b, crate::placement::effect_palette::RED);
                pick_ally(g, p);
                sounds(g, p, &rom::WARD_SOUNDS);
            }
            if countdown(g, p) {
                next_step(g, p);
                p.set_u16(g, field::TIMER, 0x20);
                let anim = if p.u8(g, field::AREA) != 0 { 0x3d } else { 0x3c };
                animation::start_plain(g, p, anim);
                sounds(g, p, &rom::WARD_END_SOUNDS);
            }
        }
        4 => {
            animation::step(g, p);
            if take_event(g, p) {
                if is_cancelled(g, p) {
                    p.set_u8(g, Player::STEP, 8);
                    p.set_u16(g, field::TIMER, 0x10);
                    let spark = if p.u8(g, field::AREA) != 0 { 0xb } else { 0xa };
                    common::item_spark(g, p, spark);
                    return wind_down(g, p, 2);
                }
                spawn::ring(g, p, 1);
            }
            if countdown(g, p) {
                next_step(g, p);
                let target = if p.u8(g, field::AREA) != 0 {
                    Player(p.u16(g, ALLY) as i16 as i32 as u32 & 0xff_ffff)
                } else {
                    p
                };
                items::red_aura(g, target, 0x708);
                animation::start_plain(g, p, 0x3e);
            }
        }
        6 => {
            if animation::step(g, p) {
                common::end_spell(g, p);
            }
        }
        8 => wind_down(g, p, 2),
        _ => {}
    }
}

/// 0x43cac: looks for the blessed ally (see `common::find_ally`, range 76): without cancellation they turn
/// toward the elf and go to the blessing pose; `AREA` = 1 if an ally is found.
fn pick_ally(g: &mut Ctx, p: Player) {
    cov!();
    let ally = common::find_ally(g, p, 0x4c, |_, _| ());
    if let Some(q) = ally {
        if !is_cancelled(g, p) {
            p.set_u16(g, ALLY, q.0 as u16);
            q.set_u32(g, Player::MAIN, 0x0200_1c06);
            q.set_u8(g, field::SPELL_FROZEN, 0);
            q.set_u8(g, Player::FACING, 0);
            if p.u16(g, Obj::X) > q.u16(g, Obj::X) {
                q.set_u8(g, Player::FACING, 0x20);
            }
        }
    }
    p.set_u8(g, field::AREA, ally.is_some() as u8);
}

/// 0x43e4e, no. 0x16 (h): red ring, then guard aura on all living players (duration per level).
fn ward_party(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => common::start_cast(g, p, 0x20, Some(0x153), 0x38),
        2 => {
            animation::step(g, p);
            if take_event(g, p) {
                spawn::ring(g, p, 0);
                crate::placement::load_effect_palette(g.b, crate::placement::effect_palette::P360);
                sounds(g, p, &rom::SHIELD_SOUNDS);
            }
            if countdown(g, p) {
                next_step(g, p);
                p.set_u16(g, field::TIMER, 0x20);
                if is_cancelled(g, p) {
                    p.set_u8(g, Player::STEP, 8);
                    p.set_u16(g, field::TIMER, 0x10);
                    common::item_spark(g, p, 0xd);
                }
                animation::start_plain(g, p, 0x39);
            }
        }
        4 => {
            animation::step(g, p);
            if take_event(g, p) {
                spawn::ring(g, p, 1);
                sounds(g, p, &rom::SHIELD_SECOND_SOUNDS);
            }
            if countdown(g, p) {
                next_step(g, p);
                let duration = by_level(g, p, WARD_DURATIONS);
                for q in Player::all() {
                    if q.u8(g, Player::ACTIVE) != 0 && q.u16(g, Player::HP) != 0 && q.u8(g, Player::POWER_10F) == 0 {
                        guard_aura(g, q, duration);
                    }
                }
                animation::start_plain(g, p, 0x3a);
            }
        }
        6 => end_when_done(g, p),
        8 => wind_down(g, p, 2),
        _ => {}
    }
}

/// Words (code): duration of the guard aura per level.
const WARD_DURATIONS: u32 = 0x43f70;

/// 0x43d5e, no. 0x17 (h): ray of light in two stages.
fn beam(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => common::start_cast(g, p, 0x38, Some(0x154), 0x29),
        2 => {
            animation::step(g, p);
            if take_event(g, p) {
                spawn::beam_cast(g, p, 0);
                crate::placement::load_effect_palette(g.b, crate::placement::effect_palette::LIGHT);
                g.sound(rom::BEAM_SOUND, p);
            }
            if countdown(g, p) {
                next_step(g, p);
                p.set_u16(g, field::TIMER, 0x3c);
                animation::start_plain(g, p, 0x2a);
            }
        }
        4 => {
            animation::step(g, p);
            if take_event(g, p) {
                if is_cancelled(g, p) {
                    p.set_u16(g, field::TIMER, 0x10);
                    common::item_spark(g, p, 0xc);
                } else {
                    spawn::beam_cast(g, p, 1);
                    g.sound(rom::BEAM_SECOND_SOUND, p);
                }
            }
            if countdown(g, p) {
                common::end_spell(g, p);
            }
        }
        _ => {}
    }
}

/// Bytes (code): variant number of the gesture of spell 0x18 per level.
const SPARK_GESTURES: u32 = 0x43476;

/// 0x433e4, no. 0x18 (h): burst of sparks, alternating two variants at each gesture.
fn sparks(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => common::start_cast(g, p, 0x3c, Some(0x151), 0x34),
        2 => {
            animation::step(g, p);
            if take_event(g, p) {
                crate::families::creators::for_players::sparks(&mut g.leftovers, g.b, p.obj());
                g.sound(rom::SPARK_SOUND, p);
            }
            if countdown(g, p) {
                next_step(g, p);
                p.set_u8(g, 0x1e8, 0);
                let level = p.u8(g, Player::LEVEL) as u32;
                let gesture = g.prog8(SPARK_GESTURES + level) as u16;
                animation::start_plain(g, p, 0x34 + gesture);
                g.sound(rom::SPARK_END_SOUND, p);
            }
        }
        4 => {
            if p.u8(g, field::EVENT) != 0 {
                let side = p.u8(g, 0x1e8) ^ 1;
                p.set_u8(g, 0x1e8, side);
                if is_cancelled(g, p) {
                    next_step(g, p);
                    p.set_u16(g, field::TIMER, 0x10);
                    common::item_spark(g, p, 7);
                    return wind_down(g, p, 2);
                }
                spawn::shower(g, p, (side == 0) as u8);
                p.set_u8(g, field::EVENT, 0);
            }
            if animation::step(g, p) {
                next_step(g, p);
                p.set_u16(g, field::TIMER, 1);
            }
        }
        6 => wind_down(g, p, 2),
        _ => {}
    }
}

/// 0x4350c, no. 0x19 (h): lightning bolt in two stages.
fn bolt(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => common::start_cast(g, p, 0x28, Some(0x153), 0x26),
        2 => {
            animation::step(g, p);
            if take_event(g, p) {
                crate::families::creators::for_players::bolt(&mut g.leftovers, g.b, p.obj(), 0);
                crate::placement::load_effect_palette(g.b, crate::placement::effect_palette::LIGHT);
                g.sound(rom::BOLT_SOUND, p);
            }
            if countdown(g, p) {
                next_step(g, p);
                p.set_u16(g, field::TIMER, 0x30);
                animation::start_plain(g, p, 0x27);
            }
        }
        4 => {
            animation::step(g, p);
            if take_event(g, p) {
                g.sound(rom::BOLT_SECOND_SOUND, p);
                if is_cancelled(g, p) {
                    next_step(g, p);
                    p.set_u16(g, field::TIMER, 0x10);
                    common::item_spark(g, p, 6);
                    return wind_down(g, p, 2);
                }
                crate::families::creators::for_players::bolt(&mut g.leftovers, g.b, p.obj(), 1);
            }
            if countdown(g, p) {
                next_step(g, p);
                p.set_u16(g, field::TIMER, 0x5a);
                animation::start_plain(g, p, 0x28);
                sounds(g, p, &rom::BOLT_END_SOUNDS);
            }
        }
        6 => wind_down(g, p, 2),
        _ => {}
    }
}

/// Words (code): duration of the freeze of spell 0x1a per level.
const FROST_DURATIONS: u32 = 0x436c8;

/// 0x4362a, no. 0x1a (h): freeze. During the final phase the player sets the intensity with up and down
/// (gesture 0x2f + intensity); the screen shakes.
fn frost(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => common::start_cast(g, p, 0x20, Some(0x153), 0x2d),
        2 => {
            animation::step(g, p);
            if take_event(g, p) {
                spawn::ice_crown(g, p);
                sounds(g, p, &rom::FROST_SOUNDS);
            }
            if countdown(g, p) {
                next_step(g, p);
                p.set_u8(g, 0x1e8, 0x10);
                p.set_u8(g, 0x1e9, 2);
                let duration = by_level(g, p, FROST_DURATIONS);
                p.set_u16(g, field::TIMER, duration);
                animation::start_plain(g, p, 0x2e);
            }
        }
        4 => {
            if !animation::step(g, p) {
                return;
            }
            next_step(g, p);
            p.set_u8(g, 0x1eb, 0);
            p.set_u8(g, 0x1ea, 4);
            p.set_u8(g, 0x1e8, 0x10);
            p.set_u8(g, 0x1e9, 2);
            animation::start_plain(g, p, 0x2f + 2);
            g.sound(rom::FROST_START_SOUND, p);
            if is_cancelled(g, p) {
                p.set_u8(g, Player::STEP, 8);
                p.set_u16(g, field::TIMER, 0x10);
                common::item_spark(g, p, 8);
                return wind_down(g, p, 2);
            }
            spawn::ice_spell(g, p);
        }
        6 => frost_steer(g, p),
        8 => wind_down(g, p, 2),
        _ => {}
    }
}

/// Step 6 of the freeze (0x43788): the screen flashes three times; the intensity follows the up and down keys.
fn frost_steer(g: &mut Ctx, p: Player) {
    cov!();
    common::blink_screen(g, p);
    if countdown(g, p) {
        next_step(g, p);
        p.set_u16(g, field::TIMER, 1);
        p.set_u8(g, 0x1eb, 0);
    }
    common::flash_screen(g, p);
    common::intensity_keys(g, p, 0x2f);
}

/// Words (code): duration of spell 0x1b per level.
const QUAKE_DURATIONS: u32 = 0x4395a;

/// Earthquake (0x43892, no. 0x1b): hurts the monsters at the right moment.
const QUAKE_STRIKE: Strike = Strike {
    damage: Damage::Table(0x10_442c, 0x10_440e),
    live: Live::Any,
    needs_hp: true,
    grounded: false,
    immune_bit: 4,
    reaction: 0x90,
    random_facing: true,
    boosts: &[Boost::Boss(0xc), Boost::Armor, Boost::Charm(5)],
    undead_die: true,
    sound: None,
};

/// 0x43892, no. 0x1b (h): earthquake.
fn quake(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            common::begin_cast(g, p, Some(0x3c), Some(0x154));
            common::dim_screen(g, p);
            { crate::palette::flash(g.b, crate::palette::Flash::A); }
            animation::start_plain(g, p, 0x2b);
        }
        2 => {
            animation::step(g, p);
            if take_event(g, p) {
                crate::families::creators::for_players::quake(&mut g.leftovers, g.b, p.obj(), true);
                crate::placement::load_effect_palette(g.b, crate::placement::effect_palette::NORMAL);
                g.sound(ext::sound::PRAYER_QUAKE, p);
            }
            if countdown(g, p) {
                next_step(g, p);
                let duration = by_level(g, p, QUAKE_DURATIONS);
                p.set_u16(g, field::TIMER, duration);
                animation::start_plain(g, p, 0x2c);
                g.sound(rom::QUAKE_START_SOUND, p);
                if is_cancelled(g, p) {
                    common::item_spark(g, p, 9);
                    p.set_u16(g, field::TIMER, 0x10);
                    return quake_rumble(g, p);
                }
                crate::families::creators::for_players::quake(&mut g.leftovers, g.b, p.obj(), false);
                crate::families::small::quake_b(g.b, p.obj());
                g.sound(rom::QUAKE_SECOND_SOUND, p);
            }
        }
        4 => quake_rumble(g, p),
        _ => {}
    }
}

/// Step 4 of the earthquake (0x4398c): at countdown turn 0x64e, the monsters are hurt.
fn quake_rumble(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    if countdown(g, p) {
        crate::palette::restore_flash(g.b);
        common::end_spell(g, p);
        return;
    }
    if !is_cancelled(g, p) && p.u16(g, field::TIMER) == 0x64 {
        foes::strike_foes(g, p, &QUAKE_STRIKE);
    }
}

/// Moon rain (0x44314, no. 0x1c, facing left): the monsters on the ground are hurt.
const MOON_STRIKE: Strike = Strike {
    damage: Damage::Table(0x10_4468, 0x10_444a),
    live: Live::Any,
    needs_hp: true,
    grounded: true,
    immune_bit: 8,
    reaction: 0xc,
    random_facing: true,
    boosts: &[Boost::Armor],
    undead_die: true,
    sound: None,
};

/// 0x43ff4, no. 0x1c (h): moon spell. Two variants depending on facing: to the right (steps 0 to 6,
/// effect 0x4e4de) or to the left (steps 8 to 0xc, effect 0x4e782).
fn moonlight(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 | 8 => {
            if p.u8(g, Player::STEP) == 0 && p.u8(g, Player::FACING) != 0 {
                p.set_u8(g, Player::STEP, 8);
            }
            common::begin_cast(g, p, Some(0xb0), Some(0x155));
            common::dim_screen(g, p);
            animation::start_plain(g, p, 0x3f);
            sounds(g, p, &rom::MOON_START_SOUNDS);
        }
        2 => {
            animation::step(g, p);
            if countdown(g, p) {
                next_step(g, p);
                p.set_u16(g, field::TIMER, 0x80);
                animation::start_plain(g, p, 0x40);
            } else if p.u16(g, field::TIMER) == 0x20 {
                if !is_cancelled(g, p) {
                    for color in [0, 2, 3] {
                        spawn::moon(g, p, color as u8);
                    }
                    crate::placement::load_effect_palette(g.b, crate::placement::effect_palette::NORMAL);
                    g.sound(rom::MOON_SOUND_B, p);
                }
                { crate::palette::flash(g.b, crate::palette::Flash::D); };
            }
        }
        4 => {
            animation::step(g, p);
            if take_event(g, p) {
                if is_cancelled(g, p) {
                    p.set_u8(g, Player::STEP, 6);
                    p.set_u16(g, field::TIMER, 0x10);
                    common::item_spark(g, p, 0xe);
                    return moon_finish(g, p);
                }
                spawn::moon(g, p, 1);
                g.sound(rom::MOON_SOUND, p);
                items::meteor_damage(g, p);
            }
            if countdown(g, p) {
                next_step(g, p);
                p.set_u16(g, field::TIMER, 0x18);
            }
        }
        6 => moon_finish(g, p),
        0xa => {
            animation::step(g, p);
            if countdown(g, p) {
                next_step(g, p);
                p.set_u16(g, field::TIMER, 0x100);
                if is_cancelled(g, p) {
                    p.set_u16(g, field::TIMER, 0xa);
                    common::item_spark(g, p, 0xe);
                }
                animation::start_plain(g, p, 0x40);
                g.sound(rom::MOON_SOUND, p);
            } else if p.u16(g, field::TIMER) == 0x20 {
                if !is_cancelled(g, p) {
                    spawn::moon_b(g, p);
                    crate::placement::load_effect_palette(g.b, crate::placement::effect_palette::NORMAL);
                }
                { crate::palette::flash(g.b, crate::palette::Flash::C); };
            }
        }
        0xc => {
            animation::step(g, p);
            if countdown(g, p) {
                crate::palette::restore_flash(g.b);
                common::end_spell(g, p);
            } else if !is_cancelled(g, p) && p.u16(g, field::TIMER) == 0x60 {
                foes::strike_foes(g, p, &MOON_STRIKE);
            }
        }
        _ => {}
    }
}

/// Step 6 of the moon spell (0x4410e): at the end of the countdown the screen flash goes out.
fn moon_finish(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    if countdown(g, p) {
        crate::palette::restore_flash(g.b);
        common::end_spell(g, p);
    }
}

action_native!(JUMP, 0x42012, "elf_jump", jump);
action_native!(ATTACK, 0x4241c, "elf_attack", attack);
action_native!(SPECIAL, 0x42b0c, "elf_special", special);
action_native!(DASH, 0x444ca, "elf_dash", common::dash);
