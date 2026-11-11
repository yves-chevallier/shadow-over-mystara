//! Scrolling (B1): cameras 1 (scroll layer 2) and 2 (layer 3) copy into their
//! layer the columns and rows of scenery that enter the screen. See `docs/video.md`.
//!
//! A camera is a work RAM record (`A5 + 0x4fe` and `A5 + 0x54e`, addressed by
//! A0 in the original code). Each frame, its mode script (original code, jump table by
//! mode) moves the target position (x, y); `draw` then compares the target with the position already
//! drawn and, in steps of 16 pixels (32 for camera 2), copies a column or a row of cells
//! of the scenery map (table of block indices in ROM, strips chosen per scene) into the layer.
//!
//! The natives chain into the original code at the entry points located after the call of the
//! mode script (0x2f81c: with target stops; 0x2f86e and 0x328a8: copy only); the mode
//! scripts and the scene initialisation (first call) remain original code.

use crate::world::World;

use crate::world::data::data;
use crate::hw::video::Vram;
use crate::world::objects::Obj;
use crate::world::ram::{Byte, Long, Word};
use crate::world::record::record;
use crate::world::tables::engine;
use crate::services::video::hw;

/// A camera: a work RAM record (`CAMERA_1`, `CAMERA_2`), whose mode script (`camera_modes`) moves the target and
/// whose scenery copy (this module) follows it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Camera(pub Obj);

impl Camera {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Camera {
        /// Non-zero once the camera is initialised for the scene (the first call of a scene initialises it); as a
        /// word with the step of the mode script.
        active, set_active, ACTIVE: u8 @ 0x00;
        state, set_state, STATE: u16 @ 0x00;
        /// Step of the mode script (increases by 4), then sub-step.
        step, set_step, STEP: u8 @ 0x01;
        sub_step, set_sub_step, SUB_STEP: u8 @ 0x02;
        /// Target position (whole pixels; as longs in 16.16 with their fractions).
        x, set_x, X: u16 @ 0x08;
        x_long, set_x_long, X_LONG: u32 @ 0x08;
        x_fraction, set_x_fraction, X_FRACTION: u16 @ 0x0a;
        y, set_y, Y: u16 @ 0x0c;
        y_long, set_y_long, Y_LONG: u32 @ 0x0c;
        y_fraction, set_y_fraction, Y_FRACTION: u16 @ 0x0e;
        /// Position already drawn (as longs: the drift of camera 2 adds a fraction).
        drawn_x, set_drawn_x, DRAWN_X: u16 @ 0x10;
        drawn_x_long, set_drawn_x_long, DRAWN_X_LONG: u32 @ 0x10;
        drawn_y, set_drawn_y, DRAWN_Y: u16 @ 0x14;
        drawn_y_long, set_drawn_y_long, DRAWN_Y_LONG: u32 @ 0x14;
        /// Script variable (starting ordinate, curve...), then vertical speed (`motion::Z`).
        scratch, set_scratch, SCRATCH: u16 @ 0x18;
        speed, set_speed, SPEED: u16 @ 0x1a;
        /// Gap target - drawn.
        dx, set_dx, DX: u16 @ 0x1c;
        dy, set_dy, DY: u16 @ 0x1e;
        /// Mode of the script (multiple of 4: `camera_modes`).
        mode, set_mode, MODE: u16 @ 0x20;
        /// Cursor in the map: offset in the strip (its low byte alone for camera 2) and strip number.
        cursor_offset, set_cursor_offset, CURSOR_OFFSET: u16 @ 0x22;
        cursor_offset_low, set_cursor_offset_low, CURSOR_OFFSET_LOW: u8 @ 0x23;
        cursor_row, set_cursor_row, CURSOR_ROW: u16 @ 0x24;
        /// Target of the previous frame (whole pixels; as longs with their fractions).
        previous_x, set_previous_x, PREVIOUS_X: u16 @ 0x26;
        previous_x_long, set_previous_x_long, PREVIOUS_X_LONG: u32 @ 0x26;
        previous_y, set_previous_y, PREVIOUS_Y: u16 @ 0x2a;
        previous_y_long, set_previous_y_long, PREVIOUS_Y_LONG: u32 @ 0x2a;
        /// Left stop (does not go back).
        min_x, set_min_x, MIN_X: u16 @ 0x2e;
        /// Per-line scroll: accumulator of the gradient (long), or offset of camera 2 from the screen (word).
        gradient, set_gradient, GRADIENT: u32 @ 0x30;
        screen_offset, set_screen_offset, SCREEN_OFFSET: u16 @ 0x30;
        /// Right stop.
        max_x, set_max_x, MAX_X: u16 @ 0x34;
        /// Per-line scroll: address (high word) of the gradient's table.
        gradient_table, set_gradient_table, GRADIENT_TABLE: u16 @ 0x36;
        /// A shake is in progress (cleared at the start of each step).
        shaking, set_shaking, SHAKING: u8 @ 0x3c;
        /// Non-zero: the left stop does not follow the target.
        hold_min, set_hold_min, HOLD_MIN: u8 @ 0x3d;
        /// Non-zero: no scrolling.
        frozen, set_frozen, FROZEN: u8 @ 0x3e;
        /// Strip step per 256-pixel row.
        row_step, set_row_step, ROW_STEP: u16 @ 0x40;
        /// Shake: rest position, counter, mode to restore.
        shake_x, set_shake_x, SHAKE_X: u16 @ 0x42;
        shake_y, set_shake_y, SHAKE_Y: u16 @ 0x44;
        shake_count, set_shake_count, SHAKE_COUNT: u16 @ 0x46;
        shake_mode, set_shake_mode, SHAKE_MODE: u16 @ 0x48;
        /// Script counters (the first also as a long with the second); the low byte of the first is the variant
        /// passed to the objects that some scripts create.
        timer, set_timer, TIMER: u16 @ 0x4a;
        timers, set_timers, TIMERS: u32 @ 0x4a;
        kind, set_kind, KIND: u8 @ 0x4b;
        timer_2, set_timer_2, TIMER_2: u16 @ 0x4c;
        /// Word cleared at the initialisation of camera 1, read by no code of the port (h).
        spare, set_spare, SPARE: u16 @ 0x4e;
    }
}

