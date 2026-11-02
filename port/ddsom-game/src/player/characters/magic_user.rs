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

use crate::machine::Machine;
use super::common::{self, countdown, field, is_cancelled, next_step, take_event};
use super::foes::{self, Boost, Damage, Live, Strike};
use super::items;
use super::super::spawn;
use super::super::actions::to_stand;
use super::super::combos::clear_recognizers;
use super::super::walk::{move_x, move_xz, move_z};
use super::super::{animation, Ctx, Player};
use crate::objects::Obj;
use crate::player::ram;
use crate::player::external as ext;

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
    let mut anim = if p.u8(g, 0x67) == 2 { 4 } else { 1 };
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
            if p.u8(g, 0x140 + i) == 0x50 {
                p.set_u8(g, 0x140 + i, 0x56);
                p.set_u8(g, 0x148 + i, 0);
                crate::families::effects_creators::for_players::thrown_shard(&mut g.leftovers, g.b, p.obj(), tag as u8);
                crate::families::creators::for_players::set_a(&mut g.leftovers, g.b, p.obj(), sound as u8);
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
                p.set_u8(g, 0x1e8, 8);
                animation::start(g, p, 0x1d);
                g.sound(ext::sound::SLIDE, p);
            }
        }
        4 => {
            let left = p.u8(g, 0x1e8).wrapping_sub(1);
            p.set_u8(g, 0x1e8, left);
            if left == 0 || p.u8(g, Player::KEYS_HELD) & 0x20 == 0 {
                next_step(g, p);
                p.set_u8(g, 0x1e8, 0xc);
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
                let left = p.u8(g, 0x1e8).wrapping_sub(1);
                p.set_u8(g, 0x1e8, left);
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

// ---- special attack (0x4619e) --------------------------------------------------------------

/// The magic-user's animations for cast spells (see `items::Style`): the target of lightning and the wall
/// is kept at `+0x1e8`, the screen is restored with mode 1.
const STYLE: items::Style = items::Style {
    cast_anim: 0x12,
    bolt_anim: 0x10,
    spark: 4,
    target: 0x1e8,
    thunder_at: 0x60,
    finish_anim: Some(0x13),
    d0_is_step: true,
    undim_mode: 1,
};

/// Screen restoration mode of the magic-user.
const UNDIM: u16 = 1;

/// The magic-user's red ring (no. 16): the countdown lasts 30 frames and the aura is only placed at the end.
const CAST_16: items::Cast = items::Cast {
    frames: 0x1e,
    fire: |g, p, _| items::ring_fire(g, p),
    on_end: |g, p| items::red_aura(g, p, 0x8000),
    ..items::CAST_13
};

/// The magic-user's healing potion (no. 19) does not initialize the blinking.
const CAST_19: items::Cast = items::Cast { prepare: common::nothing, ..items::CAST_19 };

/// 0x4619e: the chosen item or spell (`field::SPECIAL_KIND`). Numbers 2..5, 8, 11 and 21..23
/// do not exist for the magic-user: return to the standing state.
pub fn special(g: &mut Ctx, p: Player) {
    cov!();
    let which = p.u8(g, field::SPECIAL_KIND);
    ram::SPELL_ROW.set(g.b, which);
    match which {
        0 => items::throw_item(g, p, 0xf, spawn::Thrown::A),
        1 => items::throw_item(g, p, 0xf, spawn::Thrown::B),
        6 => items::throw_item(g, p, 0xe, spawn::Thrown::F),
        7 => items::throw_item(g, p, 0xe, spawn::Thrown::G),
        0xc => common::swap_weapon(g, p, Player::WEAPON_B, false),
        16 => items::cast(g, p, &CAST_16, STYLE),
        19 => items::cast(g, p, &CAST_19, STYLE),
        9 | 10 | 13..=15 | 17 | 18 | 20 => {
            items::run_shared(g, p, which, STYLE);
        }
        0x18 => sparks(g, p),
        0x19 => bolt(g, p),
        0x1a => frost(g, p),
        0x1b => quake(g, p),
        0x1c => storm(g, p),
        0x1d => rain(g, p),
        0x1e => witch(g, p),
        0x1f => curse(g, p),
        0x20 => wave(g, p),
        0x21 => portal(g, p),
        0x22 => summon(g, p),
        0x23 => wall(g, p),
        0x24 => drain(g, p),
        _ => {
            to_stand(g, p);
        }
    }
}

/// Plays ROM sounds.
fn sounds(g: &mut Ctx, p: Player, list: &[u32]) {
    for &sound in list {
        g.sound(sound, p);
    }
}

/// End of the spell at the final step: the screen is restored, without touching the cancellation flag.
fn finish_kept(g: &mut Ctx, p: Player) {
    cov!();
    common::end_spell_keep_cancel_mode(g, p, UNDIM);
}

/// Last step of most spells: the animation finishes, then the spell ends (without restoring the
/// cancellation flag).
fn end_when_done(g: &mut Ctx, p: Player) {
    cov!();
    if animation::step(g, p) {
        finish_kept(g, p);
    }
}

/// Exit step of cancelled spells: the animation advances, the countdown decreases and at zero the spell ends
/// (cancellation flag restored).
fn linger(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    if countdown(g, p) {
        common::end_spell_mode(g, p, UNDIM);
    }
}

/// ROM word (code) indexed by the caster's level.
fn by_level(g: &mut Ctx, p: Player, table: u32) -> u16 {
    let level = p.u8(g, Player::LEVEL) as u32;
    g.prog16(table + 2 * level)
}

/// Bytes (code): variant number of the gesture of spell 0x18 per level.
const SPARK_GESTURES: u32 = 0x46a46;

/// 0x469b0, no. 0x18 (h): burst of sparks, alternating two variants at each gesture.
fn sparks(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => common::start_cast(g, p, 0x3c, Some(0x151), 0x36),
        2 => {
            animation::step(g, p);
            if take_event(g, p) {
                crate::families::creators::for_players::sparks(&mut g.leftovers, g.b, p.obj());
                g.sound(0xe84c6, p);
            }
            if countdown(g, p) {
                next_step(g, p);
                p.set_u8(g, 0x1e8, 0);
                let level = p.u8(g, Player::LEVEL) as u32;
                let gesture = g.prog8(SPARK_GESTURES + level) as u16;
                animation::start_plain(g, p, 0x36 + gesture);
                g.sound(0xe2f3a, p);
            }
        }
        4 => {
            if p.u8(g, field::EVENT) != 0 {
                let side = p.u8(g, 0x1e8) ^ 1;
                p.set_u8(g, 0x1e8, side);
                if is_cancelled(g, p) {
                    p.set_u8(g, Player::STEP, 8);
                    common::cancel_cast(g, p, 0x1c);
                    return linger_quiet(g, p);
                }
                spawn::shower(g, p, (side == 0) as u8);
                p.set_u8(g, field::EVENT, 0);
            }
            if animation::step(g, p) {
                next_step(g, p);
                animation::start_plain(g, p, 0x3c);
            }
        }
        6 => end_when_done(g, p),
        8 => linger_quiet(g, p),
        _ => {}
    }
}

/// Step 8 of the cancelled burst (0x46ae6): the countdown decreases; at zero a last animation frame
/// then the end of the spell.
fn linger_quiet(g: &mut Ctx, p: Player) {
    cov!();
    if countdown(g, p) {
        animation::step(g, p);
        common::end_spell_mode(g, p, UNDIM);
    }
}

/// 0x46b1e, no. 0x19 (h): lightning bolt in two stages, then a final pose.
fn bolt(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => common::start_cast(g, p, 0x28, Some(0x153), 0x1c),
        2 => {
            animation::step(g, p);
            if take_event(g, p) {
                crate::families::creators::for_players::bolt(&mut g.leftovers, g.b, p.obj(), 0);
                crate::placement::load_effect_palette(g.b, crate::placement::effect_palette::LIGHT);
                g.sound(0xe2a34, p);
            }
            if countdown(g, p) {
                next_step(g, p);
                p.set_u16(g, field::TIMER, 0x30);
                animation::start_plain(g, p, 0x1d);
            }
        }
        4 => {
            animation::step(g, p);
            if take_event(g, p) {
                g.sound(0xe88d0, p);
                if is_cancelled(g, p) {
                    p.set_u8(g, Player::STEP, 0xa);
                    common::cancel_cast(g, p, 0x1d);
                    return linger(g, p);
                }
                crate::families::creators::for_players::bolt(&mut g.leftovers, g.b, p.obj(), 1);
            }
            if countdown(g, p) {
                next_step(g, p);
                p.set_u16(g, field::TIMER, 0x5a);
                animation::start_plain(g, p, 0x1e);
                sounds(g, p, &[0xe893a, 0xe2f7a]);
            }
        }
        6 => {
            animation::step(g, p);
            if countdown(g, p) {
                next_step(g, p);
                animation::start_plain(g, p, 0x1f);
            }
        }
        8 => end_when_done(g, p),
        0xa => linger(g, p),
        _ => {}
    }
}

