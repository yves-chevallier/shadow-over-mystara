//! Type SET 15 (0xce024) : les mises en scène de fin de niveau et d'écrans de transition (h).
//!
//! Le type est un *aiguilleur* : la variante `+3` (0 à 24) choisit une petite machine à états
//! indépendante (`+4` état principal, `+5` sous-état) dont l'adresse est dans la table 0xce034 de
//! la ROM. Ces machines animent l'écran entre deux niveaux (secousses de caméra, défilements de
//! décor chargés par 0x26d4 / 0x2766, fondus de palette, apparitions d'objets liés par `+0x4c` à un
//! objet « chef d'orchestre »). Elles ne sont jamais appelées dans les scénarios (voir
//! `docs/familles/set_c.md`). Les variantes pas encore réécrites sont exécutées par le code d'origine.

use crate::families::cx::Cx;
use crate::mem::wr16;
use crate::objects::Obj;
use m68k::SR_C;
use crate::objects::pools;
use crate::objects::Courant;
use crate::machine::Machine;

/// Table (espace programme) des déplacements des machines, un mot relatif par variante.
const VARIANTS: u32 = 25;

/// Routines du moteur appelées.
mod rom {
    /// Chargeur de blocs 8x8 du scroll 2 / 3 (D0 = grille, A1 = destination).
    pub const LOAD_BLOCKS: u32 = 0x26d4;
    /// Chargeur de blocs 4x4.
    pub const LOAD_SMALL: u32 = 0x2766;
    /// Crée un objet EFFCT de type D0.
    pub const SPAWN_EFFCT: u32 = 0xb1a7a;
    pub const SPAWN_SOBJ: u32 = 0xddeaa;
}

/// Variables de la RAM de travail (offsets depuis A5).
mod a5 {
    /// Caméra du fond : position (X, Y) et sa copie « précédente ».
    pub const CAM_X: u32 = 0x506;
    pub const CAM_Y: u32 = 0x50a;
    pub const CAM_X_PREV: u32 = 0x50e;
    pub const CAM_Y_PREV: u32 = 0x512;
    /// Second plan (scroll 2) : position et copie.
    pub const BG_X: u32 = 0x556;
    pub const BG_Y: u32 = 0x55a;
    pub const BG_X_PREV: u32 = 0x55e;
    pub const BG_Y_PREV: u32 = 0x562;
}

pub fn run(x: &mut Cx) {
    let variant = x.u8(0x3) as u32;
    if variant >= VARIANTS {
        return;
    }
    // Les tables d'états de toutes les variantes (sauf 19 et 20, sans table) finissent pareil :
    // état 4 = passer à 6 (0xce0e8) ou, pour 0 et 1, libérer ; état 6 = libérer (0xce0e2) ; pour 24
    // l'état 4 est propre à la variante.
    if !matches!(variant, 19 | 20) {
        match (variant, x.u8(0x4)) {
            (0 | 1, 4 | 6) | (_, 6) => return free(x),
            (24, 4) => {}
            (_, 4) => return x.set8(0x4, 6),
            _ => {}
        }
    }
    match variant {
        0 => title_blink(x),
        1 => quake(x),
        2 => backdrop_slide(x),
        3 => camera_script(x),
        4 => scene_scroll(x),
        5 => block_loader(x),
        6 => scene_scroll_follow(x),
        7 => status::status_window(x),
        8 => later::hp_refill_a(x),
        9 => later::spawn_player_effects(x),
        10 => later::shaking_intro(x),
        11 => later::handover_effects(x),
        12 => later::spawn_player_markers(x),
        13 => later::fireworks(x),
        14 => later::lightning(x),
        15 => later::quake_light(x),
        16 => later::ranking_start(x),
        17 => later::palette_fade(x),
        18 => later::bonus_selection(x),
        19 => later::countdown_digits(x),
        20 => later::credits_scroll(x),
        21 => later::flicker(x),
        22 => later::name_entry(x),
        23 => later::hp_refill_b(x),
        _ => later::results_panel(x),
    }
}

