//! Debris and loot (0x1172e, 0x11a36, 0xb648, 0xb780, 0xbad6, 0xe2f6).

use super::*;

use crate::ram::{Byte, LEVEL_ROW, PLAYERS_ALIVE};
use crate::record::record;

/// A debris effect (EFFCT of type 0x32) created by `debris_set` and `ground_debris`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Debris(pub Obj);

impl Debris {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Debris {
        /// Bounce of the piece (h), from the debris rows.
        bounce, set_bounce, BOUNCE: u16 @ 0x58;
    }
}

/// An object of a treasure list (ITEM of kind 5) created by `drop_treasure`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Treasure(pub Obj);

impl Treasure {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Treasure {
        /// Number of the object in the treasure dropped, from 1.
        order, set_order, ORDER: u8 @ 0x2f;
    }
}

/// Starts object `o` (active) as kind `kind`, variant `variant`.
fn start(b: &mut Cps2Bus, o: Obj, kind: u8, variant: u8) {
    o.set_active(b, 1);
    o.set_kind(b, kind);
    o.set_variant(b, variant);
}

/// Copies the position of `from` to `to`, set 16 pixels higher and just in front.
fn place_above(b: &mut Cps2Bus, from: Obj, to: Obj) {
    for field in [Obj::X, Obj::Z, Obj::Y] {
        let v = from.u16(b, field);
        to.set_u16(b, field, v);
    }
    let z = to.u16(b, Obj::Z).wrapping_add(0x10);
    to.set_u16(b, Obj::Z, z);
    let depth = to.u16(b, Obj::Y).wrapping_sub(1);
    to.set_u16(b, Obj::Y, depth);
}

/// Debris (EFFCT of type 0x32, variant 0xc) at the abscissa of `me`; its speeds come from the rows of the caller.
fn debris(b: &mut Cps2Bus, fx: Obj, me: Obj) {
    start(b, fx, 0x32, 0xc);
    let x = me.u16(b, Obj::X);
    fx.set_u16(b, Obj::X, x);
}

/// Throws an object into the air above `o` (kind `kind`, variant `variant`): speeds ±0x140 in x, 0x300 upwards,
/// `vy` in depth, flipped if `mirrored`.
fn toss_item(b: &mut Cps2Bus, o: Obj, item: Obj, kind: u8, variant: u8, vy: u16, mirrored: bool) {
    start(b, item, kind, variant);
    place_above(b, o, item);
    item.set_vz(b, 0x300);
    item.set_az(b, (-0x28i16) as u16);
    item.set_vx(b, 0x140);
    item.set_vy(b, vy);
    if mirrored {
        flip_speeds(b, item);
    }
}

/// The x and depth speeds of `item` change sign.
fn flip_speeds(b: &mut Cps2Bus, item: Obj) {
    let vx = item.vx(b).wrapping_neg();
    item.set_vx(b, vx);
    let vy = item.vy(b).wrapping_neg();
    item.set_vy(b, vy);
}

/// Number of loot draws: two with more than three players alive, otherwise one.
fn loot_rolls(b: &mut Cps2Bus) -> u16 {
    if PLAYERS_ALIVE.get(b) > 3 {
        2
    } else {
        1
    }
}

/// Special object drawn by 0xbad6: kind and variant (`A5-0x7196`, `A5-0x7195`).
const DRAWN_KIND: Byte = Byte(0xff0e6a);
const DRAWN_VARIANT: Byte = Byte(0xff0e6b);

/// Character classes (`+3` / 2 of a player) that count for the special loot.
const CLERIC: u8 = 1;
const ELF: u8 = 2;
const MAGE: u8 = 4;

