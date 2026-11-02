//! EFFCT family: scene objects that command the players (type table `0x39ce0`).
//!
//! - 11: effects of an enemy's attacks (sparks, marks, reflections, columns, trails, pieces, sparkles,
//!   sprays); variants 0, 2, 5, 8 and 9, which no scenario exercises, are validated by
//!   `examples/fuzz_effct.rs` ;
//! - 13: raft that scrolls and carries the players away (the players' directions steer it);
//! - 14: passage to a sub-scene, which oscillates, waits for a player, then launches the transition
//!   (tile curtain in background 2): entry into the sub-scene (`0xada96`: scene context
//!   saved in the object, objects frozen), then return (`0xae11a`: context restored, players repositioned).

use crate::families::cx::Bench;
use crate::families::cx::{handlers, Cx, Gx};
use crate::mem::{rd16, rd8, wr16, wr32, wr8, A5};
use crate::objects::Obj;
use crate::objects::pools;
use crate::objects::Current;
use crate::machine::Machine;

mod gates;
use gates::*;
mod attacks;
use attacks::*;
use crate::families::effects::tables::{tables, Read};
use crate::families::effects::records::*;
use crate::families::effects::vars;

mod f {
    pub const STATE: u32 = 0x04;
    pub const SUB_STATE: u32 = 0x05;
    pub const VARIANT: u32 = 0x03;
}

const ANIM_SET: crate::data::RomAddr = crate::data::RomAddr(0x1c03a6);

fn players() -> impl DoubleEndedIterator<Item = Obj> {
    (0..4).map(|k| crate::ram::PLAYERS.nth(k))
}

fn free(x: &mut impl Current) {
    x.free(&pools::EFFCT);
}

/// Common preamble: the pause hook (0x29ea, Z false: object suspended), the game pause, and at
/// least one player present in a state ≤ 2. True if the handler continues.
fn scene_running<X: Current>(x: &mut X, suspended: impl FnOnce(&mut X)) -> bool {
    if x.suspended() {
        suspended(x);
        return false;
    }
    let b = x.bus();
    if vars::PAUSE.get(b) != 0 {
        return false;
    }
    players().any(|p| p.is_active(b) && (p.u8(b, f::STATE) as i8) <= 2)
}

handlers! {
    0xabeea "effct_attack" attack_fx,
    0xad63e "effct_radeau" raft,
    0xad866 "effct_passage" gate,
}
