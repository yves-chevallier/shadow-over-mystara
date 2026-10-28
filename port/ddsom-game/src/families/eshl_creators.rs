//! Créateurs de tirs ennemis (ESHL) appelés par les gestionnaires d'ennemis et de boss : chacun alloue un
//! ou plusieurs objets ESHL (`0x22ea`), pose le type, la variante et le lien vers le créateur.
//!
//! Chaque créateur est une fonction Rust ordinaire (créateur et réglages en arguments ; le dernier objet créé en
//! retour, `None` si le pool est plein), écrite une seule fois sur un [`Registres`] comme ceux d'`effects_creators` :
//! le cœur ([`Pur`]) ne touche aucun registre ; la native, pour les appels par adresse restants, passe le processeur,
//! qui reçoit ce que l'original laisse (A1 = le dernier objet alloué, retenue levée si le pool est plein, drapeaux de
//! la dernière écriture, compteurs de boucle : `dbf` laisse le mot bas à -1).

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::Cpu;

use super::effects_creators::restes::Registres;
use crate::mem::{prog16, prog8, rd16, rts, wr16, wr32, wr8, A5};
use crate::objects::pools;
use crate::objects::Obj;
use crate::{regs, Native};

mod f {
    pub const ACTIVE: u32 = 0x00;
    pub const KIND: u32 = 0x02;
    pub const VARIANT: u32 = 0x03;
    pub const X: u32 = 0x08;
    pub const Z: u32 = 0x0c;
    pub const Y: u32 = 0x10;
    pub const VX: u32 = 0x14;
    pub const VZ: u32 = 0x18;
    pub const SWING: u32 = 0x1c;
    pub const MODE: u32 = 0x2a;
    pub const FLIP: u32 = 0x2c;
    pub const DIRECTION: u32 = 0x2d;
    pub const INDEX: u32 = 0x2f;
    pub const CREATOR: u32 = 0x42;
    pub const PARENT: u32 = 0x46;
    pub const OWNER: u32 = 0x50;
    pub const LINK: u32 = 0x5c;
}

/// Rang dynamique (`A5+0x7f92`, mot) : choisit le nombre d'éclats des rangées.
const RANG: u32 = A5 + 0x7f92;
/// Nombre de tirs de type 5 au-delà duquel 0x9e426 n'en crée plus (`A5+0x7e64`, mot).
const LIMITE_DOUZE: u32 = A5 + 0x7e64;

/// Alloue un ESHL de type `kind` (actif) ; `None` (retenue levée) si le pool est plein.
fn spawn(r: &mut impl Registres, b: &mut Cps2Bus, kind: u8) -> Option<Obj> {
    let o = r.alloue(b, &pools::ESHL)?;
    o.set_u8(b, f::ACTIVE, 1);
    o.set_u8(b, f::KIND, kind);
    Some(o)
}

/// Copie la position (mots X, Z, Y) du créateur.
fn copy_position(b: &mut Cps2Bus, from: Obj, to: Obj) {
    for off in [f::X, f::Z, f::Y] {
        let v = from.u16(b, off);
        to.set_u16(b, off, v);
    }
}

/// Fragments numérotés : `count` objets de type `kind`, variante `variant`, indice (`+0x2f`) de `count - 1`
/// à 0 (sauf `skip`), liés au créateur (`+0x46`), à sa position ; orientation : celle du créateur, ou
/// `flip` imposée. D1.w sert de compteur. Faux si le pool est plein avant la fin.
fn fragments(r: &mut impl Registres, b: &mut Cps2Bus, me: Obj, kind: u8, variant: u8, count: u16, skip: Option<u8>, flip: Option<u8>, creator_kind: bool) -> bool {
    let mut i = count.wrapping_sub(1);
    loop {
        r.d_mot(1, i);
        if skip != Some(i as u8) {
            let Some(o) = spawn(r, b, kind) else { return false };
            o.set_u8(b, f::VARIANT, variant);
            o.set_u8(b, f::INDEX, i as u8);
            o.set_u16(b, f::PARENT, me.word());
            copy_position(b, me, o);
            if creator_kind {
                let k = me.u8(b, f::KIND);
                o.set_u8(b, f::CREATOR, k);
            }
            let v = flip.unwrap_or_else(|| me.u8(b, f::FLIP));
            o.set_u8(b, f::FLIP, v);
            r.drapeaux(v as u32, Size::Byte);
        }
        i = i.wrapping_sub(1);
        if i == 0xffff {
            r.d_mot(1, i);
            return true;
        }
    }
}

