//! Boss 22, états actifs de déplacement : arrivée (`+5` = 0), combat (2), courses et sauts (6 et 8), pause (10), joueur
//! saisi (12), transformation (14).

use super::*;
use crate::objects::Courant;

/// Animation du corps `n` pour l'animation `id`, avec les réglages sonores `(0x27, 5)` et une vitesse `speed`.
fn body_pose<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>, id: u8, n: u32, speed_n: u32) {
    sounds(x, 0x27, 5);
    pose(x, Set::A, id, n);
    speed(x, speed_n);
}

/// 0x1a56 : un pas d'animation (la retenue, fin du script, n'est pas utilisée).
pub(super) fn step_anim<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) {
    x.anim_step();
}

// --- groupe 0 : arrivée ------------------------------------------------------------------------

/// 0x9b066 : attend (`+0xbc` images), puis tombe du haut de l'écran.
pub(super) fn arrive(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            if sub16(x, SPOT_TIMER, 1).pl() {
                return;
            }
            next7(x);
            x.set8(VULNERABLE, 0);
            x.set8(f::FLIP, 0);
            let px = var::SCROLL_X.get(x.b).wrapping_add(0x80);
            let pz = var::SCROLL_Y.get(x.b).wrapping_add(0x90);
            x.set16(0x8, px);
            x.set16(0x10, pz);
            x.set16(0xc, 0x120);
            x.set16(0x14, 0);
            x.set16(0x18, (-0x600i16) as u16);
            x.set16(0x1a, (-0x50i16) as u16);
            x.set16(0x16, 0);
            sounds(x, 0x2a, 5);
            x.set8(ANIM_ID, 7);
            x.set8(ANIM_LAST, 7);
            start(x, Set::A, 0x1c);
        }
        2 => {
            x.move_xz();
            if (x.u32(0xc) as i32) > 0 {
                return step_anim(x);
            }
            lib::shake_5c(x, 0x1e);
            lib::set_immunities(x);
            x.set32(0xc, 0);
            dust_line(x);
            next7(x);
            x.set16(0x30, 1);
            x.set8(VULNERABLE, 0);
            start(x, Set::A, 6);
        }
        4 => {
            if x.anim_step() {
                x.set8(f::SUB, 2);
                x.set8(0x7, 0);
                x.set8(VULNERABLE, 1);
            }
        }
        _ => {}
    }
}

// --- groupe 2 : combat ------------------------------------------------------------------------

/// 0x9b14e.
pub(super) fn fight(x: &mut Cx) {
    x.set8(HIT_TICK, 0);
    if !two(x) {
        x.set8(0xad, 0);
    }
    match x.u8(f::SUB2) {
        0 | 8 => stand(x),
        2 => turn(x),
        4 => dash(x),
        6 => leap(x),
        _ => {}
    }
    if var::PLAYERS_ALIVE.get(x.b) == 0 {
        x.set32(f::STATE, state::IDLE);
    }
}

/// Une cible au hasard (424e ou 44ee) d'après un tirage ; le résultat n'est pas utilisé.
fn pick_some_target(x: &mut Cx) {
    let r = rand_and(x, 0xf);
    if 0xffc0u32 >> r & 1 != 0 {
        lib::pick_target(x);
    } else {
        lib::random_player(x);
    }
}

