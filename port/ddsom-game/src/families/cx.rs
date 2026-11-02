//! Execution context of an object handler (ESHL, SET, SOBJ).
//!
//! A handler is called by the dispatch loop of its class for one object (A0 of the original), with the loop counter
//! (D7) and the leftovers that the handlers pass to each other (`dispatch::Dispatch`). `Cx` groups the bus, the
//! current object and this bench. The object engine (animation, movement, lists, screen, pools) is called through the
//! `objects::Current` trait (`x.move_xz()`, `x.anim_step()`, `x.reinsert()`...), the common services through
//! `machine::Machine` (`x.random()`).

use cps2::Cps2Bus;

use crate::mem::{rd16, rd32, rd8, wr16, wr32, wr8, A5};
use crate::dispatch::{Dispatch, Leftovers};
use crate::objects::pools;
use crate::objects::{Current, Obj};
use crate::{regs, Native};

/// Context of a handler: the current object (A0) and the machine.
///
/// `R` is the bench: in the game, what the dispatch loop passes ([`Gx`], no register: `x.c.d[0]` does not exist); in
/// the tools, the processor for the original code that calls a handler (`tools::families::cx`). The handlers are
/// written for both (`fn f<R: Bench>(x: &mut Cx<'_, R>)`).
pub struct Cx<'a, R> {
    pub c: R,
    pub b: &'a mut Cps2Bus,
    /// The object that the dispatch has processed (A0).
    pub o: Obj,
}

/// Context of a handler without registers: what the dispatch loop passes to it (`dispatch::Dispatch`).
pub type Gx<'a> = Cx<'a, &'a mut Dispatch>;

impl<'a> Gx<'a> {
    /// Context of the handler called by the dispatch loop.
    pub fn dispatch(b: &'a mut Cps2Bus, a: &'a mut Dispatch) -> Self {
        let o = a.object;
        Cx { c: a, b, o }
    }

    /// Leftover registers that handlers pass to each other (original glitches).
    pub fn leftovers(&mut self) -> &mut Leftovers {
        &mut self.c.leftovers
    }
}

impl<R> crate::machine::Machine for Cx<'_, R> {
    fn bus(&mut self) -> &mut Cps2Bus {
        &mut *self.b
    }
}

/// Register bench of a handler: what changes depending on whether it still works on the processor or not.
pub trait Bench {
    /// Re-listing (0x1ee1a / 0x1ef6c, `foreground`): without registers, the core, which leaves D2 to the next
    /// handler; on the processor (tools), the adapter leaves the original's registers.
    fn relink(&mut self, b: &mut Cps2Bus, o: Obj, foreground: bool);
    /// The leftovers D2 to D5 (see `dispatch::Leftovers`): the processor register, or the leftover of the dispatch
    /// loop.
    fn d2(&mut self) -> &mut u32;
    fn d3(&mut self) -> &mut u32;
    fn d4(&mut self) -> &mut u32;
    fn d5(&mut self) -> &mut u32;
    /// Dispatch loop counter (D7.w of the original): phase of the "every other frame" blinking.
    fn counter(&self) -> u16;
}

impl Bench for &mut Dispatch {
    fn relink(&mut self, b: &mut Cps2Bus, o: Obj, foreground: bool) {
        let list = if foreground { crate::objects::lists::FOREGROUND } else { crate::objects::lists::ACTORS };
        // Original glitch: the re-listing leaves D2 (the link left) to the next handler
        list.reinsert_leftovers(b, o, &mut self.leftovers.d2);
    }
    fn d2(&mut self) -> &mut u32 {
        &mut self.leftovers.d2
    }
    fn d3(&mut self) -> &mut u32 {
        &mut self.leftovers.d3
    }
    fn d4(&mut self) -> &mut u32 {
        &mut self.leftovers.d4
    }
    fn d5(&mut self) -> &mut u32 {
        &mut self.leftovers.d5
    }
    fn counter(&self) -> u16 {
        self.counter
    }
}

