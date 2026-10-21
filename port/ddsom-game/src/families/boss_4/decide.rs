//! Boss 4 : états posés d'un coup (0x8328e..0x833ac) et choix de l'attaque pendant la marche.
//!
//! Pendant la marche, quatre boîtes devant le boss décident de la suite ; chaque décision tire un nombre de 0 à 15,
//! lit dans une table d'octets de la ROM (choisie selon la phase `+0xaa` et, pour les deux boîtes de l'IA, le geste
//! 1 ou 2 rendu par 0x1047a) un indice dans une table de sauts vers l'un des états ci-dessous.

use super::*;

/// 0x8328e : combat, debout.
pub(super) fn idle(x: &mut Cx) {
    x.set8(VULNERABLE, 1);
    x.set32(f::STATE, 0x0202_0000);
}

/// 0x8329e : demi-tour, puis reprise de l'état en cours.
pub(super) fn turn(x: &mut Cx) {
    x.set8(VULNERABLE, 0);
    let s = x.u32(f::STATE);
    x.set32(SAVED_STATE, s);
    x.set32(f::STATE, 0x0202_0200);
}

/// 0x832b2 : marche vers la cible, pas rapides (tir 0x38).
pub(super) fn walk(x: &mut Cx) {
    x.set8(VULNERABLE, 1);
    x.set8(SHOT, 0x38);
    x.set32(f::STATE, 0x0202_0400);
}

/// 0x832c8 : marche lente (tir 0x39).
pub(super) fn walk_slow(x: &mut Cx) {
    x.set8(VULNERABLE, 1);
    x.set8(SHOT, 0x39);
    x.set32(f::STATE, 0x0202_0600);
}

/// 0x832de / 0x832f0 : charge vers le milieu de l'écran, ou vers la cible.
pub(super) fn charge(x: &mut Cx, at_target: bool) {
    x.set8(VULNERABLE, 0);
    x.set8(CHARGE_AT_TARGET, at_target as u8);
    x.set32(f::STATE, 0x0202_0800);
}

/// 0x83304 : bond en arrière.
fn hop(x: &mut Cx) {
    x.set8(VULNERABLE, 0);
    x.set32(f::STATE, 0x0202_0a00);
}

/// Attaque `n` (`+6` = 2n du groupe 4), vulnérable ou non.
fn attack(x: &mut Cx, n: u8, vulnerable: bool) {
    x.set8(VULNERABLE, vulnerable as u8);
    x.set32(f::STATE, 0x0204_0000 | (n as u32 * 2) << 8);
}

/// 0x8333e : traversée de l'écran en vol (attaque 3), invulnérable.
pub(super) fn swoop(x: &mut Cx) {
    x.set8(INVULNERABLE, 1);
    attack(x, 3, false);
}

/// Le premier double est-il occupé (`+0xaf`) ? Les souffles cèdent alors la place à une autre attaque.
fn double_busy(x: &mut Cx) -> bool {
    let d = double(x, DOUBLE_A);
    d.u8(x.b, 0xaf) != 0
}

/// 0x833ac : attente d'un joueur.
pub(super) fn wait(x: &mut Cx) {
    x.set32(f::STATE, 0x020a_0000);
}

/// Pose l'état de la routine `target` d'une table de sauts de décision.
fn enter(x: &mut Cx, target: u32) {
    match target {
        0x8328e => idle(x),
        0x832c8 => walk_slow(x),
        0x832f0 => charge(x, true),
        0x83304 => hop(x),
        0x83312 => attack(x, 0, true),
        0x83322 => attack(x, 1, false),
        0x83330 => attack(x, 2, false),
        0x83344 => attack(x, 3, false),
        // 0x83352 : souffle en vol (invulnérable), 0x83364 : souffle au sol, sinon envol (attaque 2)
        0x83352 | 0x83364 => {
            if double_busy(x) {
                return attack(x, 2, false);
            }
            if target == 0x83352 {
                x.set8(INVULNERABLE, 1);
            }
            attack(x, 4, false);
        }
        // 0x8337c, 0x83394 : longs souffles, sinon piqué (1) ou envol (2)
        0x8337c => {
            if double_busy(x) {
                return attack(x, 1, false);
            }
            attack(x, 5, false);
        }
        0x83394 => {
            if double_busy(x) {
                return attack(x, 2, false);
            }
            attack(x, 6, false);
        }
        _ => unreachable!("boss 4 : état {target:x}"),
    }
}

/// `move.w (tbl,PC,Dn),Dn ; jmp (tbl,PC,Dn)` : adresse d'arrivée pour l'indice `i` (mot de `D0` mis à jour).
fn jump(x: &mut Cx, table: u32, i: u16) -> u32 {
    let off = x.rom16(table + i as u32);
    setw(x, 0, off);
    table.wrapping_add(off as i16 as i32 as u32)
}

/// Tire `r` (0 à 15), lit l'indice de saut dans `bytes`, puis entre dans l'état désigné par la table `jumps`.
fn roll(x: &mut Cx, r: u32, bytes: u32, jumps: u32) {
    let k = x.rom8(bytes + r);
    x.c.d[0] = x.c.d[0] & !0xff | k as u32;
    let t = jump(x, jumps, k as u16);
    enter(x, t)
}

/// Phase `+0xaa` (octet étendu en mot).
fn phase(x: &mut Cx) -> u16 {
    x.u8(PHASE) as i8 as i16 as u16
}

/// 0x830ce : la cible est dans la boîte large (0x7ef4) : table 0x830e4 (un seul choix, le bond en arrière).
pub(super) fn from_wide_box(x: &mut Cx) {
    let r = rand_and(x, 0xf);
    roll(x, r, 0x830e4, 0x830f4);
}

/// 0x830f6 : la cible est tout près (0xb5e0) : selon la phase, tables 0x8312e, 0x8313e, 0x8314e.
pub(super) fn from_close(x: &mut Cx) {
    let r = rand_and(x, 0xf);
    let p = phase(x).wrapping_add(phase(x));
    setw(x, 1, p);
    let bytes = [0x8312e, 0x8313e, 0x8314e][p as usize / 2];
    roll(x, r, bytes, 0x8315e);
}

/// 0x83164 (boîte proche, `near`) et 0x831f6 (boîte lointaine) : selon la phase et le geste `gesture` (1 ou 2) de
/// l'IA.
pub(super) fn from_ai(x: &mut Cx, gesture: u16, near: bool) {
    setw(x, 2, gesture);
    let r = rand_and(x, 0xf);
    let p = phase(x);
    setw(x, 1, p.wrapping_mul(4));
    let row = (gesture.wrapping_sub(1).wrapping_mul(2)).wrapping_add(p.wrapping_mul(4));
    setw(x, 2, row);
    let (bytes, jumps): ([u32; 6], u32) = if near {
        ([0x831be, 0x831de, 0x831be, 0x831de, 0x831ce, 0x831de], 0x831ee)
    } else {
        ([0x83250, 0x83250, 0x83260, 0x83260, 0x83270, 0x83270], 0x83280)
    };
    roll(x, r, bytes[row as usize / 2], jumps);
}
