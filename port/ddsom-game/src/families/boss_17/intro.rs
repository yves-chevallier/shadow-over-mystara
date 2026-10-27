//! Boss 17, groupe `+5` = 0 : cinématiques (arrivée, mort et fin de scène, variantes 1 et 2).
//!
//! | `+6` | Rôle |
//! |---|---|
//! | 0 | arrivée de la variante 0 : le boss saute dans l'écran par la gauche et retombe |
//! | 2 | entrée par la droite, puis le combat commence |
//! | 4 | mort : longue cinématique de fin (dialogues, caméra qui monte, envol) |
//! | 6 | variante 1 : arrivée par le bas, charge puis atterrissage |
//! | 8 | variante 2 : comme 6, avec la caméra 2 |

use crate::objects::Obj;
use super::decide;
use super::*;
use crate::objects::pools;
use crate::objects::Courant;

pub(super) fn run(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => arrival(x),
        2 => entrance(x),
        4 => finale(x),
        6 => variant(x, false),
        8 => variant(x, true),
        _ => {}
    }
}

fn state7(x: &mut Cx) -> u32 {
    x.u8(0x7) as u32 / 2
}

/// Un effet (EFFCT) lié au boss (`+0x42`) : octet actif, type, variante facultative, orientation du boss.
/// `retry` : insiste tant que le pool est plein.
fn effect(x: &mut Cx, retry: bool, kind: u8, variant: Option<u8>, flip: bool) -> Option<Obj> {
    let e = if retry { alloc_retry(x, &pools::EFFCT)? } else { x.alloc(&pools::EFFCT)? };
    e.set_u8(x.b, 0, 1);
    e.set_u8(x.b, 2, kind);
    if let Some(v) = variant {
        e.set_u8(x.b, 3, v);
    }
    if flip {
        let side = x.u8(f::FLIP);
        e.set_u8(x.b, f::FLIP, side);
    }
    e.set_u16(x.b, 0x42, x.o.0 as u16);
    Some(e)
}

/// Ouvre une bulle de dialogue de type `kind` (0xc8312) ; son adresse va en `+0x42`.
fn open_message(x: &mut Cx, kind: u32) {
    for _ in 0..64 {
        x.c.d[0] = kind;
        x.call(call::NEW_MESSAGE);
        if !carry(x) {
            let a = x.c.a[1] as u16;
            x.set16(0x42, a);
            return;
        }
    }
}

/// Le dialogue en cours (`+0x42`) est-il terminé (octet `+0x22` non nul) ?
fn message_done(x: &mut Cx) -> bool {
    let m = x.linked(0x42);
    m.u8(x.b, 0x22) != 0
}

fn fx(x: &mut Cx, addr: u32) {
    x.call(addr);
}

// --- +6 = 0 : arrivée -----------------------------------------------------------------------------

