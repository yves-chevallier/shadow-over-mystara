//! CPS2 sound subsystem: sound driver + QSound (MAME's qsound_sub_map).
//!
//! The game's sound driver is the Rust driver of the `ddsom-sound` crate: it receives one interrupt per 250 Hz
//! period, reads and writes the shared RAM and writes to the QSound exactly like the original Z80 program, without
//! the 68000 seeing any difference.
//!
//! With the `z80` feature (reference machine and tools), the original Z80 program can run instead, on an emulated
//! Z80 at 8 MHz; it is then the default until `Sound::use_rust_driver`. Z80 map: 0000-7FFF ROM, 8000-BFFF ROM bank,
//! C000-CFFF RAM shared with the 68000, D000-D002 QSound writes, D003 bank, D007 QSound status, F000-FFFF RAM.

use qsound::QSound;

/// Sound CPU clock (Z80): 8 MHz.
pub const Z80_HZ: f64 = 8_000_000.0;
/// Periodic interrupt: 8 MHz / 32000 = 250 Hz (irq0_line_hold).
pub const Z80_IRQ_PERIOD: u64 = 32_000;

/// Observable Z80 event (analysis and validation tools for the sound driver).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// Interrupt accepted.
    Irq,
    /// Z80 read from the shared RAM (address 0-0xFFF, value read).
    Read(u16, u8),
    /// Z80 write to the shared RAM.
    Write(u16, u8),
    /// Z80 write to D000-D003: (offset, byte). D000/D001 load the data latch,
    /// D002 writes the QSound register, D003 selects the ROM bank.
    Port(u8, u8),
}

/// Write made by the sound driver (Rust or Z80), in program order: what `validate` compares between the game and the
/// reference machine (`Sound::output`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Output {
    /// QSound register `reg` written with `data`.
    Qsound { reg: u8, data: u16 },
    /// Shared RAM byte (offset 0-0xFFF) written by the driver.
    Shared { offset: u16, value: u8 },
}

/// Callback before each Z80 instruction (coverage).
#[cfg(feature = "z80")]
pub type PcHook = Box<dyn FnMut(&z80::Z80)>;

pub struct Sound {
    /// Event log, if enabled.
    pub trace: Option<Vec<Event>>,
    /// Writes of the driver (QSound registers, shared RAM), if enabled.
    pub output: Option<Vec<Output>>,
    /// Callback before each Z80 instruction (coverage), if enabled.
    #[cfg(feature = "z80")]
    pub pc_hook: Option<PcHook>,
    #[cfg(feature = "z80")]
    pub z80: z80::Z80,
    /// audiocpu region (0x50000): 0-7FFF fixed, 0x4000 banks from 0x10000.
    pub rom: Vec<u8>,
    #[cfg(feature = "z80")]
    pub bank: usize,
    /// Z80 RAM (F000-FFFF).
    #[cfg(feature = "z80")]
    pub ram: Vec<u8>,
    #[cfg(feature = "z80")]
    data_latch: u16,
    /// Sound clock (8 MHz periods elapsed), the Z80's when it runs.
    pub cycles: u64,
    pub qsound: QSound,
    /// The 68000 holds the Z80 in reset (bit 3 of 804041 at 0).
    pub in_reset: bool,
    pub next_irq: u64,
    /// Rust driver replacing the Z80 program, if enabled.
    pub driver: Option<ddsom_sound::Driver>,
    /// The driver must (re)start: activation, or Z80 reset released by the 68000.
    driver_restart: bool,
    /// Stereo samples produced (to be drained by the application).
    pub audio: Vec<[i16; 2]>,
    sample_acc: f64,
    /// The Z80 acknowledged its interrupt (the line drops: irq0_line_hold).
    #[cfg(feature = "z80")]
    irq_acked: bool,
}

/// Z80 bus view: the shared RAM belongs to the 68000 bus.
#[cfg(feature = "z80")]
pub struct Z80Bus<'a> {
    pub s: &'a mut Sound,
    pub qram: &'a mut [u8],
}

