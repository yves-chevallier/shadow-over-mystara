//! Écran des crédits (C1) : textes et chiffres des crédits et des pièces affichés pendant
//! l'attente d'un départ (`0x148be` texte du crédit, `0x14b74` texte d'un joueur, `0x14c1e` effacement,
//! `0x14652` chiffres). La tâche des crédits (`tache_credits.rs`) les appelle avec le premier monnayeur
//! (`A5+0x44e`, voir `credits.rs`, A1 de l'original) ; ils ne cèdent jamais la main.
//!
//! Les textes sont des tables de pointeurs de la ROM, indexées par le nombre de crédits ou de pièces
//! en attente : l'index se calcule en 4 octets par pas. Les chiffres s'écrivent dans la couche de
//! caractères (GFX RAM 0x912000) par `credits::ecrire_chiffre` (0x14e46).
//!
//! Variables : `A5+0x20` monnayeur partagé, `+0x21` nombre de monnayeurs moins un, `+0x22` compteurs
//! séparés, `+0x1e` pièces par crédit, `+0x456` écran à gros chiffres. Dans un monnayeur : `+1` pièces
//! en attente, `+2` crédits, `+9` crédit à deux pièces, `+0xb` dernier texte affiché.

use cps2::Cps2Bus;

use crate::credits::ecrire_chiffre;
use crate::mem::{prog16, rd32, rd8, set_low_byte, wr8, A5};
use crate::text;

/// Tables de la ROM : pointeurs de textes selon le crédit (une table par variante), et zone effacée.
mod table {
    pub const TEXT_ONE: u32 = 0x1ae10;
    pub const TEXT_ONE_EMPTY: u32 = 0x1ae80;
    pub const TEXT_TWO_COINS: u32 = 0x1aea4;
    pub const PLAYER_TEXT: [[u32; 2]; 3] = [[0x1aecc, 0x1af18], [0x1af24, 0x1af70], [0x1af7c, 0x1afc8]];
    pub const DIGIT_TEXT: [u32; 2] = [0x1ad98, 0x1ade8];
    pub const TALL_TEXT: u32 = 0x1adc0;
    /// Décalages (dans 0x912000) des chiffres de chaque joueur.
    pub const DIGIT_OFFSETS: u32 = 0x14740;
    pub const DIGIT_OFFSETS_TWO_COINS: u32 = 0x14764;
    pub const TALL_OFFSETS: u32 = 0x1489a;
    pub const RECT: u32 = 0x14e54;
}

/// Premier monnayeur (`A5+0x44e`).
pub const PREMIER_MONNAYEUR: u32 = A5 + 0x44e;

const CHARACTERS: u32 = 0x912000;

fn v8(b: &mut Cps2Bus, off: u32) -> u8 {
    rd8(b, A5 + off)
}

/// Pointeur de texte `table[index]` (A4 de l'original).
fn texte(b: &mut Cps2Bus, table: u32, index: u32) -> u32 {
    rd32(b, table + (index & 0xffff))
}

/// Écrit un chiffre (attribut 1) par `0x14e46` ; rend la case qui suit.
fn digit(b: &mut Cps2Bus, case: u32, value: u8) -> u32 {
    ecrire_chiffre(b, case, value, 1)
}

/// Reste de `value` modulo `by` par soustractions (`sub.b ; bgt`), comme l'original.
fn modulo(value: u8, by: u8) -> u8 {
    let mut v = value;
    loop {
        v = v.wrapping_sub(by);
        if (v as i8) <= 0 {
            break;
        }
    }
    v.wrapping_add(by)
}

// ---------------------------------------------------------------------------------------------
// 0x14c1e et 0x14b74

/// 0x14c1e : efface le rectangle de texte du crédit.
pub fn clear_credit_text(b: &mut Cps2Bus) {
    text::clear_box(b, table::RECT);
}

