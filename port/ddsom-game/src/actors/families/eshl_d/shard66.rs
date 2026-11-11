use super::*;
use crate::world::data::RomAddr;
use crate::actors::families::shell::tables::{tables, Read};

const ANIMS: RomAddr = RomAddr(0x17b9d6);
/// Offsets per variant of its rows in `shard66_offsets`.
const OFFSETS: [u32; 6] = [0, 0, 0x20, 0x3c, 0x5c, 0x74];
const CARRIER: u32 = 0x46;
const TIMER: u32 = 0x42;
const RANK: u32 = 0x2f;
const SFX_LAND: crate::hw::sound::Sound = crate::hw::sound::CMD_387_487;
/// States `+4.w` of the carrier in which the burst stays held.
const HELD_STATE: u16 = 0x204;

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
    x.set8(Shot::ATTACK_ID, 0x31);
    let v = x.u8(Obj::VARIANT) as usize;
    let table = OFFSETS.get(v).copied().unwrap_or(OFFSETS[0]);
    let row = table + 4 * x.u8(RANK) as u32;
    let dx = tables().shard66_offsets.at16(x.b, row);
    let dx = if x.u8(Base::FLIP) != 0 { dx.wrapping_neg() } else { dx };
    x.set16(Obj::VX, dx);
    let dz = tables().shard66_offsets.at16(x.b, row + 2);
    x.set16(Obj::VY, dz);
    x.set16(Obj::VZ, 0x60);
    x.set16(TIMER, 8);
    let n = tables().shard66_start_anims.at8(x.b, v as u32) as u32;
    x.anim_start(ANIMS.0, n as u16);
}

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    let carrier = x.u16(CARRIER);
    if carrier as i16 >= 0 {
        // no carrier: the original restarts from initialization (substate 4)
        x.set16(Obj::STATE, 4);
        return;
    }
    let carrier = Obj(carrier as i16 as i32 as u32 & 0xff_ffff);
    match x.u8(Base::SUB_STATE) {
        0 => {
            for off in [0x8, 0xc, 0x10] {
                let v = carrier.u16(x.b, off);
                x.set16(off, v);
            }
            let flip = carrier.u8(x.b, Base::FLIP);
            x.set8(Base::FLIP, flip);
            for (pos, speed) in [(0x8, 0x14), (0x10, 0x1c), (0xc, 0x18)] {
                let d = x.u16(speed);
                x.add16(pos, d);
            }
            let t = x.u16(TIMER);
            if t as i16 > 0 {
                let t = t - 1;
                x.set16(TIMER, t);
                x.add16(Obj::Y, t);
                return blink(x, t & 1 != 0);
            }
            // (test of the original on the burst itself: always false in substate 0)
            if x.u16(Obj::STATE) == HELD_STATE && (x.u8(Base::STEP) as i8) < 6 {
                return blink(x, false);
            }
            x.add8(Base::SUB_STATE, 2);
            x.set32(Obj::VX, 0);
            x.set32(Obj::VZ, 0);
            x.set32(Obj::VY, 0);
            x.set16(Obj::VZ, 0xf800);
            x.set16(Obj::AZ, 0xff00);
        }
        2 => {
            x.move_z();
            if x.u16(Obj::Z) as i16 <= 0 {
                x.set16(Obj::Z, 0);
                x.add8(Base::SUB_STATE, 2);
                x.set8(Shot::ATTACK_ID, 0x32);
                x.set16(TIMER, 16);
                let v = x.u8(Obj::VARIANT) as u32;
                let n = tables().shard66_land_anims.at8(x.b, v) as u32;
                x.anim_start(ANIMS.0, n as u16);
                crate::services::camera_modes::start_shake(x.b, crate::services::camera_modes::shake::A, 4);
                crate::hw::sound::play(x.b, SFX_LAND, x.o);
            }
            blink(x, false);
        }
        4 => {
            if count_down_signed(x) {
                x.set16(TIMER, 16);
                x.add8(Base::SUB_STATE, 2);
            } else {
                blink(x, false);
            }
        }
        _ => {
            let t = x.u16(TIMER);
            if t as i16 <= 0 {
                x.add8(Obj::STATE, 2);
                return;
            }
            let t = t - 1;
            x.set16(TIMER, t);
            blink(x, t & 1 != 0);
        }
    }
}

/// `subq.w #1,+0x42 ; bgt`: true when the counter is no longer positive.
fn count_down_signed<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    x.add16(TIMER, 0xffff);
    x.u16(TIMER) as i16 <= 0
}

/// Common end: visibility (bit 1 of `+1`: hidden), terrain, on-screen restraint, sort.
fn blink<R: Bench>(x: &mut Cx<'_, R>, hidden: bool) {
    x.set_bit(Obj::FLAGS, 1, hidden);
    x.push_terrain();
    x.keep_on_screen(16);
    x.reinsert();
}
