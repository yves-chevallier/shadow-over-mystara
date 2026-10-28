//! Petites routines qui restaient en 68000 et que le Rust appelait encore : créateurs d'objets (tirs,
//! effets, ennemis, objets système), copies de palettes, quelques variables du jeu.
//!
//! Contrat des créateurs : A0 = le créateur ; au retour A1 = l'objet créé, retenue levée si le pool est
//! plein. Les registres résiduels que l'original laisse (D0, D1, D2, D5, drapeaux) sont reproduits.

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::Cpu;

use crate::mem::{prog16, prog8, rd16, rd32, rts, set_low_word, wr16, wr32, wr8, A5};
use crate::objects::Obj;
use super::cx::{handler, Cx};
use crate::{call, regs, Native};
use crate::objects::pools;
use super::effects_creators::restes::{Pur, Registres};


/// Alloue dans un pool ; `None` (retenue levée) si le pool est plein.
fn alloc(c: &mut Cpu, b: &mut Cps2Bus, pool: &pools::Pool) -> Option<Obj> {
    pool.alloc_68k(c, b)
}

/// Drapeaux d'un `move`/`clr` de `v` : N et Z selon la valeur, V et C à zéro.
fn moved(c: &mut Cpu, v: u32, size: Size) {
    c.flags_logic(v, size);
}

/// Mot d'adresse d'un objet (`move.w An,...`).
fn word(c: &Cpu) -> u16 {
    c.a[0] as u16
}

/// Crée un objet de `pool` de type `kind`, lié au créateur par le mot `link` (drapeaux de ce `move.w`).
pub(crate) fn linked_par(r: &mut impl Registres, b: &mut Cps2Bus, createur: Obj, pool: &pools::Pool, kind: u8, link: u32) -> Option<Obj> {
    let o = r.alloue(b, pool)?;
    o.set_u8(b, Obj::ACTIVE, 1);
    o.set_u8(b, Obj::KIND, kind);
    o.set_u16(b, link, createur.word());
    r.drapeaux(createur.word() as u32, Size::Word);
    Some(o)
}

fn linked(c: &mut Cpu, b: &mut Cps2Bus, pool: &pools::Pool, kind: u8, link: u32) {
    let me = Obj(c.a[0]);
    linked_par(c, b, me, pool, kind, link);
    rts(c, b);
}

/// 0x4f132 : effet de joueur 0x21 (second temps du séisme des sorts), lié au joueur par `+0x50`.
pub fn quake_b(b: &mut Cps2Bus, joueur: Obj) -> Option<Obj> {
    linked_par(&mut Pur, b, joueur, &pools::PEFF, 0x21, 0x50)
}

/// `quake_b` avec les registres de l'original (appelants par adresse : `player/`) : A1, retenue et drapeaux.
#[track_caller]
pub fn quake_b_68k(c: &mut Cpu, b: &mut Cps2Bus, joueur: Obj) -> Option<Obj> {
    c.a[0] = joueur.0;
    let r = linked_par(c, b, joueur, &pools::PEFF, 0x21, 0x50);
    crate::diag::registres_morts(c, "effets", CREATED);
    r
}

fn peff_16(c: &mut Cpu, b: &mut Cps2Bus) {
    linked(c, b, &pools::PEFF, 0x10, 0x50);
}
fn peff_33(c: &mut Cpu, b: &mut Cps2Bus) {
    linked(c, b, &pools::PEFF, 0x21, 0x50);
}
fn sobj_7(c: &mut Cpu, b: &mut Cps2Bus) {
    linked(c, b, &pools::SOBJ, 0x7, 0x46);
}
fn sobj_10(c: &mut Cpu, b: &mut Cps2Bus) {
    linked(c, b, &pools::SOBJ, 0xa, 0x46);
}

/// 0x54700 : tir de type 0x17 qui reprend le mot `+0x50` du créateur.
fn shot_17(c: &mut Cpu, b: &mut Cps2Bus) {
    if let Some(o) = alloc(c, b, &pools::PSHL) {
        let me = Obj(c.a[0]);
        o.set_u8(b, 0x0, 1);
        o.set_u8(b, 0x2, 0x17);
        let v = me.u16(b, 0x50);
        o.set_u16(b, 0x50, v);
        moved(c, v as u32, Size::Word);
    }
    rts(c, b);
}

