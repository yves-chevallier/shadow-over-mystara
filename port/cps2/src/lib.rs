//! CPS2 machine: 68000 + bus + video, clocked line by line as in MAME.
//! Sound: Z80 + QSound (HLE), run line by line alternating with the 68000.

pub mod bus;
pub mod crypt;
pub mod eeprom;
mod sboxes;
pub mod sound;

use std::io;
use std::path::Path;

pub use bus::{Cps2Bus, Inputs};
pub use cps2_gfx::TileAtlas;
use eeprom::Eeprom93c46;
#[cfg(feature = "m68k")]
use m68k::Cpu;

/// 68000 clock: 16 MHz x 0.7375 (MAME compensates this way for the unemulated wait states).
pub const CPU_HZ: f64 = 16_000_000.0 * 0.7375;
/// 512 pixels per line at 8 MHz.
pub const LINE_HZ: f64 = 8_000_000.0 / 512.0;
pub const LINES: u32 = 262;
pub const VBLANK_LINE: u32 = 240;

/// ddsom program (Europe 960619).
pub const DDSOM_PROG: [&str; 8] = [
    "dd2e.03e", "dd2e.04e", "dd2e.05e", "dd2e.06e", "dd2e.07", "dd2e.08", "dd2e.09", "dd2e.10",
];

/// Recompiled-block executor: (pc, cpu, bus, deadline) -> Some(next pc) or None if there is no block.
#[cfg(feature = "m68k")]
pub type BlockExecutor = fn(u32, &mut Cpu, &mut Cps2Bus, u64) -> Option<u32>;

/// Cap on passes per frame through a program point (see `Machine::step_cap`).
#[derive(Clone, Copy, Debug)]
pub struct StepCap {
    /// Address of the counted point (start of the game step).
    pub pc: u32,
    /// Maximum number of passes executed per frame.
    pub max: u32,
    /// Passes executed since the start of the frame.
    count: u32,
}

impl StepCap {
    pub const fn new(pc: u32, max: u32) -> Self {
        StepCap { pc, max, count: 0 }
    }

    /// Pass at `pc`: true if it runs in this frame (it is counted), false if it is held back until the
    /// next one.
    pub fn passage(&mut self) -> bool {
        if self.count >= self.max {
            return false;
        }
        self.count += 1;
        true
    }
}

/// Processor work during a line, in place of the 68000 (see `Machine::line_work`): (board, frame step cap). It has no
/// duration: the line's time passes for the processor all the same.
pub type LineWork<B> = fn(&mut B, Option<&mut StepCap>);

/// What a machine holds besides its processor, sound and graphics: the hardware ([`Cps2Bus`]: video, sound link,
/// inputs, EEPROM) and the work RAM. The reference machine runs the 68000 on the bus itself (`Cps2Bus`, RAM
/// included); a ported game keeps its RAM and its state outside the bus (ddsom: `ddsom_game::world::World`) and
/// works on them through `Machine::line_work`.
pub trait Board: 'static {
    /// The hardware.
    fn hw(&self) -> &Cps2Bus;
    fn hw_mut(&mut self) -> &mut Cps2Bus;
    /// The work RAM (FF0000-FFFFFF), one word per entry.
    fn ram(&self) -> &[u16];
    fn ram_mut(&mut self) -> &mut [u16];
}

impl Board for Cps2Bus {
    fn hw(&self) -> &Cps2Bus {
        self
    }
    fn hw_mut(&mut self) -> &mut Cps2Bus {
        self
    }
    fn ram(&self) -> &[u16] {
        &self.ram
    }
    fn ram_mut(&mut self) -> &mut [u16] {
        &mut self.ram
    }
}

/// Fix for an undefined behavior of the original game (see `Machine::patches`).
#[cfg(feature = "m68k")]
#[derive(Clone, Copy)]
pub struct Patch {
    /// Address (block start) where it applies.
    pub entry: u32,
    /// Modifies the state and returns true to replace the execution of the instruction at `entry`.
    pub apply: fn(&mut Cpu, &mut Cps2Bus) -> bool,
}

