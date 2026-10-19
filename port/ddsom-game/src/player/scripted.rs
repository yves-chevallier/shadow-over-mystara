//! État scénarisé des joueurs (`+4` = 0) : entrées en scène, résurrection, cinématiques (P3).
//!
//! Dans cet état le joueur obéit à un petit script plutôt qu'aux manettes. L'octet `+5` choisit le
//! genre de script ; `+6` est alors le *pas* du script (et `+7` un sous-pas) :
//!
//! | `+5` | genre | aiguillage |
//! |---|---|---|
//! | 0 | cinématique de la scène | numéro de scène (`A5+0x34`) -> routine (table 0x23826) |
//! | 4 | résurrection | pas `+6` -> routine (décalages, table 0x23b98) |
//! | 8 | attente d'ouverture des commandes | pas `+6` -> routine (table 0x23d66) |
//! | 0xc | cinématique de fin | numéro de scène -> routine (table 0x23f4e), sans réinsertion |
//!
//! Comme `hurt`, les routines d'origine sont atteintes par les tables du code ; `HANDLERS`
//! donne la version Rust des routines réécrites, les autres s'exécutent telles quelles.

use super::actions::start_stand;
use super::checks::{special_attack, throw_button};
use super::effects::restore_palette;
use super::states::State;
use super::walk::{move_x, move_z};
use super::{animation, ram, Ctx, Player};
use crate::objects::lists::ACTORS;
use crate::objects::Obj;

type Handler = fn(&mut Ctx, Player);

/// Routines d'origine encore appelées telles quelles.
mod rom {
    /// Mise en place du joueur à l'écran après une résurrection.
    pub const PLACE: u32 = 0x3668;
    pub const REVIVE_SETUP: u32 = 0x4c930;
    pub const REVIVE_SOUND: u32 = 0xe24d8;
    pub const FADE: u32 = 0x1686;
    pub const RANK_RAISE: u32 = 0x1f2c0;
    pub const TERRAIN: u32 = 0x33fe4;
    pub const INTRO_SOUND: u32 = 0xab708;
    pub const INTRO_ACTION: u32 = 0xe1330;
    pub const SPECIAL_VOICE: u32 = 0x4fd2c;
}

/// Tables du code.
mod tables {
    /// Cinématiques par scène : une adresse de routine (long) par numéro de scène.
    pub const BY_SCENE: u32 = 0x23826;
    /// Résurrection : décalages (mots) relatifs à la table, indexés par le pas `+6`.
    pub const REVIVE_STEPS: u32 = 0x23b98;
    /// Attente des commandes : adresses de routine, indexées par `+6`.
    pub const WAIT_STEPS: u32 = 0x23d66;
    /// Cinématiques de fin par scène.
    pub const END_BY_SCENE: u32 = 0x23f4e;
    /// Pas de la cinématique de la scène 1.
    pub const SCENE1_STEPS: u32 = 0x24eae;
    /// Marche d'entrée : pas.
    pub const WALK_IN_STEPS: u32 = 0x23a00;
    /// Pas de l'attente des commandes (`+7`).
    pub const WAIT_SUBSTEPS: u32 = 0x23d7a;
    /// Durées (mots) de l'entrée en scène selon le joueur.
    pub const WALK_IN_WAIT: u32 = 0x23a56;
    pub const SCENE1_WAIT: u32 = 0x24f38;
    pub const WAIT_DELAYS: u32 = 0x23dac;
}

/// Réécrit : adresse d'origine -> routine Rust.
const HANDLERS: &[(u32, Handler)] = &[
    (0x24ea0, |g, p| step_table(g, p, tables::SCENE1_STEPS)),
    (0x24ebc, scene1_start),
    (0x24eea, scene1_walk),
    (0x24f12, scene1_wait_progress),
    (0x24f40, scene1_wait),
    (0x24f60, scene1_pose),
    (0x24f98, scene1_dash_start),
    (0x24fc8, scene1_dash),
    (0x239f2, |g, p| step_table(g, p, tables::WALK_IN_STEPS)),
    (0x23a0a, walk_in_start),
    (0x23a2e, walk_in_walk),
    (0x23a5e, walk_in_wait),
    (0x23a7c, walk_in_pose),
    (0x23ab2, walk_in_end),
    (0x24102, idle),
    (0x24118, |_, _| {}),
    (0x23ba6, revive_start),
    (0x23c16, revive_fall),
    (0x23c3c, revive_land),
    (0x23c58, revive_end),
    (0x23d6e, |g, p| substep_table(g, p, tables::WAIT_SUBSTEPS)),
    (0x23d82, wait_start),
    (0x23db4, wait_delay),
    (0x23dde, wait_pose),
    (0x23df2, wait_inputs),
];

