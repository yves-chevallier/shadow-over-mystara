//! Créateurs d'effets (EFFCT) : les petites routines de la famille qui allouent un effet, posent son
//! type et sa variante, puis le lient à leur créateur. Appelées par les gestionnaires des joueurs, des
//! ennemis, des boss, des décors et des scènes.
//!
//! Chaque créateur est une fonction Rust ordinaire (créateur, variante, réglage en arguments ; l'effet créé
//! en retour, `None` si le pool est plein), écrite une seule fois sur un [`restes::Registres`] : le cœur
//! ([`restes::Pur`]) ne touche aucun registre ; la native (appels par adresse) et l'adaptateur `_68k` passent le
//! processeur, qui reçoit ce que l'original laisse : A1 = l'effet créé, la retenue levée si le pool est plein
//! (les appelants testent `bcs`), les drapeaux de la dernière écriture, les registres de travail (compteurs de
//! boucle, tirages).

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::Cpu;

use crate::mem::{prog16, prog32, rd16, rd32, rd8, rts, set_low_word, wr16, wr32, A5};
use crate::objects::pools::{self, Pool};
use crate::objects::Obj;
use crate::{regs, Native};
use restes::{Pur, Registres};

/// Ce qu'une routine du lot laisse dans les registres de l'original.
pub(crate) mod restes {
    use cps2::Cps2Bus;
    use m68k::decode::Size;
    use m68k::Cpu;

    use crate::mem::{set_low_byte, set_low_word};
    use crate::objects::pools::Pool;
    use crate::objects::Obj;

    /// Le cœur d'une routine s'écrit une fois sur un `Registres` : [`Pur`] (appelants Rust) n'écrit aucun registre,
    /// le processeur (natives, adaptateurs `_68k`) reçoit ceux que l'original laisse (allocation comprise :
    /// `Pool::alloc_68k`, A1, D6 et la retenue).
    pub(crate) trait Registres {
        fn alloue(&mut self, b: &mut Cps2Bus, pool: &Pool) -> Option<Obj>;
        /// Glitch d'origine : allocation dont l'échec n'est pas testé (voir `Pool::alloc_unchecked`).
        fn alloue_sans_test(&mut self, b: &mut Cps2Bus, pool: &Pool) -> Obj;
        /// Drapeaux d'un `move`/`tst`/`and` de `v`.
        fn drapeaux(&mut self, _v: u32, _taille: Size) {}
        /// Drapeau X seul (dernier bit sorti d'un décalage, que les instructions suivantes gardent).
        fn x(&mut self, _bit: bool) {}
        /// `cmp` : drapeaux de `dst - src` (X conservé).
        fn comparaison(&mut self, _src: u32, _dst: u32, _taille: Size) {}
        /// `add` : résultat et drapeaux.
        fn addition(&mut self, src: u32, dst: u32, taille: Size) -> u32 {
            masque(dst.wrapping_add(src), taille)
        }
        /// `sub` : `dst - src`, résultat et drapeaux.
        fn soustraction(&mut self, src: u32, dst: u32, taille: Size) -> u32 {
            masque(dst.wrapping_sub(src), taille)
        }
        fn d(&mut self, _n: usize, _v: u32) {}
        fn d_mot(&mut self, _n: usize, _v: u16) {}
        fn d_octet(&mut self, _n: usize, _v: u8) {}
        fn a(&mut self, _n: usize, _v: u32) {}
        /// `move #1,CCR` : la retenue seule (refus).
        fn retenue_seule(&mut self) {}
    }

    fn masque(v: u32, taille: Size) -> u32 {
        match taille {
            Size::Byte => v & 0xff,
            Size::Word => v & 0xffff,
            _ => v,
        }
    }

    /// Registres que l'original laisse à cet endroit et que rien ne semble relire : écrits comme lui, puis
    /// empoisonnés avec `DDSOM_POISON=restes` pour le prouver avant de supprimer l'écriture.
    #[track_caller]
    #[allow(dead_code)] // outil de preuve : sert pendant une conversion, retiré avec les écritures prouvées mortes
    pub(crate) fn restes(c: &mut Cpu, masque: u32) {
        crate::diag::registres_morts(c, "restes", masque);
    }

    /// Le cœur : aucun registre.
    pub(crate) struct Pur;

    impl Registres for Pur {
        fn alloue(&mut self, b: &mut Cps2Bus, pool: &Pool) -> Option<Obj> {
            pool.alloc(b)
        }
        fn alloue_sans_test(&mut self, b: &mut Cps2Bus, pool: &Pool) -> Obj {
            pool.alloc_unchecked(b)
        }
    }

    /// Les registres de l'original.
    impl Registres for Cpu {
        #[track_caller]
        fn alloue(&mut self, b: &mut Cps2Bus, pool: &Pool) -> Option<Obj> {
            pool.alloc_68k(self, b)
        }
        #[track_caller]
        fn alloue_sans_test(&mut self, b: &mut Cps2Bus, pool: &Pool) -> Obj {
            pool.alloc_unchecked_68k(self, b)
        }
        fn drapeaux(&mut self, v: u32, taille: Size) {
            self.flags_logic(v, taille);
        }
        fn comparaison(&mut self, src: u32, dst: u32, taille: Size) {
            crate::ops::cmp(self, src, dst, taille);
        }
        fn x(&mut self, bit: bool) {
            self.sr = if bit { self.sr | m68k::SR_X } else { self.sr & !m68k::SR_X };
        }
        fn addition(&mut self, src: u32, dst: u32, taille: Size) -> u32 {
            crate::ops::add(self, src, dst, taille)
        }
        fn soustraction(&mut self, src: u32, dst: u32, taille: Size) -> u32 {
            crate::ops::sub(self, src, dst, taille)
        }
        fn d(&mut self, n: usize, v: u32) {
            self.d[n] = v;
        }
        fn d_mot(&mut self, n: usize, v: u16) {
            set_low_word(&mut self.d[n], v);
        }
        fn d_octet(&mut self, n: usize, v: u8) {
            set_low_byte(&mut self.d[n], v);
        }
        fn a(&mut self, n: usize, v: u32) {
            self.a[n] = v;
        }
        fn retenue_seule(&mut self) {
            self.sr = self.sr & 0xff00 | m68k::SR_C;
        }
    }
}

/// Variables de la RAM de travail lues par les routines du lot (en attendant `ram.rs`).
pub(crate) mod var {
    use crate::ram::{Mot, Octet};
    /// Mot bas du compteur d'images (`A5+0x6c`, long) : la phase des effets « une image sur n ».
    pub const IMAGES_MOT_BAS: Mot = Mot::a5(0x6e);
    /// Joueurs vivants (`A5+0x35`, `ram::DIFFICULTE`, mis à jour par 0xcc18e).
    pub const JOUEURS_VIVANTS: Octet = Octet::a5(0x35);
}

/// Appel d'un créateur du lot par un gestionnaire Rust, avec les registres de l'original (A0 = le créateur, A5),
/// déclarés morts ensuite (`sorties`, étiquette `effets` de `DDSOM_POISON`) : étape de preuve avant le cœur.
#[track_caller]
pub(crate) fn regs_68k<T>(c: &mut Cpu, b: &mut Cps2Bus, createur: Obj, sorties: u32, f: impl FnOnce(&mut Cpu, &mut Cps2Bus) -> T) -> T {
    c.a[0] = createur.0;
    c.a[5] = A5;
    let r = f(c, b);
    crate::diag::registres_morts(c, "effets", sorties);
    r
}

/// Joueurs : quatre enregistrements de 0x200 octets.
const PLAYERS: u32 = A5 + 0x5de;
const PLAYER_STRIDE: u32 = 0x200;
/// Caméra alternative (x, profondeur).
const ALT_CAMERA_X: u32 = A5 + 0x556;
const ALT_CAMERA_Y: u32 = A5 + 0x55a;

