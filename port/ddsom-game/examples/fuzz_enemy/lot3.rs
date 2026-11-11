//! Preparation of the states of the third ENEMY batch (types 0 to 15, 18 and 27) for `fuzz_enemy`: states
//! `(+4, +5, +6, +7)` accepted by the original's jump tables (outside these tables it jumps into
//! data) and fields set to values they tolerate.
//!
//! Each type describes its states with lines `"s4 s5 s6,s6,.. max7"`: `+4`, `+5`, the possible `+6` and
//! the largest valid (even) `+7` step for these `+6`.

use super::Rng;
use ddsom_game::world::mem::{rd16, rd8, wr16, wr32, wr8, A5};

type Bus = cps2::Cps2Bus;

/// The type belongs to the batch (states and fields are prepared here).
pub fn handles(kind: u8) -> bool {
    states(kind).is_some()
}

fn states(kind: u8) -> Option<&'static [&'static str]> {
    Some(match kind {
        1 => &[
            "0 0 0 0",
            // arrival, standing, hits, leaps, dodges, arrival fall
            "2 0 0,2,4 2",
            "2 0 6 8",
            "2 2 0,2,4,6 2",
            "2 4 0,2,4,6 10",
            "2 4 8,12 12",
            "2 4 10 6",
            "2 4 14 18",
            "2 4 16 20",
            "2 4 18,20 8",
            "2 6 0,10 2",
            "2 6 2,4,6 12",
            "2 6 8 8",
            "2 8 0,2,6 2",
            "2 8 4 10",
            "2 10 0,2,4,6 0",
            // hit
            "4 0 0 0",
            "4 2 0,2,4 0",
            "4 4 0,2,4 0",
            "4 6 0 0",
            "4 8 0,2,4,6 0",
            "4 10 0 0",
            "6 0 0 0",
        ],
        12 => &[
            "0 0 0 0",
            // arrival (the largest `+6` depends on the variant), standing, hits, after-leap hits, retreat, rest
            "2 0 0,2,4,6,8 0",
            "2 2 0,2,4 2",
            "2 4 0,2,4,6 2",
            "2 4 8 4",
            "2 4 10 2",
            "2 4 12 10",
            "2 4 14 8",
            "2 4 16 10",
            "2 4 18 10",
            "2 4 20 10",
            "2 4 22,24 0",
            "2 6 0,2 2",
            "2 6 4 0",
            "2 6 6 6",
            "2 6 8 8",
            "2 6 10 12",
            "2 6 12 8",
            "2 8 0 2",
            "2 8 2 18",
            "2 8 4 12",
            "2 8 6 0",
            "2 10 0,2 0",
            "4 0 0,2 0",
            "4 2 0,2 0",
            "4 4 0,2,4 0",
            "4 6 0 0",
            "4 8 0,2 0",
            "6 0 0 0",
        ],
        2 => &[
            "0 0 0 0",
            // arrival (the largest `+6` depends on the variant), standing, hits, guards, retreat, rest
            "2 0 0,2,4 0",
            "2 2 0,2,4 2",
            "2 2 6 4",
            "2 4 0,2,4,6,12,14,20,22,24,26 2",
            "2 4 8,10,18 4",
            "2 4 16 8",
            "2 6 0,2,4,6,8,10,14,16,18,20,22,24 2",
            "2 6 12 4",
            "2 8 0 2",
            "2 8 2 0",
            "2 8 4 6",
            "2 8 6 0",
            "2 10 0,2 0",
            "4 0 0,2 0",
            "4 2 0,2 0",
            "4 4 0,2,4 0",
            "4 6 0 0",
            "4 8 0,2,4 0",
            "6 0 0 0",
        ],
        5 => &[
            "0 0 0 0",
            // arrival (the largest `+6` depends on the variant), standing, hits, movements, rest
            "2 0 0,2,4,6,8 0",
            "2 2 0,2,4,6 2",
            "2 4 0,2,8,14 2",
            "2 4 4,10,12 6",
            "2 4 6 0",
            "2 4 16 4",
            "2 6 0,2 2",
            "2 6 4,6,8 6",
            "2 6 10 2",
            "2 8 0,2,4 4",
            "2 8 6 2",
            "2 10 0,2,4 0",
            "4 0 0,2 0",
            "4 2 0,2 0",
            "4 4 0,2,4 0",
            "4 6 0 0",
            "4 8 0,2,4 0",
            "6 0 0 0",
        ],
        9 => &[
            "0 0 0 0",
            // walk, trajectories per group (the largest `+7` depends on `+0x2f`), U-turn, leap, dormant
            "2 0 0,2 2",
            "2 2 0 2",
            "2 4 0 6",
            "2 4 2,4,6,8 8",
            "2 6 0,2,4 0",
            "2 8 0 0",
            "4 0 0,2,4 0",
            "4 2 0,2 0",
            "4 4 0,2 0",
            "6 0 0 0",
        ],
        8 => &[
            "0 0 0 0",
            // arrival, movements, hits, rest, grab
            "2 0 0 6",
            "2 0 2 6",
            "2 2 0,2,4,6 2",
            "2 2 8 10",
            "2 2 10 8",
            "2 4 0,2 4",
            "2 4 4 2",
            "2 4 6 6",
            "2 4 8 10",
            "2 4 10 8",
            "2 4 12 8",
            "2 6 0,2 0",
            "2 8 0,2 0",
            "2 10 0,2 0",
            "2 12 0 4",
            "2 14 0 0",
            "4 0 0,2 0",
            "4 2 0,2,4 0",
            "4 4 0,2,4 0",
            "4 6 0,2,4,6,8,10,12,14 0",
            "4 8 0,2,4,6 0",
            "4 10 0 0",
            "6 0 0 0",
        ],
        0 => &[
            "0 0 0 0",
            // arrival (sub-states 0 to 20), standing, hits, lurking and retreat gestures, stroll, hit
            "2 0 0 4",
            "2 0 2 2",
            "2 0 4,6,8,18,20 6",
            "2 0 10,12,16 4",
            "2 0 14 2",
            "2 2 0,8 2",
            "2 2 2,4 2",
            "2 2 6 4",
            "2 4 0,2,6 6",
            "2 4 4,8 8",
            "2 4 10,20 10",
            "2 4 12 12",
            "2 4 14,34 6",
            "2 4 16,36,18 10",
            "2 4 22,24,26,28,32 6",
            "2 4 30 14",
            "2 6 0 8",
            "2 6 2 6",
            "2 6 4,6,10 10",
            "2 6 8 12",
            "2 8 0,6 4",
            "2 8 2,4 8",
            "2 10 0,2 0",
            "4 0 0,2 0",
            "4 2 0,2 0",
            "4 4 0,2,4 0",
            "4 6 0 0",
            "4 8 0,2,4,6,8,10,12,14 0",
            "6 0 0 0",
        ],
        6 => &[
            "0 0 0 0",
            // arrival, standing, hits, lurking and retreat gestures, stroll, bite, hit
            "2 0 0,2,8,10 4",
            "2 0 4,6 6",
            "2 2 0,8 2",
            "2 2 2,4 2",
            "2 2 6 4",
            "2 2 10 6",
            "2 4 0 14",
            "2 4 2,4 6",
            "2 4 6 8",
            "2 4 8 12",
            "2 4 10,12,14 6",
            "2 4 16 8",
            "2 6 0,2 6",
            "2 6 4,6 10",
            "2 6 8,10 12",
            "2 6 12 14",
            "2 8 0,6 4",
            "2 8 2 6",
            "2 8 4 8",
            "2 10 0,2,4 0",
            "2 12 0,2 2",
            "4 0 0 0",
            "4 2 0,2 0",
            "4 4 0,2,4,6,8,10 0",
            "4 6 0 0",
            "4 8 0,2,4 0",
            "6 0 0 0",
        ],
        18 => &[
            "0 0 0 0",
            // arrival, movements, hits (thrown arms), rest, hit
            "2 0 0 14",
            "2 0 2,4,6 4",
            "2 2 0,2,4,6 2",
            "2 2 8 4",
            "2 2 10 8",
            "2 4 0 2",
            "2 4 2,4 8",
            "2 4 6 8",
            "2 4 8 12",
            "2 6 0,2 0",
            "2 8 0,2 0",
            "2 10 0,2 0",
            "4 0 0 0",
            "4 2 0,2,4 0",
            "4 4 0,2,4 0",
            "4 6 0,2,4,6,8 0",
            "4 8 0,2 0",
            "4 10 0 0",
            "6 0 0 0",
        ],
        10 => &[
            "0 0 0 0",
            // arrival (the largest `+7` of `+6` = 4), standing, hits, waiting, special hit, hits, hit
            "2 0 2,6,8 0",
            "2 0 4 4",
            "2 2 0,2,4 4",
            "2 4 0 0",
            "2 4 2,4 2",
            "2 4 6 6",
            "2 6 0,2 0",
            "2 8 0 6",
            "2 10 0 0",
            "2 10 2,4,6 2",
            "2 10 8 2",
            "4 0 0 0",
            "4 2 0,2 0",
            "4 4 0,2,4 0",
            "6 0 0 0",
        ],
        27 => &[
            "0 0 0 0",
            // arrival, rolls toward the player, weapon gestures (4), turns in place (6, 8, 10), hit
            "2 0 0 6",
            "2 0 2,4 4",
            "2 2 0,2,4,6 2",
            "2 2 6 2",
            "2 2 6 2",
            "2 2 8 6",
            "2 4 0,2,4 2",
            "2 4 6 6",
            "2 4 8 8",
            "2 4 10 6",
            "2 4 12 4",
            "2 6 0,2 0",
            "2 8 0,2 0",
            "2 10 0,2 0",
            "4 0 0 0",
            "4 0 2 0",
            "4 2 0,2,4 0",
            "4 4 0,2,4 0",
            "4 6 0,2,4,6,8 0",
            "4 8 0,2 0",
            "4 10 0,2 0",
            "6 0 0 0",
        ],
        11 => &[
            "0 0 0 0",
            // standing driven by the keys, hits (4), retreat gestures (6, 8), hit
            "2 0 0 0",
            "2 0 2 2",
            "2 2 0 0",
            "2 2 2 2",
            "2 4 0 2",
            "2 4 2,4,6 10",
            "2 4 8,12 12",
            "2 4 10 6",
            "2 4 14 18",
            "2 4 16 20",
            "2 4 18,20 8",
            "2 6 0 2",
            "2 6 2,4,6 12",
            "2 6 8 8",
            "2 6 10 2",
            "2 8 0,6 2",
            "2 8 2 2",
            "2 8 4 10",
            "4 0 0 0",
            "4 2 0,2 0",
            "4 4 0,2,4 0",
            "4 6 0 0",
            "4 8 0,2,4,6 0",
            "4 10 0 0",
            "6 0 0 0",
        ],
        3 => &[
            "0 0 0 0",
            // arrival (all variants), standing, shots (4: +6 = 0 first gesture, 2 second gesture), steps and jumps (6),
            // rest (8), waiting (10), U-turn (12), hit
            "2 0 0,2,4,6 0",
            "2 2 0,2,4 0",
            "2 4 0 10",
            "2 4 2 14",
            "2 4 4 0",
            "2 6 0 2",
            "2 6 2,8 6",
            "2 6 4,6 6",
            "2 8 0 0",
            "2 8 2,4 2",
            "2 8 6 0",
            "2 10 0,2 0",
            "2 12 0,2 0",
            "4 0 0 0",
            "4 2 0 0",
            "4 4 0,2,4 0",
            "4 6 0 0",
            "4 8 0,2,4 0",
            "6 0 0 0",
        ],
        13 => &[
            "0 0 0 0",
            // arrival (+6 = 6: variants 0 to 8, +6 = 8: variants 0 and 4, see `gnoll`), standing, gestures, rest, hit
            "2 0 0,2,4 0",
            "2 0 6 0",
            "2 0 8 0",
            "2 2 0,2,4 2",
            "2 4 0,2,4,6 2",
            "2 4 8 4",
            "2 4 10 2",
            "2 4 12 10",
            "2 4 14 8",
            "2 4 16 10",
            "2 4 18 14",
            "2 4 20 12",
            "2 4 22,24 0",
            "2 6 0,2 2",
            "2 6 4 0",
            "2 6 6 6",
            "2 6 8 8",
            "2 6 10 12",
            "2 6 12 8",
            "2 8 0 2",
            "2 8 2 18",
            "2 8 4 12",
            "2 8 6 0",
            "2 10 0,2 0",
            "4 0 0 0",
            "4 2 0,2 0",
            "4 4 0,2,4 2",
            "4 6 0 0",
            "4 8 0,2 0",
            "6 0 0 0",
        ],
        14 => &[
            "0 0 0 0",
            // arrival (+6 = 6: variants 2 and 4 only), standing, gestures (4: throw and jumps, 6: steps, jumps, 8: continuation)
            "2 0 0,2,4 0",
            "2 0 6 0",
            "2 2 0,2,4 2",
            "2 2 6 4",
            "2 4 0,2,12,14,16 2",
            "2 4 4 12",
            "2 4 6,8 10",
            "2 4 10 0",
            "2 4 18 2",
            "2 4 20 4",
            "2 6 0,2 2",
            "2 6 4 0",
            "2 6 6 6",
            "2 6 8 10",
            "2 6 10 12",
            "2 6 12 8",
            "2 8 0 2",
            "2 8 2 10",
            "2 8 4 12",
            "2 8 6 0",
            "2 10 0,2 0",
            "4 0 0 0",
            "4 2 0,2 0",
            "4 4 0,2,4 0",
            "4 6 0 0",
            "4 8 0,2 0",
            "6 0 0 0",
        ],
        15 => &[
            "0 0 0 0",
            // arrival (+6 = 6: variants 2 and 4 only), standing, gestures (4: series, 6: side steps, 8: continuation), rest, hit
            "2 0 0,2,4 0",
            "2 0 6 0",
            "2 2 0,2,4 2",
            "2 2 6 4",
            "2 4 0,2,12,14,16 2",
            "2 4 4 2",
            "2 4 6,8 10",
            "2 4 10 0",
            "2 4 18 2",
            "2 4 20 4",
            "2 6 0,2 2",
            "2 6 4 0",
            "2 6 6 6",
            "2 6 8 6",
            "2 6 10,12 8",
            "2 8 0 2",
            "2 8 2 6",
            "2 8 4 12",
            "2 8 6 0",
            "2 10 0,2 0",
            "4 0 0 0",
            "4 2 0,2 0",
            "4 4 0,2,4 0",
            "4 6 0 0",
            "4 8 0,2 0",
            "6 0 0 0",
        ],
        _ => return None,
    })
}

