//! Ennemi « Harpy » (ENEMY type 21, 0x756c4) : créature volante qui traverse l'écran. Voir
//! `docs/familles/enemy.md`.
//!
//! Dix variantes (`+3`) : 0 et 1 volent d'un bord à l'autre, 2 à 5 rasent le décor, 6 et 7 font
//! demi-tour, 8 passe au premier plan, 9 est le modèle sans mouvement propre. Chaque créature crée à sa
//! naissance un compagnon (ombre, ESHL type 0x10) dont `+0xaa` garde le lien. Sous-états en état actif :
//! 0 vol, 2 attaque (`+0x2f` choisit la série : 0 projectiles, 1 coups, 2 tir), 4 pause, 6 sortie
//! d'écran, 8 sortie par le haut.

use super::cx::{handlers, Cx};
use super::enemy_common::*;
use crate::objects::Obj;

/// Jeux d'animations : vol et attaques.
const SET_FLY: u32 = 0x16a10c;
const SET_ATTACK: u32 = 0x16a116;
const SET_HIT: u32 = 0x16a132;
/// Lignes de huit octets par variante (orientation, durées, décalage en X) et décalages en Y selon `+0x42`.
const VARIANT_ROWS: u32 = 0x756e6;
const OFFSETS_Y: u32 = 0x757d2;
/// Animation de réaction au coup, selon la série d'attaque `+0x2f`.
const HIT_ANIM: u32 = 0x75e84;

mod g {
    pub const SERIES: u32 = 0x2f;
    pub const PARAM: u32 = 0x42;
    pub const KIND: u32 = 0x43;
    pub const COMPANION: u32 = 0xaa;
    pub const TIMER_A: u32 = 0xa0;
    pub const TIMER_B: u32 = 0xa1;
    pub const STEP: u32 = 0xa2;
    pub const PASSES: u32 = 0xa3;
    pub const SHOT_DONE: u32 = 0xa5;
    pub const SHOT: u32 = 0xa6;
    pub const EXIT_LIMIT: u32 = 0xae;
    pub const TURN_Z: u32 = 0xb0;
    pub const RESUME: u32 = 0xb1;
}
use g::*;
use crate::objects::Courant;

mod call {
    pub const CREATE_COMPANION: u32 = 0xa036e;
}

pub fn run(x: &mut Cx) {
    if suspended(x) {
        return;
    }
    match x.u8(f::STATE) {
        st::INIT => init(x),
        st::ACTIVE => active(x),
        st::HIT => hit(x),
        st::DEAD => dead(x),
        _ => {}
    }
}

// --- aides ------------------------------------------------------------------------------------

fn step(x: &mut Cx) {
    x.anim_step();
}

fn set_state_long(x: &mut Cx, v: u32) {
    x.set32(f::STATE, v);
}

/// Remise en liste : la variante 8 vit dans la liste du premier plan.
fn reinsert(x: &mut Cx) {
    if x.u8(Obj::VARIANT) == 8 {
        x.reinsert_foreground();
    } else {
        x.reinsert();
    }
}

/// Le compagnon (`+0xaa`) passe à l'état 6 (libération).
fn release_companion(x: &mut Cx) {
    let c = x.linked(COMPANION);
    c.set_u8(x.b, f::STATE, 6);
}

/// Fin du vol : la créature et son compagnon passent à l'état suivant.
fn vanish(x: &mut Cx) {
    x.add8(f::STATE, 4);
    release_companion(x);
}

/// Sortie d'écran : comme `vanish`, en retirant d'abord la créature de la scène (0x27ee).
fn leave(x: &mut Cx) {
    x.add8(f::STATE, 4);
    detach_from_host(x);
    release_companion(x);
}

fn speed(x: &mut Cx, n: u32) {
    x.select_speed(&crate::objects::motion::SPEEDS_B, n as u16);
}

fn move_xz_table(x: &mut Cx) {
    x.move_by_table_xz();
}

/// Un tour de l'angle de vol : toutes les `reload` images, `+0xa2` s'ajoute à la direction. En vol libre
/// (`clamp`, 0x758aa) un angle devenu négatif est remis à zéro.
fn bank(x: &mut Cx, reload: u8, clamp: bool) {
    let t = x.u8(TIMER_A).wrapping_sub(1);
    x.set8(TIMER_A, t);
    if (t as i8) < 0 {
        x.set8(TIMER_A, reload);
        let mut angle = x.u8(Obj::SPEED_INDEX).wrapping_add(x.u8(STEP));
        if clamp && (angle as i8) < 0 {
            angle = 0;
        }
        x.set8(Obj::SPEED_INDEX, angle);
    }
}

