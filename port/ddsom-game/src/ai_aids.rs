//! AI helpers for enemies and bosses (continuation of `ai`): stance draws from tables, test of the
//! box in front of an opponent.
//!
//! These routines keep the contract of the originals: a result in D0 (1: yes, 0: no) and the
//! flags of a `moveq`, which the handlers test right after the call (`beq`).

use cps2::Cps2Bus;
use m68k::decode::{Fetch, Size};
use m68k::Cpu;

use crate::mem::{rd16, rd32, rd8, wr16, wr32, wr8, A5};
use crate::ops::logic;
use crate::objects::Obj;
use crate::rng;
use crate::objects::pools;

/// Counter of the current frame, used as an index in the stance tables.
const TICK: u32 = A5 + 0x7f92;
/// State of the pseudo-random generator.

/// Fields of the enemy (A0).
mod f {
    pub const X: u32 = 0x08;
    pub const DEPTH: u32 = 0x10;
    pub const FACING: u32 = 0x2c;
    /// Targeted object (16-bit pointer).
    pub const TARGET: u32 = 0x50;
    pub const STATE: u32 = 0x05;
    pub const SUB_STATE: u32 = 0x06;
    pub const SUB_STEP: u32 = 0x07;
    /// Table numbers: stance (0x70), timing (0x72, 0x73), attack box (0x74, 0x75).
    pub const STANCE_TABLE: u32 = 0x70;
    pub const STANCE_VALUE: u32 = 0x71;
    pub const BOX_CHANCE: u32 = 0x74;
    pub const BOX_ANIMATION: u32 = 0x75;
}

/// Pointer tables in ROM (one long per table number).
const STANCE_POINTERS: u32 = 0xa48c;
const BOX_CHANCE_POINTERS: u32 = 0x7fec;
const BOX_ANIMATION_POINTERS: u32 = 0x800c;

/// `moveq #v,D0`: D0 and the flags N, Z (V and C zero).
fn moveq(c: &mut Cpu, v: i32) {
    c.d[0] = v as u32;
    logic(c, v as u32, Size::Long);
}

/// Word of the pointer table `table` designated by `index`, sign-extended as a `Dn.w` index
/// does on the 68000.
fn offset(index: u16) -> u32 {
    index as i16 as i32 as u32
}


/// 0xa470: draws the stance of enemy `o` (byte of the table `+0x70`, indexed by the frame counter), stores it
/// at `+0x71` and returns it (the callers' `beq` tests whether it is zero).
pub fn stance(b: &mut Cps2Bus, o: Obj) -> u8 {
    stance_at(b, o).1
}

/// (table read, stance).
fn stance_at(b: &mut Cps2Bus, o: Obj) -> (u32, u8) {
    let number = rd8(b, o.0 + f::STANCE_TABLE);
    let table = crate::mem::rd32(b, STANCE_POINTERS + 4 * number as u32);
    let tick = rd16(b, TICK);
    let v = rd8(b, table.wrapping_add(offset(tick)));
    wr8(b, o.0 + f::STANCE_VALUE, v);
    (table, v)
}


/// Replaces the low byte / low word of a register (the other bits remain).
fn set_b(r: &mut u32, v: u8) {
    *r = *r & !0xff | v as u32;
}
fn set_w(r: &mut u32, v: u16) {
    *r = *r & !0xffff | v as u32;
}


/// Box in front of an object: x offset (forwards), half-width, half-depth (D0, D1, D2 of the original).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hitbox {
    pub offset: u16,
    pub half_width: u16,
    pub half_depth: u16,
}

/// Work registers left by the original's box tests (low words, if they were set).
#[derive(Clone, Copy, Debug, Default)]
struct Leftovers {
    d0: Option<u16>,
    d1: Option<u16>,
    d2: Option<u16>,
    d3: Option<u16>,
    d2_byte: Option<u8>,
    d3_byte: Option<u8>,
    d4_byte: Option<u8>,
}

impl Leftovers {
    fn set(&self, c: &mut Cpu) {
        // in the original's order: bytes then words, each in its place
        if let Some(v) = self.d3_byte {
            set_b(&mut c.d[3], v);
        }
        if let Some(v) = self.d4_byte {
            set_b(&mut c.d[4], v);
        }
        for (r, v) in [(0, self.d0), (1, self.d1), (2, self.d2), (3, self.d3)] {
            if let Some(v) = v {
                set_w(&mut c.d[r], v);
            }
        }
        if let Some(v) = self.d2_byte {
            set_b(&mut c.d[2], v);
        }
    }
}

/// Is object `other` in the box in front of `me`? Biased gaps compared unsigned, like the original.
fn in_hitbox(b: &mut Cps2Bus, me: u32, other: u32, hitbox: Hitbox, r: &mut Leftovers) -> bool {
    let facing = rd8(b, me + f::FACING);
    let offset_x = if facing != 0 { hitbox.offset.wrapping_neg() } else { hitbox.offset };
    let other_x = rd16(b, other + f::X);
    r.d3 = Some(other_x);
    let half_x = hitbox.half_width;
    let reach_x = offset_x.wrapping_add(rd16(b, me + f::X)).wrapping_sub(other_x).wrapping_add(half_x);
    r.d0 = Some(reach_x);
    r.d1 = Some(half_x.wrapping_mul(2));
    if reach_x > half_x.wrapping_mul(2) {
        return false;
    }
    let other_depth = rd16(b, other + f::DEPTH);
    r.d1 = Some(other_depth);
    let half_depth = hitbox.half_depth;
    let reach_depth = rd16(b, me + f::DEPTH).wrapping_sub(other_depth).wrapping_add(half_depth);
    r.d0 = Some(reach_depth);
    r.d2 = Some(half_depth.wrapping_mul(2));
    reach_depth <= half_depth.wrapping_mul(2)
}

/// 0x7f0c (`strict`) and 0x7ef4: is an opponent (the targeted object `+0x50`) in the box in front of enemy `o`,
/// facing it, on the ground and in state 12 (or 14 if `strict`)? If so, after a probability draw (table `+0x74`),
/// the enemy goes to state 6 with a randomly drawn animation (table `+0x75`) and the result is true.
pub fn in_front(b: &mut Cps2Bus, o: Obj, strict: bool, hitbox: Hitbox) -> bool {
    in_front_leftovers(b, o.0, strict, hitbox).0
}

