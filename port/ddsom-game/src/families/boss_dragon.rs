//! Boss de type 13 (0x8f59c) : le dragon noir en vol des scènes 7 et 9 (h). Voir `docs/familles/boss.md`.
//!
//! Il ne combat pas : il suit la caméra à gauche de l'écran (`scroll - 0x80`), flotte en haut et en bas
//! (`bob`), puis, à des abscisses de défilement fixées (`STOPS`), s'arrête pour une cinématique (un
//! objet « message » obtenu par `0xc8312`), vient vers le meneur de la caméra, le suit, s'en éloigne et
//! sort. Il crée à sa naissance deux tirs ESHL de type 0x14 qui gardent un lien vers lui (`+0x50`).
//!
//! Sous-états `+6` de l'état actif (`+5` = 0 ; `+5` = 2 libère l'objet) :
//!
//! | `+6` | Rôle |
//! |---|---|
//! | 0 | prépare l'étape |
//! | 2 | attend l'abscisse suivante de `STOPS`, puis lance la cinématique |
//! | 4 | attend la fin de la cinématique |
//! | 6 | vient vers le meneur |
//! | 8, 12 | suit le meneur un temps (`+0xa2`) |
//! | 10 | idem, puis tient la position tant que `+0xa5` bit 0 |
//! | 14 | s'éloigne et sort de l'écran |

use super::boss_common::*;
use super::cx::handlers;
use crate::mem::wr8;

/// Jeu d'animations du dragon.
const ANIMS: u32 = 0x17d7d2;
/// Seuils d'abscisse (caméra + 0x100) qui déclenchent chaque étape (mots, espace des opcodes).
const STOPS: u32 = 0x8f97a;
/// Vitesses de flottement : 4 mots (VZ, AZ, VY, AY) à partir de `4 × tirage` (fenêtres qui se
/// chevauchent, comme l'original), espace de données.
const BOB_TABLE: u32 = 0x8faa2;

mod rom {
    pub const BEGIN_A: u32 = 0xdf4bc;
    pub const NEW_MESSAGE: u32 = 0xc8312;
}

/// Champs propres à ce type.
mod g {
    /// Bit 0 : cinématique demandée ; bit 1 : étape franchie.
    pub const CUTSCENE: u32 = 0x42;
    pub const TALKING: u32 = 0x43;
    /// Phase du flottement (0 : vitesses à tirer, 1 : vitesses à inverser) et minuterie.
    pub const BOB_PHASE: u32 = 0xa0;
    pub const BOB_TIMER: u32 = 0xa1;
    /// Minuterie du sous-état.
    pub const TIMER: u32 = 0xa2;
    pub const HOLD: u32 = 0xa5;
    /// Rang de la prochaine étape de `STOPS`.
    pub const STOP_INDEX: u32 = 0xa7;
    /// Vitesses de flottement tirées (VZ, AZ, VY, AY).
    pub const BOB_SPEEDS: u32 = 0xa8;
    /// Objet « message » de la cinématique (mot).
    pub const MESSAGE: u32 = 0xb0;
    pub const LEAVING: u32 = 0xb2;
    pub const SLOW: u32 = 0xbc;
}
use g::*;
use crate::objects::pools;
use crate::objects::Courant;

/// Octet `0xff0ec6` (`A5-0x713a`) : non nul, le dragon se retire.
const RETREAT_FLAG: u32 = 0xff0ec6;
const SCENE_PHASE: u32 = 0x51e;
const SHOT_SLOTS: u32 = 0x7e64;
const PLAYERS: u32 = 0xff85de;

/// États longs (`+4..+7`) : actif, `+6` = 0 ; actif, `+6` = 0xe.
const ACTIVE_FIRST: u32 = 0x0200_0000;
const ACTIVE_LEAVE: u32 = 0x0200_0e00;

pub fn run(x: &mut Cx) {
    if var::FROZEN.get(x.b) != 0 {
        return;
    }
    match x.u8(f::STATE) {
        st::INIT => init(x),
        st::ACTIVE => active(x),
        _ => x.free(&pools::ENEMY),
    }
}

