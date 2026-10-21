//! Définitions des sprites : images (assemblages de cellules 16x16) et scripts d'animation.
//! Formats : `assets/sprites/frames.json` et `assets/sprites/anims.json`, voir docs/assets.md.

use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::json::{hex, hex_opt, hex_vec};

/// Morceau d'une image : `[dx, dy, code, attributs]`. Le coin haut-gauche du morceau est à (dx, dy)
/// pixels du point d'ancrage de l'objet (y vers le bas de l'écran : dy négatif = au-dessus) ; en miroir
/// horizontal, à (-dx - 16 - 16 (largeur - 1), dy). Attributs matériels CPS2 : bits 0-4 palette, 5 miroir
/// horizontal, 6 miroir vertical, 8-11 largeur - 1 et 12-15 hauteur - 1 (en cellules 16x16). La cellule
/// (bx, by) d'un morceau est `bank << 16 | (code & !0xf) + ((code + bx) & 0xf) + 16 by`.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Piece(pub i16, pub i16, pub u16, pub u16);

impl Piece {
    pub fn dx(&self) -> i16 {
        self.0
    }
    pub fn dy(&self) -> i16 {
        self.1
    }
    pub fn code(&self) -> u16 {
        self.2
    }
    pub fn attr(&self) -> u16 {
        self.3
    }
}

/// Une image de sprite (« cadre »), telle que l'objet la désigne en `+0x34`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    /// Adresse ROM de l'en-tête (provenance ; c'est aussi la valeur que le jeu range dans l'objet).
    #[serde(with = "hex")]
    pub rom: u32,
    /// Coût en morceaux retiré du budget de la liste de sprites (mot 0 de l'en-tête).
    pub cost: u16,
    /// Mot 1 de l'en-tête : attributs communs (`mode` nul) ou nombre de morceaux (`mode` non nul).
    pub attr: u16,
    /// Banque de cellules : la cellule d'un morceau est `bank << 16 | code`.
    pub bank: u8,
    /// Octet 5 de l'en-tête : nul, tous les morceaux prennent `attr` ; sinon chacun a ses attributs.
    pub mode: u8,
    /// Adresse ROM de la table de décalages (partagée entre images).
    #[serde(with = "hex")]
    pub offsets: u32,
    pub pieces: Vec<Piece>,
}

impl Frame {
    pub fn per_piece(&self) -> bool {
        self.mode != 0
    }
}

/// Toutes les images, dans l'ordre de la table de la ROM (numéro d'image = indice).
#[derive(Serialize, Deserialize, Debug)]
pub struct Frames {
    /// Table ROM des adresses d'images (un long par image).
    #[serde(with = "hex")]
    pub table: u32,
    pub frames: Vec<Frame>,
}

impl Frames {
    pub fn load(path: &Path) -> io::Result<Self> {
        crate::json::load(path)
    }

    /// Numéro de l'image dont l'en-tête est à l'adresse ROM `rom`.
    pub fn by_rom(&self, rom: u32) -> Option<usize> {
        self.frames.binary_search_by_key(&rom, |f| f.rom).ok()
    }
}

/// Famille du visualiseur de sprites du menu de test caché (`pl00`, `em03`, `bs10`...) : un jeu
/// d'animations par variante.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Family {
    pub name: String,
    /// Entrées longues (personnages joueurs).
    pub long: bool,
    #[serde(with = "hex_vec")]
    pub sets: Vec<u32>,
}

/// Jeu d'animations : tableau de décalages signés (octets, relatifs à `rom`) vers le début de chaque script.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct AnimSet {
    #[serde(with = "hex")]
    pub rom: u32,
    pub long: bool,
    pub scripts: Vec<i16>,
}

impl AnimSet {
    /// Adresse du script `i`.
    pub fn script(&self, i: usize) -> u32 {
        self.rom.wrapping_add(self.scripts[i] as i32 as u32)
    }
}

/// Entrée d'un script d'animation (voir `objects/anim.rs`).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    #[serde(with = "hex")]
    pub rom: u32,
    /// Entrée longue (24 octets, personnages) ou courte (16 octets).
    pub long: bool,
    /// Taille déduite de l'enchaînement, l'entrée n'étant atteinte par aucun jeu connu.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub guessed: bool,
    /// Durée en images.
    pub time: u16,
    /// Drapeaux : octet haut 0 (l'entrée suivante suit), 0xfe (fin) ou autre (saut vers `next`).
    pub flags: u16,
    /// Numéro de l'image (indice dans `Frames`).
    pub frame: u32,
    /// Octets recopiés dans l'objet en `+0x38` (8 octets : ombre, marque au sol, boîte de visibilité,
    /// priorité ; 16 pour une entrée longue, jusqu'à `+0x47`).
    pub data: Vec<u8>,
    /// Cible du saut quand l'octet haut des drapeaux n'est ni 0 ni 0xfe.
    #[serde(default, with = "hex_opt", skip_serializing_if = "Option::is_none")]
    pub next: Option<u32>,
}

impl Entry {
    /// Adresse de l'entrée suivante dans l'ordre de la ROM.
    pub fn end(&self) -> u32 {
        self.rom + if self.long { 24 } else { 16 } + if self.next.is_some() { 4 } else { 0 }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Anims {
    /// Tables du visualiseur de sprites (provenance des familles) : noms, listes de jeux, variantes.
    #[serde(with = "hex")]
    pub names_rom: u32,
    #[serde(with = "hex")]
    pub families_rom: u32,
    #[serde(with = "hex")]
    pub variants_rom: u32,
    pub families: Vec<Family>,
    pub sets: Vec<AnimSet>,
    /// Toutes les entrées connues, par adresse croissante.
    pub entries: Vec<Entry>,
}

impl Anims {
    pub fn load(path: &Path) -> io::Result<Self> {
        crate::json::load(path)
    }

    pub fn entry(&self, rom: u32) -> Option<&Entry> {
        self.entries.binary_search_by_key(&rom, |e| e.rom).ok().map(|i| &self.entries[i])
    }

    pub fn set(&self, rom: u32) -> Option<&AnimSet> {
        self.sets.iter().find(|s| s.rom == rom)
    }
}
