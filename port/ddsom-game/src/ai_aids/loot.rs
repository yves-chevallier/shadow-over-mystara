//! Debris and loot (0x1172e, 0x11a36, 0xb648, 0xb780, 0xbad6, 0xe2f6).

use super::*;

/// Allocates through `alloc` (0x2318, 0x23d8): the object, or `None` if the pool is full.
pub(super) fn alloc(b: &mut Cps2Bus, pool: &pools::Pool) -> Option<u32> {
    pool.alloc(b).map(|o| o.0)
}

/// Copies the position of `from` to `to`, set 16 pixels higher and just in front.
pub(super) fn place_above(b: &mut Cps2Bus, from: u32, to: u32) {
    for field in [0x08, 0x0c, 0x10] {
        let v = rd16(b, from + field);
        wr16(b, to + field, v);
    }
    let z = rd16(b, to + 0x0c).wrapping_add(0x10);
    wr16(b, to + 0x0c, z);
    let depth = rd16(b, to + 0x10).wrapping_sub(1);
    wr16(b, to + 0x10, depth);
}

/// Debris (EFFCT of type 0x32, variant 0xc): `+0x58`/`+0x18` then speeds read from the table A2,
/// offset and x speed flipped if the object faces left. A row is only consumed if
/// the effect could be created.
pub(super) fn debris(b: &mut Cps2Bus, fx: u32, me: u32) {
    wr8(b, fx, 1);
    wr8(b, fx + 2, 0x32);
    wr8(b, fx + 3, 0xc);
    let x = rd16(b, me + 0x08);
    wr16(b, fx + 0x08, x);
}

/// Loot thrown into the air (ITEM of kind `kind`, variant `variant`): speeds (±0x140, -0x28 in z...),
/// flipped if `mirrored`.
pub(super) fn tossed_item(b: &mut Cps2Bus, me: u32, item: u32, kind: u8, variant: u8, vy: u16, mirrored: bool) {
    wr8(b, item, 1);
    wr8(b, item + 2, kind);
    wr8(b, item + 3, variant);
    place_above(b, me, item);
    wr16(b, item + 0x18, 0x300);
    wr16(b, item + 0x1a, (-0x28i16) as u16);
    wr16(b, item + 0x14, 0x140);
    wr16(b, item + 0x1c, vy);
    if mirrored {
        for field in [0x14, 0x1c] {
            let v = rd16(b, item + field).wrapping_neg();
            wr16(b, item + field, v);
        }
    }
}

/// Number of loot draws: two with more than three players alive, otherwise one.
pub(super) fn loot_rolls(b: &mut Cps2Bus) -> u16 {
    if rd8(b, A5 + 0x35) > 3 {
        2
    } else {
        1
    }
}

/// Special object drawn by 0xbad6: kind and variant.
pub(super) const DRAWN_KIND: u32 = A5 - 0x7196;
pub(super) const DRAWN_VARIANT: u32 = A5 - 0x7195;

/// Tables of 0xbad6 (read as data, A1 = `lea` of the table): row `+0x80` (kind, variant pairs), then by difficulty
/// (`A5+0x49`).
pub(super) mod special {
    pub const ROWS: u32 = 0xbf26;
    pub const COMMON: u32 = 0xc4c6;
    pub const PAIRS: u32 = 0xca86;
    pub const KIND_2: u32 = 0xd606;
    pub const BY_CLASS: u32 = 0xd046;
    pub const CLERIC_OR_MAGE: u32 = 0xd776;
    pub const MAGES: u32 = 0xda56;
    pub const CLERICS: u32 = 0xdbc6;
    pub const MAGE_OR_CLERIC: u32 = 0xdd36;
}

/// Character classes (`+3` / 2 of a player) that count for the special loot.
pub(super) const CLERIC: u8 = 1;
pub(super) const ELF: u8 = 2;
pub(super) const MAGE: u8 = 4;

/// Steps of the 0xbad6 draw (the labels of the original).
#[derive(Clone, Copy)]
pub(super) enum Draw {
    /// 0xbaf2: row of the object.
    Row,
    /// 0xbb20: common object, or of kind 7 if a cleric, an elf or a magician is in play.
    Common,
    /// 0xbba8: (kind, variant) pair drawn.
    Pair,
    /// 0xbbda: kind 2.
    Kind2,
    /// 0xbc02: according to the classes in play.
    ByClass,
    /// 0xbcb4: certain objects are reserved for classes; otherwise the object is dropped.
    Restrict,
    Spawn,
    /// 0xbf20: next draw.
    Next,
}

