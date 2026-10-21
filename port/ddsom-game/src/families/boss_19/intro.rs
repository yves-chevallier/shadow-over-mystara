//! Boss 14 / 19, groupe `+5` = 0 : cinématique d'arrivée (si la scène en a une, `A5+0x30`), propre à chaque
//! type et variante ; elle se termine par `end` qui passe au combat.
//!
//! | Cas | `+6` |
//! |---|---|
//! | type 14 (variante 0) | 0 attente, 2 et 4 entrée en vol, 6 descente, 8 atterrissage |
//! | type 19, variante 0 (scène 32) | 0 annonce (bulles), 2 bulles puis attente, 4 entrée par la droite, 6 vol vers l'écran, 8 attend la caméra |
//! | type 19, variante 1 (dragon de Nagpa) | 0 entrée par la droite, 2 vol, 4 attend la caméra |

use super::fight;
use super::*;

pub(super) fn run(x: &mut Cx) {
    if x.a5_u8(0x30) == 0 {
        return end(x);
    }
    x.set8(SHIELD, 2);
    if x.u8(TYPE) == TYPE_GREEN {
        if x.u8(VARIANT) != 0 {
            return end(x);
        }
        x.call(0xb3f6a);
        return match x.u8(f::SUB2) {
            0 => green_wait(x),
            2 => green_enter(x),
            4 => green_enter_b(x),
            6 => green_descend(x),
            8 => green_land(x),
            _ => end(x),
        };
    }
    if x.u8(VARIANT) == 1 {
        match x.u8(f::SUB2) {
            0 => pet_enter(x),
            2 => pet_fly(x),
            4 => pet_wait_camera(x),
            _ => end(x),
        }
    } else {
        match x.u8(f::SUB2) {
            0 => announce(x),
            2 => bubbles(x),
            4 => enter(x),
            6 => fly(x),
            8 => wait_camera(x),
            _ => end(x),
        }
    }
    x.reinsert();
}

/// 0x900e2 : fin de l'arrivée : le combat commence sur-le-champ.
fn end(x: &mut Cx) {
    x.add8(f::SUB, 2);
    x.set16(f::SUB2, 0);
    x.set8(0, 1);
    x.call(rom::IMMUNITIES);
    rest(x);
    fight::run(x);
}

fn camera(x: &mut Cx) -> (u16, u16) {
    if x.a5_u8(var::CAMERA_SELECT) != 0 {
        (x.a5_u16(0x556), x.a5_u16(0x55a))
    } else {
        (x.a5_u16(var::SCROLL_X), x.a5_u16(var::SCROLL_Y))
    }
}

// --- type 14 --------------------------------------------------------------------------------------

fn green_wait(x: &mut Cx) {
    x.add8(f::SUB2, 2);
    x.set_bit(0x1, 1, true);
    x.set16(TIMER, 0x80);
    x.c.d[0] = 0x7f;
    x.call(0x289c);
}

fn green_enter(x: &mut Cx) {
    if sub16(x, TIMER, 1).gt() {
        return;
    }
    x.set8(TIMER, 0);
    x.add8(f::SUB2, 2);
    x.set_bit(0x1, 1, false);
    green_enter_b(x);
}

fn green_enter_b(x: &mut Cx) {
    x.add8(f::SUB2, 2);
    let y = x.u16(0x10).wrapping_add(0x30);
    x.set16(0x10, y);
    x.set16(0xc, (-0x100i16) as u16);
    x.set8(f::FLIP, 0);
    x.set32(0x14, 0);
    x.set32(0x1c, 0);
    x.set16(0x18, 0x400);
    x.set16(0x1a, 0);
    set_anim(x, 0, 4);
    x.call(0xb3f40);
    x.call(0xe508a);
}

fn green_descend(x: &mut Cx) {
    x.call(call::MOVE_Z);
    if cmp16(x, 0xc, 0x60).ge() {
        x.add8(f::SUB2, 2);
        x.set16(0x18, 0x380);
        x.set16(0x1a, (-0x40i16) as u16);
        x.set16(0x1c, (-0x200i16) as u16);
        x.set16(0x1e, 0);
        x.set32(0x14, 0);
        x.call(0x1f0be);
        return green_land(x);
    }
    x.call(0xb3f6a);
    x.call(0x1f0be);
    x.call(call::REINSERT_FOREGROUND);
}

fn green_land(x: &mut Cx) {
    step(x);
    x.call(call::MOVE_XYZ);
    if !tst16(x, 0x18).pl() {
        x.add8(f::SUB2, 2);
        x.set32(0x14, 0);
        x.set32(0x18, 0);
        x.set32(0x1c, 0);
    }
    x.reinsert();
}

// --- type 19, variante 0 -----------------------------------------------------------------------------

