//! Differential fuzz of the rewritten bosses (BOSS class, table 0x39b12), modeled on `fuzz_enemy.rs`:
//! a new object of the type is placed in the BOSS pool, its state is drawn from the original's jump tables
//! (`boss_states`), the variables the engine modifies (camera, players, scene flags) are perturbed
//! at each step, and each handler call is compared with the original (control mode of `ddsom_game`).
//!
//!   fuzz_boss <roms> <eeprom> <scenario> <warm-up frames> <rounds>
//!
//! Variables: `DDSOM_GRAINE=n` (seed), `DDSOM_ENTREE=address` (a single handler), `DDSOM_PAS=n` (steps per
//! trajectory, 60), `DDSOM_ETATS=1` (drawn states), `DDSOM_COUVERTURE=file` (original instructions seen).

use std::path::Path;

use ddsom_game::mem::{rd16, rd32, rd8, wr16, wr32, wr8, A5};
use ddsom_game::objects::pools::{BOSS, ENEMY, POOLS};

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
}

type Bus = cps2::Cps2Bus;

const Z: &[u8] = &[0];

/// States (`+4`, `+5`, `+6`) that the original's jump tables accept for a boss type.
fn boss_states(kind: u8) -> &'static [(&'static [u8], &'static [u8], &'static [u8])] {
    match kind {
        // boss 0: scene entrances, combat, attacks, waiting, hit state
        0 => &[
            (&[0], Z, Z),
            (&[2], &[0], &[0, 2, 4]),
            (&[2], &[2], &[0, 2, 4, 6, 8]),
            (&[2], &[4], &[0, 2, 4, 6, 8, 10, 12]),
            (&[2], &[6, 8, 10], &[0, 2]),
            (&[4], &[0], &[0, 2]),
            (&[4], &[2], &[0, 2, 4]),
            (&[4], &[4], &[0, 2, 4]),
            (&[4], &[6, 8], &[0, 2, 4, 6, 8]),
            (&[4], &[10], &[0, 2]),
            (&[6], Z, Z),
        ],
        // boss 16: awakening, combat, attacks, leap, waiting, pause, grab, hit state
        16 => &[
            (&[0], Z, Z),
            (&[2], &[0], &[0, 2]),
            (&[2], &[2], &[0, 2, 4, 6, 8]),
            (&[2], &[4], &[0, 2, 4, 6, 8, 10, 12, 14]),
            (&[2], &[6], &[0, 2, 4]),
            (&[2], &[8], &[0, 2, 4]),
            (&[2], &[10], &[0, 2]),
            (&[2], &[12], Z),
            (&[4], &[0], &[0, 2, 4, 6]),
            (&[4], &[2], &[0, 2]),
            (&[4], &[4], &[0, 2, 4]),
            (&[4], &[6], &[0, 2, 4, 6, 8]),
            (&[4], &[8, 10], Z),
            (&[6], Z, Z),
        ],
        // bosses 2 and 22: arrival, combat, attacks, runs, pause, grab, transformation, hit state
        2 | 22 => &[
            (&[0], Z, Z),
            (&[2], &[0], Z),
            (&[2], &[2], &[0, 2, 4, 6, 8]),
            (&[2], &[4], &[0, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 22]),
            (&[2], &[6], &[0, 2, 4, 6]),
            (&[2], &[8], &[0, 2, 4, 6, 8]),
            (&[2], &[10], Z),
            (&[2], &[12], &[0]),
            (&[2], &[14], &[0, 2, 4, 6, 8, 10, 12]),
            (&[4], &[0], &[0, 2]),
            (&[4], &[2], &[0, 2]),
            (&[4], &[4], &[0, 2, 4]),
            (&[4], &[6], &[0, 2, 4]),
            (&[4], &[8, 10], Z),
            (&[6], Z, Z),
        ],
        // flying dragon: +5 = 0 (steps +6 from 0 to 14) or 2 (release)
        13 => &[(&[0], Z, Z), (&[2], &[0], &[0, 2, 4, 6, 8, 10, 12, 14]), (&[2], &[2], Z), (&[4, 6], Z, Z)],
        // boss 17: groups (+5) and steps (+6) of the original's jump tables
        17 => &[
            (&[0], Z, Z),
            (&[2], &[0], &[0, 2, 4, 6, 8]),
            (&[2], &[2], &[0, 2, 4, 6, 8, 10]),
            (&[2], &[4], &[0, 2, 4, 6, 8, 10, 12, 14, 16]),
            (&[2], &[6, 8, 10], &[0, 2]),
            (&[4], &[0], Z),
            (&[4], &[2], &[0, 2, 4]),
            (&[4], &[4], &[0, 2, 4, 6]),
            (&[4], &[6, 8], &[0, 2, 4, 6, 8, 10]),
            (&[4], &[10], Z),
            (&[6], Z, Z),
        ],
        // boss 6 (Manticore)
        6 => &[
            (&[0], Z, Z),
            (&[2], &[0], &[0, 2, 4, 6, 8, 10, 12, 14]),
            (&[2], &[2], &[0, 2, 4, 6, 8, 10]),
            (&[2], &[4], &[0, 2, 4, 6, 8]),
            (&[2], &[6, 8, 10, 12], &[0, 2]),
            (&[2], &[14], Z),
            (&[4], &[0], Z),
            (&[4], &[2], &[0, 2, 4]),
            (&[4], &[4], &[0, 2, 4]),
            (&[4], &[6, 8], &[0, 2, 4, 6, 8]),
            (&[4], &[10], Z),
            (&[6], Z, Z),
        ],
        // boss 10 (Nagpa)
        10 => &[
            (&[0], Z, Z),
            (&[2], &[0], &[0, 2, 4, 6]),
            (&[2], &[2], &[0, 2, 4, 6]),
            (&[2], &[4], &[0]),
            (&[2], &[4], &[2]),
            (&[2], &[6], &[0, 2]),
            (&[4], &[0], &[0, 2, 4]),
            (&[4], &[2], Z),
            (&[4], &[4], &[0, 2, 4]),
            (&[4], &[6, 8], &[0, 2, 4, 6, 8]),
            (&[4], &[10], Z),
            (&[6], Z, Z),
        ],
        // boss 5 (Ezerhorden)
        5 => &[
            (&[0], Z, Z),
            (&[2], &[0], &[0, 4, 8, 12]),
            (&[2], &[4], &[0, 4]),
            (&[2], &[8, 20], &[0, 4]),
            (&[2], &[12], &[0, 4, 8, 12]),
            (&[2], &[16], &[0, 4]),
            (&[2], &[24], &[0, 4, 8, 12]),
            (&[4], &[0, 2, 4], &[0, 4]),
            (&[4], &[0, 2, 4], &[0, 2, 4, 6, 8, 10, 12, 14, 16]),
            (&[4], &[6], &[0, 2]),
            (&[6], Z, Z),
        ],
        // boss 21
        21 => &[
            (&[0], Z, Z),
            (&[2], &[0], &[0, 2]),
            (&[2], &[2], &[0, 2, 4, 6, 8, 10]),
            (&[2], &[4], &[0, 2, 4, 6, 8, 10, 12, 14, 16]),
            (&[2], &[6, 8, 10], &[0, 2]),
            (&[4], &[0], &[0, 2, 4]),
            (&[4], &[2], &[0, 2, 4]),
            (&[4], &[4], &[0, 2, 4]),
            (&[4], &[6, 8], &[0, 2, 4, 6, 8, 10]),
            (&[4], &[10], Z),
            (&[6], Z, Z),
        ],
        // dragons 14 and 19
        14 | 19 => &[
            (&[0], Z, Z),
            (&[2], &[0], &[0, 2, 4, 6, 8, 10]),
            (&[2], &[2], &[0, 2, 4, 6, 8]),
            (&[2], &[4], &[0, 2, 4, 6, 8, 10]),
            (&[2], &[6, 8, 10], Z),
            (&[4], &[0, 4], &[0, 2]),
            (&[4], &[2], &[0, 2]),
            (&[4], &[6], &[0, 2, 4]),
            (&[6], Z, Z),
        ],
        // red dragons 15 and 20
        15 | 20 => &[
            (&[0], Z, Z),
            (&[2], &[0], &[0, 2, 4]),
            (&[2], &[2], &[0, 2]),
            (&[2], &[4], &[0, 2, 4]),
            (&[2], &[6, 8, 10], Z),
            (&[2], &[12], &[0, 2, 4]),
            (&[2], &[4], &[6, 8, 10, 16, 18]),
            (&[2], &[4], &[12, 14]),
            (&[4], &[0], &[0, 2]),
            (&[4], &[2], &[0, 2]),
            (&[4], &[4], &[0, 2, 4, 6, 8]),
            (&[4], &[6], &[0, 2, 4]),
            (&[6], Z, Z),
        ],
        // boss 18 (Lich)
        18 => &[
            (&[0], Z, Z),
            (&[2], &[0], &[0, 2, 4, 6]),
            (&[2], &[2], &[0, 2, 4, 6, 8, 10]),
            (&[2], &[4], &[0, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20]),
            (&[2], &[6, 8, 10], &[0, 2]),
            (&[4], &[0], &[0, 2, 4, 6]),
            (&[4], &[2], &[0, 2, 4]),
            (&[4], &[4, 6, 8], &[0, 2, 4]),
            (&[4], &[10], &[0, 2]),
            (&[6], Z, Z),
        ],
        // bosses 11 and 12 (snakes)
        11 | 12 => &[
            (&[0], Z, Z),
            (&[2], &[0], &[0, 2, 4, 6]),
            (&[2], &[2, 8, 10, 12], Z),
            (&[2], &[4], &[0, 2, 4, 6, 8, 10, 12, 14]),
            (&[2], &[6], Z),
            (&[4], &[0], &[0, 2, 4, 6]),
            (&[4], &[4], &[0, 2, 4]),
            (&[4], &[2, 6, 8, 10], &[0, 2, 4]),
            (&[6], Z, Z),
        ],
        // boss 4 (Chimera)
        4 => &[
            (&[0], Z, Z),
            (&[2], &[0], &[0, 2, 4]),
            (&[2], &[2], &[0, 2, 4, 6, 8, 10]),
            (&[2], &[4], &[0, 2, 4, 6, 8, 10, 12]),
            (&[2], &[6, 8, 10, 12], &[0, 2]),
            (&[2], &[14], Z),
            (&[4], &[0], &[0, 2]),
            (&[4], &[2, 4], &[0, 2, 4]),
            (&[4], &[6, 8], &[0, 2, 4, 6]),
            (&[4], &[10], Z),
            (&[6], Z, Z),
        ],
        // boss 8 (guerrier noir 2)
        7 => &[
            (&[0], Z, Z),
            (&[2], &[0], &[0, 2]),
            (&[2], &[2], &[0, 2, 4, 6, 8]),
            (&[2], &[4], &[0, 2, 4, 6, 8, 10, 12, 14, 16]),
            (&[2], &[6], &[0, 2, 4, 6]),
            (&[2], &[8], &[0, 2, 4, 6, 8]),
            (&[2], &[10], &[0, 2]),
            (&[2], &[12], Z),
            (&[2], &[14], &[0, 2, 4]),
            (&[4], &[0], Z),
            (&[4], &[2], &[0, 2, 4]),
            (&[4], &[4], &[0, 2, 4, 6, 8]),
            (&[4], &[6], &[0, 2, 4, 6]),
            (&[4], &[8, 10], Z),
            (&[6], Z, Z),
        ],
        8 => &[
            (&[0], Z, Z),
            (&[2], &[0], &[0, 2, 4, 6, 8]),
            (&[2], &[2], &[0, 2, 4, 6, 8]),
            (&[2], &[4], &[0, 2, 4, 6, 8, 10, 12, 14, 16, 18]),
            (&[2], &[6], &[0, 2, 4, 6]),
            (&[2], &[8], &[0, 2, 4, 6, 8]),
            (&[2], &[10], &[0, 2]),
            (&[2], &[12], Z),
            (&[2], &[14], &[0, 2, 4]),
            (&[4], &[0], Z),
            (&[4], &[2], &[0, 2, 4]),
            (&[4], &[4], &[0, 2, 4, 6, 8]),
            (&[4], &[6], &[0, 2, 4, 6, 8]),
            (&[4], &[8, 10], Z),
            (&[6], Z, Z),
        ],
        // boss 1 (fauve bondissant)
        1 => &[
            (&[0], Z, Z),
            (&[2], &[0], &[0, 2, 4, 6, 8, 10, 12, 14]),
            (&[2], &[2], &[0, 2, 4, 6, 8, 10]),
            (&[2], &[4], &[0, 2, 4, 6, 8, 10]),
            (&[2], &[6, 8, 10], &[0, 2]),
            (&[2], &[12, 14], &[0]),
            (&[4], &[0], &[0, 2]),
            (&[4], &[2], &[0, 2, 4]),
            (&[4], &[4], &[0, 2, 4, 6, 8]),
            (&[4], &[6, 8], &[0, 2, 4, 6]),
            (&[4], &[10], Z),
            (&[6], Z, Z),
        ],
        // boss 3 (flying creature)
        3 => &[
            (&[0], Z, Z),
            (&[2], &[0], &[0, 2, 4, 6, 8]),
            (&[2], &[2], &[0, 2, 4, 6, 8, 10, 12, 14, 16]),
            (&[2], &[4], &[0, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 22, 24, 26, 28, 30]),
            (&[2], &[6, 8], &[0, 2, 4, 6]),
            (&[2], &[10, 12, 14], Z),
            (&[4], &[0, 4, 6], &[0, 2, 4]),
            (&[4], &[2], &[0, 2]),
            (&[4], &[10], Z),
            (&[6], Z, Z),
        ],
        // boss 9 (engin roulant)
        9 => &[
            (&[0], Z, Z),
            (&[2], &[0], &[2, 4, 6, 8, 10, 14, 16, 18, 20, 22]),
            (&[2], &[2, 6], Z),
            (&[2], &[4], &[0, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 22, 24, 26]),
            (&[4], &[0], &[0, 2, 4]),
            (&[4], &[2], &[0, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20]),
            (&[6], Z, Z),
        ],
        _ => &[(&[0, 2, 4, 6], &[0, 2, 4], &[0, 2, 4])],
    }
}