fn run_at(g: &mut Ctx, p: Player, addr: u32) {
    cov!();
    match HANDLERS.iter().find(|h| h.0 == addr) {
        Some(&(_, f)) => f(g, p),
        None => {
            g.c.d[0] = 0; // les routines d'origine complètent l'octet bas de D0 : le reste doit être nul
            g.rom(addr, p);
        }
    }
}

/// Saut par table de décalages indexée par le pas `+6`.
fn step_table(g: &mut Ctx, p: Player, base: u32) {
    cov!();
    let step = p.u8(g, Player::ACTION) as u32;
    let offset = g.prog16(base + step) as i16 as i32 as u32;
    run_at(g, p, base.wrapping_add(offset));
}

/// Idem avec le sous-pas `+7`.
fn substep_table(g: &mut Ctx, p: Player, base: u32) {
    cov!();
    let step = p.u8(g, Player::STEP) as u32;
    let offset = g.prog16(base + step) as i16 as i32 as u32;
    run_at(g, p, base.wrapping_add(offset));
}

/// 0x352da : aiguille selon `+5`.
pub fn update(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::SUB) {
        0 => {
            let scene = g.a5u8(ram::SCENE) as u32;
            let routine = g.prog32(tables::BY_SCENE + 4 * scene);
            run_at(g, p, routine);
            ACTORS.reinsert(g.b, p.obj());
        }
        4 => {
            step_table(g, p, tables::REVIVE_STEPS);
            ACTORS.reinsert(g.b, p.obj());
        }
        8 => {
            let step = p.u8(g, Player::ACTION) as u32;
            let routine = g.prog32(tables::WAIT_STEPS + step);
            run_at(g, p, routine);
            if !throw_button(g, p) {
                special_attack(g, p);
            }
            ACTORS.reinsert(g.b, p.obj());
        }
        0xc => {
            let scene = g.a5u8(ram::SCENE) as u32;
            let routine = g.prog32(tables::END_BY_SCENE + 4 * scene);
            run_at(g, p, routine);
        }
        _ => {}
    }
}

// ---- outils -----------------------------------------------------------------------------------

fn next_action(g: &mut Ctx, p: Player) {
    cov!();
    let step = p.u8(g, Player::ACTION).wrapping_add(2);
    p.set_u8(g, Player::ACTION, step);
}

fn next_step(g: &mut Ctx, p: Player) {
    cov!();
    let step = p.u8(g, Player::STEP).wrapping_add(2);
    p.set_u8(g, Player::STEP, step);
}

/// Compte à rebours en `+0x1e0` (mot) ; vrai quand il atteint zéro.
fn countdown(g: &mut Ctx, p: Player) -> bool {
    cov!();
    let left = p.u16(g, 0x1e0).wrapping_sub(1);
    p.set_u16(g, 0x1e0, left);
    left == 0
}

/// Le joueur reprend la main : état de jeu normal.
fn give_control(g: &mut Ctx, p: Player) {
    cov!();
    p.set_state(g, State::pack(2, 0, 0, 0).0);
}

/// Mot de la table indexée par le numéro du joueur.
fn per_player(g: &mut Ctx, p: Player, table: u32) -> u16 {
    cov!();
    let k = p.u8(g, Player::INDEX) as u32;
    g.prog16(table + 2 * k)
}

// ---- cinématique de la scène 1 : entrée en marchant, pose, course finale ------------------------

/// 0x24ebc : départ de la marche d'entrée (vitesse 2 pixels par image).
fn scene1_start(g: &mut Ctx, p: Player) {
    cov!();
    next_action(g, p);
    p.set_u8(g, Player::ACTIVE, 1);
    p.set_u8(g, 0x15c, 0);
    p.set_u8(g, 0x68, 0);
    p.set_u16(g, 0x1e0, 0x120);
    p.set_u32(g, Obj::VX, 0x0200_0000);
    p.set_u32(g, Obj::VZ, 0);
    p.set_u32(g, Obj::VY, 0);
    animation::start(g, p, 2);
}

/// 0x24eea : marche d'entrée ; à la fin du compte à rebours, immobile.
fn scene1_walk(g: &mut Ctx, p: Player) {
    cov!();
    if countdown(g, p) {
        next_action(g, p);
        animation::start(g, p, 0);
        return;
    }
    if p.u16(g, 0x1e0) > 0x50 {
        return;
    }
    animation::step(g, p);
    move_x(g, p);
}

