//! Machine startup (K1 `reset` 0x350 and `boot_init` 0x492, K2 RAM test 0x182a and 0x1912).
//!
//! At power-on, the 68000 (interrupts masked, supervisor stack at FF0E66):
//!
//! 1. `reset`: I/O and CPS registers, Z80 held in reset, work RAM cleared, layers and objects
//!    cleared;
//! 2. RAM test: starting palette (8 longs at 0x91C400), GFX RAM 0x910000-0x913FFF filled with cells
//!    (0x20, 0), then *non-destructive* tests (each word receives 0x8888, 0x4444, 0x2222, 0x1111 and
//!    gets its value back) of the GFX RAM 0x910000-0x91BFFF and of the work RAM;
//! 3. releases the Z80, waits for its handshake (0x77 at CFFF), tests the bytes of the shared RAM
//!    (0, then 0x55, then 0xFF except the last three, which keep 0x55);
//! 4. `boot_init`: Z80 mailbox (CFFD ← 0x88, CFFE and CFFF ← 0xFF), task stack heap,
//!    settings (0x2d30a), interrupts enabled (the pending VBLANK is taken immediately), startup
//!    task (0x11b16), scheduler.
//!
//! On the emulated machine all the RAM is good: the error branch of the test ("RAM ERROR !!" displayed
//! at 0x19ae, then infinite loop) is never taken and is not rewritten. The effects remain.
//!
//! **Timing.** At ×16, the test lasts five frames: the Z80 is released at frame 5, line 108, and
//! `boot_init` starts at line 173. These two instants matter (the Z80 starts in parallel, it waits for
//! 0xFF at CFFF to enter its main loop): the native version produces all the effects at
//! reset, then waits for the same beam position as the overclocked reference (`Z80_RELEASE`,
//! `BOOT_INIT`), whatever the 68000 frequency. Startup does not depend on the inputs (no
//! scenario gives any before frame 600): this timing is fixed (`examples/startup`). A software
//! reset (0x28cd0, after a processor exception, K4 out of scope) would not find
//! the duration of the test again. `validate` tolerates the word under test in frames 1 to 5 (`docs/noyau.md`).

use cps2::{Cps2Bus, LINES, VBLANK_LINE};
use m68k::Cpu;

use crate::mem::{rd16, rd32, rd8, set_low_word, wr16, wr32, wr8, A5};
use crate::call;
use crate::kernel::Point;
use crate::tasks::table::{table, Table};

/// Beam position: lines elapsed since power-on (a frame starts at vblank).
const fn beam(image: u64, line: u32) -> u64 {
    image * LINES as u64 + ((line + LINES - VBLANK_LINE) % LINES) as u64
}

fn position(b: &Cps2Bus) -> u64 {
    beam(b.frame, b.scanline)
}

/// The original code at ×16 releases the Z80 here (0x18cc)...
const Z80_RELEASE: u64 = beam(5, 108);
/// ... and enters `boot_init` (0x492) here.
const BOOT_INIT: u64 = beam(5, 173);

/// Executes startup point `point`; returns the next one, or `None` when it waits for its time (beam position,
/// Z80 handshake): it will retry at the next line, having changed nothing. After `boot_init`
/// (`move #0x2000,SR`), the pending interrupt is taken before the launch.
pub fn execute(c: &mut Cpu, b: &mut Cps2Bus, point: Point) -> Option<Point> {
    match point {
        Point::Reset => Some(reset(c, b)),
        Point::ReleaseZ80 => release_z80(b),
        Point::Handshake => handshake(c, b),
        Point::BootInit => boot_init(c, b),
        Point::Launch => Some(launch(c, b)),
        _ => unreachable!("point {point:?}: not a startup point"),
    }
}

