//! Boss 6, groupe `+5` = 0 : arrivée. La Manticore entre par la droite en bondissant, retombe, rugit puis
//! attend que la caméra se fige avant de combattre.

use super::decide;
use super::*;
use crate::objects::Courant;

pub(super) fn run(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => enter(x),
        2 => rise(x),
        4 => peak(x),
        6 => land(x),
        8 => roar_wait(x),
        10 => settle(x),
        12 => stand_up(x),
        14 => wait_camera(x),
        _ => {}
    }
}

/// +6 = 0 : placée à droite de la caméra, elle part en bond vers la gauche.
fn enter(x: &mut Cx) {
    x.add8(f::SUB2, 2);
    x.set_bit(0x1, 1, false);
    let px = var::CAMERA1_X.get(x.b).wrapping_add(0x1e0);
    let py = var::CAMERA1_Y.get(x.b).wrapping_add(0x48);
    x.set16(0x8, px);
    x.set16(0x10, py);
    x.set16(0x14, (-0x500i16) as u16);
    x.set16(0x16, 0);
    x.set16(0x18, 0x400);
    x.set16(0x1a, (-0x40i16) as u16);
    x.move_xz();
    fly(x, 7);
}

/// +6 = 2 : montée du bond.
fn rise(x: &mut Cx) {
    lib::step(x);
    if x.u8(f::KEY_FRAME) != 0 {
        return;
    }
    x.move_xz();
    if cmp16(x, 0x18, 0x100).le() {
        x.add8(f::SUB2, 2);
        lib::sound(x, 0xe13f0);
        fly(x, 8);
    }
}

/// +6 = 4 : sommet du bond.
fn peak(x: &mut Cx) {
    x.move_xz();
    if cmp16(x, 0x18, 0xff00).le() {
        x.add8(f::SUB2, 2);
        fly(x, 9);
    }
}

/// +6 = 6 : chute ; à l'atterrissage, secousse et poussière.
fn land(x: &mut Cx) {
    x.move_xz();
    if tst16(x, 0xc).gt() {
        return;
    }
    x.add8(f::SUB2, 2);
    x.set16(0xc, 0);
    x.set16(TIMER, 0x12);
    x.set16(COUNT, 0);
    x.set16(0x16, 0x48);
    lib::shake_58(x, 0x1e);
    lib::sound(x, 0xecc0a);
    fly(x, 0xa);
}

/// +6 = 8 : glissade d'atterrissage, avec de la poussière toutes les 4 images.
fn roar_wait(x: &mut Cx) {
    if !sub16(x, COUNT, 1).gt() {
        x.set16(COUNT, 4);
        lib::debris(x, 3);
    }
    if sub16(x, TIMER, 1).le() {
        x.add8(f::SUB2, 2);
        x.set16(TIMER, 0x14);
        return;
    }
    x.move_x();
    lib::step(x);
}

/// +6 = 10 : pause avant de se relever.
fn settle(x: &mut Cx) {
    if sub16(x, TIMER, 1).le() {
        x.add8(f::SUB2, 2);
        lib::sound(x, 0xe3da6);
        body(x, 8);
    }
}

/// +6 = 12 : se relève.
fn stand_up(x: &mut Cx) {
    if lib::step(x) {
        x.add8(f::SUB2, 2);
        body(x, 0);
    }
}

/// +6 = 14 : attend que la caméra soit prête (`A5+0x2c` = 0xc), puis combat.
fn wait_camera(x: &mut Cx) {
    if var::PHASE.get(x.b) == 0xc {
        lib::create_set_14(x);
        decide::chase(x);
    } else {
        lib::step(x);
    }
}
