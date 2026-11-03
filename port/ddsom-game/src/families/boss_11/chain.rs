//! Boss 11/12: the twelve rings. Each frame, `update` repositions the rings behind the head according to the ROM
//! tables (32-byte rows at 0x1069f8 chosen by pose `+0xaa`; paths at 0x106cb8), then copies the position of the
//! followed ring (rank `+0xa8`) into the save `+0xa2..+0xa6`.
//!
//! A ring is an ESHL object of type 5: `+0x2f` rank, `+0x42` and `+0x43` orientation and tilt (0 to 31), `+0x12`
//! state word. The neighbor searches (0x8f482, 0x8f4a2) walk through the pool with no check other than the rank.

use crate::families::cx::Bench;
use super::*;

/// First and last slots of the ESHL pool, between which the searches walk.
const FIRST_SEGMENT: u32 = A5 + pools::ESHL.first;
const LAST_SEGMENT: u32 = A5 + pools::ESHL.first + (pools::ESHL.slots - 1) * pools::ESHL.stride;

/// Rows of 32 bytes, one per pose `+0xaa`: `+2` threshold rank, `+4` kind of orientation (0: set, 1: smoothed), `+5`
/// pose of the head (0 to 6), `+6`/`+7` starting orientation and tilt, `+8..` increments per ring (two bytes each).
const ROWS: &str = "boss_11/rows";
/// Per rank (8 bytes): the address of the path table (32 orientations × 32 tilts × four words: X, height, depth
/// index) and of the depth table (32 words) of the ring.
const PATHS: &str = "boss_11/paths";
/// The tables that `PATHS` designates (`data/enemies.json`) and their sizes in bytes.
const PATH_TABLES: [(&str, u32); 6] = [
    ("boss_11/paths/0", 0x2000),
    ("boss_11/paths/8", 0x2000),
    ("boss_11/paths/10", 0x2000),
    ("boss_11/rises/0", 0x40),
    ("boss_11/rises/8", 0x40),
    ("boss_11/rises/10", 0x40),
];

/// Word at the address `a` of a path or depth table (designated by `PATHS`).
fn path16<R: Bench>(x: &mut Cx<'_, R>, a: u32) -> u16 {
    for (name, size) in PATH_TABLES {
        let t = tables::table(name);
        let off = a.wrapping_sub(t.rom);
        if off < size {
            return t.at16(x.b, off);
        }
    }
    tables::held16(x.b, a)
}


fn flip8<R: Bench>(x: &mut Cx<'_, R>) -> u32 {
    if x.u8(f::FLIP) != 0 { 0x18 } else { 8 }
}

/// 0x8f4c2: the ring of rank `+0xa8` (A1), provided there are exactly twelve rings and the rank is unique.
/// Original glitch: D2 keeps the ring found (0 if none), a leftover seen by the following handlers.
fn find_head<R: Bench>(x: &mut Cx<'_, R>) -> Option<u32> {
    let want = x.u8(Boss11::SEG);
    let mut count = 0u8;
    let mut found = 0u32;
    *x.c.d2() = 0;
    for k in 0..0x26 {
        let a1 = FIRST_SEGMENT + 0x60 * k;
        if Obj(a1).u8(x.b, Obj::ACTIVE) != 0 && Obj(a1).u8(x.b, Obj::KIND) == 5 {
            count = count.wrapping_add(1);
            if Obj(a1).u8(x.b, Ring::RANK) == want {
                if found != 0 {
                    return None;
                }
                found = a1;
                *x.c.d2() = a1;
            }
        }
    }
    if count != 0xc || found == 0 {
        return None;
    }
    Some(found)
}

/// 0x8f482: the ring of rank `want` before `from` in the pool (A3), if there is one.
fn find_prev<R: Bench>(x: &mut Cx<'_, R>, want: u8, from: u32) -> Option<u32> {
    let mut a3 = from;
    loop {
        a3 = a3.wrapping_sub(0x60);
        if (a3 as i32) < FIRST_SEGMENT as i32 {
            return None;
        }
        if Obj(a3).u8(x.b, Ring::RANK) == want {
            return Some(a3);
        }
    }
}

/// 0x8f4a2: same, after `from`.
fn find_next<R: Bench>(x: &mut Cx<'_, R>, want: u8, from: u32) -> Option<u32> {
    let mut a3 = from;
    loop {
        a3 = a3.wrapping_add(0x60);
        if (a3 as i32) > LAST_SEGMENT as i32 {
            return None;
        }
        if Obj(a3).u8(x.b, Ring::RANK) == want {
            return Some(a3);
        }
    }
}

