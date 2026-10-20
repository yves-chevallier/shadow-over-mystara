//! Tâche de partie (`0x1c45a`) : la boucle de jeu, un pas par image (voir `docs/boucle.md` §1).
//!
//! Créée au premier départ (`tache_credits.rs`) à l'emplacement 0, elle boucle sans fin :
//! compteur d'images, service tous les 60 images (`0x2e540`), cession d'une image, puis un pas de jeu
//! (la phase `A5+0x2c`, table `0x1c502`, suivie de `game::FRAME_END`). Les phases ne cèdent jamais : tout le
//! travail d'un pas tient dans une native.
//!
//! Trois étapes, chacune une native posée à l'adresse de l'original :
//!
//! | Adresse | Étape |
//! |---|---|
//! | `0x1c45a` | entrée : variables de phase remises à zéro, puis boucle |
//! | `0x1c468` | début de tour (retour de `FRAME_END`) : compteurs, service, puis cession |
//! | `0x1c48c` | cession : une image d'attente, sauf tour sauté (voir `wait`) |
//! | `0x1c4b0` | pas de jeu : appel de la phase |
//!
//! Le pas commence à `0x1c4b0` = [`crate::GAME_STEP`], où la machine plafonne le nombre de pas par image
//! (`cps2::StepCap`) : les étapes précédentes s'arrêtent donc *sur* cette adresse au lieu de l'enchaîner.

use super::{ceder, Attente};
use crate::mem::{rd8, wr32, wr8, A5};
use crate::modes::Flow;
use crate::resume;
use crate::scene::Ctx;
use crate::Native;

/// Variables de la RAM de travail (décalages depuis A5).
mod var {
    /// Phase (pas de 4), étape, et mots suivants : remis à zéro à l'entrée.
    pub const PHASE: u32 = 0x2c;
    /// Compteur d'images de la partie.
    pub const FRAMES: u32 = 0x6e;
    /// Service périodique actif.
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
    pub const YIELD: u32 = 0x1c48c;
    pub const STEP: u32 = crate::GAME_STEP;
    /// Retour d'une phase : `game::FRAME_END`.
    pub const FRAME_END: u32 = 0x1c4c4;
}

/// Routine de service périodique.
const SERVICE_ROUTINE: u32 = 0x2e540;

fn push(x: &mut Ctx, value: u32) {
    x.c.a[7] = x.c.a[7].wrapping_sub(4);
    wr32(x.b, x.c.a[7], value);
}

/// Drapeaux N et Z d'un octet chargé (`move.b`), V et C à zéro.
fn flags(x: &mut Ctx, value: u8) {
    x.c.sr = (x.c.sr & !0x0f) | (value == 0) as u16 * 4 | (value >> 7) as u16 * 8;
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
            crate::call_at(x.c, x.b, SERVICE_ROUTINE, at::YIELD);
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
        x.c.pc = at::STEP;
        return Flow::Yield;
    }
    wr8(x.b, FREE_STEPS, 0);
    ceder(x, Attente::Images(1), at::STEP)
}

/// `0x1c4b0` : le pas de jeu. La phase revient en `FRAME_END`, qui revient au début du tour.
fn step(x: &mut Ctx) -> Flow {
    wr8(x.b, STEPPED, 0xff);
    push(x, at::TURN);
    let phase = x.v8(var::PHASE);
    x.c.d[0] = phase as u32;
    flags(x, phase);
    let handler = (x.prog16(PHASES + phase as u32) as u32) << 16 | x.prog16(PHASES + phase as u32 + 2) as u32;
    x.c.a[0] = handler;
    push(x, at::FRAME_END);
    x.c.pc = handler;
    Flow::Yield
}

resume!(R_ENTRY, at::ENTRY, entry);
resume!(R_TURN, at::TURN, turn);
resume!(R_YIELD, at::YIELD, wait);
resume!(R_STEP, at::STEP, step);

pub const NATIVES: &[Native] = &[R_ENTRY, R_TURN, R_YIELD, R_STEP];
