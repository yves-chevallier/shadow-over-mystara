//! Enemy shots and attacks (ESHL class), types 40 to 63. See `docs/families/eshl_c.md`.
//!
//! Same framework as `eshl.rs`: one `mod` per type, `run` is called by the dispatch with the current object; `+4` is
//! the main state, `+5` / `+6` / `+7` the substates (even bytes).

use crate::families::cx::Bench;
use super::cx::{handlers, Cx};
use super::shell::*;
use crate::mem::wr32;
use crate::objects::Obj;

use crate::objects::pools;
use crate::objects::Current;
use crate::machine::Machine;

/// Common subroutines of the enemy AI.
mod rt {
}

/// Sounds (area 0xe0836-0xee080).
mod sfx {
    pub const ED186: u32 = 0xed186;
    pub const ED90A: u32 = 0xed90a;
    pub const ED32E: u32 = 0xed32e;
}

/// Word (16-bit RAM address) seen as an object: sign extension, like `movea.w`.
fn obj_of(word: u16) -> Obj {
    Obj(word as i16 as i32 as u32 & 0xff_ffff)
}

/// Starts an animation through the AI routine (`0x103d6`).
fn ai_anim<R: Bench>(x: &mut Cx<'_, R>, set: u32, n: u32) {
    crate::families::enemy_lib::start(x, set, n);
}

/// One animation step through the AI routine; true at the end of the script.
fn ai_step<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    crate::families::enemy_lib::step(x)
}

/// Allocates an ESHL object; `None` if the pool is full. Pseudo-random draw.
fn random<R: Bench>(x: &mut Cx<'_, R>) -> u32 {
    x.random() as u32
}

/// The three 16.16 coordinates (X, Y, Z) of `from`, copied as longs.
fn copy_xyz32<R: Bench>(x: &mut Cx<'_, R>, from: Obj) {
    for off in [0x8, 0x10, 0xc] {
        let v = from.u32(x.b, off);
        x.set32(off, v);
    }
}

/// Same thing in words (integer parts only).
fn copy_xyz16<R: Bench>(x: &mut Cx<'_, R>, from: Obj) {
    for off in [0x8, 0x10, 0xc] {
        let v = from.u16(x.b, off);
        x.set16(off, v);
    }
}

/// Orientation copied from another object.
fn copy_flip<R: Bench>(x: &mut Cx<'_, R>, from: Obj) -> u8 {
    let flip = from.u8(x.b, f::FLIP);
    x.set8(f::FLIP, flip);
    flip
}

/// Opposite of a word.
fn neg(v: u16) -> u16 {
    v.wrapping_neg()
}

/// Byte seen as signed.
fn s8(v: u8) -> i8 {
    v as i8
}

/// Word seen as signed.
fn s16(v: u16) -> i16 {
    v as i16
}

// ---------------------------------------------------------------------------------------------
// Types 40 and 41: volley of projectiles launched by a creator (the creator is in `+0x5c`)
// ---------------------------------------------------------------------------------------------

/// Shot 40 (0xa31e4): projectile that appears at the position of its creator (`+0x5c`; variants 0 to 2: X offset of 0 /
/// -10 / +10 pixels), chooses an animation at random from a 16-byte table and lasts 90 frames (`+0x42`), hidden every
/// other frame according to the parity of its variant. It disappears when the owner (`+0x50`) is in state 0x406.
mod shot40 {
    use super::*;

    const ANIMS: u32 = 0x16ff94;
    /// Animation drawn at random (16 bytes).
    const RANDOM_ANIMS: u32 = 0xa329e;
    /// Value of `+0x2b` (palette (h)) by `+0x47`.
    const PALETTES: u32 = 0xa31ba;
    const CREATOR: u32 = 0x5c;
    const LIFETIME: u32 = 0x42;
    const PALETTE_COUNTDOWN: u32 = 0x46;
    const PALETTE_INDEX: u32 = 0x47;
    /// Variable `A5+0x6e`: bit 0 = global blinking phase.
    const BLINK_PHASE: u32 = 0x6e;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => spawn(x),
            2 => live(x),
            _ => x.free_eshl(),
        }
    }

    fn spawn<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u16(CREATOR) == 0 {
            return;
        }
        x.add8(0x4, 2);
        crate::sound::play(x.b, sfx::ED186, x.o);
        x.set8(f::BOX_SET, 7);
        x.set8(f::ATTACK_POWER, 5);
        x.set8(f::ATTACK_ID, 0x1e);
        x.set16(LIFETIME, 0x5a);
        x.set8(PALETTE_COUNTDOWN, 4);
        let creator = x.linked(CREATOR);
        copy_xyz32(x, creator);
        copy_flip(x, creator);
        let owner = creator.u16(x.b, CREATOR);
        x.set16(f::OWNER, owner);
        match x.u8(0x3) {
            1 => {
                x.add16(0x8, neg(0xa));
                x.add16(0xc, 0xa);
            }
            2 => {
                x.add16(0x8, 0xa);
                x.add16(0xc, 0xa);
            }
            _ => {}
        }
        let r = random(x) & 0xf;
        let anim = x.rom8(RANDOM_ANIMS + r);
        x.anim_start(ANIMS, (anim as u32) as u16);
    }

    fn live<R: Bench>(x: &mut Cx<'_, R>) {
        cycle_palette(x);
        if x.linked(f::OWNER).u16(x.b, 0x4) == 0x406 {
            return x.free_eshl();
        }
        if x.u8(0x5) == 0 {
            x.anim_step();
            let t = x.u16(LIFETIME).wrapping_sub(1);
            x.set16(LIFETIME, t);
            if s16(t) < 0 {
                x.add8(0x4, 2);
                crate::sound::play(x.b, sfx::ED90A, x.o);
            }
        }
        flicker(x);
        x.reinsert();
    }

    /// Every 5 steps, goes to the next palette of the table (8 entries).
    fn cycle_palette<R: Bench>(x: &mut Cx<'_, R>) {
        let n = x.u8(PALETTE_COUNTDOWN).wrapping_sub(1);
        x.set8(PALETTE_COUNTDOWN, n);
        if s8(n) >= 0 {
            return;
        }
        x.set8(PALETTE_COUNTDOWN, 4);
        x.set8(0x2a, 1);
        let i = x.u8(PALETTE_INDEX).wrapping_add(1) & 7;
        x.set8(PALETTE_INDEX, i);
        let p = x.rom8(PALETTES + i as u32);
        x.set8(0x2b, p);
    }

    /// Hidden when the variant's parity differs from the global phase.
    fn flicker<R: Bench>(x: &mut Cx<'_, R>) {
        let hidden = (x.u8(0x3) & 1) as u16 != x.a5_u16(BLINK_PHASE) & 1;
        x.set_bit(0x1, 1, hidden);
    }
}

/// Shot 41 (0xa336c): ray launched toward the target of the owner of its creator (type 40, in `+0x5c`). Direction and Z
/// speed read according to the X distance to the target, reloads a 32-byte palette, then, on reaching the ground,
/// creates three shots of type 40 (creation function 0xa31c2) and finishes its animation.
mod shot41 {
    use super::*;

    const ANIMS: u32 = 0x1700f8;
    const CREATOR: u32 = 0x5c;
    /// Z speed (word) per 32-pixel slice of distance.
    const FALL_SPEEDS: u32 = 0xa3492;
    /// Palette copied (ROM to palette RAM): 8 longs.
    const PALETTE_SRC: u32 = 0x25a6e0;
    const PALETTE_DST: u32 = 0x91c2e0;
    /// Fields of the target (player object): aiming position.
    const TARGET_X: u32 = 0xb0;
    const TARGET_Y: u32 = 0xb2;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => spawn(x),
            2 => live(x),
            _ => x.free_eshl(),
        }
    }

    fn target<R: Bench>(x: &mut Cx<'_, R>) -> Obj {
        let w = x.linked(CREATOR).u16(x.b, CREATOR);
        obj_of(w)
    }

    fn spawn<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u16(CREATOR) == 0 {
            return;
        }
        x.add8(0x4, 2);
        crate::sound::play(x.b, sfx::ED32E, x.o);
        x.set8(f::BOX_SET, 7);
        x.set8(f::ATTACK_POWER, 5);
        x.set8(f::ATTACK_ID, 0x1f);
        let creator = x.linked(CREATOR);
        let owner = creator.u16(x.b, CREATOR);
        x.set16(f::OWNER, owner);
        copy_xyz32(x, creator);
        copy_flip(x, creator);
        x.add16(0x10, 0xffff);
        let t = target(x);
        let (tx, ty) = (t.u16(x.b, TARGET_X), t.u16(x.b, TARGET_Y));
        let dir = crate::aids::direction_from(x.b, x.o, (tx, ty));
        x.set8(0x2d, dir);
        x.select_speed(&crate::objects::motion::SPEEDS_B, 0x41);
        let t = target(x);
        let mut dist = s16(x.u16(0x8).wrapping_sub(t.u16(x.b, TARGET_X)));
        if dist < 0 {
            dist = dist.wrapping_neg();
        }
        let band = (1..=11).find(|&i| dist <= 0x20 * i).map_or(0x18, |i| 2 * (i - 1) as u32);
        let speed = x.rom16(FALL_SPEEDS + band);
        x.set16(0x18, speed);
        copy_palette(x);
        x.set8(0x2a, 1);
        x.set8(0x2b, 0x17);
        x.anim_start(ANIMS, 0);
    }

    /// Copies 8 longs from the ROM to the palette RAM.
    fn copy_palette<R: Bench>(x: &mut Cx<'_, R>) {
        for i in 0..8 {
            let v = x.data32(PALETTE_SRC + 4 * i);
            wr32(x.b, PALETTE_DST + 4 * i, v);
        }
    }

    fn live<R: Bench>(x: &mut Cx<'_, R>) {
        if x.linked(f::OWNER).u16(x.b, 0x4) == 0x406 {
            return x.free_eshl();
        }
        match x.u8(0x5) {
            0 => fall(x),
            2 => {
                if x.anim_step() {
                    x.add8(0x4, 2);
                }
            }
            _ => {}
        }
        x.reinsert();
    }

    fn fall<R: Bench>(x: &mut Cx<'_, R>) {
        x.move_xyz();
        if s16(x.u16(0xc)) > 0 {
            x.anim_step();
            return;
        }
        x.add8(0x5, 2);
        x.set16(0xc, 0);
        x.set8(0x2a, 1);
        x.set8(0x2b, 0x13);
        x.anim_start(ANIMS, 1);
        crate::families::eshl_creators::ring_28(&mut crate::families::effects_creators::leftovers::Pure, x.b, x.o);
    }
}

// ---------------------------------------------------------------------------------------------
// Types 42 and 43: burst attack launched from a host (`+0x42`)
// ---------------------------------------------------------------------------------------------

/// Shot 42 (0xa3568): attack stuck to its host (enemy in `+0x42`), 32 pixels in front of it and 64 in height. It
/// follows the host's animation (`+0x33`), then, when the enemy triggers it (`+0x59`), launches a salvo of 5 bursts
/// (type 43) every 8 frames whose angle (`+0x5e`) rotates by `+0x5f` (modulo 32), waits 20 frames then finishes its
/// animation. Disappears when the host goes to state 4 or when it has fallen back (Z <= 0, last substate).
mod shot42 {
    use super::*;

    const ANIMS: u32 = 0x170222;
    /// Animation of the attack according to `+0x33` of the host (byte table indexed by 7 bits).
    const ANIM_OF_HOST_FRAME: u32 = 0xa3600;
    const HOST: u32 = 0x42;
    const ARMED: u32 = 0x58;
    const TRIGGER: u32 = 0x59;
    const DONE: u32 = 0x5a;
    const SHARDS_LEFT: u32 = 0x46;
    const SHARD_DELAY: u32 = 0x48;
    const ANGLE: u32 = 0x5e;
    const ANGLE_STEP: u32 = 0x5f;
    /// Substates (`+5`).
    const IDLE: u8 = 0;
    const FIGHT: u8 = 2;
    const END: u8 = 4;
    /// Phases of the salvo (`+6`).
    const WIND_UP: u8 = 0;
    const WAIT_TRIGGER: u8 = 2;
    const BURST: u8 = 4;
    const PAUSE: u8 = 6;
    const RECOVER: u8 = 8;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        if suspended(x) {
            return;
        }
        follow_host(x);
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(f::BOX_SET, 7);
                ai_anim(x, ANIMS, 1);
            }
            2 => live(x),
            _ => x.free_eshl(),
        }
        if x.u8(0) != 0 {
            x.reinsert();
        }
    }

    /// Aligns in front of the host (32 pixels on the facing side, 64 in height, one notch closer).
    fn follow_host<R: Bench>(x: &mut Cx<'_, R>) {
        let host = x.linked(HOST);
        copy_xyz16(x, host);
        x.add16(0x10, 1);
        let dx = if x.u8(f::FLIP) != 0 { neg(0x20) } else { 0x20 };
        x.add16(0x8, dx);
        x.add16(0xc, 0x40);
    }

    fn live<R: Bench>(x: &mut Cx<'_, R>) {
        let host = x.linked(HOST);
        if host.u8(x.b, 4) == 4 {
            return x.free_eshl();
        }
        match x.u8(0x5) {
            IDLE => idle(x, host),
            FIGHT => fight(x),
            END => {
                if s16(host.u16(x.b, 0xc)) <= 0 {
                    x.free_eshl();
                }
            }
            _ => {}
        }
    }

    fn idle<R: Bench>(x: &mut Cx<'_, R>, host: Obj) {
        if x.u8(ARMED) != 0 {
            if x.u8(0x3) != 2 {
                return x.free_eshl();
            }
            return x.add8(0x5, 2);
        }
        if x.u8(0x3) == 0 {
            return;
        }
        let frame = host.u8(x.b, 0x33) & 0x7f;
        let anim = x.rom8(ANIM_OF_HOST_FRAME + frame as u32);
        ai_anim(x, ANIMS, anim as u32);
    }

    fn fight<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x6) {
            WIND_UP => {
                if ai_step(x) {
                    x.add8(0x6, 2);
                }
            }
            WAIT_TRIGGER => {
                if x.u8(TRIGGER) == 0 {
                    return;
                }
                x.add8(0x6, 2);
                x.set16(SHARDS_LEFT, 5);
                x.set16(SHARD_DELAY, 1);
                // starting angle and rotation direction: according to the owner's side and height
                let (mut a, mut step, mut a2, mut step2) = (0x16u8, 1u8, 0x1au8, 0xffu8);
                if x.u8(f::FLIP) != 0 {
                    (a, step, a2, step2) = (0xa, 0xff, 6, 1);
                }
                let owner = x.linked(f::OWNER);
                if s16(owner.u16(x.b, 0x10)) > s16(x.u16(0x10)) {
                    (a, step) = (a2, step2);
                }
                x.set8(ANGLE, a);
                x.set8(ANGLE_STEP, step);
            }
            BURST => {
                let t = x.u16(SHARD_DELAY).wrapping_sub(1);
                x.set16(SHARD_DELAY, t);
                if s16(t) > 0 {
                    return;
                }
                spawn_shard(x);
                x.set16(SHARD_DELAY, 8);
                let n = x.u16(SHARDS_LEFT).wrapping_sub(1);
                x.set16(SHARDS_LEFT, n);
                if s16(n) > 0 {
                    return;
                }
                x.add8(0x6, 2);
                x.set16(SHARDS_LEFT, 0x14);
            }
            PAUSE => {
                let n = x.u16(SHARDS_LEFT).wrapping_sub(1);
                x.set16(SHARDS_LEFT, n);
                if s16(n) > 0 {
                    return;
                }
                x.add8(0x6, 2);
                ai_anim(x, ANIMS, 6);
            }
            RECOVER => {
                if ai_step(x) {
                    x.add8(0x5, 2);
                    x.set8(DONE, 1);
                }
            }
            _ => {}
        }
    }

    /// Creates a burst (type 43) in front of the attack, with the current angle, then rotates the angle.
    fn spawn_shard<R: Bench>(x: &mut Cx<'_, R>) {
        let Some(c) = x.alloc(&pools::ESHL) else { return };
        c.set_u8(x.b, 0, 1);
        c.set_u8(x.b, 2, 0x2b);
        let (px, pz) = (x.u16(0x8), x.u16(0xc));
        c.set_u16(x.b, 0x8, px);
        c.set_u16(x.b, 0xc, pz.wrapping_sub(0x40));
        let y = x.u32(0x10);
        c.set_u32(x.b, 0x10, y);
        let y_low = c.u16(x.b, 0x12).wrapping_add(1);
        c.set_u16(x.b, 0x12, y_low);
        let flip = x.u8(f::FLIP);
        c.set_u8(x.b, f::FLIP, flip);
        let dx = if flip != 0 { 0x2a } else { neg(0x2a) };
        let cx = c.u16(x.b, 0x8).wrapping_add(dx);
        c.set_u16(x.b, 0x8, cx);
        let angle = x.u8(ANGLE);
        c.set_u8(x.b, 0x2d, angle);
        let step = x.u8(ANGLE_STEP);
        x.set8(ANGLE, angle.wrapping_add(step) & 0x1f);
    }
}

