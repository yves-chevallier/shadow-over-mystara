//! Boss de type 10 (0x8ca4a), scènes 65 et 66 : Nagpa (h). Voir `docs/familles/boss.md`.
//!
//! Même gabarit que les boss 17 et 6 (états `+4`, groupes `+5`, étapes `+6` / `+7`, états posés en longs), en
//! plus court : il apparaît (cinématique d'ouverture : il crée un ennemi de type 18 et un boss de type 19, variante 1),
//! marche vers un joueur, saute sur lui ou lui lance un tir (ESHL 0x21), puis meurt par une chute en trois temps.
//!
//! | `+5` | Rôle |
//! |---|---|
//! | 0 | ouverture : attend la scène (`A5+0x2e` = 6), crée ses deux compagnons, se présente |
//! | 2 | combat : attente (`+6` = 0), demi-tour (2), marche (4), saut (6) |
//! | 4 | tir à distance : récupération (`+6` = 0), tir avec cinématique (2) |
//! | 6 | attente sans joueur |
//! | état 4 | touché : réactions (table 0x8d176 dont 0x8d294 est propre), relevé, mort |
//!
//! Animations : 0x17b14e (mouvements), 0x17b162 (tirs), 0x17b166 (recul, mort).

use super::boss_common::*;
use super::cx::handlers;

mod g {
    pub const STUN: u32 = 0x56;
    pub const WALK_STEPS: u32 = 0x6b;
    pub const APPROACH: u32 = 0x6e;
    pub const WALK_DELAY: u32 = 0x71;
    pub const HITS: u32 = 0x84;
    pub const SHAKE: u32 = 0x90;
    pub const ANIM_KIND: u32 = 0xa0;
    pub const ANIM_LAST: u32 = 0xa1;
    pub const SAVED_STATE: u32 = 0xa2;
    pub const TIMER: u32 = 0xa6;
    pub const SHOT: u32 = 0xa8;
    pub const JUMP_X: u32 = 0xaa;
    pub const JUMP_Y: u32 = 0xac;
    pub const WAIT_BLINK: u32 = 0xae;
    pub const SAVED_X: u32 = 0xa;
}
use g::*;
use crate::objects::pools;
use crate::objects::Courant;

const SET_BODY: u32 = 0x17b14e;
const SET_SHOT: u32 = 0x17b162;
const SET_DIE: u32 = 0x17b166;
/// Jeux de marche (0x103e0) selon le sens (Z de 0x10560).
const WALK_A: u32 = 0x17b21c;
const WALK_B: u32 = 0x17b2a0;

const WAIT: u32 = 0x0206_0000;
const FIGHT: u32 = 0x0202_0000;
const INTERRUPTED: u32 = 0x0202_0200;
const SHOOT_FAR: u32 = 0x0204_0200;
const SHOOT_NEAR: u32 = 0x0204_0000;
const REACTIONS: u32 = 0x8d176;
const STUN_REACTION: u32 = 0x8d294;

pub fn run(x: &mut Cx) {
    if suspended(x) {
        return;
    }
    x.tick_counters();
    match x.u8(f::STATE) {
        st::INIT => init(x),
        st::ACTIVE => active(x),
        st::HIT => hit(x),
        _ => x.free(&pools::BOSS),
    }
    if x.u8(0) != 0 {
        x.reinsert();
    }
}

// --- aides --------------------------------------------------------------------------------------

fn body(x: &mut Cx, n: u32) {
    x.anim_start(SET_BODY, n as u16);
}

fn players_alive(x: &mut Cx) -> bool {
    var::PLAYERS_ALIVE.get(x.b) != 0
}

fn wait(x: &mut Cx) {
    x.set32(f::STATE, WAIT);
}

fn zone_and_shadow(x: &mut Cx) {
    lib::keep_in_zone(x);
    lib::collide_terrain(x);
}

