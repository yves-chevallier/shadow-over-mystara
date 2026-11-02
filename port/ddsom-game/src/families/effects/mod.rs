//! EFFCT family (class 0xE): visual effects (sparks, bursts, banners, animated interface elements). Dispatch `0x39cba`,
//! type table `0x39ce0` (92 types, 38 slots of 0x60 bytes).
//!
//! All the handlers share the same framework: the state byte `+4` chooses, through a table of relative words, the
//! initialization (0), the active state (2) or the release (4). The types ported here are those that the scenarios
//! exercise; the others stay in original code. See `docs/familles.md`.

use crate::objects::pools;
use crate::objects::Obj;

use super::{add_u8, handler, pointer};
use super::cx::{Bench, Cx};
use crate::objects::Current;
use crate::machine::Machine;

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

/// Players (4 records of 0x200 bytes).
const PLAYERS: u32 = 0xff85de;
const STEP_PLAYER: u32 = 0x200;
/// "Interface" animation set common to many effects.
const ANIMATIONS_INTERFACE: u32 = 0x1c03a6;

/// Target of the table jump `move.w (table,PC,Dn.w),D1 ; jmp (table,PC,D1.w)` for the index `index` (the address the
/// original would reach; only the indices outside the table still lead there, `outside_table`).
pub(crate) fn target<R: Bench>(ctx: &mut Cx<'_, R>, table: u32, index: u32) -> u32 {
    let word = ctx.prog16(table.wrapping_add(index)) as i16;
    table.wrapping_add(word as i32 as u32)
}

/// State or variant value outside the original's jump table, which would jump into data or into the middle of code:
/// never produced (docs/code-mort.md, batch B).
pub(crate) fn outside_table(target: u32) -> ! {
    unreachable!("index outside the jump table: the original would jump to {target:#x}")
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

/// Three-state handler (init, active, release): reads the state, jumps through the word table `table`, runs `init` or
/// `active`; state 4 frees the object.
fn three_states<R: Bench>(
    ctx: &mut Cx<'_, R>,
    table: u32,
    init: fn(&mut Cx<'_, R>, Obj),
    active: fn(&mut Cx<'_, R>, Obj),
) {
    let o = ctx.obj();
    let state = o.u8(ctx.b, Obj::STATE);
    let target = target(ctx, table, state as u32);
    match state {
        state::INIT => init(ctx, o),
        state::ACTIVE => active(ctx, o),
        state::FREED => ctx.free(&pools::EFFCT),
        state::DESTROYED => ctx.free(&pools::EFFCT),
        _ => outside_table(target),
    }
}

handler!(EFFCT_4, bubble::ENTRY, "effct_bubble", bubble_handler);
handler!(EFFCT_24, accessory::ENTRY, "effct_accessory", accessory_handler);
handler!(EFFCT_45, trigger::ENTRY, "effct_trigger", trigger_handler);
handler!(EFFCT_51, projectile::ENTRY, "effct_projectile", projectile_handler);
handler!(EFFCT_52, shimmer_class::ENTRY, "effct_shimmer_class", shimmer_class_handler);
handler!(EFFCT_56, shards_debris::ENTRY, "effct_shards_debris", shards_debris_handler);
handler!(EFFCT_8, trace::ENTRY, "effct_trace", trace_handler);
handler!(EFFCT_28, spark::ENTRY, "effct_spark", spark_handler);
handler!(EFFCT_57, mark::ENTRY, "effct_mark", mark_handler);
handler!(EFFCT_62, sheaf::ENTRY, "effct_sheaf", sheaf_handler);
handler!(EFFCT_7, shard_single::ENTRY, "effct_shard_single", shard_single_handler);
handler!(EFFCT_19, sweep::ENTRY, "effct_sweep", sweep_handler);
handler!(EFFCT_27, smoke::ENTRY, "effct_smoke", smoke_handler);
handler!(EFFCT_1, 0xa8758, "effct_removed", removed);
handler!(EFFCT_6, debris::ENTRY, "effct_debris", debris_handler);
handler!(EFFCT_29, impact::ENTRY, "effct_impact", impact_handler);
handler!(EFFCT_17, interface::ENTRY, "effct_interface", interface_handler);
handler!(EFFCT_5, halo::ENTRY, "effct_halo", halo);
handler!(EFFCT_9, trail::ENTRY, "effct_trail", trail);
handler!(EFFCT_15, shimmering::ENTRY, "effct_shimmer", shimmering);
handler!(EFFCT_21, banner::ENTRY, "effct_banner", banner);
handler!(EFFCT_25, dust::ENTRY, "effct_dust", dust);
handler!(EFFCT_26, cloud::ENTRY, "effct_cloud", cloud);
handler!(EFFCT_32, aura::ENTRY, "effct_aura", aura);
handler!(EFFCT_33, shard::ENTRY, "effct_shard", shard);
