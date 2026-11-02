//! Differential fuzz of the extra RAM of the development card (`eeprom/dev_card.rs`).
//!
//! On random states of the RAM 0x660000 (valid zones, corrupted copies, counters near
//! their bounds, blank or random RAM), compares the original code (recompiled 68000, without any native) to
//! the Rust version: complete memory and returned registers.
//!
//!   fuzz_dev_card <roms> <eeprom> [trials [seed]]

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;

use cps2::Cps2Bus;
use m68k::Cpu;

use ddsom_game::mem::{rd32, rd8, wr16, wr32, wr8, A5};

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn among<T: Copy>(&mut self, v: &[T]) -> T {
        v[self.below(v.len() as u64) as usize]
    }
}

const RAM: u32 = 0x66_0000;
/// (offset, words) of the zones of the extra RAM.
const ZONES: &[(u32, u32)] =
    &[(0x1140, 32), (0x1020, 4), (0x1028, 4), (0x1030, 4), (0x1038, 4), (0x1040, 4), (0x1048, 4), (0x1200, 48), (0x1260, 8), (0x1270, 2)];

fn write_long(b: &mut Cps2Bus, a: u32, v: u32) {
    for i in 0..4 {
        wr8(b, a + 1 + 2 * i, (v >> (24 - 8 * i)) as u8);
    }
}

/// A plausible state of the extra RAM for region `r`.
fn state(b: &mut Cps2Bus, a: &mut Rng, r: u32) {
    let values: [u32; 12] = [0, 1, 5, 999_998, 999_999, 1_000_000, 0x127a_3980, 0x127a_3981, 0x4f_5880, 0x4f_5881, 0xffff_ffff, 0x1234_5678];
    for k in 0..3 {
        for &(start, words) in ZONES {
            let base = RAM + start + 0x1000 * k;
            for i in 0..words {
                wr16(b, base + 2 * i, 0);
            }
        }
    }
    let mode = a.below(10);
    for &(start, words) in ZONES {
        // a value common to the three copies
        let mut bytes = vec![0u8; words as usize];
        for (i, o) in bytes.iter_mut().enumerate() {
            *o = match start {
                0x1140 => ddsom_game::mem::rd8(b, 0x2ebfc + ((r << 4) as u16 as u32) + i as u32),
                _ => {
                    if i % 4 == 0 && words == 4 {
                        0
                    } else {
                        { let r = a.next() as u8; a.among(&[0, 0, 1, 2, 0xff, r]) }
                    }
                }
            };
        }
        if words == 4 {
            let v = if a.below(3) == 0 { a.next() as u32 } else { a.among(&values) };
            for (i, o) in bytes.iter_mut().enumerate() {
                *o = (v >> (24 - 8 * i as u32)) as u8;
            }
        }
        for k in 0..3 {
            let base = RAM + start + 0x1000 * k;
            for (i, &o) in bytes.iter().enumerate() {
                match mode {
                    0 => wr16(b, base + 2 * i as u32, a.next() as u16),
                    1 => {}
                    _ => wr16(b, base + 2 * i as u32, if a.below(4) == 0 { (a.next() as u16) & 0xff00 | o as u16 } else { o as u16 }),
                }
            }
        }
        // corruption: one copy, sometimes two, a few bytes
        if mode >= 2 && a.below(2) == 0 {
            for _ in 0..1 + a.below(3) {
                let k = a.below(3) as u32;
                let i = a.below(words as u64 * 2) as u32;
                let v = a.next() as u8;
                wr8(b, RAM + start + 0x1000 * k + i, v);
            }
        }
    }
    // variables de travail
    for off in [0x6164u32, 0x6160, 0x615c, 0x6158, 0x6154, 0x6150, 0x611c, 0x6118] {
        let v = a.among(&values);
        wr32(b, A5 - off, v);
    }
    for i in 0..48 {
        wr8(b, A5 - 0x614c + i, a.next() as u8);
    }
}

fn run_original_code(c: &mut Cpu, b: &mut Cps2Bus) -> bool {
    let ret = ddsom_game::mem::rd32(b, c.a[7]);
    let sp_after = c.a[7].wrapping_add(4);
    let mut guard = 0;
    while !(c.pc == ret && c.a[7] == sp_after) {
        if ddsom_rc::run_block(c.pc, c, b, u64::MAX).is_none() {
            c.step(b);
        }
        guard += 1;
        if guard > 200_000 {
            return false;
        }
    }
    true
}

