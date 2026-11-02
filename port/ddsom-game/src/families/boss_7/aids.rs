//! Boss 7: common aids (choice of a player, sweep of the players, positions, choice of the next attack).

use crate::families::cx::Bench;
use crate::objects::Obj;
use super::*;
use crate::mem::A5;
use crate::machine::Machine;

/// 0x86fee: randomly picks a living player not yet designated (bit in `+0xaf`) as target `+0x50`.
pub(super) fn pick_new<R: Bench>(x: &mut Cx<'_, R>) {
    pick(x, true)
}

/// 0x87020: same without remembering the choice.
pub(super) fn pick_any<R: Bench>(x: &mut Cx<'_, R>) {
    pick(x, false)
}

fn pick<R: Bench>(x: &mut Cx<'_, R>, remember: bool) {
    for _ in 0..4096 {
        let rng = x.random();
        let n = rng as u32 & 3;
        if remember && x.u8(PICKED) >> n & 1 != 0 {
            continue;
        }
        let player = A5 + 0x5de + (n << 9);
        if crate::mem::rd8(x.b, player) == 0 {
            continue;
        }
        if remember {
            let mask = x.u8(PICKED) | 1 << n;
            x.set8(PICKED, mask);
        }
        x.set16(f::TARGET, player as u16);
        return;
    }
}

/// Sweep of the living players (4 × 0x200 bytes from `A5+0x5de`): does the box (`d0`, `d1`, `d2`) in front of the
/// boss (0x7f19c) contain one of them? D6 is the 68000's loop counter.
fn scan<R: Bench>(x: &mut Cx<'_, R>, d0: i16, d1: u16, d2: u16) -> bool {
    for k in 0..4 {
        let player = Obj(A5 + 0x5de + 0x200 * k);
        if crate::mem::rd8(x.b, player.0) != 0 && lib::in_box(x, player, d0, d1, d2).found() {
            return true;
        }
    }
    false
}

/// 0x87c9a (`far`) and 0x87ccc: is a living player in the box? D0 = 0 or 1.
pub(super) fn player_near<R: Bench>(x: &mut Cx<'_, R>, far: bool) -> bool {
    let found = if far { scan(x, -0x130, 0x40, 0x20) } else { scan(x, -0x20, 0x20, 8) };
    found
}

/// 0x87b86: true if the boss is not facing its opponent (`+0x50`) and an animation is in progress (`+0xa3`).
pub(super) fn must_turn<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if x.u8(ANIM_LAST) == 0 || x.u16(f::TARGET) == 0 {
        return false;
    }
    let t = load_target(x);
    let mine = x.u16(0x8);
    let r = Fl::sub16(t.u16(x.b, 0x8), mine);
    let face = if x.u8(f::FLIP) != 0 {
        if r.gt() { 0x20 } else { 0 }
    } else if r.le() {
        0
    } else {
        0x20
    };
    if face == x.u8(f::FLIP) {
        return false;
    }
    true
}

/// 0x87c6c: sidestep drawn from table 0x87c8a.
pub(super) fn random_sidestep<R: Bench>(x: &mut Cx<'_, R>) {
    let rng = x.random();
    let i = rng & 0xe;
    let mut d = x.rom16(0x87c8a + i as u32);
    if x.u8(f::FLIP) != 0 {
        d = d.wrapping_neg();
    }
    x.add16(0x8, d);
}

/// 0x87cfe: brings the boss back into the play area (x from 0x45f to 0x68f, y from 0x168 to 0x19f).
pub(super) fn clamp<R: Bench>(x: &mut Cx<'_, R>) {
    let y = x.u16(0x10);
    if Fl::sub16(y, 0x168).gt() {
        if !Fl::sub16(y, 0x19f).lt() {
            x.set16(0x10, 0x19f);
        }
    } else {
        x.set16(0x10, 0x168);
    }
    let px = x.u16(0x8);
    if Fl::sub16(px, 0x45f).gt() {
        if !Fl::sub16(px, 0x68f).lt() {
            x.set16(0x8, 0x68f);
        }
    } else {
        x.set16(0x8, 0x45f);
    }
}

/// A series of effect puffs (0xb10c8) along the axis: D0 = 1, D1 = 0, D2 = position, D3 = 4.
fn bursts<R: Bench>(x: &mut Cx<'_, R>, spots: &[i16]) {
    for &p in spots {
        lib::shock(x, 1, 0, p as u16, 4);
    }
}

