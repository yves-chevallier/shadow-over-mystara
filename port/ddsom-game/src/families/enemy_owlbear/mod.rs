//! "Owlbear" enemy (ENEMY type 4, 0x5d87a). See `docs/families/enemy.md`.
//!
//! Same framework as the Riyakaa (`enemy_riyakaa`): `+4` main state (0 creation, 2 active, 4 hit, 6 release), `+5`
//! group of behaviours, `+6` and `+7` steps. In the active state:
//!
//! | `+5` | File | Role (h) |
//! |---|---|---|
//! | 0 | `stance` | arrival according to variant `+3` |
//! | 2 | `stance` | standing: pursuit, turn-around, wait, approach |
//! | 4 | `attacks` | attacks (`+6` = the attack) |
//! | 6 | `attacks` | post-arrival attacks: leap, charge |
//! | 8 | `moves` | charges and jumps toward a player |
//! | 10 | `stance` | rest, waits for a player to be alive |
//! | state 4 | `hit` | reactions to hits |
//!
//! Animations: 0x160e26 (movements) and 0x160e44 (attacks). `+0xa1` is the wanted animation, `+0xa2` the current one
//! (the same one is not restarted).

use crate::families::cx::Bench;
use super::cx::{handlers, Cx};
use super::enemy_common::*;
use crate::objects::Obj;

mod attacks;
mod hit;
mod moves;
mod stance;

/// Fields specific to this type (the common fields are in `enemy_common::f`).
pub(self) mod g {
    /// Damage of the current attack: `+0x44` type, `+0x45` strength.
    pub const STRIKE_KIND: u32 = 0x44;
    pub const STRIKE_POWER: u32 = 0x45;
    /// Vulnerable (1) or not (0) during the move.
    pub const VULNERABLE: u32 = 0x67;
    /// AI parameters set at creation.
    pub const AI_STEP: u32 = 0x6b;
    pub const AI_REPEAT: u32 = 0x6e;
    pub const AI_TURN: u32 = 0x71;
    pub const AI_KICK: u32 = 0x77;
    pub const AI_SHOT: u32 = 0x79;
    /// Delay before the first attack (high byte of word `+0xaa`).
    pub const DELAY: u32 = 0xaa;
    /// Timer of the current step.
    pub const TIMER: u32 = 0xa0;
    /// Wanted animation and current animation.
    pub const WANT_ANIM: u32 = 0xa1;
    pub const CUR_ANIM: u32 = 0xa2;
    /// Long state (`+0xa6`) and step (`+0xa4`) to resume after a turn-around.
    pub const RESUME_STATE: u32 = 0xa6;
    pub const RESUME_STEP: u32 = 0xa4;
    /// Non-zero byte: the hit state does not test the play zone.
    pub const NO_ZONE: u32 = 0xa5;
    /// Repetitions of an attack: `+0xac` (series), `+0xad` (same animation).
    pub const SERIES: u32 = 0xac;
    pub const REPEAT_LEFT: u32 = 0xad;
}

use g::*;
use crate::objects::Current;

/// Animation sets: movements and attacks.
const SET_BODY: u32 = 0x160e26;
const SET_STRIKE: u32 = 0x160e44;
/// Long states (`+4..+7`).
const ACTIVE_STANCE: u32 = 0x0202_0000;
const ACTIVE_IDLE: u32 = 0x020a_0000;
const ACTIVE_TURN: u32 = 0x0202_0200;
const ACTIVE_ENGAGE: u32 = 0x0202_0002;
const KNOCKED: u32 = 0x0408_0000;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    x.tick_counters();
    match x.u8(f::STATE) {
        st::INIT => init(x),
        st::ACTIVE => active(x),
        st::HIT => hit::run(x),
        st::DEAD => free(x),
        _ => {}
    }
}

// --- aides ------------------------------------------------------------------------------------

/// Sets the wanted animation `want`; starts script `n` of set `set` if it is not already the current one.
fn pose<R: Bench>(x: &mut Cx<'_, R>, want: u8, set: u32, n: u32) {
    x.set8(WANT_ANIM, want);
    if want != x.u8(CUR_ANIM) {
        x.set8(CUR_ANIM, want);
        x.anim_start(set, n as u16);
    }
}

/// What an attack must do when it wants to replay its animation (`+0xa1`).
enum Replay {
    New,
    Again,
    End,
}

/// Like `pose`, for a repeated attack: if the animation is already running, one repetition fewer (`+0xad`), and at zero
/// the beast gets back on its feet.
fn replay<R: Bench>(x: &mut Cx<'_, R>, want: u8) -> Replay {
    x.set8(WANT_ANIM, want);
    if want != x.u8(CUR_ANIM) {
        x.set8(CUR_ANIM, want);
        return Replay::New;
    }
    let left = x.u8(REPEAT_LEFT).wrapping_sub(1);
    x.set8(REPEAT_LEFT, left);
    if left == 0 {
        set_state_long(x, ACTIVE_STANCE);
        return Replay::End;
    }
    Replay::Again
}

