//! Boss de type 5 (0x8341c), scène 58 : Ezerhorden (h). Voir `docs/familles/boss.md`.
//!
//! Gabarit ancien, sans cinématique : l'état `+4` (0 création, 2 actif, 4 touché, 6 libération) et le groupe
//! `+5` (multiples de 4) sont posés champ par champ, `+6` est l'étape. `0x83e3e` (`next_action`) choisit le
//! groupe suivant d'après la position de la cible.
//!
//! | `+5` | Rôle |
//! |---|---|
//! | 0 | arrivée : le bond d'entrée, puis la cinématique d'ouverture (attend la caméra `A5+0x2c` = 0xc) |
//! | 4 | marche lente sur place (`+0xa0` images) |
//! | 8, 0x14 | déplacement à vitesse choisie par la difficulté (X et Y), sens selon `+0x2c` |
//! | 0xc | saisie d'un joueur : le draine puis le relâche |
//! | 0x10 | attaque simple (animation jusqu'au bout) |
//! | 0x18 | poursuite : tourne vers la cible puis saisit |
//! | état 4 | touché : réactions, chute en tourbillon (`+0x49` = 0xa8), mort |
//!
//! Animations : 0x175da6 (corps), 0x175dc4 (attaques), 0x175e0c (chute, mort).

use super::boss_common::*;
use super::cx::handlers;
use crate::objects::Obj;

mod g {
    pub const STUN: u32 = 0x56;
    pub const FRAME_KIND: u32 = 0x44;
    pub const REACTION: u32 = 0x49;
    pub const BITING: u32 = 0xa7;
    pub const STEPS: u32 = 0xa3;
    pub const ANIM_N: u32 = 0xa4;
    pub const ANIM_SET: u32 = 0xa5;
    pub const SAVED_STATE: u32 = 0xa8;
    pub const COUNT: u32 = 0xa0;
    pub const BLINK: u32 = 0xa2;
    pub const COOL: u32 = 0xa6;
    pub const IN_ATTACK: u32 = 0xac;
    pub const FALL_TIMER: u32 = 0x90;
    pub const SPIN_DELAY: u32 = 0x87;
    pub const DEATH_FLAG: u32 = 0x91;
}
use g::*;
use crate::objects::pools;
use crate::objects::Courant;

const SET_BODY: u32 = 0x175da6;
const SET_ATTACK: u32 = 0x175dc4;
const SET_FALL: u32 = 0x175e0c;
/// Table des vecteurs (X, Y) par angle (32 × 4 octets), parcourue pendant le tourbillon.
const SPIN_VECTORS: u32 = 0x1c94b6;

mod call {
    pub const ANGLE_TO_TARGET: u32 = 0x1376;
}

/// Entrée : le jeu ne passe pas par le hook de pause.
pub fn run(x: &mut Cx) {
    match x.u8(f::STATE) {
        st::INIT => init(x),
        st::ACTIVE => active(x),
        st::HIT => hit(x),
        _ => x.free(&pools::BOSS),
    }
}

// --- aides --------------------------------------------------------------------------------------

/// 0x83dfe, 0x83e12, 0x83e28 : démarre le script `n` d'un des trois jeux d'animations.
fn body(x: &mut Cx, n: u32) {
    x.set8(ANIM_N, n as u8);
    x.set8(ANIM_SET, 0);
    x.anim_start(SET_BODY, n as u16);
}

fn attack_anim(x: &mut Cx, n: u32) {
    x.set8(ANIM_N, n as u8);
    x.set8(ANIM_SET, 1);
    x.anim_start(SET_ATTACK, n as u16);
}

fn fall_anim(x: &mut Cx, n: u32) {
    x.set8(ANIM_N, n as u8);
    x.set8(ANIM_SET, 2);
    x.anim_start(SET_FALL, n as u16);
}

/// 0x83dde : fige-la un cran d'animation demandé par `+0x91` (bit 7 = demande).
fn take_frame_request(x: &mut Cx) {
    let v = x.u8(DEATH_FLAG);
    if v & 0x80 == 0 {
        return;
    }
    x.set8(0x2b, v & 0x7f);
    x.set8(0x2a, 1);
    x.set8(DEATH_FLAG, 0);
}

/// Index dans les tables de difficulté : `(difficulté & 0xfc) / 2` (mots).
fn level_index(x: &mut Cx) -> u32 {
    (x.a5_u16(var::DIFFICULTY) as u32 & 0xfc) >> 1
}

