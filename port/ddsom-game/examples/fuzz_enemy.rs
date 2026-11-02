//! Variant of fuzz.rs for the ENEMY batch (ENEMY table, valid states per type, fabricated masters and targets).
//! Differential validation on random states: each native routine of a group is called
//! in control mode from randomly drawn states; the original runs alongside and the two are
//! compared (memory, contract registers, duration if declared exact). Exercises the paths
//! that the scenarios do not reach.
//!
//!   fuzz <group> <roms> <eeprom> <scenario> <warm-up frames> <rounds>
//!
//! Groups: sound (0x68c, 0xe0836-0xee000), u1 (RNG and event queues 0x19a80-0x1a000),
//! c1 (coin slots 0x13c00-0x14e50), m2 (EEPROM and configuration 0x2d300-0x2ede0),
//! video (video primitives, palettes and fades: natives `video_*` and `palette_*`),
//! camera (scenery copy by cameras 1 and 2, 0x2f81c 0x2f86e 0x328a8, and parallax scenery 0x33178...).

use std::path::Path;

use ddsom_game::mem::{wr16, wr32, wr8, A5};

#[path = "fuzz_enemy/lot3.rs"]
mod lot3;

struct Rng(u64);

/// A group of natives: its random state and the range of their addresses.
type Group = (fn(&mut Cpu, &mut Bus, &mut Rng), std::ops::Range<u32>);

impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    /// True with probability 1/n.
    fn one_chance_in(&mut self, n: u32) -> bool {
        self.next().is_multiple_of(n)
    }
}

type Cpu = m68k::Cpu;
type Bus = cps2::Cps2Bus;

/// Arbitrary registers; A0 on a player record.
fn regs(c: &mut Cpu, b: &mut Bus, rng: &mut Rng) {
    for r in 0..8 {
        c.d[r] = if rng.next() & 3 == 0 { rng.next() & 0xff } else { rng.next() };
    }
    for r in 1..5 {
        c.a[r] = 0xff0000 | (rng.next() & 0xffff);
    }
    let obj = 0xff85de + 0x200 * (rng.next() % 4);
    for o in [2u32, 3, 0x7e] {
        wr8(b, obj + o, rng.next() as u8);
    }
    wr16(b, obj + 0x48, rng.next() as u16);
    wr16(b, obj + 8, rng.next() as u16 & 0x7ff);
    c.a[0] = obj;
}

fn rng_sound(c: &mut Cpu, b: &mut Bus, rng: &mut Rng) {
    wr8(b, A5 + 0x27, rng.next() as u8 & 1);
    wr8(b, A5 + 0x30, rng.next() as u8 & 1);
    wr8(b, A5 + 0x28, rng.next() as u8 & 1);
    wr8(b, A5 + 0x36, rng.next() as u8 & 1);
    wr16(b, A5 + 0x7fcc, rng.one_chance_in(3) as u16);
    wr16(b, A5 + 0x7fc2, if rng.one_chance_in(2) { 0 } else { rng.next() as u16 & 7 });
    wr16(b, A5 + 0x7fc4, rng.next() as u16);
    wr8(b, 0xff2a1e, rng.one_chance_in(4) as u8);
    wr16(b, 0xff2960, if rng.next() & 1 == 0 { 0 } else { 0xffff });
    wr16(b, A5 + 0x5a6, rng.next() as u16 & 0x3ff);
    wr16(b, A5 + 0x5ae, rng.next() as u16 & 0x3ff);
    // queue: arbitrary heads, sometimes full or empty
    let read = (rng.next() as u16 & 0x3f) * 0x10;
    let write = match rng.next() % 4 {
        0 => read,
        1 => read.wrapping_sub(0x10) & 0x3f0,
        _ => (rng.next() as u16 & 0x3f) * 0x10,
    };
    wr16(b, 0xff295c, read);
    wr16(b, 0xff295e, write);
    wr8(b, 0x61801f, if rng.next() & 1 == 0 { 0xff } else { 0 });
    regs(c, b, rng);
}

/// A BCD counter: sometimes close to the ceiling, sometimes anything.
fn bcd(rng: &mut Rng) -> u32 {
    match rng.next() % 4 {
        0 => 0x0999_9990 + rng.next() % 10,
        1 => rng.next(),
        _ => (0..8).fold(0, |v, _| (v << 4) | (rng.next() % 10)),
    }
}

fn ring(b: &mut Bus, rng: &mut Rng, write: u32, cases: u32, read: bool) {
    wr8(b, A5 + write, (rng.next() as u8 & 0x3f) * 2);
    if read {
        wr8(b, A5 + write + 1, (rng.next() as u8 & 0x3f) * 2);
    }
    for i in 0..64 {
        let v = if rng.next().is_multiple_of(3) { rng.next() as u16 & 0x37f } else { 0xffff };
        wr16(b, A5 + cases + 2 * i, v);
    }
}

fn rng_u1(c: &mut Cpu, b: &mut Bus, rng: &mut Rng) {
    wr16(b, A5 + 0x1a, rng.next() as u16);
    ring(b, rng, 0x108, 0x10a, true);
    ring(b, rng, 0x18a, 0x18c, true);
    // pop-ups: 8-byte entries, indices multiple of 8
    wr8(b, A5 + 0x50, rng.one_chance_in(4) as u8);
    wr8(b, A5 + 0x20c, (rng.next() as u8 & 0x1f) * 8);
    wr8(b, A5 + 0x20d, (rng.next() as u8 & 0x1f) * 8);
    for i in 0..32 {
        // plausible entries: type (high byte 0-3), code < 0x18, small parameters
        let plausible = ((rng.next() % 4) as u16 * 0x100) | (rng.next() % 0x18) as u16;
        let base = A5 + 0x20e + 8 * i;
        wr16(b, base, if rng.next().is_multiple_of(3) { plausible } else { 0xffff });
        wr16(b, base + 2, rng.next() as u16 % 0x100);
        wr16(b, base + 4, if rng.next() & 1 == 0 { 0 } else { rng.next() as u16 % 0x100 });
        wr16(b, base + 6, 0x80);
    }
    // current pop-up: remaining duration 0, 1, 2, 3, 4, 5 or long
    let duration = [0u16, 1, 2, 3, 4, 5, 0x60][rng.next() as usize % 7];
    wr16(b, 0xff9180, ((rng.next() % 4) as u16 * 0x100) | (rng.next() % 0x18) as u16);
    wr16(b, 0xff9182, rng.next() as u16 % 0x100);
    wr16(b, 0xff9184, rng.next() as u16 % 0x100);
    wr16(b, 0xff9186, duration);
    for j in 0..4 {
        wr32(b, 0xff85de + 0x200 * j + 0x110, bcd(rng));
        wr32(b, 0xff85de + 0x200 * j + 0x114, bcd(rng) & 0x000f_ffff);
    }
    regs(c, b, rng);
}

/// Coin slots: arbitrary settings, records and input lines.
fn rng_c1(c: &mut Cpu, b: &mut Bus, rng: &mut Rng) {
    wr16(b, A5 + 0x7fcc, rng.one_chance_in(8) as u16);
    wr8(b, A5 + 0x1e, 1 + rng.next() as u8 % 4);
    wr8(b, A5 + 0x1f, 1 + rng.next() as u8 % 4);
    wr8(b, A5 + 0x20, rng.one_chance_in(2) as u8);
    let coin_slots = [0u8, 1, 2, 3, 0xff, 7];
    wr8(b, A5 + 0x21, coin_slots[rng.next() as usize % 6]);
    wr8(b, A5 + 0x22, rng.one_chance_in(2) as u8);
    wr8(b, A5 + 0x25, rng.one_chance_in(4) as u8);
    wr8(b, A5 + 0x30, rng.one_chance_in(2) as u8);
    wr8(b, 0xff8ec4, rng.one_chance_in(2) as u8);
    for k in 0..4 {
        let base = A5 + 0x44e + 0x10 * k;
        let debounce = [0u8, 1, 2, 3, 49, 50, 51, 255];
        wr8(b, base, if rng.next() & 1 == 0 { debounce[rng.next() as usize % 8] } else { rng.next() as u8 });
        wr8(b, base + 1, rng.next() as u8 % 6);
        wr8(b, base + 2, [0u8, 4, 8, 9, 10, 12, 0x80, 255][rng.next() as usize % 8]);
        wr8(b, base + 3, rng.next() as u8);
        wr8(b, base + 7, rng.one_chance_in(2) as u8);
    }
    b.inputs.in2 = rng.next() as u16;
    regs(c, b, rng);
}

/// EEPROM and accounting: EEPROM contents, RAM copies, transfer parameters, counters.
fn rng_m2(c: &mut Cpu, b: &mut Bus, rng: &mut Rng) {
    let mut data = [0u16; 64];
    for d in data.iter_mut() {
        *d = rng.next() as u16;
    }
    if rng.next() & 1 == 0 {
        // the two copies of the configuration agree
        for i in 0..24 {
            data[24 + i] = data[i];
        }
    }
    b.eeprom = cps2::eeprom::Eeprom93c46::new(data);
    let words = [0u16, 1, 2, 5, 23, 47, 63][rng.next() as usize % 7];
    let start = 2 * (rng.next() as u16 % (64 - words));
    wr16(b, A5 - 0x616e, words);
    wr16(b, A5 - 0x616a, start);
    wr16(b, A5 - 0x616c, words);
    wr16(b, A5 - 0x6168, start);
    wr32(b, A5 - 0x6172, if rng.next() & 1 == 0 { 0xff1d8e } else { 0xff1e0e });
    for (i, &d) in data.iter().enumerate() {
        let v = if rng.next().is_multiple_of(4) { d ^ 1 } else { d };
        wr16(b, 0xff1d8e + 2 * i as u32, if rng.next().is_multiple_of(3) { rng.next() as u16 } else { v });
        wr16(b, 0xff1e0e + 2 * i as u32, rng.next() as u16);
    }
    for (value, image) in [(0x76u32, 0xffff_9deeu32), (0x7a, 0xffff_9df2), (0x7e, 0xffff_9df6), (0x7fd4, 0xffff_9dfa)] {
        let v = rng.next() % 1000;
        wr32(b, A5 + value, v);
        wr32(b, A5.wrapping_add(image), if rng.next() & 1 == 0 { v + 1 } else { rng.next() % 1000 });
    }
    wr8(b, A5 + 0x82, rng.one_chance_in(4) as u8);
    wr8(b, A5 + 0x25, rng.one_chance_in(2) as u8);
    wr8(b, A5 + 0x74, 0);
    wr16(b, A5 + 0x16, rng.next() as u16 & 0x8f88);
    wr8(b, A5 + 0x18, rng.next() as u8 & 6);
    wr16(b, 0x664000, rng.next() as u16);
    // configuration: variant, RAM copy (valid or not), default block in A6
    let variant = 2 * (rng.next() as u16 % 6);
    wr16(b, A5 + 0x70, variant);
    for i in 0..0x80u32 {
        let small = if rng.next().is_multiple_of(3) { rng.next() as u8 } else { (rng.next() % 20) as u8 };
        wr8(b, 0xff1d8e + i, small);
    }
    let block = 0x2d656 + b_prog16(b, 0x2d656 + variant as u32) as i16 as u32;
    c.a[6] = if rng.next() & 3 == 0 { block + 0x30 } else { block };
    regs(c, b, rng);
}

