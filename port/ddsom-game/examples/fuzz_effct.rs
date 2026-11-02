//! Differential fuzz of the EFFCT handlers (table `0x39ce0`) on complete trajectories.
//!
//! For each requested type, an object of the type is created in the EFFCT pool (variant drawn, birth in
//! state 0 or an active state of the grid, owner = player or active enemy, link fields on the
//! owner) then the handler is called frame after frame, with perturbations of the owner,
//! the players and the scene. Each native routine call is compared with the original (control mode of
//! `ddsom_game`, native subroutines included). The coverage of the handler's original code (run
//! separately on a copy) is collected: at the end, the handler instructions never executed
//! by the original are listed (unvalidated paths).
//!
//!   fuzz_effct <roms> <eeprom> <scenario> <warm-up frames> <comma-separated types>
//!
//! Variables: `FUZZ_CREATEURS=b4bcc,b96c0` (creators tried from registers drawn at random),
//! `FUZZ_VARIANTES=0,2,5` (variants drawn; default 0 to 15), `FUZZ_TOURS=n` (trajectories per
//! variant, 24), `FUZZ_PAS=n` (steps per trajectory, 60), `FUZZ_GRAINE=n`, `FUZZ_PLAGE=start-end` (range of the
//! original code whose coverage is collected; by default from the entry to the next table entry).

use std::collections::BTreeSet;
use std::path::Path;

use cps2::Cps2Bus;
use m68k::decode::{Ea, Fetch, Op};
use m68k::Cpu;

use ddsom_game::mem::{rd16, rd8, wr16, wr32, wr8, A5};
use ddsom_game::objects::pools::{EFFCT, ENEMY};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn pick<T: Copy>(&mut self, v: &[T]) -> T {
        v[self.next() as usize % v.len()]
    }
    fn one_in(&mut self, n: u32) -> bool {
        self.next() % n == 0
    }
}

const PLAYERS: u32 = 0xff85de;

/// Instructions reachable from `entry` in `range` (branches, local `bsr`, jump tables whose
/// end is the start of the first code they designate).
fn reachable(b: &mut Cps2Bus, entry: u32, range: std::ops::Range<u32>) -> BTreeSet<u32> {
    let mut seen = BTreeSet::new();
    let mut todo = vec![entry];
    let inside = |t: u32| range.contains(&t) && t % 2 == 0;
    while let Some(start) = todo.pop() {
        let mut pc = start;
        while inside(pc) && !seen.contains(&pc) {
            let ins = m68k::decode(b, pc);
            seen.insert(pc);
            let next = pc + ins.len.max(2);
            let mut continue_ = true;
            match ins.op {
                Op::Bra(t) => {
                    todo.push(t);
                    continue_ = false;
                }
                Op::Bcc(_, t) | Op::Bsr(t) | Op::Dbcc(_, _, t) => todo.push(t),
                Op::Rts | Op::Rte | Op::Rtr | Op::Illegal => continue_ = false,
                Op::Jmp(ea) | Op::Jsr(ea) => {
                    match ea {
                        Ea::AbsL(t) | Ea::AbsW(t) | Ea::PcDisp(t) => todo.push(t),
                        Ea::PcIdx(base, _) => {
                            let mut end = range.end;
                            let mut at = base;
                            while at < end && at < base + 0x200 {
                                let t = (base as i32 + b.fetch16(at) as i16 as i32) as u32;
                                if inside(t) && t > at {
                                    end = end.min(t);
                                    todo.push(t);
                                }
                                at += 2;
                            }
                        }
                        _ => {}
                    }
                    if matches!(ins.op, Op::Jmp(_)) {
                        continue_ = false;
                    }
                }
                _ => {}
            }
            if !continue_ {
                break;
            }
            pc = next;
        }
    }
    seen
}

/// Runs the original code alone on a copy and returns the addresses executed in `range`.
fn coverage(c: &Cpu, b: &Cps2Bus, entry: u32, range: &std::ops::Range<u32>, seen: &mut BTreeSet<u32>) -> Option<(Cpu, Cps2Bus)> {
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let (mut c, mut b) = (c.clone(), b.clone());
        let sp = c.a[7];
        c.pc = entry;
        c.a[7] = c.a[7].wrapping_sub(4);
        wr32(&mut b, c.a[7], 0xfffe);
        let mut local = Vec::new();
        for _ in 0..200_000 {
            if c.pc == 0xfffe && c.a[7] == sp {
                return (local, Some((c, b)));
            }
            if range.contains(&c.pc) {
                local.push(c.pc);
            }
            c.step(&mut b);
        }
        (local, None)
    }));
    let (l, end) = r.ok()?;
    seen.extend(l);
    end
}