/// Appelle une routine du moteur avec A0 = `list` (puis rétablit l'objet).
fn call_on_list(x: &mut Cx, list: u32, addr: u32) {
    let me = x.o.0;
    x.c.a[0] = list;
    crate::call(x.c, x.b, addr);
    x.c.a[0] = me;
}

fn step(x: &mut Cx) -> bool {
    x.anim_step()
}

// --- état 0 : création, état 2 : actif ------------------------------------------------------------

fn init(x: &mut Cx) {
    x.add8(f::STATE, 2);
    x.set8(BLINK, 0);
    x.set8(0x67, 0);
    x.set8(0x81, 0);
    x.set8(0x7e, 0x27);
    x.set8(0x93, 1);
    x.set8(0x9a, 0xff);
    x.set8(0x80, 0x29);
    x.set_a5_u8(0x50, 1);
    lib::clear_popup(x);
    lib::empty_popups(x);
    lib::clear_all_popups(x);
    lib::create_set_15(x);
    x.set8(f::BOX_SET, 0x25);
    lib::initial_hp_boss(x);
    lib::set_immunities(x);
    x.set16(0x8, 0x120);
    x.set16(0xc, 0x30);
    x.set16(0x10, 0x178);
    active(x);
}

/// Boucle de l'état actif (0x8349c) ; l'état touché la rappelle pour reprendre le combat.
fn active(x: &mut Cx) {
    if x.a5_u8(var::PAUSE) != 0 {
        return;
    }
    if x.u8(BLINK) != 0 && sub8(x, BLINK, 1).gt() {
        x.set8(0x38, 0);
        x.set8(COOL, 0x20);
    }
    if x.u8(f::FREEZE) != 0 {
        sub8(x, f::FREEZE, 1);
        let v = x.u8(0x9) ^ 2;
        x.set8(0x9, v);
        x.set8(BLINK, 0x20);
        return;
    }
    if x.u8(COOL) != 0 {
        sub8(x, COOL, 1);
    } else {
        x.set32(0x58, 0);
    }
    x.tick_counters();
    let s = x.u32(f::STATE);
    x.set32(SAVED_STATE, s);
    match x.u8(f::SUB) {
        0x0 => intro(x),
        0x4 => stroll(x),
        0x8 => dash(x, 0x8369c),
        0xc => seize(x),
        0x10 => simple_attack(x),
        0x14 => dash(x, 0x836fa),
        0x18 => chase(x),
        _ => {}
    }
    let s = x.u32(f::STATE);
    x.set32(SAVED_STATE, s);
    x.reinsert();
}

// --- +5 = 0 : arrivée -------------------------------------------------------------------------

fn intro(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 4);
            x.set16(COUNT, 0x100);
            set_dw(x, 0, 0x100);
            x.call(0x2a42);
            x.call(0x1ab22);
            x.set_a5_u16(0x490, 0x10);
            x.set_a5_u8(0x48e, 0xf);
            set_dw(x, 0, 0x10);
            x.set_a5_u8(0x48f, 8);
            call_on_list(x, 0x14de, 0x9b6);
            intro_wait_scene(x);
        }
        4 => intro_wait_scene(x),
        8 => {
            if step(x) {
                x.add8(f::SUB2, 4);
                body(x, 0);
            }
        }
        0xc => {
            if x.a5_u8(0x2c) != 0xc {
                return;
            }
            lib::create_set_14(x);
            if let Some(set) = x.alloc(&pools::SET) {
                set.set_u8(x.b, 0, 1);
                set.set_u8(x.b, 2, 2);
                set.set_u8(x.b, 3, 0x73);
            }
            x.call(0x1abe0);
            x.set_a5_u8(0x48e, 0xf);
            x.call(0x1686);
            next_action(x);
        }
        _ => {}
    }
}

fn intro_wait_scene(x: &mut Cx) {
    if x.a5_u8(0x2e) != 4 {
        return;
    }
    x.add8(f::SUB2, 4);
    eshl_pair_3f(x);
    body(x, 1);
}

// --- +5 = 4 : marche lente ------------------------------------------------------------------------

fn stroll(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 4);
            let i = level_index(x);
            let n = x.rom16(0x8362e + i);
            x.set16(COUNT, n);
            if x.u8(ANIM_SET) != 0 || x.u8(ANIM_N) != 0 {
                return body(x, 0);
            }
            stroll_step(x);
        }
        4 => stroll_step(x),
        _ => {}
    }
}

