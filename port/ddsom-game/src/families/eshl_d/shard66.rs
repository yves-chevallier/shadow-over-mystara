use super::*;

const ANIMS: u32 = 0x17b9d6;
/// Animation per variant, at creation then at impact (bytes, program ROM).
const START_ANIMS: u32 = 0xa760e;
const LAND_ANIMS: u32 = 0xa777a;
/// Offsets (X, depth) per variant, four bytes per rank (words, in the clear).
const OFFSETS: [u32; 6] = [0xa7834, 0xa7834, 0xa7854, 0xa7870, 0xa7890, 0xa78a8];
const CARRIER: u32 = 0x46;
const TIMER: u32 = 0x42;
const RANK: u32 = 0x2f;
const SFX_LAND: crate::sound::Sound = crate::sound::CMD_387_487;
/// States `+4.w` of the carrier in which the burst stays held.
const HELD_STATE: u16 = 0x204;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => init(x),
        2 => active(x),
        4 => x.add8(0x4, 2),
        _ => x.free_eshl(),
    }
}

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x4, 2);
    x.set8(0x5, 0);
    x.set8(f::BOX_SET, 0x2c);
    x.set8(f::ATTACK_ID, 0x31);
    let v = x.u8(0x3) as usize;
    let table = OFFSETS.get(v).copied().unwrap_or(OFFSETS[0]);
    let row = table + 4 * x.u8(RANK) as u32;
    let dx = x.data16(row);
    let dx = if x.u8(f::FLIP) != 0 { dx.wrapping_neg() } else { dx };
    x.set16(0x14, dx);
    let dz = x.data16(row + 2);
    x.set16(0x1c, dz);
    x.set16(0x18, 0x60);
    x.set16(TIMER, 8);
    let n = x.rom8(START_ANIMS + v as u32) as u32;
    x.anim_start(ANIMS, n as u16);
}

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    let carrier = x.u16(CARRIER);
    if carrier as i16 >= 0 {
        // no carrier: the original restarts from initialization (substate 4)
        x.set16(0x4, 4);
        return;
    }
    let carrier = Obj(carrier as i16 as i32 as u32 & 0xff_ffff);
    match x.u8(0x5) {
        0 => {
            for off in [0x8, 0xc, 0x10] {
                let v = carrier.u16(x.b, off);
                x.set16(off, v);
            }
            let flip = carrier.u8(x.b, f::FLIP);
            x.set8(f::FLIP, flip);
            for (pos, speed) in [(0x8, 0x14), (0x10, 0x1c), (0xc, 0x18)] {
                let d = x.u16(speed);
                x.add16(pos, d);
            }
            let t = x.u16(TIMER);
            if t as i16 > 0 {
                let t = t - 1;
                x.set16(TIMER, t);
                x.add16(0x10, t);
                return blink(x, t & 1 != 0);
            }
            // (test of the original on the burst itself: always false in substate 0)
            if x.u16(0x4) == HELD_STATE && (x.u8(0x6) as i8) < 6 {
                return blink(x, false);
            }
            x.add8(0x5, 2);
            x.set32(0x14, 0);
            x.set32(0x18, 0);
            x.set32(0x1c, 0);
            x.set16(0x18, 0xf800);
            x.set16(0x1a, 0xff00);
        }
        2 => {
            x.move_z();
            if x.u16(0xc) as i16 <= 0 {
                x.set16(0xc, 0);
                x.add8(0x5, 2);
                x.set8(f::ATTACK_ID, 0x32);
                x.set16(TIMER, 16);
                let v = x.u8(0x3) as u32;
                let n = x.rom8(LAND_ANIMS + v) as u32;
                x.anim_start(ANIMS, n as u16);
                crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::A, 4);
                crate::sound::play(x.b, SFX_LAND, x.o);
            }
            blink(x, false);
        }
        4 => {
            if count_down_signed(x) {
                x.set16(TIMER, 16);
                x.add8(0x5, 2);
            } else {
                blink(x, false);
            }
        }
        _ => {
            let t = x.u16(TIMER);
            if t as i16 <= 0 {
                x.add8(0x4, 2);
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
    x.set_bit(0x1, 1, hidden);
    x.push_terrain();
    x.keep_on_screen(16);
    x.reinsert();
}
