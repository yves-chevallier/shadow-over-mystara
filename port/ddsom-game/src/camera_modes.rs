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
use m68k::decode::Fetch;
use m68k::Cpu;

use crate::mem::{rd16, rd32, rd8, rts, wr16, wr32, wr8, A5};
use crate::objects::motion::{self, LastAdd};
use crate::objects::pools::{self, Pool};
use crate::objects::Obj;
use crate::player::{Player, PLAYER_BASE, PLAYER_COUNT, PLAYER_STRIDE};
use crate::scroll::{cam, CAMERA_1, CAMERA_2};
use crate::video::Grid;
use crate::{regs, Native};

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
// Following the players
// ---------------------------------------------------------------------------------------------


fn record(k: u32) -> u32 {
    A5 + PLAYER_BASE + k * PLAYER_STRIDE
}

/// Player `k` exists.
fn present(x: &mut Cam, k: u32) -> bool {
    rd8(x.b, record(k)) != 0
}

/// Player `k` exists and has hit points.
fn alive(x: &mut Cam, k: u32) -> bool {
    present(x, k) && rd16(x.b, record(k) + Player::HP) != 0
}

fn any(x: &mut Cam, test: fn(&mut Cam, u32) -> bool) -> bool {
    (0..PLAYER_COUNT).any(|k| test(x, k))
}

/// Player active for horizontal following: in game, not yet exiting (main state < 4).
fn playing(x: &mut Cam, k: u32) -> bool {
    present(x, k) && (rd8(x.b, record(k) + Player::MAIN) as i8) < 4
}

/// Player on the ground or acting: main state <= 2.
fn grounded(x: &mut Cam, k: u32) -> bool {
    present(x, k) && (rd8(x.b, record(k) + Player::MAIN) as i8) <= 2
}

/// Largest and smallest (unsigned) value of a quantity of the retained players. Player 0,
/// if retained, sets both without comparison; without players, (0, 0xffff).
fn extent(x: &mut Cam, value: impl Fn(&mut Cam, u32) -> Option<u16>) -> (u16, u16) {
    let (mut high, mut low) = (0u16, 0xffffu16);
    for k in 0..PLAYER_COUNT {
        if let Some(v) = value(x, k) {
            if k == 0 {
                (high, low) = (v, v);
            } else {
                high = high.max(v);
                low = low.min(v);
            }
        }
    }
    (high, low)
}

/// Following window of a camera: the target is kept between `far` (front side) and `near`/`back`.
#[derive(Clone, Copy)]
struct Window {
    /// Distance from the camera to the most advanced player beyond which it advances.
    far: i16,
    /// Margin before following the least advanced player / moving back.
    near: i16,
    back: i16,
    /// Maximum step (6); beyond this limit the step is 6: the "jerky" variant of 0x30f6a
    /// tolerates 16 forwards (and then moves back by at most 16).
    limit: i16,
}

impl Window {
    const fn new(far: i16, near: i16, back: i16) -> Self {
        Window { far, near, back, limit: 6 }
    }
    const fn jerky(self) -> Self {
        Window { limit: 16, ..self }
    }
}

const X_PLAYERS: Window = Window::new(0xf0, 0x20, 0x70);
const X_PLAYERS_NARROW: Window = Window::new(0xf0, 0x5c, 0x5c);
const X_OBJECTS: Window = Window::new(0x160, 0x1c, 0x1c);
const X_OBJECT: Window = Window::new(0x80, 0x20, 0x20).jerky();
const Y_PLAYERS: Window = Window::new(0x70, 0x30, 0x30);
const Y_PLAYERS_LOW: Window = Window::new(0x90, 0x38, 0x38);
const Y_OBJECT: Window = Window::new(0x88, 0x78, 0x78);

/// New camera position (`pos`) to keep within the window the targets whose most
/// advanced is `high` and least advanced `low`: advances by at most 6 pixels per frame when the most
/// advanced leaves the window (without losing the least advanced), moves back when the least advanced is too
/// far behind.
fn follow(pos: i16, high: u16, low: u16, w: Window) -> i16 {
    let ahead = (high as i16).wrapping_sub(pos).wrapping_sub(w.far);
    let advance = |pos: i16, d: i16| pos.wrapping_add(if d > w.limit { 6 } else { d });
    if ahead > 0 {
        let near = (low as i16).wrapping_sub(pos).wrapping_sub(w.near);
        return if near <= 0 { pos } else { advance(pos, near.min(ahead)) };
    }
    let behind = (low as i16).wrapping_sub(pos).wrapping_sub(w.back);
    if behind >= 0 {
        return pos;
    }
    let pos = pos.wrapping_add(behind.max(-w.limit));
    let ahead = (high as i16).wrapping_sub(pos).wrapping_sub(w.far);
    if ahead > 0 {
        advance(pos, ahead)
    } else {
        pos
    }
}

/// Applies `follow` to the camera's field `field`.
fn track(x: &mut Cam, field: u32, (high, low): (u16, u16), w: Window) {
    let pos = follow(x.u16(field) as i16, high, low, w);
    x.set16(field, pos as u16);
}

/// Abscissa of the players retained by `keep`.
fn players_x(x: &mut Cam, keep: fn(&mut Cam, u32) -> bool) -> (u16, u16) {
    extent(x, |x, k| keep(x, k).then(|| rd16(x.b, record(k) + Obj::X)))
}

/// Depth of the players retained by `keep`.
fn players_depth(x: &mut Cam, keep: fn(&mut Cam, u32) -> bool) -> (u16, u16) {
    extent(x, |x, k| keep(x, k).then(|| rd16(x.b, record(k) + Obj::Y)))
}

/// 0x30fc8: follows the players on the ground horizontally (nothing if there are none).
fn follow_x(x: &mut Cam) {
    if any(x, grounded) {
        follow_x_playing(x);
    }
}

/// 0x31018: follows the players in game horizontally.
fn follow_x_playing(x: &mut Cam) {
    let e = players_x(x, playing);
    track(x, cam::X, e, X_PLAYERS);
}

/// Negative abscissa x brought back to 0.
fn keep_x_positive(x: &mut Cam) {
    if (x.u16(cam::X) as i16) < 0 {
        x.set16(cam::X, 0);
    }
}

/// 0x315be: follows the living players, horizontally (narrow window) then in depth.
fn follow_xy(x: &mut Cam) {
    if any(x, alive) {
        let e = players_x(x, playing);
        track(x, cam::X, e, X_PLAYERS_NARROW);
        keep_x_positive(x);
        follow_depth(x);
    }
}