/// Types d'effets créés ici.
pub(crate) mod kind {
    pub const SPARK: u8 = 4;
    pub const HALO: u8 = 5;
    pub const DEBRIS: u8 = 6;
    pub const FLASH: u8 = 7;
    pub const TRAIL: u8 = 9;
    pub const ATTACK: u8 = 11;
    pub const TWINKLE: u8 = 15;
    pub const INTERFACE: u8 = 17;
    pub const STRIP: u8 = 21;
    pub const VOTE: u8 = 23;
    pub const PICKUP: u8 = 27;
    pub const POSED_SPARK: u8 = 28;
    pub const IMPACT: u8 = 29;
    pub const FISH: u8 = 30;
    pub const COUNTDOWN: u8 = 31;
    pub const MIRRORED: u8 = 35;
    pub const BLOW: u8 = 40;
    pub const TOSS: u8 = 41;
    pub const SCENERY: u8 = 42;
    pub const DEBRIS_RAIN: u8 = 43;
    pub const SPELL: u8 = 44;
    pub const TYPE_46: u8 = 46;
    pub const BEACON: u8 = 47;
    pub const TYPE_48: u8 = 48;
    pub const PROJECTILE: u8 = 51;
    pub const SHIMMER: u8 = 52;
    pub const TYPE_53: u8 = 53;
    pub const TYPE_54: u8 = 54;
    pub const LAUNCHER: u8 = 55;
    pub const SHARDS: u8 = 56;
    pub const TYPE_57: u8 = 57;
    pub const PILLAR: u8 = 67;
    pub const TYPE_68: u8 = 68;
    pub const VICTORY: u8 = 73;
    pub const BUBBLE: u8 = 74;
    pub const TIMER: u8 = 76;
    pub const SOUND_FADE: u8 = 77;
    pub const RAIN: u8 = 83;
    pub const FINALE: u8 = 84;
    pub const WISP: u8 = 86;
    pub const DRIFT: u8 = 90;
    pub const THROWN_SHARD: u8 = 81;
    pub const POSED_FLASH: u8 = 82;
    /// Tirs ennemis (ESHL) créés par deux routines de la famille.
    pub const ESHL_RISE: u8 = 0x47;
    pub const ESHL_SLASH: u8 = 0x48;
}

/// Liens d'un effet vers son créateur (mot d'adresse, ou long pour `DRIFT`).
mod lien {
    pub const PROPRIETAIRE: u32 = 0x50;
    pub const SECOND: u32 = 0x54;
    pub const CREATEUR: u32 = 0x5c;
    pub const DECOR: u32 = 0x46;
}
/// Réglage propre au type.
const REGLAGE: u32 = 0x2f;
const RETOURNE: u32 = 0x2c;

/// Lien `off` de `o` vers `cible` (`move.w An,(off,A1)`), avec les drapeaux du `move`.
fn link(r: &mut impl Registres, b: &mut Cps2Bus, o: Obj, off: u32, cible: u32) {
    o.set_u16(b, off, cible as u16);
    r.drapeaux(cible & 0xffff, Size::Word);
}

/// `move.b v,(off,A1)` avec ses drapeaux.
fn put8(r: &mut impl Registres, b: &mut Cps2Bus, o: Obj, off: u32, v: u8) {
    o.set_u8(b, off, v);
    r.drapeaux(v as u32, Size::Byte);
}

/// `move.w v,(off,A1)` avec ses drapeaux.
fn put16(r: &mut impl Registres, b: &mut Cps2Bus, o: Obj, off: u32, v: u16) {
    o.set_u16(b, off, v);
    r.drapeaux(v as u32, Size::Word);
}

/// Alloue un objet et pose son activité et son type (`move.b #1,(A1)`, `move.b #type,(2,A1)`).
fn spawn_in(r: &mut impl Registres, b: &mut Cps2Bus, pool: &Pool, kind: u8) -> Option<Obj> {
    let o = r.alloue(b, pool)?;
    o.set_u8(b, Obj::ACTIVE, 1);
    put8(r, b, o, Obj::KIND, kind);
    Some(o)
}

fn spawn(r: &mut impl Registres, b: &mut Cps2Bus, kind: u8) -> Option<Obj> {
    spawn_in(r, b, &pools::EFFCT, kind)
}

/// `addq.b #1,(A1)` : l'activité est incrémentée, non posée.
fn bump_active(r: &mut impl Registres, b: &mut Cps2Bus, o: Obj) {
    let v = r.addition(1, o.u8(b, Obj::ACTIVE) as u32, Size::Byte);
    o.set_u8(b, Obj::ACTIVE, v as u8);
}

/// Copie les mots hauts de X, Z, Y du créateur.
fn copy_position(b: &mut Cps2Bus, to: Obj, from: Obj) {
    for off in [Obj::X, Obj::Z, Obj::Y] {
        let v = from.u16(b, off);
        to.set_u16(b, off, v);
    }
}

/// `andi.w #mask,D0` sur `v` : garde les bits de `mask` (D0.w et drapeaux du résultat).
fn and_d0(r: &mut impl Registres, v: u16, mask: u16) -> u16 {
    let v = v & mask;
    r.d_mot(0, v);
    r.drapeaux(v as u32, Size::Word);
    v
}

// ---------------------------------------------------------------------------------------------
// Créateurs simples : type, variante, réglage `+0x2f`, lien au créateur

/// 0xae776 : élément d'interface (type 17 ; curseurs de l'écran de sélection...).
fn interface(r: &mut impl Registres, b: &mut Cps2Bus) -> Option<Obj> {
    spawn(r, b, kind::INTERFACE)
}

/// 0xaf346 : bandeau d'interface (type 21) de variante `variante`.
fn strip(r: &mut impl Registres, b: &mut Cps2Bus, variante: u8) -> Option<Obj> {
    let o = spawn(r, b, kind::STRIP)?;
    put8(r, b, o, Obj::VARIANT, variante);
    Some(o)
}

/// 0xb1a7a (écran de fin de scène, type 31), 0xafe02 (vote du chemin, type 23) : variante, réglage.
fn with_setting(r: &mut impl Registres, b: &mut Cps2Bus, kind: u8, variante: u8, reglage: u8) -> Option<Obj> {
    let o = spawn(r, b, kind)?;
    o.set_u8(b, Obj::VARIANT, variante);
    put8(r, b, o, REGLAGE, reglage);
    Some(o)
}

/// 0xab7d8 : gerbe de débris (type 6) de variante `variante`, liée au créateur par `+0x50`.
pub fn debris(b: &mut Cps2Bus, createur: Obj, variante: u8) -> Option<Obj> {
    debris_par(&mut Pur, b, createur, variante)
}
pub(crate) fn debris_par(r: &mut impl Registres, b: &mut Cps2Bus, createur: Obj, variante: u8) -> Option<Obj> {
    let o = spawn(r, b, kind::DEBRIS)?;
    o.set_u8(b, Obj::VARIANT, variante);
    o.set_u8(b, 0x42, 0);
    link(r, b, o, lien::PROPRIETAIRE, createur.0);
    Some(o)
}

/// 0xabec8 (effet d'attaque, type 11) et 0xb378a (effets d'un coup, type 40) : liés au créateur par `+0x5c`,
/// variante et réglage.
pub(crate) fn attached_par(r: &mut impl Registres, b: &mut Cps2Bus, kind: u8, createur: Obj, variante: u8, reglage: u8) -> Option<Obj> {
    let o = spawn(r, b, kind)?;
    o.set_u16(b, lien::CREATEUR, createur.word());
    o.set_u8(b, Obj::VARIANT, variante);
    put8(r, b, o, REGLAGE, reglage);
    Some(o)
}

