//! Configuration du jeu (M2) : réglages gardés dans l'EEPROM, copie en RAM, valeurs par défaut.
//!
//! L'EEPROM contient deux exemplaires de la configuration (octets 0x00-0x2f et 0x30-0x5f), suivis de
//! quatre compteurs de comptabilité (octets 0x60-0x6f). L'exemplaire 1 porte en 0x20-0x2f une
//! signature propre à la variante du jeu (région, `A5+0x70`) ; à l'initialisation (0x2d30a), si la
//! signature ne correspond pas, les valeurs par défaut de la variante sont réécrites.
//!
//! Octets de la copie en RAM (`CONFIG`) et variables qu'ils alimentent (0x2d358) :
//!
//! | Octet | Variable | Rôle (h) |
//! |---|---|---|
//! | 0 | `A5+0x1c` | mode de monnayage (0-16 : table des pièces par crédit ; 17 : départ gratuit ; 18 : jeu gratuit) |
//! | 1 | `A5+0x1d` | mode de crédit : indexe les trois tables qui donnent `A5+0x20..0x22` |
//! | 2 | `A5+0x23` | |
//! | 3-5 | `A5+0x26..0x28` | |
//! | 6 | `A5+0x29` | |
//! | 7 | `A5+0x73` | |
//! | 8, 9 | `A5+0x84`, `A5+0x85` | |
//! | 16, 17 | `A5+0x2a`, `A5+0x2b` | |
//! | 0x60.. | `A5+0x76/0x7a/0x7e/0x7fd4` | compteurs de comptabilité (longs) |
//!
//! Les routines natives sont les « feuilles » de ce chargement ; l'enchaînement de l'initialisation
//! (0x2d30a) reste en code d'origine et les appelle.

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::Cpu;

use crate::chemin::Chemin;
use crate::mem::{rd8, rts};
use crate::vue::Vue;
use crate::{regs, Native};

mod ram {
    /// Variante du jeu (région), en octets (index ×2 dans la table des blocs).
    pub const VARIANTE: u32 = 0x70;
    /// Copie en RAM de l'EEPROM (64 mots).
    pub const CONFIG: u32 = 0xffff_9d8e;
    /// Mode de monnayage, mode de crédit... (destinations de `charger_variables`).
    pub const MODE_MONNAYAGE: u32 = 0x1c;
    pub const MODE_CREDIT: u32 = 0x1d;
    pub const PIECES: u32 = 0x1e;
    pub const COMPTEURS: [(u32, u32); 4] = [(0x60, 0x76), (0x64, 0x7a), (0x68, 0x7e), (0x6c, 0x7fd4)];
}

/// Distance entre l'exemplaire de la configuration et son exemplaire de secours.
const SECOURS: u32 = 0x30;
/// Table des blocs de valeurs par défaut par variante (mots relatifs à la table), en code.
const TABLE_VARIANTES: u32 = 0x2d656;
/// Tables du chargement : pièces par crédit (mots) selon le mode de monnayage, deux octets puis un
/// octet selon le mode de crédit.
const TABLE_PIECES: u32 = 0x2d38c;
const TABLE_CREDIT_A: u32 = 0x2d40e;
const TABLE_CREDIT_B: u32 = 0x2d426;

fn a5(c: &Cpu, off: u32) -> u32 {
    c.a[5].wrapping_add(off)
}

fn set_mot(r: &mut u32, v: u16) {
    *r = *r & 0xffff_0000 | v as u32;
}

// ---------------------------------------------------------------------------------------------
// 0x2d648 : bloc des valeurs par défaut de la variante

/// Rend D0 = déplacement lu dans la table, A6 = bloc des valeurs par défaut (blocs de 0x30 octets :
/// valeurs 0x00-0x11, signature 0x20-0x2f).
fn variante(c: &mut Cpu, b: &mut Cps2Bus) {
    let mut v = Vue::reelle(b);
    let index = v.rd16(a5(c, ram::VARIANTE));
    let deplacement = v.prog16(TABLE_VARIANTES.wrapping_add(index as i16 as u32));
    set_mot(&mut c.d[0], deplacement);
    c.flags_logic(deplacement as u32, Size::Word);
    c.a[6] = TABLE_VARIANTES.wrapping_add(deplacement as i16 as u32);
    rts(c, b);
}

// ---------------------------------------------------------------------------------------------
// 0x2d7aa : valeurs par défaut

/// Recopie le bloc par défaut de la variante en deux exemplaires (exemplaire et secours), puis
/// efface la suite de la copie (les compteurs de comptabilité).
fn charger_defauts(c: &mut Cpu, b: &mut Cps2Bus) {
    let mut v = Vue::reelle(b);
    let index = v.rd16(a5(c, ram::VARIANTE));
    let deplacement = v.prog16(TABLE_VARIANTES.wrapping_add(index as i16 as u32));
    let bloc = TABLE_VARIANTES.wrapping_add(deplacement as i16 as u32);
    let mut destination = a5(c, ram::CONFIG);
    for _ in 0..2 {
        for i in 0..24 {
            let mot = v.rd16(bloc + 2 * i);
            v.wr16(destination, mot);
            destination += 2;
        }
    }
    for i in 0..16 {
        v.wr16(destination + 2 * i, 0);
    }
    // restes de l'original : A1 et A6 en fin de zone, D0 = 0, D7 = 0xFFFF, drapeaux du dernier move
    c.a[1] = destination + 32;
    c.a[6] = bloc + 48;
    c.d[0] = 0;
    c.d[7] = 0xffff;
    c.flags_logic(0, Size::Word);
    rts(c, b);
}

