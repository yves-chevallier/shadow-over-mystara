//! État partagé `0xf5938` des sorties de scène : l'écran de bonus.
//!
//! Il n'a lieu que si un joueur en jeu a ses deux longs `+0x118` et `+0x11c` à -1. Fondu au noir, caméras mises de
//! côté, puis (sept sous-états, octet `+3` du bloc, pas de 2) : décor et objets du bonus (quatre effets numérotés,
//! cinq objets SET triés par adresse, dont le dernier mène le jeu), compte à rebours de 90 secondes affiché par un
//! effet (`+0x42` : secondes restantes), fondu, retour des caméras. Le jeu lui-même est dans les objets.

use super::screen::{self, copy_page};
use super::{Ctx, PLAYERS, PLAYER_STRIDE};
use crate::mem::{prog16, rd16, rd32, rd8, wr16, wr32, wr8, A5};
use crate::modes::{Flow, Sequence};
use crate::Native;
use crate::families::effects_creators::{ecran_de_fin, ou_reste};
use crate::objects::{pools, Obj};
use crate::video::{charger_grille, effacer_couche, Couche, Grille};

/// Variables de l'écran de bonus (sous A5).
mod v {
    use crate::mem::A5;
    /// Caméras mises de côté : X et Y de la principale, X et Y de l'alternative.
    pub const SAVED_CAMERAS: u32 = A5 - 0x5594;
    /// Secondes restantes (octet ; mot pendant le fondu final), images de la seconde, durée totale (mot).
    pub const SECONDS: u32 = A5 - 0x558c;
    pub const FRAMES: u32 = A5 - 0x558b;
    pub const DURATION: u32 = A5 - 0x558a;
    /// L'objet qui mène le jeu (mot), ses cinq objets triés (mots), l'effet qui affiche le compte à rebours.
    pub const LEADER: u32 = A5 - 0x5588;
    pub const OBJECTS: u32 = A5 - 0x5586;
    pub const COUNTER: u32 = A5 - 0x5564;
    /// Dernière valeur vue de l'octet `+0x17` du meneur.
    pub const LAST_SCORE: u32 = A5 - 0x55a7;
}

/// Variables de la RAM de travail (décalages depuis A5) touchées par le bonus.
mod a5 {
    pub const SCENE: u32 = 0x34;
    pub const FADE_PAGES: u32 = 0x48e;
    pub const FADE_STEPS: u32 = 0x48f;
    pub const FADE_DELAY: u32 = 0x490;
    /// Caméra principale : X, Y, copies de X et Y, et autres champs remis à zéro ; idem pour l'alternative.
    pub const CAM_X: u32 = 0x506;
    pub const CAM_Y: u32 = 0x50a;
    pub const ALT_X: u32 = 0x556;
    pub const ALT_Y: u32 = 0x55a;
}

/// Tables de la ROM et sous-routines.
mod rom {
    pub const DISPATCH: u32 = 0xf5946;
    /// Décors du bonus (rectangles de cases), selon `A5+0x70`.
    pub const BACKDROP: u32 = 0x3f7e0e;
    pub const BACKDROP_ALT: u32 = 0x3f6c08;
}
mod sub {
    pub const MUSIC: u32 = 0xe1114;
}

/// Reprises (adresses d'origine qui suivent une cession).
mod at {
    pub const FADED_OUT: u32 = 0xf5fcc;
    pub const SET_UP: u32 = 0xf60ca;
    pub const STARTED: u32 = 0xf62a8;
    pub const ENDED: u32 = 0xf6380;
    pub const SCENE_1_WAITED: u32 = 0xf63ee;
    pub const LAST_FRAME: u32 = 0xf6470;
    pub const FADED_IN: u32 = 0xf6488;
}

fn sub_step(x: &mut Ctx) -> u8 {
    rd8(x.b, x.c.a[0] + 3)
}
fn next_sub_step(x: &mut Ctx) {
    let s = sub_step(x).wrapping_add(2);
    wr8(x.b, x.c.a[0] + 3, s);
}
/// Fin de l'état : état suivant de la séquence (`+2` += 4), sous-état 0.
fn leave(x: &mut Ctx) {
    let a0 = x.c.a[0];
    let s = rd8(x.b, a0 + 2).wrapping_add(4);
    wr8(x.b, a0 + 2, s);
    wr8(x.b, a0 + 3, 0);
}

