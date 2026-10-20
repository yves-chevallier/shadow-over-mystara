//! Palettes en GFX RAM : luminosité, chargement depuis la bibliothèque de la ROM, fondus.
//!
//! Une couleur matérielle est 0xLRGB (L = luminosité, un quartet par composante). Les quatre pages
//! de palettes (`0x91c000 + 0x400 k`, 32 palettes de 16 couleurs) sont désignées par les bits 0 à 3 de
//! `PAGES`. Voir `docs/video.md`.

use cps2::Cps2Bus;
use m68k::Cpu;

use crate::chemin::Chemin;
use crate::mem::{rd16, rd32, rd8, rts, wr16, wr32};
use crate::video::{hw, tours};
use crate::vue::Vue;
use crate::{regs, Native};

/// Pages de palettes concernées (bit k : page 0x91c000 + 0x400 k).
const PAGES: u32 = 0x48e;
const PALETTES: u32 = 0x91c000;
const PAGE_SIZE: u32 = 0x400;

fn for_each_color(c: &Cpu, b: &mut Cps2Bus, mut f: impl FnMut(u16) -> u16) {
    let pages = rd8(b, c.a[5] + PAGES);
    for k in 0..4 {
        if pages & (1 << k) != 0 {
            let page = PALETTES + PAGE_SIZE * k;
            for i in 0..PAGE_SIZE / 2 {
                let a = page + 2 * i;
                let v = f(rd16(b, a));
                wr16(b, a, v);
            }
        }
    }
}

/// 0x168e : luminosité D0 ajoutée (OU) aux couleurs, ou effacée si D0 = 0.
fn or_brightness(c: &mut Cpu, b: &mut Cps2Bus) {
    let level = c.d[0] as u16;
    for_each_color(c, b, |col| if level == 0 { col & 0x0fff } else { col | level });
    rts(c, b);
}

/// 0x1686 : luminosité maximale.
fn full_brightness(c: &mut Cpu, b: &mut Cps2Bus) {
    c.d[0] = (c.d[0] & 0xffff_0000) | 0xf000;
    or_brightness(c, b);
}

/// 0x168c : luminosité minimale.
fn no_brightness(c: &mut Cpu, b: &mut Cps2Bus) {
    c.d[0] = 0;
    or_brightness(c, b);
}

/// 0x173c : luminosité remplacée par D0.
fn set_brightness(c: &mut Cpu, b: &mut Cps2Bus) {
    let level = c.d[0] as u16;
    for_each_color(c, b, |col| (col & 0x0fff) | level);
    rts(c, b);
}

/// ~ 8 cycles par couleur et par page active.
fn cost(c: &Cpu, b: &mut Cps2Bus) -> u64 {
    100 + 8 * 512 * rd8(b, c.a[5] + PAGES).count_ones().min(4) as u64
}

const fn native(entry: u32, name: &'static str, run: fn(&mut Cpu, &mut Cps2Bus)) -> Native {
    Native { entry, name, cost, run, touches_sound: false, outputs: 0, exact_cost: false }
}

pub const FULL_BRIGHTNESS: Native = native(0x1686, "palette_full_brightness", full_brightness);
pub const NO_BRIGHTNESS: Native = native(0x168c, "palette_no_brightness", no_brightness);
pub const OR_BRIGHTNESS: Native = native(0x168e, "palette_or_brightness", or_brightness);
pub const SET_BRIGHTNESS: Native = native(0x173c, "palette_set_brightness", set_brightness);

// ---------------------------------------------------------------------------------------------
// Fondus de luminosité (V3) : un pas par appel, répété par les lanceurs (0x146c, 0x14de, 0x1544)
// ---------------------------------------------------------------------------------------------

/// État d'un fondu, à `A4` (= A5 + 0x48e) : octet des pages concernées (bit k : page k), nombre de pas
/// restants, délai entre deux pas (mots) et adresses des quatre pages à traiter.
mod fade {
    pub const PAGES: u32 = 0;
    pub const PAGE_PTR: u32 = 4;
}

/// Nombre de couleurs d'une page.
const COLORS: u32 = 512;
/// Couleurs de la première page traitées par le fondu partiel (0x15c8) : les 48 dernières palettes
/// `0x91c3a0` à `0x91c400` (lues comme 3 tours de 16 mots).
const PARTIAL_WORDS: u32 = 48;
const BRIGHTNESS: u16 = 0xf000;

/// 0x1602 : ajoute le pas D1 (±0x1000) à la luminosité des 512 couleurs de la page A1, sauf si la
/// première couleur est déjà au bout (0 en descente, 0xf000 en montée) ou si elle atteindrait 0.
fn fade_page(v: &mut Vue, page: u32, delta: u32) {
    let first = v.rd16(page) & BRIGHTNESS;
    let negative = (delta as i32) < 0;
    if v.pris(!negative) {
        if v.pris(first == BRIGHTNESS) {
            return;
        }
    } else if v.pris(first == 0) {
        return;
    }
    if v.pris(first.wrapping_add(delta as u16) & BRIGHTNESS == 0) {
        return;
    }
    add_to_colors(v, page, COLORS, delta as u16);
}

/// 0x164e : comme `fade_page` sur les 48 couleurs de la fin de la première page ; la première couleur
/// ne sert qu'à tester la luminosité (en descente seulement : aucun test de butée).
fn fade_partial_page(v: &mut Vue, page: u32, delta: u32) {
    let mut first = v.rd16(page);
    if !v.pris((delta as i32) < 0) {
        first = first.wrapping_add(delta as u16);
    }
    if v.pris(first & BRIGHTNESS == 0) {
        return;
    }
    add_to_colors(v, page, PARTIAL_WORDS, delta as u16);
}

/// Ajoute `delta` (modulo 2^16) aux `n` couleurs à partir de `page` (n multiple de 16).
fn add_to_colors(v: &mut Vue, page: u32, n: u32, delta: u16) {
    if v.reel() {
        for i in 0..n {
            let a = page + 2 * i;
            let color = v.rd16(a).wrapping_add(delta);
            v.wr16(a, color);
        }
    }
    tours(&mut v.chemin, n / 16);
}

