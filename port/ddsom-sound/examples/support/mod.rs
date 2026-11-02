#![allow(dead_code)]
//! Tools shared by the examples: comparison of write sequences and reference Z80.

use ddsom_sound::{SoundData, Write};

pub mod session;
use std::collections::VecDeque;
use std::path::Path;

/// The sound ROM as the original Z80 runs it (region) and as the Rust driver reads it (data).
#[derive(Clone)]
pub struct SoundRom {
    pub region: Vec<u8>,
    pub data: SoundData,
}

impl SoundRom {
    pub fn load(dir: &Path) -> std::io::Result<Self> {
        Ok(Self::from_region(ddsom_sound::region::load(dir)?))
    }

    pub fn from_region(region: Vec<u8>) -> Self {
        SoundRom { data: SoundData::from_region(&region), region }
    }
}

/// Writes on one side, with the number of the interrupt in which they occurred.
#[derive(Default)]
pub struct Side {
    /// Main loop writes (QSound and pan report at C005).
    pub main: VecDeque<(usize, Write)>,
    /// Command acknowledgements (C00F <- FF) from the interrupt routine.
    pub acks: VecDeque<(usize, Write)>,
    pub total: usize,
    pub qsound: usize,
}

/// Results of comparing two runs.
#[derive(Default)]
pub struct Report {
    pub real: Side,
    pub drv: Side,
    pub matched_main: usize,
    pub matched_acks: usize,
    pub first_divergence: Option<String>,
    /// Interrupts where the main loop writes match exactly.
    pub exact_intervals: usize,
    /// Interrupts where the work spills over into the next one on either side.
    pub spill_intervals: usize,
    /// Interrupts with no writes on either side.
    pub idle_intervals: usize,
    /// The Z80 was busy for too long for the Rust driver, which does not model execution
    /// time, to be compared with it (run abandoned).
    pub slow: bool,
    /// QSound registers on each side, and number of interrupts where they match at the end
    /// of the interrupt.
    regs_real: Vec<Option<u16>>,
    regs_drv: Vec<Option<u16>>,
    pub same_state_intervals: usize,
}

/// The command acknowledgement is written by the interrupt routine, at the instant of the
/// interrupt: its rank relative to the main loop writes (whose work may spill over into the
/// next interrupt) depends on the duration of the Z80 instructions.
pub fn is_ack(w: &Write) -> bool {
    matches!(w, Write::Shared { offset: 0xf, value: 0xff })
}

impl Side {
    fn push(&mut self, irq: usize, writes: Vec<Write>) {
        self.total += writes.len();
        for w in writes {
            if matches!(w, Write::Qsound { .. }) {
                self.qsound += 1;
            }
            let q = if is_ack(&w) { &mut self.acks } else { &mut self.main };
            q.push_back((irq, w));
        }
    }
}

impl Report {
    /// Files the writes of an interrupt (original Z80, Rust driver) and compares them.
    pub fn add_interval(&mut self, irq: usize, real: Vec<Write>, drv: Vec<Write>) {
        let main = |v: &[Write]| v.iter().filter(|w| !is_ack(w)).copied().collect::<Vec<_>>();
        if real.is_empty() && drv.is_empty() {
            self.idle_intervals += 1;
        } else if main(&real) == main(&drv) {
            self.exact_intervals += 1;
        }
        for (regs, writes) in [(&mut self.regs_real, &real), (&mut self.regs_drv, &drv)] {
            regs.resize(256, None);
            for w in writes {
                if let Write::Qsound { reg, data } = w {
                    regs[usize::from(*reg)] = Some(*data);
                }
            }
        }
        if self.regs_real == self.regs_drv {
            self.same_state_intervals += 1;
        }
        self.real.push(irq, real);
        self.drv.push(irq, drv);
        if self.real.main.len() != self.drv.main.len() {
            self.spill_intervals += 1;
        }
        self.drain();
    }

