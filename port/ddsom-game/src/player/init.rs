//! Creation and initialization of the players (P1).
//!
//! - `clear_players` (0x2d9e) resets the four 0x200-byte records to zero;
//! - `clear_overlays` (0x2df8) resets the eight 0x60-byte sub-objects (weapons and shields)
//!   that follow the player records (`A5+0xdde`..) to zero;
//! - `init_slot` (0x2e58, 0x2e6e, 0x2e86, 0x2e9e, one entry per player) builds a player from
//!   their character (`+3`, already set by the caller): level, hit points, randomly drawn
//!   attributes, equipment, starting spells and weapons. The values come from ROM tables
//!   (one row per character, then per level) read at run time.

use crate::machine::Machine;
use cps2::Cps2Bus;

use super::{ram, Ctx, Player, PLAYER_COUNT, PLAYER_STRIDE};
use crate::mem::{wr32, A5};
use crate::Native;

/// Program ROM tables: one pointer per character (12), each to its own table.
pub(crate) mod tables {
    /// Initial level (`LEVEL`) per scene.
    pub const LEVEL_BY_SCENE: u32 = 0x3208;
    /// Hit points (words) per level.
    pub const HP_BY_LEVEL: u32 = 0x325e;
    /// Eight sets of attributes (8 bytes), one of which is drawn at random.
    pub const RANDOM_STATS: u32 = 0x32d8;
    /// Equipment per level (12 longs of 0x30 bytes per level).
    pub const GEAR_BY_LEVEL: u32 = 0x3402;
    /// List (terminated by 0) of unlocked item flags, 8 bytes per level.
    pub const UNLOCKED_BY_LEVEL: u32 = 0x345e;
    /// Ten bytes per level (two longs and a word) at `+0x150`.
    pub const SPELLS_BY_LEVEL: u32 = 0x34b4;
    /// Starting weapons per level (4 bytes per level).
    pub const WEAPONS_BY_LEVEL: u32 = 0x3522;
    /// Attack power per weapon (2 bytes per weapon number).
    pub const WEAPON_STATS: u32 = 0x3590;
}

/// Unlocked item and spell flags (byte array at `A5-0x5b88`), copied on death.
pub const UNLOCKED_FLAGS: u32 = 0xff_2478;

/// Resets `n` blocks of 32 bytes starting at `at` (the `dbf` loop of the original).
fn clear(g: &mut Ctx, at: u32, blocks: u32) {
    cov!();
    for i in 0..blocks * 8 {
        wr32(g.b, at + 4 * i, 0);
    }
}

/// Clears the record of player `k`.
pub fn clear_player(g: &mut Ctx, k: u32) {
    cov!();
    clear(g, Player::nth(k).0, PLAYER_STRIDE / 32);
}

/// Addresses of the eight sub-objects (two per player: weapon, shield).
pub fn overlay(k: u32, shield: bool) -> u32 {
    A5 + 0xdde + 0x60 * (2 * k + shield as u32)
}

/// Clears the two sub-objects of player `k`.
pub fn clear_overlays_of(g: &mut Ctx, k: u32) {
    cov!();
    clear(g, overlay(k, false), 3);
    clear(g, overlay(k, true), 3);
}

/// 0x2d9e: the four player records to zero.
pub fn clear_all_players(b: &mut Cps2Bus) {
    let mut g = Ctx::new(b);
    for k in 0..PLAYER_COUNT {
        clear_player(&mut g, k);
    }
}

/// 0x2df8: the eight player sub-objects to zero.
pub fn clear_sub_objects(b: &mut Cps2Bus) {
    let mut g = Ctx::new(b);
    for k in 0..PLAYER_COUNT {
        clear_overlays_of(&mut g, k);
    }
}

pub const CLEAR_PLAYERS: Native = Native {
    entry: 0x2d9e,
    name: "players_clear",
    run: crate::Body::Memory(clear_all_players),
    touches_sound: false,
    outputs: 0,
};

pub const CLEAR_OVERLAYS: Native = Native {
    entry: 0x2df8,
    name: "players_clear_overlays",
    run: crate::Body::Memory(clear_sub_objects),
    touches_sound: false,
    outputs: 0,
};

