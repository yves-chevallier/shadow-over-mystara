//! Phase 28 (`0xfb292`): the end of the game (credits).
//!
//! After the last scene (exit sequence 35, `scene::exit`), the game enters phase 28: the boss is
//! defeated, the players receive their rewards, dialogues and fixed screens follow one another, then
//! phase 20 (game over) hands control back to the mode machine. The states (`A5+0x2d`, step of 2) are stored
//! in order of passage; state 0 *falls through* into state 2 (the counter advances by 4).
//!
//! The state block is at `FF2B00`: `+0` timer (word), `+2` link to the current dialogue object
//! (word), `+8` signal byte read and written by the objects (through the `+0x4c` pointer of their record).
//! Each state returns to `0xfb2a4` (the resume native restores A0).

use super::{Flow, Sequence};
use crate::scene::fading;
use crate::effects_aids::scenery_6;
use crate::families::creators::set_15_pure;
use crate::families::effects_creators::end_screen;
use crate::palette::{load_palette_set, load_palette_page};
use crate::video::{load_grid, clear_layer, Layer, Grid};
use crate::mem::{rd16, rd32, rd8, wr16, wr32, wr8, A5};
use crate::scene::Ctx;
use crate::Native;
use crate::objects::{pools, Obj};

mod states;
use states::*;

const BLOCK: u32 = 0xff_2b00;
mod f {
    pub const TIMER: u32 = 0;
    pub const LINK: u32 = 2;
    pub const SIGNAL: u32 = 8;
}

mod var {
    use crate::mem::A5;
    pub const STEP: u32 = A5 + 0x2d;
    pub const PHASE: u32 = A5 + 0x2c;
    pub const VIDEO: u32 = A5 + 0x14;
    pub const SCROLL_1: u32 = A5 + 0x4b6;
    pub const SCROLL_2: u32 = A5 + 0x4ba;
    pub const CAMERA: u32 = A5 + 0x506;
    pub const CAMERA_Y: u32 = A5 + 0x50a;
    pub const CAMERA_MODE: u32 = A5 + 0x51e;
    pub const SCREEN_ID: u32 = A5 + 0x36;
    pub const CAMERA_2: u32 = A5 + 0x556;
    pub const CAMERA_2_Y: u32 = A5 + 0x55a;
    pub const CAMERA_2_COPY: u32 = A5 + 0x55e;
    pub const CAMERA_2_COPY_Y: u32 = A5 + 0x562;
    pub const CAMERA_2_MODE: u32 = A5 + 0x56e;
    pub const PLAYERS: u32 = A5 + 0x5de;
    pub const CPS_REG_4: u32 = 0x400004;
    pub const COUNTER: u32 = A5 - 0x5b8e;
}

mod at {
    pub const RETURN: u32 = 0xfb2a4;
}

const FADE_ALL: u8 = 0xf;

/// Phase 28 of the game (`0xfb292`): the ending credits. The state returns to [`Step::Return`].
pub fn phase_28(x: &mut Ctx) -> Flow {
    let step = rd8(x.b, var::STEP);
    x.push_return(Step::Return);
    state(x, step)
}

