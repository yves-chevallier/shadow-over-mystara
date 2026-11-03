//! Boss of types 14 and 19 (0x8fae4, a single routine for both), stages 32, 65 and 66. See
//! `docs/families/boss.md`.
//!
//! Dragon (types 14 "Green Dragon" and 19 "Black Dragon" in the game's debug names): type 19 of variant 0 is the
//! boss of stage 32, variant 1 is the dragon that Nagpa (`boss_10`) summons in stages 65 and 66 (`+0x46` =
//! Nagpa: the dragon disappears when Nagpa has no HP left). Type 14 was not seen in any stage of the scenarios. Enemy
//! skeleton (`+4` state, `+5` group, `+6` and `+7` steps), states set as longs.
//!
//! | `+5` | File | Role |
//! |---|---|---|
//! | 0 | `intro` | arrival: appearance cutscene, specific to each type and variant |
//! | 2 | `fight` | combat: walk toward a player (`+6` = 0, 8), about-face (2), stop (4), approach (6) |
//! | 4 | `attacks` | five attacks (`+6` = 0 to 10) |
//! | 6, 8 | here | standing |
//! | 10 | here | wait while no player is alive |
//! | state 4 | `hit` | reactions to hits (table 0x909a0), death |
//!
//! Animations: three sets (`+0xa0` = 0, 2 or 4) per type, to be chosen by `set_anim`; `+0xa1` is the current animation.
//! The dragon floats around altitude 0x20 (`hover`, 0x91424) when `+0xa4` is raised.

use crate::families::cx::Bench;
use crate::record::record;
use super::boss_common::*;
use super::cx::handlers;
use crate::objects::Obj;
use crate::player::Player;

mod attacks;
mod fight;
mod hit;
mod intro;

/// Fields of this boss beyond those of `Enemy` and `Boss` (`record!`). A meaning guessed from the code alone is
/// marked (h).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Boss19(pub Obj);

impl Boss19 {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Boss19 {
        /// Current animation set (0, 2 or 4) and current animation.
        anim_set, set_anim_set, ANIM_SET: u16 @ 0xa0;
        anim_n, set_anim_n, ANIM_N: u8 @ 0xa1;
        /// Hit boxes (value read by the collision engine) and mark set by the animation scripts.
        hurt_box, set_hurt_box, HURT_BOX: u8 @ 0xa2;
        mark, set_mark, MARK: u8 @ 0xa3;
        /// Requested floating (`+0xa4`) and its state (`+0xa5`).
        hover, set_hover, HOVER: u8 @ 0xa4;
        hover_state, set_hover_state, HOVER_STATE: u8 @ 0xa5;
        /// Countdown of a step (word; some steps use its high byte alone).
        timer, set_timer, TIMER: u16 @ 0xa6;
        /// State to resume after an about-face (long).
        saved_state, set_saved_state, SAVED_STATE: u32 @ 0xa8;
        /// Counterattack countdown (`+0xac`, 4 every 32 frames) and its rhythm (`+0xad`).
        retaliate, set_retaliate, RETALIATE: u8 @ 0xac;
        retaliate_tick, set_retaliate_tick, RETALIATE_TICK: u8 @ 0xad;
        /// Invulnerability frames remaining.
        shield, set_shield, SHIELD: u8 @ 0xae;
        /// Number of special counterattacks remaining.
        specials, set_specials, SPECIALS: u8 @ 0xaf;
        /// Bubbles of the breath left to blow, and the countdown between two.
        bubbles, set_bubbles, BUBBLES: u8 @ 0xb0;
        bubble_tick, set_bubble_tick, BUBBLE_TICK: u8 @ 0xb1;
        /// Countdown of the push of the players; its end clears the hit lock.
        pushed, set_pushed, PUSHED: u8 @ 0xb2;
    }
}
use crate::objects::pools;
use crate::objects::Current;

const TYPE_GREEN: u8 = 0xe;
const TYPE_BLACK: u8 = 0x13;

/// States set in one go (`+4..+7`).
const STAND: u32 = 0x0202_0000;
const CHASE: u32 = 0x0202_0400;
const TURN: u32 = 0x0202_0200;
const WAIT: u32 = 0x020a_0000;
const ATTACK_4: u32 = 0x0204_0400;
const ATTACK_10: u32 = 0x0204_0a00;
const DEAD_HOST: u32 = 0x0406_0000;
const FREE: u32 = 0x0600_0000;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    x.tick_counters();
    match x.u8(f::STATE) {
        st::INIT => init(x),
        st::ACTIVE => active(x),
        st::HIT => hit::run(x),
        _ => x.free(&pools::BOSS),
    }
}

// --- animations ----------------------------------------------------------------------------------

