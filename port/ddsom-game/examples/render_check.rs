//! Bit-for-bit check of the rendering and the audio (optimisation phase, batch F).
//!
//! render_check <roms> <eeprom> <scenario> <frames> [output]
//!
//! Runs the game (without 68000, without ROM, Rust sound driver) with rendering and audio and prints one line per frame: hash of the pixels, hash of the audio samples produced. With `exit`, writes
//! these lines to a file; otherwise only the summary line is printed. Compare two outputs with `cmp`/`diff`.
//! `DDSOM_ASSETS=<dir>`: game loaded from the assets only (tile atlas from `gfx/cells_NN.png`, samples from the WAVs).
//! `DDSOM_DUMP_VIDEO=<dir>` (and `DDSOM_DUMP_EVERY=<n>`, default 10): writes the video state of every n-th frame and the
//! hash of its pixels, for `cps2-gfx/examples/render_bench` (renderer alone, without the game).

use std::hash::Hasher;
use std::io::Write;
use std::path::Path;

/// FNV-1a 64 bits: simple, stable, dependency-free.
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

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 5 {
        eprintln!("usage: render_check <roms> <eeprom> <scenario> <frames> [output]");
        std::process::exit(2);
    }
    let ee = std::fs::read(&a[2]).ok();
    // the game machine (no ROM) and its data, as in `ddsom`
    let contents = match std::env::var_os("DDSOM_ASSETS") {
        Some(dir) => extract::install_game_assets(Path::new(&dir)).expect("assets"),
        None => extract::install_game(Path::new(&a[1])).expect("roms"),
    };
    let mut m = cps2::Machine::game(contents, ee.as_deref());
    let dump = std::env::var_os("DDSOM_DUMP_VIDEO").map(std::path::PathBuf::from);
    let every: usize = std::env::var("DDSOM_DUMP_EVERY").ok().and_then(|n| n.parse().ok()).unwrap_or(10);
    let mut index = dump.as_ref().map(|d| {
        std::fs::create_dir_all(d).unwrap();
        std::io::BufWriter::new(std::fs::File::create(d.join("index.txt")).unwrap())
    });
    ddsom_game::install_no_68k(&mut m);
    m.rendering = true;
    let script = ddsom_game::scenario::Scenario::load(Path::new(&a[3])).expect("scenario");
    let frames: usize = a[4].parse().unwrap();
    let mut out: Option<std::io::BufWriter<std::fs::File>> = a.get(5).map(|p| std::io::BufWriter::new(std::fs::File::create(p).unwrap()));
    let mut total = Fnv(0xcbf29ce484222325);
    let t0 = std::time::Instant::now();
    for f in 0..frames {
        script.apply(f, &mut m);
        // the frame is rendered with the object table latched at the previous vblank
        let latched = (m.obj_buffer.clone(), m.pri_ctrl_buffer);
        let img = m.run_frame();
        let mut hi = Fnv(0xcbf29ce484222325);
        for p in &img {
            hi.write(p);
        }
        let mut ha = Fnv(0xcbf29ce484222325);
        for s in m.sound.audio.drain(..) {
            ha.write(&s[0].to_le_bytes());
            ha.write(&s[1].to_le_bytes());
        }
        if let (Some(dir), Some(index)) = (dump.as_ref(), index.as_mut()) {
            if f % every == 0 {
                dump_video(&dir.join(format!("{f:05}.bin")), &m, &latched.0, &latched.1);
                writeln!(index, "{f:05}.bin {:016x}", hi.finish()).unwrap();
            }
        }
        if let Some(o) = out.as_mut() {
            writeln!(o, "{f} {:016x} {:016x}", hi.finish(), ha.finish()).unwrap();
        }
        total.write(&hi.finish().to_le_bytes());
        total.write(&ha.finish().to_le_bytes());
    }
    println!("{} frames, summary {:016x}, {:.0} frames/s", frames, total.finish(), frames as f64 / t0.elapsed().as_secs_f64());
}

/// Video state of the frame just rendered, as little-endian words: GFX RAM, CPS-A, CPS-B, object table, object
/// output registers, palette (the layout `cps2-gfx/examples/render_bench` reads).
fn dump_video(path: &Path, m: &cps2::Machine, objram: &[u16], output: &[u16; 6]) {
    let b = &m.bus;
    let parts: [&[u16]; 6] = [&b.gfxram, &b.cps_a, &b.cps_b, objram, output, &b.palette];
    let bytes: Vec<u8> = parts.iter().flat_map(|p| p.iter()).flat_map(|w| w.to_le_bytes()).collect();
    std::fs::write(path, bytes).unwrap();
}
