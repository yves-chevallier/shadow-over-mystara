//! Boss 16, états actifs de déplacement : réveil (`+5` = 0), combat (2), bond vers un point (6), attente avant
//! une attaque (8), pause (10), joueur saisi (12).

use super::*;
use crate::objects::Courant;

// --- groupe 0 : réveil --------------------------------------------------------------------------

/// 0x93454 : les variantes 0 et 1 attendent qu'un joueur s'approche ; les autres vont droit au combat.
pub(super) fn wake(x: &mut Cx) {
    match x.u8(3) {
        0 => wake_low(x),
        1 => wake_high(x),
        _ => fight_from_wake(x),
    }
}

/// 0x93606 : combat d'emblée (le groupe 2 tourne dans la même image).
fn fight_from_wake(x: &mut Cx) {
    x.set8(0, 1);
    lib::set_immunities(x);
    x.set32(f::STATE, state::WALK);
    fight(x);
}

/// 0x9351a : étage de proximité (0 à 7) du joueur le plus proche, d'après la distance horizontale (le test vertical de
/// l'original compare la position du joueur avec elle-même : il est toujours nul).
fn proximity(x: &mut Cx) -> u8 {
    let mut best: u16 = 0x7fff;
    for k in 0..4u32 {
        let p = crate::objects::Obj(0xff85de + 0x200 * k);
        if p.u8(x.b, 0) == 0 {
            continue;
        }
        let mut dx = p.u16(x.b, 0x8).wrapping_sub(x.u16(0x8));
        if (dx as i16) < 0 {
            dx = dx.wrapping_neg();
        }
        if Fl::sub16(dx, best).le() {
            best = dx;
        }
    }
    x.c.d[0] = x.c.d[0] & !0xffff | best as u32;
    if (best as i16) >= 0x1ff {
        x.c.d[0] = 0;
        return 0;
    }
    let level = x.rom8(0x93562 + (best >> 4) as u32);
    level
}

/// 0x93470 : variante 0, accroupie : se retourne quand un joueur approche, et attaque quand l'animation de
/// réveil est terminée.
fn wake_low(x: &mut Cx) {
    x.set8(FALLS, 1);
    match x.u8(f::SUB2) {
        0 => {
            next6(x);
            x.set8(0x7, 0);
            x.set8(NEAR, 0);
            x.set8(f::FLIP, 0x20);
            x.set8(TIMER, 0x7f);
            anim_a(x, 0);
        }
        2 => {
            let level = proximity(x);
            let mut near = x.u8(NEAR);
            let mut changed = false;
            if near == 0 {
                if sub8(x, TIMER, 1).gt() {
                    // pas encore
                } else {
                    near = 2;
                    x.set8(NEAR, near);
                    changed = true;
                }
            }
            if !changed && (level as i8) > near as i8 {
                x.set8(NEAR, level);
                changed = true;
            }
            if changed {
                if x.u16(ANIM_SET) != 7 {
                    let side = x.u8(f::FLIP) ^ 0x20;
                    x.set8(f::FLIP, side);
                    return anim_a(x, 7);
                }
            }
            if lib::step(x) {
                x.set8(0, 1);
                lib::set_immunities(x);
                return x.set32(f::STATE, 0x0204_0800);
            }
            let i = x.u8(NEAR) as u32;
            let v = x.rom8(0x93512 + i);
            if Fl::sub8(v, x.u8(NEAR_MAX)).ge() {
                return;
            }
            x.set8(NEAR_MAX, v);
        }
        _ => {}
    }
}

/// 0x93582 : variante 1, en hauteur : lance le script de palette 0x10298 au rythme de l'animation, puis combat.
fn wake_high(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            next6(x);
            x.set8(0x7, 0);
            x.set8(TIMER, 0x7f);
            anim_a(x, 8);
            lib::palette_script(x, 0);
            return wake_high_wait(x);
        }
        2 => wake_high_wait(x),
        _ => {}
    }
}

/// 0x935b0.
fn wake_high_wait(x: &mut Cx) {
    if x.u8(0x7) == 0 {
        if proximity(x) == 0 {
            return;
        }
        next7(x);
    }
    x.set8(FALLS, 1);
    if x.anim_step() {
        lib::palette_script(x, 7);
        return fight_from_wake(x);
    }
    let event = x.u8(0x3e);
    x.c.d[0] = event as u32;
    if event == 8 {
        x.set_a5_u16(0x4be, 2);
        x.call(0x1ac68);
    }
    let event = x.u8(0x3e);
    if event == 0 {
        return;
    }
    x.set8(0x3e, 0);
    lib::palette_script(x, (event - 1) as u16);
}