impl<R: Bench> Cx<'_, R> {
    /// Dispatch loop counter (D7.w of the original).
    pub fn counter(&self) -> u16 {
        self.c.counter()
    }

    /// 0x1ee1a: puts the object back in its place in the actor list (ordinary end of a handler; see [`Bench`]).
    #[track_caller]
    pub fn reinsert(&mut self) {
        self.c.relink(self.b, self.o, false);
    }

    /// 0x1ef6c: puts the object back in its place in the foreground list (see [`Bench`]).
    #[track_caller]
    pub fn reinsert_foreground(&mut self) {
        self.c.relink(self.b, self.o, true);
    }

    /// 0xfbb0: keeps the object on the screen within `margin` pixels; true if it was moved. Original glitch: the result
    /// stays in D5 (0 or 1), which the burst row of a crate rereads (`Leftovers::d5`).
    pub fn keep_on_screen(&mut self, margin: u16) -> bool {
        let moved = crate::ai::keep_on_screen(self.b, self.o, margin);
        *self.c.d5() = moved as u32;
        moved
    }

    /// 0xfbae: `keep_on_screen` with the play area margin (0x30).
    pub fn keep_in_zone(&mut self) -> bool {
        self.keep_on_screen(crate::ai::ZONE_MARGIN)
    }

    /// 0x33fe4: collision with the terrain (`terrain::push`); returns the original's Z flag. Original glitch: the
    /// terrain lookup leaves D2 and D3, and a hit D5 = 1 (`Leftovers`).
    pub fn push_terrain(&mut self) -> bool {
        let (mut d2, mut d3, mut d5) = (*self.c.d2(), *self.c.d3(), *self.c.d5());
        let z = crate::terrain::push_leftovers(self.b, self.o, &mut d2, &mut d3, &mut d5);
        (*self.c.d2(), *self.c.d3(), *self.c.d5()) = (d2, d3, d5);
        z
    }
}

/// Subroutines called by address (the object engine, for its part, is called through `objects::Current`).
pub mod sub {
    /// Attribute of the terrain under the object (D2); `terrain_at` takes (D0, D1) = (x, y).
    pub const TERRAIN_OF_OBJECT: u32 = 0x24a8;
    pub const TERRAIN_AT: u32 = 0x24b0;
}

