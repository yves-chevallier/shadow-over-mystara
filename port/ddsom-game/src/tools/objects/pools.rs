//! Tools: the natives of `crate::world::objects::pools` for the original code that calls them (arguments and results in the registers
//! of the original).

use crate::world::World;
use m68k::decode::Size;
use m68k::Cpu;

use crate::world::objects::pools::*;
use crate::tools::frames::*;
use crate::{regs, Native};
use crate::world::objects::lists;
use crate::world::objects::Obj;
use crate::world::mem::{wr16, wr8, A5};

macro_rules! pool_natives {
    ($pool:ident, $alloc:ident, $alloc_name:expr, $free:ident, $free_name:expr) => {
        pub(crate) const $alloc: Native = Native {
            entry: $pool.alloc_routine,
            name: $alloc_name,
            run: crate::Body::WithCpu(|c, b| alloc_native(&$pool, c, b)),
            touches_sound: false,
            outputs: regs::a(1) | regs::d(6) | regs::CCR,
        };
        pub(crate) const $free: Native = Native {
            entry: $pool.free_routine,
            name: $free_name,
            run: crate::Body::WithCpu(|c, b| free_native(&$pool, c, b)),
            touches_sound: false,
            outputs: regs::d(0) | regs::d(1) | regs::a(0) | regs::a(4) | regs::CCR,
        };
    };
}

macro_rules! init_native {
    ($const:ident, $entry:expr, $name:expr, $pools:expr) => {
        pub(crate) const $const: Native = Native {
            entry: $entry,
            name: $name,
            run: crate::Body::WithCpu(|c, b| init_native(&$pools, c, b)),
            touches_sound: false,
            outputs: regs::ALL,
        };
    };
}

/// Allocation: A1 = slot found, C on failure; D6 serves as loop counter. The natives, called by
/// address, always return the registers (never poisoned).
fn alloc_native(pool: &Pool, c: &mut Cpu, b: &mut World) {
    pool.alloc_regs(c, b);
    rts(c, b);
}

/// Release: A0 = object. The original leaves D0 = 0, D1.w = -1, A4 = end of the slot.
fn free_native(pool: &Pool, c: &mut Cpu, b: &mut World) {
    pool.release_regs(c, b, Obj(c.a[0]));
    rts(c, b);
}

/// Initialization of consecutive pools. Leaves D0 = 0, D1.w = D7.w = -1 and A0 = A4 = end of the
/// last slot (A0 = collision queue array if the last pool has one).
fn init_native(pools: &[&Pool], c: &mut Cpu, b: &mut World) {
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

impl Pool {
    /// Like `alloc`, and leaves the original's registers: A1 = slot found (on failure: the first
    /// slot if the counter is zero, the end of the pool if no slot is free), D6 = loop counter,
    /// carry = failure. For the callers not yet freed from registers (`docs/guide-refactorisation.md`).
    #[track_caller]
    pub(crate) fn alloc_68k(&self, c: &mut Cpu, b: &mut World) -> Option<Obj> {
        let (r, _) = self.alloc_regs(c, b);
        crate::tools::diag::dead_regs(c, "alloc", REGS_ALLOC);
        r
    }

    /// Allocation of the original, registers included: also returns the A1 left behind.
    fn alloc_regs(&self, c: &mut Cpu, b: &mut World) -> (Option<Obj>, Obj) {
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

    /// `alloc_unchecked` with the original's registers (see `alloc_68k`).
    #[track_caller]
    pub(crate) fn alloc_unchecked_68k(&self, c: &mut Cpu, b: &mut World) -> Obj {
        let (_, a1) = self.alloc_regs(c, b);
        crate::tools::diag::dead_regs(c, "alloc", REGS_ALLOC);
        a1
    }

    fn release_regs(&self, c: &mut Cpu, b: &mut World, obj: Obj) {
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

/// Masks of the registers that the original leaves (see `diag::dead_regs`).
pub(crate) const REGS_ALLOC: u32 = regs::a(1) | regs::d(6) | regs::CCR;

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
