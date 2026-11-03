//! Boss of type 22 (0x9aeaa): the creature that the type 2 boss releases on its death (h), to which it hands over
//! (`+0x42` = its host). See `docs/families/boss.md`.
//!
//! Same template as type 16 (`boss_16`): even `+5` groups, steps `+6` and `+7`, states set in one go as longs, two
//! animation sets (body 0x18492a, attacks 0x184964). Its second form (`+0xb5` raised) has other animations and
//! other attacks. The reactions to hits are the common routines (`reactions.rs`).
//!
//! | `+5` | File | Role |
//! |---|---|---|
//! | 0 | `moves` | arrival: falls from the top of the screen |
//! | 2 | `moves` | combat: wait (`+6` = 0 and 8), about-face (2), leap toward the target (6), charge (4) |
//! | 4 | `attacks` | attacks: six hits (`+6` = 0 to 22) |
//! | 6 | `moves` | run to a point on the screen (`+6` = 0 and 2), jump (4 and 6) |
//! | 8 | `moves` | same after an attack |
//! | 10 | `moves` | pause |
//! | 12 | `moves` | a grabbed player (`+0x50`): bites |
//! | 14 | `moves` | transformation into the second form |
//! | state 4 | `hit` | reactions to hits, knockback, on the ground, death |

use crate::families::cx::Bench;
use crate::objects::Obj;
use crate::record::record;
use crate::data::RomAddr;
use crate::player::Player;
use super::boss_common::*;
use super::cx::handlers;

mod attacks;
mod hit;
mod moves;
mod two;

/// Fields of this boss beyond those of `Enemy` and `Boss` (`record!`). A meaning guessed from the code alone is
/// marked (h).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Boss22(pub Obj);

impl Boss22 {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Boss22 {
        /// Host: the type 22 boss for type 2, and the reverse (word).
        host, set_host, HOST: u16 @ 0x42;
        /// Delay (word) before a player too close triggers the counterattack, and its countdown.
        guard, set_guard, GUARD: u16 @ 0x46;
        guard_timer, set_guard_timer, GUARD_TIMER: u16 @ 0x5c;
        /// Timers (words, or bytes at the same address).
        timer, set_timer, TIMER: u16 @ 0xa0;
        /// Current animation (number) and last one started.
        anim_id, set_anim_id, ANIM_ID: u8 @ 0xa2;
        anim_last, set_anim_last, ANIM_LAST: u8 @ 0xa3;
        /// Type 2: non-zero once the stage controller (SET 2, variant 0x3e) is created.
        controller, set_controller, CONTROLLER: u8 @ 0xa4;
        /// State to resume after an about-face: long at `+0xa6` (states) and byte at `+0xa5` (step `+6`).
        saved_step, set_saved_step, SAVED_STEP: u8 @ 0xa5;
        saved_state, set_saved_state, SAVED_STATE: u32 @ 0xa6;
        timer2, set_timer2, TIMER2: u16 @ 0xaa;
        /// Type 2: non-zero when a type 22 boss is linked to it (`HOST`).
        partnered, set_partnered, PARTNERED: u8 @ 0xac;
        /// Type 22: cleared at each frame of the combat, read by no code of the boss (h).
        fight_mark, set_fight_mark, FIGHT_MARK: u8 @ 0xad;
        /// 0xff: a run-up may be drawn (`dashes`); 2 after a run-up.
        rage, set_rage, RAGE: u8 @ 0xae;
        /// Raised at the strike of a type 2 attack, cleared when the boss is hit; the attack ends when it is zero (h).
        busy, set_busy, BUSY: u8 @ 0xaf;
        /// Aimed point (words), rotation direction of the trajectory.
        spot_x, set_spot_x, SPOT_X: u16 @ 0xb0;
        spot_z, set_spot_z, SPOT_Z: u16 @ 0xb2;
        spin, set_spin, SPIN: u8 @ 0xb4;
        /// Second form.
        second, set_second, SECOND: u8 @ 0xb5;
        /// Type 2: side of the run to a point, reversed at each new point (h).
        sway, set_sway, SWAY: u8 @ 0xb6;
        /// Counter of the dust puffs (one every eight calls).
        effect_count, set_effect_count, EFFECT_COUNT: u8 @ 0xb7;
        /// Type 2: cleared at the end of the roar, read by no code of the boss (h).
        flash_mark, set_flash_mark, FLASH_MARK: u8 @ 0xb8;
        /// Type 2: frames of screen shake left during the roar.
        shake_time, set_shake_time, SHAKE_TIME: u8 @ 0xb9;
        /// Non-zero once the hit of the current frame has been counted.
        hit_tick, set_hit_tick, HIT_TICK: u8 @ 0xba;
        /// Fuse of the counterattack.
        arm_timer, set_arm_timer, ARM_TIMER: u8 @ 0xbb;
        /// Countdown of the run to a point (type 22) or before the stage controller (type 2) (word).
        spot_timer, set_spot_timer, SPOT_TIMER: u16 @ 0xbc;
        /// Bites left on a grabbed player.
        bites, set_bites, BITES: u8 @ 0xbe;
        /// Effect of the second form.
        second_fx, set_second_fx, SECOND_FX: u8 @ 0xbf;
    }
}
use crate::objects::pools;
use crate::objects::Current;

