//! "Axe Gnoll" enemy (ENEMY type 13, 0x6ab52). See `docs/families/enemy.md`.
//!
//! Same family as the Goblin and the Troll (AI parameters, even arrival variants, moves chosen from rows of 32 bytes
//! per difficulty level) but armed with an axe: each attack sets the attack type `+0x44` and its strength `+0x45`, the
//! weapon `+0xa6` / `+0xbf` is put down or picked up by program routines (0xa09ca, 0xa114a), and several attacks are
//! jumps onto the target.
//!
//! | `+5` | File | Role (h) |
//! |---|---|---|
//! | 0 | `arrival` | arrival according to variant `+3` (0 walk, 2 and 6 fall, 4 side jump, 8 and 10 armed arrival) |
//! | 2 | `stance` | standing: approach, turn-around, wait |
//! | 4, 6, 8 | `attacks` | moves (`+6` = 0 to 24), series of axe strikes, jumps, teleportation |
//! | 10 | `stance` | waits as long as no player is alive |
//! | state 4 | `hit` | reactions to hits, recoil, fall, grab by a player |
//!
//! Animations 0x1649d0 (movements, `+0xa6` added to the taunt animation) and 0x1649e8 (attacks).

use crate::families::cx::Bench;
use crate::record::record;
use crate::data::RomAddr;
use super::cx::{handlers, Cx};
use super::enemy_common::*;
use crate::objects::Obj;

mod arrival;
mod attacks;
mod hit;
mod stance;

/// Fields of the family beyond the common ones of `Enemy` (`record!`). A meaning guessed from the code alone is
/// marked (h).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Gnoll(pub Obj);

impl Gnoll {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Gnoll {
        look_prev, set_look_prev, LOOK_PREV: u8 @ 0xa1;
        /// Timer (byte) of a gesture.
        timer, set_timer, TIMER: u8 @ 0xa2;
        cooldown, set_cooldown, COOLDOWN: u8 @ 0xa4;
        /// Chosen move (value of `+6` to resume).
        chosen, set_chosen, CHOSEN: u8 @ 0xa5;
        /// The weapon is in hand (the taunt and the get-up use animation +1).
        weapon, set_weapon, WEAPON: u8 @ 0xa6;
        /// Same flag, used by the hit state (fall animations).
        armed, set_armed, ARMED: u8 @ 0xbf;
    }
}
use crate::objects::pools;
use crate::objects::Current;

const SET_MOVE: RomAddr = RomAddr(0x1649d0);
const SET_STRIKE: RomAddr = RomAddr(0x1649e8);
const STANCE: u32 = 0x0202_0000;
const IDLE: u32 = 0x020a_0000;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    if var::GAME_ON.get(x.b) != 0 {
        x.tick_counters();
    }
    match x.u8(f::STATE) {
        st::INIT => {
            if init(x) {
                active(x);
            }
        }
        st::ACTIVE => active(x),
        st::HIT => hit::run(x),
        st::DEAD => x.free(&pools::ENEMY),
        _ => {}
    }
}

// --- aides --------------------------------------------------------------------------------------

/// Starts animation `n` of the movement set.
fn mv<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.anim_start(SET_MOVE.0, n as u16);
}

/// Starts animation `n` of the attack set.
fn strike<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.anim_start(SET_STRIKE.0, n as u16);
}

/// Starts attack `n`: attack type `kind` and strength `power`.
fn strike_with<R: Bench>(x: &mut Cx<'_, R>, n: u32, kind: u8, power: u8) {
    x.set8(Enemy::STRIKE_KIND, kind);
    x.set8(Enemy::STRIKE_POWER, power);
    strike(x, n);
}

fn set_speed<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.select_speed(&crate::objects::motion::SPEEDS_B, n as u16);
}

/// 0x6ad12: sound then animation 4.
fn wind_up<R: Bench>(x: &mut Cx<'_, R>) {
    lib::sound(x, crate::sound::CMD_335_435);
    mv(x, 4);
}

/// 0x6c01a: end of a move; standing, and if the draw says so, wait (`+5 = 10`).
fn done_attack<R: Bench>(x: &mut Cx<'_, R>) {
    set_state_long(x, STANCE);
    if lib::maybe_draw_6e(x) {
        x.set8(f::SUB, 0xa);
    }
}

/// 0x6c04a: is a player in box `(d0, d1, d2)` and accepted by the AI gates (0xa4b4, and 0xa4c2 if it looks in the same
/// direction)?
fn box_check<R: Bench>(x: &mut Cx<'_, R>, d0: i16, d1: u16, d2: u16) -> bool {
    if !lib::target_in_box(x, d0, d1, d2) {
        return false;
    }
    let target = load_target(x);
    let facing = target.u8(x.b, Enemy::FLIP);
    if facing == x.u8(f::FLIP) && !lib::decision_b(x) {
            return false;
        }
    if !lib::decision_a(x) {
        return false;
    }
    true
}

