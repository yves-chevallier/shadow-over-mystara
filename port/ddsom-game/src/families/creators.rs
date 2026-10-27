//! Petites routines de création d'objets appelées par le code des joueurs, des ennemis et des scènes :
//! elles allouent un emplacement dans un pool, posent la classe, le type et le lien vers le créateur.
//!
//! Contrat commun : A0 = le créateur ; au retour A1 = l'objet créé et la retenue levée si le pool est
//! plein (les appelants testent `bcs`) ; les autres drapeaux sont ceux de la dernière écriture.

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::{Cpu, SR_C};

use crate::mem::{prog32, prog8, rd16, rd8, rts, wr16, wr8, A5};
use crate::objects::Obj;
use crate::{call, regs, Native};
use crate::objects::pools;
use super::effects_creators::restes::{Pur, Registres};


/// Alloue dans un pool ; `None` (retenue levée) si le pool est plein.
fn alloc(c: &mut Cpu, b: &mut Cps2Bus, pool: &pools::Pool) -> Option<Obj> {
    pool.alloc_68k(c, b)
}

/// Drapeaux d'un `move` de `v` (taille `size`) : N et Z selon la valeur, V et C à zéro.
fn moved(c: &mut Cpu, v: u32, size: Size) {
    c.flags_logic(v, size);
}

/// Mot d'adresse d'un objet (`move.w An,...`).
fn word(o: Obj) -> u16 {
    o.0 as u16
}

/// 0x51c06 : tir de joueur (PSHL) qui accompagne l'arme du joueur A0, si son genre `+2` (décalé de 4
/// quand `+0x28` != 2) est dans le masque de la table 0x51c5c ; le tir et le joueur se lient par `+0x5c`.
fn weapon_shot(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = Obj(c.a[0]);
    me.set_u16(b, 0x5c, 0);
    let shift = if me.u8(b, 0x28) == 2 { 0 } else { 4 };
    c.d[1] = prog32(b, 0x51c5c + shift);
    let bit = (shift as u8).wrapping_add(me.u8(b, 0x2));
    c.d[0] = bit as u32;
    if c.d[1] >> (bit & 31) & 1 == 0 {
        c.sr = c.sr & 0xff00 | SR_C;
        return rts(c, b);
    }
    let Some(shot) = alloc(c, b, &pools::PSHL) else {
        c.sr = c.sr & 0xff00 | SR_C;
        return rts(c, b);
    };
    me.set_u8(b, 0x38, 0);
    shot.set_u8(b, 0x0, 1);
    shot.set_u8(b, 0x2, 0);
    let flip = me.u8(b, 0x2c);
    shot.set_u8(b, 0x2c, flip);
    let owner = me.u16(b, 0x54);
    shot.set_u16(b, 0x50, owner);
    me.set_u16(b, 0x5c, word(shot));
    shot.set_u16(b, 0x5c, word(me));
    let power = me.u8(b, 0x49);
    shot.set_u8(b, 0x42, power);
    moved(c, power as u32, Size::Byte);
    rts(c, b);
}

/// Effet PEFF de genre `kind` (à lier ensuite au joueur par `link_effect`).
fn player_effect(r: &mut impl Registres, b: &mut Cps2Bus, kind: u8) -> Option<Obj> {
    let fx = r.alloue(b, &pools::PEFF)?;
    fx.set_u8(b, Obj::ACTIVE, 1);
    fx.set_u8(b, Obj::KIND, kind);
    Some(fx)
}

/// Fin commune d'un effet : `+0x5d` = pas `+7` du joueur, `+0x50` = le joueur (drapeaux de ce `move.w`).
fn link_effect(r: &mut impl Registres, b: &mut Cps2Bus, fx: Obj, joueur: Obj) {
    let step = joueur.u8(b, 0x7);
    fx.set_u8(b, 0x5d, step);
    fx.set_u16(b, 0x50, joueur.word());
    r.drapeaux(joueur.word() as u32, Size::Word);
}

/// 0x4b7a0 : éclair du sort (PEFF 10, variante `variante`, rang `+3` du joueur en `+0x2f`).
pub fn bolt(b: &mut Cps2Bus, joueur: Obj, variante: u8) -> Option<Obj> {
    bolt_par(&mut Pur, b, joueur, variante)
}

/// `bolt` avec les registres de l'original (appelants par adresse : `player/`) : A1, retenue et drapeaux.
#[track_caller]
pub fn bolt_68k(c: &mut Cpu, b: &mut Cps2Bus, joueur: Obj, variante: u8) -> Option<Obj> {
    c.a[0] = joueur.0;
    let r = bolt_par(c, b, joueur, variante);
    crate::diag::registres_morts(c, "effets", CREATED);
    r
}

pub(crate) fn bolt_par(r: &mut impl Registres, b: &mut Cps2Bus, joueur: Obj, variante: u8) -> Option<Obj> {
    let fx = player_effect(r, b, 10)?;
    fx.set_u8(b, Obj::VARIANT, variante);
    let rank = joueur.u8(b, 0x3);
    fx.set_u8(b, 0x2f, rank);
    link_effect(r, b, fx, joueur);
    Some(fx)
}