// --- groupe 2 : combat -------------------------------------------------------------------------

/// 0x93618 : combat ; sans joueur en vie, le boss attend.
pub(super) fn fight(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => stand(x),
        2 => turn(x),
        4 => walk(x, 0x2f),
        6 => leap(x),
        8 => walk(x, 0x30),
        _ => {}
    }
    if var::PLAYERS_ALIVE.get(x.b) == 0 {
        x.set32(f::STATE, state::IDLE);
    }
}

/// 0x93642 : debout, attend `+0x6e` images, se retourne si la cible est derrière.
fn stand(x: &mut Cx) {
    x.set8(FALLS, 1);
    if x.u8(0x7) == 0 {
        next7(x);
        x.set8(VULNERABLE, 1);
        return anim_a(x, 0);
    }
    if sub8(x, IDLE_COUNT, 1).le() {
        return x.set32(f::STATE, state::WALK);
    }
    if target_behind(x) {
        x.set32(SAVED_STATE, state::WALK);
        x.set32(f::STATE, state::TURN);
    }
    lib::step(x);
}

/// 0x9368e : demi-tour.
fn turn(x: &mut Cx) {
    x.set8(FALLS, 1);
    match x.u8(0x7) {
        0 => {
            next7(x);
            let side = x.u8(f::FLIP) ^ 0x20;
            x.set8(f::FLIP, side);
            x.set8(VULNERABLE, 1);
            x.set8(0x2d, if side != 0 { 8 } else { 0x18 });
            anim_a(x, 1);
        }
        2 => {
            if lib::step(x) {
                let s = x.u32(SAVED_STATE);
                x.set32(f::STATE, s);
            }
        }
        _ => {}
    }
}

/// 0x936ee (kind 0x2f) et 0x936e6 (kind 0x30) : marche vers la cible ; à portée, elle devient une attaque.
fn walk(x: &mut Cx, kind: u8) {
    x.set8(0x79, kind);
    x.set8(FALLS, 1);
    if x.u8(0x7) == 0 {
        next7(x);
        x.set8(VULNERABLE, 1);
        anim_a(x, 0);
        speed(x, 3);
        if !pick_target(x) {
            return idle(x);
        }
        lib::draw_stance(x);
        return lib::draw_6b(x);
    }
    if (x.u16(0xc) as i16) <= 0 {
        lib::approach_target(x);
    }
    if sub8(x, WALK_DELAY, 1).mi() {
        lib::draw_stance(x);
        let r = rand_and(x, 0xf);
        if 0xffc0u32 >> r & 1 != 0 {
            if !pick_target(x) {
                return idle(x);
            }
        } else {
            if !lib::random_player(x) {
                return idle(x);
            }
        }
        if sub8(x, WALK_STEPS, 1).ne() {
            // la marche continue
        } else {
            lib::draw_6b(x);
            if lib::maybe_draw_6e(x) {
                x.set16(f::SUB2, 0);
                return;
            }
        }
    }
    if close_target(x) || edge_target(x, -0x50, 0x20, 0x10) {
        return attack_choice(x);
    }
    if edge_target(x, -0xc0, 0x30, 0x10) {
        return attack_choice(x);
    }
    if target_far(x) {
        return x.set32(f::STATE, state::LEAP);
    }
    lib::step(x);
    x.move_by_table_xy();
    if target_behind(x) {
        x.set32(SAVED_STATE, state::STAND);
        x.set32(f::STATE, state::TURN);
    }
}

/// 0x949b2 et 0x949d4 : une cible à portée ; au dé près le boss fonce (0x0202_0800), sinon il choisit une attaque dans la
/// table 0x94a24 (la seconde table de l'original, 0x94c24, est écrasée avant d'être lue).
fn attack_choice(x: &mut Cx) {
    let r = rand_and(x, 0xf);
    if 0xffc0u32 >> r & 1 != 0 {
        return x.set32(f::STATE, state::WALK_B);
    }
    let r = rand_and(x, 0x1f);
    let step = x.data8(0x94a24 + r);
    x.set8(f::SUB2, step);
    x.set16(f::STATE, 0x0204);
    x.set8(0x7, 0);
    lib::pick_byte(x);
}

