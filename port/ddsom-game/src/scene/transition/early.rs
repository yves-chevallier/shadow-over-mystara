//! Scripts de transition des premiers niveaux : scènes 1, 2, 4 et 10, 5, 14 et 20, 11, 13.

use super::*;

/// Routines du moteur propres à ces scripts.
mod ext {
    /// Musiques et sons d'ambiance des intermèdes.
    pub const MUSIC_A: u32 = 0xe14b0;
    pub const MUSIC_B: u32 = 0xe1564;
    pub const MUSIC_C: u32 = 0xe14ec;
    pub const MUSIC_D: u32 = 0xe1528;
    pub const MUSIC_E: u32 = 0xe15a0;
    pub const SCENE_PALETTES: u32 = 0x1a360;
    pub const PALETTE_SET: u32 = 0x1a366;
    pub const PALETTE_3: u32 = 0x1a8b2;
    pub const SCROLL_2_ON: u32 = 0x2a52;
}
use sub::{TILE_BLOCK as TB, TILE_BLOCK_2 as TB2};

// ---------------------------------------------------------------------------------------------
// Scène 1 : dialogue 96, puis la scène 2

pub fn scene_1(x: &mut Ctx) -> Flow {
    match dispatch(x, var::STATE, 0xf8292) {
        0 => show_and_fade_in(x, At(Etape::S1Shown.into())),
        _ => {
            if !message_done(x) {
                return Flow::Done;
            }
            x.c.d[0] = 2;
            finish(x, 2)
        }
    }
}
crate::etapes! {
    /// Étapes (points de reprise) de la machine à états.
    pub enum Etape {
        S1Shown = 0xf829a => |x| {
            call_d0(x, sub::SOUND, 8);
            next(x);
            s1_message(x)
        },
        S1Retry = 0xf82b8 => s1_message,
        S2Shown = 0xf8316 => |x| {
            x.call(ext::SCENE_PALETTES);
            x.set_v8(a5::FADE_PAGES, SCENE_PAGES);
            crate::palette::no_brightness(x.b);
            Flow::Done
        },
        S2Hidden = 0xf8382 => |x| {
            tiles(x, TB, 0x20a, Some(SCROLL_2));
            tiles(x, TB, 0x20b, None);
            tiles(x, TB2, 0x14c, Some(SCROLL_3));
            tiles(x, TB2, 0x14d, None);
            layers_or(x, 0xc);
            x.wait(1, Etape::S2Drawn)
        },
        S2Drawn = 0xf83cc => |x| fade(x, ALL_PAGES, crate::taches::fondu::montee, Caller),
        S2Faded = 0xf8456 => |x| hide_layers(x, At(Etape::S2Cleared.into())),
        S2Cleared = 0xf8464 => |x| {
            x.call(sub::CLEAR_SCROLL_2);
            x.call(sub::CLEAR_SCROLL_3);
            reset_pools(x);
            let scene = get8(x, var::MESSAGE);
            finish(x, scene)
        },
        S5Retry = 0xf84e8 => s5_message,
        S5Faded = 0xf8502 => |x| hide_layers(x, At(Etape::S5Hidden.into())),
        S5Hidden = 0xf8506 => |x| {
            tiles(x, TB, 0x24a, Some(SCROLL_2));
            tiles(x, TB, 0, None);
            show_layers(x, At(Etape::S5Shown.into()))
        },
        S5Shown = 0xf8524 => |x| fade(x, SCENE_PAGES, crate::taches::fondu::montee, At(Etape::S5FadedIn.into())),
        S5FadedIn = 0xf8528 => s5_second_message,
        S5Retry2 = 0xf853c => s5_second_message,
        S5TextFaded = 0xf8556 => |x| hide_layers(x, At(Etape::S5TextHidden.into())),
        S5TextHidden = 0xf855a => |x| {
            x.call(sub::CLEAR_SCROLL_2);
            scroll_text(x, 0xf858a);
            set16(x, var::TIMER, 120);
            sprites_next_frame(x, Etape::S5TextShown)
        },
        S5TextShown = 0xf8582 => |x| fade(x, SCENE_PAGES, crate::taches::fondu::montee, At(Etape::S5TextFadedIn.into())),
        S5TextFadedIn = 0xf8586 => |x| {
            next(x);
            Flow::Done
        },
        S5EndFaded = 0xf8734 => |x| {
            x.set_v32(a5::SCROLL_TEXT, 0);
            sprites_next_frame(x, Etape::S5EndDrawn)
        },
        S5EndDrawn = 0xf873c => |x| yield_frames(x, 1, At(Etape::S5End.into())),
        S5End = 0xf8744 => |x| {
            let scene = if x.v8(a5::SCENE) == 5 { 108 } else { 24 };
            finish(x, scene)
        },
        S4Shown = 0xf87c2 => |x| {
            // palette de la page 7 (huit longs, `movem.l` : les registres gardent les données)
            const FROM: u32 = 0x292260;
            const TO: u32 = 0x91c1e0;
            for k in 0..8 {
                let v = rd32(x.b, FROM + 4 * k);
                wr32(x.b, TO + 4 * k, v);
                if k < 7 {
                    x.c.d[k as usize] = v;
                } else {
                    x.c.a[3] = v;
                }
            }
            (x.c.a[1], x.c.a[2]) = (TO, FROM);
            scroll_text(x, 0xf8810);
            set16(x, var::TIMER, 120);
            x.set_v8(a5::FADE_PAGES, ALL_PAGES);
            crate::palette::no_brightness(x.b);
            fade_task(x, 3, ALL_PAGES, sub::FADE_TASK);
            Flow::Done
        },
        S4TextGone = 0xf890e => |x| hide_layers(x, At(Etape::S4Hidden.into())),
        S4Hidden = 0xf8912 => |x| {
            x.call(sub::CLEAR_SCROLL_2);
            x.call(sub::CLEAR_SCROLL_3);
            set_camera(x, a5::CAM1_X, 0);
            set_camera(x, a5::CAM1_Y, 0xfff0);
            yield_frames(x, 1, At(Etape::S4SecondText.into()))
        },
        S4SecondText = 0xf893c => |x| {
            tiles(x, TB, 0x20d, Some(SCROLL_2));
            tiles(x, TB, 0x20e, None);
            scroll_text(x, 0xf8986);
            set16(x, var::TIMER, 60);
            sprites_next_frame(x, Etape::S4SecondDrawn)
        },
        S4SecondDrawn = 0xf897c => |x| {
            next(x);
            show_layers(x, At(Etape::S4SecondShown.into()))
        },
        S4SecondShown = 0xf8982 => |x| fade(x, SCENE_PAGES, crate::taches::fondu::montee, Caller),
        S4Faded = 0xf8a84 => |x| hide_layers(x, At(Etape::S4FadedHidden.into())),
        S4FadedHidden = 0xf8a88 => |x| home_camera_1(x, At(Etape::S4Homed.into())),
        S4Homed = 0xf8a8c => |x| {
            tiles(x, TB, 0x210, Some(SCROLL_2));
            tiles(x, TB, 0x211, None);
            x.c.d[0] = 87;
            dark_palette(x);
            show_layers(x, At(Etape::S4Dark.into()))
        },
        S4Dark = 0xf8ab4 => |x| fade(x, SCENE_PAGES, crate::taches::fondu::montee, At(Etape::S4DarkIn.into())),
        S4DarkIn = 0xf8ab8 => s4_message_92,
        S4Retry92 = 0xf8acc => s4_message_92,
        S4RetryWizard = 0xf8aea => s4_wizard,
        S4SecondFaded = 0xf8b1c => |x| {
            set8(x, var::SIGNAL, 0xff);
            world(x);
            sprites_next_frame(x, Etape::S4SecondClear)
        },
        S4SecondClear = 0xf8b2a => |x| {
            x.call(sub::CLEAR_SPRITES);
            hide_layers(x, At(Etape::S4SecondHidden.into()))
        },
        S4SecondHidden = 0xf8b34 => |x| {
            x.call(sub::CLEAR_SCROLL_2);
            home_camera_1(x, At(Etape::S4SecondHomed.into()))
        },
        S4SecondHomed = 0xf8b3e => |x| {
            set8(x, var::SIGNAL, 0);
            s4_second_object(x)
        },
        S4RetryObject = 0xf8b56 => s4_second_object,
        S4Scenery = 0xf8b64 => |x| {
            // décor en huit blocs de tuiles du plan 2 (le troisième avec D4 = 0)
            for (n, at) in [(206, 0x917000), (173, 0x917400), (175, 0x917800), (175, 0x917c00), (172, 0x916000), (205, 0x916400),
                (174, 0x916800), (174, 0x916c00)]
            {
                if at == 0x917800 {
                    x.c.d[4] = 0;
                }
                tiles_w(x, TB, n, Some(at));
            }
            x.set_v16(a5::CAM2_Y, 0xffe0);
            x.set_v16(a5::CAM1_Y, 40);
            tiles(x, TB2, 0x150, Some(SCROLL_3));
            tiles(x, TB2, 0x151, None);
            x.call(ext::SCROLL_2_ON);
            x.set_v8(0x36, 1);
            x.set_v16(a5::LAYERS, 0x138e);
            wr16(x.b, 0x400004, 0x3155);
            show_layers(x, At(Etape::S4SceneryShown.into()))
        },
        S4SceneryShown = 0xf8c2c => |x| {
            fade_task(x, 3, SCENE_PAGES, sub::FADE_TASK);
            s4_fading_in(x)
        },
        S4FadingIn = 0xf8c44 => |x| yield_frames(x, 1, At(Etape::S4FadingInNext.into())),
        S4FadingInNext = 0xf8c4c => |x| {
            if x.v8(a5::FADE_STEPS) != 0 {
                return s4_fading_in(x);
            }
            s4_message_16(x)
        },
        S4Retry16 = 0xf8c66 => s4_message_16,
        S4FadingOut = 0xf8c9c => |x| yield_frames(x, 1, At(Etape::S4FadingOutNext.into())),
        S4FadingOutNext = 0xf8ca4 => |x| {
            if x.v8(a5::FADE_STEPS) != 0 {
                return s4_fading_out(x);
            }
            set8(x, var::SIGNAL, 1);
            world(x);
            sprites_next_frame(x, Etape::S4FadedOut)
        },
        S4FadedOut = 0xf8cb8 => |x| yield_frames(x, 1, At(Etape::S4End.into())),
        S4End = 0xf8cc0 => |x| hide_layers(x, At(Etape::S4EndHidden.into())),
        S4EndHidden = 0xf8cc4 => |x| {
            x.call(sub::CLEAR_SCROLL_2);
            x.call(sub::CLEAR_SCROLL_3);
            finish(x, 11)
        },
        S11RetryObject = 0xf8d5a => s11_object,
        S11Drawn = 0xf8d6c => |x| {
            layers_or(x, 0xe);
            yield_frames(x, 1, At(Etape::S11Shown.into()))
        },
        S11Shown = 0xf8d7a => |x| {
            set16(x, var::TIMER, 10);
            x.set_v16(a5::FADE_DELAY, 4);
            x.set_v8(a5::FADE_PAGES, ALL_PAGES);
            crate::taches::fondu::montee(x)
        },
        S11Retry17 = 0xf8db4 => s11_message_17,
        S11Retry18 = 0xf8e48 => s11_message_18,
        S11Faded = 0xf8e6a => |x| {
            set8(x, var::SIGNAL, 1);
            world(x);
            sprites_next_frame(x, Etape::S11DrawnOut)
        },
        S11DrawnOut = 0xf8e78 => |x| hide_layers(x, At(Etape::S11Hidden.into())),
        S11Hidden = 0xf8e7c => |x| home_camera_1(x, At(Etape::S11End.into())),
        S11End = 0xf8e80 => |x| {
            x.call(sub::CLEAR_SCROLL_2);
            x.call(sub::CLEAR_SCROLL_3);
            x.c.d[0] = 13;
            finish(x, 13)
        },
        S13RetryObject = 0xf8ee2 => s13_object,
        S13Drawn = 0xf8ef4 => |x| {
            layers_or(x, 0xe);
            yield_frames(x, 1, At(Etape::S13Shown.into()))
        },
        S13Shown = 0xf8f02 => |x| {
            x.set_v16(a5::FADE_DELAY, 4);
            x.set_v8(a5::FADE_PAGES, ALL_PAGES);
            crate::taches::fondu::montee(x)
        },
        S13LeftDrawn = 0xf8f98 => |x| yield_frames(x, 1, At(Etape::S13LeftWaited.into())),
        S13LeftWaited = 0xf8fa0 => |x| hide_layers(x, At(Etape::S13LeftHidden.into())),
        S13LeftHidden = 0xf8fa4 => |x| yield_frames(x, 4, At(Etape::S13LeftScenery.into())),
        S13LeftScenery = 0xf8fac => |x| {
            let a6 = x.c.a[6];
            call_d0(x, ext::PALETTE_SET, 1);
            call_d0(x, sub::LOAD_PALETTE, 29);
            call_d0(x, ext::PALETTE_3, 11);
            x.c.a[6] = a6;
            x.set_v8(0x36, 1);
            tiles(x, TB, 8, Some(SCROLL_2));
            for n in [9, 10, 8] {
                tiles(x, TB, n, None);
            }
            tiles(x, TB2, 115, Some(SCROLL_3));
            tiles(x, TB2, 116, None);
            s13_set_11(x)
        },
        S13Retry11 = 0xf9022 => s13_set_11,
        S13Set11Drawn = 0xf9034 => |x| yield_frames(x, 1, At(Etape::S13Set11Shown.into())),
        S13Set11Shown = 0xf903c => |x| {
            crate::sound::service(x.b, crate::sound::Service::Ff07 { verrou: 144, ligne: 0 });
            x.set_v16(a5::LAYERS, 0x138e);
            x.set_v8(a5::FADE_PAGES, SCENE_PAGES);
            x.set_v16(a5::FADE_DELAY, 4);
            x.set_v8(a5::FADE_STEPS, 1);
            start_task(x, sub::FADE_TASK);
            Flow::Done
        },
        S13OutFaded = 0xf90cc => |x| yield_frames(x, 30, At(Etape::S13OutScenery.into())),
        S13OutScenery = 0xf90d4 => s13_out_scenery,
        S13OutSceneryDrawn = 0xf911c => |x| {
            set8(x, var::SIGNAL, 0);
            s13_set_9(x)
        },
        S13Retry9 = 0xf9134 => s13_set_9,
        S13OutDrawn = 0xf9150 => |x| {
            next_sub(x);
            crate::sound::jouer_global(x.b, ext::MUSIC_E);
            for (off, palettes) in [(0x492, 0x253000), (0x49a, 0x272a60), (0x49e, 0x283140)] {
                x.set_v32(off, palettes);
            }
            x.set_v16(a5::FADE_DELAY, 1);
            x.set_v8(a5::FADE_PAGES, SCENE_PAGES);
            goto(x, crate::taches::fondu::vers_les_palettes, Caller)
        },
        /// état 6 : nouveau décor, dialogue 30
        S13Closing = 0xf919e => |x| yield_frames(x, 1, At(Etape::S13ClosingWaited.into())),
        S13ClosingWaited = 0xf91a6 => |x| hide_layers(x, At(Etape::S13ClosingHidden.into())),
        S13ClosingHidden = 0xf91aa => |x| {
            set_camera(x, a5::CAM1_X, 8);
            for off in [a5::CAM1_Y, a5::CAM2_X, a5::CAM2_Y] {
                set_camera(x, off, 0);
            }
            x.call(sub::CLEAR_SCROLL_3);
            set_d0_word(x, 87);
            x.call(sub::LOAD_PALETTE);
            set_d0_word(x, 87);
            x.call(ext::PALETTE_3);
            tiles_w(x, TB, 0x216, Some(SCROLL_2));
            tiles_w(x, TB, 0x217, None);
            yield_frames(x, 4, At(Etape::S13ClosingDrawn.into()))
        },
        S13ClosingDrawn = 0xf9210 => |x| show_layers(x, At(Etape::S13ClosingShown.into())),
        S13ClosingShown = 0xf9214 => |x| fade(x, SCENE_PAGES, crate::taches::fondu::montee, At(Etape::S13ClosingIn.into())),
        S13ClosingIn = 0xf921a => s13_message_30,
        S13Retry30 = 0xf922e => s13_message_30,
        S13Faded = 0xf938a => |x| hide_layers(x, At(Etape::S13End.into())),
        S13End = 0xf938e => |x| {
            x.call(sub::CLEAR_SCROLL_2);
            let scene = get8(x, var::FLAG);
            x.c.d[0] = scene as u32;
            finish(x, scene)
        },
    }
}
fn s1_message(x: &mut Ctx) -> Flow {
    or_yield!(message(x, 96, Etape::S1Retry));
    Flow::Done
}

