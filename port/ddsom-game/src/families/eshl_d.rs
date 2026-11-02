//! Enemy shots and attacks (ESHL class), types 64 to 72 (end of table 0x39b96). See `docs/families/eshl_c.md`.
//!
//! Same framework as `eshl.rs`: one `mod` per type, `run` is called by the dispatch with the current object; `+4` is
//! the main state, `+5` / `+6` the substates (even bytes).

use crate::families::cx::Bench;
use super::cx::{handlers, Cx};
use super::shell::*;
use crate::objects::Obj;
use crate::objects::pools;
use crate::objects::Current;
use crate::machine::Machine;

/// Allocates an ESHL object; `None` if the pool is full. `subq.w #1,(off,A0)`: true when the counter reaches zero.
fn count_down<R: Bench>(x: &mut Cx<'_, R>, off: u32) -> bool {
    x.add16(off, 0xffff);
    x.u16(off) == 0
}

// ---------------------------------------------------------------------------------------------
// Type 64: flame columns of the warrior (0xa6efa)
// ---------------------------------------------------------------------------------------------

/// Shot 64 (0xa6efa): flames placed by a creator (`+0x5c`, an enemy or another flame). Variant 0 counts its generations
/// in `+0x2f`: the fourth falls back in an arc and goes out on the ground (area damage 0x2860), the others light up in
/// place; at the end of their time (`+0x42`), they create three flames: one of variant 0 (next generation) and two
/// columns (variants 1 and 2) that rise or descend by 8 pixels per generation (`+0x46`: sound every third one).
mod flame64 {
    use super::*;

