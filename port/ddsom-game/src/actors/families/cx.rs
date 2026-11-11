//! Execution context of an object handler (ESHL, SET, SOBJ).
//!
//! A handler is called by the dispatch loop of its class for one object (A0 of the original), with the loop counter
//! (D7) and the leftovers that the handlers pass to each other (`dispatch::Dispatch`). `Cx` groups the bus, the
//! current object and this bench. The object engine (animation, movement, lists, screen, pools) is called through the
//! `objects::Current` trait (`x.move_xz()`, `x.anim_step()`, `x.reinsert()`...), the common services through
//! `machine::Machine` (`x.random()`).

use crate::world::World;

use crate::world::ram::{Byte, Long, Word};
use crate::core::dispatch::Dispatch;
use crate::world::Leftovers;
use crate::world::objects::pools;
use crate::world::objects::{Current, Obj};
use crate::{regs, Native};

/// Context of a handler: the current object (A0) and the machine.
///
/// `R` is the bench: in the game, what the dispatch loop passes ([`Gx`], no register: `x.c.d[0]` does not exist); in
/// the tools, the processor for the original code that calls a handler (`tools::families::cx`). The handlers are
/// written for both (`fn f<R: Bench>(x: &mut Cx<'_, R>)`).
pub struct Cx<'a, R> {
    pub c: R,
    pub b: &'a mut World,
    /// The object that the dispatch has processed (A0).
    pub o: Obj,
}

/// Context of a handler without registers: what the dispatch loop passes to it (`dispatch::Dispatch`).
pub type Gx<'a> = Cx<'a, &'a mut Dispatch>;

impl<'a> Gx<'a> {
    /// Context of the handler called by the dispatch loop.
    pub fn dispatch(b: &'a mut World, a: &'a mut Dispatch) -> Self {
        let o = a.object;
        Cx { c: a, b, o }
    }

    /// Leftover registers that handlers pass to each other (original glitches).
    pub fn leftovers(&mut self) -> &mut Leftovers {
        &mut self.c.leftovers
    }
}

impl<R> crate::world::machine::Machine for Cx<'_, R> {
    fn bus(&mut self) -> &mut World {
        &mut *self.b
    }
}

/// Register bench of a handler: what changes depending on whether it still works on the processor or not.
pub trait Bench {
    /// Re-listing (0x1ee1a / 0x1ef6c, `foreground`): without registers, the core, which leaves D2 to the next
    /// handler; on the processor (tools), the adapter leaves the original's registers.
    fn relink(&mut self, b: &mut World, o: Obj, foreground: bool);
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
    fn relink(&mut self, b: &mut World, o: Obj, foreground: bool) {
        let list = if foreground { crate::world::objects::lists::FOREGROUND } else { crate::world::objects::lists::ACTORS };
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
        let moved = crate::actors::ai::keep_on_screen(self.b, self.o, margin);
        *self.c.d5() = moved as u32;
        moved
    }

    /// 0xfbae: `keep_on_screen` with the play area margin (0x30).
    pub fn keep_in_zone(&mut self) -> bool {
        self.keep_on_screen(crate::actors::ai::ZONE_MARGIN)
    }

    /// 0x33fe4: collision with the terrain (`terrain::push`); returns the original's Z flag. Original glitch: the
    /// terrain test leaves D2 and D3, and a hit D5 = 1 (`Leftovers`).
    pub fn push_terrain(&mut self) -> bool {
        let (mut d2, mut d3, mut d5) = (*self.c.d2(), *self.c.d3(), *self.c.d5());
        let z = crate::world::terrain::push_leftovers(self.b, self.o, &mut d2, &mut d3, &mut d5);
        (*self.c.d2(), *self.c.d3(), *self.c.d5()) = (d2, d3, d5);
        z
    }
}

impl<R> Cx<'_, R> {
    // --- fields of the current object --------------------------------------------------------

    pub fn u8(&mut self, off: u32) -> u8 {
        self.o.u8(self.b, off)
    }
    pub fn u16(&mut self, off: u32) -> u16 {
        self.o.u16(self.b, off)
    }
    pub fn u32(&mut self, off: u32) -> u32 {
        self.o.u32(self.b, off)
    }
    pub fn set8(&mut self, off: u32, v: u8) {
        self.o.set_u8(self.b, off, v)
    }
    pub fn set16(&mut self, off: u32, v: u16) {
        self.o.set_u16(self.b, off, v)
    }
    pub fn set32(&mut self, off: u32, v: u32) {
        self.o.set_u32(self.b, off, v)
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
        Obj::from_word(self.u16(off))
    }
    /// Work RAM at `A5 + off` (the variables that the families still designate by their offset).
    pub fn a5_u8(&mut self, off: u32) -> u8 {
        Byte::a5(off).get(self.b)
    }
    pub fn a5_u16(&mut self, off: u32) -> u16 {
        Word::a5(off).get(self.b)
    }
    pub fn a5_u32(&mut self, off: u32) -> u32 {
        Long::a5(off).get(self.b)
    }
    pub fn set_a5_u8(&mut self, off: u32, v: u8) {
        Byte::a5(off).set(self.b, v)
    }
    pub fn set_a5_u16(&mut self, off: u32, v: u16) {
        Word::a5(off).set(self.b, v)
    }
    pub fn set_a5_u32(&mut self, off: u32, v: u32) {
        Long::a5(off).set(self.b, v)
    }

    /// Word of the work RAM at `A5 + offset` (sign-extended offset).
    pub fn ram16(&mut self, offset: i32) -> u16 {
        Word(crate::hw::mem::A5.wrapping_add(offset as u32)).get(self.b)
    }

    /// The frame camera: screen (x, y), the alternate camera when it is selected (`ram::camera`).
    pub fn camera(&mut self) -> (u16, u16) {
        crate::world::ram::camera(self.b)
    }

    /// End of the handler: the object is freed (shot destroyed).
    pub fn free_eshl(&mut self)
    where
        Self: Current,
    {
        self.free(&pools::ESHL);
    }
}

/// Declares the handlers of a type file: `NAME address "name" path::to::handler` (on a generic `Cx`, called as a
/// handler without registers: what the dispatch loop passes, `Gx`). Each becomes a constant `NAME`, which the dispatch
/// tables (`dispatch`) hold; `NATIVES` lists them for the tools.
macro_rules! handlers {
    ($( $const:ident $entry:literal $name:literal $f:path ),* $(,)?) => {
        $(
            pub const $const: $crate::Native = $crate::actors::families::cx::handler_without($entry, $name, |b, a| {
                $f(&mut $crate::actors::families::cx::Gx::dispatch(b, a));
            });
        )*
        pub const NATIVES: &[$crate::Native] = &[ $( $const ),* ];
    };
}
pub(crate) use handlers;

/// Native of a converted handler (`Gx`, without registers): what the dispatch loop passes as argument.
pub const fn handler_without(entry: u32, name: &'static str, run: fn(&mut World, &mut Dispatch)) -> Native {
    Native { entry, name, run: crate::Body::Handler(run), touches_sound: false, outputs: regs::a(0) | regs::d(7) }
}

impl<R> crate::world::objects::current::Current for Cx<'_, R> {
    fn obj(&self) -> crate::world::objects::Obj {
        self.o
    }
}