/// Randomly drawn routine (for states that depend on the entry being called).
static ENTRY: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Palette color: brightness often at the limits, components arbitrary or close to 0 / 0xf.
fn colour(rng: &mut Rng) -> u16 {
    let luma = match rng.next() % 4 {
        0 => 0,
        1 => 0xf000,
        _ => (rng.next() as u16 & 0xf) << 12,
    };
    let compo = |a: &mut Rng| match a.next() % 4 {
        0 => 0,
        1 => 0xf,
        _ => (a.next() & 0xf) as u16,
    };
    luma | compo(rng) << 8 | compo(rng) << 4 | compo(rng)
}

/// Video primitives, palettes and fades: arbitrary palettes, fade state, scene and cameras.
fn rng_video(c: &mut Cpu, b: &mut Bus, rng: &mut Rng) {
    let entry = ENTRY.load(std::sync::atomic::Ordering::Relaxed);
    for i in 0..0x800u32 {
        let v = if rng.next().is_multiple_of(5) { colour(rng) } else { colour(rng) & 0xf0ff | (i as u16 & 0xf00) };
        wr16(b, 0x91c000 + 2 * i, v);
    }
    // target palettes in RAM (0xff1190, 512 colours)
    for i in 0..512u32 {
        wr16(b, 0xff1190 + 2 * i, colour(rng));
    }
    wr8(b, A5 + 0x48e, rng.next() as u8 & 0xf);
    for k in 0..4u32 {
        let target = match rng.next() % 3 {
            0 => 0xff1190,
            1 => 0x253000 + 0x20 * (rng.next() % 0x1000),
            _ => 0x91c000 + 0x400 * k,
        };
        // 0x1cdd6 reads a word (color): same bytes, interpreted differently
        wr32(b, A5 + 0x492 + 4 * k, if entry == 0x1cdd6 { colour(rng) as u32 } else if entry == 0x158e || entry == 0x15c8 { 0x91c000 + 0x400 * k } else { target });
    }
    if entry == 0x15c8 {
        wr32(b, A5 + 0x492, 0x91c3a0);
    }
    wr8(b, A5 + 0x34, [2, 0x14, 0x2f, 0x42, 0x44, 0x45, 0x59, 0x5a, 0x3, 0x8][rng.next() as usize % 10]);
    if rng.next().is_multiple_of(3) {
        wr8(b, A5 + 0x34, rng.next() as u8 % 0x60);
    }
    wr8(b, A5 + 0x36, rng.one_chance_in(2) as u8);
    for off in [0x506u32, 0x50a, 0x556, 0x55a] {
        wr16(b, A5 + off, rng.next() as u16);
    }
    for i in 0..0x2000u32 {
        wr16(b, 0x914000 + 2 * i, rng.next() as u16);
    }
    wr16(b, A5 + 0x7fd0, if rng.next() & 1 == 0 { 0x400 } else { 0 });
    wr8(b, 0xff0eab, rng.one_chance_in(3) as u8);
    wr8(b, A5 + 0x18, rng.next() as u8);
    regs(c, b, rng);
    c.a[4] = A5 + 0x48e;
    // valid scene / set numbers (the ROM tables stop at scene 0x5a)
    c.d[0] = if rng.next().is_multiple_of(3) { rng.next() % 0x5b } else { rng.next() % 12 };
    c.d[1] = if rng.next() & 1 == 0 { 0x1000 } else { 0xffff_f000 };
    if entry == 0x1a60a {
        c.d[0] %= 3; // the page 1 table has only three entries
    }
    let page = 0x91c000 + 0x400 * (rng.next() % 4);
    c.a[0] = page;
    c.a[1] = match entry {
        0x1602 | 0x164e => page,
        0x1cfc8 => [0xff1190, 0x253000 + 0x20 * (rng.next() % 0x1000)][rng.next() as usize % 2],
        _ => 0x916000 + 0x400 * (rng.next() % 8),
    };
    if entry == 0x164e {
        c.a[1] = 0x91c3a0;
    }
    if matches!(entry, 0x24a8) {
        let obj = 0xff85de + 0x200 * (rng.next() % 4);
        wr16(b, obj + 8, rng.next() as u16);
        wr16(b, obj + 0x10, rng.next() as u16);
        c.a[0] = obj;
    }
    if entry == 0x1d042 {
        c.d[1] = colour(rng) as u32;
    }
    if matches!(entry, 0x255a | 0x25e8) {
        c.d[0] = if rng.next().is_multiple_of(3) { rng.next() } else { rng.next() % 130 };
        c.a[1] = 0x912000 + 0x80 * (rng.next() % 40) + 4 * (rng.next() % 30);
    }
}

/// Scroll cameras: target and drawn positions, limits and flags arbitrary but plausible
/// (offsets range from a simple shift to several steps, in all four directions).
fn rng_camera(c: &mut Cpu, b: &mut Bus, rng: &mut Rng) {
    let entry = ENTRY.load(std::sync::atomic::Ordering::Relaxed);
    if matches!(entry, 0x33178 | 0x33196 | 0x331e8 | 0x3326c) {
        // parallax scenery: current and previous scroll, object position
        for off in [0x506u32, 0x50a, 0x524, 0x528] {
            wr32(b, A5 + off, rng.next() << 8 ^ rng.next());
        }
        regs(c, b, rng);
        for off in [0x08u32, 0x0c, 0x10] {
            wr32(b, c.a[0] + off, rng.next() << 8 ^ rng.next());
        }
        return;
    }
    let cam = if entry == 0x328a8 { A5 + 0x54e } else { A5 + 0x4fe };
    let step: i32 = if entry == 0x328a8 { 0x20 } else { 0x10 };
    let gap = |a: &mut Rng, range_: i32| -> i32 {
        match a.next() % 4 {
            0 => 0,
            1 => (a.next() as i32 % (2 * step)) - step,
            _ => (a.next() as i32 % (2 * range_)) - range_,
        }
    };
    // positions that stay inside the scenery map (beyond it the ROM has no more table: meaningless reads)
    let drawn_x = 0x100 + (rng.next() % 0xb00) as i32;
    let drawn_y = 0x40 + (rng.next() % 0x260) as i32;
    let x = drawn_x + gap(rng, 4 * step);
    let y = drawn_y + gap(rng, 3 * step);
    wr16(b, cam + 0x10, drawn_x as u16);
    wr16(b, cam + 0x14, drawn_y as u16);
    wr16(b, cam + 0x08, x as u16);
    wr16(b, cam + 0x0c, y as u16);
    wr16(b, cam + 0x20, if rng.next().is_multiple_of(4) { 4 } else { 0 });
    wr16(b, cam + 0x2e, (x - rng.next() as i32 % 0x100) as u16);
    wr16(b, cam + 0x34, (x + (rng.next() as i32 % 0x200) - 0x40) as u16);
    wr8(b, cam + 0x3d, rng.one_chance_in(3) as u8);
    wr8(b, cam + 0x3e, rng.one_chance_in(8) as u8);
    wr8(b, A5 + 0x37, rng.one_chance_in(4) as u8);
    regs(c, b, rng);
    c.a[0] = cam;
}

fn b_prog16(b: &mut Bus, a: u32) -> u16 {
    m68k::Bus::read_prog16(b, a)
}

/// Object families (ESHL, SET, SOBJ): for each handler, complete trajectories. A
/// new object of the type is created in state 0 (owner and variant drawn at random), then the
/// handler is called frame after frame, with random perturbations of the inputs that the
/// collision engine or the creator modify (hit flags, owner, positions).
/// States are thus reached by the code itself. Each call is compared with the original.
/// `DDSOM_ENTREE=<hex address>` restricts to one handler.
const DDSOM_TRACE_LEN: u32 = 200000;
/// Starting states (`+4`, `+5`, `+6`) accepted by the jump table of each ESHL type 15..39
/// (beyond that, the original leaves the code): lists of possible values per byte.
/// Valid states (`+4`, `+5`, `+6`) of an ENEMY type: those covered by the original's jump tables.
fn enemy_states(kind: u8, variant: u8) -> &'static [(&'static [u8], &'static [u8], &'static [u8])] {
    const Z: &[u8] = &[0];
    match kind {
        // Riyakaa: sub-states and sub-sub-states accepted by the jump tables (the arrival depends
        // on the variant)
        16 => match variant {
            8 | 10 => &[
                (&[0], Z, Z),
                (&[2], &[0], &[0, 2, 4, 6]),
                (&[2], &[2], &[0, 2, 4]),
                (&[2], &[4], &[0, 2, 4, 6, 8, 10, 12, 14]),
                (&[2], &[6], &[0, 2, 4, 6, 8, 10]),
                (&[2], &[8], &[0, 2, 4, 6]),
                (&[2], &[10], &[0, 2]),
                (&[4], &[0], &[0]),
                (&[4], &[2], &[0, 2]),
                (&[4], &[4], &[0, 2, 4]),
                (&[4], &[6], &[0]),
                (&[4], &[8], &[0, 2, 4]),
                (&[6], Z, Z),
            ],
            _ => &[
                (&[0], Z, Z),
                (&[2], &[0], &[0, 2, 4]),
                (&[2], &[2], &[0, 2, 4]),
                (&[2], &[4], &[0, 2, 4, 6, 8, 10, 12, 14]),
                (&[2], &[6], &[0, 2, 4, 6, 8, 10]),
                (&[2], &[8], &[0, 2, 4, 6]),
                (&[2], &[10], &[0, 2]),
                (&[4], &[0], &[0]),
                (&[4], &[2], &[0, 2]),
                (&[4], &[4], &[0, 2, 4]),
                (&[4], &[6], &[0]),
                (&[4], &[8], &[0, 2, 4]),
                (&[6], Z, Z),
            ],
        },
        22..=24 => &[(&[0, 2, 4, 6], Z, Z)],
        26 => &[(&[0, 2, 4, 6], Z, Z)],
        7 => &[
            (&[0], Z, Z),
            (&[2], &[0, 6, 10], &[0, 2]),
            (&[2], &[2], &[0, 2, 4]),
            (&[2], &[4, 8], &[0, 2, 4, 6, 8]),
            (&[4, 6], Z, Z),
        ],
        28 => &[(&[0], Z, Z), (&[2], &[0, 2, 4, 6], Z), (&[4, 6], Z, Z)],
        20 => &[(&[0], Z, Z), (&[2], &[0, 2], Z), (&[4], &[0], &[0, 2]), (&[4], &[2], &[0, 2, 4, 6, 8, 10]), (&[6], Z, Z)],
        // DarkWarrior 1: arms linked to a host, step `+5` depends on the host's state
        25 => &[(&[0], Z, Z), (&[2], &[0, 2, 4, 6], Z), (&[4], Z, Z), (&[6], Z, Z)],
        // Harpy: states accepted by the original's jump tables
        21 => &[
            (&[0], Z, Z),
            (&[2], &[0], &[0, 2, 4, 6]),
            (&[2], &[2], &[0, 2, 4, 6]),
            (&[2], &[4], Z),
            (&[2], &[6], Z),
            (&[2], &[8], &[0, 2, 4]),
            (&[4], &[0], &[0, 2, 4]),
            (&[4], &[2], &[0, 2, 4]),
            (&[4], &[4], &[0, 2]),
            (&[6], Z, Z),
        ],
        // Owlbear: same framework as the Riyakaa
        4 => &[
            (&[0], Z, Z),
            (&[2], &[0], Z),
            (&[2], &[2], &[0, 2, 4, 6, 8]),
            (&[2], &[4], &[0, 2, 4, 6, 8, 10, 12, 14, 16, 18]),
            (&[2], &[6], &[0, 2, 4]),
            (&[2], &[8], &[0, 2, 4, 6]),
            (&[2], &[10], Z),
            (&[4], &[0], Z),
            (&[4], &[2], &[0, 2, 4]),
            (&[4], &[4], &[0, 2, 4]),
            (&[4], &[6], Z),
            (&[4], &[8], &[0, 2, 4]),
            (&[6], Z, Z),
        ],
        // Displacer Beast: one behavior group (+5) per active state, its steps (+6)
        19 => &[
            (&[0], Z, Z),
            (&[2], &[0], &[0, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 22]),
            (&[2], &[2], &[0, 2, 4, 6]),
            (&[2], &[4], &[0, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 22, 24, 26, 28, 30, 32, 34, 36]),
            (&[2], &[6], &[0, 2, 4, 6, 8, 10, 12]),
            (&[2], &[8], &[0, 2, 4, 6]),
            (&[2], &[10], &[0, 2, 4]),
            (&[2], &[12], &[0, 2, 4, 6, 8]),
            (&[4], &[0], Z),
            (&[4], &[2], &[0, 2]),
            (&[4], &[4], &[0, 2, 4, 6]),
            (&[4], &[6], Z),
            (&[4], &[8], &[0, 2, 4, 6, 8, 10, 12, 14]),
            (&[6], Z, Z),
        ],
        _ => &[(&[0, 2, 4, 6], &[0, 2, 4], &[0, 2, 4])],
    }
}

