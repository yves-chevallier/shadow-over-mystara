//! Object dispatch (W0): the ten loops `0x398e2-0x39f82` that walk a pool and call,
//! for each active object, the handler of its type.
//!
//! Each loop of the original is the same: a slot whose `Obj::ACTIVE` byte is zero is
//! skipped, otherwise the handler of `Obj::KIND` is called, read from a table of longs stored in the
//! code. Here, a `Class` describes the pool and its handlers, a Rust array indexed by the type, built at compile
//! time from the original table; `update` is the single loop. The original addresses (of the tables, of the loops,
//! of the handlers) are only metadata for the tools and for the comparison of the task table.
//!
//! A handler can make the task cede control (fade to black of the doors, fireworks): the loop is then
//! abandoned, and the task resumes it at the step [`Step::Dispatch`] (after the current slot, with the object and
//! the counter noted in the step), then at the continuation of the loop's caller.

use crate::world::World;

use crate::actors::families as f;
use crate::screens::modes::Flow;
use crate::world::objects::pools::{self, Pool};
use crate::world::objects::Obj;
use crate::{Body, Native};

/// A class of objects: its pool and its handlers.
pub struct Class {
    pub pool: &'static Pool,
    /// Handlers, indexed by `Obj::KIND`.
    pub handlers: &'static [Handler],
    /// Index of the class in [`CLASSES`] (the original loop that a [`Step::Dispatch`] resumes).
    index: u8,
    /// Address of the original's handler table (longs, one per type, in the code): metadata for the tools.
    pub table: u32,
    /// Address of the original's dispatch loop.
    pub entry: u32,
}

/// The handler of a type, in a dispatch table.
#[derive(Clone, Copy)]
pub enum Handler {
    /// A Rust handler without registers, and its native (original address and name: tools, diagnostics).
    Rust { run: fn(&mut World, &mut Dispatch), native: &'static Native },
    /// An entry of the original table whose code no object ever runs (`docs/code-mort.md`, batch L13): its address.
    Dead { entry: u32 },
}

/// The handler of a native declared by `handler!` or `handlers!` (checked at compile time).
const fn h(native: &'static Native) -> Handler {
    match native.run {
        Body::Handler(run) => Handler::Rust { run, native },
        _ => panic!("a dispatch table holds handlers without registers"),
    }
}

pub const PSHL: Class = Class { pool: &pools::PSHL, handlers: &PSHL_HANDLERS, index: 0, table: 0x39908, entry: 0x398e2 };
pub const PEFF: Class = Class { pool: &pools::PEFF, handlers: &PEFF_HANDLERS, index: 1, table: 0x39992, entry: 0x3996c };
pub const ENEMY: Class =
    Class { pool: &pools::ENEMY, handlers: &ENEMY_HANDLERS, index: 2, table: 0x39a78, entry: 0x39a52 };
pub const BOSS: Class = Class { pool: &pools::BOSS, handlers: &BOSS_HANDLERS, index: 3, table: 0x39b12, entry: 0x39aec };
pub const ESHL: Class = Class { pool: &pools::ESHL, handlers: &ESHL_HANDLERS, index: 4, table: 0x39b96, entry: 0x39b70 };
pub const EFFCT: Class =
    Class { pool: &pools::EFFCT, handlers: &EFFCT_HANDLERS, index: 5, table: 0x39ce0, entry: 0x39cba };
pub const BOX: Class = Class { pool: &pools::BOX, handlers: &BOX_HANDLERS, index: 6, table: 0x39e76, entry: 0x39e50 };
pub const SET: Class = Class { pool: &pools::SET, handlers: &SET_HANDLERS, index: 7, table: 0x39eac, entry: 0x39e86 };
pub const ITEM: Class = Class { pool: &pools::ITEM, handlers: &ITEM_HANDLERS, index: 8, table: 0x39f5a, entry: 0x39f34 };
pub const SOBJ: Class = Class { pool: &pools::SOBJ, handlers: &SOBJ_HANDLERS, index: 9, table: 0x39fa8, entry: 0x39f82 };

/// The ten classes, in the order of the original loops (index of [`Step::Dispatch`]).
pub const CLASSES: [&Class; 10] = [&PSHL, &PEFF, &ENEMY, &BOSS, &ESHL, &EFFCT, &BOX, &SET, &ITEM, &SOBJ];

impl Class {
    /// The handler of type `kind`. The original indexes its table by `(kind << 2) as u16` without a bound: a type
    /// beyond the table would read the code that follows it as an address; nothing creates such a type
    /// (`docs/code-mort.md`, batch L13).
    pub fn handler(&self, kind: u8) -> Handler {
        match self.handlers.get(kind as usize) {
            Some(&handler) => handler,
            None => unreachable!("dispatch {:x}: type {kind:#x} beyond the table {:x}", self.entry, self.table),
        }
    }

