//! Work RAM variables, named and typed (foundation of the refactoring, `docs/guide-refactorisation.md`).
//!
//! The game state stays in the emulated RAM as long as validation compares the RAM frame by frame with the original: a
//! variable is therefore a typed *address*, and `get`/`set` read and write the bus (big-endian, like the 68000). One
//! writes `ram::SCENE.get(b)` instead of `rd8(b, A5 + 0x34)`, and `ram::PLAYERS.nth(k)` instead of
//! `A5 + 0x5de + 0x200 * k`.
//!
//! Rule: a variable specific to one subsystem is declared in its module (`hud::var`, `objects::ram`...) with
//! these same types; only the variables read by several subsystems appear here. The names come from
//! `docs/*.md`; a guessed meaning is marked (h).

use cps2::Cps2Bus;

use crate::mem::{rd16, rd32, rd8, wr16, wr32, wr8, A5};
use crate::objects::Obj;

/// Work RAM byte.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Byte(pub u32);

/// Work RAM word (16 bits).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Word(pub u32);

/// Work RAM long (32 bits).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Long(pub u32);

impl Byte {
    /// Variable at `A5 + offset` (the negative offsets of the original are written as absolute: `0xff2472`).
    pub const fn a5(offset: u32) -> Self {
        Byte(A5 + offset)
    }
    pub fn get(self, b: &mut Cps2Bus) -> u8 {
        rd8(b, self.0)
    }
    pub fn get_i8(self, b: &mut Cps2Bus) -> i8 {
        rd8(b, self.0) as i8
    }
    pub fn set(self, b: &mut Cps2Bus, v: u8) {
        wr8(b, self.0, v)
    }
    /// Non-zero byte (`tst.b` followed by `bne`).
    pub fn is_set(self, b: &mut Cps2Bus) -> bool {
        self.get(b) != 0
    }
}

impl Word {
    pub const fn a5(offset: u32) -> Self {
        Word(A5 + offset)
    }
    pub fn get(self, b: &mut Cps2Bus) -> u16 {
        rd16(b, self.0)
    }
    pub fn get_i16(self, b: &mut Cps2Bus) -> i16 {
        rd16(b, self.0) as i16
    }
    pub fn set(self, b: &mut Cps2Bus, v: u16) {
        wr16(b, self.0, v)
    }
}

impl Long {
    pub const fn a5(offset: u32) -> Self {
        Long(A5 + offset)
    }
    pub fn get(self, b: &mut Cps2Bus) -> u32 {
        rd32(b, self.0)
    }
    pub fn set(self, b: &mut Cps2Bus, v: u32) {
        wr32(b, self.0, v)
    }
}

/// Array of fixed-size records (players, panels, coin counters...).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Table {
    pub base: u32,
    pub step: u32,
    pub count: u32,
}

impl Table {
    /// Record `k` (unchecked: the original sometimes indexes beyond).
    pub const fn nth(self, k: u32) -> Obj {
        Obj(self.base + self.step * k)
    }
    pub fn iter(self) -> impl Iterator<Item = Obj> {
        (0..self.count).map(move |k| self.nth(k))
    }
}

/// Seed of the pseudo-random generator `0x19a88` (`rng.rs`).
pub const RNG_SEED: Word = Word::a5(0x1a);
/// Cabinet type: number of players (h); selects the interface layout.
pub const CABINET: Byte = Byte::a5(0x20);
/// Game phase (step of 2: 8 scene entry, 0xc game, 0x10 exit, 0x14 end of game; `docs/boucle.md`)
/// and its step.
pub const PHASE: Byte = Byte::a5(0x2c);
pub const STEP: Byte = Byte::a5(0x2d);
/// A game is in progress.
pub const GAME: Byte = Byte::a5(0x30);
/// Scene number (1-114; `docs/rewrite-guide.md`, "Atteindre un stage").
pub const SCENE: Byte = Byte::a5(0x34);
/// Number of players alive (batch D: wrongly called "difficulty" until batch S).
pub const PLAYERS_ALIVE: Byte = Byte::a5(0x35);
/// Alternative camera selected (non-zero) instead of the main camera.
pub const CAMERA_ALT_ACTIVE: Byte = Byte::a5(0x36);
/// Display mode of the interface panels (`hud`).
pub const DISPLAY_PANELS: Byte = Byte::a5(0x3c);
/// Pause: one bit per player.
pub const PAUSE: Byte = Byte::a5(0x4a);
/// VBLANK frame counter (`vblank.rs`).
pub const IMAGES: Word = Word::a5(0x6c);
/// Desired vertical scroll of plane 1 (`vblank` copies it into the plane register): spells make the
/// screen shake by toggling it between 0 and 0x100.
pub const SCROLL_Y_PLANE1: Word = Word::a5(0x4ba);
/// Main camera: position on screen (x, y); alternative camera.
pub const CAMERA_X: Word = Word::a5(0x506);
pub const CAMERA_Y: Word = Word::a5(0x50a);
pub const CAMERA_ALT_X: Word = Word::a5(0x556);
pub const CAMERA_ALT_Y: Word = Word::a5(0x55a);
/// Switches word (DSW) read at VBLANK (`FF7FD0`): bit 0x200 no damage display, 0x400 charge gauge
/// inactive, 0x800 collisions with the terrain.
pub const DSW: Word = Word::a5(0x7fd0);
/// The four player records (0x200 bytes; fields: `player::Player`).
pub const PLAYERS: Table = Table { base: A5 + 0x5de, step: 0x200, count: 4 };

/// Camera of the frame: the alternative camera if selected, otherwise the main one.
pub fn camera(b: &mut Cps2Bus) -> (u16, u16) {
    if CAMERA_ALT_ACTIVE.is_set(b) {
        (CAMERA_ALT_X.get(b), CAMERA_ALT_Y.get(b))
    } else {
        (CAMERA_X.get(b), CAMERA_Y.get(b))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses() {
        assert_eq!(SCENE.0, 0xff8034);
        assert_eq!(PLAYERS.nth(3).0, 0xff8bde);
        assert_eq!(PLAYERS.iter().count(), 4);
    }
}
