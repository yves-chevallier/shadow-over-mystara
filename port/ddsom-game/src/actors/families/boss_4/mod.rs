//! Boss of type 4 (0x81b3a): the Chimera (stages 104 and 105, and in `cov_2p_c`, `cov_tour4_c3`). See
//! `docs/families/boss.md`.
//!
//! Skeleton of the AI bosses (`+4` state, `+5` group, `+6` and `+7` steps, states set in one go as longs,
//! AI library 0x103d6..0x105b6). The body is accompanied by three graphic doubles (ENEMY 22, 23, 24 of
//! `enemy_double`), created with it and linked by `+0xb4`, `+0xb6`, `+0xb8`; the first (`+0xb4`) replays on request
//! (`+0xb6` of the double) the breath gestures and says through its `+0x33` when to breathe.
//!
//! | `+5` | File | Role |
//! |---|---|---|
//! | 0 | `fight` | arrival, descending from the top of the screen |
//! | 2 | `fight` | combat: wait, about-face, walk (`+6` = 4, 6), charge (8), leap (10) |
//! | 4 | `attacks` | seven attacks (`+6` = 0 to 12): cry, takeoff and dive, crossing, leap, breaths |
//! | 6 to 12 | `fight` | wait while no player is alive |
//! | 14 | `hit` | return to combat |
//! | state 4 | `hit` | reactions to hits (ROM table), knockback, fall, death |
//!
//! The attack choices (draws in the ROM tables according to phase `+0xaa`) and the states they set are
//! in `decide`. Animations: 0x17557e (body), 0x175596 (flight and attacks), 0x1755ae (hits received).

use crate::actors::families::cx::Bench;
use crate::world::data::RomAddr;
use crate::world::record::record;
use super::boss_common::*;
use super::enemy_double::Double;
use super::cx::handlers;
use crate::world::objects::Obj;

mod attacks;
mod decide;
mod fight;
pub(crate) mod hit;

/// Fields of this boss beyond those of `Enemy` and `Boss` (`record!`). A meaning guessed from the code alone is
/// marked (h).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Boss4(pub Obj);

impl Boss4 {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Boss4 {
        breath_step, set_breath_step, BREATH_STEP: u8 @ 0x42;
        /// Hits taken on the ground (word).
        hits, set_hits, HITS: u16 @ 0x84;
        dust, set_dust, DUST: u16 @ 0xa8;
        /// Countdown between two jets of the breath, and rank of the jet in the angle table (`+0x42`).
        breath, set_breath, BREATH: u8 @ 0xaf;
        /// Charge: 0 toward the middle of the screen, 1 toward the target's retained X position (`+0xbc`).
        charge_at_target, set_charge_at_target, CHARGE_AT_TARGET: u8 @ 0xb2;
        /// Wing beat: alternated at each mark (0x11a36).
        wing, set_wing, WING: u8 @ 0xb3;
        /// The three doubles (words).
        double_a, set_double_a, DOUBLE_A: u8 @ 0xb4;
        double_b, set_double_b, DOUBLE_B: u8 @ 0xb6;
        double_c, set_double_c, DOUBLE_C: u8 @ 0xb8;
    }
}
use crate::world::objects::pools;
use crate::world::objects::Current;

/// Animation sets: body, flight and attacks, hits received.
const SET_BODY: RomAddr = RomAddr(0x17557e);
const SET_AIR: RomAddr = RomAddr(0x175596);
const SET_HURT: RomAddr = RomAddr(0x1755ae);

/// Starts animation `n` of a set via the AI (0x103d6).
fn body<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    lib::start(x, SET_BODY.0, n);
}

fn air<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    lib::start(x, SET_AIR.0, n);
}

/// `moveq #k,D0 ; move.b D0,(0xa0) ; move.b D0,(0xa1)` : animation kind `k`.
fn kind<R: Bench>(x: &mut Cx<'_, R>, k: u8) {
    x.set8(Boss::ANIM_KIND, k);
    x.set8(Boss::ANIM_LAST, k);
}