/// Pages cochées dans l'état de fondu, avec leur adresse.
fn selected_pages(b: &mut Cps2Bus, state: u32) -> Vec<(usize, u32)> {
    let flags = rd8(b, state + fade::PAGES);
    (0..4).filter(|k| flags >> k & 1 != 0).map(|k| (k, rd32(b, state + fade::PAGE_PTR + 4 * k as u32))).collect()
}

/// 0x158e / 0x15c8 : un pas de fondu sur les pages de l'état A4 avec le pas D1. `partial` : la première
/// page est la fin de page du fondu partiel.
fn fade_step(c: &mut Cpu, b: &mut Cps2Bus, partial: bool) {
    for (k, page) in selected_pages(b, c.a[4]) {
        let mut v = Vue::reelle(b);
        if partial && k == 0 {
            fade_partial_page(&mut v, page, c.d[1]);
        } else {
            fade_page(&mut v, page, c.d[1]);
        }
    }
    rts(c, b);
}

/// Durée de `fade_page` / `fade_partial_page` depuis leur entrée.
fn fade_page_cost(b: &mut Cps2Bus, page: u32, delta: u32, partial: bool) -> u64 {
    let mut v = Vue::simulee(b);
    if partial {
        fade_partial_page(&mut v, page, delta);
    } else {
        fade_page(&mut v, page, delta);
    }
    v.chemin.duree(b, if partial { 0x164e } else { 0x1602 })
}

fn fade_step_cost(c: &Cpu, b: &mut Cps2Bus, partial: bool) -> u64 {
    let mut skeleton = Chemin::default();
    let flags = rd8(b, c.a[4] + fade::PAGES);
    for k in 0..4 {
        skeleton.pris(flags >> k & 1 == 0);
    }
    let entry = if partial { 0x15c8 } else { 0x158e };
    let mut total = skeleton.duree_hors(b, entry, None, &[0x1602, 0x164e]);
    for (k, page) in selected_pages(b, c.a[4]) {
        total += fade_page_cost(b, page, c.d[1], partial && k == 0);
    }
    total
}

fn fade_page_native(c: &mut Cpu, b: &mut Cps2Bus) {
    fade_page(&mut Vue::reelle(b), c.a[1], c.d[1]);
    rts(c, b);
}

fn fade_partial_page_native(c: &mut Cpu, b: &mut Cps2Bus) {
    fade_partial_page(&mut Vue::reelle(b), c.a[1], c.d[1]);
    rts(c, b);
}

pub const FADE_PAGE: Native = exact(0x1602, "palette_fondu_page", |c, b| fade_page_cost(b, c.a[1], c.d[1], false), fade_page_native);
pub const FADE_PARTIAL_PAGE: Native = exact(0x164e, "palette_fondu_page_partielle", |c, b| fade_page_cost(b, c.a[1], c.d[1], true), fade_partial_page_native);
pub const FADE_STEP: Native = exact(0x158e, "palette_fondu_pas", |c, b| fade_step_cost(c, b, false), |c, b| fade_step(c, b, false));
pub const FADE_STEP_PARTIAL: Native = exact(0x15c8, "palette_fondu_pas_partiel", |c, b| fade_step_cost(c, b, true), |c, b| fade_step(c, b, true));

const fn exact(entry: u32, name: &'static str, cost: fn(&Cpu, &mut Cps2Bus) -> u64, run: fn(&mut Cpu, &mut Cps2Bus)) -> Native {
    Native { entry, name, cost, run, touches_sound: false, outputs: 0, exact_cost: true }
}

// ---------------------------------------------------------------------------------------------
// Chargement des palettes de scène depuis la bibliothèque de la ROM (V3)
// ---------------------------------------------------------------------------------------------

/// Bibliothèque de palettes (0x253000-0x293000) : palettes de 16 couleurs, 32 octets chacune.
const LIBRARY: u32 = 0x253000;
/// Par scène : 32 déplacements (mots) dans la bibliothèque, un déplacement négatif laisse la palette.
const STAGE_PALETTES: u32 = 0x2ae81e;
/// Palettes ajoutées à la suite de celles de la scène : adresse (long, 0 = aucune) par scène, en code.
const STAGE_EXTRA: u32 = 0x1a41c;
const EXTRA_PALETTES: u32 = 3;
/// Scènes dont les 32 palettes sont chargées (les autres : 29).
const FULL_RANGE: std::ops::Range<u8> = 0x45..0x5a;
const STAGE_VAR: u32 = 0x34;
const PALETTE_BYTES: u32 = 32;

fn copy_palette(v: &mut Vue, from: u32, to: u32) {
    for i in 0..PALETTE_BYTES / 4 {
        let long = v.rd32(from + 4 * i);
        v.wr32(to + 4 * i, long);
    }
}

fn prog_long(v: &mut Vue, a: u32) -> u32 {
    (v.prog16(a) as u32) << 16 | v.prog16(a + 2) as u32
}

/// Adresse d'une table de code indexée par `n * 4` (calcul 16 bits, mot signé). Les tables lues par
/// un accès relatif au PC (pages 1 à 3) sont en espace programme ; celles dont l'adresse passe par
/// un registre (`lea (d,PC),A3` puis indexation) en espace données.
fn table_entry(v: &mut Vue, table: u32, n: u32, program: bool) -> u32 {
    let at = table.wrapping_add((n as u16).wrapping_shl(2) as i16 as u32);
    if program {
        prog_long(v, at)
    } else {
        v.rd32(at)
    }
}

/// Fin du chargeur de palettes de scène : de quoi reconstituer les registres de l'original.
struct StageEnd {
    /// Destination suivante (A4).
    dst: u32,
    /// Fin de la liste de déplacements lue (A6).
    list: u32,
    /// Dernière palette copiée (destination), dont les huit longs restent dans D1-D6, A1, A2.
    last: Option<u32>,
    /// Adresse des palettes supplémentaires (D1 si nulle, A3 = adresse + 96 sinon).
    extra: u32,
}

