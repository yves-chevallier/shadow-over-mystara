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
use m68k::decode::Size;
use m68k::{Cpu, SR_X};

use crate::mem::{rd16, rd32, rd8, rts, set_low_byte, wr16, wr32, wr8};
use crate::ops;
use crate::{regs, Native};

const EMPTY: u16 = 0xffff;
/// The ring indices advance by 2 (bytes) modulo 0x80.
const MASK: u8 = 0x7f;

/// A score event ring.
struct Ring {
    /// Write index (byte); the read index is the next byte.
    write: u32,
    /// First cell (words).
    cases: u32,
}

const POINTS: Ring = Ring { write: 0x108, cases: 0x10a };
const COUNTER: Ring = Ring { write: 0x18a, cases: 0x18c };

/// Pop-up queue: write and read index (bytes), first entry.
const POPUP_WRITE: u32 = 0x20c;
const POPUP_READ: u32 = 0x20d;
const POPUP_INPUTS: u32 = 0x20e;
/// Current pop-up (4 longs at `A5-0x6e80`).
const CURRENT_POPUP: u32 = 0xffff_9180;
/// Non-zero byte: no more new pop-ups.
const POPUP_INHIBITED: u32 = 0x50;

/// Player records: the first and the size (0x200).
const PLAYER_1: u32 = 0x5de;

/// Address `A5 + off` (A5 is always `mem::A5`).
const fn a5(off: u32) -> u32 {
    crate::mem::A5.wrapping_add(off)
}

fn set_word(r: &mut u32, v: u16) {
    *r = *r & 0xffff_0000 | v as u32;
}

// ---------------------------------------------------------------------------------------------
// Clearing

/// `n` longs of 0xFFFFFFFF from `start`.
fn fill(b: &mut Cps2Bus, start: u32, n: u32) {
    for k in 0..n {
        wr32(b, start + 4 * k, 0xffff_ffff);
    }
}

/// Registers of `move.l D1,(A)+ ; dbf D0`: D1 = -1, low word of D0 = 0xFFFF, the address register `ptr` returns the end
/// of the zone.
fn fill_68k(c: &mut Cpu, end: u32, ptr: usize) {
    c.a[ptr] = end;
    set_word(&mut c.d[0], 0xffff);
    c.d[1] = 0xffff_ffff;
    ops::logic(c, 0xffff_ffff, Size::Long);
}

fn clear_ring(b: &mut Cps2Bus, ring: &Ring) {
    wr8(b, a5(ring.write), 0);
    wr8(b, a5(ring.write + 1), 0);
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
    wr8(b, a5(POPUP_WRITE), 0);
    wr8(b, a5(POPUP_READ), 0);
    fill(b, a5(POPUP_INPUTS), 64);
}

/// 0x19e10: clears the current pop-up (4 longs).
pub fn clear_current_popup(b: &mut Cps2Bus) {
    for k in 0..4 {
        wr32(b, a5(CURRENT_POPUP) + 4 * k, 0);
    }
}

fn clear_ring_68k(c: &mut Cpu, b: &mut Cps2Bus, ring: &Ring) {
    clear_ring(b, ring);
    fill_68k(c, a5(ring.cases) + 4 * 32, 0);
    rts(c, b);
}

fn clear_popups_68k(c: &mut Cpu, b: &mut Cps2Bus) {
    clear_popups(b);
    fill_68k(c, a5(POPUP_INPUTS) + 4 * 64, 1);
    rts(c, b);
}

/// Leaves A1 after the zone, D0 = 0, D6 = 0xFFFF.
fn clear_popup_68k(c: &mut Cpu, b: &mut Cps2Bus) {
    clear_current_popup(b);
    c.a[1] = a5(CURRENT_POPUP) + 16;
    c.d[0] = 0;
    c.d[6] = 0xffff;
    ops::logic(c, 0, Size::Long);
    rts(c, b);
}

// ---------------------------------------------------------------------------------------------
// Enqueuing a score event

/// Outcome of an enqueue: the write index read, and the content of the cell if it was not free.
struct Queued {
    index: u8,
    busy: Option<u16>,
}