fn stroll_step(x: &mut Cx) {
    x.anim_step();
    let n = x.u16(COUNT).wrapping_sub(1);
    x.set16(COUNT, n);
    if n & 0x8000 != 0 {
        next_action(x);
    }
}

// --- +5 = 8 et 0x14 : déplacement ---------------------------------------------------------------

/// `table` : vitesses (mots) par niveau de difficulté.
fn dash(x: &mut Cx, table: u32) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 4);
            let i = level_index(x);
            let n = x.rom16(table + i);
            set_dw(x, 0, n);
            let n = x.c.d[0] as u16;
            x.select_speed(&crate::objects::motion::SPEEDS_B, n);
            if x.u8(f::FLIP) != 0 {
                let v = x.u16(0x14).wrapping_neg();
                x.set16(0x14, v);
            }
            x.c.d[0] = 0;
            if x.u8(ANIM_SET) != 0 {
                return body(x, 0);
            }
            if x.u8(ANIM_N) != 0 {
                body(x, 0);
            }
            dash_step(x);
        }
        4 => dash_step(x),
        _ => {}
    }
}

fn dash_step(x: &mut Cx) {
    x.move_xy();
    x.anim_step();
    if sub8(x, STEPS, 1).mi() {
        next_action(x);
    }
}

// --- +5 = 0xc : saisie --------------------------------------------------------------------------

fn seize(x: &mut Cx) {
    x.set8(IN_ATTACK, 1);
    match x.u8(f::SUB2) {
        0 => grab_begin(x),
        4 => grab_hold(x),
        8 => grab_end(x),
        0xc => {
            // le joueur saisi est celui qu'A1 désigne encore (registre hérité de l'image précédente)
            lib::seize_player(x, Obj(x.c.a[1]), 0);
            grab_begin(x);
        }
        _ => {}
    }
}

fn grab_begin(x: &mut Cx) {
    x.add8(f::SUB2, 4);
    x.set16(COUNT, 0x80);
    x.set8(BITING, 1);
    x.set8(STEPS, 0);
    attack_anim(x, 1);
    grab_hold(x);
}

/// Le joueur saisi (`+0x50`) perd des PV tant que dure la prise.
fn grab_hold(x: &mut Cx) {
    let target = x.u16(f::TARGET);
    if target == 0 {
        return grab_release(x);
    }
    let p = x.linked(f::TARGET);
    x.c.a[1] = p.0;
    if x.u8(BITING) == 0 {
        p.set_u32(x.b, f::STATE, 0x0204_3200);
        p.set_u8(x.b, 0, 2);
        p.set_u16(x.b, 0x186, x.o.0 as u16);
        p.set_u8(x.b, 0x188, 1);
        x.set8(BITING, 1);
        let hp = p.u16(x.b, f::HP);
        p.set_u16(x.b, f::HP, hp.wrapping_sub(0x10));
        if Fl::sub16(hp, 0x10).le() {
            return grab_kill(x);
        }
    }
    if x.u8(f::KEY_FRAME) != 0 {
        x.c.a[1] = p.0;
        lib::sound(x, 0xe425c);
    }
    if p.u8(x.b, 0) == 0 {
        return grab_release(x);
    }
    if sub8(x, STEPS, 1).mi() {
        x.set8(STEPS, 3);
        let hp = p.u16(x.b, f::HP);
        p.set_u16(x.b, f::HP, hp.wrapping_sub(1));
        if Fl::sub16(hp, 1).le() {
            return grab_kill(x);
        }
    }
    lib::hold_seized(x);
    x.anim_step();
    let n = x.u16(COUNT).wrapping_sub(1);
    x.set16(COUNT, n);
    if n & 0x8000 == 0 {
        return;
    }
    let p = x.linked(f::TARGET);
    x.add8(f::SUB2, 4);
    x.set8(BITING, 0);
    p.set_u32(x.b, f::STATE, 0x0204_3400);
    p.set_u8(x.b, 0x188, 0);
    p.set_u8(x.b, 0x6f, 0);
    attack_anim(x, 2);
}