/// Valid `+7` step of boss 0.
fn step_0(rng: &mut Rng, s4: u8, s5: u8, s6: u8) -> u8 {
    let n: u32 = match (s4, s5, s6) {
        (2, 0, 0) => 4,
        (2, 0, 2) => 5,
        (2, 0, 4) => 6,
        (2, 2, 8) => 4,
        (2, 4, 6) => 4,
        (2, 4, 8) => 5,
        (2, 4, 10) => 4,
        (2, 4, 12) => 3,
        (2, 6 | 8 | 10, _) => 2,
        (2, 2 | 4, _) => 2,
        (4, 2 | 4, _) => 3,
        (4, 6 | 8, _) => 5,
        _ => 1,
    };
    2 * (rng.next() % n) as u8
}

/// Boss 0: the two halves (variant +3), the other half linked by `+0xba`, phase, flags and timers.
fn prep_0(b: &mut Bus, rng: &mut Rng, obj: u32) {
    let pick = |a: &mut Rng, v: &[u8]| v[a.next() as usize % v.len()];
    wr8(b, obj + 3, (rng.next() % 2) as u8);
    wr8(b, obj + 0x2c, if rng.next() % 2 == 0 { 0 } else { 0x20 });
    let s = (rd8(b, obj + 4), rd8(b, obj + 5), rd8(b, obj + 6));
    wr8(b, obj + 7, step_0(rng, s.0, s.1, s.2));
    wr8(b, obj + 0xaa, (rng.next() % 3) as u8);
    for off in [0xadu32, 0xae, 0xaf, 0xb2, 0xb3, 0xb4] {
        wr8(b, obj + off, pick(rng, &[0, 0, 1, 2, 5]));
    }
    // the other half: another BOSS slot, in one of the states the original expects
    wr16(b, obj + 0xba, 0);
    if rng.next() % 4 != 0 {
        let k = BOSS.slot((rng.next() % BOSS.slots + 1) % BOSS.slots).0;
        if k != obj {
            wr8(b, k, 1);
            wr8(b, k + 2, 0);
            wr8(b, k + 0xb4, (rng.next() % 2) as u8);
            let mine = rd32(b, obj + 4);
            wr32(b, k + 4, [0x0200_0408u32, 0x0200_040a, 0x0406_0000, 0x0202_0400, mine][rng.next() as usize % 5]);
            wr16(b, obj + 0xba, k as u16);
        }
    }
    let x = rd16(b, obj + 8).wrapping_add((rng.next() % 0x100) as u16).wrapping_sub(0x80);
    wr16(b, obj + 0xbc, x);
    wr8(b, obj + 0x90, (rng.next() % 4) as u8);
    wr16(b, obj + 0x84, (rng.next() % 4) as u16);
    wr8(b, obj + 0xa0, pick(rng, &[0, 1, 2, 3, 4, 0x10, 0x20]));
    wr8(b, obj + 0xa1, pick(rng, &[0, 2, 3, 0xff]));
    wr32(b, obj + 0xa2, [0x0202_0000u32, 0x0202_0400, 0x0202_0600][rng.next() as usize % 3]);
    wr16(b, obj + 0xa6, (rng.next() % 4) as u16);
    wr16(b, obj + 0xa8, (rng.next() % 4) as u16);
    wr8(b, obj + 0x6b, (rng.next() % 3) as u8);
    wr8(b, obj + 0x6e, (rng.next() % 3) as u8);
    wr8(b, obj + 0x71, (rng.next() % 3) as u8);
    wr8(b, obj + 0x49, pick(rng, &[0, 4, 8, 0x4c, 0x4c]));
    wr8(b, obj + 0x53, pick(rng, &[0, 0, 0, 1]));
    wr8(b, obj + 0x33, pick(rng, &[0, 0, 1, 2, 0x81]));
    wr16(b, obj + 0x14, [0u16, 0x200, 0xfe00, 0x20, 0xffe0][rng.next() as usize % 5]);
    wr16(b, obj + 0x18, [0u16, 0x80, 0x100, 0x300, 0xff00, 0xfe00][rng.next() as usize % 6]);
    wr16(b, obj + 0xc, [0u16, 1, 0x30, 0x80][rng.next() as usize % 4]);
    wr16(b, obj + 0x62, [0u16, 1, 10, 40][rng.next() as usize % 4]);
    wr16(b, obj + 0x60, [0x40u16, 0x100][rng.next() as usize % 2]);
    wr16(b, A5 + 0x7f92, (rng.next() % 32) as u16);
    wr16(b, A5 + 0x52, [0u16, 0, 0, 1][rng.next() as usize % 4]);
    let t = 0xff85de + 0x200 * (rng.next() % 4);
    wr16(b, obj + 0x50, t as u16);
    wr8(b, t, 1);
    wr8(b, t + 0x188, 1);
    // the player near the boss, in front or behind, so that the attack boxes trigger
    let (bx, by) = (rd16(b, obj + 8), rd16(b, obj + 0x10));
    let dx = [0u16, 0x20, 0x30, 0x50, 0xffe0, 0xffd0, 0xffb0, 0xff20, 0x100, 0xff00][rng.next() as usize % 10];
    wr16(b, t + 8, bx.wrapping_add(dx));
    wr16(b, t + 0x10, by.wrapping_add([0u16, 2, 4, 0xfffc, 8, 0x10][rng.next() as usize % 6]));
}

/// Valid `+7` step of boss 16.
fn step_16(rng: &mut Rng, s4: u8, s5: u8, s6: u8) -> u8 {
    let n: u32 = match (s4, s5, s6) {
        (2, 0, _) | (2, 2, _) | (2, 6, 0 | 2) | (2, 10, _) => 2,
        (2, 4, 0) => 5,
        (2, 4, 2 | 4 | 6 | 8 | 10) => 4,
        (2, 4, 12 | 14) => 3,
        (2, 6 | 8, _) => 3,
        (2, 12, _) => 3,
        _ => 1,
    };
    2 * (rng.next() % n) as u8
}

/// Boss 16: variant, prey, aimed point, timers, hit code and speeds.
fn prep_16(b: &mut Bus, rng: &mut Rng, obj: u32) {
    let pick = |a: &mut Rng, v: &[u8]| v[a.next() as usize % v.len()];
    wr8(b, obj + 3, pick(rng, &[0, 0, 1, 1, 2]));
    wr8(b, obj + 0x2c, if rng.next() % 2 == 0 { 0 } else { 0x20 });
    let s = (rd8(b, obj + 4), rd8(b, obj + 5), rd8(b, obj + 6));
    wr8(b, obj + 7, step_16(rng, s.0, s.1, s.2));
    let t = 0xff85de + 0x200 * (rng.next() % 4);
    wr16(b, obj + 0x50, if rng.next() % 10 == 0 { 0 } else { t as u16 });
    wr8(b, t, 1);
    wr8(b, t + 0x188, pick(rng, &[0, 1, 1]));
    wr8(b, t + 0x6f, 0);
    if rng.next() % 3 == 0 {
        wr32(b, t + 4, 0x0200_0e00);
    }
    let (bx, by) = (rd16(b, obj + 8), rd16(b, obj + 0x10));
    let dx = [0u16, 0x20, 0x30, 0x50, 0xffe0, 0xffd0, 0xffb0, 0xff20, 0x100, 0xff00, 0xe0, 0xff40][rng.next() as usize % 12];
    wr16(b, t + 8, bx.wrapping_add(dx));
    wr16(b, t + 0x10, by.wrapping_add([0u16, 2, 4, 0xfffc, 8, 0x10, 0x30][rng.next() as usize % 7]));
    wr16(b, t + 0x62, [1u16, 2, 0x40][rng.next() as usize % 3]);
    wr16(b, obj + 0xae, bx.wrapping_add((rng.next() % 0x100) as u16).wrapping_sub(0x80));
    wr16(b, obj + 0xb0, by.wrapping_add((rng.next() % 0x40) as u16).wrapping_sub(0x20));
    wr8(b, obj + 0xb3, (rng.next() % 2) as u8);
    wr32(b, obj + 0xa8, [0x0202_0000u32, 0x0202_0400, 0x0208_0004][rng.next() as usize % 3]);
    wr16(b, obj + 0xa6, [0u16, 1, 2, 6, 0x1e, 0x4000][rng.next() as usize % 6]);
    wr8(b, obj + 0x90, (rng.next() % 4) as u8);
    for off in [0x6bu32, 0x6e, 0x71] {
        wr8(b, obj + off, (rng.next() % 3) as u8);
    }
    wr8(b, obj + 0x49, pick(rng, &[0, 4, 8, 0x44, 0x48, 0x4c, 0x50, 0x54, 0x60, 0x60, 0x70, 0x78, 0x90, 0x94, 0xa8]));
    wr16(b, obj + 0x62, [0u16, 1, 10, 40][rng.next() as usize % 4]);
    wr16(b, obj + 0x84, (rng.next() % 4) as u16);
    wr8(b, obj + 0xa5, pick(rng, &[1, 2, 5]));
    wr8(b, obj + 0x3e, pick(rng, &[0, 0, 8, 0x81, 0x83, 1, 2]));
    wr8(b, obj + 0x33, pick(rng, &[0, 0, 1, 0x81]));
    wr8(b, obj + 0x53, pick(rng, &[0, 0, 0, 1]));
    wr8(b, obj + 0x56, pick(rng, &[0, 2, 5]));
    wr16(b, obj + 0xa0, [0xffffu16, 0, 7, 0x0210, 0x0405][rng.next() as usize % 5]);
    wr8(b, obj + 0xa2, pick(rng, &[0, 1]));
    wr8(b, obj + 0x42, pick(rng, &[0, 0, 0xff]));
    wr16(b, obj + 0x14, [0u16, 0x200, 0xfe00, 0x300, 0xfd00, 0x20, 0xffe0, 0x4000, 0xc000][rng.next() as usize % 9]);
    wr16(b, obj + 0x16, [0u16, 0x20, 0xffe0][rng.next() as usize % 3]);
    wr16(b, obj + 0x18, [0u16, 0x80, 0x100, 0x300, 0xff00, 0xfe00][rng.next() as usize % 6]);
    wr16(b, obj + 0xc, [0u16, 1, 0x30, 0x80, 0xffff, 0xfff0][rng.next() as usize % 6]);
    wr16(b, A5 + 0x6e, (rng.next() & 0xff) as u16);
    wr16(b, A5 + 0x7f92, (rng.next() % 32) as u16);
    wr8(b, A5 + 0xa, (rng.next() % 2) as u8);
    let cam = rd16(b, A5 + 0x506);
    wr16(b, A5 + 0x556, cam.wrapping_add((rng.next() % 0x200) as u16));
    wr16(b, A5 + 0x50a, [0u16, 0x100, 0x200][rng.next() as usize % 3]);
}

/// Valid `+7` step of boss 22.
fn step_22(rng: &mut Rng, two: bool, s4: u8, s5: u8, s6: u8) -> u8 {
    let n: u32 = match (s4, s5, s6) {
        (2, 0, _) if two => 6,
        (2, 4, 10) if two => 16,
        (2, 4, 12) if two => 7,
        (2, 4, 16) if two => 7,
        (2, 4, 18) if two => 4,
        (2, 0, _) => 3,
        (2, 2, 0 | 8) => 2,
        (2, 2, 2 | 6) => 3,
        (2, 2, 4) => 2,
        (2, 4, 0 | 2 | 4 | 6 | 8) => 2,
        (2, 4, 10 | 16) => 7,
        (2, 4, 12 | 14 | 18) => 4,
        (2, 4, 20 | 22) => 6,
        (2, 6, 0 | 2) => 2,
        (2, 6, 4 | 6) => 6,
        (2, 8, 0) => 3,
        (2, 8, 2) => 2,
        (2, 8, 4) => 2,
        (2, 8, 6 | 8) => 6,
        (2, 10, _) => 2,
        _ => 1,
    };
    2 * (rng.next() % n) as u8
}

