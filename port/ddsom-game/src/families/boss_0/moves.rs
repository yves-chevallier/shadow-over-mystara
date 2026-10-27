//! Boss 0, état actif : entrées en scène (`+5` = 0), combat (2), attaques (4), attente (6 à 10).

use super::*;
use crate::objects::pools;
use crate::objects::Courant;
use crate::objects::Obj;

// --- aides communes -----------------------------------------------------------------------------

/// Suite d'un coup, d'une course ou d'une attente : un nouveau départ si un joueur vit, sinon l'attente.
fn restart_or_idle(x: &mut Cx) {
    if lib::any_player_alive(x) {
        run_start(x)
    } else {
        idle_alone(x)
    }
}

/// Fin d'une animation de coup (0x79e88) : la course reprend quand elle est terminée.
fn end_of_strike(x: &mut Cx) {
    if lib::step(x) {
        restart_or_idle(x);
    }
}

/// Le boss est-il arrivé à la position qu'il vise ? `limit` : abscisse visée (sinon la caméra décalée de 0xa0
/// vers la droite, 0xe0 vers la gauche). Il faut avoir atteint ou dépassé la limite dans le sens du mouvement.
fn reached(x: &mut Cx, limit: Option<u16>) -> bool {
    let moving_left = x.u16(0x14) as i16 >= 0;
    let bound = match limit {
        Some(l) => l,
        None => {
            let reach = if moving_left { 0xa0 } else { 0xe0 };
            set_dw(x, 1, reach);
            camera_x(x).wrapping_add(reach)
        }
    };
    let px = x.u16(0x8);
    let (d0, d1) = if moving_left { (bound, px) } else { (px, bound) };
    set_dw(x, 0, d0);
    set_dw(x, 1, d1);
    Fl::sub16(d1, d0).ge()
}

/// Pas de course vers l'avant à vitesse horizontale `+0x14` : animation, déplacement, poussière à la mi-pas.
fn gallop_step(x: &mut Cx) {
    x.anim_step();
    x.move_x();
    if x.u8(f::KEY_FRAME) == 2 {
        x.set8(f::KEY_FRAME, 0);
        lib::debris(x, 1);
        lib::shake_54(x, 0x10);
        lib::sound(x, 0xe3d32);
    }
}

/// Fin de la course d'entrée : lance la glissade (vitesse `v` en `+0x16`, orientée selon le sens).
fn start_slide(x: &mut Cx, v: u16, extra: bool) {
    next7(x);
    x.set16(TIMER, 0x1e);
    x.set16(TIMER2, 0);
    let v = if x.u8(f::FLIP) != 0 { v.wrapping_neg() } else { v };
    set_dw(x, 0, v);
    x.set16(0x16, v);
    if extra {
        lib::sound(x, 0xe13b0);
    }
    lib::sound(x, 0xe3d32);
    lib::sound(x, 0xecdba);
    body(x, 9);
}

/// Glissade (0x79750) : une bouffée de poussière toutes les 4 images, puis la fin de la glissade.
fn slide(x: &mut Cx) {
    if !sub16(x, TIMER2, 1).gt() {
        x.set16(TIMER2, 4);
        lib::debris(x, 0);
    }
    if expired(x, TIMER) {
        next7(x);
        body(x, 0xa);
    } else {
        x.move_x();
        lib::step(x);
    }
}

/// La cible (`+0x50`) devient la position visée (`+0xbc`) ; D0 = angle de la charge.
fn mark_target(x: &mut Cx) {
    let t = x.linked(f::TARGET);
    let tx = t.u16(x.b, 0x8);
    x.set16(MARK_X, tx);
}

// --- groupe 0 : entrées en scène --------------------------------------------------------------------

pub(super) fn intro(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => arrival(x),
        2 => transformation(x),
        4 => partner_arrival(x),
        _ => {}
    }
}

/// 0x7964c : la moitié principale arrive en courant depuis un bord de l'écran.
fn arrival(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            next7(x);
            let cam = camera_x(x).wrapping_add(0xc0);
            set_dw(x, 2, cam);
            x.set8(f::FLIP, 0);
            let mut angle = 0x18;
            if Fl::sub16(cam, x.u16(0x8)).gt() {
                x.set8(f::FLIP, 0x20);
                angle = 8;
            }
            x.c.d[0] = angle;
            x.set8(Obj::SPEED_INDEX, angle as u8);
            speed(x, 0x21);
            lib::sound(x, 0xe3d32);
            anim_kind(x, 0x30);
            entry(x, 3);
        }
        2 => {
            gallop_step(x);
            if reached(x, None) {
                start_slide(x, 0x20, true);
            }
        }
        4 => slide(x),
        6 => {
            if lib::step(x) {
                x.set_a5_u32(0x38, 0x0459_5900);
                x.set8(INVULNERABLE, 0);
                lib::set_immunities(x);
                x.call(0xd5f30);
                run_start(x);
            }
        }
        _ => {}
    }
}

