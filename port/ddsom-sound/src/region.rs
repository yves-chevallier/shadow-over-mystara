//! The driver data in the Z80 ROM region (MAME's "audiocpu" region): reading them from the ROMs
//! (extraction, reference machine, tools) and writing them back (assets without ROMs, lossless tests).
//!
//! The Z80 sees a fixed 0000-7FFF and a 16 KB window at 8000-BFFF whose bank is selected at D003.
//! The original program handles "linear" addresses: the address of a byte of bank `b` at window
//! offset `o` is `0x8000 + 0x4000 * b + o`, i.e. the offset in `dd2.01` followed by `dd2.02`. All
//! the `addr` fields of [`SoundData`] are linear addresses.

use std::collections::BTreeSet;
use std::io;
use std::path::Path;

use crate::data::*;
use crate::tokens::param_len;

/// Size of MAME's audiocpu region.
pub const REGION_SIZE: usize = 0x50000;

/// Where the program keeps its data pointers and tables (linear addresses of the fixed ROM).
mod at {
    /// QSound registers written at initialisation (bytes, up to register 0xE3) and their values (words).
    pub const QSOUND_INIT_REGS: u32 = 0x00d5;
    pub const QSOUND_INIT_VALUES: u32 = 0x00e1;
    pub const DURATIONS: u32 = 0x1119;
    pub const OCTAVE_OFFSETS: u32 = 0x1149;
    pub const SEMITONE_OCTAVE: u32 = 0x1fc6;
    pub const OCTAVE_START: u32 = 0x2032;
    pub const RATES: u32 = 0x203b;
    pub const FINE: u32 = 0x2113;
    /// The word tables of `LevelTables`, contiguous from here.
    pub const LEVEL_TABLES: u32 = 0x33e1;
    /// Word: address of the sound header.
    pub const HEADER_PTR: u32 = 0x39e1;
    /// Word: address of the sample records.
    pub const SAMPLES_PTR: u32 = 0x39e3;
    /// Words: address of each instrument bank.
    pub const BANKS_PTRS: u32 = 0x39e5;
}

/// Sizes of the tables, in entries.
const LEVEL_TABLE_SIZES: [usize; 8] = [128, 128, 64, 64, 128, 128, 128, 128];
const BANKS: usize = 2;
const INSTRUMENTS: usize = 256;
/// Sample records: 345 (like `sounds/`, which only writes the valid ones).
const SAMPLES: usize = 345;

/// Offset of the record of sample `id` from the start of the table (the Z80 computes `id * 8` on 16 bits).
fn record_offset(id: u16) -> u32 {
    u32::from(id.wrapping_mul(8))
}

/// Linear address -> offset in the region.
pub fn offset(addr: u32) -> usize {
    let addr = addr & 0xff_ffff;
    (if addr < 0x8000 { addr } else { addr + 0x8000 }) as usize
}

/// Loads `dd2.01` and `dd2.02` like MAME: dd2.01 = 0x8000 fixed bytes then 0x18000 bytes of banks
/// starting at 0x10000; dd2.02 starting at 0x28000.
pub fn load(dir: &Path) -> io::Result<Vec<u8>> {
    let mut region = vec![0u8; REGION_SIZE];
    let d1 = std::fs::read(dir.join("dd2.01"))?;
    region[..0x8000].copy_from_slice(&d1[..0x8000]);
    region[0x10000..0x28000].copy_from_slice(&d1[0x8000..0x20000]);
    let d2 = std::fs::read(dir.join("dd2.02"))?;
    region[0x28000..0x48000].copy_from_slice(&d2);
    Ok(region)
}

/// Reader of the region by linear address (0xFF outside, like the Z80's open bus).
struct Reader<'a>(&'a [u8]);

