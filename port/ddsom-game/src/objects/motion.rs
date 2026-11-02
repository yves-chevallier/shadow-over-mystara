//! Speeds, accelerations and speed tables of the objects.
//!
//! Units: speeds and accelerations in 8.8 (pixels/frame), positions in 16.16 (pixels). Three axes:
//! X (abscissa), Y (depth on the ground, sort key) and Z (altitude).
//!
//! Two ways to move:
//! - *ballistic*: `speed += acceleration ; position += speed` (`advance`);
//! - *by table*: the speed is read from the table `Obj::SPEED_TABLE` at index `Obj::SPEED_INDEX`
//!   (two words per index: vx then vy or vz), the acceleration is added to it (`advance_by_table`).

use cps2::Cps2Bus;

use super::Obj;
use crate::mem::rd16;
use crate::regs;

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
fn displace(b: &mut Cps2Bus, obj: Obj, axis: Axis, delta: u32) -> u32 {
    let old = obj.u32(b, axis.pos);
    obj.set_u32(b, axis.pos, old.wrapping_add(delta));
    old
}

/// Ballistic motion: speed += acceleration, position += speed.
/// Returns (old position, 16.16 displacement).
pub fn advance(b: &mut Cps2Bus, obj: Obj, axis: Axis) -> (u32, u32) {
    let vel = obj.u16(b, axis.vel).wrapping_add(obj.u16(b, axis.acc));
    obj.set_u16(b, axis.vel, vel);
    let delta = to_displacement(vel);
    (displace(b, obj, axis, delta), delta)
}

/// Motion by table: speed = table value + acceleration, position += speed.
/// `entry`: 0 for the speed in X, 2 for that in Y or Z. Returns (old position, displacement).
pub fn advance_by_table(b: &mut Cps2Bus, obj: Obj, axis: Axis, entry: u32) -> (u32, u32) {
    let table = obj.u32(b, Obj::SPEED_TABLE);
    let index = (obj.u8(b, Obj::SPEED_INDEX) as u32) << 2;
    let base = rd16(b, table.wrapping_add(index + entry));
    let vel = base.wrapping_add(obj.u16(b, axis.acc));
    obj.set_u16(b, axis.vel, vel);
    let delta = to_displacement(vel);
    (displace(b, obj, axis, delta), delta)
}

/// Selects the speed table `table` (address) for the object and takes its first speed (current
/// index) in X and in Y.
pub fn select_speed_table(b: &mut Cps2Bus, obj: Obj, table: u32) {
    obj.set_u32(b, Obj::SPEED_TABLE, table);
    let index = (obj.u8(b, Obj::SPEED_INDEX) as u32) << 2;
    let vx = rd16(b, table.wrapping_add(index));
    let vy = rd16(b, table.wrapping_add(index + 2));
    obj.set_u16(b, Obj::VX, vx);
    obj.set_u16(b, Obj::VY, vy);
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
pub fn ballistic(b: &mut Cps2Bus, obj: Obj, axes: Axes) -> LastAdd {
    let mut last = LastAdd { old: 0, delta: 0 };
    for &axis in axes {
        let (old, delta) = advance(b, obj, axis);
        last = LastAdd { old, delta };
    }
    last
}

/// Motion by table on X then on `second` (Y: 0x1cb4, Z: 0x1cee). Returns the addition of the second axis.
pub fn move_by_table(b: &mut Cps2Bus, obj: Obj, second: Axis) -> LastAdd {
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
pub fn select_speed(b: &mut Cps2Bus, obj: Obj, tables: &SpeedTables, n: u16) {
    let table = prog32(b, tables.pointers.wrapping_add((n << 2) as i16 as i32 as u32));
    select_speed_table(b, obj, table);
}

/// 0x29ea: extension point left empty by the game; only tests the byte `Obj::HIDDEN`. True: object
/// suspended (the original caller chains on `bne`).
pub fn suspended(b: &mut Cps2Bus, obj: Obj) -> bool {
    obj.u8(b, Obj::HIDDEN) != 0
}

/// 0xf1bfc: end-of-handler counters. A negative byte in `COUNTER_53` is reset to zero; the
/// countdown `COUNTDOWN` decreases and, when it reaches zero (or less), clears `COUNTDOWN_FLAG`.
pub fn tick_counters(b: &mut Cps2Bus, obj: Obj) {
    if (obj.u8(b, Obj::COUNTER_53) as i8) < 0 {
        obj.set_u8(b, Obj::COUNTER_53, 0);
    }
    let left = obj.u8(b, Obj::COUNTDOWN);
    if left != 0 {
        obj.set_u8(b, Obj::COUNTDOWN, left.wrapping_sub(1));
        if left as i8 <= 1 {
            obj.set_u8(b, Obj::COUNTDOWN_FLAG, 0);
        }
    }
}

/// Reading of a long in program space (tables embedded in the code, read PC-relative).
pub(crate) fn prog32(b: &mut Cps2Bus, a: u32) -> u32 {
    (b.fetch16(a & 0xff_ffff) as u32) << 16 | b.fetch16(a.wrapping_add(2) & 0xff_ffff) as u32
}

// ---- 68000 flags ----

// ---- adaptation to the 68000 registers ----

/// Registers left behind by the original routines (see `diag::dead_regs`, label `motion`).
pub const REGS_BALLISTIC: u32 = regs::d(0) | regs::CCR;
pub const REGS_BY_TABLE: u32 = regs::d(0) | regs::d(1) | regs::a(4) | regs::CCR;
pub const REGS_SPEED: u32 = regs::d(0) | regs::a(4) | regs::CCR;

// ---- natives (remaining calls by address) ----
