//! Zones of the program ROM read by the Rust code (output of `rom_reads` of ddsom-game) that the
//! resources do not rebuild.
//!
//!   rom_coverage <assets> <reads.txt> [gap]
//!   rom_coverage <assets> --bytes <file> [--without <asset file>]
//!
//! The second form writes the coverage word by word (one byte per word, data then program, 1 = covered)
//! for `DDSOM_COUVERT` of `rom_reads`; `--without data/natives.json` writes it without that file (for
//! `tools/native_zones.py`, which regenerates it).
//!
//! Prints, per space (data `d`, program `p`), the total read, the covered share, then the read zones that are not
//! covered (merged if fewer than `gap` bytes separate them, 0x10 by default): start, end exclusive,
//! uncovered bytes read, reads, scenario mask.

use std::path::Path;

use ddsom_assets::rebuild::Program;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if (a.len() == 4 || a.len() == 6 && a[4] == "--without") && a[2] == "--bytes" {
        let without = a.get(5).cloned().unwrap_or_default();
        let p = Program::load_files(Path::new(&a[1]), &|f| f != without).expect("resources");
        let mut out = Vec::new();
        for img in [&p.data, &p.prog] {
            out.extend((0..0x200000u32).map(|w| (img.is_covered(2 * w) && img.is_covered(2 * w + 1)) as u8));
        }
        std::fs::write(&a[3], out).unwrap();
        return;
    }
    let [_, assets, reads_path, ..] = &a[..] else {
        eprintln!("usage: rom_coverage <assets> <reads.txt> [gap]");
        std::process::exit(2);
    };
    let gap: u32 = a.get(3).map_or(0x10, |g| u32::from_str_radix(g, 16).unwrap());
    let p = Program::load(Path::new(assets)).expect("resources");
    // read bytes covered by each file: (file, data, program)
    let reads: Vec<(char, u32, u32)> = std::fs::read_to_string(reads_path)
        .unwrap()
        .lines()
        .filter(|l| l.starts_with(['d', 'p']) && l.as_bytes()[1] == b' ')
        .map(|l| {
            let f: Vec<&str> = l.split(' ').collect();
            (f[0].chars().next().unwrap(), u32::from_str_radix(f[1], 16).unwrap(), u32::from_str_radix(f[2], 16).unwrap())
        })
        .collect();
    println!("# bytes read and covered, per resource file (data, program)");
    for rel in Program::FILES {
        let q = Program::load_files(Path::new(assets), &|f| f == rel).expect("resources");
        let count = |view: char, img: &ddsom_assets::rebuild::Image| -> u64 {
            reads.iter().filter(|r| r.0 == view).map(|&(_, s, e)| (s..e).filter(|&a| img.is_covered(a)).count() as u64).sum()
        };
        println!("{rel} {} {}", count('d', &q.data), count('p', &q.prog));
    }
    for (view, name, img) in [('d', "data", &p.data), ('p', "program", &p.prog)] {
        let (mut total, mut covered) = (0u64, 0u64);
        // uncovered zones: (start, end, reads, mask)
        let mut miss: Vec<(u32, u32, u64, u32)> = Vec::new();
        for l in std::fs::read_to_string(reads_path).unwrap().lines() {
            let f: Vec<&str> = l.split(' ').collect();
            if f[0].chars().next() != Some(view) {
                continue;
            }
            let (s, e) = (u32::from_str_radix(f[1], 16).unwrap(), u32::from_str_radix(f[2], 16).unwrap());
            let (mask, reads) = (u32::from_str_radix(f[3], 16).unwrap(), f[4].parse::<u64>().unwrap());
            total += (e - s) as u64;
            for w in (s..e).step_by(2) {
                if img.is_covered(w) && img.is_covered(w + 1) {
                    covered += 2;
                } else {
                    match miss.last_mut() {
                        Some(m) if w - m.1 <= gap => {
                            m.1 = w + 2;
                            m.2 += reads * 2 / (e - s) as u64;
                            m.3 |= mask;
                        }
                        _ => miss.push((w, w + 2, reads * 2 / (e - s) as u64, mask)),
                    }
                }
            }
        }
        println!("# {name} space: {total} bytes read, {covered} covered, {} uncovered zones", miss.len());
        for (s, e, reads, mask) in &miss {
            println!("{view} {s:06x} {e:06x} {} {reads} {mask:x}", e - s);
        }
    }
}