/// Shot 43 (0xa3762): burst of attack 42, which leaves in the direction `+0x2d` (speed table 0x40) in animation 0x10 +
/// a value of table 0xa37ba indexed by the direction / 2. Destroyed on contact (hit or parried) or on leaving the
/// screen window.
mod shot43 {
    use super::*;

    const ANIMS: u32 = 0x170222;
    const ANIM_OF_DIRECTION: u32 = 0xa37ba;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        if suspended(x) {
            return;
        }
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(f::BOX_SET, 7);
                x.select_speed(&crate::objects::motion::SPEEDS_B, 0x40);
                let dir = x.u8(0x2d) as u32 >> 1;
                let extra = x.rom8(ANIM_OF_DIRECTION + dir);
                ai_anim(x, ANIMS, 0x10u32.wrapping_add(extra as u32) & 0xff);
            }
            2 => fly(x),
            _ => x.free_eshl(),
        }
        if x.u8(0) != 0 {
            x.reinsert();
        }
    }

    fn fly<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(0x5) != 0 {
            return;
        }
        if x.u8(f::HIT_FLAGS) & 0x28 != 0 {
            return x.free_eshl();
        }
        x.move_xyz();
        if crate::ai_aids::outside_band(x.b, x.o, neg(0x50), 0x1d0) {
            x.free_eshl();
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Types 44 and 45: beam between two objects and debris
// ---------------------------------------------------------------------------------------------

/// Shot 44 (0xa380a): beam stretched between two linked objects (variants 0 and 1). `+0x46` is the creator (which
/// counts its beams in its byte `+0x42`) and the creator of the creator is the other end. The beam moves from end A2
/// toward end A1 in 16 frames (position = A2 + t * (A1 - A2) / 16, `+0x14` = t); when hit (bit 3 of `+0x57`) it plays
/// its end animation. Variant 1 starts placed on A2 (50 pixels higher), the start animation depends on `+0x2f` of A1,
/// and the end animation keeps the beam alive until its end.
mod beam44 {
    use super::*;

    const ANIMS: u32 = 0x17f002;
    const CREATOR: u32 = 0x46;
    /// Step count of the beam.
    const STEP: u32 = 0x14;
    /// Beam counter of the creator.
    const BEAM_COUNT: u32 = 0x42;
    /// Height above A2 of the aiming position in Z.
    const RAISE_Z: u16 = 0x50;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        if suspended(x) {
            return;
        }
        match x.u8(0x3) {
            0 => variant0(x),
            1 => variant1(x),
            _ => {}
        }
    }

    /// Living object designated by a word (negative address and non-zero activity byte).
    fn live_link<R: Bench>(x: &mut Cx<'_, R>, word: u16) -> Option<Obj> {
        if s16(word) >= 0 {
            return None;
        }
        let o = obj_of(word);
        (o.u8(x.b, 0) != 0).then_some(o)
    }

    /// The beam is finished: next state.
    fn end<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        x.set8(0x5, 0);
    }

    /// The two ends (creator and creator of the creator).
    fn ends<R: Bench>(x: &mut Cx<'_, R>) -> Option<(Obj, Obj)> {
        let w = x.u16(CREATOR);
        let a1 = live_link(x, w)?;
        let w = a1.u16(x.b, CREATOR);
        let a2 = live_link(x, w)?;
        Some((a1, a2))
    }

    /// The creator counts one more beam.
    fn count_beam<R: Bench>(x: &mut Cx<'_, R>, a1: Obj) {
        let n = a1.u8(x.b, BEAM_COUNT).wrapping_add(1);
        a1.set_u8(x.b, BEAM_COUNT, n);
    }

    fn variant0<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => {
                end(x);
                x.set8(STEP, 0);
                x.set8(f::BOX_SET, 7);
                x.set8(f::ATTACK_POWER, 0x20);
                let w = x.u16(CREATOR);
                let Some(a1) = live_link(x, w) else { return end(x) };
                count_beam(x, a1);
                ai_anim(x, ANIMS, 0xd);
            }
            2 => beam(x, 0xf, false),
            4 => release(x),
            6 => x.free_eshl(),
            _ => {}
        }
    }

    fn variant1<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => {
                end(x);
                x.set8(STEP, 0);
                let w = x.u16(CREATOR);
                let Some(a1) = live_link(x, w) else { return end(x) };
                count_beam(x, a1);
                let w = a1.u16(x.b, CREATOR);
                let Some(a2) = live_link(x, w) else { return end(x) };
                let (px, py, pz) = (a2.u16(x.b, 0x8), a2.u16(x.b, 0x10), a2.u16(x.b, 0xc));
                x.set16(0x8, px);
                x.set16(0x10, py);
                x.set16(0xc, pz.wrapping_add(RAISE_Z));
                copy_flip(x, a2);
                x.set8(0x2f, 0);
                let anim = if a1.u8(x.b, 0x2f) != 0 { 0x1a } else { 0x18 };
                ai_anim(x, ANIMS, anim);
                x.set8(f::BOX_SET, 7);
                x.set8(f::ATTACK_POWER, 0x20);
            }
            2 => beam(x, 0x1b, true),
            4 => release(x),
            6 => x.free_eshl(),
            _ => {}
        }
    }

    /// State 2. `hit_anim`: end animation launched on contact; `lingers`: variant 1, where the end of the end animation
    /// keeps the beam (and where the running animation ends it).
    fn beam<R: Bench>(x: &mut Cx<'_, R>, hit_anim: u32, lingers: bool) {
        if x.a5_u8(GAME_PAUSED) != 0 {
            return x.reinsert();
        }
        if x.u8(0x5) != 0 {
            if ai_step(x) == lingers {
                return x.reinsert();
            }
            return end(x);
        }
        if x.u8(f::HIT_FLAGS) & 0x8 != 0 {
            x.add8(0x5, 2);
            return ai_anim(x, ANIMS, hit_anim);
        }
        let Some((a1, a2)) = ends(x) else { return end(x) };
        interpolate(x, a1, a2);
        x.add8(STEP, 1);
        if s8(x.u8(STEP)) > 0xf {
            return end(x);
        }
        ai_step(x);
        x.reinsert();
    }

    /// Position = A2 + t * (A1 - A2) / 16 (word arithmetic), t = `+0x14`; nothing if t = 0. In Z, the difference is
    /// measured 0x50 higher.
    fn interpolate<R: Bench>(x: &mut Cx<'_, R>, a1: Obj, a2: Obj) {
        let t = x.u8(STEP) as u16;
        if t == 0 {
            return;
        }
        for (off, raise) in [(0x8, 0), (0xc, RAISE_Z), (0x10, 0)] {
            let base = a2.u16(x.b, off);
            let d = s16(a1.u16(x.b, off).wrapping_sub(base).wrapping_sub(raise)) >> 4;
            let pos = (d as u16).wrapping_mul(t).wrapping_add(base).wrapping_add(raise);
            x.set16(off, pos);
        }
    }

    /// State 4: gives a beam back to its creator.
    fn release<R: Bench>(x: &mut Cx<'_, R>) {
        let w = x.u16(CREATOR);
        if let Some(a1) = live_link(x, w) {
            let n = a1.u8(x.b, BEAM_COUNT).wrapping_sub(1);
            a1.set_u8(x.b, BEAM_COUNT, n);
        }
        x.add8(0x4, 2);
    }
}

/// Shot 45 (0xa3b88): debris thrown by an object (`+0x46`), two variants. Variant 0: springs out of the object with a
/// speed drawn at random from an 8-row table, bounces once then disappears at the end of the animation. Variant 1:
/// burst placed on the object (offset according to `+0x2f`), animated as long as the object is in state `+0xa4 = 4`,
/// then falls and disappears on the ground. The debris are kept on the screen (0xfbb0).
mod debris45 {
    use super::*;

    const ANIMS: u32 = 0x17f002;
    const SOURCE: u32 = 0x46;
    /// Rows of 4 words (VX, AX, VZ, VY) for variant 0 (data space).
    const BURST_SPEEDS: u32 = 0xa3e30;
    const BURST_ANIMS: u32 = 0xa3c3a;
    const HOP_VZ: u32 = 0xa3c94;
    /// Mirror of `+0x2f` for the flipped object.
    const MIRROR: u32 = 0xa3d10;
    /// Offsets of variant 1 by `+0x2f`: X, then Z by a read shifted by two bytes.
    const PLACE_OFFSETS: u32 = 0xa3dc2;
    /// Margin of the on-screen restraint.
    const MARGIN: u32 = 0x10;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        if suspended(x) {
            return;
        }
        match x.u8(0x3) {
            0 => variant0(x),
            1 => variant1(x),
            _ => {}
        }
    }

    /// The object disappears: state 6.
    fn vanish<R: Bench>(x: &mut Cx<'_, R>) {
        x.set8(0x4, 6);
        x.set8(0x5, 0);
    }

    /// Source (negative address), possibly alive.
    fn source<R: Bench>(x: &mut Cx<'_, R>, check_active: bool) -> Option<Obj> {
        let word = x.u16(SOURCE);
        if s16(word) >= 0 {
            return None;
        }
        let o = obj_of(word);
        if check_active && o.u8(x.b, 0) == 0 {
            return None;
        }
        Some(o)
    }

    fn variant0<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(0x5, 0);
                let Some(src) = source(x, true) else { return vanish(x) };
                copy_xyz16(x, src);
                copy_flip(x, src);
                x.add16(0xc, 0x50);
                let r = random(x);
                let row = BURST_SPEEDS + (r & 0x38);
                let (mut vx, mut ax) = (x.data16(row), x.data16(row + 2));
                if x.u8(f::FLIP) == 0 {
                    (vx, ax) = (neg(vx), neg(ax));
                }
                x.set16(0x14, vx);
                x.set16(0x16, ax);
                let vz = x.data16(row + 4);
                x.set16(0x18, vz);
                x.set16(0x1a, 0);
                let vy = x.data16(row + 6);
                x.set16(0x1c, vy);
                x.set16(0x1e, 0);
                let anim = x.rom8(BURST_ANIMS + (r & 3));
                ai_anim(x, ANIMS, anim as u32);
                x.set8(f::BOX_SET, 7);
                x.set8(f::ATTACK_POWER, 0);
            }
            2 => {
                if ai_step(x) {
                    return vanish(x);
                }
                if x.u8(0x5) == 0 && x.u8(f::HIT_FLAGS) & 0x8 == 0 && x.u8(0x33) == 0 {
                    x.add8(0x5, 2);
                    x.set32(0x14, 0);
                    x.set32(0x1c, 0);
                    let r = random(x) & 6;
                    let vz = x.rom16(HOP_VZ + r);
                    x.set16(0x18, vz);
                    x.set16(0x1a, 0);
                    ai_anim(x, ANIMS, 0x11);
                }
                x.move_xyz();
                settle(x);
            }
            4 => x.add8(0x4, 2),
            6 => x.free_eshl(),
            _ => {}
        }
    }

    /// Common end: kept on the screen, then put back in the sorted list.
    fn settle<R: Bench>(x: &mut Cx<'_, R>) {
        x.keep_on_screen(MARGIN as u16);
        x.reinsert();
    }

    fn variant1<R: Bench>(x: &mut Cx<'_, R>) {
        let Some(src) = source(x, false) else { return vanish(x) };
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(0x5, 0);
                let flip = copy_flip(x, src);
                if flip != 0 {
                    let k = x.u8(0x2f) as u32;
                    let mirrored = x.rom8(MIRROR + k);
                    x.set8(0x2f, mirrored);
                }
                let k = x.u8(0x2f);
                x.set8(0x2d, k.wrapping_mul(4));
                x.select_speed(&crate::objects::motion::SPEEDS_A, 0x1f);
                place(x, src);
                x.set8(f::ATTACK_ID, 0x14);
                x.anim_start(ANIMS, 0x1f);
            }
            2 => {
                match x.u8(0x5) {
                    0 => {
                        if src.u8(x.b, 0xa4) == 4 {
                            x.anim_step();
                        } else {
                            x.add8(0x5, 2);
                            x.set32(0x14, 0);
                            x.set32(0x1c, 0);
                            x.set16(0x18, neg(0x300));
                            x.set16(0x1a, 0);
                            x.anim_start(ANIMS, 0x21);
                        }
                    }
                    2 => {
                        x.anim_step();
                        x.move_by_table_xy();
                        x.move_z();
                        if s16(x.u16(0xc)) < 0 {
                            x.set16(0xc, 0);
                            x.add8(0x4, 2);
                        }
                    }
                    _ => {}
                }
                settle(x);
            }
            4 => x.add8(0x4, 2),
            6 => x.free_eshl(),
            _ => {}
        }
    }

    /// Places the burst on the source: X and Y of the source, Z + 0x30, then offset according to `+0x2f`.
    fn place<R: Bench>(x: &mut Cx<'_, R>, src: Obj) {
        let (sx, sy, sz) = (src.u16(x.b, 0x8), src.u16(x.b, 0x10), src.u16(x.b, 0xc));
        x.set16(0x8, sx);
        x.set16(0x10, sy);
        x.set16(0xc, sz.wrapping_add(0x30));
        let k = 2 * (x.u8(0x2f) & 7) as u32;
        let dx = x.rom16(PLACE_OFFSETS + k);
        // the Z row is the same table read with a two-byte offset, indexed by dx
        let dz = x.rom16((PLACE_OFFSETS + 2).wrapping_add(dx as i16 as i32 as u32));
        x.add16(0x8, dx);
        x.add16(0xc, dz);
    }
}

