use super::*;
use crate::world::data::RomAddr;
use crate::actors::families::shell::records::{Host, view};
use crate::actors::families::shell::tables::{tables, Read};

const ANIMS: RomAddr = RomAddr(0x16d39a);

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::VARIANT) {
        0 => bolt(x),
        1 => breath(x),
        _ => sparkle(x),
    }
}

fn die<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Obj::STATE, st::FREE);
}

// ----- variant 0: ball that falls back -----

/// Aligns on the owner (in front of it, slightly deeper). Without an owner, the object is marked for destruction
/// (and the caller still continues its initialization).
fn attach<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(Shot::OWNER) == 0 {
        return die(x);
    }
    let owner = x.linked(Shot::OWNER);
    for off in [0x8, 0x10, 0xc] {
        let v = owner.u32(x.b, off);
        x.set32(off, v);
    }
    let flip = owner.u8(x.b, Base::FLIP);
    x.set8(Base::FLIP, flip);
    x.add16(Obj::X, if flip != 0 { 0x3e } else { -0x3ei16 as u16 });
    x.add32(Obj::Y, 0x1200);
    x.add16(Obj::Z, -0x1fi16 as u16);
}

fn bolt<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => {
            x.add8(Obj::STATE, 2);
            attach(x);
            x.set8(Base::BOX_SET, 7);
            x.set8(Shot::ATTACK_ID, 0x20);
            x.set16(Obj::VZ, -0x400i16 as u16);
            x.set16(Obj::AZ, 0);
            // peculiarity of the original: read at a fixed offset of the object (A0 - 0x7f92)
            let phase = crate::hw::mem::rd16(x.b, x.o.0.wrapping_sub(0x7f92)) as u32;
            let mut index = (phase >> 2 & 3) + 6;
            if x.u8(Base::FLIP) == 0 {
                index += 0x10;
            }
            x.set8(Obj::SPEED_INDEX, index as u8);
            x.set32(Obj::VX, 0);
            x.set32(Obj::VY, 0);
            x.select_speed(&crate::world::objects::motion::SPEEDS_A, 0x1d);
            x.anim_start(ANIMS.0, 0);
        }
        2 => {
            if !suspended(x) {
                match x.u8(Base::SUB_STATE) {
                    0 => {
                        x.move_by_table_xy();
                        x.move_z();
                        if (x.u16(Obj::Z) as i16) > 0 {
                            x.anim_step();
                        } else {
                            x.add8(Base::SUB_STATE, 2);
                            x.set16(Obj::Z, 0);
                            // the original reads here the low word of the long +0x14 (the acceleration)
                            let (vx, vy) = (x.u16(Obj::AX) as i16 >> 1, x.u16(Obj::VY) as i16 >> 1);
                            x.set16(Obj::VX, vx as u16);
                            x.set16(Obj::VY, vy as u16);
                            x.set16(Obj::VZ, 0);
                            x.set16(Obj::AZ, 0x10);
                            x.anim_start(ANIMS.0, 1);
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
    match x.u8(Obj::STATE) {
        0 => {
            x.add8(Obj::STATE, 2);
            x.set8(Base::SUB_STATE, 0);
            x.set8(Base::SETTING, 0);
            x.set8(Base::BOX_SET, 7);
            x.set8(Shot::ATTACK_ID, 0x20);
            x.anim_start(ANIMS.0, 2);
        }
        2 => {
            if !suspended(x) {
                if (x.u16(Shot9::ENEMY) as i16) >= 0 {
                    return die(x);
                }
                let enemy = x.linked(Shot9::ENEMY);
                let (class, kind, variant, state) =
                    (enemy.u8(x.b, Obj::CLASS), enemy.u8(x.b, Obj::KIND), enemy.u8(x.b, Obj::VARIANT), enemy.u16(x.b, Obj::STATE));
                if class != 2 || kind != 7 || variant != 0 || state != 0x208 {
                    return die(x);
                }
                let target = enemy.u16(x.b, Host::TARGET);
                if (target as i16) >= 0 || Obj(target as i16 as i32 as u32 & 0xff_ffff).u8(x.b, Obj::STATE) as i8 >= 4 {
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
    match x.u8(Base::SUB_STATE) {
        RISING => {
            if enemy.u8(x.b, Base::STEP) as i8 > 4 {
                x.add8(Base::SUB_STATE, 2);
                x.set8(Base::STEP, 0);
                x.anim_start(ANIMS.0, 4);
                return end_frames(x);
            }
            match x.u8(Base::STEP) {
                0 => {
                    x.anim_step();
                    let z = x.u16(Obj::Z) as i16;
                    let k = x.u8(Shot9::RISE) as i8;
                    if z >= 0xa0 || k < 0 || (tables().shot9_rise_limits.at16(x.b, 2 * k as u32) as i16) < z {
                        return;
                    }
                    x.add8(Base::STEP, 2);
                    x.anim_start(ANIMS.0, 3);
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
    let flip = enemy.u8(x.b, Base::FLIP);
    x.set8(Base::FLIP, flip);
    x.add16(Base::Y_FRACTION, 1);
    x.add16(Obj::X, if flip != 0 { 0x3e } else { -0x3ei16 as u16 });
    x.add16(Obj::Z, -0x1fi16 as u16);
}

/// Sprite frame of the breath: set chosen according to the height, frame according to `+0x3e`.
fn pick_frame<R: Bench>(x: &mut Cx<'_, R>, set: u32) {
    let n = ((x.u16(Obj::Z) >> 4).min(0xa) as i16 - 4).max(0);
    x.set8(Base::SETTING, n as u8);
    let t = &tables().shot9_frames;
    let frames = set.wrapping_add(t.at16(x.b, set + 2 * n as u32) as i16 as i32 as u32);
    let entry = frames + 8 * x.u8(Shot9::RISE) as u32;
    let (a, b) = (t.at32(x.b, entry), t.at32(x.b, entry + 4));
    x.set32(Obj::FRAME, a);
    x.set32(Obj::FRAME_DATA_A, b);
}

// ----- variant 2: spark placed on an enemy -----

fn sparkle<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => {
            x.add8(Obj::STATE, 2);
            x.set8(Base::BOX_SET, 7);
            x.set8(Shot::ATTACK_ID, 0x20);
            x.anim_start(ANIMS.0, 5);
        }
        2 => {
            if suspended(x) {
                x.set_bit(Obj::FLAGS, 1, false);
                return x.reinsert();
            }
            if (x.u16(Shot9::ENEMY) as i16) >= 0 {
                return die(x);
            }
            let host = x.linked(Shot9::ENEMY);
            if host.u8(x.b, Obj::ACTIVE) == 0 {
                return die(x);
            }
            let hx = host.u16(x.b, Obj::X);
            x.set16(Obj::X, hx);
            let hy = host.u16(x.b, Obj::Y).wrapping_sub(8);
            x.set16(Obj::Y, hy);
            x.set16(Obj::Z, 0);
            let flip = host.u8(x.b, Base::FLIP);
            x.set8(Base::FLIP, flip);
            let growth = host.u8(x.b, Base::SETTING) as u16;
            if growth == 0 {
                x.set_bit(Obj::FLAGS, 1, true);
                return x.reinsert();
            }
            let reach = (growth << 4) + 0x74 + (host.u16(x.b, Obj::Z) & 0xf);
            x.add16(Obj::X, if flip != 0 { reach } else { reach.wrapping_neg() });
            x.anim_step();
            x.set_bit(Obj::FLAGS, 1, false);
            x.reinsert();
        }
        _ => x.free_eshl(),
    }
}

view! {
    /// Fields of shot 9.
    Shot9 {
        /// Rise of the breath: row of the frames and limits.
        rise, set_rise, RISE: u8 @ 0x3e;
        /// Enemy it is stuck to (16-bit pointer, negative when set).
        enemy, set_enemy, ENEMY: u16 @ 0x46;
    }
}