    const ANIMS: u32 = 0x1713f8;
    const CREATOR: u32 = 0x5c;
    const GENERATION: u32 = 0x2f;
    const TIMER: u32 = 0x42;
    const SOUND_COUNT: u32 = 0x46;
    const LAST_GENERATION: u8 = 4;
    const SFX_FLAME: crate::sound::Sound = crate::sound::CMD_330;
    const SFX_LAND: crate::sound::Sound = crate::sound::CMD_32F;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => {
                if x.u16(CREATOR) != 0 {
                    init(x);
                }
            }
            2 => {
                match x.u8(0x3) {
                    0 => active_0(x),
                    1 => column(x, 1),
                    _ => column(x, 2),
                }
                x.reinsert();
            }
            4 => x.add8(0x4, 2),
            _ => x.free_eshl(),
        }
    }

    fn shape<R: Bench>(x: &mut Cx<'_, R>) {
        x.set8(f::BOX_SET, 7);
        x.set8(f::ATTACK_ID, 0x26);
    }

    fn init<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(0x3) == 0 {
            let g = x.u8(GENERATION).wrapping_sub(1);
            x.set8(GENERATION, g);
            if g == 0 {
                x.set8(0x4, 4);
                return;
            }
            x.add8(0x4, 2);
            if g == LAST_GENERATION {
                x.set8(SOUND_COUNT, 3);
                shape(x);
                follow(x);
                x.set16(0x18, 0xfb40);
                x.anim_start(ANIMS, 0);
            } else {
                follow(x);
                let dx = if x.u8(f::FLIP) != 0 { 0x20 } else { 0xffe0 };
                x.add16(0x8, dx);
                x.set16(TIMER, 12);
                shape(x);
                x.anim_start(ANIMS, 2);
            }
            return;
        }
        x.add8(0x4, 2);
        let n = x.u8(SOUND_COUNT).wrapping_sub(1);
        x.set8(SOUND_COUNT, n);
        if n == 0 {
            crate::sound::play(x.b, SFX_FLAME, x.o);
            x.set8(SOUND_COUNT, 3);
        }
        follow(x);
        let dz = if x.u8(0x3) == 1 { 8 } else { 0xfff8 };
        x.add16(0x10, dz);
        let z = x.u16(0x10) as i16;
        if z <= 0x170 || z > 0x1af {
            x.set8(0x4, 4);
        }
        x.set16(TIMER, 12);
        shape(x);
        x.anim_start(ANIMS, 2);
    }

    /// Position (longs) and orientation of the creator, one pixel in front of it.
    fn follow<R: Bench>(x: &mut Cx<'_, R>) {
        let creator = x.linked(CREATOR);
        for off in [0x8, 0x10, 0xc] {
            let v = creator.u32(x.b, off);
            x.set32(off, v);
        }
        let flip = creator.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        x.add16(0x10, 0xffff);
    }

    fn active_0<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(GENERATION) != LAST_GENERATION {
            if count_down(x, TIMER) {
                spawn_all(x);
            }
            finish(x);
            return;
        }
        match x.u8(0x5) {
            0 => {
                if x.anim_step() {
                    x.add8(0x5, 2);
                    shape(x);
                    crate::sound::play(x.b, SFX_LAND, x.o);
                    x.anim_start(ANIMS, 1);
                }
            }
            2 => {
                if x.move_xyz().gt() {
                    x.anim_step();
                    return;
                }
                x.set16(0xc, 0);
                crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::B, 8);
                x.add8(0x5, 2);
                shape(x);
                x.set16(TIMER, 12);
                x.anim_start(ANIMS, 2);
                crate::sound::play(x.b, SFX_FLAME, x.o);
            }
            _ => {
                if count_down(x, TIMER) {
                    spawn_all(x);
                }
                finish(x);
            }
        }
    }

    /// Column: at the end of its time, creates the next flame of its variant.
    fn column<R: Bench>(x: &mut Cx<'_, R>, variant: u8) {
        if count_down(x, TIMER) {
            spawn(x, variant);
        }
        finish(x);
    }

    fn finish<R: Bench>(x: &mut Cx<'_, R>) {
        if x.anim_step() {
            x.add8(0x4, 2);
        }
    }

    fn spawn_all<R: Bench>(x: &mut Cx<'_, R>) {
        spawn(x, 0);
        spawn(x, 1);
        spawn(x, 2);
    }

    /// Child flame of variant `variant` (0xa6e8e, 0xa6eb2, 0xa6ed6): variant 0 inherits the generation, the columns the
    /// sound counter.
    fn spawn<R: Bench>(x: &mut Cx<'_, R>, variant: u8) {
        let Some(child) = x.alloc(&pools::ESHL) else { return };
        child.set_u8(x.b, 0x0, 1);
        child.set_u8(x.b, 0x2, 64);
        child.set_u8(x.b, 0x3, variant);
        if variant == 0 {
            let g = x.u8(GENERATION);
            child.set_u8(x.b, GENERATION, g);
            child.set_u16(x.b, CREATOR, x.o.0 as u16);
        } else {
            child.set_u16(x.b, CREATOR, x.o.0 as u16);
            let n = x.u8(SOUND_COUNT);
            child.set_u8(x.b, SOUND_COUNT, n);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 65: rain of stones (0xa72c4)
// ---------------------------------------------------------------------------------------------

/// Shot 65 (0xa72c4): stone that falls from the top of the screen at a randomly drawn position (two draws: X within the
/// screen, depth within a band of 0x50), after a wait read from a table by rank `+0x2f`; on the ground it triggers area
/// damage (0x2860). Variant 0: impact burst (0xab7d8) and disappearance; variant 1: impact animation until its end.
mod rocks65 {
    use super::*;

    /// Per variant: animation set, attack identifier, waits per rank (bytes, program ROM).
    const ANIMS: [u32; 2] = [0x16ea18, 0x171574];
    const ATTACK: [u8; 2] = [0x25, 0x38];
    const DELAYS: [u32; 2] = [0xa739a, 0xa74b6];
    const TIMER: u32 = 0x42;
    const RANK: u32 = 0x2f;
    /// Corner of the screen (player 1, or player 2 if `A5+0x36`).
    const SCREEN: u32 = 0x4fe;
    const SCREEN_P2: u32 = 0x54e;
    const SFX: [crate::sound::Sound; 2] = [crate::sound::CMD_34D_44D, crate::sound::CMD_330];

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        if suspended(x) {
            return;
        }
        let v = (x.u8(0x3) != 0) as usize;
        // variant 1 has one more state (end of animation) before the release
        let state = x.u8(0x4);
        match (state, v) {
            (0, _) => init(x, v),
            (2, _) => fall(x, v),
            (4, 1) => {
                if x.anim_step() {
                    x.add8(0x4, 2);
                }
            }
            (4, 0) | (6, 1) => x.add8(0x4, 2),
            _ => x.free_eshl(),
        }
    }

    fn init<R: Bench>(x: &mut Cx<'_, R>, v: usize) {
        x.add8(0x4, 2);
        x.set8(0x5, 0);
        x.set8(f::BOX_SET, 7);
        x.set8(f::ATTACK_ID, ATTACK[v]);
        let screen = if x.a5_u8(0x36) != 0 { SCREEN_P2 } else { SCREEN };
        x.set16(0xc, 0x100);
        let r = random(x) & 0x1ff;
        let dx = if r > 0x180 { (r >> 1) + 0x40 } else { r };
        let left = x.a5_u16(screen + 8);
        x.set16(0x8, dx.wrapping_add(left));
        let r = random(x) & 0x7f;
        let dz = if r > 0x50 { (r >> 1) + 0x10 } else { r } + 0x10;
        let top = x.a5_u16(screen + 0xc);
        x.set16(0x10, dz.wrapping_add(top));
        x.set16(0x18, 0xfe00);
        x.set16(0x1a, 0xffe0);
        x.set32(0x14, 0);
        x.set32(0x1c, 0);
        let rank = (x.u8(RANK) & 0x1f) as u32;
        let delay = x.rom8(DELAYS[v] + rank);
        x.set16(TIMER, delay as u16);
        x.anim_start(ANIMS[v], 0);
    }

    /// Pseudo-random draw (low word of D0).
    fn random<R: Bench>(x: &mut Cx<'_, R>) -> u16 {
        x.random()
    }

    fn fall<R: Bench>(x: &mut Cx<'_, R>, v: usize) {
        if x.u16(TIMER) as i16 > 0 {
            x.add16(TIMER, 0xffff);
            return;
        }
        x.anim_step();
        x.move_z();
        if x.u16(0xc) as i16 > 0 {
            x.keep_on_screen(16);
            x.reinsert();
            return;
        }
        x.set16(0xc, 0);
        x.add8(0x4, 2);
        if v == 0 {
            crate::families::effects_creators::debris(x.b, x.o, 3);
        } else {
            x.anim_start(ANIMS[1], 1);
        }
        crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::B, 8);
        crate::sound::play(x.b, SFX[v], x.o);
    }
}