/// 0x1a36c : palettes de la scène `d0` (octet bas) à partir de `dst`, puis palettes supplémentaires
/// de la scène en cours (`A5 + 0x34`).
fn load_stage_palettes(v: &mut Vue, a5: u32, d0: u32, mut dst: u32) -> StageEnd {
    let stage = d0 as u8;
    let count = if v.pris((stage as i8) < FULL_RANGE.start as i8) {
        29
    } else if v.pris(stage >= FULL_RANGE.end) {
        29
    } else {
        32
    };
    let mut list = STAGE_PALETTES + ((d0 as u16) << 6) as i16 as u32;
    let mut last = None;
    for i in 0..count {
        let offset = v.rd16(list);
        list += 2;
        if !v.pris(offset & 0x8000 != 0) {
            copy_palette(v, LIBRARY + offset as i16 as u32, dst);
            last = Some(dst);
        }
        dst += PALETTE_BYTES;
        v.pris(i + 1 < count);
    }
    let current = v.rd8(a5 + STAGE_VAR) as u32;
    let extra = table_entry(v, STAGE_EXTRA, current, false);
    if !v.pris(extra == 0) {
        for i in 0..EXTRA_PALETTES {
            copy_palette(v, extra + i * PALETTE_BYTES, dst);
            last = Some(dst);
            dst += PALETTE_BYTES;
            v.pris(i + 1 < EXTRA_PALETTES);
        }
    }
    StageEnd { dst, list, last, extra }
}

/// Les huit longs d'une palette, tels que `movem.l (A),{D1-D6,A1,A2}` les charge.
fn palette_longs(b: &mut Cps2Bus, at: u32) -> [u32; 8] {
    std::array::from_fn(|i| rd32(b, at + 4 * i as u32))
}

fn set_low_word(r: &mut u32, v: u16) {
    *r = *r & 0xffff_0000 | v as u32;
}

/// Registres que 0x1a36c laisse : D0 = scène en cours x 4, D1-D6/A1/A2 = dernière palette copiée
/// (D1 = 0 s'il n'y a pas de palettes supplémentaires), A3, A4, A6, D7 = 0xffff.
fn stage_regs(c: &mut Cpu, b: &mut Cps2Bus, end: &StageEnd) {
    let current = rd8(b, c.a[5] + STAGE_VAR) as u32;
    c.d[0] = (current << 2) & 0xffff;
    if let Some(at) = end.last {
        let l = palette_longs(b, at);
        c.d[1..7].copy_from_slice(&l[..6]);
        c.a[1] = l[6];
        c.a[2] = l[7];
    }
    if end.extra == 0 {
        c.d[1] = 0;
        c.a[3] = STAGE_EXTRA;
    } else {
        c.a[3] = end.extra + EXTRA_PALETTES * PALETTE_BYTES;
    }
    c.d[7] = 0xffff;
    c.a[4] = end.dst;
    c.a[6] = end.list;
}

/// Arguments de 0x1a350 / 0x1a360 / 0x1a366 : numéro de scène chargé (D0) et destination.
/// 0x1a350 et 0x1a360 lisent la scène en cours ; 0x1a366 prend D0 tel quel. 0x1a350 écrit dans la
/// couche de défilement 1 (scènes 0x2f et 0x42, où la page 0 sert de source de fondu).
fn stage_args(b: &mut Cps2Bus, c: &Cpu, entry: u32) -> (u32, u32) {
    let stage = rd8(b, c.a[5] + STAGE_VAR) as u32;
    match entry {
        0x1a350 => (stage, hw::SCROLL1),
        0x1a360 => (stage, PALETTES),
        _ => (c.d[0], PALETTES),
    }
}

fn stage_run(c: &mut Cpu, b: &mut Cps2Bus, entry: u32) {
    let (d0, dst) = stage_args(b, c, entry);
    let end = load_stage_palettes(&mut Vue::reelle(b), c.a[5], d0, dst);
    stage_regs(c, b, &end);
    rts(c, b);
}

fn stage_cost_raw(b: &mut Cps2Bus, a5: u32, d0: u32, dst: u32, entry: u32) -> u64 {
    let mut v = Vue::simulee(b);
    load_stage_palettes(&mut v, a5, d0, dst);
    v.chemin.duree(b, entry)
}

fn stage_cost(c: &Cpu, b: &mut Cps2Bus, entry: u32) -> u64 {
    let (d0, dst) = stage_args(b, c, entry);
    stage_cost_raw(b, c.a[5], d0, dst, entry)
}

pub const STAGE_FREE: Native = exact(0x1a366, "palette_scene_d0", |c, b| stage_cost(c, b, 0x1a366), |c, b| stage_run(c, b, 0x1a366));
pub const STAGE_PAGE0: Native = exact(0x1a360, "palette_scene", |c, b| stage_cost(c, b, 0x1a360), |c, b| stage_run(c, b, 0x1a360));
pub const STAGE_SCROLL1: Native = exact(0x1a350, "palette_scene_scroll1", |c, b| stage_cost(c, b, 0x1a350), |c, b| stage_run(c, b, 0x1a350));

/// 0x1a3d4 : les 32 palettes de l'ensemble D0 (adresse en code à `0x1a3fc + 4 D0`) dans la page 0.
const SETS: u32 = 0x1a3fc;
const PAGE_PALETTES: u32 = 32;

/// Rend l'adresse source (0 : ensemble absent).
fn load_set(v: &mut Vue, d0: u32) -> u32 {
    let source = table_entry(v, SETS, d0, false);
    if !v.pris(source == 0) {
        for i in 0..PAGE_PALETTES {
            copy_palette(v, source + i * PALETTE_BYTES, PALETTES + i * PALETTE_BYTES);
            v.pris(i + 1 < PAGE_PALETTES);
        }
    }
    source
}

