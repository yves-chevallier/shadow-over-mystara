//! Music and effects of the sound driver (Z80): `sound/music.json` holds `ddsom_sound::SoundData` (header,
//! sounds, tracks written as instruction tokens, instrument banks, sample records, driver tables; see
//! docs/assets.md and docs/son.md), plus what follows each sample in the sample ROM.

use std::io;
use std::path::Path;

use ddsom_sound::SoundData;
use serde::{Deserialize, Serialize};

use crate::json::hex;

#[derive(Serialize, Deserialize, Debug)]
pub struct Music {
    #[serde(flatten)]
    pub data: SoundData,
    /// Bytes following the end of each sample in the sample ROM (hexadecimal, trailing zeros
    /// omitted), up to the next sample of the bank or the end of the bank (`sample_tail_end`): a
    /// voice that does not loop keeps reading past the end until the driver stops it. Absent from the
    /// WAV files, which stop at the declared end.
    #[serde(default)]
    pub sample_tails: Vec<String>,
}

impl Music {
    pub fn load(path: &Path) -> io::Result<Self> {
        crate::json::load(path)
    }

    /// Number of sample records.
    pub fn sample_count(&self) -> usize {
        self.data.samples.records.len()
    }

    /// Start (address in the sample ROM) and inclusive end of sample `i`.
    pub fn sample_span(&self, i: usize) -> (usize, usize) {
        let s = self.data.samples.records[i].voice();
        let bank = usize::from(s.bank) << 16;
        (bank | usize::from(s.start), bank | usize::from(s.end))
    }

    /// Valid record: bank within the ROM, start, loop and end in order (otherwise no WAV file).
    pub fn sample_valid(&self, i: usize) -> bool {
        let s = self.data.samples.records[i].voice();
        let (start, end) = self.sample_span(i);
        let lp = usize::from(s.bank) << 16 | usize::from(s.loop_start);
        s.bank < 0x40 && start <= end && (start..=end).contains(&lp)
    }

    /// End (exclusive) of the area following sample `i`: start of the next sample of its bank, otherwise
    /// end of the bank.
    pub fn sample_tail_end(&self, i: usize) -> usize {
        let end = self.sample_span(i).1;
        let limit = ((end >> 16) + 1) << 16;
        (0..self.sample_count()).filter(|&k| self.sample_valid(k)).map(|k| self.sample_span(k).0).filter(|&s| s > end && s < limit).min().unwrap_or(limit)
    }
}

/// Tables of the QSound DSP program (`sound/qsound_dsp.json`): pan gains, ADPCM steps, filter coefficients (see
/// `qsound::DspTables`).
#[derive(Serialize, Deserialize, Debug)]
pub struct DspTables {
    pub tables: Vec<DspTable>,
}

/// Table of the DSP program: DSP word address and words.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct DspTable {
    pub name: String,
    #[serde(with = "hex")]
    pub rom: u32,
    pub values: Vec<u16>,
}

impl DspTables {
    pub fn load(path: &Path) -> io::Result<Self> {
        crate::json::load(path)
    }
}
