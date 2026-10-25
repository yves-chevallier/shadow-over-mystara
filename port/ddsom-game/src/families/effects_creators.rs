//! Créateurs d'effets (EFFCT) : les petites routines de la famille qui allouent un effet, posent son
//! type et sa variante, puis le lient à leur créateur. Appelées par les gestionnaires des joueurs, des
//! ennemis, des boss, des décors et des scènes, en Rust comme en 68000.
//!
//! Contrat commun : A0 = le créateur ; au retour A1 = l'effet créé, la retenue levée si le pool est
//! plein (les appelants testent `bcs`) ; les autres drapeaux sont ceux de la dernière écriture. Les
//! registres de travail (compteurs de boucle, tirages) sont rendus comme l'original les laisse.

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::{Cpu, SR_C};

use crate::mem::{prog16, prog32, rd16, rd32, rd8, rts, set_low_byte, set_low_word, wr16, wr32, wr8, A5};
use crate::objects::Obj;
use crate::ops::{add, logique, sub};
use crate::{call, regs, Native};

/// Allocations : effet (EFFCT), tir ennemi (ESHL), décor (SET). A1 = emplacement, retenue = échec.
const ALLOC_EFFECT: u32 = 0x2318;
const ALLOC_ESHL: u32 = 0x22ea;
const ALLOC_SET: u32 = 0x2348;
/// Générateur pseudo-aléatoire (D0).
const RNG: u32 = 0x19a88;

/// Joueurs : quatre enregistrements de 0x200 octets.
const PLAYERS: u32 = A5 + 0x5de;
const PLAYER_STRIDE: u32 = 0x200;
/// Caméra alternative (x, profondeur).
const ALT_CAMERA_X: u32 = A5 + 0x556;
const ALT_CAMERA_Y: u32 = A5 + 0x55a;

/// Types d'effets créés ici.
mod kind {
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
    pub const BLOW: u8 = 40;
    pub const TOSS: u8 = 41;
    pub const SCENERY: u8 = 42;
    pub const SPELL: u8 = 44;
    pub const BEACON: u8 = 47;
    pub const SHIMMER: u8 = 52;
    pub const TYPE_53: u8 = 53;
    pub const TYPE_54: u8 = 54;
    pub const PILLAR: u8 = 67;
    pub const TYPE_68: u8 = 68;
    pub const VICTORY: u8 = 73;
    pub const BUBBLE: u8 = 74;
    pub const TIMER: u8 = 76;
    pub const RAIN: u8 = 83;
    pub const FINALE: u8 = 84;
    pub const WISP: u8 = 86;
    pub const DRIFT: u8 = 90;
    /// Tirs ennemis (ESHL) créés par deux routines de la famille.
    pub const ESHL_RISE: u8 = 0x47;
    pub const ESHL_SLASH: u8 = 0x48;
}

fn d0b(c: &Cpu) -> u8 {
    c.d[0] as u8
}
fn d1b(c: &Cpu) -> u8 {
    c.d[1] as u8
}
/// Mot d'adresse d'un objet (`move.w An,...`), avec les drapeaux du `move`.
fn link(c: &mut Cpu, b: &mut Cps2Bus, o: Obj, off: u32, target: u32) {
    o.set_u16(b, off, target as u16);
    logique(c, target & 0xffff, Size::Word);
}
/// `move.b v,(off,A1)` avec ses drapeaux.
fn put8(c: &mut Cpu, b: &mut Cps2Bus, o: Obj, off: u32, v: u8) {
    o.set_u8(b, off, v);
    logique(c, v as u32, Size::Byte);
}

/// Alloue dans un pool ; `None` (retenue levée) si le pool est plein.
fn alloc(c: &mut Cpu, b: &mut Cps2Bus, routine: u32) -> Option<Obj> {
    call(c, b, routine);
    (c.sr & SR_C == 0).then(|| Obj(c.a[1]))
}

/// Alloue un objet et pose son activité et son type (`move.b #1,(A1)`, `move.b #type,(2,A1)`).
fn spawn_in(c: &mut Cpu, b: &mut Cps2Bus, routine: u32, kind: u8) -> Option<Obj> {
    let o = alloc(c, b, routine)?;
    o.set_u8(b, Obj::ACTIVE, 1);
    put8(c, b, o, Obj::KIND, kind);
    Some(o)
}

fn spawn(c: &mut Cpu, b: &mut Cps2Bus, kind: u8) -> Option<Obj> {
    spawn_in(c, b, ALLOC_EFFECT, kind)
}

/// Copie les mots hauts de X, Z, Y du créateur.
fn copy_position(b: &mut Cps2Bus, to: Obj, from: Obj) {
    for off in [Obj::X, Obj::Z, Obj::Y] {
        let v = from.u16(b, off);
        to.set_u16(b, off, v);
    }
}

