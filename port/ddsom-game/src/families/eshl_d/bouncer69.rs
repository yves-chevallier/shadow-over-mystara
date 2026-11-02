use super::*;

const ANIMS: u32 = 0x17b9d6;
/// Per rank: wait then animation (words, program ROM).
const RANKS: u32 = 0xa7eb4;
const RANK: u32 = 0x2f;
const TIMER: u32 = 0x42;
const SFX_BOUNCE: crate::sound::Sound = crate::sound::CMD_385_485;
const CAMERA_X: u32 = 0x506;
const CAMERA2_X: u32 = 0x556;

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
    let strong = x.u8(TIMER) == 11;
    x.set8(f::ATTACK_ID, if strong { 0x36 } else { 0x33 });
    let dx: u16 = if x.u8(f::FLIP) != 0 { 48 } else { 48u16.wrapping_neg() };
    x.add16(0x8, dx);
    x.set16(0xc, 32);
    bounce(x);
    let rank = x.u8(RANK) as u32;
    let row = RANKS + 4 * rank;
    let delay = x.rom16(row);
    x.set16(TIMER, delay);
    let anim = x.rom16(row + 2);
    x.anim_start(ANIMS, (anim as u32) as u16);
}

/// 0xa7ed4: speeds of a leap (forward), sound.
fn bounce<R: Bench>(x: &mut Cx<'_, R>) {
    let vx: u16 = if x.u8(f::FLIP) != 0 { 0x400 } else { 0x400u16.wrapping_neg() };
    x.set16(0x14, vx);
    x.set16(0x16, 0);
    x.set16(0x18, 0x800);
    x.set16(0x1a, 0xff80);
    x.set32(0x1c, 0);
    crate::sound::play(x.b, SFX_BOUNCE, x.o);
}

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(0x5) == 0 {
        x.add16(TIMER, 0xffff);
        if x.u16(TIMER) as i16 > 0 {
            return;
        }
        x.add8(0x5, 2);
    }
    x.anim_step();
    x.move_xz();
    if x.u16(0xc) as i16 <= 0 {
        x.set16(0xc, 0);
        bounce(x);
    }
    let camera = if x.a5_u8(0x36) != 0 { CAMERA2_X } else { CAMERA_X };
    let dx = x.u16(0x8).wrapping_sub(x.a5_u16(camera)) as i16;
    if dx <= -16 || dx > 400 {
        x.add8(0x4, 2);
        return;
    }
    x.set_bit(0x1, 1, false);
    x.reinsert();
}
