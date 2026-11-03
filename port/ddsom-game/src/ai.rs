//! AI helpers common to enemies and bosses (subsystem E1): choosing a player as target, initial hit points
//! and immunities, pseudo-random table-driven decisions, position tests, keeping on screen.
//!
//! Each original routine is an ordinary Rust function on the processed object (`Obj`, the A0 of the original): its
//! inputs as arguments, its result as a Rust value. The handlers (`families::enemy_lib`) call them
//! directly; none reproduces the registers of the original any more (proof: `DDSOM_POISON`, see
//! `docs/refactorisation.md`, AI batch).
//!
//! Players: 4 records of 0x200 bytes at A5+0x5de (`ram::PLAYERS`); a player is "targetable" if it is
//! active (`+0`), of main state < 4 (`+4`), and not masked (bit 1 of `+1`), with a rule on `+0x182`
//! ([`Rule`]).

use cps2::Cps2Bus;

use crate::objects::Obj;
use crate::player::Player;
use crate::ram::{self, Word, LEVEL, PLAYERS, PLAYERS_ALIVE};
use crate::rng;
use crate::tables::{engine, Table};

/// Rank (dynamic difficulty 0..31, `objects::rank`): it indexes the decision and HP tables.
pub const RANK: Word = Word::a5(0x7f92);

/// Enemy fields read or written by these helpers.
pub mod f {
    /// Target: low word of the address of a player or an object (0: none).
    pub const TARGET: u32 = 0x50;
    /// Orientation: 0 faces right, 0x20 left.
    pub const FLIP: u32 = 0x2c;
    /// Heading (0..31, index of the speed table: `Obj::SPEED_INDEX`).
    pub const HEADING: u32 = 0x2d;
    /// Hit points: maximum, current, previous.
    pub const HP_MAX: u32 = 0x60;
    pub const HP: u32 = 0x62;
    pub const HP_PREV: u32 = 0x64;
    /// Rows of the mask decision tables (0xa4b4, 0xa4c2).
    pub const DECISION_A: u32 = 0x72;
    pub const DECISION_B: u32 = 0x73;
    /// Row of the byte table by rank and drawn byte (0xac3a).
    pub const BYTE_TABLE: u32 = 0x76;
    pub const BYTE: u32 = 0x77;
    /// Immunities (long, bits per kind of attack).
    pub const IMMUNITIES: u32 = 0x94;
    /// HP rank: (players alive - 1) × 32 + even rank (column of the threshold tables).
    pub const HP_RANK: u32 = 0xac;
}

// ---------------------------------------------------------------------------------------------
// Choosing a player as target (0x424e, 0x43ae, 0x43bc, 0x44ee)

/// Eligibility rule of a player.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Rule {
    /// 0x424e, 0x44ee: non-zero `+0x182` (aura) excludes the player.
    Strict,
    /// 0x43ae, 0x43bc: non-zero `+0x182` exempts from the "masked" test.
    Lenient,
}

/// Criterion for choosing among the targetable players (D0 of 0x43bc: 0, 2, 4).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Criterion {
    /// The nearest (|dx| + |dy|); on a tie, the first.
    Nearest,
    /// The one with the fewest HP; on a tie, the last.
    LowestHp,
    /// The farthest; on a tie, the last.
    Farthest,
}

impl Criterion {
    /// Criterion byte of the original's tables (0, 2 or 4).
    pub fn from_byte(v: u8) -> Criterion {
        match v {
            0 => Criterion::Nearest,
            2 => Criterion::LowestHp,
            4 => Criterion::Farthest,
            _ => unreachable!("target criterion {v}: the tables only have 0, 2 and 4"),
        }
    }
}

/// Can player `p` be targeted?
pub fn targetable(b: &mut Cps2Bus, p: Obj, rule: Rule) -> bool {
    let aura = p.u8(b, Player::AURA) != 0;
    let hidden = p.u8(b, Player::FLAGS) & 2 != 0;
    let visible = match rule {
        Rule::Strict => !aura && !hidden,
        Rule::Lenient => aura || !hidden,
    };
    p.u8(b, Player::ACTIVE) != 0 && visible && (p.u8(b, Player::MAIN) as i8) < 4
}

/// Distance |dx| + |dy| (words, like the original: `sub`, `neg` if negative, `add`).
fn manhattan(b: &mut Cps2Bus, a: Obj, other: Obj) -> u16 {
    let dx = other.u16(b, Obj::X).wrapping_sub(a.u16(b, Obj::X)) as i16;
    let dy = other.u16(b, Obj::Y).wrapping_sub(a.u16(b, Obj::Y)) as i16;
    (dx.wrapping_abs() as u16).wrapping_add(dy.wrapping_abs() as u16)
}