/// 0xabec8 : effet d'attaque (type 11) lié au créateur.
pub fn attack(b: &mut Cps2Bus, createur: Obj, variante: u8, reglage: u8) -> Option<Obj> {
    attached_par(&mut Pur, b, kind::ATTACK, createur, variante, reglage)
}

/// 0xb378a : étoiles et étincelles d'un coup (type 40) liées au créateur.
pub fn blow(b: &mut Cps2Bus, createur: Obj, variante: u8, reglage: u8) -> Option<Obj> {
    attached_par(&mut Pur, b, kind::BLOW, createur, variante, reglage)
}

/// `blow` avec les registres de l'original (appelants par adresse : `player/`) : A0 = le créateur, D0.b =
/// variante, D1.b = réglage ; A1, retenue et drapeaux comme l'original.
#[track_caller]
pub fn blow_68k(c: &mut Cpu, b: &mut Cps2Bus, createur: Obj, variante: u8, reglage: u8) -> Option<Obj> {
    c.a[0] = createur.0;
    let r = attached_par(c, b, kind::BLOW, createur, variante, reglage);
    crate::diag::registres_morts(c, "effets", CREATED);
    r
}

/// 0xab708 : halo (type 5) de la variante du créateur, lié par `+0x50`.
pub fn halo(b: &mut Cps2Bus, createur: Obj) -> Option<Obj> {
    halo_par(&mut Pur, b, createur)
}
pub(crate) fn halo_par(r: &mut impl Registres, b: &mut Cps2Bus, createur: Obj) -> Option<Obj> {
    let o = spawn(r, b, kind::HALO)?;
    let v = createur.u8(b, Obj::VARIANT);
    o.set_u8(b, Obj::VARIANT, v);
    link(r, b, o, lien::PROPRIETAIRE, createur.0);
    Some(o)
}

/// `halo` avec les registres de l'original (appelants par adresse : `player/`) : A1, retenue et drapeaux.
#[track_caller]
pub fn halo_68k(c: &mut Cpu, b: &mut Cps2Bus, createur: Obj) -> Option<Obj> {
    c.a[0] = createur.0;
    let r = halo_par(c, b, createur);
    crate::diag::registres_morts(c, "effets", CREATED);
    r
}

/// 0xb1352 : étincelle posée (type 28) de variante `variante`, liée par `+0x5c`.
pub fn posed_spark(b: &mut Cps2Bus, createur: Obj, variante: u8) -> Option<Obj> {
    posed_spark_par(&mut Pur, b, createur, variante)
}
pub(crate) fn posed_spark_par(r: &mut impl Registres, b: &mut Cps2Bus, createur: Obj, variante: u8) -> Option<Obj> {
    let o = spawn(r, b, kind::POSED_SPARK)?;
    o.set_u8(b, Obj::VARIANT, variante);
    link(r, b, o, lien::CREATEUR, createur.0);
    Some(o)
}

/// 0xb141a : étincelle d'impact (type 29, variante 1) de vitesses `vx`, `ax`, liée par `+0x5c`. Cœur.
pub fn impact_spark(b: &mut Cps2Bus, createur: Obj, vx: u16, ax: u16) -> Option<Obj> {
    impact(&mut Pur, b, createur, vx, ax)
}

/// 0xb141a : étincelle d'impact (type 29, variante 1) de vitesses `vx`, `ax`, liée par `+0x5c`.
fn impact(r: &mut impl Registres, b: &mut Cps2Bus, createur: Obj, vx: u16, ax: u16) -> Option<Obj> {
    let o = spawn(r, b, kind::IMPACT)?;
    o.set_u8(b, Obj::VARIANT, 1);
    o.set_u16(b, Obj::VX, vx);
    o.set_u16(b, Obj::AX, ax);
    link(r, b, o, lien::CREATEUR, createur.0);
    Some(o)
}

/// 0xb12c0 : effet de ramassage (type 27) de variante `variante`, attaché au même objet que le créateur
/// (`+0x50`, s'il existe : pointeur négatif).
pub fn pickup(b: &mut Cps2Bus, createur: Obj, variante: u8) -> Option<Obj> {
    pickup_par(&mut Pur, b, createur, variante)
}
pub(crate) fn pickup_par(r: &mut impl Registres, b: &mut Cps2Bus, createur: Obj, variante: u8) -> Option<Obj> {
    let owner = createur.u16(b, lien::PROPRIETAIRE);
    r.drapeaux(owner as u32, Size::Word);
    if (owner as i16) >= 0 {
        return None;
    }
    let o = spawn(r, b, kind::PICKUP)?;
    o.set_u8(b, Obj::VARIANT, variante);
    let owner = createur.u16(b, lien::PROPRIETAIRE);
    link(r, b, o, lien::PROPRIETAIRE, owner as u32);
    Some(o)
}

/// Créateur d'un type à variante fixe, lié au créateur par `+0x5c` (types 51, 73, 74, 82, 84, 86).
fn linked(r: &mut impl Registres, b: &mut Cps2Bus, createur: Obj, kind: u8, variante: Option<u8>) -> Option<Obj> {
    let o = spawn(r, b, kind)?;
    if let Some(v) = variante {
        o.set_u8(b, Obj::VARIANT, v);
    }
    link(r, b, o, lien::CREATEUR, createur.0);
    Some(o)
}

/// 0xb97a2 : éclat posé (type 82, variante laissée telle quelle) lié au créateur.
pub fn posed_flash(b: &mut Cps2Bus, createur: Obj) -> Option<Obj> {
    linked_posed_flash_par(&mut Pur, b, createur)
}
pub(crate) fn linked_posed_flash_par(r: &mut impl Registres, b: &mut Cps2Bus, createur: Obj) -> Option<Obj> {
    linked(r, b, createur, kind::POSED_FLASH, None)
}

/// 0xb8956 : minuteur de scène (type 76) de variante `variante`.
fn timer(r: &mut impl Registres, b: &mut Cps2Bus, variante: u8) -> Option<Obj> {
    let o = spawn(r, b, kind::TIMER)?;
    put8(r, b, o, Obj::VARIANT, variante);
    Some(o)
}

/// 0xba18a : poussière qui dérive (type 90), orientée comme le créateur, liée par le long `+0x46`.
fn drift(r: &mut impl Registres, b: &mut Cps2Bus, createur: Obj) -> Option<Obj> {
    let o = spawn(r, b, kind::DRIFT)?;
    let facing = createur.u8(b, RETOURNE);
    o.set_u8(b, RETOURNE, facing);
    o.set_u32(b, lien::DECOR, createur.0);
    r.drapeaux(createur.0, Size::Long);
    Some(o)
}

/// 0xb66e4 : effet de type 53 ; l'activité est incrémentée.
fn type_53(r: &mut impl Registres, b: &mut Cps2Bus) -> Option<Obj> {
    let o = r.alloue(b, &pools::EFFCT)?;
    bump_active(r, b, o);
    put8(r, b, o, Obj::KIND, kind::TYPE_53);
    Some(o)
}

/// 0xb6a5c : machine (type 55) liée au créateur par `+0x5c` ; activité incrémentée.
pub fn launcher(b: &mut Cps2Bus, createur: Obj) -> Option<Obj> {
    launcher_par(&mut Pur, b, createur)
}
pub(crate) fn launcher_par(r: &mut impl Registres, b: &mut Cps2Bus, createur: Obj) -> Option<Obj> {
    let o = r.alloue(b, &pools::EFFCT)?;
    bump_active(r, b, o);
    o.set_u8(b, Obj::KIND, kind::LAUNCHER);
    link(r, b, o, lien::CREATEUR, createur.0);
    Some(o)
}

