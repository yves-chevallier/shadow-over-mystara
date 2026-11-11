use super::*;
use crate::actors::families::set::tables as set_tables;
use crate::actors::families::set::vars;
use crate::world::objects::Obj;
use crate::actors::families::set::fields::SetObj;

/// Script pointers, per variant (data space).
const SCRIPTS: u32 = 0xd4c5e;
/// Variant -> shape of the start / end (bytes, program space).
const START_KIND: u32 = 0xd4a0e;
const END_KIND: u32 = 0xd4bdc;
/// Creation routines by index (longs, data space).
const CREATORS: u32 = 0xd4b92;
const ENTRY_SIZE: u32 = 0x14;
const SCRIPT_END: u16 = 0x8000;
/// RAM byte (A5+0x51): active generators. Byte (A5+0x2c): game mode (0xc: in play).
const ACTIVE_COUNT: u32 = 0x51;
const GAME_MODE: u32 = 0x2c;
const PLAYING: u8 = 0xc;
/// RAM word (A5+0x51e): scroll limit imposed by the wave.
const SCROLL_LIMIT: u32 = 0x51e;
/// Fields of the object: global delay, script pointer, wait, threshold, alive.
const TIMER: u32 = 0x24;
const CURSOR: u32 = 0x20;
const DELAY: u32 = 0x26;
const THRESHOLD: u32 = 0x18;
const ALIVE: u32 = 0x2a;
const DONE: u32 = 0x2c;
const ABORT: u32 = 0x2d;
const WAIT_ALL: u32 = 0x2e;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Obj::HIDDEN) != 0 {
        return;
    }
    match x.u8(Obj::STATE) {
        0 => start(x),
        2 => play(x),
        4 => finish(x),
        _ => x.free(&pools::SET),
    }
}

/// A living player is within range of the generator.
fn player_near<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    for p in players() {
        if p.u8(x.b, pl::ACTIVE) == 0 || p.u16(x.b, pl::HEALTH) == 0 {
            continue;
        }
        let dx = p.u16(x.b, pl::X).wrapping_sub(x.u16(Obj::X)).wrapping_add(0xc0);
        if dx > 0x180 {
            continue;
        }
        let dy = p.u16(x.b, pl::Y).wrapping_sub(x.u16(Obj::Y)).wrapping_add(0x30);
        if dy <= 0x60 {
            return true;
        }
    }
    false
}

fn start<R: Bench>(x: &mut Cx<'_, R>) {
    if !player_near(x) {
        return;
    }
    let n = x.a5_u8(ACTIVE_COUNT).wrapping_add(1);
    x.set_a5_u8(ACTIVE_COUNT, n);
    x.set16(THRESHOLD, 0xffff);
    x.set16(ALIVE, 0);
    x.set8(DONE, 0);
    x.set8(ABORT, 0);
    let variant = x.u8(Obj::VARIANT) as u32;
    let script = set_tables::long(x.b, SCRIPTS + 4 * variant);
    let timer = set_tables::word(x.b, script);
    x.set16(TIMER, timer);
    x.set32(CURSOR, script + 2);
    let me = x.o;
    crate::actors::families::set_residue::begin_wave(x.b, me);
    match set_tables::pc_byte(x.b, START_KIND + variant) {
        0 => x.set_a5_u16(SCROLL_LIMIT, 0x20),
        2 => x.set_a5_u16(SCROLL_LIMIT, 4),
        _ => {}
    }
    x.add8(Obj::STATE, 2);
}

/// End of the run (stop requested, time elapsed or script finished).
fn stop<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(DONE, 1);
    end_script(x);
}

fn end_script<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Obj::STATE, 2);
    x.set16(DELAY, 0);
}

fn play<R: Bench>(x: &mut Cx<'_, R>) {
    if x.a5_u8(GAME_MODE) != PLAYING {
        return;
    }
    if x.a5_u8(vars::FROZEN) != 0 || x.u8(ABORT) != 0 {
        return stop(x);
    }
    let n = x.u16(TIMER).wrapping_sub(1);
    x.set16(TIMER, n);
    if (n as i16) < 0 {
        return stop(x);
    }
    if x.u8(WAIT_ALL) != 0 {
        return;
    }
    let threshold = x.u16(THRESHOLD) as i16;
    if threshold < 0 {
        let d = x.u16(DELAY).wrapping_sub(1);
        x.set16(DELAY, d);
        if (d as i16) >= 0 {
            return;
        }
    } else if threshold < x.u16(ALIVE) as i16 {
        return;
    }
    next_entry(x);
}

