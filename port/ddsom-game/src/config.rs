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
//! L'initialisation (0x2d30a, au démarrage et par la tâche de démarrage) lit l'EEPROM ; si la lecture
//! échoue ou que la signature ne correspond pas à la variante, les valeurs par défaut sont réécrites,
//! sinon les réglages lus sont validés ; puis les variables du jeu en sont tirées.

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::Cpu;

use crate::mem::{rd16, rd32, rd8, rts, wr16, wr32, wr8};
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
    /// Transfert EEPROM demandé : nombre de mots moins un, début (voir `eeprom.rs`).
    pub const MOTS: u32 = 0xffff_9e92;
    pub const DEBUT: u32 = 0xffff_9e96;
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
    let index = rd16(b, a5(c, ram::VARIANTE));
    let deplacement = crate::mem::prog16(b, TABLE_VARIANTES.wrapping_add(index as i16 as u32));
    set_mot(&mut c.d[0], deplacement);
    c.flags_logic(deplacement as u32, Size::Word);
    c.a[6] = TABLE_VARIANTES.wrapping_add(deplacement as i16 as u32);
    rts(c, b);
}

/// Bloc des valeurs par défaut de la variante courante.
fn bloc_defauts(b: &mut Cps2Bus) -> u32 {
    let index = crate::mem::rd16(b, crate::mem::A5 + ram::VARIANTE);
    let deplacement = crate::mem::prog16(b, TABLE_VARIANTES.wrapping_add(index as i16 as u32));
    TABLE_VARIANTES.wrapping_add(deplacement as i16 as u32)
}

/// Octets du bloc par défaut et variables du jeu qu'ils alimentent, dans l'ordre des comparaisons de
/// l'original (0x2d5c6).
const REGLAGES: [(u32, u32); 12] =
    [(0, 0x1c), (2, 0x23), (3, 0x26), (4, 0x27), (5, 0x28), (1, 0x1d), (6, 0x29), (7, 0x73), (8, 0x84), (9, 0x85), (0x10, 0x2a), (0x11, 0x2b)];

/// 0x2d5c6 : les réglages en cours sont-ils ceux d'usine (bloc par défaut de la variante) ?
pub fn is_factory(x: &mut crate::scene::Ctx) -> bool {
    let bloc = bloc_defauts(x.b);
    REGLAGES.iter().all(|&(octet, variable)| rd8(x.b, bloc + octet) == x.v8(variable))
}

// ---------------------------------------------------------------------------------------------
// 0x2d7aa : valeurs par défaut

/// Recopie le bloc par défaut de la variante en deux exemplaires (exemplaire et secours), puis
/// efface la suite de la copie (les compteurs de comptabilité).
fn charger_defauts(c: &mut Cpu, b: &mut Cps2Bus) {
    defauts(c, b);
    rts(c, b);
}

fn defauts(c: &mut Cpu, b: &mut Cps2Bus) {
    let index = rd16(b, a5(c, ram::VARIANTE));
    let deplacement = crate::mem::prog16(b, TABLE_VARIANTES.wrapping_add(index as i16 as u32));
    let bloc = TABLE_VARIANTES.wrapping_add(deplacement as i16 as u32);
    let mut destination = a5(c, ram::CONFIG);
    for _ in 0..2 {
        for i in 0..24 {
            let mot = rd16(b, bloc + 2 * i);
            wr16(b, destination, mot);
            destination += 2;
        }
    }
    for i in 0..16 {
        wr16(b, destination + 2 * i, 0);
    }
    // restes de l'original : A1 et A6 en fin de zone, D0 = 0, D7 = 0xFFFF, drapeaux du dernier move
    c.a[1] = destination + 32;
    c.a[6] = bloc + 48;
    c.d[0] = 0;
    c.d[7] = 0xffff;
    c.flags_logic(0, Size::Word);
}

// ---------------------------------------------------------------------------------------------
// 0x2d662 : validation et réparation

/// Remplace la valeur `i` de la copie (et de son secours) par la valeur par défaut.
fn reparer(b: &mut Cps2Bus, copie: u32, defauts: u32, i: u32) {
    let d = rd8(b, defauts + i);
    wr8(b, copie + i, d);
    wr8(b, copie + SECOURS + i, d);
}

