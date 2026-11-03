//! Tables of the EFFCT family (visual effects, `families/effects*` of the game): `data/effects.json`.
//!
//! Each table is named, has its address in the program ROM (its key, kept for the tools and the rebuild), the space it
//! is read in (data, or program space for the tables that the code reads PC-relative), the size of its elements and
//! its values. [`tables!`] declares them once: the list below gives the names, addresses and meanings, the extraction
//! (`extract/src/effects.rs`) reads each table up to the next table or the next code, and the game reads them by name
//! (`data().effects.shop_prices`).
//!
//! The game reads a table at a byte offset, like the original (`table + 4 × variant`); a read past the end of the
//! extracted bytes is not in the table: the game falls back to its residue (counted, docs/fin.md).

use std::collections::BTreeMap;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::json::hex;

/// One table: address, space, element size and values (big-endian, like the 68000).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Table {
    #[serde(with = "hex")]
    pub rom: u32,
    /// Read in program space (PC-relative addressing): the decrypted view of the program.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub program: bool,
    /// Size of an element in bytes: 1, 2 or 4.
    pub size: u8,
    pub values: Vec<u32>,
    /// Name of the table (the key of the file), for the residue counters of the game.
    #[serde(skip)]
    pub name: &'static str,
}

impl Table {
    /// Number of bytes of the table.
    pub fn len(&self) -> u32 {
        self.size as u32 * self.values.len() as u32
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Byte at offset `off` from the start of the table.
    pub fn byte(&self, off: u32) -> Option<u8> {
        if off >= self.len() {
            return None;
        }
        let size = self.size as u32;
        let v = self.values[(off / size) as usize];
        Some((v >> (8 * (size - 1 - off % size))) as u8)
    }

    /// Word (big-endian) at offset `off`.
    pub fn word(&self, off: u32) -> Option<u16> {
        Some(u16::from_be_bytes([self.byte(off)?, self.byte(off.checked_add(1)?)?]))
    }

    /// Long (big-endian) at offset `off`.
    pub fn long(&self, off: u32) -> Option<u32> {
        Some((self.word(off)? as u32) << 16 | self.word(off.checked_add(2)?)? as u32)
    }

    /// The table holds the address `a` (of its space).
    pub fn contains(&self, a: u32) -> bool {
        a.wrapping_sub(self.rom) < self.len()
    }

    /// The bytes of the table.
    pub fn bytes(&self) -> Vec<u8> {
        (0..self.len()).map(|off| self.byte(off).unwrap()).collect()
    }

    /// A table made of `bytes` (whole elements of `size` bytes).
    pub fn from_bytes(spec: &Spec, bytes: &[u8]) -> Table {
        let size = spec.size as usize;
        let values = bytes.chunks_exact(size).map(|c| c.iter().fold(0u32, |v, &b| v << 8 | b as u32)).collect();
        Table { rom: spec.rom, program: spec.program, size: spec.size, values, name: spec.name }
    }
}

/// Declaration of a table: name, address, space, element size, and its end when the extraction must not guess it.
#[derive(Clone, Copy, Debug)]
pub struct Spec {
    pub name: &'static str,
    pub rom: u32,
    pub program: bool,
    pub size: u8,
    /// End (excluded); `None`: up to the next table or the next code (`extract/src/effects.rs`).
    pub end: Option<u32>,
}

/// Declares a family of tables: `/// doc` then `struct Name;`, then per table `/// meaning` and
/// `name: space size @ address [.. end];` with `space` = `data` or `prog`. Produces the struct (one field per table),
/// its declarations (`SPECS`), `build`, `tables`, `load`, `to_map` and `find`. Used by every family of named tables
/// (`effects`, `objects`).
macro_rules! tables {
    ($(#[$sdoc:meta])* struct $ty:ident;
     $( $(#[$doc:meta])* $name:ident: $space:ident $size:literal @ $rom:literal $(.. $end:literal)?; )*) => {
        $(#[$sdoc])*
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub struct $ty {
            $( $(#[$doc])* pub $name: $crate::effects::Table, )*
        }

        impl $ty {
            /// The declarations of the tables, in the order of the fields.
            pub const SPECS: &'static [$crate::effects::Spec] = &[ $( $crate::effects::Spec {
                name: stringify!($name),
                rom: $rom,
                program: $crate::effects::tables!(@program $space),
                size: $size,
                end: $crate::effects::tables!(@end $($end)?),
            }, )* ];

            /// The tables built by `make` from their declarations.
            pub fn build(mut make: impl FnMut(&$crate::effects::Spec) -> $crate::effects::Table) -> Self {
                let mut specs = Self::SPECS.iter();
                $ty { $( $name: make(specs.next().unwrap()), )* }
            }

            /// The tables with their names, in the order of the fields.
            pub fn tables(&self) -> Vec<&$crate::effects::Table> {
                vec![ $( &self.$name, )* ]
            }

            /// Loads the tables written by `extract` (a JSON object, one entry per table name).
            pub fn load(path: &std::path::Path) -> std::io::Result<Self> {
                $crate::effects::load_tables(path, |make| Self::build(make))
            }

            /// The tables by name, as written in the JSON file.
            pub fn to_map(&self) -> std::collections::BTreeMap<&'static str, &$crate::effects::Table> {
                self.tables().into_iter().map(|t| (t.name, t)).collect()
            }

            /// The table that holds the address `a` of the given space (data held by address in the state of an
            /// object).
            pub fn find(&self, a: u32, program: bool) -> Option<&$crate::effects::Table> {
                self.tables().into_iter().find(|t| t.program == program && t.contains(a))
            }
        }
    };
    (@program data) => { false };
    (@program prog) => { true };
    (@end) => { None };
    (@end $end:literal) => { Some($end) };
}
pub(crate) use tables;

/// Loads a family of tables from `path` through its `build` (see [`tables!`]): every declared table must be in the
/// file.
pub fn load_tables<T>(path: &Path, build: impl FnOnce(&mut dyn FnMut(&Spec) -> Table) -> T) -> io::Result<T> {
    let mut map: BTreeMap<String, Table> = crate::json::load(path)?;
    let mut missing = None;
    let t = build(&mut |spec: &Spec| match map.remove(spec.name) {
        Some(t) => Table { name: spec.name, ..t },
        None => {
            missing.get_or_insert(spec.name);
            Table { rom: spec.rom, program: spec.program, size: spec.size, values: Vec::new(), name: spec.name }
        }
    });
    match missing {
        Some(name) => Err(io::Error::other(format!("{}: table `{name}` missing", path.display()))),
        None => Ok(t),
    }
}

tables! {
    /// The tables of the EFFCT family, one field per table.
    struct EffectTables;

    /// `effects/f`: Start position (x, depth) and arrival abscissa (`+0x18`) per variant (words, program space).
    opening_starts: prog 2 @ 0xa8602;
    /// `effects/f`.
    opening_targets: prog 2 @ 0xa8612;
    /// `creators`: Scenery lists (SET): address of each list.
    set_list_addresses: prog 4 @ 0xa8b26 .. 0xa8b52;
    /// `creators`: Scenery lists: 4-byte entries (type, setting, word whose low byte is the variant) ended by a
    /// negative type.
    set_lists: data 1 @ 0xa8b52 .. 0xa8c00;
    /// `effects/c/screen`: Screen element fixed to the camera (type 3): x and height per variant.
    screen_piece_places: prog 2 @ 0xaa944;
    /// `effects/player`: Offsets (dx, dz) of the bubble of variant 3, by the carrier class and type, its animation
    /// and its byte +0x86.
    bubble_offsets_3: data 2 @ 0xaae2a .. 0xab24c;
    /// `effects/player`: Bubble animation code per character class, for the player byte +0x77 set.
    bubble_table_77: data 1 @ 0xab4de;
    /// `effects/player`: Bubble animation code per character class, near an object (actions 16).
    bubble_table_object_a: data 1 @ 0xab4e4;
    /// `effects/player`: Bubble animation code per character class, near an object (actions 18 and 20).
    bubble_table_object_b: data 1 @ 0xab4ea;
    /// `effects/player`: Bubble animation code per character class (action 14).
    bubble_table_character: data 1 @ 0xab4f0;
    /// `effects/player`: Bubble animation code by the hit counter of the player.
    bubble_table_hit: data 1 @ 0xab4f6;
    /// `effects/player`: Bubble animation code by the player byte +0x10d or +0x10e (actions 4 and 6).
    bubble_table_10d_a: data 1 @ 0xab676;
    /// `effects/player`: Bubble animation code by the player byte +0x10d or +0x10e (action 12).
    bubble_table_10d_b: data 1 @ 0xab6a6;
    /// `effects/player`: Bubble animation code by the player byte +0x10d or +0x10e (actions 8 and 10).
    bubble_table_10d_c: data 1 @ 0xab6d6;
    /// `effects/player`: Offsets (dx, dz) per half-variant, words.
    halo_offsets: prog 2 @ 0xab79a;
    /// `effects/player`: Appearance delays according to `+0x7d` of the player.
    halo_delays: prog 1 @ 0xab7b2;
    /// `effects/bursts`.
    debris_sheets: data 2 @ 0xab8a2;
    /// `effects/bursts`: Number of siblings per variant (program space); ROM sheets (data).
    debris_numbers: prog 1 @ 0xab95e;
    /// `effects/bursts`.
    shard_single_animations_mode: prog 1 @ 0xaba84;
    /// `effects/followers`.
    trace_animations_type: prog 1 @ 0xabb26;
    /// `effects/screen`.
    trail_positions: prog 2 @ 0xabc54;
    /// `effects/c/mechanisms`: X of each slab (words, program space); variants of the SET 6 scenery placed on
    /// pressing and release (bytes, program space).
    tiles_positions: prog 2 @ 0xabd28;
    /// `effects/c/mechanisms`: Sinking slabs (type 10): variant of the SET 6 scenery placed when a slab is stepped
    /// on, per slab variant.
    tiles_pressed: prog 1 @ 0xabe7e;
    /// `effects/c/mechanisms`: Same, when the slab is released.
    tiles_released: prog 1 @ 0xabe8e;
    /// `effects/e/attacks`: Variant 0: spots around the attacker (offset word per attacker type, then eight (dx,
    /// dz)).
    attack_fx_flash_places: data 2 @ 0xabfbe .. 0xac2de;
    /// `effects/e/attacks`: Attack effects (type 11), spark of variants 1 and 5 (mode 0): animation per attacker
    /// type.
    burst_anims: prog 1 @ 0xac38c;
    /// `effects/e/attacks`: Speeds (vx, vz) of the spark of mode 0, per attacker type.
    burst_speeds: data 2 @ 0xac3cc;
    /// `effects/e/attacks`: Speeds (vx, vz) of the spark of mode 1, per class and type of the carrier.
    burst_speeds_by_type: data 2 @ 0xac526;
    /// `effects/e/attacks`.
    attack_fx_mark_anims: prog 1 @ 0xac6ca;
    /// `effects/e/attacks`: Variant 2: offsets (dx, dz) and animation per attacker type (program space).
    attack_fx_mark_places: prog 2 @ 0xac6ea;
    /// `effects/e/attacks`: Offsets (dx, dz, signed bytes) of the reflection (variant 3), per attacker type and
    /// mode.
    glint_offsets: prog 1 @ 0xac80a;
    /// `effects/e/attacks`: Animation of the rising column (variant 4) per mode.
    rise_anims: prog 1 @ 0xac9f0;
    /// `effects/e/attacks`: Top height of the rising column, per mode and attacker type.
    rise_limits: prog 2 @ 0xaca5e;
    /// `effects/e/attacks`: Pieces of a broken object (variant 6): non-zero when the attacker type has a piece of
    /// mode 2.
    parts_second: prog 1 @ 0xacafe;
    /// `effects/e/attacks`: Offsets (dx, dz) of the piece of mode 1, per attacker type.
    parts_offsets_1: prog 2 @ 0xacc06;
    /// `effects/e/attacks`: Offsets (dx, dz) of the piece of mode 2 (dx = 0: no piece), per attacker type.
    parts_offsets_2: prog 2 @ 0xacd8a;
    /// `effects/e/attacks`: Offsets (dx, dz) of the piece of mode 3 (dz = 0: placed on the attacker), per attacker
    /// type.
    parts_offsets_3: data 2 @ 0xacf02;
    /// `effects/e/attacks`: Spray of sparkles (variant 7): list number per attacker type (0: none).
    sparks_lists: prog 1 @ 0xad034;
    /// `effects/e/attacks`: Lists of the sparkles: offsets, then per list a count and (vx, vz, mode) per sparkle.
    sparks_data: data 2 @ 0xad0a8;
    /// `effects/e/attacks`: Variants 8 and 9: spray sheet per attacker type (byte, program space: 0 = none), sheets
    /// (data: offsets, then count minus one and (vx, vz, setting, padding) per spark), spark animation per setting.
    attack_fx_shower_sheets: prog 1 @ 0xad1b4;
    /// `effects/e/attacks`.
    attack_fx_shower_data: data 2 @ 0xad232;
    /// `effects/e/attacks`.
    attack_fx_shower_anims: prog 1 @ 0xad354;
    /// `effects/c/paths`: Delay (index into the previous one's history) per rank `+0x2f` (bytes, program space).
    link_delays: prog 1 @ 0xad536;
    /// `effects/c/paths`: Chain link (type 12): sheet of 16 bytes per carrier type (0x20: not an enemy). Byte 0,
    /// negative: the link disappears, zero: it lands on the carrier, offset at 6.
    link_breaks: data 2 @ 0xad5e2;
    /// `effects/e/gates`: Animation sets per mode `+0x2f` (longs, program space).
    gate_anim_sets: prog 2 @ 0xad954;
    /// `effects/e/gates`: Curtain of the passage (type 14): per step, offset in the tiles (word, negative: end) and
    /// address in the layer (long).
    gate_curtain_steps: data 2 @ 0xadad0 .. 0xadbf2;
    /// `effects/e/gates`: Tiles of the curtain of the passage, bands of 22.
    gate_curtain_tiles: data 2 @ 0xadbf2 .. 0xae11a;
    /// `effects/e/gates`: X offset of the players, per character, on return (words, program space).
    gate_return_offsets: prog 2 @ 0xae3b4;
    /// `effects/c/carriers`.
    double_directions: prog 1 @ 0xae756;
    /// `effects/c/carriers`.
    double_speeds: prog 1 @ 0xae75a;
    /// `effects/c/carriers`: Per variant: arrival x (words), initial direction and speed table (bytes), space
    /// programme.
    double_targets: prog 2 @ 0xae76e;
    /// `effects/screen`: Offsets of variant 1 per character.
    interface_offsets_1: prog 2 @ 0xae93c;
    /// `effects/screen`.
    interface_shapes_2: prog 2 @ 0xae9c2;
    /// `effects/screen`.
    interface_offsets_2: prog 2 @ 0xae9ce;
    /// `effects/screen`: Variant 2: animations and offsets per player.
    interface_animations_2: prog 2 @ 0xae9e6;
    /// `effects/screen`: Player interface (type 17): the same lists read in data space, palette slots ended by a
    /// negative word.
    interface_list_slots: data 2 @ 0xaeb62;
    /// `effects/screen`: Player interface (type 17): offset of the palette list per player (program space), the
    /// lists follow.
    interface_lists: prog 2 @ 0xaeb62;
    /// `effects/screen`.
    interface_offsets_4: prog 2 @ 0xaec8e;
    /// `effects/c/shops`: Layer address of the gold digits per player.
    purse_digits: data 4 @ 0xaef50;
    /// `effects/c/shops`: Same, two players.
    purse_digits_2p: data 4 @ 0xaef60;
    /// `effects/c/shops`: Layer address of the life gauge per player.
    purse_gauge: data 4 @ 0xaef6c;
    /// `effects/c/shops`: Same, two players.
    purse_gauge_2p: data 4 @ 0xaef7c;
    /// `effects/c/shops`: Gold donation cursor (type 20): offset (dx, dz) per mode, one or four players.
    purse_offsets: data 2 @ 0xaef88;
    /// `effects/c/shops`: Same, two players.
    purse_offsets_2p: data 2 @ 0xaef98;
    /// `effects/c/shops`: Variant of the icon (SOBJ) per player.
    purse_icon_variant: data 1 @ 0xaefa4;
    /// `effects/c/shops`: Same, two players.
    purse_icon_variant_2p: data 1 @ 0xaefa8;
    /// `effects/c/shops`: Position (x, y) of the icon per player.
    purse_icon_pos: data 2 @ 0xaefac;
    /// `effects/c/shops`: Same, two players.
    purse_icon_pos_2p: data 2 @ 0xaefbc;
    /// `effects/c/shops`: Portrait palette per class (offsets in 0x256000).
    purse_portrait: data 2 @ 0xaefc8;
    /// `effects/c/shops`: Tiles of the end of the life gauge (8 points per tile): the last one at 0xaefee for a
    /// full tile, the previous ones for 1 to 7 points.
    purse_gauge_end: data 2 @ 0xaefe0 .. 0xaeff0;
    /// `effects/c/shops`: Stock per difficulty (`A5+0x35`, bytes, program space).
    shop_stock: prog 1 @ 0xaf4c4;
    /// `effects/c/shops`.
    shop_messages: prog 2 @ 0xaf4ca;
    /// `effects/c/shops`.
    shop_prices: data 4 @ 0xaf566;
    /// `effects/c/shops`.
    shop_animations: data 2 @ 0xaf69e;
    /// `effects/c/shops`.
    shop_items: data 2 @ 0xaf73a;
    /// `effects/c/shops`: Shop counter (type 22), per variant: sales zone (4 words: dx, half-width, dy,
    /// half-depth).
    shop_zones: data 2 @ 0xaf7d6;
    /// `effects/c/shops`: Shop counter: per variant, the bra.w (opcode, displacement) to the purchase kind.
    shop_purchases: prog 2 @ 0xafade .. 0xafc16;
    /// `effects/c/voting`: Number of choices per mode `+0x2f` (bytes, data).
    vote_choices: data 1 @ 0xafec8;
    /// `effects/c/voting`: Modes where the up/down directions also count (bytes, program space).
    vote_vertical: prog 1 @ 0xb03b6;
    /// `effects/c/voting`: Vote (type 23): cursor positions. Addresses of the parts per number of players (longs),
    /// then per part an offset per mode and (x, z) per choice and player.
    vote_cursor_places: data 2 @ 0xb042c .. 0xb0990;
    /// `effects/c/voting`: Vote: countdown position, same layout as the cursor positions.
    vote_timer_places: data 2 @ 0xb09e0 .. 0xb0a7c;
    /// `effects/c/voting`: Vote: text zones of the choices that blink, same layout (count, size, then the zone).
    vote_text_zones: data 2 @ 0xb0a9c .. 0xb0ff2;
    /// `effects/player`.
    accessory_offsets: prog 2 @ 0xb10c0;
    /// `effects/screen`.
    cloud_animations_number: prog 1 @ 0xb1264;
    /// `effects/screen`.
    cloud_speeds: prog 2 @ 0xb126a;
    /// `effects/bursts`: Animation numbers drawn at random, per draw (0-15), when the target flag is zero.
    impact_random_a: prog 1 @ 0xb14a2;
    /// `effects/bursts`: Animation numbers drawn at random when the target flag is set.
    impact_random_b: prog 1 @ 0xb14b2;
    /// `effects/bursts`: Flags per enemy / boss type (two bytes: forbidden, flag).
    impact_enemies: prog 1 @ 0xb1544;
    /// `effects/bursts`.
    impact_boss: prog 1 @ 0xb1586;
    /// `effects/bursts`: Impact offsets (dx, dz) per enemy type.
    impact_offsets_enemy: data 2 @ 0xb165a;
    /// `effects/bursts`: Impact offsets (dx, dz) per type, for the targets that are not enemies.
    impact_offsets_other: data 2 @ 0xb16de;
    /// `creators`: School of fish (type 30): per fish, depth offset and speed.
    fish_school: prog 2 @ 0xb17ae;
    /// `creators`: Four fish around the alternate camera: offset (dx, dy) each.
    fish_swimmers: prog 2 @ 0xb1808;
    /// `effects/d`.
    coin_anims: prog 1 @ 0xb1b66;
    /// `effects/d`: Speeds (vx, vz, az) per draw (8 entries of 8 bytes, program space) and animations.
    coin_speeds: prog 2 @ 0xb1b76;
    /// `effects/d`: Base animations per mode `+0x2f` (words, data) in set 0x190ba8.
    reward_anims: data 2 @ 0xb1c9a;
    /// `effects/d`: Places of the trophies: x, y (words) and facing (byte), 8 bytes per place, the controller of
    /// variant 0xb using those from 0x30.
    trophy_places: data 2 @ 0xb1cb4;
    /// `effects/d`: Delays before the animation, drawn at random (bytes, program space).
    trophy_delays: prog 1 @ 0xb1d4e;
    /// `effects/d`: Vertical offset per rank (words, program space).
    line_offsets: prog 2 @ 0xb1f6a;
    /// `effects/d`: Place in background 2 (offset from 0x912000) per rank, program space.
    line_places: prog 2 @ 0xb1fa8;
    /// `effects/d`: First tile of the portrait per character (words, program space).
    line_portrait_tiles: prog 2 @ 0xb1fe0;
    /// `effects/d`.
    line_name_attrs: prog 1 @ 0xb20b4;
    /// `effects/d`: Character names (long pointers per pair of classes, program space; strings terminated by 0x40,
    /// data).
    line_names: prog 2 @ 0xb20e8;
    /// `effects/d`: Character names of the result rows, strings ended by 0x40 (designated by line_names).
    line_name_texts: data 1 @ 0xb2100 .. 0xb2130;
    /// `effects/d`: Text after the score of a result row ("pts", ended by 0x40).
    line_score_suffix: data 1 @ 0xb21d0 .. 0xb21d4;
    /// `effects/d`: Attribute of the number and of the name per player (bytes, program space).
    line_number_attrs: prog 1 @ 0xb21d4;
    /// `effects/d`: Small digits (words, program space); text that follows the score (data).
    line_small_digits: prog 2 @ 0xb2240;
    /// `effects/d`: Digit tiles (words, program space).
    digit_tiles: prog 2 @ 0xb2334;
    /// `effects/d`: Sheets of 12 bytes per rank: x, z, y, target x, target y (words) and first sub-state (byte).
    walker_places: prog 2 @ 0xb2432;
    /// `effects/d`: Heights per value `+0x42` (words, program space).
    sign_heights: prog 2 @ 0xb2908;
    /// `effects/d`: Big title characters (words, data), default titles (8 bytes per character, + 0x60 with
    /// `A5+0x70`), special titles (codes 0xf9 to 0xfe).
    line_glyphs: data 2 @ 0xb2920;
    /// `effects/d`: First tile of the special titles 0xf9 to 0xfe of a result row.
    line_special_titles: data 2 @ 0xb2a24;
    /// `effects/d`.
    line_default_titles: data 4 @ 0xb2a30;
    /// `effects/bursts`.
    shard_speeds: prog 2 @ 0xb2d52;
    /// `effects/bursts`.
    shard_positions: prog 2 @ 0xb2d72;
    /// `effects/f`: Reflection animation per silhouette frame (bytes, program space; negative: hidden).
    mirrored_reflections: prog 1 @ 0xb300c;
    /// `effects/c/attacks`.
    lob_speed_tables: prog 1 @ 0xb3614;
    /// `effects/c/attacks`.
    lob_speeds: prog 2 @ 0xb361e;
    /// `effects/c/attacks`: Per variant: target (x, y) relative to the camera (words), speed table (bytes), speed
    /// (long, 16.16), program space.
    lob_targets: prog 2 @ 0xb3762;
    /// `effects/c/attacks`: Attack effects (type 40): speeds (vx, vz) of the debris thrown behind, per half-variant
    /// of the owner.
    blow_speeds_behind: data 2 @ 0xb3862;
    /// `effects/c/attacks`: Speeds (vx, vz) of the debris thrown in front.
    blow_speeds_front: data 2 @ 0xb3894;
    /// `effects/c/attacks`: Variant 1: offset (dx, dz) per pair of attacker variants (program).
    blow_glow_places: prog 2 @ 0xb3968;
    /// `effects/c/attacks`: Variant 2: x offset per pair of variants, animation per setting, stop height per
    /// setting and pair of variants (program).
    blow_fall_places: prog 2 @ 0xb3a2a;
    /// `effects/c/attacks`.
    blow_fall_anims: prog 1 @ 0xb3a36;
    /// `effects/c/attacks`.
    blow_fall_floors: prog 2 @ 0xb3a98;
    /// `effects/c/attacks`: Variant 4: offset (x, z) and speeds (x, ax), 8 bytes per pair of variants (program).
    blow_debris: prog 2 @ 0xb3af8;
    /// `effects/c/attacks`: Per kind: speeds (x, az) (words) and start / landing sound (longs, 0: none), program
    /// space.
    toss_speeds: prog 2 @ 0xb3cbc;
    /// `effects/c/attacks`: Object thrown at random (type 41): sound envelope played at the throw, per kind (0:
    /// none).
    toss_sounds_start: prog 4 @ 0xb3cd0;
    /// `effects/c/attacks`: Sound envelope played at the landing, per kind (0: none).
    toss_sounds_land: prog 4 @ 0xb3d52;
    /// `effects/c/attacks`: Bounce speeds (x, az) according to `+0x33` (program).
    toss_bounces: prog 2 @ 0xb3dbe;
    /// `creators`: Cries of a boss (scene effects of variant 1): 8-byte sheets drawn at random, count minus one
    /// then the settings.
    cries_a: data 1 @ 0xb3e86 .. 0xb3ec6;
    /// `creators`: Same, variant 2.
    cries_b: data 1 @ 0xb3f00 .. 0xb3f40;
    /// `effects/c/scenery_fx`: Scene effects (type 42), variant 1: per setting, 16-byte sheet (dx, dy, vx, ax, vy,
    /// ay).
    scenery_throws_1: data 2 @ 0xb4202;
    /// `effects/c/scenery_fx`: Variant 2: per setting, offset (dx, dy).
    scenery_places_2: data 2 @ 0xb4394;
    /// `effects/c/scenery_fx`: Speeds (vz, az) per mode `+0x2f`: variant 3 (data), variant 4 (data).
    scenery_fall_3: data 2 @ 0xb4480;
    /// `effects/c/scenery_fx`: X speeds of variant 4 (program space, random even index).
    scenery_drift_4: prog 2 @ 0xb45aa;
    /// `effects/c/scenery_fx`.
    scenery_fall_4: data 2 @ 0xb45ba;
    /// `effects/c/scenery_fx`: Variant 8: spawn zone (x, mask, y, mask) per mode.
    scenery_areas_8: data 2 @ 0xb4986;
    /// `effects/c/scenery_fx`: Variant 9: per setting (`& 7`), 8-byte sheet (dx, dz as signed bytes, then vz, az;
    /// program).
    scenery_spray: prog 1 @ 0xb4a32;
    /// `effects/c/scenery_fx`: Variant 10: animation (bytes) and speeds (vx, ax) per mode (program space).
    scenery_anims_10: prog 1 @ 0xb4af2;
    /// `effects/c/scenery_fx`.
    scenery_speeds_10: prog 2 @ 0xb4b56;
    /// `effects/g/thrown`: Tables (program space): animation speed per variant, then draws (X and Z speeds,
    /// animation) for the rain around the camera and for the creator's sparks.
    rain_speed_by_variant: prog 1 @ 0xb4d7a;
    /// `effects/g/thrown`: Animations drawn at random, camera variant.
    rain_camera_anims: prog 1 @ 0xb4de4;
    /// `effects/g/thrown`: Rain of debris (type 43) of the camera variant: X speeds drawn at random.
    rain_camera_vx: prog 2 @ 0xb4e04;
    /// `effects/g/thrown`: Z speeds drawn at random, camera variant.
    rain_camera_vz: prog 2 @ 0xb4e24;
    /// `effects/g/thrown`.
    rain_shard_speed: prog 1 @ 0xb4e94;
    /// `effects/g/thrown`: X speeds drawn at random, shard variant.
    rain_shard_vx: prog 2 @ 0xb4ea4;
    /// `effects/g/thrown`: Z speeds drawn at random, shard variant.
    rain_shard_vz: prog 2 @ 0xb4ec4;
    /// `effects/g/thrown`: Animations drawn at random, shard variant.
    rain_shard_anims: prog 1 @ 0xb4ee4;
    /// `effects/c/attacks`.
    spell_anims_a: prog 1 @ 0xb51e4;
    /// `effects/c/attacks`.
    spell_anims_b: prog 1 @ 0xb5200;
    /// `effects/c/attacks`.
    spell_tints: prog 1 @ 0xb5224;
    /// `effects/c/attacks`: Magician spells (type 44): offset (dx, dz) per variant.
    spell_offsets_a: prog 2 @ 0xb5298;
    /// `effects/c/attacks`: Wide spells: offset (dx, dz) and turn per variant (8 bytes).
    spell_offsets_b: prog 2 @ 0xb530e;
    /// `effects/followers`.
    trigger_offsets: prog 1 @ 0xb53ec;
    /// `effects/g/enemy`.
    armour_anim_sets: prog 4 @ 0xb54dc;
    /// `effects/g/enemy`: Thresholds (bytes, program space) and animation sets (longs) per piece.
    armour_thresholds: prog 1 @ 0xb550e;
    /// `effects/g/enemy`: Launch of a shard, 16-byte sheet per kind: dx, dy, dz, vx, vy (words), vz and az (long).
    armour_shard_launch: data 2 @ 0xb55dc;
    /// `effects/g/enemy`: Armour pieces (type 46): shards of a fallen piece, per piece 4 bytes: count minus one,
    /// then the kinds.
    armour_shards: data 1 @ 0xb5720;
    /// `effects/c/screen`: Blinking marker (type 47): offsets x, z, y and animation per variant (8 bytes), from
    /// variant -1 (0xff, read by the original).
    beacon_sheets: prog 2 @ 0xb57e4;
    /// `effects/c/carriers`: Duration (variant 5) and random offsets, words indexed by 4 bits of the generator
    /// (space programme).
    attached_delays: prog 2 @ 0xb6160;
    /// `effects/c/carriers`.
    attached_jitter: prog 2 @ 0xb6180;
    /// `creators`: Sparkle of a character (type 52): rise per byte +0x60 of the creator.
    shimmer_lifts: prog 2 @ 0xb65d6;
    /// `effects/f`: Positions (x, depth) per variant (words, program space).
    passage_places: prog 2 @ 0xb693c;
    /// `effects/bursts`.
    shards_debris_speeds_x: prog 2 @ 0xb6d18 .. 0xb6d36;
    /// `effects/bursts`.
    shards_debris_speeds_y: prog 2 @ 0xb6d28 .. 0xb6d46;
    /// `effects/c/carriers`.
    double2_directions: prog 1 @ 0xb74fe;
    /// `effects/c/carriers`.
    double2_speeds: prog 1 @ 0xb7502;
    /// `effects/c/carriers`.
    double2_targets: prog 2 @ 0xb7516;
    /// `effects/c/paths`: Courier (type 65): path of pairs (direction, duration) ended by a negative direction, the
    /// object keeps its place in the path.
    courier_path: data 1 @ 0xb7b1e;
    /// `effects/c/paths`: Courier: 8-byte sheet per variant: animation, type, variant, mode (bytes), x, z (words)
    /// of the object it spawns.
    courier_cards: data 2 @ 0xb7bec;
    /// `effects/c/screen`: Four large characters in background 2 (type 66): per step, four tiles and their
    /// attribute (10 bytes), ended by a negative word
    letters_script: data 2 @ 0xb7d84 .. 0xb7dea;
    /// `effects/c/mechanisms`: Rising column (type 67): delay per variant.
    pillar_delays: prog 1 @ 0xb7ea2;
    /// `effects/c/shops`.
    counter_messages: prog 2 @ 0xb8126;
    /// `effects/c/shops`.
    counter_animations: data 2 @ 0xb812c;
    /// `effects/c/shops`: Zone (4 words, data: a single entry), announcement per variant (words, program space),
    /// animation per variant (words, data).
    counter_zone: data 2 @ 0xb8132;
    /// `effects/c/shops`: Shop counter of a scene (type 70): address of the goods list per variant.
    counter_goods: prog 4 @ 0xb82e0 .. 0xb82ec;
    /// `effects/c/shops`: Goods of a scene shop: per good, setting, variant (bytes), x, z, y (words), ended by a
    /// negative setting.
    counter_goods_lists: data 1 @ 0xb82ec .. 0xb8352;
    /// `effects/g/rockets`: Fireworks (type 80): variant of each rocket (type 50).
    rocket_variants: prog 1 @ 0xb9628;
    /// `effects/g/rockets`: Delay before the next rocket.
    rocket_delays: prog 2 @ 0xb9638;
    /// `effects/g/rockets`: Place (x, z) of each rocket, relative to the camera.
    rocket_places: prog 2 @ 0xb9658;
    /// `effects/c/ending`: Offsets (x, z) of the spark per pair of carrier variants (program space).
    shards_spark_offsets: prog 2 @ 0xb98b6;
    /// `effects/c/ending`: X offsets of the drops, 16 words per direction (program space).
    shards_rain_offsets: prog 2 @ 0xb9a2a;
    /// `effects/c/ending`: Piece of the ending sequence (type 85, variant 0): offset (x, y) per variant of its
    /// owner.
    ending_piece_offsets: prog 2 @ 0xb9d3e;
    /// `effects/c/mechanisms`: Positions (x, depth) per variant, program space.
    switch_positions: prog 2 @ 0xb9e08;
    /// `effects/c/mechanisms`: Contact box (dx, half-width, dy, half-depth), data.
    switch_hit_box: data 2 @ 0xb9f2c;
    /// `effects/c/mechanisms`: Variant of the SET 7 scenery placed under the barrier (bytes, program space).
    barrier_set_variants: prog 1 @ 0xba034;
    /// `effects/c/mechanisms`: Barrier (type 88): variants of its switches (type 87), 4 bytes per barrier variant,
    /// ended by a negative byte.
    barrier_switches: data 1 @ 0xba07c;
    /// `effects/c/screen`: Screen element fixed to the camera (type 91): x and height per variant.
    screen_piece_b_places: prog 2 @ 0xba2a0;
    /// `effects/g/thrown`: Position of the hand of a character per gesture: 8 bytes, dx and dz at offsets 2 and 4
    /// (also read by the player effects).
    hand_offsets: data 2 @ 0x11ae86 .. 0x11db2a;
    /// `effects/c/scenery_fx`: Sprite frames of the glow (variant 7), longs.
    scenery_glow_frames: data 4 @ 0x17efca;
    /// `effects/g/enemy`: Flying machine (type 48): circle of 32 positions (dx, dy) of its cannons
    flyer_path: data 2 @ 0x1ca4b6 .. 0x1ca536;
}
