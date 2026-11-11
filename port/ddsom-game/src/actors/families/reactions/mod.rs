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
//! pending link `+0x91` (bit 7) to `+0x2a`/`+0x2b`; attacker orientation `+0x4a`.

use crate::actors::families::cx::Bench;
use super::cx::Cx;
use super::enemy_lib as lib;
use super::enemy_common::var;
use super::enemy_common::detach_from_host;
use super::enemy_common::tables::{self, Table};
use super::effects::records::Effect;
use crate::world::objects::Obj;
use crate::actors::player::Player;
use crate::Native;
use crate::world::objects::pools;
use crate::world::objects::Current;
use crate::world::machine::Machine;

mod grabs;
mod hits;
mod spins;
mod steps;
mod throws;

use grabs::*;
use hits::*;
use spins::*;
use steps::*;
use throws::*;

/// Fields of the hit objects (enemies and bosses).
use super::enemy_common::Enemy as E;

/// Classes (byte `+0x28`).
const ENEMY: u8 = 2;
const BOSS: u8 = 4;

/// Empty routine (`rts`) of the per-type tables: no sound, no get-up (routine addresses, keys of the tables).
const NOTHING: [u32; 2] = [0x3e2aa, 0x3bb9a];

// ---------------------------------------------------------------------------------------------
// Helpers

fn next<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(E::STEP, 2);
}
fn next_sub<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(E::SUB_STEP, 2);
}
/// Next step, sub-step at zero.
fn next_clear<R: Bench>(x: &mut Cx<'_, R>) {
    next(x);
    x.set8(E::SUB_STEP, 0);
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
    x.u8(E::FLIP) != 0
}
fn dead<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    (x.u16(E::HP) as i16) <= 0
}
fn paused<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    var::PAUSE.get(x.b) != 0
}
/// Death: substate 6 (+ `+0` = 2).
fn mark_dead<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(E::SUB, 6);
    x.set8(E::ACTIVE, 2);
}
fn speeds<R: Bench>(x: &mut Cx<'_, R>, vx: u16, vy: u16, vz: u16, gravity: u16) {
    x.set16(E::VX, vx);
    x.set16(E::AX, vy);
    x.set16(E::VZ, vz);
    x.set16(E::GRAVITY, gravity);
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
/// Effect `0xabec8` with given variant and setting; false if the pool refused it.
fn effect<R: Bench>(x: &mut Cx<'_, R>, variant: u32, setting: u32) -> bool {
    lib::attack_effect(x, variant, setting)
}
/// Index into the per-type tables: type, plus 32 outside the ENEMY class.
fn type_index<R: Bench>(x: &mut Cx<'_, R>) -> u32 {
    let base = if x.u8(E::CLASS) == ENEMY { 0 } else { 32 };
    (base + x.u8(E::KIND) as u32) & 0xff
}

/// 0x3de26: the freeze after the hit takes the stun duration.
fn stun<R: Bench>(x: &mut Cx<'_, R>) {
    let s = x.u8(E::STUN);
    x.set8(E::FREEZE, s);
}

/// 0x3de2e: animation `n` of the type's set (types 13, 19, 34 and 54 add a variant `+0xbf`).
fn anim<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    let i = type_index(x);
    let entry = tables::anim_sets().nth32(x.b, i);
    let (set, n) = if entry >> 16 != 0 {
        (entry, n as u16)
    } else {
        // own routine: `bsr 0x3e27e ; movea.l #game,A4 ; jmp 0x1a32` (0x3e27e adds the variant to the low byte); its
        // set is in `anim_set_routines`
        let v = x.u8(E::ANIM_VARIANT) as u32;
        let add = tables::anim_variants().at8(x.b, v);
        let n = n as u16 & !0xff | (n as u8).wrapping_add(add) as u16;
        (tables::anim_set_routine(i).at32(x.b, 0), n)
    };
    x.anim_start(set, n);
}

