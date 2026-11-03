//! Boss of type 16 (0x9325e): boss of stage 14 (h). See `docs/families/boss.md`.
//!
//! Old template, like Ezerhorden (`boss_5`): `+5` (multiples of 2) and `+6`, `+7` are set field by field, the three
//! animation sets (body 0x17efe8, attacks 0x17f002, hits received 0x17f046) are chosen by `anim_a`, `anim_b` and
//! `anim_c`, which save the set and the animation (`+0xa0`, `+0xa1`) so as not to restart the same one. The boss
//! stays crouched then charges the players (`+3`: 0 at the bottom of the screen, 1 at the top, according to the
//! camera).
//!
//! | `+5` | File | Role |
//! |---|---|---|
//! | 0 | `moves` | wake-up: both variants wait for a player to come close |
//! | 2 | `moves` | combat: wait (`+6` = 0), about-face (2), walk (4 and 8), leap toward the target (6) |
//! | 4 | `attacks` | attacks: eight hits (`+6` = 0 to 14) |
//! | 6 | `moves` | run to a point on the screen (`+6` = 0 and 2), then jump (4) |
//! | 8 | `moves` | wait before an attack (`+6` = 0), run (2), jump (4) |
//! | 10 | `moves` | pause |
//! | 12 | `moves` | a grabbed player (`+0x50`): bites, then released |
//! | state 4 | `hit` | reactions to hits (specific routines), knockback, on the ground, death |

use crate::families::cx::Bench;
use super::boss_common::*;
use super::cx::handlers;

mod attacks;
mod hit;
mod moves;

/// Fields specific to this type.
mod g {
    /// Player proximity level (0 to 7) during the wake-up.
    pub const NEAR: u32 = 0x2f;
    pub const NEAR_MAX: u32 = 0x30;
    /// Non-zero byte: gravity is applied at the end of the frame (`gravity`).
    pub const FALLS: u32 = 0xa2;
    pub const VULNERABLE: u32 = 0x67;
    pub const WALK_STEPS: u32 = 0x6b;
    pub const IDLE_COUNT: u32 = 0x6e;
    pub const WALK_DELAY: u32 = 0x71;
    /// Memory of the current animation: set (0, 2 or 4) and number.
    pub const ANIM_SET: u32 = 0xa0;
    pub const ANIM_N: u32 = 0xa1;
    pub const HIT_BUDGET: u32 = 0xa5;
    /// Timer (byte or word depending on the step).
    pub const TIMER: u32 = 0xa6;
    /// State to resume after an about-face (long).
    pub const SAVED_STATE: u32 = 0xa8;
    /// Aimed point (words) and rotation direction of the trajectory.
    pub const SPOT_X: u32 = 0xae;
    pub const SPOT_Z: u32 = 0xb0;
    pub const SPIN: u32 = 0xb3;
    pub const DOWN_TIME: u32 = 0x84;
    pub const SHAKE: u32 = 0x90;
    /// Saved byte "the pause has been seen".
    pub const PAUSE_SEEN: u32 = 0x42;
}
use g::*;
use crate::objects::pools;
use crate::objects::Current;

const SET_A: u32 = 0x17efe8;
const SET_B: u32 = 0x17f002;
const SET_C: u32 = 0x17f046;

/// States set in one go (`+4..+7`).
mod state {
    pub const STAND: u32 = 0x0202_0000;
    pub const TURN: u32 = 0x0202_0200;
    pub const WALK: u32 = 0x0202_0400;
    pub const LEAP: u32 = 0x0202_0600;
    pub const WALK_B: u32 = 0x0202_0800;
    pub const IDLE: u32 = 0x020a_0000;
    pub const IDLE_PAUSED: u32 = 0x020a_0200;
}

// --- animations --------------------------------------------------------------------------------

/// 0x95256: starts animation `n` of set `set` (of kind `kind`) via the AI, unless it is already the current one.
fn play_set<R: Bench>(x: &mut Cx<'_, R>, set: u32, kind: u8, n: u32) {
    if x.u8(ANIM_SET) == kind && x.u8(ANIM_N) == n as u8 {
        return;
    }
    x.set8(ANIM_SET, kind);
    x.set8(ANIM_N, n as u8);
    lib::start(x, set, n);
}

/// 0x95234: animation `n` of the movement set.
fn anim_a<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    play_set(x, SET_A, 0, n);
}

/// 0x95240: animation `n` of the attack set.
fn anim_b<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    play_set(x, SET_B, 2, n);
}

/// 0x9524c: animation `n` of the hits-received set.
fn anim_c<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    play_set(x, SET_C, 4, n);
}

