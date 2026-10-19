//! L'elfe (personnages 4 et 5 : les deux variantes ont le même code).
//!
//! Quatre actions, atteintes par `characters::run` :
//! - `jump` (0x42012) : le saut commun (`common::jump`) avec deux gestes en l'air (coup, tir) ;
//! - `attack` (0x4241c) : le bouton 1 ; `Player::SUBACTION` choisit le coup (voir `kind`) ;
//! - `special` (0x42b0c) : l'objet ou la magie choisie (`field::SPECIAL_KIND`, 0..0x1c) ;
//! - l'action 0x10 (0x444ca) est la glissade du combattant (`common::dash`).
//!
//! Les objets lancés, la foudre, le mur et les magies 13..20 sont ceux du combattant (`items`) avec les
//! animations de l'elfe et une pose finale ; les magies 0x15..0x1c lui sont propres. Les routines d'origine
//! qui créent tirs, effets et sons restent appelées telles quelles.

use super::common::{self, call, countdown, field, is_cancelled, next_step, take_event, wind_down};
use super::foes::{self, Boost, Damage, Live, Strike};
use super::items;
use super::super::spawn;
use super::super::actions::to_stand;
use super::super::checks::item_curse;
use super::super::combos::clear_recognizers;
use super::super::walk::{move_x, move_xz};
use super::super::{animation, Ctx, Player};
use crate::objects::Obj;

/// Routines d'origine appelées telles quelles : effets joueur, sons, palettes et effets d'écran.
mod rom {
    /// Sons des coups.
    pub const SOUND_STRIKE: u32 = 0xe1e80;
    pub const SOUND_STRIKE_B: u32 = 0xe1de0;
    pub const SOUND_CHARGE_A: u32 = 0xe792e;
    pub const SOUND_CHARGE_B: u32 = 0xe1fc0;
    pub const SOUND_LEAP: u32 = 0xe1f20;
    /// Rebond sur un monstre pendant le saut (D0, D1 = numéro du joueur << 8).
    pub const BOUNCE_EFFECT: u32 = 0x19ae6;
    /// Tourbillon : effet joueur et sons.
    pub const WHIRL_EFFECT: u32 = 0x4b8d0;
    pub const WHIRL_SOUND_A: u32 = 0xe82fe;
    pub const WHIRL_SOUND_B: u32 = 0xe2060;
    /// Palettes du joueur : lumière, normale, rouge, et la palette 0x3b04.
    pub const PALETTE_LIGHT: u32 = 0x3af8;
    pub const PALETTE_NORMAL: u32 = 0x3afe;
    pub const PALETTE_RED: u32 = 0x3b14;
    pub const PALETTE_B04: u32 = 0x3b04;
    /// Effets d'écran des magies.
    pub const SCREEN_FLASH_END: u32 = 0x1abe0;
    pub const SCREEN_RIPPLE: u32 = 0x1aaa0;
    pub const SCREEN_FADE_A: u32 = 0x1aaee;
    pub const SCREEN_FADE_B: u32 = 0x1aad4;
    pub const EFFECT_SPARKS: u32 = 0x4dc9a;
    pub const EFFECT_BOLT: u32 = 0x4b7a0;
    pub const EFFECT_QUAKE: u32 = 0x4b340;
    pub const EFFECT_QUAKE_B: u32 = 0x4f132;
    pub const EFFECT_MOON: u32 = 0x4e4de;
    pub const EFFECT_MOON_B: u32 = 0x4e782;
    /// Sons des magies, dans l'ordre d'appel de chaque sort.
    pub const WARD_SOUNDS: [u32; 2] = [0xe28e4, 0xe96e0];
    pub const WARD_END_SOUNDS: [u32; 2] = [0xe2e3a, 0xe8e88];
    pub const SHIELD_SOUNDS: [u32; 2] = [0xe28e4, 0xe8e1e];
    pub const SHIELD_SECOND_SOUNDS: [u32; 2] = [0xe2e7a, 0xe8e88];
    pub const BEAM_SOUND: u32 = 0xe8ce0;
    pub const BEAM_SECOND_SOUND: u32 = 0xe2eba;
    pub const SPARK_SOUND: u32 = 0xe84c6;
    pub const SPARK_END_SOUND: u32 = 0xe2d3a;
    pub const BOLT_SOUND: u32 = 0xe28e4;
    pub const BOLT_SECOND_SOUND: u32 = 0xe88d0;
    pub const BOLT_END_SOUNDS: [u32; 2] = [0xe893a, 0xe2d7a];
    pub const FROST_SOUNDS: [u32; 2] = [0xe28e4, 0xe860c];
    pub const FROST_START_SOUND: u32 = 0xe2dba;
    pub const QUAKE_SOUND: u32 = 0xe8db4;
    pub const QUAKE_START_SOUND: u32 = 0xe2dfa;
    pub const QUAKE_SECOND_SOUND: u32 = 0xe8b22;
    pub const MOON_START_SOUNDS: [u32; 2] = [0xe910c, 0xe296c];
    pub const MOON_SOUND: u32 = 0xe2efa;
    pub const MOON_SOUND_B: u32 = 0xe9a88;
}

/// Champ qui garde l'allié béni par la magie 0x15 (mot : adresse de son enregistrement).
const ALLY: u32 = 0x1ec;

/// Table de l'elfe (données de la ROM) : vitesses des sauts (quatre mots par type de saut).
const JUMP_SPEEDS: u32 = 0x10_453e;