fn arrival(x: &mut Cx) {
    match state7(x) {
        0 => {
            x.add8(0x7, 2);
            x.set8(0, 2);
            x.set_bit(0x1, 1, false);
            x.set8(f::FLIP, 0x20);
            let cx = x.a5_u16(var::CAMERA1_X).wrapping_sub(0x10);
            x.set16(0x8, cx);
            let cy = x.a5_u16(var::CAMERA1_Y);
            x.set16(0x10, cy);
            x.set8(Obj::SPEED_INDEX, 8);
            speed(x, 0x12);
            x.set16(TIMER, 7);
            effect(x, true, 0x31, None, true);
            fx(x, 0xe4e5a);
            body(x, 0x16);
        }
        1 => {
            if !sub16(x, TIMER, 1).gt() {
                let a = x.u8(Obj::SPEED_INDEX).wrapping_sub(1);
                if (a as i8) < 0 {
                    x.add8(0x7, 2);
                    x.set16(TIMER, 0x46);
                } else {
                    x.set8(Obj::SPEED_INDEX, a);
                    speed(x, 0x12);
                    x.set16(TIMER, 7);
                }
            }
            x.move_xy();
            step(x);
        }
        2 => {
            if !sub16(x, TIMER, 1).gt() {
                x.add8(0x7, 2);
                x.set16(0x1e, (-0x20i16) as u16);
            }
            x.move_xy();
            step(x);
        }
        3 => {
            if !tst16(x, 0x1c).gt() {
                x.add8(0x7, 2);
                fx(x, 0xe4e5a);
                x.set16(TIMER, 0x30);
                x.set16(COUNT, 7);
                x.set16(0x1c, (-0x80i16) as u16);
                x.set16(0x1e, 0);
                x.set16(0x14, 0);
            }
            x.move_xy();
            step(x);
        }
        4 => {
            if !sub16(x, TIMER, 1).gt() {
                if sub16(x, COUNT, 1).le() {
                    x.add8(0x7, 2);
                    x.set16(0x18, 0);
                    fx(x, 0xe5baa);
                    x.set8(Obj::SPEED_INDEX, 5);
                    speed(x, 0x1d);
                } else {
                    x.set16(TIMER, 0x28);
                    let v = x.u16(0x1c).wrapping_neg();
                    x.set16(0x1c, v);
                    if cmp16(x, COUNT, 5).eq() {
                        decide::spawn_ring(x, 3);
                        fx(x, 0xe5ac2);
                    } else if cmp16(x, COUNT, 2).eq() {
                        fx(x, 0xe4e5a);
                    }
                }
            }
            x.move_xy();
            step(x);
        }
        5 => {
            let low = x.a5_u16(var::CAMERA1_Y).wrapping_add(0x145);
            let left = x.a5_u16(var::CAMERA1_X).wrapping_add(0x260);
            if Fl::sub16(low, x.u16(0x10)).le() || Fl::sub16(left, x.u16(0x8)).le() {
                x.add8(0x7, 2);
                x.set_bit(0x1, 1, true);
            } else {
                x.move_xy();
                step(x);
            }
        }
        6 => {
            if tst8(x, SPAWNED).le() {
                decide::start_appear(x);
            }
        }
        _ => {}
    }
}

// --- +6 = 2 : entrée par la droite ---------------------------------------------------------------

fn entrance(x: &mut Cx) {
    match state7(x) {
        0 => {
            x.add8(0x7, 2);
            fx(x, 0xecf10);
            let px = x.a5_u16(var::CAMERA1_X).wrapping_add(0xc0);
            let py = x.a5_u16(var::CAMERA1_Y).wrapping_add(0x68);
            x.set16(0x8, px);
            x.set16(0x10, py);
            x.set16(0xc, 0);
            x.set8(0, 2);
            x.set_bit(0x1, 1, false);
            x.set8(f::FLIP, 0);
            fx(x, 0x1f0be);
            body(x, 0x18);
        }
        1 => {
            step(x);
            if x.u8(f::KEY_FRAME) != 0 {
                x.add8(0x7, 2);
            }
        }
        2 => {
            let b = x.u8(0x1) ^ 0x2;
            x.set8(0x1, b);
            if step(x) {
                x.add8(0x7, 2);
                x.set_bit(0x1, 1, false);
                x.set16(TIMER, 0xa);
                body(x, 0);
            }
        }
        3 => {
            if sub16(x, TIMER, 1).le() {
                x.set_a5_u32(0x38, 0x0459_5900);
                for (off, v) in [(0x7e, 0x23), (0x7f, 0), (0x80, 0x1b), (0x81, 0x16)] {
                    x.set8(off, v);
                }
                x.set8(0, 1);
                x.call(rom::IMMUNITIES);
                x.call(0xd5f30);
                decide::chase(x);
            } else {
                step(x);
            }
        }
        _ => {}
    }
}

// --- +6 = 4 : mort, cinématique de fin -------------------------------------------------------------

