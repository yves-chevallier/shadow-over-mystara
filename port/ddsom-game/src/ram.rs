//! Variables de la RAM de travail, nommées et typées (socle de la refactorisation, `docs/guide-refactorisation.md`).
//!
//! L'état du jeu reste dans la RAM émulée tant que la validation compare la RAM image par image avec l'original : une
//! variable est donc une *adresse* typée, et `get`/`set` lisent et écrivent le bus (big-endian, comme le 68000). On
//! écrit `ram::SCENE.get(b)` au lieu de `rd8(b, A5 + 0x34)`, et `ram::JOUEURS.nth(k)` au lieu de
//! `A5 + 0x5de + 0x200 * k`.
//!
//! Règle : une variable propre à un sous-système est déclarée dans son module (`hud::var`, `objects::ram`...) avec
//! ces mêmes types ; ici ne figurent que les variables lues par plusieurs sous-systèmes. Les noms viennent de
//! `docs/*.md` ; une hypothèse de sens est marquée (h).

use cps2::Cps2Bus;

use crate::mem::{rd16, rd32, rd8, wr16, wr32, wr8, A5};
use crate::objects::Obj;

/// Octet de la RAM de travail.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Octet(pub u32);

/// Mot (16 bits) de la RAM de travail.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Mot(pub u32);

/// Long (32 bits) de la RAM de travail.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Long(pub u32);

impl Octet {
    /// Variable à `A5 + décalage` (les décalages négatifs de l'original s'écrivent en absolu : `0xff2472`).
    pub const fn a5(decalage: u32) -> Self {
        Octet(A5 + decalage)
    }
    pub fn get(self, b: &mut Cps2Bus) -> u8 {
        rd8(b, self.0)
    }
    pub fn get_i8(self, b: &mut Cps2Bus) -> i8 {
        rd8(b, self.0) as i8
    }
    pub fn set(self, b: &mut Cps2Bus, v: u8) {
        wr8(b, self.0, v)
    }
    /// Octet non nul (`tst.b` suivi de `bne`).
    pub fn est_pose(self, b: &mut Cps2Bus) -> bool {
        self.get(b) != 0
    }
}

impl Mot {
    pub const fn a5(decalage: u32) -> Self {
        Mot(A5 + decalage)
    }
    pub fn get(self, b: &mut Cps2Bus) -> u16 {
        rd16(b, self.0)
    }
    pub fn get_i16(self, b: &mut Cps2Bus) -> i16 {
        rd16(b, self.0) as i16
    }
    pub fn set(self, b: &mut Cps2Bus, v: u16) {
        wr16(b, self.0, v)
    }
}

impl Long {
    pub const fn a5(decalage: u32) -> Self {
        Long(A5 + decalage)
    }
    pub fn get(self, b: &mut Cps2Bus) -> u32 {
        rd32(b, self.0)
    }
    pub fn set(self, b: &mut Cps2Bus, v: u32) {
        wr32(b, self.0, v)
    }
}

/// Tableau d'enregistrements de taille fixe (joueurs, panneaux, compteurs de pièces...).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Table {
    pub base: u32,
    pub pas: u32,
    pub nombre: u32,
}

impl Table {
    /// Enregistrement `k` (non vérifié : l'original indexe parfois au-delà).
    pub const fn nth(self, k: u32) -> Obj {
        Obj(self.base + self.pas * k)
    }
    pub fn iter(self) -> impl Iterator<Item = Obj> {
        (0..self.nombre).map(move |k| self.nth(k))
    }
}

/// Germe du générateur pseudo-aléatoire `0x19a88` (`rng.rs`).
pub const GERME: Mot = Mot::a5(0x1a);
/// Type de borne : nombre de joueurs (h) ; choisit la disposition de l'interface.
pub const BORNE: Octet = Octet::a5(0x20);
/// Phase de la partie (pas de 2 : 8 entrée de scène, 0xc jeu, 0x10 sortie, 0x14 fin de partie ; `docs/boucle.md`)
/// et son étape.
pub const PHASE: Octet = Octet::a5(0x2c);
pub const ETAPE: Octet = Octet::a5(0x2d);
/// Une partie est en cours.
pub const PARTIE: Octet = Octet::a5(0x30);
/// Numéro de scène (1-114 ; `docs/rewrite-guide.md`, « Atteindre un stage »).
pub const SCENE: Octet = Octet::a5(0x34);
/// Nombre de joueurs en vie (lot D : appelé à tort « difficulté » jusqu'au lot S).
pub const JOUEURS_EN_VIE: Octet = Octet::a5(0x35);
/// Caméra alternative choisie (non nul) au lieu de la caméra principale.
pub const CAMERA_ALT_ACTIVE: Octet = Octet::a5(0x36);
/// Mode d'affichage des panneaux de l'interface (`hud`).
pub const AFFICHAGE_PANNEAUX: Octet = Octet::a5(0x3c);
/// Pause : un bit par joueur.
pub const PAUSE: Octet = Octet::a5(0x4a);
/// Compteur d'images du VBLANK (`vblank.rs`).
pub const IMAGES: Mot = Mot::a5(0x6c);
/// Défilement vertical voulu du plan 1 (`vblank` le recopie dans le registre du plan) : les sorts font trembler
/// l'écran en le basculant entre 0 et 0x100.
pub const DEFILEMENT_Y_PLAN1: Mot = Mot::a5(0x4ba);
/// Caméra principale : position à l'écran (x, y) ; caméra alternative.
pub const CAMERA_X: Mot = Mot::a5(0x506);
pub const CAMERA_Y: Mot = Mot::a5(0x50a);
pub const CAMERA_ALT_X: Mot = Mot::a5(0x556);
pub const CAMERA_ALT_Y: Mot = Mot::a5(0x55a);
/// Mot des commutateurs (DSW) lu au VBLANK (`FF7FD0`) : bit 0x200 sans affichage des dégâts, 0x400 jauge de
/// charge inactive, 0x800 collisions avec le décor.
pub const DSW: Mot = Mot::a5(0x7fd0);
/// Les quatre enregistrements de joueurs (0x200 octets ; champs : `player::Player`).
pub const JOUEURS: Table = Table { base: A5 + 0x5de, pas: 0x200, nombre: 4 };

/// Caméra de l'image : la caméra alternative si elle est choisie, sinon la principale.
pub fn camera(b: &mut Cps2Bus) -> (u16, u16) {
    if CAMERA_ALT_ACTIVE.est_pose(b) {
        (CAMERA_ALT_X.get(b), CAMERA_ALT_Y.get(b))
    } else {
        (CAMERA_X.get(b), CAMERA_Y.get(b))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adresses() {
        assert_eq!(SCENE.0, 0xff8034);
        assert_eq!(JOUEURS.nth(3).0, 0xff8bde);
        assert_eq!(JOUEURS.iter().count(), 4);
    }
}