/// (result, leftovers, opponent read (A1), table read (A4), D2 and D0 longs of the draw).
fn in_front_leftovers(b: &mut Cps2Bus, me: u32, strict: bool, hitbox: Hitbox) -> (bool, Leftovers, Option<u32>, Option<u32>, Option<(u32, u32)>) {
    let mut r = Leftovers::default();
    let target = rd16(b, me + f::TARGET);
    if target == 0 {
        return (false, r, None, None, None);
    }
    let other = object(target);
    let state = rd8(b, other + f::SUB_STATE);
    let refuse = if strict { rd16(b, other + 0x0c) != 0 || (state != 12 && state != 14) } else { state != 12 };
    if refuse {
        return (false, r, Some(other), None, None);
    }
    let facing = rd8(b, me + f::FACING);
    r.d3_byte = Some(facing);
    if facing == rd8(b, other + f::FACING) {
        return (false, r, Some(other), None, None);
    }
    let gap = rd16(b, me + f::X).wrapping_sub(rd16(b, other + f::X));
    r.d3 = Some(gap);
    let side = if (gap as i16) >= 0 { 0 } else { 32 };
    r.d4_byte = Some(side);
    if side != facing || !in_hitbox(b, me, other, hitbox, &mut r) {
        return (false, r, Some(other), None, None);
    }
    let number = rd8(b, me + f::BOX_CHANCE);
    let chances = crate::mem::rd32(b, BOX_CHANCE_POINTERS + 4 * number as u32);
    let tick = rd16(b, TICK);
    let mask = rd32(b, chances.wrapping_add(offset(tick << 2)));
    let roll = rng::random(b) as u32 & 31;
    if mask >> roll & 1 == 0 {
        return (false, r, Some(other), Some(chances), Some((mask, roll)));
    }
    let number = rd8(b, me + f::BOX_ANIMATION);
    // byte `+0x75` is already a multiple of 4: no shift
    let animations = rd32(b, BOX_ANIMATION_POINTERS.wrapping_add(number as u32)).wrapping_add(offset((tick >> 1) << 5));
    let pick = rng::random(b) as u32 & 31;
    wr8(b, me + f::STATE, 6);
    let v = rd8(b, animations.wrapping_add(pick));
    wr8(b, me + f::SUB_STATE, v);
    wr8(b, me + f::SUB_STEP, 0);
    (true, r, Some(other), Some(animations), Some((mask, roll)))
}

/// Position of the object relative to the box in front of another (0x7f19c).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InHitbox {
    /// Outside the box, or in the air (D0 = 0).
    No,
    /// In the box, facing (D0 = 1).
    Face,
    /// In the box, from behind (D0 = 2).
    Back,
}

/// 0x7f19c: is object `other` in the box in front of object `me`?
pub fn box_around(b: &mut Cps2Bus, me: Obj, other: Obj, hitbox: Hitbox) -> InHitbox {
    box_reach(b, me.0, other.0, hitbox, &mut Leftovers::default())
}

fn box_reach(b: &mut Cps2Bus, me: u32, other: u32, hitbox: Hitbox, r: &mut Leftovers) -> InHitbox {
    if rd16(b, other + 0x0c) != 0 || !in_hitbox(b, me, other, hitbox, r) {
        return InHitbox::No;
    }
    let facing = rd8(b, me + f::FACING);
    let gap = rd16(b, me + f::X).wrapping_sub(rd16(b, other + f::X));
    r.d1 = Some(gap);
    let side = if (gap as i16) < 0 { 32 } else { 0 };
    r.d2_byte = Some(side);
    if side == facing {
        InHitbox::Face
    } else {
        InHitbox::Back
    }
}

/// Leftovers of the box tests reread afterwards (original glitch, `dispatch::Leftovers`): D2 and D3 (speeds of
/// the spark of `flash_and_shake`, reread by boss 11) and the low byte of D4, in the order in which the original sets
/// them.
#[derive(Clone, Copy, Debug, Default)]
pub struct HitboxLeftovers {
    leftovers: Leftovers,
    /// D2 long: the mask of the `in_front` draw.
    mask: Option<u32>,
}

impl HitboxLeftovers {
    /// Sets the leftovers in D2, D3 and D4 (the registers that the original leaves and that others reread).
    pub fn set(&self, d2: &mut u32, d3: &mut u32, d4: &mut u32) {
        let r = &self.leftovers;
        if let Some(v) = r.d3_byte {
            set_b(d3, v);
        }
        if let Some(v) = r.d4_byte {
            set_b(d4, v);
        }
        if let Some(v) = r.d2 {
            set_w(d2, v);
        }
        if let Some(v) = r.d3 {
            set_w(d3, v);
        }
        if let Some(v) = r.d2_byte {
            set_b(d2, v);
        }
        if let Some(v) = self.mask {
            *d2 = v;
        }
    }
}

/// 0xb5e0 (`ai::target_in_front`): is the target `+0x50` in the box in front of the object? With the leftovers reread.
pub fn target_in_front_leftovers(b: &mut Cps2Bus, o: Obj, hitbox: Hitbox) -> (InHitbox, HitboxLeftovers) {
    let pointer = rd16(b, o.0 + f::TARGET);
    let mut leftovers = Leftovers::default();
    let found = if pointer == 0 { InHitbox::No } else { box_reach(b, o.0, object(pointer), hitbox, &mut leftovers) };
    (found, HitboxLeftovers { leftovers, mask: None })
}

/// 0x7f19c (`box_around`), with the leftovers reread.
pub fn box_around_leftovers(b: &mut Cps2Bus, me: Obj, other: Obj, hitbox: Hitbox) -> (InHitbox, HitboxLeftovers) {
    let mut leftovers = Leftovers::default();
    let found = box_reach(b, me.0, other.0, hitbox, &mut leftovers);
    (found, HitboxLeftovers { leftovers, mask: None })
}

/// 0x7f0c / 0x7ef4 (`in_front`), with the leftovers reread.
pub fn in_front_with_leftovers(b: &mut Cps2Bus, o: Obj, strict: bool, hitbox: Hitbox) -> (bool, HitboxLeftovers) {
    let (yes, leftovers, _, _, draw) = in_front_leftovers(b, o.0, strict, hitbox);
    (yes, HitboxLeftovers { leftovers, mask: draw.map(|(m, _)| m) })
}

/// 0xb5e0 (`ai::target_in_front`) with the registers of the original: A1 = the target, D0 = 0/1/2 and flags of
/// `moveq`, D1-D3 left by the box tests. For the callers whose continuation rereads these leftovers (boss 11:
/// D2, original glitch).
pub fn target_in_front_68k(c: &mut Cpu, b: &mut Cps2Bus, o: Obj, hitbox: Hitbox) -> InHitbox {
    let pointer = rd16(b, o.0 + f::TARGET);
    let mut leftovers = Leftovers::default();
    let found = if pointer == 0 {
        InHitbox::No
    } else {
        c.a[1] = object(pointer);
        box_reach(b, o.0, object(pointer), hitbox, &mut leftovers)
    };
    leftovers.set(c);
    moveq(c, found as i32);
    found
}

