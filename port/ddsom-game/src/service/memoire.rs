//! Écran 8. MEMORY CHECK du menu TEST (`0x2ceb4`) : test de chaque mémoire, puis retour au menu après
//! une seconde.
//!
//! Le test (`0x2cf26`) écrit quatre motifs dans chaque mot de la RAM de travail, de la GFX RAM, des deux
//! bancs de l'Object RAM et de la RAM partagée du Z80 (en relisant aussitôt, puis en rétablissant le mot),
//! teste l'EEPROM (`eeprom::tester`) et compare la somme d'un octet sur 0xfff de chaque bloc de 512 Ko du
//! programme à la table de l'en-tête (0xca). Chaque ligne affiche « TESTING... » puis « OK » ; une erreur
//! affiche « NG » et bloque la tâche.
//!
//! **Écart assumé** : l'original fait ce test interruptions masquées, d'un seul tenant ; il dure une
//! vingtaine d'images à ×16 (plusieurs secondes sur la borne), et sa durée dépend de la fréquence du 68000.
//! Il est instantané dans les deux machines de la validation : ici en Rust, et dans le code d'origine par
//! le correctif [`patch_instant_check`], qui l'exécute sans faire avancer le temps.

use super::{back_to_menu, f, get, layers_off, layers_on, next_step, print, set};
use crate::mem::{rd32, wr8};
use crate::modes::Flow;
use crate::scene::Ctx;
use cps2::Cps2Bus;
use m68k::Cpu;

/// Entrée du test dans l'original (appelé par `bsr` depuis l'étape 2 de l'écran).
pub const CHECK: u32 = 0x2cf26;
/// Boucle sans fin de l'original après un « NG ».
pub const BLOQUE: u32 = 0x2d112;

const TITLE: u32 = 0x2d184;
const LABELS: u32 = 0x2d1a8;
/// Textes de 10 caractères (octets en ROM).
const TESTING: u32 = 0x2d27c;
const OK: u32 = 0x2d286;
const NG: u32 = 0x2d290;
/// Décompte avant le retour au menu (champ de l'enregistrement).
const COUNTDOWN: u32 = 0x14;
const COUNTDOWN_FRAMES: u8 = 0x3c;

/// Motifs écrits dans chaque mot (table adressée relativement au PC).
const PATTERNS: u32 = 0x2d0ce;
/// Sortie qui choisit le banc de l'Object RAM vu en 0x708000.
const OBJ_BANK: u32 = 0x8040e1;
/// Octet de la RAM partagée du Z80 posé avant et après son test.
const Z80_FLAG: u32 = 0x619ffb;
/// Sommes attendues des blocs du programme (mots de l'en-tête).
const ROM_SUMS: u32 = 0xca;
const ROM_BLOCK: u32 = 0x80000;
const ROM_STRIDE: u32 = 0xfff;

/// Cases de chaque ligne de résultat.
mod cell {
    pub const WORK_RAM: u32 = 0x912ea0;
    pub const CPS_RAM: u32 = 0x912ea8;
    pub const OBJECT_RAM: u32 = 0x912eb0;
    pub const SOUND_RAM: u32 = 0x912eb8;
    pub const EEPROM: u32 = 0x912ec0;
    pub const ROM: u32 = 0x912ec8;
}

pub fn memory(x: &mut Ctx) -> Flow {
    match get(x, f::STEP) {
        0 => {
            next_step(x);
            layers_off(x);
            print(x, TITLE);
            print(x, LABELS);
            layers_on(x);
            Flow::Done
        }
        2 => {
            // l'original masque les interruptions autour du test (trap 12) : sans effet sur un test instantané
            if !check(x) {
                crate::kernel::bloquer(x.b, BLOQUE);
                return Flow::Yield;
            }
            next_step(x);
            set(x, COUNTDOWN, COUNTDOWN_FRAMES);
            Flow::Done
        }
        _ => {
            let left = get(x, COUNTDOWN) as i8 as i16 - 1;
            set(x, COUNTDOWN, left as u8);
            if left > 0 { Flow::Done } else { back_to_menu(x) }
        }
    }
}

/// `0x2d12a` : un texte de 10 caractères dans la ligne `at`.
fn label(x: &mut Ctx, at: u32, text: u32, attr: u16) {
    for k in 0..10 {
        let ch = x.rd8(text + k) as u16;
        x.wr16(at + 0x80 * k, ch);
        x.wr16(at + 0x80 * k + 2, attr);
    }
}

/// Résultat d'une ligne : « OK », ou « NG » (en couleur).
fn verdict(x: &mut Ctx, at: u32, ok: bool) -> bool {
    if ok {
        label(x, at, OK, 0);
    } else {
        label(x, at, NG, super::HIGHLIGHT);
    }
    ok
}