/// 0x797ae : la transformation (PV à la moitié) : la moitié principale se redresse, crée la seconde et attend
/// qu'elle ait fini son entrée.
fn transformation(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            next7(x);
            x.set16(TIMER, 0x3c);
            lib::sound(x, 0xe3cbe);
            anim_kind(x, 0x31);
            body(x, 0xe);
        }
        2 => {
            if !expired(x, TIMER) {
                x.anim_step();
                return;
            }
            next7(x);
            let Some(partner) = alloc_retry(x, &pools::BOSS) else { return };
            partner.set_u8(x.b, 0, 2);
            partner.set_u8(x.b, 2, 0);
            partner.set_u8(x.b, 3, 1);
            let side = x.u8(f::FLIP);
            partner.set_u8(x.b, f::FLIP, side);
            partner.set_u32(x.b, f::IMMUNITY, 0xffff_ffff);
            partner.set_u16(x.b, PARTNER, x.o.0 as u16);
            x.set16(PARTNER, partner.0 as u16);
            place_partner(x, partner);
        }
        4 => {
            let partner = x.linked(PARTNER);
            if partner.u32(x.b, f::STATE) == 0x0200_0408 {
                next7(x);
                body(x, 0xf);
            } else {
                x.anim_step();
            }
        }
        6 => {
            let partner = x.linked(PARTNER);
            if partner.u32(x.b, f::STATE) == 0x0200_040a {
                next7(x);
                x.set16(TIMER, 0x5a);
                lib::sound(x, 0xe3bd6);
                body(x, 0x10);
            } else {
                x.anim_step();
            }
        }
        8 => {
            if expired(x, TIMER) {
                x.set8(0, 1);
                x.set8(ROLE, 0);
                lib::set_immunities(x);
                x.set_a5_u16(0x51e, 0);
                run_start(x);
            } else {
                x.anim_step();
            }
        }
        _ => {}
    }
}

/// 0x7ac7a : place la seconde moitié au bord de l'écran, du côté où la moitié principale regarde : à
/// `caméra + 0x1c0` (ou `caméra - 0x40` si elle regarde à gauche), en hauteur selon le rang de la caméra.
fn place_partner(x: &mut Cx, partner: crate::objects::Obj) {
    let side = if x.u8(f::FLIP) != 0 { (-0x40i16) as u16 } else { 0x1c0 };
    let px = camera_x(x).wrapping_add(side);
    partner.set_u16(x.b, 0x8, px);
    let cy = camera_y(x);
    let reach = if Fl::sub16(cy.wrapping_add(0x43), x.u16(0x10)).ge() { 0x60 } else { 0x28 };
    partner.set_u16(x.b, 0x10, cy.wrapping_add(reach));
}

/// 0x798ac : la seconde moitié entre en courant, glisse, puis attend la fin de la cinématique de son hôte.
fn partner_arrival(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            next7(x);
            x.set16(0x16, 0);
            let angle = if x.u8(f::FLIP) != 0 { 8 } else { 0x18 };
            x.c.d[0] = angle;
            x.set8(Obj::SPEED_INDEX, angle as u8);
            speed(x, 0x21);
            lib::sound(x, 0xe3d32);
            anim_kind(x, 0x32);
            entry(x, 3);
        }
        2 => {
            gallop_step(x);
            if reached(x, None) {
                start_slide(x, 0x20, false);
            }
        }
        4 => slide(x),
        6 => {
            if lib::step(x) {
                next7(x);
                x.set16(TIMER, 0x14);
                body(x, 4);
            }
        }
        8 => {
            if expired(x, TIMER) {
                next7(x);
                x.set16(TIMER, 0x5a);
                lib::sound(x, 0xe3bd6);
                body(x, 0xd);
            } else {
                x.anim_step();
            }
        }
        10 => {
            if expired(x, TIMER) {
                x.set8(0, 1);
                x.set8(ROLE, 0);
                lib::set_immunities(x);
                x.call(0xd5ef8);
                run_start(x);
            } else {
                x.anim_step();
            }
        }
        _ => {}
    }
}

// --- groupe 2 : combat -------------------------------------------------------------------------

