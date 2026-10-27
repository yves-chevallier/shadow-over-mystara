//! Scripts de transition des niveaux suivants : scènes 27, 31, 33, 32 (34, 35, 67), 38 (40, 42, 44), 46, et la fin
//! du jeu (69, prise par la 105).

use super::*;
use crate::scene::tiles;

mod ext {
    pub const MUSIC_B: u32 = 0xe1564;
    pub const MUSIC_F: u32 = 0xe15dc;
    pub const MUSIC_G: u32 = 0xe161c;
    pub const MUSIC_H: u32 = 0xe1658;
    pub const MUSIC_FINALE: u32 = 0xe5480;
    pub const SCENE_PALETTES: u32 = 0x1a360;
    pub const PALETTE_SET: u32 = 0x1a366;
    pub const PALETTES_2: u32 = 0x1a678;
    pub const PALETTES_3: u32 = 0x1a8ac;
    pub const SCROLL_1_ON: u32 = 0x2a36;
    /// Texte (A4 = description) sur la couche de caractères.
    pub const TEXT: u32 = 0x1136;
    /// Prime versée au joueur (D0 = montant | joueur << 8, A1 = joueur).
    pub const PAY: u32 = 0x19ae6;
}
use sub::{TILE_BLOCK as TB, TILE_BLOCK_2 as TB2};

/// Page de palettes 0x91c400 et sa sauvegarde.
const PALETTE_PAGE: u32 = 0x91c400;
const PALETTE_SAVE: u32 = 0xff1190;

// ---------------------------------------------------------------------------------------------
// Scène 27 : la scène suivante dépend du choix gardé en `+4` (table 0xf9454)

pub fn scene_27(x: &mut Ctx) -> Flow {
    let choice = get8(x, var::MESSAGE);
    x.c.d[0] = choice as u32;
    if choice == 0 {
        x.set_v8(0x4e, 0);
    }
    let scene = x.prog8(0xf9454 + choice as u32);
    finish(x, scene)
}

// ---------------------------------------------------------------------------------------------
// Scène 31 : dialogue 141, prime du temps restant versée à chaque joueur, puis la scène 31

/// Prime à verser (long BCD, six chiffres) et durée affichée.
const PRIZE: u32 = 18;
const SHOWN_TIME: u32 = 16;

