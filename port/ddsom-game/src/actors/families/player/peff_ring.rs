//! Rings for choosing items and magics (PEFF types 0 and 1, class C12).
//!
//! The parent (negative variant `+3`) is linked to the player (`+0x50`); it creates six children (type 0) or one per
//! slot of the equipment row (type 1), keeps their addresses in `+0x30..` and drives them through their substates
//! (`+5`). The children turn on the ring: the angle is the speed index `+0x2d` (32 positions) and the speeds come from
//! ROM speed tables (`+0x4c`). The role names are hypotheses (h).

use super::vars;
use crate::actors::families::player::records::Shot;
use crate::world::objects::player::Player;
use crate::actors::families::cx::Bench;
use crate::actors::families::cx::{handlers, Cx};
use super::pshell::*;
use crate::world::objects::Obj;
use crate::world::objects::pools;
use crate::world::objects::Current;
use crate::actors::player::tables::{player_data, pc16, pc32, byte, word, Read};

/// Icon animation sets: normal, alternate palette.
const ICONS: crate::world::data::RomAddr = crate::world::data::RomAddr(0x1aef78);
const ICONS_ALT: crate::world::data::RomAddr = crate::world::data::RomAddr(0x1b21c2);

/// Fields of the parents and the children.
mod g {
    /// Generation of the game (`A5+0x34`) at creation time: the ring dies if it changes.
    pub const GENERATION: u32 = 0x5a;
    /// Link of the child to its parent.
    pub const PARENT: u32 = 0x52;
    /// Target angle, step of the rotation.
    pub const TARGET: u32 = 0x54;
    pub const STEP: u32 = 0x55;
    /// Children of the parent (words).
    pub const CHILDREN: u32 = 0x30;
    /// Ring delay (byte) and lifetime (word).
    pub const DELAY: u32 = 0x3c;
    /// Equipment of the icon: item class and quantity (`+0x57`, `+0x58`).
    pub const ITEM: u32 = 0x57;
    pub const COUNT: u32 = 0x58;
}

/// Signed word of the speed table `table`, converted to integer 16.16 (integer part of the 8.8 word).
fn whole_part(word: u16) -> u32 {
    ((((word as u32) << 16) as i32) >> 8) as u32 & 0xffff_0000
}

/// Sets the child's speeds for step `step` (`0x4a1e0`, `0x4a822`): speed table chosen by `step`, row chosen by angle
/// `+0x2d`.
fn set_speeds<R: Bench>(x: &mut Cx<'_, R>, tables: u32, step: u8) {
    let table = pc32(x.b, tables + 4 * (step as i8 as i16 as u16 as u32 & 0xffff));
    x.set32(Obj::SPEED_TABLE, table);
    let row = (x.u8(Obj::SPEED_INDEX) as u32) << 2 & 0xffff;
    let vx = word(x.b, table + row);
    let vz = word(x.b, table + row + 2);
    x.set16(Obj::VX, vx);
    x.set16(Obj::VZ, vz);
    x.set16(Obj::AX, 0);
    x.set16(Obj::AZ, 0);
}

/// Position of the child: that of the parent, plus twice the integer part of the speeds of the current row.
fn follow_parent<R: Bench>(x: &mut Cx<'_, R>) {
    let parent = x.linked(g::PARENT);
    let d0 = if x.u8(Obj::VARIANT) != 0 { 0x8000 } else { 0 };
    x.copy_pos(parent);
    x.set16(Shot::Y_FRACTION, d0);
    let row = (x.u8(Obj::SPEED_INDEX) as u32) << 2 & 0xffff;
    let table = x.u32(Obj::SPEED_TABLE);
    let vx = whole_part(word(x.b, table + row));
    x.add32(Obj::X, vx);
    x.add32(Obj::X, vx);
    let vz = whole_part(word(x.b, table + row + 2));
    x.add32(Obj::Z, vz);
    x.add32(Obj::Z, vz);
}

