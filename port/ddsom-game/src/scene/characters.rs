//! État partagé `0xf6492` des sorties de scène : un intermède propre à certains personnages.
//!
//! Certaines scènes (table d'octets `0xf672e` : un bit par personnage, d'après l'octet `+3` du joueur divisé par
//! deux) font jouer à chaque joueur concerné une courte scène : écran vidé, palettes chargées, puis pour chaque
//! joueur trois objets SET (types 5, 6, 7) créés dès qu'il y a de la place, 240 images, et le joueur suivant.
//! Six sous-états (octet `+3` du bloc, pas de 2) ; plusieurs cèdent la main à l'ordonnanceur.

use super::bonus::{dispatch, fade};
use crate::video::{effacer_couche, Couche};
use super::Ctx;
use crate::mem::{prog8, rd16, rd8, wr16, wr32, wr8, A5};
use crate::modes::{Flow, Sequence};
use crate::Native;

mod v {
    use crate::mem::A5;
    /// Personnages concernés (masque), joueur courant (mot), joueurs restant à examiner.
    pub const MASK: u32 = A5 - 0x5576;
    pub const PLAYER: u32 = A5 - 0x5578;
    pub const LEFT: u32 = A5 - 0x5575;
    /// Minuterie (octet) de l'intermède d'un joueur.
    pub const TIMER: u32 = A5 - 0x55a8;
}

const DISPATCH: u32 = 0xf64a0;
/// Masque des personnages concernés, par scène (octets relatifs au PC).
const MASKS: u32 = 0xf672e;

mod at {
    pub const FADED_OUT: u32 = 0xf64d4;
    /// Après une attente, nouvelle tentative de création des objets 5, 6 et 7.
    pub const RETRY_5: u32 = 0xf6600;
    pub const RETRY_6: u32 = 0xf6626;
    pub const RETRY_7: u32 = 0xf6644;
    pub const NEXT_WAITED: u32 = 0xf66aa;
}

/// Bloc de la sortie de scène : l'A0 de l'original, posé par l'aiguillage et gardé à travers les cessions.
const BLOC: u32 = super::screen::blocks::EXIT;

fn sub_step(x: &mut Ctx) -> u8 {
    rd8(x.b, BLOC + 3)
}
fn set_sub_step(x: &mut Ctx, s: u8) {
    wr8(x.b, BLOC + 3, s)
}
fn next_sub_step(x: &mut Ctx) {
    let s = sub_step(x).wrapping_add(2);
    set_sub_step(x, s);
}

/// Fin de l'état sans intermède : état suivant de la séquence.
fn skip(x: &mut Ctx) {
    let s = rd8(x.b, A5 - 0x55c8).wrapping_add(4);
    wr8(x.b, A5 - 0x55c8, s);
    wr8(x.b, A5 - 0x55c7, 0);
}

/// Le joueur `p` est-il en jeu et concerné par le masque (`btst` sur un registre : bit modulo 32) ?
fn concerned(x: &mut Ctx, p: u32, mask: u8) -> bool {
    let bit = (rd8(x.b, p + 3) >> 1) as u32 % 32;
    rd8(x.b, p) != 0 && (rd8(x.b, p + 4) as i8) < 4 && (mask as u32) >> bit & 1 != 0
}

pub fn run(x: &mut Ctx) -> Flow {
    match dispatch(x, DISPATCH) {
        0 => start(x),
        2 => next_player(x),
        4 => player_scene(x),
        6 => {
            next_sub_step(x);
            x.set_v16(0x14, 0x12ce);
            Flow::Done
        }
        8 => player_done(x),
        10 => end(x),
        _ => Flow::Done,
    }
}

/// Sous-état 0 (0xf66d6) : le masque de la scène ; sans joueur concerné, l'état est sauté, sinon fondu au noir.
fn start(x: &mut Ctx) -> Flow {
    wr8(x.b, v::MASK, 0);
    let scene = x.v8(0x34) as u32;
    let mask = prog8(x.b, MASKS + scene);
    let any = mask != 0 && (0..4).any(|k| concerned(x, super::PLAYERS + k * super::PLAYER_STRIDE, mask));
    if !any {
        skip(x);
        return Flow::Done;
    }
    wr8(x.b, v::MASK, mask);
    x.set_v8(0x5f, 0);
    fade(x, crate::taches::fondu::descente, Etape::FadedOut)
}