/// 0x7f19c (`box_around`) with the registers of the original: D0 = 0/1/2 and flags of `moveq`, D1-D3 left by
/// the box tests.
pub fn box_around_68k(c: &mut Cpu, b: &mut Cps2Bus, me: Obj, other: Obj, hitbox: Hitbox) -> InHitbox {
    let mut leftovers = Leftovers::default();
    let found = box_reach(b, me.0, other.0, hitbox, &mut leftovers);
    leftovers.set(c);
    moveq(c, found as i32);
    found
}

/// 0x7f0c / 0x7ef4 (`in_front`) with the registers of the original: D0 (1 / 0) and flags of `moveq`, D1-D4, A1
/// and A4 left as in the original.
pub fn in_front_68k(c: &mut Cpu, b: &mut Cps2Bus, o: Obj, strict: bool, hitbox: Hitbox) -> bool {
    let (yes, leftovers, other, table, draw) = in_front_leftovers(b, o.0, strict, hitbox);
    if let Some(other) = other {
        c.a[1] = other;
    }
    leftovers.set(c);
    if let Some(table) = table {
        c.a[4] = table;
    }
    if let Some((mask, roll)) = draw {
        c.d[2] = mask;
        c.d[0] = roll;
    }
    moveq(c, yes as i32);
    yes
}

/// Push profiles: byte table (indexed by the x gap / 16) in program space, then
/// 8-byte entries (maximum height, maximum depth gap, push) in data space.
const PUSH_PROFILE_INDEX: u32 = 0x1168e;
const PUSH_PROFILES: u32 = 0x116ae;

/// Byte of the program space (table addressed PC-relative).
fn prog8(b: &mut Cps2Bus, a: u32) -> u8 {
    let word = b.fetch16(a & !1);
    if a & 1 == 0 {
        (word >> 8) as u8
    } else {
        word as u8
    }
}

/// Push (16.16) that boss `me` applies to player `player` for an x gap `gap`: 0 if the
/// player is too high or too far in depth.
fn push_amount_for(b: &mut Cps2Bus, me: u32, player: u32, gap: u16) -> u32 {
    if gap as i16 >= 512 {
        return 0;
    }
    let profile = prog8(b, PUSH_PROFILE_INDEX.wrapping_add(offset(gap >> 4))) as u16;
    let entry = PUSH_PROFILES.wrapping_add(offset(profile << 3));
    let max_height = rd16(b, entry) as i16;
    let max_depth = rd16(b, entry + 2) as i16;
    let depth = (rd16(b, player + f::DEPTH).wrapping_sub(rd16(b, me + f::DEPTH)) as i16).wrapping_abs();
    if max_height >= rd16(b, player + 0x0c) as i16 && depth < max_depth {
        rd32(b, entry + 4)
    } else {
        0
    }
}

/// Boss `me` pushes player `player` in x, towards the side it faces; true if pushed.
fn push_player_once(b: &mut Cps2Bus, me: u32, player: u32) -> bool {
    let gap = (rd16(b, player + f::X).wrapping_sub(rd16(b, me + f::X)) as i16).wrapping_abs() as u16;
    let push = push_amount_for(b, me, player, gap);
    let facing = rd8(b, me + f::FACING);
    let side = if (rd16(b, me + f::X).wrapping_sub(rd16(b, player + f::X)) as i16) < 0 { 32 } else { 0 };
    if push == 0 || side != facing {
        return false;
    }
    let push = if facing == 0 { push.wrapping_neg() } else { push };
    let x = rd32(b, player + f::X).wrapping_add(push);
    wr32(b, player + f::X, x);
    true
}

// ---------------------------------------------------------------------------------------------
// Table-driven animations and orientation tests (0x103e0-0x105d0). The "body" functions do not do
// the RTS: `leaf!` adds it for the native.
// ---------------------------------------------------------------------------------------------

const ANIM_MARK: u32 = 0x3e;

/// Address of the object designated by a 16-bit pointer (sign-extended).
fn object(pointer: u16) -> u32 {
    pointer as i16 as i32 as u32
}

// ---------------------------------------------------------------------------------------------
// Attacks on the players (0xfc44-0xfda0)
// ---------------------------------------------------------------------------------------------

/// Enemy attack boxes (8-byte entries: offset and half-extent in x, z, y) and player
/// body boxes (one table per character, 8-byte entries), in data space.
const ATTACK_BOXES: u32 = 0x123602;
const BODY_BOX_TABLES: u32 = 0xfd4c;

/// Signed byte of the data space.
fn rd8s(b: &mut Cps2Bus, a: u32) -> i16 {
    rd8(b, a) as i8 as i16
}

/// Is the player at `player` hit by the attack box `attack` (entry address) of object `me`?
/// The player must be present, visible, unprotected, neither on the ground nor dead, and have a body box.
fn player_hit(b: &mut Cps2Bus, me: u32, attack: u32, player: u32) -> bool {
    let flags = rd8(b, player);
    if flags & 1 == 0 || flags & 2 != 0 || rd8(b, player + 0x188) != 0 {
        return false;
    }
    if rd16(b, player + 4) == 0x204 || rd8(b, player + 4) == 4 {
        return false;
    }
    let body = rd8(b, player + 0x38);
    if body == 0 || rd8(b, player + 0x7e) != 0 || rd16(b, player + 0x196) != 0 {
        return false;
    }
    let character = rd8(b, player + 3) & 14;
    let table = rd32(b, BODY_BOX_TABLES + 2 * character as u32);
    let mut mine = attack;
    let mut theirs = table.wrapping_add(8 * body as u32);
    // x, then z, then y: (offset, extent) of each box; only x takes the facing direction into account
    for (axis, mirror) in [(0x08, true), (0x0c, false), (0x10, false)] {
        let (mut d0, mut d1) = (rd8s(b, mine), rd8s(b, theirs));
        if mirror {
            if rd8(b, me + f::FACING) != 0 {
                d0 = -d0;
            }
            if rd8(b, player + f::FACING) != 0 {
                d1 = -d1;
            }
        }
        let gap = d0.wrapping_add(rd16(b, me + axis) as i16).wrapping_sub(d1.wrapping_add(rd16(b, player + axis) as i16));
        let reach = rd8(b, mine + 1) as i16 + rd8(b, theirs + 1) as i16;
        mine += 2;
        theirs += 2;
        if reach < gap.wrapping_abs() {
            return false;
        }
    }
    true
}

// ---------------------------------------------------------------------------------------------
// Choosing a point to walk to around the target (0xe886, 0xeff2, 0xf084)
// ---------------------------------------------------------------------------------------------

