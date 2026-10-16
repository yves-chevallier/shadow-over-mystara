//! Voies du séquenceur.
//!
//! Il y a deux jeux de 16 voies, une par canal QSound : les effets sonores (`sfx`) et la musique
//! (`music`). Quand une voie d'effet est active sur un canal, la voie de musique du même canal
//! continue de se dérouler mais n'écrit plus dans la QSound (`sfx_override`).
//!
//! Une voie est dans l'un de deux états :
//! - `state == 1` : note simple (un échantillon ou un instrument joué pendant une durée) ;
//! - `state == 0xFF` : piste séquencée, lue octet par octet dans la ROM (notes et instructions).
//!
//! Chaque tic (62,5 Hz) une voie : avance sa piste, puis fait évoluer pan, LFO, glissando et
//! enveloppe, et écrit volume et hauteur dans la QSound.

use crate::math::*;
use crate::pan::PanFade;
use crate::{shared, Driver};

#[derive(Clone, Default)]
pub struct Voice {
    /// 0 libre, 1 note simple, 0xFF piste séquencée.
    pub state: u8,
    /// Adresse de lecture dans la piste.
    pub seq: u32,
    /// Drapeaux de piste : bits 0-2 octave, 3 et 7 : voir `play_note`, 4-5 mode de durée.
    pub flags: u8,
    /// Temps restant avant le prochain évènement (virgule fixe 8.8).
    pub wait: u16,
    /// Proportion de la durée pendant laquelle la note est tenue (x/256).
    pub gate_ratio: u8,
    /// Temps restant avant le relâchement de la note.
    pub gate: u16,
    pub transpose: u8,
    pub detune: u8,
    /// Note de référence de l'échantillon (jouée à la hauteur nominale).
    pub root: u8,
    pub detune2: u8,
    /// Adresse de début de l'échantillon (réécrite à chaque déclenchement).
    pub start: u16,
    /// Glissando : 0 inactif, 1 terminé ce tic, 0xFF en cours.
    pub glide: u8,
    pub glide_rate: u8,
    pub glide_step: u16,
    /// Hauteur courante : octet haut = demi-ton, octet bas = fraction.
    pub pitch: u16,
    /// Hauteur visée.
    pub target: u16,
    /// Amplitude propre à la voie.
    pub amplitude: u16,
    /// Niveau d'envoi vers l'écho.
    pub echo: u16,
    /// Compteurs des quatre boucles imbriquées.
    pub loops: [u8; 4],
    /// Bit 0 : hauteur à réécrire. Bit 7 : suit la variation de hauteur globale.
    pub dirty: u8,
    /// Bit 0 : LFO remis à zéro à chaque note. Bits 6-7 : forme du LFO.
    pub lfo: u8,
    pub lfo_rate: u16,
    pub lfo_phase: u16,
    pub vibrato: u16,
    pub tremolo: u16,
    /// Instrument en cours (bit 15 : choisi par une instruction de piste) et banque d'instruments.
    pub instrument: u16,
    pub inst_bank: u16,
    /// Phase de l'enveloppe : 0 attaque, 1 chute, 2 tenue, 3 chute lente, 4 relâchement, 5 fin.
    pub env_phase: u8,
    pub env_level: u16,
    pub env_attack: u16,
    pub env_decay: u16,
    pub env_sustain: u16,
    pub env_slow: u16,
    pub env_release: u16,
    pub priority: u8,
    /// Pas de temps propre à la voie (effets).
    pub step: u16,
    /// Note simple qui vient de démarrer.
    pub fresh: u8,
    /// Note simple jouée jusqu'à son arrêt explicite.
    pub continuous: u8,
    /// Panoramique et son évolution.
    pub pan: PanFade,
    /// Délai avant que la voie ne démarre (jamais armé par le programme d'origine).
    pub delay: u8,
}

impl Voice {
    /// Nouvelle piste séquencée qui démarre en `seq`.
    pub(crate) fn track(seq: u32, inst_bank: u16) -> Self {
        Voice { state: 0xff, seq, inst_bank, instrument: 0xff00, lfo: 1, env_phase: 5, ..Voice::default() }
    }

