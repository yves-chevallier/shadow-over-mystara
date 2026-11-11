use super::*;
use crate::hw::mem::{rd32, wr32};
use crate::actors::families::set::tables as set_tables;
use crate::world::objects::Obj;

/// Script table (data space): script then source palettes, 8 bytes per variant.
const TABLE: u32 = 0xba57a;
/// During a scene transition: action per variant (byte, program space).
const TRANSITION_ACTION: u32 = 0xba44c;
/// End of script: continuation per variant (byte, program space).
const END_ACTION: u32 = 0xba4ca;
const PALETTE_PAGES: [u32; 4] = [0x91c800, 0x91cc00, 0x91c000, 0x91c400];
/// Fields: script, source palettes, destination, countdown.
const SCRIPT: u32 = 0x30;
const SOURCE: u32 = 0x34;
const DEST: u32 = 0x38;
const TIMER: u32 = 0x3c;
const PAGE: u32 = 0x2f;
const KEEP_BRIGHTNESS: u32 = 0xf000f000;
const LINE_BYTES: u32 = 32;
const STATE_FINISH: u16 = 0x400;
const GAME_PAUSED: u32 = 0x3d;
const TRANSITION: u32 = 0x42;
/// Variant that ignores pause and suspension.
const ALWAYS_RUNS: u8 = 0x4e;
/// Variantes de remplacement en fin de script.
const NEXT_VARIANTS: [(u8, u8); 3] = [(0x48, 0), (0x4c, 0), (0x4d, 1)];

pub fn run(x: &mut Gx) {
    let variant = x.u8(Obj::VARIANT);
    if variant != ALWAYS_RUNS && (x.a5_u8(GAME_PAUSED) != 0 || x.u8(Obj::HIDDEN) != 0) {
        return;
    }
    match x.u8(Obj::STATE) {
        0 => start(x),
        2 => play(x),
        4 => finish(x),
        _ => x.free(&pools::SET),
    }
}

fn start(x: &mut Gx) {
    x.add8(Obj::STATE, 2);
    let entry = TABLE.wrapping_add((((x.u8(Obj::VARIANT) as u16) << 3) as i16) as u32);
    let (script, source) = (set_tables::long(x.b, entry), set_tables::long(x.b, entry + 4));
    x.set32(SOURCE, source);
    x.set32(SCRIPT, script);
    let page = PALETTE_PAGES[x.u8(PAGE).min(3) as usize];
    let first_line = set_tables::word(x.b, script) as i16;
    x.set32(DEST, page.wrapping_add(((first_line as u16) << 5) as i16 as i32 as u32));
    let timer = set_tables::word(x.b, script + 2);
    x.set16(TIMER, timer);
    let dest = x.u32(DEST);
    copy_lines(x, script + 4, source, dest);
}

fn play(x: &mut Gx) {
    if x.a5_u8(TRANSITION) != 0 {
        let variant = x.u8(Obj::VARIANT) as u32;
        let action = set_tables::pc_byte(x.b, TRANSITION_ACTION + variant);
        match action {
            0 => {}
            1 => return,
            2 => {
                x.set16(Obj::STATE, STATE_FINISH);
                return finish(x);
            }
            _ => return finish(x),
        }
    }
    let t = x.u16(TIMER).wrapping_sub(1);
    x.set16(TIMER, t);
    if (t as i16) > 0 {
        return;
    }
    let mut script = x.u32(SCRIPT);
    let source = x.u32(SOURCE);
    let duration = set_tables::word(x.b, script);
    script += 2;
    x.set16(TIMER, duration);
    if (duration as i16) < 0 {
        if duration == 0xffff {
            x.set16(Obj::STATE, 0);
            start(x);
        } else {
            let target = set_tables::long(x.b, script);
            x.set32(SCRIPT, target);
        }
    } else if duration == 0 {
        x.set16(Obj::STATE, STATE_FINISH);
        finish(x);
    } else {
        let dest = x.u32(DEST);
        copy_lines(x, script, source, dest);
    }
}

/// Copies the rows of a block (number, then one index per row) to the destination palette
/// and saves the continuation of the script.
/// Original glitch: the last long copied (with the brightness) stays in D4, which the unsteered raft
/// (0xad7b0) adds to the players' position: it is a leftover from the dispatch loop (`Leftovers::d4`).
fn copy_lines(x: &mut Gx, mut script: u32, source: u32, mut dest: u32) {
    let count = set_tables::word(x.b, script).wrapping_sub(1);
    script += 2;
    let mut last = 0;
    for _ in 0..=count as u32 {
        let line = (set_tables::word(x.b, script) << 5) as u32;
        script += 2;
        let keep = rd32(x.b, dest) & KEEP_BRIGHTNESS;
        for i in (0..LINE_BYTES).step_by(4) {
            last = set_tables::long(x.b, source + line + i) | keep;
            wr32(x.b, dest + i, last);
        }
        dest += LINE_BYTES;
    }
    x.set32(SCRIPT, script);
    x.leftovers().d4 = last;
}

fn finish(x: &mut Gx) {
    let variant = x.u8(Obj::VARIANT) as u32;
    match set_tables::pc_byte(x.b, END_ACTION + variant) {
        0 => x.set8(Obj::STATE, 6),
        n @ 1..=3 => {
            let (next, page) = NEXT_VARIANTS[n as usize - 1];
            x.set8(Obj::VARIANT, next);
            x.set8(PAGE, page);
            x.set32(Obj::STATE, 0);
        }
        _ => {}
    }
}