/// Tables of candidate points by number (`+0x79`): pointer, then number of points - 1 and pairs
/// (x offset, depth offset) around the target.
pub(crate) const WALK_POINTS: u32 = 0xe93e;
const MIRRORED_WALK_POINTS: u32 = 0xf06a;

/// "City block" gap between two points: |dx| + |dy| (words).
fn manhattan(b: &mut Cps2Bus, me: u32, x: u16, depth: u16) -> u16 {
    let dx = (x.wrapping_sub(rd16(b, me + f::X)) as i16).wrapping_abs() as u16;
    let dy = (depth.wrapping_sub(rd16(b, me + f::DEPTH)) as i16).wrapping_abs() as u16;
    dx.wrapping_add(dy)
}

// ---------------------------------------------------------------------------------------------
// Miscellaneous: screen tests, random heading, grab (0x104a2-0x105d0, 0xfda2, 0xff54)
// ---------------------------------------------------------------------------------------------

/// Screen origin in x (A5 word), depending on the camera in use (`A5+0x36`).
fn screen_origin(b: &mut Cps2Bus) -> u16 {
    let offset = if rd8(b, A5 + 0x36) == 0 { 0x5a6 } else { 0x5ae };
    rd16(b, A5 + offset)
}

/// Grab movement table (signed bytes: x offset, height offset, orientation, direction).
const SEIZE_OFFSETS: u32 = 0x123652;

/// 0xfe34: object A0 is brought back into the camera's screen. Returns true if it was.
fn keep_inside_camera(b: &mut Cps2Bus, me: u32) -> bool {
    let left = if rd8(b, A5 + 0x36) == 0 { rd16(b, A5 + 0x506) } else { rd16(b, A5 + 0x556) };
    let x = rd16(b, me + f::X) as i16;
    if left as i16 > x {
        wr16(b, me + f::X, left);
        return true;
    }
    let right = left.wrapping_add(384);
    if right as i16 >= x {
        return false;
    }
    wr16(b, me + f::X, right);
    true
}

// ---------------------------------------------------------------------------------------------
// Choosing a target according to an imposed criterion (0x43bc)
// ---------------------------------------------------------------------------------------------


// ---------------------------------------------------------------------------------------------
// Table-driven draws: stance, counterattack, dodge (0x788c, 0x79ce, 0x79f6, 0xb048, 0xb068, 0xf118,
// 0x10112). A pointer table (longs) is indexed by a byte of the object; the chosen row is
// read at random (bytes) or by the frame counter (32-bit masks, a randomly drawn bit).
// ---------------------------------------------------------------------------------------------

// ---------------------------------------------------------------------------------------------
// Debris and loot (0x11a36, 0x1172e, 0xb648, 0xb780, 0xbad6, 0xe2f6)
// ---------------------------------------------------------------------------------------------

/// Allocates through `alloc` (0x2318, 0x23d8): the object, or `None` if the pool is full.
fn alloc(b: &mut Cps2Bus, pool: &pools::Pool) -> Option<u32> {
    pool.alloc(b).map(|o| o.0)
}