/// 0x8ebde.
pub(super) fn update<R: Bench>(x: &mut Cx<'_, R>) {
    let Some(a1) = find_head(x) else { return };
    follow(x, a1);
    let Some(a1) = find_head(x) else { return };
    let (a, b, c) = (Obj(a1).u16(x.b, Obj::X), Obj(a1).u16(x.b, Obj::Z), Obj(a1).u16(x.b, Obj::Y));
    x.set16(Boss11::SAVE_X, a);
    x.set16(Boss11::SAVE_Z, b);
    x.set16(Boss11::SAVE_Y, c);
}


/// 0x8ec02: places the rings according to pose `+0xaa`.
fn follow<R: Bench>(x: &mut Cx<'_, R>, a1: u32) {
    let row = ((x.u8(Boss11::POSE) as u16) << 5) as u32;
    let px = x.u16(Obj::X);
    Obj(a1).set_u16(x.b, Obj::X, px);
    let mut z = x.u16(Obj::Z);
    if (z as i16) < 0 {
        z = 0;
    }
    Obj(a1).set_u16(x.b, Obj::Z, z);
    let py = x.u16(Obj::Y);
    Obj(a1).set_u16(x.b, Obj::Y, py);
    let idx = x.u8(Boss11::SEG);
    let thr = byte(x, ROWS, row + 2);
    let r = Fl::sub8(idx, thr);
    if r.eq() {
        // equal
    } else if r.gt() {
        if !behind(x, a1, idx) {
            return;
        }
    } else {
        ahead(x, a1);
    }
    // 0x8ed58
    if idx == 0 {
        return place_head(x, a1, a1, row);
    }
    if idx == thr {
        let sub = byte(x, ROWS, row + 4) as u16;
        // jump table 0x8ed7a: entry 0 sets the angles (0x8ed7e), the others smooth them (0x8edf2); if a ring is
        // missing, the subroutine cedes control (death state set) and the placement continues
        if sub == 0 {
            rows_a(x, a1, row);
        } else {
            rows_b(x, a1, row);
        }
    }
    positions(x, a1, row)
}

/// "Rank below the threshold" branch (0x8ec3e): the ring turns toward 8 (or 0x18) and the followed rank advances
/// once it is there.
fn ahead<R: Bench>(x: &mut Cx<'_, R>, a1: u32) {
    let goal = flip8(x) as u16;
    let cur = Obj(a1).u8(x.b, Ring::ORIENTATION) as u16;
    let diff = cur.wrapping_sub(goal);
    if diff != 0 {
        let mut d = diff & 0x1f;
        if !Fl::sub16(d, 0x10).lt() {
            d = d.wrapping_sub(0x20);
        }
        let mut v = asr_w(d.wrapping_neg(), 2);
        if v == 0 {
            v = 1;
        }
        let n = Obj(a1).u8(x.b, Ring::ORIENTATION).wrapping_add(v as u8) & 0x1f;
        Obj(a1).set_u8(x.b, Ring::ORIENTATION, n);
    }
    let t = Obj(a1).u8(x.b, Ring::TILT);
    if t != 0 {
        let mut d = t as u16 & 0x1f;
        if !Fl::sub16(d, 0x10).lt() {
            d = d.wrapping_sub(0x20);
        }
        let mut v = asr_w(d, 2);
        if v == 0 {
            v = 1;
        }
        let n = t.wrapping_sub(v as u8) & 0x1f;
        Obj(a1).set_u8(x.b, Ring::TILT, n);
    }
    let cur = Obj(a1).u8(x.b, Ring::ORIENTATION);
    let goal = flip8(x);
    if cur == goal as u8 && Obj(a1).u8(x.b, Ring::TILT) == 0 {
        x.add8(Boss11::SEG, 1);
    }
}

