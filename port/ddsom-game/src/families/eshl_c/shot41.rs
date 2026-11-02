use super::*;
use crate::families::shell::tables::{tables, Read};

const ANIMS: u32 = 0x1700f8;
const CREATOR: u32 = 0x5c;
const PALETTE_DST: u32 = 0x91c2e0;
/// Fields of the target (player object): aiming position.
const TARGET_X: u32 = 0xb0;
const TARGET_Y: u32 = 0xb2;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => spawn(x),
        2 => live(x),
        _ => x.free_eshl(),
    }
}

fn target<R: Bench>(x: &mut Cx<'_, R>) -> Obj {
    let w = x.linked(CREATOR).u16(x.b, CREATOR);
    obj_of(w)
}

fn spawn<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(CREATOR) == 0 {
        return;
    }
    x.add8(0x4, 2);
    crate::sound::play(x.b, crate::sound::CMD_354, x.o);
    x.set8(f::BOX_SET, 7);
    x.set8(f::ATTACK_POWER, 5);
    x.set8(f::ATTACK_ID, 0x1f);
    let creator = x.linked(CREATOR);
    let owner = creator.u16(x.b, CREATOR);
    x.set16(f::OWNER, owner);
    copy_xyz32(x, creator);
    copy_flip(x, creator);
    x.add16(0x10, 0xffff);
    let t = target(x);
    let (tx, ty) = (t.u16(x.b, TARGET_X), t.u16(x.b, TARGET_Y));
    let dir = crate::aids::direction_from(x.b, x.o, (tx, ty));
    x.set8(0x2d, dir);
    x.select_speed(&crate::objects::motion::SPEEDS_B, 0x41);
    let t = target(x);
    let mut dist = s16(x.u16(0x8).wrapping_sub(t.u16(x.b, TARGET_X)));
    if dist < 0 {
        dist = dist.wrapping_neg();
    }
    let band = (1..=11).find(|&i| dist <= 0x20 * i).map_or(0x18, |i| 2 * (i - 1) as u32);
    let speed = tables().shot41_fall_speeds.at16(x.b, band);
    x.set16(0x18, speed);
    copy_palette(x);
    x.set8(0x2a, 1);
    x.set8(0x2b, 0x17);
    x.anim_start(ANIMS, 0);
}

/// Copies 8 longs from the ROM to the palette RAM.
fn copy_palette<R: Bench>(x: &mut Cx<'_, R>) {
    for i in 0..8 {
        let v = tables().shot41_palette.at32(x.b, 4 * i);
        wr32(x.b, PALETTE_DST + 4 * i, v);
    }
}

fn live<R: Bench>(x: &mut Cx<'_, R>) {
    if x.linked(f::OWNER).u16(x.b, 0x4) == 0x406 {
        return x.free_eshl();
    }
    match x.u8(0x5) {
        0 => fall(x),
        2 => {
            if x.anim_step() {
                x.add8(0x4, 2);
            }
        }
        _ => {}
    }
    x.reinsert();
}

fn fall<R: Bench>(x: &mut Cx<'_, R>) {
    x.move_xyz();
    if s16(x.u16(0xc)) > 0 {
        x.anim_step();
        return;
    }
    x.add8(0x5, 2);
    x.set16(0xc, 0);
    x.set8(0x2a, 1);
    x.set8(0x2b, 0x13);
    x.anim_start(ANIMS, 1);
    crate::families::eshl_creators::ring_28(&mut crate::families::effects_creators::leftovers::Pure, x.b, x.o);
}
