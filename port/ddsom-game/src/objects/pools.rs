//! The 10 object pools: initialization, allocation, release.
//!
//! A pool is an array of fixed-size slots in work RAM. A slot is
//! free when its `Obj::ACTIVE` byte is zero. A free-slot counter, kept separately, allows
//! quickly refusing an allocation; the allocation does not make the object active (the caller
//! writes `ACTIVE`): it only sets its class (`Obj::CLASS`).
//!
//! Four pools (player shots, enemies and bosses, enemy shots, boxes, items) also have a
//! collision candidate queue (`Queue`), emptied at initialization.

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::Cpu;

use super::lists;
use super::Obj;
use crate::mem::{rd16, rts, wr16, wr32, wr8, A5};
use crate::{regs, Native};

/// Queue of collision candidate objects of a category: number of entries, write
/// pointer (16-bit RAM address) and array of object pointers (offsets from A5).
pub struct Queue {
    pub count: u32,
    pub cursor: u32,
    pub array: u32,
}

impl Queue {
    pub fn reset(&self, b: &mut Cps2Bus) {
        wr16(b, A5 + self.cursor, (A5 + self.array) as u16);
        wr16(b, A5 + self.count, 0);
    }
}

pub const QUEUE_PSHL: Queue = Queue { count: 0x7e72, cursor: 0x7e7c, array: 0x7e86 };
pub const QUEUE_ENEMY_BOSS: Queue = Queue { count: 0x7e76, cursor: 0x7e80, array: 0x7ef6 };
pub const QUEUE_ESHL: Queue = Queue { count: 0x7e74, cursor: 0x7e7e, array: 0x7eaa };
pub const QUEUE_BOX: Queue = Queue { count: 0x7e78, cursor: 0x7e82, array: 0x7f28 };
pub const QUEUE_ITEM: Queue = Queue { count: 0x7e7a, cursor: 0x7e84, array: 0x7f52 };

/// An object pool.
pub struct Pool {
    /// Name given by the game (exception screen).
    pub name: &'static str,
    /// Class code written in `Obj::CLASS` at allocation.
    pub class: u8,
    /// First slot (offset from A5), size of a slot, number of slots.
    pub first: u32,
    pub stride: u32,
    pub slots: u32,
    /// Free-slot counter (offset from A5, word).
    pub free: u32,
    /// The pool's objects are in the sorted lists: release removes them from it.
    pub listed: bool,
    pub queue: Option<&'static Queue>,
    /// Allocation and release routines of the original (entry addresses of their natives).
    pub alloc_routine: u32,
    pub free_routine: u32,
}

/// Player shots.
pub const PSHL: Pool = Pool { name: "PSHL", class: 6, first: 0x10de, stride: 0x60, slots: 18, free: 0x7e5e, listed: true, queue: Some(&QUEUE_PSHL), alloc_routine: 0x225e, free_routine: 0x21d8 };
/// Player-related effects.
pub const PEFF: Pool = Pool { name: "PEFF", class: 0x12, first: 0x179e, stride: 0x60, slots: 48, free: 0x7e60, listed: true, queue: None, alloc_routine: 0x228e, free_routine: 0x21e6 };
/// Enemies.
pub const ENEMY: Pool = Pool { name: "ENEMY", class: 2, first: 0x299e, stride: 0xc0, slots: 22, free: 0x7e62, listed: true, queue: Some(&QUEUE_ENEMY_BOSS), alloc_routine: 0x22bc, free_routine: 0x21f4 };
/// Bosses (collision queue shared with the enemies).
pub const BOSS: Pool = Pool { name: "BOSS", class: 4, first: 0x3a1e, stride: 0xc0, slots: 3, free: 0x7e6c, listed: true, queue: Some(&QUEUE_ENEMY_BOSS), alloc_routine: 0x23a8, free_routine: 0x2234 };
/// Enemy shots and attacks.
pub const ESHL: Pool = Pool { name: "ESHL", class: 8, first: 0x3c5e, stride: 0x60, slots: 38, free: 0x7e64, listed: true, queue: Some(&QUEUE_ESHL), alloc_routine: 0x22ea, free_routine: 0x2202 };
/// Scenery and set pieces (outside the sorted lists).
pub const SET: Pool = Pool { name: "SET", class: 0x10, first: 0x4a9e, stride: 0x50, slots: 32, free: 0x7e66, listed: false, queue: None, alloc_routine: 0x2348, free_routine: 0x2210 };
/// Visual effects.
pub const EFFCT: Pool = Pool { name: "EFFCT", class: 0xe, first: 0x549e, stride: 0x60, slots: 38, free: 0x7e68, listed: true, queue: None, alloc_routine: 0x2318, free_routine: 0x2218 };
/// Pickable items.
pub const ITEM: Pool = Pool { name: "ITEM", class: 0xc, first: 0x62de, stride: 0x60, slots: 32, free: 0x7e6e, listed: true, queue: Some(&QUEUE_ITEM), alloc_routine: 0x23d8, free_routine: 0x2242 };
/// Crates and chests.
pub const BOX: Pool = Pool { name: "BOX", class: 0xa, first: 0x6ede, stride: 0x80, slots: 21, free: 0x7e6a, listed: true, queue: Some(&QUEUE_BOX), alloc_routine: 0x2378, free_routine: 0x2226 };
/// System and interface objects.
pub const SOBJ: Pool = Pool { name: "SOBJ", class: 0x14, first: 0x795e, stride: 0x50, slots: 16, free: 0x7e70, listed: true, queue: None, alloc_routine: 0x2408, free_routine: 0x2250 };

