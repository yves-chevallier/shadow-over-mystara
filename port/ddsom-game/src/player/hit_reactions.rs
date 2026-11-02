//! Reaction of an enemy (or a crate) hit by a player (P6).
//!
//! `enemy_reacts` (0xf232e) is called by the hit resolution (`combat`, 0x264fe) right after
//! the victim has entered the "hit" state: it chooses the victim's reaction animation
//! (byte `+0x49`) from the attacker's class, the hit identifier (`+0x47`), the victim's
//! type and a stun counter (`+0x88`) which, once exceeded, knocks the victim to the ground. Then
//! `maybe_stun` (0xf1c1a) draws at random a stagger time (`+0x87`).
//!
//! The tables are in ROM: by attacker class then by victim type, a row of
//! four bytes per hit identifier (normal reaction, airborne, stunned, addition to the counter).

use crate::machine::Machine;
use cps2::Cps2Bus;

use super::{ram, Ctx, Player};
use crate::mem::{rd16, rd8, wr8};

mod tables {
    /// By attacker class (pointers) then victim type (pointers): reaction rows.
    pub const ENEMY: u32 = 0xf2656;
    /// Same for victims that are not ordinary enemies (bosses...).
    pub const OTHER: u32 = 0xf266e;
    /// Stun thresholds: 8 bytes per victim type, one per eighth of the random counter.
    pub const THRESHOLD_ENEMY: u32 = 0xf24fe;
    /// Second table, indexed by the threshold of the first, for victims that are not enemies.
    pub const THRESHOLD_OTHER: u32 = 0xf25ae;
    /// By attacker class (address, in the code): table of stagger chances according to the hit.
    pub const STUN_CLASS: u32 = 0xf1cac;
    /// 16-bit masks: a draw of 0..15 succeeds if its bit is set.
    pub const STUN_MASKS: u32 = 0xf1d2e;
    /// Stagger durations.
    pub const STUN_TIMES: u32 = 0xf1f2e;
}

/// Fields of the victim (`v`) and the attacker (`a`).
mod f {
    pub const ACTIVE: u32 = 0x00;
    pub const KIND: u32 = 0x02;
    pub const X: u32 = 0x08;
    pub const Z: u32 = 0x0c;
    pub const CLASS: u32 = 0x28;
    pub const HIT_BY: u32 = 0x48;
    pub const REACTION: u32 = 0x49;
    pub const FACING: u32 = 0x4a;
    pub const STUN_TIMER: u32 = 0x6f;
    pub const STUN_TIME: u32 = 0x87;
    pub const STUN_COUNT: u32 = 0x88;
    pub const STUN_DECAY: u32 = 0x89;
    /// Attacker: hit type (< 2: weapon hit), hit identifier, flag, wand charge.
    pub const ATTACK_KIND: u32 = 0x3c;
    pub const BLOW_ID: u32 = 0x47;
    pub const FLAG_77: u32 = 0x77;
    pub const WAND_CHARGE: u32 = 0x191;
}

const CLASS_ENEMY: u8 = 2;
/// Magic-user hit that stuns.
const WIZARD_BLOW: u32 = 0x80;
/// Push hit: the victim turns toward the aggressor.
const PUSH_BLOW: u32 = 0x58;


