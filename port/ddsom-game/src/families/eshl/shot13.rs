use super::*;
use crate::families::shell::tables::{tables, Read};

const ANIMS: u32 = 0x17a276;
/// The handler's address (A1 left by the dispatch loop).
const HANDLER: u32 = 0x9f67c;
const CURRENT_ANIM: u32 = 0x42;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => {
            if x.a5_u8(GAME_PAUSED) != 0 {
                attach(x);
                return;
            }
            x.add8(0x4, 2);
            attach(x);
            x.set32(0xc, 0);
            x.set8(f::BOX_SET, 0x29);
            x.anim_start(ANIMS, 0xc);
        }
        2 => {
            let owner = attach(x);
            if x.a5_u8(GAME_PAUSED) != 0 {
                return;
            }
            let mode = owner.u8(x.b, 0xa1);
            let anim = if mode != 0 { 0xe } else { 0xc };
            if anim != x.u8(CURRENT_ANIM) {
                x.set8(CURRENT_ANIM, anim);
                x.anim_start(ANIMS, (anim as u32) as u16);
            }
            if mode != 2 {
                x.anim_step();
            }
            x.reinsert();
        }
        _ => x.free_eshl(),
    }
}

/// Sticks the effect to its owner (0x9f712). Without an owner, the effect goes to state 6 (`move.l #0x6000000,(4,A0)`
/// at 0x9f692, whose `rts` returns to the caller, which goes on) and A1 keeps the value left by the dispatch loop.
/// Original glitch: that A1 is the handler's address (0x9f67c); the "owner's `+0xa1`" byte read next is a constant
/// byte of its code.
fn attach<R: Bench>(x: &mut Cx<'_, R>) -> Obj {
    if x.u16(f::OWNER) == 0 {
        x.set32(0x4, 0x0600_0000);
        return Obj(HANDLER);
    }
    let owner = x.linked(f::OWNER);
    for off in [0x8, 0x10, 0xc] {
        let v = owner.u32(x.b, off);
        x.set32(off, v);
    }
    let flip = owner.u8(x.b, f::FLIP);
    x.set8(f::FLIP, flip);
    let row = (8 * x.u8(0x3) as u32) & 0xff;
    let (dx, dy, dfrac, dz) = (tables().shot13_offsets.at16(x.b, row), tables().shot13_offsets.at16(x.b, row + 2), tables().shot13_offsets.at16(x.b, row + 4), tables().shot13_offsets.at16(x.b, row + 6));
    x.add16(0x8, if flip != 0 { dx.wrapping_neg() } else { dx });
    x.add16(0x10, dy);
    x.add32(0x10, dfrac as i16 as i32 as u32);
    x.add16(0xc, dz);
    owner
}
