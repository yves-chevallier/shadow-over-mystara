use super::*;
use crate::world::data::RomAddr;
use crate::actors::families::shell::tables::{tables, Read};

const ANIMS: RomAddr = RomAddr(0x16ea18);
const IMPACT_SOUND: crate::hw::sound::Sound = crate::hw::sound::CMD_34D_44D;
const TIMER: u32 = 0x42;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => start(x),
        2 => live(x),
        4 => x.add8(Obj::STATE, 2),
        _ => x.free_eshl(),
    }
}

fn start<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Obj::STATE, 2);
    x.set8(Base::SUB_STATE, 0);
    x.set8(Base::BOX_SET, 7);
    x.set8(Shot::ATTACK_ID, 0x25);
    // camera: block of A5+0x4fe (player 1) or A5+0x54e (player 2)
    let (camera_x, camera_y) = camera(x.b);
    x.set16(Obj::Z, 0x100);
    let mut dx = x.random() & 0x1ff;
    if s16(dx) > 0x180 {
        dx = (dx >> 1) + 0x40;
    }
    let px = dx.wrapping_add(camera_x.get(x.b));
    x.set16(Obj::X, px);
    let mut dy = x.random() & 0x7f;
    if s16(dy) > 0x50 {
        dy = (dy >> 1) + 0x10;
    }
    let py = dy.wrapping_add(0x10).wrapping_add(camera_y.get(x.b));
    x.set16(Obj::Y, py);
    x.set16(Obj::VZ, neg(0x200));
    x.set16(Obj::AZ, neg(0x20));
    x.set32(Obj::VX, 0);
    x.set32(Obj::VY, 0);
    let variant = x.u8(Obj::VARIANT) & 0x1f;
    let delay = tables().meteor62_delays.at8(x.b, variant as u32);
    x.set8(TIMER, delay);
    x.anim_start(ANIMS.0, 0);
}

fn live<R: Bench>(x: &mut Cx<'_, R>) {
    if s8(x.u8(TIMER)) > 0 {
        let t = x.u8(TIMER) - 1;
        return x.set8(TIMER, t);
    }
    x.anim_step();
    x.move_z();
    if s16(x.u16(Obj::Z)) < 0 {
        crate::actors::families::effects_creators::linked(x.b, x.o, crate::actors::families::effects_creators::kind::WISP, Some(1));
        x.set16(Obj::Z, 0);
        x.add8(Obj::STATE, 2);
        crate::actors::families::effects_creators::debris(x.b, x.o, 3);
        crate::services::camera_modes::start_shake(x.b, crate::services::camera_modes::shake::B, 8);
        crate::hw::sound::play(x.b, IMPACT_SOUND, x.o);
    }
    x.keep_on_screen(0x10);
    x.reinsert();
}
