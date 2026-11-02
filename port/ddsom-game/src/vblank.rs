//! VBLANK interrupt (K1, 0x574): once per frame, before everything else.
//!
//! In order: CPS-A display registers (scroll of the three planes, 0x722), input reading
//! (0x80c), video control register (0x902), sending of commands to the Z80 (0x68c), reading of
//! the coin slots (0x143da), bookkeeping (0x2dcf6), then waking of the waiting tasks (task
//! table, `tasks/table.rs`). All registers are saved and restored.
//!
//! The original code tests three ROM bytes (0xc4, 0xc5, 0xc7), debugging flags: all
//! zero in this game (no wait for the test input, no dump combination, DSW settings
//! forced to 0xffff). They therefore do not appear here.

use cps2::Cps2Bus;

use crate::mem::{rd16, rd32, rd8, wr16, wr32, wr8, A5};

/// CPS-A registers (0x804100 + offset).
mod cps_a {
    pub const BASE: u32 = 0x804100;
    pub const PALETTE: u32 = BASE + 0x0a;
    /// X scroll of planes 1 to 3 (Y follows at +2).
    pub const PLAN1_X: u32 = BASE + 0x0c;
    pub const PLAN1_Y: u32 = BASE + 0x0e;
    pub const PLAN2_X: u32 = BASE + 0x10;
    pub const PLAN2_Y: u32 = BASE + 0x12;
    pub const PLAN3_X: u32 = BASE + 0x14;
    pub const PLAN3_Y: u32 = BASE + 0x16;
    /// Per-line scroll of plane 2 (offset).
    pub const LINES: u32 = BASE + 0x20;
    pub const CONTROL: u32 = BASE + 0x22;
    /// Page register of plane 1 (bit 3 toggled every frame in mode 1).
    pub const PAGE_PLAN_1: u32 = BASE + 0x08;
}

/// I/O ports written by VBLANK.
mod port {
    /// Output buffer (0x8040e1): copy of `A5+0x5bc`.
    pub const EXIT: u32 = 0x8040e1;
    /// Sprite table latch register (0x804170) and value written.
    pub const CPS_B_REG: u32 = 0x804170;
    /// Service and display inputs (0x804166: game bit, 0x804040: see `inputs.rs`).
    pub const OUTPUT: u32 = 0x400002;
}

/// Work RAM variables (offsets from A5).
mod var {
    /// VBLANK frame counter.
    pub const IMAGES: u32 = 0x6c;
    /// Value of the 0x8040e1 output.
    pub const EXIT: u32 = 0x5bc;
    /// Word reset to zero every frame.
    pub const RAZ: u32 = 0x7fce;
    /// Settings word (DSW).
    pub const DSW: u32 = 0x7fd0;
    /// Task slowdown count (byte): one frame in four for the first one examined.
    pub const SLOWDOWN: u32 = 0x4f;
    /// Video control and shadows.
    pub const CONTROL: u32 = 0x12;
    pub const SHADOWS: u32 = 0x26;
        /// Scroll mode: 1 and 2 alternate the pages / the lines of a plane from one to the other.
    pub const SCROLL_MODE: u32 = 0x53c;
    /// Page of plane 1 (bit 3 toggled every frame in mode 1).
    pub const PAGE: u32 = 0x534;
    /// Shake: number of frames remaining (word; bit 0 of the low byte gives the phase).
    pub const QUAKE: u32 = 0x4be;
    /// Alternation of mode 2.
    pub const ALTERNATION: u32 = 0x54c;
    /// Per-line scroll displayed (word).
    pub const LINES: u32 = 0x7fb2;
}

/// Scroll of a plane: the long (X, Y) wanted by the game, its copy from one frame to the next (the value
/// written to the register is that of the previous frame) and the register.
struct Plan {
    wanted: u32,
    copy: u32,
    register: u32,
}

const PLAN_1: Plan = Plan { wanted: 0x4b6, copy: 0x59e, register: cps_a::PLAN1_X };
const PLAN_2: Plan = Plan { wanted: 0x506, copy: 0x5a6, register: cps_a::PLAN2_X };
const PLAN_3: Plan = Plan { wanted: 0x556, copy: 0x5ae, register: cps_a::PLAN3_X };
/// The wanted longs at +4 of each plane carry the vertical scroll displayed in the Y registers.
const Y_1: Plan = Plan { wanted: 0x4ba, copy: 0x5a2, register: cps_a::PLAN1_Y };
const Y_2: Plan = Plan { wanted: 0x50a, copy: 0x5aa, register: cps_a::PLAN2_Y };
const Y_3: Plan = Plan { wanted: 0x55a, copy: 0x5b2, register: cps_a::PLAN3_Y };

const NEW_FRAME: u32 = 0xff0164;
/// Byte reset to zero by VBLANK, reread by the game loop (fast forward, `docs/boucle.md` §1).
const FAST_FORWARD: u32 = A5 - 0x660e;

fn a5(off: u32) -> u32 {
    A5 + off
}

/// Writes to the plane's X register the previous copy minus the 0x40-pixel margin, then copies
/// the wanted value.
fn advance_x(b: &mut Cps2Bus, p: &Plan) {
    let previous = rd16(b, a5(p.copy));
    wr16(b, p.register, previous.wrapping_sub(0x40));
    let wanted = rd32(b, a5(p.wanted));
    wr32(b, a5(p.copy), wanted);
}

