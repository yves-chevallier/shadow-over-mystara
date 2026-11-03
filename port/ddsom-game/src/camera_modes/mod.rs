//! Mode scripts of camera 1 (B1): each frame, before the scenery copy (`scroll`), the
//! script of the current mode (`MODE`, multiple of 4) moves the camera's target position, follows the
//! players or animates the scene (per-line scroll tables, shakes...). Each script
//! ends, as the case may be, with the per-scene dispatch routine (`stage_script`, 0x309e0) which can
//! change the mode.
//!
//! The cameras are work RAM records (`scroll::CAMERA_1` / `CAMERA_2`);
//! `Cam::o` designates the one the script modifies (a few scripts act on camera 2). The scripts only
//! touch memory: no register or processor ([`Cam`]).

use cps2::Cps2Bus;

use crate::mem::{rd16, rd32, rd8, wr16, wr32, wr8, A5};
use crate::objects::motion::{self, LastAdd};
use crate::objects::pools::{self, Pool};
use crate::objects::Obj;
use crate::player::{Player, PLAYER_BASE, PLAYER_COUNT, PLAYER_STRIDE};
use crate::scroll::{cam, CAMERA_1, CAMERA_2};
use crate::video::Grid;
use crate::Native;

mod follow;
mod stage;
mod modes;
mod camera2;
use follow::*;
use stage::*;
use modes::*;
use camera2::*;

/// Context of a camera script: the bus and the camera that the script modifies.
struct Cam<'a> {
    b: &'a mut Cps2Bus,
    o: Obj,
}

impl Cam<'_> {
    fn u8(&mut self, off: u32) -> u8 {
        rd8(self.b, self.o.0 + off)
    }
    fn u16(&mut self, off: u32) -> u16 {
        rd16(self.b, self.o.0 + off)
    }
    fn u32(&mut self, off: u32) -> u32 {
        rd32(self.b, self.o.0 + off)
    }
    fn set8(&mut self, off: u32, v: u8) {
        wr8(self.b, self.o.0 + off, v)
    }
    fn set16(&mut self, off: u32, v: u16) {
        wr16(self.b, self.o.0 + off, v)
    }
    fn set32(&mut self, off: u32, v: u32) {
        wr32(self.b, self.o.0 + off, v)
    }
    fn add8(&mut self, off: u32, v: u8) {
        let x = self.u8(off).wrapping_add(v);
        self.set8(off, x)
    }
    fn add16(&mut self, off: u32, v: u16) {
        let x = self.u16(off).wrapping_add(v);
        self.set16(off, x)
    }
    fn add32(&mut self, off: u32, v: u32) {
        let x = self.u32(off).wrapping_add(v);
        self.set32(off, x)
    }
    /// Data space (per-scene tables, RAM).
    fn data8(&mut self, a: u32) -> u8 {
        rd8(self.b, a)
    }
    fn data16(&mut self, a: u32) -> u16 {
        rd16(self.b, a)
    }
    fn data32(&mut self, a: u32) -> u32 {
        rd32(self.b, a)
    }
    /// Program space (tables read PC-relative).
    fn rom16(&mut self, a: u32) -> u16 {
        self.b.fetch16(a)
    }
    /// The game's random draw (`rng::random`).
    fn random(&mut self) -> u16 {
        crate::rng::random(self.b)
    }
    fn alloc(&mut self, pool: &Pool) -> Option<Obj> {
        pool.alloc(self.b)
    }
    /// 0x1dbc: Z.
    fn move_z(&mut self) -> LastAdd {
        motion::ballistic(self.b, self.o, motion::Z)
    }
}

/// Camera fields used by the scripts, in addition to `scroll::cam`.
mod f {
    /// Byte: step of the mode script (increases by 4), then substep.
    pub const STEP: u32 = 0x01;
    pub const SUB_STEP: u32 = 0x02;
    /// Word: vertical speed.
    pub const SPEED: u32 = 0x1a;
    /// Word: script variable (starting ordinate, curve...).
    pub const SCRATCH: u32 = 0x18;
    /// Word: buffer of the scroll lines: starting position.
    pub const GRADIENT_ACC: u32 = 0x30;
    pub const GRADIENT_POS: u32 = 0x36;
    /// Words: script counters.
    pub const TIMER: u32 = 0x4a;
    pub const TIMER_2: u32 = 0x4c;
    /// Byte: variant passed to the objects created.
    pub const KIND: u32 = 0x4b;
    /// Shake: rest position, counter, mode to restore.
    pub const SHAKE_X: u32 = 0x42;
    pub const SHAKE_Y: u32 = 0x44;
    pub const SHAKE_COUNT: u32 = 0x46;
    pub const SHAKE_MODE: u32 = 0x48;
    /// Byte: shake in progress (cleared at the start of each step).
    pub const SHAKING: u32 = 0x3c;
}