/// 0x4dc9a : deux étincelles du sort (PEFF 25, la seconde avec `+0x2f` = 1). Rend la seconde.
pub fn sparks(b: &mut Cps2Bus, joueur: Obj) -> Option<Obj> {
    sparks_par(&mut Pur, b, joueur)
}

/// `sparks` avec les registres de l'original (appelants par adresse : `player/`).
#[track_caller]
pub fn sparks_68k(c: &mut Cpu, b: &mut Cps2Bus, joueur: Obj) -> Option<Obj> {
    c.a[0] = joueur.0;
    let r = sparks_par(c, b, joueur);
    crate::diag::registres_morts(c, "effets", CREATED);
    r
}

pub(crate) fn sparks_par(r: &mut impl Registres, b: &mut Cps2Bus, joueur: Obj) -> Option<Obj> {
    let fx = player_effect(r, b, 25)?;
    link_effect(r, b, fx, joueur);
    let fx = player_effect(r, b, 25)?;
    fx.set_u8(b, 0x2f, 1);
    link_effect(r, b, fx, joueur);
    Some(fx)
}

/// 0x9ef20 : tir ESHL 9 lié à l'ennemi A0, une image sur quatre (compteur `A5+0x6e`).
fn enemy_shot_9(c: &mut Cpu, b: &mut Cps2Bus) {
    let phase = rd16(b, A5 + 0x6e) & 3;
    c.d[0] = c.d[0] & 0xffff_0000 | phase as u32;
    if phase != 0 {
        moved(c, phase as u32, Size::Word);
        return rts(c, b);
    }
    let me = c.a[0];
    if let Some(shot) = alloc(c, b, &pools::ESHL) {
        shot.set_u8(b, 0x0, 1);
        shot.set_u8(b, 0x2, 9);
        shot.set_u8(b, 0x3, 0);
        wr16(b, shot.0 + 0x50, me as u16);
        moved(c, me as u16 as u32, Size::Word);
    }
    rts(c, b);
}

/// 0xcdedc : objet SET 13 de variante D0.
fn set_13(c: &mut Cpu, b: &mut Cps2Bus) {
    if let Some(set) = alloc(c, b, &pools::SET) {
        set.set_u8(b, 0x0, 1);
        set.set_u8(b, 0x2, 13);
        let v = c.d[0] as u8;
        set.set_u8(b, 0x3, v);
        moved(c, v as u32, Size::Byte);
    }
    rts(c, b);
}

/// 0xc8312 : objet SET de variante `variante`, dont le type dépend de la scène (`A5+0x73 / 2`, table 0xc834c) ;
/// compte les SET créés (`A5+0x51`, `A5-0x5b8e`). Le processeur reçoit D0 = indice de la scène, Z = (D0.b == 0).
pub fn scene_set(b: &mut Cps2Bus, variante: u8) -> Option<Obj> {
    scene_set_par(&mut Pur, b, variante)
}

pub(crate) fn scene_set_par(r: &mut impl Registres, b: &mut Cps2Bus, variante: u8) -> Option<Obj> {
    let set = r.alloue(b, &pools::SET)?;
    for at in [A5 + 0x51, A5 - 0x5b8e] {
        let n = rd8(b, at).wrapping_add(1);
        wr8(b, at, n);
    }
    set.set_u8(b, Obj::ACTIVE, 1);
    set.set_u8(b, Obj::VARIANT, variante);
    set.set_u8(b, 0x22, 0);
    set.set_u8(b, 0x38, 0);
    set.set_u8(b, 0x37, 0xff);
    let scene = (rd8(b, A5 + 0x73) >> 1) as u32;
    r.d(0, scene);
    let kind = prog8(b, 0xc834c + scene);
    set.set_u8(b, Obj::KIND, kind);
    r.drapeaux(scene, Size::Byte);
    Some(set)
}

fn scene_set_native(c: &mut Cpu, b: &mut Cps2Bus) {
    let v = c.d[0] as u8;
    scene_set_par(c, b, v);
    rts(c, b);
}

/// 0xd981a : fin de scène propre à la variante (`+3`, table d'indices 0xd9836, routines 0xd982e : en
/// pratique toujours 0xd99f8, un simple retour).
fn scene_exit(c: &mut Cpu, b: &mut Cps2Bus) {
    let variant = rd8(b, c.a[0] + 3) as u32;
    let i = 4 * prog8(b, 0xd9836 + variant) as u32;
    c.d[0] = i;
    c.a[1] = prog32(b, 0xd982e + i);
    let target = c.a[1];
    call(c, b, target);
    rts(c, b);
}

/// Un seul objet SET 0xa à la fois (`A5-0x5b8c`), et aucun pendant `A5-0x5fbe`.
const SET_A_ACTIF: u32 = A5 - 0x5b8c;
const SET_A_BLOQUE: u32 = A5 - 0x5fbe;

