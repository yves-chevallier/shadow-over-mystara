//! Prompts task (`0xe03fe`, slot 0xd0): the text "INSERT COIN(S)", "CONTINUE", etc. blinks,
//! displayed 40 frames then erased 20 frames, as long as the mode machine does not stop it.
//!
//! The text depends on the situation: free game (`A5+0x25`) or setting `A5+0x457` (text printed by `0xd5e`,
//! erased by `0xd32`), otherwise one text per coin setting (`A5+0x1e`, table of longs `0xe048c`; printed by
//! `0xf38`, erased by `0xf4a`). The displayed text is kept at `A5-0x5ac2`: `0xe03c6` erases it when stopping the task,
//! `0xe03e4` launches it.

use crate::world::tasks::{cede, Wait};
use crate::hw::mem::{rd32, wr32, A5};
use crate::world::tasks::Flow;
use crate::world::tasks::Ctx;
use crate::Native;
pub use crate::world::tasks::stages::Prompt as Step;

/// Displayed text (long).
const SHOWN: u32 = A5 - 0x5ac2;
/// Slot of the task.
const SLOT: u32 = 0xd0;

mod var {
    pub const FREE_PLAY: u32 = 0x25;
    pub const SETTING: u32 = 0x457;
    pub const COINAGE: u32 = 0x1e;
}

/// Texts: free game, setting `A5+0x457` (the texts per coin setting are the table `prompt/coin_texts`, 0xe048c, longs,
/// the first for setting 1).
mod text {
    pub const FREE_PLAY: u32 = 0xe04b0;
    pub const SETTING: u32 = 0xe04e4;
}

/// Prints and erasures of the kept text: blocks (0xf38, 0xf4a) or simple text (0xd5e, 0xd32). The blocks only
/// read the object A0 for negative attributes: never for these texts (attribute 3).
#[derive(Clone, Copy)]
enum Print {
    Pieces,
    ClearCoins,
    Simple,
    ClearSimple,
}


const SHOW_FRAMES: u32 = 40;
const HIDE_FRAMES: u32 = 20;

crate::steps! {
    impl Step {
        Entry = 0xe03fe => entry,
        CoinsShown = 0xe0440 => |x| {
            print_shown(x, Print::ClearCoins);
            cede(x, Wait::Images(HIDE_FRAMES), Step::CoinsHidden)
        },
        CoinsHidden = 0xe0452 => |x| show(x, Print::Pieces, Step::CoinsShown),
        PlainShown = 0xe0478 => |x| {
            print_shown(x, Print::ClearSimple);
            cede(x, Wait::Images(HIDE_FRAMES), Step::PlainHidden)
        },
        PlainHidden = 0xe048a => |x| show(x, Print::Simple, Step::PlainShown),
    }
}

fn entry(x: &mut Ctx) -> Flow {
    if x.v8(var::FREE_PLAY) != 0 {
        wr32(x.b, SHOWN, text::FREE_PLAY);
        return show(x, Print::Pieces, Step::CoinsShown);
    }
    if x.v8(var::SETTING) != 0 {
        wr32(x.b, SHOWN, text::SETTING);
        return show(x, Print::Simple, Step::PlainShown);
    }
    let k = 4 * x.v8(var::COINAGE) as u32;
    let t = crate::world::tables::engine_table!("prompt/coin_texts").at32(x.b, k.wrapping_sub(4));
    wr32(x.b, SHOWN, t);
    show(x, Print::Pieces, Step::CoinsShown)
}

/// Prints the kept text, then the display wait.
fn show(x: &mut Ctx, print: Print, resume: Step) -> Flow {
    print_shown(x, print);
    cede(x, Wait::Images(SHOW_FRAMES), resume)
}
fn print_shown(x: &mut Ctx, print: Print) {
    let text = rd32(x.b, SHOWN);
    match print {
        Print::Pieces => crate::services::text::draw_blocks(x.b, text, 0),
        Print::ClearCoins => crate::services::text::clear_blocks(x.b, text, 0),
        Print::Simple => crate::services::text::print(x.b, text),
        Print::ClearSimple => crate::services::text::erase(x.b, text),
    };
}

/// `0xe03c6`: erases the text (like the coin text, 0xf4a) and stops the task (0xa14); no register
/// changes. The erasure only reads the caller's object A0 if the text attributes are negative: never for
/// the texts the task keeps (attribute 3, tables 0xe048c, 0xe04b0, 0xe04e4).
pub fn stop(b: &mut crate::world::World) {
    let text = rd32(b, SHOWN);
    crate::services::text::clear_blocks(b, text, 0);
    crate::world::tasks::traps::kill_task(b, SLOT);
}

/// `0xe03e4`: launches the task; registers kept.
pub fn launch(b: &mut crate::world::World) {
    crate::world::tasks::traps::create_task(b, SLOT, crate::world::tasks::Task::Prompt);
}

pub const STOP: Native = Native {
    entry: 0xe03c6,
    name: "prompt_stop",
    run: crate::Body::Memory(stop),
    touches_sound: false,
    outputs: 0,
};
pub const START: Native = Native {
    entry: 0xe03e4,
    name: "prompt_start",
    run: crate::Body::Memory(launch),
    touches_sound: false,
    outputs: 0,
};

pub const NATIVES: &[Native] = &[STOP, START];