    /// Calls the handler of each active object, in slot order. The state of a
    /// slot is read when the loop reaches it: an object created by a handler in
    /// a later slot is processed in the same frame.
    ///
    /// The handlers are functions without registers (`Body::Handler`): they receive the object (A0 of the original),
    /// the loop counter (D7.w) and the leftovers that they pass to each other (`Leftovers`, a game state kept in
    /// the task table). What the original loop puts in D0 (type × 4) and A1 (the handler) is reread by none of them.
    ///
    /// `Flow::Yield`: a handler ceded control (the task will resume at [`Step::Dispatch`], then at the continuation
    /// that the caller stored in its pending calls before the call), or returned higher up.
    pub fn update(&self, b: &mut World) -> Flow {
        self.sweep(b, self.pool.slot(0), (self.pool.slots - 1) as u16, false, &mut None)
    }

    /// Walk called without continuation (phase 0, screens outside the game): no handler cedes control there.
    #[track_caller]
    pub fn update_no_next(&self, b: &mut World) {
        self.update_last_kind(b);
    }

    /// [`update_no_next`](Self::update_no_next), which returns the type of the last object whose handler was called:
    /// the original loop leaves type × 4 in D0 (`lsl.w #2`), which a caller rereads (original glitch: the end credits,
    /// `modes::ending`).
    #[track_caller]
    pub fn update_last_kind(&self, b: &mut World) -> Option<u8> {
        let mut last = None;
        let flow = self.sweep(b, self.pool.slot(0), (self.pool.slots - 1) as u16, false, &mut last);
        assert!(flow == Flow::Done, "dispatch {:x}: a handler ceded control without continuation", self.entry);
        last
    }

    /// Walk of the slots from `object`, `counter` slots left after it (D7.w of the original). `after_current`: resumes
    /// just after the `jsr (A1)` of the handler of the current slot, which must not be called again.
    fn sweep(&self, b: &mut World, mut object: Obj, mut counter: u16, mut after_current: bool, last_kind: &mut Option<u8>) -> Flow {
        loop {
            if !after_current && object.is_active(b) {
                let kind = object.u8(b, Obj::KIND);
                *last_kind = Some(kind);
                let next = Step::Dispatch { class: self.index, object, counter };
                let call = crate::core::tasks::table::call_expected(b, next);
                run_handler(b, self.handler(kind), object, counter);
                if !crate::core::tasks::table::returned(b, call) {
                    return Flow::Yield;
                }
            }
            after_current = false;
            object = Obj(object.0.wrapping_add(self.pool.stride));
            counter = counter.wrapping_sub(1);
            if counter == 0xffff {
                return Flow::Done;
            }
        }
    }
}

/// Calls `handler` for `object`, then returns to the loop (its `rts`: the expected call of the loop is closed, unless
/// the handler ceded control).
fn run_handler(b: &mut World, handler: Handler, object: Obj, counter: u16) {
    let (run, native) = match handler {
        Handler::Rust { run, native } => (run, native),
        Handler::Dead { entry } => unreachable!("handler {entry:06x}: dead type of the original table"),
    };
    crate::core::frame::note(b, native);
    let mut a = Dispatch { object, counter, leftovers: b.leftovers };
    #[cfg(feature = "tools")]
    crate::tools::check_handler(native, run, b, &mut a);
    #[cfg(not(feature = "tools"))]
    run(b, &mut a);
    b.leftovers = a.leftovers;
    crate::core::tasks::return_to_caller(b);
}

/// What the dispatch loop passes to a converted handler (`Body::Handler`), instead of the registers of the
/// original.
#[derive(Clone, Copy, Debug)]
pub struct Dispatch {
    /// The object being processed (A0).
    pub object: Obj,
    /// Loop counter (D7.w): phase of the "one frame out of two" blinking.
    pub counter: u16,
    /// Leftover registers that the handlers pass to each other.
    pub leftovers: Leftovers,
}

pub use crate::world::Leftovers;

/// Resumption of a dispatch loop in which a handler made the task cede control.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// Return of the handler of the current slot of class `CLASSES[class]`: the walk continues at
    /// the next slot, then returns to the loop's caller. The original keeps the object (A0) and the counter
    /// (D7.w) in its registers, which the cession saves and restores; here they are in the step, noted at the call of
    /// the handler (no handler that cedes control changes them).
    Dispatch { class: u8, object: Obj, counter: u16 },
}

