//! Tables of the bosses, added to `data/enemies.json` by [`crate::enemies::specs`] and read by the game through
//! `families/enemy_common/tables.rs` (docs/fin.md, L9). Same recipe as the enemy families (`enemies::families`): one
//! line per table, named `<boss>/<table>`; a table whose end the code does not give runs up to the next table or code
//! (`upto`); a table of addresses of tables declares the tables it points to (`pointed`).
//!
//! The decisions of the bosses draw a byte in a table of 16 bytes (`<boss>/<...>_picks`), then jump through a table of
//! words relative to its start (`<boss>/<...>_jumps`): the address reached is the key of the state stub in the game
//! (`boss_common::decide`). A jump table holds as many entries as the largest byte of its pick tables designates.

use crate::data::{data, prog, Spec};
use crate::rom::Program;

use crate::enemies::Bounded;

/// A row of 32 bytes per pair of difficulty levels (`rank / 2`), drawn with a 5-bit random number.
const BY_RANK: (&str, usize) = ("u8*32", 16);
/// One byte per difficulty level (`rank`, 0 to 31).
const PER_RANK: usize = 32;

/// Tables of the bosses and of `boss_common`.
pub(crate) fn specs(rom: &Program, s: &mut Vec<Spec>, bounded: &mut Bounded) {
    let rank = |s: &mut Vec<Spec>, name: &str, at: u32| s.push(data(name, at, BY_RANK.0, &[], BY_RANK.1));
    let mut upto = |s: &mut Vec<Spec>, name: &str, at: u32, program: bool, format: &'static str, max: u32| {
        bounded.push((s.len(), max));
        s.push(if program { prog(name, at, format, &[], 0) } else { data(name, at, format, &[], 0) });
    };
    // ---- boss_common ----
    upto(s, "boss/bob_offsets", 0x833e4, true, "i32", 0x80);
    // ---- boss 0 ----
    decision(rom, s, "boss_0/close", &[0x7a8e8], 0x7a8f8);
    decision(rom, s, "boss_0/front", &[0x7a932], 0x7a942);
    decision(rom, s, "boss_0/near", &[0x7a9ae, 0x7a9be, 0x7a9ce], 0x7a9de);
    decision(rom, s, "boss_0/far", &[0x7aa88, 0x7aa98, 0x7aaa8, 0x7aab8], 0x7aac8);
    s.push(prog("boss_0/walk_delays", 0x7a7bc, "u8", &[], PER_RANK));
    s.push(prog("boss_0/walk_delays_back", 0x7a7dc, "u8", &[], PER_RANK));
    s.push(prog("boss_0/walk_steps", 0x7a820, "u8", &[], 32));
    s.push(prog("boss_0/walk_steps_back", 0x7a840, "u8", &[], 32));
    s.push(prog("boss_0/combo_counts", 0x7a1ca, "u8*4", &[], 16));
    // ---- boss 1 ----
    decision(rom, s, "boss_1/wide", &[0x7c0d0], 0x7c0e0);
    decision(rom, s, "boss_1/close", &[0x7c11a, 0x7c12a, 0x7c13a], 0x7c14a);
    decision(rom, s, "boss_1/near", &[0x7c1a8, 0x7c1b8], 0x7c1c8);
    decision(rom, s, "boss_1/far", &[0x7c226, 0x7c236], 0x7c246);
    s.push(prog("boss_1/combo_counts", 0x7c49a, "u8*4", &[], 16));
    upto(s, "boss_1/split_leap_masks", 0x7c534, true, "x16", 0x40);
    s.push(prog("boss_1/walk_steps", 0x7c040, "u8", &[], 32));
    s.push(prog("boss_1/walk_steps_second", 0x7c060, "u8", &[], 32));
    s.push(prog("boss_1/hop_marks", 0x7c400, "i16*2", &[], 4));
    s.push(prog("boss_1/leap_marks", 0x7c410, "i16*2", &[], 8));
    s.push(prog("boss_1/crossing_speeds", 0x7af94, "i16*2", &[], 8));
    upto(s, "boss_1/crossing_anims", 0x7afb4, true, "u16", 0x20);
    upto(s, "boss_1/bite_delays", 0x7bb3e, true, "u16", 0x10);
    // ---- boss 3 ----
    s.push(prog("boss_3/round_points", 0x81876, "i16*2 u8*2", &[], 4));
    upto(s, "boss_3/claw_steps", 0x7fdea, true, "i8", 0x40);
    upto(s, "boss_3/second_claw_steps", 0x7fdbc, true, "i8", 0x40);
    s.push(prog("boss_3/counter_steps", 0x80cf6, "u8", &[], 16));
    upto(s, "boss_3/wounds", 0x81712, true, "u8", 0x10);
    rank(s, "boss_3/near_attacks", 0x80e62);
    rank(s, "boss_3/far_attacks", 0x81062);
    rank(s, "boss_3/wounded_attacks", 0x81262);
    rank(s, "boss_3/follow_ups", 0x814f8);
    s.push(prog("boss_3/counter_masks", 0x81b1a, "x16*2", &[], 16));
    upto(s, "boss_3/front_slices", 0x81950, true, "u8", 0x20);
    upto(s, "boss_3/front_rows", 0x81970, false, "i16*4", 0x80);
    upto(s, "boss_3/side_slices", 0x81a44, true, "u8", 0x20);
    upto(s, "boss_3/side_rows", 0x81a64, false, "i16*4", 0x80);
    // ---- boss 4 ----
    decision(rom, s, "boss_4/wide", &[0x830e4], 0x830f4);
    decision(rom, s, "boss_4/close", &[0x8312e, 0x8313e, 0x8314e], 0x8315e);
    decision(rom, s, "boss_4/near", &[0x831be, 0x831ce, 0x831de], 0x831ee);
    decision(rom, s, "boss_4/far", &[0x83250, 0x83260, 0x83270], 0x83280);
    upto(s, "boss_4/rise_steps", 0x82852, true, "i32", 0x80);
    upto(s, "boss_4/breath_angles", 0x8295c, true, "u8", 0x20);
    upto(s, "boss_4/air_breath_angles", 0x82ba2, true, "u8", 0x20);
    s.push(prog("boss_4/walk_steps", 0x83054, "u8", &[], 32));
    s.push(prog("boss_4/walk_steps_slow", 0x83074, "u8", &[], 32));
    s.push(prog("boss_4/walk_delays", 0x82ff0, "u8", &[], PER_RANK));
    s.push(prog("boss_4/walk_delays_slow", 0x83010, "u8", &[], PER_RANK));
    // ---- boss 5 ----
    upto(s, "boss_5/stroll_times", 0x8362e, true, "u16", 0x20);
    upto(s, "boss_5/dash_speeds", 0x8369c, true, "u16", 0x20);
    upto(s, "boss_5/dash_speeds_b", 0x836fa, true, "u16", 0x20);
    s.push(prog("boss_5/next_actions", 0x83f4e, "u8*2", &[], 0x1e0 / 2));
    // ---- boss 6 ----
    decision(rom, s, "boss_6/close", &[0x85708], 0x85718);
    decision(rom, s, "boss_6/front", &[0x85752, 0x85762, 0x85772], 0x85782);
    decision(rom, s, "boss_6/mid", &[0x857e2], 0x857f2);
    decision(rom, s, "boss_6/far", &[0x85850, 0x85860, 0x85870], 0x85880);
    upto(s, "boss_6/low_flight", 0x8508c, true, "i32", 0x80);
    upto(s, "boss_6/skim", 0x8599a, true, "i32", 0x80);
    s.push(prog("boss_6/walk_delays", 0x85614, "u8", &[], PER_RANK));
    s.push(prog("boss_6/walk_delays_slow", 0x85634, "u8", &[], PER_RANK));
    s.push(prog("boss_6/walk_steps", 0x85678, "u8", &[], 32));
    s.push(prog("boss_6/walk_steps_slow", 0x85698, "u8", &[], 32));
    // ---- boss 7 ----
    s.push(prog("boss_7/sidesteps", 0x87c8a, "i16", &[], 8));
    pointed(rom, s, "boss_7/dodge_spots", 0x87f30, 4, true, ("i16*2 u8*4", 4));
    rank(s, "boss_7/front_attacks", 0x87126);
    rank(s, "boss_7/front_attacks_slow", 0x87526);
    rank(s, "boss_7/back_attacks", 0x87326);
    rank(s, "boss_7/back_attacks_slow", 0x87726);
    rank(s, "boss_7/follow_ups", 0x87986);
    s.push(prog("boss_7/counter_masks", 0x86dd6, "x32", &[], 5));
    s.push(prog("boss_7/idle_attacks", 0x85cd8, "u8", &[], 16));
    // ---- boss 8 ----
    s.push(prog("boss_8/spots", 0x8a384, "i16*2", &[], 32));
    s.push(prog("boss_8/sidesteps", 0x8a422, "i16", &[], 8));
    pointed(rom, s, "boss_8/dodge_spots", 0x8a6cc, 4, true, ("i16*2 u8*4", 4));
    rank(s, "boss_8/front_attacks", 0x89866);
    rank(s, "boss_8/front_attacks_slow", 0x89c66);
    rank(s, "boss_8/back_attacks", 0x89a66);
    rank(s, "boss_8/back_attacks_slow", 0x89e66);
    rank(s, "boss_8/follow_ups", 0x8a0c6);
    s.push(prog("boss_8/counter_masks", 0x894cc, "x32", &[], 5));
    s.push(prog("boss_8/idle_attacks", 0x88294, "u8", &[], 16));
    // ---- boss 9 ----
    s.push(data("boss_9/lives", 0x8ca02, "u8", &[], PER_RANK));
    s.push(data("boss_9/shot_waits", 0x8c9ce, "u8", &[], PER_RANK));
    s.push(data("boss_9/dashes", 0x8c430, "u8", &[], PER_RANK));
    pointed(rom, s, "boss_9/passes", 0x8bef0, PER_RANK as u32, false, ("u8", 32));
    pointed(rom, s, "boss_9/bombs", 0x8c476, PER_RANK as u32, false, ("u8", 16));
    pointed(rom, s, "boss_9/stops", 0x8c598, PER_RANK as u32, false, ("u8", 16));
    pointed(rom, s, "boss_9/waits", 0x8bc3e, 5, false, ("u8", PER_RANK));
    pointed(rom, s, "boss_9/stop_xs", 0x8bd08, PER_RANK as u32, true, ("u8", 32));
    rank(s, "boss_9/halts", 0x8c6d2);
    s.push(data("boss_9/lanes", 0x8bec4, "i16", &[], 3));
    s.push(data("boss_9/retargets", 0x8c9aa, "u8", &[], 16));
    s.push(prog("boss_9/speeds_hard", 0x8c092, "u8", &[], 0x28));
    upto(s, "boss_9/speeds", 0x8c0ba, true, "u8", 0x28);
    pointed(rom, s, "boss_9/dash_chances", 0x8c20c, 5, false, ("x32", PER_RANK));
    // ---- boss 10 ----
    s.push(prog("boss_10/next_steps", 0x8d540, "u8", &[], 32));
    s.push(prog("boss_10/walk_delays", 0x8d47c, "u8", &[], PER_RANK));
    s.push(prog("boss_10/walk_steps", 0x8d4ce, "u8", &[], 32));
    // ---- boss 11 (and 12) ----
    s.push(prog("boss_11/strike_kinds", 0x8eb42, "u8", &[], 4));
    s.push(prog("boss_11/walk_delays", 0x8eb5e, "u8", &[], PER_RANK));
    s.push(prog("boss_11/walk_steps", 0x8eb90, "u8", &[], 32));
    s.push(prog("boss_11/near_masks", 0x8da88, "x32", &[], 4));
    s.push(prog("boss_11/far_masks", 0x8dad2, "x32", &[], 4));
    s.push(prog("boss_11/rests", 0x8db64, "u8", &[], 4));
    s.push(data("boss_11/close_attacks", 0x8db68, "i8", &[], 16));
    s.push(data("boss_11/near_attacks", 0x8db78, "i8", &[], 16));
    s.push(data("boss_11/far_attacks", 0x8db88, "i8", &[], 16));
    // 16 masks, but the index adds the high byte of a D2 left over by the box tests (original glitch: up to
    // 0x3fff more, read in the code that follows, observed up to 0x288): the whole reachable range is kept.
    s.push(prog("boss_11/give_up_masks", 0x8dbc0, "x8", &[], 0x4008));
    s.push(data("boss_11/charge_lengths", 0x8dfcc, "u8", &[], 64));
    s.push(prog("boss_11/aim_pauses", 0x8dfbc, "u8", &[], 16));
    s.push(data("boss_11/routes", 0x8e1e8, "u8", &[], 16));
    // Relative offsets of the paths (the routes only designate the first); a path: its last point index, then points.
    s.push(prog("boss_11/route_paths", 0x8e0f8, "i16", &[], 1));
    s.push(data("boss_11/route_0", 0x8e1f8, "i16", &[], 9));
    upto(s, "boss_11/stun_times", 0x8e978, true, "u8", 0x40);
    // The rings (`boss_11::chain`): rows per pose, then per rank the path and depth tables.
    s.push(data("boss_11/rows", 0x1069f8, "u8*32", &[], 22));
    s.push(data("boss_11/paths", 0x106cb8, "addr", &[], 24));
    s.push(data("boss_11/paths/0", 0x106d18, "i16*4", &[], 0x400));
    s.push(data("boss_11/paths/8", 0x108d18, "i16*4", &[], 0x400));
    s.push(data("boss_11/paths/10", 0x10ad18, "i16*4", &[], 0x400));
    s.push(data("boss_11/rises/0", 0x10cd18, "i16", &[], 32));
    s.push(data("boss_11/rises/8", 0x10cd58, "i16", &[], 32));
    s.push(data("boss_11/rises/10", 0x10cd98, "i16", &[], 32));
    // ---- boss 15 (and 20) ----
    upto(s, "boss_15/far_masks", 0x91998, true, "x32", 0x20);
    s.push(prog("boss_15/special_moves", 0x91ab0, "u8", &[], 4));
    s.push(data("boss_15/attack_picks", 0x91a28, "i8*16", &[], 4));
    s.push(prog("boss_15/cooldowns", 0x91a18, "i8", &[], 16));
    s.push(prog("boss_15/leap", 0x91b94, "i16", &[], 2));
    // ---- boss 13 (dragon) ----
    // Four thresholds, then the code that follows: the stage index keeps growing past them (read as data, like the
    // original) until a word exceeds the camera.
    s.push(prog("boss_13/stops", 0x8f97a, "i16", &[], 48));
    // Windows of four words starting every two words (8 draws).
    s.push(data("boss_13/bob_speeds", 0x8faa2, "i16", &[], 18));
}

