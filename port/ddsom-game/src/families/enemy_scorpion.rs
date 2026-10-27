//! Ennemi « GiantScorpion » (ENEMY type 10, 0x6681c). Voir `docs/familles/enemy.md`.
//!
//! Variantes `+3` : 0 debout, 1 et 4 tombent d'en haut, 2 et 3 entrent en volant par un bord, 5 est le nid qui
//! engendre des scorpions (liste `+0x2f`) et attend d'être visible. Les valeurs `+6` d'arrivée (2, 4, 6, 8)
//! choisissent l'entrée, puis l'état actif :
//!
//! | `+5` | Rôle (h) |
//! |---|---|
//! | 0 | arrivée (chute, vol, nid dormant) |
//! | 2 | debout : marche d'avant en arrière (`+6` = 0 libre, 2 enfant d'un parent, 4 enfant libre) |
//! | 4, 10 | coups tirés au hasard (`+6` = 2, 4, 6, 8 ; tables par difficulté) |
//! | 6 | attente d'une durée tirée au hasard |
//! | 8 | coup spécial : demi-tour puis charge avec attaque `0x1f` |
//!
//! L'état touché a son propre recul (pas de table de réaction commune). Animations 0x163da8 (mouvements),
//! 0x163db8 (charge) et 0x163dba (touché).

use super::cx::{handlers, Cx};
use super::enemy_common::*;

const ANIMS: u32 = 0x163da8;
const ANIMS_STRIKE: u32 = 0x163db8;
const ANIMS_HIT: u32 = 0x163dba;

/// Champs propres à ce type.
mod g {
    pub const VARIANT: u32 = 0x3;
    pub const STEP: u32 = 0x7;
    pub const X: u32 = 0x8;
    pub const HEIGHT: u32 = 0xc;
    pub const DEPTH: u32 = 0x10;
    pub const SPEED_X: u32 = 0x14;
    pub const SPEED_Y: u32 = 0x16;
    pub const SPEED_Z: u32 = 0x18;
    pub const GRAVITY: u32 = 0x1a;
    pub const SPEED_X2: u32 = 0x1c;
    pub const ANGLE: u32 = 0x2d;
    pub const GROUP: u32 = 0x2f;
    pub const ANIM_FLAGS: u32 = 0x32;
    pub const STRIKE_KIND: u32 = 0x44;
    pub const STRIKE_POWER: u32 = 0x45;
    pub const PARENT: u32 = 0x5c;
    pub const WAIT: u32 = 0x6e;
    pub const TIMER: u32 = 0xa2;
    pub const START_X: u32 = 0xa4;
    pub const MOVE: u32 = 0xa6;
    pub const FROZEN: u32 = 0xa7;
    pub const EXIT_TIMER: u32 = 0xa8;
}
use g::*;
use crate::objects::pools;
use crate::objects::Courant;

/// Tables de la ROM (le suffixe dit l'espace de lecture : opcodes pour les `(table,PC,Dn)`, données sinon).
const SPAWN_LISTS: u32 = 0x669d6;
const SPAWN_ALLOCS: u32 = 0x669ea;
const ATTACK_PICK_A: u32 = 0x66e48;
const ATTACK_PICK_B: u32 = 0x67146;
const MOVE_PICK: u32 = 0x6773c;
const IDLE_TIMES: u32 = 0x66fbe;
const ATTACK_TABLE: u32 = 0x674fa;
const TIMER_ROW: u32 = 0x677f8;
const RETREAT_ROW: u32 = 0x677ce;
const STRIKE_TIMER: u32 = 0x670dc;
const SPEED_ROW: u32 = 0x670e0;

const STANCE: u32 = 0x0202_0000;
const REACH: u32 = 0x0208_0000;
const HELD: u32 = 0x0404_0000;
const DEAD: u32 = 0x0600_0000;
const REMOVE: u32 = 0x27ee;

pub fn run(x: &mut Cx) {
    if suspended(x) {
        return;
    }
    x.c.d[0] = 0;
    match x.u8(f::STATE) {
        st::INIT => init(x),
        st::ACTIVE => active(x),
        st::HIT => hit(x),
        st::DEAD => x.free(&pools::ENEMY),
        _ => {}
    }
}

