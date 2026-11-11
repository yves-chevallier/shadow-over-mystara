//! The renderer alone, on video states dumped by `ddsom-game/examples/render_check` (`DDSOM_DUMP_VIDEO`).
//!
//! render_bench <roms> <dump dir>... [--repeat n]
//!
//! Renders every dumped frame, checks the hash of its pixels against the one recorded in `index.txt` and prints the
//! rendering speed. A quick loop for optimising `cps2-gfx` without rebuilding the game.

use std::hash::Hasher;
use std::path::Path;

use cps2_gfx::{render, rom, DrawList, VideoState};

/// FNV-1a 64 bits, as `render_check`.
struct Fnv(u64);
impl Hasher for Fnv {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0 ^ b as u64).wrapping_mul(0x100000001b3);
        }
    }
}

/// A dumped frame: GFX RAM, CPS-A, CPS-B, object table, output registers, palette; expected hash.
struct Frame {
    words: Vec<u16>,
    hash: u64,
}

const GFXRAM: usize = 0x18000;
const OBJRAM: usize = 0x1000;

impl Frame {
    fn state(&self) -> VideoState<'_> {
        let w = &self.words;
        let (gfxram, rest) = w.split_at(GFXRAM);
        let (cps_a, rest) = rest.split_at(32);
        let (cps_b, rest) = rest.split_at(32);
        let (objram, rest) = rest.split_at(OBJRAM);
        let (output, palette) = rest.split_at(6);
        VideoState {
            gfxram,
            cps_a: cps_a.try_into().unwrap(),
            cps_b: cps_b.try_into().unwrap(),
            objram,
            output: output.try_into().unwrap(),
            palette_ram: Some(palette),
        }
    }
}

fn main() {
    let mut a: Vec<String> = std::env::args().skip(1).collect();
    let repeat = match a.iter().position(|x| x == "--repeat") {
        Some(i) => {
            let n = a[i + 1].parse().expect("repeat");
            a.drain(i..i + 2);
            n
        }
        None => 1,
    };
    if a.len() < 2 {
        eprintln!("usage: render_bench <roms> <dump dir>... [--repeat n]");
        std::process::exit(2);
    }
    let atlas = rom::load_atlas(Path::new(&a[0])).expect("graphics roms");
    let mut frames = Vec::new();
    for dir in &a[1..] {
        let index = std::fs::read_to_string(Path::new(dir).join("index.txt")).expect("index.txt");
        for line in index.lines() {
            let (name, hash) = line.split_once(' ').expect("index line");
            let bytes = std::fs::read(Path::new(dir).join(name)).expect("dump");
            let words = bytes.as_chunks::<2>().0.iter().map(|&w| u16::from_le_bytes(w)).collect();
            frames.push(Frame { words, hash: u64::from_str_radix(hash, 16).expect("hash") });
        }
    }
    let mut bad = 0;
    let mut rendering = std::time::Duration::ZERO;
    for _ in 0..repeat {
        for f in &frames {
            let t0 = std::time::Instant::now();
            let img = render(&DrawList::from_video(&f.state()), &atlas);
            rendering += t0.elapsed();
            let mut h = Fnv(0xcbf29ce484222325);
            for p in &img {
                h.write(p);
            }
            bad += (h.finish() != f.hash) as usize;
        }
    }
    let n = frames.len() * repeat;
    let dt = rendering;
    println!(
        "{n} frames, {bad} differ, {:.0} frames/s ({:.1} µs per frame)",
        n as f64 / dt.as_secs_f64(),
        dt.as_secs_f64() * 1e6 / n as f64
    );
    if bad > 0 {
        std::process::exit(1);
    }
}
