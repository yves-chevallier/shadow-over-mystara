//! Type 19, the standing beast (substate `+5 = 2`, 0x72b5c): pursuit of a player, turn-arounds and choice of an attack.
//! The steps are `+6`: 0 and 8 pursuit, 2 turn-around, 4 wait, 6 approach.

use crate::families::cx::Bench;
use super::*;
use crate::objects::Current;

/// Choice of an attack from a ROM table: `+4 = 0x0204` (attack) and `+6` drawn from the table of the difficulty level
/// (`A5+0x7f92`).
const PICK_TABLE: &str = "dbeast/attack_lists";
/// Contact attacks (0x74c3e): draw per difficulty level.
const CLOSE_TABLE: &str = "dbeast/close_moves";

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if matches!(x.u8(Obj::VARIANT), 0xc | 0xd) || grabbed(x) {
        return set_state_long(x, KNOCKED);
    }
    // a part whose parent is hit is hit too
    if (x.u8(DisplacerBeast::PART) as i8) >= 0 {
        if let Some(p) = parent(x) {
            if state_of(x, p) == 0x0402_0000 {
                return set_state_long(x, KNOCKED);
            }
        }
    }
    match x.u8(f::SUB2) {
        0 | 8 => pursue(x),
        2 => turn(x),
        4 => wait(x),
        6 => approach(x),
        _ => {}
    }
    // nobody left to fight: the beast rests
    if x.u16(Obj::Z) == 0 && var::PLAYERS_ALIVE.get(x.b) == 0 {
        set_state_long(x, ACTIVE_IDLE);
    }
}

/// +6 = 0 (0x72be0): walks toward the target, watches, then attacks or changes side.
fn pursue<R: Bench>(x: &mut Cx<'_, R>) {
    if target_far(x) {
        return set_state_long(x, 0x0202_0600);
    }
    if x.u8(Enemy::SUB_STEP) == 0 {
        x.add8(Enemy::SUB_STEP, 2);
        x.set8(Enemy::VULNERABLE, 1);
        x.set8(DisplacerBeast::WANT_ANIM, 0xc);
        if x.u8(DisplacerBeast::WANT_ANIM) != x.u8(DisplacerBeast::CUR_ANIM) {
            let want = x.u8(DisplacerBeast::WANT_ANIM);
            x.set8(DisplacerBeast::CUR_ANIM, want);
            play(x, 8);
        }
        speed(x, 0x11);
        lib::draw_stance(x);
        return;
    }
    let t = x.u8(Enemy::AI_STANCE).wrapping_sub(1);
    x.set8(Enemy::AI_STANCE, t);
    if (t as i8) < 0 {
        lib::draw_stance(x);
        pick_target(x);
        let shot = if x.u8(f::SUB2) == 0 { 0xe } else { 0xf };
        x.set8(Enemy::AI_POINT_SET, shot);
        lib::approach_target(x);
        let n = x.u8(Enemy::AI_WANDER).wrapping_sub(1);
        x.set8(Enemy::AI_WANDER, n);
        if n == 0 {
            lib::draw_6b(x);
            return pursue_tail(x);
        }
    }
    x.set8(DisplacerBeast::REPEAT_LEFT, 2);
    engage(x);
}

/// 0x72c7e: the beast freezes for a moment if a test on the target invites it to, otherwise it engages.
pub(super) fn pursue_tail<R: Bench>(x: &mut Cx<'_, R>) {
    if lib::maybe_draw_6e(x) {
        return x.set16(f::SUB2, 0x0400);
    }
    x.set8(DisplacerBeast::REPEAT_LEFT, 2);
    engage(x);
}

/// 0x72c96: is the target in range? Strikes, grabs, or keeps advancing.
pub(super) fn engage<R: Bench>(x: &mut Cx<'_, R>) {
    if lib::target_in_box(x, -0x30, 0x18, 4) {
        set_state_long(x, 0x0204_0200);
        return lib::pick_byte(x);
    }
    if reach_front(x) || reach_back(x) {
        return;
    }
    if lib::guard_in_front_strict(x, -0x30, 0x30, 0x20) {
        return lib::pick_byte(x);
    }
    step(x);
    move_xy(x);
    face_resume(x);
}

/// If the target is behind the beast, it turns around then resumes (0x72ce2 and following).
fn face_resume<R: Bench>(x: &mut Cx<'_, R>) {
    let Some(t) = target(x) else { return };
    let dx = x.u16(Obj::X).wrapping_sub(t.u16(x.b, Obj::X)) as i16;
    let want = if dx >= 0 { 0 } else { 0x20 };
    if want != x.u8(f::FLIP) {
        x.set32(DisplacerBeast::RESUME_STATE, ACTIVE_STANCE);
        x.set8(DisplacerBeast::RESUME_SUB2, 0);
        set_state_long(x, 0x0202_0200);
    }
}

/// A player is right in front (0x72d2a or 0x72da4): short or long attack. `far`: range of the second box.
fn reach_gen<R: Bench>(x: &mut Cx<'_, R>, d0: i16, close: bool) -> bool {
    if !lib::target_in_box(x, d0, 0x20, 2) {
        return false;
    }
    let Some(t) = target(x) else { return false };
    let same_side = t.u8(x.b, f::FLIP) == x.u8(f::FLIP);
    if !lib::decision(x, same_side) {
        return false;
    }
    let r = (x.random() as u32) as u16;
    let mask = if close { 0xf } else { 0x2 };
    let take_a = if close { (0xffc0u16 >> (r & mask)) & 1 != 0 } else { r & mask != 0 };
    if take_a {
        set_state_long(x, 0x0202_0800);
        return true;
    }
    set_state_long(x, 0x0204_0000);
    lib::pick_byte(x);
    choose_attack_a(x, close);
    true
}

