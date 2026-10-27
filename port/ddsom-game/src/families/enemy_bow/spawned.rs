//! Type 3 créé par un autre objet (`+0x42` non nul) : il ne suit pas l'IA. Il surgit (`+5` = 0), puis tire en place
//! jusqu'à ce que sa durée de vie `+0xa8` s'écoule (`+0xa7` : saisi), après quoi il sort de l'écran à pied. Sans zone
//! ni ombre : seules les remises en liste des routines (`jmp 0x1ee1a`) sont rejouées.

use super::*;
use crate::objects::Courant;
use crate::objects::Obj;

pub(super) fn run(x: &mut Cx) {
    if x.u8(NO_ZONE) != 0 {
        return set_state_long(x, HELD);
    }
    x.c.d[0] = x.u8(f::SUB) as u32;
    match x.u8(f::SUB) {
        0 => begin(x),
        2 => active(x),
        _ => {}
    }
}

/// 0x5befc : surgit : points de vie à 1 (mots `+0x60`, `+0x62`, `+0x64`), marche en place.
fn begin(x: &mut Cx) {
    x.add8(f::SUB, 2);
    x.set16(0x60, 1);
    x.set16(f::HP, 1);
    x.set16(0x64, 1);
    set_speed(x, 0x18);
    x.set8(TIMER, 0x10);
    walk_angle(x);
    mv(x, 2);
}

/// 0x5bf40 : tire en place.
fn active(x: &mut Cx) {
    if x.u8(EXPIRED) != 0 || grabbed(x) {
        x.set16(f::STATE, 0x0408);
        x.set16(f::SUB2, 0);
        return hit::grab(x);
    }
    x.c.d[0] = x.u8(f::SUB2) as u32;
    match x.u8(f::SUB2) {
        0 => walk(x),
        2 => leave(x),
        4 => shoot(x),
        _ => {}
    }
}

/// +6 = 0 : marche sur place pendant `+0xa4` images, puis les tirs.
fn walk(x: &mut Cx) {
    x.move_by_table_xy();
    x.anim_step();
    let n = x.u8(TIMER).wrapping_sub(1);
    x.set8(TIMER, n);
    if (n as i8) < 0 {
        x.add8(f::SUB2, 4);
    }
    x.reinsert();
}

/// +6 = 2 : la durée de vie est écoulée : le Bow Gnoll s'éloigne de l'écran, puis disparaît.
fn leave(x: &mut Cx) {
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            let mut want = 0x20u8;
            x.set8(Obj::SPEED_INDEX, 8);
            let base = if x.a5_u8(0x36) != 0 { 0x55e } else { 0x50e };
            let d = x.u16(Obj::X).wrapping_sub(x.a5_u16(base));
            if (d as i16) <= 0xc0 {
                x.set8(Obj::SPEED_INDEX, 0x18);
                want = 0;
            }
            x.c.d[4] = x.c.d[4] & !0xff | want as u32;
            let anim = if want != x.u8(f::FLIP) {
                x.add8(f::SUB3, 2);
                3
            } else {
                x.set8(EXPIRED, 1);
                set_speed(x, 0x21);
                2
            };
            mv(x, anim);
            x.reinsert();
        }
        2 => {
            x.anim_step();
            x.move_by_table_xy();
            if x.in_arena_default() {
                return;
            }
            if (x.u8(TIMED) as i8) < 0 && x.u16(f::HOST) != 0 {
                let host = x.linked(f::HOST);
                host.set_u8(x.b, Obj::SPEED_INDEX, 1);
            }
            set_state_long(x, st::DEAD_LONG);
            detach_from_host(x);
        }
        4 => {
            if x.anim_step() {
                x.set8(EXPIRED, 1);
                x.set8(f::SUB3, 0);
                let flip = x.u8(f::FLIP) ^ 0x20;
                x.set8(f::FLIP, flip);
            }
            x.reinsert();
        }
        _ => {}
    }
}

/// +6 = 4 : tire (nombre de tirs tiré dans la rangée), reste dans la boucle de tirs.
fn shoot(x: &mut Cx) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        let n = row_byte(x, SHOT_REPEATS);
        x.set8(REPEAT, n);
        shoot_anim(x);
        return x.reinsert();
    }
    if x.anim_step() {
        let n = x.u8(REPEAT).wrapping_sub(1);
        x.set8(REPEAT, n);
        if n != 0 {
            shoot_anim(x);
            return x.reinsert();
        }
        x.add8(f::SUB2, 0xfe);
        x.set8(f::SUB3, 0);
    }
    if x.u8(f::KEY_FRAME) != 0 {
        x.set8(f::KEY_FRAME, 0);
        x.call(call::SWING);
    }
    x.reinsert();
}

/// 0x5c04a : animation de tir (0 ou 5 selon le profil `+0x2f`).
fn shoot_anim(x: &mut Cx) {
    let anim = if x.u8(GROUP) != 0 { 5 } else { 0 };
    x.c.d[0] = anim;
    shot(x, anim);
}
