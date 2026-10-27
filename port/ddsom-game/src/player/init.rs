//! Création et initialisation des joueurs (P1).
//!
//! - `clear_players` (0x2d9e) remet à zéro les quatre enregistrements de 0x200 octets ;
//! - `clear_overlays` (0x2df8) remet à zéro les huit sous-objets (armes et boucliers) de 0x60 octets
//!   qui suivent les enregistrements joueur (`A5+0xdde`..) ;
//! - `init_slot` (0x2e58, 0x2e6e, 0x2e86, 0x2e9e, une entrée par joueur) construit un joueur à partir
//!   de son personnage (`+3`, déjà posé par l'appelant) : niveau, points de vie, caractéristiques tirées
//!   au hasard, équipement, sorts et armes de départ. Les valeurs viennent de tables de la ROM
//!   (une ligne par personnage, puis par niveau) lues à l'exécution.

use cps2::Cps2Bus;
use m68k::Cpu;

use super::{ram, Ctx, Player, PLAYER_COUNT, PLAYER_STRIDE};
use crate::mem::{rts, wr32, A5};
use crate::{regs, Native};

/// Tables de la ROM programme : un pointeur par personnage (12), chacun vers une table propre.
mod tables {
    /// Niveau initial (`LEVEL`) par scène.
    pub const LEVEL_BY_SCENE: u32 = 0x3208;
    /// Points de vie (mots) par niveau.
    pub const HP_BY_LEVEL: u32 = 0x325e;
    /// Huit jeux de caractéristiques (8 octets) dont l'un est tiré au hasard.
    pub const RANDOM_STATS: u32 = 0x32d8;
    /// Équipement par niveau (12 longs de 0x30 octets par niveau).
    pub const GEAR_BY_LEVEL: u32 = 0x3402;
    /// Liste (terminée par 0) des drapeaux d'objets débloqués, 8 octets par niveau.
    pub const UNLOCKED_BY_LEVEL: u32 = 0x345e;
    /// Dix octets par niveau (deux longs et un mot) en `+0x150`.
    pub const SPELLS_BY_LEVEL: u32 = 0x34b4;
    /// Armes de départ par niveau (4 octets par niveau).
    pub const WEAPONS_BY_LEVEL: u32 = 0x3522;
    /// Puissance d'attaque par arme (2 octets par numéro d'arme).
    pub const WEAPON_STATS: u32 = 0x3590;
}

/// Drapeaux d'objets et sorts débloqués (tableau d'octets en `A5-0x5b88`), copiés à la mort.
pub const UNLOCKED_FLAGS: u32 = 0xff_2478;

/// Remet à zéro `n` blocs de 32 octets à partir de `at` (la boucle `dbf` de l'original).
fn clear(g: &mut Ctx, at: u32, blocks: u32) {
    cov!();
    for i in 0..blocks * 8 {
        wr32(g.b, at + 4 * i, 0);
    }
}

/// Efface l'enregistrement du joueur `k`.
pub fn clear_player(g: &mut Ctx, k: u32) {
    cov!();
    clear(g, Player::nth(k).0, PLAYER_STRIDE / 32);
}

/// Adresses des huit sous-objets (deux par joueur : arme, bouclier).
pub fn overlay(k: u32, shield: bool) -> u32 {
    A5 + 0xdde + 0x60 * (2 * k + shield as u32)
}

/// Efface les deux sous-objets du joueur `k`.
pub fn clear_overlays_of(g: &mut Ctx, k: u32) {
    cov!();
    clear(g, overlay(k, false), 3);
    clear(g, overlay(k, true), 3);
}

fn clear_players(c: &mut Cpu, b: &mut Cps2Bus) {
    let mut g = Ctx::new(c, b);
    for k in 0..PLAYER_COUNT {
        clear_player(&mut g, k);
    }
    rts(c, b);
}

fn clear_overlays(c: &mut Cpu, b: &mut Cps2Bus) {
    let mut g = Ctx::new(c, b);
    for k in 0..PLAYER_COUNT {
        clear_overlays_of(&mut g, k);
    }
    rts(c, b);
}

pub const CLEAR_PLAYERS: Native = Native {
    entry: 0x2d9e,
    name: "players_clear",
    run: clear_players,
    touches_sound: false,
    outputs: 0,
};

pub const CLEAR_OVERLAYS: Native = Native {
    entry: 0x2df8,
    name: "players_clear_overlays",
    run: clear_overlays,
    touches_sound: false,
    outputs: 0,
};

/// Pointeur de la table du personnage de `p` dans la table de pointeurs `table`.
fn table_of(g: &mut Ctx, p: Player, table: u32) -> u32 {
    cov!();
    let character = p.u8(g, Player::CHARACTER) as u32;
    g.prog32(table + 4 * character)
}

