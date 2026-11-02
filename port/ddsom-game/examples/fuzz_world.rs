//! Fuzz of the "world" routines rewritten with their exact registers: sorted lists (0x1ee1a, 0x1ef6c), collision
//! with the scenery (0x33fe4, 0x3403c), vertical follow of the players (0x31616). The AI aids and the barrier aids no
//! longer have natives: `fuzz_ia` compares the AI cores.
//!
//! Each routine is called in check mode (the original runs alongside on a copy; memory,
//! all registers and flags are compared) from randomly drawn states: fabricated object lists,
//! positions, scenery tables, arbitrary registers.
//!
//!   fuzz_world <roms> <eeprom> <scenario> <warm-up frames> <rounds> [routine prefix]
//!
//! Variables : `DDSOM_GRAINE`, `DDSOM_VERBOSE`.

use std::path::Path;

use ddsom_game::mem::{rd16, wr16, wr32, wr8, A5};

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
    fn among<T: Copy>(&mut self, v: &[T]) -> T {
        v[self.below(v.len() as u32) as usize]
    }
}

type Cpu = m68k::Cpu;
type Bus = cps2::Cps2Bus;

/// RAM area where objects are fabricated (unused by the game at that moment: we work on a copy).
const ZONE: u32 = 0xffe000;
const STEP: u32 = 0x80;

fn regs(c: &mut Cpu, rng: &mut Rng) {
    for r in 0..8 {
        c.d[r] = match rng.below(4) {
            0 => rng.next() & 0xff,
            1 => rng.next() & 0xffff,
            _ => rng.next(),
        };
    }
    for r in 1..5 {
        c.a[r] = 0xff0000 | (rng.next() & 0xffff);
    }
    c.sr = (c.sr & !0x1f) | (rng.next() as u16 & 0x1f);
}

fn object(i: u32) -> u32 {
    ZONE + STEP * i
}

fn pose_y(b: &mut Bus, o: u32, y: u32) {
    wr32(b, o + 0x10, y);
}

/// Depth: few values, to provoke ties.
fn depth(rng: &mut Rng) -> u32 {
    if rng.below(3) == 0 {
        rng.next()
    } else {
        rng.below(8) << 16 | rng.among(&[0, 0x8000])
    }
}

fn list(c: &mut Cpu, b: &mut Bus, rng: &mut Rng, head: u32, queue: u32, end: u16) {
    let n = rng.below(7);
    let mut ys: Vec<u32> = (0..n).map(|_| depth(rng)).collect();
    ys.sort_by(|a, b| (*b as i32).cmp(&(*a as i32)));
    for (i, &y) in ys.iter().enumerate() {
        let o = object(i as u32);
        pose_y(b, o, y);
        let prev = if i == 0 { end } else { object(i as u32 - 1) as u16 };
        let next = if i as u32 + 1 == n { end } else { object(i as u32 + 1) as u16 };
        wr16(b, o + 0x24, prev);
        wr16(b, o + 0x26, next);
    }
    wr16(b, A5 + head, if n == 0 { 0 } else { object(0) as u16 });
    wr16(b, A5 + queue, if n == 0 { 0 } else { object(n - 1) as u16 });
    // the subject: an object of the list whose depth changes, or a free object
    if n > 0 && rng.below(3) != 0 {
        let k = rng.below(n);
        let y = if rng.below(2) == 0 { depth(rng) } else { ys[k as usize] };
        pose_y(b, object(k), y);
        c.a[0] = object(k);
    } else {
        let o = object(8);
        wr16(b, o + 0x24, 0);
        wr16(b, o + 0x26, 0);
        pose_y(b, o, depth(rng));
        c.a[0] = o;
    }
}

