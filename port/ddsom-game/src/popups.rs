//! Pop-ups (U1) : machine à états qui sort les textes flottants de leur file, un à la fois, et les dessine.
//!
//! `A5-0x6e80` : pop-up courant (4 mots : identifiant, deux paramètres, durée restante en images).
//! La file (`A5+0x20e`, entrées de 8 octets, lecture en `A5+0x20d`) est remplie par `create_popup`
//! (0x19dce, voir `events.rs`). À chaque image (0x19e20) :
//!
//! * pas de pop-up courant (durée nulle) : le suivant, s'il y en a un, devient courant (sa case est
//!   libérée), son dessin est lancé (0x19f24) ; sans second paramètre sa durée est de 0x60 images ;
//! * pop-up courant : sa durée diminue ; à zéro il est effacé (0x19ee0) puis remplacé par le suivant ;
//!   si un autre attend et que la durée restante est d'au moins 4 images, elle est ramenée à 4.
//!
//! ## Dessin
//!
//! L'identifiant (mot) donne dans son octet haut l'emplacement de la bulle sur la couche de texte
//! (bit 1 : décalage d'une case) et dans son octet bas le numéro du texte. Une bulle est un cadre
//! de tuiles 8x8 (largeur et hauteur en pixels : 2e et 3e mots) avec le texte en haut ; en jeu à deux
//! joueurs (`A5+0x20` bit 0) la bulle est réduite de moitié.

use cps2::Cps2Bus;
use m68k::Cpu;

use crate::mem::{prog16, rd16, rd8, rts, wr16, wr32, wr8, A5};
use crate::text::clear_cells;
use crate::Native;

const EMPTY: u16 = 0xffff;
/// Pop-up courant (4 mots) ; la durée restante est le dernier.
const CURRENT: u32 = 0xffff_9180;
const DURATION: u32 = 6;
const INPUTS: u32 = 0x20e;
const READ: u32 = 0x20d;
const ENTRY_SIZE: u8 = 8;
/// Durée par défaut d'un pop-up sans second paramètre (images), et durée raccourcie.
const DEFAULT_DURATION: u16 = 0x60;
const SHORT_DURATION: u16 = 4;
/// Bit 0 : bulles réduites.
const REDUCED: u32 = 0x20;

/// Données de la ROM programme.
mod rom {
    /// Rectangles à effacer (colonne, ligne, largeur, hauteur), un par emplacement : grands, puis réduits.
    pub const CLEARS: [u32; 2] = [0x19f02, 0x19f12];
    /// Origine de la bulle dans la couche de texte, par emplacement (4 grands, puis 4 réduits) : lue
    /// comme une table relative au PC, donc dans le programme déchiffré.
    pub const ORIGINALS: u32 = 0x19f6c;
    /// Déplacements des textes (par numéro) vers des chaînes d'indices de tuiles terminées par `/`.
    pub const TEXTS: u32 = 0x1a01a;
    /// Tuiles du bord bas du cadre (par reste de hauteur de 1 à 8 pixels).
    pub const EDGES: u32 = 0x1a25e;
}

const TEXT_LAYER: u32 = 0x912000;
const COLUMN: u32 = 0x80;
/// Fin de chaîne d'un texte.
const TEXT_END: u8 = 0x2f;
/// Cases du cadre (tuile, attribut 0xd) : bord de la dernière colonne, bord plein, et base de la case d'angle.
const EDGE_CURRENT: u32 = 0x1318_000d;
const EDGE_FULL: u32 = 0x1310_000d;
const EDGE_COIN: u16 = 0x1310;
const ATTR_FRAME: u16 = 0xd;
const ATTR_TEXT: u16 = 2;

fn reduced(b: &mut Cps2Bus, a5: u32) -> bool {
    rd8(b, a5 + REDUCED) & 1 != 0
}

/// 0x19ee0 : efface la bulle du pop-up `entry`.
fn clear(b: &mut Cps2Bus, a5: u32, entry: u32) {
    let table = rom::CLEARS[reduced(b, a5) as usize];
    let case = table + 4 * rd8(b, entry) as u32;
    let (col, row, width, height) = (rd8(b, case), rd8(b, case + 1), rd8(b, case + 2), rd8(b, case + 3));
    clear_cells(b, col, row, width, height);
}

/// 0x19f24 : dessine la bulle du pop-up `entry`.
fn draw(b: &mut Cps2Bus, a5: u32, entry: u32) {
    let id = rd16(b, entry);
    let mut width = rd16(b, entry + 2);
    let mut height = rd16(b, entry + 4);
    if (height as i16) < 0 {
        // paramètre absent
        height = 0;
    }
    let mut slot = (id >> 8) << 1;
    if width as i16 > 0xa8 {
        width >>= 1;
        height >>= 1;
    }
    let small = reduced(b, a5);
    if small {
        slot += 8;
        width >>= 1;
        height >>= 1;
    }
    let original = prog16(b, rom::ORIGINALS + slot as u32) as i16 as i32 as u32;
    let a4 = TEXT_LAYER.wrapping_add(original);
    text(b, a4, id, small);
    frame(b, a4, id, small, width as i16, height as i16);
}

/// Colonne du texte et du cadre : décalée d'une case quand le bit 9 de l'identifiant est posé.
fn shifted(id: u16, small: bool) -> bool {
    !small && id & 0x200 != 0
}

