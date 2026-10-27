//! Boss 22, état 4 « touché » (0x9cdc6). Groupes `+5` :
//!
//! | `+5` | Rôle |
//! |---|---|
//! | 0 | réaction au coup : routine commune choisie par le code `+0x49` (table 0x9ce66) |
//! | 2 | recul en glissant |
//! | 4 | à terre, relevé |
//! | 6 | mort : bond, retombée, passage de relais à l'hôte |
//! | 8, 10 | retour au combat |

use super::*;
use crate::objects::Courant;


pub(super) fn run(x: &mut Cx) {
    if x.u8(HIT_TICK) == 0 {
        x.set_a5_u16(0x51e, 0);
        x.set_a5_u8(0x43, 0xff);
        if two(x) {
            x.set8(VULNERABLE, 0);
        }
        x.set8(HIT_TICK, 1);
    }
    x.set8(0xaf, 0);
    if tst8(x, f::FREEZE).gt() {
        sub8(x, f::FREEZE, 1);
    } else {
        match x.u8(f::SUB) {
            0 => reaction(x),
            2 => pushed_back(x),
            4 => down(x),
            6 => death(x),
            8 | 10 => x.set32(f::STATE, state::STAND),
            _ => {}
        }
    }
    if x.u8(f::SUB) != 6 {
        lib::keep_on_screen(x, 0x90);
        x.call(SHADOW);
    }
    x.reinsert();
}

/// 0x9ce28 : dégâts (0x10112) ; sans PV la mort, sinon la routine commune du code de coup.
fn reaction(x: &mut Cx) {
    if !second_form(x) {
        x.set8(SECOND_FX, 0);
    }
    lib::set_68(x);
    x.set8(ARM_TIMER, 0);
    if !tst16(x, f::HP).gt() {
        x.set8(f::SUB, 6);
        x.set8(0, 2);
        return death(x);
    }
    x.add16(GUARD, 0xffff);
    x.set8(ANIM_LAST, 0x7f);
    let table = kit(x).reactions;
    react(x, table);
}

/// Une poussière (0xb10c8) sans le compteur de huit : le type 2 en lâche une quand il est repoussé vers la droite.
fn rubble_once(x: &mut Cx) {
    x.c.d[0] = 0;
    set_dw(x, 1, 0x201);
    set_dw(x, 2, 0x20);
    set_dw(x, 3, 0x20);
    x.call(call::DUST);
}

/// 0x9cf12 : recul en glissant, puis retour au combat ou riposte.
fn pushed_back(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            next6(x);
            x.set8(ANIM_ID, 0x1e);
            x.set8(ANIM_LAST, 0x1e);
            lib::set_68(x);
            lib::sound(x, 0xec6ca);
            let side = x.u8(0x4a);
            x.set8(f::FLIP, side);
            let side = x.u8(f::FLIP) ^ 0x20;
            x.set8(f::FLIP, side);
            x.set16(0x14, 0x400);
            x.set16(0x16, (-0x28i16) as u16);
            if x.u8(f::FLIP) != 0 {
                if two(x) {
                    rubble_once(x);
                }
                x.set16(0x14, 0xfc00);
                x.set16(0x16, 0x28);
            }
            let n = if second_form(x) { 0x12 } else { 0xe };
            start(x, Set::A, n);
        }
        2 => {
            // `andi.w #3,(0x6e,A5)` réduit aussi le compteur de la RAM de travail
            let tick = x.a5_u16(0x6e) & 3;
            x.set_a5_u16(0x6e, tick);
            if tick == 0 {
                x.c.d[0] = 1;
                set_dw(x, 1, 2);
                set_dw(x, 2, 0x30);
                set_dw(x, 3, 0xffff);
                x.call(call::DUST);
            }
            if x.u8(0x14) != 0 {
                x.move_x();
                return;
            }
            if !x.anim_step() {
                return;
            }
            x.set32(0x58, 0);
            x.set32(0x14, 0);
            if second_form(x) {
                x.set32(f::STATE, 0x0204_1004);
                x.set8(ANIM_LAST, 0x7f);
                return;
            }
            x.set8(SECOND_FX, 0);
            let alive = x.a5_u8(var::PLAYERS_ALIVE) as u32;
            let k = kit(x);
            let chances = rom32(x, k.chances + (alive << k.chances_shift));
            x.c.d[5] = chances;
            let r = rand_and(x, 0x1f);
            if chances >> r & 1 != 0 {
                let fuse = if two(x) { 0x50 } else { 0x46 };
                x.set8(ARM_TIMER, fuse);
                return x.set32(f::STATE, 0x0204_1600);
            }
            x.set8(0, 1);
            x.set32(f::STATE, state::STAND);
        }
        _ => {}
    }
}

