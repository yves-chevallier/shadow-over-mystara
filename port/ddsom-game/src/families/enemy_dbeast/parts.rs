//! Type 19, pièces de la bête (sous-état `+5 = 0`) et création des variantes.
//!
//! Les variantes 0 à 3 sont la bête et ses membres attachés ; les variantes 4 à 13 sont des pièces
//! lâchées (projectiles, bonds, ombre) : à la création elles reçoivent chacune un `+6` qui choisit
//! leur comportement dans `run`.

use super::*;
use crate::objects::Courant;

/// Création selon la variante `+3` (0x71f86).
pub(super) fn spawn(x: &mut Cx) {
    match x.u8(Obj::VARIANT) {
        0 => {
            x.set8(0, 2);
            x.set32(f::IMMUNITY, 0xffff_ffff);
            x.set8(f::SUB, 0);
            x.set8(ENTRY_TIMER, 0);
            set_hp(x);
            x.set16(f::SUB2, 0);
            follow_offset(x);
            play(x, 0x14);
        }
        1 => {
            x.set8(f::SUB, 0);
            x.set8(f::SUB2, 2);
            set_hp(x);
            follow_body(x);
            x.set32(f::IMMUNITY, 0x0021_8000);
            let mut n = 0x14;
            if x.u8(PART) == 6 {
                x.set32(f::IMMUNITY, 0xffff_ffff);
                x.set8(0, 2);
                x.set8(SHOTS, 7);
                n = 0x4b;
            }
            play(x, n);
        }
        2 => {
            let Some(p) = parent(x) else { return orphan(x) };
            x.set8(f::SUB, 0);
            x.set8(f::SUB2, 4);
            x.set8(0x7, 0);
            set_hp(x);
            x.set32(f::IMMUNITY, 0xffff_ffff);
            let rank = (x.u8(PART) as u16) << 3;
            let flip = p.u8(x.b, f::FLIP) != 0;
            let (cam_x, cam_y) = camera(x);
            let px = if flip { 0xfff0u16.wrapping_sub(rank) } else { 0x190 + rank };
            x.set16(0x8, px.wrapping_add(cam_x));
            x.set16(0x10, (0x20u16 + rank).wrapping_add(cam_y));
            play(x, 0x14);
        }
        3 => {
            set_state_long(x, ACTIVE_STANCE);
            x.set8(WANT_ANIM, 0xc);
            x.set8(CUR_ANIM, 0xc);
            x.set8(VULNERABLE, 1);
            let Some(p) = parent(x) else { return orphan(x) };
            let flip = p.u8(x.b, f::FLIP);
            x.set8(f::FLIP, flip);
            let (cam_x, cam_y) = camera(x);
            let px = if flip != 0 { 0xfff0u16 } else { 0x190 };
            x.set16(0x8, px.wrapping_add(cam_x));
            let py = if x.u8(PART) == 7 { 0x30 } else { 0x60 };
            x.set16(0x10, cam_y.wrapping_add(py));
            play(x, 8);
        }
        // pièces lâchées : état « actif », étape choisie par `+6`
        4 => lone(x, 0x0202_0000, 8),
        5 => lone(x, 0x0200_0800, 1),
        6 => lone(x, 0x0200_0600, 0xe),
        7 => lone(x, 0x0200_1200, 0xd),
        8 => detached(x, 0x0200_0a00, false),
        9 => detached(x, 0x0200_0c00, true),
        10 => detached(x, 0x0200_0e00, true),
        11 => detached(x, 0x0200_1000, false),
        12 => detached(x, 0x0200_1400, false),
        13 => detached(x, 0x0200_1600, false),
        _ => {}
    }
}

/// Points de vie des pièces (1) : elles meurent au premier coup.
fn set_hp(x: &mut Cx) {
    for off in [0x60, f::HP, 0x64] {
        x.set16(off, 1);
    }
}

/// Pièce lâchée qui joue aussitôt une animation (variantes 4 à 7).
fn lone(x: &mut Cx, state: u32, anim: u8) {
    set_state_long(x, state);
    x.set8(PART, 0xff);
    x.set16(0xc, 0);
    play(x, anim);
}

/// Pièce lâchée sans animation de départ (variantes 8 à 13) ; `active` : elle est active.
fn detached(x: &mut Cx, state: u32, active: bool) {
    set_state_long(x, state);
    x.set8(PART, 0xff);
    if active {
        x.set8(0, 2);
    }
}

// --- sous-état 0 --------------------------------------------------------------------------------