crate::steps! {
    /// Steps (resume points) of the state machine.
    pub enum Step {
        Return = at::RETURN => |_| Flow::Done,
        S00Video = S00_VIDEO => |x| {
            wr16(x.b, var::CPS_REG_4, 0x1355);
            for v in [var::SCROLL_1, var::SCROLL_2, var::CAMERA, var::CAMERA_Y] {
                wr32(x.b, v, 0);
            }
            wr16(x.b, var::CAMERA_MODE, 4);
            wr8(x.b, A5 + 0x53c, 0);
            wr32(x.b, var::CAMERA_2, 0);
            wr32(x.b, var::CAMERA_2_Y, 0);
            wr16(x.b, var::CAMERA_2_MODE, 4);
            wr16(x.b, var::VIDEO, 0x12c0);
            let v = rd8(x.b, A5 + 0x13) & !1;
            wr8(x.b, A5 + 0x13, v);
            x.wait(4, Step::S00Palette)
        },
        S00Palette = S00_PALETTE => |x| {
            crate::palette::load_scene_page(x.b, 1);
            load_palette_page(x.b, 2, 89);
            load_palette_page(x.b, 3, 86);
            fading::PAGES.set(x.b, 12);
            crate::palette::full_brightness(x.b);
            s02_clear(x)
        },
        S02Cleared = S02_CLEARED => |x| {
            clear_layers(x, &[Layer::Two, Layer::Three]);
            Flow::Done
        },
        S04Fade = S04_FADE => |x| {
            crate::sound::play_global(x.b, crate::sound::CMD_2C);
            fade_settings(x, 3, 4);
            x.subroutine(crate::tasks::fading::rise, Step::S04Timer)
        },
        S04Timer = S04_TIMER => |x| {
            wr16(x.b, BLOCK + f::TIMER, 60);
            Flow::Done
        },
        S06Retry = S06_RETRY => s06_object,
        S08Fade = S08_FADE => |x| {
            next_step(x);
            fade_settings(x, 3, 4);
            x.subroutine(crate::tasks::fading::descent, Step::S08Faded)
        },
        S08Faded = S08_FADED => |x| {
            let v = rd16(x.b, var::VIDEO) & !0xe;
            wr16(x.b, var::VIDEO, v);
            x.wait(4, Step::S08Picture)
        },
        S08Picture = S08_PICTURE => |x| {
            load_palette_page(x.b, 3, 87);
            load_palette_set(x.b, 6);
            tile_blocks(x, 0x91b800, 350, 351, Grid::Layer3);
            tile_blocks(x, 0x917000, 548, 549, Grid::Layer2);
            wr8(x.b, BLOCK + f::SIGNAL, 0);
            s08_object(x)
        },
        S08Retry = S08_RETRY => s08_object,
        S0aRetry = S0A_RETRY => |x| open_message(x, 89, Step::S0aRetry),
        S0cObject = S0C_OBJECT => s0c_object,
        S0cRetry = S0C_RETRY => s0c_object,
        S0eLoop = S0E_LOOP => |x| {
            render(x);
            s0e_wait_fade(x)
        },
        S0eMusic = S0E_MUSIC => |x| {
            render(x);
            s0e_wait_music(x)
        },
        S0eObjects = S0E_OBJECTS => |x| {
            wr8(x.b, var::SCREEN_ID, 1);
            // Original glitch: the carry is not tested; a failure writes into the A1 left by the allocation
            let _ = scenery_6(x.b, 11);
            let o = scenery_6(x.b, 12).unwrap_or_else(|a1| a1);
            wr8(x.b, o.0 + 0x2f, 0);
            let o = scenery_6(x.b, 12).unwrap_or_else(|a1| a1);
            wr8(x.b, o.0 + 0x2f, 1);
            let _ = set_15_pure(x.b, 14);
            wr8(x.b, BLOCK + f::SIGNAL, 0xff);
            render(x);
            wr8(x.b, BLOCK + f::SIGNAL, 1);
            wr16(x.b, BLOCK + f::TIMER, 180);
            fade_settings(x, 3, FADE_ALL);
            // `jmp 0x9b6`: the fade task, then the return
            crate::kernel::create_task(x.b, 0x10, crate::scene::fading::RISE);
            Flow::Done
        },
        S10Retry = S10_RETRY => |x| {
            render(x);
            open_message(x, 90, Step::S10Retry)
        },
        S14Retry = S14_RETRY => |x| {
            render(x);
            open_message(x, 91, Step::S14Retry)
        },
        S18Retry = S18_RETRY => s18_object,
        S1aLoop = S1A_LOOP => |x| {
            render(x);
            s1a_wait_fade(x)
        },
        S1cScroll = S1C_SCROLL => |x| {
            if !timer_tick(x) {
                return s1c_scroll(x);
            }
            wr16(x.b, var::SCROLL_2, 0);
            x.wait(4, Step::S1cPalette)
        },
        S1cPalette = S1C_PALETTE => |x| {
            crate::palette::load_scene_palettes(x.b);
            // saves the 1024 palette bytes at 0x910000, then fills them with white
            for i in 0..256u32 {
                let v = rd32(x.b, 0x91c000 + 4 * i);
                wr32(x.b, 0x910000 + 4 * i, v);
            }
            for i in 0..256u32 {
                wr32(x.b, 0x91c000 + 4 * i, 0xffff_ffff);
            }
            fading::DELAY.set(x.b, 1);
            fading::PAGES.set(x.b, FADE_ALL);
            wr32(x.b, A5 + 0x492, 0x910000);
            wr32(x.b, A5 + 0x496, 0x25c0c0);
            wr32(x.b, A5 + 0x49a, 0x269120);
            wr32(x.b, A5 + 0x49e, 0x27e600);
            crate::kernel::create_task(x.b, 0x10, crate::scene::fading::FROM_WHITE);
            crate::sound::play_global(x.b, crate::sound::CMD_33);
            fading::STEP.set(x.b, 1);
            s1c_wait_fade(x)
        },
        S1cFadeLoop = S1C_FADE_LOOP => |x| {
            render_scroll(x);
            if fading::STEP.get(x.b) != 0 {
                return s1c_wait_fade(x);
            }
            wr16(x.b, BLOCK + f::TIMER, 30);
            Flow::Done
        },
        S1eLoop = S1E_LOOP => |x| {
            render_scroll(x);
            if fading::STEP.get(x.b) != 0 {
                return s1e_loop(x);
            }
            next_step(x);
            s1e_object(x)
        },
        S1eRetry = S1E_RETRY => s1e_object,
        S22Loop = S22_LOOP => |x| {
            if fading::STEP.get(x.b) != 0 {
                return s22_wait(x);
            }
            next_step(x);
            wr16(x.b, BLOCK + f::TIMER, 60);
            wr8(x.b, BLOCK + f::SIGNAL, 0);
            render_scroll(x);
            let Ok(set) = set_15_pure(x.b, 20) else {
                return Flow::Done;
            };
            wr16(x.b, BLOCK + f::LINK, set.0 as u16);
            let n = rd8(x.b, var::COUNTER).wrapping_add(1);
            wr8(x.b, var::COUNTER, n);
            Flow::Done
        },
        S28Loop = S28_LOOP => |x| {
            render_scroll(x);
            if fading::STEP.get(x.b) != 0 {
                return s28_loop(x);
            }
            Flow::Done
        },
        S2aPlanes = S2A_PLANES => |x| {
            render_scroll(x);
            let v = rd16(x.b, var::VIDEO) & !0xc;
            wr16(x.b, var::VIDEO, v);
            x.wait(1, Step::S2aScrolls)
        },
        S2aScrolls = S2A_SCROLLS => |x| {
            clear_layers(x, &[Layer::Two, Layer::Three]);
            wr8(x.b, var::CAMERA_MODE, 4);
            for v in [var::CAMERA, var::CAMERA_Y, var::CAMERA_2, var::CAMERA_2_Y] {
                wr16(x.b, v, 0);
            }
            next_step(x);
            wr16(x.b, BLOCK + f::TIMER, 4);
            load_palette_page(x.b, 3, 87);
            load_palette_set(x.b, 6);
            s2a_wait(x)
        },
        S2aWait = S2A_WAIT => |x| {
            if !timer_tick(x) {
                return s2a_wait(x);
            }
            wr16(x.b, BLOCK + f::TIMER, 600);
            tile_blocks(x, 0x91b800, 354, 355, Grid::Layer3);
            wr8(x.b, BLOCK + f::SIGNAL, 0);
            s2a_object(x)
        },
        S2aRetry = S2A_RETRY => s2a_object,
        S2aShow = S2A_SHOW => |x| {
            wr16(x.b, var::VIDEO, 0x12ce);
            render(x);
            fade_settings(x, 3, 13);
            crate::kernel::create_task(x.b, 0x10, crate::scene::fading::RISE);
            Flow::Done
        },
        S2cLoop = S2C_LOOP => |x| {
            if fading::STEP.get(x.b) != 0 {
                return s2c_loop(x);
            }
            wr8(x.b, BLOCK + f::SIGNAL, 0xff);
            unlink_and_clear_lists(x);
            crate::objects::pools::EFFCT.reset(x.b);
            crate::objects::pools::SOBJ.reset(x.b);
            crate::video::clear_objects(x.b);
            x.wait(1, Step::S2cPlanes)
        },
        S2cPlanes = S2C_PLANES => |x| {
            let v = rd16(x.b, var::VIDEO) & !0xc;
            wr16(x.b, var::VIDEO, v);
            x.wait(1, Step::S2cCleared)
        },
        S2cCleared = S2C_CLEARED => |x| {
            clear_layers(x, &[Layer::Three, Layer::One]);
            wr16(x.b, BLOCK + f::TIMER, 4);
            let o = link(x);
            wr32(x.b, o + 4, 0x0600_0000);
            let n = rd8(x.b, var::COUNTER).wrapping_sub(1);
            wr8(x.b, var::COUNTER, n);
            wr8(x.b, var::SCREEN_ID, 0);
            wr8(x.b, A5 + 0x53c, 0);
            let v = rd8(x.b, A5 + 0x13) & !1;
            wr8(x.b, A5 + 0x13, v);
            for v in [var::CAMERA, var::CAMERA_Y, var::SCROLL_1, var::SCROLL_2] {
                wr16(x.b, v, 0);
            }
            s2c_wait_timer(x)
        },
        S2cTimer = S2C_TIMER => |x| {
            if !timer_tick(x) {
                return s2c_wait_timer(x);
            }
            load_palette_page(x.b, 2, 84);
            load_palette_page(x.b, 3, 84);
            fading::PAGES.set(x.b, 12);
            crate::palette::no_brightness(x.b);
            crate::sound::service(x.b, crate::sound::Service::Ff07 { lock: 0, line: 4 });
            // present players and players "ready to leave" (byte +0x192)
            let mut present = 0u8;
            let mut ready = 0u8;
            let count = rd8(x.b, A5 + 0x20) as u32 + 1;
            let mut fp = var::PLAYERS;
            for _ in 0..=count {
                if rd8(x.b, fp) != 0 && (rd8(x.b, fp + 4) as i8) < 4 {
                    present += 1;
                    if rd8(x.b, fp + 0x192) != 0 {
                        ready += 1;
                    }
                }
                fp += 0x200;
            }
            if ready != 0 || present.wrapping_sub(2) == rd8(x.b, A5 + 0x20) {
                if let Ok(set) = set_15_pure(x.b, 22) {
                    wr16(x.b, BLOCK + f::LINK, set.0 as u16);
                    return render_flow(x);
                }
            }
            next_step(x);
            render_flow(x)
        },
        S2eOpened = S2E_OPENED => |x| {
            let o = link(x);
            let v = rd8(x.b, o + 5).wrapping_add(2);
            wr8(x.b, o + 5, v);
            render_flow(x)
        },
        S2eClosed = S2E_CLOSED => |x| {
            let o = link(x);
            let v = rd8(x.b, o + 5).wrapping_add(2);
            wr8(x.b, o + 5, v);
            render_flow(x)
        },
        S30Wait = S30_WAIT => |x| {
            if (rd16(x.b, A5 + 0x7e68) as i16) < 3 {
                return Flow::Done;
            }
            next_step(x);
            let mut d4 = 13u8;
            for counter in (0..3u32).rev() {
                wr8(x.b, BLOCK + f::SIGNAL, 0);
                // Original glitch: the carry is not tested; a failure writes into the A1 left by the allocation
                let Obj(o) = scenery_6(x.b, 0).unwrap_or_else(|a1| a1);
                wr32(x.b, o + 0x4c, BLOCK);
                let v = rd16(x.b, var::CAMERA);
                wr16(x.b, o + 8, v);
                let v = rd16(x.b, var::CAMERA_Y);
                wr16(x.b, o + 0xc, v);
                wr32(x.b, o + 0x10, 0);
                wr8(x.b, o + 0x2f, d4);
                d4 = d4.wrapping_add(1);
                let idx = counter * 4;
                let dx = x.prog16(0xfc3e4 + idx);
                let dy = x.prog16(0xfc3e6 + idx);
                let ox = rd16(x.b, o + 8).wrapping_add(dx);
                wr16(x.b, o + 8, ox);
                let oy = rd16(x.b, o + 0xc).wrapping_add(dy);
                wr16(x.b, o + 0xc, oy);
            }
            render(x);
            wr16(x.b, var::VIDEO, 0x12ce);
            wr16(x.b, BLOCK + f::TIMER, 240);
            fade_settings(x, 3, FADE_ALL);
            crate::kernel::create_task(x.b, 0x10, crate::scene::fading::RISE);
            Flow::Done
        },
        S32Faded = S32_FADED => |x| {
            unlink_and_clear_lists(x);
            reset_pools(x);
            crate::video::clear_objects(x.b);
            x.wait(1, Step::S32Cleared)
        },
        S32Cleared = S32_CLEARED => |x| {
            clear_layers(x, &[Layer::One, Layer::Two, Layer::Three]);
            // the score (BCD long at +0x110) gets the reward (+0x114, low nibble dropped), capped at 9 999 999
            for k in 0..4u32 {
                let p = var::PLAYERS + 0x200 * k;
                if rd8(x.b, p) == 0 {
                    continue;
                }
                wr32(x.b, p + 4, 0x0600_0000);
                let reward = rd32(x.b, p + 0x114) & 0xffff_fff0;
                wr32(x.b, p + 0x114, reward);
                let score = bcd_add(rd32(x.b, p + 0x110), reward);
                wr32(x.b, p + 0x110, if score as i32 > 0x0999_9999 { 0x0999_9999 } else { score });
            }
            crate::game::update_players(x.b);
            wr8(x.b, var::PHASE, 20);
            crate::sound::service(x.b, crate::sound::Service::GeneralStop);
            // `jmp 0x133f6`: the scored startup done, then the return to the caller
            crate::families::small::startup_done(x.b, true);
            Flow::Done
        },
    }
}

