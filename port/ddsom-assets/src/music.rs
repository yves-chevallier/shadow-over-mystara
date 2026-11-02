//! Music and effects of the sound driver (Z80): sound table, tracks, instruments, samples and driver
//! tables. Format: `assets/sound/music.json`, see docs/assets.md and docs/son.md.
//!
//! "Linear" driver addresses (`ddsom_sound::Rom`): offset in `dd2.01`, then `dd2.02` following it.
//!
//! A track is written as a sequence of space-separated tokens, one per instruction:
//! * note (byte >= 0x20): `duration:note`, duration 1 to 7 (bits 7-5), note 0 to 31 (bits 4-0, 0 = rest);
//! * instruction (byte < 0x20): its name (`OPS`), followed by `=` and its comma-separated parameters:
//!   bytes in decimal, `step` as a word, jump offsets (`loopK`, `exitK`, `jump`) as a signed word
//!   relative to the address following the offset.

use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::json::hex;

/// Names of instructions 0 to 31 (docs/son.md §6).
pub const OPS: [&str; 32] = [
    "triplet", "tie", "dot", "up2", "mode", "step", "gate", "amp", "inst", "oct", "mtrans", "trans", "detune", "glide",
    "loop0", "loop1", "loop2", "loop3", "exit0", "exit1", "exit2", "exit3", "jump", "end", "pan", "echo", "vol", "vib",
    "trem", "lfo", "lfomode", "bank",
];

/// Parameter bytes of instruction `op`.
pub fn param_len(op: u8) -> usize {
    match op {
        0..=3 | 23 => 0,
        5 | 22 => 2,
        14..=21 => 3,
        _ => 1,
    }
}

/// Token of the instruction starting at `bytes[0]` (and its length), `None` if bytes are missing.
pub fn decode_op(bytes: &[u8]) -> Option<(String, usize)> {
    let b = *bytes.first()?;
    if b >= 0x20 {
        return Some((format!("{}:{}", b >> 5, b & 0x1f), 1));
    }
    let n = param_len(b);
    let p = bytes.get(1..1 + n)?;
    let be = |i: usize| u16::from_be_bytes([p[i], p[i + 1]]);
    let name = OPS[b as usize];
    let tok = match b {
        0..=3 | 23 => name.to_string(),
        5 => format!("{name}={}", be(0)),
        22 => format!("{name}={}", be(0) as i16),
        14..=21 => format!("{name}={},{}", p[0], be(1) as i16),
        _ => format!("{name}={}", p[0]),
    };
    Some((tok, 1 + n))
}

/// Bytes of a token.
pub fn encode_op(tok: &str) -> Option<Vec<u8>> {
    if let Some((d, n)) = tok.split_once(':') {
        let (d, n): (u8, u8) = (d.parse().ok()?, n.parse().ok()?);
        return (1..=7).contains(&d).then_some(vec![d << 5 | n & 0x1f]).filter(|_| n < 32);
    }
    let (name, args) = tok.split_once('=').unwrap_or((tok, ""));
    let op = OPS.iter().position(|&o| o == name)? as u8;
    let args: Vec<i32> = if args.is_empty() { Vec::new() } else { args.split(',').map(|a| a.parse().ok()).collect::<Option<_>>()? };
    let mut out = vec![op];
    match (op, args.as_slice()) {
        (0..=3 | 23, []) => {}
        (5 | 22, [v]) => out.extend((*v as u16).to_be_bytes()),
        (14..=21, [a, v]) => {
            out.push(*a as u8);
            out.extend((*v as u16).to_be_bytes());
        }
        (_, [v]) if param_len(op) == 1 => out.push(*v as u8),
        _ => return None,
    }
    Some(out)
}

/// Definition of a sound (non-null entry of the sound table).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Song {
    #[serde(with = "hex")]
    pub rom: u32,
    /// First byte: 0 music; 1 to 0x7f effect (priority); 0x80-0x8f single note (channel); 0x90-0x92 ADPCM.
    pub kind: u8,
    /// Music, effect: 16 offsets (from `rom`, 0 = no track on this channel).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tracks: Vec<u16>,
    /// Single note (10 bytes: flags, duration, hold, amplitude, instrument, note, tuning) or ADPCM
    /// (8 bytes: flags, 4 ignored bytes, amplitude, sample number as a big-endian word; see docs/son.md §5).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub body: Vec<u8>,
}

impl Song {
    pub fn bytes(&self) -> Vec<u8> {
        let mut b = vec![self.kind];
        for t in &self.tracks {
            b.extend(t.to_be_bytes());
        }
        b.extend(&self.body);
        b
    }

    /// Track addresses (channel, address).
    pub fn track_starts(&self) -> impl Iterator<Item = (usize, u32)> + '_ {
        self.tracks.iter().enumerate().filter(|(_, &o)| o != 0).map(|(c, &o)| (c, self.rom + o as u32))
    }
}

/// Track instructions contiguous in ROM (everything the tracks reach, following the jumps).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct CodeBlock {
    #[serde(with = "hex")]
    pub rom: u32,
    pub ops: String,
}

