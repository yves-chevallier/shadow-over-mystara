//! Éléments communs aux gestionnaires de la classe BOSS (pool FFBA1E, 3 × 0xC0 octets, table 0x39b12).
//! Voir `docs/familles/boss.md`.
//!
//! Un boss a la même charpente qu'un ennemi (`enemy_common`) : `+4` état principal (0 création,
//! 2 actif, 4 touché, 6 libération), `+5`, `+6`, `+7` sous-états (octets pairs qui indexent des tables
//! de sauts). On réutilise donc les champs, états et boucles de `enemy_common` ; ce module ajoute les
//! variables du jeu propres aux scènes de boss, les drapeaux du 68000 pour les comparaisons qui dépendent
//! du débordement (`Fl`) et les aides de la bibliothèque d'IA (0x103d6..0x105b6, encore en 68000).

use m68k::SR_C;

pub use super::cx::Cx;
pub use super::enemy_common::*;
use crate::objects::Obj;

/// Variables de la RAM de travail (décalages de A5) lues par les boss.
pub mod var {
    /// Abscisse de la caméra 1 (mot).
    pub const SCROLL_X: u32 = 0x506;
    /// Ordonnée de la caméra 1 (mot ; long avec la fraction).
    pub const SCROLL_Y: u32 = 0x50a;
    /// Objet suivi par la caméra (mot, adresse RAM ; 0 = aucun) : le « meneur » que certains boss accompagnent.
    pub const LEADER: u32 = 0x46;
    /// Octet non nul : tout est figé (le gestionnaire du boss ne fait rien).
    pub const FROZEN: u32 = 0x4e;
    /// Nombre de joueurs en vie (octet).
    pub const PLAYERS_ALIVE: u32 = 0x35;
    /// Octet à 1 pendant une cinématique (jeu en pause, entrées coupées).
    pub const PAUSE: u32 = 0x3d;
    /// Caméra utilisée (octet non nul : la 2) et position (X, Y) de chacune.
    pub const CAMERA_SELECT: u32 = 0x36;
    pub const CAMERA1_X: u32 = 0x5a6;
    pub const CAMERA1_Y: u32 = 0x5aa;
    pub const CAMERA2_X: u32 = 0x5ae;
    pub const CAMERA2_Y: u32 = 0x5b2;
    /// Difficulté (mot, 0 à 31).
    pub const DIFFICULTY: u32 = 0x7f92;
}

/// L'objet suivi par la caméra, s'il existe.
pub fn leader(x: &mut Cx) -> Option<Obj> {
    (x.a5_u16(var::LEADER) != 0).then(|| Obj(x.a5_u16(var::LEADER) as i16 as i32 as u32 & 0xff_ffff))
}

/// Abscisse de la caméra.
pub fn scroll_x(x: &mut Cx) -> u16 {
    x.a5_u16(var::SCROLL_X)
}

/// Abscisse de la caméra en service (1, ou 2 si `A5+0x36`).
pub fn camera_x(x: &mut Cx) -> u16 {
    let off = if x.a5_u8(var::CAMERA_SELECT) != 0 { var::CAMERA2_X } else { var::CAMERA1_X };
    x.a5_u16(off)
}

// --- drapeaux du 68000 -------------------------------------------------------------------------

/// Drapeaux N Z V C d'une soustraction ou d'une comparaison `dst - src`, pour les branchements dont
/// l'issue dépend du débordement (`ble`, `bgt`, `blt`, `bge`) : l'ancienne valeur extrême (0x80, 0x8000)
/// ne se comporte pas comme « résultat négatif ».
#[derive(Clone, Copy, Debug)]
pub struct Fl {
    pub z: bool,
    pub n: bool,
    pub v: bool,
    pub c: bool,
}

impl Fl {
    pub fn sub8(dst: u8, src: u8) -> Fl {
        let r = dst.wrapping_sub(src);
        Fl { z: r == 0, n: r & 0x80 != 0, v: (dst ^ src) & (dst ^ r) & 0x80 != 0, c: dst < src }
    }
    pub fn sub16(dst: u16, src: u16) -> Fl {
        let r = dst.wrapping_sub(src);
        Fl { z: r == 0, n: r & 0x8000 != 0, v: (dst ^ src) & (dst ^ r) & 0x8000 != 0, c: dst < src }
    }
    pub fn sub32(dst: u32, src: u32) -> Fl {
        let r = dst.wrapping_sub(src);
        Fl { z: r == 0, n: r & 0x8000_0000 != 0, v: (dst ^ src) & (dst ^ r) & 0x8000_0000 != 0, c: dst < src }
    }
    /// `tst` : N et Z seulement.
    pub fn tst8(v: u8) -> Fl {
        Fl { z: v == 0, n: v & 0x80 != 0, v: false, c: false }
    }
    pub fn tst16(v: u16) -> Fl {
        Fl { z: v == 0, n: v & 0x8000 != 0, v: false, c: false }
    }
    pub fn eq(self) -> bool {
        self.z
    }
    pub fn ne(self) -> bool {
        !self.z
    }
    pub fn mi(self) -> bool {
        self.n
    }
    pub fn pl(self) -> bool {
        !self.n
    }
    pub fn lt(self) -> bool {
        self.n != self.v
    }
    pub fn ge(self) -> bool {
        self.n == self.v
    }
    pub fn gt(self) -> bool {
        !self.z && self.n == self.v
    }
    pub fn le(self) -> bool {
        self.z || self.n != self.v
    }
}