/// Body of 0x427a / 0x42e0 / 0x4338 (strict rule) and 0x43ca / 0x4430 / 0x4488 (wide rule): the player who
/// best meets the criterion becomes the target of `o` (`+0x50`, 0 if none).
pub fn target_by(b: &mut Cps2Bus, o: Obj, criterion: Criterion, rule: Rule) -> Option<Obj> {
    let mut best: u16 = match criterion {
        Criterion::Nearest => 0xffff,
        Criterion::LowestHp => 0x7fff,
        Criterion::Farthest => 0,
    };
    let mut found = None;
    for p in PLAYERS.iter() {
        if !targetable(b, p, rule) {
            continue;
        }
        let (value, better) = match criterion {
            Criterion::LowestHp => {
                let hp = p.u16(b, Player::HP);
                (hp, hp as i16 <= best as i16)
            }
            Criterion::Nearest => {
                let d = manhattan(b, o, p);
                (d, d < best)
            }
            Criterion::Farthest => {
                let d = manhattan(b, o, p);
                (d, d >= best)
            }
        };
        if better {
            best = value;
            found = Some(p);
        }
    }
    o.set_u16(b, f::TARGET, found.map_or(0, Obj::word));
    found
}

/// Table of randomly drawn criteria of a rule (16 bytes: 10 times the nearest, 4 the fewest HP, 2 the farthest).
fn criteria(rule: Rule) -> &'static Table {
    engine().get(match rule {
        Rule::Strict => "ai/criteria_strict",
        Rule::Lenient => "ai/criteria_lenient",
    })
}

/// 0x424e (strict rule) / 0x43ae (wide rule): randomly drawn criterion, then `target_by`.
pub fn pick_target(b: &mut Cps2Bus, o: Obj, rule: Rule) -> Option<Obj> {
    let roll = rng::random(b) & 15;
    let criterion = Criterion::from_byte(criteria(rule).at8(b, roll as u32));
    target_by(b, o, criterion, rule)
}

/// 0x44ee: a targetable player (strict rule) drawn at random becomes the target: three tries, each on the player
/// following the previous one. With no player alive, neither a draw nor a write.
pub fn random_player(b: &mut Cps2Bus, o: Obj) -> Option<Obj> {
    if PLAYERS_ALIVE.get(b) == 0 {
        return None;
    }
    let mut k = rng::random(b);
    for _ in 0..3 {
        k = k.wrapping_add(1) & 3;
        let p = PLAYERS.nth(k as u32);
        if targetable(b, p, Rule::Strict) {
            o.set_u16(b, f::TARGET, p.word());
            return Some(p);
        }
    }
    None
}

// ---------------------------------------------------------------------------------------------
// Initial values: HP (0x453c, 0x4544), immunities (0xfe5a), HP rank (0x1059e)

/// Initial HP tables: bosses (0x453c) and enemies (0x4544). Words, by type (256 bytes), level, players, rank.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HpTable {
    Boss,
    Enemy,
}

impl HpTable {
    fn table(self) -> &'static Table {
        engine().get(match self {
            HpTable::Boss => "hp_boss",
            HpTable::Enemy => "hp_enemy",
        })
    }
}

/// 0x453c / 0x4544: initial HP of `o` read from `table` (block of type `+2`; second half beyond level 5;
/// row of the number of players alive; column of rank / 2), copied to `+0x60`, `+0x62` and `+0x64`.
pub fn initial_hp(b: &mut Cps2Bus, o: Obj, table: HpTable) {
    let mut index = (o.u8(b, Obj::KIND) as u16) << 8;
    if LEVEL.get(b) > 5 {
        index = index.wrapping_add(0x80);
    }
    index = index.wrapping_add(((PLAYERS_ALIVE.get(b).wrapping_sub(1) & 3) as u16) << 5);
    index = index.wrapping_add((RANK.get(b) >> 1 & 15) * 2);
    let hp = table.table().at16(b, index as i16 as u32);
    for field in [f::HP_MAX, f::HP, f::HP_PREV] {
        o.set_u16(b, field, hp);
    }
}

/// 0xfe5a: immunities of `o` (`+0x94`) according to its class and type (table `immunities`, longs: enemies, class 2,
/// then bosses, class 4, 32 types further). Returns the index (bytes) read in the table.
pub fn set_immunities(b: &mut Cps2Bus, o: Obj) -> u16 {
    let base: u8 = if o.u8(b, Obj::CLASS) == 4 { 32 } else { 0 };
    let index = base.wrapping_add(o.u8(b, Obj::KIND)) as u16 * 4;
    let v = engine().get("immunities").at32(b, index as u32);
    o.set_u32(b, f::IMMUNITIES, v);
    index
}

