//! Shared state `0xf792e` of the scene exits: the shop between two levels.
//!
//! Block `A5-0x5560` (A6 in the original): `+0` state (step of 2), `+1` exit sub-state, `+2` sub-sub-state,
//! `+4` the players' objects (four words), `+0xc` shop number (per-scene table `0xf7ad0`), `+0xd` number
//! of players in the shop, `+0xe` timer, `+0x10` return scene, `+0x11` shop already open, `+0x12`
//! pending message, `+0x14` message to open, `+0x16` open message (word), `+0x18` current message,
//! `+0x1a` exit message list (long).
//!
//! The shop is a scene (69 + number): its SOBJ (type 13) and one EFFCT per player (type 20) make the game;
//! this state opens the scene, chains the messages, then returns to the return scene (or to the scene set by
//! `0xf7d5e`, or starts a shop over if `0xf7d72` asks for it).

use super::Ctx;
use crate::video::{clear_layer, Layer};
use crate::mem::{prog32, prog8, rd16, rd32, rd8, wr16, wr32, wr8, A5};
use crate::modes::{Flow, Sequence};
use crate::Native;
use crate::objects::{pools, Obj};

/// Scene exit block: the original's A0, set by the dispatch and kept across the yields.
const EXIT_BLOCK: u32 = super::screen::blocks::EXIT;

/// Shop block.
pub const BLOCK: u32 = A5 - 0x5560;
mod f {
    pub const STATE: u32 = 0;
    pub const EXIT_STEP: u32 = 1;
    pub const EXIT_SUB: u32 = 2;
    pub const PLAYER_OBJECTS: u32 = 4;
    pub const SHOP: u32 = 0xc;
    pub const SHOPPERS: u32 = 0xd;
    pub const TIMER: u32 = 0xe;
    pub const RETURN_SCENE: u32 = 0x10;
    pub const REOPENED: u32 = 0x11;
    pub const PENDING: u32 = 0x12;
    pub const NEXT_MESSAGE: u32 = 0x14;
    pub const MESSAGE: u32 = 0x16;
    pub const TALKING: u32 = 0x18;
    pub const EXIT_MESSAGES: u32 = 0x1a;
}

/// Tables (`data`: read as data; `pc`: PC-relative).
mod rom {
    /// Shop number per scene (data).
    pub const SHOP_OF_SCENE: u32 = 0xf7ad0;
    /// Shop music: offset (data) into a routine table (data).
    pub const MUSIC_INDEX: u32 = 0xf7ab0;
    pub const MUSIC: u32 = 0xf7ac4;
    /// Welcome sound: index (data, 20 shops) into a routine table (data).
    pub const WELCOME_INDEX: u32 = 0xf7b88;
    pub const WELCOME: u32 = 0xf7b9c;
    pub const WELCOME_SHOPS: u8 = 20;
    /// Exit message lists: index (pc) into a pointer table (pc).
    pub const EXIT_LIST_INDEX: u32 = 0xf7c14;
    pub const EXIT_LISTS: u32 = 0xf7c28;
    /// Shop followed by another (data); scene imposed after the shop (data, 0: the return scene).
    pub const CHAINED: u32 = 0xf7d72;
    pub const NEXT_SCENE: u32 = 0xf7d5e;
    /// Player portraits: 8 bytes per character (address, tile, attributes), depending on the layout.
    pub const PORTRAITS_WIDE: u32 = 0xf7eb2;
    pub const PORTRAITS: u32 = 0xf7e92;
}

mod sub {
    /// Error path of the original (`tst.w 1(A5)`: address error) when a pool is full.
    pub const FULL: u32 = 0xf7dfa;
}

/// Portrait task (created when the shop opens).
const PORTRAITS_TASK: u32 = 0xf7dfe;

mod at {
    pub const OPEN_FADED: u32 = 0xf7960;
    pub const LEAVE_FADED: u32 = 0xf7c76;
    pub const LEAVE_WAITED: u32 = 0xf7c96;
    pub const CHAIN_FADED: u32 = 0xf7cfe;
    pub const REOPEN_WAITED: u32 = 0xf7d34;
}

fn b8(x: &mut Ctx, off: u32) -> u8 {
    rd8(x.b, BLOCK + off)
}
fn set_b8(x: &mut Ctx, off: u32, v: u8) {
    wr8(x.b, BLOCK + off, v)
}
fn b16(x: &mut Ctx, off: u32) -> u16 {
    rd16(x.b, BLOCK + off)
}
fn set_b16(x: &mut Ctx, off: u32, v: u16) {
    wr16(x.b, BLOCK + off, v)
}
fn add_b8(x: &mut Ctx, off: u32, v: u8) {
    let n = b8(x, off).wrapping_add(v);
    set_b8(x, off, n);
}

