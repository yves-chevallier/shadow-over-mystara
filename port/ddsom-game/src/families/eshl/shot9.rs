use super::*;
use crate::families::shell::tables::{tables, Read};

const ANIMS: u32 = 0x16d39a;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x3) {
        0 => bolt(x),
        1 => breath(x),
        _ => sparkle(x),
    }
}

fn die<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(0x4, st::FREE);
}

// ----- variant 0: ball that falls back -----

/// Aligns on the owner (in front of it, slightly deeper). Without an owner, the object is marked for destruction
/// (and the caller still continues its initialization).
fn attach<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(f::OWNER) == 0 {
        return die(x);
    }
    let owner = x.linked(f::OWNER);
    for off in [0x8, 0x10, 0xc] {
        let v = owner.u32(x.b, off);
        x.set32(off, v);
    }
    let flip = owner.u8(x.b, f::FLIP);
    x.set8(f::FLIP, flip);
    x.add16(0x8, if flip != 0 { 0x3e } else { -0x3ei16 as u16 });
    x.add32(0x10, 0x1200);
    x.add16(0xc, -0x1fi16 as u16);
}

fn bolt<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => {
            x.add8(0x4, 2);
            attach(x);
            x.set8(f::BOX_SET, 7);
            x.set8(f::ATTACK_ID, 0x20);
            x.set16(0x18, -0x400i16 as u16);
            x.set16(0x1a, 0);
            // peculiarity of the original: read at a fixed offset of the object (A0 - 0x7f92)
            let phase = crate::mem::rd16(x.b, x.o.0.wrapping_sub(0x7f92)) as u32;
            let mut index = (phase >> 2 & 3) + 6;
            if x.u8(f::FLIP) == 0 {
                index += 0x10;
            }
            x.set8(0x2d, index as u8);
            x.set32(0x14, 0);
            x.set32(0x1c, 0);
            x.select_speed(&crate::objects::motion::SPEEDS_A, 0x1d);
            x.anim_start(ANIMS, 0);
        }
        2 => {
            if !suspended(x) {
                match x.u8(0x5) {
                    0 => {
                        x.move_by_table_xy();
                        x.move_z();
                        if (x.u16(0xc) as i16) > 0 {
                            x.anim_step();
                        } else {
                            x.add8(0x5, 2);
                            x.set16(0xc, 0);
                            // the original reads here the low word of the long +0x14 (the acceleration)
                            let (vx, vy) = (x.u16(0x16) as i16 >> 1, x.u16(0x1c) as i16 >> 1);
                            x.set16(0x14, vx as u16);
                            x.set16(0x1c, vy as u16);
                            x.set16(0x18, 0);
                            x.set16(0x1a, 0x10);
                            x.anim_start(ANIMS, 1);
                        }
                    }
                    2 => {
                        x.move_xyz();
                        if x.anim_step() {
                            die(x);
                        }
                    }
                    _ => {}
                }
            }
            x.reinsert();
        }
        _ => x.free_eshl(),
    }
}

// ----- variant 1: breath stuck to an enemy of type 7 -----

/// Frame lists of the breath (offsets in `shot9_frames`): while it burns, at its end.
const FIRE_FRAMES: u32 = 0;
const END_FRAMES: u32 = 0xe;

/// The creator must be an enemy (class 2) of type 7, variant 0, in state 0x208, itself linked to a player still in
/// play; otherwise the breath disappears.
fn breath<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => {
            x.add8(0x4, 2);
            x.set8(0x5, 0);
            x.set8(0x2f, 0);
            x.set8(f::BOX_SET, 7);
            x.set8(f::ATTACK_ID, 0x20);
            x.anim_start(ANIMS, 2);
        }
        2 => {
            if !suspended(x) {
                if (x.u16(0x46) as i16) >= 0 {
                    return die(x);
                }
                let enemy = x.linked(0x46);
                let (class, kind, variant, state) =
                    (enemy.u8(x.b, 0x28), enemy.u8(x.b, 0x2), enemy.u8(x.b, 0x3), enemy.u16(x.b, 0x4));
                if class != 2 || kind != 7 || variant != 0 || state != 0x208 {
                    return die(x);
                }
                let target = enemy.u16(x.b, 0x46);
                if (target as i16) >= 0 || Obj(target as i16 as i32 as u32 & 0xff_ffff).u8(x.b, 0x4) as i8 >= 4 {
                    return die(x);
                }
                follow(x, enemy);
                breath_body(x, enemy);
            }
            x.reinsert();
        }
        _ => x.free_eshl(),
    }
}

