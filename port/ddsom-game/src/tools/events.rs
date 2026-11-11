//! Tools: the natives of `crate::world::events` for the original code that calls them (arguments and results in the registers
//! of the original).

use crate::world::World;
use m68k::decode::Size;
use m68k::{Cpu, SR_X};

use crate::world::events::*;
use crate::tools::frames::*;
use crate::{regs, Native};
use crate::hw::mem::{rd16, rd8, set_low_byte};

macro_rules! native {
    ($ident:ident, $entry:expr, $name:expr, $run:expr) => {
        pub(crate) const $ident: Native = Native {
            entry: $entry,
            name: $name,
            run: crate::Body::WithCpu($run),
            touches_sound: false,
            outputs: regs::ALL,
        };
    };
}

/// Registers of `move.l D1,(A)+ ; dbf D0`: D1 = -1, low word of D0 = 0xFFFF, the address register `ptr` returns the end
/// of the zone.
fn fill_68k(c: &mut Cpu, end: u32, ptr: usize) {
    c.a[ptr] = end;
    set_word(&mut c.d[0], 0xffff);
    c.d[1] = 0xffff_ffff;
    crate::tools::ops::logic(c, 0xffff_ffff, Size::Long);
}

fn clear_ring_68k(c: &mut Cpu, b: &mut World, ring: &Ring) {
    clear_ring(b, ring);
    fill_68k(c, a5(ring.cases) + 4 * 32, 0);
    rts(c, b);
}

fn clear_popups_68k(c: &mut Cpu, b: &mut World) {
    clear_popups(b);
    fill_68k(c, a5(POPUP_INPUTS) + 4 * 64, 1);
    rts(c, b);
}

/// Leaves A1 after the zone, D0 = 0, D6 = 0xFFFF.
fn clear_popup_68k(c: &mut Cpu, b: &mut World) {
    clear_current_popup(b);
    c.a[1] = a5(CURRENT_POPUP) + 16;
    c.d[0] = 0;
    c.d[6] = 0xffff;
    crate::tools::ops::logic(c, 0, Size::Long);
    rts(c, b);
}

/// Registers and flags of 0x19ae6 / 0x19b08: D6 (index), A4 (start of the cells).
fn enqueue_68k(c: &mut Cpu, b: &mut World, ring: &Ring, event: u16) {
    let (index, case) = write_cell(b, ring);
    let content = rd16(b, case);
    c.d[6] = index as u32;
    c.a[4] = a5(ring.cases);
    if enqueue(b, ring, event) {
        crate::tools::ops::add(c, 2, index as u32, Size::Byte);
        let reduced = index.wrapping_add(2) & MASK;
        set_word(&mut c.d[6], reduced as u16);
        crate::tools::ops::logic(c, reduced as u32, Size::Byte);
    } else {
        crate::tools::ops::cmp(c, EMPTY as u32, content as u32, Size::Word);
    }
}

fn enqueue_points_68k(c: &mut Cpu, b: &mut World) {
    enqueue_68k(c, b, &POINTS, c.d[0] as u16);
    rts(c, b);
}

fn enqueue_counter_68k(c: &mut Cpu, b: &mut World) {
    enqueue_68k(c, b, &COUNTER, c.d[0] as u16);
    rts(c, b);
}

/// 0x19ad6: D0 = the event, D1.w = the player (bits 8-9).
fn object_event_68k(c: &mut Cpu, b: &mut World) {
    let o = crate::world::objects::Obj(c.a[0]);
    let type_ = rd8(b, o.0 + 0x7e) as u32;
    c.d[0] = type_;
    let player = rd16(b, o.0 + 0x48) & 0x300;
    set_word(&mut c.d[1], player);
    let event = crate::tools::ops::add(c, player as u32, type_, Size::Word) as u16;
    set_word(&mut c.d[0], event);
    enqueue_68k(c, b, &POINTS, event);
    rts(c, b);
}

/// The last event that `credit` will process (index, event): the waiting cells from the read index, each emptied
/// when it is processed (a cell met again is empty).
fn last_credited(b: &mut World, k: &Credit) -> Option<(u8, u16)> {
    let mut index = rd8(b, a5(k.read));
    let mut seen = [false; 256];
    let mut last = None;
    while !std::mem::replace(&mut seen[index as usize], true) {
        let event = rd16(b, a5(k.ring.cases).wrapping_add(index as u32));
        if event == EMPTY {
            break;
        }
        last = Some((index, event));
        index = index.wrapping_add(2) & MASK;
    }
    last
}

/// Registers of 0x19b2a / 0x19b9c: leftovers of the last iteration, D0 = final read index.
fn credit_68k(c: &mut Cpu, b: &mut World, k: &Credit) {
    c.d[1] = 0;
    c.d[2] = 0;
    c.a[2] = a5(k.ring.cases);
    let last = last_credited(b, k);
    credit(b, k);
    if let Some((_, event)) = last {
        let (player, line) = credited(event);
        c.d[1] = line;
        c.a[0] = player;
        c.a[3] = k.table + line;
        c.a[4] = player + k.counter;
        c.d[6] = 0xffff;
    }
    c.d[0] = rd8(b, a5(k.read)) as u32;
    // last cmpi.w #-1: equality; X: carry of the `addq.b #2` on the read index of the last iteration
    crate::tools::ops::cmp(c, EMPTY as u32, EMPTY as u32, Size::Word);
    if let Some((index, _)) = last {
        crate::tools::ops::flags(c, SR_X, if index as u16 + 2 > 0xff { SR_X } else { 0 });
    }
    rts(c, b);
}

/// Registers and flags of 0x19dce: D6 (index), A2 (entries), D0.b (header), flags of the last test.
fn create_popup_68k(c: &mut Cpu, b: &mut World) {
    let p = Popup { header: c.d[0] as u16, max: c.d[1] as u16, value: c.d[2] as u16, raw: c.d[3] as u8 == 2 };
    let inhibited = rd8(b, a5(POPUP_INHIBITED));
    let (index, case) = popup_write_entry(b);
    let content = rd16(b, case);
    let created = create_popup(b, p);
    if inhibited != 0 {
        crate::tools::ops::logic(c, inhibited as u32, Size::Byte);
    } else if !created {
        c.d[6] = index as u32;
        c.a[2] = a5(POPUP_INPUTS);
        crate::tools::ops::cmp(c, EMPTY as u32, content as u32, Size::Word);
    } else {
        let header = rd16(b, case);
        c.a[2] = a5(POPUP_INPUTS);
        crate::tools::ops::cmp(c, 2, c.d[3] & 0xff, Size::Byte);
        if !p.raw {
            crate::tools::ops::add(c, 0x20, c.d[0] & 0xff, Size::Byte);
            set_low_byte(&mut c.d[0], header as u8);
        }
        let next = crate::tools::ops::add(c, 8, index as u32, Size::Byte) as u8;
        c.d[6] = next as u32;
        crate::tools::ops::logic(c, next as u32, Size::Byte);
    }
    rts(c, b);
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

fn set_word(r: &mut u32, v: u16) {
    *r = *r & 0xffff_0000 | v as u32;
}