/// 0x9b180 : debout ; la marche tire sa durée dans `+0x6b`, `+0x71`, puis une attaque est choisie selon la portée.
fn stand(x: &mut Cx) {
    if target_far(x) {
        return x.set32(f::STATE, state::LEAP);
    }
    if x.u8(0x7) == 0 {
        next7(x);
        x.set8(VULNERABLE, 1);
        sounds(x, 0x27, 5);
        pose(x, Set::A, 1, 2);
        speed(x, 0x15);
        return lib::draw_stance(x);
    }
    rubble(x, 0, 0x201, 0x20, 0x20);
    if sub8(x, WALK_DELAY, 1).mi() {
        lib::draw_stance(x);
        pick_some_target(x);
        let kind = if x.u8(f::SUB2) == 0 { 0x23 } else { 0x24 };
        x.set8(0x79, kind);
        lib::approach_target(x);
        if !sub8(x, WALK_STEPS, 1).ne() {
            lib::draw_6b(x);
            if lib::maybe_draw_6e(x) {
                x.set16(f::SUB2, 0x400);
                return;
            }
        }
    }
    if lib::target_in_box(x, -0x20, 0x20, 0x10) {
        x.set32(f::STATE, 0x0204_0000);
        return lib::pick_byte(x);
    }
    if hunt_close(x) || hunt_far(x) {
        return;
    }
    if lib::guard_in_front_strict(x, -0x60, 0x60, 0x10) {
        return lib::pick_byte(x);
    }
    step_anim(x);
    x.move_by_table_xy();
    if x.u16(f::TARGET) == 0 {
        return;
    }
    let t = x.linked(f::TARGET);
    let wanted = if (x.u16(0x8).wrapping_sub(t.u16(x.b, 0x8)) as i16) < 0 { 0x20 } else { 0 };
    if wanted != x.u8(f::FLIP) {
        x.set32(SAVED_STATE, state::STAND);
        x.set8(SAVED_STEP, 0);
        x.set32(f::STATE, state::TURN);
    }
}

/// 0x9b3fe : demi-tour.
fn turn(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            next7(x);
            x.set8(VULNERABLE, 1);
            let angle = if x.u8(f::FLIP) != 0 { 8 } else { 0x18 };
            x.set8(0x2d, angle);
            pose(x, Set::A, 1, 2);
            speed(x, 0x15);
        }
        2 => {
            if x.u16(f::TARGET) != 0 && turn_not_needed(x) {
                step_anim(x);
                x.move_by_table_xy();
                return;
            }
            next7(x);
            x.set8(VULNERABLE, 1);
            x.set8(ANIM_ID, 4);
            x.set8(ANIM_LAST, 4);
            start(x, Set::A, 1);
        }
        4 => {
            if x.anim_step() {
                let s = x.u32(SAVED_STATE);
                x.set32(f::STATE, s);
                let step = x.u8(SAVED_STEP);
                x.set8(f::SUB2, step);
                let side = x.u8(f::FLIP) ^ 0x20;
                x.set8(f::FLIP, side);
                x.set8(ANIM_ID, 0);
                x.set8(ANIM_LAST, 0);
                x.c.d[0] = 0;
                start(x, Set::A, 0);
            }
        }
        _ => {}
    }
}

/// 0x9b45c : le demi-tour continue tant que le boss est dans l'écran et que la cible est assez proche devant lui.
fn turn_not_needed<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) -> bool {
    let cam = var::SCROLL_X.get(x.b);
    if x.u8(f::FLIP) != 0 && (cam.wrapping_add(0x160).wrapping_sub(x.u16(0x8)) as i16) < 0 {
        return false;
    }
    if (cam.wrapping_add(0x20).wrapping_sub(x.u16(0x8)) as i16) >= 0 {
        return false;
    }
    let t = x.linked(f::TARGET);
    let d = t.u16(x.b, 0x8).wrapping_sub(x.u16(0x8)).wrapping_add(0x70);
    d <= 0xe0 && x.u8(0x29) == 0
}

