//! Tables des objets liées aux sprites : boîtes de collision, boîtes de visibilité, marques au sol et
//! listes d'images lues directement par le code. Format : `assets/sprites/tables.json`, voir docs/assets.md.

use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::json::{hex, hex_vec};

/// Boîte de collision (8 octets) : X (signé), demi-largeur, Z (hauteur, signé), demi-hauteur, Y
/// (profondeur, signé), demi-profondeur, puis deux octets que la collision ne lit pas. Décalages depuis
/// la position de l'objet ; X est inversé quand l'objet regarde à gauche (docs/combat.md §3).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct HitBox(pub i8, pub u8, pub i8, pub u8, pub i8, pub u8, pub u8, pub u8);

impl HitBox {
    pub fn bytes(&self) -> [u8; 8] {
        [self.0 as u8, self.1, self.2 as u8, self.3, self.4 as u8, self.5, self.6, self.7]
    }

    pub fn from_bytes(b: &[u8]) -> Self {
        HitBox(b[0] as i8, b[1], b[2] as i8, b[3], b[4] as i8, b[5], b[6], b[7])
    }
}

/// Table de jeux de boîtes : un long par jeu (`+0x2e` de l'objet), adresse de la boîte d'index 0 du jeu ;
/// la boîte d'index `n` (octet de l'objet, 0 = aucune) est à `adresse + 8 n`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct BoxTable {
    /// `body` (index `+0x38`), `push` (`+0x39`) ou `attack` (`+0x3a`).
    pub name: String,
    #[serde(with = "hex")]
    pub rom: u32,
    #[serde(with = "hex_vec")]
    pub sets: Vec<u32>,
}

/// Boîtes rangées à partir d'une adresse désignée par un jeu, jusqu'à la suivante. `tail` : octets
/// restants quand l'écart n'est pas un multiple de 8 (fin de bloc, bourrage `0x4e71`).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct BoxBlock {
    #[serde(with = "hex")]
    pub rom: u32,
    pub boxes: Vec<HitBox>,
    pub tail: Vec<u8>,
}

impl BoxBlock {
    pub fn len(&self) -> usize {
        8 * self.boxes.len() + self.tail.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Boîte de visibilité (0x1ed2, index `+0x3c`), mots : l'objet est à l'écran si `x écran + dx <= width`
/// et `hauteur écran + dy <= height` (comparaisons non signées).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bounds(pub i16, pub u16, pub i16, pub u16);

/// Marque au sol (0x1e3a6, index `+0x3b`), 8 mots.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct GroundMark {
    /// Morceaux retirés du budget de la liste de sprites.
    pub cost: u16,
    /// Code du sprite matériel.
    pub code: u16,
    /// Décalage x fixe.
    pub x: i16,
    /// Attributs du sprite (bit 5 inversé en miroir).
    pub attr: u16,
    /// Décalage x supplémentaire, opposé en miroir.
    pub dx: i16,
    /// Décalage vertical (retranché de la hauteur à l'écran).
    pub dy: i16,
    /// Bits de priorité ajoutés au mot x du sprite.
    pub priority: u16,
    pub unused: u16,
}

impl GroundMark {
    pub fn words(&self) -> [u16; 8] {
        [self.cost, self.code, self.x as u16, self.attr, self.dx as u16, self.dy as u16, self.priority, self.unused]
    }

    pub fn from_words(w: [u16; 8]) -> Self {
        GroundMark { cost: w[0], code: w[1], x: w[2] as i16, attr: w[3], dx: w[4] as i16, dy: w[5] as i16, priority: w[6], unused: w[7] }
    }
}

/// Liste d'images rangées en longs (adresses d'images) et lue directement par le code.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct FrameList {
    #[serde(with = "hex")]
    pub rom: u32,
    /// Numéros d'images (`frames.json`).
    pub frames: Vec<u32>,
}

/// Tables du tir ESHL 5 (0x9e458, anneaux du serpent et zones de contact des grands ennemis).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Shot5 {
    /// Par `+0x2f` : indices de boîtes corps, poussée, attaque, puis la marque au sol (`+0x3b`) ; espace
    /// programme.
    #[serde(with = "hex")]
    pub boxes_rom: u32,
    pub boxes: Vec<[u8; 4]>,
    /// Par `+0x2f` : déplacement (mot, depuis la table, espace programme) de la liste d'images des images
    /// paires (`even`) ou impaires (`odd`).
    #[serde(with = "hex")]
    pub even_rom: u32,
    pub even: Vec<i16>,
    #[serde(with = "hex")]
    pub odd_rom: u32,
    pub odd: Vec<i16>,
    /// (image, retournement) par `(+0x42 / 2) * 16 + +0x43 / 2` ; espace données.
    #[serde(with = "hex")]
    pub mirrored_rom: u32,
    pub mirrored: Vec<[u8; 2]>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SpriteTables {
    pub box_tables: Vec<BoxTable>,
    /// Toutes les boîtes, en blocs jointifs (un par adresse désignée), de `box_tables[..].sets` jusqu'à
    /// `box_end`.
    pub box_blocks: Vec<BoxBlock>,
    #[serde(with = "hex")]
    pub box_end: u32,
    #[serde(with = "hex")]
    pub bounds_rom: u32,
    pub bounds: Vec<Bounds>,
    #[serde(with = "hex")]
    pub marks_rom: u32,
    pub marks: Vec<GroundMark>,
    pub frame_lists: Vec<FrameList>,
    pub shot5: Shot5,
}

impl SpriteTables {
    pub fn load(path: &Path) -> io::Result<Self> {
        crate::json::load(path)
    }

    /// Boîte d'index `n` du jeu `set` de la table `table` (0 corps, 1 poussée, 2 attaque).
    pub fn hit_box(&self, table: usize, set: usize, n: usize) -> Option<HitBox> {
        let at = self.box_tables[table].sets[set] + 8 * n as u32;
        let i = self.box_blocks.partition_point(|b| b.rom <= at).checked_sub(1)?;
        let b = &self.box_blocks[i];
        let off = (at - b.rom) as usize;
        if off % 8 == 0 && off / 8 < b.boxes.len() {
            return Some(b.boxes[off / 8]);
        }
        // boîte à cheval sur un bloc et le suivant (blocs non alignés)
        let mut bytes = Vec::new();
        for blk in &self.box_blocks[i..] {
            for bx in &blk.boxes {
                bytes.extend(bx.bytes());
            }
            bytes.extend(&blk.tail);
            if bytes.len() >= off + 8 {
                break;
            }
        }
        (bytes.len() >= off + 8).then(|| HitBox::from_bytes(&bytes[off..off + 8]))
    }
}
