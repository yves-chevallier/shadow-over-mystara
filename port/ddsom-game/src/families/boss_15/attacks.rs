//! --- common steps ----------------------------------------------------------------------------------------

use crate::families::cx::Bench;
use super::*;
use crate::mem::rd16;
use crate::objects::Obj;
use cps2::Cps2Bus;
use crate::objects::Current;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => attack_0(x),
        2 => attack_2(x),
        4 => attack_4(x),
        6 => claw(x, 4, 6),
        8 => pose(x, 6),
        10 => pose(x, 8),
        12 => attack_12(x),
        14 => attack_14(x),
        16 => claw(x, 5, -6),
        18 => pose(x, 7),
        _ => {}
    }
}

// 0x91cce: start of the rise.

/// 0x91cec: rises up to altitude 0x300.
fn rise_start<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x7, 2);
    x.set32(0x14, 0);
    x.set32(0x1c, 0);
    x.set16(0x18, 0x400);
    x.set16(0x1a, 0x40);
    set_anim(x, 2, 0);
}

/// 0x91d08, 0x91d6e: waits for the timer to end.
fn rise_up<R: Bench>(x: &mut Cx<'_, R>) {
    x.move_z();
    if (x.u16(0xc) as i16) < 0x300 {
        return;
    }
    x.set16(TIMER, 0xf0);
    x.add8(0x7, 2);
}

/// 0x91d30: start of the descent.
fn pause<R: Bench>(x: &mut Cx<'_, R>) {
    if sub16(x, TIMER, 1).gt() {
        return;
    }
    x.set16(TIMER, 0);
    x.add8(0x7, 2);
}

/// 0x91d4e: descends to altitude 0x20, then waits 0x20 frames.
fn fall_start<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x7, 2);
    x.set32(0x14, 0);
    x.set32(0x1c, 0);
    x.set16(0x18, (-0x400i16) as u16);
    x.set16(0x1a, (-0x40i16) as u16);
    set_anim(x, 2, 0);
}

/// The targeted player (`+0x50`) if there is one, otherwise the dragon waits (0x91748).
fn fall_down<R: Bench>(x: &mut Cx<'_, R>) {
    x.move_z();
    if (x.u16(0xc) as i16) > 0x20 {
        return;
    }
    x.set16(0xc, 0x20);
    x.add8(0x7, 2);
    x.set16(TIMER, 0x20);
    pause(x);
}

/// Reaches in quarters altitude 0x40 and the depth of the targeted player; moves on to the next step on arrival.
fn target<R: Bench>(x: &mut Cx<'_, R>) -> Option<Obj> {
    if x.u16(f::TARGET) == 0 {
        wait(x);
        return None;
    }
    let p = x.linked(f::TARGET);
    if p.u8(x.b, 0) == 0 {
        wait(x);
        return None;
    }
    Some(p)
}

/// 0x91e16: descends by 8 per frame down to altitude 0x20.
fn reach_player<R: Bench>(x: &mut Cx<'_, R>) {
    let Some(p) = target(x) else { return };
    let y = p.u16(x.b, 0x10) as i16;
    if fight_close(x, y) {
        x.add8(0x7, 2);
    }
}

fn fight_close<R: Bench>(x: &mut Cx<'_, R>, y: i16) -> bool {
    super::fight::close_in_zy(x, 0x40, y)
}

// --- +6 = 0 -----------------------------------------------------------------------------------------------