/// 0xb6c42 : éclat (type 56) posé à (0xc0, 0x50) de la caméra ; activité incrémentée.
pub fn camera_shard(b: &mut Cps2Bus) -> Option<Obj> {
    camera_shard_par(&mut Pur, b)
}
pub(crate) fn camera_shard_par(r: &mut impl Registres, b: &mut Cps2Bus) -> Option<Obj> {
    let o = r.alloue(b, &pools::EFFCT)?;
    bump_active(r, b, o);
    o.set_u8(b, Obj::KIND, kind::SHARDS);
    let (x, y) = (crate::ram::CAMERA_X.get(b), crate::ram::CAMERA_Y.get(b));
    o.set_u16(b, Obj::X, x);
    o.set_u16(b, Obj::Y, y);
    o.set_u16(b, Obj::X, x.wrapping_add(0xc0));
    let y = r.addition(0x50, y as u32, Size::Word);
    o.set_u16(b, Obj::Y, y as u16);
    Some(o)
}

/// 0xb2ef6 : silhouette et reflet (type 35, variantes 0 et 1) du créateur, dont l'image transmise
/// (`+0xa0`) est remise à zéro.
fn mirrored_pair(r: &mut impl Registres, b: &mut Cps2Bus, createur: Obj) -> Option<Obj> {
    createur.set_u8(b, 0xa0, 0);
    let mut dernier = None;
    for variante in 0..2u8 {
        let o = spawn(r, b, kind::MIRRORED)?;
        o.set_u8(b, Obj::VARIANT, variante);
        o.set_u8(b, REGLAGE, 0);
        link(r, b, o, lien::DECOR, createur.0);
        dernier = Some(o);
    }
    dernier
}

/// 0xb6ba8 : effet de type 57 lié au créateur par `+0x50` : variante, réglage.
fn type_57(r: &mut impl Registres, b: &mut Cps2Bus, createur: Obj, variante: u8, reglage: u8) -> Option<Obj> {
    let o = r.alloue(b, &pools::EFFCT)?;
    o.set_u8(b, Obj::ACTIVE, 1);
    o.set_u16(b, lien::PROPRIETAIRE, createur.word());
    o.set_u8(b, Obj::KIND, kind::TYPE_57);
    o.set_u8(b, Obj::VARIANT, variante);
    put8(r, b, o, REGLAGE, reglage);
    Some(o)
}

/// 0xb68b0 : trois effets de type 54, variantes 0 (laissée telle quelle), 1 et 2.
fn type_54_trio(r: &mut impl Registres, b: &mut Cps2Bus) -> Option<Obj> {
    let mut dernier = None;
    for variante in 0..3u8 {
        let o = r.alloue(b, &pools::EFFCT)?;
        bump_active(r, b, o);
        put8(r, b, o, Obj::KIND, kind::TYPE_54);
        if variante != 0 {
            put8(r, b, o, Obj::VARIANT, variante);
        }
        dernier = Some(o);
    }
    dernier
}

// ---------------------------------------------------------------------------------------------
// Créateurs qui placent l'effet sur leur créateur

/// 0xae3ec : scintillement (type 15) de la variante `+0x54` du créateur, posé 16 pixels plus haut, lié
/// par `+0x5c` ; rien si `+0x54` est nul.
pub fn twinkle(b: &mut Cps2Bus, createur: Obj) -> Option<Obj> {
    twinkle_par(&mut Pur, b, createur)
}
pub(crate) fn twinkle_par(r: &mut impl Registres, b: &mut Cps2Bus, createur: Obj) -> Option<Obj> {
    let variante = createur.u8(b, lien::SECOND);
    r.drapeaux(variante as u32, Size::Byte);
    if variante == 0 {
        return None;
    }
    let o = spawn(r, b, kind::TWINKLE)?;
    o.set_u8(b, Obj::VARIANT, variante);
    copy_position(b, o, createur);
    let z = o.u16(b, Obj::Z).wrapping_add(0x10);
    o.set_u16(b, Obj::Z, z);
    link(r, b, o, lien::CREATEUR, createur.0);
    Some(o)
}

/// 0xb6596 : scintillement de personnage (type 52) posé juste devant le créateur, relevé selon son octet
/// `+0x60` (table de mots 0xb65d6), lié par `+0x5c`. D0 = la hausse.
pub fn shimmer(b: &mut Cps2Bus, createur: Obj) -> Option<Obj> {
    shimmer_par(&mut Pur, b, createur)
}
pub(crate) fn shimmer_par(r: &mut impl Registres, b: &mut Cps2Bus, createur: Obj) -> Option<Obj> {
    let o = spawn(r, b, kind::SHIMMER)?;
    copy_position(b, o, createur);
    let y = o.u16(b, Obj::Y).wrapping_sub(1);
    o.set_u16(b, Obj::Y, y);
    o.set_u16(b, lien::CREATEUR, createur.word());
    let k = 2 * createur.u8(b, 0x60) as u32;
    let lift = prog16(b, 0xb65d6 + k);
    r.d(0, lift as u32);
    let z = r.addition(lift as u32, o.u16(b, Obj::Z) as u32, Size::Word);
    o.set_u16(b, Obj::Z, z as u16);
    Some(o)
}

/// 0xb5730 : repère (type 47) à la place du créateur : réglage `+0x46`, même variante.
fn beacon(r: &mut impl Registres, b: &mut Cps2Bus, createur: Obj) -> Option<Obj> {
    let o = spawn(r, b, kind::BEACON)?;
    let setting = createur.u8(b, 0x46);
    o.set_u8(b, REGLAGE, setting);
    let v = createur.u8(b, Obj::VARIANT);
    o.set_u8(b, Obj::VARIANT, v);
    copy_position(b, o, createur);
    r.drapeaux(createur.u16(b, Obj::Y) as u32, Size::Word);
    Some(o)
}

/// 0xb3c14 : objet lancé (type 41) depuis le créateur, 24 pixels plus haut, même orientation.
fn toss(r: &mut impl Registres, b: &mut Cps2Bus, createur: Obj) -> Option<Obj> {
    let o = spawn(r, b, kind::TOSS)?;
    let facing = createur.u8(b, RETOURNE);
    o.set_u8(b, RETOURNE, facing);
    copy_position(b, o, createur);
    let z = r.addition(0x18, o.u16(b, Obj::Z) as u32, Size::Word);
    o.set_u16(b, Obj::Z, z as u16);
    Some(o)
}

/// 0xaba16 : éclat à animation unique (type 7) du créateur : orientation, position complète (longs) juste
/// devant lui, réglage `+0x2f`.
pub fn flash(b: &mut Cps2Bus, createur: Obj) -> Option<Obj> {
    flash_par(&mut Pur, b, createur)
}
pub(crate) fn flash_par(r: &mut impl Registres, b: &mut Cps2Bus, createur: Obj) -> Option<Obj> {
    let o = spawn(r, b, kind::FLASH)?;
    let facing = createur.u8(b, RETOURNE);
    o.set_u8(b, RETOURNE, facing);
    for off in [Obj::X, Obj::Z, Obj::Y] {
        let v = rd32(b, createur.0 + off);
        wr32(b, o.0 + off, v);
    }
    let y = r.soustraction(1, o.u32(b, Obj::Y), Size::Long);
    o.set_u32(b, Obj::Y, y);
    let setting = createur.u8(b, REGLAGE);
    put8(r, b, o, REGLAGE, setting);
    Some(o)
}

/// 0xaaa40 : étincelle (type 4, variante `variante`) entre le créateur et `autre`, posée en (x, z, y).
fn spark_at(r: &mut impl Registres, b: &mut Cps2Bus, createur: Obj, autre: u32, variante: u8, (x, z, y): (u16, u16, u16)) -> Option<Obj> {
    let o = spawn(r, b, kind::SPARK)?;
    o.set_u8(b, Obj::VARIANT, variante);
    let facing = createur.u8(b, RETOURNE) ^ 0x20;
    o.set_u8(b, RETOURNE, facing);
    o.set_u16(b, lien::PROPRIETAIRE, autre as u16);
    o.set_u16(b, lien::SECOND, createur.word());
    o.set_u16(b, Obj::X, x);
    o.set_u16(b, Obj::Z, z);
    o.set_u16(b, Obj::Y, y);
    r.drapeaux(y as u32, Size::Word);
    Some(o)
}

