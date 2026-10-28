//! Type SET 3 (0xc8352, réutilisé par le type 33) : fenêtre de texte.
//!
//! Une fenêtre est décrite par un enregistrement de 18 octets de la ROM (table de pointeurs indexée
//! par la variante `+3`, une table par langue) : position dans la tilemap du scroll 1, taille, texte,
//! position du texte, *style* (`+0x2f`), durée d'affichage, paramètre, puis la fenêtre suivante.
//! L'objet dessine le cadre dans la tilemap (ouverture ligne à ligne), compose le texte dans le tampon
//! 0xff2072 selon le style (noms des joueurs, messages d'équipement...), le confie au moteur de texte
//! par une des entrées 0xff85c6 (pointeur, x, y), attend la durée ou un bouton, referme le cadre ligne à
//! ligne, puis enchaîne la fenêtre suivante ou disparaît.
//!
//! États : `+4` 0 (chargement), 2 (vie), 4/6 (libération) ; en vie, `+5` : 0 début d'ouverture,
//! 2 ouverture, 4 affichage, 6 fermeture.
//!
//! Le texte composé est une suite d'octets : caractère (> 15), commande (≤ 15) suivie de son argument,
//! 0xfe + argument, 0xff + nombre de caractères écrits depuis le précédent 0xff + argument, 0xfd fin.

use crate::families::cx::Banc;
use crate::objects::Courant;
use crate::families::cx::Cx;
use crate::mem::{prog32, rd16, rd32, rd8, wr16, wr32, wr8, A5};
use crate::objects::{pools, Obj};

/// Champs de l'objet.
mod f {
    pub const STATE: u32 = 0x04;
    pub const SUB: u32 = 0x05;
    pub const X: u32 = 0x08;
    pub const Y: u32 = 0x0c;
    /// Adresse dans la tilemap du coin courant (long) ; le mot bas avance d'une ligne (4 octets).
    pub const TILES: u32 = 0x14;
    pub const TILES_LOW: u32 = 0x16;
    /// Cadre « grand » (autres tuiles, hauteur 11 au lieu de 15).
    pub const BIG: u32 = 0x18;
    /// Nombre de lignes du cadre, et lignes restant à ouvrir ou fermer.
    pub const ROWS: u32 = 0x19;
    pub const ROWS_LEFT: u32 = 0x21;
    /// Négatif : dernière fenêtre ; sinon la fenêtre suivante (`NEXT`) est chargée à la fermeture.
    pub const CHAIN: u32 = 0x1a;
    /// Délai entre deux lignes, et sa valeur de recharge.
    pub const TICK: u32 = 0x1b;
    pub const TICK_RELOAD: u32 = 0x36;
    pub const NEXT: u32 = 0x1c;
    /// Largeur du cadre (en colonnes de 8 pixels, moins un ; 0xff : pas de cadre).
    pub const WIDTH: u32 = 0x20;
    pub const CLOSED: u32 = 0x22;
    pub const INDEX: u32 = 0x23;
    /// Texte (ou paramètre du style), long.
    pub const TEXT: u32 = 0x24;
    pub const STYLE: u32 = 0x2f;
    /// Durée d'affichage en images (0xffff : jusqu'à la fin imposée par un autre objet).
    pub const DURATION: u32 = 0x32;
    pub const PARAM: u32 = 0x34;
    /// Joueur dont les boutons écourtent l'affichage (négatif : tous).
    pub const PLAYER: u32 = 0x37;
    /// Entrée du moteur de texte : décalage (octet) depuis `TEXT_SLOTS`, puis adresse (mot).
    pub const SLOT: u32 = 0x38;
    pub const SLOT_PTR: u32 = 0x3a;
    pub const CHOICE: u32 = 0x42;
    pub const PLAYER_ALT: u32 = 0x43;
    /// Objet lié (mot d'adresse).
    pub const LINKED: u32 = 0x46;
}

/// Variables du jeu (A5+).
mod ram {
    pub const BUTTONS: u32 = 0x0c;
    pub const PLAYING: u32 = 0x30;
    pub const SCENE: u32 = 0x34;
    pub const PLAYER_COUNT: u32 = 0x35;
    pub const FROZEN: u32 = 0x42;
    pub const PAUSED: u32 = 0x50;
    pub const WINDOWS: u32 = 0x51;
    pub const FRAME_PARITY: u32 = 0x6f;
    /// Langue (mot) : 4 choisit la première table de fenêtres et de messages.
    pub const LANGUAGE: u32 = 0x72;
    pub const DIM: u32 = 0x48e;
    pub const DIM_ON: u32 = 0x48f;
    pub const TEXT_SLOTS: u32 = 0x5c6;
    pub const PLAYERS: u32 = 0x5de;
}

/// Adresses absolues de la RAM de travail.
const TEXT_BUFFER: u32 = 0xff2072;
const WINDOW_COUNT: u32 = 0xff2472;
const SKIP_FLAG: u32 = 0xff2042;
const SAVED_NAME: u32 = 0xff0fd2;
const PLAYER_SIZE: u32 = 0x200;
/// Champs d'un joueur : nom (6 caractères codés), équipement.
const NAME: u32 = 0x118;
const EQUIP_A: u32 = 0x10d;
const EQUIP_B: u32 = 0x10e;
const ITEMS: u32 = 0x140;

