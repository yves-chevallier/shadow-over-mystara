//! Boss 9, état actif : entrée en scène (`+5` = 0), départ (2), passages et arrêts (4), attente (6).

use super::*;

// --- groupe 0 : entrée en scène --------------------------------------------------------------------

/// 0x8a96a : l'équipage monte, dialogue, l'engin traverse l'écran puis revient en marche arrière.
pub(super) fn intro(x: &mut Cx) {
    match x.u8(f::SUB2) {
        2 => board(x),
        4 => {
            if x.u8(CREW_LOST) != 0 {
                return;
            }
            x.set16(TIMER, 0xa);
            x.set16(0x14, 0);
            x.set16(0x16, 0);
            x.c.d[0] = 5;
            x.call(call::MESSAGE);
            if carry(x) {
                return;
            }
            x.set16(MESSAGE, x.c.a[1] as u16);
            next6(x);
        }
        6 => {
            if x.u16(MESSAGE) != 0 {
                let m = x.linked(MESSAGE);
                x.c.a[1] = m.0;
                if m.u8(x.b, 0x22) == 0 {
                    return;
                }
            }
            next6(x);
            x.set16(0x14, 0);
            x.set16(0x16, 0x28);
            x.set8(GAIT, 0);
            x.call(0xea91a);
            let n = x.a5_u8(0x4ff).wrapping_add(4);
            x.set_a5_u8(0x4ff, n);
        }
        8 => {
            jolt(x);
            smoke_every(x, 7);
            if x.u16(0x14) as i16 >= 0x300 {
                next6(x);
                x.set16(0x16, 4);
            }
            roll(x);
        }
        10 => {
            jolt(x);
            smoke_every(x, 3);
            let cam = x_cam(x);
            if left_of(x, 0x80, cam) {
                return next6(x);
            }
            roll(x);
        }
        14 => {
            smoke_every(x, 3);
            next6(x);
            x.set16(0x14, 0x800);
            x.set8(GAIT, 1);
            x.call(call::ROLL);
        }
        16 => {
            jolt(x);
            smoke_every(x, 3);
            let edge = x_cam(x).wrapping_add(0x180);
            if !right_of(x, 0x80, edge) {
                return roll(x);
            }
            next6(x);
            x.set16(TIMER, 0x3c);
            x.set8(0x38, 1);
            x.set8(0, 1);
            x.set8(f::FLIP, 0);
            x.set8(GAIT, 2);
        }
        18 => {
            if sub16(x, TIMER, 1).pl() {
                return;
            }
            next6(x);
            x.set16(TIMER, 0x70);
            x.set8(GAIT, 0);
            x.set8(0x2d, 0x18);
            x.set16(0x14, 0);
            x.set16(0x16, (-8i16) as u16);
            x.call(call::BRAKE);
        }
        20 => {
            jolt(x);
            smoke_every(x, 7);
            if sub16(x, TIMER, 1).mi() {
                next6(x);
                x.set8(GAIT, 2);
                x.call(call::STOP);
                x.call(0xd5f30);
            }
            roll(x);
        }
        22 => {
            x.set32(f::IMMUNITY, 0xffff_ffff);
            x.set8(0, 2);
            anim(x);
        }
        _ => {}
    }
}

/// Abscisse de la caméra du plan de l'engin.
fn x_cam(x: &mut Cx) -> u16 {
    x.a5_u16(CAM_X)
}

/// L'engin roule (0x1d78) et ses roues tournent (0x1a56).
pub(super) fn roll(x: &mut Cx) {
    x.call(call::MOVE_X);
    anim(x);
}

/// 0x8a992 : l'équipage monte (quatre servants ENEMY 0x13, variante 2 : `+0xbc` = 5 à 2), s'il y a la place.
fn board(x: &mut Cx) {
    if (x.a5_u16(ENEMY_FREE) as i16) < 4 {
        return;
    }
    x.set8(CREW_LOST, 0);
    for k in 0..4u32 {
        let slot = 5 - k as u8;
        x.c.d[5] = slot as u32;
        x.c.a[2] = x.o.0 + CREW + 2 * k;
        let Some(m) = try_alloc(x, rom::ALLOC_ENEMY) else { return };
        let active = x.u8(0);
        m.set_u8(x.b, 0, active);
        m.set_u8(x.b, 0xbc, slot);
        m.set_u8(x.b, 3, 2);
        m.set_u8(x.b, 2, 0x13);
        m.set_u16(x.b, 0x42, x.o.0 as u16);
        x.set16(CREW + 2 * k, m.0 as u16);
        x.c.a[2] = x.o.0 + CREW + 2 * k + 2;
        if slot == 2 {
            break;
        }
        x.add8(CREW_LOST, 1);
    }
    next6(x);
}