/// 0x31616: follows the depth of the living players.
fn follow_depth(x: &mut Cam) {
    let e = players_depth(x, alive);
    track(x, cam::Y, e, Y_PLAYERS);
}

/// 0x3147c: follows the living players, depth with the "low" window.
fn follow_snap(x: &mut Cam) {
    if any(x, alive) {
        follow_x_playing(x);
        keep_x_positive(x);
        let e = players_depth(x, alive);
        track(x, cam::Y, e, Y_PLAYERS_LOW);
    }
}

/// 0x31700: follows the middle of the living players, then the depth of all the players present.
fn follow_mid(x: &mut Cam) {
    if any(x, alive) {
        let (high, low) = players_x(x, alive);
        let mid = high.wrapping_add(low) >> 1;
        track(x, cam::X, (mid, mid), X_PLAYERS);
        keep_x_positive(x);
        let e = players_depth(x, present);
        track(x, cam::Y, e, Y_PLAYERS);
    }
}

/// Four pointers (words) to the objects followed by the mode 0x0c camera (A5 - 0x555c).
const TRACKED_OBJECTS: u32 = 0xff2aa4;

/// 0x31334: follows horizontally the objects designated by the four pointers.
fn follow_objects(x: &mut Cam) {
    let pointers: Vec<u16> = (0..4).map(|k| rd16(x.b, TRACKED_OBJECTS + 2 * k)).collect();
    if pointers.iter().fold(0u16, |s, &p| s.wrapping_add(p)) == 0 {
        return;
    }
    let mut high_low = (0u16, 0xffffu16);
    for (k, &p) in pointers.iter().enumerate() {
        if p != 0 {
            let v = rd16(x.b, p as i16 as i32 as u32 + Obj::X);
            high_low = if k == 0 { (v, v) } else { (high_low.0.max(v), high_low.1.min(v)) };
        }
    }
    track(x, cam::X, high_low, X_OBJECTS);
}

// ---------------------------------------------------------------------------------------------
// Per-scene dispatch (0x309e0)
// ---------------------------------------------------------------------------------------------

/// Writes the video control register with bit 0 set (per-line scroll) or cleared.
fn rowscroll_control(x: &mut Cam, on: bool) {
    let v = rd16(x.b, A5 + VIDEO_CONTROL_COPY);
    let v = if on { v | 1 } else { v & !1 };
    wr16(x.b, A5 + VIDEO_CONTROL_COPY, v);
    wr16(x.b, hw::VIDEO_CONTROL, v);
}

/// Enables the per-line scroll table in GFX RAM.
fn rowscroll_on(x: &mut Cam) {
    rowscroll_control(x, true);
    wr16(x.b, hw::ROWSCROLL_BASE, 0x9200);
}

/// 0x309e0: depending on the scene, sets the camera mode or plays a scene sequence.
fn stage_script(x: &mut Cam) {
    let stage = rd8(x.b, A5 + STAGE);
    match stage {
        2 | 83..=90 | 102 => scripted_scroll(x),
        6 | 12 | 17 | 19 | 97 | 101 => x.set16(cam::MODE, mode::FOLLOW_X),
        1 => arrival(x),
        3 | 13 | 18 | 21..=23 | 26 | 58 | 64 | 68 | 70 | 73..=75 | 77 | 79 | 80 | 93 | 100 => x.set16(cam::MODE, mode::FREE),
        8 => x.set16(cam::MODE, mode::PARKED),
        11 | 94 => {
            if x.u8(f::STEP) == 0 {
                load_backdrops(x, &BACKDROP_BLOCKS_A);
                rowscroll_on(x);
                x.add8(f::STEP, 1);
            }
            x.set16(cam::MODE, mode::WAVES);
        }
        20 => x.set16(cam::MODE, mode::DUAL_FOLLOW),
        38 | 40 => x.set16(cam::MODE, mode::RIPPLE),
        54 | 59 => x.set16(cam::MODE, mode::FOLLOW_X_SYNC),
        69 | 71 | 72 | 76 | 78 | 81 | 82 => {
            rowscroll_on(x);
            wr16(x.b, hw::ROWSCROLL_OFFSET, 0);
            x.set8(cam::FROZEN, 1);
            x.set16(cam::MODE, mode::SKY_GRADIENT);
        }
        4 | 91 => rise_then_follow(x),
        33 | 104 => pause_then_follow(x),
        7 | 9 => x.set16(cam::MODE, mode::SCREEN_OBJECT),
        14 | 92 | 95 | 109 => x.set16(cam::MODE, mode::SNAP),
        67 => x.set16(cam::MODE, mode::WAVES_LEFT),
        41 => x.set16(cam::MODE, mode::FOLLOW_SYNC),
        66 => climb(x, 1, 296),
        105 => x.set16(cam::MODE, mode::FOLLOW_MID),
        44 | 111 => climb(x, 2, 280),
        _ => x.set16(cam::MODE, mode::FOLLOW),
    }
}

/// Tile blocks loaded into the scroll layers (block index, GFX RAM address).
const BACKDROP_BLOCKS_A: [(u16, u32); 8] = [
    (206, 0x917000),
    (173, 0x917400),
    (175, 0x917800),
    (175, 0x917c00),
    (172, 0x916000),
    (205, 0x916400),
    (174, 0x916800),
    (174, 0x916c00),
];

fn load_backdrops(x: &mut Cam, blocks: &[(u16, u32)]) {
    for &(index, dest) in blocks {
        crate::video::load_grid(x.b, Grid::Layer2, index, dest);
    }
}