// ---------------------------------------------------------------------------------------------
// Scène 2 : dialogues 9 à 13 ; la réponse au dernier choisit la scène suivante (table 0xf847c)

pub fn scene_2(x: &mut Ctx) -> Flow {
    match dispatch(x, var::STATE, 0xf82e4) {
        0 => {
            crate::sound::jouer_global(x.b, ext::MUSIC_A);
            if !try_message(x, 9) {
                return Flow::Done;
            }
            next(x);
            x.set_v16(a5::LAYERS, 0x12c0);
            show_and_fade_in(x, At(Etape::S2Shown.into()))
        }
        2 => {
            if message_done(x) {
                next(x);
                call_d0(x, sub::SET, 8);
            }
            Flow::Done
        }
        4 => {
            if try_message(x, 10) {
                next(x);
            }
            Flow::Done
        }
        6 => {
            if !message_done(x) {
                return Flow::Done;
            }
            next(x);
            for off in [a5::CAM1_Y, a5::CAM1_Y + a5::COPY, a5::CAM2_Y, a5::CAM2_Y + a5::COPY] {
                x.set_v16(off, 0xfff0);
            }
            hide_layers(x, At(Etape::S2Hidden.into()))
        }
        8 => {
            if try_message(x, 11) {
                next(x);
            }
            Flow::Done
        }
        10 | 12 => {
            let n = if get8(x, var::STATE) == 10 { 12 } else { 13 };
            if message_done(x) && try_message(x, n) {
                next(x);
            }
            Flow::Done
        }
        14 => {
            if !message_done(x) {
                return Flow::Done;
            }
            let scene = answer(x, 0xf847c);
            set8(x, var::MESSAGE, scene);
            call_d0(x, sub::SOUND, 0);
            fade(x, ALL_PAGES, crate::taches::fondu::descente, At(Etape::S2Faded.into()))
        }
        _ => Flow::Done,
    }
}

