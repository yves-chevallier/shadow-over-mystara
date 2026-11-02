//! Music and effects: the sound driver data (`ddsom_sound::SoundData`, read from the Z80 ROM as the
//! driver program reads it) and what follows each sample in the sample ROM.

use std::io;
use std::path::Path;

use ddsom_assets::json;
use ddsom_assets::music::{DspTable, DspTables, Music};
use ddsom_sound::data::SoundKind;
use ddsom_sound::SoundData;

pub fn extract(roms: &Path, path: &Path) -> io::Result<()> {
    let data = SoundData::load(roms)?;
    let defined = data.sounds.iter().flatten().count();
    eprintln!("music: {} sounds ({defined} defined), {} tracks", data.sounds.len(), data.tracks.len());
    if let Some(s) = data.sounds.iter().flatten().find(|s| matches!(&s.kind, SoundKind::Adpcm(a) if a.channel > 2)) {
        return Err(io::Error::new(io::ErrorKind::InvalidData, format!("sound at {:#x}: unknown kind", s.addr)));
    }
    let mut music = Music { data, sample_tails: Vec::new() };
    // sample ROM (ROM_LOAD16_WORD_SWAP): what follows the end of each sample
    let mut pcm = Vec::new();
    for name in ["dd2.11m", "dd2.12m"] {
        pcm.extend(std::fs::read(roms.join(name))?.as_chunks::<2>().0.iter().flat_map(|w| [w[1], w[0]]));
    }
    music.sample_tails = (0..music.sample_count())
        .map(|i| {
            let tail = &pcm[music.sample_span(i).1 + 1..music.sample_tail_end(i)];
            let used = tail.iter().rposition(|&b| b != 0).map_or(0, |p| p + 1);
            tail[..used].iter().map(|b| format!("{b:02x}")).collect()
        })
        .collect();
    std::fs::create_dir_all(path.parent().unwrap())?;
    json::save(path, &music, 3)
}

/// Tables of the QSound DSP program (`dl-1425.bin`, little-endian words) read by the high-level emulation
/// (`qsound::DSP_TABLES`). The rest of the DSP program is code, which the emulation replaces.
pub fn extract_dsp(roms: &Path, path: &Path) -> io::Result<()> {
    let bytes = std::fs::read(roms.join("dl-1425.bin"))?;
    let word = |a: usize| u16::from_le_bytes([bytes[2 * a], bytes[2 * a + 1]]);
    let tables = qsound::DSP_TABLES
        .iter()
        .map(|&(name, at, n)| DspTable { name: name.into(), rom: u32::from(at), values: (usize::from(at)..usize::from(at) + n).map(word).collect() })
        .collect();
    json::save(path, &DspTables { tables }, 2)
}
