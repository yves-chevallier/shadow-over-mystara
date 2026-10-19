//! Le nain (personnages 6 et 7 : les deux variantes ont le même code).
//!
//! Quatre actions, atteintes par `characters::run` :
//! - `jump` (0x445ba) : saut à profil (accroupi, montée, descente, rebond), avec coup en l'air et
//!   attaque plongeante ;
//! - `attack` (0x4491a) : le bouton 1 ; `Player::SUBACTION` choisit le coup (voir `kind`) ;
//! - `special` (0x45148) : l'objet ou la magie choisie (`field::SPECIAL_KIND`, 0..20), les mêmes que
//!   ceux du combattant (`items`) ;
//! - `dash` (0x45988) : l'action 0x10, la glissade commune (`common::dash`).
//!
//! Les routines d'origine qui créent tirs, effets et sons restent appelées telles quelles.

use super::common::{self, call, field, next_step, rom as common_rom, set_forward_motion};
use super::items;
use super::super::actions::{start_stand, to_stand};
use super::super::checks::item_curse;
use super::super::combos::clear_recognizers;
use super::super::walk::{move_x, move_xz, move_z};
use super::super::{animation, Ctx, Player};
use crate::objects::Obj;

/// Routines d'origine appelées telles quelles : effets joueur et sons du nain.
mod rom {
    /// Création d'un effet joueur (pool PEFF) : D0 = 1 ou 0 (+ D1) selon le geste.
    pub const SLASH_EFFECT: u32 = 0x4a99a;
    /// Effet de la longue glissade.
    pub const DASH_EFFECT: u32 = 0x4fe18;
    /// Secousse de l'écran à l'impact de la plongée (D0 = 0x10).
    pub const SHAKE: u32 = 0x2824;
    /// Sons.
    pub const SOUND_STRIKE: u32 = 0xe1de0;
    pub const SOUND_CHARGE_A: u32 = 0xe792e;
    pub const SOUND_CHARGE_B: u32 = 0xe1fc0;
    pub const SOUND_LEAP_A: u32 = 0xe7668;
    pub const SOUND_LEAP_B: u32 = 0xe1f20;
    pub const SOUND_SPIN: u32 = 0xe2060;
    pub const SOUND_DASH: u32 = 0xe7eaa;
    pub const SOUND_BOUNCE: u32 = 0xe8370;
}

/// Table des vitesses de saut du nain (données de la ROM) : une ligne de quatre mots (vx, ax, vz, az)
/// par profil de saut (`JUMP_ROW`).
const JUMP_SPEEDS: u32 = 0x10_455e;

/// Octets de l'enregistrement propres au nain.
mod own {
    /// Profil du saut en cours : ligne de la table des vitesses. 0 sur place, 2 après une course, 3
    /// vertical, 4 puis 5 saut en avant avec une direction tenue, 6 puis 7 sans direction.
    pub const JUMP_ROW: u32 = 0x1e8;
    /// Compteur de l'action (images).
    pub const COUNTER: u32 = 0x1e8;
    /// Le tourbillon a déjà coûté des points de vie (`1ec`) ; le contact est signalé par `+0x58` bit 0.
    pub const WHIRL_PAID: u32 = 0x1ec;
    pub const CONTACT: u32 = 0x58;
}

/// Sous-actions du bouton 1 (`Player::SUBACTION`), toutes paires.
mod kind {
    pub const COMBO: u8 = 0x00;
    pub const THRUST_HIGH: u8 = 0x02;
    pub const THRUST_LOW: u8 = 0x04;
    pub const TURN_STRIKE: u8 = 0x06;
    pub const SLOW_STRIKE: u8 = 0x08;
    pub const AFTER_JUMP: u8 = 0x0a;
    /// Charge : élan de 12 pixels par image suivi d'un geste.
    pub const CHARGE: u8 = 0x0c;
    pub const RUN_STRIKE: u8 = 0x0e;
    pub const LOW_BLOW: u8 = 0x10;
    pub const BACK_HOP: u8 = 0x12;
    pub const GUARD_BREAK: u8 = 0x14;
    /// Saut périlleux : bond, demi-tour en l'air, retombée.
    pub const FLIP: u8 = 0x16;
    /// Tourbillon plongeant (coûte des points de vie).
    pub const SPIN_DROP: u8 = 0x18;
    /// Longue glissade.
    pub const LONG_DASH: u8 = 0x1a;
    pub const POSE: u8 = 0x1c;
}

