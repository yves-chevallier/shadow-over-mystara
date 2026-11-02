//! Sequencer voices.
//!
//! There are two sets of 16 voices, one per QSound channel: sound effects (`sfx`) and music
//! (`music`). While an effect voice is active on a channel, the music voice of the same channel
//! keeps running but no longer writes to the QSound (`sfx_override`).
//!
//! A voice is in one of two states:
//! - `state == 1` : simple note (a sample or an instrument played for a duration);
//! - `state == 0xFF` : sequenced track, read byte by byte from the ROM (notes and instructions).
//!
//! On each tick (62.5 Hz) a voice advances its track, then updates pan, LFO, glissando and
//! envelope, and writes volume and pitch to the QSound.

use crate::math::*;
use crate::pan::PanFade;
use crate::{shared, Driver};

/// Track flags (`Voice::flags`).
pub mod flag {
    /// Index into the octave table: bits 0-2 octave 0-7, bit 3 two more octaves.
    pub const OCTAVE_MASK: u8 = 0x0f;
    pub const OCTAVE_INDEX: u8 = 0x07;
    pub const OCTAVE_UP: u8 = 0x08;
    /// The next note is dotted (duration x 1.5); cleared after use.
    pub const DOTTED: u8 = 0x10;
    /// Triplet (duration x 2/3).
    pub const TRIPLET: u8 = 0x20;
    /// The upcoming note is tied to the next one.
    pub const TIE: u8 = 0x40;
    /// The upcoming note follows a tied note: it only changes the pitch.
    pub const SLUR: u8 = 0x80;
    /// Bits kept by the "mode" instruction: low octave, dotted and slurred.
    pub const KEEP_ON_MODE: u8 = 0x97;
}

#[derive(Clone, Default)]
pub struct Voice {
    /// 0 free, 1 simple note, 0xFF sequenced track.
    pub state: u8,
    /// Read address in the track.
    pub seq: u32,
    /// Track flags: see [`flag`].
    pub flags: u8,
    /// Time left before the next event (8.8 fixed point).
    pub wait: u16,
    /// Fraction of the duration during which the note is held (x/256).
    pub gate_ratio: u8,
    /// Time left before the note is released.
    pub gate: u16,
    pub transpose: u8,
    pub detune: u8,
    /// Reference note of the sample (played at nominal pitch).
    pub root: u8,
    pub detune2: u8,
    /// Start address of the sample (rewritten on every trigger).
    pub start: u16,
    /// Glissando: 0 inactive, 1 finished this tick, 0xFF in progress.
    pub glide: u8,
    pub glide_rate: u8,
    pub glide_step: u16,
    /// Current pitch: high byte = semitone, low byte = fraction.
    pub pitch: u16,
    /// Target pitch.
    pub target: u16,
    /// Amplitude specific to the voice.
    pub amplitude: u16,
    /// Send level to the echo.
    pub echo: u16,
    /// Counters of the four nested loops.
    pub loops: [u8; 4],
    /// Bit 0: pitch must be rewritten. Bit 7: follows the global pitch variation.
    pub dirty: u8,
    /// Bit 0: LFO reset on every note. Bits 6-7: LFO shape.
    pub lfo: u8,
    pub lfo_rate: u16,
    pub lfo_phase: u16,
    pub vibrato: u16,
    pub tremolo: u16,
    /// Current instrument (bit 15: chosen by a track instruction) and instrument bank.
    pub instrument: u16,
    pub inst_bank: u16,
    /// Envelope phase: 0 attack, 1 decay, 2 sustain, 3 slow decay, 4 release, 5 end.
    pub env_phase: u8,
    pub env_level: u16,
    pub env_attack: u16,
    pub env_decay: u16,
    pub env_sustain: u16,
    pub env_slow: u16,
    pub env_release: u16,
    pub priority: u8,
    /// Voice-specific time step (effects).
    pub step: u16,
    /// Simple note that has just started.
    pub fresh: u8,
    /// Simple note played until explicitly stopped.
    pub continuous: u8,
    /// Panning and its evolution.
    pub pan: PanFade,
    /// Ticks to wait before starting (copy on the neighbouring channel, pan type 7).
    pub delay: u8,
}

