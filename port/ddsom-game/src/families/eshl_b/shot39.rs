use super::*;
use crate::families::shell::tables::{tables, Read};

const ANIMS: u32 = 0x16ff22;
const SOUND_IMPACT: crate::sound::Sound = crate::sound::CMD_327_427_C;
const FLY: u8 = 0;
const SETTLE: u8 = 2;
const BURN: u8 = 4;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(0x5c) == 0 {
        return kill(x);
    }
    match x.u8(0x4) {
        0 => {
            x.add8(0x4, 2);
            x.set8(f::BOX_SET, 7);
            x.set8(f::ATTACK_POWER, 5);
            x.set8(f::ATTACK_ID, 0x1d);
            x.set8(0x46, 4);
            launch(x);
            speed_table(x, 0x13);
        }
        2 => active(x),
        _ => x.free_eshl(),
    }
}

fn launch<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(0x5c) == 0 {
        return;
    }
    let source = x.linked(0x5c);
    copy_position32(x, source);
    let flip = source.u8(x.b, f::FLIP);
    x.set8(f::FLIP, flip);
    x.add16(0x10, 0xffff);
    x.add16(0xc, 0x100);
    x.set16(0x18, -0x500i16 as u16);
    x.set16(0x1a, -0x38i16 as u16);
    let variant = x.u8(0x3) as u32;
    let direction = tables().shot39_directions.at8(x.b, variant);
    x.set8(0x2d, direction);
    let anim = tables().shot39_start_anims.at8(x.b, variant) as u32;
    x.anim_start(ANIMS, anim as u16);
}

/// Cycles the 8 flame frames every 4 frames.
fn spin<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x46, 0xff);
    if (x.u8(0x46) as i8) >= 0 {
        return;
    }
    x.set8(0x46, 4);
    x.set8(0x2a, 1);
    let n = x.u8(0x47).wrapping_add(1) & 7;
    x.set8(0x47, n);
    let frame = tables().shot39_frames.at8(x.b, n as u32);
    x.set8(0x2b, frame);
}

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    spin(x);
    let source = x.linked(0x5c);
    if source.u16(x.b, 0x4) == 0x406 {
        return x.free_eshl();
    }
    match x.u8(0x5) {
        FLY => {
            x.move_by_table_xy();
            x.move_z();
            if (x.u16(0xc) as i16) <= 0 {
                crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::B, 0x30);
                crate::sound::play(x.b, SOUND_IMPACT, x.o);
                x.add8(0x5, 2);
                x.set16(0xc, 0);
                x.set8(0x42, 0x30);
            }
            x.anim_step();
        }
        SETTLE => {
            x.add8(0x42, 0xff);
            if (x.u8(0x42) as i8) < 0 {
                x.add8(0x5, 2);
                x.set8(0x42, 0x1e);
                let d = x.u8(0x2d).wrapping_sub(6) & 0x1f;
                x.set8(0x2d, d);
                x.set8(0x43, 3);
            }
            x.anim_step();
        }
        BURN => {
            x.anim_step();
            if (x.u8(0x42) as i8) < 0x14 {
                x.set8(0x3a, 0);
            }
            x.add8(0x42, 0xff);
            if (x.u8(0x42) as i8) < 0 {
                x.add8(0x4, 2);
                source.set_u8(x.b, 0xae, 1);
            } else {
                x.add8(0x43, 0xff);
                if (x.u8(0x43) as i8) < 0 {
                    x.set8(0x43, 3);
                    let d = x.u8(0x2d).wrapping_add(1) & 0x1f;
                    x.set8(0x2d, d);
                }
                x.move_by_table_xy();
            }
        }
        _ => {}
    }
    blink_by_parity(x);
    x.reinsert();
}