/// 0xa7b0a : éclats du boss 11 (type 0x44) ; un tirage choisit leur nombre et celui qui manque (table
/// 0xa7b64 : couples nombre, absent).
pub(crate) fn boss11_shards(r: &mut impl Registres, b: &mut Cps2Bus, me: Obj) {
    let i = crate::rng::random(b) & 0xe;
    r.d_mot(0, i);
    let count = prog8(b, 0xa7b64 + i as u32);
    let missing = prog8(b, 0xa7b65 + i as u32);
    r.d(1, count as u32);
    r.d_octet(2, missing);
    fragments(r, b, me, 0x44, 0, count as u16, Some(missing), None, false);
}

/// 0xa7570 : fragments de type 0x42 de variante `variant`, leur nombre lu dans la table 0xa75b6.
pub(crate) fn fragments_42(r: &mut impl Registres, b: &mut Cps2Bus, me: Obj, variant: u16) {
    let count = prog8(b, 0xa75b6u32.wrapping_add(variant as i16 as i32 as u32));
    r.d(1, count as u32);
    fragments(r, b, me, 0x42, variant as u8, count as u16, None, None, false);
}

/// 0xa7da2 : deux rangées de fragments de type 0x45 (nombre lu en 0xa7d92), tournés à droite (avec le
/// type du créateur en `+0x42`) puis à gauche.
pub(crate) fn fragments_45(r: &mut impl Registres, b: &mut Cps2Bus, me: Obj) {
    r.d(0, 0);
    let count = prog8(b, 0xa7d92) as u16;
    r.d(2, count as u32);
    r.d(1, count as u32);
    if fragments(r, b, me, 0x45, 0, count, None, Some(0), true) {
        r.d_mot(1, count);
        fragments(r, b, me, 0x45, 0, count, None, Some(0x20), false);
    }
}

/// Série d'objets liés par `+0x5c` : variantes `last` à 0 (D5), `+0x2f` = `index`.
fn series(r: &mut impl Registres, b: &mut Cps2Bus, me: Obj, kind: u8, last: u8, index: u8) {
    let mut n = last as u16;
    r.d(5, n as u32);
    loop {
        let Some(o) = spawn(r, b, kind) else { break };
        o.set_u8(b, f::VARIANT, n as u8);
        o.set_u8(b, f::INDEX, index);
        o.set_u16(b, f::LINK, me.word());
        r.drapeaux(me.word() as u32, Size::Word);
        n = n.wrapping_sub(1);
        r.d_mot(5, n);
        if n == 0xffff {
            break;
        }
    }
}

/// 0xa4e84 : 17 objets de type 0x36 (variantes 16 à 0), `+0x2f` = `index`.
pub(crate) fn series_36(r: &mut impl Registres, b: &mut Cps2Bus, me: Obj, index: u8) {
    series(r, b, me, 0x36, 0x10, index);
}

/// 0xa5c9a : 16 objets de type 0x3b (variantes 15 à 0), `+0x2f` = 1.
pub(crate) fn series_3b(r: &mut impl Registres, b: &mut Cps2Bus, me: Obj) {
    series(r, b, me, 0x3b, 0xf, 1);
}

/// Un objet lié au créateur par `+0x5c`, de type `kind`, variante `variant` (None : non posée), `+0x2f` = `index`.
fn linked(r: &mut impl Registres, b: &mut Cps2Bus, me: Obj, kind: u8, variant: Option<u8>, index: u8) -> Option<Obj> {
    let o = spawn(r, b, kind)?;
    if let Some(v) = variant {
        o.set_u8(b, f::VARIANT, v);
    }
    o.set_u8(b, f::INDEX, index);
    o.set_u16(b, f::LINK, me.word());
    r.drapeaux(me.word() as u32, Size::Word);
    Some(o)
}

/// 0xa5c7c : un objet de type 0x3b, `+0x2f` = 0.
pub(crate) fn single_3b(r: &mut impl Registres, b: &mut Cps2Bus, me: Obj) -> Option<Obj> {
    linked(r, b, me, 0x3b, None, 0)
}

/// 0xa6e6a : un objet de type 0x40, variante 0, `+0x2f` = 5.
pub(crate) fn single_40(r: &mut impl Registres, b: &mut Cps2Bus, me: Obj) -> Option<Obj> {
    linked(r, b, me, 0x40, Some(0), 5)
}

/// L'orientation du créateur est recopiée dans l'objet A1 (`move.b D0b,(0x2c,A1)`).
/// Glitch d'origine : quand le pool est plein, A1 est le reste de l'allocation (premier emplacement ou fin du pool) :
/// l'octet y est écrit quand même (type 1 de `thrown`, `sweeping_8`).
fn copy_flip(r: &mut impl Registres, b: &mut Cps2Bus, me: Obj, a1: Obj) {
    let flip = me.u8(b, f::FLIP);
    r.d_octet(0, flip);
    wr8(b, a1.0 + f::FLIP, flip);
    r.drapeaux(flip as u32, Size::Byte);
}