/// Modes (field `cam::MODE`).
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

/// Hardware registers and work RAM variables.
mod hw {
    /// Address of the per-line scroll table (high part of the GFX RAM address), starting offset and
    /// video control register (copy at `A5+0x12`).
    pub const ROWSCROLL_BASE: u32 = 0x804108;
    pub const ROWSCROLL_OFFSET: u32 = 0x804120;
    pub const VIDEO_CONTROL: u32 = 0x804122;
    /// Per-line scroll table in GFX RAM.
    pub const ROWSCROLL: u32 = 0x920000;
    pub const ROWSCROLL_B: u32 = 0x920200;
}
const VIDEO_CONTROL_COPY: u32 = 0x12;
const STAGE: u32 = 0x34;
/// A5 word: scroll distance of layer 1 (background movement), long from 0x7fb2.
const BACKDROP: u32 = 0x7fb2;

/// Position of camera 2 (x, y), its previous position, its stops (offsets from A5).
const CAM2_X: u32 = CAMERA_2 + cam::X;
const CAM2_Y: u32 = CAMERA_2 + cam::Y;
const CAM2_PREVIOUS_X: u32 = CAMERA_2 + cam::PREVIOUS_X;
const CAM2_PREVIOUS_Y: u32 = CAMERA_2 + cam::PREVIOUS_Y;
const CAM2_MIN_X: u32 = CAMERA_2 + cam::MIN_X;

// ---------------------------------------------------------------------------------------------
// Initialisation and update of the cameras
// ---------------------------------------------------------------------------------------------

/// Per-scene tables in ROM (data space), also read by `scroll::return_cameras`.
pub(crate) mod tables {
    /// Camera 1: right stop (words), value of output port 0x400004 (words), layer register
    /// (words), "does not go back" byte (bytes), left stop (words), strip step pointer (longs),
    /// "frozen" byte (bytes).
    pub const MAX_X_1: u32 = 0x2f0ee;
    pub const OUTPUT: u32 = 0x2f008;
    pub const LAYERS: u32 = 0x2ef22;
    pub const HOLD_MIN: u32 = 0x2f332;
    pub const MIN_X: u32 = 0x2f1d4;
    pub const ROW_STEP_1: u32 = 0x2f3aa;
    pub const FROZEN: u32 = 0x2f2ba;
    /// Camera 2: strip step pointer (longs), starting position (x, y: words).
    pub const ROW_STEP_2: u32 = 0x32650;
    pub const START_2: u32 = 0x32484;
}

/// Output port whose value depends on the scene.
pub(crate) const OUTPUT_PORT: u32 = 0x400004;

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
pub fn update_camera_1(b: &mut Cps2Bus) -> crate::scroll::D4Leftovers {
    if rd8(b, A5 + CAMERA_1) != 0 {
        step(b)
    } else {
        init_camera_1(b)
    }
}

fn init_camera_1(b: &mut Cps2Bus) -> crate::scroll::D4Leftovers {
    let mut x = Cam { b, o: Obj(A5 + CAMERA_1) };
    x.set16(0, 0x0200);
    let stage = rd8(x.b, A5 + STAGE);
    let by_stage = 2 * stage as u32;
    let max_x = x.data16(tables::MAX_X_1 + by_stage);
    x.set16(cam::MAX_X, max_x);
    let output = x.data16(tables::OUTPUT + by_stage);
    wr16(x.b, OUTPUT_PORT, output);
    wr16(x.b, A5 + 0x14, 0);
    let layers = x.data16(tables::LAYERS + by_stage);
    crate::video::turn_layers_on(x.b, layers);
    let hold = x.data8(tables::HOLD_MIN + stage as u32);
    x.set8(cam::HOLD_MIN, hold);
    x.set16(cam::MODE, 0);
    x.set32(f::TIMER, 0);
    x.set16(0x4e, 0);
    x.set8(f::SHAKING, 0);
    let y = x.u16(cam::Y);
    x.set16(cam::DRAWN_Y, y);
    x.set16(f::SCRATCH, 0x28);
    x.set8(f::STEP, 0);
    wr32(x.b, A5 + BACKDROP, 0);
    // the left stop takes the table's value if it is positive, otherwise it is inverted
    let min = x.u16(cam::MIN_X) as i16;
    let min = if min >= 0 { x.data16(tables::MIN_X + by_stage) } else { min.wrapping_neg() as u16 };
    x.set16(cam::MIN_X, min);
    let pointer = x.data32(tables::ROW_STEP_1 + 2 * by_stage);
    let row_step = x.data16(pointer);
    x.set16(cam::ROW_STEP, row_step);
    let frozen = x.data8(tables::FROZEN + stage as u32);
    x.set8(cam::FROZEN, frozen);
    if let Some(blocks) = background_blocks_1(stage) {
        if stage == 8 || stage == 41 {
            clear_rowscroll_flag(&mut x);
        }
        load_backdrops(&mut x, &blocks);
        return None;
    }
    clear_rowscroll_flag(&mut x);
    let (px, py) = (x.u16(cam::X), x.u16(cam::Y));
    x.set16(cam::DRAWN_X, px);
    x.set16(cam::DRAWN_Y, py.wrapping_sub(0x160));
    crate::scroll::draw1(x.b)
}

