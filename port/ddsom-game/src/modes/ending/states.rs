//! States of the ending sequence (`FF0E72`, step of 2): pictures, dialogues, credits, rewards.

use super::*;

// ---------------------------------------------------------------------------------------------
// States

/// State 0: clears everything, copies the players' rewards, then falls into state 2.
pub(super) fn s00_reset(x: &mut Ctx) -> Flow {
    next_step(x);
    crate::kernel::kill_task(x.b, 0x30);
    reset_pools(x);
    crate::video::black_screen(x.b);
    unlink_and_clear_lists(x);
    crate::video::clear_objects(x.b);
    for k in 0..4u32 {
        let p = var::PLAYERS + 0x200 * k;
        if rd8(x.b, p) != 0
            && (rd8(x.b, p + 4) as i8) < 4
            && rd32(x.b, p + 0x118) == 0xffff_ffff
            && rd32(x.b, p + 0x11c) == 0xffff_ffff
        {
            reward_table(x, p);
        }
    }
    x.wait(1, Step::S00Video)
}

/// `0xfb798`: two longs of the endings table (per character and region) into the player.
pub(super) fn reward_table(x: &mut Ctx, p: u32) {
    let mut offset = (rd8(x.b, p + 3) as u32) << 3;
    if rd16(x.b, A5 + 0x70) != 0 {
        offset += 96;
    }
    for i in 0..2 {
        let v = rd32(x.b, 0xfb7bc + offset + 4 * i);
        wr32(x.b, p + 0x118 + 4 * i, v);
    }
}
pub(super) const S00_VIDEO: u32 = 0xfb712;
pub(super) const S00_PALETTE: u32 = 0xfb758;

/// State 2 (where state 0 falls): the screen is cleared.
pub(super) fn s02_clear(x: &mut Ctx) -> Flow {
    next_step(x);
    wr16(x.b, var::VIDEO, 0x12c0);
    x.wait(1, Step::S02Cleared)
}
pub(super) const S02_CLEARED: u32 = 0xfb78c;

/// State 4: background picture, opening fade.
pub(super) fn s04_picture(x: &mut Ctx) -> Flow {
    next_step(x);
    tile_blocks(x, 0x917000, 0x222, 0x223, Grid::Layer2);
    wr16(x.b, var::VIDEO, 0x12ce);
    x.wait(1, Step::S04Fade)
}
pub(super) const S04_FADE: u32 = 0xfb8a8;
pub(super) const S04_TIMER: u32 = 0xfb8c0;

/// State 6: after the timer, creates the dialogue object (SET 70).
pub(super) fn s06_wait(x: &mut Ctx) -> Flow {
    if !timer_tick(x) {
        return Flow::Done;
    }
    next_step(x);
    s06_object(x)
}
pub(super) fn s06_object(x: &mut Ctx) -> Flow {
    let Some(Obj(o)) = pools::SET.alloc(x.b) else { return x.wait(1, Step::S06Retry) };
    wr8(x.b, o, 1);
    wr8(x.b, o + 2, 0);
    wr8(x.b, o + 3, 70);
    wr8(x.b, o + 0x2f, 0);
    wr16(x.b, BLOCK + f::LINK, o as u16);
    Flow::Done
}
pub(super) const S06_RETRY: u32 = 0xfb8e0;

/// State 8: the dialogue object reaches state 6: fade to black then new terrain.
pub(super) fn s08_wait_dialogue(x: &mut Ctx) -> Flow {
    render(x);
    let o = link(x);
    if rd8(x.b, o + 4) != 6 {
        return Flow::Done;
    }
    x.wait(60, Step::S08Fade)
}
pub(super) const S08_FADE: u32 = 0xfb916;
pub(super) const S08_FADED: u32 = 0xfb92c;
pub(super) const S08_PICTURE: u32 = 0xfb93a;
pub(super) fn s08_object(x: &mut Ctx) -> Flow {
    let Ok(Obj(o)) = scenery_6(x.b, 10) else {
        return x.wait(1, Step::S08Retry);
    };
    wr32(x.b, o + 0x4c, BLOCK);
    render(x);
    let v = rd16(x.b, var::VIDEO) | 0xe;
    wr16(x.b, var::VIDEO, v);
    fade_settings(x, 3, FADE_ALL);
    crate::tasks::fading::rise(x)
}
/// Retry of the object creation: `bra 0xfb982` (the signal reset is not redone).
pub(super) const S08_RETRY: u32 = 0xfb996;