fn state(x: &mut Ctx, step: u8) -> Flow {
    match step {
        0 => s00_reset(x),
        2 => s02_clear(x),
        4 => s04_picture(x),
        6 => s06_wait(x),
        8 => s08_wait_dialogue(x),
        10 => s0a_wait_signal(x),
        12 => s0c_wait_dialogue(x),
        14 => s0e_wait_timer(x),
        16 => s10_wait_camera(x),
        18 | 22 => s12_wait_dialogue(x),
        20 => s14_wait_timer(x),
        24 => s18_object(x),
        26 => s1a_wait_signal(x),
        28 => s1c_wait(x),
        30 => s1e_fade(x),
        32 => render_scroll(x),
        34 => s22_object(x),
        36 => render_scroll(x),
        38 => s26_wait(x),
        40 => s28_fade(x),
        42 => s2a_clear(x),
        44 => s2c_wait(x),
        46 => s2e_dialogue(x),
        48 => s30_rewards(x),
        50 => s32_end(x),
        _ => Flow::Done,
    }
}

// ---------------------------------------------------------------------------------------------
// Aides

fn next_step(x: &mut Ctx) {
    let s = rd8(x.b, var::STEP).wrapping_add(2);
    wr8(x.b, var::STEP, s);
}

/// `0xfc4ea`: terrain (SET, SOBJ) and sprites; A0 is preserved.
fn render(x: &mut Ctx) {
    crate::dispatch::SET.update_no_next(x.b);
    crate::dispatch::SOBJ.update_no_next(x.b);
    crate::sprites::build_in_game(x.b);
}