/// Vérifie chaque réglage de la copie en RAM selon ses bornes ; remplace les valeurs invalides par
/// celles du bloc par défaut de la variante. L'ordre des tests et ses particularités sont ceux de l'original : par exemple,
/// l'octet 4 n'est examiné que si l'octet 3 était invalide.
fn valider(b: &mut Cps2Bus, c: &mut Cpu) {
    let copie = a5(c, ram::CONFIG);
    // l'original commence par 0x2d648 : A6 = bloc par défaut de la variante
    let variante = rd16(b, a5(c, ram::VARIANTE));
    let deplacement = crate::mem::prog16(b, TABLE_VARIANTES.wrapping_add(variante as i16 as u32));
    set_mot(&mut c.d[0], deplacement);
    let defauts = TABLE_VARIANTES.wrapping_add(deplacement as i16 as u32);
    c.a[6] = defauts;

    // octet 0 : au plus 9 (variantes 4 et 10) ou 18 ; la valeur 17 est tolérée pour 4 et 10
    let mut borne = 9;
    if !(variante == 4) && !(variante == 10) {
        borne = 18;
    }
    let mode = rd8(b, copie);
    if !(borne >= mode) {
        let mut reparer_0 = true;
        let mut variante_speciale = variante == 4;
        if !variante_speciale {
            variante_speciale = !(variante != 10);
        }
        if variante_speciale && (mode == 17) {
            reparer_0 = false;
        }
        if reparer_0 {
            reparer(b, copie, defauts, 0);
        }
    }
    // octet 1 : au plus 11
    let o1 = rd8(b, copie + 1);
    if !(o1 <= 11) {
        reparer(b, copie, defauts, 1);
    }
    // octet 2 : exactement 1 si le mode (éventuellement réparé) est 17, sinon 0 ou 1
    let mode = rd8(b, copie);
    let o2 = rd8(b, copie + 2);
    let valide_2 = if mode == 17 { o2 == 1 } else { o2 <= 1 };
    if !valide_2 {
        reparer(b, copie, defauts, 2);
    }
    // octets 3 et 4 : 0 ou 1 ; l'octet 4 n'est examiné que si l'octet 3 a dû être réparé
    let o3 = rd8(b, copie + 3);
    if !(o3 <= 1) {
        reparer(b, copie, defauts, 3);
        let o4 = rd8(b, copie + 4);
        if !(o4 <= 1) {
            reparer(b, copie, defauts, 4);
        }
    }
    // octet 5 : 0 ou 1
    let o5 = rd8(b, copie + 5);
    if !(o5 <= 1) {
        reparer(b, copie, defauts, 5);
    }
    // octet 6 : au plus 0, ou 1 pour la variante 6
    let borne6 = if variante != 6 { 0 } else { 1 };
    let o6 = rd8(b, copie + 6);
    if !(borne6 >= o6) {
        reparer(b, copie, defauts, 6);
    }
    // octet 7 : égal au défaut, ou supérieur de 2 à 10 au plus
    let o7 = rd8(b, copie + 7);
    let d7 = rd8(b, defauts + 7);
    if !(o7 == d7) && !(o7.wrapping_sub(2) <= 8) {
        reparer(b, copie, defauts, 7);
    }
    // octet 8 : 0 ou 1
    let o8 = rd8(b, copie + 8);
    if !(o8 <= 1) {
        reparer(b, copie, defauts, 8);
    }
    // octet 9 : entre 2 et 14
    let o9 = rd8(b, copie + 9) as i8;
    if (o9 < 2) || !(o9 <= 14) {
        reparer(b, copie, defauts, 9);
    }
    // octets 16 et 17 : au plus 7
    for i in [16, 17] {
        let o = rd8(b, copie + i);
        if !(o <= 7) {
            reparer(b, copie, defauts, i);
        }
    }
}

/// 0x2d662 appelé depuis Rust.
pub fn valider_copie(c: &mut Cpu, b: &mut Cps2Bus) {
    valider(b, c);
}

fn natif_valider(c: &mut Cpu, b: &mut Cps2Bus) {
    valider(b, c);
    rts(c, b);
}

// ---------------------------------------------------------------------------------------------
// 0x2d358 : de la copie en RAM aux variables du jeu

/// Recopie les réglages de la copie en RAM vers les variables du jeu et en déduit les réglages de
/// monnayage (`A5+0x1e..0x22`). Laisse D0, D1, A4 et les drapeaux comme l'original.
fn charger_variables(c: &mut Cpu, b: &mut Cps2Bus) {
    variables(c, b);
    rts(c, b);
}

