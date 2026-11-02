//! The QSound sample bank, built from the WAV files: the memory the voice registers address (bank and address
//! written by the sound driver). It is not a ROM image: only the samples and their tails are placed in it.

use std::io;
use std::path::Path;

use crate::rebuild::Image;
use crate::{Music, Sample};

/// Size of the QSound sample memory (the ROMs `dd2.11m` and `dd2.12m`).
pub const SAMPLES_SIZE: usize = 0x40_0000;

/// The sample bank: the PCM of `sounds/NNN.wav` at the addresses of the driver's sample records (`music.json`),
/// each followed by what follows it in the original memory (`Music::sample_tails`, zeros omitted), the rest zero.
/// `Image::covered` gives the filled zones.
pub fn bank(dir: &Path) -> io::Result<Image> {
    let music = Music::load(&dir.join("sound/music.json"))?;
    let mut img = Image::new(SAMPLES_SIZE);
    for i in 0..music.sample_count() {
        let path = dir.join(format!("sounds/{i:03}.wav"));
        if !path.exists() || !music.sample_valid(i) {
            continue;
        }
        let pcm: Vec<u8> = Sample::load(&path)?.pcm.iter().map(|&v| v as u8).collect();
        let (start, end) = music.sample_span(i);
        img.put(start as u32, &pcm);
        // what follows up to the next sample (zeros omitted in the file)
        let mut tail = vec![0; music.sample_tail_end(i) - end - 1];
        if let Some(hex) = music.sample_tails.get(i) {
            for (k, b) in tail.iter_mut().zip(hex.as_bytes().chunks(2)) {
                *k = u8::from_str_radix(std::str::from_utf8(b).unwrap_or("00"), 16).unwrap_or(0);
            }
        }
        img.put(end as u32 + 1, &tail);
    }
    Ok(img)
}
