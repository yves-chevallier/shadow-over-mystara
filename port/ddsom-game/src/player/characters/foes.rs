//! Spells that hurt monsters (elf, magic-user): the original program copies one loop per
//! spell; they differ only by a few parameters, described here by `Strike`.
//!
//! The loop goes through the bosses then the enemies. A monster in play, alive and without protection against this
//! spell (a bit of `+0x94`) goes to the reaction `reaction` assigned to the caster, loses hit
//! points (per-level table or fixed value, increased according to equipment) and its life bar is updated.

use crate::machine::Machine;
use super::super::{Ctx, Player};
use crate::objects::pools::{BOSS, ENEMY};
use crate::objects::Obj;
use crate::player::ram;

/// Base damage.
pub enum Damage {
    /// ROM bytes indexed by the caster's level (bosses, enemies).
    Table(u32, u32),
    /// Single value.
    Fixed(u16),
}

/// Test of the monster's byte `+1` (game state).
pub enum Live {
    /// Non-zero.
    Any,
    /// Bit 0.
    Bit0,
    /// No test.
    Unchecked,
}

/// Damage increases (`d += d >> n`, or +5), in order of application.
pub enum Boost {
    /// The monster is a boss (class 4) of type `kind`: +50%.
    Boss(u8),
    /// The caster's `WEAPON_B` equipment is `weapon`: `d >> shift` extra.
    Held(u8, u32),
    /// The caster's accessory `+0x166` is 4: +25%.
    Armor,
    /// The caster's accessory `+0x165` is `v`: +5.
    Charm(u8),
}

/// A spell that hurts monsters.
pub struct Strike {
    pub damage: Damage,
    pub live: Live,
    /// The monster must have hit points.
    pub needs_hp: bool,
    /// The monster must be on the ground (`Z` = 0).
    pub grounded: bool,
    /// Bit of `+0x94` that protects the monster.
    pub immune_bit: u32,
    /// Reaction (`+0x49`) imposed on the monster.
    pub reaction: u8,
    /// The monster is turned around at random (`+0x4a`).
    pub random_facing: bool,
    pub boosts: &'static [Boost],
    /// Monsters of type 0 and 0x13 (non-boss) die on the spot.
    pub undead_die: bool,
    /// Sound played after each life bar.
    pub sound: Option<crate::sound::Sound>,
}

/// Applies `strike` to all eligible monsters. `boss_table`/`enemy_table` replace the tables of
/// `Damage::Table` when the spell chooses its table at run time.
pub fn strike_foes(g: &mut Ctx, p: Player, s: &Strike) {
    cov!();
    let level = p.u8(g, Player::LEVEL) as u32;
    let (index, character) = (p.u8(g, Player::INDEX), p.u8(g, Player::CHARACTER));
    for (k, pool) in [&BOSS, &ENEMY].into_iter().enumerate() {
        for o in pool.iter() {
            let live = match s.live {
                Live::Any => o.u8(g.b, 1) != 0,
                Live::Bit0 => o.u8(g.b, 1) & 1 != 0,
                Live::Unchecked => true,
            };
            if o.u8(g.b, 0) == 0
                || !live
                || (s.needs_hp && o.u16(g.b, 0x62) == 0)
                || o.u8(g.b, 0x40) != 0
                || (s.grounded && o.u16(g.b, 0x0c) != 0)
                || o.u32(g.b, 0x94) >> s.immune_bit & 1 != 0
            {
                continue;
            }
            let counter = p.u16(g, 0x190).wrapping_add(2);
            p.set_u16(g, 0x190, counter);
            let hits = o.u8(g.b, 0x9c).wrapping_add(1);
            o.set_u8(g.b, 0x9c, hits);
            o.set_u32(g.b, 4, 0x0400_0000);
            o.set_u8(g.b, 1, 1);
            o.set_u8(g.b, 0x49, s.reaction);
            o.set_u8(g.b, 0x48, index);
            o.set_u8(g.b, 0x4b, character);
            o.set_u16(g.b, 0x54, p.0 as u16);
            if s.random_facing {
                o.set_u8(g.b, 0x4a, 0);
                if g.random() & 1 != 0 {
                    o.set_u8(g.b, 0x4a, 0x20);
                }
            }
            let mut d = match s.damage {
                Damage::Table(boss, enemy) => g.data8(if k == 0 { boss } else { enemy } + level) as u16,
                Damage::Fixed(v) => v,
            };
            for boost in s.boosts {
                let apply = match *boost {
                    Boost::Boss(kind) => {
                        if o.u8(g.b, 0x28) == 4 && o.u8(g.b, Obj::KIND) == kind {
                            d += d >> 1;
                        }
                        continue;
                    }
                    Boost::Held(weapon, shift) => {
                        if p.u8(g, Player::WEAPON_B) == weapon {
                            d += d >> shift;
                        }
                        continue;
                    }
                    Boost::Armor => p.u8(g, 0x166) == 4,
                    Boost::Charm(v) => {
                        if p.u8(g, 0x165) == v {
                            d += 5;
                        }
                        continue;
                    }
                };
                if apply {
                    d += d >> 2;
                }
            }
            let hp = (o.u16(g.b, 0x62) as i16).wrapping_sub(d as i16);
            o.set_u16(g.b, 0x62, if hp < 0 { 0 } else { hp as u16 });
            if s.undead_die && o.u8(g.b, 0x28) != 4 && matches!(o.u8(g.b, Obj::KIND), 0 | 0x13) {
                o.set_u16(g.b, 0x62, 0);
            }
            if o.u16(g.b, 0x62) == 0 && o.u8(g.b, 0x93) != 0 {
                p.set_u8(g, 0x18a, 1);
                let loot = o.u8(g.b, 0x9a);
                ram::KILL_FLAG.set(g.b, loot);
            }
            let bar = (index as u32) << 8 | o.u8(g.b, Obj::KIND) as u32;
            let (max, hp, class) = (o.u16(g.b, 0x60) as u32, o.u16(g.b, 0x62) as u32, o.u8(g.b, 0x28) as u32);
            crate::events::create_popup(g.b, crate::events::Popup { header: bar as u16, max: max as u16, value: hp as u16, raw: class as u8 == 2 });
            if let Some(sound) = s.sound {
                g.sound(sound, p);
            }
        }
    }
}
