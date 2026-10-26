//! Tirs (PSHL) et effets (PEFF) des joueurs : un gestionnaire par type, voir `docs/familles/joueur.md`.

use crate::Native;

pub mod peff;
pub mod peff_b;
pub mod peff_boutique;
pub mod peff_c;
pub mod peff_d;
pub mod peff_e;
pub mod peff_f;
pub mod peff_g;
pub mod peff_h;
pub mod peff_i;
pub mod peff_j;
pub mod peff_k;
pub mod peff_l;
pub mod peff_m;
pub mod peff_n;
pub mod peff_o;
pub mod peff_ring;
pub mod pshl;
pub mod pshl_b;
pub mod pshl_c;
pub mod pshl_d;
pub mod pshl_e;
pub mod pshell;

/// Toutes les routines natives des tirs et des effets des joueurs.
pub fn natives() -> impl Iterator<Item = &'static Native> {
    std::iter::empty()
        .chain(peff::NATIVES)
        .chain(peff_b::NATIVES)
        .chain(peff_boutique::NATIVES)
        .chain(&peff_boutique::TALK)
        .chain(peff_c::NATIVES)
        .chain(peff_d::NATIVES)
        .chain(peff_e::NATIVES)
        .chain(peff_f::NATIVES)
        .chain(peff_g::NATIVES)
        .chain(peff_h::NATIVES)
        .chain(peff_i::NATIVES)
        .chain(peff_j::NATIVES)
        .chain(peff_k::NATIVES)
        .chain(peff_l::NATIVES)
        .chain(peff_m::NATIVES)
        .chain(peff_n::NATIVES)
        .chain(peff_o::NATIVES)
        .chain(peff_ring::NATIVES)
        .chain(pshl::NATIVES)
        .chain(pshl_b::NATIVES)
        .chain(pshl_c::NATIVES)
        .chain(pshl_d::NATIVES)
        .chain(pshl_e::NATIVES)
}
