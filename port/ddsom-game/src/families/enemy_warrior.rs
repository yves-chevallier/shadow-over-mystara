//! "DarkWarrior 1" enemy (ENEMY type 25, 0x76f0e): arms of the dark warrior. See
//! `docs/families/enemy.md`.
//!
//! The creator (0x76ec4, not rewritten) spawns two instances, variant 0 (left side) and 1 (right
//! side), linked to their host by `+0x46`. An arm has no AI: it positions itself next to the host
//! (`+0xa8..+0xac` of the host, offset of 0x82 pixels) and plays what the host's state (`+0xa2`) commands.
//! Types 22, 23, 24 and 26 are graphical "doubles" of this type (`enemy_double.rs`).
//!
//! The position is computed in two steps: the speeds `+0x14..+0x1e` are integrated into offsets
//! `+0xb4` (X), `+0xb6` (altitude), `+0xb8` (depth) that `apply_motion` then adds to the position.

use crate::families::cx::Bench;
use crate::objects::Current;
use super::cx::{handlers, Cx};
use super::enemy_common::*;
use crate::objects::Obj;

/// Animation sets: body (0x17e03a) and hits (0x17e048).
const SET_BODY: u32 = 0x17e03a;
const SET_STRIKE: u32 = 0x17e048;
/// Table of vertical undulations (signed bytes, read in the opcode space).
const BOB_TABLE: u32 = 0x7759e;
/// Movement scenarios: table of words (offsets) followed by bytes (dx, dz) per frame.
const SCRIPTS: u32 = 0x77626;

mod g {
    pub const STRIKE_KIND: u32 = 0x44;
    pub const FLAGS: u32 = 0x1;
    pub const ANIM_SET: u32 = 0xa0;
    pub const ANIM_NUM: u32 = 0xa1;
    pub const HOST_STATE: u32 = 0xa2;
    pub const BOB_PENDING: u32 = 0xa4;
    pub const BOB_PHASE: u32 = 0xa5;
    pub const HOST_BUSY: u32 = 0xa6;
    pub const SCRIPT_LEFT: u32 = 0xae;
    pub const SLOT_X: u32 = 0xa8;
    pub const SLOT_Z: u32 = 0xaa;
    pub const SLOT_Y: u32 = 0xac;
    pub const PREV_STATE: u32 = 0xb0;
    pub const OFFSET_X: u32 = 0xb4;
    pub const OFFSET_Z: u32 = 0xb6;
    pub const OFFSET_Y: u32 = 0xb8;
    pub const HOST: u32 = 0x46;
}
use g::*;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    let Some(host) = live_host(x) else {
        x.set8(f::STATE, st::DEAD);
        return free(x);
    };
    let _ = host;
    match x.u8(f::STATE) {
        st::INIT => init(x),
        st::ACTIVE => active(x),
        st::HIT => hit(x),
        st::DEAD => free(x),
        _ => {}
    }
}

/// The host (`+0x46`) if it is still active.
fn live_host<R: Bench>(x: &mut Cx<'_, R>) -> Option<Obj> {
    let w = x.u16(HOST);
    if (w as i16) >= 0 {
        return None;
    }
    let h = x.linked(HOST);
    if h.u8(x.b, 0) == 0 {
        return None;
    }
    Some(h)
}

fn host<R: Bench>(x: &mut Cx<'_, R>) -> Obj {
    x.linked(HOST)
}

/// Starts animation `n` of the body set (0x776fe) or the hit set (0x77708) if it is not already
/// the current one.
fn play<R: Bench>(x: &mut Cx<'_, R>, set_id: u8, n: u8) {
    let set = if set_id == 0 { SET_BODY } else { SET_STRIKE };
    if set_id != x.u8(ANIM_SET) || n != x.u8(ANIM_NUM) {
        x.set8(ANIM_SET, set_id);
        x.set8(ANIM_NUM, n);
        x.anim_start(set, (n as u32) as u16);
    }
}

fn body<R: Bench>(x: &mut Cx<'_, R>, n: u8) {
    play(x, 0, n);
}

fn strike<R: Bench>(x: &mut Cx<'_, R>, n: u8) {
    play(x, 2, n);
}

fn set_state_long<R: Bench>(x: &mut Cx<'_, R>, v: u32) {
    x.set32(f::STATE, v);
}

