//! Display event queues (U1): score points, second counter, pop-ups.
//!
//! Three rings in work RAM, cleared in one go at initialisation. An empty cell is 0xFFFF.
//!
//! | Ring | Write / read index | Cells | Role |
//! |---|---|---|---|
//! | points | `A5+0x108` / `A5+0x109` | 64 words at `A5+0x10a` | score events |
//! | second counter | `A5+0x18a` / `A5+0x18b` | 64 words at `A5+0x18c` | same, 5-digit counter |
//! | pop-ups | `A5+0x20c` / `A5+0x20d` | 32 entries of 8 bytes at `A5+0x20e` | floating texts |
//!
//! A score event is a word `(player << 8) | type`: the player is read from the emitting object
//! (word `+0x48`, bits 8-9), the type is its byte `+0x7e`. Each frame, the players' interface task
//! (0x15200) empties the rings: the BCD value associated with the type (ROM table, 4 bytes per
//! type) is added to the player's BCD counter (`+0x110`, 8 digits; `+0x114`, 5 digits), capped.
//!
//! Cores without registers (`clear_points`, `enqueue_points`, `object_event`, `credit_points`, `create_popup`...)
//! and `_68k` natives for the calls by address: these reproduce the registers and flags of the original,
//! which hundreds of callers (enemy objects...) may reread.

use cps2::Cps2Bus;

use crate::objects::{Enemy, Obj};
use crate::ops;
use crate::ram::{Byte, Long, Word};

pub(crate) const EMPTY: u16 = 0xffff;
/// The ring indices advance by 2 (bytes) modulo 0x80.
pub(crate) const MASK: u8 = 0x7f;

/// A score event ring.
pub(crate) struct Ring {
    /// Write index (byte); the read index is the next byte.
    write: u32,
    /// First cell (words).
    pub(crate) cases: u32,
}

pub(crate) const POINTS: Ring = Ring { write: 0x108, cases: 0x10a };
pub(crate) const COUNTER: Ring = Ring { write: 0x18a, cases: 0x18c };

/// Pop-up queue: write and read index (bytes), first entry.
const POPUP_WRITE: u32 = 0x20c;
const POPUP_READ: u32 = 0x20d;
pub(crate) const POPUP_INPUTS: u32 = 0x20e;
/// Current pop-up (4 longs at `A5-0x6e80`).
pub(crate) const CURRENT_POPUP: u32 = 0xffff_9180;
/// Non-zero byte: no more new pop-ups.
pub(crate) const POPUP_INHIBITED: u32 = 0x50;

/// Player records: the first and the size (0x200).
const PLAYER_1: u32 = 0x5de;

/// Address `A5 + off` (A5 is always `mem::A5`).
pub(crate) const fn a5(off: u32) -> u32 {
    crate::mem::A5.wrapping_add(off)
}

// ---------------------------------------------------------------------------------------------
// Clearing

/// `n` longs of 0xFFFFFFFF from `start`.
fn fill(b: &mut Cps2Bus, start: u32, n: u32) {
    for k in 0..n {
        Long(start + 4 * k).set(b, 0xffff_ffff);
    }
}

pub(crate) fn clear_ring(b: &mut Cps2Bus, ring: &Ring) {
    Byte::a5(ring.write).set(b, 0);
    Byte::a5(ring.write + 1).set(b, 0);
    fill(b, a5(ring.cases), 32);
}

/// 0x19aa2: clears the points ring.
pub fn clear_points(b: &mut Cps2Bus) {
    clear_ring(b, &POINTS);
}

/// 0x19abc: clears the second counter ring.
pub fn clear_counter(b: &mut Cps2Bus) {
    clear_ring(b, &COUNTER);
}

/// 0x19db4: clears the pop-up queue.
pub fn clear_popups(b: &mut Cps2Bus) {
    Byte::a5(POPUP_WRITE).set(b, 0);
    Byte::a5(POPUP_READ).set(b, 0);
    fill(b, a5(POPUP_INPUTS), 64);
}

/// 0x19e10: clears the current pop-up (4 longs).
pub fn clear_current_popup(b: &mut Cps2Bus) {
    for k in 0..4 {
        Long(a5(CURRENT_POPUP) + 4 * k).set(b, 0);
    }
}

// ---------------------------------------------------------------------------------------------
// Enqueuing a score event

/// Write cell of `ring`: its index and its address.
pub(crate) fn write_cell(b: &mut Cps2Bus, ring: &Ring) -> (u8, u32) {
    let index = Byte::a5(ring.write).get(b);
    (index, a5(ring.cases).wrapping_add(index as u32))
}

/// Enqueues `event` in the write cell if it is free; returns true if enqueued.
pub(crate) fn enqueue(b: &mut Cps2Bus, ring: &Ring, event: u16) -> bool {
    let (index, case) = write_cell(b, ring);
    if Word(case).get(b) != EMPTY {
        return false;
    }
    Word(case).set(b, event);
    Byte::a5(ring.write).set(b, index.wrapping_add(2) & MASK);
    true
}

/// 0x19ae6: enqueues the score event `event` (`(player << 8) | type`) if the write cell is free;
/// returns true if enqueued.
pub fn enqueue_points(b: &mut Cps2Bus, event: u16) -> bool {
    enqueue(b, &POINTS, event)
}

/// 0x19b08: same in the second counter ring.
pub fn enqueue_counter(b: &mut Cps2Bus, event: u16) -> bool {
    enqueue(b, &COUNTER, event)
}

/// Score event of object `o`: type `+0x7e`, player `+0x48` bits 8-9.
pub fn event_of(b: &mut Cps2Bus, o: Obj) -> u16 {
    let type_ = Enemy(o).event_kind(b) as u16;
    (o.u16(b, Enemy::HIT_BY) & 0x300).wrapping_add(type_)
}

