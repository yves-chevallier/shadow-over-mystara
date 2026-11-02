use super::*;
use crate::data::RomAddr;

const ANIMS: RomAddr = RomAddr(0x171a52);
const SPEED_TABLE: u32 = 53;
const TIMER: u32 = 0x58;
const DUST_TIMER: u32 = 0x5a;
const SPIN_TIMER: u32 = 0x5e;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    match x.u8(Obj::STATE) {
        0 => init(x),
        2 => active(x),
        _ => x.free_eshl(),
    }
    if x.u8(Obj::ACTIVE) != 0 {
        x.reinsert();
    }
}

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Obj::STATE, 2);
    x.set8(Base::BOX_SET, 7);
    let dir = if x.u8(Base::FLIP) != 0 { 8 } else { 24 };
    x.set8(Obj::SPEED_INDEX, dir);
    x.select_speed(&crate::objects::motion::SPEEDS_B, SPEED_TABLE as u16);
    let step = ((x.u16(Obj::VX) as i16).unsigned_abs() as i32) << 16 >> 8;
    let owner = x.linked(Shot::OWNER);
    let mut distance = (owner.u32(x.b, Obj::X).wrapping_sub(x.u32(Obj::X)) as i32).wrapping_abs();
    let mut vz = (x.u32(Obj::Z) as i32 >> 8) as i16 >> 6;
    loop {
        distance = distance.wrapping_sub(step);
        if distance <= 0 {
            break;
        }
        vz = vz.wrapping_sub(12);
    }
    let vz = vz.clamp(16, 256);
    x.set16(Obj::AZ, (vz as u16).wrapping_neg());
    crate::families::enemy_lib::start(x, ANIMS.0, 21);
}

/// 0xa7af4: flips the orientation every 4 frames.
fn spin<R: Bench>(x: &mut Cx<'_, R>) {
    x.add16(SPIN_TIMER, 0xffff);
    if x.u16(SPIN_TIMER) as i16 > 0 {
        return;
    }
    x.set16(SPIN_TIMER, 4);
    let flip = x.u8(Base::FLIP) ^ 0x20;
    x.set8(Base::FLIP, flip);
}

fn impact<R: Bench>(x: &mut Cx<'_, R>, kind: u32) {
    crate::ai_aids::debris_set(x.b, x.o, kind as u16);
    // Original glitch: the exhausted `dbf` counter stays in D5 (`dispatch::Leftovers`)
    *x.c.d5() = 0xffff;
}

/// Movement in X and in height; true when the object touches the ground (height reset to 0).
fn lands<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    x.move_xz();
    if x.u16(Obj::Z) as i16 > 0 {
        return false;
    }
    x.set16(Obj::Z, 0);
    true
}

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Base::SUB_STATE) {
        0 => {
            if crate::ai_aids::outside_band(x.b, x.o, 0xffc0, 448) {
                return x.free_eshl();
            }
            if x.u8(Shot::HIT_FLAGS) & 0x28 != 0 {
                x.set8(Base::SUB_STATE, 4);
                let vx = (x.u16(Obj::VX) as i16 >> 3) as u16;
                x.set16(Obj::VX, vx.wrapping_neg());
                x.set16(Obj::VZ, 0x200);
                x.set16(Obj::AZ, 0xffe0);
                return;
            }
            spin(x);
            if lands(x) {
                x.set8(Base::SUB_STATE, 2);
                x.set16(TIMER, 30);
                x.set8(Obj::ACTIVE, 2);
                let friction = (x.u16(Obj::VX) as i16 >> 5) as u16;
                x.set16(Obj::AX, friction.wrapping_neg());
            }
        }
        2 => {
            spin(x);
            x.move_x();
            if count_down_one(x, TIMER) {
                x.set8(Base::SUB_STATE, 6);
                x.set16(TIMER, 60);
            } else if count_down_one(x, DUST_TIMER) {
                x.set16(DUST_TIMER, 6);
                impact(x, 9);
            }
        }
        4 => match x.u8(Base::STEP) {
            0 => {
                spin(x);
                x.move_xz();
                if x.u16(Obj::VZ) as i16 <= 0 {
                    x.add8(Base::STEP, 2);
                    x.set8(Obj::ACTIVE, 2);
                    x.set16(Obj::AZ, 0xffa0);
                }
            }
            2 => {
                spin(x);
                if lands(x) {
                    x.add8(Base::STEP, 2);
                    x.set16(Obj::VZ, 0x300);
                    x.set16(Obj::AZ, 0xffc0);
                    impact(x, 12);
                }
            }
            4 => {
                if lands(x) {
                    x.add8(Base::STEP, 2);
                    x.set16(Obj::VZ, 0x200);
                    x.set16(Obj::AZ, 0xffc0);
                    impact(x, 13);
                }
            }
            _ => {
                if lands(x) {
                    x.set8(Base::SUB_STATE, 6);
                    x.set16(TIMER, 60);
                    impact(x, 8);
                }
            }
        },
        _ => {
            if count_down_one(x, TIMER) {
                return x.free_eshl();
            }
            if x.u16(TIMER) as i16 <= 30 {
                let v = x.u8(Obj::FLAGS) ^ 2;
                x.set8(Obj::FLAGS, v);
            }
        }
    }
}

/// `subq.w #1 ; ble`: true when the counter is no longer positive.
fn count_down_one<R: Bench>(x: &mut Cx<'_, R>, off: u32) -> bool {
    x.add16(off, u16::MAX);
    x.u16(off) as i16 <= 0
}