/// 0x46c92, no. 0x1a (h): freeze. During the final phase the player sets the intensity with up and down;
/// the screen shakes then its colors are restored.
fn frost(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => common::start_cast(g, p, 0x2c, Some(0x153), 0x20),
        2 => {
            if countdown(g, p) {
                next_step(g, p);
                p.set_u16(g, field::TIMER, 0x28);
                animation::start_plain(g, p, 0x21);
            } else {
                animation::step(g, p);
            }
        }
        4 => {
            animation::step(g, p);
            if take_event(g, p) {
                spawn::ice_crown(g, p);
                sounds(g, p, &[0xe2a34, 0xe860c]);
            }
            if countdown(g, p) {
                next_step(g, p);
                p.set_u8(g, 0x1e8, 0x10);
                p.set_u8(g, 0x1e9, 2);
                p.set_u16(g, field::TIMER, 0x78);
                animation::start_plain(g, p, 0x22);
                g.sound(0xe2fba, p);
            }
        }
        6 => {
            if !animation::step(g, p) {
                return;
            }
            next_step(g, p);
            p.set_u8(g, 0x1eb, 0);
            p.set_u8(g, 0x1ea, 4);
            p.set_u8(g, 0x1e8, 0x10);
            p.set_u8(g, 0x1e9, 2);
            animation::start_plain(g, p, 0x23 + 2);
            if is_cancelled(g, p) {
                p.set_u8(g, Player::STEP, 0xc);
                common::cancel_cast(g, p, 0x1e);
                return frost_wait(g, p);
            }
            spawn::ice_spell(g, p);
            crate::palette::set_flash_color(g.b, crate::palette::FLASH_COLOR_WHITE_VALUE);
        }
        8 => {
            common::blink_screen(g, p);
            if countdown(g, p) {
                next_step(g, p);
                p.set_u8(g, 0x1eb, 0);
                animation::start_plain(g, p, 0x28);
                return;
            }
            common::flash_screen(g, p);
            common::intensity_keys(g, p, 0x23);
        }
        0xa => {
            if animation::step(g, p) {
                frost_end(g, p);
            }
        }
        0xc => frost_wait(g, p),
        _ => {}
    }
}

