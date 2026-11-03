//! Type 0, substate 4: the attacks. `+6`: 0 to 6 club strikes in place, 8 double strike, 10 and 20 leap, 12 paw strike,
//! 14 and 34 breath, 16 and 36 side step and turn-around, 18 run and turn-around, 22 to 28 series of strikes, 30 strike
//! followed by a leap, 32 short paw strike.
//! Each move begins with `begin_wait` (step 0) and `hold` (step 2, pursuit in place).

use crate::families::cx::Bench;
use crate::families::eshl_creators;
use super::*;
use crate::objects::Current;
use crate::objects::Obj;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    // common steps: 0 wait, 2 pursuit in place (back to state 4)
    match x.u8(f::SUB3) {
        0 => return begin_wait(x),
        2 => return hold(x, 0x0204_0000),
        _ => {}
    }
    match x.u8(f::SUB2) {
        0 => club(x),
        2 => club_b(x),
        4 => thrust(x),
        6 => combo(x),
        8 => double_hit(x),
        10 | 20 => jump_attack(x),
        12 => paw(x),
        14 | 34 => breath(x),
        16 | 36 => sidestep(x),
        18 => dash_turn(x),
        22 => series_a(x),
        24 => series_b(x),
        26 => series_c(x),
        28 => series_d(x),
        30 => strike_then_jump(x),
        32 => short_paw(x),
        _ => {}
    }
}

/// 0x557bc: prepares the wait.
pub(super) fn begin_wait<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB3, 2);
    x.set8(Enemy::VULNERABLE, 1);
    x.set8(Kobold::LOOK, 0);
    mv(x, 0);
}

/// 0x5590e / 0x55902: pursuit in place until counter `+0x77` ends, with a turn-around if the target changes side;
/// `resume` is the state to resume after the turn-around.
pub(super) fn hold<R: Bench>(x: &mut Cx<'_, R>, resume: u32) {
    x.set32(Kobold::RESUME_STATE, resume);
    let n = x.u8(Enemy::AI_BYTE).wrapping_sub(1);
    x.set8(Enemy::AI_BYTE, n);
    if (n as i8) < 0 {
        return x.add8(f::SUB3, 2);
    }
    if must_turn(x) {
        let sub2 = x.u8(f::SUB2);
        x.set8(Kobold::RESUME_SUB2, sub2);
        set_state_long(x, 0x0202_0200);
    }
    x.anim_step();
}

/// 0x57924 after a step back of 0x20: the Kobold restores its place in front of the attack.
fn shift_back<R: Bench>(x: &mut Cx<'_, R>) {
    let dx: u16 = if x.u8(f::FLIP) == 0 { 0xffe0 } else { 0x20 };
    x.add16(Obj::X, dx);
}

/// End of an attack: walking animation, recoil of 0x20, then the continuation.
fn end_strike<R: Bench>(x: &mut Cx<'_, R>) {
    mv(x, 0);
    shift_back(x);
    after_strike(x);
}

/// 0x55a4e: end of the move.
fn step_then_after<R: Bench>(x: &mut Cx<'_, R>) {
    if x.anim_step() {
        after_strike(x);
    }
}

/// Animation key: launched shot `throw` (0x9dda8 or 0x9e062; the key is reset to zero).
fn key_effect<R: Bench>(x: &mut Cx<'_, R>, throw: fn(&mut cps2::Cps2Bus, crate::objects::Obj)) {
    if x.u8(f::KEY_FRAME) != 0 {
        x.set8(f::KEY_FRAME, 0);
        throw(x.b, x.o);
    }
}

/// Start of an attack: look `look` is set; if it already was, `+0xad` counts down and brings it back to standing at
/// zero; otherwise attack animation `anim` starts.
fn strike_look<R: Bench>(x: &mut Cx<'_, R>, look: u8, anim: u32) {
    x.set8(Kobold::LOOK, look);
    if x.u8(Kobold::LOOK) == x.u8(Kobold::LOOK_PREV) {
        let g = x.u8(Kobold::GUARD).wrapping_sub(1);
        x.set8(Kobold::GUARD, g);
        if g == 0 {
            return set_state_long(x, STANCE);
        }
    }
    x.set8(Kobold::LOOK_PREV, look);
    strike(x, anim);
}

