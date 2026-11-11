//! CPS2 68000 memory map (MAME's cps2_state::cps2_map).

use std::sync::Arc;

#[cfg(feature = "m68k")]
use m68k::decode::Fetch;

use crate::eeprom::Eeprom93c46;

/// Panel inputs (active low, as on the hardware).
#[derive(Clone, Copy, Debug)]
pub struct Inputs {
    pub in0: u16,
    pub in1: u16,
    /// Bits 1..15 of IN2 (bit 0 comes from the EEPROM).
    pub in2: u16,
}

impl Default for Inputs {
    fn default() -> Self {
        Inputs { in0: 0xffff, in1: 0xffff, in2: 0xffff }
    }
}

/// State of a ported game kept outside the emulated RAM (for example ddsom's Rust task table): opaque to
/// the machine, copied with the bus.
pub trait Extension: std::any::Any + Send {
    fn clone_box(&self) -> Box<dyn Extension>;
    fn as_any(&self) -> &dyn std::any::Any;
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any>;
}

impl<T: std::any::Any + Send + Clone> Extension for T {
    fn clone_box(&self) -> Box<dyn Extension> {
        Box::new(self.clone())
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self
    }
}

impl Clone for Box<dyn Extension> {
    fn clone(&self) -> Self {
        (**self).clone_box()
    }
}

#[derive(Clone)]
pub struct Cps2Bus {
    /// The program ROM, shared between bus copies. `None` on the game machine, which reads its data through the typed
    /// tables (`ddsom_data::GameData`): only the reference machine and the tools run the original program.
    pub rom: Option<Arc<ProgramRom>>,
    pub ram: Vec<u16>,       // FF0000-FFFFFF
    pub extra_ram: Vec<u16>, // 660000-664001
    pub gfxram: Vec<u16>,    // 900000-92FFFF
    pub objram: [Vec<u16>; 2],
    pub objram_bank: u16,
    pub output: [u16; 6], // 400000-40000B
    pub cps_a: [u16; 32],
    pub cps_b: [u16; 32],
    /// RAM shared with the Z80 (C000-CFFF on the Z80 side), one byte per word on the 68000 side.
    pub qram: Vec<u8>,
    pub eeprom: Eeprom93c46,
    pub eeprom_out: u16,
    pub inputs: Inputs,
    /// Palette copied on write to CPS-A register 0x0a (6 pages of 0x200 words).
    pub palette: Vec<u16>,
    /// Requested vblank/raster IRQ line (68000 level), cleared on acknowledge.
    pub irq_vblank: bool,
    pub irq_raster: bool,
    /// CPS-B raster counters (registers 0x0e/0x10/0x12).
    pub raster_counter: [u16; 3],
    pub raster_reload: [u16; 3],
    /// Current line (for reading the raster counters).
    pub scanline: u32,
    /// Frame in progress since power-on (`Machine::frame`): with `scanline`, the beam position.
    pub frame: u64,
    /// The Z80 is held in reset while bit 3 of 804041 is 0.
    pub z80_running: bool,
    /// First CPU cycle at which an interrupt can be raised (end of frame, or end of line if the
    /// raster counters are armed). An atomic operation that completes before it remains exact.
    pub irq_horizon: u64,
    /// State of the game ported outside the emulated RAM (see [`Extension`]).
    pub ext: Option<Box<dyn Extension>>,
}

/// The 68000 program ROM: data view (plain) and program-space view (decrypted opcodes and PC-relative reads),
/// big-endian words.
pub struct ProgramRom {
    pub data: Vec<u16>,
    pub decrypted: Vec<u16>,
}

/// A read of the program ROM on a machine without ROM: the game never reads it (its data are typed tables).
#[cold]
fn no_rom(a: u32) -> ! {
    panic!("read of the program ROM at {a:#08x} on a machine without ROM (the game reads its data through `GameData`)")
}

