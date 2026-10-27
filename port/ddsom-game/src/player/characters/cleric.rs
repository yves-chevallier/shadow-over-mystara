//! Le clerc (personnages 2 et 3 : les deux variantes ont le même code).
//!
//! Quatre actions, atteintes par `characters::run` :
//! - `jump` (0x3fd08) : le saut commun (`common::jump`) avec le coup en l'air du clerc ;
//! - `attack` (0x3ff2a) : le bouton 1 ; `Player::SUBACTION` choisit le coup (voir `kind`) ;
//! - `special` (0x406d4) : l'objet ou la prière choisie (`field::SPECIAL_KIND`) : les objets et magies
//!   communes avec le combattant (`items`) et les dix prières du clerc (numéros 0x24..0x2d) ;
//! - l'action 0x10 (0x41f22) est la même glissade que celle du combattant (`common::dash`).
//!
//! Les prières assombrissent l'écran, figent les autres joueurs et mettent le jeu en pause jusqu'à
//! leur fin (`common::begin_spell` / `end_spell`). Les routines d'origine qui créent tirs, effets et
//! sons restent appelées telles quelles.

use crate::machine::Machine;
use super::common::{self, call, field, next_step};
use super::items;
use super::super::spawn;
use super::super::actions::{start_stand, to_stand};
use super::super::combos::clear_recognizers;
use super::super::init::init_weapon_stats;
use super::super::walk::move_xz;
use super::super::{animation, ram, Ctx, Player};
use crate::mem::{wr8, A5};
use crate::objects::pools::{BOSS, ENEMY};
use crate::objects::Obj;
use crate::player::externes as ext;

/// Routines d'origine appelées telles quelles : effets joueur, sons et palettes du clerc.
mod rom {
    /// Sons des coups.
    pub const SOUND_STRIKE_B: u32 = 0xe8156;
    /// Sons du saut tournoyant.
    pub const SOUND_LEAP_LAND: u32 = 0xe2760;
    pub const SOUND_SPIN: u32 = 0xe81c8;
    /// Effets du renvoi des morts-vivants (attaque 0x18) : écran, palettes, sons.
    pub const TURN_EFFECTS: [u32; 5] = [0x4b518, 0x3b0c, 0x1ac86, 0x51bb6, 0xe27da];
    /// Prières : effets joueur (pool PEFF) créés au geste, avec leurs sons.
    pub const PRAYER_EVENT_SOUND: u32 = 0xe2aba;
    pub const PRAYER_AURA_SOUND: u32 = 0xe96e0;
    pub const PRAYER_AURA_B_SOUND: u32 = 0xe9030;
    pub const PRAYER_STORM_SOUND: u32 = 0xe2afa;
    pub const PRAYER_SWARM_START_SOUND: u32 = 0xe2bba;
    pub const SOUND_PRAYER: u32 = 0xe2c7a;
    pub const PRAYER_BANISH_SOUND: u32 = 0xe9e34;
    pub const PRAYER_BANISH_START_SOUND: u32 = 0xe2cba;
    pub const PRAYER_HOLY_SOUND: u32 = 0xe2cfa;
    pub const PRAYER_BLESS_SOUND: u32 = 0xe8e1e;
    pub const PRAYER_BLESS_SOUND_AREA: u32 = 0xe285c;
    pub const PRAYER_BLESS_SOUND_SELF: u32 = 0xe2b3a;
    pub const PRAYER_PROTECT_SOUND: u32 = 0xe2c3a;
    pub const PRAYER_HEAL_SOUND_SELF_A: u32 = 0xe2b7a;
    pub const PRAYER_HEAL_SOUND_SELF_B: u32 = 0xe2bfa;
}

/// Table du clerc (données de la ROM) : vitesses des sauts (quatre mots par type de saut).
const JUMP_SPEEDS: u32 = 0x10_451e;

/// Sous-actions du bouton 1 (`Player::SUBACTION`), toutes paires.
mod kind {
    pub const COMBO: u8 = 0x00;
    pub const THRUST_HIGH: u8 = 0x02;
    pub const THRUST_LOW: u8 = 0x04;
    pub const TURN_STRIKE: u8 = 0x06;
    pub const SLOW_STRIKE: u8 = 0x08;
    pub const AFTER_JUMP: u8 = 0x0a;
    pub const CHARGE: u8 = 0x0c;
    pub const RUN_STRIKE: u8 = 0x0e;
    pub const LOW_BLOW: u8 = 0x10;
    pub const BACK_HOP: u8 = 0x12;
    pub const GUARD_BREAK: u8 = 0x14;
    /// Saut tournoyant (attaque spéciale 0x2d).
    pub const LEAP: u8 = 0x16;
    /// Prière de renvoi des morts-vivants.
    pub const TURN_UNDEAD: u8 = 0x18;
    pub const POSE_A: u8 = 0x1a;
    pub const POSE_B: u8 = 0x1c;
}

// ---- saut (0x3fd08) --------------------------------------------------------------------------

