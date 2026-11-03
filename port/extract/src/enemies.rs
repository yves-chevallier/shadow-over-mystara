//! Tables of the common enemy code, added to `data/enemies.json` (`combat::enemies`): read by the game through
//! `families/enemy_common/tables.rs` (`data().enemies`; docs/fin.md, L8).
//!
//! * the reaction tables of the families (one routine address per hit code `+0x49`, program space), keyed by their
//!   address: the families and the bosses pass that address to `enemy_common::react`;
//! * the per-type tables of the common reactions (`families/reactions`): sounds, get-up routines, the animation sets
//!   that a small routine of the original gives, and the small tables of the reaction steps;
//! * the tables of `enemy_lib` (decision masks, palette scripts) and the piece lists of `enemy_common::spawn_list`.
//!
//! * the tables of the families themselves (`enemy_*/`: moves by difficulty level, decisions, offsets...), named
//!   `<family>/<table>` ([`families`]);
//! * the tables of the bosses, named `<boss>/<table>` (`crate::bosses`).

use crate::data::{data, prog, row_size, Spec};
use crate::rom::Program;

/// Number of hit codes of a reaction table (codes 0 to 0xa8, step 4).
const REACTION_CODES: usize = 43;

/// Reaction tables (address, name): enemies by family, then bosses. The two tables of the golem stop one entry
/// short (code 0xa8 would read the code that follows).
const REACTIONS: [(u32, &str, usize); 47] = [
    (0x56816, "kobold_first", REACTION_CODES),
    (0x56a0e, "kobold_second", REACTION_CODES),
    (0x58d0a, "skeleton_first", REACTION_CODES),
    (0x58f2e, "skeleton_second", REACTION_CODES),
    (0x5acfc, "trog_first", REACTION_CODES),
    (0x5ae8e, "trog_second", REACTION_CODES),
    (0x5cbe8, "bow_first", REACTION_CODES),
    (0x5cd14, "bow_second", REACTION_CODES),
    (0x5ea5c, "owlbear_first", REACTION_CODES),
    (0x5ecb8, "owlbear_second", REACTION_CODES),
    (0x6090c, "ghoul_first", REACTION_CODES),
    (0x60abe, "ghoul_second", REACTION_CODES),
    (0x62aa2, "hellhound_first", REACTION_CODES),
    (0x62d76, "hellhound_second", REACTION_CODES),
    (0x6518a, "gargoyle", REACTION_CODES),
    (0x68542, "golem_first", REACTION_CODES - 1),
    (0x68724, "golem_second", REACTION_CODES - 1),
    (0x6a206, "elf_first", REACTION_CODES),
    (0x6a3a0, "elf_second", REACTION_CODES),
    (0x6bb4c, "gnoll_first", REACTION_CODES),
    (0x6bd3a, "gnoll_second", REACTION_CODES),
    (0x6d0f0, "troll_first", REACTION_CODES),
    (0x6d284, "troll_second", REACTION_CODES),
    (0x6e68c, "goblin_first", REACTION_CODES),
    (0x6e824, "goblin_second", REACTION_CODES),
    (0x6fe22, "riyakaa_standing", REACTION_CODES),
    (0x70018, "riyakaa_acting", REACTION_CODES),
    (0x715d4, "ogre", REACTION_CODES),
    (0x740c0, "dbeast_part", REACTION_CODES),
    (0x742fa, "dbeast", REACTION_CODES),
    (0x77aec, "double", REACTION_CODES),
    (0x78a26, "warmachine", REACTION_CODES),
    (0x7a396, "boss_0", REACTION_CODES),
    (0x7bbbe, "boss_1", REACTION_CODES),
    (0x7e5e2, "boss_22_first", REACTION_CODES),
    (0x80b0a, "boss_3", REACTION_CODES),
    (0x82c8a, "boss_4", REACTION_CODES),
    (0x8527a, "boss_6", REACTION_CODES),
    (0x86c3a, "boss_7", REACTION_CODES),
    (0x89330, "boss_8", REACTION_CODES),
    (0x8d176, "boss_10", REACTION_CODES),
    (0x8e3dc, "boss_11", REACTION_CODES),
    (0x909a0, "boss_19", REACTION_CODES),
    (0x940a8, "boss_16", REACTION_CODES),
    (0x96a0c, "boss_17", REACTION_CODES),
    (0x9a40a, "boss_21", REACTION_CODES),
    (0x9ce66, "boss_22_second", REACTION_CODES),
];