// --- aides --------------------------------------------------------------------------------------

fn anim_play(x: &mut Cx, n: u32) {
    x.anim_start(ANIMS, n as u16);
}

/// Hauteur de départ d'une chute : l'objet apparaît au-dessus de l'écran.
fn drop_from_above(x: &mut Cx) {
    let cam = x.a5_u16(0x50a).wrapping_add(0x100);
    let depth = x.u16(DEPTH);
    x.set16(HEIGHT, cam.wrapping_sub(depth));
    set_dw(x, 0, depth);
    x.set16(SPEED_Z, 0);
    x.set16(GRAVITY, -0x40i16 as u16);
}

/// Abscisse de la caméra selon le mode (`A5+0x36` : second écran).
fn camera_x(x: &mut Cx) -> u16 {
    if x.a5_u8(0x36) != 0 { x.a5_u16(0x556) } else { x.a5_u16(0x506) }
}

/// 0x67808 : avance en X et en profondeur selon les vitesses (virgule fixe 8.8).
fn advance(x: &mut Cx) {
    for (pos, speed) in [(X, SPEED_X), (DEPTH, SPEED_X2)] {
        let d = ((x.u16(speed) as u32) << 16) as i32 >> 8;
        x.add32(pos, d as u32);
    }
}

/// 0x6771a : choisit le geste (`+0xa6`) dans la ligne de la difficulté.
fn pick_move(x: &mut Cx) {
    let row = ((x.a5_u16(0x7f92) >> 2) << 4) as u32;
    let r = rand_and(x, 0xf);
    let v = x.data8(MOVE_PICK + row + r);
    x.set8(MOVE, v);
}

/// 0x677e8 : durée du geste (`+0xa2`) selon le rang `d0` et le geste.
fn set_timer(x: &mut Cx, d0: u32) {
    let idx = ((d0 << 2) & !0xff) | (((d0 << 2) as u8).wrapping_add(x.u8(MOVE)) as u32);
    x.c.d[0] = idx;
    let t = x.rom8(TIMER_ROW + idx);
    x.set8(TIMER, t);
}

/// 0x677de : durée tirée au hasard (rang 0 à 3).
fn random_timer(x: &mut Cx) {
    let r = rand_and(x, 3);
    set_timer(x, r);
}

/// 0x670d2 : table de vitesses du geste.
fn speed_row(x: &mut Cx) {
    let idx = x.u8(MOVE) as u32;
    let d0 = x.rom8(SPEED_ROW + idx);
    x.select_speed(&crate::objects::motion::SPEEDS_B, (d0 as u32) as u16);
}

/// Choix du coup suivant dans la ligne de la difficulté : tire `+6` = 2 + octet de la table.
fn pick_attack(x: &mut Cx, table: u32) {
    x.add8(f::SUB2, 2);
    let row = ((x.a5_u16(0x7f92) >> 2) << 4) as u32;
    x.c.d[0] = row;
    let r = rand_and(x, 0xf);
    let v = x.data8(table + row + r);
    x.add8(f::SUB2, v);
    pick_move(x);
}

// --- état 0 : création --------------------------------------------------------------------------

