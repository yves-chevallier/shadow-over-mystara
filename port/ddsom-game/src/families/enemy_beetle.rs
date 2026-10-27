//! Ennemi « GiantBeetle » (ENEMY type 9, 0x65ae8). Voir `docs/familles/enemy.md`.
//!
//! Insecte qui défile sur le côté de l'écran (variantes `+3` = 0 à 4). La variante 4 est le nid : elle
//! engendre des pièces (ESHL 0x38) d'après une liste de la ROM puis attend d'être visible. Le groupe de
//! trajectoire `+0x2f` (0 à 7) choisit le comportement de l'état actif, sous-état 4 ; `+7` en est l'étape.
//!
//! | `+5` | Rôle (h) |
//! |---|---|
//! | 0, 2 | marche avec demi-tours (`steer`), surveille un joueur à mordre (`bite_check`), sort de l'écran |
//! | 4 | trajectoires par groupe `+0x2f` : 0-1 traversée, 2-3 dérive, 4-5 et 6-7 vol arrêté devant un joueur |
//! | 6 | bond : élan, vol, retombée |
//! | 8 | dormant tant que l'insecte est hors de l'écran |
//!
//! État 4 (touché) : recul, chute puis mort clignotante.
//! Animations 0x163a86 (mouvements) et 0x163a9e (touché).

use super::cx::{handlers, Cx};
use super::enemy_common::*;
use crate::objects::Obj;

const ANIMS: u32 = 0x163a86;
const ANIMS_HIT: u32 = 0x163a9e;

/// Champs propres à ce type.
mod g {
    pub const GROUP: u32 = 0x2f;
    /// Table de vitesses courante (long).
    pub const SPEEDS: u32 = 0x4c;
    pub const HIT_CODE: u32 = 0x49;
    pub const FREEZE_AT_HIT: u32 = 0x56;
    /// Position mémorisée (X et profondeur) et cadence de virage.
    pub const SAVED_X: u32 = 0xa8;
    pub const SAVED_DEPTH: u32 = 0xac;
    pub const TURN_PERIOD: u32 = 0xa2;
    pub const TURN_TIMER: u32 = 0xa3;
    pub const SIDE: u32 = 0xa4;
    pub const TURN_STEP: u32 = 0xa6;
    pub const STOP_STEP: u32 = 0xa7;
    pub const BITING: u32 = 0xb8;
    pub const DRIFT: u32 = 0xb4;
    pub const STOP_X: u32 = 0xb6;
    pub const SAVED_STATE: u32 = 0x8a;
}
use g::*;
use crate::objects::pools;
use crate::objects::Courant;

/// Tables de la ROM : vitesses par difficulté (pointeurs), temporisations, sens de virage.
const SPEEDS_WALK: u32 = 0x66700;
const SPEEDS_FLY: u32 = 0x66720;
const SPEEDS_DRIFT: u32 = 0x1c9036;
const TIMER_A: u32 = 0x666b4;
const TIMER_B: u32 = 0x666b8;
const TURN_DELTA: u32 = 0x66750;
const TURN_TARGETS: u32 = 0x667d4;
const STOP_XS: u32 = 0x660d6;
const SPEED_Y_ROW: u32 = 0x66666;
const BITE_ODDS: u32 = 0x6623a;
const SHAKE_TIMES: u32 = 0x664c6;
/// Listes de pièces du nid (pointeurs par `+0x42`) et table des routines d'allocation.
const NEST_LISTS: u32 = 0x65c78;
const NEST_ALLOCS: u32 = 0x65c88;

const STANCE: u32 = 0x0200_0000;
const HELD: u32 = 0x0404_0000;
const POUNCE: u32 = 0x0206_0000;
const AFTER_POUNCE: u32 = 0x0204_0200;
const DEAD: u32 = 0x0600_0000;

pub fn run(x: &mut Cx) {
    if suspended(x) {
        return;
    }
    x.c.d[0] = 0;
    match x.u8(f::STATE) {
        st::INIT => init(x),
        st::ACTIVE => active(x),
        st::HIT => hit(x),
        st::DEAD => free(x),
        _ => {}
    }
}

// --- aides --------------------------------------------------------------------------------------

fn anim_play(x: &mut Cx, n: u32) {
    x.anim_start(ANIMS, n as u16);
}