/// Scenes 2, 83-90, 102: camera 2 scrolls according to a script (step of camera 1).
fn scripted_scroll(x: &mut Cam) {
    let sync_previous = |x: &mut Cam| {
        let v = rd16(x.b, A5 + CAM2_X);
        wr16(x.b, A5 + CAM2_PREVIOUS_X, v);
    };
    match x.u8(f::STEP) {
        0x00 => x.set16(cam::MODE, mode::DUAL_FOLLOW),
        0x08 => {
            wr16(x.b, A5 + CAM2_MIN_X, 0);
            sync_previous(x);
            let v = rd16(x.b, A5 + CAM2_X).wrapping_sub(4);
            wr16(x.b, A5 + CAM2_X, v);
            if (v as i16) < 0 {
                wr16(x.b, A5 + CAM2_X, 0);
                x.add8(f::STEP, 4);
                sync_previous(x);
            }
        }
        0x0c => sync_previous(x),
        0x10 => {
            sync_previous(x);
            let v = rd16(x.b, A5 + CAM2_X).wrapping_add(4);
            wr16(x.b, A5 + CAM2_X, v);
            if (v as i16) >= 384 {
                x.add8(f::STEP, 4);
            }
        }
        0x14 => {
            sync_previous(x);
            let v = rd16(x.b, A5 + CAM2_X).wrapping_add(16);
            wr16(x.b, A5 + CAM2_X, v);
            if (v as i16) >= 800 {
                x.add8(f::STEP, 4);
                wr16(x.b, A5 + CAM2_X, 800);
                wr16(x.b, A5 + CAM2_PREVIOUS_X, 800);
                x.set16(f::TIMER, 120);
                // 0xa8ae8: scenery list 2
                crate::families::effects_creators::set_list(x.b, 2);
            }
        }
        0x18 => {
            let t = x.u16(f::TIMER).wrapping_sub(1);
            x.set16(f::TIMER, t);
            if (t as i16) < 0 {
                x.add8(f::STEP, 4);
                x.set16(cam::MODE, mode::STAGE);
                sync_previous(x);
                start_shake(x.b, shake::A, 30);
            }
        }
        0x20 => {
            sync_previous(x);
            let v = rd16(x.b, A5 + CAM2_X).wrapping_add(8);
            wr16(x.b, A5 + CAM2_X, v);
            if (v as i16) >= 1280 {
                wr16(x.b, A5 + CAM2_X, 1280);
                x.add8(f::STEP, 4);
            }
        }
        0x04 | 0x1c | 0x24 => {
            sync_previous(x);
            let v = rd16(x.b, A5 + CAM2_Y);
            wr16(x.b, A5 + CAM2_PREVIOUS_Y, v);
        }
        _ => {}
    }
}

/// Scene 1: the camera locks according to the arrival abscissa.
fn arrival(x: &mut Cam) {
    let step = x.u8(f::STEP);
    let px = x.u16(cam::X) as i16;
    let first = if step != 0 { px <= 299 } else { px <= 0 };
    if first {
        x.set16(cam::Y, if step != 0 { 470 } else { 544 });
        x.set16(cam::MODE, mode::FOLLOW_X);
    } else if px <= 320 {
        x.set16(cam::MODE, mode::PUSH);
    } else {
        x.set16(cam::Y, 464);
        x.set16(cam::PREVIOUS_Y, 464);
        x.set16(cam::MODE, mode::FOLLOW_X);
    }
}

/// Scenes 4 and 91: wait, then the camera rises to `48` and switches to following.
fn rise_then_follow(x: &mut Cam) {
    match x.u8(f::SUB_STEP) {
        0x00 => x.add8(f::SUB_STEP, 4),
        0x08 => {
            let v = x.u16(cam::X).wrapping_sub(2);
            x.set16(cam::X, v);
            if (v as i16) <= 0x30 {
                x.add8(f::SUB_STEP, 4);
                let a = rd8(x.b, A5 + 0x54f).wrapping_add(4);
                wr8(x.b, A5 + 0x54f, a);
                x.set16(f::TIMER, 0);
            }
        }
        0x0c => x.set16(cam::MODE, mode::FOLLOW_X),
        _ => {}
    }
}

/// Scenes 33 and 104: pause of 64 frames then normal following.
fn pause_then_follow(x: &mut Cam) {
    match x.u8(f::STEP) {
        0x00 => {
            x.add8(f::STEP, 4);
            x.set16(f::TIMER, 0x40);
        }
        0x04 => {
            let t = x.u16(f::TIMER).wrapping_sub(1);
            x.set16(f::TIMER, t);
            if (t as i16) < 0 {
                x.add8(f::STEP, 4);
            }
        }
        0x08 => x.set16(cam::MODE, mode::FOLLOW),
        _ => {}
    }
}

/// Scenes 66 and 44/111: the camera rises by `speed` pixels per frame to `top`, then following.
fn climb(x: &mut Cam, speed: u16, top: i16) {
    match x.u8(f::SUB_STEP) {
        0x00 => {
            let y = x.u16(cam::Y).wrapping_sub(speed);
            x.set16(cam::Y, y);
            if (y as i16) <= top {
                x.set16(cam::Y, top as u16);
                x.add8(f::SUB_STEP, 4);
            }
        }
        0x04 => x.set16(cam::MODE, mode::FOLLOW),
        _ => {}
    }
}

// ---------------------------------------------------------------------------------------------
// Mode scripts
// ---------------------------------------------------------------------------------------------

/// Drawn position = target position.
fn sync_drawn(x: &mut Cam) {
    let (px, py) = (x.u16(cam::X), x.u16(cam::Y));
    x.set16(cam::DRAWN_X, px);
    x.set16(cam::DRAWN_Y, py);
}

/// 0x31888: direction of the vertical push according to the drawn position, then movement.
fn push_vertical(x: &mut Cam) {
    x.set16(f::SPEED, 0xffff);
    if x.u16(cam::DRAWN_Y) as i16 > x.u16(cam::Y) as i16 {
        let v = 0u16.wrapping_sub(x.u16(f::SPEED));
        x.set16(f::SPEED, v);
    }
    x.move_z();
}

/// 0x3016e: sky gradient in the per-line scroll table.
fn sky_gradient(x: &mut Cam) {
    follow_objects(x);
    if x.u8(f::STEP) == 0 {
        x.add8(f::STEP, 1);
        rowscroll_on(x);
        x.set16(f::GRADIENT_POS, 0x9200);
        for k in 0..0x101 {
            wr32(x.b, hw::ROWSCROLL + 4 * k, 0x0090_0090);
        }
        return;
    }
    let px = x.u16(cam::X) as i16;
    x.set16(cam::X, px.clamp(0, 0xd0) as u16);
    let step = ((0x90u16.wrapping_sub(rd16(x.b, A5 + 0x5a6)) as u32) << 16) as i32 >> 7;
    let table = ((x.u16(f::GRADIENT_POS) as u32 ^ 8) << 8) + 0x1c0;
    let mut acc: u32 = 0x0090_0000;
    x.set32(f::GRADIENT_ACC, acc);
    let first = (acc >> 16) as u16;
    let mut at = table;
    for _ in 0..32 {
        at -= 2;
        wr16(x.b, at, first);
    }
    for _ in 0..80 {
        acc = acc.wrapping_add(step as u32);
        at -= 2;
        wr16(x.b, at, (acc >> 16) as u16);
    }
    x.set32(f::GRADIENT_ACC, acc);
    wr16(x.b, A5 + 0x57e, (acc >> 16) as u16);
    for _ in 0..56 {
        at -= 4;
        wr32(x.b, at, (first as u32) << 16 | first as u32);
    }
}

