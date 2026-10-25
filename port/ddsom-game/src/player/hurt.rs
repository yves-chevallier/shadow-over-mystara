//! Réactions aux coups : joueur touché (`+5` = 4), joueur à terre (`+4` = 4), mort (P4).
//!
//! Quand un coup touche un joueur (`combat`), il passe dans l'état « touché » (`0x02040000`, voir
//! `states`) ou, s'il n'a plus de points de vie, « à terre » (`0x0400xxxx`). L'octet `+6` choisit alors la
//! *réaction* (recul, projection, chute...), chacune étant un petit automate dont `+7` est le pas.
//! Réactions et pas sont des routines du code d'origine, atteintes par des tables de décalages ;
//! beaucoup de pas sont partagés (projection en l'air, rebond, relevé, clignotement d'invulnérabilité).
//!
//! La table `HANDLERS` donne, pour une adresse d'origine, la version Rust ; toute adresse absente
//! est exécutée telle quelle (`Ctx::rom`). Les tables d'origine restent donc la source de vérité de
//! l'aiguillage et les réactions non réécrites continuent de fonctionner.

use super::actions::to_stand;
use super::walk::{clamp_to_screen, move_x, move_xyz, move_xz};
use super::{animation, ram, Ctx, Player};
use crate::mem::{wr16, wr8, A5};
use crate::objects::anim::{load_entry, script_of};
use crate::objects::lists::ACTORS;
use crate::objects::Obj;

type Handler = fn(&mut Ctx, Player);

/// Routines d'origine encore appelées telles quelles.
mod rom {
    pub const BEFORE_HURT_A: u32 = 0xe2100;
    pub const BEFORE_HURT_B: u32 = 0xdb6c0;
    /// Collision avec le décor.
    pub const TERRAIN: u32 = 0x33fe4;
    /// Effets de la mort (son, points).
    pub const DEATH_EFFECT: u32 = 0xe21a0;
    /// Poussière et son à l'atterrissage.
    pub const LANDING_EFFECT: u32 = 0xe7a12;
    /// Poussière qui accompagne un glissement.
    pub const SLIDE_DUST: u32 = 0xb10c8;
    /// Sons de relevé.
    pub const GET_UP_SOUND_A: u32 = 0xe22e8;
    pub const GET_UP_SOUND_B: u32 = 0xe2240;
}

/// Tables de pas (mots : décalages relatifs à la table, indexés par `+7`).
mod steps {
    pub const R6: u32 = 0x3a4d2;
    pub const R14: u32 = 0x3a858;
    pub const R26: u32 = 0x3aa9e;
    pub const R32: u32 = 0x3ad3a;
    pub const DOWN_6: u32 = 0x3b86e;
    pub const DOWN_A: u32 = 0x3b890;
}

/// Tables d'aiguillage par action (`+6`).
const HURT_ACTIONS: u32 = 0x3a0f4;
const DOWN_ACTIONS: u32 = 0x3b820;

/// Animations de réaction (jeux d'entrées longues par personnage, lus comme des données).
const HURT_ANIMATIONS: u32 = 0x3bb52;

