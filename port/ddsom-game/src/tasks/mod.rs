//! Tâches de l'ordonnanceur écrites en Rust (voir `docs/noyau.md`, « Tâches Rust »).
//!
//! Une tâche Rust est une machine à états dont chaque *pas* s'exécute entre deux cessions de la main. Chaque point
//! de reprise est une étape ([`Step`], une `enum` par module : `steps!`) dont l'adresse d'origine (celle qui suit
//! le `jsr 0xa40` ou `0xa86` de l'original) n'est plus qu'un nom ; le pas rend une [`Wait`], que [`cede`] remet au
//! noyau. La table des tâches ([`table`]) range l'état, le compte, l'étape de reprise, les registres et les appels en
//! cours de la tâche ([`table::Frame`]) : sa pile 68000 est vide à chaque cession. L'ordonnanceur (`kernel`) lance une
//! tâche en appelant le pas de son étape ([`execute`]).
//!
//! Tâches : [`game`] (la partie, `0x1c45a`), [`freeze`] (avance rapide, `0xee080`), [`fading`] (fondus de palette,
//! tâches et sous-programmes qui cèdent la main), [`fixed_screen`] (tableau des scores de l'attract, `0x1835e`),
//! [`service`] (menu TEST, `0x29162`) et [`prompt`] (texte « INSERT COIN » clignotant, `0xe03fe`).

pub mod fixed_screen;
pub mod fading;
pub mod freeze;
pub mod prompt;
pub mod game;
pub mod service;
pub mod table;

use cps2::Cps2Bus;
use m68k::Cpu;

use crate::modes::Flow;
use crate::scene::Ctx;
use crate::Native;
use table::Next;

/// Ce qu'une tâche demande à l'ordonnanceur à la fin d'un pas.
#[derive(Clone, Copy)]
pub enum Wait {
    /// Dort ce nombre d'images (trap 3 : `jsr 0xa40`, D0 = nombre d'images).
    Images(u32),
    /// Reste prête : l'ordonnanceur la relance dès qu'il revient à elle (trap 4 : `jsr 0xa86`).
    Ready,
}

/// Cède la main : le noyau range l'attente et la reprise de la tâche dans la table ; elle reprendra à `resume`
/// (une étape Rust, qui porte ce qu'elle relira).
pub fn cede(x: &mut Ctx, wait_kind: Wait, resume: impl Into<Next>) -> Flow {
    let resume = resume.into();
    match wait_kind {
        Wait::Images(n) => {
            x.c.d[0] = n;
            crate::kernel::wait(x.c, x.b, resume);
        }
        Wait::Ready => crate::kernel::cede_ready(x.c, x.b, resume),
    }
    Flow::Yield
}

/// `jsr 0xb64` : cède une image (D0.w = 1, le mot haut de D0 est gardé) ; la tâche reprendra à `resume`.
pub fn one_frame(x: &mut Ctx, resume: impl Into<Next>) -> Flow {
    crate::mem::set_low_word(&mut x.c.d[0], 1);
    crate::kernel::wait(x.c, x.b, resume);
    Flow::Yield
}

/// `jsr 0xa40` : cède D0.b images (D0 tel quel) ; la tâche reprendra à `resume`.
pub fn wait(x: &mut Ctx, resume: impl Into<Next>) -> Flow {
    crate::kernel::wait(x.c, x.b, resume);
    Flow::Yield
}

/// `jmp 0xa40` : cède D0.b images ; la tâche reprendra à la suite de l'appelant (l'appel en cours).
pub fn wait_then_return(x: &mut Ctx) -> Flow {
    let next = table::pop_return(x.c, x.b);
    wait(x, next)
}

/// `jmp 0x9b6` : crée la tâche D0 (décalage) à l'entrée A0, puis revient à l'appelant.
pub fn create_then_return(x: &mut Ctx) -> Flow {
    crate::kernel::create_task(x.b, x.c.d[0], x.c.a[0]);
    Flow::Done
}

/// `jmp 0xae4` : la tâche courante repart de zéro à l'entrée A0.
pub fn restart(x: &mut Ctx) -> Flow {
    crate::kernel::restart(x.c, x.b);
    Flow::Yield
}

/// `jsr 0x9f8` : la tâche courante se termine.
pub fn finish(x: &mut Ctx) -> Flow {
    crate::kernel::finish(x.c, x.b);
    Flow::Yield
}