/// 0x9b51a : élan ; avec une chance sur huit, le boss se laisse tomber (état touché).
fn dash(x: &mut Cx) {
    if x.u8(0x7) != 0 {
        if sub8(x, IDLE_COUNT, 1).mi() {
            return x.set32(f::STATE, state::STAND);
        }
        if needs_turn(x) {
            x.set32(SAVED_STATE, state::STAND);
            x.set8(SAVED_STEP, 4);
            x.set32(f::STATE, state::TURN);
        }
        return step_anim(x);
    }
    if x.u8(RAGE) == 0xff {
        let r = rand_and(x, 0xf);
        let dash = kit(x).dash;
        if x.rom8(dash + r) != 0 {
            x.set32(f::STATE, 0x0400_0000);
            x.set8(RAGE, 2);
            return;
        }
    }
    next7(x);
    sub8(x, RAGE, 1);
    if !Fl::sub8(x.u8(RAGE), 0xff).ge() {
        x.set8(RAGE, 0xff);
    }
    x.set8(VULNERABLE, 1);
    x.set8(ANIM_ID, 0);
    sounds(x, 0x27, 5);
    if x.u8(ANIM_ID) != x.u8(ANIM_LAST) {
        x.set8(ANIM_LAST, 0);
        x.c.d[0] = 0;
        start(x, Set::A, 0);
    }
}

/// 0x9b5e6 : bond vers la cible, qui se retourne si besoin.
fn leap(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            next7(x);
            x.set16(TIMER, 0x1e);
            lib::draw_stance(x);
            leap_pose(x);
        }
        4 => {
            x.add8(0x7, 0xfe);
            leap_pose(x);
        }
        2 => {
            rubble(x, 0, 0x201, 0x20, 0x20);
            if sub16(x, TIMER, 1).mi() {
                return x.set32(f::STATE, state::STAND);
            }
            if sub8(x, WALK_DELAY, 1).mi() {
                lib::draw_stance(x);
                pick_some_target(x);
                x.set8(0x79, 0x23);
                lib::approach_target(x);
            }
            if needs_turn(x) {
                x.set32(SAVED_STATE, 0x0202_0004);
                let step = x.u8(f::SUB2);
                x.set8(SAVED_STEP, step);
                x.set32(f::STATE, state::TURN);
            }
            x.move_by_table_xy();
            step_anim(x);
        }
        _ => {}
    }
}

fn leap_pose<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) {
    x.set8(VULNERABLE, 1);
    body_pose(x, 3, 4, 0x1f);
}

// --- groupes 6 et 8 : course vers un point de l'écran -------------------------------------------

/// 0x9c506 : course vers le point choisi (`+6` = 0), pause (2), bond (4 et 6).
pub(super) fn run_to_spot(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => spot_run(x),
        2 => spot_pause(x),
        4 => spot_leap_a(x),
        6 => spot_leap_b(x),
        _ => {}
    }
}

/// 0x9c90c : même chose après une attaque ; `+6` = 0 est une courte attente.
pub(super) fn run_to_spot_after_attack(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => spot_wait(x),
        2 => spot_pause(x),
        4 => spot_run(x),
        6 => spot_leap_a(x),
        8 => spot_leap_b(x),
        _ => {}
    }
}

/// 0x9c586.
fn spot_run(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            next7(x);
            x.set16(TIMER, 6);
            x.set16(TIMER2, 0x12c);
            choose_spot(x);
            x.set8(VULNERABLE, 1);
            body_pose(x, 2, 2, 0x15);
            x.move_by_table_xy();
            step_anim(x);
        }
        2 => {
            rubble(x, 0, 0x201, 0x20, 0x20);
            let timer = sub16(x, TIMER2, 1);
            if timer.mi() || lib::target_in_box(x, 0xc0, 0xc0, 0x10) {
                return x.set32(f::STATE, state::STAND);
            }
            let dx = x.u16(SPOT_X).wrapping_sub(x.u16(0x8)).wrapping_add(8);
            let dz = x.u16(SPOT_Z).wrapping_sub(x.u16(0x10)).wrapping_add(8);
            if dx <= 0x10 && dz <= 0x10 {
                return x.set32(f::STATE, state::STAND);
            }
            if sub16(x, TIMER, 1).mi() {
                let a = x.u8(0x2d);
                let a = if x.u8(SPIN) != 0 { a.wrapping_sub(1) } else { a.wrapping_add(1) };
                x.set8(0x2d, a & 0x1f);
                x.set16(TIMER, 6);
            }
            x.move_by_table_xy();
            step_anim(x);
        }
        _ => {}
    }
}

