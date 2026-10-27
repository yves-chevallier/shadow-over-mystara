//! Boss 22, groupe 4 : les attaques (`+6` = 0 à 22). Chacune se termine par `next_attack` (0x9d680).

use super::moves::step_anim;
use super::*;
use crate::objects::Courant;

pub(super) fn run(x: &mut Cx) {
    let step = x.u8(f::SUB2);
    match (two(x), step) {
        (_, 0 | 6) => shot(x),
        (_, 2 | 8) => breath(x),
        (_, 4) => bite(x),
        (true, 10) => two::slam(x),
        (true, 12) => two::spin_roar(x),
        (true, 16) | (false, 10 | 16) => second_form_strike(x),
        (true, 18) | (false, 12 | 18) => charge_strike(x),
        (_, 14) => stomp(x),
        (_, 20) => dive(x),
        (_, 22) => dive_down(x),
        _ => {}
    }
}

/// 0x9b752 : image-clé de l'animation d'attaque (son 0xebfdc), puis fin quand l'animation est finie.
fn finish_attack(x: &mut Cx) {
    if x.u8(f::KEY_FRAME) != 0 {
        lib::sound(x, 0xebfdc);
        x.set8(f::KEY_FRAME, 0);
    }
    if x.anim_step() {
        next_attack(x);
    }
}

/// 0x9b712 : tir.
fn shot(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            next7(x);
            x.set8(VULNERABLE, 1);
            x.set8(ANIM_ID, 0xa);
            sounds(x, 0x28, 5);
            x.set8(ANIM_LAST, 0xa);
            x.c.d[0] = 0;
            start(x, Set::B, 0);
        }
        2 => finish_attack(x),
        _ => {}
    }
}

/// 0x9b774 : souffle.
fn breath(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            next7(x);
            if two(x) {
                x.set8(VULNERABLE, 1);
            }
            x.set8(ANIM_ID, 0xe);
            sounds(x, 0x29, 6);
            x.set8(ANIM_LAST, 0xe);
            start(x, Set::B, 4);
        }
        2 => finish_attack(x),
        _ => {}
    }
}

/// 0x9bfb4 : image-clé de la morsure : saisit un joueur à portée (0xfc44, 0xfd64), sinon fin de l'animation.
fn grab_on_key(x: &mut Cx) {
    if x.u8(f::KEY_FRAME) != 0 {
        if let Some(player) = lib::attack_hits_a_player(x, 5) {
            return lib::seize_player(x, player, 0);
        }
    }
    if x.anim_step() {
        next_attack(x);
    }
}

/// 0x9bf80 : morsure.
fn bite(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            next7(x);
            x.set8(VULNERABLE, 1);
            x.set8(ANIM_ID, 0xc);
            x.set8(ANIM_LAST, 0xc);
            start(x, Set::B, 3);
        }
        2 => grab_on_key(x),
        _ => {}
    }
}

/// Fin de la course d'une attaque de la seconde forme : le boss se tourne vers sa cible s'il lui tourne le dos.
fn facing_wrong(x: &mut Cx) -> bool {
    if x.u16(f::TARGET) == 0 {
        return false;
    }
    let t = x.linked(f::TARGET);
    let wanted = if (x.u16(0x8).wrapping_sub(t.u16(x.b, 0x8)) as i16) < 0 { 0x20 } else { 0 };
    wanted != x.u8(f::FLIP)
}

