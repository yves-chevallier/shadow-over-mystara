//! Le jeu sans 68000 : natives seules et boucle d'image du noyau (voir `docs/noyau.md`, « Vers une boucle sans
//! 68000 »).
//!
//! Le `Cpu` de la machine n'est plus qu'un banc de registres (D0-D7, A0-A7, SR) et de pile. Le processeur est à un
//! *point* (`kernel::Point`, rangé dans la table des tâches) : une étape du démarrage, un point de l'ordonnanceur
//! (machine à états Rust, `kernel`), une tâche, ou le pas de jeu retenu. Une tâche est lancée en appelant le pas de son
//! étape (`taches::executer`) ; tant qu'elle continue par un saut d'une native à une autre (`Sequence::jump_to`, retour
//! vers une adresse), la native posée au PC s'exécute ([`pas`]). Une native s'exécute toujours en entier, même quand
//! elle déborderait sur l'interruption suivante : aucune n'est remplacée par son original. Une adresse sans native
//! arrête net le jeu avec un diagnostic ([`ARRET`]) au lieu d'exécuter du code d'origine.
//!
//! ## L'image
//!
//! La machine garde son calendrier (`cps2::Machine::line_work`) : à chaque ligne, [`ligne`] fait le travail du
//! processeur, puis le Z80 joue sa ligne. Le travail du processeur n'a pas de durée :
//!
//! 1. l'interruption de VBLANK, levée à la fin de l'image précédente, est prise dès que le SR l'autorise, entre deux
//!    points (au début de l'image : l'ordonnanceur l'attend, le pas de jeu retenu aussi ; pendant le démarrage, quand
//!    `boot_init` démasque) ; elle s'exécute en entier (`vblank::interruption`) ;
//! 2. puis les points s'enchaînent (ordonnanceur, tâches, cessions) jusqu'à ce que l'ordonnanceur tourne à vide
//!    ([`au_repos`] : plus aucune tâche prête, ou seule la tâche du gel sans rien à faire), ou que le pas de jeu
//!    atteigne le plafond de l'image (`MAX_STEPS_PER_FRAME` pas, [`pas_de_jeu`] : le pas retenu s'exécute au début de
//!    l'image suivante, juste après le VBLANK) ;
//! 3. au démarrage, un point qui attend une position du faisceau (relâchement du Z80, `boot_init` :
//!    `demarrage.rs`) rend la main sans rien changer : la ligne s'arrête là et il réessaie à la suivante.
//!
//! Tout le travail de l'image se fait donc au début de sa première ligne (240), avant le Z80, comme à ×16 où il finit
//! en quelques lignes : seuls le VBLANK et l'envoi des commandes au Z80 (`0x68c`, qu'il appelle) touchent la RAM
//! partagée, et ils sont les premiers. Le surcadencement, `idle`, les durées des natives et les correctifs de la
//! machine (`patches`, qui s'appliquent au code d'origine) ne servent plus.
//!
//! Une tâche prête qui attend un événement amené par le VBLANK (fin d'un fondu...) fait tourner l'ordonnanceur en rond :
//! l'original tourne ainsi jusqu'à l'interruption. Ici, quand l'état revient identique d'un parcours de
//! l'ordonnanceur au suivant ([`Instantane`]), l'image est finie. Plafond de sûreté : au plus [`PLAFOND_NATIVES`]
//! points par image (compté dans [`bilan`]).

use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use cps2::{Cps2Bus, StepCap};
use m68k::Cpu;

use crate::kernel::Point;
use crate::mem::rd32;
use crate::taches::table::{lire, table};
use crate::Native;

/// Le jeu tourne sans 68000 (une seule machine du processus, celle du jeu : la machine de référence de `validate`
/// n'exécute pas de natives).
static ACTIF: AtomicBool = AtomicBool::new(false);

pub fn actif() -> bool {
    ACTIF.load(Ordering::Relaxed)
}

/// Branche la boucle sans 68000 sur la machine (le plafond de pas de la machine ne sert que pour sa valeur).
pub(crate) fn installer(m: &mut cps2::Machine) {
    ACTIF.store(true, Ordering::Relaxed);
    m.executor = None;
    m.idle = None;
    m.patches.clear();
    m.line_work = Some(ligne);
}

/// Points au plus par image : une image ordinaire en exécute quelques milliers.
pub const PLAFOND_NATIVES: u32 = 1_000_000;

/// Parcours de l'ordonnanceur dans une image à partir duquel on cherche une attente active (voir [`Instantane`]).
const PARCOURS_ORDINAIRES: u32 = 3;