/// State 10: waits for the object's signal, then opens message 89.
pub(super) fn s0a_wait_signal(x: &mut Ctx) -> Flow {
    render(x);
    if rd8(x.b, BLOCK + f::SIGNAL) == 0 {
        return Flow::Done;
    }
    next_step(x);
    open_message(x, 89, Step::S0aRetry)
}
/// Message `n` (0xc8312): the dialogue object is stored in the block; `retry`: yield on failure.
pub(super) fn open_message(x: &mut Ctx, n: u8, retry: Step) -> Flow {
    let Some(message) = crate::families::creators::scene_set(x.b, n) else {
        return x.wait(1, retry);
    };
    wr16(x.b, BLOCK + f::LINK, message.0 as u16);
    Flow::Done
}
pub(super) const S0A_RETRY: u32 = 0xfb9dc;

/// State 12: the message is finished: 30 frames, then the SET 13 object.
pub(super) fn s0c_wait_dialogue(x: &mut Ctx) -> Flow {
    render(x);
    let o = link(x);
    if rd8(x.b, o + 0x22) == 0 {
        return Flow::Done;
    }
    next_step(x);
    x.wait(30, Step::S0cObject)
}
pub(super) const S0C_OBJECT: u32 = 0xfb9fa;
pub(super) fn s0c_object(x: &mut Ctx) -> Flow {
    let Ok(Obj(set)) = set_15_pure(x.b, 13) else {
        return x.wait(1, Step::S0cRetry);
    };
    wr32(x.b, set + 0x4c, BLOCK);
    wr16(x.b, BLOCK + f::TIMER, 180);
    Flow::Done
}
pub(super) const S0C_RETRY: u32 = 0xfba14;

/// State 14: after the timer, fade to black (task), next frame, palette, players' objects.
pub(super) fn s0e_wait_timer(x: &mut Ctx) -> Flow {
    render(x);
    if !timer_tick(x) {
        return Flow::Done;
    }
    next_step(x);
    fade_settings(x, 3, FADE_ALL);
    fading::STEP.set(x.b, 1);
    fade_task(x, crate::scene::fading::DESCENT);
    s0e_wait_fade(x)
}
pub(super) fn s0e_wait_fade(x: &mut Ctx) -> Flow {
    if fading::STEP.get(x.b) != 0 {
        return x.wait(1, Step::S0eLoop);
    }
    tile_blocks(x, 0x91b000, 352, 353, Grid::Layer3);
    tile_blocks(x, 0x916000, 543, 0, Grid::Layer2);
    wr16(x.b, var::CAMERA_2_Y, 256);
    wr16(x.b, var::CAMERA_2, 0);
    wr16(x.b, var::CAMERA_Y, 256);
    wr16(x.b, var::CAMERA, 128);
    // palette: eight longs copied then darkened (AND 0x0fff0fff on the next eight)
    let words: Vec<u32> = (0..8).map(|i| rd32(x.b, 0x291980 + 4 * i)).collect();
    for (i, w) in words.iter().enumerate() {
        wr32(x.b, 0x91c280 + 4 * i as u32, w & 0x0fff_0fff);
    }
    s0e_wait_music(x)
}
pub(super) const S0E_LOOP: u32 = 0xfba68;
/// Waits for the music to advance (`7e70` >= 3 or `7e66` >= 1), one frame at a time.
pub(super) fn s0e_wait_music(x: &mut Ctx) -> Flow {
    let ready = (rd16(x.b, A5 + 0x7e70) as i16) >= 3 && (rd16(x.b, A5 + 0x7e66) as i16) >= 1;
    x.wait(1, if ready { Step::S0eObjects } else { Step::S0eMusic })
}
pub(super) const S0E_MUSIC: u32 = 0xfbb18;
pub(super) const S0E_OBJECTS: u32 = 0xfbb26;