// ---------------------------------------------------------------------------------------------
// Scènes 5, 14 et 20 : dialogues 93 et 94, texte défilant ; scène 108 après la 5, sinon 24

pub fn scene_5(x: &mut Ctx) -> Flow {
    match dispatch(x, var::STATE, 0xf848a) {
        0 => {
            next(x);
            crate::sound::jouer_global(x.b, ext::MUSIC_B);
            tiles(x, TB, 0x218, Some(SCROLL_2));
            tiles(x, TB, 0x219, None);
            set16(x, var::TIMER, 60);
            show_and_fade_in(x, Caller)
        }
        2 => {
            if !any_button(x) && !tick(x) {
                return Flow::Done;
            }
            s5_message(x)
        }
        4 => {
            if !message_done(x) {
                return Flow::Done;
            }
            fade(x, SCENE_PAGES, crate::taches::fondu::descente, At(Etape::S5Faded.into()))
        }
        6 => {
            if !message_done(x) {
                return Flow::Done;
            }
            fade(x, SCENE_PAGES, crate::taches::fondu::descente, At(Etape::S5TextFaded.into()))
        }
        8 => {
            if !any_button(x) && !tick(x) {
                return Flow::Done;
            }
            call_d0(x, sub::SOUND, 0);
            fade(x, SCENE_PAGES, crate::taches::fondu::descente, At(Etape::S5EndFaded.into()))
        }
        _ => Flow::Done,
    }
}
fn s5_message(x: &mut Ctx) -> Flow {
    or_yield!(message(x, 93, Etape::S5Retry));
    next(x);
    Flow::Done
}
fn s5_second_message(x: &mut Ctx) -> Flow {
    or_yield!(message(x, 94, Etape::S5Retry2));
    next(x);
    Flow::Done
}