/// Tir de type 0x18 lié au créateur par `+0x5c`, résistances 0x10, variante `variant` ; D0 = 1 si
/// créé. `always_zero` : 0x54872 rend D0 = 0 même quand le tir est créé (bizarrerie de l'original).
fn shot_18(c: &mut Cpu, b: &mut Cps2Bus, variant: u8, always_zero: bool) {
    let me = word(c);
    let created = alloc(c, b, &pools::PSHL);
    if let Some(shot) = created {
        shot.set_u8(b, 0x0, 1);
        shot.set_u8(b, 0x2, 0x18);
        if variant != 0 {
            shot.set_u8(b, 0x3, variant);
        }
        shot.set_u8(b, 0x45, 0x10);
        shot.set_u8(b, 0x46, 0x10);
        shot.set_u16(b, 0x5c, me);
    }
    c.d[0] = (created.is_some() && !always_zero) as u32;
    moved(c, c.d[0], Size::Long);
    rts(c, b);
}
fn shot_18_a(c: &mut Cpu, b: &mut Cps2Bus) {
    shot_18(c, b, 0, false);
}
fn shot_18_c(c: &mut Cpu, b: &mut Cps2Bus) {
    shot_18(c, b, 2, true);
}

/// 0x76ec4 : s'il reste plus de deux emplacements ENEMY, crée deux ennemis de type 0x19 (variantes 0 et
/// 1) liés au créateur ; D0 = 1 si créés, 0 sinon. L'original ne teste pas la retenue des allocations.
fn enemy_pair(c: &mut Cpu, b: &mut Cps2Bus) {
    if (rd16(b, A5 + 0x7e62) as i16) <= 2 {
        c.d[0] = 0;
        moved(c, 0, Size::Long);
        return rts(c, b);
    }
    for variant in 0..2 {
        // Glitch d'origine : l'échec de l'allocation n'est pas testé (voir `Pool::alloc_unchecked`).
        let o = pools::ENEMY.alloc_unchecked_68k(c, b);
        o.set_u8(b, 0x0, 1);
        o.set_u8(b, 0x2, 0x19);
        o.set_u8(b, 0x3, variant);
        o.set_u8(b, 0x2f, 0);
        o.set_u16(b, 0x46, word(c));
    }
    c.d[0] = 1;
    moved(c, 1, Size::Long);
    rts(c, b);
}

/// 0x792f8 et 0x71e3c : `+0x53` reprend `+0x56`.
fn copy_56_to_53(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = Obj(c.a[0]);
    let v = me.u8(b, 0x56);
    me.set_u8(b, 0x53, v);
    moved(c, v as u32, Size::Byte);
    rts(c, b);
}

/// 0xa66c0 : crée D5 + 1 tirs ESHL de type 0x3e dont la variante décroît de D5 (octet de la table 0xa66e2
/// indexée par D2) à 0.
fn eshl_volley(c: &mut Cpu, b: &mut Cps2Bus) {
    let row = 0xa66e2u32.wrapping_add(c.d[2] as i16 as i32 as u32);
    c.d[5] = prog8(b, row) as u32;
    loop {
        let Some(o) = alloc(c, b, &pools::ESHL) else { break };
        o.set_u8(b, 0x0, 1);
        o.set_u8(b, 0x2, 0x3e);
        o.set_u8(b, 0x3, c.d[5] as u8);
        moved(c, c.d[5], Size::Byte);
        let n = (c.d[5] as u16).wrapping_sub(1);
        set_low_word(&mut c.d[5], n);
        if n == 0xffff {
            break;
        }
    }
    rts(c, b);
}