/// Pointer to the table of `p`'s character in the pointer table `table`.
pub(crate) fn table_of(g: &mut Ctx, p: Player, table: u32) -> u32 {
    cov!();
    let character = p.u8(g, Player::CHARACTER) as u32;
    g.prog32(table + 4 * character)
}

/// 0x31f0: level according to the scene.
fn init_level(g: &mut Ctx, p: Player) {
    cov!();
    let t = table_of(g, p, tables::LEVEL_BY_SCENE);
    let scene = ram::SCENE.get(g.b) as u32;
    let level = g.data8(t + scene);
    p.set_u8(g, Player::LEVEL, level);
}

/// 0x3238: hit points (maximum, current, previous) according to the level.
fn init_hp(g: &mut Ctx, p: Player) {
    cov!();
    let t = table_of(g, p, tables::HP_BY_LEVEL);
    let level = p.u8(g, Player::LEVEL) as u32;
    let hp = g.data16(t + 2 * level);
    p.set_u16(g, Player::HP_MAX, hp);
    p.set_u16(g, Player::HP, hp);
    p.set_u16(g, Player::HP_PREV, hp);
}

/// 0x328e: one of the character's eight attribute sets, drawn at random.
fn init_random_stats(g: &mut Ctx, p: Player) {
    cov!();
    let t = table_of(g, p, tables::RANDOM_STATS);
    let row = t + 8 * (g.random() & 7) as u32;
    for (i, dst) in [0x100, 0x101, 0x102, 0x103, 0x104, 0x105, 0x106, 0x108].into_iter().enumerate() {
        let v = g.data8(row + i as u32);
        p.set_u8(g, dst, v);
    }
}

/// 0x3308: starting equipment (twelve longs at `+0x120`); the magic-users (characters 8 and 9)
/// receive a bonus of 0x14 for each of their three spells (codes 0x52, 0x53, 0x54).
/// (0x33b0: only the first eight longs, on resuming after a "continue".)
fn init_gear(g: &mut Ctx, p: Player, longs: u32) {
    cov!();
    // only pointer table read in data space (`lea` then register indexing)
    let character = p.u8(g, Player::CHARACTER) as u32;
    let t = g.data32(tables::GEAR_BY_LEVEL + 4 * character);
    let level = p.u8(g, Player::LEVEL) as u32;
    let row = t + 0x30 * level;
    for i in 0..longs {
        let v = g.data32(row + 4 * i);
        p.set_u32(g, Player::SLOT_TABLE + 4 * i, v);
    }
    if matches!(p.u8(g, Player::CHARACTER), 8 | 9) {
        for i in 0..6 {
            match p.u8(g, 0x140 + i) {
                0x52 => p.set_u8(g, 0x18c, 0x14),
                0x53 => p.set_u8(g, 0x18b, 0x14),
                0x54 => p.set_u8(g, 0x18d, 0x14),
                _ => {}
            }
        }
    }
}

/// 0xcf182: raises the spell counters (`+0x151..+0x159`, nine signed bytes) to the minimum planned for
/// the player's level. The per-class table (0xcf1b0) has a ten-byte row per level (starting from the
/// class base level, bytes 0xcf17c): a first negative byte disables it, then the nine minima.
pub fn raise_spells(g: &mut Ctx, p: Player) {
    const BASE_LEVEL: u32 = 0xcf17c;
    const MINIMA: u32 = 0xcf1b0;
    let class = (p.u8(g, Player::CHARACTER) >> 1) as u32;
    let level = p.u8(g, Player::LEVEL).wrapping_sub(g.prog8(BASE_LEVEL + class)) as u32;
    let row = g.prog32(MINIMA + 4 * class) + 10 * level;
    if g.data8(row) as i8 >= 0 {
        for i in 0..9 {
            let minimum = g.data8(row + 1 + i) as i8;
            if minimum > p.u8(g, 0x151 + i) as i8 {
                p.set_u8(g, 0x151 + i, minimum as u8);
            }
        }
    }
}

/// 0x3432: unlocks the listed flags (non-zero bytes) for this character and level.
fn init_unlocked(g: &mut Ctx, p: Player) {
    cov!();
    let t = table_of(g, p, tables::UNLOCKED_BY_LEVEL);
    let level = p.u8(g, Player::LEVEL) as u32;
    let mut at = t + 8 * level;
    loop {
        let flag = g.data8(at) as u32;
        if flag == 0 {
            break;
        }
        crate::mem::wr8(g.b, UNLOCKED_FLAGS + flag, 1);
        at += 1;
    }
}