// ---------------------------------------------------------------------------------------------
// Scènes 4 et 10 : textes défilants, dialogues 15, 92 et 16, deux objets système ; puis la scène 11

pub fn scene_4(x: &mut Ctx) -> Flow {
    match dispatch(x, var::STATE, 0xf876a) {
        0 => {
            next(x);
            crate::sound::jouer_global(x.b, ext::MUSIC_A);
            tiles_w(x, TB, 0x20c, Some(SCROLL_2));
            tiles_w(x, TB, 0x20c, None);
            tiles_w(x, TB2, 0x14e, Some(SCROLL_3));
            tiles_w(x, TB2, 0x14f, None);
            x.set_v16(a5::LAYERS, 0x138e);
            yield_frames(x, 1, At(Etape::S4Shown.into()))
        }
        2 => {
            drift(x);
            if x.v8(a5::FADE_STEPS) != 0 {
                return Flow::Done;
            }
            if any_button(x) {
                set16(x, var::TIMER, 0);
            }
            if get16(x, var::TIMER) != 0 {
                return Flow::Done;
            }
            next(x);
            fade_task(x, 4, SCENE_PAGES, sub::FADE_TASK_PAGES);
            Flow::Done
        }
        4 => {
            drift(x);
            if x.v8(a5::FADE_STEPS) != 0 {
                return Flow::Done;
            }
            x.set_v32(a5::SCROLL_TEXT, 0);
            sprites_next_frame(x, Etape::S4TextGone)
        }
        6 => {
            if any_button(x) {
                set16(x, var::TIMER, 1);
            }
            if tick(x) && try_message(x, 15) {
                next(x);
                x.set_v32(a5::SCROLL_TEXT, 0);
            }
            Flow::Done
        }
        8 => {
            if !message_done(x) {
                return Flow::Done;
            }
            fade(x, SCENE_PAGES, crate::taches::fondu::descente, At(Etape::S4Faded.into()))
        }
        10 => {
            if !message_done(x) {
                return Flow::Done;
            }
            fade(x, SCENE_PAGES, crate::taches::fondu::descente, At(Etape::S4SecondFaded.into()))
        }
        12 => {
            if !message_done(x) {
                return Flow::Done;
            }
            call_d0(x, sub::SOUND, 0);
            fade_task(x, 3, SCENE_PAGES, sub::FADE_TASK_PAGES);
            s4_fading_out(x)
        }
        _ => Flow::Done,
    }
}