/// The pools in order of their initialization (`0x2062`).
pub const POOLS: [&Pool; 10] = [&PSHL, &PEFF, &ENEMY, &BOSS, &ESHL, &SET, &EFFCT, &BOX, &ITEM, &SOBJ];

/// Pool whose allocation routine is `routine`: some ROM tables designate the pool of an object to create
/// by the address of its allocation routine.
pub fn pool_of_alloc_routine(routine: u32) -> &'static Pool {
    POOLS.into_iter().find(|p| p.alloc_routine == routine).unwrap_or_else(|| panic!("{routine:#x}: not an allocation routine"))
}

/// Pool corresponding to a class code (`Obj::CLASS`).
pub fn pool_of_class(class: u8) -> Option<&'static Pool> {
    POOLS.into_iter().find(|p| p.class == class)
}

/// Result of the search for a slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Scan {
    /// Free-slot counter zero: no search.
    NoneLeft,
    /// Free slot with the given index.
    Found(u32),
    /// Non-zero counter but no free slot.
    Exhausted,
}

impl Pool {
    pub fn slot(&self, i: u32) -> Obj {
        Obj(A5 + self.first + i * self.stride)
    }

    /// Address following the last slot.
    pub fn end(&self) -> u32 {
        A5 + self.first + self.slots * self.stride
    }

    pub fn free_count(&self, b: &mut Cps2Bus) -> u16 {
        rd16(b, A5 + self.free)
    }

    /// Slots of the pool, in order.
    pub fn iter(&self) -> impl Iterator<Item = Obj> + '_ {
        (0..self.slots).map(|i| self.slot(i))
    }

    /// Empties all the slots and resets the counters to new.
    pub fn reset(&self, b: &mut Cps2Bus) {
        for i in 0..self.slots {
            self.clear_slot(b, self.slot(i));
        }
        if let Some(q) = self.queue {
            q.reset(b);
        }
        wr16(b, A5 + self.free, self.slots as u16);
    }

    fn clear_slot(&self, b: &mut Cps2Bus, obj: Obj) {
        for off in (0..self.stride).step_by(4) {
            wr32(b, obj.at(off), 0);
        }
    }

    fn scan(&self, b: &mut Cps2Bus) -> Scan {
        if self.free_count(b) == 0 {
            return Scan::NoneLeft;
        }
        match (0..self.slots).find(|&i| !self.slot(i).is_active(b)) {
            Some(i) => Scan::Found(i),
            None => Scan::Exhausted,
        }
    }

    /// Reserves a slot: the counter decreases and the class is written. The object is not
    /// yet active.
    pub fn alloc(&self, b: &mut Cps2Bus) -> Option<Obj> {
        match self.scan(b) {
            Scan::Found(i) => {
                let obj = self.slot(i);
                let left = self.free_count(b);
                wr16(b, A5 + self.free, left.wrapping_sub(1));
                obj.set_u8(b, Obj::CLASS, self.class);
                Some(obj)
            }
            _ => None,
        }
    }

    /// Releases an object: removed from the sorted lists, counter incremented, slot reset to zero.
    pub fn release(&self, b: &mut Cps2Bus, obj: Obj) {
        if self.listed {
            lists::unlink(b, obj);
        }
        let left = self.free_count(b);
        wr16(b, A5 + self.free, left.wrapping_add(1));
        self.clear_slot(b, obj);
    }

}