/// `0x2d096` : chaque mot de `start..end` reçoit les quatre motifs, relus aussitôt (octet de poids faible
/// seulement pour la RAM du Z80, `byte`), puis sa valeur d'origine.
fn test_words(x: &mut Ctx, start: u32, end: u32, byte: bool) -> bool {
    let patterns: [u16; 4] = std::array::from_fn(|k| x.prog16(PATTERNS + 2 * k as u32));
    for a in (start..end).step_by(2) {
        let saved = x.rd16(a);
        for p in patterns {
            x.wr16(a, p);
            let ok = if byte { x.rd8(a + 1) == p as u8 } else { x.rd16(a) == p };
            if !ok {
                return false;
            }
            x.wr16(a, saved);
        }
    }
    true
}

/// `bchg #0` de la sortie du banc de l'Object RAM (relue comme le fait l'original).
fn toggle_bank(x: &mut Ctx) {
    let v = x.rd8(OBJ_BANK) ^ 1;
    wr8(x.b, OBJ_BANK, v);
}

/// `0x2d0d6` : somme d'un octet sur 0xfff du bloc `k` du programme.
fn rom_sum(x: &mut Ctx, k: u32) -> u16 {
    let start = ROM_BLOCK * k;
    let mut sum = 0u32;
    let mut a = start;
    while a < start + ROM_BLOCK {
        sum = sum.wrapping_add(x.rd8(a) as u32);
        a += ROM_STRIDE;
    }
    sum as u16
}

/// Une ligne de test de RAM : « TESTING... », le test, puis « OK » (sauf `quiet`) ou « NG ».
fn test_line(x: &mut Ctx, at: u32, (start, end): (u32, u32), byte: bool, quiet: bool) -> bool {
    let ok = test_words(x, start, end, byte);
    if !ok || !quiet {
        verdict(x, at, ok);
    }
    ok
}

/// `0x2cf26` : le test complet. Faux après un « NG » (l'original reste bloqué).
fn check(x: &mut Ctx) -> bool {
    label(x, cell::WORK_RAM, TESTING, 0);
    if !test_line(x, cell::WORK_RAM, (0xff_0000, 0x100_0000), false, false) {
        return false;
    }
    label(x, cell::CPS_RAM, TESTING, 0);
    if !test_line(x, cell::CPS_RAM, (0x90_0000, 0x93_0000), false, false) {
        return false;
    }
    // Object RAM : le banc vu en 0x708000 (sans « OK »), puis l'autre
    label(x, cell::OBJECT_RAM, TESTING, 0);
    if !test_line(x, cell::OBJECT_RAM, (0x70_8000, 0x70_a000), false, true) {
        return false;
    }
    toggle_bank(x);
    if !test_line(x, cell::OBJECT_RAM, (0x70_8000, 0x70_a000), false, false) {
        return false;
    }
    toggle_bank(x);
    wr8(x.b, Z80_FLAG, 0);
    label(x, cell::SOUND_RAM, TESTING, 0);
    if !test_line(x, cell::SOUND_RAM, (0x61_8000, 0x61_a000), true, false) {
        return false;
    }
    wr8(x.b, Z80_FLAG, 0x88);
    label(x, cell::EEPROM, TESTING, 0);
    let ok = crate::eeprom::tester(x.c, x.b);
    if !verdict(x, cell::EEPROM, ok) {
        return false;
    }
    for k in 0..8 {
        let at = cell::ROM + 4 * k;
        label(x, at, TESTING, 0);
        let ok = rom_sum(x, k) == x.rd16(ROM_SUMS + 2 * k);
        if !verdict(x, at, ok) {
            return false;
        }
    }
    true
}

/// Correctif commun aux deux machines (`lib.rs`, `patches`) : le test d'origine (`0x2cf26`, entré par
/// `bsr`) est exécuté jusqu'à son retour par l'interpréteur, sans faire avancer le temps de la machine.
/// Après un « NG », le test boucle sans fin : l'exécution s'arrête au bout d'un budget d'instructions et
/// la machine reprend la boucle.
pub fn patch_instant_check(c: &mut Cpu, b: &mut Cps2Bus) -> bool {
    let cycles = c.cycles;
    let sp = c.a[7];
    let ret = rd32(b, sp);
    let mut budget = 200_000_000u32;
    loop {
        c.step(b);
        budget -= 1;
        if (c.pc == ret && c.a[7] == sp.wrapping_add(4)) || budget == 0 {
            break;
        }
    }
    c.cycles = cycles;
    true
}
