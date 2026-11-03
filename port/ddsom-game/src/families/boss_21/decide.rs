//! Boss 21: choice of the next action (state changes) and draws in the ROM tables.
//!
//! The states are set in one go as longs (`+4..+7`): `0x0202_0400` = active, group 2 (combat), step 4.
//! The tables are read from the ROM (opcode space: PC-relative addressing). The two-level choices
//! (byte of table 1, then word of table 2) give the address of one of the state routines below;
//! `go` finds the routine from that address.

use crate::families::cx::Bench;
use super::*;

/// Wait: no player alive (group 10).
pub(super) const WAIT: u32 = 0x020a_0000;
const STAND: u32 = 0x0202_0000;
const INTERRUPTED: u32 = 0x0202_0200;
const CHASE: u32 = 0x0202_0400;
const CHASE_B: u32 = 0x0202_0600;
const DASH: u32 = 0x0202_0a00;
/// Attacks (group 4), numbered by their step `+6`.
const ATTACK_0: u32 = 0x0204_0000;
const ATTACK_2: u32 = 0x0204_0200;
const ATTACK_4: u32 = 0x0204_0400;
const ATTACK_6: u32 = 0x0204_0600;
const ATTACK_10: u32 = 0x0204_0a00;
const ATTACK_12: u32 = 0x0204_0c00;
const ATTACK_16: u32 = 0x0204_1000;

/// 0x9ac32: the boss waits (no player alive).
pub(super) fn wait<R: Bench>(x: &mut Cx<'_, R>) {
    x.set32(f::STATE, WAIT);
}

/// 0x9aaf2: standing, motionless and vulnerable.
pub(super) fn stand<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Enemy::VULNERABLE, 1);
    x.set32(f::STATE, STAND);
}

/// 0x9ab02: interrupts the action in progress (the state is saved in `+0xa2`).
pub(super) fn interrupt<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Enemy::VULNERABLE, 0);
    let state = x.u32(f::STATE);
    x.set32(Boss::SAVED_STATE, state);
    x.set32(f::STATE, INTERRUPTED);
}

/// 0x9ab16: walk toward a player (shot 0x31).
pub(super) fn chase<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Enemy::VULNERABLE, 1);
    x.set8(Boss::SHOT, 0x31);
    x.set32(f::STATE, CHASE);
}

/// 0x9ab2c: walk toward a player (shot 0x32).
pub(super) fn chase_b<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Enemy::VULNERABLE, 1);
    x.set8(Boss::SHOT, 0x32);
    x.set32(f::STATE, CHASE_B);
}

/// 0x9ab7c : charge.
pub(super) fn dash<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Enemy::VULNERABLE, 0);
    x.set32(f::STATE, DASH);
}

/// 0x9ab8a: attack 0 (vulnerable).
pub(super) fn attack_0<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Enemy::VULNERABLE, 1);
    x.set32(f::STATE, ATTACK_0);
}

fn attack<R: Bench>(x: &mut Cx<'_, R>, state: u32) {
    x.set8(Enemy::VULNERABLE, 0);
    x.set32(f::STATE, state);
}

/// 0x9ab9a : attaque 2.
pub(super) fn attack_2<R: Bench>(x: &mut Cx<'_, R>) {
    attack(x, ATTACK_2);
}

/// 0x9aba8: attack 4, invulnerable.
pub(super) fn attack_4_shielded<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Boss::INVULNERABLE, 1);
    attack_4(x);
}

/// 0x9abae : attaque 4.
pub(super) fn attack_4<R: Bench>(x: &mut Cx<'_, R>) {
    attack(x, ATTACK_4);
}

/// 0x9abbc : attaque 6.
pub(super) fn attack_6<R: Bench>(x: &mut Cx<'_, R>) {
    attack(x, ATTACK_6);
}

/// 0x9abd4: attack 10 as long as some remain.
pub(super) fn attack_10<R: Bench>(x: &mut Cx<'_, R>) {
    if (x.u8(Boss21::BUDGET_A) as i8) <= 0 {
        return attack_6(x);
    }
    attack(x, ATTACK_10);
}

/// 0x9abe8: attack 12 as long as some remain.
pub(super) fn attack_12<R: Bench>(x: &mut Cx<'_, R>) {
    if (x.u8(Boss21::BUDGET_B) as i8) <= 0 {
        return attack_6(x);
    }
    attack(x, ATTACK_12);
}

/// 0x9ac18: attack 16 (end of the slow motion).
pub(super) fn attack_16<R: Bench>(x: &mut Cx<'_, R>) {
    if (x.u8(Boss21::SLOW) as i8) > 0 {
        return chase(x);
    }
    x.set8(Boss21::STRIKES, 0);
    attack(x, ATTACK_16);
}