/// 0x3022a: undulation (starting per-line scroll table in ROM, then background offset).
fn ripple(x: &mut Cam) {
    if x.u8(f::STEP) == 0 {
        x.add8(f::STEP, 1);
        let y = x.u16(cam::Y);
        x.set16(f::SCRATCH, y);
        rowscroll_on(x);
        let mut at = hw::ROWSCROLL;
        for _ in 0..9 {
            for k in 0..16 {
                let v = x.data32(0x3028e + 4 * k);
                wr32(x.b, at, v);
                at += 4;
            }
        }
        return;
    }
    let reference = rd16(x.b, A5 + 0x5aa);
    let delta = x.u16(f::SCRATCH).wrapping_sub(reference);
    x.set16(f::SCRATCH, reference);
    let v = rd16(x.b, A5 + BACKDROP).wrapping_add(delta);
    wr16(x.b, A5 + BACKDROP, v);
    let long = crate::mem::rd32(x.b, A5 + BACKDROP).wrapping_add(0x4000);
    wr32(x.b, A5 + BACKDROP, long);
    let v = rd16(x.b, A5 + BACKDROP) & 0x1f;
    wr16(x.b, A5 + BACKDROP, v);
}

/// Oscillating per-line scroll table: two counters (0x4a, 0x4c) that advance by 1 and 2,
/// written 129 times; the order of the two words depends on the active table.
fn waves(x: &mut Cam) {
    let a = x.u16(f::TIMER).wrapping_add(1);
    let a = if (a as i16) < 0x100 { a } else { 0 };
    x.set16(f::TIMER, a);
    let b = x.u16(f::TIMER_2).wrapping_add(2);
    let b = if (b as i16) < 0x100 { b } else { 0 };
    x.set16(f::TIMER_2, b);
    let (mut at, first, second) = if rd16(x.b, A5 + BACKDROP) != 0 { (hw::ROWSCROLL, b, a) } else { (hw::ROWSCROLL_B, a, b) };
    for _ in 0..129 {
        wr16(x.b, at, first);
        wr16(x.b, at + 2, second);
        at += 4;
    }
}

/// Camera 2: copy of the previous position before moving it.
fn remember_camera_2(x: &mut Cam) {
    let (px, py) = (rd16(x.b, A5 + CAM2_X), rd16(x.b, A5 + CAM2_Y));
    wr16(x.b, A5 + CAM2_PREVIOUS_X, px);
    wr16(x.b, A5 + CAM2_PREVIOUS_Y, py);
}

/// 0x303f2: camera 2 stays between camera 1's stop and its own left stop.
fn clamp_camera_2(x: &mut Cam) {
    let max = x.u16(cam::MAX_X) as i16;
    if max < rd16(x.b, A5 + CAM2_X) as i16 {
        wr16(x.b, A5 + CAM2_X, max as u16);
    }
    let min = rd16(x.b, A5 + CAM2_MIN_X);
    if min as i16 > rd16(x.b, A5 + CAM2_X) as i16 {
        wr16(x.b, A5 + CAM2_X, min);
    }
    stage_script(x);
}

/// Runs `f` with camera 2 as the current camera.
fn on_camera_2(x: &mut Cam, f: fn(&mut Cam)) {
    let cam1 = x.o;
    x.o = Obj(A5 + CAMERA_2);
    f(x);
    x.o = cam1;
}

/// 0x30308: undulation, then camera 2 follows the players.
fn waves_and_follow(x: &mut Cam) {
    waves(x);
    let px = rd16(x.b, A5 + CAM2_X);
    wr16(x.b, A5 + CAM2_PREVIOUS_X, px);
    on_camera_2(x, follow_x);
    clamp_camera_2(x);
}

/// 0x3041c: camera 2 follows the depth of the living players.
fn camera_2_depth(x: &mut Cam) {
    remember_camera_2(x);
    if any(x, alive) {
        on_camera_2(x, follow_depth);
    }
}

/// 0x303c8: camera 2 follows the players (narrow), then stays within the stops.
fn camera_2_follow(x: &mut Cam) {
    remember_camera_2(x);
    if any(x, present) {
        on_camera_2(x, follow_xy);
        clamp_camera_2(x);
    }
}

/// Creation of a scenery object (SET) at the end of a scroll: `kind` written to its type.
fn spawn_gate(x: &mut Cam, pool: &pools::Pool, kind: u8, bump: u8) -> bool {
    let Some(Obj(obj)) = x.alloc(pool) else { return false };
    let variant = x.u8(f::KIND);
    wr8(x.b, obj, 1);
    wr8(x.b, obj + 2, kind);
    wr8(x.b, obj + 3, variant.wrapping_add(bump));
    true
}

/// 0x3047e / 0x3053c: the camera goes to `target` (2 pixels per frame); on arrival: an object is created and
/// the mode switches to `next`.
fn advance_to(x: &mut Cam, target: i16, next: u16) {
    let mut px = x.u16(cam::X) as i16;
    if target >= px {
        px = px.wrapping_add(2);
        x.set16(cam::X, px as u16);
        if target > px {
            return;
        }
    } else {
        px = px.wrapping_sub(2);
        x.set16(cam::X, px as u16);
        if target < px {
            return;
        }
    }
    x.set16(cam::X, target as u16);
    if spawn_gate(x, &pools::EFFCT, 0, 0) {
        x.set16(cam::MODE, next);
        x.set8(f::STEP, 0);
        x.add16(f::TIMER, 1);
    }
}

/// 0x304dc: with the portal created, the camera locks and shifts the players present.
fn gate(x: &mut Cam) {
    if x.u8(f::STEP) == 0 {
        return;
    }
    if !spawn_gate(x, &pools::SET, 2, 1) {
        return;
    }
    x.set16(cam::X, 0x468);
    x.set16(cam::MIN_X, 0x468);
    let shift = if x.u16(f::TIMER) == 1 { 0x400u16 } else { 0xfec0 };
    shift_players(x, shift);
    stage_script(x);
}