pub(super) fn fight(x: &mut Cx) {
    if x.u8(3) == 0 && x.u8(TRANSFORMED) == 0 && Fl::sub16(x.u16(0x60) >> 1, x.u16(f::HP)).gt() {
        return transform(x);
    }
    match x.u8(f::SUB2) {
        0 => idle(x),
        2 => turn_around(x),
        4 => run_phase(x, 2),
        6 => run_phase(x, 3),
        8 => charge_phase(x),
        _ => {}
    }
}

/// 0x79a70 : debout, attend un temps tiré puis repart.
fn idle(x: &mut Cx) {
    if !lib::any_player_alive(x) {
        return idle_alone(x);
    }
    if x.u8(0x7) == 0 {
        next7(x);
        lib::draw_6e(x);
        x.c.d[0] = 0;
        x.set8(ANIM_KIND, 0);
        if x.u8(ANIM_LAST) != 0 {
            x.set8(ANIM_LAST, 0);
            body(x, 4);
        }
        return;
    }
    if lib::outside_band(x, -0x30, 0x1b0) {
        return charge(x, false);
    }
    if sub8(x, IDLE_COUNT, 1).le() {
        return run_start(x);
    }
    if !lib::faces_target(x) {
        return turn(x);
    }
    lib::step(x);
}

/// 0x79ad4 : demi-tour, puis l'état mémorisé est repris.
fn turn_around(x: &mut Cx) {
    if x.u8(0x7) == 0 {
        next7(x);
        let side = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, side);
        x.c.d[0] = 1;
        x.set8(ANIM_KIND, 1);
        return body(x, 5);
    }
    if !lib::step(x) {
        return;
    }
    if !lib::any_player_alive(x) {
        return idle_alone(x);
    }
    let saved = x.u32(SAVED_STATE);
    x.set32(f::STATE, saved);
    let last = x.u8(ANIM_LAST);
    if last == 2 || last == 3 {
        x.set8(ANIM_KIND, last);
        run_animation(x);
    } else {
        body(x, 4);
    }
}

/// 0x79bb8 : animation de course vers l'avant (6) ou vers l'arrière (7) selon le sens de la cible.
fn run_animation(x: &mut Cx) {
    let n = if lib::moves_forward(x) { 6 } else { 7 };
    x.c.d[0] = n;
    body(x, n);
}

/// 0x79c92 : choisit, dans la table de l'animation de course, l'entrée du geste en cours.
fn select_run(x: &mut Cx) {
    let set = if lib::moves_forward(x) { SET_RUN_BACK } else { SET_RUN_FORWARD };
    lib::switch_keeping_time(x, set);
}

/// 0x79b3a (kind 2) et 0x79b5c (kind 3) : course ; elle commence par tirer le nombre de pas et leur délai.
fn run_phase(x: &mut Cx, kind: u8) {
    if !lib::any_player_alive(x) {
        return idle_alone(x);
    }
    if x.u8(0x7) != 0 {
        return keep_running(x);
    }
    draw_steps(x, kind);
    walk_delay(x, kind);
    x.set8(ANIM_KIND, kind);
    next7(x);
    x.c.d[0] = 0;
    x.set16(0x16, 0);
    x.set16(0x1e, 0);
    if !pick_target(x) {
        return idle_alone(x);
    }
    lib::approach_target(x);
    speed(x, 8);
    let last = x.u8(ANIM_LAST);
    if last == 2 || last == 3 {
        let kind = x.u8(ANIM_KIND);
        x.set8(ANIM_LAST, kind);
        select_run(x);
    } else {
        let kind = x.u8(ANIM_KIND);
        x.set8(ANIM_LAST, kind);
        run_animation(x);
    }
}

/// 0x79bf0 : un pas de course ; tous les `+0x71` images la cible est reconsidérée et les attaques possibles sont
/// essayées dans l'ordre des portées.
fn keep_running(x: &mut Cx) {
    if sub8(x, WALK_DELAY, 1).mi() {
        if sub8(x, WALK_STEPS, 1).le() {
            return run_end(x);
        }
        lib::approach_target(x);
        speed(x, 8);
        let kind = x.u8(ANIM_KIND);
        walk_delay(x, kind);
        return select_run(x);
    }
    if lib::outside_band(x, -0x30, 0x1b0) {
        return charge(x, false);
    }
    if x.u8(ANIM_KIND) == 3 {
        if let Some(g) = close_gesture(x) {
            return from_near_gesture(x, g);
        }
    }
    if let Some(g) = far_gesture(x) {
        return from_far_gesture(x, g);
    }
    if lib::target_in_box(x, -0x10, 0x10, 8) {
        return from_front(x);
    }
    if lib::guard_in_front(x, -0x30, 0x18, 8) {
        return from_close(x);
    }
    if !lib::faces_target(x) {
        return turn(x);
    }
    if (x.u8(f::KEY_FRAME) as i8) >= 0 {
        x.move_by_table_xy();
    }
    lib::step(x);
}

