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
pub mod objects;
pub mod ops;
pub mod placement;
pub mod ram;
pub mod record;
pub mod rng;
pub mod screen_tables;
pub mod tables;
pub mod terrain;

use cps2::{Board, Cps2Bus};

use crate::core::tasks::table::Table;

/// What the world holds besides the RAM, parked in the bus extension (`Cps2Bus::ext`) while the RAM is on a bus
/// (tools).
#[derive(Clone, Default)]
pub struct Parked {
    pub tasks: Option<Table>,
    pub leftovers: Leftovers,
}

/// Words of the work RAM.
pub const RAM_WORDS: usize = 0x8000;

/// The game's world (see the module header).
#[derive(Clone)]
pub struct World {
    /// Work RAM, FF0000-FFFFFF: word `i` is at `0xff0000 + 2 i` (big-endian, like the 68000).
    pub ram: Box<[u16; RAM_WORDS]>,
    /// The task table, created by the first task (none before the startup has run).
    pub tasks: Option<Table>,
    /// Leftover registers that the object handlers and a few routines pass to each other.
    pub leftovers: Leftovers,
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
        let Parked { tasks, leftovers } = hw
            .ext
            .take()
            .map_or_else(Parked::default, |e| *e.into_any().downcast::<Parked>().expect("bus extension: parked world"));
        World { ram, tasks, leftovers, hw }
    }

    /// Back to a bus that holds the RAM and the task table.
    pub fn into_bus(self) -> Cps2Bus {
        let World { ram, tasks, leftovers, mut hw } = self;
        hw.ram = (ram as Box<[u16]>).into_vec();
        hw.ext = Some(Box::new(Parked { tasks, leftovers }));
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
        let empty = World { ram: Box::new([0; RAM_WORDS]), tasks: None, leftovers: Leftovers::default(), hw: Cps2Bus::detached() };
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

impl crate::hw::mem::Memory for World {
    #[inline]
    fn read8(&mut self, a: u32) -> u8 {
        World::read8(self, a)
    }
    #[inline]
    fn read16(&mut self, a: u32) -> u16 {
        World::read16(self, a)
    }
    #[inline]
    fn write8(&mut self, a: u32, v: u8) {
        World::write8(self, a, v)
    }
    #[inline]
    fn write16(&mut self, a: u32, v: u16) {
        World::write16(self, a, v)
    }
    fn fetch16(&mut self, a: u32) -> u16 {
        self.hw.fetch16(a)
    }
}

/// Leftover registers that a handler leaves and that a following handler rereads (original glitches). The probe
/// `DDSOM_POISON=exit` (registers changed by each handler, inverted on its return) pointed to them in all
/// scenarios; the other registers that a handler leaves are reread by nobody. Other routines also write them (sprite
/// list, cameras, input copy, gauge: `sprites::build_in_game`...).
///
/// They are a game state, kept in the world ([`World::leftovers`]); the natives called by the
/// original code (tools) find them in their register bench ([`Leftovers::to_bench`] and [`Leftovers::from_bench`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Leftovers {
    /// D2: what is left by the collision with the terrain (0x33fe4, `terrain::push`), the box tests (0xb5e0,
    /// 0x7f19c, 0x7ef4), the loot treasure line (0xb780); the spark of `flash_and_shake` (0x103ba) takes
    /// its speed from it, the bounce of the frost shard (0x4bda8) its setting, the ITEM 5 object (0xdc382) rereads it.
    pub d2: u32,
    /// D3: like D2 (collision, box tests, special loot); acceleration of the spark of `flash_and_shake`.
    pub d3: u32,
    /// D4: the last long copied by the palette animation (SET 0, 0xba310), the raft movement (EFFCT,
    /// 0xad63e), the attributes of the last piece of the sprite list...; the pilotless raft adds it to the
    /// players' position (0xad7b0).
    pub d4: u32,
    /// D5: 0 or 1 (the object moved) after keeping on screen (0xfbae, `ai::keep_on_screen`), 1 after a
    /// collision with the terrain (0x33fe4), the exhausted counter of a `dbf` loop...; the row of shards of a crate
    /// (0xa17d0, `eshl_creators::row_1a`) rereads its high byte.
    pub d5: u32,
}
