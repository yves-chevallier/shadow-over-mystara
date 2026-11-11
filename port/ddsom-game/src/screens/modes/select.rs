//! Phase 4 (`0x1ca20`): character selection.
//!
//! After the opening bet, the game starts with this phase: each present player moves a
//! cursor over the twelve cells (six characters, two variants each), a thirty-second countdown
//! forces the choice, two players cannot take the same cell. When everyone has
//! confirmed, the phase moves to 8 (entry of the first scene).
//!
//! The phase is a state machine (`A5+0x2d`, step of 2) that takes one step per frame; only the
//! opening and closing steps cede control (fades), at original addresses taken up by
//! Rust steps (`steps!`).
//!
//! | Step | Role |
//! |---|---|
//! | 0 | selection screen: cursor objects (`ae776`), background, opening fade |
//! | 2 | player cursors, waiting queue, countdown (30 s) |
//! | 4 | wait of 20 frames before continuing |
//! | 6 | players frozen on their character, fade, waiting screen |
//! | 8 | background of the second screen (route choice), objects |
//! | 10 / 12 | wait for a button, blinking announcement |
//! | 14 | fondu final, phase 8 |

use super::{Flow, Sequence};
use crate::screens::scene::fading;
use crate::actors::dispatch;
use crate::actors::families::effects_creators::{banner, element_d_interface};
use crate::hw::video::{palette, SCROLL2, SCROLL3};
use crate::world::objects::player::Player;
use crate::world::ram::{Byte, Long, Word};
use crate::world::screen_tables::Rom;
use crate::world::objects::{pools, Obj};
use crate::screens::scene::{PLAYERS, PLAYER_STRIDE};
use crate::world::tasks::Ctx;
use crate::services::video::{load_grid, clear_layer, Layer, Grid};
pub use crate::world::tasks::stages::Select as Step;

/// Work RAM variables (absolute addresses).
mod var {
    use crate::world::ram::{Byte, Long, Word};
    /// Game phase, step (bytes), sub-step (word, and its high byte).
    pub const PHASE: Byte = Byte::a5(0x2c);
    pub const STEP: Byte = Byte::a5(0x2d);
    pub const SUB: Word = Word::a5(0x2e);
    pub const SUB_HIGH: Byte = Byte::a5(0x2e);
    /// Frozen players (!= 0); game active; buttons; mask of players in play.
    pub const PLAYERS_FROZEN: Byte = Byte::a5(0x4a);
    pub const GAME_ACTIVE: Byte = Byte::a5(0x30);
    pub const BUTTONS_A: Byte = Byte::a5(4);
    pub const BUTTONS_B: Byte = Byte::a5(0xa);
    pub const FRAME_COUNTER: Byte = Byte::a5(0x6f);
    pub const MODE_FLAG: Byte = Byte::a5(0x73);
    pub const SCREEN_ID: Byte = Byte::a5(0x3c);
    pub const VIDEO: Word = Word::a5(0x14);
    /// Rising edges of the buttons, one byte per player.
    pub const PRESSED: u32 = 0xff_800c;
    /// Fade from white: the four library palettes it starts from (ROM addresses, longs).
    pub const FADE_SOURCES: Long = Long::a5(0x492);
    /// Requested scrolls and cameras (see `vblank.rs`).
    pub const SCROLL_1: Long = Long::a5(0x4b6);
    pub const SCROLL_2: Long = Long::a5(0x4ba);
    pub const CAMERA: Word = Word::a5(0x506);
    pub const CAMERA_Y: Word = Word::a5(0x50a);
    pub const CAMERA_2: Word = Word::a5(0x556);
    pub const CAMERA_2_Y: Word = Word::a5(0x55a);
    /// Blinking announcement: text address (long), x and y.
    pub const BANNER: Long = Long::a5(0x5c6);
    pub const BANNER_X: Word = Word::a5(0x5ca);
    pub const BANNER_Y: Word = Word::a5(0x5cc);
    /// Waiting-screen flag (byte at FF0EAB).
    pub const SCREEN_WAIT: Byte = Byte(0xff_0eab);