    /// Lit l'octet suivant de la piste.
    pub(crate) fn fetch(&mut self, rom: &crate::Rom) -> u8 {
        let b = rom.byte(self.seq);
        self.seq = (self.seq + 1) & 0xff_ffff;
        b
    }

    /// Lit un mot grand-boutiste.
    pub(crate) fn fetch_be16(&mut self, rom: &crate::Rom) -> u16 {
        let h = self.fetch(rom);
        let l = self.fetch(rom);
        u16::from(h) << 8 | u16::from(l)
    }
}

impl Driver {
    // ---------------------------------------------------------------- QSound

    /// Coupe un canal : plus d'écho, volume et fréquence à zéro.
    pub(crate) fn silence_voice(&mut self, ch: u8) {
        let base = ch.wrapping_mul(8);
        self.qsound(0xba_u8.wrapping_add(ch), 0);
        self.qsound(base.wrapping_add(6), 0);
        self.qsound(base.wrapping_add(2), 0);
        self.qsound(base.wrapping_add(2), 0x1000);
    }

    /// Écrit un registre de pan (0x80 + canal, 0x90 + canal ADPCM) ; en mono le pan est toujours
    /// centré. Le Z80 recopie aussi la valeur en 0xC005 (rapport au 68000).
    pub(crate) fn write_pan(&mut self, reg: u8, pan: u8, report: bool) {
        let pan = if self.stereo == 0 { 0x10 } else { pan };
        if report {
            self.write_shared(shared::PAN_REPORT, pan);
        }
        self.qsound(reg, PAN_BASE + u16::from(pan));
    }

    /// Règle le pan d'une voie qui démarre, puis coupe son canal.
    pub(crate) fn pan_and_mute(&mut self, v: &Voice, ch: u8) {
        self.write_pan(0x80 + ch, v.pan.pan, true);
        self.silence_voice(ch);
    }

    /// Charge un échantillon dans le canal courant (adresses, boucle, banque).
    /// Les entrées de la table font 8 octets : banque, début, début de boucle, fin, note de référence.
    pub(crate) fn start_sample(&mut self, v: &mut Voice, id: u16) {
        let base = self.cur.wrapping_mul(8);
        let p = u32::from(self.rom.word(SAMPLE_PTR)) + ((u32::from(id) * 8) & 0xffff);
        self.qsound(base.wrapping_add(2), 0);
        let bank = self.rom.byte(p);
        v.start = self.rom.word(p + 1);
        let loop_start = self.rom.word(p + 3);
        let end = self.rom.word(p + 5);
        v.root = self.rom.byte(p + 7);
        self.qsound(base.wrapping_add(1), v.start);
        self.qsound(base.wrapping_add(5), end);
        self.qsound(base.wrapping_add(4), end.wrapping_sub(loop_start));
        // La banque d'un canal se règle dans le registre 0 du canal précédent.
        let bank_reg = self.cur.wrapping_sub(1).rotate_left(3) & 0x78;
        self.qsound(bank_reg, 0x8000 | u16::from(bank));
    }

    /// Déclenche la note : départ de la lecture de l'échantillon, puis volume et hauteur.
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

    /// Fréquence de lecture QSound pour une hauteur (demi-ton.fraction) et la note de référence.
    fn pitch_rate(&self, pitch: u16, root: u8) -> u16 {
        let note = ((pitch >> 8) as u8).wrapping_sub(1);
        let a = 0x3c_u8.wrapping_sub(root).wrapping_add(note);
        // Bornage de l'original : au-delà de 0x6B, 0x6B si le résultat de `a - 0x6C` a son
        // bit de signe, sinon 0 (le programme d'origine rapporte donc 0 pour 0x6C-0xEB).
        let n = if a < 0x6c {
            a
        } else if a.wrapping_sub(0x6c) & 0x80 != 0 {
            0x6b
        } else {
            0
        };
        let octave = self.rom.byte(TABLE_OCTAVE + u32::from(n));
        let semitone = n.wrapping_sub(self.rom.byte(TABLE_OCTAVE_START + u32::from(octave)));
        let fine_at = TABLE_FINE + (u32::from(semitone) << 8 | u32::from(pitch as u8));
        let fine = u16::from(self.rom.byte(fine_at)) << octave;
        self.rom.word(TABLE_RATE + 2 * u32::from(n)).wrapping_add(fine)
    }