/// Number of variants that the game gives to the objects of handler `entry`, where the default draw (`DDSOM_VARIANTES`)
/// goes beyond them: the original then jumps outside its variant table (out of contract, not a mismatch).
/// * ESHL 15 (0x9fe0a): created with variants 0 and 1 by the parts of a composite enemy (0x9fa3c, 0x9face, 0x9fc14);
/// * ESHL 45 (0xa3b88): flames of boss 16, variants 0 and 1 (0xa3b4a, 0xa3b66);
/// * ENEMY 28 (0x79300): the Nagpa, variants 0 and 1 (the native ignores the others);
/// * SET 0xcc25a (scene controller creators): one-entry variant table (`jmp (PC, variant * 4)`: 0 and 1 land on
///   the creation).
fn variants_of(entry: u32) -> Option<u32> {
    match entry {
        0x9fe0a | 0xa3b88 | 0x79300 | 0xcc25a => Some(2),
        _ => None,
    }
}

/// Valid `+7` step of the Owlbear (type 4) for a state `(+4, +5, +6)`.
fn step_owlbear(rng: &mut Rng, s4: u8, s5: u8, s6: u8) -> u8 {
    let n = match (s4, s5, s6) {
        (2, 0, _) => 4,
        (2, 2, _) => 2,
        (2, 4, 0) => 6,
        (2, 4, 2 | 4 | 12) => 12,
        (2, 4, 6) => 18,
        (2, 4, 8) => 6,
        (2, 4, 10) => 8,
        (2, 4, 14) => 16,
        (2, 4, 16 | 18) => 10,
        (2, 6, 2) => 12,
        (2, 6, 4) => 6,
        (2, 8, _) => 2,
        (2, 10, _) => 4,
        _ => 0,
    };
    2 * (rng.next() % (n / 2 + 1)) as u8
}

/// Fields of the arm (type 25) and its host set to values the original's tables accept:
/// host state `+0xa2` (0 to 8), valid sub-state `+5` for the chosen path, close positions.
fn prep_warrior(b: &mut cps2::Cps2Bus, rng: &mut Rng, obj: u32) {
    use ddsom_game::mem::{rd16, rd8, wr16, wr32, wr8};
    let host = rd16(b, obj + 0x46) as u32 | 0xff0000;
    let hs = (rng.next() % 9) as u8;
    wr8(b, host + 0xa2, hs);
    wr8(b, host + 0x5, 2 * (rng.next() % 4) as u8);
    wr8(b, host + 0x2, if rng.next().is_multiple_of(2) { 0xf } else { 0x19 });
    wr8(b, host + 0x42, (rng.next() % 2) as u8);
    for off in [0xa8u32, 0xaa, 0xac] {
        wr16(b, host + off, (0x100 + rng.next() % 0x80) as u16);
    }
    wr16(b, host + 0xa6, (rng.next() % 3) as u16);
    wr16(b, host + 0x62, (rng.next() % 40) as u16);
    if !rng.next().is_multiple_of(4) {
        wr8(b, obj + 0xa2, hs);
    }
    let variant = rd8(b, obj + 3);
    let swings = (hs == 4 && variant == 0) || (hs == 5 && variant != 0);
    let drifts = (hs == 4 && variant != 0) || (hs == 5 && variant == 0) || ((hs == 6 && variant != 0) || (hs == 7 && variant == 0));
    let rises = matches!(hs, 6..=8) && !drifts;
    let sub = if swings || rises { 2 * (rng.next() % 4) as u8 } else if drifts { 2 * (rng.next() % 2) as u8 } else { (rng.next() % 3) as u8 * 2 };
    wr8(b, obj + 5, sub);
    wr16(b, obj + 0xae, (rng.next() % 6) as u16);
    for off in [0xb4u32, 0xb6, 0xb8] {
        wr16(b, obj + off, (rng.next() % 0x40) as u16);
    }
    wr32(b, obj + 0xb0, if rng.next().is_multiple_of(2) { 0x0200_0000 } else { 0 });
    wr16(b, obj + 0xc, (rng.next() % 3) as u16);
    wr16(b, obj + 0x16, (rng.next() % 0x200) as u16);
    wr16(b, obj + 0x1a, 0xffc8);
    wr8(b, obj + 0xa4, (rng.next() % 2) as u8);
    wr16(b, obj + 0x60, (rng.next() % 40) as u16);
    wr8(b, obj + 0x34, 0);
}

/// Harpy (type 21) fields set to values its tables accept.
fn prep_harpy(b: &mut cps2::Cps2Bus, rng: &mut Rng, obj: u32) {
    use ddsom_game::mem::{wr16, wr32, wr8};
    wr8(b, obj + 0x43, (rng.next() % 2) as u8);
    wr8(b, obj + 0x42, [0u8, 4, 2, 6][rng.next() as usize % 4]);
    for off in [0xa0u32, 0xa1, 0xa2, 0xa3, 0xa4, 0x77] {
        wr8(b, obj + off, (rng.next() % 5) as u8);
    }
    wr8(b, obj + 0xa2, (rng.next() % 2) as u8);
    // the flight angle stays below 0x40 (the variant dispatch reads D0 left by the speed table)
    wr8(b, obj + 0x2d, (rng.next() % 0x20) as u8);
    wr16(b, obj + 0xb0, (rng.next() % 0x200) as u16);
    wr8(b, obj + 0xb1, [0u8, 2, 6][rng.next() as usize % 3]);
    wr8(b, obj + 0xa5, (rng.next() % 2) as u8);
    wr16(b, obj + 0xae, [0x100u16, 0x280, 0xff00][rng.next() as usize % 3]);
    // the companion is an existing object; the shot (+0xa6) another one
    wr16(b, obj + 0xaa, 0x85de + 0x200 * (rng.next() % 4) as u16);
    wr32(b, obj + 0xa6, 0xff0000 | (0x85de + 0x200 * (rng.next() % 4)));
    wr8(b, obj + 0x49, 4 * (rng.next() % 43) as u8);
}

/// Owlbear fields set to values its tables accept.
fn prep_owlbear(b: &mut cps2::Cps2Bus, rng: &mut Rng, obj: u32) {
    use ddsom_game::mem::{rd16, rd8, wr16, wr32, wr8};
    let (s4, s5, s6) = (rd8(b, obj + 4), rd8(b, obj + 5), rd8(b, obj + 6));
    if s4 == 2 {
        let step = step_owlbear(rng, s4, s5, s6);
        wr8(b, obj + 7, step);
    }
    wr8(b, obj + 0x49, 4 * (rng.next() % 43) as u8);
    wr8(b, obj + 0x4a, 0x20 * (rng.next() % 2) as u8);
    // often at ground level, falling back: the jump landings
    if rng.next().is_multiple_of(2) {
        wr16(b, obj + 0xc, (rng.next() % 3) as u16);
        wr16(b, obj + 0x18, 0xff00);
        wr16(b, obj + 0x1a, 0xffc8);
    }
    for off in [0x6b, 0x6e, 0x71, 0x77, 0xa0, 0xac, 0xad, 0xaa] {
        wr8(b, obj + off, (rng.next() % 5) as u8);
    }
    wr16(b, obj + 0x84, (rng.next() % 4) as u16);
    wr8(b, obj + 0xa5, (rng.next() % 2) as u8);
    // wanted and current animations: often identical (repeated hits)
    const ANIMS: [u8; 14] = [0, 2, 4, 6, 8, 0xa, 0xc, 0xe, 0x15, 0x16, 0x1a, 0x1c, 0x7f, 0xff];
    let anim = ANIMS[rng.next() as usize % ANIMS.len()];
    wr8(b, obj + 0xa1, anim);
    wr8(b, obj + 0xa2, if rng.next().is_multiple_of(2) { anim } else { ANIMS[rng.next() as usize % ANIMS.len()] });
    // state to resume after a U-turn
    let resumes: [u32; 3] = [0x0202_0000, 0x0204_0000, 0x0206_0000];
    wr32(b, obj + 0xa6, resumes[rng.next() as usize % 3]);
    wr8(b, obj + 0xa4, 2 * (rng.next() % 3) as u8);
    wr16(b, obj + 0x50, 0x85de + 0x200 * (rng.next() % 4) as u16);
    // a possible host: orientation
    let host = rd16(b, obj + 0x46) as u32 | 0xff0000;
    wr8(b, host + 0x2c, 0x20 * (rng.next() % 2) as u8);
}

/// Valid `+7` step of the Displacer Beast (type 19) for a state `(+4, +5, +6)`: those the tables
/// de sauts de l'original acceptent.
fn step_dbeast(rng: &mut Rng, s4: u8, s5: u8, s6: u8) -> u8 {
    let n = match (s4, s5, s6) {
        (2, 0, 4 | 10 | 12 | 14 | 16 | 20 | 22) => 4,
        (2, 0, 6 | 18) | (2, 0, 0) => 2,
        (2, 2, _) => 2,
        (2, 4, 4 | 8) => 8,
        (2, 4, 10 | 18 | 20) => 10,
        (2, 4, 12) => 12,
        (2, 4, 16 | 30 | 36) => 14,
        (2, 4, _) => 6,
        (2, 6, 0) => 8,
        (2, 6, 2) => 6,
        (2, 6, 4 | 6 | 10) => 10,
        (2, 6, 8) => 12,
        (2, 6, 12) => 2,
        (2, 8, 0 | 6) => 4,
        (2, 8, _) => 8,
        _ => 0,
    };
    2 * (rng.next() % (n / 2 + 1)) as u8
}

