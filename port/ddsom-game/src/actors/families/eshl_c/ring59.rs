use super::*;
use crate::world::data::RomAddr;
use crate::actors::families::shell::records::Host;
use crate::actors::families::shell::tables::{tables, Read};

const ANIMS: RomAddr = RomAddr(0x171026);
const LINK: u32 = 0x5c;
const TIMER: u32 = 0x42;
const FX_START_0: crate::services::sound::Sound = crate::services::sound::CMD_389;
const FX_START_1: crate::services::sound::Sound = crate::services::sound::CMD_38B;
const FX_END: crate::services::sound::Sound = crate::services::sound::CMD_38A;
const PALETTE_COUNTDOWN: u32 = 0x46;
const PALETTE_INDEX: u32 = 0x47;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => start(x),
        2 => {
            match x.u8(Base::SETTING) {
                0 => ring(x),
                1 => satellite(x),
                _ => {}
            }
            x.reinsert();
        }
        _ => x.free_eshl(),
    }
}

fn start<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(LINK) == 0 {
        return;
    }
    x.add8(Obj::STATE, 2);
    match x.u8(Base::SETTING) {
        0 => {
            x.set8(Base::BOX_SET, 7);
            x.set8(Shot::ATTACK_ID, 0x24);
            x.set8(PALETTE_COUNTDOWN, 2);
            crate::services::sound::play(x.b, FX_START_0, x.o);
            let maker = x.linked(LINK);
            copy_xyz32(x, maker);
            copy_flip(x, maker);
            x.add16(Obj::Y, 1);
            x.set16(Obj::Z, 0);
            let owner = maker.u16(x.b, LINK);
            x.set16(Shot::OWNER, owner);
            x.add16(Obj::Y, neg(4));
            x.anim_start(ANIMS.0, 1);
        }
        1 => {
            x.set8(Base::BOX_SET, 7);
            x.set8(Shot::ATTACK_ID, 0x24);
            x.set8(PALETTE_COUNTDOWN, 2);
            crate::services::sound::play(x.b, FX_START_1, x.o);
            let maker = x.linked(LINK);
            let (mx, my) = (maker.u16(x.b, Obj::X), maker.u16(x.b, Obj::Y));
            let z = maker.u32(x.b, Obj::Z);
            x.set32(Obj::Z, z);
            copy_flip(x, maker);
            x.set16(Obj::Z, 0);
            let row = 4 * x.u8(Obj::VARIANT) as u32;
            let (dx, dy) = (tables().ring59_offsets.at16(x.b, row), tables().ring59_offsets.at16(x.b, row + 2));
            x.set16(Obj::X, dx.wrapping_add(mx));
            x.set16(Obj::Y, dy.wrapping_add(my));
            x.anim_start(ANIMS.0, 0);
        }
        _ => {}
    }
}

/// Sub-type 0: follows the owner.
fn ring<R: Bench>(x: &mut Cx<'_, R>) {
    let owner = x.linked(Shot::OWNER);
    if owner.u16(x.b, Obj::STATE) == 0x406 {
        return x.free_eshl();
    }
    match x.u8(Base::SUB_STATE) {
        0 => {
            if x.anim_step() {
                x.add8(Base::SUB_STATE, 2);
                x.set16(TIMER, 0x12c);
                x.anim_start(ANIMS.0, 2);
            }
        }
        2 => {
            let t = x.u16(TIMER).wrapping_sub(1);
            x.set16(TIMER, t);
            if s16(t) >= 0 {
                x.anim_step();
            } else {
                x.add8(Base::SUB_STATE, 2);
                x.anim_start(ANIMS.0, 3);
            }
        }
        4 => {
            if x.anim_step() {
                x.add8(Obj::STATE, 2);
                crate::services::sound::play(x.b, FX_END, x.o);
            }
        }
        _ => {}
    }
    cycle_palette(x);
}

/// Sub-type 1: animation then signal to the creator.
fn satellite<R: Bench>(x: &mut Cx<'_, R>) {
    let maker = x.linked(LINK);
    if maker.u16(x.b, Obj::STATE) == 0x406 {
        return x.free_eshl();
    }
    if x.anim_step() {
        x.add8(Obj::STATE, 2);
        let maker = x.linked(LINK);
        maker.set_u8(x.b, Host::SHOT_DONE, 1);
    }
    cycle_palette(x);
    blink_by_variant(x);
}

fn cycle_palette<R: Bench>(x: &mut Cx<'_, R>) {
    let n = x.u8(PALETTE_COUNTDOWN).wrapping_sub(1);
    x.set8(PALETTE_COUNTDOWN, n);
    if s8(n) >= 0 {
        return;
    }
    x.set8(PALETTE_COUNTDOWN, 2);
    x.set8(Base::OWN_PALETTE, 1);
    let i = x.u8(PALETTE_INDEX).wrapping_add(1) & 7;
    x.set8(PALETTE_INDEX, i);
    let p = tables().ring59_palettes.at8(x.b, i as u32);
    x.set8(Base::PALETTE, p);
}
