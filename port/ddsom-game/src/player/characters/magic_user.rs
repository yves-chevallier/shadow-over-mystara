//! Le magicien (personnages 8 et 9 : les deux variantes ont le même code).
//!
//! Quatre actions, atteintes par `characters::run` :
//! - `jump` (0x45a78) : le saut commun (`common::jump`) avec un coup de bâton en l'air ;
//! - `attack` (0x45cfc) : le bouton 1 ; `Player::SUBACTION` choisit le coup (voir `kind`) ;
//! - `special` (0x4619e) : l'objet ou la magie choisie (`field::SPECIAL_KIND`, 0..0x24) ;
//! - `dash` (0x4879a) : l'action 0x10, glissade propre au magicien.
//!
//! L'arme du magicien est dans `WEAPON_B` (`+0x10e`) : un bâton ou l'une des quatre baguettes (0x24..0x27)
//! qui tirent au geste porté (`rod_shot`). Les objets lancés, la foudre, le mur et les magies 13..20 sont
//! ceux des autres personnages (`items`) ; les magies 0x18..0x24 lui sont propres. Les routines
//! d'origine qui créent tirs, effets et sons restent appelées telles quelles.

use super::common::{self, call, countdown, field, is_cancelled, next_step, take_event};
use super::foes::{self, Boost, Damage, Live, Strike};
use super::items;
use super::super::actions::to_stand;
use super::super::combos::clear_recognizers;
use super::super::walk::{move_x, move_xz, move_z};
use super::super::{animation, Ctx, Player};
use crate::objects::Obj;

/// Routines d'origine appelées telles quelles : tirs, effets joueur, sons, palettes et effets d'écran.
mod rom {
    pub const SOUND_STRIKE: u32 = 0xe1e80;
    pub const SOUND_STRIKE_B: u32 = 0xe1de0;
    pub const SLIDE_SOUND: u32 = 0xe78e8;
    /// Interface des baguettes : mise à jour de la liste d'objets (D0 = reste, D1 = baguette) et son.
    pub const ROD_LIST: u32 = 0xb96c0;
    pub const ROD_SOUND: u32 = 0xcc352;
    /// Palettes du joueur : lumière, normale.
    pub const PALETTE_LIGHT: u32 = 0x3af8;
    pub const PALETTE_NORMAL: u32 = 0x3afe;
    /// Effets d'écran des magies.
    pub const SCREEN_FLASH_END: u32 = 0x1abe0;
    pub const SCREEN_RIPPLE: u32 = 0x1aaa0;
    pub const SCREEN_FLASH_A: u32 = 0x1ab56;
    pub const SCREEN_FLASH_B: u32 = 0x1ab08;
    pub const SCREEN_GLOW: u32 = 0x1aaba;
    pub const SCREEN_SHATTER: u32 = 0x1ac86;
    /// Effets joueur (pool PEFF) des magies.
    pub const EFFECT_SPARKS: u32 = 0x4dc9a;
    pub const EFFECT_SHOWER: u32 = 0x4dcda;
    pub const EFFECT_BOLT: u32 = 0x4b7a0;
    pub const EFFECT_FROST: u32 = 0x4bc42;
    pub const EFFECT_FROST_ALL: [u32; 4] = [0x52b54, 0x4bb68, 0x4bbbe, 0x4bbf6];
    pub const EFFECT_QUAKE: u32 = 0x4b340;
    pub const EFFECT_QUAKE_B: u32 = 0x4f132;
    pub const EFFECT_SUMMON: u32 = 0x4ccac;
    pub const EFFECT_FLARE: u32 = 0x4cd8e;
    pub const EFFECT_STORM: u32 = 0x4d020;
    pub const EFFECT_STORM_B: u32 = 0x51aa4;
    pub const EFFECT_RAIN: u32 = 0x4d660;
    pub const EFFECT_RAIN_B: u32 = 0x54274;
    pub const EFFECT_PORTAL: u32 = 0x4d3c6;
    pub const EFFECT_PORTAL_B: u32 = 0x51872;
    pub const EFFECT_PORTAL_C: u32 = 0x4cfee;
    pub const EFFECT_PORTAL_TEST: u32 = 0x4d064;
    pub const EFFECT_WALL: u32 = 0x4d2c0;
    pub const EFFECT_WAVE: u32 = 0x4dfe2;
    pub const EFFECT_WITCH: u32 = 0x53918;
    pub const EFFECT_WITCH_A: u32 = 0x53954;
    pub const EFFECT_WITCH_B: u32 = 0x539e4;
}

/// Table du magicien (données de la ROM) : vitesses des sauts (quatre mots par type de saut).
const JUMP_SPEEDS: u32 = 0x10_459e;

/// Sous-actions du bouton 1 (`Player::SUBACTION`), toutes paires.
mod kind {
    /// Coup de bâton, éventuellement suivi d'une fente.
    pub const STAFF: [u8; 8] = [0x00, 0x02, 0x04, 0x0c, 0x14, 0x16, 0x18, 0x1a];
    pub const TURN_STRIKE: u8 = 0x06;
    pub const SLOW_STRIKE: u8 = 0x08;
    pub const AFTER_JUMP: u8 = 0x0a;
    pub const RUN_STRIKE: u8 = 0x0e;
    pub const LOW_BLOW: u8 = 0x10;
    pub const BACK_HOP: u8 = 0x12;
    /// Fente seule.
    pub const LUNGE: u8 = 0x1c;
}

// ---- saut (0x45a78) --------------------------------------------------------------------------

/// Le saut du magicien : table de vitesses 0x10459e, gravité plus faible après une course, geste en l'air
/// au pas 0xa pendant lequel le saut est suspendu.
const JUMP_KIT: common::JumpKit = common::JumpKit {
    speeds: JUMP_SPEEDS,
    air_attack,
    busy: |g, p| p.u8(g, field::AIR_ATTACK_DONE) != 0,
    apex_gravity: Some((-0x10, -0x38)),
    extra_step,
    ..common::JumpKit::PLAIN
};

/// 0x45a78 : saut.
pub fn jump(g: &mut Ctx, p: Player) {
    cov!();
    common::jump(g, p, &JUMP_KIT);
}