/// Le saut du clerc : vitesses de décollage dans la table 0x10451e, coup en l'air sans arme maudite.
const JUMP_KIT: common::JumpKit = common::JumpKit {
    speeds: JUMP_SPEEDS,
    air_attack,
    ..common::JumpKit::PLAIN
};

/// 0x3fd08 : saut.
pub fn jump(g: &mut Ctx, p: Player) {
    cov!();
    common::jump(g, p, &JUMP_KIT);
}

/// 0x3feca : le bouton 1 en l'air donne un coup (une fois par saut) ; bas : coup plongeant.
fn air_attack(g: &mut Ctx, p: Player) {
    cov!();
    if !common::air_attack_pressed(g, p) {
        return;
    }
    p.set_u8(g, field::MOVE_STATE, 0);
    p.set_u8(g, field::AIR_ATTACK_DONE, 1);
    let mut anim = if p.u8(g, 0x67) == 2 { 0xc } else { 9 };
    if p.u8(g, Player::KEYS_HELD) & 4 != 0 {
        anim = 0xb;
    }
    common::start_with_voice(g, p, anim);
    common::aura_hit_countdown(g, p);
}

// ---- attaque (0x3ff2a) -----------------------------------------------------------------------

/// 0x3ff2a : attaque du bouton 1. Les coups au sol sont suivis d'un contrôle de prolongement.
pub fn attack(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::SUBACTION) {
        kind::COMBO => {
            combo(g, p);
            common::follow_up(g, p);
        }
        kind::THRUST_HIGH => {
            thrust(g, p, 1 << 1, 5);
            common::follow_up(g, p);
        }
        kind::THRUST_LOW => {
            thrust(g, p, 1 << 2, 7);
            common::follow_up(g, p);
        }
        kind::TURN_STRIKE => {
            turn_strike(g, p);
            common::follow_up(g, p);
        }
        kind::SLOW_STRIKE => slow_strike(g, p),
        kind::AFTER_JUMP => {
            after_jump(g, p);
            common::follow_up(g, p);
        }
        kind::CHARGE => charge(g, p),
        kind::RUN_STRIKE => run_strike(g, p),
        kind::LOW_BLOW => {
            common::single_strike(g, p, 0x11);
            common::follow_up(g, p);
        }
        kind::BACK_HOP => common::back_hop(g, p, 0x1c, true),
        kind::GUARD_BREAK => guard_break(g, p),
        kind::LEAP => leap(g, p),
        kind::TURN_UNDEAD => turn_undead(g, p),
        kind::POSE_A | kind::POSE_B => {
            common::pose(g, p, 0x17);
            common::follow_up(g, p);
        }
        _ => {}
    }
}

/// Coup de base (0x3ff8c) : jusqu'à cinq coups enchaînés, l'animation suit le rang.
fn combo(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, field::MOVE_STATE, 0);
            p.set_u8(g, field::COMBO_RELEASED, 0);
            p.set_u8(g, field::COMBO_WINDOW, 6);
            let rank = common::chain_rank(g, p, 1, 4);
            common::start_with_voice(g, p, rank as u16);
        }
        2 => {
            common::combo_follow(g, p, 0x17);
        }
        4 => common::wait_then_stand(g, p),
        _ => {}
    }
}

/// Coups d'estoc haut (0x4003c) et bas (0x4008e) : un geste, de rang 0 ou 1.
fn thrust(g: &mut Ctx, p: Player, hit_bit: u8, base_anim: u16) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, field::MOVE_STATE, 0);
            let rank = common::chain_rank(g, p, hit_bit, 1);
            common::start_with_voice(g, p, base_anim + rank as u16);
        }
        2 => common::wait_then_stand(g, p),
        _ => {}
    }
}

/// Coup donné en se retournant (0x400e0) : le pas 2 est le pas 0 d'une entrée par état (combos).
fn turn_strike(g: &mut Ctx, p: Player) {
    cov!();
    let step = p.u8(g, Player::STEP);
    match step {
        0 | 2 => {
            if step == 0 {
                p.set_u8(g, field::MOVE_STATE, 0);
            }
            p.set_u8(g, Player::STEP, 4);
            common::start_with_voice(g, p, if step == 0 { 0x10 } else { 0x1b });
        }
        4 => common::wait_then_stand(g, p),
        _ => {}
    }
}

/// Coup donné en marche lente (0x4012c) : sans prolongement.
fn slow_strike(g: &mut Ctx, p: Player) {
    cov!();
    common::single_strike(g, p, 0x13);
}

/// Coup de retombée (0x40166) : la masse d'armes (arme 0x1e) a son propre geste.
fn after_jump(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, field::MOVE_STATE, 0);
            if p.u8(g, Player::WEAPON_A) == 0x1e {
                animation::start_plain(g, p, 0x18);
                clear_recognizers(g, p);
                g.rom(rom::SOUND_STRIKE_B, p);
            } else {
                animation::start_plain(g, p, 0xd);
                clear_recognizers(g, p);
                common::weapon_voice(g, p);
            }
            g.rom(ext::son::STRIKE_LIGHT, p);
        }
        2 => common::wait_then_stand(g, p),
        _ => {}
    }
}

