//! Driver arithmetic.

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

/// Clamps a pitch (semitone.fraction) to semitone 0x6B.
pub fn clamp_pitch(pitch: u16) -> u16 {
    if (pitch >> 8) as u8 >= 0x6c {
        0x6b00 | (pitch & 0xff)
    } else {
        pitch
    }
}
