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
use crate::screens::scene::fading;
use crate::actors::effects_aids::scenery_6;
use crate::actors::families::creators::set_15_pure;
use crate::actors::families::effects_creators::end_screen;
use crate::services::palette::{load_palette_set, load_palette_page};
use crate::services::video::{load_grid, clear_layer, Layer, Grid};
use crate::hw::video::{regs, Vram, PALETTES, SCROLL1, SCROLL3};
use crate::world::objects::player::Player;
use crate::world::ram::{Long, Word};
use crate::world::screen_tables::Rom;
use crate::screens::vars as sv;
use crate::screens::scene::Ctx;
use crate::Native;
use crate::world::objects::{pools, Obj};

mod states;
use states::*;

const BLOCK: u32 = 0xff_2b00;
mod f {
    pub const TIMER: u32 = 0;
    pub const LINK: u32 = 2;
    pub const SIGNAL: u32 = 8;
}

mod var {
    use crate::world::ram::{Byte, Word};
    pub const STEP: Byte = Byte::a5(0x2d);
    pub const PHASE: Byte = Byte::a5(0x2c);
    pub const VIDEO: Word = Word::a5(0x14);
    pub const SCREEN_ID: Byte = Byte::a5(0x36);
    pub const CAMERA_2_COPY: Word = Word::a5(0x55e);
    pub const CAMERA_2_COPY_Y: Word = Word::a5(0x562);
    pub const CAMERA_2_MODE: Word = Word::a5(0x56e);
    pub const COUNTER: Byte = Byte(0xff_2472);
    /// Frames since the start of the last dialogue page (h), read to time the rewards.
    pub const DIALOGUE_FRAMES: Word = Word::a5(0x7e68);
}

/// Offsets (x, y) of the three reward objects from the camera (PC-relative words).
const REWARD_OFFSETS: Rom = Rom::at(0xfc3e4);
/// Fade from white of the credits: the saved palettes (in layer 1, used as scratch), then three library palettes.
const CREDITS_FADE: [u32; 4] = [SCROLL1.base, 0x25c0c0, 0x269120, 0x27e600];
/// Library palettes of the credits fade.
const PALETTE_SAVE: Vram = Vram(SCROLL1.base);

const FADE_ALL: u8 = 0xf;

/// Phase 28 of the game (`0xfb292`): the ending credits. The state returns to [`Step::Return`].
pub fn phase_28(x: &mut Ctx) -> Flow {
    let step = var::STEP.get(x.b);
    x.push_return(Step::Return);
    state(x, step)
}

