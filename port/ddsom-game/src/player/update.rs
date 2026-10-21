//! Boucle des quatre joueurs et aiguillage des états (P4).
//!
//! `update_players` (0x35292) est appelée une fois par image par la boucle de partie (0x1c86e). Pour
//! chaque joueur actif, l'octet `+4` aiguille vers l'état principal :
//!
//! | `+4` | état |
//! |---|---|
//! | 0 | scénarisé (cinématiques par scène, P3) ; `+5` choisit le sous-état |
//! | 2 | jeu normal ; `+5` : 0 action, 4 touché, 8 paré |
//! | 4 | à terre / expulsé (`+5` doit valoir 0) |
//! | 6 | retiré : sauvegarde du joueur, effacement de l'enregistrement et de ses sous-objets |

use cps2::Cps2Bus;
use m68k::Cpu;

use super::init::{clear_overlays_of, clear_player, UNLOCKED_FLAGS};
use super::states::sub;
use super::{actions, checks, combos, effects, hurt, ram, scripted, Ctx, Player};
use crate::mem::{rts, wr32, wr8};
use crate::objects::lists;
use crate::Native;

/// Routines d'origine encore appelées telles quelles.
mod rom {
    pub const SCENE_29_HOOK: u32 = 0xb9c6a;
    pub const HIGH_SCORE: u32 = 0x180c8;
    /// Table (adresses de 4 octets, dans le code) des gestionnaires de sous-état `+5`.
    pub const SUB_HANDLERS: u32 = 0x3541c;
}

/// Une image de tous les joueurs actifs.
pub fn update_all(g: &mut Ctx) {
    cov!();
    for p in Player::all() {
        if p.is_active(g) {
            update_one(g, p);
        }
    }
}

/// 0x352c4 : aiguillage selon l'état principal.
pub fn update_one(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::MAIN) {
        0 => scripted::update(g, p),
        2 => update_playing(g, p),
        4 => {
            if p.u8(g, Player::SUB) == 0 {
                hurt::downed(g, p);
            }
        }
        6 => remove(g, p),
        _ => {}
    }
}

/// 0x352f4 : un joueur en jeu normal. Décompte des compteurs, effets de statut, puis le gestionnaire
/// du sous-état, puis les contrôles de fin d'image.
fn update_playing(g: &mut Ctx, p: Player) {
    cov!();
    loop {
        combos::detect(g, p);
        checks::near_death(g, p);
        let freeze = p.u8(g, Player::FREEZE_194);
        if freeze != 0 {
            p.set_u8(g, Player::FREEZE_194, freeze.wrapping_sub(1) & 0x3f);
        }
        let hitstop = p.u8(g, Player::HITSTOP);
        if hitstop != 0 && (hitstop as i8) >= 0 {
            // gel de coup : le gestionnaire ne tourne pas
            p.set_u8(g, Player::HITSTOP, hitstop - 1);
            return end_of_frame(g, p);
        }
        if p.u8(g, Player::SUB) == 0 {
            let invuln = p.u8(g, Player::INVULN);
            if invuln != 0 {
                p.set_u8(g, Player::INVULN, invuln - 1);
            }
        }
        let timer = p.u8(g, Player::RECOIL_TIMER);
        if timer != 0 {
            p.set_u8(g, Player::RECOIL_TIMER, timer - 1);
        }
        if timer <= 1 {
            p.set_u8(g, Player::RECOIL, 0);
        }
        if p.u8(g, 0x16a) != 0 {
            return end_of_frame(g, p);
        }
        if g.a5u8(ram::PAUSE) == 0 && p.u8(g, Player::POWER_10F) == 0 {
            status_effects(g, p);
        }
        match p.u8(g, Player::SUB) {
            sub::ACTING => actions::act(g, p),
            sub::HURT => hurt::hurt(g, p),
            sub::PARRY => super::moves::parry(g, p),
            other => {
                let handler = g.prog32(rom::SUB_HANDLERS + other as u32);
                g.c.d[0] = other as u32;
                g.rom(handler, p);
            }
        }
        if p.u16(g, Player::HP) == 0 {
            return finish(g, p);
        }
        // l'aura de vitesse (+0x183) fait jouer l'image deux fois, sauf pendant une attaque 0xe
        if p.u8(g, Player::AURA_B) == 0
            || p.state(g) & !0xff == 0x0200_0e00
            || g.a5u8(ram::SCENE) == 0x43
        {
            return end_of_frame(g, p);
        }
        let parity = p.u8(g, 0x1e3).wrapping_add(1) & 1;
        p.set_u8(g, 0x1e3, parity);
        if parity != 0 {
            return end_of_frame(g, p);
        }
    }
}

