//! Tables of the enemies: `data/enemies.json` (`data().enemies`, extracted by `extract/src/enemies.rs` and
//! `combat::enemies`; docs/fin.md, L8).
//!
//! Each table is named in the file, with its address, its space (data, or program for the tables that the original
//! reads PC-relative), its format and, when known, the names of its columns. The game reads a table by name
//! ([`table`], or the named accessors below), at a byte offset like the original (`table + 4 × code`). Two kinds of
//! tables are designated by their address instead, as the original keeps it: the reaction tables of the families
//! (the address is the key that a family passes to `enemy_common::react`) and the data that a table points to (the
//! piece lists of `spawn_list`): [`reactions`], [`held8`], [`held16`].
//!
//! A read outside the bytes of a table is not in the data: it falls back to the residue (`Missing`, counted, fatal
//! with `DDSOM_STRICT=1`).

use std::collections::HashMap;
use std::sync::OnceLock;

use cps2::Cps2Bus;

use crate::data::{data, Missing, RomAddr};

/// One table: its bytes as the ROM stores them.
pub(crate) struct Table {
    pub name: &'static str,
    pub rom: u32,
    pub program: bool,
    bytes: Vec<u8>,
}

impl Table {
    fn missing(&self) -> Missing {
        Missing { table: self.name, key: self.rom }
    }

    /// Byte at offset `off` from the start of the table.
    pub fn at8(&self, b: &mut Cps2Bus, off: u32) -> u8 {
        match self.bytes.get(off as usize) {
            Some(&v) => v,
            None => self.missing().byte_in(b, self.rom.wrapping_add(off), self.program),
        }
    }

    /// Word (big-endian) at offset `off`.
    pub fn at16(&self, b: &mut Cps2Bus, off: u32) -> u16 {
        match self.slice(off, 2) {
            Some(s) => u16::from_be_bytes([s[0], s[1]]),
            None => self.missing().word_in(b, self.rom.wrapping_add(off), self.program),
        }
    }

    /// Long (big-endian) at offset `off`.
    pub fn at32(&self, b: &mut Cps2Bus, off: u32) -> u32 {
        match self.slice(off, 4) {
            Some(s) => u32::from_be_bytes([s[0], s[1], s[2], s[3]]),
            None => self.missing().long_in(b, self.rom.wrapping_add(off), self.program),
        }
    }

    /// Long `i` (offset `4 i`).
    pub fn nth32(&self, b: &mut Cps2Bus, i: u32) -> u32 {
        self.at32(b, i.wrapping_mul(4))
    }

    fn slice(&self, off: u32, n: usize) -> Option<&[u8]> {
        let start = off as usize;
        self.bytes.get(start..start.checked_add(n)?)
    }

    /// The table holds the address `a` (of its space).
    fn holds(&self, a: u32) -> bool {
        (a.wrapping_sub(self.rom) as usize) < self.bytes.len()
    }
}

struct Index {
    by_name: HashMap<&'static str, Table>,
    /// Reaction tables by address.
    reactions: HashMap<u32, &'static str>,
}

/// Prefix of the names of the reaction tables.
const REACTIONS: &str = "reactions/";

fn index() -> &'static Index {
    static INDEX: OnceLock<Index> = OnceLock::new();
    INDEX.get_or_init(|| {
        let mut by_name = HashMap::new();
        let mut reactions = HashMap::new();
        for t in &data().enemies.tables {
            let name: &'static str = Box::leak(t.name.clone().into_boxed_str());
            let bytes = t.bytes().unwrap_or_else(|e| panic!("enemy tables: {e}"));
            if name.starts_with(REACTIONS) {
                reactions.insert(t.rom, name);
            }
            by_name.insert(name, Table { name, rom: t.rom, program: t.program, bytes });
        }
        Index { by_name, reactions }
    })
}

/// The table named `name` of `data/enemies.json`. Panics if it does not exist: the names are those of the extraction.
pub(crate) fn table(name: &str) -> &'static Table {
    index().by_name.get(name).unwrap_or_else(|| panic!("data/enemies.json: no table `{name}`"))
}

/// The reaction table at the address `key` (one routine address per hit code `+0x49`, see `enemy_common::react`).
pub(crate) fn reactions(key: u32) -> &'static Table {
    let i = index();
    let name = i.reactions.get(&key).unwrap_or_else(|| panic!("data/enemies.json: no reaction table at {key:#x}"));
    &i.by_name[name]
}

