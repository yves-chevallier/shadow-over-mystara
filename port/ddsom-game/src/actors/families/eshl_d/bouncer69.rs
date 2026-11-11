use super::*;
use crate::world::data::RomAddr;
use crate::actors::families::shell::tables::{tables, Read};

const ANIMS: RomAddr = RomAddr(0x17b9d6);
const RANK: u32 = 0x2f;
const TIMER: u32 = 0x42;
const SFX_BOUNCE: crate::hw::sound::Sound = crate::hw::sound::CMD_385_485;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => init(x),
        2 => active(x),
        4 => x.add8(Obj::STATE, 2),
        _ => x.free_eshl(),
    }
}

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Obj::STATE, 2);
    x.set8(Base::SUB_STATE, 0);
    x.set8(Base::BOX_SET, 0x2c);
    let strong = x.u8(TIMER) == 11;
    x.set8(Shot::ATTACK_ID, if strong { 0x36 } else { 0x33 });
    let dx: u16 = if x.u8(Base::FLIP) != 0 { 48 } else { 48u16.wrapping_neg() };
    x.add16(Obj::X, dx);
    x.set16(Obj::Z, 32);
    bounce(x);
    let rank = x.u8(RANK) as u32;
    let row = 4 * rank;
    let delay = tables().bouncer69_ranks.at16(x.b, row);
    x.set16(TIMER, delay);
    let anim = tables().bouncer69_ranks.at16(x.b, row + 2);
    x.anim_start(ANIMS.0, (anim as u32) as u16);
}

/// 0xa7ed4: speeds of a leap (forward), sound.
fn bounce<R: Bench>(x: &mut Cx<'_, R>) {
    let vx: u16 = if x.u8(Base::FLIP) != 0 { 0x400 } else { 0x400u16.wrapping_neg() };
    x.set16(Obj::VX, vx);
    x.set16(Obj::AX, 0);
    x.set16(Obj::VZ, 0x800);
    x.set16(Obj::AZ, 0xff80);
    x.set32(Obj::VY, 0);
    crate::hw::sound::play(x.b, SFX_BOUNCE, x.o);
}

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Base::SUB_STATE) == 0 {
        x.add16(TIMER, 0xffff);
        if x.u16(TIMER) as i16 > 0 {
            return;
        }
        x.add8(Base::SUB_STATE, 2);
    }
    x.anim_step();
    x.move_xz();
    if x.u16(Obj::Z) as i16 <= 0 {
        x.set16(Obj::Z, 0);
        bounce(x);
    }
    let dx = x.u16(Obj::X).wrapping_sub(camera(x.b).0.get(x.b)) as i16;
    if dx <= -16 || dx > 400 {
        x.add8(Obj::STATE, 2);
        return;
    }
    x.set_bit(Obj::FLAGS, 1, false);
    x.reinsert();
}