/// State 16: waits for the camera to stop and the timer, then opens message 90.
pub(super) fn s10_wait_camera(x: &mut Ctx) -> Flow {
    render(x);
    if rd16(x.b, var::CAMERA) != 0 || !timer_tick(x) {
        return Flow::Done;
    }
    next_step(x);
    open_message(x, 90, Step::S10Retry)
}
pub(super) const S10_RETRY: u32 = 0xfbbae;

/// States 18 and 22: the message is finished: 30 frames.
pub(super) fn s12_wait_dialogue(x: &mut Ctx) -> Flow {
    render(x);
    let o = link(x);
    if rd8(x.b, o + 0x22) == 0 {
        return Flow::Done;
    }
    wr16(x.b, BLOCK + f::TIMER, 30);
    next_step(x);
    Flow::Done
}

/// State 20: after the timer, opens message 91.
pub(super) fn s14_wait_timer(x: &mut Ctx) -> Flow {
    render(x);
    if !timer_tick(x) {
        return Flow::Done;
    }
    next_step(x);
    open_message(x, 91, Step::S14Retry)
}
pub(super) const S14_RETRY: u32 = 0xfbbf6;

/// State 24: SET 13 object, cameras reset.
pub(super) fn s18_object(x: &mut Ctx) -> Flow {
    let Ok(Obj(set)) = set_15_pure(x.b, 13) else {
        return x.wait(1, Step::S18Retry);
    };
    wr32(x.b, set + 0x4c, BLOCK);
    for v in [var::CAMERA, var::CAMERA_Y, var::CAMERA_2, var::CAMERA_2_Y] {
        wr16(x.b, v, 0);
    }
    next_step(x);
    wr8(x.b, BLOCK + f::SIGNAL, 2);
    render(x);
    Flow::Done
}
pub(super) const S18_RETRY: u32 = 0xfbc16;

/// State 26: the signal becomes 1: fade (task), clears everything, prepares the players.
pub(super) fn s1a_wait_signal(x: &mut Ctx) -> Flow {
    render(x);
    if rd8(x.b, BLOCK + f::SIGNAL) != 1 {
        return Flow::Done;
    }
    next_step(x);
    crate::sound::service(x.b, crate::sound::Service::Ff07 { lock: 0, line: 4 });
    fading::STEP.set(x.b, 1);
    fading::DELAY.set(x.b, 4);
    fading::PAGES.set(x.b, FADE_ALL);
    fade_task(x, crate::scene::fading::TO_WHITE);
    s1a_wait_fade(x)
}
pub(super) fn s1a_wait_fade(x: &mut Ctx) -> Flow {
    if fading::STEP.get(x.b) != 0 {
        return x.wait(1, Step::S1aLoop);
    }
    wr16(x.b, BLOCK + f::TIMER, 120);
    wr8(x.b, BLOCK + f::SIGNAL, 0xff);
    render(x);
    unlink_and_clear_lists(x);
    reset_pools(x);
    crate::video::clear_layer1_start(x.b);
    wr16(x.b, var::SCROLL_2, 256);
    // `jmp 0xc58`
    crate::video::clear_objects(x.b);
    Flow::Done
}
pub(super) const S1A_LOOP: u32 = 0xfbc92;

