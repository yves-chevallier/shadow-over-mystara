//! Tools: the natives of `crate::events` for the original code that calls them (arguments and results in the registers
//! of the original).

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::{Cpu, SR_X};

use crate::events::*;
use crate::tools::frames::*;
use crate::{regs, Native};
use crate::mem::{rd16, rd8, set_low_byte};

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
    crate::tools::ops::logic(c, 0, Size::Long);
    rts(c, b);
}

/// Registers and flags of 0x19ae6 / 0x19b08: D6 (index), A4 (start of the cells).
fn enqueue_68k(c: &mut Cpu, b: &mut Cps2Bus, ring: &Ring, event: u16) {
    let e = enqueue(b, ring, event);
    c.d[6] = e.index as u32;
    c.a[4] = a5(ring.cases);
    match e.busy {
        Some(content) => crate::tools::ops::cmp(c, EMPTY as u32, content as u32, Size::Word),
        None => {
            crate::tools::ops::add(c, 2, e.index as u32, Size::Byte);
            let reduced = e.index.wrapping_add(2) & MASK;
            set_word(&mut c.d[6], reduced as u16);
            crate::tools::ops::logic(c, reduced as u32, Size::Byte);
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
    let event = crate::tools::ops::add(c, player as u32, type_, Size::Word) as u16;
    set_word(&mut c.d[0], event);
    enqueue_68k(c, b, &POINTS, event);
    rts(c, b);
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
    crate::tools::ops::cmp(c, EMPTY as u32, EMPTY as u32, Size::Word);
    if let Some(d) = last {
        crate::tools::ops::flags(c, SR_X, if d.index as u16 + 2 > 0xff { SR_X } else { 0 });
    }
    rts(c, b);
}

/// Registers and flags of 0x19dce: D6 (index), A2 (entries), D0.b (header), flags of the last test.
fn create_popup_68k(c: &mut Cpu, b: &mut Cps2Bus) {
    let p = Popup { header: c.d[0] as u16, max: c.d[1] as u16, value: c.d[2] as u16, raw: c.d[3] as u8 == 2 };
    match create(b, p) {
        Creation::Inhibited(inhibited) => crate::tools::ops::logic(c, inhibited as u32, Size::Byte),
        Creation::Busy { index, content } => {
            c.d[6] = index as u32;
            c.a[2] = a5(POPUP_INPUTS);
            crate::tools::ops::cmp(c, EMPTY as u32, content as u32, Size::Word);
        }
        Creation::Done { index, header } => {
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