// ---------------------------------------------------------------------------------------------
// Types 46 to 50
// ---------------------------------------------------------------------------------------------

/// Shot 46 (0xa3e72): mobile scenery object (animation chosen by the variant) that scrolls by 4 pixels per frame to the
/// left and disappears when it passes the left edge of the screen (`A5+0x506` - 0x50).
mod scroller46 {
    use super::*;

    const ANIMS: u32 = 0x1703e6;
    /// Left edge of the screen (variable `A5+0x506`).
    const SCREEN_LEFT: u32 = 0x506;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                let variant = x.u8(0x3) as u32;
                x.anim_start(ANIMS, variant as u16);
                x.reinsert();
            }
            2 => {
                x.add16(0x8, neg(4));
                let edge = x.a5_u16(SCREEN_LEFT).wrapping_sub(0x50);
                if s16(edge) > s16(x.u16(0x8)) {
                    x.free_eshl();
                }
            }
            _ => x.free_eshl(),
        }
    }
}

/// Shot 47 (0xa3ebe): invisible appearance, randomly offset around the edge of the camera (`A5+0x5a6` in X, `A5+0x5aa`
/// in Y), which waits for a delay drawn from a table (`+0x58`), becomes visible then plays its animation to the end.
mod spawn47 {
    use super::*;

    const ANIMS: u32 = 0x16fbf4;
    /// Tables of 16 words of offsets: in X (inside the band / outside the band), in Y (same), delays.
    const X_INSIDE: u32 = 0xa3fd4;
    const X_OUTSIDE: u32 = 0xa3ff4;
    const Y_INSIDE: u32 = 0xa4014;
    const Y_OUTSIDE: u32 = 0xa4034;
    const DELAYS: u32 = 0xa4068;
    const CAMERA_X: u32 = 0x5a6;
    const CAMERA_Y: u32 = 0x5aa;
    const DELAY: u32 = 0x58;
    const WAIT: u8 = 0;
    const PLAY: u8 = 2;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        if suspended(x) {
            return;
        }
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(f::BOX_SET, 7);
                x.set8(f::ATTACK_POWER, 0x1e);
                x.set_bit(0x1, 1, true);
                place(x);
                let r = random(x);
                let delay = x.rom16(DELAYS + 2 * (r & 0xf));
                x.set16(DELAY, delay);
                ai_anim(x, ANIMS, 1);
            }
            2 => match x.u8(0x5) {
                WAIT => {
                    let t = x.u16(DELAY).wrapping_sub(1);
                    x.set16(DELAY, t);
                    if s16(t) <= 0 {
                        x.add8(0x5, 2);
                        x.set_bit(0x1, 1, false);
                    }
                }
                PLAY => {
                    if ai_step(x) {
                        x.add8(0x4, 2);
                    }
                }
                _ => {}
            },
            _ => x.free_eshl(),
        }
        if x.u8(0) != 0 {
            x.reinsert();
        }
    }

    /// Offsets the position at random: brings it back toward the screen if it is too far from it.
    fn place<R: Bench>(x: &mut Cx<'_, R>) {
        let idx = 2 * (random(x) & 0xf);
        let cam = x.a5_u16(CAMERA_X);
        let (lo, hi) = (cam.wrapping_add(0x20), cam.wrapping_add(0x160));
        let px = x.u16(0x8);
        let dx = if s16(lo) >= s16(px) {
            x.rom16(X_OUTSIDE + idx)
        } else if s16(hi) <= s16(px) {
            neg(x.rom16(X_OUTSIDE + idx))
        } else {
            x.rom16(X_INSIDE + idx)
        };
        x.add16(0x8, dx);
        let idx = 2 * (random(x) & 0xf);
        let cam = x.a5_u16(CAMERA_Y);
        let (lo, hi) = (cam.wrapping_add(0x38), cam.wrapping_add(0x68));
        let py = x.u16(0x10);
        let dy = if s16(lo) >= s16(py) {
            x.rom16(Y_OUTSIDE + idx)
        } else if s16(hi) <= s16(py) {
            neg(x.rom16(Y_OUTSIDE + idx))
        } else {
            x.rom16(Y_INSIDE + idx)
        };
        x.add16(0x10, dy);
    }
}

/// Shot 48 (0xa4088): projectile launched in a lob by a launcher (`+0x5c`, whose fallen shots it counts in its byte
/// `+0x1d`). Waits 16 frames, rises (VZ = -0x100) until Z <= 0x20, waits 16 frames, then descends back to the starting
/// height (`+0x4e`); it can then destroy the object spotted in `+0x58`. It then frees its slot in the launcher (word
/// `+8+variant`).
mod lob48 {
    use super::*;

    const ANIMS: u32 = 0x17041e;
    const LAUNCHER: u32 = 0x5c;
    const WAIT: u32 = 0x4c;
    const HOME_Z: u32 = 0x4e;
    const TARGET: u32 = 0x58;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set16(WAIT, 0x10);
                let z = x.u16(0xc);
                x.set16(HOME_Z, z);
                x.set8(f::BOX_SET, 7);
                x.set8(f::ATTACK_ID, 0x17);
                x.anim_start(ANIMS, 1);
                x.reinsert();
            }
            2 => {
                sub_state(x);
                if x.u8(f::HIT_FLAGS) & 0x8 != 0 {
                    x.set8(f::HIT_FLAGS, 0);
                    let launcher = x.linked(LAUNCHER);
                    let n = launcher.u8(x.b, 0x1d).wrapping_add(1);
                    launcher.set_u8(x.b, 0x1d, n);
                }
            }
            4 => {
                x.add8(0x4, 2);
                let launcher = x.linked(LAUNCHER);
                let slot = (8u8.wrapping_add(x.u8(0x3))) as u32;
                launcher.set_u16(x.b, slot, 0);
            }
            _ => x.free_eshl(),
        }
    }

    fn sub_state<R: Bench>(x: &mut Cx<'_, R>) {
                match x.u8(0x5) {
                    0 => {
                        if dec_wait(x) {
                            return;
                        }
                        x.add8(0x5, 2);
                        x.set16(0x18, neg(0x100));
                        x.set16(0x1a, neg(0x20));
                        // Original glitch: 0x54872 returns D0 = 0 even when the shot is created, so the `bne` that was
                        // supposed to hold it in `TARGET` is never taken
                        crate::families::small::shot_18(&mut crate::families::effects_creators::leftovers::Pure, x.b, x.o, 2);
                    }
                    2 => {
                        x.move_z();
                        if s16(x.u16(0xc)) <= 0x20 {
                            x.add8(0x5, 2);
                            x.set16(WAIT, 0x10);
                        }
                    }
                    4 => {
                        if dec_wait(x) {
                            return;
                        }
                        x.add8(0x5, 2);
                        x.set16(0x18, 0x180);
                        x.set16(0x1a, 0);
                    }
                    6 => {
                        x.move_z();
                        if s16(x.u16(HOME_Z)) > s16(x.u16(0xc)) {
                            return;
                        }
                        x.add8(0x4, 2);
                        if x.u16(TARGET) != 0 {
                            let t = x.linked(TARGET);
                            t.set_u8(x.b, 0x4, 6);
                        }
                    }
                    _ => {}
                }
    }

    /// Countdown of the delay `+0x4c`: true as long as it has not elapsed.
    fn dec_wait<R: Bench>(x: &mut Cx<'_, R>) -> bool {
        let t = x.u16(WAIT).wrapping_sub(1);
        x.set16(WAIT, t);
        t != 0
    }
}

/// Shot 49 (0xa418e): blow dealt 0x6e pixels in front of the enemy that created it (`+0x42`), at height 0x35. It plays
/// the start animation as long as the host has not gone to substate 6 (`+7`); it finishes its animation afterwards, or
/// disappears if the host is stunned (state 4) without the animation having reached its mark (`+0x33`).
mod strike49 {
    use super::*;

    const ANIMS: u32 = 0x170468;
    const HOST: u32 = 0x42;
    const SOUND: u32 = 0xed398;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        if suspended(x) {
            return;
        }
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(f::BOX_SET, 7);
                x.set8(f::ATTACK_POWER, 0x14);
                let host = x.linked(HOST);
                let px = host.u16(x.b, 0x8);
                x.set16(0x8, px);
                x.set16(0xc, 0x35);
                let py = host.u16(x.b, 0x10);
                x.set16(0x10, py);
                x.add32(0x10, 2);
                let flip = copy_flip(x, host);
                x.add16(0x8, if flip != 0 { 0x6e } else { neg(0x6e) });
                crate::sound::play(x.b, SOUND, x.o);
                ai_anim(x, ANIMS, 0);
            }
            2 => match x.u8(0x5) {
                0 => {
                    let host = x.linked(HOST);
                    if host.u8(x.b, 7) == 6 {
                        return finish(x);
                    }
                    if host.u8(x.b, 4) == 4 {
                        if x.u8(0x33) != 0 {
                            finish(x);
                        } else {
                            x.free_eshl();
                        }
                    } else {
                        ai_step(x);
                    }
                }
                2 => {
                    if ai_step(x) {
                        x.free_eshl();
                    }
                }
                _ => {}
            },
            _ => x.free_eshl(),
        }
        if x.u8(0) != 0 {
            x.reinsert();
        }
    }

    /// Goes to the final animation.
    fn finish<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x5, 2);
        x.set8(0x6, 0);
        ai_anim(x, ANIMS, 2);
    }
}

/// Shot 50 (0xa4266): strikes of an enemy (`+0x42`) in two variants (0: ball that leaves in a lob 0x70 pixels in front
/// of the host; 1: slow ball at 0x76 pixels). The animation advances by phases (`+0x5a`, phase duration `+0x5b` read
/// from table 0xa44aa) with the animation value `phase + 3`. When the host is stunned (state 4) or its trajectory ends,
/// goes to the animation `phase + 6` (substate 4) then disappears. Kept on the screen and tested by 0x33fe4 at each
/// frame.
mod bullet50 {
    use super::*;

    const ANIMS: u32 = 0x170468;
    const HOST: u32 = 0x42;
    const SOUND: u32 = 0xed398;
    /// Duration of the animation phases (bytes).
    const PHASE_LENGTHS: u32 = 0xa44aa;
    const PHASE: u32 = 0x5a;
    const PHASE_TIMER: u32 = 0x5b;
    const FALL_TIMER: u32 = 0x58;
    /// Saving of `+0x3b` (frame priority) across the animation.
    const SAVED_PRIORITY: u32 = 0x5e;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        if suspended(x) {
            return;
        }
        match x.u8(0x4) {
            0 => match x.u8(0x3) {
                0 => launch(x, 0x12, 0x70, 0x1e, neg(0x380), true),
                _ => launch(x, 0x17, 0x76, 0x2b, 0x40, false),
            },
            2 => match x.u8(0x3) {
                0 => lob(x),
                _ => slow(x),
            },
            _ => x.free_eshl(),
        }
        if x.u8(0) != 0 {
            x.keep_in_zone();
            x.push_terrain();
            x.reinsert();
        }
    }

    /// Creation: variant 0 (`lob = true`) or 1.
    fn launch<R: Bench>(x: &mut Cx<'_, R>, speed: u32, dx: u16, dz: u16, vz: u16, lob: bool) {
        x.add8(0x4, 2);
        if !lob {
            x.set16(FALL_TIMER, 0x28);
        }
        x.set8(f::BOX_SET, 7);
        x.set8(f::ATTACK_POWER, 0x14);
        x.select_speed(&crate::objects::motion::SPEEDS_B, speed as u16);
        let host = x.linked(HOST);
        copy_xyz16(x, host);
        let flip = x.u8(f::FLIP);
        x.add16(0x8, if flip != 0 { dx as u16 } else { neg(dx as u16) });
        x.add16(0xc, dz);
        x.add32(0x10, 2);
        x.set16(0x18, vz);
        crate::sound::play(x.b, SOUND, x.o);
        start_phase(x);
    }

    /// Advances one phase when its duration has elapsed (and we are not already in phase 2).
    fn phase_tick<R: Bench>(x: &mut Cx<'_, R>) {
        if s8(x.u8(PHASE)) < 2 {
            let n = x.u8(PHASE_TIMER).wrapping_sub(1);
            x.set8(PHASE_TIMER, n);
            if s8(n) <= 0 {
                x.add8(PHASE, 1);
                start_phase(x);
                return;
            }
        }
        step_phase(x);
    }

    /// Starts the animation of the current phase, then one step.
    fn start_phase<R: Bench>(x: &mut Cx<'_, R>) {
        let phase = x.u8(PHASE);
        let len = x.rom8(PHASE_LENGTHS + phase as u32);
        x.set8(PHASE_TIMER, len);
        ai_anim(x, ANIMS, phase as u32 + 3);
        let p = x.u8(0x3b);
        x.set8(SAVED_PRIORITY, p);
        step_phase(x);
    }

    fn step_phase<R: Bench>(x: &mut Cx<'_, R>) {
        ai_step(x);
        let p = x.u8(0x3b);
        x.set8(SAVED_PRIORITY, p);
    }

    fn restore_priority<R: Bench>(x: &mut Cx<'_, R>) {
        let p = x.u8(SAVED_PRIORITY);
        x.set8(0x3b, p);
    }

    /// The host is stunned: goes to the end animation (`phase + 6`), substate 4 for variant 0 (state 2, substate 4),
    /// next substate for variant 1.
    fn host_stunned<R: Bench>(x: &mut Cx<'_, R>) -> bool {
        let host = x.linked(HOST);
        host.u8(x.b, 4) == 4
    }

    fn end_animation<R: Bench>(x: &mut Cx<'_, R>) {
        let phase = x.u8(PHASE) as u32;
        ai_anim(x, ANIMS, phase + 6);
    }

    fn lob<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x5) {
            0 => {
                if host_stunned(x) {
                    x.set16(0x4, 0x204);
                    return end_animation(x);
                }
                x.move_xyz();
                phase_tick(x);
                if s16(x.u16(0xc)) > 0 {
                    return restore_priority(x);
                }
                x.add8(0x5, 2);
                x.select_speed(&crate::objects::motion::SPEEDS_B, 0x17);
                x.set16(FALL_TIMER, 0x1a);
                x.set16(0xc, 0);
                restore_priority(x);
            }
            2 => {
                if host_stunned(x) {
                    x.set16(0x4, 0x204);
                    return end_animation(x);
                }
                x.move_xy();
                let t = x.u16(FALL_TIMER).wrapping_sub(1);
                x.set16(FALL_TIMER, t);
                if s16(t) <= 0 {
                    x.set16(0x4, 0x204);
                    return end_animation(x);
                }
                phase_tick(x);
                restore_priority(x);
            }
            4 => {
                if ai_step(x) {
                    x.free_eshl();
                }
            }
            _ => {}
        }
    }

    fn slow<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x5) {
            0 => {
                if host_stunned(x) {
                    return end_slow(x);
                }
                x.move_xyz();
                phase_tick(x);
                let t = x.u16(FALL_TIMER).wrapping_sub(1);
                x.set16(FALL_TIMER, t);
                if s16(t) <= 0 {
                    return end_slow(x);
                }
                restore_priority(x);
            }
            2 => {
                x.move_xy();
                if ai_step(x) {
                    x.free_eshl();
                }
            }
            _ => {}
        }
    }

    fn end_slow<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x5, 2);
        end_animation(x);
    }
}