#[cfg(feature = "m68k")]
/// Callback after each interpreted instruction (diagnostic tools): (cpu, bus, pc).
pub type StepHook = Box<dyn FnMut(&Cpu, &Cps2Bus, u32)>;

pub struct Machine<B: Board = Cps2Bus> {
    #[cfg(feature = "m68k")]
    /// Recompiled code (None = everything interpreted).
    pub executor: Option<BlockExecutor>,
    #[cfg(feature = "m68k")]
    /// Addresses where no recompiled block existed (executed by the interpreter).
    pub misses: std::collections::BTreeMap<u32, u64>,
    #[cfg(feature = "m68k")]
    /// 68000 instructions executed by the interpreter (the rest is run by the recompiled code).
    pub interp_steps: u64,
    #[cfg(feature = "m68k")]
    /// Callback after each interpreted instruction (diagnostic tools): (cpu, bus, pc).
    pub step_hook: Option<StepHook>,
    #[cfg(feature = "m68k")]
    /// Is the program only waiting for an interrupt? (the game's wait loop) The remaining time
    /// of the line is then skipped.
    pub idle: Option<fn(&Cpu, &Cps2Bus) -> bool>,
    /// Cap on game steps per frame. The original game sometimes chains several steps within one frame
    /// ("fast forward": a task makes the main task ready again without waiting for the frame); their number is
    /// then (frame duration) / (step duration) and so depends on the 68000 frequency. When `max` passes
    /// at `pc` have occurred in the frame, the next one is held back: the remaining frame time is skipped (like
    /// `idle`) and the step runs at the start of the next frame, after the vblank interrupt.
    pub step_cap: Option<StepCap>,
    #[cfg(feature = "m68k")]
    /// Fixes for behaviors the original game leaves to chance (uninitialized registers or memory) and
    /// that no rewrite can reproduce: applied identically by the game and by the reference machine.
    pub patches: Vec<Patch>,
    /// Replaces the execution of the 68000 (executor, interpreter, `idle`, fixes): called once per line, it
    /// does the processor's work of the line; the Z80 follows as usual. `cpu` is then unused, only its cycle counter
    /// advances (game without 68000, `ddsom_game::install_no_68k`).
    pub line_work: Option<LineWork<B>>,
    /// `run_frame` returns the frame (true by default). False: the frame is not computed and
    /// `run_frame` returns an empty vector.
    /// Rendering only reads the state (no side effect); tools that do not look at the frame (`validate`, `bench`)
    /// turn it off: it is most of the time of a frame in the game without 68000.
    pub rendering: bool,
    #[cfg(feature = "m68k")]
    pub cpu: Cpu,
    /// The hardware and the work RAM (see [`Board`]).
    pub bus: B,
    pub sound: sound::Sound,
    /// The graphics, as the renderer reads them.
    pub gfx: TileAtlas,
    pub frame: u64,
    /// Object table latched at the last vblank (displayed on the next frame).
    pub obj_buffer: Vec<u16>,
    pub pri_ctrl_buffer: [u16; 6],
    line_cycles: f64,
    /// Absolute CPU cycle at the end of the current line.
    cycle_debt: f64,
    /// Same for the Z80.
    z80_debt: f64,
}

/// ddsom 68000 program: (plain data view, decrypted program-space view), big-endian words.
pub fn load_program(romdir: &Path) -> io::Result<(Vec<u16>, Vec<u16>)> {
    let mut rom = Vec::with_capacity(0x200000);
    for name in DDSOM_PROG {
        let d = std::fs::read(romdir.join(name))?;
        // ROM_LOAD16_WORD_SWAP: words stored little-endian in the chip
        rom.extend(d.as_chunks::<2>().0.iter().map(|w| u16::from_le_bytes([w[0], w[1]])));
    }
    let key = std::fs::read(romdir.join("ddsom.key"))?;
    let (k, lower, upper) = crypt::parse_key(&key);
    let dec = crypt::decrypt(&rom, k, lower, upper);
    Ok((rom, dec))
}