/// Animation sets per type of the common reactions (`anim_sets`, data space) and their number of entries.
const ANIM_SETS: u32 = 0x3de58;
const ANIM_SET_COUNT: u32 = 55;

/// Piece lists of `enemy_common::spawn_list`: (name, list addresses, number of lists, allocation routines).
const SPAWN_LISTS: [(&str, u32, u32, u32); 2] = [("beetle_nest", 0x65c78, 4, 0x65c88), ("scorpion", 0x669d6, 5, 0x669ea)];

/// Entry of a piece list: allocation routine (index), type, variant, group `+0x2f`, X, height, depth.
const PIECE: &[&str] = &["alloc", "kind", "variant", "group", "x", "height", "depth"];

pub fn specs(rom: &Program) -> Vec<Spec> {
    let mut s: Vec<Spec> =
        REACTIONS.iter().map(|&(at, name, rows)| prog(&format!("reactions/{name}"), at, "addr", &[], rows)).collect();
    // ---- reactions: per-type tables (index: type, plus 32 outside the ENEMY class) ----
    s.push(prog("hit_sounds", 0x3e2ac, "addr", &[], 55));
    s.push(prog("death_sounds", 0x3e3a4, "addr", &[], 55));
    s.push(prog("get_up_routines", 0x3e000, "addr", &[], 55));
    // the entries of `anim_sets` whose high word is zero designate a routine (`bsr ; movea.l #set,A4 ; jmp`): its
    // set is the immediate operand at +6
    for i in 0..ANIM_SET_COUNT {
        let entry = rom.rd32(ANIM_SETS + 4 * i);
        if entry >> 16 == 0 {
            s.push(prog(&format!("anim_set_routines/{i}"), ANIM_SETS + (entry & 0xffff) + 6, "addr", &[], 1));
        }
    }
    // ---- reactions: tables of the steps ----
    s.push(prog("pop_speeds", 0x3c5f2, "i16*2", &["vz", "gravity"], 2));
    s.push(prog("daze_delays", 0x3c904, "u8", &[], 4));
    s.push(prog("seized_offsets", 0x3d07e, "i16*2", &["dx", "unread"], 23));
    s.push(prog("seized_damage", 0x3d0da, "u8", &[], 14));
    s.push(prog("seized_hit_speeds", 0x3d198, "i16*2", &["vx", "ax"], 23));
    s.push(prog("quiver_delays", 0x3d42a, "u8", &[], 4));
    // ---- enemy_lib ----
    s.push(prog("rank_masks", 0xfff4, "x16", &[], 4));
    s.push(data("chance_rows", 0xadb0, "addr", &[], 6));
    s.push(data("chance_masks", 0xadc8, "x32*32", &[], 5));
    s.push(data("palette_scripts", 0x102ba, "x32*8", &[], 8));
    // ---- enemy_common::spawn_list ----
    for (name, lists, count, allocs) in SPAWN_LISTS {
        s.push(prog(&format!("{name}/lists"), lists, "addr", &[], count as usize));
        s.push(prog(&format!("{name}/allocs"), allocs, "addr", &[], 1));
        for k in 0..count {
            let at = rom.prog32(lists + 4 * k);
            let entries = rom.rd16(at) as usize + 1;
            s.push(data(&format!("{name}/{k}.count"), at, "u16", &[], 1));
            s.push(data(&format!("{name}/{k}"), at + 2, "u8*4 u16*3", PIECE, entries));
        }
    }
    let mut bounded = Vec::new();
    families(rom, &mut s, &mut bounded);
    crate::bosses::specs(rom, &mut s, &mut bounded);
    resolve(&mut s, bounded);
    s
}

