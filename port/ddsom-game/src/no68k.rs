//! Le jeu sans 68000 : natives seules et boucle d'image du noyau (voir `docs/noyau.md`, « Vers une boucle sans
//! 68000 »).
//!
//! Le `Cpu` de la machine n'est plus qu'un banc de registres (D0-D7, A0-A7, SR) et de pile. Le processeur est à un
//! *point* (`kernel::Point`, rangé dans la table des tâches) : une étape du démarrage, un point de l'ordonnanceur
//! (machine à états Rust, `kernel`), une tâche, ou le pas de jeu retenu. Une tâche est lancée en appelant le pas de son
//! étape (`tasks::execute`) ; tant qu'elle continue par un saut d'une native à une autre (`Sequence::jump_to`, retour
//! vers une adresse), la native posée au PC s'exécute ([`step`]). Une native s'exécute toujours en entier, même quand
//! elle déborderait sur l'interruption suivante : aucune n'est remplacée par son original. Une adresse sans native
//! arrête net le jeu avec un diagnostic ([`STOP`]) au lieu d'exécuter du code d'origine.
//!
//! ## L'image
//!
//! La machine garde son calendrier (`cps2::Machine::line_work`) : à chaque ligne, [`line`] fait le travail du
//! processeur, puis le Z80 joue sa ligne. Le travail du processeur n'a pas de durée :
//!
//! 1. l'interruption de VBLANK, levée à la fin de l'image précédente, est prise dès que le SR l'autorise, entre deux
//!    points (au début de l'image : l'ordonnanceur l'attend, le pas de jeu retenu aussi ; pendant le démarrage, quand
//!    `boot_init` démasque) : c'est un point du noyau (`Point::Interruption`, le point interrompu rangé dans la table,
//!    sans PC ni SR empilés), qui s'exécute en entier (`vblank::interruption`) puis revient au point interrompu ;
//! 2. puis les points s'enchaînent (ordonnanceur, tâches, cessions) jusqu'à ce que l'ordonnanceur tourne à vide
//!    ([`at_rest`] : plus aucune tâche prête, ou seule la tâche du gel sans rien à faire), ou que le pas de jeu
//!    atteigne le plafond de l'image (`MAX_STEPS_PER_FRAME` pas, [`game_step`] : le pas retenu s'exécute au début de
//!    l'image suivante, juste après le VBLANK) ;
//! 3. au démarrage, un point qui attend une position du faisceau (relâchement du Z80, `boot_init` :
//!    `startup.rs`) rend la main sans rien changer : la ligne s'arrête là et il réessaie à la suivante.
//!
//! Tout le travail de l'image se fait donc au début de sa première ligne (240), avant le Z80, comme à ×16 où il finit
//! en quelques lignes : seuls le VBLANK et l'envoi des commandes au Z80 (`0x68c`, qu'il appelle) touchent la RAM
//! partagée, et ils sont les premiers. Le surcadencement, `idle`, les durées des natives et les correctifs de la
//! machine (`patches`, qui s'appliquent au code d'origine) ne servent plus.
//!
//! Une tâche prête qui attend un événement amené par le VBLANK (fin d'un fondu...) fait tourner l'ordonnanceur en rond :
//! l'original tourne ainsi jusqu'à l'interruption. Ici, quand l'état revient identique d'un parcours de
//! l'ordonnanceur au suivant ([`Snapshot`]), l'image est finie. Plafond de sûreté : au plus [`NATIVES_CAP`]
//! points par image (compté dans [`summary`]).

use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use cps2::{Cps2Bus, StepCap};
use m68k::Cpu;

use crate::kernel::Point;
use crate::mem::rd32;
use crate::tasks::table::{read, table};
use crate::Native;

/// Le jeu tourne sans 68000 (une seule machine du processus, celle du jeu : la machine de référence de `validate`
/// n'exécute pas de natives).
static ACTIVE: AtomicBool = AtomicBool::new(false);

pub fn active() -> bool {
    ACTIVE.load(Ordering::Relaxed)
}

/// Branche la boucle sans 68000 sur la machine (le plafond de pas de la machine ne sert que pour sa valeur).
pub(crate) fn install(m: &mut cps2::Machine) {
    ACTIVE.store(true, Ordering::Relaxed);
    m.executor = None;
    m.idle = None;
    m.patches.clear();
    m.line_work = Some(line);
}