/// 0x9d026 : à terre (`+0x84` images hors pause), puis relevé.
fn down(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            next6(x);
            lib::behaviour_by_kind(x);
            let n = if second_form(x) { 0x10 } else { 0xd };
            start(x, Set::A, n);
        }
        2 => {
            if x.a5_u8(var::PAUSE) != 0 || sub16(x, 0x84, 1).gt() {
                return;
            }
            next6(x);
            x.set8(0x86, 0);
            x.set16(0x84, 0);
            x.set8(0x49, 0);
        }
        4 => {
            if x.a5_u8(var::PAUSE) != 0 || !x.anim_step() {
                return;
            }
            x.set8(0, 1);
            x.set8(VULNERABLE, 1);
            if second_form(x) {
                x.set32(f::STATE, 0x0204_1004);
                x.set8(ANIM_LAST, 0x7f);
                return;
            }
            if two(x) {
                x.set8(SECOND_FX, 0);
            }
            x.set32(f::STATE, state::STAND);
            x.set8(ANIM_ID, 0);
            x.set8(ANIM_LAST, 0);
            x.c.d[0] = 0;
            start(x, Set::A, 0);
        }
        _ => {}
    }
}

/// 0x9d0d6 : mort. Si l'hôte a rendu l'âme avant ou en même temps, seule une chute (0x19ad6) ; sinon la scène est
/// prévenue : boss vaincu (type 2).
fn death(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            lib::sound(x, 0xe3b6c);
            x.set32(f::IMMUNITY, 0xffff_ffff);
            x.set8(0, 2);
            let freeze = x.u8(0x56);
            x.set8(f::FREEZE, freeze);
            let host = x.linked(HOST);
            let mut last = true;
            if !two(x) || x.u8(0xac) != 0 {
                if host.u32(x.b, f::STATE) == x.u32(f::STATE) {
                    x.set8(0x7f, 1);
                    last = false;
                } else if host.u8(x.b, 0x7f) == 0 && host.u16(x.b, f::STATE) != 0x406 {
                    last = false;
                }
            }
            if last {
                x.set_a5_u8(0x42, 1);
                x.set_a5_u8(0x50, 0);
                let score = x.u8(0x7e);
                set_abs8(x, A5_DEATH, 2);
                set_abs8(x, A5_DEATH + 1, 0);
                set_abs8(x, A5_DEATH + 2, score);
                x.set8(0x93, 1);
                x.set_a5_u8(0x4f, 0xff);
                for r in [0xe0a10, 0xe3b6c, 0x103ba] {
                    x.call(r);
                }
            } else {
                x.call(0x19ad6);
            }
            for r in [0xb780, 0xbad6, 0xe2f6] {
                x.call(r);
            }
            x.set16(0x16, 0);
            x.set16(0x18, 0x600);
            x.set16(0x1a, (-0x38i16) as u16);
            // l'original charge 0x180 dans D0 puis l'écrase par la position de la caméra : la vitesse est cette position + 0xc0
            let center = x.a5_u16(var::SCROLL_X).wrapping_add(0xc0);
            let (vx, side) = if Fl::sub16(center, x.u16(0x8)).ge() { (center, 0u8) } else { (center.wrapping_neg(), 0x20) };
            x.c.d[0] = vx as u32;
            x.set16(0x14, vx);
            x.set8(f::FLIP, side);
            x.move_xz();
            next6(x);
            let n = if second_form(x) { 0x17 } else { 0x16 };
            start(x, Set::A, n);
        }
        2 => {
            x.move_xz();
            if (x.u16(0xc) as i16) > 0 {
                return;
            }
            next6(x);
            x.set16(0xc, 0);
            let right = x.u8(f::FLIP) != 0;
            dust_trail(x, right);
            lib::sound(x, 0xec9e8);
            if !second_form(x) {
                effct_33(x);
            }
            start(x, Set::A, 0x18);
        }
        4 => {
            if x.anim_step() {
                next6(x);
            }
        }
        _ => {}
    }
}
