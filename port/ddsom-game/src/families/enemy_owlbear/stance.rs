//! Type 4, arrival (`+5 = 0`), standing beast (`+5 = 2`) and rest (`+5 = 10`).

use crate::families::cx::Bench;
use super::*;

/// ROM tables: attacks to choose per difficulty level (`A5+0x7f92`), 32 bytes per level.
const CLOSE_ATTACKS: u32 = 0x5eec6;
const FAR_ATTACKS: u32 = 0x5f0c6;
const NEXT_MOVES: u32 = 0x5f3d8;
/// Draw of the chance to attack, one long per difficulty level.
const ATTACK_CHANCE: u32 = 0x5f2ea;

// --- substate 0: arrival ----------------------------------------------------------------------

pub(super) fn arrival<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB2) != 0 {
        return;
    }
    let left = x.u8(DELAY).wrapping_sub(1);
    x.set8(DELAY, left);
    if left == 0 && attack_chance(x) {
        return set_state_long(x, ACTIVE_STANCE);
    }
    match x.u8(0x7) {
        0 => walk_begin(x),
        2 => {
            if !pick_target(x) {
                return;
            }
            let t = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, t);
            if t == 0 {
                x.add8(0x7, 2);
                x.set8(VULNERABLE, 1);
                x.set8(WANT_ANIM, 0);
                x.set8(CUR_ANIM, 0);
                return body(x, 1);
            }
            if lib::target_in_box(x, 0, 0x80, 0x20) {
                set_state_long(x, ACTIVE_STANCE);
                lib::draw_6b(x);
                return lib::draw_stance(x);
            }
            step(x);
            move_xy(x);
        }
        4 => turn_end(x),
        _ => {}
    }
}

/// 0x5daa6 : marche vers l'avant (animation 2, vitesse 0xb) pendant `0x24` images.
pub(super) fn walk_begin<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x7, 2);
    x.set8(TIMER, 0x24);
    x.set8(VULNERABLE, 1);
    set_angle(x, 8, 0x18);
    pose(x, 2, SET_BODY, 2);
    speed(x, 0xb);
}

/// 0x5db66: end of an arrival turn-around.
pub(super) fn turn_end<R: Bench>(x: &mut Cx<'_, R>) {
    if x.anim_step() {
        x.set8(0x7, 0);
        let flip = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
        x.set8(VULNERABLE, 1);
        x.set8(TIMER, 0);
        body(x, 3);
    }
}

/// 0x5f2c6: does the beast feel like attacking? (one bit drawn from a mask per difficulty level).
fn attack_chance<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let level = (var::RANK.get(x.b).wrapping_mul(4)) as u32;
    let mask = rom32(x, ATTACK_CHANCE + level);
    let bit = random(x) & 0x1f;
    mask >> bit & 1 != 0
}

// --- substate 2: standing -----------------------------------------------------------------------

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if grabbed(x) {
        return set_state_long(x, KNOCKED);
    }
    if var::PLAYERS_ALIVE.get(x.b) == 0 {
        return set_state_long(x, ACTIVE_IDLE);
    }
    match x.u8(f::SUB2) {
        0 | 8 => pursue(x),
        2 => turn(x),
        4 => wait(x),
        6 => approach(x),
        _ => {}
    }
}

/// +6 = 0 and 8 (0x5dc2c): walks toward the target, watches, then attacks or changes side.
fn pursue<R: Bench>(x: &mut Cx<'_, R>) {
    if target_far(x) {
        return set_state_long(x, 0x0202_0600);
    }
    if x.u8(0x7) == 0 {
        x.add8(0x7, 2);
        x.set8(VULNERABLE, 1);
        pose(x, 2, SET_BODY, 2);
        speed(x, 0xb);
        return lib::draw_stance(x);
    }
    let t = x.u8(AI_TURN).wrapping_sub(1);
    x.set8(AI_TURN, t);
    if (t as i8) < 0 {
        lib::draw_stance(x);
        pick_target(x);
        let shot = if x.u8(f::SUB2) == 0 { 0 } else { 1 };
        x.set8(AI_SHOT, shot);
        lib::approach_target(x);
        let n = x.u8(AI_STEP).wrapping_sub(1);
        x.set8(AI_STEP, n);
        if n == 0 {
            lib::draw_6b(x);
            return pursue_tail(x);
        }
    }
    x.set8(REPEAT_LEFT, 1);
    engage(x);
}