/// 0xf232e: `a` is the player who strikes, `v` the victim (object address).
pub fn enemy_reacts(g: &mut Ctx, a: Player, v: u32) {
    cov!();
    let att = a.0;
    let index = rd8(g.b, att + Player::INDEX);
    wr8(g.b, v + f::HIT_BY, index);
    let blow = rd8(g.b, att + f::BLOW_ID) as u32;
    if blow == PUSH_BLOW && rd16(g.b, att + f::X) < rd16(g.b, v + f::X) {
        wr8(g.b, v + f::FACING, 0x20);
    } else if blow == PUSH_BLOW {
        wr8(g.b, v + f::FACING, 0);
    }
    // reaction rows of this victim type for this attacker class
    let is_enemy = rd8(g.b, v + f::CLASS) == CLASS_ENEMY;
    let by_class = if is_enemy { tables::ENEMY } else { tables::OTHER };
    let class_offset = (rd8(g.b, att + Player::CHARACTER) as u32 & 0xe) * 2;
    let kind = rd8(g.b, v + f::KIND) as u32;
    let per_kind = g.data32(by_class + class_offset);
    let rows = g.data32(per_kind + 4 * kind);
    // stun threshold: two table levels for victims that are not enemies
    let slot = (kind * 8 + ((ram::TICK.get(g.b) as u32 & 0x1c) >> 2)) & 0xffff;
    let mut threshold = g.data8(tables::THRESHOLD_ENEMY + slot) as u32;
    if !is_enemy {
        threshold = g.data8(tables::THRESHOLD_OTHER + ((slot & 0xff00) | threshold)) as u32;
    }
    let threshold = threshold as u8 as i8;
    let row = |g: &mut Ctx, n: u32| g.data8(rows + blow + n);

    if rd8(g.b, att + f::ATTACK_KIND) < 2 {
        let wizard = rd8(g.b, att + Player::CHARACTER) & 0xe == 4 && blow == WIZARD_BLOW;
        if !wizard {
            // weapons that stun for sure: fixed reaction and counter that goes up by 2
            let fixed = match (rd8(g.b, att + Player::WEAPON_A), rd8(g.b, att + Player::WEAPON_B)) {
                (0x10, _) => Some(0x60),
                (0x11, _) => Some(0x70),
                (0x12, _) => Some(0x74),
                (_, 0x27) => Some(0xa0),
                _ => None,
            };
            if let Some(reaction) = fixed {
                wr8(g.b, v + f::STUN_DECAY, 0x5a);
                let count = rd8(g.b, v + f::STUN_COUNT).wrapping_add(2);
                wr8(g.b, v + f::STUN_COUNT, count);
                if threshold <= count as i8 {
                    wr8(g.b, v + f::ACTIVE, 2);
                }
                wr8(g.b, v + f::REACTION, reaction);
                if reaction == 0xa0 {
                    let charge = rd8(g.b, att + f::WAND_CHARGE);
                    if charge != 0 {
                        wr8(g.b, att + f::WAND_CHARGE, charge - 1);
                        if charge - 1 == 5 {
                            crate::families::creators::for_players::set_a(&mut g.leftovers, g.b, a.obj(), 0xf);
                        }
                    }
                }
                return;
            }
        }
        wr8(g.b, v + f::STUN_DECAY, 0x5a);
        let add = row(g, 3);
        let count = rd8(g.b, v + f::STUN_COUNT).wrapping_add(add);
        wr8(g.b, v + f::STUN_COUNT, count);
        if threshold <= count as i8 {
            // threshold exceeded: the victim is knocked to the ground
            wr8(g.b, v + f::STUN_COUNT, 0);
            wr8(g.b, v + f::ACTIVE, 2);
            let reaction = row(g, 2);
            wr8(g.b, v + f::REACTION, reaction);
            return maybe_stun(g, a, v);
        }
    }
    let reaction = if rd16(g.b, v + f::Z) as i16 > 0 {
        row(g, 1)
    } else if rd8(g.b, v + f::STUN_TIME) != 0 {
        wr8(g.b, v + f::STUN_TIME, 0);
        row(g, 2)
    } else if rd8(g.b, att + f::FLAG_77) != 0 {
        row(g, 2)
    } else {
        row(g, 0)
    };
    wr8(g.b, v + f::REACTION, reaction);
    maybe_stun(g, a, v);
}

/// 0xf1c1a: the attack may stun the victim: a draw according to the attacker class, the hit and
/// a counter decides whether a stagger time is set.
fn maybe_stun(g: &mut Ctx, a: Player, v: u32) {
    cov!();
    let att = a.0;
    if rd8(g.b, att + Player::WEAPON_A) == 0x1d {
        if rd8(g.b, v + f::CLASS) == CLASS_ENEMY || g.random() & 1 != 0 {
            set_stun_time(g, v);
        }
        return;
    }
    let class_offset = (rd8(g.b, att + Player::CHARACTER) as u32 & 0xe) * 2;
    let per_class = g.prog32(tables::STUN_CLASS + class_offset);
    let blow = rd8(g.b, att + f::BLOW_ID) as u32;
    let chance = g.data8(per_class + (blow >> 2));
    if chance == 0 {
        return;
    }
    let tick = (ram::TICK.get(g.b) & 0x1e) * 2;
    let offset = (((chance - 1) as u16) << 10).wrapping_add(tick);
    let mask = g.data16(tables::STUN_MASKS.wrapping_add(offset as i16 as i32 as u32)) as u32;
    let roll = (g.random() & 0x1f) as u32;
    if mask >> roll & 1 != 0 {
        set_stun_time(g, v);
    }
}

/// 0xf1c6e: stagger duration, drawn from the table (according to the victim's `+0x6f`, the random
/// counter and a draw).
fn set_stun_time(g: &mut Ctx, v: u32) {
    cov!();
    let timer = (rd8(g.b, v + f::STUN_TIMER) as u16) << 8;
    let tick = ram::TICK.get(g.b) << 4;
    let roll = g.random() & 0xf;
    let index = timer.wrapping_add(tick).wrapping_add(roll);
    let time = g.data8(tables::STUN_TIMES.wrapping_add(index as i16 as i32 as u32));
    wr8(g.b, v + f::STUN_TIME, time);
}

/// 0xf232e, `enemy_reacts` for the hit resolution (`combat`): returns the A2 that the original leaves, the threshold
/// table, which the hit resolution reads back (original glitch: `combat::World::a2`).
pub fn enemy_reacts_from_hit(b: &mut Cps2Bus, a: Player, v: u32) -> u32 {
    let is_enemy = rd8(b, v + f::CLASS) == CLASS_ENEMY;
    enemy_reacts(&mut Ctx::new(b), a, v);
    if is_enemy { tables::THRESHOLD_ENEMY } else { tables::THRESHOLD_OTHER }
}