/// 0x8d526 : choisit l'étape suivante du combat (`+6` = 0, 4 ou 6) d'après une table de 32 octets.
fn next_action(x: &mut Cx) {
    let r = random(x) & 0x1f;
    x.set32(f::STATE, FIGHT);
    let step = x.rom8(0x8d540 + r);
    x.set8(f::SUB2, step);
}

/// 0x8d560 : interrompt l'action (état mémorisé).
fn interrupt(x: &mut Cx) {
    let s = x.u32(f::STATE);
    x.set32(SAVED_STATE, s);
    x.set32(f::STATE, INTERRUPTED);
}

fn needs_turn(x: &mut Cx) -> bool {
    !lib::faces_target(x)
}

fn facing_forward(x: &mut Cx) -> bool {
    lib::moves_forward(x)
}

/// Un joueur est-il dans la boîte ? 0 sinon (0x8d4ee, 0x8d50a).
fn probe(x: &mut Cx, d0: i16, d1: u16, d2: u16) -> bool {
    if !lib::target_in_box(x, d0, d1, d2) {
        return false;
    }
    lib::turn_decision(x) != lib::Turn::None
}

// --- état 0 : création ------------------------------------------------------------------------

fn init(x: &mut Cx) {
    lib::clear_popup(x);
    lib::empty_popups(x);
    lib::clear_all_popups(x);
    x.add8(f::STATE, 2);
    x.set_a5_u8(0x50, 1);
    x.set_a5_u32(0x38, 0x0459_5900);
    var::SCENE_LEVEL.set(x.b, 0);
    x.set8(0x2a, 1);
    x.set8(0x2b, 0x18);
    x.set16(0x8, 0xf0);
    x.set16(0x10, 0x160);
    x.set16(0xc, 0);
    x.set8(f::BOX_SET, 0x31);
    lib::initial_hp_boss(x);
    pick_target(x);
    x.set8(0x93, 1);
    x.set8(0x9a, 0xff);
    for off in [0x76, 0x70, 0x6a, 0x75, 0x80, 0x68, 0x69, 0x67] {
        x.set8(off, 0);
    }
    for (off, v) in [(0x72, 1), (0x73, 1), (0x6c, 6), (0x6d, 6), (0x74, 7), (0x78, 0xff), (0x8e, 3), (0x6f, 3)] {
        x.set8(off, v);
    }
    x.set32(f::IMMUNITY, 0xffff_ffff);
    x.set8(ANIM_KIND, 0);
    x.set8(ANIM_LAST, 0xff);
    for (off, v) in [(0x7e, 0x21), (0x7f, 0), (0x81, 0)] {
        x.set8(off, v);
    }
    body(x, 0);
    intro(x);
}

// --- état 2 : actif -----------------------------------------------------------------------------

fn active(x: &mut Cx) {
    if tst8(x, f::FREEZE).gt() {
        sub8(x, f::FREEZE, 1);
        return zone_and_shadow(x);
    }
    if var::PAUSE.get(x.b) == 0 {
        match x.u8(f::SUB) {
            0 => intro(x),
            2 => fight(x),
            4 => shoot(x),
            6 => wait_state(x),
            _ => {}
        }
        if x.u8(f::SUB) == 0 {
            return;
        }
    }
    zone_and_shadow(x);
}

// --- +5 = 0 : ouverture -------------------------------------------------------------------------

