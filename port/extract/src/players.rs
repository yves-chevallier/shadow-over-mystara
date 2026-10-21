//! Tables des joueurs : `data/players.json` (`player/init.rs`, `player/walk.rs`, `player/characters/`).
//!
//! La région 0x101166-0x1051ae ne contient que des tables par personnage : chaque table d'adresses (12
//! longs, un par personnage, souvent deux fois la même adresse) désigne une table propre au personnage.
//! Les tailles viennent de l'écart entre les tables désignées.

use ddsom_assets::data::Region;

use crate::data::{data, prog, region, row_size, span, Spec};
use crate::rom::Program;

/// Région des tables par personnage (la fiche des caisses `0x1051ae` suit).
const START: u32 = 0x101166;
const END: u32 = 0x1051ae;

const CLASSES: [&str; 6] = ["fighter", "cleric", "elf", "dwarf", "magic_user", "thief"];

/// Nom des personnages qui partagent une table : la classe si ce sont ses deux variantes, sinon chaque
/// variante (`classe.0`, `classe.1`) ; plusieurs classes sont jointes par `+`.
fn owners(chars: &[usize]) -> String {
    let mut names = Vec::new();
    for (c, class) in CLASSES.iter().enumerate() {
        match (chars.contains(&(2 * c)), chars.contains(&(2 * c + 1))) {
            (true, true) => names.push(class.to_string()),
            (true, false) => names.push(format!("{class}.0")),
            (false, true) => names.push(format!("{class}.1")),
            _ => {}
        }
    }
    names.join("+")
}

/// Table d'adresses `pointers` (12 longs) et les tables qu'elle désigne, jusqu'à `end`.
#[allow(clippy::too_many_arguments)]
fn per_character(
    rom: &Program,
    specs: &mut Vec<Spec>,
    role: &str,
    pointers: u32,
    program: bool,
    end: u32,
    format: &'static str,
    fields: &'static [&'static str],
) {
    let targets: Vec<u32> = (0..12).map(|k| rom.long(pointers + 4 * k, program)).collect();
    let pointer_spec = if program { prog } else { data };
    specs.push(pointer_spec(role, pointers, "addr", &[], 12));
    let mut starts = targets.clone();
    starts.sort();
    starts.dedup();
    for (i, &t) in starts.iter().enumerate() {
        let next = starts.get(i + 1).copied().unwrap_or(end);
        let chars: Vec<usize> = (0..12).filter(|&k| targets[k] == t).collect();
        specs.push(span(&format!("{role}/{}", owners(&chars)), t, next, false, format, fields));
    }
}

pub fn specs(rom: &Program) -> (Vec<Region>, Vec<Spec>) {
    let mut s = Vec::new();
    // ---- init.rs : création du joueur ----
    per_character(rom, &mut s, "level_by_scene", 0x3208, true, 0x101436, "u8", &[]);
    per_character(rom, &mut s, "hp_by_level", 0x325e, true, 0x101556, "u16", &[]);
    per_character(rom, &mut s, "weapons_by_level", 0x3522, true, 0x101722, "u8*4", &["weapon_a", "weapon_b", "flag_a", "flag_b"]);
    per_character(rom, &mut s, "random_stats", 0x32d8, true, 0x1018a2, "u8*8", &[]);
    per_character(rom, &mut s, "weapon_power", 0x3590, true, 0x101a82, "u8*2", &["power", "cap"]);
    per_character(rom, &mut s, "unlocked_by_level", 0x345e, true, 0x101e1a, "u8*8", &[]);
    per_character(rom, &mut s, "gear_by_level", 0x3402, false, 0x103c7a, "u8*48", &[]);
    per_character(rom, &mut s, "spells_by_level", 0x34b4, true, 0x10417a, "u8*16", &[]);
    // ---- dégâts des sorts et objets par niveau du lanceur : (ennemis, chefs) ----
    for i in 0..15u32 {
        let at = 0x10417a + 60 * i;
        s.push(data(&format!("magic_damage/{i}"), at, "u8*30", &[], 2));
    }
    // ---- vitesses de saut (une ligne par type de saut `+0x67`) ----
    let jumps = [
        ("fighter", 0x1044fe),
        ("cleric", 0x10451e),
        ("elf", 0x10453e),
        ("dwarf", 0x10455e),
        ("magic_user", 0x10459e),
        ("thief", 0x1045be),
    ];
    for (i, &(class, at)) in jumps.iter().enumerate() {
        let next = jumps.get(i + 1).map(|j| j.1).unwrap_or(0x1045de);
        s.push(span(&format!("jump_speeds/{class}"), at, next, false, "i16*4", &["vx", "ax", "vz", "az"]));
    }
    // ---- vitesses de marche : 16 couples (vx, vy) par combinaison de directions ----
    let walks = [
        ("walk_normal", 0x1045de),
        ("walk_unused", 0x10478e),
        ("walk_slow", 0x10493e),
        ("walk_run", 0x104aee),
        ("walk_fast", 0x104c9e),
        ("walk_crawl", 0x104e4e),
        ("walk_slow_b", 0x104ffe),
    ];
    for (i, &(name, at)) in walks.iter().enumerate() {
        let next = walks.get(i + 1).map(|w| w.1).unwrap_or(END);
        per_character(rom, &mut s, name, at, false, next, "i16*2", &["vx", "vy"]);
    }
    // ---- tables rangées dans le code ----
    s.push(data("shock_damage", 0x10108, "u16", &[], 5));
    s.push(prog("throw_kind", 0x371b2, "u8", &[], 12));
    s.push(prog("special_anim", 0x372e4, "u8", &[], 88));
    s.push(prog("ungrabbable", 0x37588, "u8", &[], 32));
    s.push(prog("dust_stand", 0x3faac, "i16*2", &["vx", "vy"], 6));
    s.push(prog("dust_run", 0x3fb02, "i16*2", &["vx", "vy"], 6));
    s.push(prog("dust_jump", 0x3fb48, "i16*2", &["vx", "vy"], 6));
    s.push(prog("item_sparks", 0x3fbb8, "i16*4", &["d0", "d1", "d2", "height"], 42));
    s.push(prog("undead_counts", 0x4054e, "u8*2", &["kind1", "kind5"], 30));
    s.push(prog("cure_counts", 0x40ece, "u8", &[], 30));
    s.push(prog("blessing_durations", 0x410a6, "u16", &[], 25));
    s.push(prog("hold_counts", 0x4131e, "u8", &[], 40));
    s.push(data("heal_amounts", 0x416cc, "u16*24", &[], 3));
    s.push(data("heal_flicker", 0x41792, "u16*8", &[], 3));
    s.push(prog("banish_counts", 0x41afe, "u8", &[], 40));
    s.push(prog("protection_durations", 0x41e4c, "u16", &[], 25));
    s.push(prog("portal_speeds", 0x47e78, "u16", &[], 12));
    s.push(prog("regroup_speeds", 0x4851c, "u16", &[], 12));
    s.push(prog("backstab_offsets", 0x4935a, "i16*2", &["dx", "dz"], 23));
    debug_assert!(row_size("u8*48") == 0x30);
    (vec![region("players", START, END)], s)
}