/// 0xcc352 / 0xcc38e : objet SET 0xa de variante `variante` (effet d'un objet utilisé, baguette), qui retient la
/// classe `classe` (`+0x2f`) ; refusé (le processeur reçoit la retenue seule, `move #1,CCR`) s'il en existe déjà
/// un, pendant `SET_A_BLOQUE` ou si le pool est plein. La classe est lue après l'allocation.
pub(crate) fn set_a_par(r: &mut impl Registres, b: &mut Cps2Bus, variante: u8, classe: impl FnOnce(&mut Cps2Bus) -> u8) -> Option<Obj> {
    let cree = (rd8(b, SET_A_ACTIF) == 0 && rd8(b, SET_A_BLOQUE) == 0).then(|| r.alloue(b, &pools::SET)).flatten();
    let Some(set) = cree else {
        r.retenue_seule();
        return None;
    };
    wr8(b, SET_A_ACTIF, 1);
    set.set_u8(b, Obj::ACTIVE, 1);
    set.set_u8(b, Obj::KIND, 0xa);
    set.set_u8(b, Obj::VARIANT, variante);
    let v = classe(b);
    set.set_u8(b, 0x2f, v);
    r.drapeaux(v as u32, Size::Byte);
    Some(set)
}

/// 0xcc352 : la classe est celle du créateur.
pub fn set_a(b: &mut Cps2Bus, createur: Obj, variante: u8) -> Option<Obj> {
    set_a_par(&mut Pur, b, variante, |b| createur.u8(b, Obj::VARIANT))
}

/// `set_a` avec les registres de l'original (appelants par adresse : `player/`) : A0 = le créateur, D0.b = la
/// variante ; A1, retenue et drapeaux.
#[track_caller]
pub fn set_a_68k(c: &mut Cpu, b: &mut Cps2Bus, createur: Obj, variante: u8) -> Option<Obj> {
    c.a[0] = createur.0;
    let r = set_a_par(c, b, variante, |b| createur.u8(b, Obj::VARIANT));
    crate::diag::registres_morts(c, "effets", CREATED);
    r
}

fn set_a_createur(c: &mut Cpu, b: &mut Cps2Bus) {
    let (me, v) = (Obj(c.a[0]), c.d[0] as u8);
    set_a_par(c, b, v, |b| me.u8(b, Obj::VARIANT));
    rts(c, b);
}

/// 0xcc38e : la classe est celle du joueur lié au créateur (`+0x50`, laissé dans A4).
pub(crate) fn set_a_joueur_par(r: &mut impl Registres, b: &mut Cps2Bus, createur: Obj, variante: u8) -> Option<Obj> {
    let mut joueur = None;
    let cree = set_a_par(r, b, variante, |b| {
        let p = Obj(rd16(b, createur.0 + 0x50) as i16 as i32 as u32);
        joueur = Some(p);
        p.u8(b, Obj::VARIANT)
    });
    if let Some(p) = joueur {
        r.a(4, p.0);
    }
    cree
}

/// 0xcc38e : `set_a`, la classe étant celle du joueur lié au créateur (`+0x50`, laissé dans A4).
pub fn set_a_joueur(b: &mut Cps2Bus, createur: Obj, variante: u8) -> Option<Obj> {
    set_a_joueur_par(&mut Pur, b, createur, variante)
}

fn set_a_joueur_native(c: &mut Cpu, b: &mut Cps2Bus) {
    let (me, v) = (Obj(c.a[0]), c.d[0] as u8);
    set_a_joueur_par(c, b, me, v);
    rts(c, b);
}

/// 0xdc5fe : objet ITEM 6 (objet de scène) de variante D0, sauf quand `A5-0x5ac6` est négatif (le
/// drapeau N du test reste alors levé) ; retenue si le pool est plein.
fn scene_item(c: &mut Cpu, b: &mut Cps2Bus) {
    let niveau = rd8(b, A5 - 0x5ac6);
    if (niveau as i8) < 0 {
        moved(c, niveau as u32, Size::Byte);
        return rts(c, b);
    }
    if let Some(item) = alloc(c, b, &pools::ITEM) {
        item.set_u8(b, 0x0, 1);
        item.set_u8(b, 0x2, 6);
        item.set_u8(b, 0x3, c.d[0] as u8);
        item.set_u8(b, 0x2f, 0);
        moved(c, 0, Size::Byte);
    }
    rts(c, b);
}

/// 0xd9778 : musique propre à la variante de la scène (`+3`, table d'indices 0xd9798, routines 0xd978c :
/// rien (0xd99f8) ou la musique de la scène 0xe0c5c). D0 = décalage dans la table, A1 = la routine.
fn scene_music(c: &mut Cpu, b: &mut Cps2Bus) {
    let variant = rd8(b, c.a[0] + 3) as u32;
    let i = 4 * prog8(b, 0xd9798 + variant) as u32;
    c.d[0] = i;
    c.a[1] = prog32(b, 0xd978c + i);
    let target = c.a[1];
    call(c, b, target);
    rts(c, b);
}

/// 0xd99f8 : routine vide des tables de scène.
fn nothing(c: &mut Cpu, b: &mut Cps2Bus) {
    rts(c, b);
}

/// 0xce00a : objet SET 15 de variante D0.
fn set_15(c: &mut Cpu, b: &mut Cps2Bus) {
    if let Some(set) = alloc(c, b, &pools::SET) {
        set.set_u8(b, 0x0, 1);
        set.set_u8(b, 0x2, 0xf);
        let v = c.d[0] as u8;
        set.set_u8(b, 0x3, v);
        moved(c, v as u32, Size::Byte);
    }
    rts(c, b);
}