/// AI fields of an enemy with complex behavior (type 16) set to values its tables accept:
/// short counters, sub-state to resume, fallback state, reaction to the last hit, target = a player.
fn prep_enemy_ai(b: &mut cps2::Cps2Bus, rng: &mut Rng, obj: u32) {
    use ddsom_game::mem::{rd16, rd8, wr16, wr32, wr8};
    for off in [0x6b, 0x6e, 0x71, 0x77, 0xa2, 0x90] {
        wr8(b, obj + off, (rng.next() % 5) as u8);
    }
    // counter of remaining paw hits: 1 at most, and only in sub-sub-state 4
    if rd8(b, obj + 5) == 4 {
        let n = if rd8(b, obj + 6) == 4 { (rng.next() % 2) as u8 } else { 0 };
        wr8(b, obj + 0xa2, n);
    }
    wr8(b, obj + 0xa3, 2 * (rng.next() % 8) as u8);
    wr8(b, obj + 0xa0, rng.next() as u8);
    wr8(b, obj + 0xa1, (rng.next() % 0x20) as u8);
    wr8(b, obj + 0xac, (rng.next() % 3) as u8);
    wr8(b, obj + 0xad, (rng.next() % 2) as u8);
    wr8(b, obj + 0xae, (rng.next() % 2) as u8);
    wr8(b, obj + 0x7, 2 * (rng.next() % 3) as u8);
    wr32(b, obj + 0xa8, if rng.next().is_multiple_of(2) { 0x0204_0200 } else { 0x0202_0000 });
    wr16(b, obj + 0x84, (rng.next() % 4) as u16);
    wr8(b, obj + 0x49, 4 * (rng.next() % 41) as u8);
    wr8(b, obj + 0x4a, 0x20 * (rng.next() % 2) as u8);
    // AI parameters from the type 16 initialization (decision masks, durations)
    for (off, v) in [(0x68, 2), (0x69, 2), (0x70, 0), (0x6a, 1), (0x6c, 2), (0x6d, 4), (0x72, 2), (0x73, 2), (0x74, 2), (0x75, 0), (0x76, 3), (0x79, 6), (0x80, 7), (0x81, 6)] {
        wr8(b, obj + off, v);
    }
    wr8(b, obj + 0x72, (rng.next() % 4) as u8);
    wr8(b, obj + 0x73, (rng.next() % 4) as u8);
    // a host looking one way or the other (grabbed by a player)
    let host = rd16(b, obj + 0x46) as u32 | 0xff0000;
    wr8(b, host + 0x2c, 0x20 * (rng.next() % 2) as u8);
    // the target (+0x50) is one of the players
    wr16(b, obj + 0x50, 0x85de + 0x200 * (rng.next() % 4) as u16);
}

/// Displacer Beast (type 19) fields set to values its tables accept: `+7` step, reaction
/// code, short counters, current animation, state to resume after a U-turn.
fn prep_dbeast(b: &mut cps2::Cps2Bus, rng: &mut Rng, obj: u32) {
    use ddsom_game::mem::{rd16, rd8, wr16, wr32, wr8};
    let (s4, s5, s6) = (rd8(b, obj + 4), rd8(b, obj + 5), rd8(b, obj + 6));
    if s4 == 2 {
        let step = step_dbeast(rng, s4, s5, s6);
        wr8(b, obj + 7, step);
    }
    wr8(b, obj + 0x49, 4 * (rng.next() % 43) as u8);
    wr8(b, obj + 0x4a, 0x20 * (rng.next() % 2) as u8);
    // often at ground level, falling back: the jump landings
    if rng.next().is_multiple_of(2) {
        wr16(b, obj + 0xc, (rng.next() % 3) as u16);
        wr16(b, obj + 0x18, 0xff00);
        wr16(b, obj + 0x1a, 0xffc8);
    }
    for off in [0x6b, 0x6e, 0x71, 0x77, 0x90, 0xa0, 0xab, 0xae, 0xb5] {
        wr8(b, obj + off, (rng.next() % 5) as u8);
    }
    wr8(b, obj + 0xbf, (rng.next() % 16) as u8);
    // attached piece (variant 0 to 3: rank) or dropped
    wr8(b, obj + 0xbc, if rng.next().is_multiple_of(3) { 0xff } else { (rng.next() % 8) as u8 });
    // state to resume after a U-turn: a valid active state
    let resumes: [u32; 4] = [0x0202_0000, 0x0204_0000, 0x0206_0000, 0x0208_0004];
    wr32(b, obj + 0xa6, resumes[rng.next() as usize % 4]);
    wr8(b, obj + 0xa4, 2 * (rng.next() % 3) as u8);
    wr16(b, obj + 0x84, (rng.next() % 4) as u16);
    // current animation: often the one the step will want to start (repeated hits)
    const ANIMS: [u8; 22] = [0, 5, 6, 7, 0xc, 0xd, 0xe, 0xf, 0x16, 0x18, 0x19, 0x1c, 0x1d, 0x1e, 0x28, 0x2a, 0x2b, 0x2d, 0x2e, 0x2f, 0x7f, 0xff];
    wr8(b, obj + 0xa2, ANIMS[rng.next() as usize % ANIMS.len()]);
    wr8(b, obj + 0xaf, ANIMS[rng.next() as usize % ANIMS.len()]);
    // the target is a player
    wr16(b, obj + 0x50, 0x85de + 0x200 * (rng.next() % 4) as u16);
    // the parent (+0x42): state, variant, animation index, arm counter
    let parent = rd16(b, obj + 0x42) as u32 | 0xff0000;
    if rd16(b, obj + 0x42) != 0 {
        const STATES: [u32; 6] = [0x0402_0000, 0x0402_0c00, 0x0200_0600, 0x0202_0000, 0x0402_0200, 0x0408_0000];
        wr32(b, parent + 4, STATES[rng.next() as usize % 6]);
        wr8(b, parent + 3, (rng.next() % 3) as u8);
        wr8(b, parent + 0xa1, (rng.next() % 0x20) as u8);
        wr8(b, parent + 0xaa, (rng.next() % 3) as u8);
        wr8(b, parent + 0x2c, 0x20 * (rng.next() % 2) as u8);
    }
}

/// Master states of a type 26 enemy (+0xb8 to +0xbb, plus +4/+5 and the reaction code +0x49) that
/// the original's jump tables cover.
fn master_state_26(b: &mut cps2::Cps2Bus, rng: &mut Rng, master: u32) {
    use ddsom_game::mem::{wr16, wr32, wr8};
    let pair = |rng: &mut Rng, n: u32| 2 * (rng.next() % n) as u8;
    let group = pair(rng, 4);
    let (mode, phase, step);
    if group == 4 {
        mode = pair(rng, 6);
        (phase, step) = match mode {
            // the reaction routines are only safe in their initial state
            0 => (0, 0),
            2 => (pair(rng, 3), 0),
            4 => (pair(rng, 5), 0),
            _ => (pair(rng, 4), 0),
        };
    } else {
        mode = pair(rng, 8);
        (phase, step) = match mode {
            0 => (pair(rng, 8), 0),
            2 => {
                let ph = pair(rng, 6);
                (ph, if ph >= 4 { pair(rng, 5) } else { 0 })
            }
            4 => {
                let ph = pair(rng, 6);
                (ph, if ph == 8 { pair(rng, 3) } else if ph == 10 { pair(rng, 5) } else { 0 })
            }
            _ => (0, 0),
        };
    }
    wr8(b, master + 0xb8, group);
    wr8(b, master + 0xb9, mode);
    wr8(b, master + 0xba, phase);
    wr8(b, master + 0xbb, step);
    wr8(b, master + 0x33, (rng.next() % 2) as u8);
    // reaction code: multiple of 4 in the table (45 entries)
    wr8(b, master + 0x49, 4 * (rng.next() % 45) as u8);
    for off in [0x14u32, 0x18, 0x1c] {
        wr32(b, master + off, rng.next() % 0x400);
    }
    wr16(b, master + 0x16, (rng.next() % 0x40) as u16);
    // master state: 0x406 (mid-hit) triggers the double's disappearance
    wr16(b, master + 4, if rng.next().is_multiple_of(12) { 0x406 } else { 0x200 });
}

/// Master states of a type 24 enemy (bytes +4 to +7) that the original's jump tables cover.
fn master_state_24(b: &mut cps2::Cps2Bus, rng: &mut Rng, master: u32) {
    use ddsom_game::mem::wr8;
    let pair = |rng: &mut Rng, n: u32| 2 * (rng.next() % n) as u8;
    let state = pair(rng, 4);
    let (sub, s6, s7);
    if state == 4 {
        sub = pair(rng, 6);
        s6 = pair(rng, 3);
        s7 = pair(rng, 3);
    } else {
        sub = pair(rng, 7);
        s6 = match sub {
            0 => pair(rng, 3),
            2 => pair(rng, 6),
            _ => pair(rng, 7),
        };
        s7 = match (sub, s6) {
            (4, 0) => pair(rng, 2),
            (4, 8) => pair(rng, 7),
            (4, 2 | 4 | 6) | (2, 10) => pair(rng, 5),
            (2, 8) => pair(rng, 3),
            (4, 10) => pair(rng, 5),
            (4, 12) => pair(rng, 6),
            _ => pair(rng, 3),
        };
    }
    wr8(b, master + 4, state);
    wr8(b, master + 5, sub);
    wr8(b, master + 6, s6);
    wr8(b, master + 7, s7);
    wr8(b, master + 0xad, rng.next().is_multiple_of(3) as u8);
}

fn eshl_states(kind: u8) -> &'static [(&'static [u8], &'static [u8], &'static [u8])] {
    const T: &[u8] = &[2];
    const TQ: &[u8] = &[2, 4];
    const Z: &[u8] = &[0];
    const Z2: &[u8] = &[0, 2];
    match kind {
        15 => &[(T, &[0], &[0, 2, 4, 6, 8, 10]), (T, &[2], Z), (T, &[4], &[0, 2, 4]), (&[4], Z, Z)],
        16 => &[(TQ, Z2, Z)],
        17 => &[(T, Z, Z), (T, &[2], Z2), (T, &[4], Z), (&[4], Z, Z)],
        18 | 24 | 25 | 28 | 38 => &[(TQ, Z, Z)],
        19 => &[(TQ, Z, Z)],
        20 => &[(T, Z, Z2), (T, &[2], &[0, 2, 4, 6])],
        26 => &[(T, &[0, 2, 4], Z), (&[4], Z, Z)],
        30 => &[(TQ, &[0, 2, 4, 6], Z)],
        32 | 39 => &[(T, &[0, 2, 4], Z)],
        33 => &[(T, Z2, Z), (T, &[4], Z2)],
        21 | 22 | 34 | 37 => &[(T, Z2, Z)],
        _ => &[(TQ, Z2, Z)],
    }
}

