//! Mode scripts of camera 1 (B1): each frame, before the scenery copy (`scroll`), the
//! script of the current mode (`MODE`, multiple of 4) moves the camera's target position, follows the
//! players or animates the scene (per-line scroll tables, shakes...). Each script
//! ends, as the case may be, with the per-scene dispatch routine (`stage_script`, 0x309e0) which can
//! change the mode.
//!
//! The cameras are work RAM records (`scroll::CAMERA_1` / `CAMERA_2`);
//! `Cam::o` designates the one the script modifies (a few scripts act on camera 2). The scripts only
//! touch memory: no register or processor ([`Cam`]).

use crate::world::World;

use crate::families::cx::Cx;
use crate::hw::video::{regs, Vram};
use crate::machine::Machine;
use crate::objects::pools;
use crate::objects::{Current, Obj};
use crate::player::Player;
use crate::ram::{self, Word};
use crate::scroll::{Camera, BACKDROP, BACKDROP_WORD, CAMERA_1, CAMERA_2};
use crate::tables::engine;
use crate::video::Grid;
use crate::Native;

mod camera2;
mod follow;
mod modes;
mod stage;
use camera2::*;
use follow::*;
use modes::*;
use stage::*;

/// Context of a camera script: the bus and the camera that the script modifies (`x.o`; a few scripts act on camera
/// 2). The fields are those of `scroll::Camera`; the object engine (`x.move_z()`, `x.alloc(pool)`) and the random
/// draw (`x.random()`) are those of every object handler.
type Cam<'a> = Cx<'a, ()>;

/// Context of a script on camera `cam`.
fn on(b: &mut World, cam: Camera) -> Cam<'_> {
    Cx { c: (), b, o: cam.0 }
}

/// Modes (field `Camera::MODE`).
mod mode {
    pub const STAGE: u16 = 0x00;
    pub const FREE: u16 = 0x04;
    pub const FOLLOW: u16 = 0x08;
    pub const SKY_GRADIENT: u16 = 0x0c;
    pub const FOLLOW_X: u16 = 0x14;
    pub const RIPPLE: u16 = 0x18;
    pub const FOLLOW_SYNC: u16 = 0x1c;
    pub const WAVES: u16 = 0x2c;
    pub const DUAL_FOLLOW: u16 = 0x38;
    pub const WALL_RIGHT: u16 = 0x3c;
    pub const GATE: u16 = 0x40;
    pub const PUSH: u16 = 0x44;
    pub const PARKED: u16 = 0x48;
    pub const WALL_FAR: u16 = 0x4c;
    pub const GATE_FAR: u16 = 0x50;
    pub const SNAP: u16 = 0x64;
    pub const FOLLOW_X_SYNC: u16 = 0x74;
    pub const SCREEN_OBJECT: u16 = 0x7c;
    pub const WAVES_LEFT: u16 = 0x84;
    pub const FOLLOW_MID: u16 = 0x88;
}

/// Per-line scroll table in GFX RAM (two halves, one per frame parity of the waves).
const ROW_SCROLL: Vram = crate::hw::video::SCRATCH;
const ROW_SCROLL_B: Vram = ROW_SCROLL.offset(0x200);
/// Copy of the video control register (`regs::VIDEO_CONTROL`), bit 0: per-line scroll.
use crate::video::VIDEO_CONTROL as VIDEO_CONTROL_COPY;

// ---------------------------------------------------------------------------------------------
// Initialisation and update of the cameras
// ---------------------------------------------------------------------------------------------

/// Tile blocks (index, address in layer 2) loaded at the entry of certain scenes, camera 1.
fn background_blocks_1(stage: u8) -> Option<Vec<(u16, u32)>> {
    const SLOTS: [u32; 4] = [0x917000, 0x917400, 0x917800, 0x917c00];
    let indices: &[u16] = match stage {
        8 | 41 => &[130, 131, 130, 131],
        54 | 59 => &[173, 173, 173, 173],
        69 => &[211, 212, 213],
        71 => &[216, 217, 218],
        72 => &[222, 223, 224],
        76 => &[231, 232, 233],
        78 => &[239, 240, 241],
        80 => &[244, 245, 246],
        81 => &[219, 220, 221],
        82 => &[234, 235, 236],
        _ => return None,
    };
    Some(indices.iter().zip(SLOTS).map(|(&index, slot)| (index, slot)).collect())
}

