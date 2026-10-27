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

use super::motion::{cmp, lsl_word, prog32};
use super::Obj;
use crate::mem::{rd16, rd32, rd8, rts, set_low_byte, set_low_word};
use crate::{regs, Native};

/// Résultat d'un pas d'animation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// La durée de l'entrée courante n'est pas écoulée.
    Running,
    /// La durée était nulle : elle repart de 0xFFFF. Glitch d'origine : la soustraction emprunte, et la retenue
    /// que les appelants testent (`bcs`) est levée comme en fin de script.
    Wrapped,
    /// Une nouvelle entrée a été chargée.
    Loaded,
    /// Fin du script : l'objet reste sur son dernier cadre.
    Ended,
}

impl Step {
    /// La retenue de l'original est levée : fin du script, ou durée nulle qui repart de 0xFFFF (glitch d'origine).
    pub fn finished(self) -> bool {
        matches!(self, Step::Ended | Step::Wrapped)
    }
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

/// Adresse du script `index` dans un jeu d'animations (décalage lu à `set + 2 × index`, sans repli sur 16 bits :
/// voir `start` pour l'indexation exacte de l'original).
pub fn script_of(b: &mut Cps2Bus, set: u32, index: u16) -> u32 {
    let off = rd16(b, set.wrapping_add(2 * index as u32)) as i16;
    set.wrapping_add(off as i32 as u32)
}

/// Un pas d'animation : décompte la durée et, à zéro, charge l'entrée suivante.
pub fn step(b: &mut Cps2Bus, obj: Obj, long: bool) -> Step {
    let before = obj.u16(b, Obj::ANIM_TIMER);
    let timer = before.wrapping_sub(1);
    obj.set_u16(b, Obj::ANIM_TIMER, timer);
    if timer != 0 {
        return if before == 0 { Step::Wrapped } else { Step::Running };
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

/// 0x1a32 : démarre le script `n` du jeu d'animations `set` (`lea set,A4 ; moveq #n,D0 ; jsr 0x1a32`). L'index
/// est un mot (`add.w D0,D0 ; move.w (A4,D0.w),D0`) : le double est étendu en signe. Rend l'adresse qui suit
/// l'entrée chargée.
pub fn start(b: &mut Cps2Bus, obj: Obj, set: u32, n: u16) -> u32 {
    let script = script_at(b, set, n);
    load_entry(b, obj, script, false)
}

fn script_at(b: &mut Cps2Bus, set: u32, n: u16) -> u32 {
    let off = rd16(b, set.wrapping_add(n.wrapping_add(n) as i16 as i32 as u32));
    set.wrapping_add(off as i16 as i32 as u32)
}

/// 0x1a3c : démarre l'animation à l'entrée `at` (A4 de l'original).
pub fn start_entry(b: &mut Cps2Bus, obj: Obj, at: u32) -> u32 {
    load_entry(b, obj, at, false)
}

/// Jeux d'animations propres à chaque personnage (entrées longues) : une table de pointeurs lue dans le code,
/// un jeu par variante `Obj::VARIANT`.
pub struct CharacterSets {
    /// Routine d'origine qui démarre une animation de ces jeux.
    pub routine: u32,
    /// Table des pointeurs de jeux (espace programme).
    pub pointers: u32,
    /// Sans arme (octet `0x68` nul), les numéros d'animation sont décalés de 0x40.
    pub weapon_offset: bool,
    /// La variante est étendue en signe avant l'indexation.
    pub sign_extend: bool,
}

/// 0x2c16 : jeux des joueurs, numéros décalés de 0x40 sans arme en main.
pub const SETS_ARMED: CharacterSets =
    CharacterSets { routine: 0x2c16, pointers: 0x2c58, weapon_offset: true, sign_extend: true };
/// 0x2c88 : seconde table, numéro direct.
pub const SETS_PLAIN: CharacterSets =
    CharacterSets { routine: 0x2c88, pointers: 0x2cc0, weapon_offset: false, sign_extend: true };
/// 0x2cf0 : troisième table, variante prise sans extension de signe.
pub const SETS_BY_VARIANT: CharacterSets =
    CharacterSets { routine: 0x2cf0, pointers: 0x2d28, weapon_offset: false, sign_extend: false };

/// Octet de l'objet : le personnage tient son arme.
const ARMED: u32 = 0x68;

/// Démarre l'animation `n` (D6 de l'original) du jeu propre au personnage de l'objet (entrées longues).
pub fn start_by_character(b: &mut Cps2Bus, obj: Obj, sets: &CharacterSets, n: u16) -> u32 {
    let n = if sets.weapon_offset && obj.u8(b, ARMED) == 0 { n.wrapping_add(0x40) } else { n };
    let variant = obj.u8(b, Obj::VARIANT);
    let v = if sets.sign_extend { variant as i8 as i16 as u16 } else { variant as u16 };
    let set = prog32(b, sets.pointers.wrapping_add((v << 2) as i16 as i32 as u32));
    let script = script_at(b, set, n);
    load_entry(b, obj, script, true)
}

// ---- adaptation aux registres du 68000 ----

/// Registres que laissent les routines d'origine (voir `diag::registres_morts`, étiquette `anim`).
pub const REGS_START: u32 = regs::d(0) | regs::a(4) | regs::CCR;
pub const REGS_STEP: u32 = regs::d(0) | regs::a(4) | regs::CCR;
pub const REGS_BY_CHARACTER: u32 = regs::d(5) | regs::d(6) | regs::a(4) | regs::CCR;

/// `start` avec les registres de l'original : D0 = `n` puis le décalage du script (mot bas), A4 = fin de
/// l'entrée, drapeaux du dernier `move.l` (X : retenue du doublement de l'index).
#[track_caller]
pub fn start_68k(c: &mut Cpu, b: &mut Cps2Bus, obj: Obj, set: u32, n: u16) -> u32 {
    c.d[0] = n as u32;
    c.a[4] = set;
    let end = start_regs(c, b, obj);
    crate::diag::registres_morts(c, "anim", REGS_START);
    end
}

/// `start_entry` avec les registres de l'original : A4 = fin de l'entrée, drapeaux du `move.l`.
#[track_caller]
pub fn start_entry_68k(c: &mut Cpu, b: &mut Cps2Bus, obj: Obj, at: u32) -> u32 {
    let end = start_entry(b, obj, at);
    c.a[4] = end;
    c.flags_logic(end, Size::Long);
    crate::diag::registres_morts(c, "anim", REGS_START);
    end
}

/// `step` avec les registres de l'original : A4 = entrée suivante, D0.b = drapeaux, retenue = fin de script.
#[track_caller]
pub fn step_68k(c: &mut Cpu, b: &mut Cps2Bus, obj: Obj, long: bool) -> Step {
    let r = step_regs(c, b, obj, long);
    crate::diag::registres_morts(c, "anim", REGS_STEP);
    r
}

/// `start_by_character` avec les registres de l'original (D5 = variante × 4, D6 = décalage du script, A4 = fin
/// de l'entrée).
#[track_caller]
pub fn start_by_character_68k(c: &mut Cpu, b: &mut Cps2Bus, obj: Obj, sets: &CharacterSets, n: u16) -> u32 {
    c.d[6] = c.d[6] & 0xffff_0000 | n as u32;
    let end = by_character_regs(c, b, obj, sets);
    crate::diag::registres_morts(c, "anim", REGS_BY_CHARACTER);
    end
}

/// 0x1a32 : démarre le script `D0` du jeu d'animations A4 pour l'objet. Laisse A4 après l'entrée.
fn start_regs(c: &mut Cpu, b: &mut Cps2Bus, obj: Obj) -> u32 {
    // add.w D0w,D0w ; move.w (A4,D0w),D0w
    let d0 = c.d[0] as u16;
    let doubled = c.addsub(false, d0 as u32, d0 as u32, false, Size::Word, false) as u16;
    let off = rd16(b, c.a[4].wrapping_add(doubled as i16 as i32 as u32));
    set_low_word(&mut c.d[0], off);
    c.flags_logic(off as u32, Size::Word);
    c.a[4] = c.a[4].wrapping_add(off as i16 as i32 as u32);
    let end = load_entry(b, obj, c.a[4], false);
    c.a[4] = end;
    c.flags_logic(end, Size::Long);
    end
}

/// Pas d'animation avec la sémantique complète des registres : A4 = entrée suivante, D0.b = drapeaux,
/// C = fin de script.
fn step_regs(c: &mut Cpu, b: &mut Cps2Bus, obj: Obj, long: bool) -> Step {
    let timer = obj.u16(b, Obj::ANIM_TIMER);
    let left = c.addsub(true, 1, timer as u32, false, Size::Word, false) as u16;
    obj.set_u16(b, Obj::ANIM_TIMER, left);
    if left != 0 {
        return if timer == 0 { Step::Wrapped } else { Step::Running };
    }
    c.a[4] = obj.u32(b, Obj::ANIM_NEXT);
    let flags = obj.u8(b, Obj::ANIM_FLAGS);
    set_low_byte(&mut c.d[0], flags);
    c.flags_logic(flags as u32, Size::Byte);
    if flags != 0 {
        cmp(c, 0xfe, flags as u32, Size::Byte);
        if flags == 0xfe {
            obj.set_u16(b, Obj::ANIM_TIMER, 1);
            c.set_ccr(1);
            return Step::Ended;
        }
        c.a[4] = rd32(b, c.a[4]);
    }
    let end = load_entry(b, obj, c.a[4], long);
    c.a[4] = end;
    c.flags_logic(end, Size::Long);
    Step::Loaded
}

/// Démarre l'animation `D6` du jeu propre au personnage de l'objet (entrées longues), registres compris.
fn by_character_regs(c: &mut Cpu, b: &mut Cps2Bus, obj: Obj, sets: &CharacterSets) -> u32 {
    if sets.weapon_offset && obj.u8(b, ARMED) == 0 {
        let v = c.addsub(false, 0x40, c.d[6], false, Size::Word, false);
        set_low_word(&mut c.d[6], v as u16);
    }
    let variant = rd8(b, obj.at(Obj::VARIANT));
    let d5 = if sets.sign_extend {
        set_low_byte(&mut c.d[5], variant);
        (variant as i8 as i16) as u16
    } else {
        c.d[5] = variant as u32;
        variant as u16
    };
    let d5 = lsl_word(c, d5, 2);
    set_low_word(&mut c.d[5], d5);
    let set = prog32(b, sets.pointers.wrapping_add(d5 as i16 as i32 as u32));
    c.a[4] = set;
    let d6 = c.d[6] as u16;
    let doubled = c.addsub(false, d6 as u32, d6 as u32, false, Size::Word, false) as u16;
    let off = rd16(b, set.wrapping_add(doubled as i16 as i32 as u32));
    set_low_word(&mut c.d[6], off);
    c.a[4] = set.wrapping_add(off as i16 as i32 as u32);
    let end = load_entry(b, obj, c.a[4], true);
    c.a[4] = end;
    c.flags_logic(end, Size::Long);
    end
}

// ---- natives (appels par adresse restants) ----

macro_rules! anim_native {
    ($const:ident, $entry:expr, $name:expr, $run:expr) => {
        pub const $const: Native = Native {
            entry: $entry,
            name: $name,
            run: |c, b| {
                let obj = Obj(c.a[0]);
                $run(c, b, obj);
                rts(c, b);
            },
            touches_sound: false,
            outputs: regs::ALL,
        };
    };
}

anim_native!(ANIM_START_INDEXED, 0x1a32, "anim_start_indexed", |c, b, o| {
    start_regs(c, b, o);
});
anim_native!(ANIM_START, 0x1a3c, "anim_start", |c: &mut Cpu, b: &mut Cps2Bus, o| {
    let end = start_entry(b, o, c.a[4]);
    c.a[4] = end;
    c.flags_logic(end, Size::Long);
});
anim_native!(ANIM_STEP, 0x1a56, "anim_step", |c, b, o| {
    step_regs(c, b, o, false);
});
anim_native!(ANIM_STEP_LONG, 0x2d58, "anim_step_long", |c, b, o| {
    step_regs(c, b, o, true);
});
anim_native!(ANIM_START_LONG_DOUBLE, 0x2c16, "anim_start_long_double", |c, b, o| {
    by_character_regs(c, b, o, &SETS_ARMED);
});
anim_native!(ANIM_START_LONG_BY_CHAR, 0x2c88, "anim_start_long_by_char", |c, b, o| {
    by_character_regs(c, b, o, &SETS_PLAIN);
});
anim_native!(ANIM_START_LONG_BY_VARIANT, 0x2cf0, "anim_start_long_by_variant", |c, b, o| {
    by_character_regs(c, b, o, &SETS_BY_VARIANT);
});