/// Points au plus par image : une image ordinaire en exécute quelques milliers.
pub const NATIVES_CAP: u32 = 1_000_000;

/// Parcours de l'ordonnanceur dans une image à partir duquel on cherche une attente active (voir [`Snapshot`]).
const ORDINARY_WALKS: u32 = 3;

/// Images finies sur une attente active reconnue, et images arrêtées par [`NATIVES_CAP`].
static IDLE_FRAMES: AtomicU64 = AtomicU64::new(0);
static CAPPED_FRAMES: AtomicU64 = AtomicU64::new(0);

/// Bilan pour `validate` : images finies sur une attente active, images arrêtées par le plafond de natives.
pub fn summary() -> (u64, u64) {
    (IDLE_FRAMES.load(Ordering::Relaxed), CAPPED_FRAMES.load(Ordering::Relaxed))
}

/// État de la machine au début d'un parcours de l'ordonnanceur (`0x932`) : mémoire que les natives écrivent, table des
/// tâches, registres. S'il revient identique d'un parcours au suivant, les tâches tournent en rond (une tâche prête
/// attend un événement que seul le VBLANK amènera, comme la fin d'un fondu) : rien ne changera plus avant
/// l'interruption, l'image est finie. L'original, lui, tourne jusqu'à l'interruption.
#[derive(PartialEq)]
struct Snapshot {
    ram: Vec<u16>,
    extra_ram: Vec<u16>,
    gfxram: Vec<u16>,
    objram: [Vec<u16>; 2],
    palette: Vec<u16>,
    qram: Vec<u8>,
    regs: ([u16; 6], [u16; 32], [u16; 32], u16, u16),
    table: Option<crate::tasks::table::Table>,
    cpu: ([u32; 8], [u32; 8], u16, u32),
}

impl Snapshot {
    fn take(c: &Cpu, b: &Cps2Bus) -> Self {
        let mut ram = b.ram.clone();
        // la RAM des piles d'origine ne compte pas (changée à chaque point par la sonde `stack`)
        ram[STACKS.0 / 2..STACKS.1 / 2].fill(0);
        Snapshot {
            ram,
            extra_ram: b.extra_ram.clone(),
            gfxram: b.gfxram.clone(),
            objram: b.objram.clone(),
            palette: b.palette.clone(),
            qram: b.qram.clone(),
            regs: (b.output, b.cps_a, b.cps_b, b.objram_bank, b.eeprom_out),
            table: read(b).cloned(),
            cpu: (c.d, c.a, c.sr, c.other_sp),
        }
    }
}

/// Où en est l'image en cours.
#[derive(Default)]
struct Image {
    number: u64,
    natives: u32,
    walk: u32,
    /// Pas de jeu exécutés dans l'image, et leur plafond (`cps2::StepCap::max` ; aucun : pas de plafond).
    step: u32,
    cap: Option<u32>,
    /// Plus rien à faire avant l'interruption (attente active reconnue, ou plafond).
    finished: bool,
    snapshot: Option<Snapshot>,
}

thread_local! {
    static IMAGE: RefCell<Image> = RefCell::new(Image { number: u64::MAX, ..Image::default() });
    /// Dernières natives exécutées (diagnostic d'un arrêt).
    static LAST: RefCell<VecDeque<&'static Native>> = const { RefCell::new(VecDeque::new()) };
}