/// La minuterie descend ; toutes les quatre images, la caméra 1 avance d'un pixel.
fn drift(x: &mut Ctx) {
    let t = get16(x, var::TIMER).wrapping_sub(1);
    set16(x, var::TIMER, t);
    let phase = t as u8 & 3;
    set_d0_byte(x, phase);
    if phase == 0 {
        let cam = x.v16(a5::CAM1_X).wrapping_add(1);
        set_camera(x, a5::CAM1_X, cam);
    }
}

fn s4_message_92(x: &mut Ctx) -> Flow {
    or_yield!(message(x, 92, Etape::S4Retry92));
    set8(x, var::SIGNAL, 0);
    s4_wizard(x)
}
/// Objet système 0, placé (sous-type 2, X 144, Y 188).
fn s4_wizard(x: &mut Ctx) -> Flow {
    let Some(o) = system_object(x, 0, Etape::S4RetryWizard) else { return Flow::Yield };
    wr8(x.b, o.0 + 0x2f, 2);
    wr16(x.b, o.0 + 8, 144);
    wr16(x.b, o.0 + 0x10, 0);
    wr16(x.b, o.0 + 0xc, 188);
    next(x);
    Flow::Done
}

fn s4_second_object(x: &mut Ctx) -> Flow {
    if system_object(x, 1, Etape::S4RetryObject).is_none() {
        return Flow::Yield;
    }
    world(x);
    sprites_next_frame(x, Etape::S4Scenery)
}
/// Le monde tourne jusqu'à la fin du fondu (0xf8c3c), puis le dialogue 16.
fn s4_fading_in(x: &mut Ctx) -> Flow {
    world(x);
    sprites_next_frame(x, Etape::S4FadingIn)
}
fn s4_message_16(x: &mut Ctx) -> Flow {
    or_yield!(message(x, 16, Etape::S4Retry16));
    next(x);
    Flow::Done
}
/// 0xf8c94 : le monde tourne jusqu'à la fin du fondu de sortie.
fn s4_fading_out(x: &mut Ctx) -> Flow {
    world(x);
    sprites_next_frame(x, Etape::S4FadingOut)
}