/// 0x348e: spell reserve (two longs and a word at `+0x150`).
fn init_spells(g: &mut Ctx, p: Player) {
    cov!();
    let t = table_of(g, p, tables::SPELLS_BY_LEVEL);
    let level = p.u8(g, Player::LEVEL) as u32;
    let row = t + 16 * level;
    let (a, b) = (g.data32(row), g.data32(row + 4));
    let c = g.data16(row + 8);
    p.set_u32(g, 0x150, a);
    p.set_u32(g, 0x154, b);
    p.set_u16(g, 0x158, c);
}

/// 0x34e4: starting weapons (`WEAPON_A`, `WEAPON_B`) and two unlocked item flags.
fn init_weapons(g: &mut Ctx, p: Player) {
    cov!();
    let t = table_of(g, p, tables::WEAPONS_BY_LEVEL);
    let level = p.u8(g, Player::LEVEL) as u32;
    let row = t + 4 * level;
    let (w1, w2, f1, f2) = (g.data8(row), g.data8(row + 1), g.data8(row + 2) as u32, g.data8(row + 3) as u32);
    p.set_u8(g, Player::WEAPON_A, w1);
    p.set_u8(g, Player::WEAPON_B, w2);
    crate::mem::wr8(g.b, UNLOCKED_FLAGS + f1, 1);
    crate::mem::wr8(g.b, UNLOCKED_FLAGS + f2, 1);
}

/// 0x3552: base power of the weapon (`+0x109` for `WEAPON_A`, `+0x10a` for `WEAPON_B`, numbers
/// up to 0x27) and damage cap (`+0x16e`, two bytes per weapon in the table).
pub fn init_weapon_stats(g: &mut Ctx, p: Player) {
    cov!();
    let t = table_of(g, p, tables::WEAPON_STATS);
    for (weapon, dst) in [(Player::WEAPON_A, 0x109), (Player::WEAPON_B, 0x10a)] {
        let w = p.u8(g, weapon) as u32;
        if w <= 0x27 {
            let (power, cap) = (g.data8(t + 2 * w), g.data8(t + 2 * w + 1));
            p.set_u8(g, dst, power);
            p.set_u8(g, 0x16e, cap);
        }
    }
}

/// 0x2eb2: construction of player number `k`.
pub fn init_slot(g: &mut Ctx, p: Player, k: u8) {
    cov!();
    p.set_u8(g, Player::INDEX, k);
    let mask = ram::PLAYERS_MASK.get(g.b) | 1 << k;
    ram::PLAYERS_MASK.set(g.b, mask);
    p.set_u16(g, Player::ACTIVE, 0x101);
    p.set_u8(g, Player::PALETTE_GROUP, 2);
    p.set_u8(g, Player::PALETTE, k);
    p.set_u8(g, Player::SLOW_8A, 0);
    p.set_u8(g, Player::POWER_10F, 0);
    p.set_u8(g, 0x18a, 0);
    p.set_u8(g, 0x193, 0);
    p.set_u32(g, Player::SCORE_BCD, 0);
    init_level(g, p);
    init_hp(g, p);
    init_random_stats(g, p);
    init_spells(g, p);
    init_gear(g, p, 12);
    init_unlocked(g, p);
    init_weapons(g, p);
    init_weapon_stats(g, p);
    p.set_u32(g, 0x160, !0);
    p.set_u32(g, 0x164, !0);
    p.set_u32(g, 0x114, 0x50);
    p.set_u32(g, 0x118, !0);
    p.set_u32(g, 0x11c, !0);
    let class = p.class(g);
    p.set_u8(g, Player::BOX_SET, class);
    p.set_u8(g, 0x161, class);
    let spare = p.u8(g, Player::WEAPON_B).wrapping_sub(0x28);
    if (spare as i8) >= 0 {
        p.set_u8(g, 0x164, spare);
    }
    let alive = ram::PLAYERS_ALIVE.get(g.b).wrapping_add(1);
    ram::PLAYERS_ALIVE.set(g.b, alive);
}

/// The entries 0x2e58, 0x2e6e, 0x2e86, 0x2e9e: build player `k` (the original keeps its registers: `movem`).
pub fn initialize(b: &mut Cps2Bus, k: u32) {
    init_slot(&mut Ctx::new(b), Player::nth(k), k as u8);
}