fn set_run(c: &mut Cpu, b: &mut Cps2Bus) {
    let source = load_set(&mut Vue::reelle(b), c.d[0]);
    // registres de l'original : D0.w décalé, A4 = fin de la page, A3 = fin de la source
    let shifted = (c.d[0] as u16) << 2;
    set_low_word(&mut c.d[0], shifted);
    if source == 0 {
        c.d[1] = 0;
        c.a[3] = SETS;
        c.a[4] = PALETTES;
    } else {
        let l = palette_longs(b, PALETTES + (PAGE_PALETTES - 1) * PALETTE_BYTES);
        c.d[1..7].copy_from_slice(&l[..6]);
        c.a[1] = l[6];
        c.a[2] = l[7];
        set_low_word(&mut c.d[7], 0xffff);
        c.a[3] = source + PAGE_PALETTES * PALETTE_BYTES;
        c.a[4] = PALETTES + PAGE_PALETTES * PALETTE_BYTES;
    }
    rts(c, b);
}

fn set_cost(c: &Cpu, b: &mut Cps2Bus) -> u64 {
    let mut v = Vue::simulee(b);
    let _ = load_set(&mut v, c.d[0]);
    v.chemin.duree(b, 0x1a3d4)
}

pub const LOAD_SET: Native = exact(0x1a3d4, "palette_ensemble", set_cost, set_run);

/// Pages 1 à 3 : table de code (adresse par scène), entrée « scène en cours », entrée « D0 donné ».
const PAGE_TABLES: [(u32, u32, u32); 3] = [(0x1a62c, 0x1a608, 0x1a60a), (0x1a6a0, 0x1a678, 0x1a67e), (0x1a8d4, 0x1a8ac, 0x1a8b2)];

/// Les 32 palettes de la page `k + 1` d'après la table de la page, entrée `d0`.
fn load_page(v: &mut Vue, k: usize, d0: u32) -> u32 {
    let source = table_entry(v, PAGE_TABLES[k].0, d0, true);
    let page = PALETTES + PAGE_SIZE * (k as u32 + 1);
    for i in 0..PAGE_PALETTES {
        copy_palette(v, source + i * PALETTE_BYTES, page + i * PALETTE_BYTES);
        v.pris(i + 1 < PAGE_PALETTES);
    }
    source
}

/// Registres que laisse un chargeur de page : D0-D6 = sept premiers longs de la dernière palette,
/// le huitième dans A4 (pages 1 et 2) ou A2 (page 3) ; source et destination (fin) dans A2/A3
/// (pages 1 et 2) ou A4/A1 (page 3) ; D7.w = -1.
fn page_regs(c: &mut Cpu, b: &mut Cps2Bus, k: usize, source: u32) {
    let page = PALETTES + PAGE_SIZE * (k as u32 + 1);
    let l = palette_longs(b, page + (PAGE_PALETTES - 1) * PALETTE_BYTES);
    c.d[..7].copy_from_slice(&l[..7]);
    let (source_end, page_end) = (source + PAGE_PALETTES * PALETTE_BYTES, page + PAGE_PALETTES * PALETTE_BYTES);
    if k < 2 {
        c.a[4] = l[7];
        c.a[2] = source_end;
        c.a[3] = page_end;
    } else {
        c.a[2] = l[7];
        c.a[4] = source_end;
        c.a[1] = page_end;
    }
    set_low_word(&mut c.d[7], 0xffff);
}

/// D0 effectif d'un chargeur de page : celui de l'appelant pour l'entrée « D0 », sinon 0 (page 1)
/// ou la scène en cours.
fn page_d0(b: &mut Cps2Bus, c: &Cpu, k: usize, entry: u32) -> u32 {
    if entry == PAGE_TABLES[k].2 {
        c.d[0]
    } else if k == 0 {
        0
    } else {
        rd8(b, c.a[5] + STAGE_VAR) as u32
    }
}

fn page_run(c: &mut Cpu, b: &mut Cps2Bus, k: usize, entry: u32) {
    let d0 = page_d0(b, c, k, entry);
    let source = load_page(&mut Vue::reelle(b), k, d0);
    page_regs(c, b, k, source);
    rts(c, b);
}

fn page_cost_raw(b: &mut Cps2Bus, k: usize, d0: u32, entry: u32) -> u64 {
    let mut v = Vue::simulee(b);
    load_page(&mut v, k, d0);
    v.chemin.duree(b, entry)
}

macro_rules! page_native {
    ($name:ident, $nom:literal, $k:literal, $entry:expr) => {
        pub const $name: Native = exact(
            $entry,
            $nom,
            |c, b| {
                let d0 = page_d0(b, c, $k, $entry);
                page_cost_raw(b, $k, d0, $entry)
            },
            |c, b| page_run(c, b, $k, $entry),
        );
    };
}

page_native!(PAGE1_STAGE, "palette_page1", 0, 0x1a608);
page_native!(PAGE1_D0, "palette_page1_d0", 0, 0x1a60a);
page_native!(PAGE2_STAGE, "palette_page2", 1, 0x1a678);
page_native!(PAGE2_D0, "palette_page2_d0", 1, 0x1a67e);
page_native!(PAGE3_STAGE, "palette_page3", 2, 0x1a8ac);
page_native!(PAGE3_D0, "palette_page3_d0", 2, 0x1a8b2);

/// 0x1a296 : toutes les palettes de la scène en cours (pages 0 à 3), met à zéro le mot `0x91d7fe` ;
/// pour les scènes 0x2f et 0x42 la page 0 est chargée dans la couche de défilement 1 et les quatre
/// sources du fondu (`A5 + 0x492`) sont initialisées.
const FADE_SOURCES: u32 = 0x492;
const CLEARED_WORD: u32 = 0x91d7fe;

/// Sources du fondu des scènes 0x2f et 0x42 : (couche 1, tampon de travail, deux palettes en ROM).
fn special_sources(stage: u8) -> Option<[u32; 4]> {
    match stage {
        0x2f => Some([hw::SCROLL1, 0xff1190, 0x275360, 0x285680]),
        0x42 => Some([hw::SCROLL1, 0xff1190, 0x277780, 0x287ae0]),
        _ => None,
    }
}