/// Index (en octets) du texte de crédit du monnayeur `m` : 4 octets par crédit entier, plus une fraction de pièces
/// quand le crédit est à une pièce.
fn credit_index(b: &mut Cps2Bus, m: u32) -> u32 {
    let (coins, credits, per_credit) = (rd8(b, m + 1), rd8(b, m + 2), v8(b, 0x1e));
    let mut d0: u32;
    if rd8(b, m + 9) != 0 {
        d0 = (credits as u32) << 2 & 0xffff;
        if d0 as u8 > 8 {
            set_low_byte(&mut d0, 8);
        }
    } else {
        d0 = 0;
        if credits == 0 {
            d0 = (per_credit as u32) << 2;
            if coins != 0 {
                let mut byte = 0x28u8;
                let mut left = per_credit.wrapping_sub(coins);
                loop {
                    byte = byte.wrapping_add(4);
                    left = left.wrapping_sub(1);
                    if left == 0 {
                        break;
                    }
                }
                set_low_byte(&mut d0, byte);
            }
        }
    }
    d0
}

/// `0x14b74` : texte du crédit du monnayeur `m` en colonne `col` (D5), ligne 0xd (D6).
fn count_text(b: &mut Cps2Bus, m: u32, col: u8) {
    let index = credit_index(b, m);
    let two_coins = (rd8(b, m + 9) != 0) as usize;
    let variant = match v8(b, 0x21) {
        3 => 2,
        2 => 1,
        _ => 0,
    };
    let t = texte(b, table::PLAYER_TEXT[variant][two_coins], index);
    text::print_tall_at(b, t, col, 0xd);
}

// ---------------------------------------------------------------------------------------------
// 0x148be : texte du crédit

/// `0x148be` : texte du crédit, d'après le premier monnayeur `m`.
pub fn credit_text(b: &mut Cps2Bus, m: u32) {
    if v8(b, 0x22) != 0 {
        return credit_text_separate(b);
    }
    let two_coins = rd8(b, m + 9) != 0;
    if two_coins {
        // crédit à deux pièces : le texte suit le nombre de crédits, plafonné selon le partage
        let limit = match v8(b, 0x20) {
            0 => 4,
            1 => 6,
            _ => 8,
        };
        let d0 = (rd8(b, m + 2) as i8).min(limit);
        let index = (d0 as u8 as u32) << 2;
        if index as u8 == rd8(b, m + 0xb) {
            return;
        }
        wr8(b, m + 0xb, index as u8);
        clear_credit_text(b);
        let index = rd8(b, m + 0xb) as u32;
        let t = texte(b, table::TEXT_TWO_COINS, index);
        text::print_tall(b, t);
        return;
    }
    let credits = rd8(b, m + 2);
    let shared = v8(b, 0x20);
    // 4 octets par crédit entier jusqu'à 3 (selon le partage), puis la fraction de pièces
    let mut d0 = 0u8;
    let mut fraction = true;
    if credits >= 2 {
        d0 = 0x24;
        if shared == 0 {
            fraction = false;
        } else if credits >= 3 {
            d0 = 0x48;
            if shared == 1 {
                fraction = false;
            } else if credits >= 4 {
                d0 = 0x6c;
                fraction = false;
            }
        }
    }
    if fraction {
        d0 = fraction_index(b, m, d0);
    }
    wr8(b, m + 0xb, d0);
    clear_credit_text(b);
    let table = if rd8(b, m + 2) != 0 { table::TEXT_ONE } else { table::TEXT_ONE_EMPTY };
    let index = rd8(b, m + 0xb) as u32;
    let t = texte(b, table, index);
    text::print_tall(b, t);
}

/// Texte intermédiaire (0x14912) : `d0` plus 4 octets par pièce manquante pour le crédit suivant.
fn fraction_index(b: &mut Cps2Bus, m: u32, d0: u8) -> u8 {
    let mut d1 = 0u8;
    let coins = rd8(b, m + 1);
    if coins != 0 {
        let mut left = v8(b, 0x1e).wrapping_sub(coins);
        loop {
            d1 = d1.wrapping_add(4);
            left = left.wrapping_sub(1);
            if left == 0 {
                break;
            }
        }
    }
    d0.wrapping_add(d1)
}