/// 0x9ab50: the charge if the boss is in the useful band of the screen, otherwise attack 4 when it faces the right
/// way.
pub(super) fn dash_or_attack<R: Bench>(x: &mut Cx<'_, R>) {
    let here = x.u16(Obj::X);
    let mut edge = var::CAMERA1_X.get(x.b).wrapping_add(0x38);
    let mut side = 0x20;
    if !Fl::sub16(edge, here).ge() {
        side = 0;
        edge = edge.wrapping_add(0x110);
        if !Fl::sub16(edge, here).le() {
            return dash(x);
        }
    }
    if x.u8(f::FLIP) == side {
        attack_4(x);
    } else {
        dash(x);
    }
}

// --- starting states ----------------------------------------------------------------------------

/// 0x9aac4: arrival on stage.
pub(super) fn start_intro<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Boss::ANIM_KIND, 0x30);
    x.set32(f::STATE, 0x0200_0000);
}

/// 0x9aad4: death of the host, start of the exit from the stage.
pub(super) fn start_death<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Boss::ANIM_KIND, 0x31);
    x.set32(f::IMMUNITY, 0xffff_ffff);
    var::NO_VICTORY.set(x.b, 1);
    x.set32(f::STATE, 0x0200_0200);
}

/// 0x9ae76: the host (`+0x46`) is dead (state 0x0406).
pub(super) fn host_dead<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let host = x.linked(f::HOST);
    host.u16(x.b, f::STATE) == 0x0406
}

/// End of attack: the host's death, then the absence of players, take precedence; otherwise `then`.
pub(super) fn after_attack<R: Bench>(x: &mut Cx<'_, R>, then: fn(&mut Cx<'_, R>)) {
    if host_dead(x) {
        return start_death(x);
    }
    if var::PLAYERS_ALIVE.get(x.b) == 0 {
        return wait(x);
    }
    then(x);
}

// --- tirages ------------------------------------------------------------------------------------

/// 0x9a850: delay before the next step, depending on difficulty.
pub(super) fn walk_delay_fast<R: Bench>(x: &mut Cx<'_, R>) {
    let v = per_rank(x, "boss_21/walk_delays");
    x.set8(Boss::WALK_DELAY, v);
}

/// 0x9a85c: same, table of the slow walks.
pub(super) fn walk_delay_slow<R: Bench>(x: &mut Cx<'_, R>) {
    let v = per_rank(x, "boss_21/walk_delays_slow");
    x.set8(Boss::WALK_DELAY, v);
}

/// 0x9a8a8: number of steps of a walk.
pub(super) fn walk_steps_fast<R: Bench>(x: &mut Cx<'_, R>) {
    let r = (x.random() as u32) & 0x1f;
    let v = byte(x, "boss_21/walk_steps", r);
    x.set8(Boss::WALK_STEPS, v);
}

/// 0x9a8ba.
pub(super) fn walk_steps_slow<R: Bench>(x: &mut Cx<'_, R>) {
    let r = (x.random() as u32) & 0x1f;
    let v = byte(x, "boss_21/walk_steps_slow", r);
    x.set8(Boss::WALK_STEPS, v);
}

/// Index into the difficulty table: `(difficulty & 0x1e) × k + (players - 1)`.
fn level_index<R: Bench>(x: &mut Cx<'_, R>, k: u32) -> u32 {
    let level = (var::RANK.get(x.b) & 0x1e) as u32 * k;
    let players = var::PLAYERS_ALIVE.get(x.b) as u32;
    level + players.saturating_sub(1)
}

/// 0x9ac3c: number of hits before attack 16, depending on difficulty and number of players.
pub(super) fn pick_strike_limit<R: Bench>(x: &mut Cx<'_, R>) {
    let i = level_index(x, 2);
    let v = byte(x, "boss_21/strike_limits", i);
    x.set8(Boss21::STRIKE_LIMIT, v);
}

/// 0x9ac9a: duration of the slow motion (word).
pub(super) fn pick_slow_time<R: Bench>(x: &mut Cx<'_, R>) {
    let level = (var::RANK.get(x.b) & 0x1e) as u32 * 4;
    let players = var::PLAYERS_ALIVE.get(x.b) as u32;
    let i = level + 2 * players.saturating_sub(1);
    let v = word(x, "boss_21/slow_times", i);
    x.set16(Boss21::SLOW_TIME, v);
}

/// 0x9ad3c: slow-motion mode (1 or 2), drawn from a bit mask by difficulty and number of players.
pub(super) fn pick_slow_mode<R: Bench>(x: &mut Cx<'_, R>) {
    let players = var::PLAYERS_ALIVE.get(x.b) as u32;
    let row = players.saturating_sub(1) << 3;
    let level = ((var::RANK.get(x.b) & 0x18) >> 2) as u32;
    let bit = (x.random() as u32) & 0x1f & 7;
    let set = byte(x, "boss_21/slow_modes", row + level) >> bit & 1 != 0;
    x.set8(Boss21::SLOW, if set { 2 } else { 1 });
}

