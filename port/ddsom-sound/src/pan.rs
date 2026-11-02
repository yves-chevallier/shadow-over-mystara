//! Evolution of a voice's panning (sequencer voices and ADPCM voices).
//!
//! The pan goes from 0 (left) to 0x20 (right), in 8.8 fixed point while it evolves.

use crate::math::clamp_pan;

/// How the pan evolves.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum FadeMode {
    /// Back and forth: each time a target is reached, the next target is offset by `delta`.
    PingPong,
    /// Ramp to the target, then fixed pan.
    Ramp,
    /// Ramp to the target, then ramp to the limit at the second speed.
    TwoStage,
    /// Fixed pan.
    #[default]
    Fixed,
}

impl FadeMode {
    /// Value used by the original program (0xFF = fixed).
    pub fn code(self) -> u8 {
        match self {
            FadeMode::PingPong => 0,
            FadeMode::Ramp => 1,
            FadeMode::TwoStage => 2,
            FadeMode::Fixed => 0xff,
        }
    }
}

#[derive(Clone, Copy, Default, Debug)]
pub struct PanFade {
    pub mode: FadeMode,
    /// Position and fraction.
    pub pan: u8,
    pub frac: u8,
    pub delta: u8,
    pub target: u8,
    pub speed: u16,
    pub speed2: u16,
}

impl PanFade {
    fn position(&self) -> u16 {
        u16::from(self.pan) << 8 | u16::from(self.frac)
    }

    /// Setting requested by a 68000 command. Byte 6: type, 7: start pan (0-0x20),
    /// 8: amplitude, 9-10: speed, 11-12: second speed. `None` if the type is unknown.
    /// Type 7 is a ramp to the right that also requests a copy to the left on the
    /// neighbouring channel: see [`PanFade::mirrored`].
    pub fn from_command(cmd: &[u8; 16]) -> Option<PanFade> {
        let start = clamp_pan(cmd[7]);
        let amount = clamp_pan(cmd[8]);
        let speed = u16::from(cmd[9]) << 8 | u16::from(cmd[10]);
        let speed2 = u16::from(cmd[11]) << 8 | u16::from(cmd[12]);
        let neg = |x: u16| 0u16.wrapping_sub(x);
        // Target: `start + amount` or `start - amount`, clamped to 0-0x20.
        let up = start.saturating_add(amount).min(0x20);
        let down = start.saturating_sub(amount);
        let mut f = PanFade { pan: start, ..PanFade::default() };
        match cmd[6] {
            0 => {}
            // Back and forth between the start pan and the opposite end.
            1 => {
                f.mode = FadeMode::PingPong;
                if start < 0x11 {
                    (f.delta, f.target, f.speed) = (0xe0, 0x20, speed);
                } else {
                    (f.delta, f.target, f.speed) = (0x20, 0, neg(speed));
                }
            }
            // Back and forth with amplitude `amount`.
            2 => {
                f.mode = FadeMode::PingPong;
                if start < 0x11 {
                    (f.delta, f.target, f.speed) = (0u8.wrapping_sub(amount), up, speed);
                } else {
                    (f.delta, f.target, f.speed) = (amount, down, neg(speed));
                }
            }
            // Ramp to the right (3, 7) or to the left (4).
            3 | 7 => (f.mode, f.target, f.speed) = (FadeMode::Ramp, up, speed),
            4 => (f.mode, f.target, f.speed) = (FadeMode::Ramp, down, neg(speed)),
            // Two-stage ramp to the right (5) or to the left (6).
            5 => (f.mode, f.target, f.speed, f.speed2) = (FadeMode::TwoStage, up, speed, speed2),
            6 => (f.mode, f.target, f.speed, f.speed2) = (FadeMode::TwoStage, down, neg(speed), neg(speed2)),
            _ => return None,
        }
        Some(f)
    }

    /// Type 7 variant for the neighbouring channel: ramp to the left.
    pub fn mirrored(&self, cmd: &[u8; 16]) -> PanFade {
        let amount = clamp_pan(cmd[8]);
        let speed = u16::from(cmd[9]) << 8 | u16::from(cmd[10]);
        PanFade { target: self.pan.saturating_sub(amount), speed: 0u16.wrapping_sub(speed), ..*self }
    }

    /// One evolution step; returns `true` if the pan changed and `false` if the pan is
    /// fixed (nothing to write).
    pub fn step(&mut self) -> bool {
        if self.mode == FadeMode::Fixed {
            return false;
        }
        // One ramp step: new position, and is the target not yet reached?
        let (pos, carry) = self.speed.overflowing_add(self.position());
        let below = self.target < (pos >> 8) as u8;
        let moving = 0xff_u8.wrapping_add(carry as u8).wrapping_add(below as u8) != 0;
        let reached = u16::from(self.target) << 8 | (pos & 0xff);
        let pos = match self.mode {
            _ if moving => pos,
            FadeMode::PingPong => {
                // Target reached: the target moves to the other side, the speed reverses.
                self.target = self.target.wrapping_add(self.delta);
                self.delta = 0u8.wrapping_sub(self.delta);
                self.speed = 0u16.wrapping_sub(self.speed);
                reached
            }
            FadeMode::TwoStage => {
                // Limit: 0x20 when rising, 0 when falling.
                let limit = if self.speed & 0x8000 != 0 { 0 } else { 0x20 };
                if limit == self.target {
                    self.mode = FadeMode::Fixed;
                } else {
                    self.target = limit;
                    self.speed = self.speed2;
                }
                reached
            }
            _ => {
                self.mode = FadeMode::Fixed;
                reached
            }
        };
        self.pan = (pos >> 8) as u8;
        self.frac = pos as u8;
        true
    }
}