    /// Écrit volume et fréquence de la voie courante.
    pub(crate) fn write_volume_pitch(&mut self, v: &mut Voice) {
        let base = self.cur.wrapping_mul(8);
        // Volume : base (effets ou musique), fondu, enveloppe, trémolo, amplitude de la voie.
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
        // Hauteur : seulement si elle a changé, ou si un LFO / glissando la fait bouger.
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

    /// Valeur du LFO (triangle selon le bit 6 : montant ou descendant).
    fn lfo_value(&self, v: &Voice) -> u16 {
        if v.lfo & 0x40 != 0 {
            0xffff - v.lfo_phase
        } else {
            v.lfo_phase
        }
    }

    // ------------------------------------------------------------- évolution

    /// Pan, LFO, glissando et enveloppe d'une voie, puis écriture dans la QSound.
    pub(crate) fn update_voice(&mut self, v: &mut Voice) {
        if v.pan.step() {
            self.write_pan(0x80 + self.cur, v.pan.pan, true);
        }
        // LFO : la phase avance, un débordement inverse le sens du triangle (bit 6).
        let (phase, carry) = v.lfo_phase.overflowing_add(v.lfo_rate);
        v.lfo_phase = phase;
        if carry {
            v.lfo = v.lfo.wrapping_add(0x40);
        }
        self.update_glide(v);
        self.update_envelope(v);
    }

    /// Glissando : la hauteur rejoint la hauteur visée par pas de `glide_step`.
    fn update_glide(&mut self, v: &mut Voice) {
        match v.glide {
            0 => {}
            1 => v.glide = 0,
            _ => {
                let new = v.pitch.wrapping_add(v.glide_step);
                let negative = v.glide_step & 0x8000 != 0;
                let (diff, borrow) = new.overflowing_sub(v.target);
                // Atteinte (ou dépassée) selon le sens du glissando.
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

    /// Enveloppe de volume, puis écriture dans la QSound si le canal est libre.
    fn update_envelope(&mut self, v: &mut Voice) {
        let level = v.env_level;
        let new = match v.env_phase {
            // Attaque : monte jusqu'au maximum, puis chute.
            0 => match level.overflowing_add(v.env_attack) {
                (l, false) => l,
                _ => {
                    v.env_phase = 1;
                    0xffff
                }
            },
            // Chute jusqu'au niveau de tenue ; sans niveau de tenue la note s'arrête.
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
            // Chute lente puis relâchement : jusqu'à zéro.
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
            // Tenue ou fin : rien à écrire.
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

    // ---------------------------------------------------- tic du séquenceur

    /// Un tic (62,5 Hz) : toutes les voies, puis les fondus généraux.
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
        }
        self.update_master_fade();
        self.update_music_ramp();
    }

    /// Fondu général (commandes 12 et 13) : le niveau évolue d'un pas par tic.
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
            // Débordement par le haut : fondu terminé ; par le bas : niveau nul.
            if carry_in & 0x80 == 0 {
                g.fade_dir = 0;
            }
            g.fade_level = 0;
        }
    }

    /// Rampe du volume de la musique (commandes 6 et 7).
    fn update_music_ramp(&mut self) {
        let g = &mut self.g;
        if g.ramp_dir == 0 {
            return;
        }
        let base = if g.ramp_dir & 0x80 != 0 { g.ramp_dir } else { g.ramp_dir - 1 };
        let (level, carry) = g.ramp_level.overflowing_add(g.ramp_step);
        let mut out_of_range = base.wrapping_add(carry as u8) != 0;
        if !out_of_range {
            // Dans la plage : la cible est-elle atteinte ?
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