pub(super) fn drawn(b: &mut Cps2Bus) -> (u8, u8) {
    (rd8(b, DRAWN_KIND), rd8(b, DRAWN_VARIANT))
}
pub(super) fn set_drawn(b: &mut Cps2Bus, kind: u8, variant: u8) {
    wr8(b, DRAWN_KIND, kind);
    wr8(b, DRAWN_VARIANT, variant);
}
/// New kind 4 variant drawn in `at`: 0 cancels the draw, 1 and 2 refer back to the draws `on_1`, `on_2`.
pub(super) fn redraw(b: &mut Cps2Bus, at: u32, on_1: Draw, on_2: Draw) -> Draw {
    let variant = rd8(b, at);
    set_drawn(b, 4, variant);
    match variant {
        0 => Draw::Next,
        1 => on_1,
        2 => on_2,
        _ => Draw::Restrict,
    }
}

pub(super) fn reroll_or_none(b: &mut Cps2Bus, at: u32) -> Draw {
    let variant = rd8(b, at);
    set_drawn(b, 4, variant);
    if variant == 0 { Draw::Next } else { Draw::Spawn }
}

/// Debris sets of 0x1172e.
pub(super) const DEBRIS_SETS: u32 = 0x1178a;

/// 0x1172e: debris set no. `set` (table: number - 1, then rows of 4 words: x offset, x speed, `+0x18`,
/// `+0x58`) around `o`, flipped if `o` faces left. A row is only read if the effect could be created.
pub fn debris_set(b: &mut Cps2Bus, o: Obj, set: u16) {
    let mut at = crate::mem::prog32(b, DEBRIS_SETS.wrapping_add(offset(set.wrapping_mul(4))));
    let mut next = |b: &mut Cps2Bus| {
        let v = rd16(b, at);
        at += 2;
        v
    };
    let count = next(b);
    let mirrored = o.u8(b, f::FACING) != 0;
    for _ in 0..=count {
        let Some(fx) = alloc(b, &pools::EFFCT) else { continue };
        debris(b, fx, o.0);
        let z = o.u16(b, Obj::Z);
        wr16(b, fx + 0x0c, z);
        let depth = o.u32(b, Obj::Y).wrapping_sub(1);
        wr32(b, fx + 0x10, depth);
        let (mut dx, mut vx) = (next(b), next(b));
        let v18 = next(b);
        wr16(b, fx + 0x18, v18);
        let v58 = next(b);
        wr16(b, fx + 0x58, v58);
        if mirrored {
            dx = dx.wrapping_neg();
            vx = vx.wrapping_neg();
        }
        let x = rd16(b, fx + 0x08).wrapping_add(dx);
        wr16(b, fx + 0x08, x);
        wr16(b, fx + 0x14, vx);
    }
}

/// Rows of the ground debris of 0x11a36 (the second if `second`).
pub(super) const GROUND_DEBRIS: [u32; 2] = [0x11a9e, 0x11ada];

/// 0x11a36: five pieces of ground debris (depth `A5+0x5aa`), rows of 6 words (x offset, x speed, z speed,
/// `+0x18`, depth offset, `+0x1c`), flipped if `o` faces left; then the sound 0xede08.
pub fn ground_debris(b: &mut Cps2Bus, o: Obj, second: bool) {
    let mut at = GROUND_DEBRIS[second as usize];
    let mut next = |b: &mut Cps2Bus| {
        let v = rd16(b, at);
        at += 2;
        v
    };
    let mirrored = o.u8(b, f::FACING) != 0;
    for _ in 0..5 {
        let Some(fx) = alloc(b, &pools::EFFCT) else { continue };
        debris(b, fx, o.0);
        let ground = rd16(b, A5 + 0x5aa);
        wr16(b, fx + 0x10, ground);
        let (mut dx, mut vx, mut vz) = (next(b), next(b), next(b));
        let v18 = next(b);
        wr16(b, fx + 0x18, v18);
        let ddepth = next(b);
        let v1c = next(b);
        wr16(b, fx + 0x1c, v1c);
        if mirrored {
            dx = dx.wrapping_neg();
            vx = vx.wrapping_neg();
            vz = vz.wrapping_neg();
        }
        let x = rd16(b, fx + 0x08).wrapping_add(dx);
        wr16(b, fx + 0x08, x);
        let v14 = rd16(b, fx + 0x14).wrapping_add(vx);
        wr16(b, fx + 0x14, v14);
        wr16(b, fx + 0x16, vz);
        let depth = rd16(b, fx + 0x10).wrapping_add(ddepth);
        wr16(b, fx + 0x10, depth);
    }
    crate::sound::play(b, crate::sound::CMD_380, o);
}

