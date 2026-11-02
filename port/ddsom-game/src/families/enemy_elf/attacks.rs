//! Type 12, attacks. `+5 = 4`: `+6` = 0 approach and announcement, 2 turn-around, 4 and 6 simple attack, 8 triple
//! attack, 10 double attack, 12 disappearance and fall onto the target, 14 run-up, 16 to 20 fall from higher up, 22 and
//! 24 back to walking. `+5 = 6`: post-leap attacks. `+5 = 8`: fallback attacks. `+7` is the step.

use crate::families::cx::Bench;
use super::stance::{begin_walk, begin_walk_step, front_box, needs_turn_around, pick_attack, side_box};
use super::*;
use crate::objects::Current;
use crate::objects::Obj;

/// Arrival positions (offsets in X then in depth) after a jump toward the target, drawn by 0x69eee.
const JUMP_DX: u32 = 0x69f4a;
const JUMP_DY: u32 = 0x69f42;

// --- substate 4 --------------------------------------------------------------------------------

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => approach(x),
        2 => turn_around(x),
        4 => cut(x, 0x1b, 0),
        6 => cut(x, 0x1c, 2),
        8 => triple(x),
        10 => double(x),
        12 => drop_strike(x, Drop::Low),
        14 => dash(x),
        16 => drop_strike(x, Drop::High),
        18 => drop_strike(x, Drop::HighCombo),
        20 => drop_strike(x, Drop::HighCombo2),
        22 => {
            set_state_long(x, STANCE);
            begin_walk_step(x);
        }
        24 => {
            set_state_long(x, STANCE);
            x.set8(0x79, 0x1c);
            begin_walk(x);
        }
        _ => {}
    }
}

// --- common steps ----------------------------------------------------------------------------

/// 0x69de8 / 0x69a3a : s'accroupit.
fn crouch_step<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB3, 2);
    crouch(x);
}

/// 0x69df0: waits for the end of the animation.
fn wait_step<R: Bench>(x: &mut Cx<'_, R>) {
    if step(x) {
        x.add8(f::SUB3, 2);
    }
}

/// 0x69e1c : fin de l'animation, retour debout.
fn finish_back<R: Bench>(x: &mut Cx<'_, R>) {
    if step(x) {
        back_to_stance(x);
    }
}

/// 0x69e4e: falls to the ground, then landing animation.
fn fall<R: Bench>(x: &mut Cx<'_, R>) {
    x.move_xz();
    if (x.u16(Obj::Z) as i16) <= 0 {
        x.add8(f::SUB3, 2);
        x.set32(Obj::Z, 0);
        body(x, 5);
    }
}

/// 0x698ac: end of the animation, then the next attack.
fn finish<R: Bench>(x: &mut Cx<'_, R>) {
    if step(x) {
        end_attack(x);
    }
}

// --- coups ---------------------------------------------------------------------------------------

/// `+6` = 0: announcement of the attack, walking animation `+0xa6`, then switch to the planned attack `+0xa5`.
fn approach<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(VULNERABLE, 1);
        lib::pick_byte(x);
        if x.u8(COOLDOWN) == 0 {
            x.set8(f::SUB3, 0);
            let next = x.u8(NEXT_ATTACK);
            x.set8(f::SUB2, next);
        }
        let anim = x.u8(APPROACH_ANIM);
        return body(x, anim as u32);
    }
    step(x);
    let n = x.u8(COOLDOWN).wrapping_sub(1);
    x.set8(COOLDOWN, n);
    if (n as i8) < 0 {
        x.set8(f::SUB3, 0);
        if needs_turn_around(x) {
            x.add8(f::SUB2, 2);
        } else {
            let next = x.u8(NEXT_ATTACK);
            x.set8(f::SUB2, next);
        }
    }
}

/// `+6` = 2: turn-around then back to the approach (`+6 = 0x0c`).
fn turn_around<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(VULNERABLE, 1);
        return body(x, 2);
    }
    if step(x) {
        let flip = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
        x.set16(f::SUB2, 0xc00);
        let anim = x.u8(APPROACH_ANIM);
        body(x, anim as u32);
    }
}

/// Simple attack (`+6` = 4: type 0x1b, animation 0; `+6` = 6: type 0x1c, animation 2).
/// It can also be cancelled in favour of resuming the walk.
fn cut<R: Bench>(x: &mut Cx<'_, R>, kind: u8, anim: u32) {
    match x.u8(f::SUB3) {
        0 => {
            let r = rand_and(x, 0xf);
            if 0xffc0u32 >> r & 1 != 0 {
                x.set8(0x79, 0x1b);
                set_state_long(x, STANCE);
                begin_walk(x);
            } else {
                x.add8(f::SUB3, 2);
                x.set8(STRIKE_KIND, kind);
                x.set8(STRIKE_POWER, 5);
                strike(x, anim);
            }
        }
        _ => finish(x),
    }
}

