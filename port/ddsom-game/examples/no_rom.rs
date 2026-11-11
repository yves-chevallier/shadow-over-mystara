//! The game without ROM: does the game loaded from the extracted resources alone play like the one extracted from the
//! ROMs?
//!
//!   no_rom <roms> <eeprom> <assets> <scenario>:<frames> ...
//!
//! The game (machine without ROM, `cps2::Machine::game`, without 68000, Rust sound driver) replays each scenario twice,
//! in two processes (the game data are installed once per process):
//!   R: game data, residue and machine contents extracted in memory from the ROMs (`extract::install_game`);
//!   A: the same loaded from the assets folder (`extract::install_game_assets`), no ROM file opened.
//! At every frame: the displayed frame, the audio samples, the work RAM, the GFX RAM, the object RAM, the palette and
//! the shared RAM must be identical (compared through a hash per frame and per part). A stop of the game without 68000
//! on both at the same frame ends the scenario without counting it as a failure (a hole of the no-68000 mode, not of
//! the resources). Output: "IDENTICAL" or the first difference, per scenario, and the consultations of the residue.

use std::hash::Hasher;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};

/// FNV-1a 64 bits.
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

fn hash_words(w: &[u16]) -> u64 {
    let mut h = Fnv(0xcbf29ce484222325);
    for v in w {
        h.write(&v.to_le_bytes());
    }
    h.finish()
}

/// Child process: plays the scenario in one mode and prints one line per frame (the hash of each part), then the
/// residue line.
fn play(mode: &str, roms: &str, eeprom: &str, assets: &str, scenario: &str, frames: usize) {
    let ee = std::fs::read(eeprom).ok();
    let contents = match mode {
        "roms" => extract::install_game(Path::new(roms)).expect("roms"),
        _ => extract::install_game_assets(Path::new(assets)).expect("assets"),
    };
    let mut m = cps2::Machine::game(contents, ee.as_deref());
    ddsom_game::install_no_68k(&mut m);
    let script = ddsom_game::scenario::Scenario::load(Path::new(scenario)).expect("scenario");
    let mut out = std::io::BufWriter::new(std::io::stdout().lock());
    for f in 0..frames {
        script.apply(f, &mut m);
        let image = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| m.run_frame())) {
            Ok(image) => image,
            Err(e) => {
                let msg = e.downcast_ref::<String>().cloned().unwrap_or_default();
                match msg.strip_prefix(ddsom_game::no68k::STOP) {
                    Some(stop) => {
                        writeln!(out, "stop {}", stop.chars().take(200).collect::<String>().replace('\n', " ")).unwrap();
                        break;
                    }
                    None => std::panic::resume_unwind(e),
                }
            }
        };
        let mut hi = Fnv(0xcbf29ce484222325);
        for p in &image {
            hi.write(p);
        }
        let mut ha = Fnv(0xcbf29ce484222325);
        for s in m.sound.audio.drain(..) {
            ha.write(&s[0].to_le_bytes());
            ha.write(&s[1].to_le_bytes());
        }
        let b = &m.bus;
        let mut hq = Fnv(0xcbf29ce484222325);
        hq.write(&b.qram);
        let parts = [
            ("displayed frame", hi.finish()),
            ("audio samples", ha.finish()),
            ("work RAM", hash_words(&b.ram)),
            ("GFX RAM", hash_words(&b.gfxram)),
            ("object RAM 0", hash_words(&b.objram[0])),
            ("object RAM 1", hash_words(&b.objram[1])),
            ("palette", hash_words(&b.palette)),
            ("shared RAM", hq.finish()),
        ];
        let line: Vec<String> = parts.iter().map(|(name, h)| format!("{name}={h:016x}")).collect();
        writeln!(out, "frame {}", line.join(";")).unwrap();
    }
    let residue: u64 = ddsom_game::data::residue().iter().map(|r| r.1).sum();
    writeln!(out, "residue {residue}").unwrap();
}

/// A child process playing `scenario` in `mode`, and its lines.
fn child(mode: &str, a: &[String], scenario: &str, frames: usize) -> (Child, impl Iterator<Item = String>) {
    let exe = std::env::current_exe().expect("executable");
    let mut c = Command::new(exe)
        .args(["--play", mode, &a[1], &a[2], &a[3], scenario, &frames.to_string()])
        .stdout(Stdio::piped())
        .spawn()
        .expect("child process");
    let lines = BufReader::new(c.stdout.take().unwrap()).lines().map_while(Result::ok);
    (c, lines)
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if let [_, flag, mode, roms, eeprom, assets, scenario, frames] = &a[..] {
        if flag == "--play" {
            play(mode, roms, eeprom, assets, scenario, frames.parse().expect("frames"));
            return;
        }
    }
    if a.len() < 5 {
        eprintln!("usage: no_rom <roms> <eeprom> <assets> <scenario>:<frames> ...");
        std::process::exit(2);
    }
    let mut failures = 0;
    for spec in &a[4..] {
        let (path, frames) = spec.rsplit_once(':').expect("scenario:frames");
        let frames: usize = frames.parse().expect("frames");
        let t0 = std::time::Instant::now();
        let ((mut cr, mut r), (mut cs, mut s)) = (child("roms", &a, path, frames), child("assets", &a, path, frames));
        let mut verdict = None;
        let mut residue = (String::new(), String::new());
        for f in 1.. {
            match (r.next(), s.next()) {
                (Some(x), Some(y)) if x.starts_with("residue") => {
                    residue = (x, y);
                    break;
                }
                (Some(x), Some(y)) if x == y && x.starts_with("stop") => {
                    verdict = Some(format!("IDENTICAL over {} frames (then both stop without 68000: {})", f - 1, &x[5..]));
                }
                (Some(x), Some(y)) if x == y => {}
                (Some(x), Some(y)) => {
                    let what = x
                        .split(';')
                        .zip(y.split(';'))
                        .find(|(p, q)| p != q)
                        .map_or(format!("{x} / {y}"), |(p, _)| p.trim_start_matches("frame ").split('=').next().unwrap().to_string());
                    verdict = Some(format!("DIFFERENT at frame {f}: {what}"));
                    failures += 1;
                    break;
                }
                _ => {
                    verdict = Some(format!("DIFFERENT at frame {f}: a process ended early"));
                    failures += 1;
                    break;
                }
            }
        }
        // a child stopped early by a difference is not waited for
        for c in [&mut cr, &mut cs] {
            let _ = c.kill();
            let _ = c.wait();
        }
        let verdict = verdict.unwrap_or(format!("IDENTICAL over {frames} frames"));
        println!("{path} : {verdict}; {} (ROMs), {} (assets) ({:.1?})", residue.0, residue.1, t0.elapsed());
    }
    std::process::exit(if failures == 0 { 0 } else { 1 });
}
