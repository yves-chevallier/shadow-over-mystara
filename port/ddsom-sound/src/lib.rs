//! Pilote son de ddsom réécrit en Rust.
//!
//! Il remplace le programme du Z80 (`dd2.01`) : mêmes écritures dans les registres de la QSound,
//! dans le même ordre, et mêmes écritures dans la RAM partagée avec le 68000, pour les mêmes
//! commandes. Le fonctionnement est décrit dans `docs/son.md`.
//!
//! Organisation :
//! - [`Driver`] : interruption à 250 Hz, file de commandes, boucle principale (lib.rs) ;
//! - `command` : exécution des commandes envoyées par le 68000 ;
//! - `voice` : voies de séquenceur (musiques et effets), enveloppes, sortie vers la QSound ;
//! - `adpcm` : les trois voies ADPCM ;
//! - [`Rom`] : accès aux données du Z80 (séquences, instruments, échantillons).

mod adpcm;
mod command;
pub mod image;
mod math;
mod pan;
mod rom;
mod track;
mod voice;

pub use rom::Rom;

use adpcm::AdpcmChannel;
use voice::Voice;

/// Durée de l'interruption à 250 Hz, en cycles du Z80 (8 MHz).
pub const IRQ_PERIOD_CYCLES: u64 = 32_000;

/// Interruptions de 250 Hz que le Z80 d'origine perd au démarrage : il masque les interruptions
/// pendant l'initialisation de la QSound et la remise à zéro de sa RAM (environ 128 000 cycles,
/// mesurés dans la machine complète), et n'en prend qu'une seule en les réactivant.
const INIT_MASKED_PERIODS: u32 = 3;

/// Taille de la RAM partagée avec le 68000 (côté Z80 : C000-CFFF).
pub const SHARED_SIZE: usize = 0x1000;

/// Décalages dans la RAM partagée.
mod shared {
    /// Boîte aux lettres : 16 octets de commande en 000-00F ; l'octet 00F vaut 0xFF quand le
    /// Z80 a pris la commande (le 68000 n'écrit une nouvelle commande que s'il voit 0xFF).
    pub const MAILBOX: usize = 0x000;
    pub const MAILBOX_FLAG: usize = 0x00f;
    /// Octet de pan de la dernière voie réglée (rapport du Z80 au 68000).
    pub const PAN_REPORT: usize = 0x005;
    /// 0x88 : le 68000 a initialisé la boîte aux lettres.
    pub const MAILBOX_READY: usize = 0xffd;
    /// Réglage stéréo (0 = mono).
    pub const STEREO: usize = 0xffe;
    /// Poignée de main : le Z80 écrit 0x77 au démarrage, le 68000 répond 0xFF.
    pub const HANDSHAKE: usize = 0xfff;
}

/// Écriture produite par le pilote, dans l'ordre où le Z80 les fait.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Write {
    /// Registre de la QSound (écriture de `data` dans le registre `reg`).
    Qsound { reg: u8, data: u16 },
    /// Écriture du Z80 dans la RAM partagée (décalage 0-0xFFF).
    Shared { offset: u16, value: u8 },
}

/// Où en est le démarrage du pilote.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Boot {
    /// Attente du premier « tic » (4 interruptions).
    WaitFirstTick,
    /// Attente du deuxième tic, puis initialisation de la QSound.
    WaitSecondTick,
    /// Initialisation faite : attente de la réponse 0xFF du 68000 en 0xFFF.
    WaitHandshake,
    /// Boucle principale.
    Running,
}

/// Rythme des tâches déclenchées par l'interruption (une sur quatre à chaque fois).
#[derive(Default)]
struct Pending {
    /// Commandes en attente de traitement (interruptions impaires).
    commands: u8,
    /// Tics du séquenceur, 62,5 Hz.
    ticks: u8,
    /// Tics des voies ADPCM, 62,5 Hz (décalés de deux interruptions).
    adpcm: u8,
}

/// Réglages globaux (musique et effets).
#[derive(Default)]
pub(crate) struct Globals {
    /// Volume de base des effets (table indexée par l'en-tête des musiques).
    pub sfx_volume: u16,
    /// Facteur de volume de la musique (en-tête).
    pub music_factor: u16,
    /// Volume courant de la musique.
    pub music_volume: u16,
    /// Pas de temps des voies de musique (instruction 5) et ajustement de tempo (commande 8).
    pub music_step: u16,
    pub tempo_adjust: u16,
    /// Transposition globale de la musique (instruction 10).
    pub music_transpose: u8,
    /// Hauteur ajoutée aux voies qui l'ont demandé (commande 9).
    pub pitch_bend: u16,
    /// Pas de temps de la voie en cours de traitement.
    pub step: u16,
    /// Fondu général (commandes 12 et 13) : sens (0 = inactif, 1 = montée, 0xFF = descente),
    /// pas et niveau courant.
    pub fade_dir: u8,
    pub fade_step: u16,
    pub fade_level: u16,
    /// Rampe de volume de la musique (commandes 6 et 7) : sens, pas, niveau et cible.
    pub ramp_dir: u8,
    pub ramp_step: u16,
    pub ramp_level: u16,
    pub ramp_target: u16,
}