fn load_all(c: &mut Cpu, b: &mut Cps2Bus) {
    let stage = rd8(b, c.a[5] + STAGE_VAR);
    let special = special_sources(stage);
    let entry = if special.is_some() { 0x1a350 } else { 0x1a360 };
    let (d0, dst) = stage_args(b, c, entry);
    // une vue par chargeur : chacun a ses propres décisions de boucle
    let end = load_stage_palettes(&mut Vue::reelle(b), c.a[5], d0, dst);
    stage_regs(c, b, &end);
    for (k, d0) in [(0, 0), (1, stage as u32), (2, stage as u32)] {
        let source = load_page(&mut Vue::reelle(b), k, d0);
        page_regs(c, b, k, source);
    }
    wr16(b, CLEARED_WORD, 0);
    c.flags_logic(0, m68k::decode::Size::Word);
    if let Some(sources) = special {
        for (k, source) in sources.into_iter().enumerate() {
            wr32(b, c.a[5] + FADE_SOURCES + 4 * k as u32, source);
        }
    }
    rts(c, b);
}

fn load_all_cost(c: &Cpu, b: &mut Cps2Bus) -> u64 {
    let stage = rd8(b, c.a[5] + STAGE_VAR);
    let special = special_sources(stage).is_some();
    let mut skeleton = Chemin::default();
    skeleton.pris(stage == 0x2f);
    if stage != 0x2f {
        skeleton.pris(stage == 0x42);
    }
    let entry = if special { 0x1a350 } else { 0x1a360 };
    let externes = [0x1a350, 0x1a360, 0x1a608, 0x1a678, 0x1a8ac];
    let (d0, dst) = stage_args(b, c, entry);
    skeleton.duree_hors(b, 0x1a296, None, &externes)
        + stage_cost_raw(b, c.a[5], d0, dst, entry)
        + page_cost_raw(b, 0, 0, 0x1a608)
        + page_cost_raw(b, 1, stage as u32, 0x1a678)
        + page_cost_raw(b, 2, stage as u32, 0x1a8ac)
}

pub const LOAD_ALL: Native = exact(0x1a296, "palette_chargement_scene", load_all_cost, load_all);

// ---------------------------------------------------------------------------------------------
// Fondus par composante (V3) : chaque couleur approche d'une cible d'un cran par composante
// ---------------------------------------------------------------------------------------------

/// Sources des fondus (`A5 + 0x492`, 4 mots longs : une palette complète de 512 couleurs par page
/// pour 0x1cd7c, une couleur (mot) par page pour 0x1cdd6).
const SOURCES: u32 = 0x492;
const PAGES_FLAG: u32 = 0x48e;

/// Position d'une composante par rapport à sa cible.
#[derive(Clone, Copy)]
enum Cmp {
    Equal,
    Up,
    Down,
}

/// Fait avancer d'un cran chaque composante RGB (quartets 0 à 2) de `color` vers `target`. La
/// luminosité (quartet 3) est conservée. Rend la couleur et la position de chaque composante.
fn approach(color: u16, target: u16) -> (u16, [Cmp; 3]) {
    let mut out = color;
    let mut cmps = [Cmp::Equal; 3];
    for (n, cmp) in cmps.iter_mut().enumerate() {
        let shift = 4 * n as u32;
        let (cur, goal) = (color >> shift & 0xf, target >> shift & 0xf);
        let next = match goal.cmp(&cur) {
            std::cmp::Ordering::Equal => cur,
            std::cmp::Ordering::Greater => cur + 1,
            std::cmp::Ordering::Less => cur - 1,
        };
        *cmp = match goal.cmp(&cur) {
            std::cmp::Ordering::Equal => Cmp::Equal,
            std::cmp::Ordering::Greater => Cmp::Up,
            std::cmp::Ordering::Less => Cmp::Down,
        };
        out = out & !(0xf << shift) | next << shift;
    }
    (out, cmps)
}

/// Fait avancer chaque composante RGB d'un cran vers 0xf (blanc).
fn brighten(color: u16) -> (u16, [Cmp; 3]) {
    let mut out = color;
    let mut cmps = [Cmp::Equal; 3];
    for (n, cmp) in cmps.iter_mut().enumerate() {
        let shift = 4 * n as u32;
        if color >> shift & 0xf != 0xf {
            out += 1 << shift;
            *cmp = Cmp::Up;
        }
    }
    (out, cmps)
}

/// Repères d'une boucle sur les 512 couleurs d'une page.
#[derive(Clone, Copy)]
struct ColorLoop {
    /// Première instruction du traitement d'une couleur.
    start: u32,
    /// Première instruction du traitement d'une composante, et ce qui suit le dernier tour.
    nibble: u32,
    after_nibbles: u32,
    /// Décisions d'une composante : égal / plus / moins, ou seulement (butée / plus).
    three_way: bool,
}

const TARGET_LOOP: ColorLoop = ColorLoop { start: 0x1cfcc, nibble: 0x1cfd6, after_nibbles: 0x1d004, three_way: true };
const CONSTANT_LOOP: ColorLoop = ColorLoop { start: 0x1d046, nibble: 0x1d04e, after_nibbles: 0x1d07c, three_way: true };
const WHITE_LOOP: ColorLoop = ColorLoop { start: 0x1d012, nibble: 0x1d01c, after_nibbles: 0x1d038, three_way: false };

impl ColorLoop {
    fn nibble_cost(&self, b: &mut Cps2Bus, cmp: Cmp, last: bool) -> u64 {
        let mut p = Chemin::default();
        match cmp {
            Cmp::Equal => p.pris(true),
            Cmp::Up => {
                p.pris(false);
                if self.three_way {
                    p.pris(true);
                }
            }
            Cmp::Down => {
                p.pris(false);
                p.pris(false);
            }
        }
        p.pris(!last);
        p.duree_entre(b, self.nibble, Some(if last { self.after_nibbles } else { self.nibble }))
    }

