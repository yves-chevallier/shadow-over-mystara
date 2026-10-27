//! Type 3, sous-états 4 (tirs, `+6` = 0 premier geste choisi dans la rangée de difficulté, 2 second geste, 4 fuite
//! d'arrivée) et 6 (pas de côté et sauts).

use super::stance::stance_begin;
use super::*;
use crate::objects::Courant;

pub(super) fn run(x: &mut Cx) {
    if !x.in_arena(8) {
        // hors de l'écran : il revient par l'arrivée de la variante 2
        set_state_long(x, 0x0200_0000);
        return arrival::walk_in(x, 0x20, 0x17);
    }
    x.c.d[0] = x.u8(f::SUB2) as u32;
    match x.u8(f::SUB2) {
        0 => first(x),
        2 => second(x),
        4 => {
            x.set16(RESUME_STATE, 0x0204);
            x.set16(RESUME_SUB2, 0x0202);
            set_state_long(x, 0x020c_0000);
            turn_to_resume(x);
        }
        _ => {}
    }
}

/// 0x5c8aa : le demi-tour du sous-état 12.
fn turn_to_resume(x: &mut Cx) {
    x.add8(f::SUB2, 2);
    mv(x, 3);
}

// --- premier geste ------------------------------------------------------------------------------------

fn first(x: &mut Cx) {
    x.c.d[0] = x.u8(STEP) as u32;
    match x.u8(STEP) {
        0 => {
            let action = row_byte(x, FIRST_ACTIONS);
            x.c.d[0] = x.c.d[0] & !0xff | action as u32;
            match action {
                0 => shoot_or_approach(x),
                2 => {
                    x.set8(SAVED_STEP, 2);
                    x.set8(TIMER, 0x20);
                    x.set8(ANGLE, 8);
                    if x.u8(f::FLIP) == 0 {
                        x.set8(ANGLE, 0x1c);
                    }
                    x.set8(STEP, 8);
                    set_speed(x, 0xd);
                    mv(x, 2);
                }
                4 => {
                    x.c.d[6] = 3;
                    let d6 = if rand_and(x, 1) != 0 { 3 } else { 2 };
                    x.c.d[6] = d6;
                    x.set8(AI_WANDER, d6 as u8);
                    set_speed(x, 0x21);
                    x.set8(STEP, 0xa);
                    reload_shot(x);
                }
                6 => {
                    set_state_long(x, STANCE);
                    stance_begin(x);
                }
                8 => {
                    x.set8(REPEAT, 1);
                    x.set8(STEP, 4);
                    mv(x, 1);
                }
                _ => {}
            }
        }
        2 => aim_again(x),
        4 => fire_loop(x),
        6 => side_walk(x),
        8 => {
            let n = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, n);
            if (n as i8) < 0 {
                let saved = x.u8(SAVED_STEP);
                x.set8(STEP, saved);
            }
            x.move_by_table_xy();
            x.anim_step();
        }
        10 => {
            if x.anim_step() {
                x.set8(STEP, 6);
                side_walk(x);
            }
        }
        _ => {}
    }
}

/// 0x5c37a : au tirage, un tir sur place, sinon rapprochement (type de tir 4).
fn shoot_or_approach(x: &mut Cx) {
    x.set8(STEP, 2);
    if shoot_roll(x) {
        return aim_again(x);
    }
    x.set8(SHOT_KIND, 4);
    approach_target(x);
}

/// 0x5c44e : un tir sur place : nombre de tirs tiré dans la rangée, puis animation de tir.
fn aim_again(x: &mut Cx) {
    x.add8(STEP, 2);
    let n = row_byte(x, SHOT_REPEATS);
    x.set8(REPEAT, n);
    reload_shot(x);
}

/// 0x5c472 : l'animation de tir (0 ou 5 selon le profil `+0x2f`).
fn reload_shot(x: &mut Cx) {
    let anim = if x.u8(GROUP) != 0 { 5 } else { 0 };
    x.c.d[0] = anim;
    shot(x, anim);
}

