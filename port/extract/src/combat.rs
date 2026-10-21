//! Tables du combat, des ennemis et des caisses : `data/combat.json` (`combat/contact.rs`),
//! `data/enemies.json` (`families/reactions.rs`, `ai.rs`, `families/enemy_common.rs`), `data/boxes.json`
//! (`families/boxes.rs`, `families/items.rs`).

use ddsom_assets::data::Region;

use crate::data::{data, prog, region, span, Spec};
use crate::rom::Program;

/// Réactions du joueur aux coups et aux tirs ennemis (région qui suit les fiches des caisses).
pub fn combat() -> (Vec<Region>, Vec<Spec>) {
    const REACTION: &[&str] = &["ground", "air", "power", "power_hell"];
    let s = vec![
        data("pad/0x1067a2", 0x1067a2, "x16", &[], 1),
        span("enemy_attack_reactions", 0x1067a4, 0x106910, false, "u8*4", REACTION),
        data("pad/0x106910", 0x106910, "x16", &[], 1),
        span("enemy_shot_reactions", 0x106912, 0x1069f6, false, "u8*4", REACTION),
        data("pad/0x1069f6", 0x1069f6, "x16", &[], 1),
        data("shot_guard", 0x27690, "u8", &[], 80),
    ];
    (vec![region("reactions", 0x1067a2, 0x1069f8)], s)
}

/// Tables par type d'ennemi, PV initiaux, points d'approche de l'IA.
pub fn enemies(rom: &Program) -> (Vec<Region>, Vec<Spec>) {
    let mut s = vec![
        // ---- reactions.rs ----
        data("anim_sets", 0x3de58, "addr", &[], 55),
        prog("anim_variants", 0x3e28a, "u8", &[], 4),
        prog("no_grab_down", 0x3e1ee, "u8", &[], 64),
        prog("launch_speeds", 0x3bdcc, "i16*4", &["vx", "vy", "vz", "gravity"], 4),
        prog("launch_counters", 0x3bdec, "u8", &[], 4),
        data("player_hurt_anims", 0x3bb52, "addr", &[], 12),
        // ---- enemy_common.rs : immunités (0xfe5a) et PV initiaux (0x4544, 0x453c) ----
        prog("immunities", 0xfe78, "x32", &[], 55),
        span("hp_boss", 0x458c, 0x5c8c, false, "u16*16", &[]),
        span("hp_enemy", 0x5c8c, 0x788c, false, "u16*16", &[]),
    ];
    // ---- ai.rs : points d'approche (0xe76c) ----
    const TABLE: u32 = 0xe93e;
    let first = rom.rd32(TABLE);
    let count = (first - TABLE) / 4;
    s.push(data("approach_tables", TABLE, "addr", &[], count as usize));
    let targets: Vec<u32> = (0..count).map(|i| rom.rd32(TABLE + 4 * i)).collect();
    let mut lists = targets.clone();
    lists.sort();
    lists.dedup();
    for at in lists {
        let k = targets.iter().position(|&t| t == at).unwrap();
        let n = rom.rd16(at) as usize + 1;
        s.push(data(&format!("approach/{k}.count"), at, "u16", &[], 1));
        s.push(data(&format!("approach/{k}"), at + 2, "i16*2", &["dx", "dy"], n));
    }
    let end = s.iter().filter(|t| t.name.starts_with("approach/")).map(|t| t.rom).max().unwrap();
    let last = s.iter().find(|t| t.rom == end).unwrap();
    let end = end + 4 * last.rows as u32;
    (vec![region("hp", 0x458c, 0x788c), region("approach", TABLE, end)], s)
}

/// Fiches des caisses, butins, chances d'usure de l'équipement.
pub fn boxes(rom: &Program) -> (Vec<Region>, Vec<Spec>) {
    const START: u32 = 0x1051ae;
    const COUNT: u32 = 122;
    const FICHE: &str = "u8*10 u8*16 u8*16";
    let fiches: Vec<u32> = (0..COUNT).map(|i| rom.rd32(START + 4 * i)).collect();
    // une seule fiche est suivie de 8 octets inutilisés : les fiches forment deux suites
    let gap = fiches.windows(2).find(|w| w[1] - w[0] != 0x2a).map(|w| w[0] + 0x2a).unwrap();
    let next = fiches.iter().copied().find(|&f| f > gap).unwrap();
    let end = fiches[COUNT as usize - 1] + 0x2a;
    let s = vec![
        data("fiches", START, "addr", &[], COUNT as usize),
        span("fiches/a", fiches[0], gap, false, FICHE, &[]),
        span("pad/fiches", gap, next, false, "u8", &[]),
        span("fiches/b", next, end, false, FICHE, &[]),
        data("loot", 0xda28a, "u8*2", &["kind", "variant"], 135),
        data("wear_chances", 0xdb7b8, "x16*6", &[], 3),
    ];
    (vec![region("fiches", START, end)], s)
}