/// 0x2ede0: camera 1 (A5+0x4fe) is initialised at the first call of a scene, then updated each frame.
/// Returns the D4 left by the scenery copy.
pub fn update_camera_1(b: &mut World) -> crate::scroll::D4Leftovers {
    if CAMERA_1.active(b) != 0 {
        step(b)
    } else {
        init_camera_1(b)
    }
}

fn init_camera_1(b: &mut World) -> crate::scroll::D4Leftovers {
    let mut x = on(b, CAMERA_1);
    x.set16(Camera::STATE, 0x0200);
    let stage = ram::SCENE.get(x.b);
    let by_stage = 2 * stage as u32;
    let max_x = crate::tables::engine_table!("camera_max_x").at16(x.b, by_stage);
    x.set16(Camera::MAX_X, max_x);
    let output = crate::tables::engine_table!("camera_output").at16(x.b, by_stage);
    regs::OBJECT_OUTPUT.set(x.b, output);
    crate::video::LAYER_ENABLE.set(x.b, 0);
    let layers = crate::tables::engine_table!("camera_layers").at16(x.b, by_stage);
    crate::video::turn_layers_on(x.b, layers);
    let hold = crate::tables::engine_table!("camera_hold_min").at8(x.b, stage as u32);
    x.set8(Camera::HOLD_MIN, hold);
    x.set16(Camera::MODE, 0);
    x.set32(Camera::TIMERS, 0);
    x.set16(Camera::SPARE, 0);
    x.set8(Camera::SHAKING, 0);
    let y = x.u16(Camera::Y);
    x.set16(Camera::DRAWN_Y, y);
    x.set16(Camera::SCRATCH, 0x28);
    x.set8(Camera::STEP, 0);
    BACKDROP.set(x.b, 0);
    // the left stop takes the table's value if it is positive, otherwise it is inverted
    let min = x.u16(Camera::MIN_X) as i16;
    let min = if min >= 0 { crate::tables::engine_table!("camera_min_x").at16(x.b, by_stage) } else { min.wrapping_neg() as u16 };
    x.set16(Camera::MIN_X, min);
    let pointer = crate::tables::engine_table!("camera_row_step").at32(x.b, 2 * by_stage);
    let row_step = engine().word(x.b, pointer);
    x.set16(Camera::ROW_STEP, row_step);
    let frozen = crate::tables::engine_table!("camera_frozen").at8(x.b, stage as u32);
    x.set8(Camera::FROZEN, frozen);
    if let Some(blocks) = background_blocks_1(stage) {
        if stage == 8 || stage == 41 {
            clear_rowscroll_flag(&mut x);
        }
        load_backdrops(&mut x, &blocks);
        return None;
    }
    clear_rowscroll_flag(&mut x);
    let (px, py) = (x.u16(Camera::X), x.u16(Camera::Y));
    x.set16(Camera::DRAWN_X, px);
    x.set16(Camera::DRAWN_Y, py.wrapping_sub(0x160));
    crate::scroll::draw1(x.b)
}

/// Destinations (layer 3) of the eight blocks loaded by scenes 54 and 59.
const BACKDROP_SLOTS_2: [u32; 8] = [0x91b800, 0x91b900, 0x91b000, 0x91b100, 0x91a800, 0x91a900, 0x91a000, 0x91a100];

/// Loads into layer 3 tile blocks designated by a table `camera2/blocks@…` (block, offset from the start of the layer);
/// camera 2 is already positioned.
fn load_scroll3_table(x: &mut Cam, table: &str, count: u32) {
    x.set8(Camera::SUB_STEP, 0);
    let px = x.u16(Camera::X);
    x.set16(Camera::DRAWN_X, px);
    let blocks = engine().get(table);
    for i in 0..count {
        let n = blocks.at16(x.b, 4 * i);
        let destination = crate::video::hw::SCROLL3.wrapping_add(blocks.at16(x.b, 4 * i + 2) as i16 as i32 as u32);
        crate::video::load_grid(x.b, Grid::Layer3, n, destination);
    }
}

/// Loads consecutive blocks of indices `first..` at the addresses `slots`.
fn load_scroll3_blocks(x: &mut Cam, first: u32, slots: [u32; 8]) {
    for (i, slot) in slots.into_iter().enumerate() {
        crate::video::load_grid(x.b, Grid::Layer3, (first + i as u32) as u16, slot);
    }
}

