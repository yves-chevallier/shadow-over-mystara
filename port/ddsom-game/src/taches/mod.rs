//! Tâches de l'ordonnanceur écrites en Rust (voir `docs/noyau.md`, « Tâches Rust »).
//!
//! Une tâche Rust est une machine à états dont chaque *pas* s'exécute entre deux cessions de la main. Le pas est une
//! native posée à l'adresse que le code d'origine reprend après la cession (l'adresse empilée par le `jsr 0xa40` ou
//! `0xa86`) ; il rend une [`Attente`], que [`ceder`] traduit en cession à l'ordonnanceur. La table FF0064 et la pile
//! de la tâche restent celles de l'original (état, compte, PC `0xa4c`/`0xa8c`, USP, 15 registres sauvés, adresse de
//! reprise) : une tâche Rust et une tâche en 68000 cohabitent, et l'ordonnanceur ne les distingue pas.
//!
//! Tâches : [`partie`] (la partie, `0x1c45a`), [`gel`] (avance rapide, `0xee080`) et [`service`] (menu TEST,
//! `0x29162`).

pub mod gel;
pub mod partie;
pub mod service;

use crate::modes::{Flow, Sequence};
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

/// Cède la main ; la tâche reprendra dans la native posée à l'adresse `reprise`.
pub fn ceder(x: &mut Ctx, attente: Attente, reprise: u32) -> Flow {
    match attente {
        Attente::Images(n) => x.wait(n, reprise),
        Attente::Prete => x.tail(0xa86, reprise),
    }
}

/// Natives des tâches Rust.
pub fn natives() -> impl Iterator<Item = &'static Native> {
    partie::NATIVES.iter().chain(gel::NATIVES).chain(service::NATIVES)
}
