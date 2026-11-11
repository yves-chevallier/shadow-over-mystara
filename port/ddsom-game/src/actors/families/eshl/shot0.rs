use super::*;
use crate::world::data::RomAddr;

pub const ANIMS: RomAddr = RomAddr(0x160350);
/// Substates of the fall (`+6`).
const FALL_START: u8 = 0;
const FALL_DROP: u8 = 2;
const FALL_BLINK: u8 = 4;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => {
            if !spawn(x) {
                return;
            }
            fly(x);
        }
        2 => fly(x),
        4 => fall(x),
        _ => x.free_eshl(),
    }
}

/// Creation: places the projectile in front of the owner. False if the owner has disappeared.
fn spawn<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    x.add8(Obj::STATE, 2);
    x.set8(Base::BOX_SET, 7);
    if x.u16(Shot::OWNER) == 0 {
        return false;
    }
    let owner = x.linked(Shot::OWNER);
    place_in_front(x, owner, 0x40, 0x38);
    x.set32(Obj::VZ, 0);
    x.set8(Obj::SPEED_INDEX, 0x18);
    x.set16(Obj::VX, -0x500i16 as u16);
    if x.u8(Base::FLIP) != 0 {
        x.set8(Obj::SPEED_INDEX, 8);
        x.set16(Obj::VX, 0x500);
    }
    let (anim, id, power) = if x.u8(Obj::VARIANT) == 0 { (2, 0x18, 3) } else { (0xf, 0x19, 5) };
    x.set8(Shot::ATTACK_POWER, power);
    x.set8(Shot::ATTACK_ID, id);
    x.anim_start(ANIMS.0, anim);
    crate::hw::sound::play(x.b, crate::hw::sound::CMD_322_422, x.o);
    true
}

/// Flight: destroyed on contact, stops on terrains 5 to 8 without a wall (bit 6), otherwise advances.
fn fly<R: Bench>(x: &mut Cx<'_, R>) {
    let hit = x.u8(Shot::HIT_FLAGS);
    if hit & 0x08 != 0 {
        crate::hw::sound::play(x.b, crate::hw::sound::CMD_122_222, x.o);
        x.free_eshl();
        return;
    }
    if hit & 0x20 != 0 {
        x.add8(Obj::STATE, 2);
        return fall(x);
    }
    let t = crate::services::video::terrain_of(x.b, x.o);
    let terrain = t.number as u32;
    let ground = (terrain & 0x3f).wrapping_sub(5) as u16 <= 3;
    if ground && terrain & 0x40 == 0 {
        return fall(x);
    }
    x.move_xz();
    x.anim_step();
    x.cull_offscreen();
    x.reinsert();
}

/// Fall after the impact: falls, then blinks for 20 frames before disappearing.
fn fall<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Base::STEP) {
        FALL_START => {
            x.add8(Base::STEP, 2);
            crate::hw::sound::play(x.b, crate::hw::sound::CMD_123_223, x.o);
            x.set16(Obj::VZ, -0xc0i16 as u16);
            x.set16(Obj::AZ, -0x10i16 as u16);
            x.set16(Obj::VX, 0);
            let anim = if x.u8(Obj::VARIANT) != 0 { 0x10 } else { 3 };
            x.anim_start(ANIMS.0, anim);
        }
        FALL_DROP => {
            x.move_xz();
            x.anim_step();
            if (x.u16(Obj::Z) as i16) < 0 {
                x.set_bit(Obj::FLAGS, 1, true);
                x.set16(Obj::Z, 0);
                x.add8(Base::STEP, 2);
                x.set8(Shot::TIMER, 0x14);
            }
        }
        FALL_BLINK => {
            let t = x.u8(Shot::TIMER).wrapping_sub(1);
            x.set8(Shot::TIMER, t);
            if (t as i8) < 0 {
                x.set8(Obj::STATE, st::FREE);
            }
            blink(x);
            x.anim_step();
        }
        _ => {}
    }
}
