//! Boss 16, état 4 « touché » (0x94050). Groupes `+5` :
//!
//! | `+5` | Rôle |
//! |---|---|
//! | 0 | réaction au coup : routine propre choisie par le code `+0x49` (table 0x940a8) |
//! | 2 | recul en glissant |
//! | 4 | à terre, relevé |
//! | 6 | mort (deux séquences selon le code du coup) |
//! | 8 | rien |
//! | 10 | retour au combat |

use super::*;
use crate::objects::Courant;

/// Table des routines de réaction, indexée par le code de coup `+0x49` (longs).
const REACTIONS: u32 = 0x940a8;
/// Routines de réaction propres au boss.
const KNOCKBACK: u32 = 0x9416e;
const KNOCKBACK_HARD: u32 = 0x941f8;
const BURST: u32 = 0x942da;
const KILL: u32 = 0x94154;
/// Table des séquences de mort, indexée par le code de coup.
const DEATHS: u32 = 0x94644;
const DEATH_HEAVY: u32 = 0x947c6;

pub(super) fn run(x: &mut Cx) {
    x.set_bit(0x1, 1, false);
    if tst8(x, f::FREEZE).gt() {
        sub8(x, f::FREEZE, 1);
    } else {
        match x.u8(f::SUB) {
            0 => reaction(x),
            2 => pushed_back(x),
            4 => down(x),
            6 => death(x),
            10 => next_attack(x),
            _ => {}
        }
    }
    x.c.d[0] = 0x50;
    x.call(call::IN_ARENA);
    x.call(SHADOW);
    x.reinsert();
}

/// Mort immédiate : plus de PV, séquence de mort dans la même image.
fn die(x: &mut Cx) {
    x.set8(f::SUB, 6);
    x.set16(f::SUB2, 0);
    death_normal(x);
}

/// 0x94092 : premier passage : dégâts (0xb648) ; puis la routine du code de coup.
fn reaction(x: &mut Cx) {
    if x.u8(f::SUB2) == 0 {
        x.call(0xb648);
    }
    let code = x.u8(0x49) as u32;
    x.c.d[0] = code;
    match rom32(x, REACTIONS + code) {
        KNOCKBACK => knockback(x, false),
        KNOCKBACK_HARD => knockback(x, true),
        BURST => burst(x),
        KILL => {
            if x.a5_u8(var::PAUSE) == 0 {
                x.set16(f::HP, 0);
                die(x);
            }
        }
        other => panic!("boss 16 : réaction inconnue {other:#x}"),
    }
}

/// 0x9453a : compte les coups encaissés d'affilée (`+0xa5`, 5 au départ) ; au dernier, le boss contre-attaque
/// (état 0x0204_0c00).
fn counter(x: &mut Cx) -> bool {
    if sub8(x, HIT_BUDGET, 1).gt() {
        return false;
    }
    x.set32(f::STATE, 0x0204_0c00);
    x.set8(TIMER, 6);
    x.set8(0x38, 0);
    x.set32(0x58, 0);
    x.set8(0x49, 0);
    true
}

/// Fin des reculs : le boss se relève à l'écart des bords (attaque 0xe pour revenir vers l'écran) ou marche.
fn recover_position(x: &mut Cx) {
    x.set8(0, 1);
    x.set8(0x49, 0);
    x.set32(0x58, 0);
    let mut edge = x.a5_u16(var::SCROLL_X);
    if x.a5_u8(0xa) != 0 {
        edge = x.a5_u16(0x556);
    }
    let mut side = 0x20u8;
    let left = edge.wrapping_add(0x30);
    if Fl::sub16(left, x.u16(0x8)).ge() {
        // près du bord gauche : regarde à droite
    } else {
        side = 0;
        let right = edge.wrapping_add(0x30).wrapping_add(0x120);
        if !Fl::sub16(right, x.u16(0x8)).le() {
            return x.set32(f::STATE, state::WALK);
        }
    }
    x.set8(TIMER, 8);
    x.set8(f::FLIP, side);
    x.set32(f::STATE, 0x0204_0e00);
}