fn me(c: &Cpu) -> Obj {
    Obj(c.a[0])
}

/// Tirage du générateur (D0).
fn random(c: &mut Cpu, b: &mut Cps2Bus) -> u16 {
    call(c, b, RNG);
    c.d[0] as u16
}

/// `andi.w #mask,D0` : garde les bits de `mask` du mot bas de D0 (drapeaux du résultat).
fn and_d0(c: &mut Cpu, mask: u16) -> u16 {
    let v = c.d[0] as u16 & mask;
    set_low_word(&mut c.d[0], v);
    logique(c, v as u32, Size::Word);
    v
}

// ---------------------------------------------------------------------------------------------
// Créateurs simples : type, variante (D0), réglage `+0x2f` (D1), lien au créateur

/// 0xae776 : élément d'interface (type 17 ; curseurs de l'écran de sélection...).
fn interface(c: &mut Cpu, b: &mut Cps2Bus) {
    spawn(c, b, kind::INTERFACE);
}

/// 0xaf346 : bandeau d'interface (type 21) de variante D0.
fn strip(c: &mut Cpu, b: &mut Cps2Bus) {
    if let Some(o) = spawn(c, b, kind::STRIP) {
        put8(c, b, o, Obj::VARIANT, d0b(c));
    }
}

/// 0xb1a7a : écran de fin de scène (type 31) : variante D0, réglage D1.
fn screen(c: &mut Cpu, b: &mut Cps2Bus) {
    if let Some(o) = spawn(c, b, kind::COUNTDOWN) {
        o.set_u8(b, Obj::VARIANT, d0b(c));
        put8(c, b, o, 0x2f, d1b(c));
    }
}

/// 0xafe02 : vote du chemin (type 23) : variante D0, réglage D1.
fn vote(c: &mut Cpu, b: &mut Cps2Bus) {
    if let Some(o) = spawn(c, b, kind::VOTE) {
        o.set_u8(b, Obj::VARIANT, d0b(c));
        put8(c, b, o, 0x2f, d1b(c));
    }
}

/// 0xab7d8 : gerbe de débris (type 6) de variante D0, lié au créateur par `+0x50`.
fn debris(c: &mut Cpu, b: &mut Cps2Bus) {
    if let Some(o) = spawn(c, b, kind::DEBRIS) {
        o.set_u8(b, Obj::VARIANT, d0b(c));
        o.set_u8(b, 0x42, 0);
        link(c, b, o, 0x50, c.a[0]);
    }
}

/// 0xabec8 : effet d'attaque (type 11) liée au créateur par `+0x5c` : variante D0, réglage D1.
fn attack(c: &mut Cpu, b: &mut Cps2Bus) {
    if let Some(o) = spawn(c, b, kind::ATTACK) {
        o.set_u16(b, 0x5c, c.a[0] as u16);
        o.set_u8(b, Obj::VARIANT, d0b(c));
        put8(c, b, o, 0x2f, d1b(c));
    }
}

/// 0xb378a : effets d'une attaque (type 40) liés par `+0x5c` : variante D0, réglage D1.
fn blow(c: &mut Cpu, b: &mut Cps2Bus) {
    if let Some(o) = spawn(c, b, kind::BLOW) {
        o.set_u16(b, 0x5c, c.a[0] as u16);
        o.set_u8(b, Obj::VARIANT, d0b(c));
        put8(c, b, o, 0x2f, d1b(c));
    }
}

/// 0xab708 : halo (type 5) de la variante du créateur, lié par `+0x50`.
fn halo(c: &mut Cpu, b: &mut Cps2Bus) {
    if let Some(o) = spawn(c, b, kind::HALO) {
        let v = me(c).u8(b, Obj::VARIANT);
        o.set_u8(b, Obj::VARIANT, v);
        link(c, b, o, 0x50, c.a[0]);
    }
}

/// 0xb1352 : étincelle posée (type 28) de variante D0, lié par `+0x5c`.
fn posed_spark(c: &mut Cpu, b: &mut Cps2Bus) {
    if let Some(o) = spawn(c, b, kind::POSED_SPARK) {
        o.set_u8(b, Obj::VARIANT, d0b(c));
        link(c, b, o, 0x5c, c.a[0]);
    }
}

/// 0xb141a : étincelle d'impact (type 29, variante 1) de vitesses D2, D3, liée par `+0x5c`.
fn impact(c: &mut Cpu, b: &mut Cps2Bus) {
    if let Some(o) = spawn(c, b, kind::IMPACT) {
        o.set_u8(b, Obj::VARIANT, 1);
        o.set_u16(b, Obj::VX, c.d[2] as u16);
        o.set_u16(b, Obj::AX, c.d[3] as u16);
        link(c, b, o, 0x5c, c.a[0]);
    }
}