pub fn scene_31(x: &mut Ctx) -> Flow {
    match dispatch(x, var::STATE, 0xf9522) {
        0 => show_and_fade_in(x, At(Etape::S31Shown.into())),
        2 => {
            if !tick(x) {
                return Flow::Done;
            }
            x.set_v8(0x33, 1);
            set16(x, var::TIMER, 120);
            next(x);
            x.c.a[4] = 0xf985e;
            x.call(ext::TEXT);
            // prime et durée d'après le temps restant (table de 8 octets par dizaine de secondes)
            let clock = x.v16(a5::CLOCK);
            let at = (clock >> 8 & 15) * 48 + (clock & 0xf0) / 2;
            x.c.d[0] = at as u32;
            let time = x.prog16(0xf95ba + at as u32);
            set16(x, SHOWN_TIME, time);
            let prize = crate::mem::prog32(x.b, 0xf95bc + at as u32);
            wr32(x.b, VARS + PRIZE, prize);
            x.c.d[1] = 2;
            duration(x, 0x912958, time);
            tiles::number6(x.b, PRIZE_AT, VARS + PRIZE, 2);
            Flow::Done
        }
        4 => {
            if get16(x, var::TIMER) != 0 && !tick(x) {
                return Flow::Done;
            }
            set16(x, var::TIMER, 2);
            if pay_players(x) {
                return Flow::Done;
            }
            next(x);
            set16(x, var::TIMER, 30);
            Flow::Done
        }
        6 => {
            if tick(x) {
                next(x);
                let o = message_obj(x);
                wr16(x.b, o.0 + 0x32, 1);
                x.set_v8(0x33, 0);
            }
            Flow::Done
        }
        8 => {
            if !message_done(x) {
                return Flow::Done;
            }
            finish(x, 31)
        }
        _ => Flow::Done,
    }
}
const PRIZE_AT: u32 = 0x912c58;
crate::etapes! {
    /// Étapes (points de reprise) de la machine à états.
    pub enum Etape {
        S31Shown = 0xf9530 => |x| {
            next(x);
            s31_message(x)
        },
        S31Retry = 0xf9548 => s31_message,
        S33RetryObject = 0xf98d2 => s33_object,
        S33Drawn = 0xf98f8 => |x| show_and_fade_in(x, At(Etape::S33Shown.into())),
        S33Shown = 0xf98fc => s33_message_39,
        S33Retry39 = 0xf9910 => s33_message_39,
        S33Faded = 0xf992a => |x| {
            set8(x, var::SIGNAL, 0xff);
            world(x);
            sprites_next_frame(x, Etape::S33Clear)
        },
        S33Clear = 0xf9938 => |x| {
            x.call(sub::CLEAR_SPRITES);
            hide_layers(x, At(Etape::S33Hidden.into()))
        },
        S33Hidden = 0xf9942 => |x| {
            x.call(sub::CLEAR_SCROLL_3);
            set_d0_word(x, 88);
            dark_palette(x);
            let a = x.c.a;
            set_d0_word(x, 33);
            x.call(ext::PALETTE_SET);
            x.c.a[..7].copy_from_slice(&a[..7]);
            yield_frames(x, 4, At(Etape::S33Dark.into()))
        },
        S33Dark = 0xf996a => |x| {
            tiles(x, TB, 0x20f, Some(SCROLL_2));
            show_layers(x, At(Etape::S33Drawn2.into()))
        },
        S33Drawn2 = 0xf9980 => |x| fade(x, SCENE_PAGES, crate::taches::fondu::montee, At(Etape::S33Shown2.into())),
        S33Shown2 = 0xf9984 => |x| {
            if try_message(x, 40) {
                next(x);
                return Flow::Done;
            }
            yield_frames(x, 1, At(Etape::S33Retry40.into()))
        },
        S33Retry40 = 0xf9998 => s33_object,
        S33EndFaded = 0xf99cc => |x| hide_layers(x, At(Etape::S33EndHidden.into())),
        S33EndHidden = 0xf99d0 => |x| {
            x.call(sub::CLEAR_SCROLL_2);
            show_layers(x, At(Etape::S33End.into()))
        },
        S33End = 0xf99da => |x| finish(x, 34),
        S32Shown = 0xf9a4a => |x| {
            x.call(ext::SCROLL_1_ON);
            next(x);
            fade_task(x, 3, ALL_PAGES, sub::FADE_TASK);
            Flow::Done
        },
        /// La reprise ne recharge pas D0 : l'original rouvre alors le message 1 (D0 de l'attente).
        S32Retry59 = 0xf9a7e => s32_message_59,
        S32Hidden = 0xf9acc => |x| {
            x.call(sub::CLEAR_SCROLL_2);
            x.call(sub::CLEAR_SCROLL_3);
            s32_message_60(x)
        },
        S32Retry60 = 0xf9af0 => s32_message_60,
        S32SecondShown = 0xf9b40 => |x| {
            next(x);
            fade(x, SCENE_PAGES, crate::taches::fondu::montee, At(Etape::S32SecondIn.into()))
        },
        S32SecondIn = 0xf9b46 => s32_message_61,
        S32Retry61 = 0xf9b5a => s32_message_61,
        S32Coloured = 0xf9b8c => s32_message_62,
        S32Retry62 = 0xf9ba0 => s32_message_62,
        S32Faded = 0xf9bba => |x| hide_layers(x, At(Etape::S32End.into())),
        S32End = 0xf9bbe => |x| {
            x.call(sub::CLEAR_SCROLL_2);
            x.call(sub::CLEAR_SCROLL_3);
            end(x)
        },
        S38Drawn = 0xf9c54 => |x| {
            set16(x, var::TIMER, 300);
            show_and_fade_in(x, Caller)
        },
        S38Closed = 0xf9caa => s38_message_73,
        S38Retry73 = 0xf9cbe => s38_message_73,
        S38Gone = 0xf9cfc => |x| {
            set8(x, var::SIGNAL, 0);
            x.call(sub::CLEAR_SPRITES);
            hide_layers(x, At(Etape::S38Hidden.into()))
        },
        S38Hidden = 0xf9d0a => |x| home_cameras(x, At(Etape::S38Homed.into())),
        S38Homed = 0xf9d0e => |x| {
            x.call(sub::CLEAR_SCROLL_3);
            x.call(sub::CLEAR_SCROLL_2);
            yield_frames(x, 4, At(Etape::S38Scenery.into()))
        },
        S38Scenery = 0xf9d22 => |x| {
            // décor 540, ou 551 si un joueur en jeu est dans la posture 11 (le premier dans la posture 10 arrête la recherche)
            set_d0_word(x, 540);
            x.c.d[1] = 0xffff;
            for k in 0..4u32 {
                let p = A5 + a5::PLAYERS + k * 0x200;
                if rd8(x.b, p) == 0 || (rd8(x.b, p + 4) as i8) >= 4 {
                    continue;
                }
                match rd8(x.b, p + 3) {
                    10 => {
                        x.c.d[1] = 3 - k;
                        break;
                    }
                    11 => set_d0_word(x, 551),
                    _ => {}
                }
            }
            x.c.a[1] = SCROLL_2;
            x.call(TB);
            show_layers(x, At(Etape::S38SceneryShown.into()))
        },
        S38SceneryShown = 0xf9d70 => |x| fade(x, SCENE_PAGES, crate::taches::fondu::montee, At(Etape::S38SceneryIn.into())),
        S38SceneryIn = 0xf9d74 => s38_message_98,
        S38Retry98 = 0xf9d88 => s38_message_98,
        S38LastFaded = 0xf9da2 => |x| hide_layers(x, At(Etape::S38LastHidden.into())),
        S38LastHidden = 0xf9da6 => |x| yield_frames(x, 4, At(Etape::S38LastScenery.into())),
        S38LastScenery = 0xf9dae => |x| {
            tiles(x, TB, 0x21d, Some(SCROLL_2));
            tiles(x, TB, 0x21e, None);
            show_layers(x, At(Etape::S38LastShown.into()))
        },
        S38LastShown = 0xf9dd0 => |x| fade(x, SCENE_PAGES, crate::taches::fondu::montee, At(Etape::S38LastIn.into())),
        S38LastIn = 0xf9dd4 => s38_message_74,
        S38Retry74 = 0xf9de8 => s38_message_74,
        S38End = 0xf9e26 => |x| {
            call_d0(x, sub::SOUND, 0);
            finish(x, 47)
        },
        S46Homed = 0xf9e7c => |x| {
            x.set_v8(0x3c, 0xff);
            tiles_w(x, TB2, 385, Some(0x91b000));
            tiles_w(x, TB2, 386, None);
            tiles_w(x, TB2, 387, Some(SCROLL_3));
            tiles_w(x, TB2, 388, None);
            set8(x, var::SIGNAL, 0);
            if try_set_object(x, 12).is_none() {
                return Flow::Done;
            }
            next(x);
            set8(x, var::SUB, 0);
            let y = x.v16(a5::CAM2_Y);
            wr32(x.b, VARS + var::MESSAGE, (y as u32) << 16);
            try_set_object(x, 21);
            world(x);
            sprites_next_frame(x, Etape::S46Drawn)
        },
        S46Drawn = 0xf9ef2 => |x| {
            x.call(ext::MUSIC_FINALE);
            set16(x, var::TIMER, 300);
            layers_or(x, 0xe);
            yield_frames(x, 1, At(Etape::S46Shown.into()))
        },
        S46Shown = 0xf9f0c => |x| {
            fade_task(x, 3, ALL_PAGES, sub::FADE_TASK);
            Flow::Done
        },
        S46Gone = 0xf9f96 => |x| {
            x.call(sub::CLEAR_SPRITES);
            hide_layers(x, At(Etape::S46Hidden.into()))
        },
        S46Hidden = 0xf9fa0 => |x| {
            x.call(sub::CLEAR_SCROLL_2);
            x.call(sub::CLEAR_SCROLL_3);
            home_cameras(x, At(Etape::S46End.into()))
        },
        S46End = 0xf9fb0 => |x| {
            set8(x, var::SIGNAL, 0);
            finish(x, 46)
        },
        FinaleFaded = 0xfa058 => |x| {
            x.set_v8(a5::FADE_PAGES, ALL_PAGES);
            x.set_v16(a5::FADE_DELAY, 1);
            goto(x, crate::taches::fondu::descente, At(Etape::FinaleBlack.into()))
        },
        FinaleBlack = 0xfa06a => |x| {
            x.set_v16(a5::LAYERS, 0x12c0);
            yield_frames(x, 1, At(Etape::FinalePalettes.into()))
        },
        FinalePalettes = 0xfa078 => |x| {
            x.call(ext::SCENE_PALETTES);
            copy_palettes(x, PALETTE_SAVE, PALETTE_PAGE);
            x.call(ext::PALETTES_2);
            x.call(ext::PALETTES_3);
            yield_frames(x, 4, At(Etape::FinaleEnd.into()))
        },
        FinaleEnd = 0xfa0a2 => |x| {
            x.set_v8(a5::FADE_PAGES, ALL_PAGES);
            x.call(sub::PALETTES_WHITE);
            x.set_v8(0x42, 1);
            let a0 = x.c.a[0];
            let step = rd8(x.b, a0 + 2).wrapping_add(4);
            wr8(x.b, a0 + 2, step);
            for (off, v) in [(a5::CAM1_X, 96), (a5::CAM1_Y, 296), (a5::CAM2_X, 64), (a5::CAM2_Y, 1552)] {
                x.set_v16(off, v);
            }
            x.set_v16(a5::LAYERS, 0x12ce);
            set8(x, var::STATE, 0);
            wr8(x.b, a0 + 3, 0);
            x.set_v16(0x56e, 0);
            Flow::Done
        },
    }
}
fn s31_message(x: &mut Ctx) -> Flow {
    set_d0_word(x, 141);
    or_yield!(message_d0(x, Etape::S31Retry));
    set16(x, var::TIMER, 60);
    Flow::Done
}