    /// Countdown word (high byte: seconds, low byte: frames), cells taken (bits), list of choices.
    pub const COUNTDOWN: Word = Word(0xff_19fe);
    pub const COUNTDOWN_SECONDS: Byte = Byte(0xff_19fe);
    pub const COUNTDOWN_TICK: Byte = Byte(0xff_19ff);
    pub const TAKEN: Word = Word(0xff_1a00);
    pub const CHOICES: u32 = 0xff_1a02;
    pub const WORD_9A0E: Word = Word(0xff_1a0e);
    pub const LINK_9A10: Word = Word(0xff_1a10);
    pub const PLAYERS_PRESENT: Byte = Byte(0xff_1a12);
    pub const LOCKED_FLAG: Byte = Byte(0xff_1a13);
    pub const SLOT_LINKS: u32 = 0xff_19f4;
    pub const MAIN_LINK: Word = Word(0xff_19fc);
}

/// ROM tables: variant bytes per character (code), announcements, cursor moves (data). The
/// initialisation routines of the four players (0x1d0f6) are those of `player::init::initialize`.
mod table {
    use crate::world::screen_tables::Rom;
    pub const VARIANT: Rom = Rom::at(0x1d118);
    pub const BANNERS: Rom = Rom::at(0x1d3aa);
    pub const CURSOR_DOWN_DELTAS: Rom = Rom::at(0x1d6d2);
    pub const CURSOR_UP_DELTAS: Rom = Rom::at(0x1d6cc);
}

/// Fields of the interface elements of the screen (EFFCT objects `element_d_interface`): kind of element
/// (`+3`: 0 the main element, 1 a cursor, 2 a marker, 3 the counter, 4 a cell), slot (`+0x2f`), main element
/// (`+0x1e`, word), the choices published by the main element, the counter's seconds.
mod el {
    use crate::world::objects::Obj;
    pub const KIND: u32 = Obj::VARIANT;
    pub const SLOT: u32 = 0x2f;
    pub const MAIN: u32 = 0x1e;
    pub const CHOICES: u32 = 0x08;
    pub const TAKEN: u32 = 0x14;
    pub const LOCKED: u32 = 0x18;
    pub const SHOWN: u32 = 0x1c;
    pub const SECONDS: u32 = 0x14;
    pub const FRACTION: u32 = 0x15;
    /// Cursor: parity of the cell of its player.
    pub const PARITY: u32 = 0x16;
    /// Announcement banner: the variable it watches (long).
    pub const WATCHED: u32 = 0x58;
}

/// Player fields of the selection: the cursor (`+3`, the character, bit 7: confirmed), selected (`+0x192`).
const SELECTED: u32 = 0x192;
/// Selection family, and the setting of character 5 (`hud::player`).
const SELECTION_KIND: u32 = 0x15d;
const SETTING_16B: u32 = 0x16b;
/// Set when a player of character 5 starts (byte `+0x3f` of the record at `A5-0x5b88`, h).
const CHARACTER_5_FLAG: Byte = Byte(0xff_2478 + 0x3f);

/// Byte `i` of the list of choices (two places per character).
fn choice(i: u32) -> Byte {
    Byte(var::CHOICES + i)
}

/// Element of the interface linked to slot `k`.
fn slot_link(k: u32) -> Word {
    Word(var::SLOT_LINKS + 2 * k)
}

const SLOTS: usize = 12;
/// Library palettes the opening fade from white starts from.
const FADE_SOURCES: [Rom; 4] = [Rom::at(0x28ff00), Rom::at(0x25c0c0), Rom::at(0x279460), Rom::at(0x28c6e0)];
/// Where the blocks of layers 2 and 3 of the screens are loaded.
const LAYER2_BLOCKS: u32 = SCROLL2.0 + 0x3000;
const LAYER3_BLOCKS: u32 = SCROLL3.0 + 0x3800;
const COUNTDOWN_START: u16 = 0x1432;

/// Program-space word sign-extended (`movea.w` address).
fn sign_ext(v: u16) -> u32 {
    v as i16 as i32 as u32 & 0xff_ffff
}

fn player(k: u32) -> Obj {
    Obj(PLAYERS + k * PLAYER_STRIDE)
}

/// Dispatch of the objects of the given classes, then the sprite list (`jsr 0x39cba`... `jsr 0x1de96`).
fn render(x: &mut Ctx, classes: &[&dispatch::Class]) {
    for class in classes {
        class.update_no_next(x.b);
    }
    crate::services::sprites::build_in_game(x.b);
}

/// Two tile blocks (`n`, `n + 1`) one after the other from `destination`.
fn two_blocks(x: &mut Ctx, grid: Grid, n: u16, destination: u32) {
    let end = load_grid(x.b, grid, n, destination);
    load_grid(x.b, grid, n + 1, end.destination);
}