// ---------------------------------------------------------------------------------------------
// Type 66: bursts held then dropped (0xa75c6)
// ---------------------------------------------------------------------------------------------

/// Shot 66 (0xa75c6): burst stuck to its carrier (`+0x46`, word) at an offset read from a table by variant and rank
/// `+0x2f`, which approaches it blinking for 8 frames, then is thrown upward, falls back (sound, area damage 0x2824),
/// waits 16 frames and blinks 16 frames before disappearing. Without a carrier, it restarts from initialization.
mod shard66 {
    use super::*;

    const ANIMS: u32 = 0x17b9d6;
    /// Animation per variant, at creation then at impact (bytes, program ROM).
    const START_ANIMS: u32 = 0xa760e;
    const LAND_ANIMS: u32 = 0xa777a;
    /// Offsets (X, depth) per variant, four bytes per rank (words, in the clear).
    const OFFSETS: [u32; 6] = [0xa7834, 0xa7834, 0xa7854, 0xa7870, 0xa7890, 0xa78a8];
    const CARRIER: u32 = 0x46;
    const TIMER: u32 = 0x42;
    const RANK: u32 = 0x2f;
    const SFX_LAND: crate::sound::Sound = crate::sound::CMD_387_487;
    /// States `+4.w` of the carrier in which the burst stays held.
    const HELD_STATE: u16 = 0x204;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => init(x),
            2 => active(x),
            4 => x.add8(0x4, 2),
            _ => x.free_eshl(),
        }
    }

    fn init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        x.set8(0x5, 0);
        x.set8(f::BOX_SET, 0x2c);
        x.set8(f::ATTACK_ID, 0x31);
        let v = x.u8(0x3) as usize;
        let table = OFFSETS.get(v).copied().unwrap_or(OFFSETS[0]);
        let row = table + 4 * x.u8(RANK) as u32;
        let dx = x.data16(row);
        let dx = if x.u8(f::FLIP) != 0 { dx.wrapping_neg() } else { dx };
        x.set16(0x14, dx);
        let dz = x.data16(row + 2);
        x.set16(0x1c, dz);
        x.set16(0x18, 0x60);
        x.set16(TIMER, 8);
        let n = x.rom8(START_ANIMS + v as u32) as u32;
        x.anim_start(ANIMS, n as u16);
    }

    fn active<R: Bench>(x: &mut Cx<'_, R>) {
        let carrier = x.u16(CARRIER);
        if carrier as i16 >= 0 {
            // no carrier: the original restarts from initialization (substate 4)
            x.set16(0x4, 4);
            return;
        }
        let carrier = Obj(carrier as i16 as i32 as u32 & 0xff_ffff);
        match x.u8(0x5) {
            0 => {
                for off in [0x8, 0xc, 0x10] {
                    let v = carrier.u16(x.b, off);
                    x.set16(off, v);
                }
                let flip = carrier.u8(x.b, f::FLIP);
                x.set8(f::FLIP, flip);
                for (pos, speed) in [(0x8, 0x14), (0x10, 0x1c), (0xc, 0x18)] {
                    let d = x.u16(speed);
                    x.add16(pos, d);
                }
                let t = x.u16(TIMER);
                if t as i16 > 0 {
                    let t = t - 1;
                    x.set16(TIMER, t);
                    x.add16(0x10, t);
                    return blink(x, t & 1 != 0);
                }
                // (test of the original on the burst itself: always false in substate 0)
                if x.u16(0x4) == HELD_STATE && (x.u8(0x6) as i8) < 6 {
                    return blink(x, false);
                }
                x.add8(0x5, 2);
                x.set32(0x14, 0);
                x.set32(0x18, 0);
                x.set32(0x1c, 0);
                x.set16(0x18, 0xf800);
                x.set16(0x1a, 0xff00);
            }
            2 => {
                x.move_z();
                if x.u16(0xc) as i16 <= 0 {
                    x.set16(0xc, 0);
                    x.add8(0x5, 2);
                    x.set8(f::ATTACK_ID, 0x32);
                    x.set16(TIMER, 16);
                    let v = x.u8(0x3) as u32;
                    let n = x.rom8(LAND_ANIMS + v) as u32;
                    x.anim_start(ANIMS, n as u16);
                    crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::A, 4);
                    crate::sound::play(x.b, SFX_LAND, x.o);
                }
                blink(x, false);
            }
            4 => {
                if count_down_signed(x) {
                    x.set16(TIMER, 16);
                    x.add8(0x5, 2);
                } else {
                    blink(x, false);
                }
            }
            _ => {
                let t = x.u16(TIMER);
                if t as i16 <= 0 {
                    x.add8(0x4, 2);
                    return;
                }
                let t = t - 1;
                x.set16(TIMER, t);
                blink(x, t & 1 != 0);
            }
        }
    }

    /// `subq.w #1,+0x42 ; bgt`: true when the counter is no longer positive.
    fn count_down_signed<R: Bench>(x: &mut Cx<'_, R>) -> bool {
        x.add16(TIMER, 0xffff);
        x.u16(TIMER) as i16 <= 0
    }

    /// Common end: visibility (bit 1 of `+1`: hidden), terrain, on-screen restraint, sort.
    fn blink<R: Bench>(x: &mut Cx<'_, R>, hidden: bool) {
        x.set_bit(0x1, 1, hidden);
        x.push_terrain();
        x.keep_on_screen(16);
        x.reinsert();
    }
}

