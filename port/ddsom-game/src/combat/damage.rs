//! Formules de dégâts (sous-système X2) : coups des joueurs, tirs des joueurs, attaques ennemies,
//! et affichage du nombre de points de vie retirés.
//!
//! Les barèmes sont des tables de la ROM (lues à l'exécution) ; seules les formules sont ici.

use super::*;
use crate::objects::pools;
use crate::machine::Machine;

/// Tables de barème de la ROM programme. Les tables lues par adressage relatif au PC de l'original
/// sont dans l'espace programme (`pc8`/`pc16`), les autres en lecture ordinaire.
mod table {
    /// 0x3be4 (PC) : variation aléatoire ajoutée aux dégâts, octets signés, 32 par ligne
    /// (ligne = min(dégâts de base - 1, 4)).
    pub const ROLL: u32 = 0x3be4;
    /// 0x3c84 (PC) : pour un coup critique, selon (PV bas, aléa 4 bits) : numéro de ligne de 0x3ca2.
    pub const CRIT_ROW: u32 = 0x3c84;
    /// 0x3ca2 (PC) : masques de 16 bits ; le critique a lieu si le bit tiré (aléa 4 bits) est à 1.
    pub const CRIT_MASK: u32 = 0x3ca2;
    /// 0x3d8e (données) : par arme (4 octets) : bonus de dégâts selon le nombre de joueurs
    /// (2, 3, 4 : octets signés) puis plafond.
    pub const PARTY_BONUS: u32 = 0x3d8e;
    /// 0x3e4e et 0x3ef8 (PC) : tables du coup critique du magicien (classe 4).
    pub const MAGE_ROW: u32 = 0x3e4e;
    pub const MAGE_SHIFT: u32 = 0x3ef8;
    /// 0x3fea (PC) : pointeurs vers les barèmes d'attaque ennemie par réglage de jeu (`A5+0x70`).
    pub const ENEMY_SCALE: u32 = 0x3fea;
    /// 0x406d (PC) : nombre de tranches de 1/32 retirées selon la protection (`+0x108`).
    pub const PROTECTION: u32 = 0x406d;
    /// 0x40aa et 0x416a (données) : à PV bas, par (classe, PV) : tranches de 1/32 retirées.
    pub const LOW_HP_SHIELD: [u32; 2] = [0x40aa, 0x416a];
    /// 0x3936 / 0x3954 (PC) : butin du voleur : probabilité par niveau, masques de tirage.
    pub const LOOT_LEVEL: u32 = 0x3936;
    pub const LOOT_MASK: u32 = 0x3954;
    /// 0x39d8 (données) : butins possibles (8 octets par type : 2 entrées de 4 octets).
    pub const LOOT: u32 = 0x39d8;
}

/// 0x3bbe : ajoute à `base` une variation tirée dans la table `ROLL` (ligne selon la valeur).
pub(super) fn roll_variation(w: &mut World, base: u16) -> u16 {
    cov::hit("roll_variation");
    let line = base.wrapping_sub(1).min(4) as u8;
    let line = (line << 5) as u16;
    let d0 = w.random() & 0x1f;
    let delta = w.prog8(table::ROLL + (d0 + line) as u32) as i8 as i16 as u16;
    base.wrapping_add(delta)
}

/// 0x422a : dégâts d'un tir de joueur. Puissance A contre un ennemi ordinaire, B sinon.
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