/// Tables de fenêtres (données), selon la langue.
const WINDOWS_LANG4: u32 = 0x1cd000;
const WINDOWS_OTHER: u32 = 0x1e78da;
/// Tilemap du scroll 1 et palettes de la fenêtre.
const TILEMAP: u32 = 0x912000;
const PALETTE_SRC: u32 = 0x292260;
const PALETTE_SRC_B: u32 = 0x292200;
const PALETTE: u32 = 0x91c1e0;
const PALETTE_ALT: u32 = 0x91c380;
const PALETTE_B: u32 = 0x91c300;
const PALETTE_C: u32 = 0x91c180;
/// Couleur dont la luminosité est recopiée quand l'écran est assombri.
const DIM_REFERENCE: u32 = 0x91c2a0;

/// Tables du code. Espace programme (lectures relatives au PC) :
const STYLE5_OFFSETS: u32 = 0xc851c;
const MESSAGES_LANG4: u32 = 0xc8e34;
const MESSAGES_OTHER: u32 = 0xc8e48;
/// Espace de données (lectures par registre d'adresse) :
const CLASS_TITLES: u32 = 0xc91c6;
const DEFAULT_NAMES: u32 = 0xc8948;
const GLYPHS: u32 = 0xc9216;
const SEPARATOR: u32 = 0xc91c4;
const EQUIP_LISTS: [(u32, u32); 2] = [(0xc8e0c, 0xc8e12), (0xc8e18, 0xc8e1e)];

/// Mots de tuile (numéro << 16 | attribut) du cadre.
const BLANK: u32 = 0x0020_0000;
const FLIP: u32 = 0x20;
const ROW_STRIDE: u32 = 0x80;

pub fn run(x: &mut Cx) {
    match x.u8(f::STATE) {
        0 => start(x),
        2 => match x.u8(f::SUB) {
            0 => open_start(x),
            2 => opening(x),
            4 => shown(x),
            6 => closing(x),
            _ => {}
        },
        _ => free(x),
    }
}

/// 0xc8368 : charge la fenêtre de la variante et pose sa palette.
fn start<R: Banc>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    x.set8(f::TICK_RELOAD, 3);
    let table = if x.a5_u16(ram::LANGUAGE) == 4 { WINDOWS_LANG4 } else { WINDOWS_OTHER };
    let variant = x.u8(0x3) as u32;
    let desc = rd32(x.b, table + 4 * variant);
    load(x, desc);
    let scene = x.a5_u8(ram::SCENE);
    let palette = if (0x45..0x5a).contains(&scene) { PALETTE_ALT } else { PALETTE };
    match x.u8(f::STYLE) {
        17 => {}
        21 => {
            if rd8(x.b, SKIP_FLAG) != 0 {
                x.set8(f::STYLE, 0);
            }
        }
        _ => {
            copy_palette(x, PALETTE_SRC, palette);
            dim_palette(x, palette);
        }
    }
}

/// 0xc83ea : écran assombri, la palette prend la luminosité de la couleur de référence. Rend
/// l'adresse qui suit la palette traitée (registre A1 de l'original).
fn dim_palette<R: Banc>(x: &mut Cx<'_, R>, palette: u32) -> u32 {
    if x.a5_u8(ram::DIM_ON) == 0 || x.a5_u8(ram::DIM) & 1 == 0 {
        return palette;
    }
    let level = rd32(x.b, DIM_REFERENCE) & 0xf000_f000;
    for i in 0..8 {
        let at = palette + 4 * i;
        let v = rd32(x.b, at) & 0x0fff_0fff | level;
        wr32(x.b, at, v);
    }
    palette + 32
}

/// Seize couleurs (`movem.l` de 8 longs).
fn copy_palette<R: Banc>(x: &mut Cx<'_, R>, src: u32, dst: u32) {
    for i in 0..8 {
        let v = rd32(x.b, src + 4 * i);
        wr32(x.b, dst + 4 * i, v);
    }
}

/// 0xc8a82 : seconde palette.
fn copy_palette_b<R: Banc>(x: &mut Cx<'_, R>) {
    copy_palette(x, PALETTE_SRC_B, PALETTE_B);
}

/// Fenêtre sans cadre : affichage direct (sous-état `sub`).
fn frameless<R: Banc>(x: &mut Cx<'_, R>, sub: u8) {
    x.set8(f::SUB, sub);
    x.set8(f::ROWS, 0);
    x.set8(f::ROWS_LEFT, 0);
    x.set8(f::WIDTH, 0xff);
    x.set8(f::TICK, 1);
}

/// Registres laissés par le chargement : A1, et D6 et A6 quand un style les écrase (palette copiée par
/// `movem`, recherche d'un joueur). La fermeture les compare à la largeur (D6) et à la place (A6) de
/// l'ancienne fenêtre, puis efface à partir de A6 sur la largeur D6.
struct Loaded {
    a1: u32,
    d6: Option<u32>,
    a6: Option<u32>,
}

