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

use cps2::Cps2Bus;

use crate::mem::{rd16, rd32, rd8, wr16, wr32, wr8, A5};
use crate::objects::Obj;
use crate::video::hw;
use crate::Native;

/// Fields of the camera record (offsets from its address).
pub(crate) mod cam {
    /// Target position.
    pub const X: u32 = 0x08;
    pub const Y: u32 = 0x0c;
    /// Target of the previous frame.
    pub const PREVIOUS_X: u32 = 0x26;
    pub const PREVIOUS_Y: u32 = 0x2a;
    /// Position already drawn.
    pub const DRAWN_X: u32 = 0x10;
    pub const DRAWN_Y: u32 = 0x14;
    /// Gap target - drawn.
    pub const DX: u32 = 0x1c;
    pub const DY: u32 = 0x1e;
    pub const MODE: u32 = 0x20;
    /// Cursor in the map: offset in the strip (+0x22) and strip number (+0x24).
    pub const CURSOR_OFFSET: u32 = 0x22;
    pub const CURSOR_ROW: u32 = 0x24;
    /// Left stop (does not go back), right stop.
    pub const MIN_X: u32 = 0x2e;
    pub const MAX_X: u32 = 0x34;
    /// Non-zero: the left stop does not follow the target.
    pub const HOLD_MIN: u32 = 0x3d;
    /// Non-zero: no scrolling.
    pub const FROZEN: u32 = 0x3e;
    /// Strip step per 256-pixel row.
    pub const ROW_STEP: u32 = 0x40;
}

/// Records of cameras 1 and 2 (offsets from A5).
pub(crate) const CAMERA_1: u32 = 0x4fe;
pub(crate) const CAMERA_2: u32 = 0x54e;
pub(crate) const STAGE: u32 = 0x34;
const TERRAIN_VIEW: u32 = 0x37;
const MAP: u32 = 0x1f9ba0;
const BANKS: u32 = 0x320f6;
/// Table (in code) of the strip tables per scene.
const ROW_TABLES: u32 = 0x2ff94;
/// Cells per column and per row copied at each step.
const COLUMN_CELLS: u32 = 19;
const ROW_CELLS: u32 = 35;
/// Terrain tiles displayed in debug mode according to bit 13 of the attribute.
const TERRAIN_TILES: [u16; 2] = [0x1bc0, 0x1d80];
/// Tile offset added to the map cells.
const MAP_TILE_OFFSET: u32 = 0x1800_0000;

/// Machine access of a camera.
struct Exec<'a> {
    b: &'a mut Cps2Bus,
}

impl<'a> Exec<'a> {
    fn real(b: &'a mut Cps2Bus) -> Self {
        Exec { b }
    }
}

// ---------------------------------------------------------------------------------------------
// Camera 1: scenery of scroll layer 2
// ---------------------------------------------------------------------------------------------

