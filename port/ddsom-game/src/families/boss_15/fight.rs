//! Boss 15 / 20, group `+5` = 2: the combat. The dragon stays around the center of the screen (`steer`), turns
//! around or moves away when it is too far, and chooses an attack (`+5` = 4) according to the distance of the
//! targeted player and its health.
//!
//! | `+6` | Role |
//! |---|---|
//! | 0 | watches (draws, range probes) |
//! | 2 | leap toward a computed point (jump speeds drawn from a distance) |
//! | 4 | reaches the targeted player |

use crate::families::cx::Bench;
use super::*;
use crate::objects::Obj;
use crate::objects::Current;

/// Attack choice bytes (4 rows of 16: 0x91a28, 0x91a38, 0x91a48, 0x91a58) and patience threshold.
const PICKS: [u32; 4] = [0x91a28, 0x91a38, 0x91a48, 0x91a58];
const COOLDOWNS: u32 = 0x91a18;
/// Bit table (one long per quarter of HP) that decides on a shot of 0x43ae.
const FAR_MASKS: u32 = 0x91998;
/// Gestures triggered by the special counterattack (`+6` of attack 4).
const SPECIAL_MOVES: u32 = 0x91ab0;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => watch(x),
        2 => jump(x),
        4 => approach(x),
        _ => {}
    }
    if var::PLAYERS_ALIVE.get(x.b) == 0 {
        wait(x);
    }
}

// --- +6 = 0 : surveille ---------------------------------------------------------------------------------

fn watch<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(HURT_BOX, 0);
    x.set8(SHOT, 0x20);
    if x.u8(0x7) == 0 {
        x.add8(0x7, 2);
        x.set8(VULNERABLE, 1);
        speed(x, 0x11);
        lib::draw_stance(x);
        return rest(x);
    }
    x.anim_step();
    if far_from_center(x) {
        return wander(x);
    }
    if !sub8(x, WALK_DELAY, 1).gt() {
        lib::draw_stance(x);
        if !sub8(x, WALK_STEPS, 1).pl() {
            lib::draw_6b(x);
        }
    }
    if special_due(x) {
        return;
    }
    if probe(x, 0, 0x20, 0x100) {
        return pick_attack(x, 0);
    }
    if far_roll(x) {
        if probe(x, 0, 0x50, 0x100) {
            return pick_attack(x, 1);
        }
        if probe(x, 0x88, 0x38, 0x100) {
            return pick_attack(x, 2);
        }
        if probe(x, -0x88, 0x38, 0x100) {
            return pick_attack(x, 3);
        }
    }
    steer(x);
}

/// 0x91ab8: the dragon is more than 0x80 pixels from the center of the area (camera + 0xc0).
fn far_from_center<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let d = var::SCROLL_X.get(x.b).wrapping_add(0xc0).wrapping_sub(x.u16(0x8)) as i16;
    d.wrapping_abs() >= 0x80
}

/// Is a player in the box `(d0, d1, d2)` in front of the dragon?
fn probe<R: Bench>(x: &mut Cx<'_, R>, d0: i16, d1: u16, d2: u16) -> bool {
    lib::target_in_box(x, d0, d1, d2)
}

/// 0x919b0: number (0 to 3) of the quarter of HP remaining, from `+0x62` relative to `+0x60`.
fn quarter<R: Bench>(x: &mut Cx<'_, R>) -> u32 {
    let d1 = x.u16(0x60) as u32;
    let d3 = (x.u16(0x62) as u32) << 2;
    let (mut d0, mut d2) = (-1i32, 0u32);
    // the original loops forever if `+0x60` is zero
    for _ in 0..0x20000 {
        d2 = d2.wrapping_add(d1);
        d0 += 1;
        if (d2 as i32) >= (d3 as i32) {
            break;
        }
    }
    d0 as u16 as u32
}

