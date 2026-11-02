//! ddsom sound driver rewritten in Rust.
//!
//! It replaces the Z80 program (`dd2.01`): same writes to the QSound registers, in the same
//! order, and same writes to the RAM shared with the 68000, for the same commands. How it works
//! is described in `docs/son.md`.
//!
//! Layout:
//! - [`Driver`] : 250 Hz interrupt, command queue, main loop (lib.rs);
//! - `command` : execution of the commands sent by the 68000;
//! - `voice` : sequencer voices (music and effects), envelopes, output to the QSound;
//! - `adpcm` : the three ADPCM voices;
//! - [`Rom`] : access to the Z80 data (sequences, instruments, samples).

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

/// Duration of the 250 Hz interrupt, in Z80 cycles (8 MHz).
pub const IRQ_PERIOD_CYCLES: u64 = 32_000;

/// 250 Hz interrupts that the original Z80 loses at startup: it masks interrupts during QSound
/// initialisation and the clearing of its RAM (about 128,000 cycles, measured on the full
/// machine), and takes only one of them when re-enabling them.
const INIT_MASKED_PERIODS: u32 = 3;

/// Size of the RAM shared with the 68000 (Z80 side: C000-CFFF).
pub const SHARED_SIZE: usize = 0x1000;

/// Offsets in the shared RAM.
mod shared {
    /// Mailbox: 16 command bytes at 000-00F; byte 00F is 0xFF once the Z80 has taken the
    /// command (the 68000 only writes a new command if it sees 0xFF).
    pub const MAILBOX: usize = 0x000;
    pub const MAILBOX_FLAG: usize = 0x00f;
    /// Pan byte of the last voice set (Z80 report to the 68000).
    pub const PAN_REPORT: usize = 0x005;
    /// 0x88: the 68000 has initialised the mailbox.
    pub const MAILBOX_READY: usize = 0xffd;
    /// Stereo setting (0 = mono).
    pub const STEREO: usize = 0xffe;
    /// Handshake: the Z80 writes 0x77 at startup, the 68000 answers 0xFF.
    pub const HANDSHAKE: usize = 0xfff;
}

/// Write produced by the driver, in the order the Z80 makes them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Write {
    /// QSound register (write of `data` to register `reg`).
    Qsound { reg: u8, data: u16 },
    /// Z80 write to the shared RAM (offset 0-0xFFF).
    Shared { offset: u16, value: u8 },
}

/// Where the driver startup stands.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Boot {
    /// Waiting for the first "tick" (4 interrupts).
    WaitFirstTick,
    /// Waiting for the second tick, then QSound initialisation.
    WaitSecondTick,
    /// Initialisation done: waiting for the 68000's 0xFF answer at 0xFFF.
    WaitHandshake,
    /// Main loop.
    Running,
}

/// Pacing of the tasks triggered by the interrupt (one in four each time).
#[derive(Default)]
struct Pending {
    /// Commands waiting to be processed (odd interrupts).
    commands: u8,
    /// Sequencer ticks, 62.5 Hz.
    ticks: u8,
    /// ADPCM voice ticks, 62.5 Hz (offset by two interrupts).
    adpcm: u8,
}

/// Global settings (music and effects).
#[derive(Default)]
pub(crate) struct Globals {
    /// Base volume of the effects (table indexed by the music header).
    pub sfx_volume: u16,
    /// Music volume factor (header).
    pub music_factor: u16,
    /// Current music volume.
    pub music_volume: u16,
    /// Time step of the music voices (instruction 5) and tempo adjustment (command 8).
    pub music_step: u16,
    pub tempo_adjust: u16,
    /// Global music transposition (instruction 10).
    pub music_transpose: u8,
    /// Pitch added to the voices that asked for it (command 9).
    pub pitch_bend: u16,
    /// Time step of the voice being processed.
    pub step: u16,
    /// Global fade (commands 12 and 13): direction (0 = inactive, 1 = up, 0xFF = down),
    /// step and current level.
    pub fade_dir: u8,
    pub fade_step: u16,
    pub fade_level: u16,
    /// Music volume ramp (commands 6 and 7): direction, step, level and target.
    pub ramp_dir: u8,
    pub ramp_step: u16,
    pub ramp_level: u16,
    pub ramp_target: u16,
}

