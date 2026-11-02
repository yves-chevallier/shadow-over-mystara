//! Common reactions of enemies and bosses to hits (O3, `0x3bb00-0x3e480`).
//!
//! When a hit lands, the collision stores a reaction code in `+0x49`; each type's "hit" state
//! calls the routine designated by this code in its reaction table (longs, for example `0x56816` for the
//! Kobold, see `enemy_common::react`). These routines are common to all types: recoil, throw into the air,
//! fall, bounce on the ground, get-up, flashing death, grab, slide... Each is a state machine on `+6`
//! (step of 2, PC-relative jump table) whose steps are often shared (`fall`, `bounce`, `get-up`,
//! `dead`). The animations and sounds depend on the type (`+2`) and the class (`+0x28`: 2 ENEMY, 4 BOSS),
//! through ROM tables.
//!
//! Fields (besides those of `enemy_common::f`): speeds `+0x14` (X), `+0x16` (depth), `+0x18`
//! (height), `+0x1a` (gravity), `+0x1c`/`+0x1e` (secondary); height `+0xc` (long); counter `+0x90`;
//! liaison en attente `+0x91` (bit 7) vers `+0x2a`/`+0x2b` ; orientation de l'agresseur `+0x4a`.

use crate::families::cx::Bench;
use super::cx::Cx;
use super::enemy_lib as lib;
use super::enemy_common::var;
use super::enemy_common::detach_from_host;
use crate::mem::{rd16, rd32, rd8};
use crate::Native;
use crate::objects::pools;
use crate::objects::Current;
use crate::machine::Machine;

/// Fields of the hit objects.
mod f {
    pub const ACTIVE: u32 = 0;
    pub const DISPLAY: u32 = 1;
    pub const KIND: u32 = 2;
    pub const STATE: u32 = 4;
    pub const SUB: u32 = 5;
    pub const STEP: u32 = 6;
    pub const SUB_STEP: u32 = 7;
    pub const X: u32 = 8;
    pub const X_SAVED: u32 = 0xa;
    pub const HEIGHT: u32 = 0xc;
    pub const Y: u32 = 0x10;
    pub const VX: u32 = 0x14;
    pub const VY: u32 = 0x16;
    pub const VZ: u32 = 0x18;
    pub const GRAVITY: u32 = 0x1a;
    pub const VX2: u32 = 0x1c;
    pub const VY2: u32 = 0x1e;
    pub const CLASS: u32 = 0x28;
    /// Contact with an obstacle during movement.
    pub const BLOCKED: u32 = 0x29;
    pub const LINKED: u32 = 0x2a;
    pub const LINK: u32 = 0x2b;
    pub const FLIP: u32 = 0x2c;
    pub const ANGLE: u32 = 0x2d;
    /// End of the animation (word; byte `+0x33`: animation event).
    pub const ANIM_END: u32 = 0x32;
    pub const ANIM_EVENT: u32 = 0x33;
    pub const REACTION: u32 = 0x49;
    pub const ATTACKER_FLIP: u32 = 0x4a;
    pub const OWNER: u32 = 0x50;
    pub const FREEZE: u32 = 0x53;
    pub const STUN: u32 = 0x56;
    pub const HP: u32 = 0x62;
    pub const RECOVER: u32 = 0x84;
    pub const DOWNED: u32 = 0x86;
    pub const GRABBED: u32 = 0x87;
    pub const RESUME_STATE: u32 = 0x8a;
    pub const COUNTER: u32 = 0x90;
    pub const PENDING: u32 = 0x91;
}

/// Classes (octet `+0x28`).
const ENEMY: u8 = 2;
const BOSS: u8 = 4;

/// ROM tables.
pub(crate) mod rom {
    /// Animation sets per type (longs read as data; high word zero: own routine, offset).
    pub const ANIM_SETS: u32 = 0x3de58;
    /// Animation variant by `+0xbf` (bytes, PC-relative).
    pub const ANIM_VARIANTS: u32 = 0x3e28a;
    /// Hit and death sounds per type (PC-relative longs), get-up routine per type.
    pub const HIT_SOUNDS: u32 = 0x3e2ac;
    pub const DEATH_SOUNDS: u32 = 0x3e3a4;
    pub const GET_UP: u32 = 0x3e000;
    /// Types that do not stay on the ground when grabbed (PC-relative bytes).
    pub const NO_GRAB_DOWN: u32 = 0x3e1ee;
    /// Predefined throws: counter (bytes), speeds (4 words per entry).
    pub const LAUNCH_COUNTERS: u32 = 0x3bdec;
    pub const LAUNCH_SPEEDS: u32 = 0x3bdcc;
    /// Empty routine (`rts`) of the per-type tables.
    pub const NOTHING: [u32; 2] = [0x3e2aa, 0x3bb9a];
}

// ---------------------------------------------------------------------------------------------
// Aides

fn next<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STEP, 2);
}
fn next_sub<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB_STEP, 2);
}
/// Next step, sub-step at zero.
fn next_clear<R: Bench>(x: &mut Cx<'_, R>) {
    next(x);
    x.set8(f::SUB_STEP, 0);
}
fn neg16<R: Bench>(x: &mut Cx<'_, R>, off: u32) {
    let v = x.u16(off).wrapping_neg();
    x.set16(off, v);
}
fn neg32<R: Bench>(x: &mut Cx<'_, R>, off: u32) {
    let v = x.u32(off).wrapping_neg();
    x.set32(off, v);
}
fn flipped<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    x.u8(f::FLIP) != 0
}
fn dead<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    (x.u16(f::HP) as i16) <= 0
}
fn paused<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    var::PAUSE.get(x.b) != 0
}
/// Death: substate 6 (+ `+0` = 2).
fn mark_dead<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(f::SUB, 6);
    x.set8(f::ACTIVE, 2);
}
fn speeds<R: Bench>(x: &mut Cx<'_, R>, vx: u16, vy: u16, vz: u16, gravity: u16) {
    x.set16(f::VX, vx);
    x.set16(f::VY, vy);
    x.set16(f::VZ, vz);
    x.set16(f::GRAVITY, gravity);
}
fn anim_step<R: Bench>(x: &mut Cx<'_, R>) {
    x.anim_step();
}
fn move_x<R: Bench>(x: &mut Cx<'_, R>) {
    x.move_x();
}
fn move_xz<R: Bench>(x: &mut Cx<'_, R>) {
    x.move_xz();
}
fn move_xyz<R: Bench>(x: &mut Cx<'_, R>) {
    x.move_xyz();
}
fn landed<R: Bench>(x: &mut Cx<'_, R>) {
    lib::landing_by_kind(x);
}
fn random<R: Bench>(x: &mut Cx<'_, R>) -> u32 {
    x.random() as u32
}
/// Effect `0xabec8` with given variant and setting; false if the pool refused it.
fn effect<R: Bench>(x: &mut Cx<'_, R>, variant: u32, setting: u32) -> bool {
    lib::attack_effect(x, variant, setting)
}
/// Index into the per-type tables: type, plus 32 outside the ENEMY class.
fn type_index<R: Bench>(x: &mut Cx<'_, R>) -> u32 {
    let base = if x.u8(f::CLASS) == ENEMY { 0 } else { 32 };
    (base + x.u8(f::KIND) as u32) & 0xff
}

/// 0x3de26: the freeze after the hit takes the stun duration.
fn stun<R: Bench>(x: &mut Cx<'_, R>) {
    let s = x.u8(f::STUN);
    x.set8(f::FREEZE, s);
}

/// 0x3de2e: animation `n` of the type's set (types 13, 19, 34 and 54 add a variant `+0xbf`).
fn anim<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    let i = 4 * type_index(x);
    let entry = rd32(x.b, rom::ANIM_SETS + i);
    let (set, n) = if entry >> 16 != 0 {
        (entry, n as u16)
    } else {
        // own routine: `bsr 0x3e27e ; movea.l #game,A4 ; jmp 0x1a32` (0x3e27e adds the variant to the low byte)
        let routine = rom::ANIM_SETS + (entry & 0xffff);
        let v = x.u8(0xbf) as u32;
        let add = x.rom8(rom::ANIM_VARIANTS + v);
        let n = n as u16 & !0xff | (n as u8).wrapping_add(add) as u16;
        ((x.rom16(routine + 6) as u32) << 16 | x.rom16(routine + 8) as u32, n)
    };
    x.anim_start(set, n);
}

/// Routine from a per-type table (PC-relative longs, `jsr (A1)` with D0 = the index); `None` for empty entries
/// (`rts`).
fn per_type<R: Bench>(x: &mut Cx<'_, R>, table: u32, index: u32) -> Option<u32> {
    let i = 4 * index;
    let routine = (x.rom16(table + i) as u32) << 16 | x.rom16(table + i + 2) as u32;
    (!rom::NOTHING.contains(&routine)).then_some(routine)
}
/// 0x3e28e and 0x3e388: hit sound, death sound (per-type sound tables: the routine read is the sound).
fn hit_sound<R: Bench>(x: &mut Cx<'_, R>) {
    let i = type_index(x);
    if let Some(sound) = per_type(x, rom::HIT_SOUNDS, i) {
        lib::sound(x, sound);
    }
}
fn death_sound<R: Bench>(x: &mut Cx<'_, R>) {
    let i = type_index(x);
    if let Some(sound) = per_type(x, rom::DEATH_SOUNDS, i) {
        lib::sound(x, sound);
    }
}
/// 0x3e000: type-specific get-up (per-type table; the address read designates its family's handler).
fn get_up_by_kind<R: Bench>(x: &mut Cx<'_, R>, index: u32) {
    let Some(routine) = per_type(x, rom::GET_UP, index) else { return };
    match routine {
        0x653b2 => super::enemy_gargoyle::attacks::regroup(x),
        0x717ea => super::enemy_ogre::hit::regroup(x),
        0x78c3c => super::enemy_warmachine::hit::recover(x),
        0x7a5ac => super::boss_0::hit::get_up(x),
        0x7bece => super::boss_1::hit::get_up(x),
        0x82ea0 => super::boss_4::hit::resume(x),
        0x85490 => super::boss_6::hit::recovered_tail(x),
        0x96ce0 => super::boss_17::hit::recovered_tail(x),
        0x9a6d4 => super::boss_21::hit::recovered(x),
        _ => unreachable!("get-up of type {index}: routine {routine:#x} outside table 0x3e000"),
    }
}
/// 0x3e180: hit sound, or death (substate 6) and death sound.
fn hurt<R: Bench>(x: &mut Cx<'_, R>) {
    if dead(x) {
        mark_dead(x);
        death_sound(x);
    } else {
        hit_sound(x);
    }
}