// --- aides ------------------------------------------------------------------------------------

fn next_state(x: &mut Cx) {
    x.add8(0x4, 2);
}

fn next_substate(x: &mut Cx) {
    x.add8(0x5, 2);
}

/// Passe l'objet à l'état 6 (libération au passage suivant).
fn free(x: &mut Cx) {
    x.free(&pools::SET);
}

/// Le chef d'orchestre : l'objet pointé par `+0x4c` (adresse longue).
fn conductor(x: &mut Cx) -> Obj {
    Obj(x.u32(0x4c))
}

fn rng(x: &mut Cx) -> u16 {
    x.random()
}

/// Charge la grille de blocs `grid` à `dest` par 0x26d4 ou 0x2766 (D0 = grille, A1 = destination).
fn load_blocks(x: &mut Cx, routine: u32, dest: u32, grid: u16) {
    x.c.a[1] = dest;
    load_next(x, routine, grid);
}

/// Idem sans repositionner A1 (le chargeur le laisse après les blocs écrits).
fn load_next(x: &mut Cx, routine: u32, grid: u16) {
    // `move.w (A4)+,D0w` : seul le mot bas de D0 change (le chargeur laisse D0 en sortie)
    x.c.d[0] = x.c.d[0] & 0xffff_0000 | grid as u32;
    x.call(routine);
}

/// Secousse : ajoute à la caméra le déplacement n° (`+0x18` & 3) de la table `table` (couples de mots).
fn shake(x: &mut Cx, table: u32) {
    let step = (x.u8(0x18) & 3) as u32;
    x.add8(0x18, 1);
    let (dx, dy) = (x.data16(table + 4 * step), x.data16(table + 4 * step + 2));
    // l'original laisse les décalages dans D1 et D2 (la création d'effet qui suit lit D1)
    x.c.d[1] = x.c.d[1] & 0xffff_0000 | dx as u32;
    x.c.d[2] = x.c.d[2] & 0xffff_0000 | dy as u32;
    let (cx, cy) = (x.a5_u16(a5::CAM_X), x.a5_u16(a5::CAM_Y));
    x.set_a5_u16(a5::CAM_X, cx.wrapping_add(dx));
    x.set_a5_u16(a5::CAM_Y, cy.wrapping_add(dy));
}

/// Avance un champ de 32 bits d'un mot de vitesse 8.8 (`vitesse << 8` signée).
fn velocity(speed: u16) -> u32 {
    (((speed as u32) << 16) as i32 >> 8) as u32
}

// --- variante 0 : clignotement de titre ------------------------------------------------------------

/// Variante 0 (0xce066) : initialise deux routines (0x3525c, 0x1ac86), puis, de temps à autre (rng
/// et parité de l'image), bascule `+0x43` et lance l'une de deux routines sonores/vidéo.
fn title_blink(x: &mut Cx) {
    const INIT_A: u32 = 0x3525c;
    const ON: u32 = 0xea22c;
    const OFF: u32 = 0xea1d0;
    const FLASH: u32 = 0x4ba;
    match x.u8(0x4) {
        0 => {
            next_state(x);
            x.call(INIT_A);
            x.call(crate::palette::FLASH_COLOR_WHITE.entry);
            x.set8(0x42, 0);
            x.set_a5_u16(FLASH, 0);
        }
        2 => {
            if x.u8(0x42) != 0 {
                x.add8(0x42, 0xff);
                if x.u8(0x42) == 0 {
                    x.set_a5_u16(FLASH, 0);
                }
                return;
            }
            if x.a5_u8(0x6d) & 3 != 0 || rng(x) & 7 != 0 {
                return;
            }
            x.set8(0x42, 3);
            x.set_a5_u16(FLASH, 0x100);
            let was_on = x.bit(0x43, 0);
            x.set_bit(0x43, 0, !was_on);
            crate::sound::jouer_global(x.b, if was_on { ON } else { OFF });
        }
        _ => free(x),
    }
}