/// Le pilote son.
pub struct Driver {
    rom: Rom,
    boot: Boot,
    /// Copie de la RAM partagée pendant le traitement d'une interruption.
    shared: Vec<u8>,
    out: Vec<Write>,
    /// Compteur d'interruptions (modulo 256).
    irq_count: u8,
    pending: Pending,
    /// Copie de l'octet stéréo du 68000 (0 = mono).
    stereo: u8,
    /// File de commandes : 16 entrées de 16 octets, indices d'octets qui bouclent à 256.
    queue: [u8; 256],
    queue_head: u8,
    queue_tail: u8,
    /// En-tête de la table des musiques : nombre d'entrées et adresse de la table.
    song_count: u16,
    song_table: u32,
    /// Adresse de la musique ou de l'effet en cours de démarrage.
    song: u32,
    /// Priorité de la commande en cours.
    priority: u8,
    pub(crate) g: Globals,
    /// Voie en cours de traitement : indice, et `true` si c'est une voie de musique.
    cur: u8,
    is_music: bool,
    /// Le pilote d'origine boucle indéfiniment sur une piste sans note qui se rappelle elle-même :
    /// le séquenceur est alors gelé (les interruptions continuent de s'accumuler).
    hung: bool,
    /// Périodes de 250 Hz à ignorer encore (initialisation en cours dans le Z80 d'origine).
    masked_periods: u32,
    /// Une voie d'effet occupe le canal en cours : la voie de musique ne touche pas à la QSound.
    sfx_override: bool,
    /// Voies d'effets sonores puis voies de musique, une par canal QSound.
    pub(crate) sfx: [Voice; 16],
    pub(crate) music: [Voice; 16],
    pub(crate) adpcm: [AdpcmChannel; 3],
}

impl Driver {
    pub fn new(rom: Rom) -> Self {
        Driver {
            rom,
            boot: Boot::WaitFirstTick,
            shared: vec![0; SHARED_SIZE],
            out: Vec::new(),
            irq_count: 0,
            pending: Pending::default(),
            stereo: 0,
            queue: [0; 256],
            queue_head: 0,
            queue_tail: 0,
            song_count: 0,
            song_table: 0,
            song: 0,
            priority: 0,
            g: Globals::default(),
            cur: 0,
            is_music: false,
            sfx_override: false,
            hung: false,
            masked_periods: 0,
            sfx: Default::default(),
            music: Default::default(),
            adpcm: Default::default(),
        }
    }

    /// Mise sous tension : le Z80 annonce sa présence au 68000.
    pub fn reset(&mut self, shared: &mut [u8], out: &mut Vec<Write>) {
        let rom = std::mem::replace(&mut self.rom, Rom::from_region(Vec::new()));
        *self = Driver::new(rom);
        self.shared.copy_from_slice(shared);
        self.write_shared(shared::HANDSHAKE, 0x77);
        self.finish(shared, out);
    }

    /// Une interruption à 250 Hz prise par le Z80 : ce que fait la routine d'interruption, puis ce
    /// que fait la boucle principale avant l'interruption suivante. Les écritures dans la QSound
    /// sont rendues dans `out`, celles dans la RAM partagée sont aussi faites dans `shared`.
    pub fn interrupt(&mut self, shared: &mut [u8], out: &mut Vec<Write>) {
        self.shared.copy_from_slice(shared);
        self.irq_handler();
        self.main_loop();
        self.finish(shared, out);
    }

    /// Une période de 250 Hz s'est écoulée (toutes les [`IRQ_PERIOD_CYCLES`] du Z80). Équivaut à
    /// [`Driver::interrupt`], sauf pendant l'initialisation où le Z80 d'origine ne répond pas.
    pub fn irq_request(&mut self, shared: &mut [u8], out: &mut Vec<Write>) {
        if self.masked_periods > 0 {
            self.masked_periods -= 1;
        } else {
            self.interrupt(shared, out);
        }
    }

