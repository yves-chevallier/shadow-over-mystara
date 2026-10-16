//! Commandes du 68000 : file d'attente, démarrage des musiques et des effets, commandes système.
//!
//! Une commande fait 16 octets. Si l'octet 0 vaut 0xFF c'est une commande système dont l'octet 1
//! est le numéro. Sinon les octets 0-1 (grand-boutiste) sont le numéro d'une musique ou d'un effet,
//! et les octets 6 et suivants règlent le pan (voir `setup_pan`).

use crate::math::*;
use crate::pan::PanFade;
use crate::voice::Voice;
use crate::Driver;

impl Driver {
    /// Prend la prochaine commande de la file et l'exécute.
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
        // Numéro de musique ou d'effet (modulo leur nombre) -> entrée de 4 octets : adresse sur 3
        // octets de la définition.
        let number = (u16::from(cmd[0]) << 8 | u16::from(cmd[1])) % self.song_count.max(1);
        let entry = self.song_table.wrapping_add(u32::from(number.wrapping_mul(4))) & 0xff_ffff;
        let song = u32::from(self.rom.byte(entry)) << 16 | u32::from(self.rom.word(entry + 1).swap_bytes());
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

    /// Musique : 16 pistes (une par canal), chacune désignée par un déplacement de 2 octets
    /// depuis le début de la définition (0 = canal absent).
    fn start_music(&mut self, mut p: u32) {
        for ch in 0..16u8 {
            self.cur = ch;
            let mut m = std::mem::take(&mut self.music[usize::from(ch)]);
            let was_active = m.state != 0;
            self.priority = m.state;
            m.state = 0;
            let offset = u32::from(self.rom.word(p).swap_bytes());
            p += 2;
            if offset != 0 {
                m = Voice::track(self.song + offset, self.inst_bank0());
                m.pan.pan = 0x10;
            }
            self.music[usize::from(ch)] = m;
            // Canal libre côté effets : pan centré, et coupure si une musique y jouait.
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

    /// Banc d'instruments par défaut des pistes.
    fn inst_bank0(&self) -> u16 {
        self.rom.word(u32::from(INSTRUMENT_BANKS_PTR))
    }

    /// Effet à plusieurs pistes : même format que la musique, mais une piste ne démarre que si la
    /// priorité de la commande est au moins celle de la voie qu'elle remplace.
    fn start_effect(&mut self, priority: u8, mut p: u32, cmd: &[u8; 16]) {
        self.priority = priority;
        for ch in 0..16u8 {
            self.cur = ch;
            let offset = u32::from(self.rom.word(p).swap_bytes());
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

    /// Effet à une seule voie (note simple d'un échantillon ou d'un instrument) ; les voies
    /// 0x10 et suivantes sont les voies ADPCM.
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
        let duration = self.rom.word(p).swap_bytes();
        p += 2;
        if duration == 0 {
            return;
        }
        let mut v = Voice::default();
        v.wait = duration;
        v.gate = self.rom.word(p).swap_bytes();
        v.amplitude = self.rom.table(TABLE_AMPLITUDE, self.rom.byte(p + 2));
        v.instrument = u16::from(self.rom.byte(p + 3)) << 8 | u16::from(self.rom.byte(p + 4));
        let note = u16::from(self.rom.byte(p + 5).wrapping_add(1)) << 8;
        v.pitch = clamp_pitch(note.wrapping_add(sext(self.rom.byte(p + 6))));
        v.state = 1;
        v.fresh = 0xff;
        v.priority = priority;
        v.continuous = flags & 0x80;
        self.setup_pan(&mut v, cmd);
        self.sfx[slot] = v;
    }

    /// Règle le pan d'une voie qui démarre d'après la commande, puis coupe le canal (la voie
    /// démarre au tic suivant). Pour le type 7 le canal voisin reçoit une copie de la voie qui
    /// part dans l'autre sens, après un délai de 4 tics, si sa priorité le permet.
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
    /// Commandes système (octet 0 = 0xFF, numéro en octet 1, paramètres en 2 et suivants).
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

    /// Coupe tout : voies d'effets, de musique et ADPCM.
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

    /// Coupe la musique ; les canaux occupés par un effet continuent de sonner.
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

    /// Rampe du volume de la musique : octet 2 = niveau visé (index de la table de niveaux),
    /// octets 3-4 = vitesse. `dir` : 1 monte depuis 0, 0xFF descend depuis le maximum.
    fn music_ramp(&mut self, cmd: &[u8; 16], dir: u8) {
        if self.g.ramp_dir == 0 {
            self.g.ramp_level = if dir == 1 { 0 } else { 0xffff };
        }
        self.g.ramp_dir = dir;
        let step = u16::from(cmd[3]) << 8 | u16::from(cmd[4]);
        self.g.ramp_step = if dir == 1 { step } else { 0u16.wrapping_sub(step) };
        self.g.ramp_target = self.rom.word(TABLE_LEVELS + u32::from(cmd[2] & 0xfe));
    }

    /// Fondu général de tous les canaux : octets 3-4 = vitesse.
    fn master_fade(&mut self, cmd: &[u8; 16], dir: u8) {
        if self.g.fade_dir == 0 {
            self.g.fade_level = if dir == 1 { 0 } else { 0xffff };
        }
        self.g.fade_dir = dir;
        let step = u16::from(cmd[3]) << 8 | u16::from(cmd[4]);
        self.g.fade_step = if dir == 1 { step } else { 0u16.wrapping_sub(step) };
    }

    /// Ajustement du tempo de la musique : octet 4 = valeur, octet 3 = 0xFF si négatif.
    fn set_tempo(&mut self, cmd: &[u8; 16]) {
        let value = (u32::from(cmd[4]) * 0x369 >> 8) as u16;
        self.g.tempo_adjust = if cmd[3] == 0xff { 0u16.wrapping_sub(value) } else { value };
    }

    /// Variation de hauteur des voies de musique : octet 4 = valeur, octet 3 = 0xFF si négatif.
    fn set_pitch_bend(&mut self, cmd: &[u8; 16]) {
        let value = u16::from(cmd[4]) << 4;
        self.g.pitch_bend = if cmd[3] == 0xff { 0u16.wrapping_sub(value) } else { value };
    }
}

/// Borne la hauteur à 0x6B demi-tons.
fn clamp_pitch(pitch: u16) -> u16 {
    if (pitch >> 8) as u8 >= 0x6c {
        0x6b00 | (pitch & 0xff)
    } else {
        pitch
    }
}