/// 0x6c040: player very close.
fn near_box<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    box_check(x, 0, 0x70, 0x10)
}

/// 0x6c032: player at a short distance, in front.
fn far_box<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    box_check(x, -0x80, 0x30, 0x10)
}

/// 0x6c078: the Gnoll has its back to its target.
fn faces_wrong<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if x.u16(f::TARGET) == 0 {
        return false;
    }
    let target = load_target(x);
    let dx = x.u16(Obj::X).wrapping_sub(target.u16(x.b, Obj::X));
    let want = if (dx as i16) < 0 { 0x20 } else { 0 };
    want as u8 != x.u8(f::FLIP)
}

const NEAR_TABLE: &str = "gnoll/near_moves";
const FAR_TABLE: &str = "gnoll/far_moves";
/// Rows of moves that follow a series (substate 8).
const FOLLOW_TABLE: &str = "gnoll/follow_moves";

/// 0x6c0a6 (near series) / 0x6c0a0 (far series): chooses the move (`+5 = 4`, `+6 = 0`).
fn pick_attack<R: Bench>(x: &mut Cx<'_, R>, table: &str) -> u8 {
    let b = rank_byte(x, table);
    x.set8(f::SUB, 4);
    x.set16(f::SUB2, 0);
    x.set8(Gnoll::CHOSEN, b);
    b
}

/// 0x6bdec: after a series, chooses the fallback move (substate 8).
fn follow_up<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Enemy::AI_SHOT, 0xff);
    let b = rank_byte(x, FOLLOW_TABLE);
    x.set8(f::SUB, 8);
    x.set8(f::SUB2, b);
    x.set8(f::SUB3, 0);
}

/// 0x6baf2: the Gnoll is grabbed by a player: drop animation, then it disappears.
fn grab<R: Bench>(x: &mut Cx<'_, R>) {
    if paused(x) {
        return;
    }
    if x.u8(f::SUB2) == 0 {
        x.add8(f::SUB2, 2);
        return wind_up(x);
    }
    if x.anim_step() {
        detach_from_host(x);
        x.add8(f::STATE, 2);
    }
}

// --- state 0: creation --------------------------------------------------------------------------

/// False if the game is paused (nothing is done); otherwise the body of the active state follows within the same frame.
fn init<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if paused(x) {
        return false;
    }
    x.add8(f::STATE, 2);
    x.set8(f::BOX_SET, 0x19);
    lib::initial_hp(x);
    lib::set_immunities(x);
    for (off, v) in [
        (Enemy::AI_68, 4),
        (Enemy::AI_69, 1),
        (Enemy::AI_STANCE_ROW, 0),
        (Enemy::AI_WANDER_ROW, 1),
        (Enemy::AI_WAIT_MASKS, 3),
        (Enemy::AI_WAIT_ROW, 3),
        (Enemy::AI_DECISION_A, 3),
        (Enemy::AI_DECISION_B, 2),
        (Enemy::AI_BOX_CHANCE, 3),
        (Enemy::AI_BOX_ANIM, 0x38),
        (Enemy::AI_BYTE_ROW, 3),
        (Gnoll::TIMER, 0x20),
        (Gnoll::LOOK_PREV, 0),
        (Gnoll::COOLDOWN, 2),
        (Enemy::VULNERABLE, 1),
        (Enemy::AI_ROW_8E, 0),
        (Enemy::EVENT_LOW, 0),
        (Enemy::EVENT_KIND, 0xe),
    ] {
        x.set8(off, v);
    }
    x.set32(Enemy::RESUME_STATE, 0);
    x.set8(Gnoll::WEAPON, 0);
    x.set8(Gnoll::ARMED, 0);
    x.set8(Enemy::AI_SHOT, 0xff);
    x.set8(Enemy::LOOT_ROW, 0xd);
    x.set8(Enemy::LOOT, 0xb);
    true
}

// --- state 2: active -----------------------------------------------------------------------------

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    let freeze = x.u8(f::FREEZE) as i8;
    if freeze > 0 {
        x.set8(f::FREEZE, (freeze - 1) as u8);
    } else if !paused(x) {
        match x.u8(f::SUB) {
            0 => arrival::run(x),
            2 => stance::run(x),
            4 | 6 | 8 => attacks::run(x),
            10 => stance::idle(x),
            _ => {}
        }
    }
    super::boss_common::push_out_of_boss(x);
    zone_and_terrain(x);
    x.reinsert();
}

handlers! {
    0x6ab52 "enemy_13" run,
}