// --- variante 1 : secousse de fin de niveau --------------------------------------------------------

/// Variante 1 (0xce0f0) : place l'objet au bas de l'écran, attend, puis fait trembler la caméra avec
/// des tables de secousses (0xd0b10, 0xd0b20) en lançant des effets ; le chef d'orchestre (`+0x4c`)
/// est prévenu par son octet `+8` et par `+0xc`.
fn quake(x: &mut Cx) {
    const SHAKE_A: u32 = 0xd0b10;
    const SHAKE_B: u32 = 0xd0b20;
    const DELAYS: u32 = 0xce138;
    match x.u8(0x4) {
        0 => {
            next_state(x);
            x.set8(0x14, 0x28);
            let v = x.a5_u16(a5::CAM_X).wrapping_add(0xc0);
            x.set16(0x8, v);
            let v = x.a5_u16(a5::CAM_Y).wrapping_add(0x80);
            x.set16(0xc, v);
            quake_delay(x, DELAYS);
        }
        2 => match x.u8(0x5) {
            0 => {
                x.add8(0x14, 0xff);
                if x.u8(0x14) != 0 {
                    return;
                }
                next_substate(x);
                x.set8(0x14, 0x3c);
                x.set8(0x18, 0);
            }
            2 => {
                shake(x, SHAKE_A);
                x.add8(0x14, 0xff);
                if x.u8(0x14) != 0 {
                    return;
                }
                x.set8(0x14, 0x78);
                next_substate(x);
            }
            4 => {
                shake(x, SHAKE_B);
                quake_effects(x, DELAYS);
                x.add8(0x14, 0xff);
                if x.u8(0x14) != 0 {
                    return;
                }
                next_substate(x);
                let boss = conductor(x);
                let v = boss.u8(x.b, 0xc).wrapping_sub(1);
                boss.set_u8(x.b, 0xc, v);
            }
            6 => {
                shake(x, SHAKE_B);
                let boss = conductor(x);
                if boss.u8(x.b, 0x8) != 2 {
                    return;
                }
                next_substate(x);
                x.add16(0xc, 0u16.wrapping_sub(0x290));
            }
            8 => {
                shake(x, SHAKE_B);
                quake_effects(x, DELAYS);
                let boss = conductor(x);
                if boss.u8(x.b, 0x8) != 3 {
                    return;
                }
                next_state(x);
                x.set_a5_u16(a5::CAM_X, 0);
                x.set_a5_u16(a5::CAM_Y, 0x70);
            }
            _ => {}
        },
        _ => free(x),
    }
}

/// Délai aléatoire avant le prochain effet : octet de la table 0xce138 tiré par le rng.
fn quake_delay(x: &mut Cx, table: u32) {
    let r = rng(x) & 7;
    let d = x.rom8(table + r as u32);
    x.set8(0x42, d);
}

/// Crée un effet (type 0 de 0xb1a7a) lié à l'objet quand le délai `+0x42` est écoulé.
fn quake_effects(x: &mut Cx, table: u32) {
    x.add8(0x42, 0xff);
    if x.u8(0x42) != 0 {
        return;
    }
    x.set8(0x42, 1);
    x.c.d[0] = 0;
    x.call(rom::SPAWN_EFFCT);
    if x.c.sr & SR_C != 0 {
        return;
    }
    let child = Obj(x.c.a[1]);
    child.set_u16(x.b, 0x42, x.o.0 as u16);
    let owner = x.u32(0x4c);
    child.set_u32(x.b, 0x4c, owner);
    quake_delay(x, table);
}

// --- variante 2 : fond qui glisse ---------------------------------------------------------------