/// 0x5c418 : s'oriente vers la cible vivante, ou se met en attente si elle est tombée.
fn approach_target(x: &mut Cx) {
    let target = load_target(x);
    if target.u8(x.b, 0) == 0 {
        set_state_long(x, IDLE);
        return;
    }
    set_state_long(x, 0x0202_0200);
    x.call(call::TURN_TO_TARGET);
    set_speed(x, 0xd);
    mv(x, 2);
}

/// +7 = 4 : la flèche part à la clé d'animation ; à la fin de l'animation, un tir de moins.
fn fire_loop(x: &mut Cx) {
    if !x.anim_step() {
        if x.u8(KEY) != 0 {
            x.set8(KEY, 0);
            x.call(call::SWING);
        }
        return;
    }
    let n = x.u8(REPEAT).wrapping_sub(1);
    x.set8(REPEAT, n);
    if n != 0 {
        return reload_shot(x);
    }
    set_state_long(x, 0x0208_0000);
}

/// +7 = 6 : pas de côté vers la profondeur de la cible, puis nouvelle visée.
fn side_walk(x: &mut Cx) {
    x.set8(ANGLE, 0);
    let target = load_target(x);
    let mine = x.u16(DEPTH);
    if mine >= target.u16(x.b, DEPTH) {
        x.add8(ANGLE, 0x10);
    }
    let n = x.u8(AI_WANDER).wrapping_sub(1);
    x.set8(AI_WANDER, n);
    if (n as i8) >= 0 {
        x.set8(TIMER, 0x20);
        let step = x.u8(STEP);
        x.set8(SAVED_STEP, step);
        x.add8(STEP, 2);
        let anim = 0x10 + x.u8(GROUP) as u32;
        return mv(x, anim);
    }
    let dx = x.u16(X).wrapping_sub(target.u16(x.b, X));
    let want = if (dx as i16) < 0 { 0x20 } else { 0 };
    x.c.d[1] = want;
    if want as u8 != x.u8(f::FLIP) {
        let state = x.u32(f::STATE);
        x.set32(RESUME_STATE, state);
        x.set8(f::SUB, 0xc);
        x.set16(f::SUB2, 0);
        return turn_to_resume(x);
    }
    x.set8(STEP, 2);
    aim_again(x);
}

// --- second geste (+6 = 2) ----------------------------------------------------------------------------

fn second(x: &mut Cx) {
    x.c.d[0] = x.u8(STEP) as u32;
    match x.u8(STEP) {
        0 => {
            let action = row_byte(x, SECOND_ACTIONS);
            x.c.d[0] = x.c.d[0] & !0xff | action as u32;
            match action {
                0 => {
                    x.set8(STEP, 2);
                    if shoot_roll(x) {
                        return start_shot(x);
                    }
                    x.set8(SHOT_KIND, 5);
                    approach_target(x);
                }
                2 => {
                    x.set8(SAVED_STEP, 2);
                    dash_start(x);
                }
                4 => {
                    x.set8(SAVED_STEP, 6);
                    dash_start(x);
                }
                6 => {
                    x.set8(STEP, 0xa);
                    x.set16(GRAVITY, -0x38i16 as u16);
                    jump_speeds(x);
                    mv(x, 8);
                }
                _ => {}
            }
        }
        2 => start_shot(x),
        4 => {
            if x.anim_step() {
                follow_up(x);
            }
        }
        6 => {
            set_state_long(x, STANCE);
            stance_begin(x);
        }
        8 => shoot_or_approach(x),
        10 => {
            x.move_xz();
            x.anim_step();
            if (x.u16(HEIGHT) as i16) >= 0 {
                return;
            }
            x.set16(HEIGHT, 0);
            x.add8(STEP, 2);
        }
        12 => {
            x.set8(SHOT_KIND, 4);
            approach_target(x);
        }
        14 => {
            let n = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, n);
            if (n as i8) < 0 {
                let saved = x.u8(SAVED_STEP);
                x.set8(STEP, saved);
            }
            x.move_by_table_xy();
            x.anim_step();
        }
        _ => {}
    }
}