impl Voice {
    /// New sequenced track starting at `seq`.
    pub(crate) fn track(seq: u32, inst_bank: u16) -> Self {
        Voice { state: 0xff, seq, inst_bank, instrument: 0xff00, lfo: 1, env_phase: 5, ..Voice::default() }
    }

    /// Reads the next byte of the track.
    pub(crate) fn fetch(&mut self, rom: &crate::Rom) -> u8 {
        let b = rom.byte(self.seq);
        self.seq = (self.seq + 1) & 0xff_ffff;
        b
    }

    /// Reads a big-endian word.
    pub(crate) fn fetch_be16(&mut self, rom: &crate::Rom) -> u16 {
        let h = self.fetch(rom);
        let l = self.fetch(rom);
        u16::from(h) << 8 | u16::from(l)
    }
}

impl Driver {
    // ---------------------------------------------------------------- QSound

    /// Silences a channel: no more echo, volume and frequency at zero.
    pub(crate) fn silence_voice(&mut self, ch: u8) {
        let base = ch.wrapping_mul(8);
        self.qsound(0xba_u8.wrapping_add(ch), 0);
        self.qsound(base.wrapping_add(6), 0);
        self.qsound(base.wrapping_add(2), 0);
        self.qsound(base.wrapping_add(2), 0x1000);
    }

    /// Writes a pan register (0x80 + channel, 0x90 + ADPCM channel); in mono the pan is always
    /// centred. The Z80 also copies the value to 0xC005 (report to the 68000).
    pub(crate) fn write_pan(&mut self, reg: u8, pan: u8, report: bool) {
        let pan = if self.stereo == 0 { 0x10 } else { pan };
        if report {
            self.write_shared(shared::PAN_REPORT, pan);
        }
        self.qsound(reg, PAN_BASE + u16::from(pan));
    }

    /// Sets the pan of a starting voice, then mutes its channel.
    pub(crate) fn pan_and_mute(&mut self, v: &Voice, ch: u8) {
        self.write_pan(0x80 + ch, v.pan.pan, true);
        self.silence_voice(ch);
    }

    /// Loads a sample into the current channel (addresses, loop, bank).
    /// Table entries are 8 bytes: bank, start, loop start, end, reference note.
    pub(crate) fn start_sample(&mut self, v: &mut Voice, id: u16) {
        let base = self.cur.wrapping_mul(8);
        self.qsound(base.wrapping_add(2), 0);
        let sample = self.rom.sample(id);
        let (bank, loop_start, end) = (sample.bank, sample.loop_start, sample.end);
        v.start = sample.start;
        v.root = sample.root;
        self.qsound(base.wrapping_add(1), v.start);
        self.qsound(base.wrapping_add(5), end);
        self.qsound(base.wrapping_add(4), end.wrapping_sub(loop_start));
        // A channel's bank is set in register 0 of the previous channel.
        let bank_reg = self.cur.wrapping_sub(1).rotate_left(3) & 0x78;
        self.qsound(bank_reg, 0x8000 | u16::from(bank));
    }

    /// Triggers the note: starts sample playback, then volume and pitch.
    pub(crate) fn key_on(&mut self, v: &mut Voice) {
        let base = self.cur.wrapping_mul(8);
        self.qsound(base.wrapping_add(2), 0);
        self.qsound(0x93, 0x27ff);
        self.qsound(0xd9, 0x07ff);
        self.qsound(0xba_u8.wrapping_add(self.cur), v.echo);
        self.qsound(base.wrapping_add(1), v.start);
        self.qsound(base.wrapping_add(3), 0x8000);
        self.write_volume_pitch(v);
    }