// --- aides --------------------------------------------------------------------------------------

/// 0x95210: cancels the speeds.
fn stop<R: Bench>(x: &mut Cx<'_, R>) {
    x.set32(0x14, 0);
    x.set32(0x18, 0);
    x.set32(0x1c, 0);
}

/// 0x93420: no target left, wait.
fn idle<R: Bench>(x: &mut Cx<'_, R>) {
    x.set32(f::STATE, state::IDLE);
}

fn next7<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x7, 2);
}
fn next6<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB2, 2);
}

/// 0x95220: the camera in use (its record; X at `+8`, Y at `+0xc`).
fn camera_record<R: Bench>(x: &mut Cx<'_, R>) -> crate::objects::Obj {
    crate::objects::Obj(if var::CAMERA_SELECT.get(x.b) != 0 { 0xff854e } else { 0xff84fe })
}

/// 0x950b8: is the target behind the boss? (with no target, the boss waits and the answer is yes.)
fn target_behind<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if x.u16(f::TARGET) as i16 >= 0 {
        idle(x);
        return true;
    }
    let t = x.linked(f::TARGET);
    let mut d = t.u16(x.b, 0x8).wrapping_sub(x.u16(0x8));
    if x.u8(f::FLIP) == 0 {
        d = d.wrapping_neg();
    }
    (d.wrapping_add(0x20) as i16) < 0
}

/// 0x9489a: is the target far (outside ±0x20 in depth or ±0xc0 in X)?
fn target_far<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if x.u16(f::TARGET) as i16 >= 0 {
        idle(x);
        return true;
    }
    let t = x.linked(f::TARGET);
    let dz = t.u16(x.b, 0x10).wrapping_sub(x.u16(0x10)).wrapping_add(0x20);
    if dz <= 0x40 {
        return false;
    }
    let dx = t.u16(x.b, 0x8).wrapping_sub(x.u16(0x8)).wrapping_add(0xc0);
    dx > 0x180
}

/// 0x94902, 0x9491e, 0x94968: a target in the range box (and, for the last two, possible hits).
fn close_target<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    lib::target_in_box(x, 0, 0x28, 0x20)
}
fn edge_target<R: Bench>(x: &mut Cx<'_, R>, d0: i16, d1: u16, d2: u16) -> bool {
    if !lib::target_in_box(x, d0, d1, d2) {
        return false;
    }
    if x.u16(f::TARGET) as i16 >= 0 {
        idle(x);
        return true;
    }
    let t = x.linked(f::TARGET);
    let same_side = t.u8(x.b, f::FLIP) == x.u8(f::FLIP);
    lib::decision(x, same_side)
}

/// 0x951dc: soft gravity when the step asked for it (`+0xa2`): the boss falls back to the ground.
fn gravity<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(FALLS) == 0 {
        return;
    }
    x.set8(FALLS, 0);
    x.move_z();
    if (x.u16(0x1a) as i16) >= 0 {
        x.set16(0x1a, (-0x18i16) as u16);
    }
    if (x.u16(0xc) as i16) < 0 {
        x.set16(0xc, 0);
        x.set16(0x18, 0x200);
        x.set16(0x1a, (-0x18i16) as u16);
    }
}

/// 0x95272: players in front of the boss in state 0x0200_0e00 (which leaves it defenseless): they receive `+0x189`.
fn count_victims<R: Bench>(x: &mut Cx<'_, R>) -> u32 {
    let mut n = 0;
    for k in 0..4u32 {
        let p = crate::objects::Obj(0xff85de + 0x200 * k);
        if p.u8(x.b, 0) == 0 {
            continue;
        }
        let mut d = p.u16(x.b, 0x8).wrapping_sub(x.u16(0x8));
        if x.u8(f::FLIP) == 0 {
            d = d.wrapping_neg();
        }
        if (d as i16) < 0 {
            continue;
        }
        if p.u32(x.b, f::STATE) & !0xff == 0x0200_0e00 {
            p.set_u8(x.b, 0x189, 1);
            n += 1;
        }
    }
    n
}

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    x.tick_counters();
    x.set8(0xa4, 0);
    match x.u8(f::STATE) {
        st::INIT => init(x),
        st::ACTIVE => active(x),
        st::HIT => hit::run(x),
        _ => x.free(&pools::BOSS),
    }
}

// --- state 0: creation --------------------------------------------------------------------------