fn init(x: &mut Cx) {
    if (x.a5_u16(SHOT_SLOTS) as i16) < 2 {
        return;
    }
    let Some(lead) = leader(x) else { return };
    x.add8(f::STATE, 2);
    var::FROZEN.set(x.b, 0);
    x.set32(f::IMMUNITY, 0xffff_ffff);
    x.set8(f::FLIP, 0x20);
    x.set8(0x6f, 2);
    x.set8(f::BOX_SET, 0x29);
    let y = lead.u16(x.b, 0xc).wrapping_sub(0x10);
    x.set16(0xc, y);
    let left = scroll_x(x).wrapping_sub(0x80);
    x.set16(0x8, left);
    x.set8(BOB_PHASE, 0);
    x.set8(BOB_TIMER, 0x10);
    x.set16(TIMER, 0x258);
    x.set16(0x18, 0x300);
    x.set16(0x1a, (-0x28i16) as u16);
    for variant in [1, 0] {
        let shot = alloc(x, &pools::ESHL);
        let active = x.u8(0);
        shot.set_u8(x.b, 0, active);
        shot.set_u8(x.b, 3, variant);
        shot.set_u8(x.b, 2, 0x14);
        shot.set_u16(x.b, f::OWNER, x.o.0 as u16);
    }
    x.anim_start(ANIMS, 0);
}

fn active(x: &mut Cx) {
    if suspended(x) {
        return;
    }
    if x.u8(f::SUB) == 0 {
        stage(x);
    } else {
        x.free(&pools::ENEMY);
    }
    x.reinsert();
}

fn stage(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => begin(x),
        2 => wait_stop(x),
        4 => wait_cutscene(x),
        6 => approach(x),
        8 | 12 => follow(x),
        10 => follow_and_hold(x),
        14 => leave(x),
        _ => {}
    }
}

/// Vol libre : accroché à la gauche de l'écran, avec le flottement.
fn drift(x: &mut Cx) {
    let left = scroll_x(x).wrapping_sub(0x80);
    x.set16(0x8, left);
    bob(x);
    x.move_yz();
    x.anim_step();
}

/// Déplacement du dragon qui suit le meneur.
fn glide(x: &mut Cx) {
    chase(x);
    x.move_yz();
    x.anim_step();
}

/// La caméra a-t-elle dépassé le seuil suivant de `STOPS` ? Si oui, passe au suivant.
fn next_stop(x: &mut Cx) -> bool {
    let reach = scroll_x(x).wrapping_add(0x100) as i16;
    let i = x.u8(STOP_INDEX) as u32;
    let stop = x.rom16(STOPS + 2 * i) as i16;
    if stop >= reach {
        return false;
    }
    x.add8(STOP_INDEX, 1);
    true
}

/// Fin demandée (`RETREAT_FLAG`) ou seuil franchi : le dragon passe en mode « départ » (`+6` = 0xe).
fn must_leave(x: &mut Cx) -> bool {
    x.data8(RETREAT_FLAG) != 0 || next_stop(x)
}

fn leave_now(x: &mut Cx) {
    x.set32(f::STATE, ACTIVE_LEAVE);
    x.set8(SLOW, 1);
}

fn begin(x: &mut Cx) {
    x.add8(f::SUB2, 2);
    x.set_bit(CUTSCENE, 0, false);
    x.set_bit(CUTSCENE, 1, false);
    x.set8(LEAVING, 0);
    x.call(rom::BEGIN_A);
    drift(x);
}

/// Fige ou libère le jeu et les joueurs pendant la cinématique.
fn freeze_players(x: &mut Cx, on: bool) {
    for k in 0..4 {
        let player = PLAYERS + 0x200 * k;
        if x.data8(player) != 0 {
            wr8(x.b, player + 0x16a, on as u8);
        }
    }
}

fn wait_stop(x: &mut Cx) {
    if next_stop(x) {
        x.set32(f::STATE, ACTIVE_FIRST);
        x.set_bit(CUTSCENE, 1, true);
        return;
    }
    if x.bit(CUTSCENE, 0) && x.a5_u16(SCENE_PHASE) != 4 && var::FROZEN.get(x.b) == 0 {
        var::PAUSE.set(x.b, 1);
        x.set_a5_u8(0x3c, 1);
        x.set_a5_u8(0x33, 1);
        freeze_players(x, true);
        x.c.d[0] = 0xe;
        x.call(rom::NEW_MESSAGE);
        if !alloc_failed(x) {
            x.set16(MESSAGE, x.c.a[1] as u16);
            x.add8(f::SUB2, 2);
            lib::sound(x, 0xe0d8c);
            x.set8(TALKING, 1);
            x.set_bit(CUTSCENE, 0, false);
        }
    }
    drift(x);
}

fn wait_cutscene(x: &mut Cx) {
    if x.u16(MESSAGE) != 0 && x.linked(MESSAGE).u8(x.b, 0x22) != 0 {
        var::PAUSE.set(x.b, 0);
        x.set_a5_u8(0x33, 0);
        freeze_players(x, false);
        x.add8(f::SUB2, 2);
        x.set16(TIMER, 0x12c);
        x.set16(0x14, 0x200);
        if let Some(lead) = leader(x) {
            let y = lead.u16(x.b, 0x10).wrapping_add(0x10);
            x.set16(0x10, y);
            return;
        }
    }
    drift(x);
}