/// Aiguillage d'origine (`moveq #0,D0 ; move.b 3(A0),D0 ; move.w table(PC,D0),D1`) : D0 et D1 sont relus plus loin.
pub fn dispatch(x: &mut Ctx, table: u32) -> u8 {
    let s = sub_step(x);
    x.c.d[0] = s as u32;
    let off = prog16(x.b, table + s as u32);
    x.c.d[1] = x.c.d[1] & !0xffff | off as u32;
    s
}

/// Fondu au noir ou depuis le noir (délai 4, toutes les pages, 15 pas) ; la routine cède la main et revient en
/// `resume`.
pub fn fade(x: &mut Ctx, routine: fn(&mut Ctx) -> Flow, resume: impl Into<crate::taches::table::Suite>) -> Flow {
    x.set_v8(a5::FADE_STEPS, 15);
    x.set_v16(a5::FADE_DELAY, 4);
    x.set_v8(a5::FADE_PAGES, 15);
    x.sous_programme(routine, resume)
}

/// `movem.l A0,-(SP) ; jsr 0x1de96 ; movem.l (SP)+,A0` : la liste de sprites, A0 gardé.
pub fn sprites_keep_a0(x: &mut Ctx) {
    let a0 = x.c.a[0];
    crate::sprites::construire_68k(x.c, x.b);
    x.c.a[0] = a0;
}

/// Un état de 0xf5938.
pub fn run(x: &mut Ctx) -> Flow {
    match dispatch(x, rom::DISPATCH) {
        0 => start(x),
        2 => set_up(x),
        4 => objects(x),
        6 => {
            countdown(x);
            Flow::Done
        }
        8 => end(x),
        10 => {
            if tick16(x, v::SECONDS) {
                next_sub_step(x);
                for k in 0..4 {
                    let w = rd16(x.b, v::SAVED_CAMERAS + 2 * k);
                    let at = [a5::CAM_X, a5::CAM_Y, a5::ALT_X, a5::ALT_Y][k as usize];
                    x.set_v16(at, w);
                }
            }
            Flow::Done
        }
        12 => {
            x.or16(0x14, 12);
            sprites_keep_a0(x);
            x.wait(1, Etape::LastFrame)
        }
        _ => Flow::Done,
    }
}

/// Mot décrémenté ; vrai quand il arrive à zéro.
fn tick16(x: &mut Ctx, at: u32) -> bool {
    let t = rd16(x.b, at).wrapping_sub(1);
    wr16(x.b, at, t);
    t == 0
}

/// Les joueurs en jeu (présents, état < 4) qui ont gagné le bonus (`+0x118` et `+0x11c` à -1).
fn qualified(x: &mut Ctx) -> u8 {
    let mut n = 0u8;
    for k in 0..4 {
        let p = PLAYERS + k * PLAYER_STRIDE;
        if rd8(x.b, p) != 0 && (rd8(x.b, p + 4) as i8) < 4 && rd32(x.b, p + 0x118) == u32::MAX && rd32(x.b, p + 0x11c) == u32::MAX {
            n += 1;
        }
    }
    n
}

/// Sous-état 0 : sans joueur qualifié, l'état est sauté ; sinon fondu au noir.
fn start(x: &mut Ctx) -> Flow {
    if qualified(x) == 0 {
        let s = rd8(x.b, A5 - 0x55c8).wrapping_add(4);
        wr8(x.b, A5 - 0x55c8, s);
        return Flow::Done;
    }
    fade(x, crate::taches::fondu::descente, Etape::FadedOut)
}

