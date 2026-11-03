//! The residue: the bytes of the program ROM that no typed table describes, read as a last resort.
//!
//! The game reads its data through the typed tables of [`crate::GameData`]. A read that no table answers (a key that
//! the extraction does not know, an index past the end of a table on a path that no scenario plays) falls back to the
//! residue, which the game counts and logs (`ddsom-game/src/data.rs`, `Missing`): docs/fin.md, L14.
//!
//! The residue is not on the bus of the game machine (it has no ROM). Its bytes come from:
//! * with the ROMs, the program itself (data view and decrypted program view) ([`Residue::from_program`]);
//! * with the assets, `data/natives.json` (zones read without a described table) and `data/reste.json` (everything
//!   that is not 68000 code, docs/sans-rom.md) ([`Residue::load`]): the only reader of these two files. A byte that
//!   neither describes (a byte of a typed table, or of the code) reads 0 there.
//!
//! The residue is installed lazily ([`install`]): the files are only read at the first consultation, which the 309
//! scenarios never make.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use crate::remainder::Remainder;
use crate::DataFile;

/// Size of the program ROM (2 MB of data space, mirrored program space).
const PROGRAM_SIZE: u32 = 0x40_0000;

/// Runs of known bytes (start, bytes), sorted by address, of one space.
#[derive(Default)]
struct Runs(Vec<(u32, Vec<u8>)>);

impl Runs {
    fn new(mut zones: Vec<(u32, Vec<u8>)>) -> Runs {
        zones.sort_by_key(|z| z.0);
        let mut out: Vec<(u32, Vec<u8>)> = Vec::new();
        for (start, bytes) in zones {
            match out.last_mut() {
                Some((s, run)) if start <= *s + run.len() as u32 => {
                    let skip = (*s + run.len() as u32 - start) as usize;
                    run.extend(bytes.iter().skip(skip));
                }
                _ => out.push((start, bytes)),
            }
        }
        Runs(out)
    }

    fn byte(&self, a: u32) -> Option<u8> {
        let i = self.0.partition_point(|z| z.0 <= a).checked_sub(1)?;
        let (start, run) = &self.0[i];
        run.get((a - start) as usize).copied()
    }
}

/// The bytes of the residue: data space and program space (decrypted, read PC-relative by the original).
pub struct Residue {
    spaces: [Runs; 2],
}

impl Residue {
    /// The whole program ROM: data view and decrypted program view, big-endian words.
    pub fn from_program(data: &[u16], program: &[u16]) -> Residue {
        let bytes = |w: &[u16]| w.iter().flat_map(|v| v.to_be_bytes()).collect::<Vec<u8>>();
        Residue { spaces: [Runs(vec![(0, bytes(data))]), Runs(vec![(0, bytes(program))])] }
    }

    /// The residue of an assets folder: `data/natives.json` and `data/reste.json`.
    pub fn load(dir: &Path) -> io::Result<Residue> {
        let mut zones: [Vec<(u32, Vec<u8>)>; 2] = [Vec::new(), Vec::new()];
        let natives = DataFile::load(&dir.join("data/natives.json"))?;
        for t in &natives.tables {
            zones[t.program as usize].push((t.rom, t.bytes().map_err(io::Error::other)?));
        }
        for t in &natives.texts {
            zones[0].push((t.rom, t.bytes()));
        }
        let rest = Remainder::load(&dir.join("data/reste.json"))?;
        for (space, blocks) in [&rest.data, &rest.program].into_iter().enumerate() {
            for b in blocks {
                zones[space].push((b.start, b.bytes().map_err(io::Error::other)?));
            }
        }
        // beyond the encrypted zone, the program space is the data space
        let plain: Vec<(u32, Vec<u8>)> = zones[0]
            .iter()
            .filter(|z| z.0 + z.1.len() as u32 > rest.encrypted_end)
            .map(|(start, bytes)| {
                let skip = rest.encrypted_end.saturating_sub(*start);
                (start + skip, bytes[skip as usize..].to_vec())
            })
            .collect();
        zones[1].extend(plain);
        Ok(Residue { spaces: zones.map(Runs::new) })
    }

    /// Byte at `a` (24-bit address of the program ROM) of the data space, or of the program space if `program`.
    /// `None`: a byte that the residue does not hold.
    pub fn byte(&self, a: u32, program: bool) -> Option<u8> {
        (a < PROGRAM_SIZE).then(|| self.spaces[program as usize].byte(a)).flatten()
    }
}

/// Where the residue comes from, until its first consultation.
pub enum Source {
    /// An assets folder (see [`Residue::load`]).
    Assets(PathBuf),
    /// The residue itself (the program ROM, already in memory).
    Ready(Residue),
}

static SOURCE: Mutex<Option<Source>> = Mutex::new(None);
static RESIDUE: OnceLock<Residue> = OnceLock::new();

/// Makes `source` the residue of the process, unless one is already installed (like [`crate::install`]: the first
/// stays). Nothing is read before the first consultation.
pub fn install(source: Source) {
    let mut s = SOURCE.lock().unwrap();
    if s.is_none() && RESIDUE.get().is_none() {
        *s = Some(source);
    }
}

/// A residue is installed (or already read).
pub fn installed() -> bool {
    RESIDUE.get().is_some() || SOURCE.lock().unwrap().is_some()
}

/// The residue, read at the first call. Panics if none is installed, or if the assets cannot be read.
pub fn residue() -> &'static Residue {
    RESIDUE.get_or_init(|| match SOURCE.lock().unwrap().take() {
        Some(Source::Ready(r)) => r,
        Some(Source::Assets(dir)) => Residue::load(&dir)
            .unwrap_or_else(|e| panic!("residue: cannot read data/natives.json and data/reste.json of {}: {e}", dir.display())),
        None => panic!("residue not installed: install `residue::Source` with the game data"),
    })
}