impl Reader<'_> {
    fn byte(&self, addr: u32) -> u8 {
        self.0.get(offset(addr)).copied().unwrap_or(0xff)
    }
    fn bytes<const N: usize>(&self, addr: u32) -> [u8; N] {
        std::array::from_fn(|k| self.byte(addr + k as u32))
    }
    fn le(&self, addr: u32) -> u16 {
        u16::from_le_bytes(self.bytes(addr))
    }
    fn be(&self, addr: u32) -> u16 {
        u16::from_be_bytes(self.bytes(addr))
    }
    fn le_words(&self, addr: u32, n: usize) -> Vec<u16> {
        (0..n as u32).map(|i| self.le(addr + 2 * i)).collect()
    }
    fn byte_vec(&self, addr: u32, n: usize) -> Vec<u8> {
        (0..n as u32).map(|i| self.byte(addr + i)).collect()
    }
}

impl SoundData {
    /// Reads the data from the ROMs (`dd2.01`, `dd2.02`).
    pub fn load(dir: &Path) -> io::Result<Self> {
        Ok(Self::from_region(&load(dir)?))
    }

    /// Reads the data from a Z80 ROM region, as the original program reads it.
    pub fn from_region(region: &[u8]) -> Self {
        let r = Reader(region);
        let header_addr = u32::from(r.le(at::HEADER_PTR));
        let count = r.be(header_addr);
        let header = Header {
            addr: header_addr,
            music_factor: r.byte(header_addr + 2),
            sfx_volume: r.byte(header_addr + 3),
            unused: r.bytes(header_addr + 4),
        };
        // Definitions, with the addresses of their tracks.
        let definitions: Vec<Option<(u32, u8)>> = (0..u32::from(count))
            .map(|i| {
                let entry = header_addr + 6 + 4 * i;
                let addr = u32::from(r.byte(entry)) << 16 | u32::from(r.be(entry + 1));
                (addr != 0).then(|| (addr, r.byte(addr)))
            })
            .collect();
        let channel_tracks = |addr: u32| -> [Option<u32>; 16] {
            std::array::from_fn(|c| match u32::from(r.be(addr + 1 + 2 * c as u32)) {
                0 => None,
                o => Some((addr + o) & 0xff_ffff),
            })
        };
        let starts = definitions.iter().flatten().filter(|(_, kind)| *kind < 0x80).flat_map(|&(addr, _)| channel_tracks(addr)).flatten();
        let tracks = cut_tracks(&r, starts.collect());
        let locate = |a: u32| locate(&tracks, a).expect("every track start is in a track");
        let sounds: Vec<Option<Sound>> = definitions
            .iter()
            .map(|d| {
                let &(addr, kind) = d.as_ref()?;
                let body = addr + 1;
                let kind = match kind {
                    0 => SoundKind::Music { tracks: channel_tracks(addr).map(|t| t.map(locate)) },
                    1..=0x7f => SoundKind::Effect { priority: kind, tracks: channel_tracks(addr).map(|t| t.map(locate)) },
                    0x80..=0x8f => SoundKind::Note(Note {
                        channel: kind & 0x7f,
                        flags: r.byte(body),
                        duration: r.be(body + 1),
                        gate: r.be(body + 3),
                        amplitude: r.byte(body + 5),
                        instrument: r.be(body + 6),
                        note: r.byte(body + 8),
                        detune: r.byte(body + 9),
                    }),
                    _ => SoundKind::Adpcm(AdpcmNote {
                        channel: kind.wrapping_sub(0x90),
                        flags: r.byte(body),
                        unused: r.bytes(body + 1),
                        amplitude: r.byte(body + 5),
                        sample: r.be(body + 6),
                    }),
                };
                Some(Sound { addr, kind })
            })
            .collect();
        let banks: Vec<Bank> = (0..BANKS as u32)
            .map(|k| {
                let addr = u32::from(r.le(at::BANKS_PTRS + 2 * k));
                let instruments = (0..INSTRUMENTS as u32)
                    .map(|i| {
                        let p = addr + 8 * i;
                        Instrument { sample: r.le(p), detune: r.byte(p + 2), envelope: r.bytes(p + 3) }
                    })
                    .collect();
                Bank { addr, instruments }
            })
            .collect();
        let samples_addr = u32::from(r.le(at::SAMPLES_PTR));
        let record = |id: u16| SampleRecord(r.bytes(samples_addr + record_offset(id)));
        let mut referenced: BTreeSet<u16> = BTreeSet::from([NO_INSTRUMENT & 0x1fff]);
        referenced.extend(banks.iter().flat_map(|b: &Bank| b.instruments.iter().map(|i| i.sample & 0x1fff)));
        referenced.extend(sounds.iter().flatten().filter_map(|s: &Sound| match &s.kind {
            SoundKind::Note(n) if n.instrument & 0x8000 == 0 => Some(n.instrument & 0x1fff),
            SoundKind::Adpcm(a) => Some(a.sample & 0x1fff),
            _ => None,
        }));
        let samples = Samples {
            addr: samples_addr,
            records: (0..SAMPLES as u16).map(record).collect(),
            past_end: referenced.into_iter().filter(|&id| usize::from(id) >= SAMPLES).map(|id| (id, record(id))).collect(),
        };
        let mut level_at = at::LEVEL_TABLES;
        let mut level = |n: usize| {
            let t = r.le_words(level_at, n);
            level_at += 2 * n as u32;
            t
        };
        let [amplitudes, levels, attack, decay, lfo_rates, vibrato, tremolo, after_tremolo] = LEVEL_TABLE_SIZES.map(&mut level);
        let mut qsound_init = Vec::new();
        for i in 0.. {
            let reg = r.byte(at::QSOUND_INIT_REGS + i);
            qsound_init.push((reg, r.le(at::QSOUND_INIT_VALUES + 2 * i)));
            if reg == 0xe3 {
                break;
            }
        }
        let tables = Tables {
            qsound_init,
            durations: r.le_words(at::DURATIONS, 24),
            octave_offsets: r.byte_vec(at::OCTAVE_OFFSETS, 16),
            semitone_octave: r.byte_vec(at::SEMITONE_OCTAVE, 108),
            octave_start: r.byte_vec(at::OCTAVE_START, 9),
            rates: r.le_words(at::RATES, 108),
            fine: (0..12).map(|s| r.byte_vec(at::FINE + 256 * s, 256)).collect(),
            levels: LevelTables { amplitudes, levels, attack, decay, lfo_rates, vibrato, tremolo, after_tremolo },
        };
        SoundData { header, sounds, tracks, banks, samples, tables }
    }