/// 0x9c69c : marche courte vers l'avant puis retour au combat.
fn spot_pause(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            next7(x);
            x.set16(TIMER, 0x1e);
            let angle = if x.u8(f::FLIP) != 0 { 0x18 } else { 8 };
            x.set8(0x2d, angle);
            x.set8(VULNERABLE, 1);
            body_pose(x, 3, 3, 0x15);
        }
        2 => {
            rubble(x, 0, 0x202, 0x20, 0x20);
            if sub8(x, TIMER, 1).mi() {
                return x.set32(f::STATE, state::STAND);
            }
            x.move_by_table_xy();
            step_anim(x);
        }
        _ => {}
    }
}

/// 0x9c924 : courte attente avant la course (`+0xa0` images), puis retour au combat ou demi-tour.
fn spot_wait(x: &mut Cx) {
    match x.u8(0x7) {
        0 | 4 => {
            if x.u8(0x7) == 0 {
                next7(x);
                x.set16(TIMER, 0x14);
            } else {
                x.add8(0x7, 0xfe);
            }
            x.set8(VULNERABLE, 1);
            x.set8(ANIM_ID, 0);
            sounds(x, 0x27, 5);
            let id = x.u8(ANIM_ID);
            x.set8(ANIM_LAST, id);
            x.c.d[0] = 0;
            start(x, Set::A, 0);
        }
        2 => {
            if sub16(x, TIMER, 1).mi() {
                return x.set32(f::STATE, state::STAND);
            }
            if needs_turn(x) {
                x.set32(SAVED_STATE, 0x0208_0004);
                let step = x.u8(f::SUB2);
                x.set8(SAVED_STEP, step);
                x.set32(f::STATE, state::TURN);
                return;
            }
            step_anim(x);
        }
        _ => {}
    }
}

/// Étape commune des bonds : attend l'image-clé, monte, redescend, retombe.
fn leap_rise_wait<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) {
    if x.u8(f::KEY_FRAME) != 0 {
        next7(x);
        x.set8(VULNERABLE, 0);
    }
    step_anim(x);
}

fn leap_climb(x: &mut Cx, f_move: fn(&mut Cx), up: bool) {
    f_move(x);
    let vz = x.u16(0x18);
    let reached = if up { Fl::sub16(vz, 0x100).gt() } else { Fl::sub16(vz, 0xff00).gt() };
    if !reached {
        next7(x);
        x.set16(0x30, 1);
    }
    step_anim(x);
}

fn leap_land(x: &mut Cx, f_move: fn(&mut Cx)) {
    f_move(x);
    if (x.u32(0xc) as i32) > 0 {
        return step_anim(x);
    }
    lib::shake_54(x, 0x1e);
    lib::sound(x, 0xecd50);
    x.set32(0xc, 0);
    dust_line(x);
    next7(x);
    x.set16(0x30, 1);
    x.set8(VULNERABLE, 1);
    start(x, Set::A, 6);
}

fn leap_end<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) {
    if x.anim_step() {
        x.set8(VULNERABLE, 1);
        x.set32(f::STATE, state::STAND);
    }
}

/// 0x9c74a : grand bond vers l'avant (vitesse horizontale).
fn spot_leap_a(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            next7(x);
            x.set8(VULNERABLE, 1);
            let v = if x.u8(f::FLIP) != 0 { (-0x380i16) as u16 } else { 0x380 };
            x.set16(0x14, v);
            let rise = if two(x) { 0x500 } else { 0x700 };
            x.set16(0x18, rise);
            x.set16(0x1a, (-0x50i16) as u16);
            x.set8(ARM_TIMER, 0x1e);
            x.set8(ANIM_ID, 0x28);
            x.set8(ANIM_LAST, 0x28);
            sounds(x, 0x2a, 6);
            start(x, Set::A, 5);
        }
        2 => leap_rise_wait(x),
        4 => leap_climb(x, |x| { x.move_xz(); }, true),
        6 => leap_climb(x, |x| { x.move_xz(); }, false),
        8 => leap_land(x, |x| { x.move_xz(); }),
        10 => leap_end(x),
        _ => {}
    }
}