/// Contents of the ddsom chips as the ROMs hold them: the machine that runs the original program (reference machine,
/// tools, `ddsom --features original`). The game machine has no program ROM nor Z80 program: [`GameContents`].
#[derive(Clone)]
pub struct Contents {
    /// 68000 program, data view (big-endian words, 0x200000 words).
    pub program: Vec<u16>,
    /// 68000 program, decrypted program-space view (opcodes and PC-relative reads).
    pub decrypted_program: Vec<u16>,
    /// Graphics, decoded into 8-bit pens (`cps2_gfx::TileAtlas`).
    pub graphics: TileAtlas,
    /// Z80 region (0x50000: 0-7FFF fixed, banks from 0x10000).
    pub audio: Vec<u8>,
    /// QSound DSP program (`dl-1425.bin`, at least 0x2000 bytes: only its tables are used).
    pub dsp: Vec<u8>,
    /// QSound sample bank (4 MB: the sample ROMs, or `ddsom_assets::samples::bank` from the WAV files).
    pub samples: Vec<u8>,
}

/// The Z80 region of the ROMs `dd2.01` and `dd2.02` (0x50000 bytes): `dd2.01` = 0x8000 at 0, then 0x18000 at 0x10000
/// (ROM_CONTINUE); `dd2.02` at 0x28000.
pub fn load_audio_region(romdir: &Path) -> io::Result<Vec<u8>> {
    let mut audio = vec![0u8; 0x50000];
    let d1 = std::fs::read(romdir.join("dd2.01"))?;
    audio[..0x8000].copy_from_slice(&d1[..0x8000]);
    audio[0x10000..0x28000].copy_from_slice(&d1[0x8000..0x20000]);
    let d2 = std::fs::read(romdir.join("dd2.02"))?;
    audio[0x28000..0x48000].copy_from_slice(&d2);
    Ok(audio)
}

/// The QSound sample ROMs `dd2.11m` and `dd2.12m` (ROM_LOAD16_WORD_SWAP), 4 MB.
pub fn load_samples(romdir: &Path) -> io::Result<Vec<u8>> {
    let mut samples = Vec::with_capacity(0x400000);
    for name in ["dd2.11m", "dd2.12m"] {
        let d = std::fs::read(romdir.join(name))?;
        samples.extend(d.as_chunks::<2>().0.iter().flat_map(|w| [w[1], w[0]]));
    }
    Ok(samples)
}

impl Contents {
    /// Reads the ROMs (ddsom.zip and dl-1425.bin). `gfx` and `samples`, if given, replace the graphics ROMs
    /// (`dd2.13m`...) and the sample ROMs (`dd2.11m`, `dd2.12m`), which are then not read.
    pub fn from_roms(romdir: &Path, gfx: Option<TileAtlas>, samples: Option<Vec<u8>>) -> io::Result<Self> {
        let (program, decrypted_program) = load_program(romdir)?;
        let graphics = match gfx {
            Some(g) => g,
            None => cps2_gfx::rom::load_atlas(romdir)?,
        };
        let samples = match samples {
            Some(s) => s,
            None => load_samples(romdir)?,
        };
        let dsp = std::fs::read(romdir.join("dl-1425.bin"))?;
        Ok(Contents { program, decrypted_program, graphics, audio: load_audio_region(romdir)?, dsp, samples })
    }
}

