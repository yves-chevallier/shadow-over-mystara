//! Fondus de palette (V3) : un pas par cession, jusqu'à la fin du compte (voir `docs/video.md`, « Fondus »).
//!
//! Deux familles, chacune sous deux formes : une *tâche* (créée par `0x9b6`, elle se termine par `0x9f8` à la
//! fin du fondu) et un *sous-programme* (appelé par `jsr` depuis une séquence, il revient quand le fondu est fini ;
//! l'appelant attend donc la fin). Les pas cèdent la main par `jsr 0xa40` (D0.w = délai `A5+0x490`) ; chaque
//! adresse qui suit cette cession reçoit la native du pas suivant.
//!
//! * **Luminosité** : chaque pas ajoute ±0x1000 aux couleurs des pages cochées (`palette::brightness_step`).
//!   L'état est à `A5+0x48e` (pages, pas restants, délai, adresses des quatre pages) ; le sens n'est que dans
//!   D1, que la cession sauve et rend.
//! * **Par composante** : chaque pas rapproche d'un cran chaque composante (`palette::component_step`) ;
//!   16 pas, `A5-0x65ec` levé pendant le fondu, registres sauvés à l'entrée (`movem`) et rendus à la fin.

use crate::mem::{rd16, rd32, rd8, set_low_word, wr32, wr8, A5};
use crate::modes::{Flow, Sequence};
use crate::palette::{brightness_step, component_step, white_out, Cible};
use crate::resume;
use crate::scene::Ctx;
use crate::Native;

/// État du fondu de luminosité (`A4` dans l'original).
mod etat {
    use crate::mem::A5;
    pub const BASE: u32 = A5 + 0x48e;
    /// Pages concernées (bit k : page k), remis à zéro à la fin du fondu.
    pub const PAGES: u32 = BASE;
    /// Pas restants.
    pub const PAS: u32 = BASE + 1;
    /// Délai entre deux pas (mot, images).
    pub const DELAI: u32 = BASE + 2;
    /// Adresses des quatre pages (longs).
    pub const ADRESSES: u32 = BASE + 4;
}

/// « Fondu par composante en cours » (octet, A5 - 0x65ec).
const COMPOSANTE_EN_COURS: u32 = A5 - 0x65ec;

/// Sous-programmes du noyau.
const ATTENDRE: u32 = 0xa40;
const TERMINER: u32 = 0x9f8;

/// Pas de luminosité : monter (vers la pleine luminosité) ou descendre.
const MONTE: u32 = 0x1000;
const DESCEND: u32 = 0xffff_f000;
/// Nombre de pas d'un fondu complet.
const PAS_COMPLET: u8 = 0x10;

/// Pages des fondus : les quatre pages, ou la fin de la page 0 pour le fondu partiel.
const PAGES: [u32; 4] = [0x91c000, 0x91c400, 0x91c800, 0x91cc00];
const DEBUT_PARTIEL: u32 = 0x91c3a0;

/// Cède `delai` images (D0.w, le mot haut de D0 est gardé) ; la suite est la native posée à `reprise`.
fn attendre(x: &mut Ctx, delai: u16, reprise: u32) -> Flow {
    set_low_word(&mut x.c.d[0], delai);
    x.tail(ATTENDRE, reprise)
}

/// La tâche se termine (`jsr 0x9f8`, dont l'adresse de retour reste sur la pile).
fn terminer(x: &mut Ctx, retour: u32) -> Flow {
    x.tail(TERMINER, retour)
}

// ---------------------------------------------------------------------------------------------
// Luminosité

/// Un fondu de luminosité : sens (D1), forme (sous-programme ou tâche), pages (complètes ou partielle).
#[derive(Clone, Copy)]
struct Luminosite {
    delta: u32,
    tache: bool,
    partiel: bool,
}

impl Luminosite {
    /// Adresse qui suit la cession du pas.
    fn reprise(self) -> u32 {
        match (self.tache, self.partiel) {
            (false, _) => 0x14b4,
            (true, false) => 0x152c,
            (true, true) => 0x158c,
        }
    }

    /// Lit le fondu interrompu à `reprise` : le sens est dans D1.
    fn repris(x: &Ctx, reprise: u32) -> Self {
        Luminosite { delta: x.c.d[1], tache: reprise != 0x14b4, partiel: reprise == 0x158c }
    }

    /// Début d'une tâche : `pas` = nombre de pas imposé (sinon celui de l'état). Elle ne démarre que si les
    /// pages, le délai et le nombre de pas sont non nuls.
    fn debut_tache(self, x: &mut Ctx, pas: Option<u8>) -> Flow {
        x.c.d[1] = self.delta;
        if let Some(n) = pas {
            wr8(x.b, etat::PAS, n);
        }
        x.c.a[4] = etat::BASE;
        if rd8(x.b, etat::PAGES) == 0 || rd16(x.b, etat::DELAI) == 0 || rd8(x.b, etat::PAS) == 0 {
            return self.fin(x);
        }
        self.premier_pas(x)
    }