/// The sound driver.
pub struct Driver {
    rom: Rom,
    boot: Boot,
    /// Copy of the shared RAM while an interrupt is being processed.
    shared: Vec<u8>,
    out: Vec<Write>,
    /// Interrupt counter (modulo 256).
    irq_count: u8,
    pending: Pending,
    /// Copy of the 68000's stereo byte (0 = mono).
    stereo: u8,
    /// Command queue: 16 entries of 16 bytes, byte indices wrapping at 256.
    queue: [u8; 256],
    queue_head: u8,
    queue_tail: u8,
    /// Header of the music table: number of entries and table address.
    song_count: u16,
    song_table: u32,
    /// Address of the music or effect being started.
    song: u32,
    /// Priority of the current command.
    priority: u8,
    pub(crate) g: Globals,
    /// Voice being processed: index, and `true` if it is a music voice.
    cur: u8,
    is_music: bool,
    /// The original driver loops forever on a noteless track that calls itself:
    /// the sequencer is then frozen (interrupts keep accumulating).
    hung: bool,
    /// 250 Hz periods still to ignore (initialisation in progress in the original Z80).
    masked_periods: u32,
    /// An effect voice occupies the current channel: the music voice does not touch the QSound.
    sfx_override: bool,
    /// Sound-effect voices then music voices, one per QSound channel.
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

    /// Power-up (or reset released by the 68000): the Z80 announces its presence. Its RAM is
    /// not cleared by the reset: the interrupt counter, which paces startup, is kept.
    pub fn reset(&mut self, shared: &mut [u8], out: &mut Vec<Write>) {
        let rom = std::mem::replace(&mut self.rom, Rom::from_region(Vec::new()));
        let irq_count = self.irq_count;
        *self = Driver::new(rom);
        self.irq_count = irq_count;
        self.shared.copy_from_slice(shared);
        self.write_shared(shared::HANDSHAKE, 0x77);
        self.finish(shared, out);
    }

    /// One 250 Hz interrupt taken by the Z80: what the interrupt routine does, then what the
    /// main loop does before the next interrupt. QSound writes are returned in `out`; those to
    /// the shared RAM are also made in `shared`.
    pub fn interrupt(&mut self, shared: &mut [u8], out: &mut Vec<Write>) {
        self.shared.copy_from_slice(shared);
        self.irq_handler();
        self.main_loop();
        self.finish(shared, out);
    }

    /// A 250 Hz period has elapsed (every [`IRQ_PERIOD_CYCLES`] of the Z80). Equivalent to
    /// [`Driver::interrupt`], except during initialisation when the original Z80 does not respond.
    pub fn irq_request(&mut self, shared: &mut [u8], out: &mut Vec<Write>) {
        if self.masked_periods > 0 {
            self.masked_periods -= 1;
        } else {
            self.interrupt(shared, out);
        }
    }

    /// Hands the shared RAM back to the caller along with the writes produced.
    fn finish(&mut self, shared: &mut [u8], out: &mut Vec<Write>) {
        shared.copy_from_slice(&self.shared);
        out.append(&mut self.out);
    }

    /// Interrupt routine: counts, paces the tasks, copies the 68000's command.
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

    /// Main loop: startup, then processing of the pending tasks.
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
        // One pass: a command, an ADPCM tick, a sequencer tick, in that order.
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

    /// QSound initialisation, RAM clearing and reading of the music header.
    fn init_hardware(&mut self) {
        // Register table (0xD5) and their values (0xE1): filter and echo settings.
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
        // The Z80 RAM is cleared (interrupt counters included).
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
        // Music header: header address, then [number of songs (2 bytes,
        // big-endian), music factor index, effects volume index].
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
