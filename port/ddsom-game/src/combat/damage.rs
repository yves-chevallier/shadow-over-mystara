//! Damage formulas (subsystem X2): player hits, player shots, enemy attacks,
//! and display of the number of hit points removed.
//!
//! The scales are ROM tables (read at run time); only the formulas are here.

use crate::objects::Obj;
use crate::player::Player;
use crate::objects::Enemy;
use super::*;
use crate::objects::pools;
use crate::machine::Machine;
use crate::player::tables::{player_data, pc16, byte, Aligned, Read};

/// Scale tables of the program ROM. The tables read by PC-relative addressing in the original
/// are in the program space (`pc8`/`pc16`), the others as ordinary reads.
mod table {
    /// 0x3d8e (data): per weapon (4 bytes): damage bonus according to the number of players
    /// (2, 3, 4: signed bytes) then cap.
    pub const PARTY_BONUS: u32 = 0x3d8e;
    /// 0x3fea (PC): pointers to the enemy attack scales by game setting (`A5+0x70`).
    pub const ENEMY_SCALE: u32 = 0x3fea;
    /// 0x40aa and 0x416a (data): at low HP, by (class, HP): 1/32 slices removed.
    pub const LOW_HP_SHIELD: [u32; 2] = [0x40aa, 0x416a];
    /// 0x39d8 (data): possible loot (8 bytes per type: 2 entries of 4 bytes).
    pub const LOOT: u32 = 0x39d8;
}

/// 0x3bbe: adds to `base` a variation drawn from the `ROLL` table (row according to the value).
pub(super) fn roll_variation(w: &mut World, base: u16) -> u16 {
    cov::hit("roll_variation");
    let line = base.wrapping_sub(1).min(4) as u8;
    let line = (line << 5) as u16;
    let d0 = w.random() & 0x1f;
    let delta = player_data().damage_roll.at8(w.b, (d0 + line) as u32) as i8 as i16 as u16;
    base.wrapping_add(delta)
}

/// 0x422a: damage of a player shot. Power A against an ordinary enemy, B otherwise.
pub(super) fn shot_damage(w: &mut World, shot: Obj, e: Obj) -> u16 {
    cov::hit("shot_damage");
    let mut base = shot.u8(w.b, f::POWER_A);
    if e.u8(w.b, f::CLASS) != 2 {
        base = shot.u8(w.b, f::POWER_B);
    }
    let d = roll_variation(w, base as u16);
    if (d as i16) < 0 {
        1
    } else {
        d
    }
}

/// 0x3b36: damage of a player `p` hit on enemy `e`.
///
/// Base: weapon power (`+0x109` or `+0x10a`, according to `ATTACK_KIND` 0 or 1; 1 for
/// type 2), +50% under bonus (`+0x79`), random variation, then modifiers: position
/// bonus (`+0x100`), critical hit, enemy weakness/resistance, group bonus. The result
/// is also displayed (`show_damage`).
pub(super) fn player_damage(w: &mut World, p: Obj, e: Obj) -> u16 {
    cov::hit("player_damage");
    let kind = p.u8(w.b, f::ATTACK_KIND);
    let mut d5: u16 = match kind {
        0 => p.u8(w.b, Player::POWER_A) as u16,
        1 => p.u8(w.b, Player::POWER_B) as u16,
        2 => 0,
        _ => return show_damage(w, p, 0, DamageDisplay::Dealt),
    };
    if kind != 2 {
        if p.u8(w.b, Player::POWERUP) != 0 {
            let half = (d5 as u8) >> 1;
            d5 = (d5 & 0xff00) | (d5 as u8).wrapping_add(half) as u16;
        }
        if d5 as u8 == 0 {
            d5 = 1;
        }
    } else {
        d5 = 1;
    }
    d5 = roll_variation(w, d5);
    d5 = d5.wrapping_add(p.u8(w.b, Player::STATS) as u16);
    if (d5 as i16) < 0 {
        d5 = 1;
    }
    if kind >= 2 {
        return d5;
    }
    let step = p.u8(w.b, f::SUBACTION);
    if step == 0xc || step == 0x16 {
        d5 = d5.wrapping_add(d5 >> 1);
    } else {
        p.set_u8(w.b, Player::MARK, 0);
        if p.u8(w.b, f::VARIANT) >> 1 == 4 {
            if let Some(d0) = mage_critical(w, p, e) {
                return d0;
            }
        } else {
            d5 = critical_hit(w, p, d5);
        }
    }
    d5 = elemental(w, p, e, d5);
    d5 = party_bonus(w, p, d5);
    show_damage(w, p, d5, DamageDisplay::Dealt)
}

/// 0x3cb6..: critical hit of the classes other than the magician: +5 damage, flag `+0x77`.
fn critical_hit(w: &mut World, p: Obj, d5: u16) -> u16 {
    cov::hit("critical_hit");
    if (p.u8(w.b, Player::BUBBLE_ACTION) as i8) > 2 {
        return d5;
    }
    let mut row: u8 = if (p.u16(w.b, f::HP) as i16) > 16 { 0 } else { 16 };
    row = row.wrapping_add((w.random() & 15) as u8);
    let row = player_data().damage_crit_row.at8(w.b, row as u32);
    if row == 0 {
        return d5;
    }
    let mask = player_data().damage_crit_mask.word_at(w.b, row as u32);
    let bit = w.random() & 15;
    if mask >> bit & 1 == 0 {
        return d5;
    }
    p.set_u8(w.b, Player::MARK, 1);
    d5.wrapping_add(5)
}

