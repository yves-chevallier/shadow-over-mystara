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
//! Les sous-programmes sont des fonctions ([`rise`], [`descente`], [`descent_d0`], [`to_white`],
//! [`to_colours`], [`to_palettes`]) que les pas appellent directement (les portes SET aussi :
//! `families::set::fondu_de_sortie`).

use crate::mem::{rd16, rd8, set_low_word, wr32, wr8, A5};
use crate::modes::Flow;
use crate::palette::{brightness_step, component_step, white_out, Target};
use crate::scene::Ctx;
use crate::Native;

/// État du fondu de luminosité (`A4` dans l'original).
mod state {
    use crate::mem::A5;
    pub const BASE: u32 = A5 + 0x48e;
    /// Pages concernées (bit k : page k), remis à zéro à la fin du fondu.
    pub const PAGES: u32 = BASE;
    /// Pas restants.
    pub const STEP: u32 = BASE + 1;
    /// Délai entre deux pas (mot, images).
    pub const DELAY: u32 = BASE + 2;
    /// Adresses des quatre pages (longs).
    pub const ADDRESSES: u32 = BASE + 4;
}

/// « Fondu par composante en cours » (octet, A5 - 0x65ec).
const CURRENT_COMPONENT: u32 = A5 - 0x65ec;

/// Pas de luminosité : monter (vers la pleine luminosité) ou descendre.
const RISES: u32 = 0x1000;
const DESCEND: u32 = 0xffff_f000;
/// Nombre de pas d'un fondu complet.
const STEP_FULL: u8 = 0x10;

/// Pages des fondus : les quatre pages, ou la fin de la page 0 pour le fondu partiel.
const PAGES: [u32; 4] = [0x91c000, 0x91c400, 0x91c800, 0x91cc00];
const PARTIAL_START: u32 = 0x91c3a0;

/// Étapes d'un fondu : le début d'une tâche de fondu (d'après son entrée), puis le pas suivant après chaque cession.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Etape {
    /// Début d'une tâche de fondu de luminosité ; `step` : nombre de pas imposé (sinon celui de l'état).
    BrightnessTask { fondu: Brightness, step: Option<u8> },
    /// Début d'une tâche de fondu par composante (`whiten` : la tâche `0x1ced4` blanchit d'abord les pages).
    ComponentTask { fondu: Component, whiten: bool },
    /// Pas suivant d'un fondu de luminosité ; `a0` : A0 de l'appelant du sous-programme, que le fondu ne touche pas
    /// (l'original le garde dans ses registres, que la cession sauve et rend) et que l'appelant relit au retour (bloc
    /// de l'état partagé des séquences de sortie de scène, objet d'une porte SET...).
    Brightness { fondu: Brightness, a0: u32 },
    /// Pas suivant d'un fondu par composante ; `regs` : D0-D7/A0-A6 sauvés à l'entrée.
    Component { fondu: Component, regs: Registres },
}

/// Registres sauvés par `movem.l D0-D7/A0-A6,-(SP)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Registres {
    d: [u32; 8],
    a: [u32; 7],
}

/// Les tâches de fondu, par entrée.
const TASKS: [(u32, Etape); 10] = [
    (0x14ba, Etape::BrightnessTask { fondu: task(RISES, false), step: Some(STEP_FULL) }),
    (0x14c8, Etape::BrightnessTask { fondu: task(RISES, false), step: None }),
    (0x14d0, Etape::BrightnessTask { fondu: task(DESCEND, false), step: Some(STEP_FULL) }),
    (0x14de, Etape::BrightnessTask { fondu: task(DESCEND, false), step: None }),
    (0x1536, Etape::BrightnessTask { fondu: task(RISES, true), step: None }),
    (0x153e, Etape::BrightnessTask { fondu: task(DESCEND, true), step: None }),
    (0x1ccf8, Etape::ComponentTask { fondu: COMPONENTS[3].1, whiten: false }),
    (0x1ce64, Etape::ComponentTask { fondu: COMPONENTS[4].1, whiten: false }),
    (0x1ced4, Etape::ComponentTask { fondu: COMPONENTS[5].1, whiten: true }),
    (0x1cf12, Etape::ComponentTask { fondu: COMPONENTS[5].1, whiten: false }),
];

