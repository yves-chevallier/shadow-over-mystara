//! Tools: the natives of `crate::families::reactions` for the original code that calls them (arguments and results in the registers
//! of the original).


use crate::tools::frames::*;
use crate::Native;
use crate::mem::{rd16, rd32, rd8};
use crate::objects::anim::load_entry;
use crate::objects::Obj;

/// Player reaction animations (longs per character, data).
const PLAYER_HURT_SETS: u32 = 0x3bb52;

/// 0x3bb16: player reaction animation `D6` (set per character `+3`, table 0x3bb52).
fn player_hurt_animation(c: &mut m68k::Cpu, b: &mut cps2::Cps2Bus) {
    let o = Obj(c.a[0]);
    let character = rd8(b, o.0 + 3) as u32;
    c.d[5] = 4 * character;
    let set = rd32(b, PLAYER_HURT_SETS + 4 * character);
    let index = (c.d[6] as u16).wrapping_mul(2) as i16 as i32 as u32;
    let off = rd16(b, set.wrapping_add(index));
    c.d[6] = c.d[6] & !0xffff | off as u32;
    let at = set.wrapping_add(off as i16 as i32 as u32);
    c.a[4] = load_entry(b, o, at, true);
    rts(c, b);
}

/// Player reaction animation (called by the player states that remained in 68000).
pub(crate) const PLAYER_HURT_ANIMATION: Native = Native {
    entry: 0x3bb16,
    name: "player_hurt_animation",
    run: crate::Body::WithCpu(player_hurt_animation),
    touches_sound: false,
    outputs: 0,
};
