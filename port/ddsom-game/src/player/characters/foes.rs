//! Sorts qui blessent les monstres (elfe, magicien) : le programme d'origine recopie une boucle par
//! sort ; elles ne diffèrent que par quelques paramètres, décrits ici par `Strike`.
//!
//! La boucle parcourt les chefs puis les ennemis. Un monstre en jeu, vivant et sans protection contre ce
//! sort (un bit de `+0x94`) passe à la réaction `reaction` attribuée au lanceur, perd des points de
//! vie (table par niveau ou valeur fixe, majorée selon l'équipement) et sa barre de vie est mise à jour.

use crate::machine::Machine;
use super::common::call;
use super::super::{Ctx, Player};
use crate::objects::pools::{BOSS, ENEMY};
use crate::objects::Obj;
use crate::player::ram;
use crate::player::externes as ext;

/// Dégâts de base.
pub enum Damage {
    /// Octets de la ROM indexés par le niveau du lanceur (chefs, ennemis).
    Table(u32, u32),
    /// Valeur unique.
    Fixed(u16),
}

/// Test de l'octet `+1` du monstre (état de jeu).
pub enum Live {
    /// Non nul.
    Any,
    /// Bit 0.
    Bit0,
    /// Pas de test.
    Unchecked,
}

/// Majoration des dégâts (`d += d >> n`, ou +5), dans l'ordre d'application.
pub enum Boost {
    /// Le monstre est un chef (classe 4) de type `kind` : +50 %.
    Boss(u8),
    /// L'équipement `WEAPON_B` du lanceur vaut `weapon` : `d >> shift` en plus.
    Held(u8, u32),
    /// L'accessoire `+0x166` du lanceur vaut 4 : +25 %.
    Armor,
    /// L'accessoire `+0x165` du lanceur vaut `v` : +5.
    Charm(u8),
}

/// Un sort qui blesse les monstres.
pub struct Strike {
    pub damage: Damage,
    pub live: Live,
    /// Le monstre doit avoir des points de vie.
    pub needs_hp: bool,
    /// Le monstre doit être au sol (`Z` = 0).
    pub grounded: bool,
    /// Bit de `+0x94` qui protège le monstre.
    pub immune_bit: u32,
    /// Réaction (`+0x49`) imposée au monstre.
    pub reaction: u8,
    /// Le monstre est retourné au hasard (`+0x4a`).
    pub random_facing: bool,
    pub boosts: &'static [Boost],
    /// Les monstres de type 0 et 0x13 (non chefs) meurent sur le coup.
    pub undead_die: bool,
    /// Son joué après chaque barre de vie.
    pub sound: Option<u32>,
}

/// Applique `strike` à tous les monstres éligibles. `boss_table`/`enemy_table` remplacent les tables de
/// `Damage::Table` quand le sort choisit sa table à l'exécution.
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
            call(g, p, ext::effet::HEALTH_BAR, &[(0, bar), (1, max), (2, hp), (3, class)]);
            if let Some(sound) = s.sound {
                g.rom(sound, p);
            }
        }
    }
}