/// Boss 22: form, host (another boss), target, aimed points, timers and hit code.
fn prep_22(b: &mut Bus, rng: &mut Rng, obj: u32, two: bool) {
    let pick = |a: &mut Rng, v: &[u8]| v[a.next() as usize % v.len()];
    wr8(b, obj + 3, 0);
    wr8(b, obj + 0x2c, if rng.next() % 2 == 0 { 0 } else { 0x20 });
    let s = (rd8(b, obj + 4), rd8(b, obj + 5), rd8(b, obj + 6));
    wr8(b, obj + 7, step_22(rng, two, s.0, s.1, s.2));
    wr8(b, obj + 0xb5, pick(rng, &[0, 0, 1]));
    wr8(b, obj + 0xac, pick(rng, &[0, 1]));
    wr8(b, obj + 0xa4, pick(rng, &[0, 1]));
    wr8(b, obj + 0xb6, pick(rng, &[4, 0xfc]));
    wr8(b, obj + 0xb9, pick(rng, &[0, 1, 4]));
    wr8(b, obj + 0xaf, pick(rng, &[0, 1]));
    wr8(b, obj + 0xbf, pick(rng, &[0, 1]));
    wr8(b, obj + 0xae, pick(rng, &[0xff, 0xff, 0, 1, 2]));
    wr8(b, obj + 0xb4, (rng.next() % 2) as u8);
    wr8(b, obj + 0xb7, (rng.next() % 8) as u8);
    wr8(b, obj + 0xbb, pick(rng, &[0, 0, 3]));
    wr8(b, obj + 0xba, pick(rng, &[0, 1]));
    wr8(b, obj + 0xbe, pick(rng, &[1, 2, 8]));
    wr16(b, obj + 0xbc, (rng.next() % 3) as u16);
    // the host: another BOSS slot, in one of the states the original expects
    let k = BOSS.slot((rng.next() % BOSS.slots + 1) % BOSS.slots).0;
    if k != obj {
        wr8(b, k, 1);
        wr8(b, k + 2, if two { 0x16 } else { 2 });
        wr8(b, k + 0x7f, (rng.next() % 2) as u8);
        let mine = rd32(b, obj + 4);
        wr32(b, k + 4, [0x0406_0000u32, 0x0202_0400, 0x0200_0000, mine][rng.next() as usize % 4]);
        wr16(b, obj + 0x42, k as u16);
    }
    let t = 0xff85de + 0x200 * (rng.next() % 4);
    wr16(b, obj + 0x50, if rng.next() % 10 == 0 { 0 } else { t as u16 });
    wr8(b, t, 1);
    wr8(b, t + 0x188, pick(rng, &[0, 1, 1]));
    wr8(b, t + 0x6f, 0);
    let (bx, by) = (rd16(b, obj + 8), rd16(b, obj + 0x10));
    let dx = [0u16, 0x20, 0x30, 0x50, 0xffe0, 0xffd0, 0xffb0, 0xff20, 0x100, 0xff00, 0xe0, 0xff40][rng.next() as usize % 12];
    wr16(b, t + 8, bx.wrapping_add(dx));
    wr16(b, t + 0x10, by.wrapping_add([0u16, 2, 4, 0xfffc, 8, 0x10, 0x30][rng.next() as usize % 7]));
    wr16(b, t + 0xc, [0u16, 0, 0, 0x10][rng.next() as usize % 4]);
    wr16(b, t + 0x62, [1u16, 2, 0x40][rng.next() as usize % 3]);
    wr16(b, obj + 0xb0, bx.wrapping_add((rng.next() % 0x100) as u16).wrapping_sub(0x80));
    wr16(b, obj + 0xb2, by.wrapping_add((rng.next() % 0x40) as u16).wrapping_sub(0x20));
    wr32(b, obj + 0xa6, [0x0202_0000u32, 0x0202_0400, 0x0208_0004][rng.next() as usize % 3]);
    wr8(b, obj + 0xa5, pick(rng, &[0, 4]));
    wr16(b, obj + 0xa0, [0u16, 1, 2, 6, 0x1e, 0x4000][rng.next() as usize % 6]);
    wr16(b, obj + 0xaa, [0u16, 1, 6, 0x12c][rng.next() as usize % 4]);
    wr8(b, obj + 0xa2, pick(rng, &[0, 1, 3, 5, 0x28]));
    wr8(b, obj + 0xa3, pick(rng, &[0, 1, 3, 5, 0x7f]));
    wr16(b, obj + 0x46, [0u16, 0xb4, 0xff00][rng.next() as usize % 3]);
    wr16(b, obj + 0x5c, [0u16, 1, 2, 0xf0][rng.next() as usize % 4]);
    for off in [0x6bu32, 0x6e, 0x71] {
        wr8(b, obj + off, (rng.next() % 3) as u8);
    }
    wr8(b, obj + 0x49, pick(rng, &[0, 4, 8, 0x4c, 0x4c]));
    wr16(b, obj + 0x62, [0u16, 1, 10, 40][rng.next() as usize % 4]);
    wr16(b, obj + 0x84, (rng.next() % 4) as u16);
    wr8(b, obj + 0x33, pick(rng, &[0, 0, 1, 0x81]));
    wr8(b, obj + 0x29, pick(rng, &[0, 0, 1]));
    wr8(b, obj + 0x53, pick(rng, &[0, 0, 0, 1]));
    wr8(b, obj + 0x56, pick(rng, &[0, 2, 5]));
    wr16(b, obj + 0x14, [0u16, 0x200, 0xfe00, 0x300, 0xfd00, 0x20, 0xffe0][rng.next() as usize % 7]);
    wr16(b, obj + 0x16, [0u16, 0x20, 0xffe0][rng.next() as usize % 3]);
    wr16(b, obj + 0x18, [0u16, 0x80, 0x100, 0x300, 0xff00, 0xfe00, 0x101][rng.next() as usize % 7]);
    wr16(b, obj + 0xc, [0u16, 1, 0x30, 0x80, 0xffff, 0xfff0][rng.next() as usize % 6]);
    wr16(b, A5 + 0x6e, (rng.next() & 0xff) as u16);
    wr16(b, A5 + 0x7f92, (rng.next() % 32) as u16);
    wr16(b, A5 + 0x50a, [0u16, 0x100, 0x200][rng.next() as usize % 3]);
}

/// Valid `+7` step of bosses 11 and 12.
fn step_11(rng: &mut Rng, s4: u8, s5: u8, s6: u8) -> u8 {
    let n: u32 = match (s4, s5, s6) {
        (2, 2 | 8 | 10 | 12 | 6, _) => 2,
        (2, 4, 0) => 2,
        (2, 4, 2 | 12) => 5,
        (2, 4, 8) => 5,
        (2, 4, _) => 3,
        (4, 4, _) => 2,
        _ => 1,
    };
    2 * (rng.next() % n) as u8
}

/// Valid `+7` step of boss 8.
fn step_8(rng: &mut Rng, s4: u8, s5: u8, s6: u8) -> u8 {
    let n: u32 = match (s4, s5, s6) {
        (2, 2, 6) => 3,
        (2, 2, _) => 2,
        (2, 4, 0) => 2,
        (2, 4, 2 | 4 | 6 | 8 | 14 | 16 | 18) => 4,
        (2, 4, 10) => 5,
        (2, 4, 12) => 6,
        (2, 6, _) => 4,
        (2, 8, 0) => 3,
        (2, 8, 2 | 4 | 8) => 4,
        (2, 8, 6) => 6,
        _ => 1,
    };
    2 * (rng.next() % n) as u8
}

/// Valid `+7` step of boss 7.
fn step_7(rng: &mut Rng, s4: u8, s5: u8, s6: u8) -> u8 {
    let n: u32 = match (s4, s5, s6) {
        (2, 0, 0) => 4,
        (2, 2, 6) => 3,
        (2, 2, _) => 2,
        (2, 4, 0) => 2,
        (2, 4, 6) => 3,
        (2, 4, 10) => 5,
        (2, 4, 12) => 6,
        (2, 4, _) => 4,
        (2, 6, _) => 4,
        (2, 8, 0) => 3,
        (2, 8, 6) => 6,
        (2, 8, _) => 4,
        _ => 1,
    };
    2 * (rng.next() % n) as u8
}

/// Valid `+7` step of boss 4.
fn step_4(rng: &mut Rng, s4: u8, s5: u8, s6: u8) -> u8 {
    let n: u32 = match (s4, s5, s6) {
        (2, 2, 8) => 3,
        (2, 2, 10) => 5,
        (2, 2, _) => 2,
        (2, 4, 0) => 2,
        (2, 4, 8) => 7,
        (2, 4, 12) => 6,
        (2, 4, _) => 5,
        _ => 1,
    };
    2 * (rng.next() % n) as u8
}

/// Boss 4: parameters shared with 17, then its doubles (ENEMY) and its own fields.
fn prep_4(b: &mut Bus, rng: &mut Rng, obj: u32) {
    let pick = |a: &mut Rng, v: &[u8]| v[a.next() as usize % v.len()];
    prep_17(b, rng, obj);
    let s = (rd8(b, obj + 4), rd8(b, obj + 5), rd8(b, obj + 6));
    wr8(b, obj + 3, 0);
    wr8(b, obj + 7, step_4(rng, s.0, s.1, s.2));
    wr8(b, obj + 0xaa, (rng.next() % 3) as u8);
    wr8(b, obj + 0xab, 2);
    for l in [0xb4u32, 0xb6, 0xb8] {
        let d = ENEMY.slot(rng.next() % ENEMY.slots).0;
        wr16(b, obj + l, d as u16);
        wr8(b, d + 0x33, pick(rng, &[0, 0, 1]));
        wr8(b, d + 0xaf, pick(rng, &[0, 0, 1]));
    }
    wr8(b, obj + 0xb2, (rng.next() % 2) as u8);
    wr8(b, obj + 0xb3, (rng.next() % 2) as u8);
    let mark = rd16(b, obj + 8).wrapping_add((rng.next() % 0x100) as u16).wrapping_sub(0x80);
    wr16(b, obj + 0xbc, mark);
    wr8(b, obj + 0x90, (rng.next() % 4) as u8);
    wr16(b, obj + 0x84, (rng.next() % 4) as u16);
    wr8(b, obj + 0x42, (rng.next() % 16) as u8);
    wr8(b, obj + 0xaf, (rng.next() % 3) as u8);
    wr8(b, obj + 0xad, (rng.next() % 2) as u8);
    wr8(b, obj + 0x30, (rng.next() % 2) as u8);
    wr8(b, A5 + 0x30, (rng.next() % 6 != 0) as u8);
    wr16(b, obj + 0x60, [0x40u16, 0x100, 0x300][rng.next() as usize % 3]);
    wr32(b, obj + 0xa2, [0x0202_0000u32, 0x0202_0400, 0x0202_0600][rng.next() as usize % 3]);
    wr8(b, obj + 0x33, pick(rng, &[0, 1, 3, 4, 0x80, 0x81, 0x84]));
    wr16(b, obj + 0x14, [0u16, 0x200, 0xfe00, 0x900, 0xf700][rng.next() as usize % 5]);
    wr16(b, obj + 0x1a, [0u16, 0xffc0, 0xfff0, 0xffd0][rng.next() as usize % 4]);
}

/// Valid `+7` step of boss 18.
fn step_18(rng: &mut Rng, s4: u8, s5: u8, s6: u8) -> u8 {
    let n: u32 = match (s4, s5, s6) {
        (2, 2, 0 | 2 | 10) => 2,
        (2, 2, 8) => 4,
        (2, 4, 0 | 4) => 2,
        (2, 4, 2 | 8 | 10) => 4,
        (2, 4, 6) => 3,
        (2, 4, 12) => 6,
        (2, 4, 14 | 16 | 18) => 5,
        (2, 4, 20) => 4,
        _ => 1,
    };
    2 * (rng.next() % n) as u8
}

/// Valid `+7` step of dragons 15 and 20.
fn step_15(rng: &mut Rng, s4: u8, s5: u8, s6: u8) -> u8 {
    let n: u32 = match (s4, s5, s6) {
        (2, 2, 0 | 2) => 2,
        (2, 4, 0) => 7,
        (2, 4, 2) => 13,
        (2, 4, 4) => 10,
        (2, 4, 6 | 16) => 4,
        (2, 4, 8 | 10 | 18) => 2,
        (2, 4, 12) => 8,
        (2, 4, 14) => 4,
        (2, 12, _) => 3,
        _ => 1,
    };
    2 * (rng.next() % n) as u8
}

/// Valid `+7` step of dragons 14 and 19.
fn step_19(rng: &mut Rng, s4: u8, s5: u8, s6: u8) -> u8 {
    let n: u32 = match (s4, s5, s6) {
        (2, 2, 0 | 8) => 2,
        (2, 4, 0) => 3,
        (2, 4, 8 | 10) => 4,
        (2, 4, _) => 2,
        _ => 1,
    };
    2 * (rng.next() % n) as u8
}