/// Phase 4 of the game (`0x1ca20`): the selection screen, one step per frame.
pub fn phase_4(x: &mut Ctx) -> Flow {
    let step = var::STEP.get(x.b);
    match step {
        0 => step_0(x),
        2 => step_2(x),
        4 => step_4(x),
        6 => step_6(x),
        8 => step_8(x),
        10 => step_a(x),
        12 => step_c(x),
        14 => step_e(x),
        _ => Flow::Done,
    }
}

// ---------------------------------------------------------------------------------------------
// Step 0: selection screen

fn step_0(x: &mut Ctx) -> Flow {
    var::SCREEN_WAIT.set(x.b, 0);
    let s = var::STEP.get(x.b).wrapping_add(2);
    var::STEP.set(x.b, s);
    var::SUB_HIGH.set(x.b, 0);
    crate::services::video::clear_objects(x.b);
    for pool in [&crate::world::objects::pools::EFFCT, &crate::world::objects::pools::SET, &crate::world::objects::pools::SOBJ] {
        pool.reset(x.b);
    }
    crate::services::sprites::clear_lists(x.b);
    var::SCROLL_1.set(x.b, 0);
    var::SCROLL_2.set(x.b, 0);
    // starting cursor of each player: one cell bit per player
    for (k, cell) in [0u8, 2, 4, 8].into_iter().enumerate() {
        player(k as u32).set_u8(x.b, Player::CHARACTER, cell);
    }
    for k in 0..4 {
        player(k).set_u8(x.b, SELECTED, 0);
    }
    for k in 0..4 {
        for half in [0, 4] {
            player(k).set_u32(x.b, crate::screens::hud::player::ITEMS + half, 0xffff_ffff);
        }
    }
    var::TAKEN.set(x.b, 0);
    var::WORD_9A0E.set(x.b, 0);
    var::LOCKED_FLAG.set(x.b, 0);
    var::CAMERA.set(x.b, 0x20);
    var::CAMERA_Y.set(x.b, 0x20);
    var::CAMERA_2_Y.set(x.b, 0x30);
    var::CAMERA_2.set(x.b, 0x10);
    var::CAMERA_2_Y.set(x.b, 0xfff8);
    for off in [0u32, 4, 8] {
        Long(var::CHOICES + off).set(x.b, 0xffff_ffff);
    }
    crate::screens::hud::forget_displayed(x.b);
    crate::services::sound::play_global(x.b, crate::services::sound::CMD_02);
    var::COUNTDOWN.set(x.b, 0x203c);
    var::COUNTDOWN.set(x.b, COUNTDOWN_START);
    objects_0(x)
}