/// Sous-actions du bouton 1 (`Player::SUBACTION`), toutes paires.
mod kind {
    /// Enchaînement de sept coups.
    pub const COMBO: u8 = 0x00;
    pub const THRUST_HIGH: u8 = 0x02;
    pub const THRUST_LOW: u8 = 0x04;
    pub const TURN_STRIKE: u8 = 0x06;
    pub const SLOW_STRIKE: u8 = 0x08;
    pub const AFTER_JUMP: u8 = 0x0a;
    /// Charge : élan de 12 pixels par image suivi de deux effets.
    pub const CHARGE: u8 = 0x0c;
    pub const RUN_STRIKE: u8 = 0x0e;
    pub const LOW_BLOW: u8 = 0x10;
    pub const BACK_HOP: u8 = 0x12;
    pub const GUARD_BREAK: u8 = 0x14;
    /// Tir en reculant.
    pub const RETREAT_SHOT: u8 = 0x16;
    /// Tourbillon (sacrifie des points de vie).
    pub const WHIRL: u8 = 0x18;
    /// Coup avec un petit bond en arrière.
    pub const HOP_STRIKE_A: u8 = 0x1a;
    pub const HOP_STRIKE_B: u8 = 0x1c;
    /// Volée de tirs.
    pub const VOLLEY: u8 = 0x1e;
}

// ---- saut (0x42012) --------------------------------------------------------------------------

/// Le saut de l'elfe : table de vitesses 0x10453e, poussée de 0x20 en l'air, deux gestes en l'air (pas
/// 0xa : coup, 0xc : tir d'objet) pendant lesquels le saut est suspendu.
const JUMP_KIT: common::JumpKit = common::JumpKit {
    speeds: JUMP_SPEEDS,
    steer_push: 0x20,
    air_attack,
    after_recover: common::curse_check,
    finish: common::flush_pending_effect,
    takeoff: |g, p| {
        p.set_u8(g, 0x59, 0);
        p.set_u8(g, 0x58, 0);
        p.set_u8(g, 0x1e8, 0);
    },
    busy: |g, p| p.u8(g, field::AIR_ATTACK_DONE) == 2,
    apex_gravity: None,
    extra_step,
};

/// 0x42012 : saut.
pub fn jump(g: &mut Ctx, p: Player) {
    cov!();
    common::jump(g, p, &JUMP_KIT);
}

/// 0x4233a : en l'air, le bouton 1 donne un coup (`AIR_ATTACK_DONE` = 1, pas 0xa) ; la touche 4 avec
/// l'objet 0x23..0x26 en ligne 2 lance un tir (`AIR_ATTACK_DONE` = 2, pas 0xc). Une fois par saut.
fn air_attack(g: &mut Ctx, p: Player) {
    cov!();
    if p.u8(g, field::AIR_ATTACK_DONE) != 0 {
        return;
    }
    let keys = p.u8(g, Player::KEYS_NEW);
    if keys & 0x10 != 0 {
        p.set_u8(g, Player::STEP, 0xa);
        p.set_u8(g, field::MOVE_STATE, 0);
        p.set_u8(g, field::AIR_ATTACK_DONE, 1);
        let (mut anim, mut style) = (0xb, 1);
        if p.u8(g, 0x67) == 2 {
            (anim, style) = (0xe, 2);
        }
        if p.u8(g, Player::KEYS_HELD) & 4 != 0 {
            (anim, style) = (0xd, 3);
        }
        p.set_u8(g, 0x59, style);
        common::start_with_voice(g, p, anim);
        item_curse(g, p);
        common::aura_hit_countdown(g, p);
    } else if keys & 0x80 != 0 && p.u8(g, 0x15d) == 2 && (0x23..=0x26).contains(&p.u8(g, field::ITEM)) {
        p.set_u8(g, Player::STEP, 0xc);
        p.set_u8(g, field::AIR_ATTACK_DONE, 2);
        animation::start_plain(g, p, 0x22);
        common::aura_hit_countdown(g, p);
    }
}

/// Pas 0xa et 0xc du saut : le coup en l'air (rebond possible sur un monstre) et le tir d'objet.
fn extra_step(g: &mut Ctx, p: Player, step: u8) {
    cov!();
    match step {
        0xa => {
            if p.u8(g, 0x59) == 3 && p.u8(g, 0x58) & 1 != 0 && p.u8(g, 0x1e8) <= 4 {
                bounce(g, p);
            } else {
                air_fall(g, p);
            }
        }
        0xc => {
            if !air_fall(g, p) && p.u8(g, field::AIR_ATTACK_DONE) == 2 && take_event(g, p) {
                match p.u8(g, field::ITEM) {
                    0x24 => spawn::thrown(g, p, spawn::Thrown::C),
                    0x25 => spawn::thrown(g, p, spawn::Thrown::D),
                    0x26 => spawn::thrown(g, p, spawn::Thrown::E),
                    _ => false,
                };
            }
        }
        _ => {}
    }
}

/// Rebond : le coup plongeant a touché, l'elfe repart vers le haut (au plus cinq fois).
fn bounce(g: &mut Ctx, p: Player) {
    cov!();
    p.set_u8(g, 0x58, 0);
    p.set_u8(g, field::AIR_STEERED, 0);
    let n = p.u8(g, 0x1e8) + 1;
    p.set_u8(g, 0x1e8, n);
    p.set_u16(g, Obj::VZ, 0x600);
    p.set_u16(g, Obj::AZ, (-0x48i16) as u16);
    common::set_forward_motion(g, p, 0x180, 0);
    animation::start_plain(g, p, 0x41);
    let tag = (p.u8(g, Player::INDEX) as u32) << 8;
    call(g, p, rom::BOUNCE_EFFECT, &[(0, tag), (1, tag)]);
}