fn enqueue(b: &mut Cps2Bus, ring: &Ring, event: u16) -> Queued {
    let index = rd8(b, a5(ring.write));
    let case = a5(ring.cases).wrapping_add(index as u32);
    let content = rd16(b, case);
    if content != EMPTY {
        return Queued { index, busy: Some(content) };
    }
    wr16(b, case, event);
    wr8(b, a5(ring.write), index.wrapping_add(2) & MASK);
    Queued { index, busy: None }
}

/// 0x19ae6: enqueues the score event `event` (`(player << 8) | type`) if the write cell is free;
/// returns true if enqueued.
pub fn enqueue_points(b: &mut Cps2Bus, event: u16) -> bool {
    enqueue(b, &POINTS, event).busy.is_none()
}

/// 0x19b08: same in the second counter ring.
pub fn enqueue_counter(b: &mut Cps2Bus, event: u16) -> bool {
    enqueue(b, &COUNTER, event).busy.is_none()
}

/// Score event of object `o`: type `+0x7e`, player `+0x48` bits 8-9.
pub fn event_of(b: &mut Cps2Bus, o: crate::objects::Obj) -> u16 {
    let type_ = rd8(b, o.0 + 0x7e) as u16;
    (rd16(b, o.0 + 0x48) & 0x300).wrapping_add(type_)
}

/// 0x19ad6: enqueues the score event of object `o` in the points ring.
pub fn object_event(b: &mut Cps2Bus, o: crate::objects::Obj) -> bool {
    let event = event_of(b, o);
    enqueue_points(b, event)
}

/// Registers and flags of 0x19ae6 / 0x19b08: D6 (index), A4 (start of the cells).
fn enqueue_68k(c: &mut Cpu, b: &mut Cps2Bus, ring: &Ring, event: u16) {
    let e = enqueue(b, ring, event);
    c.d[6] = e.index as u32;
    c.a[4] = a5(ring.cases);
    match e.busy {
        Some(content) => ops::cmp(c, EMPTY as u32, content as u32, Size::Word),
        None => {
            ops::add(c, 2, e.index as u32, Size::Byte);
            let reduced = e.index.wrapping_add(2) & MASK;
            set_word(&mut c.d[6], reduced as u16);
            ops::logic(c, reduced as u32, Size::Byte);
        }
    }
}

fn enqueue_points_68k(c: &mut Cpu, b: &mut Cps2Bus) {
    enqueue_68k(c, b, &POINTS, c.d[0] as u16);
    rts(c, b);
}

fn enqueue_counter_68k(c: &mut Cpu, b: &mut Cps2Bus) {
    enqueue_68k(c, b, &COUNTER, c.d[0] as u16);
    rts(c, b);
}

/// 0x19ad6: D0 = the event, D1.w = the player (bits 8-9).
fn object_event_68k(c: &mut Cpu, b: &mut Cps2Bus) {
    let o = crate::objects::Obj(c.a[0]);
    let type_ = rd8(b, o.0 + 0x7e) as u32;
    c.d[0] = type_;
    let player = rd16(b, o.0 + 0x48) & 0x300;
    set_word(&mut c.d[1], player);
    let event = ops::add(c, player as u32, type_, Size::Word) as u16;
    set_word(&mut c.d[0], event);
    enqueue_68k(c, b, &POINTS, event);
    rts(c, b);
}

// ---------------------------------------------------------------------------------------------
// Crediting the points

/// A ring to empty, with the value table and the player counter to increment.
struct Credit {
    ring: Ring,
    /// Read index (byte).
    read: u32,
    /// ROM table of BCD values (4 bytes per type).
    table: u32,
    /// Offset of the BCD counter in the player record.
    counter: u32,
    /// BCD cap (signed 32-bit comparison).
    cap: u32,
}

const CREDIT_POINTS: Credit = Credit {
    ring: POINTS,
    read: 0x109,
    table: 0x19c0e,
    counter: 0x110,
    cap: 0x0999_9999,
};
const CREDIT_COUNTER: Credit = Credit {
    ring: COUNTER,
    read: 0x18b,
    table: 0x19d52,
    counter: 0x114,
    cap: 0x0009_9999,
};

