//! Contrôles de la refactorisation (`docs/guide-refactorisation.md`, « Registres morts »).
//!
//! Une routine convertie en fonction Rust ordinaire cesse d'écrire les registres que l'original laissait (A1 d'une
//! allocation, D0/D1/A4 d'une libération, drapeaux...). Avant de les abandonner, il faut savoir que personne ne les
//! relit : l'appelant Rust, mais aussi la suite (gestionnaire suivant de la boucle de dispatch, tâche qui hérite des
//! registres, code qui teste un drapeau). L'adaptateur `_68k` qui les écrit encore appelle
//! [`dead_regs`] juste après : avec `DDSOM_POISON=étiquette,...`, ces registres sont alors **inversés** (valeur
//! complémentée, drapeaux inversés), ce qui garantit une valeur fausse. Si la validation reste VALIDE, aucun lecteur
//! n'existe sur les scénarios : l'adaptateur peut être remplacé par la fonction pure. Sinon l'écart désigne le
//! lecteur (image, adresse) ; on garde l'écriture à cet endroit précis, commentée.
//!
//! Sans la variable, `dead_regs` ne coûte qu'une lecture atomique.

use std::sync::OnceLock;

use m68k::Cpu;

use crate::regs;

/// Étiquettes empoisonnées (`DDSOM_POISON`, séparées par des virgules ; `all` les prend toutes).
fn labels() -> &'static [String] {
    static E: OnceLock<Vec<String>> = OnceLock::new();
    E.get_or_init(|| {
        std::env::var("DDSOM_POISON").map(|v| v.split(',').map(str::to_owned).collect()).unwrap_or_default()
    })
}

/// L'étiquette est empoisonnée.
pub fn poison_active(label: &str) -> bool {
    let e = labels();
    !e.is_empty() && e.iter().any(|x| x == label || x == "tout")
}

/// Registres visés au site `site` (`file:line:column`, ou nom d'étape) par l'une des étiquettes `names`, ou
/// `None` si aucune entrée ne vise le site. Une entrée `étiquette@pattern` ne vaut que pour les sites qui contiennent le
/// motif (`motion@families/sobj`) ; `étiquette.d4` (`.a1`, `.ccr`) ne vise qu'un registre ; une entrée précédée de
/// `!` exclut ces sites ou ce registre (`motion,!motion@sobj.rs:120`, `exit,!exit.d4@ba310`), pour isoler par
/// bissection le lecteur d'un registre. Une entrée `étiquette@pattern#a1+d0` vise ce site sauf A1 et D0 (lecture déjà
/// connue et écrite en clair, pour chercher les autres) ; précédée de `=`, elle épargne ces registres sans viser le
/// site.
fn selection(names: &[&str], site: &str) -> Option<u32> {
    const ALL: u32 = 0x7fff | regs::CCR | X;
    let (mut vises, mut spared, mut active) = (0, 0, false);
    for x in labels() {
        let (excluded, x) = match x.strip_prefix('!') {
            Some(x) => (true, x),
            None => (false, x.as_str()),
        };
        let (guard_only, x) = match x.strip_prefix('=') {
            Some(x) => (true, x),
            None => (false, x),
        };
        let (x, guard) = match x.split_once('#') {
            Some((x, g)) => (x, mask_of(g)),
            None => (x, 0),
        };
        let (name, pattern) = match x.split_once('@') {
            Some((n, m)) => (n, Some(m)),
            None => (x, None),
        };
        let (name, reg_names) = match name.split_once('.') {
            Some((n, r)) => (n, mask_of(r)),
            None => (name, ALL),
        };
        if !(names.contains(&name) || name == "tout") || !pattern.map_or(true, |m| site.contains(m)) {
            continue;
        }
        if excluded {
            if reg_names == ALL {
                return None;
            }
            spared |= reg_names;
        } else {
            if !guard_only {
                active = true;
                vises |= reg_names;
            }
            spared |= guard;
        }
    }
    active.then_some(vises & !spared)
}

/// [`selection`] pour une seule étiquette.
fn guards(label: &str, site: &str) -> Option<u32> {
    selection(&[label], site)
}

