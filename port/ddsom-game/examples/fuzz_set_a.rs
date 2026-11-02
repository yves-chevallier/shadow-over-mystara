//! Variant of fuzz.rs for batch SET 0-14 (options DDSOM_SCENE, DDSOM_CHAMPS, DDSOM_PROCHE, DDSOM_PROGRES).
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

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    /// True with probability 1/n.
    fn one_chance_in(&mut self, n: u32) -> bool {
        self.next() % n == 0
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
        _ => (0..8).fold(0, |v, _| v << 4 | rng.next() % 10),
    }
}

fn ring(b: &mut Bus, rng: &mut Rng, write: u32, cases: u32, read: bool) {
    wr8(b, A5 + write, (rng.next() as u8 & 0x3f) * 2);
    if read {
        wr8(b, A5 + write + 1, (rng.next() as u8 & 0x3f) * 2);
    }
    for i in 0..64 {
        let v = if rng.next() % 3 == 0 { (rng.next() as u16 & 0x37f) | 0x0000 } else { 0xffff };
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
        let plausible = (rng.next() % 4) as u16 * 0x100 | (rng.next() % 0x18) as u16;
        let base = A5 + 0x20e + 8 * i;
        wr16(b, base, if rng.next() % 3 == 0 { plausible } else { 0xffff });
        wr16(b, base + 2, rng.next() as u16 % 0x100);
        wr16(b, base + 4, if rng.next() & 1 == 0 { 0 } else { rng.next() as u16 % 0x100 });
        wr16(b, base + 6, 0x80);
    }
    // current pop-up: remaining duration 0, 1, 2, 3, 4, 5 or long
    let duration = [0u16, 1, 2, 3, 4, 5, 0x60][rng.next() as usize % 7];
    wr16(b, 0xff9180, (rng.next() % 4) as u16 * 0x100 | (rng.next() % 0x18) as u16);
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
    for i in 0..64 {
        let v = if rng.next() % 4 == 0 { data[i] ^ 1 } else { data[i] };
        wr16(b, 0xff1d8e + 2 * i as u32, if rng.next() % 3 == 0 { rng.next() as u16 } else { v });
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
        let small = if rng.next() % 3 == 0 { rng.next() as u8 } else { (rng.next() % 20) as u8 };
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
        let v = if rng.next() % 5 == 0 { colour(rng) } else { colour(rng) & 0xf0ff | (i as u16 & 0xf00) };
        wr16(b, 0x91c000 + 2 * i, v);
    }
    // palettes cibles en RAM (0xff1190, 512 couleurs)
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
    if rng.next() % 3 == 0 {
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
    c.d[0] = if rng.next() % 3 == 0 { rng.next() % 0x5b } else { rng.next() % 12 };
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
        c.d[0] = if rng.next() % 3 == 0 { rng.next() } else { rng.next() % 130 };
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
    wr16(b, cam + 0x20, if rng.next() % 4 == 0 { 4 } else { 0 });
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
/// `DDSOM_SCENE` mode of the families fuzz: perturbs scene variables (A5), the players and
/// the free fields of the object, with "interesting" values (0, 1, game thresholds, random).
fn perturb_scene(b: &mut Bus, rng: &mut Rng, obj: u32, stride: u32) {
    let near = std::env::var("DDSOM_PROCHE").is_ok();
    let choose = |rng: &mut Rng, special: &[u32]| -> u32 {
        match rng.next() % 4 {
            0 => 0,
            1 => 1,
            2 => special[rng.next() as usize % special.len()],
            _ => rng.next() as u32,
        }
    };
    // A5 byte variables: game in progress, pause, end-of-scene flags...
    for off in [0x35u32, 0x36, 0x42, 0x43, 0x4a, 0x50, 0x51, 0x34] {
        if rng.next() % 4 == 0 {
            let v = choose(rng, &[2, 4]) as u8;
            wr8(b, A5 + off, v);
        }
    }
    // player inputs: held keys (A5+0..3), pressed (A5+0xc..0xf), button states (A5+4, A5+0xa)
    for off in [0x0u32, 1, 2, 3, 4, 0xa, 0xc, 0xd, 0xe, 0xf] {
        if rng.next() % 3 == 0 {
            let v = match rng.next() % 4 {
                0 => 0,
                1 => 1 << (rng.next() % 8),
                2 => [0x10u8, 0x20, 0x70, 0x80, 0x90][rng.next() as usize % 5],
                _ => rng.next() as u8,
            };
            wr8(b, A5 + off, v);
        }
    }
    // word variables: progress, camera, game mode, free-slot counters
    for (off, special) in [(0x556u32, &[0x320u32, 0x321, 0x400][..]), (0x506, &[0x1c80, 0x1c7f, 0x2000]), (0x70, &[1]), (0x52, &[0]), (0x7e62, &[2, 3, 4])] {
        if rng.next() % 4 == 0 {
            let v = choose(rng, special) as u16;
            wr16(b, A5 + off, v);
        }
    }
    // players: presence, life, states
    for j in 0..4u32 {
        let p = 0xff85de + 0x200 * j;
        if !near && rng.next() % 3 == 0 {
            let present = rng.next() % 4 != 0;
            wr8(b, p, present as u8);
            wr16(b, p + 0x62, if rng.next() % 4 != 0 { 1 + (rng.next() % 99) as u16 } else { 0 });
            let st = [0x200u16, 0x0, 0x402, 0x400, 0x600, rng.next() as u16][rng.next() as usize % 6];
            wr16(b, p + 4, st);
            wr8(b, p + 6, (rng.next() % 3 == 0) as u8 * 2);
        }
    }
    // a player lands on the object, free to move (exit doors, proximity triggers)
    // (DDSOM_PROCHE=1: at every frame, for the dwell measurements)
    if near || rng.next() % 4 != 0 {
        let p = 0xff85de + 0x200 * (rng.next() % 4);
        for off in [0x8u32, 0x10] {
            let v = ddsom_game::mem::rd16(b, obj + off);
            wr16(b, p + off, v);
        }
        wr16(b, p + 0xc, 0);
        wr8(b, p, 1);
        wr16(b, p + 0x62, 1 + (rng.next() % 50) as u16);
        wr32(b, p + 4, [0x2000000u32, 0x2000600, 0x2002010][rng.next() as usize % 3]);
        wr8(b, p + 0x6c, 0xff);
        wr8(b, A5 - 0x5ac4, 0);
        for off in [0x66u32, 0x15c, 0x8a] {
            wr8(b, p + off, 0);
        }
    }
    // free fields of the object (beyond the common fields)
    // (DDSOM_CHAMPS=start-end[,start-end...] in hexadecimal bounds the perturbed offsets, 14-50 by default)
    let ranges: Vec<(u32, u32)> = std::env::var("DDSOM_CHAMPS")
        .ok()
        .map(|v| v.split(',').filter_map(|p| p.split_once('-').and_then(|(d, f)| Some((u32::from_str_radix(d, 16).ok()?, u32::from_str_radix(f, 16).ok()?)))).collect())
        .unwrap_or_else(|| vec![(0x14, 0x50)]);
    for off in ranges.iter().flat_map(|&(d, f)| d..f.min(stride)) {
        if rng.next() % 24 == 0 {
            // state machine states (bytes 4 to 7): even values, jump table indices
            let v = if off == 4 {
                2 * (rng.next() % 4) as u8
            } else if (5..8).contains(&off) {
                2 * (rng.next() % 8) as u8
            } else { choose(rng, &[2, 8, 0x10, 0xff]) as u8 };
            wr8(b, obj + off, v);
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

fn families(roms: &str, eeprom: &str, scenario: &str, warmup: u32, rounds: u32, filter: Option<u32>) {
    use ddsom_game::mem::{rd16, rd8};
    use ddsom_game::objects::pools::{Pool, ENEMY, ESHL, SET, SOBJ};
    use m68k::decode::Fetch;
    let ee = std::fs::read(eeprom).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(roms), ee.as_deref()).expect("roms");
    ddsom_game::install(&mut m);
    let script = ddsom_game::scenario::Scenario::load(Path::new(scenario)).expect("scenario");
    for f in 0..warmup as usize {
        script.apply(f, &mut m);
        m.run_frame();
    }
    ddsom_game::set_check(true);
    std::panic::set_hook(Box::new(|_| {}));
    let mut rng = Rng(0x1234_5678_9abc_def1);
    let tables: [(&Pool, u32, u32); 3] = [(&ESHL, 0x39b96, 64), (&SET, 0x39eac, 34), (&SOBJ, 0x39fa8, 16)];
    let mut inputs: Vec<(u32, &Pool, u8)> = Vec::new();
    for (pool, table, n) in tables {
        for k in 0..n {
            let a = (m.bus.fetch16(table + 4 * k) as u32) << 16 | m.bus.fetch16(table + 4 * k + 2) as u32;
            if ddsom_game::all_natives().any(|nat| nat.entry == a) && filter.map_or(true, |f| f == a) {
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
    let scene = std::env::var("DDSOM_SCENE").is_ok();
    let progress = std::env::var("DDSOM_PROGRES").is_ok();
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
            let obj_index = rng.next() % pool.slots;
            let obj = pool.slot(obj_index);
            for off in (0..pool.stride).step_by(2) {
                wr16(&mut b, obj.0 + off, 0);
            }
            let owner = owners[rng.next() as usize % owners.len()];
            wr8(&mut b, obj.0, 1);
            wr8(&mut b, obj.0 + 2, kind);
            wr8(&mut b, obj.0 + 3, (rng.next() % variants.min(table_variants(entry))) as u8);
            wr8(&mut b, obj.0 + 0x28, pool.class);
            wr8(&mut b, obj.0 + 0x2c, (rng.next() & 1) as u8);
            wr8(&mut b, obj.0 + 0x2f, if rng.next() % 4 == 0 { rng.next() as u8 % 3 } else { 0 });
            wr16(&mut b, obj.0 + 0x50, owner as u16);
            wr16(&mut b, obj.0 + 0x46, owner as u16);
            wr16(&mut b, obj.0 + 0x5a, owner as u16);
            // DDSOM_SCENE: the name entry board (SET 5, variant 0) needs its four cursors,
            // and a cursor of its player (+0x22) and its number (+0x2f)
            let mut slaves: Vec<u32> = Vec::new();
            if scene && pool.class == SET.class && kind == 5 {
                let player = |j: u32| 0xff85de + 0x200 * j;
                if rd8(&mut b, obj.0 + 3) == 0 {
                    for j in 0..4u32 {
                        let e = pool.slot((obj_index + 1 + j) % pool.slots);
                        for off in (0..pool.stride).step_by(2) {
                            wr16(&mut b, e.0 + off, 0);
                        }
                        wr8(&mut b, e.0, 1);
                        wr8(&mut b, e.0 + 2, 5);
                        wr8(&mut b, e.0 + 3, 1);
                        wr8(&mut b, e.0 + 0x28, pool.class);
                        wr16(&mut b, e.0 + 0x22, player(j) as u16);
                        wr8(&mut b, e.0 + 0x2f, j as u8);
                        wr16(&mut b, obj.0 + 0x18 + 2 * j, e.0 as u16);
                        slaves.push(e.0);
                    }
                } else {
                    let j = rng.next() as u32 % 4;
                    wr16(&mut b, obj.0 + 0x22, player(j) as u16);
                    wr8(&mut b, obj.0 + 0x2f, j as u8);
                }
            }
            for off in [0x8u32, 0xc, 0x10] {
                let v = (rd16(&mut b, owner + off) as u32) << 16 | (rng.next() & 0xffff);
                wr32(&mut b, obj.0 + off, v);
            }
            wr16(&mut b, obj.0 + 0xc, (rng.next() % 0x60) as u16);
            // consistent free-slot count
            // (at warm-up time the pool may be full: the handlers test this counter)
            wr16(&mut b, A5 + pool.free, (pool.slots / 2) as u16);
            for step in 0..60 {
                if rd8(&mut b, obj.0) == 0 {
                    break;
                }
                if step > 0 {
                    if rng.next() % 6 == 0 {
                        wr8(&mut b, obj.0 + 0x57, [0x08, 0x20, 0x28, 0x10, 0x02, 0x40][rng.next() as usize % 6]);
                    }
                    if rng.next() % 8 == 0 {
                        wr8(&mut b, obj.0 + 0x33, rng.next() as u8 & 1);
                    }
                    if rng.next() % 10 == 0 {
                        wr8(&mut b, owner + 4, (rng.next() % 8) as u8);
                    }
                    // the owner changes state: state-machine states, attack flags, counters
                    if rng.next() % 3 == 0 {
                        for off in [4u32, 5, 6, 7] {
                            if rng.next() % 2 == 0 {
                                wr8(&mut b, owner + off, 2 * (rng.next() % 6) as u8);
                            }
                        }
                        for off in [0x53u32, 0xa0, 0xa1, 0xa8, 0xa9, 0xb0, 0xbd, 0x3e, 0x2f, 0x42, 0x5d] {
                            if rng.next() % 3 == 0 {
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
                    if rng.next() % 12 == 0 {
                        wr8(&mut b, obj.0 + 0x3e, rng.next() as u8 % 6);
                    }
                    if rng.next() % 10 == 0 {
                        wr8(&mut b, obj.0 + 0x53, (rng.next() % 5) as u8);
                    }
                }
                // DDSOM_SCENE=1: scene variables (A5), players and free object fields drawn at random
                // (SET types that read the camera, progress, players...)
                if scene && (std::env::var("DDSOM_PROCHE").is_ok() || rng.next() % 2 == 0) {
                    perturb_scene(&mut b, &mut rng, obj.0, pool.stride);
                }
                // game paused (rare) or not; suspended object (byte +0x40) very rarely
                wr8(&mut b, A5 + 0x3d, (rng.next() % 10 == 0) as u8);
                wr8(&mut b, obj.0 + 0x40, (rng.next() % 40 == 0) as u8);
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
                    ddsom_game::mem::wr32(&mut b2, c2.a[7], 0xfffe);
                    for _ in 0..20_000 {
                        if c2.pc == 0xfffe {
                            break;
                        }
                        seen.insert(c2.pc);
                        c2.step(&mut b2);
                    }
                }
                let prev_obj: Vec<u8> = (0..pool.stride).map(|i| rd8(&mut b, obj.0 + i)).collect();
                if progress {
                    eprintln!("call {entry:06x} variant {} step {step} object {:02x?}", prev_obj[3], &prev_obj[..8]);
                }
                let mut r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| ddsom_game::call(&mut c, &mut b, entry)));
                // the name entry board cursors rotate too (same handler, A0 = the cursor)
                let after_master = ddsom_game::report().values().map(|s| s.mismatches).sum::<u64>();
                for &e in &slaves {
                    if r.is_ok() {
                        c.a[0] = e;
                        c.d[0] = 4 * kind as u32;
                        r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| ddsom_game::call(&mut c, &mut b, entry)));
                    }
                }
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
                    println!("  object before: {:02x?}{}", prev_obj, if after_master > before { " (mismatch from the main object)" } else { "" });
                    if std::env::var("DDSOM_TRACE_ORIG").is_ok() {
                        // step-by-step execution of the original code alone from the previous state
                        let (mut c2, mut b2) = (cs.clone(), bs.clone());
                        let sp = c2.a[7];
                        c2.pc = entry;
                        c2.a[7] = sp.wrapping_sub(4);
                        ddsom_game::mem::wr32(&mut b2, c2.a[7], 0xfffe);
                        for _ in 0..400 {
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
    println!("{tests} handler calls");
    if let Ok(file) = std::env::var("DDSOM_COUVERTURE") {
        let text: Vec<String> = seen.iter().map(|a| format!("{a:06x}")).collect();
        std::fs::write(file, text.join("\n")).expect("coverage write");
    }
    println!("{}", if total == 0 { "VALID: no mismatch" } else { "FAILED" });
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
    let (rng_state, range): (fn(&mut Cpu, &mut Bus, &mut Rng), std::ops::Range<u32>) = match group.as_str() {
        "sound" => (rng_sound, 0x68c..0xee000),
        "u1" => (rng_u1, 0x19a80..0x1a000),
        "c1" => (rng_c1, 0x13c00..0x14e50),
        "m2" => (rng_m2, 0x2d300..0x2ede0),
        "video" => (rng_video, 0xb40..0x1d100),
        "camera" => (rng_camera, 0x2f000..0x33400),
        _ => {
            eprintln!("groupe inconnu : {group}");
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
                format!(" var={var} copie={copy:02x?} defauts={block:02x?}")
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
    let mut total = 0;
    for (name, st) in ddsom_game::report() {
        total += st.mismatches;
        println!("  {name:28} {:>7} calls {:>5} mismatches{}", st.calls, st.mismatches,
            st.first.map(|f| format!(" — {f}")).unwrap_or_default());
    }
    println!("{}", if total == 0 { "VALID: no mismatch" } else { "FAILED" });
}