/// Creates the screen objects: four cursors, a panel, six markers, a counter, eight
/// cells; if a pool is full the step stops (the original exits without undoing anything).
fn objects_0(x: &mut Ctx) -> Flow {
    for (link, slot) in (0..4u32).rev().enumerate() {
        let Some(o) = element_d_interface(x.b) else {
            return Flow::Done;
        };
        o.set_u8(x.b, el::KIND, 1);
        o.set_u8(x.b, el::SLOT, slot as u8);
        slot_link(link as u32).set(x.b, o.0 as u16);
    }
    let Some(main) = element_d_interface(x.b) else {
        return Flow::Done;
    };
    main.set_u8(x.b, el::KIND, 0);
    main.set_u8(x.b, el::SHOWN, 0);
    var::MAIN_LINK.set(x.b, main.0 as u16);
    publish_choices(x, main);
    for k in 0..4u32 {
        let o = Obj(sign_ext(slot_link(k).get(x.b)));
        o.set_u16(x.b, el::MAIN, main.0 as u16);
    }
    for slot in (0..6u32).rev() {
        let Some(o) = element_d_interface(x.b) else {
            return Flow::Done;
        };
        o.set_u8(x.b, el::KIND, 2);
        o.set_u8(x.b, el::SLOT, slot as u8);
        o.set_u16(x.b, el::MAIN, main.0 as u16);
    }
    let Some(o) = element_d_interface(x.b) else {
        return Flow::Done;
    };
    o.set_u8(x.b, el::KIND, 3);
    o.set_u8(x.b, el::SLOT, 0);
    var::LINK_9A10.set(x.b, o.0 as u16);
    let seconds = var::COUNTDOWN_SECONDS.get(x.b);
    o.set_u8(x.b, el::SECONDS, seconds);
    o.set_u8(x.b, el::FRACTION, 0);
    for slot in (0..8u32).rev() {
        let Some(o) = element_d_interface(x.b) else {
            return Flow::Done;
        };
        o.set_u8(x.b, el::KIND, 4);
        o.set_u8(x.b, el::SLOT, slot as u8);
    }
    render(x, &[&dispatch::EFFCT]);
    x.wait(2, Step::FadeStart)
}
crate::steps! {
    impl Step {
        FadeStart = 0x1cc0c => |x| {
            fading::DELAY.set(x.b, 1);
            fading::PAGES.set(x.b, 0xf);
            for (k, source) in FADE_SOURCES.into_iter().enumerate() {
                Long(var::FADE_SOURCES.0 + 4 * k as u32).set(x.b, source.addr());
            }
            create_task(x, crate::screens::scene::fading::FROM_WHITE, 0x10);
            fading::STEP.set(x.b, 1);
            two_blocks(x, Grid::Layer2, 0x203, LAYER2_BLOCKS);
            two_blocks(x, Grid::Layer3, 0xfc, LAYER3_BLOCKS);
            var::VIDEO.set(x.b, 0x12ce);
            fade_wait(x)
        },
        FadeLoop = 0x1cca4 => fade_wait,
        Step6Faded = 0x1d19c => |x| {
            dispatch::EFFCT.update_no_next(x.b);
            crate::world::objects::pools::EFFCT.reset(x.b);
            crate::services::sprites::clear_lists(x.b);
            crate::services::sprites::build_in_game(x.b);
            let v = var::VIDEO.get(x.b) & !0xe;
            var::VIDEO.set(x.b, v);
            x.wait(1, Step::Step6Clear)
        },
        Step6Clear = 0x1d1c2 => |x| {
            var::SCREEN_ID.set(x.b, 0xff);
            crate::services::video::clear_objects(x.b);
            clear_layers(x);
            x.wait(4, Step::Step6Palette)
        },
        Step6Palette = 0x1d1e8 => |x| {
            // 0x1a608 (page 1 takes entry 0), 0x1a67e, 0x1a8b2, 0x1a3d4
            crate::services::palette::load_scene_page(x.b, 1);
            crate::services::palette::load_palette_page(x.b, 2, 0x56);
            crate::services::palette::load_palette_page(x.b, 3, 0x56);
            crate::services::palette::load_palette_set(x.b, 3);
            for i in 0..8 {
                for j in 0..8 {
                    palette(0, 16 * i + 2 * j).set_long(x.b, 0);
                }
            }
            fading::PAGES.set(x.b, 0xf);
            crate::services::palette::full_brightness(x.b);
            var::SCROLL_1.set(x.b, 0);
            var::SCROLL_2.set(x.b, 0);
            for v in [var::CAMERA, var::CAMERA_Y, var::CAMERA_2, var::CAMERA_2_Y] {
                Long(v.0).set(x.b, 0);
            }
            Flow::Done
        },
        Step8Show = 0x1d31c => |x| {
            let v = var::VIDEO.get(x.b) | 0xe;
            var::VIDEO.set(x.b, v);
            crate::world::tasks::wait_then_return(x, 1)
        },
        StepEFaded = 0x1d498 => |x| {
            crate::world::objects::pools::SET.reset(x.b);
            crate::world::objects::pools::SOBJ.reset(x.b);
            crate::services::sprites::clear_lists(x.b);
            crate::services::video::clear_objects(x.b);
            x.wait(1, Step::StepEHide)
        },
        StepEHide = 0x1d4b8 => |x| {
            let v = var::VIDEO.get(x.b) & !0xe;
            var::VIDEO.set(x.b, v);
            x.wait(1, Step::StepEClear)
        },
        StepEClear = 0x1d4c6 => |x| {
            clear_layers(x);
            x.wait(4, Step::StepEFade)
        },
        StepEFade = 0x1d4e0 => |x| {
            fading::DELAY.set(x.b, 3);
            fading::PAGES.set(x.b, 0xf);
            create_task(x, crate::screens::scene::fading::RISE, 0x10);
            Flow::Done
        },
    }
}
/// Waits for the opening fade to finish (one frame per turn), then turns the cursors on.
fn fade_wait(x: &mut Ctx) -> Flow {
    if fading::STEP.get(x.b) != 0 {
        render(x, &[&dispatch::EFFCT]);
        return x.wait(1, Step::FadeLoop);
    }
    var::SCREEN_ID.set(x.b, 3);
    let main = Obj(sign_ext(var::MAIN_LINK.get(x.b)));
    main.set_u8(x.b, el::SHOWN, 1);
    // `jmp 0x1de96` at the end
    render(x, &[&dispatch::EFFCT]);
    Flow::Done
}

