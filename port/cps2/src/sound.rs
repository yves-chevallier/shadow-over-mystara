//! CPS2 sound subsystem: Z80 at 8 MHz + QSound (MAME's qsound_sub_map).
//!
//! Z80: 0000-7FFF ROM, 8000-BFFF ROM bank, C000-CFFF RAM shared with the 68000,
//! D000-D002 QSound writes, D003 bank, D007 QSound status, F000-FFFF RAM.
//!
//! Option: the Z80 program can be replaced by the Rust driver of the `ddsom-sound` crate
//! (`Sound::use_rust_driver`). It receives one interrupt per 250 Hz period, reads and writes the
//! shared RAM and writes to the QSound exactly like the Z80, without the 68000 seeing any
//! difference.

use qsound::QSound;

/// Z80 : 8 MHz.
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

pub struct Sound {
    /// Event log, if enabled.
    pub trace: Option<Vec<Event>>,
    /// Callback before each Z80 instruction (coverage), if enabled.
    pub pc_hook: Option<Box<dyn FnMut(&z80::Z80)>>,
    pub z80: z80::Z80,
    /// audiocpu region (0x50000): 0-7FFF fixed, 0x4000 banks from 0x10000.
    pub rom: Vec<u8>,
    pub bank: usize,
    pub ram: Vec<u8>, // F000-FFFF
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
    irq_acked: bool,
}

/// Z80 bus view: the shared RAM belongs to the 68000 bus.
pub struct Z80Bus<'a> {
    pub s: &'a mut Sound,
    pub qram: &'a mut [u8],
}

impl Sound {
    pub fn new(rom: Vec<u8>, qsound: QSound) -> Self {
        let mut z80 = z80::Z80::new();
        z80.reset();
        Sound {
            trace: None,
            pc_hook: None,
            z80,
            rom,
            bank: 0,
            ram: vec![0; 0x1000],
            qsound,
            in_reset: false,
            next_irq: Z80_IRQ_PERIOD,
            driver: None,
            driver_restart: false,
            audio: Vec::new(),
            sample_acc: 0.0,
            irq_acked: false,
        }
    }

    pub fn set_reset(&mut self, asserted: bool) {
        if asserted && !self.in_reset {
            self.z80.reset();
        }
        if !asserted && self.in_reset {
            self.driver_restart = true;
        }
        self.in_reset = asserted;
    }

    /// Replaces the Z80 program with the Rust driver. To be called before the first frame.
    pub fn use_rust_driver(&mut self) {
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
        self.z80.cycles = self.z80.cycles.max(target);
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
            if let ddsom_sound::Write::Qsound { reg, data } = w {
                self.qsound.write(0, (data >> 8) as u8);
                self.qsound.write(1, data as u8);
                self.qsound.write(2, reg);
            }
        }
    }

    /// Runs the Z80 up to absolute cycle `target`, then produces the QSound samples
    /// corresponding to `seconds` of machine time.
    pub fn run(&mut self, qram: &mut [u8], target: u64, seconds: f64) {
        if self.in_reset {
            // clock stopped on the program side, but time passes
            if self.z80.cycles < target {
                self.z80.cycles = target;
            }
            while self.next_irq <= target {
                self.next_irq += Z80_IRQ_PERIOD;
            }
        }
        if self.driver.is_some() {
            self.run_driver(qram, target);
        }
        // the Z80 is taken out of Sound for the duration of the run (the bus borrows Sound)
        let mut cpu = std::mem::take(&mut self.z80);
        while self.driver.is_none() && cpu.cycles < target {
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
        self.z80 = cpu;
        self.sample_acc += seconds * qsound::SAMPLE_RATE;
        while self.sample_acc >= 1.0 {
            self.sample_acc -= 1.0;
            self.qsound.update_sample();
            self.audio.push(self.qsound.out);
        }
    }
}

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
            }
            0xd000..=0xd002 => self.s.qsound.write((a - 0xd000) as u8, v),
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
