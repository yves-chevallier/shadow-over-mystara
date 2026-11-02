use super::*;
use crate::data::RomAddr;
use crate::families::shell::vars;
use crate::families::shell::records::{Host, view};

const ANIMS: RomAddr = RomAddr(0x16f58c);
const SOUND_FLYING: crate::sound::Sound = crate::sound::CMD_32F;
const SOUND_LANDING: crate::sound::Sound = crate::sound::CMD_330;
const FLY: u8 = 0;
const FALL: u8 = 2;
const END: u8 = 4;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    guarded(x, state)
}

fn state<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => init(x),
        2 => match x.u8(Base::SUB_STATE) {
            FLY => {
                if host_dead(x) {
                    return x.free_eshl();
                }
                if x.u8(Shot32::ARMED) != 0 {
                    x.add8(Base::SUB_STATE, 2);
                    crate::sound::play(x.b, SOUND_FLYING, x.o);
                }
                ai_step(x);
            }
            FALL => {
                if host_dead(x) {
                    return x.free_eshl();
                }
                x.move_xyz();
                if (x.u16(Obj::Z) as i16) > 0 {
                    ai_step(x);
                    return;
                }
                x.add8(Base::SUB_STATE, 2);
                x.set16(Obj::Z, 0);
                x.set8(Shot32::DONE, 1);
                let left = vars::CAMERA1_X.get(x.b);
                let x_pos = x.u16(Obj::X);
                let inside = (left.wrapping_sub(0x30) as i16) < x_pos as i16 && (left.wrapping_add(0x1b0) as i16) > x_pos as i16;
                if inside {
                    crate::sound::play(x.b, SOUND_LANDING, x.o);
                }
                ai_start(x, ANIMS.0, 2);
            }
            END => {
                if ai_step(x) {
                    x.add8(Obj::STATE, 2);
                }
            }
            _ => {}
        },
        _ => x.free_eshl(),
    }
}

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Obj::STATE, 2);
    x.set8(Base::BOX_SET, 7);
    x.set8(Shot::ATTACK_POWER, 0x14);
    crate::placement::load_effect_palette(x.b, crate::placement::effect_palette::LIGHT);
    let variant = x.u8(Obj::VARIANT);
    let (target, angle);
    if variant == 0 {
        target = x.linked(Shot::OWNER);
        angle = crate::aids::direction_to(x.b, x.o, target);
    } else {
        target = x.linked(Shot32::TARGET);
        let to = (target.u16(x.b, Host::TARGET_X), target.u16(x.b, Host::TARGET_Y));
        angle = crate::aids::direction_from(x.b, x.o, to);
    }
    x.set8(Obj::SPEED_INDEX, angle);
    speed_table(x, 0x41);
    // horizontal distance to the target
    let reference = if variant == 0 { target.u32(x.b, Obj::X) } else { target.u32(x.b, Host::TARGET_X) };
    let mut distance = x.u32(Obj::X).wrapping_sub(reference);
    if (distance as i32) < 0 {
        distance = distance.wrapping_neg();
    }
    distance &= 0x7fff_0000;
    // number of 9-pixel steps to cover it
    let step = 0x0009_0000u32;
    let mut steps: u32 = 0;
    let mut rest = distance as i32;
    loop {
        rest = rest.wrapping_sub(step as i32);
        if rest <= 0 {
            break;
        }
        steps = (steps + 1) & 0xffff;
    }
    let quotient = crate::aids::divide(x.u16(Obj::Z) as u32, steps).0;
    let slope = (quotient as i32).wrapping_neg() as u32;
    let slope = (slope.rotate_left(16) as i32 >> 8) as u16;
    x.set16(Obj::VZ, slope);
    ai_start(x, ANIMS.0, 0);
}

view! {
    /// Fields of shot 32.
    Shot32 {
        /// Target (16-bit pointer).
        target, set_target, TARGET: u16 @ 0x42;
        /// Non-zero: armed (h).
        armed, set_armed, ARMED: u8 @ 0x58;
        /// Set to 1 at its end (h).
        done, set_done, DONE: u8 @ 0x5a;
    }
}
