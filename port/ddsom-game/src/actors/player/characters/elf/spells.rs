//! Special attack of the elf and its spells (wards, beam, sparks, bolt, frost, quake, moonlight).

use crate::world::objects::Obj;
use crate::actors::player::Player;
use super::*;
use crate::actors::player::tables::{player_data, pc16, Read};

// ---- special attack (0x42b0c) --------------------------------------------------------------

/// The elf's animations for cast spells (see `items::Style`): the target of lightning and of the
/// wall is kept at `+0x1ec`, the monsters are hurt on the last frame of the lightning.
pub(super) const STYLE: items::Style = items::Style {
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
pub(super) const CAST_16: items::Cast = items::Cast {
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
pub(super) fn timed_throw(g: &mut Ctx, p: Player) {
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
pub(super) fn sounds(g: &mut Ctx, p: Player, list: &[crate::hw::sound::Sound]) {
    for &sound in list {
        g.sound(sound, p);
    }
}

/// End of the spells that first go through a final animation step (without restoring the cancellation flag).
pub(super) fn end_when_done(g: &mut Ctx, p: Player) {
    cov!();
    if animation::step(g, p) {
        common::end_spell_keep_cancel(g, p);
    }
}

/// ROM word (code) indexed by the caster's level.
pub(super) fn by_level(g: &mut Ctx, p: Player, table: u32) -> u16 {
    let level = p.u8(g, Player::LEVEL) as u32;
    pc16(g.b, table + 2 * level)
}

/// Aura for `target`: duration `duration` (`AURA_TIMER`), power 1, without the other flags.
pub(super) fn guard_aura(g: &mut Ctx, target: Player, duration: u16) {
    target.set_u16(g, Player::AURA_TIMER, duration);
    target.set_u8(g, Player::AURA_B, 1);
    target.set_u8(g, Player::AURA, 0);
    target.set_u8(g, Player::AURA_C, 0);
}

/// 0x43b04, no. 0x15 (h): red ring on the ally in front of the elf (within 76 pixels) or on themselves.
pub(super) fn ward_ally(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => common::start_cast(g, p, 0x20, Some(0x152), 0x3b),
        2 => {
            animation::step(g, p);
            if take_event(g, p) {
                spawn::ring(g, p, 0);
                crate::world::placement::load_effect_palette(g.b, crate::world::placement::effect_palette::RED);
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
pub(super) fn pick_ally(g: &mut Ctx, p: Player) {
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
pub(super) fn ward_party(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => common::start_cast(g, p, 0x20, Some(0x153), 0x38),
        2 => {
            animation::step(g, p);
            if take_event(g, p) {
                spawn::ring(g, p, 0);
                crate::world::placement::load_effect_palette(g.b, crate::world::placement::effect_palette::P360);
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
pub(super) const WARD_DURATIONS: u32 = 0x43f70;

/// 0x43d5e, no. 0x17 (h): ray of light in two stages.
pub(super) fn beam(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => common::start_cast(g, p, 0x38, Some(0x154), 0x29),
        2 => {
            animation::step(g, p);
            if take_event(g, p) {
                spawn::beam_cast(g, p, 0);
                crate::world::placement::load_effect_palette(g.b, crate::world::placement::effect_palette::LIGHT);
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


/// 0x433e4, no. 0x18 (h): burst of sparks, alternating two variants at each gesture.
pub(super) fn sparks(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => common::start_cast(g, p, 0x3c, Some(0x151), 0x34),
        2 => {
            animation::step(g, p);
            if take_event(g, p) {
                crate::actors::families::creators::for_players::sparks(&mut g.leftovers, g.b, p.obj());
                g.sound(rom::SPARK_SOUND, p);
            }
            if countdown(g, p) {
                next_step(g, p);
                p.set_u8(g, Player::ACTION_COUNT, 0);
                let level = p.u8(g, Player::LEVEL) as u32;
                let gesture = player_data().elf_spark_gestures.at8(g.b, level) as u16;
                animation::start_plain(g, p, 0x34 + gesture);
                g.sound(rom::SPARK_END_SOUND, p);
            }
        }
        4 => {
            if p.u8(g, field::EVENT) != 0 {
                let side = p.u8(g, Player::ACTION_COUNT) ^ 1;
                p.set_u8(g, Player::ACTION_COUNT, side);
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
pub(super) fn bolt(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => common::start_cast(g, p, 0x28, Some(0x153), 0x26),
        2 => {
            animation::step(g, p);
            if take_event(g, p) {
                crate::actors::families::creators::for_players::bolt(&mut g.leftovers, g.b, p.obj(), 0);
                crate::world::placement::load_effect_palette(g.b, crate::world::placement::effect_palette::LIGHT);
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
                crate::actors::families::creators::for_players::bolt(&mut g.leftovers, g.b, p.obj(), 1);
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
pub(super) const FROST_DURATIONS: u32 = 0x436c8;

/// 0x4362a, no. 0x1a (h): freeze. During the final phase the player sets the intensity with up and down
/// (gesture 0x2f + intensity); the screen shakes.
pub(super) fn frost(g: &mut Ctx, p: Player) {
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
                p.set_u8(g, Player::ACTION_COUNT, 0x10);
                p.set_u8(g, Player::SPELL_LEVEL, 2);
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
            p.set_u8(g, Player::SPELL_PHASE, 0);
            p.set_u8(g, Player::SPELL_BLINKS, 4);
            p.set_u8(g, Player::ACTION_COUNT, 0x10);
            p.set_u8(g, Player::SPELL_LEVEL, 2);
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
pub(super) fn frost_steer(g: &mut Ctx, p: Player) {
    cov!();
    common::blink_screen(g, p);
    if countdown(g, p) {
        next_step(g, p);
        p.set_u16(g, field::TIMER, 1);
        p.set_u8(g, Player::SPELL_PHASE, 0);
    }
    common::flash_screen(g, p);
    common::intensity_keys(g, p, 0x2f);
}

/// Words (code): duration of spell 0x1b per level.
pub(super) const QUAKE_DURATIONS: u32 = 0x4395a;

/// Earthquake (0x43892, no. 0x1b): hurts the monsters at the right moment.
pub(super) const QUAKE_STRIKE: Strike = Strike {
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
pub(super) fn quake(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            common::begin_cast(g, p, Some(0x3c), Some(0x154));
            common::dim_screen(g, p);
            { crate::services::palette::flash(g.b, crate::services::palette::Flash::A); }
            animation::start_plain(g, p, 0x2b);
        }
        2 => {
            animation::step(g, p);
            if take_event(g, p) {
                crate::actors::families::creators::for_players::quake(&mut g.leftovers, g.b, p.obj(), true);
                crate::world::placement::load_effect_palette(g.b, crate::world::placement::effect_palette::NORMAL);
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
                crate::actors::families::creators::for_players::quake(&mut g.leftovers, g.b, p.obj(), false);
                crate::actors::families::small::quake_b(g.b, p.obj());
                g.sound(rom::QUAKE_SECOND_SOUND, p);
            }
        }
        4 => quake_rumble(g, p),
        _ => {}
    }
}

/// Step 4 of the earthquake (0x4398c): at countdown turn 0x64e, the monsters are hurt.
pub(super) fn quake_rumble(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    if countdown(g, p) {
        crate::services::palette::restore_flash(g.b);
        common::end_spell(g, p);
        return;
    }
    if !is_cancelled(g, p) && p.u16(g, field::TIMER) == 0x64 {
        foes::strike_foes(g, p, &QUAKE_STRIKE);
    }
}

/// Moon rain (0x44314, no. 0x1c, facing left): the monsters on the ground are hurt.
pub(super) const MOON_STRIKE: Strike = Strike {
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
pub(super) fn moonlight(g: &mut Ctx, p: Player) {
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
                    crate::world::placement::load_effect_palette(g.b, crate::world::placement::effect_palette::NORMAL);
                    g.sound(rom::MOON_SOUND_B, p);
                }
                { crate::services::palette::flash(g.b, crate::services::palette::Flash::D); };
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
                    crate::world::placement::load_effect_palette(g.b, crate::world::placement::effect_palette::NORMAL);
                }
                { crate::services::palette::flash(g.b, crate::services::palette::Flash::C); };
            }
        }
        0xc => {
            animation::step(g, p);
            if countdown(g, p) {
                crate::services::palette::restore_flash(g.b);
                common::end_spell(g, p);
            } else if !is_cancelled(g, p) && p.u16(g, field::TIMER) == 0x60 {
                foes::strike_foes(g, p, &MOON_STRIKE);
            }
        }
        _ => {}
    }
}

/// Step 6 of the moon spell (0x4410e): at the end of the countdown the screen flash goes out.
pub(super) fn moon_finish(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    if countdown(g, p) {
        crate::services::palette::restore_flash(g.b);
        common::end_spell(g, p);
    }
}