/// Comparison done here (`FUZZ_PROPRE=1`, without control mode): A0, D7, RAM outside the stack and the task table
/// of the 68000 (`0xff0000-0xff0400`, which the kernel natives keep in Rust) and outside the private sound stack, GFX RAM,
/// palette, OBJ RAM.
fn compare(orig: &(Cpu, Cps2Bus), c: &Cpu, b: &Cps2Bus) -> Vec<String> {
    let (co, bo) = orig;
    let mut d = Vec::new();
    if co.a[0] != c.a[0] {
        d.push(format!("a0={:08x} au lieu de {:08x}", c.a[0], co.a[0]));
    }
    if co.d[7] != c.d[7] {
        d.push(format!("d7={:08x} au lieu de {:08x}", c.d[7], co.d[7]));
    }
    let sp = co.a[7] & 0xff_ffff;
    // private stack of the sound wrappers (pointer at FF2A1A), like `mem::diff_bus`
    let private = ((bo.ram[0x2a1a / 2] as u32) << 16 | bo.ram[0x2a1c / 2] as u32) & 0xff_ffff;
    for i in 0..bo.ram.len() {
        let a = 0xff0000 + 2 * i as u32;
        let dead = (sp.saturating_sub(0x400)..sp).contains(&a) || (private.saturating_sub(0x400)..private).contains(&a) || a < 0xff0400;
        if bo.ram[i] != b.ram[i] && !dead && d.len() < 8 {
            d.push(format!("ram[{a:06x}]={:04x} au lieu de {:04x}", b.ram[i], bo.ram[i]));
        }
    }
    for (name, x, y) in [("gfxram", &b.gfxram, &bo.gfxram), ("palette", &b.palette, &bo.palette), ("objram0", &b.objram[0], &bo.objram[0])] {
        if let Some(i) = x.iter().zip(y.iter()).position(|(p, q)| p != q) {
            d.push(format!("{name}[{i:x}]={:04x} au lieu de {:04x}", x[i], y[i]));
        }
    }
    d
}

/// Sub-states (`+5`) of the active state that the original's table provides (beyond that, it jumps into data).
fn sub_states(t: u32, v: u32) -> &'static [u8] {
    match (t, v) {
        (80, 0) => &[0, 2, 4, 6, 8, 10, 12, 14, 16],
        (80, 1) => &[0, 2, 4],
        (80, 6) => &[0, 2, 4, 6],
        (80, _) => &[0, 2],
        _ => type_sub_states(t),
    }
}

fn type_sub_states(t: u32) -> &'static [u8] {
    match t {
        63 | 43 => &[0, 2, 4],
        46 | 48 | 70 => &[0, 2],
        40 => &[0, 2, 4],
        _ => &[0],
    }
}

