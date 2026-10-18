//! Éléments communs aux gestionnaires de la classe ENEMY (ennemis, pool de 22 × 0xC0 octets) :
//! champs, états, allocation de sous-objets, mort, aides de rang et de pause.
//! Voir `docs/familles/enemy.md`.
//!
//! Tous les ennemis ont la même charpente : l'octet `+4` est l'état principal (0 création, 2 actif,
//! 4 touché, 6 mort), `+5` et `+6` des sous-états ; les valeurs sont des octets pairs qui indexent
//! des tables de sauts. L'état « mort » libère l'emplacement (`0x21f4`).

use m68k::SR_C;

use super::cx::Cx;
use crate::objects::Obj;

/// Champs des objets ENEMY.
pub mod f {
    /// État principal, sous-état, sous-sous-état.
    pub const STATE: u32 = 0x4;
    pub const SUB: u32 = 0x5;
    pub const SUB2: u32 = 0x6;
    /// Jeu de boîtes de collision (voir `docs/combat.md` §3).
    pub const BOX_SET: u32 = 0x2e;
    /// Orientation (non nul : regarde à gauche).
    pub const FLIP: u32 = 0x2c;
    /// Hôte (mot, adresse RAM) d'une partie d'ennemi composite.
    pub const HOST: u32 = 0x46;
    /// Parent ou propriétaire (mot, adresse RAM) ; `+0x42` : créateur d'un sous-objet.
    pub const OWNER: u32 = 0x50;
    pub const CREATOR: u32 = 0x42;
    /// Compteur de gel après un coup (décompté par `0xf1bfc`).
    pub const FREEZE: u32 = 0x53;
    /// Points de vie (mot signé) et valeur mémorisée au dernier coup.
    pub const HP: u32 = 0x62;
    /// Immunités (long : un bit par comportement de tir).
    pub const IMMUNITY: u32 = 0x94;
}

/// États principaux (`+4`), valeurs longues pour `Cx::set32(0x4, …)` : état dans l'octet haut.
pub mod st {
    pub const INIT: u8 = 0;
    pub const ACTIVE: u8 = 2;
    pub const HIT: u8 = 4;
    pub const DEAD: u8 = 6;
    /// Long d'état « mort » (`+4..+7`) : libération au prochain passage.
    pub const DEAD_LONG: u32 = 0x0600_0000;
}

/// Routines du moteur appelées par les ennemis.
pub mod rom {
    pub const FREE_ENEMY: u32 = 0x21f4;
    pub const ALLOC_ENEMY: u32 = 0x22bc;
    pub const ALLOC_ESHL: u32 = 0x22ea;
    pub const ALLOC_EFFCT: u32 = 0x2318;
    pub const ALLOC_ITEM: u32 = 0x23d8;
    /// PV initiaux selon le type, la difficulté et le nombre de joueurs.
    pub const INITIAL_HP: u32 = 0x4544;
    /// Immunités du type (rangées en `+0x94`).
    pub const IMMUNITIES: u32 = 0xfe5a;
    pub const RNG: u32 = 0x19a88;
    /// Compteurs de fin de handler (gel `+0x53`, compte à rebours `+0x89`).
    pub const TICK_COUNTERS: u32 = 0xf1bfc;
    /// Variable `A5+0x3d` : jeu en pause.
    pub const PAUSE_FLAG: u32 = 0x3d;
}

/// Alloue un objet du pool `alloc` (adresse d'une routine d'allocation) et rend son adresse (`A1`).
/// L'original n'examine pas la retenue : il écrit à travers `A1` même si le pool est plein.
pub fn alloc(x: &mut Cx, alloc: u32) -> Obj {
    x.call(alloc);
    Obj(x.c.a[1])
}

/// Vrai si l'allocation a échoué (retenue levée par la routine d'allocation).
pub fn alloc_failed(x: &Cx) -> bool {
    x.c.sr & SR_C != 0
}

/// Libère l'ennemi courant (état « mort » traité).
pub fn free(x: &mut Cx) {
    x.call(rom::FREE_ENEMY);
}

/// Tirage pseudo-aléatoire du jeu (mot dans D0).
pub fn random(x: &mut Cx) -> u32 {
    x.call(rom::RNG);
    x.c.d[0]
}

