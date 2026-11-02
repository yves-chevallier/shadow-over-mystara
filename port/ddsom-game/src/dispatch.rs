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
use m68k::decode::Size;
use m68k::Cpu;

use crate::mem::rts;
use crate::modes::Flow;
use crate::objects::pools::{self, Pool};
use crate::objects::Obj;
use crate::Native;

/// X flag of the status register.
const SR_X: u16 = 0x10;

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
    /// The registers pass from one handler to the next as in the original loop (A0 =
    /// object, D7.w = counter, D0 = type × 4, A1 = handler): the game depends on it (a handler
    /// reads the D0 left by the previous one) and a handler can modify A0 and D7.
    ///
    /// `Flow::Yield`: a handler ceded control (the task will resume at [`Step::Dispatch`], then at the continuation
    /// that the caller stored in its pending calls before the call), or returned higher up.
    pub fn update(&self, c: &mut Cpu, b: &mut Cps2Bus) -> Flow {
        c.a[0] = self.pool.slot(0).0;
        set_low_word(&mut c.d[7], (self.pool.slots - 1) as u16);
        self.sweep(c, b, false)
    }

    /// Walk called without continuation (phase 0, screens outside the game): no handler cedes control there. Probe
    /// `DDSOM_POISON=handback`: the registers inherited from the caller (all but A5 and A7) are inverted before the
    /// walk, to prove that the handlers do not reread anything from the screen that calls them.
    #[track_caller]
    pub fn update_no_next(&self, c: &mut Cpu, b: &mut Cps2Bus) {
        crate::diag::dead_regs(c, "handback", crate::diag::TASK_REGS);
        assert!(self.update(c, b) == Flow::Done, "dispatch {:x}: a handler ceded control without continuation", self.entry);
    }

    /// Walk of the slots from A0 / D7. `after_current`: resumes just after the
    /// `jsr (A1)` of the handler of the current slot, which must not be called again.
    fn sweep(&self, c: &mut Cpu, b: &mut Cps2Bus, mut after_current: bool) -> Flow {
        let index = CLASSES.iter().position(|k| k.entry == self.entry).expect("object class") as u8;
        loop {
            let obj = Obj(c.a[0]);
            if !after_current && obj.is_active(b) {
                let kind = obj.u8(b, Obj::KIND);
                c.d[0] = ((kind as u16) << 2) as u32;
                // flags of the `lsl.w #2,D0` preceding the jump: N and Z of the result, V, C and X zero
                c.flags_logic(c.d[0], Size::Word);
                c.sr &= !SR_X;
                let address = self.handler(b, kind);
                c.a[1] = address;
                let before = (c.d, c.a, c.sr);
                let next = Step::Dispatch { class: index, object: obj, counter: c.d[7] as u16 };
                let call = crate::tasks::table::call_expected(b, next);
                if !crate::call_expected(c, b, address, call) {
                    return Flow::Yield;
                }
                crate::diag::probe_exit(c, &before, self.entry, address);
            }
            after_current = false;
            c.a[0] = c.a[0].wrapping_add(self.pool.stride);
            let counter = (c.d[7] as u16).wrapping_sub(1);
            set_low_word(&mut c.d[7], counter);
            if counter == 0xffff {
                return Flow::Done;
            }
        }
    }
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
/// scenarios; the other registers that a handler leaves are reread by nobody. They still live in the
/// processor's register bench, which other routines (sprite list, cameras) also write: the loop takes them from there
/// and puts them back.
///
/// They are a game state, stored in the task table (`tasks::table::Table::leftovers`): see
/// [`Leftovers::from_bench`] and [`Leftovers::to_bench`].
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
        // the loop resumes on the processor, with its object and its counter
        x.c.a[0] = object.0;
        set_low_word(&mut x.c.d[7], counter);
        CLASSES[class as usize].sweep(x.c, x.b, true)
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

/// Offset of the return address of the `jsr (A1)` that calls the handler, in each of the ten
/// original loops (`entry + 0x1a`).
const HANDLER_RETURN: u32 = 0x1c;

fn set_low_word(r: &mut u32, v: u16) {
    *r = *r & 0xffff_0000 | v as u32;
}

macro_rules! natives {
    ($($ident:ident, $name:literal, $class:expr;)*) => {$(
        pub const $ident: Native = Native {
            entry: $class.entry,
            name: $name,
            run: crate::Body::Processeur(|c, b| {
                // called by address (screens outside the game): a handler never cedes control there
                if $class.update(c, b) == Flow::Done {
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