// --- état 0 : création --------------------------------------------------------------------------

fn init(x: &mut Cx) {
    x.add8(f::STATE, 2);
    x.set8(0, 1);
    x.set8(f::BOX_SET, 0xe);
    lib::initial_hp(x);
    lib::set_immunities(x);
    x.set8(0x68, 4);
    x.set8(0x69, 4);
    x.set8(0x76, 4);
    x.set8(0x67, 0);
    let d0 = x.c.d[0] as u8;
    x.set8(0x8e, d0);
    x.set8(0x7f, 0);
    x.set8(0x7e, 0xc);
    x.set32(0x8a, 0);
    x.set8(PASSES, 2);
    // ligne de la variante : orientation, durées `+0xa0..+0xa3`, décalage en X
    let row = VARIANT_ROWS + x.u8(Obj::VARIANT).wrapping_shl(3) as u32;
    let flip = x.data16(row);
    x.set16(f::FLIP, flip);
    let timers = x.data32(row + 2);
    x.set32(TIMER_A, timers);
    let dx = x.data16(row + 6);
    x.add16(0x8, dx);
    let mut d4 = x.u16(0xc) as u32;
    if x.u8(KIND) != 0 {
        d4 = d4 & !0xff | x.u8(PARAM) as u32;
        let a2 = OFFSETS_Y.wrapping_add(d4 as u16 as i16 as i32 as u32);
        let dy = x.data16(a2);
        x.add16(0x10, dy);
        d4 = d4 & !0xffff | x.data16(a2 + 2) as u32;
    }
    let d4 = d4 as u16;
    let variant = x.u8(Obj::VARIANT);
    if variant == 8 || (variant as i8) < 2 {
        x.set16(EXIT_LIMIT, 0x100u16.wrapping_add(d4));
        x.set16(0xc, d4);
    } else {
        let limit = if variant & 1 != 0 { -0x100i16 as u16 } else { 0x280 };
        x.set16(EXIT_LIMIT, limit);
    }
    let anim = (x.u8(KIND).wrapping_add(x.u8(SERIES)) as i8) as i16 as u16;
    x.anim_start(SET_FLY, (anim as u32) as u16);
    speed(x, 0x18);
    if variant >> 1 == 0 {
        let vz = x.u16(0x1c);
        x.set16(0x18, vz);
    }
    x.call(call::CREATE_COMPANION);
    if x.c.cond(5) {
        x.add8(f::STATE, 4);
    }
    let companion = x.c.a[1] as u16;
    x.set16(COMPANION, companion);
    reinsert(x);
}

// --- état 2 : actif -----------------------------------------------------------------------------

fn active(x: &mut Cx) {
    let freeze = x.u8(f::FREEZE) as i8;
    if freeze > 0 {
        x.set8(f::FREEZE, (freeze - 1) as u8);
    }
    match x.u8(f::SUB) {
        0 => fly(x),
        2 => attack(x),
        4 => pause(x),
        6 => exit_flight(x),
        8 => attack_end(x),
        _ => {}
    }
    reinsert(x);
}

/// +5 = 0 (0x7594a) : vol de la variante.
fn fly(x: &mut Cx) {
    if grabbed(x) {
        return set_state_long(x, 0x0206_0000);
    }
    match x.u8(Obj::VARIANT) {
        0 | 1 => fly_loop(x, 0xc),
        2..=5 => fly_loop_b(x),
        6 | 7 => turn_back(x),
        8 => fly_front(x),
        _ => {}
    }
}

/// Fin d'un passage : `+0xa1` écoulé, la créature repart ou attaque (variantes 0 et 1).
fn fly_loop(x: &mut Cx, reload: u8) {
    move_xz_table(x);
    step(x);
    bank(x, reload, false);
    let t = x.u8(TIMER_B).wrapping_sub(1);
    x.set8(TIMER_B, t);
    if (t as i8) < 0 {
        let passes = x.u8(PASSES).wrapping_sub(1);
        x.set8(PASSES, passes);
        if (passes as i8) < 0 {
            return x.add8(f::SUB, 6);
        }
        x.set8(TIMER_B, 0x14);
        x.add8(f::SUB, 2);
        return x.set8(RESUME, 0);
    }
    if (x.u16(0xc) as i16) > (x.u16(EXIT_LIMIT) as i16) {
        vanish(x);
    }
}