/// Type-specific settings, after the generic creation of the object.
fn prepare(t: u32, b: &mut Cps2Bus, obj: u32, owner: u32, rng: &mut Rng) {
    let player = PLAYERS + 0x200 * (rng.next() % 4);
    match t {
        // fireworks: scene step, fade, host (+0x56), scenery (+0x42), delays
        80 => {
            wr8(b, A5 + 0x2e, rng.pick(&[0, 0x12, 0x12, 0xc]));
            wr8(b, A5 + 0x48f, rng.pick(&[0, 1]));
            wr16(b, obj + 0x56, owner as u16);
            wr16(b, obj + 0x42, rng.pick(&[owner as u16, PLAYERS as u16]));
            wr8(b, owner + 0x22, rng.pick(&[0, 1]));
            wr16(b, obj + 0x58, (rng.next() % 18) as u16);
            wr16(b, obj + 0x5a, (rng.next() % 4) as u16);
            wr8(b, obj + 0x33, rng.pick(&[0, 0, 1]));
            wr16(b, obj + 0xc, (rng.next() % 0x40) as u16);
            if rng.one_in(3) {
                // almost at the top of the screen
                let high = rd16(b, A5 + 0x5aa).wrapping_add(0x100).wrapping_sub(rd16(b, obj + 0x10));
                wr16(b, obj + 0xc, high.wrapping_sub((rng.next() % 8) as u16));
            }
        }
        // player-linked effects: the owner is a player
        81 => {
            wr16(b, obj + 0x50, player as u16);
            for off in [0x8u32, 0xc, 0x10] {
                let w = rd16(b, player + off);
                wr16(b, obj + off, w);
            }
            wr16(b, player + 0x42, (rng.next() % 6) as u16);
        }
        // creator at +0x42
        63 => wr16(b, obj + 0x42, owner as u16),
        // carrier (ENEMY) at +0x46, piece 0 to 2; life around the thresholds; hits received
        46 => {
            let carrier = if rng.one_in(8) { obj } else { owner };
            wr16(b, obj + 0x46, carrier as u16);
            wr8(b, obj + 0x2f, (rng.next() % 3) as u8);
            wr16(b, carrier + 0x60, 0x100);
            wr16(b, carrier + 0x62, (rng.next() % 0x140) as u16);
            wr8(b, carrier + 0x33, rng.pick(&[0, 0, 1, 2, 0x81, 5]));
            wr8(b, obj + 0x5c, (rng.next() % 4) as u8);
        }
        // carrier at +0x46 (itself following an object), phase +0xa4; arming, countdown, angle
        48 => {
            wr16(b, obj + 0x46, if rng.one_in(10) { 0 } else { owner as u16 });
            let other = PLAYERS + 0x200 * (rng.next() % 4);
            wr16(b, owner + 0x46, rng.pick(&[0, owner as u16, other as u16]));
            wr8(b, owner + 0xa4, rng.pick(&[0, 1, 1, 3, 3, 2]));
            wr8(b, obj + 0x42, rng.pick(&[0, 1, 0x80]));
            wr8(b, obj + 0x43, rng.pick(&[0, 1, 2, 0x20]));
            wr16(b, obj + 0x5c, rng.next() as u16);
            wr32(b, obj + 0x4c, 0x1ca4b6);
            wr8(b, owner + 0x2c, rng.pick(&[0, 0x20]));
            wr8(b, A5 + 0x36, rng.pick(&[0, 0, 1]));
        }
        // action bubble: carrier (+0x50) player whose action (+0x3d) is in the table (0 to 32), carried object
        4 => {
            let j = PLAYERS + 0x200 * (rng.next() % 4);
            wr16(b, obj + 0x50, j as u16);
            // actions provided by each variant's tables (0: 0 to 32; 2: 0 to 18; 1, 5, 6: 0 to 4)
            let actions: &[u8] = match rd8(b, obj + 3) {
                0 => &[0, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 22, 24, 26, 28, 30, 32],
                2 => &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
                1 => &[0, 2, 4],
                _ => &[0, 1, 2],
            };
            wr8(b, j + 0x3d, rng.pick(actions));
            wr8(b, owner + 0x3d, rng.pick(actions));
            wr16(b, obj + 0x54, rng.pick(&[owner as u16, j as u16]));
            for off in [0x10du32, 0x10e] {
                wr8(b, j + off, (rng.next() % 6) as u8);
            }
            wr8(b, j + 0x77, rng.pick(&[0, 1]));
            wr8(b, j + 3, 2 * (1 + rng.next() % 6) as u8);
        }
        // shops: buyers (list A5-0x555c) in the zone, ready to buy (+0x58), player (+0x50) with
        // gold, a life, an inventory; scene counter: sale in progress, seller (+0x1a)
        22 | 70 => {
            for k in 0..4u32 {
                let buyer = PLAYERS + 0x200 * k;
                wr16(b, 0xff2aa4 + 2 * k, if rng.one_in(3) { 0 } else { buyer as u16 });
                wr8(b, buyer + 0x58, rng.pick(&[0, 1, 1, 0x80]));
                wr16(b, buyer + 0x50, buyer as u16);
                let x0 = rd16(b, obj + 8);
                wr16(b, buyer + 8, x0.wrapping_add((rng.next() % 0x40) as u16).wrapping_sub(0x20));
                let y0 = rd16(b, obj + 0x10);
                wr16(b, buyer + 0x10, y0.wrapping_sub((rng.next() % 0x40) as u16));
                wr16(b, buyer + 0xc, 0);
                wr32(b, buyer + 0x114, rng.pick(&[0, 0x10, 0x100, 0x9999, 0x0099_9999]));
                wr8(b, buyer + 0x167, rng.pick(&[0, 9]));
                wr16(b, buyer + 0x60, 0x80);
                wr16(b, buyer + 0x62, rng.pick(&[0x10, 0x70, 0x80]));
                wr8(b, buyer + 3, 2 * (1 + rng.next() % 6) as u8);
                for slot in 0..6u32 {
                    wr8(b, buyer + 0x140 + slot, rng.pick(&[0x56, 0x56, 1, 2, 0x23]));
                    wr8(b, buyer + 0x148 + slot, rng.pick(&[0, 1, 9]));
                }
            }
            wr8(b, obj + 0x18, rng.pick(&[0, 1, 2]));
            // zone and price of an already open stall (birth replaces them)
            for (off, v) in [(0x14u32, 0x40u16), (0x16, 0x40), (0x54, 0xfff0), (0x56, 0x1c), (0x52, 0x100)] {
                wr16(b, obj + off, v);
            }
            wr16(b, obj + 0x58, rng.pick(&[0x0102, 0x0523, 0x0001]));
            wr8(b, A5 + 0x35, (rng.next() % 6) as u8);
            wr16(b, 0xff2abe, rng.pick(&[0, 0, obj as u16, owner as u16]));
            wr16(b, obj + 0x1a, owner as u16);
            wr16(b, owner + 0x32, rng.pick(&[0, 1, 5]));
            wr8(b, obj + 6, rng.pick(&[0, 2, 4]));
            if rng.one_in(2) {
                // the object is placed on screen, in front of the scene counter's buyer zone
                let (cx, cy) = (rd16(b, A5 + 0x506), rd16(b, A5 + 0x50a));
                for k in 0..4u32 {
                    let buyer = PLAYERS + 0x200 * k;
                    wr16(b, buyer + 8, cx.wrapping_add(0xa8 + (rng.next() % 0x58) as u16));
                    wr16(b, buyer + 0x10, cy.wrapping_add(0x48 + (rng.next() % 0x14) as u16));
                }
            }
        }
        // sorts d'un magicien : six modes
        44 => {
            wr8(b, obj + 0x2f, (rng.next() % 6) as u8);
            wr16(b, obj + 0x5c, owner as u16);
        }
        // vote: restricted setting (ballot)
        23 => {
            wr8(b, obj + 0x2f, rng.pick(&[0, 1, 2]));
            // text of the winning choice (pointer `+0x30`, length `+0x34`) in the ballot buffer
            wr32(b, obj + 0x30, 0xff2072 + 2 * (rng.next() % 0x20));
            wr16(b, obj + 0x34, (rng.next() % 8) as u16);
        }
        // chain link: previous (+0x5c), head (+0x46) whose byte +0x58 announces the end or the break,
        // head's carrier (+0x54), rank
        12 => {
            let head = PLAYERS + 0x200 * (rng.next() % 4);
            wr16(b, obj + 0x5c, owner as u16);
            wr16(b, obj + 0x46, head as u16);
            wr8(b, head + 0x58, rng.pick(&[0, 0x80, 0xfe, 0xfe]));
            wr16(b, head + 0x54, owner as u16);
            wr8(b, obj + 0x2f, (rng.next() % 4) as u8);
            wr8(b, owner + 0x28, rng.pick(&[2, 2, 4]));
        }
        // scene effects: followed object (+0x46, sometimes none), setting
        42 => {
            wr16(b, obj + 0x46, if rng.one_in(8) { 0 } else { owner as u16 });
            wr8(b, obj + 0x2f, (rng.next() % 8) as u8);
            wr8(b, owner + 0x2c, rng.pick(&[0, 0x20]));
        }
        // hit effects: attacker at +0x5c in state 0x204 and a table step (+6), setting
        40 => {
            wr16(b, obj + 0x5c, owner as u16);
            wr16(b, owner + 4, rng.pick(&[0x204, 0x204, 0x202]));
            wr8(b, owner + 6, rng.pick(&[0x38, 0x3a, 0x3c, 0x10]));
            wr8(b, owner + 7, (rng.next() % 6) as u8);
            wr8(b, owner + 3, (rng.next() % 8) as u8);
            wr8(b, owner + 0x2c, rng.pick(&[0, 0x20]));
            let settings: &[u8] = if rd8(b, obj + 3) == 0 { &[0, 1] } else { &[0, 1, 2, 3] };
            wr8(b, obj + 0x2f, rng.pick(settings));
            wr8(b, A5 + 0x36, rng.pick(&[0, 0, 1]));
        }
        // attack effects: attacker at +0x5c (action +0x49, step +6, type, class), setting
        11 => {
            wr16(b, obj + 0x5c, owner as u16);
            wr8(b, owner + 0x49, rng.pick(&[0x70, 0x90, 0x94, 0xa8, 0xa8, 0x10]));
            wr8(b, owner + 4, rng.pick(&[2, 4, 4, 6]));
            wr8(b, owner + 6, (rng.next() % 9) as u8);
            wr8(b, owner + 2, (rng.next() % 0x20) as u8);
            wr8(b, owner + 0x2c, rng.pick(&[0, 0x20]));
            wr8(b, owner + 0x28, rng.pick(&[2, 2, 4]));
            wr8(b, A5 + 0x6f, rng.next() as u8);
            let modes: &[u8] = match rd8(b, obj + 3) {
                8 | 9 => &[0, 0, 1, 2, 3, 4],
                _ => &[0, 1],
            };
            wr8(b, obj + 0x2f, rng.pick(modes));
            let action = rd8(b, owner + 0x49);
            wr8(b, obj + 0x42, rng.pick(&[0x70, 0x94, action]));
            wr8(b, obj + 0x5, rng.pick(&[0, 0, 2]));
        }
        // followed object at +0x5c (sometimes none), setting 0 or 1
        43 => {
            wr16(b, obj + 0x5c, if rng.one_in(10) { 0 } else { owner as u16 });
            wr8(b, obj + 0x2f, rng.pick(&[0, 0, 1]));
            wr16(b, obj + 0x42, (rng.next() % 0x28) as u16);
        }
        _ => {}
    }
}

