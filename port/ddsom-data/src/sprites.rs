//! Sprite definitions: frames (assemblies of 16x16 cells) and animation scripts.
//! Formats: `assets/sprites/frames.json` and `assets/sprites/anims.json`, see docs/assets.md.

use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::json::{hex, hex_opt, hex_vec};
use crate::pointers::Pointers;
use crate::RomAddr;

/// Piece of a frame: `[dx, dy, code, attributes]`. The top-left corner of the piece is at (dx, dy)
/// pixels from the object's anchor point (y towards the bottom of the screen: negative dy = above); when mirrored
/// horizontally, at (-dx - 16 - 16 (width - 1), dy). CPS2 hardware attributes: bits 0-4 palette, 5 horizontal
/// mirror, 6 vertical mirror, 8-11 width - 1 and 12-15 height - 1 (in 16x16 cells). The cell
/// (bx, by) of a piece is `bank << 16 | (code & !0xf) + ((code + bx) & 0xf) + 16 by`.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Piece(pub i16, pub i16, pub u16, pub u16);

impl Piece {
    pub fn dx(&self) -> i16 {
        self.0
    }
    pub fn dy(&self) -> i16 {
        self.1
    }
    pub fn code(&self) -> u16 {
        self.2
    }
    pub fn attr(&self) -> u16 {
        self.3
    }
}

/// A sprite frame, as the object designates it at `+0x34`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    /// ROM address of the header (provenance; it is also the value the game stores in the object).
    #[serde(with = "hex")]
    pub rom: u32,
    /// Cost in pieces deducted from the sprite list budget (word 0 of the header).
    pub cost: u16,
    /// Word 1 of the header: common attributes (`mode` zero) or number of pieces (`mode` non-zero).
    pub attr: u16,
    /// Cell bank: the cell of a piece is `bank << 16 | code`.
    pub bank: u8,
    /// Byte 5 of the header: zero, all pieces take `attr`; otherwise each has its own attributes.
    pub mode: u8,
    /// ROM address of the offset table (shared between frames).
    #[serde(with = "hex")]
    pub offsets: u32,
    pub pieces: Vec<Piece>,
}

impl Frame {
    pub fn per_piece(&self) -> bool {
        self.mode != 0
    }
}

/// All the frames, in the order of the ROM table (frame number = index).
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct Frames {
    /// ROM table of frame addresses (one long per frame).
    #[serde(with = "hex")]
    pub table: u32,
    pub frames: Vec<Frame>,
    /// Frames outside the table, that only the code designates (by increasing address): their numbers follow those
    /// of the table.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra: Vec<Frame>,
}

impl Frames {
    pub fn load(path: &Path) -> io::Result<Self> {
        crate::json::load(path)
    }

    /// Number of the frame whose header is at ROM address `rom`.
    pub fn by_rom(&self, rom: u32) -> Option<usize> {
        self.frames.binary_search_by_key(&rom, |f| f.rom).ok()
    }

    /// The frame whose header is at `rom` (the value an object keeps at `+0x34`), in the table or outside.
    pub fn at(&self, rom: RomAddr) -> Option<&Frame> {
        let extra = || self.extra.binary_search_by_key(&rom.0, |f| f.rom).ok().map(|i| &self.extra[i]);
        self.by_rom(rom.0).map(|i| &self.frames[i]).or_else(extra)
    }

    /// Frame number `n` (numbers past the table designate the frames outside it).
    pub fn number(&self, n: u32) -> &Frame {
        let n = n as usize;
        self.frames.get(n).unwrap_or_else(|| &self.extra[n - self.frames.len()])
    }
}

/// Family of the sprite viewer of the hidden test menu (`pl00`, `em03`, `bs10`...): one set
/// of animations per variant.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Family {
    pub name: String,
    /// Long entries (player characters).
    pub long: bool,
    #[serde(with = "hex_vec")]
    pub sets: Vec<u32>,
}

/// Animation set: array of signed offsets (bytes, relative to `rom`) to the start of each script.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct AnimSet {
    #[serde(with = "hex")]
    pub rom: u32,
    pub long: bool,
    pub scripts: Vec<i16>,
}