// ---- adaptation to the 68000 registers ----

/// Masks of the registers that the original leaves (see `diag::dead_regs`).
pub const REGS_ALLOC: u32 = regs::a(1) | regs::d(6) | regs::CCR;
pub const REGS_FREE: u32 = regs::d(0) | regs::d(1) | regs::a(4) | regs::CCR;

impl Pool {
    /// Like `alloc`, and leaves the original's registers: A1 = slot found (on failure: the first
    /// slot if the counter is zero, the end of the pool if no slot is free), D6 = loop counter,
    /// carry = failure. For the callers not yet freed from registers (`docs/guide-refactorisation.md`).
    #[track_caller]
    pub fn alloc_68k(&self, c: &mut Cpu, b: &mut Cps2Bus) -> Option<Obj> {
        let (r, _) = self.alloc_regs(c, b);
        crate::diag::dead_regs(c, "alloc", REGS_ALLOC);
        r
    }

    /// Allocation of the original, registers included: also returns the A1 left behind.
    fn alloc_regs(&self, c: &mut Cpu, b: &mut Cps2Bus) -> (Option<Obj>, Obj) {
        let r = match self.scan(b) {
            Scan::NoneLeft => {
                c.a[1] = A5 + self.first;
                c.set_ccr(1);
                None
            }
            Scan::Found(i) => {
                let obj = self.slot(i);
                let left = self.free_count(b);
                let new = c.addsub(true, 1, left as u32, false, Size::Word, false);
                wr16(b, A5 + self.free, new as u16);
                wr8(b, obj.at(Obj::CLASS), self.class);
                c.a[1] = obj.0;
                c.d[6] = self.slots - 1 - i;
                // or.b D0b,D0b
                c.flags_logic(c.d[0], Size::Byte);
                Some(obj)
            }
            Scan::Exhausted => {
                c.a[1] = self.end();
                c.d[6] = 0xffff;
                c.set_ccr(1);
                None
            }
        };
        (r, Obj(c.a[1]))
    }

    /// Allocation whose original caller does not test the failure: the returned object is the A1 that the original
    /// leaves.
    /// Original glitch: pool full, the caller writes into the first slot (counter zero, object possibly
    /// active) or just after the last one (in the next pool).
    pub fn alloc_unchecked(&self, b: &mut Cps2Bus) -> Obj {
        self.alloc_or_leftover(b).unwrap_or_else(|a1| a1)
    }

    /// Allocation whose failure also returns the A1 that the original leaves: the first slot (counter zero) or the
    /// end of the
    /// pool (no free slot). For original callers that re-read A1 after a failure.
    pub fn alloc_or_leftover(&self, b: &mut Cps2Bus) -> Result<Obj, Obj> {
        match self.scan(b) {
            Scan::NoneLeft => Err(Obj(A5 + self.first)),
            Scan::Found(_) => Ok(self.alloc(b).expect("slot found")),
            Scan::Exhausted => Err(Obj(self.end())),
        }
    }

    /// `alloc_unchecked` with the original's registers (see `alloc_68k`).
    #[track_caller]
    pub fn alloc_unchecked_68k(&self, c: &mut Cpu, b: &mut Cps2Bus) -> Obj {
        let (_, a1) = self.alloc_regs(c, b);
        crate::diag::dead_regs(c, "alloc", REGS_ALLOC);
        a1
    }

    /// Like `release`, and leaves the original's registers: D0 = 0, D1.w = -1, A4 = end of the slot,
    /// flags of `tst.l D0`. A0 (the object) is not touched.
    #[track_caller]
    pub fn release_68k(&self, c: &mut Cpu, b: &mut Cps2Bus, obj: Obj) {
        self.release_regs(c, b, obj);
        crate::diag::dead_regs(c, "free", REGS_FREE);
    }

    fn release_regs(&self, c: &mut Cpu, b: &mut Cps2Bus, obj: Obj) {
        if self.listed {
            lists::unlink(b, obj);
        }
        let left = self.free_count(b);
        let new = c.addsub(false, 1, left as u32, false, Size::Word, false);
        wr16(b, A5 + self.free, new as u16);
        self.clear_slot(b, obj);
        c.flags_logic(0, Size::Long);
        c.d[0] = 0;
        c.d[1] |= 0xffff;
        c.a[4] = obj.0 + self.stride;
    }
}