/// 0x5596a : coup de massue.
fn club_begin<R: Bench>(x: &mut Cx<'_, R>, look: u8, kind: u8, anim: u32) {
    x.add8(f::SUB3, 2);
    x.set8(Enemy::VULNERABLE, 0);
    x.set8(Kobold::LOOK, look);
    x.set8(Enemy::STRIKE_KIND, kind);
    set_power(x);
    strike_look(x, look, anim);
}

// --- +6 = 0 to 6 -------------------------------------------------------------------------------------------

fn club<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        4 => club_begin(x, 0x2d, 0, 5),
        6 => {
            if x.anim_step() {
                x.set8(Enemy::AI_SHOT, 0xff);
                end_strike(x);
            }
        }
        _ => {}
    }
}

fn club_b<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        4 => club_begin(x, 0x28, 1, 0),
        6 => step_then_after(x),
        _ => {}
    }
}

fn thrust<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        4 => {
            x.add8(f::SUB3, 2);
            x.set16(Kobold::TIMER, 0x14);
            x.set8(Enemy::STRIKE_KIND, 3);
            set_power(x);
            x.set8(Enemy::VULNERABLE, 1);
            x.set8(Kobold::LOOK, 0x16);
            set_angle(x);
            mv(x, 0x16);
            set_speed(x, 0x17);
        }
        6 => {
            let n = x.u16(Kobold::TIMER).wrapping_sub(1);
            x.set16(Kobold::TIMER, n);
            if (n as i16) >= 0 {
                x.anim_step();
                x.move_by_table_xy();
                return;
            }
            x.add8(f::SUB3, 2);
            x.set16(Obj::VX, 0x180);
            x.set16(Obj::AX, -0x10i16 as u16);
            if x.u8(f::FLIP) != 0 {
                negate(x, Obj::VX);
                negate(x, Obj::AX);
            }
            strike(x, 0);
        }
        8 => {
            if x.u8(Obj::VX) != 0 {
                x.move_x();
            }
            if x.anim_step() {
                x.set32(Obj::VX, 0);
                after_strike(x);
            }
        }
        _ => {}
    }
}

/// +6 = 6: repeated strikes (`+0xac` times).
fn combo<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        4 => {
            if x.u8(Enemy::GROUP) != 1 {
                x.set8(Enemy::AI_SHOT, 2);
            }
            let r = rand_and(x, 1);
            x.set8(Kobold::REPEAT, 2 + r as u8);
            club_begin(x, 0x2d, 0, 5);
        }
        6 => {
            if !x.anim_step() {
                return;
            }
            mv(x, 0);
            shift_back(x);
            let n = x.u8(Kobold::REPEAT).wrapping_sub(1);
            x.set8(Kobold::REPEAT, n);
            if n == 0 {
                return after_strike(x);
            }
            x.set8(Enemy::AI_SHOT, 0xff);
            strike(x, 5);
        }
        _ => {}
    }
}

/// +6 = 8: double strike (animations 2 then 3).
fn double_hit<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        4 => {
            x.add8(f::SUB3, 2);
            x.set8(Enemy::VULNERABLE, 0);
            x.set8(Enemy::STRIKE_POWER, 5);
            if x.u8(Enemy::GROUP) != 1 {
                x.set8(Enemy::AI_SHOT, 2);
            }
            x.set8(Kobold::LOOK, 0x2a);
            strike(x, 2);
            let r = rand_and(x, 1);
            if r != 0 {
                x.add8(f::SUB3, 2);
                x.set8(Kobold::LOOK, 0x2b);
                strike(x, 3);
            }
            if x.u8(Kobold::LOOK) == x.u8(Kobold::LOOK_PREV) {
                let g = x.u8(Kobold::GUARD).wrapping_sub(1);
                x.set8(Kobold::GUARD, g);
                if g == 0 {
                    return set_state_long(x, STANCE);
                }
            }
            let l = x.u8(Kobold::LOOK);
            x.set8(Kobold::LOOK_PREV, l);
        }
        6 => step_then_after(x),
        8 => {
            if x.anim_step() {
                x.set8(Enemy::AI_SHOT, 0xff);
                end_strike(x);
            }
        }
        _ => {}
    }
}

