//! Créateurs de tirs ennemis (ESHL) appelés par les gestionnaires d'ennemis et de boss : chacun alloue un
//! ou plusieurs objets ESHL (`0x22ea`), pose le type, la variante et le lien vers le créateur (A0).
//!
//! Contrat : A0 = le créateur ; au retour A1 = le dernier objet alloué, retenue levée si le pool est plein
//! (l'appel s'arrête là) ; sinon drapeaux de la dernière écriture et compteurs de boucle tels que l'original
//! les laisse (`dbf` : mot bas à -1). Ce contrat est relu : avec `DDSOM_POISON` sur les registres que ces créateurs
//! changent, la validation diverge dans une trentaine de scénarios (gestionnaires d'ennemis et de boss qui testent la
//! retenue ou A1, sauvegarde des registres par les appels de son qui suivent).
//!
//! Chaque créateur est une fonction publique `(c, b)` (A0 = le créateur) qu'un appelant Rust appelle directement
//! ([`appeler`]) ; la native, pour les appels par adresse restants, n'y ajoute que le `rts`.

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::Cpu;

use crate::mem::{prog16, prog8, rd16, rts, wr16, wr32};
use crate::objects::Obj;
use crate::{regs, Native};
use crate::objects::pools;

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

/// Alloue un ESHL de type `kind` (actif) ; `None` (retenue levée) si le pool est plein.
fn spawn(c: &mut Cpu, b: &mut Cps2Bus, kind: u8) -> Option<Obj> {
    let o = pools::ESHL.alloc_68k(c, b)?;
    o.set_u8(b, f::ACTIVE, 1);
    o.set_u8(b, f::KIND, kind);
    Some(o)
}

/// Drapeaux d'un `move` (N, Z ; V et C à zéro).
fn moved(c: &mut Cpu, v: u32, size: Size) {
    c.flags_logic(v, size);
}

fn set_low_byte(r: &mut u32, v: u8) {
    *r = *r & !0xff | v as u32;
}

