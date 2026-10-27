//! Boss 4, groupes 0 (arrivée), 2 (combat) et 6 à 12 (attente sans joueur).
//!
//! | `+6` du groupe 2 | Rôle |
//! |---|---|
//! | 0 | debout, attend (`+0x6e` images) |
//! | 2 | demi-tour, puis reprise de l'état mémorisé |
//! | 4, 6 | marche vers la cible (pas rapides, pas lents) ; les boîtes devant le boss décident de l'attaque |
//! | 8 | charge (vers le milieu de l'écran ou la cible) |
//! | 10 | bond en arrière |

use super::decide;
use super::*;
use crate::objects::Courant;

/// Jeux de marche choisis par la marque `+0x33` (0x103e0) : de côté, de face.
const WALK_SIDEWAYS: u32 = 0x17569c;
const WALK_FORWARD: u32 = 0x175740;

// --- groupe 0 : arrivée ------------------------------------------------------------------------

/// 0x81daa : le boss paraît en haut de l'écran, descend, rugit, puis se met en marche.
pub(super) fn intro(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set_bit(0x1, 1, false);
            let cx = var::CAMERA1_X.get(x.b).wrapping_add(0x120);
            let cy = var::CAMERA1_Y.get(x.b);
            setw(x, 0, cx);
            setw(x, 1, cy.wrapping_add(0x40));
            x.set16(0x8, cx);
            x.set16(0x10, cy.wrapping_add(0x40));
            let h = cy.wrapping_add(0xe0).wrapping_sub(x.u16(0x10));
            setw(x, 0, h);
            x.set16(0xc, h);
            lib::sound(x, 0xe3f76);
            x.c.d[0] = 1;
            air(x, 1);
        }
        2 => {
            let z = x.u32(0xc);
            x.set32(0xc, z.wrapping_sub(0x18000));
            if Fl::sub32(z, 0x18000).gt() {
                lib::step(x);
                return;
            }
            x.add8(f::SUB2, 2);
            x.set16(0xc, 0);
            x.set16(TIMER, 0x28);
            lib::shake_54(x, 0xa);
            lib::debris(x, 5);
            body(x, 0);
        }
        _ => {
            if !expired(x, TIMER) {
                lib::step(x);
                return;
            }
            lib::create_set_14(x);
            lib::set_immunities(x);
            x.set8(0, 1);
            decide::walk(x);
        }
    }
}

// --- groupe 2 : combat -----------------------------------------------------------------------

pub(super) fn run(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => idle(x),
        2 => turn(x),
        4 => walk(x, false),
        6 => walk(x, true),
        8 => charge(x),
        _ => hop(x),
    }
}

/// La cible est-elle hors de la zone `[caméra - 0x50, caméra + 0x1d0]` (0x104a2) ?
fn off_screen(x: &mut Cx) -> bool {
    lib::outside_band(x, -0x50, 0x1d0)
}

/// Le boss doit-il se retourner vers sa cible (0x1050c) ?
fn needs_turn(x: &mut Cx) -> bool {
    !lib::faces_target(x)
}

/// Avance-t-il dans le sens où il regarde (0x10560) ?
fn facing_forward(x: &mut Cx) -> bool {
    lib::moves_forward(x)
}

/// 0x81e7a : debout ; sorti de l'écran il charge, au bout de l'attente il marche, sinon il se tourne vers sa cible.
fn idle(x: &mut Cx) {
    if !alive(x) {
        return decide::wait(x);
    }
    if x.u8(0x7) == 0 {
        x.add8(0x7, 2);
        lib::draw_6e(x);
        x.c.d[0] = 0;
        x.set8(ANIM_KIND, 0);
        if x.u8(ANIM_LAST) != 0 {
            x.set8(ANIM_LAST, 0);
            body(x, 0);
        }
        return;
    }
    if off_screen(x) {
        return decide::charge(x, false);
    }
    if !sub8(x, IDLE_COUNT, 1).gt() {
        return decide::walk(x);
    }
    if needs_turn(x) {
        return decide::turn(x);
    }
    lib::step(x);
}

/// Animation de marche 2 (de face) ou 3 (de côté).
fn walk_anim(x: &mut Cx) {
    x.c.d[0] = 2;
    let n = if facing_forward(x) { 2 } else { 3 };
    x.c.d[0] = n;
    body(x, n);
}

/// Animation de marche reprise à la même marque dans le jeu du sens de marche.
fn walk_select(x: &mut Cx) {
    let set = if facing_forward(x) { WALK_FORWARD } else { WALK_SIDEWAYS };
    lib::switch_keeping_time(x, set);
}