/// "Rank above the threshold" branch (0x8ecc6): the previous ring turns toward 8 (or 0x18). False if the ring is
/// missing.
fn behind<R: Bench>(x: &mut Cx<'_, R>, a1: u32, idx: u8) -> bool {
    let Some(a3) = find_prev(x, idx.wrapping_sub(1), a1) else {
        lost(x);
        return false;
    };
    let goal = flip8(x) as u16;
    let mut diff = (Obj(a3).u8(x.b, Ring::ORIENTATION) as u16).wrapping_sub(goal);
    if diff != 0 {
        let mut d = diff & 0x1f;
        if !Fl::sub16(d, 0x10).lt() {
            d = d.wrapping_sub(0x20);
        }
        let mut v = asr_w(d, 2);
        if v == 0 {
            v = 1;
        }
        diff = v;
    }
    let d1 = diff as u8;
    let n = Obj(a3).u8(x.b, Ring::ORIENTATION).wrapping_sub(d1) as u16 & 0x1f;
    Obj(a3).set_u8(x.b, Ring::ORIENTATION, n as u8);
    let t = Obj(a3).u8(x.b, Ring::TILT);
    let mut d2 = t as u16;
    if t != 0 {
        d2 = d2.wrapping_sub(0x10);
        d2 = asr_w(d2, 2);
        if d2 == 0 {
            d2 = 1;
        }
    }
    let n3 = (Obj(a3).u8(x.b, Ring::TILT).wrapping_add(d2 as u8) as u16) & 0x1f;
    Obj(a3).set_u8(x.b, Ring::TILT, n3 as u8);
    let cur = Obj(a3).u8(x.b, Ring::ORIENTATION);
    let goal = flip8(x);
    if cur == goal as u8 && Obj(a3).u8(x.b, Ring::TILT) == 0 {
        let v = x.u8(Boss11::SEG).wrapping_sub(1);
        x.set8(Boss11::SEG, v);
    }
    true
}

/// 0x8ed7e: orients the previous rings according to a list of increments (A4 = row + 8).
fn rows_a<R: Bench>(x: &mut Cx<'_, R>, a1: u32, row: u32) -> bool {
    let (mut d1, mut d2) = base_angles(x, row);
    let mut a4 = row + 8;
    let mut a2 = a1;
    let flip = x.u8(f::FLIP) != 0;
    let mut d0 = Obj(a1).u8(x.b, Ring::RANK).wrapping_sub(1) as u16;
    loop {
        let Some(a3) = find_prev(x, d0 as u8, a2) else {
            lost(x);
            return false;
        };
        let mut inc = byte(x, ROWS, a4);
        a4 += 1;
        if flip {
            inc = 0x20u8.wrapping_sub(inc);
        }
        d1 = (d1.wrapping_add(inc as u16)) & 0x1f;
        Obj(a3).set_u8(x.b, Ring::ORIENTATION, d1 as u8);
        let mut inc = byte(x, ROWS, a4);
        a4 += 1;
        if flip {
            inc = 0x20u8.wrapping_sub(inc);
        }
        d2 = (d2.wrapping_add(inc as u16)) & 0x1f;
        Obj(a3).set_u8(x.b, Ring::TILT, d2 as u8);
        a2 = a3;
        d0 = d0.wrapping_sub(1);
        if d0 == 0xffff {
            break;
        }
    }
    true
}

/// Starting angles of the row (bytes +6 and +7), reversed if the snake looks left.
fn base_angles<R: Bench>(x: &mut Cx<'_, R>, row: u32) -> (u16, u16) {
    let mut d1 = byte(x, ROWS, row + 6) as u16;
    let mut d2 = byte(x, ROWS, row + 7) as u16;
    if x.u8(f::FLIP) != 0 {
        d1 = 0x20u16.wrapping_sub(d1);
        d2 = 0x20u16.wrapping_sub(d2);
    }
    (d1 & 0x1f, d2 & 0x1f)
}

/// Signed gap brought back to -16..16 then reduced (quarter, negative of the gap): the step applied to the rings
/// (0x8ee48...).
fn smooth(diff: u16) -> u16 {
    let mut d4 = diff;
    if (diff as i16) < 0 {
        d4 = d4.wrapping_neg();
    }
    if Fl::sub16(d4, 0x10).gt() {
        d4 = d4.wrapping_sub(0x10);
    }
    d4 = d4.wrapping_neg();
    if (diff as i16) >= 0 {
        // tst.w D3w ; bpl: D4 stays negative
    } else {
        d4 = d4.wrapping_neg();
    }
    let mut s = asr_w(d4, 2);
    if s == 0 {
        s = 1;
    }
    s
}

