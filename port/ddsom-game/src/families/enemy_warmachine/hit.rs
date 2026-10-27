//! Type 27, état touché (`+4 = 4`, 0x789d0). `+5` : 0 réaction par table (ou début de la mort si les PV sont épuisés),
//! 2 recul, 4 chute et relevé (avec palier de PV), 6 mort en cinq temps, 8 fuite hors de l'écran, 10 assommé. Zone et
//! ombre sont testées sauf en fuite (`+5` = 8).

use super::*;
use crate::objects::pools;
use crate::objects::Courant;
use crate::objects::Obj;

/// Table de réactions communes aux coups, indexée par le code `+0x49`.
const REACT: u32 = 0x78a26;

pub(super) fn run(x: &mut Cx) {
    if (x.u8(f::FREEZE) as i8) > 0 {
        let n = x.u8(f::FREEZE).wrapping_sub(1);
        x.set8(f::FREEZE, n);
    } else {
        x.c.d[0] = x.u8(f::SUB) as u32;
        match x.u8(f::SUB) {
            0 => first(x),
            2 => hop_back(x),
            4 => get_up(x),
            6 => death(x),
            8 => run_off(x),
            10 => stunned(x),
            _ => {}
        }
        if x.u8(f::SUB) == 8 {
            return;
        }
    }
    zone_shadow(x);
}

/// 0x792e0 : PV épuisés : passe en mort (`+5` = 6) ; vrai dans ce cas.
fn dying(x: &mut Cx) -> bool {
    if (x.u16(f::HP) as i16) > 0 {
        x.c.d[0] = 0;
        return false;
    }
    x.set8(f::SUB, 6);
    x.set8(0, 2);
    x.c.d[0] = 1;
    true
}

fn first(x: &mut Cx) {
    if x.u8(f::SUB2) == 0 {
        x.set8(FLAG, 0);
        if dying(x) {
            return death(x);
        }
    }
    react(x, REACT);
}

/// 0x78b4a : recul vers l'arrière.
fn hop_back(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB2) as u32;
    match x.u8(f::SUB2) {
        0 => {
            x.set16(TIMER, 0x14);
            let (angle, v) = if x.u8(f::FLIP) != 0 { (0x18, 0x20i16) } else { (8, -0x20i16) };
            x.set8(Obj::SPEED_INDEX, angle);
            x.set16(Obj::AX, v as u16);
            set_speed(x, 0x17);
            look(x, 0x20);
            x.add8(f::SUB2, 2);
            lib::sound(x, 0xec5e6);
            mv(x, 0xb);
        }
        2 => {
            let t = x.u16(TIMER).wrapping_sub(1);
            x.set16(TIMER, t);
            if (t as i16) <= 0 {
                x.add8(f::SUB2, 2);
                x.set16(TIMER, 6);
                x.set16(Obj::AX, 0);
            }
            x.move_x();
            lib::step(x);
        }
        4 => {
            let t = x.u16(TIMER).wrapping_sub(1);
            x.set16(TIMER, t);
            if (t as i16) > 0 {
                x.move_x();
                lib::step(x);
                return;
            }
            x.c.d[0] = 0;
            x.set32(0x58, 0);
            go::attack(x, 0xa, true);
        }
        _ => {}
    }
}

/// 0x78be2 : chute puis relevé.
fn get_up(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB2) as u32;
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(LOOK_PREV, 0xff);
            x.anim_start(SET_MOVE, 0x12);
        }
        2 => {
            if (x.a5_u8(rom::PAUSE_FLAG) as i8) > 0 {
                return;
            }
            let n = x.u16(0x84).wrapping_sub(1);
            x.set16(0x84, n);
            if (n as i16) > 0 {
                return;
            }
            x.add8(f::SUB2, 2);
            x.set8(VULNERABLE, 1);
            x.set8(0x86, 0);
            x.set8(0x49, 0);
        }
        4 => {
            if paused(x) {
                return;
            }
            if x.anim_step() {
                recover(x);
            }
        }
        _ => {}
    }
}