/// The ring dies if the generation has changed, if the game is paused or if the player is dead.
fn ring_alive<R: Bench>(x: &mut Cx<'_, R>, generation: u32) -> bool {
    if vars::PAUSED.get(x.b) != 0 {
        return false;
    }
    let current = vars::SCENE.get(x.b);
    if current != x.u8(generation) {
        return false;
    }
    let player = x.linked(f::OWNER);
    player.u16(x.b, Player::HP) != 0
}

/// The parent carries on as long as the game is not paused, the generation has not changed and the player lives.
fn parent_alive<R: Bench>(x: &mut Cx<'_, R>, generation: u32, player: Obj) -> bool {
    if vars::PAUSED.get(x.b) != 0 {
        return false;
    }
    let current = vars::SCENE.get(x.b);
    current == x.u8(generation) && player.u16(x.b, Player::HP) != 0
}

/// Parent: follows the player at an offset (words read from ROM, according to the player's variant).
/// Original glitch: D2.w keeps the Z offset (`dispatch::Leftovers`); the following PEFF handler may re-read it (the
/// freeze bounce, 0x4bda8, indexes its table by D2: bot `longue_a`, frame 59 919).
fn parent_follow<R: Bench>(x: &mut Cx<'_, R>, offsets: u32) {
    let player = x.linked(f::OWNER);
    x.copy_pos(player);
    x.add16(Obj::Y, 0xffff);
    let row = (player.u8(x.b, Player::CHARACTER) as u32) << 2 & 0xffff;
    let mut dx = pc16(x.b, offsets + row);
    let dz = pc16(x.b, offsets + row + 2);
    if player.u8(x.b, f::FLIP) == 0 {
        dx = dx.wrapping_neg();
    }
    *x.c.d2() = *x.c.d2() & !0xffff | dz as u32;
    x.add16(Obj::X, dx);
    x.add16(Obj::Z, dz);
}

/// For each of the parent's `n` children, calls `f`.
fn for_children<R: Bench>(x: &mut Cx<'_, R>, n: u32, mut f: impl FnMut(&mut Cx<'_, R>, Obj)) {
    let mut a2 = x.o.0 + g::CHILDREN;
    for _ in 0..n {
        let child = Obj(x.u16(a2 - x.o.0) as i16 as i32 as u32 & 0xff_ffff);
        f(x, child);
        a2 += 2;
    }
}

/// Ring of magics/items of the cleric, the elf...: type 0 (0x49e36).
mod ring0 {
    use super::*;

    /// Lifetime of the ring (word).
    const LIFE: u32 = 0x3e;