/// 0x31f0 : niveau selon la scène.
fn init_level(g: &mut Ctx, p: Player) {
    cov!();
    let t = table_of(g, p, tables::LEVEL_BY_SCENE);
    let scene = g.a5u8(ram::SCENE) as u32;
    let level = g.data8(t + scene);
    p.set_u8(g, Player::LEVEL, level);
}

/// 0x3238 : points de vie (maximum, courants, précédents) selon le niveau.
fn init_hp(g: &mut Ctx, p: Player) {
    cov!();
    let t = table_of(g, p, tables::HP_BY_LEVEL);
    let level = p.u8(g, Player::LEVEL) as u32;
    let hp = g.data16(t + 2 * level);
    p.set_u16(g, Player::HP_MAX, hp);
    p.set_u16(g, Player::HP, hp);
    p.set_u16(g, Player::HP_PREV, hp);
}

/// 0x328e : un des huit jeux de caractéristiques du personnage, tiré au hasard.
fn init_random_stats(g: &mut Ctx, p: Player) {
    cov!();
    let t = table_of(g, p, tables::RANDOM_STATS);
    let row = t + 8 * (g.random() & 7) as u32;
    for (i, dst) in [0x100, 0x101, 0x102, 0x103, 0x104, 0x105, 0x106, 0x108].into_iter().enumerate() {
        let v = g.data8(row + i as u32);
        p.set_u8(g, dst, v);
    }
}

/// 0x3308 : équipement de départ (douze longs en `+0x120`) ; les magiciens (personnages 8 et 9)
/// reçoivent un bonus de 0x14 pour chacun de leurs trois sorts (codes 0x52, 0x53, 0x54).
/// (0x33b0 : seulement les huit premiers longs, à la reprise après « continue ».)
fn init_gear(g: &mut Ctx, p: Player, longs: u32) {
    cov!();
    // seule table de pointeurs lue en espace données (`lea` puis indexation par registre)
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

/// 0xcf182 : relève les compteurs de sorts (`+0x151..+0x159`, neuf octets signés) au minimum prévu pour
/// le niveau du joueur. La table par classe (0xcf1b0) a une ligne de dix octets par niveau (à partir du
/// niveau de base de la classe, octets 0xcf17c) : un premier octet négatif la désactive, puis les neuf minima.
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

/// 0x3432 : débloque les drapeaux listés (octets non nuls) pour ce personnage et ce niveau.
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

/// 0x348e : réserve de sorts (deux longs et un mot en `+0x150`).
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

/// 0x34e4 : armes de départ (`WEAPON_A`, `WEAPON_B`) et deux drapeaux d'objets débloqués.
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

/// 0x3552 : puissance de base de l'arme (`+0x109` pour `WEAPON_A`, `+0x10a` pour `WEAPON_B`, numéros
/// jusqu'à 0x27) et plafond de dégâts (`+0x16e`, deux octets par arme dans la table).
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

/// 0x2eb2 : construction d'un joueur de numéro `k`.
pub fn init_slot(g: &mut Ctx, p: Player, k: u8) {
    cov!();
    p.set_u8(g, Player::INDEX, k);
    let mask = g.a5u8(ram::PLAYERS_MASK) | 1 << k;
    g.set_a5u8(ram::PLAYERS_MASK, mask);
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
    let alive = g.a5u8(ram::PLAYERS_ALIVE).wrapping_add(1);
    g.set_a5u8(ram::PLAYERS_ALIVE, alive);
}

/// Les entrées 0x2e58.. sauvegardent D0 et A0-A3 (movem) puis construisent le joueur `k`.
fn init_entry(c: &mut Cpu, b: &mut Cps2Bus, k: u32) {
    let (d0, a) = (c.d[0], [c.a[0], c.a[1], c.a[2], c.a[3]]);
    init_slot(&mut Ctx::new(c, b), Player::nth(k), k as u8);
    c.d[0] = d0;
    c.a[..4].copy_from_slice(&a);
    rts(c, b);
}

fn init_p1(c: &mut Cpu, b: &mut Cps2Bus) {
    init_entry(c, b, 0)
}
fn init_p2(c: &mut Cpu, b: &mut Cps2Bus) {
    init_entry(c, b, 1)
}
fn init_p3(c: &mut Cpu, b: &mut Cps2Bus) {
    init_entry(c, b, 2)
}
fn init_p4(c: &mut Cpu, b: &mut Cps2Bus) {
    init_entry(c, b, 3)
}

const fn init_native(entry: u32, name: &'static str, run: fn(&mut Cpu, &mut Cps2Bus)) -> Native {
    Native {
        entry,
        name,
        run,
        touches_sound: false,
        outputs: regs::d(0) | regs::a(0) | regs::a(1) | regs::a(2) | regs::a(3),
    }
}

pub const INIT_P1: Native = init_native(0x2e58, "player_init_1", init_p1);
pub const INIT_P2: Native = init_native(0x2e6e, "player_init_2", init_p2);
pub const INIT_P3: Native = init_native(0x2e86, "player_init_3", init_p3);
pub const INIT_P4: Native = init_native(0x2e9e, "player_init_4", init_p4);

// ---- arrivée d'un joueur en cours de partie ---------------------------------------------------------

/// Trois façons d'entrer en cours de partie (une entrée par joueur pour chacune).
#[derive(Clone, Copy, PartialEq)]
enum Join {
    /// 0x2f46.. : nouveau joueur (score remis à zéro).
    New,
    /// 0x3048.. : reprise après « continue » (le score est gardé).
    Continue,
    /// 0x3142.. : reprise sans nouveau tirage : PV, équipement (huit longs) et drapeaux seulement.
    Refill,
}

/// Le joueur `k` arrive en cours de partie : il tombe du ciel (état scénarisé de résurrection, `0x00040000` ;
/// `0x00040800` entrées figées, `0x00040c00` marche lente globale), puis tous les ennemis et boss reçoivent
/// un choc (`join_shockwave`).
fn join(g: &mut Ctx, k: u8, how: Join) {
    let p = Player::nth(k as u32);
    p.set_u8(g, Player::INDEX, k);
    let mask = g.a5u8(ram::PLAYERS_MASK) | 1 << k;
    g.set_a5u8(ram::PLAYERS_MASK, mask);
    let state = if g.a5u8(ram::INPUT_RAW) != 0 {
        0x0004_0800
    } else if g.a5u8(ram::SLOW_MODE) != 0 {
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
    let alive = g.a5u8(ram::PLAYERS_ALIVE).wrapping_add(1);
    g.set_a5u8(ram::PLAYERS_ALIVE, alive);
    let strength = if how == Join::New { 0 } else { alive };
    g.c.a[0] = p.0;
    join_shockwave(g, p, strength);
}

/// Dégâts du choc par nombre de joueurs (mots, lus en espace de données par `lea` puis registre ; masqués à
/// 63) ; mot de RAM qui les garde.
const SHOCK_DAMAGE: u32 = 0x10108;
const SHOCK_DAMAGE_RAM: u32 = A5 - 0x7194;
/// Jauge de vie d'un ennemi touché (D0 = joueur << 8 | type, D1 = PV max, D2 = PV, D3 = classe).
const SHOW_GAUGE: u32 = 0x19dce;

/// 0x10022 : choc de l'arrivée du joueur `p` : chaque boss puis chaque ennemi vivant, visible, non protégé
/// (`+0x40`, bit 31 de `+0x94`) est projeté (état `0x04000000`, réaction 12, sens opposé au sien) et perd
/// des PV (les boss à 32 PV ou moins sont épargnés, et gardent au moins 1 PV).
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
            g.c.d[0] = (index as u32) << 8 | kind as u32;
            g.c.d[1] = g.c.d[1] & 0xffff_0000 | crate::mem::rd16(g.b, e + 0x60) as u32;
            g.c.d[2] = g.c.d[2] & 0xffff_0000 | crate::mem::rd16(g.b, e + 0x62) as u32;
            g.c.d[3] = g.c.d[3] & 0xffff_ff00 | crate::mem::rd8(g.b, e + 0x28) as u32;
            g.c.a[1] = e;
            g.c.a[0] = p.0;
            crate::call(g.c, g.b, SHOW_GAUGE);
        }
        if g.a5u16(0x4ba) != 0 {
            g.set_a5u16(0x4ba, 0);
        }
    }
}