/// Animation sets of a boss.
#[derive(Clone, Copy)]
enum Set {
    A,
    B,
}

/// What distinguishes the type 2 boss from the type 22 one: the code is that of a single family, copied with other
/// animation sets and other ROM tables.
struct Kit {
    set_a: RomAddr,
    set_b: RomAddr,
    /// Attack tables (32 bytes per pair of levels): short range, long range, end of attack.
    close: &'static str,
    far: &'static str,
    next: &'static str,
    /// Arrival points of the run (6-byte rows: X, Z, angle, direction) and counterattack chances (rows of
    /// `1 << chances_shift` bytes).
    spot: &'static str,
    chances: &'static str,
    chances_shift: u32,
    /// Run-up draw table; hit reaction table (the key of `reactions/boss_22_*`).
    dash: &'static str,
    reactions: RomAddr,
}

const TWO_KIT: Kit = Kit {
    set_a: RomAddr(0x1733ac),
    set_b: RomAddr(0x1733e6),
    close: "boss_2/close_attacks",
    far: "boss_2/far_attacks",
    next: "boss_2/next_attacks",
    spot: "boss_2/spots",
    chances: "boss_2/counter_chances",
    chances_shift: 2,
    dash: "boss_2/dashes",
    reactions: RomAddr(0x7e5e2),
};

const PART_KIT: Kit = Kit {
    set_a: RomAddr(0x18492a),
    set_b: RomAddr(0x184964),
    close: "boss_22/close_attacks",
    far: "boss_22/far_attacks",
    next: "boss_22/next_attacks",
    spot: "boss_22/spots",
    chances: "boss_22/counter_chances",
    chances_shift: 4,
    dash: "boss_22/dashes",
    reactions: RomAddr(0x9ce66),
};

/// Is the boss type 2 (and not type 22)?
fn two<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    x.u8(Enemy::KIND) == 2
}

fn kit<R: Bench>(x: &mut Cx<'_, R>) -> &'static Kit {
    if two(x) { &TWO_KIT } else { &PART_KIT }
}

/// Starts animation `n` of set `set` (0x1a32).
fn start<R: Bench>(x: &mut Cx<'_, R>, set: Set, n: u32) {
    let k = kit(x);
    x.anim_start(if matches!(set, Set::A) { k.set_a.0 } else { k.set_b.0 }, n as u16);
}

/// States set in one go (`+4..+7`).
mod state {
    pub const STAND: u32 = 0x0202_0000;
    pub const TURN: u32 = 0x0202_0200;
    pub const LEAP: u32 = 0x0202_0600;
    pub const IDLE: u32 = 0x020a_0000;
}

// --- animations --------------------------------------------------------------------------------

/// Animation number `id` (+0xa2); starts animation `n` of set `set` if it is not the last one started (+0xa3).
fn pose<R: Bench>(x: &mut Cx<'_, R>, set: Set, id: u8, n: u32) {
    x.set8(Boss22::ANIM_ID, id);
    if x.u8(Boss22::ANIM_ID) != x.u8(Boss22::ANIM_LAST) {
        let id = x.u8(Boss22::ANIM_ID);
        x.set8(Boss22::ANIM_LAST, id);
        start(x, set, n);
    }
}

/// Sound settings `+0x44` and `+0x45` set along with the animation.
fn sounds<R: Bench>(x: &mut Cx<'_, R>, a: u8, b: u8) {
    x.set8(Enemy::STRIKE_KIND, a);
    x.set8(Enemy::STRIKE_POWER, b);
}

/// Animation of the body or of the attacks depending on whether it is in its second form.
fn second_form<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    x.u8(Boss22::SECOND) != 0
}

fn next7<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Enemy::SUB_STEP, 2);
}
fn next6<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB2, 2);
}

