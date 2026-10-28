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

/// L'étiquette est empoisonnée au site d'appel `site` (`fichier:ligne:colonne`). Une entrée `étiquette@motif` ne vaut
/// que pour les sites qui contiennent le motif (`motion@families/sobj`) ; une entrée précédée de `!` exclut ces sites
/// (`motion,!motion@sobj.rs:120`), pour isoler par bissection le lecteur d'un registre.
fn empoisonne(etiquette: &str, site: &str) -> bool {
    let mut actif = false;
    for x in etiquettes() {
        let (exclu, x) = match x.strip_prefix('!') {
            Some(x) => (true, x),
            None => (false, x.as_str()),
        };
        let (nom, motif) = match x.split_once('@') {
            Some((n, m)) => (n, Some(m)),
            None => (x, None),
        };
        if (nom == etiquette || nom == "tout") && motif.map_or(true, |m| site.contains(m)) {
            if exclu {
                return false;
            }
            actif = true;
        }
    }
    actif
}

/// Sonde d'un appel par adresse (`crate::call`) : avec l'étiquette `appel_<adresse en hexa>` dans `DDSOM_POISON`, les
/// registres et drapeaux que la routine appelée a changés (comparés à `avant`) sont inversés au retour, au site
/// `site`. Si la validation reste VALIDE, aucune suite ne relit ce que la routine laisse : l'appel peut devenir celui
/// d'un cœur sans registres.
pub fn sonder_appel(c: &mut Cpu, adresse: u32, avant: &([u32; 8], [u32; 8], u16), site: &std::panic::Location) {
    if etiquettes().is_empty() {
        return;
    }
    let etiquette = format!("appel_{adresse:x}");
    let site = site.to_string();
    if !empoisonne(&etiquette, &site) {
        return;
    }
    let mut masque = 0;
    for i in 0..8 {
        if c.d[i] != avant.0[i] {
            masque |= regs::d(i as u32);
        }
        if i < 7 && c.a[i] != avant.1[i] {
            masque |= regs::a(i as u32);
        }
    }
    if (c.sr ^ avant.2) & 0x1f != 0 {
        masque |= regs::CCR;
    }
    if std::env::var_os("DDSOM_POISON_TRACE").is_some() {
        eprintln!("POISON {etiquette} {site} masque={masque:05x}");
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

/// Registres que l'adaptateur `etiquette` écrit comme l'original et dont on veut prouver qu'ils sont morts
/// (masque de `crate::regs` : `regs::a(1) | regs::d(6) | regs::CCR`...). Avec `DDSOM_POISON`, ils sont inversés.
#[track_caller]
pub fn registres_morts(c: &mut Cpu, etiquette: &str, masque: u32) {
    if etiquettes().is_empty() {
        return;
    }
    // le site est celui de l'appelant de l'adaptateur (`#[track_caller]` sur les adaptateurs et les méthodes de
    // `objects::Courant`)
    let site = std::panic::Location::caller().to_string();
    if !empoisonne(etiquette, &site) {
        return;
    }
    // DDSOM_POISON_TRACE=1 : chaque empoisonnement, avec son site d'appel et A1 (diagnostic d'un écart)
    if std::env::var_os("DDSOM_POISON_TRACE").is_some() {
        eprintln!("POISON {etiquette} {site} a1={:06x}", c.a[1]);
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