/// Copies the position of `from` to `to`, set 16 pixels higher and just in front.
fn place_above(b: &mut Cps2Bus, from: u32, to: u32) {
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
fn debris(b: &mut Cps2Bus, fx: u32, me: u32) {
    wr8(b, fx, 1);
    wr8(b, fx + 2, 0x32);
    wr8(b, fx + 3, 0xc);
    let x = rd16(b, me + 0x08);
    wr16(b, fx + 0x08, x);
}

/// Loot thrown into the air (ITEM of kind `kind`, variant `variant`): speeds (±0x140, -0x28 in z...),
/// flipped if `mirrored`.
fn tossed_item(b: &mut Cps2Bus, me: u32, item: u32, kind: u8, variant: u8, vy: u16, mirrored: bool) {
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
fn loot_rolls(b: &mut Cps2Bus) -> u16 {
    if rd8(b, A5 + 0x35) > 3 {
        2
    } else {
        1
    }
}

/// Special object drawn by 0xbad6: kind and variant.
const DRAWN_KIND: u32 = A5 - 0x7196;
const DRAWN_VARIANT: u32 = A5 - 0x7195;

/// Tables of 0xbad6 (read as data, A1 = `lea` of the table): row `+0x80` (kind, variant pairs), then by difficulty
/// (`A5+0x49`).
mod special {
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
    (rd8(b, DRAWN_KIND), rd8(b, DRAWN_VARIANT))
}
fn set_drawn(b: &mut Cps2Bus, kind: u8, variant: u8) {
    wr8(b, DRAWN_KIND, kind);
    wr8(b, DRAWN_VARIANT, variant);
}
/// New kind 4 variant drawn in `at`: 0 cancels the draw, 1 and 2 refer back to the draws `on_1`, `on_2`.
fn redraw(b: &mut Cps2Bus, at: u32, on_1: Draw, on_2: Draw) -> Draw {
    let variant = rd8(b, at);
    set_drawn(b, 4, variant);
    match variant {
        0 => Draw::Next,
        1 => on_1,
        2 => on_2,
        _ => Draw::Restrict,
    }
}

fn reroll_or_none(b: &mut Cps2Bus, at: u32) -> Draw {
    let variant = rd8(b, at);
    set_drawn(b, 4, variant);
    if variant == 0 { Draw::Next } else { Draw::Spawn }
}

/// Landing routines by type (longs of the code): enemies (class 2) at indices 0..31, bosses afterwards.
const LANDING_BY_KIND: u32 = 0x3e49c;


// ---------------------------------------------------------------------------------------------
// Cores of the AI batch: each original routine is a Rust function on the processed object, without registers.
// ---------------------------------------------------------------------------------------------

/// Enemy fields specific to these helpers.
mod g {
    /// Animation key of the variant (7 bits) and duration of the current entry.
    pub const VARIANT_KEY: u32 = 0x33;
    pub const ANIM_TIMER: u32 = 0x30;
    /// Mark of the animation entry (hit type), copied to `+0x44` after each step.
    pub const ANIM_MARK: u32 = 0x3e;
    pub const ANIM_MARK_COPY: u32 = 0x44;
    /// Master (16-bit pointer) whose variant or mark chooses the animation.
    pub const MASTER: u32 = 0xb2;
    /// Targeted point (x, depth), point set and number of the last point retained.
    pub const GOAL_X: u32 = 0x7a;
    pub const GOAL_DEPTH: u32 = 0x7c;
    pub const POINT_SET: u32 = 0x79;
    pub const LAST_POINT: u32 = 0x99;
    /// HP tiers: current tier, number of tiers, column (HP rank).
    pub const LEVEL: u32 = 0xaa;
    pub const LEVELS: u32 = 0xab;
    /// Behaviour parameter (and countdown of 0xff54).
    pub const BEHAVIOUR: u32 = 0x98;
    /// Rows of the draw tables: `+0x6a` → `+0x6b`, `+0x6c` (masks) and `+0x6d` → `+0x6e`, `+0x8e` →
    /// `+0x85` (`+0x84`: off screen).
    pub const ROW_6A: u32 = 0x6a;
    pub const DRAW_6B: u32 = 0x6b;
    pub const ROW_6C: u32 = 0x6c;
    pub const ROW_6D: u32 = 0x6d;
    pub const DRAW_6E: u32 = 0x6e;
    pub const OFFSCREEN_84: u32 = 0x84;
    pub const DRAW_85: u32 = 0x85;
    pub const ROW_8E: u32 = 0x8e;
    /// Parry level (0x10112).
    pub const PARRY_LEVEL: u32 = 0x68;
    /// Loot: table row (`+0x80`), mask and flag of the object dropped only once.
    pub const LOOT_ROW: u32 = 0x80;
    pub const ONCE_MASK: u32 = 0x9c;
    pub const ONCE_DONE: u32 = 0x9d;
}

/// The object designated by the word `pointer` (sign-extended, like `movea.w`).
fn linked(pointer: u16) -> Obj {
    Obj(object(pointer))
}

// --- table-driven animations (0x103e0, 0x10428, 0x10450) -----------------------------------------

/// First entry (16 bytes) of set `set` whose byte `+3` equals `key` under `mask`.
fn find_animation(b: &mut Cps2Bus, set: u32, key: u8, mask: u8) -> u32 {
    let mut at = set;
    while rd8(b, at + 3) & mask != key & mask {
        at = at.wrapping_add(16);
    }
    at
}

/// Copy of the animation mark (0x1046c).
fn copy_mark(b: &mut Cps2Bus, o: Obj) {
    let v = o.u8(b, g::ANIM_MARK);
    o.set_u8(b, g::ANIM_MARK_COPY, v);
}

/// 0x103e0: starts the animation of set `set` with the same key as the current variant (`+0x33`), keeping the duration
/// of the current entry.
pub fn animation_by_variant(b: &mut Cps2Bus, o: Obj, set: u32) {
    let timer = o.u16(b, g::ANIM_TIMER);
    let key = o.u8(b, g::VARIANT_KEY);
    let at = find_animation(b, set, key, 0x7f);
    crate::objects::anim::start_entry(b, o, at);
    o.set_u16(b, g::ANIM_TIMER, timer);
    copy_mark(b, o);
}

/// 0x10428: starts the animation of set `set` with the same key as the master's variant (`+0xb2`).
pub fn animation_of_master(b: &mut Cps2Bus, o: Obj, set: u32) {
    let master = linked(o.u16(b, g::MASTER));
    let key = master.u8(b, g::VARIANT_KEY);
    let at = find_animation(b, set, key, 0x7f);
    crate::objects::anim::start_entry(b, o, at);
    copy_mark(b, o);
}

/// 0x10450: starts the animation of set `set` whose mark (`+3`) is that of the master's current entry.
pub fn animation_of_mark(b: &mut Cps2Bus, o: Obj, set: u32) {
    let master = linked(o.u16(b, g::MASTER));
    let key = master.u8(b, g::ANIM_MARK);
    let at = find_animation(b, set, key, 0xff);
    crate::objects::anim::start_entry(b, o, at);
}

// --- players and tiers (0x10474, 0x104a2, 0x105b6) -------------------------------------------------

/// 0x10474: at least one player is alive.
pub fn players_alive(b: &mut Cps2Bus) -> bool {
    crate::ram::PLAYERS_ALIVE.get(b) != 0
}

/// 0x104a2: is `o` outside the strip `[screen + left, screen + right]` (limits excluded from the inside)? The screen
/// is the origin of the display camera (`A5+0x5a6`, or `A5+0x5ae` in two-camera mode).
pub fn outside_band(b: &mut Cps2Bus, o: Obj, left: u16, right: u16) -> bool {
    let origin = screen_origin(b);
    let x = o.u16(b, Obj::X) as i16;
    let (low, high) = (left.wrapping_add(origin) as i16, right.wrapping_add(origin) as i16);
    low >= x || high <= x
}

/// 0x105b6: passes the next HP tier (`+0xaa` < `+0xab`) if its threshold in `table` (tier row,
/// column `+0xac`, words) reaches the HP (`+0x62`). True if a tier is passed.
pub fn next_threshold(b: &mut Cps2Bus, o: Obj, table: u32) -> bool {
    let level = o.u8(b, g::LEVEL) as i8;
    if level >= o.u8(b, g::LEVELS) as i8 {
        return false;
    }
    let column = o.u8(b, crate::ai::f::HP_RANK) as i8 as i16;
    let index = (level as i16).wrapping_shl(7).wrapping_add(column);
    let threshold = rd16(b, table.wrapping_add(offset(index as u16))) as i16;
    if threshold < o.u16(b, crate::ai::f::HP) as i16 {
        return false;
    }
    o.set_u8(b, g::LEVEL, level.wrapping_add(1) as u8);
    true
}

// --- walking points around the target (0xe76c, 0xe7fa, 0xe886, 0xe914, 0xeff2, 0xf084) --------------

/// The walking point retained: the nearest or the farthest from `o`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Pick {
    Nearest,
    Farthest,
}

/// Retains, among the points of the table of `o` (`+0x79`) around its target, the one that is nearest (on a
/// tie, the last) or farthest (on a tie, the first) from `o`, and stores it at `+0x7a`/`+0x7c` (the
/// target's position if none). `skip`: point number skipped without reading its pair (original glitch: the following
/// points take the shifted pair); the point retained is then noted at `+0x99`. `mirror`: x offsets
/// flipped if the target faces left. Returns the point retained.
fn pick_point(b: &mut Cps2Bus, o: Obj, table: u32, pick: Pick, skip: Option<u8>, mirror: bool) -> (u16, u16) {
    let target = linked(o.u16(b, crate::ai::f::TARGET));
    let set = o.u8(b, g::POINT_SET) as u32;
    let mut at = rd32(b, table + 4 * set);
    let last = rd16(b, at);
    at += 2;
    let (target_x, target_depth) = (target.u16(b, Obj::X), target.u16(b, Obj::Y));
    o.set_u16(b, g::GOAL_X, target_x);
    o.set_u16(b, g::GOAL_DEPTH, target_depth);
    let mut best: u16 = if pick == Pick::Farthest { 0 } else { 0x7fff };
    for point in (0..=last).rev() {
        if skip == Some(point as u8) {
            continue;
        }
        let mut dx = rd16(b, at);
        let dy = rd16(b, at + 2);
        at += 4;
        if mirror && target.u8(b, f::FACING) != 0 {
            dx = dx.wrapping_neg();
        }
        let (x, depth) = (dx.wrapping_add(target_x), dy.wrapping_add(target_depth));
        let distance = manhattan(b, o.0, x, depth);
        let better = match pick {
            Pick::Farthest => distance > best,
            Pick::Nearest => distance <= best,
        };
        if better {
            best = distance;
            o.set_u16(b, g::GOAL_X, x);
            o.set_u16(b, g::GOAL_DEPTH, depth);
            if skip.is_some() {
                o.set_u8(b, g::LAST_POINT, point as u8);
            }
        }
    }
    (o.u16(b, g::GOAL_X), o.u16(b, g::GOAL_DEPTH))
}

/// Is `o` more than 384 pixels to the right of the left edge of the camera in service?
fn far_from_camera(b: &mut Cps2Bus, o: Obj) -> bool {
    let (camera, _) = crate::ram::camera(b);
    o.u16(b, Obj::X).wrapping_sub(camera) > 0x180
}

/// 0xf084: aims at the farthest point around the target and turns one notch towards it.
pub fn walk_far_point(b: &mut Cps2Bus, o: Obj) {
    let goal = pick_point(b, o, WALK_POINTS, Pick::Farthest, None, false);
    crate::ai::turn_toward(b, o, goal);
}

/// 0xe76c: aims at the nearest point around the target (far from the camera: the farthest, 0xf084) and
/// turns one notch towards it. Nothing without a target.
pub fn approach_target(b: &mut Cps2Bus, o: Obj) {
    if o.u16(b, crate::ai::f::TARGET) == 0 {
        return;
    }
    if far_from_camera(b, o) {
        return walk_far_point(b, o);
    }
    let goal = pick_point(b, o, WALK_POINTS, Pick::Nearest, None, false);
    crate::ai::turn_toward(b, o, goal);
}

/// 0xeff2: like `approach_target` (without the far case), the offsets flipped according to the target's direction.
pub fn walk_near_point(b: &mut Cps2Bus, o: Obj) {
    if o.u16(b, crate::ai::f::TARGET) == 0 {
        return;
    }
    let goal = pick_point(b, o, MIRRORED_WALK_POINTS, Pick::Nearest, None, true);
    crate::ai::turn_toward(b, o, goal);
}

/// 0xe886: retains the farthest point around the target, except the last one retained (`+0x99`); far from the
/// camera, aims straight away at a far point (0xf084). Nothing without a target.
pub fn choose_walk_point(b: &mut Cps2Bus, o: Obj) {
    if o.u16(b, crate::ai::f::TARGET) == 0 {
        return;
    }
    if far_from_camera(b, o) {
        return walk_far_point(b, o);
    }
    let last = o.u8(b, g::LAST_POINT);
    pick_point(b, o, WALK_POINTS, Pick::Farthest, Some(last), false);
}

/// 0xe914: is the targeted point reached (to within 8 pixels on each axis)? Then another is chosen (0xe886).
pub fn walk_point_reached(b: &mut Cps2Bus, o: Obj) {
    let near = |goal: u16, here: u16| goal.wrapping_sub(here).wrapping_add(8) <= 16;
    let (gx, x) = (o.u16(b, g::GOAL_X), o.u16(b, Obj::X));
    let (gy, y) = (o.u16(b, g::GOAL_DEPTH), o.u16(b, Obj::Y));
    if near(gx, x) && near(gy, y) {
        choose_walk_point(b, o);
    }
}

/// 0xe7fa: retains the nearest point around the target and notes its number (`+0x99`, 0xff if none); far
/// from the camera, aims at a far point (0xf084). Nothing without a target.
pub fn choose_near_walk_point(b: &mut Cps2Bus, o: Obj) {
    if o.u16(b, crate::ai::f::TARGET) == 0 {
        return;
    }
    if far_from_camera(b, o) {
        return walk_far_point(b, o);
    }
    o.set_u8(b, g::LAST_POINT, 0xff);
    let target = linked(o.u16(b, crate::ai::f::TARGET));
    let set = o.u8(b, g::POINT_SET) as u32;
    let mut at = rd32(b, WALK_POINTS + 4 * set);
    let last = rd16(b, at);
    at += 2;
    let (target_x, target_depth) = (target.u16(b, Obj::X), target.u16(b, Obj::Y));
    o.set_u16(b, g::GOAL_X, target_x);
    o.set_u16(b, g::GOAL_DEPTH, target_depth);
    let mut best: u16 = 0x7fff;
    for point in (0..=last).rev() {
        let (x, depth) = (rd16(b, at).wrapping_add(target_x), rd16(b, at + 2).wrapping_add(target_depth));
        at += 4;
        let distance = manhattan(b, o.0, x, depth);
        if distance <= best {
            best = distance;
            o.set_u16(b, g::GOAL_X, x);
            o.set_u16(b, g::GOAL_DEPTH, depth);
            o.set_u8(b, g::LAST_POINT, point as u8);
        }
    }
}

// --- grabs (0xfc44, 0xfd64, 0xfd8c, 0xfda2) --------------------------------------------------------

/// 0xfc44: is a player hit by the attack box `attack` of `o`? Returns the first one hit.
pub fn attack_hits_a_player(b: &mut Cps2Bus, o: Obj, attack: u8) -> Option<Obj> {
    let boxes = ATTACK_BOXES.wrapping_add(offset((attack as u16) << 3));
    crate::ram::PLAYERS.iter().find(|p| player_hit(b, o.0, boxes, p.0))
}

/// 0xfd64: player `player` is grabbed by `o` (grab animation `anim` of `o`).
pub fn seize_player(b: &mut Cps2Bus, o: Obj, player: Obj, anim: u8) {
    player.set_u32(b, Obj::STATE, 0x0204_3200);
    player.set_u8(b, Obj::ACTIVE, 2);
    player.set_u16(b, 0x186, o.word());
    player.set_u8(b, 0x188, 1);
    o.set_u16(b, crate::ai::f::TARGET, player.word());
    o.set_u32(b, Obj::STATE, 0x020c_0000);
    o.set_u8(b, f::SUB_STATE, anim);
}

/// 0xfd8c: player `player` is released (action 0x34).
pub fn release_player(b: &mut Cps2Bus, player: Obj) {
    player.set_u32(b, Obj::STATE, 0x0204_0000);
    player.set_u8(b, f::SUB_STATE, 0x34);
    player.set_u8(b, 0x188, 0);
}

/// 0xfda2: `o` is brought back into the screen, then the object it holds (`+0x50`) follows it according to the step
/// (`+0x3e`) of the grab animation (movement table: x, height, orientation, direction of `+0x12`).
pub fn hold_seized(b: &mut Cps2Bus, o: Obj) {
    let me = o.0;
    keep_inside_camera(b, me);
    let pointer = rd16(b, me + f::TARGET);
    if (pointer as i16) >= 0 {
        return;
    }
    let held = object(pointer);
    let step = rd8(b, me + ANIM_MARK) as u32;
    if step == 0 {
        return;
    }
    let mut at = SEIZE_OFFSETS + 4 * step;
    let mirrored = rd8(b, me + f::FACING) != 0;
    let mut dx = rd8s(b, at);
    if mirrored {
        dx = -dx;
    }
    let x = (dx as u16).wrapping_add(rd16(b, me + f::X));
    wr16(b, held + f::X, x);
    let rise = (rd8s(b, at + 1) as u16).wrapping_add(rd16(b, me + 0x0c));
    let height = if (rise as i16) < 0 {
        let z = rd16(b, me + 0x0c).wrapping_sub(rise);
        wr16(b, me + 0x0c, z);
        0
    } else {
        rise
    };
    wr16(b, held + 0x0c, height);
    let depth = rd16(b, me + f::DEPTH);
    wr16(b, held + f::DEPTH, depth);
    wr16(b, me + 0x12, 1);
    wr16(b, held + 0x12, 1);
    at += 2;
    let faces = rd8(b, at) != 0;
    let mut facing: u8 = if faces { 32 } else { 0 };
    if mirrored {
        facing = if facing != 0 { 0 } else { 32 };
    }
    wr8(b, held + f::FACING, facing);
    let direction: i16 = if (rd8(b, at + 1) as i8) < 0 { 1 } else { -1 };
    let v = rd16(b, me + 0x12).wrapping_add(direction as u16);
    wr16(b, me + 0x12, v);
}

// --- states and parameters (0xff54, 0xfffc) ---------------------------------------------------------

/// Masks (words, by players alive and rank) of 0xff54.
const COUNTDOWN_MASKS: u32 = 0xff9a;

/// 0xff54: countdown `+0x98`; at its end, and if the bit drawn in the mask of the players alive and the rank
/// is set, state 0x020e0000; otherwise `o` stays (or becomes again) active in state 0x04040200.
pub fn countdown_then_state(b: &mut Cps2Bus, o: Obj) {
    let left = o.u8(b, g::BEHAVIOUR).wrapping_sub(1);
    o.set_u8(b, g::BEHAVIOUR, left);
    let again = |b: &mut Cps2Bus| {
        o.set_u8(b, Obj::ACTIVE, 1);
        o.set_u32(b, Obj::STATE, 0x0404_0200);
    };
    if left as i8 > 0 {
        return again(b);
    }
    let row = ((crate::ram::PLAYERS_ALIVE.get(b) as u16) << 4).wrapping_add((rd16(b, TICK) >> 2) << 1);
    let mask = crate::mem::prog16(b, COUNTDOWN_MASKS.wrapping_add(offset(row))) as u32;
    let roll = rng::random(b) as u32 & 15;
    if mask >> roll & 1 == 0 {
        return again(b);
    }
    o.set_u32(b, Obj::STATE, 0x020e_0000);
}

/// Behaviour parameter by type of 0xfffc.
const BEHAVIOUR_BY_KIND: u32 = 0x1000a;

/// 0xfffc: behaviour parameter `+0x98` of `o`, according to its type.
pub fn behaviour_by_kind(b: &mut Cps2Bus, o: Obj) {
    let kind = o.u8(b, Obj::KIND) as u32;
    let v = crate::mem::prog8(b, BEHAVIOUR_BY_KIND + kind);
    o.set_u8(b, g::BEHAVIOUR, v);
}

// --- table-driven draws (0x788c, 0x79ce, 0x79f6, 0xb048, 0xb068, 0xf118, 0x10112) ------------------

/// 32-bit mask of row `number` of the table `pointers`, chosen by the rank; one of its bits is drawn:
/// true if it is set.
fn rank_mask_roll(b: &mut Cps2Bus, pointers: u32, number: u8) -> bool {
    let row = rd32(b, pointers + 4 * number as u32);
    let tick = rd16(b, TICK);
    let mask = rd32(b, row.wrapping_add(offset(tick << 2)));
    let roll = rng::random(b) as u32 & 31;
    mask >> roll & 1 != 0
}

/// Byte drawn at random (32 choices) in row `number` of the table `pointers`.
fn random_byte(b: &mut Cps2Bus, pointers: u32, number: u8) -> u8 {
    let row = rd32(b, pointers + 4 * number as u32);
    let roll = rng::random(b) as u32 & 31;
    rd8(b, row + roll)
}

/// 0x788c: `+0x6b` drawn in row `+0x6a`.
pub fn draw_6b(b: &mut Cps2Bus, o: Obj) {
    let number = o.u8(b, g::ROW_6A);
    let v = random_byte(b, 0x78ae, number);
    o.set_u8(b, g::DRAW_6B, v);
}

/// 0x79f6: inside the play area, `+0x6e` is drawn in row `+0x6d`; otherwise it is 1.
pub fn draw_6e(b: &mut Cps2Bus, o: Obj) {
    let v = if crate::objects::cull::in_arena(b, o, 0) {
        let number = o.u8(b, g::ROW_6D);
        random_byte(b, 0x7a50, number)
    } else {
        1
    };
    o.set_u8(b, g::DRAW_6E, v);
}

/// 0x79ce: if the bit drawn in the mask of row `+0x6c` is set, `draw_6e` and true; otherwise false.
pub fn maybe_draw_6e(b: &mut Cps2Bus, o: Obj) -> bool {
    let number = o.u8(b, g::ROW_6C);
    if !rank_mask_roll(b, 0x7a34, number) {
        return false;
    }
    draw_6e(b, o);
    true
}

/// The two parry tables: row `+0x68` (0xb048) or `+0x69` (0xb068) of the victim.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Parry {
    A,
    B,
}