/// Valid `+7` step of boss 21 for a state `(+4, +5, +6)`.
fn step_21(rng: &mut Rng, s4: u8, s5: u8, s6: u8) -> u8 {
    let n: u32 = match (s4, s5, s6) {
        (2, 0, 0) => 8,
        (2, 0, 2) => 3,
        (2, 2, 8 | 10) => 5,
        (2, 2, _) => 2,
        (2, 4, 0 | 2) => 2,
        (2, 4, 12) => 6,
        (2, 4, _) => 5,
        _ => 1,
    };
    2 * (rng.next() % n) as u8
}

/// Valid `+7` step of boss 10 for an active state `(+4, +5, +6)`.
fn step_10(rng: &mut Rng, s4: u8, s5: u8, s6: u8) -> u8 {
    let n: u32 = match (s4, s5, s6) {
        (2, 2, 6) => 3,
        (2, 4, 0) => 2,
        (2, 4, 2) => 6,
        _ => 2,
    };
    2 * (rng.next() % n) as u8
}

/// Valid `+7` step of boss 6 for an active state `(+4, +5, +6)`.
fn step_6(rng: &mut Rng, s4: u8, s5: u8, s6: u8) -> u8 {
    let n: u32 = match (s4, s5, s6) {
        (2, 2, 8) => 3,
        (2, 2, 10) => 5,
        (2, 4, 0) => 2,
        (2, 4, 2) => 5,
        (2, 4, 4) => 5,
        (2, 4, 6) => 5,
        (2, 4, 8) => 7,
        _ => 1,
    };
    2 * (rng.next() % n) as u8
}

/// Valid `+7` step of boss 17 for a state `(+4, +5, +6)` (active states; hit states do not need one).
fn step_17(rng: &mut Rng, s4: u8, s5: u8, s6: u8) -> u8 {
    let n: u32 = match (s4, s5, s6) {
        (2, 0, 0) => 7,
        (2, 0, 2) => 4,
        (2, 0, 4) => 28,
        (2, 0, 6 | 8) => 8,
        (2, 2, 0 | 2) => 2,
        (2, 2, 8 | 10) => 5,
        (2, 4, 0 | 2) => 2,
        (2, 4, 12) => 6,
        (2, 4, _) => 5,
        _ => 1,
    };
    2 * (rng.next() % n) as u8
}

/// Type-specific preparation (fields that the creator or another routine sets).

/// Valid `+7` steps of bosses 1, 3 and 9 (number of jump table entries).
fn step_139(kind: u8, rng: &mut Rng, s4: u8, s5: u8, s6: u8) -> u8 {
    let n: u32 = match (kind, s4, s5, s6) {
        (1, 2, 2, 0 | 2) => 2,
        (1, 2, 2, 4 | 6 | 8) => 6,
        (1, 2, 2, 10) => 5,
        (1, 2, 4, 0..=6) => 2,
        (1, 2, 4, 8) => 3,
        (1, 2, 4, 10) => 5,
        (1, 2, 12, _) => 3,
        (3, 2, 2, 12) => 1,
        (3, 2, 2, _) => 2,
        (3, 2, 4, 4 | 18 | 22 | 26) => 3,
        (3, 2, 4, 16 | 24) => 5,
        (3, 2, 4, 20 | 28) => 4,
        (3, 2, 4, 30) => 7,
        (3, 2, 4, _) => 2,
        (3, 2, 6, 4) => 3,
        (3, 2, 6, _) => 2,
        (3, 2, 8, 0 | 2) => 3,
        (3, 2, 8, _) => 2,
        (3, 2, 12, _) => 3,
        (9, 2, 4, 2 | 18) => 3,
        (9, 2, 4, 6 | 8 | 10 | 20 | 22) => 4,
        (9, 2, 4, _) => 2,
        _ => 1,
    };
    2 * (rng.next() % n) as u8
}

/// Fields of bosses 1, 3 and 9 drawn in plausible ranges (including `+7` steps).
fn prep_1(b: &mut Bus, rng: &mut Rng, obj: u32) {
    let pick = |a: &mut Rng, v: &[u8]| v[a.next() as usize % v.len()];
    wr8(b, obj + 3, 0);
    wr8(b, obj + 0x2c, if rng.next() % 2 == 0 { 0 } else { 0x20 });
    wr16(b, obj + 0x50, (0x85de + 0x200 * (rng.next() % 4)) as u16);
    let s = (rd8(b, obj + 4), rd8(b, obj + 5), rd8(b, obj + 6));
    wr8(b, obj + 7, step_139(1, rng, s.0, s.1, s.2));
    wr8(b, obj + 0xaa, (rng.next() % 3) as u8);
    wr8(b, obj + 0xab, 2);
    for off in [0xadu32, 0xae, 0xaf, 0xb2, 0xb3] {
        wr8(b, obj + off, pick(rng, &[0, 0, 1, 2, 5]));
    }
    wr8(b, obj + 0xb4, pick(rng, &[7, 0xf]));
    wr16(b, obj + 0xb0, if rng.next() % 3 == 0 { ENEMY.slot(rng.next() % ENEMY.slots).0 as u16 } else { 0 });
    let x = rd16(b, obj + 8).wrapping_add((rng.next() % 0x100) as u16).wrapping_sub(0x80);
    wr16(b, obj + 0xbc, x);
    let v = rd16(b, obj + 0x10).wrapping_add((rng.next() % 0x40) as u16).wrapping_sub(0x20);
    wr16(b, obj + 0xbe, v);
    wr8(b, obj + 0x90, (rng.next() % 4) as u8);
    wr16(b, obj + 0x84, (rng.next() % 4) as u16);
    wr8(b, obj + 0xa0, pick(rng, &[0, 1, 2, 3, 4, 5, 0x10]));
    wr8(b, obj + 0xa1, pick(rng, &[0, 2, 3, 0xff]));
    wr32(b, obj + 0xa2, [0x0202_0000u32, 0x0202_0400, 0x0202_0600][rng.next() as usize % 3]);
    wr16(b, obj + 0xa6, (rng.next() % 4) as u16);
    wr16(b, obj + 0xa8, (rng.next() % 4) as u16);
    wr8(b, obj + 0x6b, (rng.next() % 3) as u8);
    wr8(b, obj + 0x6e, (rng.next() % 3) as u8);
    wr8(b, obj + 0x49, pick(rng, &[0, 4, 8, 0x4c, 0x4c]));
    wr8(b, obj + 0x57, pick(rng, &[0, 4]));
    wr8(b, obj + 0x53, pick(rng, &[0, 0, 0, 1]));
    wr8(b, obj + 0x33, pick(rng, &[0, 0, 1, 0x81]));
    wr16(b, obj + 0x14, [0u16, 0x200, 0xfe00, 0xf000][rng.next() as usize % 4]);
    wr16(b, obj + 0x18, [0u16, 0x80, 0x100, 0x300, 0xff00, 0xfe00][rng.next() as usize % 6]);
    wr16(b, obj + 0xc, [0u16, 1, 0x30, 0x80][rng.next() as usize % 4]);
    wr16(b, obj + 0x62, [0u16, 1, 10, 40][rng.next() as usize % 4]);
    wr16(b, obj + 0x60, 0x100);
    wr16(b, obj + 0x54, if rng.next() % 2 == 0 { 0 } else { (0x85de + 0x200 * (rng.next() % 4)) as u16 });
    if rng.next() % 2 == 0 {
        let t = 0xff85de + 0x200 * (rng.next() % 4);
        wr16(b, obj + 0x50, t as u16);
        wr8(b, t, 1);
        wr8(b, t + 0x188, 1);
    }
}

fn prep_3(b: &mut Bus, rng: &mut Rng, obj: u32) {
    let pick = |a: &mut Rng, v: &[u8]| v[a.next() as usize % v.len()];
    wr8(b, obj + 3, pick(rng, &[0, 0, 1]));
    wr8(b, obj + 0x2c, if rng.next() % 2 == 0 { 0 } else { 0x20 });
    wr16(b, obj + 0x50, if rng.next() % 8 == 0 { 0 } else { (0x85de + 0x200 * (rng.next() % 4)) as u16 });
    let s = (rd8(b, obj + 4), rd8(b, obj + 5), rd8(b, obj + 6));
    wr8(b, obj + 7, step_139(3, rng, s.0, s.1, s.2));
    for off in [0xa2u32, 0xa5, 0xb5] {
        wr8(b, obj + off, pick(rng, &[0, 0, 1, 2]));
    }
    wr8(b, obj + 0xa3, 2 * (rng.next() % 3) as u8);
    wr16(b, obj + 0xa6, [0u16, 1, 2, 0x100, 0x2000][rng.next() as usize % 5]);
    wr32(b, obj + 0xaa, [0x0202_0000u32, 0x0202_0400, 0x0208_0004][rng.next() as usize % 3]);
    let x = rd16(b, obj + 8).wrapping_add((rng.next() % 0x40) as u16).wrapping_sub(0x20);
    wr16(b, obj + 0xb0, x);
    let v = rd16(b, obj + 0x10).wrapping_add((rng.next() % 0x20) as u16).wrapping_sub(0x10);
    wr16(b, obj + 0xb2, v);
    for off in [0x6bu32, 0x6e, 0x71] {
        wr8(b, obj + off, (rng.next() % 3) as u8);
    }
    wr16(b, obj + 0x84, (rng.next() % 4) as u16);
    wr8(b, obj + 0x49, pick(rng, &[0, 4, 8, 0x4c]));
    wr8(b, obj + 0x57, pick(rng, &[0, 4]));
    wr8(b, obj + 0x53, pick(rng, &[0, 0, 0, 1]));
    wr8(b, obj + 0x33, pick(rng, &[0, 0, 1, 2, 0x81]));
    wr16(b, obj + 0x14, [0u16, 0x200, 0xfe00, 0x900, 0xf600][rng.next() as usize % 5]);
    wr16(b, obj + 0x18, [0u16, 0x80, 0x400, 0xff00, 0xfc00][rng.next() as usize % 5]);
    wr16(b, obj + 0xc, [0u16, 8, 0x30, 0x50, 0x70, 0xc0, 0xff80][rng.next() as usize % 7]);
    wr16(b, obj + 0x62, [0u16, 1, 10, 40, 0x100][rng.next() as usize % 5]);
    wr16(b, obj + 0x60, 0x100);
    if rng.next() % 2 == 0 {
        let t = 0xff85de + 0x200 * (rng.next() % 4);
        wr16(b, obj + 0x50, t as u16);
        wr8(b, t, 1);
        wr8(b, t + 0x188, 1);
    }
}

fn prep_9(b: &mut Bus, rng: &mut Rng, obj: u32) {
    let pick = |a: &mut Rng, v: &[u8]| v[a.next() as usize % v.len()];
    wr8(b, obj + 0x2c, if rng.next() % 2 == 0 { 0 } else { 0x20 });
    wr16(b, obj + 0x50, if rng.next() % 8 == 0 { 0 } else { (0x85de + 0x200 * (rng.next() % 4)) as u16 });
    let s = (rd8(b, obj + 4), rd8(b, obj + 5), rd8(b, obj + 6));
    wr8(b, obj + 7, step_139(9, rng, s.0, s.1, s.2));
    for l in [0xacu32, 0xae, 0xb0, 0xb2, 0xb4] {
        let d = if rng.next() % 4 == 0 { 0 } else { ENEMY.slot(rng.next() % ENEMY.slots).0 };
        wr16(b, obj + l, d as u16);
        if d != 0 {
            wr16(b, d + 4, [0x0200u16, 0x0200, 0x0400, 0][rng.next() as usize % 4]);
        }
    }
    wr8(b, obj + 0xaa, pick(rng, &[0, 0, 0, 3, 4]));
    wr8(b, obj + 0xa8, (rng.next() & 0xff) as u8);
    wr8(b, obj + 0xa9, (rng.next() & 0xff) as u8);
    wr8(b, obj + 0xa0, pick(rng, &[0, 0, 1, 5]));
    wr8(b, obj + 0xa1, pick(rng, &[0, 1, 2]));
    wr16(b, obj + 0xa2, [0u16, 1, 0x14, 0xffff][rng.next() as usize % 4]);
    for off in [0xb9u32, 0xba, 0xbd, 0xbe, 0xbf, 0x6b, 0x6e, 0x43, 0x5c, 0x5d, 0xab] {
        wr8(b, obj + off, pick(rng, &[0, 0, 1, 2, 0xff]));
    }
    wr16(b, obj + 0x78, [0u16, 1, 0x300][rng.next() as usize % 3]);
    let v = rd16(b, obj + 8).wrapping_add((rng.next() % 0x40) as u16).wrapping_sub(0x20);
    wr16(b, obj + 0xb8, v);
    wr32(b, obj + 0x8a, [0u32, 0x0202_0000, 0x0204_0002, 0x0204_0400][rng.next() as usize % 4]);
    wr16(b, obj + 0x68, 0);
    wr8(b, obj + 0x53, pick(rng, &[0, 0, 0, 1, 0xff]));
    wr16(b, obj + 0x14, [0u16, 0x80, 0x500, 0xfb00, 0x800, 0xf800][rng.next() as usize % 6]);
    wr16(b, obj + 0x1c, [0x120u16, 0xfee0][rng.next() as usize % 2]);
    wr16(b, obj + 0x10, [0x40u16, 0x48, 0x60, 0x78][rng.next() as usize % 4]);
    wr16(b, obj + 0x62, [0u16, 1, 10, 40, 0x100][rng.next() as usize % 5]);
    wr16(b, obj + 0x60, 0x100);
    wr16(b, obj + 0x54, if rng.next() % 2 == 0 { 0 } else { (0x85de + 0x200 * (rng.next() % 4)) as u16 });
    let v = rd16(b, obj + 8).wrapping_sub((rng.next() % 0x200) as u16);
    wr16(b, A5 + 0x556, v);
    wr16(b, A5 + 0x70, (rng.next() % 2) as u16);
    wr16(b, obj + 0x62, (rng.next() % 0x110) as u16);
    wr8(b, obj + 0xab, pick(rng, &[0, 1, 2]));
    if rng.next() % 2 == 0 {
        // full crew
        for k in 0..4u32 {
            let d = ENEMY.slot(k).0;
            wr16(b, obj + 0xae + 2 * k, d as u16);
            wr16(b, d + 4, 0x0200);
        }
    }
}