/// Le joueur de l'intermède en cours : son enregistrement (A6 de l'original) et son octet `+3` (D1), que l'original
/// garde dans les registres pendant les cessions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Joueur {
    enregistrement: u32,
    octet: u8,
}

/// Étapes (points de reprise) de la machine à états.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Etape {
    /// Écran vidé, caméras à l'origine, palettes de l'intermède (84) ; puis le premier joueur.
    FadedOut,
    /// Après une attente, nouvelle tentative de création des objets 5, 6 et 7.
    Retry5(Joueur),
    Retry6(Joueur),
    Retry7(Joueur),
    NextWaited,
}

impl Etape {
    /// Adresse que l'original reprend à cette étape.
    pub const fn adresse(self) -> u32 {
        match self {
            Etape::FadedOut => at::FADED_OUT,
            Etape::Retry5(_) => at::RETRY_5,
            Etape::Retry6(_) => at::RETRY_6,
            Etape::Retry7(_) => at::RETRY_7,
            Etape::NextWaited => at::NEXT_WAITED,
        }
    }

    /// Le pas de l'étape.
    pub fn pas(self, x: &mut Ctx) -> Flow {
        match self {
            Etape::FadedOut => faded_out(x),
            Etape::Retry5(j) => objects(x, j),
            Etape::Retry6(j) => objects_6_7(x, j),
            Etape::Retry7(j) => object_7(x, j),
            Etape::NextWaited => {
                for couche in [Couche::Un, Couche::Deux, Couche::Trois] {
                    effacer_couche(x.b, couche);
                }
                Flow::Done
            }
        }
    }
}

/// Écran vidé, caméras à l'origine, palettes de l'intermède (84) ; puis le premier joueur.
fn faded_out(x: &mut Ctx) -> Flow {
    for off in [0x50a, 0x55a, 0x506, 0x556] {
        x.set_v16(off, 0);
    }
    x.set_v8(0x53c, 0);
    super::bonus::reset_camera_copies(x);
    x.set_v8(0x42, 2);
    next_sub_step(x);
    wr16(x.b, v::PLAYER, super::PLAYERS as u16);
    wr8(x.b, v::LEFT, 4);
    reset_objects(x);
    effacer_couche(x.b, Couche::Un);
    x.set_v8(0x3c, 0xff);
    crate::palette::charger_page(x.b, 2, 84);
    crate::palette::charger_page(x.b, 3, 84);
    crate::palette::charger_page(x.b, 1, 0);
    x.set_v8(0x48e, 15);
    crate::palette::full_brightness(x.b);
    next_player(x)
}

/// Sous-état 2 : le prochain joueur concerné, ou la fin de l'état.
fn next_player(x: &mut Ctx) -> Flow {
    if rd8(x.b, v::LEFT) == 0 {
        return end(x);
    }
    let mut p = rd16(x.b, v::PLAYER) as i16 as i32 as u32;
    x.set_v8(0x33, 1);
    let mask = rd8(x.b, v::MASK);
    loop {
        if concerned(x, p & 0xff_ffff, mask) {
            next_sub_step(x);
            wr16(x.b, v::PLAYER, p as u16);
            return Flow::Done;
        }
        p = p.wrapping_add(0x200);
        let left = rd8(x.b, v::LEFT).wrapping_sub(1);
        wr8(x.b, v::LEFT, left);
        if left == 0 {
            return end(x);
        }
    }
}