/// Jeu en pause (`A5+0x3d` non nul).
pub fn paused(x: &mut Cx) -> bool {
    x.a5_u8(rom::PAUSE_FLAG) != 0
}

/// Le gestionnaire est suspendu (hook de pause `0x29ea`, Z = non suspendu).
pub fn suspended(x: &mut Cx) -> bool {
    x.call(super::cx::sub::PAUSE_HOOK);
    x.c.cond(6)
}

/// Objet désigné par un mot de 16 bits de l'objet courant (`movea.w (off,A0),A1`).
pub fn link(x: &mut Cx, off: u32) -> Obj {
    x.linked(off)
}

/// Écrit le long d'état (`+4..+7`) d'un autre objet.
pub fn set_state(x: &mut Cx, o: Obj, state: u32) {
    o.set_u32(x.b, f::STATE, state);
}

/// Test « dans la zone de jeu » et ombre portée, fin commune des états actifs.
pub const IN_ZONE: u32 = 0xfbae;
pub const SHADOW: u32 = 0x33fe4;

/// Boucle commune des états actifs des ennemis à IA : tant que le gel `+0x53` est positif il décroît
/// et rien d'autre ne bouge ; sinon, hors pause, `sub` exécute le sous-état (il rend vrai pour
/// abandonner l'image sans la suite, comme l'original qui dépile son adresse de retour). La suite
/// teste la zone et dessine l'ombre dès que le sous-état n'est plus 0, puis remet l'objet en liste.
pub fn active_loop(x: &mut Cx, sub: impl FnOnce(&mut Cx) -> bool) {
    let freeze = x.u8(f::FREEZE) as i8;
    if freeze > 0 {
        x.set8(f::FREEZE, (freeze - 1) as u8);
    } else if !paused(x) && sub(x) {
        return;
    }
    if x.u8(f::SUB) != 0 {
        x.call(IN_ZONE);
        x.call(SHADOW);
    }
    x.reinsert();
}

/// Boucle commune de l'état « touché » : même gel ; la zone n'est testée que si `+0xae` est nul.
pub fn hit_loop(x: &mut Cx, sub: impl FnOnce(&mut Cx)) {
    let freeze = x.u8(f::FREEZE) as i8;
    if freeze > 0 {
        x.set8(f::FREEZE, (freeze - 1) as u8);
    } else {
        sub(x);
    }
    if x.u8(0xae) == 0 {
        x.call(IN_ZONE);
        x.call(SHADOW);
    }
    x.reinsert();
}

/// Long lu dans une table du programme (adressage relatif au PC, espace des opcodes).
pub fn rom32(x: &mut Cx, addr: u32) -> u32 {
    (x.rom16(addr) as u32) << 16 | x.rom16(addr + 2) as u32
}

/// Réaction à un coup : la routine commune, choisie dans `table` (longs, indexés par le code de
/// réaction `+0x49`) par la collision, joue l'animation de recul, de chute ou de mort.
pub fn react(x: &mut Cx, table: u32) {
    let code = x.u8(0x49) as u32;
    x.c.d[0] = code;
    let routine = rom32(x, table + code);
    x.c.a[1] = routine;
    x.call(routine);
}

/// `move.w #v,Dn` : ne change que le mot de poids faible du registre.
pub fn set_dw(x: &mut Cx, n: usize, v: u16) {
    x.c.d[n] = x.c.d[n] & 0xffff_0000 | v as u32;
}

/// `add.b #v,(off,An)` sur un autre objet (modulo 2^8).
pub fn add8(x: &mut Cx, o: Obj, off: u32, v: u8) {
    let n = o.u8(x.b, off).wrapping_add(v);
    o.set_u8(x.b, off, n);
}

/// `add.w #v,(off,An)` sur un autre objet (modulo 2^16).
pub fn add16(x: &mut Cx, o: Obj, off: u32, v: u16) {
    let n = o.u16(x.b, off).wrapping_add(v);
    o.set_u16(x.b, off, n);
}

/// Démarre le script `n` du jeu d'animations `set` (`movea.l set,A4 ; moveq n,D0 ; jsr 0x1a32`).
pub fn anim(x: &mut Cx, set: u32, n: u32) {
    x.anim_start(set, n);
}