/// 0xb048 / 0xb068: does the victim `victim` parry the hit of `attacker`? It must know how to parry (`+0x67`),
/// be on the ground and face the aggressor; a bit is then drawn in the mask of its parry row.
pub fn parries(b: &mut Cps2Bus, attacker: Obj, victim: Obj, table: Parry) -> bool {
    let facing = attacker.u8(b, f::FACING);
    if victim.u8(b, 0x67) == 0 || victim.u16(b, Obj::Z) != 0 || facing == victim.u8(b, f::FACING) {
        return false;
    }
    let field = match table {
        Parry::A => 0x68,
        Parry::B => 0x69,
    };
    let number = victim.u8(b, field);
    rank_mask_roll(b, 0xb0b0, number)
}

/// 0xf118: outside the play area, `+0x84` = 1; otherwise `+0x84` = 0 and `+0x85` is drawn in row `+0x8e`, block
/// chosen by the rank (32 bytes per pair of ranks).
pub fn draw_84(b: &mut Cps2Bus, o: Obj) {
    if !crate::objects::cull::in_arena(b, o, 0) {
        o.set_u16(b, g::OFFSCREEN_84, 1);
        return;
    }
    let number = o.u8(b, g::ROW_8E) as u32;
    let block = (rd16(b, TICK) >> 1) << 5;
    let row = rd32(b, 0xf160 + 4 * number).wrapping_add(offset(block));
    let roll = rng::random(b) as u32 & 31;
    let v = rd8(b, row + roll);
    o.set_u8(b, g::DRAW_85, v);
    o.set_u8(b, g::OFFSCREEN_84, 0);
}