/// The table of the data space that holds the address `a`.
fn holding(a: u32) -> Option<&'static Table> {
    index().by_name.values().find(|t| !t.program && t.holds(a))
}

fn residue(a: u32) -> Missing {
    Missing { table: "enemies (by address)", key: a }
}

/// Byte at the address `a` that a table points to (data space).
pub(crate) fn held8(b: &mut Cps2Bus, a: u32) -> u8 {
    match holding(a) {
        Some(t) => t.at8(b, a - t.rom),
        None => residue(a).read8(b, a),
    }
}

/// Word at the address `a` that a table points to (data space).
pub(crate) fn held16(b: &mut Cps2Bus, a: u32) -> u16 {
    match holding(a) {
        Some(t) => t.at16(b, a - t.rom),
        None => residue(a).read16(b, a),
    }
}

/// Long at the address `a` that a table points to (data space).
pub(crate) fn held32(b: &mut Cps2Bus, a: u32) -> u32 {
    match holding(a) {
        Some(t) => t.at32(b, a - t.rom),
        None => residue(a).read32(b, a),
    }
}

/// Named tables: `name()` is the table `"name"` of the file.
macro_rules! named {
    ($( $(#[$doc:meta])* $f:ident; )*) => {$(
        $(#[$doc])*
        pub(crate) fn $f() -> &'static Table {
            table(stringify!($f))
        }
    )*};
}

named! {
    /// Animation set per type (longs, data space; index: type, plus 32 outside the ENEMY class). An entry whose high
    /// word is zero is the offset of a small routine that adds the variant `+0xbf` (`anim_set_routines`).
    anim_sets;
    /// Variant added to the animation number by the routines of `anim_set_routines`, by `+0xbf` (bytes).
    anim_variants;
    /// Hit sound per type (sound envelopes; `0x3e2aa`: none).
    hit_sounds;
    /// Death sound per type.
    death_sounds;
    /// Type-specific get-up routine per type (`0x3bb9a`: none).
    get_up_routines;
    /// Types that do not stay on the ground when grabbed (bytes per type).
    no_grab_down;
    /// Predefined throws: four words per throw (X speed, X acceleration, height speed, gravity).
    launch_speeds;
    /// Predefined throws: counter per throw.
    launch_counters;
    /// Throw by reaction code (codes 68 and 72): height speed and gravity.
    pop_speeds;
    /// Stun delays drawn at random (bytes).
    daze_delays;
    /// X offset of a grabbed object in front of the player, per type (two words per type, the first read).
    seized_offsets;
    /// Damage of a grab hit of the character 5, by their hit `+0x10c` - 11.
    seized_damage;
    /// Speeds (X, X acceleration) of the impact spark of a grab hit, per type.
    seized_hit_speeds;
    /// Shaking delays drawn at random (bytes).
    quiver_delays;
    /// `enemy_lib::roll_ffda`: decision masks (words) by rank / 8.
    rank_masks;
    /// `enemy_lib::roll_78`: address of the row of masks for each value of `+0x78`.
    chance_rows;
    /// `enemy_lib::palette_script`: eight longs of colours per script.
    palette_scripts;
}

/// The circle of the whirls (`reactions`): X and depth offsets per angle (0 to 31), a speed table of
/// `data/speeds.json` read by its address.
const CIRCLE: RomAddr = RomAddr(0x1c94b6);

/// Word at byte `off` of the circle of the whirls.
pub(crate) fn circle(b: &mut Cps2Bus, off: u32) -> u16 {
    speed16(b, CIRCLE.offset(off as i32).0)
}

/// Word at the address `a` of a speed table of `data/speeds.json` (the families keep the address of their table).
pub(crate) fn speed16(b: &mut Cps2Bus, a: u32) -> u16 {
    match data().speeds.at(RomAddr(a)) {
        Some(v) => v,
        None => Missing { table: "speed table", key: a }.read16(b, a),
    }
}

/// Animation set given by the small routine of the entry `i` of `anim_sets` (the immediate operand of its
/// `movea.l #set,A4`).
pub(crate) fn anim_set_routine(i: u32) -> &'static Table {
    table(&format!("anim_set_routines/{i}"))
}

/// Tables of a piece list family of `spawn_list` (`beetle_nest`, `scorpion`): addresses of the lists, allocation
/// routines.
pub(crate) fn piece_lists(family: &str) -> (&'static Table, &'static Table) {
    (table(&format!("{family}/lists")), table(&format!("{family}/allocs")))
}
