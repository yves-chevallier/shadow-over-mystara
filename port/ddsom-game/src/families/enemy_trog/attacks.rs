//! Type 2, coups. `+5 = 4` : `+6` = 0, 2, 4 combinaisons de coups, 6 double coup, 8, 10 et 18 bond-coup,
//! 12 estoc répété, 14 jet, 16 course ou bond, 20 attaque spéciale, 22 annonce, 24 demi-tour, 26 geste long.
//! `+5 = 6` : gardes et bonds. `+5 = 8` : coups de repli. `+7` est l'étape.

use crate::families::cx::Banc;
use super::*;
use crate::objects::Courant;
use crate::objects::Obj;

/// Poussée d'esprit inconnue de la ROM : lecture de la table de coups d'un bond, par difficulté.
fn pick_jump_move(x: &mut Cx) {
    let row = difficulty_row(x);
    x.c.d[2] = x.c.d[2] & 0xffff_0000 | row;
    let mv = pick_byte(x, JUMP_MOVES);
    x.set8(f::SUB2, mv);
    x.set8(f::SUB3, 0);
}

// --- sous-état 4 --------------------------------------------------------------------------------

pub(super) fn run(x: &mut Cx) {
    if x.u8(BLINK_PHASE) != 0 && (x.u8(BLINK_PHASE) as i8) > 0 {
        let n = x.u16(BLINK_COUNT).wrapping_sub(1);
        x.set16(BLINK_COUNT, n);
        if (n as i16) < 0 {
            x.set8(BLINK_ON, 0);
            x.set8(BLINK_WAIT, 0);
            x.set8(BLINK_PHASE, 0xff);
            x.set_bit(1, 1, false);
        }
    }
    match x.u8(f::SUB2) {
        id @ (0 | 2 | 4) => combo(x, id),
        6 => double(x),
        id @ (8 | 10 | 18) => leap_strike(x, id),
        12 => stab(x),
        14 => throw(x),
        16 => dash(x),
        20 => special(x),
        22 => approach(x),
        24 => turn_around(x),
        26 => long_gesture(x),
        _ => {}
    }
}

/// 0, 2 ou 4 : une série de coups d'animation 15fabe (nombre de répétitions `+0xa2`).
fn combo(x: &mut Cx, id: u8) {
    if x.u8(f::SUB3) != 0 {
        if step(x) {
            let n = x.u8(REPEAT).wrapping_sub(1);
            x.set8(REPEAT, n);
            if (n as i8) < 0 {
                return end_attack(x);
            }
            strike(x, 0);
        }
        return;
    }
    x.add8(f::SUB3, 2);
    let look = x.u8(LOOK);
    x.set8(LOOK_PREV, look);
    x.set8(REPEAT, 0);
    let mut d3 = 0;
    x.set8(LOOK, 0xff);
    let mut anim = id as u32;
    match id {
        2 => {}
        0 => {
            x.set8(LOOK, 0xfe);
        }
        _ => {
            x.set8(AI_KIND, 3);
            x.set8(LOOK, 0xfd);
            d3 = 2;
            let r = rand_and(x, 1);
            d3 += r;
            anim = r;
        }
    }
    x.set8(STRIKE_KIND, 0x15);
    x.set8(REPEAT, d3 as u8);
    x.set8(STRIKE_POWER, 5);
    strike(x, anim);
}

/// 6 : double coup.
fn double(x: &mut Cx) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(AI_KIND, 3);
        x.set8(REPEAT, 1);
        let r = rand_and(x, 1);
        x.add8(REPEAT, r as u8);
        x.set8(STRIKE_KIND, 0x15);
        x.set8(STRIKE_POWER, 5);
        return strike(x, 0);
    }
    if step(x) {
        let n = x.u8(REPEAT).wrapping_sub(1);
        x.set8(REPEAT, n);
        if (n as i8) < 0 {
            return end_attack(x);
        }
        x.set8(STRIKE_KIND, 0x16);
        x.set8(STRIKE_POWER, 5);
        strike(x, 1);
    }
}

