//! Sounds: the QSound samples described by the Z80 driver table, as 8-bit WAV.

use std::io::{self, Write};
use std::path::Path;

/// Sound driver sample table (Z80 ROM, dd2.01).
const TABLE: usize = 0x49e8;
/// Number of entries: the table ends after the last non-null entry (343: `0xa2000`; 344: null entry).
/// Three entries (290, 293 and 305) are invalid (start after end): no file.
const COUNT: usize = 345;
/// QSound output frequency: playback rate 1.0.
const RATE: u32 = 24_038;

/// Table entry: reference note, bank, start, loop start, end (addresses within the bank).
struct Sample {
    root: u8,
    bank: u8,
    start: u16,
    loop_start: u16,
    end: u16,
}

fn table(z80: &[u8]) -> Vec<Sample> {
    z80[TABLE..]
        .chunks_exact(8)
        .map(|e| Sample {
            root: e[0],
            bank: e[1],
            start: u16::from_le_bytes([e[2], e[3]]),
            loop_start: u16::from_le_bytes([e[4], e[5]]),
            end: u16::from_le_bytes([e[6], e[7]]),
        })
        .take(COUNT)
        .collect()
}

pub fn extract(roms: &Path, dir: &Path) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let z80 = std::fs::read(roms.join("dd2.01"))?;
    // sample ROM: ROM_LOAD16_WORD_SWAP
    let mut pcm = Vec::new();
    for name in ["dd2.11m", "dd2.12m"] {
        pcm.extend(std::fs::read(roms.join(name))?.chunks_exact(2).flat_map(|w| [w[1], w[0]]));
    }
    for (i, s) in table(&z80).iter().enumerate() {
        if !(s.bank < 0x40 && s.start <= s.end && (s.start..=s.end).contains(&s.loop_start)) {
            continue;
        }
        let base = (s.bank as usize) << 16;
        let data = &pcm[base + s.start as usize..=base + s.end as usize];
        let looped = s.loop_start < s.end;
        write_wav(&dir.join(format!("{i:03}.wav")), data, s.root, looped.then_some((s.loop_start - s.start) as u32))?;
    }
    Ok(())
}

/// 8-bit mono PCM WAV (unsigned, hence the ^ 0x80) with a `smpl` chunk (note, loop).
fn write_wav(path: &Path, signed: &[u8], root: u8, loop_start: Option<u32>) -> io::Result<()> {
    let data: Vec<u8> = signed.iter().map(|b| b ^ 0x80).collect();
    let mut smpl = Vec::new();
    let le = |v: u32| v.to_le_bytes();
    smpl.extend(le(0)); // manufacturer
    smpl.extend(le(0)); // product
    smpl.extend(le(1_000_000_000 / RATE)); // sample period (ns)
    smpl.extend(le(root as u32)); // MIDI reference note
    smpl.extend([le(0), le(0), le(0)].concat()); // detune, SMPTE format, offset
    smpl.extend(le(loop_start.is_some() as u32));
    smpl.extend(le(0));
    if let Some(start) = loop_start {
        smpl.extend([le(0), le(0), le(start), le(data.len() as u32 - 1), le(0), le(0)].concat());
    }

    let mut f = io::BufWriter::new(std::fs::File::create(path)?);
    let chunk = |f: &mut io::BufWriter<_>, id: &[u8], body: &[u8]| -> io::Result<()> {
        f.write_all(id)?;
        f.write_all(&(body.len() as u32).to_le_bytes())?;
        f.write_all(body)?;
        if body.len() % 2 == 1 {
            f.write_all(&[0])?;
        }
        Ok(())
    };
    let fmt = [&1u16.to_le_bytes()[..], &1u16.to_le_bytes(), &le(RATE), &le(RATE), &1u16.to_le_bytes(), &8u16.to_le_bytes()].concat();
    let pad = data.len() % 2;
    let size = 4 + (8 + fmt.len()) + (8 + data.len() + pad) + (8 + smpl.len());
    f.write_all(b"RIFF")?;
    f.write_all(&(size as u32).to_le_bytes())?;
    f.write_all(b"WAVE")?;
    chunk(&mut f, b"fmt ", &fmt)?;
    chunk(&mut f, b"data", &data)?;
    chunk(&mut f, b"smpl", &smpl)?;
    f.flush()
}
