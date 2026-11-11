//! The object engine seen from a handler: the routines applied to the object being processed.
//!
//! The [`Current`] trait gives the contexts that process an object (`families::cx::Cx`) the engine
//! routines under Rust names: `x.move_xz()`, `x.anim_step()`, `x.reinsert()`... instead of
//! `x.call(sub::MOVE_XZ)`. Each method calls the module function (`motion`, `anim`, `lists`, `cull`, `pools`)
//! on `self.obj()`.
//!
//! The methods call the core (`motion::ballistic(b, o, motion::XZ)`): no register. The re-listing, which leaves D2 to
//! the next handler, goes through the bench of the handler (`families::cx::Bench::relink`). The tools keep, for the
//! original code, the natives with the registers that the original leaves (`tools::objects`).

use super::anim::{self, CharacterSets};
use super::cull;
use super::lists::{self};
use super::motion::{self, LastAdd, SpeedTables, AXIS_Y, AXIS_Z};
use super::pools::Pool;
use super::Obj;
use crate::world::machine::Machine;

/// Context that processes an object (A0 of the original).
pub trait Current: Machine {
    /// The object being processed.
    fn obj(&self) -> Obj;

    // --- pools ----------------------------------------------------------------------------------

    /// Allocates an object in `pool`: the reserved slot (class written, not yet active), or `None` if the pool
    /// is full. Core.
    fn alloc(&mut self, pool: &Pool) -> Option<Obj> {
        pool.alloc(self.bus())
    }

    /// Allocation whose failure the original does not test (see `Pool::alloc_unchecked`, original glitch). Core.
    fn alloc_unchecked(&mut self, pool: &Pool) -> Obj {
        pool.alloc_unchecked(self.bus())
    }

    /// Allocation whose failure returns the A1 that the original leaves (see `Pool::alloc_or_leftover`). Core.
    fn alloc_or_leftover(&mut self, pool: &Pool) -> Result<Obj, Obj> {
        pool.alloc_or_leftover(self.bus())
    }

    /// Frees the processed object in its pool (end of handler: the object disappears). Core.
    fn free(&mut self, pool: &Pool) {
        let o = self.obj();
        pool.release(self.bus(), o)
    }

    // --- animation (`anim`) -----------------------------------------------------------------------

    /// 0x1a32: starts script `n` of the animation set `set` (`lea set,A4 ; moveq #n,D0 ; jsr 0x1a32`). Returns
    /// the address following the loaded entry (never zero: the `bne` that sometimes follows the call is always taken).
    fn anim_start(&mut self, set: u32, n: u16) -> u32 {
        let o = self.obj();
        let b = self.bus();
        anim::start(b, o, set, n)
    }

    /// 0x1a3c: starts the animation at entry `at`.
    fn anim_start_entry(&mut self, at: u32) {
        let o = self.obj();
        let b = self.bus();
        anim::start_entry(b, o, at);
    }

    /// 0x2c16, 0x2c88, 0x2cf0: starts animation `n` of the set specific to the object's character.
    fn anim_start_by_character(&mut self, sets: &CharacterSets, n: u16) {
        let o = self.obj();
        let b = self.bus();
        anim::start_by_character(b, o, sets, n);
    }

    /// 0x1a56: one animation step; true when the script is finished, or the duration was zero (carry of
    /// the original, see `anim::Step::finished`).
    fn anim_step(&mut self) -> bool {
        let o = self.obj();
        let b = self.bus();
        anim::step(b, o, false).finished()
    }

    /// 0x2d58: one animation step with long entries (characters); true at the end of the script.
    fn anim_step_long(&mut self) -> bool {
        let o = self.obj();
        let b = self.bus();
        anim::step(b, o, true).finished()
    }

    // --- motion (`motion`) ---------------------------------------------------------------------
    // The motions return the last addition (`LastAdd`): `if x.move_xz().gt()` replaces the `bgt` of
    // the original; a caller that tests nothing ignores the value.