// --- +6 = 10, 20, 30 : bond ---------------------------------------------------------------------------------

/// 0x55c68 : bond d'attaque, vitesse horizontale `vx`.
fn jump_begin<R: Bench>(x: &mut Cx<'_, R>, vx: i16, kind: u8) {
    x.set16(Obj::VX, vx as u16);
    x.set8(Enemy::STRIKE_KIND, kind);
    set_power(x);
    x.add8(f::SUB3, 2);
    x.set8(Enemy::VULNERABLE, 1);
    x.set8(Kobold::LOOK, 0x2f);
    if x.u8(f::FLIP) != 0 {
        negate(x, Obj::VX);
    }
    x.set16(Obj::AX, 0);
    x.set16(Obj::VZ, 0x6c0);
    x.set16(Obj::AZ, -0x38i16 as u16);
    if x.u8(Kobold::LOOK) == x.u8(Kobold::LOOK_PREV) {
        let g = x.u8(Kobold::GUARD).wrapping_sub(1);
        x.set8(Kobold::GUARD, g);
        if g == 0 {
            return set_state_long(x, STANCE);
        }
    }
    let l = x.u8(Kobold::LOOK);
    x.set8(Kobold::LOOK_PREV, l);
    strike(x, 7);
}

fn jump_attack<R: Bench>(x: &mut Cx<'_, R>) {
    let (vx, kind) = if x.u8(f::SUB2) == 10 { (-0x100, 2) } else { (-0x2a0, 3) };
    jump_series(x, vx, kind);
}

/// Steps of the leap: 4 start, 6 waits for the key, 8 landing, 10 end.
fn jump_series<R: Bench>(x: &mut Cx<'_, R>, vx: i16, kind: u8) {
    match x.u8(f::SUB3) {
        4 => jump_begin(x, vx, kind),
        6 => wait_key(x),
        8 => land(x),
        10 => step_then_after(x),
        _ => {}
    }
}

/// 0x54fc4: landing of a jump → landing animation 0x18.
fn land<R: Bench>(x: &mut Cx<'_, R>) {
    if x.move_xz().gt() {
        x.anim_step();
        return;
    }
    x.set32(Obj::Z, 0);
    x.add8(f::SUB3, 2);
    x.set8(Enemy::VULNERABLE, 1);
    look(x, 0x18);
    mv(x, 0x18);
}

fn strike_then_jump<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        4 => series_strike(x),
        6 => {
            key_effect(x, eshl_creators::throw_2);
            if x.anim_step() {
                x.add8(f::SUB3, 2);
                strike(x, 7);
                shift_back(x);
            }
        }
        8 => jump_begin(x, -0x200, 3),
        10 => wait_key(x),
        12 => land(x),
        14 => step_then_after(x),
        _ => {}
    }
}

// --- +6 = 12 : coup de patte --------------------------------------------------------------------------------

/// 0x565d2: paw strike with a run-up or while running (two variants drawn at random).
pub(super) fn paw_begin<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Enemy::VULNERABLE, 1);
    let r = rand_and(x, 1);
    if r == 0 {
        x.add8(f::SUB3, 4);
        x.set16(Obj::VX, 0x180);
        if x.u8(f::FLIP) != 0 {
            negate(x, Obj::VX);
        }
        x.set16(Obj::AX, 0);
        x.set16(Obj::VZ, 0x3c0);
        x.set16(Obj::AZ, -0x38i16 as u16);
        look(x, 0x1c);
        return mv(x, 0x1c);
    }
    x.add8(f::SUB3, 2);
    let mut angle = 0x18;
    if x.u16(f::TARGET) != 0 {
        let target = x.linked(f::TARGET);
        let d = target.u16(x.b, Obj::X).wrapping_sub(x.u16(Obj::X)) as i16;
        if d >= 0 {
            angle = 8;
        }
    }
    x.set8(Obj::SPEED_INDEX, angle);
    look(x, 0x1d);
    x.set16(Kobold::TIMER, 0x24);
    mv(x, 0x1d);
    set_speed(x, 0xd);
}