/// Le joueur n'a plus de PV : il est relâché mourant (1 PV).
fn grab_kill(x: &mut Cx) {
    let p = x.linked(f::TARGET);
    p.set_u16(x.b, f::HP, 1);
    p.set_u32(x.b, f::STATE, 0x0204_3400);
    p.set_u8(x.b, 0x188, 0);
    p.set_u8(x.b, 0x6f, 1);
    x.set8(BITING, 0);
    grab_release(x);
}

fn grab_release(x: &mut Cx) {
    attack_anim(x, 2);
    x.add8(f::SUB2, 4);
    grab_end(x);
}

fn grab_end(x: &mut Cx) {
    if step(x) {
        next_action(x);
    }
}

// --- +5 = 0x10 : attaque simple ---------------------------------------------------------------

fn simple_attack(x: &mut Cx) {
    x.set8(IN_ATTACK, 1);
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 4);
            x.set8(FRAME_KIND, 0x4b);
            lib::sound(x, 0xe425c);
            attack_anim(x, 0);
            end_of_script(x);
        }
        4 => end_of_script(x),
        _ => {}
    }
}

fn end_of_script(x: &mut Cx) {
    if step(x) {
        next_action(x);
    }
}

// --- +5 = 0x18 : poursuite --------------------------------------------------------------------

fn chase(x: &mut Cx) {
    x.set8(IN_ATTACK, 1);
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 4);
            x.set8(BLINK, 0x14);
            lib::sound(x, 0xe425c);
            x.select_speed(&crate::objects::motion::SPEEDS_B, 0x13);
            x.set8(Obj::SPEED_INDEX, 0x18);
            if x.u8(f::FLIP) != 0 {
                x.set8(Obj::SPEED_INDEX, 8);
            }
            attack_anim(x, 0x21);
            chase_turn(x);
        }
        4 => chase_turn(x),
        8 => chase_follow(x),
        0xc => end_of_script(x),
        _ => {}
    }
}

fn chase_turn(x: &mut Cx) {
    if !step(x) {
        return;
    }
    x.add8(f::SUB2, 4);
    let n = x.u8(STEPS);
    x.set8(COUNT, n);
    let r = random(x) as u8 & 2;
    x.set8(STEPS, r);
    attack_anim(x, 0x22);
    chase_follow(x);
}

/// Tourne d'un cran vers la cible, avance, saisit un joueur à portée.
fn chase_follow(x: &mut Cx) {
    x.anim_step();
    let criterion = x.u8(STEPS);
    if !lib::target_by_criterion(x, criterion) {
        return chase_stop(x);
    }
    x.call(call::ANGLE_TO_TARGET);
    let mine = x.u8(Obj::SPEED_INDEX);
    let diff = (x.c.d[0] as u8).wrapping_sub(mine) & 0x1f;
    if diff != 0 {
        let mut a = mine;
        if diff >= 0x10 {
            a = a.wrapping_sub(2);
        }
        a = a.wrapping_add(1) & 0x1f;
        x.set8(Obj::SPEED_INDEX, a);
    }
    let a = x.u8(Obj::SPEED_INDEX);
    if a > 0x10 {
        x.set8(f::FLIP, 0);
    } else if a != 0x10 && a != 0 {
        x.set8(f::FLIP, 0x20);
    }
    x.move_by_table_xy();
    if let Some(player) = lib::attack_hits_a_player(x, 8) {
        lib::seize_player(x, player, 0);
        return grab_begin(x);
    }
    let n = x.u8(COUNT).wrapping_sub(1);
    x.set8(COUNT, n);
    if n & 0x80 == 0 {
        return;
    }
    chase_stop(x);
}

fn chase_stop(x: &mut Cx) {
    x.add8(f::SUB2, 4);
    attack_anim(x, 2);
    end_of_script(x);
}

// --- choix de l'action suivante (0x83e3e) ----------------------------------------------------------

