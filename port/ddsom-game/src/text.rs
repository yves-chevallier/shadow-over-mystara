//! Textes et effacements sur la couche de caractères 8x8 (tilemap 0x912000).
//!
//! La couche est rangée par colonnes : une case = 2 mots (tuile, attributs), la case suivante d'une
//! même colonne est 4 octets plus loin, la colonne suivante 0x80 octets plus loin.
//! Un texte est décrit en ROM par : colonne, ligne, attributs, puis les caractères et des codes de
//! contrôle. Toutes ces routines rendent `A4` positionné après le descripteur (les appelants
//! enchaînent les textes) et préservent les autres registres.

use cps2::Cps2Bus;
use m68k::{Cpu, SR_C, SR_N, SR_V, SR_X, SR_Z};

use crate::mem::{rd16, rd8, rts, wr16};
use crate::{regs, Native};

const LAYER: u32 = 0x912000;
const ROW: u32 = 4;
const COLUMN: u32 = 0x80;
const BLANK: u16 = 0x20;

/// Lecteur séquentiel du descripteur pointé par A4.
struct Script<'a> {
    b: &'a mut Cps2Bus,
    at: u32,
}

impl Script<'_> {
    fn byte(&mut self) -> u8 {
        let v = rd8(self.b, self.at);
        self.at += 1;
        v
    }
    fn word(&mut self) -> u16 {
        let v = rd16(self.b, self.at);
        self.at += 2;
        v
    }
}

/// Case (colonne, ligne) de la couche ; positions signées, avec le calcul 16 bits de l'original
/// (ext.l puis décalage du mot bas seulement). Renvoie aussi la retenue de l'addition finale
/// (drapeau X laissé par l'original).
fn cell_signed(col: u8, row: u8) -> (u32, bool) {
    let shift = |v: u8, n: u32| {
        let x = v as i8 as i32 as u32;
        (x & 0xffff_0000) | ((x << n) & 0xffff)
    };
    let (sum, carry) = shift(col, 7).overflowing_add(shift(row, 2));
    (LAYER.wrapping_add(sum) & 0xff_ffff, carry)
}

fn cell(col: u8, row: u8) -> u32 {
    LAYER + COLUMN * col as u32 + ROW * row as u32
}

fn put(b: &mut Cps2Bus, at: u32, code: u16, attr: u16) {
    wr16(b, at, code);
    wr16(b, at + 2, attr);
}

/// Fin de routine : A4 avancé, drapeaux de la comparaison finale (égalité) et X laissé par la
/// dernière opération qui le modifie.
fn finish(c: &mut Cpu, b: &mut Cps2Bus, a4: u32, x: bool) {
    c.a[4] = a4;
    c.sr = (c.sr & !(SR_X | SR_N | SR_V | SR_C)) | SR_Z | if x { SR_X } else { 0 };
    rts(c, b);
}

/// Les trois variantes du texte simple.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Sorte {
    /// 0x0d5e : '@' fin, '/' ligne suivante, 0xff n : n espaces.
    Texte,
    /// 0x0d32 : mêmes descripteurs, chaque caractère devient un espace (les attributs sont sans effet).
    Effacement,
    /// 0x0dca : comme le texte, mais '_' passe à la ligne et les espaces laissent le fond intact.
    Surimpression,
}

/// Rend la fin du descripteur et le drapeau X que laisse l'original.
fn texte(b: &mut Cps2Bus, descripteur: u32, sorte: Sorte) -> (u32, bool) {
    let mut s = Script { b, at: descripteur };
    let (col, row) = (s.byte(), s.byte());
    let attr = s.word();
    let (mut line, mut x) = cell_signed(col, row);
    let mut at = line;
    let a_la_ligne = if sorte == Sorte::Surimpression { b'_' } else { b'/' };
    loop {
        match s.byte() {
            b'@' => break,
            0xff => {
                let n = s.byte();
                x = n == 0; // subq.b #1
                for _ in 0..n.wrapping_sub(1) as u16 + 1 {
                    put(s.b, at, BLANK, 0);
                    at += COLUMN;
                }
            }
            ch if ch == a_la_ligne => {
                line += ROW;
                at = line;
            }
            b' ' if sorte == Sorte::Surimpression => at += COLUMN,
            ch => {
                let (code, attr) = if sorte == Sorte::Effacement { (BLANK, 0) } else { (ch as u16, attr) };
                put(s.b, at, code, attr);
                at += COLUMN;
            }
        }
    }
    (s.at, x)
}