    /// QSound playback frequency for a pitch (semitone.fraction) and the reference note.
    fn pitch_rate(&self, pitch: u16, root: u8) -> u16 {
        let note = ((pitch >> 8) as u8).wrapping_sub(1);
        let a = 0x3c_u8.wrapping_sub(root).wrapping_add(note);
        // Original clamping: beyond 0x6B, 0x6B if the result of `a - 0x6C` has its
        // sign bit set, otherwise 0 (so the original program yields 0 for 0x6C-0xEB).
        let n = if a < 0x6c {
            a
        } else if a.wrapping_sub(0x6c) & 0x80 != 0 {
            0x6b
        } else {
            0
        };
        let octave = self.rom.semitone_octave(n);
        let semitone = n.wrapping_sub(self.rom.octave_start(octave));
        let fine = u16::from(self.rom.fine(semitone, pitch as u8)) << octave;
        self.rom.rate(n).wrapping_add(fine)
    }

    /// Writes volume and frequency of the current voice.
    pub(crate) fn write_volume_pitch(&mut self, v: &mut Voice) {
        let base = self.cur.wrapping_mul(8);
        // Volume: base (effects or music), fade, envelope, tremolo, voice amplitude.
        let mut vol = self.g.sfx_volume;
        if self.is_music {
            vol = self.g.music_volume;
            if self.g.ramp_dir != 0 {
                vol = hi16(vol, self.g.ramp_level);
            }
        }
        if self.g.fade_dir != 0 {
            vol = hi16(vol, self.g.fade_level);
        }
        vol = hi16(vol, v.env_level);
        if v.tremolo != 0 {
            let depth = hi16(self.lfo_value(v), v.tremolo);
            vol = hi16(0xffff - depth, vol);
        }
        vol = hi16(vol, v.amplitude);
        self.qsound(base.wrapping_add(6), vol);
        // Pitch: only if it changed, or if an LFO / glissando moves it.
        let mut pitch = v.pitch;
        if v.vibrato == 0 {
            if v.dirty & 1 == 0 && v.glide == 0 {
                return;
            }
        } else {
            let mut shift = hi16(self.lfo_value(v), v.vibrato);
            if v.lfo & 0x80 != 0 {
                shift = 0u16.wrapping_sub(shift);
            }
            pitch = pitch.wrapping_add(shift);
        }
        if v.dirty & 0x80 != 0 {
            pitch = pitch.wrapping_add(self.g.pitch_bend);
        }
        let rate = self.pitch_rate(pitch, v.root);
        self.qsound(base.wrapping_add(2), rate);
        v.dirty &= !1;
    }

    /// LFO value (triangle depending on bit 6: rising or falling).
    fn lfo_value(&self, v: &Voice) -> u16 {
        if v.lfo & 0x40 != 0 {
            0xffff - v.lfo_phase
        } else {
            v.lfo_phase
        }
    }

    // ------------------------------------------------------------- evolution

    /// Pan, LFO, glissando and envelope of a voice, then write to the QSound.
    pub(crate) fn update_voice(&mut self, v: &mut Voice) {
        if v.pan.step() {
            self.write_pan(0x80 + self.cur, v.pan.pan, true);
        }
        // LFO: the phase advances, an overflow flips the direction of the triangle (bit 6).
        let (phase, carry) = v.lfo_phase.overflowing_add(v.lfo_rate);
        v.lfo_phase = phase;
        if carry {
            v.lfo = v.lfo.wrapping_add(0x40);
        }
        self.update_glide(v);
        self.update_envelope(v);
    }

    /// Glissando: the pitch approaches the target pitch in steps of `glide_step`.
    fn update_glide(&mut self, v: &mut Voice) {
        match v.glide {
            0 => {}
            1 => v.glide = 0,
            _ => {
                let new = v.pitch.wrapping_add(v.glide_step);
                let negative = v.glide_step & 0x8000 != 0;
                let (diff, borrow) = new.overflowing_sub(v.target);
                // Reached (or exceeded) depending on the glissando direction.
                let still_moving = diff != 0 && if negative { !borrow } else { borrow };
                if still_moving {
                    v.pitch = new;
                } else {
                    v.pitch = v.target;
                    v.glide = 1;
                }
            }
        }
    }

