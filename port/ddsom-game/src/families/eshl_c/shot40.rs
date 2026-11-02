use super::*;

const ANIMS: u32 = 0x16ff94;
/// Animation drawn at random (16 bytes).
const RANDOM_ANIMS: u32 = 0xa329e;
/// Value of `+0x2b` (palette (h)) by `+0x47`.
const PALETTES: u32 = 0xa31ba;
const CREATOR: u32 = 0x5c;
const LIFETIME: u32 = 0x42;
const PALETTE_COUNTDOWN: u32 = 0x46;
const PALETTE_INDEX: u32 = 0x47;
/// Variable `A5+0x6e`: bit 0 = global blinking phase.
const BLINK_PHASE: u32 = 0x6e;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => spawn(x),
        2 => live(x),
        _ => x.free_eshl(),
    }
}

fn spawn<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(CREATOR) == 0 {
        return;
    }
    x.add8(0x4, 2);
    crate::sound::play(x.b, crate::sound::CMD_346, x.o);
    x.set8(f::BOX_SET, 7);
    x.set8(f::ATTACK_POWER, 5);
    x.set8(f::ATTACK_ID, 0x1e);
    x.set16(LIFETIME, 0x5a);
    x.set8(PALETTE_COUNTDOWN, 4);
    let creator = x.linked(CREATOR);
    copy_xyz32(x, creator);
    copy_flip(x, creator);
    let owner = creator.u16(x.b, CREATOR);
    x.set16(f::OWNER, owner);
    match x.u8(0x3) {
        1 => {
            x.add16(0x8, neg(0xa));
            x.add16(0xc, 0xa);
        }
        2 => {
            x.add16(0x8, 0xa);
            x.add16(0xc, 0xa);
        }
        _ => {}
    }
    let r = random(x) & 0xf;
    let anim = x.rom8(RANDOM_ANIMS + r);
    x.anim_start(ANIMS, (anim as u32) as u16);
}

fn live<R: Bench>(x: &mut Cx<'_, R>) {
    cycle_palette(x);
    if x.linked(f::OWNER).u16(x.b, 0x4) == 0x406 {
        return x.free_eshl();
    }
    if x.u8(0x5) == 0 {
        x.anim_step();
        let t = x.u16(LIFETIME).wrapping_sub(1);
        x.set16(LIFETIME, t);
        if s16(t) < 0 {
            x.add8(0x4, 2);
            crate::sound::play(x.b, crate::sound::CMD_337, x.o);
        }
    }
    flicker(x);
    x.reinsert();
}

/// Every 5 steps, goes to the next palette of the table (8 entries).
fn cycle_palette<R: Bench>(x: &mut Cx<'_, R>) {
    let n = x.u8(PALETTE_COUNTDOWN).wrapping_sub(1);
    x.set8(PALETTE_COUNTDOWN, n);
    if s8(n) >= 0 {
        return;
    }
    x.set8(PALETTE_COUNTDOWN, 4);
    x.set8(0x2a, 1);
    let i = x.u8(PALETTE_INDEX).wrapping_add(1) & 7;
    x.set8(PALETTE_INDEX, i);
    let p = x.rom8(PALETTES + i as u32);
    x.set8(0x2b, p);
}

/// Hidden when the variant's parity differs from the global phase.
fn flicker<R: Bench>(x: &mut Cx<'_, R>) {
    let hidden = (x.u8(0x3) & 1) as u16 != x.a5_u16(BLINK_PHASE) & 1;
    x.set_bit(0x1, 1, hidden);
}