/// 0x9ad94: drops objects around the camera (0x57c8c); `+0xaf` counts those that could be born.
pub(super) fn spawn_by_level<R: Bench>(x: &mut Cx<'_, R>) {
    let i = level_index(x, 2);
    let mut d5 = byte(x, "boss_21/spawn_counts", i) as i8 as i16 as u16;
    loop {
        let i = (d5 as u32).wrapping_mul(4) & 0xffff;
        let dx = word(x, "boss_21/spawn_spots", i);
        let dy = word(x, "boss_21/spawn_spots", i + 2);
        let px = dx.wrapping_add(var::CAMERA1_X.get(x.b));
        let py = dy.wrapping_add(var::CAMERA1_Y.get(x.b));
        // (D0, D1, D2) = (x, 0, y); D2 (low word) remains
        *x.c.d2() = *x.c.d2() & 0xffff_0000 | py as u32;
        if crate::families::creators::enemy_1_at_pure(x.b, x.o, (px, 0, py)).is_some() {
            x.add8(Boss21::SPAWNED, 1);
        }
        if d5 == 0 {
            break;
        }
        d5 = d5.wrapping_sub(1);
    }
    // `dbf D5`: D5.w = -1 on exit (leftover; the high word is the caller's)
    *x.c.d5() = *x.c.d5() & 0xffff_0000 | 0xffff;
}

// --- attack choice from the range boxes ------------------------------------------------

/// 0x9a90c, 0x9a928: is a player in the box `(d0, d1, d2)`? Returns 0 if not, or 1 / 2: the side (0x1047a).
pub(super) fn probe<R: Bench>(x: &mut Cx<'_, R>, d0: i16, d1: u16, d2: u16) -> u8 {
    if !lib::target_in_box(x, d0, d1, d2) {
        return 0;
    }
    lib::turn_decision(x) as u8
}

/// Runs the state routine whose address the table gives.
fn go<R: Bench>(x: &mut Cx<'_, R>, routine: u32) {
    match routine {
        0x9ab50 => dash_or_attack(x),
        0x9ab7c => dash(x),
        0x9ab8a => attack_0(x),
        0x9ab9a => attack_2(x),
        0x9abae => attack_4(x),
        0x9abbc => attack_6(x),
        0x9abd4 => attack_10(x),
        0x9abe8 => attack_12(x),
        t => unreachable!("boss 21: state routine {t:x}"),
    }
}

/// 0x9a944: a player is ahead, at short range: the charge.
pub(super) fn choose_close<R: Bench>(x: &mut Cx<'_, R>) {
    let r = (x.random() as u32) & 0xf;
    let routine = decide(x, "boss_21/close_picks", r, "boss_21/close_jumps");
    go(x, routine);
}

/// 0x9a96c: a player is ahead; the phase sets the choice.
pub(super) fn choose_front<R: Bench>(x: &mut Cx<'_, R>) {
    let r = (x.random() as u32) & 0xf;
    let phase = x.u8(Boss::PHASE) as i8 as i32;
    let picks = match jump_target(x, "boss_21/front_phases", (2 * phase) as u32) {
        0x9a990 | 0x9a996 => "boss_21/front_picks_0",
        0x9a99c | 0x9a9a2 | 0x9a9a8 => "boss_21/front_picks_1",
        t => unreachable!("0x9a96c: target {t:x}"),
    };
    let routine = decide(x, picks, r, "boss_21/front_jumps");
    go(x, routine);
}

/// 0x9a9d8: a player is at mid-distance (both sides read the same tables).
pub(super) fn choose_mid<R: Bench>(x: &mut Cx<'_, R>) {
    let r = (x.random() as u32) & 0xf;
    let routine = decide(x, "boss_21/mid_picks", r, "boss_21/mid_jumps");
    go(x, routine);
}

/// 0x9aa20: a player is far; `side` = 1 or 2.
pub(super) fn choose_far<R: Bench>(x: &mut Cx<'_, R>, side: u8) {
    let r = (x.random() as u32) & 0xf;
    let phase = x.u8(Boss::PHASE) as i8 as i32;
    let entry = (side as i32 - 1) * 2 + 4 * phase;
    let routine = jump_target(x, "boss_21/far_phases", entry as u32);
    // ten routines: the first four read the first pick table, the others the second
    let picks = if (0x9aa58..0x9aa70).contains(&routine) { "boss_21/far_picks_0" } else { "boss_21/far_picks_1" };
    let routine = decide(x, picks, r, "boss_21/far_jumps");
    go(x, routine);
}