/// What the game machine holds: the graphics, the data of the Rust sound driver, the tables of the QSound DSP and the
/// sample memory. No program ROM (the game reads `ddsom_data::GameData`), no Z80 program, no DSP program: from the
/// ROMs ([`GameContents::from_roms`]) or from the assets (`ddsom_assets::game_contents`).
#[derive(Clone)]
pub struct GameContents {
    /// Graphics, decoded into 8-bit pens.
    pub graphics: TileAtlas,
    /// Everything the sound driver reads (`sound/music.json`, or read from the Z80 region of the ROMs).
    pub sound: ddsom_sound::SoundData,
    /// Tables of the DSP program (`sound/qsound_dsp.json`, or read from `dl-1425.bin`).
    pub dsp: qsound::DspTables,
    /// QSound sample memory (4 MB).
    pub samples: Vec<u8>,
}

impl GameContents {
    /// From the ROMs: the regions are read, then reduced to the data the game uses.
    pub fn from_roms(romdir: &Path) -> io::Result<Self> {
        Ok(GameContents {
            graphics: cps2_gfx::rom::load_atlas(romdir)?,
            sound: ddsom_sound::SoundData::from_region(&load_audio_region(romdir)?),
            dsp: qsound::DspTables::from_program(&std::fs::read(romdir.join("dl-1425.bin"))?),
            samples: load_samples(romdir)?,
        })
    }
}

impl Machine {
    /// Loads ddsom from a folder of extracted ROMs. `eeprom`: MAME nvram contents (128 bytes).
    pub fn load_ddsom(romdir: &Path, eeprom: Option<&[u8]>) -> io::Result<Self> {
        Self::load_ddsom_with(romdir, eeprom, None, None)
    }

    /// Like `load_ddsom`, but `gfx` (tile atlas) and `samples` (QSound sample bank, 4 MB)
    /// replace, if given, the ROMs `dd2.13m`... and `dd2.11m`/`dd2.12m`, which are then not read
    /// (see `ddsom_assets::tile_atlas`).
    pub fn load_ddsom_with(
        romdir: &Path,
        eeprom: Option<&[u8]>,
        gfx: Option<TileAtlas>,
        samples: Option<Vec<u8>>,
    ) -> io::Result<Self> {
        Ok(Self::ddsom(Contents::from_roms(romdir, gfx, samples)?, eeprom))
    }

    /// ddsom machine built from these chip contents (no file read), reset: the 68000 reads its vectors.
    pub fn ddsom(contents: Contents, eeprom: Option<&[u8]>) -> Self {
        let Contents { program, decrypted_program, graphics, audio, dsp, samples } = contents;
        let rom = bus::ProgramRom { data: program, decrypted: decrypted_program };
        let sound = sound::Sound::new(audio, qsound::QSound::new(&dsp, samples));
        let mut m = Self::build(Some(rom), sound, graphics, eeprom);
        m.reset();
        m
    }

    /// The game machine: no program ROM (its bus has none, the reset vectors are not read), the Rust sound driver on
    /// the driver's data, QSound on the DSP tables. The game must be installed on it (`ddsom_game::install_no_68k`).
    pub fn game(contents: GameContents, eeprom: Option<&[u8]>) -> Self {
        let GameContents { graphics, sound, dsp, samples } = contents;
        let sound = sound::Sound::with_driver(sound, qsound::QSound::with_tables(dsp, samples));
        Self::build(None, sound, graphics, eeprom)
    }

    fn build(rom: Option<bus::ProgramRom>, sound: sound::Sound, gfx: TileAtlas, eeprom: Option<&[u8]>) -> Self {
        let ee = match eeprom {
            Some(b) => Eeprom93c46::from_bytes(b),
            None => Eeprom93c46::new([0xffff; 64]),
        };
        Machine {
            #[cfg(feature = "m68k")]
            executor: None,
            #[cfg(feature = "m68k")]
            misses: Default::default(),
            #[cfg(feature = "m68k")]
            interp_steps: 0,
            #[cfg(feature = "m68k")]
            step_hook: None,
            #[cfg(feature = "m68k")]
            idle: None,
            step_cap: None,
            #[cfg(feature = "m68k")]
            patches: Vec::new(),
            line_work: None,
            rendering: true,
            #[cfg(feature = "m68k")]
            cpu: Cpu::new(),
            bus: Cps2Bus::new(rom, ee),
            sound,
            gfx,
            frame: 0,
            obj_buffer: vec![0; 0x1000],
            pri_ctrl_buffer: [0; 6],
            line_cycles: CPU_HZ / LINE_HZ,
            cycle_debt: 0.0,
            z80_debt: 0.0,
        }
    }

}

