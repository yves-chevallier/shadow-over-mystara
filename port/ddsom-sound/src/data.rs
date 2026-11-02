//! Sound driver data as indexed tables: sounds, tracks, instrument banks, sample records and the
//! driver's tables (docs/son.md §7, `sound/music.json`).
//!
//! The driver state holds indices into these tables (track positions, bank and sound numbers),
//! never Z80 addresses. Each table keeps the Z80 address it comes from (`addr`) only for the tools:
//! `Driver::ram_image` projects the driver state onto the Z80 RAM, and [`crate::region`] converts
//! between these tables and the Z80 ROM region.

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[cfg(feature = "serde")]
use crate::serde_formats::{hex, tokens};

/// Everything the driver reads.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct SoundData {
    pub header: Header,
    /// One entry per sound number (`None`: empty entry). The number of a command is taken modulo
    /// their count.
    pub sounds: Vec<Option<Sound>>,
    pub tracks: Vec<Track>,
    pub banks: Vec<Bank>,
    pub samples: Samples,
    pub tables: Tables,
}

/// Header of the sound table.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Header {
    /// Z80 address of the header; the sound table follows it at `addr + 6` (4 bytes per sound:
    /// big-endian 3-byte address of the definition, 0).
    #[cfg_attr(feature = "serde", serde(with = "hex"))]
    pub addr: u32,
    /// Indices into the level table: music volume factor and base volume of the effects.
    pub music_factor: u8,
    pub sfx_volume: u8,
    /// Two bytes the driver never reads.
    pub unused: [u8; 2],
}

/// Definition of a sound.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Sound {
    #[cfg_attr(feature = "serde", serde(with = "hex"))]
    pub addr: u32,
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub kind: SoundKind,
}

/// The four kinds of sound, told apart by the first byte of the definition.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize), serde(rename_all = "snake_case"))]
pub enum SoundKind {
    /// Song (first byte 0): one track per channel.
    Music { tracks: [Option<TrackPos>; 16] },
    /// Multi-track effect (first byte 1-0x7F, its priority): a track only starts on a channel whose
    /// effect has a priority no higher.
    Effect { priority: u8, tracks: [Option<TrackPos>; 16] },
    /// Single note of a sample or an instrument (first byte 0x80 + channel).
    Note(Note),
    /// Sample played on an ADPCM channel (first byte 0x90 + channel).
    Adpcm(AdpcmNote),
}

/// Single note: the 10 bytes following the first byte of the definition.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Note {
    /// Channel 0-15.
    pub channel: u8,
    /// Bits 0-6: priority; bit 7: played until explicitly stopped.
    pub flags: u8,
    /// Duration and hold time, in ticks.
    pub duration: u16,
    pub gate: u16,
    /// Index into the amplitude table.
    pub amplitude: u8,
    /// Sample number, or with bit 15 an instrument: bank in bits 8-14, record in bits 0-7.
    pub instrument: u16,
    /// Note (semitone - 1) and fine tuning.
    pub note: u8,
    pub detune: u8,
}

/// ADPCM sample: the 8 bytes following the first byte of the definition.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct AdpcmNote {
    /// ADPCM channel; the driver ignores the definitions of a channel above 2.
    pub channel: u8,
    /// Bits 0-6: priority; bit 7: replayed until explicitly stopped.
    pub flags: u8,
    /// Four bytes the driver never reads.
    pub unused: [u8; 4],
    /// Index into the amplitude table.
    pub amplitude: u8,
    pub sample: u16,
}

/// Position of a track reader: track number and byte offset in the track.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize), serde(from = "(u16, u16)", into = "(u16, u16)"))]
pub struct TrackPos {
    pub track: u16,
    pub pos: u16,
}

impl From<(u16, u16)> for TrackPos {
    fn from((track, pos): (u16, u16)) -> Self {
        TrackPos { track, pos }
    }
}

impl From<TrackPos> for (u16, u16) {
    fn from(p: TrackPos) -> Self {
        (p.track, p.pos)
    }
}

/// Sequencer bytes (instructions and notes, docs/son.md §6) contiguous in the Z80 ROM. A relative
/// jump may land in another track: tracks are cut where the bytes the driver reads stop being
/// contiguous, not where a song's channel starts.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Track {
    #[cfg_attr(feature = "serde", serde(with = "hex"))]
    pub addr: u32,
    /// Written as instruction tokens in `music.json` (`crate::tokens`).
    #[cfg_attr(feature = "serde", serde(rename = "ops", with = "tokens"))]
    pub bytes: Vec<u8>,
}

/// Instrument bank: 256 records.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Bank {
    #[cfg_attr(feature = "serde", serde(with = "hex"))]
    pub addr: u32,
    pub instruments: Vec<Instrument>,
}

/// Instrument record (8 bytes).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Instrument {
    /// Sample number; bit 15: the voice follows the global pitch bend (track instrument) or, for a
    /// single note, the record is an instrument rather than a sample.
    pub sample: u16,
    /// Fine tuning (track) or pitch offset (single note).
    pub detune: u8,
    /// Envelope: indices into the `attack`, `decay`, `levels` (sustain), `decay` (slow decay) and
    /// `decay` (release) tables.
    pub envelope: [u8; 5],
}

