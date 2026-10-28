//! Variante 7 du type SET 15 (0xce714) : la fenêtre de dialogue de fin de niveau (h).
//!
//! Elle s'ouvre pour le joueur désigné par `+0x34` (adresse RAM sur 16 bits) : choisit dans les
//! tables de la ROM le message qui correspond à son personnage (`+3 >> 1`), à son niveau d'avancement
//! (`+0x10c` du joueur, plafonné) et à la langue (`A5+0x72`), applique le bonus que le message décrit
//! (statistiques `+0x151..` du joueur, équipement `+0x10d`, `+0x10e`, `+0x140..` puis recalcul par
//! 0x3552), dessine le cadre de la fenêtre dans la tilemap et compose le texte dans le tampon
//! 0xff2072 (nom du joueur, puis message) que le moteur de texte affiche ensuite. L'état 2 recompose
//! la suite du message page par page (`+0x10` : délai entre deux pages).

use crate::families::cx::Banc;
use super::*;
use crate::mem::{rd8, wr16, wr32, wr8, A5};

/// Tables en ROM (espace programme : accès relatifs au PC).
const LEVEL_RANGE: u32 = 0xce728;
const FRAME_BY_LEVEL: u32 = 0xce7ac;
const FRAMES: u32 = 0xce7e4;
const MESSAGE_TABLES: u32 = 0xceaa2;
/// Tables en ROM (espace de données).
const HEADER_PAGES: u32 = 0xce9a4;
const GLYPHS_JP: u32 = 0xd7e8a;
const GLYPHS_LATIN: u32 = 0xc9216;
const PALETTE_SOURCES: [(u32, u32, u32); 2] = [(0x292260, 0x91c1e0, 1), (0x292200, 0x91c300, 2)];

/// Tampon de texte composé et ses variables (A5+).
const TEXT_BUFFER: u32 = 0xff2072;
const TEXT_POINTER: u32 = 0x5c6;
const TEXT_WIDTH: u32 = 0x5ca;
const BOX_TOP: u32 = 0x5cc;
/// Langue (2 : japonais) et numéro de page de message.
const LANGUAGE: u32 = 0x73;
const MESSAGE_PAGE: u32 = 0x72;

pub fn status_window(x: &mut Cx) {
    match x.u8(0x4) {
        0 => {
            next_state(x);
            open(x);
        }
        2 => {
            x.add8(0x10, 0xff);
            if x.u8(0x10) != 0 {
                return;
            }
            if x.u8(0x5) != 0 {
                return next_state(x);
            }
            x.set8(0x10, 0x1e);
            compose_message(x, 0);
        }
        _ => {}
    }
}

fn next_state<R: Banc>(x: &mut Cx<'_, R>) {
    x.add8(0x4, 2);
}

/// État 0 : calcule le palier du joueur, crée l'objet de fenêtre, dessine le cadre et affiche le
/// premier message.
fn open(x: &mut Cx) {
    let player = adr(x.u16(0x34));
    let half = rd8(x.b, player + 3) >> 1;
    x.set8(0x37, half);
    let at = LEVEL_RANGE + 2 * half as u32;
    let (min, max) = (x.rom8(at) as i8, x.rom8(at + 1) as i8);
    let mut level = rd8(x.b, player + 0x10c).wrapping_add(1) as i8;
    if level > max {
        level = max;
    }
    wr8(x.b, player + 0x10c, level as u8);
    let step = level.wrapping_sub(min) as u8;
    x.set8(0x36, step);
    let base = x.rom16(FRAME_BY_LEVEL + 2 * half as u32);
    let low = (base as u8).wrapping_add(step);
    let frame = x.rom8(FRAME_BY_LEVEL + (base & 0xff00 | low as u16) as i16 as i32 as u32);
    x.set_a5_u16(BOX_TOP, 0xb8u16.wrapping_sub(0x18 * frame as u16));
    x.c.d[0] = 0xd;
    x.call(0xddeaa);
    if x.c.sr & m68k::SR_C == 0 {
        Obj(x.c.a[1]).set_u8(x.b, 0x2f, frame);
    }
    draw_frame(x, frame);
    pick_message(x, player);
}

/// Adresse RAM stockée sur 16 bits (`movea.w` : extension de signe).
fn adr(v: u16) -> u32 {
    v as i16 as i32 as u32
}