impl<B: Board> Machine<B> {
    /// The same machine on another board (`f` turns the bus into it: a ported game takes the RAM out of the bus).
    /// The line work, made for the old board, is removed.
    pub fn with_board<C: Board>(self, f: impl FnOnce(B) -> C) -> Machine<C> {
        Machine {
            #[cfg(feature = "m68k")]
            executor: self.executor,
            #[cfg(feature = "m68k")]
            misses: self.misses,
            #[cfg(feature = "m68k")]
            interp_steps: self.interp_steps,
            #[cfg(feature = "m68k")]
            step_hook: self.step_hook,
            #[cfg(feature = "m68k")]
            idle: self.idle,
            step_cap: self.step_cap,
            #[cfg(feature = "m68k")]
            patches: self.patches,
            line_work: None,
            rendering: self.rendering,
            #[cfg(feature = "m68k")]
            cpu: self.cpu,
            bus: f(self.bus),
            sound: self.sound,
            gfx: self.gfx,
            frame: self.frame,
            obj_buffer: self.obj_buffer,
            pri_ctrl_buffer: self.pri_ctrl_buffer,
            line_cycles: self.line_cycles,
            cycle_debt: self.cycle_debt,
            z80_debt: self.z80_debt,
        }
    }

    /// Multiplies the 68000 frequency (1.0 = arcade). Overclocked, the game always finishes its work
    /// before the end of the frame: no more slowdowns, behavior determined frame by frame.
    pub fn set_overclock(&mut self, k: f64) {
        self.line_cycles = k * CPU_HZ / LINE_HZ;
    }

    pub fn reset(&mut self) {
        #[cfg(feature = "m68k")]
        {
            self.cpu = Cpu::new();
            self.cpu.reset(self.bus.hw_mut());
        }
    }

    /// Runs the CPU for one scanline, then the Z80. The target is absolute (the overrun of a
    /// line is subtracted from the next one) to keep exactly 755.2 cycles per line on average.
    fn run_line(&mut self) {
        self.cycle_debt += self.line_cycles;
        let target = self.cycle_debt as u64;
        match self.line_work {
            Some(work) => {
                work(&mut self.bus, self.step_cap.as_mut());
                #[cfg(feature = "m68k")]
                {
                    self.cpu.cycles = self.cpu.cycles.max(target);
                }
            }
            #[cfg(feature = "m68k")]
            None => self.run_cpu(target),
            #[cfg(not(feature = "m68k"))]
            None => panic!("machine without 68000: no line work installed (cycle {target})"),
        }
        // then the Z80 over the same time slice (512 cycles at 8 MHz per line)
        let hw = self.bus.hw_mut();
        self.sound.set_reset(!hw.z80_running);
        self.z80_debt += sound::Z80_HZ / LINE_HZ;
        self.sound.run(&mut hw.qram, self.z80_debt as u64, 1.0 / LINE_HZ);
    }

