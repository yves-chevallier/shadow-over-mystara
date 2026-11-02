//! Object tables tied to sprites: collision boxes, visibility boxes, ground marks and
//! frame lists read directly by the code. Format: `assets/sprites/tables.json`, see docs/assets.md.

use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::json::{hex, hex_vec};

/// Collision box (8 bytes): X (signed), half-width, Z (height, signed), half-height, Y
/// (depth, signed), half-depth, then two bytes the collision does not read. Offsets from
/// the object's position; X is inverted when the object faces left (docs/combat.md §3).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct HitBox(pub i8, pub u8, pub i8, pub u8, pub i8, pub u8, pub u8, pub u8);

impl HitBox {
    pub fn bytes(&self) -> [u8; 8] {
        [self.0 as u8, self.1, self.2 as u8, self.3, self.4 as u8, self.5, self.6, self.7]
    }

    pub fn from_bytes(b: &[u8]) -> Self {
        HitBox(b[0] as i8, b[1], b[2] as i8, b[3], b[4] as i8, b[5], b[6], b[7])
    }
}

/// Table of box sets: one long per set (`+0x2e` of the object), address of the set's box of index 0;
/// the box of index `n` (object byte, 0 = none) is at `address + 8 n`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct BoxTable {
    /// `body` (index `+0x38`), `push` (`+0x39`) or `attack` (`+0x3a`).
    pub name: String,
    #[serde(with = "hex")]
    pub rom: u32,
    #[serde(with = "hex_vec")]
    pub sets: Vec<u32>,
}

/// Boxes stored from an address designated by a set, up to the next one. `tail`: remaining
/// bytes when the gap is not a multiple of 8 (end of block, `0x4e71` padding).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct BoxBlock {
    #[serde(with = "hex")]
    pub rom: u32,
    pub boxes: Vec<HitBox>,
    pub tail: Vec<u8>,
}

impl BoxBlock {
    pub fn len(&self) -> usize {
        8 * self.boxes.len() + self.tail.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Visibility box (0x1ed2, index `+0x3c`), words: the object is on screen if `screen x + dx <= width`
/// and `screen height + dy <= height` (unsigned comparisons).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bounds(pub i16, pub u16, pub i16, pub u16);

/// Ground mark (0x1e3a6, index `+0x3b`), 8 words.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct GroundMark {
    /// Pieces deducted from the sprite list budget.
    pub cost: u16,
    /// Hardware sprite code.
    pub code: u16,
    /// Fixed x offset.
    pub x: i16,
    /// Sprite attributes (bit 5 flipped when mirrored).
    pub attr: u16,
    /// Extra x offset, negated when mirrored.
    pub dx: i16,
    /// Vertical offset (subtracted from the on-screen height).
    pub dy: i16,
    /// Priority bits added to the sprite's x word.
    pub priority: u16,
    pub unused: u16,
}

impl GroundMark {
    pub fn words(&self) -> [u16; 8] {
        [self.cost, self.code, self.x as u16, self.attr, self.dx as u16, self.dy as u16, self.priority, self.unused]
    }

    pub fn from_words(w: [u16; 8]) -> Self {
        GroundMark { cost: w[0], code: w[1], x: w[2] as i16, attr: w[3], dx: w[4] as i16, dy: w[5] as i16, priority: w[6], unused: w[7] }
    }
}

/// List of frames stored as longs (frame addresses) and read directly by the code.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct FrameList {
    #[serde(with = "hex")]
    pub rom: u32,
    /// Frame numbers (`frames.json`).
    pub frames: Vec<u32>,
}

/// Tables of the ESHL 5 shot (0x9e458, snake rings and contact zones of large enemies).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Shot5 {
    /// Per `+0x2f`: box indices body, push, attack, then the ground mark (`+0x3b`); program
    /// space.
    #[serde(with = "hex")]
    pub boxes_rom: u32,
    pub boxes: Vec<[u8; 4]>,
    /// Per `+0x2f`: offset (word, from the table, program space) of the frame list for even
    /// (`even`) or odd (`odd`) frames.
    #[serde(with = "hex")]
    pub even_rom: u32,
    pub even: Vec<i16>,
    #[serde(with = "hex")]
    pub odd_rom: u32,
    pub odd: Vec<i16>,
    /// (frame, flip) per `(+0x42 / 2) * 16 + +0x43 / 2`; data space.
    #[serde(with = "hex")]
    pub mirrored_rom: u32,
    pub mirrored: Vec<[u8; 2]>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SpriteTables {
    pub box_tables: Vec<BoxTable>,
    /// All the boxes, in contiguous blocks (one per designated address), from `box_tables[..].sets` up to
    /// `box_end`.
    pub box_blocks: Vec<BoxBlock>,
    #[serde(with = "hex")]
    pub box_end: u32,
    #[serde(with = "hex")]
    pub bounds_rom: u32,
    pub bounds: Vec<Bounds>,
    #[serde(with = "hex")]
    pub marks_rom: u32,
    pub marks: Vec<GroundMark>,
    pub frame_lists: Vec<FrameList>,
    pub shot5: Shot5,
}

impl SpriteTables {
    pub fn load(path: &Path) -> io::Result<Self> {
        crate::json::load(path)
    }

    /// Box of index `n` of set `set` of table `table` (0 body, 1 push, 2 attack).
    pub fn hit_box(&self, table: usize, set: usize, n: usize) -> Option<HitBox> {
        let at = self.box_tables[table].sets[set] + 8 * n as u32;
        let i = self.box_blocks.partition_point(|b| b.rom <= at).checked_sub(1)?;
        let b = &self.box_blocks[i];
        let off = (at - b.rom) as usize;
        if off.is_multiple_of(8) && off / 8 < b.boxes.len() {
            return Some(b.boxes[off / 8]);
        }
        // box straddling a block and the next one (unaligned blocks)
        let mut bytes = Vec::new();
        for blk in &self.box_blocks[i..] {
            for bx in &blk.boxes {
                bytes.extend(bx.bytes());
            }
            bytes.extend(&blk.tail);
            if bytes.len() >= off + 8 {
                break;
            }
        }
        (bytes.len() >= off + 8).then(|| HitBox::from_bytes(&bytes[off..off + 8]))
    }
}
