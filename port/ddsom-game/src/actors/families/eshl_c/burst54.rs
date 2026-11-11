use super::*;
use crate::world::data::RomAddr;
use crate::actors::families::shell::tables::{tables, Read};

const ANIMS: RomAddr = RomAddr(0x1706f6);
const LINK: u32 = 0x5c;
const TIMER: u32 = 0x42;
const SOUND: crate::services::sound::Sound = crate::services::sound::CMD_366;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(LINK) == 0 {
        return x.free_eshl();
    }
    match x.u8(Obj::STATE) {
        0 => {
            crate::services::sound::play(x.b, SOUND, x.o);
            x.add8(Obj::STATE, 2);
            x.set8(Base::BOX_SET, 7);
            x.set8(Shot::ATTACK_ID, 0x23);
            let with_anim = x.u8(Base::SETTING) != 0;
            launch(x, with_anim);
        }
        2 => {
            match x.u8(Base::SUB_STATE) {
                0 => {
                    let t = x.u16(TIMER).wrapping_sub(1);
                    x.set16(TIMER, t);
                    if s16(t) < 0 {
                        x.add8(Base::SUB_STATE, 2);
                        x.set16(TIMER, 0xa);
                    }
                    x.move_xyz();
                    x.anim_step();
                }
                2 => {
                    let t = x.u16(TIMER).wrapping_sub(1);
                    x.set16(TIMER, t);
                    if s16(t) < 0 {
                        x.add8(Obj::STATE, 2);
                    }
                    x.anim_step();
                }
                _ => {}
            }
            blink_by_variant(x);
            x.reinsert();
        }
        _ => x.free_eshl(),
    }
}

/// Places itself on the launcher and takes the speeds of the variant's row.
fn launch<R: Bench>(x: &mut Cx<'_, R>, with_anim: bool) {
    let src = x.linked(LINK);
    for off in [0x8, 0x10, 0xc] {
        let v = src.u32(x.b, off);
        x.set32(off, v);
    }
    copy_flip(x, src);
    x.add16(Obj::Y, 0xffff);
    x.set16(TIMER, if with_anim { 0x1e } else { 0x3c });
    let flip = x.u8(Base::FLIP) ^ 0x20;
    x.set8(Base::FLIP, flip);
    let v = x.u8(Obj::VARIANT) as u32;
    let (table, row) = if with_anim { (&tables().burst54_rows_anim, 20 * v) } else { (&tables().burst54_rows, 18 * v) };
    let word = |x: &mut Cx<'_, R>, i: u32| table.at16(x.b, row + 2 * i);
    let (mut dx, dz, dy, mut vx, mut ax) = (word(x, 0), word(x, 1), word(x, 2), word(x, 3), word(x, 4));
    for (i, off) in [0x18u32, 0x1a, 0x1c, 0x1e].into_iter().enumerate() {
        let w = word(x, 5 + i as u32);
        x.set16(off, w);
    }
    if flip != 0 {
        dx = neg(dx);
        vx = neg(vx);
        ax = neg(ax);
    }
    x.add16(Obj::X, dx);
    x.add16(Obj::Z, dz);
    x.add16(Obj::Y, dy);
    x.set16(Obj::VX, vx);
    x.set16(Obj::AX, ax);
    let anim = if with_anim { word(x, 9) as u32 } else { 0 };
    x.anim_start(ANIMS.0, anim as u16);
}