/// Variantes 2 à 5 (0x75aec) : même vol, passage plus lent et pause de 0x30 images.
fn fly_loop_b(x: &mut Cx) {
    move_xz_table(x);
    step(x);
    bank(x, 0x10, false);
    let t = x.u8(TIMER_B).wrapping_sub(1);
    x.set8(TIMER_B, t);
    if (t as i8) < 0 {
        let passes = x.u8(PASSES).wrapping_sub(1);
        x.set8(PASSES, passes);
        if (passes as i8) < 0 {
            return x.add8(f::SUB, 6);
        }
        x.set8(RESUME, 0);
        x.set8(TIMER_B, 0x30);
        x.add8(f::SUB, 2);
    }
}

/// Variante 8 (0x75aa2) : vol au premier plan.
fn fly_front(x: &mut Cx) {
    move_xz_table(x);
    step(x);
    bank(x, 0xc, false);
    let t = x.u8(TIMER_B).wrapping_sub(1);
    x.set8(TIMER_B, t);
    if (t as i8) < 0 {
        return set_state_long(x, 0x0208_0000);
    }
    if (x.u16(0xc) as i16) > (x.u16(EXIT_LIMIT) as i16) {
        vanish(x);
    }
}

/// Variantes 6 et 7 (0x75980) : traverse l'écran puis fait demi-tour.
fn turn_back(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            x.move_x();
            let t = x.u8(TIMER_B).wrapping_sub(1);
            x.set8(TIMER_B, t);
            if (t as i8) < 0 {
                let z = x.u16(0xc);
                x.set16(TURN_Z, z);
                x.set8(Obj::SPEED_INDEX, 0);
                x.set8(RESUME, 2);
                set_state_long(x, 0x0202_0000);
            }
        }
        2 => {
            move_xz_table(x);
            step(x);
            if (x.u16(0xc) as i16) >= 0xf0 {
                let mut d = -0x30i16;
                if x.u8(f::FLIP) != 0 {
                    d = -d;
                }
                x.add16(0x8, d as u16);
                x.set8(Obj::SPEED_INDEX, 0x10);
                x.add8(f::SUB2, 2);
                let c = x.linked(COMPANION);
                let flip = c.u8(x.b, f::FLIP) ^ 0x20;
                c.set_u8(x.b, f::FLIP, flip);
                let own = x.u8(f::FLIP) ^ 0x20;
                x.set8(f::FLIP, own);
            }
        }
        4 => {
            move_xz_table(x);
            step(x);
            if (x.u16(TURN_Z) as i16) > (x.u16(0xc) as i16) {
                let angle = if x.u8(f::FLIP) == 0 { 8 } else { 0x18 };
                x.set8(Obj::SPEED_INDEX, angle);
                let z = x.u16(TURN_Z);
                x.set16(0xc, z);
                set_state_long(x, 0x0202_0000);
                x.set8(RESUME, 6);
            }
        }
        6 => {
            set_state_long(x, 0x0206_0000);
            exit_flight(x);
        }
        _ => {}
    }
}

/// +5 = 4 (0x75b2e) : attente puis retour au vol.
fn pause(x: &mut Cx) {
    step(x);
    let n = x.u8(0x77).wrapping_sub(1);
    x.set8(0x77, n);
    if (n as i8) < 0 {
        x.set8(f::SUB, 0);
    }
}

/// +5 = 6 (0x75884) : sortie d'écran selon la variante.
fn exit_flight(x: &mut Cx) {
    move_xz_table(x);
    step(x);
    match x.u8(Obj::VARIANT) >> 1 {
        0 => {
            bank(x, 0x14, true);
            if x.u16(0xc) > x.u16(EXIT_LIMIT) {
                leave(x);
            }
        }
        1..=3 => {
            let mut left = var::SCROLL_X.get(x.b);
            if x.u8(PARAM) == 4 {
                left = var::CAMERA_ALT_X.get(x.b);
            }
            let limit = x.u16(EXIT_LIMIT).wrapping_add(left) as i16;
            let here = x.u16(0x8).wrapping_add(left) as i16;
            let gone = if x.u8(Obj::VARIANT) & 1 == 0 { limit <= here } else { limit >= here };
            if gone {
                leave(x);
            }
        }
        _ => {
            let top = var::SCROLL_Y.get(x.b).wrapping_add(0x100);
            if top <= x.u16(0xc) {
                leave(x);
            }
        }
    }
}