// ---------------------------------------------------------------------------------------------
// Types 51 and 52
// ---------------------------------------------------------------------------------------------

/// Shot 51 (0xa4522): projectile falling from the top of the screen, launched by a launcher (`+0x5c`, whose fallen
/// shots it counts in its byte `+0x1d`). Waits `+0x42` frames, appears 0x100 higher than its depth, waits 30 frames
/// then falls (VZ = -0x100, AZ = -0x40) to the ground, where it triggers two impact subroutines (0xab7d8 with 3, 0x2860
/// with 8) and disappears.
mod drop51 {
    use super::*;

    const ANIMS: u32 = 0x16ea18;
    const LAUNCHER: u32 = 0x5c;
    const WAIT: u32 = 0x42;
    /// Top of the screen: `A5+0x50a` (player 1) or `A5+0x55a` (player 2, if `A5+0x36`).
    const SCREEN_TOP: u32 = 0x50a;
    const SCREEN_TOP_P2: u32 = 0x55a;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => {
                if wait(x) {
                    return;
                }
                x.add8(0x4, 2);
                x.set16(WAIT, 0x1e);
                x.set8(f::BOX_SET, 7);
                x.set8(f::ATTACK_ID, 0x17);
                let top = if x.a5_u8(0x36) != 0 { SCREEN_TOP_P2 } else { SCREEN_TOP };
                let top = x.a5_u16(top);
                let depth = x.u16(0x10);
                x.set16(0xc, top.wrapping_add(0x100).wrapping_sub(depth));
                x.anim_start(ANIMS, 0);
                x.reinsert();
            }
            2 => {
                if x.u8(0x5) == 0 {
                    if wait(x) {
                        return;
                    }
                    x.add8(0x5, 2);
                    x.set16(0x18, neg(0x100));
                    x.set16(0x1a, neg(0x40));
                    return;
                }
                x.anim_step();
                x.move_z();
                if s16(x.u16(0xc)) >= 0 {
                    if x.u8(f::HIT_FLAGS) & 0x8 != 0 {
                        x.set8(f::HIT_FLAGS, 0);
                        let launcher = x.linked(LAUNCHER);
                        let n = launcher.u8(x.b, 0x1d).wrapping_add(1);
                        launcher.set_u8(x.b, 0x1d, n);
                    }
                    return;
                }
                x.add8(0x4, 2);
                crate::families::effects_creators::debris(x.b, x.o, 3);
                crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::B, 8);
            }
            4 => {
                x.add8(0x4, 2);
                if x.u8(0x2f) == 0 {
                    let launcher = x.linked(LAUNCHER);
                    let slot = 8u8.wrapping_add(x.u8(0x3)) as u32;
                    launcher.set_u16(x.b, slot, 0);
                }
            }
            _ => x.free_eshl(),
        }
    }

    /// Countdown of the delay `+0x42`: true as long as it has not elapsed.
    fn wait<R: Bench>(x: &mut Cx<'_, R>) -> bool {
        let t = x.u16(WAIT).wrapping_sub(1);
        x.set16(WAIT, t);
        t != 0
    }
}

/// Shot 52 (0xa46c6): elements of a boss's final attack, attached to it (`+0x46`), four variants that exist only as
/// long as the boss is in the expected state `+0xa0`:
///  0 (state 0x201): ball placed on the boss, which launches three sub-elements (variants 1 to 3 of
///    `+0x2f`); hurts the players it touches;
///  1 (state 0x214): ray stuck beside the boss;
///  2 (state 0x215): full-screen veil that follows the screen and hurts the players outside the safe zones
///    of stage 0x2e or 0x69;
///  3: variant of the previous one that lasts 256 frames (`+0x42`).
/// The screen locking states are `A5+0x43` (0x3f taken, 0xff released) and `A5+0x4ba`.
mod finale52 {
    use super::*;

