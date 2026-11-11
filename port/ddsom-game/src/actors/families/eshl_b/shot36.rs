use super::*;
use crate::world::data::RomAddr;
use crate::actors::families::shell::vars;
use crate::actors::families::shell::tables::{tables, Read};

const ANIMS: RomAddr = RomAddr(0x16fbf4);
const SOUND_LAND: crate::services::sound::Sound = crate::services::sound::CMD_330;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    guarded(x, state)
}

fn state<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => {
            x.add8(Obj::STATE, 2);
            x.set8(Base::BOX_SET, 7);
            x.set8(Shot::ATTACK_POWER, 0x1e);
            launch(x);
            place(x);
            ai_start(x, ANIMS.0, 0);
        }
        2 => match x.u8(Base::SUB_STATE) {
            0 => {
                x.move_xyz();
                if (x.u16(Obj::Z) as i16) > 0 {
                    ai_step(x);
                    return;
                }
                x.add8(Base::SUB_STATE, 2);
                x.set16(Obj::Z, 0);
                crate::services::sound::play(x.b, SOUND_LAND, x.o);
                crate::actors::camera_modes::start_shake(x.b, crate::actors::camera_modes::shake::B, 0x20);
                ai_start(x, ANIMS.0, 1);
            }
            2 => {
                if ai_step(x) {
                    x.add8(Obj::STATE, 2);
                }
            }
            _ => {}
        },
        _ => x.free_eshl(),
    }
}

/// Direction according to the orientation, speed table 0x17 and randomly drawn vertical speed.
fn launch<R: Bench>(x: &mut Cx<'_, R>) {
    let direction = if x.u8(Base::FLIP) != 0 { 0x18 } else { 8 };
    x.set8(Obj::SPEED_INDEX, direction);
    speed_table(x, 0x17);
    let rng = x.random();
    let r = rng as u32 & 0xf;
    let vz = tables().shot36_speeds.at16(x.b, 2 * r);
    x.set16(Obj::VZ, vz);
}

/// Random position around the camera (left screen: offset of -0xc0).
fn place<R: Bench>(x: &mut Cx<'_, R>) {
    let rng = x.random();
    let r = rng as u32 & 0xf;
    let mut dx = tables().shot36_x_offsets.at16(x.b, 2 * r);
    let rng = x.random();
    let r = rng as u32 & 0xf;
    let dy = tables().shot36_y_offsets.at16(x.b, 2 * r);
    if x.u8(Base::FLIP) == 0 {
        dx = dx.wrapping_sub(0xc0);
    }
    let z = 0x100u16.wrapping_sub(dy);
    let px = dx.wrapping_add(vars::CAMERA1_X.get(x.b));
    let py = dy.wrapping_add(vars::CAMERA1_Y.get(x.b));
    x.set16(Obj::X, px);
    x.set16(Obj::Y, py);
    x.set16(Obj::Z, z);
}