/// Durée `m:ss` en chiffres de deux cases (attributs 2), dizaines de minutes omises si nulles (0xf96b2).
fn duration(x: &mut Ctx, at: u32, t: u16) {
    let nib = |k: u32| (t >> (12 - 4 * k)) as u8 & 15;
    if nib(0) != 0 {
        tiles::digit(x.b, at, nib(0), 2);
    }
    tiles::digit(x.b, at + tiles::COLUMN, nib(1), 2);
    tiles::tall(x.b, at + 2 * tiles::COLUMN, tiles::COLON, 2);
    tiles::digit(x.b, at + 3 * tiles::COLUMN, nib(2), 2);
    tiles::digit(x.b, at + 4 * tiles::COLUMN, nib(3), 2);
}

/// Une tranche de la prime à chaque joueur en jeu (état `+4` < 4) tant qu'il en reste ; vrai si une tranche a été
/// versée. La prime est commune : chaque joueur en reçoit une tranche à la même image.
fn pay_players(x: &mut Ctx) -> bool {
    let mut paid = false;
    for k in 0..4u32 {
        let p = A5 + a5::PLAYERS + k * 0x200;
        x.c.a[1] = p;
        if rd8(x.b, p) == 0 || (rd8(x.b, p + 4) as i8) >= 4 {
            continue;
        }
        let left = rd32(x.b, VARS + PRIZE);
        if left == 0 {
            continue;
        }
        paid = true;
        // la plus petite tranche non vide : centaines (4), milliers (14), dizaines de milliers (33)
        let (left, amount) = if left & 0xf00 != 0 {
            (left - 0x100, 4)
        } else if left & 0xf000 != 0 {
            (left - 0x1000, 14)
        } else {
            (left.wrapping_sub(0x10000), 33)
        };
        wr32(x.b, VARS + PRIZE, left);
        let player = (rd8(x.b, p + 2) as u32) << 8;
        x.c.d[1] = player;
        x.c.d[0] = amount | player;
        x.call(ext::PAY);
        tiles::number6(x.b, PRIZE_AT, VARS + PRIZE, 2);
    }
    paid
}