/// A dust puff / explosion `D0 .. D3` (0xb10c8) every eight calls (`+0xb7`).
fn rubble<R: Bench>(x: &mut Cx<'_, R>, d0: u32, d1: u32, d2: u32, d3: u32) {
    x.add8(Boss22::EFFECT_COUNT, 1);
    let n = x.u8(Boss22::EFFECT_COUNT) & 7;
    x.set8(Boss22::EFFECT_COUNT, n);
    if n != 0 {
        return;
    }
    lib::shock(x, d0, d1 as u16, d2 as u16, d3 as u16);
}

/// 0x9d8e0: is the target on the wrong side (the boss is not facing it)?
fn needs_turn<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if x.u16(f::TARGET) == 0 {
        return false;
    }
    let t = x.linked(f::TARGET);
    let ahead = (t.u16(x.b, Obj::X).wrapping_sub(x.u16(Obj::X)) as i16) > 0;
    let wanted = if ahead { 0x20 } else { 0 };
    wanted != x.u8(f::FLIP)
}

/// 0x9d928: is the target far in both depth and X?
fn target_far<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if x.u16(f::TARGET) == 0 {
        return false;
    }
    let t = x.linked(f::TARGET);
    let dz = t.u16(x.b, Obj::Y).wrapping_sub(x.u16(Obj::Y)).wrapping_add(0x20);
    if dz <= 0x40 {
        return false;
    }
    let dx = t.u16(x.b, Obj::X).wrapping_sub(x.u16(Obj::X)).wrapping_add(0xc0);
    dx > 0x180
}

/// 0x9b30a and 0x9b384: a target in the box, with possible hits (0xa4b4 / 0xa4c2); the boss charges or attacks.
fn hunt_box<R: Bench>(x: &mut Cx<'_, R>, d0: i16, d1: u16, d2: u16, table: &str) -> bool {
    if !lib::target_in_box(x, d0, d1, d2) || x.u16(f::TARGET) == 0 {
        return false;
    }
    let t = x.linked(f::TARGET);
    let same = t.u8(x.b, f::FLIP) == x.u8(f::FLIP);
    if !lib::decision(x, same) {
        return false;
    }
    let r = rand_and(x, 0xf);
    if 0xffc0u32 >> r & 1 != 0 {
        x.set32(f::STATE, 0x0202_0800);
        return true;
    }
    x.set32(f::STATE, 0x0204_0000);
    lib::pick_byte(x);
    attack_from_table(x, table);
    true
}

fn hunt_close<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let table = kit(x).close;
    hunt_box(x, -0x40, 0x40, 8, table)
}
fn hunt_far<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let table = kit(x).far;
    hunt_box(x, -0xd0, 0x50, 0x50, table)
}

/// 0x9d24e and 0x9d254: attack drawn from the difficulty row of `table` (32 bytes per pair of levels).
fn attack_from_table<R: Bench>(x: &mut Cx<'_, R>, table: &str) {
    let row = ((var::RANK.get(x.b) >> 1) as u32) << 5;
    let r = rand_and(x, 0x1f);
    let step = byte(x, table, row + r);
    x.set8(f::SUB2, step);
    x.set16(f::STATE, 0x0204);
    x.set8(Enemy::SUB_STEP, 0);
}

/// 0x9d680 (0x7ee1e for type 2): end of an attack: standing, or a new attack if a player is very close.
fn next_attack<R: Bench>(x: &mut Cx<'_, R>) {
    for k in 0..4u32 {
        let p = crate::ram::PLAYERS.nth(k);
        if p.u8(x.b, Obj::ACTIVE) == 0 {
            // type 22 stops at the first absent player, type 2 moves on to the next
            if two(x) {
                continue;
            }
            break;
        }
        if reach(x, p, -0x38, 0x38, 0x18) {
            let row = ((var::RANK.get(x.b) >> 1) as u32) << 5;
            let r = rand_and(x, 0x1f);
            let table = kit(x).next;
            let step = byte(x, table, row + r);
            x.set8(f::SUB2, step);
            x.set16(f::STATE, 0x0208);
            x.set8(Enemy::SUB_STEP, 0);
            return;
        }
    }
    x.set32(f::STATE, state::STAND);
}

/// 0x9d9d0: a player very close (box ±0x28 in X, ±8 in depth) at ground height?
fn player_within_reach<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    for k in 0..4u32 {
        let p = crate::ram::PLAYERS.nth(k);
        if p.u8(x.b, Obj::ACTIVE) == 0 {
            continue;
        }
        if reach(x, p, -0x28, 0x28, 8) {
            return true;
        }
    }
    false
}

