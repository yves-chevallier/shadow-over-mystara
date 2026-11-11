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

use crate::world::World;

use crate::world::mem::{rd16, rd32, rd8, wr16, wr32, wr8, Memory, A5};
use crate::world::objects::Obj;

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
    pub fn get(self, b: &mut impl Memory) -> u8 {
        rd8(b, self.0)
    }
    pub fn get_i8(self, b: &mut impl Memory) -> i8 {
        rd8(b, self.0) as i8
    }
    pub fn set(self, b: &mut impl Memory, v: u8) {
        wr8(b, self.0, v)
    }
    /// Non-zero byte (`tst.b` followed by `bne`).
    pub fn is_set(self, b: &mut impl Memory) -> bool {
        self.get(b) != 0
    }
}

impl Word {
    pub const fn a5(offset: u32) -> Self {
        Word(A5 + offset)
    }
    pub fn get(self, b: &mut impl Memory) -> u16 {
        rd16(b, self.0)
    }
    pub fn get_i16(self, b: &mut impl Memory) -> i16 {
        rd16(b, self.0) as i16
    }
    pub fn set(self, b: &mut impl Memory, v: u16) {
        wr16(b, self.0, v)
    }
}

impl Long {
    pub const fn a5(offset: u32) -> Self {
        Long(A5 + offset)
    }
    pub fn get(self, b: &mut impl Memory) -> u32 {
        rd32(b, self.0)
    }
    pub fn set(self, b: &mut impl Memory, v: u32) {
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
/// The phase, its step and the two bytes that follow as one long (a new phase is written in one go).
pub const PHASE_LONG: Long = Long::a5(0x2c);
/// A game is in progress.
pub const GAME: Byte = Byte::a5(0x30);
/// Scene number (1-114; `docs/rewrite-guide.md`, "Atteindre un stage").
pub const SCENE: Byte = Byte::a5(0x34);
/// Number of players alive (batch D: wrongly called "difficulty" until batch S).
pub const PLAYERS_ALIVE: Byte = Byte::a5(0x35);
/// Clock of the scene (long; its high word counts down the time left, h).
pub const STAGE_CLOCK: Long = Long::a5(0x38);
/// Alternative camera selected (non-zero) instead of the main camera.
pub const CAMERA_ALT_ACTIVE: Byte = Byte::a5(0x36);
/// Display mode of the interface panels (`hud`).
pub const DISPLAY_PANELS: Byte = Byte::a5(0x3c);
/// Byte at 1 during a cutscene (game paused, inputs cut).
pub const CUTSCENE: Byte = Byte::a5(0x3d);
/// Non-zero: the players are frozen (scripted scenes, ends of scenes); some helpers then do nothing.
pub const PLAYERS_FROZEN: Byte = Byte::a5(0x42);
/// Mask of the inputs of the players (0x3f: all buttons; written at the game over and by the bosses).
pub const INPUT_MASK: Byte = Byte::a5(0x43);
/// Object that the camera follows in its mode 0x7c, or the leading object of a scripted scene (word, RAM address).
pub const LEADER: Word = Word::a5(0x46);
/// Pause: one bit per player.
pub const PAUSE: Byte = Byte::a5(0x4a);
/// Difficulty level of the setting (h; `ai`): beyond 5, other tables of initial HP and speeds.
pub const LEVEL: Byte = Byte::a5(0x48);
/// Row of the special loot tables by difficulty (h: set with `LEVEL` by the settings).
pub const LEVEL_ROW: Byte = Byte::a5(0x49);
/// Scene level for bosses (0 nothing, 3 always; `boss_common::hp_threshold`).
pub const SCENE_LEVEL: Byte = Byte::a5(0x5a);
/// VBLANK frame counter (`vblank.rs`).
pub const IMAGES: Word = Word::a5(0x6c);
/// Desired vertical scroll of plane 1 (`vblank` copies it into the plane register): spells make the
/// screen shake by toggling it between 0 and 0x100.
pub const SCROLL_Y_PLANE1: Word = Word::a5(0x4ba);
/// Screen shake (frames left; spells and impacts set it, `vblank` shakes the planes while it counts down).
pub const SHAKE: Word = Word::a5(0x4be);
/// Main camera: position on screen (x, y); alternative camera.
pub const CAMERA_X: Word = Word::a5(0x506);
pub const CAMERA_Y: Word = Word::a5(0x50a);
pub const CAMERA_ALT_X: Word = Word::a5(0x556);
pub const CAMERA_ALT_Y: Word = Word::a5(0x55a);
/// Positions of scroll layers 2 (camera 1) and 3 (camera 2) as shown this frame: `vblank` copies the wanted
/// positions (`CAMERA_X`...) here when it writes the CPS-A registers.
pub const CAMERA1_X: Word = Word::a5(0x5a6);
pub const CAMERA1_Y: Word = Word::a5(0x5aa);
pub const CAMERA2_X: Word = Word::a5(0x5ae);
pub const CAMERA2_Y: Word = Word::a5(0x5b2);
/// Number of players of the cabinet (2 or 4: word); with 2, the texts in sprites are words.
pub const CABINET_PLAYERS: Word = Word::a5(0x72);
/// Delay before the game over sequence (`aids::game_over`), `A5-0x55c6`.
pub const GAME_OVER_DELAY: Word = Word(0xff2a3a);
/// Switches word (DSW) read at VBLANK (`FF7FD0`): bit 0x200 no damage display, 0x400 charge gauge
/// inactive, 0x800 collisions with the terrain.
pub const DSW: Word = Word::a5(0x7fd0);
/// The four player records (0x200 bytes; fields: `player::Player`).
pub const PLAYERS: Table = Table { base: A5 + 0x5de, step: 0x200, count: 4 };

/// Camera of the frame: the alternative camera if selected, otherwise the main one.
pub fn camera(b: &mut World) -> (u16, u16) {
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