/// 0x5dcc8: a test on the target can freeze the beast for a moment; otherwise it engages.
pub(super) fn pursue_tail<R: Bench>(x: &mut Cx<'_, R>) {
    if lib::maybe_draw_6e(x) {
        return x.set16(f::SUB2, 0x0400);
    }
    x.set8(REPEAT_LEFT, 1);
    engage(x);
}

/// 0x5dce0: is the target in range? Strikes, grabs, or keeps advancing.
pub(super) fn engage<R: Bench>(x: &mut Cx<'_, R>) {
    if lib::target_in_box(x, -0x28, 0x28, 0x10) {
        x.set16(f::STATE, 0x0204);
        return choose_attack(x, CLOSE_ATTACKS);
    }
    if reach(x, -0x38, 0x38, 0x10, 0xf, CLOSE_ATTACKS) || reach(x, -0xb0, 0x40, 0x10, 0x2, FAR_ATTACKS) {
        return;
    }
    if lib::guard_in_front_strict(x, -0x38, 0x38, 0x20) {
        return lib::pick_byte(x);
    }
    step(x);
    move_xy(x);
    let Some(dx) = target_dx(x) else { return };
    // the offset (beast - target) is negative when the target is on the right: the beast must look right
    let want = if dx > 0 { 0x20 } else { 0 };
    if want != x.u8(f::FLIP) {
        x.set32(g::RESUME_STATE, ACTIVE_STANCE);
        x.set8(RESUME_STEP, 0);
        set_state_long(x, ACTIVE_TURN);
    }
}

/// A player is right in front (0x5dd7a or 0x5ddf4): at random an attack from the `attacks` table or a short attack
/// (state `0x02020800`). `mask`: bits of the draw that choose the short attack (`0xf`: up to 10 out of 16, `0x2`: one
/// time in two).
fn reach<R: Bench>(x: &mut Cx<'_, R>, d0: i16, d1: u16, d2: u16, mask: u16, attacks: u32) -> bool {
    if !lib::target_in_box(x, d0, d1, d2) {
        return false;
    }
    let Some(t) = target(x) else { return false };
    let same_side = t.u8(x.b, f::FLIP) == x.u8(f::FLIP);
    if !lib::decision(x, same_side) {
        return false;
    }
    let r = random(x) as u16;
    let short = if mask == 0xf { (0xffc0u16 >> (r & mask)) & 1 != 0 } else { r & mask != 0 };
    if short {
        set_state_long(x, 0x0202_0800);
        return true;
    }
    set_state_long(x, 0x0204_0000);
    lib::pick_byte(x);
    choose_attack(x, attacks);
    true
}

/// 0x5ee94 and 0x5ee9a: draws the attack (`+6`) from the difficulty-level table.
pub(super) fn choose_attack<R: Bench>(x: &mut Cx<'_, R>, table: u32) {
    let level = (var::RANK.get(x.b) >> 1 << 5) as u32;
    let r = random(x) & 0x1f;
    let pick = x.data8(table + level + r);
    x.set8(f::SUB2, pick);
    x.set16(f::STATE, 0x0204);
    x.set8(0x7, 0);
}

/// 0x5ee68: the target is far in abscissa.
fn target_far<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let Some(t) = target(x) else { return false };
    let dx = t.u16(x.b, 0x8).wrapping_sub(x.u16(0x8)).wrapping_add(0xa0);
    dx > 0x140
}