/// 0x9da00: is `p` in the boss's box (offset `d0`, half-width `d1`, half-depth `d2`), on the ground?
fn reach<R: Bench>(x: &mut Cx<'_, R>, p: crate::objects::Obj, d0: i16, d1: u16, d2: u16) -> bool {
    if p.u16(x.b, Obj::Z) != 0 {
        return false;
    }
    let mut off = d0 as u16;
    if x.u8(f::FLIP) != 0 {
        off = off.wrapping_neg();
    }
    let dx = off.wrapping_add(x.u16(Obj::X)).wrapping_sub(p.u16(x.b, Obj::X)).wrapping_add(d1);
    if dx > d1.wrapping_mul(2) {
        return false;
    }
    let dz = x.u16(Obj::Y).wrapping_sub(p.u16(x.b, Obj::Y)).wrapping_add(d2);
    dz <= d2.wrapping_mul(2)
}

/// 0x9da5c: chooses the arrival point of the run (`+0xb0`, `+0xb2`), the angle and the rotation direction, from the
/// boss's position on the screen (table 0x9dabe: X, Z, angle, direction).
fn choose_spot<R: Bench>(x: &mut Cx<'_, R>) {
    let dx = x.u16(Obj::X).wrapping_sub(var::SCROLL_X.get(x.b));
    let dz = x.u16(Obj::Y).wrapping_sub(var::SCROLL_Y.get(x.b));
    let row = if (dx as i16) > 0xc0 {
        if (dz as i16) > 0x80 { 0x12 } else { 0xc }
    } else if (dz as i16) > 0x80 {
        6
    } else {
        0
    };
    let base = kit(x).spot;
    let sx = word(x, base, row).wrapping_add(var::SCROLL_X.get(x.b));
    x.set16(Boss22::SPOT_X, sx);
    let sz = word(x, base, row + 2).wrapping_add(var::SCROLL_Y.get(x.b));
    x.set16(Boss22::SPOT_Z, sz);
    let angle = byte(x, base, row + 4);
    x.set8(Enemy::HEADING, angle);
    let spin = byte(x, base, row + 5);
    x.set8(Boss22::SPIN, spin);
}

/// 0x9db72: five dust puffs in a line under the boss as it lands.
fn dust_line<R: Bench>(x: &mut Cx<'_, R>) {
    for dz in [-0x30i16, -0x18, 0, 0x18, 0x30] {
        lib::shock(x, 1, 0, dz as u16, 4);
    }
}

/// 0x7f328 and 0x7f3a0: dust puffs in a line, toward the right (the boss faces left) or toward the left.
fn dust_trail<R: Bench>(x: &mut Cx<'_, R>, right: bool) {
    const RIGHT: [i16; 6] = [-0x10, 8, 0x20, 0x38, 0x50, 0x68];
    const LEFT: [i16; 6] = [-0x68, -0x58, -0x38, -0x20, -0x8, 0x10];
    for dz in if right { RIGHT } else { LEFT } {
        lib::shock(x, 1, 0, dz as u16, 4);
    }
}

/// Five calls of 0x9eca4 (bursts), D5 = 4 to 0 like the original's `dbf` loop.
fn scatter<R: Bench>(x: &mut Cx<'_, R>) {
    for i in (0..5u32).rev() {
        *x.c.d5() = i;
        eshl_debris(x, i as u8);
    }
    *x.c.d5() |= 0xffff;
}

/// 0x9db28: fifteen palette colors written at 0x91c340; the boss takes palette offset 0x1a.
fn set_palette<R: Bench>(x: &mut Cx<'_, R>) {
    const COLORS: [u16; 15] = [
        0xf333, 0xfdef, 0xfaed, 0xf9b9, 0xf575, 0xf244, 0xffff, 0xfddd, 0xfa9a, 0xf656, 0xf5cb, 0xf397, 0xf075, 0xf053, 0xf031,
    ];
    for (i, c) in COLORS.iter().enumerate() {
        crate::mem::wr16(x.b, 0x91c340 + 2 * i as u32, *c);
    }
    x.set8(Enemy::PALETTE, 0x1a);
}

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    x.tick_counters();
    if two(x) {
        two::spawn_scene_controller(x);
    }
    match x.u8(f::STATE) {
        st::INIT if two(x) => two::init(x),
        st::INIT => init(x),
        st::ACTIVE => active(x),
        st::HIT => hit::run(x),
        _ => {
            if !two(x) {
                var::BOSS_FIGHT.set(x.b, 0);
            }
            x.free(&pools::BOSS);
        }
    }
}