fn pair(rng: &mut Rng, max: u8) -> u8 {
    2 * (rng.next() % (max as u32 / 2 + 1)) as u8
}

/// Draws a valid state and sets `+4..+7`; `fresh`: creation of the object (variant, series...).
pub fn prep(b: &mut Bus, rng: &mut Rng, kind: u8, obj: u32, fresh: bool) {
    let lines = states(kind).unwrap();
    let line: Vec<&str> = lines[rng.next() as usize % lines.len()].split(' ').collect();
    let s6: Vec<u8> = line[2].split(',').map(|v| v.parse().unwrap()).collect();
    let max: u8 = line[3].parse().unwrap();
    wr8(b, obj + 4, line[0].parse().unwrap());
    wr8(b, obj + 5, line[1].parse().unwrap());
    wr8(b, obj + 6, s6[rng.next() as usize % s6.len()]);
    // step 0 is the most frequent (start of the gesture)
    let step = if rng.next().is_multiple_of(3) { 0 } else { pair(rng, max) };
    wr8(b, obj + 7, step);
    common(b, rng, obj);
    // second camera (two-screen mode) and difficulty, read by several types
    if rng.next().is_multiple_of(4) {
        wr8(b, A5 + 0x36, 1);
        wr16(b, A5 + 0x556, (rng.next() % 0x200) as u16);
        wr16(b, A5 + 0x55a, (rng.next() % 0x200) as u16);
    } else {
        wr8(b, A5 + 0x36, 0);
    }
    match kind {
        1 => skeleton(b, rng, obj, fresh),
        12 => elf(b, rng, obj, fresh),
        2 => troglodyte(b, rng, obj, fresh),
        5 => ghoul(b, rng, obj, fresh),
        9 => beetle(b, rng, obj, fresh),
        10 => scorpion(b, rng, obj, fresh),
        8 => gargoyle(b, rng, obj, fresh),
        6 => dog(b, rng, obj, fresh),
        0 => kobold(b, rng, obj, fresh),
        18 => ogre(b, rng, obj, fresh),
        15 => goblin(b, rng, obj, fresh),
        14 => troll(b, rng, obj, fresh),
        13 => gnoll(b, rng, obj, fresh),
        3 => archer(b, rng, obj, fresh),
        11 => golem(b, rng, obj, fresh),
        27 => machine(b, rng, obj, fresh),
        _ => {}
    }
}

/// Fields that almost all enemies read: target, reaction code, orientation, height.
fn common(b: &mut Bus, rng: &mut Rng, obj: u32) {
    wr16(b, obj + 0x50, 0x85de + 0x200 * (rng.next() % 4) as u16);
    wr8(b, obj + 0x49, 4 * (rng.next() % 43) as u8);
    wr8(b, obj + 0x4a, 0x20 * (rng.next() % 2) as u8);
    wr8(b, obj + 0x2c, 0x20 * (rng.next() % 2) as u8);
    // at ground level, sometimes in the air, sometimes falling back
    let h = [0u16, 0, 0, 1, 2, 0x30][rng.next() as usize % 6];
    wr16(b, obj + 0xc, h);
    if rng.next().is_multiple_of(2) {
        wr16(b, obj + 0x18, [0x400u16, 0xff00, 0x100, 0xfc00][rng.next() as usize % 4]);
        wr16(b, obj + 0x1a, 0xff80);
    }
    for off in [0x14u32, 0x16, 0x1c, 0x1e] {
        wr16(b, obj + off, [0u16, 0x200, 0xfe00, 0x300, 0x480][rng.next() as usize % 5]);
    }
    wr8(b, obj + 0x2d, [0u8, 8, 0x10, 0x18][rng.next() as usize % 4]);
    wr8(b, obj + 0x32, rng.next() as u8);
    wr8(b, obj + 0x33, [0u8, 1, 0xff, 0x80][rng.next() as usize % 4]);
    wr16(b, obj + 0x84, (rng.next() % 3) as u16);
    wr8(b, obj + 0x86, rng.next().is_multiple_of(4) as u8);
    // the targeted player is alive
    let j = 0xff0000 | rd16(b, obj + 0x50) as u32;
    if !rng.next().is_multiple_of(4) {
        wr8(b, j, 1);
    }
}

