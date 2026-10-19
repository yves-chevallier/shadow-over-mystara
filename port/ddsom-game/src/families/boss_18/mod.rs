//! Boss de type 18 (0x975ec), scène 26 : le Lich (h). Voir `docs/familles/boss.md`.
//!
//! De la famille des boss 17 et 21 (charpente des ennemis `+4` état, `+5` groupe, `+6` et `+7` étapes, états posés d'un coup
//! en longs, tirages à deux niveaux dans les tables de la ROM) ; sa particularité est l'état touché : la plupart des
//! codes de coup `+0x49` ont une réaction propre (neuf routines : recul, chute, saut, tourbillon...).
//!
//! | `+5` | Fichier | Rôle |
//! |---|---|---|
//! | 0 | `intro` | cinématique d'arrivée (délais et sons, puis combat) |
//! | 2 | `fight` | combat : attente, demi-tour, marche (`+6` = 4, 6), charge (8), reprise (10) |
//! | 4 | `attacks` | onze attaques (`+6` = 0 à 20) |
//! | 6, 8, 10 | ici | attente tant qu'aucun joueur n'est en vie |
//! | état 4 | `hit` | réactions aux coups (table de neuf routines), mort |
//!
//! Animations : 0x181ae0 (mouvements), 0x181af2 (coups), 0x181b10 (réactions).

use super::boss_common::*;
use super::cx::handlers;

mod attacks;
mod decide;
mod fight;
mod hit;
mod intro;

pub(self) mod g {
    pub const ANGLE: u32 = 0x2d;
    pub const KEY: u32 = 0x33;
    pub const REACTION: u32 = 0x49;
    pub const STUN: u32 = 0x56;
    pub const VULNERABLE: u32 = 0x67;
    pub const WALK_STEPS: u32 = 0x6b;
    pub const APPROACH: u32 = 0x6e;
    pub const WALK_DELAY: u32 = 0x71;
    pub const SHOT: u32 = 0x79;
    pub const SHAKE: u32 = 0x90;
    pub const TIMER: u32 = 0xa6;
    pub const COUNT: u32 = 0xa8;
    pub const ANIM_KIND: u32 = 0xa0;
    pub const ANIM_LAST: u32 = 0xa1;
    pub const SAVED_STATE: u32 = 0xa2;
    pub const PHASE: u32 = 0xaa;
    pub const PHASES: u32 = 0xab;
    pub const INVULNERABLE: u32 = 0xad;
    /// Nombre d'attaques 4 permises (`+0xae`), objets lâchés encore en jeu (`+0xaf`), objet lié (`+0xb0`).
    pub const BUDGET_4: u32 = 0xae;
    pub const SPAWNED: u32 = 0xaf;
    pub const CHILD: u32 = 0xb0;
    /// Attaques restantes de chaque sorte.
    pub const BUDGET_A: u32 = 0xb2;
    pub const BUDGET_B: u32 = 0xb3;
    pub const BUDGET_C: u32 = 0xb4;
    pub const BUDGET_D: u32 = 0xb5;
    pub const BUDGET_E: u32 = 0xb6;
    pub const BUDGET_F: u32 = 0xb7;
    /// Minuterie de la poursuite (mot `+0xb8`) et pas entre deux visées (`+0xba`, `+0xbb`).
    pub const CHASE_TIME: u32 = 0xb8;
    pub const AIM_TICK: u32 = 0xba;
    pub const AIM_ON: u32 = 0xbb;
}
use g::*;

pub(self) const SET_BODY: u32 = 0x181ae0;
pub(self) const SET_STRIKE: u32 = 0x181af2;
pub(self) const SET_HURT: u32 = 0x181b10;

pub(self) fn body(x: &mut Cx, n: u32) {
    play(x, SET_BODY, n);
}

pub(self) fn strike(x: &mut Cx, n: u32) {
    play(x, SET_STRIKE, n);
}

pub(self) mod call {
    pub const MOVE_X: u32 = 0x1d78;
    pub const MOVE_XY: u32 = 0x1d62;
    pub const MOVE_XYZ: u32 = 0x1d90;
    pub const MOVE_XZ: u32 = 0x1da6;
    pub const MOVE_TABLE_XY: u32 = 0x1cb4;
    pub const FREE_BOSS: u32 = 0x2234;
    pub const ASK_TARGET: u32 = 0xe76c;
    pub const RANDOM_TURN: u32 = 0x10574;
    pub const TURN_NEEDED: u32 = 0x1050c;
    pub const RAND_TIMER: u32 = 0x79f6;
    pub const TARGET_BOX_FAR: u32 = 0x7ef4;
    pub const PICK_FAR: u32 = 0x43ae;
    pub const SOUND_EVENT: u32 = 0x2824;
    pub const BURST: u32 = 0xabec8;
}