/// 0x3e130: the object turns toward the aggressor; the X and depth speeds follow the orientation.
fn face_attacker<R: Bench>(x: &mut Cx<'_, R>) {
    let flip = x.u8(f::ATTACKER_FLIP) ^ 0x20;
    // leftover: the orientation in the low byte of D2 (`dispatch::Leftovers::d2`, speed of the spark of
    // `flash_and_shake`)
    *x.c.d2() = *x.c.d2() & !0xff | flip as u32;
    x.set8(f::FLIP, flip);
    if flip != 0 {
        neg16(x, f::VX);
        neg16(x, f::VY);
    }
}
/// 0x3e148: the speeds follow the current orientation.
fn orient_speeds<R: Bench>(x: &mut Cx<'_, R>) {
    if flipped(x) {
        neg16(x, f::VX);
        neg16(x, f::VY);
    }
}

/// 0x3e25e: a pending link (`+0x91`, bit 7) becomes effective (`+0x2b`, `+0x2a` raised).
fn apply_pending<R: Bench>(x: &mut Cx<'_, R>) {
    let p = x.u8(f::PENDING);
    if (p as i8) >= 0 {
        return;
    }
    x.set8(f::LINK, p & 0x7f);
    x.set8(f::LINKED, 1);
    x.set8(f::PENDING, 0);
}
/// Variant that always clears `+0x91`.
fn take_pending<R: Bench>(x: &mut Cx<'_, R>) {
    let p = x.u8(f::PENDING);
    if (p as i8) < 0 {
        x.set8(f::LINK, p & 0x7f);
        x.set8(f::LINKED, 1);
    }
    x.set8(f::PENDING, 0);
}
/// Inverse: the effective link goes back to pending (`+0x2b` | 0x80 in `+0x91`).
fn defer_link<R: Bench>(x: &mut Cx<'_, R>) {
    if (x.u8(f::PENDING) as i8) < 0 || x.u8(f::LINKED) == 0 {
        return;
    }
    let l = x.u8(f::LINK) | 0x80;
    x.set8(f::PENDING, l);
    x.set8(f::LINKED, 0);
}

/// 0x3bda2: predefined throw `n` (counter and four speeds from the ROM).
fn launch<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.set8(f::DOWNED, 0);
    next(x);
    let c = x.rom8(rom::LAUNCH_COUNTERS + n);
    x.set8(f::COUNTER, c);
    let at = rom::LAUNCH_SPEEDS + 8 * n;
    for (k, off) in [f::VX, f::VY, f::VZ, f::GRAVITY].into_iter().enumerate() {
        let v = x.rom16(at + 2 * k as u32);
        x.set16(off, v);
    }
}

/// Shaking in place: X oscillates by ±2 according to the counter parity and the orientation.
fn shake<R: Bench>(x: &mut Cx<'_, R>, amount: u16) {
    let mut d = amount;
    if x.u8(f::COUNTER) & 1 != 0 {
        d = d.wrapping_neg();
    }
    if flipped(x) {
        d = d.wrapping_neg();
    }
    x.add16(f::X, d);
}

// ---------------------------------------------------------------------------------------------
// Pas communs

/// 0x3bdf0: waits for the end of the animation.
fn wait_anim<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(f::ANIM_END) != 0 {
        next(x);
    } else {
        anim_step(x);
    }
}

/// 0x3be02: fall (X movement and height); on the ground, next step and landing.
fn fall<R: Bench>(x: &mut Cx<'_, R>) {
    move_xz(x);
    if x.u16(f::HEIGHT) as i16 > 0 {
        if x.u8(f::BLOCKED) != 0 {
            x.set32(f::VX, 0);
            x.set8(f::COUNTER, 0);
        }
        return anim_step(x);
    }
    next_clear(x);
    x.set32(f::HEIGHT, 0);
    x.set32(f::VZ, 0);
    landed(x);
}

/// 0x3be38: fall, byte `+0x38` at zero.
fn fall_38<R: Bench>(x: &mut Cx<'_, R>) {
    fall(x);
    x.set8(0x38, 0);
}

/// 0x3e22e: next step; the non-grabbed type 3 boss chains its own reaction (0x81ae0).
fn boss_3_hook<R: Bench>(x: &mut Cx<'_, R>) {
    next(x);
    if x.u8(f::KIND) == 3 && x.u8(f::CLASS) == BOSS && x.u8(f::SUB) != 6 && x.u8(f::GRABBED) == 0 {
        super::boss_3::counter_reaction(x);
    }
}

/// 0x3be52: bounce on the ground (animation 5).
fn bounce_start<R: Bench>(x: &mut Cx<'_, R>) {
    x.set16(f::VY, 0);
    x.set16(f::VZ, 0x300);
    x.set16(f::GRAVITY, -72i16 as u16);
    next(x);
    x.set8(f::SUB_STEP, 0);
    anim(x, 5);
}
/// 0x3be40 : rebond en reculant.
fn bounce_back<R: Bench>(x: &mut Cx<'_, R>) {
    x.set16(f::VX, 0x200);
    if flipped(x) {
        neg16(x, f::VX);
    }
    bounce_start(x);
}
/// 0x3be70: bounce (animation 2) after a link.
fn bounce_linked<R: Bench>(x: &mut Cx<'_, R>) {
    take_pending(x);
    x.set16(f::VZ, 0x300);
    x.set16(f::GRAVITY, -72i16 as u16);
    next(x);
    x.set8(f::SUB_STEP, 0);
    anim(x, 2);
}
/// 0x3cf74 : rebond (animation 2).
fn bounce_2<R: Bench>(x: &mut Cx<'_, R>) {
    x.set16(f::VZ, 0x300);
    x.set16(f::GRAVITY, -72i16 as u16);
    next(x);
    x.set8(f::SUB_STEP, 0);
    anim(x, 2);
}

/// 0x3bea8: the bounce, four sub-steps (`+7`).
fn bounce<R: Bench>(x: &mut Cx<'_, R>) {
    let s = x.u8(f::SUB_STEP);
    match s {
        0 => {
            anim_step(x);
            if x.u8(f::ANIM_EVENT) != 0 {
                next_sub(x);
            }
        }
        2 => {
            move_xz(x);
            if x.u8(f::BLOCKED) != 0 {
                x.set32(f::VX, 0);
            }
            if x.u16(f::HEIGHT) as i16 > 0 {
                return;
            }
            next_sub(x);
            landed(x);
        }
        4 => {
            if (x.u8(f::ANIM_END) as i8) >= 0 {
                return anim_step(x);
            }
            if x.u8(f::COUNTER) == 0 {
                return next_clear(x);
            }
            x.set16(f::VX, 0x280);
            x.set16(f::VY, -64i16 as u16);
            if flipped(x) {
                neg16(x, f::VX);
                neg16(x, f::VY);
            }
            next_sub(x);
        }
        6 => {
            move_x(x);
            if x.u8(f::VX) == 0 {
                next_clear(x);
            }
        }
        _ => {}
    }
}

/// 0x3e0dc: get-up; death (substate 6): next step.
fn get_up<R: Bench>(x: &mut Cx<'_, R>) {
    x.set32(0x16, 0);
    x.set8(0x67, 0);
    if x.u8(f::SUB) == 6 {
        return next_clear(x);
    }
    x.set8(f::DOWNED, 1);
    if !kept_down(x) {
        get_up_done(x);
    }
}
/// 0x3df7c: end of the slide: the object gets up.
fn slide_end<R: Bench>(x: &mut Cx<'_, R>) {
    x.set32(0x16, 0);
    if var::GAME_ON.get(x.b) != 0 {
        x.set8(0x67, 1);
    }
    x.set8(f::ACTIVE, 1);
    x.set8(f::REACTION, 0);
    if !kept_down(x) {
        resume_after_slide(x);
    }
}

/// 0x3e19a: a grabbed object (`+0x87`) on the ground may stay down (reaction code 104, or 108 if it was
/// down): true in that case; otherwise the caller continues (A6 designates its continuation in the original: `jmp
/// (A6)`).
fn kept_down<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if x.u8(f::GRABBED) != 0 {
        // `bhi` on the last comparison of 0x1e92 (zero margin): an object exactly on the edge in width counts as
        // dedans
        if !crate::objects::cull::beyond_arena(x.b, x.o, 0) {
            x.set16(f::RECOVER, 1);
            let i = type_index(x);
            let keep = x.rom8(rom::NO_GRAB_DOWN + i);
            if keep == 0 {
                let code = if x.u8(f::DOWNED) != 0 { 108 } else { 104 };
                x.set8(f::REACTION, code);
                x.set16(f::STEP, 0);
                return true;
            }
        }
    }
    x.set8(f::GRABBED, 0);
    false
}

/// 0x3e0fc: standing, the main state resumes (0x04040000); the boss's get-up (0xf118) except code 108.
fn get_up_done<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(f::ACTIVE, 1);
    x.set32(f::STATE, 0x0404_0000);
    x.set32(0x58, 0);
    x.set8(0x67, 0);
    x.set16(f::RECOVER, 1);
    if x.u8(f::REACTION) != 108 {
        lib::draw_84(x);
    }
}

/// 0x3df9e: after the slide, the saved state (`+0x8a`) resumes if it can, otherwise the active state and the
/// type's get-up routine.
fn resume_after_slide<R: Bench>(x: &mut Cx<'_, R>) {
    let saved = x.u32(f::RESUME_STATE);
    if saved != 0 {
        x.set32(0x58, 0);
        x.set8(f::ACTIVE, 1);
        if x.u16(f::RESUME_STATE) == 0x205 && lib::roll_78(x) {
                x.set32(f::STATE, saved);
                x.set8(f::SUB_STEP, 0);
                x.set32(f::RESUME_STATE, 0);
                x.set8(0x78, 0xff);
                return;
            }
    }
    x.set32(f::STATE, 0x0202_0000);
    x.set8(0x78, 0xff);
    let base = if x.u8(f::CLASS) == BOSS { 32 } else { 0 };
    let i = (base + x.u8(f::KIND) as u32) & 0xff;
    get_up_by_kind(x, i);
}

/// 0x3dd9c: death; 40 frames of blinking, then the object disappears (0x19ad6).
fn death<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB_STEP) == 0 {
        x.set8(f::COUNTER, 40);
        next_sub(x);
        detach_from_host(x);
        lib::drop_rewards(x);
    }
    let c = x.u8(f::COUNTER).wrapping_sub(1);
    x.set8(f::COUNTER, c);
    if c == 0 {
        x.set32(f::STATE, 0x0600_0000);
        return lib::object_event(x);
    }
    blink(x, c);
}
/// Blinking: bit 1 of `+1` (hidden) one frame out of four... according to the two low bits of the counter.
fn blink<R: Bench>(x: &mut Cx<'_, R>, c: u8) {
    x.set_bit(f::DISPLAY, 1, false);
    if c & 3 != 0 {
        x.set_bit(f::DISPLAY, 1, true);
    }
}