fn init(x: &mut Cx) {
    if paused(x) {
        return;
    }
    x.add8(f::STATE, 2);
    x.set8(f::BOX_SET, 0x16);
    x.call(rom::INITIAL_HP);
    x.call(rom::IMMUNITIES);
    for (off, v) in [
        (0x70, 0),
        (0x7e, 4),
        (0x7f, 0),
        (0x6a, 2),
        (0x6c, 3),
        (0x74, 3),
        (0x76, 3),
        (0x69, 2),
        (0x78, 3),
        (0x8e, 0),
        (EXIT_TIMER, 0),
        (0x6d, 2),
        (0x72, 4),
        (0x73, 3),
        (0x68, 2),
        (f::SUB, 0),
    ] {
        x.set8(off, v);
    }
    x.set16(f::SUB2, 0);
    let d0 = 2 * x.u8(VARIANT) as u32;
    x.c.d[0] = d0;
    match x.u8(VARIANT) {
        0 => {
            x.set8(f::SUB, 2);
            anim_play(x, 0);
        }
        1 => {
            x.set8(f::SUB2, d0 as u8);
            drop_from_above(x);
            anim_play(x, 5);
        }
        2 | 3 => {
            x.set8(f::SUB2, d0 as u8);
            let mut shift = 0x190u16;
            set_dw(x, 0, shift);
            let cam = x.a5_u16(0x506);
            x.set16(X, cam);
            x.set16(SPEED_Z, 0x400);
            x.set16(GRAVITY, -0x40i16 as u16);
            x.set16(SPEED_X, -0x400i16 as u16);
            x.set16(SPEED_Y, 0);
            if x.u8(f::FLIP) != 0 {
                shift = -0x10i16 as u16;
                set_dw(x, 0, shift);
                negate(x, SPEED_X);
            }
            x.add16(X, shift);
            anim_play(x, 6);
        }
        4 => {
            x.set8(f::SUB2, 6);
            drop_from_above(x);
            anim_play(x, 5);
        }
        5 => {
            x.set8(f::SUB2, 8);
            anim_play(x, 0);
            let list = x.u8(GROUP) as u32;
            spawn_list(x, SPAWN_LISTS, SPAWN_ALLOCS, list);
        }
        _ => {}
    }
}

// --- état 2 : actif -----------------------------------------------------------------------------

fn active(x: &mut Cx) {
    let freeze = x.u8(f::FREEZE) as i8;
    if freeze > 0 {
        x.set8(FROZEN, 1);
        x.set8(f::FREEZE, (freeze - 1) as u8);
    } else if !paused(x) {
        x.c.d[0] = x.u8(f::SUB) as u32;
        match x.u8(f::SUB) {
            0 => arrival(x),
            2 => stance(x),
            4 => melee(x),
            6 => idle(x),
            8 => charge(x),
            10 => melee_b(x),
            _ => {}
        }
    }
    if x.u8(f::SUB) != 0 {
        zone_shadow(x);
    }
    x.reinsert();
}

/// Sous-état 0 : l'arrivée dépend de `+6` (posé à la création selon la variante).
fn arrival(x: &mut Cx) {
    match x.u8(f::SUB2) {
        2 => land(x, |x| {
            x.add8(f::SUB, 2);
            x.set16(f::SUB2, 0);
        }),
        4 => fly_in(x),
        6 => land(x, |x| {
            x.add8(f::SUB, 2);
            x.set8(f::SUB2, 2);
        }),
        8 => {
            let dx = x.u16(X).wrapping_sub(x.a5_u16(0x506)).wrapping_add(0x20);
            set_dw(x, 0, dx);
            if dx <= 0x1c0 {
                x.set8(f::SUB, 2);
                x.set8(f::SUB2, 4);
            }
        }
        _ => {}
    }
}

/// Chute : tant que l'altitude est positive ; à l'atterrissage `done` change de sous-état.
fn land(x: &mut Cx, done: impl FnOnce(&mut Cx)) {
    x.move_xz();
    if (x.u16(HEIGHT) as i16) > 0 {
        return;
    }
    x.set32(HEIGHT, 0);
    done(x);
}

/// Variante 2 : entrée en vol (animation 6), atterrissage (animation 7), reprise de la marche.
fn fly_in(x: &mut Cx) {
    match x.u8(STEP) {
        0 | 4 => {
            x.anim_step();
            if (x.u16(ANIM_FLAGS) as i16) < 0 {
                if x.u8(STEP) == 0 {
                    x.add8(STEP, 2);
                } else {
                    x.add8(f::SUB, 2);
                    x.set16(f::SUB2, 0);
                }
            }
        }
        2 => {
            x.move_xz();
            if (x.u16(HEIGHT) as i16) >= 0 {
                return;
            }
            x.set32(HEIGHT, 0);
            x.add8(STEP, 2);
            anim_play(x, 7);
        }
        _ => {}
    }
}

