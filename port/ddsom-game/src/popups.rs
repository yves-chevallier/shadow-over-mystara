//! Pop-ups (U1) : machine à états qui sort les textes flottants de leur file, un à la fois, et les dessine.
//!
//! `A5-0x6e80` : pop-up courant (4 mots : identifiant, deux paramètres, durée restante en images).
//! La file (`A5+0x20e`, entrées de 8 octets, lecture en `A5+0x20d`) est remplie par `creer_popup`
//! (0x19dce, voir `evenements.rs`). À chaque image (0x19e20) :
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

const VIDE: u16 = 0xffff;
/// Pop-up courant (4 mots) ; la durée restante est le dernier.
const COURANT: u32 = 0xffff_9180;
const DUREE: u32 = 6;
const ENTREES: u32 = 0x20e;
const LECTURE: u32 = 0x20d;
const ENTREE_TAILLE: u8 = 8;
/// Durée par défaut d'un pop-up sans second paramètre (images), et durée raccourcie.
const DUREE_DEFAUT: u16 = 0x60;
const DUREE_RACCOURCIE: u16 = 4;
/// Bit 0 : bulles réduites.
const REDUIT: u32 = 0x20;

/// Données de la ROM programme.
mod rom {
    /// Rectangles à effacer (colonne, ligne, largeur, hauteur), un par emplacement : grands, puis réduits.
    pub const EFFACEMENTS: [u32; 2] = [0x19f02, 0x19f12];
    /// Origine de la bulle dans la couche de texte, par emplacement (4 grands, puis 4 réduits) : lue
    /// comme une table relative au PC, donc dans le programme déchiffré.
    pub const ORIGINES: u32 = 0x19f6c;
    /// Déplacements des textes (par numéro) vers des chaînes d'indices de tuiles terminées par `/`.
    pub const TEXTES: u32 = 0x1a01a;
    /// Tuiles du bord bas du cadre (par reste de hauteur de 1 à 8 pixels).
    pub const BORDS: u32 = 0x1a25e;
}

const COUCHE_TEXTE: u32 = 0x912000;
const COLONNE: u32 = 0x80;
/// Fin de chaîne d'un texte.
const FIN_TEXTE: u8 = 0x2f;
/// Cases du cadre (tuile, attribut 0xd) : bord de la dernière colonne, bord plein, et base de la case d'angle.
const BORD_COURANT: u32 = 0x1318_000d;
const BORD_PLEIN: u32 = 0x1310_000d;
const BORD_COIN: u16 = 0x1310;
const ATTRIBUT_CADRE: u16 = 0xd;
const ATTRIBUT_TEXTE: u16 = 2;

fn reduit(b: &mut Cps2Bus, a5: u32) -> bool {
    rd8(b, a5 + REDUIT) & 1 != 0
}

/// 0x19ee0 : efface la bulle du pop-up `entree`.
fn effacer(b: &mut Cps2Bus, a5: u32, entree: u32) {
    let table = rom::EFFACEMENTS[reduit(b, a5) as usize];
    let case = table + 4 * rd8(b, entree) as u32;
    let (col, row, largeur, hauteur) = (rd8(b, case), rd8(b, case + 1), rd8(b, case + 2), rd8(b, case + 3));
    clear_cells(b, col, row, largeur, hauteur);
}

/// 0x19f24 : dessine la bulle du pop-up `entree`.
fn dessiner(b: &mut Cps2Bus, a5: u32, entree: u32) {
    let id = rd16(b, entree);
    let mut largeur = rd16(b, entree + 2);
    let mut hauteur = rd16(b, entree + 4);
    if (hauteur as i16) < 0 {
        // paramètre absent
        hauteur = 0;
    }
    let mut emplacement = (id >> 8) << 1;
    if largeur as i16 > 0xa8 {
        largeur >>= 1;
        hauteur >>= 1;
    }
    let petit = reduit(b, a5);
    if petit {
        emplacement += 8;
        largeur >>= 1;
        hauteur >>= 1;
    }
    let origine = prog16(b, rom::ORIGINES + emplacement as u32) as i16 as i32 as u32;
    let a4 = COUCHE_TEXTE.wrapping_add(origine);
    texte(b, a4, id, petit);
    cadre(b, a4, id, petit, largeur as i16, hauteur as i16);
}

/// Colonne du texte et du cadre : décalée d'une case quand le bit 9 de l'identifiant est posé.
fn decale(id: u16, petit: bool) -> bool {
    !petit && id & 0x200 != 0
}

/// 0x19fd2 : le texte, une tuile par case, en colonne.
fn texte(b: &mut Cps2Bus, a4: u32, id: u16, petit: bool) {
    let mut case = a4 + 0x100 + if decale(id, petit) { 4 } else { 0 };
    let depart = prog16(b, rom::TEXTES + 2 * (id & 0xff) as u32) as i16 as i32 as u32;
    let mut s = rom::TEXTES.wrapping_add(depart);
    loop {
        let tuile = rd8(b, s);
        s += 1;
        if tuile == FIN_TEXTE {
            return;
        }
        wr16(b, case, tuile as u16);
        wr16(b, case + 2, ATTRIBUT_TEXTE);
        case += COLONNE;
    }
}

