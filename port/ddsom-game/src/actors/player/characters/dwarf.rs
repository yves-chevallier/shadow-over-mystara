//! The dwarf (characters 6 and 7: both variants have the same code).
//!
//! Four actions, reached through `characters::run`:
//! - `jump` (0x445ba): profile jump (crouch, rise, descent, bounce), with aerial strike and
//!   diving attack;
//! - `attack` (0x4491a): button 1; `Player::SUBACTION` selects the strike (see `kind`);
//! - `special` (0x45148): the chosen item or spell (`field::SPECIAL_KIND`, 0..20), the same as
//!   the fighter's (`items`);
//! - `dash` (0x45988): action 0x10, the common slide (`common::dash`).
//!
//! The shots, effects and sounds are created by Rust calls.

use super::common::{self, field, next_step, set_forward_motion};
use super::items;
use super::super::spawn;
use super::super::actions::{start_stand, to_stand};
use super::super::checks::item_curse;
use super::super::combos::clear_recognizers;
use super::super::walk::{move_x, move_xz, move_z};
use super::super::{animation, Ctx, Player};
use crate::world::objects::Obj;
use crate::actors::player::ram;
use crate::actors::player::external as ext;

/// Identifiers (original addresses) of the routines reached: player effects and sounds of the dwarf.
mod rom {
    /// Sounds.
    pub const SOUND_DASH: crate::hw::sound::Sound = crate::hw::sound::CMD_118_218;
    pub const SOUND_BOUNCE: crate::hw::sound::Sound = crate::hw::sound::CMD_142_242;
}

/// Dwarf jump speed table (ROM data): a row of four words (vx, ax, vz, az)
/// per jump profile (`JUMP_ROW`).
const JUMP_SPEEDS: u32 = 0x10_455e;

/// Record bytes specific to the dwarf.
mod own {
    /// Profile of the current jump: row of the speed table. 0 in place, 2 after a run, 3
    /// vertical, 4 then 5 forward jump with a direction held, 6 then 7 without direction.
    pub const JUMP_ROW: u32 = 0x1e8;
    /// Action counter (frames).
    pub const COUNTER: u32 = 0x1e8;
    /// The whirlwind has already cost hit points (`1ec`); contact is signaled by `+0x58` bit 0.
    pub const WHIRL_PAID: u32 = 0x1ec;
    pub const CONTACT: u32 = 0x58;
}

/// Sub-actions of button 1 (`Player::SUBACTION`), all even.
mod kind {
    pub const COMBO: u8 = 0x00;
    pub const THRUST_HIGH: u8 = 0x02;
    pub const THRUST_LOW: u8 = 0x04;
    pub const TURN_STRIKE: u8 = 0x06;
    pub const SLOW_STRIKE: u8 = 0x08;
    pub const AFTER_JUMP: u8 = 0x0a;
    /// Charge: run-up of 12 pixels per frame followed by a gesture.
    pub const CHARGE: u8 = 0x0c;
    pub const RUN_STRIKE: u8 = 0x0e;
    pub const LOW_BLOW: u8 = 0x10;
    pub const BACK_HOP: u8 = 0x12;
    pub const GUARD_BREAK: u8 = 0x14;
    /// Somersault: leap, turn in the air, landing.
    pub const FLIP: u8 = 0x16;
    /// Diving whirlwind (costs hit points).
    pub const SPIN_DROP: u8 = 0x18;
    /// Long slide.
    pub const LONG_DASH: u8 = 0x1a;
    pub const POSE: u8 = 0x1c;
}

// ---- jump (0x445ba) --------------------------------------------------------------------------

/// 0x445ba: jump. Step 0 crouch, 2 rise (then the whole flight of a jump in place), 4 descent
/// of a forward jump, 6 recovery, 8 dive, 0xa bounce, 0xc end of the bounce.
pub fn jump(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => takeoff(g, p),
        2 => rise(g, p),
        4 => descend(g, p),
        6 => recover(g, p),
        8 => dive(g, p),
        0xa => bounce(g, p),
        0xc => bounce_end(g, p),
        _ => {}
    }
    common::flush_pending_effect(g, p);
}