/// 0x9c7d2 : grand bond de côté (vitesse en profondeur tirée au hasard).
fn spot_leap_b(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            next7(x);
            x.set8(VULNERABLE, 1);
            let r = rand_and(x, 0xf);
            let mut v = if 0xff00u32 >> r & 1 != 0 { (-0x280i16) as u16 } else { r as u16 };
            if x.u8(f::FLIP) == 0 {
                v = v.wrapping_neg();
            }
            set_dw(x, 0, v);
            x.set16(0x1c, v);
            x.set16(0x18, 0x500);
            x.set16(0x1a, (-0x50i16) as u16);
            x.set8(ARM_TIMER, 0x1e);
            x.set8(ANIM_ID, 0x28);
            x.set8(ANIM_LAST, 0x28);
            sounds(x, 0x2a, 6);
            start(x, Set::A, 5);
        }
        2 => leap_rise_wait(x),
        4 => leap_climb(x, |x| { x.move_yz(); }, true),
        6 => leap_climb(x, |x| { x.move_yz(); }, false),
        8 => leap_land(x, |x| { x.move_yz(); }),
        10 => leap_end(x),
        _ => {}
    }
}

// --- groupe 10 : pause ----------------------------------------------------------------------------

/// 0x9ca0a : sans joueur en vie, le boss attend (animation de repos de sa forme) ; sinon il reprend.
pub(super) fn paused<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) {
    if var::PLAYERS_ALIVE.get(x.b) != 0 {
        if second_form(x) {
            return x.set32(f::STATE, 0x0204_1000);
        }
        x.set32(f::STATE, state::STAND);
        x.set8(SECOND_FX, 0);
        return x.set_a5_u16(0x51e, 0);
    }
    if second_form(x) {
        if x.u8(f::SUB2) == 0 {
            x.set8(VULNERABLE, 1);
            x.set8(ANIM_ID, 6);
            if x.u8(ANIM_ID) != x.u8(ANIM_LAST) {
                x.set8(ANIM_LAST, 6);
                start(x, Set::A, 0x14);
            }
            step_anim(x);
        }
    } else if x.u8(f::SUB2) == 0 {
        x.set8(VULNERABLE, 1);
        x.set8(ANIM_ID, 0);
        if x.u8(ANIM_ID) != x.u8(ANIM_LAST) {
            x.set8(ANIM_LAST, 0);
            start(x, Set::A, 0);
        }
        step_anim(x);
    }
}

// --- groupe 12 : joueur saisi ---------------------------------------------------------------------