/// Réactions et pas réécrits : adresse d'origine -> routine Rust.
const HANDLERS: &[(u32, Handler)] = &[
    (0x3a136, reaction_0),
    (0x3a79e, reaction_10),
    (0x3a4c6, |g, p| jump(g, p, steps::R6)),
    (0x3a84c, |g, p| jump(g, p, steps::R14)),
    (0x3aa92, |g, p| jump(g, p, steps::R26)),
    (0x3ad2e, |g, p| jump(g, p, steps::R32)),
    (0x3b862, |g, p| jump(g, p, steps::DOWN_6)),
    (0x3b884, |g, p| jump(g, p, steps::DOWN_A)),
    (0x3a4e8, |g, p| launch_with_anim(g, p, 6)),
    (0x3a4f2, |g, p| launch_with_anim(g, p, 0xa)),
    (0x3a2de, |g, p| next_animation(g, p, 7)),
    (0x3a2e4, |g, p| next_animation(g, p, 0xb)),
    (0x3a2f8, |g, p| fall_to_apex(g, p, 8)),
    (0x3a2fe, |g, p| fall_to_apex(g, p, 0xc)),
    (0x3a328, |g, p| land(g, p, 9)),
    (0x3a32e, |g, p| land(g, p, 0xd)),
    (0x3a522, bounce_up),
    (0x3a362, bounce_up_small),
    (0x3a384, land_with_effect),
    (0x3a544, slide_start),
    (0x3a3ae, slide_start_short),
    (0x3a3e2, slide),
    (0x3a424, get_up),
    (0x3a466, finish_get_up),
    (0x3a472, invulnerable_blink),
    (0x3a2ae, launch_a),
    (0x3aaac, launch_b),
    (0x3aae8, fall_b),
    (0x3ad4c, launch_c),
    (0x3ad66, fall_c),
    (0x3adbe, shock_start),
    (0x3ae0e, shock_fall),
    (0x3ae32, shock_struggle),
    (0x3ae7a, shock_release),
    (0x3aeaa, dizzy_start),
    (0x3aeca, dizzy),
    (0x3b1fa, sit_start),
    (0x3b264, sit_fall),
    (0x3b27e, |g, p| {
        if animation::step(g, p) {
            sit_down(g, p);
        }
    }),
    (0x3b28a, sit_struggle),
    (0x3b44c, held),
    (0x3a204, |g, p| {
        if is_dead(g, p) {
            knock_down(g, p);
            return downed(g, p);
        }
        next_step(g, p);
        set_launch(g, p, 0x200, 0x200);
        face_attacker(g, p);
        hurt_animation(g, p, 3);
    }),
    (0x3a238, |g, p| {
        animation::step(g, p);
        if p.u8(g, 0x33) != 0 {
            next_step(g, p);
        }
    }),
    (0x3a24c, |g, p| {
        move_xz(g, p);
        if altitude(g, p) < 0 {
            p.set_u32(g, Obj::Z, 0);
            p.set_u16(g, Obj::ANIM_TIMER, 1);
            next_step(g, p);
        }
    }),
    (0x3a268, finish_get_up),
    (0x3a2a4, |g, p| {
        hurt_animation(g, p, 6);
        launch_common(g, p);
    }),
    (0x3b040, |g, p| {
        let left = p.u8(g, Player::TIMER_6F).wrapping_sub(1);
        p.set_u8(g, Player::TIMER_6F, left);
        if left == 0 {
            next_step(g, p);
            hurt_animation(g, p, 16);
        }
    }),
    (0x3b074, |g, p| {
        next_step(g, p);
        hurt_animation(g, p, 30);
    }),
    (0x3b07e, |g, p| {
        if animation::step(g, p) {
            p.set_u8(g, Player::ACTIVE, 1);
            p.set_u8(g, Player::INVULN, 0);
            next_step(g, p);
            p.set_u8(g, Player::TIMER_6F, 96);
        }
    }),
    (0x3b0a0, |g, p| {
        if struggle_out(g, p) {
            next_step(g, p);
            hurt_animation(g, p, 32);
        }
    }),
    (0x3b054, |g, p| {
        let ended = animation::step(g, p);
        if ended || (p.u8(g, Player::SLOW) != 0 && p.u8(g, 0x33) != 0) {
            next_step(g, p);
        }
    }),
    (0x3a656, |g, p| hurl(g, p, 0x200, 6)),
    (0x3b4e0, |g, p| {
        next_step(g, p);
        p.set_u32(g, Obj::VX, 0);
        p.set_u8(g, Player::INVULN, 50);
        p.set_u16(g, Obj::VZ, 0x200);
        p.set_u16(g, Obj::AZ, (-64i16) as u16);
        hurt_animation(g, p, 22);
        knock_down_if_dead(g, p);
    }),
    (0x3b504, crash_landing),
    (0x3a68a, |g, p| hurled(g, p, 8, 16)),
    (0x3a706, |g, p| hurled_landing(g, p, 9, false)),
    (0x3a75a, |g, p| hurled_slide(g, p, 26)),
    (0x3ab7c, |g, p| hurl(g, p, 0x100, 24)),
    (0x3a8de, |g, p| hurl(g, p, 0x200, 10)),
    (0x3a912, |g, p| hurled(g, p, 12, 16)),
    (0x3abb0, |g, p| hurled(g, p, 12, 14)),
    (0x3ac2c, |g, p| hurled_landing(g, p, 13, true)),
    (0x3ac86, |g, p| start_slide(g, p, 0x400)),
    (0x3acba, |g, p| hurled_slide(g, p, 24)),
    (0x3acfe, |g, p| {
        if p.u8(g, 0x33) == 0 {
            move_xz(g, p);
            if altitude(g, p) <= 0 {
                p.set_u32(g, Obj::Z, 0);
                next_step(g, p);
                hurt_animation(g, p, 9);
                g.rom(rom::LANDING_EFFECT, p);
                return;
            }
        }
        animation::step(g, p);
    }),
    (0x3b150, |g, p| {
        if animation::step(g, p) {
            next_step(g, p);
        }
    }),
    (0x3a98e, hurt_slow),
    (0x3b2c2, hurt_short),
    (0x3a80a, |g, p| {
        if is_dead(g, p) {
            p.set_u16(g, Player::MAIN, 0x0400);
            p.set_u8(g, Player::ACTIVE, 2);
            return run_at(g, p, 0x3b884);
        }
        next_step(g, p);
        set_launch(g, p, 0x200, 0x200);
        face_attacker(g, p);
        hurt_animation(g, p, 4);
    }),
    (0x3ab34, |g, p| {
        next_step(g, p);
        set_launch(g, p, 0x180, 0x400);
        face_attacker(g, p);
        let anim = 0x16 + (g.a5u8(ram::FRAME_COUNTER) & 1) as u16;
        hurt_animation(g, p, anim);
        knock_down_if_dead(g, p);
    }),
    (0x3af68, |g, p| {
        if rehit(g, p, 4, 0x3a274) {
            return;
        }
        if p.u16(g, Obj::Z) as i16 > 0 {
            next_step(g, p);
            set_launch_keep(g, p, 0x180, 0x400);
            let anim = 0x16 + (g.a5u8(ram::FRAME_COUNTER) & 1) as u16;
            hurt_animation(g, p, anim);
            return face_attacker(g, p);
        }
        p.set_u8(g, Player::STEP, 0x10);
        let freeze = p.u8(g, 0x57);
        p.set_u8(g, Player::HITSTOP, freeze);
        face_attacker(g, p);
        let anim = if p.u8(g, Player::SLOW) != 0 { 2 } else { 0 };
        hurt_animation(g, p, anim);
    }),
    (0x3b0e8, |g, p| {
        if rehit(g, p, 4, 0x3a274) {
            return;
        }
        next_step(g, p);
        set_launch_keep(g, p, 0x200, 0x200);
        let anim = 3 + (g.a5u8(ram::FRAME_COUNTER) & 1) as u16;
        hurt_animation(g, p, anim);
        face_attacker(g, p);
    }),
    (0x3b188, |g, p| {
        if rehit(g, p, 8, 0x3a578) {
            return;
        }
        next_step(g, p);
        set_launch_keep(g, p, 0x280, 0x500);
        hurt_animation(g, p, 6);
        face_attacker(g, p);
    }),
    (0x3a5d4, |g, p| {
        if animation::step(g, p) {
            next_step(g, p);
            p.set_u16(g, Obj::VZ, 0x280);
            p.set_u16(g, Obj::AZ, (-0x40i16) as u16);
            hurt_animation(g, p, 8);
        }
    }),
    (0x3a5f6, |g, p| {
        if animation::step(g, p) {
            next_step(g, p);
            p.set_u8(g, Player::TIMER_6F, 0x14);
            p.set_u16(g, Obj::VX, 0x400);
            p.set_u16(g, Obj::AX, (-0x40i16) as u16);
            if p.faces_left(g) {
                let v = p.u32(g, Obj::VX).wrapping_neg();
                p.set_u32(g, Obj::VX, v);
            }
            hurt_animation(g, p, 0xf);
        }
    }),
];

/// Pas 0 des réactions 0x26, 0x2e et 0x30 : touché de nouveau pendant la réaction (`+0x185`), la réaction
/// `action` reprend à son début (vrai) ; sinon la marque est posée, et sans PV le joueur tombe (vrai).
fn rehit(g: &mut Ctx, p: Player, action: u8, reaction: u32) -> bool {
    if p.u8(g, REHIT) != 0 {
        p.set_u8(g, REHIT, 0);
        p.set_u8(g, Player::ACTION, action);
        run_at(g, p, reaction);
        return true;
    }
    p.set_u8(g, REHIT, 1);
    if is_dead(g, p) {
        p.set_u16(g, Player::MAIN, 0x0400);
        p.set_u8(g, Player::ACTIVE, 2);
        p.set_u8(g, Player::HITSTOP, 1);
        run_at(g, p, 0x3b862);
        return true;
    }
    false
}

/// Marque « touché pendant la réaction » (effacée à la fin de la réaction, `recover`).
const REHIT: u32 = 0x185;

/// Vitesses d'une projection sans toucher au gel du coup (`+0x53`).
fn set_launch_keep(g: &mut Ctx, p: Player, vx: u16, vz: u16) {
    p.set_u16(g, Obj::VX, vx);
    p.set_u16(g, Obj::AX, 0);
    p.set_u16(g, Obj::VZ, vz);
    p.set_u16(g, Obj::AZ, (-0x40i16) as u16);
}