    /// Début d'un sous-programme : D0.b = nombre de pas ; il ne démarre que si des pages sont cochées.
    fn debut_sous_programme(self, x: &mut Ctx) -> Flow {
        x.c.d[1] = self.delta;
        x.c.a[4] = etat::BASE;
        if rd8(x.b, etat::PAGES) == 0 {
            return self.fin(x);
        }
        wr8(x.b, etat::PAS, x.c.d[0] as u8);
        self.premier_pas(x)
    }

    fn premier_pas(self, x: &mut Ctx) -> Flow {
        for (k, page) in PAGES.into_iter().enumerate() {
            let page = if self.partiel && k == 0 { DEBUT_PARTIEL } else { page };
            wr32(x.b, etat::ADRESSES + 4 * k as u32, page);
        }
        self.pas(x)
    }

    /// Un pas, puis la cession jusqu'au suivant, ou la fin.
    fn pas(self, x: &mut Ctx) -> Flow {
        brightness_step(x.b, self.delta, self.partiel);
        let reste = rd8(x.b, etat::PAS).wrapping_sub(1);
        wr8(x.b, etat::PAS, reste);
        if reste == 0 {
            return self.fin(x);
        }
        let delai = rd16(x.b, etat::DELAI);
        attendre(x, delai, self.reprise())
    }

    /// Fin : plus de pages ; le sous-programme revient à son appelant, la tâche se termine.
    fn fin(self, x: &mut Ctx) -> Flow {
        wr8(x.b, etat::PAGES, 0);
        if self.tache {
            terminer(x, 0x1536)
        } else {
            Flow::Done
        }
    }
}

const fn sous_programme(delta: u32) -> Luminosite {
    Luminosite { delta, tache: false, partiel: false }
}
const fn tache(delta: u32, partiel: bool) -> Luminosite {
    Luminosite { delta, tache: true, partiel }
}

/// Fondu complet d'un sous-programme : D0.b = 16 pas.
fn complet(x: &mut Ctx, delta: u32) -> Flow {
    crate::mem::set_low_byte(&mut x.c.d[0], PAS_COMPLET);
    sous_programme(delta).debut_sous_programme(x)
}

// sous-programmes : 0x144c / 0x1460 fondu complet, 0x146c D0.b pas
resume!(FONDU_MONTEE, 0x144c, |x| complet(x, MONTE));
resume!(FONDU_DESCENTE, 0x1460, |x| complet(x, DESCEND));
resume!(FONDU_DESCENTE_D0, 0x146c, |x| sous_programme(DESCEND).debut_sous_programme(x));
// tâches : 0x14ba / 0x14d0 fondu complet, 0x14c8 / 0x14de nombre de pas de l'état, 0x1536 / 0x153e partiels
resume!(FONDU_TACHE_MONTEE, 0x14ba, |x| tache(MONTE, false).debut_tache(x, Some(PAS_COMPLET)));
resume!(FONDU_TACHE_MONTEE_N, 0x14c8, |x| tache(MONTE, false).debut_tache(x, None));
resume!(FONDU_TACHE_DESCENTE, 0x14d0, |x| tache(DESCEND, false).debut_tache(x, Some(PAS_COMPLET)));
resume!(FONDU_TACHE_DESCENTE_N, 0x14de, |x| tache(DESCEND, false).debut_tache(x, None));
resume!(FONDU_TACHE_RETABLIT, 0x1536, |x| tache(MONTE, true).debut_tache(x, None));
resume!(FONDU_TACHE_ATTENUE, 0x153e, |x| tache(DESCEND, true).debut_tache(x, None));
// reprises après la cession d'un pas
resume!(FONDU_PAS, 0x14b4, |x| Luminosite::repris(x, 0x14b4).pas(x));
resume!(FONDU_PAS_TACHE, 0x152c, |x| Luminosite::repris(x, 0x152c).pas(x));
resume!(FONDU_PAS_PARTIEL, 0x158c, |x| Luminosite::repris(x, 0x158c).pas(x));

// ---------------------------------------------------------------------------------------------
// Par composante

/// Un fondu par composante : cible, forme et adresse qui suit la cession du pas.
#[derive(Clone, Copy)]
struct Composante {
    cible: Cible,
    tache: bool,
    reprise: u32,
}

/// Les six fondus par composante, par adresse d'entrée, et l'adresse de retour du `jsr 0x9f8` des tâches.
const COMPOSANTES: [(u32, Composante, u32); 6] = [
    (0x1ccc2, Composante { cible: Cible::Blanc, tache: false, reprise: 0x1cce8 }, 0),
    (0x1ce30, Composante { cible: Cible::Couleurs, tache: false, reprise: 0x1ce54 }, 0),
    (0x1ce9e, Composante { cible: Cible::Palettes, tache: false, reprise: 0x1cec4 }, 0),
    (0x1ccf8, Composante { cible: Cible::Blanc, tache: true, reprise: 0x1cd1e }, 0x1cd32),
    (0x1ce64, Composante { cible: Cible::Couleurs, tache: true, reprise: 0x1ce8a }, 0x1ce9e),
    (0x1ced4, Composante { cible: Cible::Palettes, tache: true, reprise: 0x1cefe }, 0x1cf12),
];

