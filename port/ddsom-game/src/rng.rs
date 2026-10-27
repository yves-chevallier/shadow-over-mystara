//! Générateur pseudo-aléatoire du jeu (U1). Il doit rester **exact au bit près** : tous les tirages
//! du jeu (comportement des ennemis, dégâts, butins...) en dépendent, donc aussi la reproductibilité
//! des parties.
//!
//! L'état est un mot en `A5+0x1a` (octet haut en 0x1a, octet bas en 0x1b), initialisé à 0x01c3 au
//! démarrage d'une partie (0x19a80). Chaque tirage (0x19a88) :
//!
//! ```text
//! t     = ((3 × état) mod 2^16) >> 8        (octet)
//! bas   = bas + t  (mod 256)
//! haut  = t
//! résultat = nouvel état (mot)
//! ```
//!
//! Les appelants (537 sites) masquent le résultat avec `andi` ; aucun ne lit les drapeaux.

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::{Cpu, SR_X};

use crate::mem::{rd16, rts, wr16};
use crate::{regs, Native};

/// Mot d'état, relatif à A5.
const ETAT: u32 = 0x1a;
/// Graine posée au début d'une partie.
pub const GRAINE: u16 = 0x01c3;

/// État du générateur.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rng(pub u16);

impl Rng {
    pub const fn graine() -> Self {
        Rng(GRAINE)
    }

    /// Tirage suivant : renvoie le nouvel état et la retenue de l'addition d'octets (drapeau X que
    /// l'original laisse derrière lui).
    pub fn suivant(&mut self) -> (u16, bool) {
        let t = ((self.0.wrapping_mul(3)) >> 8) as u8;
        let (bas, retenue) = (self.0 as u8).overflowing_add(t);
        self.0 = (t as u16) << 8 | bas as u16;
        (self.0, retenue)
    }
}

/// 0x19a80 : remet la graine.
fn graine(c: &mut Cpu, b: &mut Cps2Bus) {
    wr16(b, c.a[5] + ETAT, GRAINE);
    c.flags_logic(GRAINE as u32, Size::Word);
    rts(c, b);
}

/// 0x19a88 : un tirage. Sortie : D0 (mot bas) = nouvel état. D0 haut est rendu intact.
fn tirage(c: &mut Cpu, b: &mut Cps2Bus) {
    let mut rng = Rng(rd16(b, c.a[5] + ETAT));
    let (etat, retenue) = rng.suivant();
    wr16(b, c.a[5] + ETAT, etat);
    c.d[0] = c.d[0] & 0xffff_0000 | etat as u32;
    // move.w final : N et Z du résultat ; X vient de l'addition d'octets
    c.flags_logic(etat as u32, Size::Word);
    c.sr = c.sr & !SR_X | if retenue { SR_X } else { 0 };
    rts(c, b);
}

pub const SEED: Native = Native {
    entry: 0x19a80,
    name: "rng_seed",
    run: graine,
    touches_sound: false,
    outputs: regs::CCR,
};

pub const NEXT: Native = Native {
    entry: 0x19a88,
    name: "rng_next",
    run: tirage,
    touches_sound: false,
    outputs: regs::d(0) | regs::CCR,
};
