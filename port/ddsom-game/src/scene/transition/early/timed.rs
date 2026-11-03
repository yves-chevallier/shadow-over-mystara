//! Transition scripts of scenes 11 and 13: the time challenge (dialogues while time remains, then the route choice).

use super::*;

// ---------------------------------------------------------------------------------------------
// Scene 11 (if time remains): system object 2, dialogues 17 and 18; then scene 13

pub fn scene_11(x: &mut Ctx) -> Flow {
    match dispatch(x, var::STATE, 0xf8ce4) {
        0 => {
            if x.v8(a5::CLOCK) == 0 {
                return finish(x, 13);
            }
            next(x);
            crate::sound::play_global(x.b, ext::MUSIC_C);
            set_camera(x, a5::CAM1_X, 288);
            let end = tiles(x, TB, 0x214, 0x917400);
            tiles(x, TB, 0x215, end);
            let end = tiles(x, TB2, 0x152, SCROLL_3);
            tiles(x, TB2, 0x153, end);
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
            if busy != 0 {
                return Flow::Done;
            }
            s11_message_18(x)
        }
        8 => {
            if !message_done(x) {
                return Flow::Done;
            }
            sound(x, 0);
            fade(x, SCENE_PAGES, crate::tasks::fading::descent, At(Step::S11Faded.into()))
        }
        _ => Flow::Done,
    }
}
pub(super) fn s11_object(x: &mut Ctx) -> Flow {
    if system_object(x, 2, Step::S11RetryObject).is_none() {
        return Flow::Yield;
    }
    set8(x, var::SIGNAL, 0);
    world(x);
    sprites_next_frame(x, Step::S11Drawn)
}
pub(super) fn s11_message_17(x: &mut Ctx) -> Flow {
    or_yield!(message(x, 17, Step::S11Retry17));
    next(x);
    set8(x, var::FLAG, 1);
    Flow::Done
}
pub(super) fn s11_message_18(x: &mut Ctx) -> Flow {
    or_yield!(message(x, 18, Step::S11Retry18));
    next(x);
    Flow::Done
}

// ---------------------------------------------------------------------------------------------
// Scene 13: by remaining time, SET objects 10 and 11 or 9; dialogue 30 then a choice (dialogues 136 to 138)
// that sets the next scene (tables 0xf92e4, 0xf932e, 0xf937a; 14 if no player is in the expected state)

pub fn scene_13(x: &mut Ctx) -> Flow {
    match dispatch(x, var::STATE, 0xf8e9e) {
        0 => {
            next(x);
            crate::sound::play_global(x.b, ext::MUSIC_D);
            let end = tiles(x, TB2, 0x154, SCROLL_3);
            tiles(x, TB2, 0x155, end);
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
            sprites_next_frame(x, Step::S13Closing)
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
        12 => fade(x, SCENE_PAGES, crate::tasks::fading::descent, At(Step::S13Faded.into())),
        _ => Flow::Done,
    }
}
pub(super) fn s13_object(x: &mut Ctx) -> Flow {
    if system_object(x, 3, Step::S13RetryObject).is_none() {
        return Flow::Yield;
    }
    set8(x, var::SIGNAL, 0);
    world(x);
    sprites_next_frame(x, Step::S13Drawn)
}

/// Time remaining: SET object 10, fade, new terrain and SET object 11; the object signals its end (`+8`).
pub(super) fn s13_time_left(x: &mut Ctx) -> Flow {
    match dispatch(x, var::SUB, 0xf8f4e) {
        0 => {
            if try_set_object(x, 10).is_some() {
                next_sub(x);
                fading::STEP.set(x.b, 1);
                fade_task(x, 4, SCENE_PAGES, crate::scene::fading::DESCENT.0);
            }
            Flow::Done
        }
        2 => {
            if fading::STEP.get(x.b) != 0 {
                return Flow::Done;
            }
            set8(x, var::SIGNAL, 0xff);
            world(x);
            sprites_next_frame(x, Step::S13LeftDrawn)
        }
        4 => {
            if fading::STEP.get(x.b) == 0 {
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
/// End of the interlude when the object has signalled (`+8` negative): fade to black.
pub(super) fn s13_leave(x: &mut Ctx) -> Flow {
    if (get8(x, var::SIGNAL) as i8) >= 0 {
        return Flow::Done;
    }
    next(x);
    set8(x, var::SUB, 0);
    fade(x, SCENE_PAGES, crate::tasks::fading::descent, Caller)
}
pub(super) fn s13_set_11(x: &mut Ctx) -> Flow {
    or_yield!(set_object(x, 11, Step::S13Retry11));
    next_sub(x);
    world(x);
    sprites_next_frame(x, Step::S13Set11Drawn)
}

/// Time elapsed: per-component fade, other terrain, SET object 9, then fade to fixed palettes.
pub(super) fn s13_time_out(x: &mut Ctx) -> Flow {
    match dispatch(x, var::SUB, 0xf90a6) {
        0 => {
            sound(x, 0);
            next_sub(x);
            fading::DELAY.set(x.b, 1);
            fading::PAGES.set(x.b, SCENE_PAGES);
            goto(x, crate::tasks::fading::to_white, At(Step::S13OutFaded.into()))
        }
        2 => s13_out_scenery(x),
        4 => {
            if get8(x, var::SIGNAL) == 0 {
                return Flow::Done;
            }
            set8(x, var::SIGNAL, 0);
            world(x);
            sprites_next_frame(x, Step::S13OutDrawn)
        }
        6 => s13_leave(x),
        _ => Flow::Done,
    }
}
pub(super) fn s13_out_scenery(x: &mut Ctx) -> Flow {
    next_sub(x);
    let end = tiles(x, TB, 0x22a, SCROLL_2);
    tiles(x, TB, 0x22b, end);
    let end = tiles(x, TB2, 0x164, SCROLL_3);
    tiles(x, TB2, 0x165, end);
    set8(x, var::SIGNAL, 0xff);
    world(x);
    sprites_next_frame(x, Step::S13OutSceneryDrawn)
}
pub(super) fn s13_set_9(x: &mut Ctx) -> Flow {
    or_yield!(set_object(x, 9, Step::S13Retry9));
    Flow::Done
}

pub(super) fn s13_message_30(x: &mut Ctx) -> Flow {
    or_yield!(message(x, 30, Step::S13Retry30));
    next(x);
    Flow::Done
}

/// State 8: the question to ask (`+6`) depends on the players: 0 if a player in the game (state `+4` < 4) is in
/// posture 4 or 5 (`+3`), 4 otherwise; plus 2 if time remains.
pub(super) fn s13_route(x: &mut Ctx) {
    let mut route = 0u8;
    let mut found = false;
    for k in 0..4u32 {
        let p = A5 + a5::PLAYERS + k * 0x200;
        if rd8(x.b, p) != 0 && (rd8(x.b, p + 4) as i8) < 4 {
            let pose = (rd8(x.b, p + 3) >> 1).wrapping_sub(2);
            if pose == 0 {
                found = true;
                break;
            }
        }
    }
    if !found {
        route += 4;
    }
    if x.v16(a5::CLOCK) != 0 {
        route += 2;
    }
    set8(x, var::FLAG, route);
}

/// Question `n`, then the next scene according to the answer (table `answers`).
pub(super) fn choice(x: &mut Ctx, n: u16, table: u32, answers: u32) -> Flow {
    match dispatch(x, var::SUB, table) {
        0 => {
            if let Some(o) = crate::families::creators::scene_set(x.b, n as u8) {
                next_sub(x);
                store_message(x, o);
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