    /// Compares the pending writes of both sides, in order.
    fn drain(&mut self) {
        for acks in [false, true] {
            loop {
                let (r, d) = if acks {
                    (self.real.acks.front().copied(), self.drv.acks.front().copied())
                } else {
                    (self.real.main.front().copied(), self.drv.main.front().copied())
                };
                let (Some((ri, r)), Some((di, d))) = (r, d) else { break };
                if r != d && self.first_divergence.is_none() {
                    let n = if acks { self.matched_acks } else { self.matched_main };
                    self.first_divergence = Some(format!(
                        "{} #{n}: Z80 {r:?} (interrupt {ri}) / driver {d:?} (interrupt {di})",
                        if acks { "acknowledgement" } else { "write" }
                    ));
                }
                if self.first_divergence.is_some() {
                    return;
                }
                if acks {
                    self.matched_acks += 1;
                    self.real.acks.pop_front();
                    self.drv.acks.pop_front();
                } else {
                    self.matched_main += 1;
                    self.real.main.pop_front();
                    self.drv.main.pop_front();
                }
            }
        }
    }

    /// Main loop writes that one of the two sides has not yet produced when
    /// the run stops (the Z80 is in the middle of a tick, or the driver is one interrupt ahead).
    pub fn tail(&self) -> usize {
        self.real.main.len().abs_diff(self.drv.main.len())
    }

    /// Are the two sequences identical from end to end, apart from the unfinished tail?
    pub fn identical(&self) -> bool {
        self.first_divergence.is_none() && self.tail() <= MAX_TAIL
    }
}

/// A full tick of 16 voices writes at most a few dozen registers: beyond that, it is no
/// longer just a run ending in the middle of a tick.
const MAX_TAIL: usize = 120;

/// Original Z80 alone (no 68000 or QSound): runs the driver ROM and records its writes.
/// The QSound always answers "ready".
pub mod reference {
    use super::SoundRom;
    use ddsom_sound::Write;

    const IRQ_PERIOD: u64 = 32_000;

    struct Bus {
        region: Vec<u8>,
        bank: usize,
        ram: Vec<u8>,
        shared: Vec<u8>,
        latch: [u8; 2],
        out: Vec<Write>,
        irq_acked: bool,
    }

    impl z80::Bus for Bus {
        fn read(&mut self, a: u16) -> u8 {
            match a {
                0x0000..=0x7fff => self.region[a as usize],
                0x8000..=0xbfff => {
                    let i = 0x10000 + self.bank * 0x4000 + (a as usize - 0x8000);
                    *self.region.get(i).unwrap_or(&0xff)
                }
                0xc000..=0xcfff => self.shared[(a - 0xc000) as usize],
                0xd007 => 0x80,
                0xf000..=0xffff => self.ram[(a - 0xf000) as usize],
                _ => 0xff,
            }
        }
        fn write(&mut self, a: u16, v: u8) {
            match a {
                0xc000..=0xcfff => {
                    self.shared[(a - 0xc000) as usize] = v;
                    self.out.push(Write::Shared { offset: a - 0xc000, value: v });
                }
                0xd000 | 0xd001 => self.latch[(a - 0xd000) as usize] = v,
                0xd002 => self.out.push(Write::Qsound { reg: v, data: u16::from(self.latch[0]) << 8 | u16::from(self.latch[1]) }),
                0xd003 => {
                    let bank = (v & 0x0f) as usize;
                    self.bank = if 0x10000 + bank * 0x4000 >= self.region.len() { 0 } else { bank };
                }
                0xf000..=0xffff => self.ram[(a - 0xf000) as usize] = v,
                _ => {}
            }
        }
        fn input(&mut self, _port: u16) -> u8 {
            0xff
        }
        fn output(&mut self, _port: u16, _v: u8) {}
        fn irq_ack(&mut self) -> u8 {
            self.irq_acked = true;
            0xff
        }
    }

    pub struct RefZ80 {
        cpu: z80::Z80,
        bus: Bus,
        next_irq: u64,
        /// Addresses of executed instructions (coverage of the Z80 program).
        pub coverage: Vec<bool>,
        /// Number of executions of each address since the last reset (profile).
        pub hits: Vec<u32>,
        /// Debug breakpoint: prints IX and the voice record.
        pub break_at: Option<u16>,
        break_count: usize,
        /// Watches an address of the Z80 RAM (debugging).
        watch: Option<u16>,
    }