impl Step {
    /// Address that the original resumes at: the one following the loop's `jsr (A1)`.
    pub const fn address(self) -> u32 {
        let Step::Dispatch { class, .. } = self;
        CLASSES[class as usize].entry + HANDLER_RETURN
    }

    pub fn step(self, x: &mut crate::screens::scene::Ctx) -> Flow {
        let Step::Dispatch { class, object, counter } = self;
        CLASSES[class as usize].sweep(x.b, object, counter, true, &mut None)
    }
}

/// The object that the current dispatch loop of the current task is processing (the most recent of its pending
/// calls [`Step::Dispatch`]): that of a handler that ceded control and resumes (fireworks).
pub fn current_object(b: &mut World) -> Option<Obj> {
    let frames = crate::core::tasks::table::table(b).frames();
    frames.iter().filter_map(|frame| match frame.next {
        crate::core::tasks::table::Next::Step(crate::core::tasks::Step::Dispatch(Step::Dispatch { object, .. })) => Some(object),
        _ => None,
    }).last()
}

/// The pending dispatch loop call of the current task (the most recent [`Step::Dispatch`]): the object being processed
/// and the loop counter.
fn current_call(b: &mut World) -> Option<(Obj, u16)> {
    let frames = crate::core::tasks::table::table(b).frames();
    frames.iter().filter_map(|frame| match frame.next {
        crate::core::tasks::table::Next::Step(crate::core::tasks::Step::Dispatch(Step::Dispatch { object, counter, .. })) => {
            Some((object, counter))
        }
        _ => None,
    }).last()
}

/// A handler that ceded control resumes (fireworks, SET doors: steps of the task): `f` runs on what the dispatch loop
/// passes, the object being `object` (or that of the pending loop), the counter that of the pending loop, the
/// leftovers those of the game (the original keeps them in the registers, which the cession saves and restores).
pub fn resume_handler(b: &mut World, object: Option<Obj>, f: impl FnOnce(&mut World, &mut Dispatch)) {
    let (current, counter) = current_call(b).expect("a handler resumes outside a dispatch loop");
    let leftovers = b.leftovers;
    let mut a = Dispatch { object: object.unwrap_or(current), counter, leftovers };
    f(b, &mut a);
    b.leftovers = a.leftovers;
}

/// Offset of the return address of the `jsr (A1)` that calls the handler, in each of the ten
/// original loops (`entry + 0x1a`).
const HANDLER_RETURN: u32 = 0x1c;

// The handlers of the ten original tables, by type (comment: the type). Some types share a handler.

