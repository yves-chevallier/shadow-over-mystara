//! States of the ending sequence (`FF0E72`, step of 2): pictures, dialogues, credits, rewards.

use super::*;
use crate::hw::video::{palette, TEXT, SCROLL2};
use crate::world::ram::Byte;
use crate::world::screen_tables as tables;

/// Rewards of the endings: two longs per character, a second series for the other language (ROM table).
const REWARDS: Rom = Rom::at(0xfb7bc);
/// Terrain of the credits (ten bytes per object, 0xffff ends), places of the score objects, last tiles.
const TERRAIN: Rom = Rom::at(0xfbe92);
const SCORE_PLACES: Rom = Rom::at(0xfbebc);
const LAST_TILES: Rom = Rom::at(0xfc4e2);
/// Library palette darkened for the credits background.
const DARK_PALETTE: Rom = Rom::at(0x291980);
/// Library palettes of the rewards screen and their places in page 0.
const REWARD_PALETTES: [(Rom, Vram); 5] = [
    (Rom::at(0x28fa80), palette(0, 0x90)),
    (Rom::at(0x28faa0), palette(0, 0xd0)),
    (Rom::at(0x28f4c0), palette(0, 0xc0)),
    (Rom::at(0x292260), palette(0, 0xf0)),
    (Rom::at(0x291f00), palette(0, 0x100)),
];
/// Progress of the music (`A5+0x7e70`: bar, `A5+0x7e66`: beat, h), awaited before the objects of state 14.
const MUSIC_BAR: crate::world::ram::Word = crate::world::ram::Word::a5(0x7e70);
const MUSIC_BEAT: crate::world::ram::Word = crate::world::ram::Word::a5(0x7e66);
/// State of camera 2 (`A5+0x54e`).
const CAMERA_2_STATE: Byte = Byte::a5(0x54e);
/// Player fields: items (two longs, the rewards of the ending), reward won (h).
const ITEMS: u32 = 0x118;
const REWARDED: u32 = 0x18a;
/// Object fields: orientation (`+0x2c`), rank of a score object (`+0x42`).
const FACING: u32 = 0x2c;
const SCORE_RANK: u32 = 0x42;

/// The dialogue object of the block has finished its message (byte `+0x22`).
fn message_done(x: &mut Ctx) -> bool {
    Obj(link(x)).u8(x.b, MESSAGE_DONE) != 0
}
const MESSAGE_DONE: u32 = 0x22;

// ---------------------------------------------------------------------------------------------
// States

/// State 0: clears everything, copies the players' rewards, then falls into state 2.
pub(super) fn s00_reset(x: &mut Ctx) -> Flow {
    next_step(x);
    crate::world::tasks::traps::kill_task(x.b, 0x30);
    reset_pools(x);
    crate::services::video::black_screen(x.b);
    unlink_and_clear_lists(x);
    crate::services::video::clear_objects(x.b);
    for k in 0..4u32 {
        let p = Obj(crate::world::ram::PLAYERS.nth(k).0);
        if p.u8(x.b, Player::ACTIVE) != 0
            && (p.u8(x.b, Player::MAIN) as i8) < 4
            && p.u32(x.b, ITEMS) == 0xffff_ffff
            && p.u32(x.b, ITEMS + 4) == 0xffff_ffff
        {
            reward_table(x, p);
        }
    }
    x.wait(1, Step::S00Video)
}

/// `0xfb798`: two longs of the endings table (per character and region) into the player.
pub(super) fn reward_table(x: &mut Ctx, p: Obj) {
    let mut offset = (p.u8(x.b, Player::CHARACTER) as u32) << 3;
    if crate::screens::hud::var::LANGUAGE.get(x.b) != 0 {
        offset += 96;
    }
    for i in 0..2 {
        let v = REWARDS.long(x.b, offset + 4 * i);
        p.set_u32(x.b, ITEMS + 4 * i, v);
    }
}

