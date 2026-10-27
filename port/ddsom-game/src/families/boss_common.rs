//! Éléments communs aux gestionnaires de la classe BOSS (pool FFBA1E, 3 × 0xC0 octets, table 0x39b12).
//! Voir `docs/familles/boss.md`.
//!
//! Un boss a la même charpente qu'un ennemi (`enemy_common`) : `+4` état principal (0 création,
//! 2 actif, 4 touché, 6 libération), `+5`, `+6`, `+7` sous-états (octets pairs qui indexent des tables
//! de sauts). On réutilise donc les champs, états et boucles de `enemy_common` ; ce module ajoute les
//! variables du jeu propres aux scènes de boss, les drapeaux du 68000 pour les comparaisons qui dépendent
//! du débordement (`Fl`) et les aides de la bibliothèque d'IA (0x103d6..0x105b6, encore en 68000).

use m68k::decode::Size;
use m68k::SR_C;

pub use super::cx::Cx;
pub use super::enemy_common::*;
use crate::mem::{prog16, prog32, rts, A5};
use crate::objects::Obj;
use crate::{call, regs, Native};
use crate::objects::pools;
use crate::objects::Courant;


/// L'objet suivi par la caméra, s'il existe.
pub fn leader(x: &mut Cx) -> Option<Obj> {
    (var::LEADER.get(x.b) != 0).then(|| Obj(var::LEADER.get(x.b) as i16 as i32 as u32 & 0xff_ffff))
}

/// Abscisse de la caméra.
pub fn scroll_x(x: &mut Cx) -> u16 {
    var::SCROLL_X.get(x.b)
}