/// 0x24f12 : attend que la scène (`A5+0x2e`) arrive à l'étape 6.
fn scene1_wait_progress(g: &mut Ctx, p: Player) {
    cov!();
    if g.a5u8(0x2e) == 6 {
        next_action(g, p);
        let k = p.u8(g, 0x7d) as u32;
        let delay = g.prog16(tables::SCENE1_WAIT + 2 * k);
        p.set_u16(g, 0x1e0, delay);
        g.rom(rom::INTRO_SOUND, p);
    }
    animation::step(g, p);
}

/// 0x24f40 : attend, puis prend la pose (animation 0x33, arme en main).
fn scene1_wait(g: &mut Ctx, p: Player) {
    cov!();
    if countdown(g, p) {
        next_action(g, p);
        p.set_u8(g, 0x68, 1);
        animation::start(g, p, 0x33);
        return;
    }
    animation::step(g, p);
}

/// 0x24f60 : pose, puis cri du héros au moment du geste (selon la classe), retour à l'immobile.
fn scene1_pose(g: &mut Ctx, p: Player) {
    cov!();
    if animation::step(g, p) {
        next_action(g, p);
        animation::start(g, p, 0);
        return;
    }
    if p.u8(g, 0x33) == 0 {
        return;
    }
    p.set_u8(g, 0x33, 0);
    let class = p.class(g);
    if class == 0 || class == 2 {
        g.c.d[0] = 0;
        g.rom(rom::SPECIAL_VOICE, p);
    }
}

/// 0x24f98 : attend l'étape 8, puis la course finale (animation 7, vitesses par table 0x21).
fn scene1_dash_start(g: &mut Ctx, p: Player) {
    cov!();
    if g.a5u8(0x2e) != 8 {
        animation::step(g, p);
        return;
    }
    next_action(g, p);
    p.set_u16(g, 0x1e0, 0x70);
    p.set_u8(g, Obj::SPEED_INDEX, 0xa);
    let table = g.prog32(0x1b84 + 4 * 0x21);
    crate::objects::motion::select_speed_table(g.b, p.obj(), table);
    animation::start(g, p, 7);
}

/// 0x24fc8 : course finale ; à la fin, le joueur reprend la main.
fn scene1_dash(g: &mut Ctx, p: Player) {
    cov!();
    crate::objects::motion::advance_by_table(g.b, p.obj(), crate::objects::motion::AXIS_X, 0);
    crate::objects::motion::advance_by_table(g.b, p.obj(), crate::objects::motion::AXIS_Y, 2);
    if countdown(g, p) {
        give_control(g, p);
    }
    animation::step(g, p);
}

// ---- marche d'entrée des scènes 2, 15, 16... ----------------------------------------------------

/// 0x23a0a.
fn walk_in_start(g: &mut Ctx, p: Player) {
    cov!();
    next_action(g, p);
    p.set_u16(g, Player::ACTIVE, 0x0101);
    p.set_u8(g, 0x15c, 0);
    p.set_u8(g, 0x68, 0);
    p.set_u16(g, 0x1e0, 0x50);
    animation::start(g, p, 2);
}

/// 0x23a2e : le joueur avance de 2 pixels par image.
fn walk_in_walk(g: &mut Ctx, p: Player) {
    cov!();
    let x = p.u16(g, 8).wrapping_add(2);
    p.set_u16(g, 8, x);
    if countdown(g, p) {
        next_action(g, p);
        let wait = per_player(g, p, tables::WALK_IN_WAIT);
        p.set_u16(g, 0x1e0, wait);
        start_stand(g, p);
        return;
    }
    animation::step(g, p);
}

/// 0x23a5e : attend, puis prend la pose.
fn walk_in_wait(g: &mut Ctx, p: Player) {
    cov!();
    if countdown(g, p) {
        next_action(g, p);
        p.set_u8(g, 0x68, 1);
        animation::start(g, p, 0x33);
        return;
    }
    animation::step(g, p);
}

/// 0x23a7c : pose ; cri du héros ; retour à l'immobile.
fn walk_in_pose(g: &mut Ctx, p: Player) {
    cov!();
    if animation::step(g, p) {
        next_action(g, p);
        start_stand(g, p);
        return;
    }
    if p.u8(g, 0x33) == 0 {
        return;
    }
    p.set_u8(g, 0x33, 0);
    let class = p.class(g);
    if class == 0 || class == 2 {
        g.c.d[0] = 0;
        g.rom(rom::SPECIAL_VOICE, p);
    }
}

