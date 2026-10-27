//! Tâches de l'ordonnanceur écrites en Rust (voir `docs/noyau.md`, « Tâches Rust »).
//!
//! Une tâche Rust est une machine à états dont chaque *pas* s'exécute entre deux cessions de la main. Le pas est une
//! native posée à l'adresse que le code d'origine reprend après la cession (l'adresse qui suit le `jsr 0xa40` ou
//! `0xa86`) ; il rend une [`Attente`], que [`ceder`] remet au noyau. La table des tâches ([`table`]) range l'état, le
//! compte, l'adresse de reprise, les registres et les appels en cours de la tâche ([`table::Cadre`]) : sa pile 68000
//! est vide à chaque cession. Une tâche Rust et une tâche en 68000 cohabitent, et l'ordonnanceur ne les distingue pas.
//!
//! Tâches : [`partie`] (la partie, `0x1c45a`), [`gel`] (avance rapide, `0xee080`), [`fondu`] (fondus de palette,
//! tâches et sous-programmes qui cèdent la main), [`ecran_fixe`] (tableau des scores de l'attract, `0x1835e`),
//! [`service`] (menu TEST, `0x29162`) et [`invite`] (texte « INSERT COIN » clignotant, `0xe03fe`).

pub mod ecran_fixe;
pub mod fondu;
pub mod gel;
pub mod invite;
pub mod partie;
pub mod service;
pub mod table;

use cps2::Cps2Bus;
use m68k::Cpu;

use crate::modes::Flow;
use crate::scene::Ctx;
use crate::Native;
use table::Suite;

/// Ce qu'une tâche demande à l'ordonnanceur à la fin d'un pas.
#[derive(Clone, Copy)]
pub enum Attente {
    /// Dort ce nombre d'images (trap 3 : `jsr 0xa40`, D0 = nombre d'images).
    Images(u32),
    /// Reste prête : l'ordonnanceur la relance dès qu'il revient à elle (trap 4 : `jsr 0xa86`).
    Prete,
}

/// Cède la main : le noyau range l'attente et le contexte de la tâche dans la table ; elle reprendra à `reprise`
/// (une étape Rust, ou la native posée à une adresse), avec ses registres et sa pile actuels.
pub fn ceder(x: &mut Ctx, attente: Attente, reprise: impl Into<Suite>) -> Flow {
    let reprise = reprise.into();
    match attente {
        Attente::Images(n) => {
            x.c.d[0] = n;
            crate::kernel::attendre(x.c, x.b, reprise);
        }
        Attente::Prete => crate::kernel::ceder_prete(x.c, x.b, reprise),
    }
    Flow::Yield
}

/// `jsr 0xb64` : cède une image (D0.w = 1, le mot haut de D0 est gardé) ; la tâche reprendra à `reprise`.
pub fn une_image(x: &mut Ctx, reprise: impl Into<Suite>) -> Flow {
    crate::mem::set_low_word(&mut x.c.d[0], 1);
    crate::kernel::attendre(x.c, x.b, reprise);
    Flow::Yield
}

/// `jsr 0xa40` : cède D0.b images (D0 tel quel) ; la tâche reprendra à `reprise`.
pub fn attendre(x: &mut Ctx, reprise: impl Into<Suite>) -> Flow {
    crate::kernel::attendre(x.c, x.b, reprise);
    Flow::Yield
}

/// `jmp 0xa40` : cède D0.b images ; la tâche reprendra à la suite de l'appelant (l'appel en cours).
pub fn attendre_puis_revenir(x: &mut Ctx) -> Flow {
    let suite = table::depiler_retour(x.c, x.b);
    attendre(x, suite)
}

/// `jmp 0x9b6` : crée la tâche D0 (décalage) à l'entrée A0, puis revient à l'appelant.
pub fn creer_puis_revenir(x: &mut Ctx) -> Flow {
    crate::kernel::creer_tache(x.b, x.c.d[0], x.c.a[0]);
    Flow::Done
}

/// `jmp 0xae4` : la tâche courante repart de zéro à l'entrée A0.
pub fn recommencer(x: &mut Ctx) -> Flow {
    crate::kernel::recommencer(x.c, x.b);
    Flow::Yield
}

/// `jsr 0x9f8` : la tâche courante se termine.
pub fn terminer(x: &mut Ctx) -> Flow {
    crate::kernel::terminer(x.c, x.b);
    Flow::Yield
}