    /// Durée de la boucle sur 512 couleurs, d'après les comparaisons de chaque composante.
    fn cost(&self, b: &mut Cps2Bus, entry: u32, cmps: &[[Cmp; 3]]) -> u64 {
        let mut table = [[0u64; 2]; 3];
        for cmp in [Cmp::Equal, Cmp::Up, Cmp::Down] {
            if self.three_way || !matches!(cmp, Cmp::Down) {
                table[cmp as usize] = [self.nibble_cost(b, cmp, false), self.nibble_cost(b, cmp, true)];
            }
        }
        let prologue = Chemin::default().duree_entre(b, self.start, Some(self.nibble));
        let (mut again, mut last) = (Chemin::default(), Chemin::default());
        again.pris(true);
        last.pris(false);
        let next_color = again.duree_entre(b, self.after_nibbles, Some(self.start));
        let finish = last.duree(b, self.after_nibbles);
        let mut total = if entry == self.start { 0 } else { Chemin::default().duree_entre(b, entry, Some(self.start)) };
        for (i, colors) in cmps.iter().enumerate() {
            total += prologue + if i + 1 < cmps.len() { next_color } else { finish };
            for (k, cmp) in colors.iter().enumerate() {
                total += table[*cmp as usize][(k == 2) as usize];
            }
        }
        total
    }
}

fn colors_of(b: &mut Cps2Bus, page: u32, f: impl Fn(&mut Cps2Bus, u32, u32) -> (u16, [Cmp; 3])) -> Vec<(u16, [Cmp; 3])> {
    (0..COLORS).map(|i| f(b, page, i)).collect()
}

fn rewrite(b: &mut Cps2Bus, page: u32, results: &[(u16, [Cmp; 3])]) {
    for (i, (color, _)) in results.iter().enumerate() {
        wr16(b, page + 2 * i as u32, *color);
    }
}

fn cmps_of(results: Vec<(u16, [Cmp; 3])>) -> Vec<[Cmp; 3]> {
    results.into_iter().map(|(_, c)| c).collect()
}

/// 0x1cfc8 : approche les 512 couleurs de la page A0 des couleurs cibles à A1.
fn approach_page_results(b: &mut Cps2Bus, page: u32, target: u32) -> Vec<(u16, [Cmp; 3])> {
    colors_of(b, page, |b, page, i| approach(rd16(b, page + 2 * i), rd16(b, target + 2 * i)))
}

fn approach_page(b: &mut Cps2Bus, page: u32, target: u32) {
    let results = approach_page_results(b, page, target);
    rewrite(b, page, &results);
}

fn approach_page_cost(b: &mut Cps2Bus, page: u32, target: u32) -> u64 {
    let cmps = cmps_of(approach_page_results(b, page, target));
    TARGET_LOOP.cost(b, 0x1cfc8, &cmps)
}

/// 0x1d042 : approche les 512 couleurs de la page A0 de la couleur unique D1.
fn approach_constant_results(b: &mut Cps2Bus, page: u32, goal: u16) -> Vec<(u16, [Cmp; 3])> {
    colors_of(b, page, |b, page, i| approach(rd16(b, page + 2 * i), goal))
}

fn approach_constant(b: &mut Cps2Bus, page: u32, goal: u16) {
    let results = approach_constant_results(b, page, goal);
    rewrite(b, page, &results);
}

fn approach_constant_cost(b: &mut Cps2Bus, page: u32, goal: u16) -> u64 {
    let cmps = cmps_of(approach_constant_results(b, page, goal));
    CONSTANT_LOOP.cost(b, 0x1d042, &cmps)
}

/// 0x1d00e : les 512 couleurs de la page A0 gagnent un cran vers le blanc.
fn brighten_page(b: &mut Cps2Bus, page: u32) {
    let results = colors_of(b, page, |b, page, i| brighten(rd16(b, page + 2 * i)));
    rewrite(b, page, &results);
}

fn brighten_page_cost(b: &mut Cps2Bus, page: u32) -> u64 {
    let cmps = cmps_of(colors_of(b, page, |b, page, i| brighten(rd16(b, page + 2 * i))));
    WHITE_LOOP.cost(b, 0x1d00e, &cmps)
}

/// Pages cochées (A5 + 0x48e) avec leur adresse en GFX RAM.
fn flagged_pages(b: &mut Cps2Bus, a5: u32) -> Vec<(usize, u32)> {
    let flags = rd8(b, a5 + PAGES_FLAG);
    (0..4).filter(|k| flags >> k & 1 != 0).map(|k| (k, PALETTES + PAGE_SIZE * k as u32)).collect()
}

/// Durée du squelette des routines qui parcourent les pages cochées et appellent `external` pour chacune.
fn pages_skeleton(b: &mut Cps2Bus, a5: u32, entry: u32, external: u32) -> u64 {
    let flags = rd8(b, a5 + PAGES_FLAG);
    let mut p = Chemin::default();
    for k in 0..4 {
        p.pris(flags >> k & 1 == 0);
    }
    p.duree_hors(b, entry, None, &[external])
}

/// 0x1cd7c : un pas de fondu de chaque page cochée vers sa palette cible (`A5 + 0x492 + 4 k`).
fn fade_to_palettes(c: &mut Cpu, b: &mut Cps2Bus) {
    for (k, page) in flagged_pages(b, c.a[5]) {
        let target = rd32(b, c.a[5] + SOURCES + 4 * k as u32);
        approach_page(b, page, target);
    }
    rts(c, b);
}

fn fade_to_palettes_cost(c: &Cpu, b: &mut Cps2Bus) -> u64 {
    let mut total = pages_skeleton(b, c.a[5], 0x1cd7c, 0x1cfc8);
    for (k, page) in flagged_pages(b, c.a[5]) {
        let target = rd32(b, c.a[5] + SOURCES + 4 * k as u32);
        total += approach_page_cost(b, page, target);
    }
    total
}