// ---------------------------------------------------------------------------------------------
// Scène 33 : objet système 0, dialogues 39 et 40 ; la réponse fixe `A5+0x59` (table 0xf99e2) ; puis la scène 34

pub fn scene_33(x: &mut Ctx) -> Flow {
    match dispatch(x, var::STATE, 0xf9884) {
        0 => {
            tiles(x, TB, 0x207, Some(SCROLL_2));
            tiles(x, TB2, 0x158, Some(SCROLL_3));
            tiles(x, TB2, 0x159, None);
            set8(x, var::SIGNAL, 0);
            s33_object(x)
        }
        2 => {
            if !message_done(x) {
                return Flow::Done;
            }
            fade(x, SCENE_PAGES, crate::taches::fondu::descente, At(Etape::S33Faded.into()))
        }
        4 => {
            if !message_done(x) {
                return Flow::Done;
            }
            let v = answer(x, 0xf99e2);
            x.set_v8(0x59, v);
            x.c.a[1] = 0xff2478;
            wr8(x.b, 0xff2478 + 0x56, v);
            fade(x, ALL_PAGES, crate::taches::fondu::descente, At(Etape::S33EndFaded.into()))
        }
        _ => Flow::Done,
    }
}
/// Objet système 0 (sous-type 3, X 96, Y 176), puis le dialogue 39 (0xf98be). La reprise du dialogue 40 y revient
/// aussi dans l'original (`bra 0xf98be`).
fn s33_object(x: &mut Ctx) -> Flow {
    let Some(o) = system_object(x, 0, Etape::S33RetryObject) else { return Flow::Yield };
    wr8(x.b, o.0 + 0x2f, 3);
    wr16(x.b, o.0 + 8, 96);
    wr16(x.b, o.0 + 0x10, 0);
    wr16(x.b, o.0 + 0xc, 176);
    world(x);
    sprites_next_frame(x, Etape::S33Drawn)
}
fn s33_message_39(x: &mut Ctx) -> Flow {
    or_yield!(message(x, 39, Etape::S33Retry39));
    next(x);
    Flow::Done
}