/// 0x9416e et 0x941f8 : projeté en l'air (`hard`) ou repoussé au sol ; à la fin le boss se relève.
fn knockback(x: &mut Cx, hard: bool) {
    if x.u8(f::SUB2) == 0 {
        return knockback_start(x, hard);
    }
    if hard {
        return knockback_hard_hold(x);
    }
    knockback_hold(x);
}

/// 0x94174 et 0x9421e... : démarrage (PV encore positifs, sinon mort).
fn knockback_start(x: &mut Cx, hard: bool) {
    if !tst16(x, f::HP).gt() {
        return die(x);
    }
    next6(x);
    let freeze = x.u8(0x56);
    x.set8(f::FREEZE, freeze);
    x.set8(SHAKE, 0x10);
    stop(x);
    if hard {
        let v = if x.u8(f::FLIP) == 0 { 0x600u16 } else { (-0x600i16) as u16 };
        x.set16(0x14, v);
        x.set16(0x16, 0);
        x.set16(0x18, 0x400);
        x.set16(0x1a, (-0x80i16) as u16);
    } else {
        let (vx, vz) = if x.u8(f::FLIP) == 0 { (0x400u16, (-0x48i16) as u16) } else { ((-0x400i16) as u16, 0x48) };
        x.set16(0x14, vx);
        x.set16(0x16, vz);
    }
    if counter(x) {
        return;
    }
    x.c.d[0] = 0;
    anim_c(x, 0);
}

/// 0x941c4 : tremblement puis glissade.
fn knockback_hold(x: &mut Cx) {
    if x.u8(SHAKE) != 0 {
        let n = x.u8(SHAKE).wrapping_sub(1);
        x.set8(SHAKE, n);
        let d: u16 = if n & 1 != 0 { 0xfffe } else { 2 };
        set_dw(x, 0, d);
        x.add16(0x8, d);
        return;
    }
    if x.u8(0x14) == 0 {
        return recover_position(x);
    }
    x.move_x();
    step(x);
}

/// 0x94254 : projeté : retombée sur le sol.
fn knockback_hard_hold(x: &mut Cx) {
    if x.u8(SHAKE) != 0 {
        sub8(x, SHAKE, 1);
        x.set8(SHAKE, 0x10);
    }
    x.move_xz();
    if (x.u16(0xc) as i16) > 0 {
        step(x);
        return;
    }
    x.set16(0xc, 0);
    recover_position(x);
}

/// 0x942da : coups spéciaux (codes 0x60, 0x70, 0x78, 0x90, 0x94) : gerbes d'effets, puis le recul.
fn burst(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            next6(x);
            x.set8(0, 2);
            let (kind, flag) = match x.u8(0x49) {
                0x60 => (7, 0),
                0x70 | 0x90 => (8, 0),
                0x78 => (1, 0),
                0x94 => (9, 0),
                _ => (0xff, 0),
            };
            if kind != 0xff {
                x.c.d[0] = kind;
                x.c.d[1] = flag;
                x.call(call::EFFECT);
            }
            let r = rand_and(x, 3);
            let t = x.rom8(0x94368 + r);
            x.set8(SHAKE, t);
            x.c.d[0] = 0;
            anim_c(x, 0);
            if !tst16(x, f::HP).gt() {
                die(x);
            }
        }
        2 => {
            if sub8(x, SHAKE, 1).le() {
                if x.u8(0x49) != 0x60 {
                    let r = rand_and(x, 3);
                    if r != 0 {
                        x.c.d[0] = 1;
                        x.c.d[1] = 1;
                        x.call(call::EFFECT);
                    }
                }
                let r = rand_and(x, 3);
                let t = x.rom8(0x94368 + r);
                x.set8(SHAKE, t);
            }
            let mut d = 1u16;
            if x.u8(SHAKE) & 1 != 0 {
                d = d.wrapping_neg();
            }
            if x.u8(f::FLIP) != 0 {
                d = d.wrapping_neg();
            }
            set_dw(x, 0, d);
            x.add16(0x8, d);
            if x.a5_u8(var::PAUSE) == 0 {
                next6(x);
            }
        }
        4 => knockback_start(x, false),
        6 => knockback_hold(x),
        _ => {}
    }
}