fn skeleton(b: &mut Bus, rng: &mut Rng, obj: u32, fresh: bool) {
    if fresh {
        wr8(b, obj + 3, (rng.next() % 6) as u8);
        wr8(b, obj + 0x2f, (rng.next() % 2) as u8);
        wr16(b, obj + 0x42, (rng.next() % 3) as u16);
    }
    wr8(b, obj + 0x8f, [0u8, 0, 0, 0, 1, 2, 3][rng.next() as usize % 7]);
    for off in [0x6bu32, 0x6e, 0x71, 0x77] {
        wr8(b, obj + off, (rng.next() % 5) as u8);
    }
    wr8(b, obj + 0x68, [3u8, 8, 9, 2][rng.next() as usize % 4]);
    wr8(b, obj + 0x79, [0xbu8, 0xc][rng.next() as usize % 2]);
    wr16(b, obj + 0xa2, (rng.next() % 4) as u16);
    let (ox, oy) = (rd16(b, obj + 8), rd16(b, obj + 0x10));
    wr16(b, obj + 0xa4, ox.wrapping_add((rng.next() % 0x80) as u16).wrapping_sub(0x40));
    wr8(b, obj + 0xa6, [0u8, 2, 3][rng.next() as usize % 3]);
    wr8(b, obj + 0xa7, rng.next().is_multiple_of(6) as u8);
    wr8(b, obj + 0xa8, [0u8, 0, 1, 0xff][rng.next() as usize % 4]);
    wr8(b, obj + 0xa9, [0u8, 2][rng.next() as usize % 2]);
    wr16(b, obj + 0x7a, ox.wrapping_add((rng.next() % 0x80) as u16));
    wr16(b, obj + 0x7c, oy.wrapping_add((rng.next() % 0x40) as u16));
    wr32(b, obj + 0x4c, [0x1c74b6u32, 0x1c7336][rng.next() as usize % 2]);
    // the enemy that created this one (counter `+0xaf`): another existing object
    wr8(b, obj + 0xac, rng.next().is_multiple_of(3) as u8);
    wr16(b, obj + 0xaa, 0x85de + 0x200 * (rng.next() % 4) as u16);
    wr8(b, obj + 0x29, (rng.next() % 2) as u8);
    let _ = rd8(b, obj);
}

fn elf(b: &mut Bus, rng: &mut Rng, obj: u32, fresh: bool) {
    if fresh {
        // the variant is a table index: 0, 2, 4 or 6
        wr8(b, obj + 3, 2 * (rng.next() % 4) as u8);
    }
    // the arrival has only a few `+6` depending on the variant
    if rd8(b, obj + 4) == 2 && rd8(b, obj + 5) == 0 {
        let largest = match rd8(b, obj + 3) {
            0 => 2,
            2 | 6 => 6,
            _ => 8,
        };
        let s6 = rd8(b, obj + 6).min(largest);
        wr8(b, obj + 6, s6);
    }
    // hit planned after the approach: a valid `+6` of the current group
    let largest = match rd8(b, obj + 5) {
        6 => 12,
        8 => 6,
        _ => 24,
    };
    wr8(b, obj + 0xa5, pair(rng, largest));
    wr8(b, obj + 0xa6, (rng.next() % 3) as u8);
    for off in [0x6bu32, 0x6e, 0x71, 0x77, 0xa2] {
        wr8(b, obj + off, (rng.next() % 5) as u8);
    }
    wr8(b, obj + 0xa1, rng.next() as u8);
    wr8(b, obj + 0xa4, (rng.next() % 4) as u8);
    wr8(b, obj + 0x79, 0x1a + (rng.next() % 3) as u8);
    wr8(b, obj + 0x67, (rng.next() % 2) as u8);
    wr8(b, obj + 0x1, 0);
    wr16(b, obj + 0x84, (rng.next() % 3) as u16);
    wr8(b, A5 + 0x30, !rng.next().is_multiple_of(5) as u8);
    wr8(b, obj + 0x3b, 0);
}

fn troglodyte(b: &mut Bus, rng: &mut Rng, obj: u32, fresh: bool) {
    if fresh {
        wr8(b, obj + 3, (rng.next() % 6) as u8);
        wr8(b, obj + 0x2f, (rng.next() % 2) as u8);
    }
    // the arrival has only a few `+6` depending on the variant
    if rd8(b, obj + 4) == 2 && rd8(b, obj + 5) == 0 {
        let largest = match rd8(b, obj + 3) {
            0 | 3 | 4 => 2,
            _ => 4,
        };
        let s6 = rd8(b, obj + 6).min(largest);
        wr8(b, obj + 6, s6);
    }
    // hit to resume: a valid `+6` of the current group
    let largest = match rd8(b, obj + 5) {
        6 => 24,
        8 => 6,
        _ => 26,
    };
    wr8(b, obj + 0xa3, pair(rng, largest));
    for off in [0x6bu32, 0x6e, 0x71, 0x77, 0xa2] {
        wr8(b, obj + off, (rng.next() % 5) as u8);
    }
    wr8(b, obj + 0x68, [3u8, 4, 9][rng.next() as usize % 3]);
    wr8(b, obj + 0x79, 0x14 + (rng.next() % 2) as u8);
    wr8(b, obj + 0x67, (rng.next() % 2) as u8);
    for off in [0xa0u32, 0xa1] {
        wr8(b, obj + off, [0u8, 1, 0xe, 0xdf, 0xe0, 0xf0, 0x7f, 0xee, rng.next() as u8][rng.next() as usize % 9]);
    }
    // blinking: phase, alternation, wait, counters
    wr8(b, obj + 0xa5, [0xffu8, 0xff, 0, 1, 2, 3][rng.next() as usize % 6]);
    wr8(b, obj + 0xa6, (rng.next() % 2) as u8);
    wr8(b, obj + 0xa7, (rng.next() % 2) as u8);
    wr16(b, obj + 0xa8, [0u16, 1, 5, 0xc, 0xffff][rng.next() as usize % 5]);
    wr16(b, obj + 0xaa, [0u16, 1, 5, 0xffff][rng.next() as usize % 4]);
    wr16(b, obj + 0xae, [0u16, 0xffff, 0x258, 0x8000][rng.next() as usize % 4]);
    wr8(b, obj + 0xb0, rng.next().is_multiple_of(4) as u8);
    wr8(b, obj + 0x84, (rng.next() % 3) as u8);
}

fn ghoul(b: &mut Bus, rng: &mut Rng, obj: u32, fresh: bool) {
    if fresh {
        wr8(b, obj + 3, (rng.next() % 6) as u8);
    }
    // the arrival has only a few `+6` depending on the variant
    if rd8(b, obj + 4) == 2 && rd8(b, obj + 5) == 0 {
        let largest = match rd8(b, obj + 3) {
            0 | 4 => 2,
            2 | 3 => 4,
            1 => 6,
            _ => 8,
        };
        let s6 = rd8(b, obj + 6).min(largest);
        wr8(b, obj + 6, s6);
    }
    wr8(b, obj + 0x8f, [0u8, 0, 0, 0, 1, 2, 3][rng.next() as usize % 7]);
    for off in [0x6bu32, 0x6e, 0x71, 0xa2] {
        wr8(b, obj + off, (rng.next() % 5) as u8);
    }
    for off in [0xa0u32, 0xa1] {
        wr8(b, obj + off, [0u8, 2, 4, 6, 8, 0xff, rng.next() as u8][rng.next() as usize % 7]);
    }
    wr8(b, obj + 0x79, [0u8, 9][rng.next() as usize % 2]);
    wr8(b, obj + 0xa6, (rng.next() % 2) as u8);
    wr16(b, obj + 0xa4, 0x85de + 0x200 * (rng.next() % 4) as u16);
    let ox = rd16(b, obj + 8);
    wr16(b, obj + 0xa8, ox.wrapping_add((rng.next() % 4) as u16));
    wr16(b, obj + 0x84, (rng.next() % 3) as u16);
    wr8(b, obj + 0x86, rng.next().is_multiple_of(4) as u8);
    wr16(b, obj + 0x42, [0x703u16, 0x307, 0x110, rng.next() as u16 & 0x707][rng.next() as usize % 4]);
}

fn beetle(b: &mut Bus, rng: &mut Rng, obj: u32, fresh: bool) {
    if fresh {
        wr8(b, obj + 3, (rng.next() % 5) as u8);
        wr8(b, obj + 0x2f, (rng.next() % 8) as u8);
        wr8(b, obj + 0x42, (rng.next() % 4) as u8);
    }
    // `+7` is a step of the table of group `+0x2f`: the drift (groups 2 and 3) has only two
    if rd8(b, obj + 4) == 2 && rd8(b, obj + 5) == 4 && rd8(b, obj + 6) == 0 {
        let largest = if matches!(rd8(b, obj + 0x2f), 2 | 3) { 2 } else { 6 };
        let e = rd8(b, obj + 7).min(largest);
        wr8(b, obj + 7, e);
    }
    wr8(b, obj + 0xa4, 0x10 * (rng.next() % 2) as u8);
    // step to resume after a side walk (2 or 6 in the game)
    wr8(b, obj + 0xa6, [2u8, 6][rng.next() as usize % 2]);
    wr8(b, obj + 0xa7, 2 * (rng.next() % 3) as u8);
    wr8(b, obj + 0xa2, 1 + (rng.next() % 4) as u8);
    wr8(b, obj + 0xa3, 1 + (rng.next() % 3) as u8);
    wr8(b, obj + 0xb8, (rng.next() % 2) as u8);
    wr16(b, obj + 0xb4, [0u16, 0x300, 0xfd00][rng.next() as usize % 3]);
    wr16(b, obj + 0xb6, [0u16, 0x80, 0x40, 0xfff0, 0x140][rng.next() as usize % 5]);
    wr8(b, obj + 0x6e, 1 + (rng.next() % 4) as u8);
    wr8(b, obj + 0x56, (rng.next() % 4) as u8);
    wr8(b, obj + 0x53, rng.next().is_multiple_of(4) as u8);
    let (ox, oy) = (rd16(b, obj + 8), rd16(b, obj + 0x10));
    wr16(b, obj + 0xa8, ox.wrapping_add((rng.next() % 0x80) as u16));
    wr16(b, obj + 0xac, oy.wrapping_add((rng.next() % 0x40) as u16));
    wr16(b, obj + 0xb0, ox);
    wr16(b, obj + 0xb2, oy);
    wr32(b, obj + 0x4c, [0x1c9036u32, 0x1c7336][rng.next() as usize % 2]);
    wr16(b, obj + 0x32, [0u16, 0x8000, 0x0080, 0xff00][rng.next() as usize % 4]);
    wr8(b, obj + 0x29, (rng.next() % 2) as u8);
    wr16(b, obj + 0x5c, 0x85de + 0x200 * (rng.next() % 4) as u16);
    // grabbed by a player (0x2810): rare
    wr8(b, A5 + 0x42, rng.next().is_multiple_of(4) as u8);
    if rng.next().is_multiple_of(2) {
        wr16(b, obj + 0x46, 0);
    }
    // the bite (0x661ce) triggers only if the target is in state 0x200, step 0xc
    if rng.next().is_multiple_of(3) {
        let p = 0xff0000 | rd16(b, obj + 0x50) as u32;
        wr16(b, p + 4, 0x200);
        wr8(b, p + 6, 0xc);
    }
}