/// State 28: the credits: terrain, objects, camera, waiting for the opening.
pub(super) fn s1c_wait(x: &mut Ctx) -> Flow {
    if !timer_tick(x) {
        return Flow::Done;
    }
    next_step(x);
    wr16(x.b, var::VIDEO, 0x138e);
    tile_blocks(x, 0x91b900, 115, 116, Grid::Layer3);
    wr8(x.b, var::SCREEN_ID, 1);
    wr16(x.b, var::CAMERA_2, 384);
    wr16(x.b, var::CAMERA_2_Y, 0);
    let v = rd16(x.b, var::CAMERA_2);
    wr16(x.b, var::CAMERA_2_COPY, v);
    let v = rd16(x.b, var::CAMERA_2_Y);
    wr16(x.b, var::CAMERA_2_COPY_Y, v);
    wr16(x.b, var::CAMERA_2_MODE, 4);
    crate::families::small::background_2_on(x.b);
    credits_objects_a(x);
    credits_objects_b(x);
    credits_objects_c(x);
    wr8(x.b, A5 + 0x34, 11);
    wr8(x.b, A5 + 0x54e, 2);
    wr16(x.b, BLOCK + f::TIMER, 3);
    s1c_scroll(x)
}
/// `0xfbdd2`: terrain objects from a ROM table (ten bytes per object, 0xffff ends).
pub(super) fn credits_objects_a(x: &mut Ctx) {
    let mut a2 = 0xfbe92;
    while rd16(x.b, a2) != 0xffff {
        let Some(Obj(o)) = pools::SOBJ.alloc(x.b) else { return };
        wr8(x.b, o, 1);
        for (dst, size) in [(8u32, 2u32), (0xc, 2), (0x10, 2), (0x2c, 1), (2, 1), (3, 1), (0x2f, 1)] {
            if size == 2 {
                let v = rd16(x.b, a2);
                wr16(x.b, o + dst, v);
            } else {
                let v = rd8(x.b, a2);
                wr8(x.b, o + dst, v);
            }
            a2 += size;
        }
    }
}
/// `0xfbe0a`: one score object per present player.
pub(super) fn credits_objects_b(x: &mut Ctx) {
    let mut fp = var::PLAYERS;
    let mut a2 = 0xfbebc;
    for (counter, d3) in (0..4u32).rev().zip(0u8..) {
        if rd8(x.b, fp) != 0 && (rd8(x.b, fp + 4) as i8) < 4 {
            if let Some(Obj(o)) = end_screen(x.b, 2, counter as u8) {
                let variant = rd8(x.b, fp + 3);
                wr8(x.b, o + 0x2f, variant);
                let (px, py) = (rd16(x.b, a2), rd16(x.b, a2 + 2));
                a2 += 4;
                let cx = rd16(x.b, var::CAMERA_2);
                let cy = rd16(x.b, var::CAMERA_2_Y);
                wr16(x.b, o + 8, px.wrapping_add(cx));
                wr16(x.b, o + 0x10, py.wrapping_add(cy));
                wr32(x.b, o + 0xc, 0);
                wr8(x.b, o + 0x42, d3);
                wr32(x.b, o + 0x4c, BLOCK);
            }
        }
        fp += 0x200;
    }
}
/// `0xfbe6e`: two objects (variant 18).
pub(super) fn credits_objects_c(x: &mut Ctx) {
    for k in 0..2 {
        let Ok(Obj(o)) = scenery_6(x.b, 18) else { return };
        wr8(x.b, o + 0x2f, k);
    }
}
/// `0xfbd38`: render with scrolling for three frames.
pub(super) fn s1c_scroll(x: &mut Ctx) -> Flow {
    render_scroll(x);
    x.wait(1, Step::S1cScroll)
}
pub(super) const S1C_SCROLL: u32 = 0xfbd44;
pub(super) const S1C_PALETTE: u32 = 0xfbd56;
pub(super) fn s1c_wait_fade(x: &mut Ctx) -> Flow {
    x.wait(1, Step::S1cFadeLoop)
}
pub(super) const S1C_FADE_LOOP: u32 = 0xfbdc0;

/// State 30: the credits scroll; at the end of the timer, fade (task), then SET 18 object.
pub(super) fn s1e_fade(x: &mut Ctx) -> Flow {
    render_scroll(x);
    if !timer_tick(x) {
        return Flow::Done;
    }
    fading::STEP.set(x.b, 8);
    fade_settings(x, 3, FADE_ALL);
    fade_task(x, crate::scene::fading::DESCENT_STEP);
    s1e_loop(x)
}
pub(super) fn s1e_loop(x: &mut Ctx) -> Flow {
    x.wait(1, Step::S1eLoop)
}
pub(super) const S1E_LOOP: u32 = 0xfbf00;
pub(super) fn s1e_object(x: &mut Ctx) -> Flow {
    if set_15_pure(x.b, 18).is_ok() {
        return Flow::Done;
    }
    // `moveq 1,D0 ; bsr 0xfc506 ; jsr 0xa40`. Original glitch: the yield lasts D0 frames, as the render leaves it
    // (type × 4 of the last object whose handler was called)
    let d0 = render_scroll_d0(x, 1);
    crate::tasks::wait(x, d0 as u8, Step::S1eRetry)
}
pub(super) const S1E_RETRY: u32 = 0xfbf26;

