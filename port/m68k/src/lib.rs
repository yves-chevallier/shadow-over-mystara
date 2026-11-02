//! Motorola 68000 interpreter.
//!
//! Serves as an oracle for the recompiler and as a safety net for non-recompiled code.
//! Semantics validated against the SingleStepTests/m68000 tests (generated with MAME's core).
//! No modeling of address errors or of exact bus timing.

pub mod decode;
pub mod exec;
pub mod timing;

pub use decode::{decode, Ea, Instr, Op, Size};

pub const SR_C: u16 = 0x01;
pub const SR_V: u16 = 0x02;
pub const SR_Z: u16 = 0x04;
pub const SR_N: u16 = 0x08;
pub const SR_X: u16 = 0x10;
pub const SR_S: u16 = 0x2000;
pub const SR_T: u16 = 0x8000;
/// SR bits that exist on the 68000.
pub const SR_MASK: u16 = 0xa71f;

/// Bus as seen by the CPU. Received addresses are already reduced to 24 bits.
#[cfg(feature = "rom-trace")]
thread_local! {
    /// True while a 68000 instruction is executing (`Cpu::step`), false in native Rust code.
    pub static IN_STEP: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub trait Bus: decode::Fetch {
    fn read8(&mut self, addr: u32) -> u8;
    fn read16(&mut self, addr: u32) -> u16;
    fn write8(&mut self, addr: u32, v: u8);
    fn write16(&mut self, addr: u32, v: u16);
    /// Data read with PC-relative addressing (program space: decrypted on CPS2).
    fn read_prog16(&mut self, addr: u32) -> u16 {
        self.fetch16(addr)
    }
    /// RESET instruction: resets the peripherals.
    fn reset_devices(&mut self) {}
    /// Acquittement d'interruption (niveau 1..7).
    fn irq_ack(&mut self, _level: u8) {}
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Cpu {
    pub d: [u32; 8],
    /// a[7] is the active stack (SSP in supervisor mode, USP otherwise).
    pub a: [u32; 8],
    pub pc: u32,
    pub sr: u16,
    /// Inactive stack (USP in supervisor mode, SSP in user mode).
    pub other_sp: u32,
    pub stopped: bool,
    /// Interrupt level presented by the hardware (0 = none).
    pub irq_level: u8,
    /// Elapsed cycles (estimate).
    pub cycles: u64,
    /// Pending trace exception: taken at the start of the next instruction.
    pub trace_pending: bool,
    /// Cycle counter at the start of the current instruction (to fix the duration of exceptions).
    pub op_start_cycles: u64,
}

pub(crate) const ADDR_MASK: u32 = 0x00ff_ffff;

impl Cpu {
    pub fn new() -> Self {
        Cpu { sr: 0x2700, ..Default::default() }
    }

    /// Hardware reset: SSP and PC read from addresses 0 and 4.
    pub fn reset<B: Bus>(&mut self, bus: &mut B) {
        self.sr = 0x2700;
        self.stopped = false;
        self.a[7] = (bus.fetch16(0) as u32) << 16 | bus.fetch16(2) as u32;
        self.pc = (bus.fetch16(4) as u32) << 16 | bus.fetch16(6) as u32;
        // reset sequence: vector reads and prefetch (~40 cycles)
        self.cycles += 40;
    }

    pub fn supervisor(&self) -> bool {
        self.sr & SR_S != 0
    }
    pub fn usp(&self) -> u32 {
        if self.supervisor() { self.other_sp } else { self.a[7] }
    }
    pub fn ssp(&self) -> u32 {
        if self.supervisor() { self.a[7] } else { self.other_sp }
    }
    pub fn set_usp(&mut self, v: u32) {
        if self.supervisor() { self.other_sp = v } else { self.a[7] = v }
    }
    pub fn set_ssp(&mut self, v: u32) {
        if self.supervisor() { self.a[7] = v } else { self.other_sp = v }
    }

    /// Writes SR, switching stacks if the S bit changes.
    pub fn set_sr(&mut self, v: u16) {
        let v = v & SR_MASK;
        if (v ^ self.sr) & SR_S != 0 {
            std::mem::swap(&mut self.a[7], &mut self.other_sp);
        }
        self.sr = v;
    }

    pub fn ccr(&self) -> u16 {
        self.sr & 0x1f
    }
    pub fn set_ccr(&mut self, v: u16) {
        self.sr = (self.sr & 0xff00) | (v & 0x1f);
    }

    /// Executes one instruction (or takes an interrupt). Returns the executed instruction, if
    /// one was executed.
    pub fn step<B: Bus>(&mut self, bus: &mut B) -> Option<Instr> {
        #[cfg(feature = "rom-trace")]
        {
            IN_STEP.set(true);
            let r = self.step_inner(bus);
            IN_STEP.set(false);
            r
        }
        #[cfg(not(feature = "rom-trace"))]
        self.step_inner(bus)
    }

    fn step_inner<B: Bus>(&mut self, bus: &mut B) -> Option<Instr> {
        let mask = ((self.sr >> 8) & 7) as u8;
        // Level 7 (non-maskable, edge-triggered) is not used by the CPS2.
        if self.irq_level > mask {
            let level = self.irq_level;
            self.stopped = false;
            bus.irq_ack(level);
            self.exception(bus, 24 + level as u32, self.pc);
            self.sr = (self.sr & !0x0700) | ((level as u16) << 8);
            self.cycles += 44;
            return None;
        }
        if self.trace_pending {
            self.trace_pending = false;
            self.exception(bus, 9, self.pc);
            return None;
        }
        if self.stopped {
            self.cycles += 4;
            return None;
        }
        let trace = self.sr & SR_T != 0;
        let ins = decode::decode(bus, self.pc);
        self.pc = self.pc.wrapping_add(ins.len);
        self.cycles += timing::base_cycles(&ins) as u64;
        self.op_start_cycles = self.cycles - timing::base_cycles(&ins) as u64;
        let excepted = self.exec(bus, &ins);
        self.trace_pending = trace && !excepted;
        Some(ins)
    }

    /// Takes a group 1/2 exception: enters supervisor mode, pushes PC then SR.
    pub(crate) fn exception<B: Bus>(&mut self, bus: &mut B, vector: u32, pc: u32) {
        let old = self.sr;
        self.set_sr((old | SR_S) & !SR_T);
        self.push32(bus, pc);
        self.push16(bus, old);
        let v = vector * 4;
        self.pc = (bus.read16(v) as u32) << 16 | bus.read16(v + 2) as u32;
    }

    pub(crate) fn push16<B: Bus>(&mut self, bus: &mut B, v: u16) {
        self.a[7] = self.a[7].wrapping_sub(2);
        bus.write16(self.a[7] & ADDR_MASK, v);
    }
    pub(crate) fn push32<B: Bus>(&mut self, bus: &mut B, v: u32) {
        self.a[7] = self.a[7].wrapping_sub(4);
        let a = self.a[7] & ADDR_MASK;
        bus.write16(a, (v >> 16) as u16);
        bus.write16((a + 2) & ADDR_MASK, v as u16);
    }
    pub(crate) fn pop16<B: Bus>(&mut self, bus: &mut B) -> u16 {
        let v = bus.read16(self.a[7] & ADDR_MASK);
        self.a[7] = self.a[7].wrapping_add(2);
        v
    }
    pub(crate) fn pop32<B: Bus>(&mut self, bus: &mut B) -> u32 {
        let hi = self.pop16(bus) as u32;
        (hi << 16) | self.pop16(bus) as u32
    }
}
