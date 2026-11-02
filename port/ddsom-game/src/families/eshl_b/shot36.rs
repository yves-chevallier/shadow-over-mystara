use super::*;

const ANIMS: u32 = 0x16fbf4;
const SPEEDS: u32 = 0xa2aa4;
const X_OFFSETS: u32 = 0xa2b0a;
const Y_OFFSETS: u32 = 0xa2b2a;
const SOUND_LAND: crate::sound::Sound = crate::sound::CMD_330;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    guarded(x, state)
}

fn state<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => {
            x.add8(0x4, 2);
            x.set8(f::BOX_SET, 7);
            x.set8(f::ATTACK_POWER, 0x1e);
            launch(x);
            place(x);
            ai_start(x, ANIMS, 0);
        }
        2 => match x.u8(0x5) {
            0 => {
                x.move_xyz();
                if (x.u16(0xc) as i16) > 0 {
                    ai_step(x);
                    return;
                }
                x.add8(0x5, 2);
                x.set16(0xc, 0);
                crate::sound::play(x.b, SOUND_LAND, x.o);
                crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::B, 0x20);
                ai_start(x, ANIMS, 1);
            }
            2 => {
                if ai_step(x) {
                    x.add8(0x4, 2);
                }
            }
            _ => {}
        },
        _ => x.free_eshl(),
    }
}

/// Direction according to the orientation, speed table 0x17 and randomly drawn vertical speed.
fn launch<R: Bench>(x: &mut Cx<'_, R>) {
    let direction = if x.u8(f::FLIP) != 0 { 0x18 } else { 8 };
    x.set8(0x2d, direction);
    speed_table(x, 0x17);
    let rng = x.random();
    let r = rng as u32 & 0xf;
    let vz = x.rom16(SPEEDS + 2 * r);
    x.set16(0x18, vz);
}

/// Random position around the camera (left screen: offset of -0xc0).
fn place<R: Bench>(x: &mut Cx<'_, R>) {
    let rng = x.random();
    let r = rng as u32 & 0xf;
    let mut dx = x.rom16(X_OFFSETS + 2 * r);
    let rng = x.random();
    let r = rng as u32 & 0xf;
    let dy = x.rom16(Y_OFFSETS + 2 * r);
    if x.u8(f::FLIP) == 0 {
        dx = dx.wrapping_sub(0xc0);
    }
    let z = 0x100u16.wrapping_sub(dy);
    let px = dx.wrapping_add(x.a5_u16(0x5a6));
    let py = dy.wrapping_add(x.a5_u16(0x5aa));
    x.set16(0x8, px);
    x.set16(0x10, py);
    x.set16(0xc, z);
}
