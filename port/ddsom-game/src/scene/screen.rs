//! Petites routines des écrans de fin de niveau : effacements de la couche de caractères, copies de pages de
//! palettes, remises à zéro de blocs d'état (W3).

use cps2::Cps2Bus;

use super::tiles::{BLANK, COLUMN};
use crate::mem::{rd32, wr32};
use crate::Native;

/// Rectangle de cases vides : `columns` colonnes de `rows` cases à partir de `at`.
pub fn blank_rect(b: &mut Cps2Bus, at: u32, columns: u32, rows: u32) {
    for col in 0..columns {
        for row in 0..rows {
            wr32(b, at + col * COLUMN + 4 * row, BLANK);
        }
    }
}

/// 0xf683c : efface la zone de texte des écrans de fin (48 colonnes de 22 cases à partir de la ligne 5).
/// L'original laisse D1.w = -1, relu par l'écran de bonus comme réglage (écrit en clair dans `bonus.rs`).
pub fn clear_text(b: &mut Cps2Bus) {
    const AT: u32 = 0x912014;
    blank_rect(b, AT, 48, 22);
}

/// 0xf688c : efface le cadre de l'écran de bonus (40 colonnes de 21 cases à partir de la ligne 6, colonne 5).
pub fn clear_bonus_frame(b: &mut Cps2Bus) {
    blank_rect(b, 0x912298, 40, 21);
}

/// 0xf617e : copie d'une page de palettes (0x400 octets).
pub fn copy_page(b: &mut Cps2Bus, from: u32, to: u32) {
    for i in (0..0x400).step_by(4) {
        let v = rd32(b, from + i);
        wr32(b, to + i, v);
    }
}

/// 0xfa240 : met à zéro 0x800 octets de la couche 0x920000.
pub fn clear_920000(b: &mut Cps2Bus) {
    for i in (0..0x800).step_by(4) {
        wr32(b, 0x920000 + i, 0);
    }
}

/// Met à zéro `n` longs à partir de `at`.
pub fn clear_longs(b: &mut Cps2Bus, at: u32, n: u32) {
    for k in 0..n {
        wr32(b, at + 4 * k, 0);
    }
}

/// Blocs d'état remis à zéro par les routines d'initialisation.
pub mod blocks {
    use crate::mem::A5;
    /// Bloc de la sortie de scène (phase 16), 8 longs (0xf3130).
    pub const EXIT: u32 = A5 - 0x55ca;
    /// Bloc de la boutique, 5 longs (0xf7916).
    pub const SHOP: u32 = A5 - 0x5560;
    /// Variables des scripts de transition (état partagé 0xf7ecc), 4 longs (0xf7ee0).
    pub const TRANSITION: u32 = A5 - 0x5520;
    /// Bloc de l'écran de classement, 4 longs (0xfc562).
    pub const RANKING: u32 = A5 - 0x54f0;
}

macro_rules! clear_native {
    ($name:ident, $entry:expr, $label:expr, $at:expr, $n:expr) => {
        pub const $name: Native = Native {
            entry: $entry,
            name: $label,
            run: crate::Corps::Memoire(|b| clear_longs(b, $at, $n)),
            touches_sound: false,
            outputs: 0,
        };
    };
}

/// 0xf3130 : plus aucun appel par adresse (l'entrée de scène appelle `clear_longs`) ; A4 et D6 laissés par la boucle
/// d'origine ne sont relus par personne (code-mort.md).
pub const RESET_EXIT: Native = Native {
    entry: 0xf3130,
    name: "scene_exit_reset",
    run: crate::Corps::Memoire(|b| clear_longs(b, blocks::EXIT, 8)),
    touches_sound: false,
    outputs: 0,
};
clear_native!(RESET_SHOP, 0xf7916, "scene_shop_reset", blocks::SHOP, 5);
pub const RESET_TRANSITION: Native = Native {
    entry: 0xf7ee0,
    name: "scene_transition_reset",
    run: crate::Corps::Memoire(|b| clear_longs(b, blocks::TRANSITION, 4)),
    touches_sound: false,
    outputs: 0,
};
clear_native!(RESET_RANKING, 0xfc562, "ranking_reset", blocks::RANKING, 4);

pub const CLEAR_TEXT: Native = Native {
    entry: 0xf683c,
    name: "scene_clear_text",
    run: crate::Corps::Memoire(clear_text),
    touches_sound: false,
    outputs: 0,
};

/// Plus aucun appel par adresse vers ces cinq natives : registres laissés par l'original non reproduits.
pub const NATIVES: &[Native] = &[RESET_EXIT, RESET_SHOP, RESET_TRANSITION, RESET_RANKING, CLEAR_TEXT];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mem::A5;

    #[test]
    fn adresses_des_blocs() {
        assert_eq!(blocks::EXIT, 0xff2a36);
        assert_eq!(blocks::RANKING, 0xff2b10);
        assert_eq!(A5 - 0x5520, 0xff2ae0);
    }
}