/// Déclare les étapes d'une tâche : une variante par point de reprise, avec l'adresse que l'original reprend
/// (`adresse`, gardée pour le diagnostic) et le pas qui s'y exécute (`pas`).
#[macro_export]
macro_rules! etapes {
    ($(#[$m:meta])* $vis:vis enum $nom:ident { $($(#[$vm:meta])* $var:ident = $adr:expr => $pas:expr,)* }) => {
        $(#[$m])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        $vis enum $nom { $($(#[$vm])* $var,)* }
        impl $nom {
            /// Adresse que l'original reprend à cette étape.
            pub const fn adresse(self) -> u32 {
                match self { $(Self::$var => $adr,)* }
            }
            /// Le pas de l'étape.
            pub fn pas(self, x: &mut $crate::scene::Ctx) -> $crate::modes::Flow {
                match self {
                    $(Self::$var => {
                        let pas: fn(&mut $crate::scene::Ctx) -> $crate::modes::Flow = $pas;
                        pas(x)
                    })*
                }
            }
        }
    };
}

/// Déclare l'étape de chaque sorte de tâche.
macro_rules! etapes_des_taches {
    ($($(#[$vm:meta])* $var:ident($t:ty),)*) => {
        /// Point de reprise d'une tâche Rust : l'étape de sa machine à états.
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Etape { $($(#[$vm])* $var($t),)* }
        impl Etape {
            /// Adresse que l'original reprend à cette étape (diagnostic, comparaison avec la table FF0064).
            pub fn adresse(self) -> u32 {
                match self { $(Etape::$var(e) => e.adresse(),)* }
            }
            fn pas(self, x: &mut Ctx) -> Flow {
                match self { $(Etape::$var(e) => e.pas(x),)* }
            }
        }
        $(
            impl From<$t> for Etape {
                fn from(e: $t) -> Self { Etape::$var(e) }
            }
            impl From<$t> for Suite {
                fn from(e: $t) -> Self { Suite::Etape(Etape::$var(e)) }
            }
        )*
    };
}

etapes_des_taches! {
    Partie(partie::Etape),
    Gel(gel::Etape),
    Invite(invite::Etape),
    Scores(ecran_fixe::Etape),
    Service(service::Etape),
    Interface(crate::hud::Etape),
    Credits(crate::tache_credits::Etape),
    Fondu(fondu::Etape),
    EcranNoir(crate::video::ecran_noir::Etape),
    // machine à modes (tâche 0 hors partie)
    Boot(crate::modes::boot::Etape),
    Attract(crate::modes::attract::Etape),
    Title(crate::modes::title::Etape),
    Select(crate::modes::select::Etape),
    Ranking(crate::modes::ranking::Etape),
    Demo(crate::modes::demo::Etape),
    Ending(crate::modes::ending::Etape),
    Gameover(crate::modes::gameover::Etape),
    // séquences de scène (tâche de partie)
    Exit(crate::scene::exit::Etape),
    Blackout(crate::scene::blackout::Etape),
    Transition(crate::scene::transition::Etape),
    TransitionEarly(crate::scene::transition::early::Etape),
    TransitionLate(crate::scene::transition::late::Etape),
    Bonus(crate::scene::bonus::Etape),
    Shop(crate::scene::shop::Etape),
    Characters(crate::scene::characters::Etape),
    Tally(crate::scene::tally::Etape),
    // gestionnaires d'objets qui appellent un sous-programme qui cède (portes SET : fondu au noir)
    PorteSet(crate::families::set::fondu_de_sortie::Etape),
}

/// Exécute l'étape `etape` de la tâche courante ; si elle finit (`Flow::Done`, retour de sous-programme), la suite de
/// l'appel en cours ([`revenir`]).
pub fn executer(etape: Etape, c: &mut Cpu, b: &mut Cps2Bus) {
    // adresse d'origine, pour les diagnostics (et la boucle d'image, si le pas rend la main sans céder)
    c.pc = etape.adresse();
    let mut x = Ctx::new(c, b);
    crate::modes::c_a5(&mut x);
    if etape.pas(&mut x) == Flow::Done {
        revenir(x.c, x.b);
    }
}

/// Retour d'un sous-programme (`rts`) : la suite de l'appel en cours, étape Rust exécutée aussitôt, ou adresse posée
/// au PC.
pub fn revenir(c: &mut Cpu, b: &mut Cps2Bus) {
    match table::depiler_retour(c, b) {
        Suite::Adresse(a) => c.pc = a,
        Suite::Etape(e) => executer(e, c, b),
    }
}

/// Première étape d'une tâche Rust neuve, d'après son entrée (sinon la native posée à l'entrée).
pub fn entree(entree: u32) -> Option<Etape> {
    [
        Etape::from(gel::Etape::Entree),
        partie::Etape::Entree.into(),
        invite::Etape::Entree.into(),
        ecran_fixe::Etape::Entree.into(),
        service::Etape::Entree.into(),
        crate::hud::Etape::Entree.into(),
        crate::tache_credits::Etape::Entree.into(),
        crate::scene::shop::Etape::Portraits.into(),
    ]
    .into_iter()
    .find(|e| e.adresse() == entree)
    .or_else(|| fondu::tache_depuis_entree(entree).map(Etape::Fondu))
}

/// Natives des tâches Rust.
pub fn natives() -> impl Iterator<Item = &'static Native> {
    partie::NATIVES
        .iter()
        .chain(fondu::NATIVES)
        .chain(invite::NATIVES)
}