/// Step 0xc of the cancelled freeze: the countdown decreases, then the freeze ends.
fn frost_wait(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    if countdown(g, p) {
        frost_end(g, p);
    }
}

/// End of the freeze: the screen colors are restored (D0 = 0xa000) before returning to the standing state.
fn frost_end(g: &mut Ctx, p: Player) {
    cov!();
    common::clear_spell_state(g);
    ram::FADE_DELAY.set(g.b, 2);
    ram::FADE_PAGES.set(g.b, 0xc);
    crate::palette::set_brightness(g.b, 0xa000);
    common::undim_screen_mode(g, p, UNDIM);
    to_stand(g, p);
}

/// Words (code): earthquake duration per level.
const QUAKE_DURATIONS: u32 = 0x46fd0;

/// Earthquake damage (0x470e4) to the bosses then the enemies.
const QUAKE_STRIKE: Strike = Strike {
    damage: Damage::Table(0x10_442c, 0x10_440e),
    live: Live::Bit0,
    needs_hp: true,
    grounded: false,
    immune_bit: 4,
    reaction: 0x90,
    random_facing: true,
    boosts: &[Boost::Boss(0xc), Boost::Held(0x20, 2), Boost::Held(0x21, 1), Boost::Armor, Boost::Charm(5)],
    undead_die: true,
    sound: None,
};

/// 0x46f2e, no. 0x1b (h): earthquake.
fn quake(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            common::begin_cast(g, p, Some(0x30), Some(0x154));
            common::dim_screen(g, p);
            { crate::palette::flash(g.b, crate::palette::Flash::A); }
            animation::start_plain(g, p, 0x3d);
        }
        2 => {
            animation::step(g, p);
            if countdown(g, p) {
                next_step(g, p);
                let duration = by_level(g, p, QUAKE_DURATIONS);
                p.set_u16(g, field::TIMER, duration);
                animation::start_plain(g, p, 0x3e);
                g.sound(0xe2ffa, p);
            } else if take_event(g, p) {
                crate::families::creators::for_players::quake(&mut g.leftovers, g.b, p.obj(), true);
                crate::placement::load_effect_palette(g.b, crate::placement::effect_palette::NORMAL);
                g.sound(0xe8db4, p);
            }
        }
        4 => {
            animation::step(g, p);
            if take_event(g, p) {
                if is_cancelled(g, p) {
                    p.set_u8(g, Player::STEP, 8);
                    common::cancel_cast(g, p, 0x1f);
                    common::item_spark(g, p, 0x20);
                    crate::palette::restore_flash(g.b);
                    return linger(g, p);
                }
                crate::families::creators::for_players::quake(&mut g.leftovers, g.b, p.obj(), false);
                crate::families::small::quake_b(g.b, p.obj());
                g.sound(0xe8b22, p);
            }
            if countdown(g, p) {
                next_step(g, p);
                animation::start_plain(g, p, 0x3f);
                crate::palette::restore_flash(g.b);
            } else if p.u16(g, field::TIMER) == 0x64 {
                foes::strike_foes(g, p, &QUAKE_STRIKE);
            }
        }
        6 => end_when_done(g, p),
        8 => linger(g, p),
        _ => {}
    }
}