/// Tir lancé devant le créateur (types 1 et 2) : `+0x2a` = 2, angle 8 (0x18 et vitesse retournée s'il
/// regarde à droite), orientation du créateur. `always_flip` : l'orientation est recopiée dans A1 même quand le pool
/// est plein (type 1, voir `copy_flip`).
fn thrown(r: &mut impl Registres, b: &mut Cps2Bus, me: Obj, kind: u8, vx: Option<u32>, always_flip: bool) -> Option<Obj> {
    let a1 = match r.alloue_ou_reste(b, &pools::ESHL) {
        Ok(o) => {
            o.set_u8(b, f::ACTIVE, 1);
            o.set_u8(b, f::KIND, kind);
            if kind == 1 {
                o.set_u8(b, f::INDEX, 0);
            }
            o.set_u8(b, f::MODE, 2);
            o.set_u16(b, f::OWNER, me.word());
            if let Some(v) = vx {
                o.set_u32(b, f::VX, v);
            }
            o.set_u32(b, f::VZ, 0);
            o.set_u8(b, f::DIRECTION, 8);
            if me.u8(b, f::FLIP) == 0 {
                o.set_u8(b, f::DIRECTION, 0x18);
                let v = o.u16(b, f::VX).wrapping_neg();
                o.set_u16(b, f::VX, v);
            }
            o
        }
        Err(_) if !always_flip => return None,
        Err(reste) => reste,
    };
    copy_flip(r, b, me, a1);
    Some(a1)
}

/// 0x9dda8 : tir de type 1 (vitesse laissée à zéro par l'allocation).
pub(crate) fn thrown_1(r: &mut impl Registres, b: &mut Cps2Bus, me: Obj) -> Option<Obj> {
    thrown(r, b, me, 1, None, true)
}

/// 0x9e062 : tir de type 2, vitesse 5 pixels par image.
pub(crate) fn thrown_2(r: &mut impl Registres, b: &mut Cps2Bus, me: Obj) -> Option<Obj> {
    thrown(r, b, me, 2, Some(0x0500_0000), false)
}

/// Cœurs de 0x9dda8 et 0x9e062 pour les gestionnaires qui reçoivent le tir à lancer en paramètre.
pub(crate) fn throw_1(b: &mut Cps2Bus, me: Obj) {
    thrown_1(&mut super::effects_creators::restes::Pur, b, me);
}
pub(crate) fn throw_2(b: &mut Cps2Bus, me: Obj) {
    thrown_2(&mut super::effects_creators::restes::Pur, b, me);
}

/// 0x9ed78 : tir de type 8 qui balaie : son angle (`+0x1c`, ±8 par tir entre -0x40 et 0x40, sens dans
/// `+0x42`) est repris du créateur, avancé puis rendu au créateur.
pub(crate) fn sweeping_8(r: &mut impl Registres, b: &mut Cps2Bus, me: Obj) {
    let a1 = match r.alloue_ou_reste(b, &pools::ESHL) {
        Ok(o) => {
            o.set_u8(b, f::ACTIVE, 1);
            o.set_u8(b, f::KIND, 8);
            o.set_u8(b, f::MODE, 2);
            o.set_u16(b, f::OWNER, me.word());
            let swing = me.u16(b, f::SWING);
            o.set_u16(b, f::SWING, swing);
            let back = me.u8(b, f::CREATOR);
            o.set_u8(b, f::CREATOR, back);
            if back == 0 {
                let s = swing.wrapping_add(8);
                o.set_u16(b, f::SWING, s);
                if s as i16 > 0x40 {
                    o.set_u8(b, f::CREATOR, back.wrapping_add(1));
                }
            } else {
                let s = swing.wrapping_sub(8);
                o.set_u16(b, f::SWING, s);
                if (s as i16) < -0x40 {
                    o.set_u8(b, f::CREATOR, 0);
                }
            }
            let s = o.u16(b, f::SWING);
            me.set_u16(b, f::SWING, s);
            let d = o.u8(b, f::CREATOR);
            me.set_u8(b, f::CREATOR, d);
            o.set_u32(b, f::VX, 0x0280_0000);
            o.set_u32(b, f::VZ, 0);
            if me.u8(b, f::FLIP) == 0 {
                let v = o.u16(b, f::VX).wrapping_neg();
                o.set_u16(b, f::VX, v);
            }
            o
        }
        Err(reste) => reste,
    };
    copy_flip(r, b, me, a1);
}