/// Original subroutines and return addresses of their `jsr`.
mod ext {
    pub const CLEAR_OBJECTS: (u32, u32) = (0xc58, 0x436);
    pub const CLEAR_PLANE_1: (u32, u32) = (0xca8, 0x43c);
    pub const CLEAR_PLANE_2: (u32, u32) = (0xcc8, 0x442);
    pub const CLEAR_PLANE_3: (u32, u32) = (0xce8, 0x448);
    pub const CONFIG: (u32, u32) = (0x2dc9a, 0x468);
    pub const SETTINGS: (u32, u32) = (0x2d30a, 0x500);
    pub const CREATE_TASK: (u32, u32) = (0x9b6, 0x530);
    pub const STARTUP_TASK: u32 = 0x11b16;
}

/// I/O registers.
mod io {
    /// Outputs 0x400000-0x40000A (priorities, control): reset values.
    pub const EXITS: [(u32, u16); 6] =
        [(0x400000, 0x7080), (0x400002, 0x807d), (0x400004, 0x2355), (0x400006, 0), (0x400008, 0x40), (0x40000a, 0x10)];
    /// EEPROM lines and Z80 reset (bit 3: Z80 running).
    pub const COMMANDS: u32 = 0x804040;
    pub const Z80_IN_RESET: u16 = 0x0f00;
    pub const Z80_RUNNING: u16 = 0x0f08;
    pub const CPS_A: u32 = 0x804100;
    pub const CPS_B_OBJECTS: u32 = 0x804170;
    pub const VIDEO: u32 = 0x804122;
    pub const VIDEO_B: u32 = 0x804166;
    /// Port read at reset (0xe021 on this board): its bit 14 being zero (never seen) makes it read the configuration
    /// (0x2dc9a) at reset, and possibly reinitialise the extra RAM (0x2e8e4, `eeprom::reset_card`).
    pub const CARD: u32 = 0x804030;
    pub const TEST: u32 = 0x804020;
    pub const BUTTONS: u32 = 0x804000;
    pub const UNKNOWN: u32 = 0x8040a0;
}

/// RAM shared with the Z80, 68000 side (odd bytes).
mod shared {
    pub const START: u32 = 0x618001;
    pub const END: u32 = 0x61a000;
    /// CFFD, CFFE, CFFF: mailbox flag, stereo, handshake.
    pub const HITBOX: u32 = 0x619ffb;
    pub const STEREO: u32 = 0x619ffd;
    pub const HANDSHAKE: u32 = 0x619fff;
    /// Index of CFFF in `Cps2Bus::qram`.
    pub const Z80_HANDSHAKE: usize = 0xfff;
    pub const Z80_READY: u8 = 0x77;
}

/// Program constants read at startup.
mod rom {
    /// Region word (copied to `A5+0x70`) and "development card" byte.
    pub const REGION: u32 = 0xc2;
    pub const DEVELOPMENT: u32 = 0xc4;
    /// Starting palette (8 longs, copied to 0x91C400).
    pub const PALETTE: u32 = 0x19f0;
    /// Stacks of the 16 tasks (16 longs, pushed below FF01A6: the stack heap).
    pub const STACKS: u32 = 0x534;
    /// "RAM ERROR !!" text of the last block tested (left in A2).
    pub const TEXT_ERROR: u32 = 0x19e0;
}

const GFX_TEST: u32 = 0x910000;
const GFX_FILLED: u32 = 0x914000;
const START_PALETTE: u32 = 0x91c400;
const RAM_START: u32 = 0xff0000;
/// Bottom of the original's task stack heap (pointer at FF005A, stacks pushed below FF01A6).
const STACKS_END: u32 = A5 - 0x7e5a;
/// Flag set when button 1 of player 1 is pressed at startup (development cards).
const BUTTON_AT_STARTUP: u32 = A5 - 0x55e2;

