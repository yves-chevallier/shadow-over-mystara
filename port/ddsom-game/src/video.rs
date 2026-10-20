//! Primitives vidéo (V1) et tilemaps (V2) : couches, registres CPS-A/B, Object RAM, blocs de tuiles.
//!
//! Voir `docs/video.md` pour les formats. Les registres `0x804100-0x80411f` (CPS-A) contiennent les
//! adresses des tables en GFX RAM (en unités de 256 octets) : objets en `0x804100`, couches 1 à 3 en
//! `0x804102/04/06` ; `0x804166` (CPS-B) active les couches.

use cps2::Cps2Bus;
use m68k::Cpu;

use crate::chemin::Chemin;
use crate::mem::{rd16, rd8, rts, set_low_byte, wr16, wr32, wr8};
use crate::vue::Vue;
use crate::{regs, Native};

/// Variables de la RAM de travail (offsets depuis A5 = FF8000) propres au sous-système vidéo.
pub mod ram {
    /// Réglages (bit 10 : pas de jauge de chargement).
    pub const DIP: u32 = 0x7fd0;
    /// Image du registre d'activation des couches (écrit en `0x804166`).
    pub const LAYER_ENABLE: u32 = 0x14;
    /// Adresse de la table d'objets (valeur de `0x804100`).
    pub const OBJ_TABLE: u32 = 0x5ba;
    /// Bascule de banque de l'Object RAM (octet, inversée à chaque effacement).
    pub const OBJ_BANK: u32 = 0x5bc;
    /// Verrous de pièces : deux images de la sortie (`+0x10`, `+0x17`) et les bits forcés (`+0x18`).
    pub const LOCKOUT: [u32; 2] = [0x10, 0x17];
    pub const LOCKOUT_FORCED: u32 = 0x18;
    /// Octet non nul : les registres d'affichage ne sont plus écrits (écran figé), `A5 - 0x7155`.
    pub const FROZEN: u32 = 0xff0eab;
    /// Scène en cours.
    pub const STAGE: u32 = 0x34;
    /// Caméra 1 (`+0x4fe`) et 2 (`+0x54e`) : scroll en `+0x08` (x) et `+0x0c` (y).
    pub const CAMERAS: [u32; 2] = [0x4fe, 0x54e];
    /// Octet non nul : la caméra 2 est la caméra courante.
    pub const USE_CAMERA_2: u32 = 0x36;
    /// Positions de scroll courantes (x, y) de la caméra 1 et de la caméra 2.
    pub const SCROLL: [u32; 2] = [0x506, 0x50a];
    pub const SCROLL_2: [u32; 2] = [0x556, 0x55a];
}

/// Mémoire graphique et registres.
pub mod hw {
    pub const TEXT_LAYER: u32 = 0x912000;
    pub const SCROLL1: u32 = 0x910000;
    pub const SCROLL2: u32 = 0x914000;
    pub const SCROLL3: u32 = 0x918000;
    pub const PALETTES: u32 = 0x91c000;
    pub const OBJECTS: u32 = 0x708000;
    pub const CPS_A_OBJECTS: u32 = 0x804100;
    pub const CPS_A_SCROLL: [u32; 3] = [0x804102, 0x804104, 0x804106];
    pub const LAYER_ENABLE: u32 = 0x804166;
    pub const OBJ_BANK: u32 = 0x8040e1;
}

/// Décisions d'une boucle `dbf` de `n` tours (n >= 1) : continuer n - 1 fois, puis sortir.
pub fn tours(p: &mut Chemin, n: u32) {
    p.repete(n - 1, true);
    p.pris(false);
}

/// Durée d'une routine « préambule, boucle de `n` tours, suite jusqu'au RTS » dont le corps est
/// régulier (trop de tours pour un `Chemin`). `avant` : décisions du préambule ; `reprise` : décisions
/// d'un tour qui boucle (la dernière est la reprise) ; `dernier` : décisions du dernier tour et de la
/// suite. `debut` : première instruction de la boucle.
pub fn duree_boucle(b: &mut Cps2Bus, entree: u32, avant: &Chemin, debut: u32, reprise: &Chemin, dernier: &Chemin, n: u64) -> u64 {
    let prefixe = if entree == debut { 0 } else { avant.duree_entre(b, entree, Some(debut)) };
    prefixe + (n - 1) * reprise.duree_entre(b, debut, Some(debut)) + dernier.duree(b, debut)
}