/// Executes the current script entry (skipping those that the number of players or the rank exclude).
fn next_entry<R: Bench>(x: &mut Cx<'_, R>) {
    let mut e = x.u32(CURSOR);
    loop {
        let head = set_tables::word(x.b, e);
        if head == SCRIPT_END {
            return end_script(x);
        }
        let players_needed = set_tables::byte(x.b, e + 0xa) as i8;
        let rank_needed = set_tables::word(x.b, e + 2) as i16;
        if players_needed > x.a5_u8(vars::PLAYERS_ALIVE) as i8 || rank_needed > x.a5_u16(vars::RANK) as i16 {
            e += ENTRY_SIZE;
            x.set32(CURSOR, e);
            continue;
        }
        return run_entry(x, e, head);
    }
}

fn run_entry<R: Bench>(x: &mut Cx<'_, R>, e: u32, head: u16) {
    x.set8(WAIT_ALL, 0);
    x.set16(THRESHOLD, 0xffff);
    x.set16(DELAY, head);
    if (head as i16) < 0 {
        let low = head & 0xff;
        if low as u8 as i8 <= -1 {
            x.set8(WAIT_ALL, 1);
        }
        x.set16(THRESHOLD, low);
    }
    let index = set_tables::byte(x.b, e + 0xb) as u32;
    let pool = match set_tables::long(x.b, CREATORS + index) {
        0x22bc => &pools::ENEMY,
        0x23a8 => &pools::BOSS,
        0x2348 => &pools::SET,
        r => unreachable!("wave creator {r:#x}: table 0xd4b92 only has the ENEMY, BOSS, SET allocations"),
    };
    let Some(o) = pool.alloc(x.b) else {
        x.set16(THRESHOLD, 0xffff);
        x.set16(DELAY, 0);
        return;
    };
    o.set_u8(x.b, Obj::ACTIVE, 1);
    let v = set_tables::byte(x.b, e + 0xc);
    o.set_u8(x.b, Obj::KIND, v);
    let v = set_tables::byte(x.b, e + 0xd);
    o.set_u8(x.b, Obj::VARIANT, v);
    let v = set_tables::byte(x.b, e + 0xe);
    o.set_u8(x.b, SetObj::PARAM, v);
    let v = set_tables::byte(x.b, e + 0xf);
    o.set_u8(x.b, SetObj::ATTR, v);
    let v = set_tables::word(x.b, e + 4);
    o.set_u16(x.b, Obj::X, v);
    let v = set_tables::word(x.b, e + 6);
    o.set_u16(x.b, Obj::Z, v);
    let v = set_tables::word(x.b, e + 8);
    o.set_u16(x.b, Obj::Y, v);
    let v = set_tables::word(x.b, e + 0x10);
    o.set_u16(x.b, SetObj::ARG, v);
    x.add16(ALIVE, 1);
    let me = x.o.0 as u16;
    o.set_u16(x.b, SetObj::OWNER, me);
    if set_tables::byte(x.b, e + 0x12) == 0 {
        // position relative to the camera
        let camera = if x.a5_u8(vars::CAMERA_ALT_ACTIVE) != 0 { 0x54e } else { 0x4fe };
        let cam = Obj(A5 + camera);
        let v = o.u16(x.b, Obj::X).wrapping_add(cam.u16(x.b, Obj::X));
        o.set_u16(x.b, Obj::X, v);
        let v = o.u16(x.b, Obj::Y).wrapping_add(cam.u16(x.b, Obj::Z));
        o.set_u16(x.b, Obj::Y, v);
    }
    x.set32(CURSOR, e + ENTRY_SIZE);
}

/// No more living objects: releases the count, lets the variant conclude and releases the scroll limit.
fn finish<R: Bench>(x: &mut Cx<'_, R>) {
    let n = x.u16(TIMER).wrapping_sub(1);
    x.set16(TIMER, n);
    if (n as i16) < 0 {
        x.set8(DONE, 1);
        x.set16(TIMER, 0x7fff);
    }
    if x.u16(ALIVE) != 0 {
        return;
    }
    let n = x.a5_u8(ACTIVE_COUNT).wrapping_sub(1);
    x.set_a5_u8(ACTIVE_COUNT, if (n as i8) < 0 { 0 } else { n });
    let me = x.o;
    crate::actors::families::set_residue::finish_wave(x.b, me);
    let variant = x.u8(Obj::VARIANT) as u32;
    match set_tables::pc_byte(x.b, END_KIND + variant) {
        0 => {
            // sets the scroll limit at the current camera
            let v = x.a5_u16(ram::CAMERA_X);
            x.set_a5_u16(vars::CAMERA_MAX_X, v);
            x.set_a5_u16(SCROLL_LIMIT, 0);
        }
        2 => x.set_a5_u16(SCROLL_LIMIT, 0),
        _ => {}
    }
    x.add8(Obj::STATE, 2);
}
