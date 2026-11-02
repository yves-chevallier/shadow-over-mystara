//! Image of the Z80 RAM (F000-FFFF) as the original program keeps it.
//!
//! The Rust driver does not need it to work: it is a validation tool that allows comparing,
//! field by field, the driver state with the original Z80 RAM.

use crate::adpcm::AdpcmChannel;
use crate::voice::Voice;
use crate::{Boot, Driver, SoundData};

/// Start of the Z80 RAM.
pub const RAM_BASE: usize = 0xf000;
/// Addresses of the voice arrays.
pub const SFX_VOICES: usize = 0xf200;
pub const MUSIC_VOICES: usize = 0xf700;
pub const ADPCM_CHANNELS: usize = 0xfc00;
pub const VOICE_SIZE: usize = 0x50;
pub const ADPCM_SIZE: usize = 0x18;

fn put16(ram: &mut [u8], addr: usize, v: u16) {
    ram[addr - RAM_BASE] = v as u8;
    ram[addr - RAM_BASE + 1] = (v >> 8) as u8;
}

fn put(ram: &mut [u8], addr: usize, v: u8) {
    ram[addr - RAM_BASE] = v;
}

impl Voice {
    /// Bytes of the voice record (0x50 bytes); track positions and instrument banks become Z80 addresses.
    pub fn image(&self, data: &SoundData) -> [u8; VOICE_SIZE] {
        let mut b = [0u8; VOICE_SIZE];
        let w = |b: &mut [u8], at: usize, v: u16| {
            b[at] = v as u8;
            b[at + 1] = (v >> 8) as u8;
        };
        // Pointer: bank (0 in the fixed area) then address in the 8000-BFFF window.
        let seq = self.seq.map_or(0, |at| data.addr(at));
        let (bank, addr) = if seq < 0x8000 { (0, seq as u16) } else { (((seq >> 14) - 2) as u8, 0x8000 | (seq & 0x3fff) as u16) };
        let inst_bank = self.inst_bank.and_then(|i| data.banks.get(usize::from(i))).map_or(0, |b| b.addr as u16);
        b[0x00] = self.state;
        b[0x01] = bank;
        w(&mut b, 0x02, addr);
        b[0x04] = self.flags;
        w(&mut b, 0x05, self.wait);
        b[0x07] = self.gate_ratio;
        w(&mut b, 0x08, self.gate);
        b[0x0a] = self.transpose;
        b[0x0b] = self.detune;
        b[0x0c] = self.root;
        b[0x0d] = self.detune2;
        w(&mut b, 0x0e, self.start);
        b[0x10] = self.glide;
        b[0x11] = self.glide_rate;
        b[0x12] = (self.glide_step >> 8) as u8;
        b[0x13] = self.glide_step as u8;
        b[0x14] = (self.pitch >> 8) as u8;
        b[0x15] = self.pitch as u8;
        b[0x16] = (self.target >> 8) as u8;
        b[0x17] = self.target as u8;
        w(&mut b, 0x18, self.amplitude);
        w(&mut b, 0x1a, self.echo);
        b[0x1c..0x20].copy_from_slice(&self.loops);
        b[0x20] = self.dirty;
        b[0x25] = self.lfo;
        w(&mut b, 0x26, self.lfo_rate);
        w(&mut b, 0x28, self.lfo_phase);
        w(&mut b, 0x2a, self.vibrato);
        w(&mut b, 0x2c, self.tremolo);
        w(&mut b, 0x2e, self.instrument);
        w(&mut b, 0x30, inst_bank);
        b[0x33] = self.env_phase;
        w(&mut b, 0x34, self.env_level);
        w(&mut b, 0x36, self.env_attack);
        w(&mut b, 0x38, self.env_decay);
        w(&mut b, 0x3a, self.env_sustain);
        w(&mut b, 0x3c, self.env_slow);
        w(&mut b, 0x3e, self.env_release);
        b[0x40] = self.priority;
        w(&mut b, 0x41, self.step);
        b[0x43] = self.fresh;
        b[0x44] = self.continuous;
        let p = &self.pan;
        b[0x46] = p.mode.code();
        b[0x47] = p.pan;
        b[0x48] = p.frac;
        b[0x49] = p.delta;
        b[0x4a] = p.target;
        w(&mut b, 0x4b, p.speed);
        w(&mut b, 0x4d, p.speed2);
        b[0x4f] = self.delay;
        b
    }
}