/// 0xdfe26 : objet SOBJ 14.
fn sobj_14(c: &mut Cpu, b: &mut Cps2Bus) {
    if let Some(o) = alloc(c, b, &pools::SOBJ) {
        o.set_u8(b, 0x0, 1);
        o.set_u8(b, 0x2, 0xe);
        moved(c, 0xe, Size::Byte);
    }
    rts(c, b);
}

/// 0xdfc78 : objet SOBJ 12 de variante D0, vitesse nulle ; drapeaux de D0.b.
fn sobj_12(c: &mut Cpu, b: &mut Cps2Bus) {
    if let Some(o) = alloc(c, b, &pools::SOBJ) {
        o.set_u8(b, 0x0, 1);
        o.set_u8(b, 0x2, 0xc);
        let v = c.d[0] as u8;
        o.set_u8(b, 0x3, v);
        o.set_u16(b, 0x14, 0);
        moved(c, v as u32, Size::Byte);
    }
    rts(c, b);
}

/// 0xdf688 : objet SOBJ 8 lié au créateur (`+0x14`), variante 1 si le créateur a lui-même un lien.
pub fn sobj_8(b: &mut Cps2Bus, createur: Obj) -> Option<Obj> {
    sobj_8_par(&mut Pur, b, createur)
}

pub(crate) fn sobj_8_par(r: &mut impl Registres, b: &mut Cps2Bus, createur: Obj) -> Option<Obj> {
    let o = r.alloue(b, &pools::SOBJ)?;
    o.set_u8(b, Obj::ACTIVE, 1);
    o.set_u8(b, Obj::KIND, 8);
    o.set_u16(b, 0x14, createur.word());
    let linked = createur.u16(b, 0x14);
    r.drapeaux(linked as u32, Size::Word);
    if linked != 0 {
        o.set_u8(b, Obj::VARIANT, 1);
        r.drapeaux(1, Size::Byte);
    }
    Some(o)
}

/// Barre de vie du boss (objet SET 0x14) retenue par 0xd5f30 (mot bas de son adresse).
const LIFE_BAR_SET: u32 = A5 - 0x5b8a;

/// 0xd5f30 : en partie, objet SET 0x14 (variante 0) lié au créateur `createur` (`+0x20`), retenu en
/// `A5-0x5b8a`. Cœur.
pub fn set_14(b: &mut Cps2Bus, createur: Obj) -> Option<Obj> {
    set_14_par(&mut Pur, b, createur)
}

fn set_14_par(r: &mut impl Registres, b: &mut Cps2Bus, createur: Obj) -> Option<Obj> {
    let game = crate::ram::PARTIE.get(b);
    r.drapeaux(game as u32, Size::Byte);
    if game == 0 {
        return None;
    }
    let set = r.alloue(b, &pools::SET)?;
    set.set_u8(b, Obj::ACTIVE, 1);
    set.set_u8(b, Obj::KIND, 0x14);
    set.set_u16(b, 0x20, createur.word());
    set.set_u8(b, Obj::VARIANT, 0);
    wr16(b, LIFE_BAR_SET, set.word());
    r.drapeaux(set.word() as u32, Size::Word);
    Some(set)
}

/// Valeur de `+0x42` de l'objet SET 0x15 de 0xd6952, par type du créateur.
const SET_15_PARAMS: u32 = 0xd697c;

/// 0xd6952 : objet SET 0x15 (variante 2) dont `+0x42` vient de la table 0xd697c selon le type du créateur. Cœur.
pub fn set_15_b(b: &mut Cps2Bus, createur: Obj) -> Option<Obj> {
    set_15_b_par(&mut Pur, b, createur)
}

fn set_15_b_par(r: &mut impl Registres, b: &mut Cps2Bus, createur: Obj) -> Option<Obj> {
    let kind = createur.u8(b, Obj::KIND) as u32;
    let set = r.alloue(b, &pools::SET)?;
    set.set_u8(b, Obj::ACTIVE, 1);
    set.set_u8(b, Obj::KIND, 0x15);
    set.set_u8(b, Obj::VARIANT, 2);
    r.d(0, 2 * kind);
    let v = crate::mem::prog16(b, SET_15_PARAMS + 2 * kind);
    set.set_u16(b, 0x42, v);
    r.drapeaux(v as u32, Size::Word);
    Some(set)
}

/// 0xc9760 : objet SET 5 (variante 1), `+0x24` nul.
fn set_5(c: &mut Cpu, b: &mut Cps2Bus) {
    if let Some(set) = alloc(c, b, &pools::SET) {
        set.set_u8(b, 0x0, 1);
        set.set_u8(b, 0x2, 5);
        set.set_u8(b, 0x3, 1);
        set.set_u8(b, 0x24, 0);
        moved(c, 0, Size::Byte);
    }
    rts(c, b);
}

/// 0xd6182 : 44 cases d'une colonne de tuiles (A1, pas de 0x80) remplies de 0x00200000 (tuile vide).
fn blank_column(c: &mut Cpu, b: &mut Cps2Bus) {
    for _ in 0..0x2c {
        crate::mem::wr32(b, c.a[1], 0x20_0000);
        c.a[1] = c.a[1].wrapping_add(0x80);
    }
    c.d[0] = 0xffff;
    c.d[4] = 0x20_0000;
    rts(c, b);
}

