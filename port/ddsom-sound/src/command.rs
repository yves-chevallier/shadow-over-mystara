//! 68000 commands: queue, starting of music and effects, system commands.
//!
//! A command is 16 bytes long. If byte 0 is 0xFF it is a system command whose byte 1 is the
//! number. Otherwise bytes 0-1 (big-endian) are the number of a song or effect, and bytes 6
//! onwards set the pan (see `setup_pan`).

use crate::math::*;
use crate::pan::PanFade;
use crate::voice::Voice;
use crate::Driver;

impl Driver {
    /// Takes the next command from the queue and executes it.
    pub(crate) fn process_command(&mut self) {
        if self.queue_tail == self.queue_head {
            return;
        }
        let at = usize::from(self.queue_tail);
        let mut cmd = [0u8; 16];
        cmd.copy_from_slice(&self.queue[at..at + 16]);
        self.queue_tail = self.queue_tail.wrapping_add(16);
        if cmd[0] == 0xff {
            return self.system_command(&cmd);
        }
        // Song or effect number (modulo their count) -> 4-byte entry: 3-byte address of the
        // definition.
        let number = (u16::from(cmd[0]) << 8 | u16::from(cmd[1])) % self.song_count.max(1);
        let entry = self.song_table.wrapping_add(u32::from(number.wrapping_mul(4))) & 0xff_ffff;
        let song = self.rom.sound(entry);
        if song == 0 {
            return;
        }
        self.song = song;
        let kind = self.rom.byte(song);
        let body = song + 1;
        match kind {
            0 => self.start_music(body),
            1..=0x7f => self.start_effect(kind, body, &cmd),
            _ => self.start_single(kind & 0x7f, body, &cmd),
        }
    }

    /// Song: 16 tracks (one per channel), each designated by a 2-byte offset
    /// from the start of the definition (0 = channel absent).
    fn start_music(&mut self, mut p: u32) {
        for ch in 0..16u8 {
            self.cur = ch;
            let mut m = std::mem::take(&mut self.music[usize::from(ch)]);
            let was_active = m.state != 0;
            self.priority = m.state;
            m.state = 0;
            let offset = u32::from(self.rom.word_be(p));
            p += 2;
            if offset != 0 {
                m = Voice::track(self.song + offset, self.inst_bank0());
                m.pan.pan = 0x10;
            }
            self.music[usize::from(ch)] = m;
            // Channel free on the effects side: centred pan, and silenced if a song was playing on it.
            if self.sfx[usize::from(ch)].state == 0 {
                self.qsound(0x80 + ch, PAN_BASE + 0x10);
                if was_active {
                    self.silence_voice(ch);
                }
            }
        }
        let g = &mut self.g;
        g.music_step = 0;
        g.tempo_adjust = 0;
        g.music_transpose = 0;
        g.pitch_bend = 0;
        g.ramp_dir = 0;
        g.music_volume = hi16(hi16(0xfffe, g.sfx_volume), g.music_factor);
    }

    /// Default instrument bank of the tracks.
    fn inst_bank0(&self) -> u16 {
        self.rom.instrument_bank(0)
    }

    /// Multi-track effect: same format as a song, but a track only starts if the command
    /// priority is at least that of the voice it replaces.
    fn start_effect(&mut self, priority: u8, mut p: u32, cmd: &[u8; 16]) {
        self.priority = priority;
        for ch in 0..16u8 {
            self.cur = ch;
            let offset = u32::from(self.rom.word_be(p));
            p += 2;
            if offset == 0 || priority < self.sfx[usize::from(ch)].priority {
                continue;
            }
            let mut v = Voice::track(self.song + offset, self.inst_bank0());
            v.priority = priority;
            self.setup_pan(&mut v, cmd);
            self.sfx[usize::from(ch)] = v;
        }
    }

    /// Single-voice effect (simple note of a sample or an instrument); voices
    /// 0x10 and above are the ADPCM voices.
    fn start_single(&mut self, channel: u8, mut p: u32, cmd: &[u8; 16]) {
        if channel >= 0x10 {
            return self.start_adpcm(channel - 0x10, p, cmd);
        }
        self.cur = channel;
        let flags = self.rom.byte(p);
        p += 1;
        self.priority = flags;
        let priority = flags & 0x7f;
        let slot = usize::from(channel);
        if priority < self.sfx[slot].priority {
            return;
        }
        let duration = self.rom.word_be(p);
        p += 2;
        if duration == 0 {
            return;
        }
        let note = u16::from(self.rom.byte(p + 5).wrapping_add(1)) << 8;
        let mut v = Voice {
            state: 1,
            fresh: 0xff,
            wait: duration,
            gate: self.rom.word_be(p),
            amplitude: self.rom.amplitude(self.rom.byte(p + 2)),
            instrument: u16::from(self.rom.byte(p + 3)) << 8 | u16::from(self.rom.byte(p + 4)),
            pitch: clamp_pitch(note.wrapping_add(sext(self.rom.byte(p + 6)))),
            priority,
            continuous: flags & 0x80,
            ..Voice::default()
        };
        // Once the voice is accepted, the Z80 keeps only the priority (without the "continuous" bit).
        self.priority = priority;
        self.setup_pan(&mut v, cmd);
        self.sfx[slot] = v;
    }