/// Routine from a per-type table (PC-relative longs, `jsr (A1)` with D0 = the index); `None` for empty entries
/// (`rts`).
fn per_type<R: Bench>(x: &mut Cx<'_, R>, table: &Table, index: u32) -> Option<u32> {
    let routine = table.nth32(x.b, index);
    (!NOTHING.contains(&routine)).then_some(routine)
}
/// 0x3e28e and 0x3e388: hit sound, death sound (per-type sound tables: the routine read is the sound).
fn hit_sound<R: Bench>(x: &mut Cx<'_, R>) {
    let i = type_index(x);
    if let Some(sound) = per_type(x, tables::hit_sounds(), i) {
        lib::sound(x, crate::hw::sound::Sound::at(sound));
    }
}
fn death_sound<R: Bench>(x: &mut Cx<'_, R>) {
    let i = type_index(x);
    if let Some(sound) = per_type(x, tables::death_sounds(), i) {
        lib::sound(x, crate::hw::sound::Sound::at(sound));
    }
}
/// 0x3e000: type-specific get-up (per-type table; the address read designates its family's handler).
fn get_up_by_kind<R: Bench>(x: &mut Cx<'_, R>, index: u32) {
    let Some(routine) = per_type(x, tables::get_up_routines(), index) else { return };
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
    let flip = x.u8(E::ATTACKER_FLIP) ^ 0x20;
    // leftover: the orientation in the low byte of D2 (`dispatch::Leftovers::d2`, speed of the spark of
    // `flash_and_shake`)
    *x.c.d2() = *x.c.d2() & !0xff | flip as u32;
    x.set8(E::FLIP, flip);
    if flip != 0 {
        neg16(x, E::VX);
        neg16(x, E::AX);
    }
}
/// 0x3e148: the speeds follow the current orientation.
fn orient_speeds<R: Bench>(x: &mut Cx<'_, R>) {
    if flipped(x) {
        neg16(x, E::VX);
        neg16(x, E::AX);
    }
}

/// 0x3e25e: a pending link (`+0x91`, bit 7) becomes effective (`+0x2b`, `+0x2a` raised).
fn apply_pending<R: Bench>(x: &mut Cx<'_, R>) {
    let p = x.u8(E::PENDING_PALETTE);
    if (p as i8) >= 0 {
        return;
    }
    x.set8(E::PALETTE, p & 0x7f);
    x.set8(E::OWN_PALETTE, 1);
    x.set8(E::PENDING_PALETTE, 0);
}
/// Variant that always clears `+0x91`.
fn take_pending<R: Bench>(x: &mut Cx<'_, R>) {
    let p = x.u8(E::PENDING_PALETTE);
    if (p as i8) < 0 {
        x.set8(E::PALETTE, p & 0x7f);
        x.set8(E::OWN_PALETTE, 1);
    }
    x.set8(E::PENDING_PALETTE, 0);
}
/// Inverse: the effective link goes back to pending (`+0x2b` | 0x80 in `+0x91`).
fn defer_link<R: Bench>(x: &mut Cx<'_, R>) {
    if (x.u8(E::PENDING_PALETTE) as i8) < 0 || x.u8(E::OWN_PALETTE) == 0 {
        return;
    }
    let l = x.u8(E::PALETTE) | 0x80;
    x.set8(E::PENDING_PALETTE, l);
    x.set8(E::OWN_PALETTE, 0);
}

/// 0x3bda2: predefined throw `n` (counter and four speeds from the ROM).
fn launch<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.set8(E::DOWNED, 0);
    next(x);
    let c = tables::launch_counters().at8(x.b, n);
    x.set8(E::COUNTER, c);
    let speeds = tables::launch_speeds();
    for (k, off) in [E::VX, E::AX, E::VZ, E::GRAVITY].into_iter().enumerate() {
        let v = speeds.at16(x.b, 8 * n + 2 * k as u32);
        x.set16(off, v);
    }
}

/// Shaking in place: X oscillates by ±2 according to the counter parity and the orientation.
fn shake<R: Bench>(x: &mut Cx<'_, R>, amount: u16) {
    let mut d = amount;
    if x.u8(E::COUNTER) & 1 != 0 {
        d = d.wrapping_neg();
    }
    if flipped(x) {
        d = d.wrapping_neg();
    }
    x.add16(E::X, d);
}

// ---------------------------------------------------------------------------------------------
// The reaction routines (entries of the per-type tables)