fn prep(kind: u8, b: &mut Bus, rng: &mut Rng, obj: u32) {
    match kind {
        0 => prep_0(b, rng, obj),
        16 => prep_16(b, rng, obj),
        2 => prep_22(b, rng, obj, true),
        22 => prep_22(b, rng, obj, false),
        1 => prep_1(b, rng, obj),
        3 => prep_3(b, rng, obj),
        9 => prep_9(b, rng, obj),
        _ => {}
    }
    if kind == 17 {
        prep_17(b, rng, obj);
    }
    if kind == 6 {
        prep_6(b, rng, obj);
    }
    if kind == 10 {
        prep_10(b, rng, obj);
    }
    if kind == 5 {
        prep_5(b, rng, obj);
    }
    if kind == 21 {
        prep_21(b, rng, obj);
    }
    if kind == 14 || kind == 19 {
        prep_19(b, rng, obj, kind);
    }
    if kind == 15 || kind == 20 {
        prep_15(b, rng, obj);
    }
    if kind == 18 {
        prep_18(b, rng, obj);
    }
    if kind == 4 {
        prep_4(b, rng, obj);
    }
    if kind == 8 || kind == 7 {
        prep_8(b, rng, obj);
    }
    if kind == 7 {
        let s = (rd8(b, obj + 4), rd8(b, obj + 5), rd8(b, obj + 6));
        wr8(b, obj + 7, step_7(rng, s.0, s.1, s.2));
        wr8(b, A5 + 0x30, (rng.next() % 6 != 0) as u8);
        wr16(b, obj + 0x60, [0x40u16, 0x100, 0x300][rng.next() as usize % 3]);
    }
    if kind == 11 || kind == 12 {
        prep_11(b, rng, obj);
    }
    if kind == 13 {
        // short hovering timer, step rank of the STOPS table (4 values), cutscene message
        wr8(b, obj + 0xa1, (rng.next() % 4) as u8);
        wr8(b, obj + 0xa0, (rng.next() % 2) as u8);
        wr8(b, obj + 0xa7, (rng.next() % 4) as u8);
        wr16(b, obj + 0xa2, [0, 1, 2, 0x12c][rng.next() as usize % 4]);
        wr8(b, obj + 0xa5, (rng.next() % 2) as u8);
        wr8(b, obj + 7, 2 * (rng.next() % 2) as u8);
        wr8(b, obj + 0x42, (rng.next() % 4) as u8);
        wr16(b, obj + 0xb0, if rng.next() % 2 == 0 { 0 } else { ENEMY.slot(rng.next() % ENEMY.slots).0 as u16 });
    }
}

/// Boss 17: AI parameters, phase, budgets, target, links and plausible scene variables.
fn prep_17(b: &mut Bus, rng: &mut Rng, obj: u32) {
    let pick = |a: &mut Rng, v: &[u8]| v[a.next() as usize % v.len()];
    wr8(b, obj + 3, (rng.next() % 3) as u8);
    wr8(b, obj + 0x2c, if rng.next() % 2 == 0 { 0 } else { 0x20 });
    wr16(b, obj + 0x50, (0x85de + 0x200 * (rng.next() % 4)) as u16);
    let s = (rd8(b, obj + 4), rd8(b, obj + 5), rd8(b, obj + 6));
    wr8(b, obj + 7, step_17(rng, s.0, s.1, s.2));
    wr8(b, obj + 0xaa, (rng.next() % 5) as u8);
    wr8(b, obj + 0xab, 4);
    wr8(b, obj + 0xac, (rng.next() % 0x40) as u8);
    for off in [0xb2u32, 0xb3, 0xb4, 0xaf] {
        wr8(b, obj + off, pick(rng, &[0, 1, 2, 5, 0xff]));
    }
    wr8(b, obj + 0xad, (rng.next() % 4 == 0) as u8);
    wr8(b, obj + 0xbb, pick(rng, &[0, 0, 0, 1, 2]));
    wr8(b, obj + 0xba, (rng.next() % 4) as u8);
    wr16(b, obj + 0xb8, (rng.next() % 4) as u16);
    wr8(b, obj + 0xb5, (rng.next() % 12) as u8);
    wr8(b, obj + 0xb6, (rng.next() % 5) as u8);
    wr8(b, obj + 0xb7, (rng.next() % 6) as u8);
    wr8(b, obj + 0xa0, pick(rng, &[0, 1, 2, 3, 5, 0x30, 0x31, 0x32, 0x33, 0x34]));
    wr8(b, obj + 0xa1, pick(rng, &[0, 1, 2, 3, 5, 0xff]));
    wr16(b, obj + 0xa6, (rng.next() % 6) as u16);
    wr16(b, obj + 0xa8, (rng.next() % 8) as u16);
    wr8(b, obj + 0x6b, (rng.next() % 4) as u8);
    wr8(b, obj + 0x6e, (rng.next() % 4) as u8);
    wr8(b, obj + 0x71, (rng.next() % 4) as u8);
    wr8(b, obj + 0x79, pick(rng, &[0x31, 0x32]));
    wr8(b, obj + 0x49, 4 * (rng.next() % 43) as u8);
    wr8(b, obj + 0x56, (rng.next() % 6) as u8);
    wr8(b, obj + 0x53, pick(rng, &[0, 0, 0, 1]));
    wr8(b, obj + 0x33, pick(rng, &[0, 0, 1, 0x81, 0xff]));
    wr8(b, obj + 0x2d, (rng.next() % 32) as u8);
    wr16(b, obj + 0x14, [0u16, 0x200, 0xfe00, 0x400, 0xfc00][rng.next() as usize % 5]);
    wr16(b, obj + 0x18, [0u16, 0x80, 0x100, 0x120, 0x300, 0xff00, 0xff60, 0xff80][rng.next() as usize % 8]);
    wr16(b, obj + 0x1c, [0u16, 1, 0xff00, 0xff80][rng.next() as usize % 4]);
    wr16(b, obj + 0xc, [0u16, 1, 0x30, 0x50, 0x60][rng.next() as usize % 5]);
    wr16(b, obj + 0x62, [0u16, 1, 10, 40, 100][rng.next() as usize % 5]);
    // links: message (+0x42, +0xb0) to an effect slot; player who hit (+0x54)
    for l in [obj + 0x42, obj + 0xb0] {
        let m = ddsom_game::objects::pools::EFFCT.slot(rng.next() % 3).0;
        wr16(b, l, m as u16);
        wr8(b, m + 0x22, (rng.next() % 2) as u8);
        wr16(b, m + 4, [0x020a, 0x021e, 0x0202, 0][rng.next() as usize % 4]);
    }
    wr16(b, obj + 0x54, if rng.next() % 3 == 0 { 0x85de + 0x200 * (rng.next() % 4) as u16 } else { 0 });
    // scene variables
    wr8(b, A5 + 0x34, pick(rng, &[13, 0x2c, 0x6f, 5]));
    wr8(b, A5 + 0x35, (rng.next() % 5) as u8);
    wr8(b, A5 + 0x36, (rng.next() % 4 == 0) as u8);
    wr8(b, A5 + 0x2c, pick(rng, &[0, 0xc, 4]));
    wr8(b, A5 + 0x500, (rng.next() % 2) as u8);
    wr16(b, A5 + 0x7f92, (rng.next() % 32) as u16);
    wr8(b, 0xff2a38, pick(rng, &[0, 0xc, 0x10]));
    let (ox, oy) = (rd16(b, obj + 8), rd16(b, obj + 0x10));
    for (cx, cy, kx, ky) in [(0x5a6u32, 0x5aau32, 0x506u32, 0x50au32), (0x5ae, 0x5b2, 0x556, 0x55a)] {
        wr16(b, A5 + cx, ox.wrapping_sub((rng.next() % 0x200) as u16).wrapping_add(0x40));
        wr16(b, A5 + cy, oy.wrapping_sub((rng.next() % 0x100) as u16));
        wr16(b, A5 + kx, ox.wrapping_sub((rng.next() % 0x200) as u16).wrapping_add(0x40));
        wr16(b, A5 + ky, [0u16, 0x100, 0x1c0, 0x1c8, 0x1d0][rng.next() as usize % 5]);
    }
    for j in 0..4u32 {
        let q = 0xff85de + 0x200 * j;
        let dx = (rng.next() % 0x200) as u16;
        wr8(b, q, (rng.next() % 4 != 0) as u8);
        wr16(b, q + 8, ox.wrapping_add(dx).wrapping_sub(0x100));
        wr16(b, q + 0x10, oy.wrapping_add((rng.next() % 0x20) as u16).wrapping_sub(0x10));
        wr16(b, q + 0xc, (rng.next() % 4) as u16);
    }
}

/// Boss 6: same scene variables as 17; own parameters.
fn prep_6(b: &mut Bus, rng: &mut Rng, obj: u32) {
    let pick = |a: &mut Rng, v: &[u8]| v[a.next() as usize % v.len()];
    prep_17(b, rng, obj);
    wr8(b, obj + 3, if rng.next() % 4 == 0 { 1 } else { 0 });
    let s = (rd8(b, obj + 4), rd8(b, obj + 5), rd8(b, obj + 6));
    wr8(b, obj + 7, step_6(rng, s.0, s.1, s.2));
    wr8(b, obj + 0xaa, (rng.next() % 3) as u8);
    wr8(b, obj + 0xab, 2);
    wr8(b, obj + 0xb2, (rng.next() % 2) as u8);
    wr8(b, obj + 0xb3, (rng.next() % 2) as u8);
    wr16(b, obj + 0xbc, (rng.next() % 0x400) as u16);
    wr16(b, obj + 0x84, (rng.next() % 4) as u16);
    wr8(b, obj + 0x90, (rng.next() % 20) as u8);
    wr8(b, obj + 0x33, pick(rng, &[0, 0, 1, 2, 3, 4, 0x81, 0x82, 0x84, 0xff]));
    wr16(b, obj + 0x46, ENEMY.slot(rng.next() % ENEMY.slots).0 as u16);
    wr8(b, obj + 0x32, (rng.next() % 2) as u8);
    // a linked shot for the attacks that drive it
    let shot = ddsom_game::objects::pools::ESHL.slot(rng.next() % 4).0;
    wr16(b, obj + 0xb0, shot as u16);
    wr8(b, shot, (rng.next() % 4 != 0) as u8);
    wr8(b, shot + 0x5a, (rng.next() % 2) as u8);
}

/// Boss 5: same scene variables as 17; own parameters.
fn prep_5(b: &mut Bus, rng: &mut Rng, obj: u32) {
    let pick = |a: &mut Rng, v: &[u8]| v[a.next() as usize % v.len()];
    prep_17(b, rng, obj);
    wr8(b, obj + 3, 0);
    wr8(b, obj + 7, 2 * (rng.next() % 2) as u8);
    let s = (rd8(b, obj + 4), rd8(b, obj + 5), rd8(b, obj + 6));
    // outside knockback (+6 = 0 or 4), only code 0xa8 (whirlwind fall) has steps
    let code = if s.0 == 4 && s.1 <= 4 && s.2 != 0 && s.2 != 4 { 0xa8 } else { pick(rng, &[0xa8, 0xa8, 0x38, 0x4c, 0]) };
    wr8(b, obj + 0x49, code);
    wr8(b, obj + 0x87, (rng.next() % 3) as u8);
    // remembered state (+0xa8): a valid active state
    wr32(b, obj + 0xa8, 0x0200_0000 | (pick(rng, &[0, 4, 8, 12, 16, 20, 24]) as u32) << 16);
    wr8(b, obj + 0x90, (rng.next() % 20) as u8);
    wr8(b, obj + 0x91, pick(rng, &[0, 0x80, 0x85, 3]));
    wr8(b, obj + 0x33, pick(rng, &[0, 0, 1, 0xff]));
    wr8(b, obj + 0xa2, pick(rng, &[0, 1, 2, 0x20]));
    wr8(b, obj + 0xa3, (rng.next() % 6) as u8);
    wr8(b, obj + 0xa4, pick(rng, &[0, 1, 2]));
    wr8(b, obj + 0xa5, pick(rng, &[0, 1, 2]));
    wr8(b, obj + 0xa6, (rng.next() % 3) as u8);
    wr8(b, obj + 0xa7, (rng.next() % 2) as u8);
    wr16(b, obj + 0xa0, (rng.next() % 4) as u16);
    wr8(b, obj + 0x56, (rng.next() % 6) as u8);
    wr8(b, A5 + 0x2e, pick(rng, &[0, 4]));
    wr8(b, A5 + 0x2c, pick(rng, &[0, 0xc, 4]));
}