/// Starts animation `n` of set `set` (0, 2 or 4) of the type, unless it is already playing (0x914c6, 0x914e0,
/// 0x914fa).
fn set_anim<R: Bench>(x: &mut Cx<'_, R>, set: u8, n: u32) {
    let black = x.u8(Enemy::KIND) == TYPE_BLACK;
    let base = match (set, black) {
        (0, false) => 0x17dbb0,
        (0, true) => 0x1824d4,
        (2, false) => 0x17dbba,
        (2, true) => 0x1824de,
        (_, false) => 0x17dbd2,
        (_, true) => 0x1824f6,
    };
    if set == x.u8(Boss19::ANIM_SET) && n as u8 == x.u8(Boss19::ANIM_N) {
        return;
    }
    x.set8(Boss19::ANIM_SET, set);
    x.set8(Boss19::ANIM_N, n as u8);
    x.anim_start(base, n as u16);
}

/// Rest animation (set 0).
fn rest<R: Bench>(x: &mut Cx<'_, R>) {
    set_anim(x, 0, 0);
}

fn step<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    x.anim_step()
}

// --- state changes ---------------------------------------------------------------------------

/// 0x913da: standing, at rest.
fn stand<R: Bench>(x: &mut Cx<'_, R>) {
    x.set32(f::STATE, STAND);
    rest(x);
}

/// 0x913e8: walk toward a player.
fn chase<R: Bench>(x: &mut Cx<'_, R>) {
    x.set32(f::STATE, CHASE);
    rest(x);
}

/// 0x913f6: about-face.
fn turn<R: Bench>(x: &mut Cx<'_, R>) {
    x.set32(f::STATE, TURN);
    let side = x.u8(f::FLIP) ^ 0x20;
    x.set8(f::FLIP, side);
    set_anim(x, 0, 1);
}

/// 0x9140a: attack 4 (breath), invulnerable for a moment.
fn attack_4<R: Bench>(x: &mut Cx<'_, R>) {
    x.set32(f::STATE, ATTACK_4);
    x.set8(Boss19::SHIELD, 8);
}

/// 0x8fcf0: waits for a player.
fn wait<R: Bench>(x: &mut Cx<'_, R>) {
    x.set32(f::STATE, WAIT);
}

/// 0x9141a: no host left: the boss disappears.
fn host_gone<R: Bench>(x: &mut Cx<'_, R>) {
    x.set32(f::STATE, DEAD_HOST);
}

// --- state 0: creation --------------------------------------------------------------------------

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    if var::PAUSE.get(x.b) != 0 {
        return;
    }
    if !super::creators::enemy_7_pair_created(x.b, x.o) {
        return;
    }
    x.add8(f::STATE, 2);
    x.set8(f::SUB, 0);
    x.set16(f::SUB2, 0);
    x.set8(Boss::WEAK_ELEMENT, 5);
    x.set8(Boss::RESISTED_ELEMENT, 0);
    x.set8(Enemy::EVENT_KIND, 0x26);
    x.set8(Enemy::LOOT_ROW, 0x24);
    let kind = x.u8(Enemy::KIND);
    if kind == TYPE_GREEN {
        x.set8(Enemy::EVENT_KIND, 0x4b);
        x.set8(Enemy::LOOT_ROW, 0x20);
        x.set8(Enemy::OWN_PALETTE, 1);
        x.set8(Enemy::PALETTE, 0x1c);
    }
    if kind == TYPE_BLACK && x.u8(Obj::VARIANT) == 1 {
        lib::initial_hp_boss(x);
    } else {
        if var::GAME_ON.get(x.b) != 0 {
            lib::sound(x, crate::sound::CMD_20);
        }
        lib::empty_popups(x);
        lib::clear_popup(x);
        lib::clear_all_popups(x);
        var::STAGE_CLOCK.set(x.b, 0x0459_5900);
        var::BOSS_FIGHT.set(x.b, 1);
        x.set8(Boss::DEATH_CREDIT, 1);
        x.set8(Boss::DEATH_VALUE, 0xff);
        lib::initial_hp_boss(x);
        lib::create_set_14(x);
        lib::create_set_15(x);
    }
    x.set8(Enemy::AI_SHOT, 0xff);
    x.set8(f::BOX_SET, 0x2e);
    x.set8(Enemy::WEAK_TO_B, 1);
    x.set32(f::IMMUNITY, 0xffff_ffff);
    for (off, v) in [(Enemy::AI_BYTE_ROW, 5), (Enemy::AI_ROW_8E, 3), (Enemy::AI_SPARE_ROW, 2), (Enemy::AI_68, 5), (Enemy::AI_69, 5), (Enemy::AI_WAIT_MASKS, 4), (Enemy::AI_WAIT_ROW, 8), (Enemy::AI_WANDER_ROW, 3)] {
        x.set8(off, v);
    }
    var::SCENE_LEVEL.set(x.b, 0);
    lib::draw_6b(x);
    x.set8(Enemy::AI_STANCE_ROW, 0);
    lib::draw_stance(x);
    x.set8(Enemy::LOOT, 0);
    x.set8(Boss19::SHIELD, 0);
    x.set8(Boss19::SPECIALS, 3);
    x.set16(Boss19::ANIM_SET, 0xffff);
    rest(x);
}