/// 0x45c96 : en l'air, le bouton 1 donne un coup de bâton (une fois par saut, pas 0xa).
fn air_attack(g: &mut Ctx, p: Player) {
    cov!();
    if !common::air_attack_pressed(g, p) {
        return;
    }
    p.set_u8(g, Player::STEP, 0xa);
    p.set_u8(g, field::MOVE_STATE, 0);
    p.set_u8(g, field::AIR_ATTACK_DONE, 1);
    let mut anim = if p.u8(g, 0x67) == 2 { 4 } else { 1 };
    if p.u8(g, Player::KEYS_HELD) & 4 != 0 {
        anim = 3;
    }
    animation::start_plain(g, p, anim);
    common::weapon_voice_b(g, p);
    common::aura_hit_countdown(g, p);
}

/// Pas 0xa du saut (0x45c12) : le coup de bâton en l'air ; à l'atterrissage le saut reprend à la
/// récupération (pas 8).
fn extra_step(g: &mut Ctx, p: Player, step: u8) {
    cov!();
    if step != 0xa {
        return;
    }
    animation::step(g, p);
    move_xz(g, p);
    if (p.u16(g, Obj::Z) as i16) < 0 {
        p.set_u8(g, Player::STEP, 8);
        common::land(g, p);
    } else {
        rod_shot(g, p);
    }
}

/// 0x46044 : au geste porté, la baguette tenue (`WEAPON_B` 0x24..0x27) tire. Une baguette épuisée
/// (drapeau de réserve `+0x18b..+0x191` à zéro) redevient un bâton (0x22) et disparaît de la liste d'objets.
fn rod_shot(g: &mut Ctx, p: Player) {
    cov!();
    if !take_event(g, p) {
        return;
    }
    // (baguette, tir, drapeau de réserve, repère de l'interface, son de l'interface)
    const RODS: [(u8, Option<u32>, u32, u32, u32); 4] = [
        (0x24, Some(0x5336a), 0x18c, 0, 0xa),
        (0x25, Some(0x53478), 0x18b, 2, 8),
        (0x26, Some(0x535f2), 0x18d, 4, 9),
        (0x27, None, 0x191, 6, 0xb),
    ];
    for (weapon, shot, flag, tag, sound) in RODS {
        if p.u8(g, Player::WEAPON_B) != weapon {
            continue;
        }
        if let Some(shot) = shot {
            g.rom(shot, p);
        }
        if p.u8(g, flag) != 0 {
            continue;
        }
        p.set_u8(g, Player::WEAPON_B, 0x22);
        for i in 0..6u32 {
            if p.u8(g, 0x140 + i) == 0x50 {
                p.set_u8(g, 0x140 + i, 0x56);
                p.set_u8(g, 0x148 + i, 0);
                call(g, p, rom::ROD_LIST, &[(0, 5 - i), (1, tag)]);
                call(g, p, rom::ROD_SOUND, &[(0, sound)]);
                return;
            }
        }
        return;
    }
}

// ---- attaque (0x45cfc) -----------------------------------------------------------------------

/// 0x45cfc : attaque du bouton 1. Pas de prolongement ni d'effet en attente ; la plupart des coups
/// finissent par `rod_shot`.
pub fn attack(g: &mut Ctx, p: Player) {
    cov!();
    let sub = p.u8(g, Player::SUBACTION);
    match sub {
        s if kind::STAFF.contains(&s) => staff(g, p),
        kind::TURN_STRIKE => turn_strike(g, p),
        kind::SLOW_STRIKE => simple_strike(g, p, 9, true),
        kind::AFTER_JUMP => after_jump(g, p),
        kind::RUN_STRIKE => run_strike(g, p),
        kind::LOW_BLOW => simple_strike(g, p, 8, false),
        kind::BACK_HOP => common::back_hop(g, p, 0xb, false),
        kind::LUNGE => lunge(g, p),
        _ => {}
    }
}

/// Vitesse de la fente : 4 pixels par image qui diminuent, vers l'avant.
fn start_lunge(g: &mut Ctx, p: Player) {
    cov!();
    common::set_forward_motion(g, p, 0x400, (-0x38i16) as u16);
    animation::start_plain(g, p, 0xd);
    g.rom(rom::SOUND_STRIKE, p);
}

/// Coup de bâton (0x45d88) : un coup, puis, si le bouton reste appuyé et que le coup a porté, une fente.
fn staff(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, field::MOVE_STATE, 0);
            p.set_u8(g, field::COMBO_RELEASED, 0);
            p.set_u8(g, field::COMBO_WINDOW, 6);
            animation::start_plain(g, p, 0);
            common::weapon_voice_b(g, p);
        }
        2 => {
            common::combo_window(g, p);
            if !animation::step(g, p) {
                return rod_shot(g, p);
            }
            if p.u8(g, field::COMBO_RELEASED) == 0 && p.u8(g, field::HITS) != 0 {
                next_step(g, p);
                start_lunge(g, p);
            } else {
                to_stand(g, p);
            }
        }
        4 => lunge_slide(g, p),
        _ => {}
    }
}

/// Fente (0x45d28) : élan vers l'avant, jusqu'à l'arrêt.
fn lunge(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, field::MOVE_STATE, 0);
            start_lunge(g, p);
        }
        2 => lunge_slide(g, p),
        _ => {}
    }
}

/// La fente glisse jusqu'à l'arrêt de l'élan.
fn lunge_slide(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    move_x(g, p);
    if common::momentum_reversed(g, p) {
        to_stand(g, p);
    }
}

/// Coup donné en se retournant (0x45e44) : le pas 2 est le pas 0 d'une entrée par état (combos).
fn turn_strike(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 | 2 => {
            let first = p.u8(g, Player::STEP) == 0;
            p.set_u8(g, Player::STEP, 4);
            if first {
                p.set_u8(g, field::MOVE_STATE, 0);
            }
            common::start_with_voice_b(g, p, if first { 7 } else { 0xa });
        }
        4 => wait_then_rod(g, p),
        _ => {}
    }
}

/// Un coup à un geste : animation `anim` avec la voix ; au pas 2 le geste s'achève (avec le tir de la
/// baguette si `rod`).
fn simple_strike(g: &mut Ctx, p: Player, anim: u16, rod: bool) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, field::MOVE_STATE, 0);
            common::start_with_voice_b(g, p, anim);
        }
        2 => {
            if rod {
                wait_then_rod(g, p);
            } else {
                common::wait_then_stand(g, p);
            }
        }
        _ => {}
    }
}

/// Attend la fin du geste puis retourne à l'état immobile ; entre-temps la baguette peut tirer.
fn wait_then_rod(g: &mut Ctx, p: Player) {
    cov!();
    if animation::step(g, p) {
        to_stand(g, p);
    } else {
        rod_shot(g, p);
    }
}

