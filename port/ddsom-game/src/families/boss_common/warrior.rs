//! Shared by the two black warriors, bosses 7 and 8 ("DarkWarrior 1" and "DarkWarrior 2"): their common fields
//! (`Warrior`) and their common aids (choice of a player, sweep of the players, about-face, HP threshold of the slow
//! motion). The tables and play areas that differ are passed by each boss.

use crate::families::cx::Bench;
use crate::objects::Obj;
use crate::record::record;
use crate::ram::PLAYERS;

use super::*;

/// The fields of a black warrior beyond those of `Enemy` and `Boss` (`record!`). A meaning guessed from the code alone
/// is marked (h).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Warrior(pub Obj);

impl Warrior {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Warrior {
        /// Threshold of the slow motion armed (`hp_threshold`: half the HP kept in `WEAK_HP`).
        threshold_armed, set_threshold_armed, THRESHOLD_ARMED: u8 @ 0x42;
        /// No threshold of the slow motion (`hp_threshold`): the boss never slows down.
        no_threshold, set_no_threshold, NO_THRESHOLD: u8 @ 0x43;
        /// Non-zero while the frame is replayed by the slow motion (a hit then adds a frame of freeze).
        replay, set_replay, REPLAY: u8 @ 0x46;
        /// Hits received before the getting up (word).
        hits, set_hits, HITS: u16 @ 0x84;
        /// Countdown of the steps (word).
        timer, set_timer, TIMER: u16 @ 0xa0;
        /// Animation key requested and last one started.
        anim_kind, set_anim_kind, ANIM_KIND: u8 @ 0xa2;
        anim_last, set_anim_last, ANIM_LAST: u8 @ 0xa3;
        /// Step `+6` and state long to resume after an about-face.
        resume_sub, set_resume_sub, RESUME_SUB: u8 @ 0xa5;
        resume_state, set_resume_state, RESUME_STATE: u32 @ 0xa6;
        /// Frames during which the frame tag `+0x38` is cleared after an attack (h).
        phase_timer, set_phase_timer, PHASE_TIMER: u8 @ 0xab;
        /// Shots still to fire in the current volley.
        volley, set_volley, VOLLEY: u8 @ 0xac;
        /// Living players at the creation (1 to 3): rule of the slow-motion threshold.
        phases, set_phases, PHASES: u8 @ 0xad;
        /// Set when a hit is acknowledged (the boss becomes vulnerable again at the end of its combo).
        hit_ack, set_hit_ack, HIT_ACK: u8 @ 0xae;
        /// Players already aimed at by the volley (one bit per player).
        picked, set_picked, PICKED: u8 @ 0xaf;
        /// Frames before the creation of the auxiliary creature (word).
        spawn_time, set_spawn_time, SPAWN_TIME: u16 @ 0xb4;
        /// HP below which the slow motion starts (word).
        weak_hp, set_weak_hp, WEAK_HP: u16 @ 0xb6;
        /// The auxiliary creature was created.
        spawned, set_spawned, SPAWNED: u8 @ 0xb8;
        /// Slow motion: active, step (3 to 0), counter of the repeated attack.
        slow, set_slow, SLOW: u8 @ 0xb9;
        slow_tick, set_slow_tick, SLOW_TICK: u8 @ 0xba;
        third, set_third, THIRD: u8 @ 0xbb;
        /// Countdowns of the wait without fighting (words).
        idle_time, set_idle_time, IDLE_TIME: u16 @ 0xbc;
        idle_wait, set_idle_wait, IDLE_WAIT: u16 @ 0xbe;
    }
}

/// What differs between the two warriors: the prefix of their tables in `data/enemies.json` (`boss_7`, `boss_8`), their
/// play area, the depth below which a dodge takes the lower spots, the number of consecutive attacks 4 that the slow
/// motion turns into attack 14, and the box of the follow-up of an attack.
pub struct Kit {
    pub tables: Tables,
    /// Play area: X from `x.0` to `x.1`, depth from `y.0` to `y.1` (exclusive bounds of the original's comparisons).
    pub x: (u16, u16),
    pub y: (u16, u16),
    pub dodge_low: u16,
    pub repeats: u8,
    pub follow_box: (i16, u16, u16),
}

/// The tables of a warrior.
pub struct Tables {
    pub sidesteps: &'static str,
    pub dodge_spots: &'static str,
    pub front_attacks: &'static str,
    pub front_attacks_slow: &'static str,
    pub back_attacks: &'static str,
    pub back_attacks_slow: &'static str,
    pub follow_ups: &'static str,
}

/// Combat state (group 2, walk).
const FIGHT: u32 = 0x0202_0000;

