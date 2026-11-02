//! Instructions 68000 à drapeaux, pour les routines natives qui doivent laisser exactement les
//! mêmes drapeaux que l'original (les appelants peuvent les lire au retour).

use m68k::decode::Size;
use m68k::{Cpu, SR_C, SR_N, SR_V, SR_X, SR_Z};

/// CMP / CMPI : drapeaux de `dst - src` ; X est conservé.
pub fn cmp(c: &mut Cpu, src: u32, dst: u32, size: Size) {
    let x = c.sr & SR_X;
    c.addsub(true, src, dst, false, size, false);
    c.sr = c.sr & !SR_X | x;
}

/// ADD / ADDQ : résultat et drapeaux (X compris).
pub fn add(c: &mut Cpu, src: u32, dst: u32, size: Size) -> u32 {
    c.addsub(false, src, dst, false, size, false)
}

/// SUB / SUBQ / SUBI : `dst - src`, résultat et drapeaux (X compris).
pub fn sub(c: &mut Cpu, src: u32, dst: u32, size: Size) -> u32 {
    c.addsub(true, src, dst, false, size, false)
}

/// Codes de condition de `Cpu::cond` (Bcc / Scc).
pub mod cc {
    pub const HI: u8 = 2;
    pub const LS: u8 = 3;
    pub const CC: u8 = 4;
    pub const CS: u8 = 5;
    pub const NE: u8 = 6;
    pub const EQ: u8 = 7;
    pub const PL: u8 = 10;
    pub const MI: u8 = 11;
    pub const GE: u8 = 12;
    pub const LT: u8 = 13;
    pub const GT: u8 = 14;
    pub const LE: u8 = 15;
}

/// MOVE / TST / AND / OR... : N et Z selon la valeur, V et C à zéro, X conservé.
pub fn logique(c: &mut Cpu, v: u32, size: Size) {
    c.flags_logic(v, size);
}

/// Pose les drapeaux N, Z, V, C, X d'un coup (masque `SR_N | ...`).
pub fn drapeaux(c: &mut Cpu, masque: u16, valeurs: u16) {
    c.sr = c.sr & !masque | valeurs & masque;
}

/// Tous les drapeaux de condition (X N Z V C).
pub const TOUS: u16 = SR_X | SR_N | SR_Z | SR_V | SR_C;

/// ABCD d'un octet : `dst + src + X` en décimal codé binaire. Retourne le résultat et la retenue
/// (qui est aussi X). Algorithme du 68000 réel (valable aussi pour des quartets invalides).
pub fn abcd(src: u8, dst: u8, x: bool) -> (u8, bool) {
    let (sv, dv, x) = (src as u32, dst as u32, x as u32);
    let somme = sv + dv + x;
    let mut ajuste = 0;
    if (sv & 0x0f) + (dv & 0x0f) + x > 9 {
        ajuste |= 0x06;
    }
    let retenue = somme > 0x99;
    if retenue {
        ajuste |= 0x60;
    }
    ((somme + ajuste) as u8, retenue)
}