/// Sous-état 2 : debout. `+6` = 0 libre, 2 enfant d'un parent (`+0x5c`), 4 enfant libre.
fn stance(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => free_stance(x),
        2 => {
            let parent = x.linked(PARENT);
            if parent.u8(x.b, 4) == 4 {
                return set_state_long(x, HELD);
            }
            walk(x);
        }
        4 => walk(x),
        _ => {}
    }
}

fn free_stance(x: &mut Cx) {
    if grabbed(x) {
        return set_state_long(x, HELD);
    }
    if find_player(x, -0x28, 0x18, 8) {
        return set_state_long(x, REACH);
    }
    if find_player(x, -8, 0x48, 0x10) {
        return choose_attack(x);
    }
    walk(x);
}

/// 0x674ca : tire le prochain sous-état d'attaque dans la ligne de la difficulté.
fn choose_attack(x: &mut Cx) {
    let row = ((x.a5_u16(0x7f92) >> 1) << 5) as u32;
    let r = rand_and(x, 0x1f);
    let v = x.data8(ATTACK_TABLE + row + r);
    x.set8(f::SUB, v);
    x.add8(f::SUB, 2);
    x.set8(f::STATE, 2);
    x.set16(f::SUB2, 0);
}

/// Marche : aller (animation 1), demi-tour (4), retour.
fn walk(x: &mut Cx) {
    x.c.d[0] = x.u8(STEP) as u32;
    match x.u8(STEP) {
        0 => {
            x.add8(STEP, 2);
            x.set8(TIMER, 0x28);
            let px = x.u16(X);
            x.set16(START_X, px);
            x.set8(ANGLE, 0x18);
            if x.u8(f::FLIP) != 0 {
                x.set8(ANGLE, 8);
            }
            x.select_speed(&crate::objects::motion::SPEEDS_B, 0xd);
            anim_play(x, 1);
        }
        2 => {
            let d = ((x.u16(SPEED_X) as u32) << 16) as i32 >> 8;
            x.add32(X, d as u32);
            let n = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, n);
            if n != 0 {
                x.anim_step();
                return;
            }
            negate(x, SPEED_X);
            x.add8(STEP, 2);
            anim_play(x, 4);
        }
        4 => {
            x.anim_step();
            if (x.u8(ANIM_FLAGS) as i8) < 0 {
                x.set8(STEP, 2);
                let flip = x.u8(f::FLIP) ^ 0x20;
                x.set8(f::FLIP, flip);
                x.set8(TIMER, 0x28);
                anim_play(x, 1);
            } else {
                x.anim_step();
            }
        }
        _ => {}
    }
}

// --- sous-états 4 et 10 : coups ----------------------------------------------------------------

/// Sous-état 4 : `+6` = 0 tirage, 2 et 4 déplacements d'attaque, 6 demi-tour puis charge.
fn melee(x: &mut Cx) {
    if x.u8(f::SUB2) == 0 {
        pick_attack(x, ATTACK_PICK_A);
    }
    x.c.d[0] = x.u8(f::SUB2) as u32;
    match x.u8(f::SUB2) {
        2 => {
            if x.u8(STEP) != 0 {
                return attack_tail(x);
            }
            x.add8(STEP, 2);
            face(x, true);
            random_timer(x);
            attack_move(x);
        }
        4 => {
            if x.u8(STEP) != 0 {
                return attack_tail(x);
            }
            x.add8(STEP, 2);
            let r = rand_and(x, 0xf);
            let d0 = x.rom8(RETREAT_ROW + r) as u32;
            set_timer(x, d0);
            face(x, false);
            attack_move(x);
        }
        6 => turn_and_charge(x),
        _ => {}
    }
}