    /// Rend la RAM partagée à l'appelant et lui remet les écritures produites.
    fn finish(&mut self, shared: &mut [u8], out: &mut Vec<Write>) {
        shared.copy_from_slice(&self.shared);
        out.append(&mut self.out);
    }

    /// Routine d'interruption : compte, cadence les tâches, recopie la commande du 68000.
    fn irq_handler(&mut self) {
        self.irq_count = self.irq_count.wrapping_add(1);
        match self.irq_count & 3 {
            0 => self.pending.ticks = self.pending.ticks.wrapping_add(1),
            2 => self.pending.adpcm = self.pending.adpcm.wrapping_add(1),
            _ => self.pending.commands = self.pending.commands.wrapping_add(1),
        }
        if self.shared[shared::MAILBOX_READY] != 0x88 {
            return;
        }
        if self.shared[shared::MAILBOX_FLAG] != 0xff {
            let at = usize::from(self.queue_head);
            self.queue[at..at + 16].copy_from_slice(&self.shared[shared::MAILBOX..shared::MAILBOX + 16]);
            self.queue_head = self.queue_head.wrapping_add(16);
            self.write_shared(shared::MAILBOX_FLAG, 0xff);
        }
        self.stereo = self.shared[shared::STEREO];
    }

    /// Boucle principale : démarrage, puis traitement des tâches en attente.
    fn main_loop(&mut self) {
        if self.boot == Boot::WaitFirstTick && self.pending.ticks != 0 {
            self.pending.ticks = 0;
            self.boot = Boot::WaitSecondTick;
        }
        if self.boot == Boot::WaitSecondTick && self.pending.ticks != 0 {
            self.init_hardware();
            self.boot = Boot::WaitHandshake;
        }
        if self.boot == Boot::WaitHandshake && self.shared[shared::HANDSHAKE] == 0xff {
            self.boot = Boot::Running;
        }
        if self.boot != Boot::Running || self.hung {
            return;
        }
        // Une passe : une commande, un tic ADPCM, un tic du séquenceur, dans cet ordre.
        while self.pending.commands != 0 || self.pending.adpcm != 0 || self.pending.ticks != 0 {
            if self.pending.commands != 0 {
                self.pending.commands -= 1;
                self.process_command();
            }
            if self.pending.adpcm != 0 {
                self.pending.adpcm -= 1;
                self.adpcm_tick();
            }
            if self.pending.ticks != 0 {
                self.pending.ticks -= 1;
                self.sequencer_tick();
            }
            if self.hung {
                return;
            }
        }
    }

    /// Initialisation de la QSound, mise à zéro de la RAM et lecture de l'en-tête des musiques.
    fn init_hardware(&mut self) {
        // Table de registres (0xD5) et de leurs valeurs (0xE1) : réglage des filtres et de l'écho.
        for i in 0.. {
            let reg = self.rom.byte(0xd5 + i);
            let data = self.rom.word(0xe1 + 2 * i);
            self.qsound(reg, data);
            if reg == 0xe3 {
                break;
            }
        }
        for ch in 0..16 {
            self.silence_voice(ch);
        }
        for ch in 0..3 {
            self.silence_adpcm(ch);
        }
        self.masked_periods = INIT_MASKED_PERIODS;
        // La RAM du Z80 est remise à zéro (compteurs d'interruption compris).
        self.irq_count = 0;
        self.pending = Pending::default();
        self.stereo = 0;
        self.queue = [0; 256];
        self.queue_head = 0;
        self.queue_tail = 0;
        self.g = Globals::default();
        self.sfx = Default::default();
        self.music = Default::default();
        self.adpcm = Default::default();
        self.song = 0;
        self.priority = 0;
        self.cur = 0;
        self.is_music = false;
        self.sfx_override = false;
        // En-tête des musiques : adresse de l'en-tête, puis [nombre de musiques (2 octets,
        // grand-boutiste), index du facteur de musique, index du volume des effets].
        let header = u32::from(self.rom.word(math::HEADER_PTR));
        self.song_count = u16::from(self.rom.byte(header)) << 8 | u16::from(self.rom.byte(header + 1));
        self.g.music_factor = self.rom.table(math::TABLE_LEVELS, self.rom.byte(header + 2));
        self.g.sfx_volume = self.rom.table(math::TABLE_LEVELS, self.rom.byte(header + 3));
        self.song_table = header + 6;
    }

    pub(crate) fn qsound(&mut self, reg: u8, data: u16) {
        self.out.push(Write::Qsound { reg, data });
    }

    pub(crate) fn write_shared(&mut self, offset: usize, value: u8) {
        self.shared[offset] = value;
        self.out.push(Write::Shared { offset: offset as u16, value });
    }
}
