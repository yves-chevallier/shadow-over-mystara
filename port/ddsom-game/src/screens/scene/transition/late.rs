//! Transition scripts of the following levels: scenes 27, 31, 33, 32 (34, 35, 67), 38 (40, 42, 44), 46, and the end
//! of the game (69, taken over by 105).

use super::*;
use crate::world::screen_tables::Rom;
use crate::screens::scene::fading;
use crate::screens::scene::tiles;
use crate::world::objects::Obj;
use crate::screens::obj;
pub use crate::world::tasks::stages::TransitionLate as Step;

mod ext {
    pub const MUSIC_B: crate::services::sound::Sound = crate::services::sound::CMD_16;
    pub const MUSIC_F: crate::services::sound::Sound = crate::services::sound::CMD_2E;
    pub const MUSIC_G: crate::services::sound::Sound = crate::services::sound::CMD_31;
    pub const MUSIC_H: crate::services::sound::Sound = crate::services::sound::CMD_1FC;
    pub const MUSIC_FINALE: crate::services::sound::Sound = crate::services::sound::CMD_31A;
}

/// Palette page 0x91c400 and its backup.
const PALETTE_PAGE: u32 = crate::hw::video::palette(1, 0).0;
const PALETTE_SAVE: u32 = 0xff1190;
/// Next scene by the choice of scene 27, and the remaining-time prizes of scene 31 (words then longs, PC-relative).
const NEXT_SCENES: Rom = Rom::at(0xf9454);
const TIME_PRIZES: Rom = Rom::at(0xf95ba);

// ---------------------------------------------------------------------------------------------
// Scene 27: the next scene depends on the choice kept in `+4` (table 0xf9454)

pub fn scene_27(x: &mut Ctx) -> Flow {
    let choice = VARS_BLOCK.get8(x.b, var::MESSAGE);
    if choice == 0 {
        x.set_v8(0x4e, 0);
    }
    let scene = NEXT_SCENES.pc_byte(x.b, choice as u32);
    finish(x, scene)
}

// ---------------------------------------------------------------------------------------------
// Scene 31: dialogue 141, remaining-time bonus paid to each player, then scene 31

/// Bonus to pay (BCD long, six digits) and displayed duration.
const PRIZE: u32 = 18;
const SHOWN_TIME: u32 = 16;

