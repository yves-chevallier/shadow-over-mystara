use super::*;
use crate::actors::families::shell::tables::{tables, Read};

/// Per variant: animation set, attack identifier (the waits per rank are `rocks65_delays_0` and `_1`).
const ANIMS: [u32; 2] = [0x16ea18, 0x171574];
const ATTACK: [u8; 2] = [0x25, 0x38];
const TIMER: u32 = 0x42;
const RANK: u32 = 0x2f;
const SFX: [crate::services::sound::Sound; 2] = [crate::services::sound::CMD_34D_44D, crate::services::sound::CMD_330];

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    let v = (x.u8(Obj::VARIANT) != 0) as usize;
    // variant 1 has one more state (end of animation) before the release
    let state = x.u8(Obj::STATE);
    match (state, v) {
        (0, _) => init(x, v),
        (2, _) => fall(x, v),
        (4, 1) => {
            if x.anim_step() {
                x.add8(Obj::STATE, 2);
            }
        }
        (4, 0) | (6, 1) => x.add8(Obj::STATE, 2),
        _ => x.free_eshl(),
    }
}

fn init<R: Bench>(x: &mut Cx<'_, R>, v: usize) {
    x.add8(Obj::STATE, 2);
    x.set8(Base::SUB_STATE, 0);
    x.set8(Base::BOX_SET, 7);
    x.set8(Shot::ATTACK_ID, ATTACK[v]);
    let (screen_x, screen_y) = camera(x.b);
    x.set16(Obj::Z, 0x100);
    let r = x.random() & 0x1ff;
    let dx = if r > 0x180 { (r >> 1) + 0x40 } else { r };
    let left = screen_x.get(x.b);
    x.set16(Obj::X, dx.wrapping_add(left));
    let r = x.random() & 0x7f;
    let dz = if r > 0x50 { (r >> 1) + 0x10 } else { r } + 0x10;
    let top = screen_y.get(x.b);
    x.set16(Obj::Y, dz.wrapping_add(top));
    x.set16(Obj::VZ, 0xfe00);
    x.set16(Obj::AZ, 0xffe0);
    x.set32(Obj::VX, 0);
    x.set32(Obj::VY, 0);
    let rank = (x.u8(RANK) & 0x1f) as u32;
    let delays = [&tables().rocks65_delays_0, &tables().rocks65_delays_1][v];
    let delay = delays.at8(x.b, rank);
    x.set16(TIMER, delay as u16);
    x.anim_start(ANIMS[v], 0);
}

fn fall<R: Bench>(x: &mut Cx<'_, R>, v: usize) {
    if x.u16(TIMER) as i16 > 0 {
        x.add16(TIMER, 0xffff);
        return;
    }
    x.anim_step();
    x.move_z();
    if x.u16(Obj::Z) as i16 > 0 {
        x.keep_on_screen(16);
        x.reinsert();
        return;
    }
    x.set16(Obj::Z, 0);
    x.add8(Obj::STATE, 2);
    if v == 0 {
        crate::actors::families::effects_creators::debris(x.b, x.o, 3);
    } else {
        x.anim_start(ANIMS[1], 1);
    }
    crate::services::camera_modes::start_shake(x.b, crate::services::camera_modes::shake::B, 8);
    crate::services::sound::play(x.b, SFX[v], x.o);
}