/// 0x9eba6 : tir de type 6, variante `variant`, orientation du créateur.
pub(crate) fn shot_6(r: &mut impl Registres, b: &mut Cps2Bus, me: Obj, variant: u8) -> Option<Obj> {
    let o = spawn(r, b, 6)?;
    o.set_u8(b, f::VARIANT, variant);
    let flip = me.u8(b, f::FLIP);
    r.d_octet(0, flip);
    o.set_u8(b, f::FLIP, flip);
    o.set_u16(b, f::OWNER, me.word());
    r.drapeaux(me.word() as u32, Size::Word);
    Some(o)
}

/// Objets de type `kind` liés au créateur (`link` : `+0x5c`, sinon `+0x50`), variantes `count - 1` à 0 (D1) ; s'arrête
/// si le pool est plein (faux, retenue levée).
fn ring(r: &mut impl Registres, b: &mut Cps2Bus, me: Obj, kind: u8, count: u16, link: u32) -> bool {
    let mut n = count - 1;
    r.d(1, n as u32);
    loop {
        let Some(o) = spawn(r, b, kind) else { return false };
        o.set_u8(b, f::VARIANT, n as u8);
        o.set_u16(b, link, me.word());
        r.drapeaux(me.word() as u32, Size::Word);
        n = n.wrapping_sub(1);
        r.d_mot(1, n);
        if n == 0xffff {
            return true;
        }
    }
}

/// 0x9e194 : quatre objets de type 3 (variantes 3 à 0, D1), liés par `+0x50` ; faux si le pool est plein.
pub(crate) fn four_3(r: &mut impl Registres, b: &mut Cps2Bus, me: Obj) -> bool {
    ring(r, b, me, 3, 4, f::OWNER)
}

/// 0x9e2a0 : objet de type 4 au-dessus du créateur (Z = 0x28), variante = `+0x2f` du créateur, lié par un
/// long en `+0x46` ; le créateur reçoit 0x8000 en `+0x12`.
pub(crate) fn above_4(r: &mut impl Registres, b: &mut Cps2Bus, me: Obj) -> Option<Obj> {
    let o = spawn(r, b, 4)?;
    wr32(b, o.0 + f::PARENT, me.0);
    let v = me.u8(b, f::INDEX);
    o.set_u8(b, f::VARIANT, v);
    let (x, y) = (me.u32(b, f::X), me.u32(b, f::Y));
    o.set_u32(b, f::X, x);
    o.set_u32(b, f::Y, y);
    o.set_u16(b, f::Z, 0x28);
    wr16(b, me.0 + 0x12, 0x8000);
    let flip = me.u8(b, f::FLIP);
    o.set_u8(b, f::FLIP, flip);
    r.drapeaux(flip as u32, Size::Byte);
    Some(o)
}

/// 0x9eede : deux objets de type 9 (variantes 1 et 2), le second lié au premier ; D0 = 1.
pub(crate) fn pair_9(r: &mut impl Registres, b: &mut Cps2Bus, me: Obj) -> Option<Obj> {
    let first = spawn(r, b, 9)?;
    first.set_u8(b, f::VARIANT, 1);
    first.set_u16(b, f::PARENT, me.word());
    r.a(2, first.word() as i16 as i32 as u32);
    let second = spawn(r, b, 9)?;
    second.set_u8(b, f::VARIANT, 2);
    second.set_u16(b, f::PARENT, first.word());
    r.d(0, 1);
    r.drapeaux(1, Size::Long);
    Some(second)
}

/// 0x9dbd6 : objet de type 0, variante = `+0x2f` du créateur, lié par `+0x42` et `+0x50`.
pub(crate) fn shot_0(r: &mut impl Registres, b: &mut Cps2Bus, me: Obj) -> Option<Obj> {
    let o = spawn(r, b, 0)?;
    o.set_u16(b, f::CREATOR, me.word());
    o.set_u16(b, f::OWNER, me.word());
    let v = me.u8(b, f::INDEX);
    o.set_u8(b, f::VARIANT, v);
    let flip = me.u8(b, f::FLIP);
    o.set_u8(b, f::FLIP, flip);
    r.drapeaux(flip as u32, Size::Byte);
    Some(o)
}