fn init_p1(b: &mut Cps2Bus) {
    initialize(b, 0)
}
fn init_p2(b: &mut Cps2Bus) {
    initialize(b, 1)
}
fn init_p3(b: &mut Cps2Bus) {
    initialize(b, 2)
}
fn init_p4(b: &mut Cps2Bus) {
    initialize(b, 3)
}

const fn init_native(entry: u32, name: &'static str, run: fn(&mut Cps2Bus)) -> Native {
    Native { entry, name, run: crate::Body::Memory(run), touches_sound: false, outputs: 0 }
}

pub const INIT_P1: Native = init_native(0x2e58, "player_init_1", init_p1);
pub const INIT_P2: Native = init_native(0x2e6e, "player_init_2", init_p2);
pub const INIT_P3: Native = init_native(0x2e86, "player_init_3", init_p3);
pub const INIT_P4: Native = init_native(0x2e9e, "player_init_4", init_p4);

// ---- arrival of a player during the game ---------------------------------------------------------

/// Three ways to join during the game (one entry per player for each).
#[derive(Clone, Copy, PartialEq)]
pub enum Join {
    /// 0x2f46..: new player (score reset to zero).
    New,
    /// 0x3048..: resume after "continue" (the score is kept).
    Continue,
    /// 0x3142..: resume without a new draw: HP, equipment (eight longs) and flags only.
    Refill,
}

/// Player `k` arrives during the game: they fall from the sky (scripted revival state, `0x00040000`;
/// `0x00040800` frozen inputs, `0x00040c00` global slow walk), then all enemies and bosses receive
/// a shock (`join_shockwave`).
/// The entries 0x2f46.., 0x3048.., 0x3142.. (one per player `k`): the player arrives during the game.
pub fn join_player(b: &mut Cps2Bus, k: u8, how: Join) {
    join(&mut Ctx::new(b), k, how);
}

fn join(g: &mut Ctx, k: u8, how: Join) {
    let p = Player::nth(k as u32);
    p.set_u8(g, Player::INDEX, k);
    let mask = ram::PLAYERS_MASK.get(g.b) | 1 << k;
    ram::PLAYERS_MASK.set(g.b, mask);
    let state = if ram::INPUT_RAW.get(g.b) != 0 {
        0x0004_0800
    } else if ram::SLOW_MODE.get(g.b) != 0 {
        0x0004_0c00
    } else {
        0x0004_0000
    };
    p.set_state(g, state);
    p.set_u16(g, Player::ACTIVE, 0x101);
    p.set_u8(g, Player::PALETTE_GROUP, 2);
    p.set_u8(g, Player::PALETTE, k);
    p.set_u8(g, Player::SLOW_8A, 0);
    p.set_u8(g, Player::POWER_10F, 0);
    p.set_u8(g, 0x18a, 0);
    if how == Join::Refill {
        init_hp(g, p);
        init_gear(g, p, 8);
        init_unlocked(g, p);
        raise_spells(g, p);
    } else {
        if how == Join::New {
            p.set_u32(g, Player::SCORE_BCD, 0);
        }
        init_level(g, p);
        init_hp(g, p);
        init_random_stats(g, p);
        init_spells(g, p);
        init_gear(g, p, 12);
        init_unlocked(g, p);
        init_weapons(g, p);
        init_weapon_stats(g, p);
        if how == Join::New {
            p.set_u32(g, 0x114, 0x200);
        }
        p.set_u32(g, 0x160, !0);
        p.set_u32(g, 0x164, !0);
        p.set_u32(g, 0x118, !0);
        p.set_u32(g, 0x11c, !0);
        let class = p.class(g);
        p.set_u8(g, Player::BOX_SET, class);
        p.set_u8(g, 0x161, class);
        let spare = p.u8(g, Player::WEAPON_B).wrapping_sub(0x28);
        if (spare as i8) >= 0 {
            p.set_u8(g, 0x164, spare);
        }
    }
    let alive = ram::PLAYERS_ALIVE.get(g.b).wrapping_add(1);
    ram::PLAYERS_ALIVE.set(g.b, alive);
    let strength = if how == Join::New { 0 } else { alive };
    join_shockwave(g, p, strength);
}