/// 0x8edf2: like `rows_a`, but each ring reaches the target orientation by a quarter of the gap.
fn rows_b<R: Bench>(x: &mut Cx<'_, R>, a1: u32, row: u32) -> bool {
    let (mut d1, mut d2) = base_angles(x, row);
    let mut a4 = row + 8;
    let mut a2 = a1;
    let flip = x.u8(f::FLIP) != 0;
    let mut d0 = Obj(a1).u8(x.b, Ring::RANK).wrapping_sub(1) as u16;
    loop {
        let Some(a3) = find_prev(x, d0 as u8, a2) else {
            lost(x);
            return false;
        };
        let mut inc = byte(x, ROWS, a4);
        a4 += 1;
        if flip {
            inc = 0x20u8.wrapping_sub(inc);
        }
        d1 = (d1.wrapping_add(inc as u16)) & 0x1f;
        let cur = Obj(a3).u8(x.b, Ring::ORIENTATION) as u16;
        let diff = cur.wrapping_sub(d1);
        if diff != 0 {
            let s = smooth(diff);
            Obj(a3).set_u8(x.b, Ring::ORIENTATION, (s.wrapping_add(cur) & 0x1f) as u8);
        }
        let mut inc = byte(x, ROWS, a4);
        a4 += 1;
        if flip {
            inc = 0x20u8.wrapping_sub(inc);
        }
        d2 = (d2.wrapping_add(inc as u16)) & 0x1f;
        let cur = Obj(a3).u8(x.b, Ring::TILT) as u16;
        let diff = cur.wrapping_sub(d2);
        let step = if diff != 0 { smooth(diff) } else { cur_zero(cur) };
        Obj(a3).set_u8(x.b, Ring::TILT, (step.wrapping_add(cur) & 0x1f) as u8);
        a2 = a3;
        d0 = d0.wrapping_sub(1);
        if d0 == 0xffff {
            break;
        }
    }
    true
}

/// When the gap is zero, the original adds D3b = 0: the orientation does not change.
fn cur_zero(_cur: u16) -> u16 {
    0
}

/// 0x8eeca and 0x8ef60: positions of the previous rings according to the paths, then of the head.
fn positions<R: Bench>(x: &mut Cx<'_, R>, a1: u32, row: u32) {
    let mut a2 = a1;
    let n = Obj(a1).u8(x.b, Ring::RANK) as i32 - 1;
    if n >= 0 {
        let mut d0 = n as u16;
        loop {
            let Some(a3) = find_prev(x, d0 as u8, a2) else {
                return lost(x);
            };
            { let t = Obj(a2).u16(x.b, Obj::X); Obj(a3).set_u16(x.b, Obj::X, t); }
            { let t = Obj(a2).u16(x.b, Obj::Z); Obj(a3).set_u16(x.b, Obj::Z, t); }
            { let t = Obj(a2).u16(x.b, Obj::Y); Obj(a3).set_u16(x.b, Obj::Y, t); }
            let a4 = (d0 << 3) as u32;
            let mut a6 = long(x, PATHS, a4);
            let ori = Obj(a3).u8(x.b, Ring::ORIENTATION) as u16;
            a6 = a6.wrapping_add((ori.wrapping_add(0x10) & 0x1f) as u32 * 0x100);
            let tilt = (Obj(a3).u8(x.b, Ring::TILT) as u32) << 3;
            let ax = asr_w(path16(x, a6 + tilt), 8);
            { let t = Obj(a3).u16(x.b, Obj::X).wrapping_add(ax); Obj(a3).set_u16(x.b, Obj::X, t); }
            let az = asr_w(path16(x, a6 + 2 + tilt), 8);
            { let t = Obj(a3).u16(x.b, Obj::Z).wrapping_add(az); Obj(a3).set_u16(x.b, Obj::Z, t); }
            let k = (path16(x, a6 + 4 + tilt) & 0x1f) * 2;
            let a6b = long(x, PATHS, a4 + 4);
            let ay = asr_w(path16(x, a6b + k as u32), 8);
            { let t = Obj(a3).u16(x.b, Obj::Y).wrapping_add(ay); Obj(a3).set_u16(x.b, Obj::Y, t); }
            let s = Obj(a3).u8(x.b, Obj::VARIANT) as u16 + 1;
            Obj(a3).set_u16(x.b, Ring::POSE, s);
            a2 = a3;
            d0 = d0.wrapping_sub(1);
            if d0 == 0xffff {
                break;
            }
        }
    }
    place_head(x, a1, a2, row)
}

