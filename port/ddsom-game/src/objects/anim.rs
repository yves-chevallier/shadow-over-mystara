//! Scripts d'animation des objets.
//!
//! Un script est une suite d'*entrées* en ROM. L'objet n'en garde que l'entrée courante, recopiée
//! dans ses champs, et l'adresse de la suivante (`Obj::ANIM_NEXT`). Format d'une entrée :
//!
//! | décalage | taille | contenu | champ de l'objet |
//! |---|---|---|---|
//! | +0x00 | mot | durée (images) | `ANIM_TIMER` |
//! | +0x02 | mot | drapeaux (octet haut utilisé) | `ANIM_FLAGS` |
//! | +0x04 | long | cadre de sprite | `FRAME` |
//! | +0x08 | long | données du cadre A (ombre) | `FRAME_DATA_A` |
//! | +0x0c | long | données du cadre B (boîte de visibilité, priorité) | `FRAME_DATA_B` |
//! | +0x10 | 2 longs | seulement dans les entrées *longues* (personnages) | `FRAME_DATA_C/D` |
//!
//! Si l'octet de drapeaux de l'entrée vaut 0, la suivante suit immédiatement ; 0xFE signale la fin
//! du script (l'objet reste sur son dernier cadre, `anim_step` renvoie C) ; toute autre valeur est
//! suivie d'un long qui désigne l'entrée suivante (boucle ou saut).
//!
//! Un *jeu* d'animations est un tableau de mots (décalages signés relatifs au début du tableau) ;
//! chaque décalage désigne le premier élément d'un script.

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::Cpu;

use super::motion::{cmp, lsl_word};
use super::Obj;
use crate::mem::{rd16, rd32, rd8, rts, set_low_byte};
use crate::{regs, Native};

/// Résultat d'un pas d'animation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// La durée de l'entrée courante n'est pas écoulée.
    Running,
    /// Une nouvelle entrée a été chargée.
    Loaded,
    /// Fin du script : l'objet reste sur son dernier cadre.
    Ended,
}

/// Recopie l'entrée située en `at` dans l'objet et renvoie l'adresse qui la suit.
/// `long` : l'entrée contient aussi `FRAME_DATA_C/D`.
pub fn load_entry(b: &mut Cps2Bus, obj: Obj, at: u32, long: bool) -> u32 {
    let timer = rd16(b, at);
    obj.set_u16(b, Obj::ANIM_TIMER, timer);
    let flags = rd16(b, at + 2);
    obj.set_u16(b, Obj::ANIM_FLAGS, flags);
    let longs: &[u32] = if long {
        &[Obj::FRAME, Obj::FRAME_DATA_A, Obj::FRAME_DATA_B, Obj::FRAME_DATA_C, Obj::FRAME_DATA_D]
    } else {
        &[Obj::FRAME, Obj::FRAME_DATA_A, Obj::FRAME_DATA_B]
    };
    let mut p = at + 4;
    for &field in longs {
        let v = rd32(b, p);
        obj.set_u32(b, field, v);
        p += 4;
    }
    obj.set_u32(b, Obj::ANIM_NEXT, p);
    p
}

/// Adresse du script `index` dans un jeu d'animations.
pub fn script_of(b: &mut Cps2Bus, set: u32, index: u16) -> u32 {
    let off = rd16(b, set.wrapping_add(2 * index as u32)) as i16;
    set.wrapping_add(off as i32 as u32)
}

/// Un pas d'animation : décompte la durée et, à zéro, charge l'entrée suivante.
pub fn step(b: &mut Cps2Bus, obj: Obj, long: bool) -> Step {
    let timer = obj.u16(b, Obj::ANIM_TIMER).wrapping_sub(1);
    obj.set_u16(b, Obj::ANIM_TIMER, timer);
    if timer != 0 {
        return Step::Running;
    }
    let next = obj.u32(b, Obj::ANIM_NEXT);
    match obj.u8(b, Obj::ANIM_FLAGS) {
        0 => {
            load_entry(b, obj, next, long);
        }
        0xfe => {
            obj.set_u16(b, Obj::ANIM_TIMER, 1);
            return Step::Ended;
        }
        _ => {
            let target = rd32(b, next);
            load_entry(b, obj, target, long);
        }
    }
    Step::Loaded
}