/// 0xc8430 : charge l'enregistrement `desc` ; certains styles le remplacent par un autre (choisi
/// selon les joueurs, le nombre de joueurs...). Rend les registres de l'original que la fermeture
/// relit (voir `Loaded`).
fn load<R: Banc>(x: &mut Cx<'_, R>, mut desc: u32) -> Loaded {
    let mut regs = Loaded { a1: 0, d6: None, a6: None };
    let done = |regs: Loaded, a1: u32| Loaded { a1, ..regs };
    loop {
        let col = rd8(x.b, desc) as u16;
        let row = rd8(x.b, desc + 1) as u16;
        let v = rd8(x.b, desc + 2);
        x.set8(f::BIG, v);
        let v = rd8(x.b, desc + 3);
        x.set8(f::ROWS, v);
        let v = rd32(x.b, desc + 4);
        x.set32(f::TEXT, v);
        let v = rd16(x.b, desc + 8);
        x.set16(f::X, v);
        let v = rd16(x.b, desc + 10);
        x.set16(f::Y, v);
        let v = rd8(x.b, desc + 12);
        x.set8(f::STYLE, v);
        let v = rd8(x.b, desc + 13);
        x.set8(f::CHAIN, v);
        let v = rd16(x.b, desc + 14);
        x.set16(f::DURATION, v);
        let v = rd16(x.b, desc + 16);
        x.set16(f::PARAM, v);
        x.set32(f::NEXT, desc + 18);
        let offset = (col << 7).wrapping_add(row << 2) as i16 as i32 as u32;
        let tiles = TILEMAP.wrapping_add(offset);
        x.set32(f::TILES, tiles);
        let width = if x.u8(f::BIG) != 0 { 11 } else { 15 };
        x.set8(f::WIDTH, width);
        let rows = x.u8(f::ROWS);
        x.set8(f::ROWS_LEFT, rows);
        let text = x.u32(f::TEXT);
        match x.u8(f::STYLE) {
            // une fenêtre selon le nombre de joueurs
            1 => {
                let k = if x.a5_u8(ram::PLAYER_COUNT) == 1 { 0 } else { 4 };
                desc = rd32(x.b, text + k);
            }
            // cadre élargi selon le nombre de joueurs
            2 | 4 => {
                let extra = x.a5_u8(ram::PLAYER_COUNT).wrapping_sub(1).wrapping_mul(2);
                x.add8(f::ROWS, extra);
                let rows = x.u8(f::ROWS);
                x.set8(f::ROWS_LEFT, rows);
                return done(regs, tiles);
            }
            5 => {
                let i = x.u8(f::CHOICE).wrapping_mul(2).wrapping_add(x.a5_u8(ram::FRAME_PARITY) & 1);
                desc = text + x.rom8(STYLE5_OFFSETS + i as u32) as u32;
            }
            9 => {
                frameless(x, 2);
                x.set8(f::STYLE, 0);
                return done(regs, tiles);
            }
            // (la recherche d'un joueur de la classe 5 n'a pas d'effet, hormis A1)
            10 => return done(regs, find_player(x, |p| p == 5, false).map_or(A5 + ram::PLAYERS + 4 * PLAYER_SIZE, |p| p.0)),
            11 => {
                let k = x.u8(f::INDEX).wrapping_mul(4) as u32;
                desc = rd32(x.b, text + k);
            }
            13 => {
                copy_palette_b(x);
                regs.d6 = Some(rd32(x.b, PALETTE_SRC_B + 24));
                return done(regs, PALETTE_B + 0x20);
            }
            14 => {
                x.set8(f::TICK_RELOAD, 1);
                x.set8(f::STYLE, 0);
                return done(regs, tiles);
            }
            17 => {
                frameless(x, 2);
                copy_palette_b(x);
                x.set16(f::DURATION, 0xffff);
                copy_palette(x, PALETTE_SRC, PALETTE);
                for base in [PALETTE, PALETTE_B] {
                    for i in 0..8 {
                        let v = rd32(x.b, base + 4 * i) & 0x0fff_0fff;
                        wr32(x.b, base + 4 * i, v);
                    }
                }
                regs.d6 = Some(rd32(x.b, PALETTE_SRC + 24));
                return done(regs, PALETTE + 0x20);
            }
            19 => {
                x.set8(f::SLOT, 0x10);
                desc = text + 18 * x.u8(f::CHOICE) as u32;
            }
            // seconde fenêtre si un joueur de la classe 3 est en jeu
            20 => {
                desc = text;
                let found = find_player(x, |p| p == 3, false);
                regs.a6 = Some(found.map_or(A5 + ram::PLAYERS + 4 * PLAYER_SIZE, |p| p.0));
                if found.is_some() {
                    desc += 36;
                }
            }
            21 => {
                if rd8(x.b, SKIP_FLAG) != 0 {
                    frameless(x, 6);
                    return done(regs, tiles);
                }
                frameless(x, 2);
                copy_palette(x, PALETTE_SRC, PALETTE_C);
                regs.d6 = Some(rd32(x.b, PALETTE_SRC + 24));
                let a1 = dim_palette(x, PALETTE_C);
                return done(regs, a1);
            }
            22 => {
                x.set8(f::TICK_RELOAD, 1);
                x.set8(f::STYLE, 1);
                return done(regs, tiles);
            }
            23 => {
                frameless(x, 2);
                return done(regs, tiles);
            }
            _ => return done(regs, tiles),
        }
    }
}

/// Premier joueur (dans l'ordre des emplacements) présent, dont l'état `+4` est inférieur à 4 (ou
/// différent de 4 si `not_four`), et dont la classe (`+3 >> 1`) satisfait `class`.
fn find_player<R: Banc>(x: &mut Cx<'_, R>, class: impl Fn(u8) -> bool, not_four: bool) -> Option<Obj> {
    (0..4).map(|k| Obj(A5 + ram::PLAYERS + k * PLAYER_SIZE)).find(|&p| {
        let state = p.u8(x.b, 4);
        p.u8(x.b, 0) != 0 && (if not_four { state != 4 } else { (state as i8) < 4 }) && class(p.u8(x.b, 3) >> 1)
    })
}

