//! Noyau (K1) : ordonnanceur coopératif de tâches (0x928) et traps 0 à 7 et 12.
//!
//! Le jeu fait tourner 16 tâches en alternance. Une tâche rend la main par une *enveloppe de cession* (`jsr 0xa40`,
//! `0xa86`...) ; l'ordonnanceur choisit alors la première tâche prête de la table. L'état des tâches est la table Rust
//! de [`crate::tasks::table`]. L'ordonnanceur est une machine à états Rust ([`Point`], rangé dans la table) que la
//! boucle d'image (`no68k`) fait avancer d'un point à la fois, en prenant l'interruption de VBLANK entre deux
//! points quand le SR l'autorise, comme le processeur entre deux instructions ; il garde les registres que l'original
//! laisse et la fenêtre d'interruption entre deux emplacements, mais plus aucun enregistrement en RAM (FF0064), ni
//! pile (ni A7 ni USP : FF0166-FF0E65 n'est plus lu, sonde `DDSOM_POISON=stack`), ni tas des piles (FF005A), ni
//! registres sauvés. Il ne pose plus de PC : une tâche est lancée en appelant le pas de son étape.
//!
//! ## Ordonnanceur
//!
//! Il parcourt les emplacements dans l'ordre (A0 = adresse de l'enregistrement d'origine, D0.w = emplacements
//! restants, de 15 à 0). Chaque emplacement est examiné par un point séparé ([`Point::Exam`]), interruptions
//! masquées jusqu'au niveau 6 ; entre deux, la fenêtre de 0x952 les autorise. Un VBLANK pose l'indicateur
//! `NEW_FRAME` (FF0164) : le parcours reprend alors au début. Une tâche prête reprend à l'étape qui suit son appel
//! de l'enveloppe, SR = 0, sans registre : ce que l'étape relit est dans ses champs (`tasks::resume_step`) ; une tâche neuve démarre à son
//! entrée, SR = 0 (mode utilisateur), sans registre : elle ne lit pas ceux que l'original lui donne (A0 =
//! enregistrement, A1 = entrée, A4 = pile, D1.w = état ; sonde `DDSOM_POISON=neuve`). Quand elle cède, l'ordonnanceur
//! reprend à l'emplacement suivant ([`Point::Return`] 0x984, puis [`Point::NextTask`] 0x952).
//!
//! ## Enveloppes (et traps de l'original)
//!
//! | Enveloppe | Trap | Effet |
//! |---|---|---|
//! | 0x9b6 | 0 | crée la tâche D0 (décalage dans la table) avec l'entrée A0 |
//! | 0x9f8 | 1 | la tâche courante se termine |
//! | 0xa14 | 2 | tue la tâche D0 |
//! | 0xa40, 0xb64 | 3 | cède D0 images (0 : suspendue jusqu'au trap 6) ; 0xb64 : une image |
//! | 0xa86 | 4 | cède, reste prête |
//! | 0xaac | 5 | suspend la tâche D0 |
//! | 0xac8 | 6 | réveille la tâche D0 avec le compte D1 |
//! | 0xae4 | 7 | la tâche courante recommence en A0 |
//! | 0xaf6 | 12 | masque (D0 ≠ 0) ou démasque les interruptions |
//!
//! Dans l'original, une enveloppe sauve les 15 registres sur la pile de la tâche, prend le trap, et sa reprise
//! (0x9bc, 0xa4c...) les restaure : les enveloppes qui ne cèdent pas laissent donc registres et drapeaux intacts, et
//! celles qui cèdent sauvent ici le contexte dans la table. Les handlers des traps ne sont atteints que par leurs
//! enveloppes (`trap #7` l'est aussi par le menu de mise au point d'origine, 0x2c3e0, que le jeu n'exécute pas) : ils
//! n'ont pas de native. Les enveloppes restent des natives pour les appels par adresse et les sauts qui subsistent.

use cps2::Cps2Bus;
use m68k::Cpu;

use crate::mem::{rd16, rd8, rts, set_low_byte, set_low_word, wr8, A5};
use crate::tasks::table::{slot, record, table, State, Resume, Next, RECORDS};
use crate::Native;

/// Adresses de la RAM.
mod ram {
    /// Indicateur posé par le VBLANK : le parcours de l'ordonnanceur recommence.
    pub const NEW_FRAME: u32 = 0xff0164;
    /// Mot de réglages lu en VBLANK (DSW) : bits 0 à 2, vitesse du jeu.
    pub const DSW: u32 = 0xffffd0;
}