/// Travail du processeur pendant une ligne (`cps2::Machine::line_work`). Le plafond des pas de jeu n'est lu que pour
/// sa valeur : le compte des pas de l'image est tenu ici ([`game_step`]).
pub fn line(c: &mut Cpu, b: &mut Cps2Bus, end: u64, cap: Option<&mut StepCap>) {
    if b.irq_raster {
        stop(c, b, "interruption raster (niveau 4) : jamais armée par le jeu".into());
    }
    IMAGE.with(|i| {
        let mut i = i.borrow_mut();
        if i.number != b.frame {
            *i = Image { number: b.frame, ..Image::default() };
        }
        i.cap = cap.map(|p| p.max);
    });
    let poison_stack = crate::diag::poison_active("pile");
    loop {
        // un point de premier niveau : aucune cession en cours
        crate::tasks::forget_cession();
        let point = crate::kernel::point(b);
        if poison_stack && !point.startup() {
            poison_stacks(b);
        }
        let level = b.irq_level();
        if point != Point::Interruption && level > (c.sr >> 8 & 7) as u8 {
            crate::kernel::interrupt(b, point);
            continue;
        }
        // (une interruption masquée attend que l'ordonnanceur rouvre sa fenêtre)
        if level == 0 && (at_rest(b, point) || frame_done(c, b, point)) {
            break;
        }
        let next = match point {
            Point::Reset | Point::ReleaseZ80 | Point::Handshake | Point::BootInit | Point::Launch => {
                // `None` : il attend sa position du faisceau
                let Some(next) = crate::startup::execute(c, b, point) else { break };
                next
            }
            Point::Start | Point::Walk | Point::Exam | Point::NextTask | Point::Return => {
                crate::kernel::execute(c, b, point)
            }
            Point::Interruption => {
                crate::vblank::interruption(b, level);
                crate::kernel::interrupt_end(b)
            }
            // une tâche ne continue plus par un saut vers une native : chaque pas cède, se termine ou bloque
            Point::Task => stop(c, b, "la tâche a rendu la main sans céder".into()),
            // la boucle sans fin de l'original : l'image est finie
            Point::Blocked(_) => break,
            Point::StepKept { d4 } => {
                if cap_reached() {
                    break;
                }
                table(b).point = Point::Task;
                crate::tasks::execute(crate::tasks::game::Step::Frame { d4 }.into(), c, b);
                crate::kernel::point(b)
            }
        };
        if next == Point::Task {
            stop(c, b, "la tâche a rendu la main sans céder".into());
        }
        table(b).point = next;
    }
    c.cycles = c.cycles.max(end);
}

/// RAM des piles d'origine (décalages depuis FF0000) : tas des piles, piles des tâches et du superviseur
/// (FF0166-FF0E65, que `validate` ignore : `tasks::table::kernel_ram`). Au-delà, FF0E66 et la suite sont des
/// variables du jeu (relatives à A5 : `placement::LAST_X` en A5 - 0x719a...).
const STACKS: (usize, usize) = (0x166, 0xe66);

/// Sonde `DDSOM_POISON=stack` : avant chaque point de l'ordonnanceur ou d'une tâche, la RAM des piles d'origine
/// ([`STACKS`]) est remplie d'une valeur qui change à chaque fois. Si la validation reste VALIDE, le jeu n'y lit rien :
/// il n'a plus de pile 68000.
fn poison_stacks(b: &mut Cps2Bus) {
    static N: AtomicU64 = AtomicU64::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    let pattern = (n as u16).wrapping_mul(0x9e37) ^ 0xa5c3;
    for (i, word) in b.ram[STACKS.0 / 2..STACKS.1 / 2].iter_mut().enumerate() {
        *word = pattern.wrapping_add(i as u16);
    }
}

/// L'ordonnanceur tourne à vide, entre deux tâches : aucune tâche prête et pas de nouvelle image, ou seule la tâche du
/// gel est prête et son prochain pas ne ferait rien (`tasks::freeze::idle_wait`).
fn at_rest(b: &Cps2Bus, point: Point) -> bool {
    const NEW_FRAME: usize = 0x164 / 2; // FF0164 (octet haut) : posé par le VBLANK, effacé au début d'un parcours
    if !point.between_two_tasks() || b.ram[NEW_FRAME] >> 8 != 0 {
        return false;
    }
    let Some(t) = read(b) else { return false };
    t.slots.iter().all(|e| !e.state.castable()) || crate::tasks::freeze::idle_wait(t, &b.ram)
}

/// Un pas de jeu commence (`tasks::game`) : vrai s'il s'exécute dans cette image (il est compté), faux s'il est
/// retenu jusqu'à la suivante (plafond des pas par image, `crate::MAX_STEPS_PER_FRAME` : écart volontaire avec le jeu
/// d'origine surcadencé, appliqué à l'identique par la machine de référence).
pub fn game_step() -> bool {
    IMAGE.with(|i| {
        let mut i = i.borrow_mut();
        if i.cap.is_some_and(|max| i.step >= max) {
            return false;
        }
        i.step += 1;
        true
    })
}

