//! Contrôles de la refactorisation (`docs/guide-refactorisation.md`, « Registres morts »).
//!
//! Une routine convertie en fonction Rust ordinaire cesse d'écrire les registres que l'original laissait (A1 d'une
//! allocation, D0/D1/A4 d'une libération, drapeaux...). Avant de les abandonner, il faut savoir que personne ne les
//! relit : l'appelant Rust, mais aussi la suite (gestionnaire suivant de la boucle de dispatch, tâche qui hérite des
//! registres, code qui teste un drapeau). L'adaptateur `_68k` qui les écrit encore appelle
//! [`registres_morts`] juste après : avec `DDSOM_POISON=étiquette,...`, ces registres sont alors **inversés** (valeur
//! complémentée, drapeaux inversés), ce qui garantit une valeur fausse. Si la validation reste VALIDE, aucun lecteur
//! n'existe sur les scénarios : l'adaptateur peut être remplacé par la fonction pure. Sinon l'écart désigne le
//! lecteur (image, adresse) ; on garde l'écriture à cet endroit précis, commentée.
//!
//! Sans la variable, `registres_morts` ne coûte qu'une lecture atomique.

use std::sync::OnceLock;

use m68k::Cpu;

use crate::regs;

/// Étiquettes empoisonnées (`DDSOM_POISON`, séparées par des virgules ; `tout` les prend toutes).
fn etiquettes() -> &'static [String] {
    static E: OnceLock<Vec<String>> = OnceLock::new();
    E.get_or_init(|| {
        std::env::var("DDSOM_POISON").map(|v| v.split(',').map(str::to_owned).collect()).unwrap_or_default()
    })
}

/// L'étiquette est empoisonnée.
pub fn poison_actif(etiquette: &str) -> bool {
    let e = etiquettes();
    !e.is_empty() && e.iter().any(|x| x == etiquette || x == "tout")
}

/// Registres que l'adaptateur `etiquette` écrit comme l'original et dont on veut prouver qu'ils sont morts
/// (masque de `crate::regs` : `regs::a(1) | regs::d(6) | regs::CCR`...). Avec `DDSOM_POISON`, ils sont inversés.
pub fn registres_morts(c: &mut Cpu, etiquette: &str, masque: u32) {
    if !poison_actif(etiquette) {
        return;
    }
    for i in 0..8 {
        if masque & regs::d(i) != 0 {
            c.d[i as usize] = !c.d[i as usize];
        }
        if i < 7 && masque & regs::a(i) != 0 {
            c.a[i as usize] = !c.a[i as usize];
        }
    }
    if masque & regs::CCR != 0 {
        c.sr ^= 0x1f;
    }
}