/// `0xfc506`: like `render`, with the effects and the cameras (whose D4 is re-read by handlers: see
/// `scroll::update_cameras_in_game`).
fn render_scroll(x: &mut Ctx) -> Flow {
    render_scroll_d0(x, 0);
    Flow::Done
}

/// [`render_scroll`] called with D0 = `d0`: returns the D0 that it leaves (type × 4 of the last object whose handler
/// was called, `dispatch::Class::update_last_kind`; otherwise `d0`).
fn render_scroll_d0(x: &mut Ctx, d0: u32) -> u32 {
    let mut kinds = [None; 3];
    kinds[0] = crate::dispatch::SET.update_last_kind(x.b);
    kinds[1] = crate::dispatch::EFFCT.update_last_kind(x.b);
    crate::scroll::update_cameras_in_game(x.b);
    kinds[2] = crate::dispatch::SOBJ.update_last_kind(x.b);
    crate::sprites::build_in_game(x.b);
    kinds.into_iter().flatten().last().map_or(d0, |kind| ((kind as u16) << 2) as u32)
}

/// Cleared layers (0xca8, 0xcc8, 0xce8).
fn clear_layers(x: &mut Ctx, layers: &[Layer]) {
    for &layer in layers {
        clear_layer(x.b, layer);
    }
}

