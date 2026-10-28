//! Machine à modes (tâche 0 hors partie) : démarrage, écrans d'attract, titre. Voir `docs/modes.md`.
//!
//! Hors partie, la tâche 0 exécute la boucle de `0x133fe` : à chaque tour elle appelle le gestionnaire
//! du *mode* (`FF0E70` : 0 initialisation, 2 exécution), qui aiguille sur le *sous-mode* (`FF0E71`,
//! pas de 2) ; le tour finit par une cession d'une image à l'ordonnanceur. Un gestionnaire de
//! sous-mode est lui-même une machine à états (`FF0E72`) qui peut céder la main en son milieu
//! (attente de fondu, d'une image, d'un chargement) : chaque gestionnaire est découpé aux points de
//! cession, et la reprise est une étape Rust (`etapes!`, l'adresse que l'original reprend n'étant plus
//! que le nom de l'étape ; voir [`Flow`]).

pub mod attract;
pub mod boot;
pub mod demo;
pub mod ending;
pub mod gameover;
pub mod ranking;
pub mod select;
pub mod title;

use cps2::Cps2Bus;
use m68k::Cpu;

use crate::mem::{rd16, rd8, wr16, wr8, A5};
use crate::scene::Ctx;
use crate::taches::table::Suite;
use crate::Native;

/// Bloc d'état de la machine à modes (`A0` dans l'original) et ses champs.
pub mod mode {
    pub const BASE: u32 = 0xff_0e70;
    /// Mode : 0 initialisation, 2 exécution.
    pub const MODE: u32 = BASE;
    /// Sous-mode (pas de 2) : l'écran en cours.
    pub const SUB: u32 = BASE + 1;
    /// Étape du sous-mode.
    pub const STEP: u32 = BASE + 2;
    pub const STEP_2: u32 = BASE + 3;
    /// Minuterie de l'écran (mot, en images).
    pub const TIMER: u32 = BASE + 8;
    /// Octet ≠ 0 : l'écran de démarrage a déjà été montré.
    pub const BOOTED: u32 = BASE + 0x32;
}

/// Adresses de la boucle de `0x133fe`.
mod at {
    pub const LOOP: u32 = 0x133fe;
    /// Retour du gestionnaire de mode : cession d'une image.
    pub const AFTER_HANDLER: u32 = 0x1341a;
    /// Reprise après la cession : tour suivant.
    pub const AFTER_WAIT: u32 = 0x13422;
    /// Table des gestionnaires de mode, et table des sous-modes.
    pub const MODE_TABLE: u32 = 0x13424;
}

/// Sous-routines du moteur appelées par les écrans.
pub mod sub {
    /// Effacements de la VRAM : sprites, plan 1, plan 2, plan 3.
    pub const CLEAR_SPRITES: u32 = 0xc58;
    pub const CLEAR_SCROLL_1: u32 = 0xca8;
    pub const CLEAR_SCROLL_2: u32 = 0xcc8;
    pub const CLEAR_SCROLL_3: u32 = 0xce8;
    /// Charge la palette n (D0) des pages de fond.
    pub const LOAD_PALETTE: u32 = 0x1a67e;
    /// Copie un bloc de tuiles (D0 = numéro, A1 = destination) dans un plan.
    pub const TILE_BLOCK: u32 = 0x26d4;
}

/// Suite d'un gestionnaire : terminé (il revient à la suite de l'appel en cours) ou non (il a cédé la main à
/// l'ordonnanceur, sauté vers une autre native, ou abandonné : la tâche continuera ailleurs).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Flow {
    Done,
    Yield,
}