fn shift_players(x: &mut Cam, shift: u16) {
    for k in 0..PLAYER_COUNT {
        if present(x, k) {
            let v = rd16(x.b, record(k) + Obj::X).wrapping_add(shift);
            wr16(x.b, record(k) + Obj::X, v);
        }
    }
}

/// 0x3058e: exit from the second portal.
fn gate_far(x: &mut Cam) {
    match x.u8(f::STEP) {
        0x04 => x.add16(cam::X, 0xfffe),
        0x08 => {
            if !spawn_gate(x, &pools::SET, 2, 1) {
                return;
            }
            x.set16(cam::X, 0x820);
            x.set16(cam::MIN_X, 0x820);
            shift_players(x, 0x2c0);
            stage_script(x);
            start_shake(x.b, shake::C, 0x20);
        }
        _ => {}
    }
}

/// 0x305fc: the camera follows the players forwards; the depth follows a quarter of the movement.
fn push(x: &mut Cam) {
    let before = x.u32(cam::X);
    follow_x(x);
    if (x.u16(cam::X) as i16) < x.u16(cam::MIN_X) as i16 {
        let m = x.u16(cam::MIN_X);
        x.set16(cam::X, m);
    }
    let moved = (before.wrapping_sub(x.u32(cam::X)) as i32) >> 2;
    x.add32(cam::Y, moved as u32);
    stage_script(x);
}

/// 0x3063a: shake of 4 positions (movements `dx`, `dy` per position), then end.
fn shake(x: &mut Cam, amplitude: i16) {
    let t = x.u16(f::SHAKE_COUNT).wrapping_sub(1);
    x.set16(f::SHAKE_COUNT, t);
    if (t as i16) < 0 {
        let (sx, sy, sm) = (x.u16(f::SHAKE_X), x.u16(f::SHAKE_Y), x.u16(f::SHAKE_MODE));
        x.set16(cam::X, sx);
        x.set16(cam::Y, sy);
        x.set16(cam::MODE, sm);
        x.set8(f::SHAKING, 0);
        return;
    }
    x.set8(f::SHAKING, 1);
    const OFFSETS: [(i16, i16); 4] = [(1, -1), (1, 1), (-1, -1), (-1, 1)];
    let (dx, dy) = OFFSETS[(t & 3) as usize];
    let (sx, sy) = (x.u16(f::SHAKE_X), x.u16(f::SHAKE_Y));
    x.set16(cam::X, sx.wrapping_add((dx * amplitude) as u16));
    x.set16(cam::Y, sy.wrapping_add((dy * amplitude) as u16));
}

/// Fills the per-line scroll table with random values (mask `mask`): 513 words.
fn rowscroll_noise(x: &mut Cam, mask: u16) {
    for k in 0..0x201 {
        let v = x.random() & mask;
        wr16(x.b, hw::ROWSCROLL + 2 * k, v);
    }
}

/// Shifts the per-line scroll table by one word towards the start and adds a draw at the end.
fn rowscroll_shift(x: &mut Cam, mask: u16, tail_offset: u32) {
    for k in 0..0x200 {
        let v = rd16(x.b, hw::ROWSCROLL + 2 * k + 2);
        wr16(x.b, hw::ROWSCROLL + 2 * k, v);
    }
    let v = x.random() & mask;
    wr16(x.b, hw::ROWSCROLL + 0x400 + tail_offset, v);
}

/// 0x308da: storm (noise in the per-line scroll table), return to free mode.
fn storm(x: &mut Cam) {
    match x.u8(f::STEP) {
        0 => {
            x.add8(f::STEP, 4);
            rowscroll_on(x);
            wr16(x.b, hw::ROWSCROLL_OFFSET, 0);
            rowscroll_noise(x, 0xf);
        }
        4 => {
            let t = x.u16(f::TIMER).wrapping_sub(1);
            x.set16(f::TIMER, t);
            if (t as i16) < 0 {
                x.set16(cam::MODE, mode::FREE);
                x.set8(f::STEP, 0);
                rowscroll_control(x, false);
            } else {
                rowscroll_shift(x, 7, 0);
            }
        }
        _ => {}
    }
}

/// Animated palettes (in ROM: source and destination pointer per 8-byte entry).
const PALETTE_STEPS: u32 = 0x30816;

/// 0x3071a: thunderstorm (like `storm`, then fade of the lightning palettes).
fn thunder(x: &mut Cam) {
    match x.u8(f::STEP) {
        0 => {
            x.add8(f::STEP, 4);
            x.set16(f::TIMER, 24);
            rowscroll_on(x);
            wr16(x.b, hw::ROWSCROLL_OFFSET, 0);
            rowscroll_noise(x, 0xf);
        }
        4 => {
            let t = x.u16(f::TIMER).wrapping_sub(1);
            x.set16(f::TIMER, t);
            if (t as i16) < 0 {
                x.set32(f::TIMER, 0);
                x.add8(f::STEP, 4);
            } else {
                rowscroll_shift(x, 7, 2);
            }
        }
        8 => {
            rowscroll_shift(x, 0xf, 2);
            let t = x.u16(f::TIMER).wrapping_sub(1);
            x.set16(f::TIMER, t);
            if (t as i16) >= 0 {
                return;
            }
            x.set16(f::TIMER, 1);
            // the original calls the same subroutine twice (`bsr` to the next instruction)
            for _ in 0..2 {
                palette_step(x);
            }
        }
        _ => {}
    }
}

/// 0x307d8: one step of the palette fade; end of the table: the per-line scroll stops.
fn palette_step(x: &mut Cam) {
    let at = x.u16(f::TIMER_2) as i16 as i32 as u32;
    let rom32 = |x: &mut Cam, a: u32| (x.rom16(a) as u32) << 16 | x.rom16(a + 2) as u32;
    let source = rom32(x, PALETTE_STEPS.wrapping_add(at));
    if source == 0 {
        rowscroll_control(x, false);
        stage_script(x);
        return;
    }
    let mut dest = rom32(x, PALETTE_STEPS.wrapping_add(at) + 4);
    let mut from = source;
    for _ in 0..12 {
        let kept = rd16(x.b, dest + 2) as u32 & 0xff80;
        let v = kept.wrapping_add(x.data32(from));
        from += 4;
        wr32(x.b, dest, v);
        dest += 4;
    }
    x.add16(f::TIMER_2, 8);
}

