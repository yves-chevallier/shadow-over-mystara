//! Le combattant (personnages 0 et 1 : les deux variantes ont le même code).
//!
//! Quatre actions, atteintes par `characters::run` :
//! - `jump` (0x3e578) : saut en trois temps (accroupi, montée, descente) avec attaque en l'air ;
//! - `attack` (0x3e7e4) : le bouton 1 ; `Player::SUBACTION` choisit le coup (voir `kind`) ;
//! - `special` (0x3f0ae) : l'objet ou la magie choisie (`field::SPECIAL_KIND`) ;
//! - `dash` (0x3f9bc) : l'action 0x10, charge puis glissade.
//!
//! Chaque action est un automate dont le pas est `Player::STEP` (pair). Les routines d'origine qui
//! créent tirs, effets, sons et projectiles restent appelées telles quelles (voir `common::rom` et les
//! constantes `rom` ci-dessous).

use super::common::{self, call, field, next_step, rom as common_rom, set_forward_motion};
use super::items;
use crate::mem::rd32;
use super::super::actions::to_stand;
use super::super::checks::item_curse;
use super::super::combos::clear_recognizers;
use super::super::init::init_weapon_stats;
use super::super::walk::{move_xz, move_z};
use super::super::{animation, Ctx, Player};
use crate::objects::Obj;

/// Routines d'origine appelées telles quelles : effets joueur et sons du combattant.
mod rom {
    /// Création d'un effet joueur (pool PEFF) : D0 = 1 ou 0 (+ D1) selon le geste.
    pub const SLASH_EFFECT: u32 = 0x4a99a;
    /// Effet du bond tournoyant (D0 = 0).
    pub const LEAP_EFFECT: u32 = 0x4fbde;
    /// Effet du tourbillon qui coûte des points de vie.
    pub const BLOOD_EFFECT: u32 = 0x4a860;
    /// Échange d'équipement : effet joueur (D0 = 0).
    pub const SWAP_EFFECT: u32 = 0x4fd2c;
    /// Sons.
    pub const SOUND_CHARGE_A: u32 = 0xe792e;
    pub const SOUND_CHARGE_B: u32 = 0xe1fc0;
    pub const SOUND_LEAP: u32 = 0xe1f20;
    pub const SOUND_LEAP_END: u32 = 0xe7668;
    pub const SOUND_SPIN_A: u32 = 0xe2060;
    pub const SOUND_SPIN_B: u32 = 0xe828c;
    pub const SOUND_STRIKE: u32 = 0xe1de0;
}

/// Table du combattant (données de la ROM) : vitesses des sauts (quatre mots par type de saut).
const JUMP_SPEEDS: u32 = 0x10_44fe;

/// Sous-actions du bouton 1 (`Player::SUBACTION`), toutes paires.
mod kind {
    /// Coup au sol : enchaînement de coups de plus en plus longs.
    pub const COMBO: u8 = 0x00;
    /// Coup d'estoc (un ennemi est à portée de la première boîte d'attaque).
    pub const THRUST_HIGH: u8 = 0x02;
    /// Coup bas (seconde boîte d'attaque).
    pub const THRUST_LOW: u8 = 0x04;
    /// Coup donné en se retournant.
    pub const TURN_STRIKE: u8 = 0x06;
    /// Coup donné en marche lente.
    pub const SLOW_STRIKE: u8 = 0x08;
    /// Coup en l'air (retombée), suite d'un saut.
    pub const AFTER_JUMP: u8 = 0x0a;
    /// Charge : élan de 12 pixels par image suivi de deux gestes.
    pub const CHARGE: u8 = 0x0c;
    /// Coup donné en courant.
    pub const RUN_STRIKE: u8 = 0x0e;
    pub const LOW_BLOW: u8 = 0x10;
    /// Saut en arrière.
    pub const BACK_HOP: u8 = 0x12;
    pub const GUARD_BREAK: u8 = 0x14;
    /// Grand saut tournoyant (attaque spéciale du combattant).
    pub const LEAP: u8 = 0x16;
    /// Tourbillon (sacrifie des points de vie).
    pub const WHIRL: u8 = 0x18;
    /// Geste vers l'avant sans arme.
    pub const POSE_A: u8 = 0x1a;
    pub const POSE_B: u8 = 0x1c;
}