// ---- saut (0x445ba) --------------------------------------------------------------------------

/// 0x445ba : saut. Pas 0 accroupissement, 2 montée (puis vol entier d'un saut sur place), 4 descente
/// d'un saut en avant, 6 récupération, 8 plongée, 0xa rebond, 0xc fin du rebond.
pub fn jump(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => takeoff(g, p),
        2 => rise(g, p),
        4 => descend(g, p),
        6 => recover(g, p),
        8 => dive(g, p),
        0xa => bounce(g, p),
        0xc => bounce_end(g, p),
        _ => {}
    }
    common::flush_pending_effect(g, p);
}

/// Pas 0 : à la fin de l'accroupissement le saut part. Un saut en avant prend le profil 4 si une
/// direction est tenue, 6 sinon.
fn takeoff(g: &mut Ctx, p: Player) {
    cov!();
    if !animation::step(g, p) {
        return;
    }
    next_step(g, p);
    p.set_u8(g, field::AIR_STEERED, 0);
    p.set_u8(g, field::AIR_ATTACK_DONE, 0);
    let kind = p.u8(g, 0x67);
    let x = p.u16(g, Obj::X);
    p.set_u16(g, field::TAKEOFF_X, x);
    let anim = common::by_jump_kind(g, p, 0x15, 0x10, 0xb);
    animation::start(g, p, anim);
    let mut row = kind;
    if kind == 1 {
        row = if matches!(p.u8(g, Player::KEYS_HELD) & 0xf, 1 | 2) { 4 } else { 6 };
    }
    p.set_u8(g, own::JUMP_ROW, row);
    common::launch_jump(g, p, JUMP_SPEEDS, row);
    g.rom(common_rom::JUMP_VOICE, p);
}

/// Pas 2 : montée. Un saut des profils 0..3 se termine ici (retombée au sol) ; un saut en avant passe
/// au profil suivant et au pas 4 quand la vitesse verticale devient négative.
fn rise(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    move_z(g, p);
    common::drift(g, p);
    common::air_steer(g, p, 0x30);
    air_attack(g, p);
    if p.u8(g, field::AIR_ATTACK_DONE) == 2 {
        return;
    }
    let row = p.u8(g, own::JUMP_ROW);
    if row <= 3 {
        if (p.u16(g, Obj::Z) as i16) >= 0 {
            return;
        }
        p.set_u8(g, Player::STEP, 6);
        p.set_u32(g, Obj::Z, 0);
        let anim = common::by_jump_kind(g, p, 0x16, 0x11, 0xc);
        animation::start(g, p, anim);
        g.rom(common_rom::LANDING_SOUND, p);
    } else {
        if (p.u16(g, Obj::VZ) as i16) >= 0 {
            return;
        }
        next_step(g, p);
        p.set_u8(g, own::JUMP_ROW, row + 1);
        common::launch_jump(g, p, JUMP_SPEEDS, row + 1);
    }
}

/// Pas 4 : descente d'un saut en avant.
fn descend(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    move_z(g, p);
    move_x(g, p);
    common::air_steer(g, p, 0x30);
    air_attack(g, p);
    if p.u8(g, field::AIR_ATTACK_DONE) == 2 || (p.u16(g, Obj::Z) as i16) >= 0 {
        return;
    }
    next_step(g, p);
    p.set_u32(g, Obj::Z, 0);
    animation::start(g, p, 0x13);
    g.rom(common_rom::LANDING_SOUND, p);
}

/// Pas 6 : récupération, puis retour à l'état immobile (et chute de l'arme maudite).
fn recover(g: &mut Ctx, p: Player) {
    cov!();
    if animation::step(g, p) {
        to_stand(g, p);
        common::curse_check(g, p);
    }
}

