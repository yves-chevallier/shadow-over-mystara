//! Tools: the natives of `crate::hw::inputs` for the original code that calls them (arguments and results in the registers
//! of the original).

use crate::world::World;
use m68k::decode::Size;
use m68k::{Cpu, SR_C, SR_N, SR_V, SR_X, SR_Z};

use crate::hw::inputs::*;
use crate::tools::frames::*;
use crate::Native;
use crate::world::mem::{rd8, set_low_byte};

fn set_nz_byte(c: &mut Cpu, v: u8) {
    c.flags_logic(v as u32, Size::Byte);
}

fn read_inputs(c: &mut Cpu, b: &mut World) {
    let r = read_panel(b);
    for (n, v) in r.d.into_iter().enumerate() {
        set_low_byte(&mut c.d[n], v);
    }
    if !r.four_players {
        // lsl.b #3 on (cur2 & 0x10): the last bit shifted out is 0, X and C become 0
        c.sr &= !SR_X;
    }
    match r.last {
        Last::Byte(v) => set_nz_byte(c, v),
        Last::Word(v) => c.flags_logic(v as u32, Size::Word),
    }
    rts(c, b);
}

pub(crate) const READ_INPUTS: Native = Native {
    entry: 0x080c,
    name: "read_inputs",
    run: crate::Body::WithCpu(read_inputs),
    touches_sound: false,
    outputs: crate::regs::ALL,
};

/// `copy` with the registers of the original.
pub(crate) fn copy_68k(c: &mut Cpu, b: &mut World) {
    match copy(b) {
        // final clr.b
        InputCopy::Frozen => c.sr = (c.sr & !(SR_N | SR_V | SR_C)) | SR_Z,
        InputCopy::Masked { mask, d } => {
            set_low_byte(&mut c.d[4], mask);
            for (n, v) in d.into_iter().enumerate() {
                set_low_byte(&mut c.d[n], v);
            }
            // final move.b d3,(...)
            set_nz_byte(c, d[3]);
        }
    }
}

fn copy_inputs_to_players(c: &mut Cpu, b: &mut World) {
    copy_68k(c, b);
    rts(c, b);
}

pub(crate) const COPY_INPUTS_TO_PLAYERS: Native = Native {
    entry: 0x2aec,
    name: "copy_inputs_to_players",
    run: crate::Body::WithCpu(copy_inputs_to_players),
    touches_sound: false,
    outputs: crate::regs::ALL,
};

/// 0x2bfe: inputs of the player of object A0. Output: D0 = held, D1 = new presses.
fn get_player_input(c: &mut Cpu, b: &mut World) {
    let player = rd8(b, c.a[0] + 2) as u32;
    c.d[2] = player;
    c.a[3] = c.a[5];
    c.a[4] = c.a[5].wrapping_add(ram::NEW);
    let held = rd8(b, c.a[3] + player);
    let new = rd8(b, c.a[4] + player);
    set_low_byte(&mut c.d[0], held);
    set_low_byte(&mut c.d[1], new);
    set_nz_byte(c, new);
    rts(c, b);
}

pub(crate) const GET_PLAYER_INPUT: Native = Native {
    entry: 0x2bfe,
    name: "get_player_input",
    run: crate::Body::WithCpu(get_player_input),
    touches_sound: false,
    outputs: crate::regs::ALL,
};

/// `clear_inputs`, flags of the last `clr.b`.
fn clear_player_inputs(c: &mut Cpu, b: &mut World) {
    clear_inputs(b);
    c.sr = c.sr & !0xf | m68k::SR_Z;
    rts(c, b);
}

pub(crate) const CLEAR_PLAYER_INPUTS: Native = Native {
    entry: 0x2bdc,
    name: "clear_player_inputs",
    run: crate::Body::WithCpu(clear_player_inputs),
    touches_sound: false,
    outputs: crate::regs::CCR,
};