/// Masque `crate::regs` d'une liste `d0+a1+ccr`.
fn mask_of(list: &str) -> u32 {
    list
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

/// Sonde d'un appel par adresse (`crate::call`) : avec l'étiquette `appel_<address en hexa>` dans `DDSOM_POISON`, les
/// registres et drapeaux que la routine appelée a changés (comparés à `before`) sont inversés au retour, au site
/// `site`. Si la validation reste VALIDE, aucune suite ne relit ce que la routine laisse : l'appel peut devenir celui
/// d'un cœur sans registres.
pub fn probe_call(c: &mut Cpu, address: u32, before: &([u32; 8], [u32; 8], u16), site: &std::panic::Location) {
    if labels().is_empty() {
        return;
    }
    let label = format!("appel_{address:x}");
    let site = site.to_string();
    // `call` seul vise tous les appels par adresse (`call@families/` : ceux des familles)
    let Some(chosen) = selection(&[&label, "appel"], &site) else {
        return;
    };
    invert_changes(c, before, !chosen, &label, &site);
}

/// Sonde de la sortie d'un gestionnaire d'objet (boucle de dispatch, `dispatch.rs`) : avec l'étiquette `exit` dans
/// `DDSOM_POISON`, les registres et drapeaux que le gestionnaire a changés (sauf A0 et D7, que la boucle relit) sont
/// inversés à son retour. Le site est `<looped> <handler>` en hexadécimal (`39f82 a17d0`) : `exit@a17d0` vise
/// un gestionnaire, `exit@39e86` une classe. Si la validation reste VALIDE, aucun gestionnaire suivant ni aucune suite
/// ne relit ce que le gestionnaire laisse : il peut devenir une fonction Rust sans registres de sortie.
pub fn probe_exit(c: &mut Cpu, before: &([u32; 8], [u32; 8], u16), looped: u32, handler: u32) {
    if labels().is_empty() {
        return;
    }
    let site = format!("{looped:x} {handler:x}");
    let Some(chosen) = selection(&["sortie"], &site) else {
        return;
    };
    invert_changes(c, before, !chosen | regs::a(0) | regs::d(7), "sortie", &site);
}

/// Inverse les registres et drapeaux changés depuis `before`, sauf ceux de `guard`.
fn invert_changes(c: &mut Cpu, before: &([u32; 8], [u32; 8], u16), guard: u32, label: &str, site: &str) {
    let mut mask = 0;
    for i in 0..8 {
        if c.d[i] != before.0[i] {
            mask |= regs::d(i as u32);
        }
        if i < 7 && c.a[i] != before.1[i] {
            mask |= regs::a(i as u32);
        }
    }
    if (c.sr ^ before.2) & 0x0f != 0 {
        mask |= regs::CCR;
    }
    if (c.sr ^ before.2) & 0x10 != 0 {
        mask |= X;
    }
    mask &= !guard;
    if std::env::var_os("DDSOM_POISON_TRACE").is_some() {
        eprintln!("POISON {label} {site} masque={mask:05x}");
    }
    let (d, a, sr) = (&mut c.d, &mut c.a, &mut c.sr);
    invert(d, &mut a[..7], sr, mask);
}

/// Registres d'une tâche qu'une étape peut hériter : D0-D7, A0-A4, A6 et les drapeaux (A5 est remis par chaque pas,
/// A7 est la pile).
pub const TASK_REGS: u32 = 0xff | 0x1f00 | 0x4000 | regs::CCR | X;

/// Sondes : le drapeau X, à part des autres drapeaux (`regs::CCR` : N, Z, V et C), pour savoir lequel est relu.
const X: u32 = 1 << 17;

/// Inverse N, Z, V, C (`regs::CCR`) et X (`X`) selon le masque.
fn invert_flags(sr: &mut u16, mask: u32) {
    if mask & regs::CCR != 0 {
        *sr ^= 0x0f;
    }
    if mask & X != 0 {
        *sr ^= 0x10;
    }
}

/// Inverse les registres du masque (`crate::regs`).
fn invert(d: &mut [u32; 8], a: &mut [u32], sr: &mut u16, mask: u32) {
    for i in 0..8 {
        if mask & regs::d(i) != 0 {
            d[i as usize] = !d[i as usize];
        }
        if i < 7 && mask & regs::a(i) != 0 {
            a[i as usize] = !a[i as usize];
        }
    }
    invert_flags(sr, mask);
}

/// Sonde du retour d'une étape (`tasks::execute`, pas qui rend `Flow::Done`) : avec l'étiquette `ret`
/// (`ret@<pattern>`, motif cherché dans le nom de l'étape), tous les registres de la tâche (D0-D7, A0-A4, A6,
/// drapeaux) sont inversés avant la suite de l'appel en cours. VALIDE : aucune suite ne relit ce que l'étape (ou ce
/// qui précédait l'appel) laisse dans les registres.
pub fn probe_return(c: &mut Cpu, step: &dyn std::fmt::Debug) {
    if labels().is_empty() {
        return;
    }
    let site = format!("{step:?}");
    let Some(chosen) = guards("retour", &site) else {
        return;
    };
    if std::env::var_os("DDSOM_POISON_TRACE").is_some() {
        eprintln!("POISON retour {site}");
    }
    let (d, a, sr) = (&mut c.d, &mut c.a, &mut c.sr);
    invert(d, &mut a[..7], sr, TASK_REGS & chosen);
}

/// Registres que l'adaptateur `label` écrit comme l'original et dont on veut prouver qu'ils sont morts
/// (masque de `crate::regs` : `regs::a(1) | regs::d(6) | regs::CCR`...). Avec `DDSOM_POISON`, ils sont inversés.
#[track_caller]
pub fn dead_regs(c: &mut Cpu, label: &str, mask: u32) {
    if labels().is_empty() {
        return;
    }
    // le site est celui de l'appelant de l'adaptateur (`#[track_caller]` sur les adaptateurs et les méthodes de
    // `objects::Current`)
    let site = std::panic::Location::caller().to_string();
    let Some(chosen) = guards(label, &site) else {
        return;
    };
    // `regs::CCR` d'un adaptateur : tous les drapeaux, X compris
    let mask = if mask & regs::CCR != 0 { mask | X } else { mask } & chosen;
    // DDSOM_POISON_TRACE=1 : chaque empoisonnement, avec son site d'appel et A1 (diagnostic d'un écart)
    if std::env::var_os("DDSOM_POISON_TRACE").is_some() {
        eprintln!("POISON {label} {site} a1={:06x}", c.a[1]);
    }
    for i in 0..8 {
        if mask & regs::d(i) != 0 {
            c.d[i as usize] = !c.d[i as usize];
        }
        if i < 7 && mask & regs::a(i) != 0 {
            c.a[i as usize] = !c.a[i as usize];
        }
    }
    invert_flags(&mut c.sr, mask);
}

/// Sonde des registres qu'une tâche retrouve quand l'ordonnanceur la lance (`kernel::launch`) : avec l'étiquette
/// `label` (`resume` : registres rendus par le contexte d'une tâche qui a cédé ; `neuve` : registres hérités par
/// une tâche neuve) dans `DDSOM_POISON`, les registres choisis (tous, ou `resume.d3`...), sauf ceux de `guard`, sont
/// inversés. Le site est l'étape lancée (`Dispatch(3)`...). Si la validation reste VALIDE, la tâche ne relit pas ces
/// registres : le contexte n'a pas à les garder.
pub fn probe_launch(c: &mut Cpu, label: &str, step: &dyn std::fmt::Debug, guard: u32) {
    if labels().is_empty() {
        return;
    }
    // sans espaces ni virgules : le site reste un seul mot d'une liste `DDSOM_POISON` (`tools/bissecte_poison.py`)
    let site: String = format!("{step:?}").chars().filter(|c| *c != ' ').map(|c| if c == ',' { ';' } else { c }).collect();
    let Some(chosen) = guards(label, &site) else {
        return;
    };
    let mask = TASK_REGS & chosen & !guard;
    if std::env::var_os("DDSOM_POISON_TRACE").is_some() {
        eprintln!("POISON {label} {site} masque={mask:05x}");
    }
    let (d, a, sr) = (&mut c.d, &mut c.a, &mut c.sr);
    invert(d, &mut a[..7], sr, mask);
}