// --- state 0: creation --------------------------------------------------------------------------

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    x.set8(f::BOX_SET, 0x46);
    x.set32(f::IMMUNITY, 0xffff_ffff);
    x.set32(PREV_STATE, 0);
    x.set16(OFFSET_X, 0);
    x.set16(OFFSET_Z, 0);
    x.set16(OFFSET_Y, 0);
    // variant 0 looks left
    let flip = if x.u8(Obj::VARIANT) == 0 { 0x20 } else { 0 };
    x.set8(f::FLIP, flip);
    x.set16(ANIM_SET, 0xffff);
    body(x, 5);
}

// --- state 2: active -----------------------------------------------------------------------------

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(0x38, 0);
    if paused(x) {
        return end_part(x);
    }
    follow_host(x);
    let h = host(x);
    let (px, pz, py) = (x.u16(SLOT_X), x.u16(SLOT_Z), x.u16(SLOT_Y));
    x.set16(0x8, px);
    x.set16(0xc, pz);
    x.set16(0x10, py);
    let hs = h.u8(x.b, HOST_STATE);
    if hs != x.u8(HOST_STATE) {
        x.set8(HOST_STATE, hs);
        x.set8(f::SUB, 0);
        x.set16(f::SUB2, 0);
    }
    match hs {
        4 => {
            let power = hit_power(x);
            x.set8(STRIKE_KIND, power);
            if x.u8(Obj::VARIANT) == 0 { swing(x) } else { drift(x) }
        }
        5 => {
            let power = hit_power(x);
            x.set8(STRIKE_KIND, power);
            if x.u8(Obj::VARIANT) != 0 { swing(x) } else { drift(x) }
        }
        6 => {
            x.set8(STRIKE_KIND, 0x4a);
            if x.u8(Obj::VARIANT) != 0 { rise_b(x) } else { rise(x) }
        }
        7 => {
            x.set8(STRIKE_KIND, 0x4a);
            if x.u8(Obj::VARIANT) == 0 { rise_b(x) } else { rise(x) }
        }
        8 => {
            x.set8(STRIKE_KIND, 0x4a);
            rise(x)
        }
        _ => idle(x),
    }
    apply_motion(x);
    let state = x.u32(f::STATE);
    x.set32(PREV_STATE, state);
}

/// Hit strength: 0x4a, or 0x5a when the host is not of type 0xf.
fn hit_power<R: Bench>(x: &mut Cx<'_, R>) -> u8 {
    let h = host(x);
    if h.u8(x.b, 2) == 0xf { 0x4a } else { 0x5a }
}

/// Common ending: "host visible" flag, then re-listing (0x77016).
fn end_part<R: Bench>(x: &mut Cx<'_, R>) {
    x.set_bit(FLAGS, 1, false);
    let h = host(x);
    if h.u8(x.b, 0x42) != 0 {
        x.set_bit(FLAGS, 1, true);
    }
    x.reinsert();
}

/// 0x774a8: the arm positions itself next to the host, 0x82 pixels on either side depending on its side.
fn follow_host<R: Bench>(x: &mut Cx<'_, R>) {
    let h = host(x);
    let (px, pz, py) = (h.u16(x.b, SLOT_X), h.u16(x.b, SLOT_Z), h.u16(x.b, SLOT_Y));
    let dx: u16 = if x.u8(f::FLIP) != 0 { 0xff7e } else { 0x82 };
    x.set16(SLOT_X, px.wrapping_add(dx));
    x.set16(SLOT_Z, pz);
    x.set16(SLOT_Y, py);
}

/// 0x774e0: adds the offsets to the position; the altitude does not go below 0 and the
/// depth keeps the arm at least 0x60 pixels below the top of the screen.
fn apply_motion<R: Bench>(x: &mut Cx<'_, R>) {
    let px = x.u16(0x8).wrapping_add(x.u16(OFFSET_X));
    x.set16(0x8, px);
    let mut z = x.u16(0xc).wrapping_add(x.u16(OFFSET_Z));
    if (z as i16) < 0 {
        let off = x.u16(OFFSET_Z).wrapping_add(z);
        x.set16(OFFSET_Z, off);
        z = 0;
    }
    x.set16(0xc, z);
    let mut y = x.u16(0x10).wrapping_add(x.u16(OFFSET_Y));
    let cam_y = var::SCROLL_Y.get(x.b);
    let t = (x.u16(0xc).wrapping_add(y).wrapping_sub(cam_y)) as i16;
    if t < 0x60 {
        y = cam_y.wrapping_add(0x60).wrapping_sub(x.u16(0xc));
    }
    x.set16(0x10, y);
}

