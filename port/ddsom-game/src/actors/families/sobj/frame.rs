use super::*;
use crate::actors::families::shell::vars;
use crate::world::ram;
use crate::actors::families::shell::records::{Base, view};
use crate::actors::families::shell::tables::{tables, Read};

const BLINK_FRAMES: u16 = 0x14;
const BLINKS: u8 = 8;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if vars::FROZEN.get(x.b) != 0 {
        x.set32(Obj::STATE, 0x0600_0000);
    }
    if suspended(x) {
        return;
    }
    match x.u8(Obj::STATE) {
        0 => {
            x.add8(Obj::STATE, 2);
            x.set16(Frame::BLINK_TIMER, BLINK_FRAMES);
            x.set8(Frame::BLINKS, BLINKS);
            place(x);
            let variant = x.u8(Obj::VARIANT) as u32;
            let anim = tables().sobj_frame_anims_by_variant.at16(x.b, 2 * variant) as u32;
            x.anim_start(ANIMS.0, anim as u16);
            x.reinsert();
        }
        2 => {
            place(x);
            let n = x.u16(Frame::BLINK_TIMER).wrapping_sub(1);
            x.set16(Frame::BLINK_TIMER, n);
            if (n as i16) >= 0 {
                return;
            }
            let v = x.u8(Obj::FLAGS) ^ 2;
            x.set8(Obj::FLAGS, v);
            x.set16(Frame::BLINK_TIMER, BLINK_FRAMES);
            let n = x.u8(Frame::BLINKS).wrapping_sub(1);
            x.set8(Frame::BLINKS, n);
            if (n as i8) < 0 {
                x.set8(Obj::STATE, 4);
            }
        }
        _ => x.free(&pools::SOBJ),
    }
}

/// Position: with `+0x2f`, it only stays if the followed player is in the depth zone
/// 0x170..0x260; otherwise, camera + table offset.
fn place<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Base::SETTING) != 0 {
        let who = vars::LEADER.get(x.b);
        let ok = who != 0 && {
            let depth = Obj(who as i16 as i32 as u32 & 0xff_ffff).u16(x.b, Obj::Y) as i16;
            (0x170..0x260).contains(&depth)
        };
        if !ok {
            return x.set16(Obj::STATE, LEAVE_6_0);
        }
    }
    let row = 4 * x.u8(Obj::VARIANT) as u32;
    let dx = tables().sobj_frame_offsets.at16(x.b, row);
    let dz = tables().sobj_frame_offsets.at16(x.b, row + 2);
    let v = ram::CAMERA_X.get(x.b).wrapping_add(dx);
    x.set16(Obj::X, v);
    let v = ram::CAMERA_Y.get(x.b).wrapping_add(dz);
    x.set16(Obj::Z, v);
    x.set32(Obj::Y, 0);
}

view! {
    /// Fields of the blinking frame.
    Frame {
        /// Frames before the next blink.
        blink_timer, set_blink_timer, BLINK_TIMER: u16 @ 0x14;
        /// Blinks left.
        blinks, set_blinks, BLINKS: u8 @ 0x16;
    }
}
