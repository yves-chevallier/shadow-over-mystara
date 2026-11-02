//! Player effects (PEFF), fourteenth series: type 41 (the guardian summoned by the wall magic, created by
//! `player/spawn.rs::wall_target`). See `docs/families/player.md`.

use crate::families::cx::Bench;
use crate::families::cx::{handlers, Cx};
use super::pshell::*;
use crate::mem::{rd32, rd8, wr32, wr8};
use crate::objects::Obj;
use crate::objects::Current;

const ANIMS: u32 = 0x1bc9b6;
const SPEEDS: u32 = 0x1c7836;
const FRAMES: u32 = 0x11bffa;
/// Palette copied into the graphics RAM at creation (8 longs, read in the clear).
const PALETTE_SOURCE: u32 = 0x5036a;
const PALETTE_WINDOW: u32 = 0x91c180;
const OFFSETS: u32 = 0x5043a;
/// Boxes: to engage, to hurt.
const BOX_ENGAGE: u32 = 0x50b8a;
const BOX_STRIKE: u32 = 0x50b92;
/// Compteur de gardiens en vie (`A5-0x5fbe`, adresse absolue).
const COUNTER: u32 = 0xff2042;

fn signed(v: u16, negate: bool) -> u16 {
    if negate { v.wrapping_neg() } else { v }
}