/// 0x91950: the dragon is at its guard altitude (0x40), its patience is exhausted and a favorable draw (one bit
/// of the HP long 0x91998) lets it aim at a distant player (0x43ae).
fn far_roll<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if x.u16(0xc) != 0x40 {
        return false;
    }
    if (x.u8(COOLDOWN) as i8) > 0 {
        sub8(x, COOLDOWN, 1);
        return false;
    }
    let q = quarter(x);
    let mask = x.prog32(FAR_MASKS + (((q as u16) << 2) as u32));
    let bit = (x.random() as u32) & 0x1f;
    if mask >> bit & 1 == 0 {
        return false;
    }
    lib::pick_target_any(x)
}

/// 0x91a68: is a special counterattack due? The HP drop below `SPECIALS` quarters of `+0x60`: the dragon launches
/// attack 4 (gesture drawn from 0x91ab0).
fn special_due<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let left = x.u8(SPECIALS);
    if left == 0 {
        return false;
    }
    let quarter_hp = x.u16(0x60) >> 2;
    let mut sum = 0u16;
    let mut n = left as i16;
    loop {
        sum = sum.wrapping_add(quarter_hp);
        n -= 1;
        if n <= 0 {
            break;
        }
    }
    if Fl::sub16(sum, x.u16(f::HP)).le() {
        return false;
    }
    sub8(x, SPECIALS, 1);
    var::INPUT_MASK.set(x.b, 0x7f);
    x.set8(SHIELD, 8);
    x.set32(f::STATE, ATTACK_4);
    let r = (x.random() as u32) & 3;
    let which = x.rom8(SPECIAL_MOVES + r);
    x.set8(f::SUB2, which);
    true
}

/// 0x919cc..0x91a16: chooses an attack from row `row` of the tables; a negative byte refers to the patience row
/// (0x91a18), which can also refer to a random choice (`wander`).
fn pick_attack<R: Bench>(x: &mut Cx<'_, R>, row: usize) {
    let r = (x.random() as u32) & 0xf;
    let b = x.data8(PICKS[row] + r) as i8;
    if b >= 0 {
        x.set32(f::STATE, 0x0204_0000);
        x.set8(f::SUB2, b as u8);
        return;
    }
    let r = (x.random() as u32) & 0xf;
    let c = x.rom8(COOLDOWNS + r) as i8;
    if c < 0 {
        return wander(x);
    }
    x.set8(COOLDOWN, c as u8);
}

/// 0x91ad8: stays at the center of the area (camera + 0xc0), at the depth of a player (or of the camera) and at
/// altitude 0x40.
pub(super) fn steer<R: Bench>(x: &mut Cx<'_, R>) {
    let target_x = var::SCROLL_X.get(x.b).wrapping_add(0xc0);
    let d = target_x.wrapping_sub(x.u16(0x8)) as i16;
    if d != 0 {
        let step: i16 = if d < 0 { -1 } else { 1 };
        x.add16(0x8, step as u16);
    }
    let mut y = var::SCROLL_Y.get(x.b).wrapping_add(0x60);
    if lib::pick_target_any(x) {
        y = x.linked(f::TARGET).u16(x.b, Obj::Y);
    }
    let d = y.wrapping_sub(x.u16(0x10)) as i16;
    if d != 0 {
        let step: i16 = if d < 0 { -1 } else { 1 };
        x.add16(0x10, step as u16);
    }
    let z = x.u16(0xc) as i16;
    if z != 0x40 {
        let step: i16 = if z > 0x40 { -1 } else { 1 };
        x.add16(0xc, step as u16);
    }
    x.set8(HOVER, 1);
}

// --- +6 = 2 : bond ---------------------------------------------------------------------------------------

fn jump<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(0x7) == 0 {
        x.add8(0x7, 2);
        jump_speeds(x);
        rest(x);
        x.set8(0x38, 0);
        return;
    }
    x.move_xz();
    if x.u16(0xc) as i16 >= 0x2c0 {
        x.set16(0xc, 0x2c0);
    }
    if x.u16(0xc) as i16 > 0x30 {
        return;
    }
    x.set16(0xc, 0x30);
    stand(x);
}

