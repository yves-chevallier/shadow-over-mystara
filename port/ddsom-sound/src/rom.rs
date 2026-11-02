//! Sound driver data: MAME's "audiocpu" region, read through named accessors.
//!
//! The Z80 sees a fixed 0000-7FFF and a 16 KB window at 8000-BFFF whose bank is selected at
//! D003. The driver handles "linear" pointers: the Z80 address of a byte of bank `b` at window
//! offset `o` is `0x8000 + 0x4000 * b + o`, i.e. the offset in the file `dd2.01` (followed by
//! `dd2.02`). Reading through a linear pointer amounts to what the Z80 does when switching to
//! the next bank each time it reaches C000.
//!
//! The driver reads its data through the accessors below (tables of the fixed ROM, sound header,
//! instruments, sample records, and the bytes of the sound definitions and tracks), never by raw
//! address: each accessor names what it reads (docs/son.md §7, docs/assets.md `sound/music.json`).
//! The data stay at their original addresses because the driver state holds such addresses (track
//! read pointers, instrument banks, sound definitions), as the Z80 RAM did (`Driver::ram_image`), and
//! because the original reads past the end of some tables, which an index out of range must
//! reproduce (see `duration`).

use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Size of MAME's audiocpu region.
const REGION_SIZE: usize = 0x50000;

/// QSound registers written at initialisation (bytes, up to register 0xE3) and their values (words).
const INIT_REGISTERS: u32 = 0x00d5;
const INIT_VALUES: u32 = 0x00e1;
/// Note durations (words), indexed by a byte offset: 2 x the note value, plus the dotted and triplet flags.
const DURATIONS: u32 = 0x1119;
/// Octave offset in semitones, indexed by the low 4 bits of the track flags.
const OCTAVE_OFFSETS: u32 = 0x1149;
/// Octave of each semitone (0-107), then first semitone of each octave.
const SEMITONE_OCTAVE: u32 = 0x1fc6;
const OCTAVE_START: u32 = 0x2032;
/// Base playback rate of each semitone (words), and fine interpolation (12 x 256 bytes).
const RATES: u32 = 0x203b;
const FINE: u32 = 0x2113;
/// Word tables indexed by a byte (docs/son.md §7).
const AMPLITUDES: u32 = 0x33e1;
const LEVELS: u32 = 0x34e1;
const ATTACK: u32 = 0x35e1;
const DECAY: u32 = 0x3661;
const LFO_RATES: u32 = 0x36e1;
const VIBRATO: u32 = 0x37e1;
const TREMOLO: u32 = 0x38e1;
/// Word: address of the sound header.
const HEADER_PTR: u32 = 0x39e1;
/// Word: address of the sample table (8-byte records).
const SAMPLES_PTR: u32 = 0x39e3;
/// Table of instrument bank addresses (words).
const INSTRUMENT_BANKS: u32 = 0x39e5;

/// Header of the sound table.
pub struct SoundHeader {
    /// Number of sounds (the command number is taken modulo this count).
    pub count: u16,
    /// Indices into the `level` table: music volume factor and base volume of the effects.
    pub music_factor: u8,
    pub sfx_volume: u8,
    /// Address of the sound table: 4-byte entries (address of the definition over 3 bytes, 0).
    pub table: u32,
}

/// Instrument record (8 bytes, in a bank of 256).
pub struct Instrument {
    /// Sample number; bit 15: the voice follows the global pitch bend (track instrument) or, for a
    /// single note, the record is an instrument rather than a sample.
    pub sample: u16,
    /// Fine tuning (track) or pitch offset (single note).
    pub detune: u8,
    /// Envelope: indices into the `attack`, `decay`, `level` (sustain), `decay` (slow decay) and
    /// `decay` (release) tables.
    pub envelope: [u8; 5],
}

/// Sample record of the sequencer voices: `[bank, start, loop start, end, reference note]`.
pub struct Sample {
    pub bank: u8,
    pub start: u16,
    pub loop_start: u16,
    pub end: u16,
    /// Note at which the sample plays at its nominal pitch.
    pub root: u8,
}

/// Sample record as the ADPCM voices read it (same table, another layout: bank, one ignored byte, start, end).
pub struct AdpcmSample {
    pub bank: u8,
    pub start: u16,
    pub end: u16,
}