/// 0x1059e: HP rank of `o` (`+0xac`) = (players alive - 1) × 32 + (rank & 0x1e), on one byte.
pub fn hp_rank(b: &mut Cps2Bus, o: Obj) {
    let players = PLAYERS_ALIVE.get(b).wrapping_sub(1) << 5;
    let v = players.wrapping_add(RANK.get(b) as u8 & 0x1e);
    o.set_u8(b, f::HP_RANK, v);
}

// ---------------------------------------------------------------------------------------------
// Table-driven decisions (0xa4b4, 0xa4c2, 0xac3a)

/// Mask decision: row `+0x72` (`decision_a`, 0xa4b4) or `+0x73` (`decision_b`, 0xa4c2) gives a
/// table of 32-bit masks indexed by rank; a bit in it is drawn at random. True if it is set.
fn decision(b: &mut Cps2Bus, o: Obj, field: u32, table: &str) -> bool {
    let line = (o.u8(b, field) as u16) << 2;
    let masks = engine().get(table).at32(b, line as i16 as u32);
    let rank = RANK.get(b);
    let mask = engine().long(b, masks.wrapping_add((rank << 2) as i16 as u32));
    let bit = rng::random(b) & 31;
    mask >> bit & 1 != 0
}

/// 0xa4b4: does the AI act on this frame (row `+0x72`)?
pub fn decision_a(b: &mut Cps2Bus, o: Obj) -> bool {
    decision(b, o, f::DECISION_A, "ai/decisions_a")
}

/// 0xa4c2: same with row `+0x73`.
pub fn decision_b(b: &mut Cps2Bus, o: Obj) -> bool {
    decision(b, o, f::DECISION_B, "ai/decisions_b")
}

/// 0xac3a: `+0x77` = byte of the table of row `+0x76` (`ai/bytes_by_rank`), at the rank index.
pub fn pick_byte(b: &mut Cps2Bus, o: Obj) {
    let line = (o.u8(b, f::BYTE_TABLE) as u16) << 2;
    let table = engine().get("ai/bytes_by_rank").at32(b, line as i16 as u32);
    let rank = RANK.get(b);
    let v = engine().byte(b, table.wrapping_add(rank as i16 as u32));
    o.set_u8(b, f::BYTE, v);
}

// ---------------------------------------------------------------------------------------------
// Position of the target (0xb5e0, 0x1050c, 0x1052e, 0x104e0, 0x1047a, 0x10560)

/// The target of `o` (`+0x50`), if designated by a negative word (an object in high RAM).
fn linked_target(b: &mut Cps2Bus, o: Obj) -> Option<Obj> {
    let w = o.u16(b, f::TARGET);
    ((w as i16) < 0).then(|| Obj::from_word(w))
}

/// Orientation looking from `o` towards `other`: 0x20 (left) if `other` is to the left (`o.x - other.x` > 0
/// is false for a negative difference: the original tests the sign of the subtraction).
fn side_of(b: &mut Cps2Bus, o: Obj, other: Obj) -> u8 {
    if (o.u16(b, Obj::X).wrapping_sub(other.u16(b, Obj::X)) as i16) < 0 {
        0x20
    } else {
        0
    }
}

/// 0xb5e0: is the target of `o` (`+0x50`, on the ground) inside the box `hitbox` in front of `o` (0x7f19c on the
/// target)?
pub fn target_in_front(b: &mut Cps2Bus, o: Obj, hitbox: crate::ai_aids::Hitbox) -> crate::ai_aids::InHitbox {
    match o.u16(b, f::TARGET) {
        0 => crate::ai_aids::InHitbox::No,
        w => crate::ai_aids::box_around(b, o, Obj::from_word(w), hitbox),
    }
}

/// 0x1050c: is `o` facing its target? True also without a target (positive word).
pub fn facing_target(b: &mut Cps2Bus, o: Obj) -> bool {
    let Some(t) = linked_target(b, o) else { return true };
    side_of(b, o, t) == o.u8(b, f::FLIP)
}

/// 0x1052e: is the target facing `o`?
pub fn target_faces_me(b: &mut Cps2Bus, o: Obj) -> bool {
    let Some(t) = linked_target(b, o) else { return false };
    let gap = o.u16(b, Obj::X).wrapping_sub(t.u16(b, Obj::X)) & 0x8000;
    let flip = (o.u8(b, f::FLIP) as u16).rotate_right(6);
    let wanted = if (flip & gap) as i16 >= 0 { 0x20 } else { 0 };
    wanted == t.u8(b, f::FLIP)
}