/// Durée : deux appels de `variante` (sans branchement) et trois boucles `dbf`.
fn cout_charger_defauts(_: &Cpu, b: &mut Cps2Bus) -> u64 {
    let mut p = Chemin::default();
    for n in [24, 24, 16] {
        p.repete(n - 1, true);
        p.pris(false);
    }
    p.duree(b, 0x2d7aa)
}

// ---------------------------------------------------------------------------------------------
// 0x2d662 : validation et réparation

/// Remplace la valeur `i` de la copie (et de son secours) par la valeur par défaut.
fn reparer(v: &mut Vue, copie: u32, defauts: u32, i: u32) {
    let d = v.rd8(defauts + i);
    v.wr8(copie + i, d);
    v.wr8(copie + SECOURS + i, d);
}

/// Vérifie chaque réglage de la copie en RAM selon ses bornes ; remplace les valeurs invalides par
/// celles du bloc par défaut de la variante. L'ordre des tests et ses particularités sont ceux de l'original : par exemple,
/// l'octet 4 n'est examiné que si l'octet 3 était invalide.
fn valider(v: &mut Vue, c: &mut Cpu) {
    let copie = a5(c, ram::CONFIG);
    // l'original commence par 0x2d648 : A6 = bloc par défaut de la variante
    let variante = v.rd16(a5(c, ram::VARIANTE));
    let deplacement = v.prog16(TABLE_VARIANTES.wrapping_add(variante as i16 as u32));
    set_mot(&mut c.d[0], deplacement);
    let defauts = TABLE_VARIANTES.wrapping_add(deplacement as i16 as u32);
    c.a[6] = defauts;

    // octet 0 : au plus 9 (variantes 4 et 10) ou 18 ; la valeur 17 est tolérée pour 4 et 10
    let mut borne = 9;
    if !v.pris(variante == 4) && !v.pris(variante == 10) {
        borne = 18;
    }
    let mode = v.rd8(copie);
    if !v.pris(borne >= mode) {
        let mut reparer_0 = true;
        let mut variante_speciale = v.pris(variante == 4);
        if !variante_speciale {
            variante_speciale = !v.pris(variante != 10);
        }
        if variante_speciale && v.pris(mode == 17) {
            reparer_0 = false;
        }
        if reparer_0 {
            reparer(v, copie, defauts, 0);
        }
    }
    // octet 1 : au plus 11
    let o1 = v.rd8(copie + 1);
    if !v.pris(o1 <= 11) {
        reparer(v, copie, defauts, 1);
    }
    // octet 2 : exactement 1 si le mode (éventuellement réparé) est 17, sinon 0 ou 1
    let mode = v.rd8(copie);
    let o2 = v.rd8(copie + 2);
    let valide_2 = if v.pris(mode == 17) { v.pris(o2 == 1) } else { v.pris(o2 <= 1) };
    if !valide_2 {
        reparer(v, copie, defauts, 2);
    }
    // octets 3 et 4 : 0 ou 1 ; l'octet 4 n'est examiné que si l'octet 3 a dû être réparé
    let o3 = v.rd8(copie + 3);
    if !v.pris(o3 <= 1) {
        reparer(v, copie, defauts, 3);
        let o4 = v.rd8(copie + 4);
        if !v.pris(o4 <= 1) {
            reparer(v, copie, defauts, 4);
        }
    }
    // octet 5 : 0 ou 1
    let o5 = v.rd8(copie + 5);
    if !v.pris(o5 <= 1) {
        reparer(v, copie, defauts, 5);
    }
    // octet 6 : au plus 0, ou 1 pour la variante 6
    let borne6 = if v.pris(variante != 6) { 0 } else { 1 };
    let o6 = v.rd8(copie + 6);
    if !v.pris(borne6 >= o6) {
        reparer(v, copie, defauts, 6);
    }
    // octet 7 : égal au défaut, ou supérieur de 2 à 10 au plus
    let o7 = v.rd8(copie + 7);
    let d7 = v.rd8(defauts + 7);
    if !v.pris(o7 == d7) && !v.pris(o7.wrapping_sub(2) <= 8) {
        reparer(v, copie, defauts, 7);
    }
    // octet 8 : 0 ou 1
    let o8 = v.rd8(copie + 8);
    if !v.pris(o8 <= 1) {
        reparer(v, copie, defauts, 8);
    }
    // octet 9 : entre 2 et 14
    let o9 = v.rd8(copie + 9) as i8;
    if v.pris(o9 < 2) || !v.pris(o9 <= 14) {
        reparer(v, copie, defauts, 9);
    }
    // octets 16 et 17 : au plus 7
    for i in [16, 17] {
        let o = v.rd8(copie + i);
        if !v.pris(o <= 7) {
            reparer(v, copie, defauts, i);
        }
    }
}