/// 0x3a98e (réaction 0x14, aussi prise par les réactions 0 et 10 en marche lente) : recul en tremblant
/// (`flinch`) ; sans PV, chute (0x3b8a6).
fn hurt_slow(g: &mut Ctx, p: Player) {
    if p.u8(g, Player::STEP) != 0 {
        return flinch(g, p);
    }
    if is_dead(g, p) {
        p.set_u16(g, Player::MAIN, 0x0400);
        p.set_u8(g, Player::ACTIVE, 2);
        g.rom(rom::DEATH_EFFECT, p);
        return run_at(g, p, 0x3b8a6);
    }
    next_step(g, p);
    p.set_u16(g, Obj::VX, 0x280);
    p.set_u16(g, Obj::AX, (-0x50i16) as u16);
    let freeze = p.u8(g, 0x57);
    p.set_u8(g, Player::HITSTOP, freeze);
    p.set_u8(g, Player::TIMER_6F, 8);
    face_attacker(g, p);
    hurt_animation(g, p, 2);
}

/// 0x3b2c2 (réaction 0x2a) : courte secousse sur place, puis retour à la pose ; sans PV, chute (0x3b8c4).
fn hurt_short(g: &mut Ctx, p: Player) {
    if p.u8(g, Player::STEP) != 0 {
        if animation::step(g, p) {
            recover(g, p);
        }
        return;
    }
    if is_dead(g, p) {
        p.set_u16(g, Player::MAIN, 0x0400);
        p.set_u8(g, Player::ACTIVE, 2);
        p.set_u8(g, Player::HITSTOP, 1);
        return run_at(g, p, 0x3b8c4);
    }
    next_step(g, p);
    let freeze = p.u8(g, 0x57);
    p.set_u8(g, Player::HITSTOP, freeze);
    p.set_u8(g, Player::TIMER_6F, 8);
    face_attacker(g, p);
    hurt_animation(g, p, 0);
}

/// Réactions qui aiguillent leurs pas par une table (`+7` -> décalage relatif à la table) : adresse de la
/// réaction, adresse de sa table. Les pas partagés entre réactions sont ainsi atteints par `HANDLERS`.
const STEP_TABLES: &[(u32, u32)] = &[
    (0x3a1f0, 0x3a1fc),
    (0x3a274, 0x3a280),
    (0x3a578, 0x3a584),
    (0x3a62a, 0x3a636),
    (0x3a7f6, 0x3a802),
    (0x3a86e, 0x3a87a),
    (0x3a890, 0x3a89c),
    (0x3a8b2, 0x3a8be),
    (0x3a9da, 0x3a9e6),
    (0x3a9f8, 0x3aa04),
    (0x3ab16, 0x3ab22),
    (0x3ab52, 0x3ab5e),
    (0x3ad96, 0x3ada2),
    (0x3ae8a, 0x3ae96),
    (0x3af44, 0x3af50),
    (0x3b0cc, 0x3b0d8),
    (0x3b160, 0x3b16c),
    (0x3b1e4, 0x3b1f0),
    (0x3b30e, 0x3b31a),
    (0x3b4c4, 0x3b4d0),
    (0x3b570, 0x3b57c),
    (0x3b66e, 0x3b67a),
    (0x3b72c, 0x3b738),
    (0x3b7ba, 0x3b7c6),
    (0x3b8a6, 0x3b8b2),
    (0x3b8c4, 0x3b8d0),
];

/// Pas vide (un simple RTS), partagé par plusieurs réactions.
const IDLE_STEP: u32 = 0x3a1a6;

/// Exécute la routine d'origine à `addr`, réécrite si possible.
pub fn run_at(g: &mut Ctx, p: Player, addr: u32) {
    cov!();
    if let Some(&(_, f)) = HANDLERS.iter().find(|h| h.0 == addr) {
        f(g, p);
    } else if let Some(&(_, table)) = STEP_TABLES.iter().find(|t| t.0 == addr) {
        jump(g, p, table);
    } else if addr != IDLE_STEP {
        g.rom(addr, p);
    }
}

/// Saut par table de décalages : l'entrée `index` de la table `base` donne l'adresse de la suite. Comme
/// l'original, D0 = `index` et D1 = décalage à l'entrée de la suite.
fn jump_table(g: &mut Ctx, p: Player, base: u32, index: u8) {
    cov!();
    let offset = g.prog16(base + index as u32);
    g.c.d[0] = index as u32;
    g.c.d[1] = g.c.d[1] & 0xffff_0000 | offset as u32;
    run_at(g, p, base.wrapping_add(offset as i16 as i32 as u32));
}

/// Réaction à pas : aiguille selon `+7`.
fn jump(g: &mut Ctx, p: Player, base: u32) {
    cov!();
    let step = p.u8(g, Player::STEP);
    jump_table(g, p, base, step);
}

// ---- outils communs -------------------------------------------------------------------------

/// 0x3bb16 : démarre l'animation de réaction `n` du personnage.
pub fn hurt_animation(g: &mut Ctx, p: Player, n: u16) {
    cov!();
    let character = p.u8(g, Player::CHARACTER) as u32;
    let set = g.data32(HURT_ANIMATIONS + 4 * character);
    let script = script_of(g.b, set, n);
    load_entry(g.b, p.obj(), script, true);
}

/// 0x3b93c : le joueur se retourne face à l'agresseur (`+0x56` = orientation de l'agresseur,
/// inversée) et sa vitesse horizontale s'inverse en conséquence.
fn face_attacker(g: &mut Ctx, p: Player) {
    cov!();
    let facing = p.u8(g, 0x56) ^ 0x20;
    p.set_u8(g, Player::FACING, facing);
    if facing != 0 {
        let vx = p.u32(g, Obj::VX).wrapping_neg();
        p.set_u32(g, Obj::VX, vx);
    }
}

/// Le joueur n'a plus de points de vie ?
fn is_dead(g: &mut Ctx, p: Player) -> bool {
    cov!();
    (p.u16(g, Player::HP) as i16) <= 0
}

/// 0x3b91c : s'il n'a plus de points de vie, le joueur passe à terre (état principal 4).
fn knock_down_if_dead(g: &mut Ctx, p: Player) {
    cov!();
    if is_dead(g, p) {
        knock_down(g, p);
    }
}

fn knock_down(g: &mut Ctx, p: Player) {
    cov!();
    p.set_u16(g, Player::MAIN, 0x0400);
    p.set_u8(g, Player::HITSTOP, 1);
    p.set_u8(g, Player::ACTIVE, 2);
    g.rom(rom::DEATH_EFFECT, p);
}

/// 0x3a1e2 : fin de la réaction, retour à l'état immobile.
fn recover(g: &mut Ctx, p: Player) {
    cov!();
    p.set_u8(g, Player::ACTIVE, 1);
    p.set_u8(g, REHIT, 0);
    to_stand(g, p);
}