/// Sample records: 8 bytes each, read with two layouts (`SampleRecord::voice`, `SampleRecord::adpcm`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Samples {
    #[cfg_attr(feature = "serde", serde(with = "hex"))]
    pub addr: u32,
    pub records: Vec<SampleRecord>,
    /// What the original reads as the record of a sample number past the table, for the numbers the data
    /// reference (sorted): the placeholder instruments 175-255 of bank 1 (samples 431-511), and
    /// `NO_INSTRUMENT`, which a track voice that has not chosen an instrument yet loads when an effect ends on
    /// its channel.
    pub past_end: Vec<(u16, SampleRecord)>,
}

/// Instrument of a track voice until its first `inst` instruction.
pub const NO_INSTRUMENT: u16 = 0xff00;

/// Raw sample record: `[bank, start (LE), loop start (LE), end (LE), reference note]` for the
/// sequencer voices; the ADPCM voices read bank, start and end one byte further.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct SampleRecord(pub [u8; 8]);

/// Sample as the sequencer voices read it.
pub struct Sample {
    pub bank: u8,
    pub start: u16,
    pub loop_start: u16,
    pub end: u16,
    /// Note at which the sample plays at its nominal pitch.
    pub root: u8,
}

/// Sample as the ADPCM voices read it.
pub struct AdpcmSample {
    pub bank: u8,
    pub start: u16,
    pub end: u16,
}

impl SampleRecord {
    fn word(&self, at: usize) -> u16 {
        u16::from_le_bytes([self.0[at], self.0[at + 1]])
    }

    pub fn voice(&self) -> Sample {
        Sample { bank: self.0[0], start: self.word(1), loop_start: self.word(3), end: self.word(5), root: self.0[7] }
    }

    pub fn adpcm(&self) -> AdpcmSample {
        AdpcmSample { bank: self.0[0], start: self.word(2), end: self.word(4) }
    }
}

/// Tables of the driver program (fixed ROM).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Tables {
    /// QSound registers written at initialisation and their values (filters and echo).
    pub qsound_init: Vec<(u8, u16)>,
    /// Note durations: 2 x the note value (1-7), plus 0x10 dotted, 0x20 triplet, as a byte offset.
    pub durations: Vec<u16>,
    /// Octave offset in semitones, indexed by the low 4 bits of the track flags.
    pub octave_offsets: Vec<u8>,
    /// Octave of each semitone (0-107), first semitone of each octave.
    pub semitone_octave: Vec<u8>,
    pub octave_start: Vec<u8>,
    /// Base playback rate of each semitone, and its fine interpolation (12 semitones x 256 fractions).
    pub rates: Vec<u16>,
    pub fine: Vec<Vec<u8>>,
    pub levels: LevelTables,
}

/// The word tables indexed by a byte read from a track, an instrument or a command. They follow each
/// other in the Z80 ROM, in this order, and the original reads the next table when an index passes
/// the end of one (only synthetic data do: the game's indices stay within bounds).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct LevelTables {
    /// Voice amplitude (instruction 7, single notes), echo send (instruction 25).
    pub amplitudes: Vec<u16>,
    /// Base volumes, ramp targets, music volume (instruction 26), envelope sustain.
    pub levels: Vec<u16>,
    /// Envelope attack rates, then decay rates (decay, slow decay, release).
    pub attack: Vec<u16>,
    pub decay: Vec<u16>,
    /// LFO rate (instruction 29), vibrato and tremolo depths (instructions 27 and 28).
    pub lfo_rates: Vec<u16>,
    pub vibrato: Vec<u16>,
    pub tremolo: Vec<u16>,
    /// The 128 words after the tremolo table (data pointers, then instrument bank 0), which an index
    /// past the tremolo table reads.
    pub after_tremolo: Vec<u16>,
}

impl LevelTables {
    /// The tables in ROM order.
    pub(crate) fn in_rom_order(&self) -> [&[u16]; 8] {
        [
            &self.amplitudes,
            &self.levels,
            &self.attack,
            &self.decay,
            &self.lfo_rates,
            &self.vibrato,
            &self.tremolo,
            &self.after_tremolo,
        ]
    }

    /// Entry `i` of the table `first` (position in ROM order), reading on into the following tables
    /// like the original.
    fn read_on(&self, first: usize, i: u8) -> u16 {
        let mut i = usize::from(i);
        for table in &self.in_rom_order()[first..] {
            match table.get(i) {
                Some(&v) => return v,
                None => i -= table.len(),
            }
        }
        unreachable!("a byte index cannot pass the tremolo table and the 128 words after it")
    }
}

impl SoundData {
    /// Voice amplitude (instruction 7, single notes) and echo send (instruction 25).
    pub fn amplitude(&self, i: u8) -> u16 {
        self.tables.levels.read_on(0, i)
    }