/// Parry levels by scene level and players alive of 0x10112.
const PARRY_LEVELS: u32 = 0x10144;

/// 0x10112: parry level `+0x68` = 8 without a scene level (`A5+0x5a`), otherwise read according to it (capped at 4)
/// and the number of players alive.
pub fn set_68(b: &mut Cps2Bus, o: Obj) {
    let level = rd8(b, A5 + 0x5a);
    let v = if level == 0 {
        8
    } else {
        let players = crate::ram::PLAYERS_ALIVE.get(b).wrapping_mul(4);
        let row = ((level.wrapping_sub(1) as i8).min(3) as u8).wrapping_add(players);
        crate::mem::prog8(b, PARRY_LEVELS + row as u32)
    };
    o.set_u8(b, g::PARRY_LEVEL, v);
}

// --- push of the players (0x115e2) ----------------------------------------------------------------

/// 0x115e2: boss `o` pushes each player present in front of it, except the thief or the dwarf in their state 26
/// (`+0x6a`), a player exiting (`+0x66`) or protected (`+0x188`).
pub fn push_players(b: &mut Cps2Bus, o: Obj) {
    for p in crate::ram::PLAYERS.iter() {
        let player = p.0;
        if rd8(b, player) == 0 {
            continue;
        }
        if rd8(b, player + 3) >> 1 == 5 && rd8(b, player + 0x6a) == 26 {
            continue;
        }
        if rd8(b, player + 0x66) == 0 && rd8(b, player + 0x188) == 0 {
            push_player_once(b, o.0, player);
        }
    }
}

