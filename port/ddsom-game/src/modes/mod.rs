//! Machine à modes (tâche 0 hors partie) : démarrage, écrans d'attract, titre. Voir `docs/modes.md`.
//!
//! Hors partie, la tâche 0 exécute la boucle de `0x133fe` : à chaque tour elle appelle le gestionnaire
//! du *mode* (`FF0E70` : 0 initialisation, 2 exécution), qui aiguille sur le *sous-mode* (`FF0E71`,
//! pas de 2) ; le tour finit par une cession d'une image à l'ordonnanceur. Un gestionnaire de
//! sous-mode est lui-même une machine à états (`FF0E72`) qui peut céder la main en son milieu
//! (attente de fondu, d'une image, d'un chargement) : une native ne pouvant céder, chaque gestionnaire
//! est découpé aux points de cession, et la reprise est une native posée à l'adresse que l'original
//! reprend (voir [`Flow`] et la macro `resume!`).

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

use crate::mem::{rd16, rd8, rts, wr16, wr32, wr8, A5};
use crate::scene::Ctx;
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
    pub const SUB_TABLE: u32 = 0x1345c;
}

/// Sous-routines du moteur appelées par les écrans.
pub mod sub {
    /// Cession à l'ordonnanceur : D0 = nombre d'images.
    pub const WAIT: u32 = 0xa40;
    /// Création d'une tâche : D0 = emplacement, A0 = entrée.
    pub const TASK_CREATE: u32 = 0x9b6;
    /// Fondu vers le noir et fondu depuis le noir, avec attente de la fin (cèdent la main).
    pub const FADE_OUT: u32 = 0x1460;
    pub const FADE_IN: u32 = 0x144c;
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

/// Suite d'un gestionnaire : terminé (il rend la main par `rts`) ou cédé à l'ordonnanceur (la reprise
/// se fera à l'adresse empilée).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Flow {
    Done,
    Yield,
}

/// Cessions et appels qui peuvent céder, vus depuis une séquence d'écran.
pub trait Sequence {
    /// Cède `frames` images à l'ordonnanceur ; la tâche reprendra en `resume` (adresse qui suit le
    /// `jsr 0xa40` de l'original). La séquence doit rendre `Flow::Yield` aussitôt.
    fn wait(&mut self, frames: u32, resume: u32) -> Flow;
    /// Appelle une routine d'origine qui peut céder : elle revient (`rts`) en `resume`.
    fn tail(&mut self, addr: u32, resume: u32) -> Flow;
    /// Continue dans le code d'origine à `addr`, qui rendra la main à l'appelant de la séquence.
    fn jump(&mut self, addr: u32) -> Flow;
    /// Comme `jump`, sans toucher à A0 (appel terminal `jmp` d'une routine d'origine).
    fn jump_to(&mut self, addr: u32) -> Flow;
    /// Empile une adresse de retour (appel `bsr`/`jsr` dont la suite est une native de reprise).
    fn push_return(&mut self, addr: u32);
    /// Mots et octets de la RAM de travail, par décalage depuis A5.
    fn or16(&mut self, off: u32, v: u16);
    fn and16(&mut self, off: u32, v: u16);
}

fn push(c: &mut Cpu, b: &mut Cps2Bus, v: u32) {
    c.a[7] = c.a[7].wrapping_sub(4);
    wr32(b, c.a[7], v);
}