/// 0x3e158: next step, then 0x51c06.
fn step_51c06<R: Bench>(x: &mut Cx<'_, R>) {
    next(x);
    crate::families::creators::weapon_shot(&mut crate::families::effects_creators::leftovers::Pure, x.b, x.o);
}
/// 0x3e162: next step.
fn skip<R: Bench>(x: &mut Cx<'_, R>) {
    next(x);
}
/// `rts`.
fn idle<R: Bench>(_: &mut Cx<'_, R>) {}

// ---------------------------------------------------------------------------------------------
// Throws (first step of the reactions)

/// 0x3bb9c (code 0): recoil in place; death if the HP are exhausted.
fn recoil<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::STEP) != 0 {
        return recoil_shake(x);
    }
    x.set8(0x67, 0);
    apply_pending(x);
    if dead(x) {
        x.set8(f::SUB, 6);
        x.set8(f::ACTIVE, 2);
        return run_entry(x, 0x3dd72);
    }
    recoil_start(x, 0);
}
/// Start of the recoil (16 frames of shaking, small push), animation `n`.
fn recoil_start<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    next(x);
    stun(x);
    x.set8(f::COUNTER, 16);
    x.set16(f::VX, 0x180);
    x.set16(f::VY, -56i16 as u16);
    face_attacker(x);
    anim(x, n);
    hit_sound(x);
}
/// 0x3bbec: shaking, then slide (0x3bc10).
fn recoil_shake<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::COUNTER) != 0 {
        let c = x.u8(f::COUNTER).wrapping_sub(1);
        x.set8(f::COUNTER, c);
        return shake(x, 2);
    }
    slide(x);
}
/// 0x3bc10 / 0x3d7f8: slide until it stops, then get-up.
fn slide<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::VX) == 0 {
        return slide_end(x);
    }
    move_x(x);
    anim_step(x);
}

/// 0x3bf76 (code 0x38 and neighbors): like the recoil, animation 1, death through 0x3ddf6.
fn recoil_1<R: Bench>(x: &mut Cx<'_, R>) {
    if dead(x) {
        mark_dead(x);
        return run_entry(x, 0x3ddf6);
    }
    if x.u8(f::STEP) != 0 {
        return recoil_shake(x);
    }
    apply_pending(x);
    next(x);
    x.set8(f::COUNTER, 16);
    stun(x);
    x.set16(f::VX, 0x180);
    x.set16(f::VY, -56i16 as u16);
    face_attacker(x);
    anim(x, 1);
    hit_sound(x);
}

/// Throw into the air: X speed `vx` (long), height 0x400, gravity -64, counter `counter`, animation
/// `anim_n`; `steps` steps of advance (two for some variants); `clear_67`: byte `+0x67` is cleared.
fn throw<R: Bench>(x: &mut Cx<'_, R>, vx: u32, counter: u8, anim_n: u32, steps: u8, clear_67: bool) {
    if clear_67 {
        x.set8(0x67, 0);
    }
    apply_pending(x);
    x.set8(f::DOWNED, 0);
    anim(x, anim_n);
    x.add8(f::STEP, 2 * steps);
    x.set8(f::COUNTER, counter);
    x.set32(f::VX, vx);
    x.set16(f::VZ, 0x400);
    x.set16(f::GRAVITY, -64i16 as u16);
    x.set32(f::VX2, 0);
    face_attacker(x);
    stun(x);
    hurt(x);
}

/// 0x3bc42, 0x3bca0, 0x3bd00, 0x3bd60 : projections (animation 2), de plus en plus fortes.
fn throw_1<R: Bench>(x: &mut Cx<'_, R>) {
    throw(x, 0x0180_0000, 0, 2, 1, true);
}
fn throw_2<R: Bench>(x: &mut Cx<'_, R>) {
    throw(x, 0x0200_0000, 1, 2, 1, true);
}
fn throw_3<R: Bench>(x: &mut Cx<'_, R>) {
    throw(x, 0x0280_0000, 1, 2, 1, true);
}
fn throw_4<R: Bench>(x: &mut Cx<'_, R>) {
    throw(x, 0x0300_0000, 1, 2, 1, true);
}
/// 0x3bfde, 0x3c03a, 0x3c098, 0x3c0f2: same throws, animation 3 (the first two skip a step).
fn throw_1b<R: Bench>(x: &mut Cx<'_, R>) {
    throw(x, 0x0180_0000, 0, 3, 2, false);
}
fn throw_2b<R: Bench>(x: &mut Cx<'_, R>) {
    throw(x, 0x0200_0000, 1, 3, 2, false);
}
fn throw_3b<R: Bench>(x: &mut Cx<'_, R>) {
    throw(x, 0x0280_0000, 1, 3, 1, false);
}
fn throw_4b<R: Bench>(x: &mut Cx<'_, R>) {
    throw(x, 0x0300_0000, 1, 2, 1, false);
}

// ---------------------------------------------------------------------------------------------
// High throws (0x3c14e and neighbors) and slides on the ground

/// 0x3c15e: hit that makes it take off (X speeds 0x600, height 0x280) after animation `n`.
fn uppercut<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    anim(x, n);
    x.set8(0x67, 0);
    apply_pending(x);
    x.set8(f::DOWNED, 0);
    hurt(x);
    next(x);
    stun(x);
    speeds(x, 0x600, 0, 0x280, -64i16 as u16);
    face_attacker(x);
}
fn uppercut_2<R: Bench>(x: &mut Cx<'_, R>) {
    uppercut(x, 2);
}
fn uppercut_3<R: Bench>(x: &mut Cx<'_, R>) {
    uppercut(x, 3);
}

/// 0x3c192: high bounce (animation 5).
fn bounce_high<R: Bench>(x: &mut Cx<'_, R>) {
    next(x);
    x.set16(f::VX, 0x300);
    x.set16(f::VZ, 0x300);
    x.set16(f::GRAVITY, -72i16 as u16);
    orient_speeds(x);
    x.set8(f::SUB_STEP, 0);
    anim(x, 5);
}

/// Start of a slide on the ground (animation 6) at speed `vx`.
fn slide_start<R: Bench>(x: &mut Cx<'_, R>, vx: u16) {
    next(x);
    x.set8(f::SUB_STEP, 0);
    x.set16(f::VX, vx);
    x.set16(f::VY, -64i16 as u16);
    orient_speeds(x);
    anim(x, 6);
}
fn slide_start_fast<R: Bench>(x: &mut Cx<'_, R>) {
    slide_start(x, 0x600);
}
fn slide_start_slow<R: Bench>(x: &mut Cx<'_, R>) {
    slide_start(x, 0x400);
}

/// Slide dust, every other frame (0xb10c8).
fn dust<R: Bench>(x: &mut Cx<'_, R>) {
    let was = x.bit(f::COUNTER, 0);
    x.set_bit(f::COUNTER, 0, !was);
    if !was {
        return;
    }
    lib::shock(x, 0, 1, 0, 0xffff);
}

/// 0x3c270: slide; stopped, the type 0 ENEMY waits for its animation's event for animation 7.
fn slide_fast<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB_STEP) != 0 {
        anim_step(x);
        if x.u8(f::ANIM_EVENT) == 0 {
            return dust(x);
        }
        return next(x);
    }
    anim_step(x);
    move_x(x);
    if x.u8(f::BLOCKED) != 0 {
        x.set32(f::VX, 0);
    }
    if x.u8(f::VX) == 0 {
        x.set32(f::VX, 0);
        if x.u8(f::KIND) == 0 && x.u8(f::CLASS) == ENEMY {
            if x.u8(f::ANIM_EVENT) != 0 {
                next_sub(x);
                return anim(x, 7);
            }
        } else {
            next(x);
        }
    }
    dust(x)
}

/// 0x3c500: slow slide; stopped, animation 7 when the current animation signals it.
fn slide_slow<R: Bench>(x: &mut Cx<'_, R>) {
    anim_step(x);
    if x.u8(f::SUB_STEP) != 0 {
        if x.u8(f::ANIM_EVENT) == 0 {
            return dust(x);
        }
        return next_clear(x);
    }
    move_x(x);
    if x.u8(f::BLOCKED) != 0 {
        x.set16(f::VX, 0);
        x.set16(f::VY, 0);
    }
    if x.u8(f::VX) == 0 {
        x.set16(f::VX, 0);
        x.set16(f::VY, 0);
        if x.u8(f::ANIM_EVENT) != 0 {
            next_sub(x);
            return anim(x, 7);
        }
    }
    dust(x)
}

/// 0x3c30a : coup bas (animation 4).
fn low_hit<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(0x67, 0);
    apply_pending(x);
    hurt(x);
    next(x);
    stun(x);
    speeds(x, 0x200, 0, 0x180, -72i16 as u16);
    face_attacker(x);
    anim(x, 4);
}

/// 0x3c374 : projection rasante (18 images, animation 1).
fn skim<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(0x67, 0);
    apply_pending(x);
    hurt(x);
    next(x);
    x.set8(f::SUB_STEP, 0);
    stun(x);
    x.set8(f::COUNTER, 18);
    speeds(x, 0x800, 0, 0x100, -4i16 as u16);
    face_attacker(x);
    anim(x, 1);
}

/// 0x3c3ce: skimming flight (`+7`): 0 glide, 4 fall, 2 fall that restarts backward.
fn skim_flight<R: Bench>(x: &mut Cx<'_, R>) {
    let s = x.u8(f::SUB_STEP);
    match s {
        0 => {
            move_xz(x);
            let c = x.u8(f::COUNTER).wrapping_sub(1);
            x.set8(f::COUNTER, c);
            if c == 0 {
                x.add8(f::SUB_STEP, 4);
                x.set8(f::COUNTER, 1);
                x.set16(f::GRAVITY, -64i16 as u16);
                x.set16(f::VY, -64i16 as u16);
                if flipped(x) {
                    neg16(x, f::VY);
                }
                return;
            }
            if x.u32(f::HEIGHT) as i32 <= 0 {
                return next(x);
            }
            let camera = if var::CAMERA_SELECT.get(x.b) != 0 { var::CAMERA_ALT_X.get(x.b) } else { var::SCROLL_X.get(x.b) };
            let dx = x.u16(f::X).wrapping_sub(camera);
            if dx <= 0x180 && x.u8(f::BLOCKED) == 0 {
                return;
            }
            next_sub(x);
            x.set8(f::COUNTER, 0);
            x.set32(f::VX, 0);
            x.set16(f::VZ, 0);
            x.set16(f::GRAVITY, -64i16 as u16);
        }
        2 => {
            move_xz(x);
            if x.u16(f::HEIGHT) as i16 > 0 {
                return;
            }
            next(x);
            x.set32(f::HEIGHT, 0);
            x.set32(f::VX, 0);
            landed(x);
        }
        4 => {
            move_xz(x);
            if x.u16(f::HEIGHT) as i16 > 0 {
                return;
            }
            next(x);
            x.set32(f::HEIGHT, 0);
            x.set16(f::VY, 0);
            landed(x);
            x.set16(f::VX, 0x200);
            if flipped(x) {
                neg16(x, f::VX);
            }
        }
        _ => {}
    }
}

