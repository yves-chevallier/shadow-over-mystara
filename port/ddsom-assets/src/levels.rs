//! Cartes des décors : `assets/levels.json`, voir docs/assets.md.
//!
//! Chaque couche (scroll 2 : cases de 16 pixels ; scroll 3 : cases de 32 pixels) est faite de *bandes*
//! de 256x256 pixels. Une bande est une grille de *blocs* de 2x2 cases ; un bloc donne le code de tuile
//! et les attributs de ses quatre cases. Pour chaque scène, une *table de bandes* donne la grille des
//! numéros de bandes de la carte.

use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::json::hex;

/// Bloc de 2x2 cases : (code, attributs) des cases (0,0), (0,1), (1,0), (1,1) (x puis y).
pub type Block = [u16; 8];

/// Banque de blocs.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Bank {
    #[serde(with = "hex")]
    pub rom: u32,
    pub blocks: Vec<Block>,
}

/// Table de bandes d'une ou plusieurs scènes.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct StripTable {
    /// Adresse ROM du mot d'en-tête (largeur).
    #[serde(with = "hex")]
    pub rom: u32,
    /// Lignes dans l'ordre de la ROM, du haut de la carte vers le bas ; chacune a la largeur de
    /// l'en-tête (dans le scroll 2, son dernier mot est une sentinelle, 0xfe).
    pub rows: Vec<Vec<u16>>,
    /// Ligne d'ordonnée 0, celle que désigne le pointeur de la scène (les suivantes sont sous 0).
    pub zero: usize,
}

impl StripTable {
    pub fn width(&self) -> usize {
        self.rows[0].len()
    }

    /// Valeur du pointeur de scène qui désigne cette table.
    pub fn pointer(&self) -> u32 {
        self.rom + 2 + (2 * self.width() * self.zero) as u32
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Layer {
    pub name: String,
    /// Côté d'une case en pixels (16 ou 32) ; une bande a `256 / case` cases de côté.
    pub case: u16,
    /// Ajouté au code d'une case pour obtenir la tuile de la couche (0x1800 ou 0x1f00) ; base de palette.
    pub tile_offset: u16,
    pub palette_base: u16,
    /// Numéros de bandes de remplissage, jamais à l'écran (non définies).
    pub fillers: Vec<u16>,
    /// Table ROM des banques de blocs (une banque par groupe de 256 bandes).
    #[serde(with = "hex")]
    pub banks_rom: u32,
    pub banks: Vec<Bank>,
    /// Bandes : numéros de blocs, colonne par colonne (8x8 dans le scroll 2, 4x4 dans le scroll 3).
    #[serde(with = "hex")]
    pub strips_rom: u32,
    pub strips: Vec<Vec<u16>>,
    /// Table ROM des pointeurs de scènes (espace programme), une par scène 0..114.
    #[serde(with = "hex")]
    pub scenes_rom: u32,
    /// Table de bandes de chaque scène (indice dans `tables`).
    pub scenes: Vec<usize>,
    pub tables: Vec<StripTable>,
}

impl Layer {
    /// Cases d'une bande par côté.
    pub fn strip_cases(&self) -> usize {
        256 / self.case as usize
    }

    /// Banque de la bande `strip`.
    pub fn bank_of(&self, strip: usize) -> usize {
        strip / 256 % if self.case == 16 { 4 } else { 16 }
    }

    /// Case (cx, cy) de la bande `strip` : (tuile, attributs), tuile déjà décalée de `tile_offset`.
    pub fn case_at(&self, strip: usize, cx: usize, cy: usize) -> (u16, u16) {
        let blocks_per_side = self.strip_cases() / 2;
        let index = self.strips[strip][(cx / 2) * blocks_per_side + cy / 2] as usize;
        let block = &self.banks[self.bank_of(strip)].blocks[index];
        let k = 2 * ((cx % 2) * 2 + cy % 2);
        (block[k].wrapping_add(self.tile_offset), block[k + 1])
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Levels {
    pub layers: Vec<Layer>,
}

impl Levels {
    pub fn load(path: &Path) -> io::Result<Self> {
        crate::json::load(path)
    }
}
