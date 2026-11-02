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
//!   `<family>/<table>` ([`families`]).

use crate::data::{data, prog, Spec};
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
    families(rom, &mut s);
    s
}

/// A row of 32 bytes per pair of difficulty levels (`rank / 2`), drawn with a 5-bit random number.
const BY_RANK: (&str, usize) = ("u8*32", 16);

/// Tables of the families (`enemy_*/`), one per line: name `<family>/<table>`, space, address, format, rows.
fn families(rom: &Program, s: &mut Vec<Spec>) {
    let _ = rom;
    let rank = |s: &mut Vec<Spec>, name: &str, at: u32| s.push(data(name, at, BY_RANK.0, &[], BY_RANK.1));
    // ---- goblin (type 15) ----
    rank(s, "goblin/near_moves", 0x6ebc0);
    rank(s, "goblin/far_moves", 0x6edc0);
    rank(s, "goblin/series", 0x6efc0);
    rank(s, "goblin/follow_moves", 0x6e904);
    s.push(prog("goblin/teleport_x", 0x6e44c, "i16", &[], 4));
    s.push(prog("goblin/teleport_y", 0x6e43e, "i16", &[], 8));
    // ---- gnoll (type 14) ----
    s.push(prog("gnoll/teleport_x", 0x6b8a4, "i16", &[], 4));
    s.push(prog("gnoll/teleport_y", 0x6b89c, "i16", &[], 4));
    // ---- troll (type 13) ----
    s.push(prog("troll/teleport_x", 0x6ceaa, "i16", &[], 4));
    s.push(prog("troll/teleport_y", 0x6ce9c, "i16", &[], 8));
}