/// Première étape de la tâche de fondu qui commence à `entry`.
pub fn task_from_entry(entry: u32) -> Option<Etape> {
    TASKS.into_iter().find(|&(e, _)| e == entry).map(|(_, step)| step)
}

impl Etape {
    /// Adresse que l'original reprend à cette étape (entrée de la tâche, ou retour de la cession du pas).
    pub fn adresse(self) -> u32 {
        match self {
            Etape::BrightnessTask { .. } | Etape::ComponentTask { .. } => {
                TASKS.into_iter().find(|&(_, e)| e == self).expect("tâche de fondu").0
            }
            Etape::Brightness { fondu, .. } => fondu.resume(),
            Etape::Component { fondu, .. } => fondu.resume,
        }
    }

    pub fn pas(self, x: &mut Ctx) -> Flow {
        match self {
            Etape::BrightnessTask { fondu, step } => fondu.task_start(x, step),
            Etape::ComponentTask { fondu, whiten } => fondu.debut(x, whiten),
            Etape::Brightness { fondu, a0 } => {
                x.c.a[0] = a0;
                fondu.pas(x)
            }
            Etape::Component { fondu, regs } => fondu.pas(x, regs),
        }
    }
}

/// Cède `delay` images (D0.w, le mot haut de D0 est gardé) ; la suite est l'étape `resume`.
fn attendre(x: &mut Ctx, delay: u16, resume: Etape) -> Flow {
    set_low_word(&mut x.c.d[0], delay);
    crate::kernel::attendre(x.c, x.b, resume);
    Flow::Yield
}

// ---------------------------------------------------------------------------------------------
// Luminosité

/// Un fondu de luminosité : sens (D1), forme (sous-programme ou tâche), pages (complètes ou partielle).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Brightness {
    delta: u32,
    task: bool,
    partial: bool,
}

impl Brightness {
    /// Adresse qui suit la cession du pas.
    fn resume(self) -> u32 {
        match (self.task, self.partial) {
            (false, _) => 0x14b4,
            (true, false) => 0x152c,
            (true, true) => 0x158c,
        }
    }

    /// Début d'une tâche : `step` = nombre de pas imposé (sinon celui de l'état). Elle ne démarre que si les
    /// pages, le délai et le nombre de pas sont non nuls.
    fn task_start(self, x: &mut Ctx, step: Option<u8>) -> Flow {
        x.c.d[1] = self.delta;
        if let Some(n) = step {
            wr8(x.b, state::STEP, n);
        }
        x.c.a[4] = state::BASE;
        if rd8(x.b, state::PAGES) == 0 || rd16(x.b, state::DELAY) == 0 || rd8(x.b, state::STEP) == 0 {
            return self.fin(x);
        }
        self.first_step(x)
    }

    /// Début d'un sous-programme : D0.b = nombre de pas ; il ne démarre que si des pages sont cochées.
    fn subroutine_start(self, x: &mut Ctx) -> Flow {
        x.c.d[1] = self.delta;
        x.c.a[4] = state::BASE;
        if rd8(x.b, state::PAGES) == 0 {
            return self.fin(x);
        }
        wr8(x.b, state::STEP, x.c.d[0] as u8);
        self.first_step(x)
    }

    fn first_step(self, x: &mut Ctx) -> Flow {
        for (k, page) in PAGES.into_iter().enumerate() {
            let page = if self.partial && k == 0 { PARTIAL_START } else { page };
            wr32(x.b, state::ADDRESSES + 4 * k as u32, page);
        }
        self.pas(x)
    }

    /// Un pas, puis la cession jusqu'au suivant, ou la fin.
    fn pas(self, x: &mut Ctx) -> Flow {
        brightness_step(x.b, self.delta, self.partial);
        let remainder = rd8(x.b, state::STEP).wrapping_sub(1);
        wr8(x.b, state::STEP, remainder);
        if remainder == 0 {
            return self.fin(x);
        }
        let delay = rd16(x.b, state::DELAY);
        let a0 = x.c.a[0];
        attendre(x, delay, Etape::Brightness { fondu: self, a0 })
    }

    /// Fin : plus de pages ; le sous-programme revient à son appelant, la tâche se termine.
    fn fin(self, x: &mut Ctx) -> Flow {
        wr8(x.b, state::PAGES, 0);
        if self.task {
            super::finish(x)
        } else {
            Flow::Done
        }
    }
}