/// Coup de retombée (0x45ed8).
fn after_jump(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, field::MOVE_STATE, 1);
            p.set_u8(g, field::MOVE_KIND, 1);
            animation::start_plain(g, p, 0xc);
            g.rom(rom::SOUND_STRIKE_B, p);
            clear_recognizers(g, p);
        }
        2 => common::wait_then_stand(g, p),
        _ => {}
    }
}

/// Coup donné en courant (0x45f1c) : élan de 5 pixels par image, puis arrêt.
fn run_strike(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, field::MOVE_STATE, 0);
            common::set_forward_motion(g, p, 0x500, (-0x28i16) as u16);
            animation::start_plain(g, p, 5);
            clear_recognizers(g, p);
            common::weapon_voice_b(g, p);
        }
        2 => {
            if common::run_strike_slide(g, p, 6) {
                rod_shot(g, p);
            }
        }
        4 => wait_then_rod(g, p),
        _ => {}
    }
}

// ---- action 0x10 : élan et glissade (0x4879a) ---------------------------------------------------

/// 0x4879a : élan, bond bref (tant que le bouton de saut est tenu), glissade en deux temps.
pub fn dash(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            common::set_forward_motion(g, p, 0xb00, 0);
            animation::start(g, p, 0x1c);
            clear_recognizers(g, p);
        }
        2 => {
            if animation::step(g, p) {
                next_step(g, p);
                p.set_u8(g, 0x1e8, 8);
                animation::start(g, p, 0x1d);
                g.rom(rom::SLIDE_SOUND, p);
            }
        }
        4 => {
            let left = p.u8(g, 0x1e8).wrapping_sub(1);
            p.set_u8(g, 0x1e8, left);
            if left == 0 || p.u8(g, Player::KEYS_HELD) & 0x20 == 0 {
                next_step(g, p);
                p.set_u8(g, 0x1e8, 0xc);
                let brake = if p.u8(g, Player::FACING) != 0 { -0xc0i16 } else { 0xc0 };
                p.set_u16(g, Obj::AX, brake as u16);
            }
            common::dust_jump(g, p);
            move_x(g, p);
        }
        6 => {
            move_x(g, p);
            animation::step(g, p);
            let vx = p.u16(g, Obj::VX) as i16;
            let along = if p.u8(g, Player::FACING) == 0 { vx.wrapping_neg() } else { vx };
            if along as u16 > 0x500 {
                let left = p.u8(g, 0x1e8).wrapping_sub(1);
                p.set_u8(g, 0x1e8, left);
                if (left as i8) >= 0 {
                    common::dust_jump(g, p);
                }
            } else {
                next_step(g, p);
                animation::start(g, p, 0x1e);
            }
        }
        8 => {
            move_x(g, p);
            animation::step(g, p);
            if common::momentum_reversed(g, p) {
                to_stand(g, p);
            }
        }
        _ => {}
    }
}

// ---- attaque spéciale (0x4619e) --------------------------------------------------------------

/// Animations du magicien pour les magies lancées (voir `items::Style`) : la cible de la foudre et du mur
/// est gardée en `+0x1e8`, l'écran est rétabli avec le mode 1.
const STYLE: items::Style = items::Style {
    cast_anim: 0x12,
    bolt_anim: 0x10,
    spark: 4,
    target: 0x1e8,
    thunder_at: 0x60,
    finish_anim: Some(0x13),
    d0_is_step: true,
    undim_mode: 1,
};

/// Mode de rétablissement de l'écran du magicien.
const UNDIM: u16 = 1;

/// L'anneau rouge (n° 16) du magicien : le décompte dure 30 images et l'aura n'est posée qu'à la fin.
const CAST_16: items::Cast = items::Cast {
    frames: 0x1e,
    fire: items::ring_fire,
    on_end: |g, p| items::red_aura(g, p, 0x8000),
    ..items::CAST_13
};

/// La potion de soin (n° 19) du magicien n'initialise pas le clignotement.
const CAST_19: items::Cast = items::Cast { prepare: common::nothing, ..items::CAST_19 };

/// 0x4619e : l'objet ou la magie choisie (`field::SPECIAL_KIND`). Les numéros 2..5, 8, 11 et 21..23
/// n'existent pas pour le magicien : retour à l'état immobile.
pub fn special(g: &mut Ctx, p: Player) {
    cov!();
    let which = p.u8(g, field::SPECIAL_KIND);
    g.set_a5u8(0x5b, which);
    match which {
        0 => items::throw_item(g, p, 0xf, items::rom::THROW_A),
        1 => items::throw_item(g, p, 0xf, items::rom::THROW_B),
        6 => items::throw_item(g, p, 0xe, items::rom::THROW_F),
        7 => items::throw_item(g, p, 0xe, items::rom::THROW_G),
        0xc => common::swap_weapon(g, p, Player::WEAPON_B, false),
        16 => items::cast(g, p, &CAST_16, STYLE),
        19 => items::cast(g, p, &CAST_19, STYLE),
        9 | 10 | 13..=15 | 17 | 18 | 20 => {
            items::run_shared(g, p, which, STYLE);
        }
        0x18 => sparks(g, p),
        0x19 => bolt(g, p),
        0x1a => frost(g, p),
        0x1b => quake(g, p),
        0x1c => storm(g, p),
        0x1d => rain(g, p),
        0x1e => witch(g, p),
        0x1f => curse(g, p),
        0x20 => wave(g, p),
        0x21 => portal(g, p),
        0x22 => summon(g, p),
        0x23 => wall(g, p),
        0x24 => drain(g, p),
        _ => {
            to_stand(g, p);
        }
    }
}

/// Joue des sons de la ROM.
fn sounds(g: &mut Ctx, p: Player, list: &[u32]) {
    for &sound in list {
        g.rom(sound, p);
    }
}

/// Fin du sort au pas final : l'écran est rétabli, sans toucher au drapeau d'annulation.
fn finish_kept(g: &mut Ctx, p: Player) {
    cov!();
    common::end_spell_keep_cancel_mode(g, p, UNDIM);
}

/// Dernier pas de la plupart des sorts : l'animation finit, puis le sort s'achève (sans rétablir le
/// drapeau d'annulation).
fn end_when_done(g: &mut Ctx, p: Player) {
    cov!();
    if animation::step(g, p) {
        finish_kept(g, p);
    }
}

/// Pas de sortie des sorts annulés : l'animation avance, le décompte diminue et à zéro le sort s'achève
/// (drapeau d'annulation rétabli).
fn linger(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    if countdown(g, p) {
        common::end_spell_mode(g, p, UNDIM);
    }
}