/// Step 0: at the end of the crouch the jump starts. A forward jump takes profile 4 if a
/// direction is held, 6 otherwise.
fn takeoff(g: &mut Ctx, p: Player) {
    cov!();
    if !animation::step(g, p) {
        return;
    }
    next_step(g, p);
    p.set_u8(g, field::AIR_STEERED, 0);
    p.set_u8(g, field::AIR_ATTACK_DONE, 0);
    let kind = p.u8(g, Player::JUMP_KIND);
    let x = p.u16(g, Obj::X);
    p.set_u16(g, field::TAKEOFF_X, x);
    let anim = common::by_jump_kind(g, p, 0x15, 0x10, 0xb);
    animation::start(g, p, anim);
    let mut row = kind;
    if kind == 1 {
        row = if matches!(p.u8(g, Player::KEYS_HELD) & 0xf, 1 | 2) { 4 } else { 6 };
    }
    p.set_u8(g, own::JUMP_ROW, row);
    common::launch_jump(g, p, JUMP_SPEEDS, row);
    g.sound(crate::hw::sound::JUMP_VOICE, p);
}

/// Step 2: rise. A jump of profiles 0..3 ends here (landing on the ground); a forward jump moves on
/// to the next profile and to step 4 when the vertical speed becomes negative.
fn rise(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    move_z(g, p);
    common::drift(g, p);
    common::air_steer(g, p, 0x30);
    air_attack(g, p);
    if p.u8(g, field::AIR_ATTACK_DONE) == 2 {
        return;
    }
    let row = p.u8(g, own::JUMP_ROW);
    if row <= 3 {
        if (p.u16(g, Obj::Z) as i16) >= 0 {
            return;
        }
        p.set_u8(g, Player::STEP, 6);
        p.set_u32(g, Obj::Z, 0);
        let anim = common::by_jump_kind(g, p, 0x16, 0x11, 0xc);
        animation::start(g, p, anim);
        g.sound(ext::sound::LANDING, p);
    } else {
        if (p.u16(g, Obj::VZ) as i16) >= 0 {
            return;
        }
        next_step(g, p);
        p.set_u8(g, own::JUMP_ROW, row + 1);
        common::launch_jump(g, p, JUMP_SPEEDS, row + 1);
    }
}

/// Step 4: descent of a forward jump.
fn descend(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    move_z(g, p);
    move_x(g, p);
    common::air_steer(g, p, 0x30);
    air_attack(g, p);
    if p.u8(g, field::AIR_ATTACK_DONE) == 2 || (p.u16(g, Obj::Z) as i16) >= 0 {
        return;
    }
    next_step(g, p);
    p.set_u32(g, Obj::Z, 0);
    animation::start(g, p, 0x13);
    g.sound(ext::sound::LANDING, p);
}

/// Step 6: recovery, then return to the standing state (and fall of the cursed weapon).
fn recover(g: &mut Ctx, p: Player) {
    cov!();
    if animation::step(g, p) {
        to_stand(g, p);
        common::curse_check(g, p);
    }
}

/// Button 1 in the air gives a strike (once per jump); jumping with the down key during a
/// jump without momentum launches the dive.
fn air_attack(g: &mut Ctx, p: Player) {
    cov!();
    if p.u8(g, field::AIR_ATTACK_DONE) != 0 {
        return;
    }
    if p.u8(g, Player::KEYS_NEW) & 0x10 != 0 {
        p.set_u8(g, field::MOVE_STATE, 0);
        p.set_u8(g, field::AIR_ATTACK_DONE, 1);
        let mut anim = if p.u8(g, Player::JUMP_KIND) == 2 { 0xc } else { 9 };
        if common::holds_special_weapon(g, p) {
            anim = 0xc;
        }
        if p.u8(g, Player::KEYS_HELD) & 4 != 0 {
            anim = 0xb;
        }
        common::start_with_voice(g, p, anim);
        item_curse(g, p);
        common::aura_hit_countdown(g, p);
        return;
    }
    let row = p.u8(g, own::JUMP_ROW);
    let keys = p.u8(g, Player::KEYS_HELD);
    if !matches!(row, 0 | 3 | 6 | 7) || p.u8(g, Player::KEYS_NEW) & 0x20 == 0 || keys & 4 == 0 {
        return;
    }
    p.set_u8(g, Player::STEP, 8);
    p.set_u8(g, field::MOVE_STATE, 0);
    p.set_u8(g, field::AIR_ATTACK_DONE, 2);
    p.set_u16(g, Obj::VZ, (-0x300i16) as u16);
    p.set_u16(g, Obj::AZ, (-0x60i16) as u16);
    p.set_u32(g, Obj::VX, 0);
    if keys & 3 != 0 {
        set_forward_motion(g, p, 0x600, 0);
    }
    animation::start_plain(g, p, 0x21);
}

