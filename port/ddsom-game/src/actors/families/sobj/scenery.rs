use super::*;
use crate::actors::families::shell::vars;
use crate::world::ram;
use crate::actors::families::shell::records::{Base, view};
use crate::actors::families::shell::tables::{tables, Read};

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
    match x.u8(Obj::STATE) {
        WAIT => init(x),
        FOLLOW => follow(x),
        GONE => x.add8(Obj::STATE, 2),
        FREE => x.free(&pools::SOBJ),
        _ => {}
    }
}

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Obj::STATE, 2);
    x.set16(Obj::VX, 0);
    x.set8(Scenery::FOREGROUND, 0);
    let variant = x.u8(Obj::VARIANT) as u32;
    x.anim_start(ANIMS.0, variant as u16);
    match tables().sobj_scenery_init_kind.at8(x.b, variant) {
        0 => {}
        1 => {
            let v = x.u16(Obj::X);
            x.set16(Base::X_FRACTION, v);
            let v = x.u16(Obj::Y);
            x.set16(Base::Y_FRACTION, v);
        }
        2 => {
            x.set8(Scenery::FOREGROUND, 1);
            let v = vars::CAMERA_Y_LONG.get(x.b);
            vars::CAMERA_TARGET_X.set(x.b, v);
        }
        3 => {
            x.set8(Obj::SPEED_INDEX, 0);
            x.select_speed(&crate::world::objects::motion::SPEEDS_B, 0xd);
        }
        4 => {
            let v = ram::CAMERA_ALT_X.get(x.b);
            x.set16(Base::X_FRACTION, v);
            let v = ram::CAMERA_ALT_Y.get(x.b);
            x.set16(Base::Z_FRACTION, v);
            x.add16(Base::Y_FRACTION, 0xd8);
            x.add16(Base::Z_FRACTION, 0x190);
        }
        5 => place_at(x, 0, 0x2c0),
        6 => x.set8(Scenery::BLINK, 0xe),
        7 => x.set8(Scenery::VISIBILITY, 2),
        8 => place_at(x, 0, 0x2f0),
        9 => place_at(x, 0x100, 0x2f0),
        10 => place_at(x, 0x100, 0x2d0),
        11 => {
            x.set16(Obj::VX, 0);
            x.set16(Obj::VY, 0x200);
        }
        _ => x.set8(Scenery::FOREGROUND, 1),
    }
    reinsert_by_layer(x);
}

/// Positions the scenery (X, Y) and lifts it (Z = -0x100), in the foreground.
fn place_at<R: Bench>(x: &mut Cx<'_, R>, px: u16, py: u16) {
    x.set16(Obj::X, px);
    x.set16(Obj::Y, py);
    x.set16(Obj::Z, 0xff00);
    x.set8(Scenery::FOREGROUND, 1);
}