/// A state of 0xf792e. A6 designates the block throughout the state, as in the original.
pub fn run(x: &mut Ctx) -> Flow {
    let s = b8(x, f::STATE);
    match s {
        0 => open(x),
        2 => {
            let t = b16(x, f::TIMER).wrapping_sub(1);
            set_b16(x, f::TIMER, t);
            if (t as i16) < 0 {
                add_b8(x, f::STATE, 2);
            }
            Flow::Done
        }
        4 => welcome(x),
        6 => {
            if (b8(x, f::SHOPPERS) as i8) > 0 {
                messages(x);
                return Flow::Done;
            }
            set_b16(x, f::STATE, 0x800);
            // D0 still holds the state (6), set for the dispatch: it serves as the row of the delay table
            crate::sound::service(x.b, crate::sound::Service::Ff07 { lock: 0, line: 6 });
            Flow::Done
        }
        8 => match b8(x, f::EXIT_STEP) {
            0 => {
                exit_messages(x);
                Flow::Done
            }
            4 => leave(x),
            _ => Flow::Done,
        },
        _ => Flow::Done,
    }
}

/// State 0: fade to black (delay 2) before opening the shop.
fn open(x: &mut Ctx) -> Flow {
    x.set_v8(0x42, 2);
    x.set_v16(0x490, 2);
    x.set_v8(0x48e, 15);
    x.subroutine(crate::tasks::fading::descent, Step::OpenFaded)
}

