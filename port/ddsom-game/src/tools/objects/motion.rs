//! Tools: the natives of `crate::world::objects::motion` for the original code that calls them (arguments and results in the registers
//! of the original).

use crate::world::World;
use m68k::decode::Size;
use m68k::{Cpu, SR_C, SR_N, SR_X, SR_Z};

use crate::world::objects::motion::*;
use crate::tools::prog32;
use crate::tools::frames::*;
use crate::{regs, Native};
use crate::world::objects::Obj;

macro_rules! motion_native {
    ($const:ident, $entry:expr, $name:expr, $run:expr) => {
        pub(crate) const $const: Native = Native {
            entry: $entry,
            name: $name,
            run: crate::Body::WithCpu(|c, b| {
                let obj = Obj(c.a[0]);
                $run(c, b, obj);
                rts(c, b);
            }),
            touches_sound: false,
            outputs: regs::ALL,
        };
    };
}

/// `cmp`: flags of dst - src, X unchanged.
pub(crate) fn cmp(c: &mut Cpu, src: u32, dst: u32, size: Size) {
    let x = c.sr & SR_X;
    c.addsub(true, src, dst, false, size, false);
    c.sr = (c.sr & !SR_X) | x;
}

/// `lsl.w #n`: result and flags (N, Z, C = X = last bit shifted out, V = 0).
pub(crate) fn lsl_word(c: &mut Cpu, v: u16, n: u32) -> u16 {
    let wide = (v as u32) << n;
    let r = wide as u16;
    let out = (wide >> 16) & 1 != 0;
    let mut f = 0;
    if r & 0x8000 != 0 {
        f |= SR_N;
    }
    if r == 0 {
        f |= SR_Z;
    }
    if out {
        f |= SR_C | SR_X;
    }
    c.sr = (c.sr & !0x1f) | f;
    r
}

/// Flags of the last addition `add.l D0,(position)`.
fn add_flags(c: &mut Cpu, old: u32, delta: u32) {
    c.addsub(false, delta, old, false, Size::Long, false);
}

fn ballistic_regs(c: &mut Cpu, b: &mut World, obj: Obj, axes: Axes) -> LastAdd {
    let mut last = LastAdd { old: 0, delta: 0 };
    for &axis in axes {
        let (old, delta) = advance(b, obj, axis);
        add_flags(c, old, delta);
        c.d[0] = delta;
        last = LastAdd { old, delta };
    }
    last
}

fn by_table_regs(c: &mut Cpu, b: &mut World, obj: Obj, second: Axis) -> LastAdd {
    c.d[0] = (obj.u8(b, Obj::SPEED_INDEX) as u32) << 2;
    c.a[4] = obj.u32(b, Obj::SPEED_TABLE);
    advance_by_table(b, obj, AXIS_X, 0);
    let (old, delta) = advance_by_table(b, obj, second, 2);
    add_flags(c, old, delta);
    c.d[1] = delta;
    LastAdd { old, delta }
}

/// Speed table selection: D0 = table number. Leaves D0 = index << 2 and A4 = table.
fn speed_regs(c: &mut Cpu, b: &mut World, obj: Obj, tables: &SpeedTables) {
    let n = lsl_word(c, c.d[0] as u16, 2);
    let table = prog32(b, tables.pointers.wrapping_add(n as i16 as i32 as u32));
    select_speed_table(b, obj, table);
    let index = lsl_word(c, obj.u8(b, Obj::SPEED_INDEX) as u16, 2);
    c.d[0] = index as u32;
    c.a[4] = table;
    c.flags_logic(obj.u16(b, Obj::VY) as u32, Size::Word);
}

/// Flags left by 0xf1bfc (without the writes, done by `tick_counters`).
fn countdown_flags(c: &mut Cpu, b: &mut World, obj: Obj) {
    let v53 = obj.u8(b, Obj::COUNTER_53);
    c.flags_logic(v53 as u32, Size::Byte);
    if (v53 as i8) < 0 {
        c.flags_logic(0, Size::Byte);
    }
    let left = obj.u8(b, Obj::COUNTDOWN);
    c.flags_logic(left as u32, Size::Byte);
    if left != 0 {
        c.addsub(true, 1, left as u32, false, Size::Byte, false);
        if left as i8 <= 1 {
            c.flags_logic(0, Size::Byte);
        }
    }
}

motion_native!(MOVE_XY, 0x1d62, "obj_move_xy", |c, b, o| _ = ballistic_regs(c, b, o, XY));

motion_native!(MOVE_X, 0x1d78, "obj_move_x", |c, b, o| _ = ballistic_regs(c, b, o, X));

motion_native!(MOVE_XYZ, 0x1d90, "obj_move_xyz", |c, b, o| _ = ballistic_regs(c, b, o, XYZ));

motion_native!(MOVE_XZ, 0x1da6, "obj_move_xz", |c, b, o| _ = ballistic_regs(c, b, o, XZ));

motion_native!(MOVE_Z, 0x1dbc, "obj_move_z", |c, b, o| _ = ballistic_regs(c, b, o, Z));

motion_native!(MOVE_YZ, 0x1dd4, "obj_move_yz", |c, b, o| _ = ballistic_regs(c, b, o, YZ));

motion_native!(MOVE_Y, 0x1e02, "obj_move_y", |c, b, o| _ = ballistic_regs(c, b, o, Y));

motion_native!(MOVE_BY_TABLE_XY, 0x1cb4, "obj_move_table_xy", |c, b, o| _ = by_table_regs(c, b, o, AXIS_Y));

motion_native!(MOVE_BY_TABLE_XZ, 0x1cee, "obj_move_table_xz", |c, b, o| _ = by_table_regs(c, b, o, AXIS_Z));

motion_native!(SPEED_TABLE_A, 0x1a94, "obj_select_speed_table_a", |c, b, o| speed_regs(c, b, o, &SPEEDS_A));

motion_native!(SPEED_TABLE_B, 0x1b64, "obj_select_speed_table_b", |c, b, o| speed_regs(c, b, o, &SPEEDS_B));

motion_native!(PAUSE_HOOK, 0x29ea, "obj_pause_hook", |c: &mut Cpu, b: &mut World, o: Obj| {
    let v = o.u8(b, Obj::HIDDEN);
    c.flags_logic(v as u32, Size::Byte);
});

motion_native!(COUNTDOWNS, 0xf1bfc, "obj_tick_counters", |c: &mut Cpu, b: &mut World, o: Obj| {
    countdown_flags(c, b, o);
    tick_counters(b, o);
});