/// Groupe `+5` et angle d'après la position de la cible : lignes de la table 0x83f4e (2 octets par entrée :
/// angle et groupe, durée et demi-tour).
fn next_action(x: &mut Cx) {
    x.set8(BITING, 0);
    x.set8(IN_ATTACK, 0);
    if !lib::target_by_criterion(x, lib::NEAREST) {
        x.set8(f::SUB, 4);
        x.set16(f::SUB2, 0);
        x.set8(STEPS, 0x10);
        return;
    }
    let target = Obj(x.c.a[1]);
    let mut row: u32 = 0;
    let dy = target.u16(x.b, 0x10).wrapping_sub(x.u16(0x10)) as i16;
    row += if dy >= 0x14 {
        0
    } else if dy < -0x14 {
        0x40
    } else if dy >= 4 {
        0x10
    } else if dy < -4 {
        0x30
    } else {
        0x20
    };
    let mut dx = x.u16(0x8).wrapping_sub(target.u16(x.b, 0x8)) as i16;
    if x.u8(f::FLIP) != 0 {
        dx = dx.wrapping_neg();
    }
    row += if dx >= 0x80 {
        0xa0
    } else if dx < -0x80 {
        0x190
    } else if dx >= 0x40 {
        0x50
    } else if dx < -0x40 {
        0x140
    } else if dx >= 0 {
        0
    } else {
        0xf0
    };
    row += random(x) & 0xe;
    let a = x.rom8(0x83f4e + row);
    x.set8(Obj::SPEED_INDEX, (a << 1) & 0x1e);
    x.set8(f::SUB, (a & 0xf0) >> 2);
    x.set16(f::SUB2, 0);
    let b = x.rom8(0x83f4f + row);
    x.set8(STEPS, b);
    if b & 0x80 != 0 {
        let v = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, v);
    }
    x.set8(STEPS, b & 0x7f);
}

// --- état 4 : touché --------------------------------------------------------------------------------

fn hit(x: &mut Cx) {
    let freeze = x.u8(f::FREEZE) as i8;
    if freeze > 0 {
        x.set8(f::FREEZE, (freeze - 1) as u8);
    } else {
        x.set8(BITING, 0);
        x.set8(IN_ATTACK, 0);
        match x.u8(f::SUB) {
            0 | 2 | 4 => reaction(x),
            6 => dying(x),
            _ => {}
        }
    }
    lib::keep_in_zone(x);
    lib::collide_terrain(x);
}

fn reaction(x: &mut Cx) {
    if x.u8(REACTION) == 0xa8 {
        return tumble(x);
    }
    if x.u16(f::HP) == 0 {
        x.set32(f::STATE, 0x0406_0000);
        return;
    }
    if x.u8(REACTION) == 0x38 {
        // coup sans effet : retour à l'état mémorisé
        let s = x.u32(SAVED_STATE);
        x.set32(f::STATE, s);
        let stun = x.u8(STUN);
        x.set8(f::FREEZE, stun);
        x.set8(0, 1);
        return active(x);
    }
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 4);
            let stun = x.u8(STUN);
            x.set8(f::FREEZE, stun);
            fall_anim(x, 0);
            recover(x);
        }
        4 => recover(x),
        _ => {}
    }
}

/// Attend la fin du recul, puis reprend le combat (ou saute à la poursuite).
fn recover(x: &mut Cx) {
    if x.u8(f::FREEZE) != 0 {
        sub8(x, f::FREEZE, 1);
        return;
    }
    if !step(x) {
        return;
    }
    x.set8(0, 1);
    x.set8(BLINK, 0x10);
    x.set8(f::STATE, 2);
    if random(x) & 1 != 0 {
        return next_action(x);
    }
    x.set8(STEPS, 0x40);
    x.set32(f::STATE, 0x0218_0000);
}