/// 0xa7234 et 0xa727c : gonflement du rugissement de Boss 15 : un nombre de tirs ESHL de type 0x41 (variante
/// `variant`) donné par la table (selon `A5+0x7f92`), numérotés de n-1 à 0 par `+0x2f`, liés par `+0x5c`.
fn roar(c: &mut Cpu, b: &mut Cps2Bus, table: u32, variant: u8) {
    let i = rd16(b, A5 + 0x7f92) >> 1;
    set_low_word(&mut c.d[0], i);
    c.d[1] = prog8(b, table + i as u32) as u32;
    let mut n = (c.d[1] as u16).wrapping_sub(1);
    set_low_word(&mut c.d[1], n);
    loop {
        let Some(o) = alloc(c, b, &pools::ESHL) else { break };
        let me = Obj(c.a[0]);
        o.set_u8(b, 0x0, 1);
        o.set_u8(b, 0x2, 0x41);
        o.set_u8(b, 0x3, variant);
        o.set_u8(b, 0x2f, c.d[1] as u8);
        let link = me.u16(b, 0x5c);
        o.set_u16(b, 0x5c, link);
        moved(c, link as u32, Size::Word);
        n = (c.d[1] as u16).wrapping_sub(1);
        set_low_word(&mut c.d[1], n);
        if n == 0xffff {
            break;
        }
    }
    rts(c, b);
}
fn roar_big(c: &mut Cpu, b: &mut Cps2Bus) {
    roar(c, b, 0xa726c, 0);
}
fn roar_small(c: &mut Cpu, b: &mut Cps2Bus) {
    roar(c, b, 0xa72b4, 1);
}

/// 0xa036e : harpie, créature compagne (ESHL 0x10) : copie la variante, la position décalée de ±0x2f en X
/// et de -0x78 en Z ; la moitié des PV du créateur (`+0x60 / 4`) va dans `+0x58` du créateur.
fn harpy_companion(c: &mut Cpu, b: &mut Cps2Bus) {
    let Some(o) = alloc(c, b, &pools::ESHL) else { return rts(c, b) };
    let me = Obj(c.a[0]);
    let flip = me.u8(b, 0x2c) != 0;
    let v = me.u8(b, 0x3);
    o.set_u8(b, 0x3, v);
    o.set_u8(b, 0x0, 1);
    o.set_u8(b, 0x2, 0x10);
    o.set_u8(b, 0x2e, 0x3e);
    o.set_u16(b, 0x50, word(c));
    let v = me.u8(b, 0x43);
    o.set_u8(b, 0x43, v);
    let hp = me.u16(b, 0x60) >> 2;
    set_low_word(&mut c.d[2], hp);
    me.set_u16(b, 0x58, hp);
    let v = me.u8(b, 0xa2);
    o.set_u8(b, 0x56, v);
    for off in [0x8, 0x10, 0xc] {
        let v = me.u32(b, off);
        o.set_u32(b, off, v);
    }
    let dx: u16 = if flip { 0x2fu16.wrapping_neg() } else { 0x2f };
    set_low_word(&mut c.d[0], dx);
    set_low_word(&mut c.d[1], 0x78u16.wrapping_neg());
    let x = o.u16(b, 0x8).wrapping_add(dx);
    o.set_u16(b, 0x8, x);
    let z = o.u16(b, 0xc).wrapping_add(0x78u16.wrapping_neg());
    o.set_u16(b, 0xc, z);
    let y = o.u32(b, 0x10).wrapping_add(1);
    o.set_u32(b, 0x10, y);
    let z = o.u32(b, 0xc).wrapping_sub(1);
    o.set_u32(b, 0xc, z);
    let f = me.u8(b, 0x2c);
    o.set_u8(b, 0x2c, f);
    moved(c, f as u32, Size::Byte);
    rts(c, b);
}

/// 0xa0566 : harpie, projectile ESHL 0x11 : D5 va dans `+0x42` du créateur, A2 et le créateur sont les
/// liens `+0x50` et `+0x56` (longs) ; le créateur remet `+0xa5` à zéro.
fn harpy_shot(c: &mut Cpu, b: &mut Cps2Bus) {
    let Some(o) = alloc(c, b, &pools::ESHL) else { return rts(c, b) };
    let me = Obj(c.a[0]);
    me.set_u8(b, 0x42, c.d[5] as u8);
    let v = me.u8(b, 0x43);
    o.set_u8(b, 0x43, v);
    o.set_u32(b, 0x50, c.a[2]);
    o.set_u32(b, 0x56, c.a[0]);
    me.set_u8(b, 0xa5, 0);
    o.set_u8(b, 0x0, 1);
    o.set_u8(b, 0x2, 0x11);
    for off in [0x8, 0x10, 0xc] {
        let v = me.u32(b, off);
        o.set_u32(b, off, v);
    }
    let v = me.u8(b, 0x2c);
    o.set_u8(b, 0x2c, v);
    let v = me.u8(b, 0x3);
    o.set_u8(b, 0x3, v);
    moved(c, v as u32, Size::Byte);
    rts(c, b);
}

