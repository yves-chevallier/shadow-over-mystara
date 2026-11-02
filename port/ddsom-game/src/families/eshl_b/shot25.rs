use super::*;

const ANIMS: u32 = 0x16ea18;
const IMPACT_EFFECT: crate::sound::Sound = crate::sound::CMD_34D_44D;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => wait_and_drop(x),
        2 => fall(x),
        4 => x.add8(0x4, 2),
        _ => x.free_eshl(),
    }
}

fn wait_and_drop<R: Bench>(x: &mut Cx<'_, R>) {
    x.add16(0x58, 0xffff);
    if x.u16(0x58) != 0 {
        return;
    }
    x.add8(0x4, 2);
    x.set8(f::BOX_SET, 7);
    x.set8(f::ATTACK_ID, 0x17);
    let source = x.linked(0x5c);
    let (sx, sy) = (source.u16(x.b, 0x8), source.u16(x.b, 0x10));
    x.set16(0x8, sx);
    x.set16(0x10, sy);
    let camera = if x.a5_u8(0x36) != 0 { x.a5_u16(0x55a) } else { x.a5_u16(0x50a) };
    x.set16(0xc, camera.wrapping_add(0x100).wrapping_sub(sy));
    x.set16(0x18, -0x100i16 as u16);
    x.set16(0x1a, -0x40i16 as u16);
    let variant = x.u8(0x3);
    if variant != 0 {
        let rng = x.random();
        let mut dx = rng & 0x7f;
        if variant & 1 != 0 {
            dx = dx.wrapping_neg();
        }
        x.add16(0x8, dx);
        let rng = x.random();
        let r = rng & 0x3f;
        let dy = if r & 1 != 0 { 0xffff } else { 0 };
        x.add16(0x10, dy);
    }
    x.anim_start(ANIMS, 0);
    if let Some(shadow) = crate::families::small::shot_18(&mut crate::families::effects_creators::leftovers::Pure, x.b, x.o, 0) {
        x.set16(0x5a, low16(shadow));
    }
    x.reinsert();
}

fn fall<R: Bench>(x: &mut Cx<'_, R>) {
    x.anim_step();
    x.move_z();
    if (x.u16(0xc) as i16) >= 0 {
        return;
    }
    x.add8(0x4, 2);
    x.set32(0xc, 0);
    crate::families::effects_creators::debris(x.b, x.o, 3);
    crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::B, 8);
    crate::sound::play(x.b, IMPACT_EFFECT, x.o);
    if x.u16(0x5a) != 0 {
        let shadow = x.linked(0x5a);
        shadow.set_u8(x.b, 0x4, 6);
    }
}
