//! Transition scripts of the first levels: scenes 1, 2, 4 and 10, 5, 14 and 20, 11, 13.

use super::*;
use crate::screens::scene::fading;

mod timed;
use timed::*;
use crate::screens::obj;
pub use timed::{scene_11, scene_13};

/// Engine routines specific to these scripts.
mod ext {
    /// Music and ambient sounds of the interludes.
    pub const MUSIC_A: crate::services::sound::Sound = crate::services::sound::CMD_2B;
    pub const MUSIC_B: crate::services::sound::Sound = crate::services::sound::CMD_16;
    pub const MUSIC_C: crate::services::sound::Sound = crate::services::sound::CMD_19;
    pub const MUSIC_D: crate::services::sound::Sound = crate::services::sound::CMD_2C;
    pub const MUSIC_E: crate::services::sound::Sound = crate::services::sound::CMD_2F;
}

// ---------------------------------------------------------------------------------------------
// Scene 1: dialogue 96, then scene 2

pub fn scene_1(x: &mut Ctx) -> Flow {
    match dispatch(x, var::STATE, 0xf8292) {
        0 => show_and_fade_in(x, At(Step::S1Shown.into())),
        _ => {
            if !message_done(x) {
                return Flow::Done;
            }
            finish(x, 2)
        }
    }
}
crate::steps! {
    /// Steps (resume points) of the state machine.
    pub enum Step {
        S1Shown = 0xf829a => |x| {
            sound(x, 8);
            next(x);
            s1_message(x)
        },
        S1Retry = 0xf82b8 => s1_message,
        S2Shown = 0xf8316 => |x| {
            crate::services::palette::load_scene_palettes(x.b);
            fading::PAGES.set(x.b, SCENE_PAGES);
            crate::services::palette::no_brightness(x.b);
            Flow::Done
        },
        S2Hidden = 0xf8382 => |x| {
            let end = tiles(x, TB, 0x20a, SCROLL_2);
            tiles(x, TB, 0x20b, end);
            let end = tiles(x, TB2, 0x14c, SCROLL_3);
            tiles(x, TB2, 0x14d, end);
            layers_or(x, 0xc);
            x.wait(1, Step::S2Drawn)
        },
        S2Drawn = 0xf83cc => |x| fade(x, ALL_PAGES, crate::core::tasks::fading::rise, Caller),
        S2Faded = 0xf8456 => |x| hide_layers(x, At(Step::S2Cleared.into())),
        S2Cleared = 0xf8464 => |x| {
            clear_layer(x.b, Layer::Two);
            clear_layer(x.b, Layer::Three);
            reset_pools(x);
            let scene = VARS_BLOCK.get8(x.b, var::MESSAGE);
            finish(x, scene)
        },
        S5Retry = 0xf84e8 => s5_message,
        S5Faded = 0xf8502 => |x| hide_layers(x, At(Step::S5Hidden.into())),
        S5Hidden = 0xf8506 => |x| {
            let end = tiles(x, TB, 0x24a, SCROLL_2);
            tiles(x, TB, 0, end);
            show_layers(x, At(Step::S5Shown.into()))
        },
        S5Shown = 0xf8524 => |x| fade(x, SCENE_PAGES, crate::core::tasks::fading::rise, At(Step::S5FadedIn.into())),
        S5FadedIn = 0xf8528 => s5_second_message,
        S5Retry2 = 0xf853c => s5_second_message,
        S5TextFaded = 0xf8556 => |x| hide_layers(x, At(Step::S5TextHidden.into())),
        S5TextHidden = 0xf855a => |x| {
            clear_layer(x.b, Layer::Two);
            scroll_text(x, 0xf858a);
            VARS_BLOCK.set16(x.b, var::TIMER, 120);
            sprites_next_frame(x, Step::S5TextShown)
        },
        S5TextShown = 0xf8582 => |x| fade(x, SCENE_PAGES, crate::core::tasks::fading::rise, At(Step::S5TextFadedIn.into())),
        S5TextFadedIn = 0xf8586 => |x| {
            next(x);
            Flow::Done
        },
        S5EndFaded = 0xf8734 => |x| {
            x.set_v32(a5::SCROLL_TEXT, 0);
            sprites_next_frame(x, Step::S5EndDrawn)
        },
        S5EndDrawn = 0xf873c => |x| yield_frames(x, 1, At(Step::S5End.into())),
        S5End = 0xf8744 => |x| {
            let scene = if x.v8(a5::SCENE) == 5 { 108 } else { 24 };
            finish(x, scene)
        },
        S4Shown = 0xf87c2 => |x| {
            // palette of page 7 (eight longs, `movem.l`: the registers keep the data)
            const FROM: crate::world::screen_tables::Rom = crate::world::screen_tables::Rom::at(0x292260);
            const TO: crate::hw::video::Vram = crate::hw::video::palette(0, 0xf0);
            for k in 0..8 {
                let v = FROM.long(x.b, 4 * k);
                TO.offset(4 * k).set_long(x.b, v);
            }
            scroll_text(x, 0xf8810);
            VARS_BLOCK.set16(x.b, var::TIMER, 120);
            fading::PAGES.set(x.b, ALL_PAGES);
            crate::services::palette::no_brightness(x.b);
            fade_task(x, 3, ALL_PAGES, crate::screens::scene::fading::RISE);
            Flow::Done
        },
        S4TextGone = 0xf890e => |x| hide_layers(x, At(Step::S4Hidden.into())),
        S4Hidden = 0xf8912 => |x| {
            clear_layer(x.b, Layer::Two);
            clear_layer(x.b, Layer::Three);
            set_camera(x, a5::CAM1_X, 0);
            set_camera(x, a5::CAM1_Y, 0xfff0);
            yield_frames(x, 1, At(Step::S4SecondText.into()))
        },
        S4SecondText = 0xf893c => |x| {
            let end = tiles(x, TB, 0x20d, SCROLL_2);
            tiles(x, TB, 0x20e, end);
            scroll_text(x, 0xf8986);
            VARS_BLOCK.set16(x.b, var::TIMER, 60);
            sprites_next_frame(x, Step::S4SecondDrawn)
        },
        S4SecondDrawn = 0xf897c => |x| {
            next(x);
            show_layers(x, At(Step::S4SecondShown.into()))
        },
        S4SecondShown = 0xf8982 => |x| fade(x, SCENE_PAGES, crate::core::tasks::fading::rise, Caller),
        S4Faded = 0xf8a84 => |x| hide_layers(x, At(Step::S4FadedHidden.into())),
        S4FadedHidden = 0xf8a88 => |x| home_camera_1(x, At(Step::S4Homed.into())),
        S4Homed = 0xf8a8c => |x| {
            let end = tiles(x, TB, 0x210, SCROLL_2);
            tiles(x, TB, 0x211, end);
            dark_palette(x, 87);
            show_layers(x, At(Step::S4Dark.into()))
        },
        S4Dark = 0xf8ab4 => |x| fade(x, SCENE_PAGES, crate::core::tasks::fading::rise, At(Step::S4DarkIn.into())),
        S4DarkIn = 0xf8ab8 => s4_message_92,
        S4Retry92 = 0xf8acc => s4_message_92,
        S4RetryWizard = 0xf8aea => s4_wizard,
        S4SecondFaded = 0xf8b1c => |x| {
            VARS_BLOCK.set8(x.b, var::SIGNAL, 0xff);
            world(x);
            sprites_next_frame(x, Step::S4SecondClear)
        },
        S4SecondClear = 0xf8b2a => |x| {
            crate::services::video::clear_objects(x.b);
            hide_layers(x, At(Step::S4SecondHidden.into()))
        },
        S4SecondHidden = 0xf8b34 => |x| {
            clear_layer(x.b, Layer::Two);
            home_camera_1(x, At(Step::S4SecondHomed.into()))
        },
        S4SecondHomed = 0xf8b3e => |x| {
            VARS_BLOCK.set8(x.b, var::SIGNAL, 0);
            s4_second_object(x)
        },
        S4RetryObject = 0xf8b56 => s4_second_object,
        S4Scenery = 0xf8b64 => |x| {
            // terrain in eight plane 2 tile blocks (the third with D4 = 0)
            for (n, at) in [(206, crate::hw::video::SCROLL2.offset(0x3000).0), (173, crate::hw::video::SCROLL2.offset(0x3400).0), (175, crate::hw::video::SCROLL2.offset(0x3800).0), (175, crate::hw::video::SCROLL2.offset(0x3c00).0), (172, crate::hw::video::SCROLL2.offset(0x2000).0), (205, crate::hw::video::SCROLL2.offset(0x2400).0),
                (174, crate::hw::video::SCROLL2.offset(0x2800).0), (174, crate::hw::video::SCROLL2.offset(0x2c00).0)]
            {
                tiles(x, TB, n, at);
            }
            x.set_v16(a5::CAM2_Y, 0xffe0);
            x.set_v16(a5::CAM1_Y, 40);
            let end = tiles(x, TB2, 0x150, SCROLL_3);
            tiles(x, TB2, 0x151, end);
            crate::actors::families::small::background_2_on(x.b);
            x.set_v8(0x36, 1);
            x.set_v16(a5::LAYERS, 0x138e);
            crate::hw::video::regs::OBJECT_OUTPUT.set(x.b, 0x3155);
            show_layers(x, At(Step::S4SceneryShown.into()))
        },
        S4SceneryShown = 0xf8c2c => |x| {
            fade_task(x, 3, SCENE_PAGES, crate::screens::scene::fading::RISE);
            s4_fading_in(x)
        },
        S4FadingIn = 0xf8c44 => |x| yield_frames(x, 1, At(Step::S4FadingInNext.into())),
        S4FadingInNext = 0xf8c4c => |x| {
            if fading::STEP.get(x.b) != 0 {
                return s4_fading_in(x);
            }
            s4_message_16(x)
        },
        S4Retry16 = 0xf8c66 => s4_message_16,
        S4FadingOut = 0xf8c9c => |x| yield_frames(x, 1, At(Step::S4FadingOutNext.into())),
        S4FadingOutNext = 0xf8ca4 => |x| {
            if fading::STEP.get(x.b) != 0 {
                return s4_fading_out(x);
            }
            VARS_BLOCK.set8(x.b, var::SIGNAL, 1);
            world(x);
            sprites_next_frame(x, Step::S4FadedOut)
        },
        S4FadedOut = 0xf8cb8 => |x| yield_frames(x, 1, At(Step::S4End.into())),
        S4End = 0xf8cc0 => |x| hide_layers(x, At(Step::S4EndHidden.into())),
        S4EndHidden = 0xf8cc4 => |x| {
            clear_layer(x.b, Layer::Two);
            clear_layer(x.b, Layer::Three);
            finish(x, 11)
        },
        S11RetryObject = 0xf8d5a => s11_object,
        S11Drawn = 0xf8d6c => |x| {
            layers_or(x, 0xe);
            yield_frames(x, 1, At(Step::S11Shown.into()))
        },
        S11Shown = 0xf8d7a => |x| {
            VARS_BLOCK.set16(x.b, var::TIMER, 10);
            fading::DELAY.set(x.b, 4);
            fading::PAGES.set(x.b, ALL_PAGES);
            crate::core::tasks::fading::rise(x)
        },
        S11Retry17 = 0xf8db4 => s11_message_17,
        S11Retry18 = 0xf8e48 => s11_message_18,
        S11Faded = 0xf8e6a => |x| {
            VARS_BLOCK.set8(x.b, var::SIGNAL, 1);
            world(x);
            sprites_next_frame(x, Step::S11DrawnOut)
        },
        S11DrawnOut = 0xf8e78 => |x| hide_layers(x, At(Step::S11Hidden.into())),
        S11Hidden = 0xf8e7c => |x| home_camera_1(x, At(Step::S11End.into())),
        S11End = 0xf8e80 => |x| {
            clear_layer(x.b, Layer::Two);
            clear_layer(x.b, Layer::Three);
            finish(x, 13)
        },
        S13RetryObject = 0xf8ee2 => s13_object,
        S13Drawn = 0xf8ef4 => |x| {
            layers_or(x, 0xe);
            yield_frames(x, 1, At(Step::S13Shown.into()))
        },
        S13Shown = 0xf8f02 => |x| {
            fading::DELAY.set(x.b, 4);
            fading::PAGES.set(x.b, ALL_PAGES);
            crate::core::tasks::fading::rise(x)
        },
        S13LeftDrawn = 0xf8f98 => |x| yield_frames(x, 1, At(Step::S13LeftWaited.into())),
        S13LeftWaited = 0xf8fa0 => |x| hide_layers(x, At(Step::S13LeftHidden.into())),
        S13LeftHidden = 0xf8fa4 => |x| yield_frames(x, 4, At(Step::S13LeftScenery.into())),
        S13LeftScenery = 0xf8fac => |x| {
            crate::services::palette::load_scene_palettes_of(x.b, 1);
            crate::services::palette::load_palette_page(x.b, 2, 29);
            crate::services::palette::load_palette_page(x.b, 3, 11);
            x.set_v8(0x36, 1);
            // four blocks in a row: 8, 9, 10, 8
            let mut end = tiles(x, TB, 8, SCROLL_2);
            for n in [9, 10] {
                end = tiles(x, TB, n, end);
            }
            tiles(x, TB, 8, end);
            let end = tiles(x, TB2, 115, SCROLL_3);
            tiles(x, TB2, 116, end);
            s13_set_11(x)
        },
        S13Retry11 = 0xf9022 => s13_set_11,
        S13Set11Drawn = 0xf9034 => |x| yield_frames(x, 1, At(Step::S13Set11Shown.into())),
        S13Set11Shown = 0xf903c => |x| {
            crate::services::sound::service(x.b, crate::services::sound::Service::Ff07 { lock: 144, line: 0 });
            x.set_v16(a5::LAYERS, 0x138e);
            fading::PAGES.set(x.b, SCENE_PAGES);
            fading::DELAY.set(x.b, 4);
            fading::STEP.set(x.b, 1);
            start_task(x, crate::screens::scene::fading::RISE);
            Flow::Done
        },
        S13OutFaded = 0xf90cc => |x| yield_frames(x, 30, At(Step::S13OutScenery.into())),
        S13OutScenery = 0xf90d4 => s13_out_scenery,
        S13OutSceneryDrawn = 0xf911c => |x| {
            VARS_BLOCK.set8(x.b, var::SIGNAL, 0);
            s13_set_9(x)
        },
        S13Retry9 = 0xf9134 => s13_set_9,
        S13OutDrawn = 0xf9150 => |x| {
            next_sub(x);
            crate::services::sound::play_global(x.b, ext::MUSIC_E);
            for (off, palettes) in [(0x492, 0x253000), (0x49a, 0x272a60), (0x49e, 0x283140)] {
                x.set_v32(off, palettes);
            }
            fading::DELAY.set(x.b, 1);
            fading::PAGES.set(x.b, SCENE_PAGES);
            goto(x, crate::core::tasks::fading::to_palettes, Caller)
        },
        /// state 6: new terrain, dialogue 30
        S13Closing = 0xf919e => |x| yield_frames(x, 1, At(Step::S13ClosingWaited.into())),
        S13ClosingWaited = 0xf91a6 => |x| hide_layers(x, At(Step::S13ClosingHidden.into())),
        S13ClosingHidden = 0xf91aa => |x| {
            set_camera(x, a5::CAM1_X, 8);
            for off in [a5::CAM1_Y, a5::CAM2_X, a5::CAM2_Y] {
                set_camera(x, off, 0);
            }
            clear_layer(x.b, Layer::Three);
            crate::services::palette::load_palette_page(x.b, 2, 87);
            crate::services::palette::load_palette_page(x.b, 3, 87);
            let end = tiles(x, TB, 0x216, SCROLL_2);
            tiles(x, TB, 0x217, end);
            yield_frames(x, 4, At(Step::S13ClosingDrawn.into()))
        },
        S13ClosingDrawn = 0xf9210 => |x| show_layers(x, At(Step::S13ClosingShown.into())),
        S13ClosingShown = 0xf9214 => |x| fade(x, SCENE_PAGES, crate::core::tasks::fading::rise, At(Step::S13ClosingIn.into())),
        S13ClosingIn = 0xf921a => s13_message_30,
        S13Retry30 = 0xf922e => s13_message_30,
        S13Faded = 0xf938a => |x| hide_layers(x, At(Step::S13End.into())),
        S13End = 0xf938e => |x| {
            clear_layer(x.b, Layer::Two);
            let scene = VARS_BLOCK.get8(x.b, var::FLAG);
            finish(x, scene)
        },
    }
}
fn s1_message(x: &mut Ctx) -> Flow {
    or_yield!(message(x, 96, Step::S1Retry));
    Flow::Done
}