/// Adds the offsets to the position, with no bounds (states 4 to 8: 0x77252).
fn add_offsets<R: Bench>(x: &mut Cx<'_, R>) {
    for (off, o) in [(0x8, OFFSET_X), (0xc, OFFSET_Z), (0x10, OFFSET_Y)] {
        let v = x.u16(off).wrapping_add(x.u16(o));
        x.set16(off, v);
    }
}

/// 0x776a2..: integrates a speed (`vel`, acceleration `vel + 2`) into an integer offset `int` and the
/// fraction `frac` of the position.
fn integrate<R: Bench>(x: &mut Cx<'_, R>, vel: u32, int: u32, frac: u32) {
    let v = x.u16(vel).wrapping_add(x.u16(vel + 2));
    x.set16(vel, v);
    let delta = ((v as i16 as i32) << 8) as u32;
    let d1 = ((x.u16(int) as u32) << 16 | x.u16(frac) as u32).wrapping_add(delta);
    x.set16(frac, d1 as u16);
    x.set16(int, (d1 >> 16) as u16);
}

/// 0x77684: integrates the three speeds.
fn integrate_all<R: Bench>(x: &mut Cx<'_, R>) {
    integrate(x, 0x1c, OFFSET_Y, 0x12);
    integrate(x, 0x14, OFFSET_X, 0xa);
    integrate(x, 0x18, OFFSET_Z, 0xe);
}

/// 0x77032: brings the offsets closer to the targets `(d0, d1, d2)` by half each frame; true when
/// all three have arrived.
fn converge<R: Bench>(x: &mut Cx<'_, R>, targets: [i16; 3]) -> bool {
    let mut arrived = 0;
    for (t, off) in targets.into_iter().zip([OFFSET_X, OFFSET_Z, OFFSET_Y]) {
        let diff = t.wrapping_sub(x.u16(off) as i16);
        if diff == 0 {
            arrived += 1;
            continue;
        }
        let mut stepv = diff >> 1;
        if stepv == 0 {
            stepv = 1;
        }
        let v = x.u16(off).wrapping_add(stepv as u16);
        x.set16(off, v);
        if diff.wrapping_abs() <= 1 {
            arrived += 1;
        }
    }
    arrived >= 3
}

/// 0x775de: one step of movement scenario `n`; true when it is finished. The first frame reads
/// the first pair of bytes; the following ones resume at the end (index = remaining counter).
fn script_step<R: Bench>(x: &mut Cx<'_, R>, n: u32) -> bool {
    let offset = x.rom16(SCRIPTS + 2 * n) as i16 as i32 as u32;
    let a2 = SCRIPTS.wrapping_add(offset);
    let cur = x.u16(SCRIPT_LEFT);
    if (cur as i16) < 0 {
        x.set16(SCRIPT_LEFT, 0xffff);
        return true;
    }
    if cur == 0 {
        let count = x.data16(a2.wrapping_sub(2));
        x.set16(SCRIPT_LEFT, count);
    }
    let idx = cur.wrapping_mul(2) as u32;
    let mut dx = x.data8(a2.wrapping_add(idx)) as i8 as i16;
    if x.u8(f::FLIP) != 0 {
        dx = -dx;
    }
    x.set16(OFFSET_X, dx as u16);
    let dz = x.data8(a2.wrapping_add(idx).wrapping_add(1)) as i8 as i16;
    x.set16(OFFSET_Z, dz as u16);
    let left = x.u16(SCRIPT_LEFT).wrapping_sub(1);
    x.set16(SCRIPT_LEFT, left);
    if (left as i16) <= 0 {
        x.set16(SCRIPT_LEFT, 0xffff);
        return true;
    }
    false
}

// --- host states without gesture (0 to 3, 9 and above) ----------------------------------------------

/// 0x76ff4: the arm stands next to the host and undulates.
fn idle<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB) == 0 {
        x.add8(f::SUB, 2);
        body(x, 5);
        x.set8(0x3b, 0);
    }
    x.set8(BOB_PENDING, 1);
    bob(x);
    damp(x);
    end_part(x);
}