/// 0x6662e : position de la caméra, X dans le mot bas de D6, Y dans le mot haut.
fn camera(x: &mut Cx) -> (u16, u16) {
    let (cx, cy) = if var::CAMERA_SELECT.get(x.b) != 0 { (var::CAMERA_ALT_X.get(x.b), var::CAMERA_ALT_Y.get(x.b)) } else { (var::SCROLL_X.get(x.b), var::SCROLL_Y.get(x.b)) };
    (cx, cy)
}

/// 0x666de : adopte une table de vitesses (paires de mots par angle `+0x2d`).
fn speeds(x: &mut Cx, table: u32) {
    x.set32(SPEEDS, table);
    let off = (x.u8(Obj::SPEED_INDEX) as u32) << 2;
    let (vx, vy) = (x.data16(table + off), x.data16(table + off + 2));
    x.set16(Obj::VX, vx);
    x.set16(Obj::VY, vy);
    x.set16(Obj::AX, 0);
    x.set16(Obj::AY, 0);
}

/// 0x666d2 : table de vitesses choisie par la difficulté (`A5+0x7f92` & 0x1c).
fn speeds_by_level(x: &mut Cx, list: u32) {
    let level = (var::RANK.get(x.b) & 0x1c) as u32;
    let table = x.data32(list + level);
    speeds(x, table);
}

/// 0x666a2 : cadence de virage (octets `+0xa2/+0xa3`) selon la difficulté.
fn turn_timer(x: &mut Cx, table: u32) {
    let off = ((var::RANK.get(x.b) & 0x10) >> 3) as u32;
    let v = x.data16(table + off);
    x.set16(TURN_PERIOD, v);
}

/// 0x6664c / 0x6664e : vitesse `+0x1e` lue dans la table selon la difficulté, `base` = décalage de ligne.
fn set_speed_y(x: &mut Cx, base: u16) {
    let off = ((var::RANK.get(x.b) & 0x18) >> 2).wrapping_add(base);
    let v = x.data16(SPEED_Y_ROW + off as u32);
    x.set16(Obj::AY, v);
}

// --- état 0 : création --------------------------------------------------------------------------

fn init(x: &mut Cx) {
    x.add8(f::STATE, 2);
    x.set16(f::SUB2, 0);
    x.set8(f::BOX_SET, 0x15);
    lib::initial_hp(x);
    lib::set_immunities(x);
    for (off, v) in [(0x7e, 4), (0x7f, 0), (0x6c, 3), (0x74, 3), (0x76, 3)] {
        x.set8(off, v);
    }
    x.set16(Obj::Z, 0x36);
    x.set8(0xa5, 0);
    x.set8(BITING, 0);
    match x.u8(Obj::VARIANT) {
        0 | 1 | 3 => start_walk(x),
        2 => enter_from_edge(x),
        4 => nest(x),
        _ => {}
    }
}

/// 0x65b6c : l'insecte part dans le sens que dicte la parité de `+0x2f`.
fn start_walk(x: &mut Cx) {
    let (px, pz) = (x.u16(Obj::X), x.u16(Obj::Y));
    x.set16(SAVED_X, px);
    x.set16(SAVED_DEPTH, pz);
    x.set8(SIDE, 0);
    x.set8(Obj::SPEED_INDEX, 8);
    x.set8(f::FLIP, 0x20);
    x.set8(TURN_STEP, 8);
    if x.u8(GROUP) & 1 == 0 {
        x.set8(Obj::SPEED_INDEX, 0x18);
        x.set8(f::FLIP, 0);
        x.set8(TURN_STEP, 0);
    }
    anim_play(x, 0);
}

/// 0x65bb4 : reprise de la marche après un bond, selon l'orientation et la parité de `+0x2f`.
fn resume_walk(x: &mut Cx) {
    let (px, pz) = (x.u16(Obj::X), x.u16(Obj::Y));
    x.set16(SAVED_X, px);
    x.set16(SAVED_DEPTH, pz);
    let flipped = x.u8(f::FLIP) != 0;
    x.set8(Obj::SPEED_INDEX, if flipped { 8 } else { 0x18 });
    x.set8(TURN_STEP, if flipped { 8 } else { 0 });
    let odd_group = x.u8(GROUP) != 0;
    x.set8(SIDE, if flipped != odd_group { 0x10 } else { 0 });
    anim_play(x, 0);
}

