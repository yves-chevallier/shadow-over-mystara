//! Type 18, sous-états 4 (coups) et 6, 8, 10 (repos).
//!
//! Sous-état 4, `+6` : 0 rugissement, 2 et 4 l'ogre s'envole et lance des bras (objets de type 0x2a, variante 1),
//! 6 bond et chute sur la cible, 8 bras variante 2 : l'ogre monte en suivant le bras, puis redescend.

use super::*;
use crate::objects::pools;
use crate::objects::Courant;
use crate::objects::Obj;

/// Table des montées de l'ogre qui suit son bras, indexée par la clé d'animation (longs).
const RISE: u32 = 0x713e8;

pub(super) fn run(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB2) as u32;
    match x.u8(f::SUB2) {
        0 => roar(x),
        2 => fly_up(x),
        4 => fly_up_b(x),
        6 => crash(x),
        8 => arm_swing(x),
        _ => {}
    }
}

/// Plus personne en vie → retraite ; sinon `then` (0x71094 : debout, 0x71cd0 : nouveau geste).
fn end_with(x: &mut Cx, stand: bool) {
    if !lib::players_alive(x) {
        return go::retire(x);
    }
    if stand {
        go::stand(x);
    } else {
        go::wander(x);
    }
}

// --- +6 = 0 : rugissement -----------------------------------------------------------------------------

fn roar(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB3) as u32;
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            x.call(0xe3da6);
            look(x, 0x10);
            act(x, 0);
        }
        2 => {
            if lib::step(x) {
                end_with(x, false);
            }
        }
        _ => {}
    }
}

// --- +6 = 2 et 4 : vol et bras ----------------------------------------------------------------------------

/// 0x70eca : crée le bras (type 0x2a, variante `variant`) lié à l'ogre ; faux si l'allocation échoue.
fn throw_arm(x: &mut Cx, variant: Option<u8>) -> bool {
    let Some(arm) = x.alloc(&pools::ESHL) else { return false };
    x.set16(ARM, arm.0 as u16);
    arm.set_u8(x.b, 0, 1);
    arm.set_u8(x.b, 2, 0x2a);
    if let Some(v) = variant {
        arm.set_u8(x.b, 3, v);
    }
    arm.set_u16(x.b, f::CREATOR, x.o.0 as u16);
    let target = x.u16(f::TARGET);
    arm.set_u16(x.b, f::TARGET, target);
    let flip = x.u8(f::FLIP);
    arm.set_u8(x.b, f::FLIP, flip);
    true
}

/// 0x70ef8 : l'ogre s'élève.
fn take_off(x: &mut Cx) {
    x.add8(f::SUB3, 2);
    x.set16(Obj::VZ, 0x400);
    x.set16(Obj::AZ, 0);
    act(x, 1);
}

/// `+6` = 2 : l'ogre lance un bras, monte hors de l'écran, redescend au-dessus du joueur et retombe.
fn fly_up(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB3) as u32;
    match x.u8(f::SUB3) {
        0 => {
            if !pick_target(x) {
                return go::retire(x);
            }
            look(x, 0x11);
            if !throw_arm(x, Some(1)) {
                return end_with(x, true);
            }
            take_off(x);
        }
        _ => fly_common(x),
    }
}

/// `+6` = 4 : comme 2, après un bond d'arrivée par le haut (0x710ba, 0x710d8..).
fn fly_up_b(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB3) as u32;
    match x.u8(f::SUB3) {
        0 => {
            if !pick_target(x) {
                return go::retire(x);
            }
            x.call(0xe3da6);
            look(x, 0x12);
            if !throw_arm(x, Some(1)) {
                return end_with(x, true);
            }
            take_off(x);
        }
        2 => fly_loop(x),
        4 => {
            if !count_down(x, TIMER) {
                x.add8(f::SUB3, 2);
                x.set_bit(1, 1, false);
                let t = x.linked(f::TARGET);
                let tz = t.u16(x.b, Obj::Y);
                x.set16(Obj::Y, tz);
                x.set16(Obj::Z, 0x70);
                let mut d0 = 0u16;
                let mut vx = 0x700i16;
                if x.u8(f::FLIP) == 0 {
                    d0 = 0x180;
                    vx = -vx;
                }
                let px = d0.wrapping_add(x.a5_u16(0x5a6));
                x.set16(Obj::X, px);
                x.set16(Obj::VX, vx as u16);
                x.set16(Obj::VZ, -0x680i16 as u16);
                x.set16(Obj::AX, 0);
                x.set16(Obj::AZ, 0);
                x.call(0xe3da6);
                x.set8(INVULNERABLE, 0);
                act(x, 5);
            }
        }
        6 => {
            x.move_xz();
            if (x.u16(Obj::Z) as i16) > 0 {
                return;
            }
            x.add8(f::SUB3, 2);
            x.set16(TIMER, 0x1e);
            x.set16(TIMER2, 0);
            x.set16(Obj::Z, 0);
            let vy = if x.u8(f::FLIP) != 0 { -0x38i16 } else { 0x38 };
            x.set16(Obj::AX, vy as u16);
            x.c.d[0] = 0x1e;
            x.call(call::SHOCK);
            x.call(0xec976);
            x.call(0xecdba);
            act(x, 6);
        }
        8 => {
            if !count_down(x, TIMER2) {
                x.set16(TIMER2, 4);
                spark(x, 3);
            }
            if count_down(x, TIMER) {
                x.move_x();
                lib::step(x);
            } else {
                go::stand(x);
            }
        }
        _ => {}
    }
}