fn body<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.anim_start(SET_BODY, n as u16);
}

fn strike<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.anim_start(SET_STRIKE, n as u16);
}

fn step<R: Bench>(x: &mut Cx<'_, R>) {
    x.anim_step();
}

fn move_xy<R: Bench>(x: &mut Cx<'_, R>) {
    x.move_by_table_xy();
}

fn speed<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.select_speed(&crate::objects::motion::SPEEDS_B, n as u16);
}

fn set_state_long<R: Bench>(x: &mut Cx<'_, R>, v: u32) {
    x.set32(f::STATE, v);
}

/// Walking direction according to the orientation: `flipped` if the beast looks left (byte `+0x2c` non-zero),
/// `otherwise` if not.
fn set_angle<R: Bench>(x: &mut Cx<'_, R>, flipped: u8, otherwise: u8) {
    let angle = if x.u8(f::FLIP) != 0 { flipped } else { otherwise };
    x.set8(Obj::SPEED_INDEX, angle);
}

/// X offset from the target to the beast (positive: the target is on the right), `None` without a target.
fn target_dx<R: Bench>(x: &mut Cx<'_, R>) -> Option<i16> {
    let t = target(x)?;
    Some(t.u16(x.b, 0x8).wrapping_sub(x.u16(0x8)) as i16)
}

/// Same thing in depth (positive: the target is lower on the screen).
fn target_dy<R: Bench>(x: &mut Cx<'_, R>) -> Option<i16> {
    let t = target(x)?;
    Some(t.u16(x.b, 0x10).wrapping_sub(x.u16(0x10)) as i16)
}

/// 0x5f710: the target is behind the beast, it turns around.
fn turn_if_behind<R: Bench>(x: &mut Cx<'_, R>) {
    if needs_turn(x) {
        set_state_long(x, ACTIVE_TURN);
    }
}

// --- state 0: creation --------------------------------------------------------------------------

const AI_PARAMS: [(u32, u8); 14] =
    [(0x68, 2), (0x69, 2), (0x70, 0), (0x6a, 2), (0x6c, 2), (0x6d, 2), (0x72, 4), (0x73, 3), (0x74, 2), (0x75, 4), (0x76, 3), (0x78, 0xff), (0x81, 9), (0x80, 0xa)];

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    if paused(x) {
        return;
    }
    let scene = var::SCENE.get(x.b);
    if scene == 6 || scene == 0xc {
        x.set8(0x2a, 1);
        x.set8(0x2b, 0x18);
    }
    x.add8(f::STATE, 2);
    x.set8(f::BOX_SET, 0x10);
    lib::initial_hp(x);
    for (off, v) in AI_PARAMS {
        x.set8(off, v);
    }
    x.set32(f::IMMUNITY, 0);
    lib::set_immunities(x);
    x.set8(0x7e, 0xd);
    x.set8(0x7f, 0);
    x.set8(0x8e, 0);
    x.set16(DELAY, 0x258);
    pick_target(x);
    lib::draw_stance(x);
    lib::draw_6b(x);
    match x.u8(Obj::VARIANT) {
        0 => {
            set_state_long(x, ACTIVE_STANCE);
            x.set8(WANT_ANIM, 2);
            x.set8(VULNERABLE, 1);
            body(x, 2);
        }
        1 => {
            set_state_long(x, 0x0200_0000);
            x.set8(TIMER, 0x29);
            x.set8(VULNERABLE, 1);
            x.set8(WANT_ANIM, 2);
            // the original does not save the launched animation here
            if x.u8(WANT_ANIM) != x.u8(CUR_ANIM) {
                body(x, 2);
            }
        }
        2 => {
            x.set16(0x14, (-0x500i16) as u16);
            set_state_long(x, 0x0204_0404);
            x.set8(WANT_ANIM, 0xc);
            x.set8(VULNERABLE, 0);
            strike(x, 1);
            attacks::leap_begin(x);
        }
        _ => {}
    }
}

// --- state 2: active -----------------------------------------------------------------------------

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    active_loop(x, |x| {
        match x.u8(f::SUB) {
            0 => stance::arrival(x),
            2 => stance::run(x),
            4 => attacks::run(x),
            6 => attacks::run_b(x),
            8 => moves::run(x),
            10 => stance::rest(x),
            _ => {}
        }
        false
    });
}

handlers! {
    0x5d87a "enemy_4" #[without] run,
}
