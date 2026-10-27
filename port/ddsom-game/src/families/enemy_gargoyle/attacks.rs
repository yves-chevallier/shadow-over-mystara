//! Type 8, sous-états 4 (coups), 6, 8, 10 (repos), 12 (saisie) et 14 (fin de geste).
//! Sous-état 4, `+6` : 0 et 2 griffe, 4 coup bref, 6 charge latérale, 8 saut en arrière, 10 plongeon,
//! 12 saisie d'un joueur.

use super::*;
use crate::objects::Courant;

/// Fin d'un geste : si plus personne n'est en vie la créature se retire, sinon elle repart au hasard.
fn finish(x: &mut Cx) {
    if lib::players_alive(x) {
        go::wander(x);
    } else {
        go::retire(x);
    }
}

/// Le coup frappe quand la clé d'animation `+0x33` passe : elle est aussitôt remise à zéro.
fn strike_hit(x: &mut Cx) {
    if x.u8(KEY) != 0 {
        x.set8(KEY, 0);
        x.call(0xebfdc);
    }
}

/// Libère la victime saisie (`+0x54`) : ses drapeaux de saisie retombent.
fn release_victim(x: &mut Cx) {
    if x.u16(VICTIM) != 0 {
        let v = x.linked(VICTIM);
        v.set_u8(x.b, 0x7e, 0);
        v.set_u8(x.b, 0x53, 0);
    }
}

pub(super) fn run(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB2) as u32;
    match x.u8(f::SUB2) {
        0 => claw(x),
        2 => claw_b(x),
        4 => short_blow(x),
        6 => sidestep(x),
        8 => jump_back(x),
        10 => plunge_attack(x),
        12 => stoop(x),
        _ => {}
    }
}

// --- +6 = 0 : griffe ----------------------------------------------------------------------------

fn claw(x: &mut Cx) {
    x.c.d[0] = x.u8(STEP) as u32;
    match x.u8(STEP) {
        0 => {
            x.add8(STEP, 2);
            look(x, 0x10);
            strike(x, 0);
        }
        2 => {
            if !lib::step(x) {
                return strike_hit(x);
            }
            let grab = lib::players_alive(x) && x.u8(GRABBED_ONCE) != 0 && x.u8(HIT_FLAGS) & 4 != 0;
            if grab {
                release_victim(x);
                return go::strike_2(x, true);
            }
            x.add8(STEP, 2);
            strike(x, 1);
        }
        4 => {
            if lib::step(x) {
                finish(x);
            }
        }
        _ => {}
    }
}

// --- +6 = 2 : griffe lourde ---------------------------------------------------------------------

fn claw_b(x: &mut Cx) {
    x.c.d[0] = x.u8(STEP) as u32;
    match x.u8(STEP) {
        0 => {
            x.add8(STEP, 2);
            look(x, 0x11);
            strike(x, 2);
        }
        2 => {
            if !lib::step(x) {
                return strike_hit(x);
            }
            let grab = lib::players_alive(x) && x.u8(GRABBED_ONCE) != 0 && x.u8(HIT_FLAGS) & 4 != 0;
            if grab {
                release_victim(x);
                return go::strike_4(x, true);
            }
            x.add8(STEP, 2);
            strike(x, 3);
        }
        4 => {
            if lib::step(x) {
                finish(x);
            }
        }
        _ => {}
    }
}

// --- +6 = 4 : coup bref --------------------------------------------------------------------------

fn short_blow(x: &mut Cx) {
    x.c.d[0] = x.u8(STEP) as u32;
    match x.u8(STEP) {
        0 => {
            x.add8(STEP, 2);
            look(x, 0x12);
            let n = if x.u8(GRABBED_ONCE) != 0 { 5 } else { 4 };
            strike(x, n);
        }
        2 => {
            if !lib::step(x) {
                return strike_hit(x);
            }
            finish(x);
        }
        _ => {}
    }
}

// --- +6 = 6 : charge latérale --------------------------------------------------------------------