/// 0x81ede : demi-tour (animation 1), puis reprise de l'état mémorisé.
fn turn(x: &mut Cx) {
    if x.u8(0x7) == 0 {
        x.add8(0x7, 2);
        let side = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, side);
        x.set8(ANIM_KIND, 1);
        x.c.d[0] = 1;
        return body(x, 1);
    }
    if !lib::step(x) {
        return;
    }
    if !alive(x) {
        return decide::wait(x);
    }
    let s = x.u32(SAVED_STATE);
    x.set32(f::STATE, s);
    let last = x.u8(ANIM_LAST);
    if last == 2 || last == 3 {
        x.set8(ANIM_KIND, last);
        walk_anim(x);
    } else {
        x.c.d[0] = 0;
        body(x, 0);
    }
}

/// 0x81f44 (`slow` = faux) et 0x81f66 : marche vers la cible, `+0x6b` pas espacés de `+0x71` images.
fn walk(x: &mut Cx, slow: bool) {
    if !alive(x) {
        return decide::wait(x);
    }
    if x.u8(0x7) != 0 {
        return walking(x);
    }
    let (steps, delay, k) = if slow { (0x83074, 0x83010, 3) } else { (0x83054, 0x82ff0, 2) };
    let r = rand_and(x, 0x1f);
    let n = x.rom8(steps + r);
    x.set8(WALK_STEPS, n);
    set_delay(x, delay);
    x.set8(ANIM_KIND, k);
    x.add8(0x7, 2);
    x.c.d[0] = 0;
    x.set16(0x16, 0);
    x.set16(0x1e, 0);
    if !pick_target(x) {
        return decide::wait(x);
    }
    aim(x);
    let last = x.u8(ANIM_LAST);
    x.set8(ANIM_LAST, k);
    if last == 2 || last == 3 {
        walk_select(x);
    } else {
        walk_anim(x);
    }
}

/// Délai entre deux pas, selon la difficulté (table de la ROM `table`).
fn set_delay(x: &mut Cx, table: u32) {
    let level = var::RANK.get(x.b);
    setw(x, 0, level);
    let d = x.rom8(table + level as u32);
    x.set8(WALK_DELAY, d);
}

/// Vise la cible (0xe76c) et prend la table de vitesses 0xd.
fn aim(x: &mut Cx) {
    lib::approach_target(x);
    speed(x, 0xd);
}

/// La marche : un pas toutes les `+0x71` images ; entre deux pas, les boîtes devant le boss décident de la suite.
fn walking(x: &mut Cx) {
    if sub8(x, WALK_DELAY, 1).mi() {
        if !sub8(x, WALK_STEPS, 1).gt() {
            return decide::walk_slow(x);
        }
        aim(x);
        let slow = x.u8(ANIM_KIND) == 3;
        set_delay(x, if slow { 0x83010 } else { 0x82ff0 });
        return walk_select(x);
    }
    if off_screen(x) {
        return decide::charge(x, false);
    }
    // marche lente : boîte proche de l'IA (0x83094)
    if x.u8(ANIM_KIND) == 3 {
        if let Some(g) = ai_box(x, -0x68, 0x38, 6) {
            return decide::from_ai(x, g, true);
        }
    }
    if let Some(g) = ai_box(x, -0xd0, 0x20, 0xc) {
        return decide::from_ai(x, g, false);
    }
    if lib::target_in_box(x, -0x19, 0x18, 0xe) {
        return decide::from_close(x);
    }
    if target_in_box_7ef4(x, -0x70, 0x10, 8) {
        return decide::from_wide_box(x);
    }
    if needs_turn(x) {
        return decide::turn(x);
    }
    if (x.u8(f::KEY_FRAME) as i8) >= 0 {
        x.move_by_table_xy();
    }
    lib::step(x);
}

/// 0x83094 / 0x830b2 : la cible est dans la boîte et l'IA (0x1047a) choisit un geste (1 ou 2).
fn ai_box(x: &mut Cx, d0: i16, d1: u16, d2: u16) -> Option<u16> {
    if !lib::target_in_box(x, d0, d1, d2) {
        return None;
    }
    let turn = lib::turn_decision(x);
    (turn != lib::Turn::None).then_some(turn as u16)
}

/// Boîte large devant le boss (0x7ef4).
fn target_in_box_7ef4(x: &mut Cx, d0: i16, d1: u16, d2: u16) -> bool {
    lib::guard_in_front(x, d0, d1, d2)
}

/// 0x820b6 : charge en ligne droite jusqu'au point visé, poussière et grondement, puis marche.
fn charge(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            x.set16(0x16, 0);
            let mut angle = 0x18;
            if x.u8(CHARGE_AT_TARGET) == 0 {
                // vers le milieu de l'écran
                x.set8(f::FLIP, 0);
                let px = x.u16(0x8);
                setw(x, 1, px);
                if !Fl::sub16(px, var::CAMERA1_X.get(x.b)).gt() {
                    x.set8(f::FLIP, 0x20);
                    angle = 8;
                }
            } else {
                let t = load_target(x);
                let tx = t.u16(x.b, 0x8);
                x.set16(MARK_X, tx);
                if x.u8(f::FLIP) != 0 {
                    angle = 8;
                }
            }
            x.set8(Obj::SPEED_INDEX, angle);
            speed(x, 0x35);
            kind(x, 4);
            body(x, 4);
        }
        2 => charging(x),
        _ => {
            dust_tick(x);
            if !expired(x, TIMER) {
                x.move_x();
                return;
            }
            if !alive(x) {
                return decide::wait(x);
            }
            decide::walk(x);
        }
    }
}