/// Perturbs the global variables read by the SET / SOBJ controllers: game mode, number of
/// players, cameras, rank, and for each player its state, life, keys and position
/// autour de l'objet `obj`.
fn perturb_globals(b: &mut cps2::Cps2Bus, rng: &mut Rng, step: u32, obj: u32) {
    use ddsom_game::mem::{rd16, wr8};
    if step != 0 && !rng.next().is_multiple_of(4) {
        return;
    }
    // work RAM bytes (A5+) and possible values
    let bytes: [(u32, &[u8]); 16] = [
        (0x2c, &[0xc, 0xc, 0xc, 0, 8]),
        (0x30, &[0, 1, 1, 1]),
        (0x34, &[0, 1, 4, 6, 7, 0x1c, 0x1f, 0x2b, 0x45, 0x50]),
        (0x35, &[0, 1, 2, 3, 4]),
        (0x36, &[0, 0, 1]),
        (0x42, &[0, 0, 0, 1]),
        (0x4e, &[0, 1, 4]),
        (0x50, &[0, 0, 0, 1]),
        (0x51, &[0, 1, 2]),
        (0x58, &[0, 0, 1]),
        (0x59, &[0, 1]),
        (0x6f, &[0, 1, 7, 8, 0x10]),
        (0x48e, &[0, 1, 0xc, 0xf]),
        (0x48f, &[0, 1, 7]),
        (0x53a, &[0, 0, 0, 1]),
        (0x70, &[0, 0, 1]),
    ];
    for (off, values) in bytes {
        if rng.next().is_multiple_of(3) {
            wr8(b, A5 + off, values[rng.next() as usize % values.len()]);
        }
    }
    // cameras, rank
    for off in [0x506u32, 0x556] {
        if rng.next().is_multiple_of(2) {
            wr16(b, A5 + off, [0, 0x100, 0x180, 0x1c0, 0x200, 0x2b0, 0x300, 0xa00][rng.next() as usize % 8]);
        }
    }
    for off in [0x50au32, 0x55a] {
        if rng.next().is_multiple_of(2) {
            wr16(b, A5 + off, (rng.next() % 0x200) as u16);
        }
    }
    if rng.next().is_multiple_of(3) {
        wr16(b, A5 + 0x7f92, (rng.next() % 32) as u16);
    }
    // players: active, state, life, keys, position close to the object
    for j in 0..4u32 {
        let p = 0xff85de + 0x200 * j;
        if rng.next().is_multiple_of(3) {
            wr8(b, p, [0, 1, 1, 1, 2][rng.next() as usize % 5]);
        }
        if rng.next().is_multiple_of(4) {
            wr8(b, p + 4, [0, 1, 2, 2, 3, 4][rng.next() as usize % 6]);
            wr8(b, p + 5, 0);
            wr8(b, p + 6, 0);
        }
        if rng.next().is_multiple_of(3) {
            wr16(b, p + 0x62, [0, 0x08, 0x20, 0x40, 0x80][rng.next() as usize % 5]);
        }
        if rng.next().is_multiple_of(3) {
            wr8(b, p + 0x6c, rng.next() as u8);
            wr8(b, 0xff800c + j, rng.next() as u8 & 0xf0);
        }
        if rng.next().is_multiple_of(3) {
            let x = rd16(b, obj + 8).wrapping_add((rng.next() % 0x60) as u16).wrapping_sub(0x30);
            let y = rd16(b, obj + 0x10).wrapping_add((rng.next() % 0x30) as u16).wrapping_sub(0x18);
            wr16(b, p + 8, x);
            wr16(b, p + 0x10, y);
            wr16(b, p + 0xc, if rng.next().is_multiple_of(4) { 0x30 } else { 0 });
        }
        if rng.next().is_multiple_of(4) {
            wr8(b, p + 3, (rng.next() % 12) as u8);
        }
    }
}