// --- state 0: creation --------------------------------------------------------------------------

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    set_palette(x);
    let r = x.random() as u32;
    x.set8(Boss22::EFFECT_COUNT, r as u8);
    x.set8(f::BOX_SET, 0x22);
    lib::create_life_bar(x);
    lib::initial_hp_boss(x);
    for (off, v) in [(Enemy::AI_STANCE_ROW, 0), (Enemy::EVENT_KIND, 0x23), (Enemy::EVENT_LOW, 0), (Enemy::AI_ROW_8E, 3), (Enemy::AI_SHOT, 0xff), (Enemy::AI_68, 8), (Enemy::AI_69, 8), (Enemy::AI_WANDER_ROW, 3), (Enemy::AI_WAIT_MASKS, 1), (Enemy::AI_WAIT_ROW, 8)] {
        x.set8(off, v);
    }
    let speed = if x.u16(f::TARGET) != 0 && var::PLAYERS_ALIVE.get(x.b) != 1 { 7 } else { 6 };
    x.set8(Enemy::AI_DECISION_A, speed);
    x.set8(Enemy::AI_DECISION_B, speed);
    for (off, v) in [(Enemy::AI_BOX_CHANCE, 5), (Enemy::AI_BOX_ANIM, 0x3c), (Enemy::AI_BYTE_ROW, 6), (Enemy::LOOT, 0x12), (Enemy::LOOT_ROW, 0x17)] {
        x.set8(off, v);
    }
    x.set16(Boss22::GUARD, 0xb4);
    x.set16(Boss22::GUARD_TIMER, 0xf0);
    var::SCENE_LEVEL.set(x.b, 0);
    x.set32(f::IMMUNITY, 0xffff_ffff);
    pick_target(x);
    lib::draw_stance(x);
    lib::draw_6b(x);
    x.set8(Boss22::RAGE, 0xff);
    x.set8(Boss22::SECOND_FX, 0);
    x.set16(Boss22::SPOT_TIMER, 0x10);
    x.set8(Boss::DEATH_CREDIT, 0);
    // `jmp 0x1a32`: the registers it leaves to the following handler are re-read by no one (`exit` probe)
    let set = kit(x).set_a.0;
    x.anim_start(set, 0);
}

// --- state 2: active -----------------------------------------------------------------------------

/// 0x9afbc: one group per frame (outside freeze and pause); a countdown (`+0x5c`, restart at 0xf0) triggers, when
/// the boss is on the ground and `+0x46` designates an object, state 0x0204_1600 (anger).
fn active<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(f::TARGET) == 0 {
        pick_target(x);
    }
    if tst8(x, f::FREEZE).gt() {
        sub8(x, f::FREEZE, 1);
    } else if var::PAUSE.get(x.b) == 0 {
        group(x);
        if tst8(x, Boss22::ARM_TIMER).gt() {
            sub8(x, Boss22::ARM_TIMER, 1);
            x.set8(Enemy::FRAME_TAG, 0);
        }
    }
    let expired = sub16(x, Boss22::GUARD_TIMER, 1).le();
    let fire = !expired && (x.u16(Boss22::GUARD) as i16) < 0 && x.u16(Obj::Z) == 0;
    if fire {
        x.set8(Enemy::DOWNED, 0);
        x.set16(Boss::DOWN_TIME, 0);
        x.set8(Enemy::REACTION, 0);
        x.set8(Enemy::ACTIVE, 1);
        let fuse = if two(x) { 0x46 } else { 0x50 };
        x.set8(Boss22::ARM_TIMER, fuse);
        x.set32(f::STATE, 0x0204_1600);
    }
    if expired || fire {
        x.set16(Boss22::GUARD, 0xb4);
        x.set16(Boss22::GUARD_TIMER, 0xf0);
    }
    lib::keep_on_screen(x, 0x30);
    lib::collide_terrain(x);
    x.reinsert();
}

fn group<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB) {
        0 if two(x) => two::arrive(x),
        0 => moves::arrive(x),
        2 => moves::fight(x),
        4 => attacks::run(x),
        6 => moves::run_to_spot(x),
        8 => moves::run_to_spot_after_attack(x),
        10 => moves::paused(x),
        12 => moves::holding(x),
        14 => moves::transform(x),
        _ => {}
    }
}

handlers! {
    BOSS_22 0x9aeaa "boss_22" run,
    BOSS_2 0x7c5a8 "boss_2" run,
}