/// Dragons 14 and 19: variant, host (variant 1 of 19), reactions, remembered state.
fn prep_19(b: &mut Bus, rng: &mut Rng, obj: u32, kind: u8) {
    let pick = |a: &mut Rng, v: &[u8]| v[a.next() as usize % v.len()];
    prep_17(b, rng, obj);
    let variant = if kind == 19 { (rng.next() % 3 == 0) as u8 } else { 0 };
    wr8(b, obj + 3, variant);
    // the death of Nagpa's dragon has only two steps
    if variant != 0 && rd8(b, obj + 4) == 4 && rd8(b, obj + 5) == 6 && rd8(b, obj + 6) == 4 {
        wr8(b, obj + 6, 2);
    }
    // the arrival of Nagpa's dragon has only three steps
    if variant != 0 && rd8(b, obj + 4) == 2 && rd8(b, obj + 5) == 0 && rd8(b, obj + 6) >= 6 {
        wr8(b, obj + 6, 4);
    }
    let s = (rd8(b, obj + 4), rd8(b, obj + 5), rd8(b, obj + 6));
    wr8(b, obj + 7, step_19(rng, s.0, s.1, s.2));
    wr8(b, A5 + 0x30, (rng.next() % 4 != 0) as u8);
    wr8(b, A5 + 0x2c, pick(rng, &[0, 4, 0xc, 0xd]));
    wr16(b, obj + 0x60, [40u16, 100, 700][rng.next() as usize % 3]);
    wr8(b, obj + 0x49, pick(rng, &[0, 4, 0x60, 0x68, 0x70, 0x78, 0x80, 0x90, 0x94, 0xa8, 0xa8]));
    wr8(b, obj + 0x56, (rng.next() % 6) as u8);
    wr8(b, obj + 0x87, (rng.next() % 4) as u8);
    wr8(b, obj + 0x90, (rng.next() % 20) as u8);
    wr8(b, obj + 0xa0, pick(rng, &[0, 2, 4, 0xff]));
    wr8(b, obj + 0xa1, pick(rng, &[0, 1, 2, 4, 5, 7, 0xff]));
    wr8(b, obj + 0xa3, (rng.next() % 2) as u8);
    wr8(b, obj + 0xa4, (rng.next() % 2) as u8);
    wr8(b, obj + 0xa5, pick(rng, &[0, 0xff]));
    wr16(b, obj + 0xa6, [0u16, 1, 2, 5, 0x20, 0x64, 0x65, 0x80][rng.next() as usize % 8]);
    wr16(b, obj + 0xc, [0x1fu16, 0x20, 0x21, 0, 0x30, 0x60, 0x100, 0xff00][rng.next() as usize % 8]);
    wr8(b, obj + 0xad, (rng.next() % 0x22) as u8);
    wr8(b, obj + 0xac, (rng.next() % 6) as u8);
    wr8(b, obj + 0xae, (rng.next() % 3) as u8);
    wr8(b, obj + 0xaf, (rng.next() % 5) as u8);
    wr8(b, obj + 0xb2, pick(rng, &[0, 0, 1, 2, 0x30]));
    wr8(b, obj + 0xb0, (rng.next() % 5) as u8);
    wr8(b, obj + 0xb1, (rng.next() % 3) as u8);
    // remembered state: the breath (attack 4) one time in three
    let saved = if rng.next() % 3 == 0 { 0x0204_0400 } else { 0x0202_0000 };
    wr32(b, obj + 0xa8, saved);
    // host: a living enemy or not, with or without HP
    let host = ENEMY.slot(rng.next() % ENEMY.slots).0;
    wr16(b, obj + 0x46, if rng.next() % 4 == 0 { 0 } else { host as u16 });
    wr8(b, host, (rng.next() % 4 != 0) as u8);
    wr16(b, host + 0x62, [0u16, 1, 50][rng.next() as usize % 3]);
    if s.0 == 4 && s.1 == 0 && (s.2 == 2) {
        // step 2 of the reaction is only used for the slide (code 0x68, 0x80, 0xa8 or ordinary)
    }
    close_players(b, rng, obj);
}

/// Red dragons 15 and 20: remembered positions, armor, previous state, script tables.
fn prep_15(b: &mut Bus, rng: &mut Rng, obj: u32) {
    let pick = |a: &mut Rng, v: &[u8]| v[a.next() as usize % v.len()];
    prep_17(b, rng, obj);
    let s = (rd8(b, obj + 4), rd8(b, obj + 5), rd8(b, obj + 6));
    wr8(b, obj + 7, step_15(rng, s.0, s.1, s.2));
    wr8(b, A5 + 0x2c, pick(rng, &[0, 4, 0xc, 0xd]));
    wr16(b, obj + 0x60, [40u16, 100, 700][rng.next() as usize % 3]);
    wr8(b, obj + 0x49, pick(rng, &[0, 4, 8, 0x60, 0xa8]));
    wr8(b, obj + 0x56, (rng.next() % 6) as u8);
    wr16(b, obj + 0x84, (rng.next() % 4) as u16);
    wr8(b, obj + 0x90, (rng.next() % 20) as u8);
    wr8(b, obj + 0xa0, pick(rng, &[0, 2, 4, 0xff]));
    wr8(b, obj + 0xa1, pick(rng, &[0, 1, 2, 4, 5, 0x13, 0x14, 0x15, 0x16, 0x22, 0xff]));
    wr8(b, obj + 0xa4, (rng.next() % 2) as u8);
    wr8(b, obj + 0xa5, (rng.next() % 40) as u8);
    wr16(b, obj + 0xa6, [0u16, 1, 2, 5, 0x20, 0x40, 0xf0][rng.next() as usize % 7]);
    wr16(b, obj + 0xc, [0x1fu16, 0x20, 0x21, 0x30, 0x40, 0x60, 0x2c0, 0x300, 0x380, 0xff00][rng.next() as usize % 10]);
    let (ox, oz, oy) = (rd16(b, obj + 8), rd16(b, obj + 0xc), rd16(b, obj + 0x10));
    wr16(b, obj + 0xa8, ox);
    wr16(b, obj + 0xaa, oz);
    wr16(b, obj + 0xac, oy);
    wr8(b, obj + 0xae, (rng.next() % 3) as u8);
    wr8(b, obj + 0xaf, (rng.next() % 4) as u8);
    wr8(b, obj + 0xb4, (rng.next() % 0x42) as u8);
    wr8(b, obj + 0xb5, (rng.next() % 5) as u8);
    wr8(b, obj + 0xb7, (rng.next() % 3) as u8);
    wr8(b, obj + 0xb8, pick(rng, &[0, 0, 1, 2, 0x10]));
    wr8(b, obj + 0x33, pick(rng, &[0, 0, 1, 0xff]));
    wr8(b, obj + 0x43, (rng.next() % 4 == 0) as u8);
    wr8(b, obj + 0x2d, (rng.next() % 32) as u8);
    // breath vector table (+0x4c): null or the ROM's angle table
    wr32(b, obj + 0x4c, if rng.next() % 2 == 0 { 0 } else { 0x1c6636 });
    wr8(b, A5 + 0x42, (rng.next() % 4) as u8);
    // previous frame state: active (02) or other
    let saved = [0x0202_0000u32, 0x0202_0400, 0x0204_0200, 0x0000_0000][rng.next() as usize % 4];
    wr32(b, obj + 0xb0, saved);
    wr8(b, obj + 0xb9, (rng.next() % 9) as u8);
    close_players(b, rng, obj);
    // grabbable players (+0x188) and 0x91f6c: at the exact attack position
    for j in 0..4u32 {
        wr8(b, 0xff85de + 0x200 * j + 0x188, (rng.next() % 2) as u8);
    }
    if (s.0, s.1, s.2) == (2, 4, 10) && rng.next() % 2 == 0 {
        let edge = if rd8(b, obj + 0x2c) != 0 { 0x48u16 } else { 0x138 };
        let scroll = rd16(b, A5 + 0x506);
        wr16(b, obj + 8, scroll.wrapping_add(edge));
    }
}

/// Bosses 11 and 12: twelve rings around the head, tracked pose and rank, hit codes consistent with the step.
fn prep_11(b: &mut Bus, rng: &mut Rng, obj: u32) {
    let pick = |a: &mut Rng, v: &[u8]| v[a.next() as usize % v.len()];
    prep_17(b, rng, obj);
    let s = (rd8(b, obj + 4), rd8(b, obj + 5), rd8(b, obj + 6));
    wr8(b, obj + 3, 0);
    wr8(b, obj + 7, step_11(rng, s.0, s.1, s.2));
    // rings: ranks 0 to 11 in the first twelve ESHL slots, the others empty
    let esh = &ddsom_game::objects::pools::ESHL;
    for i in 0..esh.slots {
        let o = esh.slot(i).0;
        if i < 12 && s.0 != 0 {
            wr8(b, o, 1);
            wr8(b, o + 2, 5);
            wr8(b, o + 0x2f, i as u8);
            wr16(b, o + 0x46, obj as u16);
            let bx = rd16(b, obj + 8);
            let by = rd16(b, obj + 0x10);
            wr16(b, o + 8, bx.wrapping_add((rng.next() % 0x80) as u16).wrapping_sub(0x40));
            wr16(b, o + 0xc, [0u16, 3, 0x0f, 0xfffd, 0xfff0, 0xffe0][rng.next() as usize % 6]);
            wr16(b, o + 0x10, by.wrapping_add((rng.next() % 0x20) as u16).wrapping_sub(0x10));
            wr8(b, o + 0x42, (rng.next() % 32) as u8);
            wr8(b, o + 0x43, if rng.next() % 3 == 0 { (rng.next() % 32) as u8 } else { 0 });
            wr8(b, o + 3, (rng.next() % 3) as u8);
        } else {
            wr8(b, o, 0);
        }
    }
    match rng.next() % 24 {
        0 => wr8(b, esh.slot(rng.next() % 12).0, 0),
        1 => wr8(b, esh.slot(rng.next() % 12).0 + 0x2f, 0x40),
        2 => wr8(b, esh.slot(12).0, 0), // no twelfth ring
        _ => {}
    }
    // pose (32-byte row of 0x1069f8) and tracked rank, around the row threshold
    let aa = (rng.next() % 22) as u8;
    wr8(b, obj + 0xaa, aa);
    let thr = rd8(b, 0x1069f8 + 32 * aa as u32 + 2);
    let seg = match rng.next() % 4 {
        0 => thr,
        1 => thr.wrapping_sub(1),
        2 => thr.wrapping_add(1),
        _ => (rng.next() % 12) as u8,
    };
    wr8(b, obj + 0xa8, if rng.next() % 10 == 0 { 0 } else { seg % 12 });
    wr8(b, obj + 0x2d, (rng.next() % 32) as u8);
    let (bx, by) = (rd16(b, obj + 8), rd16(b, obj + 0x10));
    wr16(b, obj + 0xa2, bx.wrapping_add((rng.next() % 8) as u16));
    wr16(b, obj + 0xa4, [0u16, 1, 2, 3, 0x11, 0x40, 0x5f, 0x60, 0xfff0][rng.next() as usize % 9]);
    wr16(b, obj + 0xa6, by.wrapping_add((rng.next() % 4) as u16));
    wr16(b, obj + 0xa0, [0u16, 1, 8, 9, 0x0200, 0x0402, 0xffff][rng.next() as usize % 7]);
    wr8(b, obj + 0xa0, pick(rng, &[0, 0, 0, 2, 4]));
    wr8(b, obj + 0xa1, pick(rng, &[0, 1, 8, 9, 7, 3, 0x24]));
    wr32(b, obj + 0x4c, 0x1c6636);
    wr8(b, obj + 0xb2, (rng.next() % 2) as u8);
    wr8(b, obj + 0xb6, (rng.next() % 4) as u8);
    wr8(b, obj + 0xb7, (rng.next() % 2) as u8);
    wr8(b, obj + 0xb8, (rng.next() % 5) as u8);
    wr8(b, obj + 0xb9, pick(rng, &[0, 0, 1, 2]));
    wr16(b, obj + 0xb0, (rng.next() % 5) as u16);
    if rng.next() % 2 == 0 {
        wr8(b, obj + 0xb0, 1 + (rng.next() % 4) as u8);
    }
    wr8(b, obj + 0xb1, (rng.next() % 4) as u8);
    wr8(b, obj + 0xb3, (rng.next() % 32) as u8);
    wr8(b, obj + 0xb4, (rng.next() % 16) as u8);
    wr8(b, obj + 0xb5, (rng.next() % 6) as u8);
    wr8(b, obj + 0xac, pick(rng, &[0, 1, 0xff]));
    wr8(b, obj + 0xad, (rng.next() % 2) as u8);
    let tgt = ENEMY.slot(rng.next() % ENEMY.slots).0;
    wr8(b, tgt, 1);
    wr8(b, tgt + 2, if rng.next() % 2 == 0 { 0x47 } else { 2 });
    wr16(b, obj + 0x5c, if rng.next() % 2 == 0 { 0 } else { tgt as u16 });
    wr16(b, obj + 0x60, [0x40u16, 0x100, 0x300][rng.next() as usize % 3]);
    wr16(b, obj + 0x62, [0u16, 1, 0x20, 0x60, 0x100, 0x2ff][rng.next() as usize % 6]);
    wr16(b, obj + 0x84, (rng.next() % 4) as u16);
    wr8(b, obj + 0x86, (rng.next() % 2) as u8);
    wr8(b, obj + 0x87, (rng.next() % 4) as u8);
    wr8(b, obj + 0x90, (rng.next() % 4) as u8);
    wr8(b, obj + 0x29, (rng.next() % 2) as u8);
    wr8(b, obj + 0x33, pick(rng, &[0, 0, 1, 0x81, 0xff]));
    wr8(b, obj + 0x3e, (rng.next() % 22) as u8);
    wr8(b, obj + 0x56, (rng.next() % 6) as u8);
    wr8(b, obj + 0x53, pick(rng, &[0, 0, 0, 1]));
    wr16(b, obj + 0x14, [0u16, 0x200, 0xfe00, 0x400, 0xfc00][rng.next() as usize % 5]);
    wr16(b, obj + 0x16, [0u16, 0x20, 0xffe0][rng.next() as usize % 3]);
    wr16(b, obj + 0x18, [0u16, 0x80, 0x100, 0x600, 0xff00, 0xff80][rng.next() as usize % 6]);
    wr16(b, obj + 0x1a, [0u16, 0xff80, 0xffc0, 0xffe0][rng.next() as usize % 4]);
    wr16(b, obj + 0xc, [0u16, 1, 0x10, 0x11, 0x40, 0xfff0][rng.next() as usize % 6]);
    wr8(b, A5 + 0x2c, pick(rng, &[0, 5, 0xb, 0xc, 0xd]));
    wr16(b, A5 + 0x6e, (rng.next() % 8) as u16);
    wr16(b, A5 + 0x7f92, (rng.next() % 32) as u16);
    // hit code consistent with the reaction step
    if s.0 == 4 && s.1 == 0 {
        let codes: &[u8] = match s.2 {
            0 => &[0, 4, 0x1c, 0x4c, 0x68, 0xa4, 0xa8, 0x78, 0x38],
            2 => &[0, 4, 0x1c, 0x4c, 0x68, 0xa4, 0x78],
            4 => &[4, 0xa4, 0x14],
            _ => &[0xa4],
        };
        wr8(b, obj + 0x49, pick(rng, codes));
    }
    close_players(b, rng, obj);
}