fn families(roms: &str, eeprom: &str, scenario: &str, warmup: u32, rounds: u32, filter: Option<u32>) {
    use ddsom_game::mem::{rd16, rd8};
    use ddsom_game::objects::pools::{Pool, ENEMY, ESHL, SET, SOBJ};
    let ee = std::fs::read(eeprom).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(roms), ee.as_deref()).expect("roms");
    ddsom_game::install(&mut m);
    let script = ddsom_game::scenario::Scenario::load(Path::new(scenario)).expect("scenario");
    for f in 0..warmup as usize {
        script.apply(f, &mut m);
        m.run_frame();
    }
    ddsom_game::set_check(true);
    // panics are counted and summarized at the end (message and location), not displayed one by one
    std::panic::set_hook(Box::new(|info| {
        let msg = info.payload().downcast_ref::<String>().cloned()
            .or_else(|| info.payload().downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_default();
        let place = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_default();
        *LAST_PANIC.lock().unwrap() = Some(format!("{msg} ({place})"));
    }));
    let mut panics: std::collections::BTreeMap<String, u32> = Default::default();
    let seed: u64 = std::env::var("DDSOM_GRAINE").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    let mut rng = Rng(0x1234_5678_9abc_def1 ^ seed.wrapping_mul(0x9e37_79b9_7f4a_7c15));
    let tables: [(&Pool, u32, u32); 4] = [(&ESHL, 0x39b96, 64), (&SET, 0x39eac, 34), (&SOBJ, 0x39fa8, 16), (&ENEMY, 0x39a78, 29)];
    let mut inputs: Vec<(u32, &Pool, u8)> = Vec::new();
    for (pool, table, n) in tables {
        for k in 0..n {
            let a = (m.bus.fetch16(table + 4 * k) as u32) << 16 | m.bus.fetch16(table + 4 * k + 2) as u32;
            if ddsom_game::all_natives().any(|nat| nat.entry == a) && filter.is_none_or(|f| f == a) {
                inputs.push((a, pool, k as u8));
            }
        }
    }
    let (c0, b0) = (m.cpu.clone(), m.bus.clone());
    if std::env::var("DDSOM_VERBOSE").is_ok() {
        let mut b = b0.clone();
        println!("places libres : ESHL {} SET {} SOBJ {}", rd16(&mut b, A5 + ESHL.free), rd16(&mut b, A5 + SET.free), rd16(&mut b, A5 + SOBJ.free));
    }
    // number of variants drawn (3 by default: beyond that, many types leave their tables)
    let variants: u32 = std::env::var("DDSOM_VARIANTES").ok().and_then(|v| v.parse().ok()).unwrap_or(3);
    let duration: u32 = std::env::var("DDSOM_PAS").ok().and_then(|v| v.parse().ok()).unwrap_or(60);
    let mut shown = 0;
    let mut tests = 0;
    // coverage of the handlers' original code (DDSOM_COUVERTURE=file: writes the addresses seen)
    let coverage = std::env::var("DDSOM_COUVERTURE").is_ok();
    let mut seen: std::collections::BTreeSet<u32> = Default::default();
    for _ in 0..rounds {
        for &(entry, pool, kind) in &inputs {
            let (mut c, mut b) = (c0.clone(), b0.clone());
            b.irq_horizon = u64::MAX;
            // possible owners: active enemies, otherwise players
            let mut owners: Vec<u32> = ENEMY.iter().filter(|o| rd8(&mut b, o.0) != 0).map(|o| o.0).collect();
            owners.extend((0..4).map(|j| 0xff85de + 0x200 * j));
            let obj = pool.slot(rng.next() % pool.slots);
            for off in (0..pool.stride).step_by(2) {
                wr16(&mut b, obj.0 + off, 0);
            }
            if pool.class == 2 && (matches!(kind, 4 | 19 | 21 | 25) || lot3::handles(kind)) {
                // an owner distinct from the object: its states are rewritten at random afterwards
                owners.retain(|&o| o != obj.0);
            }
            let mut owner = owners[rng.next() as usize % owners.len()];
            if pool.class == 2 {
                // the master is not the object itself: take another enemy, otherwise a player
                owners.retain(|&o| o != obj.0 && o >= ENEMY.slot(0).0 && o < ENEMY.slot(ENEMY.slots).0);
                if owners.is_empty() {
                    // no other enemy in the scene: fabricate one, inactive in every respect
                    let other = ENEMY.slot((rng.next() % ENEMY.slots).min(ENEMY.slots - 1));
                    let other = if other.0 == obj.0 { ENEMY.slot(0) } else { other };
                    let other = if other.0 == obj.0 { ENEMY.slot(1) } else { other };
                    for off in (0..ENEMY.stride).step_by(2) {
                        wr16(&mut b, other.0 + off, 0);
                    }
                    wr8(&mut b, other.0, 1);
                    wr8(&mut b, other.0 + 2, 25);
                    wr8(&mut b, other.0 + 0x28, 2);
                    owners.push(other.0);
                }
                owner = owners[rng.next() as usize % owners.len()];
            }
            wr8(&mut b, obj.0, 1);
            wr8(&mut b, obj.0 + 2, kind);
            wr8(&mut b, obj.0 + 3, (rng.next() % variants_of(entry).unwrap_or(variants)) as u8);
            if pool.class == 2 && kind == 16 {
                wr8(&mut b, obj.0 + 3, 2 * (rng.next() % 6) as u8);
            }
            wr8(&mut b, obj.0 + 0x28, pool.class);
            wr8(&mut b, obj.0 + 0x2c, (rng.next() & 1) as u8);
            wr8(&mut b, obj.0 + 0x2f, if rng.next().is_multiple_of(4) { rng.next() as u8 % 3 } else { 0 });
            if pool.class == 2 && kind == 21 {
                // the attack series indexes word tables: 0, 2 or 4
                let series = 2 * (rd8(&mut b, obj.0 + 0x2f) % 3);
                wr8(&mut b, obj.0 + 0x2f, series);
            }
            if pool.class == 2 && kind == 19 {
                // the AI profile has only two values (the original's jump table has no other)
                let profile = rd8(&mut b, obj.0 + 0x2f) % 2;
                wr8(&mut b, obj.0 + 0x2f, profile);
            }
            if entry == 0xa63d8 {
                // ESHL 61: its creator (`+0x5c`, the effect 0xb8b..) has a variant of 0 to 2, which indexes the
                // offset tables (beyond them, the original jumps outside its table)
                wr8(&mut b, owner + 3, (rng.next() % 3) as u8);
            }
            wr16(&mut b, obj.0 + 0x50, owner as u16);
            wr16(&mut b, obj.0 + 0x46, owner as u16);
            wr16(&mut b, obj.0 + 0x5a, owner as u16);
            // ESHL 15..39: creator at +0x5c, host at +0x42, long pointers (types 17 and 19), delays,
            // level variables read by the shots
            if pool.class == 8 && kind >= 15 {
                wr16(&mut b, obj.0 + 0x5c, owner as u16);
                if rng.next().is_multiple_of(2) {
                    wr16(&mut b, obj.0 + 0x42, owner as u16);
                }
                wr16(&mut b, obj.0 + 0x58, (rng.next() % 6) as u16);
                if kind == 17 || kind == 19 {
                    wr32(&mut b, obj.0 + 0x50, owner);
                    wr32(&mut b, obj.0 + 0x56, owner);
                    wr8(&mut b, obj.0 + 0x43, [0, 1, 0xff][rng.next() as usize % 3]);
                    wr8(&mut b, obj.0 + 0x42, 4 * (rng.next() % 2) as u8);
                }
                if std::env::var("DDSOM_ETATS").is_ok() && rng.next().is_multiple_of(2) {
                    let hitboxes = eshl_states(kind);
                    let (s4, s5, s6) = hitboxes[rng.next() as usize % hitboxes.len()];
                    wr8(&mut b, obj.0 + 4, s4[rng.next() as usize % s4.len()]);
                    wr8(&mut b, obj.0 + 5, s5[rng.next() as usize % s5.len()]);
                    wr8(&mut b, obj.0 + 6, s6[rng.next() as usize % s6.len()]);
                }
                // most often, the owner is on screen (otherwise the shot is immediately removed by culling)
                if !rng.next().is_multiple_of(4) {
                    let (cx, cy) = (rd16(&mut b, A5 + 0x506), rd16(&mut b, A5 + 0x50a));
                    wr16(&mut b, owner + 8, cx.wrapping_add(0x60 + (rng.next() % 0x80) as u16));
                    wr16(&mut b, owner + 0x10, cy.wrapping_add(0x60 + (rng.next() % 0x40) as u16));
                }
                wr16(&mut b, A5 + 0x7f92, (rng.next() % 8) as u16);
                wr8(&mut b, A5 + 0x35, (rng.next() % 5) as u8);
                wr8(&mut b, A5 + 0x36, (rng.next() % 2) as u8);
            }
            // ENEMY: host at +0x42, HP, drawn states (DDSOM_ETATS), nearby players
            if pool.class == 2 {
                wr16(&mut b, obj.0 + 0xb2, owner as u16);
                if kind == 26 {
                    wr16(&mut b, obj.0 + 0xb0, owner as u16);
                    master_state_26(&mut b, &mut rng, owner);
                } else if matches!(kind, 22..=24) {
                    master_state_24(&mut b, &mut rng, owner);
                }
                if rng.next().is_multiple_of(2) {
                    wr16(&mut b, obj.0 + 0x42, owner as u16);
                }
                wr16(&mut b, obj.0 + 0x62, (rng.next() % 40) as u16);
                // host of a composite part (type 7): its type, its states and its last hit
                if kind == 7 {
                    wr8(&mut b, owner + 2, [0xe, 0x13, 5][rng.next() as usize % 3]);
                    wr8(&mut b, owner + 0xa2, (rng.next() % 7) as u8);
                    wr8(&mut b, owner + 0x3e, if rng.next().is_multiple_of(3) { 0xff } else { 0 });
                    wr8(&mut b, owner + 0x2c, (rng.next() % 2) as u8 * 0x20);
                }
                // life tracking of the doubles (type 23): minimum known HP, delay, "wounded" flag
                wr16(&mut b, obj.0 + 0x64, (rng.next() % 50) as u16);
                wr16(&mut b, obj.0 + 0xb4, (rng.next() % 3) as u16);
                wr8(&mut b, obj.0 + 0xaf, rng.next().is_multiple_of(3) as u8);
                wr8(&mut b, obj.0 + 0xb6, rng.next().is_multiple_of(3) as u8);
                // linked parts (halos, flames, arms, shadow): existing objects drawn at random
                if kind == 20 && !rng.next().is_multiple_of(4) {
                    for off in [0xa4u32, 0xa6, 0xa8, 0xaa, 0xac, 0xae, 0xb4] {
                        wr16(&mut b, obj.0 + off, owners[rng.next() as usize % owners.len()] as u16);
                    }
                    wr8(&mut b, obj.0 + 0xb0, (rng.next() % 4) as u8);
                }
                wr16(&mut b, obj.0 + 0x60, 40);
                if std::env::var("DDSOM_ETATS").is_ok() && rng.next().is_multiple_of(2) && !lot3::handles(kind) {
                    let hitboxes = enemy_states(kind, rd8(&mut b, obj.0 + 3));
                    let (s4, s5, s6) = hitboxes[rng.next() as usize % hitboxes.len()];
                    wr8(&mut b, obj.0 + 4, s4[rng.next() as usize % s4.len()]);
                    wr8(&mut b, obj.0 + 5, s5[rng.next() as usize % s5.len()]);
                    wr8(&mut b, obj.0 + 6, s6[rng.next() as usize % s6.len()]);
                }
                if kind == 16 {
                    prep_enemy_ai(&mut b, &mut rng, obj.0);
                }
                if kind == 19 {
                    prep_dbeast(&mut b, &mut rng, obj.0);
                }
                if kind == 4 {
                    prep_owlbear(&mut b, &mut rng, obj.0);
                }
                if kind == 21 {
                    prep_harpy(&mut b, &mut rng, obj.0);
                }
                if kind == 25 {
                    prep_warrior(&mut b, &mut rng, obj.0);
                }
                if !rng.next().is_multiple_of(4) {
                    let (cx, cy) = (rd16(&mut b, A5 + 0x506), rd16(&mut b, A5 + 0x50a));
                    wr16(&mut b, obj.0 + 8, cx.wrapping_add(0x60 + (rng.next() % 0x80) as u16));
                    wr16(&mut b, obj.0 + 0x10, cy.wrapping_add(0x60 + (rng.next() % 0x40) as u16));
                    wr16(&mut b, owner + 8, cx.wrapping_add(0x60 + (rng.next() % 0x80) as u16));
                    wr16(&mut b, owner + 0x10, cy.wrapping_add(0x60 + (rng.next() % 0x40) as u16));
                }
            }
            if entry == 0xd5f5a {
                // life bar (SET 20): followed player at +0x20, second player at +0x34
                wr16(&mut b, obj.0 + 0x20, 0x85de + 0x200 * (rng.next() % 4) as u16);
                wr16(&mut b, obj.0 + 0x34, 0x85de + 0x200 * (rng.next() % 4) as u16);
            }
            for off in [0x8u32, 0xc, 0x10] {
                let v = (rd16(&mut b, owner + off) as u32) << 16 | (rng.next() & 0xffff);
                wr32(&mut b, obj.0 + off, v);
            }
            wr16(&mut b, obj.0 + 0xc, (rng.next() % 0x60) as u16);
            // consistent free-slot count
            // (at warm-up time the pool may be full: the handlers test this counter)
            wr16(&mut b, A5 + pool.free, (pool.slots / 2) as u16);
            if pool.class == SET.class && matches!(kind, 2 | 15) {
                scene_state(&mut b, &mut rng, obj.0, kind, true);
            }
            for step in 0..duration {
                if rd8(&mut b, obj.0) == 0 {
                    break;
                }
                if step > 0 {
                    if pool.class == SET.class && matches!(kind, 2 | 15) && rng.next().is_multiple_of(6) {
                        scene_state(&mut b, &mut rng, obj.0, kind, false);
                    }
                    if rng.next().is_multiple_of(6) {
                        wr8(&mut b, obj.0 + 0x57, [0x08, 0x20, 0x28, 0x10, 0x02, 0x40][rng.next() as usize % 6]);
                    }
                    if rng.next().is_multiple_of(8) {
                        wr8(&mut b, obj.0 + 0x33, rng.next() as u8 & 1);
                    }
                    if rng.next().is_multiple_of(10) && !(pool.class == 2 && matches!(kind, 22..=24 | 26)) {
                        wr8(&mut b, owner + 4, (rng.next() % 8) as u8);
                    }
                    if pool.class == 2 && matches!(kind, 22..=24 | 26) && rng.next().is_multiple_of(3) {
                        if kind == 26 {
                            master_state_26(&mut b, &mut rng, owner);
                        } else {
                            master_state_24(&mut b, &mut rng, owner);
                        }
                    }
                    // the owner changes state: state-machine states, attack flags, counters
                    if rng.next().is_multiple_of(3) && !(pool.class == 2 && matches!(kind, 22..=24 | 26)) {
                        for off in [4u32, 5, 6, 7] {
                            if rng.next().is_multiple_of(2) {
                                wr8(&mut b, owner + off, 2 * (rng.next() % 6) as u8);
                            }
                        }
                        for off in [0x53u32, 0xa0, 0xa1, 0xa8, 0xa9, 0xb0, 0xbd, 0x3e, 0x2f, 0x42, 0x5d] {
                            if rng.next().is_multiple_of(3) {
                                let v = match rng.next() % 4 {
                                    0 => 0,
                                    1 => 1,
                                    2 => (rng.next() % 4) as u8,
                                    _ => rng.next() as u8,
                                };
                                wr8(&mut b, owner + off, v);
                            }
                        }
                    }
                    // its own variant / control flags
                    if rng.next().is_multiple_of(12) {
                        wr8(&mut b, obj.0 + 0x3e, rng.next() as u8 % 6);
                    }
                    if rng.next().is_multiple_of(10) {
                        wr8(&mut b, obj.0 + 0x53, (rng.next() % 5) as u8);
                    }
                }
                if pool.class == 2 {
                    // free-slot counters of all pools, consistent with their contents (the composite
                    // enemies read them to know whether they can create their parts)
                    for q in ddsom_game::objects::pools::POOLS {
                        let free = (0..q.slots).filter(|&i| rd8(&mut b, q.slot(i).0) == 0).count();
                        wr16(&mut b, A5 + q.free, free as u16);
                    }
                }
                if pool.class == 2 && rng.next().is_multiple_of(5) {
                    // difficulty, scene and living players read by the decision tables
                    wr16(&mut b, A5 + 0x7f92, (rng.next() % 8) as u16);
                    wr8(&mut b, A5 + 0x34, if rng.next().is_multiple_of(4) { 4 } else { (rng.next() % 12) as u8 });
                    wr8(&mut b, A5 + 0x35, (rng.next() % 5) as u8);
                }
                if pool.class == 2 && rng.next().is_multiple_of(2) {
                    // active players, sometimes near the enemy sometimes far
                    for j in 0..4u32 {
                        let q = 0xff85de + 0x200 * j;
                        if !rng.next().is_multiple_of(3) {
                            wr8(&mut b, q, 1);
                        }
                        let x = rd16(&mut b, obj.0 + 8).wrapping_add((rng.next() % 0x90) as u16).wrapping_sub(0x48);
                        let y = rd16(&mut b, obj.0 + 0x10).wrapping_add((rng.next() % 0x40) as u16).wrapping_sub(0x20);
                        wr16(&mut b, q + 8, x);
                        wr16(&mut b, q + 0x10, y);
                        wr16(&mut b, q + 0xc, (rng.next() % 0x30) as u16);
                    }
                }
                if pool.class == 2 && lot3::handles(kind) && (step == 0 || rng.next().is_multiple_of(3)) {
                    // valid state and fields of the third batch, redrawn from time to time
                    lot3::prep(&mut b, &mut rng, kind, obj.0, step == 0);
                }
                if pool.class == 2 && matches!(kind, 4 | 19 | 21 | 25) && step > 0 && rng.next().is_multiple_of(3) {
                    // new valid state each time: visits many more steps than the trajectory alone
                    let hitboxes = enemy_states(kind, rd8(&mut b, obj.0 + 3));
                    let (s4, s5, s6) = hitboxes[rng.next() as usize % hitboxes.len()];
                    wr8(&mut b, obj.0 + 4, s4[rng.next() as usize % s4.len()]);
                    wr8(&mut b, obj.0 + 5, s5[rng.next() as usize % s5.len()]);
                    wr8(&mut b, obj.0 + 6, s6[rng.next() as usize % s6.len()]);
                    if kind == 25 {
                        prep_warrior(&mut b, &mut rng, obj.0);
                    } else if kind == 21 {
                        prep_harpy(&mut b, &mut rng, obj.0);
                    } else if kind == 4 {
                        prep_owlbear(&mut b, &mut rng, obj.0);
                    } else {
                        prep_dbeast(&mut b, &mut rng, obj.0);
                    }
                }
                if pool.class == 2 && (matches!(kind, 4 | 19) || lot3::handles(kind)) && rng.next().is_multiple_of(2) {
                    // players in the beast's range boxes (narrow width, depth ±8)
                    for j in 0..4u32 {
                        let q = 0xff85de + 0x200 * j;
                        if !rng.next().is_multiple_of(4) {
                            wr8(&mut b, q, 1);
                        }
                        let dx = (rng.next() % 0x180) as u16;
                        let dy = (rng.next() % 0x12) as u16;
                        let (ox, oy) = (rd16(&mut b, obj.0 + 8), rd16(&mut b, obj.0 + 0x10));
                        wr16(&mut b, q + 8, ox.wrapping_add(dx).wrapping_sub(0x100));
                        wr16(&mut b, q + 0x10, oy.wrapping_add(dy).wrapping_sub(9));
                        wr16(&mut b, q + 0xc, (rng.next() % 4) as u16);
                    }
                }
                if pool.class == 2 && kind == 7 && rng.next().is_multiple_of(4) {
                    wr8(&mut b, owner + 0xa2, (rng.next() % 7) as u8);
                    wr8(&mut b, owner + 0x3e, if rng.next().is_multiple_of(3) { 0xff } else { 0 });
                    wr8(&mut b, owner + 0xa3, (rng.next() % 2) as u8);
                    if rng.next().is_multiple_of(5) {
                        wr8(&mut b, owner, 0);
                    }
                }
                if pool.class == 2 && step > 0 && !lot3::handles(kind) {
                    for off in [0x33u32, 0x53, 0x56, 0x57, 0xa2, 0xa3, 0xa4, 0xb0, 0xb2, 0xb6, 0x20, 0x21, 0x24, 0x26, 0x30, 0x32, 0x3e] {
                        // +0x24 and +0x26 are the sorted-list links: only type 20 uses them
                        // as counters (type 19 parent), they are corrupted only for it
                        let links = matches!(off, 0x20 | 0x24 | 0x26) || (kind == 16 && matches!(off, 0xa2 | 0xa3)) || (kind == 19 && off == 0xa4) || (kind == 4 && matches!(off, 0xa4 | 0xa3)) || (kind == 21 && off == 0xa2);
                        if rng.next().is_multiple_of(9) && !(matches!(kind, 22..=24 | 26) && matches!(off, 0xb0 | 0xb2)) && !(links && (kind != 20 || off == 0xa3)) {
                            let v = match rng.next() % 4 {
                                0 => 0,
                                1 => 1,
                                2 => (rng.next() % 4) as u8,
                                _ => rng.next() as u8,
                            };
                            wr8(&mut b, obj.0 + off, v);
                            // the state of the host of a part (type 7) stays within the ROM table
                            if rng.next().is_multiple_of(3) && !(kind == 7 && matches!(off, 0xa2 | 0xa3)) && !(kind == 25 && off == 0xa2) {
                                wr8(&mut b, owner + off, v);
                            }
                        }
                    }
                    if rng.next().is_multiple_of(15) {
                        wr16(&mut b, obj.0 + 0x62, (rng.next() % 5) as u16);
                        wr16(&mut b, obj.0 + 0x64, (rng.next() % 50) as u16);
                    }
                }
                // SET and SOBJ: scene and interface controllers, which read global variables
                // (game mode, number of players, cameras, players): these are drawn too
                if pool.class == 0x10 || pool.class == 0x14 {
                    perturb_globals(&mut b, &mut rng, step, obj.0);
                }
                // game paused (rare) or not; suspended object (byte +0x40) very rarely
                wr8(&mut b, A5 + 0x3d, rng.next().is_multiple_of(10) as u8);
                if pool.class == 8 && kind >= 15 {
                    wr8(&mut b, A5 + 0x6f, step as u8);
                    wr16(&mut b, A5 + 0x6e, rng.next() as u16);
                    wr8(&mut b, A5 + 0x42, (rng.next() % 2) as u8);
                    wr8(&mut b, A5 + 0x4e, rng.next().is_multiple_of(16) as u8);
                }
                wr8(&mut b, obj.0 + 0x40, rng.next().is_multiple_of(40) as u8);
                c.a[0] = obj.0;
                c.a[5] = A5;
                c.d[7] = step;
                // dispatch loop registers: D0 = type * 4 (high byte 0 empty), A1 = handler
                c.d[0] = 4 * kind as u32;
                c.a[1] = entry;
                tests += 1;
                let mismatches = || ddsom_game::report().into_iter().map(|(n, s)| (n, s.mismatches)).collect::<std::collections::BTreeMap<_, _>>();
                let before = mismatches();
                let (cs, bs) = (c.clone(), b.clone());
                if coverage {
                    // addresses of the instructions the original executes in this call (interpreter)
                    let (mut c2, mut b2) = (cs.clone(), bs.clone());
                    c2.pc = entry;
                    c2.a[7] = cs.a[7].wrapping_sub(4);
                    ddsom_game::mem::wr32(&mut b2, c2.a[7], 0xfffe);
                    for _ in 0..20_000 {
                        if c2.pc == 0xfffe {
                            break;
                        }
                        seen.insert(c2.pc);
                        c2.step(&mut b2);
                    }
                }
                if std::env::var("DDSOM_MAITRE").is_ok() && pool.class == 2 {
                    let m = rd16(&mut b, obj.0 + 0xb2) as u32 | 0xff0000;
                    let o: Vec<u8> = (0..8).map(|i| rd8(&mut b, m + i)).collect();
                    println!("step {step} master {m:06x} : {o:02x?} obj {:06x}", obj.0);
                }
                let prev_obj: Vec<u8> = (0..pool.stride).map(|i| rd8(&mut b, obj.0 + i)).collect();
                if std::env::var("DDSOM_PAS_ETAT").is_ok() {
                    println!("step {step} variant {:02x} states {:02x?} +2f {:02x}", prev_obj[3], &prev_obj[4..8], prev_obj[0x2f]);
                }
                let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| ddsom_game::call(&mut c, &mut b, entry)));
                if std::env::var("DDSOM_PAS_ETAT").is_ok() {
                    let after: Vec<u8> = (4..8).map(|i| rd8(&mut b, obj.0 + i)).collect();
                    println!("   after: {after:02x?}");
                }
                if r.is_err() {
                    // the original went off the rails (inconsistent entry), or the native panicked: the
                    // trajectory is abandoned, and the panic is counted (final summary)
                    let m = LAST_PANIC.lock().unwrap().take().unwrap_or_default();
                    // grouping by handler and by cause (the text after the last ": ", without the location)
                    let cause = m.rsplit(" : ").next().unwrap_or("").split(" (").next().unwrap_or("");
                    let key = format!("{entry:06x} {cause}");
                    let distinct = panics.len();
                    let n = panics.entry(key).or_insert(0);
                    if *n == 0 && distinct < 40 {
                        println!("panic at {entry:06x} type {kind} step {step}: {m}");
                    }
                    *n += 1;
                    let _ = (cs, bs);
                    break;
                }
                if std::env::var("DDSOM_MAITRE").is_ok() && pool.class == 2 {
                    let m = rd16(&mut b, obj.0 + 0xb2) as u32 | 0xff0000;
                    let o: Vec<u8> = (0..8).map(|i| rd8(&mut b, m + i)).collect();
                    println!("   after: {o:02x?}");
                }
                // the natives of this call that differ from the original (the handler or a routine it calls)
                let after = mismatches();
                let differ: Vec<_> = after.iter().filter(|(n, &m)| m > before.get(*n).copied().unwrap_or(0)).map(|(n, _)| *n).collect();
                if !differ.is_empty() && shown < 12 {
                    shown += 1;
                    let report = ddsom_game::report();
                    let last = differ.iter().filter_map(|n| report.get(n).and_then(|s| s.last.clone()).map(|l| format!("{n}: {l}"))).collect::<Vec<_>>().join("; ");
                    println!("mismatch at {entry:06x} type {kind} step {step} obj {:06x}: {last}", obj.0);
                    println!("  object before: {:02x?}", prev_obj);
                    if pool.class == 2 {
                        let m = ((prev_obj[0xb2] as u32) << 8 | prev_obj[0xb3] as u32) | 0xff0000;
                        let bytes: Vec<u8> = (0..8).map(|i| rd8(&mut bs.clone(), m + i)).collect();
                        println!("  master {m:06x} before: {bytes:02x?}");
                    }
                    if std::env::var("DDSOM_TRACE_ORIG").is_ok() {
                        // step-by-step execution of the original code alone from the previous state
                        let (mut c2, mut b2) = (cs.clone(), bs.clone());
                        let sp = c2.a[7];
                        c2.pc = entry;
                        c2.a[7] = sp.wrapping_sub(4);
                        ddsom_game::mem::wr32(&mut b2, c2.a[7], 0xfffe);
                        for _ in 0..DDSOM_TRACE_LEN {
                            if c2.pc == 0xfffe {
                                break;
                            }
                            let ins = m68k::decode(&mut b2, c2.pc);
                            println!("    {:06x} {:?}", c2.pc, ins.op);
                            c2.step(&mut b2);
                        }
                    }
                }
            }
        }
    }
    let mut total = 0;
    for (name, st) in ddsom_game::report() {
        total += st.mismatches;
        println!("  {name:28} {:>7} calls {:>5} mismatches{}", st.calls, st.mismatches,
            st.first.map(|f| format!(" — {f}")).unwrap_or_default());
    }
    let compares: u64 = ddsom_game::report().values().map(|s| s.calls).sum();
    let n_panics: u32 = panics.values().sum();
    println!("{tests} handler calls, {compares} native calls compared with the original, {n_panics} panics");
    for (key, n) in &panics {
        println!("  {n:6} × {key}");
    }
    if let Ok(file) = std::env::var("DDSOM_COUVERTURE") {
        let text: Vec<String> = seen.iter().map(|a| format!("{a:06x}")).collect();
        std::fs::write(file, text.join("\n")).expect("coverage write");
    }
    // nothing compared (every call panicked): this is not a validation
    println!("{}", if compares == 0 { "FAILED: no call compared" } else if total == 0 { "VALID: no mismatch" } else { "FAILED" });
}