/// Cadre de la fenêtre n° `frame` (0xce7fc) : enregistrement de 8 octets (adresse de GFX RAM, largeur)
/// dans la table 0xce7e4 ; un bord haut, deux colonnes latérales (42 lignes de 0x80 octets) et un fond.
fn draw_frame(x: &mut Cx, frame: u8) {
    let entry = FRAMES + 8 * frame as u32;
    let origin = (x.rom16(entry) as u32) << 16 | x.rom16(entry + 2) as u32;
    let width = x.rom16(entry + 4);
    let longs = width as u32 + 1;
    let row = |x: &mut Cx, at: u32, value: u32, count: u32, stride: u32| {
        for i in 0..count {
            wr32(x.b, at + i * stride, value);
        }
    };
    wr16(x.b, origin, 0x145);
    wr16(x.b, origin + 2, 2);
    let mut at = origin + 4;
    row(x, at, 0x14b_0002, longs, 4);
    at += 4 * longs;
    wr32(x.b, at, 0x147_0002);
    row(x, at + 0x80, 0x14a_0002, 42, 0x80);
    row(x, origin + 0x80, 0x149_0002, 42, 0x80);
    let at = origin + 0x80 + 0x80 * 42;
    wr32(x.b, at, 0x146_0002);
    row(x, at + 4, 0x14c_0002, longs, 4);
    wr32(x.b, at + 4 + 4 * longs, 0x148_0002);
    let mut column = origin + 0x80;
    for _ in 0..longs {
        row(x, column + 4, 0x431_000f, 42, 0x80);
        column += 4;
    }
}

/// Choisit le message (0xceaba) : table d'enregistrements par personnage, rang de la page, palier.
fn pick_message(x: &mut Cx, player: u32) {
    let half = x.u8(0x37) as u32;
    let tables = {
        let at = MESSAGE_TABLES + 4 * half;
        (x.rom16(at) as u32) << 16 | x.rom16(at + 2) as u32
    };
    let page = (x.a5_u16(MESSAGE_PAGE).wrapping_sub(2)).wrapping_mul(2);
    let messages = x.data32(tables.wrapping_add(page as i16 as i32 as u32));
    let step = x.u8(0x36) as u16;
    let record = tables + 0xc + step.wrapping_mul(20) as i16 as i32 as u32;
    let hp = x.data16(record);
    wr16(x.b, player + 0x60, hp);
    let choice = if rd8(x.b, player + 3) & 1 != 0 { x.data16(record + 4) } else { x.data16(record + 2) };
    let pointer = x.data32(messages.wrapping_add(choice as i16 as i32 as u32));
    x.set32(0x8, pointer);
    let next = merge_statistics(x, player, record + 6);
    equip(x, player, next);
    let count = compose_header(x, player);
    x.set8(0x10, 0x1e);
    compose_message(x, count);
}

/// 0xceb20 : prend le maximum, octet par octet, de neuf valeurs de la ROM et des statistiques
/// `+0x151..` du joueur (sauf si l'enregistrement commence par un octet négatif). Rend la suite.
fn merge_statistics<R: Banc>(x: &mut Cx<'_, R>, player: u32, at: u32) -> u32 {
    let skip = x.data8(at) as i8 <= -1;
    if !skip {
        for i in 0..9 {
            let v = x.data8(at + 1 + i) as i8;
            let current = rd8(x.b, player + 0x151 + i) as i8;
            if v > current {
                wr8(x.b, player + 0x151 + i, v as u8);
            }
        }
    }
    at + 10
}

/// 0xceb6c : change une pièce d'équipement du joueur. L'enregistrement donne (nouvelle valeur,
/// ancienne valeur) pour `+0x10d` ou, sinon, pour la case de `+0x140..` qui porte l'ancienne valeur ;
/// puis 0x3552 recalcule les caractéristiques du joueur.
fn equip(x: &mut Cx, player: u32, at: u32) {
    if x.data8(at) as i8 <= -1 {
        return;
    }
    let (new, old) = (x.data8(at), x.data8(at + 1));
    let kind = rd8(x.b, player + 0x10e).wrapping_sub(4);
    if kind <= 3 || old != rd8(x.b, player + 0x10d) {
        let (new, old) = (x.data8(at + 2), x.data8(at + 3));
        let mut slot = player + 0x140;
        loop {
            let found = rd8(x.b, slot) == old;
            slot += 1;
            if found {
                break;
            }
        }
        wr8(x.b, slot - 1, new);
    } else {
        wr8(x.b, player + 0x10d, new);
    }
    // le recalcul travaille sur le joueur (A0)
    x.c.a[0] = player;
    x.c.a[5] = A5;
    crate::call(x.c, x.b, 0x3552);
    x.c.a[0] = x.o.0;
}

/// Palette de la fenêtre et en-tête du texte (0xce88a) : recopie trois blocs de palette, fixe le
/// tampon de texte et y compose le nom du joueur. Rend le nombre de caractères écrits.
fn compose_header<R: Banc>(x: &mut Cx<'_, R>, player: u32) -> u16 {
    for (from, to, blocks) in PALETTE_SOURCES {
        for i in 0..8 * blocks {
            let v = x.data32(from + 4 * i);
            wr32(x.b, to + 4 * i, v);
        }
    }
    x.set_a5_u32(TEXT_POINTER, TEXT_BUFFER);
    x.set_a5_u16(TEXT_WIDTH, 0x60);
    let (end, count) = write_name(x, player, TEXT_BUFFER);
    x.set32(0xc, end);
    count
}