/// High throw (animation `n`, X speed `vx`, height `vz`, gravity `g`), counter 1.
fn pop<R: Bench>(x: &mut Cx<'_, R>, n: u32, vx: u16, vz: u16, g: u16, clear_67: bool) {
    if clear_67 {
        x.set8(0x67, 0);
    }
    apply_pending(x);
    next(x);
    x.set8(f::SUB_STEP, 0);
    stun(x);
    anim(x, n);
    hurt(x);
    x.set8(f::COUNTER, 1);
    speeds(x, vx, 0, vz, g);
    face_attacker(x);
}

/// 0x3c58c: throw whose height depends on the reaction code (table 0x3c5f2), animation 9.
fn pop_by_code<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(0x67, 0);
    apply_pending(x);
    next(x);
    x.set8(f::SUB_STEP, 0);
    stun(x);
    anim(x, 9);
    hurt(x);
    x.set8(f::COUNTER, 1);
    let i = (x.u8(f::REACTION) as u32).wrapping_sub(68) & 0xffff;
    let at = 0x3c5f2u32.wrapping_add(i as u16 as i16 as i32 as u32);
    let (vz, g) = (x.rom16(at), x.rom16(at + 2));
    speeds(x, 0x100, 0, vz, g);
    face_attacker(x);
}
fn pop_10<R: Bench>(x: &mut Cx<'_, R>) {
    pop(x, 10, 64, 0xc00, -128i16 as u16, true);
}
fn pop_11<R: Bench>(x: &mut Cx<'_, R>) {
    pop(x, 11, 0x600, 0xf800, -64i16 as u16, false);
}

/// 0x3c5fa: fall that animates while descending.
fn fall_animated<R: Bench>(x: &mut Cx<'_, R>) {
    move_xz(x);
    if x.u16(f::HEIGHT) as i16 > 0 {
        if x.u8(f::BLOCKED) != 0 {
            x.set16(f::VX, 0);
            x.set16(f::VY, 0);
            x.set8(f::COUNTER, 0);
        }
        if (x.u8(f::VZ) as i8) < 0 {
            anim_step(x);
        }
        return;
    }
    next(x);
    x.set32(f::HEIGHT, 0);
    x.set32(f::VZ, 0);
    landed(x);
}

fn held_start<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(0x67, 0);
    let px = x.u16(f::X);
    x.set16(f::X_SAVED, px);
    let n = x.u16(f::RECOVER).wrapping_sub(10);
    x.set16(f::RECOVER, n);
    x.set8(f::ACTIVE, 2);
    hurt(x);
    next(x);
    stun(x);
    x.set8(f::COUNTER, 16);
    anim(x, 8);
}
fn held_shake<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::COUNTER) == 0 {
        next(x);
        let px = x.u16(f::X_SAVED);
        return x.set16(f::X, px);
    }
    let c = x.u8(f::COUNTER).wrapping_sub(1);
    x.set8(f::COUNTER, c);
    shake(x, 2);
}
fn held_end<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB) == 6 {
        next_clear(x);
        return anim_step(x);
    }
    x.set32(0x58, 0);
    if x.u16(f::RECOVER) as i16 <= 0 {
        x.set8(f::ACTIVE, 2);
    }
    if x.u8(f::CLASS) == BOSS {
        lib::countdown_then_state(x);
        return anim_step(x);
    }
    x.set32(0x16, 0);
    x.set8(f::ACTIVE, 1);
    x.set32(f::STATE, 0x0404_0200);
    anim_step(x);
}

/// 0x3c7fa: two sparks (EFFCT type 8, variants 1 and 0) if the terrain allows.
fn sparks<R: Bench>(x: &mut Cx<'_, R>) {
    next(x);
    if x.a5_u16(0x7e68) < 2 {
        return;
    }
    for v in [1u8, 0] {
        // Original glitch: the allocation failure is not tested (see `Pool::alloc_unchecked`).
        let o = x.alloc_unchecked(&pools::EFFCT);
        o.set_u8(x.b, 0, 1);
        o.set_u8(x.b, 2, 8);
        o.set_u8(x.b, 3, v);
        o.set_u16(x.b, 0x5c, x.o.0 as u16);
    }
}

/// 0x3c828: disintegration (type 1 ENEMY: 0xeb448, otherwise 0xe35a6), then blinking and disappearance.
fn disintegrate<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if x.u8(f::STEP) == 0 {
        next(x);
        x.set8(f::SUB, 6);
        x.set8(f::ACTIVE, 2);
        x.set8(0x81, 0);
        x.set8(0x80, 0);
        anim(x, 12);
        let sound = if x.u8(f::KIND) == 1 { 0xeb448 } else { 0xe35a6 };
        lib::sound(x, sound);
        return true;
    }
    if (x.u16(f::ANIM_END) as i16) >= 0 {
        anim_step(x);
        return true;
    }
    if x.u8(f::SUB_STEP) == 0 {
        x.set8(f::COUNTER, 40);
        next_sub(x);
        detach_from_host(x);
    }
    let c = x.u8(f::COUNTER).wrapping_sub(1);
    x.set8(f::COUNTER, c);
    if c == 0 {
        x.set32(f::STATE, 0x0600_0000);
        return true;
    }
    blink(x, c);
    true
}

/// 0x3c8d8: stunned: randomly drawn delay (table 0x3c904), stars (0xabec8) at the end of each delay.
fn dazed<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(0x67, 0);
    apply_pending(x);
    next(x);
    anim(x, 3);
    face_attacker(x);
    dazed_delay(x);
}
fn dazed_delay<R: Bench>(x: &mut Cx<'_, R>) {
    let r = random(x) & 3;
    let d = x.rom8(0x3c904 + r);
    x.set8(f::COUNTER, d);
    hurt(x);
}
fn dazed_wait<R: Bench>(x: &mut Cx<'_, R>) {
    if !paused(x) {
        next(x);
    }
    let c = x.u8(f::COUNTER).wrapping_sub(1);
    x.set8(f::COUNTER, c);
    if c != 0 {
        return;
    }
    if effect(x, 0, 0) {
        return dazed_delay(x);
    }
    x.set8(f::COUNTER, 1);
}
/// 0x3c936: predefined throw 1, freeze of one frame.
fn knock<R: Bench>(x: &mut Cx<'_, R>) {
    hurt(x);
    launch(x, 1);
    x.set8(f::FREEZE, 1);
    if flipped(x) {
        neg32(x, f::VX);
    }
}

/// 0x3c970: big skimming throw (stars 0xabec8(2)).
fn blast<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(0x67, 0);
    next(x);
    apply_pending(x);
    x.set8(f::SUB_STEP, 0);
    x.set8(f::COUNTER, 0xff);
    hurt(x);
    stun(x);
    x.set16(f::VX, 0x800);
    x.set16(f::VY, 0);
    x.set32(f::VZ, 0x0100_0000);
    x.set16(f::GRAVITY, 0);
    anim(x, 1);
    if flipped(x) {
        neg16(x, f::VX);
    }
    x.set8(f::COUNTER, 18);
    effect(x, 2, 0);
}

/// 0x3c9ec: animation 19 (before a throw).
fn wind_up<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(0x67, 0);
    apply_pending(x);
    next(x);
    x.set8(f::SUB_STEP, 0);
    anim(x, 19);
}
/// 0x3ca04: the animation plays while the game is paused, then next step.
fn wind_up_wait<R: Bench>(x: &mut Cx<'_, R>) {
    anim_step(x);
    if !paused(x) {
        next(x);
    }
}
/// 0x3ca18 / 0x3ca3a: predefined throw 3 (animation 2), HP at zero for the first.
fn launch_3<R: Bench>(x: &mut Cx<'_, R>, kill: bool) {
    anim(x, 2);
    if kill {
        x.set16(f::HP, 0);
    }
    hurt(x);
    launch(x, 3);
    if flipped(x) {
        neg32(x, f::VX);
    }
}
fn launch_3_kill<R: Bench>(x: &mut Cx<'_, R>) {
    launch_3(x, true);
}
fn launch_3_hurt<R: Bench>(x: &mut Cx<'_, R>) {
    launch_3(x, false);
}

/// 0x3ca96 : saut vertical (animation 0).
fn hop<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(0x67, 0);
    apply_pending(x);
    next(x);
    if dead(x) {
        mark_dead(x);
    }
    x.set16(f::VZ, 0x400);
    x.set16(f::GRAVITY, 64);
    x.set32(f::VX, 0);
    anim(x, 0);
}
/// 0x3cacc: rises to the top of the screen (0x110 above the camera), then 0x3cafc.
fn hop_rise<R: Bench>(x: &mut Cx<'_, R>) {
    move_xz(x);
    let camera = if var::CAMERA_SELECT.get(x.b) != 0 { var::CAMERA_ALT_Y.get(x.b) } else { var::SCROLL_Y.get(x.b) };
    let above = x.u16(f::HEIGHT).wrapping_add(x.u16(f::Y)).wrapping_sub(camera);
    if above >= 0x110 {
        next(x);
    }
    hop_top(x);
}
/// 0x3cafc: when the linked object (`+0x54`) is at sub-step 4, falls back from a random height (animation 11).
fn hop_top<R: Bench>(x: &mut Cx<'_, R>) {
    let a1 = x.u16(0x54) as i16 as i32 as u32;
    if rd8(x.b, a1 + 7) != 4 {
        return;
    }
    let r = (random(x) & 7) << 4;
    x.add16(f::HEIGHT, r as u16);
    x.set8(f::STEP, 6);
    x.set16(f::VZ, 0xfc00);
    x.set16(f::GRAVITY, -128i16 as u16);
    anim(x, 11);
}
/// 0x3cb32: fall; on the ground, sound and 0x2824 (D0 = 8).
fn hop_land<R: Bench>(x: &mut Cx<'_, R>) {
    fall(x);
    if x.u16(f::HEIGHT) != 0 {
        return;
    }
    hurt(x);
    lib::shake_54(x, 8);
}

