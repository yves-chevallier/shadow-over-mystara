use super::*;
use crate::data::RomAddr;
use crate::families::shell::vars;
use crate::ram;
use crate::families::shell::records::Base;

const PALETTE: RomAddr = RomAddr(0x28ff20);
const PALETTE_DST: u32 = 0x91c3c0;
const SLIDE_ANIMS: RomAddr = RomAddr(0x1c40c6);

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Obj::STATE) == 0 {
        x.add8(Obj::STATE, 2);
        copy_longs(x, PALETTE, PALETTE_DST, 8);
        or_longs(x, PALETTE_DST, BRIGHT, 8);
        x.set8(Base::PALETTE, 0x1e);
        x.set8(Base::OWN_PALETTE, 1);
        let v = ram::CAMERA_X.get(x.b).wrapping_add(0x20);
        x.set16(Obj::X, v);
        x.set16(Obj::Z, 0x1c8);
        x.set16(Obj::Y, 0xff00);
        x.anim_start(SLIDE_ANIMS.0, 0x3b);
        x.reinsert();
    }
    let d = ram::CAMERA_X.get(x.b).wrapping_sub(vars::PREV_CAMERA_X_WORD.get(x.b));
    x.add16(Obj::X, d);
    if !x.anim_step() {
        return;
    }
    for i in 0..4 {
        let at = PLAYERS + 2 * i;
        let who = rd16(x.b, at);
        if who != 0 {
            wr16(x.b, who as i16 as i32 as u32 + 4, 0x400);
            wr16(x.b, at, 0);
        }
    }
    x.free(&pools::SOBJ);
}