impl Sound {
    /// The sound of the machine: the Rust driver, or with the `z80` feature the original Z80 program (until
    /// `use_rust_driver`).
    pub fn new(rom: Vec<u8>, qsound: QSound) -> Self {
        let mut sound = Sound {
            trace: None,
            output: None,
            #[cfg(feature = "z80")]
            pc_hook: None,
            #[cfg(feature = "z80")]
            z80: {
                let mut z80 = z80::Z80::new();
                z80.reset();
                z80
            },
            rom,
            #[cfg(feature = "z80")]
            bank: 0,
            #[cfg(feature = "z80")]
            ram: vec![0; 0x1000],
            #[cfg(feature = "z80")]
            data_latch: 0,
            cycles: 0,
            qsound,
            in_reset: false,
            next_irq: Z80_IRQ_PERIOD,
            driver: None,
            driver_restart: false,
            audio: Vec::new(),
            sample_acc: 0.0,
            #[cfg(feature = "z80")]
            irq_acked: false,
        };
        if !cfg!(feature = "z80") {
            sound.use_rust_driver();
        }
        sound
    }

    pub fn set_reset(&mut self, asserted: bool) {
        #[cfg(feature = "z80")]
        if asserted && !self.in_reset {
            self.z80.reset();
        }
        if !asserted && self.in_reset {
            self.driver_restart = true;
        }
        self.in_reset = asserted;
    }

    /// Replaces the Z80 program with the Rust driver (always the case without the `z80` feature). To be called before
    /// the first frame.
    pub fn use_rust_driver(&mut self) {
        if self.driver.is_some() {
            return;
        }
        self.driver = Some(ddsom_sound::Driver::new(ddsom_sound::Rom::from_region(self.rom.clone())));
        self.driver_restart = true;
    }

    /// One 250 Hz period of the Rust driver: time passes, the driver answers the interrupts.
    fn run_driver(&mut self, qram: &mut [u8], target: u64) {
        let Some(driver) = self.driver.as_mut() else { return };
        let mut writes = Vec::new();
        if self.driver_restart && !self.in_reset {
            self.driver_restart = false;
            driver.reset(qram, &mut writes);
        }
        self.cycles = self.cycles.max(target);
        // An interrupt that falls exactly at the end of the slice is taken by the Z80 at the
        // start of the next one, that is after the 68000 writes on the next line.
        while self.next_irq < target {
            self.next_irq += Z80_IRQ_PERIOD;
            if !self.in_reset {
                if let (Some(t), 0x88) = (self.trace.as_mut(), qram[0xffd]) {
                    t.push(Event::Irq);
                    t.push(Event::Read(0xf, qram[0xf]));
                }
                driver.irq_request(qram, &mut writes);
            }
        }
        for w in writes {
            match w {
                ddsom_sound::Write::Qsound { reg, data } => {
                    self.qsound.write(0, (data >> 8) as u8);
                    self.qsound.write(1, data as u8);
                    self.qsound.write(2, reg);
                    if let Some(o) = self.output.as_mut() {
                        o.push(Output::Qsound { reg, data });
                    }
                }
                ddsom_sound::Write::Shared { offset, value } => {
                    if let Some(o) = self.output.as_mut() {
                        o.push(Output::Shared { offset, value });
                    }
                }
            }
        }
    }

    /// Runs the sound driver up to absolute cycle `target`, then produces the QSound samples
    /// corresponding to `seconds` of machine time.
    pub fn run(&mut self, qram: &mut [u8], target: u64, seconds: f64) {
        if self.in_reset {
            // clock stopped on the program side, but time passes
            self.cycles = self.cycles.max(target);
            while self.next_irq <= target {
                self.next_irq += Z80_IRQ_PERIOD;
            }
        }
        if self.driver.is_some() {
            self.run_driver(qram, target);
        } else {
            #[cfg(feature = "z80")]
            self.run_z80(qram, target);
        }
        self.sample_acc += seconds * qsound::SAMPLE_RATE;
        while self.sample_acc >= 1.0 {
            self.sample_acc -= 1.0;
            self.qsound.update_sample();
            self.audio.push(self.qsound.out);
        }
    }