// ---- cadre ----------------------------------------------------------------------------------------

fn big<R: Banc>(x: &mut Cx<'_, R>) -> bool {
    x.u8(f::BIG) != 0
}

/// Écrit `count` tuiles d'une ligne, de colonne en colonne, la tuile changeant de `step` à chaque fois.
fn strip<R: Banc>(x: &mut Cx<'_, R>, at: &mut u32, tile: &mut u32, step: u32, count: u32) {
    for _ in 0..count {
        wr32(x.b, *at, *tile);
        *tile = tile.wrapping_add(step);
        *at += ROW_STRIDE;
    }
}

/// Une ligne symétrique : `width + 1` tuiles montantes, puis les mêmes retournées, en descendant.
fn mirrored<R: Banc>(x: &mut Cx<'_, R>, mut at: u32, mut tile: u32) {
    let n = x.u8(f::WIDTH) as u32 + 1;
    strip(x, &mut at, &mut tile, 0x1_0000, n);
    tile = (tile | FLIP).wrapping_sub(0x1_0000);
    strip(x, &mut at, &mut tile, 0x1_0000u32.wrapping_neg(), n);
}

/// 0xc90ce : bord haut (deux lignes).
fn draw_top<R: Banc>(x: &mut Cx<'_, R>, at: u32) {
    let (mut a, mut b) = if big(x) { (0x0434_000f, 0x0444_000f) } else { (0x0400_000f, 0x0410_000f) };
    let n = x.u8(f::WIDTH) as u32 + 1;
    let mut at = at;
    for (step, last) in [(0x1_0000u32, false), (0x1_0000u32.wrapping_neg(), true)] {
        if last {
            a = (a | FLIP).wrapping_add(step);
            b = (b | FLIP).wrapping_add(step);
        }
        for _ in 0..n {
            wr32(x.b, at, a);
            wr32(x.b, at + 4, b);
            a = a.wrapping_add(step);
            b = b.wrapping_add(step);
            at += ROW_STRIDE;
        }
    }
}

/// 0xc9128 : ligne du milieu (bords gauche et droit, fond).
fn draw_middle<R: Banc>(x: &mut Cx<'_, R>, at: u32) {
    let edge = if big(x) { 0x0432_000f } else { 0x0430_000f };
    wr32(x.b, at, edge);
    let inner = (x.u8(f::WIDTH) as u16 * 2).wrapping_sub(1) as u32 + 1;
    let mut at = at + ROW_STRIDE;
    for _ in 0..inner {
        wr32(x.b, at, 0x0431_000f);
        at += ROW_STRIDE;
    }
    wr32(x.b, at, edge | FLIP);
}

/// 0xc9164 : bord bas.
fn draw_bottom<R: Banc>(x: &mut Cx<'_, R>, at: u32) {
    let tile = if big(x) { 0x0454_000f } else { 0x0420_000f };
    mirrored(x, at, tile);
}

/// 0xc91a8 : efface une ligne.
fn clear_row<R: Banc>(x: &mut Cx<'_, R>, mut at: u32) {
    let n = x.u8(f::WIDTH) as u32 * 2 + 2;
    for _ in 0..n {
        wr32(x.b, at, BLANK);
        at += ROW_STRIDE;
    }
}

/// 0xc9036 : efface trois lignes à partir de `at`, sur la largeur `width` (registre D6 : rien si son
/// octet est négatif, son mot donne la largeur).
fn clear_window<R: Banc>(x: &mut Cx<'_, R>, mut at: u32, width: u32) {
    if (width as u8 as i8) < 0 {
        return;
    }
    for _ in 0..(width as u16).wrapping_mul(2).wrapping_add(1) as u32 + 1 {
        for i in 0..3 {
            wr32(x.b, at + 4 * i, BLANK);
        }
        at += ROW_STRIDE;
    }
}

/// Délai entre deux lignes ; vrai quand il est écoulé (il est alors rechargé).
fn tick<R: Banc>(x: &mut Cx<'_, R>) -> bool {
    let t = x.u8(f::TICK);
    x.set8(f::TICK, t.wrapping_sub(1));
    // `subq.b #1 ; bgt` : strictement positif après décrément, débordement compris
    if t as i8 > 1 {
        return false;
    }
    let reload = x.u8(f::TICK_RELOAD);
    x.set8(f::TICK, reload);
    true
}

// ---- vie de la fenêtre --------------------------------------------------------------------------

/// 0xc870c : bord haut et première ligne.
fn open_start<R: Banc>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB, 2);
    let reload = x.u8(f::TICK_RELOAD);
    x.set8(f::TICK, reload);
    let at = x.u32(f::TILES);
    draw_top(x, at);
    x.add16(f::TILES_LOW, 8);
    let at = x.u32(f::TILES);
    draw_bottom(x, at);
}

/// 0xc872a : une ligne de plus à chaque délai ; à la fin, le texte.
fn opening(x: &mut Cx) {
    if !tick(x) {
        return;
    }
    let left = x.u8(f::ROWS_LEFT);
    if left == 0 {
        return show_text(x);
    }
    x.set8(f::ROWS_LEFT, left - 1);
    let at = x.u32(f::TILES);
    draw_middle(x, at);
    x.add16(f::TILES_LOW, 4);
    let at = x.u32(f::TILES);
    draw_bottom(x, at);
}