// ---- saut (0x3e578) --------------------------------------------------------------------------

/// Le saut du combattant : vitesses de décollage dans la table 0x1044fe, coup en l'air propre, retour
/// de l'arme maudite et création de l'effet joueur en attente.
const JUMP_KIT: common::JumpKit = common::JumpKit {
    speeds: JUMP_SPEEDS,
    air_attack,
    after_recover: common::curse_check,
    finish: common::flush_pending_effect,
    ..common::JumpKit::PLAIN
};

/// 0x3e578 : saut.
pub fn jump(g: &mut Ctx, p: Player) {
    cov!();
    common::jump(g, p, &JUMP_KIT);
}

/// 0x3e754 : le bouton 1 en l'air donne un coup (une fois par saut) dont l'animation dépend du type de
/// saut, de l'arme et de la direction (bas : coup plongeant).
fn air_attack(g: &mut Ctx, p: Player) {
    cov!();
    if !common::air_attack_pressed(g, p) {
        return;
    }
    p.set_u8(g, field::MOVE_STATE, 0);
    p.set_u8(g, field::AIR_ATTACK_DONE, 1);
    let heavy = common::holds_heavy_second(g, p);
    let mut anim = if heavy { 9 } else { 0x2a };
    if p.u8(g, 0x67) == 2 {
        anim = if heavy { 0xc } else { 0x2b };
    }
    if common::holds_special_weapon(g, p) {
        anim = 0x32;
    }
    if p.u8(g, Player::KEYS_HELD) & 4 != 0 {
        anim = 0xb;
    }
    common::start_with_voice(g, p, anim);
    item_curse(g, p);
    common::aura_hit_countdown(g, p);
}

// ---- attaque (0x3e7e4) -----------------------------------------------------------------------

/// 0x3e7e4 : attaque du bouton 1. `kind` (voir plus haut) choisit le coup ; les coups au sol sont suivis
/// d'un contrôle de prolongement (`common::follow_up`).
pub fn attack(g: &mut Ctx, p: Player) {
    cov!();
    let sub = p.u8(g, Player::SUBACTION);
    match sub {
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
        kind::BACK_HOP => common::back_hop(g, p, 0x1f, false),
        kind::GUARD_BREAK => guard_break(g, p),
        kind::LEAP => leap(g, p),
        kind::WHIRL => whirl(g, p),
        kind::POSE_A | kind::POSE_B => {
            common::pose(g, p, 0x1a);
            common::follow_up(g, p);
        }
        _ => {}
    }
    common::flush_pending_effect(g, p);
}

/// Avec l'arme particulière (8, 0x18) tous les coups deviennent le coup de base.
fn fall_back_to_combo(g: &mut Ctx, p: Player) -> bool {
    cov!();
    if !common::holds_special_weapon(g, p) {
        return false;
    }
    p.set_u8(g, Player::SUBACTION, kind::COMBO);
    true
}

/// Début d'un coup de base (0x3e86e) : le rang de l'enchaînement avance si le coup précédent a porté
/// (deux coups, quatre avec l'équipement lourd).
fn combo_begin(g: &mut Ctx, p: Player) {
    cov!();
    next_step(g, p);
    p.set_u8(g, field::MOVE_STATE, 0);
    p.set_u8(g, field::COMBO_RELEASED, 0);
    p.set_u8(g, field::COMBO_WINDOW, 6);
    let heavy = common::holds_heavy_second(g, p);
    let rank = common::chain_rank(g, p, 1, if heavy { 3 } else { 1 });
    let anim = if common::holds_special_weapon(g, p) { 0x30 } else { (if heavy { 0 } else { 0x26 }) + rank as u16 };
    common::start_with_voice(g, p, anim);
}

/// Coup de base (0x3e858) : trois pas.
fn combo(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => combo_begin(g, p),
        2 => {
            common::combo_follow(g, p, 0x1a);
        }
        4 => common::wait_then_stand(g, p),
        _ => {}
    }
}


