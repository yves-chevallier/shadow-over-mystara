use super::*;
use crate::families::shell::tables::{tables, Read};

const ANIMS: u32 = 0x16fe3e;
const FALL: u8 = 0;
const BLINK: u8 = 2;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(0x5c) == 0 {
        return kill(x);
    }
    match x.u8(0x4) {
        0 => {
            x.add8(0x4, 2);
            x.set8(f::BOX_SET, 7);
            x.set8(f::ATTACK_POWER, 5);
            x.set8(f::ATTACK_ID, 0x17);
            x.set8(0x50, 0x14);
            launch(x);
            place(x);
        }
        2 => active(x),
        _ => x.free_eshl(),
    }
}

fn launch<R: Bench>(x: &mut Cx<'_, R>) {
    let rng = x.random();
    let r = rng as u32 & 0xf;
    let vz = tables().shot37_speeds.at16(x.b, 2 * r);
    x.set16(0x18, vz);
    let rng = x.random();
    let r = rng as u32 & 7;
    let anim = tables().shot37_start_anims.at8(x.b, r) as u32;
    x.anim_start(ANIMS, anim as u16);
}

fn place<R: Bench>(x: &mut Cx<'_, R>) {
    let rng = x.random();
    let r = rng as u32 & 0xf;
    let dx = tables().shot37_x_offsets.at16(x.b, 2 * r);
    let rng = x.random();
    let r = rng as u32 & 0xf;
    let dy = tables().shot37_y_offsets.at16(x.b, 2 * r);
    let px = dx.wrapping_add(x.a5_u16(0x506));
    let py = dy.wrapping_add(x.a5_u16(0x50a));
    x.set16(0x8, px);
    x.set16(0x10, py);
    x.set16(0xc, 0x100);
}

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    let source = x.linked(0x5c);
    if (source.u8(x.b, 0x4) as i8) >= 4 {
        return x.free_eshl();
    }
    match x.u8(0x5) {
        FALL => {
            x.move_z();
            if (x.u16(0xc) as i16) > 0 {
                x.anim_step();
            } else {
                x.add8(0x5, 2);
                x.set16(0xc, 0);
                x.set8(0x42, 0x19);
                *x.c.d2() &= 0xffff_0000;
                *x.c.d3() &= 0xffff_0000;
                let impact = crate::effects_aids::shock(x.b, x.o, 0, 0x201, 0, 0);
                // Original glitch: the debris variant is the D1.b that the shock (0xb10c8) leaves: the high byte
                // of its setting, or the low byte when the shock is not created (scene 41, pool full).
                let variant = if impact.is_some() { 2 } else { 1 };
                for _ in 0..2 {
                    *x.c.d2() = 1;
                    crate::families::effects_creators::debris_rain(x.b, x.o, variant, 1);
                }
            }
        }
        BLINK => {
            x.add8(0x42, 0xff);
            if (x.u8(0x42) as i8) < 0 {
                x.add8(0x4, 2);
                x.set_bit(0x1, 1, true);
                x.set8(0x0, 2);
            } else {
                let hidden = x.u8(0x42) & 3 != 0;
                x.set_bit(0x1, 1, hidden);
            }
        }
        _ => {}
    }
    x.reinsert();
}