const PSHL_HANDLERS: [Handler; 25] = [
    h(&f::player::pshl::PSHL_0), // 0
    h(&f::player::pshl::PSHL_1), // 1
    h(&f::player::pshl::PSHL_2), // 2
    h(&f::player::pshl::PSHL_3), // 3
    h(&f::player::pshl::PSHL_4), // 4
    h(&f::player::pshl::PSHL_5), // 5
    h(&f::player::pshl_e::PSHL_6), // 6
    h(&f::player::pshl_b::PSHL_7), // 7
    h(&f::player::pshl_b::PSHL_8), // 8
    h(&f::player::pshl::PSHL_1), // 9
    h(&f::player::pshl::PSHL_2), // 10
    h(&f::player::pshl::PSHL_2), // 11
    h(&f::player::pshl_c::PSHL_12), // 12
    h(&f::player::pshl_c::PSHL_13), // 13
    h(&f::player::pshl_c::PSHL_14), // 14
    h(&f::player::pshl_c::PSHL_15), // 15
    h(&f::player::pshl_c::PSHL_16), // 16
    h(&f::player::pshl_c::PSHL_17), // 17
    h(&f::player::pshl_e::PSHL_18), // 18
    h(&f::player::pshl_d::PSHL_19), // 19
    h(&f::player::pshl_d::PSHL_20), // 20
    h(&f::player::pshl_d::PSHL_21), // 21
    h(&f::player::pshl_d::PSHL_22), // 22
    h(&f::player::pshl_d::PSHL_23), // 23
    h(&f::player::pshl_d::PSHL_24), // 24
];
const PEFF_HANDLERS: [Handler; 48] = [
    h(&f::player::peff_ring::PEFF_0), // 0
    h(&f::player::peff_ring::PEFF_1), // 1
    h(&f::player::peff_c::PEFF_2), // 2
    h(&f::player::peff_c::PEFF_3), // 3
    h(&f::player::peff_c::PEFF_4), // 4
    h(&f::player::peff_c::PEFF_5), // 5
    h(&f::player::peff_d::PEFF_6), // 6
    h(&f::player::peff_d::PEFF_7), // 7
    h(&f::player::peff_d::PEFF_8), // 8
    h(&f::player::peff_d::PEFF_9), // 9
    h(&f::player::peff_d::PEFF_10), // 10
    h(&f::player::peff_b::PEFF_11), // 11
    h(&f::player::peff_e::PEFF_12), // 12
    h(&f::player::peff_e::PEFF_13), // 13
    h(&f::player::peff_e::PEFF_14), // 14
    h(&f::player::peff_f::PEFF_15), // 15
    h(&f::player::peff_f::PEFF_16), // 16
    h(&f::player::peff_l::PEFF_17), // 17
    h(&f::player::peff_f::PEFF_18), // 18
    h(&f::player::peff_f::PEFF_19), // 19
    h(&f::player::peff_b::PEFF_20), // 20
    h(&f::player::peff_g::PEFF_21), // 21
    h(&f::player::peff_g::PEFF_22), // 22
    h(&f::player::peff_g::PEFF_23), // 23
    h(&f::player::peff_o::PEFF_24), // 24
    h(&f::player::peff_h::PEFF_25), // 25
    h(&f::player::peff_h::PEFF_26), // 26
    h(&f::player::peff_h::PEFF_27), // 27
    h(&f::player::peff_h::PEFF_28), // 28
    h(&f::player::peff_h::PEFF_29), // 29
    h(&f::player::peff_i::PEFF_30), // 30
    h(&f::player::peff_i::PEFF_31), // 31
    h(&f::player::peff_i::PEFF_32), // 32
    h(&f::player::peff_b::PEFF_33), // 33
    h(&f::player::peff_m::PEFF_34), // 34
    h(&f::player::peff_j::PEFF_35), // 35
    h(&f::player::peff_j::PEFF_36), // 36
    h(&f::player::peff_j::PEFF_37), // 37
    h(&f::player::peff_j::PEFF_38), // 38
    h(&f::player::peff_j::PEFF_39), // 39
    h(&f::player::peff_k::PEFF_40), // 40
    h(&f::player::peff_n::PEFF_41), // 41
    h(&f::player::peff_shop::PEFF_42), // 42
    h(&f::player::peff::PEFF_43), // 43
    h(&f::player::peff::PEFF_44), // 44
    h(&f::player::peff_k::PEFF_45), // 45
    h(&f::player::peff_j::PEFF_46), // 46
    h(&f::player::peff_j::PEFF_47), // 47
];
const ENEMY_HANDLERS: [Handler; 29] = [
    h(&f::enemy_kobold::ENEMY_0), // 0
    h(&f::enemy_skeleton::ENEMY_1), // 1
    h(&f::enemy_trog::ENEMY_2), // 2
    h(&f::enemy_bow::ENEMY_3), // 3
    h(&f::enemy_owlbear::ENEMY_4), // 4
    h(&f::enemy_ghoul::ENEMY_5), // 5
    h(&f::enemy_hellhound::ENEMY_6), // 6
    h(&f::enemy_part::ENEMY_7), // 7
    h(&f::enemy_gargoyle::ENEMY_8), // 8
    h(&f::enemy_beetle::ENEMY_9), // 9
    h(&f::enemy_scorpion::ENEMY_10), // 10
    h(&f::enemy_golem::ENEMY_11), // 11
    h(&f::enemy_elf::ENEMY_12), // 12
    h(&f::enemy_gnoll::ENEMY_13), // 13
    h(&f::enemy_troll::ENEMY_14), // 14
    h(&f::enemy_goblin::ENEMY_15), // 15
    h(&f::enemy_riyakaa::ENEMY_16), // 16
    Handler::Dead { entry: 0x39aec }, // 17: empty slot of the table: points at the BOSS loop (0x39aec), never created
    h(&f::enemy_ogre::ENEMY_18), // 18
    h(&f::enemy_dbeast::ENEMY_19), // 19
    h(&f::enemy::ENEMY_20), // 20
    h(&f::enemy_harpy::ENEMY_21), // 21
    h(&f::enemy_double::ENEMY_22), // 22
    h(&f::enemy_double::ENEMY_23), // 23
    h(&f::enemy_double::ENEMY_24), // 24
    h(&f::enemy_warrior::ENEMY_25), // 25
    h(&f::enemy_double::ENEMY_26), // 26
    h(&f::enemy_warmachine::ENEMY_27), // 27
    h(&f::enemy::ENEMY_28), // 28
];
const BOSS_HANDLERS: [Handler; 23] = [
    h(&f::boss_0::BOSS_0), // 0
    h(&f::boss_1::BOSS_1), // 1
    h(&f::boss_22::BOSS_2), // 2
    h(&f::boss_3::BOSS_3), // 3
    h(&f::boss_4::BOSS_4), // 4
    h(&f::boss_5::BOSS_5), // 5
    h(&f::boss_6::BOSS_6), // 6
    h(&f::boss_7::BOSS_7), // 7
    h(&f::boss_8::BOSS_8), // 8
    h(&f::boss_9::BOSS_9), // 9
    h(&f::boss_10::BOSS_10), // 10
    h(&f::boss_11::BOSS_11), // 11
    h(&f::boss_11::BOSS_11), // 12
    h(&f::boss_dragon::BOSS_13), // 13
    h(&f::boss_19::BOSS_19), // 14
    h(&f::boss_15::BOSS_15), // 15
    h(&f::boss_16::BOSS_16), // 16
    h(&f::boss_17::BOSS_17), // 17
    h(&f::boss_18::BOSS_18), // 18
    h(&f::boss_19::BOSS_19), // 19
    h(&f::boss_15::BOSS_15), // 20
    h(&f::boss_21::BOSS_21), // 21
    h(&f::boss_22::BOSS_22), // 22
];
const ESHL_HANDLERS: [Handler; 73] = [
    h(&f::eshl::ESHL_00), // 0
    h(&f::eshl::ESHL_01), // 1
    h(&f::eshl::ESHL_02), // 2
    h(&f::eshl::ESHL_03), // 3
    h(&f::eshl::ESHL_04), // 4
    h(&f::eshl::ESHL_05), // 5
    h(&f::eshl::ESHL_06), // 6
    h(&f::eshl::ESHL_07), // 7
    h(&f::eshl::ESHL_08), // 8
    h(&f::eshl::ESHL_09), // 9
    h(&f::eshl::ESHL_10), // 10
    h(&f::eshl::ESHL_11), // 11
    h(&f::eshl::ESHL_12), // 12
    h(&f::eshl::ESHL_13), // 13
    h(&f::eshl::ESHL_14), // 14
    h(&f::eshl_b::ESHL_15), // 15
    h(&f::eshl_b::ESHL_16), // 16
    h(&f::eshl_b::ESHL_17), // 17
    h(&f::eshl_b::ESHL_18), // 18
    h(&f::eshl_b::ESHL_19), // 19
    h(&f::eshl_b::ESHL_20), // 20
    h(&f::eshl_b::ESHL_21), // 21
    h(&f::eshl_b::ESHL_22), // 22
    h(&f::eshl_b::ESHL_23), // 23
    h(&f::eshl_b::ESHL_24), // 24
    h(&f::eshl_b::ESHL_25), // 25
    h(&f::eshl_b::ESHL_26), // 26
    h(&f::eshl_b::ESHL_27), // 27
    h(&f::eshl_b::ESHL_28), // 28
    h(&f::eshl_b::ESHL_29), // 29
    h(&f::eshl_b::ESHL_30), // 30
    h(&f::eshl_b::ESHL_31), // 31
    h(&f::eshl_b::ESHL_32), // 32
    h(&f::eshl_b::ESHL_33), // 33
    h(&f::eshl_b::ESHL_34), // 34
    h(&f::eshl_b::ESHL_35), // 35
    h(&f::eshl_b::ESHL_36), // 36
    h(&f::eshl_b::ESHL_37), // 37
    h(&f::eshl_b::ESHL_38), // 38
    h(&f::eshl_b::ESHL_39), // 39
    h(&f::eshl_c::ESHL_40), // 40
    h(&f::eshl_c::ESHL_41), // 41
    h(&f::eshl_c::ESHL_42), // 42
    h(&f::eshl_c::ESHL_43), // 43
    h(&f::eshl_c::ESHL_44), // 44
    h(&f::eshl_c::ESHL_45), // 45
    h(&f::eshl_c::ESHL_46), // 46
    h(&f::eshl_c::ESHL_47), // 47
    h(&f::eshl_c::ESHL_48), // 48
    h(&f::eshl_c::ESHL_49), // 49
    h(&f::eshl_c::ESHL_50), // 50
    h(&f::eshl_c::ESHL_51), // 51
    h(&f::eshl_c::ESHL_52), // 52
    h(&f::eshl_c::ESHL_53), // 53
    h(&f::eshl_c::ESHL_54), // 54
    h(&f::eshl_c::ESHL_55), // 55
    h(&f::eshl_c::ESHL_56), // 56
    h(&f::eshl_c::ESHL_57), // 57
    h(&f::eshl_c::ESHL_58), // 58
    h(&f::eshl_c::ESHL_59), // 59
    h(&f::eshl_c::ESHL_60), // 60
    h(&f::eshl_c::ESHL_61), // 61
    h(&f::eshl_c::ESHL_62), // 62
    h(&f::eshl_c::ESHL_63), // 63
    h(&f::eshl_d::ESHL_64), // 64
    h(&f::eshl_d::ESHL_65), // 65
    h(&f::eshl_d::ESHL_66), // 66
    h(&f::eshl_d::ESHL_67), // 67
    h(&f::eshl_d::ESHL_68), // 68
    h(&f::eshl_d::ESHL_69), // 69
    h(&f::eshl_d::ESHL_70), // 70
    h(&f::eshl_d::ESHL_71), // 71
    h(&f::eshl_d::ESHL_72), // 72
];
const EFFCT_HANDLERS: [Handler; 92] = [
    h(&f::effects::f::EFFCT_OPENING), // 0
    h(&f::effects::EFFCT_1), // 1
    Handler::Dead { entry: 0xaa71a }, // 2: dead type (docs/familles.md): its only creator, 0x17626, is unreachable
    h(&f::effects::c::EFFCT_FIXED_SCREEN), // 3
    h(&f::effects::EFFCT_4), // 4
    h(&f::effects::EFFCT_5), // 5
    h(&f::effects::EFFCT_6), // 6
    h(&f::effects::EFFCT_7), // 7
    h(&f::effects::EFFCT_8), // 8
    h(&f::effects::EFFCT_9), // 9
    h(&f::effects::c::EFFCT_DALLE), // 10
    h(&f::effects::e::EFFCT_ATTACK), // 11
    h(&f::effects::c::EFFCT_MAILLON), // 12
    h(&f::effects::e::EFFCT_RADEAU), // 13
    h(&f::effects::e::EFFCT_PASSAGE), // 14
    h(&f::effects::EFFCT_15), // 15
    h(&f::effects::c::EFFCT_DOUBLE), // 16
    h(&f::effects::EFFCT_17), // 17
    h(&f::set::SET_COMMON_CC25A), // 18
    h(&f::effects::EFFCT_19), // 19
    h(&f::effects::c::EFFCT_BOURSE), // 20
    h(&f::effects::EFFCT_21), // 21
    h(&f::effects::c::EFFCT_SHOP), // 22
    h(&f::effects::c::EFFCT_VOTE), // 23
    h(&f::effects::EFFCT_24), // 24
    h(&f::effects::EFFCT_25), // 25
    h(&f::effects::EFFCT_26), // 26
    h(&f::effects::EFFCT_27), // 27
    h(&f::effects::EFFCT_28), // 28
    h(&f::effects::EFFCT_29), // 29
    h(&f::effects::c::EFFCT_POISSON), // 30
    h(&f::effects::d::EFFCT_COUNTDOWN), // 31
    h(&f::effects::EFFCT_32), // 32
    h(&f::effects::EFFCT_33), // 33
    h(&f::effects::c::EFFCT_PLATEAU), // 34
    h(&f::effects::f::EFFCT_SILHOUETTE_REFLECTION), // 35
    h(&f::effects::c::EFFCT_FLOTTEUR), // 36
    h(&f::effects::c::EFFCT_TITLE), // 37
    h(&f::effects::c::EFFCT_RESULT), // 38
    h(&f::effects::c::EFFCT_PROJETE), // 39
    h(&f::effects::c::EFFCT_HIT), // 40
    h(&f::effects::c::EFFCT_THROWN), // 41
    h(&f::effects::c::EFFCT_SCENERY), // 42
    h(&f::effects::g::EFFCT_RAIN_DEBRIS), // 43
    h(&f::effects::c::EFFCT_SPELL), // 44
    h(&f::effects::EFFCT_45), // 45
    h(&f::effects::g::EFFCT_ARMURE), // 46
    h(&f::effects::c::EFFCT_MARKER), // 47
    h(&f::effects::g::EFFCT_MACHINE_VOLANTE), // 48
    h(&f::effects::c::EFFCT_ACCESSORY_CARRIED), // 49
    h(&f::effects::c::EFFCT_ATTACHE), // 50
    h(&f::effects::EFFCT_51), // 51
    h(&f::effects::EFFCT_52), // 52
    h(&f::effects::f::EFFCT_TRAVERSEE), // 53
    h(&f::effects::f::EFFCT_PASSAGE), // 54
    h(&f::effects::f::EFFCT_LAUNCHER), // 55
    h(&f::effects::EFFCT_56), // 56
    h(&f::effects::EFFCT_57), // 57
    h(&f::effects::c::EFFCT_MACHINE), // 58
    h(&f::effects::c::EFFCT_SURGISSANT), // 59
    h(&f::effects::c::EFFCT_DOUBLE_B), // 60
    h(&f::effects::c::EFFCT_NAGEUR), // 61
    h(&f::effects::EFFCT_62), // 62
    h(&f::effects::g::EFFCT_ENEMY_PIECE), // 63
    h(&f::effects::c::EFFCT_SHARD_FALLING), // 64
    h(&f::effects::c::EFFCT_MESSAGER), // 65
    h(&f::effects::c::EFFCT_LETTERS), // 66
    h(&f::effects::c::EFFCT_COLUMN), // 67
    h(&f::effects::f::EFFCT_SIGN_START), // 68
    h(&f::effects::c::EFFCT_SILHOUETTE), // 69
    h(&f::effects::c::EFFCT_COMPTOIR), // 70
    Handler::Dead { entry: 0xb841a }, // 71: dead type (docs/familles.md): an `rts`, never created
    h(&f::effects::b::EFFCT_DETECTEUR), // 72
    h(&f::effects::b::EFFCT_VICTORY), // 73
    h(&f::effects::b::EFFCT_BUBBLE_CREATOR), // 74
    h(&f::effects::b::EFFCT_BUTTON), // 75
    h(&f::effects::c::EFFCT_MINUTEUR), // 76
    h(&f::effects::f::EFFCT_SOUND_FADE), // 77
    Handler::Dead { entry: 0xb8a1e }, // 78: dead type (docs/familles.md): never created
    h(&f::effects::b::EFFCT_BLINK), // 79
    h(&f::effects::g::EFFCT_FIREWORK), // 80
    h(&f::effects::g::EFFCT_SHARD_THROWN), // 81
    h(&f::effects::c::EFFCT_SHARD_PLACED), // 82
    h(&f::effects::c::EFFCT_RAIN), // 83
    h(&f::effects::c::EFFCT_FINALE), // 84
    h(&f::effects::c::EFFCT_END_PIECE), // 85
    h(&f::effects::c::EFFCT_EMBER), // 86
    h(&f::effects::c::EFFCT_INTERRUPTEUR), // 87
    h(&f::effects::c::EFFCT_BARRIER), // 88
    h(&f::effects::f::EFFCT_PASSANT), // 89
    h(&f::effects::c::EFFCT_DUST_HIGH), // 90
    h(&f::effects::c::EFFCT_FIXED_SCREEN_B), // 91
];
const BOX_HANDLERS: [Handler; 4] = [
    h(&f::boxes::chest::BOX_CRATE), // 0
    h(&f::boxes::BOX_1), // 1
    h(&f::boxes::BOX_2), // 2
    h(&f::boxes::pushes::BOX_PUSHES), // 3
];
const SET_HANDLERS: [Handler; 34] = [
    h(&f::set::SET_0), // 0
    h(&f::set::SET_1), // 1
    h(&f::set_c::SET_SCENE_BC814), // 2
    h(&f::set_c::SET_WINDOW_C8352), // 3
    h(&f::set::SET_4), // 4
    h(&f::set::SET_5), // 5
    h(&f::set::SET_6), // 6
    h(&f::set::SET_7), // 7
    h(&f::set::SET_COMMON_CC25A), // 8
    h(&f::set::SET_9), // 9
    h(&f::set::SET_10), // 10
    h(&f::set::SET_11), // 11
    h(&f::set::SET_12), // 12
    h(&f::set::SET_13), // 13
    h(&f::set::SET_14), // 14
    h(&f::set_c::SET_SCREENS_CE024), // 15
    h(&f::set_b::SET_16), // 16
    h(&f::set_b::SET_17), // 17
    h(&f::set_b::SET_18), // 18
    h(&f::set_b::SET_19), // 19
    h(&f::set_b::SET_20), // 20
    h(&f::set_b::SET_21), // 21
    h(&f::set_b::SET_22), // 22
    h(&f::set_b::SET_23), // 23
    h(&f::set_b::SET_24), // 24
    h(&f::set_b::SET_25), // 25
    h(&f::set_b::SET_26), // 26
    h(&f::set_b::SET_27), // 27
    h(&f::set_b::SET_28), // 28
    h(&f::set_b::SET_29), // 29
    h(&f::set_b::SET_30), // 30
    h(&f::set_b::SET_31), // 31
    h(&f::set_b::SET_32), // 32
    h(&f::set_c::SET_WINDOW_C8352), // 33
];
const ITEM_HANDLERS: [Handler; 10] = [
    h(&f::items::ITEM_0), // 0
    h(&f::items::ITEM_1), // 1
    h(&f::items_b::ITEM_PIECE), // 2
    h(&f::items::ITEM_3), // 3
    h(&f::items::ITEM_4), // 4
    h(&f::items::ITEM_5), // 5
    h(&f::items::ITEM_6), // 6
    h(&f::items::ITEM_7), // 7
    h(&f::items::ITEM_8), // 8
    h(&f::items::ITEM_9), // 9
];
const SOBJ_HANDLERS: [Handler; 16] = [
    h(&f::sobj::SOBJ_00), // 0
    h(&f::sobj::SOBJ_01), // 1
    h(&f::sobj::SOBJ_02), // 2
    h(&f::sobj::SOBJ_03), // 3
    h(&f::set::SET_COMMON_CC25A), // 4
    h(&f::sobj::SOBJ_05), // 5
    h(&f::sobj::SOBJ_06), // 6
    h(&f::sobj::SOBJ_07), // 7
    h(&f::sobj::SOBJ_08), // 8
    h(&f::sobj::SOBJ_09), // 9
    h(&f::sobj::SOBJ_10), // 10
    h(&f::sobj::SOBJ_11), // 11
    h(&f::sobj::SOBJ_12), // 12
    h(&f::sobj::SOBJ_13), // 13
    h(&f::sobj::SOBJ_14), // 14
    h(&f::sobj::SOBJ_15), // 15
];