/// 0x9e426 : douze objets de type 5 (`+0x2f` = 0 à 11, liés par `+0x46`) si `count` est inférieur au mot
/// `A5+0x7e64` ; vrai (D0 = 1) s'ils sont créés.
pub(crate) fn twelve_5(r: &mut impl Registres, b: &mut Cps2Bus, me: Obj, count: u16) -> bool {
    r.d(1, 12);
    let limit = rd16(b, LIMITE_DOUZE);
    if (count as i16) >= limit as i16 {
        r.d(0, 0);
        r.drapeaux(0, Size::Long);
        return false;
    }
    r.d_octet(1, 11);
    r.d(2, 0);
    for i in 0..12u16 {
        // Glitch d'origine : l'échec de l'allocation n'est pas testé (voir `Pool::alloc_unchecked`).
        let o = r.alloue_sans_test(b, &pools::ESHL);
        o.set_u8(b, f::ACTIVE, 1);
        o.set_u8(b, f::KIND, 5);
        o.set_u8(b, f::INDEX, i as u8);
        o.set_u16(b, f::PARENT, me.word());
        r.d_mot(2, i + 1);
        r.d_mot(1, 10u16.wrapping_sub(i));
    }
    r.d(0, 1);
    r.drapeaux(1, Size::Long);
    true
}

/// Rangée d'éclats (types 0x19 et 0x1a) : `count` + 1 objets (D5.w) numérotés D3 = 0, 1..., `+0x58` = 1, 9, 17... (D4),
/// avec le `+0x5c` du créateur ; `directions` : octet `+0x2d` de chacun (type 0x1a), lu dans une table qui avance (A2).
fn row(r: &mut impl Registres, b: &mut Cps2Bus, me: Obj, kind: u8, mut count: u16, mut directions: Option<u32>) -> Option<Obj> {
    let (mut n3, mut n4) = (0u8, 1u16);
    r.d(3, 0);
    r.d(4, 1);
    loop {
        let o = spawn(r, b, kind)?;
        o.set_u8(b, f::VARIANT, n3);
        if let Some(at) = directions.as_mut() {
            let d = crate::mem::rd8(b, *at);
            *at += 1;
            r.a(2, *at);
            o.set_u8(b, f::DIRECTION, d);
        }
        o.set_u16(b, 0x58, n4);
        n3 = n3.wrapping_add(1);
        n4 = n4.wrapping_add(8);
        r.d_octet(3, n3);
        r.d_mot(4, n4);
        let link = me.u16(b, f::LINK);
        o.set_u16(b, f::LINK, link);
        r.drapeaux(link as u32, Size::Word);
        count = count.wrapping_sub(1);
        r.d_mot(5, count);
        if count == 0xffff {
            return Some(o);
        }
    }
}

/// Nombre d'éclats moins un, lu dans une table (programme) indexée par le rang / 2.
fn count_by_rank(r: &mut impl Registres, b: &mut Cps2Bus, table: u32) -> u8 {
    let i = rd16(b, RANG) >> 1;
    r.d_mot(0, i);
    prog8(b, table.wrapping_add(i as i16 as i32 as u32))
}

/// 0xa1670 : rangée d'éclats de type 0x19 (nombre tiré de la table 0xa16ac).
pub(crate) fn row_19(r: &mut impl Registres, b: &mut Cps2Bus, me: Obj) -> Option<Obj> {
    let n = count_by_rank(r, b, 0xa16ac);
    r.d(5, n as u32);
    row(r, b, me, 0x19, n as u16, None)
}

/// 0xa17d0 : rangée d'éclats de type 0x1a (`sorte`, mémorisée en `+0x56`) ; leurs directions viennent d'une table
/// (en clair) choisie par la sorte, ou par la position de la cible `+0x6c`, puis par le hasard.
/// Glitch d'origine : `move.b (table),D5.b` sans `moveq #0,D5` (contrairement à 0xa1670) : l'octet haut du mot D5
/// laissé par l'appelant (`d5`, la caisse `boxes::effet_special`) s'ajoute au compte de la boucle `dbf D5.w`, qui
/// peut alors remplir le pool ESHL.
pub(crate) fn row_1a(r: &mut impl Registres, b: &mut Cps2Bus, me: Obj, sorte: u16, d5: u16) -> Option<Obj> {
    me.set_u16(b, 0x56, sorte);
    let n = count_by_rank(r, b, 0xa184c);
    r.d_octet(5, n);
    let table = match sorte as u8 {
        1 => 0xa185c,
        2 => 0xa187c,
        _ => {
            let cible = me.u16(b, 0x6c) as i16 as i32 as u32;
            r.a(1, cible);
            let tx = rd16(b, cible + f::X);
            r.d_mot(1, tx);
            if tx <= me.u16(b, f::X) { 0xa189c } else { 0xa18bc }
        }
    };
    let alea = (crate::rng::random(b) & 3) << 3;
    r.d_mot(0, alea);
    row(r, b, me, 0x1a, d5 & 0xff00 | n as u16, Some(table + alea as u32))
}

