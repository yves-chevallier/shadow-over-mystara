//! Boss 11/12, état 4 « touché » (0x8e39a). Groupes `+5` : 0 réaction (table de longs 0x8e3dc : six routines selon le code de
//! coup `+0x49`), 4 étourdi, 2, 6, 8, 10 mort.

use super::*;
use crate::objects::Courant;
use crate::machine::Machine;

pub(super) fn run(x: &mut Cx) {
    if tst8(x, f::FREEZE).gt() {
        sub8(x, f::FREEZE, 1);
        return;
    }
    x.set8(0xb2, 0);
    match dispatch0(x, 0x8e3bc, f::SUB) {
        0x8e3c8 => reaction(x),
        0x8e922 => stunned(x),
        _ => death(x),
    }
}

/// 0x8e8ca : vrai si le jeu est en pause (le serpent reste alors affiché : `+0` = 2).
fn paused(x: &mut Cx) -> bool {
    if var::PAUSE.get(x.b) != 0 {
        x.set8(0, 2);
        return true;
    }
    false
}

/// 0x8e8de : fin de la réaction, retour à la poursuite.
fn recover(x: &mut Cx) {
    x.set8(0, 1);
    x.set8(0x86, 0);
    x.set8(0x87, 0);
    x.set8(REACTION, 0);
    x.set32(0x58, 0);
    x.set32(0x8a, 0);
    to_chase(x)
}

/// Plus de PV : le groupe 6 (mort) prend le relais.
fn dying(x: &mut Cx) {
    x.set8(f::SUB, 6);
    x.set16(f::SUB2, 0);
    death(x)
}

fn reaction(x: &mut Cx) {
    let code = x.u8(REACTION) as u32;
    x.c.d[0] = code;
    let routine = rom32(x, 0x8e3dc + code);
    match routine {
        0x8e488 => {
            if var::PAUSE.get(x.b) != 0 {
                return;
            }
            x.set16(f::HP, 0);
            x.set8(f::SUB, 6);
            x.set16(f::SUB2, 0);
            death(x)
        }
        0x8e4a2 => hit_a(x),
        0x8e574 => hit_b(x),
        0x8e5da => generic(x),
        0x8e69e => stun(x),
        _ => throw(x),
    }
}

/// 0x8e4a2 : recul (codes 0, 0x1c, 0x38, 0x3c, 0x6c, 0x98).
fn hit_a(x: &mut Cx) {
    if x.u8(f::SUB2) == 0 {
        if (x.u16(f::HP) as i16) <= 0 {
            return dying(x);
        }
        if Fl::sub16(x.u16(0xc), 0x10).gt() {
            x.set8(REACTION, 4);
            return generic(x);
        }
        x.set16(0xc, 0);
        x.add8(f::SUB2, 2);
        lib::sound(x, 0xe4bb6);
        if x.u8(REACTION) == 0x78 {
            lib::attack_effect(x, 1, 0);
        }
        paused(x);
        let stun = x.u8(STUN);
        x.set8(f::FREEZE, stun);
        x.set8(0x90, 0x10);
        stop(x);
        let (mut d0, mut d1) = (0x400u16, (-0x48i16) as u16);
        if x.u8(f::FLIP) != 0 {
            d0 = d0.wrapping_neg();
            d1 = d1.wrapping_neg();
        }
        x.set16(0x14, d0);
        x.set16(0x16, d1);
        return hurt(x, 0);
    }
    if paused(x) {
        return;
    }
    x.set8(0, 1);
    x.move_x();
    let mut d0 = x.u16(0x14);
    if x.u8(f::FLIP) != 0 {
        d0 = d0.wrapping_neg();
    }
    if (d0 as i16) < 0 {
        x.set32(0x14, 0);
    }
    if !x.anim_step() {
        return sync(x);
    }
    if x.u8(0x87) == 0 {
        return recover(x);
    }
    x.set8(REACTION, 0x68);
    x.set8(f::SUB2, 0);
    hit_b(x)
}