/// Le bouton 1 en l'air donne un coup (une fois par saut) ; sauter avec la touche du bas pendant un
/// saut sans élan lance la plongée.
fn air_attack(g: &mut Ctx, p: Player) {
    cov!();
    if p.u8(g, field::AIR_ATTACK_DONE) != 0 {
        return;
    }
    if p.u8(g, Player::KEYS_NEW) & 0x10 != 0 {
        p.set_u8(g, field::MOVE_STATE, 0);
        p.set_u8(g, field::AIR_ATTACK_DONE, 1);
        let mut anim = if p.u8(g, 0x67) == 2 { 0xc } else { 9 };
        if common::holds_special_weapon(g, p) {
            anim = 0xc;
        }
        if p.u8(g, Player::KEYS_HELD) & 4 != 0 {
            anim = 0xb;
        }
        common::start_with_voice(g, p, anim);
        item_curse(g, p);
        common::aura_hit_countdown(g, p);
        return;
    }
    let row = p.u8(g, own::JUMP_ROW);
    let keys = p.u8(g, Player::KEYS_HELD);
    if !matches!(row, 0 | 3 | 6 | 7) || p.u8(g, Player::KEYS_NEW) & 0x20 == 0 || keys & 4 == 0 {
        return;
    }
    p.set_u8(g, Player::STEP, 8);
    p.set_u8(g, field::MOVE_STATE, 0);
    p.set_u8(g, field::AIR_ATTACK_DONE, 2);
    p.set_u16(g, Obj::VZ, (-0x300i16) as u16);
    p.set_u16(g, Obj::AZ, (-0x60i16) as u16);
    p.set_u32(g, Obj::VX, 0);
    if keys & 3 != 0 {
        set_forward_motion(g, p, 0x600, 0);
    }
    animation::start_plain(g, p, 0x21);
}

/// Pas 8 : plongée ; à l'impact le nain rebondit.
fn dive(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    move_xz(g, p);
    if (p.u16(g, Obj::Z) as i16) >= 0 {
        return;
    }
    next_step(g, p);
    p.set_u32(g, Obj::Z, 0);
    p.set_u8(g, own::JUMP_ROW, 0);
    p.set_u16(g, Obj::VZ, 0x600);
    p.set_u16(g, Obj::AZ, (-0x50i16) as u16);
    if p.u16(g, Obj::VX) != 0 {
        let vx = if p.u8(g, Player::FACING) == 0 { -0x200i16 } else { 0x200 };
        p.set_u16(g, Obj::VX, vx as u16);
    }
    call(g, p, rom::SHAKE, &[(0, 0x10)]);
    g.rom(rom::SOUND_BOUNCE, p);
}

/// Pas 0xa : rebond après la plongée, puis retombée.
fn bounce(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    move_xz(g, p);
    if (p.u16(g, Obj::VZ) as i16) < 0 && p.u8(g, own::JUMP_ROW) == 0 {
        p.set_u8(g, own::JUMP_ROW, 1);
        animation::start(g, p, 0x12);
    }
    if (p.u16(g, Obj::Z) as i16) >= 0 {
        return;
    }
    next_step(g, p);
    p.set_u32(g, Obj::Z, 0);
    animation::start(g, p, 0x11);
    g.rom(common_rom::LANDING_SOUND, p);
}

/// Pas 0xc : fin du rebond.
fn bounce_end(g: &mut Ctx, p: Player) {
    cov!();
    if animation::step(g, p) {
        to_stand(g, p);
    }
}

// ---- attaque (0x4491a) -----------------------------------------------------------------------

/// 0x4491a : attaque du bouton 1. `kind` choisit le coup ; les coups au sol sont suivis d'un contrôle
/// de prolongement (`common::follow_up`).
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
        kind::BACK_HOP => common::back_hop(g, p, 0x19, false),
        kind::GUARD_BREAK => guard_break(g, p),
        kind::FLIP => flip(g, p),
        kind::SPIN_DROP => spin_drop(g, p),
        kind::LONG_DASH => long_dash(g, p),
        kind::POSE => {
            pose(g, p);
            common::follow_up(g, p);
        }
        _ => {}
    }
    common::flush_pending_effect(g, p);
}

/// Avec l'arme particulière (8, 0x18) tous les coups deviennent le coup de base ; `clear_hits` : les
/// coups portés sont oubliés.
fn fall_back_to_combo(g: &mut Ctx, p: Player, clear_hits: bool) -> bool {
    cov!();
    if !common::holds_special_weapon(g, p) {
        return false;
    }
    p.set_u8(g, Player::SUBACTION, kind::COMBO);
    if clear_hits {
        p.set_u8(g, field::HITS, 0);
    }
    true
}

/// Début d'un coup de base (0x449e8) : le rang de l'enchaînement avance si le coup précédent a porté
/// (cinq coups).
fn combo_begin(g: &mut Ctx, p: Player) {
    cov!();
    next_step(g, p);
    p.set_u8(g, field::MOVE_STATE, 0);
    p.set_u8(g, field::COMBO_RELEASED, 0);
    p.set_u8(g, field::COMBO_WINDOW, 6);
    let rank = common::chain_rank(g, p, 1, 4);
    let anim = if common::holds_special_weapon(g, p) { 0x25 } else { rank as u16 };
    common::start_with_voice(g, p, anim);
}