/// 0x23ab2 : en phase de jeu, le joueur reprend la main.
fn walk_in_end(g: &mut Ctx, p: Player) {
    cov!();
    if g.a5u8(ram::PHASE) == 0xc {
        give_control(g, p);
    }
    animation::step(g, p);
}

/// 0x24102 : cinématique vide : une pose immobile.
fn idle(g: &mut Ctx, p: Player) {
    cov!();
    if p.u8(g, Player::ACTION) == 0 {
        next_action(g, p);
        start_stand(g, p);
    } else {
        animation::step(g, p);
    }
}

// ---- résurrection -------------------------------------------------------------------------------

/// 0x23ba6 : le joueur tombe du ciel à sa place.
fn revive_start(g: &mut Ctx, p: Player) {
    cov!();
    next_action(g, p);
    p.set_u8(g, Player::ACTIVE, 2);
    p.set_u8(g, 0x68, 1);
    p.set_u8(g, Player::FACING, 0x20);
    p.set_u16(g, Obj::VZ, (-0x300i16) as u16);
    p.set_u16(g, Obj::AZ, (-0x80i16) as u16);
    let flags = p.u8(g, Player::FLAGS) & !2;
    p.set_u8(g, Player::FLAGS, flags);
    for field in [Player::SLOW, 0x15c, Player::POWERUP, Player::AURA, Player::AURA_B, Player::AURA_C] {
        p.set_u8(g, field, 0);
    }
    g.rom(rom::PLACE, p);
    g.rom(rom::REVIVE_SETUP, p);
    animation::start(g, p, 0xd);
    restore_palette(g, p);
    g.set_a5u8(0x48e, 0xf);
    g.rom(rom::FADE, p);
    g.rom(rom::RANK_RAISE, p);
    g.rom(rom::TERRAIN, p);
}

/// 0x23c16 : chute jusqu'au sol.
fn revive_fall(g: &mut Ctx, p: Player) {
    cov!();
    move_z(g, p);
    if (p.u16(g, Obj::Z) as i16) >= 0 {
        animation::step(g, p);
        return;
    }
    next_action(g, p);
    p.set_u32(g, Obj::Z, 0);
    p.set_u32(g, Obj::VZ, 0);
    animation::start(g, p, 0xe);
}

/// 0x23c3c : atterrissage, puis l'animation 0x23 et le son de résurrection.
fn revive_land(g: &mut Ctx, p: Player) {
    cov!();
    if !animation::step(g, p) {
        return;
    }
    next_action(g, p);
    animation::start(g, p, 0x23);
    g.rom(rom::REVIVE_SOUND, p);
}

/// 0x23c58 : fin de l'animation : le joueur reprend la main, invulnérable 150 images.
fn revive_end(g: &mut Ctx, p: Player) {
    cov!();
    if !animation::step(g, p) {
        return;
    }
    give_control(g, p);
    p.set_u8(g, Player::ACTIVE, 1);
    p.set_u8(g, Player::INVULN, 0x96);
    start_stand(g, p);
}

// ---- attente d'ouverture des commandes (début de scène) ------------------------------------------

/// 0x23d82.
fn wait_start(g: &mut Ctx, p: Player) {
    cov!();
    next_step(g, p);
    let flags = p.u8(g, Player::FLAGS) & !2;
    p.set_u8(g, Player::FLAGS, flags);
    let k = p.u8(g, 0x7d) as u32;
    let delay = g.prog16(tables::WAIT_DELAYS + 2 * k);
    p.set_u16(g, 0x1e0, delay);
    g.rom(rom::INTRO_SOUND, p);
    animation::step(g, p);
    g.rom(rom::INTRO_ACTION, p);
}

/// 0x23db4 : attend, puis se tourne vers la droite si nécessaire.
fn wait_delay(g: &mut Ctx, p: Player) {
    cov!();
    if !countdown(g, p) {
        animation::step(g, p);
        return;
    }
    next_step(g, p);
    if p.u8(g, Player::FACING) == 0 {
        next_step(g, p);
        animation::step(g, p);
        return;
    }
    p.flip(g);
    animation::start(g, p, 5);
}

/// 0x23dde : pose de demi-tour.
fn wait_pose(g: &mut Ctx, p: Player) {
    cov!();
    if animation::step(g, p) {
        next_step(g, p);
        start_stand(g, p);
    }
}

/// 0x23df2 : dès que les commandes sont ouvertes, le joueur reprend la main.
fn wait_inputs(g: &mut Ctx, p: Player) {
    cov!();
    if g.a5u8(ram::INPUT_MASK) != 0 {
        give_control(g, p);
    }
}