/// 0x8e574 : recul répété `+0x87` fois (code 0x68).
fn hit_b(x: &mut Cx) {
    if x.u8(f::SUB2) != 0 {
        if !sub8(x, 0x87, 1).eq() {
            x.anim_step();
            return sync(x);
        }
        return recover(x);
    }
    if (x.u16(f::HP) as i16) <= 0 {
        return dying(x);
    }
    if Fl::sub16(x.u16(0xc), 0x10).gt() {
        x.set8(REACTION, 4);
        return generic(x);
    }
    x.set16(0xc, 0);
    x.add8(f::SUB2, 2);
    paused(x);
    let stun = x.u8(STUN);
    x.set8(f::FREEZE, stun);
    stop(x);
    body(x, 2);
    x.c.d[0] = 3;
    x.c.d[1] = 0;
    x.call(0xabec8)
}

/// 0x8e5da : projection (codes 4 à 0x94 sans routine propre).
fn generic(x: &mut Cx) {
    match dispatch0(x, 0x8e5e8, f::SUB2) {
        0x8e5ee => {
            if (x.u16(f::HP) as i16) <= 0 {
                return dying(x);
            }
            x.add8(f::SUB2, 2);
            lib::sound(x, 0xe4bb6);
            x.set8(0x87, 0);
            paused(x);
            let stun = x.u8(STUN);
            x.set8(f::FREEZE, stun);
            x.set8(0x90, 0x10);
            stop(x);
            let mut d0 = 0x500u16;
            if x.u8(f::FLIP) != 0 {
                d0 = d0.wrapping_neg();
            }
            x.set16(0x14, d0);
            x.set16(0x18, 0x600);
            x.set16(0x1a, (-0x80i16) as u16);
            hurt(x, 1)
        }
        0x8e646 => {
            if paused(x) {
                return;
            }
            x.set8(0, 1);
            x.anim_step();
            sync(x);
            x.move_xz();
            if tst16(x, 0xc).gt() {
                return;
            }
            x.set16(0xc, 0);
            x.add8(f::SUB2, 2);
            stop(x);
            lib::shake_58(x, 8);
            lib::sound(x, 0xec892);
            hurt(x, 2)
        }
        _ => {
            if x.anim_step() {
                return fallen(x);
            }
            sync(x);
        }
    }
}

/// 0x8e8fc : relevé après la chute (état 4, groupe 4).
fn fallen(x: &mut Cx) {
    x.set8(0, 1);
    x.set32(f::STATE, 0x0404_0000);
    x.set8(0x86, 1);
    x.set8(0x87, 0);
    x.set8(REACTION, 0);
    x.set32(0x8a, 0);
    lib::draw_84(x)
}

/// 0x8e69e : étourdissement (code 0x4c).
fn stun(x: &mut Cx) {
    match dispatch0(x, 0x8e6ac, f::SUB2) {
        0x8e6b0 => {
            if (x.u16(f::HP) as i16) <= 0 {
                return dying(x);
            }
            if !sub8(x, 0xb8, 1).gt() {
                x.set8(0, 1);
                x.c.d[0] = 0;
                x.set8(0x86, 0);
                x.set8(REACTION, 0);
                x.set32(0x8a, 0);
                x.set8(SLOW, 8);
                return to_attack_7(x);
            }
            x.add8(f::SUB2, 2);
            let v = x.u16(0x84).wrapping_sub(8);
            x.set16(0x84, v);
            lib::sound(x, 0xe4bb6);
            paused(x);
            let stun = x.u8(STUN);
            x.set8(f::FREEZE, stun);
            let px = x.u16(0x8);
            x.set16(0xa, px);
            x.set8(0x90, 0x10);
            stop(x);
            hurt(x, 2);
            x.set8(0, 2);
        }
        _ => {
            x.c.d[0] = 2;
            let d1 = rd16(x.b, x.o.0.wrapping_sub(0x7f92)) & 1;
            let mut d = 2u16;
            if d1 != 0 {
                d = d.wrapping_neg();
            }
            setw(x, 0, d);
            x.add16(0x8, d);
            if x.anim_step() {
                let px = x.u16(0xa);
                x.set16(0x8, px);
                x.set8(0, 1);
                let mut d0 = 0x0404_0000u32;
                d0 = d0 & !0xff | x.u8(0x7) as u32;
                x.c.d[0] = d0;
                x.set32(f::STATE, d0);
                x.set8(0x86, 1);
                x.c.d[0] = 0;
                x.set8(REACTION, 0);
                x.set32(0x58, 0);
                x.set32(0x8a, 0);
                return;
            }
            sync(x);
        }
    }
}

