//! Type 27, sous-état 4 : les gestes de l'arme. `+6` = 0, 2 et 4 canon (animations 0 à 2 du jeu de l'arme), 6 charge
//! de projectiles, 8 bond d'écrasement, 10 salve, 12 lancer d'un objet ; sous-états 6, 8 et 10 : tourne sur place
//! quand plus aucun joueur n'est en vie.

use super::arrival::hop_loop;
use super::*;
use crate::objects::pools;
use crate::objects::Courant;
use crate::objects::Obj;

pub(super) fn run(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB2) as u32;
    match x.u8(f::SUB2) {
        0 => cannon(x, 0x10, 0),
        2 => cannon(x, 0x11, 1),
        4 => cannon(x, 0x12, 2),
        6 => rush(x),
        8 => smash(x),
        10 => volley(x),
        12 => grenade(x),
        _ => {}
    }
}

/// Fin de geste : à la fin de l'animation, retour à `fire_a`, ou attente s'il n'y a plus de joueur en vie.
fn end_of_animation(x: &mut Cx) {
    if lib::step(x) {
        end_gesture(x);
    }
}

fn end_gesture(x: &mut Cx) {
    if !lib::players_alive(x) {
        return go::idle(x);
    }
    go::fire_a(x);
}

/// +6 = 0, 2, 4 : une animation de l'arme (`look` = aspect, `anim` = numéro) jouée jusqu'au bout.
fn cannon(x: &mut Cx, look_v: u8, anim: u32) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        look(x, look_v);
        return weapon(x, anim);
    }
    end_of_animation(x);
}

/// +6 = 6 : charge vers la cible, étincelles pendant le roulement, puis bond.
fn rush(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB3) as u32;
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            x.set16(Obj::AX, 0);
            let mut angle = 0x18;
            if x.u8(f::FLIP) != 0 {
                angle = 8;
            }
            let target = load_target(x);
            let goal = target.u16(x.b, Obj::X);
            x.set16(GOAL_X, goal);
            x.set8(Obj::SPEED_INDEX, angle);
            x.select_speed(&crate::objects::motion::SPEEDS_B, 0x2b);
            spark(x, 1);
            lib::sound(x, 0xe3d32);
            look(x, 0x13);
            weapon(x, 3);
        }
        2 => {
            x.anim_step();
            x.move_x();
            if x.u8(f::KEY_FRAME) == 2 {
                x.set8(f::KEY_FRAME, 0);
                spark(x, 1);
                lib::shake_54(x, 0x10);
                lib::sound(x, 0xe3d32);
            }
            let (mut d0, mut d1) = (x.u16(GOAL_X), x.u16(Obj::X));
            if (x.u16(Obj::VX) as i16) < 0 {
                std::mem::swap(&mut d0, &mut d1);
            }
            if (d1 as i16) < (d0 as i16) || x.u8(f::KEY_FRAME) == 0 {
                return;
            }
            x.add8(f::SUB3, 2);
            x.set16(TIMER, 0x1e);
            x.set16(TIMER2, 0);
            x.set8(FLAG, 0);
            let v: i16 = if x.u8(f::FLIP) != 0 { -0x30 } else { 0x30 };
            x.set16(Obj::AX, v as u16);
            x.c.d[0] = v as u16 as u32;
            lib::sound(x, 0xe3d32);
            lib::sound(x, 0xecdba);
            mv(x, 9);
        }
        4 => hop_loop(x),
        6 => end_of_animation(x),
        _ => {}
    }
}

