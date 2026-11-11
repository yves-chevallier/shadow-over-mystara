//! Fixed-screen task (`0x1835e`, F4): the attract high-score table (sub-modes 0 and 6 of the
//! mode machine, `modes::attract::fixed_screen`).
//!
//! On each frame: the screen stage (state block `FF0F50`), then the EFFCT and SET objects and the sprite
//! list, then a cession frame (`jsr 0xb64`, resume at stage `Resume`, `0x18384`). Stages (`+0`, step of 2):
//!
//! * 0 (`0x1838a`): scenery (palettes, two tile blocks per layer, title palettes), six rows of the
//!   table (ranks 5 to 10, `0x18b8e`), frame SET object, brightness levels;
//! * 2 (`0x18530`): sub-stages (`+1`) — 0 waits 0x40 frames then draws the first three ranks
//!   (`0x18a3e`), their four EFFCT objects, the title and the best score; 2 scrolls layer 2 up by
//!   0xe0 lines; 4 waits 0x90 frames, raises the "screen finished" flag (`+3`) and ends the task.
//!
//! A table row (14 bytes): `+0` level, `+1` character, `+2..+7` name (0xff: blank), `+8` score
//! (7 BCD digits), `+0xc` emblem variant, `+0xd` emblem present (EFFCT object of type 0x5b).
//!
//! The cells of the text layer (scroll 1) are pairs of words (tile, attribute), stored by columns:
//! `+4` next row, `+0x80` next column. The tile tables are read from the ROM (data).

use crate::actors::aids::divide;
use crate::screens::scene::fading;
use crate::hw::mem::{rd16, rd32, rd8, wr16, wr32, wr8};
use crate::screens::modes::{Flow, Sequence};
use crate::screens::scene::Ctx;
use crate::services::video::Grid;
use crate::world::objects::pools;

/// State block of the screen (`A0`, A5 - 0x70b0).
mod state {
    pub const BASE: u32 = 0xff_0f50;
    pub const STEP: u32 = BASE;
    pub const SUB_STEP: u32 = BASE + 1;
    /// Screen finished: the mode machine moves on to the next one.
    pub const FINISHED: u32 = BASE + 3;
    /// Timer (signed word).
    pub const TIMER_LIGHT: u32 = BASE + 0x10;
}

/// Table rows: ranks 5 to 10 (from `LOW_RANKS` downward in memory), ranks 2 to 4, best score.
const LOW_RANKS: u32 = 0xff_104e;
const HIGH_RANKS: u32 = 0xff_0fde;
const BEST: u32 = 0xff_0fd0;
const LINE: u32 = 0xe;
/// Byte `+1` of the four EFFCT objects of the first ranks, read from `EMBLEMS` downward (step `LINE`).
const EMBLEMS: u32 = 0xff_0ffa;

/// Work RAM variables (offsets from A5).
mod var {
    pub const LAYERS: u32 = 0x14;
    pub const TERRAIN_VIEW: u32 = 0x36;
    pub const SCROLL_2: u32 = 0x4ba;
    pub const CAMERA_X: u32 = 0x506;
    pub const CAMERA_Y: u32 = 0x50a;
    pub const CAMERA_2_X: u32 = 0x556;
    pub const CAMERA_2_Y: u32 = 0x55a;
}

crate::steps! {
    /// Stages of the task: the entry and the cession return, each of which does one turn.
    pub enum Step {
        Entry = 0x1835e => tour,
        Resume = 0x18384 => tour,
        /// Return from the stage call (`jsr (table,PC)`): the objects, the sprites, a cession frame.
        AfterStep = at::AFTER_STEP => after_step,
    }
}

/// Addresses of the original.
mod at {
    /// Return from the stage call (`jsr (table,PC)`).
    pub const AFTER_STEP: u32 = 0x1836e;
}