/// Étapes 2 à 8 communes aux `+6` = 2 et 4 : le vol (70f14), la retombée (70fd6, 7102e, 7107e).
fn fly_common(x: &mut Cx) {
    match x.u8(f::SUB3) {
        2 => fly_loop(x),
        4 => fly_down_from_above(x),
        6 => land_heavily(x),
        8 => {
            if (x.u16(TIMER) as i16) > 0 {
                x.add16(TIMER, 0xffff);
                return;
            }
            if x.anim_step() {
                end_with(x, true);
            }
        }
        _ => {}
    }
}

/// 0x70f14 : vol en montée avec les bras ; fini quand la hauteur dépasse le haut de l'écran.
fn fly_loop(x: &mut Cx) {
    let top = x.a5_u16(0x5aa).wrapping_add(0xf0);
    let here = x.u16(Obj::Y).wrapping_add(x.u16(Obj::Z));
    if (here as i16) >= (top as i16) {
        x.add8(f::SUB3, 2);
        x.set16(TIMER, 0x28);
        x.set8(INVULNERABLE, 1);
        x.set_bit(1, 1, true);
        let arm = x.linked(ARM);
        arm.set_u8(x.b, 0x58, 1);
        return;
    }
    x.call(0x115e2);
    lib::step(x);
    let key = x.u8(f::KEY_FRAME) & 0x7f;
    if key < 3 {
        flash_key(x);
        let v = x.u16(Obj::VZ).wrapping_add(-0x28i16 as u16);
        x.set16(Obj::VZ, v);
        let d = ((v as u32) << 16) as i32 >> 8;
        x.add32(Obj::Z, d as u32);
        let v = x.u16(Obj::VZ);
        x.set16(Obj::AZ, v);
    } else {
        flash_key(x);
        let v = x.u16(Obj::AZ).wrapping_add(-0x30i16 as u16);
        x.set16(Obj::AZ, v);
        let d = ((v as u32) << 16) as i32 >> 8;
        x.add32(Obj::Z, d as u32);
        x.set16(Obj::VZ, 0x400);
    }
}

/// 0x70fd6 : l'ogre disparaît par le haut, puis reparaît au-dessus du joueur et plonge.
fn fly_down_from_above(x: &mut Cx) {
    let n = x.u16(TIMER).wrapping_sub(1);
    x.set16(TIMER, n);
    if (n as i16) <= 0 {
        x.add8(f::SUB3, 2);
        x.set8(INVULNERABLE, 0);
        return act(x, 2);
    }
    if n != 0xa {
        return;
    }
    x.set_bit(1, 1, false);
    let t = x.linked(f::TARGET);
    let (tx, tz) = (t.u16(x.b, Obj::X), t.u16(x.b, Obj::Y));
    x.set16(Obj::X, tx);
    x.set16(Obj::Y, tz);
    let h = x.a5_u16(0x5aa).wrapping_add(0xf0).wrapping_sub(tz);
    x.set16(Obj::Z, h);
    x.set16(Obj::VZ, -0x500i16 as u16);
    x.set16(Obj::AZ, -0x10i16 as u16);
}

/// 0x7102e : chute libre jusqu'au sol, secousse et animation d'impact.
fn land_heavily(x: &mut Cx) {
    let g = x.u16(Obj::AZ);
    x.add16(Obj::VZ, g);
    let d = ((x.u16(Obj::VZ) as u32) << 16) as i32 >> 8;
    x.add32(Obj::Z, d as u32);
    if (x.u16(Obj::Z) as i16) > 0 {
        return;
    }
    x.add8(f::SUB3, 2);
    x.set16(TIMER, 0x1e);
    x.set16(Obj::Z, 0);
    x.c.d[0] = 0x1e;
    x.call(0x2860);
    spark(x, 5);
    x.call(0xecc0a);
    act(x, 3);
}

// --- +6 = 6 : bond et chute sur la cible ---------------------------------------------------------------------

