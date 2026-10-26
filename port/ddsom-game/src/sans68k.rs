//! Le jeu sans 68000 : natives seules et boucle d'image du noyau (voir `docs/noyau.md`, « Vers une boucle sans
//! 68000 »).
//!
//! Le `Cpu` de la machine n'est plus qu'un banc de registres (D0-D7, A0-A7, SR, PC) et de pile : l'exécuteur cherche
//! la native posée à l'adresse du PC et l'exécute, puis recommence (`while let Some(n) = lookup(c.pc)`). Une native
//! s'exécute toujours en entier, même quand elle déborderait sur l'interruption suivante : aucune n'est remplacée par
//! son original. Une adresse sans native, ou une native qui se déclare hors contrat (`families::COUT_HORS_CONTRAT`),
//! arrête net le jeu avec un diagnostic ([`ARRET`]) au lieu d'exécuter du code d'origine.
//!
//! ## L'image
//!
//! La machine garde son calendrier (`cps2::Machine::line_work`) : à chaque ligne, [`ligne`] fait le travail du
//! processeur, puis le Z80 joue sa ligne. Le travail du processeur n'a pas de durée :
//!
//! 1. l'interruption de VBLANK, levée à la fin de l'image précédente, est prise dès que le SR l'autorise (au début de
//!    l'image : l'ordonnanceur l'attend, le pas de jeu retenu aussi ; pendant le démarrage, quand `boot_init` démasque) ;
//!    elle empile PC et SR comme le processeur et lance la native du VBLANK (`0x574`), qui finit par `RTE` ;
//! 2. puis les natives s'enchaînent (ordonnanceur, tâches, cessions) jusqu'à ce que l'ordonnanceur tourne à vide
//!    (`crate::idle` : plus aucune tâche prête, ou seule la tâche du gel sans rien à faire), ou que le pas de jeu
//!    atteigne le plafond de l'image (`MAX_STEPS_PER_FRAME` passages en `0x1c4b0`, `cps2::StepCap` : le pas retenu
//!    s'exécute au début de l'image suivante, juste après le VBLANK) ;
//! 3. au démarrage, une native qui attend une position du faisceau (relâchement du Z80, `boot_init` :
//!    `demarrage.rs`) rend la main sans rien changer (PC et registres) : la ligne s'arrête là et elle réessaie à la
//!    suivante.
//!
//! Tout le travail de l'image se fait donc au début de sa première ligne (240), avant le Z80, comme à ×16 où il finit
//! en quelques lignes : seuls le VBLANK et l'envoi des commandes au Z80 (`0x68c`, qu'il appelle) touchent la RAM
//! partagée, et ils sont les premiers. Le surcadencement, `idle`, les durées des natives et les correctifs de la
//! machine (`patches`, qui s'appliquent au code d'origine) ne servent plus. Plafond de sûreté : au plus
//! [`PLAFOND_NATIVES`] natives par image ; au-delà, l'image s'arrête comme une image de l'original qui finit au milieu
//! d'une attente active (compté dans [`bilan`]).

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use cps2::{Cps2Bus, StepCap};
use m68k::Cpu;

use crate::mem::{rd32, wr16, wr32};
use crate::taches::table::lire;
use crate::Native;

/// Le jeu tourne sans 68000 (une seule machine du processus, celle du jeu : la machine de référence de `validate`
/// n'appelle jamais `call_at`).
static ACTIF: AtomicBool = AtomicBool::new(false);

pub fn actif() -> bool {
    ACTIF.load(Ordering::Relaxed)
}

/// Branche la boucle sans 68000 sur la machine (après `install`, dont seul le plafond de pas sert encore).
pub(crate) fn installer(m: &mut cps2::Machine) {
    ACTIF.store(true, Ordering::Relaxed);
    m.executor = None;
    m.idle = None;
    m.patches.clear();
    m.line_work = Some(ligne);
}

/// Natives au plus par image : une image ordinaire en exécute quelques milliers.
pub const PLAFOND_NATIVES: u32 = 1_000_000;

/// Images arrêtées par [`PLAFOND_NATIVES`].
static IMAGES_PLAFONNEES: AtomicU64 = AtomicU64::new(0);

/// Bilan pour `validate` : images arrêtées par le plafond de natives.
pub fn bilan() -> u64 {
    IMAGES_PLAFONNEES.load(Ordering::Relaxed)
}

thread_local! {
    /// Image en cours et natives exécutées depuis son début.
    static IMAGE: Cell<(u64, u32)> = const { Cell::new((u64::MAX, 0)) };
    /// Dernières natives exécutées (diagnostic d'un arrêt).
    static DERNIERES: RefCell<VecDeque<&'static Native>> = const { RefCell::new(VecDeque::new()) };
}

const SR_S: u16 = 0x2000;