/// Message and location of the last panic (the panic hook is silent).
static LAST_PANIC: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let [_, group, roms, eeprom, scenario, warmup, rounds] = &a[..] else {
        eprintln!("usage: fuzz <group> <roms> <eeprom> <scenario> <warm-up frames> <rounds>");
        std::process::exit(2);
    };
    if group == "families" {
        families(roms, eeprom, scenario, warmup.parse().unwrap(), rounds.parse().unwrap(), std::env::var("DDSOM_ENTREE").ok().and_then(|v| u32::from_str_radix(&v, 16).ok()));
        return;
    }
    let (rng_state, range): Group = match group.as_str() {
        "sound" => (rng_sound, 0x68c..0xee000),
        "u1" => (rng_u1, 0x19a80..0x1a000),
        "c1" => (rng_c1, 0x13c00..0x14e50),
        "m2" => (rng_m2, 0x2d300..0x2ede0),
        "video" => (rng_video, 0xb40..0x1d100),
        "camera" => (rng_camera, 0x2f000..0x33400),
        _ => {
            eprintln!("unknown group: {group}");
            std::process::exit(2);
        }
    };
    let ee = std::fs::read(eeprom).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(roms), ee.as_deref()).expect("roms");
    ddsom_game::install(&mut m);
    let script = ddsom_game::scenario::Scenario::load(Path::new(scenario)).expect("scenario");
    for f in 0..warmup.parse().unwrap() {
        script.apply(f, &mut m);
        m.run_frame();
    }
    ddsom_game::set_check(true);
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    let inputs: Vec<u32> = ddsom_game::all_natives()
        .map(|n| n.entry)
        .filter(|e| range.contains(e) && (*group != "sound" || *e == 0x68c || *e >= 0xe0836))
        .filter(|e| *group != "camera" || matches!(*e, 0x2f81c | 0x2f86e | 0x328a8 | 0x33178 | 0x33196 | 0x331e8 | 0x3326c))
        .filter(|e| *group != "video" || matches!(*e, 0xb40 | 0xb46 | 0xb96 | 0xbac | 0xbc6 | 0xbfc | 0xc58 | 0xca8 | 0xcc8 | 0xce8 | 0x158e | 0x15c8 | 0x1602 | 0x164e | 0x1a296 | 0x1a350 | 0x1a360 | 0x1a366 | 0x1a3d4 | 0x1a608 | 0x1a60a | 0x1a678 | 0x1a67e | 0x1a8ac | 0x1a8b2 | 0x1cd32 | 0x1cd7c | 0x1cdd6 | 0x1cf3c | 0x1cfb8 | 0x1cfc8 | 0x1d00e | 0x1d042 | 0x24a8 | 0x24b0 | 0x255a | 0x25e8 | 0x260c | 0x26d4 | 0x2766))
        .collect();
    let (c, b) = (&mut m.cpu, &mut m.bus);
    b.irq_horizon = u64::MAX;
    let mut shown = 0;
    for _ in 0..rounds.parse::<u32>().unwrap() {
        for &entry in &inputs {
            ENTRY.store(entry, std::sync::atomic::Ordering::Relaxed);
            rng_state(c, b, &mut rng);
            let (d0, a0) = (c.d[0], c.a[0]);
            let context = if entry == 0x2d662 {
                let var = ddsom_game::mem::rd16(b, A5 + 0x70);
                let copy: Vec<u8> = (0..18).map(|i| ddsom_game::mem::rd8(b, 0xff1d8e + i)).collect();
                let block: Vec<u8> = (0..18).map(|i| ddsom_game::mem::rd8(b, c.a[6] + i)).collect();
                format!(" var={var} copy={copy:02x?} defaults={block:02x?}")
            } else if *group == "camera" {
                let cam = c.a[0];
                let fields: Vec<String> = [0x08u32, 0x0c, 0x10, 0x14, 0x20, 0x2e, 0x34, 0x40]
                    .iter()
                    .map(|o| format!("{o:02x}={:04x}", ddsom_game::mem::rd16(b, cam + o)))
                    .collect();
                format!(" scene={:02x} terrain={} {}", ddsom_game::mem::rd8(b, A5 + 0x34), ddsom_game::mem::rd8(b, A5 + 0x37), fields.join(" "))
            } else {
                String::new()
            };
            let before = ddsom_game::report().values().map(|s| s.mismatches).sum::<u64>();
            ddsom_game::call(c, b, entry);
            let after = ddsom_game::report().values().map(|s| s.mismatches).sum::<u64>();
            if after > before && shown < 12 {
                shown += 1;
                let last = ddsom_game::report().values().filter_map(|s| s.last.clone()).next();
                println!("mismatch at {entry:06x} (d0={d0:08x} a0={a0:06x}){context}: {}", last.unwrap_or_default());
            }
        }
    }
    let (mut total, mut compared) = (0, 0);
    for (name, st) in ddsom_game::report() {
        total += st.mismatches;
        compared += st.calls;
        println!("  {name:28} {:>7} calls {:>5} mismatches{}", st.calls, st.mismatches,
            st.first.map(|f| format!(" — {f}")).unwrap_or_default());
    }
    println!("{}", if compared == 0 { "FAILED: no call compared" } else if total == 0 { "VALID: no mismatch" } else { "FAILED" });
}