// ---------------------------------------------------------------------------------------------
// Scène 11 (s'il reste du temps) : objet système 2, dialogues 17 et 18 ; puis la scène 13

pub fn scene_11(x: &mut Ctx) -> Flow {
    match dispatch(x, var::STATE, 0xf8ce4) {
        0 => {
            if x.v8(a5::CLOCK) == 0 {
                x.c.d[0] = 13;
                return finish(x, 13);
            }
            next(x);
            crate::sound::jouer_global(x.b, ext::MUSIC_C);
            set_camera(x, a5::CAM1_X, 288);
            tiles(x, TB, 0x214, Some(0x917400));
            tiles(x, TB, 0x215, None);
            tiles(x, TB2, 0x152, Some(SCROLL_3));
            tiles(x, TB2, 0x153, None);
            s11_object(x)
        }
        2 => {
            if !tick(x) {
                return Flow::Done;
            }
            set16(x, var::TIMER, 60);
            s11_message_17(x)
        }
        4 => {
            if get8(x, var::FLAG) != 0 && message_done(x) {
                set8(x, var::FLAG, 0);
                set16(x, var::TIMER, 1);
            }
            if tick(x) {
                set16(x, var::TIMER, 120);
                set8(x, var::SIGNAL, 1);
                next(x);
            }
            Flow::Done
        }
        6 => {
            if get8(x, var::FLAG) != 0 && message_done(x) {
                set8(x, var::FLAG, 0);
            }
            if get16(x, var::TIMER) != 0 {
                if !tick(x) {
                    return Flow::Done;
                }
                set8(x, var::SIGNAL, 0);
            }
            let busy = get8(x, var::SIGNAL).wrapping_add(get8(x, var::FLAG));
            x.c.d[0] = busy as u32;
            if busy != 0 {
                return Flow::Done;
            }
            s11_message_18(x)
        }
        8 => {
            if !message_done(x) {
                return Flow::Done;
            }
            call_d0(x, sub::SOUND, 0);
            fade(x, SCENE_PAGES, crate::taches::fondu::descente, At(Etape::S11Faded.into()))
        }
        _ => Flow::Done,
    }
}
fn s11_object(x: &mut Ctx) -> Flow {
    if system_object(x, 2, Etape::S11RetryObject).is_none() {
        return Flow::Yield;
    }
    set8(x, var::SIGNAL, 0);
    world(x);
    sprites_next_frame(x, Etape::S11Drawn)
}
fn s11_message_17(x: &mut Ctx) -> Flow {
    or_yield!(message(x, 17, Etape::S11Retry17));
    next(x);
    set8(x, var::FLAG, 1);
    Flow::Done
}
fn s11_message_18(x: &mut Ctx) -> Flow {
    or_yield!(message(x, 18, Etape::S11Retry18));
    next(x);
    Flow::Done
}