/// Variante 2 : entre par le bord droit (ou gauche pour les groupes impairs) de l'écran.
fn enter_from_edge(x: &mut Cx) {
    x.set8(f::SUB, 4);
    let (cx, _) = camera(x);
    let mut offset = 0x190u16;
    x.set8(SIDE, 0);
    x.set8(f::FLIP, 0);
    x.set8(TURN_STEP, 0);
    x.set8(Obj::SPEED_INDEX, 0x18);
    if x.u8(GROUP) & 1 != 0 {
        offset = -0x10i16 as u16;
        x.set8(Obj::SPEED_INDEX, 8);
        x.set8(f::FLIP, 0x20);
        x.set8(TURN_STEP, 8);
    }
    let px = cx.wrapping_add(offset);
    x.set16(Obj::X, px);
    anim_play(x, 0);
}

/// Variante 4 : le nid engendre ses pièces (liste `+0x42`) puis dort jusqu'à être visible.
fn nest(x: &mut Cx) {
    start_walk(x);
    x.set8(f::SUB, 8);
    let list = x.u8(0x42) as u32;
    if list == 3 {
        x.set16(Obj::Z, 0x60);
    }
    spawn_list(x, NEST_LISTS, NEST_ALLOCS, list);
}

// --- état 2 : actif -----------------------------------------------------------------------------

fn active(x: &mut Cx) {
    if !paused(x) {
        let freeze = x.u8(f::FREEZE) as i8;
        if freeze > 0 {
            x.set8(f::FREEZE, (freeze - 1) as u8);
        } else {
            match x.u8(f::SUB) {
                0 | 2 => march(x),
                4 => routes(x),
                6 => pounce(x),
                8 => dormant(x),
                _ => {}
            }
        }
    }
    x.reinsert();
}

/// Sous-états 0 et 2 : marche avec demi-tours ; sort de l'écran quand il en est trop loin.
fn march(x: &mut Cx) {
    if grabbed(x) {
        return set_state_long(x, HELD);
    }
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        speeds_by_level(x, SPEEDS_WALK);
        turn_timer(x, TIMER_B);
    }
    steer(x);
    bite_check(x);
    let (cx, cy) = camera(x);
    let d5 = x.u16(SAVED_X).wrapping_sub(cx).wrapping_add(0x80);
    let inside = d5 < 0x280 && x.u16(SAVED_DEPTH).wrapping_add(x.u16(Obj::Z)).wrapping_sub(cy).wrapping_add(0x10) <= 0x120;
    if !inside {
        set_state_long(x, DEAD);
        detach_from_host(x);
    }
    x.anim_step();
}

/// 0x66740 : avance dans la table de vitesses ; toutes les `+0xa2` images tourne d'un cran vers l'angle voulu
/// et enchaîne l'animation de virage.
fn steer(x: &mut Cx) {
    x.move_by_table_xy();
    let n = x.u8(TURN_TIMER).wrapping_sub(1);
    x.set8(TURN_TIMER, n);
    if n != 0 {
        return;
    }
    let period = x.u8(TURN_PERIOD);
    x.set8(TURN_TIMER, period);
    let idx = ((x.u8(TURN_STEP) & 8) >> 3).wrapping_add(x.u8(SIDE) >> 3) as u32;
    let delta = x.rom8(TURN_DELTA + idx);
    let angle = x.u8(Obj::SPEED_INDEX).wrapping_add(delta) & 0x1f;
    x.set8(Obj::SPEED_INDEX, angle);
    let key = x.u8(TURN_STEP).wrapping_add(x.u8(SIDE)) as u32;
    let entry = TURN_TARGETS + key;
    if angle & 0xf != x.data8(entry) {
        return;
    }
    if x.u8(TURN_STEP) & 6 == 6 {
        let flip = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
        let side = x.u8(SIDE) ^ 0x10;
        x.set8(SIDE, side);
    }
    let step = x.u8(TURN_STEP).wrapping_add(2) & 0xf;
    x.set8(TURN_STEP, step);
    let n = x.data8(entry + 1);
    anim_play(x, n as u32);
}