// ---------------------------------------------------------------------------------------------
// Effets de décor (type 42) liés par `+0x46`

/// Effet de décor (type 42) de variante `variante` et de réglage `reglage`, lié par `+0x46`.
fn scenery(r: &mut impl Registres, b: &mut Cps2Bus, createur: Obj, variante: u8, reglage: Option<u8>) -> Option<Obj> {
    let o = spawn(r, b, kind::SCENERY)?;
    o.set_u8(b, Obj::VARIANT, variante);
    if let Some(s) = reglage {
        o.set_u8(b, REGLAGE, s);
    }
    link(r, b, o, lien::DECOR, createur.0);
    Some(o)
}

/// Boucle `dbf Dn` de `dernier` à 0 : `corps(r, b, n)` à chaque tour ; le compteur (mot bas de `Dn`) passe au
/// processeur après chaque tour. S'arrête au premier `None` (pool plein) ; rend le dernier objet créé.
fn repete<R: Registres>(
    r: &mut R,
    b: &mut Cps2Bus,
    dn: usize,
    dernier: u16,
    mut corps: impl FnMut(&mut R, &mut Cps2Bus, u16) -> Option<Obj>,
) -> Option<Obj> {
    let mut n = dernier;
    loop {
        let o = corps(r, b, n)?;
        n = n.wrapping_sub(1);
        r.d_mot(dn, n);
        if n == 0xffff {
            return Some(o);
        }
    }
}

/// Plusieurs effets de décor : D1 compte de `dernier` à 0, réglage = D1 ou nul.
fn scenery_burst(r: &mut impl Registres, b: &mut Cps2Bus, createur: Obj, variante: u8, dernier: u16, reglage_compteur: bool) -> Option<Obj> {
    r.d(1, dernier as u32);
    repete(r, b, 1, dernier, |r, b, n| scenery(r, b, createur, variante, Some(if reglage_compteur { n as u8 } else { 0 })))
}

/// 0xb3df6 : deux éclats brefs (variante 0, réglages 0 et 1) posés sur le créateur (x et profondeur).
fn scenery_0_pair(r: &mut impl Registres, b: &mut Cps2Bus, createur: Obj) -> Option<Obj> {
    let mut dernier = None;
    for setting in 0..2u8 {
        let o = spawn(r, b, kind::SCENERY)?;
        o.set_u8(b, Obj::VARIANT, 0);
        o.set_u8(b, REGLAGE, setting);
        let (x, y) = (createur.u16(b, Obj::X), createur.u16(b, Obj::Y));
        o.set_u16(b, Obj::X, x);
        o.set_u16(b, Obj::Y, y);
        link(r, b, o, lien::DECOR, createur.0);
        dernier = Some(o);
    }
    dernier
}

/// 0xb3f6a : une image sur quatre (`A5+0x6e`), bouffée (variante 4) de réglage tiré au hasard ;
/// `+0x58` = bit 2 du tirage.
fn scenery_4(r: &mut impl Registres, b: &mut Cps2Bus, createur: Obj) -> Option<Obj> {
    let frame = var::IMAGES_MOT_BAS.get(b);
    r.d_mot(0, frame);
    r.d_mot(1, frame);
    if and_d0(r, frame, 3) != 0 {
        return None;
    }
    let o = spawn(r, b, kind::SCENERY)?;
    o.set_u8(b, Obj::VARIANT, 4);
    let alea = crate::rng::random(b);
    let v = and_d0(r, alea, 7);
    o.set_u8(b, REGLAGE, v as u8);
    let high = (v & 4) >> 2;
    r.d_mot(0, high);
    o.set_u8(b, 0x58, high as u8);
    link(r, b, o, lien::DECOR, createur.0);
    Some(o)
}

/// 0xb3fd4 (variante 6, une fois sur seize) et 0xb4088 (variante 10, réglage nul, une fois sur quatre).
fn scenery_by_chance(r: &mut impl Registres, b: &mut Cps2Bus, createur: Obj, masque: u16, variante: u8, reglage: Option<u8>) -> Option<Obj> {
    let alea = crate::rng::random(b);
    if and_d0(r, alea, masque) != 0 {
        return None;
    }
    scenery(r, b, createur, variante, reglage)
}

// ---------------------------------------------------------------------------------------------
// Sorts d'un ennemi magicien (type 44) liés par `+0x5c`

/// 0xb4f44 : un effet de sort, réglage `reglage` (variante laissée telle quelle).
fn spell(r: &mut impl Registres, b: &mut Cps2Bus, createur: Obj, reglage: u8) -> Option<Obj> {
    let o = spawn(r, b, kind::SPELL)?;
    o.set_u8(b, REGLAGE, reglage);
    link(r, b, o, lien::CREATEUR, createur.0);
    Some(o)
}

/// 0xb4ef4 / 0xb4f60 / 0xb4f1c : six effets de sort (variantes 5 à 0) de réglage `reglage`.
fn spell_ring(r: &mut impl Registres, b: &mut Cps2Bus, createur: Obj, reglage: u8) -> Option<Obj> {
    r.d(1, 5);
    repete(r, b, 1, 5, |r, b, n| {
        let o = spawn(r, b, kind::SPELL)?;
        o.set_u8(b, Obj::VARIANT, n as u8);
        o.set_u8(b, REGLAGE, reglage);
        link(r, b, o, lien::CREATEUR, createur.0);
        Some(o)
    })
}

/// 0xb3e4e, 0xb3ec6 : cris d'un boss (effet de scène 42, variante `variante`) : une fiche tirée au hasard
/// (`rng & 0x38`, octets en `table`) donne le nombre d'effets moins un, puis le réglage `+0x2f` de chacun ;
/// liés au créateur par `+0x46`. A2 suit la lecture de la fiche.
fn shouts(r: &mut impl Registres, b: &mut Cps2Bus, createur: Obj, variante: u8, table: u32) -> Option<Obj> {
    let alea = crate::rng::random(b);
    let mut at = table + and_d0(r, alea, 0x38) as u32;
    let n = rd8(b, at);
    r.d(1, n as u32);
    at += 1;
    repete(r, b, 1, n as u16, |r, b, _| {
        r.a(2, at);
        let o = spawn(r, b, kind::SCENERY)?;
        o.set_u8(b, Obj::VARIANT, variante);
        let setting = rd8(b, at);
        at += 1;
        r.a(2, at);
        o.set_u8(b, REGLAGE, setting);
        link(r, b, o, lien::DECOR, createur.0);
        Some(o)
    })
}

/// 0xb4bcc : pluie de débris (type 43) de variante `variante`, réglage `reglage`, liée par `+0x5c`.
fn debris_rain(r: &mut impl Registres, b: &mut Cps2Bus, createur: Obj, variante: u8, reglage: u8) -> Option<Obj> {
    let o = spawn(r, b, kind::DEBRIS_RAIN)?;
    o.set_u8(b, Obj::VARIANT, variante);
    o.set_u8(b, REGLAGE, reglage);
    link(r, b, o, lien::CREATEUR, createur.0);
    Some(o)
}

/// 0xb53f4 : trois effets 46 (réglages 2, 1, 0) liés par `+0x46`.
fn type_46_trio(r: &mut impl Registres, b: &mut Cps2Bus, createur: Obj) -> Option<Obj> {
    let mut dernier = None;
    for setting in [2u8, 1, 0] {
        let o = spawn(r, b, kind::TYPE_46)?;
        o.set_u8(b, Obj::VARIANT, 0);
        o.set_u8(b, REGLAGE, setting);
        link(r, b, o, lien::DECOR, createur.0);
        dernier = Some(o);
    }
    dernier
}