impl AdpcmChannel {
    /// Bytes of the ADPCM voice record (0x18 bytes).
    pub fn image(&self) -> [u8; ADPCM_SIZE] {
        let mut b = [0u8; ADPCM_SIZE];
        let w = |b: &mut [u8], at: usize, v: u16| {
            b[at] = v as u8;
            b[at + 1] = (v >> 8) as u8;
        };
        b[0x00] = if self.fresh { 0xff } else { 0 };
        b[0x01] = if self.continuous { 0x80 } else { 0 };
        w(&mut b, 0x02, self.remaining);
        w(&mut b, 0x04, self.volume);
        b[0x06] = self.pan.mode.code();
        b[0x07] = self.pan.pan;
        b[0x08] = self.pan.frac;
        b[0x09] = self.pan.delta;
        b[0x0a] = self.pan.target;
        w(&mut b, 0x0b, self.pan.speed);
        w(&mut b, 0x0d, self.pan.speed2);
        b[0x0f] = self.delay;
        b[0x10] = self.priority;
        w(&mut b, 0x11, self.length);
        b[0x13] = self.bank;
        w(&mut b, 0x14, self.start);
        w(&mut b, 0x16, self.end);
        b
    }
}

impl Driver {
    /// Reconstructed F000-FFFF RAM of the original Z80 (bytes the driver does not model: 0).
    pub fn ram_image(&self) -> Vec<u8> {
        let mut r = vec![0u8; 0x1000];
        let g = &self.g;
        put(&mut r, 0xf000, self.irq_count);
        put(&mut r, 0xf001, self.pending.commands);
        put(&mut r, 0xf002, self.pending.ticks);
        put(&mut r, 0xf003, self.pending.adpcm);
        put(&mut r, 0xf004, if self.is_music { 0xff } else { 0 });
        put(&mut r, 0xf005, self.cur);
        put(&mut r, 0xf006, self.cur.wrapping_mul(8));
        put(&mut r, 0xf008, self.stereo);
        put16(&mut r, 0xf009, g.sfx_volume);
        put16(&mut r, 0xf00b, g.step);
        // Sound header, read at initialisation: number of sounds, address of the sound table.
        if self.boot >= Boot::WaitHandshake {
            let table = self.data.header.addr + 6;
            put16(&mut r, 0xf010, self.data.sounds.len() as u16);
            put(&mut r, 0xf012, (table >> 16) as u8);
            put16(&mut r, 0xf013, table as u16);
        }
        put(&mut r, 0xf015, self.queue_head);
        put(&mut r, 0xf016, self.queue_tail);
        let song = self.song.and_then(|n| self.data.sounds[usize::from(n)].as_ref()).map_or(0, |s| s.addr);
        put(&mut r, 0xf017, (song >> 16) as u8);
        put16(&mut r, 0xf018, song as u16);
        put(&mut r, 0xf01b, self.priority);
        put16(&mut r, 0xf020, g.music_step);
        put16(&mut r, 0xf022, g.tempo_adjust);
        put16(&mut r, 0xf024, g.music_factor);
        put16(&mut r, 0xf026, g.music_volume);
        put(&mut r, 0xf028, g.music_transpose);
        put16(&mut r, 0xf029, g.pitch_bend);
        put(&mut r, 0xf030, if self.sfx_override { 0xff } else { 0 });
        put(&mut r, 0xf040, g.fade_dir);
        put16(&mut r, 0xf041, g.fade_step);
        put16(&mut r, 0xf043, g.fade_level);
        put(&mut r, 0xf045, g.ramp_dir);
        put16(&mut r, 0xf046, g.ramp_step);
        put16(&mut r, 0xf048, g.ramp_level);
        put16(&mut r, 0xf04a, g.ramp_target);
        r[0x100..0x200].copy_from_slice(&self.queue);
        for (i, v) in self.sfx.iter().enumerate() {
            let at = SFX_VOICES - RAM_BASE + i * VOICE_SIZE;
            r[at..at + VOICE_SIZE].copy_from_slice(&v.image(&self.data));
        }
        for (i, v) in self.music.iter().enumerate() {
            let at = MUSIC_VOICES - RAM_BASE + i * VOICE_SIZE;
            r[at..at + VOICE_SIZE].copy_from_slice(&v.image(&self.data));
        }
        for (i, a) in self.adpcm.iter().enumerate() {
            let at = ADPCM_CHANNELS - RAM_BASE + i * ADPCM_SIZE;
            r[at..at + ADPCM_SIZE].copy_from_slice(&a.image());
        }
        r
    }
}
