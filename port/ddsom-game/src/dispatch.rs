//! Object dispatch (W0): the ten loops `0x398e2-0x39f82` that walk a pool and call,
//! for each active object, the handler of its type.
//!
//! Each loop of the original is the same: a slot whose `Obj::ACTIVE` byte is zero is
//! skipped, otherwise the handler of `Obj::KIND` is called, read from a table of longs stored in the
//! code. Here, a `Class` describes the pool and its table; `update` is the single loop. The handlers are
//! natives, designated by their address in the ROM table.
//!
//! A handler can make the task cede control (fade to black of the doors, fireworks): the loop is then
//! abandoned, and the task resumes it at the step [`Step::Dispatch`] (after the current slot, with the object and
//! the counter noted in the step), then at the continuation of the loop's caller.

use cps2::Cps2Bus;
use m68k::Cpu;

use crate::mem::rts;
use crate::modes::Flow;
use crate::objects::pools::{self, Pool};
use crate::objects::Obj;
use crate::Native;

/// A class of objects: its pool and its handler table.
pub struct Class {
    pub pool: &'static Pool,
    /// Handler table (longs, one per type), in the code.
    pub table: u32,
    /// Address of the original's dispatch loop.
    pub entry: u32,
}

pub const PSHL: Class = Class { pool: &pools::PSHL, table: 0x39908, entry: 0x398e2 };
pub const PEFF: Class = Class { pool: &pools::PEFF, table: 0x39992, entry: 0x3996c };
pub const ENEMY: Class = Class { pool: &pools::ENEMY, table: 0x39a78, entry: 0x39a52 };
pub const BOSS: Class = Class { pool: &pools::BOSS, table: 0x39b12, entry: 0x39aec };
pub const ESHL: Class = Class { pool: &pools::ESHL, table: 0x39b96, entry: 0x39b70 };
pub const EFFCT: Class = Class { pool: &pools::EFFCT, table: 0x39ce0, entry: 0x39cba };
pub const BOX: Class = Class { pool: &pools::BOX, table: 0x39e76, entry: 0x39e50 };
pub const SET: Class = Class { pool: &pools::SET, table: 0x39eac, entry: 0x39e86 };
pub const ITEM: Class = Class { pool: &pools::ITEM, table: 0x39f5a, entry: 0x39f34 };
pub const SOBJ: Class = Class { pool: &pools::SOBJ, table: 0x39fa8, entry: 0x39f82 };

/// The ten classes, in the order of the original loops (index of [`Step::Dispatch`]).
const CLASSES: [&Class; 10] = [&PSHL, &PEFF, &ENEMY, &BOSS, &ESHL, &EFFCT, &BOX, &SET, &ITEM, &SOBJ];