/// 0x9c0ac : attaque de la seconde forme (rugissement qui la déclenche, course, coup).
fn second_form_strike(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            if second_form(x) {
                return x.add8(0x7, 4);
            }
            x.set8(VULNERABLE, 1);
            next7(x);
            x.set8(ANIM_ID, 0x11);
            x.set8(ANIM_LAST, 0x11);
            sounds(x, 0x27, 5);
            start(x, Set::B, 7);
        }
        2 => {
            if x.u8(f::KEY_FRAME) != 0 {
                x.set8(f::KEY_FRAME, 0);
                eshl_wave_1f(x);
                x.set8(SECOND, 1);
                x.set8(SECOND_FX, 1);
            }
            if x.anim_step() {
                next7(x);
            }
        }
        4 => {
            if !second_form(x) {
                x.set8(SECOND_FX, 0);
                return next_attack(x);
            }
            next7(x);
            x.set8(VULNERABLE, 1);
            x.set8(ANIM_ID, 5);
            if x.u8(ANIM_ID) != x.u8(ANIM_LAST) {
                x.set8(ANIM_LAST, 5);
                start(x, Set::A, 0xc);
            }
            speed(x, 0x15);
        }
        6 => {
            rubble(x, 0, 0x201, 0x20, 0x20);
            if !second_form(x) {
                x.set8(SECOND_FX, 0);
                return next_attack(x);
            }
            x.set8(0x79, 0x23);
            lib::approach_target(x);
            if sub8(x, WALK_DELAY, 1).mi() {
                lib::draw_stance(x);
                let r = rand_and(x, 0xf);
                if 0xffc0u32 >> r & 1 != 0 {
                    lib::pick_target(x);
                } else {
                    lib::random_player(x);
                }
                if sub8(x, WALK_STEPS, 1).eq() {
                    lib::draw_6b(x);
                }
            }
            step_anim(x);
            x.move_by_table_xy();
            if facing_wrong(x) {
                next7(x);
            }
        }
        8 => {
            if !second_form(x) {
                x.set8(SECOND_FX, 0);
                return next_attack(x);
            }
            next7(x);
            x.set8(VULNERABLE, 1);
            x.set8(ANIM_ID, 5);
            let angle = if x.u8(f::FLIP) != 0 { 8 } else { 0x18 };
            x.set8(0x2d, angle);
            if x.u8(ANIM_ID) != x.u8(ANIM_LAST) {
                x.set8(ANIM_LAST, 5);
                start(x, Set::A, 0xc);
            }
            speed(x, 0x15);
        }
        10 => {
            if !second_form(x) {
                x.set8(SECOND_FX, 0);
                return next_attack(x);
            }
            if x.u16(f::TARGET) != 0 && still_close(x) {
                step_anim(x);
                x.move_by_table_xy();
                return;
            }
            next7(x);
            x.set8(VULNERABLE, 1);
            start(x, Set::A, 0xf);
        }
        12 => {
            if !second_form(x) {
                x.set8(SECOND_FX, 0);
                return next_attack(x);
            }
            if x.anim_step() {
                x.set8(0x7, 4);
                let side = x.u8(f::FLIP) ^ 0x20;
                x.set8(f::FLIP, side);
                x.set8(ANIM_ID, 5);
                x.set8(ANIM_LAST, 5);
                start(x, Set::A, 0xc);
            }
        }
        _ => {}
    }
}

/// 0x9c29a : le boss reste dans l'écran et la cible est assez proche devant lui (comme au demi-tour du combat).
fn still_close(x: &mut Cx) -> bool {
    let cam = x.a5_u16(var::SCROLL_X);
    if x.u8(f::FLIP) != 0 && (cam.wrapping_add(0x160).wrapping_sub(x.u16(0x8)) as i16) < 0 {
        return false;
    }
    if (cam.wrapping_add(0x20).wrapping_sub(x.u16(0x8)) as i16) >= 0 {
        return false;
    }
    let t = x.linked(f::TARGET);
    let d = t.u16(x.b, 0x8).wrapping_sub(x.u16(0x8));
    d <= 0xe0 && x.u8(0x29) == 0
}

/// 0x9b7ae : charge (course vers la cible, coup à portée).
fn charge_strike(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            next7(x);
            x.set16(TIMER, 0x32);
            charge_start(x);
        }
        6 => {
            x.set8(0x7, 2);
            charge_start(x);
        }
        2 => {
            rubble(x, 0, 0x201, 0x20, 0x20);
            x.set8(0x79, 0x25);
            lib::approach_target(x);
            if lib::target_in_box(x, -0x50, 0x30, 0x10) {
                next7(x);
                sounds(x, 0x29, 6);
                return start(x, Set::B, 4);
            }
            if lib::target_in_box(x, -0x10, 0x10, 0x10) || sub16(x, TIMER, 1).mi() {
                return next_attack(x);
            }
            if needs_turn(x) {
                x.set32(SAVED_STATE, 0x0204_1206);
                let step = x.u8(f::SUB2);
                x.set8(SAVED_STEP, step);
                x.set32(f::STATE, state::TURN);
            }
            step_anim(x);
            x.move_by_table_xy();
        }
        4 => finish_attack(x),
        _ => {}
    }
}

fn charge_start(x: &mut Cx) {
    x.set8(VULNERABLE, 1);
    x.set8(ANIM_ID, 0xf);
    x.set8(ANIM_LAST, 0xf);
    sounds(x, 0x27, 5);
    start(x, Set::A, 2);
    speed(x, 0x15);
}