fn variables(c: &mut Cpu, b: &mut Cps2Bus) {
    let a4 = a5(c, ram::CONFIG);
    c.a[4] = a4;
    for k in [0x23, 0x25, 0x24] {
        wr8(b, a5(c, k), 0);
    }
    let mode = rd8(b, a4);
    wr8(b, a5(c, ram::MODE_MONNAYAGE), mode);
    // pièces par crédit : table, ou 0x0101 pour les modes 17 (départ gratuit) et 18 (jeu gratuit)
    let (pieces, commun) = match mode {
        18 => {
            wr8(b, a5(c, 0x25), 1);
            (0x0101, false)
        }
        17 => {
            wr8(b, a5(c, 0x23), 1);
            wr8(b, a5(c, 0x24), 1);
            (0x0101, true)
        }
        _ => (crate::mem::prog16(b, TABLE_PIECES + 2 * mode as u32), false),
    };
    wr16(b, a5(c, ram::PIECES), pieces);
    if !commun {
        let o2 = rd8(b, a4 + 2);
        wr8(b, a5(c, 0x23), o2);
    }
    for (octet, k) in [(3, 0x26), (4, 0x27), (5, 0x28)] {
        let o = rd8(b, a4 + octet);
        wr8(b, a5(c, k), o);
    }
    let credit = rd8(b, a4 + 1);
    wr8(b, a5(c, ram::MODE_CREDIT), credit);
    let double = 2 * credit as u32;
    let a = crate::mem::prog8(b, TABLE_CREDIT_A + double);
    let a2 = crate::mem::prog8(b, TABLE_CREDIT_A + 1 + double);
    let b3 = crate::mem::prog8(b, TABLE_CREDIT_B + credit as u32);
    wr8(b, a5(c, 0x21), a);
    wr8(b, a5(c, 0x22), a2);
    wr8(b, a5(c, 0x20), b3);
    for (octet, k) in [(6, 0x29), (7, 0x73), (8, 0x84), (9, 0x85), (16, 0x2a), (17, 0x2b)] {
        let o = rd8(b, a4 + octet);
        wr8(b, a5(c, k), o);
    }
    wr8(b, a5(c, 0x72), 0);
    let dernier = {
        let mut dernier = 0;
        for (place, variable) in ram::COMPTEURS {
            dernier = rd32(b, a4 + place);
            wr32(b, a5(c, variable), dernier);
        }
        dernier
    };
    c.d[0] = double;
    c.d[1] = credit as u32;
    c.flags_logic(dernier, Size::Long);
}

pub const VARIANTE: Native = Native {
    entry: 0x2d648,
    name: "config_variante",
    run: crate::Corps::Processeur(variante),
    touches_sound: false,
    outputs: regs::d(0) | regs::a(6) | regs::CCR,
};

pub const DEFAUTS: Native = Native {
    entry: 0x2d7aa,
    name: "config_defauts",
    run: crate::Corps::Processeur(charger_defauts),
    touches_sound: false,
    outputs: 0,
};

pub const VALIDER: Native = Native {
    entry: 0x2d662,
    name: "config_valider",
    run: crate::Corps::Processeur(natif_valider),
    touches_sound: false,
    outputs: 0,
};

pub const VARIABLES: Native = Native {
    entry: 0x2d358,
    name: "config_variables",
    run: crate::Corps::Processeur(charger_variables),
    touches_sound: false,
    outputs: regs::d(0) | regs::d(1) | regs::a(4) | regs::CCR,
};

// ---------------------------------------------------------------------------------------------
// 0x2d30a : initialisation

/// Signature de la variante : octets 0x20-0x2f du bloc par défaut et de l'exemplaire 1.
const SIGNATURE: u32 = 0x20;
const SIGNATURE_LEN: u32 = 16;
/// Message « EEPROM ERROR » (12 caractères, attribut 8) affiché si l'écriture des valeurs par défaut
/// échoue, et boucle sans fin de l'original.
const MESSAGE_ERREUR: u32 = 0x2d7fc;
const CASE_ERREUR: u32 = 0x912938;
pub const BLOQUE: u32 = 0x2d7fa;

