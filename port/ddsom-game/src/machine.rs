//! Common context of the native routines: the machine (processor and bus) and the shared services.
//!
//! Each subsystem has its own context (`families::cx::Cx`, `player::Ctx`, `scene::Ctx`,
//! `combat::World`...); all of them hold `b: &mut Cps2Bus` (and some still `c: &mut Cpu`). Rather than a base structure that
//! they would embed (the separate `x.c`, `x.b` accesses, everywhere, would not survive a `Deref`), they implement
//! the [`Machine`] trait, which gives them the common services once: the random draw, and reads in the
//! program space and the data space. The work RAM variables are read through their typed views
//! (`ram::SCENE.get(x.bus())`, `Obj`, `Player`). The object engine (animation, movement, lists, screen) is in
//! [`crate::objects::Current`], for the contexts that process an object.
//!
//! The processor is only used by the `_68k` adapters (registers of the original, `docs/guide-refactorisation.md`); a
//! converted routine only uses `bus()`.

use cps2::Cps2Bus;
use m68k::decode::Fetch;
use m68k::Cpu;

use crate::mem::{rd16, rd32, rd8};
use crate::rng;

/// Common access to the machine (see the module header).
pub trait Machine {
    /// The bus: the game memory.
    fn bus(&mut self) -> &mut Cps2Bus;

    /// Draw of the game's pseudo-random generator (0x19a88, `rng::random`); callers mask the result.
    fn random(&mut self) -> u16 {
        rng::random(self.bus())
    }

    /// Byte, word and long of the program space (tables read PC-relative in the original: the decrypted
    /// program).
    fn prog8(&mut self, a: u32) -> u8 {
        crate::mem::prog8(self.bus(), a)
    }
    fn prog16(&mut self, a: u32) -> u16 {
        self.bus().fetch16(a)
    }
    fn prog32(&mut self, a: u32) -> u32 {
        crate::mem::prog32(self.bus(), a)
    }

    /// Byte, word and long of the data space (ROM tables reached through an absolute address or a pointer,
    /// RAM).
    fn data8(&mut self, a: u32) -> u8 {
        rd8(self.bus(), a)
    }
    fn data16(&mut self, a: u32) -> u16 {
        rd16(self.bus(), a)
    }
    fn data32(&mut self, a: u32) -> u32 {
        rd32(self.bus(), a)
    }
}

/// Context that also holds the processor: the `_68k` adapters (registers of the original) go through it.
pub trait WithCpu: Machine {
    /// The processor and the bus, borrowed together (the `_68k` adapters need both).
    fn machine(&mut self) -> (&mut Cpu, &mut Cps2Bus);
}

/// Implements `Machine` and `WithCpu` for a context that has the fields `c` and `b`.
macro_rules! machine {
    ($($t:ty),* $(,)?) => {
        $(impl Machine for $t {
            fn bus(&mut self) -> &mut Cps2Bus {
                &mut *self.b
            }
        }
        impl WithCpu for $t {
            fn machine(&mut self) -> (&mut Cpu, &mut Cps2Bus) {
                (&mut *self.c, &mut *self.b)
            }
        })*
    };
}

machine!(crate::scene::Ctx<'_>);