/// 0x35344..0x353a4 : bonus temporaires (puissance, aura), clignotement et couleurs d'aura.
fn status_effects(g: &mut Ctx, p: Player) {
    cov!();
    if p.u8(g, Player::POWERUP) != 0 {
        let left = p.u16(g, Player::POWERUP_TIMER).wrapping_sub(1);
        p.set_u16(g, Player::POWERUP_TIMER, left);
        if (left as i16) < 0 {
            p.set_u16(g, Player::POWERUP_TIMER, 0);
            p.set_u8(g, Player::POWERUP, 0);
        }
    }
    let auras = p.u8(g, Player::AURA).wrapping_add(p.u8(g, Player::AURA_B)).wrapping_add(p.u8(g, Player::AURA_C));
    if auras != 0 {
        let left = p.u16(g, Player::AURA_TIMER).wrapping_sub(1);
        p.set_u16(g, Player::AURA_TIMER, left);
        if (left as i16) < 0 {
            p.set_u8(g, Player::AURA, 0);
            p.set_u8(g, Player::AURA_B, 0);
            p.set_u8(g, Player::AURA_C, 0);
            let flags = p.u8(g, Player::FLAGS) & !2;
            p.set_u8(g, Player::FLAGS, flags);
            effects::restore_palette(g, p);
        }
    }
    effects::blink(g, p);
    effects::cycle_aura_b(g, p);
    effects::cycle_aura_c(g, p);
    effects::dying_countdown(g, p);
}

/// 0x353ea : lancer d'arme (bouton 3) ou attaque spéciale (bouton 4).
fn end_of_frame(g: &mut Ctx, p: Player) {
    cov!();
    if !checks::throw_button(g, p) {
        checks::special_attack(g, p);
    }
    finish(g, p);
}

/// 0x353f4 : fin d'image.
fn finish(g: &mut Ctx, p: Player) {
    cov!();
    if g.a5u8(ram::SCENE) == 0x29
        && p.u16(g, 0xc) == 0
        && (g.a5u8(ram::FRAME_COUNTER).wrapping_add(p.u8(g, Player::INDEX)) & 0xf) == 0
    {
        g.rom(rom::SCENE_29_HOOK, p);
    }
    lists::ACTORS.reinsert(g.b, p.obj());
}

/// 0x35436 : retire le joueur de la partie : sauvegarde de ses données (score, objets, personnage),
/// effacement de son enregistrement et de ses deux sous-objets.
fn remove(g: &mut Ctx, p: Player) {
    cov!();
    lists::unlink(g.b, p.obj());
    let k = p.u8(g, Player::INDEX) as u32 & 3;
    g.rom(rom::HIGH_SCORE, p);
    let saved = UNLOCKED_FLAGS + 0x70 + 0x10 * k;
    let (a, b, c) = (p.u32(g, 0x118), p.u32(g, 0x11c), p.u32(g, Player::SCORE_BCD));
    wr32(g.b, saved, a);
    wr32(g.b, saved + 4, b);
    wr32(g.b, saved + 8, c);
    for (i, field) in [Player::CHARACTER, Player::LEVEL, 0x193].into_iter().enumerate() {
        let v = p.u8(g, field);
        wr8(g.b, saved + 0xc + i as u32, v);
    }
    clear_player(g, k);
    clear_overlays_of(g, k);
}

fn update_players(c: &mut Cpu, b: &mut Cps2Bus) {
    let mut g = Ctx::new(c, b);
    update_all(&mut g);
    rts(c, b);
}

pub const UPDATE_PLAYERS: Native = Native {
    entry: 0x35292,
    name: "players_update",
    cost: |_, _| 3_000,
    run: update_players,
    touches_sound: false,
    outputs: 0,
    exact_cost: false,
};