/// 0x1a20e : le cadre sous le texte. `largeur` et `hauteur` en pixels : des colonnes entières de bord, puis
/// la colonne du reste (une tuile choisie par le nombre de pixels restants).
fn cadre(b: &mut Cps2Bus, a4: u32, id: u16, petit: bool, mut d1: i16, mut d2: i16) {
    let mut case = a4 + 0x104 - if decale(id, petit) { 4 } else { 0 };
    loop {
        d1 = d1.wrapping_sub(8);
        d2 = d2.wrapping_sub(8);
        if d2 < 0 {
            break;
        }
        wr32(b, case, BORD_COURANT);
        case += COLONNE;
    }
    loop {
        d2 = d2.wrapping_add(8);
        d1 = d1.wrapping_add(8);
        if d1 == 0 {
            return;
        }
        if d1 <= 8 {
            let bord = prog16(b, rom::BORDS.wrapping_add_signed(2 * d1 as i32));
            wr16(b, case, bord.wrapping_add(d2 as u16));
            wr16(b, case + 2, ATTRIBUT_CADRE);
            return;
        }
        wr16(b, case, BORD_COIN.wrapping_add(d2 as u16));
        wr16(b, case + 2, ATTRIBUT_CADRE);
        case += COLONNE;
        d1 = d1.wrapping_sub(8);
        d2 = -8;
        loop {
            d1 = d1.wrapping_sub(8);
            if d1 < 0 {
                break;
            }
            wr32(b, case, BORD_PLEIN);
            case += COLONNE;
        }
    }
}

/// Fait du prochain pop-up de la file le pop-up courant (s'il y en a un) et lance son dessin. Rend l'index de
/// lecture lu.
fn suivant(b: &mut Cps2Bus, courant: u32) -> u8 {
    let index = rd8(b, A5 + LECTURE);
    let case = A5 + ENTREES + index as u32;
    if rd16(b, case) == VIDE {
        return index;
    }
    for decalage in [0, 2, 4, 6] {
        let mot = rd16(b, case + decalage);
        wr16(b, courant + decalage, mot);
        wr16(b, case + decalage, VIDE);
    }
    if rd16(b, courant + 4) == 0 {
        wr16(b, courant + DUREE, DUREE_DEFAUT);
    }
    dessiner(b, A5, courant);
    wr8(b, A5 + LECTURE, index.wrapping_add(ENTREE_TAILLE));
    index
}

/// 0x19e20 : un pas de la machine à états des pop-ups. Rend l'index de lecture de la file s'il a été lu (le D0.b de
/// l'original). Cœur.
pub fn animer(b: &mut Cps2Bus) -> Option<u8> {
    let courant = A5.wrapping_add(COURANT);
    let duree = rd16(b, courant + DUREE);
    if duree != 0 {
        let duree = duree - 1;
        wr16(b, courant + DUREE, duree);
        if duree != 0 {
            // le pop-up courant continue ; s'il y en a un autre en attente, on abrège
            if duree < DUREE_RACCOURCIE {
                return None;
            }
            let index = rd8(b, A5 + LECTURE);
            if rd16(b, A5 + ENTREES + index as u32) != VIDE {
                wr16(b, courant + DUREE, DUREE_RACCOURCIE);
            }
            return Some(index);
        }
        effacer(b, A5, courant);
    }
    Some(suivant(b, courant))
}

/// `animer` avec les registres de l'original : A0 = le pop-up courant, A2 = la file, D0 = l'index lu.
/// `animer` avec les registres de l'original (A0 = le pop-up courant, A2 = la file, D0 = l'index lu).
pub fn animer_68k(c: &mut Cpu, b: &mut Cps2Bus) {
    c.a[0] = A5.wrapping_add(COURANT);
    c.a[2] = A5 + ENTREES;
    if let Some(index) = animer(b) {
        c.d[0] = index as u32;
    }
}

/// 0x19eb0 : efface les bulles de tous les emplacements (4, ou 3 réduits), du dernier au premier. Rend la table
/// des effacements lue (l'A2 de l'original). Cœur.
pub fn effacer_tout(b: &mut Cps2Bus) -> u32 {
    let petit = reduit(b, A5);
    let table = rom::EFFACEMENTS[petit as usize];
    let dernier = if petit { 2 } else { 3 };
    for emplacement in (0..=dernier).rev() {
        let case = table + 4 * emplacement;
        let (col, row, largeur, hauteur) = (rd8(b, case), rd8(b, case + 1), rd8(b, case + 2), rd8(b, case + 3));
        clear_cells(b, col, row, largeur, hauteur);
    }
    table
}

pub const ANIMER: Native = Native {
    entry: 0x19e20,
    name: "popups_animer",
    run: crate::Corps::Processeur(|c, b| {
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
    fn decalage_des_bulles() {
        assert!(decale(0x0200, false));
        assert!(!decale(0x0200, true));
        assert!(!decale(0x0100, false));
    }
}
