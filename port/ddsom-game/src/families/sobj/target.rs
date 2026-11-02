use super::*;
use crate::families::shell::tables::{tables, Read};

const HITS_TO_BREAK: u8 = 3;
const SOUND: crate::sound::Sound = crate::sound::CMD_135_235;
const SHOOTER_CODE: u32 = 0xff_2ab2;
const FLAG_A: u32 = 0xff_2ac0;
const FLAG_B: u32 = 0xff_2ac1;
const VARIANT_A: u8 = 0x25;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => {
            x.add8(0x4, 2);
            x.set8(0x14, 0);
            let row = if x.u8(0x3) == VARIANT_A { 0 } else { 1 };
            let rect = 8 * row;
            for i in 0..4 {
                let v = tables().sobj_target_rects.at16(x.b, rect + 2 * i);
                x.set16(0x16 + 2 * i, v);
            }
            let v = x.u16(0x8);
            x.set16(0xa, v);
            let v = x.u16(0x10);
            x.set16(0x12, v);
            let variant = x.u8(0x3) as u32;
            x.anim_start(0x1c40c6, variant as u16);
            x.reinsert();
        }
        2 => {
            follow_camera2(x);
            let y = x.u16(0x12).wrapping_sub(cam2_dy(x));
            x.set16(0x10, y);
            x.set8(0x15, 0);
            for i in 0..4 {
                let who = rd16(x.b, PLAYERS + 2 * i);
                if who != 0 {
                    probe(x, Obj(who as i16 as i32 as u32 & 0xff_ffff));
                }
            }
            follow_camera2(x);
        }
        4 => follow_camera2(x),
        _ => x.free(&pools::SOBJ),
    }
}

fn cam2_dx<R: Bench>(x: &mut Cx<'_, R>) -> u16 {
    x.a5_u16(ram::CAMERA2_X).wrapping_sub(x.a5_u16(ram::CAMERA_X))
}

fn cam2_dy<R: Bench>(x: &mut Cx<'_, R>) -> u16 {
    x.a5_u16(ram::CAMERA2_Y).wrapping_sub(x.a5_u16(ram::CAMERA_Y))
}

fn follow_camera2<R: Bench>(x: &mut Cx<'_, R>) {
    let v = x.u16(0xa).wrapping_sub(cam2_dx(x));
    x.set16(0x8, v);
}

/// Is player `p` inside the rectangle? If so, `+0x15` = 1; an attack in progress
/// (`+0x58` > 0) compte un coup.
fn probe<R: Bench>(x: &mut Cx<'_, R>, p: Obj) {
    let dx = x.u16(0x8).wrapping_sub(p.u16(x.b, 0x8)).wrapping_add(x.u16(0x16));
    if dx > x.u16(0x18) {
        return;
    }
    let dy = x
        .u16(0xc)
        .wrapping_add(x.u16(0x10))
        .wrapping_sub(p.u16(x.b, 0xc))
        .wrapping_sub(p.u16(x.b, 0x10))
        .wrapping_add(x.u16(0x1a));
    if dy > x.u16(0x1c) {
        return;
    }
    x.set8(0x15, 1);
    if x.u8(0x7) == 0 {
        let code = x.u16(0x1e);
        if code != 0 {
            wr16(x.b, SHOOTER_CODE, code);
            x.set8(0x7, 1);
        }
    }
    if (p.u8(x.b, 0x58) as i8) > 0 {
        hit(x, p);
    }
}

fn hit<R: Bench>(x: &mut Cx<'_, R>, p: Obj) {
    p.set_u8(x.b, 0x58, 0xff);
    let n = x.u8(0x14) + 1;
    x.set8(0x14, n);
    if n < HITS_TO_BREAK {
        return;
    }
    crate::sound::play(x.b, SOUND, x.o);
    let flag = if x.u8(0x3) == VARIANT_A { FLAG_A } else { FLAG_B };
    wr8(x.b, flag, 1);
    x.add8(0x4, 2);
}