const SR_SUPERVISOR: u16 = 0x2000;
const SR_WALK: u16 = 0x2600;
/// Emplacements examinés par un parcours, moins un (D0.w au début).
const LAST: u16 = 15;

/// Où en est le processeur : un point du démarrage ou de l'ordonnanceur, ou une tâche. La boucle d'image
/// (`no68k::line`) exécute un point à la fois ; l'interruption de VBLANK peut être prise entre deux, si le SR
/// l'autorise. Chaque point garde l'adresse du code d'origine qu'il remplace (diagnostic).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Point {
    /// Mise sous tension (`0x350`), puis les étapes du démarrage à l'heure de l'original (`startup.rs`).
    #[default]
    Reset,
    ReleaseZ80,
    Handshake,
    BootInit,
    Launch,
    /// `0x928` : l'ordonnanceur prend la main.
    Start,
    /// `0x932` : nouveau parcours de la table.
    Walk,
    /// `0x93e` : examen d'un emplacement (D0 = reste à voir, A0 = enregistrement).
    Exam,
    /// `0x952` : passage à l'emplacement suivant, après une cession (fenêtre d'interruption).
    NextTask,
    /// `0x984` : retour à l'ordonnanceur d'une tâche qui a cédé (ou s'est terminée).
    Return,
    /// Une tâche s'exécute (le pas de son étape, `tasks::execute`) ; elle en sort en cédant la main, en se terminant
    /// ou en bloquant.
    Task,
    /// Le pas de jeu attend l'image suivante (plafond des pas par image, `tasks::game`), avec le D4 qu'il relira
    /// (`tasks::game::Step::Step`).
    StepKept { d4: u32 },
    /// Boucle sans fin de l'original (`bra.b *` à cette adresse : blocages du menu TEST et de l'EEPROM, [`block`]) :
    /// plus rien ne s'exécute, hors des interruptions.
    Blocked(u32),
    /// `0x574` : l'interruption de VBLANK est prise ([`interrupt`]) ; après son traitement, le processeur revient
    /// (`rte`) au point interrompu, que la table garde (`Table::interrupted`) au lieu du PC et du SR empilés.
    Interruption,
}

impl Point {
    /// Adresse du code d'origine (diagnostic).
    pub fn address(self, c: &Cpu) -> u32 {
        match self {
            Point::Reset => 0x350,
            Point::ReleaseZ80 => 0x18cc,
            Point::Handshake => 0x18e2,
            Point::BootInit => 0x492,
            Point::Launch => 0x50a,
            Point::Start => 0x928,
            Point::Walk => 0x932,
            Point::Exam => 0x93e,
            Point::NextTask => 0x952,
            Point::Return => 0x984,
            Point::Task => c.pc,
            Point::StepKept { .. } => crate::GAME_STEP,
            Point::Blocked(address) => address,
            Point::Interruption => 0x574,
        }
    }

    /// Un point du démarrage (avant que l'ordonnanceur prenne la main).
    pub fn startup(self) -> bool {
        matches!(self, Point::Reset | Point::ReleaseZ80 | Point::Handshake | Point::BootInit | Point::Launch)
    }

    /// L'ordonnanceur entre deux tâches (où l'original tourne à vide en attendant une tâche prête).
    pub fn between_two_tasks(self) -> bool {
        matches!(self, Point::Walk | Point::Exam | Point::NextTask)
    }
}

/// Le point où en est le processeur (avant la création de la table : la mise sous tension).
pub fn point(b: &Cps2Bus) -> Point {
    crate::tasks::table::read(b).map_or(Point::Reset, |t| t.point)
}

/// Exécute le point `point` de l'ordonnanceur et rend le suivant (les points du démarrage et des tâches sont
/// exécutés par `no68k`).
pub fn execute(c: &mut Cpu, b: &mut Cps2Bus, point: Point) -> Point {
    match point {
        Point::Start => start(c),
        Point::Walk => walk(c, b),
        Point::Exam => exam(c, b),
        Point::NextTask => after_cession(c),
        Point::Return => scheduler_return(c, b),
        _ => unreachable!("point {point:?} : pas un point de l'ordonnanceur"),
    }
}