/// Durée d'une routine sans branchement conditionnel.
pub fn ligne_droite(b: &mut Cps2Bus, entree: u32) -> u64 {
    Chemin::default().duree(b, entree)
}

fn a5(c: &Cpu, off: u32) -> u32 {
    c.a[5].wrapping_add(off)
}

fn set_low_word(r: &mut u32, v: u16) {
    *r = *r & 0xffff_0000 | v as u32;
}

// ---------------------------------------------------------------------------------------------
// V1 : jauge de chargement, verrous, couches, effacements
// ---------------------------------------------------------------------------------------------

/// Valeur de remplissage de la jauge de chargement : tuile 0x20 (espace), attribut 3.
const GAUGE: u32 = 0x0020_0003;
/// La jauge occupe les 32 premiers longs de la couche de caractères (la première colonne).
const GAUGE_LONGS: u32 = 32;

/// Décisions de 0xb46 pour l'état courant (jauge masquée par le bit 10 des réglages).
fn gauge_path(b: &mut Cps2Bus, a5: u32) -> Chemin {
    let mut p = Chemin::default();
    let hidden = rd16(b, a5 + ram::DIP) & 0x400 != 0;
    p.pris(hidden);
    if !hidden {
        tours(&mut p, GAUGE_LONGS);
    }
    p
}

/// 0xb40 : appelée à chaque attente (0xa40) ; fixe D1 puis dessine la jauge (0xb46).
fn gauge_with_value(c: &mut Cpu, b: &mut Cps2Bus) {
    c.d[1] = GAUGE;
    gauge(c, b);
}

/// 0xb46 : remplit la première colonne de la couche de caractères avec D1 (la jauge de chargement)
/// sauf si les réglages la masquent.
fn gauge(c: &mut Cpu, b: &mut Cps2Bus) {
    let hidden = rd16(b, a5(c, ram::DIP)) & 0x400 != 0;
    // Registres laissés pour les appelants (la mise à jour du monde les hérite) : D2 et les
    // drapeaux du dernier calcul, A0 après la dernière écriture.
    let flags_clear = !(m68k::SR_N | m68k::SR_Z | m68k::SR_V | m68k::SR_C);
    if hidden {
        c.d[2] = c.d[2] & 0xffff_0000 | 0x400;
        c.sr &= flags_clear;
    } else {
        for i in 0..GAUGE_LONGS {
            wr32(b, hw::TEXT_LAYER + 4 * i, c.d[1]);
        }
        c.d[2] |= 0xffff;
        c.a[0] = hw::TEXT_LAYER + 4 * GAUGE_LONGS;
        c.sr = c.sr & flags_clear
            | if c.d[1] == 0 { m68k::SR_Z } else { 0 }
            | if c.d[1] & 0x8000_0000 != 0 { m68k::SR_N } else { 0 };
    }
    rts(c, b);
}

fn gauge_cost(c: &Cpu, b: &mut Cps2Bus) -> u64 {
    gauge_path(b, c.a[5]).duree(b, 0xb46)
}

fn gauge_with_value_cost(c: &Cpu, b: &mut Cps2Bus) -> u64 {
    gauge_path(b, c.a[5]).duree(b, 0xb40)
}

/// 0xb96 : verrouille les compteurs de pièces des bits D0 (en plus des bits forcés).
fn lock(c: &mut Cpu, b: &mut Cps2Bus) {
    let forced = rd8(b, a5(c, ram::LOCKOUT_FORCED));
    for off in ram::LOCKOUT {
        let v = rd8(b, a5(c, off)) | c.d[0] as u8 | forced;
        wr8(b, a5(c, off), v);
    }
    set_low_byte(&mut c.d[1], forced);
    rts(c, b);
}

/// 0xbac : déverrouille les bits D0 (sauf les bits forcés). D0 est rendu inchangé.
fn unlock(c: &mut Cpu, b: &mut Cps2Bus) {
    let forced = rd8(b, a5(c, ram::LOCKOUT_FORCED));
    for off in ram::LOCKOUT {
        let v = rd8(b, a5(c, off)) & !(c.d[0] as u8) | forced;
        wr8(b, a5(c, off), v);
    }
    set_low_byte(&mut c.d[1], forced);
    rts(c, b);
}