/// Shock damage by number of players (words, read in data space by `lea` then register; masked to
/// 63); RAM word that holds them.
const SHOCK_DAMAGE: u32 = 0x10108;
const SHOCK_DAMAGE_RAM: u32 = A5 - 0x7194;
/// 0x10022: shock of the arrival of player `p`: each boss then each living, visible, unprotected enemy
/// (`+0x40`, bit 31 of `+0x94`) is thrown (state `0x04000000`, reaction 12, direction opposite to its own) and loses
/// HP (bosses with 32 HP or less are spared, and keep at least 1 HP).
fn join_shockwave(g: &mut Ctx, p: Player, strength: u8) {
    let damage = g.data16(SHOCK_DAMAGE + 2 * strength as u32) & 63;
    crate::mem::wr16(g.b, SHOCK_DAMAGE_RAM, damage);
    for pool in [&crate::objects::pools::BOSS, &crate::objects::pools::ENEMY] {
        for o in pool.iter() {
            let e = o.0;
            let b = &mut *g.b;
            let hp = crate::mem::rd16(b, e + 0x62);
            if crate::mem::rd8(b, e) == 0
                || crate::mem::rd8(b, e + 1) & 1 == 0
                || hp == 0
                || crate::mem::rd8(b, e + 0x40) != 0
                || (crate::mem::rd32(b, e + 0x94) as i32) < 0
            {
                continue;
            }
            wr32(b, e + 4, 0x0400_0000);
            crate::mem::wr8(b, e + 1, 1);
            let (index, character) = (crate::mem::rd8(b, p.at(2)), crate::mem::rd8(b, p.at(3)));
            crate::mem::wr8(b, e + 0x48, index);
            crate::mem::wr8(b, e + 0x4b, character);
            crate::mem::wr16(b, e + 0x54, p.0 as u16);
            wr32(b, e + 0x8a, 0x0400_0000);
            crate::mem::wr8(b, e + 0x49, 12);
            let facing = crate::mem::rd8(b, e + 0x2c) ^ 0x20;
            crate::mem::wr8(b, e + 0x4a, facing);
            let boss = crate::mem::rd8(b, e + 0x28) == 4;
            if !(boss && hp <= 32) {
                let left = hp.wrapping_sub(damage);
                let left = if (left as i16) > 0 { left } else if boss { 1 } else { 0 };
                crate::mem::wr16(b, e + 0x62, left);
            }
            let kind = crate::mem::rd8(b, e + 2);
            let bar = crate::events::Popup {
                header: (index as u16) << 8 | kind as u16,
                max: crate::mem::rd16(g.b, e + 0x60),
                value: crate::mem::rd16(g.b, e + 0x62),
                raw: crate::mem::rd8(g.b, e + 0x28) == 2,
            };
            crate::events::create_popup(g.b, bar);
        }
        if ram::SCROLL1_Y.get(g.b) != 0 {
            ram::SCROLL1_Y.set(g.b, 0);
        }
    }
}

const fn join_native(entry: u32, name: &'static str, run: fn(&mut Cps2Bus)) -> Native {
    Native { entry, name, run: crate::Body::Memory(run), touches_sound: false, outputs: 0 }
}

macro_rules! join_entry {
    ($entry:literal, $name:literal, $k:literal, $how:expr) => {
        join_native($entry, $name, |b| join_player(b, $k, $how))
    };
}

pub const JOIN: &[Native] = &[
    join_entry!(0x2f46, "player_join_new_1", 0, Join::New),
    join_entry!(0x2f58, "player_join_new_2", 1, Join::New),
    join_entry!(0x2f6c, "player_join_new_3", 2, Join::New),
    join_entry!(0x2f80, "player_join_new_4", 3, Join::New),
    join_entry!(0x3048, "player_join_continue_1", 0, Join::Continue),
    join_entry!(0x305a, "player_join_continue_2", 1, Join::Continue),
    join_entry!(0x306e, "player_join_continue_3", 2, Join::Continue),
    join_entry!(0x3082, "player_join_continue_4", 3, Join::Continue),
    join_entry!(0x3142, "player_join_refill_1", 0, Join::Refill),
    join_entry!(0x3154, "player_join_refill_2", 1, Join::Refill),
    join_entry!(0x3168, "player_join_refill_3", 2, Join::Refill),
    join_entry!(0x317c, "player_join_refill_4", 3, Join::Refill),
];