/// Damage of the storm to the right (0x47370) and to the left (0x47750).
const STORM_STRIKE: Strike = Strike {
    damage: Damage::Table(0x10_4468, 0x10_444a),
    live: Live::Bit0,
    needs_hp: true,
    grounded: false,
    immune_bit: 5,
    reaction: 0x60,
    random_facing: true,
    boosts: &[
        Boost::Boss(0xb),
        Boost::Held(0x1f, 2),
        Boost::Held(0x20, 2),
        Boost::Held(0x21, 1),
        Boost::Armor,
        Boost::Charm(4),
    ],
    undead_die: true,
    sound: None,
};

const STORM_STRIKE_LEFT: Strike = Strike {
    immune_bit: 6,
    reaction: 0x74,
    boosts: &[Boost::Held(0x1f, 2), Boost::Held(0x20, 2), Boost::Held(0x21, 1), Boost::Armor],
    sound: Some(0xe98fa),
    ..STORM_STRIKE
};

/// 0x4722a, no. 0x1c (h): storm. Two variants depending on facing: to the right (steps 0 to 6, bolt
/// then effect 0x4cd8e) or to the left (steps 8 to 0xe, effect 0x4d020).
fn storm(g: &mut Ctx, p: Player) {
    cov!();
    let step = p.u8(g, Player::STEP);
    match step {
        0 | 8 => {
            let left = step == 8 || p.u8(g, Player::FACING) != 0;
            if step == 0 && left {
                p.set_u8(g, Player::STEP, 8);
            }
            common::start_cast(g, p, if left { 0x3c } else { 0x30 }, Some(0x155), 0x29);
        }
        2 | 0xa => {
            animation::step(g, p);
            if take_event(g, p) {
                crate::families::creators::for_players::bolt(&mut g.leftovers, g.b, p.obj(), 0);
                crate::placement::load_effect_palette(g.b, if step == 2 { crate::placement::effect_palette::LIGHT } else { crate::placement::effect_palette::NORMAL });
                g.sound(0xe2a74, p);
                g.sound(if step == 2 { 0xe97b4 } else { 0xe974a }, p);
            }
            if countdown(g, p) {
                next_step(g, p);
                p.set_u16(g, field::TIMER, if step == 2 { 0xc0 } else { 0xb0 });
                animation::start_plain(g, p, 0x2a);
                g.sound(0xe303a, p);
            }
        }
        4 | 0xc => {
            animation::step(g, p);
            if take_event(g, p) {
                if is_cancelled(g, p) {
                    p.set_u8(g, Player::STEP, 0x10);
                    common::cancel_cast(g, p, 0x21);
                    return linger(g, p);
                }
                if step == 4 {
                    spawn::flare(g, p);
                    { crate::palette::flash(g.b, crate::palette::Flash::Gray); };
                } else {
                    spawn::storm_spell(g, p);
                    { crate::palette::flash(g.b, crate::palette::Flash::B); };
                    spawn::storm_cloud(g, p);
                }
            }
            if p.u16(g, field::TIMER) == 0x60 {
                foes::strike_foes(g, p, if step == 4 { &STORM_STRIKE } else { &STORM_STRIKE_LEFT });
            }
            if countdown(g, p) {
                next_step(g, p);
                animation::start_plain(g, p, 0x2b);
            }
        }
        6 | 0xe => {
            if animation::step(g, p) {
                crate::palette::restore_flash(g.b);
                finish_kept(g, p);
            }
        }
        0x10 => linger(g, p),
        _ => {}
    }
}