/// 0x87048 (boss 7) and 0x89788 (boss 8): depending on the scene difficulty (`var::SCENE_LEVEL`: 0 nothing, 3 always)
/// and the living players at the creation (`PHASES`, 2 or 3: depending on HP), sets `NO_THRESHOLD` or, if half the HP
/// exceeds 72, keeps that threshold in `WEAK_HP` and sets `THRESHOLD_ARMED`. The slow motion stays off.
pub fn hp_threshold<R: Bench>(x: &mut Cx<'_, R>) {
    let level = var::SCENE_LEVEL.get(x.b);
    if level == 0 {
        return;
    }
    let by_hp = level != 3 && matches!(x.u8(Warrior::PHASES), 2 | 3);
    if by_hp {
        if x.u8(Warrior::THRESHOLD_ARMED) != 0 {
            return;
        }
        let half = x.u16(Enemy::HP) >> 1;
        // the original computes the half in D1.w (output contract)
        if half as i16 > 72 {
            x.set8(Warrior::SLOW, 0);
            x.set16(Warrior::WEAK_HP, half);
            x.set8(Warrior::THRESHOLD_ARMED, 1);
            return;
        }
    }
    x.set8(Warrior::NO_THRESHOLD, 1);
    x.set8(Warrior::SLOW, 0);
}

/// 0x86fee / 0x8972e (`remember`) and 0x87020 / 0x89760: randomly picks a living player as target `+0x50`; with
/// `remember`, only a player not yet designated (bit in `PICKED`), then marks it.
pub fn pick<R: Bench>(x: &mut Cx<'_, R>, remember: bool) {
    for _ in 0..4096 {
        let rng = x.random();
        let n = rng as u32 & 3;
        if remember && x.u8(Warrior::PICKED) >> n & 1 != 0 {
            continue;
        }
        let player = PLAYERS.nth(n);
        if Enemy(player).active(x.b) == 0 {
            continue;
        }
        if remember {
            let mask = x.u8(Warrior::PICKED) | 1 << n;
            x.set8(Warrior::PICKED, mask);
        }
        x.set16(Enemy::TARGET, player.0 as u16);
        return;
    }
}

/// Sweep of the players: is one of them in the box (`d0`, `d1`, `d2`) in front of the boss (0x7f19c)? `exactly_one`
/// tests their activity byte against 1 (`cmpi.b #1`) instead of non-zero (`tst.b`).
pub fn scan<R: Bench>(x: &mut Cx<'_, R>, exactly_one: bool, d0: i16, d1: u16, d2: u16) -> bool {
    for player in PLAYERS.iter() {
        let alive = Enemy(player).active(x.b);
        let ok = if exactly_one { alive == 1 } else { alive != 0 };
        if ok && lib::in_box(x, player, d0, d1, d2).found() {
            return true;
        }
    }
    false
}

/// 0x87b86 / 0x8a2c6: true if an animation is in progress (`ANIM_LAST`) and the boss is not facing its opponent
/// (`+0x50`).
pub fn must_turn<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if x.u8(Warrior::ANIM_LAST) == 0 || x.u16(Enemy::TARGET) == 0 {
        return false;
    }
    let t = load_target(x);
    let mine = x.u16(Obj::X);
    let face = if Fl::sub16(t.u16(x.b, Obj::X), mine).gt() { 0x20 } else { 0 };
    face != x.u8(Enemy::FLIP)
}

/// 0x87c6c / 0x8a404: sidestep drawn from the table of sidesteps, toward the back.
pub fn sidestep<R: Bench>(x: &mut Cx<'_, R>, kit: &Kit) {
    let rng = x.random();
    let i = rng & 0xe;
    let mut d = word(x, kit.tables.sidesteps, i as u32);
    if x.u8(Enemy::FLIP) != 0 {
        d = d.wrapping_neg();
    }
    x.add16(Obj::X, d);
}

/// 0x87cfe / 0x8a49a: brings the boss back into its play area.
pub fn clamp<R: Bench>(x: &mut Cx<'_, R>, kit: &Kit) {
    let y = x.u16(Obj::Y);
    if Fl::sub16(y, kit.y.0).gt() {
        if !Fl::sub16(y, kit.y.1).lt() {
            x.set16(Obj::Y, kit.y.1);
        }
    } else {
        x.set16(Obj::Y, kit.y.0);
    }
    let px = x.u16(Obj::X);
    if Fl::sub16(px, kit.x.0).gt() {
        if !Fl::sub16(px, kit.x.1).lt() {
            x.set16(Obj::X, kit.x.1);
        }
    } else {
        x.set16(Obj::X, kit.x.0);
    }
}