/// Un pas de vol pendant un geste en l'air ; à l'atterrissage le saut reprend à la récupération (pas
/// 8). Renvoie vrai si l'elfe a atterri.
fn air_fall(g: &mut Ctx, p: Player) -> bool {
    cov!();
    animation::step(g, p);
    move_xz(g, p);
    common::air_steer(g, p, 0x20);
    if (p.u16(g, Obj::Z) as i16) < 0 {
        p.set_u8(g, Player::STEP, 8);
        common::land(g, p);
        return true;
    }
    false
}

// ---- attaque (0x4241c) -----------------------------------------------------------------------

/// 0x4241c : attaque du bouton 1. `kind` (voir plus haut) choisit le coup ; la plupart des coups sont
/// suivis d'un contrôle de prolongement (`common::follow_up`).
pub fn attack(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::SUBACTION) {
        kind::COMBO => {
            combo(g, p);
            common::follow_up(g, p);
        }
        kind::THRUST_HIGH => {
            thrust(g, p, 1 << 1, 7);
            common::follow_up(g, p);
        }
        kind::THRUST_LOW => {
            thrust(g, p, 1 << 2, 9);
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
            common::single_strike(g, p, 0x13);
            common::follow_up(g, p);
        }
        kind::BACK_HOP => common::back_hop(g, p, 0x1d, false),
        kind::GUARD_BREAK => common::single_strike(g, p, 0x14),
        kind::RETREAT_SHOT => retreat_shot(g, p),
        kind::WHIRL => whirl(g, p),
        kind::HOP_STRIKE_A | kind::HOP_STRIKE_B => {
            hop_strike(g, p);
            common::follow_up(g, p);
        }
        kind::VOLLEY => volley(g, p),
        _ => {}
    }
    common::flush_pending_effect(g, p);
}

/// Coup de base (0x424ee) : sept gestes enchaînés tant que les coups portent ; le dernier peut finir par
/// un petit bond en arrière.
fn combo(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, field::MOVE_STATE, 0);
            p.set_u8(g, field::COMBO_RELEASED, 0);
            p.set_u8(g, field::COMBO_WINDOW, 6);
            let rank = common::chain_rank(g, p, 1, 6);
            common::start_with_voice(g, p, rank as u16);
        }
        2 => {
            if common::combo_follow(g, p, 0x1a) {
                g.rom(rom::SOUND_STRIKE, p);
            }
        }
        4 => hop_wait(g, p),
        6 => hop_flight(g, p),
        _ => {}
    }
}

/// Coups d'estoc (0x425f8 haut, 0x4265a bas) : un seul geste, de rang 0 ou 1 selon le coup précédent.
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

/// Coup donné en se retournant (0x426ac) : le pas 2 est le pas 0 d'une entrée par état (combos).
fn turn_strike(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            p.set_u8(g, Player::STEP, 4);
            p.set_u8(g, field::MOVE_STATE, 0);
            animation::start_plain(g, p, 0x12);
        }
        2 => {
            p.set_u8(g, Player::STEP, 4);
            common::start_with_voice(g, p, 0x1c);
        }
        4 => common::wait_then_stand(g, p),
        _ => {}
    }
}

/// Coup donné en marche lente (0x426ee) : sans prolongement.
fn slow_strike(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, field::MOVE_STATE, 0);
            common::start_with_voice(g, p, 0x15);
        }
        2 => common::wait_then_stand(g, p),
        _ => {}
    }
}

/// Coup de retombée (0x42728).
fn after_jump(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, field::MOVE_STATE, 0);
            animation::start_plain(g, p, 0xf);
            clear_recognizers(g, p);
            g.rom(rom::SOUND_STRIKE_B, p);
        }
        2 => common::wait_then_stand(g, p),
        _ => {}
    }
}

/// Charge (0x4275a) : élan, premier geste, second geste avec deux effets, glissade.
fn charge(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, 0x5b, 0);
            p.set_u8(g, field::MOVE_STATE, 1);
            p.set_u8(g, field::MOVE_KIND, 1);
            common::set_forward_motion(g, p, 0xc00, (-0x98i16) as u16);
            animation::start_plain(g, p, 0x16);
            clear_recognizers(g, p);
        }
        2 => {
            animation::step(g, p);
            if p.u8(g, field::EVENT) != 0 {
                next_step(g, p);
                p.set_u8(g, field::EVENT, 0);
            }
        }
        4 => common::charge_slide(g, p, |g, p| {
            if take_event(g, p) {
                spawn::slash(g, p, Some(0));
                spawn::slash(g, p, None);
                g.rom(rom::SOUND_CHARGE_B, p);
            }
        }),
        _ => {}
    }
}

/// Coup donné en courant (0x42810) : élan de 5 pixels par image, puis arrêt.
fn run_strike(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, field::MOVE_STATE, 0);
            common::set_forward_motion(g, p, 0x500, (-0x28i16) as u16);
            animation::start_plain(g, p, 0x10);
            clear_recognizers(g, p);
            common::weapon_voice(g, p);
            g.rom(rom::SOUND_CHARGE_A, p);
        }
        2 => {
            common::run_strike_slide(g, p, 0x11);
        }
        4 => common::wait_then_stand(g, p),
        _ => {}
    }
}