/// Déclare les étapes d'une tâche : une variante par point de reprise, avec l'adresse que l'original reprend
/// (`address`, gardée pour le diagnostic) et le pas qui s'y exécute (`step`).
#[macro_export]
macro_rules! steps {
    ($(#[$m:meta])* $vis:vis enum $name:ident { $($(#[$vm:meta])* $var:ident = $adr:expr => $step:expr,)* }) => {
        $(#[$m])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        $vis enum $name { $($(#[$vm])* $var,)* }
        impl $name {
            /// Adresse que l'original reprend à cette étape.
            pub const fn address(self) -> u32 {
                match self { $(Self::$var => $adr,)* }
            }
            /// Le pas de l'étape.
            pub fn step(self, x: &mut $crate::scene::Ctx) -> $crate::modes::Flow {
                match self {
                    $(Self::$var => {
                        let step: fn(&mut $crate::scene::Ctx) -> $crate::modes::Flow = $step;
                        step(x)
                    })*
                }
            }
        }
    };
}

/// Déclare l'étape de chaque sorte de tâche.
macro_rules! task_steps {
    ($($(#[$vm:meta])* $var:ident($t:ty),)*) => {
        /// Point de reprise d'une tâche Rust : l'étape de sa machine à états.
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Step { $($(#[$vm])* $var($t),)* }
        impl Step {
            /// Adresse que l'original reprend à cette étape (diagnostic, comparaison avec la table FF0064).
            pub fn address(self) -> u32 {
                match self { $(Step::$var(e) => e.address(),)* }
            }
            fn step(self, x: &mut Ctx) -> Flow {
                match self { $(Step::$var(e) => e.step(x),)* }
            }
        }
        $(
            impl From<$t> for Step {
                fn from(e: $t) -> Self { Step::$var(e) }
            }
            impl From<$t> for Next {
                fn from(e: $t) -> Self { Next::Step(Step::$var(e)) }
            }
        )*
    };
}

task_steps! {
    Game(game::Step),
    Freeze(freeze::Step),
    Prompt(prompt::Step),
    Scores(fixed_screen::Step),
    Service(service::Step),
    Interface(crate::hud::Step),
    Credits(crate::credits_task::Step),
    Fading(fading::Step),
    BlackScreen(crate::video::black_screen::Step),
    // machine à modes (tâche 0 hors partie) : sa boucle, puis ses écrans
    Modes(crate::modes::Step),
    Boot(crate::modes::boot::Step),
    Attract(crate::modes::attract::Step),
    Title(crate::modes::title::Step),
    Select(crate::modes::select::Step),
    Ranking(crate::modes::ranking::Step),
    Demo(crate::modes::demo::Step),
    Ending(crate::modes::ending::Step),
    Gameover(crate::modes::gameover::Step),
    // séquences de scène (tâche de partie)
    Exit(crate::scene::exit::Step),
    Blackout(crate::scene::blackout::Step),
    Transition(crate::scene::transition::Step),
    TransitionEarly(crate::scene::transition::early::Step),
    TransitionLate(crate::scene::transition::late::Step),
    Bonus(crate::scene::bonus::Step),
    Shop(crate::scene::shop::Step),
    Characters(crate::scene::characters::Step),
    Tally(crate::scene::tally::Step),
    // boucle de partie quand un gestionnaire d'objet fait céder la main à la tâche : mise à jour du monde, aiguillage
    World(crate::game::Step),
    Dispatch(crate::dispatch::Step),
    // gestionnaires d'objets qui appellent un sous-programme qui cède (portes SET : fondu au noir)
    DoorSet(crate::families::set::exit_fading::Step),
    // gestionnaires d'objets qui font attendre la tâche de partie
    Firework(crate::families::effects_g::Step),
}

/// Exécute l'étape `step` de la tâche courante ; si elle finit (`Flow::Done`, retour de sous-programme), la suite de
/// l'appel en cours ([`return_to_caller`]).
pub fn execute(step: Step, c: &mut Cpu, b: &mut Cps2Bus) {
    let mut x = Ctx::new(c, b);
    crate::modes::c_a5(&mut x);
    if step.step(&mut x) == Flow::Done {
        crate::diag::probe_return(x.c, &step);
        return_to_caller(x.c, x.b);
    }
}

/// Reprise d'une tâche qui a cédé la main : l'étape `step`. L'original rend à la tâche les registres sauvés à la
/// cession ; seuls quelques-uns sont relus (sonde `DDSOM_POISON=resume`), et ils sont nommés : champs des étapes qui
/// les relisent (objet et compteur de la boucle d'aiguillage, D4 du pas de jeu, A0 de l'appelant d'un fondu), que leur
/// pas remet dans le banc, ou A0 que les étapes de certaines tâches retrouvent toujours ([`resume_a0`]).
pub fn resume_step(step: Step, c: &mut Cpu, b: &mut Cps2Bus) {
    if let Some(a0) = resume_a0(step, b) {
        c.a[0] = a0;
    }
    execute(step, c, b);
}

/// L'A0 qu'une étape reprise retrouve et relit, quand il ne dépend que d'elle :
/// * les séquences de sortie de scène (aiguillage, états partagés, transitions) travaillent sur le bloc de la sortie
///   de scène, que l'aiguillage pose dans A0 et que les cessions gardent ;
/// * le gestionnaire qui fait attendre la partie (feu d'artifice) retrouve son objet : celui de la boucle d'aiguillage
///   en cours (`dispatch::current_object`).
fn resume_a0(step: Step, b: &mut Cps2Bus) -> Option<u32> {
    match step {
        Step::Exit(_)
        | Step::Bonus(_)
        | Step::Tally(_)
        | Step::Characters(_)
        | Step::Transition(_)
        | Step::TransitionEarly(_)
        | Step::TransitionLate(_) => Some(crate::scene::screen::blocks::EXIT),
        Step::Firework(_) => {
            let object = crate::dispatch::current_object(b).expect("reprise du feu d'artifice hors d'une boucle d'aiguillage");
            Some(object.0)
        }
        _ => None,
    }
}

/// Retour d'un sous-programme (`rts`) : la suite de l'appel en cours, étape Rust exécutée aussitôt ; rien si une
/// fonction Rust attendait ce retour (`table::call_expected`, `crate::call`), ou si la tâche a déjà cédé la main (le
/// code Rust qui suit une cession ne fait que revenir : voir [`has_ceded`]).
pub fn return_to_caller(c: &mut Cpu, b: &mut Cps2Bus) {
    if has_ceded() {
        return;
    }
    match table::ret(b) {
        table::Return::Render => {}
        table::Return::Next(Next::Step(e)) => execute(e, c, b),
        // un cadre dont la suite est une adresse : outils (natives appelées par un outil de diagnostic)
        table::Return::Next(Next::Address(a)) => c.pc = a,
        // jeu : chaque retour est celui d'un appel en cours (étape, appel attendu, appel par adresse)
        table::Return::Stack68000 if crate::no68k::active() => panic!("retour sans appel en cours"),
        // outils (natives appelées par le code d'origine) : l'adresse de retour dépilée
        table::Return::Stack68000 => c.pc = table::pop_68000(c, b),
    }
}

thread_local! {
    /// La tâche courante a cédé la main (ou s'est terminée) pendant la native en cours : les fonctions Rust encore
    /// en cours sous elle reviennent sans rien faire (voir [`has_ceded`]).
    static CEDED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// La tâche courante a cédé la main pendant la native en cours (`kernel::cede`, `finish`, `restart`) : sa reprise
/// est rangée dans la table (étape et appels en cours), et le code Rust encore en cours sous la cession (boucle
/// d'aiguillage des objets, mise à jour du monde, phase...) doit revenir sans rien faire. C'est ce que faisait
/// l'abandon par panique (`Unwound`) des fonctions Rust dépassées.
pub fn has_ceded() -> bool {
    CEDED.with(|c| c.get())
}

/// Le noyau rend la main à l'ordonnanceur pour la tâche courante.
pub(crate) fn note_cession() {
    CEDED.with(|c| c.set(true));
}

/// Une native de premier niveau commence (ordonnanceur, VBLANK, tâche relancée) : aucune cession en cours.
pub(crate) fn forget_cession() {
    CEDED.with(|c| c.set(false));
}

/// Première étape d'une tâche Rust neuve, d'après son entrée (sinon la native posée à l'entrée).
pub fn entry(entry: u32) -> Option<Step> {
    [
        Step::from(freeze::Step::Entry),
        game::Step::Entry.into(),
        prompt::Step::Entry.into(),
        fixed_screen::Step::Entry.into(),
        service::Step::Entry.into(),
        crate::hud::Step::Entry.into(),
        crate::credits_task::Step::Entry.into(),
        crate::scene::shop::Step::Portraits.into(),
        crate::modes::boot::Step::Entry.into(),
        crate::modes::Step::Entry.into(),
    ]
    .into_iter()
    .find(|e| e.address() == entry)
    .or_else(|| fading::task_from_entry(entry).map(Step::Fading))
}

/// Natives des tâches Rust.
pub fn natives() -> impl Iterator<Item = &'static Native> {
    fading::NATIVES
        .iter()
        .chain(prompt::NATIVES)
}