pub(super) fn run(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => attached(x),
        2 => arm(x),
        4 => reach(x),
        6 => spit(x),
        8 => spit_once(x),
        10 => bound(x),
        12 => bound_x(x),
        14 => bound_back(x),
        16 => drop_in(x),
        18 => slide(x),
        20 => sneak(x),
        22 => sneak_b(x),
        _ => {}
    }
}

/// +6 = 0 (0x7245e) : membre attaché ; il se détache quand la bête est loin de la caméra.
fn attached(x: &mut Cx) {
    if x.u8(0x7) == 0 {
        let mut cam = x.a5_u16(0x506);
        if x.a5_u8(0x36) != 0 {
            cam = x.a5_u16(0x556);
        }
        let d = x.u16(0x8).wrapping_sub(cam) as i16;
        if d <= 0x140 {
            x.add8(0x7, 2);
            let p = x.linked(PARENT);
            if p.u8(x.b, Obj::VARIANT) != 1 {
                x.set8(0, 1);
                x.set32(f::IMMUNITY, 0);
            }
        }
    }
    if x.u8(ENTRY_TIMER) != 0 {
        x.add8(f::SUB, 2);
        x.set16(f::SUB2, 0);
        x.set8(WANT_ANIM, 0xff);
        x.set8(0, 1);
        x.set16(f::IMMUNITY, 0);
        return;
    }
    follow_offset(x);
    step(x);
}

/// +6 = 2 (0x724cc) : bras de la bête : suit le parent, joue l'animation de son état et frappe un
/// joueur proche.
fn arm(x: &mut Cx) {
    const BY_PARENT_STATE: u32 = 0x724c8;
    follow_body(x);
    if x.u8(DONE) == 0 {
        let mut n = 0x4b;
        if x.u8(PART) != 6 {
            let Some(p) = parent(x) else { return orphan(x) };
            let idx = p.u8(x.b, 0xa1) as u32;
            n = x.rom8(BY_PARENT_STATE + idx);
            if (p.u32(x.b, f::STATE) as i32) <= 0x0200_0600 {
                n = 0;
            }
        }
        play_once(x, n);
    }
    if x.u8(PART) == 6 {
        let t = x.u8(TIMER).wrapping_sub(1);
        x.set8(TIMER, t);
        if (t as i8) < 0 {
            x.set8(TIMER, 0);
            if player_near(x) {
                x.set8(DONE, 1);
                play_once(x, 0x4e);
                if (x.a5_u8(0x35) as i8) > 1 && x.u8(f::KEY_FRAME) != 0 {
                    x.set8(f::KEY_FRAME, 0);
                    x.c.d[1] = x.c.d[1] & !0xff | x.u8(SHOTS) as u32;
                    eshl_thrown_by_beast(x);
                    let s = x.u8(SHOTS).wrapping_sub(1);
                    x.set8(SHOTS, s);
                    if (s as i8) < 0 {
                        x.set8(TIMER, 0x28);
                        arm_rest(x);
                    }
                }
            } else {
                arm_rest(x);
            }
        }
    }
    step(x);
    let p = x.linked(PARENT);
    if (p.u32(x.b, f::STATE) as i32) >= 0x0402_0c00 {
        x.set8(LAYER, 3);
    }
}

/// 0x725a0 : le bras se repose.
fn arm_rest(x: &mut Cx) {
    x.set8(DONE, 0);
    x.set8(SHOTS, 7);
}

/// Se dirige vers le point de ralliement de la pièce (0x721a2) ; vrai quand elle y est.
fn rally(x: &mut Cx) -> bool {
    const OFFSETS: u32 = 0x72224;
    let Some(p) = parent(x) else {
        orphan(x);
        return !x.c.cond(7);
    };
    let flip = p.u8(x.b, f::FLIP);
    x.set8(f::FLIP, flip);
    let row = (x.u8(PART).wrapping_mul(4)) as u32;
    let mut dx = x.rom16(OFFSETS + row);
    let dy = x.rom16(OFFSETS + row + 2);
    if flip != 0 {
        dx = dx.wrapping_neg();
    }
    let ax = dx.wrapping_add(p.u16(x.b, 0x8));
    let ay = dy.wrapping_add(p.u16(x.b, 0x10));
    x.set16(AIM_X, ax);
    x.set16(AIM_Y, ay);
    set_dw(x, 1, ax);
    set_dw(x, 2, ay);
    x.call(call::ANGLE);
    let angle = x.c.d[0] as u8;
    x.set8(Obj::SPEED_INDEX, angle);
    move_xy(x);
    let near = |a: u16, b: u16| a.wrapping_sub(b).wrapping_add(8) <= 0x10;
    near(x.u16(AIM_X), x.u16(0x8)) && near(x.u16(AIM_Y), x.u16(0x10))
}