/// The three cleared layers.
fn clear_layers(x: &mut Ctx) {
    for layer in [Layer::One, Layer::Two, Layer::Three] {
        clear_layer(x.b, layer);
    }
}

/// Creates the entry task `entry` in slot `slot` (`move.w #slot,D0w ; jsr 0x9b6`).
fn create_task(x: &mut Ctx, task: crate::world::tasks::Task, slot: u16) {
    crate::world::tasks::traps::create_task(x.b, slot as u32, task);
}

// ---------------------------------------------------------------------------------------------
// Step 2: cursors, countdown

fn step_2(x: &mut Ctx) -> Flow {
    count_players(x);
    if countdown(x) {
        // countdown over: continue
        var::STEP.set(x.b, 4);
        var::PLAYERS_FROZEN.set(x.b, 1);
    }
    for k in 0..4 {
        let p = player(k);
        if p.u8(x.b, Player::ACTIVE) != 0 {
            cursor(x, p);
        }
    }
    queue(x);
    mark_choices(x);
    check_all_chosen(x);
    let main = Obj(sign_ext(var::MAIN_LINK.get(x.b)));
    publish_choices(x, main);
    var::LOCKED_FLAG.set(x.b, 0);
    // `jmp 0x1de96` at the end
    render(x, &[&dispatch::EFFCT]);
    Flow::Done
}

/// Copies the players' choices into the main object (read by the cursor objects).
fn publish_choices(x: &mut Ctx, main: Obj) {
    for k in 0..3u32 {
        let v = Long(var::CHOICES + 4 * k).get(x.b);
        main.set_u32(x.b, el::CHOICES + 4 * k, v);
    }
    let v = var::WORD_9A0E.get(x.b);
    main.set_u16(x.b, el::TAKEN, v);
    let v = var::LOCKED_FLAG.get(x.b);
    main.set_u8(x.b, el::LOCKED, v);
}

/// `0x1d6d8`: counts the present players and re-arms the countdown when their number changes.
fn count_players(x: &mut Ctx) {
    let mut n = 0u8;
    for k in 0..4 {
        let p = player(k);
        if p.u8(x.b, Player::ACTIVE) != 0 {
            n += 1;
            p.set_u8(x.b, SELECTED, 1);
        }
    }
    if n != var::PLAYERS_PRESENT.get(x.b) {
        var::COUNTDOWN.set(x.b, COUNTDOWN_START);
        let panel = Obj(sign_ext(var::LINK_9A10.get(x.b)));
        panel.set_u8(x.b, el::SECONDS, 20);
    }
    var::PLAYERS_PRESENT.set(x.b, n);
}

/// `0x1d802`: one frame of the countdown; true when the seconds have elapsed (Z flag of the original).
fn countdown(x: &mut Ctx) -> bool {
    let tick = var::COUNTDOWN_TICK.get(x.b).wrapping_sub(1);
    var::COUNTDOWN_TICK.set(x.b, tick);
    if tick != 0 {
        return false;
    }
    let seconds = var::COUNTDOWN_SECONDS.get(x.b);
    if seconds == 0 {
        return true;
    }
    let seconds = seconds - 1;
    var::COUNTDOWN_SECONDS.set(x.b, seconds);
    let panel = Obj(sign_ext(var::LINK_9A10.get(x.b)));
    panel.set_u8(x.b, el::SECONDS, seconds);
    var::COUNTDOWN_TICK.set(x.b, 50);
    false
}

