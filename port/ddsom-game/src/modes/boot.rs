//! Startup task (`0x11b16`): game initialisation, two warning screens depending on the region,
//! creation of the credit and freeze tasks, then the mode machine.
//!
//! Each warning stays on screen two or three seconds; a button (bit 4 of port `IN1`) speeds up
//! the countdown. The text depends on the region (program byte 0xc4 and setting `A5+0x70`).

use super::{Flow, Sequence};
use crate::video::Layer;
use crate::mem::{rd16, rd32, rd8, wr16, wr32, wr8, A5};
use crate::scene::Ctx;
use crate::Native;

/// Tasks created at the end of startup: credits and freeze.
mod task {
    pub const CREDITS: u32 = 0x13c0a;
    pub const FREEZE: u32 = 0xee080;
}

/// Starting palette: eight longs from the ROM copied into the palette table.
const START_PALETTE_SRC: u32 = 0x133ba;
const START_PALETTE_DST: u32 = 0x91c580;
/// Game region (program byte) and text tables (one per region, indexed by `A5+0x70`).
const REGION_BYTE: u32 = 0xc4;
const WARNING_TABLES: [u32; 3] = [0x11cba, 0x11cd2, 0x11cea];
const NOTICE_TABLE: u32 = 0x11d02;
/// Button port (bit 4: a pressed button speeds up the countdown).
const PORT_IN1: u32 = 0x804001;
/// Countdown of the warning screens: word at FF0E6E.
const COUNTDOWN: u32 = A5 - 0x7192;
const WARNING_FRAMES: u16 = 0x78;
const NOTICE_FRAMES: u16 = 0xb4;
const CPS_REG_VIDEO: u32 = 0x804122;
const CPS_REG_VIDEO_B: u32 = 0x804166;
const SLOT_CREDITS: u32 = 0x20;
const SLOT_FREEZE: u32 = 0xe0;

const INIT: u32 = 0x11b22;
crate::steps! {
    /// Steps (resume points) of the state machine.
    pub enum Step {
        /// Task entry (`0x11b16`): black screen, then initialisation.
        Entry = 0x11b16 => |x| x.subroutine(crate::video::black_screen::start, Step::Init),
        Init = INIT => |x| {
            crate::palette::load_all_palettes(x.b);
            for i in 0..8 {
                let v = rd32(x.b, START_PALETTE_SRC + 4 * i);
                wr32(x.b, START_PALETTE_DST + 4 * i, v);
            }
            wr8(x.b, A5 + 0x48e, 0xf);
            crate::palette::full_brightness(x.b);
            if !crate::config::initialize(x.c, x.b) {
                return Flow::Yield;
            }
            crate::credits::initialize(x.b);
            crate::events::clear_points(x.b);
            crate::events::clear_counter(x.b);
            crate::events::clear_popups(x.b);
            crate::sound::clear_private_stack(x.b);
            crate::player::init::clear_all_players(x.c, x.b);
            crate::player::init::clear_sub_objects(x.c, x.b);
            clear_mode_state(x);
            crate::rng::seed(x.b);
            default_high_scores(x);
            crate::video::copy_fixed_screens(x.b);
            crate::families::items::clear_picked_up(x.b);
            crate::tasks::game::clear_variables(x.c, x.b);
            wr8(x.b, A5 - 0x713c, 0);
            wr16(x.b, CPS_REG_VIDEO, 0xe);
            wr16(x.b, A5 + 0x12, 0xe);
            wr16(x.b, CPS_REG_VIDEO_B, 0x12ce);
            wr16(x.b, A5 + 0x14, 0x12ce);
            let region = rd8(x.b, REGION_BYTE);
            let table = match region {
                0 => WARNING_TABLES[0],
                1 => WARNING_TABLES[1],
                _ => WARNING_TABLES[2],
            };
            print_region_text(x, table);
            wr16(x.b, COUNTDOWN, WARNING_FRAMES);
            x.wait(1, Step::WarningTick)
        },
        WarningTick = WARNING_TICK => |x| {
            if countdown_tick(x) {
                return x.wait(1, Step::WarningTick);
            }
            crate::video::clear_layer(x.b, Layer::One);
            print_region_text(x, NOTICE_TABLE);
            wr16(x.b, COUNTDOWN, NOTICE_FRAMES);
            x.wait(1, Step::NoticeTick)
        },
        NoticeTick = NOTICE_TICK => |x| {
            if countdown_tick(x) {
                return x.wait(1, Step::NoticeTick);
            }
            crate::video::clear_layer(x.b, Layer::One);
            wr16(x.b, A5 + 0x7fc6, 0);
            if rd8(x.b, A5 + 4) & 1 != 0 {
                wr16(x.b, A5 + 0x7fc6, 1);
            }
            crate::eeprom::initialize_accounting(x.b);
            if !crate::config::initialize(x.c, x.b) {
                return Flow::Yield;
            }
            crate::kernel::create_task(x.b, SLOT_CREDITS, task::CREDITS);
            crate::kernel::create_task(x.b, SLOT_FREEZE, task::FREEZE);
            // the task continues in the mode machine loop (`jmp 0x133fe`)
            crate::modes::start(x)
        },
    }
}

/// Mode machine state (16 longs at `A5-0x7190`) and `A5-0x54f0..` (4 longs) cleared (`0x133dc`, then
/// `0xfc562`).
fn clear_mode_state(x: &mut Ctx) {
    const MODES: u32 = A5 - 0x7190;
    const SCENE_SCRIPT: u32 = A5 - 0x54f0;
    for i in 0..16 {
        wr32(x.b, MODES + 4 * i, 0);
    }
    for i in 0..4 {
        wr32(x.b, SCENE_SCRIPT + 4 * i, 0);
    }
}

/// Default high-score table (`0x18082`): ten 14-byte entries copied from ROM to
/// FF0FD0; another table for the alternate-palette version (`A5+0x70`).
fn default_high_scores(x: &mut Ctx) {
    const TABLES: [u32; 2] = [0x18f1e, 0x18faa];
    const SCORES: u32 = 0xff0fd0;
    const SIZE: u32 = 10 * 14;
    let src = TABLES[(rd16(x.b, A5 + 0x70) != 0) as usize];
    for i in (0..SIZE).step_by(2) {
        let w = rd16(x.b, src + i);
        wr16(x.b, SCORES + i, w);
    }
}

/// Displays the text of the current region: word `A5+0x70` (modulo 16) indexes the pointer table.
fn print_region_text(x: &mut Ctx, table: u32) {
    let region = (rd16(x.b, A5 + 0x70) & 0xf) as u32;
    let text = rd32(x.b, table + 2 * region);
    crate::text::print(x.b, text);
}

/// One countdown turn: a pressed button shortens it by nine frames.
fn countdown_tick(x: &mut Ctx) -> bool {
    let mut left = rd16(x.b, COUNTDOWN) as i16;
    if rd8(x.b, PORT_IN1) & 0x10 == 0 {
        left = left.wrapping_sub(8);
    }
    left = left.wrapping_sub(1);
    wr16(x.b, COUNTDOWN, left as u16);
    left > 0
}

const WARNING_TICK: u32 = 0x11c06;

const NOTICE_TICK: u32 = 0x11c4a;

pub const NATIVES: &[Native] = &[];