/// 0x82128 : la charge avance jusqu'au point visé ; à l'arrivée (marque négative) le boss freine.
fn charging(x: &mut Cx) {
    x.anim_step();
    let key = x.u8(f::KEY_FRAME);
    if key != 0x80 {
        x.move_x();
        let (goal, px) = if x.u8(CHARGE_AT_TARGET) == 0 {
            let cam = var::CAMERA1_X.get(x.b);
            let d = if (x.u16(0x14) as i16) < 0 { 0xf0 } else { 0x90 };
            (cam.wrapping_add(d), x.u16(0x8))
        } else {
            (x.u16(MARK_X), x.u16(0x8))
        };
        setw(x, 0, goal);
        setw(x, 1, px);
        // vers la gauche, les rôles s'échangent (`exg D0,D1`)
        if (x.u16(0x14) as i16) < 0 {
            x.c.d.swap(0, 1);
        }
        let (a, b) = (x.c.d[0] as u16, x.c.d[1] as u16);
        if !Fl::sub16(b, a).lt() && (x.u8(f::KEY_FRAME) as i8) < 0 {
            return brake(x);
        }
    }
    if x.u8(f::KEY_FRAME) == 0x84 {
        lib::sound(x, 0xecdba);
        lib::debris(x, 4);
        lib::shake_54(x, 0xa);
    }
}

/// 0x82196 : freinage (animation 5, 0x14 images, recul de 0x40).
fn brake(x: &mut Cx) {
    x.add8(0x7, 2);
    x.set16(TIMER, 0x14);
    x.set16(DUST, 0);
    let mut d = 0x40u32;
    if x.u8(f::FLIP) != 0 {
        d = d & 0xffff_0000 | (d as u16).wrapping_neg() as u32;
    }
    x.c.d[0] = d;
    x.set16(0x16, d as u16);
    lib::shake_54(x, 0x14);
    lib::sound(x, 0xec976);
    lib::sound(x, 0xecdba);
    x.c.d[0] = 5;
    body(x, 5);
}

/// 0x82202 : bond en arrière (élan, saut, chute, glissade).
fn hop(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            kind(x, 5);
            x.c.d[0] = 6;
            body(x, 6);
        }
        2 => {
            lib::step(x);
            if x.u8(f::KEY_FRAME) == 0 {
                return;
            }
            x.add8(0x7, 2);
            let (angle, vy) = if x.u8(f::FLIP) != 0 { (0x18u8, 0x20u16) } else { (8, (-0x20i16) as u16) };
            x.c.d[0] = angle as u32;
            setw(x, 1, vy);
            x.set8(Obj::SPEED_INDEX, angle);
            x.set16(0x16, vy);
            lib::drift_heading(x);
            speed(x, 0x35);
            x.set16(0x18, 0x300);
            x.set16(0x1a, (-0x40i16) as u16);
            x.move_xz();
        }
        4 => {
            x.move_xz();
            if (x.u16(0x18) as i16) <= 0 {
                x.add8(0x7, 2);
                x.set16(0x16, 0);
            }
        }
        6 => {
            x.move_xz();
            if (x.u16(0xc) as i16) > 0 {
                lib::step(x);
                return;
            }
            x.add8(0x7, 2);
            x.set16(TIMER, 0x14);
            x.set16(DUST, 0);
            x.set16(0xc, 0);
            let mut d = (-0x40i16) as u16;
            if x.u8(f::FLIP) != 0 {
                d = d.wrapping_neg();
            }
            setw(x, 0, d);
            x.set16(0x16, d);
            x.move_x();
            x.c.d[0] = 7;
            body(x, 7);
        }
        _ => {
            dust_tick(x);
            if !expired(x, TIMER) {
                x.move_x();
                lib::step(x);
                return;
            }
            if !alive(x) {
                return decide::wait(x);
            }
            decide::walk(x);
        }
    }
}

// --- groupes 6 à 12 : attente ---------------------------------------------------------------

/// 0x82be2 : sans joueur en vie, le boss se tient debout ; dès qu'un joueur revient (en partie), il marche.
pub(super) fn wait_for_players(x: &mut Cx) {
    if var::GAME_ON.get(x.b) != 0 && (var::PLAYERS_ALIVE.get(x.b) as i8) > 0 {
        return decide::walk(x);
    }
    if x.u8(f::SUB2) == 0 {
        x.add8(f::SUB2, 2);
        x.set8(ANIM_LAST, 0xff);
        x.c.d[0] = 0;
        body(x, 0);
    } else {
        lib::step(x);
    }
}