// ---------------------------------------------------------------------------------------------
// Scènes 32, 34, 35, 67 : dialogues 59 à 62, fondu vers une couleur ; la sortie de scène continue

pub fn scene_32(x: &mut Ctx) -> Flow {
    match dispatch(x, var::STATE, 0xf99f0) {
        0 => {
            x.set_v16(a5::LAYERS, 0x138e);
            x.call(ext::MUSIC_B);
            tiles(x, TB, 0x228, Some(SCROLL_2));
            tiles(x, TB, 0x229, None);
            tiles(x, TB2, 0x15a, Some(SCROLL_3));
            tiles(x, TB2, 0x15b, None);
            show_layers(x, At(Etape::S32Shown.into()))
        }
        2 => {
            x.c.d[0] = 59;
            if x.v8(a5::FADE_STEPS) != 0 {
                return Flow::Done;
            }
            s32_message_59(x)
        }
        4 => {
            if message_done(x) {
                call_d0(x, sub::SOUND, 0);
                next(x);
                fade_task(x, 3, SCENE_PAGES, sub::FADE_TASK_PAGES);
            }
            Flow::Done
        }
        6 => {
            if x.v8(a5::FADE_STEPS) != 0 {
                return Flow::Done;
            }
            x.set_v16(0x51e, 4);
            let f = x.v8(0x12) & !1;
            x.set_v8(0x12, f);
            clear_920000(x);
            x.set_v16(a5::LAYERS, 0x12ce);
            hide_layers(x, At(Etape::S32Hidden.into()))
        }
        8 => {
            if !message_done(x) {
                return Flow::Done;
            }
            x.call(ext::MUSIC_F);
            tiles(x, TB, 0x226, Some(SCROLL_2));
            tiles(x, TB2, 8, Some(SCROLL_3));
            tiles(x, TB2, 8, None);
            wr16(x.b, 0x91cc00, 0);
            show_layers(x, At(Etape::S32SecondShown.into()))
        }
        10 => {
            if !message_done(x) {
                return Flow::Done;
            }
            x.set_v16(a5::FADE_DELAY, 1);
            x.set_v8(a5::FADE_PAGES, 12);
            x.set_v16(0x49a, 15);
            x.set_v16(0x49e, 15);
            goto(x, crate::taches::fondu::vers_les_couleurs, At(Etape::S32Coloured.into()))
        }
        12 => {
            if !message_done(x) {
                return Flow::Done;
            }
            fade(x, ALL_PAGES, crate::taches::fondu::descente, At(Etape::S32Faded.into()))
        }
        _ => Flow::Done,
    }
}
fn s32_message_59(x: &mut Ctx) -> Flow {
    or_yield!(message_d0(x, Etape::S32Retry59));
    next(x);
    Flow::Done
}
fn s32_message_60(x: &mut Ctx) -> Flow {
    or_yield!(message(x, 60, Etape::S32Retry60));
    next(x);
    Flow::Done
}
fn s32_message_61(x: &mut Ctx) -> Flow {
    or_yield!(message(x, 61, Etape::S32Retry61));
    Flow::Done
}
fn s32_message_62(x: &mut Ctx) -> Flow {
    or_yield!(message(x, 62, Etape::S32Retry62));
    next(x);
    Flow::Done
}