/// 0x0d5e : écrit le texte décrit en `descripteur` (colonne, ligne, attributs, caractères) ; rend l'adresse qui
/// suit le descripteur (A4 de l'original : les appelants enchaînent les textes).
pub fn print(b: &mut Cps2Bus, descripteur: u32) -> u32 {
    texte(b, descripteur, Sorte::Texte).0
}

/// 0x0d32 : efface le texte décrit en `descripteur` ; rend la fin du descripteur.
pub fn erase(b: &mut Cps2Bus, descripteur: u32) -> u32 {
    texte(b, descripteur, Sorte::Effacement).0
}

/// 0x0dca : texte en surimpression ; rend la fin du descripteur.
pub fn print_over(b: &mut Cps2Bus, descripteur: u32) -> u32 {
    texte(b, descripteur, Sorte::Surimpression).0
}

fn texte_native(c: &mut Cpu, b: &mut Cps2Bus, sorte: Sorte) {
    let (end, x) = texte(b, c.a[4], sorte);
    finish(c, b, end, x);
}

/// Corps commun des textes hauts (2 tuiles l'une sous l'autre, codes sur un mot) :
/// '@@' fin, '##' colonne sautée, '/@' ligne suivante, '//' deux lignes plus bas.
fn tall_body(s: &mut Script, start: u32, attr: u16) {
    let mut line = start;
    let mut at = line;
    loop {
        match s.word() {
            0x2323 => at += COLUMN,
            0x4040 => return,
            0x2f40 => {
                line += ROW;
                at = line;
            }
            0x2f2f => {
                line += 2 * ROW;
                at = line;
            }
            ch => {
                put(s.b, at, ch, attr);
                put(s.b, at + ROW, ch.wrapping_add(0x10), attr);
                at += COLUMN;
            }
        }
    }
}

/// 0x1136 : texte haut à la position du descripteur ; rend la fin du descripteur.
pub fn print_tall(b: &mut Cps2Bus, descripteur: u32) -> u32 {
    let mut s = Script { b, at: descripteur };
    let (col, row) = (s.byte(), s.byte());
    let attr = s.word();
    tall_body(&mut s, cell(col, row), attr);
    s.at
}

/// 0x1234 : texte haut à la position (`col`, `row`) (D5, D6 de l'original) ; le descripteur commence aux
/// attributs. Rend la fin du descripteur.
pub fn print_tall_at(b: &mut Cps2Bus, descripteur: u32, col: u8, row: u8) -> u32 {
    let mut s = Script { b, at: descripteur };
    let attr = s.word();
    tall_body(&mut s, cell(col, row), attr);
    s.at
}

fn print_tall_native(c: &mut Cpu, b: &mut Cps2Bus) {
    let end = print_tall(b, c.a[4]);
    finish(c, b, end, false);
}

fn print_tall_at_native(c: &mut Cpu, b: &mut Cps2Bus) {
    let end = print_tall_at(b, c.a[4], c.d[5] as u8, c.d[6] as u8);
    finish(c, b, end, false);
}

/// Blocs de 2x2 cases : '@@' fin, '##' saute deux colonnes, '//' deux lignes plus bas, 0xffff n : n
/// blocs vides (tuile 0x20). Un autre mot `t` dessine un bloc : tuiles `t, t+1` puis `t+0x10, t+0x11` en
/// dessous avec les attributs du descripteur (négatifs : palette `+0x2b` de l'objet A0) pour 0x0f38 ;
/// bloc vide (tuile 0x2fc, attributs 0) pour 0x0f4a.
fn blocks(b: &mut Cps2Bus, descripteur: u32, objet: u32, drawn: bool) -> (u32, bool) {
    let attr = rd16(b, descripteur + 2);
    let attr = if attr & 0x8000 != 0 { rd8(b, objet + 0x2b) as u16 } else { attr };
    let mut s = Script { b, at: descripteur };
    let (col, row) = (s.byte(), s.byte());
    s.word();
    let mut line = cell(col, row);
    let mut at = line;
    let mut x = false;
    loop {
        match s.word() {
            0x2323 => at += 2 * COLUMN,
            0x4040 => break,
            0xffff => {
                let n = s.word();
                for _ in 0..n.wrapping_sub(1) as u32 + 1 {
                    for (dc, dr) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
                        put(s.b, at + dc * COLUMN + dr * ROW, BLANK, 0);
                    }
                    at += 2 * COLUMN;
                }
                x = false; // add.w D4,D1 (D4 = 0)
            }
            0x2f2f => {
                line += 2 * ROW;
                at = line;
            }
            tile => {
                let (code, attr) = if drawn { (tile, attr) } else { (0x2fc, 0) };
                put(s.b, at, code, attr);
                put(s.b, at + ROW, code.wrapping_add(0x10), attr);
                put(s.b, at + COLUMN, code.wrapping_add(1), attr);
                put(s.b, at + COLUMN + ROW, code.wrapping_add(0x11), attr);
                // retenue du dernier addi.w #0x10
                x = code.wrapping_add(1).checked_add(0x10).is_none();
                at += 2 * COLUMN;
            }
        }
    }
    (s.at, x)
}

