use super::*;
use crate::world::data::RomAddr;
use crate::world::ram;
use crate::actors::families::shell::records::view;
use crate::actors::families::shell::tables::{tables, Read};

const ANIMS: RomAddr = RomAddr(0x16fe3e);
const FALL: u8 = 0;
const BLINK: u8 = 2;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(Shot37::SOURCE) == 0 {
        return kill(x);
    }
    match x.u8(Obj::STATE) {
        0 => {
            x.add8(Obj::STATE, 2);
            x.set8(Base::BOX_SET, 7);
            x.set8(Shot::ATTACK_POWER, 5);
            x.set8(Shot::ATTACK_ID, 0x17);
            x.set8(Shot::OWNER, 0x14);
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
    x.set16(Obj::VZ, vz);
    let rng = x.random();
    let r = rng as u32 & 7;
    let anim = tables().shot37_start_anims.at8(x.b, r) as u32;
    x.anim_start(ANIMS.0, anim as u16);
}

fn place<R: Bench>(x: &mut Cx<'_, R>) {
    let rng = x.random();
    let r = rng as u32 & 0xf;
    let dx = tables().shot37_x_offsets.at16(x.b, 2 * r);
    let rng = x.random();
    let r = rng as u32 & 0xf;
    let dy = tables().shot37_y_offsets.at16(x.b, 2 * r);
    let px = dx.wrapping_add(ram::CAMERA_X.get(x.b));
    let py = dy.wrapping_add(ram::CAMERA_Y.get(x.b));
    x.set16(Obj::X, px);
    x.set16(Obj::Y, py);
    x.set16(Obj::Z, 0x100);
}

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    let source = x.linked(Shot37::SOURCE);
    if (source.u8(x.b, Obj::STATE) as i8) >= 4 {
        return x.free_eshl();
    }
    match x.u8(Base::SUB_STATE) {
        FALL => {
            x.move_z();
            if (x.u16(Obj::Z) as i16) > 0 {
                x.anim_step();
            } else {
                x.add8(Base::SUB_STATE, 2);
                x.set16(Obj::Z, 0);
                x.set8(Shot37::TIMER, 0x19);
                *x.c.d2() &= 0xffff_0000;
                *x.c.d3() &= 0xffff_0000;
                let impact = crate::services::effects_aids::shock(x.b, x.o, 0, 0x201, 0, 0);
                // Original glitch: the debris variant is the D1.b that the shock (0xb10c8) leaves: the high byte
                // of its setting, or the low byte when the shock is not created (scene 41, pool full).
                let variant = if impact.is_some() { 2 } else { 1 };
                for _ in 0..2 {
                    *x.c.d2() = 1;
                    crate::actors::families::effects_creators::debris_rain(x.b, x.o, variant, 1);
                }
            }
        }
        BLINK => {
            x.add8(Shot37::TIMER, 0xff);
            if (x.u8(Shot37::TIMER) as i8) < 0 {
                x.add8(Obj::STATE, 2);
                x.set_bit(Obj::FLAGS, 1, true);
                x.set8(Obj::ACTIVE, 2);
            } else {
                let hidden = x.u8(Shot37::TIMER) & 3 != 0;
                x.set_bit(Obj::FLAGS, 1, hidden);
            }
        }
        _ => {}
    }
    x.reinsert();
}

view! {
    /// Fields of shot 37.
    Shot37 {
        /// Countdown (h).
        timer, set_timer, TIMER: u8 @ 0x42;
        /// Source object (16-bit pointer).
        source, set_source, SOURCE: u16 @ 0x5c;
    }
}