/// 0x93286: waits for the end of the pause; variants 0 and 1 are chosen according to the camera height.
fn init<R: Bench>(x: &mut Cx<'_, R>) {
    if var::PAUSE.get(x.b) != 0 {
        return;
    }
    x.add8(f::STATE, 2);
    var::STAGE_CLOCK.set(x.b, 0x0459_5900);
    var::SCENE_LEVEL.set(x.b, 0);
    if (x.u8(3) as i8) < 2 {
        let (variant, z) = if (var::SCROLL_Y.get(x.b) as i16) >= 0x130 { (0, 0x1f8) } else { (1, 0x140) };
        x.set8(3, variant);
        x.set16(0x10, z);
    }
    x.set8(f::SUB, 0);
    x.set16(f::SUB2, 0);
    var::BOSS_FIGHT.set(x.b, 1);
    x.set8(0x93, 1);
    x.set8(0x9a, 0xff);
    pick_target(x);
    lib::draw_6b(x);
    x.set8(f::BOX_SET, 0x30);
    lib::initial_hp_boss(x);
    lib::create_set_14(x);
    lib::create_set_15(x);
    lib::clear_popup(x);
    lib::empty_popups(x);
    lib::clear_all_popups(x);
    for (off, v) in [(0x70, 0), (0x8e, 3), (0x78, 0xff), (0x68, 8), (0x69, 8)] {
        x.set8(off, v);
    }
    x.set32(f::IMMUNITY, 0xffff_ffff);
    x.set8(0, 2);
    for (off, v) in [(0x6a, 3), (0x6c, 4), (0x6d, 8), (TIMER, 0)] {
        x.set8(off, v);
    }
    let speed = if x.u16(f::TARGET) != 0 && var::PLAYERS_ALIVE.get(x.b) != 1 { 7 } else { 6 };
    x.set8(0x72, speed);
    x.set8(0x73, speed);
    for (off, v) in [(0x74, 1), (0x75, 0x44), (0x76, 6), (0x7e, 0x4a), (0x7f, 0), (0x81, 0), (0x80, 0x1a), (0xbf, 0)] {
        x.set8(off, v);
    }
    effct_trio_2e(x);
    x.set16(ANIM_SET, 0xffff);
    anim_a(x, 0);
    lib::sound(x, crate::sound::CMD_25);
}

// --- state 2: active -----------------------------------------------------------------------------

/// 0x933b8: one group per frame; during a stage pause, the boss stays waiting (0x020a0200) as long as players
/// are under its grip.
fn active<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(HIT_BUDGET, 5);
    if tst8(x, f::FREEZE).gt() {
        sub8(x, f::FREEZE, 1);
    } else if x.u8(f::SUB) == 0 || paused_dispatch(x) {
        group(x);
        gravity(x);
        if tst8(x, TIMER).gt() {
            sub8(x, TIMER, 1);
            x.set8(0x38, 0);
        }
    }
    lib::keep_on_screen(x, 0x40);
    lib::collide_terrain(x);
    x.reinsert();
}

/// 0x933d0..0x93452: true if the group must run this frame (outside pause, or waiting for a pause).
fn paused_dispatch<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if var::PAUSE.get(x.b) == 0 {
        x.set8(PAUSE_SEEN, 0);
        return true;
    }
    let held = x.u32(f::STATE) & !0xff;
    if held == state::IDLE_PAUSED {
        return true;
    }
    if x.u8(PAUSE_SEEN) != 0 {
        return false;
    }
    x.set8(PAUSE_SEEN, 0xff);
    if count_victims(x) == 0 {
        return false;
    }
    x.set32(f::STATE, state::IDLE_PAUSED);
    true
}

fn group<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB) {
        0 => moves::wake(x),
        2 => moves::fight(x),
        4 => attacks::run(x),
        6 => moves::jump_to_spot(x),
        8 => moves::before_attack(x),
        10 => moves::paused(x),
        12 => moves::holding(x),
        _ => {}
    }
}

/// 0x94e24: choice of what follows an attack: standing if the target is out of range, otherwise a new attack drawn
/// from the difficulty table (0x94e7c, one 32-byte row per pair of levels).
fn next_attack<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(f::TARGET) as i16 >= 0 {
        return idle(x);
    }
    if !lib::target_in_box(x, -0x40, 0x40, 0x20) {
        return x.set32(f::STATE, state::STAND);
    }
    let row = ((var::RANK.get(x.b) >> 1) as u32) << 5;
    let r = rand_and(x, 0x1f);
    let step = x.data8(0x94e7c + row + r);
    x.set8(f::SUB2, step);
    x.set16(f::STATE, 0x0208);
    x.set8(0x7, 0);
}

handlers! {
    0x9325e "boss_16" run,
}