/// 0x937e2 : bond vers la cible (le boss est loin d'elle).
fn leap(x: &mut Cx) {
    x.set8(FALLS, 1);
    match x.u8(0x7) {
        0 => {
            next7(x);
            x.set16(TIMER, 0x1e);
            x.set8(VULNERABLE, 1);
            speed(x, 8);
            anim_a(x, 2);
        }
        2 => {
            if sub16(x, TIMER, 1).mi() {
                return x.set32(f::STATE, state::WALK);
            }
            x.set8(0x79, 0x2f);
            lib::approach_target(x);
            if target_behind(x) {
                x.set32(SAVED_STATE, state::STAND);
                x.set32(f::STATE, state::TURN);
            }
            x.move_by_table_xy();
            lib::step(x);
        }
        _ => {}
    }
}

// --- groupe 6 : course vers un point de l'écran ---------------------------------------------------

/// 0x93cfa : course (`+6` = 0, 2) puis saut (4) vers un point près de la caméra ; sans joueur, attente.
pub(super) fn jump_to_spot(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 | 2 => run_to_spot(x),
        4 => spot_jump(x),
        _ => {}
    }
    if var::PLAYERS_ALIVE.get(x.b) == 0 {
        x.set32(f::STATE, state::IDLE);
    }
}

/// 0x9514a : choisit le point d'arrivée (`+0xae`, `+0xb0`), le sens de rotation (`+0xb3`) et l'angle (`+0x2d`) d'après la
/// position du boss par rapport à la caméra (quatre coins, table 0x951b0 : X, Z, angle, sens).
fn choose_spot(x: &mut Cx) {
    let cam = camera_record(x);
    let dx = x.u16(0x8).wrapping_sub(cam.u16(x.b, 0x8));
    let dz = x.u16(0x10).wrapping_sub(cam.u16(x.b, 0xc));
    let row = if Fl::sub16(dx, 0xc0).gt() {
        if Fl::sub16(dz, 0x80).gt() { 0x12 } else { 0xc }
    } else if Fl::sub16(dz, 0x80).gt() {
        6
    } else {
        0
    };
    let sx = x.rom16(0x951b0 + row).wrapping_add(cam.u16(x.b, 0x8));
    x.set16(SPOT_X, sx);
    let sz = x.rom16(0x951b2 + row).wrapping_add(cam.u16(x.b, 0xc));
    x.set16(SPOT_Z, sz);
    let angle = x.rom8(0x951b4 + row);
    x.set8(0x2d, angle);
    let spin = x.rom8(0x951b5 + row);
    x.set8(SPIN, spin);
}

/// 0x93d20.
fn run_to_spot(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            next7(x);
            x.set16(TIMER, 6);
            choose_spot(x);
            anim_a(x, 2);
            speed(x, 8);
            x.move_by_table_xy();
            lib::step(x);
        }
        2 => {
            let on_target = lib::target_in_box(x, 0x80, 0x100, 0x10);
            let mut arrived = on_target;
            if !arrived {
                let dx = (x.u16(SPOT_X).wrapping_sub(x.u16(0x8)).wrapping_add(8)) as i16;
                let dz = (x.u16(SPOT_Z).wrapping_sub(x.u16(0x10)).wrapping_add(8)) as i16;
                arrived = dx <= 0x10 && dz <= 0x10;
            }
            if arrived {
                return x.set32(f::STATE, state::WALK);
            }
            if sub16(x, TIMER, 1).mi() {
                let a = x.u8(0x2d);
                let a = if x.u8(SPIN) != 0 { a.wrapping_sub(1) } else { a.wrapping_add(1) };
                x.set8(0x2d, a & 0x1f);
                x.set16(TIMER, 6);
            }
            x.move_by_table_xy();
            lib::step(x);
        }
        _ => {}
    }
}

/// 0x93dd4 : saut sur place.
fn spot_jump(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            next7(x);
            let angle = if x.u8(f::FLIP) != 0 { 0x18 } else { 8 };
            x.set8(0x2d, angle);
            stop(x);
            x.set16(0x18, 0x600);
            x.set16(0x1a, (-0x80i16) as u16);
            speed(x, 8);
            anim_a(x, 0);
        }
        2 => {
            lib::step(x);
            x.move_by_table_xy();
            x.move_z();
            if (x.u16(0xc) as i16) >= 0 {
                return;
            }
            x.set16(0xc, 0);
            x.set32(0x18, 0);
            next7(x);
            anim_a(x, 0);
        }
        4 => x.set32(f::STATE, state::WALK),
        _ => {}
    }
}

// --- groupe 8 : attente avant une attaque ---------------------------------------------------------

/// 0x93e52.
pub(super) fn before_attack(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => wait_then_turn(x),
        2 => spot_jump(x),
        4 => run_to_spot(x),
        _ => {}
    }
}