/// 0x661ce : un joueur devant l'insecte, dans le sol 0xc de l'état 2 : tirage qui le déclenche en bond.
fn bite_check(x: &mut Cx) {
    if !find_player(x, -0x20, 0x20, 8) {
        return x.set8(BITING, 0);
    }
    if x.u8(BITING) == 0 {
        let player = x.linked(f::TARGET);
        if player.u16(x.b, 4) == 0x200 && player.u8(x.b, 6) == 0xc {
            let roll = random(x) & 0x1f;
            let row = ((var::RANK.get(x.b) >> 1) as u32) << 2;
            let odds = x.rom32(BITE_ODDS + row);
            if odds >> roll & 1 != 0 {
                let state = x.u32(f::STATE);
                x.set32(SAVED_STATE, state);
                set_state_long(x, POUNCE);
            }
        }
    }
    x.set8(BITING, 1);
}

// --- sous-état 4 : trajectoires -----------------------------------------------------------------

fn routes(x: &mut Cx) {
    if x.u8(f::SUB2) != 0 {
        return turn_back(x);
    }
    match x.u8(GROUP) {
        0 | 1 => cross(x),
        2 | 3 => drift(x),
        4 | 5 => halt_at(x),
        6 | 7 => halt_and_bite(x),
        _ => {}
    }
}

/// 0x65ea8 : début de traversée.
fn cross_start(x: &mut Cx) {
    x.add8(f::SUB3, 2);
    speeds_by_level(x, SPEEDS_FLY);
    turn_timer(x, TIMER_A);
    anim_play(x, 0);
}

/// Groupes 0 et 1 : traversée, arrêt de la marche puis virage, sortie.
fn cross(x: &mut Cx) {
    match x.u8(f::SUB3) {
        0 => cross_start(x),
        2 => {
            x.move_xy();
            let (cx, _) = camera(x);
            let d5 = x.u16(Obj::X).wrapping_sub(cx);
            let arrived = if x.u8(GROUP) & 1 != 0 { (d5 as i16) >= 0x140 } else { (d5 as i16) <= 0x40 };
            if arrived {
                x.add8(f::SUB3, 2);
                set_speed_y(x, 0x28);
            }
            x.anim_step();
        }
        4 => {
            let group = x.u8(GROUP);
            cross_turn(x, group)
        }
        6 => leave(x),
        _ => {}
    }
}

/// 0x65f18 : virage jusqu'à l'angle de sortie, `side` = la parité qui oriente le virage.
fn cross_turn(x: &mut Cx, side: u8) {
    x.c.d[0] = x.c.d[0] & !0xff | side as u32;
    if side & 1 != 0 {
    }
    steer(x);
    let wanted = if x.u8(GROUP) & 1 != 0 { 0x18 } else { 8 };
    if x.u8(Obj::SPEED_INDEX) == wanted {
        x.add8(f::SUB3, 2);
        x.set8(0, 1);
        speeds_by_level(x, SPEEDS_WALK);
    }
}

/// 0x65f5c : sortie par le bord ; l'insecte est retiré quand il l'a passé.
fn leave(x: &mut Cx) {
    x.move_xy();
    let (cx, _) = camera(x);
    let d5 = x.u16(Obj::X).wrapping_sub(cx);
    let out = if x.u8(GROUP) & 1 != 0 { (d5 as i16) < -0x10 } else { (d5 as i16) >= 0x190 };
    if out {
        x.set8(f::STATE, 6);
        detach_from_host(x);
    }
    x.anim_step();
}

/// Groupes 2 et 3 : dérive latérale constante (`+0xb4`) jusqu'à la sortie de l'écran.
fn drift(x: &mut Cx) {
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            x.set8(Obj::SPEED_INDEX, 0x18);
            let mut d6 = 0x11;
            if x.u8(GROUP) & 1 != 0 {
                x.set8(Obj::SPEED_INDEX, 8);
                d6 = 1;
            }
            speeds(x, SPEEDS_DRIFT);
            x.set16(TURN_PERIOD, 0x101);
            x.set16(Obj::VX, 0x300);
            if x.u8(Obj::SPEED_INDEX) != 8 {
                negate(x, Obj::VX);
            }
            let vx = x.u16(Obj::VX);
            x.set16(DRIFT, vx);
            x.set8(Obj::SPEED_INDEX, d6 as u8);
            anim_play(x, 0);
        }
        2 => {
            let pos = x.u32(Obj::X);
            x.set32(SAVED_X, pos);
            x.move_by_table_xz();
            let pos = x.u32(SAVED_X);
            x.set32(Obj::X, pos);
            let shift = ((x.u16(DRIFT) as i16 as i32) << 8) as u32;
            x.add32(Obj::X, shift);
            let n = x.u8(TURN_TIMER).wrapping_sub(1);
            x.set8(TURN_TIMER, n);
            if n == 0 {
                let period = x.u8(TURN_PERIOD);
                x.set8(TURN_TIMER, period);
                x.add8(Obj::SPEED_INDEX, 1);
                let a = x.u8(Obj::SPEED_INDEX) & 0x1f;
                x.set8(Obj::SPEED_INDEX, a);
                let (cx, _) = camera(x);
                let d5 = x.u16(Obj::X).wrapping_sub(cx);
                let out = if x.u8(GROUP) & 1 != 0 { (d5 as i16) >= 0x190 } else { (d5 as i16) <= -0x10 };
                if out {
                    set_state_long(x, DEAD);
                    detach_from_host(x);
                }
            }
            x.anim_step();
        }
        _ => {}
    }
}