fn intro(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            if x.a5_u8(0x2e) != 6 {
                x.anim_step();
                return;
            }
            x.add8(f::SUB2, 2);
            x.set16(TIMER, 0x20);
            x.anim_start(SET_BODY, 6);
            lib::sound(x, 0xe4a18);
        }
        2 => {
            if sub16(x, TIMER, 1).ne() {
                x.anim_step();
                return;
            }
            x.add8(f::SUB2, 2);
            x.set16(TIMER, 0x60);
            // un ennemi (type 18, variante 0xff) et un boss (type 19, variante 1) liés à Nagpa
            let enemy = alloc(x, &pools::ENEMY);
            enemy.set_u8(x.b, 0, 1);
            enemy.set_u8(x.b, 2, 0x12);
            enemy.set_u8(x.b, 3, 0xff);
            enemy.set_u16(x.b, f::HOST, x.o.0 as u16);
            let boss = alloc(x, &pools::BOSS);
            boss.set_u8(x.b, 0, 1);
            boss.set_u8(x.b, 2, 0x13);
            boss.set_u8(x.b, 3, 1);
            boss.set_u16(x.b, f::HOST, x.o.0 as u16);
            body(x, 0);
        }
        4 => {
            if sub16(x, TIMER, 1).ne() {
                x.anim_step();
                return;
            }
            x.add8(f::SUB2, 2);
            x.anim_start(SET_SHOT, 0);
        }
        6 => {
            if !x.anim_step() {
                return;
            }
            x.add8(f::SUB, 2);
            x.set16(f::SUB2, 0);
            body(x, 0);
            lib::set_immunities(x);
            lib::create_set_14(x);
        }
        _ => {}
    }
}

// --- +5 = 2 : combat -----------------------------------------------------------------------------

fn fight(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => idle(x),
        2 => turn(x),
        4 => walk(x),
        6 => jump(x),
        _ => {}
    }
}

fn idle(x: &mut Cx) {
    if !players_alive(x) {
        return x.set32(f::STATE, WAIT);
    }
    if x.u8(0x7) == 0 {
        x.add8(0x7, 2);
        lib::draw_6e(x);
        x.set8(ANIM_KIND, 0);
        if x.u8(ANIM_LAST) != 0 {
            x.set8(ANIM_LAST, 0);
            body(x, 0);
        }
        return;
    }
    if sub8(x, APPROACH, 1).mi() {
        return next_action(x);
    }
    if needs_turn(x) {
        return interrupt(x);
    }
    x.anim_step();
}

fn turn(x: &mut Cx) {
    if x.u8(0x7) == 0 {
        x.add8(0x7, 2);
        x.set8(ANIM_KIND, 1);
        body(x, 1);
        return;
    }
    if !x.anim_step() {
        return;
    }
    let side = x.u8(f::FLIP) ^ 0x20;
    x.set8(f::FLIP, side);
    body(x, 0);
    next_action(x);
}

fn walk(x: &mut Cx) {
    if !players_alive(x) {
        return wait(x);
    }
    if x.u8(0x7) != 0 {
        return keep_walking(x);
    }
    walk_steps(x);
    walk_delay(x);
    x.set8(ANIM_KIND, 2);
    x.add8(0x7, 2);
    x.set16(0x16, 0);
    x.set16(0x1e, 0);
    if !pick_target(x) {
        return wait(x);
    }
    lib::approach_target(x);
    speed(x, 0xd);
    let last = x.u8(ANIM_LAST);
    if last == 2 {
        let kind = x.u8(ANIM_KIND);
        x.set8(ANIM_LAST, kind);
        return select_walk(x, false);
    }
    let kind = x.u8(ANIM_KIND);
    x.set8(ANIM_LAST, kind);
    let n = if facing_forward(x) { 2 } else { 3 };
    body(x, n);
}

/// Choisit l'animation de marche par l'image-clé (0x103e0) ; `mirror` : le test de sens est inversé.
fn select_walk(x: &mut Cx, mirror: bool) {
    let z = lib::moves_forward(x);
    let set = if z != mirror { WALK_A } else { WALK_B };
    lib::switch_keeping_time(x, set);
}

fn keep_walking(x: &mut Cx) {
    if sub8(x, WALK_DELAY, 1).mi() {
        if !sub8(x, WALK_STEPS, 1).pl() {
            return next_action(x);
        }
        lib::approach_target(x);
        speed(x, 0xd);
        walk_delay(x);
        return select_walk(x, true);
    }
    if probe(x, -0xc0, 0x40, 0xc) {
        return x.set32(f::STATE, SHOOT_FAR);
    }
    if probe(x, -0x40, 0x30, 6) {
        return x.set32(f::STATE, SHOOT_NEAR);
    }
    if lib::target_in_box(x, -0x10, 0x10, 8) {
        return x.set32(f::STATE, SHOOT_FAR);
    }
    if needs_turn(x) {
        return interrupt(x);
    }
    x.move_by_table_xy();
    x.anim_step();
}