/// Tile tables (ROM, data).
/// Tile tables of the ranking screen (engine data `ranking/*`, read by address).
mod tiles {
    /// Large digits (4 x 4 cells) of the best score.
    pub const BIG_DIGITS: u32 = 0x18d58;
    /// Digits (2 x 2 cells).
    pub const DIGITS: u32 = 0x18d6c;
    /// Rank numbers 5 to 10 (2 x 2).
    pub const RANKS: u32 = 0x18d82;
    /// Portrait of each character: tile and attribute (4 x 2 cells).
    pub const PORTRAITS: u32 = 0x18d8e;
    /// Name letters (2 x 2).
    pub const LETTERS: u32 = 0x18dbe;
    /// Numbers of ranks 2 to 4 (6 x 4 cells).
    pub const BIG_RANKS: u32 = 0x18b88;
    /// First rank (6 x 4).
    pub const FIRST_RANK: u16 = 0x230;
    /// Empty cell of the large characters.
    pub const WHITE: u16 = 0x2fc;
}

/// Palettes of the first stage: ROM source and destination, all at full brightness.
const PALETTES: [(u32, u32); 2] = [(0x2537c0, 0x91c3c0), (0x28f4c0, 0x91c180)];
/// Title: large characters (2 x 2 cells, attribute 1) written starting from `TITLE_CELL`; 0 = blank.
const TITLE_CELL: u32 = 0x91021c;
const TITLE: [u16; 20] = [
    0x192, 0x194, 0x190, 0x192, 0, 0x190, 0x1b6, 0x174, 0x17a, 0x17c, 0, 0x1ba, 0x1b6, 0x174, 0x1be, 0x17c, 0x19a, 0x17c,
    0x1b6, 0x1b8,
];

/// A table row.
#[derive(Clone, Copy)]
struct Line(u32);

impl Line {
    fn level(self, x: &mut Ctx) -> u8 {
        rd8(x.b, self.0)
    }
    fn character(self, x: &mut Ctx) -> u8 {
        rd8(x.b, self.0 + 1)
    }
    fn letter(self, x: &mut Ctx, i: u32) -> u8 {
        rd8(x.b, self.0 + 2 + i)
    }
    fn score(self, x: &mut Ctx) -> u32 {
        rd32(x.b, self.0 + 8)
    }
}

/// Brush on the text layer: `p` is the address of the current cell.
struct Brush(u32);

impl Brush {
    fn advance(&mut self, d: i32) {
        self.0 = self.0.wrapping_add(d as u32);
    }
    fn case(&self, x: &mut Ctx, d: u32, tile: u16, attr: u16) {
        wr16(x.b, self.0 + d, tile);
        wr16(x.b, self.0 + d + 2, attr);
    }
    /// Character of 2 x 2 cells: `t`, `t + 1` to the right, `t + 0x10` and `t + 0x11` below.
    fn car(&self, x: &mut Ctx, d: u32, t: u16, attr: u16) {
        self.case(x, d, t, attr);
        self.case(x, d + 0x80, t + 1, attr);
        self.case(x, d + 4, t + 0x10, attr);
        self.case(x, d + 0x84, t + 0x11, attr);
    }
    /// Block of `width` x `height` cells, consecutive tiles per row, rows of 16 tiles.
    fn block(&mut self, x: &mut Ctx, t: u16, width: u16, height: u16, attr: u16) {
        for line in 0..height {
            for col in 0..width {
                self.case(x, 0x80 * col as u32 + 4 * line as u32, t + 0x10 * line + col, attr);
            }
        }
    }
    /// Six-letter name (2 x 2, attribute 2), one letter every two columns.
    fn name(&mut self, x: &mut Ctx, line: Line) {
        for i in 0..6 {
            let c = match line.letter(x, i) {
                0xff => 0x81,
                c => c,
            };
            let t = crate::world::tables::engine().word(x.b, tiles::LETTERS + 2 * c as u32);
            self.car(x, 0, t, 2);
            self.advance(0x100);
        }
    }
    /// The seven low digits of the score (2 x 2, attribute 3).
    fn score(&mut self, x: &mut Ctx, line: Line) {
        let score = line.score(x);
        for i in (0..7).rev() {
            let t = crate::world::tables::engine().word(x.b, tiles::DIGITS + 2 * (score >> (4 * i) & 0xf));
            self.car(x, 0, t, 3);
            self.advance(0x100);
        }
    }
    /// Level in two digits: tile = digit, attribute 1, over two columns.
    fn level(&self, x: &mut Ctx, line: Line) {
        let (tens, units) = divide(line.level(x) as u32, 10);
        self.case(x, 0, tens as u16, 1);
        self.case(x, 0x80, units as u16, 1);
    }
}