/// 0x9cade.
pub(super) fn holding(x: &mut Cx) {
    if x.u8(f::SUB2) != 0 {
        return;
    }
    if x.u16(f::TARGET) as i16 >= 0 {
        return x.set32(f::STATE, state::STAND);
    }
    let prey = x.linked(f::TARGET);
    if prey.u8(x.b, 0) == 0 || prey.u8(x.b, 0x188) == 0 {
        return x.set32(f::STATE, state::STAND);
    }
    match x.u8(0x7) {
        0 => {
            next7(x);
            let z = prey.u16(x.b, 0x10);
            x.set16(0x10, z);
            x.c.a[1] = prey.0;
            if prey.u16(x.b, 0xc) != 0 {
                return lib::release_player(x, prey);
            }
            let h = prey.u16(x.b, 0xc);
            x.set16(0xc, h);
            x.set8(BITES, 8);
            start(x, Set::B, 9);
            lib::hold_seized(x);
        }
        2 => {
            step_anim(x);
            if x.u8(f::KEY_FRAME) == 0 {
                return lib::hold_seized(x);
            }
            x.set8(f::KEY_FRAME, 0);
            if sub8(x, BITES, 1).le() {
                return release_prey(x);
            }
            x.set8(0x53, 8);
            x.c.a[1] = prey.0;
            x.c.d[1] = 2;
            let mut dx = 0x4eu16;
            if x.u8(f::FLIP) == 0 {
                dx = dx.wrapping_neg();
            }
            let pos = (dx.wrapping_add(x.u16(0x8)), x.u16(0xc).wrapping_add(0x38), x.u16(0x10).wrapping_sub(1));
            (x.c.d[2], x.c.d[3], x.c.d[4]) = (pos.0 as u32, pos.1 as u32, pos.2 as u32);
            crate::families::effects_creators::spark_at(x.b, x.o, prey.0, 2, pos);
            if (prey.u16(x.b, f::HP) as i16) <= 1 {
                return release_prey(x);
            }
            lib::sound(x, 0xedd96);
            let hp = prey.u16(x.b, f::HP).wrapping_sub(4);
            prey.set_u16(x.b, f::HP, hp);
            if (hp as i16) > 0 {
                return lib::hold_seized(x);
            }
            prey.set_u16(x.b, f::HP, 1);
            release_prey(x);
        }
        _ => {}
    }
}

fn release_prey(x: &mut Cx) {
    let prey = x.linked(f::TARGET);
    prey.set_u8(x.b, 0x6f, 1);
    lib::release_player(x, prey);
}

// --- groupe 14 : transformation en seconde forme ----------------------------------------------------

/// 0xffda : tirage dont les chances (un bit par résultat, mot de la table 0xfff4 choisi par la difficulté) décident si la
/// transformation s'enchaîne sur une riposte.
fn flee_roll(x: &mut Cx) -> bool {
    let row = (((var::RANK.get(x.b) >> 3) << 1)) as u32;
    let chances = x.rom16(0xfff4 + row);
    let r = rand_and(x, 0xf);
    (chances >> r) & 1 != 0
}

/// 0x9cbe8.
pub(super) fn transform(x: &mut Cx) {
    if second_form(x) {
        return x.set32(f::STATE, 0x0404_0200);
    }
    x.set8(SECOND_FX, 0);
    match x.u8(f::SUB2) {
        0 => {
            if !two(x) {
                x.set8(0, 2);
            }
            x.set8(0x86, 0);
            x.set16(0x84, 0);
            x.set8(0x49, 0);
            if !flee_roll(x) {
                x.set8(ARM_TIMER, 0x46);
                x.set8(0, 1);
                x.set32(f::STATE, 0x0204_1400);
            } else {
                x.set8(ARM_TIMER, 0x50);
                x.set8(0, 1);
                next6(x);
            }
        }
        2 => {
            next6(x);
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
        4 => {
            if x.u8(f::KEY_FRAME) != 0 {
                next6(x);
                x.set8(VULNERABLE, 0);
            }
            step_anim(x);
        }
        6 => leap_transform(x, true),
        8 => leap_transform(x, false),
        10 => {
            x.move_xz();
            if (x.u32(0xc) as i32) > 0 {
                return step_anim(x);
            }
            lib::shake_54(x, 0x1e);
            lib::sound(x, 0xecce6);
            x.set32(0xc, 0);
            dust_line(x);
            next6(x);
            x.set16(0x30, 1);
            scatter(x);
            start(x, Set::A, 6);
        }
        12 => {
            if x.anim_step() {
                x.set32(f::STATE, state::STAND);
                x.set8(VULNERABLE, 1);
                if !two(x) {
                    x.set8(0, 1);
                }
            }
        }
        _ => {}
    }
}

fn leap_transform<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>, up: bool) {
    x.move_xz();
    let vz = x.u16(0x18);
    let reached = if up { Fl::sub16(vz, 0x100).gt() } else { Fl::sub16(vz, 0xff00).gt() };
    if !reached {
        next6(x);
        x.set16(0x30, 1);
    }
    step_anim(x);
}
