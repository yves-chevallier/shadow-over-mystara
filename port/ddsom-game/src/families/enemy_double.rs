//! Graphical doubles of enemies (ENEMY types 22, 23 and 24). See `docs/families/enemy.md`.
//!
//! A double is an enemy with no logic of its own: it copies its master (word `+0xb2`, an enemy of
//! type 25 or 26 that creates it in pairs) and plays the animation that the master's state commands. Two
//! sets of decision tables, with the same frame: the master's state (`+4`) leads to its substate
//! (`+5`), then to `+6` and the current gesture `+7`, which designate a ROM animation set.

use crate::families::cx::Bench;
use super::cx::{handlers, Cx};
use super::enemy_common::*;

/// Animation to start: nothing, via `0x10428` (set chosen from the master's `+0x33`, then
/// `+0x44 = +0x3e`) or via `0x10450` (from the master's `+0x3e`: last hit).
#[derive(Debug, PartialEq)]
enum Play {
    Nothing,
    Anim(u32),
    Hit(u32),
    /// If `+0xb6` is raised: lowers it and starts entry `n` of set 0x16abd6 (`0x103d6`); otherwise one step
    /// d'animation (`0x1040c`).
    Restart(u32),
    /// Like `Restart`, and at the end of an animation cycle (`+0x33`) creates an ESHL 0x31 shot from the master.
    RestartSpawn(u32),
    /// Animation via `0x10428`, then `+0x38 = 0`.
    AnimClear(u32),
    /// One animation step (`0x1040c`).
    Step,
}
use Play::*;
use crate::objects::pools;
use crate::objects::Current;

/// Master states (`+4` to `+7`).
struct Master {
    state: u8,
    sub: u8,
    s6: u8,
    s7: u8,
}

impl Master {
    fn read<R: Bench>(x: &mut Cx<'_, R>) -> Master {
        let m = x.linked(MASTER);
        Master { state: m.u8(x.b, 4), sub: m.u8(x.b, 5), s6: m.u8(x.b, 6), s7: m.u8(x.b, 7) }
    }
}

/// Master field (16-bit RAM address).
const MASTER: u32 = 0xb2;

fn play<R: Bench>(x: &mut Cx<'_, R>, play: Play) {
    match play {
        Anim(set) => lib::animation_of_master(x, set),
        Hit(set) => lib::animation_of_mark(x, set),
        Restart(n) => restart_or_step(x, n),
        RestartSpawn(n) => {
            restart_or_step_spawn(x, n);
        }
        AnimClear(set) => {
            lib::animation_of_master(x, set);
            x.set8(0x38, 0);
        }
        Step => {
            lib::step(x);
        }
        Nothing => {}
    }
}

/// Animation set of the Chimera's gestures.
const GESTURES: u32 = 0x16abd6;

fn restart_or_step<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    if x.u8(0xb6) != 0 {
        x.set8(0xb6, 0);
        lib::start(x, GESTURES, n);
    } else {
        lib::step(x);
    }
}

fn restart_or_step_spawn<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    if x.u8(0xb6) != 0 {
        x.set8(0xb6, 0);
        return lib::start(x, GESTURES, n);
    }
    lib::step(x);
    if x.u8(0x33) == 0 {
        return;
    }
    x.set8(0x33, 0);
    // the original retries as long as the shot pool is full
    let shot = loop {
        if let Some(o) = x.alloc(&pools::ESHL) {
            break o;
        }
    };
    shot.set_u8(x.b, 0, 1);
    shot.set_u8(x.b, 2, 0x31);
    let master = x.u16(MASTER);
    shot.set_u16(x.b, f::CREATOR, master);
}

/// HP loss of a double that follows its own life (types 22 and 23): the freeze (`+0x58`) drops after 20
/// frames; sound; at 0 HP the double becomes "wounded" (`+0xaf`), the game records `dying_code` in `+0x7e`
/// and creates an EFFCT 0x3f effect of variant `effect_variant`.
fn hurt<R: Bench>(x: &mut Cx<'_, R>, dying_code: u8, effect_variant: u8) {
    let wait = x.u16(0xb4) as i16;
    if wait > 0 {
        x.set16(0xb4, (wait - 1) as u16);
        if wait - 1 <= 0 {
            x.set32(0x58, 0);
        }
    }
    let hp = x.u16(f::HP) as i16;
    if hp >= x.u16(0x64) as i16 {
        return;
    }
    x.set16(0x64, hp as u16);
    x.set16(0xb4, 0x14);
    if hp > 0 {
        return lib::sound(x, 0xe40b4);
    }
    x.set8(0xaf, 1);
    lib::sound(x, 0xe4188);
    x.set8(0x7e, dying_code);
    lib::object_event(x);
    let Some(e) = x.alloc(&pools::EFFCT) else { return };
    e.set_u8(x.b, 0, 1);
    e.set_u8(x.b, 2, 0x3f);
    e.set_u8(x.b, 3, effect_variant);
    e.set_u16(x.b, f::CREATOR, x.o.0 as u16);
}

