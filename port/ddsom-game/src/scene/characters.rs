//! État partagé `0xf6492` des sorties de scène : un intermède propre à certains personnages.
//!
//! Certaines scènes (table d'octets `0xf672e` : un bit par personnage, d'après l'octet `+3` du joueur divisé par
//! deux) font jouer à chaque joueur concerné une courte scène : écran vidé, palettes chargées, puis pour chaque
//! joueur trois objets SET (types 5, 6, 7) créés dès qu'il y a de la place, 240 images, et le joueur suivant.
//! Six sous-états (octet `+3` du bloc, pas de 2) ; plusieurs cèdent la main à l'ordonnanceur.

use super::bonus::{call_keep_a0, dispatch, fade};
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
const SET: u32 = 0xce00a;

mod at {
    pub const FADED_OUT: u32 = 0xf64d4;
    /// Après une attente, nouvelle tentative de création des objets 5, 6 et 7.
    pub const RETRY_5: u32 = 0xf6600;
    pub const RETRY_6: u32 = 0xf6626;
    pub const RETRY_7: u32 = 0xf6644;
    pub const NEXT_WAITED: u32 = 0xf66aa;
}

fn sub_step(x: &mut Ctx) -> u8 {
    rd8(x.b, x.c.a[0] + 3)
}
fn set_sub_step(x: &mut Ctx, s: u8) {
    wr8(x.b, x.c.a[0] + 3, s)
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
    fade(x, 0x1460, Etape::FadedOut)
}

crate::etapes! {
    /// Étapes (points de reprise) de la machine à états.
    pub enum Etape {
        /// Écran vidé, caméras à l'origine, palettes de l'intermède (84) ; puis le premier joueur.
        FadedOut = at::FADED_OUT => |x| {
            for off in [0x50a, 0x55a, 0x506, 0x556] {
                x.set_v16(off, 0);
            }
            x.set_v8(0x53c, 0);
            super::bonus::reset_camera_copies(x);
            x.set_v8(0x42, 2);
            next_sub_step(x);
            wr16(x.b, v::PLAYER, super::PLAYERS as u16);
            wr8(x.b, v::LEFT, 4);
            call_keep_a0(x, &[0x1f184, 0x1de7c, 0x2062]);
            x.call(0xca8);
            x.set_v8(0x3c, 0xff);
            x.c.d[0] = x.c.d[0] & !0xffff | 84;
            x.call(0x1a67e);
            x.c.d[0] = x.c.d[0] & !0xffff | 84;
            x.call(0x1a8b2);
            x.c.d[0] = 0;
            x.call(0x1a60a);
            x.set_v8(0x48e, 15);
            x.call(0x1686);
            next_player(x)
        },
        Retry5 = at::RETRY_5 => |x| {
            let Some(o) = object(x, 5, 4, Etape::Retry5) else { return Flow::Yield };
            bind(x, o);
            objects_6_7(x)
        },
        Retry6 = at::RETRY_6 => objects_6_7,
        Retry7 = at::RETRY_7 => object_7,
        NextWaited = at::NEXT_WAITED => |x| {
            for r in [0xca8, 0xcc8, 0xce8] {
                x.call(r);
            }
            Flow::Done
        },
    }
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

/// Sous-état 4 : l'intermède du joueur, après quatre images ; A6 = le joueur et D1 = son octet `+3` restent dans
/// les registres pendant les cessions, comme dans l'original.
fn player_scene(x: &mut Ctx) -> Flow {
    next_sub_step(x);
    wr8(x.b, v::TIMER, 0xf0);
    x.set_v16(0x14, 0x12c0);
    let p = rd16(x.b, v::PLAYER) as i16 as i32 as u32;
    x.c.a[6] = p;
    x.c.d[1] = rd8(x.b, p + 3) as u32;
    x.wait(4, Etape::Retry5)
}

/// Crée l'objet SET de type `kind` ; s'il n'y a pas de place, attend `frames` images et reprend en `retry`.
fn object(x: &mut Ctx, kind: u32, frames: u32, retry: Etape) -> Option<u32> {
    x.c.d[0] = kind;
    x.call(SET);
    if x.c.sr & m68k::SR_C != 0 {
        x.wait(frames, retry);
        return None;
    }
    Some(x.c.a[1])
}

/// L'objet est lié au joueur (octet `+0x2f` = D1) et au bloc (long `+0x4c` = A0).
fn bind(x: &mut Ctx, o: u32) {
    wr8(x.b, o + 0x2f, x.c.d[1] as u8);
    wr32(x.b, o + 0x4c, x.c.a[0]);
}

fn objects_6_7(x: &mut Ctx) -> Flow {
    let Some(o) = object(x, 6, 1, Etape::Retry6) else { return Flow::Yield };
    bind(x, o);
    object_7(x)
}

fn object_7(x: &mut Ctx) -> Flow {
    let Some(o) = object(x, 7, 1, Etape::Retry7) else { return Flow::Yield };
    wr16(x.b, o + 0x34, x.c.a[6] as u16);
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
    call_keep_a0(x, &[0x1f184, 0x1de7c, 0x2062, 0x1de96, 0xc58]);
    x.set_v16(0x14, 0x12c0);
    x.wait(1, Etape::NextWaited)
}

/// Fin de l'état (0xf66bc) : état suivant de la séquence, une image cédée (retour direct à l'aiguillage).
fn end(x: &mut Ctx) -> Flow {
    skip(x);
    x.set_v8(0x33, 0);
    x.set_v8(0x3c, 1);
    x.c.d[0] = 1;
    x.jump_to(0xa40)
}

pub const CHARACTERS: Native = Native {
    entry: 0xf6492,
    name: "scene_characters",
    run: |c, b| {
        let mut x = Ctx::new(c, b);
        if run(&mut x) == Flow::Done {
            crate::mem::rts(x.c, x.b);
        }
    },
    touches_sound: false,
    outputs: 0,
};

pub const NATIVES: &[Native] = &[
    CHARACTERS,
];