/// A decision: the pick tables `picks` (16 bytes each, program space) named `<name>_picks` (one table) or
/// `<name>_picks_<k>`, and the relative jump table `<name>_jumps`, as long as the largest byte of the picks designates.
fn decision(rom: &Program, s: &mut Vec<Spec>, name: &str, picks: &[u32], jumps: u32) {
    let mut largest = 0;
    for (k, &at) in picks.iter().enumerate() {
        let table = if picks.len() == 1 { format!("{name}_picks") } else { format!("{name}_picks_{k}") };
        s.push(prog(&table, at, "u8", &[], 16));
        largest = largest.max((0..16).map(|i| rom.prog8(at + i)).max().unwrap());
    }
    s.push(prog(&format!("{name}_jumps"), jumps, "i16", &[], largest as usize / 2 + 1));
}

/// A table of `count` addresses (longs of the given space) and the tables of the data space it points to, named
/// `<name>/<k>` (`k`: the first entry that designates it; `target`: their format and rows) unless an earlier declaration
/// already holds them.
fn pointed(rom: &Program, s: &mut Vec<Spec>, name: &str, at: u32, count: u32, program: bool, target: (&'static str, usize)) {
    let (format, rows) = target;
    s.push(if program { prog(name, at, "addr", &[], count as usize) } else { data(name, at, "addr", &[], count as usize) });
    for k in 0..count {
        let target = rom.long(at + 4 * k, program);
        if !s.iter().any(|t| t.rom == target && !t.program) {
            s.push(data(&format!("{name}/{k}"), target, format, &[], rows));
        }
    }
}
