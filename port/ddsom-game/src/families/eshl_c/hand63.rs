use super::*;

const ANIMS: u32 = 0x175da6;
const LINK: u32 = 0x5c;
const DELAY: u32 = 0x42;
/// Pointer table (per number of players) to 32 delays (words).
const DELAY_TABLES: u32 = 0xa6b0e;
/// Pointer table (per number of players) to 32-bit masks indexed by `FFFF92`.
const GATE_TABLES: u32 = 0xa6c54;
const PLAYER_COUNT: u32 = 0x35;
const GATE_INDEX: u32 = 0x7f92;
/// Phase of the boss.
const BOSS_PHASE: u32 = 0xac;
/// Distance in front of the boss and approach speed.
const REACH: u16 = 0x30;
const SPEED: u16 = 2;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => {
            x.add8(0x4, 2);
            x.set16(0xc, 0x30);
            let boss = x.linked(LINK);
            let (bx, by) = (boss.u16(x.b, 0x8), boss.u16(x.b, 0x10));
            x.set16(0x8, bx);
            x.set16(0x10, by);
            resting_or_attacking(x);
        }
        2 => resting_or_attacking(x),
        4 => striking(x),
        _ => x.free_eshl(),
    }
}

/// Phase 1 / 2 (`+4 = 2`), then put back in the sorted list.
fn resting_or_attacking<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(0x5) == 0 {
        match x.u8(0x6) {
            0 => {
                x.add8(0x6, 4);
                pick_delay(x);
                x.anim_start(ANIMS, 3);
                rest(x);
            }
            4 => rest(x),
            8 => finish_attack(x),
            _ => {}
        }
    } else {
        match x.u8(0x6) {
            0 => {
                x.add8(0x6, 4);
                x.anim_start(ANIMS, 5);
                attack(x);
            }
            4 => attack(x),
            8 => back_to_rest(x),
            _ => {}
        }
    }
    x.reinsert();
}

/// Delay before the next draw of lots, according to the number of players.
fn pick_delay<R: Bench>(x: &mut Cx<'_, R>) {
    let players = x.a5_u8(PLAYER_COUNT) as u32;
    let table = x.data32(DELAY_TABLES + 4 * players);
    let r = random(x) & 0x1f;
    let d = x.data16(table + 2 * r);
    x.set16(DELAY, d);
}

/// Draw: is the bit drawn in the mask of the current configuration set?
fn gate<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let players = x.a5_u8(PLAYER_COUNT) as u32;
    let table = x.data32(GATE_TABLES + 4 * players);
    let index = x.a5_u16(GATE_INDEX).wrapping_shl(2) as u32;
    let mask = x.data32(table + index);
    let r = random(x) & 0x1f;
    mask >> r & 1 != 0
}

fn lost_boss<R: Bench>(x: &mut Cx<'_, R>) {
    x.set16(0x4, 0x400);
}

/// Rest: countdown, draw, then attack if the target is on the right side.
fn rest<R: Bench>(x: &mut Cx<'_, R>) {
    let boss = x.linked(LINK);
    if x.a5_u8(GAME_PAUSED) == 0 {
        let t = x.u16(DELAY).wrapping_sub(1);
        x.set16(DELAY, t);
        if s16(t) < 0 {
            pick_delay(x);
            if gate(x) && boss.u16(x.b, f::OWNER) != 0 {
                let target = obj_of(boss.u16(x.b, f::OWNER));
                let d = s16(boss.u16(x.b, 0x8).wrapping_sub(target.u16(x.b, 0x8)));
                let facing = x.u8(f::FLIP) != 0;
                if (d >= 0) != facing {
                    x.set16(0x6, 0x800);
                    x.anim_start(ANIMS, 5);
                    return;
                }
            }
        }
    }
    if boss.u8(x.b, 0) == 0 {
        return lost_boss(x);
    }
    match s8(boss.u8(x.b, BOSS_PHASE)) {
        1 => {
            // the boss attacks: goes to the attack phase
            x.add8(0x5, 4);
            x.set16(0x6, 0);
        }
        2.. => lost_boss(x),
        _ => {
            follow(x);
            x.anim_step();
        }
    }
}

/// End of the rest phase by `+6 = 8`: waits for the end of the animation then creates a shot 53.
fn finish_attack<R: Bench>(x: &mut Cx<'_, R>) {
    let boss = x.linked(LINK);
    if boss.u16(x.b, f::OWNER) == 0 {
        return x.set16(0x6, 0);
    }
    if boss.u8(x.b, 0) == 0 {
        return lost_boss(x);
    }
    match s8(boss.u8(x.b, BOSS_PHASE)) {
        1 => {
            x.add8(0x5, 4);
            x.set16(0x6, 0);
            return;
        }
        2.. => return lost_boss(x),
        _ => {}
    }
    if x.anim_step() {
        if let Some(shot) = x.alloc(&pools::ESHL) {
            shot.set_u8(x.b, 0, 1);
            shot.set_u8(x.b, 2, 0x35);
            shot.set_u16(x.b, LINK, x.o.0 as u16);
        }
        x.set16(0x6, 0);
    }
}

/// Attack in progress (the boss is in phase 1): follows the boss, plays the animation, then ends.
fn attack<R: Bench>(x: &mut Cx<'_, R>) {
    follow(x);
    if !x.anim_step() {
        return;
    }
    x.set32(0x34, 0);
    x.add8(0x6, 4);
    back_to_rest(x);
}

/// Has the boss finished its attack? Rest if the phase has dropped below 1.
fn back_to_rest<R: Bench>(x: &mut Cx<'_, R>) {
    let boss = x.linked(LINK);
    if boss.u8(x.b, 0) == 0 {
        return x.set8(0x4, 6);
    }
    match s8(boss.u8(x.b, BOSS_PHASE)) {
        1 => {}
        2.. => x.set8(0x4, 6),
        _ => {
            x.set8(0x5, 0);
            x.set16(0x6, 0);
        }
    }
}

/// State 4: animation 5 until the end, then disappearance.
fn striking<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x5) {
        0 => {
            x.add8(0x5, 4);
            x.anim_start(ANIMS, 5);
            strike_step(x);
        }
        4 => strike_step(x),
        _ => {}
    }
    x.reinsert();
}

fn strike_step<R: Bench>(x: &mut Cx<'_, R>) {
    follow(x);
    if x.anim_step() {
        x.set16(0x4, 0x600);
    }
}

/// Moves the hand closer to its target position (in front of the boss, on the side it faces), by 2 pixels per frame
/// in X and Y. Boss gone: state 4 (and return to the caller which continues).
fn follow<R: Bench>(x: &mut Cx<'_, R>) {
    let boss = x.linked(LINK);
    if boss.u8(x.b, 0) == 0 {
        return lost_boss(x);
    }
    let reach = if x.u8(f::FLIP) != 0 { REACH } else { neg(REACH) };
    let goal_x = reach.wrapping_add(boss.u16(x.b, 0x8));
    let goal_y = boss.u16(x.b, 0x10);
    let px = approach(goal_x, x.u16(0x8));
    x.set16(0x8, px);
    let py = approach(goal_y, x.u16(0x10));
    x.set16(0x10, py);
}

/// A step of 2 toward `goal` without overshooting it.
fn approach(goal: u16, pos: u16) -> u16 {
    if s16(goal) == s16(pos) {
        pos
    } else if s16(goal) < s16(pos) {
        let p = pos.wrapping_sub(SPEED);
        if s16(goal) <= s16(p) { p } else { goal }
    } else {
        let p = pos.wrapping_add(SPEED);
        if s16(goal) >= s16(p) { p } else { goal }
    }
}