/// Low-order word of a data register: only D2 to D5 are kept (leftovers re-read by the following handlers);
/// D0, D1 and D6 are re-read by no one.
fn setw<R: Bench>(x: &mut Cx<'_, R>, n: usize, v: u16) {
    let r = match n {
        2 => x.c.d2(),
        3 => x.c.d3(),
        4 => x.c.d4(),
        5 => x.c.d5(),
        _ => return,
    };
    *r = *r & 0xffff_0000 | v as u32;
}

/// `subq.w #1,(off,A0)`: true when the countdown has elapsed (`ble`).
fn expired<R: Bench>(x: &mut Cx<'_, R>, off: u32) -> bool {
    sub16(x, off, 1).le()
}

/// At least one player alive (0x10474).
fn alive<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    lib::players_alive(x)
}

/// The double `off` (`movea.w (off,A0),A1`).
fn double<R: Bench>(x: &mut Cx<'_, R>, off: u32) -> Obj {
    x.linked(off)
}

/// Asks the first double to replay its gesture (`+0xb6` of the double).
fn cue_double<R: Bench>(x: &mut Cx<'_, R>) {
    let d = double(x, Boss4::DOUBLE_A);
    Double(d).set_restart(x.b, 1);
}

/// Every 4 frames, a puff of dust (0x1172e, kind 3).
fn dust_tick<R: Bench>(x: &mut Cx<'_, R>) {
    if !sub16(x, Boss4::DUST, 1).gt() {
        x.set16(Boss4::DUST, 4);
        lib::debris(x, 3);
    }
}

/// Marked key frame (bit 7 of `+0x33`): a wing beat, on alternating sides.
fn flap<R: Bench>(x: &mut Cx<'_, R>) {
    if (x.u8(f::KEY_FRAME) as i8) < 0 {
        let k = x.u8(f::KEY_FRAME) & 0x7f;
        x.set8(f::KEY_FRAME, k);
        let w = x.u8(Boss4::WING) ^ 1;
        x.set8(Boss4::WING, w);
        lib::ground_debris(x, w);
    }
}

/// `moveq #0,D0 ; move.w (off,A0),D0 ; swap D0 ; asr.l #8,D0`: speed (word) as a long with 8 fractional bits.
fn speed_long<R: Bench>(x: &mut Cx<'_, R>, off: u32) -> u32 {
    ((x.u16(off) as i16 as i32) << 8) as u32
}

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
    if x.u8(Boss::INVULNERABLE) != 0 {
        x.set8(Enemy::FRAME_TAG, 0);
    }
    if x.u8(Enemy::ACTIVE) != 0 {
        x.reinsert();
    }
}

// --- state 0: creation --------------------------------------------------------------------------

