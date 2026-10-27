//! Collision d'un objet avec le décor : 0x33fe4 (joueurs, ennemis, ombres...) et sa variante 0x3403c.
//!
//! L'attribut de terrain sous l'objet (`video::terrain`, 0x24b0) indexe une table de types en ROM
//! (entrées de 16 octets). Un type de genre 0 est franchissable ; sinon il décrit un obstacle dont
//! le profil dépend des 4 bits bas de la position dans la case (D0 = x, D1 = y) : l'objet est
//! repoussé hors de l'obstacle (`Obj::X` ou `Obj::Y` corrigé) et D5 vaut 1. Pour un obstacle plein
//! (genre 1) on recommence tant qu'il en reste (sauf si le champ `STOP` du type est non nul) ; pour
//! une pente (genre 2) un seul passage.
//!
//! Registres : comme les handlers qui suivent relisent D0-D2, D5, A1, A2 et les drapeaux, la routine
//! est transcrite instruction par instruction (A1 = adresse du sous-programme de l'original, que le
//! 68000 atteignait par une table de sauts, D1 corrigé...).

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::{Cpu, SR_C, SR_X};

use crate::mem::{rd16, rts, set_low_word, wr16, wr32, wr8, A5};
use crate::objects::Obj;
use crate::{call, ops, regs, Native};

/// Mot de réglages de A5 : bit 0x800, collision avec le décor active.
const ENABLED: u32 = 0x7fd0;
/// Dernier objet testé (A0), sa position x et sa position y : consultés par d'autres routines.
const LAST_OBJECT: u32 = 0x64;
const LAST_X: u32 = 0x60;
const LAST_Y: u32 = 0x62;
/// Octet de l'objet : numéro de terrain sous lui.
const OBJ_TERRAIN: u32 = 0x29;
/// Routine de l'attribut de terrain.
pub const TERRAIN_ATTRIBUTE: u32 = 0x24b0;

/// Champs d'un type de terrain (entrée de 16 octets, mots).
mod kind {
    /// 0 : franchissable ; 1 : obstacle plein ; plus : pente.
    pub const GENRE: u32 = 0;
    /// Sous-programme de l'obstacle (0, 4, 8 ou 12).
    pub const ROUTINE: u32 = 2;
    /// Pente : sens (0 ou autre).
    pub const FLIP: u32 = 4;
    /// Pente : décalage de la hauteur.
    pub const SHIFT: u32 = 6;
    /// Profondeur (pente : hauteur de base) du bord de l'obstacle.
    pub const EDGE: u32 = 8;
    /// Obstacle plein : non nul, un seul passage.
    pub const STOP: u32 = 10;
}

/// Les deux copies de la routine (`Variant`) : seule la table des types et les adresses des sous-programmes
/// (visibles dans A1) changent.
#[derive(Clone, Copy)]
struct Variant {
    entry: u32,
    /// La seconde copie pose D5 avant de savoir si l'objet touche l'obstacle plein.
    early_flag: bool,
    types: u32,
    solid: [u32; 4],
}

const FIRST: Variant = Variant { entry: 0x33fe4, early_flag: false, types: 0x3425c, solid: [0x340b6, 0x340d4, 0x340f2, 0x34108] };
const SECOND: Variant = Variant { entry: 0x3403c, early_flag: true, types: 0x34a5c, solid: [0x34120, 0x3413e, 0x3415c, 0x34172] };
/// Sous-programmes des pentes, communs aux deux copies.
const SLOPES: [u32; 4] = [0x341a4, 0x341d0, 0x341fe, 0x3422e];

struct Push<'a> {
    c: &'a mut Cpu,
    b: &'a mut Cps2Bus,
    variant: Variant,
}

