//! Texts of the 8x8 character layer (descriptors read by the V2 routines, `ddsom-game/src/text.rs`) and
//! popup names (`popups.rs`). Format: `assets/texts.json`, see docs/assets.md.
//!
//! The bytes of a text are 8x8 tile numbers: the font follows ASCII (0x20 to 0x7e), 0xfc is
//! the © symbol. In the JSON, a byte text is a string: printable ASCII characters are
//! written as is, except `{` and `}`; any other byte is written `{xx}` (hexadecimal), and so is the byte following
//! 0xff (number of spaces). The terminator is not written.

use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::json::{hex, hex_vec};

/// Routine that reads a descriptor.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TextKind {
    /// 0xd5e (writes), 0xd32 (erases), 0xdca (writes without erasing the background): column, row, attributes (word),
    /// bytes terminated by `@`; `/` (0xdca: `_`) goes to the next line, 0xff n writes n spaces.
    Text,
    /// 0x1136: column, row, attributes, words terminated by `@@`; each word `t` writes tile `t` and
    /// `t + 0x10` below it; `##` skips a column, `/@` goes to the next line, `//` two lines down.
    Tall,
    /// 0x1234: like `Tall`, without column or row (position in D5, D6).
    TallAt,
    /// 0xf38 (draws), 0xf4a (erases): blocks of 2x2 tiles `t, t+1, t+0x10, t+0x11`; `##` skips two
    /// columns, `//` two lines, 0xffff n: n empty blocks, `@@` end.
    Blocks,
    /// 0x1324: rectangle of spaces (column, row, width, height).
    Box,
}

/// A descriptor. Fields depending on the kind: `col`, `row` (except `tall_at`), `attr` (except `box`), then `text`
/// (`text`), `words` (`tall`, `tall_at`, `blocks`, without the final `@@`) or `size` (`box`).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct TextDesc {
    #[serde(with = "hex")]
    pub rom: u32,
    pub kind: TextKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub col: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub row: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attr: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub words: Option<Vec<u16>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<[u8; 2]>,
    /// Found by following the code (`false`) or only by pattern search (`true`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub scanned: bool,
}

impl TextDesc {
    /// Bytes of the descriptor in ROM, terminator included.
    pub fn bytes(&self) -> Vec<u8> {
        let mut b = Vec::new();
        if let (Some(c), Some(r)) = (self.col, self.row) {
            b.extend([c, r]);
        }
        if let Some(a) = self.attr {
            b.extend(a.to_be_bytes());
        }
        match self.kind {
            TextKind::Text => {
                b.extend(decode(self.text.as_deref().unwrap_or("")));
                b.push(b'@');
            }
            TextKind::Tall | TextKind::TallAt | TextKind::Blocks => {
                for w in self.words.iter().flatten() {
                    b.extend(w.to_be_bytes());
                }
                b.extend(*b"@@");
            }
            TextKind::Box => b.extend(self.size.unwrap_or_default()),
        }
        b
    }
}

/// Descriptor table read by the code: longs (addresses) or words (offsets from the table).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct TextTable {
    #[serde(with = "hex")]
    pub rom: u32,
    /// Read in program space (PC-relative access).
    pub program: bool,
    /// Offset words from the table rather than longs.
    pub offsets: bool,
    /// Designated descriptors, in table order.
    #[serde(with = "hex_vec")]
    pub entries: Vec<u32>,
}

/// Names displayed in popups (0x19fd2): tile strings terminated by `/`, designated by number.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PopupNames {
    /// Offset table (words, program space), one per text number.
    #[serde(with = "hex")]
    pub table: u32,
    pub offsets: Vec<i16>,
    /// The strings, contiguous from `strings_rom`, without the final `/`.
    #[serde(with = "hex")]
    pub strings_rom: u32,
    pub strings: Vec<String>,
}

/// `@`-terminated names (without position or attributes), contiguous in ROM, designated by a table of longs:
/// names of the bosses of the life bar (0xd6232, data), of the characters (0xb20e8, program).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct NameGroup {
    #[serde(with = "hex")]
    pub table: u32,
    pub program: bool,
    #[serde(with = "hex_vec")]
    pub entries: Vec<u32>,
    /// The names, contiguous from `rom`, without the final `@` (including those that no entry
    /// designates).
    #[serde(with = "hex")]
    pub rom: u32,
    pub names: Vec<String>,
}

impl NameGroup {
    /// Address of each name.
    pub fn starts(&self) -> Vec<u32> {
        let mut a = self.rom;
        self.names
            .iter()
            .map(|n| {
                let s = a;
                a += decode(n).len() as u32 + 1;
                s
            })
            .collect()
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Texts {
    pub texts: Vec<TextDesc>,
    pub tables: Vec<TextTable>,
    pub popups: PopupNames,
    pub names: Vec<NameGroup>,
}

impl Texts {
    pub fn load(path: &Path) -> io::Result<Self> {
        crate::json::load(path)
    }

    pub fn by_rom(&self, rom: u32) -> Option<&TextDesc> {
        self.texts.binary_search_by_key(&rom, |t| t.rom).ok().map(|i| &self.texts[i])
    }
}

/// Bytes -> JSON string (see the module header).
pub fn encode(bytes: &[u8]) -> String {
    let mut s = String::new();
    let mut count = false;
    for &b in bytes {
        if !count && (0x20..=0x7e).contains(&b) && b != b'{' && b != b'}' {
            s.push(b as char);
        } else {
            s.push_str(&format!("{{{b:02x}}}"));
        }
        count = !count && b == 0xff;
    }
    s
}

/// JSON string -> bytes.
pub fn decode(s: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '{' {
            let h: String = chars.by_ref().take(3).collect();
            out.push(u8::from_str_radix(&h[..2], 16).unwrap_or(0));
        } else {
            out.push(c as u8);
        }
    }
    out
}
