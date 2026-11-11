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
//! camera (scenery copy by cameras 1 and 2, 0x2f81c 0x2f86e 0x328a8, and parallax scenery 0x33178...),
//! createurs (object creators of batch B: `effects_creators`, `creators`, `small`, `effects_aids`).

use std::path::Path;

use ddsom_game::world::mem::{wr16, wr32, wr8, A5};

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
    use ddsom_game::world::mem::{rd16, wr8};
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

/// Number of variants (byte `+3`) covered by the original jump table (0xcc25a, ESHL 5, 9, 15). Beyond that, the original
/// reads code or data as an offset and jumps anywhere: these cases are out of contract
/// (no game object has these variants) and are not drawn. 0xcc25a: one table entry (the
/// variant 1 falls back on the same target), variant 2 jumps into the middle of a `jsr`.
fn table_variants(entry: u32) -> u32 {
    match entry {
        0xcc25a => 2,
        0x9e458 | 0x9ef46 => 3,
        0x9fe0a => 2,
        _ => u32::MAX,
    }
}

/// Registers that the dispatch loop does not set (it sets A0, D7, A5, D0 = type × 4 and A1 =
/// handler): these are the leftovers of the previous handler.
const LEFTOVER_NAMES: [&str; 10] = ["d1", "d2", "d3", "d4", "d5", "d6", "a2", "a3", "a4", "d0h"];

/// Runs the original code alone from `entry` up to the `rts` (at most 50,000 instructions).
fn run_original(c0: &Cpu, b0: &Bus, entry: u32, patch: &dyn Fn(&mut Cpu)) -> Option<(Cpu, Bus)> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let (mut c, mut b) = (c0.clone(), b0.clone());
        patch(&mut c);
        c.pc = entry;
        c.a[7] = c.a[7].wrapping_sub(4);
        wr32(&mut b, c.a[7], 0xfffe);
        for _ in 0..50_000 {
            if c.pc == 0xfffe {
                c.a[7] = c.a[7].wrapping_add(4);
                return Some((c, b));
            }
            c.step(&mut b);
        }
        None
    }))
    .ok()
    .flatten()
}

/// Replays the original twice in parallel (different leftover register `r` of `LEFTOVER_NAMES`) and returns
/// the address of the first instruction after which the state diverges (other than the register
/// itself): this is the instruction that reads the leftover.
fn read_site(c: &Cpu, b: &Bus, entry: u32, r: usize, v1: &[u32; 10], v2: &[u32; 10], set: fn(&mut Cpu, &[u32; 10])) -> Option<String> {
    let (mut c1, mut b1) = (c.clone(), b.clone());
    let (mut c2, mut b2) = (c.clone(), b.clone());
    set(&mut c1, v1);
    set(&mut c2, v2);
    for (c, b) in [(&mut c1, &mut b1), (&mut c2, &mut b2)] {
        c.pc = entry;
        c.a[7] = c.a[7].wrapping_sub(4);
        wr32(b, c.a[7], 0xfffe);
    }
    let mut story: Vec<u32> = Vec::new();
    for step in 0..20_000 {
        if c1.pc == 0xfffe && c2.pc == 0xfffe {
            return None;
        }
        let pc = c1.pc;
        story.push(pc);
        let ins = m68k::decode(&mut b1, pc);
        c1.step(&mut b1);
        c2.step(&mut b2);
        let mut d1 = c1.d;
        let mut d2 = c2.d;
        let (mut a1, mut a2) = (c1.a, c2.a);
        match r {
            0..=5 => {
                d1[r + 1] = 0;
                d2[r + 1] = 0;
            }
            6..=8 => {
                a1[r - 4] = 0;
                a2[r - 4] = 0;
            }
            _ => {
                d1[0] &= 0xffff;
                d2[0] &= 0xffff;
            }
        }
        let stack = |i: usize| {
            let a = 2 * i;
            (0x0a00..0x1000).contains(&a) || (0x2900..0x2a1a).contains(&a)
        };
        let ram = (0..b1.ram.len()).find(|&i| b1.ram[i] != b2.ram[i] && !stack(i));
        if c1.pc != c2.pc || d1 != d2 || a1[..7] != a2[..7] || ram.is_some() || b1.gfxram != b2.gfxram {
            let end = story.len().saturating_sub(400);
            let h: Vec<String> = story[end..].iter().map(|p| format!("{p:x}")).collect();
            return Some(format!("step {step} at {pc:06x} {:?}{} after {}", ins.op, ram.map_or(String::new(), |i| format!(" (ram {:06x})", 0xff0000 + 2 * i)), h.join(" ")));
        }
    }
    None
}