fn set_low_word(r: &mut u32, v: u16) {
    *r = *r & !0xffff | v as u32;
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
/// `flip` imposée. D1.w sert de compteur.
fn fragments(c: &mut Cpu, b: &mut Cps2Bus, kind: u8, variant: u8, count: u16, skip: Option<u8>, flip: Option<u8>, creator_kind: bool) -> bool {
    let me = Obj(c.a[0]);
    set_low_word(&mut c.d[1], count.wrapping_sub(1));
    loop {
        let i = c.d[1] as u8;
        if skip != Some(i) {
            let Some(o) = spawn(c, b, kind) else { return false };
            o.set_u8(b, f::VARIANT, variant);
            o.set_u8(b, f::INDEX, i);
            o.set_u16(b, f::PARENT, me.0 as u16);
            copy_position(b, me, o);
            if creator_kind {
                let k = me.u8(b, f::KIND);
                o.set_u8(b, f::CREATOR, k);
            }
            let v = flip.unwrap_or_else(|| me.u8(b, f::FLIP));
            o.set_u8(b, f::FLIP, v);
            moved(c, v as u32, Size::Byte);
        }
        let n = (c.d[1] as u16).wrapping_sub(1);
        set_low_word(&mut c.d[1], n);
        if n == 0xffff {
            return true;
        }
    }
}

/// 0xa7b0a : éclats du boss 11 (type 0x44) ; un tirage choisit leur nombre et celui qui manque (table
/// 0xa7b64 : couples nombre, absent).
pub fn boss11_shards(c: &mut Cpu, b: &mut Cps2Bus) {
    crate::rng::random_68k(c, b);
    let i = c.d[0] as u16 & 0xe;
    set_low_word(&mut c.d[0], i);
    let count = prog8(b, 0xa7b64 + i as u32);
    let missing = prog8(b, 0xa7b65 + i as u32);
    c.d[1] = count as u32;
    set_low_byte(&mut c.d[2], missing);
    fragments(c, b, 0x44, 0, count as u16, Some(missing), None, false);
}

/// 0xa7570 : D0 = variante ; fragments de type 0x42, leur nombre lu dans la table 0xa75b6.
pub fn fragments_42(c: &mut Cpu, b: &mut Cps2Bus) {
    let variant = c.d[0] as u16;
    let count = prog8(b, 0xa75b6u32.wrapping_add(variant as i16 as i32 as u32));
    c.d[1] = count as u32;
    fragments(c, b, 0x42, variant as u8, count as u16, None, None, false);
}

/// 0xa7da2 : deux rangées de fragments de type 0x45 (nombre lu en 0xa7d92), tournés à droite (avec le
/// type du créateur en `+0x42`) puis à gauche.
pub fn fragments_45(c: &mut Cpu, b: &mut Cps2Bus) {
    c.d[0] = 0;
    let count = prog8(b, 0xa7d92) as u16;
    c.d[2] = count as u32;
    c.d[1] = count as u32;
    if fragments(c, b, 0x45, 0, count, None, Some(0), true) {
        set_low_word(&mut c.d[1], count);
        fragments(c, b, 0x45, 0, count, None, Some(0x20), false);
    }
}

/// Série d'objets liés par `+0x5c` : variantes `last` à 0 (D5), `+0x2f` = `index`.
fn series(c: &mut Cpu, b: &mut Cps2Bus, kind: u8, last: u8, index: u8) {
    let me = c.a[0];
    c.d[5] = last as u32;
    loop {
        let Some(o) = spawn(c, b, kind) else { break };
        o.set_u8(b, f::VARIANT, c.d[5] as u8);
        o.set_u8(b, f::INDEX, index);
        o.set_u16(b, f::LINK, me as u16);
        moved(c, me & 0xffff, Size::Word);
        let n = (c.d[5] as u16).wrapping_sub(1);
        set_low_word(&mut c.d[5], n);
        if n == 0xffff {
            break;
        }
    }
}

/// 0xa4e84 : 17 objets de type 0x36 (variantes 16 à 0), `+0x2f` = D4.
pub fn series_36(c: &mut Cpu, b: &mut Cps2Bus) {
    let index = c.d[4] as u8;
    series(c, b, 0x36, 0x10, index);
}

/// 0xa5c9a : 16 objets de type 0x3b (variantes 15 à 0), `+0x2f` = 1.
pub fn series_3b(c: &mut Cpu, b: &mut Cps2Bus) {
    series(c, b, 0x3b, 0xf, 1);
}

/// Un objet lié au créateur par `+0x5c`, de type `kind`, variante `variant` (None : non posée), `+0x2f` = `index`.
fn linked(c: &mut Cpu, b: &mut Cps2Bus, kind: u8, variant: Option<u8>, index: u8) {
    let me = c.a[0];
    if let Some(o) = spawn(c, b, kind) {
        if let Some(v) = variant {
            o.set_u8(b, f::VARIANT, v);
        }
        o.set_u8(b, f::INDEX, index);
        o.set_u16(b, f::LINK, me as u16);
        moved(c, me & 0xffff, Size::Word);
    }
}

/// 0xa5c7c : un objet de type 0x3b, `+0x2f` = 0.
pub fn single_3b(c: &mut Cpu, b: &mut Cps2Bus) {
    linked(c, b, 0x3b, None, 0);
}

/// 0xa6e6a : un objet de type 0x40, variante 0, `+0x2f` = 5.
pub fn single_40(c: &mut Cpu, b: &mut Cps2Bus) {
    linked(c, b, 0x40, Some(0), 5);
}

/// Tir lancé devant le créateur (types 1 et 2) : `+0x2a` = 2, angle 8 (0x18 et vitesse retournée s'il
/// regarde à droite), orientation du créateur. L'orientation est recopiée dans A1 même quand le pool est
/// plein pour le type 1 (`always_flip`), comme l'original.
fn thrown(c: &mut Cpu, b: &mut Cps2Bus, kind: u8, vx: Option<u32>, always_flip: bool) {
    let me = Obj(c.a[0]);
    match spawn(c, b, kind) {
        Some(o) => {
            if kind == 1 {
                o.set_u8(b, f::INDEX, 0);
            }
            o.set_u8(b, f::MODE, 2);
            o.set_u16(b, f::OWNER, me.0 as u16);
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
        }
        None if !always_flip => return,
        None => {}
    }
    let flip = me.u8(b, f::FLIP);
    set_low_byte(&mut c.d[0], flip);
    wr8_flags(c, b, c.a[1] + f::FLIP, flip);
}

/// `move.b D0b,(off,A1)` : écrit et pose les drapeaux.
fn wr8_flags(c: &mut Cpu, b: &mut Cps2Bus, at: u32, v: u8) {
    crate::mem::wr8(b, at, v);
    moved(c, v as u32, Size::Byte);
}

/// 0x9dda8 : tir de type 1 (vitesse laissée à zéro par l'allocation).
pub fn thrown_1(c: &mut Cpu, b: &mut Cps2Bus) {
    thrown(c, b, 1, None, true);
}

/// 0x9e062 : tir de type 2, vitesse 5 pixels par image.
pub fn thrown_2(c: &mut Cpu, b: &mut Cps2Bus) {
    thrown(c, b, 2, Some(0x0500_0000), false);
}

/// 0x9ed78 : tir de type 8 qui balaie : son angle (`+0x1c`, ±8 par tir entre -0x40 et 0x40, sens dans
/// `+0x42`) est repris du créateur, avancé puis rendu au créateur.
pub fn sweeping_8(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = Obj(c.a[0]);
    if let Some(o) = spawn(c, b, 8) {
        o.set_u8(b, f::MODE, 2);
        o.set_u16(b, f::OWNER, me.0 as u16);
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
    }
    let flip = me.u8(b, f::FLIP);
    set_low_byte(&mut c.d[0], flip);
    wr8_flags(c, b, c.a[1] + f::FLIP, flip);
}

/// 0x9eba6 : tir de type 6, variante D0, orientation du créateur.
pub fn shot_6(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = Obj(c.a[0]);
    if let Some(o) = spawn(c, b, 6) {
        o.set_u8(b, f::VARIANT, c.d[0] as u8);
        let flip = me.u8(b, f::FLIP);
        set_low_byte(&mut c.d[0], flip);
        o.set_u8(b, f::FLIP, flip);
        o.set_u16(b, f::OWNER, me.0 as u16);
        moved(c, me.0 & 0xffff, Size::Word);
    }
}

/// 0x9e194 : quatre objets de type 3 (variantes 3 à 0, D1).
pub fn four_3(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = c.a[0];
    c.d[1] = 3;
    loop {
        let Some(o) = spawn(c, b, 3) else { break };
        o.set_u8(b, f::VARIANT, c.d[1] as u8);
        o.set_u16(b, f::OWNER, me as u16);
        moved(c, me & 0xffff, Size::Word);
        let n = (c.d[1] as u16).wrapping_sub(1);
        set_low_word(&mut c.d[1], n);
        if n == 0xffff {
            break;
        }
    }
}

/// 0x9e2a0 : objet de type 4 au-dessus du créateur (Z = 0x28), variante = `+0x2f` du créateur, lié par un
/// long en `+0x46` ; le créateur reçoit 0x8000 en `+0x12`.
pub fn above_4(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = Obj(c.a[0]);
    if let Some(o) = spawn(c, b, 4) {
        wr32(b, o.0 + f::PARENT, c.a[0]);
        let v = me.u8(b, f::INDEX);
        o.set_u8(b, f::VARIANT, v);
        let (x, y) = (me.u32(b, f::X), me.u32(b, f::Y));
        o.set_u32(b, f::X, x);
        o.set_u32(b, f::Y, y);
        o.set_u16(b, f::Z, 0x28);
        wr16(b, me.0 + 0x12, 0x8000);
        let flip = me.u8(b, f::FLIP);
        o.set_u8(b, f::FLIP, flip);
        moved(c, flip as u32, Size::Byte);
    }
}

/// 0x9eede : deux objets de type 9 (variantes 1 et 2), le second lié au premier ; D0 = 1.
pub fn pair_9(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = c.a[0];
    let Some(first) = spawn(c, b, 9) else { return };
    first.set_u8(b, f::VARIANT, 1);
    first.set_u16(b, f::PARENT, me as u16);
    c.a[2] = first.0 as u16 as i16 as i32 as u32;
    let Some(second) = spawn(c, b, 9) else { return };
    second.set_u8(b, f::VARIANT, 2);
    second.set_u16(b, f::PARENT, c.a[2] as u16);
    c.d[0] = 1;
    moved(c, 1, Size::Long);
}

/// 0x9dbd6 : objet de type 0, variante = `+0x2f` du créateur, lié par `+0x42` et `+0x50`.
pub fn shot_0(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = Obj(c.a[0]);
    if let Some(o) = spawn(c, b, 0) {
        o.set_u16(b, f::CREATOR, me.0 as u16);
        o.set_u16(b, f::OWNER, me.0 as u16);
        let v = me.u8(b, f::INDEX);
        o.set_u8(b, f::VARIANT, v);
        let flip = me.u8(b, f::FLIP);
        o.set_u8(b, f::FLIP, flip);
        moved(c, flip as u32, Size::Byte);
    }
}

/// 0x9e426 : douze objets de type 5 (`+0x2f` = 0 à 11, liés par `+0x46`) si D0.w est inférieur au mot
/// `A5+0x7e64` ; D0 = 1 s'ils sont créés, 0 sinon (la place n'est pas vérifiée).
pub fn twelve_5(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = c.a[0];
    c.d[1] = 12;
    let limit = rd16(b, crate::mem::A5 + 0x7e64);
    if (c.d[0] as u16 as i16) >= limit as i16 {
        c.d[0] = 0;
        moved(c, 0, Size::Long);
        return;
    }
    set_low_byte(&mut c.d[1], 11);
    c.d[2] = 0;
    loop {
        // Glitch d'origine : l'échec de l'allocation n'est pas testé (voir `Pool::alloc_unchecked`).
        let o = pools::ESHL.alloc_unchecked_68k(c, b);
        o.set_u8(b, f::ACTIVE, 1);
        o.set_u8(b, f::KIND, 5);
        o.set_u8(b, f::INDEX, c.d[2] as u8);
        o.set_u16(b, f::PARENT, me as u16);
        let n = (c.d[2] as u16).wrapping_add(1);
        set_low_word(&mut c.d[2], n);
        let n = (c.d[1] as u16).wrapping_sub(1);
        set_low_word(&mut c.d[1], n);
        if n == 0xffff {
            break;
        }
    }
    c.d[0] = 1;
    moved(c, 1, Size::Long);
}

/// Rangée d'éclats (types 0x19 et 0x1a) : D5 + 1 objets numérotés D3 = 0, 1..., `+0x58` = 1, 9, 17... (D4),
/// avec le `+0x5c` du créateur ; `directions` : octet `+0x2d` de chacun (type 0x1a).
fn row(c: &mut Cpu, b: &mut Cps2Bus, kind: u8, mut directions: Option<u32>) {
    let me = Obj(c.a[0]);
    c.d[3] = 0;
    c.d[4] = 1;
    loop {
        let Some(o) = spawn(c, b, kind) else { return };
        o.set_u8(b, f::VARIANT, c.d[3] as u8);
        if let Some(at) = directions.as_mut() {
            let d = crate::mem::rd8(b, *at);
            *at += 1;
            c.a[2] = *at;
            o.set_u8(b, f::DIRECTION, d);
        }
        o.set_u16(b, 0x58, c.d[4] as u16);
        let (v3, v4) = ((c.d[3] as u8).wrapping_add(1), (c.d[4] as u16).wrapping_add(8));
        set_low_byte(&mut c.d[3], v3);
        set_low_word(&mut c.d[4], v4);
        let link = me.u16(b, f::LINK);
        o.set_u16(b, f::LINK, link);
        moved(c, link as u32, Size::Word);
        let n = (c.d[5] as u16).wrapping_sub(1);
        set_low_word(&mut c.d[5], n);
        if n == 0xffff {
            return;
        }
    }
}

/// Nombre d'éclats moins un, lu dans une table (programme) indexée par le compteur `A5+0x7f92` / 2.
fn count_by_tick(c: &mut Cpu, b: &mut Cps2Bus, table: u32) -> u8 {
    let i = rd16(b, crate::mem::A5 + 0x7f92) >> 1;
    set_low_word(&mut c.d[0], i);
    prog8(b, table.wrapping_add(i as i16 as i32 as u32))
}

/// 0xa1670 : rangée d'éclats de type 0x19 (nombre tiré de la table 0xa16ac).
pub fn row_19(c: &mut Cpu, b: &mut Cps2Bus) {
    c.d[5] = count_by_tick(c, b, 0xa16ac) as u32;
    row(c, b, 0x19, None);
}

/// 0xa17d0 : rangée d'éclats de type 0x1a (D1 = sorte, mémorisée en `+0x56`) ; leurs directions viennent
/// d'une table (en clair) choisie par la sorte, ou par la position de la cible `+0x6c`, puis par le hasard.
pub fn row_1a(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = Obj(c.a[0]);
    me.set_u16(b, 0x56, c.d[1] as u16);
    // Glitch d'origine : `move.b (table),D5.b` sans `moveq #0,D5` (contrairement à 0xa1670) : l'octet haut du
    // mot D5 laissé par l'appelant (la caisse, `boxes::effet_special`) s'ajoute au compte de la boucle `dbf D5.w`,
    // qui peut alors remplir le pool ESHL.
    let n = count_by_tick(c, b, 0xa184c);
    set_low_byte(&mut c.d[5], n);
    let table = match c.d[1] as u8 {
        1 => 0xa185c,
        2 => 0xa187c,
        _ => {
            c.a[1] = me.u16(b, 0x6c) as i16 as i32 as u32;
            let tx = rd16(b, c.a[1] + f::X);
            set_low_word(&mut c.d[1], tx);
            if tx <= me.u16(b, f::X) { 0xa189c } else { 0xa18bc }
        }
    };
    crate::rng::random_68k(c, b);
    let r = (c.d[0] as u16 & 3) << 3;
    set_low_word(&mut c.d[0], r);
    row(c, b, 0x1a, Some(table + r as u32));
}

/// 0xa1c6a : quatre objets de type 0x1c (variantes 0 à 3, `+0x5c` du créateur), puis le son 0xebef8.
pub fn four_1c(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = Obj(c.a[0]);
    c.d[4] = 0;
    c.d[5] = 3;
    loop {
        let Some(o) = spawn(c, b, 0x1c) else { break };
        o.set_u8(b, f::VARIANT, c.d[4] as u8);
        let v4 = (c.d[4] as u8).wrapping_add(1);
        set_low_byte(&mut c.d[4], v4);
        let link = me.u16(b, f::LINK);
        o.set_u16(b, f::LINK, link);
        let n = (c.d[5] as u16).wrapping_sub(1);
        set_low_word(&mut c.d[5], n);
        if n == 0xffff {
            break;
        }
    }
    crate::sound::jouer(b, 0xebef8, me);
}

/// Objets de type `kind` liés au créateur par `+0x5c`, variantes `count - 1` à 0 (D1) ; s'arrête si le pool
/// est plein.
fn linked_ring(c: &mut Cpu, b: &mut Cps2Bus, kind: u8, count: u16) {
    let me = c.a[0];
    c.d[1] = count as u32 - 1;
    loop {
        let Some(o) = spawn(c, b, kind) else { break };
        o.set_u8(b, f::VARIANT, c.d[1] as u8);
        o.set_u16(b, f::LINK, me as u16);
        moved(c, me & 0xffff, Size::Word);
        let n = (c.d[1] as u16).wrapping_sub(1);
        set_low_word(&mut c.d[1], n);
        if n == 0xffff {
            break;
        }
    }
}

/// 0xa31c2 : trois objets de type 0x28.
pub fn ring_28(c: &mut Cpu, b: &mut Cps2Bus) {
    linked_ring(c, b, 0x28, 3);
}

/// 0xa2f96 : seize objets de type 0x27.
pub fn ring_27(c: &mut Cpu, b: &mut Cps2Bus) {
    linked_ring(c, b, 0x27, 16);
}

/// 0xa6824 : deux objets de type 0x3f liés au créateur, tournés à droite puis à gauche (variante 0).
pub fn pair_3f(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = c.a[0];
    for flip in [0, 0x20] {
        let Some(o) = spawn(c, b, 0x3f) else { break };
        o.set_u16(b, f::LINK, me as u16);
        o.set_u8(b, f::FLIP, flip);
        o.set_u8(b, f::VARIANT, 0);
        moved(c, 0, Size::Byte);
    }
}

/// 0xa3354 : un objet de type 0x29 lié au créateur.
pub fn single_29(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = c.a[0];
    if let Some(o) = spawn(c, b, 0x29) {
        o.set_u16(b, f::LINK, me as u16);
        moved(c, me & 0xffff, Size::Word);
    }
}

/// 0x9ddee : un tir de type 1 (`+0x2f` = 1) de variante D1, lié au créateur par `+0x50`.
pub fn single_1(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = c.a[0];
    if let Some(o) = spawn(c, b, 1) {
        o.set_u8(b, f::INDEX, 1);
        o.set_u8(b, f::VARIANT, c.d[1] as u8);
        o.set_u16(b, f::OWNER, me as u16);
        moved(c, me & 0xffff, Size::Word);
    }
}

/// Objet de type 0x34 (`+0x2f` = 0) de variante `variant`, lié par `+0x46` ; `copy_flip` : orientation du créateur.
fn linked_34(c: &mut Cpu, b: &mut Cps2Bus, variant: u8, copy_flip: bool) {
    let me = Obj(c.a[0]);
    if let Some(o) = spawn(c, b, 0x34) {
        o.set_u8(b, f::VARIANT, variant);
        o.set_u8(b, f::INDEX, 0);
        if copy_flip {
            let flip = me.u8(b, f::FLIP);
            o.set_u8(b, f::FLIP, flip);
        }
        o.set_u16(b, f::PARENT, me.0 as u16);
        moved(c, me.0 & 0xffff, Size::Word);
    }
}

/// 0xa460a, 0xa464e, 0xa4676 et 0xa462c : le même objet 0x34, variantes 2, 1 (avec l'orientation), 0 et 3.
pub fn linked_34_3(c: &mut Cpu, b: &mut Cps2Bus) {
    linked_34(c, b, 3, false);
}
pub fn linked_34_2(c: &mut Cpu, b: &mut Cps2Bus) {
    linked_34(c, b, 2, false);
}
pub fn linked_34_1(c: &mut Cpu, b: &mut Cps2Bus) {
    linked_34(c, b, 1, true);
}
pub fn linked_34_0(c: &mut Cpu, b: &mut Cps2Bus) {
    linked_34(c, b, 0, false);
}

/// 0xa5f38 : un objet de type 0x3c, variante 2, `+0x2f` et liens du créateur (`+0x2f` recopié, `+0x5c` = lui).
pub fn single_3c(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = Obj(c.a[0]);
    if let Some(o) = spawn(c, b, 0x3c) {
        o.set_u8(b, f::VARIANT, 2);
        let v = me.u8(b, f::INDEX);
        o.set_u8(b, f::INDEX, v);
        o.set_u16(b, f::LINK, me.0 as u16);
        moved(c, me.0 & 0xffff, Size::Word);
    }
}

/// 0xa2b4a : un objet de type 0x25 lié au créateur (`+0x5c`).
pub fn single_25(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = Obj(c.a[0]);
    if let Some(o) = spawn(c, b, 0x25) {
        o.set_u16(b, f::LINK, me.0 as u16);
        moved(c, me.0 & 0xffff, Size::Word);
    }
}

/// 0xa1360 : un objet de type 0x18 qui hérite du `+0x5c` du créateur, puis le son 0xebe14.
pub fn single_18(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = Obj(c.a[0]);
    if let Some(o) = spawn(c, b, 0x18) {
        let link = me.u16(b, f::LINK);
        o.set_u16(b, f::LINK, link);
        moved(c, link as u32, Size::Word);
    }
    crate::sound::jouer(b, 0xebe14, me);
}

/// 0xa1a40 : un objet de type 0x1b ; le créateur garde D1 en `+0x56`, l'objet reprend son `+0x5c` et sa
/// cible (`+0x6c` -> `+0x50`).
pub fn single_1b(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = Obj(c.a[0]);
    if let Some(o) = spawn(c, b, 0x1b) {
        me.set_u16(b, 0x56, c.d[1] as u16);
        let link = me.u16(b, f::LINK);
        o.set_u16(b, f::LINK, link);
        let target = me.u16(b, 0x6c);
        o.set_u16(b, f::OWNER, target);
        moved(c, target as u32, Size::Word);
    }
}

/// 0xa44bc : trois objets de type 0x33 (variante D0) autour de la position de A2, aux décalages de la table
/// 0xa4516 (deux mots par objet) et numérotés 2 à 0 ; D0 = 1 s'ils sont tous créés, 0 sinon.
pub fn chasers(c: &mut Cpu, b: &mut Cps2Bus) {
    const OFFSETS: u32 = 0xa4516;
    let (me, target) = (c.a[0], Obj(c.a[2]));
    c.d[4] = 1;
    c.d[5] = 2;
    loop {
        let variant = c.d[0] as u8;
        let Some(o) = spawn(c, b, 0x33) else {
            c.d[0] = 0;
            moved(c, 0, Size::Long);
            return;
        };
        o.set_u8(b, f::VARIANT, variant);
        o.set_u8(b, f::INDEX, c.d[5] as u8);
        let (x, y) = (target.u16(b, f::X), target.u16(b, f::Y));
        let i = (c.d[5] as u16) << 2;
        set_low_word(&mut c.d[3], i);
        let (dx, dy) = (prog16(b, OFFSETS + i as u32), prog16(b, OFFSETS + 2 + i as u32));
        set_low_word(&mut c.d[1], dx);
        set_low_word(&mut c.d[2], dy);
        o.set_u16(b, f::X, x.wrapping_add(dx));
        o.set_u16(b, f::Y, y.wrapping_add(dy));
        o.set_u16(b, f::CREATOR, c.d[4] as u16);
        let d4 = (c.d[4] as u16).wrapping_add(8);
        set_low_word(&mut c.d[4], d4);
        o.set_u16(b, f::OWNER, c.a[2] as u16);
        o.set_u16(b, f::LINK, me as u16);
        moved(c, me & 0xffff, Size::Word);
        let n = (c.d[5] as u16).wrapping_sub(1);
        set_low_word(&mut c.d[5], n);
        if n == 0xffff {
            break;
        }
    }
    c.d[0] = 1;
    moved(c, 1, Size::Long);
}

/// Appel direct d'un créateur par un gestionnaire : A0 = l'objet courant, A5 = la RAM de travail (comme `Cx::call`).
pub fn appeler(x: &mut super::cx::Cx, createur: fn(&mut Cpu, &mut Cps2Bus)) {
    x.c.a[0] = x.o.0;
    x.c.a[5] = crate::mem::A5;
    createur(x.c, x.b);
}

/// Native d'un créateur appelé par adresse : la fonction, puis `rts`.
macro_rules! native {
    ($entry:literal, $name:literal, $f:ident, $outputs:expr) => {
        Native { entry: $entry, name: $name, run: crate::Corps::Processeur(|c, b| { $f(c, b); rts(c, b) }), touches_sound: false, outputs: $outputs }
    };
}

const CREATED: u32 = regs::a(1) | regs::CCR;

pub const NATIVES: &[Native] = &[
    native!(0xa7b0a, "eshl_eclats_44", boss11_shards, CREATED | regs::d(0) | regs::d(1) | regs::d(2)),
    native!(0xa7570, "eshl_fragments_42", fragments_42, CREATED | regs::d(1)),
    native!(0xa7da2, "eshl_fragments_45", fragments_45, CREATED | regs::d(0) | regs::d(1) | regs::d(2)),
    native!(0xa4e84, "eshl_serie_36", series_36, CREATED | regs::d(5)),
    native!(0xa5c9a, "eshl_serie_3b", series_3b, CREATED | regs::d(5)),
    native!(0xa5c7c, "eshl_lie_3b", single_3b, CREATED),
    native!(0xa6e6a, "eshl_lie_40", single_40, CREATED),
    native!(0x9dda8, "eshl_lance_1", thrown_1, CREATED | regs::d(0)),
    native!(0x9e062, "eshl_lance_2", thrown_2, CREATED | regs::d(0)),
    native!(0x9ed78, "eshl_balaye_8", sweeping_8, CREATED | regs::d(0)),
    native!(0x9eba6, "eshl_tir_6", shot_6, CREATED | regs::d(0)),
    native!(0x9e194, "eshl_quatre_3", four_3, CREATED | regs::d(1)),
    native!(0x9e2a0, "eshl_dessus_4", above_4, CREATED),
    native!(0x9eede, "eshl_paire_9", pair_9, CREATED | regs::d(0) | regs::a(2)),
    native!(0x9dbd6, "eshl_tir_0", shot_0, CREATED),
    native!(0x9e426, "eshl_douze_5", twelve_5, CREATED | regs::d(0) | regs::d(1) | regs::d(2)),
    native!(0xa1670, "eshl_rangee_19", row_19, CREATED | regs::d(0) | regs::d(3) | regs::d(4) | regs::d(5)),
    native!(0xa17d0, "eshl_rangee_1a", row_1a, CREATED | regs::d(0) | regs::d(1) | regs::d(3) | regs::d(4) | regs::d(5) | regs::a(2)),
    native!(0xa1c6a, "eshl_quatre_1c", four_1c, 0),
    native!(0xa31c2, "eshl_anneau_28", ring_28, CREATED | regs::d(1)),
    native!(0xa2f96, "eshl_anneau_27", ring_27, CREATED | regs::d(1)),
    native!(0xa6824, "eshl_paire_3f", pair_3f, CREATED),
    native!(0xa3354, "eshl_lie_29", single_29, CREATED),
    native!(0x9ddee, "eshl_tir_1", single_1, CREATED),
    native!(0xa460a, "eshl_lie_34_2", linked_34_2, CREATED),
    native!(0xa464e, "eshl_lie_34_1", linked_34_1, CREATED),
    native!(0xa4676, "eshl_lie_34_0", linked_34_0, CREATED),
    native!(0xa462c, "eshl_lie_34_3", linked_34_3, CREATED),
    native!(0xa2b4a, "eshl_lie_25", single_25, CREATED),
    native!(0xa5f38, "eshl_lie_3c", single_3c, CREATED),
    native!(0xa1360, "eshl_lie_18", single_18, CREATED),
    native!(0xa1a40, "eshl_lie_1b", single_1b, CREATED),
    native!(0xa44bc, "eshl_poursuivants_33", chasers, CREATED | regs::d(0) | regs::d(1) | regs::d(2) | regs::d(3) | regs::d(4) | regs::d(5)),
];
