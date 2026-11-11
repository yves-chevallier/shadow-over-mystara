//! Prayers of the cleric, continued: beam, banish, holy light, protection, blessing, heal.

use crate::world::objects::Enemy;
use crate::world::objects::player::Player;
use crate::world::objects::Obj;
use super::*;
use crate::actors::player::tables::{pc8, pc16, byte};

/// 0x4187e, no. 0x27: holy ray. Gesture 0x30 prays; at the gesture a healing effect is born, then the ray
/// (0x4b5ac, D0 = 1) aims at a monster, stored in `SPELL_TARGET`, which receives the flag `+0x5e` at the end.
pub(super) fn prayer_beam(g: &mut Ctx, p: Player) {
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
                crate::world::placement::load_effect_palette(g.b, crate::world::placement::effect_palette::NORMAL);
                g.sound(ext::sound::PRAYER_QUAKE, p);
            }
            if !countdown(g, p) {
                return;
            }
            next_step(g, p);
            if p.u8(g, field::SPELL_CANCELLED) == 0 {
                // Original glitch: the target is the A1 left by the creator, also when the pool is full
                let target = spawn::aimed_beam(g, p);
                p.set_u16(g, field::SPELL_TARGET, target.word());
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
pub(super) fn prayer_banish(g: &mut Ctx, p: Player) {
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
                    crate::world::placement::load_effect_palette(g.b, crate::world::placement::effect_palette::LIGHT);
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
pub(super) fn banish_foes(g: &mut Ctx, p: Player) -> u8 {
    cov!();
    let mut found = 0u8;
    let level = p.u8(g, Player::LEVEL) as u32;
    let mut left = pc8(g.b, counts::BANISH + level);
    for pool in [&BOSS, &ENEMY] {
        for o in pool.iter() {
            if o.u8(g.b, Obj::ACTIVE) == 0
                || o.u8(g.b, Obj::FLAGS) & 1 == 0
                || o.u16(g.b, Enemy::HP) == 0
                || o.u8(g.b, Enemy::BOXES_OFF) != 0
                || o.u32(g.b, Enemy::IMMUNITY) >> 19 & 1 != 0
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
pub(super) fn prayer_holy_light(g: &mut Ctx, p: Player) {
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
                crate::world::placement::load_effect_palette(g.b, crate::world::placement::effect_palette::LIGHT);
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
pub(super) mod damage {
    pub const BOSS: u32 = 0x10_433c;
    pub const ENEMY: u32 = 0x10_431e;
}

/// 0x41c30: the holy light hurts all unprotected monsters in play on the ground (bit 20 of `+0x94`):
/// they go to reaction 0x10 (assigned to the cleric), lose the hit points from the table (+25% with
/// equipment `+0x166` = 4), non-boss monsters of type 0 or 0x13 die on the spot. The life bar
/// is updated.
pub(super) fn holy_damage(g: &mut Ctx, p: Player) {
    cov!();
    let level = p.u8(g, Player::LEVEL) as u32;
    let (index, character) = (p.u8(g, Player::INDEX), p.u8(g, Player::CHARACTER));
    for (pool, table) in [(&BOSS, damage::BOSS), (&ENEMY, damage::ENEMY)] {
        for o in pool.iter() {
            if o.u8(g.b, Obj::ACTIVE) == 0
                || o.u8(g.b, Obj::FLAGS) & 1 == 0
                || o.u16(g.b, Enemy::HP) == 0
                || o.u8(g.b, Enemy::BOXES_OFF) != 0
                || o.u16(g.b, Obj::Z) != 0
                || o.u32(g.b, Enemy::IMMUNITY) >> 20 & 1 != 0
            {
                continue;
            }
            o.set_u32(g.b, Obj::STATE, 0x0400_0000);
            o.set_u8(g.b, Obj::FLAGS, 1);
            o.set_u8(g.b, Enemy::HIT_BY_KIND, index);
            o.set_u8(g.b, Enemy::REACTION, 0x10);
            o.set_u8(g.b, Enemy::HIT_BY_VARIANT, character);
            o.set_u16(g.b, Enemy::HOLDER, p.0 as u16);
            o.set_u8(g.b, Enemy::FLIP, 0);
            if g.random() & 1 != 0 {
                o.set_u8(g.b, Enemy::FLIP, 0x20);
            }
            let facing = o.u8(g.b, Enemy::FLIP);
            o.set_u8(g.b, Enemy::ATTACKER_FLIP, facing);
            let counter = p.u16(g, Player::SPELL_KILLS).wrapping_add(2);
            p.set_u16(g, Player::SPELL_KILLS, counter);
            let hits = o.u8(g.b, Enemy::HITS_TAKEN).wrapping_add(1);
            o.set_u8(g.b, Enemy::HITS_TAKEN, hits);
            let mut damage = byte(g.b, table + level) as u16;
            if p.u8(g, Player::ARMOR) == 4 {
                damage = damage.wrapping_add(damage >> 2);
            }
            let hp = (o.u16(g.b, Enemy::HP) as i16).wrapping_sub(damage as i16);
            o.set_u16(g.b, Enemy::HP, if hp < 0 { 0 } else { hp as u16 });
            if o.u8(g.b, Obj::CLASS) != 4 && matches!(o.u8(g.b, Obj::KIND), 0 | 0x13) {
                o.set_u16(g.b, Enemy::HP, 0);
            }
            if o.u16(g.b, Enemy::HP) == 0 && o.u8(g.b, Enemy::DEATH_CREDIT) != 0 {
                p.set_u8(g, Player::HOLY, 1);
                let loot = o.u8(g.b, Enemy::DEATH_VALUE);
                ram::KILL_FLAG.set(g.b, loot);
            }
            let bar = (index as u32) << 8 | o.u8(g.b, Obj::KIND) as u32;
            let (max, hp, class) = (o.u16(g.b, Enemy::HP_MAX) as u32, o.u16(g.b, Enemy::HP) as u32, o.u8(g.b, Obj::CLASS) as u32);
            crate::world::events::create_popup(g.b, crate::world::events::Popup { header: bar as u16, max: max as u16, value: hp as u16, raw: class as u8 == 2 });
        }
    }
}

/// 0x41d48, no. 0x2a: protection. A protection aura (duration per level) is placed on all
/// living players at the end of the spell.
pub(super) fn prayer_protection(g: &mut Ctx, p: Player) {
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
                crate::world::placement::load_effect_palette(g.b, crate::world::placement::effect_palette::LIGHT);
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
pub(super) fn protect_players(g: &mut Ctx, p: Player) {
    cov!();
    common::clear_spell_state(g);
    p.set_u8(g, field::SPELL_CANCELLED, 0);
    let level = p.u8(g, Player::LEVEL) as u32;
    let duration = pc16(g.b, durations::PROTECTION + 2 * level);
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
pub(super) fn prayer_blessing(g: &mut Ctx, p: Player) {
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
                crate::world::placement::load_effect_palette(g.b, crate::world::placement::effect_palette::LIGHT);
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
pub(super) fn bless_ally(g: &mut Ctx, p: Player) {
    cov!();
    let level = p.u8(g, Player::LEVEL) as u32;
    let duration = pc16(g.b, durations::BLESSING + 2 * level);
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
pub(super) fn end_blessing(g: &mut Ctx, p: Player) {
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
pub(super) fn prayer_heal(g: &mut Ctx, p: Player, variant: u32) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            begin_prayer(g, p, Some(0x60), 0x154 + variant);
            p.set_u8(g, Player::SPELL_REPEAT, 0);
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
                crate::world::placement::load_effect_palette(g.b, crate::world::placement::effect_palette::NORMAL);
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
