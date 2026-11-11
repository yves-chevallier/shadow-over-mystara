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

use crate::actors::families::cx::Bench;
use crate::world::objects::Obj;
use crate::world::record::record;
use super::boss_common::*;
use super::cx::handlers;
use crate::hw::mem::{rd16, rd8, wr8, A5};

mod attacks;
mod behave;
mod chain;
mod hit;

/// Fields of this boss beyond those of `Enemy` and `Boss` (`record!`). A meaning guessed from the code alone is
/// marked (h).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Boss11(pub Obj);

impl Boss11 {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Boss11 {
        /// Pose set by the animation scripts (`Enemy::STRIKE_KEY`), copied into `POSE` (`sync`).
        pose_key, set_pose_key, POSE_KEY: u8 @ 0x3e;
        /// Animation set (0, 2 or 4) and animation playing (`play`); 0xffff as a word: none.
        anim_kind, set_anim_kind, ANIM_KIND: u8 @ 0xa0;
        anim_n, set_anim_n, ANIM_N: u8 @ 0xa1;
        /// Position of the head at the end of the previous frame (words), restored at the start of the next.
        save_x, set_save_x, SAVE_X: u16 @ 0xa2;
        save_z, set_save_z, SAVE_Z: u16 @ 0xa4;
        save_y, set_save_y, SAVE_Y: u16 @ 0xa6;
        /// Rank of the ring that the head follows (0 to 11).
        seg, set_seg, SEG: u8 @ 0xa8;
        /// Pose: row of `boss_11/rows`.
        pose, set_pose, POSE: u8 @ 0xaa;
        /// Cleared at the creation, never read (h).
        pose_last, set_pose_last, POSE_LAST: u8 @ 0xab;
        /// Direction of the undulation of pose 1 (1 or -1).
        wave, set_wave, WAVE: u8 @ 0xac;
        /// Bit 0: a ring went below the ground during the placement.
        ring_flags, set_ring_flags, RING_FLAGS: u8 @ 0xad;
        /// Countdown of a step (word); its high byte is also `CHARGE_LEFT` and its low byte `AIM_PAUSE`.
        timer, set_timer, TIMER: u16 @ 0xb0;
        /// Charge: aimings left, and frames before the next aiming.
        charge_left, set_charge_left, CHARGE_LEFT: u8 @ 0xb0;
        aim_pause, set_aim_pause, AIM_PAUSE: u8 @ 0xb1;
        /// Non-zero during an attack.
        attacking, set_attacking, ATTACKING: u8 @ 0xb2;
        /// Charge: heading aimed at the target.
        aim, set_aim, AIM: u8 @ 0xb3;
        /// Approach: path (byte offset in `boss_11/routes`) and next point.
        route, set_route, ROUTE: u8 @ 0xb4;
        route_point, set_route_point, ROUTE_POINT: u8 @ 0xb5;
        /// Rest drawn when an attack is given up (frames waiting).
        rest, set_rest, REST: u8 @ 0xb6;
        /// Non-zero: the screen bounds are not applied this frame.
        no_zone, set_no_zone, NO_ZONE: u8 @ 0xb7;
        /// Countdown of a stun on the ground.
        stun_time, set_stun_time, STUN_TIME: u8 @ 0xb8;
        /// Slow-down frames after a stun (`+0x38` cleared while positive).
        slow, set_slow, SLOW: u8 @ 0xb9;
    }
}
/// A ring of the snake (ESHL object of type 5).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Ring(pub Obj);

impl Ring {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Ring {
        /// Pose word (variant + 1), read by the ring's handler (h).
        pose, set_pose, POSE: u16 @ 0x12;
        /// Rank behind the head (0 to 11).
        rank, set_rank, RANK: u8 @ 0x2f;
        /// Orientation and tilt (0 to 31), also copied together as a word.
        angles, set_angles, ANGLES: u16 @ 0x42;
        orientation, set_orientation, ORIENTATION: u8 @ 0x42;
        tilt, set_tilt, TILT: u8 @ 0x43;
    }
}

use crate::world::objects::pools;
use crate::world::objects::Current;

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

/// Abscissa and ordinate of the camera in use.
fn cam_x<R: Bench>(x: &mut Cx<'_, R>) -> u16 {
    if var::CAMERA_SELECT.get(x.b) != 0 { var::CAMERA_ALT_X.get(x.b) } else { var::SCROLL_X.get(x.b) }
}

fn cam_y<R: Bench>(x: &mut Cx<'_, R>) -> u16 {
    if var::CAMERA_SELECT.get(x.b) != 0 { var::CAMERA_ALT_Y.get(x.b) } else { var::SCROLL_Y.get(x.b) }
}

/// Types 12 and 11 are distinguished by byte `+2`.
fn is_12<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    x.u8(Enemy::KIND) == 0xc
}

/// `move.b (0x3e,A0),(0xaa,A0)`.
fn sync<R: Bench>(x: &mut Cx<'_, R>) {
    let v = x.u8(Boss11::POSE_KEY);
    x.set8(Boss11::POSE, v);
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
    if x.u8(Boss11::ANIM_KIND) == kind && x.u8(Boss11::ANIM_N) == n as u8 {
        return;
    }
    x.set8(Boss11::ANIM_KIND, kind);
    x.set8(Boss11::ANIM_N, n as u8);
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
    let second = is_12(x) as u32;
    let v = byte(x, "boss_11/strike_kinds", d as u32 + second);
    x.set8(Enemy::STRIKE_KIND, v);
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
    x.set8(Boss::SHOT, 0x3f);
    body(x, 0)
}