/// Mot de la ROM (code) indexé par le niveau du lanceur.
fn by_level(g: &mut Ctx, p: Player, table: u32) -> u16 {
    let level = p.u8(g, Player::LEVEL) as u32;
    g.prog16(table + 2 * level)
}

/// Octets (code) : numéro de la variante du geste de la magie 0x18 par niveau.
const SPARK_GESTURES: u32 = 0x46a46;

/// 0x469b0, n° 0x18 (h) : gerbe d'étincelles, qui alterne deux variantes à chaque geste.
fn sparks(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => common::start_cast(g, p, 0x3c, Some(0x151), 0x36),
        2 => {
            animation::step(g, p);
            if take_event(g, p) {
                g.rom(rom::EFFECT_SPARKS, p);
                g.rom(0xe84c6, p);
            }
            if countdown(g, p) {
                next_step(g, p);
                p.set_u8(g, 0x1e8, 0);
                let level = p.u8(g, Player::LEVEL) as u32;
                let gesture = g.prog8(SPARK_GESTURES + level) as u16;
                animation::start_plain(g, p, 0x36 + gesture);
                g.rom(0xe2f3a, p);
            }
        }
        4 => {
            if p.u8(g, field::EVENT) != 0 {
                let side = p.u8(g, 0x1e8) ^ 1;
                p.set_u8(g, 0x1e8, side);
                if is_cancelled(g, p) {
                    p.set_u8(g, Player::STEP, 8);
                    common::cancel_cast(g, p, 0x1c);
                    return linger_quiet(g, p);
                }
                call(g, p, rom::EFFECT_SHOWER, &[(0, (side == 0) as u32)]);
                p.set_u8(g, field::EVENT, 0);
            }
            if animation::step(g, p) {
                next_step(g, p);
                animation::start_plain(g, p, 0x3c);
            }
        }
        6 => end_when_done(g, p),
        8 => linger_quiet(g, p),
        _ => {}
    }
}

/// Pas 8 de la gerbe annulée (0x46ae6) : le décompte diminue ; à zéro une dernière image d'animation
/// puis la fin du sort.
fn linger_quiet(g: &mut Ctx, p: Player) {
    cov!();
    if countdown(g, p) {
        animation::step(g, p);
        common::end_spell_mode(g, p, UNDIM);
    }
}

/// 0x46b1e, n° 0x19 (h) : éclair en deux temps, puis une pose finale.
fn bolt(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => common::start_cast(g, p, 0x28, Some(0x153), 0x1c),
        2 => {
            animation::step(g, p);
            if take_event(g, p) {
                call(g, p, rom::EFFECT_BOLT, &[(0, 0)]);
                g.rom(rom::PALETTE_LIGHT, p);
                g.rom(0xe2a34, p);
            }
            if countdown(g, p) {
                next_step(g, p);
                p.set_u16(g, field::TIMER, 0x30);
                animation::start_plain(g, p, 0x1d);
            }
        }
        4 => {
            animation::step(g, p);
            if take_event(g, p) {
                g.rom(0xe88d0, p);
                if is_cancelled(g, p) {
                    p.set_u8(g, Player::STEP, 0xa);
                    common::cancel_cast(g, p, 0x1d);
                    return linger(g, p);
                }
                call(g, p, rom::EFFECT_BOLT, &[(0, 1)]);
            }
            if countdown(g, p) {
                next_step(g, p);
                p.set_u16(g, field::TIMER, 0x5a);
                animation::start_plain(g, p, 0x1e);
                sounds(g, p, &[0xe893a, 0xe2f7a]);
            }
        }
        6 => {
            animation::step(g, p);
            if countdown(g, p) {
                next_step(g, p);
                animation::start_plain(g, p, 0x1f);
            }
        }
        8 => end_when_done(g, p),
        0xa => linger(g, p),
        _ => {}
    }
}

/// 0x46c92, n° 0x1a (h) : gelée. Pendant la phase finale le joueur règle l'intensité avec haut et bas ;
/// l'écran tremble puis ses couleurs sont rétablies.
fn frost(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => common::start_cast(g, p, 0x2c, Some(0x153), 0x20),
        2 => {
            if countdown(g, p) {
                next_step(g, p);
                p.set_u16(g, field::TIMER, 0x28);
                animation::start_plain(g, p, 0x21);
            } else {
                animation::step(g, p);
            }
        }
        4 => {
            animation::step(g, p);
            if take_event(g, p) {
                call(g, p, rom::EFFECT_FROST, &[(0, 0)]);
                sounds(g, p, &[0xe2a34, 0xe860c]);
            }
            if countdown(g, p) {
                next_step(g, p);
                p.set_u8(g, 0x1e8, 0x10);
                p.set_u8(g, 0x1e9, 2);
                g.c.d[0] = 0;
                p.set_u16(g, field::TIMER, 0x78);
                animation::start_plain(g, p, 0x22);
                g.rom(0xe2fba, p);
            }
        }
        6 => {
            if !animation::step(g, p) {
                return;
            }
            next_step(g, p);
            p.set_u8(g, 0x1eb, 0);
            p.set_u8(g, 0x1ea, 4);
            p.set_u8(g, 0x1e8, 0x10);
            p.set_u8(g, 0x1e9, 2);
            animation::start_plain(g, p, 0x23 + 2);
            if is_cancelled(g, p) {
                p.set_u8(g, Player::STEP, 0xc);
                common::cancel_cast(g, p, 0x1e);
                return frost_wait(g, p);
            }
            for effect in rom::EFFECT_FROST_ALL {
                g.rom(effect, p);
            }
            g.rom(rom::SCREEN_SHATTER, p);
        }
        8 => {
            common::blink_screen(g, p);
            if countdown(g, p) {
                next_step(g, p);
                p.set_u8(g, 0x1eb, 0);
                animation::start_plain(g, p, 0x28);
                return;
            }
            common::flash_screen(g, p);
            common::intensity_keys(g, p, 0x23);
        }
        0xa => {
            if animation::step(g, p) {
                frost_end(g, p);
            }
        }
        0xc => frost_wait(g, p),
        _ => {}
    }
}

/// Pas 0xc de la gelée annulée : le décompte diminue, puis la gelée s'achève.
fn frost_wait(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    if countdown(g, p) {
        frost_end(g, p);
    }
}

