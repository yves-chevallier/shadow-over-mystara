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
use crate::mem::{wr16, wr8};
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
    /// Réaction avec la marche lente forcée (non exercée).
    pub const HURT_SLOW: u32 = 0x3a98e;
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
];

/// Exécute la routine d'origine à `addr`, réécrite si possible.
pub fn run_at(g: &mut Ctx, p: Player, addr: u32) {
    cov!();
    match HANDLERS.iter().find(|h| h.0 == addr) {
        Some(&(_, f)) => f(g, p),
        None => {
            g.c.d[0] = 0; // les routines d'origine complètent l'octet bas de D0 : le reste doit être nul
            g.rom(addr, p);
        }
    }
}

/// Saut par table de décalages : l'entrée `index` de la table `base` donne l'adresse de la suite.
fn jump_table(g: &mut Ctx, p: Player, base: u32, index: u8) {
    cov!();
    let offset = g.prog16(base + index as u32) as i16 as i32 as u32;
    run_at(g, p, base.wrapping_add(offset));
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
    p.set_u8(g, 0x185, 0);
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
        g.rom(rom::HURT_SLOW, p);
        return;
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
        g.rom(rom::HURT_SLOW, p);
        return;
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
