//! Tools: the natives of `crate::families::cx` for the original code that calls them (arguments and results in the registers
//! of the original).

use cps2::Cps2Bus;
use m68k::Cpu;

use crate::families::cx::*;
use crate::tools::frames::*;
use crate::mem::A5;
use crate::objects::Obj;

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

    // --- subroutines ---------------------------------------------------------------------------

    /// JSR `addr` with A0 = the object and A5 = base of the work RAM.
    #[track_caller]
    pub fn call(&mut self, addr: u32) {
        self.c.a[0] = self.o.0;
        self.c.a[5] = A5;
        crate::tools::call(self.c, self.b, addr);
    }

    /// RTS.
    pub(crate) fn ret(self) {
        rts(self.c, self.b);
    }
}