/// State 2 (where state 0 falls): the screen is cleared.
pub(super) fn s02_clear(x: &mut Ctx) -> Flow {
    next_step(x);
    var::VIDEO.set(x.b, 0x12c0);
    x.wait(1, Step::S02Cleared)
}

/// State 4: background picture, opening fade.
pub(super) fn s04_picture(x: &mut Ctx) -> Flow {
    next_step(x);
    tile_blocks(x, SCROLL2.0 + 0x3000, 0x222, 0x223, Grid::Layer2);
    var::VIDEO.set(x.b, 0x12ce);
    x.wait(1, Step::S04Fade)
}

/// State 6: after the timer, creates the dialogue object (SET 70).
pub(super) fn s06_wait(x: &mut Ctx) -> Flow {
    if !timer_tick(x) {
        return Flow::Done;
    }
    next_step(x);
    s06_object(x)
}
pub(super) fn s06_object(x: &mut Ctx) -> Flow {
    let Some(o) = pools::SET.alloc(x.b) else { return x.wait(1, Step::S06Retry) };
    o.set_active(x.b, 1);
    o.set_kind(x.b, 0);
    o.set_variant(x.b, 70);
    o.set_u8(x.b, SCENERY_INDEX, 0);
    Obj(BLOCK).set_u16(x.b, f::LINK, o.0 as u16);
    Flow::Done
}

/// State 8: the dialogue object reaches state 6: fade to black then new terrain.
pub(super) fn s08_wait_dialogue(x: &mut Ctx) -> Flow {
    render(x);
    let o = Obj(link(x));
    if o.state(x.b) != 6 {
        return Flow::Done;
    }
    x.wait(60, Step::S08Fade)
}
pub(super) fn s08_object(x: &mut Ctx) -> Flow {
    let Ok(o) = scenery_6(x.b, 10) else {
        return x.wait(1, Step::S08Retry);
    };
    o.set_u32(x.b, SIGNAL_POINTER, BLOCK);
    render(x);
    let v = var::VIDEO.get(x.b) | 0xe;
    var::VIDEO.set(x.b, v);
    fade_settings(x, 3, FADE_ALL);
    crate::services::fading::rise(x)
}

/// State 10: waits for the object's signal, then opens message 89.
pub(super) fn s0a_wait_signal(x: &mut Ctx) -> Flow {
    render(x);
    if Obj(BLOCK).u8(x.b, f::SIGNAL) == 0 {
        return Flow::Done;
    }
    next_step(x);
    open_message(x, 89, Step::S0aRetry)
}
/// Message `n` (0xc8312): the dialogue object is stored in the block; `retry`: yield on failure.
pub(super) fn open_message(x: &mut Ctx, n: u8, retry: Step) -> Flow {
    let Some(message) = crate::actors::families::creators::scene_set(x.b, n) else {
        return x.wait(1, retry);
    };
    Obj(BLOCK).set_u16(x.b, f::LINK, message.0 as u16);
    Flow::Done
}

/// State 12: the message is finished: 30 frames, then the SET 13 object.
pub(super) fn s0c_wait_dialogue(x: &mut Ctx) -> Flow {
    render(x);
    if !message_done(x) {
        return Flow::Done;
    }
    next_step(x);
    x.wait(30, Step::S0cObject)
}
pub(super) fn s0c_object(x: &mut Ctx) -> Flow {
    let Ok(set) = set_15_pure(x.b, 13) else {
        return x.wait(1, Step::S0cRetry);
    };
    set.set_u32(x.b, SIGNAL_POINTER, BLOCK);
    Obj(BLOCK).set_u16(x.b, f::TIMER, 180);
    Flow::Done
}