/// What the processing of an event leaves in the registers.
struct Handled {
    player: u32,
    line: u32,
}

/// Processes the event of cell `index`: empties the cell, adds the BCD value of the type to the player's
/// counter, caps it, advances the read index.
fn handle(b: &mut Cps2Bus, k: &Credit, index: u8, event: u16) -> Handled {
    wr16(b, a5(k.ring.cases).wrapping_add(index as u32), EMPTY);
    let player = a5(PLAYER_1) + (((event & 0x300) as u32) << 1);
    let line = (event as u32 & 0xff) << 2;
    let (table, counter) = (k.table + line, player + k.counter);
    // abcd -(A3),-(A4) ×4: low-order byte first, chained carry, initial X zero
    let mut carry = false;
    for i in (0..4).rev() {
        let (sum, x) = ops::abcd(rd8(b, table + i), rd8(b, counter + i), carry);
        wr8(b, counter + i, sum);
        carry = x;
    }
    // cmpi.l #cap,(A4) ; blt: replaced by the cap if the value is not lower
    let capped = rd32(b, counter) as i32 >= k.cap as i32;
    if capped {
        wr32(b, counter, k.cap);
    }
    wr8(b, a5(k.read), index.wrapping_add(2) & MASK);
    Handled { player, line }
}

/// Event waiting at the read index, if there is one.
fn pending(b: &mut Cps2Bus, k: &Credit) -> Option<(u8, u16)> {
    let index = rd8(b, a5(k.read));
    let event = rd16(b, a5(k.ring.cases).wrapping_add(index as u32));
    (event != EMPTY).then_some((index, event))
}

/// Last event processed by a credit: enough to rebuild the registers of the original.
struct Last {
    index: u8,
    handled: Handled,
}

fn credit(b: &mut Cps2Bus, k: &Credit) -> Option<Last> {
    let mut last = None;
    while let Some((index, event)) = pending(b, k) {
        let handled = handle(b, k, index, event);
        last = Some(Last { index, handled });
    }
    last
}

/// 0x19b2a: credits the waiting events to each player's points counter.
pub fn credit_points(b: &mut Cps2Bus) {
    credit(b, &CREDIT_POINTS);
}

/// 0x19b9c: same for the second counter.
pub fn credit_counter(b: &mut Cps2Bus) {
    credit(b, &CREDIT_COUNTER);
}