/// Sorted lists undone (0x1f184) and sprite lists emptied (0x1de7c).
fn unlink_and_clear_lists(x: &mut Ctx) {
    crate::objects::lists::unlink_all(x.b);
    crate::sprites::clear_lists(x.b);
}

/// The ten pools reset (0x2062).
fn reset_pools(x: &mut Ctx) {
    for pool in pools::POOLS {
        pool.reset(x.b);
    }
}

fn timer_tick(x: &mut Ctx) -> bool {
    let left = rd16(x.b, BLOCK + f::TIMER).wrapping_sub(1);
    wr16(x.b, BLOCK + f::TIMER, left);
    left == 0
}

fn link(x: &mut Ctx) -> u32 {
    rd16(x.b, BLOCK + f::LINK) as i16 as i32 as u32 & 0xff_ffff
}

fn fade_settings(x: &mut Ctx, delay: u16, pages: u8) {
    fading::DELAY.set(x.b, delay);
    fading::PAGES.set(x.b, pages);
}

/// Creates the entry fade task `entry` (slot 0x10).
fn fade_task(x: &mut Ctx, entry: u32) {
    crate::kernel::create_task(x.b, 0x10, entry);
}

/// Two tile blocks of the grid `grid` one after the other from `destination`.
fn tile_blocks(x: &mut Ctx, destination: u32, first: u16, second: u16, grid: Grid) {
    let end = load_grid(x.b, grid, first, destination);
    load_grid(x.b, grid, second, end.destination);
}

pub const NATIVES: &[Native] = &[
    
];

#[cfg(test)]
mod tests {
    use super::bcd_add;

    #[test]
    fn addition_decimal() {
        assert_eq!(bcd_add(0x0000_0199, 0x0000_0001), 0x0000_0200);
        assert_eq!(bcd_add(0x0123_4567, 0x0876_5430), 0x0999_9997);
        // the final carry is lost
        assert_eq!(bcd_add(0x9999_9999, 0x0000_0001), 0x0000_0000);
    }
}
