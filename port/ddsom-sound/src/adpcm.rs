//! Voies ADPCM : trois canaux qui jouent un échantillon une fois (ou en boucle) sans séquenceur.
//!
//! Le programme d'origine n'a pas de notion de hauteur ici : une voie ADPCM est démarrée par une
//! commande « effet à une voie » dont le numéro de voie est au moins 0x10.

use crate::math::*;
use crate::pan::PanFade;
use crate::Driver;

#[derive(Clone, Default)]
pub struct AdpcmChannel {
    /// Vient de démarrer : le prochain tic programme les registres.
    pub fresh: bool,
    /// Rejoue l'échantillon en boucle jusqu'à l'arrêt explicite.
    pub continuous: bool,
    /// Durée restante, décomptée par pas de 0x40 à chaque tic ; 0 = canal libre.
    pub remaining: u16,
    pub volume: u16,
    pub pan: PanFade,
    pub delay: u8,
    pub priority: u8,
    /// Durée totale, pour la reprise en boucle.
    pub length: u16,
    pub bank: u8,
    pub start: u16,
    pub end: u16,
}

impl Driver {
    /// Coupe un canal ADPCM.
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

    /// Démarre une voie ADPCM. `p` pointe sur la définition de l'effet : drapeaux et priorité,
    /// 4 octets ignorés, amplitude, numéro d'échantillon.
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
        let amplitude = self.rom.table(TABLE_AMPLITUDE, self.rom.byte(p + 4));
        let sample = u16::from(self.rom.byte(p + 5)) << 8 | u16::from(self.rom.byte(p + 6));
        // Fiche de l'échantillon : banque, un octet ignoré, début, fin.
        let entry = u32::from(self.rom.word(SAMPLE_PTR)) + ((u32::from(sample) * 8) & 0xffff);
        let start = self.rom.word(entry + 2);
        let end = self.rom.word(entry + 4);
        let Some(fade) = PanFade::from_command(cmd) else { return };
        let mut a = AdpcmChannel {
            fresh: true,
            continuous: flags & 0x80 != 0,
            remaining: end.wrapping_sub(start),
            volume: hi16(amplitude, self.g.sfx_volume) << 2,
            pan: fade,
            priority: flags & 0x7f,
            length: end.wrapping_sub(start),
            bank: self.rom.byte(entry),
            start,
            end,
            ..AdpcmChannel::default()
        };
        if cmd[6] == 7 {
            // Le canal voisin reçoit une copie qui part dans l'autre sens, si sa priorité le permet.
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

    /// Tic des trois canaux ADPCM.
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
        if std::mem::take(&mut a.fresh) {
            self.write_pan(0x90 + ch, a.pan.pan, true);
        } else if a.pan.step() {
            self.write_pan(0x90 + ch, a.pan.pan, true);
        }
    }

    /// Écrit adresses, banque, volume et lance la lecture.
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