/// 0x4788a, no. 0x1d (h): rain, in two stages.
fn rain(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => common::start_cast(g, p, 0x2c, Some(0x156), 0x33),
        2 => {
            animation::step(g, p);
            if take_event(g, p) {
                spawn::rain_glow(g, p, 0);
                crate::placement::load_effect_palette(g.b, crate::placement::effect_palette::NORMAL);
                sounds(g, p, &[0xe2a34, 0xe9424]);
            }
            if countdown(g, p) {
                next_step(g, p);
                p.set_u16(g, field::TIMER, 0x60);
                animation::start_plain(g, p, 0x34);
                g.sound(0xe317a, p);
            }
        }
        4 => {
            animation::step(g, p);
            if take_event(g, p) {
                if is_cancelled(g, p) {
                    p.set_u8(g, Player::STEP, 8);
                    common::cancel_cast(g, p, 0x22);
                    return linger(g, p);
                }
                spawn::rain_glow(g, p, 1);
                spawn::rain(g, p);
                g.sound(0xe948e, p);
            }
            if countdown(g, p) {
                next_step(g, p);
                animation::start_plain(g, p, 0x35);
            }
        }
        6 => end_when_done(g, p),
        8 => {
            animation::step(g, p);
            if countdown(g, p) {
                finish_kept(g, p);
            }
        }
        _ => {}
    }
}

/// 0x479e0, no. 0x1e (h): witch. The spell summons (0x53918) then, depending on whether the witch hit
/// (`+0x1d4`), a long or short phase; gestures 1 and 2 trigger two effects.
fn witch(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => common::start_cast(g, p, 0x14, Some(0x154), 0x17),
        2 => {
            if countdown(g, p) {
                next_step(g, p);
                p.set_u8(g, 0x1e8, 0);
                p.set_u32(g, 0x1d0, 0);
                p.set_u16(g, 0x1d4, 0);
                p.set_u16(g, field::TIMER, 0x2c);
                animation::start_plain(g, p, 0x18);
                g.sound(0xe2a34, p);
                g.sound(0xe960c, p);
                return;
            }
            if p.u16(g, field::TIMER) == 0x13 && !is_cancelled(g, p) {
                spawn::witch(g, p);
            }
            animation::step(g, p);
        }
        4 => {
            if countdown(g, p) {
                next_step(g, p);
                crate::placement::load_effect_palette(g.b, crate::placement::effect_palette::LIGHT);
                let (timer, anim) = if p.u8(g, 0x1e8) != 0 { (0x80, 0x1a) } else { (0x50, 0x19) };
                p.set_u16(g, field::TIMER, timer);
                animation::start_plain(g, p, anim);
                g.sound(0xe31ba, p);
                g.sound(0xe9676, p);
                return;
            }
            if is_cancelled(g, p) {
                if p.u16(g, field::TIMER) <= 0x10 {
                    p.set_u8(g, Player::STEP, 8);
                    common::cancel_cast(g, p, 0x23);
                    common::item_spark(g, p, 0x24);
                    return linger(g, p);
                }
            } else if p.u16(g, 0x1d4) != 0 {
                p.set_u8(g, 0x1e8, 1);
            }
            animation::step(g, p);
        }
        6 => {
            if countdown(g, p) {
                return finish_kept(g, p);
            }
            match p.u8(g, field::EVENT) {
                1 => {
                    p.set_u8(g, field::EVENT, 0);
                    spawn::witch_strike(g, p, 1);
                }
                2 => {
                    p.set_u8(g, field::EVENT, 0);
                    spawn::witch_strike(g, p, 4);
                }
                _ => {}
            }
            animation::step(g, p);
        }
        8 => linger(g, p),
        _ => {}
    }
}

/// Words (code): value of `+0x196` set by spell 0x1f, per level.
const CURSE_DURATIONS: u32 = 0x47c4e;

/// 0x47b88, no. 0x1f (h): curse: at the end the player receives a countdown (`+0x196`).
fn curse(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => common::start_cast(g, p, 0x18, Some(0x156), 0x29),
        2 => {
            animation::step(g, p);
            if countdown(g, p) {
                next_step(g, p);
                p.set_u16(g, field::TIMER, 0x30);
                animation::start_plain(g, p, 0x2a);
                g.sound(0xe31fa, p);
            }
        }
        4 => {
            animation::step(g, p);
            if p.u16(g, field::TIMER) == 0x18 {
                g.sound(0xea106, p);
            }
            if countdown(g, p) {
                let timer = by_level(g, p, CURSE_DURATIONS);
                p.set_u16(g, Player::DYING_TIMER, timer);
                common::end_spell_mode(g, p, UNDIM);
            }
        }
        _ => {}
    }
}