/// Fin de la gelée : les couleurs de l'écran sont rétablies (D0 = 0xa000) avant le retour à l'état immobile.
fn frost_end(g: &mut Ctx, p: Player) {
    cov!();
    common::clear_spell_state(g);
    g.set_a5u16(0x490, 2);
    g.set_a5u8(0x48e, 0xc);
    g.c.d[0] = g.c.d[0] & 0xffff_0000 | 0xa000;
    g.rom(common::rom::PALETTE_FADE, p);
    common::undim_screen_mode(g, p, UNDIM);
    to_stand(g, p);
}

/// Mots (code) : durée du tremblement de terre par niveau.
const QUAKE_DURATIONS: u32 = 0x46fd0;

/// Dégâts du tremblement de terre (0x470e4) sur les chefs puis les ennemis.
const QUAKE_STRIKE: Strike = Strike {
    damage: Damage::Table(0x10_442c, 0x10_440e),
    live: Live::Bit0,
    needs_hp: true,
    grounded: false,
    immune_bit: 4,
    reaction: 0x90,
    random_facing: true,
    boosts: &[Boost::Boss(0xc), Boost::Held(0x20, 2), Boost::Held(0x21, 1), Boost::Armor, Boost::Charm(5)],
    undead_die: true,
    sound: None,
};

/// 0x46f2e, n° 0x1b (h) : tremblement de terre.
fn quake(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            common::begin_cast(g, p, Some(0x30), Some(0x154));
            common::dim_screen(g, p);
            g.rom(rom::SCREEN_RIPPLE, p);
            animation::start_plain(g, p, 0x3d);
        }
        2 => {
            animation::step(g, p);
            if countdown(g, p) {
                next_step(g, p);
                let duration = by_level(g, p, QUAKE_DURATIONS);
                p.set_u16(g, field::TIMER, duration);
                animation::start_plain(g, p, 0x3e);
                g.rom(0xe2ffa, p);
            } else if take_event(g, p) {
                call(g, p, rom::EFFECT_QUAKE, &[(0, 0)]);
                g.rom(rom::PALETTE_NORMAL, p);
                g.rom(0xe8db4, p);
            }
        }
        4 => {
            animation::step(g, p);
            if take_event(g, p) {
                if is_cancelled(g, p) {
                    p.set_u8(g, Player::STEP, 8);
                    common::cancel_cast(g, p, 0x1f);
                    common::item_spark(g, p, 0x20);
                    g.rom(rom::SCREEN_FLASH_END, p);
                    return linger(g, p);
                }
                call(g, p, rom::EFFECT_QUAKE, &[(0, 1)]);
                g.rom(rom::EFFECT_QUAKE_B, p);
                g.rom(0xe8b22, p);
            }
            if countdown(g, p) {
                next_step(g, p);
                animation::start_plain(g, p, 0x3f);
                g.rom(rom::SCREEN_FLASH_END, p);
            } else if p.u16(g, field::TIMER) == 0x64 {
                foes::strike_foes(g, p, &QUAKE_STRIKE);
            }
        }
        6 => end_when_done(g, p),
        8 => linger(g, p),
        _ => {}
    }
}

/// Dégâts de la tempête à droite (0x47370) et à gauche (0x47750).
const STORM_STRIKE: Strike = Strike {
    damage: Damage::Table(0x10_4468, 0x10_444a),
    live: Live::Bit0,
    needs_hp: true,
    grounded: false,
    immune_bit: 5,
    reaction: 0x60,
    random_facing: true,
    boosts: &[
        Boost::Boss(0xb),
        Boost::Held(0x1f, 2),
        Boost::Held(0x20, 2),
        Boost::Held(0x21, 1),
        Boost::Armor,
        Boost::Charm(4),
    ],
    undead_die: true,
    sound: None,
};

const STORM_STRIKE_LEFT: Strike = Strike {
    immune_bit: 6,
    reaction: 0x74,
    boosts: &[Boost::Held(0x1f, 2), Boost::Held(0x20, 2), Boost::Held(0x21, 1), Boost::Armor],
    sound: Some(0xe98fa),
    ..STORM_STRIKE
};

/// 0x4722a, n° 0x1c (h) : tempête. Deux variantes selon le regard : vers la droite (pas 0 à 6, éclair
/// puis effet 0x4cd8e) ou vers la gauche (pas 8 à 0xe, effet 0x4d020).
fn storm(g: &mut Ctx, p: Player) {
    cov!();
    let step = p.u8(g, Player::STEP);
    match step {
        0 | 8 => {
            let left = step == 8 || p.u8(g, Player::FACING) != 0;
            if step == 0 && left {
                p.set_u8(g, Player::STEP, 8);
            }
            common::start_cast(g, p, if left { 0x3c } else { 0x30 }, Some(0x155), 0x29);
        }
        2 | 0xa => {
            animation::step(g, p);
            if take_event(g, p) {
                call(g, p, rom::EFFECT_BOLT, &[(0, 0)]);
                g.rom(if step == 2 { rom::PALETTE_LIGHT } else { rom::PALETTE_NORMAL }, p);
                g.rom(0xe2a74, p);
                g.rom(if step == 2 { 0xe97b4 } else { 0xe974a }, p);
            }
            if countdown(g, p) {
                next_step(g, p);
                p.set_u16(g, field::TIMER, if step == 2 { 0xc0 } else { 0xb0 });
                animation::start_plain(g, p, 0x2a);
                g.rom(0xe303a, p);
            }
        }
        4 | 0xc => {
            animation::step(g, p);
            if take_event(g, p) {
                if is_cancelled(g, p) {
                    p.set_u8(g, Player::STEP, 0x10);
                    common::cancel_cast(g, p, 0x21);
                    return linger(g, p);
                }
                if step == 4 {
                    call(g, p, rom::EFFECT_FLARE, &[(0, 1)]);
                    g.rom(rom::SCREEN_FLASH_A, p);
                } else {
                    call(g, p, rom::EFFECT_STORM, &[(0, 1)]);
                    g.rom(rom::SCREEN_GLOW, p);
                    g.rom(rom::EFFECT_STORM_B, p);
                }
            }
            if p.u16(g, field::TIMER) == 0x60 {
                foes::strike_foes(g, p, if step == 4 { &STORM_STRIKE } else { &STORM_STRIKE_LEFT });
            }
            if countdown(g, p) {
                next_step(g, p);
                animation::start_plain(g, p, 0x2b);
            }
        }
        6 | 0xe => {
            if animation::step(g, p) {
                g.rom(rom::SCREEN_FLASH_END, p);
                finish_kept(g, p);
            }
        }
        0x10 => linger(g, p),
        _ => {}
    }
}

