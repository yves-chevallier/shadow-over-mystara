//! Boss of types 11 and 12 (0x8d59e), stages 42 and 40: the snakes (mobile head and twelve rings). See
//! `docs/families/boss.md`.
//!
//! The head is the boss (A0); the twelve rings are ESHL objects of type 5 (`+0x2f` = rank 0 to 11, `+0x46` = the
//! head) created at initialization by 0x9e426. Each frame, `chain::update` repositions the rings behind the head
//! according to the ROM tables (0x1069f8: 32-byte rows per pose `+0xaa`; 0x106cb8: paths), and the head is copied
//! from the last ring. Types 11 (`+2` = 0xb) and 12 (0xc) share everything except the animation sets and a few
//! attacks.
//!
//! | `+5` | File | Role |
//! |---|---|---|
//! | 0 | `behave` | arrival: run-up, fall, bounce, first walk |
//! | 2, 8, 10, 12 | `behave` | pursuit: choice of the attack or maneuver (`tests`) |
//! | 4 | `attacks` | eight attacks (`+6` = 0 to 14) |
//! | 6 | `behave` | wait with no player |
//! | state 4 | `hit` | reactions to hits, death |
//! | all | `chain` | positioning of the rings |
//!
//! Animations: 0x17b9c2, 0x17b9d6, 0x17ba32 (type 12) and 0x17c8ca, 0x17c8de, 0x17c93a (type 11), chosen by `play`.

use crate::families::cx::Bench;
use super::boss_common::*;
use super::cx::handlers;
use crate::mem::{rd16, rd32, rd8, wr16, wr8, A5};

mod attacks;
mod behave;
mod chain;
mod hit;

mod g {
    pub const REACTION: u32 = 0x49;
    pub const STUN: u32 = 0x56;
    pub const ANIM_KIND: u32 = 0xa0;
    pub const ANIM_N: u32 = 0xa1;
    pub const SAVE_X: u32 = 0xa2;
    pub const SAVE_Z: u32 = 0xa4;
    pub const SAVE_Y: u32 = 0xa6;
    pub const SEG: u32 = 0xa8;
    pub const FRAME: u32 = 0xaa;
    pub const FRAME_SRC: u32 = 0x3e;
    pub const NO_ZONE: u32 = 0xb7;
    pub const SLOW: u32 = 0xb9;
    pub const TIMER: u32 = 0xb0;
}
use g::*;
use crate::objects::pools;
use crate::objects::Current;

/// The leftovers D2 to D5 (re-read by the following handlers); D0, D1 and D6 are re-read by no one.
fn remainder<'a, R: Bench>(x: &'a mut Cx<'_, R>, n: usize) -> Option<&'a mut u32> {
    match n {
        2 => Some(x.c.d2()),
        3 => Some(x.c.d3()),
        4 => Some(x.c.d4()),
        5 => Some(x.c.d5()),
        _ => None,
    }
}

/// `move.b #v,Dn`: the low-order byte of a data register (kept if it is a leftover).
fn setb<R: Bench>(x: &mut Cx<'_, R>, n: usize, v: u8) {
    if let Some(r) = remainder(x, n) {
        *r = *r & !0xff | v as u32;
    }
}

/// `move.w #v,Dn`: the low-order word of a data register (kept if it is a leftover).
fn setw<R: Bench>(x: &mut Cx<'_, R>, n: usize, v: u16) {
    if let Some(r) = remainder(x, n) {
        *r = *r & 0xffff_0000 | v as u32;
    }
}

/// `asr.w #k,Dn`.
fn asr_w(v: u16, k: u32) -> u16 {
    ((v as i16) >> k) as u16
}

/// Types 12 and 11 are distinguished by byte `+2`.
fn is_12<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    x.u8(2) == 0xc
}

/// `move.b (0x3e,A0),(0xaa,A0)`.
fn sync<R: Bench>(x: &mut Cx<'_, R>) {
    let v = x.u8(FRAME_SRC);
    x.set8(FRAME, v);
}

// --- animations (0x8f508, 0x8f530, 0x8f558) ------------------------------------------------------------------

/// Starts script `n` (already in D0) of set `kind` (0, 2 or 4: D1b) for the type, unless it is already running.
fn play<R: Bench>(x: &mut Cx<'_, R>, kind: u8, n: u32) {
    let t12 = is_12(x);
    let table = match (kind, t12) {
        (0, true) => 0x17b9c2,
        (0, false) => 0x17c8ca,
        (2, true) => 0x17b9d6,
        (2, false) => 0x17c8de,
        (_, true) => 0x17ba32,
        (_, false) => 0x17c93a,
    };
    if x.u8(ANIM_KIND) == kind && x.u8(ANIM_N) == n as u8 {
        return;
    }
    x.set8(ANIM_KIND, kind);
    x.set8(ANIM_N, n as u8);
    x.anim_start(table, n as u16);
}

