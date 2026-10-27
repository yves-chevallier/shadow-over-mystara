//! Éléments communs aux gestionnaires des tirs (PSHL) et des effets (PEFF) des joueurs : champs d'un
//! objet, libération, création d'objets frères.
//!
//! Un objet PSHL ou PEFF naît d'un créateur (`player/spawn.rs`) avec son genre (`+2`), sa variante (`+3`)
//! et le lien `+0x50` vers le joueur ; certains types gardent un lien `+0x52` ou `+0x5c` vers un objet
//! frère. Les positions suivent le joueur avec un décalage lu dans une table de la ROM.

use crate::machine::Machine;
use crate::objects::Courant;
use crate::families::cx::Cx;
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

impl Cx<'_> {
    /// `movea.w (off,A0),A1` : le joueur (ou l'objet) lié ; le registre A1 en garde l'adresse étendue.
    pub fn link_a1(&mut self, off: u32) -> Obj {
        let w = self.u16(off);
        self.c.a[1] = w as i16 as i32 as u32;
        Obj(self.c.a[1] & 0xff_ffff)
    }

    /// `jsr rng ; andi.w #mask,D0w` : D0 garde son mot haut, le mot bas est masqué ; renvoie le mot bas.
    pub fn rand_and(&mut self, mask: u16) -> u32 {
        let alea = self.random_68k();
        let low = alea & mask;
        self.c.d[0] = self.c.d[0] & 0xffff_0000 | low as u32;
        low as u32
    }

    /// `jsr rng ; andi.b #mask,D0b` : seul l'octet bas de D0 est masqué (le reste du mot garde le tirage).
    pub fn rand_and_b(&mut self, mask: u8) -> u8 {
        let alea = self.random_68k();
        let low = alea as u8 & mask;
        self.c.d[0] = self.c.d[0] & !0xff | low as u32;
        low
    }

    /// Début d'un gestionnaire : lit l'état `+4` et, comme `jmp (table,PC,D0.w)`, laisse D0 = état et D1.w =
    /// mot de la table (les registres que l'original laisse au gestionnaire suivant).
    pub fn state(&mut self, table: u32) -> u8 {
        let state = self.u8(0x4);
        self.c.d[0] = state as u32;
        let word = self.rom16(table + state as u32);
        self.c.d[1] = self.c.d[1] & 0xffff_0000 | word as u32;
        state
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

    /// Remplace le mot bas de D0 (`move.w`).
    pub fn set_d0w(&mut self, v: u16) {
        self.c.d[0] = self.c.d[0] & 0xffff_0000 | v as u32;
    }
}

/// Copie l'orientation et la position du joueur lié (`+0x50`) ; renvoie le joueur.
pub fn follow_owner(x: &mut Cx) -> Obj {
    let owner = x.link_a1(f::OWNER);
    let flip = owner.u8(x.b, f::FLIP);
    x.set8(f::FLIP, flip);
    x.copy_pos(owner);
    owner
}

/// Décalage (X, Z) lu dans une table de mots de la ROM (lecture relative au PC), ligne `row` ; D1 et D2
/// en gardent les valeurs comme l'original.
pub fn offset(x: &mut Cx, table: u32, row: u32) -> (u16, u16) {
    let dx = x.rom16(table + row);
    let dz = x.rom16(table + row + 2);
    x.c.d[1] = dx as u32;
    x.c.d[2] = dz as u32;
    (dx, dz)
}