crate::steps! {
    /// Steps (resume points) of the state machine.
    pub enum Step {
        Return = 0xfb2a4 => |_| Flow::Done,
        S00Video = 0xfb712 => |x| {
            regs::OBJECT_OUTPUT.set(x.b, 0x1355);
            for v in [sv::SCROLL_1, sv::SCROLL_2, Long(sv::CAMERA_X.0), Long(sv::CAMERA_Y.0)] {
                v.set(x.b, 0);
            }
            sv::CAMERA_MODE.set(x.b, 4);
            sv::SCROLL_MODE.set(x.b, 0);
            Long(sv::CAMERA_2_X.0).set(x.b, 0);
            Long(sv::CAMERA_2_Y.0).set(x.b, 0);
            var::CAMERA_2_MODE.set(x.b, 4);
            var::VIDEO.set(x.b, 0x12c0);
            let v = sv::VIDEO_FLAGS_LOW.get(x.b) & !1;
            sv::VIDEO_FLAGS_LOW.set(x.b, v);
            x.wait(4, Step::S00Palette)
        },
        S00Palette = 0xfb758 => |x| {
            crate::services::palette::load_scene_page(x.b, 1);
            load_palette_page(x.b, 2, 89);
            load_palette_page(x.b, 3, 86);
            fading::PAGES.set(x.b, 12);
            crate::services::palette::full_brightness(x.b);
            s02_clear(x)
        },
        S02Cleared = 0xfb78c => |x| {
            clear_layers(x, &[Layer::Two, Layer::Three]);
            Flow::Done
        },
        S04Fade = 0xfb8a8 => |x| {
            crate::services::sound::play_global(x.b, crate::services::sound::CMD_2C);
            fade_settings(x, 3, 4);
            x.subroutine(crate::core::tasks::fading::rise, Step::S04Timer)
        },
        S04Timer = 0xfb8c0 => |x| {
            Obj(BLOCK).set_u16(x.b, f::TIMER, 60);
            Flow::Done
        },
        S06Retry = 0xfb8e0 => s06_object,
        S08Fade = 0xfb916 => |x| {
            next_step(x);
            fade_settings(x, 3, 4);
            x.subroutine(crate::core::tasks::fading::descent, Step::S08Faded)
        },
        S08Faded = 0xfb92c => |x| {
            let v = var::VIDEO.get(x.b) & !0xe;
            var::VIDEO.set(x.b, v);
            x.wait(4, Step::S08Picture)
        },
        S08Picture = 0xfb93a => |x| {
            load_palette_page(x.b, 3, 87);
            load_palette_set(x.b, 6);
            tile_blocks(x, crate::hw::video::SCROLL3.offset(0x3800).0, 350, 351, Grid::Layer3);
            tile_blocks(x, crate::hw::video::SCROLL2.offset(0x3000).0, 548, 549, Grid::Layer2);
            Obj(BLOCK).set_u8(x.b, f::SIGNAL, 0);
            s08_object(x)
        },
        S08Retry = 0xfb996 => s08_object,
        S0aRetry = 0xfb9dc => |x| open_message(x, 89, Step::S0aRetry),
        S0cObject = 0xfb9fa => s0c_object,
        S0cRetry = 0xfba14 => s0c_object,
        S0eLoop = 0xfba68 => |x| {
            render(x);
            s0e_wait_fade(x)
        },
        S0eMusic = 0xfbb18 => |x| {
            render(x);
            s0e_wait_music(x)
        },
        S0eObjects = 0xfbb26 => |x| {
            var::SCREEN_ID.set(x.b, 1);
            // Original glitch: the carry is not tested; a failure writes into the A1 left by the allocation
            let _ = scenery_6(x.b, 11);
            let o = scenery_6(x.b, 12).unwrap_or_else(|a1| a1);
            o.set_u8(x.b, SCENERY_INDEX, 0);
            let o = scenery_6(x.b, 12).unwrap_or_else(|a1| a1);
            o.set_u8(x.b, SCENERY_INDEX, 1);
            let _ = set_15_pure(x.b, 14);
            Obj(BLOCK).set_u8(x.b, f::SIGNAL, 0xff);
            render(x);
            Obj(BLOCK).set_u8(x.b, f::SIGNAL, 1);
            Obj(BLOCK).set_u16(x.b, f::TIMER, 180);
            fade_settings(x, 3, FADE_ALL);
            // `jmp 0x9b6`: the fade task, then the return
            crate::core::kernel::create_task(x.b, 0x10, crate::screens::scene::fading::RISE);
            Flow::Done
        },
        S10Retry = 0xfbbae => |x| {
            render(x);
            open_message(x, 90, Step::S10Retry)
        },
        S14Retry = 0xfbbf6 => |x| {
            render(x);
            open_message(x, 91, Step::S14Retry)
        },
        S18Retry = 0xfbc16 => s18_object,
        S1aLoop = 0xfbc92 => |x| {
            render(x);
            s1a_wait_fade(x)
        },
        S1cScroll = 0xfbd44 => |x| {
            if !timer_tick(x) {
                return s1c_scroll(x);
            }
            Word(sv::SCROLL_2.0).set(x.b, 0);
            x.wait(4, Step::S1cPalette)
        },
        S1cPalette = 0xfbd56 => |x| {
            crate::services::palette::load_scene_palettes(x.b);
            // saves the 1024 palette bytes at 0x910000, then fills them with white
            for i in 0..256u32 {
                let v = PALETTES.offset(4 * i).long(x.b);
                PALETTE_SAVE.offset(4 * i).set_long(x.b, v);
            }
            for i in 0..256u32 {
                PALETTES.offset(4 * i).set_long(x.b, 0xffff_ffff);
            }
            fading::DELAY.set(x.b, 1);
            fading::PAGES.set(x.b, FADE_ALL);
            for (k, source) in CREDITS_FADE.into_iter().enumerate() {
                sv::fade_source(k as u32).set(x.b, source);
            }
            crate::core::kernel::create_task(x.b, 0x10, crate::screens::scene::fading::FROM_WHITE);
            crate::services::sound::play_global(x.b, crate::services::sound::CMD_33);
            fading::STEP.set(x.b, 1);
            s1c_wait_fade(x)
        },
        S1cFadeLoop = 0xfbdc0 => |x| {
            render_scroll(x);
            if fading::STEP.get(x.b) != 0 {
                return s1c_wait_fade(x);
            }
            Obj(BLOCK).set_u16(x.b, f::TIMER, 30);
            Flow::Done
        },
        S1eLoop = 0xfbf00 => |x| {
            render_scroll(x);
            if fading::STEP.get(x.b) != 0 {
                return s1e_loop(x);
            }
            next_step(x);
            s1e_object(x)
        },
        S1eRetry = 0xfbf26 => s1e_object,
        S22Loop = 0xfbf64 => |x| {
            if fading::STEP.get(x.b) != 0 {
                return s22_wait(x);
            }
            next_step(x);
            Obj(BLOCK).set_u16(x.b, f::TIMER, 60);
            Obj(BLOCK).set_u8(x.b, f::SIGNAL, 0);
            render_scroll(x);
            let Ok(set) = set_15_pure(x.b, 20) else {
                return Flow::Done;
            };
            Obj(BLOCK).set_u16(x.b, f::LINK, set.0 as u16);
            let n = var::COUNTER.get(x.b).wrapping_add(1);
            var::COUNTER.set(x.b, n);
            Flow::Done
        },
        S28Loop = 0xfbfe8 => |x| {
            render_scroll(x);
            if fading::STEP.get(x.b) != 0 {
                return s28_loop(x);
            }
            Flow::Done
        },
        S2aPlanes = 0xfc022 => |x| {
            render_scroll(x);
            let v = var::VIDEO.get(x.b) & !0xc;
            var::VIDEO.set(x.b, v);
            x.wait(1, Step::S2aScrolls)
        },
        S2aScrolls = 0xfc034 => |x| {
            clear_layers(x, &[Layer::Two, Layer::Three]);
            sv::CAMERA_MODE_HIGH.set(x.b, 4);
            for v in [sv::CAMERA_X, sv::CAMERA_Y, sv::CAMERA_2_X, sv::CAMERA_2_Y] {
                v.set(x.b, 0);
            }
            next_step(x);
            Obj(BLOCK).set_u16(x.b, f::TIMER, 4);
            load_palette_page(x.b, 3, 87);
            load_palette_set(x.b, 6);
            s2a_wait(x)
        },
        S2aWait = 0xfc07e => |x| {
            if !timer_tick(x) {
                return s2a_wait(x);
            }
            Obj(BLOCK).set_u16(x.b, f::TIMER, 600);
            tile_blocks(x, SCROLL3.0 + 0x3800, 354, 355, Grid::Layer3);
            Obj(BLOCK).set_u8(x.b, f::SIGNAL, 0);
            s2a_object(x)
        },
        S2aRetry = 0xfc0c2 => s2a_object,
        S2aShow = 0xfc0ee => |x| {
            var::VIDEO.set(x.b, 0x12ce);
            render(x);
            fade_settings(x, 3, 13);
            crate::core::kernel::create_task(x.b, 0x10, crate::screens::scene::fading::RISE);
            Flow::Done
        },
        S2cLoop = 0xfc14c => |x| {
            if fading::STEP.get(x.b) != 0 {
                return s2c_loop(x);
            }
            Obj(BLOCK).set_u8(x.b, f::SIGNAL, 0xff);
            unlink_and_clear_lists(x);
            crate::world::objects::pools::EFFCT.reset(x.b);
            crate::world::objects::pools::SOBJ.reset(x.b);
            crate::services::video::clear_objects(x.b);
            x.wait(1, Step::S2cPlanes)
        },
        S2cPlanes = 0xfc17e => |x| {
            let v = var::VIDEO.get(x.b) & !0xc;
            var::VIDEO.set(x.b, v);
            x.wait(1, Step::S2cCleared)
        },
        S2cCleared = 0xfc18c => |x| {
            clear_layers(x, &[Layer::Three, Layer::One]);
            Obj(BLOCK).set_u16(x.b, f::TIMER, 4);
            let o = Obj(link(x));
            o.set_u32(x.b, Obj::STATE, 0x0600_0000);
            let n = var::COUNTER.get(x.b).wrapping_sub(1);
            var::COUNTER.set(x.b, n);
            var::SCREEN_ID.set(x.b, 0);
            sv::SCROLL_MODE.set(x.b, 0);
            let v = sv::VIDEO_FLAGS_LOW.get(x.b) & !1;
            sv::VIDEO_FLAGS_LOW.set(x.b, v);
            for v in [sv::CAMERA_X, sv::CAMERA_Y, Word(sv::SCROLL_1.0), Word(sv::SCROLL_2.0)] {
                v.set(x.b, 0);
            }
            s2c_wait_timer(x)
        },
        S2cTimer = 0xfc1d8 => |x| {
            if !timer_tick(x) {
                return s2c_wait_timer(x);
            }
            load_palette_page(x.b, 2, 84);
            load_palette_page(x.b, 3, 84);
            fading::PAGES.set(x.b, 12);
            crate::services::palette::no_brightness(x.b);
            crate::services::sound::service(x.b, crate::services::sound::Service::Ff07 { lock: 0, line: 4 });
            // present players and players "ready to leave" (byte +0x192)
            let mut present = 0u8;
            let mut ready = 0u8;
            let count = sv::CABINET.get(x.b) as u32 + 1;
            for k in 0..=count {
                let fp = Obj(crate::world::ram::PLAYERS.nth(k).0);
                if fp.u8(x.b, Player::ACTIVE) != 0 && (fp.u8(x.b, Player::MAIN) as i8) < 4 {
                    present += 1;
                    if fp.u8(x.b, READY) != 0 {
                        ready += 1;
                    }
                }
            }
            if ready != 0 || present.wrapping_sub(2) == sv::CABINET.get(x.b) {
                if let Ok(set) = set_15_pure(x.b, 22) {
                    Obj(BLOCK).set_u16(x.b, f::LINK, set.0 as u16);
                    return render_flow(x);
                }
            }
            next_step(x);
            render_flow(x)
        },
        S2eOpened = 0xfc286 => |x| {
            next_sub_state(x);
            render_flow(x)
        },
        S2eClosed = 0xfc2ae => |x| {
            next_sub_state(x);
            render_flow(x)
        },
        S30Wait = 0xfc366 => |x| {
            if (var::DIALOGUE_FRAMES.get(x.b) as i16) < 3 {
                return Flow::Done;
            }
            next_step(x);
            let mut d4 = 13u8;
            for counter in (0..3u32).rev() {
                Obj(BLOCK).set_u8(x.b, f::SIGNAL, 0);
                // Original glitch: the carry is not tested; a failure writes into the A1 left by the allocation
                let o = scenery_6(x.b, 0).unwrap_or_else(|a1| a1);
                o.set_u32(x.b, SIGNAL_POINTER, BLOCK);
                let v = sv::CAMERA_X.get(x.b);
                o.set_u16(x.b, Obj::X, v);
                let v = sv::CAMERA_Y.get(x.b);
                o.set_u16(x.b, Obj::Z, v);
                o.set_y(x.b, 0);
                o.set_u8(x.b, SCENERY_INDEX, d4);
                d4 = d4.wrapping_add(1);
                let idx = counter * 4;
                let dx = REWARD_OFFSETS.pc_word(x.b, idx);
                let dy = REWARD_OFFSETS.pc_word(x.b, idx + 2);
                let ox = o.u16(x.b, Obj::X).wrapping_add(dx);
                o.set_u16(x.b, Obj::X, ox);
                let oy = o.u16(x.b, Obj::Z).wrapping_add(dy);
                o.set_u16(x.b, Obj::Z, oy);
            }
            render(x);
            var::VIDEO.set(x.b, 0x12ce);
            Obj(BLOCK).set_u16(x.b, f::TIMER, 240);
            fade_settings(x, 3, FADE_ALL);
            crate::core::kernel::create_task(x.b, 0x10, crate::screens::scene::fading::RISE);
            Flow::Done
        },
        S32Faded = 0xfc450 => |x| {
            unlink_and_clear_lists(x);
            reset_pools(x);
            crate::services::video::clear_objects(x.b);
            x.wait(1, Step::S32Cleared)
        },
        S32Cleared = 0xfc470 => |x| {
            clear_layers(x, &[Layer::One, Layer::Two, Layer::Three]);
            // the score (BCD long at +0x110) gets the reward (+0x114, low nibble dropped), capped at 9 999 999
            for k in 0..4u32 {
                let p = Obj(crate::world::ram::PLAYERS.nth(k).0);
                if p.u8(x.b, Player::ACTIVE) == 0 {
                    continue;
                }
                p.set_u32(x.b, Player::MAIN, 0x0600_0000);
                let reward = p.u32(x.b, MONEY) & 0xffff_fff0;
                p.set_u32(x.b, MONEY, reward);
                let score = bcd_add(p.u32(x.b, Player::SCORE_BCD), reward);
                p.set_u32(x.b, Player::SCORE_BCD, if score as i32 > 0x0999_9999 { 0x0999_9999 } else { score });
            }
            crate::core::game::update_players(x.b);
            var::PHASE.set(x.b, 20);
            crate::services::sound::service(x.b, crate::services::sound::Service::GeneralStop);
            // `jmp 0x133f6`: the scored startup done, then the return to the caller
            crate::actors::families::small::startup_done(x.b, true);
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
    let s = var::STEP.get(x.b).wrapping_add(2);
    var::STEP.set(x.b, s);
}

/// `0xfc4ea`: terrain (SET, SOBJ) and sprites; A0 is preserved.
fn render(x: &mut Ctx) {
    crate::core::dispatch::SET.update_no_next(x.b);
    crate::core::dispatch::SOBJ.update_no_next(x.b);
    crate::services::sprites::build_in_game(x.b);
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
    kinds[0] = crate::core::dispatch::SET.update_last_kind(x.b);
    kinds[1] = crate::core::dispatch::EFFCT.update_last_kind(x.b);
    crate::services::scroll::update_cameras_in_game(x.b);
    kinds[2] = crate::core::dispatch::SOBJ.update_last_kind(x.b);
    crate::services::sprites::build_in_game(x.b);
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
    crate::world::objects::lists::unlink_all(x.b);
    crate::services::sprites::clear_lists(x.b);
}

/// The ten pools reset (0x2062).
fn reset_pools(x: &mut Ctx) {
    for pool in pools::POOLS {
        pool.reset(x.b);
    }
}

fn timer_tick(x: &mut Ctx) -> bool {
    let left = Obj(BLOCK).u16(x.b, f::TIMER).wrapping_sub(1);
    Obj(BLOCK).set_u16(x.b, f::TIMER, left);
    left == 0
}

/// Index of a scenery object (`+0x2f`), and its pointer to the signal byte of the block (`+0x4c`).
const SCENERY_INDEX: u32 = 0x2f;
const SIGNAL_POINTER: u32 = 0x4c;
/// Player fields: money (BCD long, the reward), ready to leave.
const MONEY: u32 = 0x114;
const READY: u32 = 0x192;

/// Next sub-state (`+5`) of the dialogue object of the block.
fn next_sub_state(x: &mut Ctx) {
    let o = Obj(link(x));
    let v = o.u8(x.b, SUB_STATE).wrapping_add(2);
    o.set_u8(x.b, SUB_STATE, v);
}
const SUB_STATE: u32 = 0x05;

fn link(x: &mut Ctx) -> u32 {
    Obj(BLOCK).u16(x.b, f::LINK) as i16 as i32 as u32 & 0xff_ffff
}

fn fade_settings(x: &mut Ctx, delay: u16, pages: u8) {
    fading::DELAY.set(x.b, delay);
    fading::PAGES.set(x.b, pages);
}

/// Creates the entry fade task `entry` (slot 0x10).
fn fade_task(x: &mut Ctx, task: crate::core::tasks::Task) {
    crate::core::kernel::create_task(x.b, 0x10, task);
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
