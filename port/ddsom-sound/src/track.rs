//! Lecture des pistes : notes simples, évènements et instructions des pistes séquencées.

use crate::math::*;
use crate::voice::{flag, Voice};
use crate::Driver;

/// Instructions de piste (octets < 0x20) ; les octets >= 0x20 sont des notes. Entre parenthèses :
/// les paramètres lus dans la piste.
mod op {
    /// Bascule triolet / durée normale.
    pub const TOGGLE_TRIPLET: u8 = 0;
    /// Bascule « lier à la note suivante ».
    pub const TOGGLE_TIE: u8 = 1;
    /// La prochaine note est pointée.
    pub const DOTTED_NEXT: u8 = 2;
    /// Bascule « deux octaves plus haut ».
    pub const TOGGLE_OCTAVE_UP: u8 = 3;
    /// Remplace les drapeaux 6, 5 et 3 (1 octet).
    pub const SET_MODE: u8 = 4;
    /// Pas de temps par tic : celui de la voie, ou celui de toute la musique (2 octets).
    pub const SET_STEP: u8 = 5;
    /// Proportion de la durée où la note est tenue (1 octet).
    pub const SET_GATE: u8 = 6;
    /// Amplitude de la voie, index de table (1 octet).
    pub const SET_AMPLITUDE: u8 = 7;
    /// Instrument, index dans le banc (1 octet).
    pub const SET_INSTRUMENT: u8 = 8;
    /// Octave, 0-7 (1 octet).
    pub const SET_OCTAVE: u8 = 9;
    /// Transposition de toute la musique, en demi-tons (1 octet).
    pub const SET_MUSIC_TRANSPOSE: u8 = 10;
    /// Transposition de la voie, en demi-tons (1 octet).
    pub const SET_TRANSPOSE: u8 = 11;
    /// Accord fin de la voie (1 octet).
    pub const SET_DETUNE: u8 = 12;
    /// Vitesse du glissando, 0 = aucun (1 octet).
    pub const SET_GLIDE: u8 = 13;
    /// Début de boucle (compteur 0-3) : nombre de tours (1 octet), saut en arrière (2 octets).
    pub const LOOP_BEGIN: std::ops::RangeInclusive<u8> = 14..=17;
    /// Sortie de boucle au dernier tour : drapeaux (1 octet), saut en avant (2 octets).
    pub const LOOP_EXIT: std::ops::RangeInclusive<u8> = 18..=21;
    /// Saut relatif (2 octets).
    pub const JUMP: u8 = 22;
    /// Fin de la piste.
    pub const END: u8 = 23;
    /// Pan de la voie de musique, 0-0x20 (1 octet).
    pub const SET_PAN: u8 = 24;
    /// Niveau d'envoi vers l'écho, index de table (1 octet).
    pub const SET_ECHO: u8 = 25;
    /// Volume de la musique, index de table (1 octet).
    pub const SET_MUSIC_VOLUME: u8 = 26;
    /// Profondeur de vibrato, index de table (1 octet).
    pub const SET_VIBRATO: u8 = 27;
    /// Profondeur de trémolo, index de table (1 octet).
    pub const SET_TREMOLO: u8 = 28;
    /// Vitesse du LFO, index de table (1 octet).
    pub const SET_LFO_RATE: u8 = 29;
    /// Mode du LFO (1 octet).
    pub const SET_LFO_MODE: u8 = 30;
    /// Banc d'instruments, index de table (1 octet).
    pub const SET_INSTRUMENT_BANK: u8 = 31;
}

/// Nombre d'instructions de piste consécutives au-delà duquel on considère le séquenceur bloqué.
const MAX_INSTRUCTIONS: u32 = 100_000;

impl Driver {
    // -------------------------------------------------------- notes simples