/// Boss 8: scene variables of 17, own parameters.
fn prep_8(b: &mut Bus, rng: &mut Rng, obj: u32) {
    let pick = |a: &mut Rng, v: &[u8]| v[a.next() as usize % v.len()];
    prep_17(b, rng, obj);
    let s = (rd8(b, obj + 4), rd8(b, obj + 5), rd8(b, obj + 6));
    wr8(b, obj + 3, 0);
    wr8(b, obj + 7, step_8(rng, s.0, s.1, s.2));
    wr8(b, A5 + 0x5a, (rng.next() % 4) as u8);
    wr8(b, obj + 0xad, 1 + (rng.next() % 3) as u8);
    wr8(b, obj + 0x42, (rng.next() % 4 == 0) as u8);
    wr8(b, obj + 0x43, (rng.next() % 4 == 0) as u8);
    wr16(b, obj + 0x62, [0u16, 1, 10, 40, 100, 0x160][rng.next() as usize % 6]);
    wr16(b, obj + 0x18, [0u16, 0x100, 0xfd00, 0xfc00, 0x300][rng.next() as usize % 5]);
    wr8(b, obj + 0x3e, (rng.next() % 3 == 0) as u8);
    wr8(b, obj + 0x4a, pick(rng, &[0, 0x20]));
    wr8(b, obj + 0x46, (rng.next() % 2) as u8);
    wr8(b, obj + 0x87, (rng.next() % 2) as u8);
    wr16(b, obj + 0x60, [0x40u16, 0x100, 0x300][rng.next() as usize % 3]);
    wr16(b, obj + 0xb6, [0u16, 0x20, 0x50, 0x90][rng.next() as usize % 4]);
    wr16(b, obj + 0xb4, [0u16, 1, 0xf0, 0xffff][rng.next() as usize % 4]);
    wr8(b, obj + 0xb8, (rng.next() % 2) as u8);
    wr8(b, obj + 0xb9, (rng.next() % 2) as u8);
    wr8(b, obj + 0xba, (rng.next() % 3) as u8);
    wr8(b, obj + 0xbb, (rng.next() % 4) as u8);
    wr16(b, obj + 0xbc, [0u16, 1, 2, 0x12c][rng.next() as usize % 4]);
    wr16(b, obj + 0xbe, [0u16, 0xffff, 0x3c, 1][rng.next() as usize % 4]);
    wr8(b, obj + 0xab, pick(rng, &[0, 0, 1, 0x10]));
    wr8(b, obj + 0xac, (rng.next() % 4) as u8);
    wr8(b, obj + 0xaa, (rng.next() % 3) as u8);
    wr8(b, obj + 0xae, (rng.next() % 2) as u8);
    wr8(b, obj + 0xaf, 0);
    wr8(b, obj + 0xa2, pick(rng, &[0, 1, 3, 4, 0x7f]));
    wr8(b, obj + 0xa3, pick(rng, &[0, 1, 3, 4, 0x7f]));
    wr8(b, obj + 0xa5, pick(rng, &[0, 2, 4, 6]));
    wr32(b, obj + 0xa6, [0x0202_0000u32, 0x0202_0004, 0x0208_0004, 0x0204_0000][rng.next() as usize % 4]);
    wr16(b, obj + 0xa0, [0u16, 1, 2, 5, 0x14, 0x32][rng.next() as usize % 6]);
    wr16(b, obj + 0x84, (rng.next() % 4) as u16);
    wr8(b, obj + 0x6b, (rng.next() % 4) as u8);
    wr8(b, obj + 0x6e, (rng.next() % 4) as u8);
    wr8(b, obj + 0x71, (rng.next() % 4) as u8);
    wr16(b, A5 + 0x7f92, (rng.next() % 32) as u16);
    wr16(b, A5 + 0x6e, (rng.next() % 8) as u16);
    // the burst shots target distinct players: all alive
    if s.0 == 2 && s.1 == 4 && (s.2 == 4 || s.2 == 6 || s.2 == 14) {
        for j in 0..4u32 {
            wr8(b, 0xff85de + 0x200 * j, 1);
        }
        wr8(b, A5 + 0x35, 1 + (rng.next() % 3) as u8);
    }
    close_players(b, rng, obj);
}

/// Boss 18: phases, budgets, linked shot, hit codes valid for the reaction step.
fn prep_18(b: &mut Bus, rng: &mut Rng, obj: u32) {
    let pick = |a: &mut Rng, v: &[u8]| v[a.next() as usize % v.len()];
    prep_17(b, rng, obj);
    let s = (rd8(b, obj + 4), rd8(b, obj + 5), rd8(b, obj + 6));
    wr8(b, obj + 7, step_18(rng, s.0, s.1, s.2));
    wr8(b, A5 + 0x30, (rng.next() % 5 != 0) as u8);
    wr8(b, obj + 0xaa, (rng.next() % 3) as u8);
    wr8(b, obj + 0xab, 2);
    for off in [0xb2u32, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xae, 0xaf] {
        wr8(b, obj + off, pick(rng, &[0, 0, 1, 2, 5]));
    }
    wr16(b, obj + 0xa6, [0u16, 1, 2, 5, 0x14][rng.next() as usize % 5]);
    wr16(b, obj + 0xa8, (rng.next() % 6) as u16);
    wr16(b, obj + 0xb8, [0u16, 1, 5, 0x168][rng.next() as usize % 4]);
    wr8(b, obj + 0xba, (rng.next() % 4) as u8);
    wr8(b, obj + 0xbb, (rng.next() % 2) as u8);
    wr8(b, obj + 0x33, pick(rng, &[0, 0, 1, 0x81, 0xff]));
    wr8(b, obj + 0x90, (rng.next() % 20) as u8);
    wr8(b, obj + 0x4a, pick(rng, &[0, 0x20]));
    wr8(b, obj + 0x56, (rng.next() % 6) as u8);
    wr32(b, obj + 0xa2, [0x0202_0000u32, 0x0202_0400, 0x0202_0600, 0x0204_0400][rng.next() as usize % 4]);
    wr8(b, obj + 0xa0, pick(rng, &[0, 1, 2, 3, 4, 5, 0x10, 0x20]));
    wr8(b, obj + 0xa1, pick(rng, &[0, 1, 2, 3, 5, 0xff]));
    // hit code: all the specific reaction routines (the advanced steps accept only some of them)
    let codes: &[u8] = if s.0 == 4 && s.1 == 0 && s.2 >= 4 {
        &[0x20, 0x38, 0x44, 0x50, 0x5c, 0x9c, 0x8, 0x60]
    } else if s.0 == 4 && s.1 == 0 && s.2 == 2 {
        &[0, 0x1c, 0x20, 0x38, 0x44, 0x50, 0x5c, 0x9c, 0xa8]
    } else {
        &[0, 4, 0x1c, 0x20, 0x38, 0x44, 0x50, 0x5c, 0x9c, 0xa8]
    };
    wr8(b, obj + 0x49, pick(rng, codes));
    let shot = ddsom_game::objects::pools::ESHL.slot(rng.next() % 4).0;
    wr16(b, obj + 0xb0, shot as u16);
    wr8(b, shot, (rng.next() % 4 != 0) as u8);
    wr8(b, shot + 0x5a, (rng.next() % 2) as u8);
    close_players(b, rng, obj);
    for j in 0..4u32 {
        let q = 0xff85de + 0x200 * j;
        wr16(b, q + 0xc, [0u16, 0, 0x10][rng.next() as usize % 3]);
        wr32(b, q + 4, [0x0202_0000u32, 0x0200_2200, 0x0200_1400, 0x0204_0e00][rng.next() as usize % 4]);
        wr8(b, q + 0x66, (rng.next() % 3 == 0) as u8);
        wr8(b, q + 0x8b, (rng.next() % 6) as u8);
    }
}

/// Boss 21: same scene variables as 17; host (`+0x46`) sometimes dead, linked shot.
fn prep_21(b: &mut Bus, rng: &mut Rng, obj: u32) {
    let pick = |a: &mut Rng, v: &[u8]| v[a.next() as usize % v.len()];
    prep_17(b, rng, obj);
    wr8(b, obj + 3, 0);
    let s = (rd8(b, obj + 4), rd8(b, obj + 5), rd8(b, obj + 6));
    wr8(b, obj + 7, step_21(rng, s.0, s.1, s.2));
    if s.0 == 4 && s.1 == 0 && s.2 != 0 {
        wr8(b, obj + 0x49, 0x4c);
    }
    let host = ENEMY.slot(rng.next() % ENEMY.slots).0;
    wr16(b, obj + 0x46, host as u16);
    wr16(b, host + 4, if rng.next() % 6 == 0 { 0x0406 } else { 0x0202 });
    wr8(b, obj + 0x33, pick(rng, &[0, 0, 1, 0x81, 0xff]));
    wr16(b, obj + 0x84, (rng.next() % 4) as u16);
    wr8(b, obj + 0x90, (rng.next() % 20) as u8);
    wr16(b, obj + 0xa6, (rng.next() % 6) as u16);
    wr8(b, obj + 0xa0, pick(rng, &[0, 1, 2, 3, 5, 0x10, 0x20, 0x30, 0x31]));
    wr8(b, obj + 0xaa, (rng.next() % 5) as u8);
    wr8(b, obj + 0xab, 4);
    wr8(b, A5 + 0x2e, pick(rng, &[0, 4]));
    let shot = ddsom_game::objects::pools::ESHL.slot(rng.next() % 4).0;
    wr16(b, obj + 0xb0, shot as u16);
    wr8(b, shot, (rng.next() % 4 != 0) as u8);
    wr8(b, shot + 0x5a, (rng.next() % 2) as u8);
    close_players(b, rng, obj);
}

