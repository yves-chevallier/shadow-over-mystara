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

/// L'étiquette est-elle empoisonnée au site d'appel `site` (`fichier:ligne:colonne`) ? Une entrée `étiquette@motif`
/// ne vaut que pour les sites qui contiennent le motif (`motion@families/sobj`) ; une entrée précédée de `!` exclut
/// ces sites (`motion,!motion@sobj.rs:120`), pour isoler par bissection le lecteur d'un registre. Rend les registres
/// à épargner : une entrée `étiquette@motif#a1+d0` empoisonne ce site sauf A1 et D0 (lecture déjà connue et écrite
/// en clair, pour chercher les autres).
fn gardes(etiquette: &str, site: &str) -> Option<u32> {
    let mut actif = None;
    for x in etiquettes() {
        let (exclu, x) = match x.strip_prefix('!') {
            Some(x) => (true, x),
            None => (false, x.as_str()),
        };
        let (x, garde) = match x.split_once('#') {
            Some((x, g)) => (x, masque_de(g)),
            None => (x, 0),
        };
        let (nom, motif) = match x.split_once('@') {
            Some((n, m)) => (n, Some(m)),
            None => (x, None),
        };
        if (nom == etiquette || nom == "tout") && motif.map_or(true, |m| site.contains(m)) {
            if exclu {
                return None;
            }
            actif = Some(actif.unwrap_or(0) | garde);
        }
    }
    actif
}

/// Masque `crate::regs` d'une liste `d0+a1+ccr`.
fn masque_de(liste: &str) -> u32 {
    liste
        .split('+')
        .map(|r| match (r.get(..1), r.get(1..).and_then(|n| n.parse::<u32>().ok())) {
            (Some("d"), Some(n)) if n < 8 => regs::d(n),
            (Some("a"), Some(n)) if n < 8 => regs::a(n),
            _ if r == "ccr" => regs::CCR | X,
            _ if r == "nzvc" => regs::CCR,
            _ if r == "x" => X,
            _ => panic!("DDSOM_POISON : registre {r:?}"),
        })
        .fold(0, |m, r| m | r)
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
    let Some(garde) = gardes(&etiquette, &site) else {
        return;
    };
    let mut masque = 0;
    for i in 0..8 {
        if c.d[i] != avant.0[i] {
            masque |= regs::d(i as u32);
        }
        if i < 7 && c.a[i] != avant.1[i] {
            masque |= regs::a(i as u32);
        }
    }
    if (c.sr ^ avant.2) & 0x0f != 0 {
        masque |= regs::CCR;
    }
    if (c.sr ^ avant.2) & 0x10 != 0 {
        masque |= X;
    }
    masque &= !garde;
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
    inverser_drapeaux(&mut c.sr, masque);
}

/// Registres d'une tâche qu'une étape peut hériter : D0-D7, A0-A4, A6 et les drapeaux (A5 est remis par chaque pas,
/// A7 est la pile).
const REGISTRES_DE_TACHE: u32 = 0xff | 0x1f00 | 0x4000 | regs::CCR | X;

/// Sondes : le drapeau X, à part des autres drapeaux (`regs::CCR` : N, Z, V et C), pour savoir lequel est relu.
const X: u32 = 1 << 17;

/// Inverse N, Z, V, C (`regs::CCR`) et X (`X`) selon le masque.
fn inverser_drapeaux(sr: &mut u16, masque: u32) {
    if masque & regs::CCR != 0 {
        *sr ^= 0x0f;
    }
    if masque & X != 0 {
        *sr ^= 0x10;
    }
}

/// Inverse les registres du masque (`crate::regs`).
fn inverser(d: &mut [u32; 8], a: &mut [u32], sr: &mut u16, masque: u32) {
    for i in 0..8 {
        if masque & regs::d(i) != 0 {
            d[i as usize] = !d[i as usize];
        }
        if i < 7 && masque & regs::a(i) != 0 {
            a[i as usize] = !a[i as usize];
        }
    }
    inverser_drapeaux(sr, masque);
}

/// Sonde d'une cession (`kernel::ceder`) : avec l'étiquette `cession` (`cession@<motif>` : le motif est cherché dans
/// le nom de l'étape de reprise, `Ending(S04Timer)`), les registres que la tâche retrouvera à sa reprise (D0-D7,
/// A0-A4, A6, drapeaux) sont inversés. Si la validation reste VALIDE, aucune étape de ces tâches ne relit, après une
/// cession, un registre d'avant : leur contexte de registres est mort (sinon il devient un champ nommé de l'état).
pub fn sonder_cession(d: &mut [u32; 8], a: &mut [u32; 7], sr: &mut u16, etape: &dyn std::fmt::Debug) {
    if etiquettes().is_empty() {
        return;
    }
    let site = format!("{etape:?}");
    let Some(garde) = gardes("cession", &site) else {
        return;
    };
    if std::env::var_os("DDSOM_POISON_TRACE").is_some() {
        eprintln!("POISON cession {site}");
    }
    inverser(d, a, sr, REGISTRES_DE_TACHE & !garde);
}

/// Sonde du retour d'une étape (`taches::executer`, pas qui rend `Flow::Done`) : avec l'étiquette `retour`
/// (`retour@<motif>`, motif cherché dans le nom de l'étape), tous les registres de la tâche (D0-D7, A0-A4, A6,
/// drapeaux) sont inversés avant la suite de l'appel en cours. VALIDE : aucune suite ne relit ce que l'étape (ou ce
/// qui précédait l'appel) laisse dans les registres.
pub fn sonder_retour(c: &mut Cpu, etape: &dyn std::fmt::Debug) {
    if etiquettes().is_empty() {
        return;
    }
    let site = format!("{etape:?}");
    let Some(garde) = gardes("retour", &site) else {
        return;
    };
    if std::env::var_os("DDSOM_POISON_TRACE").is_some() {
        eprintln!("POISON retour {site}");
    }
    let (d, a, sr) = (&mut c.d, &mut c.a, &mut c.sr);
    inverser(d, &mut a[..7], sr, REGISTRES_DE_TACHE & !garde);
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
    let Some(garde) = gardes(etiquette, &site) else {
        return;
    };
    // `regs::CCR` d'un adaptateur : tous les drapeaux, X compris
    let masque = if masque & regs::CCR != 0 { masque | X } else { masque } & !garde;
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
    inverser_drapeaux(&mut c.sr, masque);
}