/// Registers of 0x19b2a / 0x19b9c: leftovers of the last iteration, D0 = final read index.
fn credit_68k(c: &mut Cpu, b: &mut Cps2Bus, k: &Credit) {
    c.d[1] = 0;
    c.d[2] = 0;
    c.a[2] = a5(k.ring.cases);
    let last = credit(b, k);
    if let Some(d) = &last {
        c.d[0] = d.index as u32;
        c.d[1] = d.handled.line;
        c.a[0] = d.handled.player;
        c.a[3] = k.table + d.handled.line;
        c.a[4] = d.handled.player + k.counter;
        c.d[6] = 0xffff;
    }
    c.d[0] = rd8(b, a5(k.read)) as u32;
    // last cmpi.w #-1: equality; X: carry of the `addq.b #2` on the read index of the last iteration
    ops::cmp(c, EMPTY as u32, EMPTY as u32, Size::Word);
    if let Some(d) = last {
        ops::flags(c, SR_X, if d.index as u16 + 2 > 0xff { SR_X } else { 0 });
    }
    rts(c, b);
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

/// Outcome of 0x19dce: nothing (pop-ups inhibited), cell occupied, or enqueued.
enum Creation {
    Inhibited(u8),
    Busy { index: u8, content: u16 },
    Done { index: u8, header: u16 },
}

fn create(b: &mut Cps2Bus, p: Popup) -> Creation {
    let inhibited = rd8(b, a5(POPUP_INHIBITED));
    if inhibited != 0 {
        return Creation::Inhibited(inhibited);
    }
    let index = rd8(b, a5(POPUP_WRITE));
    let case = a5(POPUP_INPUTS).wrapping_add(index as u32);
    let content = rd16(b, case);
    if content != EMPTY {
        return Creation::Busy { index, content };
    }
    let header = if p.raw { p.header } else { p.header & 0xff00 | (p.header as u8).wrapping_add(0x20) as u16 };
    for (offset, value) in [(0, header), (2, p.max), (4, p.value), (6, 0x80)] {
        wr16(b, case + offset, value);
    }
    // the index advances by 8 without a mask: it goes around the 32 entries by overflowing its byte
    wr8(b, a5(POPUP_WRITE), index.wrapping_add(8));
    Creation::Done { index, header }
}

/// Are pop-ups inhibited (`A5+0x50`)? 0x19dce then writes nothing and leaves A2 intact; otherwise it leaves
/// A2 = [`POPUP_INPUTS_ADDR`] (reread by combat: `combat::show_enemy_hp`).
pub fn popups_inhibited(b: &mut Cps2Bus) -> bool {
    rd8(b, a5(POPUP_INHIBITED)) != 0
}

/// First entry of the pop-up queue (A2 left by 0x19dce).
pub const POPUP_INPUTS_ADDR: u32 = a5(POPUP_INPUTS);

/// 0x19dce: enqueues a pop-up in the write cell if it is free and pop-ups are not inhibited;
/// returns true if enqueued.
pub fn create_popup(b: &mut Cps2Bus, p: Popup) -> bool {
    matches!(create(b, p), Creation::Done { .. })
}

/// Registers and flags of 0x19dce: D6 (index), A2 (entries), D0.b (header), flags of the last test.
fn create_popup_68k(c: &mut Cpu, b: &mut Cps2Bus) {
    let p = Popup { header: c.d[0] as u16, max: c.d[1] as u16, value: c.d[2] as u16, raw: c.d[3] as u8 == 2 };
    match create(b, p) {
        Creation::Inhibited(inhibited) => ops::logic(c, inhibited as u32, Size::Byte),
        Creation::Busy { index, content } => {
            c.d[6] = index as u32;
            c.a[2] = a5(POPUP_INPUTS);
            ops::cmp(c, EMPTY as u32, content as u32, Size::Word);
        }
        Creation::Done { index, header } => {
            c.a[2] = a5(POPUP_INPUTS);
            ops::cmp(c, 2, c.d[3] & 0xff, Size::Byte);
            if !p.raw {
                ops::add(c, 0x20, c.d[0] & 0xff, Size::Byte);
                set_low_byte(&mut c.d[0], header as u8);
            }
            let next = ops::add(c, 8, index as u32, Size::Byte) as u8;
            c.d[6] = next as u32;
            ops::logic(c, next as u32, Size::Byte);
        }
    }
    rts(c, b);
}

macro_rules! native {
    ($ident:ident, $entry:expr, $name:expr, $run:expr) => {
        pub const $ident: Native = Native {
            entry: $entry,
            name: $name,
            run: crate::Body::WithCpu($run),
            touches_sound: false,
            outputs: regs::ALL,
        };
    };
}

native!(CLEAR_POINTS, 0x19aa2, "evt_clear_points", |c, b| clear_ring_68k(c, b, &POINTS));
native!(CLEAR_COUNTER, 0x19abc, "evt_clear_counter", |c, b| clear_ring_68k(c, b, &COUNTER));
native!(CLEAR_POPUPS, 0x19db4, "evt_clear_popups", clear_popups_68k);
native!(CLEAR_POPUP, 0x19e10, "evt_clear_popup", clear_popup_68k);
native!(ENQUEUE_POINTS, 0x19ae6, "evt_enqueue_points", enqueue_points_68k);
native!(ENQUEUE_COUNTER, 0x19b08, "evt_enqueue_counter", enqueue_counter_68k);
native!(OBJECT_EVENT, 0x19ad6, "evt_event_object", object_event_68k);
native!(CREATE_POPUP, 0x19dce, "evt_create_popup", create_popup_68k);
native!(CREDIT_POINTS_NATIVE, 0x19b2a, "evt_credit_points", |c, b| credit_68k(c, b, &CREDIT_POINTS));
native!(CREDIT_COUNTER_NATIVE, 0x19b9c, "evt_credit_counter", |c, b| credit_68k(c, b, &CREDIT_COUNTER));