    impl RefZ80 {
        pub fn new(rom: &SoundRom) -> Self {
            let mut cpu = z80::Z80::new();
            cpu.reset();
            let bus = Bus {
                region: rom.region.clone(),
                bank: 0,
                ram: vec![0; 0x1000],
                shared: vec![0; ddsom_sound::SHARED_SIZE],
                latch: [0; 2],
                out: Vec::new(),
                irq_acked: false,
            };
            RefZ80 { cpu, bus, next_irq: IRQ_PERIOD, coverage: vec![false; 0x10000], hits: vec![0; 0x10000], break_at: std::env::var("FUZZ_BREAK").ok().and_then(|v| u16::from_str_radix(&v, 16).ok()), break_count: 0, watch: std::env::var("FUZZ_WATCH").ok().and_then(|v| u16::from_str_radix(&v, 16).ok()) }
        }

        /// Shared RAM as seen by the Z80 (the 68000 writes to it).
        pub fn shared(&mut self) -> &mut [u8] {
            &mut self.bus.shared
        }

        /// Is the main loop at rest (the Z80 has just been interrupted in its wait loop)?
        /// This is read from the return address pushed by the interrupt.
        pub fn is_idle(&self) -> bool {
            let sp = usize::from(self.cpu.sp);
            if sp < 0xf000 {
                return false;
            }
            let ret = u16::from(self.bus.ram[sp - 0xf000]) | u16::from(self.bus.ram[sp - 0xf000 + 1]) << 8;
            let pending = self.bus.ram[1] | self.bus.ram[2] | self.bus.ram[3];
            (0x017d..=0x019a).contains(&ret) && !matches!(ret, 0x0185 | 0x0190 | 0x019b) && pending == 0
        }

        /// Z80 RAM (F000-FFFF): for debugging.
        pub fn ram(&self) -> &[u8] {
            &self.bus.ram
        }

        /// Runs until the acknowledgement of the next interrupt (not included) and returns the
        /// writes made since the previous one. Interrupts requested while the Z80 masks them
        /// (initialisation) produce only a single acknowledgement.
        pub fn run_interval(&mut self) -> Vec<Write> {
            loop {
                if self.cpu.cycles >= self.next_irq {
                    self.cpu.irq_line = true;
                    self.next_irq += IRQ_PERIOD;
                }
                self.coverage[usize::from(self.cpu.pc)] = true;
                self.hits[usize::from(self.cpu.pc)] += 1;
                if let Some(b) = self.break_at {
                    if self.cpu.pc == b && std::env::var("FUZZ_BREAK_SHORT").is_ok() {
                        println!("  [{b:04x}] cycle {} f000 {:02x} f001-3 {:02x?} queue {:02x}/{:02x}", self.cpu.cycles, self.bus.ram[0], &self.bus.ram[1..4], self.bus.ram[0x15], self.bus.ram[0x16]);
                    } else if self.cpu.pc == b && self.break_count < 6 {
                        self.break_count += 1;
                        let ix = usize::from(self.cpu.ix);
                        let rec: Vec<String> = (0..0x50).map(|i| format!("{:02x}", self.bus.ram.get(ix + i - 0xf000).copied().unwrap_or(0))).collect();
                        println!("breakpoint {b:04x}: IX={ix:04x} DE={:04x} record {}", self.cpu.de(), rec.join(" "));
                    }
                }
                let watch_before = self.watch.map(|w| self.bus.ram[usize::from(w) - 0xf000]);
                let pc_before = self.cpu.pc;
                self.cpu.step(&mut self.bus);
                if let (Some(w), Some(before)) = (self.watch, watch_before) {
                    let after = self.bus.ram[usize::from(w) - 0xf000];
                    if after != before {
                        println!("  [write {w:04x}] pc {pc_before:04x} cycle {}: {before:02x} -> {after:02x}", self.cpu.cycles);
                    }
                }
                if self.bus.irq_acked {
                    self.cpu.irq_line = false;
                    self.bus.irq_acked = false;
                    return std::mem::take(&mut self.bus.out);
                }
            }
        }
    }
}