/// Pose les vitesses d'une projection (vx, vz, accélérations), mémorise le gel du coup (`+0x57`).
fn set_launch(g: &mut Ctx, p: Player, vx: u16, vz: u16) {
    cov!();
    p.set_u16(g, Obj::VX, vx);
    p.set_u16(g, Obj::AX, 0);
    p.set_u16(g, Obj::VZ, vz);
    p.set_u16(g, Obj::AZ, (-0x40i16) as u16);
    let freeze = p.u8(g, 0x57);
    p.set_u8(g, Player::HITSTOP, freeze);
}

fn next_step(g: &mut Ctx, p: Player) {
    cov!();
    let step = p.u8(g, Player::STEP).wrapping_add(2);
    p.set_u8(g, Player::STEP, step);
}

// ---- réactions simples ------------------------------------------------------------------------

/// 0x3a17e : glissement de recul. Tant que `+0x6f` dure, le joueur tremble de 2 pixels ; ensuite
/// l'animation se déroule et la vitesse horizontale le fait glisser.
fn flinch(g: &mut Ctx, p: Player) {
    cov!();
    let left = p.u8(g, Player::TIMER_6F);
    if left != 0 {
        let mut shift = 2i16;
        if p.faces_left(g) {
            shift = -shift;
        }
        p.set_u8(g, Player::TIMER_6F, left - 1);
        if (left - 1) & 1 == 0 {
            shift = -shift;
        }
        let x = p.u16(g, 8).wrapping_add(shift as u16);
        p.set_u16(g, 8, x);
        return;
    }
    if animation::step(g, p) {
        return recover(g, p);
    }
    if p.u8(g, Obj::VX) != 0 {
        move_x(g, p);
    }
}

/// Début commun de la réaction 0 et 10 : recul avec l'animation `anim`.
fn start_flinch(g: &mut Ctx, p: Player, anim: u16) {
    cov!();
    next_step(g, p);
    p.set_u16(g, Obj::VX, 0x280);
    p.set_u16(g, Obj::AX, (-0x50i16) as u16);
    let freeze = p.u8(g, 0x57);
    p.set_u8(g, Player::HITSTOP, freeze);
    p.set_u8(g, Player::TIMER_6F, 8);
    face_attacker(g, p);
    hurt_animation(g, p, anim);
}

/// 0x3a136 : recul simple.
fn reaction_0(g: &mut Ctx, p: Player) {
    cov!();
    if p.u8(g, Player::STEP) != 0 {
        return flinch(g, p);
    }
    if p.u8(g, Player::SLOW) != 0 {
        p.set_u8(g, Player::ACTION, 0x14);
        return hurt_slow(g, p);
    }
    if is_dead(g, p) {
        // 0x3b8f6 : à terre tout de suite, et le gestionnaire « à terre » s'exécute dans la foulée
        knock_down(g, p);
        return downed(g, p);
    }
    start_flinch(g, p, 0);
}

/// 0x3a79e : recul avec animation 1.
fn reaction_10(g: &mut Ctx, p: Player) {
    cov!();
    if p.u8(g, Player::STEP) != 0 {
        return flinch(g, p);
    }
    if p.u8(g, Player::SLOW) != 0 {
        p.set_u8(g, Player::ACTION, 0x14);
        return hurt_slow(g, p);
    }
    if is_dead(g, p) {
        p.set_u16(g, Player::MAIN, 0x0400);
        p.set_u8(g, Player::ACTIVE, 2);
        return jump(g, p, steps::DOWN_A);
    }
    start_flinch(g, p, 1);
}

// ---- projections en l'air (pas partagés) --------------------------------------------------------

/// Pas 0 des projections : animation, puis vitesse (vx 0x200, vz 0x480).
fn launch_with_anim(g: &mut Ctx, p: Player, anim: u16) {
    cov!();
    hurt_animation(g, p, anim);
    next_step(g, p);
    set_launch(g, p, 0x200, 0x480);
    face_attacker(g, p);
    knock_down_if_dead(g, p);
}

/// 0x3a2ae : projection (vx 0x180, vz 0x400), animation 0xa.
fn launch_a(g: &mut Ctx, p: Player) {
    cov!();
    hurt_animation(g, p, 0xa);
    launch_common(g, p);
}

/// 0x3a2b4 : projection (vx 0x180, vz 0x400).
fn launch_common(g: &mut Ctx, p: Player) {
    cov!();
    next_step(g, p);
    set_launch(g, p, 0x180, 0x400);
    face_attacker(g, p);
    knock_down_if_dead(g, p);
}

/// 0x3aaac : projection (vx 0x200, vz 0x200), animation 0x16 ou 0x17 une image sur deux.
fn launch_b(g: &mut Ctx, p: Player) {
    cov!();
    next_step(g, p);
    set_launch(g, p, 0x200, 0x200);
    face_attacker(g, p);
    let anim = if g.a5u8(ram::FRAME_COUNTER) & 1 != 0 { 0x17 } else { 0x16 };
    hurt_animation(g, p, anim);
    knock_down_if_dead(g, p);
}

/// 0x3ad4c : projection comme `launch_common`, puis vitesse en profondeur (`VY` 0x80), animation 0x12.
fn launch_c(g: &mut Ctx, p: Player) {
    cov!();
    launch_common(g, p);
    p.set_u16(g, Obj::VY, 0x80);
    p.set_u16(g, Obj::AY, 0);
    p.set_u8(g, Player::HITSTOP, 1);
    hurt_animation(g, p, 0x12);
}

/// Contact avec un obstacle (`+0x29`) : la vitesse horizontale s'annule.
fn stop_at_wall(g: &mut Ctx, p: Player) {
    cov!();
    if p.u8(g, Player::BLOCKED) != 0 {
        p.set_u32(g, Obj::VX, 0);
    }
}

/// Altitude (mot haut de `Z`, signée).
fn altitude(g: &mut Ctx, p: Player) -> i16 {
    cov!();
    p.u16(g, Obj::Z) as i16
}

/// 0x3a2e6 : attend la fin de l'animation, puis enchaîne l'animation `anim`.
fn next_animation(g: &mut Ctx, p: Player, anim: u16) {
    cov!();
    if animation::step(g, p) {
        next_step(g, p);
        hurt_animation(g, p, anim);
    }
}

/// 0x3a300 : en l'air, jusqu'au sommet (vitesse verticale devenue négative).
fn fall_to_apex(g: &mut Ctx, p: Player, anim: u16) {
    cov!();
    animation::step(g, p);
    move_xz(g, p);
    stop_at_wall(g, p);
    if (p.u16(g, Obj::VZ) as i16) >= 0 {
        return;
    }
    next_step(g, p);
    hurt_animation(g, p, anim);
}

