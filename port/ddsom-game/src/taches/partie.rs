//! Tâche de partie (`0x1c45a`) : la boucle de jeu, un pas par image (voir `docs/boucle.md` §1).
//!
//! Créée au premier départ (`tache_credits.rs`) à l'emplacement 0, elle boucle sans fin :
//! compteur d'images, service tous les 60 images (`0x2e540`), cession d'une image, puis un pas de jeu
//! (la phase `A5+0x2c`, table `0x1c502`, suivie de la fin d'image, [`Etape::FinImage`]). Les phases ne cèdent jamais : tout le
//! travail d'un pas tient dans une native.
//!
//! | Adresse | Étape |
//! |---|---|
//! | `0x1c45a` | entrée ([`Etape::Entree`]) : variables de phase remises à zéro, puis boucle |
//! | `0x1c468` | début de tour ([`Etape::Tour`], retour de la fin d'image `0x1c4c4`, [`Etape::FinImage`]) : compteurs, service, puis la cession `0x1c48c` : une image d'attente, sauf tour sauté (voir `wait`) |
//! | `0x1c4b0` | pas de jeu ([`Etape::Pas`]) : appel de la phase |
//!
//! Le pas commence à `0x1c4b0` = [`crate::GAME_STEP`], où la machine de référence plafonne le nombre de pas par image
//! (`cps2::StepCap`, testé sur le PC) ; le jeu compte ses pas lui-même (`sans68k::pas_de_jeu`) : un pas au-delà du
//! plafond est retenu jusqu'au début de l'image suivante, juste après le VBLANK (`kernel::Point::PasRetenu`).

use super::{ceder, Attente};
use crate::mem::{rd8, wr32, wr8, A5};
use crate::modes::{Flow, Sequence};
use crate::scene::Ctx;

crate::etapes! {
    /// Étapes de la tâche de partie (le pas de jeu, `0x1c4b0`, reste une native : voir plus haut).
    pub enum Etape {
        Entree = at::ENTRY => entry,
        /// Début de tour : retour de la fin d'image, ou d'une phase qui dépile son propre retour.
        Tour = at::TURN => turn,
        /// Le pas de jeu : la phase courante.
        Pas = at::STEP => step,
        /// Retour de la phase : fin d'image (`game::fin_d_image`), puis le tour suivant.
        FinImage = at::FRAME_END => |x| {
            crate::game::fin_d_image(x.c, x.b);
            Flow::Done
        },
    }
}

/// Variables de la RAM de travail (décalages depuis A5).
mod var {
    /// Phase (pas de 4), étape, et mots suivants : remis à zéro à l'entrée.
    pub const PHASE: u32 = 0x2c;
    /// Compteur d'images de la partie.
    pub const FRAMES: u32 = 0x6e;
    /// Carte de développement : le temps de jeu est compté chaque seconde.
    pub const SERVICE: u32 = 0x82;
}

/// Octets hors de la page A5 : compteur d'images du service (A5 - 0x6610), tours sautés sans cession
/// (A5 - 0x660f) et « un pas a eu lieu depuis le VBLANK » (A5 - 0x660e).
const SERVICE_COUNT: u32 = 0xff_19f0;
const FREE_STEPS: u32 = 0xff_19f1;
const STEPPED: u32 = 0xff_19f2;

/// Images entre deux services, pas enchaînés sans cession après une cession, table des phases.
const SERVICE_PERIOD: u8 = 60;
const MAX_FREE_STEPS: i8 = 3;
const PHASES: u32 = 0x1c502;

/// Adresses de l'original.
mod at {
    pub const ENTRY: u32 = 0x1c45a;
    pub const TURN: u32 = 0x1c468;
    pub const STEP: u32 = crate::GAME_STEP;
    /// Retour d'une phase : la fin d'image.
    pub const FRAME_END: u32 = 0x1c4c4;
}

/// Appel en cours de la tâche : `suite` reprendra quand l'appelé reviendra (rien n'est empilé sur la pile 68000).
fn push(x: &mut Ctx, suite: impl Into<super::table::Suite>) {
    super::table::appeler(x.c, x.b, suite);
}

/// Drapeaux N et Z d'un octet chargé (`move.b`), V et C à zéro.
fn flags(x: &mut Ctx, value: u8) {
    x.c.sr = (x.c.sr & !0x0f) | (value == 0) as u16 * 4 | (value >> 7) as u16 * 8;
}

/// `0x1c44a` : les variables de partie `A5+0x2c..0x6b` (phase, étape...) à zéro ; registres de la boucle
/// d'origine (D0 = 0, D6.w = -1, A0 = fin de la zone).
pub fn effacer_variables(c: &mut m68k::Cpu, b: &mut cps2::Cps2Bus) {
    for i in 0..16 {
        wr32(b, A5 + var::PHASE + 4 * i, 0);
    }
    c.d[0] = 0;
    c.d[6] |= 0xffff;
    c.a[0] = A5 + var::PHASE + 0x40;
}

/// `0x1c45a` : phase et étape à zéro.
fn entry(x: &mut Ctx) -> Flow {
    wr32(x.b, A5 + var::PHASE, 0);
    wr32(x.b, A5 + var::PHASE + 4, 0);
    turn(x)
}

/// `0x1c468` : un tour. Les quatre mots `A5+0x2c..0x33` sont ceux que `0x1c44a` remet à zéro.
fn turn(x: &mut Ctx) -> Flow {
    let frames = x.v16(var::FRAMES).wrapping_add(1);
    x.set_v16(var::FRAMES, frames);
    if x.v8(var::SERVICE) != 0 {
        let count = rd8(x.b, SERVICE_COUNT).wrapping_add(1);
        wr8(x.b, SERVICE_COUNT, if count < SERVICE_PERIOD { count } else { 0 });
        if count >= SERVICE_PERIOD {
            crate::eeprom::carte_dev::temps_de_jeu(x.b);
        }
    }
    wait(x)
}

/// `0x1c48c` : cède une image, sauf si aucun pas n'a eu lieu depuis le VBLANK (qui efface `STEPPED`) et que moins
/// de quatre tours ont été sautés de suite.
fn wait(x: &mut Ctx) -> Flow {
    let stepped = rd8(x.b, STEPPED) != 0;
    let free = rd8(x.b, FREE_STEPS) as i8;
    if !stepped && free <= MAX_FREE_STEPS {
        wr8(x.b, FREE_STEPS, free.wrapping_add(1) as u8);
        return step(x);
    }
    wr8(x.b, FREE_STEPS, 0);
    ceder(x, Attente::Images(1), Etape::Pas)
}

/// `0x1c4b0` : le pas de jeu, s'il tient dans l'image (sinon il est retenu jusqu'à la suivante). La phase revient à
/// la fin d'image, qui revient au début du tour.
fn step(x: &mut Ctx) -> Flow {
    if !crate::sans68k::pas_de_jeu() {
        crate::kernel::retenir_le_pas(x.b);
        return Flow::Yield;
    }
    wr8(x.b, STEPPED, 0xff);
    push(x, Etape::Tour);
    let phase = x.v8(var::PHASE);
    x.c.d[0] = phase as u32;
    flags(x, phase);
    let handler = (x.prog16(PHASES + phase as u32) as u32) << 16 | x.prog16(PHASES + phase as u32 + 2) as u32;
    x.c.a[0] = handler;
    push(x, Etape::FinImage);
    x.jump_to(handler)
}