/// Step 8: dive; on impact the dwarf bounces.
fn dive(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    move_xz(g, p);
    if (p.u16(g, Obj::Z) as i16) >= 0 {
        return;
    }
    next_step(g, p);
    p.set_u32(g, Obj::Z, 0);
    p.set_u8(g, own::JUMP_ROW, 0);
    p.set_u16(g, Obj::VZ, 0x600);
    p.set_u16(g, Obj::AZ, (-0x50i16) as u16);
    if p.u16(g, Obj::VX) != 0 {
        let vx = if p.u8(g, Player::FACING) == 0 { -0x200i16 } else { 0x200 };
        p.set_u16(g, Obj::VX, vx as u16);
    }
    crate::services::camera_modes::start_shake(g.b, crate::services::camera_modes::shake::A, 0x10);
    g.sound(rom::SOUND_BOUNCE, p);
}

/// Step 0xa: bounce after the dive, then landing.
fn bounce(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    move_xz(g, p);
    if (p.u16(g, Obj::VZ) as i16) < 0 && p.u8(g, own::JUMP_ROW) == 0 {
        p.set_u8(g, own::JUMP_ROW, 1);
        animation::start(g, p, 0x12);
    }
    if (p.u16(g, Obj::Z) as i16) >= 0 {
        return;
    }
    next_step(g, p);
    p.set_u32(g, Obj::Z, 0);
    animation::start(g, p, 0x11);
    g.sound(ext::sound::LANDING, p);
}

/// Step 0xc: end of the bounce.
fn bounce_end(g: &mut Ctx, p: Player) {
    cov!();
    if animation::step(g, p) {
        to_stand(g, p);
    }
}

// ---- attack (0x4491a) -----------------------------------------------------------------------

/// 0x4491a: button 1 attack. `kind` selects the strike; ground strikes are followed by a
/// continuation check (`common::follow_up`).
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
        kind::BACK_HOP => common::back_hop(g, p, 0x19, false),
        kind::GUARD_BREAK => guard_break(g, p),
        kind::FLIP => flip(g, p),
        kind::SPIN_DROP => spin_drop(g, p),
        kind::LONG_DASH => long_dash(g, p),
        kind::POSE => {
            pose(g, p);
            common::follow_up(g, p);
        }
        _ => {}
    }
    common::flush_pending_effect(g, p);
}

/// With the particular weapon (8, 0x18) all strikes become the basic strike; `clear_hits`: the
/// landed hits are forgotten.
fn fall_back_to_combo(g: &mut Ctx, p: Player, clear_hits: bool) -> bool {
    cov!();
    if !common::holds_special_weapon(g, p) {
        return false;
    }
    p.set_u8(g, Player::SUBACTION, kind::COMBO);
    if clear_hits {
        p.set_u8(g, field::HITS, 0);
    }
    true
}

/// Start of a basic strike (0x449e8): the chain rank advances if the previous strike landed
/// (five strikes).
fn combo_begin(g: &mut Ctx, p: Player) {
    cov!();
    next_step(g, p);
    p.set_u8(g, field::MOVE_STATE, 0);
    p.set_u8(g, field::COMBO_RELEASED, 0);
    p.set_u8(g, field::COMBO_WINDOW, 6);
    let rank = common::chain_rank(g, p, 1, 4);
    let anim = if common::holds_special_weapon(g, p) { 0x25 } else { rank as u16 };
    common::start_with_voice(g, p, anim);
}