// ---- adaptation aux registres du 68000 ----

/// 0x1a32 : démarre le script `D0` du jeu d'animations A4 pour l'objet A0. Laisse A4 après l'entrée.
fn anim_start_indexed(c: &mut Cpu, b: &mut Cps2Bus) {
    // add.w D0w,D0w ; move.w (A4,D0w),D0w
    let d0 = c.d[0] as u16;
    let doubled = c.addsub(false, d0 as u32, d0 as u32, false, Size::Word, false) as u16;
    let off = rd16(b, c.a[4].wrapping_add(doubled as i16 as i32 as u32));
    set_low_word(&mut c.d[0], off);
    c.flags_logic(off as u32, Size::Word);
    c.a[4] = c.a[4].wrapping_add(off as i16 as i32 as u32);
    anim_start(c, b);
}

fn set_low_word(r: &mut u32, v: u16) {
    *r = (*r & 0xffff_0000) | v as u32;
}

/// 0x1a3c : démarre l'entrée d'animation située en A4 pour l'objet A0.
fn anim_start(c: &mut Cpu, b: &mut Cps2Bus) {
    let end = load_entry(b, Obj(c.a[0]), c.a[4], false);
    c.a[4] = end;
    c.flags_logic(end, Size::Long);
    rts(c, b);
}

/// Pas d'animation avec la sémantique complète des registres : A4 = entrée suivante, D0.b = drapeaux,
/// C = fin de script.
fn step_native(c: &mut Cpu, b: &mut Cps2Bus, long: bool) {
    let obj = Obj(c.a[0]);
    let timer = obj.u16(b, Obj::ANIM_TIMER);
    let left = c.addsub(true, 1, timer as u32, false, Size::Word, false) as u16;
    obj.set_u16(b, Obj::ANIM_TIMER, left);
    if left == 0 {
        c.a[4] = obj.u32(b, Obj::ANIM_NEXT);
        let flags = obj.u8(b, Obj::ANIM_FLAGS);
        set_low_byte(&mut c.d[0], flags);
        c.flags_logic(flags as u32, Size::Byte);
        if flags != 0 {
            cmp(c, 0xfe, flags as u32, Size::Byte);
            if flags == 0xfe {
                obj.set_u16(b, Obj::ANIM_TIMER, 1);
                c.set_ccr(1);
                rts(c, b);
                return;
            }
            c.a[4] = rd32(b, c.a[4]);
        }
        let end = load_entry(b, obj, c.a[4], long);
        c.a[4] = end;
        c.flags_logic(end, Size::Long);
    }
    rts(c, b);
}

fn anim_step(c: &mut Cpu, b: &mut Cps2Bus) {
    step_native(c, b, false)
}

fn anim_step_long(c: &mut Cpu, b: &mut Cps2Bus) {
    step_native(c, b, true)
}

/// Tables de jeux d'animations par personnage, lues dans le code (espace programme) : un pointeur
/// par variante `Obj::VARIANT`.
const CHARACTER_SETS: u32 = 0x2c58;
const CHARACTER_SETS_B: u32 = 0x2cc0;
const CHARACTER_SETS_C: u32 = 0x2d28;

fn prog32(b: &mut Cps2Bus, a: u32) -> u32 {
    use m68k::Bus;
    (b.read_prog16(a & 0xff_ffff) as u32) << 16 | b.read_prog16(a.wrapping_add(2) & 0xff_ffff) as u32
}