/// Triple attack (`+6` = 8): three times the low or high attack, drawn at random.
fn triple<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            x.set8(AI_KIND, 3);
            let r = rand_and(x, 1);
            x.set8(REPEAT, r as u8);
            x.set8(STRIKE_KIND, 0x1b);
            x.set8(STRIKE_POWER, 5);
            strike(x, 0);
        }
        2 => {
            if step(x) {
                x.add8(f::SUB3, 2);
                x.set8(STRIKE_KIND, 0x1b);
                x.set8(STRIKE_POWER, 4);
                strike(x, 3);
            }
        }
        4 => {
            if !step(x) {
                return;
            }
            let n = x.u8(REPEAT).wrapping_sub(1);
            x.set8(REPEAT, n);
            if (n as i8) < 0 {
                return end_attack(x);
            }
            x.set8(STRIKE_KIND, 0x1b);
            x.set8(STRIKE_POWER, 4);
            strike(x, 0);
        }
        _ => {}
    }
}

/// Double attack (`+6` = 10): two animations, the first drawn at random.
fn double<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            x.set8(AI_KIND, 3);
            x.set8(REPEAT, 1);
            if rand_and(x, 1) != 0 {
                x.set8(STRIKE_KIND, 0x1c);
                x.set8(STRIKE_POWER, 5);
                strike(x, 2);
            } else {
                x.set8(STRIKE_KIND, 0x1b);
                x.set8(STRIKE_POWER, 5);
                strike(x, 0);
            }
        }
        _ => {
            if !step(x) {
                return;
            }
            let n = x.u8(REPEAT).wrapping_sub(1);
            x.set8(REPEAT, n);
            if (n as i8) < 0 {
                return end_attack(x);
            }
            x.set8(STRIKE_KIND, 0x1c);
            x.set8(STRIKE_POWER, 4);
            strike(x, 2);
        }
    }
}

#[derive(Clone, Copy)]
enum Drop {
    /// `+6` = 12: falls from a height of 0x30 to 0x40 pixels above the target.
    Low,
    /// `+6` = 16 : tombe de 0x60.
    High,
    /// `+6` = 18 and 20: same, with chaining steps.
    HighCombo,
    HighCombo2,
}

/// The elf crouches, disappears, reappears above the target and strikes it as it falls.
fn drop_strike<R: Bench>(x: &mut Cx<'_, R>, kind: Drop) {
    match x.u8(f::SUB3) {
        0 => crouch_step(x),
        2 => {
            if step(x) {
                x.add8(f::SUB3, 2);
                x.set_bit(1, 1, true);
                x.set8(0x3b, 0);
                x.set8(REPEAT, 0x1e);
            }
        }
        4 => {
            let n = x.u8(REPEAT).wrapping_sub(1);
            x.set8(REPEAT, n);
            if n != 0 {
                return;
            }
            x.add8(f::SUB3, 2);
            let target = load_target(x);
            for off in [Obj::X, 0xa, Obj::Y, 0x12] {
                let v = target.u16(x.b, off);
                x.set16(off, v);
            }
            let (height, dx) = match kind {
                Drop::Low => (0x300000, 0x40u16),
                _ => (0x600000, 0x60),
            };
            x.set32(Obj::Z, height);
            x.set16(Obj::VZ, 0);
            x.set16(Obj::AZ, -0x40i16 as u16);
            let (dx, vx) = if x.u8(f::FLIP) != 0 { (dx.wrapping_neg(), 0x400u16) } else { (dx, -0x400i16 as u16) };
            let px = x.u16(Obj::X).wrapping_add(dx);
            x.set16(Obj::X, px);
            x.set16(Obj::VX, vx);
            x.set16(Obj::AX, 0);
            x.set_bit(1, 1, false);
            launch(x);
        }
        6 => {
            if step(x) {
                x.add8(f::SUB3, 2);
                x.set8(STRIKE_KIND, 0x1d);
                x.set8(STRIKE_POWER, 5);
                strike(x, 1);
            }
        }
        8 => {
            if (x.u16(Obj::Z) as i16) <= 0x30 {
                step(x);
            }
            x.move_xz();
            if (x.u16(Obj::Z) as i16) > 0 {
                return;
            }
            x.add8(f::SUB3, 2);
            x.set32(Obj::Z, 0);
            body(x, 5);
        }
        10 => finish(x),
        12 => match kind {
            Drop::HighCombo => {
                x.add8(f::SUB3, 2);
                x.set8(STRIKE_KIND, 0x1b);
                x.set8(STRIKE_POWER, 5);
                let anim = if rand_and(x, 1) == 0 { 0 } else { 2 };
                strike(x, anim);
            }
            Drop::HighCombo2 => {
                x.add8(f::SUB3, 2);
                x.add8(f::SUB3, 2);
                x.set8(STRIKE_KIND, 0x1b);
                x.set8(STRIKE_POWER, 5);
                strike(x, 0);
            }
            _ => {}
        },
        14 if matches!(kind, Drop::HighCombo2) => {
            if step(x) {
                x.add8(f::SUB3, 2);
                x.add8(f::SUB3, 2);
                x.set8(STRIKE_KIND, 0x1b);
                x.set8(STRIKE_POWER, 5);
                strike(x, 2);
            }
        }
        _ => {}
    }
}