pub fn scene_31(x: &mut Ctx) -> Flow {
    match dispatch(x, var::STATE, 0xf9522) {
        0 => show_and_fade_in(x, At(Step::S31Shown.into())),
        2 => {
            if !tick(x) {
                return Flow::Done;
            }
            x.set_v8(0x33, 1);
            VARS_BLOCK.set16(x.b, var::TIMER, 120);
            next(x);
            crate::services::text::print_tall(x.b, 0xf985e);
            // bonus and duration from the remaining time (8-byte table per ten seconds)
            let clock = x.v16(a5::CLOCK);
            let at = (clock >> 8 & 15) * 48 + (clock & 0xf0) / 2;
            let time = TIME_PRIZES.pc_word(x.b, at as u32);
            VARS_BLOCK.set16(x.b, SHOWN_TIME, time);
            let prize = TIME_PRIZES.pc_long(x.b, at as u32 + 2);
            Obj(VARS).set_u32(x.b, PRIZE, prize);
            duration(x, crate::hw::video::TEXT.at(0x958).0, time);
            tiles::number6(x.b, PRIZE_AT, VARS + PRIZE, 2);
            Flow::Done
        }
        4 => {
            if VARS_BLOCK.get16(x.b, var::TIMER) != 0 && !tick(x) {
                return Flow::Done;
            }
            VARS_BLOCK.set16(x.b, var::TIMER, 2);
            if pay_players(x) {
                return Flow::Done;
            }
            next(x);
            VARS_BLOCK.set16(x.b, var::TIMER, 30);
            Flow::Done
        }
        6 => {
            if tick(x) {
                next(x);
                let o = message_obj(x);
                o.set_u16(x.b, MESSAGE_TIME, 1);
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
const PRIZE_AT: u32 = crate::hw::video::TEXT.at(0xc58).0;
crate::steps! {
    impl Step {
        S31Shown = 0xf9530 => |x| {
            next(x);
            s31_message(x)
        },
        S31Retry = 0xf9548 => s31_message,
        S33RetryObject = 0xf98d2 => s33_object,
        S33Drawn = 0xf98f8 => |x| show_and_fade_in(x, At(Step::S33Shown.into())),
        S33Shown = 0xf98fc => s33_message_39,
        S33Retry39 = 0xf9910 => s33_message_39,
        S33Faded = 0xf992a => |x| {
            VARS_BLOCK.set8(x.b, var::SIGNAL, 0xff);
            world(x);
            sprites_next_frame(x, Step::S33Clear)
        },
        S33Clear = 0xf9938 => |x| {
            crate::services::video::clear_objects(x.b);
            hide_layers(x, At(Step::S33Hidden.into()))
        },
        S33Hidden = 0xf9942 => |x| {
            clear_layer(x.b, Layer::Three);
            dark_palette(x, 88);
            // 0x1a366 (A0-A6 kept): palettes of scene 33
            crate::services::palette::load_scene_palettes_of(x.b, 33);
            yield_frames(x, 4, At(Step::S33Dark.into()))
        },
        S33Dark = 0xf996a => |x| {
            tiles(x, TB, 0x20f, SCROLL_2);
            show_layers(x, At(Step::S33Drawn2.into()))
        },
        S33Drawn2 = 0xf9980 => |x| fade(x, SCENE_PAGES, crate::services::fading::rise, At(Step::S33Shown2.into())),
        S33Shown2 = 0xf9984 => |x| {
            if try_message(x, 40) {
                next(x);
                return Flow::Done;
            }
            yield_frames(x, 1, At(Step::S33Retry40.into()))
        },
        S33Retry40 = 0xf9998 => s33_object,
        S33EndFaded = 0xf99cc => |x| hide_layers(x, At(Step::S33EndHidden.into())),
        S33EndHidden = 0xf99d0 => |x| {
            clear_layer(x.b, Layer::Two);
            show_layers(x, At(Step::S33End.into()))
        },
        S33End = 0xf99da => |x| finish(x, 34),
        S32Shown = 0xf9a4a => |x| {
            crate::actors::families::small::background_1_on(x.b);
            next(x);
            fade_task(x, 3, ALL_PAGES, crate::screens::scene::fading::RISE);
            Flow::Done
        },
        S32Retry59 = 0xf9a7e => |x| s32_message_59(x, 1),
        S32Hidden = 0xf9acc => |x| {
            clear_layer(x.b, Layer::Two);
            clear_layer(x.b, Layer::Three);
            s32_message_60(x)
        },
        S32Retry60 = 0xf9af0 => s32_message_60,
        S32SecondShown = 0xf9b40 => |x| {
            next(x);
            fade(x, SCENE_PAGES, crate::services::fading::rise, At(Step::S32SecondIn.into()))
        },
        S32SecondIn = 0xf9b46 => s32_message_61,
        S32Retry61 = 0xf9b5a => s32_message_61,
        S32Coloured = 0xf9b8c => s32_message_62,
        S32Retry62 = 0xf9ba0 => s32_message_62,
        S32Faded = 0xf9bba => |x| hide_layers(x, At(Step::S32End.into())),
        S32End = 0xf9bbe => |x| {
            clear_layer(x.b, Layer::Two);
            clear_layer(x.b, Layer::Three);
            end(x)
        },
        S38Drawn = 0xf9c54 => |x| {
            VARS_BLOCK.set16(x.b, var::TIMER, 300);
            show_and_fade_in(x, Caller)
        },
        S38Closed = 0xf9caa => s38_message_73,
        S38Retry73 = 0xf9cbe => s38_message_73,
        S38Gone = 0xf9cfc => |x| {
            VARS_BLOCK.set8(x.b, var::SIGNAL, 0);
            crate::services::video::clear_objects(x.b);
            hide_layers(x, At(Step::S38Hidden.into()))
        },
        S38Hidden = 0xf9d0a => |x| home_cameras(x, At(Step::S38Homed.into())),
        S38Homed = 0xf9d0e => |x| {
            clear_layer(x.b, Layer::Three);
            clear_layer(x.b, Layer::Two);
            yield_frames(x, 4, At(Step::S38Scenery.into()))
        },
        S38Scenery = 0xf9d22 => |x| {
            // terrain 540, or 551 if a player in the game is in posture 11 (the first in posture 10 stops the search)
            let mut scenery = 540;
            for k in 0..4u32 {
                let p = A5 + a5::PLAYERS + k * 0x200;
                if Obj(p).u8(x.b, Obj::ACTIVE) == 0 || (Obj(p).u8(x.b, Obj::STATE) as i8) >= 4 {
                    continue;
                }
                match Obj(p).u8(x.b, Obj::VARIANT) {
                    10 => break,
                    11 => scenery = 551,
                    _ => {}
                }
            }
            tiles(x, TB, scenery, SCROLL_2);
            show_layers(x, At(Step::S38SceneryShown.into()))
        },
        S38SceneryShown = 0xf9d70 => |x| fade(x, SCENE_PAGES, crate::services::fading::rise, At(Step::S38SceneryIn.into())),
        S38SceneryIn = 0xf9d74 => s38_message_98,
        S38Retry98 = 0xf9d88 => s38_message_98,
        S38LastFaded = 0xf9da2 => |x| hide_layers(x, At(Step::S38LastHidden.into())),
        S38LastHidden = 0xf9da6 => |x| yield_frames(x, 4, At(Step::S38LastScenery.into())),
        S38LastScenery = 0xf9dae => |x| {
            let last_tile = tiles(x, TB, 0x21d, SCROLL_2);
            tiles(x, TB, 0x21e, last_tile);
            show_layers(x, At(Step::S38LastShown.into()))
        },
        S38LastShown = 0xf9dd0 => |x| fade(x, SCENE_PAGES, crate::services::fading::rise, At(Step::S38LastIn.into())),
        S38LastIn = 0xf9dd4 => s38_message_74,
        S38Retry74 = 0xf9de8 => s38_message_74,
        S38End = 0xf9e26 => |x| {
            sound(x, 0);
            finish(x, 47)
        },
        S46Homed = 0xf9e7c => |x| {
            x.set_v8(0x3c, 0xff);
            let last_tile = tiles(x, TB2, 385, crate::hw::video::SCROLL3.offset(0x3000).0);
            tiles(x, TB2, 386, last_tile);
            let last_tile = tiles(x, TB2, 387, SCROLL_3);
            tiles(x, TB2, 388, last_tile);
            VARS_BLOCK.set8(x.b, var::SIGNAL, 0);
            if try_set_object(x, 12).is_none() {
                return Flow::Done;
            }
            next(x);
            VARS_BLOCK.set8(x.b, var::SUB, 0);
            let y = x.v16(a5::CAM2_Y);
            Obj(VARS).set_u32(x.b, var::MESSAGE, (y as u32) << 16);
            try_set_object(x, 21);
            world(x);
            sprites_next_frame(x, Step::S46Drawn)
        },
        S46Drawn = 0xf9ef2 => |x| {
            crate::services::sound::play_global(x.b, ext::MUSIC_FINALE);
            VARS_BLOCK.set16(x.b, var::TIMER, 300);
            layers_or(x, 0xe);
            yield_frames(x, 1, At(Step::S46Shown.into()))
        },
        S46Shown = 0xf9f0c => |x| {
            fade_task(x, 3, ALL_PAGES, crate::screens::scene::fading::RISE);
            Flow::Done
        },
        S46Gone = 0xf9f96 => |x| {
            crate::services::video::clear_objects(x.b);
            hide_layers(x, At(Step::S46Hidden.into()))
        },
        S46Hidden = 0xf9fa0 => |x| {
            clear_layer(x.b, Layer::Two);
            clear_layer(x.b, Layer::Three);
            home_cameras(x, At(Step::S46End.into()))
        },
        S46End = 0xf9fb0 => |x| {
            VARS_BLOCK.set8(x.b, var::SIGNAL, 0);
            finish(x, 46)
        },
        FinaleFaded = 0xfa058 => |x| {
            fading::PAGES.set(x.b, ALL_PAGES);
            fading::DELAY.set(x.b, 1);
            goto(x, crate::services::fading::descent, At(Step::FinaleBlack.into()))
        },
        FinaleBlack = 0xfa06a => |x| {
            x.set_v16(a5::LAYERS, 0x12c0);
            yield_frames(x, 1, At(Step::FinalePalettes.into()))
        },
        FinalePalettes = 0xfa078 => |x| {
            crate::services::palette::load_scene_palettes(x.b);
            copy_palettes(x, PALETTE_SAVE, PALETTE_PAGE);
            crate::services::palette::load_scene_page(x.b, 2);
            crate::services::palette::load_scene_page(x.b, 3);
            yield_frames(x, 4, At(Step::FinaleEnd.into()))
        },
        FinaleEnd = 0xfa0a2 => |x| {
            fading::PAGES.set(x.b, ALL_PAGES);
            crate::services::palette::full_brightness(x.b);
            x.set_v8(0x42, 1);
            let step = EXIT_STEP.get(x.b).wrapping_add(4);
            EXIT_STEP.set(x.b, step);
            for (off, v) in [(a5::CAM1_X, 96), (a5::CAM1_Y, 296), (a5::CAM2_X, 64), (a5::CAM2_Y, 1552)] {
                x.set_v16(off, v);
            }
            x.set_v16(a5::LAYERS, 0x12ce);
            VARS_BLOCK.set8(x.b, var::STATE, 0);
            EXIT_SUB_STEP.set(x.b, 0);
            x.set_v16(0x56e, 0);
            Flow::Done
        },
    }
}
fn s31_message(x: &mut Ctx) -> Flow {
    or_yield!(message(x, 141, Step::S31Retry));
    VARS_BLOCK.set16(x.b, var::TIMER, 60);
    Flow::Done
}

/// `m:ss` duration in two-cell digits (attributes 2), tens of minutes omitted if zero (0xf96b2).
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

/// One slice of the bonus to each player in the game (state `+4` < 4) while any remains; true if a slice was
/// paid. The bonus is shared: each player receives a slice on the same frame.
fn pay_players(x: &mut Ctx) -> bool {
    let mut paid = false;
    for k in 0..4u32 {
        let p = A5 + a5::PLAYERS + k * 0x200;
        if Obj(p).u8(x.b, Obj::ACTIVE) == 0 || (Obj(p).u8(x.b, Obj::STATE) as i8) >= 4 {
            continue;
        }
        let left = Obj(VARS).u32(x.b, PRIZE);
        if left == 0 {
            continue;
        }
        paid = true;
        // the smallest non-empty slice: hundreds (4), thousands (14), tens of thousands (33)
        let (left, amount) = if left & 0xf00 != 0 {
            (left - 0x100, 4)
        } else if left & 0xf000 != 0 {
            (left - 0x1000, 14)
        } else {
            (left.wrapping_sub(0x10000), 33)
        };
        Obj(VARS).set_u32(x.b, PRIZE, left);
        let player = (Obj(p).u8(x.b, Obj::KIND) as u32) << 8;
        crate::world::events::enqueue_points(x.b, (amount | player) as u16);
        tiles::number6(x.b, PRIZE_AT, VARS + PRIZE, 2);
    }
    paid
}

// ---------------------------------------------------------------------------------------------
// Scene 33: system object 0, dialogues 39 and 40; the answer sets `A5+0x59` (table 0xf99e2); then scene 34

pub fn scene_33(x: &mut Ctx) -> Flow {
    match dispatch(x, var::STATE, 0xf9884) {
        0 => {
            tiles(x, TB, 0x207, SCROLL_2);
            let last_tile = tiles(x, TB2, 0x158, SCROLL_3);
            tiles(x, TB2, 0x159, last_tile);
            VARS_BLOCK.set8(x.b, var::SIGNAL, 0);
            s33_object(x)
        }
        2 => {
            if !message_done(x) {
                return Flow::Done;
            }
            fade(x, SCENE_PAGES, crate::services::fading::descent, At(Step::S33Faded.into()))
        }
        4 => {
            if !message_done(x) {
                return Flow::Done;
            }
            let v = answer(x, 0xf99e2);
            x.set_v8(0x59, v);
            RECORD_56.set(x.b, v);
            fade(x, ALL_PAGES, crate::services::fading::descent, At(Step::S33EndFaded.into()))
        }
        _ => Flow::Done,
    }
}
/// System object 0 (subtype 3, X 96, Y 176), then dialogue 39 (0xf98be). The resume of dialogue 40 comes back here
/// too in the original (`bra 0xf98be`).
fn s33_object(x: &mut Ctx) -> Flow {
    let Some(o) = system_object(x, 0, Step::S33RetryObject) else { return Flow::Yield };
    o.set_u8(x.b, obj::INDEX, 3);
    o.set_u16(x.b, Obj::X, 96);
    o.set_u16(x.b, Obj::Y, 0);
    o.set_u16(x.b, Obj::Z, 176);
    world(x);
    sprites_next_frame(x, Step::S33Drawn)
}
fn s33_message_39(x: &mut Ctx) -> Flow {
    or_yield!(message(x, 39, Step::S33Retry39));
    next(x);
    Flow::Done
}

// ---------------------------------------------------------------------------------------------
// Scenes 32, 34, 35, 67: dialogues 59 to 62, fade to a colour; the scene exit continues

pub fn scene_32(x: &mut Ctx) -> Flow {
    match dispatch(x, var::STATE, 0xf99f0) {
        0 => {
            x.set_v16(a5::LAYERS, 0x138e);
            crate::services::sound::play_global(x.b, ext::MUSIC_B);
            let last_tile = tiles(x, TB, 0x228, SCROLL_2);
            tiles(x, TB, 0x229, last_tile);
            let last_tile = tiles(x, TB2, 0x15a, SCROLL_3);
            tiles(x, TB2, 0x15b, last_tile);
            show_layers(x, At(Step::S32Shown.into()))
        }
        2 => {
            if fading::STEP.get(x.b) != 0 {
                return Flow::Done;
            }
            s32_message_59(x, 59)
        }
        4 => {
            if message_done(x) {
                sound(x, 0);
                next(x);
                fade_task(x, 3, SCENE_PAGES, crate::screens::scene::fading::DESCENT);
            }
            Flow::Done
        }
        6 => {
            if fading::STEP.get(x.b) != 0 {
                return Flow::Done;
            }
            x.set_v16(0x51e, 4);
            let f = x.v8(0x12) & !1;
            x.set_v8(0x12, f);
            clear_920000(x);
            x.set_v16(a5::LAYERS, 0x12ce);
            hide_layers(x, At(Step::S32Hidden.into()))
        }
        8 => {
            if !message_done(x) {
                return Flow::Done;
            }
            crate::services::sound::play_global(x.b, ext::MUSIC_F);
            tiles(x, TB, 0x226, SCROLL_2);
            let last_tile = tiles(x, TB2, 8, SCROLL_3);
            tiles(x, TB2, 8, last_tile);
            crate::hw::video::palette(3, 0).set(x.b, 0);
            show_layers(x, At(Step::S32SecondShown.into()))
        }
        10 => {
            if !message_done(x) {
                return Flow::Done;
            }
            fading::DELAY.set(x.b, 1);
            fading::PAGES.set(x.b, 12);
            x.set_v16(0x49a, 15);
            x.set_v16(0x49e, 15);
            goto(x, crate::services::fading::to_colours, At(Step::S32Coloured.into()))
        }
        12 => {
            if !message_done(x) {
                return Flow::Done;
            }
            fade(x, ALL_PAGES, crate::services::fading::descent, At(Step::S32Faded.into()))
        }
        _ => Flow::Done,
    }
}
/// Message `n` (59; 1 on resume, see [`Step::S32Retry59`]).
fn s32_message_59(x: &mut Ctx, n: u8) -> Flow {
    or_yield!(message(x, n, Step::S32Retry59));
    next(x);
    Flow::Done
}
fn s32_message_60(x: &mut Ctx) -> Flow {
    or_yield!(message(x, 60, Step::S32Retry60));
    next(x);
    Flow::Done
}
fn s32_message_61(x: &mut Ctx) -> Flow {
    or_yield!(message(x, 61, Step::S32Retry61));
    Flow::Done
}
fn s32_message_62(x: &mut Ctx) -> Flow {
    or_yield!(message(x, 62, Step::S32Retry62));
    next(x);
    Flow::Done
}

// ---------------------------------------------------------------------------------------------
// Scenes 38, 40, 42, 44: system objects 4 and 5, dialogues 97, 73, 98, 74; terrain depending on the characters;
// then scene 47

pub fn scene_38(x: &mut Ctx) -> Flow {
    match dispatch(x, var::STATE, 0xf9bee) {
        0 => {
            next(x);
            crate::services::sound::play_global(x.b, ext::MUSIC_G);
            set_camera(x, a5::CAM2_Y, 8);
            let last_tile = tiles(x, TB2, 0x15c, SCROLL_3);
            tiles(x, TB2, 0x15d, last_tile);
            VARS_BLOCK.set8(x.b, var::SIGNAL, 0);
            for n in [4, 5] {
                system_object_untested(x, n);
            }
            world(x);
            sprites_next_frame(x, Step::S38Drawn)
        }
        2 => {
            if VARS_BLOCK.get8(x.b, var::SIGNAL) != 0 {
                VARS_BLOCK.set8(x.b, var::SIGNAL, 0);
                let m = message_untested(x, 97);
                store_message(x, m);
                next(x);
                VARS_BLOCK.set16(x.b, var::TIMER, 180);
            }
            Flow::Done
        }
        4 => {
            if any_button(x) {
                VARS_BLOCK.set16(x.b, var::TIMER, 1);
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
                fade_task(x, 4, SCENE_PAGES, crate::screens::scene::fading::DESCENT);
            }
            Flow::Done
        }
        10 => {
            if fading::STEP.get(x.b) != 0 {
                return Flow::Done;
            }
            VARS_BLOCK.set8(x.b, var::SIGNAL, 0xff);
            world(x);
            sprites_next_frame(x, Step::S38Gone)
        }
        12 => {
            if !message_done(x) {
                return Flow::Done;
            }
            fade(x, SCENE_PAGES, crate::services::fading::descent, At(Step::S38LastFaded.into()))
        }
        14 => {
            if !message_done(x) {
                return Flow::Done;
            }
            copy_palettes(x, PALETTE_PAGE, PALETTE_SAVE);
            crate::services::sound::play_global(x.b, ext::MUSIC_H);
            fading::DELAY.set(x.b, 1);
            fading::PAGES.set(x.b, ALL_PAGES);
            goto(x, crate::services::fading::to_white, At(Step::S38End.into()))
        }
        _ => Flow::Done,
    }
}
/// Closes dialogue 97 (word `+0x32`), then dialogue 73.
fn s38_close_message(x: &mut Ctx) -> Flow {
    let o = message_obj(x);
    o.set_u16(x.b, MESSAGE_TIME, 1);
    world(x);
    sprites_next_frame(x, Step::S38Closed)
}
fn s38_message_73(x: &mut Ctx) -> Flow {
    or_yield!(message(x, 73, Step::S38Retry73));
    next(x);
    Flow::Done
}
fn s38_message_98(x: &mut Ctx) -> Flow {
    or_yield!(message(x, 98, Step::S38Retry98));
    next(x);
    Flow::Done
}
fn s38_message_74(x: &mut Ctx) -> Flow {
    or_yield!(message(x, 74, Step::S38Retry74));
    next(x);
    Flow::Done
}

// ---------------------------------------------------------------------------------------------
// Scene 46: SET objects 12 and 21, dialogue 69, camera 2 goes down; then scene 46

pub fn scene_46(x: &mut Ctx) -> Flow {
    match dispatch(x, var::STATE, 0xf9e58) {
        0 => {
            // palettes of scene 7
            x.set_v8(a5::SCENE, 7);
            crate::services::palette::load_scene_palettes(x.b);
            x.set_v8(a5::SCENE, 46);
            home_cameras(x, At(Step::S46Homed.into()))
        }
        2 => {
            if VARS_BLOCK.get8(x.b, var::SUB) == 0 && fading::STEP.get(x.b) == 0 {
                message_untested(x, 69);
                next_sub(x);
            }
            descend(x);
            if !tick(x) {
                return Flow::Done;
            }
            next(x);
            fading::STEP.set(x.b, 1);
            fade_task(x, 4, ALL_PAGES, crate::screens::scene::fading::DESCENT);
            Flow::Done
        }
        4 => {
            descend(x);
            if fading::STEP.get(x.b) != 0 {
                return Flow::Done;
            }
            VARS_BLOCK.set8(x.b, var::SIGNAL, 0xff);
            world(x);
            sprites_next_frame(x, Step::S46Gone)
        }
        _ => Flow::Done,
    }
}
/// Camera 2 goes down by half a pixel per frame (fixed-point position in `+4`).
fn descend(x: &mut Ctx) {
    let y = Obj(VARS).u32(x.b, var::MESSAGE).wrapping_add(0x8000);
    Obj(VARS).set_u32(x.b, var::MESSAGE, y);
    set_camera(x, a5::CAM2_Y, (y >> 16) as u16);
}

// ---------------------------------------------------------------------------------------------
// End of the game (scene 69, taken over by 105): flashing white screen, fades, final terrain; the scene exit continues

pub fn finale(x: &mut Ctx) -> Flow {
    match dispatch(x, var::STATE, 0xf9fc8) {
        0 => {
            next(x);
            fading::PAGES.set(x.b, 12);
            crate::services::palette::full_brightness(x.b);
            crate::services::sound::play_global(x.b, ext::MUSIC_FINALE);
            let last_tile = tiles(x, TB, 544, crate::hw::video::SCROLL2.offset(0x1000).0);
            tiles(x, TB, 545, last_tile);
            let last_tile = tiles(x, TB2, 389, crate::hw::video::SCROLL3.offset(0x1000).0);
            tiles(x, TB2, 390, last_tile);
            x.set_v16(a5::LAYERS, 0x12cc);
            VARS_BLOCK.set16(x.b, var::MESSAGE, 180);
            Flow::Done
        }
        _ => {
            // plane 1 blinks for 180 frames
            let layers = x.v8(a5::LAYERS + 1) ^ 4;
            x.set_v8(a5::LAYERS + 1, layers);
            let t = VARS_BLOCK.get16(x.b, var::MESSAGE).wrapping_sub(1);
            VARS_BLOCK.set16(x.b, var::MESSAGE, t);
            if t != 0 {
                return Flow::Done;
            }
            x.set_v8(a5::LAYERS + 1, layers | 4);
            copy_palettes(x, PALETTE_PAGE, PALETTE_SAVE);
            fading::PAGES.set(x.b, 12);
            fading::DELAY.set(x.b, 1);
            goto(x, crate::services::fading::to_white, At(Step::FinaleFaded.into()))
        }
    }
}

