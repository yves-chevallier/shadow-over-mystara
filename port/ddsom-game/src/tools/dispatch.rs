//! Tools: the natives of `crate::dispatch` for the original code that calls them (arguments and results in the registers
//! of the original).

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::{Cpu, SR_C, SR_N, SR_V, SR_X, SR_Z};

use crate::dispatch::*;
use crate::tools::frames::*;
use crate::{regs, Body, Native};
use crate::modes::Flow;
use crate::objects::pools::{self, Pool};
use crate::objects::Obj;

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