/// Coups d'estoc (0x3e930 haut, 0x3e998 bas) : un seul geste, de rang 0 ou 1 selon le coup précédent.
fn thrust(g: &mut Ctx, p: Player, hit_bit: u8, base_anim: u16) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            p.set_u8(g, field::MOVE_STATE, 0);
            if fall_back_to_combo(g, p) {
                return combo_begin(g, p);
            }
            next_step(g, p);
            let rank = common::chain_rank(g, p, hit_bit, 1);
            common::start_with_voice(g, p, base_anim + rank as u16);
        }
        2 => common::wait_then_stand(g, p),
        _ => {}
    }
}

/// Coup donné en se retournant (0x3ea00) : le pas 2 est le pas 0 d'une entrée par état (combos).
fn turn_strike(g: &mut Ctx, p: Player) {
    cov!();
    let step = p.u8(g, Player::STEP);
    match step {
        0 | 2 => {
            if step == 0 {
                p.set_u8(g, field::MOVE_STATE, 0);
            }
            if fall_back_to_combo(g, p) {
                p.set_u8(g, field::HITS, 0);
                return combo_begin(g, p);
            }
            p.set_u8(g, Player::STEP, 4);
            common::start_with_voice(g, p, if step == 0 { 0x10 } else { 0x1e });
        }
        4 => common::wait_then_stand(g, p),
        _ => {}
    }
}

/// Coup donné en marche lente (0x3ea82) : sans prolongement.
fn slow_strike(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            p.set_u8(g, field::MOVE_STATE, 0);
            if fall_back_to_combo(g, p) {
                p.set_u8(g, field::HITS, 0);
                p.set_u8(g, Player::SLOW, 0);
                return combo_begin(g, p);
            }
            next_step(g, p);
            let anim = if common::holds_heavy_second(g, p) { 0x13 } else { 0x29 };
            common::start_with_voice(g, p, anim);
        }
        2 => common::wait_then_stand(g, p),
        _ => {}
    }
}

/// Coup de retombée (0x3eae6).
fn after_jump(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, field::MOVE_STATE, 0);
            let anim = if common::holds_special_weapon(g, p) { 0x31 } else { 0xd };
            common::start_with_voice(g, p, anim);
            g.rom(rom::SOUND_STRIKE, p);
        }
        2 => common::wait_then_stand(g, p),
        _ => {}
    }
}

/// Charge (0x3eb3c) : élan, premier geste (coup porté, effet), second geste puis glissade.
fn charge(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, 0x5b, 0);
            p.set_u8(g, field::MOVE_STATE, 1);
            p.set_u8(g, field::MOVE_KIND, 1);
            set_forward_motion(g, p, 0xc00, (-0x98i16) as u16);
            let anim = if common::holds_heavy_second(g, p) { 0x14 } else { 0x28 };
            animation::start_plain(g, p, anim);
            clear_recognizers(g, p);
        }
        2 => {
            animation::step(g, p);
            if p.u8(g, field::EVENT) != 0 {
                next_step(g, p);
                p.set_u8(g, field::EVENT, 0);
                common::weapon_voice(g, p);
                g.rom(rom::SOUND_CHARGE_A, p);
                g.rom(rom::SOUND_CHARGE_B, p);
            }
        }
        4 => common::charge_slide(g, p, |g, p| match p.u8(g, field::EVENT) {
            1 => {
                p.set_u8(g, field::EVENT, 0);
                call(g, p, rom::SLASH_EFFECT, &[(0, 1)]);
            }
            2 => {
                p.set_u8(g, field::EVENT, 0);
                call(g, p, rom::SLASH_EFFECT, &[(0, 0), (1, 0)]);
                if !common::holds_heavy_second(g, p) {
                    call(g, p, rom::SLASH_EFFECT, &[(0, 0), (1, 1)]);
                }
            }
            _ => {}
        }),
        _ => {}
    }
}