/// Object dropped only once per type by 0xb648 (kind 2; 0: nothing), and luck masks.
pub(super) const ONCE_VARIANTS: u32 = 0xb6e4;
pub(super) const ONCE_MASKS: u32 = 0xb6fc;

/// Throws an object into the air above `o` (kind `kind`, variant `variant`): speeds ±0x140 in x, 0x300 upwards,
/// `vy` in depth, flipped if `mirrored`.
pub(super) fn toss_item(b: &mut Cps2Bus, o: Obj, item: u32, kind: u8, variant: u8, vy: u16, mirrored: bool) {
    tossed_item(b, o.0, item, kind, variant, vy, mirrored);
}

/// 0xb648: only once per object (`+0x9d`), if the bit drawn in the mask `+0x9c` is set, drops the kind 2 object
/// specific to its type (nothing for a zero variant), thrown to a randomly drawn side.
pub fn drop_once(b: &mut Cps2Bus, o: Obj) {
    if o.u8(b, g::ONCE_DONE) != 0 {
        return;
    }
    let roll = rng::random(b) as u32 & 31;
    let line = o.u8(b, g::ONCE_MASK) as u32;
    let mask = rd32(b, ONCE_MASKS + 4 * line);
    if mask >> roll & 1 == 0 {
        return;
    }
    let kind = o.u8(b, Obj::KIND) as u32;
    let variant = crate::mem::prog8(b, ONCE_VARIANTS + kind);
    if variant == 0 {
        return;
    }
    let Some(item) = alloc(b, &pools::ITEM) else { return };
    o.set_u8(b, g::ONCE_DONE, 1);
    toss_item(b, o, item, 2, variant, (-0x40i16) as u16, false);
    if rng::random(b) & 1 != 0 {
        for field in [Obj::VX, Obj::VY] {
            let v = rd16(b, item + field).wrapping_neg();
            wr16(b, item + field, v);
        }
    }
}

/// Loot by row (`+0x80`, 16 draws) of 0xb780; negative variant: nothing.
pub(super) const LOOT: u32 = 0xb806;

/// 0xb780: loot of `o`, one or two draws depending on the number of players; the second thrown to the other side.
pub fn drop_loot(b: &mut Cps2Bus, o: Obj) {
    let row = o.u8(b, g::LOOT_ROW) as u16;
    if row == 0 {
        return;
    }
    for left in (0..loot_rolls(b)).rev() {
        let roll = (rng::random(b) & 15).wrapping_add(row << 4);
        let variant = crate::mem::prog8(b, LOOT.wrapping_add(offset(roll)));
        if (variant as i8) < 0 {
            return;
        }
        let Some(item) = alloc(b, &pools::ITEM) else { return };
        toss_item(b, o, item, 0, variant, 0x40, left != 0);
    }
}

/// Treasures of 0xe2f6: row (`+0x80`, 16 draws) → list (longs) of kind 5 objects, ended by a negative
/// byte.
pub(super) const TREASURE_ROWS: u32 = 0xe372;
pub(super) const TREASURE_LISTS: u32 = 0xe642;