/// Perturbation at each step (owner, players, scene).
fn perturbs(b: &mut Cps2Bus, obj: u32, owner: u32, rng: &mut Rng) {
    if rng.one_in(4) {
        wr8(b, owner + 4, rng.pick(&[0, 2, 4, 6]));
        wr8(b, owner + 0x3d, 2 * (rng.next() % 3) as u8);
    }
    if rng.one_in(6) {
        wr8(b, obj + 0x33, rng.pick(&[0, 1]));
    }
    if rng.one_in(8) {
        let j = PLAYERS + 0x200 * (rng.next() % 4);
        wr8(b, j, rng.pick(&[0, 1, 1, 2]));
        wr16(b, j + 0x62, rng.pick(&[0, 0x10, 0x40]));
        let (x, y) = (rd16(b, obj + 8), rd16(b, obj + 0x10));
        wr16(b, j + 8, x.wrapping_add((rng.next() % 0x40) as u16).wrapping_sub(0x20));
        wr16(b, j + 0x10, y.wrapping_add((rng.next() % 0x20) as u16).wrapping_sub(0x10));
    }
    // scene step and fade (fireworks)
    if rng.one_in(10) {
        wr8(b, A5 + 0x2e, rng.pick(&[0, 0x12, 0xc]));
        wr8(b, A5 + 0x48f, rng.pick(&[0, 1]));
    }
    // compteur d'images
    wr16(b, A5 + 0x6e, rng.next() as u16);
    // jeu en pause (rare)
    wr8(b, A5 + 0x3d, rng.one_in(20) as u8);
}