/// Tir en reculant (0x42948) : geste, élan en avant avec deux effets, bond en arrière, retombée.
fn retreat_shot(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, 0x5b, 0);
            p.set_u8(g, field::MOVE_STATE, 1);
            p.set_u8(g, field::MOVE_KIND, 1);
            animation::start_plain(g, p, 0x17);
            clear_recognizers(g, p);
        }
        2 => {
            animation::step(g, p);
            if p.u8(g, field::EVENT) != 0 {
                next_step(g, p);
                p.set_u8(g, field::EVENT, 0);
                common::set_forward_motion(g, p, 0xa80, (-0x98i16) as u16);
                move_x(g, p);
                spawn::slash(g, p, Some(0));
                spawn::slash(g, p, None);
            }
        }
        4 => {
            if animation::step(g, p) {
                next_step(g, p);
                p.set_u16(g, Obj::VX, 0x180);
                p.set_u16(g, Obj::AX, 0);
                p.set_u16(g, Obj::VZ, 0x600);
                p.set_u16(g, Obj::AZ, (-0x38i16) as u16);
                if p.u8(g, Player::FACING) != 0 {
                    common::negate_horizontal_speed(g, p);
                }
                animation::start_plain(g, p, 0x18);
                g.rom(rom::SOUND_LEAP, p);
            } else {
                move_x(g, p);
            }
        }
        6 => {
            animation::step(g, p);
            move_xz(g, p);
            if (p.u16(g, Obj::Z) as i16) < 0 {
                next_step(g, p);
                p.set_u32(g, Obj::Z, 0);
                p.set_u8(g, Player::SLOW, 0);
                animation::start_plain(g, p, 0x19);
            }
        }
        8 => common::wait_then_stand(g, p),
        _ => {}
    }
}

/// Tourbillon (0x42a2e) : coûte 12 points de vie au premier contact.
fn whirl(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, 0x5b, 0);
            p.set_u8(g, field::MOVE_STATE, 1);
            p.set_u8(g, field::MOVE_KIND, 2);
            p.set_u8(g, 0x58, 0);
            p.set_u8(g, 0x1ef, 0);
            p.set_u8(g, 0x1e8, 0x80);
            animation::start_plain(g, p, 0x1b);
            clear_recognizers(g, p);
        }
        2 => {
            animation::step(g, p);
            if take_event(g, p) {
                g.rom(rom::WHIRL_EFFECT, p);
                g.rom(rom::WHIRL_SOUND_A, p);
                g.rom(rom::WHIRL_SOUND_B, p);
            }
            let left = p.u8(g, 0x1e8).wrapping_sub(1);
            p.set_u8(g, 0x1e8, left);
            if left == 0 {
                to_stand(g, p);
                return;
            }
            if p.u8(g, 0x1ef) == 0 && p.u8(g, 0x58) & 1 != 0 {
                p.set_u8(g, 0x1ef, 1);
                let hp = (p.u16(g, Player::HP) as i16).wrapping_sub(0xc);
                p.set_u16(g, Player::HP, if hp > 0 { hp as u16 } else { 1 });
            }
        }
        _ => {}
    }
}

/// Coup avec petit bond (sous-actions 0x1a et 0x1c, et fin du coup de base) : geste, puis, au geste porté,
/// bond en arrière.
fn hop_strike(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, field::MOVE_STATE, 0);
            animation::start_plain(g, p, 0x1a);
            g.rom(rom::SOUND_STRIKE, p);
        }
        2 => hop_wait(g, p),
        4 => hop_flight(g, p),
        _ => {}
    }
}

/// Le geste se joue ; au geste porté (`EVENT`) l'elfe bondit en arrière de 16 pixels (0x4248c).
fn hop_wait(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    if p.u8(g, field::EVENT) == 0 {
        return;
    }
    next_step(g, p);
    p.set_u16(g, Obj::VZ, 0x300);
    p.set_u16(g, Obj::AZ, (-0x40i16) as u16);
    let (mut vx, mut shift) = (0x200i16, -0x10i16);
    if p.u8(g, Player::FACING) != 0 {
        (vx, shift) = (-vx, -shift);
    }
    p.set_u16(g, Obj::VX, vx as u16);
    p.set_u16(g, Obj::AX, 0);
    let x = p.u16(g, Obj::X).wrapping_add(shift as u16);
    p.set_u16(g, Obj::X, x);
}

/// Vol du bond (0x424c8) : tant que le geste n'est pas fini, l'elfe avance ; à la fin il est de retour
/// au sol.
fn hop_flight(g: &mut Ctx, p: Player) {
    cov!();
    if p.u8(g, field::EVENT) == 0 {
        p.set_u32(g, Obj::Z, 0);
    } else {
        move_xz(g, p);
    }
    if animation::step(g, p) {
        p.set_u32(g, Obj::Z, 0);
        to_stand(g, p);
    }
}

/// Volée (0x42ac0) : 144 images de tirs d'objet au geste porté.
fn volley(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u16(g, field::TIMER, 0x90);
            animation::start_plain(g, p, 0x42);
        }
        2 => {
            animation::step(g, p);
            if countdown(g, p) {
                to_stand(g, p);
                clear_recognizers(g, p);
            } else if take_event(g, p) {
                spawn::thrown(g, p, spawn::Thrown::C);
            }
        }
        _ => {}
    }
}

