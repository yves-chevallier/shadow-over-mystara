use super::*;
use crate::families::shell::tables::{tables, Read};

const BLINK_FRAMES: u16 = 0x14;
const BLINKS: u8 = 8;
/// RAM word (A5+0x46): followed player.
const TRACKED: u32 = 0x46;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if x.a5_u8(0x42) != 0 {
        x.set32(0x4, 0x0600_0000);
    }
    if suspended(x) {
        return;
    }
    match x.u8(0x4) {
        0 => {
            x.add8(0x4, 2);
            x.set16(0x14, BLINK_FRAMES);
            x.set8(0x16, BLINKS);
            place(x);
            let variant = x.u8(0x3) as u32;
            let anim = tables().sobj_frame_anims_by_variant.at16(x.b, 2 * variant) as u32;
            x.anim_start(ANIMS, anim as u16);
            x.reinsert();
        }
        2 => {
            place(x);
            let n = x.u16(0x14).wrapping_sub(1);
            x.set16(0x14, n);
            if (n as i16) >= 0 {
                return;
            }
            let v = x.u8(0x1) ^ 2;
            x.set8(0x1, v);
            x.set16(0x14, BLINK_FRAMES);
            let n = x.u8(0x16).wrapping_sub(1);
            x.set8(0x16, n);
            if (n as i8) < 0 {
                x.set8(0x4, 4);
            }
        }
        _ => x.free(&pools::SOBJ),
    }
}

/// Position: with `+0x2f`, it only stays if the followed player is in the depth zone
/// 0x170..0x260; otherwise, camera + table offset.
fn place<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(0x2f) != 0 {
        let who = x.a5_u16(TRACKED);
        let ok = who != 0 && {
            let depth = Obj(who as i16 as i32 as u32 & 0xff_ffff).u16(x.b, 0x10) as i16;
            (0x170..0x260).contains(&depth)
        };
        if !ok {
            return x.set16(0x4, LEAVE_6_0);
        }
    }
    let row = 4 * x.u8(0x3) as u32;
    let dx = tables().sobj_frame_offsets.at16(x.b, row);
    let dz = tables().sobj_frame_offsets.at16(x.b, row + 2);
    let v = x.a5_u16(ram::CAMERA_X).wrapping_add(dx);
    x.set16(0x8, v);
    let v = x.a5_u16(ram::CAMERA_Y).wrapping_add(dz);
    x.set16(0xc, v);
    x.set32(0x10, 0);
}