/// 0x3a330 : retombe jusqu'au sol.
fn land(g: &mut Ctx, p: Player, anim: u16) {
    cov!();
    animation::step(g, p);
    move_xz(g, p);
    stop_at_wall(g, p);
    if altitude(g, p) >= 0 {
        return;
    }
    p.set_u32(g, Obj::Z, 0);
    next_step(g, p);
    hurt_animation(g, p, anim);
    g.rom(rom::LANDING_EFFECT, p);
}

/// 0x3a522 : fin de la chute, petit rebond (vz 0x240).
fn bounce_up(g: &mut Ctx, p: Player) {
    cov!();
    if animation::step(g, p) {
        next_step(g, p);
        p.set_u16(g, Obj::VZ, 0x240);
        p.set_u16(g, Obj::AZ, (-0x40i16) as u16);
        hurt_animation(g, p, 8);
    }
}

/// 0x3a362 : rebond plus haut (vz 0x1c0).
fn bounce_up_small(g: &mut Ctx, p: Player) {
    cov!();
    if animation::step(g, p) {
        next_step(g, p);
        p.set_u16(g, Obj::VZ, 0x1c0);
        p.set_u16(g, Obj::AZ, (-0x40i16) as u16);
        hurt_animation(g, p, 8);
    }
}

/// 0x3a384 : retombe du rebond.
fn land_with_effect(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    move_xz(g, p);
    if altitude(g, p) >= 0 {
        return;
    }
    p.set_u32(g, Obj::Z, 0);
    next_step(g, p);
    hurt_animation(g, p, 9);
    g.rom(rom::LANDING_EFFECT, p);
}

/// 0x3a544 : début du glissement au sol (vx 0x300 dans le sens du recul).
fn slide_start(g: &mut Ctx, p: Player) {
    cov!();
    start_slide(g, p, 0x300);
}

/// 0x3a3ae : idem avec vx 0x280.
fn slide_start_short(g: &mut Ctx, p: Player) {
    cov!();
    start_slide(g, p, 0x280);
}

fn start_slide(g: &mut Ctx, p: Player, vx: u16) {
    cov!();
    if !animation::step(g, p) {
        return;
    }
    next_step(g, p);
    p.set_u8(g, Player::TIMER_6F, 0x14);
    p.set_u16(g, Obj::VX, vx);
    p.set_u16(g, Obj::AX, (-0x40i16) as u16);
    if p.faces_left(g) {
        // `neg.l` sur le couple (vitesse, accélération) : les deux mots sont négués ensemble
        let both = p.u32(g, Obj::VX).wrapping_neg();
        p.set_u32(g, Obj::VX, both);
    }
    hurt_animation(g, p, 0xf);
}

/// 0x3a3e2 : glissement ; poussière une image sur deux ; s'arrête quand la vitesse tombe à zéro.
fn slide(g: &mut Ctx, p: Player) {
    cov!();
    move_x(g, p);
    if p.u16(g, Obj::VX) != 0 && g.a5u8(ram::FRAME_COUNTER) & 1 != 0 {
        g.c.d[0] = 0;
        g.c.d[1] = 0;
        g.c.d[2] = 0;
        g.c.d[3] = g.c.d[3] & 0xffff_0000 | 0xffff;
        g.rom(rom::SLIDE_DUST, p);
    }
    stop_at_wall(g, p);
    if p.u8(g, Obj::VX) == 0 {
        next_step(g, p);
    }
}

/// 0x3b960 : chute scénarisée (cinématique 0x2447e) : le joueur est projeté sans perdre de vie (PV gardés en
/// `+0x64`, ramenés à 1), monte, retombe, rebondit, atterrit puis glisse ; à l'arrêt, action suivante (`+6`).
/// Les variantes impaires du personnage (`+2`) prennent les animations de la seconde série (+4).
pub fn fall(g: &mut Ctx, p: Player) {
    cov!();
    let second = p.u8(g, Player::INDEX) & 1 != 0;
    let anim = |n: u16| if second { n + 4 } else { n };
    match p.u8(g, Player::STEP) {
        0 => {
            hurt_animation(g, p, anim(6));
            let hp = p.u16(g, Player::HP);
            p.set_u16(g, Player::HP_PREV, hp);
            p.set_u16(g, Player::HP, 1);
            next_step(g, p);
            for (field, v) in [(Obj::VX, 0x180), (Obj::AX, 0), (Obj::VZ, 0x400), (Obj::AZ, 0xffc0), (Obj::VY, 0xffc0), (Obj::AY, 0)] {
                p.set_u16(g, field, v);
            }
            p.set_u8(g, 0x53, 0);
            if p.faces_left(g) {
                let both = p.u32(g, Obj::VX).wrapping_neg();
                p.set_u32(g, Obj::VX, both);
            }
            g.rom(rom::BEFORE_HURT_A, p);
        }
        2 => next_animation(g, p, anim(7)),
        4 => fall_to_apex_xyz(g, p, anim(8)),
        6 => fall_to_apex_xyz(g, p, anim(9)),
        8 => {
            if animation::step(g, p) {
                next_step(g, p);
                p.set_u16(g, Obj::VZ, 0x1c0);
                p.set_u16(g, Obj::AZ, 0xffc0);
                hurt_animation(g, p, 8);
            }
        }
        10 => {
            animation::step(g, p);
            move_xyz(g, p);
            if altitude(g, p) < 0 {
                p.set_u32(g, Obj::Z, 0);
                next_step(g, p);
                hurt_animation(g, p, 9);
                g.rom(rom::LANDING_EFFECT, p);
            }
        }
        12 => {
            if animation::step(g, p) {
                next_step(g, p);
                p.set_u16(g, Obj::VX, 0x280);
                p.set_u16(g, Obj::AX, 0xffc0);
                if p.faces_left(g) {
                    let both = p.u32(g, Obj::VX).wrapping_neg();
                    p.set_u32(g, Obj::VX, both);
                }
                hurt_animation(g, p, 15);
            }
        }
        _ => {
            move_x(g, p);
            if p.u16(g, Obj::VX) != 0 && g.a5u8(ram::FRAME_COUNTER) & 1 != 0 {
                g.c.d[0] = 0;
                g.c.d[1] = 0;
                g.c.d[2] = 0;
                g.c.d[3] = g.c.d[3] & 0xffff_0000 | 0xffff;
                g.rom(rom::SLIDE_DUST, p);
            }
            stop_xy_at_wall(g, p);
            if p.u8(g, Obj::VX) == 0 {
                let action = p.u8(g, Player::ACTION).wrapping_add(2);
                p.set_u8(g, Player::ACTION, action);
                p.set_u8(g, Player::STEP, 0);
            }
        }
    }
}

/// 0x3b9ec : en l'air (déplacement sur les trois axes), jusqu'au sommet.
fn fall_to_apex_xyz(g: &mut Ctx, p: Player, anim: u16) {
    cov!();
    animation::step(g, p);
    move_xyz(g, p);
    stop_xy_at_wall(g, p);
    if (p.u16(g, Obj::VZ) as i16) < 0 {
        next_step(g, p);
        hurt_animation(g, p, anim);
    }
}