fn rng_terrain(c: &mut Cpu, b: &mut Bus, rng: &mut Rng) {
    // layer 2 attributes: valid terrain numbers (the type table has 128)
    for i in (0xa000..0xc000).step_by(2) {
        b.gfxram[i] = rng.next() as u16;
        let number = if rng.below(3) == 0 { rng.below(128) } else { rng.among(&[0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]) };
        b.gfxram[i + 1] = (number << 7) as u16 | (rng.next() as u16 & 0x7f);
    }
    wr8(b, A5 + 0x34, rng.among(&[1, 2, 5, 0x14]));
    wr8(b, A5 + 0x36, rng.below(2) as u8);
    wr16(b, A5 + 0x7fd0, if rng.below(8) == 0 { 0 } else { 0x800 | rng.next() as u16 & 0x7ff });
    for off in [0x506u32, 0x50a, 0x556, 0x55a] {
        wr16(b, A5 + off, rng.next() as u16 & 0xfff);
    }
    let o = object(0);
    wr16(b, o + 8, rng.next() as u16 & 0xfff);
    wr16(b, o + 0x10, rng.next() as u16 & 0x3ff);
    wr16(b, o + 0x14, rng.next() as u16);
    c.a[0] = o;
}

fn rng_camera_y(c: &mut Cpu, b: &mut Bus, rng: &mut Rng) {
    let cam = A5 + 0x4fe;
    let y = rng.below(0x300) as u16;
    wr16(b, cam + 0xc, y);
    for rec in [0x5deu32, 0x7de, 0x9de, 0xbde] {
        wr8(b, A5 + rec, rng.below(4).min(1) as u8);
        wr16(b, A5 + rec + 0x62, if rng.below(4) == 0 { 0 } else { rng.next() as u16 & 0x7f });
        wr16(b, A5 + rec + 0x10, y.wrapping_add(rng.below(0x200) as u16).wrapping_sub(0x40));
    }
    c.a[0] = if rng.below(4) == 0 { object(0) } else { cam };
    wr16(b, c.a[0] + 0xc, y);
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 6 {
        eprintln!("usage: fuzz_world <roms> <eeprom> <scenario> <warm-up frames> <rounds> [prefix]");
        std::process::exit(2);
    }
    let (roms, eeprom, scenario) = (&a[1], &a[2], &a[3]);
    let warmup: u32 = a[4].parse().unwrap();
    let rounds: u32 = a[5].parse().unwrap();
    let filter = a.get(6).cloned().unwrap_or_default();
    let ee = std::fs::read(eeprom).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(roms), ee.as_deref()).expect("roms");
    ddsom_game::install(&mut m);
    let script = ddsom_game::scenario::Scenario::load(Path::new(scenario)).expect("scenario");
    for f in 0..warmup as usize {
        script.apply(f, &mut m);
        m.run_frame();
    }
    ddsom_game::set_check(true);
    ddsom_game::set_check_all(true);
    std::panic::set_hook(Box::new(|_| {}));
    let seed: u64 = std::env::var("DDSOM_GRAINE").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    let mut rng = Rng(0x1234_5678_9abc_def1 ^ seed.wrapping_mul(0x9e37_79b9_7f4a_7c15));
    let (c0, b0) = (m.cpu.clone(), m.bus.clone());

    type Prep = fn(&mut Cpu, &mut Bus, &mut Rng);
    let routines: Vec<(u32, &str, Prep)> = vec![
        (0x1ee1a, "list_reinsert_actors", |c, b, a| list(c, b, a, 0x5be, 0x5c0, 1)),
        (0x1ef6c, "list_reinsert_foreground", |c, b, a| list(c, b, a, 0x5c2, 0x5c4, 2)),
        (0x33fe4, "terrain_push", rng_terrain),
        (0x3403c, "terrain_push_2", rng_terrain),
        (0x31616, "camera_follows_players_y", rng_camera_y),
    ];
    for _ in 0..rounds {
        for &(entry, name, prep) in &routines {
            if !name.starts_with(&filter) {
                continue;
            }
            let (mut c, mut b) = (c0.clone(), b0.clone());
            b.irq_horizon = u64::MAX;
            regs(&mut c, &mut rng);
            prep(&mut c, &mut b, &mut rng);
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| ddsom_game::call(&mut c, &mut b, entry)));
        }
    }
    let _ = rd16;
    let mut bad = 0;
    for (name, s) in ddsom_game::report() {
        if routines.iter().any(|r| r.1 == name) {
            println!("{name:30} {:6} calls {:6} mismatches {}", s.calls, s.mismatches, s.first.as_deref().unwrap_or(""));
            bad += s.mismatches;
        }
    }
    println!("{}", if bad == 0 { "FUZZ OK" } else { "FUZZ MISMATCHES" });
}