/// 0x77562: vertical undulation while the host acts.
fn bob<R: Bench>(x: &mut Cx<'_, R>) {
    let h = host(x);
    if h.u8(x.b, 5) == 0 {
        return;
    }
    if x.u8(BOB_PENDING) == 0 {
        return x.set8(BOB_PHASE, 0);
    }
    x.set8(BOB_PENDING, 0);
    let phase = x.u8(BOB_PHASE).wrapping_add(1);
    x.set8(BOB_PHASE, phase);
    let i = (phase as u16 >> 2 & 0x3f) as u32;
    let d = x.rom8(BOB_TABLE + i) as i8 as i16 as u16;
    x.add16(0x10, d);
}

/// 0x77530: the offsets damp by a sixteenth each frame.
fn damp<R: Bench>(x: &mut Cx<'_, R>) {
    for off in [OFFSET_X, OFFSET_Z, OFFSET_Y] {
        let v = x.u16(off) as i16;
        if v != 0 {
            let mut d = v >> 4;
            if d == 0 {
                d = 1;
            }
            x.set16(off, v.wrapping_sub(d) as u16);
        }
    }
}

// --- host states 4 and 5: hit -----------------------------------------------------------------

/// 0x770e8: the striking arm (`+5` = 0 approach, 2 wind-up, 4 blow, 6 return).
fn swing<R: Bench>(x: &mut Cx<'_, R>) {
    swing_step(x);
    end_part(x);
}

fn swing_step<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB) {
        0 => {
            if !converge(x, [0, 0x20, 0]) {
                return;
            }
            x.add8(f::SUB, 2);
            lib::sound(x, crate::sound::CMD_380);
            x.set16(0x1c, 0);
            x.set16(0x1e, 0);
            x.set16(0x18, (-0x180i16) as u16);
            x.set16(0x1a, 0);
            let (mut vx, mut vy) = (-0xc00i16, 0x80i16);
            if x.u8(f::FLIP) == 0 {
                vx = -vx;
                vy = -vy;
            }
            x.set16(0x14, vx as u16);
            x.set16(0x16, vy as u16);
            strike(x, 0xb);
        }
        2 => {
            sparks(x);
            integrate_all(x);
            let mut vx = x.u16(0x14) as i16;
            let mut off = x.u16(OFFSET_X) as i16;
            if x.u8(f::FLIP) == 0 {
                vx = -vx;
                off = -off;
            }
            if vx < 0 || off <= -0x10 {
                return;
            }
            for o in [0x16, 0x1c, 0x1e, 0x18, 0x1a] {
                x.set16(o, 0);
            }
            x.add8(f::SUB, 2);
        }
        4 => {
            sparks(x);
            x.anim_step();
            integrate_all(x);
            let mut off = x.u16(OFFSET_X) as i16;
            if x.u8(f::FLIP) == 0 {
                off = -off;
            }
            if off <= 0x30 {
                return;
            }
            x.add8(f::SUB, 2);
            strike(x, 0xc);
        }
        6 => {
            if x.anim_step() {
                x.add8(f::SUB, 2);
                let h = host(x);
                h.set_u16(x.b, HOST_BUSY, 0);
            }
        }
        _ => {}
    }
}

/// Sparks of the strike (0xb10c8) and dispersal of the effect.
/// Original glitch: the dispersed object is A1 after the shock, tested by its word (`move.w A1,D0`): the slot that
/// the refused allocation leaves, or, in scene 41 (no shock, A1 not written), the host that
/// `follow_host` (0x774a8) loaded
/// there.
fn sparks<R: Bench>(x: &mut Cx<'_, R>) {
    let e = match lib::shock(x, 3, 0, 0, 0) {
        lib::Impact::Created(o) | lib::Impact::Refuse(o) => o,
        lib::Impact::Nothing => host(x),
    };
    if e.word() == 0 {
        return;
    }
    e.set_u16(x.b, 0xc, 0);
    let dy = (random(x) & 7) as u16;
    let ny = e.u16(x.b, 0x10).wrapping_add(dy.wrapping_sub(3));
    e.set_u16(x.b, 0x10, ny);
    let dx = (random(x) & 0xf) as u16;
    let nx = e.u16(x.b, 0x8).wrapping_add(dx.wrapping_sub(7));
    e.set_u16(x.b, 0x8, nx);
}