impl<R> Cx<'_, R> {
    // --- fields of the current object --------------------------------------------------------

    pub fn u8(&mut self, off: u32) -> u8 {
        rd8(self.b, self.o.0 + off)
    }
    pub fn u16(&mut self, off: u32) -> u16 {
        rd16(self.b, self.o.0 + off)
    }
    pub fn u32(&mut self, off: u32) -> u32 {
        rd32(self.b, self.o.0 + off)
    }
    pub fn set8(&mut self, off: u32, v: u8) {
        wr8(self.b, self.o.0 + off, v)
    }
    pub fn set16(&mut self, off: u32, v: u16) {
        wr16(self.b, self.o.0 + off, v)
    }
    pub fn set32(&mut self, off: u32, v: u32) {
        wr32(self.b, self.o.0 + off, v)
    }
    /// `add.w #v,(off,A0)`: in-place addition modulo 2^16.
    pub fn add16(&mut self, off: u32, v: u16) {
        let x = self.u16(off).wrapping_add(v);
        self.set16(off, x)
    }
    pub fn add8(&mut self, off: u32, v: u8) {
        let x = self.u8(off).wrapping_add(v);
        self.set8(off, x)
    }
    pub fn add32(&mut self, off: u32, v: u32) {
        let x = self.u32(off).wrapping_add(v);
        self.set32(off, x)
    }
    /// Replaces a bit of a byte of the record (`bset` / `bclr`).
    pub fn set_bit(&mut self, off: u32, bit: u8, on: bool) {
        let v = self.u8(off);
        self.set8(off, if on { v | 1 << bit } else { v & !(1 << bit) });
    }
    pub fn bit(&mut self, off: u32, bit: u8) -> bool {
        self.u8(off) >> bit & 1 != 0
    }

    // --- other objects and game variables ---------------------------------------------------

    /// Object pointed to by a 16-bit field (`movea.w (off,A0),A1`: sign-extended address).
    pub fn linked(&mut self, off: u32) -> Obj {
        Obj(self.u16(off) as i16 as i32 as u32 & 0xff_ffff)
    }
    pub fn a5_u8(&mut self, off: u32) -> u8 {
        rd8(self.b, A5 + off)
    }
    pub fn a5_u16(&mut self, off: u32) -> u16 {
        rd16(self.b, A5 + off)
    }
    pub fn a5_u32(&mut self, off: u32) -> u32 {
        rd32(self.b, A5 + off)
    }
    pub fn set_a5_u8(&mut self, off: u32, v: u8) {
        wr8(self.b, A5 + off, v)
    }
    pub fn set_a5_u16(&mut self, off: u32, v: u16) {
        wr16(self.b, A5 + off, v)
    }
    pub fn set_a5_u32(&mut self, off: u32, v: u32) {
        wr32(self.b, A5 + off, v)
    }
    /// Word and long read in data space (ROM tables addressed in absolute).
    pub fn data16(&mut self, addr: u32) -> u16 {
        rd16(self.b, addr)
    }
    pub fn data32(&mut self, addr: u32) -> u32 {
        rd32(self.b, addr)
    }
    /// Word read in program space (PC-relative addressed table).
    pub fn rom16(&mut self, addr: u32) -> u16 {
        self.b.fetch16(addr)
    }
    pub fn rom8(&mut self, addr: u32) -> u8 {
        let w = self.b.fetch16(addr & !1);
        if addr & 1 == 0 { (w >> 8) as u8 } else { w as u8 }
    }
    pub fn data8(&mut self, addr: u32) -> u8 {
        rd8(self.b, addr)
    }
    pub fn rom_i16(&mut self, addr: u32) -> i16 {
        self.b.fetch16(addr) as i16
    }

    /// Byte of the work RAM at `A5 + offset` (sign-extended offset).
    pub fn ram8(&mut self, offset: i32) -> u8 {
        rd8(self.b, A5.wrapping_add(offset as u32))
    }
    pub fn ram16(&mut self, offset: i32) -> u16 {
        rd16(self.b, A5.wrapping_add(offset as u32))
    }
    pub fn set_ram8(&mut self, offset: i32, v: u8) {
        wr8(self.b, A5.wrapping_add(offset as u32), v)
    }

    /// The frame camera: screen (x, y), the alternate camera when `A5+0x36` is set.
    pub fn camera(&mut self) -> (u16, u16) {
        if self.a5_u8(0x36) != 0 {
            (self.a5_u16(0x556), self.a5_u16(0x55a))
        } else {
            (self.a5_u16(0x506), self.a5_u16(0x50a))
        }
    }

    /// End of the handler: the object is freed (shot destroyed).
    pub fn free_eshl(&mut self)
    where
        Self: Current,
    {
        self.free(&pools::ESHL);
    }
}

/// Declares the `NATIVES` table of a type file: `address "name" path::to::handler` (on a generic `Cx`, called as a
/// handler without registers: what the dispatch loop passes, `Gx`).
macro_rules! handlers {
    ($( $entry:literal $name:literal $f:path ),* $(,)?) => {
        pub const NATIVES: &[$crate::Native] = &[ $(
            $crate::families::cx::handlers!(@one $entry $name $f),
        )* ];
    };
    (@one $entry:literal $name:literal $f:path) => {
        $crate::families::cx::handler_without($entry, $name, |b, a| {
            $f(&mut $crate::families::cx::Gx::dispatch(b, a));
        })
    };
}
pub(crate) use handlers;

/// Native of a converted handler (`Gx`, without registers): what the dispatch loop passes as argument.
pub const fn handler_without(entry: u32, name: &'static str, run: fn(&mut Cps2Bus, &mut Dispatch)) -> Native {
    Native { entry, name, run: crate::Body::Handler(run), touches_sound: false, outputs: regs::a(0) | regs::d(7) }
}