/// `addq.b #k,(off,A0)` : ajoute `k` à l'octet et fixe les drapeaux X N Z V C comme le 68000 (utile quand une
/// routine appelée ensuite, sonore par exemple, rend les drapeaux qu'elle a reçus).
pub fn addq8(x: &mut Cx, off: u32, k: u8) {
    let old = x.u8(off);
    let new = old.wrapping_add(k);
    x.set8(off, new);
    let c = old.checked_add(k).is_none();
    let v = !(old ^ k) & (old ^ new) & 0x80 != 0;
    let ccr = (c as u16) << 4 | ((new & 0x80 != 0) as u16) << 3 | ((new == 0) as u16) << 2 | (v as u16) << 1 | c as u16;
    x.c.sr = x.c.sr & 0xffe0 | ccr;
}

/// Écrit les drapeaux X N Z V C d'une soustraction dans le registre d'état (les routines sonores rendent ceux qu'elles
/// ont reçus).
fn set_ccr(x: &mut Cx, f: Fl) {
    let ccr = (f.c as u16) << 4 | (f.n as u16) << 3 | (f.z as u16) << 2 | (f.v as u16) << 1 | f.c as u16;
    x.c.sr = x.c.sr & 0xffe0 | ccr;
}

/// `subq.b #k,(off,A0)` : retire `k` à l'octet et rend les drapeaux (aussi écrits dans le registre d'état).
pub fn sub8(x: &mut Cx, off: u32, k: u8) -> Fl {
    let old = x.u8(off);
    x.set8(off, old.wrapping_sub(k));
    let f = Fl::sub8(old, k);
    set_ccr(x, f);
    f
}

/// `subq.w #k,(off,A0)`.
pub fn sub16(x: &mut Cx, off: u32, k: u16) -> Fl {
    let old = x.u16(off);
    x.set16(off, old.wrapping_sub(k));
    let f = Fl::sub16(old, k);
    set_ccr(x, f);
    f
}

/// `cmpi.b #imm,(off,A0)` : drapeaux de `octet - imm`.
pub fn cmp8(x: &mut Cx, off: u32, imm: u8) -> Fl {
    Fl::sub8(x.u8(off), imm)
}

/// `cmpi.w #imm,(off,A0)`.
pub fn cmp16(x: &mut Cx, off: u32, imm: u16) -> Fl {
    Fl::sub16(x.u16(off), imm)
}

/// `tst.b (off,A0)`.
pub fn tst8(x: &mut Cx, off: u32) -> Fl {
    Fl::tst8(x.u8(off))
}

/// `tst.w (off,A0)`.
pub fn tst16(x: &mut Cx, off: u32) -> Fl {
    Fl::tst16(x.u16(off))
}

// --- bibliothèque d'IA du 68000 (0x103d6..) -----------------------------------------------------

/// Routines d'IA partagées par les ennemis et les boss, encore en 68000.
pub mod ai {
    /// Lance le script `D0` du jeu `A4` (`0x1a32`) puis recopie `+0x3e` en `+0x44`.
    pub const PLAY: u32 = 0x103d6;
    /// Lance l'entrée du jeu `A4` dont la marque `+3` est celle de l'image-clé `+0x33`.
    pub const SELECT: u32 = 0x103e0;
    /// Un pas d'animation (retenue = fin du script) puis `+0x44 = +0x3e`.
    pub const STEP: u32 = 0x1040c;
    /// Cible dans la boîte devant l'objet : D0 = portée avant, D1 = portée, D2 = profondeur ; Z = non.
    pub const TARGET_IN_BOX: u32 = 0xb5e0;
    pub const SPEED_TABLE: u32 = 0x1b64;
}

/// Lance le script `n` du jeu d'animations `set` (`0x103d6`).
pub fn play(x: &mut Cx, set: u32, n: u32) {
    x.c.a[4] = set;
    x.c.d[0] = n;
    x.call(ai::PLAY);
}

/// Un pas d'animation de l'IA ; vrai si le script est terminé.
pub fn step(x: &mut Cx) -> bool {
    x.call(ai::STEP);
    carry(x)
}

/// Choisit la table de vitesses `n` (`0x1b64`).
pub fn speed(x: &mut Cx, n: u32) {
    x.c.d[0] = n;
    x.call(ai::SPEED_TABLE);
}

/// Retenue levée par la dernière routine appelée.
pub fn carry(x: &Cx) -> bool {
    x.c.sr & SR_C != 0
}

/// Z levé par la dernière routine appelée.
pub fn zero(x: &Cx) -> bool {
    x.c.cond(7)
}

/// Alloue un objet de la classe `routine` (retenue levée = pool plein). Les gestionnaires qui insistent
/// (`jsr alloc ; bcs retry`) bouclent ici ; le pool ne se vide qu'avec le temps, l'original resterait
/// bloqué : on abandonne après 64 essais.
pub fn alloc_retry(x: &mut Cx, routine: u32) -> Option<Obj> {
    for _ in 0..64 {
        let o = alloc(x, routine);
        if !alloc_failed(x) {
            return Some(o);
        }
    }
    None
}

/// Alloue sans insister.
pub fn try_alloc(x: &mut Cx, routine: u32) -> Option<Obj> {
    let o = alloc(x, routine);
    (!alloc_failed(x)).then_some(o)
}

/// Écrit un octet à une adresse absolue de la RAM de travail.
pub fn set_abs8(x: &mut Cx, addr: u32, v: u8) {
    crate::mem::wr8(x.b, addr, v);
}