/// Players very close to the boss (one time in two) to reach the attack choices depending on range.
fn close_players(b: &mut Bus, rng: &mut Rng, obj: u32) {
    if rng.next() % 2 == 0 {
        return;
    }
    let (ox, oy) = (rd16(b, obj + 8), rd16(b, obj + 0x10));
    for j in 0..4u32 {
        let q = 0xff85de + 0x200 * j;
        wr8(b, q, 1);
        wr16(b, q + 8, ox.wrapping_add((rng.next() % 0x120) as u16).wrapping_sub(0x90));
        wr16(b, q + 0x10, oy.wrapping_add((rng.next() % 0x10) as u16).wrapping_sub(8));
    }
}

/// Boss 10: same scene variables as 17; own parameters.
fn prep_10(b: &mut Bus, rng: &mut Rng, obj: u32) {
    let pick = |a: &mut Rng, v: &[u8]| v[a.next() as usize % v.len()];
    prep_17(b, rng, obj);
    wr8(b, obj + 3, 0);
    let s = (rd8(b, obj + 4), rd8(b, obj + 5), rd8(b, obj + 6));
    wr8(b, obj + 7, step_10(rng, s.0, s.1, s.2));
    if s.0 == 4 && s.1 == 0 && s.2 != 0 {
        // steps 2 and 4 of the reaction are those of the stun (code 0x4c)
        wr8(b, obj + 0x49, 0x4c);
    }
    wr8(b, A5 + 0x2e, pick(rng, &[0, 6, 6]));
    wr16(b, obj + 0x84, (rng.next() % 6) as u16);
    wr8(b, obj + 0x90, (rng.next() % 20) as u8);
    wr8(b, obj + 0xae, pick(rng, &[0, 1, 2, 0xfa]));
    wr16(b, obj + 0xa6, (rng.next() % 5) as u16);
    wr16(b, obj + 0xaa, (rng.next() % 0x400) as u16);
    wr16(b, obj + 0xac, (rng.next() % 0x200) as u16);
    wr8(b, obj + 0x33, pick(rng, &[0, 0, 1, 0xff]));
    let shot = ddsom_game::objects::pools::ESHL.slot(rng.next() % 4).0;
    wr16(b, obj + 0xa8, shot as u16);
    wr8(b, shot, (rng.next() % 4 != 0) as u8);
    wr8(b, shot + 0x5a, (rng.next() % 2) as u8);
}

/// Global variables that the game loop modifies between two calls.
fn perturb(b: &mut Bus, rng: &mut Rng, obj: u32, leader: u32) {
    for (off, values) in [(0x4eu32, &[0u8, 0, 0, 0, 1][..]), (0x3d, &[0, 0, 0, 1]), (0x35, &[0, 1, 2, 3]), (0x36, &[0, 1])] {
        if rng.next() % 3 == 0 {
            wr8(b, A5 + off, values[rng.next() as usize % values.len()]);
        }
    }
    wr8(b, 0xff0ec6, [0u8, 0, 0, 0, 1][rng.next() as usize % 5]);
    if rng.next() % 3 == 0 {
        wr16(b, A5 + 0x506, [0u16, 0x100, 0x2e0, 0xe00, 0xf00, 0x1000, 0x1e00, 0x2d00, 0x3c00][rng.next() as usize % 9]);
    }
    if rng.next() % 4 == 0 {
        wr16(b, A5 + 0x51e, [0u16, 4, 8][rng.next() as usize % 3]);
    }
    // the camera leader: position around the boss, speed
    if rng.next() % 3 == 0 {
        let x = rd16(b, obj + 8).wrapping_add((rng.next() % 0x300) as u16).wrapping_sub(0x100);
        wr16(b, leader + 8, x);
        wr16(b, leader + 0x10, (rng.next() % 0x100) as u16 + 0x60);
        wr16(b, leader + 0x58, [0u16, 1, 2, 0xffff, 0xfffe][rng.next() as usize % 5]);
    }
    for j in 0..4u32 {
        if rng.next() % 4 == 0 {
            wr8(b, 0xff85de + 0x200 * j, [0u8, 1, 1][rng.next() as usize % 3]);
        }
    }
    // byte 0x22 of the cutscene message: finished or not
    let msg = rd16(b, obj + 0xb0) as u32 | 0xff0000;
    if rd16(b, obj + 0xb0) != 0 && rng.next() % 3 == 0 {
        wr8(b, msg + 0x22, (rng.next() % 2) as u8);
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let [_, roms, eeprom, scenario, warmup, rounds] = &a[..] else {
        eprintln!("usage: fuzz_boss <roms> <eeprom> <scenario> <warm-up frames> <rounds>");
        std::process::exit(2);
    };
    let ee = std::fs::read(eeprom).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(roms), ee.as_deref()).expect("roms");
    ddsom_game::install(&mut m);
    let script = ddsom_game::scenario::Scenario::load(Path::new(scenario)).expect("scenario");
    for f in 0..warmup.parse::<usize>().unwrap() {
        script.apply(f, &mut m);
        m.run_frame();
    }
    ddsom_game::set_check(true);
    std::panic::set_hook(Box::new(|_| {}));
    let seed: u64 = std::env::var("DDSOM_GRAINE").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    let mut rng = Rng(0x1234_5678_9abc_def1 ^ seed.wrapping_mul(0x9e37_79b9_7f4a_7c15));
    let filter = std::env::var("DDSOM_ENTREE").ok().and_then(|v| u32::from_str_radix(&v, 16).ok());
    let duration: u32 = std::env::var("DDSOM_PAS").ok().and_then(|v| v.parse().ok()).unwrap_or(60);
    let states = std::env::var("DDSOM_ETATS").is_ok();
    let coverage = std::env::var("DDSOM_COUVERTURE").is_ok();
    let mut seen: std::collections::BTreeSet<u32> = Default::default();
    let mut inputs: Vec<(u32, u8)> = Vec::new();
    {
        for k in 0..23u32 {
            let e = (m.bus.fetch16(0x39b12 + 4 * k) as u32) << 16 | m.bus.fetch16(0x39b12 + 4 * k + 2) as u32;
            if ddsom_game::all_natives().any(|n| n.entry == e) && filter.map_or(true, |f| f == e) {
                inputs.push((e, k as u8));
            }
        }
    }
    let (c0, b0) = (m.cpu.clone(), m.bus.clone());
    let (mut tests, mut shown) = (0, 0);
    for _ in 0..rounds.parse::<u32>().unwrap() {
        for &(entry, kind) in &inputs {
            let (mut c, mut b) = (c0.clone(), b0.clone());
            b.irq_horizon = u64::MAX;
            let obj = BOSS.slot(rng.next() % BOSS.slots);
            for off in (0..BOSS.stride).step_by(2) {
                wr16(&mut b, obj.0 + off, 0);
            }
            // the camera leader: a player, or any enemy
            let leader = 0xff85de + 0x200 * (rng.next() % 4);
            wr16(&mut b, A5 + 0x46, if rng.next() % 8 == 0 { 0 } else { leader as u16 });
            wr8(&mut b, obj.0, 1);
            wr8(&mut b, obj.0 + 2, kind);
            wr8(&mut b, obj.0 + 3, 0);
            wr8(&mut b, obj.0 + 0x28, BOSS.class);
            wr16(&mut b, obj.0 + 0x62, 40);
            for off in [0x8u32, 0xc, 0x10] {
                let v = (rd16(&mut b, leader + off) as u32) << 16 | (rng.next() & 0xffff);
                wr32(&mut b, obj.0 + off, v);
            }
            if states && rng.next() % 2 == 0 {
                let hitboxes = boss_states(kind);
                let (s4, s5, s6) = hitboxes[rng.next() as usize % hitboxes.len()];
                wr8(&mut b, obj.0 + 4, s4[rng.next() as usize % s4.len()]);
                wr8(&mut b, obj.0 + 5, s5[rng.next() as usize % s5.len()]);
                wr8(&mut b, obj.0 + 6, s6[rng.next() as usize % s6.len()]);
            }
            prep(kind, &mut b, &mut rng, obj.0);
            for step in 0..duration {
                if rd8(&mut b, obj.0) == 0 {
                    break;
                }
                if step > 0 {
                    perturb(&mut b, &mut rng, obj.0, leader);
                    if states && rng.next() % 3 == 0 {
                        let hitboxes = boss_states(kind);
                        let (s4, s5, s6) = hitboxes[rng.next() as usize % hitboxes.len()];
                        wr8(&mut b, obj.0 + 4, s4[rng.next() as usize % s4.len()]);
                        wr8(&mut b, obj.0 + 5, s5[rng.next() as usize % s5.len()]);
                        wr8(&mut b, obj.0 + 6, s6[rng.next() as usize % s6.len()]);
                        prep(kind, &mut b, &mut rng, obj.0);
                    }
                }
                // free-slot counters consistent with the pool contents
                for q in POOLS {
                    let free = (0..q.slots).filter(|&i| rd8(&mut b, q.slot(i).0) == 0).count();
                    wr16(&mut b, A5 + q.free, free as u16);
                }
                wr8(&mut b, obj.0 + 0x40, (rng.next() % 40 == 0) as u8);
                // the X flag is meaningless for a handler; the native sound routines keep it whereas
                // the original recomputes it: it is cleared so that this known mismatch does not mask the others
                c.sr &= !0x10;
                c.a[0] = obj.0;
                c.a[5] = A5;
                c.d[7] = step;
                c.d[0] = 4 * kind as u32;
                c.a[1] = entry;
                tests += 1;
                let before_names: std::collections::BTreeMap<String, u64> =
                    ddsom_game::report().iter().map(|(n, s)| (n.to_string(), s.mismatches)).collect();
                let before = before_names.values().sum::<u64>();
                let (cs, bs) = (c.clone(), b.clone());
                if coverage {
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
                let prev_obj: Vec<u8> = (0..BOSS.stride).map(|i| rd8(&mut b, obj.0 + i)).collect();
                if std::env::var("DDSOM_PAS_ETAT").is_ok() {
                    println!("step {step} states {:02x?}", &prev_obj[4..8]);
                }
                let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| ddsom_game::call(&mut c, &mut b, entry)));
                if r.is_err() {
                    // the original went off the rails (inconsistent entry): trajectory abandoned
                    break;
                }
                let after = ddsom_game::report().values().map(|s| s.mismatches).sum::<u64>();
                // `DDSOM_SEULS=prefix`: only show the mismatches of routines whose name starts that way
                let only = std::env::var("DDSOM_SEULS").ok();
                let concerns = only.as_deref().map_or(true, |p| {
                    ddsom_game::report().iter().any(|(n, s)| {
                        n.starts_with(p) && s.mismatches > before_names.get(&n.to_string()).copied().unwrap_or(0)
                    })
                });
                if after > before && concerns && shown < 12 {
                    shown += 1;
                    // routines whose mismatch count increased during this call
                    let faulty: Vec<String> = ddsom_game::report()
                        .iter()
                        .filter(|(n, s)| s.mismatches > before_names.get(&n.to_string()).copied().unwrap_or(0))
                        .map(|(n, s)| format!("{n} : {}", s.last.clone().unwrap_or_default()))
                        .collect();
                    println!("mismatch at {entry:06x} type {kind} step {step} object {:06x}: {}", obj.0, faulty.join(" ; "));
                    println!("  object before: {:02x?}", prev_obj);
                    if std::env::var("DDSOM_TRACE_ORIG").is_ok() {
                        let (mut c2, mut b2) = (cs.clone(), bs.clone());
                        c2.pc = entry;
                        c2.a[7] = cs.a[7].wrapping_sub(4);
                        ddsom_game::mem::wr32(&mut b2, c2.a[7], 0xfffe);
                        for _ in 0..200_000 {
                            if c2.pc == 0xfffe {
                                break;
                            }
                            let ins = m68k::decode(&mut b2, c2.pc);
                            if std::env::var("DDSOM_SANS_TRACE").is_err() {
                                println!("    {:06x} {:?}", c2.pc, ins.op);
                            }
                            c2.step(&mut b2);
                        }
                        // RAM bytes that differ between the native (b) and the original (b2)
                        let mut n = 0;
                        for a in 0xff8000u32..0x1000000 {
                            let (u, v) = (rd8(&mut b, a), rd8(&mut b2, a));
                            if u != v && n < 24 {
                                println!("    DIFF {a:06x} (objet+{:x}) natif {u:02x} original {v:02x}", a.wrapping_sub(obj.0));
                                n += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    let mut total = 0;
    for (name, st) in ddsom_game::report() {
        total += st.mismatches;
        println!("  {name:28} {:>7} calls {:>5} mismatches{}", st.calls, st.mismatches, st.first.map(|f| format!(" — {f}")).unwrap_or_default());
    }
    println!("{tests} handler calls");
    if let Ok(file) = std::env::var("DDSOM_COUVERTURE") {
        let text: Vec<String> = seen.iter().map(|a| format!("{a:06x}")).collect();
        std::fs::write(file, text.join("\n")).expect("coverage write");
    }
    println!("{}", if total == 0 { "VALID: no mismatch" } else { "FAILED" });
}