/// +6 = 4 (0x725e2) : la pièce se lève, attend, puis rejoint son parent.
fn reach(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            x.set8(0, 2);
            x.set8(TIMER, 0x28);
            play(x, 0xd);
            speed(x, 0x17);
            step(x);
        }
        2 => {
            let t = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, t);
            if (t as i8) < 0 {
                x.add8(0x7, 2);
            }
        }
        4 => {
            if rally(x) {
                let Some(p) = parent(x) else { return orphan(x) };
                let n = p.u8(x.b, 0xaa).wrapping_sub(1);
                p.set_u8(x.b, 0xaa, n);
                x.set32(f::IMMUNITY, 0x0021_8000);
                if n == 0xff {
                    p.set_u8(x.b, 0xaa, 0);
                }
                if (p.u32(x.b, f::STATE) as i32) < 0x0402_0000 {
                    x.set8(0, 1);
                }
                x.set8(f::SUB2, 2);
                x.set8(0x7, 0);
            }
            step(x);
        }
        _ => {}
    }
}

/// +6 = 6 (0x7267c) : crache dès qu'un joueur est devant.
fn spit(x: &mut Cx) {
    if grabbed(x) {
        return set_state_long(x, KNOCKED);
    }
    match x.u8(0x7) {
        0 => {
            if !pick_target(x) {
                return;
            }
            step(x);
            if !lib::target_in_box(x, 0, 0x80, 0x20) {
                return;
            }
            x.add8(0x7, 2);
            x.set8(VULNERABLE, 1);
            play(x, 3);
        }
        2 => finish_to_stance(x),
        _ => {}
    }
}

/// Fin d'un geste : quand l'animation est finie, retour à la bête debout (0x726e6).
pub(super) fn finish_to_stance(x: &mut Cx) {
    if x.anim_step() {
        set_state_long(x, ACTIVE_STANCE);
        play(x, 0);
    }
}

/// +6 = 8 (0x7270c) : comme `spit` mais sans attendre la fin du geste.
fn spit_once(x: &mut Cx) {
    if grabbed(x) {
        return set_state_long(x, KNOCKED);
    }
    if x.u8(0x7) == 0 {
        if !pick_target(x) {
            return;
        }
        step(x);
        if !lib::target_in_box(x, 0, 0x80, 0x20) {
            return;
        }
        x.add8(0x7, 2);
        x.set8(VULNERABLE, 1);
        set_state_long(x, ACTIVE_STANCE);
        play(x, 0);
    }
}

/// Attend la fin de la phase de bond en l'air (image-clé), 0x727da.
pub(super) fn bound_wait(x: &mut Cx) {
    if x.u8(f::KEY_FRAME) != 0 {
        x.add8(0x7, 2);
        x.set8(VULNERABLE, 0);
    }
    step(x);
}

/// Atterrissage d'un bond en X et Z (0x727f0).
pub(super) fn bound_land(x: &mut Cx) {
    if x.move_xz().gt() {
        return step(x);
    }
    x.set32(0xc, 0);
    x.add8(0x7, 2);
    x.set8(VULNERABLE, 1);
    x.set8(WANT_ANIM, 0x18);
    x.set8(CUR_ANIM, 0x18);
    play(x, 0xf);
}

/// +6 = 10 (0x7277c) : bond en X.
fn bound(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            x.set8(VULNERABLE, 1);
            x.set8(WANT_ANIM, 6);
            x.set8(CUR_ANIM, 6);
            let vx = if x.u8(f::FLIP) != 0 { 0x3c0 } else { -0x3c0 };
            set_velocity(x, vx, 0x4c0, -0x38);
            play(x, 6);
        }
        2 => bound_wait(x),
        4 => bound_land(x),
        6 => finish_to_stance(x),
        _ => {}
    }
}

/// Début d'un bond en profondeur (variantes 12 et 14) : vitesse en Y, bond haut.
fn bound_y_start(x: &mut Cx, vy: i16, anim_marker: u8, anim: u8) {
    x.set16(0x1c, vy as u16);
    x.add8(0x7, 2);
    x.set8(VULNERABLE, 1);
    x.set8(WANT_ANIM, anim_marker);
    x.set8(CUR_ANIM, anim_marker);
    x.set16(0x1e, 0);
    x.set16(0x18, 0x500);
    x.set16(0x1a, (-0x38i16) as u16);
    play(x, anim);
}

/// Retombée d'un bond en profondeur (0x728d4 / 0x728f0).
pub(super) fn bound_y_wait(x: &mut Cx) {
    if x.u8(f::KEY_FRAME) != 0 {
        lib::sound(x, 0xeb9a0);
        x.add8(0x7, 2);
        x.set8(VULNERABLE, 0);
    }
    step(x);
}