/// Sous-état 10 : même tirage avec une autre table ; `+6` = 2, 4, 6 déplacements, 8 pause.
fn melee_b(x: &mut Cx) {
    if x.u8(f::SUB2) == 0 {
        pick_attack(x, ATTACK_PICK_B);
    }
    x.c.d[0] = x.u8(f::SUB2) as u32;
    match x.u8(f::SUB2) {
        2 => {
            if x.u8(STEP) != 0 {
                return attack_tail(x);
            }
            x.add8(STEP, 2);
            set_timer(x, 1);
            let r = rand_and(x, 1);
            x.set8(ANGLE, (r << 4) as u8);
            attack_move(x);
        }
        4 | 6 => {
            if x.u8(STEP) != 0 {
                return attack_tail(x);
            }
            x.add8(STEP, 2);
            let rank = if x.u8(f::SUB2) == 4 { 1 } else { 2 };
            set_timer(x, rank);
            face(x, false);
            attack_move(x);
        }
        8 => {
            if x.u8(STEP) != 0 {
                let n = x.u8(TIMER).wrapping_sub(1);
                x.set8(TIMER, n);
                if n == 0 {
                    set_state_long(x, STANCE);
                }
                x.anim_step();
                return;
            }
            x.add8(STEP, 2);
            anim_play(x, 0);
        }
        _ => {}
    }
}

/// Direction de marche d'un geste : 8 ou 0x18 selon l'orientation ; `forward` pour avancer vers le côté regardé.
fn face(x: &mut Cx, forward: bool) {
    let flipped = x.u8(f::FLIP) != 0;
    x.set8(ANGLE, if flipped == forward { 8 } else { 0x18 });
}

/// 0x66f00 : vitesse et animation du geste, puis suite commune.
fn attack_move(x: &mut Cx) {
    x.c.d[0] = x.u8(MOVE) as u32;
    speed_row(x);
    let n = 1 + x.u8(MOVE) as u32;
    x.c.d[0] = n;
    anim_play(x, n);
    attack_tail(x);
}

/// 0x66f1c : avance, décompte et fin du geste.
fn attack_tail(x: &mut Cx) {
    advance(x);
    let n = x.u8(TIMER).wrapping_sub(1);
    x.set8(TIMER, n);
    if n == 0 {
        set_state_long(x, STANCE);
    }
    x.anim_step();
}

/// Sous-état 4, `+6` = 6 : se tourne vers la cible, puis lance le geste.
fn turn_and_charge(x: &mut Cx) {
    match x.u8(STEP) {
        0 => face_target(x),
        2 => wait_turn(x),
        4 => {
            x.add8(STEP, 2);
            random_timer(x);
            attack_move(x);
        }
        6 => attack_tail(x),
        _ => {}
    }
}

/// 0x67052 : angle vers la cible ; s'il est du côté regardé, animation 4 et attente, sinon étape 4.
fn face_target(x: &mut Cx) {
    load_target(x);
    x.call(0x1376);
    let angle = x.c.d[0] as u8;
    x.set8(ANGLE, angle);
    let side = ((angle as u16 & 0x10) * 2) as u8;
    if side != x.u8(f::FLIP) {
        x.set8(STEP, 4);
        return;
    }
    x.add8(STEP, 2);
    anim_play(x, 4);
}

/// 0x67088 : à la fin de l'animation de virage, l'orientation s'inverse.
fn wait_turn(x: &mut Cx) {
    if (x.u16(ANIM_FLAGS) as i16) < 0 {
        x.add8(STEP, 2);
        let flip = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
    }
    x.anim_step();
}

// --- sous-état 6 : attente ---------------------------------------------------------------------

fn idle(x: &mut Cx) {
    if x.u8(f::SUB2) == 0 {
        x.add8(f::SUB2, 2);
        let r = rand_and(x, 0xf);
        let t = x.data8(IDLE_TIMES + r);
        x.set8(WAIT, t);
        return anim_play(x, 0);
    }
    let n = x.u8(WAIT).wrapping_sub(1);
    x.set8(WAIT, n);
    if n != 0 {
        x.anim_step();
    } else {
        set_state_long(x, STANCE);
    }
}

// --- sous-état 8 : coup spécial ---------------------------------------------------------------

