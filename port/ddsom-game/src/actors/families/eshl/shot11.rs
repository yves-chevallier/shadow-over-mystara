use super::*;
use crate::world::data::RomAddr;
use crate::actors::families::shell::vars;
use crate::actors::families::shell::records::{Host, view};
use crate::actors::families::shell::tables::{tables, Read};

const ANIMS: RomAddr = RomAddr(0x169fb0);

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => {
            x.add8(Obj::STATE, 2);
            attach(x);
            x.set32(Obj::Z, 0);
            x.set8(Base::BOX_SET, 0x3b);
            x.anim_start(ANIMS.0, 1);
        }
        2 => {
            if frozen(x) {
                return x.reinsert();
            }
            if vars::GAME_PAUSED.get(x.b) == 0 {
                let owner = attach(x);
                let flags = owner.u8(x.b, Host::ANIM_SYNC);
                if flags != 0 {
                    let differ = (flags & 1 != 0) != (flags & 2 != 0);
                    if !differ || vars::FRAME_BYTE.get(x.b) & 1 != 0 {
                        x.anim_step();
                    }
                }
            }
            x.reinsert();
        }
        4 => {
            let owner = x.linked(Shot::OWNER);
            if owner.u8(x.b, Obj::STATE) == 4 {
                if !frozen(x) {
                    attach(x);
                }
            } else {
                let v = x.u16(Shot11::HALO);
                owner.set_u16(x.b, Host::HALO, v);
                owner.set_u32(x.b, Obj::STATE, 0x0400_0000);
            }
            x.reinsert();
        }
        _ => x.free_eshl(),
    }
}

/// Countdown `+0x53` positive: decremented, and the halo stays frozen.
fn frozen<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let n = x.u8(Obj::COUNTER_53);
    if (n as i8) > 0 {
        x.set8(Obj::COUNTER_53, n - 1);
        return true;
    }
    false
}

fn attach<R: Bench>(x: &mut Cx<'_, R>) -> Obj {
    let owner = x.linked(Shot::OWNER);
    for off in [0x8, 0x10] {
        let v = owner.u32(x.b, off);
        x.set32(off, v);
    }
    let row = (4 * x.u8(Obj::VARIANT) as u32) & 0xff;
    let (dx, dy) = (tables().shot11_offsets.at16(x.b, row), tables().shot11_offsets.at16(x.b, row + 2));
    x.add16(Obj::X, dx);
    x.add16(Obj::Y, dy);
    owner
}

view! {
    /// Fields of shot 11.
    Shot11 {
        /// Value given back to the owner at the end (h).
        halo, set_halo, HALO: u16 @ 0x54;
    }
}