/// 8, 10 et 18 : bond sur la cible avec un coup (8 et 18 enchaînent le coup suivant, 10 rejoue un bond).
fn leap_strike(x: &mut Cx, id: u8) {
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            x.set16(Obj::VX, 0);
            let target = load_target(x);
            if target.u8(x.b, 0) == 0 {
                set_state_long(x, IDLE);
                return rest_begin(x);
            }
            let look = x.u8(LOOK);
            x.set8(LOOK_PREV, look);
            x.set8(LOOK, 0xf0);
            x.set16(Obj::VZ, 0x680);
            x.set16(Obj::AZ, -0x38i16 as u16);
            x.set16(Obj::AX, 0);
            let dx = target.u16(x.b, Obj::X).wrapping_sub(x.u16(Obj::X));
            x.set16(Obj::VX, dx << 2);
            x.set8(STRIKE_KIND, 0x17);
            let mut away = x.u16(Obj::X).wrapping_sub(target.u16(x.b, Obj::X));
            if (away as i16) < 0 {
                away = away.wrapping_neg();
            }
            if away > 0x70 {
                x.set8(STRIKE_KIND, 0x18);
            }
            x.set8(STRIKE_POWER, 5);
            strike(x, 2);
        }
        2 => land_with(x),
        4 => {
            if !step(x) {
                return;
            }
            if id != 10 {
                return end_attack(x);
            }
            let r = rand_and(x, 2);
            let look = x.u8(LOOK);
            x.set8(LOOK_PREV, look);
            x.set8(f::SUB2, r as u8);
            x.set8(f::SUB3, 0);
        }
        _ => {}
    }
}

/// Montée et chute d'un bond, puis animation de réception (0xc).
fn land_with<R: Banc>(x: &mut Cx<'_, R>) {
    x.move_xz();
    if step(x) && (x.u16(Obj::Z) as i16) < 0 {
        x.add8(f::SUB3, 2);
        x.set16(Obj::Z, 0);
        body(x, 0xc);
    }
}

/// 12 : estoc répété `+0xa2` fois (animation 5).
fn stab(x: &mut Cx) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(AI_KIND, 2);
        let row = difficulty_row(x);
        let n = rand_and(x, 0x1f);
        let v = x.data8(REPEATS_TABLE + row + n);
        x.set8(REPEAT, v);
        return stab_begin(x);
    }
    if step(x) {
        let n = x.u8(REPEAT).wrapping_sub(1);
        x.set8(REPEAT, n);
        if (n as i8) >= 0 {
            return stab_begin(x);
        }
        end_attack(x);
    }
}

fn stab_begin(x: &mut Cx) {
    x.set8(STRIKE_KIND, 0x1a);
    x.set8(STRIKE_POWER, 5);
    strike(x, 5);
}

/// 14 : jet (animation 8 répétée), avec la routine 0x9e194 qui peut interrompre le geste.
fn throw(x: &mut Cx) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        if crate::families::eshl_creators::four_3(&mut crate::families::effects_creators::restes::Pur, x.b, x.o) {
            x.set8(REPEAT, 6);
            return body(x, 8);
        }
        set_state_long(x, IDLE);
        return rest_begin(x);
    }
    if step(x) {
        let n = x.u8(REPEAT).wrapping_sub(1);
        x.set8(REPEAT, n);
        if (n as i8) >= 0 {
            body(x, 8);
        } else {
            pick_jump_move(x);
        }
    }
}

/// 16 : course vers la cible (animation 2) ou bond.
fn dash(x: &mut Cx) {
    match x.u8(f::SUB3) {
        0 => {
            if rand_and(x, 1) == 0 {
                return long_jump(x);
            }
            x.add8(f::SUB3, 4);
            x.set8(LOOK_PREV, 0);
            x.set8(REPEAT, 0x20);
            walk_speed(x, 4);
            set_angle(x, 0x18, 8);
            body(x, 2);
        }
        2 => long_jump(x),
        4 => {
            move_xy(x);
            step(x);
            let n = x.u8(REPEAT).wrapping_sub(1);
            x.set8(REPEAT, n);
            if (n as i8) < 0 {
                set_state_long(x, STANCE);
            }
        }
        6 => land_with(x),
        8 => {
            if step(x) {
                set_state_long(x, STANCE);
            }
        }
        _ => {}
    }
}

/// 0x5a5a4 : grand bond de côté.
fn long_jump<R: Banc>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB3, 6);
    x.set8(LOOK_PREV, 0xcc);
    x.set16(Obj::AZ, -0x38i16 as u16);
    x.set16(Obj::VZ, 0x250);
    x.set16(Obj::VX, 0x300);
    x.set16(Obj::AX, 0);
    x.set32(Obj::VY, 0);
    x.set8(Obj::SPEED_INDEX, 8);
    if x.u8(f::FLIP) != 0 {
        x.set8(Obj::SPEED_INDEX, 0x18);
        negate(x, Obj::VX);
    }
    body(x, 0xa);
}

/// 20 : geste spécial (animation 3), interrompu par 0x9e2a0.
fn special(x: &mut Cx) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(AI_KIND, 2);
        return strike(x, 3);
    }
    if step(x) {
        return end_attack(x);
    }
    if x.u8(f::KEY_FRAME) != 0 {
        if crate::families::eshl_creators::above_4(&mut crate::families::effects_creators::restes::Pur, x.b, x.o).is_none() {
            set_state_long(x, IDLE);
        }
    }
}