/// 0x1cdd6 : un pas de fondu de chaque page cochée vers la couleur (mot) `A5 + 0x492 + 4 k`.
fn fade_to_colors(c: &mut Cpu, b: &mut Cps2Bus) {
    for (k, page) in flagged_pages(b, c.a[5]) {
        let goal = rd16(b, c.a[5] + SOURCES + 4 * k as u32);
        approach_constant(b, page, goal);
    }
    rts(c, b);
}

fn fade_to_colors_cost(c: &Cpu, b: &mut Cps2Bus) -> u64 {
    let mut total = pages_skeleton(b, c.a[5], 0x1cdd6, 0x1d042);
    for (k, page) in flagged_pages(b, c.a[5]) {
        let goal = rd16(b, c.a[5] + SOURCES + 4 * k as u32);
        total += approach_constant_cost(b, page, goal);
    }
    total
}

/// 0x1cd32 : un pas de fondu vers le blanc de chaque page cochée.
fn fade_to_white(c: &mut Cpu, b: &mut Cps2Bus) {
    for (_, page) in flagged_pages(b, c.a[5]) {
        brighten_page(b, page);
    }
    rts(c, b);
}

fn fade_to_white_cost(c: &Cpu, b: &mut Cps2Bus) -> u64 {
    let mut total = pages_skeleton(b, c.a[5], 0x1cd32, 0x1d00e);
    for (_, page) in flagged_pages(b, c.a[5]) {
        total += brighten_page_cost(b, page);
    }
    total
}

/// Recopie 8 longs (D0 à D6 et A1) sur toute la page (32 palettes) : 0x1cfb8.
fn fill_page(b: &mut Cps2Bus, page: u32, values: [u32; 8]) {
    for row in 0..32 {
        for (i, value) in values.iter().enumerate() {
            wr32(b, page + 32 * row + 4 * i as u32, *value);
        }
    }
}

fn fill_page_cost(b: &mut Cps2Bus) -> u64 {
    let mut p = Chemin::default();
    tours(&mut p, 32);
    p.duree(b, 0x1cfb8)
}

fn fill_page_native(c: &mut Cpu, b: &mut Cps2Bus) {
    let values = [c.d[0], c.d[1], c.d[2], c.d[3], c.d[4], c.d[5], c.d[6], c.a[1]];
    fill_page(b, c.a[0], values);
    c.a[0] = c.a[0].wrapping_add(0x400);
    rts(c, b);
}

/// 0x1cf3c : pages cochées toutes blanches.
fn white_pages(c: &mut Cpu, b: &mut Cps2Bus) {
    for (_, page) in flagged_pages(b, c.a[5]) {
        fill_page(b, page, [u32::MAX; 8]);
    }
    rts(c, b);
}

fn white_pages_cost(c: &Cpu, b: &mut Cps2Bus) -> u64 {
    let pages = flagged_pages(b, c.a[5]).len() as u64;
    pages_skeleton(b, c.a[5], 0x1cf3c, 0x1cfb8) + pages * fill_page_cost(b)
}

pub const FADE_TO_PALETTES: Native = exact(0x1cd7c, "palette_fondu_vers_palettes", fade_to_palettes_cost, fade_to_palettes);
pub const FADE_TO_COLORS: Native = exact(0x1cdd6, "palette_fondu_vers_couleurs", fade_to_colors_cost, fade_to_colors);
pub const FADE_TO_WHITE: Native = exact(0x1cd32, "palette_fondu_vers_blanc", fade_to_white_cost, fade_to_white);
pub const WHITE_PAGES: Native = exact(0x1cf3c, "palette_pages_blanches", white_pages_cost, white_pages);
pub const FILL_PAGE: Native = exact(0x1cfb8, "palette_page_unie", |_, b| fill_page_cost(b), fill_page_native);
pub const APPROACH_PAGE: Native = exact(
    0x1cfc8,
    "palette_approche_page",
    |c, b| approach_page_cost(b, c.a[0], c.a[1]),
    |c, b| {
        approach_page(b, c.a[0], c.a[1]);
        rts(c, b);
    },
);
pub const APPROACH_CONSTANT: Native = exact(
    0x1d042,
    "palette_approche_couleur",
    |c, b| approach_constant_cost(b, c.a[0], c.d[1] as u16),
    |c, b| {
        approach_constant(b, c.a[0], c.d[1] as u16);
        rts(c, b);
    },
);
pub const BRIGHTEN_PAGE: Native = exact(
    0x1d00e,
    "palette_eclaircit_page",
    |c, b| brighten_page_cost(b, c.a[0]),
    |c, b| {
        brighten_page(b, c.a[0]);
        rts(c, b);
    },
);

// ---------------------------------------------------------------------------------------------
// Éclair : palettes grisées (U2)
// ---------------------------------------------------------------------------------------------

/// Palettes mises en réserve par l'éclair : une page de 0x400 couleurs et 48 couleurs de la page 0.
mod flash {
    pub const PAGE: u32 = 0x91c800;
    pub const PAGE_COLORS: u32 = 0x400;
    pub const SPARE: u32 = 0x91c3a0;
    pub const SPARE_COLORS: u32 = 0x30;
    /// Copie de réserve (RAM de travail) : la page, puis les 48 couleurs.
    pub const BACKUP: u32 = 0xff1190;
    pub const BACKUP_SPARE: u32 = BACKUP + 2 * PAGE_COLORS;
}

fn copy_words(b: &mut Cps2Bus, from: u32, to: u32, words: u32) {
    for i in 0..words {
        let v = rd16(b, from + 2 * i);
        wr16(b, to + 2 * i, v);
    }
}

/// Met les palettes de l'éclair en réserve (0x1ac24) ou les remet (0x1abe0).
fn copy_flash_palettes(b: &mut Cps2Bus, restore: bool) {
    for (live, backup, colors) in
        [(flash::PAGE, flash::BACKUP, flash::PAGE_COLORS), (flash::SPARE, flash::BACKUP_SPARE, flash::SPARE_COLORS)]
    {
        if restore {
            copy_words(b, backup, live, colors);
        } else {
            copy_words(b, live, backup, colors);
        }
    }
}