// ---------------------------------------------------------------------------------------------
// Type 67: thrown barrel (0xa78c2)
// ---------------------------------------------------------------------------------------------

/// Shot 67 (0xa78c2): object thrown in an arc toward the owner (`+0x50`): the vertical speed is chosen to reach it (12
/// less per step of the horizontal distance, bounded to 16..256). It spins on itself (orientation flipped every 4
/// frames); on the ground it slides for 30 frames, slowing down (effect 0x1172e every 6 frames); hit or parried, it
/// bounces three times (effects 12, 13, 8). It blinks during its last 30 frames and disappears off the screen.
mod barrel67 {
    use super::*;

    const ANIMS: u32 = 0x171a52;
    const SPEED_TABLE: u32 = 53;
    const TIMER: u32 = 0x58;
    const DUST_TIMER: u32 = 0x5a;
    const SPIN_TIMER: u32 = 0x5e;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        if suspended(x) {
            return;
        }
        match x.u8(0x4) {
            0 => init(x),
            2 => active(x),
            _ => x.free_eshl(),
        }
        if x.u8(0x0) != 0 {
            x.reinsert();
        }
    }

    fn init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        x.set8(f::BOX_SET, 7);
        let dir = if x.u8(f::FLIP) != 0 { 8 } else { 24 };
        x.set8(0x2d, dir);
        x.select_speed(&crate::objects::motion::SPEEDS_B, SPEED_TABLE as u16);
        let step = ((x.u16(0x14) as i16).unsigned_abs() as i32) << 16 >> 8;
        let owner = x.linked(f::OWNER);
        let mut distance = (owner.u32(x.b, 0x8).wrapping_sub(x.u32(0x8)) as i32).wrapping_abs();
        let mut vz = (x.u32(0xc) as i32 >> 8) as i16 >> 6;
        loop {
            distance = distance.wrapping_sub(step);
            if distance <= 0 {
                break;
            }
            vz = vz.wrapping_sub(12);
        }
        let vz = vz.clamp(16, 256);
        x.set16(0x1a, (vz as u16).wrapping_neg());
        crate::families::enemy_lib::start(x, ANIMS, 21);
    }

    /// 0xa7af4: flips the orientation every 4 frames.
    fn spin<R: Bench>(x: &mut Cx<'_, R>) {
        x.add16(SPIN_TIMER, 0xffff);
        if x.u16(SPIN_TIMER) as i16 > 0 {
            return;
        }
        x.set16(SPIN_TIMER, 4);
        let flip = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
    }

    fn impact<R: Bench>(x: &mut Cx<'_, R>, kind: u32) {
        crate::ai_aids::debris_set(x.b, x.o, kind as u16);
        // Original glitch: the exhausted `dbf` counter stays in D5 (`dispatch::Leftovers`)
        *x.c.d5() = 0xffff;
    }

    /// Movement in X and in height; true when the object touches the ground (height reset to 0).
    fn lands<R: Bench>(x: &mut Cx<'_, R>) -> bool {
        x.move_xz();
        if x.u16(0xc) as i16 > 0 {
            return false;
        }
        x.set16(0xc, 0);
        true
    }

    fn active<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x5) {
            0 => {
                if crate::ai_aids::outside_band(x.b, x.o, 0xffc0, 448) {
                    return x.free_eshl();
                }
                if x.u8(f::HIT_FLAGS) & 0x28 != 0 {
                    x.set8(0x5, 4);
                    let vx = (x.u16(0x14) as i16 >> 3) as u16;
                    x.set16(0x14, vx.wrapping_neg());
                    x.set16(0x18, 0x200);
                    x.set16(0x1a, 0xffe0);
                    return;
                }
                spin(x);
                if lands(x) {
                    x.set8(0x5, 2);
                    x.set16(TIMER, 30);
                    x.set8(0x0, 2);
                    let friction = (x.u16(0x14) as i16 >> 5) as u16;
                    x.set16(0x16, friction.wrapping_neg());
                }
            }
            2 => {
                spin(x);
                x.move_x();
                if count_down_one(x, TIMER) {
                    x.set8(0x5, 6);
                    x.set16(TIMER, 60);
                } else if count_down_one(x, DUST_TIMER) {
                    x.set16(DUST_TIMER, 6);
                    impact(x, 9);
                }
            }
            4 => match x.u8(0x6) {
                0 => {
                    spin(x);
                    x.move_xz();
                    if x.u16(0x18) as i16 <= 0 {
                        x.add8(0x6, 2);
                        x.set8(0x0, 2);
                        x.set16(0x1a, 0xffa0);
                    }
                }
                2 => {
                    spin(x);
                    if lands(x) {
                        x.add8(0x6, 2);
                        x.set16(0x18, 0x300);
                        x.set16(0x1a, 0xffc0);
                        impact(x, 12);
                    }
                }
                4 => {
                    if lands(x) {
                        x.add8(0x6, 2);
                        x.set16(0x18, 0x200);
                        x.set16(0x1a, 0xffc0);
                        impact(x, 13);
                    }
                }
                _ => {
                    if lands(x) {
                        x.set8(0x5, 6);
                        x.set16(TIMER, 60);
                        impact(x, 8);
                    }
                }
            },
            _ => {
                if count_down_one(x, TIMER) {
                    return x.free_eshl();
                }
                if x.u16(TIMER) as i16 <= 30 {
                    let v = x.u8(0x1) ^ 2;
                    x.set8(0x1, v);
                }
            }
        }
    }

    /// `subq.w #1 ; ble`: true when the counter is no longer positive.
    fn count_down_one<R: Bench>(x: &mut Cx<'_, R>, off: u32) -> bool {
        x.add16(off, 0xffff);
        x.u16(off) as i16 <= 0
    }
}