/// A series of effect puffs (0xb10c8) along the axis: D0 = 1, D1 = 0, D2 = position, D3 = 4.
fn bursts<R: Bench>(x: &mut Cx<'_, R>, spots: &[i16]) {
    for &p in spots {
        lib::shock(x, 1, 0, p as u16, 4);
    }
}

/// 0x87d48 / 0x8a4e4: five puffs.
pub fn burst_small<R: Bench>(x: &mut Cx<'_, R>) {
    bursts(x, &[-0x28, -0x18, 0, 0x18, 0x28]);
}

/// 0x87da2 / 0x8a53e: seven puffs forward.
pub fn burst_front<R: Bench>(x: &mut Cx<'_, R>) {
    bursts(x, &[-0x50, -0x38, -0x20, -8, 0x10, 0x28, 0x40]);
}

/// 0x87e20 / 0x8a5bc: seven puffs backward.
pub fn burst_back<R: Bench>(x: &mut Cx<'_, R>) {
    bursts(x, &[0x50, 0x38, 0x20, 8, -0x10, -0x28, -0x40]);
}

/// 0x87e9e / 0x8a63a: if a player is very close (0x7f19c: 0, 0x50, 0x20), the boss jumps elsewhere.
pub fn dodge_nearby<R: Bench>(x: &mut Cx<'_, R>, kit: &Kit) {
    if scan(x, false, 0, 0x50, 0x20) {
        dodge_spot(x, kit);
    }
}

/// 0x87eca / 0x8a666: a landing point (X and depth offsets, orientation) drawn from a table chosen according to the
/// quarter of the screen where the boss is, then back into the play area.
pub fn dodge_spot<R: Bench>(x: &mut Cx<'_, R>, kit: &Kit) {
    let dx = x.u16(Obj::X).wrapping_sub(var::SCROLL_X.get(x.b));
    let dy = x.u16(Obj::Y).wrapping_sub(var::SCROLL_Y.get(x.b));
    let right = Fl::sub16(dx, 0xc0).gt();
    let low = Fl::sub16(dy, kit.dodge_low).gt();
    let quarter = match (right, low) {
        (false, false) => 0,
        (false, true) => 4,
        (true, false) => 8,
        (true, true) => 0xc,
    };
    let spots = long(x, kit.tables.dodge_spots, quarter);
    let rng = x.random();
    let at = spots.wrapping_add(((rng & 3) << 3) as u32);
    let dx = tables::held16(x.b, at);
    x.add16(Obj::X, dx);
    let dy = tables::held16(x.b, at + 2);
    x.add16(Obj::Y, dy);
    let side = tables::held8(x.b, at + 4);
    x.set8(Enemy::FLIP, side);
    clamp(x, kit)
}

/// 0x870a2 / 0x897e2 (`front`) and 0x870b4 / 0x897f4: draws the next attack (according to the slow motion and the
/// difficulty). In slow motion, the `repeats`-th consecutive attack 4 becomes attack 14.
pub fn choose_attack<R: Bench>(x: &mut Cx<'_, R>, kit: &Kit, front: bool) {
    let slow = x.u8(Warrior::SLOW) != 0;
    let t = &kit.tables;
    let table = match (front, slow) {
        (true, false) => t.front_attacks,
        (true, true) => t.front_attacks_slow,
        (false, false) => t.back_attacks,
        (false, true) => t.back_attacks_slow,
    };
    pick_row(x, table, 0x204);
    if slow && x.u8(Enemy::STEP) == 4 {
        addq8(x, Warrior::THIRD, 1);
        if x.u8(Warrior::THIRD) == kit.repeats {
            x.set32(Enemy::STATE_LONG, 0x0204_0e00);
            x.set8(Warrior::THIRD, 0);
        }
    }
}

/// Draws a step in the row of the difficulty of `table` and moves to state `state` (word `+4`), step `+7` = 0.
fn pick_row<R: Bench>(x: &mut Cx<'_, R>, table: &str, state: u16) {
    let step = rank_byte(x, table);
    x.set8(Enemy::STEP, step);
    x.set16(Enemy::STATE_WORD, state);
    x.set8(Enemy::SUB_STEP, 0);
}

/// 0x87926 / 0x8a066: end of an attack. A player in the box of the follow-up makes the follow-up be drawn (state 2,
/// group 8); otherwise the boss goes back to combat.
pub fn after_attack<R: Bench>(x: &mut Cx<'_, R>, kit: &Kit) {
    let (d0, d1, d2) = kit.follow_box;
    if !scan(x, false, d0, d1, d2) {
        x.set32(Enemy::STATE_LONG, FIGHT);
        return;
    }
    pick_row(x, kit.tables.follow_ups, 0x208);
}