impl Cps2Bus {
    /// A bus with the program ROM (`Some`: reference machine and tools) or without (the game).
    pub fn new(rom: Option<ProgramRom>, eeprom: Eeprom93c46) -> Self {
        Cps2Bus {
            rom: rom.map(Arc::new),
            ram: vec![0; 0x8000],
            extra_ram: vec![0; 0x2001],
            gfxram: vec![0; 0x18000],
            objram: [vec![0; 0x1000], vec![0; 0x1000]],
            objram_bank: 0,
            output: [0; 6],
            cps_a: [0; 32],
            cps_b: [0; 32],
            qram: vec![0; 0x1000],
            eeprom,
            eeprom_out: 0,
            inputs: Inputs::default(),
            palette: vec![0; 6 * 0x200],
            irq_vblank: false,
            irq_raster: false,
            raster_counter: [0; 3],
            raster_reload: [0x1ff, 0x1ff, 0],
            scanline: 0,
            frame: 0,
            // the Z80 starts at power-on; the 68000 puts it in reset at boot
            z80_running: true,
            irq_horizon: 0,
            ext: None,
        }
    }

    /// A bus without memory (no RAM, video memory nor palette: empty buffers, no allocation), to stand in for a bus
    /// moved elsewhere for a moment (`std::mem::replace`).
    pub fn detached() -> Self {
        Cps2Bus {
            rom: None,
            ram: Vec::new(),
            extra_ram: Vec::new(),
            gfxram: Vec::new(),
            objram: [Vec::new(), Vec::new()],
            objram_bank: 0,
            output: [0; 6],
            cps_a: [0; 32],
            cps_b: [0; 32],
            qram: Vec::new(),
            eeprom: Eeprom93c46::new([0xffff; 64]),
            eeprom_out: 0,
            inputs: Inputs::default(),
            palette: Vec::new(),
            irq_vblank: false,
            irq_raster: false,
            raster_counter: [0; 3],
            raster_reload: [0; 3],
            scanline: 0,
            frame: 0,
            z80_running: false,
            irq_horizon: 0,
            ext: None,
        }
    }

    /// Interrupt level presented to the 68000 (IPL1 = vblank -> 2, IPL2 = raster -> 4, both -> 6).
    pub fn irq_level(&self) -> u8 {
        (self.irq_vblank as u8) * 2 + (self.irq_raster as u8) * 4
    }

    fn latch_palette(&mut self) {
        let base = ((self.cps_a[0x0a / 2] as usize * 256) & !0x3ff) & 0x3ffff;
        let n = self.gfxram.len();
        let start = base / 2 % n;
        // copy in two slices if the palette overflows the end of the GFX RAM (circular addressing)
        let len = self.palette.len();
        let first = len.min(n - start);
        self.palette[..first].copy_from_slice(&self.gfxram[start..start + first]);
        for i in first..len {
            self.palette[i] = self.gfxram[(start + i) % n];
        }
    }

    fn read_word(&mut self, a: u32) -> u16 {
        match a {
            0x000000..=0x3fffff => {
                #[cfg(feature = "rom-trace")]
                rom_trace::note(false, a);
                match &self.rom {
                    Some(rom) => rom.data[(a >> 1) as usize],
                    None => no_rom(a),
                }
            }
            0x400000..=0x40000b => self.output[((a - 0x400000) >> 1) as usize],
            0x618000..=0x619fff => 0xff00 | self.qram[((a - 0x618000) >> 1) as usize] as u16,
            0x660000..=0x664001 => self.extra_ram[((a - 0x660000) >> 1) as usize],
            0x708000..=0x70ffff => {
                let bank = (self.objram_bank ^ 1) as usize & 1;
                self.objram[bank][((a & 0x1fff) >> 1) as usize]
            }
            0x800140..=0x80017f => self.cps_b_read(((a - 0x800140) >> 1) as usize),
            0x804140..=0x80417f => self.cps_b_read(((a - 0x804140) >> 1) as usize),
            0x804000..=0x804001 => self.inputs.in0,
            0x804010..=0x804011 => self.inputs.in1,
            0x804020..=0x804021 => (self.inputs.in2 & !1) | self.eeprom.do_read() as u16,
            0x804030..=0x804031 => 0xe021, // digital volume at maximum, no additional RAM
            0x8040b0..=0x8040b3 => 0xffff, // DIP (development boards only)
            0x900000..=0x92ffff => self.gfxram[((a - 0x900000) >> 1) as usize],
            0xff0000..=0xffffff => self.ram[((a - 0xff0000) >> 1) as usize],
            _ => 0, // unmapped
        }
    }