/// 0x3e8e: the magician (class 4) equipped with weapon 0x30 makes a critical hit of the "fraction
/// of the victim's max HP" kind (max HP >> n), otherwise 1 damage point; with another weapon the
/// common computation continues (`None`).
fn mage_critical(w: &mut World, p: Obj, e: Obj) -> Option<u16> {
    cov::hit("mage_critical");
    if p.u8(w.b, Player::BLOW_ID) != 0x30 {
        return None;
    }
    let mut row: u8 = if (p.u16(w.b, f::HP) as i16) > 16 { 0 } else { 16 };
    if e.u8(w.b, f::CLASS) == 4 {
        row = row.wrapping_add(0x20);
    }
    row = row.wrapping_add((w.random() & 15) as u8);
    let first = player_data().damage_mage_row.at8(w.b, row as u32) as i8;
    if first < 0 {
        return Some(show_damage(w, p, 1, DamageDisplay::Dealt));
    }
    let idx = (first as u16).wrapping_add(w.random() & 15);
    let shift = player_data().damage_mage_shift.at8(w.b, idx as u32) as i8;
    if shift < 0 {
        return Some(show_damage(w, p, 1, DamageDisplay::Dealt));
    }
    p.set_u8(w.b, Player::MARK, 1);
    p.set_u8(w.b, Player::MOVE_KIND, 3);
    let d0 = e.u16(w.b, f::HP_MAX).checked_shr(shift as u32 & 63).unwrap_or(0);
    Some(show_damage(w, p, d0, DamageDisplay::Dealt))
}

/// 0x3d10: elemental weakness/resistance of the victim (`+0x9e`, `+0x9f`) to the player's weapon
/// (`+0x107`): +4 if weak, half if resistant.
fn elemental(w: &mut World, p: Obj, e: Obj, d5: u16) -> u16 {
    cov::hit("elemental");
    let element = p.u8(w.b, Player::ELEMENT);
    if element == 0 {
        return d5;
    }
    let weak = e.u8(w.b, f::WEAKNESS);
    if weak == 0 {
        return d5;
    }
    if element == weak {
        return d5.wrapping_add(4);
    }
    if element == e.u8(w.b, f::WEAK_ELEMENT) {
        return d5 >> 1;
    }
    d5
}

/// 0x3d3a: the group of living players adjusts the damage: bonus (signed byte) from the weapon's
/// table, floor from the same record.
fn party_bonus(w: &mut World, p: Obj, d5: u16) -> u16 {
    cov::hit("party_bonus");
    let alive = ram::PLAYERS_ALIVE.get(w.b);
    if (alive as i8) <= 1 {
        return d5;
    }
    let mut weapon = p.u8(w.b, f::WEAPON_10D);
    if p.u8(w.b, f::ATTACK_KIND) != 0 {
        weapon = p.u8(w.b, f::WEAPON_10E);
    }
    let row = table::PARTY_BONUS + (weapon << 2) as u32;
    let bonus = byte(w.b, row + alive.wrapping_sub(2) as u32) as i8 as i16 as u16;
    let floor = byte(w.b, row + 3) as u16;
    let d5 = d5.wrapping_add(bonus);
    if (d5 as i16) > floor as i16 {
        d5
    } else {
        byte(w.b, row + 3) as i8 as i16 as u16
    }
}

/// Who takes the displayed damage? (the two numbers are drawn at different places of the
/// character layer)
#[derive(Clone, Copy)]
pub(super) enum DamageDisplay {
    /// Damage dealt by the player (0x3f28: column 0x9121e8).
    Dealt,
    /// Damage received by the player (0x3f32: column 0x9121ec).
    Taken,
}

/// 0x3f28 / 0x3f32: draws the damage number (two digits, low byte) next to player
/// `p`, unless the screen setting hides it (bit 0x200 of `FFFFD0`). Returns `dmg`.
pub(super) fn show_damage(w: &mut World, p: Obj, dmg: u16, which: DamageDisplay) -> u16 {
    cov::hit("show_damage");
    let base = match which {
        DamageDisplay::Dealt => 0x9121e8,
        DamageDisplay::Taken => 0x9121ec,
    };
    if ram::DSW.get(w.b) & 0x200 != 0 {
        return dmg;
    }
    let idx = p.u8(w.b, f::KIND) as i8 as i16 as u16;
    let mut at = base + (((idx << 8).wrapping_add(idx << 7)) as u32);
    // conversion to two BCD digits by subtractions of 10 (low byte only)
    let mut units = dmg & 0xff;
    let mut tens: u16 = 0;
    loop {
        units = units.wrapping_sub(10);
        if (units as i16) < 0 {
            break;
        }
        tens = tens.wrapping_add(16);
    }
    let bcd = units.wrapping_add(10).wrapping_add(tens);
    let hi = ((bcd as u8) >> 4) as u16 & 0xf;
    wr16(w.b, at, hi);
    wr16(w.b, at + 2, 0);
    at += 0x80;
    wr16(w.b, at, bcd & 0xf);
    wr16(w.b, at + 2, 0);
    dmg
}