fn finale(x: &mut Cx) {
    match state7(x) {
        0 => {
            x.add8(0x7, 2);
            fx(x, 0xecf10);
            body(x, 0x17);
        }
        1 => {
            if step(x) {
                x.add8(0x7, 2);
                x.set_bit(0x1, 1, true);
                x.set16(TIMER, 0xa);
            }
        }
        2 => {
            if sub16(x, TIMER, 1).le() {
                x.add8(0x7, 2);
                x.set_bit(0x1, 1, false);
                x.set8(f::FLIP, 0);
                let px = x.a5_u16(var::CAMERA1_X).wrapping_add(0xc0);
                let py = x.a5_u16(var::CAMERA1_Y).wrapping_add(0x80);
                x.set16(0x8, px);
                x.set16(0x10, py);
                x.set16(0xc, 0);
                fx(x, 0xecf10);
                body(x, 0x18);
            }
        }
        3 => {
            if step(x) {
                x.add8(0x7, 2);
                x.set16(TIMER, 0xa);
                fx(x, 0xe1768);
                body(x, 0);
            }
        }
        4 => {
            step(x);
            if tst16(x, TIMER).gt() {
                sub16(x, TIMER, 1);
                return;
            }
            if x.data8(0xff2a38) != 0xc {
                return;
            }
            x.add8(0x7, 2);
            fx(x, 0xea722);
            open_message(x, 0x13);
        }
        5 => {
            step(x);
            if !message_done(x) {
                return;
            }
            x.add8(0x7, 2);
            x.set16(TIMER, 0x3c);
            for d5 in (0..=3u8).rev() {
                effect(x, true, 0x10, Some(d5), true);
            }
            fx(x, 0xe5c1e);
        }
        6 => {
            if sub16(x, TIMER, 1).le() {
                x.add8(0x7, 2);
                strike(x, 0xd);
            } else {
                step(x);
            }
        }
        7 | 14 => {
            step(x);
            if x.u8(f::KEY_FRAME) != 0 {
                x.add8(0x7, 2);
                x.set16(TIMER, 0xa);
                x.set8(f::KEY_FRAME, 0);
            }
        }
        8 | 15 => {
            if sub16(x, TIMER, 1).gt() {
                return;
            }
            x.add8(0x7, 2);
            x.set16(TIMER, 0x18);
            if let Some(e) = effect(x, true, 0x27, Some(4), true) {
                let (px, py) = (x.u16(0x8), x.u16(0x10));
                e.set_u16(x.b, 0x8, px);
                e.set_u16(x.b, 0x10, py);
                e.set_u16(x.b, 0xc, 0x66);
            }
            fx(x, 0xe5c7a);
        }
        9 => {
            if sub16(x, TIMER, 1).le() {
                x.add8(0x7, 2);
                fx(x, 0xe5cd6);
                strike(x, 0xe);
            }
        }
        10 => {
            if step(x) {
                x.add8(0x7, 2);
                x.set16(TIMER, 0x96);
                body(x, 0);
            }
        }
        11 => {
            step(x);
            if sub16(x, TIMER, 1).gt() {
                return;
            }
            x.add8(0x7, 2);
            fx(x, 0xe5baa);
            open_message(x, 0x14);
        }
        12 => {
            step(x);
            if !message_done(x) {
                return;
            }
            x.add8(0x7, 2);
            fx(x, 0xea722);
            open_message(x, 0x15);
        }
        13 => {
            step(x);
            if message_done(x) {
                x.add8(0x7, 2);
                strike(x, 0xd);
            }
        }
        16 => camera_rise(x),
        17 => {
            let child = x.linked(CHILD);
            if child.u16(x.b, 4) != 0x020a {
                return;
            }
            x.add8(0x7, 2);
            x.set32(0x14, 0);
            x.set16(0x1e, 0);
            x.set16(0x1c, (-0x100i16) as u16);
            x.set16(0x18, 0x300);
            x.set16(0x1a, (-0x20i16) as u16);
            x.anim_start(SET_BODY, 0x12);
        }
        18 | 19 => fall(x),
        20 => land(x),
        21 => {
            if x.anim_step() {
                x.add8(0x7, 2);
            }
        }
        22 => {
            let child = x.linked(CHILD);
            if child.u16(x.b, 4) != 0x021e {
                return;
            }
            x.add8(0x7, 2);
            x.set8(f::FLIP, 0x20);
            fx(x, 0xea722);
            x.anim_start(SET_BODY, 0x11);
        }
        23 => {
            if x.anim_step() {
                x.add8(0x7, 2);
                x.set16(TIMER, 0x1e);
                x.anim_start(SET_BODY, 0);
            }
        }
        24 => {
            if !sub16(x, TIMER, 1).gt() {
                x.add8(0x7, 2);
                open_message(x, 0x16);
            }
            x.anim_step();
        }
        25 => {
            if message_done(x) {
                x.add8(0x7, 2);
                fx(x, 0xe5baa);
                body(x, 0x17);
            } else {
                x.anim_step();
            }
        }
        26 => {
            if x.anim_step() {
                x.add8(0x7, 2);
                x.set_bit(0x1, 1, true);
                set_abs8(x, 0xff2a38, 0x10);
            }
        }
        _ => {}
    }
}