/// `0x1d584`: cursor of player `a0` (cells 0 to 11, two per character).
fn cursor(x: &mut Ctx, a0: Obj) {
    if a0.u8(x.b, Player::CHARACTER) & 0x80 != 0 {
        return;
    }
    dedupe(x, a0);
    let d1 = crate::services::inputs::player_inputs(x.b, a0.0).fresh;
    let mut d3 = a0.u8(x.b, Player::CHARACTER) as u32;
    let slot = a0.u8(x.b, Player::INDEX);
    let mut d0: u8 = 0;
    loop {
        let d5 = ((d3 as u8) >> 1) as u32;
        if d1 & 2 != 0 {
            d0 = table::CURSOR_DOWN_DELTAS.byte(x.b, d5);
        }
        if d1 & 1 != 0 {
            d0 = table::CURSOR_UP_DELTAS.byte(x.b, d5);
        }
        if d0 == 0 {
            break;
        }
        // frees the cell being left
        let mut cell = d3 & 0xffff;
        if choice(cell).get(x.b) == slot || {
            cell += 1;
            choice(cell).get(x.b) == slot
        } {
            choice(cell).set(x.b, 0xff);
        }
        // new cell, wrapping around to the other end
        let mut n = (d3 as u8).wrapping_add(d0);
        if n & 0x80 != 0 {
            n = 10;
        }
        if n > 10 {
            n = 0;
        }
        d3 = n as u32;
        if choice(d3).get(x.b) & 0x80 != 0 {
            choice(d3).set(x.b, slot);
            break;
        }
        if choice(d3 + 1).get(x.b) & 0x80 != 0 {
            choice(d3 + 1).set(x.b, slot);
            break;
        }
    }
    // validation
    d0 = 0;
    let mut lock = true;
    if var::COUNTDOWN.get(x.b) != 0 && d1 & 0xf0 == 0 {
            let s = slot as u32;
            d0 = 1;
            let held = !var::BUTTONS_B.get(x.b) & var::BUTTONS_A.get(x.b);
            lock = (held >> s) & 1 != 0;
        }
    if lock {
        let mut taken = var::TAKEN.get(x.b) as u32;
        taken |= 1 << (d3 & 31);
        var::TAKEN.set(x.b, taken as u16);
        let mut other = var::WORD_9A0E.get(x.b) as u32;
        d3 = (d3 as u8).wrapping_add(d0) as u32;
        if (other >> (d3 & 31)) & 1 != 0 {
            d3 ^= 1;
        }
        other |= 1 << (d3 & 31);
        var::WORD_9A0E.set(x.b, other as u16);
        // 0x2e630: character stat (development card only). Original glitch: it receives D0.w, of which only the low
        // byte is the choice; the high byte is left over from previous calls: taken as zero (the development card is
        // never there, code-mort.md)
        crate::services::eeprom::stat_chosen_character(x.b, (d3 & 0xff) as u16);
        d3 |= 0x80;
        var::LOCKED_FLAG.set(x.b, 1);
    }
    a0.set_u8(x.b, Player::CHARACTER, d3 as u8);
}

/// `0x1d714`: the player must not occupy another player's cell; otherwise he advances two cells.
fn dedupe(x: &mut Ctx, a0: Obj) {
    loop {
        let cell = a0.u8(x.b, Player::CHARACTER) as u32;
        let (a1, a2) = (choice(cell), choice(cell + 1));
        let slot = a0.u8(x.b, Player::INDEX);
        let first = a1.get(x.b);
        if first & 0x80 != 0 || first == slot {
            // the first place is free or mine: the second must not be mine
            if a2.get(x.b) != slot {
                a1.set(x.b, slot);
            }
            return;
        }
        let second = a2.get(x.b);
        if second & 0x80 != 0 {
            a2.set(x.b, slot);
            return;
        }
        if second == slot {
            return;
        }
        let mut n = a0.u8(x.b, Player::CHARACTER).wrapping_add(2);
        if n > 12 {
            n = 0;
        }
        a0.set_u8(x.b, Player::CHARACTER, n);
    }
}

/// `0x1d760`: a player who has not confirmed and whose neighbouring cell was freed takes the first place.
fn queue(x: &mut Ctx) {
    let taken = var::TAKEN.get(x.b) as u32;
    for k in 0..6u32 {
        let (a1, a2) = (choice(2 * k), choice(2 * k + 1));
        if a1.get(x.b) & 0x80 == 0 {
            continue;
        }
        let other = a2.get(x.b);
        if other & 0x80 != 0 || (taken >> (2 * k)) & 1 != 0 {
            continue;
        }
        a1.set(x.b, other);
        a2.set(x.b, 0xff);
    }
}

/// `0x1d794`: for each cell cursor, `+0x16` = parity of the cell occupied by its player.
fn mark_choices(x: &mut Ctx) {
    for k in 0..4u32 {
        let o = Obj(sign_ext(slot_link(k).get(x.b)));
        let slot = o.u8(x.b, el::SLOT);
        let mut parity = 0u8;
        for i in 0..SLOTS as u32 {
            if choice(i).get(x.b) == slot {
                break;
            }
            parity ^= 1;
        }
        o.set_u8(x.b, el::PARITY, parity);
    }
}

