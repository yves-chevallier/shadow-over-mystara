//! Boss of type 3 (0x7f41a): the flying creature of stage 11 (debug name "Harpy", h). See
//! `docs/families/boss.md`.
//!
//! Skeleton of the AI bosses (`+4` state, `+5` group, `+6` and `+7` steps, states set as longs). The boss flies:
//! the height `+0xc` is its altitude, it hovers, dives on the players, grabs them and carries them off. Its body
//! pushes back the players (`+0xa4`: body shape, `body` routines) and the other objects (`boss_common`, 0x81aa4).
//!
//! | `+5` | File | Role |
//! |---|---|---|
//! | 0 | `fight` | arrival: descent from the top of the screen (variant 0 only) |
//! | 2 | `fight` | combat: hovering, about-face, approach (`+6` = 4, 8), rise, descent, patrol (16) |
//! | 4 | `attacks` | sixteen attacks (`+6` = 0 to 30): claw swipes, dives, rushes, cries, abduction |
//! | 6, 8 | `fight` | tour of the screen, takeoff |
//! | 10 | `fight` | wait while no player is alive |
//! | 12 | `fight` | a grabbed player is carried off then dropped |
//! | 14 | `fight` | randomly drawn counterattack |
//! | state 4 | `hit` | reactions to hits (ROM table), knockback, fall, death |
//!
//! Animations: 0x174776 (body), 0x1747c4 (attacks), 0x1747f0 (death).

use crate::families::cx::Bench;
use crate::record::record;
use crate::data::RomAddr;
use super::boss_common::*;
use crate::objects::Obj;

mod attacks;
pub mod body;
mod fight;
mod hit;

/// Fields of this boss beyond those of `Enemy` and `Boss` (`record!`). A meaning guessed from the code alone is
/// marked (h).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Boss3(pub Obj);

impl Boss3 {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Boss3 {
        /// Kind of aiming requested from 0xe76c.
        aim, set_aim, AIM: u8 @ 0x79;
        /// Byte reset to 0xff every frame outside pause, to 0 when the boss hovers (h).
        hover, set_hover, HOVER: u8 @ 0xa0;
        /// Flight (1) or ground (0) mode: set by the steps, and by the HP thresholds of table 0x81712 (`wound_changed`);
        /// in flight, the attacks are drawn from another table.
        flying, set_flying, FLYING: u8 @ 0xa2;
        /// Step of the dive (attack 20).
        dive, set_dive, DIVE: u8 @ 0xa3;
        /// Body shape that pushes back (0 none, 1 front, 2 sideways), reset to zero every frame.
        body_shape, set_body_shape, BODY_SHAPE: u8 @ 0xa4;
        /// Invulnerability frames remaining (`+0x38` reset to zero).
        invulnerable, set_invulnerable, INVULNERABLE: u8 @ 0xa5;
        /// Step timer (byte or word depending on the step).
        timer, set_timer, TIMER: u8 @ 0xa6;
        /// State to resume after an about-face (long).
        saved_state, set_saved_state, SAVED_STATE: u32 @ 0xaa;
        /// Patrol waypoint (words) and direction in which the angle turns.
        goal_x, set_goal_x, GOAL_X: u16 @ 0xb0;
        goal_z, set_goal_z, GOAL_Z: u16 @ 0xb2;
        goal_turn, set_goal_turn, GOAL_TURN: u8 @ 0xb5;
        /// Set to 2 when the boss reaches its target in the approach; read by no code of the port (h).
        approached, set_approached, APPROACHED: u8 @ 0xaf;
    }
}
use crate::objects::pools;
use crate::objects::Current;

/// Jeux d'animations : corps, attaques, mort.
const SET_BODY: RomAddr = RomAddr(0x174776);
const SET_ATTACK: RomAddr = RomAddr(0x1747c4);
const SET_DEATH: RomAddr = RomAddr(0x1747f0);


/// Starts animation `n` of a set via the AI (0x103d6).
fn body_anim<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    lib::start(x, SET_BODY.0, n);
}

fn attack_anim<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    lib::start(x, SET_ATTACK.0, n);
}

/// Next step (`addq.b #2,(+6)` or `(+7)`).
fn next6<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB2, 2);
}
fn next7<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Enemy::SUB_STEP, 2);
}

/// `move.w #v,(6,A0)`: steps `+6` and `+7` in one go.
fn set_steps<R: Bench>(x: &mut Cx<'_, R>, v: u16) {
    x.set16(f::SUB2, v);
}

/// 0x8188e: speeds and accelerations (`+0x14..+0x1f`) to zero.
fn stop<R: Bench>(x: &mut Cx<'_, R>) {
    x.set32(Enemy::VX_LONG, 0);
    x.set32(Enemy::VZ_LONG, 0);
    x.set32(Enemy::VY_LONG, 0);
}