/// Entrée du moteur de texte de la fenêtre (adresse sur 16 bits).
fn slot<R: Banc>(x: &mut Cx<'_, R>) -> u32 {
    x.u16(f::SLOT_PTR) as i16 as i32 as u32
}

/// 0xc875a : cadre ouvert ; confie le texte au moteur de texte, composé selon le style.
fn show_text(x: &mut Cx) {
    x.add8(f::SUB, 2);
    let rows = x.u8(f::ROWS);
    x.set8(f::ROWS_LEFT, rows);
    let slot = A5 + ram::TEXT_SLOTS + x.u8(f::SLOT) as u32;
    x.set16(f::SLOT_PTR, slot as u16);
    let (px, py) = (x.u16(f::X), x.u16(f::Y));
    wr16(x.b, slot + 4, px);
    wr16(x.b, slot + 6, py);
    let style = x.u8(f::STYLE);
    if style == 0 {
        let text = x.u32(f::TEXT);
        wr32(x.b, slot, text);
        return;
    }
    if style != 12 {
        wr32(x.b, slot, TEXT_BUFFER);
    }
    let mut t = Text { at: TEXT_BUFFER, count: 0 };
    compose(x, &mut t, style, slot);
}

/// 0xc8f76 : affichage pendant la durée (un bouton l'écourte), puis efface le texte.
fn shown<R: Banc>(x: &mut Cx<'_, R>) {
    if x.u16(f::DURATION) == 0xffff {
        return;
    }
    if buttons(x) != 0 {
        x.set16(f::DURATION, 0);
    }
    let left = x.u16(f::DURATION);
    if left != 0 {
        x.set16(f::DURATION, left - 1);
        return;
    }
    x.add8(f::SUB, 2);
    if x.u8(f::STYLE) == 12 {
        return;
    }
    let slot = slot(x);
    wr32(x.b, slot, 0);
}

/// 0xc9068 : boutons nouvellement pressés (quartet haut) des joueurs en jeu concernés.
fn buttons<R: Banc>(x: &mut Cx<'_, R>) -> u8 {
    let filter = if x.u8(0x3) == 0x8b {
        x.u8(f::PLAYER_ALT)
    } else {
        let p = x.u8(f::PLAYER);
        if (p as i8) >= 0 { p } else { 0xff }
    };
    if x.a5_u8(ram::PLAYING) == 0 {
        return 0;
    }
    let mut pressed = 0;
    for k in 0..4 {
        let p = Obj(A5 + ram::PLAYERS + k * PLAYER_SIZE);
        if p.u8(x.b, 0) != 0 && (p.u8(x.b, 4) as i8) < 4 && ((filter as i8) < 0 || filter == p.u8(x.b, 2)) {
            pressed |= x.a5_u8(ram::BUTTONS + k);
        }
    }
    pressed & 0xf0
}

/// 0xc8fb0 : efface une ligne à chaque délai ; à la fin, fenêtre suivante ou disparition.
fn closing<R: Banc>(x: &mut Cx<'_, R>) {
    if !tick(x) {
        return;
    }
    let left = x.u8(f::ROWS_LEFT);
    if left == 0 {
        return closed(x);
    }
    x.set8(f::ROWS_LEFT, left - 1);
    let at = x.u32(f::TILES);
    clear_row(x, at);
    x.add16(f::TILES_LOW, 0xfffc);
    let at = x.u32(f::TILES);
    draw_bottom(x, at);
}

/// 0xc8fe0 : efface le bord haut, sauf si la fenêtre suivante a la même place et la même largeur.
fn closed<R: Banc>(x: &mut Cx<'_, R>) {
    let top = x.u32(f::TILES).wrapping_sub(8);
    let width = x.u8(f::WIDTH);
    if (x.u8(f::CHAIN) as i8) < 0 {
        x.set8(f::CLOSED, 1);
        x.set32(f::STATE, 0x0600_0000);
        return clear_window(x, top, width as u32);
    }
    x.set8(f::SUB, 0);
    let next = x.u32(f::NEXT);
    let regs = load(x, next);
    let (d6, a6) = (regs.d6.unwrap_or(width as u32), regs.a6.unwrap_or(top));
    if d6 as u8 != x.u8(f::WIDTH) || a6 != regs.a1 {
        clear_window(x, a6, d6);
    }
}

/// 0xc9056 : libération.
fn free<R: Banc>(x: &mut Cx<'_, R>) {
    let n = x.a5_u8(ram::WINDOWS).wrapping_sub(1);
    x.set_a5_u8(ram::WINDOWS, n);
    x.set8(f::CLOSED, 0);
    let n = rd8(x.b, WINDOW_COUNT).wrapping_sub(1);
    wr8(x.b, WINDOW_COUNT, n);
    x.free(&pools::SET);
}

// ---- composition du texte -----------------------------------------------------------------------

/// Écriture dans le tampon (registre A4 de l'original) et nombre de caractères écrits (D6).
struct Text {
    at: u32,
    count: u16,
}