/// `0x1d7c0`: if all present players have confirmed, moves to step 4.
fn check_all_chosen(x: &mut Ctx) {
    if var::STEP.get(x.b) != 4 {
        let mut left = var::PLAYERS_PRESENT.get(x.b);
        for k in 0..4 {
            if player(k).u8(x.b, Player::CHARACTER) & 0x80 != 0 {
                left = left.wrapping_sub(1);
                if left == 0 {
                    var::STEP.set(x.b, 4);
                    freeze(x);
                    return;
                }
            }
        }
        return;
    }
    freeze(x);
}
fn freeze(x: &mut Ctx) {
    var::COUNTDOWN_SECONDS.set(x.b, 30);
    var::PLAYERS_FROZEN.set(x.b, 1);
}

// ---------------------------------------------------------------------------------------------
// Step 4: short wait

fn step_4(x: &mut Ctx) -> Flow {
    render(x, &[&dispatch::EFFCT]);
    let left = var::COUNTDOWN_SECONDS.get(x.b).wrapping_sub(1);
    var::COUNTDOWN_SECONDS.set(x.b, left);
    if left == 0 {
        let s = var::STEP.get(x.b).wrapping_add(2);
        var::STEP.set(x.b, s);
    }
    Flow::Done
}

// ---------------------------------------------------------------------------------------------
// Step 6: characters frozen, fade

fn step_6(x: &mut Ctx) -> Flow {
    let s = var::STEP.get(x.b).wrapping_add(2);
    var::STEP.set(x.b, s);
    for k in 0..4 {
        let p = player(k);
        if p.u8(x.b, Player::ACTIVE) == 0 {
            continue;
        }
        let cell = p.u8(x.b, Player::CHARACTER) & 0x7f;
        p.set_u8(x.b, Player::CHARACTER, cell);
        let variant = table::VARIANT.pc_byte(x.b, cell as u32 >> 1);
        p.set_u8(x.b, SELECTION_KIND, variant);
        // the routine of table 0x1d0f6 (`movea.w`, called with A0 = the player) is the initialisation of the
        // player of that number (0x2e58, 0x2e6e, 0x2e86, 0x2e9e)
        let kind = p.u8(x.b, Player::INDEX) as u32;
        assert!(kind < 4, "selection: player {kind} outside table 0x1d0f6");
        crate::actors::player::init::initialize(x.b, kind);
        if (p.u8(x.b, Player::CHARACTER) & 0x7f) >> 1 == 5 {
            p.set_u8(x.b, Player::SPEED_CLASS, 1);
            p.set_u8(x.b, SETTING_16B, 5);
            CHARACTER_5_FLAG.set(x.b, 1);
        }
    }
    crate::services::sound::service(x.b, crate::services::sound::Service::Ff07 { lock: 0, line: 0x1c });
    fading::DELAY.set(x.b, 1);
    fading::PAGES.set(x.b, 0xf);
    x.subroutine(crate::services::fading::descent, Step::Step6Faded)
}

// ---------------------------------------------------------------------------------------------
// Step 8: second screen

fn step_8(x: &mut Ctx) -> Flow {
    let s = var::STEP.get(x.b).wrapping_add(2);
    var::STEP.set(x.b, s);
    crate::services::sound::service(x.b, crate::services::sound::Service::Ff06 { lock: 0xff, line: 4 });
    crate::services::sound::play_global(x.b, crate::services::sound::CMD_2A);
    two_blocks(x, Grid::Layer2, 0x208, LAYER2_BLOCKS);
    two_blocks(x, Grid::Layer3, 0x14a, LAYER3_BLOCKS);
    let mut count = 0u8;
    for counter in (0..2u8).rev() {
        // the variant of the panel is the loop counter of the `dbf` (D0)
        let Some(o) = banner(x.b, counter) else { break };
        o.set_u32(x.b, el::WATCHED, var::PLAYERS_PRESENT.0);
        count = count.wrapping_add(1);
    }
    var::PLAYERS_PRESENT.set(x.b, count);
    let mut d4 = 0xeau8;
    for _ in 0..2 {
        let Some(o) = pools::SOBJ.alloc(x.b) else { break };
        o.set_active(x.b, 1);
        o.set_kind(x.b, 0);
        o.set_variant(x.b, d4);
        d4 = d4.wrapping_add(1);
        o.set_u16(x.b, Obj::X, 0x80);
        o.set_u16(x.b, Obj::Z, 0xc0);
        o.set_y(x.b, 1);
    }
    render(x, &[&dispatch::EFFCT, &dispatch::SOBJ]);
    x.wait(2, Step::Step8Show)
}