/// Contre un mur (`+0x29`), plus de vitesse en x ni en profondeur.
fn stop_xy_at_wall(g: &mut Ctx, p: Player) {
    if p.u8(g, Player::BLOCKED) != 0 {
        p.set_u32(g, Obj::VX, 0);
        p.set_u32(g, Obj::VY, 0);
    }
}

/// 0x3a424 : attend `+0x6f` images à terre puis se relève (animation 0x10 et son).
fn get_up(g: &mut Ctx, p: Player) {
    cov!();
    if p.u8(g, Player::MAIN) == 4 {
        // mort : le joueur reste à terre un moment, puis le clignotement de résurrection commence
        let step = p.u8(g, Player::STEP).wrapping_add(4);
        p.set_u8(g, Player::STEP, step);
        p.set_u8(g, Player::TIMER_6F, 0x3c);
        return invulnerable_blink(g, p);
    }
    let left = p.u8(g, Player::TIMER_6F).wrapping_sub(1);
    p.set_u8(g, Player::TIMER_6F, left);
    if left != 0 {
        return;
    }
    next_step(g, p);
    hurt_animation(g, p, 0x10);
    let sound = if g.a5u8(ram::FRAME_COUNTER) & 2 != 0 { rom::GET_UP_SOUND_B } else { rom::GET_UP_SOUND_A };
    g.rom(sound, p);
}

/// 0x3a466 : fin de l'animation de relevé : retour à l'état immobile.
fn finish_get_up(g: &mut Ctx, p: Player) {
    cov!();
    if animation::step(g, p) {
        recover(g, p);
    }
}

/// 0x3a472 : clignotement d'invulnérabilité pendant `+0x6f` images (le joueur est masqué une
/// image sur quatre dans les 40 dernières) ; à la fin, les effets de statut sont effacés.
fn invulnerable_blink(g: &mut Ctx, p: Player) {
    cov!();
    let flags = p.u8(g, Player::FLAGS) & !2;
    p.set_u8(g, Player::FLAGS, flags);
    let left = p.u8(g, Player::TIMER_6F).wrapping_sub(1);
    p.set_u8(g, Player::TIMER_6F, left);
    if left == 0 {
        let sub = p.u8(g, Player::SUB).wrapping_add(2);
        p.set_u8(g, Player::SUB, sub);
        p.set_u16(g, Player::ACTION, 0);
        for field in [Player::AURA, Player::AURA_B, Player::AURA_C, Player::POWERUP] {
            p.set_u8(g, field, 0);
        }
        p.set_u16(g, Player::AURA_TIMER, 0);
        p.set_u8(g, Player::SLOW, 0);
        return;
    }
    if (left as i8) > 0x28 || left & 3 == 0 {
        return;
    }
    let flags = p.u8(g, Player::FLAGS) | 2;
    p.set_u8(g, Player::FLAGS, flags);
}

/// 0x3ad66 : projection avec glissement en profondeur.
fn fall_c(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    move_xyz(g, p);
    stop_at_wall(g, p);
    if altitude(g, p) > 0 {
        return;
    }
    p.set_u32(g, Obj::Z, 0);
    next_step(g, p);
    hurt_animation(g, p, 0xd);
}

/// 0x3aae8 : projection (réaction 26) : retombe.
fn fall_b(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    move_xz(g, p);
    stop_at_wall(g, p);
    if altitude(g, p) > 0 {
        return;
    }
    p.set_u32(g, Obj::Z, 0);
    next_step(g, p);
    hurt_animation(g, p, 9);
}

// ---- réaction 0x34 (0x3b4c4) : chute violente ------------------------------------------------------

/// 0x3b504 : à l'atterrissage, l'écran tremble (0x2824, D0 = 10) et le joueur perd `+0x6f` PV (hors
/// cinématique) ; à zéro il meurt.
fn crash_landing(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    move_xz(g, p);
    stop_at_wall(g, p);
    if altitude(g, p) > 0 {
        return;
    }
    g.c.d[0] = g.c.d[0] & 0xffff_0000 | 10;
    g.rom(0x2824, p);
    p.set_u32(g, Obj::Z, 0);
    if g.a5u8(ram::FROZEN) == 0 {
        let hp = p.u16(g, Player::HP).wrapping_sub(p.u8(g, Player::TIMER_6F) as u16);
        p.set_u16(g, Player::HP, hp);
        if (hp as i16) <= 0 {
            p.set_u8(g, Player::ACTIVE, 2);
            p.set_u16(g, Player::HP, 0);
            p.set_u16(g, Player::MAIN, 0x0400);
            g.rom(rom::DEATH_EFFECT, p);
            let alive = g.a5u8(ram::PLAYERS_ALIVE).wrapping_sub(1);
            g.set_a5u8(ram::PLAYERS_ALIVE, alive);
        }
    }
    next_step(g, p);
    hurt_animation(g, p, 9);
    g.rom(rom::LANDING_EFFECT, p);
}

// ---- réaction 0x3e (0x3a62a) : projeté au loin -------------------------------------------------------

/// Le joueur, projeté, a quitté la zone de l'écran (plus de 320 pixels après le bord gauche + 32) ou
/// heurté un obstacle.
fn hurled_out(g: &mut Ctx, p: Player) -> bool {
    cov!();
    if p.u8(g, Player::BLOCKED) != 0 {
        return true;
    }
    let camera = A5 + if g.a5u8(ram::USE_ALT_CAMERA) != 0 { 0x54e } else { 0x4fe };
    let gap = p.u16(g, Obj::X).wrapping_sub(crate::mem::rd16(g.b, camera + 8)).wrapping_sub(32);
    gap > 320
}

/// 0x3a656, 0x3ab7c : projection au ras du sol (vx 0x800, vitesse verticale `vz`, 18 images), animation `anim`.
fn hurl(g: &mut Ctx, p: Player, vz: u16, anim: u16) {
    cov!();
    next_step(g, p);
    p.set_u16(g, Obj::VX, 0x800);
    p.set_u16(g, Obj::AX, 0);
    p.set_u16(g, Obj::VZ, vz);
    p.set_u16(g, Obj::AZ, (-4i16) as u16);
    let freeze = p.u8(g, 0x57);
    p.set_u8(g, Player::HITSTOP, freeze);
    face_attacker(g, p);
    p.set_u8(g, Player::TIMER_6F, 18);
    hurt_animation(g, p, anim);
    knock_down_if_dead(g, p);
}