struct Camera1<'a> {
    x: Exec<'a>,
    a5: u32,
    cam: u32,
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
        rd16(self.x.b, self.cam + field)
    }
    fn set(&mut self, field: u32, v: u16) {
        wr16(self.x.b, self.cam + field, v);
    }
    fn add(&mut self, field: u32, d: u16) {
        let v = self.get(field).wrapping_add(d);
        self.set(field, v);
    }

    /// 0x2f81c: continuation of the mode script: target limits, then scenery copy.
    fn after_mode(&mut self) {
        if rd8(self.x.b, self.cam + cam::FROZEN) != 0 {
            return;
        }
        if self.get(cam::MODE) == 4 {
            return self.draw();
        }
        let (limit, mut x) = (self.get(cam::MAX_X) as i16, self.get(cam::X) as i16);
        if !(limit >= x) {
            self.set(cam::X, limit as u16);
            x = limit;
        }
        let behind = x.wrapping_sub(0x60);
        let min = self.get(cam::MIN_X) as i16;
        if !(behind <= min) {
            if !(rd8(self.x.b, self.cam + cam::HOLD_MIN) != 0) {
                self.set(cam::MIN_X, behind as u16);
            }
        } else if !(x >= min) {
            self.set(cam::X, min as u16);
        }
        self.draw();
    }

    /// 0x2f86e: gaps target - drawn, then copy of the missing columns and rows. The four
    /// tests follow one another; after a step, the original resumes at the test of the same direction.
    fn draw(&mut self) {
        let dx = self.get(cam::X).wrapping_sub(self.get(cam::DRAWN_X));
        self.set(cam::DX, dx);
        let dy = self.get(cam::Y).wrapping_sub(self.get(cam::DRAWN_Y));
        self.set(cam::DY, dy);
        let mut test = 0;
        loop {
            match test {
                0 => {
                    if self.get(cam::DX) as i16 >= 0x10 {
                        self.column(true);
                    } else {
                        test = 1;
                    }
                }
                1 => {
                    if self.get(cam::DX) as i16 <= -0x10 {
                        if let Step::Return = self.column(false) {
                            return;
                        }
                    } else {
                        test = 2;
                    }
                }
                2 => {
                    if self.get(cam::DY) as i16 >= 0x10 {
                        self.row(true);
                    } else {
                        test = 3;
                    }
                }
                _ => {
                    if self.get(cam::DY) as i16 <= -0x10 {
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
        let mut y = self.get(cam::DRAWN_Y);
        if !(y & 0xf == 0) {
            y = y.wrapping_add(0x10);
        }
        layer_row_offset(y.wrapping_add(0x100), y)
    
    }

    /// 0x2fe30: same for the top of the screen (drawn y - 16).
    fn row_offset_up(&mut self) -> u16 {
        let y = self.get(cam::DRAWN_Y).wrapping_sub(0x10) & 0x3ff;
        layer_row_offset(y, y)
    
    }

    /// 0x2fe5c: column (offset in the layer) to the left of the screen: (drawn x - 0x40) x 4.
    fn column_left(&mut self) -> u16 {
        self.get(cam::DRAWN_X).wrapping_sub(0x40).wrapping_mul(4)
    }

    /// 0x2fe6a: column to the right of the screen: (drawn x + 0x1e0) x 4.
    fn column_right(&mut self) -> u16 {
        self.get(cam::DRAWN_X).wrapping_add(0x1e0).wrapping_mul(4)
    }

    /// 0x2fe78 (bottom) / 0x2fec0 (top): map cursor for the row at the bottom / top of the screen. The
    /// strip advances by `ROW_STEP` per 256 pixels; the offset in the strip comes from the high nibble of y.
    fn row_cursor(&mut self, down: bool) {
        let mut y = self.get(cam::DRAWN_Y);
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
        let mut step = self.get(cam::ROW_STEP);
        if !(distance as i16 >= 0) {
            distance = distance.wrapping_neg();
            step = step.wrapping_neg();
        }
        let strips = distance >> 8;
        self.set(cam::CURSOR_ROW, step.wrapping_mul(strips).wrapping_neg());
        self.set(cam::CURSOR_OFFSET, 0x3c - ((nibble_source & 0xf0) >> 2));
    
    }

    /// 0x2ff1c (right column) / 0x2fefa (left column): advances the cursor of the added column.
    fn cursor_for_column(&mut self, right: bool) {
        let x = self.get(cam::DRAWN_X);
        let column = if right { x.wrapping_add(0x1e0) } else { x.wrapping_sub(0x40) } & 0xfff0;
        self.add(cam::CURSOR_OFFSET, (column & 0xff).wrapping_mul(4));
        let strip = if right { column >> 8 } else { (column >> 8) as u8 as i8 as u16 };
        self.add(cam::CURSOR_ROW, strip);
    
    }

    /// 0x2ff66: ROM address of the map strip designated by the cursor.
    fn strip_address(&mut self) -> u32 {
        let stage = rd8(self.x.b, self.a5 + STAGE) as u32;
        let table = crate::mem::prog16(self.x.b, ROW_TABLES + 4 * stage) as u32;
        let table = table << 16 | crate::mem::prog16(self.x.b, ROW_TABLES + 4 * stage + 2) as u32;
        let strip = self.get(cam::CURSOR_ROW).wrapping_mul(2) as i16 as u32;
        let first = rd16(self.x.b, table.wrapping_add(strip)) as u32;
        MAP + (first << 10)
    
    }

    // --- copy ---

    /// Map cell at position `at` (address in the index table), in the layer format.
    fn map_cell(&mut self, at: u32) -> u32 {
        let off = at.wrapping_sub(MAP);
        let bank = rd32(self.x.b, BANKS + (off >> 16 & 0xc));
        let c = off >> 2;
        let index = rd16(self.x.b, MAP + (c & 0xe) + (c >> 1 & 0x3fff0)) as u32;
        let sub = (off as u16 >> 3 & 8) as u32 + (off & 4);
        rd32(self.x.b, bank.wrapping_add(index << 4).wrapping_add(sub))
    }

    /// Writes cell `cell` at `at`: map tile, or terrain tile in debug mode.
    fn put_cell(&mut self, at: u32, cell: u32) {
        let terrain_view = rd8(self.x.b, self.a5 + TERRAIN_VIEW) != 0;
        if !(!terrain_view) {
            let attribute = cell & 0xff80;
            let number = (attribute >> 7) as u16;
            let tile = number.wrapping_add(TERRAIN_TILES[(number & 0x40 != 0) as usize]);
            wr16(self.x.b, at, tile);
            wr16(self.x.b, at + 2, attribute as u16);
        } else {
            wr32(self.x.b, at, cell.wrapping_add(MAP_TILE_OFFSET));
        }
    }

    /// Column of 19 cells to the right (`right`) or left of the screen, written top to bottom.
    fn column(&mut self, right: bool) -> Step {
        self.add(cam::DX, if right { 0x10u16.wrapping_neg() } else { 0x10 });
        let drawn = self.get(cam::DRAWN_X).wrapping_add(if right { 0x10 } else { 0x10u16.wrapping_neg() });
        self.set(cam::DRAWN_X, drawn);
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
        let offset = self.get(cam::CURSOR_OFFSET);
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
                let step = self.get(cam::ROW_STEP);
                self.add(cam::CURSOR_ROW, step);
                a2 = self.strip_address().wrapping_add(wrap_offset as i16 as u32);
                let next = (a1 as u16).wrapping_add(0xfc0) & 0x3fc0;
                a1 = hw::SCROLL2.wrapping_add(next as i16 as u32);
            }
        }
        Step::Continue
    }

    /// Row of 35 cells at the bottom (`down`) or top of the screen, written left to right.
    fn row(&mut self, down: bool) -> Step {
        self.add(cam::DY, if down { 0x10u16.wrapping_neg() } else { 0x10 });
        let drawn = self.get(cam::DRAWN_Y).wrapping_add(if down { 0x10 } else { 0x10u16.wrapping_neg() });
        self.set(cam::DRAWN_Y, drawn);
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
        let mut offset = self.get(cam::CURSOR_OFFSET);
        for _ in 0..ROW_CELLS {
            let cell = self.map_cell(a2.wrapping_add(offset as u32));
            self.put_cell(a1.wrapping_add(column as i16 as u32), cell);
            column = column.wrapping_add(0x40) & 0xfc0;
            offset = offset.wrapping_add(0x40);
            if !(offset & 0x3c0 != 0) {
                offset = offset.wrapping_sub(0x400);
                self.add(cam::CURSOR_ROW, 1);
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

fn camera(b: &mut Cps2Bus) -> Camera1<'_> {
    Camera1 { x: Exec::real(b), a5: A5, cam: A5 + CAMERA_1, d4: None }
}

/// D4 left by the copy of a scenery column or row: value and bits replaced (`D4 & !mask |
/// value`).
pub type D4Leftovers = Option<(u32, u32)>;

/// 0x2f81c: continuation of camera 1's mode script, after the script.
pub(crate) fn after_mode(b: &mut Cps2Bus) -> D4Leftovers {
    let mut cam = camera(b);
    cam.after_mode();
    cam.d4.map(|v| (v, u32::MAX))
}

/// 0x2f86e: gaps target - drawn of camera 1, then copy of the missing columns and rows.
pub(crate) fn draw1(b: &mut Cps2Bus) -> D4Leftovers {
    let mut cam = camera(b);
    cam.draw();
    cam.d4.map(|v| (v, u32::MAX))
}

// ---------------------------------------------------------------------------------------------
// Camera 2: scenery of scroll layer 3 (32x32-pixel cells, 32-pixel step)
// ---------------------------------------------------------------------------------------------

/// Scenery map of camera 2 (indices of 2x2-cell blocks) and its 16 block banks.
const MAP_3: u32 = 0x2459c0;
const BANKS_3: u32 = 0x33a9c;
/// Table (in code) of the strip tables per scene.
const ROW_TABLES_3: u32 = 0x32d7c;
/// Cells per column and per row copied at each step.
const COLUMN_CELLS_3: u32 = 11;
const ROW_CELLS_3: u32 = 15;
/// Tile offset added to the map cells.
const MAP_TILE_OFFSET_3: u32 = 0x1f00_0000;

struct Camera2<'a> {
    x: Exec<'a>,
    a5: u32,
    cam: u32,
    /// D4 left by the last copy: 0x1f (long) for a column, 0xe0 (low word) for a row.
    d4: Option<(u32, u32)>,
}

impl Camera2<'_> {

    fn get(&mut self, field: u32) -> u16 {
        rd16(self.x.b, self.cam + field)
    }
    fn set(&mut self, field: u32, v: u16) {
        wr16(self.x.b, self.cam + field, v);
    }
    fn add(&mut self, field: u32, d: u16) {
        let v = self.get(field).wrapping_add(d);
        self.set(field, v);
    }

    /// 0x328a8: gaps target - drawn, then copy of the missing columns and rows (step of 32).
    fn draw(&mut self) {
        let dx = self.get(cam::X).wrapping_sub(self.get(cam::DRAWN_X));
        self.set(cam::DX, dx);
        let dy = self.get(cam::Y).wrapping_sub(self.get(cam::DRAWN_Y));
        self.set(cam::DY, dy);
        let mut test = 0;
        loop {
            match test {
                0 => {
                    if self.get(cam::DX) as i16 >= 0x20 {
                        self.column(true);
                    } else {
                        test = 1;
                    }
                }
                1 => {
                    if self.get(cam::DX) as i16 <= -0x20 {
                        if let Step::Return = self.column(false) {
                            return;
                        }
                    } else {
                        test = 2;
                    }
                }
                2 => {
                    if self.get(cam::DY) as i16 >= 0x20 {
                        self.row(true);
                    } else {
                        test = 3;
                    }
                }
                _ => {
                    if self.get(cam::DY) as i16 <= -0x20 {
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
        let mut y = self.get(cam::DRAWN_Y);
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
        let x = self.get(cam::DRAWN_X);
        if right { x.wrapping_add(0x180) } else { x.wrapping_sub(0x20) }
    
    }

    /// 0x32ca2 (bottom) / 0x32cea (top): map cursor for the bottom / top row.
    fn row_cursor(&mut self, down: bool) {
        let mut y = self.get(cam::DRAWN_Y);
        let mut step = self.get(cam::ROW_STEP);
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
        self.set(cam::CURSOR_ROW, step.wrapping_mul(strips).wrapping_neg());
        self.set(cam::CURSOR_OFFSET, 0x1c - ((nibble_source & 0xe0) >> 3));
    
    }

    /// 0x32d3c (right column) / 0x32d22 (left column): advances the cursor of the added column;
    /// the offset is added to the low byte only (without carry).
    fn cursor_for_column(&mut self, right: bool) {
        let x = self.get(cam::DRAWN_X);
        let column = if right { x.wrapping_add(0x180) } else { x.wrapping_sub(0x20) } & 0xffe0;
        let low = rd8(self.x.b, self.cam + cam::CURSOR_OFFSET + 1).wrapping_add(column as u8);
        wr8(self.x.b, self.cam + cam::CURSOR_OFFSET + 1, low);
        let strip = if right { column >> 8 } else { (column >> 8) as u8 as i8 as u16 };
        self.add(cam::CURSOR_ROW, strip);
    
    }

    /// 0x32d54: ROM address of the map strip designated by the cursor.
    fn strip_address(&mut self) -> u32 {
        let stage = rd8(self.x.b, self.a5 + STAGE) as u32;
        let table = (crate::mem::prog16(self.x.b, ROW_TABLES_3 + 4 * stage) as u32) << 16 | crate::mem::prog16(self.x.b, ROW_TABLES_3 + 4 * stage + 2) as u32;
        let strip = self.get(cam::CURSOR_ROW).wrapping_mul(2) as i16 as u32;
        let first = rd16(self.x.b, table.wrapping_add(strip)) as u32;
        MAP_3 + (first << 8)
    
    }

    // --- copy ---

    /// Map cell at position `at`; `banks`: address bits that choose the bank.
    fn map_cell(&mut self, at: u32, banks: u32) -> u32 {
        let off = at.wrapping_sub(MAP_3);
        let bank = rd32(self.x.b, BANKS_3 + ((off >> 16 & banks) << 2));
        let c = off >> 2;
        let index = rd16(self.x.b, MAP_3 + (c & 6) + (c >> 1 & !7)) as u32;
        let sub = (off as u16 >> 2 & 8) as u32 + (off & 4);
        rd32(self.x.b, bank.wrapping_add(index << 4).wrapping_add(sub)).wrapping_add(MAP_TILE_OFFSET_3)
    }

    /// Column of 11 cells to the right (`right`) or left of the screen, written top to bottom.
    fn column(&mut self, right: bool) -> Step {
        self.add(cam::DX, if right { 0x20u16.wrapping_neg() } else { 0x20 });
        let old = self.get(cam::DRAWN_X);
        let drawn = old.wrapping_add(if right { 0x20 } else { 0x20u16.wrapping_neg() });
        self.set(cam::DRAWN_X, drawn);
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
        let offset = self.get(cam::CURSOR_OFFSET);
        a2 = a2.wrapping_add(offset as i16 as u32);
        a1 = a1.wrapping_add(d3 as i16 as u32);
        let wrap_offset = offset & 0xe0;
        for _ in 0..COLUMN_CELLS_3 {
            let cell = self.map_cell(a2, 0xf);
            a2 += 4;
            wr32(self.x.b, a1, cell);
            a1 += 4;
            if !(a1 as u16 & 0x1f != 0) {
                // end of layer column: next strip of the map, next column of the layer
                let step = self.get(cam::ROW_STEP);
                self.add(cam::CURSOR_ROW, step);
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
        self.add(cam::DY, if down { 0x20u16.wrapping_neg() } else { 0x20 });
        let old = self.get(cam::DRAWN_Y);
        let drawn = old.wrapping_add(if down { 0x20 } else { 0x20u16.wrapping_neg() });
        self.set(cam::DRAWN_Y, drawn);
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
        let mut offset = self.get(cam::CURSOR_OFFSET);
        for _ in 0..ROW_CELLS_3 {
            let cell = self.map_cell(a2.wrapping_add(offset as u32), 1);
            wr32(self.x.b, a1.wrapping_add(column as i16 as u32), cell);
            column = column.wrapping_add(0x20) & 0x7e0;
            offset = offset.wrapping_add(0x20);
            if !(offset & 0xe0 != 0) {
                offset = offset.wrapping_sub(0x100);
                self.add(cam::CURSOR_ROW, 1);
                a2 = self.strip_address();
            }
        }
        Step::Continue
    }
}

/// 0x328a8: gaps target - drawn of camera 2, then copy of the missing columns and rows.
pub(crate) fn draw2(b: &mut Cps2Bus) -> D4Leftovers {
    let mut cam = Camera2 { x: Exec::real(b), a5: A5, cam: A5 + CAMERA_2, d4: None };
    cam.draw();
    cam.d4
}

// ---------------------------------------------------------------------------------------------
// Parallax scenery: an object moves by a fraction of the frame's scroll
// ---------------------------------------------------------------------------------------------

/// Current scroll (x, y) of the camera, in 16.16 fixed point, and that of the previous frame.
const SCROLL: [u32; 2] = [0x506, 0x50a];
const PREVIOUS_SCROLL: [u32; 2] = [0x524, 0x528];
/// Position (x, y) of an object in 16.16 fixed point.
pub(crate) const OBJECT_X: u32 = 0x08;
pub(crate) const OBJECT_Y: u32 = 0x0c;
/// Integer position drawn (x) of an object.
const OBJECT_DRAWN_X: u32 = 0x10;

/// Scroll of the frame on axis 0 (x) or 1 (y).
pub(crate) fn scroll_delta(b: &mut Cps2Bus, a5: u32, axis: usize) -> i32 {
    crate::mem::rd32(b, a5 + SCROLL[axis]).wrapping_sub(crate::mem::rd32(b, a5 + PREVIOUS_SCROLL[axis])) as i32
}

/// Adds `d` to a position of object `o`. Returns the sum (the flags of the addition: `move_object_68k`).
pub(crate) fn move_object(b: &mut Cps2Bus, o: Obj, field: u32, d: i32) -> (u32, u32) {
    let at = o.0 + field;
    let v = crate::mem::rd32(b, at);
    crate::mem::wr32(b, at, v.wrapping_add(d as u32));
    (d as u32, v)
}

/// Frame scroll for scenery that follows `(fx, fy)` of the scroll (x, y).
pub(crate) fn parallax(b: &mut Cps2Bus, fx: fn(i32) -> i32, fy: fn(i32) -> i32) -> (i32, i32) {
    (fx(scroll_delta(b, A5, 0)), fy(scroll_delta(b, A5, 1)))
}
pub(crate) const QUARTER: (fn(i32) -> i32, fn(i32) -> i32) = (|dx| dx >> 2, |dy| dy >> 1);
pub(crate) const THREE_QUARTERS: (fn(i32) -> i32, fn(i32) -> i32) = (|dx| dx >> 1, |dy| (dy >> 1) + (dy >> 2));

/// 0x33178: scenery `o` follows a quarter of the scroll in x and half in y.
pub(crate) fn parallax_quarter(b: &mut Cps2Bus, o: Obj) {
    let (dx, dy) = parallax(b, QUARTER.0, QUARTER.1);
    move_object(b, o, OBJECT_X, dx);
    move_object(b, o, OBJECT_Y, dy);
}

/// 0x33196: scenery `o` follows half of the scroll in x and three quarters in y.
pub(crate) fn parallax_three_quarters(b: &mut Cps2Bus, o: Obj) {
    let (dx, dy) = parallax(b, THREE_QUARTERS.0, THREE_QUARTERS.1);
    move_object(b, o, OBJECT_X, dx);
    move_object(b, o, OBJECT_Y, dy);
}

/// 0x331e8 / 0x3326c: scenery `o` advances 6 / 3 pixels per frame, its drawn position follows. Returns the new
/// position (flags of the original: `creep_68k`).
pub(crate) fn creep(b: &mut Cps2Bus, o: Obj, pixels: u16) -> u16 {
    let x = crate::mem::rd16(b, o.0 + OBJECT_X).wrapping_add(pixels);
    crate::mem::wr16(b, o.0 + OBJECT_X, x);
    crate::mem::wr16(b, o.0 + OBJECT_DRAWN_X, x);
    x
}

pub const NATIVES: &[Native] = &[];

// ---------------------------------------------------------------------------------------------
// Vertical tracking of the players and update of the two cameras
// ---------------------------------------------------------------------------------------------

/// 0x33e20: updates the two cameras. The first remembers its previous target (+0x26, +0x2a)
/// before its mode script (0x2ede0) moves it; the second is processed by 0x32108.
/// Returns the D4 left by the scenery copies of the two cameras (in order).
pub fn update_cameras(b: &mut Cps2Bus) -> [D4Leftovers; 2] {
    let camera = A5 + CAMERA_1;
    let (x, y) = (rd32(b, camera + cam::X), rd32(b, camera + cam::Y));
    wr32(b, camera + cam::PREVIOUS_X, x);
    wr32(b, camera + cam::PREVIOUS_Y, y);
    [crate::camera_modes::update_camera_1(b), crate::camera_modes::update_camera_2(b)]
}

/// `update_cameras` in the game. Original glitch: the D4 of the scenery copies is reread by a following handler
/// (boss13_dragon, frame 7241): it is the leftover D4 of the game (`dispatch::Leftovers`); A0 (camera 2) is reread by
/// nobody.
pub fn update_cameras_in_game(b: &mut Cps2Bus) {
    for (v, m) in update_cameras(b).into_iter().flatten() {
        let leftovers = &mut crate::tasks::table::table(b).leftovers;
        leftovers.d4 = leftovers.d4 & !m | v;
    }
}

/// Table (per scene) of the camera in service, and starting positions (x, y) of the first camera.
const SERVICE: u32 = 0x33bdc;
pub(crate) const STARTS: u32 = 0x33c54;

/// 0x33aa6: cameras of the scene at their starting position. The two cameras (`A5+0x4fe`, `A5+0x54e`)
/// are reset to zero, the start of layer 1 cleared (0x3525c); the scene (`A5+0x34`) chooses the camera
/// in service (`A5+0x36`, table 0x33bdc) and the starting position of the first (table 0x33c54: x, y),
/// then the cameras are updated (0x33e20, by the caller: `cameras_start` only does the reset).
pub fn cameras_start(b: &mut Cps2Bus) {
    for off in [0x54e, 0x556, 0x55a, 0x55e, 0x562, 0x56a, 0x4fe, 0x506, 0x50a, 0x50e, 0x512, 0x51a] {
        wr32(b, A5 + off, 0);
    }
    wr16(b, A5 + 0x566, 0);
    wr8(b, 0xff858b, 0);
    wr8(b, A5 + 0x53a, 0);
    crate::video::clear_layer1_start(b);
    let stage = rd8(b, A5 + STAGE) as u32;
    let service = rd8(b, SERVICE + stage);
    wr8(b, A5 + 0x36, service);
    for off in [0x508, 0x50c, 0x52e] {
        wr16(b, A5 + off, 0);
    }
    let start = STARTS + 4 * stage;
    let (x, y) = (rd16(b, start), rd16(b, start + 2));
    wr16(b, A5 + 0x506, x);
    wr16(b, A5 + 0x50a, y);
    wr32(b, A5 + 0x50e, 0);
}

/// 0x33b24: cameras on return from an annex zone (exit door SET 7 and doors 27, 29, before they
/// restore the remembered camera). The scene (`A5+0x34`) chooses the camera in service (`A5+0x36`, table
/// 0x33bdc), a limit (`A5+0x532`), the video control word 0x400004 and the displayed layers (0xbc6),
/// then the cameras are put back to their initial position (0x200) and their stops reloaded (per-scene
/// tables, data space), before their update (0x33e20, by the caller).
pub fn cameras_return(b: &mut Cps2Bus) {
    use crate::camera_modes::tables::{
        FROZEN as MODES, HOLD_MIN as PRIORITIES, LAYERS, MAX_X_1 as LIMITS, OUTPUT as VIDEO_CONTROL,
        ROW_STEP_1 as BOUNDS_A, ROW_STEP_2 as BOUNDS_B,
    };
    use crate::camera_modes::OUTPUT_PORT as VIDEO_CONTROL_PORT;
    let stage = rd8(b, A5 + STAGE) as u32;
    let service = rd8(b, SERVICE + stage);
    wr8(b, A5 + 0x36, service);
    let limit = rd16(b, LIMITS + 2 * stage);
    wr16(b, A5 + 0x532, limit);
    let control = rd16(b, VIDEO_CONTROL + 2 * stage);
    wr16(b, VIDEO_CONTROL_PORT, control);
    let layers = rd16(b, LAYERS + 2 * stage);
    wr16(b, A5 + 0x14, 0);
    crate::video::turn_layers_on(b, layers);
    let priority = rd8(b, PRIORITIES + stage);
    wr8(b, A5 + 0x53b, priority);
    wr16(b, A5 + 0x4fe, 0x200);
    wr16(b, A5 + 0x51e, 0);
    wr32(b, A5 + 0x548, 0);
    wr16(b, A5 + 0x54c, 0);
    wr8(b, A5 + 0x53a, 0);
    wr32(b, A5 + 0x7fb2, 0);
    wr16(b, A5 + 0x54e, 0x200);
    wr16(b, A5 + 0x56e, 0);
    let x = rd16(b, A5 + 0x506);
    wr16(b, A5 + 0x52c, x);
    let bound = rd32(b, BOUNDS_A + 4 * stage);
    let v = rd16(b, bound);
    wr16(b, A5 + 0x53e, v);
    let bound = rd32(b, BOUNDS_B + 4 * stage);
    let v = rd16(b, bound);
    wr16(b, A5 + 0x58e, v);
    let mode = rd8(b, MODES + stage);
    wr8(b, A5 + 0x53c, mode);
}