/// 0x47c8a, no. 0x20 (h): wave: two effects at the gesture.
fn wave(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => common::start_cast(g, p, 0x60, Some(0x157), 0x40),
        2 => {
            if is_cancelled(g, p) && p.u16(g, field::TIMER) <= 0x50 {
                p.set_u8(g, Player::STEP, 8);
                common::cancel_cast(g, p, 0x25);
                common::item_spark(g, p, 0x26);
                return linger(g, p);
            }
            if countdown(g, p) {
                next_step(g, p);
                p.set_u16(g, field::TIMER, 0x30);
                animation::start_plain(g, p, 0x41);
                g.sound(0xe30ba, p);
            } else if take_event(g, p) {
                spawn::wave_bolt(g, p, false);
                spawn::wave_bolt(g, p, true);
                crate::placement::load_effect_palette(g.b, crate::placement::effect_palette::LIGHT);
                g.sound(0xe9240, p);
            } else {
                animation::step(g, p);
            }
        }
        4 => {
            if countdown(g, p) {
                next_step(g, p);
                animation::start_plain(g, p, 0x3f);
            } else {
                animation::step(g, p);
            }
        }
        6 => end_when_done(g, p),
        8 => linger(g, p),
        _ => {}
    }
}

/// Words (code): duration of the stay of spell 0x21 per level, and horizontal speeds of the leap by
/// distance (steps of 16 pixels).
const PORTAL_DURATIONS: u32 = 0x47ee8;
const PORTAL_SPEEDS: u32 = 0x47e78;
const REGROUP_SPEEDS: u32 = 0x4851c;

/// Damage of spell 0x21 (0x48012): without state test, without turning the monsters around.
const PORTAL_STRIKE: Strike = Strike {
    damage: Damage::Table(0x10_43f0, 0x10_43d2),
    live: Live::Unchecked,
    needs_hp: true,
    grounded: false,
    immune_bit: 0xc,
    reaction: 0x80,
    random_facing: false,
    boosts: &[Boost::Held(0x1f, 2), Boost::Held(0x20, 2), Boost::Held(0x21, 1), Boost::Armor],
    undead_die: false,
    sound: None,
};

/// Start of the leap toward the screen center (0x47e1e, 0x484c2): the magic-user turns toward the center
/// of the screen and takes the speed (by distance, every 16 pixels) from the table `speeds`.
fn aim_at_center(g: &mut Ctx, p: Player, speeds: u32, vz: u16, az: i16) {
    cov!();
    let mut center = ram::CAMERA_X.get(g.b);
    if ram::USE_ALT_CAMERA.get(g.b) != 0 {
        center = ram::CAMERA_ALT_X.get(g.b);
    }
    let center = center.wrapping_add(0xc0);
    let x = p.u16(g, Obj::X);
    p.set_u8(g, Player::FACING, 0);
    if center > x {
        p.set_u8(g, Player::FACING, 0x20);
    }
    let mut distance = center.wrapping_sub(x) as i16;
    if distance < 0 {
        distance = distance.wrapping_neg();
    }
    let mut index = 0u8;
    loop {
        distance = distance.wrapping_sub(0x10);
        if distance < 0 {
            break;
        }
        index = index.wrapping_add(2);
    }
    let speed = g.prog16(speeds + index as u32);
    let speed = if p.u8(g, Player::FACING) != 0 { speed } else { (speed as i16).wrapping_neg() as u16 };
    p.set_u16(g, Obj::VX, speed);
    p.set_u16(g, Obj::VZ, vz);
    p.set_u16(g, Obj::AZ, az as u16);
    animation::start_plain(g, p, 0x2f);
}

/// 0x47dd6, no. 0x21 (h): portal. The magic-user leaps to the screen center (steps 0 to 2), stays there while
/// the monsters are hurt (step 4), then lands (steps 6 and 8).
fn portal(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            common::begin_cast(g, p, Some(0x3c), Some(0x155));
            common::dim_screen(g, p);
            aim_at_center(g, p, PORTAL_SPEEDS, 0x560, -0x38);
        }
        2 => {
            move_xz(g, p);
            if !animation::step(g, p) {
                return;
            }
            next_step(g, p);
            if is_cancelled(g, p) {
                p.set_u16(g, field::TIMER, 1);
                animation::start_plain(g, p, 0x30);
                return;
            }
            let duration = by_level(g, p, PORTAL_DURATIONS);
            p.set_u16(g, field::TIMER, duration);
            crate::placement::load_effect_palette(g.b, crate::placement::effect_palette::NORMAL);
            spawn::portal(g, p, 2);
            animation::start_plain(g, p, 0x30);
            g.sound(0xe91d6, p);
            g.sound(0xe2a34, p);
        }
        4 => {
            if !is_cancelled(g, p) {
                animation::step(g, p);
                if take_event(g, p) {
                    spawn::portal(g, p, 0);
                    spawn::portal(g, p, 1);
                    { crate::palette::flash(g.b, crate::palette::Flash::E); };
                    foes::strike_foes(g, p, &PORTAL_STRIKE);
                    g.sound(0xe307a, p);
                    g.sound(0xe9176, p);
                }
            }
            if countdown(g, p) {
                next_step(g, p);
                p.set_u32(g, Obj::VX, 0);
                p.set_u16(g, Obj::VZ, 0);
                if !is_cancelled(g, p) {
                    crate::palette::restore_flash(g.b);
                }
                animation::start_plain(g, p, 0x31);
            }
        }
        6 => {
            move_z(g, p);
            if (p.u16(g, Obj::Z) as i16) < 0 {
                next_step(g, p);
                p.set_u32(g, Obj::Z, 0);
                animation::start_plain(g, p, 0x32);
            }
        }
        8 => {
            if animation::step(g, p) {
                common::end_spell_mode(g, p, UNDIM);
            }
        }
        _ => {}
    }
}

