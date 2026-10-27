//! Boss 7, état 4 « touché » (0x86baa). Groupes `+5` : 0 réaction (routine commune des ennemis choisie par le code de coup
//! `+0x49` dans la table de longs 0x86c3a), 2 projeté, 4 chute, 6 mort, 8 et 10 retour au combat.

use super::aids;
use super::*;
use crate::objects::Courant;
use crate::machine::Machine;

pub(super) fn run(x: &mut Cx) {
    x.set8(HIT_ACK, 0);
    if tst8(x, f::FREEZE).gt() {
        sub8(x, f::FREEZE, 1);
    } else {
        match dispatch(x, 0x86bda, f::SUB) {
            0x86be6 => reaction(x),
            0x86ce6 => thrown(x),
            0x86e10 => fall(x),
            0x86ed8 => death(x),
            _ => x.set32(f::STATE, FIGHT),
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

/// 0x86be6 : le coup est traité par la routine commune ; sans PV le boss meurt.
fn reaction(x: &mut Cx) {
    if x.u8(f::SUB2) == 0 {
        x.set_bit(0x1, 1, false);
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
    let routine = rom32(x, 0x86c3a + (x.c.d[0] & 0xffff));
    x.c.a[1] = routine;
    x.call(routine);
    if x.u8(f::FREEZE) != 0 && x.u8(0x46) != 0 {
        addq8(x, f::FREEZE, 1);
        x.set8(0x46, 0);
    }
}

/// 0x86ce6 : le boss est projeté en arrière, rebondit, puis se relève ou riposte.
fn thrown(x: &mut Cx) {
    if paused(x) {
        return;
    }
    match step(x, 0x86cfa) {
        0x86d00 => {
            x.add8(f::SUB2, 2);
            key(x, 0x1e);
            x.call(0x10112);
            x.set8(0, 2);
            x.set16(0x14, 0x500);
            x.set16(0x16, (-0x28i16) as u16);
            let side = x.u8(0x4a) ^ 0x20;
            setb(x, 0, side);
            x.set8(f::FLIP, side);
            if side != 0 {
                let (a, b) = (x.u16(0x14).wrapping_neg(), x.u16(0x16).wrapping_neg());
                x.set16(0x14, a);
                x.set16(0x16, b);
            }
            body(x, 9);
        }
        0x86d4c => {
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
            let mask = rom32(x, 0x86dd6 + x.c.d[5]);
            x.c.d[5] = mask;
            let alea = x.random();
            let i = alea & 0x1f;
            setw(x, 0, i);
            if mask >> i & 1 == 0 {
                x.set8(0, 1);
                x.set32(f::STATE, FIGHT);
                return;
            }
            x.set8(0x45, 5);
            x.set8(0x44, 0x44);
            x.set8(0, 1);
            x.add8(f::SUB2, 2);
            strike(x, 6);
        }
        _ => {
            if x.u8(KEY) != 0 {
                x.call(0xb4f1c);
                x.set8(KEY, 0);
            }
            if x.anim_step() {
                x.set32(f::STATE, FIGHT);
            }
        }
    }
}

/// 0x86e10 : chute, attente au sol, relevé (en s'écartant si un joueur est trop près).
fn fall(x: &mut Cx) {
    match step(x, 0x86e1c) {
        0x86e26 => {
            x.add8(f::SUB2, 2);
            x.call(0xfffc);
            x.set16(0xc, 0);
            body(x, 8);
        }
        0x86e42 => {
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
        0x86e66 => {
            if paused(x) || !x.anim_step() {
                return;
            }
            x.add8(f::SUB2, 2);
            aids::dodge_nearby(x);
            x.set16(TIMER, 8);
            x.set8(0, 2);
            x.set_bit(0x1, 1, true);
            body(x, 5);
        }
        0x86e9c => {
            if paused(x) {
                return;
            }
            if timer_done(x) {
                x.add8(f::SUB2, 2);
                x.set_bit(0x1, 1, false);
            }
        }
        _ => {
            if paused(x) || !x.anim_step() {
                return;
            }
            x.set8(VULNERABLE, 1);
            x.set32(f::STATE, FIGHT);
            x.set8(0, 1);
        }
    }
}

/// 0x86ed8 : mort (projection, atterrissage dans la poussière, fin de scène).
fn death(x: &mut Cx) {
    // `move.w (tbl,PC,D0w),D0w ; jmp (tbl,PC,D0w)` : ici le décalage remplace D0 (D1 n'est pas touché)
    let i = x.u8(f::SUB2) as u32;
    let off = x.rom16(0x86ee6 + i);
    x.c.d[0] = off as u32;
    match i {
        0 => {
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
            setb(x, 0, side);
            x.set8(f::FLIP, side);
            let mut v = (-0x300i16) as u16;
            if side == 0 {
                v = v.wrapping_neg();
            }
            setw(x, 0, v);
            x.set16(0x14, v);
            x.move_xz();
            body(x, 0xa);
        }
        2 => {
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
        4 => {
            if x.anim_step() {
                x.add8(f::SUB2, 2);
                crate::mem::wr16(x.b, crate::mem::A5.wrapping_sub(0x557c), 0x700);
                let v = x.u8(0x7e);
                crate::mem::wr8(x.b, crate::mem::A5.wrapping_sub(0x557a), v);
            }
        }
        6 => {}
        _ => {
            let target = 0x86ee6u32.wrapping_add(off as i16 as i32 as u32);
            x.call(target);
        }
    }
}