/// 0xb59a6, 0xb59c8 : effet 48 de variante `variante`, lié par `+0x46`.
fn type_48(r: &mut impl Registres, b: &mut Cps2Bus, createur: Obj, variante: u8) -> Option<Obj> {
    let o = spawn(r, b, kind::TYPE_48)?;
    o.set_u8(b, Obj::VARIANT, variante);
    o.set_u8(b, REGLAGE, 0);
    link(r, b, o, lien::DECOR, createur.0);
    Some(o)
}

/// 0xb96c0 : éclat lancé (type 81) de variante `variante`, lié au joueur par `+0x50`.
fn thrown_shard(r: &mut impl Registres, b: &mut Cps2Bus, createur: Obj, variante: u8) -> Option<Obj> {
    let o = spawn(r, b, kind::THROWN_SHARD)?;
    put8(r, b, o, Obj::VARIANT, variante);
    link(r, b, o, lien::PROPRIETAIRE, createur.0);
    Some(o)
}

/// 0xb230e : bloc de 2 × 2 tuiles écrit en `at` (tuile `tile`, attribut `attr` ; rangée suivante 0x80 plus
/// loin) ; rend l'adresse après les deux rangées (A1) et la tuile + 0x11 (D0).
fn tile_block(b: &mut Cps2Bus, mut at: u32, mut tile: u16, attr: u16) -> (u32, u16) {
    for _ in 0..2 {
        wr16(b, at, tile);
        wr16(b, at + 2, attr);
        wr16(b, at + 4, tile.wrapping_add(0x10));
        wr16(b, at + 6, attr);
        at = at.wrapping_add(0x80);
        tile = tile.wrapping_add(1);
    }
    (at, tile.wrapping_add(0xf))
}

// ---------------------------------------------------------------------------------------------
// Créateurs de scène

/// Poisson (type 30) posé sur la caméra alternative.
fn fish_at_camera(b: &mut Cps2Bus, o: Obj) {
    let (x, y) = (rd16(b, ALT_CAMERA_X), rd16(b, ALT_CAMERA_Y));
    o.set_u16(b, Obj::X, x);
    o.set_u16(b, Obj::Y, y);
}

/// 0xb1764 : trois poissons (variante 0) qui traversent, 0x20 pixels à gauche de la caméra alternative, à
/// 0x40 de hauteur ; profondeur et vitesse (`+0x58`) lues en 0xb17ae (D4 : rang dans la table, D5 : compteur).
fn fish_school(r: &mut impl Registres, b: &mut Cps2Bus) -> Option<Obj> {
    r.d(4, 0);
    r.d(5, 2);
    let mut k: u16 = 0;
    repete(r, b, 5, 2, |r, b, _| {
        let o = spawn(r, b, kind::FISH)?;
        o.set_u8(b, RETOURNE, 0x20);
        fish_at_camera(b, o);
        o.set_u16(b, Obj::Z, 0xffc0);
        let x = o.u16(b, Obj::X).wrapping_sub(0x20);
        o.set_u16(b, Obj::X, x);
        let dy = prog16(b, 0xb17ae + k as u32);
        r.d_mot(0, dy);
        let y = o.u16(b, Obj::Y).wrapping_add(dy);
        o.set_u16(b, Obj::Y, y);
        let speed = prog16(b, 0xb17b0 + k as u32);
        put16(r, b, o, 0x58, speed);
        k += 4;
        r.d_mot(4, k);
        Some(o)
    })
}

/// 0xb17ba : quatre poissons (variante 1) autour de la caméra alternative (décalages en 0xb1808), un sur
/// deux retourné.
fn fish_swimmers(r: &mut impl Registres, b: &mut Cps2Bus) -> Option<Obj> {
    r.d(4, 0);
    r.d(5, 3);
    let mut k: u16 = 0;
    repete(r, b, 5, 3, |r, b, n| {
        let o = spawn(r, b, kind::FISH)?;
        o.set_u8(b, Obj::VARIANT, 1);
        fish_at_camera(b, o);
        let (dx, dy) = (prog16(b, 0xb1808 + k as u32), prog16(b, 0xb180a + k as u32));
        r.d_mot(0, dx);
        r.d_mot(1, dy);
        let x = o.u16(b, Obj::X).wrapping_add(dx);
        o.set_u16(b, Obj::X, x);
        let y = r.addition(dy as u32, o.u16(b, Obj::Y) as u32, Size::Word);
        o.set_u16(b, Obj::Y, y as u16);
        let odd = n as u8 & 1;
        r.d_octet(2, odd);
        r.drapeaux(odd as u32, Size::Byte);
        if odd != 0 {
            let f = o.u8(b, RETOURNE) ^ 0x20;
            put8(r, b, o, RETOURNE, f);
        }
        k += 4;
        r.d_mot(4, k);
        Some(o)
    })
}

/// 0xb7dec : quatre colonnes (type 67, variantes 3 à 0) sous chaque joueur présent et vivant (A2 : le
/// joueur, D6 : compteur des joueurs, D5 : des colonnes).
pub fn pillars(b: &mut Cps2Bus) -> Option<Obj> {
    pillars_par(&mut Pur, b)
}
pub(crate) fn pillars_par(r: &mut impl Registres, b: &mut Cps2Bus) -> Option<Obj> {
    r.d(6, 3);
    let mut dernier = None;
    for k in 0..4u16 {
        let p = Obj(PLAYERS + PLAYER_STRIDE * k as u32);
        r.a(2, p.0);
        if p.u8(b, Obj::ACTIVE) != 0 && p.u16(b, 0x62) != 0 {
            r.d(5, 3);
            dernier = Some(repete(r, b, 5, 3, |r, b, n| {
                let o = spawn(r, b, kind::PILLAR)?;
                o.set_u8(b, Obj::VARIANT, n as u8);
                link(r, b, o, lien::PROPRIETAIRE, p.0);
                Some(o)
            })?);
        } else {
            r.drapeaux(0, Size::Byte);
        }
        r.a(2, p.0 + PLAYER_STRIDE);
        r.d_mot(6, 2u16.wrapping_sub(k));
    }
    dernier
}

/// 0xabb70 : traînées (type 9) de l'entrée de scène. Les deux places de chaque joueur présent sont marquées
/// dans la table `A5-0x5f9a` (6 mots), puis une traînée est créée par place libre restante, `5 - A5+0x35`
/// fois.
fn trails(r: &mut impl Registres, b: &mut Cps2Bus) -> Option<Obj> {
    const SEATS: u32 = A5 - 0x5f9a;
    for k in 0..3 {
        wr32(b, SEATS + 4 * k, 0);
    }
    r.a(2, SEATS + 8);
    r.d(6, 3);
    for k in 0..4u16 {
        let p = PLAYERS + PLAYER_STRIDE * k as u32;
        if rd8(b, p) & 1 != 0 {
            let seat = (rd8(b, p + 3) & 0xe) as u32;
            r.d(0, seat);
            r.a(2, SEATS);
            wr16(b, SEATS + seat, 0x101);
        }
        r.a(1, p + PLAYER_STRIDE);
        r.d_mot(6, 2u16.wrapping_sub(k));
    }
    r.d(0, 0);
    let living = var::JOUEURS_VIVANTS.get(b);
    let mut place: u8 = 0;
    let mut rang = living;
    let mut seat = SEATS;
    r.d(5, 5u8.wrapping_sub(living) as u32);
    r.d_octet(4, living);
    r.a(4, SEATS);
    let n = 5u8.wrapping_sub(living) as u16;
    repete(r, b, 5, n, |r, b, _| {
        // Glitch d'origine : l'échec de l'allocation n'est pas testé (voir `Pool::alloc_unchecked`).
        let o = r.alloue_sans_test(b, &pools::EFFCT);
        o.set_u8(b, Obj::ACTIVE, 1);
        o.set_u8(b, Obj::KIND, kind::TRAIL);
        o.set_u8(b, RETOURNE, 0x20);
        o.set_u8(b, 0x2a, 2);
        o.set_u8(b, 0x52, rang);
        rang = rang.wrapping_add(1);
        r.d_octet(4, rang);
        while rd8(b, seat) != 0 {
            place = place.wrapping_add(2);
            seat += 2;
        }
        r.d_octet(0, place);
        r.a(4, seat);
        o.set_u8(b, Obj::VARIANT, place);
        wr16(b, seat, 0x101);
        r.drapeaux(0x101, Size::Word);
        Some(o)
    })
}

