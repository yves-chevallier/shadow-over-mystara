//! Boss 14 / 19, groupe `+5` = 4 : les attaques (`+6` = 0 à 10), chacune en étapes `+7`.
//!
//! | `+6` | Attaque |
//! |---|---|
//! | 0 | bond en arrière puis retombée, demi-tour |
//! | 2 | charge en avant (tant que la marque `+0xa3` n'est pas levée) |
//! | 4 | souffle : suit un joueur (`aim`) tant que l'animation joue |
//! | 6 | saut périlleux en arrière |
//! | 8 | saut en avant puis bond |
//! | 10 | grand saut : monte, passe de l'autre côté de l'écran, retombe |
//!
//! Elles finissent par `fight::pick_attack_b`. Plusieurs fuient d'une poussière (effet 0xb10c8).

use super::fight;
use super::*;
use crate::objects::Obj;
use crate::objects::Courant;

/// Table des joueurs (FF85DE, 0x200 octets chacun).
const PLAYERS: u32 = 0xff85de;
/// Choix de la cible du souffle (octets, par tirage) puis routine (mots relatifs à 0x90494).
const AIM_PICK: u32 = 0x9049a;
const AIM_ROUTINES: u32 = 0x90494;
/// Décalages (longs) des places de joueurs parcourues dans un ordre tiré au sort.
const PLAYER_ORDERS: u32 = 0x90556;

pub(super) fn run(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => back_hop(x),
        2 => charge(x),
        4 => breath(x),
        6 => backflip(x),
        8 => hop(x),
        10 => big_jump(x),
        _ => {}
    }
    if x.a5_u8(var::PLAYERS_ALIVE) == 0 {
        wait(x);
    }
}

// --- aides --------------------------------------------------------------------------------------

/// Poussière sous le dragon (0xb10c8) : `kind` 0x100 ou 0x101.
fn dust(x: &mut Cx, kind: u16) {
    x.c.d[0] = 0;
    set_dw(x, 1, kind);
    set_dw(x, 2, 0);
    let z = x.u16(0xc).wrapping_neg();
    set_dw(x, 3, z);
    x.call(call::EFFECT);
}

/// Mot lu en A0 − 0x7f92 : l'original voulait le compteur d'images (`A5 + 0x6e`) mais a écrit un déplacement
/// relatif à A0 ; on lit la même adresse.
fn tick_word(x: &mut Cx) -> u16 {
    let addr = x.o.0.wrapping_sub(0x7f92) & 0xff_ffff;
    x.data16(addr)
}

/// Vitesses `(vx, vz)` selon le sens : `(a, b)` si le dragon regarde à droite (`+0x2c` nul inversé).
fn signed_by_facing(x: &mut Cx, a: i16, b: i16) -> (u16, u16) {
    if x.u8(f::FLIP) != 0 {
        (a as u16, b as u16)
    } else {
        (a.wrapping_neg() as u16, b.wrapping_neg() as u16)
    }
}

/// Touche le sol : l'altitude est ramenée à 0, la vitesse verticale annulée et la vitesse horizontale divisée par deux
/// (`grow` : doublée dans le grand saut).
fn bounce(x: &mut Cx, grow: bool) {
    if (x.u16(0xc) as i16) >= 0 {
        return;
    }
    x.set16(0xc, 0);
    x.set32(0x18, 0);
    let v = x.u16(0x14) as i16;
    x.set16(0x14, if grow { v.wrapping_shl(1) } else { v >> 1 } as u16);
    x.set16(0x16, 0);
}

fn jump_back_and_forth(x: &mut Cx, vx: i16, vz: i16) {
    let (a, b) = signed_by_facing(x, vx, vz);
    x.set16(0x14, a);
    x.set16(0x16, b);
}

// --- +6 = 0 : bond en arrière -------------------------------------------------------------------