fn attack_0<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(SHIELD, 2);
    match x.u8(0x7) {
        0 => reach_player(x),
        2 => {
            x.add8(0x7, 2);
            lib::sound(x, crate::sound::CMD_38D);
            x.set16(TIMER, 0xa0);
            set_anim(x, 0, 2);
        }
        4 => {
            if !sub16(x, TIMER, 1).gt() {
                x.add8(0x7, 2);
                crate::palette::set_flash_color(x.b, crate::palette::FLASH_COLOR_WHITE_VALUE);
                x.set_a5_u16(0x4be, 8);
                return settle(x);
            }
            push_players(x);
            x.add16(0xc, 1);
            if x.u16(TIMER) & 3 == 0 {
                let z = x.u16(0xc).wrapping_sub(3);
                x.set16(0xc, z);
            }
            crate::families::effects_creators::scenery_6(x.b, x.o);
        }
        6 => settle(x),
        8 => {
            step(x);
            if x.u8(0x33) == 0 {
                return;
            }
            x.set8(0x33, 0);
            x.add8(0x7, 2);
            lib::sound(x, crate::sound::CMD_38D);
            eshl_shot_34(x, 0);
            lib::sound(x, crate::sound::CMD_317);
            x.set16(TIMER, 0x100);
            flap(x);
        }
        10 => flap(x),
        12 => {
            if step(x) {
                stand(x);
            }
        }
        _ => {}
    }
}

/// 0x91e62: flaps its wings: altitude and depth oscillate.
fn settle<R: Bench>(x: &mut Cx<'_, R>) {
    let z = x.u16(0xc).wrapping_sub(8);
    x.set16(0xc, z);
    if (z as i16) > 0x20 {
        return;
    }
    x.set16(0xc, 0x20);
    x.add8(0x7, 2);
    set_anim(x, 2, 1);
}

/// --- +6 = 2: breath across the screen ----------------------------------------------------------------------
fn flap<R: Bench>(x: &mut Cx<'_, R>) {
    step(x);
    if !sub16(x, TIMER, 1).gt() {
        x.add8(0x7, 2);
        return set_anim(x, 2, 2);
    }
    x.add16(0xc, 1);
    if x.u16(TIMER) & 3 == 0 {
        let z = x.u16(0xc).wrapping_sub(3);
        x.set16(0xc, z);
    }
    let mut d: i16 = 4;
    if x.u16(TIMER) & 1 != 0 {
        d = -d;
    }
    x.add16(0x10, d as u16);
}

// 0x91f36: pushes the players by 2 pixels on the side where the dragon enters while the timer counts down.

fn attack_2<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(SHIELD, 2);
    match x.u8(0x7) {
        0 => rise_start(x),
        2 => rise_up(x),
        4 => pause(x),
        6 => {
            x.add8(0x7, 2);
            x.set8(0x42, 1);
            x.set_a5_u16(0x51e, 4);
            let r = (x.random() as u32) as u8 & 0x20;
            x.set8(f::FLIP, r);
            let d1: i16 = if r != 0 { -0xe0 } else { 0x260 };
            let px = var::SCROLL_X.get(x.b).wrapping_add(d1 as u16);
            x.set16(0x8, px);
            x.set16(0xc, 0x30);
            let py = var::SCROLL_Y.get(x.b).wrapping_add(0x50);
            x.set16(0x10, py);
            x.set16(TIMER, 0x40);
            set_anim(x, 2, 0x13);
            lib::sound(x, crate::sound::CMD_38D);
            push_side(x);
        }
        8 => push_side(x),
        10 => align(x),
        12 => sweep(x),
        14 => sweep_end(x),
        16 => leave(x),
        18 => pause(x),
        20 => fall_start(x),
        22 => fall_down(x),
        24 => stand(x),
        _ => {}
    }
}

/// 0x91f6c: reaches the attack position (0x138 or 0x48 in front of the camera).
fn push_side<R: Bench>(x: &mut Cx<'_, R>) {
    let d: i16 = if x.u8(f::FLIP) != 0 { -2 } else { 2 };
    for k in 0..4u32 {
        let p = Obj(0xff85de + 0x200 * k);
        if p.u8(x.b, 0) != 0 {
            let nx = p.u16(x.b, 0x8).wrapping_add(d as u16);
            p.set_u16(x.b, 0x8, nx);
        }
    }
    crate::families::effects_creators::drift(x.b, x.o);
    let t = x.u16(TIMER).wrapping_sub(1);
    x.set16(TIMER, t);
    if (t as i16) >= 0 {
        return;
    }
    x.add8(0x7, 2);
    align(x);
}