/// Run-up (`+6` = 14): jumps then lands in front of the target.
fn dash<R: Bench>(x: &mut Cx<'_, R>) {
    jump_group(x, 0)
}

/// Common steps of the jump groups: 0 crouches, 2 waits, `jump` according to the group, then landing.
fn jump_group<R: Bench>(x: &mut Cx<'_, R>, _g: u8) {
    match x.u8(f::SUB3) {
        0 => crouch_step(x),
        2 | 6 => wait_step(x),
        4 => {
            x.add8(f::SUB3, 2);
            launch(x);
        }
        8 => finish_back(x),
        _ => {}
    }
}

// --- substate 6: post-leap attacks -------------------------------------------------------------

pub(super) fn run_b<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => approach(x),
        2 => turn_around(x),
        4 => back_to_stance(x),
        6 => hover_group(x),
        8 => jump_group(x, 0),
        10 => landing_group(x),
        12 => random_jump_group(x),
        _ => {}
    }
}

/// `+6` = 6: small side jump (in depth).
fn hover_group<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => crouch_step(x),
        2 => wait_step(x),
        4 => {
            let mut d4: u16 = 0x18;
            if rand_and(x, 1) != 0 {
                d4 = d4.wrapping_neg();
            }
            let y = x.u16(Obj::Y).wrapping_add(d4);
            x.set16(Obj::Y, y);
            x.add8(f::SUB3, 2);
            launch(x);
        }
        6 => finish_back(x),
        _ => {}
    }
}

/// `+6` = 10: leap with an attack on landing.
fn landing_group<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => crouch_step(x),
        2 | 6 => wait_step(x),
        4 => {
            x.add8(f::SUB3, 2);
            let h = x.u16(Obj::Z).wrapping_add(0x40);
            x.set16(Obj::Z, h);
            launch(x);
        }
        8 => {
            x.add8(f::SUB3, 2);
            x.set8(STRIKE_KIND, 0x1d);
            x.set8(STRIKE_POWER, 5);
            x.set16(Obj::VZ, 0);
            x.set16(Obj::AX, 0);
            x.set16(Obj::AZ, -0x40i16 as u16);
            let vx = if x.u8(f::FLIP) != 0 { 0x400u16 } else { -0x400i16 as u16 };
            x.set16(Obj::VX, vx);
            strike(x, 1);
        }
        10 => {
            step(x);
            x.move_xz();
            if (x.u16(Obj::Z) as i16) <= 0 {
                x.add8(f::SUB3, 2);
                x.set32(Obj::Z, 0);
                body(x, 5);
            }
        }
        12 => finish_back(x),
        _ => {}
    }
}

/// `+6` = 12: jumps near the target, to a spot drawn at random around it.
fn random_jump_group<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => crouch_step(x),
        2 | 6 => wait_step(x),
        4 => {
            x.add8(f::SUB3, 2);
            let n = rand_and(x, 3) * 2;
            let target = load_target(x);
            let dx = x.rom16(JUMP_DX + n);
            let px = dx.wrapping_add(target.u16(x.b, Obj::X));
            x.set16(Obj::X, px);
            let n = rand_and(x, 3) * 2;
            let target = load_target(x);
            let dy = x.rom16(JUMP_DY + n);
            let py = dy.wrapping_add(target.u16(x.b, Obj::Y));
            x.set16(Obj::Y, py);
            x.set8(f::FLIP, 0x20);
            let d2 = x.u16(Obj::X).wrapping_sub(target.u16(x.b, Obj::X));
            if (d2 as i16) < 0 {
                x.set8(f::FLIP, 0);
            }
            launch(x);
        }
        8 => finish_back(x),
        _ => {}
    }
}

// --- substate 8: fallback attacks -----------------------------------------------------------------