/// `0x18a0e`: the row's emblem (EFFCT object of type 0x5b), if it has one.
fn emblem(x: &mut Ctx, line: Line) {
    if rd8(x.b, line.0 + 0xd) == 0 {
        return;
    }
    if let Some(o) = allocated(x, &pools::EFFCT) {
        wr8(x.b, o, 1);
        wr8(x.b, o + 2, 0x5b);
        let variant = rd8(x.b, line.0 + 0xc);
        wr8(x.b, o + 3, variant);
        wr8(x.b, o + 0x2f, 0);
    }
}

/// Allocation of an object: its address, or nothing (carry set).
fn allocated(x: &mut Ctx, pool: &pools::Pool) -> Option<u32> {
    pool.alloc(x.b).map(|o| o.0)
}

/// `0x18b8e`: row of ranks 5 to 10 (`rank` = 10 to 5) starting from cell `p`.
fn low_line(x: &mut Ctx, line: Line, p: u32, rank: u16) {
    emblem(x, line);
    let mut p = Brush(p);
    let t = crate::world::tables::engine().word(x.b, tiles::RANKS + 2 * (rank - 5) as u32);
    p.car(x, 0, t, 2);
    p.advance(0x100);
    p.case(x, 0, 0x215, 1);
    p.case(x, 4, 0x225, 1);
    p.advance(0x80);
    p.case(x, 0, 0x1f9, 1);
    p.case(x, 4, 0x209, 1);
    p.advance(0x200);
    let portrait = tiles::PORTRAITS + 4 * line.character(x) as u32;
    let (t, attr) = (crate::world::tables::engine().word(x.b, portrait), crate::world::tables::engine().word(x.b, portrait + 2));
    p.block(x, t, 4, 2, attr);
    p.advance(0x2fc);
    p.name(x, line);
    p.advance(0x100);
    p.score(x, line);
    // "exp" after the score, "LEVEL" on the row above, then the level
    p.advance(4);
    for (k, t) in [0x65, 0x78, 0x70].into_iter().enumerate() {
        p.case(x, 0x80 * k as u32, t, 3);
    }
    p.advance(0x100 - 0xefc);
    for (k, t) in [0x4c, 0x45, 0x56, 0x45, 0x4c].into_iter().enumerate() {
        p.case(x, 0x80 * k as u32, t, 1);
    }
    p.advance(0x300);
    p.level(x, line);
}

/// `0x18a3e`: row of ranks 2 to 4 (`rank` = 2 to 4) starting from cell `p`.
fn high_line(x: &mut Ctx, line: Line, p: u32, rank: u16) {
    emblem(x, line);
    let mut p = Brush(p);
    let t = crate::world::tables::engine().word(x.b, tiles::BIG_RANKS + 2 * (rank - 2) as u32);
    p.block(x, t, 6, 4, 0x1e);
    p.advance(0x14);
    p.score(x, line);
    p.advance(-0x6f8);
    p.name(x, line);
    p.advance(-0x6fc + 0x384);
    // "LV" then the level
    p.case(x, 0, 0x4c, 1);
    p.case(x, 0x80, 0x56, 1);
    p.advance(0x100);
    p.level(x, line);
}

