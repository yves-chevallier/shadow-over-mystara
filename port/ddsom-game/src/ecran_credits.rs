//! Écran des crédits (C1) : textes et chiffres des crédits et des pièces affichés pendant
//! l'attente d'un départ (`0x148be` texte du crédit, `0x14b74` texte d'un joueur, `0x14c1e` effacement,
//! `0x14652` chiffres). Ils sont appelés par la tâche des crédits (`tache_credits.rs`) avec A1 = le
//! premier monnayeur (`A5+0x44e`, voir `credits.rs`) et ne cèdent jamais la main.
//!
//! Les textes sont des tables de pointeurs de la ROM, indexées par le nombre de crédits ou de pièces
//! en attente : l'index se calcule en 4 octets par pas. Les chiffres s'écrivent dans la couche de
//! caractères (GFX RAM 0x912000) par `0x14e46` (`credits::ECRIRE_CHIFFRE`).
//!
//! Variables : `A5+0x20` monnayeur partagé, `+0x21` nombre de monnayeurs moins un, `+0x22` compteurs
//! séparés, `+0x1e` pièces par crédit, `+0x456` écran à gros chiffres. Dans un monnayeur : `+1` pièces
//! en attente, `+2` crédits, `+9` crédit à deux pièces, `+0xb` dernier texte affiché.

use crate::mem::{rd32, rd8, rts, set_low_byte, set_low_word, wr8, A5};
use crate::scene::Ctx;
use crate::Native;

/// Sous-routines d'origine.
mod ext {
    pub const PRINT: u32 = 0xd5e;
    pub const PRINT_TALL: u32 = 0xdca;
    pub const PRINT_STRINGS: u32 = 0x1136;
    pub const PRINT_AT: u32 = 0x1234;
    pub const DIGIT: u32 = 0x14e46;
    pub const COUNT_TEXT: u32 = 0x14b74;
}

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

const CHARACTERS: u32 = 0x912000;

fn v8(x: &mut Ctx, off: u32) -> u8 {
    rd8(x.b, A5 + off)
}

/// Octet `off` de l'enregistrement pointé par le registre A1.
fn a1(x: &mut Ctx, off: u32) -> u8 {
    rd8(x.b, x.c.a[1] + off)
}

fn finish(x: &mut Ctx) {
    rts(x.c, x.b);
}

/// Lit le pointeur de texte `table[index]` (A4 de l'original) puis appelle `routine`.
fn print_from(x: &mut Ctx, table: u32, index: u32, routine: u32) {
    x.c.a[4] = rd32(x.b, table + (index & 0xffff));
    x.call(routine);
}