/// Cessions et appels qui peuvent céder, vus depuis une séquence d'écran.
pub trait Sequence {
    /// Cède `frames` images à l'ordonnanceur ; la tâche reprendra en `resume` (étape Rust, ou adresse qui suit le
    /// `jsr 0xa40` de l'original). La séquence doit rendre `Flow::Yield` aussitôt.
    fn wait(&mut self, frames: u32, resume: impl Into<Suite>) -> Flow;
    /// Appel terminal (`jmp`) de la native `n` : elle s'exécute aussitôt, et son `rts` revient à la suite de l'appel en
    /// cours (la séquence rend `Flow::Yield`, la suite ayant déjà été exécutée).
    fn sauter(&mut self, n: &'static Native) -> Flow;
    /// Appelle un sous-programme Rust qui peut céder la main (fondu, écran noir : `taches::fondu::descente`,
    /// `video::ecran_noir::debut`...) : `resume` est rangée dans les appels en cours ; s'il revient sans céder,
    /// elle s'exécute aussitôt.
    fn sous_programme(&mut self, sp: fn(&mut Ctx) -> Flow, resume: impl Into<Suite>) -> Flow;
    /// Appel `bsr`/`jsr` dont la suite est une native de reprise : rangée dans les appels en cours de la tâche
    /// (`taches::table::Cadre`), elle reprendra quand l'appelé reviendra.
    fn push_return(&mut self, suite: impl Into<Suite>);
    /// L'appelé est revenu sans passer par son `rts` (fonction Rust qui a rendu `Flow::Done`) : l'appel en cours
    /// ouvert par `push_return` est abandonné.
    fn drop_return(&mut self);
    /// Mots et octets de la RAM de travail, par décalage depuis A5.
    fn or16(&mut self, off: u32, v: u16);
    fn and16(&mut self, off: u32, v: u16);
}

/// Appel d'un sous-programme natif depuis un pas Rust : sa suite est un appel en cours de la tâche, pas une
/// adresse empilée.
fn push(c: &mut Cpu, b: &mut Cps2Bus, suite: impl Into<Suite>) {
    crate::taches::table::appeler(c, b, suite);
}

impl Sequence for Ctx<'_> {
    fn wait(&mut self, frames: u32, resume: impl Into<Suite>) -> Flow {
        crate::taches::ceder(self, crate::taches::Attente::Images(frames), resume)
    }
    fn sauter(&mut self, n: &'static Native) -> Flow {
        crate::sauter(self.c, self.b, n);
        Flow::Yield
    }
    fn push_return(&mut self, suite: impl Into<Suite>) {
        push(self.c, self.b, suite);
    }
    fn sous_programme(&mut self, sp: fn(&mut Ctx) -> Flow, resume: impl Into<Suite>) -> Flow {
        push(self.c, self.b, resume);
        if sp(self) == Flow::Done {
            crate::taches::revenir(self.c, self.b);
        }
        Flow::Yield
    }
    fn drop_return(&mut self) {
        crate::taches::table::depiler_retour(self.c, self.b);
    }
    fn or16(&mut self, off: u32, v: u16) {
        let w = self.v16(off) | v;
        self.set_v16(off, w);
    }
    fn and16(&mut self, off: u32, v: u16) {
        let w = self.v16(off) & v;
        self.set_v16(off, w);
    }
}

/// Accès au bloc d'état des écrans (`mode::BASE`) et appels usuels.
pub trait Block {
    fn m8(&mut self, off: u32) -> u8;
    fn m16(&mut self, off: u32) -> u16;
    fn set_m8(&mut self, off: u32, v: u8);
    fn set_m16(&mut self, off: u32, v: u16);
    /// Passe à l'étape suivante de l'écran (`FF0E72` += 2).
    fn next_step(&mut self);
    /// Appelle une routine d'origine avec D0 chargé (`moveq`/`move.w #n,D0`).
    fn call_d0(&mut self, addr: u32, d0: u32);
    /// Dernier appel réussi ? (retenue effacée : convention des allocateurs d'objets)
    fn ok(&self) -> bool;
    /// Un tour d'objets et de sprites (`0xfaca2`) : SET, EFFCT, SOBJ puis liste de sprites.
    fn render(&mut self);
}

