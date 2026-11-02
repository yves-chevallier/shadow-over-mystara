//! Driver arithmetic and addresses of the Z80 ROM tables.

/// Table of 128 words: voice amplitude (instruction 7, effect commands).
pub const TABLE_AMPLITUDE: u32 = 0x33e1;
/// Table of words: general levels (base volumes, ramp targets).
pub const TABLE_LEVELS: u32 = 0x34e1;
/// Table of words: envelope attack rates.
pub const TABLE_ATTACK: u32 = 0x35e1;
/// Table of words: envelope decay rates (decay, release).
pub const TABLE_DECAY: u32 = 0x3661;
/// Table of words: LFO rates (vibrato and tremolo).
pub const TABLE_LFO_RATE: u32 = 0x36e1;
/// Table of words: vibrato depths.
pub const TABLE_VIBRATO: u32 = 0x37e1;
/// Table of words: tremolo depths.
pub const TABLE_TREMOLO: u32 = 0x38e1;
/// Address (word) of the music header.
pub const HEADER_PTR: u32 = 0x39e1;
/// Address (word) of the sample table: 8-byte records [bank, start, loop
/// start, end, reference note].
pub const SAMPLE_PTR: u32 = 0x39e3;
/// Address (word) of the instrument bank table.
pub const INSTRUMENT_BANKS_PTR: u32 = 0x39e5;

/// Octave of each semitone (0-107), then octave start in semitones.
pub const TABLE_OCTAVE: u32 = 0x1fc6;
pub const TABLE_OCTAVE_START: u32 = 0x2032;
/// Base rate of each semitone (words), and fine interpolation (12 x 256 bytes).
pub const TABLE_RATE: u32 = 0x203b;
pub const TABLE_FINE: u32 = 0x2113;
/// Note durations (words) depending on the mode (normal, dotted, triplet).
pub const TABLE_DURATION: u32 = 0x1119;
/// Octave offset of the voice (low 4 bits of the flags).
pub const TABLE_OCTAVE_BASE: u32 = 0x1149;
/// Pan values: register = PAN_BASE + position (0-0x20).
pub const PAN_BASE: u16 = 0x110;

/// 16 x 16-bit product, high words (the Z80 does it with shifts and additions).
pub fn hi16(a: u16, b: u16) -> u16 {
    ((u32::from(a) * u32::from(b)) >> 16) as u16
}

/// Byte sign-extended to a signed word.
pub fn sext(b: u8) -> u16 {
    i16::from(b as i8) as u16
}

/// Clamps a pan to 0x20.
pub fn clamp_pan(p: u8) -> u8 {
    p.min(0x20)
}