impl Text {
    fn byte(&mut self, x: &mut Cx, v: u8) {
        wr8(x.b, self.at, v);
        self.at += 1;
    }
    fn word(&mut self, x: &mut Cx, v: u16) {
        wr16(x.b, self.at, v);
        self.at += 2;
    }
    /// Octet final, sans avancer.
    fn end(&mut self, x: &mut Cx, v: u8) {
        wr8(x.b, self.at, v);
    }
    fn char(&mut self) {
        self.count = self.count.wrapping_add(1);
    }
    /// Caractère (> 15) ou commande et son argument, lus en `src`.
    fn plain(&mut self, x: &mut Cx, c: u8, src: &mut u32) {
        self.byte(x, c);
        if c <= 15 {
            let arg = rd8(x.b, *src);
            *src += 1;
            self.byte(x, arg);
        }
        self.char();
    }
    /// 0xc8cda : 0xfe + argument, ou 0xff + compte + argument (l'octet qui suit 0xff est sauté).
    fn escape(&mut self, x: &mut Cx, c: u8, src: &mut u32) {
        self.byte(x, c);
        if c != 0xfe {
            let n = self.count as u8;
            self.byte(x, n);
            *src += 1;
            self.count = 0;
        }
        let arg = rd8(x.b, *src);
        *src += 1;
        self.byte(x, arg);
    }
    /// 0xc88d8 : 0xff, compte, 0x10.
    fn counted(&mut self, x: &mut Cx) {
        self.byte(x, 0xff);
        let n = self.count as u8;
        self.byte(x, n);
        self.byte(x, 0x10);
    }
}

/// Façon d'écrire un glyphe de commande (≤ 15) : son octet et le suivant, ou deux fois le même
/// (0xc88e4, variante de l'original).
#[derive(Clone, Copy, PartialEq)]
enum Glyph {
    Pair,
    Twice,
}

/// Façon de reconnaître un nom par défaut au premier octet du nom.
#[derive(Clone, Copy)]
enum Defaults {
    /// 0xf9 à 0xfe : nom par défaut n° `-octet - 3`.
    High,
    /// ≥ 130 : nom par défaut selon l'octet (136, 144, 130, 160, autre).
    Codes,
}

/// Nom de 6 caractères codés (0x51 = premier glyphe, 0xff fin) en `src`, ou nom par défaut, écrit
/// avec les glyphes de la table 0xc9216.
fn name(x: &mut Cx, t: &mut Text, mut src: u32, defaults: Defaults, glyph: Glyph) {
    let first = rd8(x.b, src);
    let default = match defaults {
        Defaults::High if first.wrapping_add(1) >= 250 => Some(first.wrapping_neg().wrapping_sub(3).wrapping_mul(4)),
        Defaults::Codes if first >= 130 => Some(match first {
            136 => 0,
            144 => 4,
            130 => 8,
            160 => 12,
            _ => 16,
        }),
        _ => None,
    };
    let mut n: u16 = 5;
    if let Some(index) = default {
        let table = rd32(x.b, DEFAULT_NAMES + index as u32);
        n = rd16(x.b, table);
        src = table + 2;
    }
    loop {
        let c = rd8(x.b, src);
        src += 1;
        if c == 0xff {
            return;
        }
        let index = c.wrapping_sub(0x51) as u32 * 2;
        let mut g = GLYPHS.wrapping_add(rd16(x.b, GLYPHS + index) as i16 as i32 as u32);
        let v = rd8(x.b, g);
        if v <= 15 {
            t.byte(x, v);
            if glyph == Glyph::Pair {
                g += 1;
            }
        }
        let v = rd8(x.b, g);
        t.byte(x, v);
        t.char();
        n = n.wrapping_sub(1);
        if n == 0xffff {
            return;
        }
    }
}

/// Copie d'un texte jusqu'à 0xfd : `special(c, src)` traite d'abord un octet (vrai s'il l'a fait).
fn copy_until_end(x: &mut Cx, t: &mut Text, mut src: u32, special: &mut dyn FnMut(&mut Cx, &mut Text, u8, &mut u32) -> bool) -> u32 {
    loop {
        let c = rd8(x.b, src);
        src += 1;
        if c == 0xfd {
            t.end(x, c);
            return src;
        }
        if special(x, t, c, &mut src) {
            continue;
        }
        if c >= 0xfe {
            t.escape(x, c, &mut src);
        } else {
            t.plain(x, c, &mut src);
        }
    }
}

/// 0xc87de : le texte tel quel.
fn copy_text(x: &mut Cx, t: &mut Text) {
    let src = x.u32(f::TEXT);
    copy_until_end(x, t, src, &mut |_, _, _, _| false);
}

/// Code 7 suivi de `arg` : insertion (nom...) à la place.
fn is_insert<R: Banc>(x: &mut Cx<'_, R>, c: u8, src: u32, arg: u8) -> bool {
    c == 7 && rd8(x.b, src) == arg
}