// --- state 2: active -----------------------------------------------------------------------------

/// Active state loop (0x8fc42); the hit state calls it again to resume the combat.
fn active<R: Bench>(x: &mut Cx<'_, R>) {
    if var::PAUSE.get(x.b) == 0 {
        if tst8(x, f::FREEZE).gt() {
            sub8(x, f::FREEZE, 1);
        } else {
            // Nagpa's dragon disappears with its host
            if x.u8(Enemy::KIND) == TYPE_BLACK && x.u8(Obj::VARIANT) == 1 {
                if (x.u16(f::HOST) as i16) >= 0 {
                    return host_gone(x);
                }
                let host = x.linked(f::HOST);
                if host.u8(x.b, Obj::ACTIVE) == 0 || host.u16(x.b, f::HP) == 0 {
                    return host_gone(x);
                }
            }
            x.set8(Boss19::HURT_BOX, 0);
            if sub8(x, Boss19::RETALIATE_TICK, 1).le() {
                x.set8(Boss19::RETALIATE_TICK, 0x20);
                x.set8(Boss19::RETALIATE, 4);
            }
            match x.u8(f::SUB) {
                0 => intro::run(x),
                2 => fight::run(x),
                4 => attacks::run(x),
                6 | 8 => stand(x),
                10 => idle(x),
                _ => {}
            }
            let s = x.u32(f::STATE);
            x.set32(Boss19::SAVED_STATE, s);
            hover(x);
            if tst8(x, Boss19::SHIELD).gt() {
                sub8(x, Boss19::SHIELD, 1);
                x.set8(Enemy::FRAME_TAG, 0);
            }
            if tst8(x, Boss19::PUSHED).gt() && sub8(x, Boss19::PUSHED, 1).le() {
                // `clr.b`: the high byte of the hit lock
                x.set8(Enemy::HIT_LOCK, 0);
            }
        }
    }
    if x.u8(f::SUB) == 0 {
        return;
    }
    lib::keep_in_zone(x);
    lib::collide_terrain(x);
    x.reinsert();
}

/// `+5` = 10 (0x908bc): waits for a player to come back.
fn idle<R: Bench>(x: &mut Cx<'_, R>) {
    if var::PLAYERS_ALIVE.get(x.b) != 0 {
        x.set32(f::STATE, STAND);
        return;
    }
    x.set8(Boss19::HOVER, 1);
    if x.u8(Enemy::SUB_STEP) == 0 {
        x.add8(Enemy::SUB_STEP, 2);
        return rest(x);
    }
    step(x);
}

/// 0x91424: floating around altitude 0x20 when `+0xa4` is raised; the altitude does not go below 0.
fn hover<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Enemy::KIND) == TYPE_GREEN && x.u8(f::SUB) == 0 {
        return;
    }
    if x.u8(Boss19::HOVER) == 0 {
        x.set8(Boss19::HOVER_STATE, 0);
    } else {
        x.set8(Boss19::HOVER, 0);
        let mut physics = true;
        if x.u8(Boss19::HOVER_STATE) == 0 {
            // reaches altitude 0x20 at one pixel per frame
            let mut d: i16 = 1;
            if (x.u16(Obj::Z).wrapping_sub(0x20) as i16) >= 0 {
                d = -d;
            }
            x.add16(Obj::Z, d as u16);
            if x.u16(Obj::Z) != 0x20 {
                physics = false;
            } else {
                x.set8(Boss19::HOVER_STATE, 0xff);
                let (mut vz, mut az) = (0x20i16, -0x200i16);
                if d >= 0 {
                    vz = -vz;
                    az = -az;
                }
                x.set16(Enemy::VZ, vz as u16);
                x.set16(Enemy::GRAVITY, az as u16);
            }
        }
        if physics {
            // oscillation: the acceleration reverses on either side of 0x20
            x.move_z();
            let (mut a, mut b) = (0x20i16, -0x200i16);
            let accel = x.u16(Enemy::GRAVITY) as i16;
            if x.u16(Obj::Z) as i16 >= 0x20 {
                if accel >= 0 {
                    a = -a;
                    b = -b;
                    x.set16(Enemy::GRAVITY, a as u16);
                    x.set16(Enemy::VZ, b as u16);
                }
            } else if accel < 0 {
                x.set16(Enemy::GRAVITY, a as u16);
                x.set16(Enemy::VZ, b as u16);
            }
        }
    }
    if (x.u16(Obj::Z) as i16) < 0 {
        x.set16(Obj::Z, 0);
    }
}

handlers! {
    BOSS_19 0x8fae4 "boss_19" run,
}