fn sidestep(x: &mut Cx) {
    x.c.d[0] = x.u8(STEP) as u32;
    match x.u8(STEP) {
        0 => {
            x.add8(STEP, 2);
            look(x, 0x13);
            strike(x, 6);
        }
        2 => {
            lib::step(x);
            if x.u8(ANIM_FLAGS) == 0 {
                return;
            }
            x.add8(STEP, 2);
            x.set16(TIMER, 0x1a);
            x.set16(TIMER2, 0);
            let (angle, vy) = if x.u8(f::FLIP) != 0 { (8, -0x38i16) } else { (0x18, 0x38) };
            x.set8(ANGLE, angle);
            x.set16(SPEED_Y, vy as u16);
            set_speed(x, 0x40);
            x.call(0xece9e);
        }
        4 => {
            if !count_down(x, TIMER2) {
                x.set16(TIMER2, 4);
                spark(x, 8);
            }
            x.move_x();
            if count_down(x, TIMER) {
                return;
            }
            x.add8(STEP, 2);
            x.set16(TIMER, 0x14);
            x.set16(SPEED_Y, 0);
            strike(x, 7);
        }
        6 => {
            if !count_down(x, TIMER2) {
                x.set16(TIMER2, 6);
                spark(x, 8);
            }
            if count_down(x, TIMER) {
                x.move_x();
                lib::step(x);
            } else if lib::players_alive(x) {
                go::wander(x);
            } else {
                go::retire(x);
            }
        }
        _ => {}
    }
}

// --- +6 = 8 : saut en arrière --------------------------------------------------------------------

fn jump_back(x: &mut Cx) {
    x.c.d[0] = x.u8(STEP) as u32;
    match x.u8(STEP) {
        0 => {
            x.add8(STEP, 2);
            look(x, 0x14);
            strike(x, 8);
        }
        2 => {
            lib::step(x);
            if x.u8(KEY) != 0 {
                return;
            }
            x.add8(STEP, 2);
            let (vx, vy) = if x.u8(f::FLIP) != 0 { (-0x400i16, 0x10i16) } else { (0x400, -0x10) };
            x.set16(SPEED_X, vx as u16);
            x.set16(SPEED_Y, vy as u16);
            x.set16(SPEED_Z, 0x5c0);
            x.set16(GRAVITY, -0x30i16 as u16);
        }
        4 => {
            x.move_xz();
            if (x.u16(SPEED_Z) as i16) > 0x1c0 {
                return;
            }
            x.add8(STEP, 2);
            strike(x, 9);
        }
        6 => {
            x.move_xz();
            if (x.u16(SPEED_Z) as i16) > -0x1e0 {
                lib::step(x);
                return;
            }
            x.add8(STEP, 2);
            x.set8(INVULNERABLE, 0);
            x.set16(SPEED_Y, 0);
            x.set16(GRAVITY, -0x20i16 as u16);
            strike(x, 0xa);
        }
        8 => {
            x.move_xz();
            if (x.u16(HEIGHT) as i16) > 0 {
                lib::step(x);
                return;
            }
            x.add8(STEP, 2);
            x.set16(HEIGHT, 0);
            fly(x, 0xb);
        }
        10 => {
            if lib::step(x) {
                finish(x);
            }
        }
        _ => {}
    }
}

// --- +6 = 10 : plongeon sur la cible ---------------------------------------------------------------

fn plunge_attack(x: &mut Cx) {
    x.c.d[0] = x.u8(STEP) as u32;
    match x.u8(STEP) {
        0 => {
            x.add8(STEP, 2);
            x.set16(SPEED_Y, 0);
            let angle = if x.u8(f::FLIP) != 0 { 8 } else { 0x18 };
            x.set8(ANGLE, angle);
            set_speed(x, 0x17);
            // nombre d'images pour couvrir la distance à la cible : la vitesse verticale en découle
            let target = load_target(x);
            let mut dist = x.u32(X).wrapping_sub(target.u32(x.b, X)) as i32;
            if dist < 0 {
                dist = dist.wrapping_neg();
            }
            let mut speed = ((x.u16(SPEED_X) as u32) << 16) as i32 >> 8;
            if speed < 0 {
                speed = speed.wrapping_neg();
            }
            let mut d1 = 0x100u16;
            let mut guard = 0;
            loop {
                dist = dist.wrapping_sub(speed);
                if dist <= 0 || guard > 0x10000 {
                    break;
                }
                d1 = d1.wrapping_add(0xe);
                guard += 1;
            }
            x.set16(SPEED_Z, d1);
            x.set16(GRAVITY, -0x20i16 as u16);
            look(x, 0x15);
            strike(x, 0xb);
        }
        2 => {
            x.anim_step();
            if x.u8(KEY) != 0 {
                return;
            }
            x.move_xz();
            if (x.u16(SPEED_Z) as i16) > 0xe0 {
                return;
            }
            x.add8(STEP, 2);
            strike(x, 0xc);
        }
        4 => {
            x.move_xz();
            if (x.u16(SPEED_Z) as i16) > -0xa0 {
                lib::step(x);
                return;
            }
            x.add8(STEP, 2);
            x.set16(GRAVITY, -0x40i16 as u16);
            strike(x, 0xd);
        }
        6 => {
            strike_hit(x);
            x.move_xz();
            if (x.u16(HEIGHT) as i16) > 0 {
                lib::step(x);
                return;
            }
            x.add8(STEP, 2);
            x.set16(TIMER, 0x14);
            x.set16(TIMER2, 0);
            x.set16(HEIGHT, 0);
            fly(x, 6);
        }
        8 => {
            if !count_down(x, TIMER2) {
                x.set16(TIMER2, 4);
                spark(x, 8);
            }
            if count_down(x, TIMER) {
                x.move_x();
                lib::step(x);
            } else {
                finish(x);
            }
        }
        _ => {}
    }
}