/// Cameras 1 (scroll layer 2) and 2 (scroll layer 3).
pub const CAMERA_1: Camera = Camera(Obj(Long::a5(0x4fe).0));
pub const CAMERA_2: Camera = Camera(Obj(Long::a5(0x54e).0));
/// Non-zero: the terrain is drawn instead of the scenery (debug).
const TERRAIN_VIEW: Byte = Byte::a5(0x37);
/// Scroll distance of layer 1 (background movement), as a long.
pub(crate) const BACKDROP: Long = Long::a5(0x7fb2);
pub(crate) const BACKDROP_WORD: Word = Word::a5(0x7fb2);
/// Cells per column and per row copied at each step.
const COLUMN_CELLS: u32 = 19;
const ROW_CELLS: u32 = 35;
/// Terrain tiles displayed in debug mode according to bit 13 of the attribute.
const TERRAIN_TILES: [u16; 2] = [0x1bc0, 0x1d80];
/// Tile offset added to the map cells.
const MAP_TILE_OFFSET: u32 = 0x1800_0000;

/// The scenery layer `i` of `levels.json` (0: scroll layer 2, 1: scroll layer 3): its map, its block banks and its
/// strip tables, read by address as the original computes them.
fn layer(i: usize) -> &'static ddsom_data::levels::Layer {
    &data().levels.layers[i]
}

// ---------------------------------------------------------------------------------------------
// Camera 1: scenery of scroll layer 2
// ---------------------------------------------------------------------------------------------

struct Camera1<'a> {
    b: &'a mut World,
    cam: Camera,
    /// D4 left by the last copy (0x3f: column, 0x40: row), reread by the following routines.
    d4: Option<u32>,
}

/// What follows a copy step: resume the tests, or leave the routine.
enum Step {
    Continue,
    Return,
}