/// +6 = 8 : bond d'écrasement vers la cible, puis choc au sol.
fn smash(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB3) as u32;
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            x.set16(Obj::AX, 0);
            let angle = heading(x);
            x.set8(Obj::SPEED_INDEX, angle);
            set_speed(x, 0x17);
            // distance horizontale à la cible, divisée par la vitesse : détermine la poussée verticale
            let target = load_target(x);
            let mut d2 = x.u32(Obj::X) as i32 - target.u32(x.b, Obj::X) as i32;
            if d2 < 0 {
                d2 = d2.wrapping_neg();
            }
            let speed = (((x.u16(Obj::VX) as u32) << 16) as i32) >> 8;
            let speed = if speed < 0 { speed.wrapping_neg() } else { speed };
            let mut thrust: u16 = 0x400;
            if speed > 0 {
                loop {
                    d2 = d2.wrapping_sub(speed);
                    if d2 <= 0 {
                        break;
                    }
                    thrust = thrust.wrapping_add(0xa);
                }
            }
            x.set16(Obj::VZ, thrust);
            x.set16(Obj::AZ, -0x28i16 as u16);
            look(x, 0x14);
            weapon(x, 4);
        }
        2 => {
            lib::step(x);
            if x.u8(f::KEY_FRAME) != 0 {
                return;
            }
            x.move_xz();
            if (x.u16(Obj::VZ) as i16) > 0x100 {
                return;
            }
            x.add8(f::SUB3, 2);
            weapon(x, 5);
        }
        4 => {
            x.move_xz();
            if (x.u16(Obj::VZ) as i16) > -0x100 {
                return;
            }
            x.add8(f::SUB3, 2);
            x.set16(Obj::AZ, -0x40i16 as u16);
            weapon(x, 6);
        }
        6 => {
            x.move_xz();
            if (x.u16(Obj::Z) as i16) > 0 {
                return;
            }
            x.add8(f::SUB3, 2);
            x.set16(Obj::Z, 0);
            x.set16(TIMER, 0x14);
            lib::shake_58(x, 0x14);
            spark(x, 2);
            lib::sound(x, 0xecb98);
            weapon(x, 7);
        }
        8 => {
            let t = x.u16(TIMER).wrapping_sub(1);
            x.set16(TIMER, t);
            if (t as i16) > 0 {
                return;
            }
            end_gesture(x);
        }
        _ => {}
    }
}

/// +6 = 10 : salve : l'arme tire un nombre de fois lu dans la table 0x78856.
fn volley(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB3) as u32;
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            look(x, 0x15);
            weapon(x, 8);
        }
        2 => {
            lib::step(x);
            if x.u8(f::KEY_FRAME) == 0 {
                return;
            }
            x.add8(f::SUB3, 2);
            x.set8(FLAG, 0);
            let level = (var::RANK.get(x.b) & 0x1e) * 2;
            let players = var::PLAYERS_ALIVE.get(x.b);
            let rank = if players == 0 { 0 } else { players - 1 } as u16;
            let v = x.rom8(0x78856 + (level + rank) as u32);
            x.set8(COUNT, v);
        }
        4 => {
            lib::step(x);
            if x.u8(f::KEY_FRAME) == 0 {
                return;
            }
            let n = x.u8(COUNT).wrapping_sub(1);
            x.set8(COUNT, n);
            if (n as i8) > 0 {
                return;
            }
            x.add8(f::SUB3, 2);
            weapon(x, 9);
        }
        6 => {
            if lib::step(x) {
                if !lib::players_alive(x) {
                    return go::idle(x);
                }
                go::stand(x);
            }
        }
        _ => {}
    }
}

/// +6 = 12 : lance un objet (type 0x43) devant la machine, puis reste en place un moment.
fn grenade(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB3) as u32;
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            look(x, 0x16);
            weapon(x, 0xa);
        }
        2 => {
            if lib::step(x) {
                x.add8(f::SUB3, 2);
                x.set16(TIMER, 0x14);
                x.set8(VULNERABLE, 1);
                return mv(x, 4);
            }
            if x.u8(f::KEY_FRAME) == 0 {
                return;
            }
            x.set8(f::KEY_FRAME, 0);
            let Some(obj) = x.alloc(&pools::ESHL) else { return };
            obj.set_u8(x.b, 0, 1);
            obj.set_u8(x.b, 2, 0x43);
            let at = x.u16(Obj::X);
            obj.set_u16(x.b, Obj::X, at);
            let h = x.u16(Obj::Z);
            obj.set_u16(x.b, Obj::Z, h);
            let depth = x.u16(0x10);
            obj.set_u16(x.b, 0x10, depth);
            let flip = x.u8(f::FLIP);
            obj.set_u8(x.b, f::FLIP, flip);
            let target = x.u16(f::TARGET);
            obj.set_u16(x.b, f::TARGET, target);
            add16(x, obj, Obj::Z, 0x30);
            let shift: i16 = if flip != 0 { 0x60 } else { -0x60 };
            add16(x, obj, Obj::X, shift as u16);
        }
        4 => {
            let t = x.u16(TIMER).wrapping_sub(1);
            x.set16(TIMER, t);
            if (t as i16) > 0 {
                lib::step(x);
                return;
            }
            end_gesture(x);
        }
        _ => {}
    }
}

/// Sous-états 6, 8 et 10 : plus aucun joueur en vie : tourne sur place tant que `A5+0x35` est nul.
pub(super) fn spin(x: &mut Cx) {
    if (var::PLAYERS_ALIVE.get(x.b) as i8) > 0 {
        return go::fire_a(x);
    }
    x.c.d[0] = x.u8(f::SUB2) as u32;
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(LOOK_PREV, 0xff);
            mv(x, 0x11);
        }
        2 => {
            lib::step(x);
        }
        _ => {}
    }
}