// ---------------------------------------------------------------------------------------------
// Scènes 38, 40, 42, 44 : objets système 4 et 5, dialogues 97, 73, 98, 74 ; décor selon les personnages ;
// puis la scène 47

pub fn scene_38(x: &mut Ctx) -> Flow {
    match dispatch(x, var::STATE, 0xf9bee) {
        0 => {
            next(x);
            x.call(ext::MUSIC_G);
            set_camera(x, a5::CAM2_Y, 8);
            tiles(x, TB2, 0x15c, Some(SCROLL_3));
            tiles(x, TB2, 0x15d, None);
            set8(x, var::SIGNAL, 0);
            for n in [4, 5] {
                call_d0(x, sub::SOBJ, n);
                attach(x);
            }
            world(x);
            sprites_next_frame(x, Etape::S38Drawn)
        }
        2 => {
            if get8(x, var::SIGNAL) != 0 {
                set8(x, var::SIGNAL, 0);
                call_d0(x, sub::MESSAGE, 97);
                store_message(x);
                next(x);
                set16(x, var::TIMER, 180);
            }
            Flow::Done
        }
        4 => {
            if any_button(x) {
                set16(x, var::TIMER, 1);
            }
            if !tick(x) {
                return Flow::Done;
            }
            next(x);
            s38_close_message(x)
        }
        6 => s38_close_message(x),
        8 => {
            if message_done(x) {
                next(x);
                fade_task(x, 4, SCENE_PAGES, sub::FADE_TASK_PAGES);
            }
            Flow::Done
        }
        10 => {
            if x.v8(a5::FADE_STEPS) != 0 {
                return Flow::Done;
            }
            set8(x, var::SIGNAL, 0xff);
            world(x);
            sprites_next_frame(x, Etape::S38Gone)
        }
        12 => {
            if !message_done(x) {
                return Flow::Done;
            }
            fade(x, SCENE_PAGES, crate::taches::fondu::descente, At(Etape::S38LastFaded.into()))
        }
        14 => {
            if !message_done(x) {
                return Flow::Done;
            }
            copy_palettes(x, PALETTE_PAGE, PALETTE_SAVE);
            x.call(ext::MUSIC_H);
            x.set_v16(a5::FADE_DELAY, 1);
            x.set_v8(a5::FADE_PAGES, ALL_PAGES);
            goto(x, crate::taches::fondu::vers_le_blanc, At(Etape::S38End.into()))
        }
        _ => Flow::Done,
    }
}
/// Ferme le dialogue 97 (mot `+0x32`), puis le dialogue 73.
fn s38_close_message(x: &mut Ctx) -> Flow {
    let o = message_obj(x);
    wr16(x.b, o.0 + 0x32, 1);
    world(x);
    sprites_next_frame(x, Etape::S38Closed)
}
fn s38_message_73(x: &mut Ctx) -> Flow {
    or_yield!(message(x, 73, Etape::S38Retry73));
    next(x);
    Flow::Done
}
fn s38_message_98(x: &mut Ctx) -> Flow {
    or_yield!(message(x, 98, Etape::S38Retry98));
    next(x);
    Flow::Done
}
fn s38_message_74(x: &mut Ctx) -> Flow {
    or_yield!(message(x, 74, Etape::S38Retry74));
    next(x);
    Flow::Done
}

