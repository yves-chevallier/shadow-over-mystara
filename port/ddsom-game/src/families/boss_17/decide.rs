//! Boss 17: choice of the next action (state changes) and draws in the ROM tables.
//!
//! The states are set in one go as longs (`+4..+7`): `0x0202_0400` = active, group 2 (combat), step 4.
//! The tables are read from the ROM (opcode space: PC-relative addressing).

use crate::families::cx::Bench;
use super::*;
use crate::objects::Obj;

/// Combat: waiting for players (group 10), walk toward a player, charge...
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
const ATTACK_14: u32 = 0x0204_0e00;
const ATTACK_16: u32 = 0x0204_1000;

/// 0x9735a: the boss waits (no player alive).
pub(super) fn wait<R: Bench>(x: &mut Cx<'_, R>) {
    x.set32(f::STATE, WAIT);
}

/// 0x971f4: standing, motionless and vulnerable.
pub(super) fn stand<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(VULNERABLE, 1);
    x.set32(f::STATE, STAND);
}

/// 0x97204: interrupts the action in progress (the state is saved in `+0xa2`).
pub(super) fn interrupt<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(VULNERABLE, 0);
    let state = x.u32(f::STATE);
    x.set32(SAVED_STATE, state);
    x.set32(f::STATE, INTERRUPTED);
}

/// 0x97218: walk toward a player (shot 0x31).
pub(super) fn chase<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(VULNERABLE, 1);
    x.set8(SHOT, 0x31);
    x.set32(f::STATE, CHASE);
}

/// 0x9722e: walk toward a player (shot 0x32).
pub(super) fn chase_b<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(VULNERABLE, 1);
    x.set8(SHOT, 0x32);
    x.set32(f::STATE, CHASE_B);
}

/// 0x97288 : charge.
pub(super) fn dash<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(VULNERABLE, 0);
    x.set32(f::STATE, DASH);
}

/// 0x97296: attack 0 (vulnerable).
pub(super) fn attack_0<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(VULNERABLE, 1);
    x.set32(f::STATE, ATTACK_0);
}

/// 0x972a6 : attaque 2.
pub(super) fn attack_2<R: Bench>(x: &mut Cx<'_, R>) {
    attack(x, ATTACK_2);
}

/// 0x972b4: attack 4, invulnerable.
pub(super) fn attack_4_shielded<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(INVULNERABLE, 1);
    attack_4(x);
}

/// 0x972ba : attaque 4.
pub(super) fn attack_4<R: Bench>(x: &mut Cx<'_, R>) {
    attack(x, ATTACK_4);
}

/// 0x972c8 : attaque 6.
pub(super) fn attack_6<R: Bench>(x: &mut Cx<'_, R>) {
    attack(x, ATTACK_6);
}

fn attack<R: Bench>(x: &mut Cx<'_, R>, state: u32) {
    x.set8(VULNERABLE, 0);
    x.set32(f::STATE, state);
}

/// Is the boss within the camera's horizontal window widened from `-0x10` to `0x190` (0x104a2)?
/// D0 = edges, D1 = width; returns D0 (0: inside, 1: outside).
fn off_screen<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    lib::outside_band(x, -0x10, 0x190)
}

/// 0x972e0: attack 10 if the boss is on screen and has some left.
pub(super) fn attack_10<R: Bench>(x: &mut Cx<'_, R>) {
    if off_screen(x) || (x.u8(BUDGET_A) as i8) <= 0 {
        return attack_6(x);
    }
    attack(x, ATTACK_10);
}

/// 0x97302 : attaque 12.
pub(super) fn attack_12<R: Bench>(x: &mut Cx<'_, R>) {
    if off_screen(x) || (x.u8(BUDGET_B) as i8) <= 0 {
        return attack_6(x);
    }
    attack(x, ATTACK_12);
}

/// 0x97324: attack 14 (dropping objects), as long as there are some left to drop.
pub(super) fn attack_14<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(BUDGET_C) == 0 {
        return attack_6(x);
    }
    if (x.u8(SPAWNED) as i8) > 0 {
        return chase(x);
    }
    attack(x, ATTACK_14);
}

/// 0x97340: attack 16 (end of the slow motion).
pub(super) fn attack_16<R: Bench>(x: &mut Cx<'_, R>) {
    if (x.u8(SLOW) as i8) > 0 {
        return chase(x);
    }
    x.set8(STRIKES, 0);
    attack(x, ATTACK_16);
}

/// 0x97252: the charge if the boss is in the useful band of the screen, otherwise attack 4 when it faces the right
/// way.
pub(super) fn dash_or_attack<R: Bench>(x: &mut Cx<'_, R>) {
    let here = x.u16(0x8);
    let mut edge = camera_x(x).wrapping_add(0x38);
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

/// 0x97162: variant 0, arrival on stage.
pub(super) fn start_intro<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(ANIM_KIND, 0x30);
    x.set32(f::STATE, 0x0200_0000);
}