/// Charge (0x401c4) : élan de 12 pixels par image, geste, deux effets, glissade.
fn charge(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, 0x5b, 0);
            p.set_u8(g, field::MOVE_STATE, 1);
            p.set_u8(g, field::MOVE_KIND, 1);
            common::set_forward_motion(g, p, 0xc00, (-0x98i16) as u16);
            animation::start_plain(g, p, 0x14);
            clear_recognizers(g, p);
        }
        2 => {
            animation::step(g, p);
            if p.u8(g, field::EVENT) != 0 {
                next_step(g, p);
                p.set_u8(g, field::EVENT, 0);
                g.rom(ext::son::CHARGE_A, p);
                common::weapon_voice(g, p);
                g.rom(ext::son::CHARGE_B, p);
            }
        }
        4 => common::charge_slide(g, p, |g, p| {
            if p.u8(g, field::EVENT) != 0 {
                p.set_u8(g, field::EVENT, 0);
                spawn::slash(g, p, Some(0));
                spawn::slash(g, p, None);
            }
        }),
        _ => {}
    }
}

/// Coup donné en courant (0x4028a).
fn run_strike(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, field::MOVE_STATE, 0);
            common::set_forward_motion(g, p, 0x500, (-0x28i16) as u16);
            animation::start_plain(g, p, 0xe);
            clear_recognizers(g, p);
            common::weapon_voice(g, p);
            g.rom(ext::son::CHARGE_A, p);
        }
        2 => {
            common::run_strike_slide(g, p, 0xf);
        }
        4 => common::wait_then_stand(g, p),
        _ => {}
    }
}

/// Coup de garde (0x4039e).
fn guard_break(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, field::MOVE_STATE, 0);
            animation::start_plain(g, p, 0x12);
        }
        2 => common::wait_then_stand(g, p),
        _ => {}
    }
}

/// Saut tournoyant (0x403be) : bond, tournoyer au sommet, retombée. Avec la masse d'armes (arme 0x1e)
/// c'est une série de cinq coups sur place (pas 8 à 0xa).
fn leap(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            if p.u8(g, Player::WEAPON_A) == 0x1e {
                leap_mace_begin(g, p);
                return;
            }
            next_step(g, p);
            p.set_u8(g, 0x5b, 0);
            p.set_u8(g, field::MOVE_STATE, 1);
            p.set_u8(g, field::MOVE_KIND, 1);
            p.set_u32(g, Obj::VX, 0x0080_0000);
            p.set_u16(g, Obj::VZ, 0x560);
            p.set_u16(g, Obj::AZ, (-0x38i16) as u16);
            if p.u8(g, Player::FACING) == 0 {
                common::negate_horizontal_speed(g, p);
            }
            common::start_with_voice(g, p, 0x15);
            clear_recognizers(g, p);
        }
        2 => {
            if animation::step(g, p) {
                next_step(g, p);
                animation::start_plain(g, p, 0x16);
                return;
            }
            if p.u8(g, field::EVENT) != 0 {
                p.set_u8(g, field::EVENT, 0);
                g.rom(ext::son::LEAP, p);
            }
        }
        4 => {
            move_xz(g, p);
            animation::step(g, p);
            if (p.u16(g, Obj::VZ) as i16) < 0 && (p.u16(g, Obj::Z) as i16) < 0 {
                p.set_u32(g, Obj::Z, 0);
                p.set_u8(g, Player::SLOW, 0);
                to_stand(g, p);
                return;
            }
            if p.u8(g, field::EVENT) != 0 {
                p.set_u8(g, field::EVENT, 0);
                g.rom(rom::SOUND_LEAP_LAND, p);
                common::weapon_voice(g, p);
            }
        }
        6 => leap_mace_begin(g, p),
        8 => {
            animation::step(g, p);
            if p.u8(g, field::EVENT) == 1 {
                p.set_u8(g, field::EVENT, 0);
                g.rom(rom::SOUND_SPIN, p);
            }
            if p.u8(g, field::EVENT) == 2 {
                p.set_u8(g, field::EVENT, 0);
                let strokes = p.u8(g, 0x1e8) + 1;
                p.set_u8(g, 0x1e8, strokes);
                if strokes == 5 {
                    next_step(g, p);
                    animation::start_plain(g, p, 0x1a);
                }
            }
        }
        0xa => {
            if animation::step(g, p) {
                p.set_u8(g, Player::SLOW, 0);
                to_stand(g, p);
            }
        }
        _ => {}
    }
}

/// Début des cinq coups de masse sur place (0x40492).
fn leap_mace_begin(g: &mut Ctx, p: Player) {
    cov!();
    p.set_u8(g, Player::STEP, 8);
    p.set_u8(g, 0x5b, 0);
    p.set_u8(g, field::MOVE_STATE, 1);
    p.set_u8(g, field::MOVE_KIND, 1);
    p.set_u8(g, 0x1e8, 0);
    animation::start_plain(g, p, 0x19);
}