/// 0x91fa6: breath: the depth follows the vector table pointed to by `+0x4c` (set by the animation script).
fn align<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(0x42, 1);
    let edge: u16 = if x.u8(f::FLIP) != 0 { 0x48 } else { 0x138 };
    let d = edge.wrapping_add(var::SCROLL_X.get(x.b)).wrapping_sub(x.u16(0x8)) as i16;
    if d != 0 {
        let mut s = d >> 4;
        if s == 0 {
            s = 1;
        }
        x.add16(0x8, s as u16);
        return;
    }
    x.add8(0x7, 2);
    set_anim(x, 2, 0x14);
    x.set32(0x4c, 0);
    sweep(x);
}

/// 0x92036: end of the breath, then starts the exit flight.
fn sweep<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(0x42, 1);
    let table = x.u32(0x4c);
    if table != 0 {
        let k = (x.u8(0x2d) as u32) << 2;
        let w = x.data16(table + 2 + k);
        let mut y = x.u32(0x10).wrapping_add((((w as u32) << 16) as i32 >> 8) as u32);
        x.set32(0x10, y);
        y >>= 16;
        let lo = var::SCROLL_Y.get(x.b).wrapping_add(0x20);
        let mut yw = y as u16;
        if !Fl::sub16(yw, lo).ge() {
            yw = lo;
        }
        if !Fl::sub16(yw, lo.wrapping_add(0x60)).le() {
            yw = lo.wrapping_add(0x60);
        }
        x.set16(0x10, yw);
        if x.a5_u16(0x6e) & 1 != 0 {
            let a = x.u8(0x2d).wrapping_add(1) & 0x1f;
            x.set8(0x2d, a);
        }
    }
    if step(x) {
        x.add8(0x7, 2);
        set_anim(x, 2, 0x22);
        return sweep_end(x);
    }
    if x.u8(0x33) == 0 {
        return;
    }
    x.set8(0x33, 0);
    eshl_shot_34(x, 1);
    lib::sound(x, crate::sound::CMD_317);
    x.set8(0x2d, 0);
    x.select_speed(&crate::objects::motion::SPEEDS_A, 0x18);
}

/// 0x92066: moves away until it leaves the screen, then repositions itself at the top right.
fn sweep_end<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(0x42, 1);
    if !step(x) {
        return;
    }
    x.add8(0x7, 2);
    let mut vx: i16 = 0x400;
    if x.u8(f::FLIP) != 0 {
        vx = -vx;
    }
    x.set16(0x14, vx as u16);
    x.set16(0x16, 0);
    x.set32(0x1c, 0);
    x.set32(0x18, 0);
    leave(x);
}

/// 0x92180: places the dragon above the screen, according to its type.
fn leave<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(0x42, 1);
    x.move_x();
    let d = x.u16(0x8).wrapping_sub(var::SCROLL_X.get(x.b)) as i16;
    if !(d < -0x80) && d < 0x200 {
        return;
    }
    x.add8(0x7, 2);
    x.set_a5_u16(0x51e, 0);
    x.set16(TIMER, 0x20);
    x.set16(0xc, 0x380);
    let px = var::SCROLL_X.get(x.b).wrapping_add(0xc0);
    x.set16(0x8, px);
    let py = var::SCROLL_Y.get(x.b).wrapping_add(0x7a);
    x.set16(0x10, py);
}

// --- +6 = 4 : plongeon ------------------------------------------------------------------------------------------

fn attack_4<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(SHIELD, 2);
    match x.u8(0x7) {
        0 => rise_start(x),
        2 => rise_up(x),
        4 => pause(x),
        6 => {
            x.add8(0x7, 2);
            x.set8(0x42, 1);
            x.set8(f::FLIP, 0);
            set_anim(x, 2, 0x15);
            x.set_a5_u8(0x48e, 4);
            crate::palette::or_brightness(x.b, 0xa000);
            dive_a(x);
        }
        8 => dive_a(x),
        10 => dive_b(x),
        12 => pause(x),
        14 => fall_start(x),
        16 => fall_down(x),
        18 => stand(x),
        _ => {}
    }
}

