//! Tools: the natives of `crate::service::memory` for the original code that calls them (arguments and results in the registers
//! of the original).

use cps2::Cps2Bus;
use m68k::Cpu;

use crate::mem::rd32;

/// Patch common to both machines (`lib.rs`, `patches`): the original test (`0x2cf26`, entered by
/// `bsr`) is run until its return by the interpreter, without advancing the machine's time.
/// After an "NG", the test loops forever: execution stops after an instruction budget and
/// the machine resumes the loop.
pub(crate) fn patch_instant_check(c: &mut Cpu, b: &mut Cps2Bus) -> bool {
    let cycles = c.cycles;
    let sp = c.a[7];
    let ret = rd32(b, sp);
    let mut budget = 200_000_000u32;
    loop {
        c.step(b);
        budget -= 1;
        if (c.pc == ret && c.a[7] == sp.wrapping_add(4)) || budget == 0 {
            break;
        }
    }
    c.cycles = cycles;
    true
}
