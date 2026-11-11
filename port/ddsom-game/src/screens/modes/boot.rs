//! Startup task (`0x11b16`): game initialisation, two warning screens depending on the region,
//! creation of the credit and freeze tasks, then the mode machine.
//!
//! Each warning stays on screen two or three seconds; a button (bit 4 of port `IN1`) speeds up
//! the countdown. The text depends on the region (program byte 0xc4 and setting `A5+0x70`).

use super::{Flow, Sequence};
use crate::services::video::Layer;
use crate::hw::io::Port;
use crate::hw::video::{palette, regs};
use crate::world::ram::{Byte, Long, Word};
use crate::screens::scene::fading;
use crate::screens::tables::Rom;
use crate::screens::vars as sv;
use crate::screens::scene::Ctx;
use crate::Native;

/// Tasks created at the end of startup: credits and freeze.
mod task {
    pub const CREDITS: crate::core::tasks::Task = crate::core::tasks::Task::Credits;
    pub const FREEZE: crate::core::tasks::Task = crate::core::tasks::Task::Freeze;
}

/// Starting palette: eight longs from the ROM copied into the palette table.
const START_PALETTE_SRC: Rom = Rom::at(0x133ba);
const START_PALETTE_DST: crate::hw::video::Vram = palette(1, 0xc0);
/// Game region (program byte, data space) and text tables (one per region, indexed by `A5+0x70`).
const REGION_BYTE: Rom = Rom::at(0xc4);
const WARNING_TABLES: [Rom; 3] = [Rom::at(0x11cba), Rom::at(0x11cd2), Rom::at(0x11cea)];
const NOTICE_TABLE: Rom = Rom::at(0x11d02);
/// Button port (low byte of `IN0`; bit 4: a pressed button speeds up the countdown).
const IN0_LOW: Port = Port(0x804001);
/// Coin sound flag (`A5-0x713c`), and the switch read at the end of startup (`A5+0x7fc6`, h).
const COIN_SOUND: Byte = Byte(0xff_0ec4);
const STARTUP_SWITCH: Word = Word::a5(0x7fc6);
/// Countdown of the warning screens: word at FF0E6E.
const COUNTDOWN: Word = Word(0xff_0e6e);
const WARNING_FRAMES: u16 = 0x78;
const NOTICE_FRAMES: u16 = 0xb4;
const SLOT_CREDITS: u32 = 0x20;
const SLOT_FREEZE: u32 = 0xe0;