/// State 14: after the timer, fade to black (task), next frame, palette, players' objects.
pub(super) fn s0e_wait_timer(x: &mut Ctx) -> Flow {
    render(x);
    if !timer_tick(x) {
        return Flow::Done;
    }
    next_step(x);
    fade_settings(x, 3, FADE_ALL);
    fading::STEP.set(x.b, 1);
    fade_task(x, crate::screens::scene::fading::DESCENT);
    s0e_wait_fade(x)
}
pub(super) fn s0e_wait_fade(x: &mut Ctx) -> Flow {
    if fading::STEP.get(x.b) != 0 {
        return x.wait(1, Step::S0eLoop);
    }
    tile_blocks(x, SCROLL3.0 + 0x3000, 352, 353, Grid::Layer3);
    tile_blocks(x, SCROLL2.0 + 0x2000, 543, 0, Grid::Layer2);
    sv::CAMERA_2_Y.set(x.b, 256);
    sv::CAMERA_2_X.set(x.b, 0);
    sv::CAMERA_Y.set(x.b, 256);
    sv::CAMERA_X.set(x.b, 128);
    // palette: eight longs copied then darkened (AND 0x0fff0fff on the next eight)
    let words: Vec<u32> = (0..8).map(|i| tables::long(x.b, DARK_PALETTE.addr() + 4 * i)).collect();
    for (i, w) in words.iter().enumerate() {
        palette(0, 0x140 + 2 * i as u32).set_long(x.b, w & 0x0fff_0fff);
    }
    s0e_wait_music(x)
}
/// Waits for the music to advance (`7e70` >= 3 or `7e66` >= 1), one frame at a time.
pub(super) fn s0e_wait_music(x: &mut Ctx) -> Flow {
    let ready = (MUSIC_BAR.get(x.b) as i16) >= 3 && (MUSIC_BEAT.get(x.b) as i16) >= 1;
    x.wait(1, if ready { Step::S0eObjects } else { Step::S0eMusic })
}

/// State 16: waits for the camera to stop and the timer, then opens message 90.
pub(super) fn s10_wait_camera(x: &mut Ctx) -> Flow {
    render(x);
    if sv::CAMERA_X.get(x.b) != 0 || !timer_tick(x) {
        return Flow::Done;
    }
    next_step(x);
    open_message(x, 90, Step::S10Retry)
}

