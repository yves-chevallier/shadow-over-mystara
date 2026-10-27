//! Boss 8, état 4 « touché » (0x89292). Groupes `+5` : 0 réaction (routine commune des ennemis choisie par le code de coup
//! `+0x49` dans la table de longs 0x89330), 2 projeté, 4 chute, 6 mort, 8 et 10 retour au combat.

use super::aids;
use super::*;
use crate::objects::Courant;
use crate::machine::Machine;

pub(super) fn run(x: &mut Cx) {
    x.set8(HIT_ACK, 0);
    if tst8(x, f::FREEZE).gt() {
        sub8(x, f::FREEZE, 1);
    } else {
        match dispatch(x, 0x892c2, f::SUB) {
            0x892ce => reaction(x),
            0x893dc => thrown(x),
            0x894f4 => fall(x),
            0x895c2 => death(x),
            _ => x.set32(f::STATE, 0x0202_0000),
        }
    }
    x.call(IN_ZONE);
    x.call(SHADOW);
}

fn step(x: &mut Cx, table: u32) -> u32 {
    dispatch(x, table, f::SUB2)
}

fn paused(x: &mut Cx) -> bool {
    x.a5_u8(var::PAUSE) != 0
}

/// 0x892ce : le coup est traité par la routine commune ; sans PV le boss meurt.
fn reaction(x: &mut Cx) {
    if x.u8(f::SUB2) == 0 {
        x.set_bit(0x1, 1, false);
    }
    if x.u8(REACTION) != 0xa8 {
        x.set8(0x87, 0);
    }
    x.call(0x10112);
    x.set8(PHASE_TIMER, 0);
    if (x.u16(f::HP) as i16) <= 0 {
        x.set8(f::SUB, 6);
        x.set8(0, 2);
        return death(x);
    }
    sub16(x, IDLE_WAIT, 1);
    let code = x.u8(REACTION) as u32;
    x.c.d[0] = x.c.d[0] & !0xff | code;
    let routine = rom32(x, 0x89330 + (x.c.d[0] & 0xffff));
    x.c.a[1] = routine;
    x.call(routine);
    if x.u8(f::FREEZE) != 0 && x.u8(0x46) != 0 {
        addq8(x, f::FREEZE, 1);
        x.set8(0x46, 0);
    }
}

/// 0x893dc : le boss est projeté en arrière, rebondit, puis se relève ou riposte.
fn thrown(x: &mut Cx) {
    if paused(x) {
        return;
    }
    match step(x, 0x893f0) {
        0x893f6 => {
            x.add8(f::SUB2, 2);
            x.set8(ANIM_KIND, 0x1e);
            x.set8(ANIM_LAST, 0x1e);
            x.call(0x10112);
            x.set8(0, 2);
            x.set16(0x14, 0x500);
            x.set16(0x16, (-0x28i16) as u16);
            let side = x.u8(0x4a) ^ 0x20;
            x.c.d[0] = x.c.d[0] & !0xff | side as u32;
            x.set8(f::FLIP, side);
            if side != 0 {
                let (a, b) = (x.u16(0x14).wrapping_neg(), x.u16(0x16).wrapping_neg());
                x.set16(0x14, a);
                x.set16(0x16, b);
            }
            body(x, 9);
        }
        0x89442 => {
            let v = x.a5_u16(0x6e) & 3;
            x.set_a5_u16(0x6e, v);
            if v == 0 {
                x.c.d[0] = 1;
                x.c.d[1] = 2;
                setw(x, 2, 0x20);
                setw(x, 3, 0xffff);
                x.call(call::BURST);
            }
            if x.u8(0x14) != 0 {
                x.move_x();
                return;
            }
            if !x.anim_step() {
                return;
            }
            x.set32(0x58, 0);
            x.set32(0x14, 0);
            let n = x.a5_u8(var::PLAYERS_ALIVE) as u32;
            x.c.d[5] = ((n as u16) << 2) as u32;
            let mask = rom32(x, 0x894cc + x.c.d[5]);
            x.c.d[5] = mask;
            let alea = x.random();
            let i = alea & 0x1f;
            setw(x, 0, i);
            if mask >> i & 1 == 0 {
                x.set8(0, 1);
                x.set32(f::STATE, 0x0202_0000);
                return;
            }
            x.set8(0x45, 5);
            x.set8(0x44, 0x44);
            x.set8(0, 1);
            x.add8(f::SUB2, 2);
            strike(x, 6);
        }
        _ => {
            if x.anim_step() {
                x.set32(f::STATE, 0x0202_0000);
            }
        }
    }
}