/// 0xb12c0 : effet de ramassage (type 27) de variante D0, attachée au même objet que le créateur (`+0x50`, s'il
/// existe : pointeur négatif).
fn pickup(c: &mut Cpu, b: &mut Cps2Bus) {
    let owner = me(c).u16(b, 0x50);
    logique(c, owner as u32, Size::Word);
    if (owner as i16) >= 0 {
        return;
    }
    if let Some(o) = spawn(c, b, kind::PICKUP) {
        o.set_u8(b, Obj::VARIANT, d0b(c));
        let owner = me(c).u16(b, 0x50);
        link(c, b, o, 0x50, owner as u32);
    }
}

/// Créateur d'un type à variante fixe, lié au créateur par `+0x5c` (types 73, 74, 84, 86).
fn linked(c: &mut Cpu, b: &mut Cps2Bus, kind: u8, variant: Option<u8>) {
    if let Some(o) = spawn(c, b, kind) {
        if let Some(v) = variant {
            o.set_u8(b, Obj::VARIANT, v);
        }
        link(c, b, o, 0x5c, c.a[0]);
    }
}

/// 0xb8956 : minuteur de scène (type 76) de variante D1.
fn timer(c: &mut Cpu, b: &mut Cps2Bus) {
    if let Some(o) = spawn(c, b, kind::TIMER) {
        put8(c, b, o, Obj::VARIANT, d1b(c));
    }
}

/// 0xba18a : poussière qui dérive (type 90), orientée comme le créateur, liée par le long `+0x46`.
fn drift(c: &mut Cpu, b: &mut Cps2Bus) {
    if let Some(o) = spawn(c, b, kind::DRIFT) {
        let facing = me(c).u8(b, 0x2c);
        o.set_u8(b, 0x2c, facing);
        o.set_u32(b, 0x46, c.a[0]);
        logique(c, c.a[0], Size::Long);
    }
}

/// 0xb66e4 : effet de type 53 ; l'activité est incrémentée (`addq.b #1,(A1)`), non posée.
fn type_53(c: &mut Cpu, b: &mut Cps2Bus) {
    if let Some(o) = alloc(c, b, ALLOC_EFFECT) {
        bump_active(c, b, o);
        put8(c, b, o, Obj::KIND, kind::TYPE_53);
    }
}

/// `addq.b #1,(A1)`.
fn bump_active(c: &mut Cpu, b: &mut Cps2Bus, o: Obj) {
    let v = add(c, 1, o.u8(b, Obj::ACTIVE) as u32, Size::Byte);
    o.set_u8(b, Obj::ACTIVE, v as u8);
}