// ---------------------------------------------------------------------------------------------
// Type 68: lifted stones (0xa7b76)
// ---------------------------------------------------------------------------------------------

/// Shot 68 (0xa7b76): stone that rises from the ground (foreground) above an abscissa read by rank `+0x2f`, then stays
/// in the air as long as its carrier (`+0x46`) is not in state 0x204 beyond substate 6; once dropped, it flies off
/// (speed table 31, direction by rank) with an animation that depends on its height on the screen, and disappears when
/// passing below the top of the screen.
mod stone68 {
    use super::*;

    const ANIMS: u32 = 0x17b9d6;
    /// Abscissa per rank (words, program ROM), direction per rank (bytes, program ROM).
    const COLUMNS: u32 = 0xa7be4;
    const DIRECTIONS: u32 = 0xa7c7c;
    /// Animations according to the height on the screen: count then pairs (animation, threshold) (words, in the clear).
    const HEIGHT_ANIMS: u32 = 0xa7d50;
    const CARRIER: u32 = 0x46;
    const RANK: u32 = 0x2f;
    const TIMER: u32 = 0x42;
    const SPEED_TABLE: u32 = 31;
    const SFX_RELEASE: crate::sound::Sound = crate::sound::CMD_387_487;
    const SCREEN_TOP: u32 = 0x50a;
    const SCREEN_TOP_P2: u32 = 0x55a;
    const RELEASE_STATE: u32 = 0x0204_0200;
    const RISE_HEIGHT: u16 = 0x20;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(0x5, 0);
                x.set8(f::BOX_SET, 0x2c);
                x.set8(f::ATTACK_ID, 0x33);
                x.set16(0x10, 0x1c0);
                x.set16(0xc, 0xff90);
                let rank = x.u8(RANK) as u32;
                let column = x.rom16(COLUMNS + 2 * rank);
                x.set16(0x8, column);
                x.set16(TIMER, 8);
                x.anim_start(ANIMS, 0x2a);
            }
            2 => active(x),
            4 => x.add8(0x4, 2),
            _ => x.free_eshl(),
        }
    }

    fn active<R: Bench>(x: &mut Cx<'_, R>) {
        let carrier = x.u16(CARRIER);
        if carrier as i16 >= 0 {
            x.set16(0x4, 4);
            return;
        }
        let carrier = Obj(carrier as i16 as i32 as u32 & 0xff_ffff);
        match x.u8(0x5) {
            0 => {
                let z = x.u16(0xc);
                let z = z.wrapping_sub((z.wrapping_sub(RISE_HEIGHT) as i16 >> 3) as u16);
                x.set16(0xc, z);
                if (z as i16) < RISE_HEIGHT as i16 {
                    x.set_bit(0x1, 1, false);
                    x.reinsert_foreground();
                    return;
                }
                x.set16(0xc, RISE_HEIGHT);
                x.add8(0x5, 2);
                x.set32(0x14, 0);
                x.set32(0x1c, 0);
                x.set32(0x18, 0);
                let rank = (x.u8(RANK) & 7) as u32;
                let dir = x.rom8(DIRECTIONS + rank);
                x.set8(0x2d, dir);
                x.select_speed(&crate::objects::motion::SPEEDS_A, SPEED_TABLE as u16);
                x.unlink();
            }
            2 => {
                let held = carrier.u8(x.b, 0x0) != 0 && carrier.u32(x.b, 0x4) & !0xff == RELEASE_STATE;
                if !(held && (carrier.u8(x.b, 0x7) as i8) < 6) {
                    x.add8(0x5, 2);
                    let rank = x.u8(RANK);
                    if rank == 0 || rank == 5 {
                        crate::sound::play(x.b, SFX_RELEASE, x.o);
                    }
                    return;
                }
            }
            _ => {
                height_anim(x);
                x.move_xy();
                let top = x.a5_u16(SCREEN_TOP);
                if (x.u16(0x10).wrapping_sub(top) as i16) <= 0 {
                    x.add8(0x4, 2);
                    return;
                }
            }
        }
        x.set_bit(0x1, 1, false);
        x.reinsert();
    }

    /// 0xa7d20: animation according to the height on the screen (increasing thresholds, the last one by default).
    fn height_anim<R: Bench>(x: &mut Cx<'_, R>) {
        let top = if x.a5_u8(0x36) != 0 { SCREEN_TOP_P2 } else { SCREEN_TOP };
        let h = x.u16(0x10).wrapping_sub(x.a5_u16(top)) as i16;
        let count = x.data16(HEIGHT_ANIMS);
        let mut anim = 0;
        for k in 0..=count as u32 {
            anim = x.data16(HEIGHT_ANIMS + 2 + 4 * k);
            if h < x.data16(HEIGHT_ANIMS + 4 + 4 * k) as i16 {
                break;
            }
        }
        x.anim_start(ANIMS, (anim as u32) as u16);
    }
}