/// Table (octets du code, 0x4054e) : deux octets par niveau, nombre de morts-vivants renvoyés par la
/// prière (monstres de type 1 et de type 5).
const UNDEAD_COUNTS: u32 = 0x4054e;

/// Renvoi des morts-vivants (0x40506) : le clerc prie, l'écran clignote, puis les morts-vivants à
/// l'écran (types 1 et 5) fuient, en nombre limité par le niveau, sauf si le clerc a un talisman
/// (`+0x165` = 3, aucun plafond).
fn turn_undead(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            let level = 2 * p.u8(g, Player::LEVEL) as u32;
            let (first, second) = (g.prog8(UNDEAD_COUNTS + level), g.prog8(UNDEAD_COUNTS + level + 1));
            let (first, second) = if p.u8(g, 0x165) == 3 { (0xff, 0xff) } else { (first, second) };
            p.set_u8(g, 0x1f0, first);
            p.set_u8(g, 0x1f1, second);
            animation::start_plain(g, p, 0x21);
        }
        2 => {
            if p.u8(g, field::EVENT) != 0 {
                next_step(g, p);
                p.set_u8(g, Player::ACTIVE, 2);
                if ram::INPUT_MASK.get(g.b) & 0x40 != 0 {
                    ram::INPUT_MASK.set(g.b, 0x7f);
                }
                p.set_u8(g, 0x1ea, 4);
                p.set_u8(g, 0x1eb, 3);
                p.set_u8(g, 0x1ec, 1);
                ram::SCROLL1_Y.set(g.b, 0x100);
                for addr in rom::TURN_EFFECTS {
                    g.rom(addr, p);
                }
            }
            animation::step(g, p);
        }
        4 => {
            let blink = p.u8(g, 0x1ea).wrapping_sub(1);
            p.set_u8(g, 0x1ea, blink);
            if blink != 0 {
                return;
            }
            p.set_u8(g, 0x1ea, 4);
            ram::SCROLL1_Y.set(g.b, 0);
            let lit = p.u8(g, 0x1ec) ^ 1;
            p.set_u8(g, 0x1ec, lit);
            if lit != 0 {
                ram::SCROLL1_Y.set(g.b, 0x100);
            }
            let flashes = p.u8(g, 0x1eb).wrapping_sub(1);
            p.set_u8(g, 0x1eb, flashes);
            if flashes != 0 {
                return;
            }
            next_step(g, p);
            p.set_u8(g, Player::ACTIVE, 1);
            if ram::INPUT_MASK.get(g.b) & 0x40 != 0 {
                ram::INPUT_MASK.set(g.b, 0xff);
            }
            ram::SCROLL1_Y.set(g.b, 0);
            scare_undead(g, p);
        }
        6 => common::wait_then_stand(g, p),
        _ => {}
    }
}

// ---- attaque spéciale (0x406d4) --------------------------------------------------------------

/// Animations du clerc pour les magies lancées (voir `items::Style`).
const STYLE: items::Style = items::Style { cast_anim: 0x20, bolt_anim: 0x1f, spark: 1, ..items::Style::CLASSIC };

/// 0x406d4 : l'objet ou la prière choisie (`field::SPECIAL_KIND`). Les numéros 0..5, 11 et 21..35
/// n'existent pas pour le clerc : retour à l'état immobile.
pub fn special(g: &mut Ctx, p: Player) {
    cov!();
    let which = p.u8(g, field::SPECIAL_KIND);
    ram::SPELL_ROW.set(g.b, which);
    match which {
        6 => items::throw_item(g, p, 0x1e, spawn::Thrown::F),
        7 => items::throw_item(g, p, 0x1e, spawn::Thrown::G),
        8 => items::throw_item(g, p, 0x1d, spawn::Thrown::H),
        12 => swap_equipment(g, p),
        0x24 => prayer_hold(g, p),
        0x25 => prayer_storm(g, p),
        0x26 => prayer_swarm(g, p),
        0x27 => prayer_beam(g, p),
        0x28 => prayer_banish(g, p),
        0x29 => prayer_holy_light(g, p),
        0x2a => prayer_protection(g, p),
        0x2b => prayer_blessing(g, p),
        0x2c => prayer_heal(g, p, 0),
        0x2d => prayer_heal(g, p, 1),
        _ => {
            if !items::run_shared(g, p, which, STYLE) {
                to_stand(g, p);
            }
        }
    }
}