/// 0x79cac : charge vers la caméra ou vers la position mémorisée de la cible.
fn charge_phase(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            next7(x);
            x.set16(0x16, 0);
            let mut angle = 0x18;
            if x.u8(CHARGE_AIMED) != 0 {
                mark_target(x);
                if x.u8(f::FLIP) != 0 {
                    angle = 8;
                }
            } else {
                let cam = camera_x(x);
                set_dw(x, 2, cam);
                x.set8(f::FLIP, 0);
                if !Fl::sub16(x.u16(0x8), cam).gt() {
                    x.set8(f::FLIP, 0x20);
                    angle = 8;
                }
            }
            x.c.d[0] = angle;
            x.set8(Obj::SPEED_INDEX, angle as u8);
            speed(x, 0x21);
            lib::debris(x, 1);
            lib::sound(x, 0xe3d32);
            anim_kind(x, 4);
            body(x, 8);
        }
        2 => {
            gallop_step(x);
            let limit = if x.u8(CHARGE_AIMED) != 0 { Some(x.u16(MARK_X)) } else { None };
            if reached(x, limit) && x.u8(f::KEY_FRAME) != 0 {
                start_slide(x, 0x20, false);
            }
        }
        4 => slide(x),
        6 => {
            if lib::step(x) {
                if x.u8(ROLE) == 1 {
                    return transform(x);
                }
                restart_or_idle(x);
            }
        }
        _ => {}
    }
}

// --- groupe 4 : attaques -------------------------------------------------------------------------

pub(super) fn attacks(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => simple_strike(x, 0x10, 0),
        2 => simple_strike(x, 0x11, 1),
        4 => simple_strike(x, 0x12, 2),
        6 => leap_strike(x),
        8 => arc_jump(x),
        10 => triple_strike(x),
        12 => spit(x),
        _ => {}
    }
}

/// 0x79e5a, 0x79ea0 et 0x79ece : un coup d'une animation (`n` du jeu des attaques).
fn simple_strike(x: &mut Cx, kind: u8, n: u32) {
    match x.u8(0x7) {
        0 => {
            next7(x);
            anim_kind(x, kind);
            entry(x, n);
        }
        2 => end_of_strike(x),
        _ => {}
    }
}

/// 0x79efc : bond sur la cible suivi d'un coup.
fn leap_strike(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            next7(x);
            x.set16(0x16, 0);
            mark_target(x);
            let angle = if x.u8(f::FLIP) != 0 { 8 } else { 0x18 };
            x.c.d[0] = angle;
            x.set8(Obj::SPEED_INDEX, angle as u8);
            speed(x, 0x2b);
            lib::debris(x, 1);
            lib::sound(x, 0xe3d32);
            anim_kind(x, 0x13);
            entry(x, 3);
        }
        2 => {
            gallop_step(x);
            let mark = x.u16(MARK_X);
            if reached(x, Some(mark)) && x.u8(f::KEY_FRAME) != 0 {
                x.set8(INVULNERABLE, 0);
                start_slide(x, 0x30, false);
            }
        }
        4 => slide(x),
        6 => {
            if lib::step(x) {
                restart_or_idle(x);
            }
        }
        _ => {}
    }
}