/// State 34: SET 16 object, fade (task), then SET 20 object.
pub(super) fn s22_object(x: &mut Ctx) -> Flow {
    if set_15_pure(x.b, 16).is_err() {
        return Flow::Done;
    }
    fading::STEP.set(x.b, 8);
    fade_settings(x, 3, FADE_ALL);
    fade_task(x, crate::scene::fading::RISE_STEP);
    s22_wait(x)
}
pub(super) fn s22_wait(x: &mut Ctx) -> Flow {
    render_scroll(x);
    x.wait(1, Step::S22Loop)
}
pub(super) const S22_LOOP: u32 = 0xfbf64;

/// State 38: after the timer, signal 2 and new timer.
pub(super) fn s26_wait(x: &mut Ctx) -> Flow {
    render_scroll(x);
    if !timer_tick(x) {
        return Flow::Done;
    }
    wr8(x.b, BLOCK + f::SIGNAL, 2);
    next_step(x);
    wr16(x.b, BLOCK + f::TIMER, 120);
    Flow::Done
}

/// State 40: after the timer, fade to black (task, waits for the end).
pub(super) fn s28_fade(x: &mut Ctx) -> Flow {
    render_scroll(x);
    if !timer_tick(x) {
        return Flow::Done;
    }
    next_step(x);
    fading::STEP.set(x.b, 1);
    fade_settings(x, 3, 13);
    fade_task(x, crate::scene::fading::DESCENT);
    s28_loop(x)
}
pub(super) fn s28_loop(x: &mut Ctx) -> Flow {
    x.wait(1, Step::S28Loop)
}
pub(super) const S28_LOOP: u32 = 0xfbfe8;

/// State 42: clears everything, removes the planes, next frame, SOBJ 0 object.
pub(super) fn s2a_clear(x: &mut Ctx) -> Flow {
    render_scroll(x);
    unlink_and_clear_lists(x);
    crate::objects::pools::EFFCT.reset(x.b);
    crate::objects::pools::SOBJ.reset(x.b);
    crate::video::clear_objects(x.b);
    x.wait(1, Step::S2aPlanes)
}
pub(super) const S2A_PLANES: u32 = 0xfc022;
pub(super) const S2A_SCROLLS: u32 = 0xfc034;
pub(super) fn s2a_wait(x: &mut Ctx) -> Flow {
    render(x);
    x.wait(1, Step::S2aWait)
}
pub(super) const S2A_WAIT: u32 = 0xfc07e;
pub(super) fn s2a_object(x: &mut Ctx) -> Flow {
    let Ok(Obj(o)) = scenery_6(x.b, 0) else {
        render(x);
        return x.wait(1, Step::S2aRetry);
    };
    wr32(x.b, o + 0x4c, BLOCK);
    wr8(x.b, o + 0x2f, 12);
    wr16(x.b, o + 8, 32);
    wr16(x.b, o + 0xc, 208);
    wr32(x.b, o + 0x10, 0);
    render(x);
    x.wait(1, Step::S2aShow)
}
pub(super) const S2A_RETRY: u32 = 0xfc0c2;
pub(super) const S2A_SHOW: u32 = 0xfc0ee;

/// State 44: after the timer, fade to black, clears everything and prepares the rewards screen.
pub(super) fn s2c_wait(x: &mut Ctx) -> Flow {
    if !timer_tick(x) {
        render(x);
        return Flow::Done;
    }
    next_step(x);
    fade_settings(x, 3, 13);
    fading::STEP.set(x.b, 1);
    fade_task(x, crate::scene::fading::DESCENT);
    s2c_loop(x)
}
pub(super) fn s2c_loop(x: &mut Ctx) -> Flow {
    render(x);
    x.wait(1, Step::S2cLoop)
}
pub(super) const S2C_LOOP: u32 = 0xfc14c;
pub(super) const S2C_PLANES: u32 = 0xfc17e;
pub(super) const S2C_CLEARED: u32 = 0xfc18c;
pub(super) fn s2c_wait_timer(x: &mut Ctx) -> Flow {
    render(x);
    x.wait(1, Step::S2cTimer)
}
pub(super) const S2C_TIMER: u32 = 0xfc1d8;
/// `bra 0xfc4ea`: a render then return.
pub(super) fn render_flow(x: &mut Ctx) -> Flow {
    render(x);
    Flow::Done
}