/// 0xbc6 : active les couches D0 (bits ajoutés à l'image du registre) et programme les registres
/// d'affichage, sauf si l'écran est figé.
fn enable_layers(c: &mut Cpu, b: &mut Cps2Bus) {
    let enable = rd16(b, a5(c, ram::LAYER_ENABLE)) | c.d[0] as u16;
    wr16(b, a5(c, ram::LAYER_ENABLE), enable);
    if rd8(b, ram::FROZEN) == 0 {
        wr16(b, hw::LAYER_ENABLE, enable);
        let objects = rd16(b, a5(c, ram::OBJ_TABLE));
        wr16(b, hw::CPS_A_OBJECTS, objects);
        for (reg, base) in hw::CPS_A_SCROLL.into_iter().zip([hw::SCROLL1, hw::SCROLL2, hw::SCROLL3]) {
            wr16(b, reg, (base >> 8) as u16);
        }
    }
    rts(c, b);
}

fn enable_layers_cost(_: &Cpu, b: &mut Cps2Bus) -> u64 {
    let mut p = Chemin::default();
    p.pris(rd8(b, ram::FROZEN) != 0);
    p.duree(b, 0xbc6)
}

/// 0xbfc : désactive les couches D0 (D0 est complémenté au retour, comme l'original).
fn disable_layers(c: &mut Cpu, b: &mut Cps2Bus) {
    let mask = !(c.d[0] as u16);
    set_low_word(&mut c.d[0], mask);
    let enable = rd16(b, a5(c, ram::LAYER_ENABLE)) & mask;
    wr16(b, a5(c, ram::LAYER_ENABLE), enable);
    wr16(b, hw::LAYER_ENABLE, enable);
    rts(c, b);
}

/// 0xc58 : écrit l'entrée de fin de liste (0x8000, 0x18000000) en tête de l'Object RAM, deux fois,
/// avec bascule de la banque entre les deux : la liste est vide dans les deux banques.
fn clear_objects(c: &mut Cpu, b: &mut Cps2Bus) {
    for _ in 0..2 {
        wr32(b, hw::OBJECTS, 0x8000);
        wr32(b, hw::OBJECTS + 4, 0x1800_0000);
        let bank = rd8(b, a5(c, ram::OBJ_BANK)) ^ 0xff;
        wr8(b, a5(c, ram::OBJ_BANK), bank);
        wr8(b, hw::OBJ_BANK, bank);
    }
    rts(c, b);
}

/// Remplit [debut, fin) de cases (tuile, 0) : 0xca8 (scroll 1), 0xcc8 (scroll 2), 0xce8 (scroll 3).
fn fill_layer(b: &mut Cps2Bus, from: u32, to: u32, tile: u16) {
    for a in (from..to).step_by(4) {
        wr16(b, a, tile);
        wr16(b, a + 2, 0);
    }
}

/// Corps commun des trois remplissages : D0 = tuile, D1 = 0 (A0 et A1 sont rétablis).
fn fill_run(c: &mut Cpu, b: &mut Cps2Bus, from: u32, to: u32, tile: u16) {
    fill_layer(b, from, to, tile);
    set_low_word(&mut c.d[0], tile);
    c.d[1] = 0;
    rts(c, b);
}

fn clear_scroll1(c: &mut Cpu, b: &mut Cps2Bus) {
    fill_run(c, b, hw::SCROLL1, hw::SCROLL2, 0x20);
}
fn clear_scroll2(c: &mut Cpu, b: &mut Cps2Bus) {
    fill_run(c, b, hw::SCROLL2, hw::SCROLL3, 0x1800);
}
fn clear_scroll3(c: &mut Cpu, b: &mut Cps2Bus) {
    fill_run(c, b, hw::SCROLL3, hw::PALETTES, 0x1f00);
}

/// Durée d'un remplissage de `cases` cases (boucle `blt` en 0xd04 : reprise sauf au dernier tour).
fn fill_cost(b: &mut Cps2Bus, entry: u32, cases: u64) -> u64 {
    let (mut reprise, mut dernier) = (Chemin::default(), Chemin::default());
    reprise.pris(true);
    dernier.pris(false);
    duree_boucle(b, entry, &Chemin::default(), 0xd04, &reprise, &dernier, cases)
}