/// Coup donné en courant (0x3ec2c) : élan de 5 pixels par image, puis arrêt.
fn run_strike(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, field::MOVE_STATE, 0);
            set_forward_motion(g, p, 0x500, (-0x28i16) as u16);
            let anim = if common::holds_heavy_second(g, p) { 0xe } else { 0x2c };
            animation::start_plain(g, p, anim);
            common::weapon_voice(g, p);
            g.rom(rom::SOUND_CHARGE_A, p);
            clear_recognizers(g, p);
        }
        2 => {
            common::run_strike_slide(g, p, 0xf);
        }
        4 => common::wait_then_stand(g, p),
        _ => {}
    }
}



/// Coup de garde (0x3ed40).
fn guard_break(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            p.set_u8(g, field::MOVE_STATE, 0);
            if fall_back_to_combo(g, p) {
                p.set_u8(g, field::HITS, 0);
                return combo_begin(g, p);
            }
            next_step(g, p);
            common::start_with_voice(g, p, 0x12);
        }
        2 => common::wait_then_stand(g, p),
        _ => {}
    }
}

/// Grand saut tournoyant (0x3ed86), attaque spéciale 0x2d : bond, tourbillon à la verticale, retombée.
/// Avec l'arme légère (`WEAPON_B` < 0x28), saut simplifié qui commence au pas 0xe.
fn leap(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            clear_recognizers(g, p);
            if common::holds_heavy_second(g, p) {
                next_step(g, p);
                p.set_u8(g, 0x5b, 0);
                p.set_u8(g, field::MOVE_STATE, 1);
                p.set_u8(g, field::MOVE_KIND, 1);
                p.set_u16(g, Obj::VX, 0x200);
                p.set_u16(g, Obj::AX, 0x20);
                p.set_u16(g, Obj::VZ, 0x600);
                p.set_u16(g, Obj::AZ, (-0x60i16) as u16);
                p.set_u16(g, Obj::AY, 0);
                if p.u8(g, Player::FACING) == 0 {
                    common::negate_horizontal_speed(g, p);
                }
                animation::start_plain(g, p, 0x15);
            } else {
                leap_light_begin(g, p);
            }
        }
        2 => leap_spin(g, p),
        4 | 8 => leap_descend(g, p),
        6 => leap_fall(g, p),
        0xa => {
            if animation::step(g, p) {
                p.set_u8(g, Player::SLOW, 0);
                to_stand(g, p);
            }
        }
        0xc => leap_light_begin(g, p),
        0xe => leap_light_wait(g, p),
        0x10 => leap_light_land(g, p),
        0x12 => {
            if animation::step(g, p) {
                p.set_u8(g, Player::SLOW, 0);
                to_stand(g, p);
            }
        }
        _ => {}
    }
}

/// Variante légère du bond (0x3ef04) : arrivée au pas 0xe.
fn leap_light_begin(g: &mut Ctx, p: Player) {
    cov!();
    p.set_u8(g, Player::STEP, 0xe);
    p.set_u8(g, 0x5b, 0);
    p.set_u8(g, field::MOVE_STATE, 1);
    p.set_u8(g, field::MOVE_KIND, 1);
    p.set_u16(g, Obj::VX, 0x200);
    p.set_u16(g, Obj::AX, 0);
    p.set_u16(g, Obj::VZ, 0x600);
    p.set_u16(g, Obj::AZ, (-0x40i16) as u16);
    p.set_u16(g, Obj::AY, 0);
    if p.u8(g, Player::FACING) == 0 {
        common::negate_horizontal_speed(g, p);
    }
    animation::start_plain(g, p, 0x2e);
}

/// Le bond léger attend le geste (événement) puis continue l'animation (0x3ef44).
fn leap_light_wait(g: &mut Ctx, p: Player) {
    cov!();
    if p.u8(g, field::EVENT) != 0 {
        next_step(g, p);
        p.set_u8(g, field::EVENT, 0);
        call(g, p, rom::LEAP_EFFECT, &[(0, 0)]);
        g.rom(rom::SOUND_LEAP, p);
    }
    animation::step(g, p);
}