impl Push<'_> {
    fn d(&self, n: usize) -> u16 {
        self.c.d[n] as u16
    }
    fn set_d(&mut self, n: usize, v: u16) {
        set_low_word(&mut self.c.d[n], v);
    }
    fn field(&mut self, a2: u32, off: u32) -> u16 {
        rd16(self.b, a2 + off)
    }
    /// `sub.w src,Dn` : drapeaux de l'original ; renvoie la retenue.
    fn sub_d(&mut self, n: usize, src: u16) -> bool {
        let r = self.c.addsub(true, src as u32, self.d(n) as u32, false, Size::Word, false);
        self.set_d(n, r as u16);
        self.c.sr & SR_C != 0
    }
    /// `add.w src,(off,A0)` / `sub.w src,(off,A0)`.
    fn add_obj(&mut self, sub: bool, src: u16, off: u32) {
        let a = self.c.a[0] + off;
        let r = self.c.addsub(sub, src as u32, rd16(self.b, a) as u32, false, Size::Word, false);
        wr16(self.b, a, r as u16);
    }
    /// `tst.w (off,A2)` : vrai si nul.
    fn zero(&mut self, a2: u32, off: u32) -> bool {
        let v = self.field(a2, off);
        ops::logique(self.c, v as u32, Size::Word);
        v == 0
    }
    /// `moveq #1,D5`.
    fn collided(&mut self) {
        self.c.d[5] = 1;
    }

    /// Répète les passages tant qu'un obstacle plein relance la routine (`beq entry`).
    fn run(&mut self) {
        while self.pass() {}
    }

    /// Un passage ; vrai pour recommencer.
    fn pass(&mut self) -> bool {
        let a0 = self.c.a[0];
        if self.variant.entry == FIRST.entry {
            wr16(self.b, A5 + LAST_OBJECT, a0 as u16);
            let (x, y) = (rd16(self.b, a0 + 8), rd16(self.b, a0 + 0x10));
            wr16(self.b, A5 + LAST_X, x);
            wr16(self.b, A5 + LAST_Y, y);
        }
        let enabled = rd16(self.b, A5 + ENABLED) & 0x800;
        self.set_d(0, enabled);
        ops::logique(self.c, enabled as u32, Size::Word);
        if enabled == 0 {
            return false;
        }
        let (x, y) = (rd16(self.b, a0 + 8), rd16(self.b, a0 + 0x10));
        self.set_d(0, x);
        self.set_d(1, y);
        call(self.c, self.b, TERRAIN_ATTRIBUTE);
        let number = self.c.d[2] as u8;
        wr8(self.b, a0 + OBJ_TERRAIN, number);
        ops::logique(self.c, number as u32, Size::Byte);
        if number == 0 {
            return self.free();
        }
        // lsl.w #4,D2w : entrée de la table des types
        let d2 = self.d(2);
        self.set_d(2, d2 << 4);
        let carry = d2 >> 12 & 1 != 0;
        ops::logique(self.c, (d2 << 4) as u32, Size::Word);
        ops::drapeaux(self.c, SR_C | SR_X, if carry { SR_C | SR_X } else { 0 });
        let a2 = self.variant.types.wrapping_add((d2 << 4) as i16 as u32);
        self.c.a[2] = a2;
        let genre = self.field(a2, kind::GENRE);
        ops::cmp(self.c, 1, genre as u32, Size::Word);
        match genre {
            0 => self.free(),
            1 => self.solid(a2),
            _ => {
                self.slope(a2);
                false
            }
        }
    }

    /// Rien à corriger : l'objet est libre (0x3402c).
    fn free(&mut self) -> bool {
        let a0 = self.c.a[0];
        wr16(self.b, A5 + LAST_OBJECT, 0);
        wr32(self.b, A5 + LAST_X, 0);
        self.c.d[2] = 0;
        wr8(self.b, a0 + OBJ_TERRAIN, 0);
        ops::logique(self.c, 0, Size::Byte);
        false
    }

    /// Obstacle plein : repousse l'objet hors de la case ; vrai pour recommencer.
    fn solid(&mut self, a2: u32) -> bool {
        let routine = self.field(a2, kind::ROUTINE);
        self.set_d(2, routine);
        self.c.a[1] = self.variant.solid[routine as usize / 4];
        let edge = self.field(a2, kind::EDGE);
        match routine {
            // en dessous / au dessus du bord : corrige y
            0 | 4 => {
                if self.variant.early_flag {
                    self.collided();
                }
                let y = if routine == 0 { self.d(1) } else { !self.d(1) };
                self.set_d(1, y & 0xf);
                if !self.sub_d(1, edge) {
                    return self.free();
                }
                self.collided();
                if routine == 0 {
                    self.set_d(1, self.d(1).wrapping_neg());
                }
                self.add_obj(false, self.d(1), Obj::Y);
            }
            // à droite / à gauche du bord : corrige x
            _ => {
                let x = if routine == 8 { self.d(0) } else { !self.d(0) };
                self.set_d(0, (x & 0xf) + 1);
                self.collided();
                self.add_obj(routine == 8, self.d(0), Obj::X);
            }
        }
        self.zero(a2, kind::STOP)
    }

    /// Pente (genre > 1) : un seul passage.
    fn slope(&mut self, a2: u32) {
        let routine = self.field(a2, kind::ROUTINE);
        self.set_d(2, routine);
        self.c.a[1] = SLOPES[routine as usize / 4];
        let flipped = self.field(a2, kind::FLIP) != 0;
        let (edge, shift) = (self.field(a2, kind::EDGE), self.field(a2, kind::SHIFT));
        match routine {
            // le bord est une rampe en x : hauteur au-dessus (0) ou au-dessous (4) de y
            0 | 4 => {
                let x = if flipped == (routine == 0) { !self.d(0) } else { self.d(0) };
                let height = ((x & 0xf) as u32 >> (shift & 63).min(31)) as u16;
                self.set_d(0, height.wrapping_add(edge).wrapping_add(1));
                self.set_d(2, shift);
                let y = if routine == 0 { self.d(1) } else { !self.d(1) };
                self.set_d(1, y & 0xf);
                if self.sub_d(0, self.d(1)) {
                    {
                    self.free();
                    return;
                }
                }
                self.collided();
                self.add_obj(routine == 4, self.d(0), Obj::Y);
            }
            // le bord est une rampe en y : largeur à gauche (8) ou à droite (12) de x
            _ => {
                let y = if flipped == (routine == 12) { !self.d(1) } else { self.d(1) };
                self.set_d(1, y & 0xf);
                if self.sub_d(1, edge) {
                    {
                    self.free();
                    return;
                }
                }
                self.set_d(2, shift);
                self.set_d(1, ((self.d(1) as u32) << (shift & 63).min(31)) as u16);
                let x = if routine == 8 { !self.d(0) } else { self.d(0) };
                self.set_d(0, x & 0xf);
                if self.sub_d(1, self.d(0)) {
                    {
                    self.free();
                    return;
                }
                }
                self.collided();
                self.add_obj(routine == 8, self.d(1), Obj::X);
            }
        }
    }
}