fn cap_reached() -> bool {
    IMAGE.with(|i| {
        let i = i.borrow();
        i.cap.is_some_and(|max| i.step >= max)
    })
}

/// L'image n'a plus rien à faire avant l'interruption : attente active reconnue au début d'un parcours de
/// l'ordonnanceur (même [`Snapshot`] qu'au parcours précédent), ou plafond de points atteint.
fn frame_done(c: &Cpu, b: &Cps2Bus, point: Point) -> bool {
    IMAGE.with(|i| {
        let mut i = i.borrow_mut();
        if i.finished {
            return true;
        }
        i.natives += 1;
        if i.natives >= NATIVES_CAP {
            CAPPED_FRAMES.fetch_add(1, Ordering::Relaxed);
            i.finished = true;
        } else if point == Point::Walk {
            i.walk += 1;
            if i.walk >= ORDINARY_WALKS {
                let now = Snapshot::take(c, b);
                if i.snapshot.as_ref() == Some(&now) {
                    IDLE_FRAMES.fetch_add(1, Ordering::Relaxed);
                    i.finished = true;
                } else {
                    i.snapshot = Some(now);
                }
            }
        }
        i.finished
    })
}

/// La native de l'adresse `address` (appel par adresse, gestionnaire d'une table de la ROM), ou l'arrêt du jeu s'il
/// n'y en a pas. Elle est notée pour le diagnostic d'un arrêt (`DDSOM_SANS_TRACE=image` : chaque native de l'image).
pub(crate) fn native(c: &Cpu, b: &mut Cps2Bus, address: u32) -> &'static Native {
    let Some(n) = crate::lookup(address) else {
        stop(c, b, format!("pas de native en {address:06x}"));
    };
    LAST.with(|d| {
        let mut d = d.borrow_mut();
        if d.len() == 8 {
            d.pop_front();
        }
        d.push_back(n);
    });
    if trace(b) {
        eprintln!("{}:{} {:06x} {} sr={:04x} sp={:x}", b.frame, b.scanline, address, n.name, c.sr, c.a[7]);
    }
    n
}

/// Diagnostic : `DDSOM_SANS_TRACE=n` affiche chaque native exécutée pendant l'image n.
fn trace(b: &Cps2Bus) -> bool {
    static IMAGE: std::sync::OnceLock<Option<u64>> = std::sync::OnceLock::new();
    *IMAGE.get_or_init(|| std::env::var("DDSOM_SANS_TRACE").ok().and_then(|v| v.parse().ok())) == Some(b.frame)
}

/// Début du message d'un arrêt (panique) du jeu sans 68000.
pub const STOP: &str = "sans 68000 : ";

/// Arrête net (panique, message commençant par [`STOP`]) : cause, image et ligne, tâche courante et ses appels en cours (cadres), sommet de la pile 68000,
/// dernières natives exécutées, appel Rust de code d'origine en cours.
fn stop(c: &Cpu, b: &mut Cps2Bus, cause: String) -> ! {
    let mut m = format!("{cause} (image {}, ligne {})", b.frame, b.scanline);
    if let Some(t) = read(b) {
        m += &format!(" ; point {:?}", t.point);
        let k = t.current;
        let e = &t.slots[k];
        let frames: Vec<String> = e.frames.iter().map(|k| format!("{:x?}@{}", k.next, k.calls)).collect();
        m += &format!(" ; tâche {k} ({:?}, {:?}) ; cadres [{}]", e.task, e.state, frames.join(" "));
    }
    let stack: Vec<String> = (0..6).map(|i| format!("{:x}", rd32(b, c.a[7].wrapping_add(4 * i)))).collect();
    m += &format!(" ; SR {:04x}, pile {:x} : {}", c.sr, c.a[7], stack.join(" "));
    let last = LAST.with(|d| d.borrow().iter().map(|n| format!("{}@{:x}", n.name, n.entry)).collect::<Vec<_>>());
    m += &format!(" ; dernières natives : {}", last.join(" "));
    if let Some((site, routine)) = crate::call_site() {
        m += &format!(" ; appel Rust en cours : {site} -> {routine:x}");
    }
    panic!("{STOP}{m}");
}