/// 0x480ec, no. 0x22 (h): summoning of a creature (0x4ccac) that lasts 360 frames.
fn summon(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => common::start_cast(g, p, 0x16, Some(0x159), 0x14),
        2 => {
            if !countdown(g, p) {
                animation::step(g, p);
                return;
            }
            next_step(g, p);
            if is_cancelled(g, p) {
                p.set_u8(g, Player::STEP, 8);
                p.set_u16(g, field::TIMER, 0x15);
            } else {
                p.set_u16(g, field::TIMER, 0x168);
                spawn::summon(g, p);
                g.sound(0xe9cd4, p);
            }
            g.sound(0xe30fa, p);
            animation::start_plain(g, p, 0x15);
        }
        4 => {
            if countdown(g, p) {
                next_step(g, p);
                animation::start_plain(g, p, 0x16);
            } else {
                animation::step(g, p);
            }
        }
        6 => end_when_done(g, p),
        8 => {
            if countdown(g, p) {
                common::end_spell_mode(g, p, UNDIM);
                return;
            }
            let timer = p.u16(g, field::TIMER);
            if timer == 0x10 {
                common::item_spark(g, p, 0x27);
                common::item_spark(g, p, 0x28);
            } else if timer > 0x10 {
                animation::step(g, p);
            }
        }
        _ => {}
    }
}

/// Fire wall damage (0x48392): single value, without hit point test.
const WALL_STRIKE: Strike = Strike {
    damage: Damage::Fixed(0x46),
    live: Live::Bit0,
    needs_hp: false,
    grounded: false,
    immune_bit: 0xe,
    reaction: 0x7c,
    random_facing: false,
    boosts: &[Boost::Held(0x20, 2), Boost::Held(0x21, 1), Boost::Armor],
    undead_die: true,
    sound: Some(0xe93ba),
};

/// 0x4822a, no. 0x23 (h): fire wall: an effect at the gesture, then fixed damage at the right moment.
fn wall(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => common::start_cast(g, p, 0x28, Some(0x159), 0x2c),
        2 => {
            animation::step(g, p);
            if take_event(g, p) {
                spawn::wall_spark(g, p, 0);
                crate::placement::load_effect_palette(g.b, crate::placement::effect_palette::LIGHT);
                g.sound(0xe92c6, p);
            }
            if countdown(g, p) {
                next_step(g, p);
                if is_cancelled(g, p) {
                    p.set_u8(g, Player::STEP, 8);
                    common::cancel_cast(g, p, 0x29);
                    animation::start_plain(g, p, 0x2d);
                    return;
                }
                p.set_u16(g, field::TIMER, 0x40);
                spawn::wall_spark(g, p, 1);
                animation::start_plain(g, p, 0x2d);
                g.sound(0xe313a, p);
                g.sound(0xe9330, p);
            }
        }
        4 => {
            animation::step(g, p);
            if countdown(g, p) {
                next_step(g, p);
                animation::start_plain(g, p, 0x2e);
            } else if p.u16(g, field::TIMER) == 0x20 {
                foes::strike_foes(g, p, &WALL_STRIKE);
            }
        }
        6 => end_when_done(g, p),
        8 => linger(g, p),
        _ => {}
    }
}