fn approach(x: &mut Cx) {
    if must_leave(x) {
        return leave_now(x);
    }
    if let Some(lead) = leader(x) {
        let dx = lead.u16(x.b, 0x58);
        x.add16(0x8, dx);
        let reach = scroll_x(x).wrapping_add(0x50);
        set_dw(x, 0, reach);
        if x.in_arena(x.c.d[0] as u16) {
            x.add8(f::SUB2, 2);
            x.set16(TIMER, 0xb4);
        }
    }
    bob(x);
    x.move_xyz();
    x.anim_step();
}

fn follow(x: &mut Cx) {
    if must_leave(x) {
        return leave_now(x);
    }
    let left = x.u16(TIMER).wrapping_sub(1);
    x.set16(TIMER, left);
    if (left as i16) >= 0 {
        glide(x);
    } else {
        x.add8(f::SUB2, 2);
    }
}

fn follow_and_hold(x: &mut Cx) {
    if must_leave(x) {
        x.set8(LEAVING, 1);
        return leave_now(x);
    }
    match x.u8(f::SUB2 + 1) {
        0 => {
            x.add8(f::SUB2 + 1, 2);
            x.set_bit(HOLD, 0, true);
            glide(x);
        }
        _ if x.bit(HOLD, 0) => glide(x),
        _ => {
            x.add8(f::SUB2, 2);
            x.set8(f::SUB2 + 1, 0);
            x.set16(TIMER, 0x12c);
        }
    }
}

fn leave(x: &mut Cx) {
    next_stop(x);
    let Some(lead) = leader(x) else { return };
    let left = scroll_x(x).wrapping_sub(0x80);
    if x.u16(0x8).wrapping_sub(left) >= 0x200 {
        lib::sound(x, 0xe0c5c);
        x.set8(TALKING, 0);
        x.set_bit(HOLD, 0, false);
        x.set8(SLOW, 0);
        x.set32(f::STATE, ACTIVE_FIRST);
        return;
    }
    let vx = if x.data8(RETREAT_FLAG) != 0 {
        -0x200
    } else if x.u8(SLOW) != 0 {
        -8
    } else {
        (lead.u16(x.b, 0x58) as i16).wrapping_sub(var::PLAYERS_ALIVE.get(x.b) as i16)
    };
    x.set16(0x14, vx as u16);
    let y = lead.u16(x.b, 0x10).wrapping_sub(0x10);
    x.set16(0x10, y);
    bob(x);
    x.move_x();
    x.move_yz();
    x.anim_step();
}

/// Se place près du meneur : même profondeur, abscisse maintenue dans `[meneur - 0xe0, meneur - 0x60]`.
fn chase(x: &mut Cx) {
    let Some(lead) = leader(x) else { return };
    let y = lead.u16(x.b, 0x10).wrapping_sub(0x10);
    x.set16(0x10, y);
    let mut dx = lead.u16(x.b, 0x58);
    let lx = lead.u16(x.b, 0x8);
    let mine = x.u16(0x8);
    if (lx.wrapping_sub(0x80).wrapping_sub(mine) as i16) >= 0 {
        dx = dx.wrapping_add(1);
    }
    let next = mine.wrapping_add(dx) as i16;
    let (hi, lo) = (lx.wrapping_sub(0x60), lx.wrapping_sub(0xe0));
    let pos = if next >= hi as i16 {
        hi
    } else if next > lo as i16 {
        next as u16
    } else {
        lo
    };
    x.set16(0x8, pos);
    bob(x);
}

/// Flottement : toutes les 16 images, de nouvelles vitesses tirées dans `BOB_TABLE`, puis leurs opposées.
fn bob(x: &mut Cx) {
    let inverse = x.u8(BOB_PHASE) != 0;
    let t = x.u8(BOB_TIMER).wrapping_sub(1);
    x.set8(BOB_TIMER, t);
    if (t as i8) >= 0 {
        return;
    }
    if inverse {
        for k in 0..4 {
            let v = x.u16(BOB_SPEEDS + 2 * k);
            x.set16(0x18 + 2 * k, v.wrapping_neg());
        }
    } else {
        let pick = (random(x) & 7) * 4;
        for k in 0..4 {
            let v = x.data16(BOB_TABLE + pick + 2 * k);
            x.set16(BOB_SPEEDS + 2 * k, v);
            x.set16(0x18 + 2 * k, v);
        }
    }
    x.set8(BOB_PHASE, !inverse as u8);
    x.set8(BOB_TIMER, 0x10);
}

handlers! {
    0x8f59c "boss_13" run,
}