/// Work RAM variables (offsets from A5).
mod var {
    pub const VIDEO: u32 = 0x12;
    pub const VIDEO_B: u32 = 0x14;
    /// Image of port 0x804040 that the writes of the EEPROM and the counters recompose: high byte, coin counter
    /// bits, forced bits (including bit 3, Z80 running; `docs/feuilles.md`, `docs/video.md`).
    pub const PORT_COMMANDS: [(u32, u8); 3] = [(0x16, 0xf), (0x17, 8), (0x18, 8)];
    pub const REGION: u32 = 0x70;
    /// Bit 14 of `io::CARD` zero (meaning unknown; also tested by the game task, `docs/looped.md` §1).
    pub const SPECIAL_CARD: u32 = 0x82;
    pub const EXIT: u32 = 0x5bc;
    pub const DSW: u32 = 0x7fd0;
}

fn a5(off: u32) -> u32 {
    A5.wrapping_add(off)
}

/// `jsr` of a startup routine (the original return address is no longer pushed: the startup stack is reread by
/// nobody).
fn jsr(c: &mut Cpu, b: &mut Cps2Bus, (routine, _ret): (u32, u32)) {
    call(c, b, routine);
}

/// 0x350: reset, then the effects of the RAM test (0x182a) up to the release of the Z80.
fn reset(c: &mut Cpu, b: &mut Cps2Bus) -> Point {
    wr16(b, io::UNKNOWN, 0);
    for (port, v) in io::EXITS[..2].iter().copied() {
        wr16(b, port, v);
    }
    wr16(b, io::CPS_A, 0x9100);
    for (port, v) in io::EXITS[2..].iter().copied() {
        wr16(b, port, v);
    }
    wr16(b, io::COMMANDS, io::Z80_IN_RESET);
    c.a[5] = A5;
    b.ram.fill(0);
    *table(b) = Table::default();
    let region = rd16(b, rom::REGION);
    wr16(b, a5(var::REGION), region);
    wr16(b, io::VIDEO, 0xe);
    wr16(b, a5(var::VIDEO), 0xe);
    wr16(b, io::VIDEO_B, 0x12ce);
    wr16(b, a5(var::VIDEO_B), 0x12ce);
    wr16(b, io::CPS_B_OBJECTS, 0xe);
    wr8(b, a5(var::EXIT), 1);
    // bases of the objects and of planes 1 and 2, scroll of the three planes to zero
    for (k, base) in [0x9100u16, 0x9140, 0x9180].into_iter().enumerate() {
        wr16(b, io::CPS_A + 2 + 2 * k as u32, base);
    }
    for reg in [0x0c, 0x10, 0x14] {
        wr32(b, io::CPS_A + reg, 0);
    }
    for routine in [ext::CLEAR_OBJECTS, ext::CLEAR_PLANE_1, ext::CLEAR_PLANE_2, ext::CLEAR_PLANE_3] {
        jsr(c, b, routine);
    }
    wr8(b, a5(var::SPECIAL_CARD), 0);
    if rd16(b, io::CARD) & 0x4000 == 0 {
        wr8(b, a5(var::SPECIAL_CARD), 1);
        jsr(c, b, ext::CONFIG);
        // button combination 0x3304 (active low) of IN2 pressed at startup
        if !rd16(b, io::TEST) & 0x3304 == 0x3304 {
            crate::eeprom::reset_card(c, b);
        }
    }
    // 0x182a: starting palette, then empty cells in the first quarter of the tested GFX RAM
    for i in 0..8 {
        let v = rd32(b, rom::PALETTE + 4 * i);
        wr32(b, START_PALETTE + 4 * i, v);
    }
    wr16(b, io::CPS_B_OBJECTS, 0xf);
    wr16(b, io::CPS_A + 0x0a, 0x91c0);
    for a in (GFX_TEST..GFX_FILLED).step_by(4) {
        wr16(b, a, 0x20);
        wr16(b, a + 2, 0);
    }
    Point::ReleaseZ80
}