crate::steps! {
    /// Steps (resume points) of the shop, and the portrait task.
    pub enum Step {
        /// Entry of the portrait task.
        Portraits = PORTRAITS_TASK => portraits,
        /// Opening: shop number, portrait task, music, shop and player objects, then the
        /// shop scene (69 + number) is loaded and its spawn script restarted.
        OpenFaded = at::OPEN_FADED => |x| {
            crate::objects::lists::unlink_all(x.b);
            for pool in crate::objects::pools::POOLS {
                pool.reset(x.b);
            }
            let scene = x.v8(0x34) as u32;
            let shop = rd8(x.b, rom::SHOP_OF_SCENE + scene);
            set_b8(x, f::SHOP, shop);
            crate::kernel::create_task(x.b, 192, PORTRAITS_TASK);
            x.set_v8(0x4e, (scene == 5) as u8);
            if b8(x, f::REOPENED) == 0 {
                set_b8(x, f::RETURN_SCENE, scene as u8);
            }
            let index = rd8(x.b, rom::MUSIC_INDEX + shop as u32) as u32;
            let music = rd32(x.b, rom::MUSIC + index);
            crate::sound::play_global(x.b, music);
            set_b16(x, f::STATE, 0x200);
            set_b8(x, f::SHOPPERS, 0);
            set_b8(x, f::TALKING, 0);
            set_b16(x, f::PENDING, 0);
            set_b16(x, f::NEXT_MESSAGE, 0);
            set_b16(x, f::TIMER, 1);
            x.set_v8(0x3c, 4);
            set_b16(x, 0x1e, 0);
            set_b16(x, 0x20, 0);
            x.set_v8(0x3d, 0);
            clear_layer(x.b, Layer::One);
            crate::hud::forget_displayed(x.b);
            // pools emptied just before: the failure (address error of the original) does not happen (code-mort.md)
            let Some(Obj(o)) = pools::SOBJ.alloc(x.b) else { crate::families::effects::address_error(sub::FULL) };
            wr8(x.b, o, 1);
            wr8(x.b, o + 2, 13);
            for k in 0..4u32 {
                let p = super::PLAYERS + k * super::PLAYER_STRIDE;
                let slot = BLOCK + f::PLAYER_OBJECTS + 2 * k;
                wr16(x.b, slot, 0);
                if rd8(x.b, p) == 0 || rd16(x.b, p + 0x62) == 0 {
                    continue;
                }
                let Some(Obj(e)) = pools::EFFCT.alloc(x.b) else { crate::families::effects::address_error(sub::FULL) };
                wr8(x.b, e, 1);
                wr8(x.b, e + 2, 20);
                let (variant, character) = (rd8(x.b, p + 3), rd8(x.b, p + 2));
                wr8(x.b, e + 3, variant);
                wr8(x.b, e + 0x2f, character);
                wr16(x.b, e + 0x50, p as u16);
                wr16(x.b, slot, e as u16);
                add_b8(x, f::SHOPPERS, 1);
            }
            let shop_scene = b8(x, f::SHOP).wrapping_add(69);
            x.set_v8(0x34, shop_scene);
            crate::palette::load_all_palettes(x.b);
            x.set_v8(0x48e, 15);
            crate::palette::no_brightness(x.b);
            // 0x33aa6 (D7 and A0 kept): cameras at start and updated (D4 re-read: `update_cameras_in_game`)
            crate::scroll::cameras_start(x.b);
            crate::scroll::update_cameras_in_game(x.b);
            x.set_v16(0x14, 0);
            crate::video::turn_layers_on(x.b, 0x12c0);
            super::script::reset(x.b);
            super::script::step(x.b);
            Flow::Done
        },
        LeaveFaded = at::LEAVE_FADED => |x| {
            crate::video::clear_objects(x.b);
            crate::objects::lists::unlink_all(x.b);
            x.set_v8(0x3d, 1);
            crate::tasks::one_frame(x, Step::LeaveWaited)
        },
        /// Return from the shop: layers cleared, next state of the sequence, return scene.
        LeaveWaited = at::LEAVE_WAITED => |x| {
            clear_layers(x);
            let s = rd8(x.b, EXIT_BLOCK + 2).wrapping_add(4);
            wr8(x.b, EXIT_BLOCK + 2, s);
            x.set_v8(0x3c, 0xff);
            x.set_v8(0x59, 0);
            x.and16(0x12, 0xfffe);
            let layers = x.v16(0x12);
            wr16(x.b, 0x804122, layers);
            let shop = rd8(x.b, BLOCK + f::SHOP) as u32;
            let next = rd8(x.b, rom::NEXT_SCENE + shop);
            x.set_v8(0x34, next);
            if next == 0 {
                let back = rd8(x.b, BLOCK + f::RETURN_SCENE);
                x.set_v8(0x34, back);
            }
            // the scene exit dispatch re-reads byte `+2` of the block designated by A0 (the block, kept by the yield)
            super::exit::set_dispatch_a0(x.b, EXIT_BLOCK);
            Flow::Done
        },
        ChainFaded = at::CHAIN_FADED => |x| {
            x.set_v16(0x14, 0);
            crate::video::turn_layers_on(x.b, 0x12c0);
            Flow::Done
        },
        ReopenWaited = at::REOPEN_WAITED => |x| {
            crate::objects::pools::SET.reset(x.b);
            crate::objects::pools::SOBJ.reset(x.b);
            crate::objects::pools::PEFF.reset(x.b);
            crate::objects::pools::EFFCT.reset(x.b);
            clear_layers(x);
            Flow::Done
        },
    }
}

/// The three cleared layers.
fn clear_layers(x: &mut Ctx) {
    for layer in [Layer::One, Layer::Two, Layer::Three] {
        clear_layer(x.b, layer);
    }
}

/// State 4: opening fade (task 0x14ba, which leaves A0 on it), first message (101), welcome sound.
fn welcome(x: &mut Ctx) -> Flow {
    add_b8(x, f::STATE, 2);
    x.set_v16(0x14, 0);
    crate::video::turn_layers_on(x.b, 0x12ce);
    x.set_v16(0x490, 2);
    x.set_v8(0x48e, 15);
    crate::kernel::create_task(x.b, 16, crate::scene::fading::RISE);
    set_b16(x, f::PENDING, 101);
    let shop = b8(x, f::SHOP);
    if shop >= rom::WELCOME_SHOPS {
        // demonstration scenes outside the table: fix shared by the two machines (`exit::patch_shared_next`)
        return Flow::Done;
    }
    let index = rd8(x.b, rom::WELCOME_INDEX + shop as u32);
    if index == 0 {
        return Flow::Done;
    }
    let sound = rd32(x.b, rom::WELCOME + 4 * index as u32);
    crate::sound::play_global(x.b, sound);
    Flow::Done
}

