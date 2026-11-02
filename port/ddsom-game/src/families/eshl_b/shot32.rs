use super::*;

const ANIMS: u32 = 0x16f58c;
const SOUND_FLYING: crate::sound::Sound = crate::sound::CMD_32F;
const SOUND_LANDING: crate::sound::Sound = crate::sound::CMD_330;
const SCREEN_LEFT: u32 = 0x5a6;
const FLY: u8 = 0;
const FALL: u8 = 2;
const END: u8 = 4;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    guarded(x, state)
}

fn state<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => init(x),
        2 => match x.u8(0x5) {
            FLY => {
                if host_dead(x) {
                    return x.free_eshl();
                }
                if x.u8(0x58) != 0 {
                    x.add8(0x5, 2);
                    crate::sound::play(x.b, SOUND_FLYING, x.o);
                }
                ai_step(x);
            }
            FALL => {
                if host_dead(x) {
                    return x.free_eshl();
                }
                x.move_xyz();
                if (x.u16(0xc) as i16) > 0 {
                    ai_step(x);
                    return;
                }
                x.add8(0x5, 2);
                x.set16(0xc, 0);
                x.set8(0x5a, 1);
                let left = x.a5_u16(SCREEN_LEFT);
                let x_pos = x.u16(0x8);
                let inside = (left.wrapping_sub(0x30) as i16) < x_pos as i16 && (left.wrapping_add(0x1b0) as i16) > x_pos as i16;
                if inside {
                    crate::sound::play(x.b, SOUND_LANDING, x.o);
                }
                ai_start(x, ANIMS, 2);
            }
            END => {
                if ai_step(x) {
                    x.add8(0x4, 2);
                }
            }
            _ => {}
        },
        _ => x.free_eshl(),
    }
}

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x4, 2);
    x.set8(f::BOX_SET, 7);
    x.set8(f::ATTACK_POWER, 0x14);
    crate::placement::load_effect_palette(x.b, crate::placement::effect_palette::LIGHT);
    let variant = x.u8(0x3);
    let (target, angle);
    if variant == 0 {
        target = x.linked(f::OWNER);
        angle = crate::aids::direction_to(x.b, x.o, target);
    } else {
        target = x.linked(0x42);
        let to = (target.u16(x.b, 0xbc), target.u16(x.b, 0xbe));
        angle = crate::aids::direction_from(x.b, x.o, to);
    }
    x.set8(0x2d, angle);
    speed_table(x, 0x41);
    // horizontal distance to the target
    let reference = if variant == 0 { target.u32(x.b, 0x8) } else { target.u32(x.b, 0xbc) };
    let mut distance = x.u32(0x8).wrapping_sub(reference);
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
    let quotient = crate::aids::divide(x.u16(0xc) as u32, steps).0;
    let slope = (quotient as i32).wrapping_neg() as u32;
    let slope = (slope.rotate_left(16) as i32 >> 8) as u16;
    x.set16(0x18, slope);
    ai_start(x, ANIMS, 0);
}