/// 0x3cbaa: backward throw (counter 1, X 0x200, height 0x600).
fn knockback_speeds<R: Bench>(x: &mut Cx<'_, R>) {
    next(x);
    x.set8(f::COUNTER, 1);
    x.set32(f::VX, 0x0200_0000);
    x.set16(f::VZ, 0x600);
    x.set16(f::GRAVITY, -64i16 as u16);
}
/// Hit sound, or death and death sound, the object remaining active (`+0` = 2).
fn hurt_active<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(f::ACTIVE, 2);
    if dead(x) {
        x.set8(f::SUB, 6);
        death_sound(x);
    } else {
        hit_sound(x);
    }
}
/// 0x3cc3e: on the ground, the link goes back to pending, animation 13.
fn tumble_anim<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(0x67, 0);
    x.set8(f::DOWNED, 0);
    defer_link(x);
    anim(x, 13);
}
/// 0x3cb6c: backward throw, somersault.
fn tumble<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(0x67, 0);
    x.set16(f::VY2, 0);
    x.set16(f::VX2, 64);
    x.set8(f::GRABBED, 0);
    knockback_speeds(x);
    face_attacker(x);
    hurt_active(x);
    tumble_anim(x);
}
/// 0x3cbca: like the somersault, animation 18.
fn tumble_18<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(0x67, 0);
    x.set8(f::DOWNED, 0);
    x.set8(f::GRABBED, 0);
    knockback_speeds(x);
    face_attacker(x);
    hurt_active(x);
    defer_link(x);
    anim(x, 18);
}
/// 0x3cc00: somersault, animation 18 and stars 0xabec8(5).
fn tumble_18_stars<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(0x67, 0);
    x.set8(f::DOWNED, 0);
    x.set8(f::GRABBED, 0);
    knockback_speeds(x);
    face_attacker(x);
    anim(x, 18);
    effect(x, 5, 0);
    hurt_active(x);
}
/// 0x3cd0c: somersault, animation 13 and stars 0xabec8(7).
fn tumble_13_stars<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(0x67, 0);
    x.set16(f::VY2, 0);
    x.set16(f::VX2, 64);
    x.set8(f::GRABBED, 0);
    x.set8(f::DOWNED, 0);
    knockback_speeds(x);
    face_attacker(x);
    anim(x, 13);
    effect(x, 7, 0);
    hurt_active(x);
}
/// 0x3cc6e: rise in three dimensions; at the top, next step (active if not dead).
fn tumble_rise<R: Bench>(x: &mut Cx<'_, R>) {
    move_xyz(x);
    if x.u16(f::VZ) as i16 > 0 {
        return tumble_blocked(x);
    }
    next(x);
    if x.u8(f::SUB) != 6 {
        x.set8(f::ACTIVE, 1);
    }
}
/// 0x3cc90: descent in three dimensions down to the ground.
fn tumble_fall<R: Bench>(x: &mut Cx<'_, R>) {
    move_xyz(x);
    if x.u16(f::HEIGHT) as i16 > 0 {
        return tumble_blocked(x);
    }
    next_clear(x);
    x.set32(f::HEIGHT, 0);
    x.set32(f::VZ, 0);
    landed(x);
}
fn tumble_blocked<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::BLOCKED) != 0 {
        x.set16(f::VX, 0);
        x.set16(f::VY, 0);
        x.set8(f::COUNTER, 0);
    }
    anim_step(x);
}
/// 0x3ccca: next step, the pending link becomes effective.
fn tumble_link<R: Bench>(x: &mut Cx<'_, R>) {
    next(x);
    take_pending(x);
}

/// 0x3cdb0 / 0x3cf1e: knocked over (animation 17, sound 0xebbda); the second adds the stars 0xabec8(8).
fn floored<R: Bench>(x: &mut Cx<'_, R>, stars: bool) {
    x.set8(0x67, 0);
    x.set8(f::DOWNED, 0);
    next(x);
    x.set8(f::GRABBED, 0);
    if dead(x) {
        mark_dead(x);
    }
    speeds(x, 0x200, 0, 0x400, -128i16 as u16);
    face_attacker(x);
    if !stars {
        defer_link(x);
    }
    anim(x, 17);
    lib::sound(x, 0xebbda);
    if stars {
        effect(x, 8, 0);
    }
}
fn floored_plain<R: Bench>(x: &mut Cx<'_, R>) {
    floored(x, false);
}
fn floored_stars<R: Bench>(x: &mut Cx<'_, R>) {
    floored(x, true);
}
/// 0x3ce4c / 0x3ce84: 30 frames on the ground; the first variant launches 0xab7d8(1).
fn floored_wait<R: Bench>(x: &mut Cx<'_, R>, cry: bool) {
    if x.u8(f::SUB_STEP) == 0 {
        next_sub(x);
        x.set8(f::COUNTER, 30);
    }
    let c = x.u8(f::COUNTER).wrapping_sub(1);
    x.set8(f::COUNTER, c);
    if c != 0 {
        return;
    }
    next_clear(x);
    x.set8(f::COUNTER, 0);
    if cry {
        lib::debris_spray(x, 1);
    }
}
fn floored_wait_cry<R: Bench>(x: &mut Cx<'_, R>) {
    floored_wait(x, true);
}
fn floored_wait_quiet<R: Bench>(x: &mut Cx<'_, R>) {
    floored_wait(x, false);
}
/// 0x3ceb4 / 0x3cecc: outside pause, next step (the first launches 0xab7d8(1)).
fn unpaused_cry<R: Bench>(x: &mut Cx<'_, R>) {
    if paused(x) {
        return;
    }
    next_clear(x);
    lib::debris_spray(x, 1);
}
fn unpaused_next<R: Bench>(x: &mut Cx<'_, R>) {
    if paused(x) {
        return;
    }
    next_clear(x);
}

/// 0x3d6f8: knocked over without a link (animation 17, stars 0xabec8(9)), oriented speeds.
fn floored_9<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(0x67, 0);
    x.set8(f::DOWNED, 0);
    next(x);
    x.set8(f::GRABBED, 0);
    x.set8(f::ACTIVE, 2);
    if dead(x) {
        x.set8(f::SUB, 6);
    }
    speeds(x, 0x200, 0, 0x400, -128i16 as u16);
    orient_speeds(x);
    anim(x, 17);
    lib::sound(x, 0xebbda);
    effect(x, 9, 0);
}

// ---------------------------------------------------------------------------------------------
// Grabs and special reactions

/// 0x3d00a: grabbed by a player (animation 14), total immunities.
fn seized<R: Bench>(x: &mut Cx<'_, R>) {
    x.set32(0x94, u32::MAX);
    apply_pending(x);
    next(x);
    x.set8(0x67, 0);
    x.set8(f::SUB_STEP, 0);
    anim(x, 14);
}
/// 0x3d02a: back to waiting for the grab.
fn seized_again<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(f::SUB_STEP, 2);
    anim(x, 14);
}
/// 0x3d036: the grab (`+7`): sticks to the player, then takes their hits.
fn seized_by<R: Bench>(x: &mut Cx<'_, R>) {
    let s = x.u8(f::SUB_STEP);
    match s {
        0 => seized_stick(x),
        2 => seized_hits(x),
        4 => {
            let c = x.u8(f::COUNTER).wrapping_sub(1);
            x.set8(f::COUNTER, c);
            if c == 0 {
                return seized_again(x);
            }
            shake(x, 2);
        }
        _ => {}
    }
}
/// 0x3d048: when the player (`+0x50`) signals the hold, the object places itself in front of them.
fn seized_stick<R: Bench>(x: &mut Cx<'_, R>) {
    let p = x.u16(f::OWNER) as i16 as i32 as u32;
    if rd8(x.b, p + 0x33) != 1 {
        return;
    }
    next_sub(x);
    let px = rd16(x.b, p + 8);
    x.set16(f::X, px);
    let i = 4 * x.u8(f::KIND) as u32;
    let mut d = x.rom16(0x3d07e + i);
    if !flipped(x) {
        d = d.wrapping_neg();
    }
    x.add16(f::X, d);
}
/// 0x3d0e8: the player (character 5, state 0x02000cxx, action 26) strikes: damage according to their hit (`+0x10c`,
/// table 0x3d0da), points (0x19dce), effect (0xb141a); otherwise end of the grab.
fn seized_hits<R: Bench>(x: &mut Cx<'_, R>) {
    let p = x.u16(f::OWNER) as i16 as i32 as u32;
    let character = rd8(x.b, p + 3) >> 1;
    if character != 5 || rd32(x.b, p + 4) & 0xffff_ff00 != 0x0200_0c00 || rd8(x.b, p + 0x6a) != 26 {
        return next_clear(x);
    }
    if (rd8(x.b, p + 0x33) as i8) >= 0 {
        return;
    }
    let hit = (rd8(x.b, p + 0x10c).wrapping_sub(11)) as u32;
    let damage = x.rom8(0x3d0da + hit) as u16;
    next_sub(x);
    crate::mem::wr8(x.b, p + 0x33, 0);
    let hp = x.u16(f::HP).wrapping_sub(damage);
    x.set16(f::HP, if (hp as i16) > 0 { hp } else { 0 });
    x.set8(f::COUNTER, 7);
    anim_step(x);
    // 0x19dce: health bar of the grabbed object, in the player's name
    let owner = x.u16(f::OWNER) as i16 as i32 as u32;
    let header = ((rd8(x.b, owner + 2) as u16) << 8) | x.u8(f::KIND) as u16;
    let popup = crate::events::Popup { header, max: x.u16(0x60), value: x.u16(f::HP), raw: x.u8(f::CLASS) == 2 };
    crate::events::create_popup(x.b, popup);
    let i = 4 * x.u8(f::KIND) as u32;
    let mut vx = x.rom16(0x3d198 + i);
    let ax = x.rom16(0x3d19a + i);
    if flipped(x) {
        vx = vx.wrapping_neg();
    }
    // leftovers: the spark's speeds in D2.w and D3.w (`dispatch::Leftovers`)
    *x.c.d2() = *x.c.d2() & !0xffff | vx as u32;
    *x.c.d3() = *x.c.d3() & !0xffff | ax as u32;
    crate::families::effects_creators::impact_spark(x.b, x.o, vx, ax);
}
/// 0x3d224: released: predefined throw 1, the type's immunities (0xfe5a).
fn released<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(f::ACTIVE, 1);
    anim(x, 3);
    hurt(x);
    launch(x, 1);
    x.set8(f::FREEZE, 1);
    if flipped(x) {
        neg32(x, f::VX);
    }
    lib::set_immunities(x);
}

/// 0x3d26a: hit received by a grabbed object (`+0x87`) or not; depending on the height, recoil (code 0) or low blow
/// (56).
fn grab_hit<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::GRABBED) == 0 {
        if x.u16(f::HEIGHT) as i16 > 0 {
            x.set8(f::REACTION, 56);
            return run_entry(x, 0x3c2ee);
        }
        x.set8(f::REACTION, 0);
        return run_entry(x, 0x3bb9c);
    }
    if dead(x) {
        mark_dead(x);
        if x.u16(f::HEIGHT) as i16 > 0 {
            x.set8(f::REACTION, 56);
            return run_entry(x, 0x3c2ee);
        }
        x.set8(f::REACTION, 0);
        return run_entry(x, 0x3dd72);
    }
    x.set8(0x67, 0);
    x.set8(f::COUNTER, 16);
    apply_pending(x);
    if x.u8(f::DOWNED) != 0 {
        x.set8(f::REACTION, 108);
        x.set8(0x67, 0);
        x.set8(f::COUNTER, 16);
        return run_entry(x, 0x3d350);
    }
    if x.u16(f::HEIGHT) as i16 <= 0 {
        x.set8(0x67, 0);
        x.set32(f::HEIGHT, 0);
        x.set8(f::STEP, 10);
        anim(x, 15);
        effect(x, 3, 0);
        return;
    }
    anim(x, 2);
    let flip = x.u8(f::FLIP) ^ 0x20;
    x.set8(f::ATTACKER_FLIP, flip);
    launch(x, 0);
    face_attacker(x);
}
/// 0x3d318: on the ground, grabbed: `+0x87` wears off, then get-up through the slide.
fn grab_down<R: Bench>(x: &mut Cx<'_, R>) {
    let c = x.u8(f::COUNTER);
    if c != 0 {
        x.set8(f::COUNTER, c - 1);
        if c == 1 {
            x.set32(0x58, 0);
            x.set8(f::ACTIVE, 1);
        }
    }
    let g = x.u8(f::GRABBED).wrapping_sub(1);
    x.set8(f::GRABBED, g);
    if g == 0 {
        return slide_end(x);
    }
    anim_step(x);
}
/// 0x3d350 (code 108): on the ground and grabbed; death through 0x3c63a.
fn grounded_grab<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if dead(x) {
        x.set8(f::SUB, 6);
        x.set8(f::REACTION, 76);
        x.set8(f::ACTIVE, 2);
        return run_entry_flag(x, 0x3c63a);
    }
    let g = x.u8(f::GRABBED).wrapping_sub(1);
    x.set8(f::GRABBED, g);
    if g == 0 {
        get_up(x);
        return true;
    }
    let c = x.u8(f::COUNTER);
    if c != 0 {
        x.set8(f::COUNTER, c - 1);
        if c == 1 {
            x.set32(0x58, 0);
            x.set8(f::ACTIVE, 1);
        }
    }
    if x.u8(f::STEP) != 0 {
        return true;
    }
    next(x);
    anim(x, 16);
    effect(x, 3, 1);
    true
}