impl Camera1<'_> {
    fn get(&mut self, field: u32) -> u16 {
        self.cam.0.u16(self.b, field)
    }
    fn set(&mut self, field: u32, v: u16) {
        self.cam.0.set_u16(self.b, field, v);
    }
    fn add(&mut self, field: u32, d: u16) {
        let v = self.get(field).wrapping_add(d);
        self.set(field, v);
    }

    /// 0x2f81c: continuation of the mode script: target limits, then scenery copy.
    fn after_mode(&mut self) {
        if self.cam.frozen(self.b) != 0 {
            return;
        }
        if self.get(Camera::MODE) == 4 {
            return self.draw();
        }
        let (limit, mut x) = (self.get(Camera::MAX_X) as i16, self.get(Camera::X) as i16);
        if !(limit >= x) {
            self.set(Camera::X, limit as u16);
            x = limit;
        }
        let behind = x.wrapping_sub(0x60);
        let min = self.get(Camera::MIN_X) as i16;
        if !(behind <= min) {
            if self.cam.hold_min(self.b) == 0 {
                self.set(Camera::MIN_X, behind as u16);
            }
        } else if !(x >= min) {
            self.set(Camera::X, min as u16);
        }
        self.draw();
    }

    /// 0x2f86e: gaps target - drawn, then copy of the missing columns and rows. The four
    /// tests follow one another; after a step, the original resumes at the test of the same direction.
    fn draw(&mut self) {
        let dx = self.get(Camera::X).wrapping_sub(self.get(Camera::DRAWN_X));
        self.set(Camera::DX, dx);
        let dy = self.get(Camera::Y).wrapping_sub(self.get(Camera::DRAWN_Y));
        self.set(Camera::DY, dy);
        let mut test = 0;
        loop {
            match test {
                0 => {
                    if self.get(Camera::DX) as i16 >= 0x10 {
                        self.column(true);
                    } else {
                        test = 1;
                    }
                }
                1 => {
                    if self.get(Camera::DX) as i16 <= -0x10 {
                        if let Step::Return = self.column(false) {
                            return;
                        }
                    } else {
                        test = 2;
                    }
                }
                2 => {
                    if self.get(Camera::DY) as i16 >= 0x10 {
                        self.row(true);
                    } else {
                        test = 3;
                    }
                }
                _ => {
                    if self.get(Camera::DY) as i16 <= -0x10 {
                        if let Step::Return = self.row(false) {
                            return;
                        }
                    } else {
                        return;
                    }
                }
            }
        }
    }

    // --- original subroutines ---

    /// 0x2fdfc: row of the cell to write (offset in the layer) for the bottom of the screen,
    /// drawn y rounded up to the next multiple of 16.
    fn row_offset_down(&mut self) -> u16 {
        let mut y = self.get(Camera::DRAWN_Y);
        if !(y & 0xf == 0) {
            y = y.wrapping_add(0x10);
        }
        layer_row_offset(y.wrapping_add(0x100), y)
    
    }

    /// 0x2fe30: same for the top of the screen (drawn y - 16).
    fn row_offset_up(&mut self) -> u16 {
        let y = self.get(Camera::DRAWN_Y).wrapping_sub(0x10) & 0x3ff;
        layer_row_offset(y, y)
    
    }

    /// 0x2fe5c: column (offset in the layer) to the left of the screen: (drawn x - 0x40) x 4.
    fn column_left(&mut self) -> u16 {
        self.get(Camera::DRAWN_X).wrapping_sub(0x40).wrapping_mul(4)
    }

    /// 0x2fe6a: column to the right of the screen: (drawn x + 0x1e0) x 4.
    fn column_right(&mut self) -> u16 {
        self.get(Camera::DRAWN_X).wrapping_add(0x1e0).wrapping_mul(4)
    }

    /// 0x2fe78 (bottom) / 0x2fec0 (top): map cursor for the row at the bottom / top of the screen. The
    /// strip advances by `ROW_STEP` per 256 pixels; the offset in the strip comes from the high nibble of y.
    fn row_cursor(&mut self, down: bool) {
        let mut y = self.get(Camera::DRAWN_Y);
        // bottom: y rounded up to the next multiple of 16, distance measured 256 pixels lower;
        // top: 16 pixels above
        let (mut distance, nibble_source) = if down {
            if !(y & 0xf == 0) {
                y = y.wrapping_add(0x10);
            }
            (y.wrapping_add(0x100), y)
        } else {
            let above = y.wrapping_sub(0x10);
            (above, above)
        };
        let mut step = self.get(Camera::ROW_STEP);
        if !(distance as i16 >= 0) {
            distance = distance.wrapping_neg();
            step = step.wrapping_neg();
        }
        let strips = distance >> 8;
        self.set(Camera::CURSOR_ROW, step.wrapping_mul(strips).wrapping_neg());
        self.set(Camera::CURSOR_OFFSET, 0x3c - ((nibble_source & 0xf0) >> 2));
    
    }

    /// 0x2ff1c (right column) / 0x2fefa (left column): advances the cursor of the added column.
    fn cursor_for_column(&mut self, right: bool) {
        let x = self.get(Camera::DRAWN_X);
        let column = if right { x.wrapping_add(0x1e0) } else { x.wrapping_sub(0x40) } & 0xfff0;
        self.add(Camera::CURSOR_OFFSET, (column & 0xff).wrapping_mul(4));
        let strip = if right { column >> 8 } else { (column >> 8) as u8 as i8 as u16 };
        self.add(Camera::CURSOR_ROW, strip);
    
    }

    /// 0x2ff66: ROM address of the map strip designated by the cursor (the strip table of the scene, then the
    /// strip in the map of layer 2).
    fn strip_address(&mut self) -> u32 {
        let row = self.get(Camera::CURSOR_ROW);
        strip_address(self.b, layer(0), row, 10)
    }

    // --- copy ---

    /// Map cell at position `at` (address in the index table), in the layer format.
    fn map_cell(&mut self, at: u32) -> u32 {
        let (map, banks) = (layer(0).strips_rom, layer(0).banks_rom);
        let off = at.wrapping_sub(map);
        let bank = engine().long(self.b, banks + (off >> 16 & 0xc));
        let c = off >> 2;
        let index = engine().word(self.b, map + (c & 0xe) + (c >> 1 & 0x3fff0)) as u32;
        let sub = (off as u16 >> 3 & 8) as u32 + (off & 4);
        engine().long(self.b, bank.wrapping_add(index << 4).wrapping_add(sub))
    }

    /// Writes cell `cell` at `at`: map tile, or terrain tile in debug mode.
    fn put_cell(&mut self, at: u32, cell: u32) {
        let at = crate::hw::video::Cell(at);
        if TERRAIN_VIEW.is_set(self.b) {
            let attribute = cell & 0xff80;
            let number = (attribute >> 7) as u16;
            let tile = number.wrapping_add(TERRAIN_TILES[(number & 0x40 != 0) as usize]);
            at.put(self.b, tile, attribute as u16);
        } else {
            at.set_long(self.b, cell.wrapping_add(MAP_TILE_OFFSET));
        }
    }

    /// Column of 19 cells to the right (`right`) or left of the screen, written top to bottom.
    fn column(&mut self, right: bool) -> Step {
        self.add(Camera::DX, if right { 0x10u16.wrapping_neg() } else { 0x10 });
        let drawn = self.get(Camera::DRAWN_X).wrapping_add(if right { 0x10 } else { 0x10u16.wrapping_neg() });
        self.set(Camera::DRAWN_X, drawn);
        if !right && ((drawn as i16) < 0) {
            return Step::Return;
        }
        self.d4 = Some(0x3f);
        let d3 = self.row_offset_down();
        let d2 = if right { self.column_right() } else { self.column_left() };
        let mut a1 = hw::SCROLL2 + (d2 & 0xfc0) as u32;
        self.row_cursor(true);
        self.cursor_for_column(right);
        let mut a2 = self.strip_address();
        let offset = self.get(Camera::CURSOR_OFFSET);
        a2 = a2.wrapping_add(offset as i16 as u32);
        a1 = a1.wrapping_add(d3 as i16 as u32);
        let wrap_offset = offset & 0x3c0;
        for _ in 0..COLUMN_CELLS {
            let cell = self.map_cell(a2);
            a2 += 4;
            self.put_cell(a1, cell);
            a1 += 4;
            if !(a1 as u16 & 0x3f != 0) {
                // end of layer column: next strip of the map, next column of the layer
                let step = self.get(Camera::ROW_STEP);
                self.add(Camera::CURSOR_ROW, step);
                a2 = self.strip_address().wrapping_add(wrap_offset as i16 as u32);
                let next = (a1 as u16).wrapping_add(0xfc0) & 0x3fc0;
                a1 = hw::SCROLL2.wrapping_add(next as i16 as u32);
            }
        }
        Step::Continue
    }

    /// Row of 35 cells at the bottom (`down`) or top of the screen, written left to right.
    fn row(&mut self, down: bool) -> Step {
        self.add(Camera::DY, if down { 0x10u16.wrapping_neg() } else { 0x10 });
        let drawn = self.get(Camera::DRAWN_Y).wrapping_add(if down { 0x10 } else { 0x10u16.wrapping_neg() });
        self.set(Camera::DRAWN_Y, drawn);
        if !down && ((drawn as i16) < 0) {
            return Step::Return;
        }
        self.d4 = Some(0x40);
        let d3 = if down { self.row_offset_down() } else { self.row_offset_up() };
        let d2 = self.column_left();
        let mut column = d2 & 0xfc0;
        let a1 = hw::SCROLL2 + d3 as u32;
        self.row_cursor(down);
        self.cursor_for_column(false);
        let mut a2 = self.strip_address();
        let mut offset = self.get(Camera::CURSOR_OFFSET);
        for _ in 0..ROW_CELLS {
            let cell = self.map_cell(a2.wrapping_add(offset as u32));
            self.put_cell(a1.wrapping_add(column as i16 as u32), cell);
            column = column.wrapping_add(0x40) & 0xfc0;
            offset = offset.wrapping_add(0x40);
            if !(offset & 0x3c0 != 0) {
                offset = offset.wrapping_sub(0x400);
                self.add(Camera::CURSOR_ROW, 1);
                a2 = self.strip_address();
            }
        }
        Step::Continue
    }
}