/// Vitesses d'un saut : vers le haut 0x200, en avant 0x280 (inversé si l'orientation est non nulle).
fn jump_speeds(x: &mut Cx) {
    x.set16(SPEED_Z, 0x200);
    x.set16(SPEED_X, 0x280);
    x.set16(SPEED_Y, 0);
    if x.u8(f::FLIP) != 0 {
        negate(x, SPEED_X);
    }
}

/// 0x5c5f6 : élan latéral (vitesse 0xb, animation 2) pendant `0x23` images.
fn dash_start(x: &mut Cx) {
    x.set8(TIMER, 0x23);
    x.set8(ANGLE, 8);
    if x.u8(f::FLIP) != 0 {
        x.set8(ANGLE, 0x1c);
    }
    x.set8(STEP, 0xe);
    set_speed(x, 0xb);
    mv(x, 2);
}

/// 0x5c59e : un tir à bout portant (flèche de type 0xa, force 4).
pub(super) fn start_shot(x: &mut Cx) {
    x.add8(STEP, 2);
    x.set8(AI_SHOT, 3);
    x.set8(STRIKE_KIND, 0xa);
    x.set8(STRIKE_POWER, 4);
    x.set32(FLAGS, 0x0204_0202);
    shot(x, 1);
}

// --- sous-état 6 --------------------------------------------------------------------------------------

pub(super) fn run_b(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB2) as u32;
    match x.u8(f::SUB2) {
        0 => crouch(x),
        2 | 8 => sidestep(x),
        4 | 6 => leap(x),
        _ => {}
    }
}

/// 0x5c6e6… : fin d'un geste du sous-état 6 : debout, et au tirage, attente.
fn done_gesture(x: &mut Cx) {
    set_state_long(x, STANCE);
    x.call(call::RAND_TARGET_CHECK);
    if !x.c.cond(7) {
        x.set8(f::SUB, 0xa);
        x.set16(f::SUB2, 0);
        stance::check_facing(x);
    }
}

/// +6 = 0 : se baisse (animation 1 ou 0xb au hasard).
fn crouch(x: &mut Cx) {
    if x.u8(STEP) == 0 {
        x.add8(STEP, 2);
        let anim = if rand_and(x, 1) != 0 { 1 } else { 0xb };
        return mv(x, anim);
    }
    if x.anim_step() {
        done_gesture(x);
    }
}

/// +6 = 2 et 8 : pas de côté pendant `+0x6b` images.
fn sidestep(x: &mut Cx) {
    match x.u8(STEP) {
        0 => {
            x.add8(STEP, 2);
            x.set8(AI_WANDER, 0xc);
            x.set8(ANGLE, 0);
            if rand_and(x, 1) != 0 {
                x.add8(ANGLE, 0x10);
            }
            set_speed(x, 0xd);
            mv(x, 2);
        }
        2 => {
            x.move_by_table_xy();
            x.anim_step();
            let n = x.u8(AI_WANDER).wrapping_sub(1);
            x.set8(AI_WANDER, n);
            if (n as i8) >= 0 {
                return;
            }
            if x.u8(f::SUB2) == 8 {
                x.add8(STEP, 2);
                return;
            }
            done_gesture(x);
        }
        4 => {
            x.add8(STEP, 2);
            shot(x, 0);
        }
        6 => {
            if x.anim_step() {
                done_gesture(x);
            }
        }
        _ => {}
    }
}

/// +6 = 4 et 6 : saut avec élan (vitesse 0x280), retombée, puis tir (6).
fn leap(x: &mut Cx) {
    match x.u8(STEP) {
        0 => {
            x.add8(STEP, 2);
            x.set16(GRAVITY, -0x38i16 as u16);
            jump_speeds(x);
            mv(x, 8);
        }
        2 => {
            x.move_xz();
            x.anim_step();
            if (x.u32(HEIGHT) as i32) >= 0 {
                return;
            }
            x.set32(HEIGHT, 0);
            if x.u8(f::SUB2) == 6 {
                x.add8(STEP, 2);
                return;
            }
            done_gesture(x);
        }
        4 => {
            x.add8(STEP, 2);
            shot(x, 1);
        }
        6 => {
            if x.anim_step() {
                done_gesture(x);
            }
        }
        _ => {}
    }
}