/// 0x0f4a : efface des blocs de 2x2 cases (`objet` : A0 de l'original, lu seulement si les attributs du
/// descripteur sont négatifs) ; rend la fin du descripteur.
pub fn clear_blocks(b: &mut Cps2Bus, descripteur: u32, objet: u32) -> u32 {
    blocks(b, descripteur, objet, false).0
}

/// 0x0f38 : dessine des blocs de 2x2 cases (attributs négatifs : palette `+0x2b` de `objet`) ; rend la fin du
/// descripteur.
pub fn draw_blocks(b: &mut Cps2Bus, descripteur: u32, objet: u32) -> u32 {
    blocks(b, descripteur, objet, true).0
}

fn blocks_native(c: &mut Cpu, b: &mut Cps2Bus, drawn: bool) {
    let (end, x) = blocks(b, c.a[4], c.a[0], drawn);
    finish(c, b, end, x);
}

/// Efface un rectangle de la couche (colonne, ligne, largeur, hauteur) avec des espaces. Une ligne
/// d'au moins 0x20 est ramenée dans la couche.
pub fn clear_cells(b: &mut Cps2Bus, col: u8, row: u8, width: u8, height: u8) {
    let row = if row >= 0x20 { row - 0x20 } else { row };
    let origin = LAYER + ((col as u32) << 7) + ((row as u32) << 2);
    for x in 0..width.wrapping_sub(1) as u32 + 1 {
        for y in 0..height.wrapping_sub(1) as u32 + 1 {
            put(b, origin + x * COLUMN + y * ROW, BLANK, 0);
        }
    }
}

/// 0x1324 : efface le rectangle décrit en `descripteur` (colonne, ligne, largeur, hauteur) avec des espaces. Rend
/// la fin du descripteur, qui n'avance que de 3 octets (la hauteur est lue sans post-incrément).
pub fn clear_box(b: &mut Cps2Bus, descripteur: u32) -> u32 {
    clear_box_height(b, descripteur).0
}

fn clear_box_height(b: &mut Cps2Bus, descripteur: u32) -> (u32, u8) {
    let mut s = Script { b, at: descripteur };
    let col = s.byte();
    let row = s.byte();
    let width = s.byte();
    let height = rd8(s.b, s.at);
    let end = s.at;
    clear_cells(b, col, row, width, height);
    (end, height)
}

fn clear_box_native(c: &mut Cpu, b: &mut Cps2Bus) {
    let (end, height) = clear_box_height(b, c.a[4]);
    c.a[4] = end;
    // drapeaux : dernier move.w #0 (Z) ; X = retenue de « hauteur - 1 »
    let x = if height == 0 { SR_X } else { 0 };
    c.sr = (c.sr & !(SR_X | SR_N | SR_V | SR_C)) | SR_Z | x;
    rts(c, b);
}

const fn native(entry: u32, name: &'static str, run: fn(&mut Cpu, &mut Cps2Bus)) -> Native {
    Native { entry, name, run, touches_sound: false, outputs: regs::a(4) | regs::CCR }
}

pub const PRINT: Native = native(0x0d5e, "text_print", |c, b| texte_native(c, b, Sorte::Texte));
pub const ERASE: Native = native(0x0d32, "text_erase", |c, b| texte_native(c, b, Sorte::Effacement));
pub const PRINT_OVER: Native = native(0x0dca, "text_print_over", |c, b| texte_native(c, b, Sorte::Surimpression));
pub const PRINT_TALL: Native = native(0x1136, "text_print_tall", print_tall_native);
pub const PRINT_TALL_AT: Native = native(0x1234, "text_print_tall_at", print_tall_at_native);
pub const CLEAR_BLOCKS: Native = native(0x0f4a, "text_clear_blocks", |c, b| blocks_native(c, b, false));
pub const DRAW_BLOCKS: Native = native(0x0f38, "text_draw_blocks", |c, b| blocks_native(c, b, true));
pub const CLEAR_BOX: Native = native(0x1324, "text_clear_box", clear_box_native);