/// 0x8ef60: the head places itself against the ring of rank 0 (`a2`) then the row's pose sets the following rings.
fn place_head<R: Bench>(x: &mut Cx<'_, R>, a1: u32, a2: u32, row: u32) {
    let mut d0 = 4u16;
    if x.u8(f::FLIP) == 0 {
        d0 = d0.wrapping_neg();
    }
    let px = d0.wrapping_add(Obj(a2).u16(x.b, Obj::X));
    x.set16(Obj::X, px);
    let pz = 3u16.wrapping_add(Obj(a2).u16(x.b, Obj::Z));
    x.set16(Obj::Z, pz);
    let py = Obj(a2).u16(x.b, Obj::Y);
    x.set16(Obj::Y, py);
    x.set16(Obj::Y, py.wrapping_sub(1));
    x.set16(Obj::Z, pz.wrapping_add(1));
    let pose = byte(x, ROWS, row + 5) as u16;
    // a missing ring interrupts the pose (death state set) but not what follows
    // jump table 0x8efa2 (0x8efb0, 0x8eff6, 0x8f0da, 0x8f1a4, 0x8f21a, 0x8f294, 0x8f30a)
    match pose {
        0 => pose_0(x, a1),
        1 => pose_1(x, a1, row),
        2 => pose_2(x, a1, row),
        3 => pose_3(x, a1, row),
        4 => pose_4(x, a1, row),
        5 => pose_5(x, a1, row),
        _ => pose_6(x, a1, row),
    };
    trail(x, a1);
}

/// Rank of ring `a1` at row `row`: below the threshold (`+2`), the pose does not apply.
fn guarded<R: Bench>(x: &mut Cx<'_, R>, a1: u32, row: u32) -> Option<u16> {
    let d0 = Obj(a1).u8(x.b, Ring::RANK);
    if Fl::sub8(d0, byte(x, ROWS, row + 2)).lt() {
        return None;
    }
    Some(d0 as u16)
}

/// Makes the orientation `(+0x42, +0x43)` follow step by step: the next ring takes that of the previous one.
fn ripple<R: Bench>(x: &mut Cx<'_, R>, mut d0: u16, start: u32) -> bool {
    let mut d2 = Obj(start).u16(x.b, Ring::ANGLES);
    let mut a2 = start;
    loop {
        if !Fl::sub16(d0, 0xb).lt() {
            return true;
        }
        d0 = d0.wrapping_add(1);
        let Some(a3) = find_next(x, d0 as u8, a2) else {
            lost(x);
            return false;
        };
        let d1 = Obj(a3).u16(x.b, Ring::ANGLES);
        Obj(a3).set_u16(x.b, Ring::ANGLES, d2);
        d2 = d1;
        a2 = a3;
    }
}

/// Pose 0 (0x8efb0): the whole snake looks in the direction of the head.
fn pose_0<R: Bench>(x: &mut Cx<'_, R>, a1: u32) -> bool {
    let v = flip8(x) as u8;
    Obj(a1).set_u8(x.b, Ring::ORIENTATION, v);
    Obj(a1).set_u8(x.b, Ring::TILT, 0);
    let mut a2 = a1;
    let mut d0 = Obj(a1).u8(x.b, Ring::RANK) as u16;
    loop {
        if !Fl::sub16(d0, 0xb).lt() {
            return true;
        }
        d0 += 1;
        let Some(a3) = find_next(x, d0 as u8, a2) else {
            lost(x);
            return false;
        };
        { let t = Obj(a2).u8(x.b, Ring::ORIENTATION); Obj(a3).set_u8(x.b, Ring::ORIENTATION, t); }
        { let t = Obj(a2).u8(x.b, Ring::TILT); Obj(a3).set_u8(x.b, Ring::TILT, t); }
        a2 = a3;
    }
}