/// Steps of the 0xbad6 draw (the labels of the original).
#[derive(Clone, Copy)]
enum Draw {
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

fn drawn(b: &mut Cps2Bus) -> (u8, u8) {
    (DRAWN_KIND.get(b), DRAWN_VARIANT.get(b))
}
fn set_drawn(b: &mut Cps2Bus, kind: u8, variant: u8) {
    DRAWN_KIND.set(b, kind);
    DRAWN_VARIANT.set(b, variant);
}
/// New kind 4 variant drawn in `at`: 0 cancels the draw, 1 and 2 refer back to the draws `on_1`, `on_2`.
fn redraw(b: &mut Cps2Bus, at: u32, on_1: Draw, on_2: Draw) -> Draw {
    let variant = engine().byte(b, at);
    set_drawn(b, 4, variant);
    match variant {
        0 => Draw::Next,
        1 => on_1,
        2 => on_2,
        _ => Draw::Restrict,
    }
}

fn reroll_or_none(b: &mut Cps2Bus, at: u32) -> Draw {
    let variant = engine().byte(b, at);
    set_drawn(b, 4, variant);
    if variant == 0 { Draw::Next } else { Draw::Spawn }
}

/// 0x1172e: debris set no. `set` (`ai/debris_sets`: number - 1, then rows of 4 words: x offset, x speed, `+0x18`,
/// `+0x58`) around `o`, flipped if `o` faces left. A row is only read if the effect could be created.
pub fn debris_set(b: &mut Cps2Bus, o: Obj, set: u16) {
    let mut at = crate::tables::engine_table!("ai/debris_sets").at32(b, offset(set.wrapping_mul(4)));
    let mut next = |b: &mut Cps2Bus| {
        let v = engine().word(b, at);
        at += 2;
        v
    };
    let count = next(b);
    let mirrored = o.flip(b) != 0;
    for _ in 0..=count {
        let Some(fx) = pools::EFFCT.alloc(b) else { continue };
        debris(b, fx, o);
        let z = o.u16(b, Obj::Z);
        fx.set_u16(b, Obj::Z, z);
        let depth = o.y(b).wrapping_sub(1);
        fx.set_y(b, depth);
        let (mut dx, mut vx) = (next(b), next(b));
        let vz = next(b);
        fx.set_vz(b, vz);
        let bounce = next(b);
        Debris(fx).set_bounce(b, bounce);
        if mirrored {
            dx = dx.wrapping_neg();
            vx = vx.wrapping_neg();
        }
        let x = fx.u16(b, Obj::X).wrapping_add(dx);
        fx.set_u16(b, Obj::X, x);
        fx.set_vx(b, vx);
    }
}

/// 0x11a36: five pieces of ground debris (depth `ram::CAMERA1_Y`), rows of 6 words (`ai/ground_debris_0`, `_1` if
/// `second`: x offset, x speed, z speed, `+0x18`, depth offset, `+0x1c`), flipped if `o` faces left; then a sound.
pub fn ground_debris(b: &mut Cps2Bus, o: Obj, second: bool) {
    let rows = table(if second { "ai/ground_debris_1" } else { "ai/ground_debris_0" });
    let mut at = 0;
    let mut next = |b: &mut Cps2Bus| {
        let v = rows.at16(b, at);
        at += 2;
        v
    };
    let mirrored = o.flip(b) != 0;
    for _ in 0..5 {
        let Some(fx) = pools::EFFCT.alloc(b) else { continue };
        debris(b, fx, o);
        let ground = crate::ram::CAMERA1_Y.get(b);
        fx.set_u16(b, Obj::Y, ground);
        let (mut dx, mut vx, mut az) = (next(b), next(b), next(b));
        let vz = next(b);
        fx.set_vz(b, vz);
        let ddepth = next(b);
        let vy = next(b);
        fx.set_vy(b, vy);
        if mirrored {
            dx = dx.wrapping_neg();
            vx = vx.wrapping_neg();
            az = az.wrapping_neg();
        }
        let x = fx.u16(b, Obj::X).wrapping_add(dx);
        fx.set_u16(b, Obj::X, x);
        let v = fx.vx(b).wrapping_add(vx);
        fx.set_vx(b, v);
        fx.set_ax(b, az);
        let depth = fx.u16(b, Obj::Y).wrapping_add(ddepth);
        fx.set_u16(b, Obj::Y, depth);
    }
    crate::sound::play(b, crate::sound::CMD_380, o);
}

/// 0xb648: only once per object (`+0x9d`), if the bit drawn in the mask `+0x9c` is set, drops the kind 2 object
/// specific to its type (`ai/once_variants`, luck masks `ai/once_masks`; nothing for a zero variant), thrown to a
/// randomly drawn side.
pub fn drop_once(b: &mut Cps2Bus, o: Obj) {
    let e = Enemy(o);
    if e.once_done(b) != 0 {
        return;
    }
    let roll = rng::random(b) as u32 & 31;
    let line = e.once_mask(b) as u32;
    let mask = crate::tables::engine_table!("ai/once_masks").nth32(b, line);
    if mask >> roll & 1 == 0 {
        return;
    }
    let kind = o.kind(b) as u32;
    let variant = crate::tables::engine_table!("ai/once_variants").at8(b, kind);
    if variant == 0 {
        return;
    }
    let Some(item) = pools::ITEM.alloc(b) else { return };
    e.set_once_done(b, 1);
    toss_item(b, o, item, 2, variant, (-0x40i16) as u16, false);
    if rng::random(b) & 1 != 0 {
        flip_speeds(b, item);
    }
}

/// 0xb780: loot of `o` (`ai/loot`, 16 draws per row `+0x80`; negative variant: nothing), one or two draws
/// depending on the number of players; the second thrown to the other side.
pub fn drop_loot(b: &mut Cps2Bus, o: Obj) {
    let row = Enemy(o).loot_row(b) as u16;
    if row == 0 {
        return;
    }
    for left in (0..loot_rolls(b)).rev() {
        let roll = (rng::random(b) & 15).wrapping_add(row << 4);
        let variant = crate::tables::engine_table!("ai/loot").at8(b, offset(roll));
        if (variant as i8) < 0 {
            return;
        }
        let Some(item) = pools::ITEM.alloc(b) else { return };
        toss_item(b, o, item, 0, variant, 0x40, left != 0);
    }
}

/// 0xe2f6: treasure of `o` (`ai/treasure_rows`: 16 draws per row `+0x80`, the number of a list of kind 5 objects of
/// `ai/treasure_lists`, ended by a negative byte; one or two draws); the objects of a list are numbered from 1 (`+0x2f`) and
/// placed above `o`. Returns the low word of D2 that the original leaves (the row, × 16 if non-zero) and its
/// D5 (the draw counter of the `dbf` loop, 0x0000ffff at the end), if it wrote it.
pub fn drop_treasure(b: &mut Cps2Bus, o: Obj) -> (u16, Option<u32>) {
    let row = Enemy(o).loot_row(b) as u16;
    if row == 0 {
        return (0, None);
    }
    let mut rank: u8 = 1;
    for left in (0..loot_rolls(b)).rev() {
        let roll = (rng::random(b) & 15).wrapping_add(row << 4);
        let list = crate::tables::engine_table!("ai/treasure_rows").at8(b, offset(roll));
        if (list as i8) < 0 {
            return (row << 4, Some(left as u32));
        }
        let mut at = crate::tables::engine_table!("ai/treasure_lists").nth32(b, list as u32);
        loop {
            let variant = engine().byte(b, at);
            at += 1;
            if (variant as i8) < 0 {
                break;
            }
            let Some(item) = pools::ITEM.alloc(b) else { return (row << 4, Some(left as u32)) };
            start(b, item, 5, variant);
            Treasure(item).set_order(b, rank);
            rank = rank.wrapping_add(1);
            place_above(b, o, item);
        }
    }
    (row << 4, Some(0xffff))
}

/// The players in play and alive (present, non-zero HP), in order, with their class (`+3` / 2).
fn party_classes(b: &mut Cps2Bus) -> Vec<u8> {
    let mut classes = Vec::with_capacity(4);
    for p in crate::ram::PLAYERS.iter().map(Player::of) {
        if p.active(b) != 0 && p.hp(b) != 0 {
            classes.push(p.character(b) >> 1);
        }
    }
    classes
}

/// Draws an entry of `table` at `base` + (draw & `mask`) × `scale`.
fn special_entry(b: &mut Cps2Bus, table: u32, base: u16, mask: u16, scale: u16) -> u32 {
    let roll = (rng::random(b) & mask).wrapping_mul(scale).wrapping_add(base);
    table.wrapping_add(offset(roll))
}

/// Start of the difficulty row (`ram::LEVEL_ROW`) of `size` bytes.
fn difficulty_line(b: &mut Cps2Bus, size: u16) -> u16 {
    LEVEL_ROW.get(b) as u16 * size
}

/// 0xbad6: special object dropped by `o` (row `+0x80` of `ai/special_rows`: kind and variant pairs, 16 draws;
/// negative kind: nothing), one or two draws depending on the number of players. A variant 0 to 3 of kind 0 refers to
/// tables by difficulty (`ram::LEVEL_ROW`), some according to the classes in play (cleric, elf, magician); certain
/// objects are reserved for a group of a single kind of spellcasters (`restrict_draw`). The object is thrown into
/// the air like loot. Returns the low word of D3 that the original leaves if it wrote it (difficulty row, or
/// classes in play).
pub fn drop_special(b: &mut Cps2Bus, o: Obj) -> Option<u16> {
    let row = Enemy(o).loot_row(b) as u16;
    if row == 0 {
        return None;
    }
    let mut d3 = None;
    let mut left = (PLAYERS_ALIVE.get(b) > 3) as u16;
    let mut step = Draw::Row;
    loop {
        step = match step {
            Draw::Row => {
                let at = special_entry(b, crate::tables::engine_table!("ai/special_rows").rom, row << 5, 15, 2);
                let (kind, variant) = (engine().byte(b, at), engine().byte(b, at + 1));
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
                let at = special_entry(b, crate::tables::engine_table!("ai/special_common").rom, line, 31, 1);
                let caster = party_classes(b).iter().any(|&k| matches!(k, CLERIC | ELF | MAGE));
                if caster {
                    let variant = engine().byte(b, at);
                    set_drawn(b, 7, variant);
                    Draw::Restrict
                } else {
                    redraw(b, at + 32, Draw::Pair, Draw::Restrict)
                }
            }
            Draw::Pair => {
                let line = difficulty_line(b, 64);
                d3 = Some(line);
                let at = special_entry(b, crate::tables::engine_table!("ai/special_pairs").rom, line, 31, 2);
                let (kind, variant) = (engine().byte(b, at), engine().byte(b, at + 1));
                set_drawn(b, kind, variant);
                Draw::Restrict
            }
            Draw::Kind2 => {
                let line = difficulty_line(b, 16);
                d3 = Some(line);
                let at = special_entry(b, crate::tables::engine_table!("ai/special_kind_2").rom, line, 15, 1);
                let variant = engine().byte(b, at);
                set_drawn(b, 2, variant);
                Draw::Restrict
            }
            Draw::ByClass => {
                let line = difficulty_line(b, 64);
                let at = special_entry(b, crate::tables::engine_table!("ai/special_by_class").rom, line, 15, 1);
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
                    let variant = engine().byte(b, at);
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
                let Some(item) = pools::ITEM.alloc(b) else { return d3 };
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
fn restrict_draw(b: &mut Cps2Bus, d3: &mut Option<u16>) -> Draw {
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
            let at = special_entry(b, crate::tables::engine_table!("ai/special_cleric_or_mage").rom, line, 15, 1);
            let variant = engine().byte(b, at + shift);
            DRAWN_VARIANT.set(b, variant);
            match variant {
                0 => Draw::Next,
                1 => Draw::Common,
                2 => Draw::Pair,
                3 => Draw::ByClass,
                _ => Draw::Spawn,
            }
        }
        (4, 15) => only_class_draw(b, d3, MAGE, crate::tables::engine_table!("ai/special_mages")),
        (4, 8 | 9) => only_class_draw(b, d3, CLERIC, crate::tables::engine_table!("ai/special_clerics")),
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
            let at = special_entry(b, crate::tables::engine_table!("ai/special_mage_or_cleric").rom, line, 15, 1);
            reroll_or_none(b, at)
        }
        _ => Draw::Spawn,
    }
}

/// All the players in play are of class `class`: new kind 4 variant drawn in `table`.
fn only_class_draw(b: &mut Cps2Bus, d3: &mut Option<u16>, class: u8, rows: &Table) -> Draw {
    if party_classes(b).iter().any(|&k| k != class) {
        return Draw::Spawn;
    }
    let line = difficulty_line(b, 16);
    *d3 = Some(line);
    let at = special_entry(b, rows.rom, line, 15, 1);
    reroll_or_none(b, at)
}