/// 0x894f4 : chute, attente au sol, relevé.
fn fall(x: &mut Cx) {
    match step(x, 0x89500) {
        0x8950a => {
            x.add8(f::SUB2, 2);
            x.call(0xfffc);
            x.set16(0xc, 0);
            body(x, 8);
        }
        0x89526 => {
            if paused(x) {
                return;
            }
            if sub16(x, HITS, 1).gt() {
                return;
            }
            x.add8(f::SUB2, 2);
            x.set8(0, 2);
            x.set16(HITS, 0);
            x.set8(0x86, 0);
            x.set8(REACTION, 0);
        }
        0x8954a => {
            if paused(x) || !x.anim_step() {
                return;
            }
            x.add8(f::SUB2, 2);
            aids::dodge_nearby(x);
            x.set16(TIMER, 8);
            x.set8(PHASE_TIMER, 0x10);
            x.set8(0, 1);
            x.set_bit(0x1, 1, true);
            body(x, 5);
        }
        0x89586 => {
            if paused(x) {
                return;
            }
            if sub16(x, TIMER, 1).mi() {
                x.add8(f::SUB2, 2);
                x.set_bit(0x1, 1, false);
            }
        }
        _ => {
            if paused(x) || !x.anim_step() {
                return;
            }
            x.set8(VULNERABLE, 1);
            x.set32(f::STATE, 0x0202_0000);
            x.set8(0, 1);
        }
    }
}

/// 0x895c2 : mort (chute, effets, attente, clignotement, libération).
fn death(x: &mut Cx) {
    match dispatch(x, 0x895d0, f::SUB2) {
        0x895da => {
            x.add8(f::SUB2, 2);
            x.call(0xe43ae);
            x.set8(0, 2);
            let stun = x.u8(STUN);
            x.set8(f::FREEZE, stun);
            x.set_a5_u8(0x42, 1);
            x.set_a5_u8(0x50, 0);
            for a in [0x103ba, 0xe3b6c, 0xe0a10, 0xb780, 0xbad6, 0xe2f6] {
                x.call(a);
            }
            x.set16(0x16, 0);
            x.set16(0x18, 0x500);
            x.set16(0x1a, (-0x38i16) as u16);
            let side = x.u8(0x4a) ^ 0x20;
            x.c.d[0] = x.c.d[0] & !0xff | side as u32;
            x.set8(f::FLIP, side);
            let mut v = (-0x300i16) as u16;
            setw(x, 0, v);
            if x.u8(f::FLIP) == 0 {
                v = v.wrapping_neg();
                setw(x, 0, v);
            }
            x.set16(0x14, v);
            x.move_xz();
            body(x, 0xa);
        }
        0x89660 => {
            if x.u8(f::KEY_FRAME) != 0 {
                x.set8(f::KEY_FRAME, 0);
                x.call(0xb85e2);
            }
            x.move_xz();
            if (x.u16(0xc) as i16) > 0 {
                x.anim_step();
                return;
            }
            x.add8(f::SUB2, 2);
            x.set16(0xc, 0);
            if x.u8(f::FLIP) != 0 {
                aids::burst_back(x);
            } else {
                aids::burst_front(x);
            }
            x.call(0xec9e8);
            body(x, 0xb);
        }
        0x896b6 => {
            if x.anim_step() {
                x.set16(TIMER, 0x96);
                x.add8(f::SUB2, 2);
                crate::mem::wr16(x.b, A5.wrapping_sub(0x557c), 0x800);
                let v = x.u8(0x7e);
                crate::mem::wr8(x.b, A5.wrapping_sub(0x557a), v);
            }
        }
        0x896d6 => {
            if sub16(x, TIMER, 1).mi() {
                x.add8(f::SUB2, 2);
                x.set16(TIMER, 0x3c);
            }
        }
        _ => {
            if sub16(x, TIMER, 1).mi() {
                x.set8(f::STATE, 6);
                return;
            }
            x.set_bit(0x1, 1, false);
            let b = x.u16(TIMER) & 1;
            setw(x, 0, b);
            if b != 0 {
                x.set_bit(0x1, 1, true);
            }
        }
    }
}