fn walk_delay(x: &mut Cx) {
    let level = var::RANK.get(x.b) as u32;
    let v = x.rom8(0x8d47c + level);
    x.set8(WALK_DELAY, v);
}

fn walk_steps(x: &mut Cx) {
    let r = random(x) & 0x1f;
    let v = x.rom8(0x8d4ce + r);
    x.set8(WALK_STEPS, v);
}

/// +6 = 6 : saut sur la cible.
fn jump(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            x.set8(ANIM_KIND, 5);
            let px = var::SCROLL_X.get(x.b);
            let py = var::SCROLL_Y.get(x.b);
            x.set16(JUMP_X, px);
            x.set16(JUMP_Y, py);
            let down = ((random(x) & 3) << 4) as u16 + 0x30;
            let y = x.u16(JUMP_Y).wrapping_add(down);
            x.set16(JUMP_Y, y);
            let across = ((random(x) & 7) << 4) as u16;
            let xx = x.u16(JUMP_X).wrapping_add(across);
            x.set16(JUMP_X, xx);
            set_dw(x, 2, 0xc0);
            let found = lib::pick_target_any(x);
            // Glitch d'origine : si personne n'est trouvé, D2 reste ce que 0x43ae a laissé
            let mut d2 = x.c.d[2] as u16;
            if found {
                d2 = 0x30;
                let target = x.linked(f::TARGET);
                let edge = var::SCROLL_X.get(x.b).wrapping_add(0xc0);
                let tx = target.u16(x.b, 0x8);
                if !(edge <= tx) {
                    d2 = 0xe0;
                }
            }
            let xx = x.u16(JUMP_X).wrapping_add(d2);
            x.set16(JUMP_X, xx);
            body(x, 7);
            lib::sound(x, 0xecf10);
        }
        2 => {
            if x.anim_step() {
                x.add8(0x7, 2);
                let (jx, jy) = (x.u16(JUMP_X), x.u16(JUMP_Y));
                x.set16(0x8, jx);
                x.set16(0x10, jy);
                x.set8(f::FLIP, 0);
                if random(x) & 1 != 0 {
                    x.set8(f::FLIP, 0x20);
                }
                body(x, 8);
                lib::sound(x, 0xecf10);
            } else if x.u8(f::KEY_FRAME) != 0 {
                x.set8(f::KEY_FRAME, 0);
                x.set8(0x2b, 0x16);
            }
        }
        4 => {
            if x.anim_step() {
                x.set32(f::STATE, FIGHT);
                body(x, 0);
            } else if x.u8(f::KEY_FRAME) != 0 {
                x.set8(f::KEY_FRAME, 0);
                x.set8(0x2b, 0x18);
            }
        }
        _ => {}
    }
}

// --- +5 = 4 : tir ---------------------------------------------------------------------------------

fn shoot(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => match x.u8(0x7) {
            0 => {
                x.add8(0x7, 2);
                x.anim_start(SET_SHOT, 0);
                lib::sound(x, 0xe48da);
            }
            2 => {
                if x.anim_step() {
                    next_action(x);
                }
            }
            _ => {}
        },
        2 => cinematic_shot(x),
        _ => {}
    }
}

/// 0x8d0a0 : fin du tir.
fn shot_end(x: &mut Cx) {
    x.add8(0x7, 2);
    x.set_a5_u16(0x51e, 0);
    x.anim_start(SET_SHOT, 0);
}