const fn fixed(entry: u32, name: &'static str, cost: fn(&Cpu, &mut Cps2Bus) -> u64, run: fn(&mut Cpu, &mut Cps2Bus), outputs: u32) -> Native {
    Native { entry, name, cost, run, touches_sound: false, outputs, exact_cost: true }
}

pub const GAUGE_NATIVE: Native = fixed(0xb46, "video_jauge", gauge_cost, gauge, regs::d(2) | regs::a(0) | regs::CCR);
/// 0xb3e : `bra.b 0xb46`, appelé par la mise à jour du monde (D1 = marqueur de la classe).
pub const GAUGE_THUNK_NATIVE: Native = fixed(0xb3e, "video_jauge_b3e", gauge_cost, gauge, regs::d(2) | regs::a(0) | regs::CCR);
pub const GAUGE_VALUE_NATIVE: Native = fixed(0xb40, "video_jauge_d1", gauge_with_value_cost, gauge_with_value, regs::d(1) | regs::d(2) | regs::a(0) | regs::CCR);
pub const LOCK: Native = fixed(0xb96, "video_verrou", |_, b| ligne_droite(b, 0xb96), lock, regs::d(1));
pub const UNLOCK: Native = fixed(0xbac, "video_deverrou", |_, b| ligne_droite(b, 0xbac), unlock, regs::d(0) | regs::d(1));
pub const ENABLE_LAYERS: Native = fixed(0xbc6, "video_couches_on", enable_layers_cost, enable_layers, 0);
pub const DISABLE_LAYERS: Native = fixed(0xbfc, "video_couches_off", |_, b| ligne_droite(b, 0xbfc), disable_layers, regs::d(0));
pub const CLEAR_OBJECTS: Native = fixed(0xc58, "video_effacer_objets", |_, b| ligne_droite(b, 0xc58), clear_objects, 0);
pub const CLEAR_SCROLL1: Native = fixed(0xca8, "video_effacer_scroll1", |_, b| fill_cost(b, 0xca8, 0x4000 / 4), clear_scroll1, regs::d(0) | regs::d(1));
pub const CLEAR_SCROLL2: Native = fixed(0xcc8, "video_effacer_scroll2", |_, b| fill_cost(b, 0xcc8, 0x4000 / 4), clear_scroll2, regs::d(0) | regs::d(1));
pub const CLEAR_SCROLL3: Native = fixed(0xce8, "video_effacer_scroll3", |_, b| fill_cost(b, 0xce8, 0x4000 / 4), clear_scroll3, regs::d(0) | regs::d(1));


// ---------------------------------------------------------------------------------------------
// V2 : attribut du terrain sous un point
// ---------------------------------------------------------------------------------------------

/// Cartes de terrain fixes des scènes 2 et 0x14 (mots en ROM, colonnes de 0x20 octets).
const TERRAIN_MAP_STAGE_2: u32 = 0x3f8e9c;
const TERRAIN_MAP_STAGE_14: u32 = 0x3f9c9c;

