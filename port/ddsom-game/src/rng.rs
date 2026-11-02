//! Générateur pseudo-aléatoire du jeu (U1). Il doit rester **exact au bit près** : tous les tirages
//! du jeu (comportement des ennemis, dégâts, butins...) en dépendent, donc aussi la reproductibilité
//! des parties.
//!
//! L'état est un mot en `A5+0x1a` (`ram::GERME` : octet haut en 0x1a, octet bas en 0x1b), initialisé à
//! 0x01c3 au démarrage d'une partie (0x19a80). Chaque tirage (0x19a88) :
//!
//! ```text
//! t     = ((3 × état) mod 2^16) >> 8        (octet)
//! bas   = bas + t  (mod 256)
//! haut  = t
//! résultat = nouvel état (mot)
//! ```
//!
//! Un seul tirage pour tout le jeu : [`random`] (cœur). Les contextes l'offrent sous `x.random()`
//! (`crate::machine::Machine`) ; la native (appels par adresse) laisse en plus D0.w et les drapeaux de l'original.

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::{Cpu, SR_X};

use crate::mem::rts;
use crate::ram::GERME;
use crate::{regs, Native};

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

/// Un tirage (0x19a88) : avance le germe et rend le nouvel état. Les appelants masquent le résultat.
pub fn random(b: &mut Cps2Bus) -> u16 {
    tirage(b).0
}

fn tirage(b: &mut Cps2Bus) -> (u16, bool) {
    let mut rng = Rng(GERME.get(b));
    let r = rng.suivant();
    GERME.set(b, r.0);
    r
}

/// `random` avec les registres de l'original (native) : D0.w = nouvel état (D0 haut intact), N et Z du mot, X =
/// retenue de l'addition d'octets.
fn random_regs(c: &mut Cpu, b: &mut Cps2Bus) -> u16 {
    let (etat, retenue) = tirage(b);
    c.d[0] = c.d[0] & 0xffff_0000 | etat as u32;
    // move.w final : N et Z du résultat ; X vient de l'addition d'octets
    c.flags_logic(etat as u32, Size::Word);
    c.sr = c.sr & !SR_X | if retenue { SR_X } else { 0 };
    etat
}

/// 0x19a80 : remet la graine.
pub fn seed(b: &mut Cps2Bus) {
    GERME.set(b, GRAINE);
}

/// `seed` avec les drapeaux de l'original (`move.w` : N et Z de la graine).
#[track_caller]
pub fn seed_68k(c: &mut Cpu, b: &mut Cps2Bus) {
    seed(b);
    c.flags_logic(GRAINE as u32, Size::Word);
    crate::diag::registres_morts(c, "random", regs::CCR);
}

pub const SEED: Native = Native {
    entry: 0x19a80,
    name: "rng_seed",
    run: crate::Corps::Processeur(|c, b| {
        seed(b);
        c.flags_logic(GRAINE as u32, Size::Word);
        rts(c, b);
    }),
    touches_sound: false,
    outputs: regs::CCR,
};

pub const NEXT: Native = Native {
    entry: 0x19a88,
    name: "rng_next",
    run: crate::Corps::Processeur(|c, b| {
        random_regs(c, b);
        rts(c, b);
    }),
    touches_sound: false,
    outputs: regs::d(0) | regs::CCR,
};