/// 0x93e64.
fn wait_then_turn(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            next7(x);
            x.set16(TIMER, 0x14);
            x.set8(VULNERABLE, 1);
            anim_a(x, 0);
        }
        4 => {
            x.add8(0x7, 0xfe);
            x.set8(VULNERABLE, 1);
            anim_a(x, 0);
        }
        2 => {
            if sub16(x, TIMER, 1).le() {
                return x.set32(f::STATE, state::WALK);
            }
            if target_behind(x) {
                x.set32(SAVED_STATE, 0x0208_0004);
                x.set32(f::STATE, state::TURN);
            } else {
                lib::step(x);
            }
        }
        _ => {}
    }
}

// --- groupe 10 : pause ------------------------------------------------------------------------------

/// 0x93ece.
pub(super) fn paused(x: &mut Cx) {
    if x.u8(f::SUB2) == 0 {
        if var::PLAYERS_ALIVE.get(x.b) != 0 {
            return x.set32(f::STATE, state::WALK);
        }
        x.set8(FALLS, 1);
        if x.u8(0x7) == 0 {
            next7(x);
            return anim_a(x, 0);
        }
        lib::step(x);
        return;
    }
    match x.u8(0x7) {
        0 => {
            next7(x);
            stop(x);
            anim_a(x, 6);
        }
        2 => {
            if var::PAUSE.get(x.b) == 0 {
                return x.set32(f::STATE, state::STAND);
            }
            lib::step(x);
        }
        _ => {}
    }
}

// --- groupe 12 : joueur saisi ---------------------------------------------------------------------------

/// 0x9402a : plus de proie, le boss s'éloigne d'un bond.
fn drop_prey(x: &mut Cx) {
    x.set32(f::STATE, state::WALK);
    x.set16(0x18, 0x400);
    x.set16(0x1a, (-0x40i16) as u16);
    let v = if x.u8(f::FLIP) == 0 { 0x300u16 } else { (-0x300i16) as u16 };
    x.set16(0x14, v);
}

/// 0x93f30 : le boss tient un joueur (`+0x50`) : morsures qui lui retirent 0x10 PV puis 1, relâché à 1 PV.
pub(super) fn holding(x: &mut Cx) {
    if x.u16(f::TARGET) as i16 >= 0 {
        return drop_prey(x);
    }
    let prey = x.linked(f::TARGET);
    if prey.u8(x.b, 0) == 0 || prey.u8(x.b, 0x188) == 0 {
        return drop_prey(x);
    }
    match x.u8(0x7) {
        0 => {
            next7(x);
            let v = (x.u16(0x14) as i16 >> 2) as u16;
            x.set16(0x14, v);
            x.set16(0x16, 0);
            x.set8(TIMER, 0x20);
            let hp = prey.u16(x.b, f::HP).wrapping_sub(0x10);
            prey.set_u16(x.b, f::HP, if (hp as i16) > 0 { hp } else { 1 });
            anim_b(x, 2);
            lib::hold_seized(x);
        }
        2 => {
            if tst8(x, TIMER).gt() {
                if sub8(x, TIMER, 1).gt() {
                    // la chute continue
                } else {
                    next7(x);
                    x.set8(TIMER, 0x10);
                    x.set32(0x14, 0);
                    x.set32(0x1c, 0);
                }
                x.move_xz();
                if (x.u16(0xc) as i16) > 0 {
                    x.set16(0xc, 0);
                }
            }
            hold_bite(x, prey);
        }
        4 => hold_bite(x, prey),
        _ => {}
    }
}

/// 0x93fbc : animation de la morsure ; à sa fin, le joueur perd 1 PV (jamais le dernier) puis le boss le relâche
/// quand le décompte est écoulé.
fn hold_bite(x: &mut Cx, prey: crate::objects::Obj) {
    if !lib::step(x) {
        return lib::hold_seized(x);
    }
    x.set16(ANIM_SET, 0xffff);
    anim_b(x, 3);
    if sub8(x, TIMER, 1).le() || prey.u16(x.b, f::HP) as i16 <= 1 {
        prey.set_u8(x.b, 0x6f, 1);
        return lib::release_player(x, prey);
    }
    add16(x, prey, f::HP, 0xffff);
    x.c.a[1] = prey.0;
    lib::sound(x, 0xedd96);
    x.c.d[1] = 2;
    let mut dx = 0x18u16;
    if x.u8(f::FLIP) == 0 {
        dx = dx.wrapping_neg();
    }
    x.c.d[2] = dx.wrapping_add(x.u16(0x8)) as u32;
    x.c.d[3] = x.u16(0xc).wrapping_add(8) as u32;
    x.c.d[4] = x.u16(0x10).wrapping_sub(2) as u32;
    x.call(call::SPLASH);
    lib::hold_seized(x);
}