/// 0x8eaea: about-face.
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

/// 0x8ebd0: speeds cleared.
fn stop<R: Bench>(x: &mut Cx<'_, R>) {
    x.set32(Enemy::VX_LONG, 0);
    x.set32(Enemy::VZ_LONG, 0);
    x.set32(Enemy::VY_LONG, 0);
}

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    x.tick_counters();
    restore(x);
    set44(x, 0);
    x.set8(Boss11::NO_ZONE, 0);
    let state = x.u8(f::STATE) as u32;
    match state {
        0 => init(x),
        2 => active(x),
        4 => hit::run(x),
        _ => x.free(&pools::BOSS),
    }
    if (x.u8(Boss11::SLOW) as i8) > 0 {
        sub8(x, Boss11::SLOW, 1);
        x.set8(Enemy::FRAME_TAG, 0);
    }
    if x.u8(f::STATE) == 0 || x.u8(Enemy::ACTIVE) == 0 {
        return;
    }
    if x.u8(f::STATE) != 4 && var::PAUSE.get(x.b) != 0 {
        return;
    }
    if x.u8(Boss11::NO_ZONE) == 0 {
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
    let mut d0 = cam_x(x);
    if !Fl::sub16(d0, x.u16(Obj::X)).le() {
        x.set16(Obj::X, d0);
    }
    d0 = d0.wrapping_add(0x180);
    if !Fl::sub16(d0, cam_x(x)).ge() {
        x.set16(Obj::X, d0);
    }
    d0 = cam_y(x);
    if !Fl::sub16(d0, x.u16(Obj::Y)).le() {
        x.set16(Obj::Y, d0);
    }
}

/// 0x8d65a: the position is that of the previous frame (copied by `chain::update`).
fn restore<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::STATE) == 0 || x.u8(Enemy::ACTIVE) == 0 {
        return;
    }
    if x.u8(f::STATE) != 4 && var::PAUSE.get(x.b) != 0 {
        return;
    }
    let (a, b, c) = (x.u16(Boss11::SAVE_X), x.u16(Boss11::SAVE_Z), x.u16(Boss11::SAVE_Y));
    x.set16(Obj::X, a);
    x.set16(Obj::Z, b);
    x.set16(Obj::Y, c);
}

// --- state 0: creation --------------------------------------------------------------------------

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    if var::PAUSE.get(x.b) != 0 {
        return;
    }
    // D0.w = the state (0, set by `run`): compared with the maximum number of shots of type 5
    if !crate::actors::families::eshl_creators::twelve_5(&mut crate::actors::families::effects_creators::leftovers::Pure, x.b, x.o, 0) {
        return;
    }
    x.add8(f::STATE, 2);
    x.set8(f::SUB, 0);
    x.set16(f::SUB2, 0);
    x.set32(f::IMMUNITY, 0xefff_ffff);
    x.set8(Enemy::BOX_SET, 0x2c);
    x.set8(Boss::SHOT, 0x3f);
    x.set8(Enemy::AI_ROW_8E, 3);
    lib::sound(x, crate::services::sound::CMD_20);
    lib::empty_popups(x);
    lib::clear_popup(x);
    var::BOSS_FIGHT.set(x.b, 1);
    x.set8(Boss::DEATH_CREDIT, 1);
    x.set8(Boss::DEATH_VALUE, 0xff);
    x.set8(Enemy::EVENT_KIND, 0x4e);
    x.set8(Enemy::LOOT, 0);
    x.set8(Enemy::LOOT_ROW, 0x26);
    x.set8(Boss::WEAK_ELEMENT, 2);
    x.set8(Boss::RESISTED_ELEMENT, 1);
    // Original glitch: `cmpi.b #0xc,D0b` was probably meant to distinguish type 12, but D0 has just been zeroed by
    // 0x19e10 (`clear_popup`): both types take these settings.
    {
        x.set8(Enemy::LOOT, 0);
        x.set8(Enemy::LOOT_ROW, 0x27);
        x.set8(Boss::WEAK_ELEMENT, 1);
        x.set8(Boss::RESISTED_ELEMENT, 2);
    }
    x.set8(Boss11::SLOW, 0);
    lib::initial_hp_boss(x);
    lib::create_set_14(x);
    var::STAGE_CLOCK.set(x.b, 0x0459_5900);
    var::SCENE_LEVEL.set(x.b, 0);
    lib::create_set_15(x);
    lib::clear_popup(x);
    lib::empty_popups(x);
    lib::clear_all_popups(x);
    let (a, b, c) = (x.u16(Obj::X), x.u16(Obj::Z), x.u16(Obj::Y));
    x.set16(Boss11::SAVE_X, a);
    x.set16(Boss11::SAVE_Z, b);
    x.set16(Boss11::SAVE_Y, c);
    x.set8(Boss11::POSE, 0);
    x.set8(Boss11::POSE_LAST, 0);
    x.set8(Boss11::WAVE, 0);
    x.set16(Boss11::ANIM_KIND, 0xffff);
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
    x.set8(Boss11::ATTACKING, 0);
    match x.u8(f::SUB) {
        0 => behave::intro(x),
        6 => behave::lone(x),
        4 => attacks::run(x),
        _ => behave::chase(x),
    }
}

handlers! {
    BOSS_11 0x8d59e "boss_11" run,
}