/// 0x3a68a, 0x3abb0 : vol bas (18 images), puis freinage (animation `brake`) ; un obstacle ou le bord de
/// l'écran arrêtent net le vol (chute au pas `out`, animation 25).
fn hurled(g: &mut Ctx, p: Player, brake: u16, out: u8) {
    cov!();
    animation::step(g, p);
    move_xz(g, p);
    let left = p.u8(g, Player::TIMER_6F).wrapping_sub(1);
    p.set_u8(g, Player::TIMER_6F, left);
    if left == 0 {
        next_step(g, p);
        p.set_u16(g, Obj::AZ, (-64i16) as u16);
        let ax: u16 = if p.faces_left(g) { 32 } else { (-32i16) as u16 };
        p.set_u16(g, Obj::AX, ax);
        return hurt_animation(g, p, brake);
    }
    if !hurled_out(g, p) {
        return;
    }
    p.set_u32(g, Obj::VX, 0);
    p.set_u16(g, Obj::VZ, 0);
    p.set_u16(g, Obj::AZ, (-64i16) as u16);
    p.set_u8(g, Player::STEP, out);
    hurt_animation(g, p, 25);
}

/// 0x3a706, 0x3ac2c : retombe ; au sol, animation `anim` (et poussière si `effect`) ; un obstacle ou le bord
/// arrêtent le glissement.
fn hurled_landing(g: &mut Ctx, p: Player, anim: u16, effect: bool) {
    cov!();
    animation::step(g, p);
    move_xz(g, p);
    if altitude(g, p) <= 0 {
        p.set_u32(g, Obj::Z, 0);
        next_step(g, p);
        hurt_animation(g, p, anim);
        if effect {
            g.rom(rom::LANDING_EFFECT, p);
        }
        return;
    }
    if hurled_out(g, p) {
        p.set_u32(g, Obj::VX, 0);
    }
}

/// 0x3a75a, 0x3acba : glissade au sol (poussière une image sur deux) ; à l'arrêt, pas `stop`.
fn hurled_slide(g: &mut Ctx, p: Player, stop: u8) {
    cov!();
    move_x(g, p);
    if p.u16(g, Obj::VX) != 0 && g.a5u8(ram::FRAME_COUNTER) & 1 != 0 {
        g.c.d[0] = 0;
        g.c.d[1] = 0;
        g.c.d[2] = 0;
        g.c.d[3] = g.c.d[3] & 0xffff_0000 | 0xffff;
        g.rom(rom::SLIDE_DUST, p);
    }
    stop_at_wall(g, p);
    if p.u8(g, Obj::VX) == 0 {
        p.set_u8(g, Player::STEP, stop);
    }
}

// ---- réaction 0x22 (0x3ad96) : pris (h) — projeté, puis il se débat --------------------------------

/// Changement des directions maintenues depuis l'image précédente (le joueur « se débat »).
fn struggling(g: &mut Ctx, p: Player) -> bool {
    p.u8(g, Player::KEYS_HELD) & 15 != p.u8(g, Player::KEYS_PREV) & 15
}

/// Durée de la prise, décomptée par les secousses.
const HOLD: u32 = 0x1f0;
/// Son de la prise et de la libération (D0 = 1).
const HOLD_SOUND: u32 = 0xab7d8;

/// 0x3adbe : petite projection, palette 1/19, 30 images ; s'il n'a plus de PV, à terre.
fn shock_start(g: &mut Ctx, p: Player) {
    cov!();
    if is_dead(g, p) {
        knock_down(g, p);
        return downed(g, p);
    }
    next_step(g, p);
    p.set_u8(g, HOLD, 8);
    p.set_u16(g, Obj::VX, 0x200);
    p.set_u16(g, Obj::AX, 0);
    p.set_u16(g, Obj::VZ, 0x400);
    p.set_u16(g, Obj::AZ, (-128i16) as u16);
    face_attacker(g, p);
    p.set_u8(g, Player::PALETTE_GROUP, 1);
    p.set_u8(g, Player::PALETTE, 19);
    p.set_u8(g, Player::TIMER_6F, 30);
    p.set_u8(g, Player::HITSTOP, 1);
    hurt_animation(g, p, 27);
    g.rom(0xebbda, p);
}

/// 0x3ae0e : retombe (sans animation).
fn shock_fall(g: &mut Ctx, p: Player) {
    cov!();
    move_xz(g, p);
    stop_at_wall(g, p);
    if altitude(g, p) > 0 {
        return;
    }
    p.set_u32(g, Obj::Z, 0);
    next_step(g, p);
}

/// 0x3ae32 : chaque secousse avance l'animation et use la prise ; prise usée : libéré. Au bout de
/// `+0x6f` images, la suite (`shock_release`).
fn shock_struggle(g: &mut Ctx, p: Player) {
    cov!();
    if struggling(g, p) {
        animation::step(g, p);
        let left = p.u8(g, HOLD).wrapping_sub(1);
        p.set_u8(g, HOLD, left);
        if left == 0 {
            g.c.d[0] = 1;
            g.rom(HOLD_SOUND, p);
            p.set_u16(g, Player::PALETTE_GROUP, 0);
            return recover(g, p);
        }
    }
    let left = p.u8(g, Player::TIMER_6F).wrapping_sub(1);
    p.set_u8(g, Player::TIMER_6F, left);
    if left != 0 {
        return;
    }
    next_step(g, p);
    g.c.d[0] = 1;
    g.rom(HOLD_SOUND, p);
}

/// 0x3ae7a : palette rétablie, projection.
fn shock_release(g: &mut Ctx, p: Player) {
    cov!();
    p.set_u16(g, Player::PALETTE_GROUP, 0);
    launch_a(g, p);
    p.set_u8(g, Player::HITSTOP, 1);
}

// ---- réaction 0x24 (0x3ae8a) : étourdi -----------------------------------------------------------

/// Étoiles de l'étourdissement (D0, D1 = réglages).
const STARS: u32 = 0xb378a;

/// 0x3aeaa : étoiles, 40 images d'étourdissement.
fn dizzy_start(g: &mut Ctx, p: Player) {
    cov!();
    g.c.d[0] = 0;
    g.c.d[1] = 0;
    g.rom(STARS, p);
    next_step(g, p);
    p.set_u8(g, Player::TIMER_6F, 40);
    p.set_u8(g, Player::HITSTOP, 1);
    hurt_animation(g, p, 0);
}

