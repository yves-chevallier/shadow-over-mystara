//! Tools: the natives of `crate::scene::exit` for the original code that calls them (arguments and results in the registers
//! of the original).

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::{Cpu, SR_C, SR_N, SR_V, SR_X, SR_Z};

use crate::scene::exit::*;
use crate::tools::frames::*;
use crate::{regs, Body, Native};
use crate::scene::fading;
use crate::scene::{player, scratch, var, Ctx, PLAYERS, PLAYER_STRIDE};
use crate::dispatch;
use crate::mem::{rd16, rd8, wr16, wr32, wr8, A5};
use crate::modes::Sequence as _;
use crate::objects::Obj;
use crate::tasks::table::Next;
use crate::objects::pools;
use crate::scene::exit::Update::{Full, NoCollisions, Nothing, Reduced, World};
use crate::scene::exit::St::Idle;

pub const PHASE_16: Native = Native {
    entry: 0xf3142,
    name: "scene_phase_16",
    run: crate::Body::WithCpu(|c, b| {
        if !phase_16(b) {
            rts(c, b);
        }
    }),
    touches_sound: false,
    outputs: 0,
};

pub fn patch_shared_next(c: &mut Cpu, b: &mut Cps2Bus) -> bool {
    if rd8(b, c.a[6] + 0xc) < SHARED_NEXT_TABLE_LEN {
        return false;
    }
    wr16(b, c.a[6] + 0x12, 0x65);
    c.d[0] &= 0xffff_0000;
    c.a[1] = 0xf7b88;
    c.pc = SHARED_NEXT_NONE;
    true
}

const SHARED_NEXT_TABLE_LEN: u8 = 20;

const SHARED_NEXT_NONE: u32 = 0xf792c;