/// 22 : annonce du coup, puis passage au coup prévu `+0xa3` (ou demi-tour si la cible est derrière).
fn approach(x: &mut Cx) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(LOOK_PREV, 1);
        lib::pick_byte(x);
        if x.u8(COOLDOWN) == 0 {
            x.set8(f::SUB3, 0);
            let next = x.u8(RESUME);
            x.set8(f::SUB2, next);
        } else {
            x.set8(LOOK_PREV, 1);
        }
        return body(x, 0);
    }
    step(x);
    let n = x.u8(COOLDOWN).wrapping_sub(1);
    x.set8(COOLDOWN, n);
    if (n as i8) < 0 {
        x.set8(f::SUB3, 0);
        if needs_turn_around(x) {
            x.add8(f::SUB2, 2);
        } else {
            let next = x.u8(RESUME);
            x.set8(f::SUB2, next);
        }
    }
}

/// 24 : demi-tour, puis reprise du coup prévu.
fn turn_around<R: Banc>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(LOOK_PREV, 0xe);
        let flip = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
        return body(x, 1);
    }
    if step(x) {
        let next = x.u8(RESUME);
        x.set8(f::SUB2, next);
        x.set8(f::SUB3, 0);
    }
}

/// 26 : geste long (animation 8 répétée `+0xa2` fois).
fn long_gesture(x: &mut Cx) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(REPEAT, 6);
        x.set8(LOOK_PREV, 0xee);
        return body(x, 8);
    }
    if step(x) {
        let n = x.u8(REPEAT).wrapping_sub(1);
        x.set8(REPEAT, n);
        if (n as i8) >= 0 {
            body(x, 8);
        } else {
            end_attack(x);
        }
    }
}

// --- sous-état 6 : gardes et bonds --------------------------------------------------------------

pub(super) fn run_b(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 | 16 | 18 | 20 => hold(x),
        2 => guard(x),
        4 => side_hop(x),
        6 | 8 | 10 => forward_hop(x),
        12 => hop_strike(x),
        14 => throw(x),
        22 => approach(x),
        24 => turn_around(x),
        _ => {}
    }
}

/// Garde sur place (animation 8) jusqu'à la fin de l'animation, puis retour debout.
fn hold<R: Banc>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(LOOK_PREV, 0xee);
        x.set8(VULNERABLE, 1);
        return body(x, 8);
    }
    if step(x) {
        back_to_stance(x);
    }
}

/// 2 : garde en posture 9 pendant `0x3c` images.
fn guard<R: Banc>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(POSTURE, 9);
        x.set8(LOOK, 0xdf);
        x.set8(REPEAT, 0x3c);
        return body(x, 0xd);
    }
    let n = x.u8(REPEAT).wrapping_sub(1);
    x.set8(REPEAT, n);
    if n == 0 {
        x.set8(POSTURE, 4);
        back_to_stance(x);
    }
}

/// 4 : bond de côté vers la profondeur de la cible.
fn side_hop(x: &mut Cx) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(LOOK_PREV, 0xb1);
        x.set16(Obj::VZ, 0x600);
        x.set16(Obj::AZ, -0x38i16 as u16);
        x.set16(Obj::AY, 0);
        x.set16(Obj::VY, 0x80);
        let target = load_target(x);
        if target.u8(x.b, 0) == 0 {
            set_state_long(x, IDLE);
            return rest_begin(x);
        }
        let dy = target.u16(x.b, Obj::Y).wrapping_sub(x.u16(Obj::Y));
        if (dy as i16) < 0 {
            negate(x, Obj::VY);
        }
        return body(x, 0xa);
    }
    x.move_yz();
    if step(x) && (x.u16(Obj::Z) as i16) < 0 {
        x.set16(Obj::Z, 0);
        back_to_stance(x);
    }
}

/// 6, 8 et 10 : course vers l'avant (animation 3) ; à la fin, 6 retourne debout, 8 et 10 enchaînent un coup.
fn forward_hop(x: &mut Cx) {
    if x.u8(f::SUB3) == 0 {
        return forward_hop_begin(x);
    }
    move_xy(x);
    if !step(x) {
        return;
    }
    match x.u8(f::SUB2) {
        8 => set_state_long(x, 0x0204_0200),
        s if s > 8 => set_state_long(x, 0x0204_1400),
        _ => back_to_stance(x),
    }
}

/// 0x5a8c8.
pub(super) fn forward_hop_begin(x: &mut Cx) {
    x.add8(f::SUB3, 2);
    x.set8(LOOK_PREV, 0xb0);
    x.set8(Obj::SPEED_INDEX, 8);
    if x.u8(f::FLIP) != 0 {
        x.add8(Obj::SPEED_INDEX, 0x10);
    }
    walk_speed(x, 2);
    body(x, 3);
}