// ---------------------------------------------------------------------------------------------
// Type 69: bouncing stone (0xa7e30)
// ---------------------------------------------------------------------------------------------

/// Shot 69 (0xa7e30): stone thrown in front of the creator (48 pixels, height 32) after a wait read by rank `+0x2f`; it
/// bounces at each contact with the ground (sound) and disappears when it leaves the screen (16 pixels to the left, 400
/// to the right). `+0x42` (byte, set by the creator) = 11: stronger hit (0x36).
mod bouncer69 {
    use super::*;

    const ANIMS: u32 = 0x17b9d6;
    /// Per rank: wait then animation (words, program ROM).
    const RANKS: u32 = 0xa7eb4;
    const RANK: u32 = 0x2f;
    const TIMER: u32 = 0x42;
    const SFX_BOUNCE: crate::sound::Sound = crate::sound::CMD_385_485;
    const CAMERA_X: u32 = 0x506;
    const CAMERA2_X: u32 = 0x556;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => init(x),
            2 => active(x),
            4 => x.add8(0x4, 2),
            _ => x.free_eshl(),
        }
    }

    fn init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        x.set8(0x5, 0);
        x.set8(f::BOX_SET, 0x2c);
        let strong = x.u8(TIMER) == 11;
        x.set8(f::ATTACK_ID, if strong { 0x36 } else { 0x33 });
        let dx: u16 = if x.u8(f::FLIP) != 0 { 48 } else { 48u16.wrapping_neg() };
        x.add16(0x8, dx);
        x.set16(0xc, 32);
        bounce(x);
        let rank = x.u8(RANK) as u32;
        let row = RANKS + 4 * rank;
        let delay = x.rom16(row);
        x.set16(TIMER, delay);
        let anim = x.rom16(row + 2);
        x.anim_start(ANIMS, (anim as u32) as u16);
    }

    /// 0xa7ed4: speeds of a leap (forward), sound.
    fn bounce<R: Bench>(x: &mut Cx<'_, R>) {
        let vx: u16 = if x.u8(f::FLIP) != 0 { 0x400 } else { 0x400u16.wrapping_neg() };
        x.set16(0x14, vx);
        x.set16(0x16, 0);
        x.set16(0x18, 0x800);
        x.set16(0x1a, 0xff80);
        x.set32(0x1c, 0);
        crate::sound::play(x.b, SFX_BOUNCE, x.o);
    }

    fn active<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(0x5) == 0 {
            x.add16(TIMER, 0xffff);
            if x.u16(TIMER) as i16 > 0 {
                return;
            }
            x.add8(0x5, 2);
        }
        x.anim_step();
        x.move_xz();
        if x.u16(0xc) as i16 <= 0 {
            x.set16(0xc, 0);
            bounce(x);
        }
        let camera = if x.a5_u8(0x36) != 0 { CAMERA2_X } else { CAMERA_X };
        let dx = x.u16(0x8).wrapping_sub(x.a5_u16(camera)) as i16;
        if dx <= -16 || dx > 400 {
            x.add8(0x4, 2);
            return;
        }
        x.set_bit(0x1, 1, false);
        x.reinsert();
    }
}

// ---------------------------------------------------------------------------------------------
// Type 70: gratings that rise according to the scroll (0xa801a)
// ---------------------------------------------------------------------------------------------

/// Shot 70 (0xa801a): dangerous scenery element linked to a SET (type 7, `+0x5c`) that signals it as long as the screen
/// has not reached a given abscissa (0x500 for variant 0, 0x650 for variant 1); past this point, the SET is notified
/// (`+0x2f` = 0) and the object disappears. Variant 1 alternates wait (`+0x42`, 4 frames) and animation.
mod gate70 {
    use super::*;