/// 0x32108: camera 2 (A5+0x54e) is initialised at the first call of a scene, then updated each frame.
/// Returns the D4 left by the scenery copy.
pub fn update_camera_2(b: &mut World) -> crate::scroll::D4Leftovers {
    if CAMERA_2.active(b) != 0 {
        step_2(b)
    } else {
        init_camera_2(b)
    }
}

fn init_camera_2(b: &mut World) -> crate::scroll::D4Leftovers {
    let mut x = on(b, CAMERA_2);
    x.set16(Camera::STATE, 0x0200);
    x.set16(Camera::MODE, 0);
    x.set16(Camera::MIN_X, 0);
    let stage = ram::SCENE.get(x.b) as u32;
    let pointer = crate::tables::engine_table!("camera2_row_step").at32(x.b, 4 * stage);
    let row_step = engine().word(x.b, pointer);
    x.set16(Camera::ROW_STEP, row_step);
    x.set16(Camera::X_FRACTION, 0);
    x.set16(Camera::Y_FRACTION, 0);
    let starts = crate::tables::engine_table!("camera2_start");
    let (start_x, start_y) = (starts.at16(x.b, 4 * stage), starts.at16(x.b, 4 * stage + 2));
    x.set16(Camera::X, start_x);
    x.set16(Camera::Y, start_y);
    match stage {
        4 | 91 => load_scroll3_table(&mut x, "camera2/blocks@0x321fc", 24),
        6 => load_scroll3_table(&mut x, "camera2/blocks@0x32284", 8),
        12 | 101 => load_scroll3_table(&mut x, "camera2/blocks@0x322cc", 8),
        18 => load_scroll3_table(&mut x, "camera2/blocks@0x32314", 8),
        8 | 41 => {
            for i in 0..8 {
                crate::video::load_grid(x.b, Grid::Layer3, 53, 0x91b800 + 0x100 * i);
            }
        }
        54 => load_scroll3_blocks(&mut x, 16, BACKDROP_SLOTS_2),
        59 => load_scroll3_blocks(&mut x, 39, BACKDROP_SLOTS_2),
        _ => {
            let (px, py) = (x.u16(Camera::X), x.u16(Camera::Y));
            x.set16(Camera::DRAWN_Y, py);
            x.set16(Camera::DRAWN_X, px.wrapping_add(0x200) & 0xffe0);
            return crate::scroll::draw2(x.b);
        }
    }
    None
}

/// 0x2f80c: one step of the initialised camera 1: mode script, then target stops and scenery
/// copy (`scroll`).
fn step(b: &mut World) -> crate::scroll::D4Leftovers {
    let mut x = on(b, CAMERA_1);
    x.set8(Camera::SHAKING, 0);
    let m = x.u16(Camera::MODE);
    run_mode(&mut x, m);
    crate::scroll::after_mode(x.b)
}

/// Camera shake modes (`Camera::MODE`) and their original entries.
pub mod shake {
    /// 0x2824.
    pub const A: u16 = 0x54;
    /// 0x2860.
    pub const B: u16 = 0x58;
    /// 0x289c.
    pub const C: u16 = 0x5c;

    /// The shake mode whose routine is given by a ROM table.
    pub fn of_routine(routine: u32) -> u16 {
        match routine {
            0x2824 => A,
            0x2860 => B,
            0x289c => C,
            _ => unreachable!("shake: routine {routine:06x} (0x2824, 0x2860 or 0x289c only)"),
        }
    }
}

/// 0x2824, 0x2860, 0x289c: starts a shake of the camera in service (1, or 2 if `A5+0x36`) in mode
/// `mode` for `frames` frames. If no shake is in progress, the rest position and the current
/// mode are kept for the end of the shake. Returns the camera.
pub fn start_shake(b: &mut World, mode: u16, frames: u16) -> Obj {
    let cam = if ram::CAMERA_ALT_ACTIVE.is_set(b) { CAMERA_2 } else { CAMERA_1 };
    if cam.shaking(b) == 0 {
        cam.set_shaking(b, 1);
        let current = cam.mode(b);
        cam.set_shake_mode(b, current);
        let x = cam.x(b);
        cam.set_shake_x(b, x);
        let y = cam.y(b);
        cam.set_shake_y(b, y);
    }
    cam.set_mode(b, mode);
    cam.set_shake_count(b, frames);
    cam.0
}

pub const NATIVES: &[Native] = &[];