    /// Volume envelope, then write to the QSound if the channel is free.
    fn update_envelope(&mut self, v: &mut Voice) {
        let level = v.env_level;
        let new = match v.env_phase {
            // Attack: rises to the maximum, then decays.
            0 => match level.overflowing_add(v.env_attack) {
                (l, false) => l,
                _ => {
                    v.env_phase = 1;
                    0xffff
                }
            },
            // Decay down to the sustain level; with no sustain level the note stops.
            1 => {
                let (l, borrow) = level.overflowing_sub(v.env_decay);
                if !borrow && v.env_sustain.overflowing_sub(l).1 {
                    l
                } else if v.env_sustain == 0 {
                    v.env_phase = 5;
                    0
                } else {
                    v.env_phase = 3;
                    v.env_sustain
                }
            }
            // Slow decay then release: down to zero.
            3 | 4 => {
                let rate = if v.env_phase == 3 { v.env_slow } else { v.env_release };
                let (l, borrow) = level.overflowing_sub(rate);
                if l != 0 && !borrow {
                    l
                } else {
                    v.env_phase = 5;
                    0
                }
            }
            // Sustain or end: nothing to write.
            _ => {
                v.dirty &= !1;
                return;
            }
        };
        v.env_level = new;
        if self.sfx_override {
            v.dirty &= !1;
        } else {
            self.write_volume_pitch(v);
        }
    }

    // ---------------------------------------------------- sequencer tick

    /// One tick (62.5 Hz): all voices, then the global fades.
    pub(crate) fn sequencer_tick(&mut self) {
        for ch in 0..16u8 {
            self.cur = ch;
            self.is_music = false;
            self.sfx_override = false;
            let mut v = std::mem::take(&mut self.sfx[usize::from(ch)]);
            if v.state != 0 {
                if v.delay != 0 {
                    v.delay -= 1;
                    if v.delay == 0 {
                        self.pan_and_mute(&v, ch);
                        self.sfx_override = true;
                    }
                } else {
                    self.g.step = v.step;
                    self.update_sfx(&mut v);
                    self.sfx_override = v.state != 0;
                }
            }
            self.sfx[usize::from(ch)] = v;
            let mut m = std::mem::take(&mut self.music[usize::from(ch)]);
            if m.state != 0 {
                self.is_music = true;
                self.g.step = self.g.music_step.wrapping_add(self.g.tempo_adjust);
                if self.g.pitch_bend != 0 {
                    m.dirty |= 1;
                }
                self.update_sequenced(&mut m);
            }
            self.music[usize::from(ch)] = m;
            if self.hung {
                return;
            }
        }
        self.update_master_fade();
        self.update_music_ramp();
    }

    /// Global fade (commands 12 and 13): the level moves one step per tick.
    fn update_master_fade(&mut self) {
        let g = &mut self.g;
        if g.fade_dir == 0 {
            return;
        }
        let mut carry_in = if g.fade_dir & 0x80 != 0 { g.fade_dir } else { g.fade_dir - 1 };
        let (level, carry) = g.fade_level.overflowing_add(g.fade_step);
        carry_in = carry_in.wrapping_add(carry as u8);
        if carry_in == 0 {
            g.fade_level = level;
        } else {
            // Overflow at the top: fade finished; at the bottom: level zero.
            if carry_in & 0x80 == 0 {
                g.fade_dir = 0;
            }
            g.fade_level = 0;
        }
    }

    /// Music volume ramp (commands 6 and 7).
    fn update_music_ramp(&mut self) {
        let g = &mut self.g;
        if g.ramp_dir == 0 {
            return;
        }
        let base = if g.ramp_dir & 0x80 != 0 { g.ramp_dir } else { g.ramp_dir - 1 };
        let (level, carry) = g.ramp_level.overflowing_add(g.ramp_step);
        let mut out_of_range = base.wrapping_add(carry as u8) != 0;
        if !out_of_range {
            // Within range: has the target been reached?
            let reached = base.wrapping_add(level.overflowing_sub(g.ramp_target).1 as u8) == 0;
            if reached {
                out_of_range = true;
            } else {
                g.ramp_level = level;
            }
        }
        if out_of_range {
            if g.ramp_target == 0xfffe {
                g.ramp_dir = 0;
            }
            g.ramp_level = g.ramp_target;
        }
    }
}