#[derive(Clone)]
pub struct Rom {
    region: Vec<u8>,
    /// Bytes of the region read by the driver (`trace_reads`), if enabled.
    reads: Option<Arc<[AtomicBool]>>,
}

impl Rom {
    /// Loads `dd2.01` and `dd2.02` like MAME: dd2.01 = 0x8000 fixed bytes then 0x18000 bytes
    /// of banks starting at 0x10000; dd2.02 starting at 0x28000.
    pub fn load(dir: &Path) -> io::Result<Self> {
        let mut region = vec![0u8; REGION_SIZE];
        let d1 = std::fs::read(dir.join("dd2.01"))?;
        region[..0x8000].copy_from_slice(&d1[..0x8000]);
        region[0x10000..0x28000].copy_from_slice(&d1[0x8000..0x20000]);
        let d2 = std::fs::read(dir.join("dd2.02"))?;
        region[0x28000..0x48000].copy_from_slice(&d2);
        Ok(Rom::from_region(region))
    }

    /// Builds the ROM from an already assembled audiocpu region.
    pub fn from_region(region: Vec<u8>) -> Self {
        Rom { region, reads: None }
    }

    /// Complete audiocpu region (test tools: reference Z80).
    pub fn region(&self) -> &[u8] {
        &self.region
    }

    /// Records from now on which bytes of the region the driver reads (measure: `read_bytes`).
    pub fn trace_reads(&mut self) {
        self.reads = Some((0..self.region.len()).map(|_| AtomicBool::new(false)).collect());
    }

    /// Region offsets read since `trace_reads` (empty if it was not called).
    pub fn read_bytes(&self) -> Vec<usize> {
        let Some(r) = &self.reads else { return Vec::new() };
        r.iter().enumerate().filter(|(_, b)| b.load(Ordering::Relaxed)).map(|(i, _)| i).collect()
    }

    /// Byte at linear address `p` (0xFF outside the ROM, like the Z80's open bus): the bytes of the sound
    /// definitions and of the tracks, which the voices read through their read pointer.
    pub fn byte(&self, p: u32) -> u8 {
        let p = p & 0xff_ffff;
        let i = if p < 0x8000 { p } else { p + 0x8000 } as usize;
        if let Some(r) = &self.reads {
            if let Some(b) = r.get(i) {
                b.store(true, Ordering::Relaxed);
            }
        }
        self.region.get(i).copied().unwrap_or(0xff)
    }

    /// Little-endian word at linear address `p` (raw access: extraction and test tools; the driver reads its
    /// tables through the named accessors).
    pub fn word(&self, p: u32) -> u16 {
        u16::from(self.byte(p)) | u16::from(self.byte(p + 1)) << 8
    }

    /// Big-endian word at linear address `p` (offsets and durations of the sound definitions).
    pub fn word_be(&self, p: u32) -> u16 {
        self.word(p).swap_bytes()
    }

    /// Entry `i` of a table of little-endian words located at `base`.
    fn table(&self, base: u32, i: u8) -> u16 {
        self.word(base + 2 * u32::from(i))
    }

    // ------------------------------------------------------------------ fixed ROM tables

    /// QSound registers written at initialisation and their values (filters and echo), up to register 0xE3.
    pub fn init_registers(&self) -> Vec<(u8, u16)> {
        let mut regs = Vec::new();
        for i in 0.. {
            let reg = self.byte(INIT_REGISTERS + i);
            regs.push((reg, self.word(INIT_VALUES + 2 * i)));
            if reg == 0xe3 {
                return regs;
            }
        }
        unreachable!()
    }

    /// Voice amplitude (instruction 7, effect commands) and echo send (instruction 25).
    pub fn amplitude(&self, i: u8) -> u16 {
        self.table(AMPLITUDES, i)
    }

    /// General levels: base volumes, ramp targets, music volume (instruction 26), envelope sustain.
    pub fn level(&self, i: u8) -> u16 {
        self.table(LEVELS, i)
    }

    /// Envelope attack rate.
    pub fn attack(&self, i: u8) -> u16 {
        self.table(ATTACK, i)
    }

