//! Special attack of the cleric: equipment swap, turning away the undead, prayer skeleton, hold, storm, swarm.

use crate::player::Player;
use crate::objects::Obj;
use super::*;
use crate::player::tables::{players, Read};

// ---- special attack (0x406d4) --------------------------------------------------------------

/// The cleric's animations for cast spells (see `items::Style`).
pub(super) const STYLE: items::Style = items::Style { cast_anim: 0x20, bolt_anim: 0x1f, spark: 1, ..items::Style::CLASSIC };

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
pub(super) fn swap_equipment(g: &mut Ctx, p: Player) {
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
pub(super) fn scare_undead(g: &mut Ctx, p: Player) {
    cov!();
    let level = p.u8(g, Player::LEVEL);
    let talisman = p.u8(g, Player::TALISMAN) == 3;
    for k in 0..23 {
        let o = Obj(A5 + 0x299e + 0xc0 * k);
        let in_play = o.u8(g.b, Obj::ACTIVE) != 0 && o.u8(g.b, Obj::FLAGS) & 1 != 0;
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
pub(super) fn begin_prayer(g: &mut Ctx, p: Player, timer: Option<u16>, reserve: u32) {
    cov!();
    common::begin_spell(g, p, timer);
    let left = p.u8(g, reserve).wrapping_sub(1);
    p.set_u8(g, reserve, left);
    common::face_held_direction(g, p);
    common::freeze_others(g, p);
}

/// One frame of countdown: `field::TIMER` decreases; true when it reaches zero.
pub(super) fn countdown(g: &mut Ctx, p: Player) -> bool {
    let left = p.u16(g, field::TIMER).wrapping_sub(1);
    p.set_u16(g, field::TIMER, left);
    left == 0
}

/// Prayer cancelled (a protection, `SPELL_CANCELLED`): the countdown goes to 16 frames and the failure
/// spark of kind `spark` is played.
pub(super) fn cancel_prayer(g: &mut Ctx, p: Player, spark: u16) {
    cov!();
    p.set_u16(g, field::TIMER, 0x10);
    common::item_spark(g, p, spark);
}

/// Table (code bytes): maximum number of monsters hit per level, per prayer.
pub(super) mod counts {
    pub const BANISH: u32 = 0x41afe;
}

/// Table (code words, 0x410a6): duration of the blessing bonus per level, and (0x41e4c) duration of
/// the protection aura.
pub(super) mod durations {
    pub const BLESSING: u32 = 0x410a6;
    pub const PROTECTION: u32 = 0x41e4c;
}

/// 0x4119c, no. 0x24: holy aura that immobilizes monsters. Animation 0x2a prepares, 0x2b prays; at the
/// gesture the aura is created, then the first monsters designated by `hold_foes` are frozen.
pub(super) fn prayer_hold(g: &mut Ctx, p: Player) {
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
pub(super) fn hold_foes(g: &mut Ctx, p: Player) {
    cov!();
    let level = p.u8(g, Player::LEVEL) as u32;
    let mut left = players("hold_counts").at8(g.b, level);
    let (index, character) = (p.u8(g, Player::INDEX), p.u8(g, Player::CHARACTER));
    for pool in [&BOSS, &ENEMY] {
        for o in pool.iter() {
            if o.u8(g.b, Obj::ACTIVE) == 0
                || o.u8(g.b, Obj::FLAGS) & 1 == 0
                || o.u16(g.b, 0x62) == 0
                || o.u8(g.b, 0x40) != 0
                || o.u8(g.b, 0x49) == 0x9c
                || o.u32(g.b, 0x94) >> 15 & 1 != 0
            {
                continue;
            }
            o.set_u32(g.b, Obj::STATE, 0x0400_0000);
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
pub(super) fn prayer_storm(g: &mut Ctx, p: Player) {
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
pub(super) fn prayer_swarm(g: &mut Ctx, p: Player) {
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
                    let count = players("cure_counts").at8(g.b, level).wrapping_sub(1);
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
pub(super) fn prayer_finish_timer(g: &mut Ctx, p: Player) {
    cov!();
    if countdown(g, p) {
        common::end_spell(g, p);
    }
}