/// 0x3d3c8: shakes in place (stars 0xabec8(1)), randomly drawn delay (table 0x3d42a).
fn quiver<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(0x67, 0);
    x.set8(f::DOWNED, 0);
    apply_pending(x);
    x.set8(f::ACTIVE, 2);
    effect(x, 1, 0);
    anim(x, 3);
    x.set8(f::DOWNED, 0);
    next(x);
    x.set8(f::COUNTER, 0);
    x.set32(f::VX, 0x0180_0000);
    x.set16(f::VZ, 0x400);
    x.set16(f::GRAVITY, -64i16 as u16);
    x.set32(f::VX2, 0);
    quiver_delay(x);
    orient_speeds(x);
    stun(x);
    hurt(x);
}
fn quiver_delay<R: Bench>(x: &mut Cx<'_, R>) {
    let r = random(x) & 3;
    let d = x.rom8(0x3d42a + r);
    x.set8(f::COUNTER, d);
}
/// 0x3d42e: the shaking; at each delay, stars one time out of four... three out of four.
fn quiver_shake<R: Bench>(x: &mut Cx<'_, R>) {
    let c = x.u8(f::COUNTER).wrapping_sub(1);
    x.set8(f::COUNTER, c);
    if (c as i8) <= 0 {
        let r = random(x) & 3;
        if r != 0 {
            effect(x, 1, 1);
        }
        quiver_delay(x);
    }
    shake(x, 1);
    if !paused(x) {
        next(x);
    }
}

/// 0x3d4a6 / 0x3d522: low somersault (X 0x100, height 0x800); the second has animation 13 and its stars.
fn roll<R: Bench>(x: &mut Cx<'_, R>, stars: bool) {
    x.set8(0x67, 0);
    next(x);
    x.set8(f::GRABBED, 0);
    if stars {
        x.set8(f::DOWNED, 0);
    }
    x.set8(f::FREEZE, 1);
    x.set8(f::COUNTER, 1);
    speeds(x, 0x100, 0, 0x800, -64i16 as u16);
    x.set16(f::VY2, 0);
    x.set16(f::VX2, 64);
    orient_speeds(x);
    if stars {
        anim(x, 13);
        effect(x, 7, 0);
        return hurt_active(x);
    }
    // 0x3d4e6: death, both sounds; then the somersault animation
    x.set8(f::ACTIVE, 2);
    if dead(x) {
        x.set8(f::SUB, 6);
        death_sound(x);
    }
    hit_sound(x);
    tumble_anim(x);
}
fn roll_plain<R: Bench>(x: &mut Cx<'_, R>) {
    roll(x, false);
}
fn roll_stars<R: Bench>(x: &mut Cx<'_, R>) {
    roll(x, true);
}

/// 0x3d5a2: electrocuted (80 frames, animation 0), it dies.
fn zapped<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(0x67, 0);
    apply_pending(x);
    next(x);
    x.set8(f::ACTIVE, 2);
    x.set8(f::SUB, 6);
    x.set8(f::COUNTER, 80);
    anim(x, 0);
}
/// 0x3d5c6: sparks every 8 frames (0xb10c8), then disappearance.
fn zapped_burn<R: Bench>(x: &mut Cx<'_, R>) {
    let c = x.u8(f::COUNTER).wrapping_sub(1);
    x.set8(f::COUNTER, c);
    if c == 0 {
        crate::families::effects_creators::toss(x.b, x.o);
        x.set32(f::STATE, 0x0600_0000);
        detach_from_host(x);
        return lib::object_event(x);
    }
    if c & 7 != 0 {
        return;
    }
    // Original glitch: in scene 41, 0xb10c8 cedes control with no effect or carry, and the rest writes into the A1
    // left over, the reaction routine itself (ROM): the writes are lost.
    let lib::Impact::Created(o) = lib::shock(x, 7, 0x200, 0, 0xffff) else { return };
    o.set_u8(x.b, 0x43, 1);
    let h = o.u16(x.b, 0xc).wrapping_add(16);
    o.set_u16(x.b, 0xc, h);
}

/// 0x3d62c: petrified (animation 20); in the air, it falls first.
fn petrified<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(0x67, 0);
    apply_pending(x);
    next(x);
    x.set8(f::SUB, 6);
    x.set8(f::ACTIVE, 2);
    defer_link(x);
    anim(x, 20);
    if x.u16(f::HEIGHT) as i16 <= 0 {
        next(x);
        x.set8(f::COUNTER, 40);
        return lib::sound(x, 0xebc4c);
    }
    x.set16(f::VZ, 0);
    x.set16(f::GRAVITY, -64i16 as u16);
    x.set32(f::VX, 0);
}
fn petrified_fall<R: Bench>(x: &mut Cx<'_, R>) {
    move_xz(x);
    if x.u16(f::HEIGHT) as i16 > 0 {
        return;
    }
    x.set32(f::HEIGHT, 0);
    next(x);
    x.set8(f::COUNTER, 1);
    landed(x);
}
fn petrified_end<R: Bench>(x: &mut Cx<'_, R>) {
    if paused(x) {
        return;
    }
    lib::debris_spray(x, 5);
    x.set32(f::STATE, 0x0600_0000);
    detach_from_host(x);
    lib::object_event(x);
}

/// 0x3d762 (code 0x5c...): in the air, throw (code 4); death on the ground; otherwise 60 frames of shaking then
/// glissade.
fn stagger<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(0x67, 0);
    if x.u16(f::HEIGHT) as i16 > 0 {
        x.set8(f::REACTION, 4);
        return run_entry(x, 0x3bc24);
    }
    if dead(x) {
        mark_dead(x);
        return run_entry(x, 0x3dd72);
    }
    next(x);
    x.set8(f::COUNTER, 60);
    stun(x);
    x.set16(f::VX, 0x180);
    x.set16(f::VY, -56i16 as u16);
    face_attacker(x);
    anim(x, 0);
}
fn stagger_shake<R: Bench>(x: &mut Cx<'_, R>) {
    let c = x.u8(f::COUNTER).wrapping_sub(1);
    x.set8(f::COUNTER, c);
    if c == 40 {
        x.set32(0x58, 0);
    }
    shake(x, 2);
    if x.u8(f::COUNTER) == 0 {
        next(x);
    }
}

/// 0x3d828: small throw (animation 3), oriented speeds.
fn hop_back<R: Bench>(x: &mut Cx<'_, R>) {
    apply_pending(x);
    next(x);
    if dead(x) {
        mark_dead(x);
    }
    anim(x, 3);
    x.set8(f::COUNTER, 1);
    x.set32(f::VX, 0x0200_0000);
    x.set16(f::VZ, 0x400);
    x.set16(f::GRAVITY, -64i16 as u16);
    x.set32(f::VX2, 0);
    orient_speeds(x);
    stun(x);
}

/// 0x3d882: burned (0xe909a), animation D0 left as is, six stars (0xabec8(4)).
fn burnt<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(0x67, 0);
    lib::sound(x, 0xe909a);
    apply_pending(x);
    next(x);
    x.set8(f::ACTIVE, 1);
    if x.u16(f::HEIGHT) as i16 > 0 {
        x.set16(f::VZ, 0);
        x.set16(f::GRAVITY, -64i16 as u16);
        x.set32(f::VX, 0);
        x.add8(f::STEP, (-2i8) as u8);
    }
    next(x);
    x.set8(f::DOWNED, 0);
    x.set8(f::GRABBED, 0);
    x.set8(f::COUNTER, 0xf0);
    // Original glitch: the animation is the D0 left by the dispatch on `+6`, the current step (0)
    anim(x, 0);
    if !lib::attack_effects(x, 4, 5) {
        x.set8(f::STEP, 6);
    }
}
fn burnt_fall<R: Bench>(x: &mut Cx<'_, R>) {
    move_xz(x);
    if x.u16(f::HEIGHT) as i16 > 0 {
        return;
    }
    x.set32(f::HEIGHT, 0);
    next(x);
}
fn burnt_wait<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(f::HEIGHT) != 0 {
        move_xz(x);
        if x.u16(f::HEIGHT) as i16 > 0 {
            return;
        }
        x.set32(f::HEIGHT, 0);
    }
    anim_step(x);
    let c = x.u8(f::COUNTER).wrapping_sub(1);
    x.set8(f::COUNTER, c);
    if c == 0 {
        slide_end(x);
    }
}