/// 0x8189e: record of the camera in use (A2: 0xff84fe, or 0xff854e if `A5+0x36`).
fn camera<R: Bench>(x: &mut Cx<'_, R>) -> Obj {
    let a = if var::CAMERA_SELECT.get(x.b) != 0 { 0xff854e } else { 0xff84fe };
    Obj(a)
}

/// 0x7f5ae: no target left, wait (`+4` = 0x020a0000).
fn idle_alone<R: Bench>(x: &mut Cx<'_, R>) {
    x.set32(f::STATE, 0x020a_0000);
}

/// 0x8173c: is the target behind the boss? With no target: wait, and true.
fn target_behind<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if (x.u16(f::TARGET) as i16) >= 0 {
        idle_alone(x);
        return true;
    }
    let t = load_target(x);
    let mut d = t.u16(x.b, Obj::X).wrapping_sub(x.u16(Obj::X));
    if x.u8(f::FLIP) == 0 {
        d = d.wrapping_neg();
    }
    (d as i16) < 0
}

/// 0x81764: is the target far (more than 0x20 in depth or 0xc0 in width)? With no target: wait, and true.
fn target_far<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if x.u16(f::TARGET) == 0 {
        idle_alone(x);
        return true;
    }
    let t = load_target(x);
    let dz = t.u16(x.b, Obj::Y).wrapping_sub(x.u16(Obj::Y)).wrapping_add(0x20);
    if dz <= 0x40 {
        false
    } else {
        let dx = t.u16(x.b, Obj::X).wrapping_sub(x.u16(Obj::X)).wrapping_add(0xc0);
        dx > 0x180
    }
}

/// 0x81722: sixteenths of HP remaining (0 to 15).
fn hp_sixteenths<R: Bench>(x: &mut Cx<'_, R>) -> u32 {
    let mut d1 = x.u16(f::HP) << 4;
    let max = x.u16(Enemy::HP_MAX);
    *x.c.d2() = *x.c.d2() & 0xffff_0000 | max as u32;
    let mut d0 = 0xfu32;
    loop {
        d1 = d1.wrapping_sub(max);
        if d1 & 0x8000 != 0 {
            break;
        }
        d0 -= 1;
        if d0 == 0 {
            break;
        }
    }
    d0
}

/// 0x816f8: has the boss just changed HP threshold (`+0xa2`, table 0x81712)?
fn wound_changed<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let n = hp_sixteenths(x);
    let w = byte(x, "boss_3/wounds", n);
    let changed = w != x.u8(Boss3::FLYING);
    if changed {
        x.set8(Boss3::FLYING, w);
    }
    changed
}

/// Draw of an attack from a table of 32 bytes per difficulty level: `+6` = the attack, `+4..+5` = active / attacks,
/// `+7` = 0.
fn draw_attack<R: Bench>(x: &mut Cx<'_, R>, table: &str) {
    let k = rank_byte(x, table);
    x.set8(f::SUB2, k);
    x.set16(f::STATE, 0x0204);
    x.set8(Enemy::SUB_STEP, 0);
}

/// 0x80e1c / 0x80e22: attack drawn for a nearby target (`far`: at a distance); when wounded, the wounded table.
fn choose_attack<R: Bench>(x: &mut Cx<'_, R>, far: bool) {
    let table = if x.u8(Boss3::FLYING) != 0 {
        "boss_3/wounded_attacks"
    } else if far {
        "boss_3/far_attacks"
    } else {
        "boss_3/near_attacks"
    };
    draw_attack(x, table)
}

/// 0x81462: end of an attack: new attack if the target is very close, otherwise resumption of the flight (or wait).
fn after_attack<R: Bench>(x: &mut Cx<'_, R>) {
    if !pick_target(x) {
        let s = if x.u8(Boss3::FLYING) != 0 { 0x0202_0a00 } else { 0x0202_0400 };
        return x.set32(f::STATE, s);
    }
    load_target(x);
    let close = lib::target_in_box(x, -0x40, 0x40, 0x20);
    if x.u8(Boss3::FLYING) != 0 {
        return x.set32(f::STATE, 0x0202_0a00);
    }
    if close {
        return draw_attack(x, "boss_3/follow_ups");
    }
    x.set32(f::STATE, 0x0202_0400);
}

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    x.tick_counters();
    if var::PAUSE.get(x.b) == 0 {
        x.set8(Boss3::HOVER, 0xff);
    }
    x.set8(Boss3::BODY_SHAPE, 0);
    match x.u8(f::STATE) {
        st::INIT => init(x),
        st::ACTIVE => active(x),
        st::HIT => hit::run(x),
        _ => {
            var::BODY_OWNER.set(x.b, 0);
            x.free(&pools::BOSS);
        }
    }
}

// --- state 0: creation --------------------------------------------------------------------------