/// Variante 2 (0xce294) : un fond (scroll 3, GFX 0x920000) dont la position `+0x34` accélère ;
/// `+5` = 0 : il entre ; 2 : il sort. Écrit sa position dans les 176 mots du bloc quand elle change.
fn backdrop_slide(x: &mut Cx) {
    const TILEMAP: u32 = 0x920000;
    match x.u8(0x4) {
        0 => {
            next_state(x);
            x.set32(0x8, 0);
            x.set32(0x34, 0);
        }
        2 => {
            let leaving = x.u8(0x5) != 0;
            x.add32(0x34, if leaving { 0x4000 } else { 0x3800 });
            let at = x.a5_u16(a5::CAM_X).wrapping_add(x.u16(0x34));
            if leaving {
                if at >= 0xa0 {
                    x.set8(0x4, 6);
                    return;
                }
            } else if at >= 0x60 {
                return next_substate(x);
            }
            let pos = x.u16(0x34);
            if pos == x.u16(0x8) {
                return;
            }
            x.set16(0x8, pos);
            for i in 0..0xb0 {
                wr16(x.b, TILEMAP + 2 * i, pos);
            }
        }
        _ => {}
    }
}

// --- variante 3 : le décor se place d'après le chef d'orchestre -----------------------------------

/// Variante 3 (0xce324) : lit dans une table (0xce37a, 16 octets) la position de caméra, la vitesse
/// et le but selon `+0x3f` du chef d'orchestre, charge deux grilles de blocs dans le scroll 3
/// (0x917000), puis fait glisser la caméra vers le but ; le chef d'orchestre est alors décrémenté.
fn camera_script(x: &mut Cx) {
    const TABLE: u32 = 0xce37a;
    match x.u8(0x4) {
        0 => {
            next_state(x);
            let boss = conductor(x);
            let row = TABLE + ((boss.u8(x.b, 0x3f) as u32) << 4);
            let w = |x: &mut Cx, i: u32| x.data16(row + 2 * i);
            let (cam_x, cam_y) = (w(x, 0), w(x, 1));
            x.set_a5_u16(a5::CAM_X, cam_x);
            x.set_a5_u16(a5::CAM_Y, cam_y);
            for (field, i) in [(0x14, 2), (0x18, 3), (0x8, 4), (0xc, 5)] {
                let v = w(x, i);
                x.set16(field, v);
            }
            let (g0, g1) = (w(x, 6), w(x, 7));
            x.c.d[0] = 0;
            load_blocks(x, rom::LOAD_BLOCKS, 0x917000, g0);
            load_next(x, rom::LOAD_BLOCKS, g1);
        }
        2 => {
            let (vx, vy) = (velocity(x.u16(0x14)), velocity(x.u16(0x18)));
            let v = x.a5_u32(a5::CAM_X).wrapping_add(vx);
            x.set_a5_u32(a5::CAM_X, v);
            let v = x.a5_u32(a5::CAM_Y).wrapping_add(vy);
            x.set_a5_u32(a5::CAM_Y, v);
            if x.u16(0x8) != x.a5_u16(a5::CAM_X) || x.u16(0xc) != x.a5_u16(a5::CAM_Y) {
                return;
            }
            next_state(x);
            let boss = conductor(x);
            let v = boss.u8(x.b, 0x8).wrapping_sub(1);
            boss.set_u8(x.b, 0x8, v);
        }
        _ => {}
    }
}

// --- variantes 4 et 6 : défilement d'une scène --------------------------------------------------------

/// Paramètres de scène (sous-routine 0xce4a2, table de pointeurs 0xce4ea indexée par `index`
/// octets) : position et vitesses du fond, puis une liste de blocs 4x4 chargés dans 0x918000.
fn load_scene(x: &mut Cx, index: u32) {
    const SCENES: u32 = 0xce4ea;
    let at = SCENES.wrapping_add(index);
    let mut p = (x.rom16(at) as u32) << 16 | x.rom16(at + 2) as u32;
    x.c.d[0] = 0;
    let mut next = |x: &mut Cx| {
        let w = x.data16(p);
        p += 2;
        w
    };
    for field in [0x8, 0xc, 0x14, 0x18, 0x1c, 0x1e] {
        let w = next(x);
        x.set16(field, w);
    }
    let (px, py) = (x.u16(0x8), x.u16(0xc));
    x.set_a5_u16(a5::BG_X, px);
    x.set_a5_u16(a5::BG_Y, py);
    let count = next(x);
    let w = next(x);
    x.set16(0x16, w);
    for _ in 0..=count {
        let offset = next(x) as i16 as i32 as u32;
        let grid = next(x);
        load_blocks(x, rom::LOAD_SMALL, 0x918000u32.wrapping_add(offset), grid);
    }
}