/// 0x87d48: five puffs.
pub(super) fn burst_small<R: Bench>(x: &mut Cx<'_, R>) {
    bursts(x, &[-0x28, -0x18, 0, 0x18, 0x28]);
}

/// 0x87da2: seven puffs forward.
pub(super) fn burst_front<R: Bench>(x: &mut Cx<'_, R>) {
    bursts(x, &[-0x50, -0x38, -0x20, -8, 0x10, 0x28, 0x40]);
}

/// 0x87e20: seven puffs backward.
pub(super) fn burst_back<R: Bench>(x: &mut Cx<'_, R>) {
    bursts(x, &[0x50, 0x38, 0x20, 8, -0x10, -0x28, -0x40]);
}

/// 0x87e9e: if a player is very close (0x7f19c: 0, 0x50, 0x20), the boss jumps elsewhere (0x87eca).
pub(super) fn dodge_nearby<R: Bench>(x: &mut Cx<'_, R>) {
    if scan(x, 0, 0x50, 0x20) {
        dodge_spot(x);
    }
}

/// 0x87eca: a landing point drawn from a table chosen according to the screen quarter where the boss is.
pub(super) fn dodge_spot<R: Bench>(x: &mut Cx<'_, R>) {
    let dx = x.u16(0x8).wrapping_sub(var::SCROLL_X.get(x.b));
    let dy = x.u16(0x10).wrapping_sub(var::SCROLL_Y.get(x.b));
    let right = Fl::sub16(dx, 0xc0).gt();
    let low = Fl::sub16(dy, 0x64).gt();
    let d6 = match (right, low) {
        (false, false) => 0,
        (false, true) => 4,
        (true, false) => 8,
        (true, true) => 0xc,
    };
    let table = rom32(x, 0x87f30 + d6);
    let rng = x.random();
    let i = ((rng) & 3) << 3;
    let a = table.wrapping_add(i as u32);
    let dx = x.data16(a);
    x.add16(0x8, dx);
    let dy = x.data16(a + 2);
    x.add16(0x10, dy);
    let side = x.data8(a + 4);
    x.set8(f::FLIP, side);
    clamp(x)
}

/// 0x870a2 (`front`) / 0x870b4: draws the next attack from the ROM table (according to the slow motion and the
/// difficulty). In slow motion, the third consecutive attack 4 becomes attack 14.
pub(super) fn choose_attack<R: Bench>(x: &mut Cx<'_, R>, front: bool) {
    let slow = x.u8(SLOW) != 0;
    let table = match (front, slow) {
        (true, false) => 0x87126,
        (true, true) => 0x87526,
        (false, false) => 0x87326,
        (false, true) => 0x87726,
    };
    pick_row(x, table, 0x204);
    if slow && x.u8(f::SUB2) == 4 {
        addq8(x, THIRD, 1);
        if x.u8(THIRD) == 3 {
            x.set32(f::STATE, 0x0204_0e00);
            x.set8(THIRD, 0);
        }
    }
}

/// Picks a 32-byte row of `table` according to the difficulty, draws a step from it and moves to state `state` (word
/// `+4`).
fn pick_row<R: Bench>(x: &mut Cx<'_, R>, table: u32, state: u16) {
    // D0.w = difficulty / 2 (the `moveq #0,D0` of one of the callers does not change the low word)
    let level = var::RANK.get(x.b);
    let row = ((level >> 1) << 5) as i16 as i32 as u32;
    let a4 = table.wrapping_add(row);
    let rng = x.random();
    let i = rng & 0x1f;
    let step = x.data8(a4 + i as u32);
    x.set8(f::SUB2, step);
    x.set16(f::STATE, state);
    x.set8(f::SUB3, 0);
}

/// 0x87926: end of an attack. A living player in the zone (0x7f19c: -0x38, 0x38, 0x18) makes the follow-up be drawn
/// from table 0x87986 (state 2, group 8); otherwise the boss goes back to combat.
pub(super) fn after_attack<R: Bench>(x: &mut Cx<'_, R>) {
    if !scan(x, -0x38, 0x38, 0x18) {
        x.set32(f::STATE, FIGHT);
        return;
    }
    pick_row(x, 0x87986, 0x208);
}