fn natif_valider(c: &mut Cpu, b: &mut Cps2Bus) {
    valider(&mut Vue::reelle(b), c);
    rts(c, b);
}

fn cout_valider(c: &Cpu, b: &mut Cps2Bus) -> u64 {
    let mut c2 = c.clone();
    let mut v = Vue::simulee(b);
    valider(&mut v, &mut c2);
    let chemin = v.chemin;
    chemin.duree(b, 0x2d662)
}

// ---------------------------------------------------------------------------------------------
// 0x2d358 : de la copie en RAM aux variables du jeu

/// Recopie les réglages de la copie en RAM vers les variables du jeu et en déduit les réglages de
/// monnayage (`A5+0x1e..0x22`). Laisse D0, D1, A4 et les drapeaux comme l'original.
fn charger_variables(c: &mut Cpu, b: &mut Cps2Bus) {
    let mut v = Vue::reelle(b);
    let a4 = a5(c, ram::CONFIG);
    c.a[4] = a4;
    for k in [0x23, 0x25, 0x24] {
        v.wr8(a5(c, k), 0);
    }
    let mode = v.rd8(a4);
    v.wr8(a5(c, ram::MODE_MONNAYAGE), mode);
    // pièces par crédit : table, ou 0x0101 pour les modes 17 (départ gratuit) et 18 (jeu gratuit)
    let (pieces, commun) = match mode {
        18 => {
            v.wr8(a5(c, 0x25), 1);
            (0x0101, false)
        }
        17 => {
            v.wr8(a5(c, 0x23), 1);
            v.wr8(a5(c, 0x24), 1);
            (0x0101, true)
        }
        _ => (v.prog16(TABLE_PIECES + 2 * mode as u32), false),
    };
    v.wr16(a5(c, ram::PIECES), pieces);
    if !commun {
        let o2 = v.rd8(a4 + 2);
        v.wr8(a5(c, 0x23), o2);
    }
    for (octet, k) in [(3, 0x26), (4, 0x27), (5, 0x28)] {
        let o = v.rd8(a4 + octet);
        v.wr8(a5(c, k), o);
    }
    let credit = v.rd8(a4 + 1);
    v.wr8(a5(c, ram::MODE_CREDIT), credit);
    let double = 2 * credit as u32;
    let a = v.prog8(TABLE_CREDIT_A + double);
    let a2 = v.prog8(TABLE_CREDIT_A + 1 + double);
    let b3 = v.prog8(TABLE_CREDIT_B + credit as u32);
    v.wr8(a5(c, 0x21), a);
    v.wr8(a5(c, 0x22), a2);
    v.wr8(a5(c, 0x20), b3);
    for (octet, k) in [(6, 0x29), (7, 0x73), (8, 0x84), (9, 0x85), (16, 0x2a), (17, 0x2b)] {
        let o = v.rd8(a4 + octet);
        v.wr8(a5(c, k), o);
    }
    v.wr8(a5(c, 0x72), 0);
    let dernier = {
        let mut dernier = 0;
        for (place, variable) in ram::COMPTEURS {
            dernier = v.rd32(a4 + place);
            v.wr32(a5(c, variable), dernier);
        }
        dernier
    };
    c.d[0] = double;
    c.d[1] = credit as u32;
    c.flags_logic(dernier, Size::Long);
    rts(c, b);
}

fn cout_charger_variables(c: &Cpu, b: &mut Cps2Bus) -> u64 {
    let mode = rd8(b, a5(c, ram::CONFIG));
    let mut p = Chemin::default();
    p.pris(mode == 18);
    if mode != 18 {
        p.pris(mode == 17);
    }
    p.duree(b, 0x2d358)
}

pub const VARIANTE: Native = Native {
    entry: 0x2d648,
    name: "config_variante",
    cost: |_, b| Chemin::default().duree(b, 0x2d648),
    run: variante,
    touches_sound: false,
    outputs: regs::d(0) | regs::a(6) | regs::CCR,
    exact_cost: true,
};

pub const DEFAUTS: Native = Native {
    entry: 0x2d7aa,
    name: "config_defauts",
    cost: cout_charger_defauts,
    run: charger_defauts,
    touches_sound: false,
    outputs: 0,
    exact_cost: true,
};

pub const VALIDER: Native = Native {
    entry: 0x2d662,
    name: "config_valider",
    cost: cout_valider,
    run: natif_valider,
    touches_sound: false,
    outputs: 0,
    exact_cost: true,
};

pub const VARIABLES: Native = Native {
    entry: 0x2d358,
    name: "config_variables",
    cost: cout_charger_variables,
    run: charger_variables,
    touches_sound: false,
    outputs: regs::d(0) | regs::d(1) | regs::a(4) | regs::CCR,
    exact_cost: true,
};
