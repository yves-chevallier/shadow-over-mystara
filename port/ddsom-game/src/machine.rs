//! Common context of the native routines: the machine (the bus) and the shared services.
//!
//! Each subsystem has its own context (`families::cx::Cx`, `player::Ctx`, `scene::Ctx`,
//! `combat::Combat`...); all of them hold `b: &mut World`. Rather than a base structure that
//! they would embed (the separate `x.c`, `x.b` accesses, everywhere, would not survive a `Deref`), they implement
//! the [`Machine`] trait, which gives them the common services once: the random draw. The data of the original are
//! read through the typed tables (`crate::tables`, `crate::data`). The work RAM variables are read through their typed views
//! (`ram::SCENE.get(x.bus())`, `Obj`, `Player`). The object engine (animation, movement, lists, screen) is in
//! [`crate::objects::Current`], for the contexts that process an object.
//!
//! The game has no processor; only the tools (`crate::tools`, natives for the original code) add one
//! (`tools::WithCpu`).

use crate::world::World;

use crate::rng;

/// Common access to the machine (see the module header).
pub trait Machine {
    /// The bus: the game memory.
    fn bus(&mut self) -> &mut World;

    /// Draw of the game's pseudo-random generator (0x19a88, `rng::random`); callers mask the result.
    fn random(&mut self) -> u16 {
        rng::random(self.bus())
    }
}

impl Machine for crate::scene::Ctx<'_> {
    fn bus(&mut self) -> &mut World {
        &mut *self.b
    }
}
