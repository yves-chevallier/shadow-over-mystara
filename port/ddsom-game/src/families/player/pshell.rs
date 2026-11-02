//! Éléments communs aux gestionnaires des tirs (PSHL) et des effets (PEFF) des joueurs : champs d'un
//! objet, libération, création d'objets frères.
//!
//! Un objet PSHL ou PEFF naît d'un créateur (`player/spawn.rs`) avec son genre (`+2`), sa variante (`+3`)
//! et le lien `+0x50` vers le joueur ; certains types gardent un lien `+0x52` ou `+0x5c` vers un objet
//! frère. Les positions suivent le joueur avec un décalage lu dans une table de la ROM.

use crate::machine::Machine;
use crate::objects::Current;
use crate::families::cx::{Bench, Cx};
use crate::objects::Obj;
use crate::objects::pools;

/// Champs propres aux objets joueur.
pub mod f {
    /// Orientation héritée du joueur (non nul : regarde à droite).
    pub const FLIP: u32 = 0x2c;
    /// Jeu de boîtes de collision.
    pub const BOX_SET: u32 = 0x2e;
    /// Identifiant (`+0x45`) et puissance (`+0x46`) du coup.
    pub const POWER: u32 = 0x45;
    pub const POWER2: u32 = 0x46;
    /// Joueur propriétaire (adresse RAM sur 16 bits).
    pub const OWNER: u32 = 0x50;
    /// Drapeaux posés par le moteur de collisions.
    pub const HIT_FLAGS: u32 = 0x57;
}

/// États principaux (`+4`).
pub mod st {
    pub const FREE: u8 = 4;
}

/// Valeur d'un registre d'adresse chargé par `movea.w` : le mot de l'adresse, étendu en signe.
pub fn ext(o: Obj) -> u32 {
    o.0 as u16 as i16 as i32 as u32
}


/// Décalages de la RAM de travail (A5).
pub mod a5 {
    /// Caméra secondaire.
    pub const CAM2_X: u32 = 0x556;
    pub const CAM2_Y: u32 = 0x55a;
    /// Caméra principale.
    pub const CAM_X: u32 = 0x506;
    pub const CAM_Y: u32 = 0x50a;
    /// Non nul : la caméra de référence est la secondaire.
    pub const USE_CAM2: u32 = 0x36;
    /// Non nul : version à palette alternative (`tst.w (0x70,A5)`).
    pub const ALT_PALETTE: u32 = 0x70;
    /// Compteur d'images du jeu.
    pub const FRAME: u32 = 0x6f;
    /// Emplacements libres du pool PEFF.
    pub const PEFF_FREE: u32 = 0x7e60;
    /// Jeu en pause (octet non nul).
    pub const PAUSED: u32 = 0x42;
}

impl<R: Bench> Cx<'_, R> {
    /// Tirage masqué (`jsr rng ; andi.w #mask,D0`) : le mot bas du tirage, masqué.
    pub fn rand_and(&mut self, mask: u16) -> u32 {
        (self.random() & mask) as u32
    }

    /// Tirage masqué (`jsr rng ; andi.w #mask,D0`), sans registre.
    pub fn rand_mask(&mut self, mask: u16) -> u16 {
        self.random() & mask
    }

    /// `jsr rng ; andi.b #mask,D0b` : l'octet bas du tirage, masqué.
    pub fn rand_and_b(&mut self, mask: u8) -> u8 {
        self.random() as u8 & mask
    }

    /// `rand_and_b`, mais rend le mot D0.w qu'il laisse : l'octet haut du tirage et l'octet bas masqué (relu par
    /// l'éclat de gel, index de ses délais).
    pub fn rand_and_b_word(&mut self, mask: u8) -> u16 {
        let rng = self.random();
        rng & 0xff00 | (rng as u8 & mask) as u16
    }

    /// État `+4` du gestionnaire (`jmp (table,PC,D0.w)` de l'original ; `_table` : sa table de sauts).
    pub fn state(&mut self, _table: u32) -> u8 {
        self.u8(0x4)
    }

    /// Libère le tir.
    pub fn free_pshl(&mut self) {
        self.free(&pools::PSHL);
    }

    /// Libère l'effet.
    pub fn free_peff(&mut self) {
        self.free(&pools::PEFF);
    }

    /// Copie la position (X, Z, Y entiers) d'un autre objet.
    pub fn copy_pos(&mut self, from: Obj) {
        for off in [0x8, 0xc, 0x10] {
            let v = from.u16(self.b, off);
            self.set16(off, v);
        }
    }

    /// Long de la ROM programme (tables lues relativement au PC).
    pub fn rom32(&mut self, addr: u32) -> u32 {
        (self.rom16(addr) as u32) << 16 | self.rom16(addr + 2) as u32
    }

    /// Démarre le script `n` du jeu d'animations `set` (`anim_start_indexed`).
    pub fn anim(&mut self, set: u32, n: u32) {
        self.anim_start(set, n as u16);
    }
}

/// Copie l'orientation et la position du joueur lié (`+0x50`) ; renvoie le joueur.
pub fn follow_owner<R: Bench>(x: &mut Cx<'_, R>) -> Obj {
    let owner = x.linked(f::OWNER);
    let flip = owner.u8(x.b, f::FLIP);
    x.set8(f::FLIP, flip);
    x.copy_pos(owner);
    owner
}

/// Décalage (X, Z) lu dans une table de mots de la ROM (lecture relative au PC), ligne `row`.
/// Glitch d'origine : D2 garde le décalage Z (reste relu par d'autres gestionnaires, `dispatch::Leftovers`).
pub fn offset<R: Bench>(x: &mut Cx<'_, R>, table: u32, row: u32) -> (u16, u16) {
    let dx = x.rom16(table + row);
    let dz = x.rom16(table + row + 2);
    *x.c.d2() = dz as u32;
    (dx, dz)
}

/// Restes de la recherche du terrain (0x24a8 / 0x24b0, `video::terrain_68k`) : D2 (numéro), D3 (index de la case),
/// D4.w (colonne). Glitch d'origine : des gestionnaires suivants les relisent (`dispatch::Leftovers` ; D3 :
/// `boss11_scene42_mort`, D2 : rebond de l'éclat de gel).
pub fn leftovers_terrain<R: Bench>(x: &mut Cx<'_, R>, t: &crate::video::Terrain) {
    if t.fixed_map {
        *x.c.d3() = t.index as u32;
    } else {
        *x.c.d3() = *x.c.d3() & 0xffff_0000 | t.index as u32;
        *x.c.d2() = 0;
    }
    *x.c.d4() = *x.c.d4() & 0xffff_0000 | t.column as u32;
    *x.c.d2() = *x.c.d2() & 0xffff_0000 | t.number as u32;
}