/// 0x3aeca : le joueur vacille d'un pixel à gauche et à droite (parfois de nouvelles étoiles), puis tombe
/// en arrière (animation 22 ou 23 une image sur deux).
fn dizzy(g: &mut Ctx, p: Player) {
    cov!();
    let left = p.u8(g, Player::TIMER_6F).wrapping_sub(1);
    p.set_u8(g, Player::TIMER_6F, left);
    if left != 0 {
        if g.random() & 127 == 0 {
            g.c.d[0] = 0;
            g.c.d[1] = 1;
            g.rom(STARS, p);
        }
        let mut dx: u16 = 1;
        if left & 1 != 0 {
            dx = dx.wrapping_neg();
        }
        if p.faces_left(g) {
            dx = dx.wrapping_neg();
        }
        g.c.d[0] = dx as u32;
        let x = p.u16(g, Obj::X).wrapping_add(dx);
        p.set_u16(g, Obj::X, x);
        return;
    }
    next_step(g, p);
    p.set_u16(g, Obj::VX, 0x180);
    p.set_u16(g, Obj::AX, 0);
    p.set_u32(g, Obj::VY, 0);
    p.set_u16(g, Obj::VZ, 0x400);
    p.set_u16(g, Obj::AZ, (-64i16) as u16);
    if p.faces_left(g) {
        // 0x3b952 : `neg.l` du couple (vitesse, accélération)
        let pair = p.u32(g, Obj::VX).wrapping_neg();
        p.set_u32(g, Obj::VX, pair);
    }
    let anim = if g.a5u8(ram::FRAME_COUNTER) & 1 != 0 { 23 } else { 22 };
    hurt_animation(g, p, anim);
    p.set_u8(g, Player::HITSTOP, 0);
    knock_down_if_dead(g, p);
}

// ---- réaction 0x28 (0x3b1e4) : assis par terre, il se débat pour se relever ------------------------------

/// 0x3b1fa.
fn sit_start(g: &mut Ctx, p: Player) {
    cov!();
    if is_dead(g, p) {
        p.set_u16(g, Player::MAIN, 0x0400);
        p.set_u8(g, Player::ACTIVE, 2);
        p.set_u8(g, Player::HITSTOP, 1);
        return jump(g, p, steps::DOWN_6);
    }
    p.set_u8(g, Player::TIMER_6F, 120);
    next_step(g, p);
    if p.u16(g, Obj::Z) != 0 {
        p.set_u32(g, Obj::VX, 0);
        p.set_u16(g, Obj::VZ, 0);
        p.set_u16(g, Obj::AZ, (-64i16) as u16);
        return hurt_animation(g, p, 3);
    }
    sit(g, p);
}

/// 0x3b23c : assis (animation 35), ou en marche lente directement relevé (0x3b24e).
fn sit(g: &mut Ctx, p: Player) {
    cov!();
    next_step(g, p);
    if p.u8(g, Player::SLOW) == 0 {
        return hurt_animation(g, p, 35);
    }
    sit_down(g, p);
}

/// 0x3b24e : la lutte commence (animation 34), plus d'invulnérabilité.
fn sit_down(g: &mut Ctx, p: Player) {
    cov!();
    p.set_u8(g, Player::ACTIVE, 1);
    p.set_u8(g, Player::INVULN, 0);
    next_step(g, p);
    hurt_animation(g, p, 34);
}

/// 0x3b264 : retombe, puis s'assied.
fn sit_fall(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    move_xz(g, p);
    if altitude(g, p) > 0 {
        return;
    }
    p.set_u16(g, Obj::Z, 0);
    sit(g, p);
}

/// 0x3b28a : chaque secousse use deux images de `+0x6f` (une sinon) ; au bout, il se relève (animation 32,
/// 48 images d'invulnérabilité).
fn sit_struggle(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    if !struggle_out(g, p) {
        return;
    }
    next_step(g, p);
    p.set_u8(g, Player::INVULN, 48);
    hurt_animation(g, p, 32);
}

/// `+0x6f` baisse d'une image, de deux si le joueur se débat ; vrai quand il est épuisé.
fn struggle_out(g: &mut Ctx, p: Player) -> bool {
    cov!();
    let mut left = p.u8(g, Player::TIMER_6F);
    if struggling(g, p) {
        left = left.wrapping_sub(1);
    }
    left = left.wrapping_sub(1);
    p.set_u8(g, Player::TIMER_6F, left);
    (left as i8) <= 0
}

// ---- réaction 0x32 (0x3b44c) : tenu par un ennemi ----------------------------------------------------

/// Ennemi qui tient le joueur (pointeur de 16 bits) et drapeau « tenu ».
const HOLDER: u32 = 0x186;
const HELD: u32 = 0x188;
/// État de l'ennemi qui tient sa prise.
const HOLDING: u16 = 0x020c;

/// 0x3b44c : le joueur, tenu, se débat : 20 nouvelles directions le libèrent ; il est aussi libéré si
/// l'ennemi lâche prise (son état n'est plus 0x020c).
fn held(g: &mut Ctx, p: Player) {
    cov!();
    if p.u8(g, Player::STEP) == 0 {
        next_step(g, p);
        p.set_u8(g, Player::TIMER_6F, 20);
        return hurt_animation(g, p, 31);
    }
    animation::step(g, p);
    let pressed = p.u8(g, Player::KEYS_HELD) & 15 & !(p.u8(g, Player::KEYS_PREV) & 15);
    if pressed != 0 {
        let left = p.u8(g, Player::TIMER_6F).wrapping_sub(1);
        p.set_u8(g, Player::TIMER_6F, left);
        if left == 0 {
            return break_free(g, p);
        }
    }
    let holder = p.u16(g, HOLDER) as i16 as i32 as u32;
    if crate::mem::rd16(g.b, holder + 4) != HOLDING {
        break_free(g, p);
    }
}

/// Libéré : réaction 0x34 (relâché).
fn break_free(g: &mut Ctx, p: Player) {
    cov!();
    p.set_u8(g, HELD, 0);
    p.set_u8(g, Player::ACTIVE, 1);
    p.set_state(g, 0x0204_3400);
    p.set_u8(g, Player::TIMER_6F, 0);
}

// ---- gestionnaires par sous-état ---------------------------------------------------------------

/// 0x3a0a8 : joueur touché (`+5` = 4).
pub fn hurt(g: &mut Ctx, p: Player) {
    cov!();
    if p.u8(g, Player::STEP) == 0 {
        g.rom(rom::BEFORE_HURT_A, p);
        g.rom(rom::BEFORE_HURT_B, p);
        if p.u8(g, Player::SLOW_8A) != 0 {
            p.set_u8(g, Player::SLOW_8A, 0);
            let held = p.u32(g, Player::LINK);
            wr8(g.b, held + 5, 8);
            wr16(g.b, held + 6, 0);
        }
    }
    let action = p.u8(g, Player::ACTION);
    jump_table(g, p, HURT_ACTIONS, action);
    if g.a5u8(ram::INPUT_RAW) == 0 {
        g.rom(rom::TERRAIN, p);
    }
    clamp_to_screen(g.b, p);
}

/// 0x3b7f4 : joueur à terre (`+4` = 4).
pub fn downed(g: &mut Ctx, p: Player) {
    cov!();
    p.set_u8(g, 0x192, 0);
    p.set_u16(g, Player::DYING_TIMER, 0);
    let action = p.u8(g, Player::ACTION);
    jump_table(g, p, DOWN_ACTIONS, action);
    g.rom(rom::TERRAIN, p);
    if p.u8(g, Player::ACTIVE) != 0 {
        ACTORS.reinsert(g.b, p.obj());
    }
    clamp_to_screen(g.b, p);
}