fn cinematic_shot(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            x.set_a5_u16(0x51e, 4);
            set_abs8(x, 0xff8043, 0x7f);
            x.set8(0, 2);
            x.anim_start(SET_SHOT, 1);
            lib::sound(x, 0xe48da);
        }
        2 => {
            x.anim_step();
            if x.u8(f::KEY_FRAME) == 0 {
                return;
            }
            x.add8(0x7, 2);
            x.set16(TIMER, 0xa);
            x.set8(f::KEY_FRAME, 0);
            let Some(shot) = x.alloc(&pools::ESHL) else { return shot_end(x) };
            shot.set_u8(x.b, 0, 1);
            shot.set_u8(x.b, 2, 0x21);
            shot.set_u16(x.b, 0x42, x.o.0 as u16);
            let side = x.u8(f::FLIP);
            shot.set_u8(x.b, f::FLIP, side ^ 0x20);
            for off in [0x8u32, 0xc, 0x10] {
                let v = x.u16(off);
                shot.set_u16(x.b, off, v);
            }
            let dx: i16 = if side == 0 { -0x4c } else { 0x4c };
            let px = shot.u16(x.b, 0x8).wrapping_add(dx as u16);
            let pz = shot.u16(x.b, 0xc).wrapping_add(0x20);
            shot.set_u16(x.b, 0x8, px);
            shot.set_u16(x.b, 0xc, pz);
            x.set16(SHOT, shot.0 as u16);
        }
        4 => {
            if sub16(x, TIMER, 1).gt() {
                x.anim_step();
                return;
            }
            x.add8(0x7, 2);
            x.set16(TIMER, 0x20);
            let shot = x.linked(SHOT);
            shot.set_u8(x.b, 0x58, 1);
            lib::sound(x, 0xe4a82);
            x.anim_step();
        }
        6 => {
            if sub16(x, TIMER, 1).gt() {
                x.anim_step();
                return;
            }
            x.add8(0x7, 2);
            let shot = x.linked(SHOT);
            shot.set_u8(x.b, 0x59, 1);
            x.anim_step();
        }
        8 => {
            let shot = x.linked(SHOT);
            if shot.u8(x.b, 0x5a) != 0 {
                return shot_end(x);
            }
            x.anim_step();
        }
        10 => {
            if x.anim_step() {
                set_abs8(x, 0xff8043, 0xff);
                x.set8(0, 1);
                next_action(x);
            }
        }
        _ => {}
    }
}

// --- +5 = 6 : attente -------------------------------------------------------------------------

fn wait_state(x: &mut Cx) {
    if x.u8(f::SUB2) == 0 {
        x.add8(f::SUB2, 2);
        x.set8(WAIT_BLINK, 0xfa);
        x.anim_start(SET_BODY, 6);
        return lib::sound(x, 0xe4a18);
    }
    if var::PLAYERS_ALIVE.get(x.b) != 0 {
        lib::pick_target_any(x);
        return next_action(x);
    }
    if !sub8(x, WAIT_BLINK, 1).eq() {
        x.anim_step();
        return;
    }
    x.set8(WAIT_BLINK, 0xfa);
    lib::sound(x, 0xe4a18);
    x.anim_step();
}

// --- état 4 : touché --------------------------------------------------------------------------------

fn hit(x: &mut Cx) {
    if tst8(x, f::FREEZE).gt() {
        sub8(x, f::FREEZE, 1);
    } else {
        match x.u8(f::SUB) {
            0 | 2 => reaction(x),
            4 => get_up(x),
            6 | 8 => down(x),
            _ => {}
        }
    }
    x.set8(0x67, 0);
    lib::collide_terrain(x);
}

/// 0x8d57a : plus de PV ? Passe alors au groupe 6 (mort).
fn out_of_hp(x: &mut Cx) -> bool {
    if tst16(x, f::HP).gt() {
        return false;
    }
    x.set8(f::SUB, 6);
    x.set8(0, 2);
    true
}

fn reaction(x: &mut Cx) {
    x.set8(0x2b, 0x18);
    if x.u8(f::SUB2) == 0 {
        let flags = x.a5_u8(0x43) | 0x80;
        x.set_a5_u8(0x43, flags);
        if out_of_hp(x) {
            return down(x);
        }
    }
    react_with(x, REACTIONS, STUN_REACTION, stun);
}

