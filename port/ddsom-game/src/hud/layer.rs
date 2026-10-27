//! Couche de caractères (tilemap 8x8 à `0x912000`) vue par les panneaux des joueurs.
//!
//! Une case est un long (tuile, attributs). La couche est rangée par colonnes : la case suivante d'une
//! colonne est 4 octets plus loin, la colonne suivante 0x80 octets plus loin. Les panneaux sont des
//! rectangles de cases dont l'origine dépend du joueur, de la zone et de la disposition de l'écran.

use cps2::Cps2Bus;

use crate::mem::{prog16, rd16, wr16, A5};

pub const LAYER: u32 = 0x912000;
pub const COLUMN: u32 = 0x80;
pub const CELL: u32 = 4;
/// Case vide : tuile 0x20 (espace), attributs nuls.
pub const EMPTY: u32 = 0x0020_0000;

/// Pinceau : un pointeur d'écriture qui avance case par case, comme `(A1)+` de l'original.
pub struct Brush<'a> {
    pub b: &'a mut Cps2Bus,
    pub at: u32,
}

impl<'a> Brush<'a> {
    pub fn new(b: &'a mut Cps2Bus, at: u32) -> Self {
        Brush { b, at }
    }

    /// Écrit une case (tuile, attributs) et passe à la suivante de la colonne.
    pub fn cell(&mut self, tile: u16, attr: u16) {
        wr16(self.b, self.at, tile);
        wr16(self.b, self.at + 2, attr);
        self.at += CELL;
    }

    /// Passe à la colonne suivante, à la même hauteur que le début d'une série de `written` cases.
    pub fn next_column(&mut self, written: u32) {
        self.at = self.at + COLUMN - written * CELL;
    }

    /// Avance d'une colonne depuis la case courante (déjà au bout de sa case).
    pub fn skip_column_after_cell(&mut self) {
        self.at += COLUMN - CELL;
    }

    /// Série verticale de trois cases lues dans une table (tuiles consécutives), attributs communs, puis
    /// colonne suivante : l'unité des grandes images d'attente.
    pub fn column3(&mut self, tiles: [u16; 3], attr: u16) {
        for tile in tiles {
            self.cell(tile, attr);
        }
        self.next_column(3);
    }
}

/// Origine d'une zone du panneau du joueur d'indice `player` : table de la ROM programme (lue relative au PC,
/// donc déchiffrée), une ligne de 8 mots par zone, quatre positions par disposition ; la disposition à
/// deux colonnes (`A5+0x344` impair) utilise la seconde moitié de la ligne.
pub fn origin(b: &mut Cps2Bus, player: u8, zone: u16) -> u32 {
    const ORIGINS: u32 = 0x172b4;
    let narrow = crate::mem::rd8(b, A5 + 0x344) & 1 != 0;
    let index = 2 * player as u32 + 16 * zone as u32 + if narrow { 8 } else { 0 };
    LAYER.wrapping_add(prog16(b, ORIGINS + index) as i16 as i32 as u32)
}

/// Série de colonnes de trois cases lues dans la ROM (`n` colonnes, trois mots par colonne).
pub fn columns(brush: &mut Brush, table: u32, n: u32, attr: u16) {
    for i in 0..n {
        let tiles = [0, 1, 2].map(|k| rd16(brush.b, table + 6 * i + 2 * k));
        brush.column3(tiles, attr);
    }
}
