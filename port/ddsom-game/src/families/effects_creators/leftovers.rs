//! What a routine of the batch leaves in the original's registers.

use crate::world::World;

use crate::ops::Size;

use crate::mem::{set_low_byte, set_low_word};
use crate::objects::pools::Pool;
use crate::objects::Obj;

/// The core of a routine is written once on a `Regs`: [`Pure`] writes no register, the game's leftovers
/// (`dispatch::Leftovers`, [`OnBench`]) keep D2-D5 that the original leaves to what follows; in the tools, the
/// processor (natives for the original code) receives all those that the original leaves (allocation included: A1,
/// D6 and the carry).
pub(crate) trait Regs {
    fn allocated(&mut self, b: &mut World, pool: &Pool) -> Option<Obj>;
    /// Original glitch: allocation whose failure is not tested (see `Pool::alloc_unchecked`).
    fn alloc_without_test(&mut self, b: &mut World, pool: &Pool) -> Obj;
    /// Allocation whose failure returns the A1 that the original leaves (see `Pool::alloc_or_leftover`).
    fn alloc_or_rest(&mut self, b: &mut World, pool: &Pool) -> Result<Obj, Obj>;
    /// Flags of a `move`/`tst`/`and` of `v`.
    fn flags(&mut self, _v: u32, _size: Size) {}
    /// X flag alone (last bit shifted out, which the following instructions keep).
    fn x(&mut self, _bit: bool) {}
    /// `cmp`: flags of `dst - src` (X kept).
    fn comparison(&mut self, _src: u32, _dst: u32, _size: Size) {}
    /// `add`: result and flags.
    fn addition(&mut self, src: u32, dst: u32, size: Size) -> u32 {
        mask(dst.wrapping_add(src), size)
    }
    /// `sub`: `dst - src`, result and flags.
    fn subtract(&mut self, src: u32, dst: u32, size: Size) -> u32 {
        mask(dst.wrapping_sub(src), size)
    }
    fn d(&mut self, _n: usize, _v: u32) {}
    fn d_word(&mut self, _n: usize, _v: u16) {}
    fn d_byte(&mut self, _n: usize, _v: u8) {}
    fn a(&mut self, _n: usize, _v: u32) {}
    /// `move #1,CCR`: the carry alone (refusal).
    fn carry_only(&mut self) {}
}

fn mask(v: u32, size: Size) -> u32 {
    match size {
        Size::Byte => v & 0xff,
        Size::Word => v & 0xffff,
        _ => v,
    }
}

/// The core: no register.
pub(crate) struct Pure;

impl Regs for Pure {
    fn allocated(&mut self, b: &mut World, pool: &Pool) -> Option<Obj> {
        pool.alloc(b)
    }
    fn alloc_without_test(&mut self, b: &mut World, pool: &Pool) -> Obj {
        pool.alloc_unchecked(b)
    }
    fn alloc_or_rest(&mut self, b: &mut World, pool: &Pool) -> Result<Obj, Obj> {
        pool.alloc_or_leftover(b)
    }
}

/// Register `n` among the leftovers D2-D5.
pub(crate) fn leftover(l: &mut crate::dispatch::Leftovers, n: usize) -> Option<&mut u32> {
    match n {
        2 => Some(&mut l.d2),
        3 => Some(&mut l.d3),
        4 => Some(&mut l.d4),
        5 => Some(&mut l.d5),
        _ => None,
    }
}

/// Code without a bench that keeps by name the leftovers D2-D5 that the original leaves to what follows
/// (`player::Ctx::leftovers`); the rest is dead.
impl Regs for crate::dispatch::Leftovers {
    fn allocated(&mut self, b: &mut World, pool: &Pool) -> Option<Obj> {
        pool.alloc(b)
    }
    fn alloc_without_test(&mut self, b: &mut World, pool: &Pool) -> Obj {
        pool.alloc_unchecked(b)
    }
    fn alloc_or_rest(&mut self, b: &mut World, pool: &Pool) -> Result<Obj, Obj> {
        pool.alloc_or_leftover(b)
    }
    fn d(&mut self, n: usize, v: u32) {
        if let Some(r) = leftover(self, n) {
            *r = v;
        }
    }
    fn d_word(&mut self, n: usize, v: u16) {
        if let Some(r) = leftover(self, n) {
            set_low_word(r, v);
        }
    }
    fn d_byte(&mut self, n: usize, v: u8) {
        if let Some(r) = leftover(self, n) {
            set_low_byte(r, v);
        }
    }
}

/// The core called by a generic handler (`Cx<'_, R: Bench>`): only the leftovers D2-D5 that the original leaves to
/// the following handlers (`dispatch::Leftovers`) are kept, in the bench; the rest is dead.
pub(crate) struct OnBench<'a, R>(pub &'a mut R);

impl<R: crate::families::cx::Bench> OnBench<'_, R> {
    fn remainder(&mut self, n: usize) -> Option<&mut u32> {
        match n {
            2 => Some(self.0.d2()),
            3 => Some(self.0.d3()),
            4 => Some(self.0.d4()),
            5 => Some(self.0.d5()),
            _ => None,
        }
    }
}

impl<R: crate::families::cx::Bench> Regs for OnBench<'_, R> {
    fn allocated(&mut self, b: &mut World, pool: &Pool) -> Option<Obj> {
        pool.alloc(b)
    }
    fn alloc_without_test(&mut self, b: &mut World, pool: &Pool) -> Obj {
        pool.alloc_unchecked(b)
    }
    fn alloc_or_rest(&mut self, b: &mut World, pool: &Pool) -> Result<Obj, Obj> {
        pool.alloc_or_leftover(b)
    }
    fn d(&mut self, n: usize, v: u32) {
        if let Some(r) = self.remainder(n) {
            *r = v;
        }
    }
    fn d_word(&mut self, n: usize, v: u16) {
        if let Some(r) = self.remainder(n) {
            set_low_word(r, v);
        }
    }
    fn d_byte(&mut self, n: usize, v: u8) {
        if let Some(r) = self.remainder(n) {
            set_low_byte(r, v);
        }
    }
}
