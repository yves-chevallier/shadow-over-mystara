//! EFFCT family (class 0xE): visual effects (sparks, bursts, banners, animated interface elements). Dispatch `0x39cba`,
//! type table `0x39ce0` (92 types, 38 slots of 0x60 bytes).
//!
//! All the handlers share the same framework: the state byte `+4` chooses, through a table of relative words, the
//! initialization (0), the active state (2) or the release (4). The types ported here are those that the scenarios
//! exercise; the others stay in original code. See `docs/familles.md`.

use crate::world::objects::pools;
use crate::world::objects::Obj;

use super::{add_u8, handler, pointer};
use super::cx::{Bench, Cx};
use crate::world::objects::Current;
use crate::world::machine::Machine;

pub mod b;
pub mod c;
pub mod d;
pub mod e;
pub mod f;
pub mod g;

mod player;
use player::*;
mod screen;
use screen::*;
mod bursts;
use bursts::*;
mod followers;
use followers::*;

pub(crate) mod tables;
pub mod records;
pub mod vars;
use records::*;
use tables::{tables, Read, Table};

/// Values of the state byte `+4`.
mod state {
    pub const INIT: u8 = 0;
    pub const ACTIVE: u8 = 2;
    pub const FREED: u8 = 4;
    /// Set by the parent object that destroys its effects (or by the effect itself, `accessory_follow`): release,
    /// without the work of state 4 (`jmp 0x2218`).
    pub const DESTROYED: u8 = 6;
}

/// Shared fields.
mod field {
    pub const SUB_STATE: u32 = 0x05;
    pub const MODE: u32 = 0x2f;
    pub const FLIP: u32 = 0x2c;
    pub const OWNER: u32 = 0x50;
}

/// "Interface" animation set common to many effects.
const ANIMATIONS_INTERFACE: crate::world::data::RomAddr = crate::world::data::RomAddr(0x1c03a6);

/// State, sub-state or variant `index` outside the original's jump table of the handler, which would jump into data or
/// into the middle of code: never produced (docs/code-mort.md, batch B).
pub(crate) fn bad_index(index: impl Into<u32>) -> ! {
    unreachable!("index {:#x} outside the jump table of the handler", index.into())
}

/// Pool full where the original jumps to `address`: `tst.w (1,A5)`, reading a word at an odd address. The 68000 raises
/// an address error and the game stops on its exception screen; the engine stops too (docs/code-mort.md, batch B).
pub(crate) fn address_error(address: u32) -> ! {
    panic!("pool full: the original reads a word at an odd address {address:#x} (address error, exception screen)")
}

/// Copies `X`, `Z`, `Y` (high words) of `source` to those of `o`.
fn copy_position<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, source: Obj) {
    for off in [Obj::X, Obj::Z, Obj::Y] {
        let v = source.u16(ctx.b, off);
        o.set_u16(ctx.b, off, v);
    }
}

/// Adds `n` to the high word of a coordinate.
fn shifted_u16<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, off: u32, n: u16) {
    let v = o.u16(ctx.b, off).wrapping_add(n);
    o.set_u16(ctx.b, off, v);
}

/// Three-state handler (init, active, release): reads the state, runs `init` or `active`; state 4 frees the object.
fn three_states<R: Bench>(
    ctx: &mut Cx<'_, R>,
    init: fn(&mut Cx<'_, R>, Obj),
    active: fn(&mut Cx<'_, R>, Obj),
) {
    let o = ctx.obj();
    let state = o.u8(ctx.b, Obj::STATE);
    match state {
        state::INIT => init(ctx, o),
        state::ACTIVE => active(ctx, o),
        state::FREED => ctx.free(&pools::EFFCT),
        state::DESTROYED => ctx.free(&pools::EFFCT),
        _ => bad_index(state),
    }
}

handler!(EFFCT_4, 0xaaac0, "effct_bubble", bubble_handler);
handler!(EFFCT_24, 0xb0ff2, "effct_accessory", accessory_handler);
handler!(EFFCT_45, 0xb5340, "effct_trigger", trigger_handler);
handler!(EFFCT_51, 0xb64c8, "effct_projectile", projectile_handler);
handler!(EFFCT_52, 0xb65e0, "effct_shimmer_class", shimmer_class_handler);
handler!(EFFCT_56, 0xb6c6e, "effct_shards_debris", shards_debris_handler);
handler!(EFFCT_8, 0xababa, "effct_trace", trace_handler);
handler!(EFFCT_28, 0xb136e, "effct_spark", spark_handler);
handler!(EFFCT_57, 0xb6bca, "effct_mark", mark_handler);
handler!(EFFCT_62, 0xb77ac, "effct_sheaf", sheaf_handler);
handler!(EFFCT_7, 0xaba4e, "effct_shard_single", shard_single_handler);
handler!(EFFCT_19, 0xaecce, "effct_sweep", sweep_handler);
handler!(EFFCT_27, 0xb12e6, "effct_smoke", smoke_handler);
handler!(EFFCT_1, 0xa8758, "effct_removed", removed);
handler!(EFFCT_6, 0xab7fa, "effct_debris", debris_handler);
handler!(EFFCT_29, 0xb1440, "effct_impact", impact_handler);
handler!(EFFCT_17, 0xae78c, "effct_interface", interface_handler);
handler!(EFFCT_5, 0xab726, "effct_halo", halo);
handler!(EFFCT_9, 0xabbee, "effct_trail", trail);
handler!(EFFCT_15, 0xae428, "effct_shimmer", shimmering);
handler!(EFFCT_21, 0xaf360, "effct_banner", banner);
handler!(EFFCT_25, 0xb111a, "effct_dust", dust);
handler!(EFFCT_26, 0xb11f4, "effct_cloud", cloud);
handler!(EFFCT_32, 0xb2c56, "effct_aura", aura);
handler!(EFFCT_33, 0xb2cca, "effct_shard", shard);