/// --- +6 = 6 and 16: claw swipe --------------------------------------------------------------------------
fn dive_position<R: Bench>(x: &mut Cx<'_, R>) {
    let big = x.u8(TYPE) == TYPE_BIG;
    let px = var::SCROLL_X.get(x.b).wrapping_add(if big { 0x96 } else { 0xc0 });
    x.set16(0x8, px);
    x.set16(0x10, 0x170);
    let z = var::SCROLL_Y.get(x.b).wrapping_sub(if big { 0xf0 } else { 0xd8 });
    x.set16(0xc, z);
}

fn dive_a<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(0x42, 1);
    x.set8(FOREGROUND, 1);
    dive_position(x);
    if step(x) {
        x.add8(0x7, 2);
        set_anim(x, 2, 0x16);
        crate::families::effects_creators::scenery_8(x.b, x.o);
        return dive_b(x);
    }
    if x.u8(0x33) == 0 {
        return;
    }
    x.set8(0x33, 0);
    eshl_shot_34(x, 2);
    lib::sound(x, crate::sound::CMD_317);
}

fn dive_b<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(0x42, 1);
    x.set8(FOREGROUND, 1);
    x.set_a5_u8(0x48e, 4);
    crate::palette::or_brightness(x.b, 0xf000);
    dive_position(x);
    if !step(x) {
        return;
    }
    x.add8(0x7, 2);
    x.set16(TIMER, 0x20);
    x.set16(0xc, 0x380);
}

// `boxes`: hit boxes (`+0xa2`); `dx`: movement per frame of the hit (6 to the right, -6 to the left).

/// 0xa7234 (large) or 0xa727c (small)
fn claw<R: Bench>(x: &mut Cx<'_, R>, boxes: u8, dx: i16) {
    match x.u8(0x7) {
        0 => reach_player(x),
        2 => {
            x.set8(HURT_BOX, boxes);
            x.add8(0x7, 2);
            x.set16(TIMER, 0x18);
        }
        4 => {
            x.set8(HURT_BOX, boxes);
            if sub16(x, TIMER, 1).gt() {
                return;
            }
            x.add8(0x7, 2);
            x.set16(TIMER, 0x20);
            claw_sweep(x, boxes, dx);
        }
        6 => claw_sweep(x, boxes, dx),
        _ => {}
    }
}

fn claw_sweep<R: Bench>(x: &mut Cx<'_, R>, boxes: u8, dx: i16) {
    x.set8(HURT_BOX, boxes);
    x.add16(0x8, dx as u16);
    if sub16(x, TIMER, 1).gt() {
        return;
    }
    x.set16(TIMER, 0);
    stand(x);
}

// --- +6 = 8, 10, 18 : pose -----------------------------------------------------------------------------------

fn pose<R: Bench>(x: &mut Cx<'_, R>, boxes: u8) {
    x.set8(HURT_BOX, boxes);
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            x.set16(TIMER, 0x100);
        }
        2 => {
            if sub16(x, TIMER, 1).gt() {
                return;
            }
            x.set16(TIMER, 0);
            stand(x);
        }
        _ => {}
    }
}

// --- +6 = 12 : rugissement -----------------------------------------------------------------------------------------

fn attack_12<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x7) {
        0 => rise_start(x),
        2 => rise_up(x),
        4 => {
            x.add8(0x7, 2);
            x.set16(TIMER, 0x50);
            lib::shake_5c(x, 0x10);
            // --- +6 = 14: dive onto a player -------------------------------------------------------------------------
            let big = x.u8(TYPE) == TYPE_BIG;
            super::super::small::roar_created(x.b, x.o, big);
        }
        6 => {
            if sub16(x, TIMER, 1).gt() {
                return;
            }
            x.add8(0x7, 2);
        }
        8 => pause(x),
        10 => fall_start(x),
        12 => fall_down(x),
        14 => stand(x),
        _ => {}
    }
}

