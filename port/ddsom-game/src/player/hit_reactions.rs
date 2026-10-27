//! Réaction d'un ennemi (ou d'une caisse) touché par un joueur (P6).
//!
//! `enemy_reacts` (0xf232e) est appelée par la résolution des coups (`combat`, 0x264fe) juste après
//! que la victime est passée en état « touchée » : elle choisit l'animation de réaction de la victime
//! (octet `+0x49`) d'après la classe de l'attaquant, l'identifiant du coup (`+0x47`), le type de la
//! victime et un compteur d'étourdissement (`+0x88`) qui, une fois dépassé, la jette à terre. Puis
//! `maybe_stun` (0xf1c1a) tire au sort un temps de sonnement (`+0x87`).
//!
//! Les tables sont dans la ROM : par classe de l'attaquant puis par type de victime, une ligne de
//! quatre octets par identifiant de coup (réaction normale, en l'air, étourdie, ajout au compteur).

use crate::machine::Machine;
use cps2::Cps2Bus;
use m68k::Cpu;

use super::{ram, Ctx, Player};
use crate::mem::{rd16, rd8, rts, wr8};
use crate::Native;
use crate::player::externes as ext;

mod tables {
    /// Par classe de l'attaquant (pointeurs) puis type de victime (pointeurs) : lignes de réaction.
    pub const ENEMY: u32 = 0xf2656;
    /// Idem pour les victimes qui ne sont pas des ennemis ordinaires (boss...).
    pub const OTHER: u32 = 0xf266e;
    /// Seuils d'étourdissement : 8 octets par type de victime, un par huitième du compteur de hasard.
    pub const THRESHOLD_ENEMY: u32 = 0xf24fe;
    /// Seconde table, indexée par le seuil de la première, pour les victimes qui ne sont pas des ennemis.
    pub const THRESHOLD_OTHER: u32 = 0xf25ae;
    /// Par classe de l'attaquant (adresse, dans le code) : table de chances de sonner selon le coup.
    pub const STUN_CLASS: u32 = 0xf1cac;
    /// Masques de 16 bits : un tirage de 0..15 réussit si son bit est levé.
    pub const STUN_MASKS: u32 = 0xf1d2e;
    /// Durées de sonnement.
    pub const STUN_TIMES: u32 = 0xf1f2e;
}

/// Champs de la victime (`v`) et de l'attaquant (`a`).
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
    /// Attaquant : type de coup (< 2 : coup d'arme), identifiant du coup, drapeau, charge de baguette.
    pub const ATTACK_KIND: u32 = 0x3c;
    pub const BLOW_ID: u32 = 0x47;
    pub const FLAG_77: u32 = 0x77;
    pub const WAND_CHARGE: u32 = 0x191;
}

const CLASS_ENEMY: u8 = 2;
/// Coup du magicien qui étourdit.
const WIZARD_BLOW: u32 = 0x80;
/// Coup de poussée : la victime se retourne vers l'agresseur.
const PUSH_BLOW: u32 = 0x58;


/// 0xf232e : `a` est le joueur qui frappe, `v` la victime (adresse de l'objet).
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
    // lignes de réaction de ce type de victime pour cette classe d'attaquant
    let is_enemy = rd8(g.b, v + f::CLASS) == CLASS_ENEMY;
    let by_class = if is_enemy { tables::ENEMY } else { tables::OTHER };
    let class_offset = (rd8(g.b, att + Player::CHARACTER) as u32 & 0xe) * 2;
    let kind = rd8(g.b, v + f::KIND) as u32;
    let per_kind = g.data32(by_class + class_offset);
    let rows = g.data32(per_kind + 4 * kind);
    // seuil d'étourdissement : deux niveaux de table pour les victimes qui ne sont pas des ennemis
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
            // armes qui étourdissent à coup sûr : réaction fixe et compteur qui monte de 2
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
                            g.c.d[0] = 0xf;
                            g.rom(ext::effet::WAND, a);
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
            // seuil dépassé : la victime est jetée à terre
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

/// 0xf1c1a : l'attaque peut étourdir la victime : un tirage selon la classe de l'attaquant, le coup et
/// un compteur décide si un temps de sonnement est posé.
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
    let tick = ((ram::TICK.get(g.b) & 0x1e) * 2) as u16;
    let offset = (((chance - 1) as u16) << 10).wrapping_add(tick);
    let mask = g.data16(tables::STUN_MASKS.wrapping_add(offset as i16 as i32 as u32)) as u32;
    let roll = (g.random() & 0x1f) as u32;
    if mask >> roll & 1 != 0 {
        set_stun_time(g, v);
    }
}

/// 0xf1c6e : durée du sonnement, tirée dans la table (selon `+0x6f` de la victime, le compteur de
/// hasard et un tirage).
fn set_stun_time(g: &mut Ctx, v: u32) {
    cov!();
    let timer = (rd8(g.b, v + f::STUN_TIMER) as u16) << 8;
    let tick = ram::TICK.get(g.b) << 4;
    let roll = g.random() & 0xf;
    let index = timer.wrapping_add(tick).wrapping_add(roll);
    let time = g.data8(tables::STUN_TIMES.wrapping_add(index as i16 as i32 as u32));
    wr8(g.b, v + f::STUN_TIME, time);
}

fn enemy_reacts_native(c: &mut Cpu, b: &mut Cps2Bus) {
    let (a, v) = (Player(c.a[0]), c.a[1]);
    let is_enemy = rd8(b, v + f::CLASS) == CLASS_ENEMY;
    enemy_reacts(&mut Ctx::new(c, b), a, v);
    // l'original laisse dans A2 la table de seuils choisie ; la résolution des coups la relit
    c.a[2] = if is_enemy { tables::THRESHOLD_ENEMY } else { tables::THRESHOLD_OTHER };
    (c.a[0], c.a[1]) = (a.0, v);
    rts(c, b);
}

pub const ENEMY_REACTS: Native = Native {
    entry: 0xf232e,
    name: "enemy_reacts_to_player_hit",
    run: enemy_reacts_native,
    touches_sound: false,
    outputs: crate::regs::a(0) | crate::regs::a(1) | crate::regs::a(2),
};
