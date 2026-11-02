//! Player inputs.
//!
//! Each frame, `read_inputs` (called by VBLANK) rereads the panel. The RAM keeps the current
//! state (active at 1), the previous state and the new presses. Then `copy_inputs_to_players`
//! copies these bytes, masked, into the record of each player.
//!
//! Bits of a player byte: 0 right, 1 left, 2 down, 3 up, 4..7 buttons 1..4.

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::{Cpu, SR_C, SR_N, SR_V, SR_X, SR_Z};

use crate::mem::{rd16, rd8, rts, set_low_byte, wr16, wr8, A5};
use crate::Native;

/// Offsets relative to A5 (RAM FF8000).
pub mod ram {
    /// Current state: P1, P2, P3, P4, system low, system high.
    pub const CUR: u32 = 0x00;
    /// State of the previous frame (same order).
    pub const PREV: u32 = 0x06;
    /// New presses of P1..P4.
    pub const NEW: u32 = 0x0c;
    /// 4-player mode (0: the buttons of P3 complete those of P1/P2).
    pub const FOUR_PLAYERS: u32 = 0x20;
    /// Input mask applied to the players (0 during cutscenes).
    pub const INPUT_MASK: u32 = 0x43;
    /// Inputs frozen: the players receive the raw state, limited to right/left.
    pub const INPUT_RAW: u32 = 0x44;
    /// Copy of the CPS-B register 0x26 (layer control).
    pub const LAYER_CTRL: u32 = 0x14;
    /// Copy of port 804040 (EEPROM, coins).
    pub const EEPROM_PORT: u32 = 0x16;
    /// The EEPROM port is driven elsewhere (EEPROM access in progress).
    pub const EEPROM_BUSY: u32 = 0x74;
    /// Terrain layers cut off (A5 - 0x7155).
    pub const LAYERS_OFF: u32 = 0xffff_8eab;
    /// First player record; the following ones are spaced 0x200 apart.
    pub const PLAYER1: u32 = 0x5de;
    pub const PLAYER_STRIDE: u32 = 0x200;
    /// In a player record: inputs held, previous, new.
    pub const P_HELD: u32 = 0x6c;
    pub const P_PREV: u32 = 0x6d;
    pub const P_NEW: u32 = 0x6e;
}

/// Panel ports, in the order in which the original reads them (low-order byte first).
const PORTS: [u32; 6] = [0x804001, 0x804000, 0x804011, 0x804010, 0x804020, 0x804021];

fn set_nz_byte(c: &mut Cpu, v: u8) {
    c.flags_logic(v as u32, Size::Byte);
}

/// 0x080c (VBLANK): panel read. Returns what the original leaves in its registers (`InputsRead`).
pub fn read_panel(b: &mut Cps2Bus) -> InputsRead {
    let a5 = A5;
    let four_players = rd8(b, a5 + ram::FOUR_PLAYERS) != 0;
    let layers_on = rd8(b, a5.wrapping_add(ram::LAYERS_OFF)) == 0;
    let eeprom_busy = rd8(b, a5 + ram::EEPROM_BUSY) != 0;

    let mut cur = [0u8; 6];
    for i in 0..6 {
        let old = rd8(b, a5 + ram::CUR + i as u32);
        wr8(b, a5 + ram::PREV + i as u32, old);
        cur[i] = !rd8(b, PORTS[i]);
        wr8(b, a5 + ram::CUR + i as u32, cur[i]);
    }
    let prev: Vec<u8> = (0..4).map(|i| rd8(b, a5 + ram::PREV + i)).collect();
    let pressed: Vec<u8> = (0..4).map(|i| cur[i] & !prev[i]).collect();
    for (i, &p) in pressed.iter().enumerate() {
        wr8(b, a5 + ram::NEW + i as u32, p);
    }

    let d0 = if four_players {
        cur[2]
    } else {
        // 2-player cabinet: button 1 of P3 becomes button 4 of P2, right of P3 becomes
        // button 4 of P1 (wiring of the extra buttons)
        let p2_b4 = (cur[2] & 0x10) << 3;
        let p1_b4 = (cur[2] & 0x01) << 7;
        wr8(b, a5 + ram::CUR + 1, cur[1] | p2_b4);
        wr8(b, a5 + ram::CUR, cur[0] | p1_b4);
        p1_b4
    };

    if !layers_on {
        let v = rd16(b, a5 + ram::LAYER_CTRL) & 0xfff1;
        wr16(b, a5 + ram::LAYER_CTRL, v);
    }
    let layer_ctrl = rd16(b, a5 + ram::LAYER_CTRL);
    wr16(b, 0x804166, layer_ctrl);
    let last = if eeprom_busy {
        Last::Byte(rd8(b, a5 + ram::EEPROM_BUSY))
    } else {
        let v = rd16(b, a5 + ram::EEPROM_PORT);
        wr16(b, 0x804040, v);
        Last::Word(v)
    };
    InputsRead { d: [d0, cur[3], pressed[2], pressed[3]], four_players: four_players, last }
}

