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
use crate::mem::{rd16, rd32, rd8, rts, wr16, wr32, wr8, A5};
use crate::scene::Ctx;
use crate::Native;
use crate::objects::{pools, Obj};

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

mod ext {
    pub const SOUND_E1528: u32 = 0xe1528;
    pub const SOUND_E16F0: u32 = 0xe16f0;
}

mod at {
    pub const RETURN: u32 = 0xfb2a4;
}

const FADE_ALL: u8 = 0xf;

pub const PHASE_28: Native = Native {
    entry: 0xfb292,
    name: "ending_phase_28",
    run: crate::Body::WithCpu(|c, b| {
        let mut x = Ctx::new(c, b);
        if phase_28(&mut x) == Flow::Done {
            rts(x.c, x.b);
        }
    }),
    touches_sound: false,
    outputs: 0,
};
/// Phase 28 of the game (`0xfb292`): the ending credits. The state returns to [`Step::Return`].
pub fn phase_28(x: &mut Ctx) -> Flow {
    x.c.a[0] = BLOCK;
    let step = rd8(x.b, var::STEP);
    x.push_return(Step::Return);
    state(x, step)
}

crate::steps! {
    /// Steps (resume points) of the state machine.
    pub enum Step {
        Return = at::RETURN => |x| {
            x.c.a[0] = BLOCK;
            Flow::Done
        },
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
            crate::sound::play_global(x.b, ext::SOUND_E1528);
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
            crate::sound::play_global(x.b, ext::SOUND_E16F0);
            fading::STEP.set(x.b, 1);
            x.c.a[0] = BLOCK;
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
            x.c.a[0] = BLOCK;
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
            crate::player::on_bench(x.c, x.b, crate::player::update::update_all);
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
    let a0 = x.c.a[0];
    crate::dispatch::SET.update_no_next(x.c, x.b);
    crate::dispatch::SOBJ.update_no_next(x.c, x.b);
    crate::sprites::build_68k(x.c, x.b);
    x.c.a[0] = a0;
}

/// `0xfc506`: like `render`, with the effects and the cameras (whose D4 is re-read by handlers: see
/// `scroll::update_cameras_68k`).
fn render_scroll(x: &mut Ctx) -> Flow {
    let a0 = x.c.a[0];
    crate::dispatch::SET.update_no_next(x.c, x.b);
    crate::dispatch::EFFCT.update_no_next(x.c, x.b);
    crate::scroll::update_cameras_68k(x.c, x.b);
    crate::dispatch::SOBJ.update_no_next(x.c, x.b);
    crate::sprites::build_68k(x.c, x.b);
    x.c.a[0] = a0;
    Flow::Done
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

// ---------------------------------------------------------------------------------------------
// States

/// State 0: clears everything, copies the players' rewards, then falls into state 2.
fn s00_reset(x: &mut Ctx) -> Flow {
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
fn reward_table(x: &mut Ctx, p: u32) {
    let mut offset = (rd8(x.b, p + 3) as u32) << 3;
    if rd16(x.b, A5 + 0x70) != 0 {
        offset += 96;
    }
    for i in 0..2 {
        let v = rd32(x.b, 0xfb7bc + offset + 4 * i);
        wr32(x.b, p + 0x118 + 4 * i, v);
    }
}
const S00_VIDEO: u32 = 0xfb712;
const S00_PALETTE: u32 = 0xfb758;

/// State 2 (where state 0 falls): the screen is cleared.
fn s02_clear(x: &mut Ctx) -> Flow {
    next_step(x);
    wr16(x.b, var::VIDEO, 0x12c0);
    x.wait(1, Step::S02Cleared)
}
const S02_CLEARED: u32 = 0xfb78c;

/// State 4: background picture, opening fade.
fn s04_picture(x: &mut Ctx) -> Flow {
    next_step(x);
    tile_blocks(x, 0x917000, 0x222, 0x223, Grid::Layer2);
    wr16(x.b, var::VIDEO, 0x12ce);
    x.wait(1, Step::S04Fade)
}
const S04_FADE: u32 = 0xfb8a8;
const S04_TIMER: u32 = 0xfb8c0;

/// State 6: after the timer, creates the dialogue object (SET 70).
fn s06_wait(x: &mut Ctx) -> Flow {
    if !timer_tick(x) {
        return Flow::Done;
    }
    next_step(x);
    s06_object(x)
}
fn s06_object(x: &mut Ctx) -> Flow {
    let Some(Obj(o)) = pools::SET.alloc(x.b) else { return x.wait(1, Step::S06Retry) };
    wr8(x.b, o, 1);
    wr8(x.b, o + 2, 0);
    wr8(x.b, o + 3, 70);
    wr8(x.b, o + 0x2f, 0);
    wr16(x.b, BLOCK + f::LINK, o as u16);
    Flow::Done
}
const S06_RETRY: u32 = 0xfb8e0;

/// State 8: the dialogue object reaches state 6: fade to black then new terrain.
fn s08_wait_dialogue(x: &mut Ctx) -> Flow {
    render(x);
    let o = link(x);
    if rd8(x.b, o + 4) != 6 {
        return Flow::Done;
    }
    x.wait(60, Step::S08Fade)
}
const S08_FADE: u32 = 0xfb916;
const S08_FADED: u32 = 0xfb92c;
const S08_PICTURE: u32 = 0xfb93a;
fn s08_object(x: &mut Ctx) -> Flow {
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
const S08_RETRY: u32 = 0xfb996;

/// State 10: waits for the object's signal, then opens message 89.
fn s0a_wait_signal(x: &mut Ctx) -> Flow {
    render(x);
    if rd8(x.b, BLOCK + f::SIGNAL) == 0 {
        return Flow::Done;
    }
    next_step(x);
    open_message(x, 89, Step::S0aRetry)
}
/// Message `n` (0xc8312): the dialogue object is stored in the block; `retry`: yield on failure.
fn open_message(x: &mut Ctx, n: u8, retry: Step) -> Flow {
    let Some(message) = crate::families::creators::scene_set(x.b, n) else {
        return x.wait(1, retry);
    };
    wr16(x.b, BLOCK + f::LINK, message.0 as u16);
    Flow::Done
}
const S0A_RETRY: u32 = 0xfb9dc;

/// State 12: the message is finished: 30 frames, then the SET 13 object.
fn s0c_wait_dialogue(x: &mut Ctx) -> Flow {
    render(x);
    let o = link(x);
    if rd8(x.b, o + 0x22) == 0 {
        return Flow::Done;
    }
    next_step(x);
    x.wait(30, Step::S0cObject)
}
const S0C_OBJECT: u32 = 0xfb9fa;
fn s0c_object(x: &mut Ctx) -> Flow {
    let Ok(Obj(set)) = set_15_pure(x.b, 13) else {
        return x.wait(1, Step::S0cRetry);
    };
    wr32(x.b, set + 0x4c, BLOCK);
    wr16(x.b, BLOCK + f::TIMER, 180);
    Flow::Done
}
const S0C_RETRY: u32 = 0xfba14;

/// State 14: after the timer, fade to black (task), next frame, palette, players' objects.
fn s0e_wait_timer(x: &mut Ctx) -> Flow {
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
fn s0e_wait_fade(x: &mut Ctx) -> Flow {
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
const S0E_LOOP: u32 = 0xfba68;
/// Waits for the music to advance (`7e70` >= 3 or `7e66` >= 1), one frame at a time.
fn s0e_wait_music(x: &mut Ctx) -> Flow {
    let ready = (rd16(x.b, A5 + 0x7e70) as i16) >= 3 && (rd16(x.b, A5 + 0x7e66) as i16) >= 1;
    x.wait(1, if ready { Step::S0eObjects } else { Step::S0eMusic })
}
const S0E_MUSIC: u32 = 0xfbb18;
const S0E_OBJECTS: u32 = 0xfbb26;

/// State 16: waits for the camera to stop and the timer, then opens message 90.
fn s10_wait_camera(x: &mut Ctx) -> Flow {
    render(x);
    if rd16(x.b, var::CAMERA) != 0 || !timer_tick(x) {
        return Flow::Done;
    }
    next_step(x);
    open_message(x, 90, Step::S10Retry)
}
const S10_RETRY: u32 = 0xfbbae;

/// States 18 and 22: the message is finished: 30 frames.
fn s12_wait_dialogue(x: &mut Ctx) -> Flow {
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
fn s14_wait_timer(x: &mut Ctx) -> Flow {
    render(x);
    if !timer_tick(x) {
        return Flow::Done;
    }
    next_step(x);
    open_message(x, 91, Step::S14Retry)
}
const S14_RETRY: u32 = 0xfbbf6;

/// State 24: SET 13 object, cameras reset.
fn s18_object(x: &mut Ctx) -> Flow {
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
const S18_RETRY: u32 = 0xfbc16;

/// State 26: the signal becomes 1: fade (task), clears everything, prepares the players.
fn s1a_wait_signal(x: &mut Ctx) -> Flow {
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
fn s1a_wait_fade(x: &mut Ctx) -> Flow {
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
const S1A_LOOP: u32 = 0xfbc92;

/// State 28: the credits: terrain, objects, camera, waiting for the opening.
fn s1c_wait(x: &mut Ctx) -> Flow {
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
fn credits_objects_a(x: &mut Ctx) {
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
fn credits_objects_b(x: &mut Ctx) {
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
fn credits_objects_c(x: &mut Ctx) {
    for k in 0..2 {
        let Ok(Obj(o)) = scenery_6(x.b, 18) else { return };
        wr8(x.b, o + 0x2f, k);
    }
}
/// `0xfbd38`: render with scrolling for three frames.
fn s1c_scroll(x: &mut Ctx) -> Flow {
    render_scroll(x);
    x.wait(1, Step::S1cScroll)
}
const S1C_SCROLL: u32 = 0xfbd44;
const S1C_PALETTE: u32 = 0xfbd56;
fn s1c_wait_fade(x: &mut Ctx) -> Flow {
    x.wait(1, Step::S1cFadeLoop)
}
const S1C_FADE_LOOP: u32 = 0xfbdc0;

/// State 30: the credits scroll; at the end of the timer, fade (task), then SET 18 object.
fn s1e_fade(x: &mut Ctx) -> Flow {
    render_scroll(x);
    if !timer_tick(x) {
        return Flow::Done;
    }
    fading::STEP.set(x.b, 8);
    fade_settings(x, 3, FADE_ALL);
    fade_task(x, crate::scene::fading::DESCENT_STEP);
    s1e_loop(x)
}
fn s1e_loop(x: &mut Ctx) -> Flow {
    x.wait(1, Step::S1eLoop)
}
const S1E_LOOP: u32 = 0xfbf00;
fn s1e_object(x: &mut Ctx) -> Flow {
    if set_15_pure(x.b, 18).is_ok() {
        return Flow::Done;
    }
    // `moveq 1,D0 ; bsr 0xfc506 ; jsr 0xa40`. Original glitch: the yield lasts D0 frames, as the render leaves it
    // (the last object handler called)
    x.c.d[0] = 1;
    render_scroll(x);
    crate::tasks::wait(x, Step::S1eRetry)
}
const S1E_RETRY: u32 = 0xfbf26;

/// State 34: SET 16 object, fade (task), then SET 20 object.
fn s22_object(x: &mut Ctx) -> Flow {
    if set_15_pure(x.b, 16).is_err() {
        return Flow::Done;
    }
    fading::STEP.set(x.b, 8);
    fade_settings(x, 3, FADE_ALL);
    fade_task(x, crate::scene::fading::RISE_STEP);
    s22_wait(x)
}
fn s22_wait(x: &mut Ctx) -> Flow {
    render_scroll(x);
    x.wait(1, Step::S22Loop)
}
const S22_LOOP: u32 = 0xfbf64;

/// State 38: after the timer, signal 2 and new timer.
fn s26_wait(x: &mut Ctx) -> Flow {
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
fn s28_fade(x: &mut Ctx) -> Flow {
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
fn s28_loop(x: &mut Ctx) -> Flow {
    x.wait(1, Step::S28Loop)
}
const S28_LOOP: u32 = 0xfbfe8;

/// State 42: clears everything, removes the planes, next frame, SOBJ 0 object.
fn s2a_clear(x: &mut Ctx) -> Flow {
    render_scroll(x);
    unlink_and_clear_lists(x);
    crate::objects::pools::EFFCT.reset(x.b);
    crate::objects::pools::SOBJ.reset(x.b);
    crate::video::clear_objects(x.b);
    x.wait(1, Step::S2aPlanes)
}
const S2A_PLANES: u32 = 0xfc022;
const S2A_SCROLLS: u32 = 0xfc034;
fn s2a_wait(x: &mut Ctx) -> Flow {
    render(x);
    x.wait(1, Step::S2aWait)
}
const S2A_WAIT: u32 = 0xfc07e;
fn s2a_object(x: &mut Ctx) -> Flow {
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
const S2A_RETRY: u32 = 0xfc0c2;
const S2A_SHOW: u32 = 0xfc0ee;

/// State 44: after the timer, fade to black, clears everything and prepares the rewards screen.
fn s2c_wait(x: &mut Ctx) -> Flow {
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
fn s2c_loop(x: &mut Ctx) -> Flow {
    render(x);
    x.wait(1, Step::S2cLoop)
}
const S2C_LOOP: u32 = 0xfc14c;
const S2C_PLANES: u32 = 0xfc17e;
const S2C_CLEARED: u32 = 0xfc18c;
fn s2c_wait_timer(x: &mut Ctx) -> Flow {
    render(x);
    x.wait(1, Step::S2cTimer)
}
const S2C_TIMER: u32 = 0xfc1d8;
/// `bra 0xfc4ea`: a render then return.
fn render_flow(x: &mut Ctx) -> Flow {
    render(x);
    Flow::Done
}

/// State 46: the ending dialogue opens then closes the screen (fades to black and from black).
fn s2e_dialogue(x: &mut Ctx) -> Flow {
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
const S2E_OPENED: u32 = 0xfc286;
const S2E_CLOSED: u32 = 0xfc2ae;

/// State 48: rewards: palettes, then one object per player who won a reward.
fn s30_rewards(x: &mut Ctx) -> Flow {
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
const S30_WAIT: u32 = 0xfc366;

/// State 50: last screen (four tile frames), then end of the game: phase 20.
fn s32_end(x: &mut Ctx) -> Flow {
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
const S32_FADED: u32 = 0xfc450;
const S32_CLEARED: u32 = 0xfc470;

/// Decimal-coded addition (eight digits, final carry lost): four `abcd`.
fn bcd_add(dest: u32, src: u32) -> u32 {
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

pub const NATIVES: &[Native] = &[
    PHASE_28,
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