// --- +6 = 12 : le gargouille fond sur la cible --------------------------------------------------------

fn stoop(x: &mut Cx) {
    x.c.d[0] = x.u8(STEP) as u32;
    match x.u8(STEP) {
        0 => {
            x.add8(STEP, 2);
            look(x, 4);
            fly(x, 1);
        }
        2 => {
            if (x.u16(HEIGHT) as i16) >= 0x34 && x.u8(KEY) == 4 {
                x.add8(STEP, 2);
                x.set16(TIMER, 2);
            } else {
                rise(x);
            }
            lib::step(x);
        }
        4 => {
            bob(x);
            face_target(x);
            if count_down(x, TIMER) {
                lib::step(x);
            } else {
                x.add8(STEP, 2);
                aim_dive(x);
            }
        }
        6 => {
            x.move_xyz();
            if (x.u16(HEIGHT) as i16) <= 0 {
                return land_from_dive(x);
            }
            if (x.u16(TIMER) as i16) > 0 {
                let n = x.u16(TIMER).wrapping_sub(1);
                x.set16(TIMER, n);
                if (n as i16) <= 0 {
                    x.set16(TIMER, 0);
                    x.set16(SPEED_Z, 0);
                    x.set16(GRAVITY, -0x30i16 as u16);
                }
            }
            x.c.d[0] = 9;
            x.call(0xfc44);
            if x.c.cond(7) {
                return;
            }
            x.c.d[0] = 0;
            x.call(0xfd64);
        }
        8 => {
            if !count_down(x, TIMER2) {
                x.set16(TIMER2, 5);
                spark(x, 8);
            }
            lib::step(x);
            if count_down(x, TIMER) {
                x.move_xy();
            } else if lib::players_alive(x) {
                go::wander(x);
            } else {
                go::retire(x);
            }
        }
        _ => {}
    }
}

/// 0x64e00 : vise la cible : angle, vitesse verticale pour y arriver en autant d'images que la course horizontale.
fn aim_dive(x: &mut Cx) {
    let target = load_target(x);
    let (tx, tz) = (target.u16(x.b, X), target.u16(x.b, DEPTH));
    x.c.d[1] = tx as u32;
    x.c.d[2] = tz as u32;
    x.set16(GOAL_X, tx);
    x.set16(GOAL_DEPTH, tz);
    x.call(0x137e);
    let angle = x.c.d[0] as u8;
    x.set8(ANGLE, angle);
    set_speed(x, 0x35);
    let abs = |v: u16| (v as i16).unsigned_abs();
    let mut dist = abs(x.u16(GOAL_X).wrapping_sub(x.u16(X))) as i32;
    let dz = abs(x.u16(GOAL_DEPTH).wrapping_sub(x.u16(DEPTH))) as i32;
    let mut speed = abs(x.u16(SPEED_X)) as i32;
    let speed2 = abs(x.u16(SPEED_X2)) as i32;
    if (dist as i16) < (dz as i16) {
        dist = dz;
        speed = speed2;
    }
    let mut remaining = dist << 16;
    let step = ((speed << 16) as i32) >> 8;
    let mut frames = 0u16;
    let mut guard = 0;
    loop {
        remaining = remaining.wrapping_sub(step);
        if remaining <= 0 || guard > 0x10000 {
            break;
        }
        frames = frames.wrapping_add(1);
        guard += 1;
    }
    x.c.d[0] = x.u16(HEIGHT) as u32;
    x.c.d[1] = frames as u32;
    x.call(0x17ea);
    let mut d0 = (x.c.d[0].rotate_left(16) as i32 >> 8) as i16;
    if d0 > 0x400 {
        d0 = 0x400;
    }
    x.set16(SPEED_Z, d0.wrapping_neg() as u16);
    x.set16(SPEED_Y, 0);
    x.set16(GRAVITY, 0);
    x.set16(SPEED_Y2, 0);
    x.call(0xe3776);
    x.set16(TIMER, 0x5a);
    strike(x, 0xe);
}

