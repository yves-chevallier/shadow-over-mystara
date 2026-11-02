//! The remainder of the program region (`data/remainder.json`): all the bytes that the other files do not describe,
//! except the 68000 code.
//!
//! The game without the 68000 no longer runs the original code: everything else in the program region is data that a
//! native routine may read. Rather than extracting only the zones that a measurement saw being read (`data/natives.json`),
//! this file stores everything that is not code, which makes the program region complete by construction. The
//! excluded code is the code the reference machine executes on all scenarios (`port/extract/code_68k.txt`,
//! docs/sans-rom.md « Couverture complète des données »); code that is never executed is therefore stored here too, like
//! tables stored inside the code and data that a call jumps over.
//!
//! * `data`: blocks of the data space (68000 reads), `start` then the bytes in hexadecimal, 32 per
//!   line;
//! * `program`: blocks of the decrypted program space (PC-relative reads), in the encrypted zone
//!   only: beyond `encrypted_end`, the program space equals the data space and the rebuild
//!   copies it;
//! * `code`: the excluded code ranges (`start-end`, end exclusive). The assets render everything else; part
//!   of this code is nevertheless described by other files (zones read by the natives, `natives.json`).

use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::json;

/// Bytes per line of a block.
pub const LINE: usize = 32;

#[derive(Serialize, Deserialize, Default)]
pub struct Remainder {
    pub description: String,
    /// End of the program's encrypted zone (exclusive): beyond it, the program space equals the data space.
    #[serde(rename = "fin_chiffree", with = "json::hex")]
    pub encrypted_end: u32,
    /// Code bytes (total of the `code` ranges).
    #[serde(rename = "octets_de_code")]
    pub code_bytes: u32,
    /// Excluded code ranges, `"0x400-0x4ec"` (end exclusive).
    pub code: Vec<String>,
    #[serde(rename = "donnees")]
    pub data: Vec<Block>,
    #[serde(rename = "programme")]
    pub program: Vec<Block>,
}

/// Consecutive bytes starting at `start`.
#[derive(Serialize, Deserialize)]
pub struct Block {
    #[serde(rename = "debut", with = "json::hex")]
    pub start: u32,
    /// Lines of LINE bytes (the last may be shorter), in hexadecimal.
    #[serde(rename = "octets")]
    pub bytes: Vec<String>,
}

impl Block {
    pub fn new(start: u32, bytes: &[u8]) -> Self {
        let lines = bytes.chunks(LINE).map(|l| l.iter().map(|b| format!("{b:02x}")).collect()).collect();
        Block { start, bytes: lines }
    }

    pub fn bytes(&self) -> Result<Vec<u8>, String> {
        let mut out = Vec::new();
        for l in &self.bytes {
            if l.len() % 2 != 0 {
                return Err(format!("block {:#x}: odd line « {l} »", self.start));
            }
            for i in (0..l.len()).step_by(2) {
                out.push(u8::from_str_radix(&l[i..i + 2], 16).map_err(|e| format!("block {:#x}: {e}", self.start))?);
            }
        }
        Ok(out)
    }
}

impl Remainder {
    pub fn load(path: &Path) -> io::Result<Self> {
        json::load(path)
    }

    /// Writes the file: one code range or one line of bytes per line.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        let string = |s: &str| serde_json::to_string(s).unwrap();
        let list = |v: &[String], n: usize| {
            let pad = "  ".repeat(n);
            let l: Vec<String> = v.iter().map(|x| format!("{pad}{}", string(x))).collect();
            if l.is_empty() { "[]".to_string() } else { format!("[\n{}\n{}]", l.join(",\n"), "  ".repeat(n - 1)) }
        };
        let blocks = |v: &[Block]| {
            let b: Vec<String> = v
                .iter()
                .map(|b| format!("    {{\"debut\": \"{:#x}\", \"octets\": {}}}", b.start, list(&b.bytes, 3)))
                .collect();
            if b.is_empty() { "[]".to_string() } else { format!("[\n{}\n  ]", b.join(",\n")) }
        };
        let out = format!(
            "{{\n  \"description\": {},\n  \"fin_chiffree\": \"{:#x}\",\n  \"octets_de_code\": {},\n  \"code\": {},\n  \"donnees\": {},\n  \"programme\": {}\n}}\n",
            string(&self.description),
            self.encrypted_end,
            self.code_bytes,
            list(&self.code, 2),
            blocks(&self.data),
            blocks(&self.program)
        );
        std::fs::write(path, out)
    }

    /// Code ranges `(start, end exclusive)`.
    pub fn code_ranges(&self) -> Result<Vec<(u32, u32)>, String> {
        self.code
            .iter()
            .map(|p| {
                let (a, b) = p.split_once('-').ok_or_else(|| format!("unreadable code range « {p} »"))?;
                match (json::hex::parse(a), json::hex::parse(b)) {
                    (Some(a), Some(b)) if a < b => Ok((a, b)),
                    _ => Err(format!("unreadable code range « {p} »")),
                }
            })
            .collect()
    }
}

/// Ranges `(start, end exclusive)` of the positions where `guard` is true.
pub fn ranges(n: u32, guard: impl Fn(u32) -> bool) -> Vec<(u32, u32)> {
    let mut out: Vec<(u32, u32)> = Vec::new();
    for a in 0..n {
        if guard(a) {
            match out.last_mut() {
                Some(r) if r.1 == a => r.1 += 1,
                _ => out.push((a, a + 1)),
            }
        }
    }
    out
}