/// 0x4847a, no. 0x24 (h): sacrifice. The magic-user leaps to the screen center, opens a portal, then their
/// rods disappear and all living players drop to one hit point.
fn drain(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            common::begin_cast(g, p, Some(0x3c), None);
            p.set_u8(g, 0x1ef, 0);
            common::dim_screen(g, p);
            aim_at_center(g, p, REGROUP_SPEEDS, 0x200, -8);
        }
        2 => {
            move_xz(g, p);
            if animation::step(g, p) {
                next_step(g, p);
                p.set_u16(g, field::TIMER, 0x120);
                crate::placement::load_effect_palette(g.b, crate::placement::effect_palette::NORMAL);
                spawn::portal(g, p, 2);
                animation::start_plain(g, p, 0x30);
                g.sound(0xe91d6, p);
            }
        }
        4 => {
            animation::step(g, p);
            if take_event(g, p) {
                spawn::portal_glow(g, p);
                spawn::portal(g, p, 0);
                spawn::portal_sparks(g, p);
                g.sound(0xe323a, p);
            }
            if countdown(g, p) {
                next_step(g, p);
                p.set_u32(g, Obj::VX, 0);
                p.set_u16(g, Obj::VZ, 0);
                animation::start_plain(g, p, 0x31);
                return;
            }
            let timer = p.u16(g, field::TIMER);
            if timer <= 0x100 {
                let tally = p.u8(g, 0x1ef);
                if spawn::portal_burst(g, p, tally) {
                    p.set_u8(g, 0x1ef, tally.wrapping_add(1));
                }
            }
            if timer == 0xe0 {
                sacrifice(g, p);
            }
        }
        6 => {
            move_z(g, p);
            if (p.u16(g, Obj::Z) as i16) < 0 {
                next_step(g, p);
                p.set_u32(g, Obj::Z, 0);
                animation::start_plain(g, p, 0x32);
                p.set_u8(g, Player::WEAPON_B, 0x22);
                for i in 0..6u32 {
                    if p.u8(g, 0x140 + i) == 0x4f {
                        p.set_u8(g, 0x140 + i, 0x56);
                        p.set_u8(g, 0x148 + i, 0);
                    }
                }
            }
        }
        8 => {
            if animation::step(g, p) {
                common::end_spell_mode(g, p, UNDIM);
            }
        }
        _ => {}
    }
}

/// 0x4864c: the sacrifice. All unfrozen monsters in play are struck (reaction 0xa8): bosses of
/// type 0xf or 0x14 lose 500 hit points, the others keep only one; then all living players
/// drop to one hit point.
fn sacrifice(g: &mut Ctx, p: Player) {
    cov!();
    use crate::objects::pools::{BOSS, ENEMY};
    let (index, character) = (p.u8(g, Player::INDEX), p.u8(g, Player::CHARACTER));
    for pool in [&BOSS, &ENEMY] {
        for o in pool.iter() {
            if o.u8(g.b, 0) == 0 || o.u8(g.b, 1) == 0 || o.u8(g.b, 0x40) != 0 {
                continue;
            }
            let counter = p.u16(g, 0x190).wrapping_add(2);
            p.set_u16(g, 0x190, counter);
            let hits = o.u8(g.b, 0x9c).wrapping_add(1);
            o.set_u8(g.b, 0x9c, hits);
            o.set_u32(g.b, 4, 0x0400_0000);
            o.set_u8(g.b, 1, 1);
            o.set_u8(g.b, 0x49, 0xa8);
            o.set_u8(g.b, 0x48, index);
            o.set_u8(g.b, 0x4b, character);
            o.set_u16(g.b, 0x54, p.0 as u16);
            o.set_u8(g.b, 0x4a, 0);
            if g.random() & 1 != 0 {
                o.set_u8(g.b, 0x4a, 0x20);
            }
            let kind = o.u8(g.b, Obj::KIND);
            if o.u8(g.b, 0x28) == 4 && matches!(kind, 0xf | 0x14) {
                let hp = (o.u16(g.b, 0x62) as i16).wrapping_sub(0x1f4);
                o.set_u16(g.b, 0x62, if hp < 0 { 0 } else { hp as u16 });
            } else {
                o.set_u16(g.b, 0x62, 1);
            }
            if o.u16(g.b, 0x62) == 0 && o.u8(g.b, 0x93) != 0 {
                p.set_u8(g, 0x18a, 1);
                let loot = o.u8(g.b, 0x9a);
                ram::KILL_FLAG.set(g.b, loot);
            }
            let bar = (index as u32) << 8 | kind as u32;
            let (max, hp, class) = (o.u16(g.b, 0x60) as u32, o.u16(g.b, 0x62) as u32, o.u8(g.b, 0x28) as u32);
            crate::events::create_popup(g.b, crate::events::Popup { header: bar as u16, max: max as u16, value: hp as u16, raw: class as u8 == 2 });
        }
    }
    for q in Player::all() {
        if q.u8(g, Player::ACTIVE) != 0 && q.u16(g, Player::HP) != 0 {
            q.set_u16(g, Player::HP, 1);
        }
    }
}

action_native!(JUMP, 0x45a78, "mage_jump", jump);
action_native!(ATTACK, 0x45cfc, "mage_attack", attack);
action_native!(SPECIAL, 0x4619e, "mage_special", special);
action_native!(DASH, 0x4879a, "mage_dash", dash);
