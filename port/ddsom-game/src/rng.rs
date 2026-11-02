//! The game's pseudo-random generator (U1). It must remain **bit-exact**: every draw
//! in the game (enemy behaviour, damage, loot...) depends on it, hence also the reproducibility
//! of the games.
//!
//! The state is a word at `A5+0x1a` (`ram::RNG_SEED`: high byte at 0x1a, low byte at 0x1b), initialised to
//! 0x01c3 at the start of a game (0x19a80). Each draw (0x19a88):
//!
//! ```text
//! t     = ((3 × state) mod 2^16) >> 8        (byte)
//! low   = low + t  (mod 256)
//! high  = t
//! result = new state (word)
//! ```
//!
//! A single draw for the whole game: [`random`] (core). The contexts offer it as `x.random()`
//! (`crate::machine::Machine`); the native (calls by address) also leaves D0.w and the flags of the original.

use cps2::Cps2Bus;

use crate::ram::RNG_SEED;

/// Seed set at the start of a game.
pub const DEFAULT_SEED: u16 = 0x01c3;

/// State of the generator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rng(pub u16);

impl Rng {
    pub const fn seed() -> Self {
        Rng(DEFAULT_SEED)
    }

    /// Next draw: returns the new state and the carry of the byte addition (X flag that
    /// the original leaves behind).
    pub fn next(&mut self) -> (u16, bool) {
        let t = ((self.0.wrapping_mul(3)) >> 8) as u8;
        let (low, carry) = (self.0 as u8).overflowing_add(t);
        self.0 = (t as u16) << 8 | low as u16;
        (self.0, carry)
    }
}

/// One draw (0x19a88): advances the seed and returns the new state. Callers mask the result.
pub fn random(b: &mut Cps2Bus) -> u16 {
    draw(b).0
}

pub(crate) fn draw(b: &mut Cps2Bus) -> (u16, bool) {
    let mut rng = Rng(RNG_SEED.get(b));
    let r = rng.next();
    RNG_SEED.set(b, r.0);
    r
}

/// 0x19a80: resets the seed.
pub fn seed(b: &mut Cps2Bus) {
    RNG_SEED.set(b, DEFAULT_SEED);
}