    /// Tic d'une voie d'effet : note simple, ou piste séquencée.
    pub(crate) fn update_sfx(&mut self, v: &mut Voice) {
        if v.state & 0x80 != 0 {
            return self.update_sequenced(v);
        }
        if v.fresh != 0 {
            return self.start_simple_note(v);
        }
        if v.continuous == 0 {
            // Le relâchement a lieu à la fin du temps de tenue (instruments à enveloppe).
            if v.instrument & 0x8000 != 0 && v.gate != 0 {
                v.gate -= 1;
                if v.gate == 0 && v.env_phase != 5 {
                    v.env_phase = 4;
                }
            }
            v.wait = v.wait.wrapping_sub(1);
            if v.wait == 0 {
                return self.end_sfx(v);
            }
        }
        self.update_voice(v);
    }

    /// Premier tic d'une note simple : instrument, échantillon, déclenchement.
    fn start_simple_note(&mut self, v: &mut Voice) {
        v.fresh = 0;
        let mut level = 0xffff;
        let mut sample = v.instrument;
        if sample & 0x8000 != 0 {
            // Instrument : fiche de 8 octets dans le banc choisi par l'octet haut.
            let bank = self.rom.table(INSTRUMENT_BANKS_PTR, (sample >> 8) as u8 & 0x7f);
            let p = u32::from(bank) + (sample & 0xff) as u32 * 8;
            sample = u16::from(self.rom.byte(p + 1) & 0x7f) << 8 | u16::from(self.rom.byte(p));
            v.pitch = clamp_note(v.pitch.wrapping_add(sext(self.rom.byte(p + 2))));
            self.load_envelope(v, p + 3);
            level = v.env_attack;
        }
        v.env_level = level;
        v.env_phase = 0;
        self.start_sample(v, sample);
        v.dirty |= 1;
        self.key_on(v);
    }

    /// Lit les cinq paramètres d'enveloppe d'une fiche d'instrument (indices dans les tables).
    fn load_envelope(&self, v: &mut Voice, p: u32) {
        let rom = &self.rom;
        v.env_attack = rom.table(TABLE_ATTACK, rom.byte(p));
        v.env_decay = rom.table(TABLE_DECAY, rom.byte(p + 1));
        v.env_sustain = rom.table(TABLE_LEVELS, rom.byte(p + 2));
        v.env_slow = rom.table(TABLE_DECAY, rom.byte(p + 3));
        v.env_release = rom.table(TABLE_DECAY, rom.byte(p + 4));
    }

    /// Fin d'une voie d'effet : le canal est coupé, et la musique du même canal reprend.
    pub(crate) fn end_sfx(&mut self, v: &mut Voice) {
        let ch = self.cur;
        v.priority = 0;
        v.state = 0;
        self.silence_voice(ch);
        let mut m = std::mem::take(&mut self.music[usize::from(ch)]);
        if m.state != 0 {
            let instrument = m.instrument;
            self.start_sample(&mut m, instrument);
            self.write_pan(0x80 + ch, m.pan.pan, false);
            if m.flags & flag::SLUR != 0 {
                self.key_on(&mut m);
            } else {
                m.env_phase = 5;
            }
        }
        self.music[usize::from(ch)] = m;
    }

    // ----------------------------------------------------- pistes séquencées

    /// Tic d'une voie séquencée : temps, puis prochain évènement s'il est dû.
    pub(crate) fn update_sequenced(&mut self, v: &mut Voice) {
        if v.wait != 0 {
            let dt = self.g.step;
            if v.gate != 0 {
                let (rest, borrow) = v.gate.overflowing_sub(dt);
                if rest == 0 || borrow {
                    if v.env_phase != 5 {
                        v.env_phase = 4;
                    }
                    v.gate = 0;
                } else {
                    v.gate = rest;
                }
            }
            let (rest, borrow) = v.wait.overflowing_sub(dt);
            v.wait = rest;
            if rest != 0 && !borrow && rest >> 8 < 0xc1 {
                return self.update_voice(v);
            }
        }
        self.play_event(v);
    }