// ---------------------------------------------------------------------------------------------
// Variables du jeu, copies de palettes, enchaînements d'initialisations
// ---------------------------------------------------------------------------------------------

/// `movem.l (addr)+,liste` : charge les registres du masque (D0-D7 puis A0-A7) ; renvoie l'adresse suivante.
fn movem_load(c: &mut Cpu, b: &mut Cps2Bus, mask: u16, mut addr: u32) -> u32 {
    for bit in (0..16).filter(|bit| mask >> bit & 1 != 0) {
        let v = rd32(b, addr);
        if bit < 8 {
            c.d[bit] = v;
        } else {
            c.a[bit - 8] = v;
        }
        addr += 4;
    }
    addr
}

/// `movem.l liste,(addr)` : range les registres du masque ; renvoie l'adresse suivante.
fn movem_store(c: &Cpu, b: &mut Cps2Bus, mask: u16, mut addr: u32) -> u32 {
    for bit in (0..16).filter(|bit| mask >> bit & 1 != 0) {
        wr32(b, addr, if bit < 8 { c.d[bit] } else { c.a[bit - 8] });
        addr += 4;
    }
    addr
}

/// D0-D6 et A3.
const MASK_D0_D6_A3: u16 = 0x087f;

/// 0x10158 : quatre blocs de huit couleurs recopiés dans la palette (GFX RAM) avec le bit de poids fort
/// de chaque quartet levé (`ori.l #0xF000F000`).
fn palette_blocks_lit(c: &mut Cpu, b: &mut Cps2Bus) {
    for (dst, src) in [(0x91c2e0, 0x2538c0), (0x91c300, 0x2538e0), (0x91c360, 0x253900), (0x91c3e0, 0x253920)] {
        movem_load(c, b, MASK_D0_D6_A3, src);
        c.a[2] = src;
        movem_store(c, b, MASK_D0_D6_A3, dst);
        for k in 0..8 {
            let at = dst + 4 * k;
            let v = rd32(b, at) | 0xf000_f000;
            wr32(b, at, v);
            moved(c, v, Size::Long);
        }
        c.a[1] = dst + 0x20;
        c.d[6] = 0xffff;
    }
    rts(c, b);
}

/// 0x101da : comme 0x10158 pour la scène 0x69, sans éclaircir : quatre blocs de huit couleurs lus à la suite
/// en 0x10218 (espace données).
fn palette_blocks(c: &mut Cpu, b: &mut Cps2Bus) {
    let mut src = 0x10218;
    for dst in [0x91c2e0, 0x91c300, 0x91c360, 0x91c3e0] {
        src = movem_load(c, b, MASK_D0_D6_A3, src);
        c.a[1] = dst;
        movem_store(c, b, MASK_D0_D6_A3, dst);
    }
    c.a[2] = src;
    rts(c, b);
}

/// 0x4aed0 : deux effets de type 5, variante 1, indices `+0x2f` 0 et 1, liés au créateur (`+0x50`).
fn peff_5_pair(c: &mut Cpu, b: &mut Cps2Bus) {
    for index in [0, 1] {
        let Some(o) = alloc(c, b, &pools::PEFF) else { break };
        o.set_u8(b, 0x0, 1);
        o.set_u8(b, 0x2, 5);
        o.set_u8(b, 0x3, 1);
        o.set_u8(b, 0x2f, index);
        let me = word(c);
        o.set_u16(b, 0x50, me);
        moved(c, me as u32, Size::Word);
    }
    rts(c, b);
}

