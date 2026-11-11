//! Speeds, accelerations and speed tables of the objects.
//!
//! Units: speeds and accelerations in 8.8 (pixels/frame), positions in 16.16 (pixels). Three axes:
//! X (abscissa), Y (depth on the ground, sort key) and Z (altitude).
//!
//! Two ways to move:
//! - *ballistic*: `speed += acceleration ; position += speed` (`advance`);
//! - *by table*: the speed is read from the table `Obj::SPEED_TABLE` at index `Obj::SPEED_INDEX`
//!   (two words per index: vx then vy or vz), the acceleration is added to it (`advance_by_table`).

use crate::world::World;

use super::Obj;
use crate::world::data::{data, Missing, RomAddr};

/// One axis of motion: offsets of the speed, the acceleration and the position.
#[derive(Clone, Copy)]
pub struct Axis {
    pub vel: u32,
    pub acc: u32,
    pub pos: u32,
}

pub const AXIS_X: Axis = Axis { vel: Obj::VX, acc: Obj::AX, pos: Obj::X };
pub const AXIS_Y: Axis = Axis { vel: Obj::VY, acc: Obj::AY, pos: Obj::Y };
pub const AXIS_Z: Axis = Axis { vel: Obj::VZ, acc: Obj::AZ, pos: Obj::Z };

/// Signed 8.8 speed converted to a 16.16 displacement.
fn to_displacement(vel: u16) -> u32 {
    ((vel as i16 as i32) << 8) as u32
}

/// Applies `delta` to the position of the axis; returns the old position.
fn displace(b: &mut World, obj: Obj, axis: Axis, delta: u32) -> u32 {
    let old = obj.u32(b, axis.pos);
    obj.set_u32(b, axis.pos, old.wrapping_add(delta));
    old
}

/// Ballistic motion: speed += acceleration, position += speed.
/// Returns (old position, 16.16 displacement).
pub fn advance(b: &mut World, obj: Obj, axis: Axis) -> (u32, u32) {
    let vel = obj.u16(b, axis.vel).wrapping_add(obj.u16(b, axis.acc));
    obj.set_u16(b, axis.vel, vel);
    let delta = to_displacement(vel);
    (displace(b, obj, axis, delta), delta)
}

/// Speed word at byte `offset` of the speed table `table` (the original reads `table + offset`, past the end of the
/// table if the index goes beyond it).
fn speed(b: &mut World, table: u32, offset: u32) -> u16 {
    let a = table.wrapping_add(offset);
    match data().speeds.at(RomAddr(a)) {
        Some(v) => v,
        None => Missing { table: "speed table", key: table }.read16(b, a),
    }
}

/// Motion by table: speed = table value + acceleration, position += speed.
/// `entry`: 0 for the speed in X, 2 for that in Y or Z. Returns (old position, displacement).
pub fn advance_by_table(b: &mut World, obj: Obj, axis: Axis, entry: u32) -> (u32, u32) {
    let table = obj.speed_table(b);
    let index = (obj.speed_index(b) as u32) << 2;
    let base = speed(b, table, index + entry);
    let vel = base.wrapping_add(obj.u16(b, axis.acc));
    obj.set_u16(b, axis.vel, vel);
    let delta = to_displacement(vel);
    (displace(b, obj, axis, delta), delta)
}

/// Selects the speed table `table` (address) for the object and takes its first speed (current
/// index) in X and in Y.
pub fn select_speed_table(b: &mut World, obj: Obj, table: u32) {
    obj.set_speed_table(b, table);
    let index = (obj.speed_index(b) as u32) << 2;
    let vx = speed(b, table, index);
    let vy = speed(b, table, index + 2);
    obj.set_vx(b, vx);
    obj.set_vy(b, vy);
}

/// Last addition `add.l D0,(position)` of a motion: the original handlers test its flags right
/// after (`bgt`: the object is still in the air...). The conditions are computed like the 68000, without registers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LastAdd {
    /// Position before the addition.
    pub old: u32,
    /// Displacement added (16.16).
    pub delta: u32,
}