/// Start angle of a whirlwind: random, centered on the left (8) or on the right (24) depending on the orientation.
fn spin_angle<R: Bench>(x: &mut Cx<'_, R>) {
    let d2 = if flipped(x) { 24 } else { 8 };
    let r = random(x);
    let a = (r as u16).wrapping_add(d2 as u16 & 15) & 31;
    x.set8(f::ANGLE, a as u8);
    anim(x, 3);
}
/// 0x3d950: carried off by a whirlwind.
fn whirl<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(0x67, 0);
    x.set8(f::DOWNED, 0);
    apply_pending(x);
    next(x);
    x.set32(f::VX, 0);
    x.set32(f::VZ, 0);
    x.set32(f::VX2, 0);
    x.set8(f::GRABBED, 0);
    x.set8(f::DOWNED, 0);
    let c = if x.counter() & 1 != 0 { 1 } else { 2 };
    x.set8(f::COUNTER, c);
    spin_angle(x);
}
/// Next turn of the angle (step of 2 out of 32); the orientation follows the half-circle.
fn turn_angle<R: Bench>(x: &mut Cx<'_, R>) {
    let a = x.u8(f::ANGLE).wrapping_add(2) & 31;
    x.set8(f::ANGLE, a);
    x.set8(f::FLIP, 0);
    let d = a.wrapping_sub(8);
    if d >= 16 {
        x.set8(f::FLIP, 32);
    }
}
/// 0x3da0e: position on the circle (table 0x1c94b6: X, depth), speeds added.
fn orbit<R: Bench>(x: &mut Cx<'_, R>) {
    const CIRCLE: u32 = 0x1c94b6;
    let i = 4 * x.u8(f::ANGLE) as u32;
    let vx = rd16(x.b, CIRCLE + i).wrapping_add(x.u16(f::VY));
    x.set16(f::VX, vx);
    x.add32(f::X, ((vx as i16 as i32) << 8) as u32);
    let vy = rd16(x.b, CIRCLE + i + 2).wrapping_add(x.u16(f::VY2));
    x.set16(f::VX2, vy);
    let d = ((vy as i16 as i32) << 8) as u32;
    x.add32(f::Y, d);
}
/// 0x3d9b2: rises while spinning up to 96, then falls back.
fn whirl_rise<R: Bench>(x: &mut Cx<'_, R>) {
    x.add32(f::HEIGHT, 0x2_0000);
    if x.u16(f::HEIGHT) as i16 >= 96 {
        next(x);
        x.set16(f::VZ, 0x200);
        x.set16(f::GRAVITY, -128i16 as u16);
        return;
    }
    let c = x.u8(f::COUNTER).wrapping_sub(1);
    x.set8(f::COUNTER, c);
    if c == 0 {
        x.set8(f::COUNTER, 1);
        turn_angle(x);
    }
    orbit(x);
}
/// 0x3da44: falls back; on the ground, landing and sound.
fn whirl_fall<R: Bench>(x: &mut Cx<'_, R>) {
    anim_step(x);
    move_xyz(x);
    if x.u16(f::HEIGHT) as i16 > 0 {
        if x.u8(f::BLOCKED) != 0 {
            x.set16(f::VX, 0);
            x.set16(f::VY, 0);
            x.set8(f::COUNTER, 0);
        }
        return;
    }
    next(x);
    x.set32(f::HEIGHT, 0);
    x.set32(f::VZ, 0);
    landed(x);
    hurt(x);
}

/// 0x3dc30: boss whirlwind (60 frames).
fn spin<R: Bench>(x: &mut Cx<'_, R>) {
    next(x);
    x.set8(f::COUNTER, 60);
    apply_pending(x);
    x.set32(f::VX, 0);
    x.set32(f::VZ, 0);
    x.set32(f::VX2, 0);
    let g = if x.counter() & 1 != 0 { 1 } else { 2 };
    x.set8(f::GRABBED, g);
    spin_angle(x);
}
/// 0x3dc86: spins while rising up to 64.
fn spin_rise<R: Bench>(x: &mut Cx<'_, R>) {
    let c = x.u8(f::COUNTER).wrapping_sub(1);
    x.set8(f::COUNTER, c);
    if c == 0 {
        return next(x);
    }
    x.add32(f::HEIGHT, 0x2_0000);
    if x.u16(f::HEIGHT) as i16 >= 64 {
        x.set32(f::HEIGHT, 0x0040_0000);
    }
    let g = x.u8(f::GRABBED).wrapping_sub(1);
    x.set8(f::GRABBED, g);
    if g == 0 {
        x.set8(f::GRABBED, 1);
        turn_angle(x);
    }
    orbit(x);
}
/// 0x3db4e: the boss becomes active again (state 4) and falls back knocked over (animation 17).
fn spin_drop<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(f::STATE, 4);
    x.set8(f::ACTIVE, 2);
    next(x);
    x.set8(f::COUNTER, 30);
    x.set8(f::SUB_STEP, 0);
    x.set8(0x67, 0);
    x.set8(f::DOWNED, 0);
    x.set8(f::GRABBED, 0);
    x.set32(f::VX, 0);
    x.set16(f::VZ, 0x600);
    x.set16(f::GRAVITY, -128i16 as u16);
    orient_speeds(x);
    defer_link(x);
    anim(x, 17);
    lib::sound(x, 0xebbda);
}
/// 0x3dada: variant: animation 17 and stars 0xabec8(8).
fn spin_drop_stars<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(0x67, 0);
    next(x);
    x.set8(f::COUNTER, 30);
    x.set8(f::SUB_STEP, 0);
    x.set32(f::VX, 0);
    x.set16(f::VZ, 0x600);
    x.set16(f::GRAVITY, -128i16 as u16);
    orient_speeds(x);
    anim(x, 17);
    lib::sound(x, 0xebbda);
    effect(x, 8, 0);
}
/// 0x3dbb4: animation; fall to the ground, then wait for the counter to end.
fn spin_fall<R: Bench>(x: &mut Cx<'_, R>) {
    anim_step(x);
    if x.u8(f::SUB_STEP) == 0 {
        move_xz(x);
    }
    let c = x.u8(f::COUNTER).wrapping_sub(1);
    x.set8(f::COUNTER, c);
    if c == 0 {
        return next_clear(x);
    }
    if x.u16(f::HEIGHT) as i16 > 0 {
        return;
    }
    x.set32(f::HEIGHT, 0);
    next_sub(x);
}
/// 0x3dbec: high bounce (60 frames, animation 13).
fn spin_bounce<R: Bench>(x: &mut Cx<'_, R>) {
    next(x);
    x.set32(f::VX, 0);
    x.set8(f::COUNTER, 60);
    x.set16(f::VZ, 0x800);
    x.set16(f::GRAVITY, -64i16 as u16);
    hit_sound(x);
    defer_link(x);
    anim(x, 13);
}
/// 0x3db18: variant: stars 0xabec8(7), then the sound.
fn spin_bounce_stars<R: Bench>(x: &mut Cx<'_, R>) {
    next(x);
    x.set8(f::COUNTER, 60);
    x.set32(f::VX, 0);
    x.set32(f::VX2, 0);
    x.set16(f::VZ, 0x800);
    x.set16(f::GRAVITY, -64i16 as u16);
    orient_speeds(x);
    anim(x, 13);
    effect(x, 7, 0);
    hit_sound(x);
}
/// 0x3dcea: final skimming throw (stars 0xabec8(2)).
fn spin_blast<R: Bench>(x: &mut Cx<'_, R>) {
    next(x);
    apply_pending(x);
    x.set8(f::COUNTER, 18);
    x.set16(f::VX, 0x1000);
    x.set16(f::VY, 0);
    x.set32(f::VZ, 0x0100_0000);
    x.set16(f::GRAVITY, 0);
    anim(x, 1);
    if flipped(x) {
        neg16(x, f::VX);
    }
    effect(x, 2, 0);
}
/// 0x3dd2e: boss death; outside ENEMY, `A5+0x4f` signals the end; some types keep their step.
fn spin_death<R: Bench>(x: &mut Cx<'_, R>) {
    next(x);
    x.set8(f::ACTIVE, 2);
    x.set16(f::HP, 0);
    x.set8(f::SUB, 6);
    if x.u8(f::CLASS) != ENEMY {
        x.set_a5_u8(0x4f, 0xff);
        return x.set16(f::STEP, 0);
    }
    if [8, 18, 27].contains(&x.u8(f::KIND)) {
        x.set16(f::STEP, 0);
    }
}

/// 0x3dd8c / 0x3dd94: death in the air: sound, then low throw (animation 2 or 3).
fn death_throw<R: Bench>(x: &mut Cx<'_, R>) {
    death_sound(x);
    throw_1(x);
}
fn death_throw_b<R: Bench>(x: &mut Cx<'_, R>) {
    death_sound(x);
    throw_1b(x);
}

// ---------------------------------------------------------------------------------------------
// The reaction routines (entries of the per-type tables)

/// Original dispatch on `+6` (`moveq #0,D0 ; move.b 6(A0),D0 ; move.w table(PC,D0),D1 ; jmp`): step
/// `+6` = 2 × i runs the i-th function of the list; false for a step outside the table (or odd).
macro_rules! step {
    ($x:ident; $($f:path),+ $(,)?) => {{
        let s = $x.u8(f::STEP);
        s & 1 == 0 && step!(@ $x, s, 0u8; $($f),+)
    }};
    (@ $x:ident, $s:ident, $i:expr; $f:path $(, $remainder:path)*) => {
        if $s == $i {
            $f($x);
            true
        } else {
            step!(@ $x, $s, $i + 2; $($remainder),*)
        }
    };
    (@ $x:ident, $s:ident, $i:expr;) => {
        false
    };
}

/// Common end steps: get-up, death.
macro_rules! standard {
    ($x:ident, $first:path) => {
        step!($x; $first, wait_anim, fall, boss_3_hook, bounce_start, bounce, get_up, death)
    };
}
macro_rules! high {
    ($x:ident, $first:path) => {
        step!($x; $first, wait_anim, step_51c06, fall_38, skip, bounce_high, bounce, get_up, death)
    };
}
macro_rules! slides {
    ($x:ident, $first:path) => {
        step!($x; $first, wait_anim, step_51c06, fall_38, skip, slide_start_fast, slide_fast, get_up, death)
    };
}

/// The reaction routines with a jump table at the head: (entry, table on `+6`).
#[cfg(test)]
const REACTIONS: &[(u32, u32)] = &[
    (0x3bc24, 0x3bc32),
    (0x3bc82, 0x3bc90),
    (0x3bce2, 0x3bcf0),
    (0x3bd42, 0x3bd50),
    (0x3bf56, 0x3bf64),
    (0x3bfc0, 0x3bfce),
    (0x3c01c, 0x3c02a),
    (0x3c07a, 0x3c088),
    (0x3c0d4, 0x3c0e2),
    (0x3c12e, 0x3c13c),
    (0x3c1c2, 0x3c1d0),
    (0x3c1e2, 0x3c1f0),
    (0x3c2ee, 0x3c2fc),
    (0x3c35a, 0x3c368),
    (0x3c4b4, 0x3c4c2),
    (0x3c570, 0x3c57e),
    (0x3c70c, 0x3c71a),
    (0x3c784, 0x3c792),
    (0x3c8b8, 0x3c8c6),
    (0x3c954, 0x3c962),
    (0x3c9cc, 0x3c9da),
    (0x3ca56, 0x3ca64),
    (0x3ca76, 0x3ca84),
    (0x3cb4c, 0x3cb5a),
    (0x3ccee, 0x3ccfc),
    (0x3cd70, 0x3cd7e),
    (0x3cd90, 0x3cd9e),
    (0x3cede, 0x3ceec),
    (0x3cefe, 0x3cf0c),
    (0x3cf8e, 0x3cf9c),
    (0x3cfd0, 0x3cfde),
    (0x3cfea, 0x3cff8),
    (0x3d250, 0x3d25e),
    (0x3d3a8, 0x3d3b6),
    (0x3d488, 0x3d496),
    (0x3d504, 0x3d512),
    (0x3d590, 0x3d59e),
    (0x3d618, 0x3d626),
    (0x3d6d8, 0x3d6e6),
    (0x3d74e, 0x3d75c),
    (0x3d80c, 0x3d81a),
    (0x3d932, 0x3d940),
    (0x3da86, 0x3da94),
    (0x3dab0, 0x3dabe),
    (0x3dd72, 0x3dd80),
    (0x3ddf6, 0x3de04),
];