// --- debris and loot (0x1172e, 0x11a36, 0xb648, 0xb780, 0xbad6, 0xe2f6) --------------------------

/// Debris sets of 0x1172e.
const DEBRIS_SETS: u32 = 0x1178a;

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
const GROUND_DEBRIS: [u32; 2] = [0x11a9e, 0x11ada];

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
    crate::sound::play(b, 0xede08, o);
}

/// Object dropped only once per type by 0xb648 (kind 2; 0: nothing), and luck masks.
const ONCE_VARIANTS: u32 = 0xb6e4;
const ONCE_MASKS: u32 = 0xb6fc;

/// Throws an object into the air above `o` (kind `kind`, variant `variant`): speeds ±0x140 in x, 0x300 upwards,
/// `vy` in depth, flipped if `mirrored`.
fn toss_item(b: &mut Cps2Bus, o: Obj, item: u32, kind: u8, variant: u8, vy: u16, mirrored: bool) {
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
const LOOT: u32 = 0xb806;

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
const TREASURE_ROWS: u32 = 0xe372;
const TREASURE_LISTS: u32 = 0xe642;

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
fn party_classes(b: &mut Cps2Bus) -> Vec<u8> {
    let mut classes = Vec::with_capacity(4);
    for p in crate::ram::PLAYERS.iter() {
        if p.u8(b, Obj::ACTIVE) != 0 && p.u16(b, crate::ai::f::HP) != 0 {
            classes.push(p.u8(b, Obj::VARIANT) >> 1);
        }
    }
    classes
}

/// Draws an entry of `table` at `base` + (draw & `mask`) × `scale`.
fn special_entry(b: &mut Cps2Bus, table: u32, base: u16, mask: u16, scale: u16) -> u32 {
    let roll = (rng::random(b) & mask).wrapping_mul(scale).wrapping_add(base);
    table.wrapping_add(offset(roll))
}

/// Start of the difficulty row (`A5+0x49`) of `size` bytes.
fn difficulty_line(b: &mut Cps2Bus, size: u16) -> u16 {
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
fn only_class_draw(b: &mut Cps2Bus, d3: &mut Option<u16>, class: u8, table: u32) -> Draw {
    if party_classes(b).iter().any(|&k| k != class) {
        return Draw::Spawn;
    }
    let line = difficulty_line(b, 16);
    *d3 = Some(line);
    let at = special_entry(b, table, line, 15, 1);
    reroll_or_none(b, at)
}

// --- lightning and shake (0x103ba), landing by type (0x3e480) --------------------------------------

/// 0x103ba: red flash, 8-frame shake (`A5+0x4be`) and impact spark (0xb141a, speeds `vx`, `ax`)
/// whose variant is reset to zero.
/// Original glitch: the original does not set the spark's speeds; it takes the D2 and D3 left by the
/// routines called before (the callers pass them in the clear).
pub fn flash_and_shake(b: &mut Cps2Bus, o: Obj, vx: u16, ax: u16) {
    crate::palette::set_flash_color(b, crate::palette::FLASH_COLOR_RED);
    wr16(b, A5 + 0x4be, 8);
    if let Some(spark) = crate::families::effects_creators::impact_spark(b, o, vx, ax) {
        spark.set_u8(b, Obj::VARIANT, 0);
    }
}

/// 0x3e480: the landing routine specific to the type of `o` (enemies: class 2, types 0..31; bosses afterwards) and
/// the index read in the table, which the original leaves in D0 when jumping to the routine (A1). `None` for the
/// types without a routine (a plain `rts`: 0x3e2aa, 0x3bb9a).
pub fn landing_routine(b: &mut Cps2Bus, o: Obj) -> Option<(u32, u16)> {
    let base: u8 = if o.u8(b, Obj::CLASS) == 2 { 0 } else { 32 };
    let index = base.wrapping_add(o.u8(b, Obj::KIND)) as u16 * 4;
    let routine = crate::mem::prog32(b, LANDING_BY_KIND.wrapping_add(offset(index)));
    (routine != 0x3e2aa && routine != 0x3bb9a).then_some((routine, index))
}