/// 0x8f508.
fn body<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    play(x, 0, n)
}

/// 0x8f530.
fn act<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    play(x, 2, n)
}

/// 0x8f558.
fn hurt<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    play(x, 4, n)
}

/// 0x8eb28: byte `+0x44` according to the type and `n`.
fn set44<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    let d = (n as u16).wrapping_mul(2);
    let base = if is_12(x) { 0x8eb43 } else { 0x8eb42 };
    let v = x.rom8(base + d as u32);
    x.set8(0x44, v);
}

// --- state changes (0x8eac8...) ---------------------------------------------------------------------------

/// 0x8eac8: no target left, wait (state 2, group 6).
fn to_wait<R: Bench>(x: &mut Cx<'_, R>) {
    x.set32(f::STATE, 0x0206_0000);
    body(x, 0)
}

/// 0x8ead6: return to the pursuit.
fn to_chase<R: Bench>(x: &mut Cx<'_, R>) {
    x.set32(f::STATE, 0x0202_0000);
    x.set8(0x79, 0x3f);
    body(x, 0)
}

/// 0x8eaea : demi-tour.
fn to_turn<R: Bench>(x: &mut Cx<'_, R>) {
    x.set32(f::STATE, 0x0202_0000);
    let flip = x.u8(f::FLIP) ^ 0x20;
    x.set8(f::FLIP, flip);
    body(x, 1)
}

/// 0x8eafe: attack 2 (group 4, step 4).
fn to_attack_2<R: Bench>(x: &mut Cx<'_, R>) {
    x.set32(f::STATE, 0x0204_0400);
    body(x, 8)
}

/// 0x8eb0c.
fn to_attack_3<R: Bench>(x: &mut Cx<'_, R>) {
    x.set32(f::STATE, 0x0204_0600);
    body(x, 0)
}

/// 0x8eb1a.
fn to_attack_7<R: Bench>(x: &mut Cx<'_, R>) {
    x.set32(f::STATE, 0x0204_0e00);
    body(x, 0)
}

/// 0x8d592: a ring is missing, the snake dies (state 4, group 6).
fn lost<R: Bench>(x: &mut Cx<'_, R>) {
    x.set32(f::STATE, 0x0406_0000);
}

/// 0x8ebd0 : vitesses nulles.
fn stop<R: Bench>(x: &mut Cx<'_, R>) {
    x.set32(0x14, 0);
    x.set32(0x18, 0);
    x.set32(0x1c, 0);
}

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    x.tick_counters();
    restore(x);
    set44(x, 0);
    x.set8(NO_ZONE, 0);
    let state = x.u8(f::STATE) as u32;
    let off = x.rom16(0x8d5ca + state);
    setw(x, 1, off);
    match state {
        0 => init(x),
        2 => active(x),
        4 => hit::run(x),
        _ => x.free(&pools::BOSS),
    }
    if (x.u8(SLOW) as i8) > 0 {
        sub8(x, SLOW, 1);
        x.set8(0x38, 0);
    }
    if x.u8(f::STATE) == 0 || x.u8(0) == 0 {
        return;
    }
    if x.u8(f::STATE) != 4 && var::PAUSE.get(x.b) != 0 {
        return;
    }
    if x.u8(NO_ZONE) == 0 {
        lib::keep_on_screen(x, 0x30);
        lib::collide_terrain(x);
    }
    keep_in_screen(x);
    chain::update(x);
    if Fl::sub8(x.u8(f::STATE), 6).ge() {
        return;
    }
    x.reinsert();
}

/// 0x8d60e: the snake does not leave the screen through the left or the top.
fn keep_in_screen<R: Bench>(x: &mut Cx<'_, R>) {
    let cam = if var::CAMERA_SELECT.get(x.b) != 0 { A5 + 0x54e } else { A5 + 0x4fe };
    let mut d0 = rd16(x.b, cam + 8);
    if !Fl::sub16(d0, x.u16(0x8)).le() {
        x.set16(0x8, d0);
    }
    d0 = d0.wrapping_add(0x180);
    if !Fl::sub16(d0, rd16(x.b, cam + 8)).ge() {
        x.set16(0x8, d0);
    }
    d0 = rd16(x.b, cam + 0xc);
    if !Fl::sub16(d0, x.u16(0x10)).le() {
        x.set16(0x10, d0);
    }
}