/// Common ending: copies the master's state (activity, flags, position offset by `dy` in
/// depth, orientation, owner) and re-lists the object, unless it was just released.
fn mirror<R: Bench>(x: &mut Cx<'_, R>, dy: u32) {
    let master = x.linked(MASTER);
    if master.u8(x.b, 0xad) != 0 {
        x.set8(0x38, 0);
    }
    if x.u8(0) == 0 {
        return;
    }
    let v = master.u8(x.b, 0);
    x.set8(0, v);
    let v = master.u8(x.b, 1);
    x.set8(1, v);
    let v = master.u16(x.b, 0x8);
    x.set16(0x8, v);
    let v = master.u16(x.b, 0xc);
    x.set16(0xc, v);
    let v = master.u32(x.b, 0x10).wrapping_add(dy);
    x.set32(0x10, v);
    let v = master.u16(x.b, 0x2c);
    x.set16(0x2c, v);
    let v = master.u16(x.b, f::OWNER);
    x.set16(f::OWNER, v);
    x.reinsert();
}

// ---------------------------------------------------------------------------------------------
// Type 24 (0x76afc): "Manticore" (h)
// ---------------------------------------------------------------------------------------------

/// Graphical double of an enemy of type 25. It sits one pixel lower than its master (depth
/// + 1) and picks its animation with `choose_active` (master in state 0 or 2, outside pause) or
/// `choose_hit` (master hit).
mod manticore {
    use super::*;