/// Échange d'équipement (0x40944) : l'objet ramassé remplace l'arme ; l'ancienne arme prend sa place
/// dans la liste d'objets.
fn swap_equipment(g: &mut Ctx, p: Player) {
    cov!();
    if p.u8(g, Player::STEP) == 0 {
        next_step(g, p);
        animation::start(g, p, 0x32);
        return;
    }
    if animation::step(g, p) {
        to_stand(g, p);
        return;
    }
    if p.u8(g, field::EVENT) != 0 {
        p.set_u8(g, field::EVENT, 0);
        let new = p.u8(g, field::ITEM_NEW).wrapping_sub(0x2e);
        let held = p.u8(g, Player::WEAPON_A);
        p.set_u8(g, field::ITEM, held.wrapping_add(0x2e));
        p.set_u8(g, Player::WEAPON_A, new);
        init_weapon_stats(g, p);
    }
}

// ---- renvoi des morts-vivants : effet sur les monstres (0x40648) -----------------------------------

/// Les 23 premiers emplacements de la zone des ennemis (les 22 ennemis puis le premier chef) : un monstre
/// de type 1 en jeu, ou un emplacement de type 5, reçoit la marque `+0x8f` : 1 tant que le compteur de
/// renvois (`field` 0x1f0 pour le type 1, 0x1f1 pour le type 5) n'est pas épuisé (ou avec le talisman),
/// sinon 2 (3 à partir du niveau 10).
fn scare_undead(g: &mut Ctx, p: Player) {
    cov!();
    let level = p.u8(g, Player::LEVEL);
    let talisman = p.u8(g, 0x165) == 3;
    for k in 0..23 {
        let o = Obj(A5 + 0x299e + 0xc0 * k);
        let in_play = o.u8(g.b, 0) != 0 && o.u8(g.b, 1) & 1 != 0;
        let kind = o.u8(g.b, Obj::KIND);
        let counter = if in_play && kind == 1 {
            0x1f0
        } else if kind == 5 {
            0x1f1
        } else {
            continue;
        };
        let left = p.u8(g, counter);
        let mark = if talisman {
            1
        } else if left != 0 {
            p.set_u8(g, counter, left - 1);
            1
        } else if level >= 0xa {
            3
        } else {
            2
        };
        o.set_u8(g.b, 0x8f, mark);
    }
}

// ---- prières -----------------------------------------------------------------------------------

/// Début commun des prières : pas suivant, pause, indicateur de sort, durée, une charge de la réserve
/// (`reserve` : octet de `+0x152..+0x157`), orientation, gel des autres joueurs.
fn begin_prayer(g: &mut Ctx, p: Player, timer: Option<u16>, reserve: u32) {
    cov!();
    common::begin_spell(g, p, timer);
    let left = p.u8(g, reserve).wrapping_sub(1);
    p.set_u8(g, reserve, left);
    common::face_held_direction(g, p);
    common::freeze_others(g, p);
}

/// Une image de décompte : `field::TIMER` diminue ; vrai quand il atteint zéro.
fn countdown(g: &mut Ctx, p: Player) -> bool {
    let left = p.u16(g, field::TIMER).wrapping_sub(1);
    p.set_u16(g, field::TIMER, left);
    left == 0
}

/// Prière annulée (une protection, `SPELL_CANCELLED`) : le décompte passe à 16 images et l'étincelle
/// d'échec de genre `spark` est jouée.
fn cancel_prayer(g: &mut Ctx, p: Player, spark: u16) {
    cov!();
    p.set_u16(g, field::TIMER, 0x10);
    common::item_spark(g, p, spark);
}

/// Table (octets du code) : nombre maximum de monstres touchés par niveau, par prière.
mod counts {
    pub const HOLD: u32 = 0x4131e;
    pub const CURE: u32 = 0x40ece;
    pub const BANISH: u32 = 0x41afe;
}

/// Table (mots du code, 0x410a6) : durée du bonus de la bénédiction par niveau, et (0x41e4c) durée de
/// l'aura de protection.
mod durations {
    pub const BLESSING: u32 = 0x410a6;
    pub const PROTECTION: u32 = 0x41e4c;
}

/// 0x4119c, n° 0x24 : aura sacrée qui immobilise des monstres. L'animation 0x2a prépare, 0x2b prie ; au
/// geste l'aura est créée, puis les premiers monstres désignés par `hold_foes` sont figés.
fn prayer_hold(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            begin_prayer(g, p, Some(0x40), 0x152);
            common::dim_screen(g, p);
            animation::start_plain(g, p, 0x2a);
        }
        2 => {
            if countdown(g, p) {
                next_step(g, p);
                p.set_u16(g, field::TIMER, 0x40);
                animation::start_plain(g, p, 0x2b);
                return;
            }
            if p.u8(g, field::EVENT) != 0 {
                p.set_u8(g, field::EVENT, 0);
                spawn::aura(g, p, 0);
                crate::placement::load_effect_palette_68k(g.c, g.b, crate::placement::effect_palette::LIGHT);
                g.rom(rom::PRAYER_AURA_SOUND, p);
            } else {
                animation::step(g, p);
            }
        }
        4 => {
            if countdown(g, p) {
                next_step(g, p);
                p.set_u16(g, field::TIMER, 1);
            }
            if p.u8(g, field::EVENT) != 0 {
                g.rom(rom::PRAYER_EVENT_SOUND, p);
                if p.u8(g, field::SPELL_CANCELLED) != 0 {
                    next_step(g, p);
                    cancel_prayer(g, p, 0x12);
                    return;
                }
                p.set_u8(g, field::EVENT, 0);
                spawn::aura(g, p, 1);
                g.rom(rom::PRAYER_AURA_B_SOUND, p);
                hold_foes(g, p);
            }
            animation::step(g, p);
        }
        6 => {
            if countdown(g, p) {
                common::end_spell(g, p);
            }
        }
        _ => {}
    }
}