/// Lance la tâche de fondu `list` (0x9b6 avec A0 = l'entrée, emplacement 0x10 ; `taches/fondu.rs`).
fn open_list(x: &mut Cx, list: u32) {
    let me = x.o.0;
    x.c.a[0] = list;
    set_dw(x, 0, 0x10);
    crate::call(x.c, x.b, 0x9b6);
    x.c.a[0] = me;
}

fn announce(x: &mut Cx) {
    x.add8(f::SUB2, 2);
    x.set_bit(0x1, 1, true);
    x.set16(TIMER, 0xb4);
    x.set_a5_u16(0x490, 3);
    x.set_a5_u8(0x48e, 0xc);
    x.set_a5_u8(0x48f, 9);
    x.set8(BUBBLES, 4);
    x.set8(BUBBLE_TICK, 0);
    open_list(x, 0x14de);
    x.call(0xe50fe);
}

/// 0x8fe68 : une bulle de plus toutes les 15 à 78 images.
fn bubble_tick(x: &mut Cx) {
    if (x.u8(BUBBLES) as i8) <= 0 {
        return;
    }
    if !sub8(x, BUBBLE_TICK, 1).gt() {
        x.set_a5_u16(0x4be, 6);
        x.call(0x1ac86);
        let r = (random(x) & 0x3f) as u8 + 0xf;
        x.set8(BUBBLE_TICK, r);
        sub8(x, BUBBLES, 1);
        x.call(0xead04);
    }
}

fn bubbles(x: &mut Cx) {
    bubble_tick(x);
    match x.u16(TIMER) as i16 {
        t if t < 0x64 => x.call(0xb4088),
        0x64 => {
            set_dw(x, 0, 0xa0);
            x.call(0x2860);
            x.call(0xe50fe);
            x.call(0xb4088);
        }
        _ => {}
    }
    if sub16(x, TIMER, 1).gt() {
        return;
    }
    x.add8(f::SUB2, 2);
    x.set_a5_u16(0x490, 3);
    x.set_a5_u8(0x48e, 0xe);
    x.set_a5_u8(0x48f, 9);
    x.set16(BUBBLES, 0);
    open_list(x, 0x14c8);
}

/// Position d'entrée à droite de la caméra, à la profondeur `dy` sous son bord haut.
fn enter_from_right(x: &mut Cx, dy: u16) {
    x.add8(f::SUB2, 2);
    x.set_bit(0x1, 1, false);
    let (cx, cy) = camera(x);
    x.set16(0x8, cx.wrapping_add(0x180));
    x.set16(0x10, cy.wrapping_add(dy));
    x.set16(0xc, 0x40);
    x.set16(0x14, (-0x200i16) as u16);
    x.set16(0x16, 0);
    x.set32(0x1c, 0);
    x.set16(0x18, (-0x100i16) as u16);
    x.set16(0x1a, 0);
}

fn enter(x: &mut Cx) {
    enter_from_right(x, 0x50);
    rest(x);
    fly(x);
}

/// Vol d'approche : la retombée est coupée sous l'altitude 0x20.
fn glide(x: &mut Cx) {
    x.call(call::MOVE_XYZ);
    if x.u16(0x18) != 0 && (x.u16(0xc) as i16) < 0x20 {
        x.set16(0x18, 0);
    }
}

fn fly(x: &mut Cx) {
    x.call(0xb4088);
    glide(x);
    let (cx, _) = camera(x);
    if Fl::sub16(cx.wrapping_add(0x130), x.u16(0x8)).le() {
        return finish_step(x);
    }
    x.add8(f::SUB2, 2);
    x.set_a5_u8(0x2e, 6);
    x.call(0xe50fe);
    x.set32(0x14, 0);
    x.set32(0x18, 0);
    x.set32(0x1c, 0);
    finish_step(x);
}

fn finish_step(x: &mut Cx) {
    step(x);
}

fn wait_camera(x: &mut Cx) {
    x.set8(HOVER, 1);
    step(x);
    if (x.a5_u8(0x2c) as i8) < 0xc {
        return;
    }
    x.add8(f::SUB2, 2);
}

// --- type 19, variante 1 -------------------------------------------------------------------------------

fn pet_enter(x: &mut Cx) {
    enter_from_right(x, 0x38);
    rest(x);
    pet_fly(x);
}

fn pet_fly(x: &mut Cx) {
    glide(x);
    let (cx, _) = camera(x);
    if Fl::sub16(cx.wrapping_add(0x128), x.u16(0x8)).le() {
        return finish_step(x);
    }
    x.add8(f::SUB2, 2);
    x.set16(TIMER, 0x100);
    x.set32(0x14, 0);
    x.set32(0x18, 0);
    x.set32(0x1c, 0);
    finish_step(x);
}

fn pet_wait_camera(x: &mut Cx) {
    x.set8(HOVER, 1);
    step(x);
    if (x.a5_u8(0x2c) as i8) < 0xc {
        return;
    }
    x.add8(f::SUB2, 2);
}
