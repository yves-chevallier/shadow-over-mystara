//! Boss 17, group `+5` = 0: cutscenes (arrival, death and end of stage, variants 1 and 2).
//!
//! | `+6` | Role |
//! |---|---|
//! | 0 | arrival of variant 0: the boss jumps into the screen from the left and lands |
//! | 2 | entrance from the right, then the combat begins |
//! | 4 | death: long ending cutscene (dialogues, rising camera, flight) |
//! | 6 | variant 1: arrival from below, charge then landing |
//! | 8 | variant 2: like 6, with camera 2 |

use crate::families::cx::Bench;
use crate::objects::Obj;
use super::decide;
use super::*;
use crate::objects::pools;
use crate::objects::Current;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => arrival(x),
        2 => entrance(x),
        4 => finale(x),
        6 => variant(x, false),
        8 => variant(x, true),
        _ => {}
    }
}

fn state7<R: Bench>(x: &mut Cx<'_, R>) -> u32 {
    x.u8(0x7) as u32 / 2
}

/// An effect (EFFCT) linked to the boss (`+0x42`): active byte, type, optional variant, orientation of the boss.
/// `retry`: insists as long as the pool is full.
fn effect<R: Bench>(x: &mut Cx<'_, R>, retry: bool, kind: u8, variant: Option<u8>, flip: bool) -> Option<Obj> {
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

/// Opens a dialogue bubble of type `kind` (0xc8312); its address goes to `+0x42`.
fn open_message<R: Bench>(x: &mut Cx<'_, R>, kind: u32) {
    for _ in 0..64 {
        if let Some(bubble) = crate::families::creators::scene_set(x.b, kind as u8) {
            x.set16(0x42, bubble.word());
            return;
        }
    }
}

/// Is the current dialogue (`+0x42`) finished (byte `+0x22` non-zero)?
fn message_done<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let m = x.linked(0x42);
    m.u8(x.b, 0x22) != 0
}

/// Sound `addr` (`jsr`).
fn fx<R: Bench>(x: &mut Cx<'_, R>, addr: u32) {
    lib::sound(x, addr);
}

// --- +6 = 0: arrival -----------------------------------------------------------------------------

