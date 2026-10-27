//! Boss 14 / 19, état 4 « touché » (0x908ea). Groupes `+5` : 0 et 4 réaction au coup (table 0x909a0 selon le code
//! `+0x49`), 2 recul (la routine 0x90cb8), 6 mort.
//!
//! Une touche qui arrive pendant le souffle (attaque 4) est sans effet sur l'attaque : le dragon la reprend (`resume`).
//! Les routines de réaction propres au dragon sont 0x90b26 (coup ordinaire : recul, glissade), 0x90a4c (code 0x68 :
//! couché), 0x90c34 (0x80) et 0x90a96 (0xa8 : tremblement puis mort).

use super::*;
use crate::objects::Courant;
use crate::objects::Obj;

/// Table des routines de réaction, indexée par le code de coup `+0x49` (longs).
const REACTIONS: u32 = 0x909a0;
/// Décalages du tremblement (mots relatifs à 0x90ada).
const SHAKE_JUMPS: u32 = 0x90ada;

pub(super) fn run(x: &mut Cx) {
    let alive = (x.u16(f::HP) as i16) > 0;
    if alive && x.u8(f::SUB) == 0 && x.u32(SAVED_STATE) & 0xffff_ff00 == ATTACK_4 && x.a5_u8(var::PAUSE) == 0 {
        // touché pendant le souffle : le dragon le reprend
        x.set8(SHAKE, 0x10);
        x.set8(SHIELD, 8);
        let s = x.u32(SAVED_STATE);
        x.set32(f::STATE, s);
        x.set8(0, 1);
        x.set8(REACTION, 0);
        x.set8(PUSHED, 0x30);
        return active(x);
    }
    if alive && x.u8(f::SUB) != 0 {
        return dispatch(x);
    }
    x.set32(SAVED_STATE, 0);
    if tst8(x, f::FREEZE).gt() {
        x.set8(SHIELD, 2);
        sub8(x, f::FREEZE, 1);
        return tail(x);
    }
    dispatch(x);
}

fn dispatch(x: &mut Cx) {
    match x.u8(f::SUB) {
        0 | 4 => reaction(x),
        2 => pushed_back(x),
        6 => death(x),
        _ => {}
    }
    if tst8(x, SHIELD).gt() {
        sub8(x, SHIELD, 1);
        x.set8(0x38, 0);
    }
    tail(x);
}

fn tail(x: &mut Cx) {
    x.call(SHADOW);
    x.reinsert();
}

/// 0x9097c : réaction au coup : la routine de la table, d'après le code du coup.
fn reaction(x: &mut Cx) {
    if x.u8(f::SUB2) == 0 {
        lib::set_68(x);
        if x.a5_u8(0x34) == 0x20 {
            lib::drop_once(x);
        }
    }
    let index = ((x.c.d[0] & 0xff00) | x.u8(REACTION) as u32) as u16 as i16 as i32;
    let routine = rom32(x, (REACTIONS as i32 + index) as u32);
    match routine {
        0x90b26 => knock(x),
        0x90a4c => lie_down(x),
        0x90c34 => stagger(x),
        0x90a96 => shake(x),
        t => unreachable!("boss 19 : réaction {t:x}"),
    }
}

// --- 0x90b26 : coup ordinaire ----------------------------------------------------------------------

fn knock(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            if (x.u16(f::HP) as i16) <= 0 {
                x.set8(f::SUB, 6);
                return death(x);
            }
            if retaliate(x) {
                return;
            }
            x.add8(f::SUB2, 2);
            let burst = match x.u8(REACTION) {
                0x60 => Some(7),
                0x70 | 0x90 => Some(8),
                0x78 => Some(1),
                0x94 => Some(9),
                _ => None,
            };
            if let Some(n) = burst {
                lib::attack_effect(x, n, 0);
            }
            let stun = x.u8(STUN);
            x.set8(f::FREEZE, stun);
            x.set8(SHAKE, 0x10);
            let (vx, vz) = if x.u8(f::FLIP) != 0 { (-0x800i16, 0x80i16) } else { (0x800, -0x80) };
            x.set16(0x14, vx as u16);
            x.set16(0x16, vz as u16);
            x.set32(0x18, 0);
            x.set32(0x1c, 0);
            rest(x);
            knock_slide(x);
        }
        2 => knock_slide(x),
        _ => {}
    }
}

/// 0x90c8c : la riposte est due (le compte `+0xac` est épuisé) : le dragon attaque (attaque 10) au lieu de reculer.
fn retaliate(x: &mut Cx) -> bool {
    if sub8(x, RETALIATE, 1).gt() {
        return false;
    }
    x.set8(0, 1);
    x.set32(f::STATE, ATTACK_10);
    x.set8(SHIELD, 8);
    x.set8(REACTION, 0);
    x.set32(0x58, 0);
    true
}