/// Pose 1 (0x8eff6): undulation: the orientation reaches 8/0x18, the tilt oscillates between 4 and 0x1c.
fn pose_1<R: Bench>(x: &mut Cx<'_, R>, a1: u32, row: u32) -> bool {
    let Some(d0) = guarded(x, a1, row) else { return true };
    if d0 > 0xb {
        return true;
    }
    let goal = flip8(x) as u16;
    let cur = Obj(a1).u8(x.b, Ring::ORIENTATION) as u16;
    let diff = goal.wrapping_sub(cur);
    if diff != 0 {
        let mut d = diff & 0x1f;
        if !Fl::sub16(d, 0x10).lt() {
            d = d.wrapping_sub(0x20);
        }
        let mut v = asr_w(d.wrapping_neg(), 2);
        if v == 0 {
            v = 1;
        }
        Obj(a1).set_u8(x.b, Ring::ORIENTATION, (v.wrapping_add(cur) & 0x1f) as u8);
        x.set8(Boss11::WAVE, 1);
    }
    let mut d1 = Obj(a1).u8(x.b, Ring::TILT);
    let mut d2: u8 = 1;
    if !(Fl::sub8(d1, 4).le() || Fl::sub8(d1, 0x1c).gt()) {
        if !Fl::sub8(d1, 0x10).lt() {
            d2 = d2.wrapping_neg();
        }
        d1 = d1.wrapping_sub(d2) & 0x1f;
        Obj(a1).set_u8(x.b, Ring::TILT, d1);
        return true;
    }
    if var::FRAME_COUNTER.get(x.b) & 1 == 0 {
        return true;
    }
    let mut d2 = x.u8(Boss11::WAVE);
    let mut d3 = d2;
    if (d3 as i8) < 0 {
        d3 = d3.wrapping_neg();
    }
    if d3 != 1 {
        d2 = 1;
        x.set8(Boss11::WAVE, d2);
    }
    d1 = d1.wrapping_add(d2) & 0x1f;
    if !Fl::sub8(d1, 4).le() && !Fl::sub8(d1, 0x1c).gt() {
        d2 = d2.wrapping_neg();
        x.set8(Boss11::WAVE, d2);
    } else {
        Obj(a1).set_u8(x.b, Ring::TILT, d1);
    }
    ripple_words(x, d0, a1)
}

/// Makes the word `(+0x42)` follow step by step from ring `start` (rank `d0`).
fn ripple_words<R: Bench>(x: &mut Cx<'_, R>, d0: u16, start: u32) -> bool {
    ripple(x, d0, start)
}

/// Pose 2 (0x8f0da): the head looks toward its previous point; the orientations shift from one ring to the next.
fn pose_2<R: Bench>(x: &mut Cx<'_, R>, a1: u32, row: u32) -> bool {
    let Some(d0) = guarded(x, a1, row) else { return true };
    if !Fl::sub16(d0, 0xb).lt() {
        return true;
    }
    let (d1, d2) = (x.u16(Obj::X), x.u16(Obj::Z));
    let (d3, d4) = (x.u16(Boss11::SAVE_X), x.u16(Boss11::SAVE_Z));
    // D2.w, D3.w and D4.w: leftovers for the following handlers (`dispatch::Leftovers`)
    setw(x, 2, d2);
    setw(x, 3, d3);
    setw(x, 4, d4);
    if d3 != d1 || d4 != d2 {
        let angle = lib::aim(x, (d3, d4), (d1, d2)).wrapping_sub(Obj(a1).u8(x.b, Ring::ORIENTATION));
        if angle != 0 {
            // D0.w = the gap (high byte zero: the direction fits in one byte)
            let w = (angle as u16).wrapping_add(8) & 0x1f;
            let mut s: u8 = 1;
            if !Fl::sub16(w, 0x10).lt() {
                s = 0xff;
            }
            let n = Obj(a1).u8(x.b, Ring::ORIENTATION).wrapping_add(s) & 0x1f;
            Obj(a1).set_u8(x.b, Ring::ORIENTATION, n);
        }
        Obj(a1).set_u8(x.b, Ring::ORIENTATION, 0);
    }
    let mut d0 = Obj(a1).u8(x.b, Ring::RANK) as u16;
    let mut a2 = a1;
    let mut d2 = Obj(a2).u8(x.b, Ring::ORIENTATION) as u16;
    loop {
        if !Fl::sub16(d0, 0xb).lt() {
            return true;
        }
        d0 += 1;
        let Some(a3) = find_next(x, d0 as u8, a2) else {
            lost(x);
            return false;
        };
        let d1 = Obj(a3).u8(x.b, Ring::ORIENTATION);
        Obj(a3).set_u8(x.b, Ring::ORIENTATION, d2 as u8);
        d2 = d1 as u16;
        a2 = a3;
    }
}