/// Numéro de terrain sous le point (x, y) = (D0, D1) : bits 7 à 15 du mot d'attribut de la case.
/// Scènes 2 et 0x14 : carte fixe en ROM (y décalé de -/+ 0x10) ; ailleurs : case de la couche de
/// défilement 2 en GFX RAM, avec la correction de caméra quand la caméra 2 est courante.
/// Rend D2 (numéro), D0/D1 (coordonnées corrigées), A1 (adresse de la case) et Z (numéro nul).
fn terrain_attribute(v: &mut Vue, c: &mut Cpu) {
    let stage = v.rd8(a5(c, ram::STAGE));
    let (mut x, mut y) = (c.d[0] as u16, c.d[1] as u16);
    let fixed_map = if v.pris(stage == 2) {
        Some((TERRAIN_MAP_STAGE_2, 0x10u16.wrapping_neg()))
    } else if v.pris(stage == 0x14) {
        Some((TERRAIN_MAP_STAGE_14, 0x10))
    } else {
        None
    };
    let word = if let Some((map, dy)) = fixed_map {
        y = y.wrapping_add(dy);
        let row = 0x1e - ((y as u8 >> 3) & 0x1e) as u16;
        let column = x.wrapping_add(x) & 0xffe0;
        // restes du calcul (D3 = index dans la carte, D4 = colonne) : lus par des appelants qui les prennent pour arguments
        c.d[3] = row.wrapping_add(column) as u32;
        set_low_word(&mut c.d[4], column);
        c.a[1] = map.wrapping_add(row.wrapping_add(column) as i16 as u32);
        v.rd16(c.a[1])
    } else {
        let camera_2 = v.rd8(a5(c, ram::USE_CAMERA_2)) != 0;
        if !v.pris(!camera_2) {
            x = x.wrapping_add(v.rd16(a5(c, ram::SCROLL[0])).wrapping_sub(v.rd16(a5(c, ram::SCROLL_2[0]))));
            y = y.wrapping_add(v.rd16(a5(c, ram::SCROLL[1])).wrapping_sub(v.rd16(a5(c, ram::SCROLL_2[1]))));
        }
        let page = 0x3000u16.wrapping_sub((y & 0x300) << 4);
        let row = 0x3c - ((y as u8 >> 2) & 0x3c) as u16;
        let column = x.wrapping_shl(2) & 0xfc0;
        set_low_word(&mut c.d[3], page.wrapping_add(row).wrapping_add(column));
        set_low_word(&mut c.d[4], column);
        c.a[1] = hw::SCROLL2.wrapping_add(page.wrapping_add(row).wrapping_add(column) as i16 as u32);
        c.d[2] = 0;
        v.rd16(c.a[1] + 2)
    };
    let attribute = (word & 0xff80) >> 7;
    set_low_word(&mut c.d[0], x);
    set_low_word(&mut c.d[1], y);
    set_low_word(&mut c.d[2], attribute);
    c.sr = c.sr & !crate::ops::TOUS | if attribute == 0 { m68k::SR_Z } else { 0 };
}

/// 0x24b0 : numéro de terrain en (D0, D1).
fn terrain_native(c: &mut Cpu, b: &mut Cps2Bus) {
    terrain_attribute(&mut Vue::reelle(b), c);
    rts(c, b);
}

fn terrain_cost(c: &Cpu, b: &mut Cps2Bus) -> u64 {
    let mut v = Vue::simulee(b);
    terrain_attribute(&mut v, &mut c.clone());
    v.chemin.duree(b, 0x24b0)
}

/// 0x24a8 : numéro de terrain sous l'objet A0 (position x en +8, y en +0x10).
fn terrain_of_object(c: &mut Cpu, b: &mut Cps2Bus) {
    set_low_word(&mut c.d[0], rd16(b, c.a[0] + 8));
    set_low_word(&mut c.d[1], rd16(b, c.a[0] + 0x10));
    terrain_native(c, b);
}

fn terrain_of_object_cost(c: &Cpu, b: &mut Cps2Bus) -> u64 {
    let mut c2 = c.clone();
    set_low_word(&mut c2.d[0], rd16(b, c.a[0] + 8));
    set_low_word(&mut c2.d[1], rd16(b, c.a[0] + 0x10));
    let mut v = Vue::simulee(b);
    terrain_attribute(&mut v, &mut c2);
    v.chemin.duree(b, 0x24a8)
}

const TERRAIN_OUT: u32 = regs::d(0) | regs::d(1) | regs::d(2) | regs::d(3) | regs::d(4) | regs::a(1) | regs::CCR;

pub const TERRAIN: Native = fixed(0x24b0, "video_terrain", terrain_cost, terrain_native, TERRAIN_OUT);
pub const TERRAIN_OF_OBJECT: Native = fixed(0x24a8, "video_terrain_objet", terrain_of_object_cost, terrain_of_object, TERRAIN_OUT);

// ---------------------------------------------------------------------------------------------
// V2 : chargeurs de blocs de tuiles
// ---------------------------------------------------------------------------------------------

/// Géométrie d'un chargeur : D0 choisit une grille de `grid` x `grid` indices de blocs (mots en ROM) ;
/// chaque bloc est `cells` x `cells` cases, chaque case un long (tuile, attribut) copié depuis la ROM
/// avec `code_offset` ajouté à la tuile. A1 est la destination en GFX RAM.
struct Blocks {
    index_base: u32,
    /// Octets d'indices par valeur de D0.
    index_size: u32,
    grid: u32,
    cells: u32,
    /// Octets par ligne de cases dans la destination.
    row_stride: u32,
    code_offset: u16,
    /// Si présent, la destination revient au début de sa page (de cette taille) en fin de ligne de blocs.
    page: Option<u32>,
    source: Source,
}