/// 0xa1c6a : quatre objets de type 0x1c (variantes 0 à 3, `+0x5c` du créateur), puis le son 0xebef8.
pub(crate) fn four_1c(r: &mut impl Registres, b: &mut Cps2Bus, me: Obj) {
    r.d(4, 0);
    r.d(5, 3);
    for k in 0..4u16 {
        let Some(o) = spawn(r, b, 0x1c) else { break };
        o.set_u8(b, f::VARIANT, k as u8);
        r.d_octet(4, k as u8 + 1);
        let link = me.u16(b, f::LINK);
        o.set_u16(b, f::LINK, link);
        r.d_mot(5, 2u16.wrapping_sub(k));
    }
    crate::sound::jouer(b, 0xebef8, me);
}

/// 0xa31c2 : trois objets de type 0x28.
pub(crate) fn ring_28(r: &mut impl Registres, b: &mut Cps2Bus, me: Obj) {
    ring(r, b, me, 0x28, 3, f::LINK);
}

/// 0xa2f96 : seize objets de type 0x27.
pub(crate) fn ring_27(r: &mut impl Registres, b: &mut Cps2Bus, me: Obj) {
    ring(r, b, me, 0x27, 16, f::LINK);
}

/// 0xa6824 : deux objets de type 0x3f liés au créateur, tournés à droite puis à gauche (variante 0).
pub(crate) fn pair_3f(r: &mut impl Registres, b: &mut Cps2Bus, me: Obj) {
    for flip in [0, 0x20] {
        let Some(o) = spawn(r, b, 0x3f) else { break };
        o.set_u16(b, f::LINK, me.word());
        o.set_u8(b, f::FLIP, flip);
        o.set_u8(b, f::VARIANT, 0);
        r.drapeaux(0, Size::Byte);
    }
}

/// 0xa3354 : un objet de type 0x29 lié au créateur.
pub(crate) fn single_29(r: &mut impl Registres, b: &mut Cps2Bus, me: Obj) -> Option<Obj> {
    let o = spawn(r, b, 0x29)?;
    o.set_u16(b, f::LINK, me.word());
    r.drapeaux(me.word() as u32, Size::Word);
    Some(o)
}

/// 0x9ddee : un tir de type 1 (`+0x2f` = 1) de variante `variant`, lié au créateur par `+0x50`.
pub(crate) fn single_1(r: &mut impl Registres, b: &mut Cps2Bus, me: Obj, variant: u8) -> Option<Obj> {
    let o = spawn(r, b, 1)?;
    o.set_u8(b, f::INDEX, 1);
    o.set_u8(b, f::VARIANT, variant);
    o.set_u16(b, f::OWNER, me.word());
    r.drapeaux(me.word() as u32, Size::Word);
    Some(o)
}

/// 0xa460a, 0xa464e, 0xa4676 et 0xa462c : un objet de type 0x34 (`+0x2f` = 0) de variante 2, 1 (avec l'orientation
/// du créateur), 0 et 3, lié par `+0x46`.
pub(crate) fn linked_34(r: &mut impl Registres, b: &mut Cps2Bus, me: Obj, variant: u8) -> Option<Obj> {
    let o = spawn(r, b, 0x34)?;
    o.set_u8(b, f::VARIANT, variant);
    o.set_u8(b, f::INDEX, 0);
    if variant == 1 {
        let flip = me.u8(b, f::FLIP);
        o.set_u8(b, f::FLIP, flip);
    }
    o.set_u16(b, f::PARENT, me.word());
    r.drapeaux(me.word() as u32, Size::Word);
    Some(o)
}

/// 0xa5f38 : un objet de type 0x3c, variante 2, `+0x2f` et liens du créateur (`+0x2f` recopié, `+0x5c` = lui).
pub(crate) fn single_3c(r: &mut impl Registres, b: &mut Cps2Bus, me: Obj) -> Option<Obj> {
    let o = spawn(r, b, 0x3c)?;
    o.set_u8(b, f::VARIANT, 2);
    let v = me.u8(b, f::INDEX);
    o.set_u8(b, f::INDEX, v);
    o.set_u16(b, f::LINK, me.word());
    r.drapeaux(me.word() as u32, Size::Word);
    Some(o)
}

/// 0xa2b4a : un objet de type 0x25 lié au créateur (`+0x5c`).
pub(crate) fn single_25(r: &mut impl Registres, b: &mut Cps2Bus, me: Obj) -> Option<Obj> {
    let o = spawn(r, b, 0x25)?;
    o.set_u16(b, f::LINK, me.word());
    r.drapeaux(me.word() as u32, Size::Word);
    Some(o)
}