    const OFFSETS: u32 = 0x49f88;
    const SPEEDS: u32 = 0x4a20a;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(Obj::STATE) {
            0 => init(x),
            2 => {
                if (x.u8(Obj::VARIANT) as i8) < 0 {
                    parent(x)
                } else {
                    child(x)
                }
            }
            4 => {
                if (x.u8(Obj::VARIANT) as i8) < 0 {
                    let player = x.linked(f::OWNER);
                    player.set_u16(x.b, Player::SPECIAL_EFFECT, 0);
                    player.set_u8(x.b, Player::SPECIAL_ACTIVE, 0);
                }
                x.free_peff();
            }
            _ => x.free_peff(),
        }
    }

    fn init<R: Bench>(x: &mut Cx<'_, R>) {
        if (x.u8(Obj::VARIANT) as i8) < 0 {
            return parent_init(x);
        }
        x.add8(Obj::STATE, 2);
        let gen = vars::SCENE.get(x.b);
        x.set8(g::GENERATION, gen);
        x.set8(g::STEP, 0);
        let variant = x.u8(Obj::VARIANT) as u32;
        let angle = player_data().ring0_initial_angle.at8(x.b, variant);
        x.set8(Obj::SPEED_INDEX, angle);
        set_speeds(x, SPEEDS, 0);
        icon(x);
    }

    /// Icon animation from the equipment (`0x49edc`).
    fn icon<R: Bench>(x: &mut Cx<'_, R>) {
        let item = x.u8(g::ITEM) as u32;
        let count = x.u8(g::COUNT) as u32;
        let count = if x.u8(Obj::SPEED_INDEX) != 0 { 0xa } else { count };
        let set = if vars::ALT_PALETTE.get(x.b) != 0 { ICONS_ALT.0 } else { ICONS.0 };
        x.anim_start(set, (((item << 4) + count) & 0xffff) as u16);
    }

    fn parent_init<R: Bench>(x: &mut Cx<'_, R>) {
        if pools::PEFF.free_count(x.b) <= 6 {
            return;
        }
        x.add8(Obj::STATE, 2);
        let gen = vars::SCENE.get(x.b);
        x.set8(g::GENERATION, gen);
        x.set16(LIFE, 0x258);
        x.set8(g::DELAY, 0x10);
        let player = x.linked(f::OWNER);
        player.set_u8(x.b, Player::SPECIAL_COUNT, 2);
        let mut a2 = x.o.0 + g::CHILDREN;
        for k in 0..6u32 {
            // Original glitch: the allocation failure is not tested (see `Pool::alloc_unchecked`).
            let child = x.alloc_unchecked(&pools::PEFF);
            child.set_u8(x.b, Obj::ACTIVE, 1);
            child.set_u8(x.b, Obj::KIND, 0);
            child.set_u8(x.b, Obj::VARIANT, k as u8);
            child.set_u16(x.b, g::PARENT, x.o.0 as u16);
            x.set16(a2 - x.o.0, child.0 as u16);
            a2 += 2;
            let owner = x.u16(f::OWNER);
            child.set_u16(x.b, f::OWNER, owner);
            let item = player.u8(x.b, Player::INVENTORY + k);
            let count = player.u8(x.b, Player::QUANTITIES + k);
            child.set_u8(x.b, g::ITEM, item);
            child.set_u8(x.b, g::COUNT, count);
        }
    }

    fn parent<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(Shot::SUB_STATE) {
            0 => {
                // wait for the ring to open
                x.add8(g::DELAY, 0xff);
                if x.u8(g::DELAY) == 0 {
                    x.add8(Shot::SUB_STATE, 2);
                }
            }
            2 => open(x),
            _ => {
                x.add8(g::DELAY, 0xff);
                if x.u8(g::DELAY) == 0 {
                    x.add8(Obj::STATE, 2);
                    for_children(x, 6, |x, child| {
                        let v = child.u8(x.b, Obj::STATE).wrapping_add(2);
                        child.set_u8(x.b, Obj::STATE, v)
                    });
                }
            }
        }
        parent_follow(x, OFFSETS);
        let owner = x.linked(f::OWNER);
        if parent_alive(x, g::GENERATION, owner) {
            return;
        }
        x.set8(Obj::STATE, 4);
    }

    /// The ring is open: the button rotates the equipment row, the end closes the ring.
    fn open<R: Bench>(x: &mut Cx<'_, R>) {
        x.add16(LIFE, 0xffff);
        if x.u16(LIFE) == 0 {
            return close(x);
        }
        if vars::PAUSED.get(x.b) != 0 {
            return close(x);
        }
        let player = x.linked(f::OWNER);
        let keys = player.u8(x.b, Player::KEYS_NEW);
        if keys & 0x10 != 0 || keys & 0x80 != 0 {
            return close(x);
        }
        let held = player.u8(x.b, Player::KEYS_HELD);
        if held & 0x40 == 0 {
            return;
        }
        x.add8(Shot::SUB_STATE, 0xfe);
        x.set8(g::DELAY, 0x10);
        for_children(x, 6, |x, child| child.set_u8(x.b, Shot::SUB_STATE, 2));
        // rotation of the equipment row (6 slots, quantities at +8)
        let last = player.u8(x.b, Player::INVENTORY + 5);
        let last_count = player.u8(x.b, Player::QUANTITIES + 5);
        for k in (0..5u32).rev() {
            let (a, b) = (player.u8(x.b, Player::INVENTORY + k), player.u8(x.b, Player::QUANTITIES + k));
            player.set_u8(x.b, Player::INVENTORY + 1 + k, a);
            player.set_u8(x.b, Player::QUANTITIES + 1 + k, b);
        }
        player.set_u8(x.b, Player::INVENTORY, last);
        player.set_u8(x.b, Player::QUANTITIES, last_count);
    }

    fn close<R: Bench>(x: &mut Cx<'_, R>) {
        x.set8(Shot::SUB_STATE, 4);
        x.set8(g::DELAY, 8);
        for_children(x, 6, |x, child| child.set_u8(x.b, Shot::SUB_STATE, 6));
    }

    fn child<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(Shot::SUB_STATE) {
            0 => {
                x.add8(g::STEP, 1);
                if x.u8(g::STEP) > 4 {
                    x.set8(Shot::SUB_STATE, 8);
                } else {
                    let step = x.u8(g::STEP);
                    set_speeds(x, SPEEDS, step);
                }
            }
            2 => {
                x.add8(Shot::SUB_STATE, 2);
                let n = x.u8(Obj::VARIANT).wrapping_add(1);
                let n = if n > 5 { 0 } else { n };
                x.set8(Obj::VARIANT, n);
                let target = player_data().ring0_next_angle.at8(x.b, n as u32);
                x.set8(g::TARGET, target);
            }
            4 => {
                let angle = x.u8(Obj::SPEED_INDEX).wrapping_add(1) & 0x1f;
                x.set8(Obj::SPEED_INDEX, angle);
                if angle == x.u8(g::TARGET) {
                    x.add8(Shot::SUB_STATE, 0xfe);
                    icon(x);
                    let target = x.u8(g::TARGET);
                    x.set8(Obj::SPEED_INDEX, target);
                    let n = x.u8(Obj::VARIANT) as i8 as i16 as u16 as u32;
                    let next = player_data().ring0_next_angle.at8(x.b, n);
                    x.set8(g::TARGET, next);
                    x.set8(Shot::SUB_STATE, 8);
                }
            }
            6 => {
                if x.u8(g::STEP) == 0 {
                    x.set32(Obj::VX, 0);
                    x.set32(Obj::VZ, 0);
                } else {
                    x.add8(g::STEP, 0xff);
                    let step = x.u8(g::STEP);
                    set_speeds(x, SPEEDS, step);
                }
            }
            _ => x.set8(Shot::SUB_STATE, 8),
        }
        follow_parent(x);
        if !ring_alive(x, g::GENERATION) {
            x.set8(Obj::STATE, 4);
        }
        x.reinsert();
    }
}

