//! ADPCM voices: three channels that play a sample once (or looped) without a sequencer.
//!
//! The original program has no notion of pitch here: an ADPCM voice is started by a
//! "single-voice effect" command whose voice number is at least 0x10.

use crate::math::*;
use crate::pan::PanFade;
use crate::Driver;

#[derive(Clone, Default)]
pub struct AdpcmChannel {
    /// Has just started: the next tick programs the registers.
    pub fresh: bool,
    /// Replays the sample in a loop until explicitly stopped.
    pub continuous: bool,
    /// Remaining duration, counted down in steps of 0x40 per tick; 0 = free channel.
    pub remaining: u16,
    pub volume: u16,
    pub pan: PanFade,
    pub delay: u8,
    pub priority: u8,
    /// Total duration, for looping.
    pub length: u16,
    pub bank: u8,
    pub start: u16,
    pub end: u16,
}

impl Driver {
    /// Silences an ADPCM channel.
    pub(crate) fn silence_adpcm(&mut self, ch: u8) {
        self.qsound(0xcd_u8.wrapping_add(ch.wrapping_mul(4)), 0);
        self.qsound(0xd6_u8.wrapping_add(ch), 1);
    }

    pub(crate) fn stop_all_adpcm(&mut self) {
        for ch in 0..3u8 {
            let a = &mut self.adpcm[usize::from(ch)];
            let in_use = a.remaining != 0;
            a.priority = 0;
            a.remaining = 0;
            if in_use {
                self.silence_adpcm(ch);
            }
        }
    }

    pub(crate) fn stop_adpcm(&mut self, ch: u8) {
        let Some(a) = self.adpcm.get_mut(usize::from(ch)) else { return };
        if a.remaining == 0 {
            return;
        }
        a.priority = 0;
        a.remaining = 0;
        self.silence_adpcm(ch);
    }

    /// Starts an ADPCM voice. `p` points to the effect definition: flags and priority,
    /// 4 ignored bytes, amplitude, sample number.
    pub(crate) fn start_adpcm(&mut self, ch: u8, mut p: u32, cmd: &[u8; 16]) {
        let slot = usize::from(ch);
        if slot >= self.adpcm.len() {
            return;
        }
        self.cur = ch;
        let flags = self.rom.byte(p);
        p += 1;
        self.priority = flags;
        if flags & 0x7f < self.adpcm[slot].priority {
            return;
        }
        let amplitude = self.rom.amplitude(self.rom.byte(p + 4));
        let sample = self.rom.adpcm_sample(self.rom.word_be(p + 5));
        let (start, end) = (sample.start, sample.end);
        let Some(fade) = PanFade::from_command(cmd) else { return };
        let mut a = AdpcmChannel {
            fresh: true,
            continuous: flags & 0x80 != 0,
            remaining: end.wrapping_sub(start),
            volume: hi16(amplitude, self.g.sfx_volume) << 2,
            pan: fade,
            priority: flags & 0x7f,
            length: end.wrapping_sub(start),
            bank: sample.bank,
            start,
            end,
            ..AdpcmChannel::default()
        };
        if cmd[6] == 7 {
            // The neighbouring channel receives a copy moving in the other direction, if its priority allows it.
            let next = usize::from((ch + 1) % 3);
            if self.priority >= self.adpcm[next].priority {
                let mut copy = a.clone();
                copy.pan = fade.mirrored(cmd);
                copy.delay = 4;
                self.adpcm[next] = copy;
            }
        }
        a.delay = 0;
        self.adpcm[slot] = a;
        self.silence_adpcm(ch);
    }

    /// Tick of the three ADPCM channels.
    pub(crate) fn adpcm_tick(&mut self) {
        for ch in 0..3u8 {
            self.cur = ch;
            if self.adpcm[usize::from(ch)].remaining != 0 {
                let mut a = std::mem::take(&mut self.adpcm[usize::from(ch)]);
                self.adpcm_channel_tick(&mut a, ch);
                self.adpcm[usize::from(ch)] = a;
            }
        }
    }

    fn adpcm_channel_tick(&mut self, a: &mut AdpcmChannel, ch: u8) {
        if a.delay != 0 {
            a.delay -= 1;
            if a.delay == 0 {
                self.silence_adpcm(ch);
            }
            return;
        }
        let mut program = a.fresh;
        if !a.fresh {
            a.remaining = a.remaining.saturating_sub(0x40);
            if a.remaining == 0 {
                if !a.continuous {
                    a.priority = 0;
                    self.silence_adpcm(ch);
                    return;
                }
                a.remaining = a.length;
                program = true;
            }
        }
        if program {
            self.program_adpcm(a, ch);
        }
        // The first tick writes the start pan; after that it evolves.
        let first = std::mem::take(&mut a.fresh);
        if first || a.pan.step() {
            self.write_pan(0x90 + ch, a.pan.pan, true);
        }
    }

    /// Writes addresses, bank, volume and starts playback.
    fn program_adpcm(&mut self, a: &AdpcmChannel, ch: u8) {
        let base = ch.wrapping_mul(4);
        self.qsound(0xca_u8.wrapping_add(base), a.start);
        self.qsound(0xcb_u8.wrapping_add(base), a.end);
        self.qsound(0xcc_u8.wrapping_add(base), 0x8000 | u16::from(a.bank));
        let mut volume = a.volume;
        if self.g.fade_dir != 0 {
            volume = hi16(volume, self.g.fade_level);
        }
        self.qsound(0xcd_u8.wrapping_add(base), volume);
        self.qsound(0xd6_u8.wrapping_add(ch), 1);
    }
}