crate::steps! {
    /// Steps (resume points) of the state machine.
    pub enum Step {
        /// Task entry (`0x11b16`): black screen, then initialisation.
        Entry = 0x11b16 => |x| x.subroutine(crate::services::video::black_screen::start, Step::Init),
        Init = 0x11b22 => |x| {
            crate::services::palette::load_all_palettes(x.b);
            for i in 0..8 {
                let v = START_PALETTE_SRC.long(x.b, 4 * i);
                START_PALETTE_DST.offset(4 * i).set_long(x.b, v);
            }
            fading::PAGES.set(x.b, 0xf);
            crate::services::palette::full_brightness(x.b);
            if !crate::core::config::initialize(x.b) {
                return Flow::Yield;
            }
            crate::screens::credits::initialize(x.b);
            crate::world::events::clear_points(x.b);
            crate::world::events::clear_counter(x.b);
            crate::world::events::clear_popups(x.b);
            crate::hw::sound::clear_private_stack(x.b);
            crate::actors::player::init::clear_all_players(x.b);
            crate::actors::player::init::clear_sub_objects(x.b);
            clear_mode_state(x);
            crate::world::rng::seed(x.b);
            default_high_scores(x);
            crate::services::video::copy_fixed_screens(x.b);
            crate::actors::families::items::clear_picked_up(x.b);
            crate::core::tasks::game::clear_variables(x.b);
            COIN_SOUND.set(x.b, 0);
            regs::VIDEO_CONTROL.set(x.b, 0xe);
            sv::VIDEO_FLAGS.set(x.b, 0xe);
            regs::LAYER_CONTROL.set(x.b, 0x12ce);
            sv::LAYERS.set(x.b, 0x12ce);
            let region = REGION_BYTE.byte(x.b, 0);
            let table = match region {
                0 => WARNING_TABLES[0],
                1 => WARNING_TABLES[1],
                _ => WARNING_TABLES[2],
            };
            print_region_text(x, table);
            COUNTDOWN.set(x.b, WARNING_FRAMES);
            x.wait(1, Step::WarningTick)
        },
        WarningTick = 0x11c06 => |x| {
            if countdown_tick(x) {
                return x.wait(1, Step::WarningTick);
            }
            crate::services::video::clear_layer(x.b, Layer::One);
            print_region_text(x, NOTICE_TABLE);
            COUNTDOWN.set(x.b, NOTICE_FRAMES);
            x.wait(1, Step::NoticeTick)
        },
        NoticeTick = 0x11c4a => |x| {
            if countdown_tick(x) {
                return x.wait(1, Step::NoticeTick);
            }
            crate::services::video::clear_layer(x.b, Layer::One);
            STARTUP_SWITCH.set(x.b, 0);
            if crate::screens::hud::var::START_NOW.get(x.b) & 1 != 0 {
                STARTUP_SWITCH.set(x.b, 1);
            }
            crate::hw::eeprom::initialize_accounting(x.b);
            if !crate::core::config::initialize(x.b) {
                return Flow::Yield;
            }
            crate::core::kernel::create_task(x.b, SLOT_CREDITS, task::CREDITS);
            crate::core::kernel::create_task(x.b, SLOT_FREEZE, task::FREEZE);
            // the task continues in the mode machine loop (`jmp 0x133fe`)
            crate::screens::modes::start(x)
        },
    }
}

/// Mode machine state (16 longs at `A5-0x7190`) and `A5-0x54f0..` (4 longs) cleared (`0x133dc`, then
/// `0xfc562`).
fn clear_mode_state(x: &mut Ctx) {
    for i in 0..16 {
        Long(super::mode::BASE + 4 * i).set(x.b, 0);
    }
    for i in 0..4 {
        Long(super::ranking::STATE + 4 * i).set(x.b, 0);
    }
}

/// Default high-score table (`0x18082`): ten 14-byte entries copied from ROM to
/// FF0FD0; another table for the alternate-palette version (`A5+0x70`).
fn default_high_scores(x: &mut Ctx) {
    const TABLES: [Rom; 2] = [Rom::at(0x18f1e), Rom::at(0x18faa)];
    const SCORES: u32 = 0xff0fd0;
    const SIZE: u32 = 10 * 14;
    let src = TABLES[(crate::screens::hud::var::LANGUAGE.get(x.b) != 0) as usize];
    for i in (0..SIZE).step_by(2) {
        let w = src.word(x.b, i);
        Word(SCORES + i).set(x.b, w);
    }
}

/// Displays the text of the current region: word `A5+0x70` (modulo 16) indexes the pointer table.
fn print_region_text(x: &mut Ctx, table: Rom) {
    let region = (crate::screens::hud::var::LANGUAGE.get(x.b) & 0xf) as u32;
    let text = table.long(x.b, 2 * region);
    crate::services::text::print(x.b, text);
}

/// One countdown turn: a pressed button shortens it by nine frames.
fn countdown_tick(x: &mut Ctx) -> bool {
    let mut left = COUNTDOWN.get(x.b) as i16;
    if IN0_LOW.get(x.b) & 0x10 == 0 {
        left = left.wrapping_sub(8);
    }
    left = left.wrapping_sub(1);
    COUNTDOWN.set(x.b, left as u16);
    left > 0
}



pub const NATIVES: &[Native] = &[];