fn arrival<R: Bench>(x: &mut Cx<'_, R>) {
    match state7(x) {
        0 => {
            x.add8(0x7, 2);
            x.set8(0, 2);
            x.set_bit(0x1, 1, false);
            x.set8(f::FLIP, 0x20);
            let cx = var::CAMERA1_X.get(x.b).wrapping_sub(0x10);
            x.set16(0x8, cx);
            let cy = var::CAMERA1_Y.get(x.b);
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
            lib::step(x);
        }
        2 => {
            if !sub16(x, TIMER, 1).gt() {
                x.add8(0x7, 2);
                x.set16(0x1e, (-0x20i16) as u16);
            }
            x.move_xy();
            lib::step(x);
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
            lib::step(x);
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
                        // `moveq #3,D5`
                        *x.c.d5() = 3;
                        decide::spawn_ring(x, 3);
                        fx(x, 0xe5ac2);
                    } else if cmp16(x, COUNT, 2).eq() {
                        fx(x, 0xe4e5a);
                    }
                }
            }
            x.move_xy();
            lib::step(x);
        }
        5 => {
            let low = var::CAMERA1_Y.get(x.b).wrapping_add(0x145);
            let left = var::CAMERA1_X.get(x.b).wrapping_add(0x260);
            if Fl::sub16(low, x.u16(0x10)).le() || Fl::sub16(left, x.u16(0x8)).le() {
                x.add8(0x7, 2);
                x.set_bit(0x1, 1, true);
            } else {
                x.move_xy();
                lib::step(x);
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

// --- +6 = 2: entrance from the right ---------------------------------------------------------------

fn entrance<R: Bench>(x: &mut Cx<'_, R>) {
    match state7(x) {
        0 => {
            x.add8(0x7, 2);
            fx(x, 0xecf10);
            let px = var::CAMERA1_X.get(x.b).wrapping_add(0xc0);
            let py = var::CAMERA1_Y.get(x.b).wrapping_add(0x68);
            x.set16(0x8, px);
            x.set16(0x10, py);
            x.set16(0xc, 0);
            x.set8(0, 2);
            x.set_bit(0x1, 1, false);
            x.set8(f::FLIP, 0);
            x.unlink(); // 0x1f0be
            body(x, 0x18);
        }
        1 => {
            lib::step(x);
            if x.u8(f::KEY_FRAME) != 0 {
                x.add8(0x7, 2);
            }
        }
        2 => {
            let b = x.u8(0x1) ^ 0x2;
            x.set8(0x1, b);
            if lib::step(x) {
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
                lib::set_immunities(x);
                lib::create_set_14(x);
                decide::chase(x);
            } else {
                lib::step(x);
            }
        }
        _ => {}
    }
}

// --- +6 = 4: death, ending cutscene -------------------------------------------------------------

fn finale<R: Bench>(x: &mut Cx<'_, R>) {
    match state7(x) {
        0 => {
            x.add8(0x7, 2);
            fx(x, 0xecf10);
            body(x, 0x17);
        }
        1 => {
            if lib::step(x) {
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
                let px = var::CAMERA1_X.get(x.b).wrapping_add(0xc0);
                let py = var::CAMERA1_Y.get(x.b).wrapping_add(0x80);
                x.set16(0x8, px);
                x.set16(0x10, py);
                x.set16(0xc, 0);
                fx(x, 0xecf10);
                body(x, 0x18);
            }
        }
        3 => {
            if lib::step(x) {
                x.add8(0x7, 2);
                x.set16(TIMER, 0xa);
                fx(x, 0xe1768);
                body(x, 0);
            }
        }
        4 => {
            lib::step(x);
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
            lib::step(x);
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
                lib::step(x);
            }
        }
        7 | 14 => {
            lib::step(x);
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
            if lib::step(x) {
                x.add8(0x7, 2);
                x.set16(TIMER, 0x96);
                body(x, 0);
            }
        }
        11 => {
            lib::step(x);
            if sub16(x, TIMER, 1).gt() {
                return;
            }
            x.add8(0x7, 2);
            fx(x, 0xe5baa);
            open_message(x, 0x14);
        }
        12 => {
            lib::step(x);
            if !message_done(x) {
                return;
            }
            x.add8(0x7, 2);
            fx(x, 0xea722);
            open_message(x, 0x15);
        }
        13 => {
            lib::step(x);
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

/// +7 = 32 (0x959cc): the camera rises by 4 per frame up to 0x1c8, then the boss drops a cloud of 4 effects around
/// a central effect.
fn camera_rise<R: Bench>(x: &mut Cx<'_, R>) {
    if tst16(x, TIMER).gt() && sub16(x, TIMER, 1).gt() {
        return;
    }
    let y = var::SCROLL_Y.get(x.b).wrapping_add(4);
    var::SCROLL_Y.set(x.b, y);
    if Fl::sub16(y, 0x1c8).lt() {
        return;
    }
    x.add8(0x7, 2);
    var::SCROLL_Y.set(x.b, 0x1c8);
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

/// +7 = 36 and 38 (0x95a8e, 0x95abc): the boss flies off, the camera rises faster than it does.
/// Y position of the main camera read as a long (16.16).
const SCROLL_Y_LONG: crate::ram::Long = crate::ram::Long(var::SCROLL_Y.0);

fn fall<R: Bench>(x: &mut Cx<'_, R>) {
    let first = x.u8(0x7) == 36;
    let rate = if first { 0x12000 } else { 0x11000 };
    let y = SCROLL_Y_LONG.get(x.b).wrapping_sub(rate);
    SCROLL_Y_LONG.set(x.b, y);
    x.move_xyz();
    let (limit, next) = if first { (0x80, 0x13) } else { (0xff80, 0x14) };
    if cmp16(x, 0x18, limit).le() {
        x.add8(0x7, 2);
        x.anim_start(SET_BODY, next);
    } else {
        x.anim_step();
    }
}

/// +7 = 40 (0x95aea): falls back to the ground.
fn land<R: Bench>(x: &mut Cx<'_, R>) {
    let y = SCROLL_Y_LONG.get(x.b).wrapping_sub(0x10000);
    SCROLL_Y_LONG.set(x.b, y);
    x.move_xyz();
    if tst16(x, 0xc).gt() {
        return;
    }
    x.add8(0x7, 2);
    x.set16(0xc, 0);
    fx(x, 0xec7ae);
    x.anim_start(SET_BODY, 0x15);
}

// --- +6 = 6 and 8: variants 1 and 2 ----------------------------------------------------------------

/// `second`: variant 2 (camera 2; its exit from the foreground tests the height alone).
fn variant<R: Bench>(x: &mut Cx<'_, R>, second: bool) {
    match state7(x) {
        0 => variant_start(x, second),
        1 => {
            if !sub16(x, TIMER, 1).gt() {
                x.add8(0x7, 2);
            }
            lib::step(x);
        }
        2 => {
            sub16(x, 0xc, 1);
            let low = if second { cmp16(x, 0xc, 0x50).le() } else { x.a5_u8(0x500) != 0 && cmp16(x, 0xc, 0x50).le() };
            if low {
                x.add8(0x7, 2);
                x.set16(TIMER, 0x14);
            }
            lib::step(x);
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
                lib::step(x);
            }
        }
        4 => {
            x.move_xyz();
            if tst16(x, 0x18).gt() {
                lib::step(x);
            } else {
                x.add8(0x7, 2);
                body(x, 7);
            }
        }
        5 => {
            x.move_xyz();
            if tst16(x, 0xc).gt() {
                lib::step(x);
            } else {
                x.add8(0x7, 2);
                x.set16(0xc, 0);
                body(x, 9);
            }
        }
        6 => {
            if lib::step(x) {
                x.add8(0x7, 2);
                let scene = var::SCENE.get(x.b);
                if scene == 0x2c || scene == 0x6f {
                    x.set_a5_u8(0x2e, 6);
                }
                body(x, 0);
            }
        }
        7 => {
            if var::PHASE.get(x.b) != 0xc {
                lib::step(x);
                return;
            }
            x.set_a5_u32(0x38, 0x0459_5900);
            for (off, v) in [(0x7e, 0x26), (0x7f, 0), (0x80, 0x1c), (0x81, 0x17)] {
                x.set8(off, v);
            }
            x.set8(0, 1);
            lib::set_immunities(x);
            lib::create_set_14(x);
            decide::chase(x);
        }
        _ => {}
    }
}

/// First step of variants 1 (0x95bf8) and 2 (0x95dd8): the boss falls from the top of the screen.
fn variant_start<R: Bench>(x: &mut Cx<'_, R>, second: bool) {
    x.add8(0x7, 2);
    x.set8(0, 2);
    x.set_bit(0x1, 1, false);
    x.set8(f::FLIP, 0);
    if second {
        let px = var::CAMERA2_X.get(x.b).wrapping_add(0x80);
        let py = var::CAMERA2_Y.get(x.b).wrapping_add(0x60);
        x.set16(0x8, px);
        x.set16(0x10, py);
        let h = var::CAMERA2_Y.get(x.b).wrapping_add(0x148).wrapping_sub(py);
        x.set16(0xc, h);
    } else {
        let px = var::SCROLL_X.get(x.b).wrapping_add(0x80);
        x.set16(0x8, px);
        x.set16(0x10, 0x170);
        let h = var::SCROLL_Y.get(x.b).wrapping_sub(0x170).wrapping_sub(0x30);
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