/// The guardian: comes out of the ground (substate 0), looks for the nearest enemy (2), wanders for lack of a target
/// (4), leaps on it (6), then disappears blinking (state 4). Lifetime `+0x42` (0x4b0 frames).
mod guardian {
    use super::*;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        if x.a5_u8(0x34) != x.u8(0x5b) {
            return gone(x);
        }
        match x.state(0x50362) {
            0 => init(x),
            2 => active(x),
            4 => leaving(x),
            _ => gone(x),
        }
    }

    fn gone<R: Bench>(x: &mut Cx<'_, R>) {
        let n = rd8(x.b, COUNTER).wrapping_sub(1);
        wr8(x.b, COUNTER, n);
        x.free_peff();
    }

    fn die<R: Bench>(x: &mut Cx<'_, R>) {
        x.set32(0x4, 0x0400_0000);
    }

    fn init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        for i in 0..8u32 {
            let v = rd32(x.b, PALETTE_SOURCE + 4 * i);
            wr32(x.b, PALETTE_WINDOW + 4 * i, v);
        }
        let owner = x.linked(f::OWNER);
        for off in [0x8, 0xc, 0x10] {
            let v = owner.u32(x.b, off);
            x.set32(off, v);
        }
        let y = x.u32(0x10).wrapping_sub(1);
        x.set32(0x10, y);
        x.set8(0x4a, 0);
        x.set8(0x46, 0xa);
        x.set8(0x48, 0x3c);
        x.set32(0x4c, SPEEDS);
        x.set16(0x18, 0x400);
        x.set16(0x1a, -0x40i16 as u16);
        x.set16(0x16, 0);
        x.set32(0x1c, 0);
        let (mut vx, mut angle) = (-0x200i16 as u16, 0x18u8);
        let row = ((owner.u8(x.b, 0x3) >> 1) as u32) << 2;
        let mut dx = x.rom16(OFFSETS + row);
        let dz = x.rom16(OFFSETS + row + 2);
        let flip = owner.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        if flip != 0 {
            dx = dx.wrapping_neg();
            vx = vx.wrapping_neg();
            angle = 8;
        }
        x.add16(0xc, dz);
        x.add16(0x8, dx);
        x.set16(0x14, vx);
        x.set8(0x2d, angle);
        x.set16(0x42, 0x4b0);
        x.anim(ANIMS, 0);
        x.reinsert();
    }

    fn active<R: Bench>(x: &mut Cx<'_, R>) {
        let owner = x.linked(f::OWNER);
        if owner.u8(x.b, 0x4) as i8 >= 4 || x.a5_u8(a5::PAUSED) != 0 || x.a5_u8(0x35) == 0 {
            return die(x);
        }
        // the jump to state 4 comes back here (the substate was called by `jsr`): the end runs in every case
        match x.u8(0x5) {
            0 => start(x),
            2 => seek(x),
            4 => wander(x),
            _ => lunge(x),
        }
        x.keep_in_zone();
        // Original glitch: the collision registers (D2...) pass on to the following handlers; the freeze shard
        // (0x4bda8) draws its bounce in the inherited D2 (`sorts_combattant`, `sorts_voleur`)
        x.push_terrain();
        x.reinsert();
    }

    /// End of life: state 4, substates zero, then the exit (`bra 0x50794`).
    fn leave<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        x.set8(0x5, 0);
        x.set16(0x6, 0);
        leaving(x);
    }

    // --- substate 0: comes out of the ground -------------------------------------------------------------------

    fn start<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x6) {
            0 => {
                x.move_xyz();
                if (x.u16(0x18) as i16) <= 0 {
                    x.add8(0x6, 2);
                    x.set8(0x4b, 0x1e);
                }
            }
            2 => {
                x.add8(0x4b, 0xff);
                if x.u8(0x4b) == 0 {
                    x.add8(0x6, 2);
                    x.set8(0x2a, 1);
                    x.set8(0x2b, 0xc);
                    x.anim(ANIMS, 6);
                } else if x.u8(0x4b) & 1 != 0 {
                    dust(x);
                }
            }
            4 => {
                x.anim_step();
                x.move_xz();
                if (x.u16(0xc) as i16) <= 0 {
                    x.add8(0x6, 2);
                    x.set32(0xc, 0);
                    x.anim(ANIMS, 5);
                }
            }
            _ => {
                if x.anim_step() {
                    x.add8(0x5, 2);
                    x.set8(0x6, 0);
                    x.anim(ANIMS, 1);
                }
            }
        }
    }

    /// Dust of the arrival (`0xb10c8`): EFFCT 7 effect at a slightly higher height.
    fn dust<R: Bench>(x: &mut Cx<'_, R>) {
        let Some(o) = crate::effects_aids::shock(x.b, x.o, 7, 0x200, 0, 0xffff) else { return };
        o.set_u8(x.b, 0x43, 1);
        let z = o.u16(x.b, 0xc).wrapping_add(0x18);
        o.set_u16(x.b, 0xc, z);
    }

    // --- substate 2: heads for its target ----------------------------------------------------------------

    fn seek<R: Bench>(x: &mut Cx<'_, R>) {
        x.add16(0x42, 0xffff);
        if x.u16(0x42) == 0 {
            return leave(x);
        }
        if x.u8(0x4a) != 0 {
            return flip_turn(x);
        }
        if x.u8(0x6) == 0 {
            x.anim(ANIMS, 3);
            find_target(x);
            if x.u16(0x5c) != 0 {
                x.add8(0x6, 2);
            } else {
                x.set8(0x5, 4);
                x.set8(0x6, 0);
            }
            return;
        }
        x.anim_step();
        x.add8(0x46, 0xff);
        if x.u8(0x46) == 0 {
            x.set8(0x46, 2);
            // A1: the target, or the last object examined by the search (original glitch: not necessarily the new
            // target)
            let mut toward = x.linked(0x5c);
            let mut lost = false;
            if toward.u16(x.b, 0x62) == 0 {
                if let Some(o) = find_target(x) {
                    toward = o;
                }
                lost = x.u16(0x5c) == 0;
                if lost {
                    x.set8(0x5, 4);
                    x.set8(0x6, 0);
                }
            }
            if lost {
                return;
            }
            turn(x, toward);
        }
        if (x.u8(0x33) as i8) >= 0 {
            x.move_by_table_xy();
        }
        engage_test(x);
    }

    /// Turn-around in progress: animation 2 then back to frame 3 with the facing inverted.
    fn flip_turn<R: Bench>(x: &mut Cx<'_, R>) {
        if !x.anim_step() {
            return;
        }
        let f = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, f);
        x.set8(0x4a, 0);
        x.anim(ANIMS, 3);
    }

    // --- substate 4: wandering without a target ---------------------------------------------------------------------

    fn wander<R: Bench>(x: &mut Cx<'_, R>) {
        x.add16(0x42, 0xffff);
        if x.u16(0x42) == 0 {
            return leave(x);
        }
        if x.u8(0x4a) != 0 {
            return flip_turn(x);
        }
        x.anim_step();
        x.add8(0x46, 0xff);
        if x.u8(0x46) == 0 {
            x.set8(0x46, 2);
            find_target(x);
            if x.u16(0x5c) != 0 {
                x.add8(0x5, 0xfe);
                x.set8(0x6, 2);
                return;
            }
            let owner = x.linked(f::OWNER);
            turn(x, owner);
        }
        if (x.u8(0x33) as i8) >= 0 {
            x.move_by_table_xy();
        }
    }

    // --- substate 6: leap onto the target -----------------------------------------------------------------------

    fn lunge<R: Bench>(x: &mut Cx<'_, R>) {
        x.add16(0x42, 0xffff);
        if x.u16(0x42) == 0 {
            return leave(x);
        }
        match x.u8(0x7) {
            0 => {
                x.add8(0x7, 2);
                x.anim(ANIMS, 4);
            }
            2 => {
                if !x.anim_step() {
                    return;
                }
                x.add8(0x7, 2);
                x.set16(0x18, 0x400);
                x.set16(0x1a, -0x40i16 as u16);
                x.set16(0x14, 0x300);
                x.set16(0x16, 0);
                x.set32(0x1c, 0);
                if x.u8(f::FLIP) == 0 {
                    let vx = x.u16(0x14);
                    x.set16(0x14, vx.wrapping_neg());
                }
            }
            4 => {
                strike(x);
                x.move_xz();
                if (x.u16(0xc) as i16) > 0 {
                    return;
                }
                x.set32(0xc, 0);
                let mut vy = -0x40i16 as u16;
                if x.u8(f::FLIP) == 0 {
                    vy = vy.wrapping_neg();
                }
                x.set16(0x16, vy);
                x.add8(0x7, 2);
                x.anim(ANIMS, 5);
            }
            _ => {
                x.add8(0x4b, 0xff);
                let t = x.u8(0x4b) & 3;
                x.set8(0x4b, t);
                if t == 0 {
                    crate::effects_aids::shock(x.b, x.o, 1, 1, 0xfff0, 0xffff);
                }
                x.move_x();
                if x.u8(0x14) == 0 {
                    x.set32(0x4, 0x0202_0000);
                }
            }
        }
    }

    // --- state 4: exit ------------------------------------------------------------------------------------

    fn leaving<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x5) {
            0 => {
                x.add8(0x5, 2);
                x.set8(0x4b, 0x1e);
                fade_dust(x);
            }
            2 => fade_dust(x),
            4 => {
                x.add8(0x5, 2);
                x.set16(0x18, 0x400);
                x.set16(0x1a, -0x40i16 as u16);
                x.set32(0x14, 0);
            }
            6 => {
                x.move_xz();
                if (x.u16(0xc) as i16) <= 0 {
                    x.set32(0xc, 0);
                    x.add8(0x5, 2);
                    x.set8(0x46, 0x28);
                }
            }
            _ => {
                let v = x.u8(0x1) ^ 2;
                x.set8(0x1, v);
                x.add8(0x46, 0xff);
                if x.u8(0x46) == 0 {
                    x.add8(0x4, 2);
                }
            }
        }
        x.reinsert();
    }

    /// `0x507bc`: dust of the disappearance for 0x1e frames, then the guardian sinks.
    fn fade_dust<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4b, 0xff);
        if x.u8(0x4b) == 0 {
            x.add8(0x5, 2);
            x.set8(0x2a, 0);
            x.anim(ANIMS, 0);
        } else if x.u8(0x4b) & 1 != 0 {
            dust(x);
        }
    }

    // --- searches and tests ---------------------------------------------------------------------------------------

    /// `0x5085a`: the nearest enemy (then boss); `+0x5c` receives its address.
    fn find_target<R: Bench>(x: &mut Cx<'_, R>) -> Option<Obj> {
        let mut best = u32::MAX;
        let mut found = 0u16;
        let mut last = None;
        for (first, free, total) in [(0x299eu32, 0x7e62u32, 0x16u16), (0x3a1e, 0x7e6c, 3)] {
            let mut left = total.wrapping_sub(x.a5_u16(free)) as u8;
            if left == 0 {
                continue;
            }
            let mut a1 = 0xff8000 + first;
            loop {
                let o = Obj(a1);
                if o.u8(x.b, 0) != 0 {
                    last = Some(o);
                    if o.u16(x.b, 0x62) != 0 && o.u8(x.b, 0x95) & 2 == 0 && o.u8(x.b, 0x1) & 2 == 0 {
                        let mut d = 0u32;
                        for off in [0x8u32, 0xc, 0x10] {
                            d = d.wrapping_add(o.u32(x.b, off).abs_diff(x.u32(off)));
                        }
                        if d < best {
                            best = d;
                            found = o.0 as u16;
                        }
                    }
                    left -= 1;
                    if left == 0 {
                        break;
                    }
                }
                a1 += 0xc0;
            }
        }
        x.set16(0x5c, found);
        last
    }

    /// `0x508fc`: turns one notch toward `toward` (the object the caller leaves in A1); if the facing must change,
    /// starts the turn-around.
    fn turn<R: Bench>(x: &mut Cx<'_, R>, toward: Obj) {
        let diff = crate::aids::direction_to(x.b, x.o, toward).wrapping_sub(x.u8(0x2d)) & 0x1f;
        let step: u8 = if diff > 0x10 { 0xff } else { 1 };
        let angle = x.u8(0x2d).wrapping_add(step) & 0x1f;
        x.set8(0x2d, angle);
        let want: u8 = if (angle as i8) > 0x10 { 0 } else { 0x20 };
        if want != x.u8(f::FLIP) {
            x.set8(0x4a, 1);
            x.anim(ANIMS, 2);
        }
    }

    fn overlap<R: Bench>(x: &mut Cx<'_, R>, m: Obj, frame: u32, mine: u32, with_z: bool) -> bool {
        let (e, p) = (frame, mine);
        let d = (x.data8(e + 4) as i8 as i16 as u16).wrapping_add(m.u16(x.b, 0x10));
        let q = (x.data8(p + 4) as i8 as i16 as u16).wrapping_add(x.u16(0x10));
        let reach = x.data8(e + 5).wrapping_add(x.data8(p + 5)) as u16;
        if d.wrapping_sub(q).wrapping_add(reach) > reach.wrapping_mul(2) {
            return false;
        }
        let a = signed(x.data8(e) as i8 as i16 as u16, m.u8(x.b, f::FLIP) != 0);
        let c = signed(x.data8(p) as i8 as i16 as u16, x.u8(f::FLIP) != 0);
        let d = a.wrapping_sub(c).wrapping_add(m.u16(x.b, 0x8)).wrapping_sub(x.u16(0x8));
        let reach = x.data8(e + 1).wrapping_add(x.data8(p + 1)) as u16;
        if d.wrapping_add(reach) > reach.wrapping_mul(2) {
            return false;
        }
        if !with_z {
            return true;
        }
        let d = (x.data8(e + 2) as i8 as i16 as u16).wrapping_add(m.u16(x.b, 0xc));
        let q = (x.data8(p + 2) as i8 as i16 as u16).wrapping_add(x.u16(0xc));
        let reach = x.data8(e + 3).wrapping_add(x.data8(p + 3)) as u16;
        d.wrapping_sub(q).wrapping_add(reach) <= reach.wrapping_mul(2)
    }

    fn frame_of<R: Bench>(x: &mut Cx<'_, R>, m: Obj) -> u32 {
        let boxes = (m.u8(x.b, 0x2e) as u32) << 2;
        let table = rd32(x.b, FRAMES + boxes);
        table.wrapping_add((m.u8(x.b, 0x38) as u32) << 3)
    }

    /// `0x5094a`: the guardian engages when a monster touches its box (depth and width).
    fn engage_test<R: Bench>(x: &mut Cx<'_, R>) {
        for (first, count) in [(0x299eu32, 22u32), (0x3a1e, 3)] {
            let mut a1 = 0xff8000 + first;
            for _ in 0..count {
                let m = Obj(a1);
                a1 += 0xc0;
                if m.u8(x.b, 0) == 0 || m.u8(x.b, 0x38) == 0 {
                    continue;
                }
                let frame = frame_of(x, m);
                if overlap(x, m, frame, BOX_ENGAGE, false) {
                    x.set16(0x5c, m.0 as u16);
                    x.set16(0x4, 0x206);
                    x.set16(0x6, 0);
                    return;
                }
            }
        }
    }

    /// `0x509ae`: during the leap, hurts all the monsters that the guardian's box touches.
    fn strike<R: Bench>(x: &mut Cx<'_, R>) {
        let owner = x.linked(f::OWNER);
        for (first, count) in [(0x299eu32, 22u32), (0x3a1e, 3)] {
            let mut a1 = 0xff8000 + first;
            for _ in 0..count {
                let m = Obj(a1);
                a1 += 0xc0;
                if m.u8(x.b, 0) != 1 || m.u32(x.b, 0x94) >> 17 & 1 != 0 || m.u8(x.b, 0x38) == 0 || m.u8(x.b, 0x53) != 0 {
                    continue;
                }
                let frame = frame_of(x, m);
                if overlap(x, m, frame, BOX_STRIKE, true) {
                    hurt(x, owner, m);
                }
            }
        }
    }

    fn hurt<R: Bench>(x: &mut Cx<'_, R>, owner: Obj, m: Obj) {
        m.set_u32(x.b, 0x4, 0x0400_0000);
        m.set_u8(x.b, 0x49, 8);
        m.set_u8(x.b, 0, 2);
        let flip = x.u8(f::FLIP);
        m.set_u8(x.b, 0x4a, flip);
        m.set_u8(x.b, 0x56, 8);
        let player = x.u16(f::OWNER);
        m.set_u16(x.b, 0x54, player);
        let mut damage = owner.u8(x.b, 0x109) as u16;
        if damage == 0 {
            damage = owner.u8(x.b, 0x10a) as u16;
        }
        let hp = m.u16(x.b, 0x62);
        if hp >= damage {
            m.set_u16(x.b, 0x62, hp - damage);
            return;
        }
        m.set_u16(x.b, 0x62, hp.wrapping_sub(damage));
        m.set_u32(x.b, 0x4, 0x0400_0000);
        m.set_u16(x.b, 0x62, 0);
        if m.u8(x.b, 0x28) == 4 && m.u8(x.b, 0x93) != 0 {
            owner.set_u8(x.b, 0x18a, 1);
            let v = m.u8(x.b, 0x9a);
            x.set_a5_u8(0x4f, v);
        }
    }
}

handlers! {
    0x50348 "peff_41" #[without] guardian::run,
}