/// Scene controller (SET 2) and end-of-level screens (SET 15): they depend on
/// game variables (level, difficulty, frozen scene...), the variant (script) and the player state,
/// which the generic fuzz does not vary. `start`: initial object state (variant, state).
fn scene_state(b: &mut Bus, rng: &mut Rng, obj: u32, kind: u8, start: bool) {
    use ddsom_game::mem::{rd16, rd32, rd8};
    wr8(b, A5 + 0x34, if rng.next().is_multiple_of(3) { 4 } else { 1 + (rng.next() % 7) as u8 });
    wr8(b, A5 + 0x35, if rng.next().is_multiple_of(10) { 0 } else { 1 + (rng.next() % 4) as u8 });
    wr8(b, A5 + 0x2c, [0xc, 4, 8, 0, 0x18][rng.next() as usize % 5]);
    wr8(b, A5 + 0x42, rng.next().is_multiple_of(8) as u8);
    wr8(b, A5 + 0x36, rng.next().is_multiple_of(4) as u8);
    wr8(b, A5 + 0x50, (rng.next() % 2) as u8);
    wr8(b, A5 + 0x66, (rng.next() % 2) as u8);
    wr16(b, A5 + 0x7f92, (rng.next() % 6) as u16);
    for j in 0..4u32 {
        let p = 0xff85de + 0x200 * j;
        if rng.next().is_multiple_of(3) {
            wr16(b, p + 4, [0x200, 0x200, 0, 0x600, 0x204][rng.next() as usize % 5]);
            wr8(b, p + 6, rng.next().is_multiple_of(3) as u8);
            wr16(b, p + 0x62, [0, 5, 100][rng.next() as usize % 3]);
        }
    }
    if kind == 15 {
        // variables read by the end screens: fade finished, cameras, score, rng, player objects
        wr8(b, A5 + 0x48f, rng.next().is_multiple_of(3) as u8);
        wr8(b, A5 + 0x6d, rng.next() as u8);
        wr16(b, A5 + 0x6e, rng.next() as u16 & [0xf, 0xff, 0][rng.next() as usize % 3]);
        for off in [0x506u32, 0x50a, 0x556, 0x55a] {
            wr16(b, A5 + off, [0, 0, 1, 0x70, rng.next() as u16 & 0x1ff][rng.next() as usize % 5]);
        }
        wr16(b, A5 + 0x38, [0x80, 0x120, 0x1f0, 0x220, 0x300][rng.next() as usize % 5]);
        wr8(b, A5 + 0x5d, (rng.next() % 2) as u8);
        for j in 0..4u32 {
            let p = 0xff85de + 0x200 * j;
            wr8(b, p + 0x18a, (rng.next() % 2) as u8);
            wr8(b, p + 3, (rng.next() % 6) as u8);
            wr8(b, p + 0x22, (rng.next() % 2) as u8);
            wr16(b, p + 0x60, [50, 100, 160][rng.next() as usize % 3]);
            wr16(b, p + 0x64, (rng.next() % 20) as u16);
            if rng.next().is_multiple_of(4) {
                wr8(b, p + 0x118, [0xff, 0xf9, 0xfa, 0x10, 0][rng.next() as usize % 5]);
            }
        }
        if start {
            if rng.next().is_multiple_of(4) {
                wr8(b, obj + 0x29, 0x1f);
                wr16(b, obj + 0x24, 1 + (rng.next() % 2) as u16);
            }
            for off in [0x14u32, 0x18, 0x22, 0x23, 0x24, 0x2a, 0x2b, 0x30, 0x31, 0x42, 0x43] {
                if rng.next().is_multiple_of(3) {
                    wr8(b, obj + off, [0, 1, 2, 3, rng.next() as u8][rng.next() as usize % 5]);
                }
            }
        }
    }
    if start && kind == 15 {
        // end screens: variant, states, conductor (+0x4c, another SET object)
        wr8(b, obj + 3, (rng.next() % 25) as u8);
        wr8(b, obj + 4, [0u8, 0, 2, 2, 4, 6][rng.next() as usize % 6]);
        // valid sub-states of each variant (the others make the original jump into data)
        let variant = rd8(b, obj + 3);
        let sub_states = match variant {
            1 => 5,
            9 | 10 | 11 | 17 => 3,
            7 | 14 | 16 => 2,
            18 | 22 => 9,
            _ => 1,
        };
        wr8(b, obj + 5, 2 * (rng.next() % sub_states) as u8);
        // player targeted by variants 18 and 22, target object of 17
        let player = 0xff85de + 0x200 * (rng.next() % 4);
        wr16(b, obj + 0x20, player as u16);
        wr16(b, obj + 0x26, player as u16);
        wr16(b, obj + 0x46, player as u16);
        // dialogue window (variant 7): targeted player, message to compose, text buffer
        wr16(b, obj + 0x34, player as u16);
        wr32(b, obj + 8, 0xcec00 + 2 * (rng.next() % 0x400));
        wr32(b, obj + 0xc, 0xff2072);
        wr8(b, A5 + 0x73, [2, 1, 0][rng.next() as usize % 3]);
        wr16(b, A5 + 0x72, 2 + (rng.next() % 3) as u16);
        let leader = if obj == 0xffca9e { 0xffcaee } else { 0xffca9e };
        wr32(b, obj + 0x4c, leader);
        wr8(b, leader + 8, [0xff, 0, 1, 2, 3][rng.next() as usize % 5]);
        wr8(b, leader + 3, [0, 2, 3, 1][rng.next() as usize % 4]);
        wr8(b, leader + 0x3f, (rng.next() % 4) as u8);
        wr8(b, leader + 0xc, (rng.next() % 2) as u8);
        wr8(b, obj + 0x2f, (rng.next() % 8) as u8);
        return;
    }
    if !start || kind != 2 {
        return;
    }
    let variant = rng.next() % 0x78;
    wr8(b, obj + 3, variant as u8);
    let script = rd32(b, 0xbd276 + 4 * variant);
    let state = [0u8, 0, 2, 2, 2, 4, 4, 6][rng.next() as usize % 8];
    wr8(b, obj + 4, state);
    if state == 2 {
        wr16(b, obj + 0x18, 0xffff);
        let first = rd16(b, script);
        wr16(b, obj + 0x24, first);
        wr32(b, obj + 0x20, script + 2);
    } else if state == 4 {
        wr16(b, obj + 0x24, (rng.next() % 4) as u16);
        wr16(b, obj + 0x2a, rng.next().is_multiple_of(4) as u16);
        if rng.next().is_multiple_of(2) {
            // curtain (end action 6): already armed, short countdown, cursor in the table
            wr8(b, obj + 5, 2);
            wr16(b, obj + 0x14, (rng.next() % 3) as u16);
            wr16(b, obj + 0x26, 6 * (rng.next() % 9) as u16);
        }
    }
}