/// 0x4002: damage of an enemy attack (`atk`) on player `p`.
///
/// Base drawn from a 32-entry scale (indexed by the counter FFFF92) according to the game setting,
/// plus the attack power (`+0x45`), random variation, minus the player's defence
/// (`+0x10b`); then reduction in 1/32 slices according to protection (`+0x108`) or, at 32 HP and
/// below, according to the low-HP table. Displays the result.
pub(super) fn damage_vs_player(w: &mut World, p: Obj, atk: Obj) -> u16 {
    cov::hit("damage_vs_player");
    let setting = ram::DAMAGE_SETTING.get(w.b);
    let table = {
        let at = table::ENEMY_SCALE + 2 * setting as u32;
        pc16(w.b, at) as u32 * 0x10000 + pc16(w.b, at + 2) as u32
    };
    let tick = ram::TICK.get(w.b);
    let mut d5 = byte(w.b, table + tick as u32) as i8 as i16 as u16;
    d5 = d5.wrapping_add(atk.u8(w.b, f::POWER_A) as u16);
    d5 = if (d5 as i16) > 0 { roll_variation(w, d5) } else { 1 };
    let defense = p.u8(w.b, Player::DEFENSE) as u16;
    let (diff, borrow) = d5.overflowing_sub(defense);
    d5 = if !borrow && diff != 0 { diff } else { 1 };
    // reduction by 1/32 slices (n slices => d5 -= d5 * n / 32)
    let slices = if (p.u16(w.b, f::HP) as i16) > 0x20 {
        let prot = p.u8(w.b, Player::PROTECTION);
        if (prot as i8) >= 9 {
            None
        } else {
            Some(player_data().damage_protection.at8(w.b, prot as u32))
        }
    } else {
        let class = (p.u8(w.b, f::VARIANT) >> 1) as u16;
        let idx = (class << 5).wrapping_sub(1).wrapping_add(p.u16(w.b, f::HP));
        let t = table::LOW_HP_SHIELD[(ram::DAMAGE_SETTING.get(w.b) == 6) as usize];
        let n = byte(w.b, t.wrapping_add(idx as i16 as i32 as u32));
        if n == 0 {
            None
        } else {
            Some(n)
        }
    };
    if let Some(n) = slices {
        let mut sum: u16 = 0;
        for _ in 0..=(n as u16) {
            sum = sum.wrapping_add(d5);
        }
        let cut = sum >> 5;
        let left = d5.wrapping_sub(cut);
        d5 = if (left as i16) > 0 { left } else { d5 };
    }
    show_damage(w, p, d5, DamageDisplay::Taken)
}

/// 0x388a: a thief's hit (`ATTACK_KIND` 3) can make enemy `e` drop loot.
pub(super) fn steal_loot(w: &mut World, p: Obj, e: Obj) {
    cov::hit("steal_loot");
    if p.u8(w.b, f::VARIANT) >> 1 != 5 {
        return;
    }
    let loot = e.u8(w.b, Enemy::LOOT) as u32;
    if loot == 0 {
        return;
    }
    let entry = table::LOOT + (loot << 3) + ((w.random() & 1) as u32) * 4;
    let weight = byte(w.b, entry + 2).wrapping_add(byte(w.b, entry + 3));
    let level = p.u8(w.b, Player::LEVEL) as u32;
    let mut chance = player_data().damage_loot_level.at8(w.b, level).wrapping_sub(weight);
    if (chance as i8) < 0 {
        chance = 0;
    }
    let mask = (player_data().damage_loot_mask.word_at(w.b, 4 * chance as u32) as u32) << 16 | player_data().damage_loot_mask.word_at(w.b, 4 * chance as u32 + 2) as u32;
    let bit = w.random() & 0x1f;
    if mask >> bit & 1 == 0 {
        return;
    }
    // object to create (ITEM) in place of the enemy; failure if the pool is full (carry)
    w.a2 = e.0;
    let Some(item) = pools::ITEM.alloc(w.b) else { return };
    item.set_u8(w.b, f::ACTIVE, 1);
    let (kind, var) = (byte(w.b, entry), byte(w.b, entry + 1));
    item.set_u8(w.b, f::KIND, kind);
    item.set_u8(w.b, f::VARIANT, var);
    let (x, z, y) = (e.u16(w.b, f::X), e.u16(w.b, f::Z), e.u16(w.b, f::Y));
    item.set_u16(w.b, f::X, x);
    item.set_u16(w.b, f::Z, z.wrapping_add(16));
    item.set_u16(w.b, f::Y, y.wrapping_sub(1));
    item.set_u16(w.b, Obj::VZ, 0x300);
    item.set_u16(w.b, Obj::AZ, 0xffd8);
    e.set_u8(w.b, Enemy::LOOT, 0);
}