// 0x9249e: a player within range (0xfc44) is grabbed; returns true.

fn attack_14<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            x.set16(0x18, 0x300);
            x.set16(0x1a, (-0x20i16) as u16);
            set_anim(x, 2, 3);
        }
        2 => {
            step(x);
            x.move_z();
            follow(x);
            if (x.u16(0xc) as i16) >= 0x80 {
                x.add8(0x7, 2);
                x.set16(TIMER, 8);
                return set_anim(x, 2, 0x26);
            }
            if (x.u16(0x18) as i16) >= 0x100 {
                return;
            }
            x.set16(0x18, 0x100);
            x.set16(0x1a, 0);
        }
        4 => {
            follow(x);
            step(x);
            if sub16(x, TIMER, 1).gt() {
                return;
            }
            x.add8(0x7, 2);
            x.set16(0x1c, (-0x200i16) as u16);
            x.set16(0x1e, 0);
            x.set16(0x18, (-0x1000i16) as u16);
            x.set16(0x1a, 0);
            x.set32(0x14, 0);
        }
        6 => {
            follow(x);
            if try_seize(x) {
                return;
            }
            x.move_y();
            x.move_z();
            if (x.u16(0xc) as i16) > 0 {
                let low = x.u16(0xc).wrapping_add(x.u16(0x10));
                let floor = var::SCROLL_Y.get(x.b).wrapping_add(0x60);
                if (low as i16) > floor as i16 {
                    return;
                }
            } else {
                x.set16(0xc, 0);
            }
            x.add8(0x7, 2);
            stand(x);
        }
        _ => {}
    }
}

/// 0x931ea: gets closer to the targeted player by 2 pixels per frame in X and Y (without overshooting).
fn try_seize<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let Some(player) = lib::attack_hits_a_player(x, 7) else { return false };
    lib::seize_player(x, player, 0);
    true
}

/// 0x930e2: pushes away players that are too close (less than 0x10 pixels in X + Y) along the dragon-player axis.
fn follow<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(f::TARGET) == 0 {
        return;
    }
    let p = x.linked(f::TARGET);
    let (tx, ty) = (p.u16(x.b, 0x8) as i16, p.u16(x.b, 0x10) as i16);
    for (off, t) in [(0x8, tx), (0x10, ty)] {
        let here = x.u16(off) as i16;
        if t == here {
            continue;
        }
        let mut n = if t > here { here.wrapping_add(2) } else { here.wrapping_sub(2) };
        if (t > here && n >= t) || (t < here && n <= t) {
            n = t;
        }
        x.set16(off, n as u16);
    }
}

/// 0x1386: direction from the player (D3, D4) to the dragon (D1, D2). Original glitch: the working registers
/// of 0x1386 (D2, D3, D4: Z gap, step, unit) stay for the following handlers (`dispatch::Leftovers`).
fn push_players<R: Bench>(x: &mut Cx<'_, R>) {
    for k in 0..4u32 {
        let p = Obj(0xff85de + 0x200 * k);
        if p.u8(x.b, 0) == 0 {
            continue;
        }
        if (p.u8(x.b, 3) >> 1) == 5 && p.u8(x.b, 0x6a) == 0x1a {
            continue;
        }
        let dx = (x.u16(0x8).wrapping_sub(p.u16(x.b, 0x8)) as i16).wrapping_abs();
        let dy = (x.u16(0x10).wrapping_sub(p.u16(x.b, 0x10)) as i16).wrapping_abs();
        if dx.wrapping_add(dy) <= 0x10 {
            continue;
        }
        // `add.l #0x8000,D4` (carry into the high word), then the unit in D4.w
        let (bx, bz) = (x.u16(0x8) as u32, x.u16(0x10) as u32);
        let (px, pz) = (p.u16(x.b, 0x8) as u32, p.u16(x.b, 0x10) as u32);
        let r = crate::aids::compute(x.b, bx, bz, px, pz);
        *x.c.d2() = r.d2;
        *x.c.d3() = r.d3;
        // Slope (in 1/256 pixel) of the `shove` steps: 32 directions, (dx, dy) in words.
        *x.c.d4() = pz.wrapping_add(0x8000) & 0xffff_0000 | r.unit as u32;
        step_toward(x.b, p, r.direction);
    }
}