/// 0x8e740 : projection en arc (code 0xa4).
fn throw(x: &mut Cx) {
    match dispatch(x, 0x8e74e, f::SUB2) {
        0x8e756 => {
            x.set8(0x67, 0);
            x.set8(0x86, 0);
            x.add8(f::SUB2, 2);
            x.c.d[0] = 0;
            x.set32(0x14, 0);
            x.set32(0x18, 0);
            x.set32(0x1c, 0);
            x.set8(0x87, 0);
            x.set8(0x86, 0);
            x.set8(0x90, 2);
            if x.c.d[7] & 1 != 0 {
                x.set8(0x90, 1);
            }
            let mut d2 = if x.u8(f::FLIP) != 0 { 0x18 } else { 8 };
            x.random_68k();
            d2 &= 0xf;
            let d0 = ((x.c.d[0] as u16).wrapping_add(d2 as u16)) & 0x1f;
            setw(x, 0, d0);
            x.set8(0x2d, d0 as u8);
            x.c.d[0] = 1;
            hurt(x, 1)
        }
        0x8e7b4 => arc(x),
        0x8e846 => {
            x.anim_step();
            sync(x);
            x.move_xyz();
            if tst16(x, 0xc).gt() {
                return arc_end(x);
            }
            x.add8(f::SUB2, 2);
            x.set32(0xc, 0);
            x.set32(0x18, 0);
            lib::sound(x, 0xec892);
            if (x.u16(f::HP) as i16) > 0 {
                return arc_end(x);
            }
            x.set8(f::SUB, 6);
            x.set16(f::SUB2, 0);
            death(x)
        }
        _ => fallen(x),
    }
}

fn arc_end(x: &mut Cx) {
    if x.u8(0x29) == 0 {
        return;
    }
    x.set16(0x14, 0);
    x.set16(0x16, 0);
}

/// 0x8e7b4 : la tête décrit un arc (vecteurs de la table 0x1c94b6).
fn arc(x: &mut Cx) {
    let a4 = 0x1c94b6;
    let z = x.u32(0xc).wrapping_add(0x20000);
    x.set32(0xc, z);
    if !Fl::sub16(x.u16(0xc), 0x60).lt() {
        x.add8(f::SUB2, 2);
        x.set16(0x18, 0x200);
        x.set16(0x1a, (-0x80i16) as u16);
        return;
    }
    if !sub8(x, 0x90, 1).eq() {
    } else {
        x.set8(0x90, 1);
        let a = x.u8(0x2d).wrapping_add(2) & 0x1f;
        x.set8(0x2d, a);
        x.set8(f::FLIP, 0);
        let d0 = a.wrapping_sub(8);
        setb(x, 0, d0);
        if d0 >= 0x10 {
            x.set8(f::FLIP, 0x20);
        }
    }
    x.c.d[0] = x.u8(0x2d) as u32;
    let i = (x.c.d[0] as u16) << 2;
    let vx = x.data16(a4 + i as u32).wrapping_add(x.u16(0x16));
    x.set16(0x14, vx);
    let step = (((vx as u32) << 16) as i32 >> 8) as u32;
    x.add32(0x8, step);
    let vy = x.data16(a4 + 2 + i as u32).wrapping_add(x.u16(0x1e));
    x.set16(0x1c, vy);
    let step = (((vy as u32) << 16) as i32 >> 8) as u32;
    x.add32(0x10, step);
}