// ---------------------------------------------------------------------------------------------
// Scène 13 : selon le temps restant, objets SET 10 et 11 ou 9 ; dialogue 30 puis un choix (dialogues 136 à 138)
// qui fixe la scène suivante (tables 0xf92e4, 0xf932e, 0xf937a ; 14 si aucun joueur n'est dans l'état attendu)

pub fn scene_13(x: &mut Ctx) -> Flow {
    match dispatch(x, var::STATE, 0xf8e9e) {
        0 => {
            next(x);
            crate::sound::jouer_global(x.b, ext::MUSIC_D);
            tiles_w(x, TB2, 0x154, Some(SCROLL_3));
            tiles_w(x, TB2, 0x155, None);
            s13_object(x)
        }
        2 => {
            if get8(x, var::SIGNAL) != 0 {
                set8(x, var::SIGNAL, 0);
                next(x);
                set8(x, var::SUB, 0);
            }
            Flow::Done
        }
        4 if x.v16(a5::CLOCK) != 0 => s13_time_left(x),
        4 => s13_time_out(x),
        6 => {
            world(x);
            sprites_next_frame(x, Etape::S13Closing)
        }
        8 => {
            if !message_done(x) {
                return Flow::Done;
            }
            next(x);
            set8(x, var::SUB, 0);
            s13_route(x);
            Flow::Done
        }
        10 => match dispatch(x, var::FLAG, 0xf9294) {
            0 => choice(x, 138, 0xf92a8, 0xf92e4),
            2 => choice(x, 136, 0xf92f2, 0xf932e),
            6 => choice(x, 137, 0xf933e, 0xf937a),
            _ => {
                next(x);
                set8(x, var::FLAG, 14);
                Flow::Done
            }
        },
        12 => fade(x, SCENE_PAGES, crate::taches::fondu::descente, At(Etape::S13Faded.into())),
        _ => Flow::Done,
    }
}
fn s13_object(x: &mut Ctx) -> Flow {
    if system_object(x, 3, Etape::S13RetryObject).is_none() {
        return Flow::Yield;
    }
    set8(x, var::SIGNAL, 0);
    world(x);
    sprites_next_frame(x, Etape::S13Drawn)
}

/// Temps restant : objet SET 10, fondu, nouveau décor et objet SET 11 ; l'objet signale sa fin (`+8`).
fn s13_time_left(x: &mut Ctx) -> Flow {
    match dispatch(x, var::SUB, 0xf8f4e) {
        0 => {
            if try_set_object(x, 10).is_some() {
                next_sub(x);
                x.set_v8(a5::FADE_STEPS, 1);
                fade_task(x, 4, SCENE_PAGES, sub::FADE_TASK_PAGES);
            }
            Flow::Done
        }
        2 => {
            if x.v8(a5::FADE_STEPS) != 0 {
                return Flow::Done;
            }
            set8(x, var::SIGNAL, 0xff);
            world(x);
            sprites_next_frame(x, Etape::S13LeftDrawn)
        }
        4 => {
            if x.v8(a5::FADE_STEPS) == 0 {
                set8(x, var::SIGNAL, 1);
                next_sub(x);
            }
            Flow::Done
        }
        6 => {
            if get8(x, var::SIGNAL) == 2 {
                next_sub(x);
            }
            Flow::Done
        }
        8 => s13_leave(x),
        _ => Flow::Done,
    }
}
/// Fin de l'intermède quand l'objet a signalé (`+8` négatif) : fondu vers le noir.
fn s13_leave(x: &mut Ctx) -> Flow {
    if (get8(x, var::SIGNAL) as i8) >= 0 {
        return Flow::Done;
    }
    next(x);
    set8(x, var::SUB, 0);
    fade(x, SCENE_PAGES, crate::taches::fondu::descente, Caller)
}
fn s13_set_11(x: &mut Ctx) -> Flow {
    or_yield!(set_object(x, 11, Etape::S13Retry11));
    next_sub(x);
    world(x);
    sprites_next_frame(x, Etape::S13Set11Drawn)
}