fn charge(x: &mut Cx) {
    match x.u8(STEP) {
        0 => face_target(x),
        2 => wait_turn(x),
        4 => {
            x.add8(STEP, 2);
            x.set8(STRIKE_KIND, 0x1f);
            x.set8(STRIKE_POWER, 2);
            pick_move(x);
            x.anim_start(ANIMS_STRIKE, 0);
            x.set8(FROZEN, 0);
            let m = x.u8(MOVE) as u32;
            let t = x.rom8(STRIKE_TIMER + m);
            x.set8(TIMER, t);
            speed_row(x);
        }
        6 => {
            advance(x);
            x.anim_step();
            let n = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, n);
            if n == 0 {
                set_state_long(x, STANCE);
            }
        }
        _ => {}
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
            0 => knock_back(x),
            2 => recoil(x),
            4 => grabbed_flight(x),
            _ => {}
        }
    }
    x.reinsert();
}

fn knock_back(x: &mut Cx) {
    x.add8(f::SUB, 2);
    let freeze = x.u8(0x56);
    x.set8(f::FREEZE, freeze);
    x.set16(SPEED_Z, 0x400);
    x.set16(GRAVITY, -0x40i16 as u16);
    x.set16(SPEED_X, 0x200);
    x.set16(SPEED_Y, 0);
    let flip = x.u8(0x4a) ^ 0x20;
    x.c.d[2] = x.c.d[2] & !0xff | flip as u32;
    x.set8(f::FLIP, flip);
    if flip != 0 {
        negate(x, SPEED_X);
        negate(x, SPEED_Y);
    }
    x.set8(0, 2);
    x.anim_start(ANIMS_HIT, 0);
    zone_shadow(x);
}

fn recoil(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            x.move_xz();
            if x.u8(0x29) != 0 {
                x.set16(SPEED_X, 0);
                x.set16(SPEED_Y, 0);
            }
            if (x.u16(HEIGHT) as i16) <= 0 {
                x.add8(f::SUB2, 2);
                x.set32(HEIGHT, 0);
                x.set8(TIMER, 0x28);
                x.set16(0x30, 1);
                x.call(REMOVE);
            }
            x.anim_step();
        }
        2 => {
            let n = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, n);
            if n == 0 {
                set_state_long(x, DEAD);
                x.call(0x19ad6);
            } else {
                x.set_bit(1, 1, n & 3 != 0);
                x.anim_step();
            }
        }
        _ => {}
    }
    zone_shadow(x);
}

/// Sous-état 4 : saisi par un joueur, le scorpion court vers le bord de l'écran le plus proche.
fn grabbed_flight(x: &mut Cx) {
    if x.a5_u8(0x3d) != 0 {
        return;
    }
    x.c.d[0] = x.u8(f::SUB2) as u32;
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            let cam = camera_x(x);
            set_dw(x, 0, cam);
            x.set16(SPEED_X, 0x300);
            let mut want = 0x20;
            let dx = x.u16(X).wrapping_sub(cam);
            set_dw(x, 1, dx);
            if (dx as i16) < 0xc0 {
                negate(x, SPEED_X);
                want = 0;
            }
            x.c.d[2] = want as u32;
            if want != x.u8(f::FLIP) {
                return anim_play(x, 4);
            }
            run_off(x);
        }
        2 => {
            if x.anim_step() {
                let flip = x.u8(f::FLIP) ^ 0x20;
                x.set8(f::FLIP, flip);
                run_off(x);
            }
        }
        4 => {
            x.move_x();
            if !x.in_arena_default() {
                x.call(REMOVE);
                set_state_long(x, DEAD);
            }
            if x.u8(EXIT_TIMER) != 0 {
                let n = x.u8(EXIT_TIMER).wrapping_sub(1);
                x.set8(EXIT_TIMER, n);
                zone_shadow(x);
            }
            x.anim_step();
        }
        _ => {}
    }
}

/// 0x673e4 : le scorpion court (animation 2) pendant 0xff images au plus.
fn run_off(x: &mut Cx) {
    x.add8(f::SUB2, 2);
    x.set8(EXIT_TIMER, 0xff);
    anim_play(x, 2);
}

handlers! {
    0x6681c "enemy_10" run,
}