fn scorpion(b: &mut Bus, rng: &mut Rng, obj: u32, fresh: bool) {
    if fresh {
        wr8(b, obj + 3, (rng.next() % 6) as u8);
        wr8(b, obj + 0x2f, (rng.next() % 5) as u8);
    }
    wr8(b, obj + 0xa2, 1 + (rng.next() % 4) as u8);
    wr8(b, obj + 0xa6, (rng.next() % 3) as u8);
    wr8(b, obj + 0xa7, (rng.next() % 2) as u8);
    wr8(b, obj + 0xa8, [0u8, 0, 1, 0xff][rng.next() as usize % 4]);
    wr8(b, obj + 0x6e, 1 + (rng.next() % 4) as u8);
    wr8(b, obj + 0x53, rng.next().is_multiple_of(4) as u8);
    wr8(b, obj + 0x56, (rng.next() % 4) as u8);
    wr16(b, obj + 0x32, [0u16, 0x8000, 0x0080, 0xff00][rng.next() as usize % 4]);
    wr8(b, obj + 0x29, (rng.next() % 2) as u8);
    let ox = rd16(b, obj + 8);
    wr16(b, obj + 0xa4, ox.wrapping_add((rng.next() % 0x40) as u16));
    // parent of the "child" scorpions (+6 = 2 of the standing state): an existing object, in a drawn state
    wr16(b, obj + 0x5c, 0x85de + 0x200 * (rng.next() % 4) as u16);
    if rng.next().is_multiple_of(6) {
        let parent = 0xff0000 | rd16(b, obj + 0x5c) as u32;
        wr8(b, parent + 4, 4);
    }
    wr8(b, A5 + 0x42, rng.next().is_multiple_of(4) as u8);
    if rng.next().is_multiple_of(2) {
        wr16(b, obj + 0x46, 0);
    }
}

fn gargoyle(b: &mut Bus, rng: &mut Rng, obj: u32, fresh: bool) {
    if fresh {
        // variant 2 jumps into data in the original: it never appears in the game
        wr8(b, obj + 3, (rng.next() % 2) as u8);
    }
    // valid states to restore (`+0xa2` after a U-turn, `+0xb8` after a hit that missed)
    let valid = [0x0202_0000u32, 0x0202_0400, 0x0202_0600, 0x0202_0800, 0x0204_0000, 0x0204_0200, 0x0204_0400, 0x0204_0c00, 0x020a_0000];
    wr32(b, obj + 0xa2, valid[rng.next() as usize % valid.len()]);
    wr32(b, obj + 0xb8, valid[rng.next() as usize % valid.len()]);
    wr8(b, obj + 0xb7, 4 * (rng.next() % 43) as u8);
    // target: sometimes none (positive word)
    if rng.next().is_multiple_of(8) {
        wr16(b, obj + 0x50, 0);
    }
    // seized victim
    let victim = 0x85de + 0x200 * (rng.next() % 4) as u16;
    wr16(b, obj + 0x54, if rng.next().is_multiple_of(5) { 0 } else { victim });
    let v = 0xff0000 | victim as u32;
    wr8(b, v + 0x10d, [0xdu8, 0x10, 0x11, 0x13, 0x14, 0x0, 0xf][rng.next() as usize % 7]);
    wr8(b, v + 0x188, !rng.next().is_multiple_of(4) as u8);
    wr8(b, v, !rng.next().is_multiple_of(5) as u8);
    wr8(b, obj + 0x57, [0u8, 1, 4, 5][rng.next() as usize % 4]);
    wr8(b, obj + 0xaf, (rng.next() % 2) as u8);
    wr8(b, obj + 0xaa, (rng.next() % 3) as u8);
    wr8(b, obj + 0xab, 2);
    wr8(b, obj + 0xac, 2 * (rng.next() % 16) as u8);
    wr16(b, obj + 0x62, (rng.next() % 40) as u16);
    wr16(b, obj + 0x64, 40);
    wr8(b, obj + 0xad, rng.next().is_multiple_of(3) as u8);
    wr8(b, obj + 0xb2, (rng.next() % 2) as u8);
    wr16(b, obj + 0xb4, [0u16, 0, 1, 5, 0x1e][rng.next() as usize % 5]);
    for off in [0x6bu32, 0x6e, 0x71] {
        wr8(b, obj + off, (rng.next() % 4) as u8);
    }
    for off in [0xa6u32, 0xa8] {
        wr16(b, obj + off, [0u16, 1, 2, 3, 0x14][rng.next() as usize % 5]);
    }
    wr8(b, obj + 0xa0, [0u8, 1, 2, 3, 4, 5, 0x10, 0x20, 0x30, 0x40][rng.next() as usize % 10]);
    wr8(b, obj + 0xa1, [0u8, 1, 2, 3, 4, 5, 0xff][rng.next() as usize % 7]);
    wr8(b, obj + 0x33, [0u8, 4, 4, 1, 0x80][rng.next() as usize % 5]);
    wr8(b, obj + 0x32, rng.next() as u8);
    wr8(b, obj + 0x86, rng.next().is_multiple_of(4) as u8);
    wr8(b, obj + 0x67, (rng.next() % 2) as u8);
    wr8(b, obj + 0x79, 0x3b);
    // AI parameters read by decisions 0xa4b4 and 0xa4c2
    for off in [0x72u32, 0x73] {
        wr8(b, obj + off, (rng.next() % 4) as u8);
    }
    wr16(b, obj + 0x84, (rng.next() % 3) as u16);
    let (ox, oy) = (rd16(b, obj + 8), rd16(b, obj + 0x10));
    wr16(b, obj + 0xbc, ox.wrapping_add((rng.next() % 0x80) as u16));
    wr16(b, obj + 0xbe, oy.wrapping_add((rng.next() % 0x40) as u16));
    // players placed at the distances tested by the decision boxes (in front or behind, close depth)
    if rng.next().is_multiple_of(2) {
        let offsets = [-0x90i16, -0x3a, -0x10, 0, 0x20, 0x40, 0x80, 0x90];
        for j in 0..4u32 {
            let q = 0xff85de + 0x200 * j;
            let off = offsets[rng.next() as usize % offsets.len()] + (rng.next() % 0x12) as i16 - 9;
            wr8(b, q, !rng.next().is_multiple_of(5) as u8);
            wr16(b, q + 8, ox.wrapping_add(off as u16));
            wr16(b, q + 0x10, oy.wrapping_add((rng.next() % 9) as u16).wrapping_sub(4));
            wr8(b, q + 0x2c, 0x20 * (rng.next() % 2) as u8);
        }
    }
    wr8(b, A5 + 0x30, rng.next().is_multiple_of(3) as u8);
    wr8(b, A5 + 0x35, (rng.next() % 5) as u8);
    wr8(b, A5 + 0x42, rng.next().is_multiple_of(4) as u8);
    if rng.next().is_multiple_of(2) {
        wr16(b, obj + 0x46, 0);
    }
    // the cameras are sometimes near the object (out of zone otherwise)
    let cam = ox.wrapping_sub(0x80 + (rng.next() % 0x100) as u16);
    wr16(b, A5 + 0x5a6, cam);
    wr16(b, A5 + 0x5ae, cam);
}