/// Offset (in scroll layer 2) of the row of cells for ordinate `y`: the layer is
/// divided into pages of 0x1000 bytes (4 pages of 256 pixels), 16 cells of 4 bytes per column.
/// `page` is used to choose the page, `y` the cell within the page.
fn layer_row_offset(page: u16, y: u16) -> u16 {
    0x3000u16.wrapping_sub((page & 0x300) << 4).wrapping_add(0x3c - (((y & 0xf0) >> 2) as u8 as u16))
}

fn camera(b: &mut World) -> Camera1<'_> {
    Camera1 { b, cam: CAMERA_1, d4: None }
}

/// ROM address of the map strip designated by the cursor row `row`: the scene's strip table (its address in the
/// program-space table of the layer, by scene), the strip number in it, then the strip in the map (`1 << shift`
/// bytes per strip as the cursors count them).
fn strip_address(b: &mut World, layer: &ddsom_data::levels::Layer, row: u16, shift: u32) -> u32 {
    let stage = crate::world::ram::SCENE.get(b) as u32;
    let table = engine().pc_long(b, layer.scenes_rom + 4 * stage);
    let strip = row.wrapping_mul(2) as i16 as u32;
    let first = engine().word(b, table.wrapping_add(strip)) as u32;
    layer.strips_rom + (first << shift)
}