    /// General levels: base volumes, ramp targets, music volume (instruction 26), envelope sustain.
    pub fn level(&self, i: u8) -> u16 {
        self.tables.levels.read_on(1, i)
    }

    pub fn attack(&self, i: u8) -> u16 {
        self.tables.levels.read_on(2, i)
    }

    pub fn decay(&self, i: u8) -> u16 {
        self.tables.levels.read_on(3, i)
    }

    pub fn lfo_rate(&self, i: u8) -> u16 {
        self.tables.levels.read_on(4, i)
    }

    pub fn vibrato(&self, i: u8) -> u16 {
        self.tables.levels.read_on(5, i)
    }

    pub fn tremolo(&self, i: u8) -> u16 {
        self.tables.levels.read_on(6, i)
    }

    /// Note duration at byte offset `offset` (always even). With both the dotted and the triplet flags
    /// the offset passes the 24 words of the table, and the original reads the octave offsets that
    /// follow it, two bytes as a little-endian word.
    pub fn duration(&self, offset: u8) -> u16 {
        let i = usize::from(offset / 2);
        let durations = &self.tables.durations;
        match durations.get(i) {
            Some(&d) => d,
            None => {
                let at = 2 * (i - durations.len());
                let octaves = &self.tables.octave_offsets;
                u16::from_le_bytes([octaves[at], octaves[at + 1]])
            }
        }
    }

    /// Octave offset in semitones (low 4 bits of the track flags).
    pub fn octave_offset(&self, i: u8) -> u8 {
        self.tables.octave_offsets[usize::from(i)]
    }

    /// QSound playback rate of semitone `n` (0-107) with the fraction `fraction` of a semitone.
    pub fn pitch_rate(&self, n: u8, fraction: u8) -> u16 {
        let t = &self.tables;
        let octave = t.semitone_octave[usize::from(n)];
        let semitone = n.wrapping_sub(t.octave_start[usize::from(octave)]);
        let fine = u16::from(t.fine[usize::from(semitone)][usize::from(fraction)]) << octave;
        t.rates[usize::from(n)].wrapping_add(fine)
    }

    /// Instrument bank `i`; the game only selects existing banks.
    pub fn bank(&self, i: u8) -> &Bank {
        self.banks.get(usize::from(i)).unwrap_or_else(|| panic!("instrument bank {i} does not exist"))
    }

    /// Sample record `id` (the Z80 computes `id * 8` on 16 bits, hence `id & 0x1fff`).
    pub fn sample(&self, id: u16) -> &SampleRecord {
        let i = id & 0x1fff;
        if let Some(record) = self.samples.records.get(usize::from(i)) {
            return record;
        }
        let past_end = &self.samples.past_end;
        match past_end.binary_search_by_key(&i, |&(id, _)| id) {
            Ok(k) => &past_end[k].1,
            Err(_) => panic!("sample {i:#x}: past the table and referenced by no data"),
        }
    }

    /// Byte `pos` of a track. The bytes the driver reads are contiguous, so a reader never leaves its
    /// track, except at the very end of the Z80 ROM region, where the bus reads 0xFF.
    pub fn track_byte(&self, at: TrackPos) -> u8 {
        self.tracks[usize::from(at.track)].bytes.get(usize::from(at.pos)).copied().unwrap_or(0xff)
    }

    /// Position reached by a relative jump of `offset` bytes from `from`: in the same track, or in the
    /// track that holds the target (all the targets were found when the tracks were cut).
    pub fn jump(&self, from: TrackPos, offset: i16) -> TrackPos {
        let track = &self.tracks[usize::from(from.track)];
        let pos = i32::from(from.pos) + i32::from(offset);
        if (0..track.bytes.len() as i32).contains(&pos) {
            return TrackPos { track: from.track, pos: pos as u16 };
        }
        let addr = (track.addr as i32 + pos) as u32 & 0xff_ffff;
        // Only a jump past the end of the Z80 ROM region (synthetic data) lands outside the tracks: the bus then
        // reads 0xFF, like past the end of a track.
        self.locate(addr).unwrap_or(TrackPos { track: from.track, pos: u16::MAX })
    }

    /// Track position of the Z80 address `addr`, if a track holds it.
    pub fn locate(&self, addr: u32) -> Option<TrackPos> {
        locate(&self.tracks, addr)
    }

    /// Z80 address of a track position (tools).
    pub fn addr(&self, at: TrackPos) -> u32 {
        self.tracks[usize::from(at.track)].addr + u32::from(at.pos)
    }
}

/// Position of the Z80 address `addr` in the tracks (sorted by address), if one holds it.
pub(crate) fn locate(tracks: &[Track], addr: u32) -> Option<TrackPos> {
    let i = tracks.partition_point(|t| t.addr <= addr).checked_sub(1)?;
    let pos = addr - tracks[i].addr;
    (pos < tracks[i].bytes.len() as u32).then_some(TrackPos { track: i as u16, pos: pos as u16 })
}