/// 0xceb46 : fiche de bonus en A4 (octet de garde, puis 9 octets) : si la garde est positive, chaque
/// caractéristique `+0x151..` du joueur A0 prend le maximum ; A4 passe la fiche.
fn bonus_sheet(c: &mut Cpu, b: &mut Cps2Bus) {
    let guard = rd8(b, c.a[4]);
    c.a[4] = c.a[4].wrapping_add(1);
    if (guard as i8) < 0 {
        c.a[4] = c.a[4].wrapping_add(9);
        return rts(c, b);
    }
    let mut at = c.a[0] + 0x151;
    for _ in 0..9 {
        let v = rd8(b, c.a[4]);
        c.a[4] = c.a[4].wrapping_add(1);
        c.d[1] = c.d[1] & !0xff | v as u32;
        if v as i8 > rd8(b, at) as i8 {
            wr8(b, at, v);
        }
        at += 1;
    }
    c.a[1] = at;
    c.d[0] = 0xffff;
    rts(c, b);
}

/// 0xcc0ca : la scène repart de son début (h) : caméras (0x33aa6), décor et palettes (0xf2e56, 0x1a360,
/// 0x1a678, 0x1a8ac), luminosité pleine (0x1686) ; chaque joueur vivant revient en jeu normal (masqué et
/// en entrée scénarisée s'il était à terre ou mort), est replacé (0x20de6) puis trié. A0 et D7 conservés.
pub(crate) fn restart_scene(c: &mut Cpu, b: &mut Cps2Bus) {
    let (a0, d7) = (c.a[0], c.d[7]);
    call(c, b, 0x33aa6);
    (c.a[0], c.d[7]) = (a0, d7);
    call(c, b, 0xf2e56);
    let (a0, d7) = (c.a[0], c.d[7]);
    for routine in [0x1a360, 0x1a678, 0x1a8ac] {
        call(c, b, routine);
    }
    (c.a[0], c.d[7]) = (a0, d7);
    wr8(b, A5 + 0x48e, 0xf);
    crate::palette::full_brightness(b);
    wr16(b, A5 + 0x52, 0);
    let alive = |b: &mut Cps2Bus, p: u32| rd8(b, p) != 0 && rd16(b, p + 0x62) != 0;
    for k in 0..4 {
        let p = A5 + 0x5de + 0x200 * k;
        if !alive(b, p) {
            continue;
        }
        wr8(b, p, 1);
        let state = rd8(b, p + 4);
        c.d[0] = c.d[0] & !0xff | state as u32;
        crate::mem::wr32(b, p + 4, 0);
        let flags = rd8(b, p + 1) & !2;
        wr8(b, p + 1, flags);
        for off in [0x66, 0x15c, 0x8a, 0x10f] {
            wr8(b, p + off, 0);
        }
        if state as i8 > 2 {
            wr8(b, p + 1, flags | 2);
            wr16(b, p + 4, 0x402);
        }
    }
    c.a[1] = A5 + 0x5de + 0x800;
    call(c, b, 0x20de6);
    let a0 = c.a[0];
    for k in 0..4 {
        let p = A5 + 0x5de + 0x200 * k;
        if alive(b, p) {
            c.a[0] = p;
            crate::objects::lists::ACTORS.reinsert_68k(c, b, Obj(c.a[0]));
        }
    }
    c.a[0] = a0;
    c.d[6] = 0xffff;
}

/// Tir de joueur créé : actif, type `kind`, résistances `+0x45`/`+0x46` lues dans deux tables (en clair)
/// selon le niveau `+0x10c` du joueur (A4 garde la seconde table).
fn shot_strength(c: &mut Cpu, b: &mut Cps2Bus, shot: Obj, level: u8, tables: (u32, u32)) {
    let (a, d) = (rd8(b, tables.0 + level as u32), rd8(b, tables.1 + level as u32));
    shot.set_u8(b, 0x45, a);
    shot.set_u8(b, 0x46, d);
    c.a[4] = tables.1;
    moved(c, d as u32, Size::Byte);
}

/// 0x53e48 : éclair du joueur (PSHL 0x13), vitesses D0.w et D1.w, tant qu'il en reste (`A5-0x5fb4`).
fn bolt_shot(c: &mut Cpu, b: &mut Cps2Bus) {
    let left = rd16(b, A5 - 0x5fb4);
    moved(c, left as u32, Size::Word);
    if left == 0 {
        return rts(c, b);
    }
    wr16(b, A5 - 0x5fb4, left - 1);
    let me = Obj(c.a[0]);
    if let Some(shot) = alloc(c, b, &pools::PSHL) {
        shot.set_u8(b, 0x0, 1);
        shot.set_u8(b, 0x2, 0x13);
        shot.set_u16(b, 0x50, me.0 as u16);
        shot.set_u16(b, 0x14, c.d[0] as u16);
        shot.set_u16(b, 0x18, c.d[1] as u16);
        let level = me.u8(b, 0x10c);
        c.d[6] = level as u32;
        shot_strength(c, b, shot, level, (0x1042a6, 0x1042c4));
    }
    rts(c, b);
}

