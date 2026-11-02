//! Phase 24 (`0x21c2a`): scripted sequences between two game phases (h).
//!
//! The phase is triggered by the scene controller (SET 2, end action 4: the players exit) or
//! by a scene object. It contains two sequences, chosen by the step (`A5+0x2d`, step of 2):
//!
//! * step 0: a boss (type 9) enters, a dialogue (message 4) opens, the camera arrives at 800, the
//!   boss advances, a second dialogue (message 6) concludes and the game resumes (phase 12);
//! * step 2: forced transition (`A5+0x51e` = 76 -> 92), the camera advances to 2408, then the game resumes.
//!
//! Each frame then calls the input copy, the world update and the sprites.

use cps2::Cps2Bus;

use super::{scratch, var, Ctx};
use crate::mem::wr32;
use crate::Native;
use crate::objects::pools;

/// Temporary variables of the two sequences.
const TIMER: u32 = scratch(0x9a2a);
const MESSAGE: u32 = scratch(0x9a2c);
const BOSS: u32 = scratch(0x9a30);

/// Wait duration depending on the number of players (program-space words, in the code).
const WAITS: u32 = 0x21c94;
/// State counter of the entering boss (byte): 12 = has finished entering.
const BOSS_STATE: u32 = 0x4ff;
/// Camera x position (word) to reach / forced scroll speed.
const CAMERA_X: u32 = 0x506;
const ALT_CAMERA_X: u32 = 0x556;
const TRANSITION: u32 = 0x51e;
/// BCD game clock on return to the game: 4:59:59... (minutes, seconds, frames).
const GAME_CLOCK: u32 = 0x38;
const CLOCK_START: u32 = 0x0459_5900;
/// Phase 12, step and sub-step at zero.
const GAME_PHASE: u32 = 0x0c00_0000;

const STEP_BOSS_INTRO: u8 = 0;
const STEP_SCROLL: u8 = 2;

pub fn phase_24(b: &mut Cps2Bus) {
    let mut x = Ctx::new(b);
    match x.v8(var::STEP) {
        STEP_BOSS_INTRO => boss_intro(&mut x),
        STEP_SCROLL => scroll_out(&mut x),
        _ => {}
    }
    x.inputs();
    x.world();
}

/// Hands control back to the game: phase 12, free scene, clock re-armed.
fn resume_game(x: &mut Ctx) {
    x.set_v32(var::PHASE, GAME_PHASE);
    x.set_v8(var::HOLD, 0);
    x.set_v8(var::PLAYERS_FROZEN, 0);
    x.set_v32(GAME_CLOCK, CLOCK_START);
}

fn boss_intro(x: &mut Ctx) {
    match x.sub() {
        0 => {
            x.next_sub();
            x.lock();
            x.set_v8(var::INPUT_LOCK, 0);
            let players = x.v8(var::PLAYERS) as u32;
            let wait = x.prog16(WAITS + 2 * players);
            x.wr16(TIMER, wait);
        }
        2 => {
            if x.tick(TIMER) {
                x.next_sub();
                x.set_v8(BOSS_STATE, 8);
                let boss = x.spawn(&pools::BOSS, 9);
                x.store_obj(BOSS, boss);
            }
        }
        4 => {
            if x.v8(BOSS_STATE) == 12 {
                x.next_sub();
                x.message(4, MESSAGE);
            }
        }
        6 => {
            if x.message_done(MESSAGE) {
                x.next_sub();
                let boss = x.load_obj(BOSS);
                boss.set_u8(x.b, 6, 2);
            }
        }
        8 => {
            if x.v16(ALT_CAMERA_X) == 800 {
                x.next_sub();
                x.set_v8(var::INPUT_LOCK, 0xff);
                x.wr16(TIMER, 80);
            }
        }
        10 => {
            if x.tick(TIMER) {
                x.next_sub();
                x.set_v8(var::HOLD, 0);
                let boss = x.load_obj(BOSS);
                let state = boss.u8(x.b, 6).wrapping_add(2);
                boss.set_u8(x.b, 6, state);
            } else if x.rd16(TIMER) == 40 {
                crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::B, 40);
            }
        }
        12 => {
            let boss = x.load_obj(BOSS);
            if boss.u8(x.b, 6) == 0x16 {
                x.next_sub();
                x.set_v8(var::HOLD, 1);
                x.wr16(TIMER, 40);
            }
        }
        14 => {
            if x.tick(TIMER) {
                x.next_sub();
                x.message(6, MESSAGE);
            }
        }
        16 => {
            if x.message_done(MESSAGE) {
                resume_game(x);
                let boss = x.load_obj(BOSS);
                wr32(x.b, boss.0 + 4, 0x0202_0000);
            }
        }
        _ => {}
    }
}

fn scroll_out(x: &mut Ctx) {
    match x.sub() {
        0 => {
            x.next_sub();
            x.lock();
            x.set_v8(var::INPUT_LOCK, 0xff);
            x.wr16(TIMER, 20);
        }
        2 => {
            if x.rd16(TIMER) != 0 {
                if x.tick(TIMER) {
                    x.set_v16(TRANSITION, 76);
                    crate::families::effects_creators::types_54(x.b);
                }
            } else if x.v16(TRANSITION) == 92 {
                x.next_sub();
                x.wr16(TIMER, 48);
            }
        }
        4 => {
            if x.rd16(TIMER) != 0 {
                x.tick(TIMER);
            } else if x.v16(CAMERA_X) <= 2408 {
                let camera = x.v16(CAMERA_X).wrapping_add(2);
                x.set_v16(CAMERA_X, camera);
            } else {
                x.next_sub();
                x.set_v16(CAMERA_X, 2408);
                crate::families::effects_creators::type_53_pure(x.b);
            }
        }
        6 => {} // 0xb66e4 itself moves to sub-step 8
        8 => {
            x.set_v32(var::PHASE, GAME_PHASE);
            x.set_v8(var::HOLD, 0);
            x.set_v8(var::PLAYERS_FROZEN, 0);
            x.set_v8(var::INPUT_LOCK, 0xff);
            x.set_v32(GAME_CLOCK, CLOCK_START);
        }
        _ => {}
    }
}

pub const PHASE_24: Native = Native {
    entry: 0x21c2a,
    name: "scene_phase_24",
    run: crate::Body::Memory(phase_24),
    touches_sound: false,
    outputs: 0,
};