/// Temps écoulé : fondu par composante, autre décor, objet SET 9, puis fondu vers des palettes fixées.
fn s13_time_out(x: &mut Ctx) -> Flow {
    match dispatch(x, var::SUB, 0xf90a6) {
        0 => {
            call_d0(x, sub::SOUND, 0);
            next_sub(x);
            x.set_v16(a5::FADE_DELAY, 1);
            x.set_v8(a5::FADE_PAGES, SCENE_PAGES);
            goto(x, crate::taches::fondu::vers_le_blanc, At(Etape::S13OutFaded.into()))
        }
        2 => s13_out_scenery(x),
        4 => {
            if get8(x, var::SIGNAL) == 0 {
                return Flow::Done;
            }
            set8(x, var::SIGNAL, 0);
            world(x);
            sprites_next_frame(x, Etape::S13OutDrawn)
        }
        6 => s13_leave(x),
        _ => Flow::Done,
    }
}
fn s13_out_scenery(x: &mut Ctx) -> Flow {
    next_sub(x);
    tiles_w(x, TB, 0x22a, Some(SCROLL_2));
    tiles_w(x, TB, 0x22b, None);
    tiles_w(x, TB2, 0x164, Some(SCROLL_3));
    tiles(x, TB2, 0x165, None);
    set8(x, var::SIGNAL, 0xff);
    world(x);
    sprites_next_frame(x, Etape::S13OutSceneryDrawn)
}
fn s13_set_9(x: &mut Ctx) -> Flow {
    or_yield!(set_object(x, 9, Etape::S13Retry9));
    Flow::Done
}

fn s13_message_30(x: &mut Ctx) -> Flow {
    or_yield!(message(x, 30, Etape::S13Retry30));
    next(x);
    Flow::Done
}

/// État 8 : la question à poser (`+6`) dépend des joueurs : 0 si un joueur en jeu (état `+4` < 4) est dans la
/// posture 4 ou 5 (`+3`), 4 sinon ; plus 2 s'il reste du temps.
fn s13_route(x: &mut Ctx) {
    let mut route = 0u8;
    let mut found = false;
    for k in 0..4u32 {
        let p = A5 + a5::PLAYERS + k * 0x200;
        x.c.a[1] = p;
        x.c.d[1] = 3 - k;
        if rd8(x.b, p) != 0 && (rd8(x.b, p + 4) as i8) < 4 {
            let pose = (rd8(x.b, p + 3) >> 1).wrapping_sub(2);
            x.c.d[2] = x.c.d[2] & !0xff | pose as u32;
            if pose == 0 {
                found = true;
                break;
            }
        }
    }
    if !found {
        x.c.a[1] = A5 + a5::PLAYERS + 0x800;
        x.c.d[1] = 0xffff;
        route += 4;
    }
    if x.v16(a5::CLOCK) != 0 {
        route += 2;
    }
    x.c.d[0] = route as u32;
    set8(x, var::FLAG, route);
}

/// Question `n`, puis la scène suivante d'après la réponse (table `answers`).
fn choice(x: &mut Ctx, n: u16, table: u32, answers: u32) -> Flow {
    match dispatch(x, var::SUB, table) {
        0 => {
            set_d0_word(x, n);
            x.call(sub::MESSAGE);
            if x.c.sr & 1 == 0 {
                next_sub(x);
                store_message(x);
            }
        }
        _ => {
            if message_done(x) {
                let scene = answer(x, answers);
                set8(x, var::FLAG, scene);
                next(x);
                set8(x, var::SUB, 0);
            }
        }
    }
    Flow::Done
}

pub const NATIVES: &[Native] = &[];