/// Coup de base (0x449ce) : le pas 2 prolonge le coup, les pas 4 et 6 sont une fente en avant.
fn combo(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => combo_begin(g, p),
        2 => {
            common::combo_follow(g, p, 0x15);
        }
        4 => lunge_start(g, p),
        6 => lunge_slide(g, p),
        _ => {}
    }
}

/// La fente part au geste porté (l'événement n'est pas effacé).
fn lunge_start(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    if p.u8(g, field::EVENT) != 0 {
        next_step(g, p);
        set_forward_motion(g, p, 0x400, (-0x38i16) as u16);
    }
}

/// La fente glisse jusqu'à l'arrêt.
fn lunge_slide(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    move_x(g, p);
    if common::momentum_reversed(g, p) {
        to_stand(g, p);
    }
}

/// Coups d'estoc (0x44ace haut, 0x44b48 bas) : un seul geste, de rang 0 ou 1 selon le coup précédent.
fn thrust(g: &mut Ctx, p: Player, hit_bit: u8, base_anim: u16) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            p.set_u8(g, field::MOVE_STATE, 0);
            if fall_back_to_combo(g, p, false) {
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

/// Coup donné en se retournant (0x44bb2) : le pas 2 est le pas 0 d'une entrée par état (combos).
fn turn_strike(g: &mut Ctx, p: Player) {
    cov!();
    let step = p.u8(g, Player::STEP);
    match step {
        0 | 2 => {
            if step == 0 {
                p.set_u8(g, field::MOVE_STATE, 0);
            }
            if fall_back_to_combo(g, p, true) {
                return combo_begin(g, p);
            }
            p.set_u8(g, Player::STEP, 4);
            common::start_with_voice(g, p, if step == 0 { 0x10 } else { 0x18 });
        }
        4 => common::wait_then_stand(g, p),
        _ => {}
    }
}

/// Coup donné en marche lente (0x44c36) : sans prolongement.
fn slow_strike(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            p.set_u8(g, field::MOVE_STATE, 0);
            if fall_back_to_combo(g, p, true) {
                p.set_u8(g, Player::SLOW, 0);
                return combo_begin(g, p);
            }
            next_step(g, p);
            common::start_with_voice(g, p, 0x13);
        }
        2 => common::wait_then_stand(g, p),
        _ => {}
    }
}

/// Coup de retombée (0x44c90).
fn after_jump(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, field::MOVE_STATE, 0);
            let anim = if common::holds_special_weapon(g, p) { 0x26 } else { 0xd };
            animation::start_plain(g, p, anim);
            g.rom(rom::SOUND_STRIKE, p);
            common::weapon_voice(g, p);
            clear_recognizers(g, p);
        }
        2 => common::wait_then_stand(g, p),
        _ => {}
    }
}

/// Charge (0x44cde) : élan, geste (coup porté, effet), glissade.
fn charge(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, 0x5b, 0);
            p.set_u8(g, field::MOVE_STATE, 1);
            p.set_u8(g, field::MOVE_KIND, 1);
            set_forward_motion(g, p, 0xc00, (-0x98i16) as u16);
            animation::start_plain(g, p, 0x14);
            g.rom(rom::SOUND_CHARGE_B, p);
            clear_recognizers(g, p);
        }
        2 => {
            animation::step(g, p);
            if p.u8(g, field::EVENT) != 0 {
                next_step(g, p);
                p.set_u8(g, field::EVENT, 0);
                call(g, p, rom::SLASH_EFFECT, &[(0, 1)]);
                g.rom(rom::SOUND_CHARGE_A, p);
            }
        }
        4 => common::charge_slide(g, p, |g, p| {
            if p.u8(g, field::EVENT) != 0 {
                p.set_u8(g, field::EVENT, 0);
                call(g, p, rom::SLASH_EFFECT, &[(0, 0), (1, 2)]);
                common::weapon_voice(g, p);
            }
        }),
        _ => {}
    }
}

/// Coup donné en courant (0x44da4) : élan de 5 pixels par image, puis arrêt.
fn run_strike(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, field::MOVE_STATE, 0);
            set_forward_motion(g, p, 0x500, (-0x28i16) as u16);
            animation::start_plain(g, p, 0xe);
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