    /// Prochain évènement de la piste : instructions, puis une note ou un silence.
    fn play_event(&mut self, v: &mut Voice) {
        let first = v.fetch(&self.rom);
        let event = if first < 0x20 {
            match self.run_instructions(v, first) {
                Some(e) => e,
                None => return,
            }
        } else {
            first
        };
        // Durée : table selon le mode (normal, pointé, triolet) et la valeur de la note.
        let mode = v.flags & (flag::DOTTED | flag::TRIPLET);
        v.flags &= !flag::DOTTED;
        let duration = self.rom.word(TABLE_DURATION + u32::from(((event >> 4) & 0x0e) | mode));
        v.wait = v.wait.wrapping_add(duration);
        let note = event & 0x1f;
        if note == 0 {
            return;
        }
        let note = note.wrapping_add(self.rom.byte(TABLE_OCTAVE_BASE + u32::from(v.flags & flag::OCTAVE_MASK)));
        let transpose = if self.is_music { self.g.music_transpose } else { 0 };
        let note = transpose.wrapping_add(v.transpose).wrapping_add(note);
        let detune = sext(v.detune.wrapping_add(v.detune2));
        let target = clamp_note((u16::from(note) << 8).wrapping_add(detune));
        // Glissando depuis la note précédente si la voie en demande un.
        v.glide = 0;
        let mut start = target;
        let previous = v.target;
        if v.glide_rate != 0 && previous >> 8 != 0 && previous >> 8 != target >> 8 {
            let rate = u16::from(v.glide_rate);
            v.glide_step = if previous >> 8 < target >> 8 { rate } else { 0u16.wrapping_sub(rate) };
            start = previous;
            v.glide = 0xff;
        }
        v.target = target;
        v.pitch = start;
        // Une note liée (TIE) fait de la suivante une note « coulée » (SLUR) : elle ne reprend ni
        // l'attaque ni l'échantillon.
        let flags = v.flags;
        v.flags = flags & !flag::SLUR | (flags << 1) & flag::SLUR;
        if flags & flag::TIE == 0 {
            // Temps de tenue proportionnel à la durée.
            v.gate = hi16(v.wait, u16::from(v.gate_ratio) << 8);
        }
        v.dirty |= 1;
        if flags & flag::SLUR != 0 {
            return self.write_volume_pitch_if_free(v);
        }
        if v.lfo & 1 != 0 {
            v.lfo &= 0x3f;
            v.lfo_phase = 0;
        }
        v.env_level = v.env_attack;
        v.env_phase = 0;
        if self.sfx_override {
            v.dirty &= !1;
            return;
        }
        self.key_on(v);
    }

    pub(crate) fn write_volume_pitch_if_free(&mut self, v: &mut Voice) {
        if self.sfx_override {
            v.dirty &= !1;
        } else {
            self.write_volume_pitch(v);
        }
    }