/// 0x19ad6: enqueues the score event of object `o` in the points ring.
pub fn object_event(b: &mut Cps2Bus, o: Obj) -> bool {
    let event = event_of(b, o);
    enqueue_points(b, event)
}

// ---------------------------------------------------------------------------------------------
// Crediting the points

/// A ring to empty, with the value table and the player counter to increment.
pub(crate) struct Credit {
    pub(crate) ring: Ring,
    /// Read index (byte).
    pub(crate) read: u32,
    /// ROM table of BCD values (4 bytes per type, in `treasure_bonus` of `data/scenes.json`).
    pub(crate) table: u32,
    /// Offset of the BCD counter in the player record.
    pub(crate) counter: u32,
    /// BCD cap (signed 32-bit comparison).
    cap: u32,
}

pub(crate) const CREDIT_POINTS: Credit = Credit {
    ring: POINTS,
    read: 0x109,
    table: 0x19c0e,
    counter: 0x110,
    cap: 0x0999_9999,
};
pub(crate) const CREDIT_COUNTER: Credit = Credit {
    ring: COUNTER,
    read: 0x18b,
    table: 0x19d52,
    counter: 0x114,
    cap: 0x0009_9999,
};

/// Player record and line of the value table (type x 4) of an event.
pub(crate) fn credited(event: u16) -> (u32, u32) {
    let player = a5(PLAYER_1) + (((event & 0x300) as u32) << 1);
    (player, (event as u32 & 0xff) << 2)
}

/// Processes the event of cell `index`: empties the cell, adds the BCD value of the type to the player's
/// counter, caps it, advances the read index.
fn handle(b: &mut Cps2Bus, k: &Credit, index: u8, event: u16) {
    Word(a5(k.ring.cases).wrapping_add(index as u32)).set(b, EMPTY);
    let (player, line) = credited(event);
    let (table, counter) = (k.table + line, player + k.counter);
    // abcd -(A3),-(A4) ×4: low-order byte first, chained carry, initial X zero
    let mut carry = false;
    for i in (0..4).rev() {
        let value = crate::tables::engine().byte(b, table + i);
        let (sum, x) = ops::abcd(value, Byte(counter + i).get(b), carry);
        Byte(counter + i).set(b, sum);
        carry = x;
    }
    // cmpi.l #cap,(A4) ; blt: replaced by the cap if the value is not lower
    let capped = Long(counter).get(b) as i32 >= k.cap as i32;
    if capped {
        Long(counter).set(b, k.cap);
    }
    Byte::a5(k.read).set(b, index.wrapping_add(2) & MASK);
}

/// Event waiting at the read index, if there is one.
fn pending(b: &mut Cps2Bus, k: &Credit) -> Option<(u8, u16)> {
    let index = Byte::a5(k.read).get(b);
    let event = Word(a5(k.ring.cases).wrapping_add(index as u32)).get(b);
    (event != EMPTY).then_some((index, event))
}

/// Credits every waiting event, in order.
pub(crate) fn credit(b: &mut Cps2Bus, k: &Credit) {
    while let Some((index, event)) = pending(b, k) {
        handle(b, k, index, event);
    }
}

/// 0x19b2a: credits the waiting events to each player's points counter.
pub fn credit_points(b: &mut Cps2Bus) {
    credit(b, &CREDIT_POINTS);
}

/// 0x19b9c: same for the second counter.
pub fn credit_counter(b: &mut Cps2Bus) {
    credit(b, &CREDIT_COUNTER);
}

// ---------------------------------------------------------------------------------------------
// Pop-ups

/// A pop-up (health bar of a hit monster, `player::external::effect::HEALTH_BAR`): D0, D1, D2 of the original.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Popup {
    /// Player and type (D0.w): the low byte gains 0x20 unless `raw`.
    pub header: u16,
    /// D1.w, D2.w: maximum HP and HP (h).
    pub max: u16,
    pub value: u16,
    /// D3.b = 2: the header is taken as is.
    pub raw: bool,
}

/// Write entry of the pop-up queue: its index and its address.
pub(crate) fn popup_write_entry(b: &mut Cps2Bus) -> (u8, u32) {
    let index = Byte::a5(POPUP_WRITE).get(b);
    (index, a5(POPUP_INPUTS).wrapping_add(index as u32))
}

/// 0x19dce: enqueues a pop-up in the write cell if it is free and pop-ups are not inhibited;
/// returns true if enqueued.
pub fn create_popup(b: &mut Cps2Bus, p: Popup) -> bool {
    if Byte::a5(POPUP_INHIBITED).is_set(b) {
        return false;
    }
    let (index, case) = popup_write_entry(b);
    if Word(case).get(b) != EMPTY {
        return false;
    }
    let header = if p.raw { p.header } else { p.header & 0xff00 | (p.header as u8).wrapping_add(0x20) as u16 };
    for (offset, value) in [(0, header), (2, p.max), (4, p.value), (6, 0x80)] {
        Word(case + offset).set(b, value);
    }
    // the index advances by 8 without a mask: it goes around the 32 entries by overflowing its byte
    Byte::a5(POPUP_WRITE).set(b, index.wrapping_add(8));
    true
}

/// Are pop-ups inhibited (`A5+0x50`)? 0x19dce then writes nothing and leaves A2 intact; otherwise it leaves
/// A2 = [`POPUP_INPUTS_ADDR`] (reread by combat: `combat::show_enemy_hp`).
pub fn popups_inhibited(b: &mut Cps2Bus) -> bool {
    Byte::a5(POPUP_INHIBITED).is_set(b)
}

/// First entry of the pop-up queue (A2 left by 0x19dce).
pub const POPUP_INPUTS_ADDR: u32 = a5(POPUP_INPUTS);