crate::etapes! {
    /// Étapes (points de reprise) de la machine à états.
    pub enum Etape {
        /// Les caméras sont mises de côté et placées sur le décor du bonus.
        FadedOut = at::FADED_OUT => |x| {
            next_sub_step(x);
            let main = (x.v16(a5::CAM_X), x.v16(a5::CAM_Y));
            wr16(x.b, v::SAVED_CAMERAS, main.0);
            wr16(x.b, v::SAVED_CAMERAS + 2, main.1);
            x.set_v8(0x53c, 0);
            let alt = (x.v16(a5::ALT_X), x.v16(a5::ALT_Y));
            wr16(x.b, v::SAVED_CAMERAS + 4, alt.0);
            wr16(x.b, v::SAVED_CAMERAS + 6, alt.1);
            x.set_v16(a5::CAM_Y, 0x408);
            x.set_v16(a5::ALT_Y, 0x400);
            x.set_v16(a5::CAM_X, 0);
            x.set_v16(a5::ALT_X, 0);
            x.set_v16(0x4ba, 0);
            if x.v8(a5::SCENE) != 1 {
                reset_camera_copies(x);
            }
            Flow::Done
        },
        SetUp = at::SET_UP => |x| {
            next_sub_step(x);
            for (from, to) in PAGES {
                copy_page(x.b, from, to);
            }
            crate::palette::charger_page(x.b, 2, 85);
            crate::palette::charger_page(x.b, 3, 85);
            crate::palette::charger_ensemble(x.b, 2);
            Flow::Done
        },
        /// Le titre du bonus (objet SET type 0, variante 69).
        Started = at::STARTED => |x| {
            if let Some(Obj(o)) = pools::SET.alloc(x.b) {
                for (off, v) in [(0, 1), (2, 0), (3, 69), (0x2f, 3)] {
                    wr8(x.b, o + off, v);
                }
            }
            Flow::Done
        },
        /// L'écran du bonus est effacé ; hors scène 1 l'état est fini, dans la scène 1 les pages de palettes reviennent
        /// après quatre images.
        Ended = at::ENDED => |x| {
            next_sub_step(x);
            screen::clear_bonus_frame(x.b);
            let counter = rd16(x.b, v::COUNTER) as i16 as i32 as u32 & 0xff_ffff;
            wr32(x.b, counter + 4, 0x0600_0000);
            let a0 = x.c.a[0];
            crate::dispatch::EFFCT.update_sans_suite(x.c, x.b);
            crate::sprites::construire_68k(x.c, x.b);
            x.c.a[0] = a0;
            if x.v8(a5::SCENE) != 1 {
                x.set_v8(0x3c, 0xff);
                effacer_couche(x.b, Couche::Deux);
                effacer_couche(x.b, Couche::Trois);
                crate::video::vider_objets(x.b);
                leave(x);
                return Flow::Done;
            }
            x.set_v8(0x3c, 1);
            wr16(x.b, v::SECONDS, 4);
            x.and16(0x14, 0xfff3);
            x.wait(4, Etape::Scene1Waited)
        },
        Scene1Waited = at::SCENE_1_WAITED => |x| {
            for (to, from) in PAGES {
                copy_page(x.b, from, to);
            }
            Flow::Done
        },
        /// Dernière image : fondu depuis le noir, puis l'état suivant de la séquence.
        LastFrame = at::LAST_FRAME => |x| fade(x, crate::taches::fondu::montee, Etape::FadedIn),
        FadedIn = at::FADED_IN => |x| {
            leave(x);
            Flow::Done
        },
    }
}

/// Copies des positions des caméras et champs de défilement remis à leurs valeurs de départ.
pub fn reset_camera_copies(x: &mut Ctx) {
    let (cx, cy, ax, ay) = (x.v16(a5::CAM_X), x.v16(a5::CAM_Y), x.v16(a5::ALT_X), x.v16(a5::ALT_Y));
    x.set_v16(0x50e, cx);
    x.set_v16(0x512, cy);
    x.set_v16(0x55e, ax);
    x.set_v16(0x562, ay);
    x.set_v16(0x52c, 0);
    x.set_v16(0x57c, 0);
    x.set_v16(0x51e, 4);
    x.set_v16(0x56e, 4);
    let f = x.v8(0x13) & !1;
    x.set_v8(0x13, f);
    x.set_v8(0x36, 0);
}