/// Pose 3 (0x8f1a4): all the rings reach the head's orientation (reversed during the reaction).
fn pose_3<R: Bench>(x: &mut Cx<'_, R>, a1: u32, row: u32) -> bool {
    let Some(d0) = guarded(x, a1, row) else { return true };
    if !Fl::sub16(d0, 0xb).lt() {
        return true;
    }
    let mut d5 = x.u8(f::FLIP);
    if x.u8(f::STATE) == 4 {
        d5 ^= 0x20;
    }
    let mut d0 = d0;
    let mut a2 = a1;
    loop {
        let goal = if d5 != 0 { 0x18u16 } else { 8 };
        let cur = Obj(a2).u8(x.b, Ring::ORIENTATION) as u16;
        let diff = cur.wrapping_sub(goal);
        if diff != 0 {
            let mut d = diff & 0x1f;
            if !Fl::sub16(d, 0x10).lt() {
                d = d.wrapping_sub(0x20);
            }
            let mut v = asr_w(d.wrapping_neg(), 2);
            if v == 0 {
                v = 1;
            }
            Obj(a2).set_u8(x.b, Ring::ORIENTATION, (cur.wrapping_add(v) & 0x1f) as u8);
        }
        if !Fl::sub16(d0, 0xb).lt() {
            return true;
        }
        d0 = (d0 & 0xff00) | (d0 as u8).wrapping_add(1) as u16;
        let Some(a3) = find_next(x, d0 as u8, a2) else {
            lost(x);
            return false;
        };
        a2 = a3;
    }
}

/// Pose 4 (0x8f21a): the head's tilt follows direction `+0x2d`, the orientations shift.
fn pose_4<R: Bench>(x: &mut Cx<'_, R>, a1: u32, row: u32) -> bool {
    let Some(d0) = guarded(x, a1, row) else { return true };
    if !Fl::sub16(d0, 0xb).lt() {
        return true;
    }
    let w = (x.u8(Enemy::HEADING) as u16).wrapping_add(8) & 0x1f;
    let diff = (w as u8).wrapping_sub(Obj(a1).u8(x.b, Ring::TILT));
    if diff != 0 {
        // D0.w: the low byte replaced by the gap (the high byte of `w` is zero)
        let w = diff as u16 & 0x1f;
        let mut s: u8 = 1;
        if !Fl::sub16(w, 0x10).lt() {
            s = 0xff;
        }
        let n = Obj(a1).u8(x.b, Ring::TILT).wrapping_add(s) & 0x1f;
        Obj(a1).set_u8(x.b, Ring::TILT, n);
    }
    Obj(a1).set_u8(x.b, Ring::ORIENTATION, 8);
    let d0 = Obj(a1).u8(x.b, Ring::RANK) as u16;
    shift_words(x, d0, a1)
}

/// Shifts the words `(+0x42)` from one ring to the next starting at `start` (rank `d0`).
fn shift_words<R: Bench>(x: &mut Cx<'_, R>, mut d0: u16, start: u32) -> bool {
    let mut a2 = start;
    let mut d2 = Obj(a2).u16(x.b, Ring::ANGLES);
    loop {
        if !Fl::sub16(d0, 0xb).lt() {
            return true;
        }
        d0 += 1;
        let Some(a3) = find_next(x, d0 as u8, a2) else {
            lost(x);
            return false;
        };
        let d1 = Obj(a3).u16(x.b, Ring::ANGLES);
        Obj(a3).set_u16(x.b, Ring::ANGLES, d2);
        d2 = d1;
        a2 = a3;
    }
}

/// Pose 5 (0x8f294): the head turns one notch each frame, without going further.
fn pose_5<R: Bench>(x: &mut Cx<'_, R>, a1: u32, row: u32) -> bool {
    let Some(d0) = guarded(x, a1, row) else { return true };
    if !Fl::sub16(d0, 0xb).lt() {
        return true;
    }
    let goal = flip8(x) as u8;
    if Obj(a1).u8(x.b, Ring::ORIENTATION) != goal {
        Obj(a1).set_u8(x.b, Ring::ORIENTATION, goal);
        x.set8(Boss11::WAVE, 1);
    }
    let step: u8 = if x.u8(f::FLIP) != 0 { 1 } else { 0xff };
    let n = Obj(a1).u8(x.b, Ring::TILT).wrapping_add(step) & 0x1f;
    Obj(a1).set_u8(x.b, Ring::TILT, n);
    shift_words(x, d0, a1)
}