    /// The original Z80 program up to absolute cycle `target`.
    #[cfg(feature = "z80")]
    fn run_z80(&mut self, qram: &mut [u8], target: u64) {
        self.z80.cycles = self.z80.cycles.max(self.cycles);
        // the Z80 is taken out of Sound for the duration of the run (the bus borrows Sound)
        let mut cpu = std::mem::take(&mut self.z80);
        while cpu.cycles < target {
            if cpu.cycles >= self.next_irq {
                cpu.irq_line = true;
                self.next_irq += Z80_IRQ_PERIOD;
            }
            if let Some(h) = self.pc_hook.as_mut() {
                h(&cpu);
            }
            let mut bus = Z80Bus { s: self, qram };
            cpu.step(&mut bus);
            if self.irq_acked {
                cpu.irq_line = false;
                self.irq_acked = false;
            }
        }
        self.cycles = cpu.cycles;
        self.z80 = cpu;
    }
}

#[cfg(feature = "z80")]
impl z80::Bus for Z80Bus<'_> {
    fn read(&mut self, a: u16) -> u8 {
        match a {
            0x0000..=0x7fff => self.s.rom[a as usize],
            0x8000..=0xbfff => {
                let i = 0x10000 + self.s.bank * 0x4000 + (a as usize - 0x8000);
                *self.s.rom.get(i).unwrap_or(&0xff)
            }
            0xc000..=0xcfff => {
                let v = self.qram[(a - 0xc000) as usize];
                if let Some(t) = self.s.trace.as_mut() {
                    t.push(Event::Read(a - 0xc000, v));
                }
                v
            }
            0xd007 => self.s.qsound.read(),
            0xf000..=0xffff => self.s.ram[(a - 0xf000) as usize],
            _ => 0xff,
        }
    }
    fn write(&mut self, a: u16, v: u8) {
        if let (Some(t), 0xd000..=0xd003) = (self.s.trace.as_mut(), a) {
            t.push(Event::Port((a - 0xd000) as u8, v));
        }
        match a {
            0xc000..=0xcfff => {
                self.qram[(a - 0xc000) as usize] = v;
                if let Some(t) = self.s.trace.as_mut() {
                    t.push(Event::Write(a - 0xc000, v));
                }
                if let Some(o) = self.s.output.as_mut() {
                    o.push(Output::Shared { offset: a - 0xc000, value: v });
                }
            }
            0xd000 => {
                self.s.data_latch = self.s.data_latch & 0xff | u16::from(v) << 8;
                self.s.qsound.write(0, v);
            }
            0xd001 => {
                self.s.data_latch = self.s.data_latch & 0xff00 | u16::from(v);
                self.s.qsound.write(1, v);
            }
            0xd002 => {
                self.s.qsound.write(2, v);
                if let Some(o) = self.s.output.as_mut() {
                    o.push(Output::Qsound { reg: v, data: self.s.data_latch });
                }
            }
            0xd003 => {
                let bank = (v & 0x0f) as usize;
                self.s.bank = if 0x10000 + bank * 0x4000 >= self.s.rom.len() { 0 } else { bank };
            }
            0xf000..=0xffff => self.s.ram[(a - 0xf000) as usize] = v,
            _ => {}
        }
    }
    fn input(&mut self, _port: u16) -> u8 {
        0xff
    }
    fn output(&mut self, _port: u16, _v: u8) {}
    fn irq_ack(&mut self) -> u8 {
        // irq0_line_hold: the line drops at acknowledge
        self.s.irq_acked = true;
        if let Some(t) = self.s.trace.as_mut() {
            t.push(Event::Irq);
        }
        0xff
    }
}