    const ANIMS: u32 = 0x17e048;
    const PARENT: u32 = 0x46;
    const PARENT_STATE: u32 = 0xa0;
    const SUBTYPE: u32 = 0x2f;
    const TIMER: u32 = 0x42;
    /// Animation per sub-type of variant 0 (bytes).
    const ANIM_OF_SUBTYPE: u32 = 0xa473e;
    /// Sub-types of the three elements launched by variant 0.
    const ORB_SUBTYPES: u32 = 0xa46c2;
    /// Screen: X (`A5+0x506`), Y (long `A5+0x50a`).
    const SCREEN_X: u32 = 0x506;
    const SCREEN_Y: u32 = 0x50a;
    const STAGE: u32 = 0x34;
    const SCREEN_LOCK: u32 = 0x43;
    const LOCK_TAKEN: u8 = 0x3f;
    const LOCK_FREE: u8 = 0xff;
    const FLAG_4BA: u32 = 0x4ba;
    /// Stage 0x2e: pairs (start, length) of the safe zones in X, terminated by 0.
    const SAFE_RANGES: u32 = 0xa4bca;
    /// Stage 0x69: safe rectangles (X, width, Y, height), one per bit of `0xff2064`.
    const SAFE_RECTS: u32 = 0xa4bd4;
    const RECT_FLAGS: u32 = 0xff2064;
    /// Attack boxes (rows of 16 bytes per `+0x3e`) and pointers to the players' boxes.
    const ATTACK_BOXES: u32 = 0xa4cd2;
    const PLAYER_BOXES: u32 = 0xa4cba;
    /// First player (`A5+0x5de`) and step.
    const PLAYERS: u32 = 0x5de;
    const PLAYER_SIZE: u32 = 0x200;
    const SCREEN_END_FX: u32 = 0xe55fe;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        if suspended(x) {
            return;
        }
        match x.u8(0x3) {
            0 => variant0(x),
            1 => variant1(x),
            2 => variant2(x),
            3 => variant3(x),
            _ => {}
        }
    }

    fn vanish<R: Bench>(x: &mut Cx<'_, R>) {
        x.set8(0x4, 6);
        x.set8(0x5, 0);
    }

    /// Living boss (negative address, non-zero activity byte).
    fn parent<R: Bench>(x: &mut Cx<'_, R>) -> Option<Obj> {
        let w = x.u16(PARENT);
        if s16(w) >= 0 {
            return None;
        }
        let o = obj_of(w);
        (o.u8(x.b, 0) != 0).then_some(o)
    }

    fn init_boxes<R: Bench>(x: &mut Cx<'_, R>) {
        x.set8(f::BOX_SET, 7);
        x.set8(f::ATTACK_POWER, 0);
    }

    /// Aligns on the boss; the sub-type offsets in height (0x1c minus the sub-type).
    fn attach<R: Bench>(x: &mut Cx<'_, R>, boss: Obj) {
        copy_xyz16(x, boss);
        let sub = x.u8(SUBTYPE) as u16;
        if sub == 0 {
            x.add32(0x10, 1);
        }
        x.add16(0xc, 0x1cu16.wrapping_sub(sub));
        x.add16(0x10, sub);
    }

    /// End of the frame: kept on the screen then put back in the sorted list.
    fn settle<R: Bench>(x: &mut Cx<'_, R>) {
        x.keep_on_screen(0x10);
        x.reinsert();
    }

    fn unlock_screen<R: Bench>(x: &mut Cx<'_, R>) {
        x.set_a5_u8(SCREEN_LOCK, LOCK_FREE);
        x.set_a5_u16(FLAG_4BA, 0);
    }

    // ----- variant 0 -----

    fn variant0<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(0x5, 0);
                let Some(boss) = parent(x) else { return vanish(x) };
                attach(x, boss);
                let sub = x.u8(SUBTYPE) as u32;
                let anim = x.rom8(ANIM_OF_SUBTYPE + sub);
                x.anim_start(ANIMS, (anim as u32) as u16);
                init_boxes(x);
            }
            2 => {
                let Some(boss) = parent(x) else { return vanish(x) };
                if boss.u16(x.b, PARENT_STATE) != 0x201 {
                    return vanish(x);
                }
                x.set_bit(0x1, 1, false);
                match x.u8(SUBTYPE) {
                    0 => orb(x, boss),
                    1..=3 => follower(x, boss),
                    _ => {}
                }
                x.set_a5_u16(FLAG_4BA, 0);
                hit_players(x);
                x.reinsert();
            }
            4 => x.add8(0x4, 2),
            6 => x.free_eshl(),
            _ => {}
        }
    }

    /// Sub-type 0: follows the launch animation, then launches three elements and sticks beside the boss.
    fn orb<R: Bench>(x: &mut Cx<'_, R>, boss: Obj) {
        let mut source = boss;
        if x.u8(0x5) == 0 {
            if !x.anim_step() {
                return attach(x, boss);
            }
            x.add8(0x5, 2);
            let last = spawn_followers(x);
            x.anim_start(ANIMS, 0x1b);
            // Original glitch: the position is then copied from A1, which then designates the last element created (and
            // not the boss), or, if the pool was full, the slot that the allocation leaves in A1.
            source = last;
        }
        x.anim_step();
        copy_xyz16(x, source);
        copy_flip(x, source);
        let dx = if x.u8(f::FLIP) != 0 { 8 } else { neg(8) };
        x.add16(0x8, dx);
        x.add16(0xc, 0xe);
        x.add16(0x10, 0xffff);
    }

    /// Creates three elements of sub-types 3, 2, 1 (table), linked to the same boss. Returns the original's A1: the
    /// last element created, or what the failing allocation leaves (`Pool::alloc_or_leftover`).
    fn spawn_followers<R: Bench>(x: &mut Cx<'_, R>) -> Obj {
        let mut a1 = None;
        for i in (0..3u32).rev() {
            let c = match x.alloc_or_leftover(&pools::ESHL) {
                Ok(c) => c,
                Err(a1) => return a1,
            };
            a1 = Some(c);
            c.set_u8(x.b, 0, 1);
            c.set_u8(x.b, 2, 0x34);
            c.set_u8(x.b, 3, 0);
            let sub = x.rom8(ORB_SUBTYPES + i);
            c.set_u8(x.b, SUBTYPE, sub);
            let boss = x.u16(PARENT);
            c.set_u16(x.b, PARENT, boss);
        }
        a1.expect("three followers created")
    }

    /// Sub-types 1 to 3: invisible as long as the boss has not armed `+0x3e` (except sub-type 2).
    fn follower<R: Bench>(x: &mut Cx<'_, R>, boss: Obj) {
        attach(x, boss);
        x.set_bit(0x1, 1, true);
        let always = x.u8(SUBTYPE) & 7 == 2;
        if !always && boss.u8(x.b, 0x3e) == 0 {
            return;
        }
        x.set_bit(0x1, 1, false);
        x.anim_step();
    }

    // ----- variant 1 -----

    fn variant1<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(0x5, 0);
                let Some(boss) = parent(x) else { return vanish(x) };
                attach(x, boss);
                x.anim_start(ANIMS, 0x1c);
                init_boxes(x);
            }
            2 => {
                x.set_a5_u16(FLAG_4BA, 0);
                let Some(boss) = parent(x) else { return vanish(x) };
                x.set_bit(0x1, 1, false);
                if x.u8(0x5) == 0 {
                    if boss.u16(x.b, PARENT_STATE) == 0x214 {
                        x.anim_step();
                        beside(x, boss);
                        hit_players(x);
                        return settle(x);
                    }
                    x.add8(0x5, 2);
                    x.anim_start(ANIMS, 0x1d);
                }
                if x.anim_step() {
                    return vanish(x);
                }
                beside(x, boss);
                hit_players(x);
                settle(x);
            }
            4 => x.add8(0x4, 2),
            6 => x.free_eshl(),
            _ => {}
        }
    }

    /// Stuck to the boss, 8 pixels in front of it and 0x28 higher; the boss advances one notch in Y.
    fn beside<R: Bench>(x: &mut Cx<'_, R>, boss: Obj) {
        let (px, pz) = (boss.u16(x.b, 0x8), boss.u16(x.b, 0xc));
        x.set16(0x8, px);
        x.set16(0xc, pz);
        let py = boss.u32(x.b, 0x10);
        x.set32(0x10, py);
        copy_flip(x, boss);
        let dx = if x.u8(f::FLIP) != 0 { neg(8) } else { 8 };
        x.add16(0x8, dx);
        x.add16(0xc, 0x28);
        let n = boss.u16(x.b, 0x12).wrapping_add(1);
        boss.set_u16(x.b, 0x12, n);
    }

    // ----- variants 2 and 3: full-screen veil -----

    /// Aligns the veil on the screen (X, Y) at height 0x108; the boss advances one notch in Y if there is one.
    fn cover_screen<R: Bench>(x: &mut Cx<'_, R>, boss: Option<Obj>) {
        let sx = x.a5_u16(SCREEN_X);
        x.set16(0x8, sx);
        x.set16(0xc, 0);
        let sy = x.a5_u32(SCREEN_Y);
        x.set32(0x10, sy);
        x.add16(0xc, 0x108);
        if let Some(boss) = boss {
            let n = boss.u16(x.b, 0x12).wrapping_add(1);
            boss.set_u16(x.b, 0x12, n);
        }
    }

    fn lock_screen<R: Bench>(x: &mut Cx<'_, R>) {
        x.set_a5_u8(SCREEN_LOCK, LOCK_TAKEN);
        init_boxes(x);
    }

    fn variant2<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(0x5, 0);
                let Some(boss) = parent(x) else { return vanish(x) };
                attach(x, boss);
                x.anim_start(ANIMS, 0x1e);
                lock_screen(x);
                crate::families::small::four_palette_blocks(x, true);
            }
            2 => {
                let boss = parent(x).filter(|b| b.u16(x.b, PARENT_STATE) == 0x215);
                let Some(boss) = boss else {
                    x.add8(0x4, 2);
                    x.add8(0x4, 2);
                    return unlock_screen(x);
                };
                x.set_bit(0x1, 1, false);
                if x.u8(0x5) == 0 {
                    if x.anim_step() {
                        x.add8(0x5, 2);
                        x.anim_start(ANIMS, 0x1f);
                        return;
                    }
                    cover_screen(x, Some(boss));
                    let d = boss.u16(x.b, 0x10).wrapping_sub(x.u16(0x10));
                    x.add16(0x10, d);
                    x.add16(0x10, 0xffff);
                    x.add16(0xc, neg(d));
                    x.add16(0xc, 1);
                } else {
                    x.set_a5_u16(FLAG_4BA, 0);
                    x.anim_step();
                    fall_check(x);
                    // original trap: the player loop left A1 after the fourth player (0xff8dde) and 0xa49fc increments
                    // the word +0x12 of this "object", not that of the boss
                    cover_screen(x, Some(Obj(0xff_8000 + PLAYERS + 4 * PLAYER_SIZE)));
                }
                settle(x);
            }
            4 => {
                x.add8(0x4, 2);
                unlock_screen(x);
            }
            6 => x.free_eshl(),
            _ => {}
        }
    }

    fn variant3<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(0x5, 0);
                x.anim_start(ANIMS, 0x23);
                lock_screen(x);
                // 0x101da in scene 0x69, 0x10158 otherwise
                let brighten = x.a5_u8(STAGE) != 0x69;
                crate::families::small::four_palette_blocks(x, brighten);
            }
            2 => {
                if x.a5_u8(STAGE) == 0x2d && s16(x.a5_u16(SCREEN_X)) >= 0x340 {
                    crate::sound::play(x.b, SCREEN_END_FX, x.o);
                    return end3(x);
                }
                if x.u8(0x5) == 0 {
                    if x.anim_step() {
                        x.add8(0x5, 2);
                        x.set16(TIMER, 0x100);
                        x.anim_start(ANIMS, 0x1f);
                        return;
                    }
                } else {
                    x.set_a5_u16(FLAG_4BA, 0);
                    let t = x.u16(TIMER).wrapping_sub(1);
                    x.set16(TIMER, t);
                    if s16(t) <= 0 {
                        return end3(x);
                    }
                    x.anim_step();
                    fall_check(x);
                }
                cover_screen(x, None);
                settle(x);
            }
            4 => {
                x.add8(0x4, 2);
                unlock_screen(x);
            }
            6 => x.free_eshl(),
            _ => {}
        }
    }

    fn end3<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        x.add8(0x4, 2);
        unlock_screen(x);
    }

    // ----- collisions with the players -----

    fn players() -> impl Iterator<Item = Obj> {
        (0..4).map(|i| Obj(0xff_8000 + PLAYERS + PLAYER_SIZE * i))
    }

    /// Player in normal play: active, alive (non-zero HP), state 2, not invulnerable.
    fn alive<R: Bench>(x: &mut Cx<'_, R>, p: Obj) -> bool {
        p.u8(x.b, 0) != 0 && p.u8(x.b, 0x7e) == 0 && p.u8(x.b, 4) == 2 && p.u8(x.b, 0x10f) == 0 && p.u16(x.b, 0x62) != 0
    }

    /// The veil hurts the players touched by the attack (boxes `+0x3e`).
    fn hit_players<R: Bench>(x: &mut Cx<'_, R>) {
        for p in players() {
            if alive(x, p) && overlap(x, p) {
                hurt(x, p);
            }
        }
    }

    /// Hurts the players who are not in a safe zone (stages 0x2e and 0x69).
    fn fall_check<R: Bench>(x: &mut Cx<'_, R>) {
        for p in players() {
            if !alive(x, p) {
                continue;
            }
            let stage = x.a5_u8(STAGE);
            let hurts = if stage == 0x2e {
                !on_safe_range(x, p)
            } else if stage == 0x69 {
                let flags = x.data8(RECT_FLAGS);
                !(flags & 2 != 0 && in_rect(x, p, SAFE_RECTS) || flags & 1 != 0 && in_rect(x, p, SAFE_RECTS + 8))
            } else {
                true
            };
            if hurts {
                hurt(x, p);
            }
        }
    }

    /// Stage 0x2e: the player is on the ground (Z < 0x100) and in one of the safe zones.
    fn on_safe_range<R: Bench>(x: &mut Cx<'_, R>, p: Obj) -> bool {
        if p.u8(x.b, 0xc) != 0 {
            return false;
        }
        let mut a2 = SAFE_RANGES;
        loop {
            let start = x.data16(a2);
            a2 += 2;
            if start == 0 {
                return false;
            }
            let d = p.u16(x.b, 0x8).wrapping_sub(start);
            if s16(d) < 0 {
                return false;
            }
            let len = x.data16(a2);
            a2 += 2;
            if s16(d) <= s16(len) {
                return true;
            }
        }
    }

    /// Stage 0x69: the player is on the ground and in the rectangle (X, width, Y, height).
    fn in_rect<R: Bench>(x: &mut Cx<'_, R>, p: Obj, rect: u32) -> bool {
        if p.u8(x.b, 0xc) != 0 {
            return false;
        }
        let px = s16(p.u16(x.b, 0x8));
        let x0 = x.data16(rect);
        if s16(x0) >= px {
            return false;
        }
        if s16(x0.wrapping_add(x.data16(rect + 2))) <= px {
            return false;
        }
        let y1 = x.data16(rect + 4).wrapping_add(x.data16(rect + 6));
        s16(y1) > s16(p.u16(x.b, 0x10))
    }

    /// Overlap of the attack boxes (row `+0x3e` - 1) and the player's (pointer according to its character number, row
    /// according to `+0x38`), axis by axis: X (mirrored according to the orientation), Z then Y. Absolute distance of
    /// the centers against the sum of the half-sizes.
    fn overlap<R: Bench>(x: &mut Cx<'_, R>, p: Obj) -> bool {
        if p.u8(x.b, 0x38) == 0 {
            return false;
        }
        let n = x.u8(0x3e);
        if n == 0 {
            return false;
        }
        let rows = ATTACK_BOXES + 16 * (n - 1) as u32;
        let character = p.u8(x.b, 3) as u32 & 0xe;
        let table = x.data32(PLAYER_BOXES + 2 * character);
        let boxes = table + 8 * p.u8(x.b, 0x38) as u32;
        for (k, pos) in [0x8u32, 0xc, 0x10].into_iter().enumerate() {
            let k = k as u32;
            let mut mine = x.data16(rows + 4 * k);
            let mut theirs = x.data8(boxes + 2 * k) as i8 as i16 as u16;
            if k == 0 {
                if x.u8(f::FLIP) != 0 {
                    mine = neg(mine);
                }
                if p.u8(x.b, f::FLIP) != 0 {
                    theirs = neg(theirs);
                }
            }
            let a = mine.wrapping_add(x.u16(pos));
            let b = theirs.wrapping_add(p.u16(x.b, pos));
            let mut d = a.wrapping_sub(b);
            if s16(d) < 0 {
                d = neg(d);
            }
            let reach = x.data16(rows + 4 * k + 2).wrapping_add(x.data8(boxes + 2 * k + 1) as u16);
            if s16(reach) < s16(d) {
                return false;
            }
        }
        true
    }

    /// Wound: the player goes to state 0x02042000; HP -= max HP / 2 (except attacks 0x2c), a player at 0 HP is removed
    /// from the count `A5+0x35`.
    fn hurt<R: Bench>(x: &mut Cx<'_, R>, p: Obj) {
        p.set_u32(x.b, 0x4, 0x0204_2000);
        let mut hp = 0u16;
        if p.u8(x.b, 0x10d) == 0x2c || p.u8(x.b, 0x10e) == 0x2c {
            hp = p.u16(x.b, 0x62);
            let half = p.u16(x.b, 0x60) >> 1;
            let below = s16(hp) < s16(half);
            hp = hp.wrapping_sub(half);
            if below {
                hp = 0;
            }
        }
        p.set_u16(x.b, 0x62, hp);
        if hp == 0 {
            let n = x.a5_u8(0x35).wrapping_sub(1);
            x.set_a5_u8(0x35, n);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Types 53 to 55
// ---------------------------------------------------------------------------------------------

/// Hidden every other frame according to the parity of the variant compared with the global phase `A5+0x6e`.
fn blink_by_variant<R: Bench>(x: &mut Cx<'_, R>) {
    let hidden = (x.u8(0x3) & 1) as u16 != x.a5_u16(0x6e) & 1;
    x.set_bit(0x1, 1, hidden);
}

/// Top of the screen (`A5+0x50a`, or `A5+0x55a` in two-player mode `A5+0x36`).
fn screen_top<R: Bench>(x: &mut Cx<'_, R>) -> u16 {
    let off = if x.a5_u8(0x36) != 0 { 0x55a } else { 0x50a };
    x.a5_u16(off)
}

/// Shot 53 (0xa4d82): projectile aiming at the player targeted by the creator of its creator. It leaves from the
/// position of its creator (`+0x5c`, itself linked to an enemy whose `+0x50` designates the target), in the direction
/// (0 to 8 approximately) given by routine 0x1376; animation and orientation read from a 2-byte table per direction.
/// Destroyed on contact (hit or parried); removed off the screen.
mod aimed53 {
    use super::*;

    const ANIMS: u32 = 0x175dc4;
    const LINK: u32 = 0x5c;
    /// For each direction: (animation, orientation).
    const DIRECTIONS: u32 = 0xa4e16;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => spawn(x),
            2 => {
                if x.u8(f::HIT_FLAGS) & 0x28 != 0 {
                    return x.free_eshl();
                }
                x.move_by_table_xy();
                x.anim_step();
                x.cull_offscreen();
                x.reinsert();
            }
            _ => x.free_eshl(),
        }
    }

    fn spawn<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        if x.u16(LINK) == 0 {
            return x.free_eshl();
        }
        x.set8(f::BOX_SET, 7);
        x.set8(f::ATTACK_ID, 0x23);
        let creator = x.linked(LINK);
        if creator.u16(x.b, LINK) == 0 {
            return x.free_eshl();
        }
        copy_xyz16(x, creator);
        let enemy = obj_of(creator.u16(x.b, LINK));
        let target = enemy.u16(x.b, f::OWNER);
        if target == 0 {
            return x.free_eshl();
        }
        let dir = crate::aids::direction_to(x.b, x.o, obj_of(target));
        x.set8(0x2d, dir);
        x.select_speed(&crate::objects::motion::SPEEDS_B, 0x24);
        let row = DIRECTIONS + 2 * dir as u32;
        let flip = x.rom8(row + 1);
        x.set8(f::FLIP, flip);
        let anim = x.rom8(row);
        x.anim_start(ANIMS, (anim as u32) as u16);
        x.reinsert();
    }
}

/// Shot 54 (0xa4eaa): salvo of projectiles launched by an object (`+0x5c`, which has a creator), 17 copies numbered by
/// the variant `+3` (created by 0xa4e84). Sub-type `+0x2f` 0: leaves the object with the offsets and speeds of row `+3`
/// of table 0xa4fe6 (18 bytes); other sub-type: 20-byte row of table 0xa51b4 (including the animation). Moves for 60
/// (or 30) frames, then 10 more frames, then its animation ends; blinks according to its variant. The shot disappears
/// as soon as its launcher no longer exists.
mod burst54 {
    use super::*;

    const ANIMS: u32 = 0x1706f6;
    const LINK: u32 = 0x5c;
    const TIMER: u32 = 0x42;
    const SOUND: u32 = 0xe4560;
    /// Rows of 9 words: DX, DZ, DY, VX, AX, VZ, AZ, VY, AY.
    const ROWS: u32 = 0xa4fe6;
    /// Rows of 10 words: the same, then the animation.
    const ROWS_ANIM: u32 = 0xa51b4;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u16(LINK) == 0 {
            return x.free_eshl();
        }
        match x.u8(0x4) {
            0 => {
                crate::sound::play(x.b, SOUND, x.o);
                x.add8(0x4, 2);
                x.set8(f::BOX_SET, 7);
                x.set8(f::ATTACK_ID, 0x23);
                let with_anim = x.u8(0x2f) != 0;
                launch(x, with_anim);
            }
            2 => {
                match x.u8(0x5) {
                    0 => {
                        let t = x.u16(TIMER).wrapping_sub(1);
                        x.set16(TIMER, t);
                        if s16(t) < 0 {
                            x.add8(0x5, 2);
                            x.set16(TIMER, 0xa);
                        }
                        x.move_xyz();
                        x.anim_step();
                    }
                    2 => {
                        let t = x.u16(TIMER).wrapping_sub(1);
                        x.set16(TIMER, t);
                        if s16(t) < 0 {
                            x.add8(0x4, 2);
                        }
                        x.anim_step();
                    }
                    _ => {}
                }
                blink_by_variant(x);
                x.reinsert();
            }
            _ => x.free_eshl(),
        }
    }

    /// Places itself on the launcher and takes the speeds of the variant's row.
    fn launch<R: Bench>(x: &mut Cx<'_, R>, with_anim: bool) {
        let src = x.linked(LINK);
        for off in [0x8, 0x10, 0xc] {
            let v = src.u32(x.b, off);
            x.set32(off, v);
        }
        copy_flip(x, src);
        x.add16(0x10, 0xffff);
        x.set16(TIMER, if with_anim { 0x1e } else { 0x3c });
        let flip = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
        let v = x.u8(0x3) as u32;
        let row = if with_anim { ROWS_ANIM + 20 * v } else { ROWS + 18 * v };
        let word = |x: &mut Cx<'_, R>, i: u32| x.rom16(row + 2 * i);
        let (mut dx, dz, dy, mut vx, mut ax) = (word(x, 0), word(x, 1), word(x, 2), word(x, 3), word(x, 4));
        for (i, off) in [0x18u32, 0x1a, 0x1c, 0x1e].into_iter().enumerate() {
            let w = word(x, 5 + i as u32);
            x.set16(off, w);
        }
        if flip != 0 {
            dx = neg(dx);
            vx = neg(vx);
            ax = neg(ax);
        }
        x.add16(0x8, dx);
        x.add16(0xc, dz);
        x.add16(0x10, dy);
        x.set16(0x14, vx);
        x.set16(0x16, ax);
        let anim = if with_anim { word(x, 9) as u32 } else { 0 };
        x.anim_start(ANIMS, anim as u16);
    }
}