pub(super) fn bound_y_land(x: &mut Cx) {
    if x.move_yz().gt() {
        return step(x);
    }
    x.set32(0xc, 0);
    x.add8(0x7, 2);
    x.set8(VULNERABLE, 1);
    x.set8(0, 1);
    x.set8(WANT_ANIM, 0x18);
    x.set8(CUR_ANIM, 0x18);
    play(x, 0xf);
}

/// +6 = 12 (0x7282c) : bond en profondeur vers l'avant.
fn bound_x(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            x.set16(0x1c, 0x140);
            x.add8(0x7, 2);
            x.set8(0, 2);
            x.set8(VULNERABLE, 1);
            x.set8(WANT_ANIM, 0xd);
            x.set8(CUR_ANIM, 0xd);
            x.set16(0x1e, 0);
            x.set16(0x18, 0x500);
            x.set16(0x1a, (-0x38i16) as u16);
            play(x, 9);
        }
        2 => bound_y_wait(x),
        4 => bound_y_land(x),
        6 => finish_to_stance(x),
        _ => {}
    }
}

/// +6 = 14 (0x72840) : bond en profondeur vers l'arrière.
fn bound_back(x: &mut Cx) {
    match x.u8(0x7) {
        0 => bound_y_start(x, -0x140, 0xe, 0xa),
        2 => bound_y_wait(x),
        4 => bound_y_land(x),
        6 => finish_to_stance(x),
        _ => {}
    }
}

/// +6 = 16 (0x72930) : tombe du haut de l'écran.
fn drop_in(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            x.set8(VULNERABLE, 1);
            x.set8(WANT_ANIM, 0x19);
            x.set16(0x18, (-0x300i16) as u16);
            x.set16(0x1a, (-0x10i16) as u16);
            let h = x.a5_u16(0x50a).wrapping_add(0x100).wrapping_sub(x.u16(0x10));
            x.set16(0xc, h);
            play(x, 0x10);
        }
        2 => {
            x.move_z();
            if (x.u16(0xc) as i16) < 0 {
                x.add8(0x7, 2);
                x.set32(0xc, 0);
                play(x, 0xf);
            }
        }
        4 => finish_to_stance(x),
        _ => {}
    }
}

/// Glissement vers l'avant avec la vitesse de la table (0x729bc / 0x72a2a).
fn slide_start(x: &mut Cx, anim: u8, delay: u16) {
    x.add8(0x7, 2);
    x.set16(TIMER, delay);
    x.set8(Obj::SPEED_INDEX, 8);
    if x.u8(f::FLIP) == 0 {
        x.set8(Obj::SPEED_INDEX, 0x18);
    }
    play(x, anim);
    speed(x, 0x17);
}

/// +6 = 18 (0x729ac) : glisse 30 images vers l'avant.
fn slide(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            x.set8(VULNERABLE, 1);
            slide_start(x, 0xd, 0x1e);
        }
        2 => {
            let t = x.u16(TIMER).wrapping_sub(1);
            x.set16(TIMER, t);
            if (t as i16) < 0 {
                set_state_long(x, ACTIVE_STANCE);
            }
            step(x);
            move_xy(x);
        }
        _ => {}
    }
}

/// Rampe : durée aléatoire, puis la bête surgit et frappe (+6 = 20 et 22, 0x72a16 / 0x72b16).
fn sneak_gen(x: &mut Cx, sound: u32) {
    const DELAYS: u32 = 0x72a6e;
    match x.u8(0x7) {
        0 => {
            let r = random(x) & 0xf;
            let delay = x.rom16(DELAYS + 2 * r);
            slide_start(x, 8, delay);
        }
        2 => {
            let t = x.u16(TIMER).wrapping_sub(1);
            x.set16(TIMER, t);
            if (t as i16) >= 0 {
                step(x);
                return move_xy(x);
            }
            x.add8(0x7, 2);
            play_b(x, 4);
        }
        4 => {
            if x.u8(f::KEY_FRAME) != 0 {
                x.set8(f::KEY_FRAME, 0);
                x.call(sound);
            }
            if x.anim_step() {
                x.add8(0x7, 2);
                play(x, 5);
            }
        }
        6 => {
            if x.anim_step() {
                set_state_long(x, KNOCKED);
                let flip = x.u8(f::FLIP) ^ 0x20;
                x.set8(f::FLIP, flip);
                play(x, 0);
            }
        }
        _ => {}
    }
}

fn sneak(x: &mut Cx) {
    sneak_gen(x, 0x9dda8);
}

fn sneak_b(x: &mut Cx) {
    sneak_gen(x, 0x9e062);
}