/// Equipment ring of type 1 (0x4a21e): variant of 0 that reads the player's quantities.
mod ring1 {
    use super::*;

    /// Lifetime of the ring (word).
    const LIFE: u32 = 0x40;
    /// Set when an item is chosen (h).
    const CHOSEN: u32 = 0x56;
    /// Delay of the cursor (h).
    const DELAY: u32 = 0x5a;

    const OFFSETS: u32 = 0x4a3ac;
    const SPEEDS: u32 = 0x4a84c;
    /// Ring mode: `+0x59` is 2 for the row of magics.
    const MODE: u32 = 0x59;
    const GENERATION: u32 = 0x5b;
    const CHILD_COUNT: u32 = 0x3d;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(Obj::STATE) {
            0 => init(x),
            2 => {
                if (x.u8(Obj::VARIANT) as i8) < 0 {
                    parent(x)
                } else {
                    child(x)
                }
            }
            4 => {
                if (x.u8(Obj::VARIANT) as i8) < 0 {
                    let player = x.linked(f::OWNER);
                    player.set_u16(x.b, Player::SPECIAL_EFFECT, 0);
                    player.set_u8(x.b, Player::SPECIAL_ACTIVE, 0);
                }
                x.free_peff();
            }
            _ => x.free_peff(),
        }
    }

    fn player_is_8_or_9<R: Bench>(x: &mut Cx<'_, R>) -> bool {
        let player = x.linked(f::OWNER);
        let v = player.u8(x.b, Player::CHARACTER);
        v == 8 || v == 9
    }

    fn init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(Obj::STATE, 2);
        if (x.u8(Obj::VARIANT) as i8) < 0 {
            return parent(x);
        }
        let gen = vars::SCENE.get(x.b);
        x.set8(GENERATION, gen);
        x.set8(g::STEP, 0);
        let v = x.u8(Obj::VARIANT) as u32;
        let angle = player_data().ring1_angle_a.at8(x.b, v);
        x.set8(Obj::SPEED_INDEX, angle);
        if x.u8(MODE) != 2 {
            let angle = player_data().ring1_angle_b.at8(x.b, v);
            x.set8(Obj::SPEED_INDEX, angle);
            if !player_is_8_or_9(x) {
                let angle = player_data().ring1_angle_c.at8(x.b, v);
                x.set8(Obj::SPEED_INDEX, angle);
            }
        }
        set_speeds(x, SPEEDS, 0);
        icon(x);
    }

    /// Icon animation (`0x4a29c`).
    fn icon<R: Bench>(x: &mut Cx<'_, R>) {
        let item = x.u8(g::ITEM) as u32;
        let count = x.u8(g::COUNT) as u32;
        let (d0, d1) = if x.u8(MODE) == 2 {
            (item << 4 & 0xffff, if x.u8(Obj::SPEED_INDEX) != 0 { 0xa } else { count })
        } else {
            let player = x.linked(f::OWNER);
            let kind = player.u8(x.b, Player::CHARACTER) >> 1;
            let item = if kind == 2 && item == 0xf { 0x57 } else { item };
            let d0w = (item << 4) & 0xffff;
            let stock = player.u8(x.b, Player::SPELL_RESERVE + count);
            (d0w, if x.u8(Obj::SPEED_INDEX) != 0 {
                if stock == 0 { 0xb } else { 0xa }
            } else {
                stock as u32
            })
        };
        let set = if vars::ALT_PALETTE.get(x.b) != 0 { ICONS_ALT.0 } else { ICONS.0 };
        x.anim_start(set, ((d0 + d1) & 0xffff) as u16);
    }

    fn parent<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(Shot::SUB_STATE) {
            0 => create(x),
            2 => {
                x.add8(g::DELAY, 0xff);
                if x.u8(g::DELAY) == 0 {
                    x.add8(Shot::SUB_STATE, 2);
                }
            }
            4 => rotate_or_close(x),
            6 => {
                x.add8(g::DELAY, 0xff);
                if x.u8(g::DELAY) == 0 {
                    x.set8(Shot::SUB_STATE, 0);
                    let player = x.linked(f::OWNER);
                    let n = player.u8(x.b, Player::SPECIAL_COUNT).wrapping_add(1);
                    player.set_u8(x.b, Player::SPECIAL_COUNT, n);
                    if n == 3 {
                        player.set_u8(x.b, Player::SPECIAL_COUNT, 0);
                    }
                }
            }
            _ => {
                x.add8(g::DELAY, 0xff);
                if x.u8(g::DELAY) == 0 {
                    x.add8(Obj::STATE, 2);
                    let n = x.u8(CHILD_COUNT) as u32 + 1;
                    for_children(x, n, |x, child| {
                        let v = child.u8(x.b, Obj::STATE).wrapping_add(2);
                        child.set_u8(x.b, Obj::STATE, v)
                    });
                }
            }
        }
        // follows the player
        parent_follow(x, OFFSETS);
        let player = x.linked(f::OWNER);
        if parent_alive(x, GENERATION, player) {
            return;
        }
        x.set8(Obj::STATE, 4);
    }

    /// Substate 0 of the parent: prepares the ring (number of children and their equipment).
    fn create<R: Bench>(x: &mut Cx<'_, R>) {
        if pools::PEFF.free_count(x.b) <= 6 {
            return;
        }
        x.add8(Shot::SUB_STATE, 2);
        let gen = vars::SCENE.get(x.b);
        x.set8(GENERATION, gen);
        x.set16(LIFE, 0x258);
        x.set8(g::DELAY, 0x10);
        let player = x.linked(f::OWNER);
        let page = player.u8(x.b, Player::SPECIAL_COUNT);
        x.set8(MODE, page);
        let mut count = player_data().ring1_counts_magic_user.at8(x.b, page as u32);
        if !matches!(player.u8(x.b, Player::CHARACTER), 8 | 9) {
            count = player_data().ring1_counts.at8(x.b, page as u32);
        }
        x.set8(CHILD_COUNT, count);
        let mut a2 = x.o.0 + g::CHILDREN;
        for k in 0..=count as u32 {
            // Original glitch: the allocation failure is not tested (see `Pool::alloc_unchecked`).
            let child = x.alloc_unchecked(&pools::PEFF);
            child.set_u8(x.b, Obj::ACTIVE, 1);
            child.set_u8(x.b, Obj::KIND, 1);
            child.set_u8(x.b, Obj::VARIANT, k as u8);
            child.set_u8(x.b, MODE, page);
            child.set_u16(x.b, g::PARENT, x.o.0 as u16);
            x.set16(a2 - x.o.0, child.0 as u16);
            a2 += 2;
            let owner = x.u16(f::OWNER);
            child.set_u16(x.b, f::OWNER, owner);
            let at = ((page as u32) << 4) + k;
            let item = player.u8(x.b, Player::SLOT_TABLE + at);
            let amount = player.u8(x.b, Player::SLOT_COUNTS + at);
            child.set_u8(x.b, g::ITEM, item);
            child.set_u8(x.b, g::COUNT, amount);
        }
    }

    fn rotate_or_close<R: Bench>(x: &mut Cx<'_, R>) {
        x.add16(LIFE, 0xffff);
        if x.u16(LIFE) == 0 {
            return shut(x, 8);
        }
        if vars::PAUSED.get(x.b) != 0 {
            return shut(x, 8);
        }
        let player = x.linked(f::OWNER);
        let keys = player.u8(x.b, Player::KEYS_NEW);
        if keys & 0x40 != 0 {
            return turn(x, player);
        }
        if keys & 0x20 != 0 {
            return shut(x, 6);
        }
        if keys & 0x10 != 0 || keys & 0x80 != 0 {
            return shut(x, 8);
        }
        let held = player.u8(x.b, Player::KEYS_HELD);
        if held & 0x40 != 0 {
            turn(x, player);
        }
    }

    /// Closes the ring: all the children in substate 6, the parent in `sub`.
    fn shut<R: Bench>(x: &mut Cx<'_, R>, sub: u8) {
        x.set8(Shot::SUB_STATE, sub);
        x.set8(g::DELAY, 8);
        let n = x.u8(CHILD_COUNT) as u32 + 1;
        for_children(x, n, |x, child| child.set_u8(x.b, Shot::SUB_STATE, 6));
    }

    /// Rotates the row of equipment of the current page.
    fn turn<R: Bench>(x: &mut Cx<'_, R>, player: Obj) {
        x.add8(Shot::SUB_STATE, 0xfe);
        x.set8(g::DELAY, 0x10);
        let n = x.u8(CHILD_COUNT) as u32 + 1;
        for_children(x, n, |x, child| child.set_u8(x.b, Shot::SUB_STATE, 2));
        let n = x.u8(CHILD_COUNT);
        let base = player.0 + 0x120 + ((x.u8(MODE) as u32) << 4);
        let last = base + n as u32;
        let (keep, keep_count) = (byte(x.b, last), byte(x.b, last + 8));
        let mut dst = last;
        let mut src = last - 1;
        for _ in 0..n {
            let (a, b) = (byte(x.b, src), byte(x.b, src + 8));
            crate::hw::mem::wr8(x.b, dst, a);
            crate::hw::mem::wr8(x.b, dst + 8, b);
            src = src.wrapping_sub(1);
            dst = dst.wrapping_sub(1);
        }
        crate::hw::mem::wr8(x.b, base, keep);
        crate::hw::mem::wr8(x.b, base + 8, keep_count);
    }

    fn child<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(Shot::SUB_STATE) {
            0 => {
                x.add8(g::STEP, 1);
                if x.u8(g::STEP) > 4 {
                    x.set8(Shot::SUB_STATE, 8);
                } else {
                    let step = x.u8(g::STEP);
                    set_speeds(x, SPEEDS, step);
                }
            }
            2 => {
                x.add8(Shot::SUB_STATE, 2);
                let parent = x.linked(g::PARENT);
                x.set8(CHOSEN, 1);
                let n = x.u8(Obj::VARIANT).wrapping_add(1);
                let n = if n > parent.u8(x.b, CHILD_COUNT) { 0 } else { n };
                x.set8(Obj::VARIANT, n);
                let mut target = player_data().ring1_next_a.at8(x.b, n as u32);
                x.set8(g::TARGET, target);
                if x.u8(MODE) != 2 {
                    target = player_data().ring1_next_b.at8(x.b, n as u32);
                    x.set8(g::TARGET, target);
                    if !player_is_8_or_9(x) {
                        target = player_data().ring1_next_c.at8(x.b, n as u32);
                        x.set8(g::TARGET, target);
                    }
                }
            }
            4 => {
                let angle = x.u8(Obj::SPEED_INDEX).wrapping_add(1) & 0x1f;
                x.set8(Obj::SPEED_INDEX, angle);
                if angle == x.u8(g::TARGET) {
                    x.add8(Shot::SUB_STATE, 0xfe);
                    icon(x);
                    let target = x.u8(g::TARGET);
                    x.set8(Obj::SPEED_INDEX, target);
                    let n = x.u8(Obj::VARIANT) as i8 as i16 as u16 as u32;
                    let mut next = player_data().ring1_next2_a.at8(x.b, n);
                    x.set8(g::TARGET, next);
                    x.set8(Shot::SUB_STATE, 8);
                    if x.u8(MODE) != 2 {
                        next = player_data().ring1_next2_b.at8(x.b, n);
                        x.set8(g::TARGET, next);
                        if !player_is_8_or_9(x) {
                            next = player_data().ring1_next2_b.at8(x.b, n);
                            x.set8(g::TARGET, next);
                        }
                    }
                }
            }
            6 => {
                if x.u8(g::STEP) == 0 {
                    x.set8(Shot::SUB_STATE, 0xa);
                    x.set8(DELAY, 4);
                    set_speeds(x, SPEEDS, 0);
                } else {
                    x.add8(g::STEP, 0xff);
                    let step = x.u8(g::STEP);
                    set_speeds(x, SPEEDS, step);
                }
            }
            0xa => {
                if x.u8(DELAY) == 0 {
                    x.add8(Obj::STATE, 2);
                }
                x.add8(DELAY, 0xff);
            }
            0xc => {
                if x.u8(g::STEP) == 0 {
                    x.set32(Obj::VX, 0);
                    x.set32(Obj::VZ, 0);
                } else {
                    x.add8(g::STEP, 0xff);
                    let step = x.u8(g::STEP);
                    set_speeds(x, SPEEDS, step);
                }
            }
            _ => x.set8(Shot::SUB_STATE, 8),
        }
        follow_parent(x);
        if !ring_alive(x, GENERATION) {
            x.set8(Obj::STATE, 4);
        }
        x.reinsert();
    }
}

handlers! {
    PEFF_0 0x49e36 "peff_0" ring0::run,
    PEFF_1 0x4a21e "peff_1" ring1::run,
}