/// State 46: the ending dialogue opens then closes the screen (fades to black and from black).
pub(super) fn s2e_dialogue(x: &mut Ctx) -> Flow {
    let o = link(x);
    if rd8(x.b, o + 5) == 6 {
        fade_settings(x, 3, FADE_ALL);
        return x.subroutine(crate::tasks::fading::rise, Step::S2eOpened);
    }
    if rd8(x.b, o + 5) != 10 {
        return render_flow(x);
    }
    fade_settings(x, 3, FADE_ALL);
    x.subroutine(crate::tasks::fading::descent, Step::S2eClosed)
}
pub(super) const S2E_OPENED: u32 = 0xfc286;
pub(super) const S2E_CLOSED: u32 = 0xfc2ae;

/// State 48: rewards: palettes, then one object per player who won a reward.
pub(super) fn s30_rewards(x: &mut Ctx) -> Flow {
    for (src, dst) in [(0x28fa80u32, 0x91c120u32), (0x28faa0, 0x91c1a0), (0x28f4c0, 0x91c180), (0x292260, 0x91c1e0), (0x291f00, 0x91c200)] {
        for i in 0..8 {
            let v = rd32(x.b, src + 4 * i);
            wr32(x.b, dst + 4 * i, v);
        }
    }
    fading::PAGES.set(x.b, 1);
    crate::palette::no_brightness(x.b);
    render(x);
    let mut fp = var::PLAYERS;
    for _ in 0..4 {
        if rd8(x.b, fp) != 0 && (rd8(x.b, fp + 4) as i8) < 4 && rd8(x.b, fp + 0x18a) != 0 {
            let character = rd8(x.b, fp + 3);
            crate::families::set_residue::two_grids(x.b, character as u16);
            break;
        }
        fp += 0x200;
    }
    x.wait(1, Step::S30Wait)
}
pub(super) const S30_WAIT: u32 = 0xfc366;

/// State 50: last screen (four tile frames), then end of the game: phase 20.
pub(super) fn s32_end(x: &mut Ctx) -> Flow {
    render(x);
    if rd16(x.b, BLOCK + f::TIMER) == 120 {
        let mut a1 = 0x91336c;
        let mut a2 = 0xfc4e2;
        for _ in 0..4 {
            let first = rd16(x.b, a2);
            a2 += 2;
            // two rows of four tiles: n, n + 16 then n + 1, n + 17 (attribute 2)
            for tile in [first, first.wrapping_add(1)] {
                for t in [tile, tile.wrapping_add(16)] {
                    wr16(x.b, a1, t);
                    wr16(x.b, a1 + 2, 2);
                    a1 += 4;
                }
                a1 += 120;
            }
        }
    }
    if !timer_tick(x) {
        return Flow::Done;
    }
    fade_settings(x, 3, FADE_ALL);
    x.subroutine(crate::tasks::fading::descent, Step::S32Faded)
}
pub(super) const S32_FADED: u32 = 0xfc450;
pub(super) const S32_CLEARED: u32 = 0xfc470;

/// Decimal-coded addition (eight digits, final carry lost): four `abcd`.
pub(super) fn bcd_add(dest: u32, src: u32) -> u32 {
    let mut carry = 0u32;
    let mut out = 0u32;
    for i in 0..4 {
        let (d, s) = ((dest >> (8 * i)) & 0xff, (src >> (8 * i)) & 0xff);
        let mut lo = (d & 0xf) + (s & 0xf) + carry;
        let mut hi = (d >> 4) + (s >> 4);
        if lo > 9 {
            lo -= 10;
            hi += 1;
        }
        carry = 0;
        if hi > 9 {
            hi -= 10;
            carry = 1;
        }
        out |= (hi << 4 | lo) << (8 * i);
    }
    out
}