/// Destinations (layer 3) of the eight blocks loaded by scenes 54 and 59.
const BACKDROP_SLOTS_2: [u32; 8] = [0x91b800, 0x91b900, 0x91b000, 0x91b100, 0x91a800, 0x91a900, 0x91a000, 0x91a100];

/// Loads into layer 3 tile blocks designated by a ROM table (index, offset from
/// 0x918000); camera 2 is already positioned.
fn load_scroll3_table(x: &mut Cam, table: u32, count: u32) {
    x.set8(f::SUB_STEP, 0);
    let px = x.u16(cam::X);
    x.set16(cam::DRAWN_X, px);
    for i in 0..count {
        let n = x.data16(table + 4 * i);
        let destination = 0x918000u32.wrapping_add(x.data16(table + 4 * i + 2) as i16 as i32 as u32);
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
pub fn update_camera_2(b: &mut Cps2Bus) -> crate::scroll::D4Leftovers {
    if rd8(b, A5 + CAMERA_2) != 0 {
        step_2(b)
    } else {
        init_camera_2(b)
    }
}

fn init_camera_2(b: &mut Cps2Bus) -> crate::scroll::D4Leftovers {
    let mut x = Cam { b, o: Obj(A5 + CAMERA_2) };
    x.set16(0, 0x0200);
    x.set16(cam::MODE, 0);
    x.set16(cam::MIN_X, 0);
    let stage = rd8(x.b, A5 + STAGE) as u32;
    let pointer = x.data32(tables::ROW_STEP_2 + 4 * stage);
    let row_step = x.data16(pointer);
    x.set16(cam::ROW_STEP, row_step);
    x.set16(0x0a, 0);
    x.set16(0x0e, 0);
    let (start_x, start_y) = (x.data16(tables::START_2 + 4 * stage), x.data16(tables::START_2 + 4 * stage + 2));
    x.set16(cam::X, start_x);
    x.set16(cam::Y, start_y);
    match stage {
        4 | 91 => load_scroll3_table(&mut x, 0x321fc, 24),
        6 => load_scroll3_table(&mut x, 0x32284, 8),
        12 | 101 => load_scroll3_table(&mut x, 0x322cc, 8),
        18 => load_scroll3_table(&mut x, 0x32314, 8),
        8 | 41 => {
            for i in 0..8 {
                crate::video::load_grid(x.b, Grid::Layer3, 53, 0x91b800 + 0x100 * i);
            }
        }
        54 => load_scroll3_blocks(&mut x, 16, BACKDROP_SLOTS_2),
        59 => load_scroll3_blocks(&mut x, 39, BACKDROP_SLOTS_2),
        _ => {
            let (px, py) = (x.u16(cam::X), x.u16(cam::Y));
            x.set16(cam::DRAWN_Y, py);
            x.set16(cam::DRAWN_X, px.wrapping_add(0x200) & 0xffe0);
            return crate::scroll::draw2(x.b);
        }
    }
    None
}

/// 0x2f80c: one step of the initialised camera 1: mode script, then target stops and scenery
/// copy (`scroll`).
fn step(b: &mut Cps2Bus) -> crate::scroll::D4Leftovers {
    let mut x = Cam { b, o: Obj(A5 + CAMERA_1) };
    x.set8(f::SHAKING, 0);
    let m = x.u16(cam::MODE);
    run_mode(&mut x, m);
    crate::scroll::after_mode(x.b)
}

/// Camera shake modes (`cam::MODE`) and their original entries.
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
pub fn start_shake(b: &mut Cps2Bus, mode: u16, frames: u16) -> Obj {
    let cam = Obj(A5 + if rd8(b, A5 + 0x36) != 0 { CAMERA_2 } else { CAMERA_1 });
    if cam.u8(b, f::SHAKING) == 0 {
        cam.set_u8(b, f::SHAKING, 1);
        let current = cam.u16(b, cam::MODE);
        cam.set_u16(b, f::SHAKE_MODE, current);
        let x = cam.u16(b, 0x8);
        cam.set_u16(b, f::SHAKE_X, x);
        let y = cam.u16(b, 0xc);
        cam.set_u16(b, f::SHAKE_Y, y);
    }
    cam.set_u16(b, cam::MODE, mode);
    cam.set_u16(b, f::SHAKE_COUNT, frames);
    cam
}

pub const NATIVES: &[Native] = &[];