fn back_hop(x: &mut Cx) {
    x.set8(FRAME_KIND, 0x47);
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            jump_back_and_forth(x, 0x100, 0x40);
            x.set16(0x18, (-0x100i16) as u16);
            x.set16(0x1a, 0);
            let side = x.u8(f::FLIP) ^ 0x20;
            x.set8(f::FLIP, side);
            set_anim(x, 2, 4);
        }
        2 => {
            x.move_xz();
            bounce(x, false);
            if !step(x) {
                return;
            }
            x.add8(0x7, 2);
            x.set32(0x14, 0);
            x.set16(0x18, 0x100);
            x.set16(0x1a, 0);
            let side = x.u8(f::FLIP) ^ 0x20;
            x.set8(f::FLIP, side);
            set_anim(x, 2, 5);
        }
        4 => {
            if tick_word(x) & 3 == 0 {
                dust(x, 0x101);
            }
            x.move_xz();
            if (x.u16(0xc) as i16) > 0x28 {
                x.set32(0x18, 0);
            }
            if step(x) {
                fight::pick_attack_b(x);
            }
        }
        _ => {}
    }
}

/// Octet de l'animation (`+0x44`) lu par les scripts.
const FRAME_KIND: u32 = 0x44;

// --- +6 = 2 : charge ----------------------------------------------------------------------------

fn charge(x: &mut Cx) {
    x.set8(HOVER, 1);
    x.set8(HURT_BOX, 1);
    x.set8(FRAME_KIND, 0x48);
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            x.set8(MARK, 0);
            let (a, _) = signed_by_facing(x, 0x200, 0);
            x.set16(0x14, a);
            x.set16(0x16, 0);
            x.set32(0x18, 0);
            x.set32(0x1c, 0);
        }
        2 => {
            x.move_x();
            if x.u8(MARK) != 0 {
                return fight::pick_attack_b(x);
            }
            x.anim_step();
            if tick_word(x) & 3 == 0 {
                dust(x, 0x101);
            }
        }
        _ => {}
    }
}

// --- +6 = 4 : souffle ---------------------------------------------------------------------------

fn breath(x: &mut Cx) {
    let boxes = if x.u8(TYPE) == TYPE_GREEN { 2 } else { 4 };
    x.set8(HURT_BOX, boxes);
    x.set8(HOVER, 1);
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            x.set8(MARK, 0);
            x.set8(TIMER, 0x10);
            speed(x, 0x11);
            breath_step(x);
        }
        2 => breath_step(x),
        _ => {}
    }
}

fn breath_step(x: &mut Cx) {
    if x.u8(MARK) != 0 {
        return fight::pick_attack_b(x);
    }
    aim_choice(x);
    x.anim_step();
}

/// Le joueur `a2` est-il utilisable (0x9059e) : place occupée et `+0x182` nul.
fn usable(x: &mut Cx, a2: Obj) -> bool {
    a2.u8(x.b, 0) != 0 && a2.u8(x.b, 0x182) == 0
}

/// 0x90458 : garde la cible actuelle tant que le minuteur (octet `+0xa6`) tourne, sinon en tire une autre.
fn aim_choice(x: &mut Cx) {
    if x.a5_u8(var::PLAYERS_ALIVE) == 0 {
        return;
    }
    if sub8(x, TIMER, 1).gt() {
        // 0x90596 : la cible (+0x50) si elle est utilisable
        let t = x.u16(f::TARGET);
        if (t as i16) < 0 {
            let target = x.linked(f::TARGET);
            if usable(x, target) {
                return aim(x, target);
            }
        }
    }
    x.set8(TIMER, 0x10);
    let r = random(x) & 0xf;
    let pick = x.rom8(AIM_PICK + r) as u32;
    let routine = (AIM_ROUTINES as i32 + x.rom16(AIM_ROUTINES + pick) as i16 as i32) as u32;
    let found = match routine {
        0x904aa => nearest(x, false),
        0x904ee => nearest(x, true),
        0x90524 => in_random_order(x),
        t => unreachable!("boss 19 : choix de cible {t:x}"),
    };
    if let Some(p) = found {
        aim(x, p);
    }
}