/// 12 : bond avec coup (animation 6 de la série des coups).
fn hop_strike(x: &mut Cx) {
    match x.u8(f::SUB3) {
        0 => hop_strike_begin(x),
        2 => {
            x.move_xz();
            step(x);
            let h = x.u16(Obj::Z) as i16;
            if h < 0 {
                x.add8(f::SUB3, 2);
                x.set16(Obj::Z, 0);
                body(x, 0xc);
            }
        }
        _ => {
            if step(x) {
                back_to_stance(x);
            }
        }
    }
}

/// 0x5a934.
pub(super) fn hop_strike_begin(x: &mut Cx) {
    x.add8(f::SUB3, 2);
    x.set8(LOOK_PREV, 0xb2);
    x.set16(Obj::VZ, 0x680);
    x.set16(Obj::AZ, -0x38i16 as u16);
    x.set16(Obj::AX, 0);
    x.set16(Obj::VX, 0x200);
    if x.u8(f::FLIP) != 0 {
        negate(x, Obj::VX);
    }
    x.set8(STRIKE_KIND, 0x16);
    x.set8(STRIKE_POWER, 5);
    strike(x, 6);
}

// --- sous-état 8 : coups de repli -----------------------------------------------------------------

pub(super) fn run_fallback(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            if x.u8(f::SUB3) == 0 {
                x.add8(f::SUB3, 2);
                x.set8(VULNERABLE, 1);
                x.set8(REPEAT, 0x10);
                x.set8(LOOK_PREV, 1);
                x.set8(LOOK, 1);
                return body(x, 0);
            }
            step(x);
            let n = x.u8(REPEAT).wrapping_sub(1);
            x.set8(REPEAT, n);
            if (n as i8) < 0 {
                set_state_long(x, STANCE);
            }
        }
        2 => {
            x.set8(f::SUB, 6);
            if x.u8(SERIES) == 0 {
                x.set8(f::SUB2, 6);
                forward_hop_begin(x);
            } else {
                x.set8(f::SUB2, 0xc);
                hop_strike_begin(x);
            }
        }
        4 => sidestep(x),
        6 => choose(x),
        _ => {}
    }
}

/// `+6` = 4 : pas de côté, soit course (étape 0), soit bond en profondeur.
fn sidestep(x: &mut Cx) {
    match x.u8(f::SUB3) {
        0 => {
            if rand_and(x, 1) == 0 {
                x.set8(REPEAT, 0x10);
                x.set8(Obj::SPEED_INDEX, 0);
                if x.u8(f::FLIP) == 0 {
                    x.set8(Obj::SPEED_INDEX, 0x10);
                }
                walk_speed(x, 4);
                return body(x, 2);
            }
            let target = load_target(x);
            let mut d0 = 0;
            if target.u8(x.b, 0) == 0 {
                set_state_long(x, IDLE);
            }
            let dy = target.u16(x.b, Obj::Y).wrapping_sub(x.u16(Obj::Y));
            if (dy as i16) < 0 {
                d0 += 2;
            }
            x.add8(f::SUB2, 2);
            x.set16(Obj::VZ, 0x600);
            x.set16(Obj::AZ, -0x38i16 as u16);
            x.set16(Obj::VY, 0x80);
            x.set16(Obj::AY, 0);
            if d0 != 0 {
                negate(x, Obj::VY);
            }
            body(x, 0xa);
        }
        2 => {
            move_xy(x);
            step(x);
            let n = x.u8(REPEAT).wrapping_sub(1);
            x.set8(REPEAT, n);
            if (n as i8) < 0 {
                set_state_long(x, STANCE);
            }
        }
        4 => {
            x.move_yz();
            if step(x) && (x.u16(Obj::Z) as i16) < 0 {
                x.set8(LOOK_PREV, 0xe0);
                x.set16(Obj::Z, 0);
                set_state_long(x, STANCE);
            }
        }
        6 => {
            if step(x) {
                set_state_long(x, STANCE);
            }
        }
        _ => {}
    }
}

/// `+6` = 6 : choisit le prochain coup selon la cible.
fn choose(x: &mut Cx) {
    x.set8(LOOK_PREV, 1);
    body(x, 0);
    if lib::target_in_box(x, -0x20, 0x20, 8) {
        set_state_long(x, 0x0204_1600);
        return x.set8(RESUME, 6);
    }
    if front_box(x) {
        return super::stance::pick_attack_close(x);
    }
    if side_box(x) {
        return super::stance::pick_attack_far(x);
    }
    set_state_long(x, STANCE);
}