// ---- attaque spéciale (0x42b0c) --------------------------------------------------------------

/// Animations de l'elfe pour les magies lancées (voir `items::Style`) : la cible de la foudre et du
/// mur est gardée en `+0x1ec`, les monstres sont blessés à la dernière image de la foudre.
const STYLE: items::Style = items::Style {
    cast_anim: 0x24,
    bolt_anim: 0x23,
    spark: 2,
    target: 0x1ec,
    thunder_at: 1,
    finish_anim: Some(0x25),
    d0_is_step: true,
    undim_mode: 2,
};

/// L'anneau rouge (n° 16) de l'elfe : l'aura n'est posée qu'à la fin du décompte.
const CAST_16: items::Cast = items::Cast {
    fire: items::ring_fire,
    on_end: |g, p| items::red_aura(g, p, 0x708),
    ..items::CAST_13
};

/// 0x42b0c : l'objet ou la magie choisie (`field::SPECIAL_KIND`).
pub fn special(g: &mut Ctx, p: Player) {
    cov!();
    let which = p.u8(g, field::SPECIAL_KIND);
    g.set_a5u8(0x5b, which);
    match which {
        0 => items::throw_item(g, p, 0x20, spawn::Thrown::A),
        1 | 0xb => items::throw_item(g, p, 0x20, spawn::Thrown::B),
        2..=5 => timed_throw(g, p),
        6 => items::throw_item(g, p, 0x1f, spawn::Thrown::F),
        7 => items::throw_item(g, p, 0x1f, spawn::Thrown::G),
        8 => items::throw_item(g, p, 0x1e, spawn::Thrown::H),
        0xc => common::swap_weapon(g, p, Player::WEAPON_A, true),
        16 => items::cast(g, p, &CAST_16, STYLE),
        0x15 => ward_ally(g, p),
        0x16 => ward_party(g, p),
        0x17 => beam(g, p),
        0x18 => sparks(g, p),
        0x19 => bolt(g, p),
        0x1a => frost(g, p),
        0x1b => quake(g, p),
        0x1c => moonlight(g, p),
        _ => {
            items::run_shared(g, p, which, STYLE);
        }
    }
}

/// Objets 2..5 (0x42bb0) : une pose de 48 images, un tir au geste porté ; le numéro 3 ne consomme pas de
/// charge.
fn timed_throw(g: &mut Ctx, p: Player) {
    cov!();
    if p.u8(g, Player::STEP) == 0 {
        next_step(g, p);
        p.set_u16(g, field::TIMER, 0x30);
        animation::start_plain(g, p, 0x21);
        return;
    }
    animation::step(g, p);
    if countdown(g, p) {
        to_stand(g, p);
        return;
    }
    if !take_event(g, p) {
        return;
    }
    let which = p.u8(g, field::SPECIAL_KIND);
    let launcher = match which {
        2 | 3 => spawn::Thrown::C,
        4 => spawn::Thrown::D,
        _ => spawn::Thrown::E,
    };
    if spawn::thrown(g, p, launcher) && which != 3 {
        common::consume_charge(g, p);
    }
}


/// Joue des sons de la ROM.
fn sounds(g: &mut Ctx, p: Player, list: &[u32]) {
    for &sound in list {
        g.rom(sound, p);
    }
}

/// Fin des sorts qui passent d'abord par un pas final d'animation (sans rétablir le drapeau d'annulation).
fn end_when_done(g: &mut Ctx, p: Player) {
    cov!();
    if animation::step(g, p) {
        common::end_spell_keep_cancel(g, p);
    }
}

/// Mot de la ROM (code) indexé par le niveau du lanceur.
fn by_level(g: &mut Ctx, p: Player, table: u32) -> u16 {
    let level = p.u8(g, Player::LEVEL) as u32;
    g.prog16(table + 2 * level)
}

/// Aura pour `target` : durée `duration` (`AURA_TIMER`), puissance 1, sans les autres drapeaux.
fn guard_aura(g: &mut Ctx, target: Player, duration: u16) {
    target.set_u16(g, Player::AURA_TIMER, duration);
    target.set_u8(g, Player::AURA_B, 1);
    target.set_u8(g, Player::AURA, 0);
    target.set_u8(g, Player::AURA_C, 0);
}

/// 0x43b04, n° 0x15 (h) : anneau rouge sur l'allié devant l'elfe (à portée de 76 pixels) ou sur lui-même.
fn ward_ally(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => common::start_cast(g, p, 0x20, Some(0x152), 0x3b),
        2 => {
            animation::step(g, p);
            if take_event(g, p) {
                spawn::ring(g, p, 0);
                g.rom(rom::PALETTE_RED, p);
                pick_ally(g, p);
                sounds(g, p, &rom::WARD_SOUNDS);
            }
            if countdown(g, p) {
                next_step(g, p);
                p.set_u16(g, field::TIMER, 0x20);
                let anim = if p.u8(g, field::AREA) != 0 { 0x3d } else { 0x3c };
                animation::start_plain(g, p, anim);
                sounds(g, p, &rom::WARD_END_SOUNDS);
            }
        }
        4 => {
            animation::step(g, p);
            if take_event(g, p) {
                if is_cancelled(g, p) {
                    p.set_u8(g, Player::STEP, 8);
                    p.set_u16(g, field::TIMER, 0x10);
                    let spark = if p.u8(g, field::AREA) != 0 { 0xb } else { 0xa };
                    common::item_spark(g, p, spark);
                    return wind_down(g, p, 2);
                }
                spawn::ring(g, p, 1);
            }
            if countdown(g, p) {
                next_step(g, p);
                let target = if p.u8(g, field::AREA) != 0 {
                    Player(p.u16(g, ALLY) as i16 as i32 as u32 & 0xff_ffff)
                } else {
                    p
                };
                items::red_aura(g, target, 0x708);
                animation::start_plain(g, p, 0x3e);
            }
        }
        6 => {
            if animation::step(g, p) {
                common::end_spell(g, p);
            }
        }
        8 => wind_down(g, p, 2),
        _ => {}
    }
}