/// 0x8d294 : étourdissement.
fn stun(x: &mut Cx) {
    sub16(x, HITS, 1);
    match x.u8(f::SUB2) {
        0 => {
            let px = x.u16(0x8);
            x.set16(SAVED_X, px);
            sub16(x, HITS, 0xa);
            x.set8(0, 2);
            x.add8(f::SUB2, 2);
            let stun = x.u8(STUN);
            x.set8(f::FREEZE, stun);
            x.set8(SHAKE, 0x10);
            x.anim_start(SET_DIE, 8);
            lib::sound(x, 0xe4944);
        }
        2 => {
            if x.u8(SHAKE) == 0 {
                x.add8(f::SUB2, 2);
                let px = x.u16(SAVED_X);
                x.set16(0x8, px);
                return;
            }
            sub8(x, SHAKE, 1);
            let mut d = 2i16;
            if x.u8(SHAKE) & 1 != 0 {
                d = -d;
            }
            if x.u8(f::FLIP) != 0 {
                d = -d;
            }
            x.add16(0x8, d as u16);
        }
        4 => {
            x.set32(0x58, 0);
            if !tst16(x, HITS).gt() {
                x.set8(0, 2);
            }
            x.set32(0x16, 0);
            x.set32(f::STATE, 0x0404_0200);
            x.set16(HITS, 1);
            x.anim_step();
        }
        _ => {}
    }
}

fn get_up(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.anim_start(SET_BODY, 9);
        }
        2 => {
            if var::PAUSE.get(x.b) != 0 || sub16(x, HITS, 1).gt() {
                return;
            }
            x.set8(0, 2);
            x.add8(f::SUB2, 2);
            x.set16(HITS, 0);
            x.set8(0x86, 0);
            x.set8(0x49, 0);
        }
        4 => {
            if var::PAUSE.get(x.b) != 0 {
                return;
            }
            if x.anim_step() {
                x.set32(f::STATE, 0x0202_0600);
                x.set8(0, 1);
                x.anim_start(SET_BODY, 7);
            }
        }
        _ => {}
    }
}

fn down(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(0, 2);
            x.set_a5_u8(0x42, 1);
            x.set_a5_u8(0x50, 0);
            let kind = x.u8(0x2);
            set_abs8(x, 0xff2a84, kind);
            set_abs8(x, 0xff2a85, 0);
            let bar = x.u8(0x7e);
            set_abs8(x, 0xff2a86, bar);
            x.set32(f::IMMUNITY, 0x0fff_ffff);
            lib::flash_and_shake(x);
            lib::sound(x, 0xe0a10);
            lib::sound(x, 0xe3b6c);
            x.set16(0x16, 0);
            x.set16(0x18, 0x500);
            x.set16(0x1a, (-0x38i16) as u16);
            let mut vx: i16 = 0x200;
            let mut side = 0;
            let edge = var::CAMERA1_X.get(x.b).wrapping_add(0xc0);
            if !Fl::sub16(edge, x.u16(0x8)).ge() {
                vx = -vx;
                side = 0x20;
            }
            x.set16(0x14, vx as u16);
            x.set8(f::FLIP, side);
            x.anim_start(SET_DIE, 0x15);
            lib::sound(x, 0xe49ae);
        }
        2 => {
            x.move_xz();
            if cmp16(x, 0x18, 0x100).le() {
                x.add8(f::SUB2, 2);
                x.anim_start(SET_DIE, 0x16);
            } else {
                x.anim_step();
            }
        }
        4 => {
            x.move_xz();
            if cmp16(x, 0x18, 0xff00).le() {
                x.add8(f::SUB2, 2);
                x.anim_start(SET_DIE, 0x17);
            }
        }
        6 => {
            x.move_xz();
            if tst16(x, 0xc).gt() {
                return;
            }
            x.add8(f::SUB2, 2);
            x.set16(0xc, 0);
            lib::sound(x, 0xecabc);
            x.anim_start(SET_DIE, 0x18);
        }
        8 => {
            if x.anim_step() {
                x.add8(f::SUB2, 2);
            }
        }
        _ => {}
    }
}

handlers! {
    0x8ca4a "boss_10" run,
}
