//! Spells of the magic-user, continued: storm, rain, witch, curse, wave, portal, summon, wall, drain, sacrifice.

use crate::objects::Enemy;
use crate::families::player::records::Victim;
use crate::player::Player;
use crate::objects::Obj;
use super::*;
use crate::player::tables::{pc16};

/// 0x4722a, no. 0x1c (h): storm. Two variants depending on facing: to the right (steps 0 to 6, bolt
/// then effect 0x4cd8e) or to the left (steps 8 to 0xe, effect 0x4d020).
pub(super) fn storm(g: &mut Ctx, p: Player) {
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
                g.sound(crate::sound::STORM, p);
                g.sound(if step == 2 { crate::sound::CMD_181 } else { crate::sound::CMD_180 }, p);
            }
            if countdown(g, p) {
                next_step(g, p);
                p.set_u16(g, field::TIMER, if step == 2 { 0xc0 } else { 0xb0 });
                animation::start_plain(g, p, 0x2a);
                g.sound(crate::sound::CMD_74, p);
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
pub(super) fn rain(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => common::start_cast(g, p, 0x2c, Some(0x156), 0x33),
        2 => {
            animation::step(g, p);
            if take_event(g, p) {
                spawn::rain_glow(g, p, 0);
                crate::placement::load_effect_palette(g.b, crate::placement::effect_palette::NORMAL);
                sounds(g, p, &[crate::sound::CMD_F6, crate::sound::CMD_17A_B]);
            }
            if countdown(g, p) {
                next_step(g, p);
                p.set_u16(g, field::TIMER, 0x60);
                animation::start_plain(g, p, 0x34);
                g.sound(crate::sound::CMD_79, p);
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
                g.sound(crate::sound::CMD_17C, p);
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
pub(super) fn witch(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => common::start_cast(g, p, 0x14, Some(0x154), 0x17),
        2 => {
            if countdown(g, p) {
                next_step(g, p);
                p.set_u8(g, Player::ACTION_COUNT, 0);
                p.set_u32(g, Player::RECOGNIZERS + 0x30, 0);
                p.set_u16(g, Player::RECOGNIZERS + 0x34, 0);
                p.set_u16(g, field::TIMER, 0x2c);
                animation::start_plain(g, p, 0x18);
                g.sound(crate::sound::CMD_F6, p);
                g.sound(crate::sound::CMD_17D, p);
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
                let (timer, anim) = if p.u8(g, Player::ACTION_COUNT) != 0 { (0x80, 0x1a) } else { (0x50, 0x19) };
                p.set_u16(g, field::TIMER, timer);
                animation::start_plain(g, p, anim);
                g.sound(crate::sound::CMD_7A, p);
                g.sound(crate::sound::CMD_17E, p);
                return;
            }
            if is_cancelled(g, p) {
                if p.u16(g, field::TIMER) <= 0x10 {
                    p.set_u8(g, Player::STEP, 8);
                    common::cancel_cast(g, p, 0x23);
                    common::item_spark(g, p, 0x24);
                    return linger(g, p);
                }
            } else if p.u16(g, Player::RECOGNIZERS + 0x34) != 0 {
                p.set_u8(g, Player::ACTION_COUNT, 1);
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
pub(super) const CURSE_DURATIONS: u32 = 0x47c4e;

/// 0x47b88, no. 0x1f (h): curse: at the end the player receives a countdown (`+0x196`).
pub(super) fn curse(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => common::start_cast(g, p, 0x18, Some(0x156), 0x29),
        2 => {
            animation::step(g, p);
            if countdown(g, p) {
                next_step(g, p);
                p.set_u16(g, field::TIMER, 0x30);
                animation::start_plain(g, p, 0x2a);
                g.sound(crate::sound::CMD_7B, p);
            }
        }
        4 => {
            animation::step(g, p);
            if p.u16(g, field::TIMER) == 0x18 {
                g.sound(crate::sound::CMD_196, p);
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
pub(super) fn wave(g: &mut Ctx, p: Player) {
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
                g.sound(crate::sound::CMD_76, p);
            } else if take_event(g, p) {
                spawn::wave_bolt(g, p, false);
                spawn::wave_bolt(g, p, true);
                crate::placement::load_effect_palette(g.b, crate::placement::effect_palette::LIGHT);
                g.sound(crate::sound::CMD_179, p);
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
pub(super) const PORTAL_DURATIONS: u32 = 0x47ee8;
pub(super) const PORTAL_SPEEDS: u32 = 0x47e78;
pub(super) const REGROUP_SPEEDS: u32 = 0x4851c;

/// Damage of spell 0x21 (0x48012): without state test, without turning the monsters around.
pub(super) const PORTAL_STRIKE: Strike = Strike {
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
pub(super) fn aim_at_center(g: &mut Ctx, p: Player, speeds: u32, vz: u16, az: i16) {
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
    let speed = pc16(g.b, speeds + index as u32);
    let speed = if p.u8(g, Player::FACING) != 0 { speed } else { (speed as i16).wrapping_neg() as u16 };
    p.set_u16(g, Obj::VX, speed);
    p.set_u16(g, Obj::VZ, vz);
    p.set_u16(g, Obj::AZ, az as u16);
    animation::start_plain(g, p, 0x2f);
}

/// 0x47dd6, no. 0x21 (h): portal. The magic-user leaps to the screen center (steps 0 to 2), stays there while
/// the monsters are hurt (step 4), then lands (steps 6 and 8).
pub(super) fn portal(g: &mut Ctx, p: Player) {
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
            g.sound(crate::sound::CMD_184, p);
            g.sound(crate::sound::CMD_F6, p);
        }
        4 => {
            if !is_cancelled(g, p) {
                animation::step(g, p);
                if take_event(g, p) {
                    spawn::portal(g, p, 0);
                    spawn::portal(g, p, 1);
                    { crate::palette::flash(g.b, crate::palette::Flash::E); };
                    foes::strike_foes(g, p, &PORTAL_STRIKE);
                    g.sound(crate::sound::CMD_75, p);
                    g.sound(crate::sound::CMD_178, p);
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
pub(super) fn summon(g: &mut Ctx, p: Player) {
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
                g.sound(crate::sound::CMD_185, p);
            }
            g.sound(crate::sound::CMD_77, p);
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
pub(super) const WALL_STRIKE: Strike = Strike {
    damage: Damage::Fixed(0x46),
    live: Live::Bit0,
    needs_hp: false,
    grounded: false,
    immune_bit: 0xe,
    reaction: 0x7c,
    random_facing: false,
    boosts: &[Boost::Held(0x20, 2), Boost::Held(0x21, 1), Boost::Armor],
    undead_die: true,
    sound: Some(crate::sound::CMD_186),
};

/// 0x4822a, no. 0x23 (h): fire wall: an effect at the gesture, then fixed damage at the right moment.
pub(super) fn wall(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => common::start_cast(g, p, 0x28, Some(0x159), 0x2c),
        2 => {
            animation::step(g, p);
            if take_event(g, p) {
                spawn::wall_spark(g, p, 0);
                crate::placement::load_effect_palette(g.b, crate::placement::effect_palette::LIGHT);
                g.sound(crate::sound::CMD_17A, p);
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
                g.sound(crate::sound::CMD_78, p);
                g.sound(crate::sound::CMD_17B, p);
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
pub(super) fn drain(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            common::begin_cast(g, p, Some(0x3c), None);
            p.set_u8(g, Player::SECOND_STRIKE_LANDED, 0);
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
                g.sound(crate::sound::CMD_184, p);
            }
        }
        4 => {
            animation::step(g, p);
            if take_event(g, p) {
                spawn::portal_glow(g, p);
                spawn::portal(g, p, 0);
                spawn::portal_sparks(g, p);
                g.sound(crate::sound::CMD_7C, p);
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
                let tally = p.u8(g, Player::SECOND_STRIKE_LANDED);
                if spawn::portal_burst(g, p, tally) {
                    p.set_u8(g, Player::SECOND_STRIKE_LANDED, tally.wrapping_add(1));
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
                    if p.u8(g, Player::INVENTORY + i) == 0x4f {
                        p.set_u8(g, Player::INVENTORY + i, 0x56);
                        p.set_u8(g, Player::QUANTITIES + i, 0);
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
pub(super) fn sacrifice(g: &mut Ctx, p: Player) {
    cov!();
    use crate::objects::pools::{BOSS, ENEMY};
    let (index, character) = (p.u8(g, Player::INDEX), p.u8(g, Player::CHARACTER));
    for pool in [&BOSS, &ENEMY] {
        for o in pool.iter() {
            if o.u8(g.b, Obj::ACTIVE) == 0 || o.u8(g.b, Obj::FLAGS) == 0 || o.u8(g.b, Victim::BOXES_OFF) != 0 {
                continue;
            }
            let counter = p.u16(g, Player::SPELL_KILLS).wrapping_add(2);
            p.set_u16(g, Player::SPELL_KILLS, counter);
            let hits = o.u8(g.b, Victim::HITS_TAKEN).wrapping_add(1);
            o.set_u8(g.b, Victim::HITS_TAKEN, hits);
            o.set_u32(g.b, Obj::STATE, 0x0400_0000);
            o.set_u8(g.b, Obj::FLAGS, 1);
            o.set_u8(g.b, Enemy::REACTION, 0xa8);
            o.set_u8(g.b, Victim::HIT_BY_KIND, index);
            o.set_u8(g.b, Victim::HIT_BY_VARIANT, character);
            o.set_u16(g.b, Enemy::HOLDER, p.0 as u16);
            o.set_u8(g.b, Enemy::ATTACKER_FLIP, 0);
            if g.random() & 1 != 0 {
                o.set_u8(g.b, Enemy::ATTACKER_FLIP, 0x20);
            }
            let kind = o.u8(g.b, Obj::KIND);
            if o.u8(g.b, Obj::CLASS) == 4 && matches!(kind, 0xf | 0x14) {
                let hp = (o.u16(g.b, Enemy::HP) as i16).wrapping_sub(0x1f4);
                o.set_u16(g.b, Enemy::HP, if hp < 0 { 0 } else { hp as u16 });
            } else {
                o.set_u16(g.b, Enemy::HP, 1);
            }
            if o.u16(g.b, Enemy::HP) == 0 && o.u8(g.b, Victim::DEATH_CREDIT) != 0 {
                p.set_u8(g, Player::HOLY, 1);
                let loot = o.u8(g.b, Victim::DEATH_VALUE);
                ram::KILL_FLAG.set(g.b, loot);
            }
            let bar = (index as u32) << 8 | kind as u32;
            let (max, hp, class) = (o.u16(g.b, Enemy::HP_MAX) as u32, o.u16(g.b, Enemy::HP) as u32, o.u8(g.b, Obj::CLASS) as u32);
            crate::events::create_popup(g.b, crate::events::Popup { header: bar as u16, max: max as u16, value: hp as u16, raw: class as u8 == 2 });
        }
    }
    for q in Player::all() {
        if q.u8(g, Player::ACTIVE) != 0 && q.u16(g, Player::HP) != 0 {
            q.set_u16(g, Player::HP, 1);
        }
    }
}
