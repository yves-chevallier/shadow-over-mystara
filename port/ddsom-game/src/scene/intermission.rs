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

use crate::screens::tables::Rom;
use cps2::Cps2Bus;

use super::{scratch, var, Ctx};
use crate::objects::Obj;
use crate::Native;
use crate::objects::pools;
use crate::ram::{Byte, Long, Word};

/// Action of the boss (byte `+6` of its state): the sequences drive its entry through it.
const BOSS_ACTION: u32 = 6;

/// Temporary variables of the two sequences.
const TIMER: u32 = scratch(0x9a2a);
const MESSAGE: u32 = scratch(0x9a2c);
const BOSS: u32 = scratch(0x9a30);

/// Wait duration depending on the number of players (program-space words, in the code).
const WAITS: Rom = Rom::at(0x21c94);
/// State counter of the entering boss (byte): 12 = has finished entering.
const BOSS_STATE: Byte = Byte::a5(0x4ff);
/// Camera x position (word) to reach / forced scroll speed.
const CAMERA_X: Word = Word::a5(0x506);
const ALT_CAMERA_X: Word = Word::a5(0x556);
const TRANSITION: Word = Word::a5(0x51e);
/// BCD game clock on return to the game: 4:59:59... (minutes, seconds, frames).
const GAME_CLOCK: Long = Long::a5(0x38);
const CLOCK_START: u32 = 0x0459_5900;
/// Phase 12, step and sub-step at zero.
const GAME_PHASE: u32 = 0x0c00_0000;

const STEP_BOSS_INTRO: u8 = 0;
const STEP_SCROLL: u8 = 2;

pub fn phase_24(b: &mut Cps2Bus) {
    let mut x = Ctx::new(b);
    match var::STEP.get(x.b) {
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
    var::HOLD.set(x.b, 0);
    var::PLAYERS_FROZEN.set(x.b, 0);
    GAME_CLOCK.set(x.b, CLOCK_START);
}

fn boss_intro(x: &mut Ctx) {
    match x.sub() {
        0 => {
            x.next_sub();
            x.lock();
            var::INPUT_LOCK.set(x.b, 0);
            let players = var::PLAYERS.get(x.b) as u32;
            let wait = WAITS.pc_word(x.b, 2 * players);
            Word(TIMER).set(x.b, wait);
        }
        2 => {
            if x.tick(TIMER) {
                x.next_sub();
                BOSS_STATE.set(x.b, 8);
                let boss = x.spawn(&pools::BOSS, 9);
                x.store_obj(BOSS, boss);
            }
        }
        4 => {
            if BOSS_STATE.get(x.b) == 12 {
                x.next_sub();
                x.message(4, MESSAGE);
            }
        }
        6 => {
            if x.message_done(MESSAGE) {
                x.next_sub();
                let boss = x.load_obj(BOSS);
                boss.set_u8(x.b, BOSS_ACTION, 2);
            }
        }
        8 => {
            if ALT_CAMERA_X.get(x.b) == 800 {
                x.next_sub();
                var::INPUT_LOCK.set(x.b, 0xff);
                Word(TIMER).set(x.b, 80);
            }
        }
        10 => {
            if x.tick(TIMER) {
                x.next_sub();
                var::HOLD.set(x.b, 0);
                let boss = x.load_obj(BOSS);
                let state = boss.u8(x.b, BOSS_ACTION).wrapping_add(2);
                boss.set_u8(x.b, BOSS_ACTION, state);
            } else if Word(TIMER).get(x.b) == 40 {
                crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::B, 40);
            }
        }
        12 => {
            let boss = x.load_obj(BOSS);
            if boss.u8(x.b, BOSS_ACTION) == 0x16 {
                x.next_sub();
                var::HOLD.set(x.b, 1);
                Word(TIMER).set(x.b, 40);
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
                boss.set_u32(x.b, Obj::STATE, 0x0202_0000);
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
            var::INPUT_LOCK.set(x.b, 0xff);
            Word(TIMER).set(x.b, 20);
        }
        2 => {
            if Word(TIMER).get(x.b) != 0 {
                if x.tick(TIMER) {
                    TRANSITION.set(x.b, 76);
                    crate::families::effects_creators::types_54(x.b);
                }
            } else if TRANSITION.get(x.b) == 92 {
                x.next_sub();
                Word(TIMER).set(x.b, 48);
            }
        }
        4 => {
            if Word(TIMER).get(x.b) != 0 {
                x.tick(TIMER);
            } else if CAMERA_X.get(x.b) <= 2408 {
                let camera = CAMERA_X.get(x.b).wrapping_add(2);
                CAMERA_X.set(x.b, camera);
            } else {
                x.next_sub();
                CAMERA_X.set(x.b, 2408);
                crate::families::effects_creators::type_53_pure(x.b);
            }
        }
        6 => {} // 0xb66e4 itself moves to sub-step 8
        8 => {
            x.set_v32(var::PHASE, GAME_PHASE);
            var::HOLD.set(x.b, 0);
            var::PLAYERS_FROZEN.set(x.b, 0);
            var::INPUT_LOCK.set(x.b, 0xff);
            GAME_CLOCK.set(x.b, CLOCK_START);
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
