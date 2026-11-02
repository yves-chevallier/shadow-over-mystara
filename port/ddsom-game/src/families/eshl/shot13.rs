use super::*;
use crate::data::RomAddr;
use crate::families::shell::vars;
use crate::families::shell::records::Host;
use crate::families::shell::tables::{tables, Read};

const ANIMS: RomAddr = RomAddr(0x17a276);
/// The handler's address (A1 left by the dispatch loop).
const HANDLER: RomAddr = RomAddr(0x9f67c);
const CURRENT_ANIM: u32 = 0x42;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => {
            if vars::GAME_PAUSED.get(x.b) != 0 {
                attach(x);
                return;
            }
            x.add8(Obj::STATE, 2);
            attach(x);
            x.set32(Obj::Z, 0);
            x.set8(Base::BOX_SET, 0x29);
            x.anim_start(ANIMS.0, 0xc);
        }
        2 => {
            let owner = attach(x);
            if vars::GAME_PAUSED.get(x.b) != 0 {
                return;
            }
            let mode = owner.u8(x.b, Host::ATTACK_DETAIL);
            let anim = if mode != 0 { 0xe } else { 0xc };
            if anim != x.u8(CURRENT_ANIM) {
                x.set8(CURRENT_ANIM, anim);
                x.anim_start(ANIMS.0, (anim as u32) as u16);
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
    if x.u16(Shot::OWNER) == 0 {
        x.set32(Obj::STATE, 0x0600_0000);
        return Obj(HANDLER.0);
    }
    let owner = x.linked(Shot::OWNER);
    for off in [0x8, 0x10, 0xc] {
        let v = owner.u32(x.b, off);
        x.set32(off, v);
    }
    let flip = owner.u8(x.b, Base::FLIP);
    x.set8(Base::FLIP, flip);
    let row = (8 * x.u8(Obj::VARIANT) as u32) & 0xff;
    let (dx, dy, dfrac, dz) = (tables().shot13_offsets.at16(x.b, row), tables().shot13_offsets.at16(x.b, row + 2), tables().shot13_offsets.at16(x.b, row + 4), tables().shot13_offsets.at16(x.b, row + 6));
    x.add16(Obj::X, if flip != 0 { dx.wrapping_neg() } else { dx });
    x.add16(Obj::Y, dy);
    x.add32(Obj::Y, dfrac as i16 as i32 as u32);
    x.add16(Obj::Z, dz);
    owner
}