/// 0x97172: exit from the arrival cutscene.
pub(super) fn start_appear<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(ANIM_KIND, 0x31);
    x.set32(f::STATE, 0x0200_0200);
}

/// 0x97182: death, start of the ending cutscene.
pub(super) fn start_death<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(ANIM_KIND, 0x32);
    x.set8(SLOW, 0);
    x.set8(0, 2);
    x.set32(f::IMMUNITY, 0xffff_ffff);
    x.set_a5_u8(0x42, 1);
    x.set_a5_u8(0x50, 0);
    let kind = x.u8(0x2);
    set_abs8(x, 0xff2a84, kind);
    set_abs8(x, 0xff2a85, 1);
    let bar = x.u8(0x7e);
    set_abs8(x, 0xff2a86, bar);
    if x.u16(0x54) != 0 {
        let killer = x.linked(0x54);
        killer.set_u8(x.b, 0x18a, 1);
    }
    x.set32(f::STATE, 0x0200_0400);
}

/// 0x971ce : variante 1.
pub(super) fn start_variant1<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(ANIM_KIND, 0x33);
    x.set_a5_u8(0x66, 1);
    x.set32(f::STATE, 0x0200_0600);
}

/// 0x971e4 : variante 2.
pub(super) fn start_variant2<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(ANIM_KIND, 0x34);
    x.set32(f::STATE, 0x0200_0800);
}

// --- tirages ------------------------------------------------------------------------------------

/// Table byte read from the ROM (`(table,PC,Dn)` addressing).
fn table8<R: Bench>(x: &mut Cx<'_, R>, table: u32, index: u32) -> u8 {
    x.rom8(table + index)
}

fn table16<R: Bench>(x: &mut Cx<'_, R>, table: u32, index: u32) -> u16 {
    x.rom16(table + index)
}

/// 0x96ef0: delay before the next step, depending on difficulty.
pub(super) fn walk_delay_fast<R: Bench>(x: &mut Cx<'_, R>) {
    let level = var::RANK.get(x.b) as u32;
    let v = table8(x, 0x96f08, level);
    x.set8(WALK_DELAY, v);
}

/// 0x96efc: same, table of the slow walks.
pub(super) fn walk_delay_slow<R: Bench>(x: &mut Cx<'_, R>) {
    let level = var::RANK.get(x.b) as u32;
    let v = table8(x, 0x96f28, level);
    x.set8(WALK_DELAY, v);
}

/// 0x96f48: number of steps of a walk.
pub(super) fn walk_steps_fast<R: Bench>(x: &mut Cx<'_, R>) {
    let r = random(x) & 0x1f;
    let v = table8(x, 0x96f6c, r);
    x.set8(WALK_STEPS, v);
}

/// 0x96f5a.
pub(super) fn walk_steps_slow<R: Bench>(x: &mut Cx<'_, R>) {
    let r = random(x) & 0x1f;
    let v = table8(x, 0x96f8c, r);
    x.set8(WALK_STEPS, v);
}

/// Index into the difficulty table: `(difficulty & 0x1e) × k + (players - 1)`.
fn level_index<R: Bench>(x: &mut Cx<'_, R>, k: u32) -> u32 {
    let level = (var::RANK.get(x.b) & 0x1e) as u32 * k;
    let players = var::PLAYERS_ALIVE.get(x.b) as u32;
    level + players.saturating_sub(1)
}

/// 0x97364: number of hits before the charge, depending on difficulty and number of players.
pub(super) fn pick_strike_limit<R: Bench>(x: &mut Cx<'_, R>) {
    let i = level_index(x, 2);
    let v = table8(x, 0x97382, i);
    x.set8(STRIKE_LIMIT, v);
}

/// 0x973c2: duration of the slow motion (word).
pub(super) fn pick_slow_time<R: Bench>(x: &mut Cx<'_, R>) {
    let level = (var::RANK.get(x.b) & 0x1e) as u32 * 4;
    let players = var::PLAYERS_ALIVE.get(x.b) as u32;
    let i = level + 2 * players.saturating_sub(1);
    let v = table16(x, 0x973e4, i);
    x.set16(SLOW_TIME, v);
}

/// 0x97464: slow-motion mode (1 or 2), drawn from a bit mask by difficulty and number of players.
pub(super) fn pick_slow_mode<R: Bench>(x: &mut Cx<'_, R>) {
    let players = var::PLAYERS_ALIVE.get(x.b) as u32;
    let row = players.saturating_sub(1) << 3;
    let level = ((var::RANK.get(x.b) & 0x18) >> 2) as u32;
    let i = row + level;
    let bit = random(x) & 0x1f & 7;
    let (table, set_means_one) = if x.u8(Obj::VARIANT) == 0 { (0x974aa, true) } else { (0x974ca, false) };
    let set = table8(x, table, i) >> bit & 1 != 0;
    x.set8(SLOW, if set == set_means_one { 1 } else { 2 });
}