/// Compare la signature de la copie en RAM à celle de la variante, comme la boucle de l'original (registres
/// compris : D0 dernier octet attendu, D7 compteur, A1 et A6 après le dernier octet examiné).
fn signature_ok(c: &mut Cpu, b: &mut Cps2Bus) -> bool {
    let bloc = bloc_defauts(b);
    c.a[6] = bloc + SIGNATURE;
    c.a[1] = a5(c, ram::CONFIG) + SIGNATURE;
    c.d[7] = SIGNATURE_LEN - 1;
    for _ in 0..SIGNATURE_LEN {
        let attendu = rd8(b, c.a[6]);
        let lu = rd8(b, c.a[1]);
        c.d[0] = c.d[0] & !0xff | attendu as u32;
        c.a[6] += 1;
        c.a[1] += 1;
        if attendu != lu {
            return false;
        }
        let reste = (c.d[7] as u16).wrapping_sub(1);
        set_mot(&mut c.d[7], reste);
    }
    true
}

/// 0x2d30a : lecture de l'EEPROM, réglages par défaut si besoin, variables du jeu.
fn init(c: &mut Cpu, b: &mut Cps2Bus) {
    crate::eeprom::toute_eeprom(b);
    let lu = crate::eeprom::lire_verifier(b);
    let valide = lu && {
        // D0 de 0x2d648 (déplacement du bloc), comme l'original avant sa boucle
        let index = crate::mem::rd16(b, a5(c, ram::VARIANTE));
        set_mot(&mut c.d[0], crate::mem::prog16(b, TABLE_VARIANTES.wrapping_add(index as i16 as u32)));
        signature_ok(c, b)
    };
    if valide {
        valider(b, c);
        c.a[1] = a5(c, ram::CONFIG);
    } else {
        defauts(c, b);
        crate::eeprom::toute_eeprom(b);
        if !crate::eeprom::ecrire_verifier(b) {
            erreur(b);
            return;
        }
    }
    variables(c, b);
    rts(c, b);
}

/// 0x2d7d2 : « EEPROM ERROR » et blocage (jamais vu : l'EEPROM émulée ne refuse pas l'écriture).
fn erreur(b: &mut Cps2Bus) {
    for k in 0..12 {
        let ch = rd8(b, MESSAGE_ERREUR + k) as u16;
        crate::mem::wr16(b, CASE_ERREUR + 0x80 * k, ch);
        crate::mem::wr16(b, CASE_ERREUR + 0x80 * k + 2, 8);
    }
    crate::kernel::bloquer(b, BLOQUE);
}

pub const INIT: Native = Native {
    entry: 0x2d30a,
    name: "config_init",
    run: crate::Corps::Processeur(init),
    touches_sound: false,
    outputs: regs::d(0) | regs::d(1) | regs::a(4) | regs::CCR,
};

// ---------------------------------------------------------------------------------------------
// Menu TEST, 7. CONFIGURATION : réglages d'usine et enregistrement

/// 0x2d546 : les réglages d'usine de la variante dans les variables du jeu.
pub fn reset_factory(x: &mut crate::scene::Ctx) {
    let bloc = bloc_defauts(x.b);
    for (octet, variable) in REGLAGES {
        let v = rd8(x.b, bloc + octet);
        x.set_v8(variable, v);
    }
}

/// 0x2d47c : les variables du jeu dans les deux exemplaires de la copie en RAM (avec la signature de la
/// variante), écrites dans l'EEPROM, puis relues dans les variables (0x2d358). Faux si l'écriture a échoué :
/// « EEPROM ERROR » est affiché et le PC posé sur la boucle sans fin de l'original.
pub fn save(c: &mut Cpu, b: &mut Cps2Bus) -> bool {
    let bloc = bloc_defauts(b);
    for copie in [a5(c, ram::CONFIG), a5(c, ram::CONFIG) + SECOURS] {
        for (octet, variable) in REGLAGES {
            let v = rd8(b, a5(c, variable));
            crate::mem::wr8(b, copie + octet, v);
        }
        for k in 0..SIGNATURE_LEN {
            let v = rd8(b, bloc + SIGNATURE + k);
            crate::mem::wr8(b, copie + SIGNATURE + k, v);
        }
    }
    // les deux exemplaires : 0x30 mots
    crate::mem::wr16(b, a5(c, ram::MOTS), 0x2f);
    crate::mem::wr16(b, a5(c, ram::DEBUT), 0);
    if !crate::eeprom::ecrire_verifier(b) {
        erreur(b);
        return false;
    }
    variables(c, b);
    true
}