/// 0x660bc : abscisse d'arrêt (`+0xb6`) lue dans la table selon le rang d'arrêt.
fn set_stop(x: &mut Cx, rank: u8) {
    let base = STOP_XS + if x.u8(GROUP) & 1 != 0 { 6 } else { 0 };
    let v = x.data16(base + rank as u32);
    x.set16(STOP_X, v);
}

/// 0x660aa : début d'un vol arrêté.
fn halt_start(x: &mut Cx) {
    cross_start(x);
    x.set8(STOP_STEP, 0);
    speeds_by_level(x, SPEEDS_FLY);
    turn_timer(x, TIMER_A);
    set_stop(x, 0);
}

/// 0x660e2 : vol jusqu'à l'abscisse d'arrêt ; devant un joueur, s'arrête de rang en rang puis charge.
fn halt_run(x: &mut Cx) {
    x.move_xy();
    let (cx, _) = camera(x);
    let d5 = x.u16(Obj::X).wrapping_sub(cx);
    let stop = x.u16(STOP_X);
    set_dw(x, 5, d5);
    let before = if x.u8(GROUP) & 1 != 0 { (d5 as i16) < stop as i16 } else { (d5 as i16) > stop as i16 };
    if before {
        x.anim_step();
        return;
    }
    if x.u8(STOP_STEP) != 4 {
        let d1 = if x.u8(STOP_STEP) != 0 { 0x60 } else { 0x80 };
        if find_player(x, 0, d1, 0x30) {
            x.add8(STOP_STEP, 2);
            let rank = x.u8(STOP_STEP);
            set_stop(x, rank);
            x.anim_step();
            return;
        }
    }
    x.set8(f::SUB3, 4);
    if !find_player(x, 0, 0x180, 0x30) {
        set_speed_y(x, 0x28);
    } else {
        let player = load_target(x);
        x.set8(SIDE, 0);
        let mut gap = x.u16(Obj::Y).wrapping_sub(player.u16(x.b, Obj::Y)).wrapping_sub(4);
        if (gap as i16) < 0 {
            gap = gap.wrapping_neg();
            x.set8(SIDE, 0x10);
        }
        if gap > 0x30 {
            set_speed_y(x, 0x28);
        } else {
            set_speed_y(x, gap & 0x38);
        }
    }
    if x.u8(SIDE) != 0 {
        negate(x, Obj::AY);
    }
    x.anim_step();
}

/// Groupes 4 et 5.
fn halt_at(x: &mut Cx) {
    match x.u8(f::SUB3) {
        0 => halt_start(x),
        2 => halt_run(x),
        4 => side_turn(x),
        6 => leave(x),
        _ => {}
    }
}

/// Groupes 6 et 7 : comme 4 et 5, avec la morsure surveillée à chaque étape.
fn halt_and_bite(x: &mut Cx) {
    match x.u8(f::SUB3) {
        0 => {
            halt_start(x);
            x.set8(BITING, 0);
        }
        2 => {
            halt_run(x);
            bite_check(x);
        }
        4 => {
            side_turn(x);
            bite_check(x);
        }
        6 => {
            leave(x);
            bite_check(x);
        }
        _ => {}
    }
}