/// Écrit un chiffre (D0 = valeur, A4 = destination, D1 = attribut 1) par `0x14e46`.
fn digit(x: &mut Ctx, value: u8) {
    x.c.d[0] = value as u32;
    x.c.d[1] = 1;
    x.call(ext::DIGIT);
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

/// Efface le rectangle de texte du crédit.
fn clear_credit_text(x: &mut Ctx) {
    crate::text::clear_box(x.b, table::RECT);
}

/// Index (en octets) du texte de crédit : 4 octets par crédit entier, plus une fraction de pièces
/// quand le crédit est à une pièce.
fn credit_index(x: &mut Ctx) -> u32 {
    let (coins, credits, per_credit) = (a1(x, 1), a1(x, 2), v8(x, 0x1e));
    let mut d0: u32;
    if a1(x, 9) != 0 {
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

/// `0x14b74` : texte du crédit d'un joueur, à la position D4 (ligne), D5 (colonne), D6 (attribut).
fn count_text(x: &mut Ctx) {
    let index = credit_index(x);
    let two_coins = (a1(x, 9) != 0) as usize;
    let variant = match v8(x, 0x21) {
        3 => 2,
        2 => 1,
        _ => 0,
    };
    print_from(x, table::PLAYER_TEXT[variant][two_coins], index, ext::PRINT_AT);
    finish(x);
}

/// Appelle `0x14b74` pour le monnayeur `a1` (décalage depuis A5) aux coordonnées données.
fn player_text(x: &mut Ctx, monnayeur: u32, d4: u16, d5: u16) {
    x.c.a[1] = A5 + monnayeur;
    set_low_word(&mut x.c.d[4], d4);
    set_low_word(&mut x.c.d[5], d5);
    set_low_word(&mut x.c.d[6], 0xd);
    x.call(ext::COUNT_TEXT);
}

// ---------------------------------------------------------------------------------------------
// 0x148be : texte du crédit

fn credit_text(x: &mut Ctx) {
    if v8(x, 0x22) != 0 {
        return credit_text_separate(x);
    }
    let two_coins = a1(x, 9) != 0;
    if two_coins {
        // crédit à deux pièces : le texte suit le nombre de crédits, plafonné selon le partage
        let limit = if v8(x, 0x20) == 0 {
            4
        } else if v8(x, 0x20) == 1 {
            6
        } else {
            8
        };
        let mut d0 = a1(x, 2) as i8;
        if d0 > limit {
            d0 = limit;
        }
        let index = (d0 as u8 as u32) << 2;
        if index as u8 == a1(x, 0xb) {
            return finish(x);
        }
        wr8(x.b, x.c.a[1] + 0xb, index as u8);
        clear_credit_text(x);
        let index = a1(x, 0xb) as u32;
        print_from(x, table::TEXT_TWO_COINS, index, ext::PRINT_STRINGS);
        return finish(x);
    }
    let credits = a1(x, 2);
    let shared = v8(x, 0x20);
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
        d0 = fraction_index(x, d0);
    }
    wr8(x.b, x.c.a[1] + 0xb, d0);
    clear_credit_text(x);
    let table = if a1(x, 2) != 0 { table::TEXT_ONE } else { table::TEXT_ONE_EMPTY };
    let index = a1(x, 0xb) as u32;
    print_from(x, table, index, ext::PRINT_STRINGS);
    finish(x);
}

/// Texte intermédiaire (0x14912) : `d0` plus 4 octets par pièce manquante pour le crédit suivant.
fn fraction_index(x: &mut Ctx, d0: u8) -> u8 {
    let mut d1 = 0u8;
    let coins = a1(x, 1);
    if coins != 0 {
        let mut left = v8(x, 0x1e).wrapping_sub(coins);
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

/// Compteurs séparés (0x149aa) : un texte par joueur, aux positions de chaque configuration.
fn credit_text_separate(x: &mut Ctx) {
    clear_credit_text(x);
    const P1: u32 = 0x44e;
    const P2: u32 = 0x45e;
    const P3: u32 = 0x46e;
    const P4: u32 = 0x47e;
    let layout: &[(u32, u16, u16)] = if v8(x, 0x21) == 3 {
        &[(P1, 0, 1), (P2, 4, 0xd), (P3, 8, 0x19), (P4, 0xc, 0x25)]
    } else if v8(x, 0x21) == 2 {
        &[(P1, 0, 1), (P2, 4, 0x11), (P3, 8, 0x21)]
    } else if v8(x, 0x20) == 1 {
        &[(P1, 0, 1), (P1, 4, 0x11), (P2, 8, 0x21)]
    } else if v8(x, 0x20) == 2 {
        if v8(x, 0x21) == 1 {
            &[(P1, 0, 0xa), (P2, 4, 0x18)]
        } else {
            &[(P1, 0, 1), (P1, 4, 0xd), (P2, 8, 0x19), (P2, 0xc, 0x25)]
        }
    } else {
        &[(P1, 0, 0xa), (P2, 4, 0x18)]
    };
    for &(monnayeur, d4, d5) in layout {
        player_text(x, monnayeur, d4, d5);
    }
    x.c.a[1] = A5 + P1;
    finish(x);
}

// ---------------------------------------------------------------------------------------------
// 0x14652 : chiffres des crédits et des pièces

/// Variante du tableau de joueurs : 0 pour un monnayeur, 2 pour deux, 5 pour trois et quatre.
fn rank(x: &mut Ctx) -> u8 {
    match v8(x, 0x21) {
        1 => 0,
        2 => 2,
        _ => 5,
    }
}

fn digits(x: &mut Ctx) {
    let tall = v8(x, 0x456) != 0;
    if v8(x, 0x21) == 0 || v8(x, 0x22) == 0 {
        return if tall { digits_tall_single(x) } else { digits_single(x) };
    }
    let mut a2 = A5 + 0x44e;
    for player in 0..=v8(x, 0x21) as u32 {
        let index = (rank(x) as u32 + player) << 2;
        if tall {
            print_from(x, table::TALL_TEXT, index, ext::PRINT_TALL);
            x.c.a[4] = CHARACTERS;
            let offset = rd32(x.b, table::TALL_OFFSETS + index);
            x.c.a[4] = x.c.a[4].wrapping_add(offset as u16 as i16 as i32 as u32);
            let credits = rd8(x.b, a2 + 2);
            digit(x, credits);
            x.c.a[4] = x.c.a[4].wrapping_sub(0x7e);
            if v8(x, 0x21) != 3 {
                x.c.a[4] = x.c.a[4].wrapping_add(0x17c);
            }
            let coins = modulo(rd8(x.b, a2 + 1), v8(x, 0x1e));
            digit(x, coins);
            x.c.a[4] = x.c.a[4].wrapping_add(0xfe);
            let per = v8(x, 0x1e);
            digit(x, per);
        } else {
            let two_coins = a1(x, 9) != 0;
            print_from(x, table::DIGIT_TEXT[two_coins as usize], index, ext::PRINT);
            x.c.a[4] = CHARACTERS;
            // la table des décalages se lit relativement au PC (programme déchiffré)
            let mut offset = x.prog16(table::DIGIT_OFFSETS + index + 2) as u32;
            if a1(x, 9) != 0 {
                offset = rd32(x.b, table::DIGIT_OFFSETS_TWO_COINS + index);
            }
            x.c.a[4] = x.c.a[4].wrapping_add(offset as u16 as i16 as i32 as u32);
            let credits = rd8(x.b, a2 + 2);
            digit(x, credits);
        }
        a2 += 0x10;
    }
    finish(x);
}

/// Un seul tableau (0x14700) : crédits d'un monnayeur.
fn digits_single(x: &mut Ctx) {
    let two_coins = a1(x, 9) != 0;
    print_from(x, table::DIGIT_TEXT[two_coins as usize], 0x24, ext::PRINT);
    x.c.a[4] = if a1(x, 9) != 0 { 0x912df4 } else { 0x912e74 };
    let credits = v8(x, 0x450);
    digit(x, credits);
    finish(x);
}

/// Gros chiffres, un seul tableau (0x1484c).
fn digits_tall_single(x: &mut Ctx) {
    print_from(x, table::TALL_TEXT, 0x24, ext::PRINT_TALL);
    x.c.a[4] = 0x912cf4;
    let credits = v8(x, 0x450);
    digit(x, credits);
    x.c.a[4] = x.c.a[4].wrapping_add(0xfe);
    let coins = modulo(v8(x, 0x44f), v8(x, 0x1e));
    digit(x, coins);
    x.c.a[4] = x.c.a[4].wrapping_add(0xfe);
    let per = v8(x, 0x1e);
    digit(x, per);
    finish(x);
}

macro_rules! native {
    ($nom:ident, $entree:expr, $corps:expr) => {
        pub const $nom: Native = Native {
            entry: $entree,
            name: stringify!($nom),
            run: |c, b| {
                let mut x = Ctx::new(c, b);
                x.c.a[5] = A5;
                let corps: fn(&mut Ctx) = $corps;
                corps(&mut x);
            },
            touches_sound: false,
            outputs: 0,
        };
    };
}

native!(CREDIT_TEXT, 0x148be, credit_text);
native!(DIGITS, 0x14652, digits);
native!(COUNT_TEXT, 0x14b74, count_text);
native!(CLEAR_CREDIT_TEXT, 0x14c1e, |x| {
    clear_credit_text(x);
    finish(x)
});

pub const NATIVES: &[Native] = &[CREDIT_TEXT, DIGITS, COUNT_TEXT, CLEAR_CREDIT_TEXT];