    const ANIMS: u32 = 0x1717be;
    const SET: u32 = 0x5c;
    const TIMER: u32 = 0x42;
    const CAMERA_X: u32 = 0x506;
    const LIMITS: [u16; 2] = [0x500, 0x650];

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        if suspended(x) {
            return;
        }
        let variant = x.u8(0x3);
        match x.u8(0x4) {
            0 => init(x, variant),
            2 => {
                let limit = LIMITS[(variant != 0) as usize];
                if (x.a5_u16(CAMERA_X) as i16) < limit as i16 {
                    if variant == 0 {
                        x.anim_step();
                    } else {
                        wait_and_animate(x);
                    }
                    return;
                }
                let set = x.linked(SET);
                set.set_u8(x.b, 0x2f, 0);
                x.add8(0x4, 2);
                if variant == 0 {
                    x.anim_step();
                }
            }
            _ => x.free_eshl(),
        }
    }

    fn init<R: Bench>(x: &mut Cx<'_, R>, variant: u8) {
        x.add8(0x4, 2);
        if let Some(set) = x.alloc(&pools::SET) {
            set.set_u8(x.b, 0x0, 1);
            set.set_u8(x.b, 0x2, 7);
            set.set_u8(x.b, 0x3, if variant == 0 { 0x2c } else { 0x2e });
            set.set_u8(x.b, 0x2f, 1);
            x.set16(SET, set.0 as u16);
        }
        if variant != 0 {
            x.set16(TIMER, 4);
        }
        x.set8(f::BOX_SET, 7);
        x.set8(f::ATTACK_ID, if variant == 0 { 0x34 } else { 0x35 });
        x.anim_start(ANIMS, ((variant != 0) as u32) as u16);
        x.reinsert();
    }

    fn wait_and_animate<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(0x5) == 0 {
            x.add16(TIMER, 0xffff);
            if x.u16(TIMER) as i16 >= 0 {
                return;
            }
            x.add8(0x5, 2);
            x.anim_start(ANIMS, 1);
        } else if x.anim_step() {
            x.add8(0x5, 0xfe);
            x.set16(TIMER, 4);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 71: rolling balls (0xa81ca)
// ---------------------------------------------------------------------------------------------

/// Shot 71 (0xa81ca): ball launched by an enemy (`+0x46`). Variant 0: held until `+0x58` releases it (disappears if the
/// launcher goes to state 4 other than 0x204), leaves in an arc (speed table 33, direction according to the
/// orientation), rolls on the ground; when hit (`+0x33`), it creates in front of it a ball of variant 1 if it is on the
/// screen. Variant 1: rolls until the end of its animation.
mod ball71 {
    use super::*;

    const ANIMS: u32 = 0x16f58c;
    const OWNER: u32 = 0x46;
    const RELEASE: u32 = 0x58;
    const HIT: u32 = 0x33;
    const SPEED_TABLE: u32 = 33;
    const SFX_THROW: crate::sound::Sound = crate::sound::CMD_32F;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        if suspended(x) {
            return;
        }
        let variant = x.u8(0x3);
        match x.u8(0x4) {
            0 => init(x, variant),
            2 => match (variant, x.u8(0x5)) {
                (0, 0) => held(x),
                (0, 2) => {
                    x.move_xyz();
                    if x.u16(0xc) as i16 > 0 {
                        crate::families::enemy_lib::step(x);
                    } else {
                        x.add8(0x5, 2);
                        x.set16(0xc, 0);
                        ai_anim(x, 3);
                    }
                }
                _ => rolling(x),
            },
            _ => x.free_eshl(),
        }
        if x.u8(0x0) != 0 {
            x.reinsert();
        }
    }

    fn ai_anim<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
        crate::families::enemy_lib::start(x, ANIMS, n);
    }

    fn init<R: Bench>(x: &mut Cx<'_, R>, variant: u8) {
        x.add8(0x4, 2);
        x.set8(f::BOX_SET, 7);
        x.set8(f::ATTACK_POWER, 0x14);
        if variant != 0 {
            return ai_anim(x, 3);
        }
        crate::placement::load_effect_palette(x.b, crate::placement::effect_palette::LIGHT);
        let dir = if x.u8(f::FLIP) != 0 { 8 } else { 24 };
        x.set8(0x2d, dir);
        x.select_speed(&crate::objects::motion::SPEEDS_B, SPEED_TABLE as u16);
        x.set16(0x18, 0xfc00);
        ai_anim(x, 0);
    }

    /// Held by the launcher: released by `+0x58`, destroyed if the launcher dies (state 4 other than 0x204).
    fn held<R: Bench>(x: &mut Cx<'_, R>) {
        let owner = x.linked(OWNER);
        let dying = owner.u8(x.b, 0x4) == 4 && owner.u16(x.b, 0x4) != 0x204;
        if dying {
            return x.free_eshl();
        }
        if x.u8(RELEASE) != 0 {
            x.add8(0x5, 2);
            crate::sound::play(x.b, SFX_THROW, x.o);
        }
        crate::families::enemy_lib::step(x);
    }

    fn rolling<R: Bench>(x: &mut Cx<'_, R>) {
        if crate::families::enemy_lib::step(x) {
            return x.free_eshl();
        }
        if x.u8(HIT) == 0 {
            return;
        }
        x.set8(HIT, 0);
        if crate::ai_aids::outside_band(x.b, x.o, 0xfff0, 400) {
            return;
        }
        let Some(ball) = x.alloc(&pools::ESHL) else { return };
        ball.set_u8(x.b, 0x0, 1);
        ball.set_u8(x.b, 0x2, 71);
        ball.set_u8(x.b, 0x3, 1);
        for off in [0x8, 0x10] {
            let v = x.u16(off);
            ball.set_u16(x.b, off, v);
        }
        let flip = x.u8(f::FLIP);
        ball.set_u8(x.b, f::FLIP, flip);
        let dx: u16 = if flip != 0 { 32 } else { 32u16.wrapping_neg() };
        let bx = ball.u16(x.b, 0x8).wrapping_add(dx);
        ball.set_u16(x.b, 0x8, bx);
    }
}