/// 0x53746 : douze pierres (tirs de type 0x11, variantes 0 à 11) réparties autour de la caméra en service,
/// décalages X, Y en table (0x537ca, espace programme), hauteur 0x100 au-dessus de leur point de chute, délai
/// `+0x5c` de 0x40 à 0x148 par pas de 0x18 ; orientation et propriétaire du créateur.
fn pshl_stones(c: &mut Cpu, b: &mut Cps2Bus) {
    const OFFSETS: u32 = 0x537ca;
    const COUNT: u16 = 12;
    let me = Obj(c.a[0]);
    c.d[3] = 0;
    set_low_word(&mut c.d[4], 0x40);
    set_low_word(&mut c.d[5], COUNT - 1);
    loop {
        let Some(o) = alloc(c, b, &pools::PSHL) else { break };
        o.set_u8(b, 0x0, 1);
        o.set_u8(b, 0x2, 0x11);
        let v = me.u8(b, 0x2c);
        o.set_u8(b, 0x2c, v);
        let v = me.u16(b, 0x50);
        o.set_u16(b, 0x50, v);
        o.set_u8(b, 0x3, c.d[3] as u8);
        o.set_u16(b, 0x5c, c.d[4] as u16);
        let (cx, cy) = if crate::ram::CAMERA_ALT_ACTIVE.get(b) != 0 { (0x556, 0x55a) } else { (0x506, 0x50a) };
        let (x, y) = (rd16(b, A5 + cx), rd16(b, A5 + cy));
        let index = (c.d[3] as u16).wrapping_shl(2) as u32;
        let (dx, dy) = (prog16(b, OFFSETS + index), prog16(b, OFFSETS + 2 + index));
        let ground = y.wrapping_add(dy);
        o.set_u16(b, 0x8, x.wrapping_add(dx));
        o.set_u16(b, 0x10, ground);
        o.set_u16(b, 0xc, y.wrapping_add(0x100).wrapping_sub(ground));
        set_low_word(&mut c.d[0], ground);
        set_low_word(&mut c.d[1], dx);
        set_low_word(&mut c.d[2], dy);
        let variant = (c.d[3] as u8).wrapping_add(1);
        crate::mem::set_low_byte(&mut c.d[3], variant);
        let d4 = c.d[4];
        let delay = crate::ops::add(c, 0x18, d4, Size::Word);
        set_low_word(&mut c.d[4], delay as u16);
        let n = (c.d[5] as u16).wrapping_sub(1);
        set_low_word(&mut c.d[5], n);
        if n == 0xffff {
            break;
        }
    }
    rts(c, b);
}

/// 0x1a5e8 : douze blocs de huit couleurs recopiés dans la palette 0x91c480.
fn palette_blocks_copy(c: &mut Cpu, b: &mut Cps2Bus) {
    const MASK: u16 = 0x067e; // D1-D6, A1, A2
    let (mut dst, mut src) = (0x91c480, 0x25c4c0);
    // `moveq #0xb,D7` : seuls drapeaux posés (ni `movem` ni `dbf` n'en changent)
    moved(c, 0xb, Size::Long);
    for _ in 0..12 {
        src = movem_load(c, b, MASK, src);
        dst = movem_store(c, b, MASK, dst);
    }
    c.a[3] = src;
    c.a[4] = dst;
    c.d[7] = 0xffff;
    rts(c, b);
}

/// 0x1ad6a : 32 blocs de 32 octets lus à partir de 0x285280 sont rangés tour à tour au même endroit,
/// FF1190 (l'original n'avance pas sa destination : seul le dernier bloc y reste), puis huit couleurs de
/// 0x1ad4a vont en FF1390.
pub(crate) fn palette_page_restore(c: &mut Cpu, b: &mut Cps2Bus) {
    const MASK: u16 = 0x183f; // D0-D5, A3, A4
    let mut src = 0x285280;
    for _ in 0..32 {
        src = movem_load(c, b, MASK, src);
        movem_store(c, b, MASK, 0xff1190);
    }
    c.a[2] = src;
    c.d[6] = 0xffff;
    c.a[2] = movem_load(c, b, MASK_D0_D6_A3, 0x1ad4a);
    c.a[1] = 0xff1390;
    movem_store(c, b, MASK_D0_D6_A3, 0xff1390);
}