    fn cps_b_read(&self, off: usize) -> u16 {
        match off {
            // hardware multiplier (CPS_B_21_DEF: factors 0x00/0x02, result 0x04/0x06)
            2 => (self.cps_b[0] as u32 * self.cps_b[1] as u32) as u16,
            3 => ((self.cps_b[0] as u32 * self.cps_b[1] as u32) >> 16) as u16,
            7 => ((self.raster_counter[2] << 1) & 0x1fe) | (self.cps_b[7] & 1),
            8 | 9 => self.raster_counter[off & 1] & 0x1ff,
            _ => 0xffff,
        }
    }

    fn cps_b_write(&mut self, off: usize, v: u16, mask: u16) {
        let v = (self.cps_b[off] & !mask) | (v & mask);
        self.cps_b[off] = v;
        match off {
            7 => self.raster_reload[2] = (v >> 1) & 0xff,
            8 | 9 => {
                self.raster_reload[off & 1] = v & 0x1ff;
                if v & 0x8000 != 0 {
                    self.raster_counter[off & 1] = self.raster_reload[off & 1];
                }
            }
            _ => {}
        }
    }

    /// Writes a word with a byte mask (COMBINE_DATA).
    fn write_word(&mut self, a: u32, v: u16, mask: u16) {
        let comb = |old: u16| (old & !mask) | (v & mask);
        match a {
            0x400000..=0x40000b => {
                let i = ((a - 0x400000) >> 1) as usize;
                self.output[i] = comb(self.output[i]);
            }
            0x618000..=0x619fff => {
                if mask & 0xff != 0 {
                    self.qram[((a - 0x618000) >> 1) as usize] = v as u8;
                }
            }
            0x660000..=0x664001 => {
                let i = ((a - 0x660000) >> 1) as usize;
                self.extra_ram[i] = comb(self.extra_ram[i]);
            }
            0x700000..=0x701fff => {
                let bank = self.objram_bank as usize & 1;
                let i = ((a & 0x1fff) >> 1) as usize;
                self.objram[bank][i] = comb(self.objram[bank][i]);
            }
            0x708000..=0x70ffff => {
                let bank = (self.objram_bank ^ 1) as usize & 1;
                let i = ((a & 0x1fff) >> 1) as usize;
                self.objram[bank][i] = comb(self.objram[bank][i]);
            }
            0x800100..=0x80013f | 0x804100..=0x80413f => {
                let i = ((a & 0x3f) >> 1) as usize;
                self.cps_a[i] = comb(self.cps_a[i]);
                if i == 0x0a / 2 {
                    self.latch_palette();
                }
            }
            0x800140..=0x80017f | 0x804140..=0x80417f => self.cps_b_write(((a & 0x3f) >> 1) as usize, v, mask),
            0x804040..=0x804041 => {
                self.eeprom_out = comb(self.eeprom_out);
                if mask & 0xff00 != 0 {
                    let d = self.eeprom_out;
                    self.eeprom.write_lines(d & 0x1000 != 0, d & 0x2000 != 0, d & 0x4000 != 0);
                }
                if mask & 0x00ff != 0 {
                    self.z80_running = self.eeprom_out & 0x08 != 0;
                }
            }
            0x8040e0..=0x8040e1 => {
                if mask & 0xff != 0 {
                    self.objram_bank = v & 1;
                }
            }
            0x900000..=0x92ffff => {
                let i = ((a - 0x900000) >> 1) as usize;
                self.gfxram[i] = comb(self.gfxram[i]);
            }
            0xff0000..=0xffffff => {
                let i = ((a - 0xff0000) >> 1) as usize;
                self.ram[i] = comb(self.ram[i]);
            }
            _ => {}
        }
    }
}