/// 0xa8a42 : caméra de scène en mode 0x60 (`A5+0x51e`), `A5+0x4ff` effacé.
fn camera_mode_60(c: &mut Cpu, b: &mut Cps2Bus) {
    crate::mem::wr8(b, A5 + 0x4ff, 0);
    wr16(b, A5 + 0x51e, 0x60);
    c.flags_logic(0x60, Size::Word);
}

/// 0xa8af8 (et 0xa8ae8, qui prend la liste dans D0) : crée la liste de décors (SET) numéro `liste` (table de
/// pointeurs 0xa8b26) : entrées de 4 octets (type, réglage `+0x2f`, mot dont l'octet bas est la variante),
/// terminées par un type négatif ; s'arrête au premier pool plein. A4 suit la lecture ; D0.w = le mot lu.
pub fn set_list(b: &mut Cps2Bus, liste: u8) -> Option<Obj> {
    set_list_par(&mut Pur, b, liste)
}
pub(crate) fn set_list_par(r: &mut impl Registres, b: &mut Cps2Bus, liste: u8) -> Option<Obj> {
    let k = (liste as u16) << 2;
    r.d_mot(1, k);
    let mut at = prog32(b, 0xa8b26 + k as u32);
    r.a(4, at);
    let mut dernier = None;
    loop {
        let kind = rd8(b, at);
        at += 1;
        r.a(4, at);
        r.d_octet(0, kind);
        r.drapeaux(kind as u32, Size::Byte);
        if (kind as i8) < 0 {
            return dernier;
        }
        let o = r.alloue(b, &pools::SET)?;
        o.set_u8(b, Obj::ACTIVE, 1);
        o.set_u8(b, Obj::KIND, kind);
        let setting = rd8(b, at);
        o.set_u8(b, REGLAGE, setting);
        let word = rd16(b, at + 1);
        at += 3;
        r.a(4, at);
        r.d_mot(0, word);
        put8(r, b, o, Obj::VARIANT, word as u8);
        dernier = Some(o);
    }
}

// ---------------------------------------------------------------------------------------------
// Tirs ennemis (ESHL) créés par la famille

/// 0xa836c : coup (ESHL 0x48, variante 0) de réglage `reglage`, lié par `+0x5c`.
fn eshl_slash(r: &mut impl Registres, b: &mut Cps2Bus, createur: Obj, reglage: u8) -> Option<Obj> {
    let o = spawn_in(r, b, &pools::ESHL, kind::ESHL_SLASH)?;
    o.set_u8(b, Obj::VARIANT, 0);
    o.set_u8(b, REGLAGE, reglage);
    link(r, b, o, lien::CREATEUR, createur.0);
    Some(o)
}

/// 0xa817c : colonne montante (ESHL 0x47) posée sur le créateur, 0x80 plus haut, juste devant ; D0 = 1, ou 0 si le
/// pool est plein.
fn eshl_rise(r: &mut impl Registres, b: &mut Cps2Bus, createur: Obj) -> Option<Obj> {
    let Some(o) = spawn_in(r, b, &pools::ESHL, kind::ESHL_RISE) else {
        r.d(0, 0);
        r.drapeaux(0, Size::Long);
        return None;
    };
    o.set_u8(b, Obj::VARIANT, 0);
    o.set_u8(b, REGLAGE, 0);
    o.set_u16(b, lien::DECOR, createur.word());
    let facing = createur.u8(b, RETOURNE);
    o.set_u8(b, RETOURNE, facing);
    copy_position(b, o, createur);
    // `addi.w`, `subi.w` : leur X reste après `moveq`
    let z = r.addition(0x80, o.u16(b, Obj::Z) as u32, Size::Word);
    o.set_u16(b, Obj::Z, z as u16);
    let y = r.soustraction(1, o.u16(b, Obj::Y) as u32, Size::Word);
    o.set_u16(b, Obj::Y, y as u16);
    r.d(0, 1);
    r.drapeaux(1, Size::Long);
    Some(o)
}

// ---------------------------------------------------------------------------------------------
// Natives : les registres de l'original (A0 = le créateur, D0/D1... = paramètres)

const fn creator(entry: u32, name: &'static str, run: fn(&mut Cpu, &mut Cps2Bus), outputs: u32) -> Native {
    Native { entry, name, run: crate::Corps::Processeur(run), touches_sound: false, outputs }
}

/// Déclare les natives : `adresse "nom" |c, b, me| corps, sorties;` (`me` = A0, le créateur) ; chaque native se
/// termine par un RTS.
macro_rules! creators {
    ($( $entry:literal $name:literal |$c:ident, $b:ident, $me:ident| $body:expr, $out:expr; )*) => {
        pub const NATIVES: &[Native] = &[ $(
            creator($entry, $name, |$c, $b| {
                let $me = Obj($c.a[0]);
                let _ = $me;
                let _ = $body;
                rts($c, $b)
            }, $out),
        )* ];
    };
}

/// Paramètres lus dans les registres.
fn d0b(c: &Cpu) -> u8 {
    c.d[0] as u8
}
fn d1b(c: &Cpu) -> u8 {
    c.d[1] as u8
}

/// 0xaaa40 : A1 à A4 sont rendus.
fn spark_at_native(c: &mut Cpu, b: &mut Cps2Bus, me: Obj) {
    let saved = [c.a[1], c.a[2], c.a[3], c.a[4]];
    let (autre, variante, pos) = (c.a[1], d1b(c), (c.d[2] as u16, c.d[3] as u16, c.d[4] as u16));
    spark_at(c, b, me, autre, variante, pos);
    [c.a[1], c.a[2], c.a[3], c.a[4]] = saved;
}

fn tile_block_native(c: &mut Cpu, b: &mut Cps2Bus) {
    let (at, tile) = tile_block(b, c.a[1], c.d[0] as u16, c.d[1] as u16);
    c.a[1] = at;
    set_low_word(&mut c.d[0], tile);
}

pub(crate) const CREATED: u32 = regs::a(1) | regs::CCR;
use regs::d as D;