    /// Runs the 68000 up to cycle `target`.
    #[cfg(feature = "m68k")]
    fn run_cpu(&mut self, target: u64) {
        let bus = self.bus.hw_mut();
        while self.cpu.cycles < target {
            self.cpu.irq_level = bus.irq_level();
            if self.idle.is_some_and(|idle| idle(&self.cpu, bus)) && self.cpu.irq_level == 0 {
                self.cpu.cycles = target;
                break;
            }
            let mask = ((self.cpu.sr >> 8) & 7) as u8;
            let plain = self.cpu.irq_level <= mask && !self.cpu.stopped && !self.cpu.trace_pending
                && self.cpu.sr & m68k::SR_T == 0;
            if let Some(cap) = self.step_cap.as_mut().filter(|cap| plain && cap.pc == self.cpu.pc) {
                if !cap.passage() {
                    // step held back until the next frame (time passes, the vblank interrupt will come)
                    self.cpu.cycles = target;
                    break;
                }
            }
            if plain && self.patches.iter().any(|p| p.entry == self.cpu.pc && (p.apply)(&mut self.cpu, bus)) {
                continue;
            }
            if let Some(run) = self.executor {
                if plain {
                    if run(self.cpu.pc, &mut self.cpu, bus, target).is_some() {
                        continue;
                    }
                    *self.misses.entry(self.cpu.pc).or_default() += 1;
                }
            }
            let pc = self.cpu.pc;
            self.cpu.step(bus);
            self.interp_steps += 1;
            if let Some(h) = self.step_hook.as_mut() {
                h(&self.cpu, bus, pc);
            }
        }
    }

    /// Runs one frame like MAME: at reset the beam is at the start of vblank (line 240) and
    /// MAME's frame notification falls at the start of each vblank. A frame thus runs
    /// lines 240..261 then 0..239; it ends on the vblank event (screen render,
    /// object table latch, IRQ2), handled at the start of the next frame.
    /// Returns the 384x224 RGB frame.
    pub fn run_frame(&mut self) -> Vec<[u8; 3]> {
        if let Some(cap) = self.step_cap.as_mut() {
            cap.count = 0;
        }
        self.bus.hw_mut().frame = self.frame;
        let frame_end = (self.cycle_debt + self.line_cycles * LINES as f64) as u64;
        for i in 0..LINES {
            let line = (VBLANK_LINE + i) % LINES;
            self.bus.hw_mut().scanline = line;
            self.raster_tick(line);
            // armed raster counters (reload < number of lines): an IRQ may fall on
            // every line; otherwise the next possible interrupt is the end-of-frame vblank
            let raster = self.bus.hw_mut().raster_reload[..2].iter().any(|&r| (r as u32) < LINES);
            self.bus.hw_mut().irq_horizon = if raster { (self.cycle_debt + self.line_cycles) as u64 } else { frame_end };
            self.run_line();
        }
        let image = if self.rendering { self.render() } else { Vec::new() };
        let bank = self.bus.hw_mut().objram_bank as usize & 1;
        self.obj_buffer.copy_from_slice(&self.bus.hw().objram[bank]);
        self.pri_ctrl_buffer = self.bus.hw_mut().output;
        self.bus.hw_mut().irq_vblank = true;
        self.frame += 1;
        image
    }

    /// CPS-B raster counters (cps_state::raster_scanline). IRQ4 is triggered without delay.
    fn raster_tick(&mut self, line: u32) {
        let b = self.bus.hw_mut();
        for i in 0..2 {
            if line == 0 {
                b.raster_counter[i] = b.raster_reload[i];
            } else {
                b.raster_counter[i] = b.raster_counter[i].wrapping_sub(1) & 0x1ff;
            }
            if b.raster_counter[i] == 0 {
                b.irq_raster = true;
            }
        }
        b.raster_counter[2] = b.raster_reload[2];
    }

    pub fn render(&self) -> Vec<[u8; 3]> {
        let b = self.bus.hw();
        let v = cps2_gfx::VideoState {
            gfxram: &b.gfxram,
            cps_a: &b.cps_a,
            cps_b: &b.cps_b,
            objram: &self.obj_buffer,
            output: &self.pri_ctrl_buffer,
            palette_ram: Some(&b.palette),
        };
        cps2_gfx::render(&v, &self.gfx)
    }
}