/// Sous-état 2 : quand le décor est chargé (`A5+0x7e68` ≥ 5), l'écran est vidé (sauf scène 1) et les effets du
/// bonus créés ; quatre images d'attente.
fn set_up(x: &mut Ctx) -> Flow {
    if (x.v16(0x7e68) as i16) < 5 {
        return Flow::Done;
    }
    // Glitch d'origine : l'effet du compte à rebours reçoit en `+0x2f` l'octet bas de D1 tel que le laisse
    // l'effacement de la zone de texte (D1.w = 0xffff), ou, dans la scène 1, l'aiguillage (le déplacement de la
    // sous-étape 2 dans sa table)
    let reglage = if x.v8(a5::SCENE) != 1 {
        x.set_v8(0x42, 2);
        crate::objects::lists::unlink_all(x.b);
        crate::sprites::vider_listes(x.b);
        for pool in crate::objects::pools::POOLS {
            pool.reset(x.b);
        }
        effacer_couche(x.b, Couche::Deux);
        effacer_couche(x.b, Couche::Trois);
        screen::clear_text(x.c, x.b);
        0xff
    } else {
        prog16(x.b, rom::DISPATCH + 2) as u8
    };
    // Glitch d'origine : les échecs d'allocation ne sont pas testés (l'A1 laissé reçoit les écritures)
    let cree = ecran_de_fin(x.b, 3, reglage);
    let counter = ou_reste(x.b, cree).0;
    wr16(x.b, v::COUNTER, counter as u16);
    wr8(x.b, counter + 0x42, 20);
    for k in 0..4u8 {
        let cree = ecran_de_fin(x.b, 8, 3 - k);
        let o = ou_reste(x.b, cree);
        wr8(x.b, o.0 + 0x2f, k);
    }
    x.c.d[1] = 0xffff;
    x.c.d[2] = 4;
    crate::sound::jouer_global(x.b, sub::MUSIC);
    x.wait(4, Etape::SetUp)
}

/// Pages de palettes du bonus : la page 0x91c400 passe en 0x910000, et ainsi de suite.
const PAGES: [(u32, u32); 4] = [(0x91c400, 0x910000), (0x91c800, 0x910400), (0x91cc00, 0x910800), (0x91c000, 0x910c00)];

/// Sous-état 4 : quand les décors sont prêts (`A5+0x7e66` ≥ 5), cinq objets SET, triés par adresse : les quatre
/// premiers sont liés aux joueurs, le dernier (le meneur) aux quatre autres. Compte à rebours de 90 secondes,
/// décor, blocs de tuiles, fondu depuis le noir.
fn objects(x: &mut Ctx) -> Flow {
    if x.v16(0x7e66) < 5 {
        return Flow::Done;
    }
    // Glitch d'origine : les échecs d'allocation ne sont pas testés (l'A1 laissé est rangé)
    let mut objs = [0u16; 5];
    for o in objs.iter_mut() {
        *o = crate::families::creators::set_5_pur(x.b).unwrap_or_else(|a1| a1).0 as u16;
    }
    // tri à bulles de l'original sur les adresses étendues en signe (cmpa.w)
    objs.sort_by_key(|&w| w as i16 as i32 as u32);
    for (k, &o) in objs.iter().enumerate() {
        wr16(x.b, v::OBJECTS + 2 * k as u32, o);
    }
    let addr = |w: u16| w as i16 as i32 as u32 & 0xff_ffff;
    let leader = addr(objs[4]);
    for k in 0..4 {
        let o = addr(objs[k]);
        wr8(x.b, o + 0x2f, k as u8);
        wr16(x.b, o + 0x26, objs[4]);
        wr16(x.b, o + 0x22, (PLAYERS + k as u32 * PLAYER_STRIDE) as u16);
    }
    wr8(x.b, leader + 3, 0);
    let (first, second) = (rd32(x.b, v::OBJECTS), rd32(x.b, v::OBJECTS + 4));
    wr32(x.b, leader + 0x18, first);
    wr32(x.b, leader + 0x1c, second);
    wr16(x.b, v::LEADER, objs[4]);
    let score = rd8(x.b, leader + 0x17);
    wr8(x.b, v::LAST_SCORE, score);
    next_sub_step(x);
    wr8(x.b, v::SECONDS, 20);
    wr8(x.b, v::FRAMES, 60);
    wr16(x.b, v::DURATION, 5400);
    x.set_v8(0x5f, 0);
    backdrop(x);
    show_seconds(x);
    x.set_v16(0x14, 0x12ce);
    let f = x.v8(0x13) & !1;
    x.set_v8(0x13, f);
    for (grille, n, destination) in [(Grille::Couche2, 0x205, 0x917000), (Grille::Couche3, 0x172, 0x919800)] {
        let fin = charger_grille(x.b, grille, n, destination);
        charger_grille(x.b, grille, n + 1, fin.destination);
    }
    x.set_v8(0x3c, 2);
    crate::palette::charger_page(x.b, 1, 1);
    fade(x, crate::taches::fondu::montee, Etape::Started)
}

