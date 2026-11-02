//! Common context of the native routines: the machine (the bus) and the shared services.
//!
//! Each subsystem has its own context (`families::cx::Cx`, `player::Ctx`, `scene::Ctx`,
//! `combat::World`...); all of them hold `b: &mut Cps2Bus`. Rather than a base structure that
//! they would embed (the separate `x.c`, `x.b` accesses, everywhere, would not survive a `Deref`), they implement
//! the [`Machine`] trait, which gives them the common services once: the random draw, and reads in the
//! program space and the data space. The work RAM variables are read through their typed views
//! (`ram::SCENE.get(x.bus())`, `Obj`, `Player`). The object engine (animation, movement, lists, screen) is in
//! [`crate::objects::Current`], for the contexts that process an object.
//!
//! The game has no processor; only the tools (`crate::tools`, natives for the original code) add one
//! (`tools::WithCpu`).

use cps2::Cps2Bus;

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

impl Machine for crate::scene::Ctx<'_> {
    fn bus(&mut self) -> &mut Cps2Bus {
        &mut *self.b
    }
}