/// 0x93158 then the addition to the position: moves `o` one step in direction `direction` (0x1386); returns the
/// step (dx, dy: longs, 8 fractional bits).
const SHOVE_STEPS: u32 = 0x1c6636;

/// Boss 15 / 20, group `+5` = 4: the attacks (`+6` = 0 to 18), each in steps `+7`. The rises (`rise`), descents
/// (`fall`) and waits (`pause`) are shared by several attacks.
///
/// | `+6` | Attack |
/// |---|---|
/// | 0 | reaches the player, flaps its wings (pushes them), lands |
/// | 2 | rises, enters from an edge of the screen, flies across and leaves (fire breath) |
/// | 4 | rises, dives while spinning and lands |
/// | 6, 16 | claw swipe to the right (6) or to the left (16) |
/// | 8, 10, 18 | pose of 0x100 frames (hit boxes 6, 8, 7) |
/// | 12 | rises, roars (0xa7234 / 0xa727c), descends |
/// | 14 | dives onto a player and grabs them |
fn step_toward(b: &mut Cps2Bus, o: Obj, direction: u8) -> (u32, u32) {
    let a = (direction as u16 & 0x1f) * 4;
    let dy = ((rd16(b, SHOVE_STEPS + 2 + a as u32) as i16 as i32) << 8) as u32;
    let dx = ((rd16(b, SHOVE_STEPS + a as u32) as i16 as i32) << 8) as u32;
    let nx = o.u32(b, 0x8).wrapping_add(dx);
    o.set_u32(b, 0x8, nx);
    let ny = o.u32(b, 0x10).wrapping_add(dy);
    o.set_u32(b, 0x10, ny);
    (dx, dy)
}

/// 0x930a4 (called by an EFFCT, 0xb482e, `effects_c::spark_active`): the current object steps toward `other` while it
/// is
/// more than 0x10 pixels away (X + Y), by 0x1386 and 0x93158. Original glitch: 0x1386 leaves its work registers (D2,
/// D3, D4) to the next handlers (`dispatch::Leftovers`); the X written to D1.w keeps a null high word (the caller's
/// D1).
pub(crate) fn approach_toward<R: Bench>(x: &mut Cx<'_, R>, other: Obj) {
    let me = x.o;
    let dx = (me.u16(x.b, 0x8).wrapping_sub(other.u16(x.b, 0x8)) as i16).wrapping_abs() as u16;
    let dy = (me.u16(x.b, 0x10).wrapping_sub(other.u16(x.b, 0x10)) as i16).wrapping_abs() as u16;
    if (dx.wrapping_add(dy) as i16) <= 0x10 {
        return;
    }
    let low = |r: u32, v: u16| r & 0xffff_0000 | v as u32;
    let d1 = other.u16(x.b, 0x8) as u32;
    let d2 = low(*x.c.d2(), other.u16(x.b, 0x10));
    let d3 = low(*x.c.d3(), me.u16(x.b, 0x8));
    let d4 = low(*x.c.d4(), me.u16(x.b, 0x10));
    let r = crate::aids::compute(x.b, d1, d2, d3, d4);
    *x.c.d2() = r.d2;
    *x.c.d3() = r.d3;
    // `add.l #0x8000,D4` (carry into the high word), then the unit in D4.w
    *x.c.d4() = d4.wrapping_add(0x8000) & 0xffff_0000 | r.unit as u32;
    step_toward(x.b, me, r.direction);
}

