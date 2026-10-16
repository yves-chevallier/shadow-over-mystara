//! Sons : les échantillons QSound décrits par la table du pilote Z80, en WAV 8 bits.

use std::io::{self, Write};
use std::path::Path;

/// Table des échantillons du pilote son (ROM Z80, dd2.01).
const TABLE: usize = 0x49e8;
/// Fréquence de sortie de la QSound : vitesse de lecture 1.0.
const RATE: u32 = 24_038;

/// Entrée de la table : note de référence, banque, début, début de boucle, fin (adresses dans la banque).
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
        .take_while(|s| s.bank < 0x40 && s.start <= s.end && (s.start..=s.end).contains(&s.loop_start))
        .collect()
}

pub fn extract(roms: &Path, dir: &Path) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let z80 = std::fs::read(roms.join("dd2.01"))?;
    // ROM d'échantillons : ROM_LOAD16_WORD_SWAP
    let mut pcm = Vec::new();
    for name in ["dd2.11m", "dd2.12m"] {
        pcm.extend(std::fs::read(roms.join(name))?.chunks_exact(2).flat_map(|w| [w[1], w[0]]));
    }
    for (i, s) in table(&z80).iter().enumerate() {
        let base = (s.bank as usize) << 16;
        let data = &pcm[base + s.start as usize..=base + s.end as usize];
        let looped = s.loop_start < s.end;
        write_wav(&dir.join(format!("{i:03}.wav")), data, s.root, looped.then_some((s.loop_start - s.start) as u32))?;
    }
    Ok(())
}

/// WAV PCM 8 bits mono (non signé, d'où le ^ 0x80) avec un bloc `smpl` (note, boucle).
fn write_wav(path: &Path, signed: &[u8], root: u8, loop_start: Option<u32>) -> io::Result<()> {
    let data: Vec<u8> = signed.iter().map(|b| b ^ 0x80).collect();
    let mut smpl = Vec::new();
    let le = |v: u32| v.to_le_bytes();
    smpl.extend(le(0)); // fabricant
    smpl.extend(le(0)); // produit
    smpl.extend(le(1_000_000_000 / RATE)); // période d'échantillonnage (ns)
    smpl.extend(le(root as u32)); // note de référence MIDI
    smpl.extend([le(0), le(0), le(0)].concat()); // désaccord, format SMPTE, décalage
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