/// 0xa1360 : un objet de type 0x18 qui hérite du `+0x5c` du créateur, puis le son 0xebe14.
pub(crate) fn single_18(r: &mut impl Registres, b: &mut Cps2Bus, me: Obj) {
    if let Some(o) = spawn(r, b, 0x18) {
        let link = me.u16(b, f::LINK);
        o.set_u16(b, f::LINK, link);
        r.drapeaux(link as u32, Size::Word);
    }
    crate::sound::jouer(b, 0xebe14, me);
}

/// 0xa1a40 : un objet de type 0x1b ; le créateur garde `sorte` en `+0x56`, l'objet reprend son `+0x5c` et sa
/// cible (`+0x6c` -> `+0x50`).
pub(crate) fn single_1b(r: &mut impl Registres, b: &mut Cps2Bus, me: Obj, sorte: u16) -> Option<Obj> {
    let o = spawn(r, b, 0x1b)?;
    me.set_u16(b, 0x56, sorte);
    let link = me.u16(b, f::LINK);
    o.set_u16(b, f::LINK, link);
    let target = me.u16(b, 0x6c);
    o.set_u16(b, f::OWNER, target);
    r.drapeaux(target as u32, Size::Word);
    Some(o)
}

/// 0xa44bc : trois objets de type 0x33 (variante `variant`) autour de la position de `cible`, aux décalages de la
/// table 0xa4516 (deux mots par objet) et numérotés 2 à 0 ; le dernier créé (D0 = 1, A1), `None` (D0 = 0) si le pool
/// est plein avant la fin.
pub(crate) fn chasers(r: &mut impl Registres, b: &mut Cps2Bus, me: Obj, variant: u8, cible: Obj) -> Option<Obj> {
    const OFFSETS: u32 = 0xa4516;
    let mut d4 = 1u16;
    r.d(4, 1);
    r.d(5, 2);
    let mut last = None;
    for n in (0..3u16).rev() {
        let Some(o) = spawn(r, b, 0x33) else {
            r.d(0, 0);
            r.drapeaux(0, Size::Long);
            return None;
        };
        last = Some(o);
        o.set_u8(b, f::VARIANT, variant);
        o.set_u8(b, f::INDEX, n as u8);
        let (x, y) = (cible.u16(b, f::X), cible.u16(b, f::Y));
        let i = n << 2;
        r.d_mot(3, i);
        let (dx, dy) = (prog16(b, OFFSETS + i as u32), prog16(b, OFFSETS + 2 + i as u32));
        r.d_mot(1, dx);
        r.d_mot(2, dy);
        o.set_u16(b, f::X, x.wrapping_add(dx));
        o.set_u16(b, f::Y, y.wrapping_add(dy));
        o.set_u16(b, f::CREATOR, d4);
        d4 = d4.wrapping_add(8);
        r.d_mot(4, d4);
        o.set_u16(b, f::OWNER, cible.word());
        o.set_u16(b, f::LINK, me.word());
        r.drapeaux(me.word() as u32, Size::Word);
        r.d_mot(5, n.wrapping_sub(1));
    }
    r.d(0, 1);
    r.drapeaux(1, Size::Long);
    last
}

/// Natives des créateurs (appels par adresse restants) : A0 = le créateur, paramètres dans les registres ; la
/// fonction sur le processeur, puis `rts`.
macro_rules! native {
    ($entry:literal, $name:literal, |$c:ident, $b:ident, $me:ident| $body:expr, $outputs:expr) => {
        Native {
            entry: $entry,
            name: $name,
            run: crate::Corps::Processeur(|$c, $b| {
                let $me = Obj($c.a[0]);
                $body;
                rts($c, $b)
            }),
            touches_sound: false,
            outputs: $outputs,
        }
    };
}

const CREATED: u32 = regs::a(1) | regs::CCR;

/// D0.w, D1.w : paramètres de l'original.
fn d0w(c: &Cpu) -> u16 {
    c.d[0] as u16
}
fn d1w(c: &Cpu) -> u16 {
    c.d[1] as u16
}

