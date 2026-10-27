//! Fondus de palette (V3) : un pas par cession, jusqu'à la fin du compte (voir `docs/video.md`, « Fondus »).
//!
//! Deux familles, chacune sous deux formes : une *tâche* (créée par `0x9b6`, elle se termine par `0x9f8` à la
//! fin du fondu) et un *sous-programme* (appelé par `jsr` depuis une séquence, il revient quand le fondu est fini ;
//! l'appelant attend donc la fin). Les pas cèdent la main par `jsr 0xa40` (D0.w = délai `A5+0x490`) ; le pas
//! suivant est une étape de la machine à états ([`Etape`]), qui porte le fondu en cours.
//!
//! * **Luminosité** : chaque pas ajoute ±0x1000 aux couleurs des pages cochées (`palette::brightness_step`).
//!   L'état est à `A5+0x48e` (pages, pas restants, délai, adresses des quatre pages) ; le sens est un champ de
//!   l'étape (l'original le garde dans D1, que la cession sauve et rend, et que le contexte garde aussi).
//! * **Par composante** : chaque pas rapproche d'un cran chaque composante (`palette::component_step`) ;
//!   16 pas, `A5-0x65ec` levé pendant le fondu, registres sauvés à l'entrée (`movem`, gardés dans l'étape) et
//!   rendus à la fin.
//!
//! Les sous-programmes sont des fonctions ([`montee`], [`descente`], [`descente_d0`], [`vers_le_blanc`],
//! [`vers_les_couleurs`], [`vers_les_palettes`]) que les pas appellent directement ; seul le fondu au noir `0x1460`
//! garde une native, que des gestionnaires d'objets appellent encore par adresse (`call_at`).

use crate::mem::{rd16, rd8, set_low_word, wr32, wr8, A5};
use crate::modes::Flow;
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

/// Pas de luminosité : monter (vers la pleine luminosité) ou descendre.
const MONTE: u32 = 0x1000;
const DESCEND: u32 = 0xffff_f000;
/// Nombre de pas d'un fondu complet.
const PAS_COMPLET: u8 = 0x10;

/// Pages des fondus : les quatre pages, ou la fin de la page 0 pour le fondu partiel.
const PAGES: [u32; 4] = [0x91c000, 0x91c400, 0x91c800, 0x91cc00];
const DEBUT_PARTIEL: u32 = 0x91c3a0;

/// Étapes d'un fondu : le début d'une tâche de fondu (d'après son entrée), puis le pas suivant après chaque cession.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Etape {
    /// Début d'une tâche de fondu de luminosité ; `pas` : nombre de pas imposé (sinon celui de l'état).
    TacheLuminosite { fondu: Luminosite, pas: Option<u8> },
    /// Début d'une tâche de fondu par composante (`blanchir` : la tâche `0x1ced4` blanchit d'abord les pages).
    TacheComposante { fondu: Composante, blanchir: bool },
    /// Pas suivant d'un fondu de luminosité.
    Luminosite(Luminosite),
    /// Pas suivant d'un fondu par composante ; `registres` : D0-D7/A0-A6 sauvés à l'entrée.
    Composante { fondu: Composante, registres: Registres },
}

/// Registres sauvés par `movem.l D0-D7/A0-A6,-(SP)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Registres {
    d: [u32; 8],
    a: [u32; 7],
}

/// Les tâches de fondu, par entrée.
const TACHES: [(u32, Etape); 10] = [
    (0x14ba, Etape::TacheLuminosite { fondu: tache(MONTE, false), pas: Some(PAS_COMPLET) }),
    (0x14c8, Etape::TacheLuminosite { fondu: tache(MONTE, false), pas: None }),
    (0x14d0, Etape::TacheLuminosite { fondu: tache(DESCEND, false), pas: Some(PAS_COMPLET) }),
    (0x14de, Etape::TacheLuminosite { fondu: tache(DESCEND, false), pas: None }),
    (0x1536, Etape::TacheLuminosite { fondu: tache(MONTE, true), pas: None }),
    (0x153e, Etape::TacheLuminosite { fondu: tache(DESCEND, true), pas: None }),
    (0x1ccf8, Etape::TacheComposante { fondu: COMPOSANTES[3].1, blanchir: false }),
    (0x1ce64, Etape::TacheComposante { fondu: COMPOSANTES[4].1, blanchir: false }),
    (0x1ced4, Etape::TacheComposante { fondu: COMPOSANTES[5].1, blanchir: true }),
    (0x1cf12, Etape::TacheComposante { fondu: COMPOSANTES[5].1, blanchir: false }),
];

/// Première étape de la tâche de fondu qui commence à `entree`.
pub fn tache_depuis_entree(entree: u32) -> Option<Etape> {
    TACHES.into_iter().find(|&(e, _)| e == entree).map(|(_, etape)| etape)
}

impl Etape {
    /// Adresse que l'original reprend à cette étape (entrée de la tâche, ou retour de la cession du pas).
    pub fn adresse(self) -> u32 {
        match self {
            Etape::TacheLuminosite { .. } | Etape::TacheComposante { .. } => {
                TACHES.into_iter().find(|&(_, e)| e == self).expect("tâche de fondu").0
            }
            Etape::Luminosite(f) => f.reprise(),
            Etape::Composante { fondu, .. } => fondu.reprise,
        }
    }