/// 0x412a8 : désigne les premiers monstres (chefs puis ennemis) qui peuvent être touchés : en jeu,
/// vivants, non protégés (bit 15 de `+0x94`), pas déjà figés (`+0x49` = 0x9c). Leur nombre maximum est lu
/// par niveau dans une table du code. Ils passent à la réaction 0x9c, attribuée au clerc.
fn hold_foes(g: &mut Ctx, p: Player) {
    cov!();
    let level = p.u8(g, Player::LEVEL) as u32;
    let mut left = g.prog8(counts::HOLD + level);
    let (index, character) = (p.u8(g, Player::INDEX), p.u8(g, Player::CHARACTER));
    for pool in [&BOSS, &ENEMY] {
        for o in pool.iter() {
            if o.u8(g.b, 0) == 0
                || o.u8(g.b, 1) & 1 == 0
                || o.u16(g.b, 0x62) == 0
                || o.u8(g.b, 0x40) != 0
                || o.u8(g.b, 0x49) == 0x9c
                || o.u32(g.b, 0x94) >> 15 & 1 != 0
            {
                continue;
            }
            o.set_u32(g.b, 4, 0x0400_0000);
            o.set_u8(g.b, 0x49, 0x9c);
            o.set_u8(g.b, 0x48, index);
            o.set_u8(g.b, 0x4b, character);
            o.set_u16(g.b, 0x54, p.0 as u16);
            left = left.wrapping_sub(1);
            if left == 0 {
                break;
            }
        }
    }
}

/// 0x417c2, n° 0x25 : tempête. Un seul geste ; l'effet (0x4ec66) naît au premier événement.
fn prayer_storm(g: &mut Ctx, p: Player) {
    cov!();
    if p.u8(g, Player::STEP) == 0 {
        begin_prayer(g, p, Some(0x60), 0x153);
        common::dim_screen(g, p);
        animation::start_plain(g, p, 0x28);
        return;
    }
    if countdown(g, p) {
        let flags = p.u8(g, Player::FLAGS) & !2;
        p.set_u8(g, Player::FLAGS, flags);
        ram::SCROLL1_Y.set(g.b, 0);
        common::end_spell(g, p);
        return;
    }
    animation::step(g, p);
    if p.u8(g, field::EVENT) == 1 {
        p.set_u8(g, field::EVENT, 0);
        if p.u8(g, field::SPELL_CANCELLED) != 0 {
            cancel_prayer(g, p, 0x16);
            return;
        }
        spawn::storm(g, p, 0);
    }
    if p.u16(g, field::TIMER) == 0x20 {
        g.rom(rom::PRAYER_STORM_SOUND, p);
    }
}

/// 0x40e2e, n° 0x26 : essaim. Le geste 0x2c prépare, 0x2d lance autant d'effets que le niveau le permet
/// (0x4f272) ; au premier événement un effet de soin est créé.
fn prayer_swarm(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            begin_prayer(g, p, Some(0x40), 0x154);
            common::dim_screen(g, p);
            animation::start_plain(g, p, 0x2c);
        }
        2 => {
            if countdown(g, p) {
                next_step(g, p);
                if p.u8(g, field::SPELL_CANCELLED) == 0 {
                    let level = p.u8(g, Player::LEVEL) as u32;
                    let count = g.prog8(counts::CURE + level).wrapping_sub(1);
                    spawn::swarm(g, p, count as u32 + 1);
                }
                animation::start_plain(g, p, 0x2d);
                g.rom(rom::PRAYER_SWARM_START_SOUND, p);
                return;
            }
            if p.u8(g, field::EVENT) != 0 {
                p.set_u8(g, field::EVENT, 0);
                spawn::beam(g, p, false);
                crate::placement::load_effect_palette_68k(g.c, g.b, crate::placement::effect_palette::NORMAL);
                g.rom(ext::son::PRAYER_QUAKE, p);
            }
            animation::step(g, p);
        }
        4 => {
            if !animation::step(g, p) {
                return;
            }
            if p.u8(g, field::SPELL_CANCELLED) != 0 {
                next_step(g, p);
                cancel_prayer(g, p, 0xf);
                prayer_finish_timer(g, p);
            } else {
                common::end_spell_keep_cancel(g, p);
            }
        }
        6 => prayer_finish_timer(g, p),
        _ => {}
    }
}