/// 0x4788a, n° 0x1d (h) : pluie, en deux temps.
fn rain(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => common::start_cast(g, p, 0x2c, Some(0x156), 0x33),
        2 => {
            animation::step(g, p);
            if take_event(g, p) {
                call(g, p, rom::EFFECT_RAIN, &[(0, 0)]);
                g.rom(rom::PALETTE_NORMAL, p);
                sounds(g, p, &[0xe2a34, 0xe9424]);
            }
            if countdown(g, p) {
                next_step(g, p);
                p.set_u16(g, field::TIMER, 0x60);
                animation::start_plain(g, p, 0x34);
                g.rom(0xe317a, p);
            }
        }
        4 => {
            animation::step(g, p);
            if take_event(g, p) {
                if is_cancelled(g, p) {
                    p.set_u8(g, Player::STEP, 8);
                    common::cancel_cast(g, p, 0x22);
                    return linger(g, p);
                }
                call(g, p, rom::EFFECT_RAIN, &[(0, 1)]);
                g.rom(rom::EFFECT_RAIN_B, p);
                g.rom(0xe948e, p);
            }
            if countdown(g, p) {
                next_step(g, p);
                animation::start_plain(g, p, 0x35);
            }
        }
        6 => end_when_done(g, p),
        8 => {
            animation::step(g, p);
            if countdown(g, p) {
                finish_kept(g, p);
            }
        }
        _ => {}
    }
}

/// 0x479e0, n° 0x1e (h) : sorcière. Le sort invoque (0x53918) puis, selon que la sorcière a touché
/// (`+0x1d4`), une phase longue ou courte ; les gestes 1 et 2 déclenchent deux effets.
fn witch(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => common::start_cast(g, p, 0x14, Some(0x154), 0x17),
        2 => {
            if countdown(g, p) {
                next_step(g, p);
                p.set_u8(g, 0x1e8, 0);
                p.set_u32(g, 0x1d0, 0);
                p.set_u16(g, 0x1d4, 0);
                p.set_u16(g, field::TIMER, 0x2c);
                animation::start_plain(g, p, 0x18);
                g.rom(0xe2a34, p);
                g.rom(0xe960c, p);
                return;
            }
            if p.u16(g, field::TIMER) == 0x13 && !is_cancelled(g, p) {
                call(g, p, rom::EFFECT_WITCH, &[(0, 0)]);
            }
            animation::step(g, p);
        }
        4 => {
            if countdown(g, p) {
                next_step(g, p);
                g.rom(rom::PALETTE_LIGHT, p);
                let (timer, anim) = if p.u8(g, 0x1e8) != 0 { (0x80, 0x1a) } else { (0x50, 0x19) };
                p.set_u16(g, field::TIMER, timer);
                animation::start_plain(g, p, anim);
                g.rom(0xe31ba, p);
                g.rom(0xe9676, p);
                return;
            }
            if is_cancelled(g, p) {
                if p.u16(g, field::TIMER) <= 0x10 {
                    p.set_u8(g, Player::STEP, 8);
                    common::cancel_cast(g, p, 0x23);
                    common::item_spark(g, p, 0x24);
                    return linger(g, p);
                }
            } else if p.u16(g, 0x1d4) != 0 {
                p.set_u8(g, 0x1e8, 1);
            }
            animation::step(g, p);
        }
        6 => {
            if countdown(g, p) {
                return finish_kept(g, p);
            }
            match p.u8(g, field::EVENT) {
                1 => {
                    p.set_u8(g, field::EVENT, 0);
                    g.rom(rom::EFFECT_WITCH_A, p);
                }
                2 => {
                    p.set_u8(g, field::EVENT, 0);
                    g.rom(rom::EFFECT_WITCH_B, p);
                }
                _ => {}
            }
            animation::step(g, p);
        }
        8 => linger(g, p),
        _ => {}
    }
}

/// Mots (code) : valeur de `+0x196` posée par le sort 0x1f, par niveau.
const CURSE_DURATIONS: u32 = 0x47c4e;

/// 0x47b88, n° 0x1f (h) : malédiction : à la fin le joueur reçoit un décompte (`+0x196`).
fn curse(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => common::start_cast(g, p, 0x18, Some(0x156), 0x29),
        2 => {
            animation::step(g, p);
            if countdown(g, p) {
                next_step(g, p);
                p.set_u16(g, field::TIMER, 0x30);
                animation::start_plain(g, p, 0x2a);
                g.rom(0xe31fa, p);
            }
        }
        4 => {
            animation::step(g, p);
            if p.u16(g, field::TIMER) == 0x18 {
                g.rom(0xea106, p);
            }
            if countdown(g, p) {
                let timer = by_level(g, p, CURSE_DURATIONS);
                p.set_u16(g, Player::DYING_TIMER, timer);
                common::end_spell_mode(g, p, UNDIM);
            }
        }
        _ => {}
    }
}

/// 0x47c8a, n° 0x20 (h) : vague : deux effets au geste.
fn wave(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => common::start_cast(g, p, 0x60, Some(0x157), 0x40),
        2 => {
            if is_cancelled(g, p) && p.u16(g, field::TIMER) <= 0x50 {
                p.set_u8(g, Player::STEP, 8);
                common::cancel_cast(g, p, 0x25);
                common::item_spark(g, p, 0x26);
                return linger(g, p);
            }
            if countdown(g, p) {
                next_step(g, p);
                p.set_u16(g, field::TIMER, 0x30);
                animation::start_plain(g, p, 0x41);
                g.rom(0xe30ba, p);
            } else if take_event(g, p) {
                call(g, p, rom::EFFECT_WAVE, &[(0, 0)]);
                call(g, p, rom::EFFECT_WAVE, &[(0, 1)]);
                g.rom(rom::PALETTE_LIGHT, p);
                g.rom(0xe9240, p);
            } else {
                animation::step(g, p);
            }
        }
        4 => {
            if countdown(g, p) {
                next_step(g, p);
                animation::start_plain(g, p, 0x3f);
            } else {
                animation::step(g, p);
            }
        }
        6 => end_when_done(g, p),
        8 => linger(g, p),
        _ => {}
    }
}

/// Mots (code) : durée du séjour de la magie 0x21 par niveau, et vitesses horizontales du bond par
/// distance (pas de 16 pixels).
const PORTAL_DURATIONS: u32 = 0x47ee8;
const PORTAL_SPEEDS: u32 = 0x47e78;
const REGROUP_SPEEDS: u32 = 0x4851c;