/// +5 = 8 (0x75b40) : fin du vol de la variante 8.
fn attack_end(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => attack_start(x),
        2 => attack_aim(x),
        4 => attack_wait(x),
        _ => {}
    }
}

// --- sous-état 2 : attaque ----------------------------------------------------------------------

fn attack(x: &mut Cx) {
    match x.u8(SERIES) {
        0 => attack_shots(x),
        2 => attack_claws(x),
        4 => attack_fire(x),
        _ => {}
    }
}

fn attack_shots(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => attack_start(x),
        2 => attack_aim(x),
        4 => attack_wait(x),
        6 => attack_return(x),
        _ => {}
    }
}

/// 0x75b7c : la créature vise un joueur et lance un projectile.
fn attack_start(x: &mut Cx) {
    x.add8(f::SUB2, 2);
    let target_ok = pick_target(x);
    if target_ok {
        x.c.a[2] = x.c.a[1];
        x.c.d[5] = 0;
        x.call(0xa0566);
        if !x.c.cond(5) {
            let shot = x.c.a[1];
            x.set32(SHOT, shot);
            let n = x.u8(KIND) as u32;
            x.anim_start(SET_ATTACK, n as u16);
            return lib::sound(x, 0xed5b2);
        }
    }
    set_state_long(x, 0x0204_0000);
    x.set8(0x77, 0x1e);
    let n = x.u8(KIND) as u32;
    x.anim_start(SET_FLY, n as u16);
}

/// 0x75bd0 : attend la fin du geste de tir.
fn attack_aim(x: &mut Cx) {
    step(x);
    if x.u8(SHOT_DONE) != 0 {
        x.add8(f::SUB2, 2);
        let n = 2 + x.u8(KIND) as u32;
        x.anim_start(SET_ATTACK, n as u16);
    }
}

/// 0x75bf4 : lâche le projectile à l'image-clé.
fn attack_wait(x: &mut Cx) {
    if x.anim_step() {
        return x.add8(f::SUB2, 2);
    }
    if x.u8(f::KEY_FRAME) != 0 {
        x.set8(f::KEY_FRAME, 0);
        let shot = Obj(x.u32(SHOT) & 0xff_ffff);
        shot.set_u8(x.b, 0x55, 1);
        x.set8(TIMER_A, 0x1e);
        lib::sound(x, 0xed61c);
    }
}

/// 0x75c36 : retour au vol après le tir.
fn attack_return(x: &mut Cx) {
    let t = x.u8(TIMER_A).wrapping_sub(1);
    x.set8(TIMER_A, t);
    if (t as i8) >= 0 {
        return;
    }
    set_state_long(x, 0x0202_0000);
    if random(x) & 1 != 0 {
        let n = x.u8(0xa4).wrapping_sub(1);
        x.set8(0xa4, n);
        if (n as i8) >= 0 {
            return;
        }
        x.set8(0xa4, 2);
    }
    set_state_long(x, 0x0200_0000);
    let resume = x.u8(RESUME);
    x.set8(f::SUB2, resume);
}

fn attack_claws(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            let n = 4 + x.u8(KIND) as u32;
            x.anim_start(SET_ATTACK, n as u16);
        }
        2 => {
            if x.anim_step() {
                x.add8(f::SUB2, 2);
                let n = 6 + x.u8(KIND) as u32;
                x.anim_start(SET_ATTACK, n as u16);
            }
        }
        4 => {
            if x.anim_step() {
                x.set8(TIMER_A, 0x1e);
                x.add8(f::SUB2, 2);
                let n = 2 + x.u8(KIND) as u32;
                x.anim_start(SET_FLY, n as u16);
            } else if x.u8(f::KEY_FRAME) != 0 {
                x.set8(f::KEY_FRAME, 0);
                if pick_target(x) {
                    if let (Some(shot), false) = lib::eshl_12(x) {
                        x.set32(SHOT, shot.0);
                    }
                }
            }
        }
        _ => attack_return(x),
    }
}

fn attack_fire(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.anim_start(SET_ATTACK, 9);
        }
        2 => {
            if x.anim_step() {
                x.add8(f::SUB2, 2);
                x.set8(TIMER_A, 0x30);
                x.anim_start(SET_FLY, 4);
            } else if x.u8(f::KEY_FRAME) != 0 {
                x.set8(f::KEY_FRAME, 0);
                if pick_target(x) {
                    x.c.a[2] = x.c.a[1];
                    let target = x.c.a[2];
                    if let Some(shot) = lib::eshl_13(x, target) {
                        x.set32(SHOT, shot.0);
                    }
                }
            }
        }
        _ => attack_return(x),
    }
}

