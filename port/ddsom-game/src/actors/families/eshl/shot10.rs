use super::*;
use crate::world::data::RomAddr;
use crate::actors::families::shell::records::view;
use crate::actors::families::shell::tables::{tables, Read};

const ANIMS: RomAddr = RomAddr(0x16da84);

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => {
            x.add8(Obj::STATE, 2);
            x.set8(Shot::ATTACK_POWER, 0x10);
            x.set8(Shot::ATTACK_ID, 1);
            x.set8(Base::BOX_SET, 0x40);
            x.add32(Obj::Y, u32::MAX);
            // Original glitch: the placement row is the low word of D5, left by the previous handler
            let row = *x.c.d5() as u16 as i16 as i32 as u32;
            let (z, dx) = (tables().shot10_placement.at16(x.b, row), tables().shot10_placement.at16(x.b, row + 2));
            x.set16(Obj::Z, z);
            let flip = x.u8(Base::FLIP) != 0;
            x.add16(Obj::X, if flip { dx } else { dx.wrapping_neg() });
            x.anim_start(ANIMS.0, 0);
        }
        2 => {
            let owner = x.linked(Shot::OWNER);
            if owner.u8(x.b, Obj::STATE) as i8 >= 4 {
                return x.free_eshl();
            }
            match x.u8(Base::SUB_STATE) {
                2 => {
                    x.add8(Base::SUB_STATE, 2);
                    x.set16(Shot10::TIMER, 0xf0);
                    x.anim_start(ANIMS.0, 1);
                }
                0 | 1 => {
                    x.anim_step();
                    x.reinsert();
                }
                _ => {
                    let t = x.u16(Shot10::TIMER).wrapping_sub(1);
                    x.set16(Shot10::TIMER, t);
                    if (t as i16) < 0 {
                        x.add8(Obj::STATE, 2);
                    }
                    x.anim_step();
                    x.reinsert();
                }
            }
        }
        _ => x.free_eshl(),
    }
}

view! {
    /// Fields of shot 10.
    Shot10 {
        /// Countdown (h).
        timer, set_timer, TIMER: u16 @ 0x52;
    }
}