impl AnimSet {
    /// Address of script `i`.
    pub fn script(&self, i: usize) -> u32 {
        self.rom.wrapping_add(self.scripts[i] as i32 as u32)
    }
}

/// Entry of an animation script (see `objects/anim.rs`).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    #[serde(with = "hex")]
    pub rom: u32,
    /// Long entry (24 bytes, characters) or short (16 bytes).
    pub long: bool,
    /// Size deduced from the chaining, the entry being reached by no known set.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub guessed: bool,
    /// Duration in frames.
    pub time: u16,
    /// Flags: high byte 0 (the next entry follows), 0xfe (end) or other (jump to `next`).
    pub flags: u16,
    /// Frame number (index into `Frames`).
    pub frame: u32,
    /// Bytes copied into the object at `+0x38` (8 bytes: shadow, ground mark, visibility box,
    /// priority; 16 for a long entry, up to `+0x47`).
    pub data: Vec<u8>,
    /// Jump target when the high byte of the flags is neither 0 nor 0xfe.
    #[serde(default, with = "hex_opt", skip_serializing_if = "Option::is_none")]
    pub next: Option<u32>,
}

impl Entry {
    /// Size of an entry, without the jump address that may follow it.
    pub fn size(long: bool) -> u32 {
        if long {
            24
        } else {
            16
        }
    }

    /// Address of the next entry in ROM order.
    pub fn end(&self) -> u32 {
        self.rom + Self::size(self.long) + if self.next.is_some() { 4 } else { 0 }
    }

    /// The longs the object receives after the duration and the flags: frame address, then the frame data
    /// (`FRAME_DATA_A`...: 2 longs, 4 for a long entry).
    pub fn longs(&self, frames: &Frames) -> impl Iterator<Item = u32> + '_ {
        let frame = frames.number(self.frame).rom;
        std::iter::once(frame).chain(self.data.chunks(4).map(|c| u32::from_be_bytes([c[0], c[1], c[2], c[3]])))
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct Anims {
    /// Tables of the sprite viewer (provenance of the families): names, set lists, variants.
    #[serde(with = "hex")]
    pub names_rom: u32,
    #[serde(with = "hex")]
    pub families_rom: u32,
    #[serde(with = "hex")]
    pub variants_rom: u32,
    pub families: Vec<Family>,
    pub sets: Vec<AnimSet>,
    /// All known entries, by increasing address.
    pub entries: Vec<Entry>,
    /// Animation sets of the characters, one per variant (routines 0x2c16, 0x2c88 and 0x2cf0; program space).
    #[serde(default)]
    pub character_sets: Vec<Pointers>,
}

impl Anims {
    pub fn load(path: &Path) -> io::Result<Self> {
        crate::json::load(path)
    }

    pub fn entry(&self, rom: u32) -> Option<&Entry> {
        self.entries.binary_search_by_key(&rom, |e| e.rom).ok().map(|i| &self.entries[i])
    }

    pub fn set(&self, rom: u32) -> Option<&AnimSet> {
        self.sets.binary_search_by_key(&rom, |s| s.rom).ok().map(|i| &self.sets[i])
    }

    /// The entry at `at`, if it has the size the reader expects (long: 24 bytes).
    pub fn entry_at(&self, at: RomAddr, long: bool) -> Option<&Entry> {
        self.entry(at.0).filter(|e| e.long == long)
    }

    /// Jump target stored at `at`, right after an entry whose flags make it jump.
    pub fn jump_at(&self, at: RomAddr) -> Option<RomAddr> {
        [false, true]
            .into_iter()
            .find_map(|long| self.entry_at(at.offset(-(Entry::size(long) as i32)), long)?.next)
            .map(RomAddr)
    }

    /// Offset word at byte `offset` of the set at `set` (the offset of script `offset / 2`).
    pub fn script_offset(&self, set: RomAddr, offset: i32) -> Option<i16> {
        let i = usize::try_from(offset).ok().filter(|o| o % 2 == 0)? / 2;
        self.set(set.0)?.scripts.get(i).copied()
    }

    /// Array of character sets stored at `rom`.
    pub fn character_sets(&self, rom: RomAddr) -> Option<&Pointers> {
        self.character_sets.iter().find(|p| p.rom == rom.0)
    }
}