/// 0x81b74: boss setup, creation of the three doubles; in game, it enters from the top of the screen.
fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    var::BOSS_FIGHT.set(x.b, 1);
    var::STAGE_CLOCK.set(x.b, 0x0459_5900);
    pick_target(x);
    x.set8(Boss::PHASES, 2);
    x.set8(f::BOX_SET, 0x24);
    lib::initial_hp_boss(x);
    var::SCENE_LEVEL.set(x.b, 0);
    lib::spread_index(x);
    lib::clear_popup(x);
    lib::empty_popups(x);
    lib::clear_all_popups(x);
    for (off, v) in [(Enemy::AI_BYTE_ROW, 0), (Enemy::AI_STANCE_ROW, 0), (Enemy::AI_WANDER_ROW, 2), (Enemy::AI_BOX_ANIM, 0)] {
        x.set8(off, v);
    }
    x.set32(f::IMMUNITY, 0xffff_ffff);
    x.set8(Enemy::ACTIVE, 2);
    x.set_bit(Enemy::DISPLAY, 1, true);
    for (off, v) in [
        (Boss::DEATH_CREDIT, 1),
        (Boss::DEATH_VALUE, 0xff),
        (Enemy::AI_DECISION_A, 1),
        (Enemy::AI_DECISION_B, 1),
        (Enemy::AI_WAIT_MASKS, 6),
        (Enemy::AI_WAIT_ROW, 1),
        (Enemy::AI_BOX_CHANCE, 7),
        (Enemy::AI_SHOT, 0xff),
        (Enemy::AI_ROW_8E, 3),
        (Enemy::AI_68, 8),
        (Enemy::AI_69, 1),
        (Enemy::VULNERABLE, 0),
        (Boss::ANIM_KIND, 0),
        (Boss::ANIM_LAST, 0xff),
        (Enemy::EVENT_KIND, 0x4d),
        (Enemy::EVENT_LOW, 0),
        (Enemy::LOOT_ROW, 0x25),
        (Enemy::LOOT, 0x1e),
    ] {
        x.set8(off, v);
    }
    // two doubles with their own HP (the boss's minus 0xfa or 0x12c, at least 0x96 or 0x64), a third without
    spawn_double(x, 0x16, Some((0xfa, 0x96, 0x43)), Boss4::DOUBLE_A);
    spawn_double(x, 0x17, Some((0x12c, 0x64, 0x44)), Boss4::DOUBLE_B);
    spawn_double(x, 0x18, None, Boss4::DOUBLE_C);
    if var::GAME_ON.get(x.b) != 0 {
        body(x, 0);
        lib::sound(x, crate::hw::sound::CMD_20);
    } else {
        x.set8(Enemy::ACTIVE, 1);
        x.set_bit(Enemy::DISPLAY, 1, false);
        decide::wait(x);
        lib::set_immunities(x);
        body(x, 0);
    }
}

/// Creates an ENEMY double of type `kind` (the pool is awaited as long as it is full) and links it to the boss
/// (`link`). `life`: (deduction, minimum, box set) for a double that has its own HP.
fn spawn_double<R: Bench>(x: &mut Cx<'_, R>, kind: u8, life: Option<(u16, u16, u8)>, link: u32) {
    let Some(d) = alloc_retry(x, &pools::ENEMY) else { return };
    d.set_u8(x.b, Enemy::ACTIVE, 1);
    d.set_u8(x.b, Enemy::KIND, kind);
    if let Some((cut, least, boxes)) = life {
        let hp = x.u16(Enemy::HP_MAX);
        let mut v = hp.wrapping_sub(cut);
        if (v as i16) < 0 {
            v = least;
        }
        for off in [Enemy::HP_MAX, Enemy::HP, Enemy::HP_LAST] {
            d.set_u16(x.b, off, v);
        }
        d.set_u8(x.b, f::BOX_SET, boxes);
    }
    d.set_u32(x.b, f::IMMUNITY, 0xffff_ffff);
    if life.is_some() {
        for (off, v) in [(Boss::ANIM_KIND, 0), (Boss::ANIM_LAST, 0xff), (Enemy::EVENT_LOW, 0), (Enemy::LOOT_ROW, 0), (Enemy::LOOT, 0)] {
            d.set_u8(x.b, off, v);
        }
    }
    Double(d).set_master(x.b, x.o.0 as u16);
    x.set16(link, d.0 as u16);
}

// --- state 2: active -----------------------------------------------------------------------------

/// 0x81d6e: one group per frame (outside freeze and pause), then play area and shadow.
fn active<R: Bench>(x: &mut Cx<'_, R>) {
    if tst8(x, f::FREEZE).gt() {
        sub8(x, f::FREEZE, 1);
    } else if var::PAUSE.get(x.b) == 0 {
        match x.u8(f::SUB) {
            0 => fight::intro(x),
            2 => fight::run(x),
            4 => attacks::run(x),
            6..=12 => fight::wait_for_players(x),
            _ => hit::resume(x),
        }
    }
    lib::keep_in_zone(x);
    lib::collide_terrain(x);
}

handlers! {
    BOSS_4 0x81b3a "boss_4" run,
    // return to combat after a common reaction (table 0x3e000 of `reactions.rs`)
    BOSS_4_GETUP 0x82ea0 "boss_4_getup" hit::resume,
}