/// 0x78c3c : relevé : choisit une cible, franchit un palier de PV, puis tir ou bond d'attaque.
pub(super) fn recover(x: &mut Cx) {
    x.set8(0, 1);
    x.set8(LOOK_PREV, 0xff);
    if !pick_target(x) {
        return go::idle(x);
    }
    let table = if x.a5_u8(0x48) > 5 { 0x109e2 } else { 0x108e2 };
    x.c.a[1] = table;
    lib::hp_threshold(x, table);
    if lib::target_in_box(x, 0, 0x38, 0xc) {
        return go::attack(x, 6, true);
    }
    go::fire_a(x);
}

/// 0x78c84 : mort en cinq temps : éjection, vol, retombée, tremblement, disparition.
fn death(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB2) as u32;
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(0, 2);
            x.set32(f::IMMUNITY, 0xffff_ffff);
            lib::sound(x, 0xe3cbe);
            lib::object_event(x);
            lib::drop_loot(x);
            lib::drop_special(x);
            lib::drop_treasure(x);
            detach_from_host(x);
            x.set16(Obj::AX, 0);
            x.set16(Obj::VZ, 0x500);
            x.set16(Obj::AZ, -0x38i16 as u16);
            let cam = camera_x(x).wrapping_add(0xc0);
            let (mut speed, mut flip) = (0x200i16, 0u8);
            if (cam as i16) < (x.u16(Obj::X) as i16) {
                speed = -speed;
                flip = 0x20;
            }
            x.set16(Obj::VX, speed as u16);
            x.set8(f::FLIP, flip);
            x.move_xz();
            x.anim_start(SET_MOVE, 0x13);
        }
        2 => {
            x.move_xz();
            if (x.u16(Obj::VZ) as i16) > 0 {
                lib::step(x);
                return;
            }
            x.add8(f::SUB2, 2);
            x.anim_start(SET_MOVE, 0x14);
        }
        4 => {
            x.move_xz();
            if (x.u16(Obj::Z) as i16) > 0 {
                return;
            }
            x.add8(f::SUB2, 2);
            x.set16(Obj::Z, 0);
            spark(x, 5);
            lib::sound(x, 0xec904);
            lib::step(x);
        }
        6 => {
            if x.anim_step() {
                x.add8(f::SUB2, 2);
                x.set16(TIMER, 0x3c);
            }
        }
        8 => {
            let t = x.u16(TIMER).wrapping_sub(1);
            x.set16(TIMER, t);
            if (t as i16) <= 0 {
                return x.free(&pools::ENEMY);
            }
            let flags = x.u8(1) ^ 2;
            x.set8(1, flags);
        }
        _ => {}
    }
}

/// 0x78d92 : saisie : la machine s'enfuit hors de l'écran puis disparaît.
fn run_off(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB2) as u32;
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set16(Obj::AX, 0);
            let angle = heading(x);
            x.set8(Obj::SPEED_INDEX, angle);
            set_speed(x, 0x21);
            spark(x, 1);
            lib::sound(x, 0xe3d32);
            look(x, 0x40);
            mv(x, 8);
        }
        2 => {
            x.move_x();
            lib::step(x);
            if x.u8(f::KEY_FRAME) == 2 {
                x.set8(f::KEY_FRAME, 0);
                spark(x, 1);
                lib::shake_54(x, 0x10);
                lib::sound(x, 0xe3d32);
            }
            if lib::outside(x, -0x30, 0x1b0) {
                detach_from_host(x);
                x.free(&pools::ENEMY);
            }
        }
        _ => {}
    }
}

/// 0x78e34 : assommé : animation 0xc, puis tir.
fn stunned(x: &mut Cx) {
    if x.u8(f::SUB2) == 0 {
        x.add8(f::SUB3, 2);
        return mv(x, 0xc);
    }
    if !lib::step(x) {
        return;
    }
    if grabbed(x) {
        return go::held(x);
    }
    if !lib::players_alive(x) {
        return go::idle(x);
    }
    recover(x);
}