/// 0xb68b0 : trois effets de type 54, variantes 0 (laissée telle quelle), 1 et 2.
fn type_54_trio(c: &mut Cpu, b: &mut Cps2Bus) {
    for variant in 0..3u8 {
        let Some(o) = alloc(c, b, ALLOC_EFFECT) else { return };
        bump_active(c, b, o);
        put8(c, b, o, Obj::KIND, kind::TYPE_54);
        if variant != 0 {
            put8(c, b, o, Obj::VARIANT, variant);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Créateurs qui placent l'effet sur leur créateur

/// 0xae3ec : scintillement (type 15) de la variante `+0x54` du créateur, posée 16 pixels plus haut, liée
/// par `+0x5c` ; rien si `+0x54` est nul.
fn twinkle(c: &mut Cpu, b: &mut Cps2Bus) {
    let variant = me(c).u8(b, 0x54);
    logique(c, variant as u32, Size::Byte);
    if variant == 0 {
        return;
    }
    if let Some(o) = spawn(c, b, kind::TWINKLE) {
        let from = me(c);
        o.set_u8(b, Obj::VARIANT, variant);
        copy_position(b, o, from);
        let z = o.u16(b, Obj::Z).wrapping_add(0x10);
        o.set_u16(b, Obj::Z, z);
        link(c, b, o, 0x5c, c.a[0]);
    }
}

/// 0xb6596 : scintillement de personnage (type 52) posée juste devant le créateur, relevée selon son octet `+0x60`
/// (table de mots 0xb65d6), liée par `+0x5c`.
fn shimmer(c: &mut Cpu, b: &mut Cps2Bus) {
    let Some(o) = spawn(c, b, kind::SHIMMER) else { return };
    let from = me(c);
    copy_position(b, o, from);
    let y = o.u16(b, Obj::Y).wrapping_sub(1);
    o.set_u16(b, Obj::Y, y);
    o.set_u16(b, 0x5c, from.0 as u16);
    let k = 2 * from.u8(b, 0x60) as u32;
    let lift = prog16(b, 0xb65d6 + k);
    c.d[0] = lift as u32;
    let z = add(c, lift as u32, o.u16(b, Obj::Z) as u32, Size::Word);
    o.set_u16(b, Obj::Z, z as u16);
}

/// 0xb5730 : repère (type 47) à la place du créateur : réglage `+0x46`, même variante.
fn beacon(c: &mut Cpu, b: &mut Cps2Bus) {
    if let Some(o) = spawn(c, b, kind::BEACON) {
        let from = me(c);
        let setting = from.u8(b, 0x46);
        o.set_u8(b, 0x2f, setting);
        let v = from.u8(b, Obj::VARIANT);
        o.set_u8(b, Obj::VARIANT, v);
        copy_position(b, o, from);
        logique(c, from.u16(b, Obj::Y) as u32, Size::Word);
    }
}

/// 0xb3c14 : objet lancé (type 41) depuis le créateur, 24 pixels plus haut, même orientation.
fn toss(c: &mut Cpu, b: &mut Cps2Bus) {
    if let Some(o) = spawn(c, b, kind::TOSS) {
        let from = me(c);
        let facing = from.u8(b, 0x2c);
        o.set_u8(b, 0x2c, facing);
        copy_position(b, o, from);
        let z = add(c, 0x18, o.u16(b, Obj::Z) as u32, Size::Word);
        o.set_u16(b, Obj::Z, z as u16);
    }
}

/// 0xaba16 : éclat à animation unique (type 7) du créateur : orientation, position complète (longs) juste devant lui,
/// réglage `+0x2f`.
fn flash(c: &mut Cpu, b: &mut Cps2Bus) {
    if let Some(o) = spawn(c, b, kind::FLASH) {
        let from = me(c);
        let facing = from.u8(b, 0x2c);
        o.set_u8(b, 0x2c, facing);
        for off in [Obj::X, Obj::Z, Obj::Y] {
            let v = rd32(b, from.0 + off);
            wr32(b, o.0 + off, v);
        }
        let y = sub(c, 1, o.u32(b, Obj::Y), Size::Long);
        o.set_u32(b, Obj::Y, y);
        let setting = from.u8(b, 0x2f);
        put8(c, b, o, 0x2f, setting);
    }
}

/// 0xaaa40 : étincelle (type 4, variante D1) entre le créateur et l'objet A1, posée en (D2, D3, D4).
/// A1 à A4 sont rendus.
fn spark_at(c: &mut Cpu, b: &mut Cps2Bus) {
    let saved = [c.a[1], c.a[2], c.a[3], c.a[4]];
    let other = c.a[1];
    if let Some(o) = spawn(c, b, kind::SPARK) {
        let from = me(c);
        o.set_u8(b, Obj::VARIANT, d1b(c));
        let facing = from.u8(b, 0x2c) ^ 0x20;
        o.set_u8(b, 0x2c, facing);
        o.set_u16(b, 0x50, other as u16);
        o.set_u16(b, 0x54, from.0 as u16);
        o.set_u16(b, Obj::X, c.d[2] as u16);
        o.set_u16(b, Obj::Z, c.d[3] as u16);
        o.set_u16(b, Obj::Y, c.d[4] as u16);
        logique(c, c.d[4] & 0xffff, Size::Word);
    }
    [c.a[1], c.a[2], c.a[3], c.a[4]] = saved;
}

// ---------------------------------------------------------------------------------------------
// Effets de décor (type 42) liés par `+0x46`

/// Effet de décor (type 42) de variante `variant` et de réglage `setting`, lié par `+0x46`.
fn scenery(c: &mut Cpu, b: &mut Cps2Bus, variant: u8, setting: Option<u8>) -> Option<Obj> {
    let o = spawn(c, b, kind::SCENERY)?;
    o.set_u8(b, Obj::VARIANT, variant);
    if let Some(s) = setting {
        o.set_u8(b, 0x2f, s);
    }
    link(c, b, o, 0x46, c.a[0]);
    Some(o)
}

/// Plusieurs effets de décor : D1 compte de `last` à 0 (`dbf`), réglage = D1 ou fixe.
fn scenery_burst(c: &mut Cpu, b: &mut Cps2Bus, variant: u8, last: u16, setting_is_counter: bool) {
    set_low_word(&mut c.d[1], last);
    loop {
        let setting = if setting_is_counter { d1b(c) } else { 0 };
        if scenery(c, b, variant, Some(setting)).is_none() {
            return;
        }
        let n = (c.d[1] as u16).wrapping_sub(1);
        set_low_word(&mut c.d[1], n);
        if n == 0xffff {
            return;
        }
    }
}

/// 0xb3f40 : sept éclats (variante 3, réglages 6 à 0).
fn scenery_3(c: &mut Cpu, b: &mut Cps2Bus) {
    c.d[1] = 6;
    scenery_burst(c, b, 3, 6, true);
}

/// 0xb3f6a : une image sur quatre (`A5+0x6e`), bouffée (variante 4) de réglage tiré au hasard ;
/// `+0x58` = bit 2 du tirage.
fn scenery_4(c: &mut Cpu, b: &mut Cps2Bus) {
    let frame = rd16(b, A5 + 0x6e);
    set_low_word(&mut c.d[0], frame);
    set_low_word(&mut c.d[1], frame);
    if and_d0(c, 3) != 0 {
        return;
    }
    let Some(o) = spawn(c, b, kind::SCENERY) else { return };
    o.set_u8(b, Obj::VARIANT, 4);
    random(c, b);
    let r = and_d0(c, 7);
    o.set_u8(b, 0x2f, r as u8);
    let high = (r & 4) >> 2;
    set_low_word(&mut c.d[0], high);
    o.set_u8(b, 0x58, high as u8);
    link(c, b, o, 0x46, c.a[0]);
}

/// 0xb3fb0 : variante 5, réglage D1.
fn scenery_5(c: &mut Cpu, b: &mut Cps2Bus) {
    let setting = d1b(c);
    scenery(c, b, 5, Some(setting));
}

/// 0xb3fd4 : une fois sur seize (tirage), variante 6.
fn scenery_6(c: &mut Cpu, b: &mut Cps2Bus) {
    random(c, b);
    if and_d0(c, 0xf) != 0 {
        return;
    }
    scenery(c, b, 6, None);
}

/// 0xb4002 : variante 7, réglage nul.
fn scenery_7(c: &mut Cpu, b: &mut Cps2Bus) {
    scenery(c, b, 7, Some(0));
}

/// 0xb4026 : dix effets de variante 8, réglage nul.
fn scenery_8(c: &mut Cpu, b: &mut Cps2Bus) {
    c.d[1] = 9;
    scenery_burst(c, b, 8, 9, false);
}

/// 0xb4088 : une fois sur quatre (tirage), variante 10, réglage nul.
fn scenery_10(c: &mut Cpu, b: &mut Cps2Bus) {
    random(c, b);
    if and_d0(c, 3) != 0 {
        return;
    }
    scenery(c, b, 10, Some(0));
}

// ---------------------------------------------------------------------------------------------
// Sorts d'un ennemi magicien (type 44) liés par `+0x5c`

/// 0xb4f44 : un effet de sort, réglage D1 (variante laissée telle quelle).
fn spell(c: &mut Cpu, b: &mut Cps2Bus) {
    if let Some(o) = spawn(c, b, kind::SPELL) {
        o.set_u8(b, 0x2f, d1b(c));
        link(c, b, o, 0x5c, c.a[0]);
    }
}

/// 0xb4ef4 / 0xb4f60 : six effets de sort (variantes 5 à 0) de réglage `setting`.
fn spell_ring(c: &mut Cpu, b: &mut Cps2Bus, setting: u8) {
    c.d[1] = 5;
    loop {
        let Some(o) = spawn(c, b, kind::SPELL) else { return };
        o.set_u8(b, Obj::VARIANT, d1b(c));
        o.set_u8(b, 0x2f, setting);
        link(c, b, o, 0x5c, c.a[0]);
        let n = (c.d[1] as u16).wrapping_sub(1);
        set_low_word(&mut c.d[1], n);
        if n == 0xffff {
            return;
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Créateurs de scène

/// Poissons (type 30) autour de la caméra alternative : décalages (dx, dy) lus en `table`, mots.
fn fish_at_camera(b: &mut Cps2Bus, o: Obj) {
    let (x, y) = (rd16(b, ALT_CAMERA_X), rd16(b, ALT_CAMERA_Y));
    o.set_u16(b, Obj::X, x);
    o.set_u16(b, Obj::Y, y);
}

/// 0xb1764 : trois poissons (variante 0) qui traversent, 0x20 pixels à gauche de la caméra
/// alternative, à 0x40 de hauteur ; profondeur et vitesse (`+0x58`) lues en 0xb17ae.
fn fish_school(c: &mut Cpu, b: &mut Cps2Bus) {
    c.d[4] = 0;
    c.d[5] = 2;
    loop {
        let Some(o) = spawn(c, b, kind::FISH) else { return };
        o.set_u8(b, 0x2c, 0x20);
        fish_at_camera(b, o);
        o.set_u16(b, Obj::Z, 0xffc0);
        let x = o.u16(b, Obj::X).wrapping_sub(0x20);
        o.set_u16(b, Obj::X, x);
        let k = c.d[4] as u16 as u32;
        let dy = prog16(b, 0xb17ae + k);
        set_low_word(&mut c.d[0], dy);
        let y = o.u16(b, Obj::Y).wrapping_add(dy);
        o.set_u16(b, Obj::Y, y);
        let speed = prog16(b, 0xb17b0 + k);
        put16(c, b, o, 0x58, speed);
        set_low_word(&mut c.d[4], k as u16 + 4);
        if dbf(&mut c.d[5]) {
            return;
        }
    }
}

/// 0xb17ba : quatre poissons (variante 1) autour de la caméra alternative (décalages en 0xb1808),
/// un sur deux retourné.
fn fish_swimmers(c: &mut Cpu, b: &mut Cps2Bus) {
    c.d[4] = 0;
    c.d[5] = 3;
    loop {
        let Some(o) = spawn(c, b, kind::FISH) else { return };
        o.set_u8(b, Obj::VARIANT, 1);
        fish_at_camera(b, o);
        let k = c.d[4] as u16 as u32;
        let (dx, dy) = (prog16(b, 0xb1808 + k), prog16(b, 0xb180a + k));
        set_low_word(&mut c.d[0], dx);
        set_low_word(&mut c.d[1], dy);
        let x = o.u16(b, Obj::X).wrapping_add(dx);
        o.set_u16(b, Obj::X, x);
        let y = add(c, dy as u32, o.u16(b, Obj::Y) as u32, Size::Word);
        o.set_u16(b, Obj::Y, y as u16);
        let odd = c.d[5] as u8 & 1;
        set_low_byte(&mut c.d[2], odd);
        logique(c, odd as u32, Size::Byte);
        if odd != 0 {
            let f = o.u8(b, 0x2c) ^ 0x20;
            put8(c, b, o, 0x2c, f);
        }
        set_low_word(&mut c.d[4], k as u16 + 4);
        if dbf(&mut c.d[5]) {
            return;
        }
    }
}

/// `move.w v,(off,A1)` avec ses drapeaux.
fn put16(c: &mut Cpu, b: &mut Cps2Bus, o: Obj, off: u32, v: u16) {
    o.set_u16(b, off, v);
    logique(c, v as u32, Size::Word);
}

/// `dbf Dn` : décrémente le mot bas ; vrai quand la boucle est finie (passage à -1).
fn dbf(r: &mut u32) -> bool {
    let n = (*r as u16).wrapping_sub(1);
    set_low_word(r, n);
    n == 0xffff
}

/// 0xb7dec : quatre colonnes (type 67, variantes 3 à 0) sous chaque joueur présent et vivant.
fn pillars(c: &mut Cpu, b: &mut Cps2Bus) {
    c.d[6] = 3;
    c.a[2] = PLAYERS;
    loop {
        let p = Obj(c.a[2]);
        if p.u8(b, Obj::ACTIVE) != 0 && p.u16(b, 0x62) != 0 {
            c.d[5] = 3;
            loop {
                let Some(o) = spawn(c, b, kind::PILLAR) else { return };
                o.set_u8(b, Obj::VARIANT, d0b_of(c.d[5]));
                link(c, b, o, 0x50, c.a[2]);
                if dbf(&mut c.d[5]) {
                    break;
                }
            }
        } else {
            logique(c, 0, Size::Byte);
        }
        c.a[2] = c.a[2].wrapping_add(PLAYER_STRIDE);
        if dbf(&mut c.d[6]) {
            return;
        }
    }
}

fn d0b_of(r: u32) -> u8 {
    r as u8
}

/// 0xabb70 : traînées (type 9) de l'entrée de scène. Les deux places de chaque
/// joueur présent sont marquées dans la table `A5-0x5f9a` (6 mots), puis une traînée est créée par
/// place libre restante, `5 - A5+0x35` fois (le pool n'est pas testé, comme l'original).
fn trails(c: &mut Cpu, b: &mut Cps2Bus) {
    const SEATS: u32 = A5 - 0x5f9a;
    for k in 0..3 {
        wr32(b, SEATS + 4 * k, 0);
    }
    c.a[2] = SEATS + 8;
    c.a[1] = PLAYERS;
    c.d[6] = 3;
    loop {
        let p = c.a[1];
        if rd8(b, p) & 1 != 0 {
            let seat = (rd8(b, p + 3) & 0xe) as u32;
            c.d[0] = seat;
            c.a[2] = SEATS;
            wr16(b, SEATS + seat, 0x101);
        }
        c.a[1] = p + PLAYER_STRIDE;
        if dbf(&mut c.d[6]) {
            break;
        }
    }
    c.d[0] = 0;
    let living = rd8(b, A5 + 0x35);
    c.d[5] = 5u8.wrapping_sub(living) as u32;
    set_low_byte(&mut c.d[4], living);
    c.a[4] = SEATS;
    loop {
        call(c, b, ALLOC_EFFECT);
        let o = Obj(c.a[1]);
        o.set_u8(b, Obj::ACTIVE, 1);
        o.set_u8(b, Obj::KIND, kind::TRAIL);
        o.set_u8(b, 0x2c, 0x20);
        o.set_u8(b, 0x2a, 2);
        o.set_u8(b, 0x52, c.d[4] as u8);
        let n = (c.d[4] as u8).wrapping_add(1);
        set_low_byte(&mut c.d[4], n);
        while rd8(b, c.a[4]) != 0 {
            let n = (c.d[0] as u8).wrapping_add(2);
            set_low_byte(&mut c.d[0], n);
            c.a[4] += 2;
        }
        o.set_u8(b, Obj::VARIANT, c.d[0] as u8);
        wr16(b, c.a[4], 0x101);
        logique(c, 0x101, Size::Word);
        if dbf(&mut c.d[5]) {
            return;
        }
    }
}

/// 0xa8a42 : caméra de scène en mode 0x60 (`A5+0x51e`), `A5+0x4ff` effacé.
fn camera_mode_60(c: &mut Cpu, b: &mut Cps2Bus) {
    wr8(b, A5 + 0x4ff, 0);
    wr16(b, A5 + 0x51e, 0x60);
    logique(c, 0x60, Size::Word);
}

/// 0xa8ae8 : `D1 = D0`, puis 0xa8af8.
fn set_list_d0(c: &mut Cpu, b: &mut Cps2Bus) {
    let v = c.d[0] as u16;
    set_low_word(&mut c.d[1], v);
    set_list(c, b);
}

/// 0xa8af8 : crée la liste de décors (SET) numéro D1 (table de pointeurs 0xa8b26) : entrées de
/// 4 octets (type, réglage `+0x2f`, mot dont l'octet bas est la variante), terminées par un type
/// négatif. A4 = fin de liste lue ; s'arrête au premier pool plein.
fn set_list(c: &mut Cpu, b: &mut Cps2Bus) {
    let k = (c.d[1] as u16 & 0xff) << 2;
    set_low_word(&mut c.d[1], k);
    c.a[4] = prog32(b, 0xa8b26 + k as u32);
    loop {
        let kind = rd8(b, c.a[4]);
        c.a[4] += 1;
        set_low_byte(&mut c.d[0], kind);
        logique(c, kind as u32, Size::Byte);
        if (kind as i8) < 0 {
            return;
        }
        call(c, b, ALLOC_SET);
        if c.sr & SR_C != 0 {
            return;
        }
        let o = Obj(c.a[1]);
        o.set_u8(b, Obj::ACTIVE, 1);
        o.set_u8(b, Obj::KIND, c.d[0] as u8);
        let setting = rd8(b, c.a[4]);
        o.set_u8(b, 0x2f, setting);
        let word = rd16(b, c.a[4] + 1);
        c.a[4] += 3;
        set_low_word(&mut c.d[0], word);
        put8(c, b, o, Obj::VARIANT, word as u8);
    }
}

// ---------------------------------------------------------------------------------------------
// Tirs ennemis (ESHL) créés par la famille

/// 0xa836c : coup (ESHL 0x48, variante 0) de réglage D4, lié par `+0x5c`.
fn eshl_slash(c: &mut Cpu, b: &mut Cps2Bus) {
    if let Some(o) = spawn_in(c, b, ALLOC_ESHL, kind::ESHL_SLASH) {
        o.set_u8(b, Obj::VARIANT, 0);
        o.set_u8(b, 0x2f, c.d[4] as u8);
        link(c, b, o, 0x5c, c.a[0]);
    }
}

/// 0xa817c : colonne montante (ESHL 0x47) posée sur le créateur, 0x80 plus haut, juste devant ;
/// rend D0 = 1.
fn eshl_rise(c: &mut Cpu, b: &mut Cps2Bus) {
    let Some(o) = spawn_in(c, b, ALLOC_ESHL, kind::ESHL_RISE) else { return };
    let from = me(c);
    o.set_u8(b, Obj::VARIANT, 0);
    o.set_u8(b, 0x2f, 0);
    o.set_u16(b, 0x46, from.0 as u16);
    let facing = from.u8(b, 0x2c);
    o.set_u8(b, 0x2c, facing);
    copy_position(b, o, from);
    let z = o.u16(b, Obj::Z).wrapping_add(0x80);
    o.set_u16(b, Obj::Z, z);
    let y = o.u16(b, Obj::Y).wrapping_sub(1);
    o.set_u16(b, Obj::Y, y);
    c.d[0] = 1;
    logique(c, 1, Size::Long);
}

const fn creator(entry: u32, name: &'static str, run: fn(&mut Cpu, &mut Cps2Bus), outputs: u32) -> Native {
    Native { entry, name, cost: |_, _| 100, run, touches_sound: false, outputs, exact_cost: false }
}

/// Déclare les natives : chaque créateur se termine par un RTS.
macro_rules! creators {
    ($( $entry:literal $name:literal $f:expr, $out:expr; )*) => {
        pub const NATIVES: &[Native] = &[ $(
            creator($entry, $name, |c, b| { ($f)(c, b); rts(c, b) }, $out),
        )* ];
    };
}

const CREATED: u32 = regs::a(1) | regs::CCR;
use regs::d as D;

creators! {
    0xae776 "cree_interface" interface, CREATED;
    0xaf346 "cree_bandeau" strip, CREATED;
    0xb1a7a "cree_ecran_fin" screen, CREATED;
    0xafe02 "cree_vote" vote, CREATED;
    0xab7d8 "cree_gerbe_debris" debris, CREATED;
    0xabec8 "cree_effet_attaque" attack, CREATED;
    0xb378a "cree_coup" blow, CREATED;
    0xab708 "cree_halo" halo, CREATED;
    0xb1352 "cree_etincelle_posee" posed_spark, CREATED;
    0xb141a "cree_impact" impact, CREATED;
    0xb12c0 "cree_ramassage" pickup, CREATED;
    0xb9c6a "cree_flammeche" |c, b| linked(c, b, kind::WISP, Some(0)), CREATED;
    0xb9c88 "cree_flammeche_b" |c, b| linked(c, b, kind::WISP, Some(1)), CREATED;
    0xb9aa0 "cree_finale" |c, b| linked(c, b, kind::FINALE, None), CREATED;
    0xb84ae "cree_ecran_victoire" |c, b| linked(c, b, kind::VICTORY, None), CREATED;
    0xb85e2 "cree_bulle_createur" |c, b| linked(c, b, kind::BUBBLE, None), CREATED;
    0xb9942 "cree_pluie" |c, b| { spawn(c, b, kind::RAIN); }, CREATED;
    0xb7f14 "cree_type_68" |c, b| { spawn(c, b, kind::TYPE_68); }, CREATED;
    0xb8956 "cree_minuteur" timer, CREATED;
    0xba18a "cree_derive" drift, CREATED;
    0xb66e4 "cree_type_53" type_53, CREATED;
    0xb68b0 "cree_types_54" type_54_trio, CREATED;
    0xae3ec "cree_scintillement" twinkle, CREATED;
    0xb6596 "cree_scintillement_perso" shimmer, CREATED | D(0);
    0xb5730 "cree_repere" beacon, CREATED;
    0xb3c14 "cree_lance" toss, CREATED;
    0xaba16 "cree_eclat_unique" flash, CREATED;
    0xaaa40 "cree_etincelle_en" spark_at, regs::CCR;
    0xb3f40 "cree_decor_3" scenery_3, CREATED | D(1);
    0xb3f6a "cree_decor_4" scenery_4, CREATED | D(0) | D(1);
    0xb3fb0 "cree_decor_5" scenery_5, CREATED;
    0xb3fd4 "cree_decor_6" scenery_6, CREATED | D(0);
    0xb4002 "cree_decor_7" scenery_7, CREATED;
    0xb4026 "cree_decor_8" scenery_8, CREATED | D(1);
    0xb4088 "cree_decor_10" scenery_10, CREATED | D(0);
    0xb4f44 "cree_sort" spell, CREATED;
    0xb4ef4 "cree_sorts" |c, b| spell_ring(c, b, 0), CREATED | D(1);
    0xb4f60 "cree_sorts_b" |c, b| spell_ring(c, b, 2), CREATED | D(1);
    0xb1764 "cree_banc" fish_school, CREATED | D(0) | D(4) | D(5);
    0xb17ba "cree_nageurs" fish_swimmers, CREATED | D(0) | D(1) | D(2) | D(4) | D(5);
    0xb7dec "cree_colonnes" pillars, CREATED | D(5) | D(6) | regs::a(2);
    0xabb70 "cree_trainees" trails, CREATED | D(0) | D(4) | D(5) | D(6) | regs::a(2) | regs::a(4);
    0xa8a42 "camera_mode_60" camera_mode_60, regs::CCR;
    0xa8ae8 "cree_liste_decors_d0" set_list_d0, CREATED | D(0) | D(1) | regs::a(4);
    0xa8af8 "cree_liste_decors" set_list, CREATED | D(0) | D(1) | regs::a(4);
    0xa836c "cree_tir_coup" eshl_slash, CREATED;
    0xa817c "cree_tir_colonne" eshl_rise, CREATED | D(0);
}