/// `0x1d686`: has a player in play (state < 4) pressed a button? (Z flag of the original: false)
fn any_button(x: &mut Ctx) -> bool {
    if var::GAME_ACTIVE.get(x.b) == 0 {
        return false;
    }
    let mut buttons = 0u8;
    for k in 0..4u32 {
        let p = player(3 - k);
        if p.u8(x.b, Player::ACTIVE) == 0 || (p.u8(x.b, Player::MAIN) as i8) >= 4 {
            continue;
        }
        buttons |= Byte(var::PRESSED + (3 - k)).get(x.b);
    }
    buttons & 0xf0 != 0
}

// ---------------------------------------------------------------------------------------------
// Steps 10, 12, 14

fn step_a(x: &mut Ctx) -> Flow {
    render(x, &[&dispatch::EFFCT, &dispatch::SOBJ]);
    if any_button(x) {
        var::COUNTDOWN.set(x.b, 1);
        crate::world::objects::pools::EFFCT.reset(x.b);
    } else if var::PLAYERS_PRESENT.get(x.b) != 0 {
        return Flow::Done;
    }
    let s = var::STEP.get(x.b).wrapping_add(2);
    var::STEP.set(x.b, s);
    let which = var::MODE_FLAG.get(x.b).wrapping_sub(2) as u16;
    let banner = table::BANNERS.pc_long(x.b, 2 * which as u32);
    var::BANNER.set(x.b, banner);
    var::BANNER_X.set(x.b, 0x44);
    var::BANNER_Y.set(x.b, 0xc0);
    var::PLAYERS_PRESENT.set(x.b, 0);
    var::LOCKED_FLAG.set(x.b, 0);
    fading::DELAY.set(x.b, 3);
    fading::PAGES.set(x.b, 0xf);
    fading::STEP.set(x.b, 7);
    create_task(x, crate::screens::scene::fading::DESCENT_STEP, 0x10);
    Flow::Done
}

fn step_c(x: &mut Ctx) -> Flow {
    render(x, &[&dispatch::SET, &dispatch::SOBJ]);
    if var::FRAME_COUNTER.get(x.b) & 3 == 0 {
        let y = var::BANNER_Y.get(x.b).wrapping_sub(1);
        var::BANNER_Y.set(x.b, y);
        if y <= 0x40 {
            let s = var::STEP.get(x.b).wrapping_add(2);
            var::STEP.set(x.b, s);
            var::BANNER_Y.set(x.b, 0x40);
            var::COUNTDOWN.set(x.b, 0x258);
            return finish_c(x);
        }
        let phase = (y.wrapping_sub(1) & 0x10) as u8;
        if phase != var::PLAYERS_PRESENT.get(x.b) && var::LOCKED_FLAG.get(x.b) != 7 {
            let v = var::PLAYERS_PRESENT.get(x.b) ^ 0x10;
            var::PLAYERS_PRESENT.set(x.b, v);
            // 0xcdedc: SET object of type 13 (the blinking), variant is the counter
            let n = var::LOCKED_FLAG.get(x.b);
            if crate::actors::families::creators::set_13_pure(x.b, n).is_ok() {
                var::LOCKED_FLAG.set(x.b, n + 1);
            }
        }
    }
    finish_c(x)
}
fn finish_c(x: &mut Ctx) -> Flow {
    if any_button(x) {
        let s = var::STEP.get(x.b).wrapping_add(2);
        var::STEP.set(x.b, s);
        var::COUNTDOWN.set(x.b, 1);
    }
    Flow::Done
}

fn step_e(x: &mut Ctx) -> Flow {
    render(x, &[&dispatch::SET, &dispatch::SOBJ]);
    if fading::STEP.get(x.b) != 0 {
        return Flow::Done;
    }
    if any_button(x) {
        var::COUNTDOWN.set(x.b, 1);
    }
    let left = var::COUNTDOWN.get(x.b).wrapping_sub(1);
    var::COUNTDOWN.set(x.b, left);
    if left != 0 {
        return Flow::Done;
    }
    crate::services::sound::service(x.b, crate::services::sound::Service::Ff07 { lock: 0, line: 0x1c });
    let phase = var::PHASE.get(x.b).wrapping_add(4);
    var::PHASE.set(x.b, phase);
    var::STEP.set(x.b, 0);
    var::SUB.set(x.b, 0);
    fading::DELAY.set(x.b, 1);
    fading::PAGES.set(x.b, 0xf);
    x.subroutine(|x| crate::services::fading::descent_steps(x, 8), Step::StepEFaded)
}