/// Coup de garde (0x44eb2) : sans voix.
fn guard_break(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            p.set_u8(g, field::MOVE_STATE, 0);
            if fall_back_to_combo(g, p, true) {
                return combo_begin(g, p);
            }
            next_step(g, p);
            animation::start_plain(g, p, 0x12);
        }
        2 => common::wait_then_stand(g, p),
        _ => {}
    }
}

/// Saut périlleux (0x44eee) : élan, demi-tour en l'air (le nain change de côté), retombée. Tant que
/// l'animation n'est pas finie, certains pas la font avancer deux fois par image (comme l'original).
fn flip(g: &mut Ctx, p: Player) {
    cov!();
    /// Un pas d'animation ; s'il n'est pas le dernier, un second suit.
    fn double_step(g: &mut Ctx, p: Player) -> bool {
        let done = animation::step(g, p);
        if !done {
            animation::step(g, p);
        }
        done
    }
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, 0x5b, 0);
            p.set_u8(g, field::MOVE_STATE, 1);
            p.set_u8(g, field::MOVE_KIND, 1);
            set_forward_motion(g, p, 0x400, 0);
            animation::start_plain(g, p, 0x16);
            clear_recognizers(g, p);
        }
        2 => {
            move_x(g, p);
            if double_step(g, p) {
                next_step(g, p);
                p.set_u16(g, Obj::VZ, 0x600);
                p.set_u16(g, Obj::AZ, (-0x40i16) as u16);
                p.flip(g);
                animation::start_plain(g, p, 0x17);
                g.rom(rom::SOUND_LEAP_A, p);
                g.rom(rom::SOUND_LEAP_B, p);
            }
        }
        4 => {
            move_z(g, p);
            if (p.u16(g, Obj::VZ) as i16) < 0 {
                next_step(g, p);
                animation::start(g, p, 0x12);
            } else {
                animation::step(g, p);
            }
        }
        6 => {
            move_z(g, p);
            if (p.u16(g, Obj::Z) as i16) < 0 {
                next_step(g, p);
                p.set_u32(g, Obj::Z, 0);
                animation::start(g, p, 0x13);
            } else {
                animation::step(g, p);
            }
        }
        8 => {
            if double_step(g, p) {
                p.set_u8(g, Player::SLOW, 0);
                to_stand(g, p);
            }
        }
        _ => {}
    }
}

/// Tourbillon plongeant (0x44fd0) : bond avec demi-tour, descente en tournoyant ; coûte 12 points de
/// vie au premier contact.
fn spin_drop(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, 0x5b, 0);
            p.set_u8(g, field::MOVE_STATE, 1);
            p.set_u8(g, field::MOVE_KIND, 2);
            p.set_u8(g, own::CONTACT, 0);
            p.set_u8(g, own::WHIRL_PAID, 0);
            p.set_u16(g, Obj::VZ, 0x600);
            p.set_u16(g, Obj::AZ, (-0x60i16) as u16);
            p.flip(g);
            animation::start_plain(g, p, 0x22);
            g.rom(rom::SOUND_SPIN, p);
        }
        2 => {
            move_z(g, p);
            if (p.u16(g, Obj::VZ) as i16) < 0 {
                next_step(g, p);
                animation::start_plain(g, p, 0x23);
            } else {
                animation::step(g, p);
                whirl_cost(g, p);
            }
        }
        4 => {
            move_z(g, p);
            if (p.u16(g, Obj::Z) as i16) > 0 {
                animation::step(g, p);
            } else {
                next_step(g, p);
                animation::start_plain(g, p, 0x24);
                call(g, p, rom::SHAKE, &[(0, 8)]);
            }
        }
        6 => {
            if animation::step(g, p) {
                p.set_u8(g, Player::SLOW, 0);
                to_stand(g, p);
                return;
            }
            animation::step(g, p);
            whirl_cost(g, p);
        }
        _ => {}
    }
}

/// Le premier contact du tourbillon coûte 12 points de vie (au moins un reste).
fn whirl_cost(g: &mut Ctx, p: Player) {
    cov!();
    if p.u8(g, own::WHIRL_PAID) == 0 && p.u8(g, own::CONTACT) & 1 != 0 {
        p.set_u8(g, own::WHIRL_PAID, 1);
        let hp = (p.u16(g, Player::HP) as i16).wrapping_sub(0xc);
        p.set_u16(g, Player::HP, if hp > 0 { hp as u16 } else { 1 });
    }
}

