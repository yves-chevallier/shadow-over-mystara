//! Boss 3, état 4 « touché » (0x80a90). Groupes `+5` :
//!
//! | `+5` | Rôle |
//! |---|---|
//! | 0 | réaction au coup : routine de la table 0x80b0a (code `+0x49`) |
//! | 2 | recul en vol |
//! | 4 | à terre, puis relevé (riposte tirée au sort) |
//! | 6 | mort : chute, rebond, effondrement |
//! | 10 | retour au combat |

use super::*;
use crate::objects::Courant;

/// Table des routines de réaction, indexée par le code de coup `+0x49` (longs, toutes communes).
const REACTIONS: u32 = 0x80b0a;

pub(super) fn run(x: &mut Cx) {
    if x.u8(f::SUB) == 0 && !tst16(x, f::HP).gt() {
        x.set8(f::SUB, 6);
        x.set8(f::SUB2, 0);
        return death(x);
    }
    let freeze = x.u8(f::FREEZE);
    if freeze != 0 && (freeze as i8) > 0 {
        x.set8(f::FREEZE, freeze - 1);
    } else {
        x.c.d[0] = x.u8(f::SUB) as u32;
        match x.u8(f::SUB) {
            0 => reaction(x),
            2 => pushed_back(x),
            4 => down(x),
            6 => death(x),
            8 => {}
            _ => after_attack(x),
        }
    }
    end_frame(x, 0x50);
}

/// 0x80ae8 : premier passage : immunités du type ; puis la routine commune du code de coup.
fn reaction(x: &mut Cx) {
    x.set8(HOVER, 0xff);
    if x.u8(f::SUB2) == 0 {
        x.set_bit(0x1, 1, false);
        lib::set_immunities(x);
    }
    let code = x.u8(0x49);
    x.c.d[0] = x.c.d[0] & 0xffff_ff00 | code as u32;
    let i = x.c.d[0] as u16 as i16 as u32;
    let routine = rom32(x, REACTIONS.wrapping_add(i));
    x.c.a[1] = routine;
    x.call(routine);
}

/// 0x80bb6 : recul en vol (animation 9) jusqu'à l'arrêt ; il se retrouve à terre.
fn pushed_back(x: &mut Cx) {
    if x.u8(f::SUB2) == 0 {
        next6(x);
        x.set8(0x68, 1);
        x.set8(0x69, 1);
        x.set16(0x14, 0x400);
        x.set16(0x16, (-0x88i16) as u16);
        if x.u8(f::FLIP) != 0 {
            negate(x, 0x14);
            negate(x, 0x16);
        }
        return body_anim(x, 9);
    }
    x.move_x();
    if (x.u8(0x14) as i8) < 0 {
        return stop(x);
    }
    if !lib::step(x) {
        return;
    }
    x.set32(0x58, 0);
    x.set32(f::STATE, 0x0204_0400);
    x.set8(INVULNERABLE, 6);
}

/// 0x80c2c : à terre (`+0x84` images hors pause), puis relevé ou riposte.
fn down(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            next6(x);
            lib::behaviour_by_kind(x);
            body_anim(x, 0x11);
        }
        2 => {
            if var::PAUSE.get(x.b) != 0 || sub16(x, DOWN_TIME, 1).gt() {
                return;
            }
            x.set16(DOWN_TIME, 0);
            next6(x);
            x.set8(0x86, 0);
            x.set8(0x49, 0);
            // 0x80cb0 : riposte 6 chances sur 16
            let r = rand_and(x, 0xf);
            if 0xfc00u32 >> r & 1 == 0 {
                return;
            }
            let r = rand_and(x, 0xf);
            let k = x.rom8(0x80cf6 + r);
            x.set8(f::SUB2, k);
            x.set16(f::STATE, 0x0204);
            x.set8(0x7, 0);
            x.set8(0x86, 0);
            x.set8(VULNERABLE, 1);
            x.set8(INVULNERABLE, 6);
            x.set8(0x38, 0);
            x.set8(0, 1);
        }
        _ => {
            if var::PAUSE.get(x.b) != 0 || !lib::step(x) {
                return;
            }
            x.set8(VULNERABLE, 1);
            x.set8(0, 1);
            x.set32(f::STATE, 0x0202_0400);
            body_anim(x, 0);
        }
    }
}

/// 0x80d16 : mort : la scène est prévenue (0x300 : boss 3 vaincu), chute, rebond, effondrement.
fn death(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            next6(x);
            x.set_a5_u8(0x42, 1);
            x.set_a5_u8(0x50, 0);
            crate::mem::wr16(x.b, A5_DEATH, 0x300);
            let score = x.u8(0x7e);
            set_abs8(x, A5_DEATH + 2, score);
            lib::drop_rewards(x);
            crate::sound::service(x.b, crate::sound::Service::Ff05);
            lib::sound(x, 0xe3b6c);
            lib::flash_and_shake(x);
            x.set8(0, 2);
            let f = x.u8(0x56);
            x.set8(f::FREEZE, f);
            stop(x);
            x.set16(0x18, 0x400);
            x.set16(0x1a, (-0x40i16) as u16);
            let mut v = 0x200u16;
            if x.u8(f::FLIP) != 0 {
                v = v.wrapping_neg();
            }
            set_dw(x, 0, v);
            x.set16(0x14, v);
            x.anim_start(SET_DEATH, 2);
            falling(x)
        }
        2 => falling(x),
        4 => {
            x.move_x();
            if !x.anim_step() {
                return;
            }
            next6(x);
            x.anim_start(SET_DEATH, 0x16);
        }
        _ => {}
    }
}

/// 0x80da2 : chute jusqu'au sol, puis rebond.
fn falling(x: &mut Cx) {
    x.anim_step();
    x.move_xz();
    if (x.u16(0xc) as i16) >= 0 {
        return;
    }
    x.set16(0xc, 0);
    let v = ((x.u16(0x14) as i16) >> 2) as u16;
    x.set16(0x14, v);
    x.set32(0x1c, 0);
    x.set32(0x18, 0);
    next6(x);
    x.anim_start(SET_DEATH, 0x10);
    lib::sound(x, 0xecabc);
}

/// Boss vaincu : type et variante (mot), score (`A5-0x557c..`), lus par la fin de scène.
const A5_DEATH: u32 = 0xff8000 - 0x557c;