/// 0x77244: the non-striking arm slowly descends (`+5` = 0 start, 2 acceleration).
fn drift<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB) {
        0 => {
            x.add8(f::SUB, 2);
            x.set32(OFFSET_X, 0);
            x.set32(OFFSET_Y, 0);
            x.set16(OFFSET_Z, 0);
            x.set16(0x18, 1);
            body(x, 6);
        }
        2 => {
            let v = x.u16(0x18).wrapping_add(x.u16(OFFSET_Z));
            x.set16(OFFSET_Z, v);
            if (v as i16) >= 0x10 {
                x.add8(f::SUB, 2);
                x.set16(OFFSET_Z, 0x10);
            }
        }
        _ => {}
    }
    add_offsets(x);
    end_part(x);
}

// --- host states 6 to 8: entrance on stage ------------------------------------------------------

/// Variant that stays on the ground while the host rises: same movement as `drift`.
fn rise_b<R: Bench>(x: &mut Cx<'_, R>) {
    drift(x);
}

/// 0x772d6: follows the host's scenario (`+5` = 0 start, 2 first scenario, 4 end, 6 second).
fn rise<R: Bench>(x: &mut Cx<'_, R>) {
    rise_step(x);
    add_offsets(x);
    end_part(x);
}

fn rise_step<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB) {
        0 => {
            x.add8(f::SUB, 2);
            x.set32(OFFSET_X, 0);
            x.set32(0xb6, 0);
            x.set32(OFFSET_Y, 0);
            lib::sound(x, crate::sound::CMD_38C_48C);
            x.set16(SCRIPT_LEFT, 0);
            body(x, 6);
        }
        2 => {
            if !script_step(x, 1) {
                return;
            }
            x.add8(f::SUB, 2);
            x.set16(0xa6, 0x50);
            strike(x, 0x25);
            crate::families::effects_creators::scenery_7(x.b, x.o);
        }
        4 => {
            if !x.anim_step() {
                return;
            }
            x.add8(f::SUB, 2);
            x.set16(SCRIPT_LEFT, 0);
            body(x, 3);
            if var::SCENE.get(x.b) != 0x2e {
                return finale(x);
            }
            crate::families::eshl_creators::single_40(&mut crate::families::effects_creators::leftovers::Pure, x.b, x.o);
            lib::sound(x, crate::sound::CMD_385_485);
            rise_script_two(x);
        }
        6 => rise_script_two(x),
        _ => {}
    }
}

fn rise_script_two<R: Bench>(x: &mut Cx<'_, R>) {
    if script_step(x, 2) {
        x.add8(f::SUB, 2);
        let h = host(x);
        h.set_u16(x.b, HOST_BUSY, 0);
    }
}

/// 0x77470: death of the host, the three pieces of the variant leave.
fn finale<R: Bench>(x: &mut Cx<'_, R>) {
    let base = if x.u8(Obj::VARIANT) == 0 { 0 } else { 3 };
    for k in 0..3 {
        *x.c.d4() = base + k;
        crate::families::effects_creators::eshl_slash(x.b, x.o, (base + k) as u8);
    }
}

// --- state 4: hit ----------------------------------------------------------------------------

fn hit<R: Bench>(x: &mut Cx<'_, R>) {
    let h = host(x);
    let freeze = x.u8(f::FREEZE) as i8;
    if freeze > 0 {
        x.set8(0, 2);
        let left = x.u8(f::FREEZE).wrapping_sub(2);
        x.set8(f::FREEZE, left);
        return end_part(x);
    }
    x.set8(0, 1);
    let mut hp = x.u16(0x60) as i16;
    if hp > 0 {
        hp = hp.wrapping_sub(x.u16(f::HP) as i16);
        if hp > 0 {
            let host_hp = h.u16(x.b, f::HP).wrapping_sub(hp as u16);
            h.set_u16(x.b, f::HP, host_hp);
            if (host_hp as i16) < 0 {
                h.set_u16(x.b, f::HP, 0);
            }
        }
    }
    let host_hp = h.u16(x.b, f::HP);
    x.set16(0x60, host_hp);
    x.set16(f::HP, host_hp);
    x.set16(0x64, host_hp);
    let mut next = 0x0200_0000;
    let prev = x.u32(PREV_STATE);
    if prev != 0 && (prev >> 24) as u8 == 2 {
        next = prev;
    }
    set_state_long(x, next);
}

handlers! {
    0x76f0e "enemy_25" run,
}