    pub fn pas(self, x: &mut Ctx) -> Flow {
        match self {
            Etape::TacheLuminosite { fondu, pas } => fondu.debut_tache(x, pas),
            Etape::TacheComposante { fondu, blanchir } => fondu.debut(x, blanchir),
            Etape::Luminosite(fondu) => fondu.pas(x),
            Etape::Composante { fondu, registres } => fondu.pas(x, registres),
        }
    }
}

/// Cède `delai` images (D0.w, le mot haut de D0 est gardé) ; la suite est l'étape `reprise`.
fn attendre(x: &mut Ctx, delai: u16, reprise: Etape) -> Flow {
    set_low_word(&mut x.c.d[0], delai);
    crate::kernel::attendre(x.c, x.b, reprise);
    Flow::Yield
}

// ---------------------------------------------------------------------------------------------
// Luminosité

/// Un fondu de luminosité : sens (D1), forme (sous-programme ou tâche), pages (complètes ou partielle).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Luminosite {
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
        attendre(x, delai, Etape::Luminosite(self))
    }

    /// Fin : plus de pages ; le sous-programme revient à son appelant, la tâche se termine.
    fn fin(self, x: &mut Ctx) -> Flow {
        wr8(x.b, etat::PAGES, 0);
        if self.tache {
            super::terminer(x)
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

/// `0x144c` : fondu complet depuis le noir (sous-programme : la suite de l'appelant attend la fin).
pub fn montee(x: &mut Ctx) -> Flow {
    complet(x, MONTE)
}

/// `0x1460` : fondu complet vers le noir.
pub fn descente(x: &mut Ctx) -> Flow {
    complet(x, DESCEND)
}

/// `0x146c` : fondu vers le noir de D0.b pas.
pub fn descente_d0(x: &mut Ctx) -> Flow {
    sous_programme(DESCEND).debut_sous_programme(x)
}

// le fondu au noir reste appelé par adresse (`call_at` des objets SET qui ferment une scène)
resume!(FONDU_DESCENTE, 0x1460, descente);
// tâches : 0x14ba / 0x14d0 fondu complet, 0x14c8 / 0x14de nombre de pas de l'état, 0x1536 / 0x153e partiels (voir
// `TACHES`)

// ---------------------------------------------------------------------------------------------
// Par composante

/// Un fondu par composante : cible, forme et adresse qui suit la cession du pas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Composante {
    cible: Cible,
    tache: bool,
    reprise: u32,
}

/// Les six fondus par composante, par adresse d'entrée.
const COMPOSANTES: [(u32, Composante); 6] = [
    (0x1ccc2, Composante { cible: Cible::Blanc, tache: false, reprise: 0x1cce8 }),
    (0x1ce30, Composante { cible: Cible::Couleurs, tache: false, reprise: 0x1ce54 }),
    (0x1ce9e, Composante { cible: Cible::Palettes, tache: false, reprise: 0x1cec4 }),
    (0x1ccf8, Composante { cible: Cible::Blanc, tache: true, reprise: 0x1cd1e }),
    (0x1ce64, Composante { cible: Cible::Couleurs, tache: true, reprise: 0x1ce8a }),
    (0x1ced4, Composante { cible: Cible::Palettes, tache: true, reprise: 0x1cefe }),
];

fn composante(entree: u32) -> Composante {
    COMPOSANTES.into_iter().find(|&(e, _)| e == entree).unwrap().1
}

impl Composante {
    /// Entrée : registres sauvés, 16 pas (la tâche `0x1ced4` blanchit d'abord les pages, son autre entrée
    /// `0x1cf12` non : `blanchir`).
    fn debut(self, x: &mut Ctx, blanchir: bool) -> Flow {
        let mut registres = Registres { d: x.c.d, a: [0; 7] };
        registres.a.copy_from_slice(&x.c.a[..7]);
        if blanchir {
            white_out(x.b);
        }
        wr8(x.b, etat::PAS, PAS_COMPLET);
        wr8(x.b, COMPOSANTE_EN_COURS, 1);
        self.pas(x, registres)
    }

    fn pas(self, x: &mut Ctx, registres: Registres) -> Flow {
        component_step(x.b, self.cible);
        let reste = rd8(x.b, etat::PAS).wrapping_sub(1);
        wr8(x.b, etat::PAS, reste);
        if reste != 0 {
            let delai = rd16(x.b, etat::DELAI);
            return attendre(x, delai, Etape::Composante { fondu: self, registres });
        }
        wr8(x.b, etat::PAGES, 0);
        wr8(x.b, COMPOSANTE_EN_COURS, 0);
        x.c.d = registres.d;
        x.c.a[..7].copy_from_slice(&registres.a);
        if self.tache {
            super::terminer(x)
        } else {
            Flow::Done
        }
    }
}

/// `0x1ccc2` : fondu par composante vers le blanc (sous-programme).
pub fn vers_le_blanc(x: &mut Ctx) -> Flow {
    composante(0x1ccc2).debut(x, false)
}

/// `0x1ce30` : fondu par composante vers les couleurs.
pub fn vers_les_couleurs(x: &mut Ctx) -> Flow {
    composante(0x1ce30).debut(x, false)
}

/// `0x1ce9e` : fondu par composante vers les palettes.
pub fn vers_les_palettes(x: &mut Ctx) -> Flow {
    composante(0x1ce9e).debut(x, false)
}

pub const NATIVES: &[Native] = &[FONDU_DESCENTE];