impl Class {
    /// Address of the handler of type `kind` (the original indexes the table by `(kind << 2) as u16`).
    pub fn handler(&self, b: &mut Cps2Bus, kind: u8) -> u32 {
        crate::mem::prog32(b, self.table.wrapping_add(((kind as u16) << 2) as u32))
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
    pub fn update(&self, b: &mut Cps2Bus) -> Flow {
        self.sweep(b, self.pool.slot(0), (self.pool.slots - 1) as u16, false, &mut None)
    }

    /// Walk called without continuation (phase 0, screens outside the game): no handler cedes control there.
    #[track_caller]
    pub fn update_no_next(&self, b: &mut Cps2Bus) {
        self.update_last_kind(b);
    }

    /// [`update_no_next`](Self::update_no_next), which returns the type of the last object whose handler was called:
    /// the original loop leaves type × 4 in D0 (`lsl.w #2`), which a caller rereads (original glitch: the end credits,
    /// `modes::ending`).
    #[track_caller]
    pub fn update_last_kind(&self, b: &mut Cps2Bus) -> Option<u8> {
        let mut last = None;
        let flow = self.sweep(b, self.pool.slot(0), (self.pool.slots - 1) as u16, false, &mut last);
        assert!(flow == Flow::Done, "dispatch {:x}: a handler ceded control without continuation", self.entry);
        last
    }

    /// Walk of the slots from `object`, `counter` slots left after it (D7.w of the original). `after_current`: resumes
    /// just after the `jsr (A1)` of the handler of the current slot, which must not be called again.
    fn sweep(&self, b: &mut Cps2Bus, mut object: Obj, mut counter: u16, mut after_current: bool, last_kind: &mut Option<u8>) -> Flow {
        let index = CLASSES.iter().position(|k| k.entry == self.entry).expect("object class") as u8;
        loop {
            if !after_current && object.is_active(b) {
                let kind = object.u8(b, Obj::KIND);
                *last_kind = Some(kind);
                let address = self.handler(b, kind);
                let next = Step::Dispatch { class: index, object, counter };
                let call = crate::tasks::table::call_expected(b, next);
                run_handler(b, address, object, counter);
                if !crate::tasks::table::returned(b, call) {
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

/// Calls the handler at `address` (an entry of a ROM table) for `object`, then returns to the loop (its `rts`: the
/// expected call of the loop is closed, unless the handler ceded control).
fn run_handler(b: &mut Cps2Bus, address: u32, object: Obj, counter: u16) {
    let native = if crate::no68k::active() {
        crate::no68k::native(b, address)
    } else {
        crate::lookup(address).unwrap_or_else(|| panic!("handler {address:06x}: no native"))
    };
    let crate::Body::Handler(handler) = native.run else {
        panic!("handler {address:06x} ({}): not a handler without registers", native.name)
    };
    let mut a = Dispatch { object, counter, leftovers: crate::tasks::table::table(b).leftovers };
    handler(b, &mut a);
    crate::tasks::table::table(b).leftovers = a.leftovers;
    crate::tasks::return_to_caller(b);
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

/// Leftover registers that a handler leaves and that a following handler rereads (original glitches). The probe
/// `DDSOM_POISON=exit` (registers changed by each handler, inverted on its return) pointed to them in all
/// scenarios; the other registers that a handler leaves are reread by nobody. Other routines also write them (sprite
/// list, cameras, input copy, gauge: `sprites::build_in_game`...).
///
/// They are a game state, stored in the task table (`tasks::table::Table::leftovers`); the code still on the
/// processor finds them in its register bench (`bench::on_bench`, [`Leftovers::to_bench`] and
/// [`Leftovers::from_bench`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Leftovers {
    /// D2: what is left by the collision with the terrain (0x33fe4, `terrain::push`), the box tests (0xb5e0,
    /// 0x7f19c, 0x7ef4), the loot treasure line (0xb780); the spark of `flash_and_shake` (0x103ba) takes
    /// its speed from it, the bounce of the frost shard (0x4bda8) its setting, the ITEM 5 object (0xdc382) rereads it.
    pub d2: u32,
    /// D3: like D2 (collision, box tests, special loot); acceleration of the spark of `flash_and_shake`.
    pub d3: u32,
    /// D4: the last long copied by the palette animation (SET 0, 0xba310), the raft movement (EFFCT,
    /// 0xad63e), the attributes of the last piece of the sprite list...; the pilotless raft adds it to the
    /// players' position (0xad7b0).
    pub d4: u32,
    /// D5: 0 or 1 (the object moved) after keeping on screen (0xfbae, `ai::keep_on_screen`), 1 after a
    /// collision with the terrain (0x33fe4), the exhausted counter of a `dbf` loop...; the row of shards of a crate
    /// (0xa17d0, `eshl_creators::row_1a`) rereads its high byte.
    pub d5: u32,
}

impl Leftovers {
    /// The code on the processor has finished: its leftovers (D2-D5 of the bench) become those of the game (task
    /// table).
    pub fn from_bench(c: &Cpu, b: &mut Cps2Bus) {
        crate::tasks::table::table(b).leftovers = Leftovers { d2: c.d[2], d3: c.d[3], d4: c.d[4], d5: c.d[5] };
    }

    /// The code on the processor resumes: the game's leftovers return to the bench (D2-D5).
    pub fn to_bench(c: &mut Cpu, b: &mut Cps2Bus) {
        let r = crate::tasks::table::table(b).leftovers;
        (c.d[2], c.d[3], c.d[4], c.d[5]) = (r.d2, r.d3, r.d4, r.d5);
    }
}

impl Dispatch {
    /// The values that the original finds in the registers on entry to the handler.
    pub fn from_cpu(c: &Cpu) -> Self {
        Dispatch { object: Obj(c.a[0]), counter: c.d[7] as u16, leftovers: Leftovers { d2: c.d[2], d3: c.d[3], d4: c.d[4], d5: c.d[5] } }
    }

    /// Returns the leftovers to the processor (the handler leaves the other registers as they are).
    pub fn give_back(&self, c: &mut Cpu) {
        c.d[2] = self.leftovers.d2;
        c.d[3] = self.leftovers.d3;
        c.d[4] = self.leftovers.d4;
        c.d[5] = self.leftovers.d5;
    }
}

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

    pub fn step(self, x: &mut crate::scene::Ctx) -> Flow {
        let Step::Dispatch { class, object, counter } = self;
        CLASSES[class as usize].sweep(x.b, object, counter, true, &mut None)
    }
}

/// The object that the current dispatch loop of the current task is processing (the most recent of its pending
/// calls [`Step::Dispatch`]): that of a handler that ceded control and resumes (fireworks).
pub fn current_object(b: &mut Cps2Bus) -> Option<Obj> {
    let frames = crate::tasks::table::table(b).frames();
    frames.iter().filter_map(|frame| match frame.next {
        crate::tasks::table::Next::Step(crate::tasks::Step::Dispatch(Step::Dispatch { object, .. })) => Some(object),
        _ => None,
    }).last()
}

/// The pending dispatch loop call of the current task (the most recent [`Step::Dispatch`]): the object being processed
/// and the loop counter.
fn current_call(b: &mut Cps2Bus) -> Option<(Obj, u16)> {
    let frames = crate::tasks::table::table(b).frames();
    frames.iter().filter_map(|frame| match frame.next {
        crate::tasks::table::Next::Step(crate::tasks::Step::Dispatch(Step::Dispatch { object, counter, .. })) => {
            Some((object, counter))
        }
        _ => None,
    }).last()
}

/// A handler that ceded control resumes (fireworks, SET doors: steps of the task): `f` runs on what the dispatch loop
/// passes, the object being `object` (or that of the pending loop), the counter that of the pending loop, the
/// leftovers those of the game (the original keeps them in the registers, which the cession saves and restores).
pub fn resume_handler(b: &mut Cps2Bus, object: Option<Obj>, f: impl FnOnce(&mut Cps2Bus, &mut Dispatch)) {
    let (current, counter) = current_call(b).expect("a handler resumes outside a dispatch loop");
    let leftovers = crate::tasks::table::table(b).leftovers;
    let mut a = Dispatch { object: object.unwrap_or(current), counter, leftovers };
    f(b, &mut a);
    crate::tasks::table::table(b).leftovers = a.leftovers;
}

/// Offset of the return address of the `jsr (A1)` that calls the handler, in each of the ten
/// original loops (`entry + 0x1a`).
const HANDLER_RETURN: u32 = 0x1c;

macro_rules! natives {
    ($($ident:ident, $name:literal, $class:expr;)*) => {$(
        pub const $ident: Native = Native {
            entry: $class.entry,
            name: $name,
            run: crate::Body::WithCpu(|c, b| {
                // called by address (tools): a handler never cedes control there; the leftovers pass from the
                // bench to the game and back
                Leftovers::from_bench(c, b);
                let flow = $class.update(b);
                Leftovers::to_bench(c, b);
                if flow == Flow::Done {
                    rts(c, b);
                }
            }),
            touches_sound: false,
            outputs: 0, // the callers chain to other calls
        };
    )*};
}

natives! {
    UPDATE_PSHL, "dispatch_pshl", PSHL;
    UPDATE_PEFF, "dispatch_peff", PEFF;
    UPDATE_ENEMY, "dispatch_enemy", ENEMY;
    UPDATE_BOSS, "dispatch_boss", BOSS;
    UPDATE_ESHL, "dispatch_eshl", ESHL;
    UPDATE_EFFCT, "dispatch_effct", EFFCT;
    UPDATE_BOX, "dispatch_box", BOX;
    UPDATE_SET, "dispatch_set", SET;
    UPDATE_ITEM, "dispatch_item", ITEM;
    UPDATE_SOBJ, "dispatch_sobj", SOBJ;
}

/// The ten dispatch loops, for the calls by address that remain (screens outside the game).
pub const NATIVES: &[Native] = &[
    UPDATE_PSHL, UPDATE_PEFF, UPDATE_ENEMY, UPDATE_BOSS, UPDATE_ESHL, UPDATE_EFFCT, UPDATE_BOX, UPDATE_SET,
    UPDATE_ITEM, UPDATE_SOBJ,
];