/// 0x91b54: leap speeds to reach the center of the area: horizontal speed 0x100 in the right direction,
/// vertical speed 0x100 × (distance × 16 / 0x100).
fn jump_speeds<R: Bench>(x: &mut Cx<'_, R>) {
    let d1 = x.rom16(0x91b94);
    let mut d2 = d1 as i16;
    let mut d0 = var::SCROLL_X.get(x.b).wrapping_add(0xc0).wrapping_sub(x.u16(0x8)) as i16;
    if d0 < 0 {
        d0 = d0.wrapping_neg();
        d2 = d2.wrapping_neg();
    }
    x.set16(0x14, d2 as u16);
    x.set16(0x16, 0);
    let az = x.rom16(0x91b96);
    x.set16(0x1a, az);
    let half = (az as i16).wrapping_neg() >> 1;
    let mut product = (d0 as u16 as u32).wrapping_mul(half as u16 as u32) as u16;
    let mut d6: i16 = -1;
    loop {
        d6 = d6.wrapping_add(1);
        product = product.wrapping_sub(d1);
        if product & 0x8000 != 0 {
            break;
        }
    }
    x.set16(0x18, (d6 as u16) << 8);
}

// --- +6 = 4: reaches the targeted player -------------------------------------------------------------------

fn approach<R: Bench>(x: &mut Cx<'_, R>) {
    let t = x.u16(TIMER).wrapping_sub(1);
    x.set16(TIMER, t);
    if t & 0x8000 != 0 {
        return stand(x);
    }
    if x.u16(f::TARGET) == 0 {
        return wait(x);
    }
    let p = x.linked(f::TARGET);
    if p.u8(x.b, 0) == 0 {
        return wait(x);
    }
    let px = p.u16(x.b, 0x8);
    if !close_x(x, 0x40, px) {
        return;
    }
    stand(x);
}

/// 0x91c22: reaches in thirds altitude `z` (1/4 of the gap per frame) and depth `y` (1/4); returns true
/// when both are reached.
pub(super) fn close_in_zy<R: Bench>(x: &mut Cx<'_, R>, z: i16, y: i16) -> bool {
    let mut reached = 0;
    let dz = z.wrapping_sub(x.u16(0xc) as i16);
    let mut s = dz >> 2;
    if s == 0 {
        s = 1;
    }
    x.add16(0xc, s as u16);
    if dz.wrapping_abs().wrapping_sub(1) <= 0 {
        reached += 1;
    }
    let dy = y.wrapping_sub(x.u16(0x10) as i16);
    let mut s = dy >> 2;
    if s == 0 {
        s = 1;
    }
    x.add16(0x10, s as u16);
    if dy.wrapping_abs().wrapping_sub(1) <= 0 {
        reached += 1;
        if reached >= 2 {
            return true;
        }
    }
    false
}

/// 0x91c6a: same for altitude `z` (1/4) and X position `px` (1/16).
fn close_x<R: Bench>(x: &mut Cx<'_, R>, z: i16, px: u16) -> bool {
    let mut reached = 0;
    let dz = z.wrapping_sub(x.u16(0xc) as i16);
    let mut s = dz >> 2;
    if s == 0 {
        s = 1;
    }
    x.add16(0xc, s as u16);
    if dz.wrapping_abs().wrapping_sub(1) <= 0 {
        reached += 1;
    }
    let dx = (px as i16).wrapping_sub(x.u16(0x8) as i16);
    let mut s = dx >> 4;
    if s == 0 {
        s = 1;
    }
    x.add16(0x8, s as u16);
    if dx.wrapping_abs().wrapping_sub(1) <= 0 {
        reached += 1;
        if reached >= 2 {
            return true;
        }
    }
    false
}
