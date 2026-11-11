use super::*;
use crate::world::data::RomAddr;
use crate::actors::families::shell::tables::{tables, Read};

const ANIMS: RomAddr = RomAddr(0x16c946);
const LOB: u8 = 0;
const LANDED: u8 = 2;
const BLINK: u8 = 4;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => {
            x.set8(Base::BOX_SET, 7);
            x.set8(Shot::ATTACK_ID, 8);
            x.set8(Shot::ATTACK_POWER, 5);
            x.add8(Obj::STATE, 2);
            if x.u8(Base::SETTING) != 0 {
                burst(x);
            } else {
                lob(x);
            }
            x.anim_start(ANIMS.0, 0);
            x.reinsert();
            body(x);
        }
        2 => body(x),
        _ => x.free_eshl(),
    }
}

/// Thrown by the owner: in front of it, horizontal speed 2.5 pixels per frame.
fn lob<R: Bench>(x: &mut Cx<'_, R>) {
    let owner = x.linked(Shot::OWNER);
    place_in_front(x, owner, 0x40, 0x20);
    let vx = if x.u8(Base::FLIP) != 0 { 0x280 } else { -0x280i16 as u16 };
    x.set16(Obj::VX, vx);
    x.set16(Obj::AX, 0);
    x.set16(Obj::VZ, 0x300);
    x.set16(Obj::AZ, -0x38i16 as u16);
    x.set16(Obj::VY, 0);
    x.set16(Obj::AY, 0);
}

/// Spring-out: position of the owner, speeds of row `+3` of the table.
fn burst<R: Bench>(x: &mut Cx<'_, R>) {
    let owner = x.linked(Shot::OWNER);
    copy_position(x, owner);
    x.add16(Obj::Y, 0xffff);
    x.add16(Obj::Z, 0x32);
    let row = 16 * x.u8(Obj::VARIANT) as u32;
    for i in 0..6 {
        let v = tables().shot1_speeds.at16(x.b, row + 2 * i);
        x.set16(Obj::VX + 2 * i, v);
    }
}

fn body<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Base::SUB_STATE) {
        LOB => {
            if !x.move_xyz().gt() {
                x.add8(Base::SUB_STATE, 2);
                x.set16(Shot::TIMER, 0x50);
                x.set32(Obj::Z, 0);
                crate::hw::sound::play(x.b, crate::hw::sound::CMD_320_420, x.o);
                x.anim_start(ANIMS.0, 1);
            }
        }
        LANDED => {
            if x.u8(Base::ANIM_EVENT) != 0 {
                x.set8(Base::ANIM_EVENT, 0);
                crate::hw::sound::play(x.b, crate::hw::sound::CMD_321_421, x.o);
            }
            let t = x.u16(Shot::TIMER).wrapping_sub(1);
            x.set16(Shot::TIMER, t);
            if t != 0 {
                x.anim_step();
            } else {
                x.add8(Base::SUB_STATE, 2);
                x.set16(Shot::TIMER, 0x50);
                x.anim_start(ANIMS.0, 2);
            }
        }
        BLINK => {
            x.anim_step();
            let t = x.u16(Shot::TIMER).wrapping_sub(1);
            x.set16(Shot::TIMER, t);
            if t == 0 {
                x.add8(Obj::STATE, 2);
            }
            blink(x);
        }
        _ => {}
    }
    finish(x);
}

/// End of the handler when the state is unchanged: sorted list then removal off the screen.
fn finish<R: Bench>(x: &mut Cx<'_, R>) {
    x.reinsert();
    x.cull_offscreen();
}