/// 0x8e922 (`+5` = 4) : au sol, étourdi.
fn stunned(x: &mut Cx) {
    if paused(x) {
        return;
    }
    x.set8(0, 1);
    match dispatch0(x, 0x8e93c, f::SUB2) {
        0x8e942 => {
            x.add8(f::SUB2, 2);
            x.set16(0xc, 0);
            wait_init(x);
            hurt(x, 3);
            sync(x);
        }
        0x8e988 => {
            if var::PAUSE.get(x.b) != 0 {
                return;
            }
            if sub16(x, 0x84, 1).gt() {
                x.anim_step();
                return;
            }
            x.add8(f::SUB2, 2);
            x.set8(0x86, 0);
            x.set8(REACTION, 0);
            body(x, 4)
        }
        _ => {
            if !x.anim_step() {
                return;
            }
            x.set8(0, 1);
            x.set8(0x86, 0);
            x.set8(REACTION, 0);
            x.set32(0x58, 0);
            x.set32(0x8a, 0);
            to_attack_3(x)
        }
    }
}

/// 0x8e95a : durée de l'étourdissement (`+0xb8`, selon la difficulté), posée à la première image.
fn wait_init(x: &mut Cx) {
    if x.u8(f::SUB3) != 0 {
        return;
    }
    x.add8(f::SUB3, 2);
    let level = x.a5_u8(0x7f92) >> 1;
    setb(x, 0, level);
    let v = x.rom8(0x8e978 + level as u32);
    setb(x, 0, v);
    x.set8(0xb8, v);
}

/// 0x8e9d2 : mort (chute, effets, attente de la fin de l'animation).
fn death(x: &mut Cx) {
    match dispatch0(x, 0x8e9e0, f::SUB2) {
        0x8e9e6 => {
            x.add8(f::SUB2, 2);
            x.set8(0, 2);
            x.set_a5_u8(0x42, 1);
            x.set_a5_u8(0x50, 0);
            let kind = x.u8(2);
            wr8(x.b, A5.wrapping_sub(0x557c), kind);
            wr8(x.b, A5.wrapping_sub(0x557b), 0);
            let v = x.u8(0x7e);
            wr8(x.b, A5.wrapping_sub(0x557a), v);
            let mut d0 = 0xb00u16;
            if x.u8(2) != 0xb {
                d0 = 0xc00;
            }
            setw(x, 0, d0);
            wr16(x.b, A5.wrapping_sub(0x557c), d0);
            x.set16(TIMER, 0x10);
            lib::drop_rewards(x);
            crate::sound::service(x.b, crate::sound::Service::Ff05);
            lib::sound(x, 0xe3b6c);
            lib::flash_and_shake(x);
            let mut d0 = 0x500u16;
            setw(x, 0, d0);
            if x.u8(f::FLIP) != 0 {
                d0 = d0.wrapping_neg();
                setw(x, 0, d0);
            }
            x.set16(0x14, d0);
            x.set16(0x18, 0x600);
            x.set16(0x1a, (-0x80i16) as u16);
            hurt(x, 1);
            lib::sound(x, 0xe4c20)
        }
        0x8ea6e => {
            x.anim_step();
            sync(x);
            x.move_xz();
            if tst16(x, 0xc).gt() {
                return;
            }
            x.set16(0xc, 0);
            x.add8(f::SUB2, 2);
            stop(x);
            hurt(x, 3);
            lib::shake_58(x, 8)
        }
        _ => {
            x.anim_step();
            sync(x);
        }
    }
}
