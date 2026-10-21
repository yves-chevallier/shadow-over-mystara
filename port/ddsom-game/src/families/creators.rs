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

/// Allocations dans les pools : A1 = emplacement, retenue = pool plein.
const ALLOC_PSHL: u32 = 0x225e;
const ALLOC_PEFF: u32 = 0x228e;
const ALLOC_ESHL: u32 = 0x22ea;
const ALLOC_SET: u32 = 0x2348;
const ALLOC_ITEM: u32 = 0x23d8;

/// Alloue dans un pool ; `None` (retenue levée) si le pool est plein.
fn alloc(c: &mut Cpu, b: &mut Cps2Bus, routine: u32) -> Option<Obj> {
    call(c, b, routine);
    (c.sr & SR_C == 0).then(|| Obj(c.a[1]))
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
    let Some(shot) = alloc(c, b, ALLOC_PSHL) else {
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
fn player_effect(c: &mut Cpu, b: &mut Cps2Bus, kind: u8) -> Option<Obj> {
    let fx = alloc(c, b, ALLOC_PEFF)?;
    fx.set_u8(b, 0x0, 1);
    fx.set_u8(b, 0x2, kind);
    Some(fx)
}

/// Fin commune d'un effet : `+0x5d` = `+7` du joueur, `+0x50` = le joueur.
fn link_effect(c: &mut Cpu, b: &mut Cps2Bus, fx: Obj) {
    let me = Obj(c.a[0]);
    let step = me.u8(b, 0x7);
    fx.set_u8(b, 0x5d, step);
    fx.set_u16(b, 0x50, word(me));
    moved(c, word(me) as u32, Size::Word);
}

/// 0x4b7a0 : éclair du sort (PEFF 10, variante D0, rang `+3` du joueur en `+0x2f`).
fn bolt(c: &mut Cpu, b: &mut Cps2Bus) {
    if let Some(fx) = player_effect(c, b, 10) {
        let me = Obj(c.a[0]);
        fx.set_u8(b, 0x3, c.d[0] as u8);
        let rank = me.u8(b, 0x3);
        fx.set_u8(b, 0x2f, rank);
        link_effect(c, b, fx);
    }
    rts(c, b);
}

/// 0x4dc9a : deux étincelles du sort (PEFF 25, la seconde avec `+0x2f` = 1).
fn sparks(c: &mut Cpu, b: &mut Cps2Bus) {
    if let Some(fx) = player_effect(c, b, 25) {
        link_effect(c, b, fx);
        if let Some(fx) = player_effect(c, b, 25) {
            fx.set_u8(b, 0x2f, 1);
            link_effect(c, b, fx);
        }
    }
    rts(c, b);
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
    if let Some(shot) = alloc(c, b, ALLOC_ESHL) {
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
    if let Some(set) = alloc(c, b, ALLOC_SET) {
        set.set_u8(b, 0x0, 1);
        set.set_u8(b, 0x2, 13);
        let v = c.d[0] as u8;
        set.set_u8(b, 0x3, v);
        moved(c, v as u32, Size::Byte);
    }
    rts(c, b);
}

/// 0xc8312 : objet SET de variante D0, dont le type dépend de la scène (`A5+0x73 / 2`, table 0xc834c) ;
/// compte les SET créés (`A5+0x51`, `A5-0x5b8e`). D0 = indice de la scène, Z = (D0.b == 0).
fn scene_set(c: &mut Cpu, b: &mut Cps2Bus) {
    if let Some(set) = alloc(c, b, ALLOC_SET) {
        for at in [A5 + 0x51, A5 - 0x5b8e] {
            let n = rd8(b, at).wrapping_add(1);
            wr8(b, at, n);
        }
        set.set_u8(b, 0x0, 1);
        let v = c.d[0] as u8;
        set.set_u8(b, 0x3, v);
        set.set_u8(b, 0x22, 0);
        set.set_u8(b, 0x38, 0);
        set.set_u8(b, 0x37, 0xff);
        let scene = (rd8(b, A5 + 0x73) >> 1) as u32;
        c.d[0] = scene;
        let kind = prog8(b, 0xc834c + scene);
        set.set_u8(b, 0x2, kind);
        moved(c, scene, Size::Byte);
    }
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

/// 0xcc352 / 0xcc38e : objet SET 0xa de variante D0 (effet d'un objet utilisé), qui retient la classe de
/// `classe` (`+0x2f`) ; refusé (retenue seule, `move #1,CCR`) s'il en existe déjà un ou si le pool est plein.
fn set_a(c: &mut Cpu, b: &mut Cps2Bus, classe: fn(&mut Cpu, &mut Cps2Bus) -> u8) {
    let cree = (rd8(b, SET_A_ACTIF) == 0 && rd8(b, SET_A_BLOQUE) == 0).then(|| alloc(c, b, ALLOC_SET)).flatten();
    match cree {
        Some(set) => {
            wr8(b, SET_A_ACTIF, 1);
            set.set_u8(b, 0x0, 1);
            set.set_u8(b, 0x2, 0xa);
            set.set_u8(b, 0x3, c.d[0] as u8);
            let v = classe(c, b);
            set.set_u8(b, 0x2f, v);
            moved(c, v as u32, Size::Byte);
        }
        None => c.sr = c.sr & 0xff00 | SR_C,
    }
    rts(c, b);
}

/// 0xcc352 : la classe est celle du créateur (A0).
fn set_a_createur(c: &mut Cpu, b: &mut Cps2Bus) {
    set_a(c, b, |c, b| rd8(b, c.a[0] + 3));
}

/// 0xcc38e : la classe est celle du joueur lié au créateur (`+0x50`, laissé dans A4).
fn set_a_joueur(c: &mut Cpu, b: &mut Cps2Bus) {
    set_a(c, b, |c, b| {
        c.a[4] = rd16(b, c.a[0] + 0x50) as i16 as i32 as u32;
        rd8(b, c.a[4] + 3)
    });
}

/// 0xdc5fe : objet ITEM 6 (objet de scène) de variante D0, sauf quand `A5-0x5ac6` est négatif (le
/// drapeau N du test reste alors levé) ; retenue si le pool est plein.
fn scene_item(c: &mut Cpu, b: &mut Cps2Bus) {
    let niveau = rd8(b, A5 - 0x5ac6);
    if (niveau as i8) < 0 {
        moved(c, niveau as u32, Size::Byte);
        return rts(c, b);
    }
    if let Some(item) = alloc(c, b, ALLOC_ITEM) {
        item.set_u8(b, 0x0, 1);
        item.set_u8(b, 0x2, 6);
        item.set_u8(b, 0x3, c.d[0] as u8);
        item.set_u8(b, 0x2f, 0);
        moved(c, 0, Size::Byte);
    }
    rts(c, b);
}

const fn native(entry: u32, name: &'static str, run: fn(&mut Cpu, &mut Cps2Bus), outputs: u32) -> Native {
    Native { entry, name, cost: |_, _| 100, run, touches_sound: false, outputs, exact_cost: false }
}

const CREATED: u32 = regs::a(1) | regs::CCR;

pub const NATIVES: &[Native] = &[
    native(0x51c06, "cree_tir_arme", weapon_shot, regs::d(0) | regs::d(1) | CREATED),
    native(0x4b7a0, "cree_eclair", bolt, CREATED),
    native(0x4dc9a, "cree_etincelles", sparks, CREATED),
    native(0x9ef20, "cree_tir_9", enemy_shot_9, regs::d(0) | CREATED),
    native(0xcdedc, "cree_set_13", set_13, CREATED),
    native(0xc8312, "cree_set_scene", scene_set, regs::d(0) | CREATED),
    native(0xd981a, "fin_de_scene_variante", scene_exit, regs::d(0) | regs::a(1)),
    native(0xcc352, "cree_set_a", set_a_createur, CREATED),
    native(0xdc5fe, "cree_item_scene", scene_item, CREATED),
    native(0xcc38e, "cree_set_a_joueur", set_a_joueur, CREATED | regs::a(4)),
];
