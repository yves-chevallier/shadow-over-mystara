//! Spawn scripts of the scenes (enemy waves, scenery, objects): `assets/data/spawns.json`, read by
//! `ddsom-game/src/scene/script.rs` (0xf2e5c). See docs/assets.md and docs/scenes.md §2.

use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::json::{hex, hex_vec};

/// Script entry (the word at `+2` tells them apart: 0xffff for a call).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(untagged)]
pub enum Entry {
    /// Call (10 bytes): at abscissa `x`, calls routine `call` with D0 = `arg`.
    Call {
        x: i16,
        #[serde(with = "hex")]
        call: u32,
        arg: u16,
    },
    /// Spawn (0x12 bytes): at camera abscissa `x`, creates an object of class `class` (even byte,
    /// index of the allocators `0xf2f38`: ENEMY, SET, EFFCT, BOX, SOBJ, BOSS, ITEM, ESHL...), of type `kind` and
    /// variant `variant`, at X = `x + dx`, Z = `z`, Y = `y`, if the game rank is at most `rank` and there are at
    /// least `players` players; `f2c`, `f2f` and `f42` are copied to `+0x2c`, `+0x2f` and `+0x42` (word) of
    /// the object; `unused` (byte `+0xa`) is not read.
    Spawn {
        x: i16,
        dx: i16,
        z: i16,
        y: i16,
        rank: u8,
        players: u8,
        unused: u8,
        class: u8,
        f2c: u8,
        kind: u8,
        variant: u8,
        f2f: u8,
        f42: u16,
    },
}

impl Entry {
    pub fn bytes(&self) -> Vec<u8> {
        let mut b = Vec::new();
        match *self {
            Entry::Call { x, call, arg } => {
                b.extend(x.to_be_bytes());
                b.extend([0xff, 0xff]);
                b.extend(call.to_be_bytes());
                b.extend(arg.to_be_bytes());
            }
            Entry::Spawn { x, dx, z, y, rank, players, unused, class, f2c, kind, variant, f2f, f42 } => {
                for w in [x, dx, z, y] {
                    b.extend(w.to_be_bytes());
                }
                b.extend([rank, players, unused, class, f2c, kind, variant, f2f]);
                b.extend(f42.to_be_bytes());
            }
        }
        b
    }
}

/// A script: entries up to the final 0xffff word (not stored), then `pad`: the padding word `0x4e71` that
/// separates some scripts from the next one.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Script {
    #[serde(with = "hex")]
    pub rom: u32,
    pub entries: Vec<Entry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pad: Vec<u8>,
}

impl Script {
    pub fn bytes(&self) -> Vec<u8> {
        let mut b: Vec<u8> = self.entries.iter().flat_map(|e| e.bytes()).collect();
        b.extend([0xff, 0xff]);
        b.extend(&self.pad);
        b
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Spawns {
    /// Script table (one long per scene, data space).
    #[serde(with = "hex")]
    pub table: u32,
    /// Script of each scene (address).
    #[serde(with = "hex_vec")]
    pub scenes: Vec<u32>,
    /// Distinct scripts, by increasing address.
    pub scripts: Vec<Script>,
}

impl Spawns {
    pub fn load(path: &Path) -> io::Result<Self> {
        crate::json::load(path)
    }

    /// Script of scene `s`.
    pub fn of_scene(&self, s: usize) -> Option<&Script> {
        let a = *self.scenes.get(s)?;
        self.scripts.iter().find(|x| x.rom == a)
    }
}