/// States 18 and 22: the message is finished: 30 frames.
pub(super) fn s12_wait_dialogue(x: &mut Ctx) -> Flow {
    render(x);
    if !message_done(x) {
        return Flow::Done;
    }
    Obj(BLOCK).set_u16(x.b, f::TIMER, 30);
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

/// State 24: SET 13 object, cameras reset.
pub(super) fn s18_object(x: &mut Ctx) -> Flow {
    let Ok(set) = set_15_pure(x.b, 13) else {
        return x.wait(1, Step::S18Retry);
    };
    set.set_u32(x.b, SIGNAL_POINTER, BLOCK);
    for v in [sv::CAMERA_X, sv::CAMERA_Y, sv::CAMERA_2_X, sv::CAMERA_2_Y] {
        v.set(x.b, 0);
    }
    next_step(x);
    Obj(BLOCK).set_u8(x.b, f::SIGNAL, 2);
    render(x);
    Flow::Done
}

/// State 26: the signal becomes 1: fade (task), clears everything, prepares the players.
pub(super) fn s1a_wait_signal(x: &mut Ctx) -> Flow {
    render(x);
    if Obj(BLOCK).u8(x.b, f::SIGNAL) != 1 {
        return Flow::Done;
    }
    next_step(x);
    crate::services::sound::service(x.b, crate::services::sound::Service::Ff07 { lock: 0, line: 4 });
    fading::STEP.set(x.b, 1);
    fading::DELAY.set(x.b, 4);
    fading::PAGES.set(x.b, FADE_ALL);
    fade_task(x, crate::screens::scene::fading::TO_WHITE);
    s1a_wait_fade(x)
}
pub(super) fn s1a_wait_fade(x: &mut Ctx) -> Flow {
    if fading::STEP.get(x.b) != 0 {
        return x.wait(1, Step::S1aLoop);
    }
    Obj(BLOCK).set_u16(x.b, f::TIMER, 120);
    Obj(BLOCK).set_u8(x.b, f::SIGNAL, 0xff);
    render(x);
    unlink_and_clear_lists(x);
    reset_pools(x);
    crate::services::video::clear_layer1_start(x.b);
    Word(sv::SCROLL_2.0).set(x.b, 256);
    // `jmp 0xc58`
    crate::services::video::clear_objects(x.b);
    Flow::Done
}

/// State 28: the credits: terrain, objects, camera, waiting for the opening.
pub(super) fn s1c_wait(x: &mut Ctx) -> Flow {
    if !timer_tick(x) {
        return Flow::Done;
    }
    next_step(x);
    var::VIDEO.set(x.b, 0x138e);
    tile_blocks(x, SCROLL3.0 + 0x3900, 115, 116, Grid::Layer3);
    var::SCREEN_ID.set(x.b, 1);
    sv::CAMERA_2_X.set(x.b, 384);
    sv::CAMERA_2_Y.set(x.b, 0);
    let v = sv::CAMERA_2_X.get(x.b);
    var::CAMERA_2_COPY.set(x.b, v);
    let v = sv::CAMERA_2_Y.get(x.b);
    var::CAMERA_2_COPY_Y.set(x.b, v);
    var::CAMERA_2_MODE.set(x.b, 4);
    crate::actors::families::small::background_2_on(x.b);
    credits_objects_a(x);
    credits_objects_b(x);
    credits_objects_c(x);
    crate::world::ram::SCENE.set(x.b, 11);
    CAMERA_2_STATE.set(x.b, 2);
    Obj(BLOCK).set_u16(x.b, f::TIMER, 3);
    s1c_scroll(x)
}
/// `0xfbdd2`: terrain objects from a ROM table (ten bytes per object, 0xffff ends).
pub(super) fn credits_objects_a(x: &mut Ctx) {
    let mut a2 = TERRAIN.addr();
    while tables::word(x.b, a2) != 0xffff {
        let Some(o) = pools::SOBJ.alloc(x.b) else { return };
        o.set_active(x.b, 1);
        for (dst, size) in [(Obj::X, 2u32), (Obj::Z, 2), (Obj::Y, 2), (FACING, 1), (Obj::KIND, 1), (Obj::VARIANT, 1), (SCENERY_INDEX, 1)] {
            if size == 2 {
                let v = tables::word(x.b, a2);
                o.set_u16(x.b, dst, v);
            } else {
                let v = tables::byte(x.b, a2);
                o.set_u8(x.b, dst, v);
            }
            a2 += size;
        }
    }
}
/// `0xfbe0a`: one score object per present player.
pub(super) fn credits_objects_b(x: &mut Ctx) {
    let mut a2 = 0;
    for (k, (counter, d3)) in (0..4u32).rev().zip(0u8..).enumerate() {
        let fp = Obj(crate::world::ram::PLAYERS.nth(k as u32).0);
        if fp.u8(x.b, Player::ACTIVE) != 0 && (fp.u8(x.b, Player::MAIN) as i8) < 4 {
            if let Some(o) = end_screen(x.b, 2, counter as u8) {
                let variant = fp.u8(x.b, Player::CHARACTER);
                o.set_u8(x.b, SCENERY_INDEX, variant);
                let (px, py) = (SCORE_PLACES.word(x.b, a2), SCORE_PLACES.word(x.b, a2 + 2));
                a2 += 4;
                let cx = sv::CAMERA_2_X.get(x.b);
                let cy = sv::CAMERA_2_Y.get(x.b);
                o.set_u16(x.b, Obj::X, px.wrapping_add(cx));
                o.set_u16(x.b, Obj::Y, py.wrapping_add(cy));
                o.set_z(x.b, 0);
                o.set_u8(x.b, SCORE_RANK, d3);
                o.set_u32(x.b, SIGNAL_POINTER, BLOCK);
            }
        }
    }
}
/// `0xfbe6e`: two objects (variant 18).
pub(super) fn credits_objects_c(x: &mut Ctx) {
    for k in 0..2 {
        let Ok(o) = scenery_6(x.b, 18) else { return };
        o.set_u8(x.b, SCENERY_INDEX, k);
    }
}
/// `0xfbd38`: render with scrolling for three frames.
pub(super) fn s1c_scroll(x: &mut Ctx) -> Flow {
    render_scroll(x);
    x.wait(1, Step::S1cScroll)
}
pub(super) fn s1c_wait_fade(x: &mut Ctx) -> Flow {
    x.wait(1, Step::S1cFadeLoop)
}

/// State 30: the credits scroll; at the end of the timer, fade (task), then SET 18 object.
pub(super) fn s1e_fade(x: &mut Ctx) -> Flow {
    render_scroll(x);
    if !timer_tick(x) {
        return Flow::Done;
    }
    fading::STEP.set(x.b, 8);
    fade_settings(x, 3, FADE_ALL);
    fade_task(x, crate::screens::scene::fading::DESCENT_STEP);
    s1e_loop(x)
}
pub(super) fn s1e_loop(x: &mut Ctx) -> Flow {
    x.wait(1, Step::S1eLoop)
}
pub(super) fn s1e_object(x: &mut Ctx) -> Flow {
    if set_15_pure(x.b, 18).is_ok() {
        return Flow::Done;
    }
    // `moveq 1,D0 ; bsr 0xfc506 ; jsr 0xa40`. Original glitch: the yield lasts D0 frames, as the render leaves it
    // (type × 4 of the last object whose handler was called)
    let d0 = render_scroll_d0(x, 1);
    crate::world::tasks::wait(x, d0 as u8, Step::S1eRetry)
}

/// State 34: SET 16 object, fade (task), then SET 20 object.
pub(super) fn s22_object(x: &mut Ctx) -> Flow {
    if set_15_pure(x.b, 16).is_err() {
        return Flow::Done;
    }
    fading::STEP.set(x.b, 8);
    fade_settings(x, 3, FADE_ALL);
    fade_task(x, crate::screens::scene::fading::RISE_STEP);
    s22_wait(x)
}
pub(super) fn s22_wait(x: &mut Ctx) -> Flow {
    render_scroll(x);
    x.wait(1, Step::S22Loop)
}

/// State 38: after the timer, signal 2 and new timer.
pub(super) fn s26_wait(x: &mut Ctx) -> Flow {
    render_scroll(x);
    if !timer_tick(x) {
        return Flow::Done;
    }
    Obj(BLOCK).set_u8(x.b, f::SIGNAL, 2);
    next_step(x);
    Obj(BLOCK).set_u16(x.b, f::TIMER, 120);
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
    fade_task(x, crate::screens::scene::fading::DESCENT);
    s28_loop(x)
}
pub(super) fn s28_loop(x: &mut Ctx) -> Flow {
    x.wait(1, Step::S28Loop)
}

/// State 42: clears everything, removes the planes, next frame, SOBJ 0 object.
pub(super) fn s2a_clear(x: &mut Ctx) -> Flow {
    render_scroll(x);
    unlink_and_clear_lists(x);
    crate::world::objects::pools::EFFCT.reset(x.b);
    crate::world::objects::pools::SOBJ.reset(x.b);
    crate::services::video::clear_objects(x.b);
    x.wait(1, Step::S2aPlanes)
}
pub(super) fn s2a_wait(x: &mut Ctx) -> Flow {
    render(x);
    x.wait(1, Step::S2aWait)
}
pub(super) fn s2a_object(x: &mut Ctx) -> Flow {
    let Ok(o) = scenery_6(x.b, 0) else {
        render(x);
        return x.wait(1, Step::S2aRetry);
    };
    o.set_u32(x.b, SIGNAL_POINTER, BLOCK);
    o.set_u8(x.b, SCENERY_INDEX, 12);
    o.set_u16(x.b, Obj::X, 32);
    o.set_u16(x.b, Obj::Z, 208);
    o.set_y(x.b, 0);
    render(x);
    x.wait(1, Step::S2aShow)
}

/// State 44: after the timer, fade to black, clears everything and prepares the rewards screen.
pub(super) fn s2c_wait(x: &mut Ctx) -> Flow {
    if !timer_tick(x) {
        render(x);
        return Flow::Done;
    }
    next_step(x);
    fade_settings(x, 3, 13);
    fading::STEP.set(x.b, 1);
    fade_task(x, crate::screens::scene::fading::DESCENT);
    s2c_loop(x)
}
pub(super) fn s2c_loop(x: &mut Ctx) -> Flow {
    render(x);
    x.wait(1, Step::S2cLoop)
}
pub(super) fn s2c_wait_timer(x: &mut Ctx) -> Flow {
    render(x);
    x.wait(1, Step::S2cTimer)
}
/// `bra 0xfc4ea`: a render then return.
pub(super) fn render_flow(x: &mut Ctx) -> Flow {
    render(x);
    Flow::Done
}

/// State 46: the ending dialogue opens then closes the screen (fades to black and from black).
pub(super) fn s2e_dialogue(x: &mut Ctx) -> Flow {
    let o = Obj(link(x));
    if o.u8(x.b, SUB_STATE) == 6 {
        fade_settings(x, 3, FADE_ALL);
        return x.subroutine(crate::services::fading::rise, Step::S2eOpened);
    }
    if o.u8(x.b, SUB_STATE) != 10 {
        return render_flow(x);
    }
    fade_settings(x, 3, FADE_ALL);
    x.subroutine(crate::services::fading::descent, Step::S2eClosed)
}

/// State 48: rewards: palettes, then one object per player who won a reward.
pub(super) fn s30_rewards(x: &mut Ctx) -> Flow {
    for (src, dst) in REWARD_PALETTES {
        for i in 0..8 {
            let v = src.long(x.b, 4 * i);
            dst.offset(4 * i).set_long(x.b, v);
        }
    }
    fading::PAGES.set(x.b, 1);
    crate::services::palette::no_brightness(x.b);
    render(x);
    for k in 0..4 {
        let fp = Obj(crate::world::ram::PLAYERS.nth(k).0);
        if fp.u8(x.b, Player::ACTIVE) != 0 && (fp.u8(x.b, Player::MAIN) as i8) < 4 && fp.u8(x.b, REWARDED) != 0 {
            let character = fp.u8(x.b, Player::CHARACTER);
            crate::actors::families::set_residue::two_grids(x.b, character as u16);
            break;
        }
    }
    x.wait(1, Step::S30Wait)
}

/// State 50: last screen (four tile frames), then end of the game: phase 20.
pub(super) fn s32_end(x: &mut Ctx) -> Flow {
    render(x);
    if Obj(BLOCK).u16(x.b, f::TIMER) == 120 {
        let mut a1 = TEXT.at(0x136c);
        for k in 0..4 {
            let first = LAST_TILES.word(x.b, 2 * k);
            // two rows of four tiles: n, n + 16 then n + 1, n + 17 (attribute 2)
            for tile in [first, first.wrapping_add(1)] {
                for t in [tile, tile.wrapping_add(16)] {
                    a1.put(x.b, t, 2);
                    a1 = a1.offset(4);
                }
                a1 = a1.offset(120);
            }
        }
    }
    if !timer_tick(x) {
        return Flow::Done;
    }
    fade_settings(x, 3, FADE_ALL);
    x.subroutine(crate::services::fading::descent, Step::S32Faded)
}

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