/// Longue glissade (0x450a8) : élan de 0x40 par image pendant 50 images (les 20 premières avancent
/// vraiment), pause, puis retour à l'état immobile.
fn long_dash(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, field::MOVE_STATE, 1);
            p.set_u8(g, field::MOVE_KIND, 2);
            p.set_u8(g, own::COUNTER, 0x32);
            p.set_u32(g, Obj::VX, 0x40_0000);
            if p.u8(g, Player::FACING) == 0 {
                common::negate_horizontal_speed(g, p);
            }
            animation::start_plain(g, p, 0x20);
            g.rom(rom::DASH_EFFECT, p);
            g.rom(rom::SOUND_LEAP_B, p);
            g.rom(rom::SOUND_DASH, p);
            clear_recognizers(g, p);
        }
        2 => {
            let left = p.u8(g, own::COUNTER).wrapping_sub(1);
            p.set_u8(g, own::COUNTER, left);
            if left == 0 {
                next_step(g, p);
                p.set_u8(g, own::COUNTER, 0x10);
                start_stand(g, p);
                return;
            }
            if left > 0x1e {
                move_x(g, p);
            }
            animation::step(g, p);
        }
        4 => {
            let left = p.u8(g, own::COUNTER).wrapping_sub(1);
            p.set_u8(g, own::COUNTER, left);
            if left == 0 {
                p.set_state(g, 0x0200_0002);
                clear_recognizers(g, p);
            }
            animation::step(g, p);
        }
        _ => {}
    }
}

/// Geste vers l'avant (0x4495a), suivi d'une fente comme le coup de base.
fn pose(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, field::MOVE_STATE, 0);
            animation::start_plain(g, p, 0x15);
        }
        2 => lunge_start(g, p),
        4 => lunge_slide(g, p),
        _ => {}
    }
}

// ---- attaque spéciale (0x45148) --------------------------------------------------------------

/// Animations du nain pour les magies lancées (voir `items::Style`).
const STYLE: items::Style = items::Style { cast_anim: 0x1f, bolt_anim: 0x1e, spark: 3, ..items::Style::CLASSIC };

/// 0x45148 : l'objet ou la magie choisie (`field::SPECIAL_KIND`, 0..20), comme le combattant ; les
/// numéros 3 et 11 ne font rien. 0..8 : objets lancés ; 12 : échange d'arme ; 9, 10 et 13..20 : `items`.
pub fn special(g: &mut Ctx, p: Player) {
    cov!();
    let which = p.u8(g, field::SPECIAL_KIND);
    g.set_a5u8(0x5b, which);
    match which {
        0 => items::throw_item(g, p, 0x1c, items::rom::THROW_A),
        1 => items::throw_item(g, p, 0x1c, items::rom::THROW_B),
        2 => items::throw_item(g, p, 0x1d, items::rom::THROW_C),
        4 => items::throw_item(g, p, 0x1d, items::rom::THROW_D),
        5 => items::throw_item(g, p, 0x1d, items::rom::THROW_E),
        6 => items::throw_item(g, p, 0x1b, items::rom::THROW_F),
        7 => items::throw_item(g, p, 0x1b, items::rom::THROW_G),
        8 => items::throw_item(g, p, 0x1a, items::rom::THROW_H),
        12 => common::swap_weapon(g, p, Player::WEAPON_A, false),
        _ => {
            items::run_shared(g, p, which, STYLE);
        }
    }
}

// ---- action 0x12 (0x35b18) -------------------------------------------------------------------

/// Roulade du nain : élan de 3 pixels par image jusqu'à la fin du geste, puis retour à l'état immobile.
pub fn roll(g: &mut Ctx, p: Player) {
    cov!();
    if p.u8(g, Player::STEP) == 0 {
        next_step(g, p);
        common::roll_start(g, p, 0x300);
        return;
    }
    if animation::step(g, p) {
        to_stand(g, p);
    } else {
        move_x(g, p);
    }
}

action_native!(JUMP, 0x445ba, "dwarf_jump", jump);
action_native!(ATTACK, 0x4491a, "dwarf_attack", attack);
action_native!(SPECIAL, 0x45148, "dwarf_special", special);
action_native!(DASH, 0x45988, "dwarf_dash", common::dash);
action_native!(ROLL, 0x35b18, "dwarf_roll", roll);