/// 0x974ea: drops a number of objects read from table 0x97530 (difficulty, players).
pub(super) fn spawn_by_level<R: Bench>(x: &mut Cx<'_, R>) {
    let i = level_index(x, 2);
    let count = table8(x, 0x97530, i) as i8 as i16;
    spawn_ring(x, count);
}

/// 0x97506: drops `count + 1` objects around the camera (0x57c8c); `+0xaf` counts those that could be born.
pub(super) fn spawn_ring<R: Bench>(x: &mut Cx<'_, R>, count: i16) {
    let mut d5 = count as u16;
    loop {
        let i = (d5 as u32).wrapping_mul(4) & 0xffff;
        let dx = table16(x, 0x97570, i);
        let dy = table16(x, 0x97572, i);
        let px = dx.wrapping_add(var::CAMERA1_X.get(x.b));
        let py = dy.wrapping_add(var::CAMERA1_Y.get(x.b));
        // (D0, D1, D2) = (x, 0, y); D2 (low word) remains
        *x.c.d2() = *x.c.d2() & 0xffff_0000 | py as u32;
        if crate::families::creators::enemy_1_at_pure(x.b, x.o, (px, 0, py)).is_some() {
            x.add8(SPAWNED, 1);
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

/// 0x96fac, 0x96fc8: is a player in the box `(d0, d1, d2)`? Returns 0 if not, or 1 / 2: the side (0x1047a).
pub(super) fn probe<R: Bench>(x: &mut Cx<'_, R>, d0: i16, d1: u16, d2: u16) -> u8 {
    if !lib::target_in_box(x, d0, d1, d2) {
        return 0;
    }
    lib::turn_decision(x) as u8
}

/// 0x96fe4: a player is very close ahead; table 0x96ffa chooses the entry of 0x9700a.
pub(super) fn choose_close<R: Bench>(x: &mut Cx<'_, R>) {
    let r = random(x) & 0xf;
    let pick = table8(x, 0x96ffa, r);
    let target = 0x9700a_i32 + table16(x, 0x9700a, pick as u32) as i16 as i32;
    match target as u32 {
        0x97288 => dash(x),
        t => unreachable!("0x96fe4: target {t:x}"),
    }
}

/// 0x9700c: a player is ahead: depending on the combat phase, a charge or an attack.
pub(super) fn choose_front<R: Bench>(x: &mut Cx<'_, R>) {
    let r = random(x) & 0xf;
    let phase = x.u8(PHASE) as i8 as i32;
    let entry = x.rom16(0x97026 + (2 * phase) as u32) as i16 as i32;
    let kind = 0x97026 + entry;
    let pick = match kind as u32 {
        0x97030 | 0x97036 => table8(x, 0x97054, r),
        0x9703c | 0x97042 | 0x97048 => table8(x, 0x97064, r),
        t => unreachable!("0x9700c: target {t:x}"),
    };
    let target = 0x97074_i32 + table16(x, 0x97074, pick as u32) as i16 as i32;
    match target as u32 {
        0x97252 => dash_or_attack(x),
        0x972ba => attack_4(x),
        t => unreachable!("0x9700c: target {t:x}"),
    }
}

/// 0x97078: a player is at mid-distance; `side` = 1 or 2 (0x1047a).
pub(super) fn choose_mid<R: Bench>(x: &mut Cx<'_, R>) {
    let r = random(x) & 0xf;
    let pick = table8(x, 0x970aa, r);
    let target = 0x970ba_i32 + table16(x, 0x970ba, pick as u32) as i16 as i32;
    match target as u32 {
        0x97296 => attack_0(x),
        0x972a6 => attack_2(x),
        0x972ba => attack_4(x),
        t => unreachable!("0x97078: target {t:x}"),
    }
}

/// 0x970c0: a player is far; `side` = 1 or 2.
pub(super) fn choose_far<R: Bench>(x: &mut Cx<'_, R>, side: u8) {
    let r = random(x) & 0xf;
    let entry = (side as u32 - 1) * 2 + 4 * (x.u8(PHASE) as i8 as i32 as u32);
    let which = x.rom16(0x970e4 + entry) as i16 as i32;
    let routine = 0x970e4 + which;
    // ten routines: the first four read 0x9713a, the others 0x9714a
    let table = if (0x970f8..0x97110).contains(&routine) { 0x9713a } else { 0x9714a };
    let pick = table8(x, table, r);
    let target = 0x9715a_i32 + table16(x, 0x9715a, pick as u32) as i16 as i32;
    match target as u32 {
        0x97296 => attack_0(x),
        0x972c8 => attack_6(x),
        0x972e0 => attack_10(x),
        0x97302 => attack_12(x),
        t => unreachable!("0x970c0: target {t:x}"),
    }
}