/// 0x52578 : tir de type 5 à la position et dans l'orientation du créateur, même joueur ; pool plein : D0 = 0.
fn shot_5(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = Obj(c.a[0]);
    let Some(shot) = alloc(c, b, &pools::PSHL) else {
        c.d[0] = 0;
        moved(c, 0, Size::Long);
        return rts(c, b);
    };
    shot.set_u8(b, 0x0, 1);
    shot.set_u8(b, 0x2, 5);
    for off in [0x8, 0xc, 0x10] {
        let v = me.u16(b, off);
        shot.set_u16(b, off, v);
    }
    let flip = me.u8(b, 0x2c);
    shot.set_u8(b, 0x2c, flip);
    let owner = me.u16(b, 0x50);
    shot.set_u16(b, 0x50, owner);
    let level = me.u8(b, 0x10c);
    c.d[1] = level as u32;
    shot_strength(c, b, shot, level, (0x10426a, 0x104288));
    rts(c, b);
}

/// 0x54842 : tir de type 0x18 (variante 1, résistances 0x10) lié par `+0x5c` ; D0 = 1, ou 0 si le pool est plein.
fn shot_18(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = c.a[0];
    c.d[0] = match alloc(c, b, &pools::PSHL) {
        Some(shot) => {
            shot.set_u8(b, 0x0, 1);
            shot.set_u8(b, 0x2, 0x18);
            shot.set_u8(b, 0x3, 1);
            shot.set_u8(b, 0x45, 0x10);
            shot.set_u8(b, 0x46, 0x10);
            shot.set_u16(b, 0x5c, me as u16);
            1
        }
        None => 0,
    };
    moved(c, c.d[0], Size::Long);
    rts(c, b);
}

/// Effet de joueur (PEFF) de type `kind`, variante `variant`, lié au joueur (`+0x50`).
fn peff(c: &mut Cpu, b: &mut Cps2Bus, kind: u8, variant: Option<u8>) -> Option<Obj> {
    let me = c.a[0];
    let fx = alloc(c, b, &pools::PEFF)?;
    fx.set_u8(b, 0x0, 1);
    fx.set_u8(b, 0x2, kind);
    if let Some(v) = variant {
        fx.set_u8(b, 0x3, v);
    }
    fx.set_u16(b, 0x50, me as u16);
    moved(c, me & 0xffff, Size::Word);
    Some(fx)
}

/// 0x4b8d0 : effet 0xb (variante 0).
fn peff_b(c: &mut Cpu, b: &mut Cps2Bus) {
    peff(c, b, 0xb, Some(0));
    rts(c, b);
}

/// 0x4b518 : effet 8.
fn peff_8(c: &mut Cpu, b: &mut Cps2Bus) {
    peff(c, b, 8, None);
    rts(c, b);
}

/// 0x51bb6 : effet 0x2f (sans lien).
fn peff_2f(c: &mut Cpu, b: &mut Cps2Bus) {
    if let Some(fx) = alloc(c, b, &pools::PEFF) {
        fx.set_u8(b, 0x0, 1);
        fx.set_u8(b, 0x2, 0x2f);
        moved(c, 0x2f, Size::Byte);
    }
    rts(c, b);
}

/// 0x4cfbc : quatre effets 0x13 (variante 1, `+0x2f` = 3 à 0, `+0x5a` = 2), puis l'éclair d'écran (0x1ac68).
fn peff_13_x4(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = c.a[0];
    c.d[5] = 3;
    loop {
        let Some(fx) = alloc(c, b, &pools::PEFF) else { break };
        fx.set_u8(b, 0x0, 1);
        fx.set_u8(b, 0x2, 0x13);
        fx.set_u8(b, 0x3, 1);
        fx.set_u8(b, 0x2f, c.d[5] as u8);
        fx.set_u8(b, 0x5a, 2);
        fx.set_u16(b, 0x50, me as u16);
        let n = (c.d[5] as u16).wrapping_sub(1);
        c.d[5] = c.d[5] & !0xffff | n as u32;
        if n == 0xffff {
            break;
        }
    }
    call(c, b, 0x1ac68);
    rts(c, b);
}

/// 0x4b340 (séisme des sorts) : `paire`, deux effets 7 (variante 0, `+0x2f` = 0 et 1) ; sinon un effet 7 de
/// variante 1. Le pas `+7` du joueur est recopié en `+0x5d`. Rend le dernier effet créé.
pub fn quake(b: &mut Cps2Bus, joueur: Obj, paire: bool) -> Option<Obj> {
    quake_par(&mut Pur, b, joueur, paire)
}

/// `quake` avec les registres de l'original (appelants par adresse : `player/`) : A0 = le joueur, D0.b nul pour la
/// paire ; A1, retenue et drapeaux.
#[track_caller]
pub fn quake_68k(c: &mut Cpu, b: &mut Cps2Bus, joueur: Obj, paire: bool) -> Option<Obj> {
    c.a[0] = joueur.0;
    let r = quake_par(c, b, joueur, paire);
    crate::diag::registres_morts(c, "effets", CREATED);
    r
}