    /// Ballistic motions (speed += acceleration, position += speed) on the axes of the original routine.
    fn move_axes(&mut self, axes: motion::Axes) -> LastAdd {
        let o = self.obj();
        let b = self.bus();
        motion::ballistic(b, o, axes)
    }
    /// 0x1d78 : X.
    fn move_x(&mut self) -> LastAdd {
        self.move_axes(motion::X)
    }
    /// 0x1e02: Y (depth).
    fn move_y(&mut self) -> LastAdd {
        self.move_axes(motion::Y)
    }
    /// 0x1dbc: Z (altitude).
    fn move_z(&mut self) -> LastAdd {
        self.move_axes(motion::Z)
    }
    /// 0x1d62: Y then X.
    fn move_xy(&mut self) -> LastAdd {
        self.move_axes(motion::XY)
    }
    /// 0x1da6: X then Z.
    fn move_xz(&mut self) -> LastAdd {
        self.move_axes(motion::XZ)
    }
    /// 0x1dd4: Y then Z.
    fn move_yz(&mut self) -> LastAdd {
        self.move_axes(motion::YZ)
    }
    /// 0x1d90: Y, X then Z.
    fn move_xyz(&mut self) -> LastAdd {
        self.move_axes(motion::XYZ)
    }

    /// 0x1cb4: X and Y from the object's speed table.
    fn move_by_table_xy(&mut self) -> LastAdd {
        let o = self.obj();
        let b = self.bus();
        motion::move_by_table(b, o, AXIS_Y)
    }
    /// 0x1cee: X and Z from the object's speed table.
    fn move_by_table_xz(&mut self) -> LastAdd {
        let o = self.obj();
        let b = self.bus();
        motion::move_by_table(b, o, AXIS_Z)
    }

    /// 0x1a94 / 0x1b64: selects speed table `n` of `tables` (`motion::SPEEDS_A`, `motion::SPEEDS_B`) and
    /// takes its current speed.
    fn select_speed(&mut self, tables: &SpeedTables, n: u16) {
        let o = self.obj();
        let b = self.bus();
        motion::select_speed(b, o, tables, n);
    }

    /// 0x29ea: the object is suspended (byte `Obj::HIDDEN`).
    fn suspended(&mut self) -> bool {
        let o = self.obj();
        let b = self.bus();
        motion::suspended(b, o)
    }

    /// 0xf1bfc: end-of-handler counters.
    fn tick_counters(&mut self) {
        let o = self.obj();
        let b = self.bus();
        motion::tick_counters(b, o);
    }

    // --- display lists (`lists`) -------------------------------------------------------------

    /// 0x1f0be: removes the object from its list. Core (the original leaves no useful register).
    fn unlink(&mut self) {
        let o = self.obj();
        lists::unlink(self.bus(), o)
    }

    // --- screen (`cull`) ----------------------------------------------------------------------------

    /// 0x1e1a: an active object that left the screen goes to state 6.
    fn cull_offscreen(&mut self) {
        let o = self.obj();
        let b = self.bus();
        cull::despawn_if_offscreen(b, o);
    }

    /// 0x1e92: the object is within the play area widened by `margin`.
    fn in_arena(&mut self, margin: u16) -> bool {
        let o = self.obj();
        let b = self.bus();
        cull::in_arena(b, o, margin)
    }

    /// 0x1e90: `in_arena` with the default margin (`cull::ARENA_MARGIN`).
    fn in_arena_default(&mut self) -> bool {
        self.in_arena(cull::ARENA_MARGIN)
    }

    /// 0x1ed2: visibility for the frame and registration for collisions; true if the object is visible.
    fn update_visibility(&mut self) -> bool {
        let o = self.obj();
        let b = self.bus();
        cull::update_visibility(b, o)
    }
}

impl<R> Current for crate::actors::families::cx::Cx<'_, R> {
    fn obj(&self) -> Obj {
        self.o
    }
}