enum Source {
    /// Une seule table de blocs de 16 cases.
    Fixed(u32),
    /// Plusieurs tables de blocs de `cells`² cases, choisies d'après la position de l'indice dans la
    /// ROM d'indices : sélecteur = `((position & mask) << shift)` échangé par moitiés (SWAP), mot
    /// signé ; adresse de la table à `list + sélecteur` (lue en espace programme si `program`, accès
    /// relatif au PC, sinon en espace données, accès par registre d'adresse).
    Banks { origin: u32, list: u32, mask: u32, shift: u32, program: bool },
}

const LOAD_4X4: Blocks = Blocks {
    index_base: 0x1f1b60,
    index_size: 0x80,
    grid: 8,
    cells: 4,
    row_stride: 0x80,
    code_offset: 0xf800,
    page: None,
    source: Source::Fixed(0x1f3b60),
};
const LOAD_SCROLL2: Blocks = Blocks {
    index_base: 0x1f9ba0,
    index_size: 0x80,
    grid: 8,
    cells: 2,
    row_stride: 0x40,
    code_offset: 0x1800,
    page: Some(0x1000),
    source: Source::Banks { origin: 0x1f9ba0, list: 0x275a, mask: 0x0fff_8000, shift: 3, program: false },
};
const LOAD_SMALL: Blocks = Blocks {
    index_base: 0x2459c0,
    index_size: 0x20,
    grid: 4,
    cells: 2,
    row_stride: 0x20,
    code_offset: 0x1f00,
    page: Some(0x800),
    source: Source::Banks { origin: 0x2459c0, list: 0x27e6, mask: 0xffff_e000, shift: 5, program: true },
};

fn prog_long(v: &mut Vue, a: u32) -> u32 {
    (v.prog16(a) as u32) << 16 | v.prog16(a + 2) as u32
}

impl Blocks {
    /// Adresse en ROM des cases du bloc dont l'indice est lu en `at`.
    fn block_source(&self, v: &mut Vue, at: u32) -> u32 {
        let index = v.rd16(at) as u32;
        match self.source {
            Source::Fixed(base) => base.wrapping_add(index << 6),
            Source::Banks { origin, list, mask, shift, program } => {
                let selector = ((at.wrapping_sub(origin) & mask) << shift).rotate_left(16) as u16 as i16;
                let at = list.wrapping_add(selector as u32);
                let table = if program { prog_long(v, at) } else { v.rd32(at) };
                table.wrapping_add(index << 4)
            }
        }
    }

    /// Charge la grille d'indices D0 en `dst`. Rend l'adresse finale de la destination et la fin des indices.
    fn load(&self, v: &mut Vue, d0: u16, mut dst: u32) -> (u32, u32) {
        let page_base = self.page.map_or(0, |p| dst & !(p - 1));
        let mut at = self.index_base.wrapping_add(d0 as u32 * self.index_size);
        let row_advance = (self.cells * self.row_stride).wrapping_sub(self.grid * self.cells * 4);
        for _ in 0..self.grid {
            for _ in 0..self.grid {
                let mut src = self.block_source(v, at);
                at += 2;
                for row in 0..self.cells {
                    for col in 0..self.cells {
                        let cell = v.rd32(src).wrapping_add((self.code_offset as u32) << 16);
                        src += 4;
                        v.wr32(dst + row * self.row_stride + col * 4, cell);
                    }
                }
                dst = dst.wrapping_add(self.cells * 4);
            }
            dst = dst.wrapping_add(row_advance);
            if let Some(p) = self.page {
                dst = (dst & (p - 1)) + page_base;
            }
        }
        (dst, at)
    }

    /// Décisions des boucles : `grid` lignes de `grid` blocs.
    fn path(&self) -> Chemin {
        let mut p = Chemin::default();
        for row in 0..self.grid {
            tours(&mut p, self.grid);
            p.pris(row + 1 < self.grid);
        }
        p
    }
}

fn load_blocks(c: &mut Cpu, b: &mut Cps2Bus, blocks: &Blocks) {
    let (dst, at) = blocks.load(&mut Vue::reelle(b), c.d[0] as u16, c.a[1]);
    c.d[0] = 0xffff;
    c.a[1] = dst;
    c.a[2] = at;
    rts(c, b);
}

const LOAD_OUT: u32 = regs::d(0) | regs::a(1) | regs::a(2);