/// Passage en superviseur, comme à la prise d'un trap.
fn supervisor(c: &mut Cpu) {
    c.set_sr((c.sr | SR_SUPERVISOR) & !m68k::SR_T);
}

// ---------------------------------------------------------------------------------------------
// Ordonnanceur

/// 0x928 : l'ordonnanceur prend la main (une fois, à la fin du démarrage).
fn start(c: &mut Cpu) -> Point {
    c.a[5] = A5;
    Point::Walk
}

/// 0x932 : nouveau parcours de la table.
fn walk(c: &mut Cpu, b: &mut Cps2Bus) -> Point {
    wr8(b, ram::NEW_FRAME, 0);
    c.a[0] = RECORDS;
    set_low_word(&mut c.d[0], LAST);
    Point::Exam
}

/// Passe à l'emplacement suivant (`dbf`), ou recommence quand la table est épuisée.
fn next(c: &mut Cpu) -> Point {
    c.a[0] += 0x10;
    let remainder = (c.d[0] as u16).wrapping_sub(1);
    set_low_word(&mut c.d[0], remainder);
    if remainder == 0xffff { Point::Walk } else { Point::Exam }
}

/// 0x93e : examen d'un emplacement, avec interruptions masquées (jusqu'au niveau 6).
fn exam(c: &mut Cpu, b: &mut Cps2Bus) -> Point {
    c.set_sr(SR_WALK);
    if rd8(b, ram::NEW_FRAME) != 0 {
        return Point::Walk;
    }
    let k = slot(c.a[0] - RECORDS);
    let state = table(b).slots[k].state;
    set_low_byte(&mut c.d[1], state.byte());
    if state.castable() {
        launch(c, b, k)
    } else {
        c.set_sr(SR_SUPERVISOR);
        next(c)
    }
}

/// 0x952 : la tâche a cédé, la fenêtre d'interruption s'ouvre et le parcours continue.
fn after_cession(c: &mut Cpu) -> Point {
    c.set_sr(SR_SUPERVISOR);
    next(c)
}

/// 0x960 : lance la tâche de l'emplacement `k` (A0 = son enregistrement) : le pas de son étape s'exécute. Rend le
/// point où la tâche a laissé le processeur (l'ordonnanceur si elle a cédé).
fn launch(c: &mut Cpu, b: &mut Cps2Bus, k: usize) -> Point {
    let t = table(b);
    t.current = k;
    t.point = Point::Task;
    let e = &mut t.slots[k];
    let step = match (e.state, e.resume) {
        (State::Fresh, Resume::Entry(entry)) => {
            e.state = State::InProgress;
            // l'original lui donne A1 = entrée, A4 = pile et D1.w = état : aucune tâche ne les lit (sonde `neuve`)
            c.set_sr(0);
            let step = crate::tasks::entry(entry).unwrap_or_else(|| panic!("tâche {k} : entrée {entry:x} sans étape Rust"));
            crate::diag::probe_launch(c, "neuve", &step, crate::regs::a(5));
            step
        }
        (State::Ready | State::InProgress, Resume::Step(step)) => {
            e.state = State::InProgress;
            // mode utilisateur, interruptions permises (aucune tâche ne cède en les masquant) ; les registres que
            // l'étape relit sont ses champs (`tasks::resume_step`)
            c.set_sr(0);
            crate::diag::probe_launch(c, "reprise", &step, crate::regs::a(5));
            crate::tasks::resume_step(step, c, b);
            return table(b).point;
        }
        (state, resume) => unreachable!("tâche {k} lancée dans l'état {state:?} avec la reprise {resume:?}"),
    };
    crate::tasks::execute(step, c, b);
    table(b).point
}

/// 0x984 : une tâche a cédé (ou s'est terminée) ; l'ordonnanceur reprend à l'emplacement qui suit le sien.
fn scheduler_return(c: &mut Cpu, b: &mut Cps2Bus) -> Point {
    let k = table(b).current;
    c.a[0] = record(k);
    set_low_word(&mut c.d[0], LAST - k as u16);
    Point::NextTask
}

/// Rend la main à l'ordonnanceur après une opération sur la tâche courante (registres laissés par le trap) : le code
/// Rust encore en cours sous la cession revient sans rien faire (`tasks::has_ceded`).
fn to_scheduler(c: &mut Cpu, b: &mut Cps2Bus) {
    let t = table(b);
    c.a[0] = record(t.current);
    t.point = Point::Return;
    supervisor(c);
    crate::tasks::note_cession();
}

