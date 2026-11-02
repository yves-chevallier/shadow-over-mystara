//! Type 12, standing elf (`+5 = 2`) and at rest (`+5 = 10`). `+6`: 0 walks toward the target and decides to attack, 2
//! turn-around, 4 wait.

use crate::families::cx::Bench;
use super::*;
use crate::objects::Current;
use crate::objects::Obj;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if grabbed(x) {
        x.set16(f::STATE, 0x408);
        x.set16(f::SUB2, 0);
        return hit::caught(x);
    }
    match x.u8(f::SUB2) {
        0 => {
            if x.u8(f::SUB3) == 0 {
                begin_walk_step(x);
            } else {
                walk(x);
            }
        }
        2 => turn(x),
        4 => wait(x),
        _ => {}
    }
    if var::PLAYERS_ALIVE.get(x.b) == 0 {
        set_state_long(x, IDLE);
    }
}

/// 0x696b0: the elf starts walking toward its target.
pub(super) fn begin_walk<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(VULNERABLE, 1);
    x.select_speed(&crate::objects::motion::SPEEDS_B, 0x12);
    body(x, 1);
    pick_target(x);
    lib::draw_stance(x);
    lib::draw_6b(x);
}

/// 0x696a0: start of the walk counting one more step.
pub(super) fn begin_walk_step<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB3, 2);
    x.set8(WALK_ANIM, 0xf0);
    x.set8(0x79, 0x1a);
    begin_walk(x);
}

/// 0x696de: walk; turns around, waits or attacks.
fn walk<R: Bench>(x: &mut Cx<'_, R>) {
    lib::approach_target(x);
    let n = x.u8(AI_WALK).wrapping_sub(1);
    x.set8(AI_WALK, n);
    if (n as i8) < 0 {
        lib::draw_stance(x);
        if !pick_target(x) {
            return;
        }
        let n = x.u8(AI_CHOICE).wrapping_sub(1);
        x.set8(AI_CHOICE, n);
        if n == 0 {
            lib::draw_6b(x);
            if lib::maybe_draw_6e(x) {
                return x.set16(f::SUB2, 0x400);
            }
        }
    }
    decide(x);
}

/// 0x69718: a player in range? If so, attacks; otherwise advances by one animation step.
fn decide<R: Bench>(x: &mut Cx<'_, R>) {
    if lib::target_in_box(x, -0x20, 0x20, 0xc) {
        x.set16(f::STATE, 0x204);
        let n = rand_and(x, 1) * 2 + 4;
        x.set8(NEXT_ATTACK, n as u8);
        return x.set16(f::SUB2, 0);
    }
    if front_box(x) {
        pick_attack(x, CLOSE_ATTACKS);
        return;
    }
    if side_box(x) {
        pick_attack(x, FAR_ATTACKS);
        return;
    }
    if lib::guard_in_front_strict(x, -0x38, 0x38, 0xc) {
        let hint = x.u8(f::SUB2);
        x.set8(NEXT_ATTACK, hint);
        x.set16(f::SUB2, 0);
        return lib::pick_byte(x);
    }
    step(x);
    if x.u8(f::KEY_FRAME) & 1 == 0 {
        move_xy(x);
    }
    if needs_turn_around(x) {
        x.add8(f::SUB2, 2);
        x.set8(f::SUB3, 0);
    }
}

/// 0x6a6a6: target in front, at short range.
pub(super) fn front_box<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    box_attack(x, 0, 0x70, 0x10)
}

/// 0x6a698: target behind or far in front (wide box).
pub(super) fn side_box<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    box_attack(x, -0x80, 0x30, 0x10)
}

/// Detection box then decisions by mask: with the same orientation, both (0xa4c2 then 0xa4b4) must accept; otherwise
/// only 0xa4b4.
fn box_attack<R: Bench>(x: &mut Cx<'_, R>, d0: i16, d1: u16, d2: u16) -> bool {
    let found = lib::target_in_box(x, d0, d1, d2);
    found && {
        let target = load_target(x);
        let theirs = target.u8(x.b, f::FLIP);
        let same = theirs == x.u8(f::FLIP);
        (if same {
            lib::decision_b(x)
        } else {
            true
        }) && {
            lib::decision_a(x)
        }
    }
}

/// 0x6a706 / 0x6a70c: the attack to play after the approach is drawn from the difficulty row; it is returned (D0.b of
/// the original).
pub(super) fn pick_attack<R: Bench>(x: &mut Cx<'_, R>, table: u32) -> u8 {
    let row = (var::RANK.get(x.b) >> 1) << 5;
    let n = rand_and(x, 0x1f);
    let mv = x.data8(table + row as u32 + n);
    x.set8(f::SUB, 4);
    x.set16(f::SUB2, 0);
    x.set8(NEXT_ATTACK, mv);
    mv
}

/// 0x6a6de: the target is on the other side of the elf's orientation.
pub(super) fn needs_turn_around<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if x.u16(f::TARGET) == 0 {
        return false;
    }
    let target = load_target(x);
    let dx = x.u16(Obj::X).wrapping_sub(target.u16(x.b, Obj::X));
    let want = if (dx as i16) < 0 { 0x20 } else { 0 };
    want != x.u8(f::FLIP) as u32
}

/// `+6 = 2`: turn-around (animation 2, then orientation inversion).
fn turn<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(VULNERABLE, 1);
        return body(x, 2);
    }
    if step(x) {
        let flip = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
        x.set8(f::SUB2, 0);
        body(x, 1);
    }
}

/// `+6 = 4` : attente en position de garde.
fn wait<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(VULNERABLE, 1);
        return body(x, 0);
    }
    let n = x.u8(AI_WAIT).wrapping_sub(1);
    x.set8(AI_WAIT, n);
    if (n as i8) < 0 {
        x.set16(f::SUB2, 0);
    }
    step(x);
}

/// Substate 10: rest until a player is alive.
pub(super) fn rest<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB2) == 0 {
        x.add8(f::SUB2, 2);
        x.set8(VULNERABLE, 1);
        return body(x, 0);
    }
    if var::PLAYERS_ALIVE.get(x.b) != 0 {
        set_state_long(x, STANCE);
        pick_target(x);
    } else {
        step(x);
    }
}