/// 0x9bfe0 : piétinement (course vers un joueur proche, puis morsure).
fn stomp(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            next7(x);
            x.set16(TIMER, 0x3c);
            stomp_start(x);
        }
        6 => {
            x.set8(0x7, 2);
            stomp_start(x);
        }
        2 => {
            rubble(x, 0, 0x201, 0x20, 0x20);
            x.set8(0x79, 0x25);
            lib::approach_target(x);
            if player_within_reach(x) {
                next7(x);
                return start(x, Set::B, 3);
            }
            if sub16(x, TIMER, 1).mi() {
                return next_attack(x);
            }
            if needs_turn(x) {
                x.set32(SAVED_STATE, 0x0204_0e06);
                let step = x.u8(f::SUB2);
                x.set8(SAVED_STEP, step);
                x.set32(f::STATE, state::TURN);
            }
            step_anim(x);
            x.move_by_table_xy();
        }
        4 => grab_on_key(x),
        _ => {}
    }
}

fn stomp_start(x: &mut Cx) {
    x.set8(VULNERABLE, 1);
    x.set8(ANIM_ID, 0xd);
    x.set8(ANIM_LAST, 0xd);
    start(x, Set::A, 9);
    speed(x, 0x15);
}

/// 0x9c354 : plongeon sur la cible.
fn dive(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            if x.u16(f::TARGET) != 0 {
                next7(x);
                x.set8(VULNERABLE, 1);
                let t = x.linked(f::TARGET);
                x.c.a[1] = t.0;
                x.c.d[1] = t.u16(x.b, 0x8) as u32;
                x.c.d[2] = t.u16(x.b, 0x10) as u32;
                x.call(0x137e);
                let angle = x.c.d[0] as u8;
                x.set8(0x2d, angle);
                let v = if x.u8(f::FLIP) != 0 { 0x500u16 } else { (-0x500i16) as u16 };
                x.set16(0x14, v);
                x.set16(0x18, 0x900);
                x.set16(0x1a, (-0x50i16) as u16);
            }
            x.set8(ANIM_ID, 0x11);
            x.set8(ANIM_LAST, 0x11);
            sounds(x, 0x2a, 6);
            start(x, Set::B, 8);
        }
        2 => rise_wait(x),
        4 => climb(x, true),
        6 => climb(x, false),
        8 => land(x, false),
        10 => {
            if x.anim_step() {
                x.set8(VULNERABLE, 1);
                next_attack(x);
            }
        }
        _ => {}
    }
}

fn rise_wait(x: &mut Cx) {
    if x.u8(f::KEY_FRAME) != 0 {
        next7(x);
        x.set8(VULNERABLE, 0);
    }
    step_anim(x);
}

fn climb(x: &mut Cx, up: bool) {
    x.move_xz();
    let vz = x.u16(0x18);
    let reached = if up { Fl::sub16(vz, 0x100).gt() } else { Fl::sub16(vz, 0xff00).gt() };
    if !reached {
        next7(x);
        x.set16(0x30, 1);
    }
    step_anim(x);
}

/// Retombée du plongeon : son, poussière en ligne ; `down` (0x9c4a2) ajoute cinq éclats et ne rend pas vulnérable.
fn land(x: &mut Cx, down: bool) {
    x.move_xz();
    if (x.u32(0xc) as i32) > 0 {
        return step_anim(x);
    }
    lib::shake_54(x, 0x1e);
    x.call(if down { 0xecce6 } else { 0xecd50 });
    x.set32(0xc, 0);
    dust_line(x);
    next7(x);
    x.set16(0x30, 1);
    if down {
        scatter(x);
    } else {
        x.set8(VULNERABLE, 1);
    }
    start(x, Set::A, 6);
}

/// 0x9c3ee : saut sur place avec retombée en poussière.
fn dive_down(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            next7(x);
            x.set8(VULNERABLE, 1);
            x.set16(0x18, 0xb00);
            x.set16(0x1a, (-0x70i16) as u16);
            x.set16(0x14, 0);
            x.set16(0x16, 0);
            sounds(x, 0x2a, 5);
            x.set8(ANIM_ID, 0x13);
            x.set8(ANIM_LAST, 0x13);
            start(x, Set::B, 0xa);
        }
        2 => {
            if x.u8(f::KEY_FRAME) != 0 {
                next7(x);
                x.set8(VULNERABLE, 0);
            }
            step_anim(x);
        }
        4 => climb(x, true),
        6 => climb(x, false),
        8 => land(x, true),
        10 => {
            if x.anim_step() {
                x.set8(0, 1);
                x.set8(VULNERABLE, 1);
                next_attack(x);
            }
        }
        _ => {}
    }
}
