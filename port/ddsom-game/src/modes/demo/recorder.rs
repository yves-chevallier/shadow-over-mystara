//! Recorded inputs of the demonstrations (`0xe05ec`): replay of the recorded entries, and the development recorder.

use super::*;

// ---------------------------------------------------------------------------------------------
// Recorded inputs (`0xe05ec`)

/// Demonstration input recorder (A5 - 0x5ab2...): pointer to the current entry (long), remaining
/// frames of the entry (word), and three flags: recording, replay, active.
mod rec {
    use crate::mem::A5;
    pub const POINTER: u32 = A5 - 0x5ab2;
    pub const LEFT: u32 = A5 - 0x5aae;
    pub const PLAYING: u32 = A5 - 0x5aac;
    pub const RECORDING: u32 = A5 - 0x5aab;
    pub const ACTIVE: u32 = A5 - 0x5aaa;
    /// An entry: duration (word), then the three input bytes of each player.
    pub const ENTRY: u32 = 0xe;
    /// Last possible entry.
    pub const LAST: u32 = 0x93fff2;
}

/// The twelve input bytes of the players (`A5 + 0x64a + 0x200 k`, three bytes each), in the order of an entry.
pub(super) fn input_byte(i: u32) -> u32 {
    A5 + 0x64a + 0x200 * (i / 3) + i % 3
}

/// `0xe05ec`: records the players' inputs (one entry per change, with its duration) or replays them (a
/// game in progress with bit 15 of the settings suspends the replay).
pub(super) fn demo_inputs(x: &mut Ctx) {
    if rd8(x.b, rec::ACTIVE) == 0 {
        return;
    }
    if rd8(x.b, rec::RECORDING) != 0 {
        record_inputs(x);
    } else if rd8(x.b, rec::PLAYING) != 0 {
        if rd8(x.b, var::GAME) != 0 && rd16(x.b, var::DSW) & 0x8000 != 0 {
            return;
        }
        let at = rd32(x.b, rec::POINTER);
        if at > rec::LAST {
            wr8(x.b, rec::PLAYING, 0);
            return;
        }
        for i in 0..12 {
            let v = rd8(x.b, at + 2 + i);
            wr8(x.b, input_byte(i), v);
        }
        let left = rd16(x.b, rec::LEFT);
        if left != 0 {
            wr16(x.b, rec::LEFT, left - 1);
        } else {
            let next = at + rec::ENTRY;
            wr32(x.b, rec::POINTER, next);
            let length = rd16(x.b, next);
            wr16(x.b, rec::LEFT, length);
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
    const DEMO: u32 = A5 - 0x5aa8;
    const RECORD_AREA: u32 = 0x93_0000;
    let mut leftovers = RecorderLeftovers::default();
    if rd8(b, var::GAME) != 0 {
        return leftovers;
    }
    wr16(b, rec::LEFT, 0);
    wr8(b, rec::RECORDING, 0);
    wr8(b, rec::PLAYING, 0);
    wr8(b, rec::ACTIVE, 0);
    let dsw = rd16(b, var::DSW);
    leftovers.settings = Some(dsw);
    if dsw & 0x2000 != 0 {
        crate::rng::seed(b);
        wr16(b, A5 + 0x6c, 0);
        wr16(b, A5 + 0x6e, 0);
        wr16(b, A5 + 0x7f92, 0xe);
        wr8(b, rec::ACTIVE, 1);
    } else if dsw & 0x8000 != 0 {
        wr32(b, rec::POINTER, RECORD_AREA);
        for off in [0, 4, 8] {
            wr32(b, RECORD_AREA + off, 0xffff_ffff);
        }
        wr16(b, RECORD_AREA + 0xc, 0xffff);
        wr8(b, rec::RECORDING, 1);
        return leftovers;
    }
    let demo = rd32(b, DEMO);
    leftovers.demo = Some(demo);
    wr32(b, rec::POINTER, demo);
    let length = rd16(b, demo);
    wr16(b, rec::LEFT, length);
    wr8(b, rec::PLAYING, 1);
    leftovers
}

/// `0xe05d6`: stops the recorder.
pub fn stop_recorder(b: &mut Cps2Bus) {
    wr16(b, rec::LEFT, 0);
    for flag in [rec::RECORDING, rec::PLAYING, rec::ACTIVE] {
        wr8(b, flag, 0);
    }
    wr32(b, A5 - 0x5aa8, 0);
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
    let length = rd16(x.b, rec::LEFT);
    let at = rd32(x.b, rec::POINTER);
    if length != 0xffff && (0..12).all(|i| rd8(x.b, input_byte(i)) == rd8(x.b, at + 2 + i)) {
        wr16(x.b, rec::LEFT, length + 1);
        return;
    }
    wr16(x.b, at, length);
    wr16(x.b, rec::LEFT, 0);
    let next = at + rec::ENTRY;
    wr32(x.b, rec::POINTER, next);
    if next > rec::LAST {
        wr8(x.b, rec::RECORDING, 0);
        return;
    }
    for i in 0..12 {
        let v = rd8(x.b, input_byte(i));
        wr8(x.b, next + 2 + i, v);
    }
}