/// Shot 55 (0xa5358): projectile that falls from the top of the screen, thrown to one side (variant 0: to the left; X
/// and depth come from `+0x58` / `+0x5a`, set by the creator). It goes diagonally (VX = ±0x800, VZ = -0xc00) to the
/// ground (Z <= 0), then stays 48 frames playing its second animation, blinking during the last 32. Motionless when
/// paused (`A5+0x3d`).
mod fall55 {
    use super::*;

    const ANIMS: u32 = 0x170c3e;
    const SOUND: u32 = 0xeb0c0;
    const X_START: u32 = 0x58;
    const Y_START: u32 = 0x5a;
    const TIMER: u32 = 0x56;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => {
                spawn(x);
                x.reinsert();
                crate::sound::play(x.b, SOUND, x.o);
            }
            2 => {
                if x.a5_u8(GAME_PAUSED) == 0 {
                    match x.u8(0x5) {
                        0 => fall(x),
                        2 => land(x),
                        _ => {}
                    }
                }
                x.reinsert();
            }
            _ => x.free_eshl(),
        }
    }

    fn spawn<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        x.set8(f::BOX_SET, 7);
        x.set8(f::ATTACK_ID, 0x2d);
        let (px, py) = (x.u16(X_START), x.u16(Y_START));
        x.set16(0x8, px);
        x.set16(0x10, py);
        let top = screen_top(x);
        x.set16(0xc, top.wrapping_add(0x100).wrapping_sub(py));
        x.set16(0x14, neg(0x800));
        x.set16(0x18, neg(0xc00));
        let mut dx = 0x80;
        if x.u8(0x3) == 0 {
            dx = neg(dx);
            let flip = x.u8(f::FLIP) ^ 0x20;
            x.set8(f::FLIP, flip);
            let vx = neg(x.u16(0x14));
            x.set16(0x14, vx);
        }
        x.add16(0x8, dx);
        x.anim_start(ANIMS, 0);
    }

    fn fall<R: Bench>(x: &mut Cx<'_, R>) {
        if x.move_xz().gt() {
            return;
        }
        x.add8(0x5, 2);
        x.set32(0xc, 0);
        x.set16(TIMER, 0x30);
        x.anim_start(ANIMS, 1);
    }

    fn land<R: Bench>(x: &mut Cx<'_, R>) {
        let t = x.u16(TIMER).wrapping_sub(1);
        x.set16(TIMER, t);
        if t == 0 {
            x.add8(0x4, 2);
        }
        if s16(x.u16(TIMER)) <= 0x20 {
            x.set_bit(0x1, 1, true);
            let visible = x.u16(TIMER).wrapping_add(x.counter() as u16) & 1 != 0;
            if visible {
                x.set_bit(0x1, 1, false);
            }
        }
        x.anim_step();
    }
}

// ---------------------------------------------------------------------------------------------
// Types 56 to 58
// ---------------------------------------------------------------------------------------------

/// The four players (records of 0x200 bytes starting at `A5+0x5de`).
fn players() -> impl Iterator<Item = Obj> {
    (0..4).map(|i| Obj(0xff_8000 + 0x5de + 0x200 * i))
}

/// Shot 56 (0xa5466): objects to pick up (12 variants, two per object type: the even variant uses palette 0x14 and
/// attack identifier 0x21, the odd one 0x13 and 0x22). Placed on the screen, they animate (blink according to `A5+0x6f`
/// and `D7`, except during `A5+0x33`); a player on the ground within 8 pixels in X and 6 in depth picks them up: the
/// object blinks for 30 frames (`+0x56`) then plays its pickup animation (types 0 to 5: animation 3 and sound 0xeb132
/// for variants 0 and 1), or, for variants 6 and 7, waits a delay drawn from a table before replaying animation 5;
/// variants 8 and 9 depend on the hit points (`+0x62`) of their creator (`+0x5c`). Disappears when it is more than
/// 0x200 pixels to the left of the screen (except stage 0x19).
mod pickup56 {
    use super::*;

    const ANIMS: u32 = 0x170c3e;
    /// Creation animation per object type (variant / 2).
    const INIT_ANIMS: [u32; 6] = [2, 4, 5, 5, 4, 7];
    /// Delays (16 words) drawn at random for variants 6 and 7.
    const DELAYS: u32 = 0xa55b2;
    const PICKUP_SOUND: u32 = 0xeb132;
    const COUNTDOWN: u32 = 0x56;
    /// Variable `A5+0x6f`: blinking phase; `A5+0x33`: blinking suspended.
    const BLINK_PHASE: u32 = 0x6f;
    const BLINK_OFF: u32 = 0x33;
    const STAGE: u32 = 0x34;
    const STAGE_NO_CULL: u8 = 0x19;
    const LINK: u32 = 0x5c;
    const HIDDEN: u32 = 0x40;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => {
                let v = x.u8(0x3);
                if v < 12 {
                    init(x, v);
                }
                x.reinsert();
            }
            2 => live(x),
            _ => x.free_eshl(),
        }
    }

    fn init<R: Bench>(x: &mut Cx<'_, R>, v: u8) {
        let k = v as usize / 2;
        x.add8(0x4, 2);
        x.set8(f::BOX_SET, 7);
        x.set8(0x2a, 1);
        x.set8(0x2b, 0x14);
        x.set8(f::ATTACK_ID, 0x21);
        if v != 2 * k as u8 {
            x.set8(0x2b, 0x13);
            x.set8(f::ATTACK_ID, 0x22);
        }
        x.anim_start(ANIMS, (INIT_ANIMS[k]) as u16);
        if k == 3 {
            random_delay(x);
        }
    }

    /// Delay drawn from table 0xa55b2.
    fn random_delay<R: Bench>(x: &mut Cx<'_, R>) {
        let r = random(x) & 0xf;
        let d = x.rom16(DELAYS + 2 * r);
        x.set16(COUNTDOWN, d);
    }

    fn live<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(HIDDEN) != 0 {
            return x.reinsert();
        }
        let v = x.u8(0x3);
        match v / 2 {
            0 => touch_then_pick(x, true),
            1 => flash_animate(x),
            2 => touch_then_pick(x, false),
            3 => wait_and_replay(x),
            4 => hp_dependent(x),
            5 => flash_animate(x),
            _ => {}
        }
        if v < 12 && x.a5_u8(STAGE) != STAGE_NO_CULL {
            let off = if x.a5_u8(0x36) != 0 { 0x556 } else { 0x506 };
            let edge = x.a5_u16(off).wrapping_sub(0x200);
            if !(s16(edge) < s16(x.u16(0x8))) {
                x.set8(0x4, 6);
            }
        }
        x.reinsert();
    }

    /// Makes the object visible then hides it every other frame (except if `A5+0x33`).
    fn flash<R: Bench>(x: &mut Cx<'_, R>) {
        x.set_bit(0x1, 1, false);
        if x.a5_u8(BLINK_OFF) == 0 && x.a5_u8(BLINK_PHASE).wrapping_add(x.counter() as u8) & 1 != 0 {
            x.set_bit(0x1, 1, true);
        }
    }

    fn flash_animate<R: Bench>(x: &mut Cx<'_, R>) {
        flash(x);
        x.anim_step();
    }

    /// A player on the ground, close enough, picks up the object: next `+5`, 30-frame countdown.
    fn pickup_touch<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(HIDDEN) != 0 || x.a5_u8(GAME_PAUSED) != 0 {
            return;
        }
        for p in players() {
            if p.u8(x.b, 0) == 0 || p.u16(x.b, 0xc) != 0 {
                continue;
            }
            let dy = x.u16(0x10).wrapping_sub(p.u16(x.b, 0x10)).wrapping_add(6);
            if dy > 0xc {
                continue;
            }
            let dx = x.u16(0x8).wrapping_sub(p.u16(x.b, 0x8)).wrapping_add(8);
            if dx > 0x10 {
                continue;
            }
            x.add8(0x5, 2);
            x.set16(COUNTDOWN, 0x1e);
            return;
        }
    }

    /// Pickup countdown: blinks every 8 steps; `true` when it has elapsed.
    fn pick_countdown<R: Bench>(x: &mut Cx<'_, R>) -> bool {
        let t = x.u16(COUNTDOWN).wrapping_sub(1);
        x.set16(COUNTDOWN, t);
        if t == 0 {
            x.add8(0x5, 2);
            x.set8(0x1, 1);
            return true;
        }
        if t & 7 == 0 {
            let flags = x.u8(0x1);
            x.set8(0x1, flags ^ 2);
        }
        false
    }

    /// Variants 0, 1 (`with_effects`) and 4, 5: wait for the pickup, blink, play the end animation.
    fn touch_then_pick<R: Bench>(x: &mut Cx<'_, R>, with_effects: bool) {
        match x.u8(0x5) {
            0 => pickup_touch(x),
            2 => {
                if pick_countdown(x) && with_effects {
                    x.anim_start(ANIMS, 3);
                    crate::sound::play(x.b, PICKUP_SOUND, x.o);
                }
            }
            4 => {
                if x.anim_step() {
                    x.add8(0x4, 2);
                }
            }
            _ => {}
        }
    }

    /// Variants 6, 7: wait for a delay (outside pause), then replay animation 5 with a new delay.
    fn wait_and_replay<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(0x5) == 0 {
            if x.u8(HIDDEN) != 0 || x.a5_u8(GAME_PAUSED) != 0 {
                return;
            }
            let t = x.u16(COUNTDOWN).wrapping_sub(1);
            x.set16(COUNTDOWN, t);
            if t == 0 {
                x.add8(0x5, 2);
            }
        } else if x.anim_step() {
            x.set8(0x5, 0);
            random_delay(x);
            x.anim_start(ANIMS, 5);
        }
    }

    /// Variants 8, 9: alive as long as the creator's HP are non-zero; at its death, animation 6, then blinks for 32
    /// frames before disappearing.
    fn hp_dependent<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x5) {
            0 => {
                let maker = x.linked(LINK);
                if maker.u16(x.b, 0x62) != 0 {
                    flash_animate(x);
                } else {
                    x.add8(0x5, 2);
                    x.anim_start(ANIMS, 6);
                }
            }
            2 => {
                if x.anim_step() {
                    x.add8(0x5, 2);
                    x.set16(COUNTDOWN, 0x20);
                }
                flash(x);
            }
            4 => {
                x.set_bit(0x1, 1, false);
                let t = x.u16(COUNTDOWN).wrapping_sub(1);
                x.set16(COUNTDOWN, t);
                if t == 0 {
                    x.add8(0x4, 2);
                }
                if x.u16(COUNTDOWN) & 1 != 0 {
                    x.set_bit(0x1, 1, true);
                }
            }
            _ => {}
        }
    }
}

/// Shot 57 (0xa593c): homing projectile of a creator (`+0x5c`): placed 0x4a pixels in front of it and 0x32 higher, it
/// aims at the creator's target (`+0x50` of the creator), heads toward it through the speed table (direction recomputed
/// every 20 frames) and explodes into 16 bursts (type 58) when it is within 24 pixels of the target or when its 360
/// frames (`+0x46`) have elapsed. If its creator dies or loses its target, it blinks and disappears.
mod seeker57 {
    use super::*;

    const ANIMS: u32 = 0x17100e;
    const LINK: u32 = 0x5c;
    const LIFE: u32 = 0x46;
    const RETARGET: u32 = 0x42;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u16(LINK) == 0 {
            return x.set32(0x4, 0x0600_0000);
        }
        match x.u8(0x4) {
            0 => init(x),
            2 => live(x),
            4 => fade(x),
            _ => x.free_eshl(),
        }
    }

    fn init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        x.set8(f::BOX_SET, 7);
        x.set8(f::ATTACK_ID, 7);
        x.set8(f::ATTACK_POWER, 5);
        let maker = x.linked(LINK);
        copy_xyz16(x, maker);
        let flip = copy_flip(x, maker);
        x.add16(0x10, 0xffff);
        x.set8(RETARGET, 0x14);
        x.set16(LIFE, 0x168);
        x.add16(0x8, if flip != 0 { 0x4a } else { neg(0x4a) });
        x.add16(0xc, 0x32);
        x.anim_start(ANIMS, 0);
        x.select_speed(&crate::objects::motion::SPEEDS_B, 7);
    }

    fn live<R: Bench>(x: &mut Cx<'_, R>) {
        let maker = x.linked(LINK);
        if maker.u16(x.b, 0x4) == 0x406 || maker.u16(x.b, f::OWNER) == 0 {
            x.set32(0x4, 0x0400_0000);
            x.set8(RETARGET, 0x1e);
            return;
        }
        if x.u8(0x5) == 0 {
            track(x, maker);
        }
        x.reinsert();
    }

    fn track<R: Bench>(x: &mut Cx<'_, R>, maker: Obj) {
        let left = x.u16(LIFE).wrapping_sub(1);
        x.set16(LIFE, left);
        if s16(left) < 0 {
            return explode(x);
        }
        let n = x.u8(RETARGET).wrapping_sub(1);
        x.set8(RETARGET, n);
        let mut aim = maker;
        if s8(n) < 0 {
            x.set8(RETARGET, 0x14);
            let w = x.linked(LINK).u16(x.b, f::OWNER);
            if w == 0 {
                return approach(x);
            }
            aim = obj_of(w);
            let dir = crate::aids::direction_to(x.b, x.o, aim);
            x.set8(0x2d, dir);
        }
        let dx = aim.u16(x.b, 0x8).wrapping_sub(x.u16(0x8)).wrapping_add(8);
        if dx > 0x20 {
            return approach(x);
        }
        let dy = aim.u16(x.b, 0x10).wrapping_sub(x.u16(0x10)).wrapping_add(8);
        if dy > 0x20 {
            return approach(x);
        }
        explode(x);
    }

    fn approach<R: Bench>(x: &mut Cx<'_, R>) {
        x.move_by_table_xy();
        x.anim_step();
    }

    /// Creates 16 bursts (type 58, variants 15 down to 0) then goes to state 6.
    fn explode<R: Bench>(x: &mut Cx<'_, R>) {
        for n in (0..16u8).rev() {
            let Some(c) = x.alloc(&pools::ESHL) else { break };
            c.set_u8(x.b, 0, 1);
            c.set_u8(x.b, 2, 0x3a);
            c.set_u8(x.b, 3, n);
            c.set_u16(x.b, LINK, x.o.0 as u16);
        }
        x.add8(0x4, 4);
    }

    fn fade<R: Bench>(x: &mut Cx<'_, R>) {
        let t = x.u16(RETARGET).wrapping_sub(1);
        x.set16(RETARGET, t);
        if s16(t) < 0 {
            return x.set8(0x4, 6);
        }
        let hidden = x.u16(RETARGET) & 3 != 0;
        x.set_bit(0x1, 1, hidden);
    }
}