/// 0x30988: waits, then the camera descends to 256 (scene 67).
fn waves_left(x: &mut Cam) {
    match x.u8(f::STEP) {
        0x00 => {
            x.add8(f::STEP, 4);
            x.set16(f::TIMER, 0x40);
        }
        0x04 => {
            let t = x.u16(f::TIMER).wrapping_sub(1);
            x.set16(f::TIMER, t);
            if (t as i16) >= 0 {
                return;
            }
            x.add8(f::STEP, 4);
        }
        0x08 => {
            if x.u16(cam::Y) as i16 <= 368 {
                wr8(x.b, A5 + 0x42, 1);
            }
            let y = (x.u16(cam::Y) as i16).wrapping_sub(4);
            x.set16(cam::Y, if y > 256 { y as u16 } else { 256 });
            stage_script(x);
        }
        _ => {}
    }
}

/// 0x31822: the camera follows an object (pointer at word `A5+0x46`).
fn screen_object(x: &mut Cam) {
    let obj = rd16(x.b, A5 + 0x46) as i16 as i32 as u32;
    let v = rd16(x.b, obj + Obj::X);
    track(x, cam::X, (v, v), X_OBJECT);
    let v = rd16(x.b, obj + Obj::Y);
    track(x, cam::Y, (v, v), Y_OBJECT);
}

/// 0x3047e..0x3071a, 0x3141a...: runs the script of mode `mode` of camera 1.
fn run_mode(x: &mut Cam, mode: u16) {
    match mode {
        mode::STAGE => stage_script(x),
        mode::FREE => {}
        mode::FOLLOW => {
            follow_xy(x);
            stage_script(x);
        }
        mode::SKY_GRADIENT => sky_gradient(x),
        0x10 => {
            follow_x(x);
            push_vertical(x);
            stage_script(x);
        }
        mode::FOLLOW_X => {
            follow_x(x);
            stage_script(x);
        }
        mode::RIPPLE => ripple(x),
        mode::FOLLOW_SYNC => {
            follow_xy(x);
            sync_drawn(x);
            stage_script(x);
        }
        0x20 => {
            if any(x, alive) {
                follow_depth(x);
            }
        }
        0x24 => {
            follow_xy(x);
            sync_drawn(x);
        }
        0x28 => {
            let sum = rd16(x.b, A5 + 0x5e6).wrapping_add(rd16(x.b, A5 + 0x7e6)) >> 1;
            x.set16(cam::X, sum.wrapping_sub(0xc0));
            follow_depth(x);
        }
        mode::WAVES => waves_and_follow(x),
        0x30 => camera_2_depth(x),
        0x34 => storm(x),
        mode::DUAL_FOLLOW => camera_2_follow(x),
        mode::WALL_RIGHT => {
            let target = if x.u16(f::TIMER) != 0 { 0x5a8 } else { 0x68 };
            advance_to(x, target, mode::GATE);
        }
        mode::GATE => gate(x),
        mode::PUSH => push(x),
        mode::PARKED => {
            if rd8(x.b, A5 + 0x2c) == 12 {
                x.add16(cam::X, 1);
            }
            let px = x.u16(cam::X);
            x.set16(cam::DRAWN_X, px);
        }
        mode::WALL_FAR => advance_to(x, 0x640, mode::GATE_FAR),
        mode::GATE_FAR => gate_far(x),
        0x54 | 0x68 | 0x6c | 0x70 => shake(x, 1),
        0x58 => shake(x, 2),
        0x5c => shake(x, 3),
        0x60 => thunder(x),
        mode::SNAP => follow_snap(x),
        mode::FOLLOW_X_SYNC => {
            follow_x(x);
            sync_drawn(x);
            stage_script(x);
        }
        0x78 => {
            follow_xy(x);
            ripple(x);
        }
        mode::SCREEN_OBJECT => screen_object(x),
        0x80 => waves(x),
        mode::WAVES_LEFT => waves_left(x),
        mode::FOLLOW_MID => follow_mid(x),
        _ => {}
    }
}

// ---------------------------------------------------------------------------------------------
// Camera 2: scenery of scroll layer 3, which follows the scroll of camera 1
// ---------------------------------------------------------------------------------------------

/// Modes of camera 2 (`cam::MODE`).
mod mode2 {
    pub const STAGE: u16 = 0x00;
    pub const FREE: u16 = 0x04;
    pub const HALF: u16 = 0x14;
    pub const FULL: u16 = 0x18;
    pub const DRIFT: u16 = 0x1c;
    pub const QUARTER_X: u16 = 0x2c;
    pub const RUBBLE: u16 = 0x34;
    pub const QUARTER_THREE: u16 = 0x38;
    pub const FOLLOWED: u16 = 0x40;
    pub const QUARTER_BOTH: u16 = 0x44;
    pub const CREEP_6: u16 = 0x48;
    pub const HALF_SYNC: u16 = 0x4c;
    pub const HALF_X: u16 = 0x50;
    pub const SCREEN_X: u16 = 0x60;
    pub const WAIT_THEN_CREEP: u16 = 0x64;
    pub const CREEP_3: u16 = 0x68;
    pub const FROM_CAMERA_1: u16 = 0x74;
    pub const DRIVES_CAMERA_1: u16 = 0x78;
}

/// Scroll of camera 1 since the previous frame on axis 0 (x) or 1 (y).
fn delta(x: &mut Cam, axis: usize) -> i32 {
    crate::scroll::scroll_delta(x.b, A5, axis)
}

/// Camera 2: advances by `dx`/`dy` (positions in 16.16).
fn drift(x: &mut Cam, dx: i32, dy: i32) {
    x.add32(cam::X, dx as u32);
    x.add32(cam::Y, dy as u32);
}

