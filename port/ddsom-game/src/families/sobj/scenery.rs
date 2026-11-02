use super::*;

/// Variante -> routine d'initialisation (13 routines).
const INIT_KIND: u32 = 0xdd6a4;
/// Variant -> follow routine (23 routines).
const STEP_KIND: u32 = 0xdd8ea;
const WAIT: u8 = 0;
const FOLLOW: u8 = 2;
const GONE: u8 = 4;
const FREE: u8 = 6;
/// End of the animation script (high byte of the flags).
const ANIM_END: u8 = 0xfe;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    match x.u8(0x4) {
        WAIT => init(x),
        FOLLOW => follow(x),
        GONE => x.add8(0x4, 2),
        FREE => x.free(&pools::SOBJ),
        _ => {}
    }
}

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x4, 2);
    x.set16(0x14, 0);
    x.set8(0x42, 0);
    let variant = x.u8(0x3) as u32;
    x.anim_start(ANIMS, variant as u16);
    match x.rom8(INIT_KIND + variant) {
        0 => {}
        1 => {
            let v = x.u16(0x8);
            x.set16(0xa, v);
            let v = x.u16(0x10);
            x.set16(0x12, v);
        }
        2 => {
            x.set8(0x42, 1);
            let v = x.a5_u32(ram::CAMERA_Y);
            x.set_a5_u32(0x50e, v);
        }
        3 => {
            x.set8(0x2d, 0);
            x.select_speed(&crate::objects::motion::SPEEDS_B, 0xd);
        }
        4 => {
            let v = x.a5_u16(ram::CAMERA2_X);
            x.set16(0xa, v);
            let v = x.a5_u16(ram::CAMERA2_Y);
            x.set16(0xe, v);
            x.add16(0x12, 0xd8);
            x.add16(0xe, 0x190);
        }
        5 => place_at(x, 0, 0x2c0),
        6 => x.set8(0x1d, 0xe),
        7 => x.set8(0x3f, 2),
        8 => place_at(x, 0, 0x2f0),
        9 => place_at(x, 0x100, 0x2f0),
        10 => place_at(x, 0x100, 0x2d0),
        11 => {
            x.set16(0x14, 0);
            x.set16(0x1c, 0x200);
        }
        _ => x.set8(0x42, 1),
    }
    reinsert_by_layer(x);
}

/// Positions the scenery (X, Y) and lifts it (Z = -0x100), in the foreground.
fn place_at<R: Bench>(x: &mut Cx<'_, R>, px: u16, py: u16) {
    x.set16(0x8, px);
    x.set16(0x10, py);
    x.set16(0xc, 0xff00);
    x.set8(0x42, 1);
}

fn follow<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(0x2f) == 0 {
        x.anim_step();
    }
    let variant = x.u8(0x3) as u32;
    let (dx, dy) = camera_delta(x);
    match x.rom8(STEP_KIND + variant) {
        0 => cull_beyond(x, 0x60),
        1 => {
            let d = x.a5_u16(ram::CAMERA2_X).wrapping_sub(x.a5_u16(ram::CAMERA_X));
            let v = x.u16(0xa).wrapping_sub(d);
            x.set16(0x8, v);
            let d = x.a5_u16(ram::CAMERA2_Y).wrapping_sub(x.a5_u16(ram::CAMERA_Y));
            let v = x.u16(0x12).wrapping_sub(d);
            x.set16(0x10, v);
            cull_beyond(x, 0x60);
        }
        2 => {
            let v = x.a5_u16(0x5e6);
            x.set16(0x8, v);
            let v = x.a5_u32(0x5ee).wrapping_add(1);
            x.set32(0x10, v);
            let n = x.u8(0x1d).wrapping_sub(1);
            x.set8(0x1d, n);
            if (n as i8) < 0 {
                x.set8(0x1d, 0xe);
                let v = x.u8(0xd) ^ 1;
                x.set8(0xd, v);
            }
            cull_beyond(x, 0x60);
            reinsert_by_layer(x);
        }
        16 => {
            let v = x.a5_u16(0x5e6);
            x.set16(0x8, v);
            let v = x.a5_u32(0x5ee).wrapping_add(1);
            x.set32(0x10, v);
            cull_beyond(x, 0x60);
            reinsert_by_layer(x);
        }
        3 => {
            x.add32(0x8, dx);
            x.add32(0x10, dy);
            reinsert_by_layer(x);
        }
        7 => {
            let d = x.a5_u32(ram::PREV_CAMERA2_X).wrapping_sub(x.a5_u32(ram::CAMERA2_X));
            x.add32(0x8, 0u32.wrapping_sub(d));
            cull_beyond(x, 0x60);
            reinsert_by_layer(x);
        }
        4 => {
            let d2 = dy.wrapping_sub(asr(dy, 1));
            x.add32(0x8, 0u32.wrapping_sub(dx));
            x.add32(0xc, 0u32.wrapping_sub(d2));
            reinsert_by_layer(x);
        }
        5 => {
            let d0 = dx.wrapping_sub(asr(dx, 3));
            let d1 = asr(dy, 3).wrapping_sub(dy);
            x.add32(0x8, d0);
            x.add32(0xc, 0u32.wrapping_sub(d1));
            reinsert_by_layer(x);
        }
        10 => {
            x.add32(0x8, asr(dx, 2));
            x.add32(0xc, asr(dy, 2));
            reinsert_by_layer(x);
        }
        11 => {
            let d = x.a5_u32(ram::CAMERA2_X).wrapping_sub(x.a5_u32(ram::PREV_CAMERA2_X));
            x.add32(0x8, 0u32.wrapping_sub(d));
            reinsert_by_layer(x);
        }
        6 => reinsert_by_layer(x),
        8 => wait_script_end(x, 2),
        9 => {
            x.move_xy();
            if x.u16(0x10) > 0x240 {
                x.add8(0x4, 2);
            }
        }
        12..=15 => wait_script_end(x, 4),
        17 => {
            let mut d0 = asr(dx, 1);
            let mut d2 = asr(dy, 1);
            if x.u8(0x3) == 0xa {
                d0 = d0.wrapping_add(asr(d0, 2));
                d2 = d2.wrapping_add(asr(d2, 2));
            }
            x.add32(0x8, d0);
            x.add32(0xc, d2);
        }
        18 => {
            x.move_y();
            if x.u8(0x1) & 1 == 0 {
                x.add8(0x4, 2);
            }
        }
        20 => {
            let limit = x.a5_u16(ram::CAMERA_X).wrapping_sub(0x40) as i16;
            if limit > x.u16(0x8) as i16 {
                x.add8(0x4, 2);
            }
        }
        21 => {
            if x.a5_u8(0x42) == 0 {
                x.anim_start(ANIMS, variant as u16);
            } else {
                wait_script_end(x, 2)
            }
        }
        22 => {
            if x.a5_u8(0x42) != 0 {
                x.move_xy();
                if x.u16(0x10) > 0x240 {
                    x.add8(0x4, 2);
                }
            }
        }
        _ => {}
    }
}

/// Goes to the next state (+`step`) when the animation is finished.
fn wait_script_end<R: Bench>(x: &mut Cx<'_, R>, step: u8) {
    if x.u8(0x32) == ANIM_END {
        x.add8(0x4, step);
    }
}