fn breath_body<R: Bench>(x: &mut Cx<'_, R>, enemy: Obj) {
    const RISING: u8 = 0;
    const ENDING: u8 = 2;
    match x.u8(0x5) {
        RISING => {
            if enemy.u8(x.b, 0x6) as i8 > 4 {
                x.add8(0x5, 2);
                x.set8(0x6, 0);
                x.anim_start(ANIMS, 4);
                return end_frames(x);
            }
            match x.u8(0x6) {
                0 => {
                    x.anim_step();
                    let z = x.u16(0xc) as i16;
                    let k = x.u8(0x3e) as i8;
                    if z >= 0xa0 || k < 0 || (tables().shot9_rise_limits.at16(x.b, 2 * k as u32) as i16) < z {
                        return;
                    }
                    x.add8(0x6, 2);
                    x.anim_start(ANIMS, 3);
                    grow(x);
                }
                _ => grow(x),
            }
        }
        ENDING => end_frames(x),
        _ => {}
    }
}

fn grow<R: Bench>(x: &mut Cx<'_, R>) {
    x.anim_step();
    pick_frame(x, FIRE_FRAMES);
}

fn end_frames<R: Bench>(x: &mut Cx<'_, R>) {
    if x.anim_step() {
        return die(x);
    }
    pick_frame(x, END_FRAMES);
}

/// Sticks the breath to its creator: position, orientation, one notch deeper.
fn follow<R: Bench>(x: &mut Cx<'_, R>, enemy: Obj) {
    for off in [0x8, 0xc, 0x10] {
        let v = enemy.u32(x.b, off);
        x.set32(off, v);
    }
    let flip = enemy.u8(x.b, f::FLIP);
    x.set8(f::FLIP, flip);
    x.add16(0x12, 1);
    x.add16(0x8, if flip != 0 { 0x3e } else { -0x3ei16 as u16 });
    x.add16(0xc, -0x1fi16 as u16);
}

/// Sprite frame of the breath: set chosen according to the height, frame according to `+0x3e`.
fn pick_frame<R: Bench>(x: &mut Cx<'_, R>, set: u32) {
    let n = ((x.u16(0xc) >> 4).min(0xa) as i16 - 4).max(0);
    x.set8(0x2f, n as u8);
    let t = &tables().shot9_frames;
    let frames = set.wrapping_add(t.at16(x.b, set + 2 * n as u32) as i16 as i32 as u32);
    let entry = frames + 8 * x.u8(0x3e) as u32;
    let (a, b) = (t.at32(x.b, entry), t.at32(x.b, entry + 4));
    x.set32(0x34, a);
    x.set32(0x38, b);
}

// ----- variant 2: spark placed on an enemy -----

fn sparkle<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => {
            x.add8(0x4, 2);
            x.set8(f::BOX_SET, 7);
            x.set8(f::ATTACK_ID, 0x20);
            x.anim_start(ANIMS, 5);
        }
        2 => {
            if suspended(x) {
                x.set_bit(0x1, 1, false);
                return x.reinsert();
            }
            if (x.u16(0x46) as i16) >= 0 {
                return die(x);
            }
            let host = x.linked(0x46);
            if host.u8(x.b, 0) == 0 {
                return die(x);
            }
            let hx = host.u16(x.b, 0x8);
            x.set16(0x8, hx);
            let hy = host.u16(x.b, 0x10).wrapping_sub(8);
            x.set16(0x10, hy);
            x.set16(0xc, 0);
            let flip = host.u8(x.b, f::FLIP);
            x.set8(f::FLIP, flip);
            let growth = host.u8(x.b, 0x2f) as u16;
            if growth == 0 {
                x.set_bit(0x1, 1, true);
                return x.reinsert();
            }
            let reach = (growth << 4) + 0x74 + (host.u16(x.b, 0xc) & 0xf);
            x.add16(0x8, if flip != 0 { reach } else { reach.wrapping_neg() });
            x.anim_step();
            x.set_bit(0x1, 1, false);
            x.reinsert();
        }
        _ => x.free_eshl(),
    }
}