fn paw<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        4 => paw_begin(x),
        6 => paw_run(x),
        8 => wait_key(x),
        10 => land(x),
        12 => {
            if x.anim_step() {
                back_on_guard(x);
            }
        }
        _ => {}
    }
}

/// 0x55ce2: run of the paw strike.
fn paw_run<R: Bench>(x: &mut Cx<'_, R>) {
    let n = x.u16(Kobold::TIMER).wrapping_sub(1);
    x.set16(Kobold::TIMER, n);
    if (n as i16) >= 0 {
        x.move_by_table_xy();
        x.anim_step();
        return;
    }
    back_on_guard(x);
}

/// Once the move is over, the Kobold resumes the approach (state 2, `+6` = 0, step 2) and re-checks the ranges
/// (0x555da).
pub(super) fn back_on_guard<R: Bench>(x: &mut Cx<'_, R>) {
    set_state_long(x, 0x0202_0002);
    x.set8(Kobold::LOOK, 0xc);
    mv(x, 0xc);
    stance::reach_boxes(x);
}

// --- +6 = 14 and 34: breath ---------------------------------------------------------------------------------

fn breath<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        4 => {
            x.add8(f::SUB3, 2);
            x.set8(Enemy::VULNERABLE, 1);
            x.set8(Kobold::LOOK, 7);
            let r = rand_and(x, 0x1f);
            let n = tables::table("kobold/breath_repeats").at8(x.b, r);
            x.set8(Kobold::REPEAT, n);
            if x.u8(Kobold::LOOK) == x.u8(Kobold::LOOK_PREV) {
                let g = x.u8(Kobold::GUARD).wrapping_sub(1);
                x.set8(Kobold::GUARD, g);
                if g == 0 {
                    return set_state_long(x, STANCE);
                }
            }
            let l = x.u8(Kobold::LOOK);
            x.set8(Kobold::LOOK_PREV, l);
            mv(x, 7);
        }
        6 => breath_tick(x),
        _ => {}
    }
}

/// 0x55e06: the animation key sometimes fires a projectile (0xe327a); at the end of the script, one breath fewer.
pub(super) fn breath_tick<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::KEY_FRAME) != 0 {
        x.set8(f::KEY_FRAME, 0);
        let r = rand_and(x, 0xf);
        let flag = tables::table("kobold/breath_sounds").at8(x.b, r);
        if flag != 0 {
            lib::sound(x, crate::sound::CMD_300_400);
        }
    }
    if !x.anim_step() {
        return;
    }
    let n = x.u8(Kobold::REPEAT).wrapping_sub(1);
    x.set8(Kobold::REPEAT, n);
    if n == 0 {
        return after_strike(x);
    }
    mv(x, 7);
}

// --- +6 = 16 and 36: side step ---------------------------------------------------------------------------------

fn sidestep<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        4 => {
            x.add8(f::SUB3, 2);
            x.set8(Enemy::VULNERABLE, 1);
            mv(x, 5);
        }
        6 => {
            if !x.anim_step() {
                return;
            }
            x.add8(f::SUB3, 2);
            x.set8(Kobold::LOOK, 8);
            x.set16(Kobold::TIMER, 0x3c);
            if x.u8(Kobold::LOOK) == x.u8(Kobold::LOOK_PREV) {
                let g = x.u8(Kobold::GUARD).wrapping_sub(1);
                x.set8(Kobold::GUARD, g);
                if g == 0 {
                    return set_state_long(x, STANCE);
                }
            }
            mv(x, 8);
        }
        8 => {
            let n = x.u16(Kobold::TIMER).wrapping_sub(1);
            x.set16(Kobold::TIMER, n);
            if (n as i16) >= 0 {
                x.anim_step();
                return;
            }
            x.add8(f::SUB3, 2);
            let flip = x.u8(f::FLIP) ^ 0x20;
            x.set8(f::FLIP, flip);
            mv(x, 5);
        }
        10 => {
            if x.anim_step() {
                let flip = x.u8(f::FLIP) ^ 0x20;
                x.set8(f::FLIP, flip);
                mv(x, 0);
                after_strike(x);
            }
        }
        _ => {}
    }
}