/// +7 = 32 (0x959cc) : la caméra monte de 4 par image jusqu'à 0x1c8, puis le boss lâche un nuage de 4 effets autour d'un effet central.
fn camera_rise(x: &mut Cx) {
    if tst16(x, TIMER).gt() && sub16(x, TIMER, 1).gt() {
        return;
    }
    let y = x.a5_u16(var::SCROLL_Y).wrapping_add(4);
    x.set_a5_u16(var::SCROLL_Y, y);
    if Fl::sub16(y, 0x1c8).lt() {
        return;
    }
    x.add8(0x7, 2);
    x.set_a5_u16(var::SCROLL_Y, 0x1c8);
    let Some(core) = effect(x, true, 0x3b, None, false) else { return };
    let (px, py) = (x.u16(0x8), x.u16(0x10));
    core.set_u16(x.b, 0x8, px);
    core.set_u16(x.b, 0x10, py);
    core.set_u16(x.b, 0xc, 0xb0);
    x.set16(CHILD, core.0 as u16);
    for d5 in (0..=3u8).rev() {
        let Some(e) = alloc_retry(x, &pools::EFFCT) else { return };
        e.set_u8(x.b, 0, 1);
        e.set_u8(x.b, 2, 0x3c);
        e.set_u8(x.b, 3, d5);
        e.set_u16(x.b, 0x42, core.0 as u16);
        for off in [0x8, 0x10, 0xc] {
            let v = core.u16(x.b, off);
            e.set_u16(x.b, off, v);
        }
    }
}

/// +7 = 36 et 38 (0x95a8e, 0x95abc) : le boss s'envole, la caméra remonte plus vite que lui.
fn fall(x: &mut Cx) {
    let first = x.u8(0x7) == 36;
    let rate = if first { 0x12000 } else { 0x11000 };
    let y = x.a5_u32(var::SCROLL_Y).wrapping_sub(rate);
    x.set_a5_u32(var::SCROLL_Y, y);
    x.move_xyz();
    let (limit, next) = if first { (0x80, 0x13) } else { (0xff80, 0x14) };
    if cmp16(x, 0x18, limit).le() {
        x.add8(0x7, 2);
        x.anim_start(SET_BODY, next);
    } else {
        x.anim_step();
    }
}

/// +7 = 40 (0x95aea) : retombe vers le sol.
fn land(x: &mut Cx) {
    let y = x.a5_u32(var::SCROLL_Y).wrapping_sub(0x10000);
    x.set_a5_u32(var::SCROLL_Y, y);
    x.move_xyz();
    if tst16(x, 0xc).gt() {
        return;
    }
    x.add8(0x7, 2);
    x.set16(0xc, 0);
    fx(x, 0xec7ae);
    x.anim_start(SET_BODY, 0x15);
}

// --- +6 = 6 et 8 : variantes 1 et 2 ----------------------------------------------------------------