// ---------------------------------------------------------------------------------------------
// Scène 46 : objets SET 12 et 21, dialogue 69, la caméra 2 descend ; puis la scène 46

pub fn scene_46(x: &mut Ctx) -> Flow {
    match dispatch(x, var::STATE, 0xf9e58) {
        0 => {
            // palettes de la scène 7
            let a6 = x.c.a[6];
            x.set_v8(a5::SCENE, 7);
            x.call(ext::SCENE_PALETTES);
            x.c.a[6] = a6;
            x.set_v8(a5::SCENE, 46);
            home_cameras(x, At(Etape::S46Homed.into()))
        }
        2 => {
            if get8(x, var::SUB) == 0 && x.v8(a5::FADE_STEPS) == 0 {
                call_d0(x, sub::MESSAGE, 69);
                next_sub(x);
            }
            descend(x);
            if !tick(x) {
                return Flow::Done;
            }
            next(x);
            x.set_v8(a5::FADE_STEPS, 1);
            fade_task(x, 4, ALL_PAGES, sub::FADE_TASK_PAGES);
            Flow::Done
        }
        4 => {
            descend(x);
            if x.v8(a5::FADE_STEPS) != 0 {
                return Flow::Done;
            }
            set8(x, var::SIGNAL, 0xff);
            world(x);
            sprites_next_frame(x, Etape::S46Gone)
        }
        _ => Flow::Done,
    }
}
/// La caméra 2 descend d'un demi-pixel par image (position à virgule en `+4`).
fn descend(x: &mut Ctx) {
    let y = rd32(x.b, VARS + var::MESSAGE).wrapping_add(0x8000);
    wr32(x.b, VARS + var::MESSAGE, y);
    set_camera(x, a5::CAM2_Y, (y >> 16) as u16);
}

// ---------------------------------------------------------------------------------------------
// Fin du jeu (scène 69, prise par la 105) : écran blanc clignotant, fondus, décor final ; la sortie de scène continue

pub fn finale(x: &mut Ctx) -> Flow {
    match dispatch(x, var::STATE, 0xf9fc8) {
        0 => {
            next(x);
            x.set_v8(a5::FADE_PAGES, 12);
            x.call(sub::PALETTES_WHITE);
            x.call(ext::MUSIC_FINALE);
            tiles_w(x, TB, 544, Some(0x915000));
            tiles_w(x, TB, 545, None);
            tiles_w(x, TB2, 389, Some(0x919000));
            tiles_w(x, TB2, 390, None);
            x.set_v16(a5::LAYERS, 0x12cc);
            set16(x, var::MESSAGE, 180);
            Flow::Done
        }
        _ => {
            // le plan 1 clignote pendant 180 images
            let layers = x.v8(a5::LAYERS + 1) ^ 4;
            x.set_v8(a5::LAYERS + 1, layers);
            let t = get16(x, var::MESSAGE).wrapping_sub(1);
            set16(x, var::MESSAGE, t);
            if t != 0 {
                return Flow::Done;
            }
            x.set_v8(a5::LAYERS + 1, layers | 4);
            copy_palettes(x, PALETTE_PAGE, PALETTE_SAVE);
            x.set_v8(a5::FADE_PAGES, 12);
            x.set_v16(a5::FADE_DELAY, 1);
            goto(x, crate::taches::fondu::vers_le_blanc, At(Etape::FinaleFaded.into()))
        }
    }
}

pub const NATIVES: &[Native] = &[];