/// Stage 0 (`0x1838a`): the scenery and ranks 5 to 10.
fn scenery(x: &mut Ctx) -> Flow {
    crate::services::video::clear_objects(x.b);
    crate::services::video::clear_layer(x.b, crate::services::video::Layer::One);
    let step = rd8(x.b, state::STEP).wrapping_add(2);
    wr8(x.b, state::STEP, step);
    wr16(x.b, state::TIMER_LIGHT, 0x40);
    x.set_v8(var::TERRAIN_VIEW, 0);
    x.set_v16(var::CAMERA_X, 0);
    x.set_v16(var::CAMERA_Y, 0x10);
    x.set_v16(var::CAMERA_2_X, 0);
    x.set_v16(var::CAMERA_2_Y, 0x20);
    // 0x1a366 (scene 1 palettes), pages 1 to 3 (0x1a60a, 0x1a67e, 0x1a8b2), 0x1a5e8
    crate::services::palette::load_scene_palettes_of(x.b, 1);
    for (page, n) in [(1, 2), (2, 0x3a), (3, 0x3a)] {
        crate::services::palette::load_palette_page(x.b, page, n);
    }
    crate::actors::families::small::palette_twelve_blocks(x.b);
    // `ori.l #0xf000f000` on the first six longs, `adda.l` (addition) on the last two
    for (source, dest) in PALETTES {
        for i in 0..8 {
            let v = crate::world::tables::engine().long(x.b, source + 4 * i);
            let v = if i < 6 { v | 0xf000_f000 } else { v.wrapping_add(0xf000_f000) };
            wr32(x.b, dest + 4 * i, v);
        }
    }
    for (grid, n, dest) in [
        (Grid::Layer2, 0x80, 0x917000),
        (Grid::Layer2, 0x81, 0x917400),
        (Grid::Layer3, 0x111, 0x91b800),
        (Grid::Layer3, 0x112, 0x91b900),
    ] {
        crate::services::video::load_grid(x.b, grid, n, dest);
    }
    wr16(x.b, 0x400004, 0x1144);
    x.set_v16(var::LAYERS, 0);
    crate::services::video::turn_layers_on(x.b, 0x1e0e);
    for k in 0..6 {
        low_line(x, Line(LOW_RANKS - LINE * k), 0x912168 - 0x10 * k, 10 - k as u16);
    }
    if let Some(o) = allocated(x, &pools::SET) {
        wr8(x.b, o, 1);
        wr8(x.b, o + 2, 4);
        wr8(x.b, o + 3, 3);
        wr8(x.b, o + 0x2f, 1);
    }
    for (pages, level) in [(0xc, 0xa000), (3, 0xf000)] {
        fading::PAGES.set(x.b, pages);
        crate::services::palette::or_brightness(x.b, level);
    }
    Flow::Done
}

/// Screen timer: counted down; true when it goes below zero.
fn timer_light(x: &mut Ctx) -> bool {
    let remainder = rd16(x.b, state::TIMER_LIGHT).wrapping_sub(1);
    wr16(x.b, state::TIMER_LIGHT, remainder);
    (remainder as i16) < 0
}

fn next_sub_step(x: &mut Ctx, timer_light: u16) {
    let s = rd8(x.b, state::SUB_STEP).wrapping_add(2);
    wr8(x.b, state::SUB_STEP, s);
    wr16(x.b, state::TIMER_LIGHT, timer_light);
}

/// Sub-stage 0 (`0x18544`): after 0x40 frames, ranks 1 to 4 and the title; then the scroll begins.
fn first_ranks(x: &mut Ctx) -> Flow {
    if !timer_light(x) {
        return Flow::Done;
    }
    next_sub_step(x, 0xe0);
    for k in 0..3 {
        high_line(x, Line(HIGH_RANKS + LINE * k), 0x9100d8 + 0x800 * k, 2 + k as u16);
    }
    for k in 0..4 {
        if let Some(o) = allocated(x, &pools::EFFCT) {
            wr8(x.b, o, 1);
            wr8(x.b, o + 2, 3);
            wr8(x.b, o + 3, 3 - k as u8);
            let v = rd8(x.b, EMBLEMS - LINE * k + 1);
            wr8(x.b, o + 0x2f, v);
        }
    }
    let title = Brush(TITLE_CELL);
    for (k, t) in TITLE.into_iter().enumerate() {
        let d = 0x100 * k as u32;
        if t == 0 {
            for case in [0, 4, 0x80, 0x84] {
                title.case(x, d + case, tiles::WHITE, 1);
            }
        } else {
            title.car(x, d, t, 1);
        }
    }
    best_score(x);
    rise(x)
}