/// Images finies sur une attente active reconnue, et images arrêtées par [`PLAFOND_NATIVES`].
static IMAGES_A_VIDE: AtomicU64 = AtomicU64::new(0);
static IMAGES_PLAFONNEES: AtomicU64 = AtomicU64::new(0);

/// Bilan pour `validate` : images finies sur une attente active, images arrêtées par le plafond de natives.
pub fn bilan() -> (u64, u64) {
    (IMAGES_A_VIDE.load(Ordering::Relaxed), IMAGES_PLAFONNEES.load(Ordering::Relaxed))
}

/// État de la machine au début d'un parcours de l'ordonnanceur (`0x932`) : mémoire que les natives écrivent, table des
/// tâches, registres. S'il revient identique d'un parcours au suivant, les tâches tournent en rond (une tâche prête
/// attend un événement que seul le VBLANK amènera, comme la fin d'un fondu) : rien ne changera plus avant
/// l'interruption, l'image est finie. L'original, lui, tourne jusqu'à l'interruption.
#[derive(PartialEq)]
struct Instantane {
    ram: Vec<u16>,
    extra_ram: Vec<u16>,
    gfxram: Vec<u16>,
    objram: [Vec<u16>; 2],
    palette: Vec<u16>,
    qram: Vec<u8>,
    registres: ([u16; 6], [u16; 32], [u16; 32], u16, u16),
    table: Option<crate::taches::table::Table>,
    cpu: ([u32; 8], [u32; 8], u16, u32),
}

impl Instantane {
    fn prendre(c: &Cpu, b: &Cps2Bus) -> Self {
        Instantane {
            ram: b.ram.clone(),
            extra_ram: b.extra_ram.clone(),
            gfxram: b.gfxram.clone(),
            objram: b.objram.clone(),
            palette: b.palette.clone(),
            qram: b.qram.clone(),
            registres: (b.output, b.cps_a, b.cps_b, b.objram_bank, b.eeprom_out),
            table: lire(b).cloned(),
            cpu: (c.d, c.a, c.sr, c.other_sp),
        }
    }
}

/// Où en est l'image en cours.
#[derive(Default)]
struct Image {
    numero: u64,
    natives: u32,
    parcours: u32,
    /// Pas de jeu exécutés dans l'image, et leur plafond (`cps2::StepCap::max` ; aucun : pas de plafond).
    pas: u32,
    plafond: Option<u32>,
    /// Plus rien à faire avant l'interruption (attente active reconnue, ou plafond).
    finie: bool,
    instantane: Option<Instantane>,
}

thread_local! {
    static IMAGE: RefCell<Image> = RefCell::new(Image { numero: u64::MAX, ..Image::default() });
    /// Dernières natives exécutées (diagnostic d'un arrêt).
    static DERNIERES: RefCell<VecDeque<&'static Native>> = const { RefCell::new(VecDeque::new()) };
}

/// Travail du processeur pendant une ligne (`cps2::Machine::line_work`). Le plafond des pas de jeu n'est lu que pour
/// sa valeur : le compte des pas de l'image est tenu ici ([`pas_de_jeu`]).
pub fn ligne(c: &mut Cpu, b: &mut Cps2Bus, fin: u64, plafond: Option<&mut StepCap>) {
    if b.irq_raster {
        arreter(c, b, "interruption raster (niveau 4) : jamais armée par le jeu".into());
    }
    IMAGE.with(|i| {
        let mut i = i.borrow_mut();
        if i.numero != b.frame {
            *i = Image { numero: b.frame, ..Image::default() };
        }
        i.plafond = plafond.map(|p| p.max);
    });
    loop {
        // un point de premier niveau : aucune cession en cours
        crate::taches::oublier_cession();
        let point = crate::kernel::point(b);
        let niveau = b.irq_level();
        if niveau > (c.sr >> 8 & 7) as u8 {
            crate::vblank::interruption(c, b, niveau, point.adresse(c));
            continue;
        }
        // (une interruption masquée attend que l'ordonnanceur rouvre sa fenêtre)
        if niveau == 0 && (au_repos(b, point) || image_finie(c, b, point)) {
            break;
        }
        let suivant = match point {
            Point::Reset | Point::RelacheZ80 | Point::Poignee | Point::BootInit | Point::Lancement => {
                // `None` : il attend sa position du faisceau
                let Some(suivant) = crate::demarrage::executer(c, b, point) else { break };
                suivant
            }
            Point::Debut | Point::Parcours | Point::Examen | Point::Suivant | Point::Retour => {
                crate::kernel::executer(c, b, point)
            }
            // une tâche ne continue plus par un saut vers une native : chaque pas cède, se termine ou bloque
            Point::Tache => arreter(c, b, "la tâche a rendu la main sans céder".into()),
            // la boucle sans fin de l'original : l'image est finie
            Point::Bloque(_) => break,
            Point::PasRetenu => {
                if plafond_atteint() {
                    break;
                }
                table(b).point = Point::Tache;
                crate::taches::executer(crate::taches::partie::Etape::Pas.into(), c, b);
                crate::kernel::point(b)
            }
        };
        if suivant == Point::Tache {
            arreter(c, b, "la tâche a rendu la main sans céder".into());
        }
        table(b).point = suivant;
    }
    c.cycles = c.cycles.max(fin);
}