/// Démarre l'animation `D6` du jeu propre au personnage de l'objet A0 (entrées longues). Variantes :
/// `weapon_offset` : sans arme (octet `0x68` nul), les numéros d'animation sont décalés de 0x40.
fn anim_start_by_character(c: &mut Cpu, b: &mut Cps2Bus, table: u32, weapon_offset: bool, sign_extend: bool) {
    let obj = Obj(c.a[0]);
    if weapon_offset && obj.u8(b, 0x68) == 0 {
        let v = c.addsub(false, 0x40, c.d[6], false, Size::Word, false);
        set_low_word(&mut c.d[6], v as u16);
    }
    let variant = rd8(b, obj.at(Obj::VARIANT));
    let d5 = if sign_extend {
        set_low_byte(&mut c.d[5], variant);
        (variant as i8 as i16) as u16
    } else {
        c.d[5] = variant as u32;
        variant as u16
    };
    let d5 = lsl_word(c, d5, 2);
    set_low_word(&mut c.d[5], d5);
    let set = prog32(b, table.wrapping_add(d5 as i16 as i32 as u32));
    c.a[4] = set;
    let d6 = c.d[6] as u16;
    let doubled = c.addsub(false, d6 as u32, d6 as u32, false, Size::Word, false) as u16;
    let off = rd16(b, set.wrapping_add(doubled as i16 as i32 as u32));
    set_low_word(&mut c.d[6], off);
    c.a[4] = set.wrapping_add(off as i16 as i32 as u32);
    let end = load_entry(b, obj, c.a[4], true);
    c.a[4] = end;
    c.flags_logic(end, Size::Long);
    rts(c, b);
}

/// 0x2c16 : jeu du personnage (table A), numéro D6 décalé de 0x40 si `0x68` est nul.
fn anim_start_long_double(c: &mut Cpu, b: &mut Cps2Bus) {
    anim_start_by_character(c, b, CHARACTER_SETS, true, true)
}

/// 0x2c88 : jeu du personnage (table B), numéro D6.
fn anim_start_long_by_char(c: &mut Cpu, b: &mut Cps2Bus) {
    anim_start_by_character(c, b, CHARACTER_SETS_B, false, true)
}

/// 0x2cf0 : jeu du personnage (table C), variante prise sans extension de signe.
fn anim_start_long_by_variant(c: &mut Cpu, b: &mut Cps2Bus) {
    anim_start_by_character(c, b, CHARACTER_SETS_C, false, false)
}

pub const ANIM_START_INDEXED: Native = Native {
    entry: 0x1a32,
    name: "anim_start_indexed",
    run: anim_start_indexed,
    touches_sound: false,
    outputs: regs::ALL,
};

pub const ANIM_START: Native = Native {
    entry: 0x1a3c,
    name: "anim_start",
    run: anim_start,
    touches_sound: false,
    outputs: regs::ALL,
};

pub const ANIM_STEP: Native = Native {
    entry: 0x1a56,
    name: "anim_step",
    run: anim_step,
    touches_sound: false,
    outputs: regs::ALL,
};

pub const ANIM_STEP_LONG: Native = Native {
    entry: 0x2d58,
    name: "anim_step_long",
    run: anim_step_long,
    touches_sound: false,
    outputs: regs::ALL,
};

pub const ANIM_START_LONG_DOUBLE: Native = Native {
    entry: 0x2c16,
    name: "anim_start_long_double",
    run: anim_start_long_double,
    touches_sound: false,
    outputs: regs::ALL,
};

pub const ANIM_START_LONG_BY_CHAR: Native = Native {
    entry: 0x2c88,
    name: "anim_start_long_by_char",
    run: anim_start_long_by_char,
    touches_sound: false,
    outputs: regs::ALL,
};

pub const ANIM_START_LONG_BY_VARIANT: Native = Native {
    entry: 0x2cf0,
    name: "anim_start_long_by_variant",
    run: anim_start_long_by_variant,
    touches_sound: false,
    outputs: regs::ALL,
};