/// Accesses of the 68000 address space, without the processor (the game without 68000 uses them directly; the
/// processor goes through the `m68k::Bus` implementation, which forwards to them).
impl Cps2Bus {
    /// Word of the program space (decrypted program below 0x400000).
    pub fn fetch16(&mut self, a: u32) -> u16 {
        let a = a & 0xff_ffff;
        if a < 0x400000 {
            #[cfg(feature = "rom-trace")]
            rom_trace::note(true, a);
            match &self.rom {
                Some(rom) => rom.decrypted[(a >> 1) as usize],
                None => no_rom(a),
            }
        } else {
            self.read_word(a & !1)
        }
    }
    pub fn read8(&mut self, a: u32) -> u8 {
        let w = self.read_word(a & !1);
        if a & 1 == 0 { (w >> 8) as u8 } else { w as u8 }
    }
    pub fn read16(&mut self, a: u32) -> u16 {
        self.read_word(a & !1)
    }
    pub fn write8(&mut self, a: u32, v: u8) {
        if a & 1 == 0 {
            self.write_word(a & !1, (v as u16) << 8, 0xff00);
        } else {
            self.write_word(a & !1, v as u16, 0x00ff);
        }
    }
    pub fn write16(&mut self, a: u32, v: u16) {
        self.write_word(a & !1, v, 0xffff);
    }
    /// Interrupt acknowledge: clears IPL1 and IPL2 (cps_state::irqack_r).
    pub fn irq_ack(&mut self) {
        self.irq_vblank = false;
        self.irq_raster = false;
    }
}

#[cfg(feature = "m68k")]
impl Fetch for Cps2Bus {
    fn fetch16(&mut self, a: u32) -> u16 {
        Cps2Bus::fetch16(self, a)
    }
}

#[cfg(feature = "m68k")]
impl m68k::Bus for Cps2Bus {
    fn read8(&mut self, a: u32) -> u8 {
        Cps2Bus::read8(self, a)
    }
    fn read16(&mut self, a: u32) -> u16 {
        Cps2Bus::read16(self, a)
    }
    fn write8(&mut self, a: u32, v: u8) {
        Cps2Bus::write8(self, a, v)
    }
    fn write16(&mut self, a: u32, v: u16) {
        Cps2Bus::write16(self, a, v)
    }
    fn irq_ack(&mut self, _level: u8) {
        Cps2Bus::irq_ack(self)
    }
}

/// Program ROM reads by the Rust code: words read, per view (data, program). Reads of the
/// 68000 instructions (`m68k::IN_STEP`) are not counted. Enabled by the `rom-trace` feature.
#[cfg(feature = "rom-trace")]
pub mod rom_trace {
    use std::cell::RefCell;

    /// Number of reads of each word from 0x000000 to 0x3fffff: `[data, program]`.
    pub struct Reads(pub [Vec<u32>; 2]);

    thread_local! {
        static READS: RefCell<Reads> = RefCell::new(Reads([vec![0; 0x200000], vec![0; 0x200000]]));
    }

    thread_local! {
        static HOOK: std::cell::Cell<Option<fn(bool, u32)>> = const { std::cell::Cell::new(None) };
    }

    /// `hook(program, address)` is called on the first read of each word (to find its origin).
    pub fn set_hook(hook: fn(bool, u32)) {
        HOOK.set(Some(hook));
    }

    pub fn note(program: bool, a: u32) {
        if !m68k::IN_STEP.get() {
            let first = READS.with(|r| {
                let c = &mut r.borrow_mut().0[program as usize][(a >> 1) as usize & 0x1fffff];
                *c += 1;
                *c == 1
            });
            if first {
                if let Some(h) = HOOK.get() {
                    h(program, a & !1);
                }
            }
        }
    }

    /// Current counters (reset to zero).
    pub fn take() -> Reads {
        READS.with(|r| std::mem::replace(&mut *r.borrow_mut(), Reads([vec![0; 0x200000], vec![0; 0x200000]])))
    }
}