/// Same for the vertical scroll: the register receives `original` minus the previous copy.
fn advance_y(b: &mut Cps2Bus, p: &Plan, original: u16) {
    let previous = rd16(b, a5(p.copy));
    wr16(b, p.register, original.wrapping_sub(previous));
    let wanted = rd32(b, a5(p.wanted));
    wr32(b, a5(p.copy), wanted);
}

/// 0x722: CPS-A scroll registers, one frame late, screen shake and alternation
/// modes (`SCROLL_MODE` = 1: pages; 2: lines).
fn display_regs(b: &mut Cps2Bus) {
    let remainder = rd16(b, a5(var::QUAKE));
    if remainder != 0 {
        wr16(b, a5(Y_1.wanted), 0x100);
        wr16(b, a5(var::QUAKE), remainder - 1);
        if rd8(b, a5(var::QUAKE + 1)) & 1 == 0 {
            wr16(b, a5(Y_1.wanted), 0);
        }
    }
    advance_x(b, &PLAN_1);
    let mode = rd8(b, a5(var::SCROLL_MODE));
    if mode == 1 {
        let x = rd16(b, a5(PLAN_2.copy)).wrapping_sub(0x90);
        wr16(b, a5(PLAN_2.copy), x);
        let page = rd16(b, a5(var::PAGE)) ^ 8;
        wr16(b, a5(var::PAGE), page);
        wr16(b, cps_a::PAGE_PLAN_1, page);
    }
    advance_x(b, &PLAN_2);
    advance_x(b, &PLAN_3);
    advance_y(b, &Y_1, 0x100);
    advance_y(b, &Y_2, 0x300);
    advance_y(b, &Y_3, 0x700);
    let lines = rd16(b, a5(var::LINES));
    wr16(b, cps_a::LINES, lines);
    if mode == 2 {
        wr16(b, a5(var::LINES), 0);
        wr16(b, a5(Y_2.wanted), 0x28);
        let old = rd8(b, a5(var::ALTERNATION));
        wr8(b, a5(var::ALTERNATION), old ^ 1);
        if old & 1 == 0 {
            wr16(b, a5(Y_2.wanted), 0x128);
            wr16(b, a5(var::LINES), 0x100);
        }
    }
}

/// 0x902: video control register (bit 15: shadows) and service output.
fn video_control(b: &mut Cps2Bus) {
    let mut control = rd16(b, a5(var::CONTROL));
    let mut exit = 0x807d;
    if rd8(b, a5(var::SHADOWS)) != 0 {
        control |= 0x8000;
        exit = 0x808e;
    }
    wr16(b, cps_a::CONTROL, control);
    wr16(b, port::OUTPUT, exit);
}

/// Wakes the tasks whose wait count falls to zero (Rust table, `tasks::table`).
fn wake_tasks(b: &mut Cps2Bus) {
    let mut slowdown = rd8(b, a5(var::SLOWDOWN));
    crate::tasks::table::table(b).count_down_frame(&mut slowdown);
    wr8(b, a5(var::SLOWDOWN), slowdown);
}

/// The VBLANK interrupt (level `level`, autovector `0x574`), taken by the kernel between two points
/// (`kernel::interrupt`): acknowledgement, then the processing. No register or stack: the original switches to
/// supervisor, pushes PC and SR and raises its mask, but its processing saves and restores all registers and
/// cannot be interrupted; the `rte` restores the SR. The point to return to is kept by the kernel
/// (`Table::interrupted`).
///
/// Accepted difference: PC and SR are no longer written to the current stack (stack of the interrupted task,
/// FF01A6-FF0DA5, or of the supervisor, below FF0E66). `validate` ignores these words
/// (`tasks::table::kernel_ram`), and the game does not reread them
/// (proof: docs/refactorisation.md, batch "CPU removal", part 4).
pub fn interruption(b: &mut Cps2Bus, level: u8) {
    m68k::Bus::irq_ack(b, level);
    vblank(b);
}

/// 0x574: the interrupt processing. The original saves and restores all registers: what its
/// services leave in them does not come out.
fn vblank(b: &mut Cps2Bus) {
    let exit = rd8(b, a5(var::EXIT));
    wr8(b, port::EXIT, exit);
    wr16(b, a5(var::RAZ), 0);
    wr16(b, port::CPS_B_REG, 0x3f);
    wr16(b, cps_a::PALETTE, 0x91c0);
    wr16(b, a5(var::DSW), 0xffff);
    let images = rd16(b, a5(var::IMAGES)).wrapping_add(1);
    wr16(b, a5(var::IMAGES), images);
    wr8(b, NEW_FRAME, 0xff);
    wr8(b, FAST_FORWARD, 0);
    display_regs(b);
    // the services return values that the original leaves in its registers; VBLANK restores them all
    crate::inputs::read_panel(b); // 0x80c
    video_control(b);
    crate::sound::send(b); // 0x68c: Z80 commands
    crate::credits::read_coin_slots(b); // 0x143da
    crate::eeprom::vblank(b); // 0x2dcf6: bookkeeping
    wake_tasks(b);
    // the original code ends with the charge gauge (0xb3e), inactive as long as DSW has its bit 0x400
}