/// Original dispatch on `+6` (`moveq #0,D0 ; move.b 6(A0),D0 ; move.w table(PC,D0),D1 ; jmp`): step
/// `+6` = 2 × i runs the i-th function of the list; false for a step outside the table (or odd).
macro_rules! step {
    ($x:ident; $($f:path),+ $(,)?) => {{
        let s = $x.u8(E::STEP);
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

/// Runs the original reaction routine `entry`; false if the step is outside its table (see `native_run`). The routines
/// without a jump table at their head come first.
fn run_entry_flag<R: Bench>(x: &mut Cx<'_, R>, entry: u32) -> bool {
    match entry {
        0x3bb9c /* recoil */ => {
            recoil(x);
            true
        }
        0x3bf76 /* recoil, animation 1 */ => {
            recoil_1(x);
            true
        }
        0x3c63a /* held */ => {
            let n = x.u16(E::RECOVER).wrapping_sub(1);
            x.set16(E::RECOVER, n);
            step!(x; held_start, held_shake, held_end, death) // table 0x3c64c
        }
        0x3c828 /* disintegration */ => disintegrate(x),
        0x3d350 /* grab on the ground */ => grounded_grab(x),
        // `move.b 6(A0),D0` without `moveq`: the index is the low word of D0, whose high byte is zero (D0 = the
        // reaction code, one byte, set by the caller: `enemy_common::react`, the bosses' own tables)
        0x3d86e /* burn */ => step!(x; burnt, burnt_fall, anim_step, burnt_wait), // table 0x3d87a
        _ => run_table(x, entry).unwrap_or(false),
    }
}

/// Jump to another reaction routine (`bra`) from a step.
fn run_entry<R: Bench>(x: &mut Cx<'_, R>, entry: u32) {
    let step = x.u8(E::STEP);
    assert!(run_entry_flag(x, entry), "reaction {entry:#x}: step {step:#x} outside the original table");
}

macro_rules! reaction_routines {
    ($($entry:expr),* $(,)?) => {
        /// The common reaction routines, by original address: the reaction tables of the enemies and bosses designate
        /// them by this address (`enemy_common::react`).
        const ROUTINES: &[u32] = &[$($entry),*];

        /// Their natives, at their original address (called by `jsr (A1)` from the per-type tables): the tools.
        pub const NATIVES: &[Native] = &[$(Native {
            entry: $entry,
            name: "enemy_reaction",
            run: crate::Body::Handler(|b, a| run_entry(&mut crate::actors::families::cx::Gx::dispatch(b, a), $entry)),
            touches_sound: false,
            outputs: 0,
        }),*];
    };
}

reaction_routines![
    0x3bb9c, 0x3bc24, 0x3bc82, 0x3bce2, 0x3bd42, 0x3bf56, 0x3bf76, 0x3bfc0, 0x3c01c, 0x3c07a, 0x3c0d4, 0x3c12e,
    0x3c1c2, 0x3c1e2, 0x3c2ee, 0x3c35a, 0x3c4b4, 0x3c570, 0x3c63a, 0x3c70c, 0x3c784, 0x3c828, 0x3c8b8, 0x3c954,
    0x3c9cc, 0x3ca56, 0x3ca76, 0x3cb4c, 0x3ccee, 0x3cd70, 0x3cd90, 0x3cede, 0x3cefe, 0x3cf8e, 0x3cfd0, 0x3cfea,
    0x3d250, 0x3d350, 0x3d3a8, 0x3d488, 0x3d504, 0x3d590, 0x3d618, 0x3d6d8, 0x3d74e, 0x3d80c, 0x3d86e, 0x3d932,
    0x3da86, 0x3dab0, 0x3dd72, 0x3ddf6,
];

/// Runs the common reaction routine `entry` if it is one (false otherwise): direct call from
/// `enemy_common::react`, without going through the native.
pub fn run_common<R: Bench>(x: &mut Cx<'_, R>, entry: u32) -> bool {
    if !ROUTINES.contains(&entry) {
        return false;
    }
    let step = x.u8(E::STEP);
    assert!(run_entry_flag(x, entry), "reaction {entry:#x}: step {step:#x} outside the original table");
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each reaction routine with a jump table is a common routine, and vice versa (plus the six without a table).
    #[test]
    fn routines_and_natives() {
        for &(entry, _) in REACTIONS {
            assert!(ROUTINES.contains(&entry), "{entry:#x}");
        }
        assert_eq!(ROUTINES.len(), REACTIONS.len() + 6);
        assert_eq!(NATIVES.len(), ROUTINES.len());
    }
}