/// 0x104e0 (Z clear): the target faces the same way as `o`, which has its back turned to it.
pub fn back_to_target(b: &mut Cps2Bus, o: Obj) -> bool {
    let Some(t) = linked_target(b, o) else { return false };
    let flip = o.u8(b, f::FLIP);
    if flip != t.u8(b, f::FLIP) {
        return false;
    }
    let wanted = if side_of(b, o, t) == 0x20 { 0 } else { 0x20 };
    wanted != flip
}

/// Gesture chosen by `turn_decision` (D0 of 0x1047a).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Turn {
    None,
    /// Face to face, decision A accepted.
    FaceToFace,
    /// Same side, decision B accepted.
    SameSide,
}

/// 0x1047a: AI decision according to the side of the target.
pub fn turn_decision(b: &mut Cps2Bus, o: Obj) -> Turn {
    if target_faces_me(b, o) {
        if decision_a(b, o) {
            Turn::FaceToFace
        } else {
            Turn::None
        }
    } else if back_to_target(b, o) && decision_b(b, o) {
        Turn::SameSide
    } else {
        Turn::None
    }
}

/// 0x10560: is the direction of the X speed (`+0x14`) that of the orientation?
pub fn moving_forward(b: &mut Cps2Bus, o: Obj) -> bool {
    let flip = (o.u8(b, f::FLIP) as u16).rotate_right(6);
    o.u16(b, Obj::VX) & 0x8000 == flip
}

// ---------------------------------------------------------------------------------------------
// Staying on screen (0xfbae, 0xfbb0)

/// Margin of 0xfbae (the handlers' end-of-frame zone test).
pub const ZONE_MARGIN: u16 = 0x30;

/// Brings an axis (`field`) of `o` back into the window `[origin - margin, origin + span + margin]`: a negative
/// position becomes 0, a position outside the window is set to the nearest edge. True if it changed.
fn pull_inside(b: &mut Cps2Bus, o: Obj, field: u32, origin: u16, span: u16, margin: u16) -> bool {
    let pos = o.u16(b, field);
    if (pos as i16) < 0 {
        o.set_u16(b, field, 0);
        return true;
    }
    let offset = pos.wrapping_sub(origin).wrapping_add(margin);
    if offset <= span.wrapping_add(margin).wrapping_add(margin) {
        return false;
    }
    let edge = if (offset as i16) < 0 { margin.wrapping_neg() } else { span.wrapping_add(margin) };
    o.set_u16(b, field, origin.wrapping_add(edge));
    true
}

/// 0xfbb0: keeps `o` within the screen of the active camera, to within `margin` pixels in X (16 pixels less than the
/// screen height in depth). True if it was moved.
pub fn keep_on_screen(b: &mut Cps2Bus, o: Obj, margin: u16) -> bool {
    let (x, y) = ram::camera(b);
    let moved_x = pull_inside(b, o, Obj::X, x, 0x180, margin);
    let moved_y = pull_inside(b, o, Obj::Y, y, 0x100, (-16i16) as u16);
    moved_x | moved_y
}

/// 0xfbae: `keep_on_screen` with the margin of 0x30 pixels.
pub fn keep_in_zone(b: &mut Cps2Bus, o: Obj) -> bool {
    keep_on_screen(b, o, ZONE_MARGIN)
}

// ---------------------------------------------------------------------------------------------
// Heading (0xf0e8, 0x10574)

/// 0xf0e8: turns the heading of `o` (`+0x2d`, 0..31) by one notch towards the point `to` (direction of 0x137e), by the
/// shortest way.
pub fn turn_toward(b: &mut Cps2Bus, o: Obj, to: (u16, u16)) {
    let wanted = crate::aids::direction_from(b, o, to);
    let heading = o.u8(b, f::HEADING);
    let diff = wanted.wrapping_sub(heading) & 0x1f;
    if diff == 0 {
        return;
    }
    let turned = if diff & 0x10 == 0 { heading.wrapping_add(1) } else { heading.wrapping_sub(1) };
    o.set_u8(b, f::HEADING, turned & 0x1f);
}

/// 0x10574: the heading of `o` changes by at most one notch, drawn at random (`ai/drift_steps`, signed bytes).
pub fn drift_heading(b: &mut Cps2Bus, o: Obj) {
    let roll = rng::random(b) & 15;
    let step = engine().get("ai/drift_steps").at8(b, roll as u32);
    let heading = o.u8(b, f::HEADING).wrapping_add(step) & 0x1f;
    o.set_u8(b, f::HEADING, heading);
}