/// D4 left by the copy of a scenery column or row: value and bits replaced (`D4 & !mask |
/// value`).
pub type D4Leftovers = Option<(u32, u32)>;

/// 0x2f81c: continuation of camera 1's mode script, after the script.
pub(crate) fn after_mode(b: &mut World) -> D4Leftovers {
    let mut cam = camera(b);
    cam.after_mode();
    cam.d4.map(|v| (v, u32::MAX))
}

/// 0x2f86e: gaps target - drawn of camera 1, then copy of the missing columns and rows.
pub(crate) fn draw1(b: &mut World) -> D4Leftovers {
    let mut cam = camera(b);
    cam.draw();
    cam.d4.map(|v| (v, u32::MAX))
}

// ---------------------------------------------------------------------------------------------
// Camera 2: scenery of scroll layer 3 (32x32-pixel cells, 32-pixel step)
// ---------------------------------------------------------------------------------------------

/// Cells per column and per row copied at each step.
const COLUMN_CELLS_3: u32 = 11;
const ROW_CELLS_3: u32 = 15;
/// Tile offset added to the map cells.
const MAP_TILE_OFFSET_3: u32 = 0x1f00_0000;

struct Camera2<'a> {
    b: &'a mut World,
    cam: Camera,
    /// D4 left by the last copy: 0x1f (long) for a column, 0xe0 (low word) for a row.
    d4: Option<(u32, u32)>,
}