/// Last value tested by 0x80c (flags of the original).
#[derive(Clone, Copy)]
pub enum Last {
    Byte(u8),
    Word(u16),
}

/// Leftovers of 0x80c in the registers of the original: low bytes of D0-D3, X cleared (2-player cabinet),
/// flags of the last value tested.
#[derive(Clone, Copy)]
pub struct InputsRead {
    pub d: [u8; 4],
    pub four_players: bool,
    pub last: Last,
}

fn read_inputs(c: &mut Cpu, b: &mut Cps2Bus) {
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

pub const READ_INPUTS: Native = Native {
    entry: 0x080c,
    name: "read_inputs",
    run: crate::Body::Processeur(read_inputs),
    touches_sound: false,
    outputs: crate::regs::ALL,
};

/// 0x2aec: copies the inputs into the player records. Returns the leftovers of the original (`InputCopy`).
pub fn copy(b: &mut Cps2Bus) -> InputCopy {
    let a5 = A5;
    let player = |k: u32| a5 + ram::PLAYER1 + ram::PLAYER_STRIDE * k;
    if rd8(b, a5 + ram::INPUT_RAW) != 0 {
        // inputs frozen: raw state limited to right/left, no new press
        for k in 0..4 {
            let held = rd8(b, a5 + ram::CUR + k) & 3;
            let prev = rd8(b, a5 + ram::PREV + k) & 3;
            wr8(b, player(k) + ram::P_HELD, held);
            wr8(b, player(k) + ram::P_PREV, prev);
        }
        for k in 0..4 {
            wr8(b, player(k) + ram::P_NEW, 0);
        }
        InputCopy::Frozen
    } else {
        let mask = rd8(b, a5 + ram::INPUT_MASK);
        let mut regs = [0u8; 4];
        for pair in [0u32, 2] {
            for (j, k) in [pair, pair + 1].into_iter().enumerate() {
                let held = rd8(b, a5 + ram::CUR + k) & mask;
                let prev = rd8(b, a5 + ram::PREV + k) & mask;
                let new = held & !prev;
                wr8(b, player(k) + ram::P_HELD, held);
                wr8(b, player(k) + ram::P_PREV, prev);
                wr8(b, player(k) + ram::P_NEW, new);
                regs[j] = held;
                regs[2 + j] = new;
            }
        }
        InputCopy::Masked { mask: mask, d: regs }
    }
}

/// Leftovers of 0x2aec in the registers of the original.
#[derive(Clone, Copy)]
pub enum InputCopy {
    /// Inputs frozen: flags of the final `clr.b`.
    Frozen,
    /// D4.b = mask, D0-D3.b = inputs and new presses of the last pair of players; flags of D3.b.
    Masked { mask: u8, d: [u8; 4] },
}

/// `copy` with the registers of the original.
pub fn copy_68k(c: &mut Cpu, b: &mut Cps2Bus) {
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

fn copy_inputs_to_players(c: &mut Cpu, b: &mut Cps2Bus) {
    copy_68k(c, b);
    rts(c, b);
}

pub const COPY_INPUTS_TO_PLAYERS: Native = Native {
    entry: 0x2aec,
    name: "copy_inputs_to_players",
    run: crate::Body::Processeur(copy_inputs_to_players),
    touches_sound: false,
    outputs: crate::regs::ALL,
};

/// Inputs of a player: keys held and new presses.
#[derive(Clone, Copy)]
pub struct Inputs {
    pub held: u8,
    pub fresh: u8,
}

/// 0x2bfe: inputs of the player of object `object` (its number at `+2`).
pub fn player_inputs(b: &mut Cps2Bus, object: u32) -> Inputs {
    let player = rd8(b, object + 2) as u32;
    Inputs { held: rd8(b, A5 + player), fresh: rd8(b, A5 + ram::NEW + player) }
}

/// 0x2bfe: inputs of the player of object A0. Output: D0 = held, D1 = new presses.
fn get_player_input(c: &mut Cpu, b: &mut Cps2Bus) {
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

pub const GET_PLAYER_INPUT: Native = Native {
    entry: 0x2bfe,
    name: "get_player_input",
    run: crate::Body::Processeur(get_player_input),
    touches_sound: false,
    outputs: crate::regs::ALL,
};

/// 0x2bdc: inputs of the four players cleared (word of held keys `+0x6c`, byte `+0x6e`).
pub fn clear_inputs(b: &mut Cps2Bus) {
    for k in 0..4 {
        let player = 0xff85de + 0x200 * k;
        crate::mem::wr16(b, player + 0x6c, 0);
        crate::mem::wr8(b, player + 0x6e, 0);
    }
}

/// `clear_inputs`, flags of the last `clr.b`.
fn clear_player_inputs(c: &mut Cpu, b: &mut Cps2Bus) {
    clear_inputs(b);
    c.sr = c.sr & !0xf | m68k::SR_Z;
    rts(c, b);
}

pub const CLEAR_PLAYER_INPUTS: Native = Native {
    entry: 0x2bdc,
    name: "clear_player_inputs",
    run: crate::Body::Processeur(clear_player_inputs),
    touches_sound: false,
    outputs: crate::regs::CCR,
};