/// Looks for the registers that the handler reads before writing them: the original code is run
/// several times from the same memory state, changing a single leftover register at a time
/// (D1-D6, A2-A4, high byte of D0 which the dispatch zeroes); if the final memory changes, the
/// handler depends on that register. Returns one flag per register of `LEFTOVER_NAMES` and the state at the
/// end of the reference run (to continue the trajectory).
fn leftovers(c: &Cpu, b: &Bus, entry: u32, rng: &mut Rng) -> ([bool; 10], Option<(Cpu, Bus)>) {
    let draw = |rng: &mut Rng| -> [u32; 10] {
        let mut v = [0u32; 10];
        for (i, x) in v.iter_mut().enumerate() {
            *x = match i {
                0..=5 => if rng.next() & 3 == 0 { rng.next() & 0xff } else { rng.next() },
                6..=8 => 0xff0000 | (rng.next() & 0xfffe),
                _ => rng.next() & 0xff00,
            };
        }
        v
    };
    fn set(c: &mut Cpu, v: &[u32; 10]) {
        c.d[1..7].copy_from_slice(&v[..6]);
        c.a[2..5].copy_from_slice(&v[6..9]);
        c.d[0] = (c.d[0] & 0xffff) | (v[9] << 16);
    }
    let base = draw(rng);
    let Some((cr, br)) = run_original(c, b, entry, &|c| set(c, &base)) else {
        return ([false; 10], None);
    };
    let mut res = [false; 10];
    for r in 0..10 {
        for _ in 0..2 {
            let mut v = base;
            v[r] = draw(rng)[r];
            if v[r] == base[r] {
                continue;
            }
            if let Some((_, b2)) = run_original(c, b, entry, &|c| set(c, &v)) {
                // the stacks (registers saved by movem: 68000 stack and sound driver stack at
                // 0xff2900-0xff2a1a) keep the leftovers: like `validate`, they are ignored
                let stack = |i: usize| {
                    let a = 2 * i;
                    (0x0a00..0x1000).contains(&a) || (0x2900..0x2a1a).contains(&a) || ((0x64..0x164).contains(&a) && (a - 0x64) % 16 < 4)
                };
                let ram_diff = (0..b2.ram.len()).any(|i| b2.ram[i] != br.ram[i] && !stack(i));
                if ram_diff || b2.gfxram != br.gfxram {
                    if std::env::var("DDSOM_RESTES_SITE").is_ok() {
                        if let Some(site) = read_site(c, b, entry, r, &base, &v, set) {
                            eprintln!("RESTES_SITE {entry:06x} {} {site}", LEFTOVER_NAMES[r]);
                        }
                    }
                    if std::env::var("DDSOM_RESTES_TRACE").is_ok() {
                        let i = (0..b2.ram.len()).find(|&i| b2.ram[i] != br.ram[i] && !stack(i));
                        eprintln!("RESTES_TRACE {entry:06x} {} ram {:?} sp {:06x}", LEFTOVER_NAMES[r], i.map(|i| format!("{:06x} {:04x}/{:04x}", 0xff0000 + 2 * i, b2.ram[i], br.ram[i])), c.a[7]);
                    }
                    res[r] = true;
                }
            }
        }
    }
    (res, Some((cr, br)))
}