fn dog(b: &mut Bus, rng: &mut Rng, obj: u32, fresh: bool) {
    if fresh {
        wr8(b, obj + 3, (rng.next() % 8) as u8);
        wr8(b, obj + 0x2f, (rng.next() % 2) as u8);
    }
    // state to resume after a U-turn: (`+0xa6`, `+0xa4`) consistent
    let resumes: [(u32, u8); 8] = [
        (0x0202_0000, 0),
        (0x0202_0000, 4),
        (0x0204_0000, 0),
        (0x0204_0000, 8),
        (0x0206_0000, 4),
        (0x0206_0000, 10),
        (0x0208_0004, 0),
        (0x0208_0004, 2),
    ];
    let (state, sub) = resumes[rng.next() as usize % resumes.len()];
    wr32(b, obj + 0xa6, state);
    wr8(b, obj + 0xa4, sub);
    for off in [0x6bu32, 0x6e, 0x71, 0x77] {
        wr8(b, obj + off, (rng.next() % 4) as u8);
    }
    wr8(b, obj + 0xa0, (rng.next() % 4) as u8);
    wr8(b, obj + 0x90, 1 + (rng.next() % 3) as u8);
    wr8(b, obj + 0xa1, [0u8, 1, 2, 3, 4, 0xc, 0xff][rng.next() as usize % 7]);
    wr8(b, obj + 0xa2, [0u8, 1, 2, 3, 4, 0xc, 0x7f][rng.next() as usize % 7]);
    wr16(b, obj + 0xaa, [0u16, 1, 2, 0x400][rng.next() as usize % 4]);
    wr8(b, obj + 0xac, (rng.next() % 14) as u8);
    wr8(b, obj + 0xad, rng.next().is_multiple_of(3) as u8);
    wr8(b, obj + 0xaf, rng.next().is_multiple_of(4) as u8);
    wr8(b, obj + 0xb0, 1 + (rng.next() % 8) as u8);
    wr8(b, obj + 0x33, [0u8, 4, 1, 0x80][rng.next() as usize % 4]);
    wr8(b, obj + 0x32, rng.next() as u8);
    wr8(b, obj + 0x43, (rng.next() % 4) as u8);
    wr16(b, obj + 0x84, (rng.next() % 3) as u16);
    wr8(b, obj + 0x86, rng.next().is_multiple_of(4) as u8);
    wr8(b, obj + 0x53, rng.next().is_multiple_of(4) as u8);
    wr8(b, obj + 0x56, (rng.next() % 4) as u8);
    wr8(b, obj + 0x29, (rng.next() % 2) as u8);
    // AI parameters read by 0xa4b4 / 0xa4c2 and the hit tables
    for off in [0x72u32, 0x73] {
        wr8(b, obj + off, (rng.next() % 4) as u8);
    }
    if rng.next().is_multiple_of(8) {
        wr16(b, obj + 0x50, 0);
    }
    // bite victim (+0x50): alive and on the ground most of the time
    let v = 0xff0000 | rd16(b, obj + 0x50) as u32;
    wr8(b, v + 0x188, !rng.next().is_multiple_of(5) as u8);
    wr16(b, v + 0x62, [0u16, 1, 2, 5, 30][rng.next() as usize % 5]);
    let (ox, oy) = (rd16(b, obj + 8), rd16(b, obj + 0x10));
    if rng.next().is_multiple_of(2) {
        let offsets = [-0xc0i16, -0x40, -0x28, -8, 0, 0x20, 0x40, 0x80, 0x100];
        for j in 0..4u32 {
            let q = 0xff85de + 0x200 * j;
            let off = offsets[rng.next() as usize % offsets.len()] + (rng.next() % 0x12) as i16 - 9;
            wr8(b, q, !rng.next().is_multiple_of(5) as u8);
            wr16(b, q + 8, ox.wrapping_add(off as u16));
            wr16(b, q + 0x10, oy.wrapping_add((rng.next() % 0x14) as u16).wrapping_sub(8));
            wr8(b, q + 0x2c, 0x20 * (rng.next() % 2) as u8);
        }
    }
    wr8(b, A5 + 0x34, if rng.next().is_multiple_of(3) { 4 } else { (rng.next() % 12) as u8 });
    wr8(b, A5 + 0x35, (rng.next() % 5) as u8);
    wr8(b, A5 + 0x42, rng.next().is_multiple_of(4) as u8);
    if rng.next().is_multiple_of(2) {
        wr16(b, obj + 0x46, 0);
    }
    wr16(b, A5 + 0x6e, rng.next() as u16);
}

fn ogre(b: &mut Bus, rng: &mut Rng, obj: u32, fresh: bool) {
    if fresh {
        wr8(b, obj + 3, [0u8, 1, 2, 0xff, 0xff][rng.next() as usize % 5]);
    }
    // state to restore after a U-turn
    let valid = [0x0202_0000u32, 0x0202_0400, 0x0202_0600, 0x0202_0800, 0x0204_0000, 0x0204_0200, 0x0204_0400, 0x020a_0000];
    wr32(b, obj + 0xa2, valid[rng.next() as usize % valid.len()]);
    // thrown arm (+0xb0) and accompanying ogre (+0x46): existing objects
    let other = 0x85de + 0x200 * (rng.next() % 4) as u16;
    // during the arrival the arm does not exist yet: the end of waiting (0x75c36) rereads `+0xb1` there
    let arrival = rd8(b, obj + 5) == 0;
    wr16(b, obj + 0xb0, if arrival { 0 } else { other });
    wr16(b, obj + 0x46, other);
    let a = 0xff0000 | other as u32;
    wr8(b, a + 0x5a, (rng.next() % 2) as u8);
    wr16(b, a + 0x62, [0u16, 5, 30][rng.next() as usize % 3]);
    if rng.next().is_multiple_of(8) {
        wr16(b, obj + 0x50, 0);
    }
    wr8(b, obj + 0xaa, (rng.next() % 3) as u8);
    wr8(b, obj + 0xab, 2);
    wr8(b, obj + 0xac, 2 * (rng.next() % 16) as u8);
    wr16(b, obj + 0x62, [0u16, 5, 30][rng.next() as usize % 3]);
    wr16(b, obj + 0x64, 40);
    wr8(b, obj + 0xad, rng.next().is_multiple_of(3) as u8);
    wr8(b, obj + 0xb2, (rng.next() % 2) as u8);
    wr8(b, obj + 0xb3, (rng.next() % 2) as u8);
    for off in [0x6bu32, 0x6e, 0x71] {
        wr8(b, obj + off, (rng.next() % 4) as u8);
    }
    for off in [0xa6u32, 0xa8] {
        wr16(b, obj + off, [0u16, 1, 2, 3, 0x14, 0xa, 0x28][rng.next() as usize % 7]);
    }
    wr8(b, obj + 0xa0, [0u8, 1, 2, 3, 4, 5, 0x10, 0x20, 0x31, 0x40][rng.next() as usize % 10]);
    wr8(b, obj + 0xa1, [0u8, 1, 2, 3, 4, 5, 0xff][rng.next() as usize % 7]);
    wr8(b, obj + 0x33, [0u8, 1, 2, 3, 4, 0x80, 0x82, 0x84][rng.next() as usize % 8]);
    wr8(b, obj + 0x32, rng.next() as u8);
    wr8(b, obj + 0x86, rng.next().is_multiple_of(4) as u8);
    wr8(b, obj + 0x67, (rng.next() % 2) as u8);
    wr16(b, obj + 0x84, (rng.next() % 3) as u16);
    for off in [0x72u32, 0x73] {
        wr8(b, obj + off, (rng.next() % 4) as u8);
    }
    let (ox, oy) = (rd16(b, obj + 8), rd16(b, obj + 0x10));
    wr16(b, obj + 0xbc, ox.wrapping_add((rng.next() % 0x80) as u16));
    if rng.next().is_multiple_of(2) {
        let offsets = [-0xc0i16, -0x80, -0x60, -0x40, -0x19, 0, 0x20, 0x40, 0x80];
        for j in 0..4u32 {
            let q = 0xff85de + 0x200 * j;
            let off = offsets[rng.next() as usize % offsets.len()] + (rng.next() % 0x12) as i16 - 9;
            wr8(b, q, !rng.next().is_multiple_of(5) as u8);
            wr16(b, q + 8, ox.wrapping_add(off as u16));
            wr16(b, q + 0x10, oy.wrapping_add((rng.next() % 9) as u16).wrapping_sub(4));
            wr8(b, q + 0x2c, 0x20 * (rng.next() % 2) as u8);
            // player on the ground or in grab position: 0x7ef4 and 0x7f0c test its state
            wr8(b, q + 6, [0u8, 0xc, 0xc, 0xe][rng.next() as usize % 4]);
            wr16(b, q + 0xc, 0);
        }
    }
    wr8(b, A5 + 0x30, rng.next().is_multiple_of(3) as u8);
    wr8(b, A5 + 0x35, (rng.next() % 5) as u8);
    wr8(b, A5 + 0x42, rng.next().is_multiple_of(4) as u8);
    wr8(b, A5 + 0x48, (rng.next() % 9) as u8);
    wr8(b, A5 + 0x2c, if rng.next().is_multiple_of(4) { 0xc } else { 0 });
    if rng.next().is_multiple_of(2) {
        wr16(b, obj + 0x46, 0x85de);
    }
    let cam = ox.wrapping_sub(0x80 + (rng.next() % 0x100) as u16);
    wr16(b, A5 + 0x5a6, cam);
    wr16(b, A5 + 0x5ae, cam);
    wr16(b, A5 + 0x5aa, oy.wrapping_sub((rng.next() % 0x100) as u16));
}