/// Travail du processeur pendant une ligne (`cps2::Machine::line_work`).
pub fn ligne(c: &mut Cpu, b: &mut Cps2Bus, fin: u64, mut plafond: Option<&mut StepCap>) {
    if b.irq_raster {
        arreter(c, b, "interruption raster (niveau 4) : jamais armée par le jeu".into());
    }
    loop {
        let niveau = b.irq_level();
        if niveau > (c.sr >> 8 & 7) as u8 {
            interruption(c, b, niveau);
        } else if niveau == 0 && crate::idle(c, b) {
            // (une interruption masquée attend que l'ordonnanceur rouvre sa fenêtre)
            break;
        }
        if let Some(cap) = plafond.as_deref_mut().filter(|cap| cap.pc == c.pc) {
            if !cap.passage() {
                break;
            }
        }
        let avant = (c.pc, c.sr, c.d, c.a);
        pas(c, b);
        if (c.pc, c.sr, c.d, c.a) == avant {
            // rien n'a bougé : une native du démarrage attend sa position du faisceau
            break;
        }
        if !compter(b) {
            IMAGES_PLAFONNEES.fetch_add(1, Ordering::Relaxed);
            break;
        }
    }
    c.cycles = c.cycles.max(fin);
}

/// Une native de plus dans l'image ; faux quand le plafond est atteint.
fn compter(b: &Cps2Bus) -> bool {
    IMAGE.with(|i| {
        let (image, n) = i.get();
        let n = if image == b.frame { n + 1 } else { 1 };
        i.set((b.frame, n));
        n < PLAFOND_NATIVES
    })
}

/// Prise d'une interruption (autovecteur), comme le processeur : superviseur, PC et SR empilés, niveau dans le SR.
fn interruption(c: &mut Cpu, b: &mut Cps2Bus, niveau: u8) {
    m68k::Bus::irq_ack(b, niveau);
    let sr = c.sr;
    c.set_sr((sr | SR_S) & !m68k::SR_T);
    c.a[7] = c.a[7].wrapping_sub(4);
    wr32(b, c.a[7], c.pc);
    c.a[7] = c.a[7].wrapping_sub(2);
    wr16(b, c.a[7], sr);
    c.sr = c.sr & !0x0700 | (niveau as u16) << 8;
    c.pc = rd32(b, (24 + niveau as u32) * 4);
}

/// Exécute la native posée au PC, ou arrête le jeu s'il n'y en a pas.
pub(crate) fn pas(c: &mut Cpu, b: &mut Cps2Bus) {
    let Some(n) = crate::lookup(c.pc) else {
        arreter(c, b, format!("pas de native en {:06x}", c.pc));
    };
    let cout = (n.cost)(c, b);
    if cout >= crate::families::COUT_HORS_CONTRAT && cout != crate::families::COUT_EPILOGUE_ETRANGER {
        arreter(c, b, format!("native {} hors contrat en {:06x}", n.name, c.pc));
    }
    DERNIERES.with(|d| {
        let mut d = d.borrow_mut();
        if d.len() == 8 {
            d.pop_front();
        }
        d.push_back(n);
    });
    if trace(b) {
        eprintln!("{}:{} {:06x} {} sr={:04x} sp={:x}", b.frame, b.scanline, c.pc, n.name, c.sr, c.a[7]);
    }
    crate::executer(n, c, b, cout);
}

/// Diagnostic : `DDSOM_SANS_TRACE=n` affiche chaque native exécutée pendant l'image n.
fn trace(b: &Cps2Bus) -> bool {
    static IMAGE: std::sync::OnceLock<Option<u64>> = std::sync::OnceLock::new();
    *IMAGE.get_or_init(|| std::env::var("DDSOM_SANS_TRACE").ok().and_then(|v| v.parse().ok())) == Some(b.frame)
}

/// Début du message d'un arrêt (panique) du jeu sans 68000.
pub const ARRET: &str = "sans 68000 : ";

/// Arrête net (panique, message commençant par [`ARRET`]) : cause, image et ligne, tâche courante et ses appels en cours (cadres), sommet de la pile 68000,
/// dernières natives exécutées, appel Rust de code d'origine en cours.
fn arreter(c: &Cpu, b: &mut Cps2Bus, cause: String) -> ! {
    let mut m = format!("{cause} (image {}, ligne {})", b.frame, b.scanline);
    if let Some(t) = lire(b) {
        let k = t.courante;
        let e = &t.emplacements[k];
        let cadres: Vec<String> = e.cadres.iter().map(|k| format!("{:x?}@{:x}", k.contenu, k.pile)).collect();
        m += &format!(" ; tâche {k} ({:?}, {:?}) ; cadres [{}]", e.tache, e.etat, cadres.join(" "));
    }
    let pile: Vec<String> = (0..6).map(|i| format!("{:x}", rd32(b, c.a[7].wrapping_add(4 * i)))).collect();
    m += &format!(" ; SR {:04x}, pile {:x} : {}", c.sr, c.a[7], pile.join(" "));
    let dernieres = DERNIERES.with(|d| d.borrow().iter().map(|n| format!("{}@{:x}", n.name, n.entry)).collect::<Vec<_>>());
    m += &format!(" ; dernières natives : {}", dernieres.join(" "));
    if let Some((site, routine)) = crate::site_d_appel() {
        m += &format!(" ; appel Rust en cours : {site} -> {routine:x}");
    }
    panic!("{ARRET}{m}");
}