// --- groupe 2 : départ ---------------------------------------------------------------------------

/// 0x8abc2 : panache de fumée (SET 2, variante 0xe), puis départ du premier passage.
pub(super) fn start(x: &mut Cx) {
    if let Some(s) = try_alloc(x, call::ALLOC_SET) {
        s.set_u8(x.b, 0, 1);
        s.set_u8(x.b, 3, 0xe);
        s.set_u8(x.b, 2, 2);
    }
    restart(x)
}

/// 0x8abdc : (re)départ : premier passage (`+5` = 4).
pub(super) fn restart(x: &mut Cx) {
    x.call(rom::IMMUNITIES);
    x.set8(0, 1);
    x.add8(f::SUB, 2);
    steps(x, 0, 2);
    x.set8(WAIT, 1);
    x.set16(0x16, 0);
    reset_lives(x);
    count_crew(x);
    set_speed(x);
    reset_dashes(x);
    draw_passes(x);
}

// --- groupe 4 : passages et arrêts ------------------------------------------------------------

/// 0x8ac14 : fumée de l'avarie (une fois) ; sans servants, l'engin attend leur remplacement ; sinon l'étape `+6`.
pub(super) fn fight(x: &mut Cx) {
    if x.u8(SMOKE_DONE) == 0 && sub16(x, SMOKE_DELAY, 1).mi() {
        x.set8(SMOKE_DONE, 1);
        if let Some(s) = try_alloc(x, call::ALLOC_SET) {
            s.set_u8(x.b, 0, 1);
            s.set_u8(x.b, 3, 0x1c);
            s.set_u8(x.b, 2, 2);
        }
    }
    if x.u8(CREW_LOST) != 0 {
        x.set8(GAIT, 2);
        count_crew(x);
        if x.u8(CREW_COUNT) == 0 {
            refill_crew(x, false);
        }
        return anim(x);
    }
    match x.u8(f::SUB2) {
        0 => pass(x),
        2 => approach(x),
        4 | 16 => fire(x),
        6 => bob(x),
        8 | 20 => bomb(x),
        10 | 22 => bomb_then_fire(x),
        12 | 24 => halt(x, 0x14),
        14 | 26 => halt(x, 0x30),
        _ => volley(x),
    }
    if x.a5_u8(var::PLAYERS_ALIVE) == 0 {
        x.set32(f::STATE, 0x0206_0000);
    }
}

/// 0x8acaa : un passage d'un bord à l'autre (après `+0x6e` images d'attente).
fn pass(x: &mut Cx) {
    if x.u8(0x7) == 0 {
        if sub8(x, WAIT, 1).pl() {
            return;
        }
        next7(x);
        x.call(call::SHOUT);
        x.set8(0x45, 0xa);
        x.set8(0x44, 0x2d);
        count_crew(x);
        if !try_dash(x) {
            set_speed(x);
        }
        retarget(x);
        return pick_lane(x);
    }
    if crew_gone(x) {
        return;
    }
    count_crew(x);
    set_speed(x);
    if x.u8(CREW_COUNT) != 0 {
        let v = x.u16(0x14);
        if v == 0xf800 || v == 0x800 {
            if every(x, 3) {
                smoke_pair(x, 1, 0);
            }
        } else {
            crew_smoke(x);
        }
    }
    if !past_edge(x) {
        return x.call(call::MOVE_X);
    }
    turn_back(x);
    if sub8(x, PASSES, 1).mi() {
        next6(x);
        x.set8(0x7, 0);
        reset_dashes(x);
        draw_passes(x);
        draw_stops(x);
        return x.call(call::MOVE_X);
    }
    x.set8(0x7, 0);
    reset_wait(x)
}