/// Abscisse de la caméra en service (1, ou 2 si `A5+0x36`).
pub fn camera_x(x: &mut Cx) -> u16 {
    let v = if var::CAMERA_SELECT.get(x.b) != 0 { var::CAMERA2_X } else { var::CAMERA1_X };
    v.get(x.b)
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




/// Choisit la table de vitesses `n` (`0x1b64`).
pub fn speed(x: &mut Cx, n: u32) {
    x.select_speed(&crate::objects::motion::SPEEDS_B, n as u16);
}

/// Retenue levée par la dernière routine appelée.
pub fn carry(x: &Cx) -> bool {
    x.c.sr & SR_C != 0
}

/// Z levé par la dernière routine appelée.
pub fn zero(x: &Cx) -> bool {
    x.c.cond(7)
}

// --- aides partagées par les boss à tables de décision ------------------------------------------







/// Cible dans la boîte (0xb5e0) puis geste choisi par l'IA (0x1047a) : `Some(D0)` si l'IA en a choisi un.
pub fn ai_box(x: &mut Cx, d0: i16, d1: u16, d2: u16) -> Option<u32> {
    if !lib::target_in_box(x, d0, d1, d2) {
        return None;
    }
    let turn = lib::turn_decision(x);
    (turn != lib::Turn::None).then_some(turn as u32)
}



/// `moveq #k,D0 ; move.b D0,(0xa0,A0) ; move.b D0,(0xa1,A0)` : sorte d'animation `k` (champs `+0xa0` et `+0xa1`).
pub fn anim_kind(x: &mut Cx, k: u8) {
    x.set8(0xa0, k);
    x.set8(0xa1, k);
}

/// `subq.w #1,(off,A0)` : vrai quand le décompte est écoulé (`ble`).
pub fn expired(x: &mut Cx, off: u32) -> bool {
    sub16(x, off, 1).le()
}

/// Mot de la table de sauts relative au PC `base` pour l'index d'octets `i` : adresse de la routine choisie
/// (`move.w (base,PC,Di),Di ; jmp (base,PC,Di)`).
pub fn jump_target(x: &mut Cx, base: u32, i: u32) -> u32 {
    base.wrapping_add(x.rom16(base + i) as i16 as u32)
}

/// Ordonnée de la caméra en service.
pub fn camera_y(x: &mut Cx) -> u16 {
    let v = if var::CAMERA_SELECT.get(x.b) != 0 { var::CAMERA2_Y } else { var::CAMERA1_Y };
    v.get(x.b)
}

/// Adresse du long « boss vaincu » (type, variante, score) lu par la fin de scène.
pub const A5_DEATH: u32 = A5 - 0x557c;

/// Alloue un objet de la classe `routine` (retenue levée = pool plein). Les gestionnaires qui insistent
/// (`jsr alloc ; bcs retry`) bouclent ici ; le pool ne se vide qu'avec le temps, l'original resterait
/// bloqué : on abandonne après 64 essais.
pub fn alloc_retry(x: &mut Cx, pool: &pools::Pool) -> Option<Obj> {
    (0..64).find_map(|_| x.alloc(pool))
}

/// Écrit un octet à une adresse absolue de la RAM de travail.
pub fn set_abs8(x: &mut Cx, addr: u32, v: u8) {
    crate::mem::wr8(x.b, addr, v);
}

// --- sous-routines communes appelées par le code d'origine des boss -------------------------

/// Boss enregistré comme obstacle pour les autres objets (mot, 0 / positif = aucun).
const BODY: u32 = A5 - 0x5fb2;
/// Formes du corps (`+0xa4` du boss) : table de sauts relatifs (1 = 0x818e6, 2 = 0x819e2).
const BODY_SHAPES: u32 = 0x81ad4;

/// 0x81aa4 : repousse l'objet hors du corps du boss enregistré, si ce boss est actif, de classe 3 et a une forme
/// non nulle (la routine de la forme, restée en code d'origine, déplace l'objet en X). Vrai si l'objet a été
/// déplacé. Registres de l'original : D0 = 1 / 0 (drapeaux de `tst.l`), A0 intact, A1 = A6 = l'objet.
pub fn push_out_of_boss(x: &mut Cx) -> bool {
    let me = x.o.0;
    x.c.a[6] = me;
    x.c.a[1] = me;
    let body = x.data16(BODY);
    let mut moved = false;
    if (body as i16) < 0 {
        x.c.a[0] = body as i16 as i32 as u32;
        let boss = Obj(x.c.a[0] & 0xff_ffff);
        if boss.u8(x.b, 0) != 0 && boss.u8(x.b, 2) == 3 {
            let shape = boss.u8(x.b, 0xa4) as u16;
            if shape != 0 {
                let off = prog16(x.b, BODY_SHAPES + 2 * shape as u32);
                x.c.d[0] = off as u32;
                // la routine de forme (0x818e6, 0x819e2) n'a pas de native : jamais atteinte par les scénarios
                call(x.c, x.b, BODY_SHAPES.wrapping_add(off as i16 as u32));
                moved = !x.c.cond(7);
            }
        }
    }
    x.c.d[0] = moved as u32;
    x.c.flags_logic(x.c.d[0], Size::Long);
    x.c.a[0] = x.c.a[6];
    moved
}

/// Oscillation verticale (par `+0x33`) : décalages longs ajoutés à la hauteur `+0xc`, dans un sens
/// puis dans l'autre selon le compte à rebours `+0xa8` (période 72 images, retournement à 36).
const BOB_OFFSETS: u32 = 0x833e4;

/// 0x833b6 : un pas de l'oscillation verticale d'un boss (D0 = décalage appliqué, comme l'original).
pub fn bob(x: &mut Cx) {
    let i = (x.u8(f::KEY_FRAME) & 0x7f) as u32 * 4;
    let mut d = prog32(x.b, BOB_OFFSETS + i);
    if x.u16(0xa8) as i16 <= 36 {
        d = d.wrapping_neg();
    }
    x.add32(Obj::Z, d);
    x.c.d[0] = d;
    // `subq.w #1` puis `bgt` : comparaison signée exacte de l'ancienne valeur (0x8000 repart aussi à 72)
    let avant = x.u16(0xa8);
    x.set16(0xa8, avant.wrapping_sub(1));
    if avant as i16 <= 1 {
        x.set16(0xa8, 72);
    }
}

/// 0x87048 : selon la difficulté de la scène (`A5+0x5a` : 0 rien, 3 toujours) et la forme `+0xad`
/// (2 ou 3 : selon les PV), pose `+0x43` (pas de seuil) ou, si la moitié des PV `+0x62` dépasse 72,
/// retient ce seuil en `+0xb6` et pose `+0x42`. D0, D1 comme l'original.
pub fn hp_threshold(x: &mut Cx) {
    let level = var::SCENE_LEVEL.get(x.b);
    x.c.d[0] = level as u32;
    if level == 0 {
        return;
    }
    let by_hp = level != 3 && matches!(x.u8(0xad), 2 | 3);
    if by_hp {
        if x.u8(0x42) != 0 {
            return;
        }
        let half = x.u16(f::HP) >> 1;
        // l'original calcule la moitié dans D1.w (contrat de sortie)
        x.c.d[1] = (x.c.d[1] & 0xffff_0000) | half as u32;
        if half as i16 > 72 {
            x.set8(0xb9, 0);
            x.set16(0xb6, half);
            x.set8(0x42, 1);
            return;
        }
    }
    x.set8(0x43, 1);
    x.set8(0xb9, 0);
}

/// Native d'une fonction de boss appelée par adresse (le RTS est ajouté).
macro_rules! natif {
    ($entry:expr, $name:expr, $outputs:expr, $f:expr) => {
        Native {
            entry: $entry,
            name: $name,
            run: |c, b| {
                $f(&mut Cx::new(c, b));
                rts(c, b);
            },
            touches_sound: false,
            outputs: $outputs,
        }
    };
}

pub const NATIVES: &[Native] = &[
    natif!(0x81aa4, "boss_repousse", regs::d(0) | regs::a(0) | regs::a(1) | regs::a(6) | regs::CCR, |x: &mut Cx| {
        push_out_of_boss(x);
    }),
    natif!(0x833b6, "boss_oscillation", regs::d(0), bob),
    natif!(0x87048, "boss_seuil_pv", regs::d(0) | regs::d(1), hp_threshold),
];
