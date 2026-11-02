//! Tools: the natives of `crate::text` for the original code that calls them (arguments and results in the registers
//! of the original).

use cps2::Cps2Bus;
use m68k::{Cpu, SR_C, SR_N, SR_V, SR_X, SR_Z};

use crate::text::*;
use crate::tools::frames::*;
use crate::{regs, Native};

/// End of routine: A4 advanced, flags of the final comparison (equality) and X left by the
/// last operation that modifies it.
fn finish(c: &mut Cpu, b: &mut Cps2Bus, a4: u32, x: bool) {
    c.a[4] = a4;
    c.sr = (c.sr & !(SR_X | SR_N | SR_V | SR_C)) | SR_Z | if x { SR_X } else { 0 };
    rts(c, b);
}

fn native_text(c: &mut Cpu, b: &mut Cps2Bus, kind: Kind) {
    let (end, x) = text(b, c.a[4], kind);
    finish(c, b, end, x);
}

fn print_tall_native(c: &mut Cpu, b: &mut Cps2Bus) {
    let end = print_tall(b, c.a[4]);
    finish(c, b, end, false);
}

fn print_tall_at_native(c: &mut Cpu, b: &mut Cps2Bus) {
    let end = print_tall_at(b, c.a[4], c.d[5] as u8, c.d[6] as u8);
    finish(c, b, end, false);
}

fn blocks_native(c: &mut Cpu, b: &mut Cps2Bus, drawn: bool) {
    let (end, x) = blocks(b, c.a[4], c.a[0], drawn);
    finish(c, b, end, x);
}

fn clear_box_native(c: &mut Cpu, b: &mut Cps2Bus) {
    let (end, height) = clear_box_height(b, c.a[4]);
    c.a[4] = end;
    // flags: last move.w #0 (Z); X = carry of "height - 1"
    let x = if height == 0 { SR_X } else { 0 };
    c.sr = (c.sr & !(SR_X | SR_N | SR_V | SR_C)) | SR_Z | x;
    rts(c, b);
}

const fn native(entry: u32, name: &'static str, run: fn(&mut Cpu, &mut Cps2Bus)) -> Native {
    Native { entry, name, run: crate::Body::WithCpu(run), touches_sound: false, outputs: regs::a(4) | regs::CCR }
}

pub(crate) const PRINT: Native = native(0x0d5e, "text_print", |c, b| native_text(c, b, Kind::Text));

pub(crate) const ERASE: Native = native(0x0d32, "text_erase", |c, b| native_text(c, b, Kind::Clear));

pub(crate) const PRINT_OVER: Native = native(0x0dca, "text_print_over", |c, b| native_text(c, b, Kind::Overlay));

pub(crate) const CLEAR_BLOCKS: Native = native(0x0f4a, "text_clear_blocks", |c, b| blocks_native(c, b, false));

pub(crate) const DRAW_BLOCKS: Native = native(0x0f38, "text_draw_blocks", |c, b| blocks_native(c, b, true));

pub(crate) const PRINT_TALL: Native = native(0x1136, "text_print_tall", print_tall_native);

pub(crate) const PRINT_TALL_AT: Native = native(0x1234, "text_print_tall_at", print_tall_at_native);

pub(crate) const CLEAR_BOX: Native = native(0x1324, "text_clear_box", clear_box_native);