creators! {
    0xae776 "cree_interface" |c, b, me| interface(c, b), CREATED;
    0xaf346 "cree_bandeau" |c, b, me| { let v = d0b(c); strip(c, b, v) }, CREATED;
    0xb1a7a "cree_ecran_fin" |c, b, me| { let (v, s) = (d0b(c), d1b(c)); with_setting(c, b, kind::COUNTDOWN, v, s) }, CREATED;
    0xafe02 "cree_vote" |c, b, me| { let (v, s) = (d0b(c), d1b(c)); with_setting(c, b, kind::VOTE, v, s) }, CREATED;
    0xab7d8 "cree_gerbe_debris" |c, b, me| { let v = d0b(c); debris_par(c, b, me, v) }, CREATED;
    0xabec8 "cree_effet_attaque" |c, b, me| { let (v, s) = (d0b(c), d1b(c)); attached_par(c, b, kind::ATTACK, me, v, s) }, CREATED;
    0xb378a "cree_coup" |c, b, me| { let (v, s) = (d0b(c), d1b(c)); attached_par(c, b, kind::BLOW, me, v, s) }, CREATED;
    0xab708 "cree_halo" |c, b, me| halo_par(c, b, me), CREATED;
    0xb1352 "cree_etincelle_posee" |c, b, me| { let v = d0b(c); posed_spark_par(c, b, me, v) }, CREATED;
    0xb141a "cree_impact" |c, b, me| { let (vx, ax) = (c.d[2] as u16, c.d[3] as u16); impact(c, b, me, vx, ax) }, CREATED;
    0xb12c0 "cree_ramassage" |c, b, me| { let v = d0b(c); pickup_par(c, b, me, v) }, CREATED;
    0xb9c6a "cree_flammeche" |c, b, me| linked(c, b, me, kind::WISP, Some(0)), CREATED;
    0xb9c88 "cree_flammeche_b" |c, b, me| linked(c, b, me, kind::WISP, Some(1)), CREATED;
    0xb9aa0 "cree_finale" |c, b, me| linked(c, b, me, kind::FINALE, None), CREATED;
    0xb84ae "cree_ecran_victoire" |c, b, me| linked(c, b, me, kind::VICTORY, None), CREATED;
    0xb85e2 "cree_bulle_createur" |c, b, me| linked(c, b, me, kind::BUBBLE, None), CREATED;
    0xb9942 "cree_pluie" |c, b, me| spawn(c, b, kind::RAIN), CREATED;
    0xb7f14 "cree_type_68" |c, b, me| spawn(c, b, kind::TYPE_68), CREATED;
    0xb8956 "cree_minuteur" |c, b, me| { let v = d1b(c); timer(c, b, v) }, CREATED;
    0xba18a "cree_derive" |c, b, me| drift(c, b, me), CREATED;
    0xb66e4 "cree_type_53" |c, b, me| type_53(c, b), CREATED;
    0xb6a5c "cree_lanceur" |c, b, me| launcher_par(c, b, me), CREATED;
    0xb6c42 "cree_eclat_camera" |c, b, me| camera_shard_par(c, b), CREATED;
    0xb2ef6 "cree_silhouette_reflet" |c, b, me| mirrored_pair(c, b, me), CREATED;
    0xb6ba8 "cree_type_57" |c, b, me| { let (v, s) = (d0b(c), d1b(c)); type_57(c, b, me, v, s) }, CREATED;
    0xb8a0a "cree_fondu_sonore" |c, b, me| spawn(c, b, kind::SOUND_FADE), CREATED;
    0xb68b0 "cree_types_54" |c, b, me| type_54_trio(c, b), CREATED;
    0xae3ec "cree_scintillement" |c, b, me| twinkle_par(c, b, me), CREATED;
    0xb6596 "cree_scintillement_perso" |c, b, me| shimmer_par(c, b, me), CREATED | D(0);
    0xb5730 "cree_repere" |c, b, me| beacon(c, b, me), CREATED;
    0xb3c14 "cree_lance" |c, b, me| toss(c, b, me), CREATED;
    0xaba16 "cree_eclat_unique" |c, b, me| flash_par(c, b, me), CREATED;
    0xaaa40 "cree_etincelle_en" |c, b, me| spark_at_native(c, b, me), regs::CCR;
    0xb3df6 "cree_decor_0" |c, b, me| scenery_0_pair(c, b, me), CREATED;
    0xb3f40 "cree_decor_3" |c, b, me| scenery_burst(c, b, me, 3, 6, true), CREATED | D(1);
    0xb3f6a "cree_decor_4" |c, b, me| scenery_4(c, b, me), CREATED | D(0) | D(1);
    0xb3fb0 "cree_decor_5" |c, b, me| { let s = d1b(c); scenery(c, b, me, 5, Some(s)) }, CREATED;
    0xb3fd4 "cree_decor_6" |c, b, me| scenery_by_chance(c, b, me, 0xf, 6, None), CREATED | D(0);
    0xb4002 "cree_decor_7" |c, b, me| scenery(c, b, me, 7, Some(0)), CREATED;
    0xb4026 "cree_decor_8" |c, b, me| scenery_burst(c, b, me, 8, 9, false), CREATED | D(1);
    0xb4088 "cree_decor_10" |c, b, me| scenery_by_chance(c, b, me, 3, 10, Some(0)), CREATED | D(0);
    0xb4f44 "cree_sort" |c, b, me| { let s = d1b(c); spell(c, b, me, s) }, CREATED;
    0xb4ef4 "cree_sorts" |c, b, me| spell_ring(c, b, me, 0), CREATED | D(1);
    0xb4f60 "cree_sorts_b" |c, b, me| spell_ring(c, b, me, 2), CREATED | D(1);
    0xb1764 "cree_banc" |c, b, me| fish_school(c, b), CREATED | D(0) | D(4) | D(5);
    0xb17ba "cree_nageurs" |c, b, me| fish_swimmers(c, b), CREATED | D(0) | D(1) | D(2) | D(4) | D(5);
    0xb7dec "cree_colonnes" |c, b, me| pillars_par(c, b), CREATED | D(5) | D(6) | regs::a(2);
    0xabb70 "cree_trainees" |c, b, me| trails(c, b), CREATED | D(0) | D(4) | D(5) | D(6) | regs::a(2) | regs::a(4);
    0xa8a42 "camera_mode_60" |c, b, me| camera_mode_60(c, b), regs::CCR;
    0xa8ae8 "cree_liste_decors_d0" |c, b, me| { let v = c.d[0] as u16; set_low_word(&mut c.d[1], v); set_list_par(c, b, v as u8) }, CREATED | D(0) | D(1) | regs::a(4);
    0xa8af8 "cree_liste_decors" |c, b, me| { let v = d1b(c); set_list_par(c, b, v) }, CREATED | D(0) | D(1) | regs::a(4);
    0xb3e4e "cree_cris" |c, b, me| shouts(c, b, me, 1, 0xb3e86), CREATED | D(0) | D(1) | regs::a(2);
    0xb3ec6 "cree_cris_b" |c, b, me| shouts(c, b, me, 2, 0xb3f00), CREATED | D(0) | D(1) | regs::a(2);
    0xb4bcc "cree_pluie_debris" |c, b, me| { let (v, s) = (d1b(c), c.d[2] as u8); debris_rain(c, b, me, v, s) }, CREATED;
    0xb4f1c "cree_sorts_c" |c, b, me| spell_ring(c, b, me, 4), CREATED | D(1);
    0xb53f4 "cree_type_46" |c, b, me| type_46_trio(c, b, me), CREATED;
    0xb59a6 "cree_type_48" |c, b, me| type_48(c, b, me, 1), CREATED;
    0xb59c8 "cree_type_48_b" |c, b, me| type_48(c, b, me, 2), CREATED;
    0xb64b0 "cree_projectile" |c, b, me| linked(c, b, me, kind::PROJECTILE, None), CREATED;
    0xb96c0 "cree_eclat_lance" |c, b, me| { let v = d1b(c); thrown_shard(c, b, me, v) }, CREATED;
    0xb97a2 "cree_eclat_pose" |c, b, me| linked(c, b, me, kind::POSED_FLASH, None), CREATED;
    0xb97ba "cree_eclat_pose_b" |c, b, me| linked(c, b, me, kind::POSED_FLASH, Some(1)), CREATED;
    0xb230e "bloc_tuiles" |c, b, me| tile_block_native(c, b), regs::d(0) | regs::a(1);
    0xa836c "cree_tir_coup" |c, b, me| { let s = c.d[4] as u8; eshl_slash(c, b, me, s) }, CREATED;
    0xa817c "cree_tir_colonne" |c, b, me| eshl_rise(c, b, me), CREATED | D(0);
}