pub(crate) fn quake_par<R: Registres>(r: &mut R, b: &mut Cps2Bus, joueur: Obj, paire: bool) -> Option<Obj> {
    let step = joueur.u8(b, 7);
    let effet = |r: &mut R, b: &mut Cps2Bus, variante: u8, index: Option<u8>| {
        let fx = player_effect(r, b, 7)?;
        fx.set_u8(b, Obj::VARIANT, variante);
        if let Some(i) = index {
            fx.set_u8(b, 0x2f, i);
        }
        fx.set_u8(b, 0x5d, step);
        fx.set_u16(b, 0x50, joueur.word());
        r.drapeaux(joueur.word() as u32, Size::Word);
        Some(fx)
    };
    if paire {
        effet(r, b, 0, Some(0))?;
        effet(r, b, 0, Some(1))
    } else {
        effet(r, b, 1, None)
    }
}

fn quake_native(c: &mut Cpu, b: &mut Cps2Bus) {
    let (me, paire) = (Obj(c.a[0]), c.d[0] as u8 == 0);
    quake_par(c, b, me, paire);
    rts(c, b);
}

/// 0x63ae0 : s'il reste plus de deux places d'ennemis (`A5+0x7e62`), deux ennemis de type 7 (variantes 0
/// et 1) liés au créateur par `+0x46` ; D0 = 1, sinon 0 (l'allocation n'est pas vérifiée).
fn enemy_7_pair(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = c.a[0];
    if rd16(b, A5 + 0x7e62) as i16 <= 2 {
        c.d[0] = 0;
        moved(c, 0, Size::Long);
        return rts(c, b);
    }
    for variant in [0, 1] {
        // Glitch d'origine : l'échec de l'allocation n'est pas testé (voir `Pool::alloc_unchecked`).
        let o = pools::ENEMY.alloc_unchecked_68k(c, b);
        o.set_u8(b, 0x0, 1);
        o.set_u8(b, 0x2, 7);
        o.set_u8(b, 0x3, variant);
        o.set_u8(b, 0x2f, 0);
        o.set_u16(b, 0x46, me as u16);
    }
    c.d[0] = 1;
    moved(c, 1, Size::Long);
    rts(c, b);
}

/// Ennemi de type `kind` (variante 5) placé en (D0, D1, D2) = (x, z, y), lié au créateur par `link` ;
/// drapeaux de D0.b (`or.b D0b,D0b`).
fn enemy_at(c: &mut Cpu, b: &mut Cps2Bus, kind: u8, link: u32, extra: &[(u32, u8)]) {
    let me = c.a[0];
    if let Some(o) = alloc(c, b, &pools::ENEMY) {
        o.set_u8(b, 0x0, 1);
        o.set_u8(b, 0x2, kind);
        o.set_u16(b, 0x8, c.d[0] as u16);
        o.set_u16(b, 0xc, c.d[1] as u16);
        o.set_u16(b, 0x10, c.d[2] as u16);
        o.set_u16(b, link, me as u16);
        o.set_u8(b, 0x3, 5);
        for &(off, v) in extra {
            o.set_u8(b, off, v);
        }
        moved(c, c.d[0] & 0xff, Size::Byte);
    }
    rts(c, b);
}

/// 0x5f764 : ennemi de type 5 en (D0, D1, D2), lié par `+0xa4`.
fn enemy_5_at(c: &mut Cpu, b: &mut Cps2Bus) {
    enemy_at(c, b, 5, 0xa4, &[]);
}

/// 0x57c8c : ennemi de type 1 en (D0, D1, D2), lié par `+0xaa` (`+0xac` = 1, `+0x2f` = 0).
fn enemy_1_at(c: &mut Cpu, b: &mut Cps2Bus) {
    enemy_at(c, b, 1, 0xaa, &[(0xac, 1), (0x2f, 0)]);
}

/// 0x57c66 : l'objet `o` est-il dans la moitié droite de l'écran (x >= caméra + 0xc0) ? Cœur.
pub fn in_right_half(b: &mut Cps2Bus, o: Obj) -> bool {
    let (camera, _) = crate::ram::camera(b);
    (camera.wrapping_add(0xc0).wrapping_sub(o.u16(b, Obj::X)) as i16) <= 0
}

/// 0x57c66 avec les registres de l'original : D0 = 1 / 0, D1.w = l'écart, X de la soustraction.
fn right_half(c: &mut Cpu, b: &mut Cps2Bus) {
    let (camera, _) = crate::ram::camera(b);
    let x = rd16(b, c.a[0] + 8);
    // `sub.w D0w,D1w` : X de la soustraction, que `moveq` garde
    let gap = crate::ops::sub(c, x as u32, camera.wrapping_add(0xc0) as u32, Size::Word) as u16;
    c.d[1] = c.d[1] & !0xffff | gap as u32;
    c.d[0] = in_right_half(b, Obj(c.a[0])) as u32;
    moved(c, c.d[0], Size::Long);
    rts(c, b);
}

/// 0x5cdc6 : tirage au sort, vrai 10 fois sur 16 (bit (hasard & 0xf) du masque 0x03ff). Cœur.
pub fn chance_10_16(b: &mut Cps2Bus) -> bool {
    let bit = crate::rng::random(b) & 0xf;
    0x03ff >> bit & 1 != 0
}