// ---------------------------------------------------------------------------------------------
// Scene 2: dialogues 9 to 13; the answer to the last one chooses the next scene (table 0xf847c)

pub fn scene_2(x: &mut Ctx) -> Flow {
    match dispatch(x, var::STATE, 0xf82e4) {
        0 => {
            crate::services::sound::play_global(x.b, ext::MUSIC_A);
            if !try_message(x, 9) {
                return Flow::Done;
            }
            next(x);
            x.set_v16(a5::LAYERS, 0x12c0);
            show_and_fade_in(x, At(Step::S2Shown.into()))
        }
        2 => {
            if message_done(x) {
                next(x);
                let _ = crate::actors::families::creators::set_15_pure(x.b, 8);
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
            hide_layers(x, At(Step::S2Hidden.into()))
        }
        8 => {
            if try_message(x, 11) {
                next(x);
            }
            Flow::Done
        }
        10 | 12 => {
            let n = if VARS_BLOCK.get8(x.b, var::STATE) == 10 { 12 } else { 13 };
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
            VARS_BLOCK.set8(x.b, var::MESSAGE, scene);
            sound(x, 0);
            fade(x, ALL_PAGES, crate::core::tasks::fading::descent, At(Step::S2Faded.into()))
        }
        _ => Flow::Done,
    }
}

// ---------------------------------------------------------------------------------------------
// Scenes 5, 14 and 20: dialogues 93 and 94, scrolling text; scene 108 after 5, otherwise 24

pub fn scene_5(x: &mut Ctx) -> Flow {
    match dispatch(x, var::STATE, 0xf848a) {
        0 => {
            next(x);
            crate::services::sound::play_global(x.b, ext::MUSIC_B);
            let end = tiles(x, TB, 0x218, SCROLL_2);
            tiles(x, TB, 0x219, end);
            VARS_BLOCK.set16(x.b, var::TIMER, 60);
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
            fade(x, SCENE_PAGES, crate::core::tasks::fading::descent, At(Step::S5Faded.into()))
        }
        6 => {
            if !message_done(x) {
                return Flow::Done;
            }
            fade(x, SCENE_PAGES, crate::core::tasks::fading::descent, At(Step::S5TextFaded.into()))
        }
        8 => {
            if !any_button(x) && !tick(x) {
                return Flow::Done;
            }
            sound(x, 0);
            fade(x, SCENE_PAGES, crate::core::tasks::fading::descent, At(Step::S5EndFaded.into()))
        }
        _ => Flow::Done,
    }
}
fn s5_message(x: &mut Ctx) -> Flow {
    or_yield!(message(x, 93, Step::S5Retry));
    next(x);
    Flow::Done
}
fn s5_second_message(x: &mut Ctx) -> Flow {
    or_yield!(message(x, 94, Step::S5Retry2));
    next(x);
    Flow::Done
}

// ---------------------------------------------------------------------------------------------
// Scenes 4 and 10: scrolling texts, dialogues 15, 92 and 16, two system objects; then scene 11

pub fn scene_4(x: &mut Ctx) -> Flow {
    match dispatch(x, var::STATE, 0xf876a) {
        0 => {
            next(x);
            crate::services::sound::play_global(x.b, ext::MUSIC_A);
            let end = tiles(x, TB, 0x20c, SCROLL_2);
            tiles(x, TB, 0x20c, end);
            let end = tiles(x, TB2, 0x14e, SCROLL_3);
            tiles(x, TB2, 0x14f, end);
            x.set_v16(a5::LAYERS, 0x138e);
            yield_frames(x, 1, At(Step::S4Shown.into()))
        }
        2 => {
            drift(x);
            if fading::STEP.get(x.b) != 0 {
                return Flow::Done;
            }
            if any_button(x) {
                VARS_BLOCK.set16(x.b, var::TIMER, 0);
            }
            if VARS_BLOCK.get16(x.b, var::TIMER) != 0 {
                return Flow::Done;
            }
            next(x);
            fade_task(x, 4, SCENE_PAGES, crate::screens::scene::fading::DESCENT);
            Flow::Done
        }
        4 => {
            drift(x);
            if fading::STEP.get(x.b) != 0 {
                return Flow::Done;
            }
            x.set_v32(a5::SCROLL_TEXT, 0);
            sprites_next_frame(x, Step::S4TextGone)
        }
        6 => {
            if any_button(x) {
                VARS_BLOCK.set16(x.b, var::TIMER, 1);
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
            fade(x, SCENE_PAGES, crate::core::tasks::fading::descent, At(Step::S4Faded.into()))
        }
        10 => {
            if !message_done(x) {
                return Flow::Done;
            }
            fade(x, SCENE_PAGES, crate::core::tasks::fading::descent, At(Step::S4SecondFaded.into()))
        }
        12 => {
            if !message_done(x) {
                return Flow::Done;
            }
            sound(x, 0);
            fade_task(x, 3, SCENE_PAGES, crate::screens::scene::fading::DESCENT);
            s4_fading_out(x)
        }
        _ => Flow::Done,
    }
}

/// The timer counts down; every four frames, camera 1 advances by one pixel.
fn drift(x: &mut Ctx) {
    let t = VARS_BLOCK.get16(x.b, var::TIMER).wrapping_sub(1);
    VARS_BLOCK.set16(x.b, var::TIMER, t);
    if t & 3 == 0 {
        let cam = x.v16(a5::CAM1_X).wrapping_add(1);
        set_camera(x, a5::CAM1_X, cam);
    }
}

fn s4_message_92(x: &mut Ctx) -> Flow {
    or_yield!(message(x, 92, Step::S4Retry92));
    VARS_BLOCK.set8(x.b, var::SIGNAL, 0);
    s4_wizard(x)
}
/// System object 0, placed (subtype 2, X 144, Y 188).
fn s4_wizard(x: &mut Ctx) -> Flow {
    let Some(o) = system_object(x, 0, Step::S4RetryWizard) else { return Flow::Yield };
    o.set_u8(x.b, obj::INDEX, 2);
    o.set_u16(x.b, Obj::X, 144);
    o.set_u16(x.b, Obj::Y, 0);
    o.set_u16(x.b, Obj::Z, 188);
    next(x);
    Flow::Done
}

fn s4_second_object(x: &mut Ctx) -> Flow {
    if system_object(x, 1, Step::S4RetryObject).is_none() {
        return Flow::Yield;
    }
    world(x);
    sprites_next_frame(x, Step::S4Scenery)
}
/// The world runs until the end of the fade (0xf8c3c), then dialogue 16.
fn s4_fading_in(x: &mut Ctx) -> Flow {
    world(x);
    sprites_next_frame(x, Step::S4FadingIn)
}
fn s4_message_16(x: &mut Ctx) -> Flow {
    or_yield!(message(x, 16, Step::S4Retry16));
    next(x);
    Flow::Done
}
/// 0xf8c94: the world runs until the end of the exit fade.
fn s4_fading_out(x: &mut Ctx) -> Flow {
    world(x);
    sprites_next_frame(x, Step::S4FadingOut)
}

pub const NATIVES: &[Native] = &[];