/// La tâche courante entre dans une boucle sans fin de l'original (`bra.b *` en `address`) : le processeur n'en
/// sortira plus que pour les interruptions. Le code Rust en cours revient sans rien faire (`tasks::has_ceded`).
pub fn block(b: &mut Cps2Bus, address: u32) {
    table(b).point = Point::Blocked(address);
    crate::tasks::note_cession();
}

/// L'interruption de VBLANK est prise au point `point` (entre deux points, le SR l'autorisant) : le noyau retient où
/// revenir. L'original empile PC et SR sur la pile courante (celle de la tâche, ou du superviseur) ; rien ne les y
/// relit hors du `rte`.
pub fn interrupt(b: &mut Cps2Bus, point: Point) {
    let t = table(b);
    assert!(t.interrupted.is_none(), "interruption prise au point {point:?} pendant une interruption");
    t.interrupted = Some(point);
    t.point = Point::Interruption;
}

/// `rte` : fin de l'interruption, retour au point interrompu.
pub fn interrupt_end(b: &mut Cps2Bus) -> Point {
    table(b).interrupted.take().expect("fin d'interruption sans point interrompu")
}

/// Le pas de jeu est retenu jusqu'à l'image suivante (plafond des pas par image) : la tâche de partie reste en cours.
pub fn keep_step(b: &mut Cps2Bus, d4: u32) {
    table(b).point = Point::StepKept { d4 };
}

// ---------------------------------------------------------------------------------------------
// Cessions : la tâche courante rend la main ; elle reprendra à l'étape `resume`, qui porte ce qu'elle relit
// (`tasks::resume_step`). Les tâches Rust cèdent directement (`tasks::cede`) ; les enveloppes de l'original dépilent
// d'abord leur adresse de retour (la suite d'un appel en cours).

/// Range la reprise de la tâche courante dans la table (son étape, sans registres : ce qu'elle relira est dans
/// l'étape), puis rend la main à l'ordonnanceur.
fn cede(c: &mut Cpu, b: &mut Cps2Bus, state: State, count: Option<u8>, resume: Next) {
    let Next::Step(step) = resume else {
        unreachable!("cession vers l'adresse {:x} : toutes les reprises sont des étapes Rust", resume.address())
    };
    // le seul masquage du jeu (test de la mémoire du menu TEST) ne cède pas : la reprise rend un SR sans masque
    assert!(c.sr & 0x0700 == 0, "cession vers {step:?} interruptions masquées");
    table(b).cede(state, count, Resume::Step(step));
    to_scheduler(c, b);
}

/// Trap 3 : cède D0.b images (plus le retard des réglages DSW), 0 : suspendue. L'enveloppe 0xa40 appelle d'abord
/// 0xb40, qui selon DSW remplit un bloc de la GFX RAM de la jauge de charge (jamais, dans ce jeu : bit 0x400 de DSW) ;
/// les drapeaux sauvés sont ceux de son test de DSW. Le trap laisse D0.w = état et compte, D1.w = 0x100 ou 0x200.
pub fn wait(c: &mut Cpu, b: &mut Cps2Bus, resume: impl Into<Next>) {
    let resume = resume.into();
    c.sr &= !0x0f;
    let setting = !rd16(b, ram::DSW) & 7;
    let count = (c.d[0] as u8).wrapping_add(setting as u8);
    let (mut d0, mut d1) = (c.d[0], c.d[1]);
    set_low_word(&mut d1, if count == 0 { 0x200 } else { 0x100 });
    let word = (d0 as u16 & 0xff00) | count as u16 | d1 as u16;
    set_low_word(&mut d0, word);
    let state = State::from_byte((word >> 8) as u8).unwrap_or_else(|| panic!("cession vers {resume:x?} : état {word:#06x}"));
    cede(c, b, state, Some(count), resume);
    (c.d[0], c.d[1]) = (d0, d1);
}

/// Trap 4 : cède en restant prête.
pub fn cede_ready(c: &mut Cpu, b: &mut Cps2Bus, resume: impl Into<Next>) {
    cede(c, b, State::Ready, None, resume.into());
}