/// Au bord : demi-tour, et l'équipage perdu est remplacé.
fn turn_back(x: &mut Cx) {
    let side = x.u8(f::FLIP) ^ 0x20;
    x.set8(f::FLIP, side);
    refill_crew(x, false);
}

/// 0x8adfc : roule jusqu'à l'abscisse d'arrêt, attend `+0xbe` images, puis s'arrête pour tirer selon la cible.
fn approach(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            next7(x);
            reset_lives(x);
            reset_count(x);
            retarget(x);
            pick_lane(x);
            draw_stop_x(x);
            count_crew(x);
            try_dash(x);
            set_speed(x);
        }
        2 => {
            if past_edge(x) {
                turn_back(x);
                x.set8(0x7, 0);
                return reset_wait(x);
            }
            count_crew(x);
            if x.u8(CREW_COUNT) != 0 {
                crew_smoke(x);
            }
            let d = x.u16(STOP_X).wrapping_sub(x.u16(0x8)).wrapping_add(8);
            set_dw(x, 0, d);
            if d > 0x20 {
                return x.call(call::MOVE_X);
            }
            x.set8(GAIT, 2);
            if sub8(x, COUNT, 1).pl() {
                return;
            }
            x.call(call::STOP);
            reset_lives(x);
            reset_count(x);
            retarget(x);
            choose_halt(x);
        }
        _ => {
            if crew_gone(x) {
                return;
            }
            if !sub8(x, STOPS, 1).ne() {
                return steps(x, 0, 2);
            }
            if sub8(x, COUNT, 1).pl() {
                return;
            }
            reset_count(x);
            retarget(x);
            choose_halt(x);
        }
    }
}

/// 0x8aed8 / 0x8af4c : arrêt de tir selon la portée de la cible ; hors de portée, nouveau passage.
fn choose_halt(x: &mut Cx) {
    match target_range(x) {
        0 => draw_halt(x, 0xe),
        2 => steps(x, 0, 2),
        _ => draw_halt(x, 2),
    }
}

/// 0x8c952 : avarie de tir (bit 0 de `+0xa8`) : l'arrêt est sauté ; vrai.
fn cannon_broken(x: &mut Cx) -> bool {
    if x.bit(DAMAGE, 0) {
        steps(x, 2, 4);
        x.c.d[0] = 1;
        return true;
    }
    x.c.d[0] = 0;
    false
}

/// 0x8c960 : avarie de bombardement (bits 3 ou 4) ; vrai.
fn bombard_broken(x: &mut Cx) -> bool {
    if x.bit(DAMAGE, 3) || x.bit(DAMAGE, 4) {
        steps(x, 2, 4);
        x.c.d[0] = 1;
        return true;
    }
    x.c.d[0] = 0;
    false
}

/// 0x8af74 : arrêt, ordre de tir aux parties (bit 0 de `+0xa9`).
fn fire_order(x: &mut Cx) {
    if cannon_broken(x) {
        return;
    }
    next7(x);
    x.set8(GAIT, 2);
    x.set16(0x14, 0);
    x.set_bit(ORDERS, 0, true);
}

/// 0x8af94 : attend la fin du tir, puis repart.
fn fire_wait(x: &mut Cx) {
    if !x.bit(ORDERS, 0) {
        if crew_gone(x) {
            return;
        }
        steps(x, 2, 4);
    }
    anim(x)
}

/// 0x8af62 / 0x8b3bc : tir.
fn fire(x: &mut Cx) {
    if x.u8(0x7) == 0 {
        fire_order(x)
    } else {
        fire_wait(x)
    }
}

/// Monte et descend en profondeur (`+0x1c`) entre 0x40 et 0x70 pendant que l'équipage tient.
fn sway(x: &mut Cx) {
    count_crew(x);
    if x.u8(CREW_COUNT) != 0 {
        let z = x.u16(0x10) as i16;
        if x.u8(COUNT) != 0 && z > 0x70 {
            negate(x, 0x1c);
            x.set8(COUNT, 0);
        } else if z <= 0x40 {
            negate(x, 0x1c);
            x.set8(COUNT, 1);
        }
        x.set16(0x14, 0);
        x.call(call::MOVE_XY);
    }
    anim(x)
}