/// The steps of reaction routine `entry` (one of `REACTIONS`), on `+6`; `None` if `entry` is not one.
fn run_table<R: Bench>(x: &mut Cx<'_, R>, entry: u32) -> Option<bool> {
    Some(match entry {
        0x3bc24 /* table 0x3bc32 */ => standard!(x, throw_1),
        0x3bc82 /* table 0x3bc90 */ => standard!(x, throw_2),
        0x3bce2 /* table 0x3bcf0 */ => standard!(x, throw_3),
        0x3bd42 /* table 0x3bd50 */ => standard!(x, throw_4),
        0x3bf56 /* table 0x3bf64 */ => high!(x, uppercut_2),
        0x3bfc0 /* table 0x3bfce */ => standard!(x, throw_1b),
        0x3c01c /* table 0x3c02a */ => standard!(x, throw_2b),
        0x3c07a /* table 0x3c088 */ => standard!(x, throw_3b),
        0x3c0d4 /* table 0x3c0e2 */ => standard!(x, throw_4b),
        0x3c12e /* table 0x3c13c */ => high!(x, uppercut_3),
        0x3c1c2 /* table 0x3c1d0 */ => slides!(x, uppercut_2),
        0x3c1e2 /* table 0x3c1f0 */ => slides!(x, uppercut_3),
        0x3c2ee /* table 0x3c2fc */ => step!(x; low_hit, fall, boss_3_hook, bounce_start, bounce, get_up, death),
        0x3c35a /* table 0x3c368 */ => step!(x; skim, skim_flight, bounce_start, bounce, get_up, death),
        0x3c4b4 /* table 0x3c4c2 */ => step!(x; throw_1, wait_anim, step_51c06, fall, slide_start_slow, slide_slow, get_up, death),
        0x3c570 /* table 0x3c57e */ => step!(x; pop_by_code, wait_anim, fall_animated, bounce_start, bounce, get_up, death),
        0x3c70c /* table 0x3c71a */ => step!(x; pop_10, wait_anim, fall_animated, bounce_start, bounce, get_up, death),
        0x3c784 /* table 0x3c792 */ => step!(x; pop_11, step_51c06, fall_38, sparks, skip, bounce_back, bounce, get_up, death),
        0x3c8b8 /* table 0x3c8c6 */ => step!(x; dazed, dazed_wait, knock, wait_anim, fall, bounce_start, bounce, get_up, death),
        0x3c954 /* table 0x3c962 */ => step!(x; blast, idle, skim_flight, bounce_start, bounce, get_up, death),
        0x3c9cc /* table 0x3c9da */ => step!(x; wind_up, wind_up_wait, launch_3_kill, wait_anim, fall, bounce_start, bounce, get_up, death),
        0x3ca56 /* table 0x3ca64 */ => step!(x; wind_up, wind_up_wait, launch_3_hurt, wait_anim, fall, bounce_start, bounce, get_up, death),
        0x3ca76 /* table 0x3ca84 */ => step!(x; hop, hop_rise, hop_top, hop_land, sparks, bounce_back, bounce, get_up, death),
        0x3cb4c /* table 0x3cb5a */ => step!(x; tumble, tumble_rise, tumble_fall, tumble_link, boss_3_hook, bounce_start, bounce, get_up, death),
        0x3ccee /* table 0x3ccfc */ => step!(x; tumble_13_stars, tumble_rise, tumble_fall, boss_3_hook, bounce_start, bounce, get_up, death),
        0x3cd70 /* table 0x3cd7e */ => step!(x; floored_plain, fall, floored_wait_cry, bounce_linked, fall, bounce_start, bounce, get_up, death),
        0x3cd90 /* table 0x3cd9e */ => step!(x; floored_stars, fall, floored_wait_quiet, bounce_2, fall, bounce_start, bounce, get_up, death),
        0x3cede /* table 0x3ceec */ => step!(x; floored_plain, fall, unpaused_cry, bounce_linked, fall, bounce_start, bounce, get_up, death),
        0x3cefe /* table 0x3cf0c */ => step!(x; floored_stars, fall, unpaused_next, bounce_2, fall, bounce_start, bounce, get_up, death),
        0x3cf8e /* table 0x3cf9c */ => step!(x; tumble_18, fall, tumble_link, bounce_start, bounce, get_up, death),
        0x3cfd0 /* table 0x3cfde */ => step!(x; tumble_18_stars, fall, bounce_start, bounce, get_up, death),
        0x3cfea /* table 0x3cff8 */ => step!(x; seized, seized_by, released, wait_anim, fall, bounce_start, bounce, get_up, death),
        0x3d250 /* table 0x3d25e */ => step!(x; grab_hit, fall, bounce_start, bounce, get_up, grab_down),
        0x3d3a8 /* table 0x3d3b6 */ => step!(x; quiver, quiver_shake, knock, wait_anim, fall, bounce_start, bounce, get_up, death),
        0x3d488 /* table 0x3d496 */ => step!(x; roll_plain, tumble_rise, tumble_fall, tumble_link, bounce_start, bounce, get_up, death),
        0x3d504 /* table 0x3d512 */ => step!(x; roll_stars, tumble_rise, tumble_fall, tumble_link, bounce_start, bounce, get_up, death),
        0x3d590 /* table 0x3d59e */ => step!(x; zapped, zapped_burn),
        0x3d618 /* table 0x3d626 */ => step!(x; petrified, petrified_fall, petrified_end),
        0x3d6d8 /* table 0x3d6e6 */ => step!(x; floored_9, fall, unpaused_next, bounce_2, fall, bounce_start, bounce, get_up, death),
        0x3d74e /* table 0x3d75c */ => step!(x; stagger, stagger_shake, slide),
        0x3d80c /* table 0x3d81a */ => step!(x; hop_back, wait_anim, fall, bounce_start, bounce, get_up, death),
        0x3d932 /* table 0x3d940 */ => step!(x; whirl, whirl_rise, whirl_fall, sparks, bounce_start, bounce, get_up, death),
        0x3da86 /* table 0x3da94 */ => step!(x; spin, spin_rise, spin_drop, spin_fall, spin_bounce, spin_fall, spin_blast, idle, spin_death, skim_flight, bounce_start, bounce, get_up, death),
        0x3dab0 /* table 0x3dabe */ => step!(x; spin, spin_rise, spin_drop_stars, spin_fall, spin_bounce_stars, spin_fall, spin_blast, idle, spin_death, skim_flight, bounce_start, bounce, get_up, death),
        0x3dd72 /* table 0x3dd80 */ => step!(x; death_throw, wait_anim, fall, bounce_start, bounce, death),
        0x3ddf6 /* table 0x3de04 */ => step!(x; death_throw_b, wait_anim, fall, bounce_start, bounce, death),
        _ => return None,
    })
}

/// Routines without a jump table at the head.
const RECOIL: u32 = 0x3bb9c;
const RECOIL_1: u32 = 0x3bf76;
const HELD: u32 = 0x3c63a;
const DISINTEGRATE: u32 = 0x3c828;
const GROUNDED_GRAB: u32 = 0x3d350;
const BURNT: u32 = 0x3d86e;

/// Runs the original reaction routine `entry`; false if the step is outside its table (see `native_run`).
fn run_entry_flag<R: Bench>(x: &mut Cx<'_, R>, entry: u32) -> bool {
    match entry {
        RECOIL => {
            recoil(x);
            true
        }
        RECOIL_1 => {
            recoil_1(x);
            true
        }
        HELD => {
            let n = x.u16(f::RECOVER).wrapping_sub(1);
            x.set16(f::RECOVER, n);
            step!(x; held_start, held_shake, held_end, death) // table 0x3c64c
        }
        DISINTEGRATE => disintegrate(x),
        GROUNDED_GRAB => grounded_grab(x),
        // `move.b 6(A0),D0` without `moveq`: the index is the low word of D0, whose high byte is zero (D0 = the
        // reaction
        // code, one byte, set by the caller: `enemy_common::react`, the bosses' own tables)
        BURNT => step!(x; burnt, burnt_fall, anim_step, burnt_wait), // table 0x3d87a
        _ => run_table(x, entry).unwrap_or(false),
    }
}

/// Jump to another reaction routine (`bra`) from a step.
fn run_entry<R: Bench>(x: &mut Cx<'_, R>, entry: u32) {
    let step = x.u8(f::STEP);
    assert!(run_entry_flag(x, entry), "reaction {entry:#x}: step {step:#x} outside the original table");
}

macro_rules! reaction_natives {
    ($($entry:expr),* $(,)?) => {
        &[$(Native {
            entry: $entry,
            name: "enemy_reaction",
            run: crate::Body::Handler(|b, a| run_entry(&mut crate::families::cx::Gx::dispatch(b, a), $entry)),
            touches_sound: false,
            outputs: 0,
        }),*]
    };
}

/// The reaction routines, at their original address (called by `jsr (A1)` from the per-type tables).
pub const NATIVES: &[Native] = reaction_natives![
    0x3bb9c, 0x3bc24, 0x3bc82, 0x3bce2, 0x3bd42, 0x3bf56, 0x3bf76, 0x3bfc0, 0x3c01c, 0x3c07a, 0x3c0d4, 0x3c12e,
    0x3c1c2, 0x3c1e2, 0x3c2ee, 0x3c35a, 0x3c4b4, 0x3c570, 0x3c63a, 0x3c70c, 0x3c784, 0x3c828, 0x3c8b8, 0x3c954,
    0x3c9cc, 0x3ca56, 0x3ca76, 0x3cb4c, 0x3ccee, 0x3cd70, 0x3cd90, 0x3cede, 0x3cefe, 0x3cf8e, 0x3cfd0, 0x3cfea,
    0x3d250, 0x3d350, 0x3d3a8, 0x3d488, 0x3d504, 0x3d590, 0x3d618, 0x3d6d8, 0x3d74e, 0x3d80c, 0x3d86e, 0x3d932,
    0x3da86, 0x3dab0, 0x3dd72, 0x3ddf6,
];

/// Runs the common reaction routine `entry` if it is one (false otherwise): direct call from
/// `enemy_common::react`, without going through the native.
pub fn run_common<R: Bench>(x: &mut Cx<'_, R>, entry: u32) -> bool {
    if !NATIVES.iter().any(|n| n.entry == entry) {
        return false;
    }
    let step = x.u8(f::STEP);
    assert!(run_entry_flag(x, entry), "reaction {entry:#x}: step {step:#x} outside the original table");
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each reaction routine has a native, and vice versa.
    #[test]
    fn routines_and_natives() {
        for &(entry, _) in REACTIONS {
            assert!(NATIVES.iter().any(|n| n.entry == entry), "{entry:#x}");
        }
        assert_eq!(NATIVES.len(), REACTIONS.len() + 6);
    }
}