/// Pose 6 (0x8f30a).
fn pose_6<R: Bench>(x: &mut Cx<'_, R>, a1: u32, row: u32) -> bool {
    let Some(d0) = guarded(x, a1, row) else { return true };
    if !Fl::sub16(d0, 0xb).lt() {
        return true;
    }
    let step: u8 = if x.u8(f::FLIP) != 0 { 1 } else { 0xff };
    let n = Obj(a1).u8(x.b, Ring::ORIENTATION).wrapping_add(step) & 0x1f;
    Obj(a1).set_u8(x.b, Ring::ORIENTATION, n);
    Obj(a1).set_u8(x.b, Ring::TILT, 0);
    shift_words(x, d0, a1)
}

/// 0x8f36a: the following rings (increasing ranks) place themselves behind the previous one according to the paths.
fn trail<R: Bench>(x: &mut Cx<'_, R>, a1: u32) -> bool {
    let mut a2 = a1;
    let v = x.u8(Boss11::RING_FLAGS) & !1;
    x.set8(Boss11::RING_FLAGS, v);
    let mut d0 = Obj(a1).u8(x.b, Ring::RANK) as u16;
    loop {
        if !Fl::sub16(d0, 0xb).lt() {
            return true;
        }
        d0 += 1;
        let Some(a3) = find_next(x, d0 as u8, a2) else {
            lost(x);
            return false;
        };
        let z = Obj(a2).u16(x.b, Obj::Z);
        Obj(a3).set_u16(x.b, Obj::Z, z);
        if (z as i16) < 0 {
            let v = x.u8(Boss11::RING_FLAGS) | 1;
            x.set8(Boss11::RING_FLAGS, v);
        }
        { let t = Obj(a2).u16(x.b, Obj::X); Obj(a3).set_u16(x.b, Obj::X, t); }
        { let t = Obj(a2).u16(x.b, Obj::Y); Obj(a3).set_u16(x.b, Obj::Y, t); }
        let a4 = ((d0 - 1) << 3) as u32;
        let (mut a6, mut d1);
        let mut d2;
        loop {
            a6 = long(x, PATHS, a4).wrapping_add((Obj(a2).u8(x.b, Ring::ORIENTATION) as u32) << 8);
            d1 = (Obj(a2).u8(x.b, Ring::TILT) as u32) << 3;
            d2 = asr_w(path16(x, a6 + 2 + d1), 8).wrapping_add(Obj(a3).u16(x.b, Obj::Z));
            if (d2 as i16) >= 0 || x.u8(Boss11::RING_FLAGS) & 1 != 0 {
                break;
            }
            let mut d5 = x.u8(f::FLIP);
            if x.u8(f::STATE) == 4 {
                d5 ^= 0x20;
            }
            let d3 = (Obj(a2).u8(x.b, Ring::ORIENTATION) as u16) & 0x1f;
            let goal = if d5 != 0 { 0x18u16 } else { 8 };
            let diff = d3.wrapping_sub(goal);
            if diff != 0 {
                let mut s: u8 = 1;
                if !Fl::sub16(diff & 0x1f, 0x10).lt() {
                    s = 0xff;
                }
                let n = Obj(a2).u8(x.b, Ring::ORIENTATION).wrapping_sub(s) & 0x1f;
                Obj(a2).set_u8(x.b, Ring::ORIENTATION, n);
                continue;
            }
            let d3 = (Obj(a2).u8(x.b, Ring::TILT) as u16) & 0x1f;
            if d3 == 0 {
                break;
            }
            let mut s: u8 = 1;
            if !Fl::sub8(d3 as u8, 0x10).lt() {
                s = 0xff;
            }
            let n = Obj(a2).u8(x.b, Ring::TILT).wrapping_sub(s) & 0x1f;
            Obj(a2).set_u8(x.b, Ring::TILT, n);
        }
        Obj(a3).set_u16(x.b, Obj::Z, d2);
        let ax = asr_w(path16(x, a6 + d1), 8);
        { let t = Obj(a3).u16(x.b, Obj::X).wrapping_add(ax); Obj(a3).set_u16(x.b, Obj::X, t); }
        let k = (path16(x, a6 + 4 + d1) & 0x1f) * 2;
        let a6b = long(x, PATHS, a4 + 4);
        let ay = asr_w(path16(x, a6b + k as u32), 8);
        { let t = Obj(a3).u16(x.b, Obj::Y).wrapping_add(ay); Obj(a3).set_u16(x.b, Obj::Y, t); }
        let s = Obj(a3).u8(x.b, Obj::VARIANT) as u16 + 1;
        Obj(a3).set_u16(x.b, Ring::POSE, s);
        a2 = a3;
    }
}