/// 0xe2f6: treasure of `o` (one or two draws); the objects of a list are numbered from 1 (`+0x2f`) and
/// placed above `o`. Returns the low word of D2 that the original leaves (the row, × 16 if non-zero) and its
/// D5 (the draw counter of the `dbf` loop, 0x0000ffff at the end), if it wrote it.
pub fn drop_treasure(b: &mut Cps2Bus, o: Obj) -> (u16, Option<u32>) {
    let row = o.u8(b, g::LOOT_ROW) as u16;
    if row == 0 {
        return (0, None);
    }
    let mut rank: u8 = 1;
    for left in (0..loot_rolls(b)).rev() {
        let roll = (rng::random(b) & 15).wrapping_add(row << 4);
        let list = crate::mem::prog8(b, TREASURE_ROWS.wrapping_add(offset(roll)));
        if (list as i8) < 0 {
            return (row << 4, Some(left as u32));
        }
        let mut at = rd32(b, TREASURE_LISTS + 4 * list as u32);
        loop {
            let variant = rd8(b, at);
            at += 1;
            if (variant as i8) < 0 {
                break;
            }
            let Some(item) = alloc(b, &pools::ITEM) else { return (row << 4, Some(left as u32)) };
            wr8(b, item, 1);
            wr8(b, item + 2, 5);
            wr8(b, item + 3, variant);
            wr8(b, item + 0x2f, rank);
            rank = rank.wrapping_add(1);
            place_above(b, o.0, item);
        }
    }
    (row << 4, Some(0xffff))
}

/// The players in play and alive (present, non-zero HP), in order, with their class (`+3` / 2).
pub(super) fn party_classes(b: &mut Cps2Bus) -> Vec<u8> {
    let mut classes = Vec::with_capacity(4);
    for p in crate::ram::PLAYERS.iter() {
        if p.u8(b, Obj::ACTIVE) != 0 && p.u16(b, crate::ai::f::HP) != 0 {
            classes.push(p.u8(b, Obj::VARIANT) >> 1);
        }
    }
    classes
}

/// Draws an entry of `table` at `base` + (draw & `mask`) × `scale`.
pub(super) fn special_entry(b: &mut Cps2Bus, table: u32, base: u16, mask: u16, scale: u16) -> u32 {
    let roll = (rng::random(b) & mask).wrapping_mul(scale).wrapping_add(base);
    table.wrapping_add(offset(roll))
}

/// Start of the difficulty row (`A5+0x49`) of `size` bytes.
pub(super) fn difficulty_line(b: &mut Cps2Bus, size: u16) -> u16 {
    rd8(b, A5 + 0x49) as u16 * size
}

/// 0xbad6: special object dropped by `o` (row `+0x80` of the table 0xbf26: kind and variant pairs, 16 draws;
/// negative kind: nothing), one or two draws depending on the number of players. A variant 0 to 3 of kind 0 refers to
/// tables by difficulty (`A5+0x49`), some according to the classes in play (cleric, elf, magician); certain
/// objects are reserved for a group of a single kind of spellcasters (`restrict_draw`). The object is thrown into
/// the air like loot. Returns the low word of D3 that the original leaves if it wrote it (difficulty row, or
/// classes in play).
pub fn drop_special(b: &mut Cps2Bus, o: Obj) -> Option<u16> {
    let row = o.u8(b, g::LOOT_ROW) as u16;
    if row == 0 {
        return None;
    }
    let mut d3 = None;
    let mut left = (crate::ram::PLAYERS_ALIVE.get(b) > 3) as u16;
    let mut step = Draw::Row;
    loop {
        step = match step {
            Draw::Row => {
                let at = special_entry(b, special::ROWS, row << 5, 15, 2);
                let (kind, variant) = (rd8(b, at), rd8(b, at + 1));
                set_drawn(b, kind, variant);
                match (kind as i8, variant) {
                    (k, _) if k < 0 => return d3,
                    (0, 0) => Draw::Common,
                    (0, 1) => Draw::Pair,
                    (0, 2) => Draw::Kind2,
                    (0, _) => Draw::ByClass,
                    _ => Draw::Restrict,
                }
            }
            Draw::Common => {
                let line = difficulty_line(b, 64);
                d3 = Some(line);
                let at = special_entry(b, special::COMMON, line, 31, 1);
                let caster = party_classes(b).iter().any(|&k| matches!(k, CLERIC | ELF | MAGE));
                if caster {
                    let variant = rd8(b, at);
                    set_drawn(b, 7, variant);
                    Draw::Restrict
                } else {
                    redraw(b, at + 32, Draw::Pair, Draw::Restrict)
                }
            }
            Draw::Pair => {
                let line = difficulty_line(b, 64);
                d3 = Some(line);
                let at = special_entry(b, special::PAIRS, line, 31, 2);
                let (kind, variant) = (rd8(b, at), rd8(b, at + 1));
                set_drawn(b, kind, variant);
                Draw::Restrict
            }
            Draw::Kind2 => {
                let line = difficulty_line(b, 16);
                d3 = Some(line);
                let at = special_entry(b, special::KIND_2, line, 15, 1);
                let variant = rd8(b, at);
                set_drawn(b, 2, variant);
                Draw::Restrict
            }
            Draw::ByClass => {
                let line = difficulty_line(b, 64);
                let at = special_entry(b, special::BY_CLASS, line, 15, 1);
                let classes = party_classes(b).iter().fold(0u8, |bits, &k| {
                    bits | match k {
                        CLERIC => 1,
                        ELF => 2,
                        MAGE => 4,
                        _ => 0,
                    }
                });
                d3 = Some(classes as u16);
                if classes == 4 {
                    let variant = rd8(b, at);
                    set_drawn(b, 3, variant);
                    Draw::Restrict
                } else {
                    let shift = match classes {
                        0 => 48,
                        1 => 16,
                        _ => 32,
                    };
                    redraw(b, at + shift, Draw::Common, Draw::Pair)
                }
            }
            Draw::Restrict => restrict_draw(b, &mut d3),
            Draw::Spawn => {
                let (kind, variant) = drawn(b);
                let Some(item) = alloc(b, &pools::ITEM) else { return d3 };
                toss_item(b, o, item, kind, variant, 0x40, left == 0);
                Draw::Next
            }
            Draw::Next => {
                if left == 0 {
                    return d3;
                }
                left -= 1;
                Draw::Row
            }
        }
    }
}