/// Allocation: A1 = slot found, C on failure; D6 serves as loop counter. The natives, called by
/// address, always return the registers (never poisoned).
fn alloc_native(pool: &Pool, c: &mut Cpu, b: &mut Cps2Bus) {
    pool.alloc_regs(c, b);
    rts(c, b);
}

/// Release: A0 = object. The original leaves D0 = 0, D1.w = -1, A4 = end of the slot.
fn free_native(pool: &Pool, c: &mut Cpu, b: &mut Cps2Bus) {
    pool.release_regs(c, b, Obj(c.a[0]));
    rts(c, b);
}

/// Initialization of consecutive pools. Leaves D0 = 0, D1.w = D7.w = -1 and A0 = A4 = end of the
/// last slot (A0 = collision queue array if the last pool has one).
fn init_native(pools: &[&Pool], c: &mut Cpu, b: &mut Cps2Bus) {
    for pool in pools {
        pool.reset(b);
    }
    let last = pools[pools.len() - 1];
    c.d[0] = 0;
    c.d[1] |= 0xffff;
    c.d[7] |= 0xffff;
    c.a[4] = last.end();
    c.a[0] = last.queue.map_or(last.end(), |q| A5 + q.array);
    c.flags_logic(last.slots, Size::Word);
    rts(c, b);
}

macro_rules! pool_natives {
    ($pool:ident, $alloc:ident, $alloc_name:expr, $free:ident, $free_name:expr) => {
        pub const $alloc: Native = Native {
            entry: $pool.alloc_routine,
            name: $alloc_name,
            run: crate::Body::Processeur(|c, b| alloc_native(&$pool, c, b)),
            touches_sound: false,
            outputs: regs::a(1) | regs::d(6) | regs::CCR,
        };
        pub const $free: Native = Native {
            entry: $pool.free_routine,
            name: $free_name,
            run: crate::Body::Processeur(|c, b| free_native(&$pool, c, b)),
            touches_sound: false,
            outputs: regs::d(0) | regs::d(1) | regs::a(0) | regs::a(4) | regs::CCR,
        };
    };
}

pool_natives!(PSHL, ALLOC_PSHL, "pool_alloc_pshl", FREE_PSHL, "pool_free_pshl");
pool_natives!(PEFF, ALLOC_PEFF, "pool_alloc_peff", FREE_PEFF, "pool_free_peff");
pool_natives!(ENEMY, ALLOC_ENEMY, "pool_alloc_enemy", FREE_ENEMY, "pool_free_enemy");
pool_natives!(ESHL, ALLOC_ESHL, "pool_alloc_eshl", FREE_ESHL, "pool_free_eshl");
pool_natives!(EFFCT, ALLOC_EFFCT, "pool_alloc_effct", FREE_EFFCT, "pool_free_effct");
pool_natives!(SET, ALLOC_SET, "pool_alloc_set", FREE_SET, "pool_free_set");
pool_natives!(BOX, ALLOC_BOX, "pool_alloc_box", FREE_BOX, "pool_free_box");
pool_natives!(BOSS, ALLOC_BOSS, "pool_alloc_boss", FREE_BOSS, "pool_free_boss");
pool_natives!(ITEM, ALLOC_ITEM, "pool_alloc_item", FREE_ITEM, "pool_free_item");
pool_natives!(SOBJ, ALLOC_SOBJ, "pool_alloc_sobj", FREE_SOBJ, "pool_free_sobj");

macro_rules! init_native {
    ($const:ident, $entry:expr, $name:expr, $pools:expr) => {
        pub const $const: Native = Native {
            entry: $entry,
            name: $name,
            run: crate::Body::Processeur(|c, b| init_native(&$pools, c, b)),
            touches_sound: false,
            outputs: regs::ALL,
        };
    };
}

// 0x2062: initializes the 10 pools.
init_native!(INIT_ALL, 0x2062, "pools_init_all", POOLS);
// 0x20ae: initializes the PEFF pool (reopening of the chained shop, `scene::shop`).
init_native!(INIT_PEFF, 0x20ae, "pool_init_peff", [&PEFF]);
// 0x2134: initializes the SET pool.
init_native!(INIT_SET, 0x2134, "pool_init_set", [&SET]);
// 0x2150: initializes the EFFCT pool.
init_native!(INIT_EFFCT, 0x2150, "pool_init_effct", [&EFFCT]);
// 0x21bc: initializes the SOBJ pool.
init_native!(INIT_SOBJ, 0x21bc, "pool_init_sobj", [&SOBJ]);