/// Shot 58 (0xa5ad6): burst of explosion 57 (16 variants from the 8-byte-per-variant table: duration `+0x42`, sub-type
/// `+0x2f`, speed index `+0x2d`, animation, orientation, speed table). Leaves from the position of its creator and
/// moves through the speed table (XY or XZ according to `+0x2f`) for its duration, blinking according to its variant.
mod shard58 {
    use super::*;

    const ANIMS: u32 = 0x1706f6;
    const LINK: u32 = 0x5c;
    const ROWS: u32 = 0xa5bd6;
    const TIMER: u32 = 0x42;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u16(LINK) == 0 {
            return x.set32(0x4, 0x0600_0000);
        }
        match x.u8(0x4) {
            0 => init(x),
            2 => {
                if x.u8(0x5) == 0 {
                    let t = x.u16(TIMER).wrapping_sub(1);
                    x.set16(TIMER, t);
                    if s16(t) < 0 {
                        x.add8(0x4, 2);
                    }
                    x.anim_step();
                    if x.u8(0x2f) == 0 {
                        x.move_by_table_xy();
                    } else {
                        x.move_by_table_xz();
                    }
                }
                blink_by_variant(x);
                x.reinsert();
            }
            _ => x.free_eshl(),
        }
    }

    fn init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        x.set8(f::BOX_SET, 7);
        x.set8(f::ATTACK_ID, 0x23);
        let maker = x.linked(LINK);
        copy_xyz32(x, maker);
        copy_flip(x, maker);
        x.add16(0x10, 0xffff);
        let flip = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
        let row = ROWS + 8 * x.u8(0x3) as u32;
        let timer = x.rom16(row);
        let subtype = x.rom8(row + 2);
        let speed_index = x.rom8(row + 3);
        let anim = x.rom8(row + 4);
        let new_flip = x.rom8(row + 5);
        let speeds = x.rom16(row + 6);
        x.set16(TIMER, timer);
        x.set8(0x2d, speed_index);
        x.set8(f::FLIP, new_flip);
        x.set8(0x2f, subtype);
        x.anim_start(ANIMS, (anim as u32) as u16);
        x.select_speed(&crate::objects::motion::SPEEDS_B, (speeds as u32) as u16);
    }
}

// ---------------------------------------------------------------------------------------------
// Types 59 to 62
// ---------------------------------------------------------------------------------------------

/// Shot 59 (0xa5cc2): effect ring of a creator (`+0x5c`), two sub-types `+0x2f`. Sub-type 0: placed on the creator,
/// with the creator's owner (`+0x50`), plays animation 1, then 2 (300 frames), 3, and when it ends triggers effect
/// 0xe448c; disappears when the owner is in state 0x406. Sub-type 1: placed near the creator (offset per variant), it
/// plays its animation and sets the creator's byte `+0xae` to 1 at the end. The palette changes every 3 frames.
mod ring59 {
    use super::*;

    const ANIMS: u32 = 0x171026;
    const LINK: u32 = 0x5c;
    const TIMER: u32 = 0x42;
    /// Offsets (X, Y) per variant for sub-type 1 (4 bytes per row).
    const OFFSETS: u32 = 0xa5db2;
    /// Value of `+0x2b` (palette (h)) by `+0x47`.
    const PALETTES: u32 = 0xa5f30;
    const FX_START_0: u32 = 0xe4422;
    const FX_START_1: u32 = 0xe44f6;
    const FX_END: u32 = 0xe448c;
    const PALETTE_COUNTDOWN: u32 = 0x46;
    const PALETTE_INDEX: u32 = 0x47;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => start(x),
            2 => {
                match x.u8(0x2f) {
                    0 => ring(x),
                    1 => satellite(x),
                    _ => {}
                }
                x.reinsert();
            }
            _ => x.free_eshl(),
        }
    }

    fn start<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u16(LINK) == 0 {
            return;
        }
        x.add8(0x4, 2);
        match x.u8(0x2f) {
            0 => {
                x.set8(f::BOX_SET, 7);
                x.set8(f::ATTACK_ID, 0x24);
                x.set8(PALETTE_COUNTDOWN, 2);
                crate::sound::play(x.b, FX_START_0, x.o);
                let maker = x.linked(LINK);
                copy_xyz32(x, maker);
                copy_flip(x, maker);
                x.add16(0x10, 1);
                x.set16(0xc, 0);
                let owner = maker.u16(x.b, LINK);
                x.set16(f::OWNER, owner);
                x.add16(0x10, neg(4));
                x.anim_start(ANIMS, 1);
            }
            1 => {
                x.set8(f::BOX_SET, 7);
                x.set8(f::ATTACK_ID, 0x24);
                x.set8(PALETTE_COUNTDOWN, 2);
                crate::sound::play(x.b, FX_START_1, x.o);
                let maker = x.linked(LINK);
                let (mx, my) = (maker.u16(x.b, 0x8), maker.u16(x.b, 0x10));
                let z = maker.u32(x.b, 0xc);
                x.set32(0xc, z);
                copy_flip(x, maker);
                x.set16(0xc, 0);
                let row = OFFSETS + 4 * x.u8(0x3) as u32;
                let (dx, dy) = (x.rom16(row), x.rom16(row + 2));
                x.set16(0x8, dx.wrapping_add(mx));
                x.set16(0x10, dy.wrapping_add(my));
                x.anim_start(ANIMS, 0);
            }
            _ => {}
        }
    }

    /// Sub-type 0: follows the owner.
    fn ring<R: Bench>(x: &mut Cx<'_, R>) {
        let owner = x.linked(f::OWNER);
        if owner.u16(x.b, 0x4) == 0x406 {
            return x.free_eshl();
        }
        match x.u8(0x5) {
            0 => {
                if x.anim_step() {
                    x.add8(0x5, 2);
                    x.set16(TIMER, 0x12c);
                    x.anim_start(ANIMS, 2);
                }
            }
            2 => {
                let t = x.u16(TIMER).wrapping_sub(1);
                x.set16(TIMER, t);
                if s16(t) >= 0 {
                    x.anim_step();
                } else {
                    x.add8(0x5, 2);
                    x.anim_start(ANIMS, 3);
                }
            }
            4 => {
                if x.anim_step() {
                    x.add8(0x4, 2);
                    crate::sound::play(x.b, FX_END, x.o);
                }
            }
            _ => {}
        }
        cycle_palette(x);
    }

    /// Sub-type 1: animation then signal to the creator.
    fn satellite<R: Bench>(x: &mut Cx<'_, R>) {
        let maker = x.linked(LINK);
        if maker.u16(x.b, 0x4) == 0x406 {
            return x.free_eshl();
        }
        if x.anim_step() {
            x.add8(0x4, 2);
            let maker = x.linked(LINK);
            maker.set_u8(x.b, 0xae, 1);
        }
        cycle_palette(x);
        blink_by_variant(x);
    }

    fn cycle_palette<R: Bench>(x: &mut Cx<'_, R>) {
        let n = x.u8(PALETTE_COUNTDOWN).wrapping_sub(1);
        x.set8(PALETTE_COUNTDOWN, n);
        if s8(n) >= 0 {
            return;
        }
        x.set8(PALETTE_COUNTDOWN, 2);
        x.set8(0x2a, 1);
        let i = x.u8(PALETTE_INDEX).wrapping_add(1) & 7;
        x.set8(PALETTE_INDEX, i);
        let p = x.rom8(PALETTES + i as u32);
        x.set8(0x2b, p);
    }
}

/// Shot 60 (0xa5f5c): mobile platform that rises and descends with the screen (height corrected by the vertical scroll
/// `A5+0x50a`/`0x55a`), 4 variants. Cycle of `+5`: wait (variant 0: `+0x42` frames, variant 1: a player climbs on it
/// (0x7f19c), variant 3: same with `A5+0x51e` = 4, variant 2: the creator is in state 0x204), rise (VZ = -0x100, AZ =
/// -0x20, actually descend: Z decreases) until Z <= 0x20 (0x38 if sub-type `+0x2f`), wait 16 frames then go back up (VZ
/// = 0x180) to the starting height (`+0x4e`), and start over. The drawing layer `+0x3b` depends on the height. Creates
/// a player shot of type 0x18 (0x54872).
mod platform60 {
    use super::*;

    const ANIMS: u32 = 0x1712d4;
    const LINK: u32 = 0x5c;
    const TIMER: u32 = 0x42;
    const RELOAD: u32 = 0x4c;
    const HOME_Z: u32 = 0x4e;
    /// Last vertical scroll (word), and its new reading.
    const LAST_SCROLL: u32 = 0x16;
    const SCROLL: u32 = 0x14;
    const THUD: u32 = 0xeb04e;
    /// Starting positions of variant 2 per sub-type (4 bytes per row).
    const START_POSITIONS: u32 = 0xa6046;
    /// Variable `A5+0x51e` (variant 3).
    const SIGNAL: u32 = 0x51e;
    /// Substates (`+5`).
    const WAIT: u8 = 0;
    const SINK: u8 = 2;
    const REST: u8 = 4;
    const RISE: u8 = 6;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        if suspended(x) {
            return;
        }
        match x.u8(0x4) {
            0 => start(x),
            2 => live(x),
            _ => x.free_eshl(),
        }
    }

    fn start<R: Bench>(x: &mut Cx<'_, R>) {
        let top = screen_top(x);
        x.set16(LAST_SCROLL, top);
        match x.u8(0x3) {
            2 => {
                if x.u16(LINK) == 0 {
                    return;
                }
                x.add8(0x4, 2);
                let row = START_POSITIONS + 4 * x.u8(0x2f) as u32;
                let (px, py) = (x.rom16(row), x.rom16(row + 2));
                x.set16(0x8, px);
                x.set16(0x10, py);
                x.set16(0xc, 0x110);
                x.set16(HOME_Z, 0x110);
                x.set16(TIMER, 0x1e);
                x.set16(RELOAD, 0x1e);
            }
            0 | 1 | 3 => {
                x.add8(0x4, 2);
                x.set16(0xc, 0x110);
                x.set16(HOME_Z, 0x110);
                let t = x.u16(TIMER);
                x.set16(RELOAD, t);
            }
            _ => return,
        }
        x.set8(f::BOX_SET, 7);
        x.set8(f::ATTACK_ID, 0x17);
        x.anim_start(ANIMS, 0);
        crate::families::small::shot_18(&mut crate::families::effects_creators::leftovers::Pure, x.b, x.o, 2);
        x.reinsert();
    }

    fn live<R: Bench>(x: &mut Cx<'_, R>) {
        // follows the vertical scroll of the screen
        let top = screen_top(x);
        x.set16(SCROLL, top);
        let delta = top.wrapping_sub(x.u16(LAST_SCROLL));
        x.add16(0xc, delta);
        x.set16(LAST_SCROLL, top);
        match x.u8(0x3) {
            0 => cycle(x, Trigger::Timer),
            1 => cycle(x, Trigger::Player { signal: false }),
            3 => cycle(x, Trigger::Player { signal: true }),
            2 => {
                if x.u16(LINK) != 0 {
                    x.set8(0x1c, 0);
                    let maker = x.linked(LINK);
                    if maker.u16(x.b, 0x4) == 0x204 {
                        x.set8(0x1c, 1);
                    }
                    cycle(x, Trigger::Maker);
                }
            }
            _ => {}
        }
        let layer = if x.u8(0x5) == 0 {
            0
        } else {
            let z = s16(x.u16(0xc));
            if z >= 0xb0 {
                0x60
            } else if z >= 0x60 {
                0x5f
            } else {
                0xc
            }
        };
        x.set8(0x3b, layer);
    }

    /// What triggers the descent.
    #[derive(Clone, Copy, PartialEq)]
    enum Trigger {
        Timer,
        Player { signal: bool },
        Maker,
    }

    fn cycle<R: Bench>(x: &mut Cx<'_, R>, trigger: Trigger) {
        match x.u8(0x5) {
            WAIT => {
                let go = match trigger {
                    Trigger::Timer => {
                        let t = x.u16(TIMER).wrapping_sub(1);
                        x.set16(TIMER, t);
                        t == 0
                    }
                    Trigger::Player { signal } => {
                        let on = player_on(x);
                        if on && signal {
                            x.set_a5_u16(SIGNAL, 4);
                        }
                        on
                    }
                    Trigger::Maker => x.u8(0x1c) == 0 && {
                        let t = x.u16(TIMER).wrapping_sub(1);
                        x.set16(TIMER, t);
                        t == 0
                    },
                };
                if go {
                    x.add8(0x5, 2);
                    x.set16(0x18, neg(0x100));
                    x.set16(0x1a, neg(0x20));
                }
            }
            SINK => {
                x.move_z();
                let floor = if trigger == Trigger::Maker || x.u8(0x2f) == 0 { 0x20 } else { 0x38 };
                if s16(x.u16(0xc)) <= floor {
                    crate::sound::play(x.b, THUD, x.o);
                    x.add8(0x5, 2);
                    x.set16(TIMER, 0x10);
                }
            }
            REST => {
                let t = x.u16(TIMER).wrapping_sub(1);
                x.set16(TIMER, t);
                if t == 0 {
                    x.add8(0x5, 2);
                    x.set16(0x18, 0x180);
                    x.set16(0x1a, 0);
                }
            }
            RISE => {
                x.move_z();
                if s16(x.u16(HOME_Z)) > s16(x.u16(0xc)) {
                    return;
                }
                let n = x.u8(0x5).wrapping_sub(6);
                x.set8(0x5, n);
                match trigger {
                    Trigger::Timer | Trigger::Maker => {
                        let reload = x.u16(RELOAD);
                        x.set16(TIMER, reload);
                    }
                    Trigger::Player { signal: true } => x.set_a5_u16(SIGNAL, 0),
                    Trigger::Player { signal: false } => {}
                }
            }
            _ => {}
        }
    }

    /// Is a player (other than character 5) on the platform? (collision 0x7f19c.)
    fn player_on<R: Bench>(x: &mut Cx<'_, R>) -> bool {
        for p in players() {
            let character = p.u8(x.b, 3) >> 1;
            if character == 5 || p.u8(x.b, 0) != 1 {
                continue;
            }
            let hitbox = crate::ai_aids::Hitbox { offset: 0, half_width: 0x28, half_depth: 8 };
            if crate::ai_aids::box_around(x.b, x.o, p, hitbox) != crate::ai_aids::InHitbox::No {
                return true;
            }
        }
        false
    }
}