fn main() {
    std::thread::Builder::new().stack_size(1 << 30).spawn(principal).unwrap().join().unwrap();
}

fn principal() {
    let a: Vec<String> = std::env::args().collect();
    let (Some(roms), Some(eeprom), Some(scenario), Some(warmup), Some(types)) = (a.get(1), a.get(2), a.get(3), a.get(4), a.get(5)) else {
        eprintln!("usage: fuzz_effct <roms> <eeprom> <scenario> <warm-up frames> <types>");
        std::process::exit(2);
    };
    let env = |k: &str, d: u32| std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d);
    let variants: Vec<u32> = std::env::var("FUZZ_VARIANTES")
        .map(|v| v.split(',').map(|x| x.parse().unwrap()).collect())
        .unwrap_or_else(|_| (0..16).collect());
    let rounds = env("FUZZ_TOURS", 24);
    let step = env("FUZZ_PAS", 60);
    let types: Vec<u32> = types.split(',').filter(|x| !x.is_empty()).map(|x| x.parse().unwrap()).collect();
    let ee = std::fs::read(eeprom).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(roms), ee.as_deref()).expect("roms");
    m.set_overclock(16.0);
    ddsom_game::install(&mut m);
    let script = ddsom_game::scenario::Scenario::load(Path::new(scenario)).expect("scenario");
    for f in 0..warmup.parse::<usize>().unwrap() {
        script.apply(f, &mut m);
        m.run_frame();
    }
    let clean = std::env::var("FUZZ_PROPRE").is_ok();
    let mut clean_gaps = 0u64;
    ddsom_game::set_check(!clean);
    std::panic::set_hook(Box::new(|_| {}));
    let (c0, b0) = (m.cpu.clone(), m.bus.clone());
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15 ^ (env("FUZZ_GRAINE", 1) as u64).wrapping_mul(0x1000_0000_01b3));
    let mut table = Vec::new();
    {
        let mut b = b0.clone();
        for k in 0..92 {
            table.push((b.fetch16(0x39ce0 + 4 * k) as u32) << 16 | b.fetch16(0x39ce2 + 4 * k) as u32);
        }
    }
    let mut calls = 0u64;
    let mut abandons = 0u64;
    // creators (FUZZ_CREATEURS=address,...): called from registers drawn at random, A0 on a
    // player, an enemy or an effect, A1 in work RAM; the EFFCT pool more or less full
    let creators: Vec<u32> = std::env::var("FUZZ_CREATEURS")
        .map(|v| v.split(',').map(|x| u32::from_str_radix(x, 16).unwrap()).collect())
        .unwrap_or_default();
    for &entry in &creators {
        for _ in 0..rounds * 50 {
            let (mut c, mut b) = (c0.clone(), b0.clone());
            b.irq_horizon = u64::MAX;
            for r in 0..8 {
                c.d[r] = if rng.one_in(2) { rng.next() & 0xff } else { rng.next() << 8 | rng.next() & 0xff };
            }
            c.d[0] = rng.next() & 0xf;
            c.d[1] = rng.next() & 0xf;
            let (e, n) = (EFFCT.slot(rng.next() % EFFCT.slots).0, ENEMY.slot(rng.next() % ENEMY.slots).0);
            c.a[0] = rng.pick(&[PLAYERS, PLAYERS + 0x200, e, n]);
            c.a[1] = 0xff_9000 + 2 * (rng.next() % 0x800);
            c.a[2] = rng.next();
            c.a[5] = A5;
            wr16(&mut b, A5 + EFFCT.free, rng.pick(&[0, 1, 2, 5, 19]));
            calls += 1;
            let orig = coverage(&c, &b, entry, &(0..0), &mut BTreeSet::new());
            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| ddsom_game::call(&mut c, &mut b, entry))).is_err() {
                abandons += 1;
                continue;
            }
            if let (true, Some(orig)) = (clean, orig) {
                let d = compare(&orig, &c, &b);
                if !d.is_empty() {
                    clean_gaps += 1;
                    if clean_gaps <= 5 {
                        println!("  mismatch {entry:06x} : {}", d.join(", "));
                    }
                }
            }
        }
    }
    for &t in &types {
        let entry = table[t as usize];
        let range = match std::env::var("FUZZ_PLAGE") {
            Ok(p) => {
                let (d, f) = p.split_once('-').unwrap();
                u32::from_str_radix(d, 16).unwrap()..u32::from_str_radix(f, 16).unwrap()
            }
            Err(_) => entry..table.iter().copied().filter(|&e| e > entry).min().unwrap_or(entry + 0x400),
        };
        let mut seen = BTreeSet::new();
        let mut gap_types = 0u64;
        for &v in &variants {
            for tour in 0..rounds {
                let (mut c, mut b) = (c0.clone(), b0.clone());
                b.irq_horizon = u64::MAX;
                let mut owners: Vec<u32> = ENEMY.iter().filter(|o| rd8(&mut b, o.0) != 0).map(|o| o.0).collect();
                owners.extend((0..4).map(|j| PLAYERS + 0x200 * j));
                let obj = EFFCT.slot(rng.next() % EFFCT.slots).0;
                for off in (0..EFFCT.stride).step_by(2) {
                    wr16(&mut b, obj + off, 0);
                }
                let owner = rng.pick(&owners);
                let birth = tour % 2 == 0;
                wr8(&mut b, obj, 1);
                wr8(&mut b, obj + 2, t as u8);
                wr8(&mut b, obj + 3, v as u8);
                // from time to time, the release state (4) or the one set by the off-screen removal (6)
                let state = if birth { 0 } else if tour % 8 == 3 { 4 } else if tour % 8 == 7 { 6 } else { 2 };
                wr8(&mut b, obj + 4, state);
                wr8(&mut b, obj + 5, if birth { 0 } else { rng.pick(sub_states(t, v)) });
                wr8(&mut b, obj + 0x28, EFFCT.class);
                for off in [0x46u32, 0x50, 0x54, 0x5a, 0x5c, 0x1e] {
                    wr16(&mut b, obj + off, owner as u16);
                }
                for off in [0x8u32, 0xc, 0x10] {
                    let w = rd16(&mut b, owner + off);
                    wr16(&mut b, obj + off, w);
                }
                for off in [0x2fu32, 0x30, 0x31, 0x42, 0x43, 0x58, 0x59] {
                    if rng.one_in(2) {
                        wr8(&mut b, obj + off, rng.pick(&[0, 1, 2, 3, 4, 8]));
                    }
                }
                wr8(&mut b, obj + 0x2c, rng.pick(&[0, 0x20]));
                wr16(&mut b, A5 + EFFCT.free, (EFFCT.slots / 2) as u16);
                prepare(t, &mut b, obj, owner, &mut rng);
                for k in 0..step {
                    if rd8(&mut b, obj) == 0 || rd8(&mut b, obj + 2) != t as u8 {
                        break;
                    }
                    if k > 0 {
                        perturbs(&mut b, obj, owner, &mut rng);
                    }
                    c.a[0] = obj;
                    c.a[5] = A5;
                    c.d[7] = k;
                    c.d[0] = 4 * t;
                    c.a[1] = entry;
                    if std::env::var_os("FUZZ_DEBUG").is_some() {
                        let o: Vec<String> = (0..0x60).step_by(2).map(|i| format!("{:04x}", rd16(&mut b, obj + i))).collect();
                        println!("step {k}: {}", o.join(" "));
                    }
                    let state_before = format!("{:02x}/{:02x}/{:02x}", rd8(&mut b, obj + 4), rd8(&mut b, obj + 5), rd8(&mut b, obj + 6));
                    let orig = coverage(&c, &b, entry, &range, &mut seen);
                    calls += 1;
                    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| ddsom_game::call(&mut c, &mut b, entry)));
                    if r.is_err() {
                        abandons += 1;
                        break;
                    }
                    if let (true, Some(orig)) = (clean, orig) {
                        let d = compare(&orig, &c, &b);
                        if !d.is_empty() {
                            clean_gaps += 1;
                            gap_types += 1;
                            if gap_types <= 3 {
                                println!("  mismatch type {t} variant {v} step {k} state {state_before}: {}", d.join(", "));
                            }
                            // restart from the original's state
                            (c, b) = orig;
                        }
                    }
                }
            }
        }
        let mut b = b0.clone();
        let all = reachable(&mut b, entry, range.clone());
        let missing: Vec<u32> = all.difference(&seen).copied().collect();
        let mut ranges: Vec<(u32, u32)> = Vec::new();
        for &p in &missing {
            match ranges.last_mut() {
                Some(r) if p - r.1 <= 0x10 => r.1 = p,
                _ => ranges.push((p, p)),
            }
        }
        println!(
            "TYPE {t} {entry:06x} : {gap_types} mismatches, {} reachable instructions, {} executed by the original, unseen: {}",
            all.len(),
            all.intersection(&seen).count(),
            ranges.iter().map(|(a, b)| format!("{a:06x}-{b:06x}")).collect::<Vec<_>>().join(" ")
        );
    }
    let mut gaps = clean_gaps;
    if clean {
        println!("  clean comparison: {clean_gaps} mismatches");
    }
    for (name, st) in ddsom_game::report() {
        if st.calls == 0 {
            continue;
        }
        gaps += st.mismatches;
        if st.mismatches > 0 || name.starts_with("effct") || name.starts_with("create") || name.starts_with("block") {
            println!("  {name:28} {:>7} calls {:>5} mismatches{}", st.calls, st.mismatches, st.first.map(|f| format!(" — {f}")).unwrap_or_default());
        }
    }
    println!("{calls} calls, {abandons} abandoned trajectories");
    println!("{}", if gaps == 0 { "FUZZ VALID: no mismatch" } else { "FUZZ FAILED" });
}
