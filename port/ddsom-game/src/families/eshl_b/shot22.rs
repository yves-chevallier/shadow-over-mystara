use super::*;
use crate::data::RomAddr;
use crate::families::shell::records::view;
use crate::families::shell::tables::{tables, Read};

const ANIMS: RomAddr = RomAddr(0x1649d0);
const FLY: u8 = 0;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => init(x),
        2 => {
            if x.u8(Base::SUB_STATE) == FLY {
                x.move_xz();
                if (x.u16(Obj::Z) as i16) > 0 {
                    return;
                }
                x.set32(Obj::Z, 0);
                x.add8(Base::SUB_STATE, 2);
            } else {
                let hidden = !x.bit(Obj::FLAGS, 1);
                x.set_bit(Obj::FLAGS, 1, hidden);
                x.add8(Shot22::TIMER, 0xff);
                if x.u8(Shot22::TIMER) == 0 {
                    x.free_eshl();
                }
            }
        }
        _ => x.free_eshl(),
    }
}

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Obj::STATE, 2);
    x.set8(Shot22::TIMER, 0x1e);
    x.set8(Shot::ATTACK_ID, 0x1a);
    x.set8(Shot::ATTACK_POWER, 5);
    let row = (x.u8(Base::SETTING) << 2) as u32;
    let mut dx = tables().shot22_offsets.at16(x.b, row);
    let dz = tables().shot22_offsets.at16(x.b, row + 2);
    x.add16(Obj::Z, dz);
    x.add16(Obj::Y, 0xffff);
    x.add16(Obj::Z, 1);
    let variant = x.u8(Obj::VARIANT);
    let row = (variant as u32) << 3;
    let vx = tables().shot22_speeds.at32(x.b, row);
    x.set32(Obj::VX, vx);
    let vz = tables().shot22_speeds.at32(x.b, row + 4);
    x.set32(Obj::VZ, vz);
    if x.u8(Base::FLIP) != 0 {
        dx = dx.wrapping_neg();
        x.set32(Obj::VX, vx.wrapping_neg());
    }
    x.add16(Obj::X, dx);
    x.anim_start(ANIMS.0, (variant.wrapping_add(10) as u32) as u16);
    x.reinsert();
}

view! {
    /// Fields of shot 22.
    Shot22 {
        /// Countdown (h).
        timer, set_timer, TIMER: u8 @ 0x54;
    }
}