/// Basic strike (0x449ce): step 2 extends the strike, steps 4 and 6 are a forward lunge.
fn combo(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => combo_begin(g, p),
        2 => {
            common::combo_follow(g, p, 0x15);
        }
        4 => lunge_start(g, p),
        6 => lunge_slide(g, p),
        _ => {}
    }
}

/// The lunge starts at the carried gesture (the event is not cleared).
fn lunge_start(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    if p.u8(g, field::EVENT) != 0 {
        next_step(g, p);
        set_forward_motion(g, p, 0x400, (-0x38i16) as u16);
    }
}

/// The lunge slides to a stop.
fn lunge_slide(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    move_x(g, p);
    if common::momentum_reversed(g, p) {
        to_stand(g, p);
    }
}

/// Thrusts (0x44ace high, 0x44b48 low): a single gesture, of rank 0 or 1 depending on the previous strike.
fn thrust(g: &mut Ctx, p: Player, hit_bit: u8, base_anim: u16) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            p.set_u8(g, field::MOVE_STATE, 0);
            if fall_back_to_combo(g, p, false) {
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

/// Strike given while turning (0x44bb2): step 2 is step 0 of a per-state entry (combos).
fn turn_strike(g: &mut Ctx, p: Player) {
    cov!();
    let step = p.u8(g, Player::STEP);
    match step {
        0 | 2 => {
            if step == 0 {
                p.set_u8(g, field::MOVE_STATE, 0);
            }
            if fall_back_to_combo(g, p, true) {
                return combo_begin(g, p);
            }
            p.set_u8(g, Player::STEP, 4);
            common::start_with_voice(g, p, if step == 0 { 0x10 } else { 0x18 });
        }
        4 => common::wait_then_stand(g, p),
        _ => {}
    }
}

/// Strike given in slow walk (0x44c36): without continuation.
fn slow_strike(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            p.set_u8(g, field::MOVE_STATE, 0);
            if fall_back_to_combo(g, p, true) {
                p.set_u8(g, Player::SLOW, 0);
                return combo_begin(g, p);
            }
            next_step(g, p);
            common::start_with_voice(g, p, 0x13);
        }
        2 => common::wait_then_stand(g, p),
        _ => {}
    }
}

/// Landing strike (0x44c90).
fn after_jump(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, field::MOVE_STATE, 0);
            let anim = if common::holds_special_weapon(g, p) { 0x26 } else { 0xd };
            animation::start_plain(g, p, anim);
            g.sound(ext::sound::STRIKE_HEAVY, p);
            common::weapon_voice(g, p);
            clear_recognizers(g, p);
        }
        2 => common::wait_then_stand(g, p),
        _ => {}
    }
}

/// Charge (0x44cde): run-up, gesture (landed hit, effect), slide.
fn charge(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, Player::STRIKE_HITS, 0);
            p.set_u8(g, field::MOVE_STATE, 1);
            p.set_u8(g, field::MOVE_KIND, 1);
            set_forward_motion(g, p, 0xc00, (-0x98i16) as u16);
            animation::start_plain(g, p, 0x14);
            g.sound(ext::sound::CHARGE_B, p);
            clear_recognizers(g, p);
        }
        2 => {
            animation::step(g, p);
            if p.u8(g, field::EVENT) != 0 {
                next_step(g, p);
                p.set_u8(g, field::EVENT, 0);
                spawn::slash(g, p, None);
                g.sound(ext::sound::CHARGE_A, p);
            }
        }
        4 => common::charge_slide(g, p, |g, p| {
            if p.u8(g, field::EVENT) != 0 {
                p.set_u8(g, field::EVENT, 0);
                spawn::slash(g, p, Some(2));
                common::weapon_voice(g, p);
            }
        }),
        _ => {}
    }
}

/// Strike given while running (0x44da4): run-up of 5 pixels per frame, then stop.
fn run_strike(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, field::MOVE_STATE, 0);
            set_forward_motion(g, p, 0x500, (-0x28i16) as u16);
            animation::start_plain(g, p, 0xe);
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

/// Guard strike (0x44eb2): without voice.
fn guard_break(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            p.set_u8(g, field::MOVE_STATE, 0);
            if fall_back_to_combo(g, p, true) {
                return combo_begin(g, p);
            }
            next_step(g, p);
            animation::start_plain(g, p, 0x12);
        }
        2 => common::wait_then_stand(g, p),
        _ => {}
    }
}