fn families(roms: &str, eeprom: &str, scenario: &str, warmup: u32, rounds: u32, filter: Option<u32>) {
    use ddsom_game::world::mem::{rd16, rd8};
    use ddsom_game::world::objects::pools::{Pool, BOX, EFFCT, ENEMY, ESHL, ITEM, SET, SOBJ};
    let ee = std::fs::read(eeprom).ok();
    let m = cps2::Machine::load_ddsom(Path::new(roms), ee.as_deref()).expect("roms");
    let mut m = ddsom_game::install(m);
    let script = ddsom_game::core::scenario::Scenario::load(Path::new(scenario)).expect("scenario");
    for f in 0..warmup as usize {
        script.apply(f, &mut m);
        m.run_frame();
    }
    ddsom_game::set_check(true);
    // DDSOM_CHECK_ALL=1: also compares all output registers (REGS lines of the report)
    ddsom_game::set_check_all(std::env::var("DDSOM_CHECK_ALL").is_ok());
    std::panic::set_hook(Box::new(|_| {}));
    let mut rng = Rng(0x1234_5678_9abc_def1);
    // DDSOM_RESTES=1: looks for handlers that read a register left by the previous one (see
    // `leftovers`), in the six tables; otherwise, checks the native against the original
    let leftovers_active = std::env::var("DDSOM_RESTES").is_ok();
    let mut tables: Vec<(&Pool, u32, u32)> = vec![(&ESHL, 0x39b96, 64), (&SET, 0x39eac, 34), (&SOBJ, 0x39fa8, 16)];
    if leftovers_active {
        tables.extend([(&ITEM, 0x39f5a, 10), (&BOX, 0x39e76, 4), (&EFFCT, 0x39ce0, 64)]);
    }
    let mut leftovers_summary: std::collections::BTreeMap<u32, (u64, [u64; 10])> = Default::default();
    let mut inputs: Vec<(u32, &Pool, u8)> = Vec::new();
    for (pool, table, n) in tables {
        for k in 0..n {
            let a = (m.bus.hw.fetch16(table + 4 * k) as u32) << 16 | m.bus.hw.fetch16(table + 4 * k + 2) as u32;
            if ddsom_game::all_natives().any(|nat| nat.entry == a) && filter.is_none_or(|f| f == a) {
                inputs.push((a, pool, k as u8));
            }
        }
    }
    let (c0, b0) = (m.cpu.clone(), m.bus.to_bus());
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
            let owner = owners[rng.next() as usize % owners.len()];
            wr8(&mut b, obj.0, 1);
            wr8(&mut b, obj.0 + 2, kind);
            wr8(&mut b, obj.0 + 3, (rng.next() % variants.min(table_variants(entry))) as u8);
            wr8(&mut b, obj.0 + 0x28, pool.class);
            wr8(&mut b, obj.0 + 0x2c, (rng.next() & 1) as u8);
            wr8(&mut b, obj.0 + 0x2f, if rng.next().is_multiple_of(4) { rng.next() as u8 % 3 } else { 0 });
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
                    if rng.next().is_multiple_of(10) {
                        wr8(&mut b, owner + 4, (rng.next() % 8) as u8);
                    }
                    // the owner changes state: state-machine states, attack flags, counters
                    if rng.next().is_multiple_of(3) {
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
                let before = ddsom_game::report().values().map(|s| s.mismatches).sum::<u64>();
                let (cs, bs) = (c.clone(), b.clone());
                if coverage {
                    // addresses of the instructions the original executes in this call (interpreter)
                    let (mut c2, mut b2) = (cs.clone(), bs.clone());
                    c2.pc = entry;
                    c2.a[7] = cs.a[7].wrapping_sub(4);
                    ddsom_game::world::mem::wr32(&mut b2, c2.a[7], 0xfffe);
                    for _ in 0..20_000 {
                        if c2.pc == 0xfffe {
                            break;
                        }
                        seen.insert(c2.pc);
                        c2.step(&mut b2);
                    }
                }
                if leftovers_active {
                    let (res, next) = leftovers(&cs, &bs, entry, &mut rng);
                    let e = leftovers_summary.entry(entry).or_default();
                    e.0 += 1;
                    for (t, r) in e.1.iter_mut().zip(res) {
                        *t += r as u64;
                    }
                    match next {
                        Some((c2, b2)) => (c, b) = (c2, b2),
                        None => break,
                    }
                    continue;
                }
                let prev_obj: Vec<u8> = (0..pool.stride).map(|i| rd8(&mut b, obj.0 + i)).collect();
                let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| ddsom_game::call(&mut c, &mut b, entry)));
                if r.is_err() {
                    // the original went off the rails (inconsistent entry): the trajectory is abandoned
                    let _ = (cs, bs);
                    break;
                }
                let after = ddsom_game::report().values().map(|s| s.mismatches).sum::<u64>();
                if after > before && shown < 12 {
                    shown += 1;
                    let last = ddsom_game::report().values().filter_map(|s| s.last.clone()).next();
                    println!("mismatch at {entry:06x} type {kind} step {step}: {}", last.unwrap_or_default());
                    println!("  object before: {:02x?}", prev_obj);
                    if std::env::var("DDSOM_TRACE_ORIG").is_ok() {
                        // step-by-step execution of the original code alone from the previous state
                        let (mut c2, mut b2) = (cs.clone(), bs.clone());
                        let sp = c2.a[7];
                        c2.pc = entry;
                        c2.a[7] = sp.wrapping_sub(4);
                        ddsom_game::world::mem::wr32(&mut b2, c2.a[7], 0xfffe);
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
    for (entry, (calls, regs)) in &leftovers_summary {
        let native_name = ddsom_game::all_natives().find(|n| n.entry == *entry).map_or("?", |n| n.name);
        let list: Vec<String> =
            regs.iter().enumerate().filter(|(_, &n)| n > 0).map(|(i, n)| format!("{}:{n}", LEFTOVER_NAMES[i])).collect();
        println!("LEFTOVERS {entry:06x} {native_name} {calls} calls: {}", if list.is_empty() { "none".into() } else { list.join(" ") });
    }
    if std::env::var("DDSOM_CHECK_ALL").is_ok() {
        for (name, st) in ddsom_game::report() {
            let list: Vec<String> = st
                .regs
                .iter()
                .enumerate()
                .filter(|(_, &n)| n > 0)
                .map(|(i, n)| format!("{}{}:{n}", if i < 8 { 'd' } else { 'a' }, i % 8))
                .collect();
            if !list.is_empty() {
                println!("REGS {name} {} calls: {}", st.calls, list.join(" "));
            }
        }
    }
    let mut total = 0;
    for (name, st) in ddsom_game::report() {
        total += st.mismatches;
        println!("  {name:28} {:>7} calls {:>5} mismatches{}", st.calls, st.mismatches,
            st.first.map(|f| format!(" — {f}")).unwrap_or_default());
    }
    println!("{tests} handler calls");
    if let Ok(file) = std::env::var("DDSOM_COUVERTURE") {
        let text: Vec<String> = seen.iter().map(|a| format!("{a:06x}")).collect();
        std::fs::write(file, text.join("\n")).expect("coverage write");
    }
    println!("{}", if total == 0 { "VALID: no mismatch" } else { "FAILED" });
}

/// Small routines called by the family handlers (creators, palettes, tile cells, cameras
/// on the zone return).
const SMALL: [u32; 8] = [0x101da, 0x1ac90, 0x4aed0, 0x53746, 0xb230e, 0xa462c, 0xa2b4a, 0x33b24];

fn rng_small_effects(c: &mut Cpu, b: &mut Bus, rng: &mut Rng) {
    use ddsom_game::world::objects::pools::{PEFF, PSHL, ESHL};
    regs(c, b, rng);
    let pool = [&PEFF, &PSHL, &ESHL][rng.next() as usize % 3];
    c.a[0] = if rng.one_chance_in(4) { 0xff85de + 0x200 * (rng.next() % 4) } else { pool.slot(rng.next() % pool.slots).0 };
    c.a[1] = 0x910000 + 4 * (rng.next() % 0xf00);
    c.a[5] = A5;
    wr8(b, A5 + 0x36, (rng.next() % 2) as u8);
    wr8(b, A5 + 0x34, (rng.next() % 0x6c) as u8);
    for p in [&PEFF, &PSHL, &ESHL] {
        // free slots: sometimes none (creation fails along the way)
        let free = if rng.one_chance_in(5) { rng.next() % 3 } else { p.slots / 2 };
        wr16(b, A5 + p.free, free as u16);
    }
}

/// Object creators of batch B (`families::effects_creators`, `creators`, `small`, `effects_aids`): creator taken
/// from a pool or among the players, registers and free slots of the drawn pools (sometimes none: creation fails
/// en cours de route).
fn rng_creators(c: &mut Cpu, b: &mut Bus, rng: &mut Rng) {
    use ddsom_game::world::objects::pools::POOLS;
    regs(c, b, rng);
    let pool = POOLS[rng.next() as usize % POOLS.len()];
    c.a[0] = if rng.one_chance_in(4) { 0xff85de + 0x200 * (rng.next() % 4) } else { pool.slot(rng.next() % pool.slots).0 };
    c.a[1] = 0x910000 + 4 * (rng.next() % 0xf00);
    c.a[4] = 0xff0000 + (rng.next() % 0x8000);
    c.a[5] = A5;
    wr8(b, A5 + 0x36, (rng.next() % 2) as u8);
    wr8(b, A5 + 0x34, (rng.next() % 0x6c) as u8);
    for p in POOLS {
        let free = if rng.one_chance_in(5) { rng.next() % 3 } else { p.slots / 2 };
        wr16(b, A5 + p.free, free as u16);
    }
}

/// Entries of the `creator_entries` group (without the table dispatches and the scene reset, which call code
/// chosen by the state and are not creators).
fn creator_entries() -> Vec<u32> {
    use ddsom_game::tools::families::{creators, effects_creators, small};
    // the sparks placed between two objects read their collision boxes: on randomly drawn objects,
    // the original loops forever (out of contract), they are validated by the scenarios
    let excluded = [0xa8ae8, 0xa8af8, 0xd981a, 0xd9778, 0xcc0ca, 0x180ac, 0x1c44a, 0xaa96c, 0xaaa04, 0xaaa84];
    effects_creators::NATIVES.iter().chain(creators::NATIVES).chain(small::NATIVES).chain(ddsom_game::tools::effects_aids::NATIVES)
        .map(|n| n.entry)
        .filter(|e| !excluded.contains(e))
        .collect()
}

/// Shop: the item on display (PEFF 42) and the counter (PEFF 43) with the merchant dialogue interpreter, on a
/// customer (PEFF object whose `+0x50` designates a player) with inventory, purse and menu state drawn; the dialogue
/// script is written in an ITEM slot (valid commands, forward jumps, ending with a stop).
fn rng_shop(c: &mut Cpu, b: &mut Bus, rng: &mut Rng) {
    use ddsom_game::world::objects::pools::{ITEM, PEFF};
    let entry = ENTRY.load(std::sync::atomic::Ordering::Relaxed);
    regs(c, b, rng);
    let me = PEFF.slot(rng.next() % 24).0;
    let parent = PEFF.slot(24 + rng.next() % 12).0;
    let client = PEFF.slot(36 + rng.next() % 12).0;
    let player = 0xff85de + 0x200 * (rng.next() % 4);
    c.a[0] = me;
    c.a[2] = client;
    c.a[5] = A5;
    c.d[0] &= 0xff; // the counter leaves a small position offset in D0.w
    wr16(b, client + 0x50, player as u16);
    wr8(b, client + 0x58, if rng.one_chance_in(4) { 0 } else { 1 + rng.next() as u8 % 3 });
    wr8(b, player + 0x3, (rng.next() % 12) as u8);
    wr8(b, player + 0x10d, [0, 8, 9, 0x1d, 0x1e, 3][rng.next() as usize % 6]);
    wr8(b, player + 0x167, [0, 1, 3, 4, 5, 8, 0xa, 0xff, 2][rng.next() as usize % 9]);
    for k in 0..6 {
        let item = [0x56, 0x56, 0x2b, 0x39, 0x4e, 0x51, 0x10, 0x20][rng.next() as usize % 8];
        wr8(b, player + 0x140 + k, item);
        wr8(b, player + 0x148 + k, (rng.next() % 11) as u8);
    }
    for k in 0..6 {
        wr8(b, player + 0x112 + k, bcd(rng) as u8);
    }
    wr16(b, A5 - 0x554e, if rng.one_chance_in(2) { 0 } else { 0x80 });
    wr8(b, A5 + 0x5e, (rng.next() % 2) as u8);
    if entry == 0x50b9c {
        wr8(b, me, 1);
        wr8(b, me + 2, 42);
        wr8(b, me + 3, (rng.next() % 12) as u8);
        wr8(b, me + 4, [0, 2, 2, 2][rng.next() as usize % 4]);
        wr16(b, me + 0x5c, parent as u16);
        wr8(b, parent + 6, if rng.one_chance_in(6) { 0 } else { 2 });
        wr8(b, A5 - 0x553f, (rng.next() % 2) as u8);
        wr8(b, A5 - 0x5540, (rng.next() % 2) as u8);
        wr8(b, me + 0x2f, [0, 0, 1, 2, 0xff][rng.next() as usize % 5]);
        wr8(b, me + 0x1a, (rng.next() % 2) as u8);
        for k in 0..6 {
            wr8(b, me + 0x4e + k, bcd(rng) as u8 & if k < 3 { 0 } else { 0xff });
        }
        // customers: the list of the four players, ours often in the zone
        for k in 0..4u32 {
            let w = if k == 0 || rng.one_chance_in(3) { client as u16 } else { 0 };
            wr16(b, 0xff2aa4 + 2 * k, w);
        }
        for off in [0x8u32, 0xc, 0x10] {
            let v = ddsom_game::world::mem::rd16(b, me + off).wrapping_add((rng.next() % 0x20) as u16);
            wr16(b, client + off, v);
        }
        for (off, v) in [(0x14u32, 0x20u16), (0x16, 0x40), (0x54, 0x20), (0x56, 0x40)] {
            wr16(b, me + off, if rng.one_chance_in(5) { rng.next() as u16 & 0xff } else { v });
        }
        wr16(b, me + 0x5a, if rng.one_chance_in(2) { 0 } else { 0x90 });
    } else {
        let script = ITEM.slot(rng.next() % ITEM.slots).0;
        let n = 1 + rng.next() % 12;
        for k in 0..20u32 {
            let at = script + 4 * k;
            let (op, jump) = if k < n {
                ([0u8, 0, 4, 8, 0xc, 0xc, 0x10, 0x14, 0x18, 0x1c, 0x20, 0x24, 0x28][rng.next() as usize % 13], 4 * (rng.next() % 3) as u16)
            } else {
                (4, 0)
            };
            wr8(b, at, op);
            wr8(b, at + 1, (rng.next() % 2) as u8);
            wr16(b, at + 2, if op == 0 { 0x80 + rng.next() as u16 % 0x20 } else { jump });
        }
        // the PEFF 43 counter (0x51076) calls the interpreter (0x514ac / 0x514cc, Rust functions) for each player
        // ready in one of its two zones: a customer in the list, at its position, and the script in both zones
        wr8(b, me, 1);
        wr8(b, me + 2, 43);
        wr8(b, me + 4, 2);
        for k in 0..4u32 {
            let w = if k == 0 || rng.one_chance_in(3) { client as u16 } else { 0 };
            wr16(b, 0xff2aa4 + 2 * k, w);
        }
        for off in [0x8u32, 0xc, 0x10] {
            let v = ddsom_game::world::mem::rd16(b, me + off).wrapping_add((rng.next() % 0x10) as u16);
            wr16(b, client + off, v);
        }
        for (off, v) in [(0x14u32, 0x20u16), (0x16, 0x40), (0x18, 0x20), (0x1a, 0x40), (0x50, 0x20), (0x52, 0x40), (0x54, 0x20), (0x56, 0x40)] {
            wr16(b, me + off, if rng.one_chance_in(5) { rng.next() as u16 & 0xff } else { v });
        }
        for field in [0x1c, 0x58] {
            wr32(b, me + field, script);
        }
    }
}

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
        "small" => (rng_small_effects, 0x0..0x100000),
        "shop" => (rng_shop, 0x50b9c..0x514d0),
        "creators" => (rng_creators, 0x0..0x100000),
        _ => {
            eprintln!("unknown group: {group}");
            std::process::exit(2);
        }
    };
    let ee = std::fs::read(eeprom).ok();
    let m = cps2::Machine::load_ddsom(Path::new(roms), ee.as_deref()).expect("roms");
    let mut m = ddsom_game::install(m);
    let script = ddsom_game::core::scenario::Scenario::load(Path::new(scenario)).expect("scenario");
    for f in 0..warmup.parse().unwrap() {
        script.apply(f, &mut m);
        m.run_frame();
    }
    ddsom_game::set_check(true);
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    // the sound dispatches choose their target in a table according to a register or the object: drawn at
    // random, these selectors leave the tables (out of contract); they are validated by the scenarios
    let inputs: Vec<u32> = ddsom_game::all_natives()
        .filter(|n| n.name != "sound_dispatch")
        .map(|n| n.entry)
        .filter(|e| range.contains(e) && (*group != "sound" || *e == 0x68c || *e >= 0xe0836))
        .filter(|e| *group != "small" || SMALL.contains(e))
        .filter(|e| *group != "creators" || creator_entries().contains(e))
        .filter(|e| *group != "shop" || matches!(*e, 0x50b9c | 0x51076))
        .filter(|e| *group != "camera" || matches!(*e, 0x2f81c | 0x2f86e | 0x328a8 | 0x33178 | 0x33196 | 0x331e8 | 0x3326c))
        .filter(|e| *group != "video" || matches!(*e, 0xb40 | 0xb46 | 0xb96 | 0xbac | 0xbc6 | 0xbfc | 0xc58 | 0xca8 | 0xcc8 | 0xce8 | 0x158e | 0x15c8 | 0x1602 | 0x164e | 0x1a296 | 0x1a350 | 0x1a360 | 0x1a366 | 0x1a3d4 | 0x1a608 | 0x1a60a | 0x1a678 | 0x1a67e | 0x1a8ac | 0x1a8b2 | 0x1cd32 | 0x1cd7c | 0x1cdd6 | 0x1cf3c | 0x1cfb8 | 0x1cfc8 | 0x1d00e | 0x1d042 | 0x24a8 | 0x24b0 | 0x255a | 0x25e8 | 0x260c | 0x26d4 | 0x2766))
        .collect();
    let mut bus = m.bus.to_bus();
    let (c, b) = (&mut m.cpu, &mut bus);
    b.irq_horizon = u64::MAX;
    let mut shown = 0;
    for _ in 0..rounds.parse::<u32>().unwrap() {
        for &entry in &inputs {
            ENTRY.store(entry, std::sync::atomic::Ordering::Relaxed);
            rng_state(c, b, &mut rng);
            let (d0, a0) = (c.d[0], c.a[0]);
            let context = if entry == 0x2d662 {
                let var = ddsom_game::world::mem::rd16(b, A5 + 0x70);
                let copy: Vec<u8> = (0..18).map(|i| ddsom_game::world::mem::rd8(b, 0xff1d8e + i)).collect();
                let block: Vec<u8> = (0..18).map(|i| ddsom_game::world::mem::rd8(b, c.a[6] + i)).collect();
                format!(" var={var} copy={copy:02x?} defaults={block:02x?}")
            } else if *group == "camera" {
                let cam = c.a[0];
                let fields: Vec<String> = [0x08u32, 0x0c, 0x10, 0x14, 0x20, 0x2e, 0x34, 0x40]
                    .iter()
                    .map(|o| format!("{o:02x}={:04x}", ddsom_game::world::mem::rd16(b, cam + o)))
                    .collect();
                format!(" scene={:02x} terrain={} {}", ddsom_game::world::mem::rd8(b, A5 + 0x34), ddsom_game::world::mem::rd8(b, A5 + 0x37), fields.join(" "))
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
    let mut total = 0;
    for (name, st) in ddsom_game::report() {
        total += st.mismatches;
        println!("  {name:28} {:>7} calls {:>5} mismatches{}", st.calls, st.mismatches,
            st.first.map(|f| format!(" — {f}")).unwrap_or_default());
    }
    println!("{}", if total == 0 { "VALID: no mismatch" } else { "FAILED" });
}

/// Scene controller (SET 2) and end-of-level screens (SET 15): they depend on
/// game variables (level, difficulty, frozen scene...), the variant (script) and the player state,
/// which the generic fuzz does not vary. `start`: initial object state (variant, state).
fn scene_state(b: &mut Bus, rng: &mut Rng, obj: u32, kind: u8, start: bool) {
    use ddsom_game::world::mem::{rd16, rd32, rd8};
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