/// 0x8afd6 : tir en se déplaçant en profondeur.
fn bob(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            if cannon_broken(x) {
                return;
            }
            next7(x);
            x.set8(GAIT, 2);
            x.set16(0x14, 0);
            x.set16(0x16, 0);
            x.set8(COUNT, 1);
            x.set16(0x1c, 0x120);
            x.set_bit(ORDERS, 0, true);
            x.set8(READY, 1);
        }
        2 => {
            if x.u8(READY) == 0 {
                next7(x);
                x.set8(READY, 1);
            }
            anim(x)
        }
        4 => {
            x.set8(GAIT, 0);
            if x.u8(READY) == 0 {
                return next7(x);
            }
            sway(x)
        }
        _ => {
            x.set8(GAIT, 2);
            if x.bit(ORDERS, 0) {
                return anim(x);
            }
            stop_over(x)
        }
    }
}

/// 0x8b100 : vers la profondeur de la cible (`+0x1c`), et nombre de bombardements.
fn aim_lane(x: &mut Cx) {
    x.set16(0x14, 0);
    x.set16(0x16, 0);
    x.set8(COUNT, 0);
    x.set16(0x1c, (-0x120i16) as u16);
    let t = load_target(x);
    let tz = t.u16(x.b, 0x10);
    set_dw(x, 0, tz);
    let d = x.u16(0x10).wrapping_sub(tz);
    set_dw(x, 1, d);
    if !Fl::sub16(x.u16(0x10), tz).gt() {
        x.set8(COUNT, 1);
        x.set16(0x1c, 0x120);
    }
    next7(x);
    draw_bombs(x)
}

/// 0x8b13e : ordre de bombardement (bits 3 et 4 de `+0xa9`).
fn bomb_order(x: &mut Cx) {
    if bombard_broken(x) {
        return;
    }
    next7(x);
    x.set8(GAIT, 2);
    x.set16(0x14, 0);
    x.set_bit(ORDERS, 3, true);
    x.set_bit(ORDERS, 4, true);
    x.set8(READY, 1);
}

/// 0x8b0ea : bombardements répétés en se déplaçant en profondeur.
fn bomb(x: &mut Cx) {
    match x.u8(0x7) {
        0 => aim_lane(x),
        2 => bomb_order(x),
        4 => {
            if x.u8(READY) == 0 {
                next7(x);
                x.set8(READY, 1);
                x.set8(GAIT, 0);
            }
        }
        _ => {
            if x.bit(ORDERS, 4) {
                return sway(x);
            }
            if sub8(x, BOMBS, 1).pl() {
                return x.add8(0x7, 0xfc);
            }
            stop_over(x)
        }
    }
}

/// 0x8b226 / 0x8b454 : bombardement puis tir.
fn bomb_then_fire(x: &mut Cx) {
    match x.u8(0x7) {
        0 => bomb_order(x),
        2 => {
            if x.bit(ORDERS, 4) {
                return anim(x);
            }
            next7(x);
        }
        4 => fire_order(x),
        _ => fire_wait(x),
    }
}

/// 0x8b328 / 0x8b33a : arrêt de `ticks` images.
fn halt(x: &mut Cx, ticks: u16) {
    if x.u8(0x7) == 0 {
        next7(x);
        x.set16(TIMER, ticks);
        x.set8(GAIT, 2);
        x.set16(0x14, 0);
        x.set16(0x16, 0);
        return;
    }
    // l'original décompte l'octet fort du mot : l'arrêt finit tout de suite
    if sub8(x, TIMER, 1).pl() {
        return;
    }
    stop_over(x)
}

/// 0x8b3ce : tirs répétés (`+0xb9` salves).
fn volley(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            next7(x);
            draw_bombs(x)
        }
        2 => fire_order(x),
        _ => {
            if x.bit(ORDERS, 0) {
                return anim(x);
            }
            if sub8(x, BOMBS, 1).pl() {
                return x.add8(0x7, 0xfe);
            }
            stop_over(x)
        }
    }
}

// --- groupe 6 : attente --------------------------------------------------------------------------

/// 0x8b46a : un joueur revient : départ.
pub(super) fn wait_for_players(x: &mut Cx) {
    x.set8(GAIT, 2);
    if x.a5_u8(var::PLAYERS_ALIVE) == 0 {
        return anim(x);
    }
    x.set32(f::STATE, 0x0202_0000);
    restart(x)
}