/// Éclair de palette : couleur 14 de la page 0x91c400 (`0x91c41c`) fixée à `v` ; drapeaux du `move.w`.
pub(crate) fn flash_68k(c: &mut Cpu, b: &mut Cps2Bus, v: u16) {
    wr16(b, 0x91c41c, v);
    moved(c, v as u32, Size::Word);
}
fn flash(c: &mut Cpu, b: &mut Cps2Bus, v: u16) {
    flash_68k(c, b, v);
    rts(c, b);
}
/// Couleurs des éclairs (`flash_68k`) : bleu (0x1ac7c).
pub(crate) const ECLAIR_BLEU: u16 = 0xf00f;
/// 0x1ac90 : fin de l'éclair (couleur 0xff50).
fn flash_off(c: &mut Cpu, b: &mut Cps2Bus) {
    flash(c, b, 0xff50);
}
fn flash_a(c: &mut Cpu, b: &mut Cps2Bus) {
    flash(c, b, 0xfff0);
}
fn flash_b(c: &mut Cpu, b: &mut Cps2Bus) {
    flash(c, b, 0xf0f0);
}
fn flash_c(c: &mut Cpu, b: &mut Cps2Bus) {
    flash(c, b, 0xf00f);
}

/// Les huit blocs de tuiles du fond 2 (0x26d4 : D0 = bloc, A1 = destination).
const BACKGROUND_BLOCKS: [(u16, u32); 8] = [
    (0xce, 0x917000),
    (0xad, 0x917400),
    (0xaf, 0x917800),
    (0xaf, 0x917c00),
    (0xac, 0x916000),
    (0xcd, 0x916400),
    (0xae, 0x916800),
    (0xae, 0x916c00),
];

/// 0x2a52 : fond 2 allumé (blocs de tuiles, bit 0 du registre vidéo `A5+0x12`, caméra 2 en mode 0x80).
fn scroll_2_on(c: &mut Cpu, b: &mut Cps2Bus) {
    for (block, dst) in BACKGROUND_BLOCKS {
        set_low_word(&mut c.d[0], block);
        c.a[1] = dst;
        call(c, b, 0x26d4);
    }
    let video = rd16(b, A5 + 0x12) | 1;
    wr16(b, A5 + 0x12, video);
    wr16(b, 0x804122, video);
    wr16(b, 0x804108, 0x9200);
    wr8(b, A5 + 0x53c, 2);
    wr16(b, A5 + 0x51e, 0x80);
    wr8(b, A5 + 0x36, 1);
    moved(c, 1, Size::Byte);
    rts(c, b);
}

/// 0x2a42 : `A5+0x548` = D0, fond 1 réglé sur 0x34 ; 0x2a36 : fond 1 réglé sur 0x18.
fn scroll_1_on_with(c: &mut Cpu, b: &mut Cps2Bus, mode: u16) {
    wr8(b, A5 + 0x4ff, 0);
    wr16(b, A5 + 0x51e, mode);
    moved(c, mode as u32, Size::Word);
    rts(c, b);
}
fn scroll_1_on(c: &mut Cpu, b: &mut Cps2Bus) {
    scroll_1_on_with(c, b, 0x18);
}
fn scroll_1_on_at_d0(c: &mut Cpu, b: &mut Cps2Bus) {
    wr16(b, A5 + 0x548, c.d[0] as u16);
    scroll_1_on_with(c, b, 0x34);
}

/// 0x133f6 / 0x133f0 : l'indicateur « démarrage fait » (`A5-0x715e`) levé / baissé.
pub(crate) fn booted_set(c: &mut Cpu, b: &mut Cps2Bus) {
    wr8(b, A5 - 0x715e, 1);
    moved(c, 1, Size::Byte);
    rts(c, b);
}
fn booted_clear(c: &mut Cpu, b: &mut Cps2Bus) {
    wr8(b, A5 - 0x715e, 0);
    moved(c, 0, Size::Byte);
    rts(c, b);
}