/// 0xbcb4: reserved objects. Kind 3 variant 5: clerics and magicians only (table 0xd776); kind 4 variant
/// 15: magicians only (0xda56); kind 4 variants 8 and 9: clerics only (0xdbc6); kind 4 variants 10
/// and 11: according to the magicians and clerics in play (0xdd36). A group that does not fit keeps the object drawn.
pub(super) fn restrict_draw(b: &mut Cps2Bus, d3: &mut Option<u16>) -> Draw {
    let (kind, variant) = drawn(b);
    match (kind, variant) {
        (3, 5) => {
            let classes = party_classes(b);
            // the original's loop stops at the first player that is neither cleric nor magician
            let other = classes.iter().any(|&k| k != CLERIC && k != MAGE);
            let mut bits = 0u8;
            for &k in classes.iter().take_while(|&&k| k == CLERIC || k == MAGE) {
                bits |= if k == CLERIC { 1 } else { 2 };
            }
            if other || bits == 0 {
                return Draw::Spawn;
            }
            let shift = if bits == 1 { 0 } else { 16 };
            let line = difficulty_line(b, 32);
            *d3 = Some(line);
            let at = special_entry(b, special::CLERIC_OR_MAGE, line, 15, 1);
            let variant = rd8(b, at + shift);
            wr8(b, DRAWN_VARIANT, variant);
            match variant {
                0 => Draw::Next,
                1 => Draw::Common,
                2 => Draw::Pair,
                3 => Draw::ByClass,
                _ => Draw::Spawn,
            }
        }
        (4, 15) => only_class_draw(b, d3, MAGE, special::MAGES),
        (4, 8 | 9) => only_class_draw(b, d3, CLERIC, special::CLERICS),
        (4, 10 | 11) => {
            let bits = party_classes(b).iter().fold(0u8, |bits, &k| {
                bits | if k == MAGE { 1 } else { 0 } | if k == CLERIC { 2 } else { 0 }
            });
            let shift = match bits {
                0 => return Draw::Spawn,
                1 => 0,
                2 => 16,
                _ => 32,
            };
            let line = difficulty_line(b, 16) + shift;
            *d3 = Some(line);
            let at = special_entry(b, special::MAGE_OR_CLERIC, line, 15, 1);
            reroll_or_none(b, at)
        }
        _ => Draw::Spawn,
    }
}

/// All the players in play are of class `class`: new kind 4 variant drawn in `table`.
pub(super) fn only_class_draw(b: &mut Cps2Bus, d3: &mut Option<u16>, class: u8, table: u32) -> Draw {
    if party_classes(b).iter().any(|&k| k != class) {
        return Draw::Spawn;
    }
    let line = difficulty_line(b, 16);
    *d3 = Some(line);
    let at = special_entry(b, table, line, 15, 1);
    reroll_or_none(b, at)
}