/// Trap 1 : la tâche courante se termine et rend sa pile.
pub fn finish(c: &mut Cpu, b: &mut Cps2Bus) {
    let t = table(b);
    let k = t.current;
    t.free(k);
    to_scheduler(c, b);
}

/// Enveloppe de cession appelée par `jsr` : son adresse de retour est la reprise (ou la suite de l'appel en cours,
/// quand un pas Rust l'a appelée).
fn envelope(c: &mut Cpu, b: &mut Cps2Bus, cession: fn(&mut Cpu, &mut Cps2Bus, Next)) {
    let resume = crate::tasks::table::pop_return(c, b);
    cession(c, b, resume);
}

/// 0xae4 (et `trap #7`, 0xae6) : la tâche courante repart de zéro en A0, même pile.
pub fn restart(c: &mut Cpu, b: &mut Cps2Bus) {
    let t = table(b);
    t.restart(c.a[0]);
    t.point = Point::Return;
    supervisor(c);
    crate::tasks::note_cession();
}

// ---------------------------------------------------------------------------------------------
// Opérations sur une autre tâche : registres et drapeaux intacts

/// 0x9b6 : crée la tâche D0 (décalage), entrée A0. Une tâche déjà là rend sa pile.
fn create(c: &mut Cpu, b: &mut Cps2Bus) {
    create_task(b, c.d[0], c.a[0]);
    rts(c, b);
}

/// Crée la tâche de décalage `offset` dans la table (`0x10 k`), qui démarrera à `entry` (enveloppe `0x9b6` : ni
/// registre ni drapeau ne change).
pub fn create_task(b: &mut Cps2Bus, offset: u32, entry: u32) {
    table(b).create(slot(offset), entry);
}

/// 0xa14 : tue la tâche D0.
fn kill(c: &mut Cpu, b: &mut Cps2Bus) {
    kill_task(b, c.d[0]);
    rts(c, b);
}

/// Tue la tâche de décalage `offset` (enveloppe `0xa14` : ni registre ni drapeau ne change).
pub fn kill_task(b: &mut Cps2Bus, offset: u32) {
    table(b).kill(slot(offset));
}

/// 0xaac : suspend la tâche D0.
fn suspend(c: &mut Cpu, b: &mut Cps2Bus) {
    table(b).suspend(slot(c.d[0]));
    rts(c, b);
}

/// 0xac8 : réveille la tâche D0 avec le compte D1.
fn wake(c: &mut Cpu, b: &mut Cps2Bus) {
    table(b).wake(slot(c.d[0]), c.d[1] as u8);
    rts(c, b);
}

/// `0xaf6` (`trap #12`) : D0.b ≠ 0 masque toutes les interruptions de la tâche, D0.b = 0 les autorise ; le SR rendu
/// par le trap est celui de l'appelant, masque changé.
fn mask(c: &mut Cpu, b: &mut Cps2Bus) {
    let sr = if c.d[0] as u8 == 0 { c.sr & 0xf8ff } else { c.sr | 0x0700 };
    c.set_sr(sr);
    rts(c, b);
}

macro_rules! native {
    ($ident:ident, $entry:expr, $body:expr) => {
        pub const $ident: Native = Native {
            entry: $entry,
            name: stringify!($ident),
            run: crate::Body::Processeur($body),
            touches_sound: false,
            outputs: 0,
        };
    };
}

// enveloppes
native!(CREATE_TASK, 0x9b6, create);
native!(FINISH_TASK, 0x9f8, finish);
native!(KILL_TASK, 0xa14, kill);
native!(WAIT, 0xa40, |c, b| envelope(c, b, wait));
native!(CEDE, 0xa86, |c, b| envelope(c, b, cede_ready));
native!(SUSPEND_TASK, 0xaac, suspend);
native!(WAKE_TASK, 0xac8, wake);
native!(RESTART_TASK, 0xae4, restart);
native!(MASK, 0xaf6, mask);
native!(WAIT_ONE_FRAME, 0xb64, |c, b| {
    set_low_word(&mut c.d[0], 1);
    envelope(c, b, wait)
});

pub const NATIVES: &[Native] = &[
    CREATE_TASK,
    FINISH_TASK,
    KILL_TASK,
    WAIT,
    CEDE,
    SUSPEND_TASK,
    WAKE_TASK,
    RESTART_TASK,
    MASK,
    WAIT_ONE_FRAME,
];