/// 0x19fd2 : le texte, une tuile par case, en colonne.
fn text(b: &mut Cps2Bus, a4: u32, id: u16, small: bool) {
    let mut case = a4 + 0x100 + if shifted(id, small) { 4 } else { 0 };
    let start = prog16(b, rom::TEXTS + 2 * (id & 0xff) as u32) as i16 as i32 as u32;
    let mut s = rom::TEXTS.wrapping_add(start);
    loop {
        let tile = rd8(b, s);
        s += 1;
        if tile == TEXT_END {
            return;
        }
        wr16(b, case, tile as u16);
        wr16(b, case + 2, ATTR_TEXT);
        case += COLUMN;
    }
}

/// 0x1a20e : le cadre sous le texte. `width` et `height` en pixels : des colonnes entières de bord, puis
/// la colonne du reste (une tuile choisie par le nombre de pixels restants).
fn frame(b: &mut Cps2Bus, a4: u32, id: u16, small: bool, mut d1: i16, mut d2: i16) {
    let mut case = a4 + 0x104 - if shifted(id, small) { 4 } else { 0 };
    loop {
        d1 = d1.wrapping_sub(8);
        d2 = d2.wrapping_sub(8);
        if d2 < 0 {
            break;
        }
        wr32(b, case, EDGE_CURRENT);
        case += COLUMN;
    }
    loop {
        d2 = d2.wrapping_add(8);
        d1 = d1.wrapping_add(8);
        if d1 == 0 {
            return;
        }
        if d1 <= 8 {
            let edge = prog16(b, rom::EDGES.wrapping_add_signed(2 * d1 as i32));
            wr16(b, case, edge.wrapping_add(d2 as u16));
            wr16(b, case + 2, ATTR_FRAME);
            return;
        }
        wr16(b, case, EDGE_COIN.wrapping_add(d2 as u16));
        wr16(b, case + 2, ATTR_FRAME);
        case += COLUMN;
        d1 = d1.wrapping_sub(8);
        d2 = -8;
        loop {
            d1 = d1.wrapping_sub(8);
            if d1 < 0 {
                break;
            }
            wr32(b, case, EDGE_FULL);
            case += COLUMN;
        }
    }
}

/// Fait du prochain pop-up de la file le pop-up courant (s'il y en a un) et lance son dessin. Rend l'index de
/// lecture lu.
fn next(b: &mut Cps2Bus, current: u32) -> u8 {
    let index = rd8(b, A5 + READ);
    let case = A5 + INPUTS + index as u32;
    if rd16(b, case) == EMPTY {
        return index;
    }
    for offset in [0, 2, 4, 6] {
        let word = rd16(b, case + offset);
        wr16(b, current + offset, word);
        wr16(b, case + offset, EMPTY);
    }
    if rd16(b, current + 4) == 0 {
        wr16(b, current + DURATION, DEFAULT_DURATION);
    }
    draw(b, A5, current);
    wr8(b, A5 + READ, index.wrapping_add(ENTRY_SIZE));
    index
}

/// 0x19e20 : un pas de la machine à états des pop-ups. Rend l'index de lecture de la file s'il a été lu (le D0.b de
/// l'original). Cœur.
pub fn animer(b: &mut Cps2Bus) -> Option<u8> {
    let current = A5.wrapping_add(CURRENT);
    let duration = rd16(b, current + DURATION);
    if duration != 0 {
        let duration = duration - 1;
        wr16(b, current + DURATION, duration);
        if duration != 0 {
            // le pop-up courant continue ; s'il y en a un autre en attente, on abrège
            if duration < SHORT_DURATION {
                return None;
            }
            let index = rd8(b, A5 + READ);
            if rd16(b, A5 + INPUTS + index as u32) != EMPTY {
                wr16(b, current + DURATION, SHORT_DURATION);
            }
            return Some(index);
        }
        clear(b, A5, current);
    }
    Some(next(b, current))
}

/// `animer` avec les registres de l'original : A0 = le pop-up courant, A2 = la file, D0 = l'index lu.
/// `animer` avec les registres de l'original (A0 = le pop-up courant, A2 = la file, D0 = l'index lu).
pub fn animer_68k(c: &mut Cpu, b: &mut Cps2Bus) {
    c.a[0] = A5.wrapping_add(CURRENT);
    c.a[2] = A5 + INPUTS;
    if let Some(index) = animer(b) {
        c.d[0] = index as u32;
    }
}

/// 0x19eb0 : efface les bulles de tous les emplacements (4, ou 3 réduits), du dernier au premier. Rend la table
/// des effacements lue (l'A2 de l'original). Cœur.
pub fn clear_all(b: &mut Cps2Bus) -> u32 {
    let small = reduced(b, A5);
    let table = rom::CLEARS[small as usize];
    let last = if small { 2 } else { 3 };
    for slot in (0..=last).rev() {
        let case = table + 4 * slot;
        let (col, row, width, height) = (rd8(b, case), rd8(b, case + 1), rd8(b, case + 2), rd8(b, case + 3));
        clear_cells(b, col, row, width, height);
    }
    table
}

pub const ANIMER: Native = Native {
    entry: 0x19e20,
    name: "popups_animer",
    run: crate::Body::Processeur(|c, b| {
        animer_68k(c, b);
        rts(c, b);
    }),
    touches_sound: false,
    outputs: 0,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bubble_offset() {
        assert!(shifted(0x0200, false));
        assert!(!shifted(0x0200, true));
        assert!(!shifted(0x0100, false));
    }
}