impl Block for Ctx<'_> {
    fn m8(&mut self, off: u32) -> u8 {
        rd8(self.b, mode::BASE + off)
    }
    fn m16(&mut self, off: u32) -> u16 {
        rd16(self.b, mode::BASE + off)
    }
    fn set_m8(&mut self, off: u32, v: u8) {
        wr8(self.b, mode::BASE + off, v)
    }
    fn set_m16(&mut self, off: u32, v: u16) {
        wr16(self.b, mode::BASE + off, v)
    }
    fn next_step(&mut self) {
        let s = self.m8(2).wrapping_add(2);
        self.set_m8(2, s);
    }
    fn call_d0(&mut self, addr: u32, d0: u32) {
        self.c.d[0] = d0;
        self.call(addr);
    }
    fn ok(&self) -> bool {
        self.c.sr & 1 == 0
    }
    fn render(&mut self) {
        let a0 = self.c.a[0];
        for f in [0x39e86, 0x39cba, 0x39f82, 0x1de96] {
            self.call(f);
        }
        self.c.a[0] = a0;
    }
}

/// A5 est toujours en place dans le code d'origine.
pub fn c_a5(x: &mut Ctx) {
    x.c.a[5] = A5;
}

// ---------------------------------------------------------------------------------------------
// La boucle

crate::etapes! {
    /// Étapes de la boucle de la machine à modes (`0x133fe`).
    pub enum Etape {
        /// Entrée de la boucle (tâche 0 relancée après une partie, ou fin du démarrage).
        Entree = at::LOOP => start,
        /// Le gestionnaire a rendu la main : une image de cession, puis un nouveau tour.
        ApresGestionnaire = at::AFTER_HANDLER => |x| x.wait(1, Etape::ApresAttente),
        /// Après la cession : un nouveau tour.
        ApresAttente = at::AFTER_WAIT => turn,
    }
}

/// Un tour : appelle le gestionnaire du mode, qui revient à [`Etape::ApresGestionnaire`].
fn turn(x: &mut Ctx) -> Flow {
    x.c.a[0] = mode::BASE;
    let m = rd8(x.b, mode::MODE);
    x.c.d[0] = m as u32;
    let offset = x.prog16(at::MODE_TABLE + m as u32);
    x.c.d[1] = offset as u32;
    push(x.c, x.b, Etape::ApresGestionnaire);
    match m {
        0 => init(x),
        2 => dispatch(x),
        // la table n'a que deux entrées, et seul `init` écrit le mode (code-mort.md, lot « sortie du Cpu »)
        _ => unreachable!("mode {m:#x} : hors de la table 0x13424 (modes 0 et 2)"),
    }
}

/// Mode 0 (`0x13428`) : passe au mode 2 et choisit le premier sous-mode.
fn init(x: &mut Ctx) -> Flow {
    let m = rd8(x.b, mode::MODE).wrapping_add(2);
    wr8(x.b, mode::MODE, m);
    x.c.d[0] = 0;
    wr8(x.b, mode::SUB, 2);
    if rd8(x.b, mode::BOOTED) != 0 {
        wr8(x.b, mode::SUB, 0x14);
    }
    wr8(x.b, mode::STEP, 0);
    wr8(x.b, mode::STEP_2, 0);
    wr16(x.b, mode::TIMER, 0x12c);
    Flow::Done
}

/// Mode 2 (`0x13450`) : aiguille sur le sous-mode.
fn dispatch(x: &mut Ctx) -> Flow {
    let sub = rd8(x.b, mode::SUB);
    x.c.d[0] = sub as u32;
    match attract::screen(x, sub).or_else(|| title::screen(x, sub)).or_else(|| ranking::screen(x, sub)).or_else(|| demo::screen(x, sub)) {
        Some(flow) => flow,
        // les onze entrées de la table (0 à 0x14) sont réécrites (code-mort.md, lot « sortie du Cpu »)
        None => unreachable!("sous-mode {sub:#x} : hors de la table 0x1345c"),
    }
}

/// `0x133fe` : entrée de la boucle.
pub fn start(x: &mut Ctx) -> Flow {
    wr8(x.b, A5.wrapping_sub(0x7151), 0xff);
    wr8(x.b, A5.wrapping_sub(0x7152), 0);
    turn(x)
}

/// Routines natives de la machine à modes.
pub fn natives() -> impl Iterator<Item = &'static Native> {
    attract::NATIVES.iter().chain(title::NATIVES).chain(select::NATIVES).chain(ranking::NATIVES).chain(gameover::NATIVES).chain(boot::NATIVES).chain(demo::NATIVES).chain(ending::NATIVES)
}