/// Pas final commun : à la fin du décompte, la prière se termine.
fn prayer_finish_timer(g: &mut Ctx, p: Player) {
    cov!();
    if countdown(g, p) {
        common::end_spell(g, p);
    }
}

/// 0x4187e, n° 0x27 : rayon sacré. Le geste 0x30 prie ; au geste un effet de soin naît, puis le rayon
/// (0x4b5ac, D0 = 1) vise un monstre, mémorisé dans `SPELL_TARGET`, qui reçoit le drapeau `+0x5e` à la fin.
fn prayer_beam(g: &mut Ctx, p: Player) {
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
                crate::placement::load_effect_palette_68k(g.c, g.b, crate::placement::effect_palette::NORMAL);
                g.rom(ext::son::PRAYER_QUAKE, p);
            }
            if !countdown(g, p) {
                return;
            }
            next_step(g, p);
            if p.u8(g, field::SPELL_CANCELLED) == 0 {
                spawn::beam(g, p, true);
                p.set_u16(g, field::SPELL_TARGET, g.c.a[1] as u16);
            }
        }
        4 => {
            next_step(g, p);
            animation::start_plain(g, p, 0x31);
            g.rom(rom::SOUND_PRAYER, p);
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

/// 0x419b8, n° 0x28 : bannissement. Au geste (0x2e) un effet est créé ; à la fin du décompte les
/// premiers monstres désignés par `banish_foes` sont chassés (0x4c26c) ; sans cible le sort est écourté.
fn prayer_banish(g: &mut Ctx, p: Player) {
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
                    crate::placement::load_effect_palette_68k(g.c, g.b, crate::placement::effect_palette::LIGHT);
                    g.rom(rom::PRAYER_BANISH_SOUND, p);
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
            g.rom(rom::PRAYER_BANISH_START_SOUND, p);
        }
        4 => prayer_finish_timer(g, p),
        _ => {}
    }
}