fn knock_slide(x: &mut Cx) {
    if x.a5_u8(var::PAUSE) != 0 {
        x.set8(0, 2);
        return;
    }
    x.set8(0, 1);
    if x.u16(0x14) != 0 {
        x.move_xz();
        let mut v = x.u16(0x14) as i16;
        if x.u8(f::FLIP) == 0 {
            v = v.wrapping_neg();
        }
        if v < 0 {
            return;
        }
        x.set32(0x14, 0);
        return;
    }
    if tst8(x, SHAKE).gt() {
        sub8(x, SHAKE, 1);
        let mut d: i16 = 2;
        if x.u8(SHAKE) & 1 != 0 {
            d = -d;
        }
        x.add16(0x8, d as u16);
        return;
    }
    recover(x);
}

/// 0x90c6c : fin de la réaction : debout.
fn recover(x: &mut Cx) {
    x.set8(0, 1);
    x.set8(0x87, 0);
    x.set8(REACTION, 0);
    x.set32(0x58, 0);
    stand(x);
}

// --- 0x90a4c : couché (code 0x68) ------------------------------------------------------------------------

fn lie_down(x: &mut Cx) {
    x.set8(HURT_BOX, 5);
    if x.u8(f::SUB2) != 0 {
        if sub8(x, 0x87, 1).eq() {
            return recover(x);
        }
        x.set8(HOVER, 1);
        hover(x);
        x.anim_step();
        return;
    }
    if (x.u16(f::HP) as i16) <= 0 {
        x.set8(f::SUB, 6);
        return death(x);
    }
    x.add8(f::SUB2, 2);
    rest(x);
    lib::attack_effect(x, 3, 0);
}

// --- 0x90c34 : chancelant (code 0x80) ----------------------------------------------------------------------

fn stagger(x: &mut Cx) {
    x.set8(HURT_BOX, 5);
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            rest(x);
            stagger_wait(x);
        }
        2 => stagger_wait(x),
        _ => {}
    }
}

fn stagger_wait(x: &mut Cx) {
    if x.a5_u8(var::PAUSE) != 0 {
        return;
    }
    x.set8(REACTION, 0);
    x.set8(f::SUB2, 0);
    knock(x);
}

// --- 0x90a96 : tremblement puis mort (code 0xa8) ---------------------------------------------------------------

fn shake(x: &mut Cx) {
    if x.u8(f::SUB2) == 0 {
        x.add8(f::SUB2, 2);
        let z = x.u16(0xc);
        x.set16(0x18, z);
        let px = x.u16(0x8);
        x.set16(0x1a, px);
        x.set16(TIMER, 0x100);
    }
    let t = x.u16(TIMER).wrapping_sub(1);
    if t & 0x8000 != 0 {
        let (px, z) = (x.u16(0x1a), x.u16(0x18));
        x.set16(0x8, px);
        x.set16(0xc, z);
        x.set16(f::HP, 0);
        x.set8(f::SUB, 6);
        x.set16(f::SUB2, 0);
        return death(x);
    }
    x.set16(TIMER, t);
    let mut d0 = t as i16;
    match x.rom16(SHAKE_JUMPS + 2 * (t as u32 & 3)) as i16 as i32 + SHAKE_JUMPS as i32 {
        0x90ae2 => {
            d0 = d0.wrapping_neg();
            shake_x(x, d0);
        }
        0x90ae4 => shake_x(x, d0),
        0x90af8 => {
            d0 = d0.wrapping_neg();
            shake_z(x, d0);
        }
        0x90afa => shake_z(x, d0),
        t => unreachable!("boss 19 : tremblement {t:x}"),
    }
}

fn shake_x(x: &mut Cx, d0: i16) {
    let px = x.u16(0x1a).wrapping_add((d0 >> 2) as u16);
    x.set16(0x8, px);
    let z = x.u16(0x18);
    x.set16(0xc, z);
}

fn shake_z(x: &mut Cx, d0: i16) {
    let z = x.u16(0x18).wrapping_add((d0 >> 1) as u16);
    x.set16(0xc, z);
}

// --- +5 = 2 : recul par un gros coup (0x90cb8) --------------------------------------------------------------------

fn pushed_back(x: &mut Cx) {
    if x.a5_u8(var::PAUSE) != 0 {
        return;
    }
    if x.u8(f::SUB2) == 0 {
        x.add8(f::SUB2, 2);
        let half = x.u8(STUN) >> 1;
        x.set8(f::FREEZE, half);
        x.set16(TIMER, 0xf);
        x.set16(0x14, 0x480);
        x.set16(0x16, (-0x80i16) as u16);
        let side = x.u8(0x4a) ^ 0x20;
        x.set8(f::FLIP, side);
        if side != 0 {
            let (a, b) = (x.u16(0x14).wrapping_neg(), x.u16(0x16).wrapping_neg());
            x.set16(0x14, a);
            x.set16(0x16, b);
        }
    }
    if x.u16(TIMER) != 0 {
        let t = x.u16(TIMER) - 1;
        x.set16(TIMER, t);
        if t != 0 {
            x.anim_step();
            return;
        }
    }
    if x.u8(0x14) != 0 {
        x.move_x();
        return;
    }
    x.set32(0x58, 0);
    stand(x);
}