fn composante(adresse: u32) -> (Composante, u32) {
    let (_, f, retour) = COMPOSANTES.into_iter().find(|&(entree, f, _)| entree == adresse || f.reprise == adresse).unwrap();
    (f, retour)
}

/// Registres sauvés à l'entrée (`movem.l D0-D7/A0-A6,-(SP)`) et rendus à la fin.
fn sauver_registres(x: &mut Ctx) {
    let sp = x.c.a[7].wrapping_sub(60);
    for i in 0..15 {
        let r = if i < 8 { x.c.d[i] } else { x.c.a[i - 8] };
        wr32(x.b, sp + 4 * i as u32, r);
    }
    x.c.a[7] = sp;
}

fn rendre_registres(x: &mut Ctx) {
    let sp = x.c.a[7];
    for i in 0..15 {
        let r = rd32(x.b, sp + 4 * i as u32);
        if i < 8 {
            x.c.d[i] = r;
        } else {
            x.c.a[i - 8] = r;
        }
    }
    x.c.a[7] = sp.wrapping_add(60);
}

impl Composante {
    /// Entrée : registres sauvés, 16 pas (la tâche `0x1ced4` blanchit d'abord les pages, son autre entrée
    /// `0x1cf12` non : `blanchir`).
    fn debut(self, x: &mut Ctx, blanchir: bool) -> Flow {
        sauver_registres(x);
        if blanchir {
            white_out(x.b);
        }
        wr8(x.b, etat::PAS, PAS_COMPLET);
        wr8(x.b, COMPOSANTE_EN_COURS, 1);
        self.pas(x)
    }

    fn pas(self, x: &mut Ctx) -> Flow {
        component_step(x.b, self.cible);
        let reste = rd8(x.b, etat::PAS).wrapping_sub(1);
        wr8(x.b, etat::PAS, reste);
        if reste != 0 {
            let delai = rd16(x.b, etat::DELAI);
            return attendre(x, delai, self.reprise);
        }
        wr8(x.b, etat::PAGES, 0);
        wr8(x.b, COMPOSANTE_EN_COURS, 0);
        rendre_registres(x);
        if self.tache {
            let (_, retour) = composante(self.reprise);
            terminer(x, retour)
        } else {
            Flow::Done
        }
    }
}

macro_rules! composante {
    ($entree:ident, $reprise:ident, $adresse:expr) => {
        resume!($entree, $adresse, |x| composante($adresse).0.debut(x, $adresse == 0x1ced4));
        resume!($reprise, composante_reprise($adresse), |x| composante($adresse).0.pas(x));
    };
}

const fn composante_reprise(adresse: u32) -> u32 {
    let mut i = 0;
    while i < COMPOSANTES.len() {
        if COMPOSANTES[i].0 == adresse {
            return COMPOSANTES[i].1.reprise;
        }
        i += 1;
    }
    panic!("fondu par composante inconnu")
}

composante!(FONDU_BLANC, FONDU_BLANC_PAS, 0x1ccc2);
composante!(FONDU_COULEURS, FONDU_COULEURS_PAS, 0x1ce30);
composante!(FONDU_PALETTES, FONDU_PALETTES_PAS, 0x1ce9e);
composante!(FONDU_TACHE_BLANC, FONDU_TACHE_BLANC_PAS, 0x1ccf8);
composante!(FONDU_TACHE_COULEURS, FONDU_TACHE_COULEURS_PAS, 0x1ce64);
composante!(FONDU_TACHE_PALETTES, FONDU_TACHE_PALETTES_PAS, 0x1ced4);
resume!(FONDU_TACHE_PALETTES_B, 0x1cf12, |x| composante(0x1ced4).0.debut(x, false));

pub const NATIVES: &[Native] = &[
    FONDU_MONTEE,
    FONDU_DESCENTE,
    FONDU_DESCENTE_D0,
    FONDU_TACHE_MONTEE,
    FONDU_TACHE_MONTEE_N,
    FONDU_TACHE_DESCENTE,
    FONDU_TACHE_DESCENTE_N,
    FONDU_TACHE_RETABLIT,
    FONDU_TACHE_ATTENUE,
    FONDU_PAS,
    FONDU_PAS_TACHE,
    FONDU_PAS_PARTIEL,
    FONDU_BLANC,
    FONDU_BLANC_PAS,
    FONDU_COULEURS,
    FONDU_COULEURS_PAS,
    FONDU_PALETTES,
    FONDU_PALETTES_PAS,
    FONDU_TACHE_BLANC,
    FONDU_TACHE_BLANC_PAS,
    FONDU_TACHE_COULEURS,
    FONDU_TACHE_COULEURS_PAS,
    FONDU_TACHE_PALETTES,
    FONDU_TACHE_PALETTES_PAS,
    FONDU_TACHE_PALETTES_B,
];