fn compose(x: &mut Cx, t: &mut Text, style: u8, slot: u32) {
    match style {
        1 | 5 | 9 | 11 | 14 | 19 | 20 | 22 => copy_text(x, t),
        // 0xc8816 : titre de classe et nom de chaque joueur présent
        2 => {
            copy_palette_b(x);
            for k in 0..4 {
                let p = Obj(A5 + ram::PLAYERS + k * PLAYER_SIZE);
                if p.u8(x.b, 0) != 0 && p.u8(x.b, 4) != 4 {
                    t.count = 0;
                    title_and_name(x, t, p);
                }
            }
            t.end(x, 0xfd);
        }
        3 => linked_effect(x, t),
        // 0xc8ac0 : noms des joueurs en jeu, séparés
        4 => {
            copy_palette_b(x);
            let mut left = x.a5_u8(ram::PLAYER_COUNT);
            for k in 0..4 {
                let p = Obj(A5 + ram::PLAYERS + k * PLAYER_SIZE);
                if p.u8(x.b, 0) == 0 || (p.u8(x.b, 4) as i8) >= 4 {
                    continue;
                }
                t.count = 0;
                prefixed_name(x, t, p.0 + NAME);
                left = left.wrapping_sub(1);
                if left != 0 {
                    let sep = rd8(x.b, SEPARATOR);
                    t.byte(x, sep);
                    t.char();
                    t.counted(x);
                }
            }
            let sep = rd8(x.b, SEPARATOR);
            t.byte(x, sep);
            t.end(x, 0xfd);
        }
        6 | 7 => words(x, t, slot),
        // 0xc8b58 : caractères écrits sur un mot (le code 0x7003 de l'original ne peut pas se présenter)
        8 => {
            let mut src = x.u32(f::TEXT);
            loop {
                let c = rd8(x.b, src);
                src += 1;
                if c == 0xfd {
                    return t.end(x, c);
                }
                if c.wrapping_add(3) & 0x80 != 0 {
                    t.escape(x, c, &mut src);
                } else {
                    t.word(x, c as u16);
                    t.char();
                }
            }
        }
        // 0xc8b9e : texte avec le nom du joueur de la classe 5, sinon second texte tel quel
        10 => {
            let text = x.u32(f::TEXT);
            let Some(p) = find_player(x, |c| c == 5, false) else {
                let v = rd32(x.b, text);
                let slot = self::slot(x);
                return wr32(x.b, slot, v);
            };
            let src = rd32(x.b, text + 4);
            copy_until_end(x, t, src, &mut |x, t, c, src| {
                if !is_insert(x, c, *src, 4) {
                    return false;
                }
                *src += 1;
                name(x, t, p.0 + NAME, Defaults::High, Glyph::Pair);
                true
            });
        }
        12 => x.set16(f::DURATION, 0xffff),
        // 0xc8c86 : texte avec le nom enregistré, encadré
        13 => {
            let src = x.u32(f::TEXT);
            copy_until_end(x, t, src, &mut |x, t, c, src| {
                if !is_insert(x, c, *src, 5) {
                    return false;
                }
                *src += 1;
                t.byte(x, 0xfe);
                t.byte(x, 0x18);
                name(x, t, SAVED_NAME, Defaults::Codes, Glyph::Pair);
                t.byte(x, 0xfe);
                t.byte(x, 0x0f);
                true
            });
        }
        // 0xc8cf6 : texte selon le nombre de joueurs, puis comme le style 3
        15 => {
            let table = x.u32(f::TEXT);
            let k = if x.a5_u8(ram::PLAYER_COUNT) == 1 { 0 } else { 4 };
            let v = rd32(x.b, table + k);
            x.set32(f::TEXT, v);
            linked_effect(x, t);
        }
        // 0xc8d12 : texte tel quel, sans composition
        16 => {
            x.set8(f::INDEX, 1);
            let text = x.u32(f::TEXT);
            let slot = self::slot(x);
            wr32(x.b, slot, text);
        }
        17 => linked_message(x, t),
        // 0xc8ece : texte tel quel et un objet SOBJ 6 (variante 19) lié
        18 => {
            copy_text(x, t);
            if let Some(o) = spawn(x, &pools::SOBJ, 6, 19) {
                o.set_u16(x.b, f::LINKED, x.o.0 as u16);
                let v = x.u8(f::PARAM);
                o.set_u8(x.b, 0x2f, v);
            }
        }
        // 0xc8eea : texte précédé d'une commande de couleur, avec le nom enregistré
        21 => {
            t.byte(x, 0xfe);
            t.byte(x, 0x0c);
            if x.a5_u8(ram::PAUSED) != 0 && x.a5_u8(ram::FROZEN) != 0 {
                t.at -= 4;
                copy_palette(x, PALETTE_SRC, PALETTE);
            }
            t.count = 0;
            let mut src = x.u32(f::TEXT);
            loop {
                let c = rd8(x.b, src);
                src += 1;
                match c {
                    0xfd => return t.end(x, c),
                    0xff => t.escape(x, c, &mut src),
                    7 if rd8(x.b, src) == 5 => {
                        src += 1;
                        name(x, t, SAVED_NAME, Defaults::Codes, Glyph::Pair);
                    }
                    _ => t.plain(x, c, &mut src),
                }
            }
        }
        23 => wr32(x.b, slot, 0),
        _ => {}
    }
}

/// 0xc8844 : titre de la classe du joueur, son nom encadré, et le compte.
fn title_and_name(x: &mut Cx, t: &mut Text, p: Obj) {
    let class = (p.u8(x.b, 3) >> 1).wrapping_mul(4);
    let mut src = rd32(x.b, CLASS_TITLES + class as u32);
    loop {
        let c = rd8(x.b, src);
        if c == 0xfd {
            break;
        }
        src += 1;
        t.byte(x, c);
        if c <= 15 {
            let arg = rd8(x.b, src);
            src += 1;
            t.byte(x, arg);
        }
        t.char();
    }
    t.byte(x, 0xfe);
    t.byte(x, 0x18);
    name(x, t, p.0 + NAME, Defaults::High, Glyph::Pair);
    t.byte(x, 0xfe);
    t.byte(x, 0x0f);
    t.counted(x);
}

