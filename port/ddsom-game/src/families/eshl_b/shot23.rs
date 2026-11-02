use super::*;
use crate::data::RomAddr;
use crate::families::shell::vars;
use crate::ram;
use crate::families::shell::records::view;

const ANIMS: RomAddr = RomAddr(0x1c03a6);
const WAIT: u8 = 0;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => init(x),
        2 => match x.u8(Base::SUB_STATE) {
            WAIT => {
                x.add8(Shot23::TIMER, 0xff);
                if (x.u8(Shot23::TIMER) as i8) < 0 {
                    x.add8(Base::SUB_STATE, 2);
                }
                slide(x);
            }
            _ => {
                if vars::FROZEN.get(x.b) == 0 {
                    x.anim_step();
                    return;
                }
                x.set8(Shot23::TIMER, 0x30);
                x.add8(Obj::STATE, 2);
                x.set8(Obj::SPEED_INDEX, 0x10);
                speed_table(x, 2);
                x.anim_step();
            }
        },
        4 => {
            x.add8(Shot23::TIMER, 0xff);
            if (x.u8(Shot23::TIMER) as i8) < 0 {
                x.add8(Obj::STATE, 2);
            }
            slide(x);
        }
        _ => x.free_eshl(),
    }
}

fn slide<R: Bench>(x: &mut Cx<'_, R>) {
    x.move_by_table_xy();
    x.anim_step();
    reinsert_foreground(x);
}

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Obj::STATE, 2);
    x.set8(Shot23::TIMER, 0x30);
    x.set8(Obj::SPEED_INDEX, 0);
    speed_table(x, 0xd);
    let camera = ram::CAMERA_ALT_X.get(x.b);
    x.set16(Obj::X, camera);
    x.set32(Obj::Z, 0);
    x.set32(Obj::Y, 0);
    x.set16(Obj::Z, 0x70);
    x.add16(Obj::Y, -0x20i16 as u16);
    let mut anim = 0x4a;
    if x.u8(Base::SETTING) != 0 {
        anim += 0x9f;
        x.add16(Obj::Y, 0x80);
        x.add16(Obj::Z, -0x10i16 as u16);
    }
    x.anim_start(ANIMS.0, anim);
}

view! {
    /// Fields of shot 23.
    Shot23 {
        /// Countdown (h).
        timer, set_timer, TIMER: u8 @ 0x54;
    }
}