pub const LOAD_BLOCKS_4X4: Native = fixed(0x260c, "video_blocs_4x4", |_, b| LOAD_4X4.path().duree(b, 0x260c), |c, b| load_blocks(c, b, &LOAD_4X4), LOAD_OUT);
pub const LOAD_BLOCKS_SCROLL2: Native = fixed(0x26d4, "video_blocs_scroll2", |_, b| LOAD_SCROLL2.path().duree(b, 0x26d4), |c, b| load_blocks(c, b, &LOAD_SCROLL2), LOAD_OUT);
pub const LOAD_BLOCKS_SMALL: Native = fixed(0x2766, "video_blocs_petits", |_, b| LOAD_SMALL.path().duree(b, 0x2766), |c, b| load_blocks(c, b, &LOAD_SMALL), LOAD_OUT);

// ---------------------------------------------------------------------------------------------
// V2 : chiffres dans la couche de caractères
// ---------------------------------------------------------------------------------------------

/// Écrit un chiffre (numéro de tuile = valeur, attribut 0) en `at` ; la case suivante de la ligne est
/// `COLUMN` plus loin dans la couche de caractères.
fn put_digit(b: &mut Cps2Bus, at: u32, digit: u16) {
    wr16(b, at, digit);
    wr16(b, at + 2, 0);
}

/// Colonne suivante de la couche de caractères (voir `text.rs`).
const COLUMN: u32 = 0x80;

/// 0x255a : D0 (0 à 99, saturé) en deux chiffres décimaux : dizaines en A1, unités une colonne plus loin.
/// Rend D0 = unités, D1 = dizaines, A1 = case des unités + 2.
fn decimal_digits(c: &mut Cpu, b: &mut Cps2Bus) {
    let mut value = c.d[0] as u16;
    if value >= 100 {
        value = 99;
    }
    let (tens, units) = (value / 10, value % 10);
    put_digit(b, c.a[1], tens);
    put_digit(b, c.a[1] + COLUMN, units);
    c.d[1] = tens as u32;
    set_low_word(&mut c.d[0], units);
    c.a[1] += COLUMN + 2;
    crate::ops::logique(c, 0, m68k::decode::Size::Word);
    rts(c, b);
}

fn decimal_digits_cost(c: &Cpu, b: &mut Cps2Bus) -> u64 {
    let value = (c.d[0] as u16).min(99);
    let mut p = Chemin::default();
    p.pris((c.d[0] as u16) < 100);
    p.repete(value as u32 / 10, false);
    p.pris(true);
    p.duree(b, 0x255a)
}

/// 0x25e8 : D0 en quatre chiffres hexadécimaux (quartet fort en premier), un par colonne à partir de A1.
/// Rend A1 = A1 + 4 colonnes.
fn hex_digits(c: &mut Cpu, b: &mut Cps2Bus) {
    for i in 0..4 {
        put_digit(b, c.a[1] + COLUMN * i, (c.d[0] as u16) >> (12 - 4 * i) & 0xf);
    }
    c.a[1] += 4 * COLUMN;
    crate::ops::logique(c, 0, m68k::decode::Size::Word);
    rts(c, b);
}

pub const DECIMAL_DIGITS: Native = fixed(0x255a, "video_chiffres_decimaux", decimal_digits_cost, decimal_digits, regs::d(0) | regs::d(1) | regs::a(1) | regs::CCR);
pub const HEX_DIGITS: Native = fixed(
    0x25e8,
    "video_chiffres_hexa",
    |_, b| {
        let mut p = Chemin::default();
        tours(&mut p, 4);
        p.duree(b, 0x25e8)
    },
    hex_digits,
    regs::a(1) | regs::CCR,
);

/// Routines natives de ce module.
pub const NATIVES: &[Native] = &[
    GAUGE_NATIVE,
    GAUGE_THUNK_NATIVE,
    GAUGE_VALUE_NATIVE,
    LOCK,
    UNLOCK,
    ENABLE_LAYERS,
    DISABLE_LAYERS,
    CLEAR_OBJECTS,
    CLEAR_SCROLL1,
    CLEAR_SCROLL2,
    CLEAR_SCROLL3,
    TERRAIN,
    TERRAIN_OF_OBJECT,
    LOAD_BLOCKS_4X4,
    LOAD_BLOCKS_SCROLL2,
    LOAD_BLOCKS_SMALL,
    DECIMAL_DIGITS,
    HEX_DIGITS,
];