/// Un pas de défilement du fond : position += vitesse (8.8), masquée par `+0x1c` / `+0x1e`, puis
/// publiée dans le second plan avec le décalage `+0x16`.
fn scroll_step(x: &mut Cx) {
    let v = x.u32(0x8).wrapping_add(velocity(x.u16(0x14)));
    x.set32(0x8, v);
    let v = x.u32(0xc).wrapping_add(velocity(x.u16(0x18)));
    x.set32(0xc, v);
    let mask = x.u16(0x1c);
    let v = x.u16(0x8) & mask;
    x.set16(0x8, v);
    let bg_x = x.u16(0x8).wrapping_add(x.u16(0x16));
    x.set_a5_u16(a5::BG_X, bg_x);
    let mask = x.u16(0x1e);
    let v = x.u16(0xc) & mask;
    x.set16(0xc, v);
    let bg_y = x.u16(0xc);
    x.set_a5_u16(a5::BG_Y, bg_y);
}

/// Variante 4 (0xce47c) : charge la scène `+0x2f` (bit 0 ignoré), puis la fait défiler sans fin.
fn scene_scroll(x: &mut Cx) {
    match x.u8(0x4) {
        0 => {
            next_state(x);
            let index = (x.u8(0x2f) as u32 & !1) * 4;
            load_scene(x, index & 0xffff);
        }
        2 => scroll_step(x),
        _ => {}
    }
}

fn publish_background(x: &mut Cx) {
    let (px, py) = (x.a5_u16(a5::BG_X), x.a5_u16(a5::BG_Y));
    x.set_a5_u16(a5::BG_X_PREV, px);
    x.set_a5_u16(a5::BG_Y_PREV, py);
}

/// Variante 6 (0xce6bc) : comme 4 (scène `+0x2f`, bit 0 compris), en recopiant le fond dans sa copie
/// « précédente », jusqu'à ce que le chef d'orchestre passe à la variante 2.
fn scene_scroll_follow(x: &mut Cx) {
    match x.u8(0x4) {
        0 => {
            next_state(x);
            let index = (x.u8(0x2f) as u32) * 4;
            load_scene(x, index & 0xffff);
            publish_background(x);
        }
        2 => {
            scroll_step(x);
            publish_background(x);
            if conductor(x).u8(x.b, 3) == 2 {
                next_state(x);
            }
        }
        _ => {}
    }
}

// --- variante 5 : chargeur de blocs ---------------------------------------------------------------------

/// Variante 5 (0xce64a) : charge deux grilles de blocs (0xce686 + 4 × `+0x2f`) dans le scroll 3.
fn block_loader(x: &mut Cx) {
    match x.u8(0x4) {
        0 => {
            next_state(x);
            let index = x.u8(0x2f) as u32;
            block_loader_for(x, index);
        }
        2 => next_state(x),
        _ => {}
    }
}

/// Charge les deux grilles de blocs n° `index` (couples de mots à 0xce686) dans le scroll 3.
fn block_loader_for(x: &mut Cx, index: u32) {
    const GRIDS: u32 = 0xce686;
    let at = GRIDS + 4 * index;
    let (g0, g1) = (x.data16(at), x.data16(at + 2));
    x.c.d[0] = 0;
    load_blocks(x, rom::LOAD_BLOCKS, 0x917000, g0);
    load_next(x, rom::LOAD_BLOCKS, g1);
}

#[path = "set_c_finale_b.rs"]
mod later;
#[path = "set_c_finale_c.rs"]
mod status;
