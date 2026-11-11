//! Special attack of the magic-user and its first spells (sparks, bolt, frost, quake).

use crate::world::objects::player::Player;
use super::*;
use crate::actors::player::tables::{player_data, pc16, Read};

// ---- special attack (0x4619e) --------------------------------------------------------------

/// The magic-user's animations for cast spells (see `items::Style`): the target of lightning and the wall
/// is kept at `+0x1e8`, the screen is restored with mode 1.
pub(super) const STYLE: items::Style = items::Style {
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
pub(super) const UNDIM: u16 = 1;

/// The magic-user's red ring (no. 16): the countdown lasts 30 frames and the aura is only placed at the end.
pub(super) const CAST_16: items::Cast = items::Cast {
    frames: 0x1e,
    fire: |g, p, _| items::ring_fire(g, p),
    on_end: |g, p| items::red_aura(g, p, 0x8000),
    ..items::CAST_13
};

/// The magic-user's healing potion (no. 19) does not initialize the blinking.
pub(super) const CAST_19: items::Cast = items::Cast { prepare: common::nothing, ..items::CAST_19 };

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
pub(super) fn sounds(g: &mut Ctx, p: Player, list: &[crate::services::sound::Sound]) {
    for &sound in list {
        g.sound(sound, p);
    }
}

/// End of the spell at the final step: the screen is restored, without touching the cancellation flag.
pub(super) fn finish_kept(g: &mut Ctx, p: Player) {
    cov!();
    common::end_spell_keep_cancel_mode(g, p, UNDIM);
}

/// Last step of most spells: the animation finishes, then the spell ends (without restoring the
/// cancellation flag).
pub(super) fn end_when_done(g: &mut Ctx, p: Player) {
    cov!();
    if animation::step(g, p) {
        finish_kept(g, p);
    }
}

/// Exit step of cancelled spells: the animation advances, the countdown decreases and at zero the spell ends
/// (cancellation flag restored).
pub(super) fn linger(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    if countdown(g, p) {
        common::end_spell_mode(g, p, UNDIM);
    }
}

/// ROM word (code) indexed by the caster's level.
pub(super) fn by_level(g: &mut Ctx, p: Player, table: u32) -> u16 {
    let level = p.u8(g, Player::LEVEL) as u32;
    pc16(g.b, table + 2 * level)
}


/// 0x469b0, no. 0x18 (h): burst of sparks, alternating two variants at each gesture.
pub(super) fn sparks(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => common::start_cast(g, p, 0x3c, Some(0x151), 0x36),
        2 => {
            animation::step(g, p);
            if take_event(g, p) {
                crate::actors::families::creators::for_players::sparks(&mut g.leftovers, g.b, p.obj());
                g.sound(crate::services::sound::CMD_160, p);
            }
            if countdown(g, p) {
                next_step(g, p);
                p.set_u8(g, Player::ACTION_COUNT, 0);
                let level = p.u8(g, Player::LEVEL) as u32;
                let gesture = player_data().magic_user_spark_gestures.at8(g.b, level) as u16;
                animation::start_plain(g, p, 0x36 + gesture);
                g.sound(crate::services::sound::CMD_70, p);
            }
        }
        4 => {
            if p.u8(g, field::EVENT) != 0 {
                let side = p.u8(g, Player::ACTION_COUNT) ^ 1;
                p.set_u8(g, Player::ACTION_COUNT, side);
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
pub(super) fn linger_quiet(g: &mut Ctx, p: Player) {
    cov!();
    if countdown(g, p) {
        animation::step(g, p);
        common::end_spell_mode(g, p, UNDIM);
    }
}

/// 0x46b1e, no. 0x19 (h): lightning bolt in two stages, then a final pose.
pub(super) fn bolt(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => common::start_cast(g, p, 0x28, Some(0x153), 0x1c),
        2 => {
            animation::step(g, p);
            if take_event(g, p) {
                crate::actors::families::creators::for_players::bolt(&mut g.leftovers, g.b, p.obj(), 0);
                crate::world::placement::load_effect_palette(g.b, crate::world::placement::effect_palette::LIGHT);
                g.sound(crate::services::sound::CMD_F6, p);
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
                g.sound(crate::services::sound::CMD_167, p);
                if is_cancelled(g, p) {
                    p.set_u8(g, Player::STEP, 0xa);
                    common::cancel_cast(g, p, 0x1d);
                    return linger(g, p);
                }
                crate::actors::families::creators::for_players::bolt(&mut g.leftovers, g.b, p.obj(), 1);
            }
            if countdown(g, p) {
                next_step(g, p);
                p.set_u16(g, field::TIMER, 0x5a);
                animation::start_plain(g, p, 0x1e);
                sounds(g, p, &[crate::services::sound::CMD_168, crate::services::sound::CMD_71]);
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
pub(super) fn frost(g: &mut Ctx, p: Player) {
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
                sounds(g, p, &[crate::services::sound::CMD_F6, crate::services::sound::CMD_163]);
            }
            if countdown(g, p) {
                next_step(g, p);
                p.set_u8(g, Player::ACTION_COUNT, 0x10);
                p.set_u8(g, Player::SPELL_LEVEL, 2);
                p.set_u16(g, field::TIMER, 0x78);
                animation::start_plain(g, p, 0x22);
                g.sound(crate::services::sound::CMD_72, p);
            }
        }
        6 => {
            if !animation::step(g, p) {
                return;
            }
            next_step(g, p);
            p.set_u8(g, Player::SPELL_PHASE, 0);
            p.set_u8(g, Player::SPELL_BLINKS, 4);
            p.set_u8(g, Player::ACTION_COUNT, 0x10);
            p.set_u8(g, Player::SPELL_LEVEL, 2);
            animation::start_plain(g, p, 0x23 + 2);
            if is_cancelled(g, p) {
                p.set_u8(g, Player::STEP, 0xc);
                common::cancel_cast(g, p, 0x1e);
                return frost_wait(g, p);
            }
            spawn::ice_spell(g, p);
            crate::services::palette::set_flash_color(g.b, crate::services::palette::FLASH_COLOR_WHITE_VALUE);
        }
        8 => {
            common::blink_screen(g, p);
            if countdown(g, p) {
                next_step(g, p);
                p.set_u8(g, Player::SPELL_PHASE, 0);
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
pub(super) fn frost_wait(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    if countdown(g, p) {
        frost_end(g, p);
    }
}

/// End of the freeze: the screen colors are restored (D0 = 0xa000) before returning to the standing state.
pub(super) fn frost_end(g: &mut Ctx, p: Player) {
    cov!();
    common::clear_spell_state(g);
    ram::FADE_DELAY.set(g.b, 2);
    ram::FADE_PAGES.set(g.b, 0xc);
    crate::services::palette::set_brightness(g.b, 0xa000);
    common::undim_screen_mode(g, p, UNDIM);
    to_stand(g, p);
}

/// Words (code): earthquake duration per level.
pub(super) const QUAKE_DURATIONS: u32 = 0x46fd0;

/// Earthquake damage (0x470e4) to the bosses then the enemies.
pub(super) const QUAKE_STRIKE: Strike = Strike {
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
pub(super) fn quake(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            common::begin_cast(g, p, Some(0x30), Some(0x154));
            common::dim_screen(g, p);
            { crate::services::palette::flash(g.b, crate::services::palette::Flash::A); }
            animation::start_plain(g, p, 0x3d);
        }
        2 => {
            animation::step(g, p);
            if countdown(g, p) {
                next_step(g, p);
                let duration = by_level(g, p, QUAKE_DURATIONS);
                p.set_u16(g, field::TIMER, duration);
                animation::start_plain(g, p, 0x3e);
                g.sound(crate::services::sound::CMD_73, p);
            } else if take_event(g, p) {
                crate::actors::families::creators::for_players::quake(&mut g.leftovers, g.b, p.obj(), true);
                crate::world::placement::load_effect_palette(g.b, crate::world::placement::effect_palette::NORMAL);
                g.sound(crate::services::sound::CMD_16A, p);
            }
        }
        4 => {
            animation::step(g, p);
            if take_event(g, p) {
                if is_cancelled(g, p) {
                    p.set_u8(g, Player::STEP, 8);
                    common::cancel_cast(g, p, 0x1f);
                    common::item_spark(g, p, 0x20);
                    crate::services::palette::restore_flash(g.b);
                    return linger(g, p);
                }
                crate::actors::families::creators::for_players::quake(&mut g.leftovers, g.b, p.obj(), false);
                crate::actors::families::small::quake_b(g.b, p.obj());
                g.sound(crate::services::sound::CMD_16B, p);
            }
            if countdown(g, p) {
                next_step(g, p);
                animation::start_plain(g, p, 0x3f);
                crate::services::palette::restore_flash(g.b);
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
pub(super) const STORM_STRIKE: Strike = Strike {
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

pub(super) const STORM_STRIKE_LEFT: Strike = Strike {
    immune_bit: 6,
    reaction: 0x74,
    boosts: &[Boost::Held(0x1f, 2), Boost::Held(0x20, 2), Boost::Held(0x21, 1), Boost::Armor],
    sound: Some(crate::services::sound::CMD_34A_44A_B),
    ..STORM_STRIKE
};
