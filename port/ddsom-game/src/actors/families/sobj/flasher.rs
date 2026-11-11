use super::*;
use crate::actors::families::shell::records::view;

const BLINKS: u8 = 0x10;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => {
            x.add8(Obj::STATE, 2);
            restart(x);
        }
        2 => {
            if x.anim_step() {
                x.set_bit(Obj::FLAGS, 1, true);
                let n = x.u8(Flasher::BLINKS).wrapping_sub(1);
                x.set8(Flasher::BLINKS, n);
                if (n as i8) < 0 {
                    x.set_bit(Obj::FLAGS, 1, false);
                    restart(x);
                }
            }
        }
        _ => return x.free(&pools::SOBJ),
    }
    let margin = *x.c.d2() as u16;
    cull_beyond(x, margin);
    x.reinsert();
}

fn restart<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Flasher::BLINKS, BLINKS);
    let variant = x.u8(Obj::VARIANT) as u32;
    x.anim_start(ANIMS.0, variant as u16);
}

view! {
    /// Fields of the blinking scenery (type 2).
    Flasher {
        /// Blinks left.
        blinks, set_blinks, BLINKS: u8 @ 0x14;
    }
}