/// 0x3b36 : dégâts d'un coup de joueur `p` sur l'ennemi `e`.
///
/// Base : puissance de l'arme (`+0x109` ou `+0x10a`, selon `ATTACK_KIND` 0 ou 1 ; 1 pour le
/// type 2), +50 % sous bonus (`+0x79`), variation aléatoire, puis modificateurs : bonus de
/// position (`+0x100`), critique, faiblesse/résistance de l'ennemi, bonus de groupe. Le résultat
/// est aussi affiché (`show_damage`).
pub(super) fn player_damage(w: &mut World, p: Obj, e: Obj) -> u16 {
    cov::hit("player_damage");
    let kind = p.u8(w.b, f::ATTACK_KIND);
    let mut d5: u16 = match kind {
        0 => p.u8(w.b, 0x109) as u16,
        1 => p.u8(w.b, 0x10a) as u16,
        2 => 0,
        _ => return show_damage(w, p, 0, DamageDisplay::Dealt),
    };
    if kind != 2 {
        if p.u8(w.b, 0x79) != 0 {
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
    d5 = d5.wrapping_add(p.u8(w.b, 0x100) as u16);
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
        p.set_u8(w.b, 0x77, 0);
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

/// 0x3cb6.. : coup critique des classes autres que le magicien : +5 de dégâts, drapeau `+0x77`.
fn critical_hit(w: &mut World, p: Obj, d5: u16) -> u16 {
    cov::hit("critical_hit");
    if (p.u8(w.b, 0x3d) as i8) > 2 {
        return d5;
    }
    let mut row: u8 = if (p.u16(w.b, f::HP) as i16) > 16 { 0 } else { 16 };
    row = row.wrapping_add((w.random() & 15) as u8);
    let row = w.prog8(table::CRIT_ROW + row as u32);
    if row == 0 {
        return d5;
    }
    let mask = w.prog16(table::CRIT_MASK + row as u32);
    let bit = w.random() & 15;
    if mask >> bit & 1 == 0 {
        return d5;
    }
    p.set_u8(w.b, 0x77, 1);
    d5.wrapping_add(5)
}

/// 0x3e8e : le magicien (classe 4) équipé de l'arme 0x30 fait un coup critique de type "fraction
/// des PV max de la victime" (PV max >> n), sinon 1 point de dégâts ; avec une autre arme le
/// calcul commun continue (`None`).
fn mage_critical(w: &mut World, p: Obj, e: Obj) -> Option<u16> {
    cov::hit("mage_critical");
    if p.u8(w.b, 0x47) != 0x30 {
        return None;
    }
    let mut row: u8 = if (p.u16(w.b, f::HP) as i16) > 16 { 0 } else { 16 };
    if e.u8(w.b, f::CLASS) == 4 {
        row = row.wrapping_add(0x20);
    }
    row = row.wrapping_add((w.random() & 15) as u8);
    let first = w.prog8(table::MAGE_ROW + row as u32) as i8;
    if first < 0 {
        return Some(show_damage(w, p, 1, DamageDisplay::Dealt));
    }
    let idx = (first as u16).wrapping_add(w.random() & 15);
    let shift = w.prog8(table::MAGE_SHIFT + idx as u32) as i8;
    if shift < 0 {
        return Some(show_damage(w, p, 1, DamageDisplay::Dealt));
    }
    p.set_u8(w.b, 0x77, 1);
    p.set_u8(w.b, 0x18e, 3);
    let d0 = e.u16(w.b, f::HP_MAX).checked_shr(shift as u32 & 63).unwrap_or(0);
    Some(show_damage(w, p, d0, DamageDisplay::Dealt))
}

/// 0x3d10 : faiblesse/résistance élémentaire de la victime (`+0x9e`, `+0x9f`) à l'arme du joueur
/// (`+0x107`) : +4 si faible, moitié si résistante.
fn elemental(w: &mut World, p: Obj, e: Obj, d5: u16) -> u16 {
    cov::hit("elemental");
    let element = p.u8(w.b, 0x107);
    if element == 0 {
        return d5;
    }
    let weak = e.u8(w.b, 0x9e);
    if weak == 0 {
        return d5;
    }
    if element == weak {
        return d5.wrapping_add(4);
    }
    if element == e.u8(w.b, 0x9f) {
        return d5 >> 1;
    }
    d5
}

/// 0x3d3a : le groupe de joueurs en vie ajuste les dégâts : bonus (octet signé) de la table de
/// l'arme, plancher du même enregistrement.
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
    let bonus = rd8(w.b, row + alive.wrapping_sub(2) as u32) as i8 as i16 as u16;
    let floor = rd8(w.b, row + 3) as u16;
    let d5 = d5.wrapping_add(bonus);
    if (d5 as i16) > floor as i16 {
        d5
    } else {
        rd8(w.b, row + 3) as i8 as i16 as u16
    }
}

/// Qui subit les dégâts affichés ? (les deux nombres sont dessinés à des endroits différents de la
/// couche de caractères)
#[derive(Clone, Copy)]
pub(super) enum DamageDisplay {
    /// Dégâts infligés par le joueur (0x3f28 : colonne 0x9121e8).
    Dealt,
    /// Dégâts reçus par le joueur (0x3f32 : colonne 0x9121ec).
    Taken,
}

/// 0x3f28 / 0x3f32 : dessine le nombre de dégâts (deux chiffres, octet bas) à côté du joueur
/// `p`, sauf si le réglage d'écran le masque (bit 0x200 de `FFFFD0`). Renvoie `dmg`.
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
    // conversion en deux chiffres BCD par soustractions de 10 (octet bas seulement)
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

/// 0x4002 : dégâts d'une attaque ennemie (`atk`) sur le joueur `p`.
///
/// Base tirée d'un barème de 32 entrées (indexé par le compteur FFFF92) selon le réglage de jeu,
/// plus la puissance de l'attaque (`+0x45`), variation aléatoire, moins la défense du joueur
/// (`+0x10b`) ; puis réduction en tranches de 1/32 selon la protection (`+0x108`) ou, à 32 PV et
/// moins, selon la table des PV bas. Affiche le résultat.
pub(super) fn damage_vs_player(w: &mut World, p: Obj, atk: Obj) -> u16 {
    cov::hit("damage_vs_player");
    let setting = ram::DAMAGE_SETTING.get(w.b);
    let table = {
        let at = table::ENEMY_SCALE + 2 * setting as u32;
        w.prog16(at) as u32 * 0x10000 + w.prog16(at + 2) as u32
    };
    let tick = ram::TICK.get(w.b);
    let mut d5 = rd8(w.b, table + tick as u32) as i8 as i16 as u16;
    d5 = d5.wrapping_add(atk.u8(w.b, f::POWER_A) as u16);
    d5 = if (d5 as i16) > 0 { roll_variation(w, d5) } else { 1 };
    let defense = p.u8(w.b, 0x10b) as u16;
    let (diff, borrow) = d5.overflowing_sub(defense);
    d5 = if !borrow && diff != 0 { diff } else { 1 };
    // réduction par tranches de 1/32 (n tranches => d5 -= d5 * n / 32)
    let slices = if (p.u16(w.b, f::HP) as i16) > 0x20 {
        let prot = p.u8(w.b, 0x108);
        if (prot as i8) >= 9 {
            None
        } else {
            Some(w.prog8(table::PROTECTION + prot as u32))
        }
    } else {
        let class = (p.u8(w.b, f::VARIANT) >> 1) as u16;
        let idx = (class << 5).wrapping_sub(1).wrapping_add(p.u16(w.b, f::HP));
        let t = table::LOW_HP_SHIELD[(ram::DAMAGE_SETTING.get(w.b) == 6) as usize];
        let n = rd8(w.b, t.wrapping_add(idx as i16 as i32 as u32));
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

/// 0x388a : un coup de voleur (`ATTACK_KIND` 3) peut faire tomber du butin de l'ennemi `e`.
pub(super) fn steal_loot(w: &mut World, p: Obj, e: Obj) {
    cov::hit("steal_loot");
    if p.u8(w.b, f::VARIANT) >> 1 != 5 {
        return;
    }
    let loot = e.u8(w.b, 0x81) as u32;
    if loot == 0 {
        return;
    }
    let entry = table::LOOT + (loot << 3) + ((w.random() & 1) as u32) * 4;
    let weight = rd8(w.b, entry + 2).wrapping_add(rd8(w.b, entry + 3));
    let level = p.u8(w.b, 0x10c) as u32;
    let mut chance = w.prog8(table::LOOT_LEVEL + level).wrapping_sub(weight);
    if (chance as i8) < 0 {
        chance = 0;
    }
    let mask = (w.prog16(table::LOOT_MASK + 4 * chance as u32) as u32) << 16 | w.prog16(table::LOOT_MASK + 4 * chance as u32 + 2) as u32;
    let bit = w.random() & 0x1f;
    if mask >> bit & 1 == 0 {
        return;
    }
    // objet à créer (ITEM) à la place de l'ennemi ; échec si le pool est plein (retenue)
    w.a2 = e.0;
    w.c.a[1] = e.0;
    w.c.a[5] = A5;
    let Some(item) = pools::ITEM.alloc_68k(w.c, w.b) else { return };
    let item = Obj(item.0);
    item.set_u8(w.b, f::ACTIVE, 1);
    let (kind, var) = (rd8(w.b, entry), rd8(w.b, entry + 1));
    item.set_u8(w.b, f::KIND, kind);
    item.set_u8(w.b, f::VARIANT, var);
    let (x, z, y) = (e.u16(w.b, f::X), e.u16(w.b, f::Z), e.u16(w.b, f::Y));
    item.set_u16(w.b, f::X, x);
    item.set_u16(w.b, f::Z, z.wrapping_add(16));
    item.set_u16(w.b, f::Y, y.wrapping_sub(1));
    item.set_u16(w.b, 0x18, 0x300);
    item.set_u16(w.b, 0x1a, 0xffd8);
    e.set_u8(w.b, 0x81, 0);
}