/// Per-scene dispatch of camera 2 (0x332c0).
fn stage_script_2(x: &mut Cam) {
    let stage = rd8(x.b, A5 + STAGE);
    match stage {
        2 | 20 | 90 | 102 => x.set16(cam::MODE, mode2::DRIVES_CAMERA_1),
        3 | 7 | 9 | 21..=23 | 26 | 36 | 37 | 47 | 50 | 61 | 62 | 64 | 65 | 93 | 100 | 114 => x.set16(cam::MODE, mode2::FULL),
        1 | 39 | 43 | 56 | 57 | 63 | 66 | 112 => x.set16(cam::MODE, mode2::QUARTER_THREE),
        67 => x.set16(cam::MODE, mode2::QUARTER_BOTH),
        4 => x.set16(cam::MODE, mode2::WAIT_THEN_CREEP),
        35 => x.set16(cam::MODE, mode2::HALF_X),
        8 | 41 => x.set16(cam::MODE, mode2::HALF_SYNC),
        105 => {}
        13 => falling_leaves(x),
        54 | 59 => x.set16(cam::MODE, mode2::FROM_CAMERA_1),
        46 | 113 => rising(x),
        69 | 71 | 72 | 76 | 78 | 81..=89 => x.set16(cam::MODE, mode2::SCREEN_X),
        11 | 94 => x.set16(cam::MODE, mode2::FOLLOWED),
        6 | 12 | 101 => x.set16(cam::MODE, mode2::CREEP_3),
        18 => x.set16(cam::MODE, mode2::DRIFT),
        25 | 106..=108 => x.set16(cam::MODE, mode2::QUARTER_X),
        91 => x.set16(cam::MODE, mode2::CREEP_6),
        _ => x.set16(cam::MODE, mode2::HALF),
    }
}

/// Scene 13: the scenery rises, then leaves fall (scenery objects created at random).
fn falling_leaves(x: &mut Cam) {
    const VARIANTS: [u8; 8] = [0x4b, 0x4c, 0x4d, 0x58, 0x4c, 0x4d, 0x58, 0x4b];
    match x.u8(f::STEP) {
        0x00 => {
            x.add8(f::STEP, 4);
            x.set16(f::TIMER, 128);
            x.set16(f::TIMER_2, 0);
        }
        0x04 => {
            let t = x.u16(f::TIMER).wrapping_sub(1);
            x.set16(f::TIMER, t);
            if (t as i16) < 0 {
                x.add8(f::STEP, 4);
            }
        }
        0x08 => {
            let y = x.u16(cam::Y).wrapping_sub(1);
            x.set16(cam::Y, y);
            if (y as i16) <= 656 {
                x.set16(cam::Y, 656);
                x.add8(f::STEP, 4);
                x.set16(f::TIMER, 512);
            }
        }
        0x0c => {
            if (rd16(x.b, A5 + 0x38) as i16) <= 0 {
                x.add8(f::STEP, 4);
                return;
            }
            let t = x.u16(f::TIMER_2).wrapping_sub(1);
            x.set16(f::TIMER_2, t);
            if (t as i16) >= 0 {
                return;
            }
            let wait = x.random() & 63;
            x.set16(f::TIMER_2, wait);
            let Some(Obj(leaf)) = x.alloc(&pools::SOBJ) else { return };
            wr8(x.b, leaf, 1);
            wr8(x.b, leaf + 2, 0);
            let variant = VARIANTS[(x.random() & 7) as usize];
            wr8(x.b, leaf + 3, variant);
            wr8(x.b, leaf + 0x2f, 0);
        }
        0x10 => {
            let y = x.u16(cam::Y).wrapping_sub(1);
            x.set16(cam::Y, y);
            if (y as i16) < 0 {
                x.set16(cam::Y, y.wrapping_add(64));
            }
        }
        _ => {}
    }
}

/// Scenes 46 and 113: the scenery descends from 0 to 704, creating an object on the way.
fn rising(x: &mut Cam) {
    let finish = |x: &mut Cam| {
        let y = x.u16(cam::Y).wrapping_add(4);
        x.set16(cam::Y, y);
        if (y as i16) >= 704 {
            x.set16(cam::Y, 704);
            x.set16(cam::MODE, mode2::FREE);
            x.add8(f::STEP, 4);
        }
    };
    match x.u8(f::STEP) {
        0x00 => {
            let y = x.u16(cam::Y).wrapping_sub(4);
            x.set16(cam::Y, y);
            if (y as i16) >= 0 {
                return;
            }
            x.set16(cam::Y, 0);
            x.set16(cam::X, 560);
            let Some(Obj(obj)) = x.alloc(&pools::BOSS) else { return };
            wr8(x.b, obj, 1);
            wr8(x.b, obj + 2, 0xf);
            x.add8(f::STEP, 4);
            finish(x);
        }
        0x04 => finish(x),
        _ => {}
    }
}

/// 0x32f7c / 0x32fd2: the scenery advances 6 pixels per frame over a 0x600-pixel buffer, which it
/// brings back by 0x200; the first time, it locks onto the next page.
fn strolling(x: &mut Cam) {
    x.add16(cam::X, 6);
    let px = x.u16(cam::X);
    x.set16(cam::DRAWN_X, px);
    if x.u8(f::STEP) == 0 {
        let px = px & 0xff;
        x.set16(cam::X, px);
        x.set16(cam::DRAWN_X, px);
        x.add16(cam::Y, 0x100);
        x.add16(cam::DRAWN_Y, 0x100);
        x.add8(f::STEP, 2);
    } else if (px as i16) >= 0x600 {
        x.add16(cam::X, 0xfe00);
        x.add16(cam::DRAWN_X, 0xfe00);
    }
}

/// 0x33152: a scenery object is created when the scenery has arrived at the bottom.
fn rubble(x: &mut Cam) {
    let Some(Obj(obj)) = x.alloc(&pools::ENEMY) else { return };
    let v = rd8(x.b, obj).wrapping_add(1);
    wr8(x.b, obj, v);
    wr8(x.b, obj + 2, 0x15);
    x.set16(cam::MODE, mode2::RUBBLE);
}

