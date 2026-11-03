//! Recorded inputs of the demonstrations (`0xe05ec`): replay of the recorded entries, and the development recorder.

use super::*;
use crate::hw::io::{DevRam, DEV_RAM};
use crate::ram::{Byte, Long};

// ---------------------------------------------------------------------------------------------
// Recorded inputs (`0xe05ec`)

/// Demonstration input recorder (A5 - 0x5ab2...): pointer to the current entry (long), remaining
/// frames of the entry (word), and three flags: recording, replay, active.
mod rec {
    use crate::ram::{Byte, Long, Word};
    pub const POINTER: Long = Long(0xff_254e);
    pub const LEFT: Word = Word(0xff_2552);
    pub const PLAYING: Byte = Byte(0xff_2554);
    pub const RECORDING: Byte = Byte(0xff_2555);
    pub const ACTIVE: Byte = Byte(0xff_2556);
    /// An entry: duration (word), then the three input bytes of each player.
    pub const ENTRY: u32 = 0xe;
    /// Last possible entry.
    pub const LAST: u32 = 0x93fff2;
}

/// The twelve input bytes of the players (`A5 + 0x64a + 0x200 k`, three bytes each), in the order of an entry.
pub(super) fn input_byte(i: u32) -> Byte {
    Byte(crate::ram::PLAYERS.nth(i / 3).0 + INPUTS + i % 3)
}

/// The three input bytes of a player record (`+0x6c`: held, previous, new).
const INPUTS: u32 = 0x6c;

/// `0xe05ec`: records the players' inputs (one entry per change, with its duration) or replays them (a
/// game in progress with bit 15 of the settings suspends the replay).
pub(super) fn demo_inputs(x: &mut Ctx) {
    if rec::ACTIVE.get(x.b) == 0 {
        return;
    }
    if rec::RECORDING.get(x.b) != 0 {
        record_inputs(x);
    } else if rec::PLAYING.get(x.b) != 0 {
        if var::GAME.get(x.b) != 0 && var::DSW.get(x.b) & 0x8000 != 0 {
            return;
        }
        let at = rec::POINTER.get(x.b);
        if at > rec::LAST {
            rec::PLAYING.set(x.b, 0);
            return;
        }
        for i in 0..12 {
            let v = tables::byte(x.b, at + 2 + i);
            input_byte(i).set(x.b, v);
        }
        let left = rec::LEFT.get(x.b);
        if left != 0 {
            rec::LEFT.set(x.b, left - 1);
        } else {
            let next = at + rec::ENTRY;
            rec::POINTER.set(x.b, next);
            let length = tables::word(x.b, next);
            rec::LEFT.set(x.b, length);
        }
    }
}

/// `0xe0538` (scene entry, outside a game): prepares the recorder. Setting bit 13: random seed and
/// frame counters to zero, then replay active; bit 15: recording at 0x930000; otherwise replay
/// (inactive) of the demonstration pointed to by `A5-0x5aa8`.
pub fn reset_recorder(b: &mut Cps2Bus) {
    reset_recorder_by(b);
}

/// What 0xe0538 leaves in the registers: D5.w = the settings read, A0 = the demonstration replayed.
#[derive(Default)]
pub(crate) struct RecorderLeftovers {
    pub(crate) settings: Option<u16>,
    pub(crate) demo: Option<u32>,
}

pub(crate) fn reset_recorder_by(b: &mut Cps2Bus) -> RecorderLeftovers {
    const DEMO: Long = Long(0xff_2558);
    let mut leftovers = RecorderLeftovers::default();
    if var::GAME.get(b) != 0 {
        return leftovers;
    }
    rec::LEFT.set(b, 0);
    rec::RECORDING.set(b, 0);
    rec::PLAYING.set(b, 0);
    rec::ACTIVE.set(b, 0);
    let dsw = var::DSW.get(b);
    leftovers.settings = Some(dsw);
    if dsw & 0x2000 != 0 {
        crate::rng::seed(b);
        var::DEMO_TIME.set(b, 0);
        var::FRAME_COUNTER.set(b, 0);
        var::RANK.set(b, 0xe);
        rec::ACTIVE.set(b, 1);
    } else if dsw & 0x8000 != 0 {
        rec::POINTER.set(b, DEV_RAM.0);
        for off in [0, 4, 8] {
            DEV_RAM.offset(off).set_long(b, 0xffff_ffff);
        }
        DEV_RAM.offset(0xc).set_word(b, 0xffff);
        rec::RECORDING.set(b, 1);
        return leftovers;
    }
    let demo = DEMO.get(b);
    leftovers.demo = Some(demo);
    rec::POINTER.set(b, demo);
    let length = tables::word(b, demo);
    rec::LEFT.set(b, length);
    rec::PLAYING.set(b, 1);
    leftovers
}

/// `0xe05d6`: stops the recorder.
pub fn stop_recorder(b: &mut Cps2Bus) {
    rec::LEFT.set(b, 0);
    for flag in [rec::RECORDING, rec::PLAYING, rec::ACTIVE] {
        flag.set(b, 0);
    }
    var::SCRIPT_TAIL.set(b, 0);
}

pub const RECORDER_STOP: Native = Native {
    entry: 0xe05d6,
    name: "demo_recorder_stop",
    run: crate::Body::Memory(stop_recorder),
    touches_sound: false,
    outputs: 0,
};

/// Recording: the current entry lasts one more frame as long as the inputs do not change (and its duration
/// fits in 16 bits); otherwise its duration is written and a new entry begins.
pub(super) fn record_inputs(x: &mut Ctx) {
    let length = rec::LEFT.get(x.b);
    let at = rec::POINTER.get(x.b);
    if length != 0xffff && (0..12).all(|i| input_byte(i).get(x.b) == tables::byte(x.b, at + 2 + i)) {
        rec::LEFT.set(x.b, length + 1);
        return;
    }
    DevRam(at).set_word(x.b, length);
    rec::LEFT.set(x.b, 0);
    let next = at + rec::ENTRY;
    rec::POINTER.set(x.b, next);
    if next > rec::LAST {
        rec::RECORDING.set(x.b, 0);
        return;
    }
    for i in 0..12 {
        let v = input_byte(i).get(x.b);
        DevRam(next + 2 + i).set_byte(x.b, v);
    }
}
