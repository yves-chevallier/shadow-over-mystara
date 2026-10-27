//! Type 0, sous-état 0 : arrivée. `+6` : 0 et 2 hurlement, 4 bond d'entrée, 6 et 8 saut périlleux (variantes 12 et 13),
//! 10 chute d'en haut, 12 apparition, 14 course d'arrivée, 16 et 20 réveil, 18 course vers le joueur.

use super::*;
use crate::objects::Courant;
use crate::objects::Obj;

pub(super) fn run(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB2) as u32;
    match x.u8(f::SUB2) {
        0 => howl(x),
        2 => growl(x),
        4 => leap_in(x),
        6 => somersault(x, 12, 0x140, 0xd),
        8 => somersault(x, 13, -0x140, 0xe),
        10 => drop_in(x),
        12 => appear(x),
        14 => run_in(x),
        16 => wake(x),
        18 => dash(x),
        20 => get_up(x),
        _ => {}
    }
}

/// Le joueur à portée : le geste est tiré au hasard entre deux animations (`a` si le tirage est pair, `b` sinon).
fn pick_anim(x: &mut Cx, a: u8, b: u8) {
    let r = rand_and(x, 1);
    x.add8(f::SUB3, 2);
    x.set8(VULNERABLE, 1);
    let n = if r == 0 { a } else { b };
    x.set8(LOOK, n);
    mv(x, n as u32);
}

/// Attend un joueur dans la boîte devant l'ennemi, anime l'attente ; vrai quand il est là.
fn player_ahead(x: &mut Cx) -> bool {
    if !pick_target(x) {
        return false;
    }
    x.anim_step();
    lib::target_in_box(x, 0, 0x80, 0x20)
}

/// +6 = 0 : hurlement, puis démarche vers la cible (animations 4 ou 3).
fn howl(x: &mut Cx) {
    if grabbed(x) {
        return set_state_long(x, HELD);
    }
    x.c.d[0] = x.u8(f::SUB3) as u32;
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            look(x, 0x17);
            mv(x, 0x17);
        }
        2 => {
            if player_ahead(x) {
                pick_anim(x, 4, 3);
            }
        }
        4 => stand_when_done(x),
        _ => {}
    }
}

/// +6 = 2 : comme 0 sans le hurlement ; animations 2 ou 1.
fn growl(x: &mut Cx) {
    if grabbed(x) {
        return set_state_long(x, HELD);
    }
    x.c.d[0] = x.u8(f::SUB3) as u32;
    match x.u8(f::SUB3) {
        0 => {
            if player_ahead(x) {
                pick_anim(x, 2, 1);
            }
        }
        2 => stand_when_done(x),
        _ => {}
    }
}

/// +6 = 4 : bond d'entrée.
fn leap_in(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB3) as u32;
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            x.set8(VULNERABLE, 1);
            look(x, 6);
            x.set16(Obj::VX, -0x3c0i16 as u16);
            if x.u8(f::FLIP) != 0 {
                negate(x, Obj::VX);
            }
            x.set16(Obj::AX, 0);
            x.set16(Obj::VZ, 0x4c0);
            x.set16(Obj::AZ, -0x38i16 as u16);
            mv(x, 6);
        }
        2 => wait_key(x),
        4 => {
            if x.move_xz().gt() {
                x.anim_step();
                return;
            }
            x.set32(Obj::Z, 0);
            x.add8(f::SUB3, 2);
            x.set8(VULNERABLE, 1);
            look(x, 0x18);
            mv(x, 0x18);
        }
        6 => stand_when_done(x),
        _ => {}
    }
}

/// +6 = 6 et 8 : saut périlleux, la clé d'animation déclenche l'effet (0xeb92e ou 0xeb9a0) ; `d3` : variante
/// qui l'accompagne, `vx2` la vitesse de côté.
fn somersault(x: &mut Cx, variant: u8, vx2: i16, anim: u32) {
    x.c.d[0] = x.u8(f::SUB3) as u32;
    match x.u8(f::SUB3) {
        0 => {
            x.set16(Obj::VY, vx2 as u16);
            x.add8(f::SUB3, 2);
            x.set8(VULNERABLE, 1);
            look(x, anim as u8);
            x.set16(Obj::AY, 0);
            x.set16(Obj::VZ, 0x500);
            x.set16(Obj::AZ, -0x38i16 as u16);
            mv(x, anim);
        }
        2 => {
            if x.u8(f::KEY_FRAME) != 0 {
                let special = matches!(var::SCENE.get(x.b), 6 | 0xc);
                if x.u8(Obj::VARIANT) == variant && special {
                    lib::sound(x, 0xeb92e);
                } else if !special {
                    lib::sound(x, 0xeb9a0);
                }
                x.add8(f::SUB3, 2);
                x.set8(VULNERABLE, 0);
            }
            x.anim_step();
        }
        4 => {
            if x.move_yz().gt() {
                x.anim_step();
                return;
            }
            x.set8(0, 1);
            x.set32(Obj::Z, 0);
            x.add8(f::SUB3, 2);
            x.set8(VULNERABLE, 1);
            look(x, 0x18);
            mv(x, 0x18);
        }
        6 => stand_when_done(x),
        _ => {}
    }
}

