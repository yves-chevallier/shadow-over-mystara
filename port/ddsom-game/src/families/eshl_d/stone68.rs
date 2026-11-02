use super::*;
use crate::data::RomAddr;
use crate::ram;
use crate::families::shell::tables::{tables, Read};

const ANIMS: RomAddr = RomAddr(0x17b9d6);
const CARRIER: u32 = 0x46;
const RANK: u32 = 0x2f;
const TIMER: u32 = 0x42;
const SPEED_TABLE: u32 = 31;
const SFX_RELEASE: crate::sound::Sound = crate::sound::CMD_387_487;
const RELEASE_STATE: u32 = 0x0204_0200;
const RISE_HEIGHT: u16 = 0x20;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => {
            x.add8(Obj::STATE, 2);
            x.set8(Base::SUB_STATE, 0);
            x.set8(Base::BOX_SET, 0x2c);
            x.set8(Shot::ATTACK_ID, 0x33);
            x.set16(Obj::Y, 0x1c0);
            x.set16(Obj::Z, 0xff90);
            let rank = x.u8(RANK) as u32;
            let column = tables().stone68_columns.at16(x.b, 2 * rank);
            x.set16(Obj::X, column);
            x.set16(TIMER, 8);
            x.anim_start(ANIMS.0, 0x2a);
        }
        2 => active(x),
        4 => x.add8(Obj::STATE, 2),
        _ => x.free_eshl(),
    }
}

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    let carrier = x.u16(CARRIER);
    if carrier as i16 >= 0 {
        x.set16(Obj::STATE, 4);
        return;
    }
    let carrier = Obj(carrier as i16 as i32 as u32 & 0xff_ffff);
    match x.u8(Base::SUB_STATE) {
        0 => {
            let z = x.u16(Obj::Z);
            let z = z.wrapping_sub((z.wrapping_sub(RISE_HEIGHT) as i16 >> 3) as u16);
            x.set16(Obj::Z, z);
            if (z as i16) < RISE_HEIGHT as i16 {
                x.set_bit(Obj::FLAGS, 1, false);
                x.reinsert_foreground();
                return;
            }
            x.set16(Obj::Z, RISE_HEIGHT);
            x.add8(Base::SUB_STATE, 2);
            x.set32(Obj::VX, 0);
            x.set32(Obj::VY, 0);
            x.set32(Obj::VZ, 0);
            let rank = (x.u8(RANK) & 7) as u32;
            let dir = tables().stone68_directions.at8(x.b, rank);
            x.set8(Obj::SPEED_INDEX, dir);
            x.select_speed(&crate::objects::motion::SPEEDS_A, SPEED_TABLE as u16);
            x.unlink();
        }
        2 => {
            let held = carrier.u8(x.b, Obj::ACTIVE) != 0 && carrier.u32(x.b, Obj::STATE) & !0xff == RELEASE_STATE;
            if !(held && (carrier.u8(x.b, Base::SUB_STEP) as i8) < 6) {
                x.add8(Base::SUB_STATE, 2);
                let rank = x.u8(RANK);
                if rank == 0 || rank == 5 {
                    crate::sound::play(x.b, SFX_RELEASE, x.o);
                }
                return;
            }
        }
        _ => {
            height_anim(x);
            x.move_xy();
            let top = ram::CAMERA_Y.get(x.b);
            if (x.u16(Obj::Y).wrapping_sub(top) as i16) <= 0 {
                x.add8(Obj::STATE, 2);
                return;
            }
        }
    }
    x.set_bit(Obj::FLAGS, 1, false);
    x.reinsert();
}

/// 0xa7d20: animation according to the height on the screen (increasing thresholds, the last one by default).
fn height_anim<R: Bench>(x: &mut Cx<'_, R>) {
    let h = x.u16(Obj::Y).wrapping_sub(camera(x.b).1.get(x.b)) as i16;
    let count = tables().stone68_height_anims.at16(x.b, 0);
    let mut anim = 0;
    for k in 0..=count as u32 {
        anim = tables().stone68_height_anims.at16(x.b, 2 + 4 * k);
        if h < tables().stone68_height_anims.at16(x.b, 4 + 4 * k) as i16 {
            break;
        }
    }
    x.anim_start(ANIMS.0, (anim as u32) as u16);
}
