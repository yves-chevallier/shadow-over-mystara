//! Couche de caractères (cases 8x8, `0x912000` pour la partie visible) vue par les écrans de fin de niveau (décompte, bonus).
//!
//! Une case est un long (tuile, attributs) ; la couche est rangée par colonnes : la case suivante d'une colonne
//! est 4 octets plus loin, la colonne suivante 0x80 octets plus loin. Les chiffres et les lettres de ces écrans
//! font deux cases de haut (la tuile du bas est celle du haut + 16), les grands chiffres deux de large en plus.

use cps2::Cps2Bus;

use crate::mem::{prog16, rd16, rd8, wr16, wr32};

pub const LAYER: u32 = 0x912000;
pub const COLUMN: u32 = 0x80;
/// Case vide : tuile 0x20 (espace), attributs nuls.
pub const BLANK: u32 = 0x0020_0000;

/// Premières tuiles des chiffres 0 à 7 et 8, 9 (deux rangées de 16 tuiles plus loin).
const DIGIT_0: u16 = 472;
const DIGIT_8: u16 = 496;
/// Deux-points d'une durée, et seconde (signe ″) du compte de secondes.
pub const COLON: u16 = 766;
pub const SECONDS: u16 = 767;
/// Tuiles des grands chiffres (mots, table lue relative au PC).
const BIG_DIGITS: u32 = 0xf6e74;

/// Une case de deux de haut : `tile` en haut, `tile + 16` en bas.
pub fn tall(b: &mut Cps2Bus, at: u32, tile: u16, attr: u16) {
    wr16(b, at, tile);
    wr16(b, at + 2, attr);
    wr16(b, at + 4, tile.wrapping_add(16));
    wr16(b, at + 6, attr);
}

/// Deux cases vides l'une sous l'autre.
pub fn blank(b: &mut Cps2Bus, at: u32) {
    wr32(b, at, BLANK);
    wr32(b, at + 4, BLANK);
}

/// `n` colonnes de deux cases vides à partir de `at` ; rend la colonne suivante.
pub fn blank_columns(b: &mut Cps2Bus, at: u32, n: u32) -> u32 {
    for k in 0..n {
        blank(b, at + k * COLUMN);
    }
    at + n * COLUMN
}

/// Un chiffre (quartet bas de `d`) de deux cases de haut (0x0f6d94).
pub fn digit(b: &mut Cps2Bus, at: u32, d: u8, attr: u16) {
    let d = (d & 15) as u16;
    let tile = if DIGIT_0 + d < 480 { DIGIT_0 + d } else { DIGIT_8 - 8 + d };
    tall(b, at, tile, attr);
}

/// Écriture d'un nombre chiffre par chiffre, une colonne par chiffre ; tant qu'aucun chiffre non nul n'est
/// écrit (`lead` faux), les zéros de tête sont des cases vides (0x0f6b72).
pub struct Number {
    pub at: u32,
    pub lead: bool,
    pub attr: u16,
}

impl Number {
    pub fn new(at: u32, attr: u16) -> Self {
        Number { at, lead: false, attr }
    }
    pub fn digit(&mut self, b: &mut Cps2Bus, d: u8) {
        let d = d & 15;
        if d == 0 && !self.lead {
            blank(b, self.at);
        } else {
            self.lead = true;
            digit(b, self.at, d, self.attr);
        }
        self.at += COLUMN;
    }
    /// Les deux chiffres d'un octet BCD (0x0f6b66).
    pub fn byte(&mut self, b: &mut Cps2Bus, v: u8) {
        self.digit(b, v >> 4);
        self.digit(b, v);
    }
    /// Dernier chiffre, toujours écrit.
    pub fn last(&mut self, b: &mut Cps2Bus, d: u8) {
        self.lead = true;
        self.digit(b, d);
    }
}

/// Les six chiffres BCD des trois derniers octets du long à `src` (RAM ou table de la ROM), zéros de tête
/// effacés, le dernier toujours écrit (0x0f6b40).
pub fn number6(b: &mut Cps2Bus, at: u32, src: u32, attr: u16) {
    let mut n = Number::new(at, attr);
    let (high, middle) = (rd8(b, src + 1), rd8(b, src + 2));
    n.byte(b, high);
    n.byte(b, middle);
    let low = rd8(b, src + 3);
    n.digit(b, low >> 4);
    n.last(b, low);
}

/// Une durée `m:ss` (mot BCD 0x0mss, le chiffre des dizaines de minutes omis s'il est nul) (0x0f6d60).
pub fn duration(b: &mut Cps2Bus, at: u32, t: u16) {
    let nib = |k: u32| (t >> (12 - 4 * k)) as u8 & 15;
    if nib(0) != 0 {
        digit(b, at, nib(0), 0);
    }
    digit(b, at + COLUMN, nib(1), 0);
    tall(b, at + 2 * COLUMN, COLON, 0);
    digit(b, at + 3 * COLUMN, nib(2), 0);
    digit(b, at + 4 * COLUMN, nib(3), 0);
}

/// Un grand chiffre (deux colonnes de deux cases), ou deux colonnes sautées pour un zéro de tête (0x0f6e2e).
pub fn big_digit(b: &mut Cps2Bus, n: &mut Number, d: u8) {
    let d = d & 15;
    if d != 0 || n.lead {
        n.lead = true;
        let tile = prog16(b, BIG_DIGITS + 2 * d as u32);
        glyph(b, n.at, tile, n.attr);
    }
    n.at += 2 * COLUMN;
}

/// Un caractère de deux colonnes : `tile`, `tile + 16` puis `tile + 1`, `tile + 17`.
pub fn glyph(b: &mut Cps2Bus, at: u32, tile: u16, attr: u16) {
    tall(b, at, tile, attr);
    tall(b, at + COLUMN, tile.wrapping_add(1), attr);
}

/// Texte en caractères de deux colonnes (0x0f6f90) : un mot d'attributs puis des tuiles ; 0 termine, 0xffff
/// relit des attributs, 0xfffe laisse une colonne vide. Rend la colonne qui suit le texte.
pub fn glyphs(b: &mut Cps2Bus, mut at: u32, mut src: u32) -> u32 {
    let mut attr = rd16(b, src);
    src += 2;
    loop {
        match rd16(b, src) {
            0 => return at,
            0xffff => {
                attr = rd16(b, src + 2);
                src += 4;
            }
            0xfffe => {
                src += 2;
                blank(b, at);
                at += COLUMN;
            }
            tile => {
                src += 2;
                glyph(b, at, tile, attr);
                at += 2 * COLUMN;
            }
        }
    }
}

/// Colonne de cases d'une table de mots (paires de tuiles haut/bas, 0xffff termine), attributs 2 (0x0f7262).
pub fn label(b: &mut Cps2Bus, mut at: u32, mut src: u32) {
    while rd16(b, src) != 0xffff {
        let (top, bottom) = (rd16(b, src), rd16(b, src + 2));
        wr16(b, at, top);
        wr16(b, at + 2, 2);
        wr16(b, at + 4, bottom);
        wr16(b, at + 6, 2);
        src += 4;
        at += COLUMN;
    }
}