pub const NATIVES: &[Native] = &[
    native!(0xa7b0a, "eshl_eclats_44", |c, b, me| boss11_shards(c, b, me), CREATED | regs::d(0) | regs::d(1) | regs::d(2)),
    native!(0xa7570, "eshl_fragments_42", |c, b, me| { let v = d0w(c); fragments_42(c, b, me, v) }, CREATED | regs::d(1)),
    native!(0xa7da2, "eshl_fragments_45", |c, b, me| fragments_45(c, b, me), CREATED | regs::d(0) | regs::d(1) | regs::d(2)),
    native!(0xa4e84, "eshl_serie_36", |c, b, me| { let i = c.d[4] as u8; series_36(c, b, me, i) }, CREATED | regs::d(5)),
    native!(0xa5c9a, "eshl_serie_3b", |c, b, me| series_3b(c, b, me), CREATED | regs::d(5)),
    native!(0xa5c7c, "eshl_lie_3b", |c, b, me| single_3b(c, b, me), CREATED),
    native!(0xa6e6a, "eshl_lie_40", |c, b, me| single_40(c, b, me), CREATED),
    native!(0x9dda8, "eshl_lance_1", |c, b, me| thrown_1(c, b, me), CREATED | regs::d(0)),
    native!(0x9e062, "eshl_lance_2", |c, b, me| thrown_2(c, b, me), CREATED | regs::d(0)),
    native!(0x9ed78, "eshl_balaye_8", |c, b, me| sweeping_8(c, b, me), CREATED | regs::d(0)),
    native!(0x9eba6, "eshl_tir_6", |c, b, me| { let v = c.d[0] as u8; shot_6(c, b, me, v) }, CREATED | regs::d(0)),
    native!(0x9e194, "eshl_quatre_3", |c, b, me| four_3(c, b, me), CREATED | regs::d(1)),
    native!(0x9e2a0, "eshl_dessus_4", |c, b, me| above_4(c, b, me), CREATED),
    native!(0x9eede, "eshl_paire_9", |c, b, me| pair_9(c, b, me), CREATED | regs::d(0) | regs::a(2)),
    native!(0x9dbd6, "eshl_tir_0", |c, b, me| shot_0(c, b, me), CREATED),
    native!(0x9e426, "eshl_douze_5", |c, b, me| { let n = d0w(c); twelve_5(c, b, me, n) }, CREATED | regs::d(0) | regs::d(1) | regs::d(2)),
    native!(0xa1670, "eshl_rangee_19", |c, b, me| row_19(c, b, me), CREATED | regs::d(0) | regs::d(3) | regs::d(4) | regs::d(5)),
    native!(0xa17d0, "eshl_rangee_1a", |c, b, me| { let (s, d5) = (d1w(c), c.d[5] as u16); row_1a(c, b, me, s, d5) }, CREATED | regs::d(0) | regs::d(1) | regs::d(3) | regs::d(4) | regs::d(5) | regs::a(2)),
    native!(0xa1c6a, "eshl_quatre_1c", |c, b, me| four_1c(c, b, me), 0),
    native!(0xa31c2, "eshl_anneau_28", |c, b, me| ring_28(c, b, me), CREATED | regs::d(1)),
    native!(0xa2f96, "eshl_anneau_27", |c, b, me| ring_27(c, b, me), CREATED | regs::d(1)),
    native!(0xa6824, "eshl_paire_3f", |c, b, me| pair_3f(c, b, me), CREATED),
    native!(0xa3354, "eshl_lie_29", |c, b, me| single_29(c, b, me), CREATED),
    native!(0x9ddee, "eshl_tir_1", |c, b, me| { let v = c.d[1] as u8; single_1(c, b, me, v) }, CREATED),
    native!(0xa460a, "eshl_lie_34_2", |c, b, me| linked_34(c, b, me, 2), CREATED),
    native!(0xa464e, "eshl_lie_34_1", |c, b, me| linked_34(c, b, me, 1), CREATED),
    native!(0xa4676, "eshl_lie_34_0", |c, b, me| linked_34(c, b, me, 0), CREATED),
    native!(0xa462c, "eshl_lie_34_3", |c, b, me| linked_34(c, b, me, 3), CREATED),
    native!(0xa2b4a, "eshl_lie_25", |c, b, me| single_25(c, b, me), CREATED),
    native!(0xa5f38, "eshl_lie_3c", |c, b, me| single_3c(c, b, me), CREATED),
    native!(0xa1360, "eshl_lie_18", |c, b, me| single_18(c, b, me), CREATED),
    native!(0xa1a40, "eshl_lie_1b", |c, b, me| { let s = d1w(c); single_1b(c, b, me, s) }, CREATED),
    native!(0xa44bc, "eshl_poursuivants_33", |c, b, me| { let (v, t) = (c.d[0] as u8, Obj(c.a[2])); chasers(c, b, me, v, t) }, CREATED | regs::d(0) | regs::d(1) | regs::d(2) | regs::d(3) | regs::d(4) | regs::d(5)),
];