/// Shot 61 (0xa63d8): marker stuck to its creator (`+0x5c`, of variant 0 to 2 which chooses one of the three offset
/// tables; 20 bytes per marker variant, 4 per sub-type `+0x2f`), which plays its animation and, at the end, sets the
/// creator's byte `+0x1c` to 1.
mod marker61 {
    use super::*;

    const ANIMS: u32 = 0x171374;
    const LINK: u32 = 0x5c;
    /// Offset tables (X, Y) according to the creator's variant.
    const OFFSET_TABLES: [u32; 3] = [0xa6538, 0xa65de, 0xa6670];

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        if suspended(x) {
            return;
        }
        match x.u8(0x4) {
            0 => {
                if x.u16(LINK) == 0 {
                    return;
                }
                let maker = x.linked(LINK);
                let kind = maker.u8(x.b, 0x3) as usize;
                let Some(&table) = OFFSET_TABLES.get(kind) else { return };
                x.add8(0x4, 2);
                x.set8(f::BOX_SET, 7);
                x.set8(f::ATTACK_ID, 0x27);
                place(x, table);
                x.anim_start(ANIMS, 0);
                x.reinsert();
            }
            2 => {
                if x.u16(LINK) == 0 {
                    return;
                }
                let maker = x.linked(LINK);
                if maker.u8(x.b, 0x3) > 2 || x.u8(0x5) != 0 {
                    return;
                }
                if x.anim_step() {
                    x.add8(0x4, 2);
                    let maker = x.linked(LINK);
                    maker.set_u8(x.b, 0x1c, 1);
                }
            }
            _ => x.free_eshl(),
        }
    }

    /// Settles on the creator with the offset from the table (mirrored according to the creator's orientation).
    fn place<R: Bench>(x: &mut Cx<'_, R>, table: u32) {
        if x.u16(LINK) == 0 {
            return;
        }
        let maker = x.linked(LINK);
        copy_xyz16(x, maker);
        let v = x.u8(0x3) as u32;
        let row = table + 20 * v + 4 * x.u8(0x2f) as u32;
        let (mut dx, dy) = (x.rom16(row), x.rom16(row + 2));
        if maker.u8(x.b, f::FLIP) != 0 {
            dx = neg(dx);
        }
        x.add16(0x8, dx);
        x.add16(0x10, dy);
    }
}

/// Shot 62 (0xa66ea): meteor that falls on the terrain near the camera. Position drawn at random around the screen (X
/// within 0x1ff + 0x40, Y within 0x7f) at height 0x100, delayed by `+0x42` frames (table according to the variant),
/// falls (VZ = -0x200, AZ = -0x20) and, on touching the ground (Z < 0), triggers the impact effects (0xb9c88, 0xab7d8
/// with 3, 0x2860 with 8, 0xeafdc).
mod meteor62 {
    use super::*;

    const ANIMS: u32 = 0x16ea18;
    /// Delay before the fall (bytes) per variant (32 entries).
    const DELAYS: u32 = 0xa679e;
    const IMPACT_SOUND: u32 = 0xeafdc;
    const TIMER: u32 = 0x42;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => start(x),
            2 => live(x),
            4 => x.add8(0x4, 2),
            _ => x.free_eshl(),
        }
    }

    fn start<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        x.set8(0x5, 0);
        x.set8(f::BOX_SET, 7);
        x.set8(f::ATTACK_ID, 0x25);
        // camera: block of A5+0x4fe (player 1) or A5+0x54e (player 2)
        let camera = if x.a5_u8(0x36) != 0 { 0x54e } else { 0x4fe };
        x.set16(0xc, 0x100);
        let mut dx = random(x) as u16 & 0x1ff;
        if s16(dx) > 0x180 {
            dx = (dx >> 1) + 0x40;
        }
        let px = dx.wrapping_add(x.a5_u16(camera + 8));
        x.set16(0x8, px);
        let mut dy = random(x) as u16 & 0x7f;
        if s16(dy) > 0x50 {
            dy = (dy >> 1) + 0x10;
        }
        let py = dy.wrapping_add(0x10).wrapping_add(x.a5_u16(camera + 0xc));
        x.set16(0x10, py);
        x.set16(0x18, neg(0x200));
        x.set16(0x1a, neg(0x20));
        x.set32(0x14, 0);
        x.set32(0x1c, 0);
        let variant = x.u8(0x3) & 0x1f;
        let delay = x.rom8(DELAYS + variant as u32);
        x.set8(TIMER, delay);
        x.anim_start(ANIMS, 0);
    }

    fn live<R: Bench>(x: &mut Cx<'_, R>) {
        if s8(x.u8(TIMER)) > 0 {
            let t = x.u8(TIMER) - 1;
            return x.set8(TIMER, t);
        }
        x.anim_step();
        x.move_z();
        if s16(x.u16(0xc)) < 0 {
            crate::families::effects_creators::linked(x.b, x.o, crate::families::effects_creators::kind::WISP, Some(1));
            x.set16(0xc, 0);
            x.add8(0x4, 2);
            crate::families::effects_creators::debris(x.b, x.o, 3);
            crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::B, 8);
            crate::sound::play(x.b, IMPACT_SOUND, x.o);
        }
        x.keep_on_screen(0x10);
        x.reinsert();
    }
}

// ---------------------------------------------------------------------------------------------
// Type 63
// ---------------------------------------------------------------------------------------------

/// Shot 63 (0xa6864): hand of a boss (`+0x5c`, whose byte `+0xac` gives the phase: < 1 rest, 1 attack, > 1 end) that
/// follows the boss (2 pixels per frame toward the target position: 0x30 in front of it, same depth) and plays
/// animations 3 (rest) and 5 (attack). At rest, `+0x42` counts the frames before drawing lots (table 0xa6c54 according
/// to the number of players `A5+0x35` and the state `A5+0xff92`) whether it attacks when the target is on its side; an
/// attack launched by the boss (phase 1) creates a shot of type 53 at the end of its animation. Substates: `+5` (0
/// rest, 4 attack) and `+6` (steps of 4).
mod hand63 {
    use super::*;

    const ANIMS: u32 = 0x175da6;
    const LINK: u32 = 0x5c;
    const DELAY: u32 = 0x42;
    /// Pointer table (per number of players) to 32 delays (words).
    const DELAY_TABLES: u32 = 0xa6b0e;
    /// Pointer table (per number of players) to 32-bit masks indexed by `FFFF92`.
    const GATE_TABLES: u32 = 0xa6c54;
    const PLAYER_COUNT: u32 = 0x35;
    const GATE_INDEX: u32 = 0x7f92;
    /// Phase of the boss.
    const BOSS_PHASE: u32 = 0xac;
    /// Distance in front of the boss and approach speed.
    const REACH: u16 = 0x30;
    const SPEED: u16 = 2;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set16(0xc, 0x30);
                let boss = x.linked(LINK);
                let (bx, by) = (boss.u16(x.b, 0x8), boss.u16(x.b, 0x10));
                x.set16(0x8, bx);
                x.set16(0x10, by);
                resting_or_attacking(x);
            }
            2 => resting_or_attacking(x),
            4 => striking(x),
            _ => x.free_eshl(),
        }
    }

    /// Phase 1 / 2 (`+4 = 2`), then put back in the sorted list.
    fn resting_or_attacking<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(0x5) == 0 {
            match x.u8(0x6) {
                0 => {
                    x.add8(0x6, 4);
                    pick_delay(x);
                    x.anim_start(ANIMS, 3);
                    rest(x);
                }
                4 => rest(x),
                8 => finish_attack(x),
                _ => {}
            }
        } else {
            match x.u8(0x6) {
                0 => {
                    x.add8(0x6, 4);
                    x.anim_start(ANIMS, 5);
                    attack(x);
                }
                4 => attack(x),
                8 => back_to_rest(x),
                _ => {}
            }
        }
        x.reinsert();
    }

    /// Delay before the next draw of lots, according to the number of players.
    fn pick_delay<R: Bench>(x: &mut Cx<'_, R>) {
        let players = x.a5_u8(PLAYER_COUNT) as u32;
        let table = x.data32(DELAY_TABLES + 4 * players);
        let r = random(x) & 0x1f;
        let d = x.data16(table + 2 * r);
        x.set16(DELAY, d);
    }

    /// Draw: is the bit drawn in the mask of the current configuration set?
    fn gate<R: Bench>(x: &mut Cx<'_, R>) -> bool {
        let players = x.a5_u8(PLAYER_COUNT) as u32;
        let table = x.data32(GATE_TABLES + 4 * players);
        let index = x.a5_u16(GATE_INDEX).wrapping_shl(2) as u32;
        let mask = x.data32(table + index);
        let r = random(x) & 0x1f;
        mask >> r & 1 != 0
    }

    fn lost_boss<R: Bench>(x: &mut Cx<'_, R>) {
        x.set16(0x4, 0x400);
    }

    /// Rest: countdown, draw, then attack if the target is on the right side.
    fn rest<R: Bench>(x: &mut Cx<'_, R>) {
        let boss = x.linked(LINK);
        if x.a5_u8(GAME_PAUSED) == 0 {
            let t = x.u16(DELAY).wrapping_sub(1);
            x.set16(DELAY, t);
            if s16(t) < 0 {
                pick_delay(x);
                if gate(x) && boss.u16(x.b, f::OWNER) != 0 {
                    let target = obj_of(boss.u16(x.b, f::OWNER));
                    let d = s16(boss.u16(x.b, 0x8).wrapping_sub(target.u16(x.b, 0x8)));
                    let facing = x.u8(f::FLIP) != 0;
                    if (d >= 0) != facing {
                        x.set16(0x6, 0x800);
                        x.anim_start(ANIMS, 5);
                        return;
                    }
                }
            }
        }
        if boss.u8(x.b, 0) == 0 {
            return lost_boss(x);
        }
        match s8(boss.u8(x.b, BOSS_PHASE)) {
            1 => {
                // the boss attacks: goes to the attack phase
                x.add8(0x5, 4);
                x.set16(0x6, 0);
            }
            2.. => lost_boss(x),
            _ => {
                follow(x);
                x.anim_step();
            }
        }
    }

    /// End of the rest phase by `+6 = 8`: waits for the end of the animation then creates a shot 53.
    fn finish_attack<R: Bench>(x: &mut Cx<'_, R>) {
        let boss = x.linked(LINK);
        if boss.u16(x.b, f::OWNER) == 0 {
            return x.set16(0x6, 0);
        }
        if boss.u8(x.b, 0) == 0 {
            return lost_boss(x);
        }
        match s8(boss.u8(x.b, BOSS_PHASE)) {
            1 => {
                x.add8(0x5, 4);
                x.set16(0x6, 0);
                return;
            }
            2.. => return lost_boss(x),
            _ => {}
        }
        if x.anim_step() {
            if let Some(shot) = x.alloc(&pools::ESHL) {
                shot.set_u8(x.b, 0, 1);
                shot.set_u8(x.b, 2, 0x35);
                shot.set_u16(x.b, LINK, x.o.0 as u16);
            }
            x.set16(0x6, 0);
        }
    }

    /// Attack in progress (the boss is in phase 1): follows the boss, plays the animation, then ends.
    fn attack<R: Bench>(x: &mut Cx<'_, R>) {
        follow(x);
        if !x.anim_step() {
            return;
        }
        x.set32(0x34, 0);
        x.add8(0x6, 4);
        back_to_rest(x);
    }

    /// Has the boss finished its attack? Rest if the phase has dropped below 1.
    fn back_to_rest<R: Bench>(x: &mut Cx<'_, R>) {
        let boss = x.linked(LINK);
        if boss.u8(x.b, 0) == 0 {
            return x.set8(0x4, 6);
        }
        match s8(boss.u8(x.b, BOSS_PHASE)) {
            1 => {}
            2.. => x.set8(0x4, 6),
            _ => {
                x.set8(0x5, 0);
                x.set16(0x6, 0);
            }
        }
    }

    /// State 4: animation 5 until the end, then disappearance.
    fn striking<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x5) {
            0 => {
                x.add8(0x5, 4);
                x.anim_start(ANIMS, 5);
                strike_step(x);
            }
            4 => strike_step(x),
            _ => {}
        }
        x.reinsert();
    }

    fn strike_step<R: Bench>(x: &mut Cx<'_, R>) {
        follow(x);
        if x.anim_step() {
            x.set16(0x4, 0x600);
        }
    }

    /// Moves the hand closer to its target position (in front of the boss, on the side it faces), by 2 pixels per frame
    /// in X and Y. Boss gone: state 4 (and return to the caller which continues).
    fn follow<R: Bench>(x: &mut Cx<'_, R>) {
        let boss = x.linked(LINK);
        if boss.u8(x.b, 0) == 0 {
            return lost_boss(x);
        }
        let reach = if x.u8(f::FLIP) != 0 { REACH } else { neg(REACH) };
        let goal_x = reach.wrapping_add(boss.u16(x.b, 0x8));
        let goal_y = boss.u16(x.b, 0x10);
        let px = approach(goal_x, x.u16(0x8));
        x.set16(0x8, px);
        let py = approach(goal_y, x.u16(0x10));
        x.set16(0x10, py);
    }

    /// A step of 2 toward `goal` without overshooting it.
    fn approach(goal: u16, pos: u16) -> u16 {
        if s16(goal) == s16(pos) {
            pos
        } else if s16(goal) < s16(pos) {
            let p = pos.wrapping_sub(SPEED);
            if s16(goal) <= s16(p) { p } else { goal }
        } else {
            let p = pos.wrapping_add(SPEED);
            if s16(goal) >= s16(p) { p } else { goal }
        }
    }
}

handlers! {
    0xa31e4 "eshl_40" shot40::run,
    0xa336c "eshl_41" shot41::run,
    0xa3568 "eshl_42" shot42::run,
    0xa3762 "eshl_43" shot43::run,
    0xa380a "eshl_44" beam44::run,
    0xa3b88 "eshl_45" debris45::run,
    0xa3e72 "eshl_46" scroller46::run,
    0xa3ebe "eshl_47" spawn47::run,
    0xa4088 "eshl_48" lob48::run,
    0xa418e "eshl_49" strike49::run,
    0xa4266 "eshl_50" bullet50::run,
    0xa4522 "eshl_51" drop51::run,
    0xa46c6 "eshl_52" finale52::run,
    0xa4d82 "eshl_53" aimed53::run,
    0xa4eaa "eshl_54" burst54::run,
    0xa5358 "eshl_55" fall55::run,
    0xa5466 "eshl_56" pickup56::run,
    0xa593c "eshl_57" seeker57::run,
    0xa5ad6 "eshl_58" shard58::run,
    0xa5cc2 "eshl_59" ring59::run,
    0xa5f5c "eshl_60" platform60::run,
    0xa63d8 "eshl_61" marker61::run,
    0xa66ea "eshl_62" meteor62::run,
    0xa6864 "eshl_63" hand63::run,
}