fn run(variant: Variant, c: &mut Cpu, b: &mut Cps2Bus) {
    Push { c, b, variant }.run();
    rts(c, b);
}

/// 0x33fe4 appelée de Rust sur l'objet `o` : laisse les registres de l'original (D0-D2, D5, A0 = `o`, A1, A2 et les
/// drapeaux) ; rend le drapeau Z (vrai : la dernière case examinée est libre, ou la collision est désactivée). Les
/// appelants des joueurs n'ont pas encore été prouvés indépendants de ces registres.
pub fn push_68k(c: &mut Cpu, b: &mut Cps2Bus, o: Obj) -> bool {
    c.a[0] = o.0;
    Push { c, b, variant: FIRST }.run();
    c.sr & m68k::SR_Z != 0
}

pub const SHADOW: Native = Native {
    entry: 0x33fe4,
    name: "terrain_push",
    run: |c, b| run(FIRST, c, b),
    touches_sound: false,
    outputs: regs::ALL,
};

pub const SHADOW_SECOND: Native = Native {
    entry: 0x3403c,
    name: "terrain_push_2",
    run: |c, b| run(SECOND, c, b),
    touches_sound: false,
    outputs: regs::ALL,
};

pub const NATIVES: &[Native] = &[SHADOW, SHADOW_SECOND];