/// 0xc88e8 : nom précédé de la commande 0xfe 0x18.
fn prefixed_name(x: &mut Cx, t: &mut Text, src: u32) {
    t.byte(x, 0xfe);
    t.byte(x, 0x18);
    name(x, t, src, Defaults::High, Glyph::Twice);
}

/// Crée un objet dans `pool` : actif, type `kind`, variante `variant`.
fn spawn<R: Banc>(x: &mut Cx<'_, R>, pool: &pools::Pool, kind: u8, variant: u8) -> Option<Obj> {
    let o = pool.alloc(x.b)?;
    o.set_u8(x.b, Obj::ACTIVE, 1);
    o.set_u8(x.b, Obj::KIND, kind);
    o.set_u8(x.b, Obj::VARIANT, variant);
    Some(o)
}

/// 0xc8a9c : un effet EFFCT 23 (réglé par `PARAM`) lié à la fenêtre, qui la fermera ; texte tel quel.
/// Sans place, on réessaie à l'image suivante.
fn linked_effect(x: &mut Cx, t: &mut Text) {
    let setting = x.u8(f::PARAM);
    let Some(o) = spawn(x, &pools::EFFCT, 23, 0) else {
        return x.add8(f::SUB, 0xfe);
    };
    o.set_u8(x.b, 0x2f, setting);
    o.set_u16(x.b, 0x50, x.o.0 as u16);
    x.set16(f::DURATION, 0xffff);
    copy_text(x, t);
}

/// 0xc8b08 : texte en mots ; 0x7001 et 0x7002 insèrent la liste de mots désignée par l'entrée du
/// moteur de texte selon le nombre de joueurs ; un mot négatif est une commande 0xfe/0xff.
fn words(x: &mut Cx, t: &mut Text, slot: u32) {
    let mut src = x.u32(f::TEXT);
    let mut list = slot;
    loop {
        let w = rd16(x.b, src);
        src += 2;
        if w == 0 {
            return wr16(x.b, t.at, 0);
        }
        if w & 0x8000 != 0 {
            t.escape(x, w as u8, &mut src);
        } else if w == 0x7001 || w == 0x7002 {
            let k = x.a5_u8(ram::PLAYER_COUNT).wrapping_sub(1) as u32 * 4;
            list = rd32(x.b, list + k);
            loop {
                let v = rd16(x.b, list);
                if v == 0 {
                    break;
                }
                list += 2;
                t.word(x, v);
                t.char();
            }
        } else {
            t.word(x, w);
            t.char();
        }
    }
}

/// 0xc8d22 : texte qui parle de l'objet lié (`LINKED`) : code 7 suivi de 6 à 9 (nom, objet
/// d'équipement) ; à la fin, un SET 15 (variante 17) lié, réglé par `PARAM`. Sans place, on
/// recommence à l'image suivante.
fn linked_message(x: &mut Cx, t: &mut Text) {
    let src = x.u32(f::TEXT);
    copy_until_end(x, t, src, &mut |x, t, c, src| {
        if c != 7 || (rd8(x.b, *src) as i8) < 6 {
            return false;
        }
        let k = rd8(x.b, *src);
        *src += 1;
        let linked = x.linked(f::LINKED);
        match k {
            6 => prefixed_name(x, t, linked.0 + NAME),
            7 | 8 => equipment_message(x, t, linked),
            9 => {
                if x.u16(f::LINKED) != 0 {
                    prefixed_name(x, t, linked.0 + NAME);
                }
            }
            _ => {}
        }
        true
    });
    let Some(o) = spawn(x, &pools::SET, 15, 17) else {
        return x.add8(f::SUB, 0xfe);
    };
    o.set_u16(x.b, f::LINKED, x.o.0 as u16);
    let v = x.u16(f::PARAM);
    o.set_u16(x.b, f::CHOICE, v);
}

/// 0xc8d88 : message de la première pièce d'équipement du joueur `p` reconnue dans les listes de sa
/// classe (deux emplacements d'arme, puis six objets).
fn equipment_message(x: &mut Cx, t: &mut Text, p: Obj) {
    let (main, items) = EQUIP_LISTS[(p.u8(x.b, 3) >> 1 != 0) as usize];
    let candidates = [(main, p.u8(x.b, EQUIP_A)), (main, p.u8(x.b, EQUIP_B))]
        .into_iter()
        .chain((0..6).map(|i| (items, p.u8(x.b, ITEMS + i))));
    for (list, item) in candidates.collect::<Vec<_>>() {
        if let Some(i) = find_in_list(x, list, item) {
            return message(x, t, i);
        }
    }
}

/// 0xc8e24 : rang de `item` dans la liste d'octets `list` (terminée par un octet négatif).
fn find_in_list<R: Banc>(x: &mut Cx<'_, R>, list: u32, item: u8) -> Option<u32> {
    (0..).map_while(|i| {
        let c = rd8(x.b, list + i);
        ((c as i8) >= 0).then_some((i, c))
    })
    .find(|&(_, c)| c == item)
    .map(|(i, _)| i)
}

/// 0xc8e5c : message n° `i` (deux tables selon la langue), jusqu'à 0xfd non compris.
fn message(x: &mut Cx, t: &mut Text, i: u32) {
    let table = if x.a5_u16(ram::LANGUAGE) == 4 { MESSAGES_LANG4 } else { MESSAGES_OTHER };
    let mut src = prog32(x.b, table + 4 * i);
    loop {
        let c = rd8(x.b, src);
        src += 1;
        if c == 0xfd {
            return;
        }
        t.plain(x, c, &mut src);
    }
}