/// 0x904aa et 0x904ee : le joueur au plus petit score. Le score est `|x| + |y|` (0x904aa) ou `+0x60` du dragon
/// (0x904ee) ; le meilleur part de -1, donc presque jamais atteint. L'original détruit son compteur de boucle
/// (`D0`) par l'appel d'usage : la boucle reprend le résultat de ce test.
fn nearest(x: &mut Cx, by_own: bool) -> Option<Obj> {
    let mut best: i16 = -1;
    let mut chosen = None;
    let mut counter: u16;
    let mut a2 = PLAYERS;
    loop {
        let player = Obj(a2);
        let ok = usable(x, player);
        counter = ok as u16;
        if ok && (player.u8(x.b, 4) as i8) < 4 {
            let score = if by_own {
                x.u16(0x60) as i16
            } else {
                let dx = (player.u16(x.b, 0x8) as i16).wrapping_abs_keep();
                let dy = (player.u16(x.b, 0x10) as i16).wrapping_abs_keep();
                dx.wrapping_add(dy)
            };
            if score < best {
                best = score;
                chosen = Some(player);
            }
        }
        a2 += 0x200;
        counter = counter.wrapping_sub(1);
        if counter == 0xffff {
            break;
        }
    }
    if best >= 0 { chosen } else { None }
}

trait AbsKeep {
    fn wrapping_abs_keep(self) -> i16;
}

impl AbsKeep for i16 {
    /// `neg.w` si négatif (−0x8000 reste négatif).
    fn wrapping_abs_keep(self) -> i16 {
        if self < 0 { self.wrapping_neg() } else { self }
    }
}

/// 0x90524 : parcourt les joueurs dans un ordre tiré au sort (table de rotations) jusqu'à un joueur utilisable.
/// Même compteur de boucle détruit que `nearest`.
fn in_random_order(x: &mut Cx) -> Option<Obj> {
    let mut order = PLAYER_ORDERS + (random(x) & 0xc);
    let mut counter: u16;
    loop {
        let offset = x.data32(order);
        order += 4;
        let player = Obj(PLAYERS.wrapping_add(offset));
        let ok = usable(x, player);
        counter = ok as u16;
        if ok && (player.u8(x.b, 4) as i8) < 4 {
            return Some(player);
        }
        counter = counter.wrapping_sub(1);
        if counter == 0xffff {
            return None;
        }
    }
}

/// 0x90576 : marche vers le joueur `p` en visant 0xc0 pixels devant lui.
fn aim(x: &mut Cx, p: Obj) {
    let mut d1: i16 = 0xc0;
    if x.u8(f::FLIP) != 0 {
        d1 = d1.wrapping_neg();
    }
    let px = p.u16(x.b, 0x8).wrapping_add(d1 as u16);
    let py = p.u16(x.b, 0x10);
    x.c.a[1] = p.0;
    lib::turn_toward(x, px, py);
    x.move_by_table_xy();
}

// --- +6 = 6 : saut périlleux en arrière ---------------------------------------------------------------------

fn backflip(x: &mut Cx) {
    x.set8(FRAME_KIND, 0x49);
    x.set8(HURT_BOX, 3);
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            x.set8(SHIELD, 8);
            x.set8(0, 1);
            let (a, b) = signed_by_facing(x, -0x400, 0x80);
            x.set16(0x14, a);
            x.set16(0x16, b);
            x.set16(0x18, 0x300);
            x.set16(0x1a, (-0x40i16) as u16);
            x.set32(0x1c, 0);
            x.set16(TIMER, 0x40);
            set_anim(x, 2, 7);
        }
        2 => {
            x.move_xz();
            bounce(x, false);
            step(x);
            let t = x.u16(TIMER).wrapping_sub(1);
            if t & 0x8000 != 0 {
                return fight::pick_attack_b(x);
            }
            x.set16(TIMER, t);
            dust(x, 0x100);
        }
        _ => {}
    }
}

// --- +6 = 8 : saut en avant puis bond -----------------------------------------------------------------------