/// 0x72d2a: a player at arm's length.
fn reach_front<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    reach_gen(x, -0x40, true)
}

/// 0x72da4: a player further behind.
fn reach_back<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    reach_gen(x, -0xb0, false)
}

/// +6 = 2 (0x72e18): turn-around; when done, the beast resumes the saved state.
fn turn<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Enemy::SUB_STEP) == 0 {
        x.add8(Enemy::SUB_STEP, 2);
        x.set8(Enemy::VULNERABLE, 1);
        x.set8(DisplacerBeast::WANT_ANIM, 5);
        let want = x.u8(DisplacerBeast::WANT_ANIM);
        x.set8(DisplacerBeast::CUR_ANIM, want);
        let flip = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
        return play(x, 5);
    }
    if x.anim_step() {
        let resume = x.u32(DisplacerBeast::RESUME_STATE);
        x.set32(f::STATE, resume);
        let sub2 = x.u8(DisplacerBeast::RESUME_SUB2);
        x.set8(f::SUB2, sub2);
        play(x, 0);
    }
}

/// +6 = 4 (0x72e78): standing wait before moving on to the attack.
fn wait<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Enemy::SUB_STEP) == 0 {
        return super::attacks::ready(x);
    }
    let n = x.u8(Enemy::AI_WAIT).wrapping_sub(1);
    x.set8(Enemy::AI_WAIT, n);
    if (n as i8) < 0 {
        return set_state_long(x, ACTIVE_STANCE);
    }
    if player_ahead(x) {
        x.set32(DisplacerBeast::RESUME_STATE, ACTIVE_STANCE);
        x.set8(DisplacerBeast::RESUME_SUB2, 4);
        set_state_long(x, 0x0202_0200);
    }
    step(x);
}

/// +6 = 6 (0x72eec): approaches the target while looking at it.
fn approach<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            x.add8(Enemy::SUB_STEP, 2);
            x.set8(DisplacerBeast::TIMER, 0x28);
            lib::draw_stance(x);
            approach_walk(x);
        }
        // resumes the walking step, without redoing the count
        4 => {
            x.add8(Enemy::SUB_STEP, 0xfe);
            approach_walk(x);
        }
        2 => {
            let t = x.u8(DisplacerBeast::TIMER).wrapping_sub(1);
            x.set8(DisplacerBeast::TIMER, t);
            if (t as i8) < 0 {
                return set_state_long(x, ACTIVE_STANCE);
            }
            let n = x.u8(Enemy::AI_STANCE).wrapping_sub(1);
            x.set8(Enemy::AI_STANCE, n);
            if (n as i8) < 0 {
                lib::draw_stance(x);
                pick_target(x);
                x.set8(Enemy::AI_POINT_SET, 0xe);
                lib::approach_target(x);
            }
            if player_ahead(x) {
                x.set32(DisplacerBeast::RESUME_STATE, 0x0202_0004);
                let sub2 = x.u8(f::SUB2);
                x.set8(DisplacerBeast::RESUME_SUB2, sub2);
                set_state_long(x, 0x0202_0200);
            }
            step(x);
            move_xy(x);
        }
        _ => {}
    }
}

fn approach_walk<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Enemy::VULNERABLE, 1);
    x.set8(DisplacerBeast::WANT_ANIM, 0x16);
    if x.u8(DisplacerBeast::WANT_ANIM) != x.u8(DisplacerBeast::CUR_ANIM) {
        let want = x.u8(DisplacerBeast::WANT_ANIM);
        x.set8(DisplacerBeast::CUR_ANIM, want);
        play(x, 0xd);
    }
    speed(x, 0x17);
}

// --- choice of the attacks ---------------------------------------------------------------------------

/// 0x745a4 (`close`) / 0x745ac: draws an attack from the table of the profile and of the difficulty level.
fn choose_attack_a<R: Bench>(x: &mut Cx<'_, R>, close: bool) {
    let base = if close { 0 } else { 0x14 };
    let profile = x.u8(Enemy::GROUP).wrapping_shl(2) as u32;
    let table = tables::table(PICK_TABLE).at32(x.b, profile);
    let level = rank_row(x);
    let r = (x.random() as u32) & 0x1f;
    let pick = tables::held8(x.b, table.wrapping_add(level).wrapping_add(r));
    x.set8(f::SUB2, pick.wrapping_add(base));
    x.set16(f::STATE, 0x0204);
    x.set8(Enemy::SUB_STEP, 0);
    if x.in_arena(0) {
        return;
    }
    // off screen: no leap
    if matches!(x.u8(f::SUB2), 0x16 | 0x18 | 0x1a | 0x1c | 0x1e) {
        x.set8(f::SUB2, 0x14);
    }
}

/// 0x74c3e: end of an attack, the beast chains into a charge or gets back on its feet.
pub(super) fn next_move<R: Bench>(x: &mut Cx<'_, R>) {
    let Some(_) = target(x) else { return set_state_long(x, ACTIVE_STANCE) };
    if !lib::target_in_box(x, -0x30, 0x30, 8) && !lib::target_in_box(x, -0xd0, 0x70, 8) {
        return set_state_long(x, ACTIVE_STANCE);
    }
    let pick = rank_byte(x, CLOSE_TABLE);
    x.set8(f::SUB2, pick);
    x.set16(f::STATE, 0x0208);
    x.set8(Enemy::SUB_STEP, 0);
}