// --- +6 = 18: run and turn-around ------------------------------------------------------------------------------

fn dash_turn<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        4 => {
            x.add8(f::SUB3, 2);
            x.set8(Enemy::VULNERABLE, 1);
            let target = x.u16(f::TARGET) != 0;
            let mut angle = 8;
            if target {
                let t = x.linked(f::TARGET);
                let d = t.u16(x.b, Obj::X).wrapping_sub(x.u16(Obj::X)) as i16;
                angle = if d >= 0 { 0x18 } else { 8 };
            }
            x.set8(Obj::SPEED_INDEX, angle);
            mv(x, 5);
        }
        6 => {
            if !x.anim_step() {
                return;
            }
            x.add8(f::SUB3, 2);
            let flip = x.u8(f::FLIP) ^ 0x20;
            x.set8(f::FLIP, flip);
            x.set8(Kobold::LOOK, 0x16);
            x.set16(Kobold::TIMER, 0x2a);
            mv(x, 0x16);
            set_speed(x, 0x17);
        }
        8 => {
            let n = x.u16(Kobold::TIMER).wrapping_sub(1);
            x.set16(Kobold::TIMER, n);
            if (n as i16) >= 0 {
                x.anim_step();
                x.move_by_table_xy();
                return;
            }
            x.add8(f::SUB3, 2);
            mv(x, 5);
        }
        10 => {
            if x.anim_step() {
                let flip = x.u8(f::FLIP) ^ 0x20;
                x.set8(f::FLIP, flip);
                back_on_guard(x);
            }
        }
        _ => {}
    }
}

// --- +6 = 22 to 28: series of strikes ----------------------------------------------------------------------------

/// 0x5603a: strike with countdown (look 0x2e, animation 6).
fn series_strike<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB3, 2);
    x.set8(Kobold::LOOK, 0x2e);
    x.set8(Enemy::VULNERABLE, 1);
    strike_look(x, 0x2e, 6);
}

fn series_a<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        4 => series_strike(x),
        6 => {
            key_effect(x, eshl_creators::throw_1);
            if x.anim_step() {
                end_strike(x);
            }
        }
        _ => {}
    }
}

/// 0x560d2: prologue of the series: `+0x78`, number of strikes.
fn series_prologue<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Enemy::GROUP) != 1 {
        x.set8(Enemy::AI_SHOT, 3);
    }
    x.set8(Kobold::REPEAT, 1);
    let r = rand_and(x, 1);
    if r != 0 {
        x.set8(Kobold::REPEAT, 2);
    }
    series_strike(x);
}

fn series_b<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        4 => series_strike(x),
        6 => {
            key_effect(x, eshl_creators::throw_2);
            if x.anim_step() {
                x.set8(Enemy::AI_SHOT, 0xff);
                end_strike(x);
            }
        }
        _ => {}
    }
}

fn series_c<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        4 => {
            x.set8(Kobold::REPEAT, 1);
            series_strike(x);
        }
        6 => {
            key_effect(x, eshl_creators::throw_1);
            if x.anim_step() {
                strike(x, 6);
                let n = x.u8(Kobold::REPEAT).wrapping_sub(1);
                x.set8(Kobold::REPEAT, n);
                if n == 0 {
                    mv(x, 0);
                }
                shift_back(x);
                after_strike(x);
            }
        }
        _ => {}
    }
}

fn series_d<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        4 => series_prologue(x),
        6 => {
            key_effect(x, eshl_creators::throw_2);
            if x.anim_step() {
                strike(x, 6);
                let n = x.u8(Kobold::REPEAT).wrapping_sub(1);
                x.set8(Kobold::REPEAT, n);
                if n == 0 {
                    mv(x, 0);
                }
                shift_back(x);
                after_strike(x);
            }
        }
        _ => {}
    }
}

// --- +6 = 32 : coup de patte court -------------------------------------------------------------------------------

fn short_paw<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        4 => {
            x.add8(f::SUB3, 2);
            x.set8(Enemy::VULNERABLE, 1);
            x.set16(Kobold::TIMER, 0xe);
            mv(x, 0xc);
            set_speed(x, 0xb);
        }
        6 => paw_run(x),
        _ => {}
    }
}
