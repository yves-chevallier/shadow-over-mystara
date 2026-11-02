//! Tools: the natives of `crate::families::reactions` for the original code that calls them (arguments and results in the registers
//! of the original).

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::{Cpu, SR_C, SR_N, SR_V, SR_X, SR_Z};

use crate::families::reactions::*;
use crate::tools::frames::*;
use crate::{regs, Body, Native};
use crate::families::cx::Bench;
use crate::families::cx::Cx;
use crate::families::enemy_lib as lib;
use crate::families::enemy_common::var;
use crate::families::enemy_common::detach_from_host;
use crate::mem::{rd16, rd32, rd8};
use crate::objects::anim::load_entry;
use crate::objects::Obj;
use crate::objects::pools;
use crate::objects::Current;
use crate::machine::Machine;

/// 0x3bb16: player reaction animation `D6` (set per character `+3`, table 0x3bb52).
fn player_hurt_animation(c: &mut m68k::Cpu, b: &mut cps2::Cps2Bus) {
    let o = Obj(c.a[0]);
    let character = rd8(b, o.0 + 3) as u32;
    c.d[5] = 4 * character;
    let set = rd32(b, rom::PLAYER_HURT + 4 * character);
    let index = (c.d[6] as u16).wrapping_mul(2) as i16 as i32 as u32;
    let off = rd16(b, set.wrapping_add(index));
    c.d[6] = c.d[6] & !0xffff | off as u32;
    let at = set.wrapping_add(off as i16 as i32 as u32);
    c.a[4] = load_entry(b, o, at, true);
    rts(c, b);
}

/// Player reaction animation (called by the player states that remained in 68000).
pub const PLAYER_HURT_ANIMATION: Native = Native {
    entry: 0x3bb16,
    name: "player_hurt_animation",
    run: crate::Body::WithCpu(player_hurt_animation),
    touches_sound: false,
    outputs: 0,
};