/// 0x94564 : recul en glissant, puis la suite par `next_attack`.
fn pushed_back(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            next6(x);
            x.set8(0x68, 8);
            x.set8(0x69, 8);
            let (vx, vz) = if x.u8(f::FLIP) == 0 { (0x300u16, (-0x28i16) as u16) } else { ((-0x300i16) as u16, 0x28) };
            x.set16(0x14, vx);
            x.set16(0x16, vz);
            anim_a(x, 5);
        }
        2 => {
            x.move_x();
            if (x.u8(0x14) as i8) < 0 {
                return stop(x);
            }
            if step(x) {
                x.set32(0x58, 0);
                next_attack(x);
            }
        }
        _ => {}
    }
}

/// 0x945c8 : à terre (`+0x84` images hors pause), puis relevé et contre-attaque.
fn down(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            next6(x);
            anim_a(x, 0x11);
        }
        2 => {
            if x.a5_u8(var::PAUSE) != 0 || sub16(x, DOWN_TIME, 1).gt() {
                return;
            }
            x.set16(DOWN_TIME, 0);
            next6(x);
            x.set8(0x86, 0);
            x.set8(0x49, 0);
        }
        4 => {
            if x.a5_u8(var::PAUSE) != 0 || !step(x) {
                return;
            }
            x.set8(VULNERABLE, 1);
            x.set8(0, 1);
            x.set8(TIMER, 6);
            x.set8(0x38, 0);
            x.set32(f::STATE, state::WALK);
            x.c.d[0] = 0;
            anim_a(x, 0);
        }
        _ => {}
    }
}

/// 0x94638 : mort ; la séquence dépend du code du coup.
fn death(x: &mut Cx) {
    let code = x.u8(0x49) as u32;
    x.c.d[0] = code;
    if rom32(x, DEATHS + code) == DEATH_HEAVY {
        death_heavy(x);
    } else {
        death_normal(x);
    }
}

/// Prévient la scène : boss vaincu (type 0x10, variante 0), score `+0x7e` ; effets de mort.
fn announce(x: &mut Cx) {
    x.set_a5_u8(0x42, 1);
    x.set_a5_u8(0x50, 0);
    let score = x.u8(0x7e);
    set_abs8(x, A5_DEATH, 0x10);
    set_abs8(x, A5_DEATH + 1, 0);
    set_abs8(x, A5_DEATH + 2, score);
    for r in [0xb780, 0xbad6, 0xe2f6, 0x103ba, 0xe0a10, 0xe3b6c] {
        x.call(r);
    }
}

/// 0x946fc : séquence de mort ordinaire (animations 0xb, 0xc).
fn death_normal(x: &mut Cx) {
    death_sequence(x, 0xb, 0xc);
}

/// 0x947c6 : séquence de mort des coups lourds (animations 3, 4).
fn death_heavy(x: &mut Cx) {
    death_sequence(x, 3, 4);
}

/// Projection (`first`), retombée (`second`) puis délai de 0x28 images.
fn death_sequence(x: &mut Cx, first: u32, second: u32) {
    match x.u8(f::SUB2) {
        0 => {
            next6(x);
            x.set8(0, 2);
            stop(x);
            x.set16(0x18, 0x400);
            x.set16(0x1a, (-0x40i16) as u16);
            let v = if x.u8(f::FLIP) == 0 { 0x200u16 } else { (-0x200i16) as u16 };
            set_dw(x, 0, v);
            x.set16(0x14, v);
            anim_a(x, first);
            announce(x);
        }
        2 => {
            x.move_xz();
            if (x.u16(0xc) as i16) >= 0 {
                return;
            }
            x.set16(0xc, 0);
            next6(x);
            anim_a(x, second);
            x.call(0xeca52);
        }
        4 => {
            if !step(x) {
                return;
            }
            next6(x);
            x.set8(SHAKE, 0x28);
            death_wait(x);
        }
        6 => death_wait(x),
        _ => {}
    }
}

/// 0x947b8 : attend la fin du délai.
fn death_wait(x: &mut Cx) {
    if sub8(x, SHAKE, 1).pl() {
        return;
    }
    next6(x);
}