// --- mort -----------------------------------------------------------------------------------------------------------

/// 0x90dca : séquence de mort, propre à chaque type et variante.
fn death(x: &mut Cx) {
    x.set8(HURT_BOX, 0);
    if x.u8(TYPE) == TYPE_GREEN {
        return death_green(x);
    }
    if x.u8(Obj::VARIANT) != 0 {
        return death_pet(x);
    }
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(0, 2);
            x.set_a5_u8(0x42, 1);
            x.set_a5_u8(0x50, 0);
            set_abs16(x, 0xff2a84, 0x1300);
            let bar = x.u8(0x7e);
            set_abs8(x, 0xff2a86, bar);
            fall_start(x);
            for addr in [0xb780, 0xbad6, 0xe2f6, 0x103ba, 0xe0a10, 0xe4f42] {
                x.call(addr);
            }
        }
        2 => {
            x.move_xz();
            let cam_y = camera_y(x);
            let d = x.u16(0x10).wrapping_sub(cam_y).wrapping_add(x.u16(0xc));
            if (d as i16) <= 0x100 {
                return;
            }
            x.add8(f::SUB2, 2);
            x.set8(TIMER, 0x20);
            x.set16(0x10, cam_y);
            x.set16(0xc, 0x100);
            x.set32(0x14, 0);
            x.set32(0x1c, 0);
            x.set16(0x18, (-0x400i16) as u16);
            x.set16(0x1a, 0);
            set_anim(x, 4, 2);
            fall_end(x);
        }
        4 => fall_end(x),
        _ => {}
    }
}

fn camera_y(x: &mut Cx) -> u16 {
    if x.a5_u8(var::CAMERA_SELECT) != 0 {
        x.a5_u16(0x55a)
    } else {
        x.a5_u16(var::SCROLL_Y)
    }
}

fn set_abs16(x: &mut Cx, addr: u32, v: u16) {
    crate::mem::wr16(x.b, addr, v);
}

/// Début de la chute des dragons non verts : gel d'invulnérabilité, descente de 0x200.
fn fall_start(x: &mut Cx) {
    let stun = x.u8(STUN);
    x.set8(f::FREEZE, stun);
    x.set32(0x14, 0);
    x.set16(0x18, 0x200);
    x.set16(0x1a, 0);
    rest(x);
}

/// Fin de la chute : la caméra est dépassée par le bas, le dragon est libéré.
fn fall_end(x: &mut Cx) {
    x.move_xz();
    if (x.u16(0xc) as i16) > -0x100 {
        return;
    }
    release(x);
}

fn release(x: &mut Cx) {
    x.set32(f::STATE, FREE);
}

/// Dragon vert : même chute avec ses étapes propres.
fn death_green(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(0, 2);
            x.set_a5_u8(0x42, 1);
            x.set_a5_u8(0x50, 0);
            set_abs16(x, 0xff2a84, 0x0e00);
            let bar = x.u8(0x7e);
            set_abs8(x, 0xff2a86, bar);
            fall_start(x);
            for addr in [0xb780, 0xbad6, 0xe2f6, 0x103ba, 0xe0a10, 0xe51e6, 0xe3b6c] {
                x.call(addr);
            }
            green_fall(x);
        }
        2 => green_fall(x),
        4 => fall_end_green(x),
        _ => {}
    }
}

fn green_fall(x: &mut Cx) {
    x.move_xz();
    let cam_y = camera_y(x);
    let d = x.u16(0x10).wrapping_sub(cam_y).wrapping_add(x.u16(0xc));
    if (d as i16) <= 0x100 {
        return;
    }
    x.add8(f::SUB2, 2);
    x.set8(TIMER, 0x20);
    let y = x.u16(0x10).wrapping_add(0x50);
    x.set16(0x10, y);
    let z = x.u16(0xc).wrapping_sub(0x50);
    x.set16(0xc, z);
    x.set32(0x14, 0);
    x.set32(0x1c, 0);
    x.set16(0x18, (-0x400i16) as u16);
    x.set16(0x1a, 0);
    x.anim_start(0x17dbd2, 1);
    fall_end_green(x);
}

fn fall_end_green(x: &mut Cx) {
    x.move_xz();
    if (x.u16(0xc) as i16) > -0x100 {
        return;
    }
    release(x);
}

/// Dragon de Nagpa : disparaît en clignotant (0x90fa8).
fn death_pet(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(0, 2);
            x.set16(f::IMMUNITY, 0xffff);
            x.set16(TIMER, 0x20);
            lib::object_event(x);
            rest(x);
            for addr in [0xb780, 0xbad6, 0xe2f6, 0xe4f42] {
                x.call(addr);
            }
        }
        2 => {
            x.set_bit(0x1, 1, true);
            if x.u16(TIMER) & 1 == 0 {
                x.set_bit(0x1, 1, false);
            }
            if sub16(x, TIMER, 1).gt() {
                return;
            }
            release(x);
        }
        _ => {}
    }
}