/// 0x65efe : virage de côté (le sens vient de `+0xa4` et de la parité de `+0x2f`).
fn side_turn(x: &mut Cx) {
    let mut d0 = x.u8(SIDE);
    let group = x.u8(GROUP);
    if group & 1 != 0 {
        d0 ^= group;
    }
    cross_turn(x, d0);
}

/// Sous-état 4 avec `+6` non nul (0x6628a) : se tourne vers un joueur puis remonte vers le haut.
fn turn_back(x: &mut Cx) {
    match x.u8(f::SUB3) {
        0 => {
            x.set8(f::SUB3, 4);
            let player = load_target(x);
            let (px, pz) = (player.u16(x.b, Obj::X), player.u16(x.b, Obj::Y));
            x.set16(0xb0, px);
            x.set16(0xb2, pz);
            lib::aim_to(x, Obj(x.c.a[1] & 0xff_ffff));
            let angle = x.c.d[0] as u8;
            x.set8(Obj::SPEED_INDEX, angle);
            if angle & 0xf != 0 {
                let side = (angle.wrapping_add(0x10) & 0x10) << 1;
                if side ^ x.u8(f::FLIP) != 0 {
                    x.set8(f::SUB3, 2);
                }
            }
            x.anim_step();
        }
        2 | 4 => {
            if x.u8(f::SUB3) == 2 {
                x.add8(f::SUB3, 2);
                let flip = x.u8(f::FLIP) ^ 0x20;
                x.set8(f::FLIP, flip);
            }
            x.add8(f::SUB3, 2);
            x.set8(TURN_PERIOD, 0x3c);
            speeds_by_level(x, SPEEDS_FLY);
            x.set16(Obj::VZ, -0x400i16 as u16);
            x.set16(Obj::AZ, 0);
            if x.u8(Obj::VARIANT) == 0 {
                x.set16(Obj::VZ, -0x200i16 as u16);
            }
            x.anim_step();
        }
        6 => {
            x.move_by_table_xy();
            x.move_z();
            let floor = if x.u8(Obj::VARIANT) == 0 { 0x30 } else { 0x10 };
            set_dw(x, 0, floor);
            if floor >= x.u16(Obj::Z) {
                x.set16(Obj::Z, floor);
                x.set16(Obj::VZ, 0);
                x.add8(f::SUB3, 2);
            }
            x.anim_step();
        }
        8 => {
            x.move_xy();
            let (cx, cy) = camera(x);
            if x.u8(Obj::VARIANT) == 0 {
                if find_player(x, -0x80, 0x40, 0x20) {
                    x.anim_step();
                } else {
                    resume_walk(x);
                    set_state_long(x, STANCE);
                }
                return;
            }
            let d5 = x.u16(Obj::X).wrapping_sub(cx).wrapping_add(0x40);
            let ok = d5 < 0x200 && x.u16(Obj::Y).wrapping_add(x.u16(Obj::Z)).wrapping_sub(cy).wrapping_add(0x10) <= 0x120;
            if !ok {
                x.set8(f::STATE, 6);
                detach_from_host(x);
            }
            x.anim_step();
        }
        _ => {}
    }
}

// --- sous-état 6 : bond -------------------------------------------------------------------------

fn pounce(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set16(Obj::VX, 0x400);
            x.set16(Obj::VZ, 0x400);
            x.set16(Obj::AX, 0);
            x.set16(Obj::AZ, 0);
            x.set8(0x6e, 0xc);
            if x.u8(f::FLIP) != 0 {
                negate(x, Obj::VX);
            }
            anim_play(x, 8);
        }
        2 => {
            x.move_xz();
            let n = x.u8(0x6e).wrapping_sub(1);
            x.set8(0x6e, n);
            if n == 0 {
                x.add8(f::SUB2, 2);
                x.set8(0x6e, 0x40);
            }
            x.anim_step();
        }
        4 => {
            let n = x.u8(0x6e).wrapping_sub(1);
            x.set8(0x6e, n);
            if n != 0 {
                x.anim_step();
                return;
            }
            x.set8(0xa5, 0);
            set_state_long(x, AFTER_POUNCE);
        }
        _ => {}
    }
}

/// Sous-état 8 : dormant ; réveillé (sous-état 0) quand il est à moins de 0x1c0 pixels de la caméra.
fn dormant(x: &mut Cx) {
    let cam = if var::CAMERA_SELECT.get(x.b) != 0 { var::CAMERA_ALT_X.get(x.b) } else { var::SCROLL_X.get(x.b) };
    let d0 = x.u16(Obj::X).wrapping_sub(cam).wrapping_add(0x20);
    if d0 <= 0x1c0 {
        x.set8(f::SUB, 0);
    }
}