/// 0x7f44e: boss setup (outside cutscene); it registers itself as an obstacle.
fn init<R: Bench>(x: &mut Cx<'_, R>) {
    if var::PAUSE.get(x.b) != 0 {
        return;
    }
    x.add8(f::STATE, 2);
    var::BOSS_FIGHT.set(x.b, 1);
    var::STAGE_CLOCK.set(x.b, 0x0459_5900);
    var::SCENE_LEVEL.set(x.b, 0);
    x.set8(Boss::DEATH_CREDIT, 1);
    x.set8(Boss::DEATH_VALUE, 0xff);
    pick_target(x);
    lib::draw_6b(x);
    x.set8(f::BOX_SET, 0x23);
    lib::initial_hp_boss(x);
    lib::create_set_14(x);
    lib::create_set_15(x);
    for (off, v) in [(Enemy::AI_STANCE_ROW, 0), (Enemy::AI_ROW_8E, 3), (Enemy::AI_SHOT, 0xff), (Enemy::AI_68, 1), (Enemy::AI_69, 1)] {
        x.set8(off, v);
    }
    x.set32(f::IMMUNITY, 0xffff_ffff);
    for (off, v) in [(Enemy::AI_WANDER_ROW, 3), (Enemy::AI_WAIT_MASKS, 4), (Enemy::AI_WAIT_ROW, 8)] {
        x.set8(off, v);
    }
    let mut n = 6;
    if x.u16(f::TARGET) != 0 && var::PLAYERS_ALIVE.get(x.b) != 1 {
        n = 7;
    }
    x.set8(Enemy::AI_DECISION_A, n);
    x.set8(Enemy::AI_DECISION_B, n);
    for (off, v) in [(Enemy::AI_BOX_CHANCE, 1), (Enemy::AI_BOX_ANIM, 0x44), (Enemy::AI_BYTE_ROW, 6), (Enemy::EVENT_KIND, 0x21), (Enemy::EVENT_LOW, 0), (Enemy::LOOT, 0), (Enemy::LOOT_ROW, 0x19), (Enemy::ANIM_VARIANT, 0)] {
        x.set8(off, v);
    }
    x.set8(Boss3::INVULNERABLE, 0);
    var::BODY_OWNER.set(x.b, x.o.0 as u16);
    crate::families::effects_creators::mirrored_pair(x.b, x.o);
    body_anim(x, 0);
    lib::empty_popups(x);
    lib::clear_popup(x);
    lib::clear_all_popups(x);
}

// --- state 2: active -----------------------------------------------------------------------------

/// 0x7f550: one group per frame (outside freeze and pause), invulnerability counted down, then zone, shadow and list.
fn active<R: Bench>(x: &mut Cx<'_, R>) {
    if tst8(x, f::FREEZE).gt() {
        sub8(x, f::FREEZE, 1);
    } else if var::PAUSE.get(x.b) == 0 {
        match x.u8(f::SUB) {
            0 => fight::intro(x),
            2 => fight::run(x),
            4 => attacks::run(x),
            6 => fight::round(x),
            8 => fight::rise(x),
            10 => fight::wait_for_players(x),
            12 => fight::carrying(x),
            _ => fight::counter(x),
        }
        if tst8(x, Boss3::INVULNERABLE).gt() {
            sub8(x, Boss3::INVULNERABLE, 1);
            x.set8(Enemy::FRAME_TAG, 0);
        }
    }
    if x.u8(f::SUB) == 0 {
        return;
    }
    end_frame(x, 0x40);
}

/// Ordinary end of frame: held within the screen (margin `margin`), shadow, put back in the list.
fn end_frame<R: Bench>(x: &mut Cx<'_, R>, margin: u16) {
    lib::keep_on_screen(x, margin);
    lib::collide_terrain(x);
    x.reinsert();
}

/// 0x81ae0: counterattack of a common hit (called by the reactions, 0x3e22e): draw according to difficulty, then
/// attack 24 (rush along the ground).
/// Returns the draw (D0 of the original, `andi.l #0xf`) and the mask (D1.w) for the native's contract.
pub(crate) fn counter_reaction<R: Bench>(x: &mut Cx<'_, R>) -> (u32, u16) {
    let r = (x.random() as u32) & 0xf;
    let level = (var::RANK.get(x.b) >> 1).wrapping_mul(4);
    // index `(d8,PC,D1.w)`: sign-extended word
    let mask = word(x, "boss_3/counter_masks", level as i16 as i32 as u32);
    if mask >> r & 1 != 0 {
        x.set8(Boss3::INVULNERABLE, 8);
        x.set32(f::STATE, 0x0204_1800);
        x.set8(Enemy::VULNERABLE, 1);
        x.set8(Enemy::ACTIVE, 1);
    }
    (r, mask)
}

/// Native of the handler.
pub const NATIVES: &[crate::Native] =
    &[crate::families::cx::handler_without(0x7f41a, "boss_3", |b, a| run(&mut crate::families::cx::Gx::dispatch(b, a)))];