/// A table whose end is not known from the code: it runs up to the next table of the file or the next code executed by
/// the original (`code_68k.txt`), at most `max` bytes (index of the table in the specs, maximum).
pub(crate) type Bounded = Vec<(usize, u32)>;

fn resolve(s: &mut [Spec], bounded: Bounded) {
    let code = crate::effects::code_starts();
    let starts: Vec<u32> = s.iter().map(|t| t.rom).collect();
    for (i, max) in bounded {
        let at = s[i].rom;
        let next_table = starts.iter().copied().filter(|&a| a > at).min();
        let next_code = code.iter().copied().find(|&a| a > at);
        let end = [next_table, next_code, Some(at + max)].into_iter().flatten().min().unwrap();
        s[i].rows = (end - at) as usize / row_size(s[i].format);
    }
}

/// A row of 32 bytes per pair of difficulty levels (`rank / 2`), drawn with a 5-bit random number.
const BY_RANK: (&str, usize) = ("u8*32", 16);

/// Tables of the families (`enemy_*/`), one per line: name `<family>/<table>`, space, address, format, rows.
fn families(rom: &Program, s: &mut Vec<Spec>, bounded: &mut Bounded) {
    let _ = rom;
    let rank = |s: &mut Vec<Spec>, name: &str, at: u32| s.push(data(name, at, BY_RANK.0, &[], BY_RANK.1));
    // a table whose end is not known (see `Bounded`), of the program or the data space
    let mut upto = |s: &mut Vec<Spec>, name: &str, at: u32, program: bool, format: &'static str, max: u32| {
        bounded.push((s.len(), max));
        s.push(if program { prog(name, at, format, &[], 0) } else { data(name, at, format, &[], 0) });
    };
    // ---- goblin (type 15) ----
    rank(s, "goblin/near_moves", 0x6ebc0);
    rank(s, "goblin/far_moves", 0x6edc0);
    rank(s, "goblin/series", 0x6efc0);
    rank(s, "goblin/follow_moves", 0x6e904);
    s.push(prog("goblin/teleport_x", 0x6e44c, "i16", &[], 4));
    s.push(prog("goblin/teleport_y", 0x6e43e, "i16", &[], 8));
    // ---- gnoll (type 13) ----
    s.push(prog("gnoll/teleport_x", 0x6b8a4, "i16", &[], 4));
    s.push(prog("gnoll/teleport_y", 0x6b89c, "i16", &[], 4));
    // ---- troll (type 14) ----
    s.push(prog("troll/teleport_x", 0x6ceaa, "i16", &[], 4));
    s.push(prog("troll/teleport_y", 0x6ce9c, "i16", &[], 8));
    rank(s, "troll/near_moves", 0x6d620);
    rank(s, "troll/far_moves", 0x6d820);
    rank(s, "troll/follow_moves", 0x6d364);
    rank(s, "gnoll/near_moves", 0x6c0d6);
    rank(s, "gnoll/far_moves", 0x6c2d6);
    rank(s, "gnoll/follow_moves", 0x6be1a);
    // ---- bow gnoll (type 3) ----
    rank(s, "bow/first_actions", 0x5d47a);
    rank(s, "bow/second_actions", 0x5d67a);
    rank(s, "bow/shot_repeats", 0x5d07a);
    rank(s, "bow/follow_moves", 0x5ce72);
    s.push(prog("bow/shot_mask", 0x5cde0, "x16", &[], 1));
    // ---- shadow elf (type 12) ----
    rank(s, "elf/fallback_moves", 0x6a480);
    rank(s, "elf/close_attacks", 0x6a73c);
    rank(s, "elf/far_attacks", 0x6a93c);
    s.push(prog("elf/jump_dx", 0x69f4a, "i16", &[], 4));
    s.push(prog("elf/jump_dy", 0x69f42, "i16", &[], 4));
    // ---- mud golem (type 11) ----
    rank(s, "golem/next_moves", 0x69146);
    upto(s, "golem/parry_by_group", 0x682e8, true, "u8", 0x10);
    // ---- kobold (type 0) ----
    lists(rom, s, "kobold/near_lists", 0x56d04, 4);
    lists(rom, s, "kobold/far_lists", 0x56d14, 4);
    rank(s, "kobold/follow_moves", 0x5799c);
    upto(s, "kobold/strike_powers", 0x57c52, false, "u8*4", 0x40);
    s.push(prog("kobold/dash_times", 0x553b4, "u16", &[], 16));
    s.push(data("kobold/breath_repeats", 0x55de6, "u8", &[], 32));
    s.push(prog("kobold/breath_sounds", 0x55e50, "u8", &[], 16));
    // ---- skeleton (type 1) ----
    rank(s, "skeleton/close_attacks", 0x5932a);
    rank(s, "skeleton/far_attacks", 0x5952a);
    rank(s, "skeleton/next_moves", 0x59982);
    upto(s, "skeleton/series_ai", 0x57de6, false, "u8*4", 0x20);
    upto(s, "skeleton/parry_by_series", 0x589da, true, "u8", 0x10);
    // ---- troglodyte (type 2) ----
    rank(s, "trog/close_attacks", 0x5b074);
    rank(s, "trog/close_attacks_armed", 0x5b274);
    rank(s, "trog/repeats", 0x5b4d6);
    rank(s, "trog/jump_moves", 0x5b6f8);
    rank(s, "trog/blink_delays", 0x5b974);
    rank(s, "trog/fallback_moves", 0x5bba6);
    s.push(prog("trog/blink_times", 0x5b96c, "u16", &[], 4));
    upto(s, "trog/walk_speeds", 0x5af64, false, "u16", 0x20);
    // ---- owlbear (type 4) ----
    rank(s, "owlbear/close_attacks", 0x5eec6);
    rank(s, "owlbear/far_attacks", 0x5f0c6);
    rank(s, "owlbear/next_moves", 0x5f3d8);
    s.push(data("owlbear/series", 0x5f5fa, "u8*16", &[], 16));
    s.push(prog("owlbear/attack_chance", 0x5f2ea, "x32", &[], 32));
    // ---- ghoul (type 5) ----
    rank(s, "ghoul/close_attacks", 0x60d38);
    rank(s, "ghoul/far_attacks", 0x60f38);
    rank(s, "ghoul/fallback_moves", 0x61164);
    // ---- hellhound (type 6) ----
    lists(rom, s, "hellhound/move_lists", 0x62f80, 4);
    rank(s, "hellhound/follow_moves", 0x637fe);
    s.push(prog("hellhound/turn_mask", 0x61a00, "x32", &[], 1));
    upto(s, "hellhound/flame_script", 0x62284, false, "u8*2", 0x20);
    // ---- gargoyle (type 8) ----
    s.push(prog("gargoyle/charge_a", 0x65654, "u8", &[], 32));
    s.push(prog("gargoyle/charge_b", 0x65674, "u8", &[], 32));
    s.push(prog("gargoyle/wander_a", 0x656b8, "u8", &[], 32));
    s.push(prog("gargoyle/wander_b", 0x656d8, "u8", &[], 32));
    s.push(prog("gargoyle/near_moves", 0x65792, "u8*16", &[], 3));
    s.push(prog("gargoyle/side_moves", 0x65822, "u8*16", &[], 2));
    s.push(prog("gargoyle/fallback_moves", 0x658a2, "u8*16", &[], 3));
    upto(s, "gargoyle/bob_steps", 0x65a5a, true, "i32", 0x40);
    upto(s, "gargoyle/rise_steps", 0x65a86, true, "i32", 0x40);
    upto(s, "gargoyle/ride_times", 0x650ba, true, "u16", 0x10);
    // ---- ogre (type 18) ----
    s.push(prog("ogre/charge_a", 0x719f6, "u8", &[], 32));
    s.push(prog("ogre/charge_b", 0x71a16, "u8", &[], 32));
    s.push(prog("ogre/wander_a", 0x71a5a, "u8", &[], 32));
    s.push(prog("ogre/wander_b", 0x71a7a, "u8", &[], 32));
    s.push(prog("ogre/side_moves", 0x71b34, "u8", &[], 16));
    s.push(prog("ogre/front_moves", 0x71ba4, "u8*16", &[], 3));
    s.push(prog("ogre/back_moves", 0x71c32, "u8*16", &[], 3));
    upto(s, "ogre/bob_steps", 0x71dc6, true, "i32", 0x80);
    upto(s, "ogre/rise_steps", 0x713e8, true, "i32", 0x40);
    // ---- war machine (type 27) ----
    s.push(prog("warmachine/charge_a", 0x78e8c, "u8", &[], 32));
    s.push(prog("warmachine/charge_b", 0x78eac, "u8", &[], 32));
    s.push(prog("warmachine/wander_a", 0x78ef0, "u8", &[], 32));
    s.push(prog("warmachine/wander_b", 0x78f10, "u8", &[], 32));
    s.push(prog("warmachine/shot_counts", 0x78856, "u8", &[], 64));
    // decisions: bytes drawn by 16 (even offsets), then words relative to their table (jumps to code)
    s.push(prog("warmachine/point_blank_picks", 0x78fb8, "u8*16", &[], 1));
    upto(s, "warmachine/point_blank_jumps", 0x78fc8, true, "i16", 0x20);
    s.push(prog("warmachine/phase_picks", 0x79002, "u8*16", &[], 1));
    upto(s, "warmachine/phase_jumps", 0x79012, true, "i16", 0x20);
    s.push(prog("warmachine/near_picks", 0x7907e, "u8*16", &[], 3));
    upto(s, "warmachine/near_jumps", 0x790ae, true, "i16", 0x20);
    s.push(prog("warmachine/wide_blocks_0", 0x790e6, "i16", &[], 6));
    s.push(prog("warmachine/wide_blocks_1", 0x790f2, "i16", &[], 6));
    s.push(prog("warmachine/wide_picks", 0x79158, "u8*16", &[], 4));
    upto(s, "warmachine/wide_jumps", 0x79198, true, "i16", 0x20);
    // ---- displacer beast (type 19) and its parts ----
    lists(rom, s, "dbeast/attack_lists", 0x7462e, 2);
    rank(s, "dbeast/close_moves", 0x74cac);
    s.push(prog("dbeast/shift_picks", 0x74f56, "u8", &[], 16));
    upto(s, "dbeast/shifts_a", 0x74f7a, true, "u8", 0x10);
    upto(s, "dbeast/shifts_b", 0x74f8a, true, "u8", 0x10);
    upto(s, "dbeast/follow_offsets", 0x7202a, true, "i16*2", 0x40);
    upto(s, "dbeast/body_offsets", 0x720ee, true, "i16*4", 0x80);
    upto(s, "dbeast/rally_offsets", 0x72224, true, "i16*2", 0x40);
    upto(s, "dbeast/arm_anims", 0x724c8, true, "u8", 0x10);
    s.push(prog("dbeast/crawl_delays", 0x72a6e, "u16", &[], 16));
    s.push(data("dbeast/pounce_counts", 0x73516, "u8", &[], 32));
    s.push(prog("dbeast/pounce_cries", 0x73584, "u8", &[], 16));
    // ---- parts of the composite enemies (type 7) ----
    upto(s, "part/offsets", 0x63f34, true, "i16*4", 0x80);
    upto(s, "part/sub_for_host", 0x63fa8, false, "u8", 0x20);
    upto(s, "part/idle_delays", 0x63c7e, true, "u16", 0x20);
    // ---- giant beetle (type 9): speed tables by difficulty (addresses in `data/speeds.json`) ----
    s.push(data("beetle/walk_speeds", 0x66700, "addr", &[], 8));
    s.push(data("beetle/fly_speeds", 0x66720, "addr", &[], 8));
    s.push(data("beetle/turn_rates_a", 0x666b4, "u8*2", &[], 2));
    s.push(data("beetle/turn_rates_b", 0x666b8, "u8*2", &[], 2));
    s.push(prog("beetle/turn_deltas", 0x66750, "i8", &[], 4));
    s.push(data("beetle/turn_targets", 0x667d4, "u8*2", &[], 16));
    upto(s, "beetle/stop_xs", 0x660d6, false, "u16", 0x20);
    s.push(data("beetle/depth_accels", 0x66666, "i16", &[], 32));
    s.push(prog("beetle/bite_odds", 0x6623a, "x32", &[], 16));
    s.push(prog("beetle/shake_times", 0x664c6, "u8", &[], 4));
    // ---- giant scorpion (type 10) ----
    s.push(data("scorpion/attacks_a", 0x66e48, "u8*16", &[], 8));
    s.push(data("scorpion/attacks_b", 0x67146, "u8*16", &[], 8));
    s.push(data("scorpion/moves", 0x6773c, "u8*16", &[], 8));
    rank(s, "scorpion/attack_states", 0x674fa);
    s.push(data("scorpion/idle_times", 0x66fbe, "u8", &[], 16));
    s.push(prog("scorpion/move_times", 0x677f8, "u8*4", &[], 4));
    s.push(prog("scorpion/retreat_ranks", 0x677ce, "u8", &[], 16));
    s.push(prog("scorpion/strike_times", 0x670dc, "u8", &[], 4));
    upto(s, "scorpion/move_speeds", 0x670e0, true, "u8", 0x10);
    // ---- harpy (type 21) ----
    upto(s, "harpy/variants", 0x756e6, false, "x16 u8*4 i16", 0x80);
    upto(s, "harpy/depth_offsets", 0x757d2, false, "i16", 0x80);
    upto(s, "harpy/hit_anims", 0x75e84, true, "u8", 0x10);
    // ---- dark warrior's arms (type 25) ----
    s.push(prog("warrior/bob", 0x7759e, "i8", &[], 64));
    upto(s, "warrior/script_offsets", 0x77626, true, "i16", 0x10);
    upto(s, "warrior/scripts", 0x77626, false, "u8", 0x200);
    // ---- riyakaa (type 16): its fallback moves are those of the bow gnolls (`bow/follow_moves`) ----
    rank(s, "riyakaa/next_attacks", 0x701dc);
    s.push(data("riyakaa/counter_masks", 0x6ff70, "x32", &[], 8));
    // ---- doubles (types 22 to 26) ----
    s.push(prog("double/reversing_terrains", 0x77d08, "u8", &[], 9));
}

/// A table of `count` addresses of tables by difficulty level (data space), and the tables it points to, named
/// `<name>/<k>` (unless an earlier list already declared them).
fn lists(rom: &Program, s: &mut Vec<Spec>, name: &str, at: u32, count: u32) {
    s.push(data(name, at, "addr", &[], count as usize));
    for k in 0..count {
        let target = rom.rd32(at + 4 * k);
        if !s.iter().any(|t| t.rom == target && !t.program) {
            s.push(data(&format!("{name}/{k}"), target, BY_RANK.0, &[], BY_RANK.1));
        }
    }
}