/// Dégâts de la magie 0x21 (0x48012) : sans test d'état, sans retournement des monstres.
const PORTAL_STRIKE: Strike = Strike {
    damage: Damage::Table(0x10_43f0, 0x10_43d2),
    live: Live::Unchecked,
    needs_hp: true,
    grounded: false,
    immune_bit: 0xc,
    reaction: 0x80,
    random_facing: false,
    boosts: &[Boost::Held(0x1f, 2), Boost::Held(0x20, 2), Boost::Held(0x21, 1), Boost::Armor],
    undead_die: false,
    sound: None,
};

/// Début du bond vers le centre de l'écran (0x47e1e, 0x484c2) : le magicien se tourne vers le centre
/// de l'écran et prend la vitesse (par distance, tous les 16 pixels) de la table `speeds`.
fn aim_at_center(g: &mut Ctx, p: Player, speeds: u32, vz: u16, az: i16) {
    cov!();
    let mut center = g.a5u16(0x506);
    if g.a5u8(0x36) != 0 {
        center = g.a5u16(0x556);
    }
    let center = center.wrapping_add(0xc0);
    let x = p.u16(g, Obj::X);
    p.set_u8(g, Player::FACING, 0);
    if center > x {
        p.set_u8(g, Player::FACING, 0x20);
    }
    let mut distance = center.wrapping_sub(x) as i16;
    if distance < 0 {
        distance = distance.wrapping_neg();
    }
    let mut index = 0u8;
    loop {
        distance = distance.wrapping_sub(0x10);
        if distance < 0 {
            break;
        }
        index = index.wrapping_add(2);
    }
    let speed = g.prog16(speeds + index as u32);
    let speed = if p.u8(g, Player::FACING) != 0 { speed } else { (speed as i16).wrapping_neg() as u16 };
    p.set_u16(g, Obj::VX, speed);
    p.set_u16(g, Obj::VZ, vz);
    p.set_u16(g, Obj::AZ, az as u16);
    animation::start_plain(g, p, 0x2f);
}

/// 0x47dd6, n° 0x21 (h) : portail. Le magicien bondit au centre de l'écran (pas 0 à 2), y reste pendant
/// que les monstres sont blessés (pas 4), puis retombe (pas 6 et 8).
fn portal(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            common::begin_cast(g, p, Some(0x3c), Some(0x155));
            common::dim_screen(g, p);
            aim_at_center(g, p, PORTAL_SPEEDS, 0x560, -0x38);
        }
        2 => {
            move_xz(g, p);
            if !animation::step(g, p) {
                return;
            }
            next_step(g, p);
            if is_cancelled(g, p) {
                p.set_u16(g, field::TIMER, 1);
                animation::start_plain(g, p, 0x30);
                return;
            }
            let duration = by_level(g, p, PORTAL_DURATIONS);
            p.set_u16(g, field::TIMER, duration);
            g.rom(rom::PALETTE_NORMAL, p);
            call(g, p, rom::EFFECT_PORTAL, &[(0, 2)]);
            animation::start_plain(g, p, 0x30);
            g.rom(0xe91d6, p);
            g.rom(0xe2a34, p);
        }
        4 => {
            if !is_cancelled(g, p) {
                animation::step(g, p);
                if take_event(g, p) {
                    call(g, p, rom::EFFECT_PORTAL, &[(0, 0)]);
                    call(g, p, rom::EFFECT_PORTAL, &[(0, 1)]);
                    g.rom(rom::SCREEN_FLASH_B, p);
                    foes::strike_foes(g, p, &PORTAL_STRIKE);
                    g.rom(0xe307a, p);
                    g.rom(0xe9176, p);
                }
            }
            if countdown(g, p) {
                next_step(g, p);
                p.set_u32(g, Obj::VX, 0);
                p.set_u16(g, Obj::VZ, 0);
                if !is_cancelled(g, p) {
                    g.rom(rom::SCREEN_FLASH_END, p);
                }
                animation::start_plain(g, p, 0x31);
            }
        }
        6 => {
            move_z(g, p);
            if (p.u16(g, Obj::Z) as i16) < 0 {
                next_step(g, p);
                p.set_u32(g, Obj::Z, 0);
                animation::start_plain(g, p, 0x32);
            }
        }
        8 => {
            if animation::step(g, p) {
                common::end_spell_mode(g, p, UNDIM);
            }
        }
        _ => {}
    }
}

/// 0x480ec, n° 0x22 (h) : invocation d'une créature (0x4ccac) qui dure 360 images.
fn summon(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => common::start_cast(g, p, 0x16, Some(0x159), 0x14),
        2 => {
            if !countdown(g, p) {
                animation::step(g, p);
                return;
            }
            next_step(g, p);
            if is_cancelled(g, p) {
                p.set_u8(g, Player::STEP, 8);
                p.set_u16(g, field::TIMER, 0x15);
            } else {
                p.set_u16(g, field::TIMER, 0x168);
                g.rom(rom::EFFECT_SUMMON, p);
                g.rom(0xe9cd4, p);
            }
            g.rom(0xe30fa, p);
            animation::start_plain(g, p, 0x15);
        }
        4 => {
            if countdown(g, p) {
                next_step(g, p);
                animation::start_plain(g, p, 0x16);
            } else {
                animation::step(g, p);
            }
        }
        6 => end_when_done(g, p),
        8 => {
            if countdown(g, p) {
                common::end_spell_mode(g, p, UNDIM);
                return;
            }
            let timer = p.u16(g, field::TIMER);
            if timer == 0x10 {
                common::item_spark(g, p, 0x27);
                common::item_spark(g, p, 0x28);
            } else if timer > 0x10 {
                animation::step(g, p);
            }
        }
        _ => {}
    }
}

/// Dégâts du mur de feu (0x48392) : valeur unique, sans test de points de vie.
const WALL_STRIKE: Strike = Strike {
    damage: Damage::Fixed(0x46),
    live: Live::Bit0,
    needs_hp: false,
    grounded: false,
    immune_bit: 0xe,
    reaction: 0x7c,
    random_facing: false,
    boosts: &[Boost::Held(0x20, 2), Boost::Held(0x21, 1), Boost::Armor],
    undead_die: true,
    sound: Some(0xe93ba),
};