impl Camera2<'_> {
    fn get(&mut self, field: u32) -> u16 {
        self.cam.0.u16(self.b, field)
    }
    fn set(&mut self, field: u32, v: u16) {
        self.cam.0.set_u16(self.b, field, v);
    }
    fn add(&mut self, field: u32, d: u16) {
        let v = self.get(field).wrapping_add(d);
        self.set(field, v);
    }

    /// 0x328a8: gaps target - drawn, then copy of the missing columns and rows (step of 32).
    fn draw(&mut self) {
        let dx = self.get(Camera::X).wrapping_sub(self.get(Camera::DRAWN_X));
        self.set(Camera::DX, dx);
        let dy = self.get(Camera::Y).wrapping_sub(self.get(Camera::DRAWN_Y));
        self.set(Camera::DY, dy);
        let mut test = 0;
        loop {
            match test {
                0 => {
                    if self.get(Camera::DX) as i16 >= 0x20 {
                        self.column(true);
                    } else {
                        test = 1;
                    }
                }
                1 => {
                    if self.get(Camera::DX) as i16 <= -0x20 {
                        if let Step::Return = self.column(false) {
                            return;
                        }
                    } else {
                        test = 2;
                    }
                }
                2 => {
                    if self.get(Camera::DY) as i16 >= 0x20 {
                        self.row(true);
                    } else {
                        test = 3;
                    }
                }
                _ => {
                    if self.get(Camera::DY) as i16 <= -0x20 {
                        if let Step::Return = self.row(false) {
                            return;
                        }
                    } else {
                        return;
                    }
                }
            }
        }
    }

    // --- original subroutines ---

    /// 0x32c2a (bottom) / 0x32c5e (top): offset in the layer of the row of cells, drawn y rounded up
    /// to the next multiple of 32 (bottom) or reduced by 32 (top).
    fn row_offset(&mut self, down: bool) -> u16 {
        let mut y = self.get(Camera::DRAWN_Y);
        let page_source = if down {
            if !(y & 0x1f == 0) {
                y = y.wrapping_add(0x20);
            }
            y.wrapping_add(0x100)
        } else {
            y = y.wrapping_sub(0x20) & 0x7ff;
            y
        };
        0x3800u16.wrapping_sub((page_source & 0x700) << 3).wrapping_add(0x1c - ((y & 0xe0) >> 3))
    
    }

    /// 0x32c8a / 0x32c96: column (x - 0x20 or x + 0x180) to the left / right of the screen.
    fn column_x(&mut self, right: bool) -> u16 {
        let x = self.get(Camera::DRAWN_X);
        if right { x.wrapping_add(0x180) } else { x.wrapping_sub(0x20) }
    
    }

    /// 0x32ca2 (bottom) / 0x32cea (top): map cursor for the bottom / top row.
    fn row_cursor(&mut self, down: bool) {
        let mut y = self.get(Camera::DRAWN_Y);
        let mut step = self.get(Camera::ROW_STEP);
        let (distance, nibble_source) = if down {
            if !(y & 0x1f == 0) {
                y = y.wrapping_add(0x20);
            }
            let mut distance = y.wrapping_add(0x100);
            if !(distance as i16 >= 0) {
                distance = distance.wrapping_neg();
                step = step.wrapping_neg();
            }
            (distance, y)
        } else {
            let mut above = y.wrapping_sub(0x20);
            if !(above as i16 >= 0) {
                above = 0;
            }
            (above, above)
        };
        let strips = distance >> 8;
        self.set(Camera::CURSOR_ROW, step.wrapping_mul(strips).wrapping_neg());
        self.set(Camera::CURSOR_OFFSET, 0x1c - ((nibble_source & 0xe0) >> 3));
    
    }

    /// 0x32d3c (right column) / 0x32d22 (left column): advances the cursor of the added column;
    /// the offset is added to the low byte only (without carry).
    fn cursor_for_column(&mut self, right: bool) {
        let x = self.get(Camera::DRAWN_X);
        let column = if right { x.wrapping_add(0x180) } else { x.wrapping_sub(0x20) } & 0xffe0;
        let low = self.cam.cursor_offset_low(self.b).wrapping_add(column as u8);
        self.cam.set_cursor_offset_low(self.b, low);
        let strip = if right { column >> 8 } else { (column >> 8) as u8 as i8 as u16 };
        self.add(Camera::CURSOR_ROW, strip);
    
    }

    /// 0x32d54: ROM address of the map strip designated by the cursor (map of layer 3).
    fn strip_address(&mut self) -> u32 {
        let row = self.get(Camera::CURSOR_ROW);
        strip_address(self.b, layer(1), row, 8)
    }

    // --- copy ---

    /// Map cell at position `at`; `banks`: address bits that choose the bank.
    fn map_cell(&mut self, at: u32, banks: u32) -> u32 {
        let (map, bank_table) = (layer(1).strips_rom, layer(1).banks_rom);
        let off = at.wrapping_sub(map);
        let bank = engine().long(self.b, bank_table + ((off >> 16 & banks) << 2));
        let c = off >> 2;
        let index = engine().word(self.b, map + (c & 6) + (c >> 1 & !7)) as u32;
        let sub = (off as u16 >> 2 & 8) as u32 + (off & 4);
        engine().long(self.b, bank.wrapping_add(index << 4).wrapping_add(sub)).wrapping_add(MAP_TILE_OFFSET_3)
    }

    /// Column of 11 cells to the right (`right`) or left of the screen, written top to bottom.
    fn column(&mut self, right: bool) -> Step {
        self.add(Camera::DX, if right { 0x20u16.wrapping_neg() } else { 0x20 });
        let old = self.get(Camera::DRAWN_X);
        let drawn = old.wrapping_add(if right { 0x20 } else { 0x20u16.wrapping_neg() });
        self.set(Camera::DRAWN_X, drawn);
        if !right && (old <= 0x20) {
            return Step::Return;
        }
        let d3 = self.row_offset(true);
        self.d4 = Some((0x1f, u32::MAX));
        let d2 = self.column_x(right);
        let mut a1 = hw::SCROLL3 + (d2 & 0x7e0) as u32;
        self.row_cursor(true);
        self.cursor_for_column(right);
        let mut a2 = self.strip_address();
        let offset = self.get(Camera::CURSOR_OFFSET);
        a2 = a2.wrapping_add(offset as i16 as u32);
        a1 = a1.wrapping_add(d3 as i16 as u32);
        let wrap_offset = offset & 0xe0;
        for _ in 0..COLUMN_CELLS_3 {
            let cell = self.map_cell(a2, 0xf);
            a2 += 4;
            Vram(a1).set_long(self.b, cell);
            a1 += 4;
            if !(a1 as u16 & 0x1f != 0) {
                // end of layer column: next strip of the map, next column of the layer
                let step = self.get(Camera::ROW_STEP);
                self.add(Camera::CURSOR_ROW, step);
                a2 = self.strip_address().wrapping_add(wrap_offset as i16 as u32);
                let here = a1 as u16 & 0x3fe0;
                a1 = a1.wrapping_sub(here as i16 as u32);
                a1 = a1.wrapping_add((here.wrapping_add(0x7e0) & 0x3fe0) as i16 as u32);
                if !right && !((a1 as i32) < hw::PALETTES as i32) {
                    a1 = a1.wrapping_sub(0x4000);
                }
            }
        }
        Step::Continue
    }

    /// Row of 15 cells at the bottom (`down`) or top of the screen, written left to right.
    fn row(&mut self, down: bool) -> Step {
        self.add(Camera::DY, if down { 0x20u16.wrapping_neg() } else { 0x20 });
        let old = self.get(Camera::DRAWN_Y);
        let drawn = old.wrapping_add(if down { 0x20 } else { 0x20u16.wrapping_neg() });
        self.set(Camera::DRAWN_Y, drawn);
        if !down {
            if old <= 0x20 {
                return Step::Return;
            }
            if drawn as i16 <= 0 {
                return Step::Continue;
            }
        }
        let d3 = self.row_offset(down);
        self.d4 = Some((0xe0, 0xffff));
        let mut column = self.column_x(false) & 0x7e0;
        let a1 = hw::SCROLL3 + d3 as u32;
        self.row_cursor(down);
        self.cursor_for_column(false);
        let mut a2 = self.strip_address();
        let mut offset = self.get(Camera::CURSOR_OFFSET);
        for _ in 0..ROW_CELLS_3 {
            let cell = self.map_cell(a2.wrapping_add(offset as u32), 1);
            Vram(a1.wrapping_add(column as i16 as u32)).set_long(self.b, cell);
            column = column.wrapping_add(0x20) & 0x7e0;
            offset = offset.wrapping_add(0x20);
            if !(offset & 0xe0 != 0) {
                offset = offset.wrapping_sub(0x100);
                self.add(Camera::CURSOR_ROW, 1);
                a2 = self.strip_address();
            }
        }
        Step::Continue
    }
}