/// 0xf67fa : le décor du bonus, rectangle de cases lu dans la ROM (largeur, hauteur, puis les cases par colonnes).
fn backdrop(x: &mut Ctx) {
    let mut src = if x.v16(0x70) != 0 { rom::BACKDROP_ALT } else { rom::BACKDROP };
    let (rows, columns) = (rd16(x.b, src) as u32, rd16(x.b, src + 2) as u32);
    src += 4;
    for col in 0..columns {
        for row in 0..rows {
            let v = rd32(x.b, src);
            wr32(x.b, 0x912014 + col * 0x80 + 4 * row, v);
            src += 4;
        }
    }
}

/// 0xf6772 : l'effet du compte à rebours affiche les secondes restantes.
fn show_seconds(x: &mut Ctx) {
    let counter = rd16(x.b, v::COUNTER) as i16 as i32 as u32 & 0xff_ffff;
    let s = rd8(x.b, v::SECONDS);
    wr8(x.b, counter + 0x42, s);
}

/// Sous-état 6 : le compte à rebours. Il repart de 20 secondes quand le score du meneur (`+0x17`) change, tombe à
/// 10 quand le meneur le demande (`+0x14`), et le jeu finit au bout des 90 secondes, à zéro, ou quand le score
/// atteint l'objectif (`+0x16`).
fn countdown(x: &mut Ctx) {
    let leader = rd16(x.b, v::LEADER) as i16 as i32 as u32 & 0xff_ffff;
    if (rd8(x.b, v::SECONDS) as i8) < 10 && rd8(x.b, leader + 0x14) != 0 {
        wr8(x.b, v::SECONDS, 10);
        wr8(x.b, v::FRAMES, 60);
        return show_seconds(x);
    }
    let score = rd8(x.b, leader + 0x17);
    if score != rd8(x.b, leader + 0x16) {
        if score != rd8(x.b, v::LAST_SCORE) {
            wr8(x.b, v::LAST_SCORE, score);
            wr8(x.b, v::SECONDS, 20);
            wr8(x.b, v::FRAMES, 60);
            return show_seconds(x);
        }
        let left = rd16(x.b, v::DURATION).wrapping_sub(1);
        wr16(x.b, v::DURATION, left);
        if left as i16 > 0 {
            let f = rd8(x.b, v::FRAMES).wrapping_sub(1);
            wr8(x.b, v::FRAMES, f);
            if f != 0 {
                return;
            }
            let s = rd8(x.b, v::SECONDS).wrapping_sub(1);
            wr8(x.b, v::SECONDS, s);
            if (s as i8) >= 0 {
                wr8(x.b, v::FRAMES, 60);
                return show_seconds(x);
            }
        } else {
            wr16(x.b, v::SECONDS, 1);
            show_seconds(x);
        }
    }
    wr32(x.b, leader + 4, 0x0600_0000);
    next_sub_step(x);
    wr16(x.b, v::SECONDS, 30);
}

/// Sous-état 8 : 30 images, puis fondu au noir.
fn end(x: &mut Ctx) -> Flow {
    if !tick16(x, v::SECONDS) {
        return Flow::Done;
    }
    fade(x, crate::taches::fondu::descente, Etape::Ended)
}

/// Entrée d'origine (séquences restées en 68000).
pub const BONUS: Native = Native {
    entry: 0xf5938,
    name: "scene_bonus",
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
    BONUS,
];