fn hop(x: &mut Cx) {
    x.set8(FRAME_KIND, 0x49);
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            x.set8(SHIELD, 8);
            x.set8(0, 1);
            let (a, _) = signed_by_facing(x, -0x100, 0);
            x.set16(0x14, a);
            x.set16(0x16, 0);
            x.set16(0x18, 0x200);
            x.set16(0x1a, (-0x20i16) as u16);
            x.set32(0x1c, 0);
            rest(x);
        }
        2 => {
            step(x);
            x.set8(SHIELD, 8);
            x.move_xz();
            if (x.u16(0x18) as i16) > 0 {
                return;
            }
            x.add8(0x7, 2);
            x.set16(TIMER, 0x10);
        }
        4 => {
            step(x);
            if sub16(x, TIMER, 1).gt() {
                return;
            }
            x.add8(0x7, 2);
            x.set16(TIMER, 0x20);
            let (a, b) = signed_by_facing(x, 0x200, 0x40);
            x.set16(0x14, a);
            x.set16(0x16, b);
            x.set16(0x18, (-0x300i16) as u16);
            x.set16(0x1a, 0);
            set_anim(x, 2, 7);
        }
        6 => {
            x.set8(HURT_BOX, 3);
            x.set8(SHIELD, 8);
            x.move_xz();
            bounce(x, false);
            step(x);
            let t = x.u16(TIMER).wrapping_sub(1);
            if t & 0x8000 != 0 {
                return fight::pick_attack_b(x);
            }
            x.set16(TIMER, t);
            if t & 3 == 0 {
                dust(x, 0x100);
            }
        }
        _ => {}
    }
}

// --- +6 = 10 : grand saut ---------------------------------------------------------------------------------

fn lift(x: &mut Cx) {
    x.set16(0x18, 0x800);
    x.set16(0x1a, (-0x40i16) as u16);
}

fn big_jump(x: &mut Cx) {
    x.set8(FRAME_KIND, 0x49);
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            x.set8(SHIELD, 8);
            x.set8(0, 1);
            lift(x);
            x.set32(0x1c, 0);
            x.set32(0x14, 0);
            rest(x);
        }
        2 => {
            x.set8(SHIELD, 8);
            x.move_z();
            if (x.u16(0xc) as i16) >= 0x100 {
                x.add8(0x7, 2);
                x.set16(TIMER, 0x20);
                return;
            }
            if (x.u16(0x18) as i16) < 0 {
                return lift(x);
            }
            x.anim_step();
        }
        4 => {
            if sub16(x, TIMER, 1).gt() {
                return;
            }
            x.add8(0x7, 2);
            // retombe de l'autre côté de l'écran
            let mut d0 = x.a5_u16(var::SCROLL_X);
            if x.u8(0xa) == 0 {
                d0 = x.a5_u16(0x556);
            }
            let mut d1 = d0.wrapping_sub(0x40).wrapping_add(0xc0);
            let (mut side, mut vx) = (0x20u8, 0x600u16);
            d1 = d1.wrapping_sub(x.u16(0x8));
            if (d1 as i16) >= 0 {
                side = 0;
                d0 = d0.wrapping_add(0x1c0);
                vx = vx.wrapping_neg();
            }
            x.set16(0x8, d0);
            x.set16(0xc, 0xc0);
            x.set8(f::FLIP, side);
            x.set16(0x14, vx);
            x.set16(0x16, 0);
            x.set16(0x18, (-0x500i16) as u16);
            x.set16(0x1a, 0);
            x.set32(0x1c, 0);
            x.set16(TIMER, 0x20);
            set_anim(x, 2, 7);
        }
        6 => {
            x.set8(HURT_BOX, 3);
            x.move_xz();
            bounce(x, true);
            step(x);
            let t = x.u16(TIMER).wrapping_sub(1);
            if t & 0x8000 != 0 {
                return fight::pick_attack_b(x);
            }
            x.set16(TIMER, t);
            if (x.u16(0xc) as i16) >= 0x30 || t & 3 != 0 {
                return;
            }
            dust(x, 0x100);
        }
        _ => {}
    }
}