/// 0x43cac : cherche l'allié béni (voir `common::find_ally`, portée 76) : sans annulation il se retourne
/// vers l'elfe et passe à la pose de bénédiction ; `AREA` = 1 si un allié est trouvé.
fn pick_ally(g: &mut Ctx, p: Player) {
    cov!();
    let ally = common::find_ally(g, p, 0x4c, |_, _| ());
    if let Some(q) = ally {
        if !is_cancelled(g, p) {
            p.set_u16(g, ALLY, q.0 as u16);
            q.set_u32(g, Player::MAIN, 0x0200_1c06);
            q.set_u8(g, field::SPELL_FROZEN, 0);
            q.set_u8(g, Player::FACING, 0);
            if p.u16(g, Obj::X) > q.u16(g, Obj::X) {
                q.set_u8(g, Player::FACING, 0x20);
            }
        }
    }
    p.set_u8(g, field::AREA, ally.is_some() as u8);
}

/// 0x43e4e, n° 0x16 (h) : anneau rouge, puis aura de garde sur tous les joueurs en vie (durée par niveau).
fn ward_party(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => common::start_cast(g, p, 0x20, Some(0x153), 0x38),
        2 => {
            animation::step(g, p);
            if take_event(g, p) {
                spawn::ring(g, p, 0);
                g.rom(rom::PALETTE_B04, p);
                sounds(g, p, &rom::SHIELD_SOUNDS);
            }
            if countdown(g, p) {
                next_step(g, p);
                p.set_u16(g, field::TIMER, 0x20);
                if is_cancelled(g, p) {
                    p.set_u8(g, Player::STEP, 8);
                    p.set_u16(g, field::TIMER, 0x10);
                    common::item_spark(g, p, 0xd);
                }
                animation::start_plain(g, p, 0x39);
            }
        }
        4 => {
            animation::step(g, p);
            if take_event(g, p) {
                spawn::ring(g, p, 1);
                sounds(g, p, &rom::SHIELD_SECOND_SOUNDS);
            }
            if countdown(g, p) {
                next_step(g, p);
                let duration = by_level(g, p, WARD_DURATIONS);
                for q in Player::all() {
                    if q.u8(g, Player::ACTIVE) != 0 && q.u16(g, Player::HP) != 0 && q.u8(g, Player::POWER_10F) == 0 {
                        guard_aura(g, q, duration);
                    }
                }
                animation::start_plain(g, p, 0x3a);
            }
        }
        6 => end_when_done(g, p),
        8 => wind_down(g, p, 2),
        _ => {}
    }
}

/// Mots (code) : durée de l'aura de garde par niveau.
const WARD_DURATIONS: u32 = 0x43f70;

/// 0x43d5e, n° 0x17 (h) : rayon de lumière en deux temps.
fn beam(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => common::start_cast(g, p, 0x38, Some(0x154), 0x29),
        2 => {
            animation::step(g, p);
            if take_event(g, p) {
                spawn::beam_cast(g, p, 0);
                g.rom(rom::PALETTE_LIGHT, p);
                g.rom(rom::BEAM_SOUND, p);
            }
            if countdown(g, p) {
                next_step(g, p);
                p.set_u16(g, field::TIMER, 0x3c);
                animation::start_plain(g, p, 0x2a);
            }
        }
        4 => {
            animation::step(g, p);
            if take_event(g, p) {
                if is_cancelled(g, p) {
                    p.set_u16(g, field::TIMER, 0x10);
                    common::item_spark(g, p, 0xc);
                } else {
                    spawn::beam_cast(g, p, 1);
                    g.rom(rom::BEAM_SECOND_SOUND, p);
                }
            }
            if countdown(g, p) {
                common::end_spell(g, p);
            }
        }
        _ => {}
    }
}

/// Octets (code) : numéro de la variante du geste de la magie 0x18 par niveau.
const SPARK_GESTURES: u32 = 0x43476;

/// 0x433e4, n° 0x18 (h) : gerbe d'étincelles, qui alterne deux variantes à chaque geste.
fn sparks(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => common::start_cast(g, p, 0x3c, Some(0x151), 0x34),
        2 => {
            animation::step(g, p);
            if take_event(g, p) {
                g.rom(rom::EFFECT_SPARKS, p);
                g.rom(rom::SPARK_SOUND, p);
            }
            if countdown(g, p) {
                next_step(g, p);
                p.set_u8(g, 0x1e8, 0);
                let level = p.u8(g, Player::LEVEL) as u32;
                let gesture = g.prog8(SPARK_GESTURES + level) as u16;
                animation::start_plain(g, p, 0x34 + gesture);
                g.rom(rom::SPARK_END_SOUND, p);
            }
        }
        4 => {
            if p.u8(g, field::EVENT) != 0 {
                let side = p.u8(g, 0x1e8) ^ 1;
                p.set_u8(g, 0x1e8, side);
                if is_cancelled(g, p) {
                    next_step(g, p);
                    p.set_u16(g, field::TIMER, 0x10);
                    common::item_spark(g, p, 7);
                    return wind_down(g, p, 2);
                }
                spawn::shower(g, p, (side == 0) as u8);
                p.set_u8(g, field::EVENT, 0);
            }
            if animation::step(g, p) {
                next_step(g, p);
                p.set_u16(g, field::TIMER, 1);
            }
        }
        6 => wind_down(g, p, 2),
        _ => {}
    }
}