/// 0x64eee : retombée du plongeon : la créature se pose en glissant dans l'autre sens.
fn land_from_dive(x: &mut Cx) {
    x.add8(STEP, 2);
    x.set16(TIMER, 0x18);
    x.set16(TIMER2, 0);
    x.set16(HEIGHT, 0);
    let vx = (x.u16(SPEED_X) as i16).wrapping_neg() >> 5;
    let vy = (x.u16(SPEED_X2) as i16).wrapping_neg() >> 5;
    x.set16(SPEED_Y, vx as u16);
    x.set16(SPEED_Y2, vy as u16);
    fly(x, 6);
}

// --- sous-états 6, 8, 10 : repos -----------------------------------------------------------------------

pub(super) fn rest(x: &mut Cx) {
    if x.a5_u8(0x30) != 0 && (x.a5_u8(0x35) as i8) > 0 {
        return go::wander(x);
    }
    x.c.d[0] = x.u8(f::SUB2) as u32;
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(LOOK_PREV, 0xff);
            fly(x, 0);
        }
        2 => {
            lib::step(x);
        }
        _ => {}
    }
}

// --- sous-état 12 : saisie ------------------------------------------------------------------------------

pub(super) fn grab(x: &mut Cx) {
    let victim = x.linked(f::TARGET);
    x.c.a[1] = victim.0 | 0xff00_0000;
    if victim.u8(x.b, 0) == 0 {
        return go::strike_10(x);
    }
    if victim.u8(x.b, 0x188) == 0 {
        return go::circle(x);
    }
    if x.u8(f::SUB2) != 0 {
        return;
    }
    x.c.d[0] = x.u8(STEP) as u32;
    match x.u8(STEP) {
        0 => {
            x.add8(STEP, 2);
            let (px, pz) = (victim.u16(x.b, X), victim.u32(x.b, DEPTH));
            x.set16(X, px);
            x.set32(DEPTH, pz.wrapping_sub(1));
            let shift = if x.u8(f::FLIP) != 0 { -0x1bi16 } else { 0x1b };
            x.add16(X, shift as u16);
            x.set16(TIMER, 0xe);
            strike(x, 0xf);
        }
        2 => {
            if count_down(x, TIMER) {
                lib::step(x);
                return;
            }
            x.add8(STEP, 2);
            let n = if (x.u8(PHASE) as i8) < 2 { 0x10 } else { 0x11 };
            strike(x, n);
            x.set16(TIMER, 6);
            x.set16(TIMER2, 0);
        }
        4 => {
            if count_down(x, TIMER2) {
                lib::step(x);
                return;
            }
            x.c.d[1] = 2;
            let shift = if x.u8(f::FLIP) != 0 { 0x23i16 } else { -0x23 };
            let (d2, d3, d4) = (x.u16(X).wrapping_add(shift as u16), x.u16(HEIGHT).wrapping_add(0x11), x.u16(DEPTH).wrapping_sub(1));
            x.c.d[2] = d2 as u32;
            x.c.d[3] = d3 as u32;
            x.c.d[4] = d4 as u32;
            x.call(0xaaa40);
            x.call(0xedd96);
            let victim = x.linked(f::TARGET);
            x.c.a[1] = victim.0 | 0xff00_0000;
            let hp = victim.u16(x.b, f::HP).wrapping_sub(4);
            victim.set_u16(x.b, f::HP, hp);
            if (hp as i16) <= 0 {
                victim.set_u16(x.b, f::HP, 1);
                return dismount(x);
            }
            if !count_down(x, TIMER) {
                return dismount(x);
            }
            let rank = x.u8(PHASE) as i8 as i16 as u16;
            let t = x.rom16(0x650ba + 2 * rank as u32);
            x.set16(TIMER2, t);
            lib::step(x);
        }
        _ => {}
    }
}

/// 0x650a6 : la victime est relâchée (`+0x6f` levé) et la créature passe en vol circulaire.
fn dismount(x: &mut Cx) {
    let victim = x.linked(f::TARGET);
    x.c.a[1] = victim.0 | 0xff00_0000;
    victim.set_u8(x.b, 0x6f, 1);
    x.call(0xfd8c);
    go::circle(x);
}

/// Sous-état 14 : fin de geste, cible de remplacement.
pub(super) fn done(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB2) as u32;
    if x.u8(f::SUB2) == 0 {
        regroup(x);
    }
}

/// 0x653b2 : le combat reprend : cible choisie, palier de PV franchi, nouveau geste.
pub(super) fn regroup(x: &mut Cx) {
    x.set8(HIT_FLAGS, 0);
    x.set8(0, 1);
    x.set8(LOOK_PREV, 0xff);
    if !pick_target(x) {
        return go::retire(x);
    }
    x.c.a[1] = 0x105e2;
    lib::hp_threshold(x, 0x105e2);
    if target_in_box(x, 0, 0x28, 0xc) {
        go::strike_0(x, false);
    } else {
        go::wander(x);
    }
}