const fn join_native(entry: u32, name: &'static str, run: fn(&mut Cpu, &mut Cps2Bus)) -> Native {
    Native { entry, name, run, touches_sound: false, outputs: 0 }
}

macro_rules! join_entry {
    ($entry:literal, $name:literal, $k:literal, $how:expr) => {
        join_native($entry, $name, |c, b| {
            join(&mut Ctx::new(c, b), $k, $how);
            rts(c, b);
        })
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

/// 0x3552 appelée par le code d'origine (fin de partie, boutique) : registres laissés comme l'original
/// (A4 = table du personnage, D0 = dernier numéro d'arme lu, doublé s'il est dans la table).
fn weapon_stats_native(c: &mut Cpu, b: &mut Cps2Bus) {
    let p = Player(c.a[0]);
    let mut g = Ctx::new(c, b);
    init_weapon_stats(&mut g, p);
    let table = table_of(&mut g, p, tables::WEAPON_STATS);
    let w = p.u8(&mut g, Player::WEAPON_B);
    g.c.a[4] = table;
    g.c.d[0] = if w <= 0x27 { (2 * w) as u32 } else { w as u32 };
    rts(c, b);
}

pub const NATIVES: &[Native] = &[Native {
    entry: 0x3552,
    name: "player_weapon_stats",
    run: weapon_stats_native,
    touches_sound: false,
    outputs: regs::d(0) | regs::a(4),
}];
