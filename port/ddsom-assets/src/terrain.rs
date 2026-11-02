//! Terrain et collision avec le décor : `assets/terrain.json`, voir docs/assets.md.
//!
//! Le numéro de terrain d'un point est le champ `attributs >> 7` de la case du scroll 2 sous lui
//! (déjà dans `levels.json`), sauf dans deux scènes qui lisent une carte fixe (`FixedMap`). Ce numéro
//! indexe une table de types (`KindTable`, deux copies selon la routine de collision).

use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::json::hex;

/// Carte de terrain fixe : colonnes de 16 pixels de large, 16 cases de 16 pixels de haut chacune.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct FixedMap {
    #[serde(with = "hex")]
    pub rom: u32,
    /// Scène (`A5+0x34`) qui lit cette carte.
    pub stage: u8,
    /// Ajouté à y avant la lecture (mot).
    pub dy: i16,
    /// Mots d'attributs, colonne par colonne (x croissant) ; dans une colonne, le mot `k` couvre
    /// `(y + dy) / 16 % 16 = 15 - k` (le haut de l'écran d'abord).
    pub columns: Vec<[u16; 16]>,
}

impl FixedMap {
    /// Mot d'attributs au point (x, y) du monde, avec l'arithmétique de 0x24b0 (`None` hors de la carte).
    pub fn word(&self, x: u16, y: u16) -> Option<u16> {
        let y = y.wrapping_add(self.dy as u16);
        let k = 15 - ((y >> 4) & 15) as usize;
        let column = (x.wrapping_add(x) & 0xffe0) as i16 as isize / 32;
        let c = self.columns.get(usize::try_from(column).ok()?)?;
        Some(c[k])
    }

    /// Numéro de terrain au point (x, y).
    pub fn terrain(&self, x: u16, y: u16) -> Option<u16> {
        self.word(x, y).map(|w| (w & 0xff80) >> 7)
    }
}

/// Type de terrain (entrée de 16 octets, mots), voir `ddsom-game/src/terrain.rs`.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Kind {
    /// 0 franchissable, 1 obstacle plein, 2 et plus pente.
    pub genre: u16,
    /// Sous-programme (0, 4, 8, 12) : bord en dessous, au-dessus, à droite, à gauche.
    pub routine: u16,
    /// Pente : sens (0 ou 1).
    pub flip: u16,
    /// Pente : décalage (échelle) de la hauteur.
    pub shift: u16,
    /// Profondeur du bord (pente : hauteur de base).
    pub edge: u16,
    /// Obstacle plein : non nul, un seul passage.
    pub stop: u16,
    /// Mots 6 et 7, inutilisés (nuls).
    pub unused: [u16; 2],
}

impl Kind {
    pub fn words(&self) -> [u16; 8] {
        [self.genre, self.routine, self.flip, self.shift, self.edge, self.stop, self.unused[0], self.unused[1]]
    }

    pub fn from_words(w: [u16; 8]) -> Self {
        Kind { genre: w[0], routine: w[1], flip: w[2], shift: w[3], edge: w[4], stop: w[5], unused: [w[6], w[7]] }
    }
}

/// Table des types d'une routine de collision (0x33fe4 : `0x3425c` ; 0x3403c : `0x34a5c`).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct KindTable {
    #[serde(with = "hex")]
    pub rom: u32,
    /// Adresse de la routine qui la lit.
    #[serde(with = "hex")]
    pub routine: u32,
    pub kinds: Vec<Kind>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Terrain {
    pub maps: Vec<FixedMap>,
    pub kinds: Vec<KindTable>,
}

impl Terrain {
    pub fn load(path: &Path) -> io::Result<Self> {
        crate::json::load(path)
    }
}
