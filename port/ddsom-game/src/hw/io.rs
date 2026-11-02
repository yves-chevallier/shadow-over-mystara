//! Input and output ports of the CPS2 board (docs/video.md, `credits.rs`).

use cps2::Cps2Bus;

use crate::mem::{rd16, rd8, wr16};

/// An input port (read only): bytes and words of the board's input registers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Port(pub u32);

impl Port {
    pub fn get(self, b: &mut Cps2Bus) -> u8 {
        rd8(b, self.0)
    }
    pub fn word(self, b: &mut Cps2Bus) -> u16 {
        rd16(b, self.0)
    }
}

/// Players 1 and 2 (`IN0`, active low).
pub const IN0: Port = Port(0x804000);
/// Players 3 and 4 (`IN1`).
pub const IN1: Port = Port(0x804010);
/// `IN2`, high byte: start buttons (bits 0 to 3) and coin slot lines (bits 4 to 7: coins 1 to 4), active low.
pub const IN2: Port = Port(0x804020);
/// `IN2`, low byte: service buttons (bit 1: TEST switch, bit 2: service), active low.
pub const IN2_SERVICE: Port = Port(0x804021);

/// An output register (word, write only).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Output(pub u32);

impl Output {
    pub fn set(self, b: &mut Cps2Bus, v: u16) {
        wr16(b, self.0, v)
    }
}