/// End of `0x18544`: the first rank (`BEST`), in large digits.
fn best_score(x: &mut Ctx) {
    let line = Line(BEST);
    let mut p = Brush(0x9100ac);
    emblem(x, line);
    if let Some(o) = allocated(x, &pools::EFFCT) {
        wr8(x.b, o, 1);
        wr8(x.b, o + 2, 0x5b);
        wr8(x.b, o + 0x2f, 1);
    }
    p.block(x, tiles::FIRST_RANK, 6, 4, 0x1e);
    p.advance(0x710);
    let score = line.score(x);
    for i in (0..7).rev() {
        let t = crate::world::tables::engine().word(x.b, tiles::BIG_DIGITS + 2 * (score >> (4 * i) & 0xf));
        p.block(x, t, 4, 4, 3);
        p.advance(0x200);
    }
    p.advance(-0xcf0);
    p.name(x, line);
    p.advance(0x300);
    // "LV" then the level in 2 x 2 digits
    p.car(x, 0, 0x19a, 1);
    p.car(x, 0x100, 0x1be, 1);
    p.advance(0x200);
    let (tens, units) = divide(line.level(x) as u32, 10);
    for (k, digit) in [tens, units].into_iter().enumerate() {
        let t = crate::world::tables::engine().word(x.b, tiles::DIGITS + 2 * digit);
        p.car(x, 0x100 * k as u32, t, 1);
    }
}

/// Sub-stage 2 (`0x189e0`): layer 2 scrolls up one line per frame for 0xe0 frames.
fn rise(x: &mut Ctx) -> Flow {
    let y = x.v16(var::SCROLL_2).wrapping_add(1);
    x.set_v16(var::SCROLL_2, y);
    if timer_light(x) {
        next_sub_step(x, 0x90);
    }
    Flow::Done
}

/// Sub-stage 4 (`0x189f8`): after 0x90 frames, the screen is finished and the task ends (`jmp 0x9f8` from
/// the stage: the return to the loop stays on the stack).
fn end(x: &mut Ctx) -> Flow {
    if !timer_light(x) {
        return Flow::Done;
    }
    let finished = rd8(x.b, state::FINISHED).wrapping_add(1);
    wr8(x.b, state::FINISHED, finished);
    super::finish(x)
}

/// One turn of the loop: the stage, the objects, the sprites, a cession frame.
fn tour(x: &mut Ctx) -> Flow {
    let step = rd8(x.b, state::STEP);
    x.push_return(Step::AfterStep);
    match (step, rd8(x.b, state::SUB_STEP)) {
        (0, _) => scenery(x),
        (2, 0) => first_ranks(x),
        (2, 2) => rise(x),
        (2, 4) => end(x),
        // the tables 0x18386 (two stages) and 0x1853c (three sub-stages) are rewritten; `end` ends the task
        // (code-mort.md, lot "CPU removal")
        (e, s) => unreachable!("score table: stage {e:#x}, sub-stage {s:#x} outside the tables"),
    }
}

/// `0x1836e`: return from the stage.
fn after_step(x: &mut Ctx) -> Flow {
    crate::core::dispatch::EFFCT.update_no_next(x.b);
    crate::core::dispatch::SET.update_no_next(x.b);
    crate::services::sprites::build_in_game(x.b);
    super::one_frame(x, Step::Resume)
}