/// 0xdd12c : le verrou d'apparition des objets de ramassage (`A5-0x5ac4`) levé.
fn item_lock_clear(c: &mut Cpu, b: &mut Cps2Bus) {
    wr8(b, A5 - 0x5ac4, 0);
    moved(c, 0, Size::Byte);
    rts(c, b);
}

/// 0x3baea : `+0x1f0` du joueur, d'après la table 0x3baf8 indexée par le quart du compteur `A5+0x7f92`.
fn player_table_byte(c: &mut Cpu, b: &mut Cps2Bus) {
    let i = rd16(b, A5 + 0x7f92) >> 2;
    set_low_word(&mut c.d[0], i);
    let v = prog8(b, 0x3baf8 + i as u32);
    wr8(b, c.a[0] + 0x1f0, v);
    moved(c, v as u32, Size::Byte);
    rts(c, b);
}

/// 0x1c44a : les variables de partie `A5+0x2c..0x6b` à zéro.
fn game_vars_clear(c: &mut Cpu, b: &mut Cps2Bus) {
    c.d[0] = 0;
    for k in 0..16 {
        wr32(b, A5 + 0x2c + 4 * k, 0);
    }
    c.a[0] = A5 + 0x6c;
    c.d[6] = 0xffff;
    moved(c, 0, Size::Long);
    rts(c, b);
}

/// 0x180ac : remise à zéro de l'écran fixe : pools SET et EFFCT, sprites, liste triée.
fn fixed_screen_reset(c: &mut Cpu, b: &mut Cps2Bus) {
    wr32(b, A5 - 0x70b0, 0);
    for routine in [0x1f184, 0x1de7c] {
        call(c, b, routine);
    }
    pools::SET.reset(b);
    pools::EFFCT.reset(b);
    rts(c, b);
}

// ---------------------------------------------------------------------------------------------
// Réaction aux coups de deux ennemis (table des réactions communes, `react`)
// ---------------------------------------------------------------------------------------------


/// 0x78ad2 et 0x71680 : l'ennemi touché tremble sur place. Pas 0 : mémorise X (`+0xa`), lance l'animation 8
/// du jeu `anim` et pose le compteur (`+0x90` = 16). Pas 2 : décale X de ±2 selon la parité du compteur et
/// l'orientation, puis, le compteur épuisé, revient à X d'origine et à l'état 0x04040200.
/// `table` : table de sauts d'origine, dont le mot lu reste dans D1.
fn shake(x: &mut Cx, table: u32, anim: u32) {
    let step = x.u8(0x6);
    x.c.d[0] = step as u32;
    let word = x.rom16(table + step as u32);
    x.c.d[1] = x.c.d[1] & 0xffff_0000 | word as u32;
    match step {
        0 => {
            x.add8(0x6, 2);
            x.set8(0x0, 2);
            let at = x.u16(0x8);
            x.set16(0xa, at);
            x.set16(0x84, 0);
            let v = x.u8(0x56);
            x.set8(0x53, v);
            x.set8(0x90, 0x10);
            crate::families::enemy_lib::start(x, anim, 8);
        }
        2 => {
            let timer = x.u8(0x90);
            if (timer as i8) <= 0 {
                let at = x.u16(0xa);
                x.set16(0x8, at);
                x.set8(0x0, 1);
                x.set32(0x4, 0x0404_0200);
                x.set32(0x58, 0);
                return;
            }
            let timer = timer - 1;
            x.set8(0x90, timer);
            let mut d: i16 = 2;
            if timer & 1 != 0 {
                d = -d;
            }
            if x.u8(0x2c) != 0 {
                d = -d;
            }
            x.c.d[0] = d as u16 as u32;
            x.add16(0x8, d as u16);
        }
        _ => {}
    }
}
fn shake_a(x: &mut Cx) {
    shake(x, 0x78ae0, 0x171a94);
}
fn shake_b(x: &mut Cx) {
    shake(x, 0x7168e, 0x1778a4);
}

const fn native(entry: u32, name: &'static str, run: fn(&mut Cpu, &mut Cps2Bus), outputs: u32) -> Native {
    Native { entry, name, run, touches_sound: false, outputs }
}

const CREATED: u32 = regs::a(1) | regs::CCR;