fn kobold(b: &mut Bus, rng: &mut Rng, obj: u32, fresh: bool) {
    if fresh {
        wr8(b, obj + 3, (rng.next() % 14) as u8);
        wr8(b, obj + 0x2f, (rng.next() % 4) as u8);
    }
    // state to resume after a U-turn: (`+0xa6`, `+0xa5`) consistent
    let resumes: [(u32, u8); 8] = [
        (0x0202_0000, 0),
        (0x0202_0000, 4),
        (0x0204_0000, 0),
        (0x0204_0000, 8),
        (0x0206_0000, 4),
        (0x0206_0000, 8),
        (0x0208_0004, 0),
        (0x0208_0004, 2),
    ];
    let (state, sub) = resumes[rng.next() as usize % resumes.len()];
    wr32(b, obj + 0xa6, state);
    wr8(b, obj + 0xa5, sub);
    for off in [0x6bu32, 0x6e, 0x71, 0x77] {
        wr8(b, obj + off, (rng.next() % 4) as u8);
    }
    wr16(b, obj + 0xa0, [0u16, 1, 2, 5, 0x14][rng.next() as usize % 5]);
    wr8(b, obj + 0xa2, [0u8, 1, 2, 3, 4, 0xc, 0x7f][rng.next() as usize % 7]);
    // previous look: often the one the gesture will resume (the `+0xad` countdown happens only if they are equal)
    let resume = [0u8, 1, 2, 3, 4, 0xc, 0x7f, 0x16, 0x1a, 0x1c, 0x1e, 0x28, 0x29, 0x2a, 0x2b, 0x2c, 0x2d, 0x2e, 0x2f];
    wr8(b, obj + 0xa3, resume[rng.next() as usize % resume.len()]);
    wr8(b, obj + 0xaa, 1 + (rng.next() % 3) as u8);
    wr8(b, obj + 0xac, 1 + (rng.next() % 3) as u8);
    wr8(b, obj + 0xad, 1 + (rng.next() % 3) as u8);
    wr8(b, obj + 0x33, [0u8, 4, 1, 0x80][rng.next() as usize % 4]);
    wr8(b, obj + 0x32, rng.next() as u8);
    wr16(b, obj + 0x42, (rng.next() % 4) as u16);
    wr8(b, obj + 0x44, (rng.next() % 4) as u8);
    wr16(b, obj + 0x84, (rng.next() % 3) as u16);
    wr8(b, obj + 0x86, rng.next().is_multiple_of(4) as u8);
    wr8(b, obj + 0x53, [0u8, 0, 0, 1, 0xff][rng.next() as usize % 5]);
    wr8(b, obj + 0x56, (rng.next() % 4) as u8);
    wr8(b, obj + 0x29, (rng.next() % 2) as u8);
    for off in [0x72u32, 0x73] {
        wr8(b, obj + off, (rng.next() % 5) as u8);
    }
    if rng.next().is_multiple_of(8) {
        wr16(b, obj + 0x50, 0);
    }
    let (ox, oy) = (rd16(b, obj + 8), rd16(b, obj + 0x10));
    if rng.next().is_multiple_of(2) {
        let offsets = [-0xb0i16, -0x40, -0x30, -0x28, 0, 0x20, 0x40, 0x80, 0x100];
        for j in 0..4u32 {
            let q = 0xff85de + 0x200 * j;
            let off = offsets[rng.next() as usize % offsets.len()] + (rng.next() % 0x12) as i16 - 9;
            wr8(b, q, !rng.next().is_multiple_of(5) as u8);
            wr16(b, q + 8, ox.wrapping_add(off as u16));
            wr16(b, q + 0x10, oy.wrapping_add((rng.next() % 0x14) as u16).wrapping_sub(8));
            wr8(b, q + 0x2c, 0x20 * (rng.next() % 2) as u8);
        }
    }
    wr8(b, A5 + 0x34, [4u8, 6, 0xc, 1, 2][rng.next() as usize % 5]);
    wr8(b, A5 + 0x35, (rng.next() % 5) as u8);
    wr8(b, A5 + 0x42, rng.next().is_multiple_of(4) as u8);
    if rng.next().is_multiple_of(2) {
        wr16(b, obj + 0x46, 0);
    }
    wr16(b, A5 + 0x6e, rng.next() as u16);
}

fn goblin(b: &mut Bus, rng: &mut Rng, obj: u32, _fresh: bool) {
    // even variant: 0 walk, 2 and 4 fall, 6 run; in arrival, +6 = 6 exists only for variants 2 and 4
    // and variant 6 has only three steps
    let (s5, s6) = (rd8(b, obj + 5), rd8(b, obj + 6));
    let variant = if s5 == 0 && s6 == 6 { [2u8, 4][rng.next() as usize % 2] } else { 2 * (rng.next() % 4) as u8 };
    wr8(b, obj + 3, variant);
    for off in [0x6bu32, 0x6e, 0x71] {
        wr8(b, obj + off, (rng.next() % 4) as u8);
    }
    wr8(b, obj + 0x77, (rng.next() % 3) as u8);
    wr8(b, obj + 0xa2, [0u8, 1, 2, 5, 0x14][rng.next() as usize % 5]);
    wr8(b, obj + 0xa4, (rng.next() % 3) as u8);
    wr8(b, obj + 0xa1, [0u8, 1, 0x80][rng.next() as usize % 3]);
    // remembered gesture: a valid `+6` of sub-state 4
    // (sub-state 6 has only seven gestures: the original would jump into data beyond)
    let gestures = if s5 == 6 { 7 } else { 11 };
    wr8(b, obj + 0xaa, 2 * (rng.next() % gestures) as u8);
    wr8(b, obj + 0xae, rng.next().is_multiple_of(6) as u8);
    wr8(b, obj + 0x33, [0u8, 4, 1, 0x80][rng.next() as usize % 4]);
    wr8(b, obj + 0x32, rng.next() as u8);
    wr8(b, obj + 0x3b, rng.next() as u8);
    wr16(b, obj + 0x84, (rng.next() % 3) as u16);
    wr8(b, obj + 0x86, rng.next().is_multiple_of(4) as u8);
    wr8(b, obj + 0x53, [0u8, 0, 0, 1, 0xff][rng.next() as usize % 5]);
    wr8(b, obj + 0x29, (rng.next() % 2) as u8);
    wr8(b, obj + 0x67, (rng.next() % 2) as u8);
    wr8(b, obj + 0x79, 0x1a);
    // AI parameters read by 0xa4b4 and 0xa4c2
    for off in [0x72u32, 0x73] {
        wr8(b, obj + off, (rng.next() % 5) as u8);
    }
    if rng.next().is_multiple_of(8) {
        wr16(b, obj + 0x50, 0);
    }
    let (ox, oy) = (rd16(b, obj + 8), rd16(b, obj + 0x10));
    if rng.next().is_multiple_of(2) {
        let offsets = [-0xd0i16, -0xa0, -0x38, -0x20, 0, 0x20, 0x38, 0x70, 0x100];
        for j in 0..4u32 {
            let q = 0xff85de + 0x200 * j;
            let off = offsets[rng.next() as usize % offsets.len()] + (rng.next() % 0x12) as i16 - 9;
            wr8(b, q, !rng.next().is_multiple_of(5) as u8);
            wr16(b, q + 8, ox.wrapping_add(off as u16));
            wr16(b, q + 0x10, oy.wrapping_add((rng.next() % 0x30) as u16).wrapping_sub(0x18));
            wr8(b, q + 0x2c, 0x20 * (rng.next() % 2) as u8);
            // player on the ground or in grab position: 0x7f0c tests its state
            wr8(b, q + 6, [0u8, 0xc, 0xc, 0xe][rng.next() as usize % 4]);
            wr16(b, q + 0xc, 0);
        }
    }
    wr8(b, A5 + 0x30, rng.next().is_multiple_of(3) as u8);
    wr8(b, A5 + 0x35, (rng.next() % 5) as u8);
    wr8(b, A5 + 0x42, rng.next().is_multiple_of(4) as u8);
    if rng.next().is_multiple_of(2) {
        wr16(b, obj + 0x46, 0);
    }
    let cam = ox.wrapping_sub(0x80 + (rng.next() % 0x100) as u16);
    wr16(b, A5 + 0x506, cam);
    wr16(b, A5 + 0x556, cam);
    wr16(b, A5 + 0x5a6, cam);
    wr16(b, A5 + 0x5ae, cam);
    wr16(b, A5 + 0x7f92, (rng.next() % 32) as u16);
}

fn troll(b: &mut Bus, rng: &mut Rng, obj: u32, _fresh: bool) {
    // even variant: 0 walk, 2 and 4 fall; in arrival, +6 = 6 exists only for variants 2 and 4
    let (s5, s6) = (rd8(b, obj + 5), rd8(b, obj + 6));
    let variant = if s5 == 0 && s6 == 6 { [2u8, 4][rng.next() as usize % 2] } else { 2 * (rng.next() % 3) as u8 };
    wr8(b, obj + 3, variant);
    for off in [0x6bu32, 0x6e, 0x71] {
        wr8(b, obj + off, (rng.next() % 4) as u8);
    }
    wr8(b, obj + 0x77, (rng.next() % 3) as u8);
    wr8(b, obj + 0xa2, [0u8, 1, 2, 5, 0x14][rng.next() as usize % 5]);
    wr8(b, obj + 0xa4, (rng.next() % 3) as u8);
    wr8(b, obj + 0xa1, [0u8, 1, 0x80, 4, 8, 0x12][rng.next() as usize % 6]);
    // remembered gesture: a valid `+6` of sub-state 4
    let gestures = if s5 == 6 { 7 } else { 11 };
    wr8(b, obj + 0xaa, 2 * (rng.next() % gestures) as u8);
    wr8(b, obj + 0xae, rng.next().is_multiple_of(6) as u8);
    wr8(b, obj + 0xab, [0u8, 0x10, 0x20, 0xf0][rng.next() as usize % 4]);
    wr8(b, obj + 0xac, [0u8, 0, 1, 0xb4][rng.next() as usize % 4]);
    // thrown object: an existing player
    let launches = 0x85de + 0x200 * (rng.next() % 4);
    wr32(b, obj + 0xa6, 0xff0000 | launches);
    wr8(b, obj + 0x33, [0u8, 4, 1, 0x80][rng.next() as usize % 4]);
    wr8(b, obj + 0x32, rng.next() as u8);
    wr8(b, obj + 0x3b, rng.next() as u8);
    wr16(b, obj + 0x84, (rng.next() % 3) as u16);
    wr8(b, obj + 0x86, rng.next().is_multiple_of(4) as u8);
    wr8(b, obj + 0x53, [0u8, 0, 0, 1, 0xff][rng.next() as usize % 5]);
    wr8(b, obj + 0x29, (rng.next() % 2) as u8);
    wr8(b, obj + 0x67, (rng.next() % 2) as u8);
    wr8(b, obj + 0x79, 0x1a);
    for off in [0x72u32, 0x73] {
        wr8(b, obj + off, (rng.next() % 5) as u8);
    }
    if rng.next().is_multiple_of(8) {
        wr16(b, obj + 0x50, 0);
    }
    let (ox, oy) = (rd16(b, obj + 8), rd16(b, obj + 0x10));
    let nobody = rng.next().is_multiple_of(8);
    if rng.next().is_multiple_of(2) {
        let offsets = [-0xd0i16, -0xa0, -0x38, -0x20, 0, 0x20, 0x38, 0x70, 0x100];
        for j in 0..4u32 {
            let q = 0xff85de + 0x200 * j;
            let off = offsets[rng.next() as usize % offsets.len()] + (rng.next() % 0x12) as i16 - 9;
            wr8(b, q, (!nobody && !rng.next().is_multiple_of(5)) as u8);
            wr16(b, q + 8, ox.wrapping_add(off as u16));
            wr16(b, q + 0x10, oy.wrapping_add((rng.next() % 0x30) as u16).wrapping_sub(0x18));
            wr8(b, q + 0x2c, 0x20 * (rng.next() % 2) as u8);
            wr8(b, q + 6, [0u8, 0xc, 0xc, 0xe][rng.next() as usize % 4]);
            wr16(b, q + 0xc, 0);
            // the thrown object: state +4 from 0 to 5
            wr8(b, q + 4, (rng.next() % 6) as u8);
        }
    }
    wr8(b, A5 + 0x30, rng.next().is_multiple_of(3) as u8);
    wr8(b, A5 + 0x35, (rng.next() % 5) as u8);
    wr8(b, A5 + 0x42, rng.next().is_multiple_of(4) as u8);
    if rng.next().is_multiple_of(2) {
        wr16(b, obj + 0x46, 0);
    }
    let cam = ox.wrapping_sub(0x80 + (rng.next() % 0x100) as u16);
    for off in [0x506u32, 0x556, 0x50e, 0x55e, 0x5a6, 0x5ae] {
        wr16(b, A5 + off, cam);
    }
    wr16(b, A5 + 0x7f92, (rng.next() % 32) as u16);
}