/// 0x8d65a: the position is that of the previous frame (copied by `chain::update`).
fn restore<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::STATE) == 0 || x.u8(0) == 0 {
        return;
    }
    if x.u8(f::STATE) != 4 && var::PAUSE.get(x.b) != 0 {
        return;
    }
    let (a, b, c) = (x.u16(SAVE_X), x.u16(SAVE_Z), x.u16(SAVE_Y));
    x.set16(0x8, a);
    x.set16(0xc, b);
    x.set16(0x10, c);
}

// --- state 0: creation --------------------------------------------------------------------------

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    if var::PAUSE.get(x.b) != 0 {
        return;
    }
    // D0.w = the state (0, set by `run`): compared with the maximum number of shots of type 5
    if !crate::families::eshl_creators::twelve_5(&mut crate::families::effects_creators::leftovers::Pure, x.b, x.o, 0) {
        return;
    }
    x.add8(f::STATE, 2);
    x.set8(f::SUB, 0);
    x.set16(f::SUB2, 0);
    x.set32(f::IMMUNITY, 0xefff_ffff);
    x.set8(0x2e, 0x2c);
    x.set8(0x79, 0x3f);
    x.set8(0x8e, 3);
    lib::sound(x, crate::sound::CMD_20);
    lib::empty_popups(x);
    lib::clear_popup(x);
    x.set_a5_u8(0x50, 1);
    x.set8(0x93, 1);
    x.set8(0x9a, 0xff);
    x.set8(0x7e, 0x4e);
    x.set8(0x81, 0);
    x.set8(0x80, 0x26);
    x.set8(0x9e, 2);
    x.set8(0x9f, 1);
    // Original glitch: `cmpi.b #0xc,D0b` was probably meant to distinguish type 12, but D0 has just been zeroed by
    // 0x19e10 (`clear_popup`): both types take these settings.
    {
        x.set8(0x81, 0);
        x.set8(0x80, 0x27);
        x.set8(0x9e, 1);
        x.set8(0x9f, 2);
    }
    x.set8(SLOW, 0);
    lib::initial_hp_boss(x);
    lib::create_set_14(x);
    x.set_a5_u32(0x38, 0x0459_5900);
    var::SCENE_LEVEL.set(x.b, 0);
    lib::create_set_15(x);
    lib::clear_popup(x);
    lib::empty_popups(x);
    lib::clear_all_popups(x);
    let (a, b, c) = (x.u16(0x8), x.u16(0xc), x.u16(0x10));
    x.set16(SAVE_X, a);
    x.set16(SAVE_Z, b);
    x.set16(SAVE_Y, c);
    x.set8(FRAME, 0);
    x.set8(0xab, 0);
    x.set8(0xac, 0);
    x.set16(ANIM_KIND, 0xffff);
    body(x, 0);
    // creation continues with the active state (0x8d782)
    active(x);
}

// --- state 2: active -----------------------------------------------------------------------------

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    if var::PAUSE.get(x.b) != 0 {
        return;
    }
    if tst8(x, f::FREEZE).gt() {
        sub8(x, f::FREEZE, 1);
        return;
    }
    x.set8(0xb2, 0);
    match dispatch(x, 0x8d7aa, f::SUB) {
        0x8d7b8 => behave::intro(x),
        0x8d8bc => behave::lone(x),
        0x8dbd0 => attacks::run(x),
        _ => behave::chase(x),
    }
}

/// `moveq 0,D0 ; move.b (idx,A0),D0b ; move.w (table,PC,D0w),D1w ; jsr (table,PC,D1w)`: arrival address.
fn dispatch<R: Bench>(x: &mut Cx<'_, R>, table: u32, idx: u32) -> u32 {
    let i = x.u8(idx) as u32;
    let off = x.rom16(table + i);
    setw(x, 1, off);
    table.wrapping_add(off as i16 as i32 as u32)
}

/// Variant that stores the offset in D0 (`move.w (table,PC,D0w),D0w ; jmp (table,PC,D0w)`).
fn dispatch0<R: Bench>(x: &mut Cx<'_, R>, table: u32, idx: u32) -> u32 {
    let i = x.u8(idx) as u32;
    let off = x.rom16(table + i);
    table.wrapping_add(off as i16 as i32 as u32)
}

handlers! {
    0x8d59e "boss_11" run,
}