/// +6 = 10 : chute d'en haut.
fn drop_in(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB3) as u32;
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            x.set8(VULNERABLE, 1);
            x.set8(LOOK, 0x19);
            x.set16(Obj::VZ, -0x300i16 as u16);
            x.set16(Obj::AZ, -0x10i16 as u16);
            let h = var::SCROLL_Y.get(x.b).wrapping_add(0x100).wrapping_sub(x.u16(Obj::Y));
            x.set16(Obj::Z, h);
            mv(x, 0x19);
        }
        2 => {
            x.move_z();
            if (x.u16(Obj::Z) as i16) >= 0 {
                return;
            }
            x.add8(f::SUB3, 2);
            x.set32(Obj::Z, 0);
            mv(x, 0x18);
        }
        4 => stand_when_done(x),
        _ => {}
    }
}

/// +6 = 12 : apparition.
fn appear(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB3) as u32;
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            x.set8(VULNERABLE, 1);
            x.set8(LOOK, 0x1a);
            x.set16(TIMER_W, 0x400);
            mv(x, 0x1a);
        }
        2 => {
            let n = x.u8(TIMER_W).wrapping_sub(1);
            x.set8(TIMER_W, n);
            if n == 0 {
                x.add8(f::SUB3, 2);
                x.set8(LOOK, 0x1b);
                mv(x, 0x1b);
            } else {
                x.anim_step();
            }
        }
        4 => stand_when_done(x),
        _ => {}
    }
}

/// +6 = 14 : course d'arrivée (0x1e images).
fn run_in(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB3) as u32;
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            x.set16(TIMER, 0x1e);
            x.set8(VULNERABLE, 1);
            set_angle(x);
            mv(x, 0x16);
            set_speed(x, 0x17);
        }
        2 => {
            let n = x.u16(TIMER).wrapping_sub(1);
            x.set16(TIMER, n);
            if (n as i16) < 0 {
                set_state_long(x, STANCE);
            }
            x.anim_step();
            x.move_by_table_xy();
        }
        _ => {}
    }
}

/// +6 = 16 : attente d'un nombre d'images (`+0x42`) puis démarche.
fn wake(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB3) as u32;
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            look(x, 0x17);
            let t = x.u16(0x42);
            x.set16(TIMER, t);
            mv(x, 0x17);
        }
        2 => {
            let n = x.u16(TIMER).wrapping_sub(1);
            x.set16(TIMER, n);
            if (n as i16) >= 0 {
                return;
            }
            x.anim_step();
            pick_anim(x, 4, 3);
        }
        4 => stand_when_done(x),
        _ => {}
    }
}

/// +6 = 18 : course vers le joueur sur une durée tirée au hasard, puis grande attaque.
fn dash(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB3) as u32;
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            let r = rand_and(x, 0xf);
            let t = x.rom16(0x553b4 + 2 * r);
            x.set16(TIMER, t);
            set_angle(x);
            mv(x, 0xc);
            set_speed(x, 0x17);
        }
        2 => {
            let n = x.u16(TIMER).wrapping_sub(1);
            x.set16(TIMER, n);
            if (n as i16) >= 0 {
                x.anim_step();
                x.move_by_table_xy();
                return;
            }
            x.add8(f::SUB3, 2);
            x.anim_start(SET_STRIKE, 6);
        }
        4 => landing_hit(x, 0x9dda8),
        6 => {
            if x.anim_step() {
                x.set32(f::STATE, 0x0408_0000);
                let flip = x.u8(f::FLIP) ^ 0x20;
                x.set8(f::FLIP, flip);
                mv(x, 0);
            }
        }
        _ => {}
    }
}

/// +6 = 20 : comme 18, avec un autre effet et une reprise différente.
fn get_up(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB3) as u32;
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            let r = rand_and(x, 0xf);
            let t = x.rom16(0x553b4 + 2 * r);
            x.set16(TIMER, t);
            set_angle(x);
            mv(x, 0xc);
            set_speed(x, 0x17);
        }
        2 => {
            let n = x.u16(TIMER).wrapping_sub(1);
            x.set16(TIMER, n);
            if (n as i16) >= 0 {
                x.anim_step();
                x.move_by_table_xy();
                return;
            }
            x.add8(f::SUB3, 2);
            x.anim_start(SET_STRIKE, 6);
        }
        4 => landing_hit(x, 0x9e062),
        6 => {
            if x.anim_step() {
                x.set32(f::STATE, 0x0408_0000);
                let flip = x.u8(f::FLIP) ^ 0x20;
                x.set8(f::FLIP, flip);
                mv(x, 0);
            }
        }
        _ => {}
    }
}

/// Clé d'animation : effet `effect` ; en fin de script, le Kobold reprend sa marche un pas en avant (`−0x20`
/// à droite, `+0x20` à gauche).
fn landing_hit(x: &mut Cx, effect: u32) {
    if x.u8(f::KEY_FRAME) != 0 {
        x.set8(f::KEY_FRAME, 0);
        x.call(effect);
    }
    if !x.anim_step() {
        return;
    }
    mv(x, 0);
    let dx: u16 = if x.u8(f::FLIP) == 0 { 0xffe0 } else { 0x20 };
    x.add16(Obj::X, dx);
    x.add8(f::SUB3, 2);
    mv(x, 5);
}