fn crash(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB3) as u32;
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            x.set16(Obj::AX, 0);
            let angle = if x.u8(f::FLIP) != 0 { 8 } else { 0x18 };
            x.set8(Obj::SPEED_INDEX, angle);
            set_speed(x, 0x40);
            // vitesse verticale : le nombre d'images pour couvrir la distance, par pas de 0x10
            let target = load_target(x);
            let mut dist = x.u32(Obj::X).wrapping_sub(target.u32(x.b, Obj::X)) as i32;
            if dist < 0 {
                dist = dist.wrapping_neg();
            }
            let mut speed = ((x.u16(Obj::VX) as u32) << 16) as i32 >> 8;
            if speed < 0 {
                speed = speed.wrapping_neg();
            }
            let mut d1 = 0x200u16;
            let mut guard = 0;
            loop {
                dist = dist.wrapping_sub(speed);
                if dist <= 0 || guard > 0x10000 {
                    break;
                }
                d1 = d1.wrapping_add(0x10);
                guard += 1;
            }
            x.set16(Obj::VZ, d1);
            x.set16(Obj::AZ, -0x38i16 as u16);
            x.call(0xe3da6);
            look(x, 0x13);
            act(x, 7);
        }
        2 => {
            lib::step(x);
            if x.u8(f::KEY_FRAME) != 0 {
                return;
            }
            x.move_xz();
            if (x.u16(Obj::VZ) as i16) <= 0x100 {
                x.add8(f::SUB3, 2);
                act(x, 8);
            }
        }
        4 => {
            x.move_xz();
            if (x.u16(Obj::VZ) as i16) <= -0x100 {
                x.add8(f::SUB3, 2);
                act(x, 9);
            }
        }
        6 => {
            x.move_xz();
            if (x.u16(Obj::Z) as i16) > 0 {
                return;
            }
            x.add8(f::SUB3, 2);
            x.set16(Obj::Z, 0);
            x.set16(TIMER, 0x14);
            x.set16(TIMER2, 0);
            x.set8(INVULNERABLE, 0);
            let vy = if x.u8(f::FLIP) != 0 { -0x5ci16 } else { 0x5c };
            x.set16(Obj::AX, vy as u16);
            x.c.d[0] = 0x14;
            x.call(call::SHOCK);
            x.call(0xec976);
            x.call(0xecdba);
            act(x, 0xa);
        }
        8 => {
            if !count_down(x, TIMER2) {
                x.set16(TIMER2, 4);
                spark(x, 3);
            }
            if count_down(x, TIMER) {
                x.move_x();
                lib::step(x);
            } else {
                end_with(x, false);
            }
        }
        _ => {}
    }
}

// --- +6 = 8 : bras variante 2 ---------------------------------------------------------------------------

fn arm_swing(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB3) as u32;
    match x.u8(f::SUB3) {
        0 => {
            if !pick_target(x) {
                return go::retire(x);
            }
            look(x, 0x14);
            if !throw_arm(x, Some(2)) {
                return end_with(x, false);
            }
            take_off(x);
        }
        2 => {
            x.call(0x115e2);
            flash_key(x);
            if (x.u16(Obj::Z) as i16) >= 0x20 && x.u8(f::KEY_FRAME) == 4 {
                x.add8(f::SUB3, 2);
                x.set8(INVULNERABLE, 0);
                x.set16(TIMER, 0xa);
                x.set16(TIMER2, 0x48);
            } else {
                let idx = (x.u8(f::KEY_FRAME) as u32) << 2;
                let d = x.rom32(RISE + idx);
                x.add32(Obj::Z, d);
            }
            lib::step(x);
        }
        4 => {
            lib::step(x);
            bob(x);
            x.call(0x115e2);
            flash_key(x);
            if !count_down(x, TIMER) {
                x.add8(f::SUB3, 2);
                let arm = x.linked(ARM);
                arm.set_u8(x.b, 0x58, 1);
            }
        }
        6 => {
            lib::step(x);
            bob(x);
            x.call(0x115e2);
            flash_key(x);
            if x.u8(ANIM_FLAGS) != 0 {
                x.add8(f::SUB3, 2);
                let arm = x.linked(ARM);
                arm.set_u8(x.b, 0x59, 1);
            }
        }
        8 => {
            flash_key(x);
            lib::step(x);
            bob(x);
            x.call(0x115e2);
            let arm = x.linked(ARM);
            if arm.u8(x.b, 0) == 0 || (arm.u8(x.b, 0x5a) != 0 && x.u8(f::KEY_FRAME) == 4) {
                x.add8(f::SUB3, 2);
            }
        }
        10 => {
            flash_key(x);
            let h = x.u32(Obj::Z).wrapping_sub(0x18000);
            x.set32(Obj::Z, h);
            if (h as i32) > 0 {
                lib::step(x);
                return;
            }
            x.add8(f::SUB3, 2);
            x.set16(Obj::Z, 0);
            x.set16(TIMER, 0x10);
            mv(x, 0);
        }
        12 => {
            if count_down(x, TIMER) {
                lib::step(x);
            } else {
                end_with(x, false);
            }
        }
        _ => {}
    }
}

// --- sous-états 6, 8, 10 : repos --------------------------------------------------------------------------

pub(super) fn rest(x: &mut Cx) {
    if (x.a5_u8(0x35) as i8) > 0 {
        return go::wander(x);
    }
    x.c.d[0] = x.u8(f::SUB2) as u32;
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(LOOK_PREV, 0xff);
            mv(x, 0);
        }
        2 => {
            lib::step(x);
        }
        _ => {}
    }
}