/// 0x328a8: gaps target - drawn of camera 2, then copy of the missing columns and rows.
pub(crate) fn draw2(b: &mut World) -> D4Leftovers {
    let mut cam = Camera2 { b, cam: CAMERA_2, d4: None };
    cam.draw();
    cam.d4
}

// ---------------------------------------------------------------------------------------------
// Parallax scenery: an object moves by a fraction of the frame's scroll
// ---------------------------------------------------------------------------------------------

/// Position (x, y) of a scenery object or a camera that follows the scroll, in 16.16 fixed point (the fields of
/// `Camera`: the parallax scenery objects have the same layout), and the integer position drawn (x).
pub(crate) const OBJECT_X: u32 = Camera::X_LONG;
pub(crate) const OBJECT_Y: u32 = Camera::Y_LONG;
const OBJECT_DRAWN_X: u32 = Camera::DRAWN_X;

/// Scroll of camera 1 since the previous frame (16.16) on axis 0 (x) or 1 (y).
pub(crate) fn scroll_delta(b: &mut World, axis: usize) -> i32 {
    let (now, before) = if axis == 0 {
        (CAMERA_1.x_long(b), CAMERA_1.previous_x_long(b))
    } else {
        (CAMERA_1.y_long(b), CAMERA_1.previous_y_long(b))
    };
    now.wrapping_sub(before) as i32
}

/// Adds `d` to a position of object `o`. Returns the sum (the flags of the addition: `move_object_68k`).
pub(crate) fn move_object(b: &mut World, o: Obj, field: u32, d: i32) -> (u32, u32) {
    let v = o.u32(b, field);
    o.set_u32(b, field, v.wrapping_add(d as u32));
    (d as u32, v)
}

/// Frame scroll for scenery that follows `(fx, fy)` of the scroll (x, y).
pub(crate) fn parallax(b: &mut World, fx: fn(i32) -> i32, fy: fn(i32) -> i32) -> (i32, i32) {
    (fx(scroll_delta(b, 0)), fy(scroll_delta(b, 1)))
}
/// Share of the scroll delta that a parallax scenery follows, in x and in y.
pub(crate) type Parallax = (fn(i32) -> i32, fn(i32) -> i32);
pub(crate) const QUARTER: Parallax = (|dx| dx >> 2, |dy| dy >> 1);
pub(crate) const THREE_QUARTERS: Parallax = (|dx| dx >> 1, |dy| (dy >> 1) + (dy >> 2));

/// 0x33178: scenery `o` follows a quarter of the scroll in x and half in y.
pub(crate) fn parallax_quarter(b: &mut World, o: Obj) {
    let (dx, dy) = parallax(b, QUARTER.0, QUARTER.1);
    move_object(b, o, OBJECT_X, dx);
    move_object(b, o, OBJECT_Y, dy);
}

/// 0x33196: scenery `o` follows half of the scroll in x and three quarters in y.
pub(crate) fn parallax_three_quarters(b: &mut World, o: Obj) {
    let (dx, dy) = parallax(b, THREE_QUARTERS.0, THREE_QUARTERS.1);
    move_object(b, o, OBJECT_X, dx);
    move_object(b, o, OBJECT_Y, dy);
}