/// `second` : variante 2 (caméra 2 ; sa sortie de premier plan teste la hauteur seule).
fn variant(x: &mut Cx, second: bool) {
    match state7(x) {
        0 => variant_start(x, second),
        1 => {
            if !sub16(x, TIMER, 1).gt() {
                x.add8(0x7, 2);
            }
            step(x);
        }
        2 => {
            sub16(x, 0xc, 1);
            let low = if second { cmp16(x, 0xc, 0x50).le() } else { x.a5_u8(0x500) != 0 && cmp16(x, 0xc, 0x50).le() };
            if low {
                x.add8(0x7, 2);
                x.set16(TIMER, 0x14);
            }
            step(x);
        }
        3 => {
            if sub16(x, TIMER, 1).le() {
                x.add8(0x7, 2);
                x.set16(0x14, 0x200);
                x.set16(0x16, 0);
                x.set16(0x18, 0x300);
                x.set16(0x1a, (-0x20i16) as u16);
                x.set32(0x1c, 0);
                x.move_xyz();
                body(x, 4);
            } else {
                step(x);
            }
        }
        4 => {
            x.move_xyz();
            if tst16(x, 0x18).gt() {
                step(x);
            } else {
                x.add8(0x7, 2);
                body(x, 7);
            }
        }
        5 => {
            x.move_xyz();
            if tst16(x, 0xc).gt() {
                step(x);
            } else {
                x.add8(0x7, 2);
                x.set16(0xc, 0);
                body(x, 9);
            }
        }
        6 => {
            if step(x) {
                x.add8(0x7, 2);
                let scene = x.a5_u8(0x34);
                if scene == 0x2c || scene == 0x6f {
                    x.set_a5_u8(0x2e, 6);
                }
                body(x, 0);
            }
        }
        7 => {
            if x.a5_u8(0x2c) != 0xc {
                step(x);
                return;
            }
            x.set_a5_u32(0x38, 0x0459_5900);
            for (off, v) in [(0x7e, 0x26), (0x7f, 0), (0x80, 0x1c), (0x81, 0x17)] {
                x.set8(off, v);
            }
            x.set8(0, 1);
            x.call(rom::IMMUNITIES);
            x.call(0xd5f30);
            decide::chase(x);
        }
        _ => {}
    }
}

/// Première étape des variantes 1 (0x95bf8) et 2 (0x95dd8) : le boss tombe du haut de l'écran.
fn variant_start(x: &mut Cx, second: bool) {
    x.add8(0x7, 2);
    x.set8(0, 2);
    x.set_bit(0x1, 1, false);
    x.set8(f::FLIP, 0);
    if second {
        let px = x.a5_u16(var::CAMERA2_X).wrapping_add(0x80);
        let py = x.a5_u16(var::CAMERA2_Y).wrapping_add(0x60);
        x.set16(0x8, px);
        x.set16(0x10, py);
        let h = x.a5_u16(var::CAMERA2_Y).wrapping_add(0x148).wrapping_sub(py);
        x.set16(0xc, h);
    } else {
        let px = x.a5_u16(var::SCROLL_X).wrapping_add(0x80);
        x.set16(0x8, px);
        x.set16(0x10, 0x170);
        let h = x.a5_u16(var::SCROLL_Y).wrapping_sub(0x170).wrapping_sub(0x30);
        x.set16(0xc, h);
    }
    x.set16(TIMER, 0xa);
    effect(x, true, 0x31, Some(1), true);
    if second {
        fx(x, 0xe4e5a);
        fx(x, 0xe1430);
    } else if let Some(boss) = alloc_retry(x, &pools::BOSS) {
        boss.set_u8(x.b, 0, 2);
        boss.set_u8(x.b, 2, 0x15);
        let side = x.u8(f::FLIP);
        boss.set_u8(x.b, f::FLIP, side);
        boss.set_u16(x.b, f::HOST, x.o.0 as u16);
        x.set16(f::HOST, boss.0 as u16);
        fx(x, 0xe4e5a);
    }
    body(x, 0x19);
}