type Rust = Box<dyn Fn(&mut Cpu, &mut Cps2Bus)>;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (Some(roms), Some(eeprom)) = (a.get(1), a.get(2)) else {
        eprintln!("usage: fuzz_dev_card <roms> <eeprom> [trials [seed]]");
        std::process::exit(2);
    };
    let tests: u32 = a.get(3).map_or(2000, |s| s.parse().unwrap());
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15 ^ a.get(4).map_or(1, |s| s.parse::<u64>().unwrap()).wrapping_mul(0x1000_0000_01b3));
    std::panic::set_hook(Box::new(|_| {}));
    let ee = std::fs::read(eeprom).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(roms), ee.as_deref()).expect("roms");
    ddsom_game::install(&mut m);
    // a few frames: the work RAM is that of a game that has started
    for _ in 0..700 {
        m.run_frame();
    }
    // (name, original entry, Rust routine, random D0 register?)
    let natives = |entry: u32| -> Rust {
        let n = ddsom_game::all_natives().find(|n| n.entry == entry).expect("native");
        Box::new(move |c, b| n.execute(c, b))
    };
    let case: Vec<(&str, u32, Rust)> = vec![
        ("startup 0x2dc9a", 0x2dc9a, natives(0x2dc9a)),
        ("reset 0x2e8e4", 0x2e8e4, Box::new(|c, b| {
            ddsom_game::eeprom::reset_card(c, b);
            ddsom_game::mem::rts(c, b);
        })),
        ("game time 0x2e540", 0x2e540, Box::new(|c, b| {
            ddsom_game::eeprom::dev_card::play_time(b);
            ddsom_game::mem::rts(c, b);
        })),
        ("coin 0x2e448", 0x2e448, natives(0x2e448)),
        ("start 0x2e49a", 0x2e49a, natives(0x2e49a)),
        ("parties 0x2e4ec", 0x2e4ec, natives(0x2e4ec)),
        ("personnage 0x2e630", 0x2e630, natives(0x2e630)),
        ("duration 0x2e696", 0x2e696, natives(0x2e696)),
    ];
    let mut total = 0;
    for (name, entry, rust) in &case {
        let (mut gaps, mut panics, mut abandons) = (0, 0, 0);
        let mut examples = Vec::new();
        for _ in 0..tests {
            let mut c = m.cpu.clone();
            let mut b = m.bus.clone();
            let region = rng.below(7) as u32;
            wr16(&mut b, A5 + 0x70, region as u16);
            wr8(&mut b, A5 + 0x82, 1);
            wr8(&mut b, A5 + 0x25, rng.below(2) as u8);
            state(&mut b, &mut rng, region);
            c.d[0] = match *entry {
                0x2e630 => rng.among(&[0, 1, 2, 3, 4, 5, 6, 11, 0xffff, 0x1234_0005]),
                0x2e696 => {
                    let (p, q) = (rng.next() as u32 & 0xffff, rng.next() as u32);
                    rng.among(&[0, 59, 60, 61, 3600, 0x7fff_ffff, 0x8000_0000, 0xffff_ffff, p, q])
                }
                _ => c.d[0],
            };
            // the duration must be compared with the longest / shortest of the moment
            c.a[7] = 0xff_f000;
            c.a[7] -= 4;
            wr32(&mut b, c.a[7], 0xfffe);
            c.pc = *entry;
            let (mut c2, mut b2) = (c.clone(), b.clone());
            let before = b.clone();
            if !run_original_code(&mut c2, &mut b2) {
                abandons += 1;
                continue;
            }
            let r = catch_unwind(AssertUnwindSafe(|| rust(&mut c, &mut b)));
            if r.is_err() {
                panics += 1;
                continue;
            }
            let mut diffs = Vec::new();
            let sp = c2.a[7] & 0xff_ffff;
            ddsom_game::mem::diff_bus(&b, &b2, sp.saturating_sub(0x400)..sp, &mut diffs);
            if std::env::var_os("FUZZ_REGISTRES").is_some() {
                for i in 0..8 {
                    if c.d[i] != c2.d[i] {
                        diffs.push(format!("d{i}={:08x} instead of {:08x}", c.d[i], c2.d[i]));
                    }
                    if i < 7 && c.a[i] != c2.a[i] {
                        diffs.push(format!("a{i}={:08x} instead of {:08x}", c.a[i], c2.a[i]));
                    }
                }
            }
            if c.pc != c2.pc {
                diffs.push(format!("pc={:06x} instead of {:06x}", c.pc, c2.pc));
            }
            if !diffs.is_empty() {
                gaps += 1;
                if std::env::var_os("FUZZ_DUMP").is_some() && examples.is_empty() {
                    for (name, bus) in [("before", &before), ("rust", &b), ("original", &b2)] {
                        for k in 0..3usize {
                            let i = (0x1020 + 0x1000 * k) / 2;
                            let sig = (0x1140 + 0x1000 * k) / 2;
                            println!("{name} {k}: {:04x?} sig {:04x?}", &bus.extra_ram[i..i + 4], &bus.extra_ram[sig..sig + 4]);
                        }
                    }
                }
                if examples.len() < 3 {
                    examples.push(format!("region {region} d0={:08x}: {}", c2.d[0], diffs.join(", ")));
                }
            }
        }
        println!("{name:28} {tests} trials, {gaps} mismatches, {panics} panics, {abandons} abandons");
        for e in examples {
            println!("    {e}");
        }
        total += gaps + panics;
    }
    let _ = rd32;
    let _ = rd8;
    println!("{}", if total == 0 { "FUZZ OK" } else { "FUZZ MISMATCHES" });
}