/// 0x7a034 : grand saut en arc vers la cible, retombée et rugissement.
fn arc_jump(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            next7(x);
            x.set16(0x16, 0);
            let angle = if x.u8(f::FLIP) != 0 { 8 } else { 0x18 };
            x.c.d[0] = angle;
            x.set8(Obj::SPEED_INDEX, angle as u8);
            speed(x, 0x17);
            let t = x.linked(f::TARGET);
            let mut gap = x.u32(0x8).wrapping_sub(t.u32(x.b, 0x8)) as i32;
            if gap < 0 {
                gap = gap.wrapping_neg();
            }
            let per_frame = ((x.u16(0x14) as i16 as i32) << 8).wrapping_abs();
            let mut lift: u16 = 0x400;
            // la vitesse verticale croît de 10 par durée de vol (en images) de l'écart horizontal
            while per_frame != 0 {
                let f = Fl::sub32(gap as u32, per_frame as u32);
                gap = gap.wrapping_sub(per_frame);
                if f.le() {
                    break;
                }
                lift = lift.wrapping_add(10);
            }
            x.c.d[0] = per_frame as u32;
            set_dw(x, 1, lift);
            x.c.d[2] = gap as u32;
            x.set16(0x18, lift);
            x.set16(0x1a, (-0x28i16) as u16);
            anim_kind(x, 0x14);
            entry(x, 4);
        }
        2 => {
            lib::step(x);
            if x.u8(f::KEY_FRAME) != 0 {
                return;
            }
            x.move_xz();
            if Fl::sub16(x.u16(0x18), 0x100).le() {
                next7(x);
                entry(x, 5);
            }
        }
        4 => {
            x.move_xz();
            if Fl::sub16(x.u16(0x18), (-0x100i16) as u16).le() {
                next7(x);
                x.set16(0x1a, (-0x40i16) as u16);
                entry(x, 6);
            }
        }
        6 => {
            x.move_xz();
            if (x.u16(0xc) as i16) <= 0 {
                next7(x);
                x.set16(0xc, 0);
                x.set16(TIMER, 0x14);
                lib::shake_58(x, 0x14);
                lib::debris(x, 2);
                lib::sound(x, 0xecb98);
                entry(x, 7);
            }
        }
        8 => {
            if expired(x, TIMER) {
                restart_or_idle(x);
            }
        }
        _ => {}
    }
}

/// 0x7a166 : trois coups enchaînés (nombre tiré dans la table 0x7a1ca d'après la difficulté et le nombre de joueurs).
fn triple_strike(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            next7(x);
            anim_kind(x, 0x15);
            entry(x, 8);
        }
        2 => {
            lib::step(x);
            if x.u8(f::KEY_FRAME) == 0 {
                return;
            }
            next7(x);
            x.set8(INVULNERABLE, 0);
            let level = (x.a5_u16(var::DIFFICULTY) & 0x1e) * 2;
            let mut players = x.a5_u8(var::PLAYERS_ALIVE);
            if players != 0 {
                players -= 1;
            }
            let i = level.wrapping_add(players as u16);
            x.c.d[0] = x.c.d[0] & 0xffff_0000 | i as u32;
            x.c.d[1] = players as u32;
            let n = x.rom8(0x7a1ca + i as u32);
            x.set8(COMBO, n);
        }
        4 => {
            lib::step(x);
            if x.u8(f::KEY_FRAME) == 0 {
                return;
            }
            if sub8(x, COMBO, 1).le() {
                next7(x);
                entry(x, 9);
            }
        }
        6 => {
            if lib::step(x) {
                if lib::any_player_alive(x) {
                    stand(x);
                } else {
                    idle_alone(x);
                }
            }
        }
        _ => {}
    }
}

/// 0x7a248 : crache un projectile (ESHL 0x43) devant lui.
fn spit(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            next7(x);
            anim_kind(x, 0x16);
            entry(x, 0xa);
        }
        2 => {
            if lib::step(x) {
                next7(x);
                x.set16(TIMER, 0x14);
                x.set8(VULNERABLE, 1);
                return body(x, 4);
            }
            if x.u8(f::KEY_FRAME) == 0 {
                return;
            }
            x.set8(f::KEY_FRAME, 0);
            let Some(shot) = x.alloc(&pools::ESHL) else { return };
            shot.set_u8(x.b, 0, 1);
            shot.set_u8(x.b, 2, 0x43);
            for off in [0x8, 0xc, 0x10] {
                let v = x.u16(off);
                shot.set_u16(x.b, off, v);
            }
            let side = x.u8(f::FLIP);
            shot.set_u8(x.b, f::FLIP, side);
            let owner = x.u16(f::OWNER);
            shot.set_u16(x.b, f::OWNER, owner);
            add16(x, shot, 0xc, 0x30);
            let dx = if side != 0 { 0x60u16 } else { (-0x60i16) as u16 };
            set_dw(x, 0, dx);
            add16(x, shot, 0x8, dx);
        }
        4 => {
            if expired(x, TIMER) {
                restart_or_idle(x);
            } else {
                lib::step(x);
            }
        }
        _ => {}
    }
}

// --- groupes 6 à 10 : attente --------------------------------------------------------------------

/// 0x7a30c : tant qu'aucun joueur n'est en vie, le boss attend en pose ; dès qu'un joueur revient, il repart.
pub(super) fn wait_for_players(x: &mut Cx) {
    if (x.a5_u8(var::PLAYERS_ALIVE) as i8) > 0 {
        return run_start(x);
    }
    match x.u8(f::SUB2) {
        0 => {
            next6(x);
            x.set8(ANIM_LAST, 0xff);
            body(x, 0x11);
        }
        2 => {
            lib::step(x);
        }
        _ => {}
    }
}