pub const NATIVES: &[Native] = &[
    native(0x4c930, "cree_effet_10", peff_16, CREATED),
    native(0x4f132, "cree_effet_21", peff_33, CREATED),
    native(0xdf4bc, "cree_sobj_7", sobj_7, CREATED),
    native(0xdf896, "cree_sobj_10", sobj_10, CREATED),
    native(0x54700, "cree_tir_17", shot_17, CREATED),
    native(0x54818, "cree_tir_18_a", shot_18_a, CREATED | regs::d(0)),
    native(0x54872, "cree_tir_18_c", shot_18_c, CREATED | regs::d(0)),
    native(0x76ec4, "cree_ennemis_19", enemy_pair, CREATED | regs::d(0)),
    native(0x792f8, "copie_56_53_a", copy_56_to_53, regs::CCR),
    native(0x71e3c, "copie_56_53_b", copy_56_to_53, regs::CCR),
    native(0xa66c0, "cree_salve_3e", eshl_volley, CREATED | regs::d(5)),
    native(0xa7234, "cree_rugissement_grand", roar_big, CREATED | regs::d(0) | regs::d(1)),
    native(0xa727c, "cree_rugissement_petit", roar_small, CREATED | regs::d(0) | regs::d(1)),
    native(0xa036e, "cree_compagnon_harpie", harpy_companion, CREATED | regs::d(0) | regs::d(1) | regs::d(2)),
    native(0xa0566, "cree_tir_harpie", harpy_shot, CREATED),
    native(0x10158, "palette_quatre_blocs", palette_blocks_lit, regs::ALL),
    native(0x101da, "palette_quatre_blocs_69", palette_blocks, regs::ALL),
    native(0x1ac90, "eclair_palette_fin", flash_off, regs::CCR),
    native(0x4aed0, "cree_effets_5", peff_5_pair, CREATED),
    native(0x53746, "cree_pierres_11", pshl_stones, CREATED | regs::ALL),
    native(0x1a5e8, "palette_douze_blocs", palette_blocks_copy, regs::ALL),
    native(0x1ad6a, "palette_page_restitue", |c, b| { palette_page_restore(c, b); rts(c, b) }, regs::ALL),
    native(0x1ac9a, "eclair_palette_jaune", flash_a, regs::CCR),
    native(0x1ac72, "eclair_palette_vert", flash_b, regs::CCR),
    native(0x1ac7c, "eclair_palette_bleu", flash_c, regs::CCR),
    native(0x2a52, "fond_2_allume", scroll_2_on, regs::d(0) | regs::a(1) | regs::CCR),
    native(0x2a36, "fond_1_allume", scroll_1_on, regs::CCR),
    native(0x2a42, "fond_1_allume_d0", scroll_1_on_at_d0, regs::CCR),
    native(0x133f6, "demarrage_leve", booted_set, regs::CCR),
    native(0x133f0, "demarrage_baisse", booted_clear, regs::CCR),
    native(0xdd12c, "verrou_item_baisse", item_lock_clear, regs::CCR),
    native(0x3baea, "octet_table_joueur", player_table_byte, regs::d(0)),
    native(0x1c44a, "variables_partie_a_zero", game_vars_clear, regs::ALL),
    native(0x180ac, "ecran_fixe_remise", fixed_screen_reset, regs::CCR),
];

/// Réactions d'ennemis : mêmes registres que `handler` (A0, D7) et ceux qu'elles modifient.
pub const SHAKES: &[Native] = &[
    Native { outputs: regs::a(0) | regs::d(7) | regs::d(0) | regs::d(1) | regs::a(4), ..handler(0x78ad2, "ennemi_tremble_a", |c, b| run_shake(c, b, shake_a)) },
    Native { outputs: regs::a(0) | regs::d(7) | regs::d(0) | regs::d(1) | regs::a(4), ..handler(0x71680, "ennemi_tremble_b", |c, b| run_shake(c, b, shake_b)) },
];

fn run_shake(c: &mut Cpu, b: &mut Cps2Bus, f: fn(&mut Cx)) {
    let mut x = Cx::new(c, b);
    f(&mut x);
    x.ret();
}