    /// Sets the pan of a starting voice from the command, then silences the channel (the voice
    /// starts on the next tick). For type 7 the neighbouring channel receives a copy of the voice
    /// moving in the other direction, after a 4-tick delay, if its priority allows it.
    pub(crate) fn setup_pan(&mut self, v: &mut Voice, cmd: &[u8; 16]) {
        let Some(fade) = PanFade::from_command(cmd) else { return };
        v.pan = fade;
        if cmd[6] == 7 {
            let next = usize::from((self.cur + 1) & 0xf);
            if self.priority >= self.sfx[next].priority {
                let mut copy = v.clone();
                copy.pan = fade.mirrored(cmd);
                copy.delay = 4;
                self.sfx[next] = copy;
            }
        }
        self.pan_and_mute(v, self.cur);
    }
}

impl Driver {
    /// System commands (byte 0 = 0xFF, number in byte 1, parameters from byte 2 on).
    fn system_command(&mut self, cmd: &[u8; 16]) {
        match cmd[1] {
            0 => self.stop_all(),
            1 => self.stop_all_effects(),
            2 => self.stop_effect(cmd[4]),
            3 => self.stop_all_adpcm(),
            4 => self.stop_adpcm(cmd[4]),
            5 => self.stop_music(),
            6 => self.music_ramp(cmd, 1),
            7 => self.music_ramp(cmd, 0xff),
            8 => self.set_tempo(cmd),
            9 => self.set_pitch_bend(cmd),
            12 => self.master_fade(cmd, 1),
            13 => self.master_fade(cmd, 0xff),
            _ => {}
        }
    }

    /// Silences everything: effect, music and ADPCM voices.
    fn stop_all(&mut self) {
        self.g.fade_dir = 0;
        for i in 0..32usize {
            let ch = (i & 0xf) as u8;
            let v = if i < 16 { &mut self.sfx[i] } else { &mut self.music[i - 16] };
            let was_active = v.state != 0;
            v.priority = 0;
            v.state = 0;
            if was_active {
                self.silence_voice(ch);
            }
        }
        self.stop_all_adpcm();
    }

    fn stop_all_effects(&mut self) {
        for ch in 0..16u8 {
            self.cur = ch;
            self.stop_effect_voice(ch);
        }
    }

    fn stop_effect(&mut self, ch: u8) {
        self.cur = ch;
        if ch < 16 {
            self.stop_effect_voice(ch);
        }
    }

    fn stop_effect_voice(&mut self, ch: u8) {
        let mut v = std::mem::take(&mut self.sfx[usize::from(ch)]);
        if v.state != 0 {
            self.end_sfx(&mut v);
        }
        self.sfx[usize::from(ch)] = v;
    }

    /// Stops the music; channels occupied by an effect keep sounding.
    fn stop_music(&mut self) {
        for ch in 0..16u8 {
            if self.music[usize::from(ch)].state != 0 {
                self.music[usize::from(ch)].state = 0;
                if self.sfx[usize::from(ch)].state == 0 {
                    self.silence_voice(ch);
                }
            }
        }
    }

    /// Music volume ramp: byte 2 = target level (index into the level table),
    /// bytes 3-4 = speed. `dir`: 1 rises from 0, 0xFF falls from the maximum.
    fn music_ramp(&mut self, cmd: &[u8; 16], dir: u8) {
        if self.g.ramp_dir == 0 {
            self.g.ramp_level = if dir == 1 { 0 } else { 0xffff };
        }
        self.g.ramp_dir = dir;
        let step = u16::from(cmd[3]) << 8 | u16::from(cmd[4]);
        self.g.ramp_step = if dir == 1 { step } else { 0u16.wrapping_sub(step) };
        self.g.ramp_target = self.rom.level(cmd[2] >> 1);
    }

    /// Global fade of all channels: bytes 3-4 = speed.
    fn master_fade(&mut self, cmd: &[u8; 16], dir: u8) {
        if self.g.fade_dir == 0 {
            self.g.fade_level = if dir == 1 { 0 } else { 0xffff };
        }
        self.g.fade_dir = dir;
        let step = u16::from(cmd[3]) << 8 | u16::from(cmd[4]);
        self.g.fade_step = if dir == 1 { step } else { 0u16.wrapping_sub(step) };
    }

    /// Music tempo adjustment: byte 4 = value, byte 3 = 0xFF if negative.
    fn set_tempo(&mut self, cmd: &[u8; 16]) {
        let value = ((u32::from(cmd[4]) * 0x369) >> 8) as u16;
        self.g.tempo_adjust = if cmd[3] == 0xff { 0u16.wrapping_sub(value) } else { value };
    }

    /// Pitch variation of the music voices: byte 4 = value, byte 3 = 0xFF if negative.
    fn set_pitch_bend(&mut self, cmd: &[u8; 16]) {
        let value = u16::from(cmd[4]) << 4;
        self.g.pitch_bend = if cmd[3] == 0xff { 0u16.wrapping_sub(value) } else { value };
    }
}

/// Clamps the pitch to 0x6B semitones.
fn clamp_pitch(pitch: u16) -> u16 {
    if (pitch >> 8) as u8 >= 0x6c {
        0x6b00 | (pitch & 0xff)
    } else {
        pitch
    }
}
