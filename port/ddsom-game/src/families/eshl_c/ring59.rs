use super::*;

const ANIMS: u32 = 0x171026;
const LINK: u32 = 0x5c;
const TIMER: u32 = 0x42;
/// Offsets (X, Y) per variant for sub-type 1 (4 bytes per row).
const OFFSETS: u32 = 0xa5db2;
/// Value of `+0x2b` (palette (h)) by `+0x47`.
const PALETTES: u32 = 0xa5f30;
const FX_START_0: crate::sound::Sound = crate::sound::CMD_389;
const FX_START_1: crate::sound::Sound = crate::sound::CMD_38B;
const FX_END: crate::sound::Sound = crate::sound::CMD_38A;
const PALETTE_COUNTDOWN: u32 = 0x46;
const PALETTE_INDEX: u32 = 0x47;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => start(x),
        2 => {
            match x.u8(0x2f) {
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
    x.add8(0x4, 2);
    match x.u8(0x2f) {
        0 => {
            x.set8(f::BOX_SET, 7);
            x.set8(f::ATTACK_ID, 0x24);
            x.set8(PALETTE_COUNTDOWN, 2);
            crate::sound::play(x.b, FX_START_0, x.o);
            let maker = x.linked(LINK);
            copy_xyz32(x, maker);
            copy_flip(x, maker);
            x.add16(0x10, 1);
            x.set16(0xc, 0);
            let owner = maker.u16(x.b, LINK);
            x.set16(f::OWNER, owner);
            x.add16(0x10, neg(4));
            x.anim_start(ANIMS, 1);
        }
        1 => {
            x.set8(f::BOX_SET, 7);
            x.set8(f::ATTACK_ID, 0x24);
            x.set8(PALETTE_COUNTDOWN, 2);
            crate::sound::play(x.b, FX_START_1, x.o);
            let maker = x.linked(LINK);
            let (mx, my) = (maker.u16(x.b, 0x8), maker.u16(x.b, 0x10));
            let z = maker.u32(x.b, 0xc);
            x.set32(0xc, z);
            copy_flip(x, maker);
            x.set16(0xc, 0);
            let row = OFFSETS + 4 * x.u8(0x3) as u32;
            let (dx, dy) = (x.rom16(row), x.rom16(row + 2));
            x.set16(0x8, dx.wrapping_add(mx));
            x.set16(0x10, dy.wrapping_add(my));
            x.anim_start(ANIMS, 0);
        }
        _ => {}
    }
}

/// Sub-type 0: follows the owner.
fn ring<R: Bench>(x: &mut Cx<'_, R>) {
    let owner = x.linked(f::OWNER);
    if owner.u16(x.b, 0x4) == 0x406 {
        return x.free_eshl();
    }
    match x.u8(0x5) {
        0 => {
            if x.anim_step() {
                x.add8(0x5, 2);
                x.set16(TIMER, 0x12c);
                x.anim_start(ANIMS, 2);
            }
        }
        2 => {
            let t = x.u16(TIMER).wrapping_sub(1);
            x.set16(TIMER, t);
            if s16(t) >= 0 {
                x.anim_step();
            } else {
                x.add8(0x5, 2);
                x.anim_start(ANIMS, 3);
            }
        }
        4 => {
            if x.anim_step() {
                x.add8(0x4, 2);
                crate::sound::play(x.b, FX_END, x.o);
            }
        }
        _ => {}
    }
    cycle_palette(x);
}

/// Sub-type 1: animation then signal to the creator.
fn satellite<R: Bench>(x: &mut Cx<'_, R>) {
    let maker = x.linked(LINK);
    if maker.u16(x.b, 0x4) == 0x406 {
        return x.free_eshl();
    }
    if x.anim_step() {
        x.add8(0x4, 2);
        let maker = x.linked(LINK);
        maker.set_u8(x.b, 0xae, 1);
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
    x.set8(0x2a, 1);
    let i = x.u8(PALETTE_INDEX).wrapping_add(1) & 7;
    x.set8(PALETTE_INDEX, i);
    let p = x.rom8(PALETTES + i as u32);
    x.set8(0x2b, p);
}
