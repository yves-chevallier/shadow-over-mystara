use super::*;
use crate::families::shell::tables::{tables, Read};

const ANIMS: u32 = 0x170222;
const HOST: u32 = 0x42;
const ARMED: u32 = 0x58;
const TRIGGER: u32 = 0x59;
const DONE: u32 = 0x5a;
const SHARDS_LEFT: u32 = 0x46;
const SHARD_DELAY: u32 = 0x48;
const ANGLE: u32 = 0x5e;
const ANGLE_STEP: u32 = 0x5f;
/// Substates (`+5`).
const IDLE: u8 = 0;
const FIGHT: u8 = 2;
const END: u8 = 4;
/// Phases of the salvo (`+6`).
const WIND_UP: u8 = 0;
const WAIT_TRIGGER: u8 = 2;
const BURST: u8 = 4;
const PAUSE: u8 = 6;
const RECOVER: u8 = 8;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    follow_host(x);
    match x.u8(0x4) {
        0 => {
            x.add8(0x4, 2);
            x.set8(f::BOX_SET, 7);
            ai_anim(x, ANIMS, 1);
        }
        2 => live(x),
        _ => x.free_eshl(),
    }
    if x.u8(0) != 0 {
        x.reinsert();
    }
}

/// Aligns in front of the host (32 pixels on the facing side, 64 in height, one notch closer).
fn follow_host<R: Bench>(x: &mut Cx<'_, R>) {
    let host = x.linked(HOST);
    copy_xyz16(x, host);
    x.add16(0x10, 1);
    let dx = if x.u8(f::FLIP) != 0 { neg(0x20) } else { 0x20 };
    x.add16(0x8, dx);
    x.add16(0xc, 0x40);
}

fn live<R: Bench>(x: &mut Cx<'_, R>) {
    let host = x.linked(HOST);
    if host.u8(x.b, 4) == 4 {
        return x.free_eshl();
    }
    match x.u8(0x5) {
        IDLE => idle(x, host),
        FIGHT => fight(x),
        END => {
            if s16(host.u16(x.b, 0xc)) <= 0 {
                x.free_eshl();
            }
        }
        _ => {}
    }
}

fn idle<R: Bench>(x: &mut Cx<'_, R>, host: Obj) {
    if x.u8(ARMED) != 0 {
        if x.u8(0x3) != 2 {
            return x.free_eshl();
        }
        return x.add8(0x5, 2);
    }
    if x.u8(0x3) == 0 {
        return;
    }
    let frame = host.u8(x.b, 0x33) & 0x7f;
    let anim = tables().shot42_anims_of_host_frame.at8(x.b, frame as u32);
    ai_anim(x, ANIMS, anim as u32);
}

fn fight<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x6) {
        WIND_UP => {
            if ai_step(x) {
                x.add8(0x6, 2);
            }
        }
        WAIT_TRIGGER => {
            if x.u8(TRIGGER) == 0 {
                return;
            }
            x.add8(0x6, 2);
            x.set16(SHARDS_LEFT, 5);
            x.set16(SHARD_DELAY, 1);
            // starting angle and rotation direction: according to the owner's side and height
            let (mut a, mut step, mut a2, mut step2) = (0x16u8, 1u8, 0x1au8, 0xffu8);
            if x.u8(f::FLIP) != 0 {
                (a, step, a2, step2) = (0xa, 0xff, 6, 1);
            }
            let owner = x.linked(f::OWNER);
            if s16(owner.u16(x.b, 0x10)) > s16(x.u16(0x10)) {
                (a, step) = (a2, step2);
            }
            x.set8(ANGLE, a);
            x.set8(ANGLE_STEP, step);
        }
        BURST => {
            let t = x.u16(SHARD_DELAY).wrapping_sub(1);
            x.set16(SHARD_DELAY, t);
            if s16(t) > 0 {
                return;
            }
            spawn_shard(x);
            x.set16(SHARD_DELAY, 8);
            let n = x.u16(SHARDS_LEFT).wrapping_sub(1);
            x.set16(SHARDS_LEFT, n);
            if s16(n) > 0 {
                return;
            }
            x.add8(0x6, 2);
            x.set16(SHARDS_LEFT, 0x14);
        }
        PAUSE => {
            let n = x.u16(SHARDS_LEFT).wrapping_sub(1);
            x.set16(SHARDS_LEFT, n);
            if s16(n) > 0 {
                return;
            }
            x.add8(0x6, 2);
            ai_anim(x, ANIMS, 6);
        }
        RECOVER => {
            if ai_step(x) {
                x.add8(0x5, 2);
                x.set8(DONE, 1);
            }
        }
        _ => {}
    }
}

/// Creates a burst (type 43) in front of the attack, with the current angle, then rotates the angle.
fn spawn_shard<R: Bench>(x: &mut Cx<'_, R>) {
    let Some(c) = x.alloc(&pools::ESHL) else { return };
    c.set_u8(x.b, 0, 1);
    c.set_u8(x.b, 2, 0x2b);
    let (px, pz) = (x.u16(0x8), x.u16(0xc));
    c.set_u16(x.b, 0x8, px);
    c.set_u16(x.b, 0xc, pz.wrapping_sub(0x40));
    let y = x.u32(0x10);
    c.set_u32(x.b, 0x10, y);
    let y_low = c.u16(x.b, 0x12).wrapping_add(1);
    c.set_u16(x.b, 0x12, y_low);
    let flip = x.u8(f::FLIP);
    c.set_u8(x.b, f::FLIP, flip);
    let dx = if flip != 0 { 0x2a } else { neg(0x2a) };
    let cx = c.u16(x.b, 0x8).wrapping_add(dx);
    c.set_u16(x.b, 0x8, cx);
    let angle = x.u8(ANGLE);
    c.set_u8(x.b, 0x2d, angle);
    let step = x.u8(ANGLE_STEP);
    x.set8(ANGLE, angle.wrapping_add(step) & 0x1f);
}
