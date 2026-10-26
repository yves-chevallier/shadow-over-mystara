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

use crate::modes::Flow;
use crate::scene::Ctx;
use crate::Native;

/// Ce qu'une tâche demande à l'ordonnanceur à la fin d'un pas.
#[derive(Clone, Copy)]
pub enum Attente {
    /// Dort ce nombre d'images (trap 3 : `jsr 0xa40`, D0 = nombre d'images).
    Images(u32),
    /// Reste prête : l'ordonnanceur la relance dès qu'il revient à elle (trap 4 : `jsr 0xa86`).
    Prete,
}

/// Cède la main : le noyau range l'attente et le contexte de la tâche dans la table ; elle reprendra dans la native
/// posée à l'adresse `reprise`, avec ses registres et sa pile actuels.
pub fn ceder(x: &mut Ctx, attente: Attente, reprise: u32) -> Flow {
    match attente {
        Attente::Images(n) => {
            x.c.d[0] = n;
            crate::kernel::attendre(x.c, x.b, reprise);
        }
        Attente::Prete => crate::kernel::ceder_prete(x.c, x.b, reprise),
    }
    Flow::Yield
}

/// Natives des tâches Rust.
pub fn natives() -> impl Iterator<Item = &'static Native> {
    partie::NATIVES
        .iter()
        .chain(gel::NATIVES)
        .chain(fondu::NATIVES)
        .chain(ecran_fixe::NATIVES)
        .chain(service::NATIVES)
        .chain(invite::NATIVES)
}