/// 0x4822a, n° 0x23 (h) : mur de feu : un effet au geste, puis des dégâts fixes au bon moment.
fn wall(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => common::start_cast(g, p, 0x28, Some(0x159), 0x2c),
        2 => {
            animation::step(g, p);
            if take_event(g, p) {
                call(g, p, rom::EFFECT_WALL, &[(0, 0)]);
                g.rom(rom::PALETTE_LIGHT, p);
                g.rom(0xe92c6, p);
            }
            if countdown(g, p) {
                next_step(g, p);
                if is_cancelled(g, p) {
                    p.set_u8(g, Player::STEP, 8);
                    common::cancel_cast(g, p, 0x29);
                    animation::start_plain(g, p, 0x2d);
                    return;
                }
                p.set_u16(g, field::TIMER, 0x40);
                call(g, p, rom::EFFECT_WALL, &[(0, 1)]);
                animation::start_plain(g, p, 0x2d);
                g.rom(0xe313a, p);
                g.rom(0xe9330, p);
            }
        }
        4 => {
            animation::step(g, p);
            if countdown(g, p) {
                next_step(g, p);
                animation::start_plain(g, p, 0x2e);
            } else if p.u16(g, field::TIMER) == 0x20 {
                foes::strike_foes(g, p, &WALL_STRIKE);
            }
        }
        6 => end_when_done(g, p),
        8 => linger(g, p),
        _ => {}
    }
}

/// 0x4847a, n° 0x24 (h) : sacrifice. Le magicien bondit au centre de l'écran, ouvre un portail, puis ses
/// baguettes disparaissent et tous les joueurs en vie tombent à un point de vie.
fn drain(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            common::begin_cast(g, p, Some(0x3c), None);
            p.set_u8(g, 0x1ef, 0);
            common::dim_screen(g, p);
            aim_at_center(g, p, REGROUP_SPEEDS, 0x200, -8);
        }
        2 => {
            move_xz(g, p);
            if animation::step(g, p) {
                next_step(g, p);
                p.set_u16(g, field::TIMER, 0x120);
                g.rom(rom::PALETTE_NORMAL, p);
                call(g, p, rom::EFFECT_PORTAL, &[(0, 2)]);
                animation::start_plain(g, p, 0x30);
                g.rom(0xe91d6, p);
            }
        }
        4 => {
            animation::step(g, p);
            if take_event(g, p) {
                g.rom(rom::EFFECT_PORTAL_B, p);
                call(g, p, rom::EFFECT_PORTAL, &[(0, 0)]);
                g.rom(rom::EFFECT_PORTAL_C, p);
                g.rom(0xe323a, p);
            }
            if countdown(g, p) {
                next_step(g, p);
                p.set_u32(g, Obj::VX, 0);
                p.set_u16(g, Obj::VZ, 0);
                animation::start_plain(g, p, 0x31);
                return;
            }
            let timer = p.u16(g, field::TIMER);
            if timer <= 0x100 {
                let tally = p.u8(g, 0x1ef) as u32;
                g.c.d[0] = g.c.d[0] & !0xff | tally;
                if g.rom(rom::EFFECT_PORTAL_TEST, p) {
                    p.set_u8(g, 0x1ef, (tally as u8).wrapping_add(1));
                }
            }
            if timer == 0xe0 {
                sacrifice(g, p);
            }
        }
        6 => {
            move_z(g, p);
            if (p.u16(g, Obj::Z) as i16) < 0 {
                next_step(g, p);
                p.set_u32(g, Obj::Z, 0);
                animation::start_plain(g, p, 0x32);
                p.set_u8(g, Player::WEAPON_B, 0x22);
                for i in 0..6u32 {
                    if p.u8(g, 0x140 + i) == 0x4f {
                        p.set_u8(g, 0x140 + i, 0x56);
                        p.set_u8(g, 0x148 + i, 0);
                    }
                }
            }
        }
        8 => {
            if animation::step(g, p) {
                common::end_spell_mode(g, p, UNDIM);
            }
        }
        _ => {}
    }
}

/// 0x4864c : le sacrifice. Tous les monstres en jeu non gelés sont frappés (réaction 0xa8) : les chefs de
/// type 0xf ou 0x14 perdent 500 points de vie, les autres n'en gardent qu'un ; puis tous les joueurs en
/// vie tombent à un point de vie.
fn sacrifice(g: &mut Ctx, p: Player) {
    cov!();
    use crate::objects::pools::{BOSS, ENEMY};
    let (index, character) = (p.u8(g, Player::INDEX), p.u8(g, Player::CHARACTER));
    for pool in [&BOSS, &ENEMY] {
        for o in pool.iter() {
            if o.u8(g.b, 0) == 0 || o.u8(g.b, 1) == 0 || o.u8(g.b, 0x40) != 0 {
                continue;
            }
            let counter = p.u16(g, 0x190).wrapping_add(2);
            p.set_u16(g, 0x190, counter);
            let hits = o.u8(g.b, 0x9c).wrapping_add(1);
            o.set_u8(g.b, 0x9c, hits);
            o.set_u32(g.b, 4, 0x0400_0000);
            o.set_u8(g.b, 1, 1);
            o.set_u8(g.b, 0x49, 0xa8);
            o.set_u8(g.b, 0x48, index);
            o.set_u8(g.b, 0x4b, character);
            o.set_u16(g.b, 0x54, p.0 as u16);
            o.set_u8(g.b, 0x4a, 0);
            if g.random() & 1 != 0 {
                o.set_u8(g.b, 0x4a, 0x20);
            }
            let kind = o.u8(g.b, Obj::KIND);
            if o.u8(g.b, 0x28) == 4 && matches!(kind, 0xf | 0x14) {
                let hp = (o.u16(g.b, 0x62) as i16).wrapping_sub(0x1f4);
                o.set_u16(g.b, 0x62, if hp < 0 { 0 } else { hp as u16 });
            } else {
                o.set_u16(g.b, 0x62, 1);
            }
            if o.u16(g.b, 0x62) == 0 && o.u8(g.b, 0x93) != 0 {
                p.set_u8(g, 0x18a, 1);
                let loot = o.u8(g.b, 0x9a);
                g.set_a5u8(0x4f, loot);
            }
            let bar = (index as u32) << 8 | kind as u32;
            let (max, hp, class) = (o.u16(g.b, 0x60) as u32, o.u16(g.b, 0x62) as u32, o.u8(g.b, 0x28) as u32);
            call(g, p, 0x19dce, &[(0, bar), (1, max), (2, hp), (3, class)]);
        }
    }
    for q in Player::all() {
        if q.u8(g, Player::ACTIVE) != 0 && q.u16(g, Player::HP) != 0 {
            q.set_u16(g, Player::HP, 1);
        }
    }
}

action_native!(JUMP, 0x45a78, "mage_jump", jump);
action_native!(ATTACK, 0x45cfc, "mage_attack", attack);
action_native!(SPECIAL, 0x4619e, "mage_special", special);
action_native!(DASH, 0x4879a, "mage_dash", dash);
