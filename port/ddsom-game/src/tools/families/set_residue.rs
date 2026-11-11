//! Tools: the natives of `crate::families::set_residue` for the original code that calls them (arguments and results in the registers
//! of the original).

use crate::world::World;
use m68k::decode::Size;
use m68k::Cpu;

use crate::families::set_residue::*;
use crate::tools::frames::*;
use crate::{regs, Native};
use crate::objects::Obj;

/// 0xd5f12: clears three columns of the tile buffer (44 rows each) for the scene exit.
fn erase_columns(c: &mut Cpu, b: &mut World) {
    let end = clear_columns(b);
    columns_regs(c, end);
    rts(c, b);
}

/// Registers left by the column clearing: A1 = end of the last one, D0 = exhausted `dbf` counter, D4 =
/// the empty tile.
fn columns_regs(c: &mut Cpu, end: u32) {
    c.a[1] = end;
    c.d[0] = 0xffff;
    c.d[4] = BLANK;
    c.flags_logic(BLANK, Size::Long);
}

/// `two_grids` (character D0) for the tools (the game calls `two_grids`).
fn load_two_grids(c: &mut Cpu, b: &mut World) {
    two_grids(b, c.d[0] as u16);
    rts(c, b);
}

const fn native(entry: u32, name: &'static str, run: fn(&mut Cpu, &mut World), outputs: u32) -> Native {
    Native { entry, name, run: crate::Body::WithCpu(run), touches_sound: false, outputs }
}

pub(crate) const NATIVES: &[Native] = &[
    native(0xd5f12, "set_clears_columns", erase_columns, regs::ALL),
    native(0xce668, "set_two_grids", load_two_grids, regs::ALL),
    native(0xd98b8, "set_wave_start", |c, b| { begin_wave(b, Obj(c.a[0])); rts(c, b) }, regs::ALL),
    native(0xd995a, "set_wave_end", |c, b| { finish_wave(b, Obj(c.a[0])); rts(c, b) }, regs::ALL),
];
