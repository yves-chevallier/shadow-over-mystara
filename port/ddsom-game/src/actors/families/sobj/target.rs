use crate::world::objects::player::Player;
use super::*;
use crate::world::ram;
use crate::actors::families::shell::records::{Base, view};
use crate::actors::families::shell::tables::{tables, Read};

const HITS_TO_BREAK: u8 = 3;
const SOUND: crate::services::sound::Sound = crate::services::sound::CMD_135_235;
const SHOOTER_CODE: u32 = 0xff_2ab2;
const FLAG_A: u32 = 0xff_2ac0;
const FLAG_B: u32 = 0xff_2ac1;
const VARIANT_A: u8 = 0x25;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => {
            x.add8(Obj::STATE, 2);
            x.set8(Target::HITS, 0);
            let row = if x.u8(Obj::VARIANT) == VARIANT_A { 0 } else { 1 };
            let rect = 8 * row;
            for i in 0..4 {
                let v = tables().sobj_target_rects.at16(x.b, rect + 2 * i);
                x.set16(Target::RECT_X + 2 * i, v);
            }
            let v = x.u16(Obj::X);
            x.set16(Base::X_FRACTION, v);
            let v = x.u16(Obj::Y);
            x.set16(Base::Y_FRACTION, v);
            let variant = x.u8(Obj::VARIANT) as u32;
            x.anim_start(0x1c40c6, variant as u16);
            x.reinsert();
        }
        2 => {
            follow_camera2(x);
            let y = x.u16(Base::Y_FRACTION).wrapping_sub(cam2_dy(x));
            x.set16(Obj::Y, y);
            x.set8(Target::STRUCK, 0);
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
    ram::CAMERA_ALT_X.get(x.b).wrapping_sub(ram::CAMERA_X.get(x.b))
}

fn cam2_dy<R: Bench>(x: &mut Cx<'_, R>) -> u16 {
    ram::CAMERA_ALT_Y.get(x.b).wrapping_sub(ram::CAMERA_Y.get(x.b))
}

fn follow_camera2<R: Bench>(x: &mut Cx<'_, R>) {
    let v = x.u16(Base::X_FRACTION).wrapping_sub(cam2_dx(x));
    x.set16(Obj::X, v);
}

/// Is player `p` inside the rectangle? If so, `+0x15` = 1; an attack in progress
/// (`+0x58` > 0) counts as a hit.
fn probe<R: Bench>(x: &mut Cx<'_, R>, p: Obj) {
    let dx = x.u16(Obj::X).wrapping_sub(p.u16(x.b, Obj::X)).wrapping_add(x.u16(Target::RECT_X));
    if dx > x.u16(Target::RECT_WIDTH) {
        return;
    }
    let dy = x
        .u16(Obj::Z)
        .wrapping_add(x.u16(Obj::Y))
        .wrapping_sub(p.u16(x.b, Obj::Z))
        .wrapping_sub(p.u16(x.b, Obj::Y))
        .wrapping_add(x.u16(Target::RECT_Y));
    if dy > x.u16(Target::RECT_HEIGHT) {
        return;
    }
    x.set8(Target::STRUCK, 1);
    if x.u8(Base::SUB_STEP) == 0 {
        let code = x.u16(Target::CODE);
        if code != 0 {
            wr16(x.b, SHOOTER_CODE, code);
            x.set8(Base::SUB_STEP, 1);
        }
    }
    if (p.u8(x.b, Player::STRIKE) as i8) > 0 {
        hit(x, p);
    }
}

fn hit<R: Bench>(x: &mut Cx<'_, R>, p: Obj) {
    p.set_u8(x.b, Player::STRIKE, 0xff);
    let n = x.u8(Target::HITS) + 1;
    x.set8(Target::HITS, n);
    if n < HITS_TO_BREAK {
        return;
    }
    crate::services::sound::play(x.b, SOUND, x.o);
    let flag = if x.u8(Obj::VARIANT) == VARIANT_A { FLAG_A } else { FLAG_B };
    wr8(x.b, flag, 1);
    x.add8(Obj::STATE, 2);
}

view! {
    /// Fields of the target to hit (type 15).
    Target {
        /// Hit rectangle: offset X (`sobj_target_rects`).
        rect_x, set_rect_x, RECT_X: u16 @ 0x16;
        /// Hit rectangle: width.
        rect_width, set_rect_width, RECT_WIDTH: u16 @ 0x18;
        /// Hit rectangle: offset Y.
        rect_y, set_rect_y, RECT_Y: u16 @ 0x1a;
        /// Hit rectangle: height.
        rect_height, set_rect_height, RECT_HEIGHT: u16 @ 0x1c;
        /// Event code of the target (h).
        code, set_code, CODE: u16 @ 0x1e;
        /// Hits received.
        hits, set_hits, HITS: u8 @ 0x14;
        /// Set to 1 when struck (h).
        struck, set_struck, STRUCK: u8 @ 0x15;
    }
}
