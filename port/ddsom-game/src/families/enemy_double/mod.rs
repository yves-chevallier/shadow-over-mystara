//! Graphical doubles of enemies (ENEMY types 22, 23 and 24). See `docs/families/enemy.md`.
//!
//! A double is an enemy with no logic of its own: it copies its master (word `+0xb2`, an enemy of
//! type 25 or 26 that creates it in pairs) and plays the animation that the master's state commands. Two
//! sets of decision tables, with the same frame: the master's state (`+4`) leads to its substate
//! (`+5`), then to `+6` and the current gesture `+7`, which designate a ROM animation set.

use crate::families::cx::Bench;
use crate::data::RomAddr;
use super::cx::{handlers, Cx};
use super::enemy_common::*;
use crate::record::record;
use crate::objects::Obj;

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
        Master { state: m.u8(x.b, Enemy::STATE), sub: m.u8(x.b, Enemy::SUB), s6: m.u8(x.b, Enemy::STEP), s7: m.u8(x.b, Enemy::SUB_STEP) }
    }
}

/// Fields of the doubles beyond the common ones of `Enemy` (`record!`), and the fields of their master (an enemy of
/// type 25 or 26) that they read. A meaning guessed from the code alone is marked (h).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Double(pub Obj);

impl Double {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Double {
        /// On the master: non-zero while its doubles hide their frame tag (h).
        master_blank, set_master_blank, MASTER_BLANK: u8 @ 0xad;
        /// Types 22 and 23: set when the double has no HP left (it plays its "wounded" set).
        wounded, set_wounded, WOUNDED: u8 @ 0xaf;
        /// Type 26: its master (word, RAM address); on the master, its double.
        replay_master, set_replay_master, REPLAY_MASTER: u16 @ 0xb0;
        /// Types 22 to 24: the master (word, RAM address).
        master, set_master, MASTER: u16 @ 0xb2;
        /// Types 22 and 23: frames before the hit lock drops (word).
        hurt_wait, set_hurt_wait, HURT_WAIT: u16 @ 0xb4;
        /// Non-zero: the next gesture restarts its animation (lowered when it does).
        restart, set_restart, RESTART: u8 @ 0xb6;
        /// On the master of a type 26: group, mode, phase and step of its move, which the double replays.
        master_group, set_master_group, MASTER_GROUP: u8 @ 0xb8;
        master_mode, set_master_mode, MASTER_MODE: u8 @ 0xb9;
        master_phase, set_master_phase, MASTER_PHASE: u8 @ 0xba;
        master_step, set_master_step, MASTER_STEP: u8 @ 0xbb;
    }
}

/// Master field (16-bit RAM address).
const MASTER: u32 = Double::MASTER;

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
            x.set8(Enemy::FRAME_TAG, 0);
        }
        Step => {
            lib::step(x);
        }
        Nothing => {}
    }
}

/// Animation set of the Chimera's gestures.
const GESTURES: RomAddr = RomAddr(0x16abd6);

fn restart_or_step<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    if x.u8(Double::RESTART) != 0 {
        x.set8(Double::RESTART, 0);
        lib::start(x, GESTURES.0, n);
    } else {
        lib::step(x);
    }
}

fn restart_or_step_spawn<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    if x.u8(Double::RESTART) != 0 {
        x.set8(Double::RESTART, 0);
        return lib::start(x, GESTURES.0, n);
    }
    lib::step(x);
    if x.u8(Enemy::KEY_FRAME) == 0 {
        return;
    }
    x.set8(Enemy::KEY_FRAME, 0);
    // the original retries as long as the shot pool is full
    let shot = loop {
        if let Some(o) = x.alloc(&pools::ESHL) {
            break o;
        }
    };
    shot.set_u8(x.b, Enemy::ACTIVE, 1);
    shot.set_u8(x.b, Enemy::KIND, 0x31);
    let master = x.u16(MASTER);
    shot.set_u16(x.b, f::CREATOR, master);
}

/// HP loss of a double that follows its own life (types 22 and 23): the freeze (`+0x58`) drops after 20
/// frames; sound; at 0 HP the double becomes "wounded" (`+0xaf`), the game records `dying_code` in `+0x7e`
/// and creates an EFFCT 0x3f effect of variant `effect_variant`.
fn hurt<R: Bench>(x: &mut Cx<'_, R>, dying_code: u8, effect_variant: u8) {
    let wait = x.u16(Double::HURT_WAIT) as i16;
    if wait > 0 {
        x.set16(Double::HURT_WAIT, (wait - 1) as u16);
        if wait - 1 <= 0 {
            x.set32(Enemy::HIT_LOCK, 0);
        }
    }
    let hp = x.u16(f::HP) as i16;
    if hp >= x.u16(Enemy::HP_LAST) as i16 {
        return;
    }
    x.set16(Enemy::HP_LAST, hp as u16);
    x.set16(Double::HURT_WAIT, 0x14);
    if hp > 0 {
        return lib::sound(x, crate::sound::CMD_359);
    }
    x.set8(Double::WOUNDED, 1);
    lib::sound(x, crate::sound::CMD_35A);
    x.set8(Enemy::EVENT_KIND, dying_code);
    lib::object_event(x);
    let Some(e) = x.alloc(&pools::EFFCT) else { return };
    e.set_u8(x.b, Enemy::ACTIVE, 1);
    e.set_u8(x.b, Enemy::KIND, 0x3f);
    e.set_u8(x.b, Enemy::VARIANT, effect_variant);
    e.set_u16(x.b, f::CREATOR, x.o.0 as u16);
}

/// Common ending: copies the master's state (activity, flags, position offset by `dy` in
/// depth, orientation, owner) and re-lists the object, unless it was just released.
fn mirror<R: Bench>(x: &mut Cx<'_, R>, dy: u32) {
    let master = x.linked(MASTER);
    if master.u8(x.b, Double::MASTER_BLANK) != 0 {
        x.set8(Enemy::FRAME_TAG, 0);
    }
    if x.u8(Enemy::ACTIVE) == 0 {
        return;
    }
    let v = master.u8(x.b, Enemy::ACTIVE);
    x.set8(Enemy::ACTIVE, v);
    let v = master.u8(x.b, Enemy::DISPLAY);
    x.set8(Enemy::DISPLAY, v);
    let v = master.u16(x.b, Obj::X);
    x.set16(Obj::X, v);
    let v = master.u16(x.b, Obj::Z);
    x.set16(Obj::Z, v);
    let v = master.u32(x.b, Obj::Y).wrapping_add(dy);
    x.set32(Obj::Y, v);
    // orientation and heading
    let v = master.u16(x.b, Enemy::FLIP);
    x.set16(Enemy::FLIP, v);
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