pub fn run(x: &mut Cx) {
    if suspended(x) {
        return;
    }
    x.call(rom::TICK_COUNTERS);
    match x.u8(f::STATE) {
        st::INIT => init(x),
        st::ACTIVE => active(x),
        st::HIT => hit::run(x),
        _ => x.call(call::FREE_BOSS),
    }
    if x.a5_u8(0x30) == 0 {
        x.set8(0x3a, 0);
    }
    if x.u8(INVULNERABLE) != 0 {
        x.set8(0x38, 0);
    }
    if x.u8(0) != 0 {
        x.reinsert();
    }
}

// --- état 0 : création --------------------------------------------------------------------------

fn init(x: &mut Cx) {
    x.add8(f::STATE, 2);
    x.set_a5_u8(0x50, 1);
    x.set_a5_u32(0x38, 0x0459_5900);
    x.call(call::PICK_FAR);
    x.set8(PHASES, 2);
    x.set8(f::BOX_SET, 0x32);
    x.call(0x453c);
    x.set_a5_u8(0x5a, 0);
    x.call(0x1059e);
    x.call(0x19e10);
    x.call(0x19db4);
    x.call(0x19eb0);
    x.set8(0x93, 1);
    x.set8(0x9a, 0xff);
    for off in [0x76, 0x70, 0x6a, 0x75, 0x8e] {
        x.set8(off, 0);
    }
    for (off, v) in [(0x72, 1), (0x73, 1), (0x6c, 6), (0x6d, 6), (0x74, 7), (0x78, 0xff), (0x68, 8), (0x69, 1), (VULNERABLE, 0)] {
        x.set8(off, v);
    }
    x.set8(ANIM_KIND, 0);
    x.set8(ANIM_LAST, 0xff);
    for (off, v) in [(BUDGET_A, 6), (BUDGET_B, 0xc), (BUDGET_C, 3), (BUDGET_D, 6), (BUDGET_E, 5), (BUDGET_F, 4)] {
        x.set8(off, v);
    }
    for (off, v) in [(0x7e, 0x26), (0x7f, 0), (0x80, 0x22), (0x81, 0x1c)] {
        x.set8(off, v);
    }
    x.set32(f::IMMUNITY, 0xffff_ffff);
    if x.a5_u8(0x30) != 0 {
        x.set8(0, 2);
        x.set_bit(0x1, 1, true);
        let px = x.a5_u16(var::CAMERA1_X).wrapping_add(0xc0);
        let py = x.a5_u16(var::CAMERA1_Y).wrapping_add(0x69);
        x.set16(0x8, px);
        x.set16(0x10, py);
        x.set16(TIMER, 0x3c);
        if let Some(e) = alloc_retry(x, rom::ALLOC_EFFCT) {
            e.set_u8(x.b, 0, 1);
            e.set_u8(x.b, 2, 0x26);
            e.set_u16(x.b, 0x42, x.o.0 as u16);
        }
    } else {
        x.set_bit(0x1, 1, false);
        decide::wait(x);
    }
    body(x, 2);
}

// --- état 2 : actif -----------------------------------------------------------------------------

fn active(x: &mut Cx) {
    if tst8(x, f::FREEZE).gt() {
        sub8(x, f::FREEZE, 1);
    } else if x.a5_u8(var::PAUSE) == 0 {
        match x.u8(f::SUB) {
            0 => intro::run(x),
            2 => fight::run(x),
            4 => attacks::run(x),
            _ => wait_for_players(x),
        }
    }
    x.call(IN_ZONE);
    x.call(SHADOW);
}

/// `+5` = 6, 8, 10 (0x98640) : attend que des joueurs reviennent.
fn wait_for_players(x: &mut Cx) {
    if x.a5_u8(0x30) != 0 && (x.a5_u8(var::PLAYERS_ALIVE) as i8) > 0 {
        return decide::chase(x);
    }
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(ANIM_LAST, 0xff);
            body(x, 8);
        }
        2 => {
            step(x);
        }
        _ => {}
    }
}

handlers! {
    0x975ec "boss_18" 1200 run,
}