/// 0x4350c, n° 0x19 (h) : éclair en deux temps.
fn bolt(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => common::start_cast(g, p, 0x28, Some(0x153), 0x26),
        2 => {
            animation::step(g, p);
            if take_event(g, p) {
                call(g, p, rom::EFFECT_BOLT, &[(0, 0)]);
                g.rom(rom::PALETTE_LIGHT, p);
                g.rom(rom::BOLT_SOUND, p);
            }
            if countdown(g, p) {
                next_step(g, p);
                p.set_u16(g, field::TIMER, 0x30);
                animation::start_plain(g, p, 0x27);
            }
        }
        4 => {
            animation::step(g, p);
            if take_event(g, p) {
                g.rom(rom::BOLT_SECOND_SOUND, p);
                if is_cancelled(g, p) {
                    next_step(g, p);
                    p.set_u16(g, field::TIMER, 0x10);
                    common::item_spark(g, p, 6);
                    return wind_down(g, p, 2);
                }
                call(g, p, rom::EFFECT_BOLT, &[(0, 1)]);
            }
            if countdown(g, p) {
                next_step(g, p);
                p.set_u16(g, field::TIMER, 0x5a);
                animation::start_plain(g, p, 0x28);
                sounds(g, p, &rom::BOLT_END_SOUNDS);
            }
        }
        6 => wind_down(g, p, 2),
        _ => {}
    }
}

/// Mots (code) : durée de la gelée de la magie 0x1a par niveau.
const FROST_DURATIONS: u32 = 0x436c8;

/// 0x4362a, n° 0x1a (h) : gelée. Pendant la phase finale le joueur règle l'intensité avec haut et bas
/// (geste 0x2f + intensité) ; l'écran tremble.
fn frost(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => common::start_cast(g, p, 0x20, Some(0x153), 0x2d),
        2 => {
            animation::step(g, p);
            if take_event(g, p) {
                spawn::ice_crown(g, p);
                sounds(g, p, &rom::FROST_SOUNDS);
            }
            if countdown(g, p) {
                next_step(g, p);
                p.set_u8(g, 0x1e8, 0x10);
                p.set_u8(g, 0x1e9, 2);
                let duration = by_level(g, p, FROST_DURATIONS);
                p.set_u16(g, field::TIMER, duration);
                animation::start_plain(g, p, 0x2e);
            }
        }
        4 => {
            if !animation::step(g, p) {
                return;
            }
            next_step(g, p);
            p.set_u8(g, 0x1eb, 0);
            p.set_u8(g, 0x1ea, 4);
            p.set_u8(g, 0x1e8, 0x10);
            p.set_u8(g, 0x1e9, 2);
            animation::start_plain(g, p, 0x2f + 2);
            g.rom(rom::FROST_START_SOUND, p);
            if is_cancelled(g, p) {
                p.set_u8(g, Player::STEP, 8);
                p.set_u16(g, field::TIMER, 0x10);
                common::item_spark(g, p, 8);
                return wind_down(g, p, 2);
            }
            spawn::ice_spell(g, p);
        }
        6 => frost_steer(g, p),
        8 => wind_down(g, p, 2),
        _ => {}
    }
}

/// Pas 6 de la gelée (0x43788) : l'écran clignote trois fois ; l'intensité suit les touches haut et bas.
fn frost_steer(g: &mut Ctx, p: Player) {
    cov!();
    common::blink_screen(g, p);
    if countdown(g, p) {
        next_step(g, p);
        p.set_u16(g, field::TIMER, 1);
        p.set_u8(g, 0x1eb, 0);
    }
    common::flash_screen(g, p);
    common::intensity_keys(g, p, 0x2f);
}

/// Mots (code) : durée de la magie 0x1b par niveau.
const QUAKE_DURATIONS: u32 = 0x4395a;

/// Tremblement de terre (0x43892, n° 0x1b) : blesse les monstres au bon moment.
const QUAKE_STRIKE: Strike = Strike {
    damage: Damage::Table(0x10_442c, 0x10_440e),
    live: Live::Any,
    needs_hp: true,
    grounded: false,
    immune_bit: 4,
    reaction: 0x90,
    random_facing: true,
    boosts: &[Boost::Boss(0xc), Boost::Armor, Boost::Charm(5)],
    undead_die: true,
    sound: None,
};