/// 0x18cc: at the original's time, releases the Z80 and clears its mailbox flag.
fn release_z80(b: &mut Cps2Bus) -> Option<Point> {
    if position(b) < Z80_RELEASE {
        return None;
    }
    wr16(b, io::COMMANDS, io::Z80_RUNNING);
    wr8(b, a5(var::PORT_COMMANDS[2].0), var::PORT_COMMANDS[2].1);
    wr8(b, shared::HITBOX, 0);
    Some(Point::Handshake)
}

/// 0x18e2: waits for the Z80 handshake, then leaves the shared RAM as its test does.
fn handshake(c: &mut Cpu, b: &mut Cps2Bus) -> Option<Point> {
    if b.qram[shared::Z80_HANDSHAKE] != shared::Z80_READY {
        return None;
    }
    for a in (shared::START..shared::END).step_by(2) {
        wr8(b, a, if a < shared::HITBOX { 0xff } else { 0x55 });
    }
    test_regs(c, b);
    Some(Point::BootInit)
}

/// Registers that the RAM test leaves to `boot_init` (and, through it, to the settings 0x2d30a): the longs of the
/// palette loaded by `movem` at 0x1834, then what the test loops put there.
fn test_regs(c: &mut Cpu, b: &mut Cps2Bus) {
    let palette: [u32; 8] = std::array::from_fn(|i| rd32(b, rom::PALETTE + 4 * i as u32));
    c.d[..7].copy_from_slice(&palette[..7]);
    set_low_word(&mut c.d[0], 0x5555); // last pattern of the shared RAM test
    c.d[1] = 0;
    let last_word = rd16(b, RAM_START + 0xfffc); // value of the last RAM word tested
    set_low_word(&mut c.d[2], last_word);
    c.d[3] = 0xff;
    c.d[7] = 0xffff;
    c.a[0] = shared::END - 6;
    c.a[1] = shared::END - 6;
    c.a[2] = rom::TEXT_ERROR;
    c.a[3] = shared::START - 1;
    c.a[4] = 0;
    c.a[6] = 0x492; // `boot_init`, return address of the test (`lea`)
}

/// 0x492: at the original's time, Z80 mailbox, stack heap, settings, then interrupts.
fn boot_init(c: &mut Cpu, b: &mut Cps2Bus) -> Option<Point> {
    if position(b) < BOOT_INIT {
        return None;
    }
    wr8(b, shared::HITBOX, 0x88);
    wr8(b, shared::STEREO, 0xff);
    wr8(b, shared::HANDSHAKE, 0xff);
    c.a[5] = A5;
    wr16(b, io::COMMANDS, io::Z80_RUNNING);
    for (off, v) in var::PORT_COMMANDS {
        wr8(b, a5(off), v);
    }
    wr16(b, a5(var::DSW), 0xffff);
    // 16 stacks, pushed (`move.l (A0)+,-(A1)` below FF01A6): the last of the table at the top. The heap is that of
    // the Rust task table; A1 keeps the top of the original's.
    let stacks = (0..16).map(|i| rd32(b, rom::STACKS + 4 * i)).collect();
    table(b).stacks = stacks;
    set_low_word(&mut c.d[0], 0xffff);
    c.a[0] = rom::STACKS + 0x40;
    c.a[1] = STACKS_END - 0x40;
    jsr(c, b, ext::SETTINGS);
    c.set_sr(0x2000);
    Some(Point::Launch)
}

/// 0x50a: creates the startup task (slot 0) and hands control to the scheduler.
fn launch(c: &mut Cpu, b: &mut Cps2Bus) -> Point {
    if rd8(b, rom::DEVELOPMENT) != 0 {
        let pressed = rd8(b, io::BUTTONS) & 0x10 == 0;
        wr8(b, BUTTON_AT_STARTUP, if pressed { 0xff } else { 0 });
    }
    c.a[0] = ext::STARTUP_TASK;
    set_low_word(&mut c.d[0], 0);
    jsr(c, b, ext::CREATE_TASK);
    Point::Start
}