fn follow<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Base::SETTING) == 0 {
        x.anim_step();
    }
    let variant = x.u8(Obj::VARIANT) as u32;
    let (dx, dy) = camera_delta(x);
    match tables().sobj_scenery_step_kind.at8(x.b, variant) {
        0 => cull_beyond(x, 0x60),
        1 => {
            let d = ram::CAMERA_ALT_X.get(x.b).wrapping_sub(ram::CAMERA_X.get(x.b));
            let v = x.u16(Base::X_FRACTION).wrapping_sub(d);
            x.set16(Obj::X, v);
            let d = ram::CAMERA_ALT_Y.get(x.b).wrapping_sub(ram::CAMERA_Y.get(x.b));
            let v = x.u16(Base::Y_FRACTION).wrapping_sub(d);
            x.set16(Obj::Y, v);
            cull_beyond(x, 0x60);
        }
        2 => {
            let v = crate::world::ram::PLAYERS.nth(0).u16(x.b, Obj::X);
            x.set16(Obj::X, v);
            let v = crate::world::ram::PLAYERS.nth(0).u32(x.b, Obj::Y).wrapping_add(1);
            x.set32(Obj::Y, v);
            let n = x.u8(Scenery::BLINK).wrapping_sub(1);
            x.set8(Scenery::BLINK, n);
            if (n as i8) < 0 {
                x.set8(Scenery::BLINK, 0xe);
                let v = x.u8(Scenery::PHASE) ^ 1;
                x.set8(Scenery::PHASE, v);
            }
            cull_beyond(x, 0x60);
            reinsert_by_layer(x);
        }
        16 => {
            let v = crate::world::ram::PLAYERS.nth(0).u16(x.b, Obj::X);
            x.set16(Obj::X, v);
            let v = crate::world::ram::PLAYERS.nth(0).u32(x.b, Obj::Y).wrapping_add(1);
            x.set32(Obj::Y, v);
            cull_beyond(x, 0x60);
            reinsert_by_layer(x);
        }
        3 => {
            x.add32(Obj::X, dx);
            x.add32(Obj::Y, dy);
            reinsert_by_layer(x);
        }
        7 => {
            let d = vars::PREV_CAMERA_ALT_X.get(x.b).wrapping_sub(vars::CAMERA_ALT_X_LONG.get(x.b));
            x.add32(Obj::X, 0u32.wrapping_sub(d));
            cull_beyond(x, 0x60);
            reinsert_by_layer(x);
        }
        4 => {
            let d2 = dy.wrapping_sub(asr(dy, 1));
            x.add32(Obj::X, 0u32.wrapping_sub(dx));
            x.add32(Obj::Z, 0u32.wrapping_sub(d2));
            reinsert_by_layer(x);
        }
        5 => {
            let d0 = dx.wrapping_sub(asr(dx, 3));
            let d1 = asr(dy, 3).wrapping_sub(dy);
            x.add32(Obj::X, d0);
            x.add32(Obj::Z, 0u32.wrapping_sub(d1));
            reinsert_by_layer(x);
        }
        10 => {
            x.add32(Obj::X, asr(dx, 2));
            x.add32(Obj::Z, asr(dy, 2));
            reinsert_by_layer(x);
        }
        11 => {
            let d = vars::CAMERA_ALT_X_LONG.get(x.b).wrapping_sub(vars::PREV_CAMERA_ALT_X.get(x.b));
            x.add32(Obj::X, 0u32.wrapping_sub(d));
            reinsert_by_layer(x);
        }
        6 => reinsert_by_layer(x),
        8 => wait_script_end(x, 2),
        9 => {
            x.move_xy();
            if x.u16(Obj::Y) > 0x240 {
                x.add8(Obj::STATE, 2);
            }
        }
        12..=15 => wait_script_end(x, 4),
        17 => {
            let mut d0 = asr(dx, 1);
            let mut d2 = asr(dy, 1);
            if x.u8(Obj::VARIANT) == 0xa {
                d0 = d0.wrapping_add(asr(d0, 2));
                d2 = d2.wrapping_add(asr(d2, 2));
            }
            x.add32(Obj::X, d0);
            x.add32(Obj::Z, d2);
        }
        18 => {
            x.move_y();
            if x.u8(Obj::FLAGS) & 1 == 0 {
                x.add8(Obj::STATE, 2);
            }
        }
        20 => {
            let limit = ram::CAMERA_X.get(x.b).wrapping_sub(0x40) as i16;
            if limit > x.u16(Obj::X) as i16 {
                x.add8(Obj::STATE, 2);
            }
        }
        21 => {
            if vars::FROZEN.get(x.b) == 0 {
                x.anim_start(ANIMS.0, variant as u16);
            } else {
                wait_script_end(x, 2)
            }
        }
        22 => {
            if vars::FROZEN.get(x.b) != 0 {
                x.move_xy();
                if x.u16(Obj::Y) > 0x240 {
                    x.add8(Obj::STATE, 2);
                }
            }
        }
        _ => {}
    }
}

/// Goes to the next state (+`step`) when the animation is finished.
fn wait_script_end<R: Bench>(x: &mut Cx<'_, R>, step: u8) {
    if x.u8(Scenery::ANIM_END_FLAGS) == ANIM_END {
        x.add8(Obj::STATE, step);
    }
}

view! {
    /// Fields of the scenery tied to the cameras (type 0).
    Scenery {
        /// Blink phase (h).
        phase, set_phase, PHASE: u8 @ 0xd;
        /// Blink countdown (h).
        blink, set_blink, BLINK: u8 @ 0x1d;
        /// High byte of the animation flags.
        anim_end, set_anim_end, ANIM_END_FLAGS: u8 @ 0x32;
        /// Visibility setting (h).
        visibility, set_visibility, VISIBILITY: u8 @ 0x3f;
        /// Non-zero: in the foreground list.
        foreground, set_foreground, FOREGROUND: u8 @ 0x42;
    }
}