/// +6 = 2 (0x5dbd4): turn-around (animation 1 then 3) then resumes the saved state.
fn turn<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(0x7) == 0 {
        x.add8(0x7, 2);
        x.set8(VULNERABLE, 1);
        x.set8(WANT_ANIM, 0);
        x.set8(CUR_ANIM, 0);
        return body(x, 1);
    }
    if x.anim_step() {
        let resume = x.u32(RESUME_STATE);
        x.set32(f::STATE, resume);
        let step = x.u8(RESUME_STEP);
        x.set8(f::SUB2, step);
        let flip = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
        body(x, 3);
    }
}

/// +6 = 4 (0x5de68): standing wait before moving on to the attack.
fn wait<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(0x7) == 0 {
        return ready(x);
    }
    let n = x.u8(AI_REPEAT).wrapping_sub(1);
    x.set8(AI_REPEAT, n);
    if (n as i8) < 0 {
        return set_state_long(x, ACTIVE_STANCE);
    }
    x.set32(RESUME_STATE, ACTIVE_STANCE);
    x.set8(RESUME_STEP, 4);
    turn_if_behind(x);
    step(x);
}

/// +7 = 0 of the attacks (0x5de70): the beast gets ready (rest animation).
pub(super) fn ready<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x7, 2);
    x.set8(VULNERABLE, 1);
    pose(x, 6, SET_BODY, 0);
}

/// +6 = 6 (0x5decc): approaches the target while looking at it.
fn approach<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            x.set8(TIMER, 0x28);
            lib::draw_stance(x);
            x.set8(VULNERABLE, 1);
            pose(x, 4, SET_BODY, 5);
            speed(x, 0x1a);
        }
        2 => {
            let t = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, t);
            if (t as i8) < 0 {
                return set_state_long(x, ACTIVE_STANCE);
            }
            let n = x.u8(AI_TURN).wrapping_sub(1);
            x.set8(AI_TURN, n);
            if (n as i8) < 0 {
                lib::draw_stance(x);
                pick_target(x);
                x.set8(AI_SHOT, 0);
                lib::approach_target(x);
            }
            x.set32(RESUME_STATE, ACTIVE_STANCE);
            let sub2 = x.u8(f::SUB2);
            x.set8(RESUME_STEP, sub2);
            turn_if_behind(x);
            step(x);
            move_xy(x);
        }
        _ => {}
    }
}

/// 0x5f36a: end of an attack; the beast chains a charge if a player is near, otherwise gets back on its feet.
pub(super) fn next_move<R: Bench>(x: &mut Cx<'_, R>) {
    let Some(_) = target(x) else { return set_state_long(x, ACTIVE_STANCE) };
    if !lib::target_in_box(x, -0x38, 0x38, 0x20) && !lib::target_in_box(x, -0xb0, 0x40, 0x20) {
        return set_state_long(x, ACTIVE_STANCE);
    }
    let level = (var::RANK.get(x.b) >> 1 << 5) as u32;
    let r = random(x) & 0x1f;
    let pick = x.data8(NEXT_MOVES + level + r);
    x.set8(f::SUB2, pick);
    x.set16(f::STATE, 0x0208);
    x.set8(0x7, 0);
}

// --- substate 10: rest -----------------------------------------------------------------------

/// Repos : attend qu'un joueur soit en vie (`+6` = 0 seulement).
pub(super) fn rest<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB2) == 0 {
        match x.u8(0x7) {
            0 => walk_begin(x),
            2 => {
                let t = x.u8(TIMER).wrapping_sub(1);
                x.set8(TIMER, t);
                if t != 0 {
                    step(x);
                    move_xy(x);
                } else {
                    x.add8(0x7, 2);
                    x.set8(VULNERABLE, 1);
                    x.set8(WANT_ANIM, 0);
                    x.set8(CUR_ANIM, 0);
                    body(x, 1);
                }
            }
            4 => turn_end(x),
            _ => {}
        }
    }
    if var::PLAYERS_ALIVE.get(x.b) != 0 {
        set_state_long(x, ACTIVE_STANCE);
    }
}