/// 0x43892, n° 0x1b (h) : tremblement de terre.
fn quake(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            common::begin_cast(g, p, Some(0x3c), Some(0x154));
            common::dim_screen(g, p);
            g.rom(rom::SCREEN_RIPPLE, p);
            animation::start_plain(g, p, 0x2b);
        }
        2 => {
            animation::step(g, p);
            if take_event(g, p) {
                call(g, p, rom::EFFECT_QUAKE, &[(0, 0)]);
                g.rom(rom::PALETTE_NORMAL, p);
                g.rom(rom::QUAKE_SOUND, p);
            }
            if countdown(g, p) {
                next_step(g, p);
                let duration = by_level(g, p, QUAKE_DURATIONS);
                p.set_u16(g, field::TIMER, duration);
                animation::start_plain(g, p, 0x2c);
                g.rom(rom::QUAKE_START_SOUND, p);
                if is_cancelled(g, p) {
                    common::item_spark(g, p, 9);
                    p.set_u16(g, field::TIMER, 0x10);
                    return quake_rumble(g, p);
                }
                call(g, p, rom::EFFECT_QUAKE, &[(0, 1)]);
                g.rom(rom::EFFECT_QUAKE_B, p);
                g.rom(rom::QUAKE_SECOND_SOUND, p);
            }
        }
        4 => quake_rumble(g, p),
        _ => {}
    }
}

/// Pas 4 du tremblement de terre (0x4398c) : au 0x64e tour de décompte, les monstres sont blessés.
fn quake_rumble(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    if countdown(g, p) {
        g.rom(rom::SCREEN_FLASH_END, p);
        common::end_spell(g, p);
        return;
    }
    if !is_cancelled(g, p) && p.u16(g, field::TIMER) == 0x64 {
        foes::strike_foes(g, p, &QUAKE_STRIKE);
    }
}

/// Pluie de lune (0x44314, n° 0x1c, regard à gauche) : les monstres au sol sont blessés.
const MOON_STRIKE: Strike = Strike {
    damage: Damage::Table(0x10_4468, 0x10_444a),
    live: Live::Any,
    needs_hp: true,
    grounded: true,
    immune_bit: 8,
    reaction: 0xc,
    random_facing: true,
    boosts: &[Boost::Armor],
    undead_die: true,
    sound: None,
};

/// 0x43ff4, n° 0x1c (h) : magie de la lune. Deux variantes selon le regard : vers la droite (pas 0 à 6,
/// effet 0x4e4de) ou vers la gauche (pas 8 à 0xc, effet 0x4e782).
fn moonlight(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 | 8 => {
            if p.u8(g, Player::STEP) == 0 && p.u8(g, Player::FACING) != 0 {
                p.set_u8(g, Player::STEP, 8);
            }
            common::begin_cast(g, p, Some(0xb0), Some(0x155));
            common::dim_screen(g, p);
            animation::start_plain(g, p, 0x3f);
            sounds(g, p, &rom::MOON_START_SOUNDS);
        }
        2 => {
            animation::step(g, p);
            if countdown(g, p) {
                next_step(g, p);
                p.set_u16(g, field::TIMER, 0x80);
                animation::start_plain(g, p, 0x40);
            } else if p.u16(g, field::TIMER) == 0x20 {
                if !is_cancelled(g, p) {
                    for color in [0, 2, 3] {
                        call(g, p, rom::EFFECT_MOON, &[(0, color)]);
                    }
                    g.rom(rom::PALETTE_NORMAL, p);
                    g.rom(rom::MOON_SOUND_B, p);
                }
                g.rom(rom::SCREEN_FADE_A, p);
            }
        }
        4 => {
            animation::step(g, p);
            if take_event(g, p) {
                if is_cancelled(g, p) {
                    p.set_u8(g, Player::STEP, 6);
                    p.set_u16(g, field::TIMER, 0x10);
                    common::item_spark(g, p, 0xe);
                    return moon_finish(g, p);
                }
                call(g, p, rom::EFFECT_MOON, &[(0, 1)]);
                g.rom(rom::MOON_SOUND, p);
                items::meteor_damage(g, p);
            }
            if countdown(g, p) {
                next_step(g, p);
                p.set_u16(g, field::TIMER, 0x18);
            }
        }
        6 => moon_finish(g, p),
        0xa => {
            animation::step(g, p);
            if countdown(g, p) {
                next_step(g, p);
                p.set_u16(g, field::TIMER, 0x100);
                if is_cancelled(g, p) {
                    p.set_u16(g, field::TIMER, 0xa);
                    common::item_spark(g, p, 0xe);
                }
                animation::start_plain(g, p, 0x40);
                g.rom(rom::MOON_SOUND, p);
            } else if p.u16(g, field::TIMER) == 0x20 {
                if !is_cancelled(g, p) {
                    call(g, p, rom::EFFECT_MOON_B, &[(0, 0)]);
                    g.c.d[0] = 2;
                    g.rom(rom::PALETTE_NORMAL, p);
                }
                g.rom(rom::SCREEN_FADE_B, p);
            }
        }
        0xc => {
            animation::step(g, p);
            if countdown(g, p) {
                g.rom(rom::SCREEN_FLASH_END, p);
                common::end_spell(g, p);
            } else if !is_cancelled(g, p) && p.u16(g, field::TIMER) == 0x60 {
                foes::strike_foes(g, p, &MOON_STRIKE);
            }
        }
        _ => {}
    }
}

/// Pas 6 de la magie de la lune (0x4410e) : à la fin du décompte l'éclair d'écran s'éteint.
fn moon_finish(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    if countdown(g, p) {
        g.rom(rom::SCREEN_FLASH_END, p);
        common::end_spell(g, p);
    }
}

action_native!(JUMP, 0x42012, "elf_jump", jump);
action_native!(ATTACK, 0x4241c, "elf_attack", attack);
action_native!(SPECIAL, 0x42b0c, "elf_special", special);
action_native!(DASH, 0x444ca, "elf_dash", common::dash);