/// Nom du joueur (0xce8de) : codes de glyphes de l'enregistrement `+0x118` du joueur, six au plus ;
/// un code >= 0xf9 renvoie à une liste de la table 0xce9a4. Rend (fin du tampon, nombre de glyphes).
fn write_name<R: Banc>(x: &mut Cx<'_, R>, player: u32, buffer: u32) -> (u32, u16) {
    let japanese = x.a5_u8(LANGUAGE) == 2;
    let mut src = player + 0x118;
    let mut out = buffer;
    let mut count = 0u16;
    let mut left = 5i32;
    if japanese {
        wr32(x.b, out, 0xfffe_0018);
        out += 4;
    } else {
        wr8(x.b, out, 0xfe);
        wr8(x.b, out + 1, 0x18);
        out += 2;
    }
    // un code >= 0xf9 remplace la source par une liste de la table (ses codes puis 0xff)
    let first = rd8(x.b, src).wrapping_add(1);
    if first >= 0xfa {
        let index = if japanese {
            (first.wrapping_neg().wrapping_sub(2)).wrapping_mul(4)
        } else {
            (rd8(x.b, src).wrapping_neg().wrapping_sub(3)).wrapping_mul(4)
        };
        src = x.data32(HEADER_PAGES + index as u32);
        left = x.data16(src) as i32;
        src += 2;
    }
    while left >= 0 {
        left -= 1;
        let code = x.data8(src);
        src += 1;
        if code == 0xff {
            break;
        }
        if japanese {
            let glyph = x.data16(GLYPHS_JP + 2 * code as u32);
            wr16(x.b, out, glyph);
            out += 2;
        } else {
            let index = code.wrapping_sub(0x51) as u32;
            let table = GLYPHS_LATIN;
            let at = table.wrapping_add(x.data16(table + 2 * index) as i16 as i32 as u32);
            let mut p = at;
            if x.data8(p) <= 0xf {
                let byte = x.data8(p);
                wr8(x.b, out, byte);
                out += 1;
                p += 1;
            }
            let byte = x.data8(p);
            wr8(x.b, out, byte);
            out += 1;
        }
        count = count.wrapping_add(1);
    }
    if japanese {
        wr32(x.b, out, 0xfffe_000f);
        out += 4;
    } else {
        wr8(x.b, out, 0xfe);
        wr8(x.b, out + 1, 0x0f);
        out += 2;
    }
    (out, count)
}

/// Composition de la suite du message (0xce9ea) : recopie dans le tampon (`+0xc`) les caractères de la
/// source (`+8`) jusqu'à une marque de fin de page ; `count` = glyphes déjà écrits sur la ligne.
fn compose_message<R: Banc>(x: &mut Cx<'_, R>, mut count: u16) {
    let mut src = x.u32(0x8);
    let mut out = x.u32(0xc);
    if x.a5_u8(LANGUAGE) == 2 {
        loop {
            let w = x.data16(src);
            src += 2;
            if (w as i16) < 0 {
                if w == 0xfffe {
                    wr16(x.b, out, w);
                    let next = x.data16(src);
                    wr16(x.b, out + 2, next);
                    out += 4;
                    src += 2;
                    continue;
                }
                wr16(x.b, out, w);
                wr16(x.b, out + 2, count.wrapping_shl(3).wrapping_neg());
                out += 4;
                src += 2;
                let tail = x.data16(src);
                src += 2;
                wr16(x.b, out, tail);
                out += 2;
                x.set32(0x8, src);
                x.set32(0xc, out);
                wr16(x.b, out, 0);
                return;
            }
            if w == 0 {
                wr16(x.b, out, 0);
                next_substate(x);
                x.set16(0x10, 0xb4);
                return;
            }
            wr16(x.b, out, w);
            out += 2;
            count = count.wrapping_add(1);
        }
    } else {
        loop {
            let c = x.data8(src);
            src += 1;
            if c >= 0xfe {
                if c == 0xfe {
                    wr8(x.b, out, c);
                    let next = x.data8(src);
                    src += 1;
                    wr8(x.b, out + 1, next);
                    out += 2;
                    continue;
                }
                wr8(x.b, out, c);
                wr8(x.b, out + 1, count as u8);
                out += 2;
                src += 1;
                let tail = x.data8(src);
                src += 1;
                wr8(x.b, out, tail);
                out += 1;
                x.set32(0x8, src);
                x.set32(0xc, out);
                wr8(x.b, out, 0xfd);
                return;
            }
            if c == 0xfd {
                wr8(x.b, out, c);
                next_substate(x);
                x.set16(0x10, 0xb4);
                return;
            }
            if c > 0xf {
                wr8(x.b, out, c);
                out += 1;
            } else {
                wr8(x.b, out, c);
                let next = x.data8(src);
                src += 1;
                wr8(x.b, out + 1, next);
                out += 2;
            }
            count = count.wrapping_add(1);
        }
    }
}

fn next_substate<R: Banc>(x: &mut Cx<'_, R>) {
    x.add8(0x5, 2);
}