/// Atterrissage du bond léger (0x3ef66).
fn leap_light_land(g: &mut Ctx, p: Player) {
    cov!();
    move_xz(g, p);
    if (p.u16(g, Obj::Z) as i16) < 0 {
        next_step(g, p);
        p.set_u32(g, Obj::Z, 0);
        animation::start_plain(g, p, 0x2f);
        g.rom(common_rom::LANDING_SOUND, p);
        return;
    }
    animation::step(g, p);
}

/// Pas 2 du bond lourd (0x3ee00) : arrivé au cadre du sommet, le bouton 1 tenu déclenche le tourbillon
/// descendant ; le geste porté crée un effet et une voix.
fn leap_spin(g: &mut Ctx, p: Player) {
    cov!();
    let apex_frame = if p.u8(g, Obj::VARIANT) == 0 { 0x2b_8d5e } else { 0x2b_fa50 };
    if rd32(g.b, p.at(Obj::FRAME)) == apex_frame {
        if p.u8(g, Player::KEYS_HELD) & 0x10 != 0 {
            p.set_u16(g, Obj::AY, 1);
        }
        move_xz(g, p);
        if (p.u16(g, Obj::VZ) as i16) < 0 {
            if p.u16(g, Obj::AY) != 0 {
                p.set_u8(g, Player::STEP, 6);
                p.set_u16(g, Obj::AY, 0);
                p.set_u16(g, Obj::VZ, 0x600);
                animation::start_plain(g, p, 0x17);
            } else {
                next_step(g, p);
                animation::start_plain(g, p, 0x16);
            }
            return;
        }
    }
    if p.u8(g, field::EVENT) != 0 {
        p.set_u8(g, field::EVENT, 0);
        call(g, p, rom::LEAP_EFFECT, &[(0, 0)]);
        g.rom(rom::SOUND_LEAP, p);
        common::weapon_voice(g, p);
        g.rom(rom::SOUND_LEAP_END, p);
    }
    animation::step(g, p);
}

/// Pas 4 et 8 du bond lourd (0x3ee8c) : descente verticale, atterrissage au sol.
fn leap_descend(g: &mut Ctx, p: Player) {
    cov!();
    move_z(g, p);
    if (p.u16(g, Obj::Z) as i16) < 0 {
        p.set_u8(g, Player::STEP, 0xa);
        p.set_u32(g, Obj::Z, 0);
        animation::start_plain(g, p, 0x19);
        g.rom(common_rom::LANDING_SOUND, p);
        return;
    }
    animation::step(g, p);
}

/// Pas 6 du bond lourd (0x3eeb6) : second tourbillon.
fn leap_fall(g: &mut Ctx, p: Player) {
    cov!();
    move_z(g, p);
    if (p.u16(g, Obj::VZ) as i16) < 0 {
        next_step(g, p);
        animation::start_plain(g, p, 0x18);
        return;
    }
    if p.u8(g, field::EVENT) != 0 {
        p.set_u8(g, field::EVENT, 0);
        call(g, p, rom::LEAP_EFFECT, &[(0, 0)]);
        common::weapon_voice(g, p);
    }
    animation::step(g, p);
}

/// Tourbillon (0x3efa2) : coûte 12 points de vie au premier contact.
fn whirl(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, 0x5b, 0);
            p.set_u8(g, field::MOVE_STATE, 1);
            p.set_u8(g, field::MOVE_KIND, 2);
            p.set_u8(g, 0x58, 0);
            p.set_u8(g, 0x1ec, 0);
            let anim = if p.u16(g, Obj::Z) != 0 { 0x1c } else { 0x1b };
            animation::start_plain(g, p, anim);
            g.rom(rom::SOUND_SPIN_A, p);
            g.rom(rom::SOUND_SPIN_B, p);
            clear_recognizers(g, p);
        }
        2 => {
            if animation::step(g, p) {
                p.set_u8(g, Player::SLOW, 0);
                to_stand(g, p);
                return;
            }
            if p.u8(g, field::EVENT) != 0 {
                p.set_u8(g, field::EVENT, 0);
                g.rom(rom::BLOOD_EFFECT, p);
            }
            if p.u8(g, 0x1ec) == 0 && p.u8(g, 0x58) & 1 != 0 {
                p.set_u8(g, 0x1ec, 1);
                let hp = (p.u16(g, Player::HP) as i16).wrapping_sub(0xc);
                p.set_u16(g, Player::HP, if hp > 0 { hp as u16 } else { 1 });
            }
        }
        _ => {}
    }
}