// ---------------------------------------------------------------------------------------------
// Type 72: bursts thrown by a creator (0xa838e)
// ---------------------------------------------------------------------------------------------

/// Shot 72 (0xa838e): burst placed on its creator (`+0x5c`) with an offset and speeds read from a sheet by rank `+0x2f`
/// and variant; it finishes its animation, leaves in an arc, falls back (area damage 0x2860 of strength 40) and
/// disappears at the end of the impact animation. Without a creator, it frees itself.
mod splinter72 {
    use super::*;

    const ANIMS: u32 = 0x1718d8;
    const IMPACT_ANIMS: u32 = 0x16d1d6;
    /// Per rank: address of an 18-byte sheet per variant (longs then words, in the clear).
    const ROWS: u32 = 0xa84f4;
    const CREATOR: u32 = 0x5c;
    const RANK: u32 = 0x2f;
    const SFX_THROW: crate::sound::Sound = crate::sound::CMD_32F;
    const SFX_LAND: crate::sound::Sound = crate::sound::CMD_330;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u16(CREATOR) == 0 {
            x.set32(0x4, 0x0600_0000);
            return;
        }
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                crate::placement::load_effect_palette(x.b, crate::placement::effect_palette::LIGHT);
                place(x);
            }
            2 => {
                match x.u8(0x5) {
                    0 => {
                        if x.anim_step() {
                            x.add8(0x5, 2);
                            x.set8(f::BOX_SET, 7);
                            x.set8(f::ATTACK_ID, 0x37);
                            crate::sound::play(x.b, SFX_THROW, x.o);
                            x.anim_start(ANIMS, 1);
                        }
                    }
                    2 => {
                        if x.move_xyz().gt() {
                            x.anim_step();
                        } else {
                            x.set16(0xc, 0);
                            crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::B, 40);
                            x.add8(0x5, 2);
                            x.set8(f::BOX_SET, 7);
                            x.set8(f::ATTACK_ID, 0x1c);
                            x.anim_start(IMPACT_ANIMS, 0);
                            crate::sound::play(x.b, SFX_LAND, x.o);
                        }
                    }
                    _ => {
                        if x.anim_step() {
                            x.add8(0x4, 2);
                        }
                    }
                }
                x.reinsert();
            }
            _ => x.free_eshl(),
        }
    }

    /// 0xa8478: position (longs) and orientation of the creator, one pixel in front of it, plus the offset and the
    /// speeds of the sheet.
    fn place<R: Bench>(x: &mut Cx<'_, R>) {
        let creator = x.linked(CREATOR);
        for off in [0x8, 0x10, 0xc] {
            let v = creator.u32(x.b, off);
            x.set32(off, v);
        }
        let flip = creator.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        x.add16(0x10, 0xffff);
        let rank = x.u8(RANK) as u32;
        let rows = x.data32(ROWS + 4 * rank);
        let variant = x.u8(0x3) as u32;
        let row = rows + 18 * variant;
        let [dx, dy, dz] = [0, 2, 4].map(|k| x.data16(row + k));
        for (field, k) in [(0x14, 6), (0x18, 8), (0x18, 10), (0x1a, 12), (0x1c, 14), (0x1e, 16)] {
            let v = x.data16(row + k);
            x.set16(field, v);
        }
        x.add16(0x8, dx);
        x.add16(0xc, dy);
        x.add16(0x10, dz);
    }
}

handlers! {
    0xa6efa "eshl_64" flame64::run,
    0xa72c4 "eshl_65" rocks65::run,
    0xa75c6 "eshl_66" shard66::run,
    0xa78c2 "eshl_67" barrel67::run,
    0xa7b76 "eshl_68" stone68::run,
    0xa7e30 "eshl_69" bouncer69::run,
    0xa801a "eshl_70" gate70::run,
    0xa81ca "eshl_71" ball71::run,
    0xa838e "eshl_72" splinter72::run,
}
