use super::*;
use crate::data::RomAddr;
use crate::families::shell::records::{Host, view};
use crate::families::shell::tables::{tables, Read};

const ANIMS: RomAddr = RomAddr(0x16ff22);
const SOUND_IMPACT: crate::sound::Sound = crate::sound::CMD_327_427_C;
const FLY: u8 = 0;
const SETTLE: u8 = 2;
const BURN: u8 = 4;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(Shot39::SOURCE) == 0 {
        return kill(x);
    }
    match x.u8(Obj::STATE) {
        0 => {
            x.add8(Obj::STATE, 2);
            x.set8(Base::BOX_SET, 7);
            x.set8(Shot::ATTACK_POWER, 5);
            x.set8(Shot::ATTACK_ID, 0x1d);
            x.set8(Shot39::STEP_TIMER, 4);
            launch(x);
            speed_table(x, 0x13);
        }
        2 => active(x),
        _ => x.free_eshl(),
    }
}

fn launch<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(Shot39::SOURCE) == 0 {
        return;
    }
    let source = x.linked(Shot39::SOURCE);
    copy_position32(x, source);
    let flip = source.u8(x.b, Base::FLIP);
    x.set8(Base::FLIP, flip);
    x.add16(Obj::Y, 0xffff);
    x.add16(Obj::Z, 0x100);
    x.set16(Obj::VZ, -0x500i16 as u16);
    x.set16(Obj::AZ, -0x38i16 as u16);
    let variant = x.u8(Obj::VARIANT) as u32;
    let direction = tables().shot39_directions.at8(x.b, variant);
    x.set8(Obj::SPEED_INDEX, direction);
    let anim = tables().shot39_start_anims.at8(x.b, variant) as u32;
    x.anim_start(ANIMS.0, anim as u16);
}

/// Cycles the 8 flame frames every 4 frames.
fn spin<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Shot39::STEP_TIMER, 0xff);
    if (x.u8(Shot39::STEP_TIMER) as i8) >= 0 {
        return;
    }
    x.set8(Shot39::STEP_TIMER, 4);
    x.set8(Base::OWN_PALETTE, 1);
    let n = x.u8(Shot39::FRAME_STEP).wrapping_add(1) & 7;
    x.set8(Shot39::FRAME_STEP, n);
    let frame = tables().shot39_frames.at8(x.b, n as u32);
    x.set8(Base::PALETTE, frame);
}

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    spin(x);
    let source = x.linked(Shot39::SOURCE);
    if source.u16(x.b, Obj::STATE) == 0x406 {
        return x.free_eshl();
    }
    match x.u8(Base::SUB_STATE) {
        FLY => {
            x.move_by_table_xy();
            x.move_z();
            if (x.u16(Obj::Z) as i16) <= 0 {
                crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::B, 0x30);
                crate::sound::play(x.b, SOUND_IMPACT, x.o);
                x.add8(Base::SUB_STATE, 2);
                x.set16(Obj::Z, 0);
                x.set8(Shot39::TIMER, 0x30);
            }
            x.anim_step();
        }
        SETTLE => {
            x.add8(Shot39::TIMER, 0xff);
            if (x.u8(Shot39::TIMER) as i8) < 0 {
                x.add8(Base::SUB_STATE, 2);
                x.set8(Shot39::TIMER, 0x1e);
                let d = x.u8(Obj::SPEED_INDEX).wrapping_sub(6) & 0x1f;
                x.set8(Obj::SPEED_INDEX, d);
                x.set8(Shot39::BLINK, 3);
            }
            x.anim_step();
        }
        BURN => {
            x.anim_step();
            if (x.u8(Shot39::TIMER) as i8) < 0x14 {
                x.set8(Base::ATTACK_BOX, 0);
            }
            x.add8(Shot39::TIMER, 0xff);
            if (x.u8(Shot39::TIMER) as i8) < 0 {
                x.add8(Obj::STATE, 2);
                source.set_u8(x.b, Host::SHOT_DONE, 1);
            } else {
                x.add8(Shot39::BLINK, 0xff);
                if (x.u8(Shot39::BLINK) as i8) < 0 {
                    x.set8(Shot39::BLINK, 3);
                    let d = x.u8(Obj::SPEED_INDEX).wrapping_add(1) & 0x1f;
                    x.set8(Obj::SPEED_INDEX, d);
                }
                x.move_by_table_xy();
            }
        }
        _ => {}
    }
    blink_by_parity(x);
    x.reinsert();
}

view! {
    /// Fields of shot 39.
    Shot39 {
        /// Countdown (h).
        timer, set_timer, TIMER: u8 @ 0x42;
        /// Blink countdown (h).
        blink, set_blink, BLINK: u8 @ 0x43;
        /// Countdown of a frame step (h).
        step_timer, set_step_timer, STEP_TIMER: u8 @ 0x46;
        /// Step of the frame cycle (`shot39_frames`).
        frame_step, set_frame_step, FRAME_STEP: u8 @ 0x47;
        /// Source object (16-bit pointer).
        source, set_source, SOURCE: u16 @ 0x5c;
    }
}
