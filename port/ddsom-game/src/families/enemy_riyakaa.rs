//! Enemy "Riyakaa" (ENEMY type 16, 0x6f1c0). See `docs/families/enemy.md`.
//!
//! Four-level state machine: `+4` main state (0 creation, 2 active, 4 hit, 6 release), `+5` substate (active state: 0
//! arrival according to the variant `+3`, 2 choice of an attack, 4 paw strikes, 6 special hits, 8 pursuit, 10 rest),
//! `+6` and `+7` sub-substates. Each function below is a substate of the original, named after its role; the original
//! address is noted when it helps to find the code.

use crate::families::cx::Bench;
use super::cx::{handlers, Cx};
use super::enemy_common::*;
use crate::objects::Current;

mod riyakaa {
    use super::*;

    /// Animation sets of the body and of the hits.
    const BODY: u32 = 0x1665d6;
    const STRIKE: u32 = 0x1665f8;
    /// Address tables of the reactions to hits (state 4: hit while standing / hit in the middle of an action).
    const REACT_STANDING: u32 = 0x6fe22;
    const REACT_ACTING: u32 = 0x70018;
    /// Bytes for choosing the next gesture, 32 per difficulty level (`A5+0x7f92` / 2), 5-bit draw.
    const NEXT_MOVES: u32 = 0x5ce72;
    const NEXT_ATTACKS: u32 = 0x701dc;
    /// Decision masks of the counter-attack (one long per difficulty level).
    const MASKS: u32 = 0x6ff70;
    /// Long state "active, substate 0": return to walking.
    const WALK: u32 = 0x0202_0000;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        if suspended(x) {
            return;
        }
        x.tick_counters();
        match x.u8(f::STATE) {
            st::INIT => {
                if paused(x) {
                    return;
                }
                init(x);
                active(x);
            }
            st::ACTIVE => active(x),
            st::HIT => hit(x),
            st::DEAD => free(x),
            _ => {}
        }
    }

    fn init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(f::STATE, 2);
        x.set8(f::BOX_SET, 0xf);
        lib::initial_hp(x);
        lib::set_immunities(x);
        // AI parameters read by the decision tables, and starting values
        for (off, v) in [
            (0x68, 2),
            (0x69, 2),
            (0x70, 0),
            (0x6a, 1),
            (0x6c, 2),
            (0x6d, 4),
            (0x72, 2),
            (0x73, 2),
            (0x74, 2),
            (0x75, 0),
            (0x76, 3),
            (0xa2, 0x20),
            (0xa1, 2),
            (0x67, 1),
            (0x8e, 0),
            (0x7f, 0),
            (0x7e, 0x44),
        ] {
            x.set8(off, v);
        }
        x.set32(0x8a, 0);
        x.set8(0xad, 0);
        x.set8(0x79, 6);
        x.set8(0x81, 6);
        x.set8(0x80, 7);
    }

    /// Plays animation `n` of the movement set.
    fn body<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
        x.anim_start(BODY, n as u16);
    }

    /// Plays animation `n` of the hit set.
    fn strike<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
        x.anim_start(STRIKE, n as u16);
    }

    /// Chooses speed table `n`.
    fn speed<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
        x.select_speed(&crate::objects::motion::SPEEDS_B, n as u16);
    }

    /// Return to the active state, substate 0.
    fn walk<R: Bench>(x: &mut Cx<'_, R>) {
        x.set32(f::STATE, WALK);
    }

    fn toggle_flip<R: Bench>(x: &mut Cx<'_, R>) {
        let flip = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
    }

    /// Decrements a byte and returns its new value.
    fn countdown<R: Bench>(x: &mut Cx<'_, R>, off: u32) -> u8 {
        let n = x.u8(off).wrapping_sub(1);
        x.set8(off, n);
        n
    }

    // --- state 2: active -----------------------------------------------------------------------

    fn active<R: Bench>(x: &mut Cx<'_, R>) {
        active_loop(x, |x| {
            match x.u8(f::SUB) {
                0 => return arrive(x),
                2 => decide(x),
                4 => swipe(x),
                6 => special(x),
                8 => chase(x),
                10 => rest(x),
                _ => {}
            }
            false
        });
    }

    /// Substate 0: arrival according to the variant `+3`. Returns true when the original abandons the frame without
    /// finishing it (variant 8, `+6 = 2`: it pops the return address of the substate function).
    fn arrive<R: Bench>(x: &mut Cx<'_, R>) -> bool {
        match x.u8(0x3) {
            0 | 4 => charge_in(x),
            2 => rise(x),
            6 => leap_in(x),
            8 => return approach(x),
            10 => hover(x),
            _ => {}
        }
        false
    }

    /// Movement common to the arrivals: the duration `+0xa2` counted down, then return to walking.
    fn fly_step<R: Bench>(x: &mut Cx<'_, R>) {
        if countdown(x, 0xa2) == 0 {
            walk(x);
            x.set8(0, 1);
        }
        x.move_by_table_xy();
        x.anim_step();
    }

    /// 0x6f2e0: enters running (variants 0 and 4).
    fn charge_in<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(f::SUB2) != 0 {
            return fly_step(x);
        }
        x.add8(f::SUB2, 2);
        x.set8(0x67, 1);
        x.set16(0xa0, 0);
        x.set8(0xa2, 0x18);
        x.set8(0x2d, 0x18);
        if x.u8(f::FLIP) != 0 {
            x.set8(0x2d, 8);
        }
        x.set16(0xa0, 0x202);
        speed(x, 0x17);
        let mut n = 3;
        if x.u8(0x3) == 0 {
            x.set16(0xa0, 0);
            speed(x, 0xd);
            n = 2;
        }
        body(x, n);
    }

    /// 0x6f35e: enters leaping (variant 6).
    fn leap_in<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(f::SUB2) != 0 {
            return fly_step(x);
        }
        x.add8(f::SUB2, 2);
        x.set8(0x67, 1);
        x.set8(0xa2, 0x20);
        x.set8(0xa1, 0xd);
        x.set8(0x2d, 0x10);
        speed(x, 0xd);
        body(x, 6);
    }

    /// 0x6f3aa: start of the fall (springs upward then falls back).
    fn rise_start<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(f::SUB2, 2);
        x.set8(0x67, 0);
        x.set8(0xa1, 0xe);
        x.set16(0x18, 0xfd00);
        x.set16(0x1a, 0xff80);
        body(x, 0xb);
    }

    /// 0x6f3d2: falls back to the ground.
    fn rise_fall<R: Bench>(x: &mut Cx<'_, R>) {
        x.move_z();
        if (x.u16(0xc) as i16) >= 0 {
            return;
        }
        x.add8(f::SUB2, 2);
        x.set32(0xc, 0);
        body(x, 0x10);
    }

    /// 0x6f3f6: end of the landing animation.
    fn rise_land<R: Bench>(x: &mut Cx<'_, R>) {
        if x.anim_step() {
            walk(x);
            x.set8(0, 1);
        }
    }

    /// Variant 2: falls from the top of the screen.
    fn rise<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(f::SUB2) {
            0 => rise_start(x),
            2 => rise_fall(x),
            4 => rise_land(x),
            _ => {}
        }
    }

    /// Variant 10: appears after 30 frames, then falls like variant 2.
    fn hover<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(f::SUB2) {
            0 => {
                x.set8(0x90, 0x1e);
                rise_start(x);
            }
            2 => {
                if countdown(x, 0x90) == 0 {
                    x.add8(f::SUB2, 2);
                }
            }
            4 => rise_fall(x),
            6 => rise_land(x),
            _ => {}
        }
    }

    /// Variant 8: approaches until it sees a player; at `+6 = 2` the original abandons the frame.
    fn approach<R: Bench>(x: &mut Cx<'_, R>) -> bool {
        match x.u8(f::SUB2) {
            0 => {
                x.add8(f::SUB2, 2);
                x.set8(0x67, 0);
                x.set8(0xa1, 0xb);
                body(x, 0xd);
            }
            2 => {
                x.anim_step();
                if lib::pick_target(x).is_some() && lib::target_in_box(x, 0, 0x80, 0x20) {
                        x.add8(f::SUB2, 2);
                    }
                return true;
            }
            4 => {
                x.add8(f::SUB2, 2);
                x.set8(0x67, 1);
                x.set8(0xa1, 0xc);
                body(x, 0xa);
            }
            6 => {
                if x.anim_step() {
                    x.set8(f::SUB, 0xa);
                    x.set16(f::SUB2, 0);
                }
            }
            _ => {}
        }
        false
    }

    /// Substate 10: waits, standing, until a player is alive.
    fn rest<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(f::SUB2) == 0 {
            x.add8(f::SUB2, 2);
            x.set8(0x67, 1);
            x.set8(0xa1, 1);
            return body(x, 0);
        }
        if var::PLAYERS_ALIVE.get(x.b) == 0 {
            x.anim_step();
        } else {
            walk(x);
            lib::pick_target(x);
        }
    }

    // --- substate 2: choice of an attack ---------------------------------------------------------

    fn decide<R: Bench>(x: &mut Cx<'_, R>) {
        if grabbed(x) {
            x.set16(f::STATE, 0x0408);
            x.set16(f::SUB2, 0);
            return grab(x);
        }
        match x.u8(f::SUB2) {
            0 => {
                if x.u8(0x7) != 0 {
                    decide_follow(x);
                } else {
                    look_begin(x);
                }
            }
            2 => decide_turn(x),
            4 => decide_wait(x),
            _ => {}
        }
        if var::PLAYERS_ALIVE.get(x.b) == 0 {
            x.set32(f::STATE, 0x020a_0000);
        }
    }

    /// 0x6f546: start of the search for a target; the animation depends on the presence of a player very close.
    fn look_begin<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x7, 2);
        x.set8(0x79, 6);
        x.set8(0x67, 1);
        let (mut d4, mut d5, mut d6) = (0xd, 0, 2);
        if !lib::target_in_box(x, 0, 0xa0, 0x20) {
            x.set8(0xad, 1);
            d4 = 0x17;
            d6 = 3;
            d5 = 2;
        }
        x.set8(0xa1, d5);
        x.anim_start(BODY, d6);
        x.select_speed(&crate::objects::motion::SPEEDS_B, d4);
        lib::pick_target(x);
        lib::draw_stance(x);
        lib::draw_6b(x);
    }

    /// 0x6f5a6: follows the target as long as the gesture lasts.
    fn decide_follow<R: Bench>(x: &mut Cx<'_, R>) {
        lib::approach_target(x);
        let n = countdown(x, 0x71) as i8;
        if n < 0 {
            lib::draw_stance(x);
            if lib::pick_target(x).is_none() {
                return;
            }
            if countdown(x, 0x6b) == 0 {
                lib::draw_6b(x);
                if lib::maybe_draw_6e(x) {
                    x.set16(f::SUB2, 0x400);
                    return;
                }
            }
        }
        decide_near(x);
    }

    /// 0x6f5e0: opponent very close (strike), within jumping range (leap) or to be reached (walk).
    fn decide_near<R: Bench>(x: &mut Cx<'_, R>) {
        if lib::target_in_box(x, -0x20, 0x20, 8) {
            x.set32(f::STATE, 0x0204_0c00);
            x.set8(0xa3, 6);
            return;
        }
        if want_attack(x) {
            return choose_attack(x);
        }
        if lib::guard_in_front_strict(x, -0x38, 0x38, 8) {
            let v = x.u8(f::SUB2);
            x.set8(0xa3, v);
            x.set16(f::SUB2, 0xc00);
            return lib::pick_byte(x);
        }
        x.anim_step();
        x.move_by_table_xy();
        if needs_turn(x) {
            x.add8(f::SUB2, 2);
            x.set8(0x7, 0);
        }
    }

    /// 0x6f650: turns around toward the target.
    fn decide_turn<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(0x7) == 0 {
            x.add8(0x7, 2);
            x.set8(0x67, 1);
            return body(x, 4);
        }
        if !x.anim_step() {
            return;
        }
        toggle_flip(x);
        x.set8(f::SUB2, 0);
        body(x, 2);
    }

    /// 0x6f694: standing wait before setting off again.
    fn decide_wait<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(0x7) == 0 {
            x.add8(0x7, 2);
            x.set8(0x67, 1);
            x.set8(0xa0, 1);
            if x.u8(0xa1) != 1 {
                x.set8(0xa1, 1);
                return body(x, 0);
            }
        }
        if (countdown(x, 0x6e) as i8) < 0 {
            x.set16(f::SUB2, 0);
        }
        x.anim_step();
    }

    // --- substate 4: paw strikes -----------------------------------------------------------------

    fn swipe<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(f::SUB2) {
            0 | 2 | 4 => swing(x),
            6 => lunge(x),
            8 | 10 => dash(x),
            12 => wait_target(x),
            14 => turn_around(x),
            _ => {}
        }
    }

    /// Setting up a paw strike: attack identifier `id`, strike animation `anim`, gaze shift `shift` (`+0xa0`).
    fn swing_pose<R: Bench>(x: &mut Cx<'_, R>, id: u8, anim: u32, shift: u8) {
        x.set8(0x44, id);
        x.set8(0xa0, shift);
        x.add8(0x7, 2);
        x.anim_start(STRIKE, anim as u16);
    }

    /// 0x6f6f0: paw strike (`+6` = 0 high, 2 middle, 4 low).
    fn swing<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(0x7) != 0 {
            if !x.anim_step() {
                return;
            }
            if (countdown(x, 0xa2) as i8) < 0 {
                return retreat(x);
            }
            x.add8(f::SUB2, 0xfe);
            return swing_pose(x, 0xb, 1, 0xf0);
        }
        x.set8(0x67, 0);
        x.set8(0xa2, 0);
        x.set8(0x45, 5);
        let v = x.u8(0xa0);
        x.set8(0xa1, v);
        match x.u8(f::SUB2) {
            2 => swing_pose(x, 0xc, 0, 0xf2),
            0 => swing_pose(x, 0xb, 1, 0xf0),
            _ => {
                x.add8(0xa2, 1);
                swing_pose(x, 0xc, 0, 0xf4);
            }
        }
    }

    /// 0x6f770: leap forward then landing (`+7`: 0 run-up, 2 flight, 4 fall back, 6 rebound).
    fn lunge<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x7) {
            0 => lunge_start(x),
            2 => {
                x.move_xy();
                if x.anim_step() {
                    x.add8(0x7, 2);
                    x.set16(0xc, 0xc);
                    x.set16(0x14, 0);
                    x.set16(0x16, 0);
                    strike(x, 4);
                }
            }
            4 => {
                x.move_xz();
                x.anim_step();
                if x.u16(0xc) < 0xc {
                    x.add8(0x7, 2);
                    body(x, 0x10);
                }
            }
            6 => {
                x.move_xz();
                x.anim_step();
                if (x.u16(0xc) as i16) < 0 {
                    x.set16(0xc, 0);
                    retreat(x);
                }
            }
            _ => {}
        }
    }

    /// 0x6f784: run-up of the leap (also used as a counter-attack, hit state).
    fn lunge_start<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x7, 2);
        x.set8(0xac, 0x1a);
        x.set8(0x45, 8);
        x.set8(0x78, 2);
        x.set8(0x67, 0);
        let v = x.u8(0xa0);
        x.set8(0xa1, v);
        x.set8(0xa0, 0xf6);
        x.set16(0x14, 0);
        x.set16(0x1c, 0);
        x.set16(0x1e, 0);
        x.set16(0x18, 0x600);
        x.set16(0x1a, 0xffa0);
        let mut d = 0xffd0u16;
        if x.u8(f::FLIP) != 0 {
            d = d.wrapping_neg();
        }
        x.set32(0x8a, 0x0204_0600);
        x.set16(0x16, d);
        strike(x, 2);
    }

    /// 0x6f85e: straight-line run (`+6` = 8: then strike, 10: then walk).
    fn dash<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(0x7) == 0 {
            x.add8(0x7, 2);
            x.set8(0x67, 0);
            x.set8(0xa1, 0);
            x.set32(0xa8, 0x0204_0200);
            if x.u8(f::SUB2) != 8 {
                x.set32(0xa8, WALK);
            }
            x.set8(0xa0, 0);
            x.set8(0xa2, 0x20);
            x.set8(0x2d, 8);
            if x.u8(f::FLIP) != 0 {
                x.set8(0x2d, 0x18);
            }
            speed(x, 0xd);
            return body(x, 2);
        }
        x.move_by_table_xy();
        x.anim_step();
        if (countdown(x, 0xa2) as i8) < 0 {
            let s = x.u32(0xa8);
            x.set32(f::STATE, s);
        }
    }

    /// 0x6f8d6: waits to know where to go (`+0xa3` = substate to resume).
    fn wait_target<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(0x7) == 0 {
            x.add8(0x7, 2);
            x.set8(0x67, 1);
            lib::pick_byte(x);
            if x.u8(0x77) == 0 {
                x.set8(0x7, 0);
                let v = x.u8(0xa3);
                x.set8(f::SUB2, v);
            } else if x.u8(0xa1) != 1 {
                x.set8(0xa1, 1);
            } else {
                return wait_target_follow(x);
            }
            return body(x, 0);
        }
        wait_target_follow(x);
    }

    fn wait_target_follow<R: Bench>(x: &mut Cx<'_, R>) {
        x.anim_step();
        if (countdown(x, 0x77) as i8) >= 0 {
            return;
        }
        x.set8(0x7, 0);
        if needs_turn(x) {
            x.add8(f::SUB2, 2);
        } else {
            let v = x.u8(0xa3);
            x.set8(f::SUB2, v);
        }
    }

    /// 0x6f93e: about-turn.
    fn turn_around<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(0x7) == 0 {
            x.add8(0x7, 2);
            x.set8(0x67, 1);
            return body(x, 4);
        }
        if !x.anim_step() {
            return;
        }
        toggle_flip(x);
        x.set16(f::SUB2, 0xc00);
        body(x, 0);
    }

    // --- substate 6: special hits ----------------------------------------------------------------

    fn special<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(f::SUB2) {
            0 | 10 => roar(x),
            2 | 8 => circle(x),
            4 | 6 => jump_strike(x),
            12 => wait_target(x),
            14 => turn_around(x),
            _ => {}
        }
    }

    /// 0x6f990: cry (if the game forbids it, a simple look).
    fn roar<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x7) {
            0 => {
                x.add8(0x7, 2);
                x.set8(0xa1, 0x1e);
                x.set8(0x67, 1);
                if lib::chance_10_16(x) {
                    return body(x, 1);
                }
                x.add8(0x7, 2);
                x.set8(0xa2, 0x21);
                body(x, 0xe);
            }
            2 => {
                if x.anim_step() {
                    stand_up(x);
                }
            }
            4 => {
                if (countdown(x, 0xa2) as i8) < 0 {
                    return stand_up(x);
                }
                x.anim_step();
                if x.u8(0x33) != 0 {
                    toggle_flip(x);
                }
            }
            _ => {}
        }
    }

    /// 0x6fa0c: circles around its target (`+6` = 2: run then end; 8: run then strike).
    fn circle<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x7) {
            0 => {
                x.add8(0x7, 2);
                x.set8(0x67, 1);
                x.set8(0x6b, 0x18);
                x.set8(0x2d, 0);
                if (x.random() as u32) & 1 != 0 {
                    x.add8(0x2d, 0x10);
                }
                speed(x, 0xd);
                x.set8(0xa1, 0);
                body(x, 2);
            }
            2 => {
                x.move_by_table_xy();
                x.anim_step();
                if (countdown(x, 0x6b) as i8) >= 0 {
                    return;
                }
                if x.u8(f::SUB2) == 8 {
                    x.set8(0xa1, 0x1d);
                    x.add8(0x7, 2);
                } else {
                    stand_up(x);
                }
            }
            4 => {
                x.add8(0x7, 2);
                x.set8(0x44, 0xc);
                x.set8(0x45, 5);
                strike(x, 0);
            }
            6 => {
                if x.anim_step() {
                    stand_up(x);
                }
            }
            _ => {}
        }
    }

    /// 0x6faba: jump with a strike (`+6` = 4: lands and ends; 6: lands and chains a strike).
    fn jump_strike<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x7) {
            0 => {
                x.add8(0x7, 2);
                x.set8(0x67, 1);
                x.set8(0xa1, 0x15);
                x.set16(0x1a, 0xffc8);
                x.set16(0x18, 0x200);
                x.set16(0x14, 0x400);
                x.set16(0x16, 0);
                if x.u8(f::FLIP) != 0 {
                    let v = (x.u16(0x14) as i16).wrapping_neg();
                    x.set16(0x14, v as u16);
                }
                body(x, 9);
            }
            2 => {
                x.move_xz();
                x.anim_step();
                if (x.u32(0xc) as i32) >= 0 {
                    return;
                }
                x.set32(0xc, 0);
                if x.u8(f::SUB2) == 6 {
                    x.add8(0x7, 2);
                } else {
                    stand_up(x);
                }
            }
            4 => {
                x.add8(0x7, 2);
                x.set8(0x44, 0xb);
                x.set8(0x45, 5);
                strike(x, 1);
            }
            6 => {
                if x.anim_step() {
                    stand_up(x);
                }
            }
            _ => {}
        }
    }

    // --- substate 8: throws itself at a player --------------------------------------------------

    fn chase<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(f::SUB2) {
            0 => chase_wait(x),
            2 | 4 => chase_run(x),
            6 => chase_pounce(x),
            _ => {}
        }
    }

    /// 0x6fbfc: wait of 16 frames.
    fn chase_wait<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(0x7) == 0 {
            x.add8(0x7, 2);
            x.set8(0x67, 1);
            x.set8(0xa2, 0x10);
            x.set8(0xa1, 1);
            x.set8(0xa0, 1);
            x.add8(0x7, 2);
            return body(x, 0);
        }
        if countdown(x, 0xa2) == 0 {
            walk(x);
        }
        x.anim_step();
    }

    /// 0x6fc44: runs toward its target (`+6` = 4 in X, 2 in depth).
    fn chase_run<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(0x7) == 0 {
            x.add8(0x7, 2);
            x.set8(0x67, 1);
            let target = x.linked(f::OWNER);
            x.set8(0xa1, 0);
            if x.u8(f::SUB2) == 4 {
                x.set8(0xa2, 0x18);
                let d = x.u16(0x8).wrapping_sub(target.u16(x.b, 0x8));
                x.set8(0x2d, 8);
                if (d as i16) < 0 {
                    x.set8(0x2d, 0x18);
                }
            } else {
                x.set8(0xa2, 0x10);
                let d = x.u16(0x10).wrapping_sub(target.u16(x.b, 0x10));
                x.set8(0x2d, 0x10);
                if (d as i16) < 0 {
                    x.set8(0x2d, 0);
                }
            }
            x.set8(0xa0, 0);
            x.set8(0x67, 1);
            x.set8(0xa1, 0);
            speed(x, 0xd);
            body(x, 2);
        }
        if countdown(x, 0xa2) == 0 {
            walk(x);
        }
        x.move_by_table_xy();
        x.anim_step();
    }

    /// 0x6fb78: jumps on the player it aims at (three gaze tries).
    fn chase_pounce<R: Bench>(x: &mut Cx<'_, R>) {
        x.set8(0x67, 1);
        let v = x.u8(0xa0);
        x.set8(0xa1, v);
        let shift = if lib::target_in_box(x, -0x28, 0x28, 0xc) {
            x.set32(f::STATE, 0x0204_0c00);
            x.set8(0xa3, 6);
            0xf6u8
        } else {
            if !want_attack(x) {
                walk(x);
                x.set8(0xac, 0);
                return look_begin(x);
            }
            choose_attack(x);
            x.u8(0xa3).wrapping_add(0xf0)
        };
        x.set8(0xa0, shift);
        if shift != x.u8(0xa1) {
            x.set8(0xac, 0xff);
        }
        x.add8(0xac, 1);
        if x.u8(0xac) == 3 {
            walk(x);
            look_begin(x);
        }
    }

    // --- common helpers --------------------------------------------------------------------------

    /// 0x700fe: return to walking; a nearby player triggers substate 10.
    fn stand_up<R: Bench>(x: &mut Cx<'_, R>) {
        walk(x);
        if lib::maybe_draw_6e(x) {
            x.set8(f::SUB, 0xa);
        }
    }

    /// 0x700ca: steps back, then chooses the next gesture in a table (substate 8).
    fn retreat<R: Bench>(x: &mut Cx<'_, R>) {
        x.set8(0x78, 0xff);
        let row = ((var::RANK.get(x.b) >> 1) << 5) as u32;
        let r = (x.random() as u32) & 0x1f;
        x.set8(f::SUB, 8);
        let v = x.data8(NEXT_MOVES + row + r);
        x.set8(f::SUB2, v);
        x.set8(0x7, 0);
    }

    /// 0x70116: a player in the box in front of it and the decision to attack is favourable.
    fn want_attack<R: Bench>(x: &mut Cx<'_, R>) -> bool {
        let found = lib::target_in_box(x, 0, 0x70, 8);
        if !found {
            return false;
        }
        if !lib::decision_b(x) {
            return false;
        }
        if !lib::decision_a(x) {
            return false;
        }
        true
    }

    /// 0x70150: is the target behind it (it must turn around)?
    fn needs_turn<R: Bench>(x: &mut Cx<'_, R>) -> bool {
        if x.u16(f::OWNER) == 0 {
            return false;
        }
        let target = x.linked(f::OWNER);
        let d = x.u16(0x8).wrapping_sub(target.u16(x.b, 0x8));
        let facing = if (d as i16) < 0 { 0x20 } else { 0 };
        facing != x.u8(f::FLIP)
    }

    /// 0x70178: draws the next attack gesture in a table; if the game allows it, goes back to substate 4 with this
    /// gesture.
    fn choose_attack<R: Bench>(x: &mut Cx<'_, R>) {
        let row = ((var::RANK.get(x.b) >> 1) << 5) as u32;
        let r = (x.random() as u32) & 0x1f;
        let pick = x.data8(NEXT_ATTACKS + row + r);
        if pick > 4 {
            return;
        }
        if !lib::chance_10_16(x) {
            x.set8(f::SUB, 4);
            x.set8(0xa3, pick);
            x.set8(0x7, 0);
            x.set8(f::SUB2, 0xc);
        } else if x.u8(0xad) != 0 {
            speed(x, 0xd);
            x.set8(0xad, 0);
            body(x, 2);
        }
    }

    // --- state 4: hit ----------------------------------------------------------------------------

    fn hit<R: Bench>(x: &mut Cx<'_, R>) {
        hit_loop(x, |x| match x.u8(f::SUB) {
            0 => react(x, REACT_STANDING),
            2 => knockback(x),
            4 => recover(x),
            6 => react(x, REACT_ACTING),
            8 => grab(x),
            _ => {}
        });
    }

    /// 0x6fece: thrown backward (speed +/- 0x500), then counter-attacks or resumes walking.
    fn knockback<R: Bench>(x: &mut Cx<'_, R>) {
        if paused(x) {
            return;
        }
        if x.u8(f::SUB2) == 0 {
            x.add8(f::SUB2, 2);
            x.set8(f::FREEZE, 4);
            x.set16(0x14, 0x500);
            x.set16(0x16, 0xffc8);
            lib::sound(x, crate::sound::CMD_127_227_B);
            let side = x.u8(0x4a) ^ 0x20;
            x.set8(0x4a, side);
            if side != 0 {
                let a = (x.u16(0x14) as i16).wrapping_neg();
                x.set16(0x14, a as u16);
                let b = (x.u16(0x16) as i16).wrapping_neg();
                x.set16(0x16, b as u16);
            }
            return body(x, 5);
        }
        if x.u8(0x14) != 0 {
            x.move_x();
            return;
        }
        x.set8(0, 1);
        x.set32(0x58, 0);
        let level = ((var::RANK.get(x.b) >> 2) << 2) as u32;
        let mask = x.data32(MASKS + level);
        let bit = (x.random() as u32) & 0x1f;
        if mask >> bit & 1 != 0 {
            x.set32(f::STATE, 0x0204_0600);
            lunge_start(x);
        } else {
            walk(x);
            body(x, 0);
        }
    }

    /// 0x6ff90: gets up after the hit (`+6`: 0 animation, 2 wait, 4 end).
    fn recover<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(f::SUB2) {
            0 => {
                x.add8(f::SUB2, 2);
                body(x, 8);
            }
            2 => {
                if paused(x) {
                    return;
                }
                let n = (x.u16(0x84) as i16).wrapping_sub(1);
                x.set16(0x84, n as u16);
                if n > 0 {
                    return;
                }
                x.set8(0x86, 0);
                x.set8(0x49, 0);
                x.set8(0, 2);
                x.add8(f::SUB2, 2);
                body(x, 8);
            }
            4 => {
                if paused(x) {
                    return;
                }
                if x.anim_step() {
                    x.set8(0x86, 0);
                    x.set8(0, 1);
                    x.set8(0x67, 1);
                    x.set32(f::STATE, WALK);
                    x.set8(0xa0, 0xc0);
                }
            }
            _ => {}
        }
    }

    /// 0x6fd22: grabbed by a player; the escape movement depends on the scene and the orientation.
    fn grab<R: Bench>(x: &mut Cx<'_, R>) {
        if paused(x) {
            return;
        }
        match x.u8(f::SUB2) {
            0 => grab_start(x),
            2 => {
                if x.anim_step() {
                    x.add8(f::SUB2, 2);
                    toggle_flip(x);
                    grab_run(x);
                }
            }
            4 => grab_flee(x),
            _ => {}
        }
    }

    fn grab_start<R: Bench>(x: &mut Cx<'_, R>) {
        x.set8(0xae, 1);
        let flipped = x.u8(f::FLIP) != 0;
        let turn = if var::SCENE.get(x.b) == 4 {
            flipped
        } else {
            if !lib::in_right_half(x) { flipped } else { !flipped }
        };
        if turn {
            x.add8(f::SUB2, 2);
            x.set8(0x67, 1);
            x.set8(0xa0, 0x7f);
            x.set8(0xa1, 0x7f);
            body(x, 4);
        } else {
            x.add8(f::SUB2, 4);
            grab_run(x);
        }
    }

    /// 0x6fdc0: flees running, facing the direction of the escape.
    fn grab_run<R: Bench>(x: &mut Cx<'_, R>) {
        x.set8(0x67, 1);
        x.set8(0x2d, 8);
        if x.u8(f::FLIP) == 0 {
            x.set8(0x2d, 0x18);
        }
        body(x, 2);
        speed(x, 0x21);
    }

    /// 0x6fdf2: flees until it leaves the screen, then disappears.
    fn grab_flee<R: Bench>(x: &mut Cx<'_, R>) {
        x.anim_step();
        x.move_by_table_xy();
        if x.in_arena_default() {
            return;
        }
        x.set32(f::STATE, st::DEAD_LONG);
        detach_from_host(x);
    }
}

handlers! {
    0x6f1c0 "enemy_16" riyakaa::run,
}