pub(super) fn run_fallback<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            if x.u8(f::SUB3) != 0 {
                let n = x.u8(REPEAT).wrapping_sub(1);
                x.set8(REPEAT, n);
                if (n as i8) < 0 {
                    set_state_long(x, STANCE);
                }
                step(x);
            } else {
                x.add8(f::SUB3, 2);
                x.set8(VULNERABLE, 1);
                x.set8(REPEAT, 0x10);
                body(x, 0);
            }
        }
        2 => backstep_group(x),
        4 => forward_group(x),
        6 => idle_choice(x),
        _ => {}
    }
}

/// `+6` = 2: disappears and reappears 0x30 pixels in front of the elf.
fn backstep_group<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => crouch_step(x),
        2 => {
            if step(x) {
                x.add8(f::SUB3, 2);
                x.set8(REPEAT, 0x1e);
                x.set_bit(1, 1, true);
                x.set8(0x3b, 0);
            }
        }
        4 => {
            let n = x.u8(REPEAT).wrapping_sub(1);
            x.set8(REPEAT, n);
            if n != 0 {
                return;
            }
            let dx = if x.u8(f::FLIP) != 0 { -0x30i16 as u16 } else { 0x30 };
            let px = x.u16(Obj::X).wrapping_add(dx);
            x.set16(Obj::X, px);
            x.set_bit(1, 1, false);
            x.add8(f::SUB3, 2);
            launch(x);
        }
        6 => {
            if step(x) {
                back_to_stance(x);
            }
        }
        8 => crouch_step(x),
        10 | 14 => wait_step(x),
        12 => {
            x.add8(f::SUB3, 2);
            x.set16(Obj::Z, 0x40);
            x.set16(Obj::VZ, 0);
            x.set16(Obj::AZ, -0x40i16 as u16);
            x.set32(Obj::VX, 0x0300_0000);
            if x.u8(f::FLIP) != 0 {
                negate(x, Obj::VX);
            }
            launch(x);
        }
        16 => fall(x),
        18 => finish_back(x),
        _ => {}
    }
}

/// `+6` = 4: jumps toward the target or walks toward it.
fn forward_group<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            if rand_and(x, 1) != 0 {
                x.add8(f::SUB3, 2);
                return crouch(x);
            }
            x.add8(f::SUB3, 0xc);
            x.set8(REPEAT, 0x10);
            x.select_speed(&crate::objects::motion::SPEEDS_B, 0xd);
            x.set8(Obj::SPEED_INDEX, 0);
            let target = load_target(x);
            let dy = x.u16(Obj::Y).wrapping_sub(target.u16(x.b, Obj::Y));
            if (dy as i16) < 0 {
                x.set8(Obj::SPEED_INDEX, 0x10);
            }
            body(x, 1);
        }
        2 | 6 => wait_step(x),
        4 => {
            x.add8(f::SUB3, 2);
            let mut d2: u16 = 0x32;
            let target = load_target(x);
            let dy = x.u16(Obj::Y).wrapping_sub(target.u16(x.b, Obj::Y));
            if (dy as i16) < 0 {
                d2 = d2.wrapping_neg();
            }
            let py = x.u16(Obj::Y).wrapping_add(d2);
            x.set16(Obj::Y, py);
            launch(x);
        }
        8 => fall(x),
        10 => finish_back(x),
        12 => {
            let n = x.u8(REPEAT).wrapping_sub(1);
            x.set8(REPEAT, n);
            if (n as i8) < 0 {
                set_state_long(x, STANCE);
            }
            move_xy(x);
            step(x);
        }
        _ => {}
    }
}

/// `+6` = 6: chooses the next attack according to what the target does within range.
fn idle_choice<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(VULNERABLE, 1);
    set_state_long(x, IDLE);
    let choice = if lib::target_in_box(x, -0x20, 0x20, 0xc) {
        set_state_long(x, 0x0204_0000);
        rand_and(x, 2);
        x.set16(f::SUB2, 0);
        x.u8(NEXT_ATTACK)
    } else if front_box(x) {
        pick_attack(x, CLOSE_ATTACKS)
    } else if side_box(x) {
        pick_attack(x, FAR_ATTACKS)
    } else {
        return;
    };
    post_choice(x, choice);
}

/// 0x69fbc: remembers the chosen action; repeats it as long as it does not change (countdown `+0xa4`).
fn post_choice<R: Bench>(x: &mut Cx<'_, R>, choice: u8) {
    if choice != x.u8(WALK_ANIM) {
        x.set8(WALK_ANIM, choice);
        x.set8(WALK_ANIM_TIMER, 2);
        return;
    }
    let n = x.u8(WALK_ANIM_TIMER).wrapping_sub(1);
    x.set8(WALK_ANIM_TIMER, n);
    if (n as i8) < 0 {
        x.set8(WALK_ANIM_TIMER, 2);
    }
}
