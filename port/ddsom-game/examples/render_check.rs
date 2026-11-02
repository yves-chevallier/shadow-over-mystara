//! Bit-for-bit check of the rendering and the audio (optimisation phase, batch F).
//!
//! render_check <roms> <eeprom> <scenario> <frames> [output]
//!
//! Runs the game (without 68000, Rust sound driver) with rendering and audio (`DDSOM_SON_Z80=1`: the emulated Z80
//! instead) and prints one line per frame: hash of the pixels, hash of the audio samples produced. With `exit`, writes
//! these lines to a file; otherwise only the summary line is printed. Compare two outputs with `cmp`/`diff`.

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
    let mut m = cps2::Machine::load_ddsom(Path::new(&a[1]), ee.as_deref()).expect("roms");
    m.set_overclock(16.0);
    if std::env::var_os("DDSOM_SON_Z80").is_none() {
        m.sound.use_rust_driver();
    }
    ddsom_game::install_no_68k(&mut m);
    m.rendering = true;
    let script = ddsom_game::scenario::Scenario::load(Path::new(&a[3])).expect("scenario");
    let frames: usize = a[4].parse().unwrap();
    let mut out: Option<std::io::BufWriter<std::fs::File>> = a.get(5).map(|p| std::io::BufWriter::new(std::fs::File::create(p).unwrap()));
    let mut total = Fnv(0xcbf29ce484222325);
    let t0 = std::time::Instant::now();
    for f in 0..frames {
        script.apply(f, &mut m);
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
        if let Some(o) = out.as_mut() {
            writeln!(o, "{f} {:016x} {:016x}", hi.finish(), ha.finish()).unwrap();
        }
        total.write(&hi.finish().to_le_bytes());
        total.write(&ha.finish().to_le_bytes());
    }
    println!("{} frames, summary {:016x}, {:.0} frames/s", frames, total.finish(), frames as f64 / t0.elapsed().as_secs_f64());
}