impl Sequence for Ctx<'_> {
    fn wait(&mut self, frames: u32, resume: u32) -> Flow {
        crate::taches::ceder(self, crate::taches::Attente::Images(frames), resume)
    }
    fn tail(&mut self, addr: u32, resume: u32) -> Flow {
        push(self.c, self.b, resume);
        self.c.pc = addr;
        Flow::Yield
    }
    fn jump(&mut self, addr: u32) -> Flow {
        self.c.a[0] = mode::BASE;
        self.c.pc = addr;
        Flow::Yield
    }
    fn jump_to(&mut self, addr: u32) -> Flow {
        self.c.pc = addr;
        Flow::Yield
    }
    fn push_return(&mut self, addr: u32) {
        push(self.c, self.b, addr);
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

/// Native qui exécute une séquence à une adresse de reprise (ou d'entrée) de l'original.
#[macro_export]
macro_rules! resume {
    ($name:ident, $entry:expr, $body:expr) => {
        pub const $name: $crate::Native = $crate::Native {
            entry: $entry,
            name: stringify!($name),
            cost: |_, _| 100,
            run: |c, b| {
                let mut x = $crate::scene::Ctx::new(c, b);
                $crate::modes::c_a5(&mut x);
                let body: fn(&mut $crate::scene::Ctx) -> $crate::modes::Flow = $body;
                if body(&mut x) == $crate::modes::Flow::Done {
                    $crate::mem::rts(x.c, x.b);
                }
            },
            touches_sound: false,
            outputs: 0,
            exact_cost: false,
        };
    };
}

/// A5 est toujours en place dans le code d'origine.
pub fn c_a5(x: &mut Ctx) {
    x.c.a[5] = A5;
}

// ---------------------------------------------------------------------------------------------
// La boucle

/// Un tour : appelle le gestionnaire du mode, qui revient en `AFTER_HANDLER`.
fn turn(x: &mut Ctx) {
    x.c.a[0] = mode::BASE;
    let m = rd8(x.b, mode::MODE);
    x.c.d[0] = m as u32;
    let offset = x.prog16(at::MODE_TABLE + m as u32);
    x.c.d[1] = offset as u32;
    push(x.c, x.b, at::AFTER_HANDLER);
    let flow = match m {
        0 => init(x),
        2 => dispatch(x),
        // mode inconnu : le gestionnaire d'origine
        _ => x.jump(at::MODE_TABLE + offset as u32),
    };
    if flow == Flow::Done {
        rts(x.c, x.b);
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
        None => {
            let offset = x.prog16(at::SUB_TABLE + sub as u32);
            x.jump(at::SUB_TABLE + offset as u32)
        }
    }
}

fn start(c: &mut Cpu, b: &mut Cps2Bus) {
    let mut x = Ctx::new(c, b);
    c_a5(&mut x);
    wr8(x.b, A5.wrapping_sub(0x7151), 0xff);
    wr8(x.b, A5.wrapping_sub(0x7152), 0);
    turn(&mut x);
}

/// Le gestionnaire a rendu la main : une image de cession, puis un nouveau tour.
fn after_handler(c: &mut Cpu, b: &mut Cps2Bus) {
    let mut x = Ctx::new(c, b);
    c_a5(&mut x);
    x.wait(1, at::AFTER_WAIT);
}

fn after_wait(c: &mut Cpu, b: &mut Cps2Bus) {
    let mut x = Ctx::new(c, b);
    c_a5(&mut x);
    turn(&mut x);
}

const fn native(entry: u32, name: &'static str, run: fn(&mut Cpu, &mut Cps2Bus)) -> Native {
    Native { entry, name, cost: |_, _| 100, run, touches_sound: false, outputs: 0, exact_cost: false }
}

pub const LOOP: Native = native(at::LOOP, "mode_loop", start);
pub const AFTER_HANDLER: Native = native(at::AFTER_HANDLER, "mode_after_handler", after_handler);
pub const AFTER_WAIT: Native = native(at::AFTER_WAIT, "mode_after_wait", after_wait);

/// Routines natives de la machine à modes.
pub fn natives() -> impl Iterator<Item = &'static Native> {
    [&LOOP, &AFTER_HANDLER, &AFTER_WAIT].into_iter().chain(attract::NATIVES).chain(title::NATIVES).chain(select::NATIVES).chain(ranking::NATIVES).chain(gameover::NATIVES).chain(boot::NATIVES).chain(demo::NATIVES).chain(ending::NATIVES)
}
