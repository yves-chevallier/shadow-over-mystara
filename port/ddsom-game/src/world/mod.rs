//! The game's world: its work RAM, its task table and the hardware it drives (docs/fin.md, section 2, "État du
//! jeu", lot L15).
//!
//! The work RAM (FF0000-FFFFFF) stays the game state, laid out as in the original, since `validate` compares it
//! with the reference machine frame by frame; but it belongs to the game, not to the bus: [`World::read16`] and the
//! other accessors index it without address decoding. The task table (the Rust state of the original kernel,
//! `tasks::table`) is a plain field. The hardware (video memory and registers, link with the sound driver, inputs,
//! EEPROM) is the machine's [`Cps2Bus`], whose own RAM the game machine leaves empty.
//!
//! The reference machine and the tools run the 68000 on a bus that holds the RAM: [`World::lend`] and
//! [`World::as_bus`] move the RAM and the table between the two shapes without copying them.

pub mod data;
pub mod diag;
pub mod events;
pub mod machine;
pub mod mem;
pub mod objects;
pub mod ops;
pub mod placement;
pub mod ram;
pub mod record;
pub mod rng;
pub mod tables;
pub mod terrain;

use cps2::{Board, Cps2Bus};

use crate::core::tasks::table::Table;

/// Words of the work RAM.
pub const RAM_WORDS: usize = 0x8000;

/// The game's world (see the module header).
#[derive(Clone)]
pub struct World {
    /// Work RAM, FF0000-FFFFFF: word `i` is at `0xff0000 + 2 i` (big-endian, like the 68000).
    pub ram: Box<[u16; RAM_WORDS]>,
    /// The task table, created by the first task (none before the startup has run).
    pub tasks: Option<Table>,
    /// The hardware: video, sound link, inputs, EEPROM (its `ram` is empty).
    pub hw: Cps2Bus,
}

/// Index of the RAM word of a work RAM address.
#[inline]
fn word(a: u32) -> usize {
    (a as usize >> 1) & (RAM_WORDS - 1)
}

/// A work RAM address (FF0000-FFFFFF, address already reduced to 24 bits by the callers, as on the bus).
#[inline]
fn in_ram(a: u32) -> bool {
    a >> 16 == 0xff
}

impl World {
    /// The world of a bus: its RAM and the task table that a tool may have left in it are taken out (no copy).
    pub fn from_bus(mut hw: Cps2Bus) -> World {
        let ram = std::mem::take(&mut hw.ram).into_boxed_slice().try_into().expect("work RAM of 0x8000 words");
        let tasks = hw.ext.take().map(|e| *e.into_any().downcast::<Table>().expect("bus extension: task table"));
        World { ram, tasks, hw }
    }

    /// Back to a bus that holds the RAM and the task table.
    pub fn into_bus(self) -> Cps2Bus {
        let World { ram, tasks, mut hw } = self;
        hw.ram = (ram as Box<[u16]>).into_vec();
        hw.ext = tasks.map(|t| Box::new(t) as Box<dyn cps2::bus::Extension>);
        hw
    }

    /// A copy of the world as a bus (tools: the original code runs on it).
    pub fn to_bus(&self) -> Cps2Bus {
        self.clone().into_bus()
    }

    /// Runs `f` on the world of `bus` (tools: a native run by the processor of a machine whose RAM is on the bus). The
    /// bus gets its RAM and table back afterwards, even if `f` panics.
    pub fn lend<R>(bus: &mut Cps2Bus, f: impl FnOnce(&mut World) -> R) -> R {
        struct Lent<'a> {
            bus: &'a mut Cps2Bus,
            world: Option<World>,
        }
        impl Drop for Lent<'_> {
            fn drop(&mut self) {
                if let Some(w) = self.world.take() {
                    *self.bus = w.into_bus();
                }
            }
        }
        let hw = std::mem::replace(bus, Cps2Bus::detached());
        let mut lent = Lent { bus, world: Some(World::from_bus(hw)) };
        f(lent.world.as_mut().expect("lent world"))
    }

    /// Runs `f` on this world seen as a bus (tools: the 68000 runs on it). The world gets its RAM and table back
    /// afterwards, even if `f` panics.
    pub fn as_bus<R>(&mut self, f: impl FnOnce(&mut Cps2Bus) -> R) -> R {
        struct Lent<'a> {
            world: &'a mut World,
            bus: Option<Cps2Bus>,
        }
        impl Drop for Lent<'_> {
            fn drop(&mut self) {
                if let Some(b) = self.bus.take() {
                    let w = World::from_bus(b);
                    *self.world = w;
                }
            }
        }
        let empty = World { ram: Box::new([0; RAM_WORDS]), tasks: None, hw: Cps2Bus::detached() };
        let w = std::mem::replace(self, empty);
        let mut lent = Lent { world: self, bus: Some(w.into_bus()) };
        f(lent.bus.as_mut().expect("lent bus"))
    }

    /// Byte at a 24-bit address: the work RAM directly, the hardware through its address decoding.
    #[inline]
    pub fn read8(&mut self, a: u32) -> u8 {
        if in_ram(a) {
            let w = self.ram[word(a)];
            if a & 1 == 0 { (w >> 8) as u8 } else { w as u8 }
        } else {
            self.hw.read8(a)
        }
    }

    #[inline]
    pub fn read16(&mut self, a: u32) -> u16 {
        if in_ram(a) { self.ram[word(a)] } else { self.hw.read16(a) }
    }

    #[inline]
    pub fn write8(&mut self, a: u32, v: u8) {
        if in_ram(a) {
            let w = &mut self.ram[word(a)];
            *w = if a & 1 == 0 { (*w & 0x00ff) | (v as u16) << 8 } else { (*w & 0xff00) | v as u16 };
        } else {
            self.hw.write8(a, v)
        }
    }

    #[inline]
    pub fn write16(&mut self, a: u32, v: u16) {
        if in_ram(a) {
            self.ram[word(a)] = v;
        } else {
            self.hw.write16(a, v)
        }
    }
}

impl Board for World {
    fn hw(&self) -> &Cps2Bus {
        &self.hw
    }
    fn hw_mut(&mut self) -> &mut Cps2Bus {
        &mut self.hw
    }
    fn ram(&self) -> &[u16] {
        &self.ram[..]
    }
    fn ram_mut(&mut self) -> &mut [u16] {
        &mut self.ram[..]
    }
}
