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

mod beasts;
mod dark_warrior;
use beasts::{chimera, manticore, spectre};
use dark_warrior::dark_warrior_double;

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
        return lib::sound(x, crate::sound::CMD_359);
    }
    x.set8(0xaf, 1);
    lib::sound(x, crate::sound::CMD_35A);
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

handlers! {
    0x77736 "enemy_26" dark_warrior_double::run,
    0x75f42 "enemy_22" chimera::run,
    0x76592 "enemy_23" spectre::run,
    0x76afc "enemy_24" manticore::run,
}