/// Somersault (0x44eee): run-up, turn in the air (the dwarf changes sides), landing. As long as
/// the animation is not finished, some steps make it advance twice per frame (like the original).
fn flip(g: &mut Ctx, p: Player) {
    cov!();
    /// One animation step; if it is not the last, a second follows.
    fn double_step(g: &mut Ctx, p: Player) -> bool {
        let done = animation::step(g, p);
        if !done {
            animation::step(g, p);
        }
        done
    }
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, Player::STRIKE_HITS, 0);
            p.set_u8(g, field::MOVE_STATE, 1);
            p.set_u8(g, field::MOVE_KIND, 1);
            set_forward_motion(g, p, 0x400, 0);
            animation::start_plain(g, p, 0x16);
            clear_recognizers(g, p);
        }
        2 => {
            move_x(g, p);
            if double_step(g, p) {
                next_step(g, p);
                p.set_u16(g, Obj::VZ, 0x600);
                p.set_u16(g, Obj::AZ, (-0x40i16) as u16);
                p.flip(g);
                animation::start_plain(g, p, 0x17);
                g.sound(ext::sound::LEAP_END, p);
                g.sound(ext::sound::LEAP, p);
            }
        }
        4 => {
            move_z(g, p);
            if (p.u16(g, Obj::VZ) as i16) < 0 {
                next_step(g, p);
                animation::start(g, p, 0x12);
            } else {
                animation::step(g, p);
            }
        }
        6 => {
            move_z(g, p);
            if (p.u16(g, Obj::Z) as i16) < 0 {
                next_step(g, p);
                p.set_u32(g, Obj::Z, 0);
                animation::start(g, p, 0x13);
            } else {
                animation::step(g, p);
            }
        }
        8 => {
            if double_step(g, p) {
                p.set_u8(g, Player::SLOW, 0);
                to_stand(g, p);
            }
        }
        _ => {}
    }
}

/// Diving whirlwind (0x44fd0): leap with a turn, descent while spinning; costs 12 hit points
/// at the first contact.
fn spin_drop(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, Player::STRIKE_HITS, 0);
            p.set_u8(g, field::MOVE_STATE, 1);
            p.set_u8(g, field::MOVE_KIND, 2);
            p.set_u8(g, own::CONTACT, 0);
            p.set_u8(g, own::WHIRL_PAID, 0);
            p.set_u16(g, Obj::VZ, 0x600);
            p.set_u16(g, Obj::AZ, (-0x60i16) as u16);
            p.flip(g);
            animation::start_plain(g, p, 0x22);
            g.sound(ext::sound::SPIN, p);
        }
        2 => {
            move_z(g, p);
            if (p.u16(g, Obj::VZ) as i16) < 0 {
                next_step(g, p);
                animation::start_plain(g, p, 0x23);
            } else {
                animation::step(g, p);
                whirl_cost(g, p);
            }
        }
        4 => {
            move_z(g, p);
            if (p.u16(g, Obj::Z) as i16) > 0 {
                animation::step(g, p);
            } else {
                next_step(g, p);
                animation::start_plain(g, p, 0x24);
                crate::services::camera_modes::start_shake(g.b, crate::services::camera_modes::shake::A, 8);
            }
        }
        6 => {
            if animation::step(g, p) {
                p.set_u8(g, Player::SLOW, 0);
                to_stand(g, p);
                return;
            }
            animation::step(g, p);
            whirl_cost(g, p);
        }
        _ => {}
    }
}

/// The first contact of the whirlwind costs 12 hit points (at least one remains).
fn whirl_cost(g: &mut Ctx, p: Player) {
    cov!();
    if p.u8(g, own::WHIRL_PAID) == 0 && p.u8(g, own::CONTACT) & 1 != 0 {
        p.set_u8(g, own::WHIRL_PAID, 1);
        let hp = (p.u16(g, Player::HP) as i16).wrapping_sub(0xc);
        p.set_u16(g, Player::HP, if hp > 0 { hp as u16 } else { 1 });
    }
}