    /// Envelope decay rates (decay, slow decay, release).
    pub fn decay(&self, i: u8) -> u16 {
        self.table(DECAY, i)
    }

    /// LFO rate (instruction 29).
    pub fn lfo_rate(&self, i: u8) -> u16 {
        self.table(LFO_RATES, i)
    }

    /// Vibrato depth (instruction 27).
    pub fn vibrato(&self, i: u8) -> u16 {
        self.table(VIBRATO, i)
    }

    /// Tremolo depth (instruction 28).
    pub fn tremolo(&self, i: u8) -> u16 {
        self.table(TREMOLO, i)
    }

    /// Note duration at byte offset `offset` (2 x the note value 1-7, plus 0x10 dotted, 0x20 triplet). With both
    /// flags the offset passes the 24 words of the table and the original reads the octave offsets that follow it.
    pub fn duration(&self, offset: u8) -> u16 {
        self.word(DURATIONS + u32::from(offset))
    }

    /// Octave offset in semitones (low 4 bits of the track flags).
    pub fn octave_offset(&self, i: u8) -> u8 {
        self.byte(OCTAVE_OFFSETS + u32::from(i))
    }

    /// Octave of semitone `n` (0-107).
    pub fn semitone_octave(&self, n: u8) -> u8 {
        self.byte(SEMITONE_OCTAVE + u32::from(n))
    }

    /// First semitone of octave `octave`.
    pub fn octave_start(&self, octave: u8) -> u8 {
        self.byte(OCTAVE_START + u32::from(octave))
    }

    /// Base playback rate of semitone `n` (0-107).
    pub fn rate(&self, n: u8) -> u16 {
        self.table(RATES, n)
    }

    /// Fine interpolation of the rate: semitone in the octave (0-11), fraction of semitone (0-255).
    pub fn fine(&self, semitone: u8, fraction: u8) -> u8 {
        self.byte(FINE + (u32::from(semitone) << 8 | u32::from(fraction)))
    }

    // ------------------------------------------------------------------ sounds, instruments, samples

    /// Header of the sound table.
    pub fn sound_header(&self) -> SoundHeader {
        let header = u32::from(self.word(HEADER_PTR));
        SoundHeader {
            count: u16::from(self.byte(header)) << 8 | u16::from(self.byte(header + 1)),
            music_factor: self.byte(header + 2),
            sfx_volume: self.byte(header + 3),
            table: header + 6,
        }
    }

    /// Address of the definition of the sound whose 4-byte entry is at `entry` (0: empty entry).
    pub fn sound(&self, entry: u32) -> u32 {
        u32::from(self.byte(entry)) << 16 | u32::from(self.word_be(entry + 1))
    }

    /// Address of instrument bank `i` (256 records of 8 bytes).
    pub fn instrument_bank(&self, i: u8) -> u16 {
        self.table(INSTRUMENT_BANKS, i)
    }

    /// Record `i` of the instrument bank at address `bank`.
    pub fn instrument(&self, bank: u16, i: u8) -> Instrument {
        let p = u32::from(bank) + u32::from(i) * 8;
        Instrument {
            sample: self.word(p),
            detune: self.byte(p + 2),
            envelope: std::array::from_fn(|k| self.byte(p + 3 + k as u32)),
        }
    }

    /// Address of the record of sample `id` (the Z80 computes `id * 8` on 16 bits).
    fn sample_record(&self, id: u16) -> u32 {
        u32::from(self.word(SAMPLES_PTR)) + ((u32::from(id) * 8) & 0xffff)
    }

    /// Sample `id` as the sequencer voices read it.
    pub fn sample(&self, id: u16) -> Sample {
        let p = self.sample_record(id);
        Sample {
            bank: self.byte(p),
            start: self.word(p + 1),
            loop_start: self.word(p + 3),
            end: self.word(p + 5),
            root: self.byte(p + 7),
        }
    }

    /// Sample `id` as the ADPCM voices read it.
    pub fn adpcm_sample(&self, id: u16) -> AdpcmSample {
        let p = self.sample_record(id);
        AdpcmSample { bank: self.byte(p), start: self.word(p + 2), end: self.word(p + 4) }
    }
}