/// `+0x49` = 0xa8 : le coup projette le boss, qui retombe en tourbillon (`+6` = 0 à 0x10).
fn tumble(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(FALL_TIMER, 0x3c);
            take_frame_request(x);
            x.c.d[0] = 0;
            x.set32(0x14, 0);
            x.set32(0x18, 0);
            x.set32(0x1c, 0);
            x.set8(SPIN_DELAY, if x.c.d[7] & 1 != 0 { 1 } else { 2 });
            let a = (random(x) as u16 + 8) & 0x1f;
            x.set8(Obj::SPEED_INDEX, a as u8);
            fall_anim(x, 0);
        }
        2 => {
            if sub8(x, FALL_TIMER, 1).ne() {
                spin(x);
            } else {
                x.add8(f::SUB2, 2);
            }
        }
        4 => {
            x.set8(0x67, 0);
            x.add8(f::SUB2, 2);
            x.set8(FALL_TIMER, 0x1e);
            x.set8(f::STATE + 3, 0);
            x.set32(0x14, 0);
            x.set16(0x18, 0x600);
            x.set16(0x1a, (-0x80i16) as u16);
            fall_anim(x, 0);
            lib::sound(x, 0xebbda);
            lib::attack_effect(x, 8, 0);
        }
        6 | 0xa => {
            x.anim_step();
            if x.u8(f::STATE + 3) == 0 {
                x.move_xz();
            }
            if sub8(x, FALL_TIMER, 1).ne() {
                if (x.u16(0xc) as i16) > 0 {
                    return;
                }
                x.set32(0xc, 0);
                x.add8(f::STATE + 3, 2);
            } else {
                x.add8(f::SUB2, 2);
                x.set8(f::STATE + 3, 0);
            }
        }
        8 => {
            x.add8(f::SUB2, 2);
            x.set8(FALL_TIMER, 0x3c);
            x.set32(0x14, 0);
            x.set32(0x1c, 0);
            x.set16(0x18, 0x800);
            x.set16(0x1a, (-0x40i16) as u16);
            fall_anim(x, 0);
            lib::attack_effect(x, 7, 0);
        }
        0xc => {
            x.add8(f::SUB2, 2);
            take_frame_request(x);
            x.set8(FALL_TIMER, 0x12);
            x.set16(0x14, 0x1000);
            x.set16(0x16, 0);
            x.set32(0x18, 0x0100_0000);
            x.set16(0x1a, 0);
            fall_anim(x, 0);
            if x.u8(f::FLIP) != 0 {
                let v = x.u16(0x14).wrapping_neg();
                x.set16(0x14, v);
            }
            lib::attack_effect(x, 2, 0);
        }
        0xe => {}
        0x10 => {
            x.set16(f::HP, 0);
            x.set8(f::SUB, 6);
            x.set16(f::SUB2, 0);
        }
        _ => {}
    }
}

/// Pas du tourbillon (0x83da8) : déplacement par la table de vecteurs de l'angle `+0x2d`.
fn spin(x: &mut Cx) {
    x.c.a[4] = SPIN_VECTORS;
    x.add32(0xc, 0x20000);
    if (x.u16(0xc) as i16) >= 0x40 {
        x.set32(0xc, 0x0040_0000);
    }
    if sub8(x, SPIN_DELAY, 1).eq() {
        x.set8(SPIN_DELAY, 1);
        let a = x.u8(Obj::SPEED_INDEX).wrapping_add(2) & 0x1f;
        x.set8(Obj::SPEED_INDEX, a);
        x.set8(f::FLIP, 0);
        if a.wrapping_sub(8) >= 0x10 {
            x.set8(f::FLIP, 0x20);
        }
    }
    let i = (x.u8(Obj::SPEED_INDEX) as u32) << 2;
    let vx = x.data16(SPIN_VECTORS + i).wrapping_add(x.u16(0x16));
    x.set16(0x14, vx);
    let dx = ((vx as u32) << 16) as i32 >> 8;
    x.add32(0x8, dx as u32);
    let vy = x.data16(SPIN_VECTORS + i + 2).wrapping_add(x.u16(0x1e));
    x.set16(0x1c, vy);
    let dy = ((vy as u32) << 16) as i32 >> 8;
    x.add32(0x10, dy as u32);
}

/// État 6 de `+5` : la mort (séquence en deux temps puis libération).
fn dying(x: &mut Cx) {
    x.set8(IN_ATTACK, 2);
    match x.u8(f::SUB2) {
        0 => {
            addq8(x, f::SUB2, 2);
            x.set16(COUNT, 0);
            x.set_a5_u8(0x50, 0);
            x.set_a5_u8(0x42, 1);
            x.call(0x1ac86);
            lib::drop_rewards(x);
            x.call(0xe0a10);
            lib::sound(x, 0xe3b6c);
            let kind = x.u8(0x2);
            set_abs8(x, 0xff2a84, kind);
            set_abs8(x, 0xff2a85, 0);
            let bar = x.u8(0x7e);
            set_abs8(x, 0xff2a86, bar);
            lib::flash_and_shake(x);
            lib::object_event(x);
            fall_anim(x, 4);
            dying_burst(x);
        }
        2 => dying_burst(x),
        _ => {}
    }
}

fn dying_burst(x: &mut Cx) {
    let n = x.u16(COUNT).wrapping_sub(1);
    x.set16(COUNT, n);
    if n & 0x8000 != 0 {
        x.set_a5_u16(0x4be, 5);
        let r = (random(x) & 0x1f) as u16 + 0x10;
        x.set16(COUNT, r);
    }
    if step(x) {
        x.free(&pools::BOSS);
    }
}

handlers! {
    0x8341c "boss_5" run,
}
