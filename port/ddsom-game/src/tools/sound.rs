//! Tools: the natives of `crate::sound` for the original code that calls them (arguments and results in the registers
//! of the original).

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::{Cpu, SR_C, SR_N, SR_V, SR_X, SR_Z};

use crate::sound::*;
use crate::tools::frames::*;
use crate::{regs, Body, Native};
use std::sync::OnceLock;
use crate::mem::{rd16, rd32, rd8, wr16, wr32, wr8};
use crate::objects::Obj;

/// Reproduces D0, A1 and the flags left by 0xe0848.
fn finish_enqueue(c: &mut Cpu, e: &Queueing) {
    c.d[0] = c.d[0] & 0xffff_0000 | e.d0 as u32;
    match e.written {
        Some(end) => {
            c.a[1] = end;
            c.flags_logic(e.d0 as u32, Size::Word);
        }
        None => c.sr = c.sr & 0xff00 | SR_C, // move #1,CCR: only the carry is raised
    }
}

fn register_command(c: &Cpu) -> Command {
    Command { d1: c.d[1], d2: c.d[2], d3: c.d[3] }
}

/// 0xe0848: enqueues D1, D2, D3 (C raised if the queue is full).
fn enqueue_direct(c: &mut Cpu, b: &mut Cps2Bus) {
    let e = enqueue(b, register_command(c));
    finish_enqueue(c, &e);
    rts(c, b);
}

/// 0xe0836: enqueues D1, D2, D3 if sound is allowed.
fn enqueue_if_sound(c: &mut Cpu, b: &mut Cps2Bus) {
    if sound_allowed(b) {
        enqueue_direct(c, b);
    } else {
        c.sr = c.sr & 0xff00 | SR_C;
        rts(c, b);
    }
}

pub const ENQUEUE_IF_SOUND: Native = Native {
    entry: 0xe0836,
    name: "sound_enqueue",
    run: crate::Body::WithCpu(enqueue_if_sound),
    touches_sound: false,
    outputs: regs::d(0) | regs::a(1) | regs::CCR,
};

pub const ENQUEUE_DIRECT: Native = Native {
    entry: 0xe0848,
    name: "sound_enqueue_direct",
    run: crate::Body::WithCpu(enqueue_direct),
    touches_sound: false,
    outputs: regs::d(0) | regs::a(1) | regs::CCR,
};

/// Native of a wrapper or a dispatcher (calls by address): the core, with what it reads from the registers;
/// all registers are returned intact, the flags are those of the last `move.l` of the private stack pointer.
fn native_sound_in(c: &mut Cpu, b: &mut Cps2Bus, sound: u32) {
    play_with(b, sound, Source { object: Some(Obj(c.a[0])), a1: Some(c.a[1]), d4: Some(c.d[4]) });
    let top = rd32(b, a5(ram::PRIVATE_STACK));
    c.flags_logic(top, Size::Long);
    rts(c, b);
}

/// The native routines of each described wrapper and of the dispatchers (built once, then kept).
pub fn native_wrappers() -> &'static [Native] {
    static LIST: OnceLock<Vec<Native>> = OnceLock::new();
    LIST.get_or_init(|| {
        let wrappers = table::WRAPPERS.iter().map(|w| (w.entry, "sound_wrapper"));
        let dispatches = DISPATCHES.iter().map(|&(entry, _)| (entry, "sound_dispatch"));
        let versions = BY_VERSION.iter().map(|&(entry, ..)| (entry, "sound_by_version"));
        wrappers
            .chain(dispatches)
            .chain(versions)
            .map(|(entry, name)| Native {
                entry,
                name,
                run: crate::Body::CpuIn(native_sound_in),
                touches_sound: false,
                outputs: 0, // all registers are returned intact: nothing to compare
            })
            .collect()
    })
}

/// Native of a service command: all registers are returned (movem on the stack), only the flags of
/// 0xe0848 remain.
fn service_native_run(c: &mut Cpu, b: &mut Cps2Bus, s: Service) {
    let e = service_enqueue(b, s);
    let (d0, a1) = (c.d[0], c.a[1]);
    finish_enqueue(c, &e);
    c.d[0] = d0;
    c.a[1] = a1;
    rts(c, b);
}

/// Service command of entry `entry`, from D0 and D1 (for the natives).
fn service_in(entry: u32, c: &Cpu) -> Service {
    let (d0, d1) = (c.d[0] as u16, c.d[1] as u8);
    match entry {
        0xe0882 => Service::GeneralStop,
        0xe0a10 => Service::Ff05,
        0xe0a2e => Service::Ff06 { lock: d1, line: d0 },
        0xe0a40 => Service::Ff06 { lock: 0xff, line: d0 },
        0xe0a70 => Service::Ff07 { lock: d1, line: d0 },
        0xe0a82 => Service::Ff07 { lock: 0, line: d0 },
        0xe0ad8 => Service::Ff08(d0),
        0xe0af8 => Service::Ff09(d0),
        _ => unreachable!("{entry:x} is not a service command"),
    }
}

const fn service_native(entry: u32, name: &'static str, touches_sound: bool) -> Native {
    Native { entry, name, run: crate::Body::CpuIn(|c, b, entry| service_native_run(c, b, service_in(entry, c))), touches_sound, outputs: regs::CCR }
}

/// `clear_private_stack`; A0 = the pointer, flags of its value.
fn init_private_stack(c: &mut Cpu, b: &mut Cps2Bus) {
    clear_private_stack(b);
    let p = a5(ram::PRIVATE_STACK);
    c.a[0] = p;
    c.flags_logic(p, Size::Long);
    rts(c, b);
}

pub const INIT_PRIVATE_STACK: Native = Native {
    entry: 0xee044,
    name: "sound_init_stack",
    run: crate::Body::WithCpu(init_private_stack),
    touches_sound: false,
    outputs: regs::a(0) | regs::CCR,
};

/// 0xec238: empty sound (a plain `rts`), target of the dispatcher 0xec200 for objects without sound.
pub const NONE: Native = Native {
    entry: NO_SOUND,
    name: "sound_none",
    run: crate::Body::WithCpu(rts),
    touches_sound: false,
    outputs: regs::ALL,
};

pub const SERVICE_FF00: Native = service_native(0xe0882, "sound_service_ff00", false);

pub const SERVICE_FF05: Native = service_native(0xe0a10, "sound_service_ff05", false);

pub const SERVICE_FF06_D1: Native = service_native(0xe0a2e, "sound_service_ff06", true);

pub const SERVICE_FF06: Native = service_native(0xe0a40, "sound_service_ff06", true);

pub const SERVICE_FF07_D1: Native = service_native(0xe0a70, "sound_service_ff07", true);

pub const SERVICE_FF07: Native = service_native(0xe0a82, "sound_service_ff07", true);

pub const SERVICE_FF08: Native = service_native(0xe0ad8, "sound_service_ff08", false);

pub const SERVICE_FF09: Native = service_native(0xe0af8, "sound_service_ff09", false);