fn gnoll(b: &mut Bus, rng: &mut Rng, obj: u32, _fresh: bool) {
    // even variant: 0 walk, 2 and 6 fall, 4 side jump, 8 and 10 armed arrival; in arrival, the large +6
    // exist only for some variants (6: all but 10, 8: 0 and 4); standing, variant 10 is a grab
    let (s4, s5, s6) = (rd8(b, obj + 4), rd8(b, obj + 5), rd8(b, obj + 6));
    let variant = match (s5, s6) {
        (0, 8) => [0u8, 4][rng.next() as usize % 2],
        (0, 6) => [0u8, 2, 4, 6, 8][rng.next() as usize % 5],
        (0, _) => 2 * (rng.next() % 6) as u8,
        _ if s4 == 2 && s5 == 2 && !rng.next().is_multiple_of(6) => 2 * (rng.next() % 5) as u8,
        _ => 2 * (rng.next() % 6) as u8,
    };
    wr8(b, obj + 3, variant);
    for off in [0x6bu32, 0x6e, 0x71] {
        wr8(b, obj + off, (rng.next() % 4) as u8);
    }
    wr8(b, obj + 0x77, (rng.next() % 3) as u8);
    wr8(b, obj + 0xa2, [0u8, 1, 2, 5, 0x14][rng.next() as usize % 5]);
    wr8(b, obj + 0xa4, (rng.next() % 3) as u8);
    wr8(b, obj + 0xa1, [0u8, 1, 0x80, 4, 6, 8, 0x12][rng.next() as usize % 7]);
    // remembered gesture: a valid `+6` of sub-state 4 (and 6)
    let gestures = if s5 == 6 { 7 } else { 13 };
    wr8(b, obj + 0xa5, 2 * (rng.next() % gestures) as u8);
    wr8(b, obj + 0xa6, (rng.next() % 2) as u8);
    wr8(b, obj + 0xbf, (rng.next() % 2) as u8);
    wr8(b, obj + 0x33, [0u8, 1, 4, 9, 0xb, 0x80][rng.next() as usize % 6]);
    wr8(b, obj + 0x32, rng.next() as u8);
    wr8(b, obj + 0x3b, rng.next() as u8);
    wr16(b, obj + 0x84, (rng.next() % 3) as u16);
    wr8(b, obj + 0x86, rng.next().is_multiple_of(4) as u8);
    wr8(b, obj + 0x53, [0u8, 0, 0, 1, 0xff][rng.next() as usize % 5]);
    wr8(b, obj + 0x29, (rng.next() % 2) as u8);
    wr8(b, obj + 0x67, (rng.next() % 2) as u8);
    wr8(b, obj + 0x79, 0x1a);
    for off in [0x72u32, 0x73] {
        wr8(b, obj + off, (rng.next() % 5) as u8);
    }
    if rng.next().is_multiple_of(8) {
        wr16(b, obj + 0x50, 0);
    }
    let (ox, oy) = (rd16(b, obj + 8), rd16(b, obj + 0x10));
    if rng.next().is_multiple_of(2) {
        let offsets = [-0xd0i16, -0x80, -0x38, -0x20, 0, 0x20, 0x30, 0x70, 0x100];
        for j in 0..4u32 {
            let q = 0xff85de + 0x200 * j;
            let off = offsets[rng.next() as usize % offsets.len()] + (rng.next() % 0x12) as i16 - 9;
            wr8(b, q, !rng.next().is_multiple_of(5) as u8);
            wr16(b, q + 8, ox.wrapping_add(off as u16));
            wr16(b, q + 0x10, oy.wrapping_add((rng.next() % 0x30) as u16).wrapping_sub(0x18));
            wr8(b, q + 0x2c, 0x20 * (rng.next() % 2) as u8);
            wr8(b, q + 6, [0u8, 0xc, 0xc, 0xe][rng.next() as usize % 4]);
            wr16(b, q + 0xc, 0);
        }
    }
    wr8(b, A5 + 0x30, rng.next().is_multiple_of(3) as u8);
    wr8(b, A5 + 0x35, (rng.next() % 5) as u8);
    wr8(b, A5 + 0x42, rng.next().is_multiple_of(4) as u8);
    if rng.next().is_multiple_of(2) {
        wr16(b, obj + 0x46, 0);
    }
    let cam = ox.wrapping_sub(0x80 + (rng.next() % 0x100) as u16);
    for off in [0x506u32, 0x556, 0x5a6, 0x5ae] {
        wr16(b, A5 + off, cam);
    }
    wr16(b, A5 + 0x7f92, (rng.next() % 32) as u16);
}

fn archer(b: &mut Bus, rng: &mut Rng, obj: u32, _fresh: bool) {
    let (s4, s5, s6) = (rd8(b, obj + 4), rd8(b, obj + 5), rd8(b, obj + 6));
    // even variant: 0 and 2 walk, 4 fall, 6 appearance, 8 run
    wr8(b, obj + 3, 2 * (rng.next() % 5) as u8);
    // created by another object: only the arrival and in-place shot states exist
    let created = s4 == 2 && s5 <= 2 && s6 <= 4 && rng.next().is_multiple_of(3);
    wr8(b, obj + 0x42, created as u8);
    if created {
        wr8(b, obj + 7, 2 * (rng.next() % 3) as u8);
    }
    wr8(b, obj + 0x43, [0u8, 0, 1, 0x80][rng.next() as usize % 4]);
    wr16(b, obj + 0xa8, [0u16, 1, 2, 0x80, 0xffff][rng.next() as usize % 5]);
    wr8(b, obj + 0xa7, rng.next().is_multiple_of(8) as u8);
    wr8(b, obj + 0xac, rng.next().is_multiple_of(8) as u8);
    wr8(b, obj + 0xa4, [0u8, 1, 2, 5, 0x20][rng.next() as usize % 5]);
    wr8(b, obj + 0xa5, 1 + (rng.next() % 3) as u8);
    // step to resume after a side walk (2 or 6 in the game)
    wr8(b, obj + 0xa6, [2u8, 6][rng.next() as usize % 2]);
    // state to resume after a U-turn
    let valid = [0x0202_0000u32, 0x0202_0200, 0x0204_0000, 0x0204_0202, 0x0208_0000, 0x020a_0000];
    wr32(b, obj + 0xa0, valid[rng.next() as usize % valid.len()]);
    for off in [0x6bu32, 0x6e, 0x71] {
        wr8(b, obj + off, (rng.next() % 4) as u8);
    }
    wr8(b, obj + 0x2f, (rng.next() % 2) as u8);
    wr8(b, obj + 0x33, [0u8, 1, 4, 0x80][rng.next() as usize % 4]);
    wr8(b, obj + 0x32, rng.next() as u8);
    wr16(b, obj + 0x84, (rng.next() % 3) as u16);
    wr8(b, obj + 0x86, rng.next().is_multiple_of(4) as u8);
    wr8(b, obj + 0x53, [0u8, 0, 0, 1, 0xff][rng.next() as usize % 5]);
    wr8(b, obj + 0x79, 3);
    if rng.next().is_multiple_of(6) {
        wr16(b, obj + 0x50, 0);
    }
    // possible creator (bit 7 of `+0x43` makes it act on it when leaving the screen)
    let host = 0x85de + 0x200 * (rng.next() % 4) as u16;
    wr16(b, obj + 0x46, if rng.next().is_multiple_of(3) { 0 } else { host });
    let (ox, oy) = (rd16(b, obj + 8), rd16(b, obj + 0x10));
    let nobody = rng.next().is_multiple_of(8);
    if rng.next().is_multiple_of(2) {
        let offsets = [-0xd0i16, -0x80, -0x38, -0x28, 0, 0x20, 0x38, 0x60, 0x100];
        for j in 0..4u32 {
            let q = 0xff85de + 0x200 * j;
            let off = offsets[rng.next() as usize % offsets.len()] + (rng.next() % 0x12) as i16 - 9;
            wr8(b, q, (!nobody && !rng.next().is_multiple_of(5)) as u8);
            wr16(b, q + 8, ox.wrapping_add(off as u16));
            wr16(b, q + 0x10, oy.wrapping_add((rng.next() % 0x30) as u16).wrapping_sub(0x18));
            wr8(b, q + 0x2c, 0x20 * (rng.next() % 2) as u8);
            wr8(b, q + 6, [0u8, 0xc, 0xc, 0xe][rng.next() as usize % 4]);
            wr16(b, q + 0xc, 0);
        }
    }
    wr8(b, A5 + 0x34, [4u8, 6, 0xc, 1][rng.next() as usize % 4]);
    wr8(b, A5 + 0x35, (rng.next() % 5) as u8);
    wr8(b, A5 + 0x42, rng.next().is_multiple_of(4) as u8);
    let cam = ox.wrapping_sub(0x80 + (rng.next() % 0x100) as u16);
    for off in [0x506u32, 0x556, 0x50e, 0x55e, 0x5a6, 0x5ae] {
        wr16(b, A5 + off, cam);
    }
    wr16(b, A5 + 0x7f92, (rng.next() % 32) as u16);
}

