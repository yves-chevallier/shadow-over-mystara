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

/// 0x0d5e : texte. '@' fin, '/' ligne suivante, 0xff n : n espaces.
fn print(c: &mut Cpu, b: &mut Cps2Bus) {
    let mut s = Script { b, at: c.a[4] };
    let (col, row) = (s.byte(), s.byte());
    let attr = s.word();
    let (mut line, mut x) = cell_signed(col, row);
    let mut at = line;
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
            b'/' => {
                line += ROW;
                at = line;
            }
            ch => {
                put(s.b, at, ch as u16, attr);
                at += COLUMN;
            }
        }
    }
    let end = s.at;
    finish(c, b, end, x);
}

/// 0x0d32 : efface un texte (mêmes descripteurs que `print`) : chaque caractère devient un espace.
fn erase(c: &mut Cpu, b: &mut Cps2Bus) {
    let mut s = Script { b, at: c.a[4] };
    let (col, row) = (s.byte(), s.byte());
    let _ = s.word(); // attributs : sans effet
    let (mut line, mut x) = cell_signed(col, row);
    let mut at = line;
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
            b'/' => {
                line += ROW;
                at = line;
            }
            _ => {
                put(s.b, at, BLANK, 0);
                at += COLUMN;
            }
        }
    }
    let end = s.at;
    finish(c, b, end, x);
}

/// 0x0dca : comme `print`, mais '_' passe à la ligne et les espaces laissent le fond intact.
fn print_over(c: &mut Cpu, b: &mut Cps2Bus) {
    let mut s = Script { b, at: c.a[4] };
    let (col, row) = (s.byte(), s.byte());
    let attr = s.word();
    let (mut line, mut x) = cell_signed(col, row);
    let mut at = line;
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
            b'_' => {
                line += ROW;
                at = line;
            }
            b' ' => at += COLUMN,
            ch => {
                put(s.b, at, ch as u16, attr);
                at += COLUMN;
            }
        }
    }
    let end = s.at;
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

/// 0x1136 : texte haut à la position du descripteur.
fn print_tall(c: &mut Cpu, b: &mut Cps2Bus) {
    let mut s = Script { b, at: c.a[4] };
    let (col, row) = (s.byte(), s.byte());
    let attr = s.word();
    tall_body(&mut s, cell(col, row), attr);
    let end = s.at;
    finish(c, b, end, false);
}

/// 0x1234 : texte haut à la position (D5, D6).
fn print_tall_at(c: &mut Cpu, b: &mut Cps2Bus) {
    let (col, row) = (c.d[5] as u8, c.d[6] as u8);
    let mut s = Script { b, at: c.a[4] };
    let attr = s.word();
    tall_body(&mut s, cell(col, row), attr);
    let end = s.at;
    finish(c, b, end, false);
}

/// 0x0f4a : efface des blocs de 2x2 cases (tuile 0x2fc) : '@@' fin, '##' saute deux colonnes,
/// '//' deux lignes plus bas, 0xffff n : n blocs vides. Attributs négatifs : palette de l'objet A0.
fn clear_blocks(c: &mut Cpu, b: &mut Cps2Bus) {
    let a0 = c.a[0];
    let mut s = Script { b, at: c.a[4] };
    let (col, row) = (s.byte(), s.byte());
    let _ = s.word(); // attributs : sans effet (l'original écrit toujours 0)
    let _ = a0;
    let mut line = cell(col, row);
    let mut at = line;
    let mut x = false;
    let block = |b: &mut Cps2Bus, at: u32, code: u16| {
        put(b, at, code, 0);
        put(b, at + ROW, code.wrapping_add(0x10), 0);
        put(b, at + COLUMN, code.wrapping_add(1), 0);
        put(b, at + COLUMN + ROW, code.wrapping_add(0x11), 0);
    };
    loop {
        match s.word() {
            0x2323 => at += 2 * COLUMN,
            0x4040 => break,
            0xffff => {
                let n = s.word();
                x = n == 0; // subq.w #1
                for _ in 0..n.wrapping_sub(1) as u32 + 1 {
                    for (dc, dr) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
                        put(s.b, at + dc * COLUMN + dr * ROW, BLANK, 0);
                    }
                    at += 2 * COLUMN;
                }
            }
            0x2f2f => {
                line += 2 * ROW;
                at = line;
            }
            _ => {
                block(s.b, at, 0x2fc);
                at += 2 * COLUMN;
            }
        }
    }
    let end = s.at;
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

/// 0x1324 : efface un rectangle (colonne, ligne, largeur, hauteur) avec des espaces.
/// A4 n'avance que de 3 octets (la hauteur est lue sans post-incrément).
fn clear_box(c: &mut Cpu, b: &mut Cps2Bus) {
    let mut s = Script { b, at: c.a[4] };
    let col = s.byte();
    let row = s.byte();
    let width = s.byte();
    let height = rd8(s.b, s.at);
    let end = s.at;
    clear_cells(b, col, row, width, height);
    c.a[4] = end;
    // drapeaux : dernier move.w #0 (Z) ; X = retenue de « hauteur - 1 »
    let x = if height == 0 { SR_X } else { 0 };
    c.sr = (c.sr & !(SR_X | SR_N | SR_V | SR_C)) | SR_Z | x;
    rts(c, b);
}

/// Coût estimé : ~40 cycles par caractère du descripteur.
fn text_cost(c: &Cpu, b: &mut Cps2Bus) -> u64 {
    let mut n = 0;
    while n < 256 && rd8(b, c.a[4] + n) != b'@' {
        n += 1;
    }
    200 + 40 * n as u64
}

const fn native(entry: u32, name: &'static str, run: fn(&mut Cpu, &mut Cps2Bus)) -> Native {
    Native { entry, name, cost: text_cost, run, touches_sound: false, outputs: regs::a(4) | regs::CCR, exact_cost: false }
}

pub const PRINT: Native = native(0x0d5e, "text_print", print);
pub const ERASE: Native = native(0x0d32, "text_erase", erase);
pub const PRINT_OVER: Native = native(0x0dca, "text_print_over", print_over);
pub const PRINT_TALL: Native = native(0x1136, "text_print_tall", print_tall);
pub const PRINT_TALL_AT: Native = native(0x1234, "text_print_tall_at", print_tall_at);
pub const CLEAR_BLOCKS: Native = native(0x0f4a, "text_clear_blocks", clear_blocks);
pub const CLEAR_BOX: Native = native(0x1324, "text_clear_box", clear_box);
