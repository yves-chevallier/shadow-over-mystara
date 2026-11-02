use super::*;
use crate::families::shell::records::view;
use crate::ram;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => {
            x.add8(Obj::STATE, 2);
            let anim = if x.u8(Obj::VARIANT) == 0 { 0x13b } else { 0x13c };
            x.anim_start(ANIMS.0, anim);
            track(x)
        }
        2 => track(x),
        _ => x.free(&pools::SOBJ),
    }
}

fn track<R: Bench>(x: &mut Cx<'_, R>) {
    let target = x.linked(Marker::TARGET);
    for off in [0x8, 0xc, 0x10] {
        let v = target.u16(x.b, off);
        x.set16(off, v);
    }
    let lift = if x.u8(Obj::VARIANT) == 0 { 0x20 } else { 0x10 };
    x.add16(Obj::Z, 0u16.wrapping_sub(lift));
    x.add16(Obj::Y, 1);
    let limit = ram::CAMERA_X.get(x.b).wrapping_sub(0x40) as i16;
    if limit > x.u16(Obj::X) as i16 {
        return x.free(&pools::SOBJ);
    }
    x.anim_step();
    x.reinsert();
}

view! {
    /// Fields of the marker that follows an object.
    Marker {
        /// Object followed (16-bit pointer).
        target, set_target, TARGET: u16 @ 0x14;
    }
}