/// 0x5cdc6 avec les registres de l'original : D0 = 1 / 0, D1.w = 0x03ff.
fn chance_10_16_native(c: &mut Cpu, b: &mut Cps2Bus) {
    crate::rng::random_68k(c, b);
    let bit = c.d[0] as u16 & 0xf;
    c.d[1] = c.d[1] & !0xffff | 0x03ff;
    c.d[0] = (0x03ff >> bit & 1) as u32;
    moved(c, c.d[0], Size::Long);
    rts(c, b);
}

/// 0x97218 : boss : `+0x67` = 1, `+0x79` = 0x31, état 0x02020400.
fn boss_state_0202(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = Obj(c.a[0]);
    me.set_u8(b, 0x67, 1);
    me.set_u8(b, 0x79, 0x31);
    me.set_u32(b, 0x4, 0x0202_0400);
    moved(c, 0x0202_0400, Size::Long);
    rts(c, b);
}

const fn native(entry: u32, name: &'static str, run: fn(&mut Cpu, &mut Cps2Bus), outputs: u32) -> Native {
    Native { entry, name, run, touches_sound: false, outputs }
}

const CREATED: u32 = regs::a(1) | regs::CCR;

pub const NATIVES: &[Native] = &[
    native(0x51c06, "cree_tir_arme", weapon_shot, regs::d(0) | regs::d(1) | CREATED),
    native(0x4b7a0, "cree_eclair", |c, b| { let (me, v) = (Obj(c.a[0]), c.d[0] as u8); bolt_par(c, b, me, v); rts(c, b) }, CREATED),
    native(0x4dc9a, "cree_etincelles", |c, b| { let me = Obj(c.a[0]); sparks_par(c, b, me); rts(c, b) }, CREATED),
    native(0x9ef20, "cree_tir_9", enemy_shot_9, regs::d(0) | CREATED),
    native(0xcdedc, "cree_set_13", set_13, CREATED),
    native(0xc8312, "cree_set_scene", scene_set_native, regs::d(0) | CREATED),
    native(0xd981a, "fin_de_scene_variante", scene_exit, regs::d(0) | regs::a(1)),
    native(0xcc352, "cree_set_a", set_a_createur, CREATED),
    native(0xdc5fe, "cree_item_scene", scene_item, CREATED),
    native(0xd9778, "musique_variante", scene_music, regs::d(0) | regs::a(1)),
    native(0xd99f8, "scene_rien", nothing, regs::ALL),
    native(0xce00a, "cree_set_15", set_15, CREATED),
    native(0xd5f30, "cree_set_14", |c, b| { let me = Obj(c.a[0]); set_14_par(c, b, me); rts(c, b) }, CREATED),
    native(0xcc0ca, "scene_repart", |c, b| { restart_scene(c, b); rts(c, b) }, regs::a(0) | regs::d(7)),
    native(0x53e48, "cree_eclair_joueur", bolt_shot, CREATED | regs::d(6) | regs::a(4)),
    native(0x52578, "cree_tir_5", shot_5, CREATED | regs::d(0) | regs::d(1) | regs::a(4)),
    native(0x54842, "cree_tir_18", shot_18, CREATED | regs::d(0)),
    native(0x4b8d0, "cree_effet_b", peff_b, CREATED),
    native(0x4b518, "cree_effet_8", peff_8, CREATED),
    native(0x51bb6, "cree_effet_2f", peff_2f, CREATED),
    native(0x4cfbc, "cree_effets_13", peff_13_x4, CREATED | regs::d(5)),
    native(0x4b340, "cree_effets_7", quake_native, CREATED),
    native(0x63ae0, "cree_ennemis_7", enemy_7_pair, CREATED | regs::d(0)),
    native(0x5f764, "cree_ennemi_5", enemy_5_at, CREATED),
    native(0x57c8c, "cree_ennemi_1", enemy_1_at, CREATED),
    native(0x57c66, "ennemi_moitie_droite", right_half, regs::d(0) | regs::d(1) | regs::CCR),
    native(0x5cdc6, "hasard_10_16", chance_10_16_native, regs::d(0) | regs::d(1) | regs::CCR),
    native(0x97218, "boss_etat_0202", boss_state_0202, regs::CCR),
    native(0xd6952, "cree_set_15_b", |c, b| { let me = Obj(c.a[0]); set_15_b_par(c, b, me); rts(c, b) }, CREATED | regs::d(0)),
    native(0xc9760, "cree_set_5", set_5, CREATED),
    native(0xd6182, "colonne_vide", blank_column, regs::a(1) | regs::d(0) | regs::d(4)),
    native(0xceb46, "fiche_bonus", bonus_sheet, regs::a(1) | regs::a(4) | regs::d(0) | regs::d(1)),
    native(0xdfe26, "cree_sobj_14", sobj_14, CREATED),
    native(0xdfc78, "cree_sobj_12", sobj_12, CREATED),
    native(0xdf688, "cree_sobj_8", |c, b| { let me = Obj(c.a[0]); sobj_8_par(c, b, me); rts(c, b) }, CREATED),
    native(0xcc38e, "cree_set_a_joueur", set_a_joueur_native, CREATED | regs::a(4)),
];