    /// The bytes of the data at their linear addresses (the inverse of `from_region`): pointers, header and
    /// sound table, definitions, tracks, banks, sample records, tables. The words read past the tremolo table
    /// are not written: they are the pointers and bank 0 written here.
    pub fn region_parts(&self) -> Vec<(u32, Vec<u8>)> {
        let le = |v: u32| (v as u16).to_le_bytes().to_vec();
        let words = |t: &[u16]| t.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<u8>>();
        let h = &self.header;
        let mut parts = vec![(at::HEADER_PTR, le(h.addr)), (at::SAMPLES_PTR, le(self.samples.addr))];
        let mut head = (self.sounds.len() as u16).to_be_bytes().to_vec();
        head.extend([h.music_factor, h.sfx_volume]);
        head.extend(h.unused);
        for s in &self.sounds {
            let a = s.as_ref().map_or(0, |s| s.addr);
            head.extend([(a >> 16) as u8, (a >> 8) as u8, a as u8, 0]);
        }
        parts.push((h.addr, head));
        for s in self.sounds.iter().flatten() {
            parts.push((s.addr, self.definition_bytes(s)));
        }
        for t in &self.tracks {
            parts.push((t.addr, t.bytes.clone()));
        }
        for (k, b) in self.banks.iter().enumerate() {
            parts.push((at::BANKS_PTRS + 2 * k as u32, le(b.addr)));
            let bytes = b.instruments.iter().flat_map(|i| {
                let mut r = i.sample.to_le_bytes().to_vec();
                r.push(i.detune);
                r.extend(i.envelope);
                r
            });
            parts.push((b.addr, bytes.collect()));
        }
        parts.push((self.samples.addr, self.samples.records.iter().flat_map(|r| r.0).collect()));
        for (id, record) in &self.samples.past_end {
            parts.push((self.samples.addr + record_offset(*id), record.0.to_vec()));
        }
        let t = &self.tables;
        let (regs, values): (Vec<u8>, Vec<u16>) = t.qsound_init.iter().copied().unzip();
        parts.push((at::QSOUND_INIT_REGS, regs));
        parts.push((at::QSOUND_INIT_VALUES, words(&values)));
        parts.push((at::DURATIONS, words(&t.durations)));
        parts.push((at::OCTAVE_OFFSETS, t.octave_offsets.clone()));
        parts.push((at::SEMITONE_OCTAVE, t.semitone_octave.clone()));
        parts.push((at::OCTAVE_START, t.octave_start.clone()));
        parts.push((at::RATES, words(&t.rates)));
        parts.push((at::FINE, t.fine.concat()));
        let levels = t.levels.in_rom_order();
        parts.push((at::LEVEL_TABLES, levels[..7].iter().flat_map(|t| words(t)).collect()));
        parts
    }