/// Niveau de gris d'une couleur : la moyenne arrondie de ses trois composantes (0 à 12).
fn flash_level(color: u16) -> u16 {
    ((color >> 8 & 0xf) + (color >> 4 & 0xf) + (color & 0xf) + 4) >> 2
}

/// Couleur d'un niveau : `base` + niveau à chaque quartet, puis masque. Le décalage de `base` fait déborder
/// le quartet de luminosité (voir les huit variantes ci-dessous).
fn flash_color(level: u16, base: u16, mask: u16) -> u16 {
    base.wrapping_add(level).wrapping_add(level << 4).wrapping_add(level << 8) & mask
}

/// 0x1ac24 : réserve des palettes de l'éclair.
fn save_flash_palettes(c: &mut Cpu, b: &mut Cps2Bus) {
    copy_flash_palettes(b, false);
    c.a[1] = flash::SPARE + 2 * flash::SPARE_COLORS;
    c.a[2] = flash::BACKUP_SPARE + 2 * flash::SPARE_COLORS;
    set_low_word(&mut c.d[6], 0xffff);
    rts(c, b);
}

/// 0x1abe0 : remise des palettes de l'éclair.
fn restore_flash_palettes(c: &mut Cpu, b: &mut Cps2Bus) {
    copy_flash_palettes(b, true);
    c.a[1] = flash::BACKUP_SPARE + 2 * flash::SPARE_COLORS;
    c.a[2] = flash::SPARE + 2 * flash::SPARE_COLORS;
    set_low_word(&mut c.d[6], 0xffff);
    rts(c, b);
}

/// Éclair : met les palettes en réserve puis les remplace par des gris teintés (`BASE` et `MASK` dans
/// D5 et D4 de l'original, `0x1ab6c`).
fn flash_gray<const BASE: u16, const MASK: u16>(c: &mut Cpu, b: &mut Cps2Bus) {
    copy_flash_palettes(b, false);
    let mut last = 0;
    for (start, colors) in [(flash::PAGE, flash::PAGE_COLORS), (flash::SPARE, flash::SPARE_COLORS)] {
        for i in 0..colors {
            last = rd16(b, start + 2 * i);
            let tinted = flash_color(flash_level(last), BASE, MASK);
            wr16(b, start + 2 * i, tinted);
        }
    }
    // registres de travail comme l'original les laisse (dernière couleur traitée)
    let level = flash_level(last);
    let (red, green) = (last >> 8 & 0xf, last >> 4 & 0xf);
    c.a[1] = flash::SPARE + 2 * flash::SPARE_COLORS;
    set_low_word(&mut c.d[0], flash_color(level, BASE, MASK));
    set_low_word(&mut c.d[1], red);
    set_low_word(&mut c.d[2], red + green);
    set_low_word(&mut c.d[3], level << 8);
    set_low_word(&mut c.d[4], MASK);
    set_low_word(&mut c.d[5], BASE);
    set_low_word(&mut c.d[6], 0xffff);
    rts(c, b);
}

const fn flash_native(entry: u32, name: &'static str, run: fn(&mut Cpu, &mut Cps2Bus)) -> Native {
    Native { entry, name, cost: |_, _| 3000, run, touches_sound: false, outputs: regs::ALL, exact_cost: false }
}

pub const FLASH_GRAY: Native = flash_native(0x1ab56, "palette_eclair_gris", flash_gray::<0xf300, 0xff00>);
pub const FLASH_A: Native = flash_native(0x1aaa0, "palette_eclair_a", flash_gray::<0xf002, 0xffff>);
pub const FLASH_B: Native = flash_native(0x1aaba, "palette_eclair_b", flash_gray::<0xf000, 0xf00f>);
pub const FLASH_C: Native = flash_native(0x1aad4, "palette_eclair_c", flash_gray::<0xf320, 0xffff>);
pub const FLASH_D: Native = flash_native(0x1aaee, "palette_eclair_d", flash_gray::<0xf033, 0xffff>);
pub const FLASH_E: Native = flash_native(0x1ab08, "palette_eclair_e", flash_gray::<0xf030, 0xffff>);
pub const FLASH_F: Native = flash_native(0x1ab22, "palette_eclair_f", flash_gray::<0xf004, 0xffff>);
pub const FLASH_G: Native = flash_native(0x1ab3c, "palette_eclair_g", flash_gray::<0xf040, 0xf0ff>);
pub const FLASH_RESTORE: Native = flash_native(0x1abe0, "palette_eclair_remet", restore_flash_palettes);
pub const FLASH_SAVE: Native = flash_native(0x1ac24, "palette_eclair_reserve", save_flash_palettes);

/// Routines natives de ce module (les réglages de luminosité sont déclarés dans `lib.rs`).
pub const NATIVES: &[Native] = &[
    FADE_PAGE,
    FADE_PARTIAL_PAGE,
    FADE_STEP,
    FADE_STEP_PARTIAL,
    STAGE_FREE,
    STAGE_PAGE0,
    STAGE_SCROLL1,
    LOAD_SET,
    PAGE1_STAGE,
    PAGE1_D0,
    PAGE2_STAGE,
    PAGE2_D0,
    PAGE3_STAGE,
    PAGE3_D0,
    LOAD_ALL,
    FADE_TO_PALETTES,
    FADE_TO_COLORS,
    FADE_TO_WHITE,
    WHITE_PAGES,
    FILL_PAGE,
    APPROACH_PAGE,
    APPROACH_CONSTANT,
    BRIGHTEN_PAGE,
    FLASH_GRAY,
    FLASH_A,
    FLASH_B,
    FLASH_C,
    FLASH_D,
    FLASH_E,
    FLASH_F,
    FLASH_G,
    FLASH_RESTORE,
    FLASH_SAVE,
];