/// Compteurs séparés (0x149aa) : un texte par joueur, aux colonnes de chaque configuration.
fn credit_text_separate(b: &mut Cps2Bus) {
    clear_credit_text(b);
    const P1: u32 = 0x44e;
    const P2: u32 = 0x45e;
    const P3: u32 = 0x46e;
    const P4: u32 = 0x47e;
    // (monnayeur, colonne) ; l'original passe aussi une ligne (D4 : 0, 4, 8, 0xc) que le texte ne lit pas
    let layout: &[(u32, u8)] = if v8(b, 0x21) == 3 {
        &[(P1, 1), (P2, 0xd), (P3, 0x19), (P4, 0x25)]
    } else if v8(b, 0x21) == 2 {
        &[(P1, 1), (P2, 0x11), (P3, 0x21)]
    } else if v8(b, 0x20) == 1 {
        &[(P1, 1), (P1, 0x11), (P2, 0x21)]
    } else if v8(b, 0x20) == 2 {
        if v8(b, 0x21) == 1 {
            &[(P1, 0xa), (P2, 0x18)]
        } else {
            &[(P1, 1), (P1, 0xd), (P2, 0x19), (P2, 0x25)]
        }
    } else {
        &[(P1, 0xa), (P2, 0x18)]
    };
    for &(monnayeur, col) in layout {
        count_text(b, A5 + monnayeur, col);
    }
}

// ---------------------------------------------------------------------------------------------
// 0x14652 : chiffres des crédits et des pièces

/// Variante du tableau de joueurs : 0 pour un monnayeur, 2 pour deux, 5 pour trois et quatre.
fn rank(b: &mut Cps2Bus) -> u8 {
    match v8(b, 0x21) {
        1 => 0,
        2 => 2,
        _ => 5,
    }
}

/// Case de la couche de caractères à un décalage signé (mot) de son début.
fn case(offset: u32) -> u32 {
    CHARACTERS.wrapping_add(offset as u16 as i16 as i32 as u32)
}

/// `0x14652` : chiffres des crédits et des pièces, d'après le premier monnayeur `m`.
pub fn digits(b: &mut Cps2Bus, m: u32) {
    let tall = v8(b, 0x456) != 0;
    if v8(b, 0x21) == 0 || v8(b, 0x22) == 0 {
        return if tall { digits_tall_single(b) } else { digits_single(b, m) };
    }
    let mut a2 = A5 + 0x44e;
    for player in 0..=v8(b, 0x21) as u32 {
        let index = (rank(b) as u32 + player) << 2;
        if tall {
            let t = texte(b, table::TALL_TEXT, index);
            text::print_over(b, t);
            let offset = rd32(b, table::TALL_OFFSETS + index);
            let credits = rd8(b, a2 + 2);
            let mut at = digit(b, case(offset), credits);
            at = at.wrapping_sub(0x7e);
            if v8(b, 0x21) != 3 {
                at = at.wrapping_add(0x17c);
            }
            let coins = modulo(rd8(b, a2 + 1), v8(b, 0x1e));
            at = digit(b, at, coins);
            at = at.wrapping_add(0xfe);
            let per = v8(b, 0x1e);
            digit(b, at, per);
        } else {
            let two_coins = rd8(b, m + 9) != 0;
            let t = texte(b, table::DIGIT_TEXT[two_coins as usize], index);
            text::print(b, t);
            // la table des décalages se lit relativement au PC (programme déchiffré)
            let mut offset = prog16(b, table::DIGIT_OFFSETS + index + 2) as u32;
            if rd8(b, m + 9) != 0 {
                offset = rd32(b, table::DIGIT_OFFSETS_TWO_COINS + index);
            }
            let credits = rd8(b, a2 + 2);
            digit(b, case(offset), credits);
        }
        a2 += 0x10;
    }
}

/// Un seul tableau (0x14700) : crédits d'un monnayeur.
fn digits_single(b: &mut Cps2Bus, m: u32) {
    let two_coins = rd8(b, m + 9) != 0;
    let t = texte(b, table::DIGIT_TEXT[two_coins as usize], 0x24);
    text::print(b, t);
    let at = if rd8(b, m + 9) != 0 { 0x912df4 } else { 0x912e74 };
    let credits = v8(b, 0x450);
    digit(b, at, credits);
}

/// Gros chiffres, un seul tableau (0x1484c).
fn digits_tall_single(b: &mut Cps2Bus) {
    let t = texte(b, table::TALL_TEXT, 0x24);
    text::print_over(b, t);
    let credits = v8(b, 0x450);
    let mut at = digit(b, 0x912cf4, credits);
    at = at.wrapping_add(0xfe);
    let coins = modulo(v8(b, 0x44f), v8(b, 0x1e));
    at = digit(b, at, coins);
    at = at.wrapping_add(0xfe);
    let per = v8(b, 0x1e);
    digit(b, at, per);
}