impl CodeBlock {
    pub fn bytes(&self) -> Option<Vec<u8>> {
        let mut b = Vec::new();
        for t in self.ops.split(' ') {
            b.extend(encode_op(t)?);
        }
        Some(b)
    }
}

/// Instrument bank: 8-byte records (sample (little-endian word, bit 15 = follows the pitch
/// variation), fine tuning, 5 envelope indices: attack, decay, sustain, slow fall, release).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Bank {
    #[serde(with = "hex")]
    pub rom: u32,
    pub instruments: Vec<[u8; 8]>,
}

/// Driver table (bytes, or little-endian words).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct DriverTable {
    pub name: String,
    #[serde(with = "hex")]
    pub rom: u32,
    pub words: bool,
    pub values: Vec<u16>,
}

impl DriverTable {
    pub fn bytes(&self) -> Vec<u8> {
        if self.words {
            self.values.iter().flat_map(|v| v.to_le_bytes()).collect()
        } else {
            self.values.iter().map(|&v| v as u8).collect()
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Music {
    /// Word (fixed ROM) giving the address of the sound header.
    #[serde(with = "hex")]
    pub header_ptr: u32,
    #[serde(with = "hex")]
    pub header: u32,
    /// Header: number of sounds (big-endian word), index of the music volume factor and of the base
    /// effects volume (`levels` table), two unread bytes; the sound table follows (4 bytes per sound:
    /// big-endian 3-byte address, 0).
    pub count: u16,
    pub music_factor: u8,
    pub sfx_volume: u8,
    pub header_rest: [u8; 2],
    /// One sound per number (`null`: empty entry).
    pub songs: Vec<Option<Song>>,
    pub code: Vec<CodeBlock>,
    /// Table of instrument banks (little-endian words, address of each bank).
    #[serde(with = "hex")]
    pub banks_table: u32,
    pub banks: Vec<Bank>,
    /// Sample table: word at `samples_ptr`; 8-byte records (bank, start, loop, end as
    /// little-endian words, reference note).
    #[serde(with = "hex")]
    pub samples_ptr: u32,
    #[serde(with = "hex")]
    pub samples_rom: u32,
    pub samples: Vec<[u8; 8]>,
    /// Bytes following the end of each sample in the sample ROM (hexadecimal, trailing zeros
    /// omitted), up to the next sample of the bank or the end of the bank (`sample_tail_end`): a
    /// voice that does not loop keeps reading past the end until the Z80 stops it. Absent from the
    /// WAV files, which stop at the declared end.
    #[serde(default)]
    pub sample_tails: Vec<String>,
    pub tables: Vec<DriverTable>,
}

impl Music {
    pub fn load(path: &Path) -> io::Result<Self> {
        crate::json::load(path)
    }

    /// Start (address in the sample ROM) and inclusive end of sample `i`.
    pub fn sample_span(&self, i: usize) -> (usize, usize) {
        let e = &self.samples[i];
        let bank = (e[0] as usize) << 16;
        (bank | u16::from_le_bytes([e[1], e[2]]) as usize, bank | u16::from_le_bytes([e[5], e[6]]) as usize)
    }

    /// Valid record: bank within the ROM, start, loop and end in order (otherwise no WAV file).
    pub fn sample_valid(&self, i: usize) -> bool {
        let e = &self.samples[i];
        let (start, end) = self.sample_span(i);
        let lp = (e[0] as usize) << 16 | u16::from_le_bytes([e[3], e[4]]) as usize;
        e[0] < 0x40 && start <= end && (start..=end).contains(&lp)
    }

    /// End (exclusive) of the area following sample `i`: start of the next sample of its bank, otherwise
    /// end of the bank.
    pub fn sample_tail_end(&self, i: usize) -> usize {
        let end = self.sample_span(i).1;
        let limit = (end >> 16) + 1 << 16;
        (0..self.samples.len()).filter(|&k| self.sample_valid(k)).map(|k| self.sample_span(k).0).filter(|&s| s > end && s < limit).min().unwrap_or(limit)
    }

    pub fn table(&self, name: &str) -> Option<&DriverTable> {
        self.tables.iter().find(|t| t.name == name)
    }

    /// Tokens of the track starting at `at`, up to its end or its first unconditional jump.
    pub fn track(&self, at: u32) -> Option<Vec<&str>> {
        let i = self.code.partition_point(|b| b.rom <= at).checked_sub(1)?;
        let block = &self.code[i];
        let mut a = block.rom;
        let mut out = Vec::new();
        for t in block.ops.split(' ') {
            let n = encode_op(t)?.len() as u32;
            if a >= at {
                out.push(t);
                if t == "end" || t.starts_with("jump=") {
                    break;
                }
            }
            a += n;
        }
        (!out.is_empty()).then_some(out)
    }
}

/// Tables of the QSound DSP program (`sound/qsound_dsp.json`): pan, ADPCM steps, filter coefficients.
/// `rom` is a DSP word address; `words` is always true.
#[derive(Serialize, Deserialize, Debug)]
pub struct DspTables {
    pub tables: Vec<DriverTable>,
}

impl DspTables {
    pub fn load(path: &Path) -> io::Result<Self> {
        crate::json::load(path)
    }
}