const fn sous_programme(delta: u32) -> Brightness {
    Brightness { delta, task: false, partial: false }
}
const fn task(delta: u32, partial: bool) -> Brightness {
    Brightness { delta, task: true, partial }
}

/// Fondu complet d'un sous-programme : D0.b = 16 pas.
fn full(x: &mut Ctx, delta: u32) -> Flow {
    crate::mem::set_low_byte(&mut x.c.d[0], STEP_FULL);
    sous_programme(delta).subroutine_start(x)
}

/// `0x144c` : fondu complet depuis le noir (sous-programme : la suite de l'appelant attend la fin).
pub fn rise(x: &mut Ctx) -> Flow {
    full(x, RISES)
}

/// `0x1460` : fondu complet vers le noir.
pub fn descente(x: &mut Ctx) -> Flow {
    full(x, DESCEND)
}

/// `0x146c` : fondu vers le noir de D0.b pas.
pub fn descent_d0(x: &mut Ctx) -> Flow {
    sous_programme(DESCEND).subroutine_start(x)
}

// tâches : 0x14ba / 0x14d0 fondu complet, 0x14c8 / 0x14de nombre de pas de l'état, 0x1536 / 0x153e partiels (voir
// `TASKS`)

// ---------------------------------------------------------------------------------------------
// Par composante

/// Un fondu par composante : cible, forme et adresse qui suit la cession du pas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Component {
    target: Target,
    task: bool,
    resume: u32,
}

/// Les six fondus par composante, par adresse d'entrée.
const COMPONENTS: [(u32, Component); 6] = [
    (0x1ccc2, Component { target: Target::White, task: false, resume: 0x1cce8 }),
    (0x1ce30, Component { target: Target::Colours, task: false, resume: 0x1ce54 }),
    (0x1ce9e, Component { target: Target::Palettes, task: false, resume: 0x1cec4 }),
    (0x1ccf8, Component { target: Target::White, task: true, resume: 0x1cd1e }),
    (0x1ce64, Component { target: Target::Colours, task: true, resume: 0x1ce8a }),
    (0x1ced4, Component { target: Target::Palettes, task: true, resume: 0x1cefe }),
];

fn component(entry: u32) -> Component {
    COMPONENTS.into_iter().find(|&(e, _)| e == entry).unwrap().1
}

impl Component {
    /// Entrée : registres sauvés, 16 pas (la tâche `0x1ced4` blanchit d'abord les pages, son autre entrée
    /// `0x1cf12` non : `whiten`).
    fn debut(self, x: &mut Ctx, whiten: bool) -> Flow {
        let mut regs = Registres { d: x.c.d, a: [0; 7] };
        regs.a.copy_from_slice(&x.c.a[..7]);
        if whiten {
            white_out(x.b);
        }
        wr8(x.b, state::STEP, STEP_FULL);
        wr8(x.b, CURRENT_COMPONENT, 1);
        self.pas(x, regs)
    }

    fn pas(self, x: &mut Ctx, regs: Registres) -> Flow {
        component_step(x.b, self.target);
        let remainder = rd8(x.b, state::STEP).wrapping_sub(1);
        wr8(x.b, state::STEP, remainder);
        if remainder != 0 {
            let delay = rd16(x.b, state::DELAY);
            return attendre(x, delay, Etape::Component { fondu: self, regs });
        }
        wr8(x.b, state::PAGES, 0);
        wr8(x.b, CURRENT_COMPONENT, 0);
        x.c.d = regs.d;
        x.c.a[..7].copy_from_slice(&regs.a);
        if self.task {
            super::finish(x)
        } else {
            Flow::Done
        }
    }
}

/// `0x1ccc2` : fondu par composante vers le blanc (sous-programme).
pub fn to_white(x: &mut Ctx) -> Flow {
    component(0x1ccc2).debut(x, false)
}

/// `0x1ce30` : fondu par composante vers les couleurs.
pub fn to_colours(x: &mut Ctx) -> Flow {
    component(0x1ce30).debut(x, false)
}

/// `0x1ce9e` : fondu par composante vers les palettes.
pub fn to_palettes(x: &mut Ctx) -> Flow {
    component(0x1ce9e).debut(x, false)
}

pub const NATIVES: &[Native] = &[];