/// L'ordonnanceur tourne à vide, entre deux tâches : aucune tâche prête et pas de nouvelle image, ou seule la tâche du
/// gel est prête et son prochain pas ne ferait rien (`taches::gel::attente_vide`).
fn au_repos(b: &Cps2Bus, point: Point) -> bool {
    const NOUVELLE_IMAGE: usize = 0x164 / 2; // FF0164 (octet haut) : posé par le VBLANK, effacé au début d'un parcours
    if !point.entre_deux_taches() || b.ram[NOUVELLE_IMAGE] >> 8 != 0 {
        return false;
    }
    let Some(t) = lire(b) else { return false };
    t.emplacements.iter().all(|e| !e.etat.lancable()) || crate::taches::gel::attente_vide(t, &b.ram)
}

/// Un pas de jeu commence (`taches::partie`) : vrai s'il s'exécute dans cette image (il est compté), faux s'il est
/// retenu jusqu'à la suivante (plafond des pas par image, `crate::MAX_STEPS_PER_FRAME` : écart volontaire avec le jeu
/// d'origine surcadencé, appliqué à l'identique par la machine de référence).
pub fn pas_de_jeu() -> bool {
    IMAGE.with(|i| {
        let mut i = i.borrow_mut();
        if i.plafond.is_some_and(|max| i.pas >= max) {
            return false;
        }
        i.pas += 1;
        true
    })
}

fn plafond_atteint() -> bool {
    IMAGE.with(|i| {
        let i = i.borrow();
        i.plafond.is_some_and(|max| i.pas >= max)
    })
}

/// L'image n'a plus rien à faire avant l'interruption : attente active reconnue au début d'un parcours de
/// l'ordonnanceur (même [`Instantane`] qu'au parcours précédent), ou plafond de points atteint.
fn image_finie(c: &Cpu, b: &Cps2Bus, point: Point) -> bool {
    IMAGE.with(|i| {
        let mut i = i.borrow_mut();
        if i.finie {
            return true;
        }
        i.natives += 1;
        if i.natives >= PLAFOND_NATIVES {
            IMAGES_PLAFONNEES.fetch_add(1, Ordering::Relaxed);
            i.finie = true;
        } else if point == Point::Parcours {
            i.parcours += 1;
            if i.parcours >= PARCOURS_ORDINAIRES {
                let maintenant = Instantane::prendre(c, b);
                if i.instantane.as_ref() == Some(&maintenant) {
                    IMAGES_A_VIDE.fetch_add(1, Ordering::Relaxed);
                    i.finie = true;
                } else {
                    i.instantane = Some(maintenant);
                }
            }
        }
        i.finie
    })
}

/// La native de l'adresse `adresse` (appel par adresse, gestionnaire d'une table de la ROM), ou l'arrêt du jeu s'il
/// n'y en a pas. Elle est notée pour le diagnostic d'un arrêt (`DDSOM_SANS_TRACE=image` : chaque native de l'image).
pub(crate) fn native(c: &Cpu, b: &mut Cps2Bus, adresse: u32) -> &'static Native {
    let Some(n) = crate::lookup(adresse) else {
        arreter(c, b, format!("pas de native en {adresse:06x}"));
    };
    DERNIERES.with(|d| {
        let mut d = d.borrow_mut();
        if d.len() == 8 {
            d.pop_front();
        }
        d.push_back(n);
    });
    if trace(b) {
        eprintln!("{}:{} {:06x} {} sr={:04x} sp={:x}", b.frame, b.scanline, adresse, n.name, c.sr, c.a[7]);
    }
    n
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
        m += &format!(" ; point {:?}", t.point);
        let k = t.courante;
        let e = &t.emplacements[k];
        let cadres: Vec<String> = e.cadres.iter().map(|k| format!("{:x?}@{:x}", k.suite, k.pile)).collect();
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