/// Long slide (0x450a8): run-up of 0x40 per frame for 50 frames (the first 20 really advance),
/// pause, then return to the standing state.
fn long_dash(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, field::MOVE_STATE, 1);
            p.set_u8(g, field::MOVE_KIND, 2);
            p.set_u8(g, own::COUNTER, 0x32);
            p.set_u32(g, Obj::VX, 0x40_0000);
            if p.u8(g, Player::FACING) == 0 {
                common::negate_horizontal_speed(g, p);
            }
            animation::start_plain(g, p, 0x20);
            spawn::dash(g, p);
            g.sound(ext::sound::LEAP, p);
            g.sound(rom::SOUND_DASH, p);
            clear_recognizers(g, p);
        }
        2 => {
            let left = p.u8(g, own::COUNTER).wrapping_sub(1);
            p.set_u8(g, own::COUNTER, left);
            if left == 0 {
                next_step(g, p);
                p.set_u8(g, own::COUNTER, 0x10);
                start_stand(g, p);
                return;
            }
            if left > 0x1e {
                move_x(g, p);
            }
            animation::step(g, p);
        }
        4 => {
            let left = p.u8(g, own::COUNTER).wrapping_sub(1);
            p.set_u8(g, own::COUNTER, left);
            if left == 0 {
                p.set_state(g, 0x0200_0002);
                clear_recognizers(g, p);
            }
            animation::step(g, p);
        }
        _ => {}
    }
}

/// Forward gesture (0x4495a), followed by a lunge like the basic strike.
fn pose(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, field::MOVE_STATE, 0);
            animation::start_plain(g, p, 0x15);
        }
        2 => lunge_start(g, p),
        4 => lunge_slide(g, p),
        _ => {}
    }
}

// ---- special attack (0x45148) --------------------------------------------------------------

/// The dwarf's animations for cast spells (see `items::Style`).
const STYLE: items::Style = items::Style { cast_anim: 0x1f, bolt_anim: 0x1e, spark: 3, ..items::Style::CLASSIC };

/// 0x45148: the chosen item or spell (`field::SPECIAL_KIND`, 0..20), like the fighter; numbers
/// 3 and 11 do nothing. 0..8: thrown items; 12: weapon swap; 9, 10 and 13..20: `items`.
pub fn special(g: &mut Ctx, p: Player) {
    cov!();
    let which = p.u8(g, field::SPECIAL_KIND);
    ram::SPELL_ROW.set(g.b, which);
    match which {
        0 => items::throw_item(g, p, 0x1c, spawn::Thrown::A),
        1 => items::throw_item(g, p, 0x1c, spawn::Thrown::B),
        2 => items::throw_item(g, p, 0x1d, spawn::Thrown::C),
        4 => items::throw_item(g, p, 0x1d, spawn::Thrown::D),
        5 => items::throw_item(g, p, 0x1d, spawn::Thrown::E),
        6 => items::throw_item(g, p, 0x1b, spawn::Thrown::F),
        7 => items::throw_item(g, p, 0x1b, spawn::Thrown::G),
        8 => items::throw_item(g, p, 0x1a, spawn::Thrown::H),
        12 => common::swap_weapon(g, p, Player::WEAPON_A, false),
        _ => {
            items::run_shared(g, p, which, STYLE);
        }
    }
}

// ---- action 0x12 (0x35b18) -------------------------------------------------------------------

/// Dwarf roll: run-up of 3 pixels per frame until the end of the gesture, then return to the standing state.
pub fn roll(g: &mut Ctx, p: Player) {
    cov!();
    if p.u8(g, Player::STEP) == 0 {
        next_step(g, p);
        common::roll_start(g, p, 0x300);
        return;
    }
    if animation::step(g, p) {
        to_stand(g, p);
    } else {
        move_x(g, p);
    }
}

action_native!(JUMP, 0x445ba, "dwarf_jump", jump);
action_native!(ATTACK, 0x4491a, "dwarf_attack", attack);
action_native!(SPECIAL, 0x45148, "dwarf_special", special);
action_native!(DASH, 0x45988, "dwarf_dash", common::dash);
action_native!(ROLL, 0x35b18, "dwarf_roll", roll);