// ---- attaque spéciale (0x3f0ae) --------------------------------------------------------------

/// Animations du combattant pour les magies lancées (voir `items::Style`).
const STYLE: items::Style = items::Style { cast_anim: 0x25, bolt_anim: 0x24, spark: 0, ..items::Style::CLASSIC };

/// 0x3f0ae : l'objet ou la magie choisie (`field::SPECIAL_KIND`, 0..20) ; les numéros 3 et 11 ne font
/// rien. 0..8 : objets lancés ; 12 : échange d'équipement ; 9, 10 et 13..20 : voir `items`.
pub fn special(g: &mut Ctx, p: Player) {
    cov!();
    let which = p.u8(g, field::SPECIAL_KIND);
    g.set_a5u8(0x5b, which);
    match which {
        0 => items::throw_item(g, p, 0x22, items::rom::THROW_A),
        1 => items::throw_item(g, p, 0x22, items::rom::THROW_B),
        2 => items::throw_item(g, p, 0x23, items::rom::THROW_C),
        4 => items::throw_item(g, p, 0x23, items::rom::THROW_D),
        5 => items::throw_item(g, p, 0x23, items::rom::THROW_E),
        6 => items::throw_item(g, p, 0x21, items::rom::THROW_F),
        7 => items::throw_item(g, p, 0x21, items::rom::THROW_G),
        8 => items::throw_item(g, p, 0x20, items::rom::THROW_H),
        12 => swap_equipment(g, p),
        _ => {
            items::run_shared(g, p, which, STYLE);
        }
    }
}

/// Échange d'équipement (0x3f39a) : l'objet ramassé (`field::ITEM_NEW`) remplace l'arme ou le bouclier
/// et l'ancien équipement prend sa place dans la liste d'objets (`field::ITEM`).
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
    match p.u8(g, field::EVENT) {
        1 => {
            p.set_u8(g, field::EVENT, 0);
            let new = p.u8(g, field::ITEM_NEW).wrapping_sub(0x2e);
            let second = p.u8(g, Player::WEAPON_B);
            if second <= 7 && second & 4 != 0 {
                // le bouclier en main est remplacé par l'équipement lourd 0x28 + niveau de protection
                let protection = p.u8(g, 0x164);
                p.set_u8(g, Player::WEAPON_B, 0x28u8.wrapping_add(protection));
                p.set_u8(g, field::ITEM, second.wrapping_add(0x2e));
                p.set_u8(g, Player::WEAPON_A, new);
            } else if new <= 7 && new & 4 != 0 {
                // un bouclier est enfilé : l'arme tenue devient l'objet de la liste
                let held = p.u8(g, Player::WEAPON_A);
                p.set_u8(g, field::ITEM, held.wrapping_add(0x2e));
                p.set_u8(g, Player::WEAPON_B, new);
                p.set_u8(g, Player::WEAPON_A, new.wrapping_sub(4));
            } else {
                let held = p.u8(g, Player::WEAPON_A);
                p.set_u8(g, field::ITEM, held.wrapping_add(0x2e));
                p.set_u8(g, Player::WEAPON_A, new);
            }
            init_weapon_stats(g, p);
        }
        2 => {
            p.set_u8(g, field::EVENT, 0);
            call(g, p, rom::SWAP_EFFECT, &[(0, 0)]);
        }
        _ => {}
    }
}

action_native!(JUMP, 0x3e578, "fighter_jump", jump);
action_native!(ATTACK, 0x3e7e4, "fighter_attack", attack);
action_native!(SPECIAL, 0x3f0ae, "fighter_special", special);
action_native!(DASH, 0x3f9bc, "fighter_dash", common::dash);