fn golem(b: &mut Bus, rng: &mut Rng, obj: u32, _fresh: bool) {
    wr8(b, obj + 0x2f, (rng.next() % 2) as u8);
    wr8(b, obj + 0x77, (rng.next() % 4) as u8);
    // received-hit flag: almost always zero
    wr8(b, obj + 0x8f, [0u8, 0, 0, 0, 1, 2, 3][rng.next() as usize % 7]);
    wr8(b, obj + 0xa6, [0u8, 1, 2, 3][rng.next() as usize % 4]);
    wr16(b, obj + 0xa2, [0u16, 1, 2, 5, 0x10][rng.next() as usize % 5]);
    wr16(b, obj + 0xa4, rng.next() as u16);
    wr8(b, obj + 0xa7, (rng.next() % 2) as u8);
    wr8(b, obj + 0x29, (rng.next() % 2) as u8);
    wr8(b, obj + 0x33, [0u8, 1, 4, 0x80][rng.next() as usize % 4]);
    wr16(b, obj + 0x30, [1u16, 2, 5][rng.next() as usize % 3]);
    wr16(b, obj + 0x32, [0u16, 0, 1, 0xffff][rng.next() as usize % 4]);
    wr16(b, obj + 0x84, (rng.next() % 3) as u16);
    wr8(b, obj + 0x86, rng.next().is_multiple_of(4) as u8);
    wr8(b, obj + 0x53, [0u8, 0, 0, 1, 0xff][rng.next() as usize % 5]);
    for off in [0x6bu32, 0x6e, 0x71] {
        wr8(b, obj + off, (rng.next() % 4) as u8);
    }
    for off in [0x72u32, 0x73] {
        wr8(b, obj + off, (rng.next() % 5) as u8);
    }
    if rng.next().is_multiple_of(8) {
        wr16(b, obj + 0x50, 0);
    }
    // the object that hit it (+0x54): an existing player, whose byte +0x57 gives the freeze
    let key = 0x85de + 0x200 * (rng.next() % 4) as u16;
    wr16(b, obj + 0x54, key);
    wr8(b, 0xff0000 | (key as u32 + 0x57), [0u8, 4, 8, 0x20][rng.next() as usize % 4]);
    let (ox, oy) = (rd16(b, obj + 8), rd16(b, obj + 0x10));
    if rng.next().is_multiple_of(2) {
        let offsets = [-0xd0i16, -0xa0, -0x68, -0x20, -8, 0, 0x30, 0x68, 0x100];
        for j in 0..4u32 {
            let q = 0xff85de + 0x200 * j;
            let off = offsets[rng.next() as usize % offsets.len()] + (rng.next() % 0x12) as i16 - 9;
            wr8(b, q, !rng.next().is_multiple_of(5) as u8);
            wr16(b, q + 8, ox.wrapping_add(off as u16));
            wr16(b, q + 0x10, oy.wrapping_add((rng.next() % 0x20) as u16).wrapping_sub(0x10));
            wr8(b, q + 0x2c, 0x20 * (rng.next() % 2) as u8);
            wr16(b, q + 0xc, 0);
        }
    }
    // keys of the fourth player
    wr8(b, A5 + 3, rng.next() as u8);
    wr8(b, A5 + 0xf, rng.next() as u8);
    wr8(b, A5 + 0x35, (rng.next() % 5) as u8);
    wr8(b, A5 + 0x36, rng.next().is_multiple_of(4) as u8);
    let cam = ox.wrapping_sub(0x80 + (rng.next() % 0x180) as u16);
    for off in [0x506u32, 0x556, 0x5a6, 0x5ae] {
        wr16(b, A5 + off, cam);
    }
    wr16(b, A5 + 0x50a, oy);
    wr16(b, A5 + 0x55a, oy);
    wr16(b, A5 + 0x7f92, (rng.next() % 32) as u16);
}

fn machine(b: &mut Bus, rng: &mut Rng, obj: u32, fresh: bool) {
    if fresh {
        wr8(b, obj + 3, (rng.next() % 3) as u8);
    }
    wr8(b, obj + 0x2f, !rng.next().is_multiple_of(3) as u8);
    wr8(b, obj, 1);
    wr8(b, obj + 0xaa, (rng.next() % 3) as u8);
    wr8(b, obj + 0xab, 2);
    wr8(b, obj + 0xac, 2 * (rng.next() % 16) as u8);
    for off in [0x6bu32, 0x6e, 0x71] {
        wr8(b, obj + off, (rng.next() % 4) as u8);
    }
    wr8(b, obj + 0x6d, (rng.next() % 9) as u8);
    // current and previous look (values taken by the machine)
    let aspects = [0u8, 1, 2, 3, 4, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x20, 0x30, 0x31, 0x32, 0x40, 0xff];
    wr8(b, obj + 0xa0, aspects[rng.next() as usize % aspects.len()]);
    wr8(b, obj + 0xa1, aspects[rng.next() as usize % aspects.len()]);
    // state to resume after a U-turn
    let valid = [0x0202_0000u32, 0x0202_0400, 0x0202_0600, 0x0202_0800, 0x0204_0000, 0x020a_0000];
    wr32(b, obj + 0xa2, valid[rng.next() as usize % valid.len()]);
    for off in [0xa6u32, 0xa8] {
        wr16(b, obj + off, [0u16, 1, 2, 3, 0x14, 0x1e][rng.next() as usize % 6]);
    }
    wr8(b, obj + 0xad, rng.next().is_multiple_of(4) as u8);
    wr8(b, obj + 0xae, [0u8, 1, 2, 5][rng.next() as usize % 4]);
    wr8(b, obj + 0xb2, (rng.next() % 2) as u8);
    wr8(b, obj + 0x33, [0u8, 0, 1, 2, 0x80][rng.next() as usize % 5]);
    wr8(b, obj + 0x32, rng.next() as u8);
    wr16(b, obj + 0x62, [0u16, 5, 30][rng.next() as usize % 3]);
    wr16(b, obj + 0x84, (rng.next() % 3) as u16);
    wr8(b, obj + 0x86, rng.next().is_multiple_of(4) as u8);
    wr8(b, obj + 0x53, [0u8, 0, 0, 1, 0xff][rng.next() as usize % 5]);
    wr16(b, obj + 0x14, [0u16, 0x200, 0xfe00, 0x300, 0xfd00][rng.next() as usize % 5]);
    wr16(b, obj + 0x16, [0u16, 0x20, 0xffe0][rng.next() as usize % 3]);
    wr16(b, obj + 0x18, [0u16, 0x400, 0xff00, 0x80, 0x500][rng.next() as usize % 5]);
    wr16(b, obj + 0x1a, 0xffc0);
    wr8(b, obj + 0x67, (rng.next() % 2) as u8);
    for off in [0x72u32, 0x73] {
        wr8(b, obj + off, (rng.next() % 3) as u8);
    }
    if rng.next().is_multiple_of(8) {
        wr16(b, obj + 0x50, 0);
    }
    let (ox, oy) = (rd16(b, obj + 8), rd16(b, obj + 0x10));
    if rng.next().is_multiple_of(2) {
        let offsets = [-0xe0i16, -0x80, -0x50, -0x30, -0x10, 0, 0x30, 0x60, 0x100];
        for j in 0..4u32 {
            let q = 0xff85de + 0x200 * j;
            let off = offsets[rng.next() as usize % offsets.len()] + (rng.next() % 0x12) as i16 - 9;
            wr8(b, q, !rng.next().is_multiple_of(5) as u8);
            wr16(b, q + 8, ox.wrapping_add(off as u16));
            wr16(b, q + 0x10, oy.wrapping_add((rng.next() % 0x30) as u16).wrapping_sub(0x18));
            wr8(b, q + 0x2c, 0x20 * (rng.next() % 2) as u8);
            wr8(b, q + 6, [0u8, 0xc, 0xc, 0xe][rng.next() as usize % 4]);
            wr16(b, q + 0xc, 0);
        }
    }
    wr8(b, A5 + 0x30, rng.next().is_multiple_of(3) as u8);
    // grab: `A5+0x42` and the link `+0x46` (2810)
    wr8(b, A5 + 0x42, rng.next().is_multiple_of(8) as u8);
    wr16(b, obj + 0x46, if rng.next().is_multiple_of(4) { 0x85de } else { 0 });
    wr8(b, A5 + 0x35, (rng.next() % 5) as u8);
    wr8(b, A5 + 0x36, rng.next().is_multiple_of(4) as u8);
    wr8(b, A5 + 0x48, (rng.next() % 9) as u8);
    let cam = if rng.next().is_multiple_of(4) {
        ox.wrapping_add((rng.next() % 0x100) as u16)
    } else {
        ox.wrapping_sub(0x80 + (rng.next() % 0x180) as u16)
    };
    for off in [0x5a6u32, 0x5ae, 0x506, 0x556] {
        wr16(b, A5 + off, cam);
    }
    wr16(b, A5 + 0x7f92, (rng.next() % 32) as u16);
    wr8(b, A5 + 0x7f93, (rng.next() % 32) as u8);
}