/// Sous-état 4 : l'intermède du joueur, après quatre images (le joueur suit dans l'étape de reprise).
fn player_scene(x: &mut Ctx) -> Flow {
    next_sub_step(x);
    wr8(x.b, v::TIMER, 0xf0);
    x.set_v16(0x14, 0x12c0);
    let p = rd16(x.b, v::PLAYER) as i16 as i32 as u32;
    let joueur = Joueur { enregistrement: p, octet: rd8(x.b, p + 3) };
    x.wait(4, Etape::Retry5(joueur))
}

/// Listes défaites et vidées (0x1f184, 0x1de7c), pools remis à zéro.
fn reset_objects(x: &mut Ctx) {
    crate::objects::lists::unlink_all(x.b);
    crate::sprites::vider_listes(x.b);
    for pool in crate::objects::pools::POOLS {
        pool.reset(x.b);
    }
}

/// Crée l'objet SET de type 15 et de variante `kind` (0xce00a) ; s'il n'y a pas de place, attend `frames` images et
/// reprend en `retry`.
fn object(x: &mut Ctx, kind: u8, frames: u32, retry: Etape) -> Option<u32> {
    match crate::families::creators::set_15_pur(x.b, kind) {
        Ok(o) => Some(o.0),
        Err(_) => {
            x.wait(frames, retry);
            None
        }
    }
}

/// L'objet est lié au joueur (octet `+0x2f`) et au bloc de la sortie de scène (long `+0x4c`, A0 de l'original).
fn bind(x: &mut Ctx, o: u32, j: Joueur) {
    wr8(x.b, o + 0x2f, j.octet);
    wr32(x.b, o + 0x4c, BLOC);
}

/// Les objets 5, 6 et 7 du joueur.
fn objects(x: &mut Ctx, j: Joueur) -> Flow {
    let Some(o) = object(x, 5, 4, Etape::Retry5(j)) else { return Flow::Yield };
    bind(x, o, j);
    objects_6_7(x, j)
}

fn objects_6_7(x: &mut Ctx, j: Joueur) -> Flow {
    let Some(o) = object(x, 6, 1, Etape::Retry6(j)) else { return Flow::Yield };
    bind(x, o, j);
    object_7(x, j)
}

fn object_7(x: &mut Ctx, j: Joueur) -> Flow {
    let Some(o) = object(x, 7, 1, Etape::Retry7(j)) else { return Flow::Yield };
    wr16(x.b, o + 0x34, j.enregistrement as u16);
    Flow::Done
}

/// Sous-état 8 : après 240 images, l'écran est vidé et le joueur suivant passe (sous-état 2).
fn player_done(x: &mut Ctx) -> Flow {
    let t = rd8(x.b, v::TIMER).wrapping_sub(1);
    wr8(x.b, v::TIMER, t);
    if t != 0 {
        return Flow::Done;
    }
    set_sub_step(x, 2);
    let p = (rd16(x.b, v::PLAYER) as i16 as i32 as u32).wrapping_add(0x200);
    wr16(x.b, v::PLAYER, p as u16);
    let left = rd8(x.b, v::LEFT).wrapping_sub(1);
    wr8(x.b, v::LEFT, left);
    reset_objects(x);
    crate::sprites::construire_68k(x.c, x.b);
    x.c.a[0] = BLOC;
    crate::video::vider_objets(x.b);
    x.set_v16(0x14, 0x12c0);
    x.wait(1, Etape::NextWaited)
}

/// Fin de l'état (0xf66bc) : état suivant de la séquence, une image cédée (retour direct à l'aiguillage).
fn end(x: &mut Ctx) -> Flow {
    skip(x);
    x.set_v8(0x33, 0);
    x.set_v8(0x3c, 1);
    x.c.d[0] = 1;
    crate::taches::attendre_puis_revenir(x)
}

pub const CHARACTERS: Native = Native {
    entry: 0xf6492,
    name: "scene_characters",
    run: crate::Corps::Processeur(|c, b| {
        let mut x = Ctx::new(c, b);
        if run(&mut x) == Flow::Done {
            crate::mem::rts(x.c, x.b);
        }
    }),
    touches_sound: false,
    outputs: 0,
};

pub const NATIVES: &[Native] = &[
    CHARACTERS,
];
