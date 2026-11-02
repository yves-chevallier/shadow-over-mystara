//! Tools: the natives of `crate::families::cx` for the original code that calls them (arguments and results in the registers
//! of the original).

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::{Cpu, SR_C, SR_N, SR_V, SR_X, SR_Z};

use crate::families::cx::*;
use crate::tools::frames::*;
use crate::{regs, Body, Native};
use crate::mem::{rd16, rd32, rd8, wr16, wr32, wr8, A5};
use crate::dispatch::{Dispatch, Leftovers};
use crate::objects::pools;
use crate::objects::{Current, Obj};

impl crate::tools::WithCpu for Cx<'_, &mut Cpu> {
    fn machine(&mut self) -> (&mut Cpu, &mut Cps2Bus) {
        (&mut *self.c, &mut *self.b)
    }
}

impl Bench for &mut Cpu {
    #[track_caller]
    fn relink(&mut self, b: &mut Cps2Bus, o: Obj, foreground: bool) {
        let list = if foreground { crate::objects::lists::FOREGROUND } else { crate::objects::lists::ACTORS };
        list.reinsert_68k(self, b, o);
    }
    fn d2(&mut self) -> &mut u32 {
        &mut self.d[2]
    }
    fn d3(&mut self) -> &mut u32 {
        &mut self.d[3]
    }
    fn d4(&mut self) -> &mut u32 {
        &mut self.d[4]
    }
    fn d5(&mut self) -> &mut u32 {
        &mut self.d[5]
    }
    fn counter(&self) -> u16 {
        self.d[7] as u16
    }
}

impl<'a> Cx<'a, &'a mut Cpu> {
    /// Context for the object designated by A0.
    pub fn new(c: &'a mut Cpu, b: &'a mut Cps2Bus) -> Self {
        let o = Obj(c.a[0]);
        Cx { c, b, o }
    }

    /// Calls the converted function `f` on the same object, without registers (the leftovers go through the processor).
    pub fn without<T>(&mut self, f: impl FnOnce(&mut Gx) -> T) -> T {
        let mut a = Dispatch::from_cpu(self.c);
        a.object = self.o;
        let r = f(&mut Cx { c: &mut a, b: &mut *self.b, o: self.o });
        a.give_back(self.c);
        r
    }

    // --- subroutines ---------------------------------------------------------------------------

    /// JSR `addr` with A0 = the object and A5 = base of the work RAM.
    #[track_caller]
    pub fn call(&mut self, addr: u32) {
        self.c.a[0] = self.o.0;
        self.c.a[5] = A5;
        crate::tools::call(self.c, self.b, addr);
    }

    /// RTS.
    pub fn ret(self) {
        rts(self.c, self.b);
    }
}

/// Native of a handler: contract of the dispatch loop (A0 and D7 preserved).
pub const fn handler(entry: u32, name: &'static str, run: fn(&mut Cpu, &mut Cps2Bus)) -> Native {
    Native { entry, name, run: crate::Body::WithCpu(run), touches_sound: false, outputs: regs::a(0) | regs::d(7) }
}