/// 0x41aa4 : désigne les premiers monstres vivants non protégés (bit 19 de `+0x94`) et leur applique
/// l'effet 0x4c26c ; leur nombre maximum dépend du niveau. Renvoie le nombre de monstres touchés. Comme
/// l'original, le plafond n'arrête que la zone en cours (les ennemis sont toujours parcourus).
fn banish_foes(g: &mut Ctx, p: Player) -> u8 {
    cov!();
    let mut found = 0u8;
    let level = p.u8(g, Player::LEVEL) as u32;
    let mut left = g.prog8(counts::BANISH + level);
    for pool in [&BOSS, &ENEMY] {
        for o in pool.iter() {
            if o.u8(g.b, 0) == 0
                || o.u8(g.b, 1) & 1 == 0
                || o.u16(g.b, 0x62) == 0
                || o.u8(g.b, 0x40) != 0
                || o.u32(g.b, 0x94) >> 19 & 1 != 0
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

/// 0x41b26, n° 0x29 : lumière sacrée. Le geste 0x32 prie ; l'effet (0x4aeb2) naît au geste ; quand le
/// décompte (posé par l'effet) est non nul, `holy_damage` frappe les monstres.
fn prayer_holy_light(g: &mut Ctx, p: Player) {
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
                    g.rom(rom::PRAYER_HOLY_SOUND, p);
                }
            } else if p.u8(g, field::EVENT) != 0 {
                p.set_u8(g, field::EVENT, 0);
                spawn::holy_light(g, p);
                crate::placement::load_effect_palette_68k(g.c, g.b, crate::placement::effect_palette::LIGHT);
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

/// Tables (octets de la ROM, données) : dégâts de la lumière sacrée par niveau, pour les chefs et pour
/// les ennemis.
mod damage {
    pub const BOSS: u32 = 0x10_433c;
    pub const ENEMY: u32 = 0x10_431e;
}

/// 0x41c30 : la lumière sacrée blesse tous les monstres en jeu au sol non protégés (bit 20 de `+0x94`) :
/// ils passent à la réaction 0x10 (attribuée au clerc), perdent les points de vie de la table (+25 % avec
/// l'équipement `+0x166` = 4), les monstres de type 0 ou 0x13 non chefs meurent sur le coup. La barre
/// de vie est mise à jour.
fn holy_damage(g: &mut Ctx, p: Player) {
    cov!();
    let level = p.u8(g, Player::LEVEL) as u32;
    let (index, character) = (p.u8(g, Player::INDEX), p.u8(g, Player::CHARACTER));
    for (pool, table) in [(&BOSS, damage::BOSS), (&ENEMY, damage::ENEMY)] {
        for o in pool.iter() {
            if o.u8(g.b, 0) == 0
                || o.u8(g.b, 1) & 1 == 0
                || o.u16(g.b, 0x62) == 0
                || o.u8(g.b, 0x40) != 0
                || o.u16(g.b, 0x0c) != 0
                || o.u32(g.b, 0x94) >> 20 & 1 != 0
            {
                continue;
            }
            o.set_u32(g.b, 4, 0x0400_0000);
            o.set_u8(g.b, 1, 1);
            o.set_u8(g.b, 0x48, index);
            o.set_u8(g.b, 0x49, 0x10);
            o.set_u8(g.b, 0x4b, character);
            o.set_u16(g.b, 0x54, p.0 as u16);
            o.set_u8(g.b, 0x2c, 0);
            if g.random() & 1 != 0 {
                o.set_u8(g.b, 0x2c, 0x20);
            }
            let facing = o.u8(g.b, 0x2c);
            o.set_u8(g.b, 0x4a, facing);
            let counter = p.u16(g, 0x190).wrapping_add(2);
            p.set_u16(g, 0x190, counter);
            let hits = o.u8(g.b, 0x9c).wrapping_add(1);
            o.set_u8(g.b, 0x9c, hits);
            let mut damage = g.data8(table + level) as u16;
            if p.u8(g, 0x166) == 4 {
                damage = damage.wrapping_add(damage >> 2);
            }
            let hp = (o.u16(g.b, 0x62) as i16).wrapping_sub(damage as i16);
            o.set_u16(g.b, 0x62, if hp < 0 { 0 } else { hp as u16 });
            if o.u8(g.b, 0x28) != 4 && matches!(o.u8(g.b, Obj::KIND), 0 | 0x13) {
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
        }
    }
}

/// 0x41d48, n° 0x2a : protection. Une aura de protection (durée par niveau) est posée sur tous les
/// joueurs en vie à la fin du sort.
fn prayer_protection(g: &mut Ctx, p: Player) {
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
                crate::placement::load_effect_palette_68k(g.c, g.b, crate::placement::effect_palette::LIGHT);
                g.rom(rom::PRAYER_BLESS_SOUND, p);
                g.rom(rom::PRAYER_PROTECT_SOUND, p);
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

/// 0x41de4 : fin de la protection : l'aura (`AURA_TIMER` = durée par niveau, `AURA_C` = 1) est posée sur
/// chaque joueur en vie qui n'est pas bloqué par un objet.
fn protect_players(g: &mut Ctx, p: Player) {
    cov!();
    common::clear_spell_state(g);
    p.set_u8(g, field::SPELL_CANCELLED, 0);
    let level = p.u8(g, Player::LEVEL) as u32;
    let duration = g.prog16(durations::PROTECTION + 2 * level);
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

/// 0x40f6c, n° 0x2b : bénédiction. Le clerc (ou le premier allié à portée) reçoit un bonus de puissance
/// temporaire (`POWERUP_TIMER`, durée par niveau) ; sans allié le clerc se bénit lui-même.
fn prayer_blessing(g: &mut Ctx, p: Player) {
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
                crate::placement::load_effect_palette_68k(g.c, g.b, crate::placement::effect_palette::LIGHT);
                g.rom(rom::PRAYER_BLESS_SOUND, p);
                g.rom(if area != 0 { rom::PRAYER_BLESS_SOUND_AREA } else { rom::PRAYER_BLESS_SOUND_SELF }, p);
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

/// 0x410d8 : choisit l'allié béni (voir `common::find_ally`, portée 0x4c) : il passe à la pose de
/// bénédiction (état 0x02001e00) et reçoit la durée de bonus ; sans allié le clerc reçoit le bonus.
fn bless_ally(g: &mut Ctx, p: Player) {
    cov!();
    let level = p.u8(g, Player::LEVEL) as u32;
    let duration = g.prog16(durations::BLESSING + 2 * level);
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

/// 0x4101a : fin de la bénédiction : levée de la pause et du gel ; les joueurs qui ont une durée de bonus
/// reçoivent le drapeau de bonus (`POWERUP`).
fn end_blessing(g: &mut Ctx, p: Player) {
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

/// 0x41346 (n° 0x2c) et 0x4149c (n° 0x2d) : soin. `variant` 0 ou 1. Le clerc (ou l'allié à portée, voir
/// `common::heal_scan`) est soigné de `common::apply_heal` à la fin.
fn prayer_heal(g: &mut Ctx, p: Player, variant: u32) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            begin_prayer(g, p, Some(0x60), 0x154 + variant);
            p.set_u8(g, 0x1ed, 0);
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
                crate::placement::load_effect_palette_68k(g.c, g.b, crate::placement::effect_palette::NORMAL);
                let sound = if p.u8(g, field::AREA) != 0 {
                    rom::PRAYER_BLESS_SOUND_AREA
                } else if variant == 0 {
                    rom::PRAYER_HEAL_SOUND_SELF_A
                } else {
                    rom::PRAYER_HEAL_SOUND_SELF_B
                };
                g.rom(sound, p);
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

action_native!(JUMP, 0x3fd08, "cleric_jump", jump);
action_native!(ATTACK, 0x3ff2a, "cleric_attack", attack);
action_native!(SPECIAL, 0x406d4, "cleric_special", special);
action_native!(DASH, 0x41f22, "cleric_dash", common::dash);