impl LastAdd {
    /// New position.
    pub fn result(self) -> u32 {
        self.old.wrapping_add(self.delta)
    }
    /// N: negative result.
    pub fn mi(self) -> bool {
        (self.result() as i32) < 0
    }
    /// N clear (`bpl`).
    pub fn pl(self) -> bool {
        !self.mi()
    }
    /// Z: zero result.
    pub fn zero(self) -> bool {
        self.result() == 0
    }
    /// V: signed overflow.
    pub fn overflow(self) -> bool {
        let r = self.result();
        ((self.old ^ r) & (self.delta ^ r)) >> 31 != 0
    }
    /// `bgt`: Z clear and N = V.
    pub fn gt(self) -> bool {
        !self.zero() && self.mi() == self.overflow()
    }
    /// `ble`: Z, or N ≠ V.
    pub fn le(self) -> bool {
        !self.gt()
    }
}

/// Chained ballistic motions of an original routine (axis order of the original).
pub type Axes = &'static [Axis];
/// 0x1d62: Y then X.
pub const XY: Axes = &[AXIS_Y, AXIS_X];
/// 0x1d78: X only.
pub const X: Axes = &[AXIS_X];
/// 0x1d90: Y, X then Z.
pub const XYZ: Axes = &[AXIS_Y, AXIS_X, AXIS_Z];
/// 0x1da6: X then Z.
pub const XZ: Axes = &[AXIS_X, AXIS_Z];
/// 0x1dbc: Z only.
pub const Z: Axes = &[AXIS_Z];
/// 0x1dd4: Y then Z.
pub const YZ: Axes = &[AXIS_Y, AXIS_Z];
/// 0x1e02: Y only.
pub const Y: Axes = &[AXIS_Y];

/// Ballistic motion on each of the axes, in order (`move_xy`, `move_xz`... of the original). Returns the addition
/// of the last axis.
pub fn ballistic(b: &mut World, obj: Obj, axes: Axes) -> LastAdd {
    let mut last = LastAdd { old: 0, delta: 0 };
    for &axis in axes {
        let (old, delta) = advance(b, obj, axis);
        last = LastAdd { old, delta };
    }
    last
}

/// Motion by table on X then on `second` (Y: 0x1cb4, Z: 0x1cee). Returns the addition of the second axis.
pub fn move_by_table(b: &mut World, obj: Obj, second: Axis) -> LastAdd {
    advance_by_table(b, obj, AXIS_X, 0);
    let (old, delta) = advance_by_table(b, obj, second, 2);
    LastAdd { old, delta }
}

/// Array of speed tables, read in the code: table `n` is pointer `n`.
pub struct SpeedTables {
    /// Original routine that selects a table from this array.
    pub routine: u32,
    /// Pointer array (program space).
    pub pointers: u32,
}

/// 0x1a94: first array of speed tables.
pub const SPEEDS_A: SpeedTables = SpeedTables { routine: 0x1a94, pointers: 0x1ab4 };
/// 0x1b64: second array (the most used).
pub const SPEEDS_B: SpeedTables = SpeedTables { routine: 0x1b64, pointers: 0x1b84 };

/// Selects the speed table `n` of the array `tables` and takes its current speed in X and in Y.
pub fn select_speed(b: &mut World, obj: Obj, tables: &SpeedTables, n: u16) {
    let index = (n << 2) as i16 as i32;
    let table = match data().speeds.array(RomAddr(tables.pointers)).and_then(|p| p.get(index / 4)) {
        Some(t) => t.0,
        None => Missing { table: "speed array", key: tables.pointers }.pc_long(b, tables.pointers.wrapping_add(index as u32)),
    };
    select_speed_table(b, obj, table);
}

/// 0x29ea: extension point left empty by the game; only tests the byte `Obj::HIDDEN`. True: object
/// suspended (the original caller chains on `bne`).
pub fn suspended(b: &mut World, obj: Obj) -> bool {
    obj.hidden(b) != 0
}

/// 0xf1bfc: end-of-handler counters. A negative byte in `COUNTER_53` is reset to zero; the
/// countdown `COUNTDOWN` decreases and, when it reaches zero (or less), clears `COUNTDOWN_FLAG`.
pub fn tick_counters(b: &mut World, obj: Obj) {
    if (obj.counter_53(b) as i8) < 0 {
        obj.set_counter_53(b, 0);
    }
    let left = obj.countdown(b);
    if left != 0 {
        obj.set_countdown(b, left.wrapping_sub(1));
        if left as i8 <= 1 {
            obj.set_countdown_flag(b, 0);
        }
    }
}

// ---- 68000 flags ----

// ---- adaptation to the 68000 registers ----

// ---- natives (remaining calls by address) ----
