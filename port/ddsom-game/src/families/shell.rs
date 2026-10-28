//! Éléments communs aux gestionnaires de la classe ESHL (tirs et attaques des ennemis) : champs
//! d'un objet ESHL, états, aides de placement et de collision avec le terrain.

use crate::families::cx::Banc;
use super::cx::Cx;
use crate::objects::Obj;
use crate::objects::Courant;

/// Champs propres aux objets ESHL.
pub mod f {
    /// Orientation (non nul : retourné), copiée du propriétaire à la création.
    pub const FLIP: u32 = 0x2c;
    /// Jeu de boîtes de collision.
    pub const BOX_SET: u32 = 0x2e;
    /// Puissance de l'attaque (`+0x44` : identifiant d'attaque, `+0x45` : puissance).
    pub const ATTACK_ID: u32 = 0x44;
    pub const ATTACK_POWER: u32 = 0x45;
    /// Propriétaire (adresse RAM sur 16 bits).
    pub const OWNER: u32 = 0x50;
    /// Drapeaux posés par le moteur de collisions (bit 3 : touché, bit 5 : paré).
    pub const HIT_FLAGS: u32 = 0x57;
    /// Compte à rebours propre au type.
    pub const TIMER: u32 = 0x5c;
}

/// États principaux communs (`+4`).
pub mod st {
    pub const FREE: u8 = 6;
}

/// Clignotement : l'objet est caché une image sur deux (parité de `+0x5c` ajoutée au compteur de
/// boucle D7 du dispatch).
pub fn blink(x: &mut Cx) {
    let visible = x.u16(f::TIMER).wrapping_add(x.c.d[7] as u16) & 1 != 0;
    x.set_bit(0x1, 1, !visible);
}

/// Copie la position (X, Z, Y : parties entières) du propriétaire dans le tir.
pub fn copy_position<R: Banc>(x: &mut Cx<'_, R>, owner: Obj) {
    for off in [0x8, 0xc, 0x10] {
        let v = owner.u16(x.b, off);
        x.set16(off, v);
    }
}

/// Place le tir devant le propriétaire : sa position, un pixel plus près (profondeur - 1), puis
/// décalé de `dx` du côté où il regarde et de `dz` en hauteur.
pub fn place_in_front<R: Banc>(x: &mut Cx<'_, R>, owner: Obj, dx: i16, dz: u16) {
    copy_position(x, owner);
    x.add16(0x10, 0xffff);
    let side = if owner.u8(x.b, f::FLIP) != 0 { dx } else { -dx };
    x.add16(0x8, side as u16);
    x.add16(0xc, dz);
}

/// Pause du jeu : vrai quand l'objet est suspendu (octet `+0x40`).
pub fn suspended<R: Banc>(x: &mut Cx<'_, R>) -> bool {
    x.suspended()
}

/// Terrain sous le tir : vrai quand le tir doit s'arrêter (terrains 5 à 8 sans le bit 6 de
/// l'attribut).
pub fn blocked_by_terrain(x: &mut Cx) -> bool {
    // registres de 0x24a8 laissés comme l'original (non prouvés morts : voir `pshl::stopped_by_terrain`)
    let t = crate::video::terrain_of(x.b, x.o);
    crate::video::terrain_68k(x.c, &t);
    let terrain = t.numero as u32;
    (terrain & 0x3f).wrapping_sub(5) as u16 <= 3 && terrain & 0x40 == 0
}

/// Variable `A5+0x3d` : jeu en pause (les tirs ne bougent pas).
pub const GAME_PAUSED: u32 = 0x3d;