/// Messages (0xf7d86): the pending message goes to the front; if none is open, it opens; an open message
/// that lasts a long time is cut short when another waits.
fn messages(x: &mut Ctx) {
    if b16(x, f::NEXT_MESSAGE) == 0 {
        let p = b16(x, f::PENDING);
        set_b16(x, f::NEXT_MESSAGE, p);
        set_b16(x, f::PENDING, 0);
    }
    if b8(x, f::TALKING) == 0 {
        let n = b16(x, f::NEXT_MESSAGE);
        if n == 0 {
            return;
        }
        let Some(message) = crate::families::creators::scene_set(x.b, n as u8) else {
            return;
        };
        set_b8(x, f::TALKING, 1);
        set_b16(x, f::MESSAGE, message.0 as u16);
        let p = b16(x, f::PENDING);
        set_b16(x, f::NEXT_MESSAGE, p);
        set_b16(x, f::PENDING, 0);
        return;
    }
    let m = rd16(x.b, BLOCK + f::MESSAGE) as i16 as i32 as u32 & 0xff_ffff;
    if rd8(x.b, m) == 0 || rd8(x.b, m + 0x22) != 0 {
        set_b8(x, f::TALKING, 0);
        return;
    }
    if b16(x, f::NEXT_MESSAGE) == 0 || (rd16(x.b, m + 0x32) as i16) <= 1 {
        return;
    }
    wr16(x.b, m + 0x32, 1);
}

/// Exit, step 0: exit messages from the shop's list, one after the other.
fn exit_messages(x: &mut Ctx) {
    messages(x);
    match b8(x, f::EXIT_SUB) {
        0 => {
            let shop = b8(x, f::SHOP) as u32;
            let index = prog8(x.b, rom::EXIT_LIST_INDEX + shop) as u32;
            let list = prog32(x.b, rom::EXIT_LISTS + 4 * index);
            wr32(x.b, BLOCK + f::EXIT_MESSAGES, list);
            if list == 0 {
                return exit_messages_done(x);
            }
            add_b8(x, f::EXIT_SUB, 4);
            next_exit_message(x);
        }
        4 => {
            if b8(x, f::TALKING) == 0 {
                next_exit_message(x);
            }
        }
        _ => {}
    }
}

fn next_exit_message(x: &mut Ctx) {
    let at = rd32(x.b, BLOCK + f::EXIT_MESSAGES);
    let n = rd16(x.b, at);
    if n == 0 {
        return exit_messages_done(x);
    }
    set_b16(x, f::PENDING, n);
    wr32(x.b, BLOCK + f::EXIT_MESSAGES, at + 2);
}

fn exit_messages_done(x: &mut Ctx) {
    add_b8(x, f::EXIT_STEP, 4);
    set_b8(x, f::EXIT_SUB, 0);
}

/// Exit, step 4: fade to black, then return to the return scene; or a chained shop.
fn leave(x: &mut Ctx) -> Flow {
    let shop = b8(x, f::SHOP) as u32;
    let chained = rd8(x.b, rom::CHAINED + shop);
    if chained == 0 {
        x.set_v16(0x490, 2);
        x.set_v8(0x48e, 15);
        return x.subroutine(crate::tasks::fading::descent, Step::LeaveFaded);
    }
    if b8(x, f::EXIT_SUB) == 0 {
        add_b8(x, f::EXIT_SUB, 2);
        x.set_v16(0x490, 2);
        x.set_v8(0x48e, 15);
        return x.subroutine(crate::tasks::fading::descent, Step::ChainFaded);
    }
    // next shop: the block restarts from state 0
    x.and16(0x12, 0xfffe);
    set_b8(x, f::REOPENED, 1);
    crate::video::clear_objects(x.b);
    wr32(x.b, BLOCK, 0);
    crate::objects::lists::unlink_all(x.b);
    crate::tasks::one_frame(x, Step::ReopenWaited)
}

/// Portrait task (0xf7dfe): for each player in the game, its portrait (two columns) and the frame that follows it;
/// then the task ends (`jmp 0x9f8`).
fn portraits(x: &mut Ctx) -> Flow {
    let b = &mut *x.b;
    let table = if rd8(b, A5 + 0x20) == 1 { rom::PORTRAITS_WIDE } else { rom::PORTRAITS };
    for k in 0..4 {
        let p = super::PLAYERS + k * super::PLAYER_STRIDE;
        if rd8(b, p) == 0 || (rd8(b, p + 4) as i8) >= 4 {
            continue;
        }
        let e = table + 8 * rd8(b, p + 2) as u32;
        let (at, tile, attr) = (rd32(b, e), rd16(b, e + 4), rd16(b, e + 6));
        super::tiles::glyph(b, at, tile, attr);
        super::tiles::glyph(b, at + 0x100, 0x1b2, attr);
    }
    crate::tasks::finish(x)
}

pub const NATIVES: &[Native] = &[
    
];