    /// Bytes of a sound definition.
    fn definition_bytes(&self, s: &Sound) -> Vec<u8> {
        let offsets = |kind: u8, tracks: &[Option<TrackPos>; 16]| {
            let mut b = vec![kind];
            for t in tracks {
                b.extend(t.map_or(0, |t| (self.addr(t) - s.addr) as u16).to_be_bytes());
            }
            b
        };
        match &s.kind {
            SoundKind::Music { tracks } => offsets(0, tracks),
            SoundKind::Effect { priority, tracks } => offsets(*priority, tracks),
            SoundKind::Note(n) => {
                let mut b = vec![0x80 | n.channel, n.flags];
                b.extend(n.duration.to_be_bytes());
                b.extend(n.gate.to_be_bytes());
                b.push(n.amplitude);
                b.extend(n.instrument.to_be_bytes());
                b.extend([n.note, n.detune]);
                b
            }
            SoundKind::Adpcm(a) => {
                let mut b = vec![a.channel.wrapping_add(0x90), a.flags];
                b.extend(a.unused);
                b.push(a.amplitude);
                b.extend(a.sample.to_be_bytes());
                b
            }
        }
    }

    /// The Z80 ROM region holding the data (without the Z80 program: only the Rust driver can play it).
    pub fn to_region(&self) -> Vec<u8> {
        let mut region = vec![0u8; REGION_SIZE];
        for (addr, bytes) in self.region_parts() {
            for (k, &b) in bytes.iter().enumerate() {
                region[offset(addr + k as u32)] = b;
            }
        }
        region
    }
}

/// Cuts the tracks: follows every instruction reachable from the track starts (the sequencer reads
/// them byte by byte, including jump targets and the instructions after a loop), then groups the bytes
/// read into maximal contiguous runs.
fn cut_tracks(r: &Reader, starts: Vec<u32>) -> Vec<Track> {
    let mut read: BTreeSet<u32> = BTreeSet::new();
    let mut seen: BTreeSet<u32> = BTreeSet::new();
    let mut todo = starts;
    // A track never leaves the region (linear addresses up to REGION_SIZE - 0x8000).
    let end = (REGION_SIZE - 0x8000) as u32;
    while let Some(mut p) = todo.pop() {
        while p < end && seen.insert(p) {
            let op = r.byte(p);
            let len = if op >= 0x20 { 1 } else { 1 + param_len(op) as u32 };
            read.extend((p..p + len).filter(|&a| a < end));
            let target = |offset_at: u32| (offset_at + 2).wrapping_add(r.be(offset_at) as i16 as u32) & 0xff_ffff;
            match op {
                22 => {
                    todo.push(target(p + 1));
                    break;
                }
                23 => break,
                14..=21 => todo.push(target(p + 2)),
                _ => {}
            }
            p += len;
        }
    }
    let mut tracks: Vec<Track> = Vec::new();
    for a in read {
        match tracks.last_mut() {
            Some(t) if t.addr + t.bytes.len() as u32 == a => t.bytes.push(r.byte(a)),
            _ => tracks.push(Track { addr: a, bytes: vec![r.byte(a)] }),
        }
    }
    tracks
}
