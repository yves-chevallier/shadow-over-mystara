//! Palettes des scènes : quelles palettes de la bibliothèque (`palettes.png`) chaque scène charge,
//! `assets/scene_palettes.json`, voir docs/assets.md et docs/video.md (V3).
//!
//! Toutes les palettes désignées sont dans la bibliothèque (ROM 0x253000) : une adresse `a` est la
//! palette `(a - 0x253000) / 32` de `palettes.png` ; une page entière est 32 palettes consécutives.

use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::json::{hex, hex_vec};

/// Table d'adresses de palettes, une par entrée (scène, ensemble...).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct AddressTable {
    #[serde(with = "hex")]
    pub rom: u32,
    /// Lue en espace programme (accès relatif au PC) plutôt qu'en espace données.
    pub program: bool,
    /// Palettes copiées depuis chaque adresse.
    pub count: u32,
    /// Adresse de la première palette (0 : aucune, ou adresse 0 de la ROM pour la page 3).
    #[serde(with = "hex_vec")]
    pub entries: Vec<u32>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ScenePalettes {
    /// Bibliothèque de palettes (32 octets chacune).
    #[serde(with = "hex")]
    pub library: u32,
    /// Page 0, par scène : 32 numéros de palettes de la bibliothèque (-1 : palette laissée en place).
    /// Le jeu n'en charge que 29, sauf pour les scènes 0x45 à 0x59.
    #[serde(with = "hex")]
    pub stages_rom: u32,
    pub stages: Vec<[i16; 32]>,
    /// Trois palettes ajoutées après celles de la page 0, par scène (0x1a41c).
    pub extra: AddressTable,
    /// Ensembles de 32 palettes chargés dans la page 0 par 0x1a3d4.
    pub sets: AddressTable,
    /// Pages 1 à 3 (32 palettes chacune), par entrée (page 1 : 0 à 2 ; pages 2 et 3 : scène).
    pub pages: Vec<AddressTable>,
}

impl ScenePalettes {
    pub fn load(path: &Path) -> io::Result<Self> {
        crate::json::load(path)
    }

    /// Numéro dans la bibliothèque de la palette à l'adresse `rom`.
    pub fn library_index(&self, rom: u32) -> Option<usize> {
        let d = rom.checked_sub(self.library)?;
        (d % 32 == 0).then_some(d as usize / 32)
    }

    /// Mot de déplacement tel que la ROM le range pour un numéro de palette de `stages`.
    pub fn stage_word(palette: i16) -> u16 {
        if palette < 0 {
            0xffff
        } else {
            palette as u16 * 32
        }
    }
}