/// Runs the script of camera 2's mode (A0).
fn run_mode_2(x: &mut Cam, mode: u16) {
    match mode {
        mode2::STAGE => stage_script_2(x),
        mode2::FREE => {}
        0x08 => {
            let dx = delta(x, 0);
            drift(x, dx >> 1, 0);
            stage_script_2(x);
        }
        0x0c => {
            let (dx, dy) = (delta(x, 0), delta(x, 1));
            drift(x, dx << 1, dy << 1);
            stage_script_2(x);
        }
        0x10 | 0x20 => strolling(x),
        mode2::HALF => {
            let (dx, dy) = (delta(x, 0), delta(x, 1));
            drift(x, dx >> 1, dy >> 1);
            stage_script_2(x);
        }
        mode2::FULL => {
            let (dx, dy) = (delta(x, 0), delta(x, 1));
            drift(x, dx, dy);
            stage_script_2(x);
        }
        mode2::DRIFT => {
            x.add32(cam::X, 0x4000);
            let px = x.u16(cam::X);
            x.set16(cam::DRAWN_X, px);
        }
        0x24 => {
            let (dx, dy) = (delta(x, 0), delta(x, 1));
            drift(x, (dx >> 1).wrapping_add(dx), dy >> 1);
            stage_script_2(x);
        }
        0x28 => {
            let dx = delta(x, 0);
            drift(x, dx, 0);
            let y = x.u16(cam::Y).wrapping_sub(8);
            x.set16(cam::Y, y);
            if (y as i16) <= 0 {
                x.add16(cam::X, 0x400);
                x.add16(cam::DRAWN_X, 0x400);
                x.set16(cam::MODE, mode2::RUBBLE);
                rubble(x);
            }
        }
        mode2::QUARTER_X => {
            let dx = delta(x, 0);
            drift(x, dx >> 2, 0);
            stage_script_2(x);
        }
        0x30 => {
            let dx = delta(x, 0);
            drift(x, dx, 0);
            if (x.u16(cam::Y) as i16) >= 944 {
                rubble(x);
            } else {
                x.add16(cam::Y, 3);
            }
        }
        mode2::RUBBLE => {
            let dx = delta(x, 0);
            drift(x, dx, 0);
        }
        mode2::QUARTER_THREE => crate::scroll::parallax_quarter(x.b, x.o),
        0x3c => crate::scroll::parallax_three_quarters(x.b, x.o),
        mode2::FOLLOWED => {
            let d = crate::mem::rd32(x.b, A5 + CAM2_X).wrapping_sub(crate::mem::rd32(x.b, A5 + CAM2_PREVIOUS_X));
            let at = A5 + CAMERA_1 + cam::X;
            let v = crate::mem::rd32(x.b, at).wrapping_add(d);
            wr32(x.b, at, v);
            stage_script_2(x);
        }
        mode2::QUARTER_BOTH => {
            let (dx, dy) = (delta(x, 0), delta(x, 1));
            drift(x, dx >> 2, dy >> 2);
        }
        mode2::CREEP_6 => {
            crate::scroll::creep(x.b, x.o, 6);
        }
        mode2::HALF_SYNC => {
            let d = (delta(x, 0) >> 1) as u32;
            x.add32(cam::X, d);
            x.add32(cam::DRAWN_X, d);
            stage_script_2(x);
        }
        mode2::HALF_X => {
            let dx = delta(x, 0);
            drift(x, dx >> 1, 0);
            stage_script_2(x);
        }
        0x54 | 0x6c | 0x70 => shake(x, 1),
        0x58 => shake(x, 2),
        0x5c => shake(x, 3),
        mode2::SCREEN_X => {
            let v = rd16(x.b, A5 + 0x5a6).wrapping_add(x.u16(0x30));
            x.set16(cam::X, v.wrapping_sub(0x90));
        }
        mode2::WAIT_THEN_CREEP => match x.u8(f::STEP) {
            0 => {
                x.add8(f::STEP, 4);
                let px = x.u16(cam::X);
                x.set16(cam::DRAWN_X, px);
            }
            8 => {
                x.add16(cam::X, 4);
                let px = x.u16(cam::X);
                x.set16(cam::DRAWN_X, px);
            }
            0xc => x.set16(cam::MODE, mode2::CREEP_6),
            _ => {}
        },
        mode2::CREEP_3 => {
            crate::scroll::creep(x.b, x.o, 3);
        }
        mode2::FROM_CAMERA_1 => {
            let y = ((rd16(x.b, A5 + CAMERA_1 + cam::X) & 0x30) << 4).wrapping_add(8);
            x.set16(cam::Y, y);
            x.set16(cam::DRAWN_Y, y);
        }
        mode2::DRIVES_CAMERA_1 => {
            let rd = |x: &mut Cam, a: u32| crate::mem::rd32(x.b, A5 + a);
            let dx = (rd(x, CAM2_X).wrapping_sub(rd(x, CAM2_PREVIOUS_X)) as i32) >> 1;
            let dy = (rd(x, CAM2_Y).wrapping_sub(rd(x, CAM2_PREVIOUS_Y)) as i32) >> 1;
            let (x1, y1) = (A5 + CAMERA_1 + cam::X, A5 + CAMERA_1 + cam::Y);
            let v = crate::mem::rd32(x.b, x1).wrapping_add(dx as u32);
            wr32(x.b, x1, v);
            let v = crate::mem::rd32(x.b, y1).wrapping_add(dy as u32);
            wr32(x.b, y1, v);
            if (rd16(x.b, x1) as i16) > 576 {
                wr16(x.b, x1, 576);
            }
            stage_script_2(x);
        }
        _ => {}
    }
}

/// 0x32898: one step of the initialised camera 2: mode script, then scenery copy.
fn step_2(b: &mut Cps2Bus) -> crate::scroll::D4Leftovers {
    let mut x = Cam { b, o: Obj(A5 + CAMERA_2) };
    x.set8(f::SHAKING, 0);
    let m = x.u16(cam::MODE);
    run_mode_2(&mut x, m);
    crate::scroll::draw2(x.b)
}

/// Clears the per-line scroll bit of the video register copy (without writing the register).
fn clear_rowscroll_flag(x: &mut Cam) {
    let v = rd16(x.b, A5 + VIDEO_CONTROL_COPY) & !1;
    wr16(x.b, A5 + VIDEO_CONTROL_COPY, v);
}

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

/// Native: D4 of the copy, A0 = the output camera.
fn camera1_update(c: &mut Cpu, b: &mut Cps2Bus) {
    if let Some((v, m)) = update_camera_1(b) {
        c.d[4] = c.d[4] & !m | v;
    }
    c.a[0] = A5 + CAMERA_1;
    rts(c, b);
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

/// Native: D4 of the copy, A0 = the output camera.
fn camera2_update(c: &mut Cpu, b: &mut Cps2Bus) {
    if let Some((v, m)) = update_camera_2(b) {
        c.d[4] = c.d[4] & !m | v;
    }
    c.a[0] = A5 + CAMERA_2;
    rts(c, b);
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

pub const CAMERA1_UPDATE: Native = camera(0x2ede0, "camera1_mise_a_jour", camera1_update);
pub const CAMERA2_UPDATE: Native = camera(0x32108, "camera2_mise_a_jour", camera2_update);

const fn camera(entry: u32, name: &'static str, run: fn(&mut Cpu, &mut Cps2Bus)) -> Native {
    Native { entry, name, run: crate::Body::Processeur(run), touches_sound: false, outputs: regs::a(0) }
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

pub const NATIVES: &[Native] = &[CAMERA1_UPDATE, CAMERA2_UPDATE];