    /// Exécute les instructions jusqu'à la prochaine note (renvoyée), ou la fin de la piste.
    fn run_instructions(&mut self, v: &mut Voice, first: u8) -> Option<u8> {
        let mut instr = first;
        // Garde-fou : le pilote d'origine ne sort jamais d'une boucle d'instructions sans note.
        for _ in 0..MAX_INSTRUCTIONS {
            let rom = &self.rom;
            match instr {
                op::TOGGLE_TRIPLET => v.flags ^= flag::TRIPLET,
                op::TOGGLE_TIE => v.flags ^= flag::TIE,
                op::DOTTED_NEXT => v.flags |= flag::DOTTED,
                op::TOGGLE_OCTAVE_UP => v.flags ^= flag::OCTAVE_UP,
                op::SET_MODE => v.flags = v.flags & flag::KEEP_ON_MODE | v.fetch(rom),
                op::SET_STEP => {
                    let step = v.fetch_be16(rom);
                    if self.is_music {
                        self.g.music_step = step;
                    } else {
                        v.step = step;
                    }
                }
                op::SET_GATE => v.gate_ratio = v.fetch(rom),
                op::SET_AMPLITUDE => v.amplitude = rom.table(TABLE_AMPLITUDE, v.fetch(rom)),
                op::SET_INSTRUMENT => self.select_instrument(v),
                op::SET_OCTAVE => v.flags = v.flags & !flag::OCTAVE_INDEX | v.fetch(rom),
                op::SET_MUSIC_TRANSPOSE => {
                    let t = v.fetch(rom);
                    if self.is_music {
                        self.g.music_transpose = t;
                    }
                }
                op::SET_TRANSPOSE => v.transpose = v.fetch(rom),
                op::SET_DETUNE => v.detune = v.fetch(rom),
                op::SET_GLIDE => v.glide_rate = v.fetch(rom).wrapping_mul(2),
                n if op::LOOP_BEGIN.contains(&n) => {
                    let k = usize::from(n - 14);
                    let count = v.fetch(rom);
                    if v.loops[k] == 0 {
                        v.loops[k] = count;
                        self.jump(v);
                    } else {
                        v.loops[k] -= 1;
                        if v.loops[k] != 0 {
                            self.jump(v);
                        } else {
                            v.seq += 2;
                        }
                    }
                }
                n if op::LOOP_EXIT.contains(&n) => {
                    let k = usize::from(n - 18);
                    if v.loops[k] == 1 {
                        v.loops[k] = 0;
                        v.flags = v.flags & flag::KEEP_ON_MODE | v.fetch(rom);
                        self.jump(v);
                    } else {
                        v.seq += 3;
                    }
                }
                op::JUMP => self.jump(v),
                op::END => {
                    if self.is_music {
                        v.state = 0;
                        if !self.sfx_override {
                            self.silence_voice(self.cur);
                        }
                    } else {
                        self.end_sfx(v);
                    }
                    return None;
                }
                op::SET_PAN => {
                    let pan = v.fetch(rom);
                    if self.is_music {
                        v.pan.pan = pan;
                        if !self.sfx_override {
                            self.write_pan(0x80 + self.cur, pan, false);
                        }
                    }
                }
                op::SET_ECHO => v.echo = rom.table(TABLE_AMPLITUDE, v.fetch(rom)) << 2,
                op::SET_MUSIC_VOLUME => {
                    let level = rom.table(TABLE_LEVELS, v.fetch(rom));
                    if self.is_music {
                        self.g.music_volume = hi16(hi16(self.g.sfx_volume, level), self.g.music_factor);
                    }
                }
                op::SET_VIBRATO => v.vibrato = rom.table(TABLE_VIBRATO, v.fetch(rom)),
                op::SET_TREMOLO => v.tremolo = rom.table(TABLE_TREMOLO, v.fetch(rom)),
                op::SET_LFO_RATE => v.lfo_rate = rom.table(TABLE_LFO_RATE, v.fetch(rom)),
                op::SET_LFO_MODE => v.lfo = v.lfo & !1 | v.fetch(rom),
                op::SET_INSTRUMENT_BANK => v.inst_bank = rom.table(INSTRUMENT_BANKS_PTR, v.fetch(rom)),
                _ => unreachable!("instruction {instr}"),
            }
            instr = v.fetch(&self.rom);
            if instr >= 0x20 {
                return Some(instr);
            }
        }
        self.hung = true;
        None
    }

    /// Saut relatif : le déplacement (mot signé grand-boutiste) part de l'adresse qui le suit.
    fn jump(&self, v: &mut Voice) {
        let offset = v.fetch_be16(&self.rom) as i16;
        v.seq = v.seq.wrapping_add(i32::from(offset) as u32) & 0xff_ffff;
    }

    /// Instruction « instrument » : fiche de 8 octets (échantillon, détune, enveloppe).
    fn select_instrument(&mut self, v: &mut Voice) {
        let index = v.fetch(&self.rom);
        let p = u32::from(v.inst_bank) + u32::from(index) * 8;
        let rom = &self.rom;
        let mut sample = u16::from(rom.byte(p + 1)) << 8 | u16::from(rom.byte(p));
        v.detune2 = rom.byte(p + 2);
        self.load_envelope(v, p + 3);
        if sample & 0x8000 != 0 {
            sample &= 0x7fff;
            v.dirty |= 0x80;
        }
        if v.instrument != sample {
            v.instrument = sample;
            if !self.sfx_override {
                self.start_sample(v, sample);
            }
        }
    }
}

/// Borne la hauteur à 0x6B demi-tons.
fn clamp_note(pitch: u16) -> u16 {
    if (pitch >> 8) as u8 >= 0x6c {
        0x6b00 | (pitch & 0xff)
    } else {
        pitch
    }
}