    /// Neutral set (rest) played when the master is in a state with no gesture of its own.
    const REST: u32 = 0x16c2da;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        if suspended(x) {
            return;
        }
        let m = Master::read(x);
        let choice = match m.state {
            0 | 2 if !paused(x) => choose_active(m.sub, m.s6, m.s7 as i8),
            4 => choose_hit(m.sub, m.s6 as i8),
            _ => Nothing,
        };
        play(x, choice);
        if m.state == st::DEAD {
            free(x);
        }
        mirror(x, 1);
    }

    /// Active master: animation from its substate `sub`, `s6` (sub-substate) and `s7` (current
    /// gesture, signed: <= 0 = nothing). The `step`s are the indices of the original's small tables
    /// (`s7 - 2`).
    fn choose_active(sub: u8, s6: u8, s7: i8) -> Play {
        let step = (s7 as u8).wrapping_sub(2);
        let gesture = s7 > 0;
        match sub {
            0 => match s6 {
                2 => Anim(0x16c59a),
                4 => Anim(REST),
                _ => Nothing,
            },
            2 => match s6 {
                0 if s7 != 0 => Anim(REST),
                2 if s7 != 0 => Anim(0x16c33a),
                4 | 6 if s7 != 0 => Anim(0x16c39a),
                8 if gesture => match step {
                    0 => Anim(0x16c43a),
                    2 => Anim(0x16c48a),
                    _ => Nothing,
                },
                10 if gesture => match step {
                    0 | 2 | 4 => Anim(0x16c49a),
                    6 => Anim(0x16c4ba),
                    _ => Nothing,
                },
                _ => Nothing,
            },
            4 if gesture => match (s6, step) {
                (0, 0) => Anim(0x16c52a),
                (2, 0) => Anim(0x16c59a),
                (2, 4) => Anim(0x16c5fa),
                (2, 6) => Anim(0x16c60a),
                (4, 0) => Anim(0x16c59a),
                (4, 4) => Anim(0x16c66a),
                (4, 6) => Anim(0x16c67a),
                (6, 0) => Anim(0x16c69a),
                (6, 2) => Anim(0x16c6ba),
                (6, 4) => Anim(0x16c6ca),
                (6, 6) => Anim(0x16c6da),
                (8, 0 | 2 | 4 | 6 | 8) => Anim(0x16c6fa),
                (8, 10) => Anim(REST),
                (10, 0 | 2 | 4 | 6) | (12, 0 | 2 | 4 | 6 | 8) => Anim(REST),
                _ => Nothing,
            },
            6 | 8 | 10 | 12 if s6 != 0 => Anim(REST),
            _ => Nothing,
        }
    }

    /// Hit master (state 4): hit animation (`s6` = gesture, signed).
    fn choose_hit(sub: u8, s6: i8) -> Play {
        match sub {
            0 | 4 | 6 | 8 if s6 > 0 => Hit(0x16c75a),
            2 if s6 > 0 && matches!((s6 as u8).wrapping_sub(2), 0 | 2) => Anim(0x16c51a),
            _ => Nothing,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 23 (0x76592): "Spectre" (h)
// ---------------------------------------------------------------------------------------------

/// Graphical double of an enemy, with two animation sets (normal and "wounded" when `+0xaf` is
/// non-zero) and its own life tracking: when its HP (`+0x62`) drop below the known minimum
/// (`+0x64`), it plays a sound and, at 0 HP, raises `+0xaf`, dies with a sound (`0xe4188`) and creates an effect
/// (EFFCT 0x3f). Position: one pixel higher than the master (depth - 1).
mod spectre {
    use super::*;
    /// Rest set (normal, wounded).
    const REST: (u32, u32) = (0x16b7c2, 0x16b822);

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        if suspended(x) {
            return;
        }
        if x.u8(0xaf) == 0 {
            hurt(x, 0x25, 1);
        }
        let m = Master::read(x);
        let injured = x.u8(0xaf) != 0;
        let choice = match m.state {
            0 | 2 if !paused(x) => choose_active(m.sub, m.s6, m.s7 as i8, injured),
            4 => choose_hit(m.sub, m.s6 as i8, injured),
            _ => Nothing,
        };
        play(x, choice);
        if m.state == st::DEAD {
            free(x);
        }
        mirror(x, 0xffff_ffff);
    }

    /// Set (normal, wounded) according to the double's state.
    fn pick(injured: bool, normal: u32, hurt: u32) -> Play {
        Anim(if injured { hurt } else { normal })
    }

    fn choose_active(sub: u8, s6: u8, s7: i8, injured: bool) -> Play {
        let step = (s7 as u8).wrapping_sub(2);
        let gesture = s7 > 0;
        let rest = || pick(injured, REST.0, REST.1);
        match sub {
            0 => match s6 {
                2 => pick(injured, 0x16bdea, 0x16be4a),
                4 => rest(),
                _ => Nothing,
            },
            2 => match s6 {
                0 if s7 != 0 => rest(),
                2 if s7 != 0 => pick(injured, 0x16b882, 0x16b8e2),
                4 | 6 if s7 != 0 => pick(injured, 0x16b942, 0x16b9e2),
                8 if gesture => match step {
                    0 => pick(injured, 0x16ba82, 0x16bad2),
                    2 => pick(injured, 0x16bb22, 0x16bb32),
                    _ => Nothing,
                },
                10 if gesture => match step {
                    0 | 2 | 4 => pick(injured, 0x16bb42, 0x16bb62),
                    6 => pick(injured, 0x16bb82, 0x16bb92),
                    _ => Nothing,
                },
                _ => Nothing,
            },
            4 if gesture => match (s6, step) {
                (0, 0) => pick(injured, 0x16bd0a, 0x16bd7a),
                (2, 0) => pick(injured, 0x16bdea, 0x16be4a),
                (2, 4) => pick(injured, 0x16beaa, 0x16beba),
                (2, 6) => pick(injured, 0x16beca, 0x16bf2a),
                (4, 0) => pick(injured, 0x16bdea, 0x16be4a),
                (4, 4) => pick(injured, 0x16bf8a, 0x16bf9a),
                (4, 6) => pick(injured, 0x16bfaa, 0x16bfca),
                (6, 0) => pick(injured, 0x16bfea, 0x16c00a),
                (6, 2) => pick(injured, 0x16c02a, 0x16c03a),
                (6, 4) => pick(injured, 0x16c04a, 0x16c05a),
                (6, 6) => pick(injured, 0x16c06a, 0x16c08a),
                (8, 0 | 2 | 4 | 6 | 8) => pick(injured, 0x16c0aa, 0x16c10a),
                (8, 10) => rest(),
                (10, 0 | 2 | 4 | 6) | (12, 0 | 2 | 4 | 6 | 8) => rest(),
                _ => Nothing,
            },
            6 | 8 | 10 | 12 if s6 != 0 => rest(),
            _ => Nothing,
        }
    }

    fn choose_hit(sub: u8, s6: i8, injured: bool) -> Play {
        match sub {
            0 | 4 | 6 | 8 if s6 > 0 => Hit(if injured { 0x16c20a } else { 0x16c16a }),
            2 if s6 > 0 && matches!((s6 as u8).wrapping_sub(2), 0 | 2) => pick(injured, 0x16bc42, 0x16bc52),
            _ => Nothing,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 22 (0x75f42): "Chimera" (h)
// ---------------------------------------------------------------------------------------------

/// Graphical double with the same frame as the Spectre (two animation sets, own life, one pixel
/// higher than the master), with in addition gestures whose animation is restarted on demand
/// (`+0xb6`, set 0x16abd6) and an ESHL 0x31 shot (breath) emitted at the end of a master gesture.
mod chimera {
    use super::*;

    /// Rest set (normal, wounded).
    const REST: (u32, u32) = (0x16ac0c, 0x16ac6c);

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        if suspended(x) {
            return;
        }
        if x.u8(0xaf) == 0 {
            hurt(x, 0x21, 0);
        }
        let m = Master::read(x);
        let injured = x.u8(0xaf) != 0;
        let choice = match m.state {
            0 | 2 if !paused(x) => choose_active(m.sub, m.s6, m.s7 as i8, injured),
            4 => choose_hit(m.sub, m.s6 as i8, injured),
            _ => Nothing,
        };
        play(x, choice);
        if m.state == st::DEAD {
            free(x);
        }
        mirror(x, 1);
    }

    fn pick(injured: bool, normal: u32, hurt: u32) -> Play {
        Anim(if injured { hurt } else { normal })
    }

    fn choose_active(sub: u8, s6: u8, s7: i8, injured: bool) -> Play {
        let step = (s7 as u8).wrapping_sub(2);
        let gesture = s7 > 0;
        let rest = || pick(injured, REST.0, REST.1);
        match sub {
            0 => match s6 {
                2 => pick(injured, 0x16b214, 0x16b274),
                4 => rest(),
                _ => Nothing,
            },
            2 => match s6 {
                0 if s7 != 0 => rest(),
                2 if s7 != 0 => pick(injured, 0x16accc, 0x16ad2c),
                4 | 6 if s7 != 0 => pick(injured, 0x16ad8c, 0x16ae2c),
                8 if gesture => match step {
                    0 => pick(injured, 0x16aecc, 0x16af1c),
                    2 => pick(injured, 0x16af6c, 0x16af7c),
                    _ => Nothing,
                },
                10 if gesture => match step {
                    0 | 2 | 4 => pick(injured, 0x16af8c, 0x16afac),
                    6 => pick(injured, 0x16afcc, 0x16afdc),
                    _ => Nothing,
                },
                _ => Nothing,
            },
            4 if gesture => match (s6, step) {
                (0, 0) => pick(injured, 0x16b134, 0x16b1a4),
                (2 | 4, 0) => pick(injured, 0x16b214, 0x16b274),
                (2, 4) => pick(injured, 0x16b2d4, 0x16b2e4),
                (2, 6) => pick(injured, 0x16b2f4, 0x16b354),
                (4, 4) => pick(injured, 0x16b3b4, 0x16b3c4),
                (4, 6) => pick(injured, 0x16b3d4, 0x16b3f4),
                (6, 0) => pick(injured, 0x16b414, 0x16b434),
                (6, 2) => pick(injured, 0x16b454, 0x16b464),
                (6, 4) => pick(injured, 0x16b474, 0x16b484),
                (6, 6) => pick(injured, 0x16b494, 0x16b4b4),
                (8, 0 | 2 | 6 | 8) => pick(injured, 0x16b214, 0x16b274),
                (8, 4) => Restart(0xb),
                (8, 10) => rest(),
                (10, 0) => AnimClear(0x16ac0c),
                (10, 2) => RestartSpawn(0xc),
                (10, 4) => Restart(0xd),
                (10, 6) => rest(),
                (12, 0) => AnimClear(0x16ac0c),
                (12, 2) => Restart(0xc),
                (12, 4) => Step,
                (12, 6) => Restart(0xd),
                (12, 8) => rest(),
                _ => Nothing,
            },
            6 | 8 | 10 | 12 if s6 != 0 => rest(),
            _ => Nothing,
        }
    }

    fn choose_hit(sub: u8, s6: i8, injured: bool) -> Play {
        match sub {
            0 | 4 | 6 | 8 if s6 > 0 => Hit(if injured { 0x16b6c0 } else { 0x16b620 }),
            2 if s6 > 0 && matches!((s6 as u8).wrapping_sub(2), 0 | 2) => pick(injured, 0x16b08c, 0x16b09c),
            _ => Nothing,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 26 (0x77736): "DarkWarrior 2" (h)
// ---------------------------------------------------------------------------------------------

/// Double of an enemy of type 25 (word `+0xb0`): instead of drawing an animation of its own, it replays
/// the master's movement. The four bytes `+0xb8` to `+0xbb` of the master (group, mode, phase, step)
/// say how to copy its speeds: identically, inverted, or by replaying the movement
/// (`0x1d90`: X, Y, Z; `0x1da6`: X, Z; `0x1d78`: X) so that the double stays in sync with the
/// obstacles. When the master is hit (`+0xb8 = 4`), it also replays the hit reaction with
/// the common routines. When the master or the double dies, it creates an effect (EFFCT 0x32).
mod dark_warrior_double {
    use super::*;

    const MASTER: u32 = 0xb0;
    /// Group, mode, phase and step of the master.
    const GROUP: u32 = 0xb8;
    const MODE: u32 = 0xb9;
    const PHASE: u32 = 0xba;
    const STEP: u32 = 0xbb;
    /// Table of hit reaction addresses (master's code `+0x49`).
    const REACTIONS: u32 = 0x77aec;
    /// Terrain under the object that reverses the depth speed (bytes per terrain 0..8).
    const REVERSING_TERRAIN: u32 = 0x77d08;
    const ATTACKED: u16 = 0x400;
    const MASTER_ATTACKED: u16 = 0x406;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        if suspended(x) {
            return;
        }
        let m = x.linked(MASTER);
        if x.u16(f::STATE) == ATTACKED || m.u16(x.b, f::STATE) == MASTER_ATTACKED {
            return vanish(x);
        }
        match m.u8(x.b, GROUP) {
            0 | 2 => follow(x),
            4 => struck(x),
            6 => free(x),
            _ => {}
        }
        if x.u8(0) == 0 {
            return;
        }
        let m = x.linked(MASTER);
        let v = m.u8(x.b, 0);
        x.set8(0, v);
        let v = m.u8(x.b, f::FLIP);
        x.set8(f::FLIP, v);
        let v = m.u16(x.b, 0x54);
        x.set16(0x54, v);
        x.set8(0x3f, 4);
        x.reinsert();
    }

    /// The master forgets its double, which creates an effect then disappears.
    fn vanish<R: Bench>(x: &mut Cx<'_, R>) {
        let m = x.linked(MASTER);
        m.set_u16(x.b, MASTER, 0);
        spawn_effect(x);
        free(x);
    }

    /// 0x77c8a: EFFCT 0x32 effect in place of the double (one pixel higher).
    fn spawn_effect<R: Bench>(x: &mut Cx<'_, R>) {
        let Some(e) = x.alloc(&pools::EFFCT) else { return };
        e.set_u8(x.b, 0, 1);
        e.set_u8(x.b, 2, 0x32);
        e.set_u8(x.b, 3, 7);
        e.set_u16(x.b, f::CREATOR, x.o.0 as u16);
        for off in [0x8, 0xc, 0x10] {
            let v = x.u16(off);
            e.set_u16(x.b, off, v);
        }
        x.add16(0x10, 1);
        let v = x.u8(f::FLIP);
        e.set_u8(x.b, f::FLIP, v);
        let v = x.u32(0x34);
        e.set_u32(x.b, 0x34, v);
        let v = x.u8(0x3c);
        e.set_u8(x.b, 0x3c, v);
        let v = x.u8(0x3f);
        e.set_u8(x.b, 0x3f, v);
        e.set_u16(x.b, 0x30, 0x28);
    }

    // --- copies from the master ----------------------------------------------------------------------

    /// 0x77cdc: copy of the animation frame (`+0x34` long, `+0x38`) from the master.
    fn copy_frame<R: Bench>(x: &mut Cx<'_, R>) {
        let m = x.linked(MASTER);
        let v = m.u32(x.b, 0x34);
        x.set32(0x34, v);
        let v = m.u8(x.b, 0x38);
        x.set8(0x38, v);
    }

    /// Master's height (`+0xc`, word), then its frame.
    fn copy_height<R: Bench>(x: &mut Cx<'_, R>) {
        let m = x.linked(MASTER);
        let v = m.u16(x.b, 0xc);
        x.set16(0xc, v);
    }

    fn copy_height_frame<R: Bench>(x: &mut Cx<'_, R>) {
        copy_height(x);
        copy_frame(x);
    }

    /// The master's three speeds (X, Z, Y: longs at `+0x14`, `+0x18`, `+0x1c`).
    fn copy_speeds<R: Bench>(x: &mut Cx<'_, R>) {
        let m = x.linked(MASTER);
        for off in [0x14, 0x18, 0x1c] {
            let v = m.u32(x.b, off);
            x.set32(off, v);
        }
    }

    /// Master's speeds inverted in X and Y (Z kept), height and frame (0x777fa, 0x77a34).
    fn mirror_speeds<R: Bench>(x: &mut Cx<'_, R>) {
        let m = x.linked(MASTER);
        let z = m.u32(x.b, 0x18);
        x.set32(0x18, z);
        let vx = m.u32(x.b, 0x14).wrapping_neg();
        let vy = m.u32(x.b, 0x1c).wrapping_neg();
        x.set32(0x14, vx);
        x.set32(0x1c, vy);
        copy_height_frame(x);
    }

    /// The master has not finished its animation step (`+0x33` zero): the double replays the movement.
    fn replay<'a, R: Bench>(x: &mut Cx<'a, R>, mover: fn(&mut Cx<'a, R>)) {
        let m = x.linked(MASTER);
        if m.u8(x.b, 0x33) == 0 {
            mover(x);
        }
    }

    // --- active master (+0xb8 = 0 or 2) -----------------------------------------------------------

    fn follow<R: Bench>(x: &mut Cx<'_, R>) {
        if !paused(x) {
            let m = x.linked(MASTER);
            match m.u8(x.b, MODE) {
                0 => mode0(x),
                2 => mode2(x),
                4 => mode4(x),
                _ => copy_frame(x),
            }
        }
        lib::keep_in_zone(x);
        lib::collide_terrain(x);
    }

    /// The master walks: speeds inverted, or movement replayed.
    fn mode0<R: Bench>(x: &mut Cx<'_, R>) {
        let m = x.linked(MASTER);
        match m.u8(x.b, PHASE) {
            0 | 2 | 4 | 6 => mirror_speeds(x),
            8 => {
                replay(x, |x| { x.move_xyz(); });
                copy_height_frame(x);
            }
            10 | 12 => {
                x.move_xyz();
                copy_height_frame(x);
            }
            _ => copy_height_frame(x),
        }
    }

    /// 0x77842: the master is flying or jumping; copy or replay depending on the phase.
    fn mode2<R: Bench>(x: &mut Cx<'_, R>) {
        let m = x.linked(MASTER);
        match m.u8(x.b, PHASE) {
            4 | 6 => {
                let step = m.u8(x.b, STEP);
                match step {
                    0 => {
                        copy_speeds(x);
                        copy_frame(x);
                    }
                    2 => {
                        if m.u8(x.b, 0x33) == 0 {
                            x.move_xyz();
                            copy_height(x);
                            if terrain_reverses(x) {
                                let v = (x.u32(0x1c) as i32).wrapping_neg();
                                x.set32(0x1c, v as u32);
                            }
                        }
                        copy_frame(x);
                    }
                    4 | 6 => {
                        x.move_xyz();
                        copy_height(x);
                        if terrain_reverses(x) {
                            let v = (x.u32(0x1c) as i32).wrapping_neg();
                            x.set32(0x1c, v as u32);
                        }
                        copy_frame(x);
                    }
                    _ => copy_frame(x),
                }
            }
            8 => {
                let step = m.u8(x.b, STEP);
                match step {
                    0 => copy_speeds(x),
                    2 => {
                        if m.u8(x.b, 0x33) == 0 {
                            x.move_xyz();
                            copy_height(x);
                        }
                    }
                    4 | 6 => {
                        x.move_xyz();
                        copy_height(x);
                    }
                    _ => {}
                }
                copy_frame(x);
            }
            10 => {
                let step = m.u8(x.b, STEP);
                match step {
                    2 => {
                        if m.u8(x.b, 0x33) != 0 {
                            copy_speeds(x);
                            x.move_xz();
                            copy_height(x);
                        }
                    }
                    4 => {
                        x.move_xz();
                        copy_height(x);
                        let v = m.u16(x.b, 0x16);
                        x.set16(0x16, v);
                    }
                    6 => {
                        x.move_xz();
                        let h = m.u16(x.b, 0xc);
                        x.set16(0xc, h);
                        if (h as i16) <= 0 {
                            let v = m.u32(x.b, 0x14);
                            x.set32(0x14, v);
                        }
                    }
                    8 => {
                        x.move_x();
                        mode4(x);
                        return;
                    }
                    _ => {}
                }
                copy_frame(x);
            }
            _ => copy_frame(x),
        }
    }

    /// 0x779da: the master is in a special state; only a few phases move the double.
    fn mode4<R: Bench>(x: &mut Cx<'_, R>) {
        let m = x.linked(MASTER);
        match m.u8(x.b, PHASE) {
            10 => match m.u8(x.b, STEP) {
                0 => mirror_speeds(x),
                2 => {
                    replay(x, |x| { x.move_xyz(); });
                    copy_height_frame(x);
                }
                4 | 6 => {
                    x.move_xyz();
                    copy_height_frame(x);
                }
                _ => copy_height_frame(x),
            },
            _ => copy_frame(x),
        }
    }

    /// 0x77cee: does the terrain under the double reverse its depth speed?
    fn terrain_reverses<R: Bench>(x: &mut Cx<'_, R>) -> bool {
        let t = crate::video::terrain_of(x.b, x.o);
        let terrain = t.number & 0x3f;
        *x.c.d2() = *x.c.d2() & !0xffff | terrain as u32;
        if terrain as u8 >= 9 {
            return false;
        }
        let b = x.rom8(REVERSING_TERRAIN + terrain as u32);
        b != 0
    }

    // --- hit master (+0xb8 = 4) ---------------------------------------------------------------

    fn struck<R: Bench>(x: &mut Cx<'_, R>) {
        x.set16(f::HP, 0x7fff);
        let m = x.linked(MASTER);
        match m.u8(x.b, MODE) {
            0 => replay_reaction(x),
            2 => match m.u8(x.b, PHASE) {
                0 => {
                    let v = m.u32(x.b, 0x14);
                    x.set32(0x14, v);
                    x.set16(0xc, 0);
                    copy_frame(x);
                }
                2 | 4 => {
                    let v = m.u16(x.b, 0x16);
                    x.set16(0x16, v);
                    x.move_x();
                    copy_frame(x);
                }
                _ => {}
            },
            4 => match m.u8(x.b, PHASE) {
                0 | 2 | 8 => copy_frame(x),
                4 => {
                    for off in [0x14, 0x18, 0x1c] {
                        let v = m.u16(x.b, off);
                        x.set16(off, v);
                    }
                    copy_frame(x);
                }
                6 => {
                    x.move_xyz();
                    copy_height_frame(x);
                }
                _ => {}
            },
            6 | 8 => match m.u8(x.b, PHASE) {
                0 => {
                    copy_speeds(x);
                    copy_frame(x);
                }
                2 => {
                    x.move_xz();
                    copy_height_frame(x);
                }
                4 | 6 => copy_frame(x),
                _ => {}
            },
            10 => copy_frame(x),
            _ => {}
        }
        lib::keep_in_zone(x);
        lib::collide_terrain(x);
    }

    /// 0x77ab6: the double takes the master's reaction state and replays its reaction routine.
    fn replay_reaction<R: Bench>(x: &mut Cx<'_, R>) {
        let m = x.linked(MASTER);
        let sub = m.u16(x.b, PHASE);
        x.set16(f::SUB2, sub);
        copy_speeds(x);
        copy_height(x);
        let code = m.u8(x.b, 0x49) as u32;
        let routine = rom32(x, REACTIONS + code);
        run_reaction(x, routine);
        x.set8(0x3b, 0);
    }
}

handlers! {
    0x77736 "enemy_26" #[without] dark_warrior_double::run,
    0x75f42 "enemy_22" #[without] chimera::run,
    0x76592 "enemy_23" #[without] spectre::run,
    0x76afc "enemy_24" #[without] manticore::run,
}