// --- état 4 : touché ----------------------------------------------------------------------------

fn hit(x: &mut Cx) {
    let freeze = x.u8(f::FREEZE) as i8;
    if freeze > 0 {
        x.set8(f::FREEZE, (freeze - 1) as u8);
    } else {
        x.c.d[0] = x.u8(f::SUB) as u32;
        match x.u8(f::SUB) {
            0 => react_first(x),
            2 => recoil(x),
            4 => fall(x),
            _ => {}
        }
    }
    x.reinsert();
}

/// Sous-état 0 : un coup de code 0x5c secoue l'insecte quelques images (`+6` = 0, 2) avant le recul.
fn react_first(x: &mut Cx) {
    if x.u8(HIT_CODE) != 0x5c {
        return knock_back(x);
    }
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            shake_timer(x);
        }
        2 => {
            if var::PAUSE.get(x.b) == 0 {
                x.add8(f::SUB2, 2);
            }
            let n = x.u8(0xa2).wrapping_sub(1);
            x.set8(0xa2, n);
            if n != 0 {
                return;
            }
            lib::attack_effect(x, 0, 0);
            if !x.c.cond(5) {
                shake_timer(x);
            } else {
                x.set8(0xa2, 1);
            }
        }
        4 => knock_back(x),
        _ => {}
    }
}

/// 0x664b4 : durée de la secousse tirée au hasard.
fn shake_timer(x: &mut Cx) {
    let r = random(x) & 3;
    let t = x.rom8(SHAKE_TIMES + r);
    x.set8(0xa2, t);
}

/// 0x664f8 : recul, la face opposée au coup.
fn knock_back(x: &mut Cx) {
    x.add8(f::SUB, 2);
    x.set8(f::SUB2, 0);
    let freeze = x.u8(FREEZE_AT_HIT);
    x.set8(f::FREEZE, freeze);
    x.set16(Obj::VZ, 0x400);
    x.set16(Obj::AZ, -0x40i16 as u16);
    x.set16(Obj::VX, 0x200);
    x.set16(Obj::AX, 0);
    let flip = x.u8(0x4a) ^ 0x20;
    x.set8(f::FLIP, flip);
    if flip != 0 {
        negate(x, Obj::VX);
        negate(x, Obj::AX);
    }
    x.set8(0, 2);
    x.anim_start(ANIMS_HIT, 0);
}

/// Sous-état 2 : retombée puis clignotement jusqu'à la mort.
fn recoil(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            x.move_xz();
            if x.u8(0x29) != 0 {
                x.set16(Obj::VX, 0);
                x.set16(Obj::AX, 0);
            }
            if (x.u16(Obj::Z) as i16) <= 0 {
                x.add8(f::SUB2, 2);
                x.set32(Obj::Z, 0);
                x.set8(0xa2, 0x28);
                x.set16(0x30, 1);
                detach_from_host(x);
            }
            x.anim_step();
        }
        2 => {
            let n = x.u8(0xa2).wrapping_sub(1);
            x.set8(0xa2, n);
            if n == 0 {
                set_state_long(x, DEAD);
                return lib::object_event(x);
            }
            x.set_bit(1, 1, n & 3 != 0);
            x.anim_step();
        }
        _ => {}
    }
}

/// Sous-état 4 : saisi par un joueur, l'insecte se débat puis quitte l'écran.
fn fall(x: &mut Cx) {
    if paused(x) {
        return;
    }
    if x.u8(f::SUB2) == 0 {
        x.add8(f::SUB2, 2);
        x.set16(Obj::AX, 0);
        x.set16(Obj::AY, 0);
        let angle = if x.u8(f::FLIP) != 0 { 8 } else { 0x18 };
        x.set8(Obj::SPEED_INDEX, angle);
        return anim_play(x, 4);
    }
    x.move_by_table_xy();
    if !x.in_arena_default() {
        detach_from_host(x);
        set_state_long(x, DEAD);
    }
    x.anim_step();
}

fn free(x: &mut Cx) {
    x.free(&pools::ENEMY);
}

handlers! {
    0x65ae8 "enemy_9" run,
}