// --- état 4 : touché ----------------------------------------------------------------------------

fn hit(x: &mut Cx) {
    let freeze = x.u8(f::FREEZE) as i8;
    if freeze > 0 {
        x.set8(f::FREEZE, (freeze - 1) as u8);
    } else {
        match x.u8(f::SUB) {
            0 => hit_recoil(x),
            2 => hit_fall(x),
            4 => hit_dying(x),
            _ => {}
        }
    }
    x.reinsert_foreground();
}

/// Animation de chute de la série d'attaque (`+0x2f`).
fn hit_anim(x: &mut Cx) -> u8 {
    let series = x.u8(SERIES) as u32;
    x.rom8(HIT_ANIM + series)
}

fn hit_recoil(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            let c = x.linked(COMPANION);
            c.set_u8(x.b, 0x5a, 1);
            x.set8(f::FREEZE, 1);
            x.anim_start(SET_HIT, 0xf);
        }
        2 => {
            if x.anim_step() {
                x.add8(f::SUB2, 2);
                x.set16(0x18, 0x400);
                x.set16(0x1a, (-0x40i16) as u16);
                let vx = if x.u8(f::FLIP) != 0 { -0x200i16 } else { 0x200 };
                x.set16(0x14, vx as u16);
                x.set16(0x16, 0);
                x.anim_start(SET_HIT, 0x10);
            }
        }
        4 => {
            step(x);
            x.move_xz();
            let z = x.u16(0xc).wrapping_add(x.u16(0x10));
            let mut limit = var::SCROLL_Y.get(x.b);
            if var::CAMERA_SELECT.get(x.b) != 0 {
                limit = var::CAMERA_ALT_Y.get(x.b);
            }
            if (z as i16) <= (limit as i16) {
                x.add8(f::STATE, 2);
            }
        }
        _ => {}
    }
}

fn hit_fall(x: &mut Cx) {
    let c = x.linked(COMPANION);
    c.set_u8(x.b, 0x5a, 1);
    let kind = x.u8(KIND) != 0;
    if kind && x.u8(f::SUB2) != 0 {
        if x.u8(1) & 2 == 0 {
            x.add8(f::STATE, 2);
        }
        x.move_z();
        return step(x);
    }
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set32(0x58, 0);
            let base = hit_anim(x) as u32;
            x.set8(TIMER_A, 0xf);
            x.set8(RESUME, base as u8 + 2);
            x.set16(0x18, (-0x300i16) as u16);
            let n = base.wrapping_add(x.u8(KIND) as u32) & 0xff;
            x.anim_start(SET_HIT, n as u16);
        }
        2 => {
            if (x.u16(0xc) as i16) >= 0 {
                x.move_z();
                return step(x);
            }
            x.add8(f::SUB2, 2);
            x.set32(0xc, 0);
            x.set8(TIMER_A, 0x10);
            let n = x.u8(RESUME) as i8 as i16 as u16;
            x.anim_start(SET_HIT, (n as u32) as u16);
        }
        _ => {
            let t = x.u8(TIMER_A).wrapping_sub(1);
            x.set8(TIMER_A, t);
            if (t as i8) < 0 {
                x.add8(f::STATE, 2);
            }
        }
    }
}

fn hit_dying(x: &mut Cx) {
    if x.u8(f::SUB2) != 0 {
        if x.anim_step() {
            set_state_long(x, 0x0200_0000);
        }
        return;
    }
    x.add8(f::SUB2, 2);
    if (x.u8(f::HP) as i8) < 0 {
        set_state_long(x, 0x0400_0000);
    }
    x.set32(0x58, 0);
    let base = hit_anim(x).wrapping_add(x.u8(KIND)).wrapping_sub(2);
    x.anim_start(SET_HIT, (base as u32) as u16);
}

// --- état 6 : libération ---------------------------------------------------------------------------

fn dead(x: &mut Cx) {
    let d = x.u16(0x48) & 0xff00;
    set_dw(x, 0, d.wrapping_add(0x12));
    detach_from_host(x);
    free(x);
}

handlers! {
    0x756c4 "enemy_21" run,
}
