//! Temporary bridge from the code without registers (kernel, tasks, screens, game loop) to the code still on the
//! processor's register bench (player update, combat, a few services and natives called by address): see
//! `docs/refactorisation.md`, "Sortie du Cpu, partie 7".
//!
//! The game no longer has a register bench of its own: each call into code on the processor gets a fresh bench, on
//! which only the game's leftovers (`dispatch::Leftovers`, D2-D5: the registers that a routine leaves and a later
//! one rereads, original glitches) enter and leave by name, plus A5 (base of the work RAM). Nothing else passes from
//! one call to the next: probe `DDSOM_POISON=bench` (all the other registers and the flags inverted on entry, site =
//! the caller of [`on_bench`]).
//!
//! When the code on the processor has been converted, this module disappears with `m68k::Cpu`.

use cps2::Cps2Bus;
use m68k::Cpu;

use crate::dispatch::Leftovers;
use crate::mem::A5;

/// Top of the supervisor stack of the original (`FF0E66`): the RAM below it is that of the original stacks, which
/// the game no longer reads (`tasks::table::kernel_ram`).
const STACK_TOP: u32 = 0xff_0e66;

/// Runs `f` (code on the processor) on a fresh register bench: the leftovers D2-D5 enter and leave it.
#[track_caller]
pub fn on_bench<T>(b: &mut Cps2Bus, f: impl FnOnce(&mut Cpu, &mut Cps2Bus) -> T) -> T {
    let mut c = Cpu::new();
    c.sr = 0;
    c.a[5] = A5;
    c.a[7] = STACK_TOP;
    Leftovers::to_bench(&mut c, b);
    // everything but the leftovers, A5 and A7
    const OTHERS: u32 = crate::diag::TASK_REGS & !(0xf << 2);
    crate::diag::dead_regs(&mut c, "bench", OTHERS);
    let r = f(&mut c, b);
    Leftovers::from_bench(&c, b);
    r
}
