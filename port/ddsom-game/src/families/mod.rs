//! Familles d'objets réécrites type par type : ESHL (tirs et attaques des ennemis), SET (objets
//! de décor) et SOBJ (objets spéciaux). Voir `docs/familles-eshl-set-sobj.md`.
//!
//! Chaque type est un *gestionnaire* : la fonction d'état que la boucle de dispatch de la classe
//! appelle pour chaque objet actif (`jsr (table + type * 4)`). Les gestionnaires sont des routines
//! natives enregistrées à l'adresse d'entrée du type.
//!
//! Fichiers : `cx` (contexte d'un gestionnaire et appels de sous-routines), `shell` (aides ESHL),
//! `eshl`, `eshl_b`, `eshl_c` (tirs, par tranches de types), `set`, `set_b`, `set_c` (décors),
//! `sobj` (objets spéciaux).

pub mod cx;
pub mod eshl;
pub mod eshl_b;
pub mod eshl_c;
pub mod set;
pub mod set_b;
pub mod set_c;
pub mod shell;
pub mod sobj;

use crate::Native;

/// Toutes les routines natives de ces familles.
pub fn natives() -> impl Iterator<Item = &'static Native> {
    [eshl::NATIVES, eshl_b::NATIVES, eshl_c::NATIVES, set::NATIVES, set_b::NATIVES, set_c::NATIVES, sobj::NATIVES]
        .into_iter()
        .flatten()
}