/// 0x331e8 / 0x3326c: scenery `o` advances 6 / 3 pixels per frame, its drawn position follows. Returns the new
/// position (flags of the original: `creep_68k`).
pub(crate) fn creep(b: &mut World, o: Obj, pixels: u16) -> u16 {
    let x = o.u16(b, OBJECT_X).wrapping_add(pixels);
    o.set_u16(b, OBJECT_X, x);
    o.set_u16(b, OBJECT_DRAWN_X, x);
    x
}

// ---------------------------------------------------------------------------------------------
// Vertical tracking of the players and update of the two cameras
// ---------------------------------------------------------------------------------------------

/// 0x33aa6: cameras of the scene at their starting position. The two cameras are reset to zero, the start of layer 1
/// cleared (0x3525c); the scene chooses the camera in service (`ram::CAMERA_ALT_ACTIVE`) and the starting position of
/// the first, then the cameras are updated (0x33e20, by the caller: `cameras_start` only does the reset).
pub fn cameras_start(b: &mut World) {
    for cam in [CAMERA_2, CAMERA_1] {
        for field in [Camera::STATE, Camera::X_LONG, Camera::Y_LONG, Camera::DRAWN_X_LONG, Camera::DRAWN_Y_LONG] {
            cam.0.set_u32(b, field, 0);
        }
        cam.0.set_u32(b, Camera::DX, 0);
        if cam == CAMERA_2 {
            CAMERA_2.set_scratch(b, 0);
            CAMERA_2.set_hold_min(b, 0);
        }
    }
    CAMERA_1.set_shaking(b, 0);
    crate::services::video::clear_layer1_start(b);
    let stage = crate::world::ram::SCENE.get(b) as u32;
    let service = crate::world::tables::engine_table!("scroll/camera_service").at8(b, stage);
    crate::world::ram::CAMERA_ALT_ACTIVE.set(b, service);
    CAMERA_1.set_x_fraction(b, 0);
    CAMERA_1.set_y_fraction(b, 0);
    CAMERA_1.set_screen_offset(b, 0);
    let starts = crate::world::tables::engine_table!("scroll/starts");
    let (x, y) = (starts.at16(b, 4 * stage), starts.at16(b, 4 * stage + 2));
    CAMERA_1.set_x(b, x);
    CAMERA_1.set_y(b, y);
    CAMERA_1.set_drawn_x_long(b, 0);
}

/// 0x33b24: cameras on return from an annex zone (exit door SET 7 and doors 27, 29, before they
/// restore the remembered camera). The scene chooses the camera in service (`ram::CAMERA_ALT_ACTIVE`), the right stop
/// of camera 1, the video control word 0x400004 and the displayed layers (0xbc6), then the cameras are put back to
/// their initial position (0x200) and their stops reloaded (per-scene tables), before their update (0x33e20, by the
/// caller).
pub fn cameras_return(b: &mut World) {
    let stage = crate::world::ram::SCENE.get(b) as u32;
    let service = crate::world::tables::engine_table!("scroll/camera_service").at8(b, stage);
    crate::world::ram::CAMERA_ALT_ACTIVE.set(b, service);
    let limit = crate::world::tables::engine_table!("camera_max_x").at16(b, 2 * stage);
    CAMERA_1.set_max_x(b, limit);
    let control = crate::world::tables::engine_table!("camera_output").at16(b, 2 * stage);
    crate::hw::video::regs::OBJECT_OUTPUT.set(b, control);
    let layers = crate::world::tables::engine_table!("camera_layers").at16(b, 2 * stage);
    crate::services::video::LAYER_ENABLE.set(b, 0);
    crate::services::video::turn_layers_on(b, layers);
    let priority = crate::world::tables::engine_table!("camera_hold_min").at8(b, stage);
    CAMERA_1.set_hold_min(b, priority);
    CAMERA_1.set_state(b, 0x200);
    CAMERA_1.set_mode(b, 0);
    CAMERA_1.set_timers(b, 0);
    CAMERA_1.set_spare(b, 0);
    CAMERA_1.set_shaking(b, 0);
    BACKDROP.set(b, 0);
    CAMERA_2.set_state(b, 0x200);
    CAMERA_2.set_mode(b, 0);
    let x = CAMERA_1.x(b);
    CAMERA_1.set_min_x(b, x);
    let bound = crate::world::tables::engine_table!("camera_row_step").at32(b, 4 * stage);
    let v = engine().word(b, bound);
    CAMERA_1.set_row_step(b, v);
    let bound = crate::world::tables::engine_table!("camera2_row_step").at32(b, 4 * stage);
    let v = engine().word(b, bound);
    CAMERA_2.set_row_step(b, v);
    let mode = crate::world::tables::engine_table!("camera_frozen").at8(b, stage);
    CAMERA_1.set_frozen(b, mode);
}
