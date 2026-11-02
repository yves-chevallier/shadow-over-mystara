//! Tools: the natives of `crate::effects_aids` for the original code that calls them (arguments and results in the registers
//! of the original).

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::{Cpu, SR_C, SR_N, SR_V, SR_X, SR_Z};

use crate::effects_aids::*;
use crate::tools::frames::*;
use crate::{regs, Body, Native};
use crate::families::effects_creators::leftovers::{Pure, Regs};
use crate::mem::{rd16, rd32, rd8, wr16, wr8, A5};
use crate::objects::{pools, Obj};

macro_rules! leaf {
    ($entry:literal, $name:literal, $body:path, $outputs:expr) => {
        aid($entry, $name, |c, b| {
            $body(c, b);
            rts(c, b);
        }, $outputs)
    };
}

/// `shock` with the registers of the original (callers by address: `player/`): A0 = the creator; A1, D1.w (high byte
/// of the setting), carry and flags as in the original.
#[track_caller]
pub fn shock_68k(c: &mut Cpu, b: &mut Cps2Bus, creator: Obj, variant: u8, setting: u16, vx: u16, vy: u16) -> Option<Obj> {
    c.a[0] = creator.0;
    let r = shock_by(c, b, creator, variant, setting, vx, vy);
    crate::tools::diag::dead_regs(c, "effects", regs::a(1) | regs::d(1) | regs::CCR);
    r
}

fn shock_native(c: &mut Cpu, b: &mut Cps2Bus) {
    let (me, variant, setting, vx, vy) = (Obj(c.a[0]), c.d[0] as u8, c.d[1] as u16, c.d[2] as u16, c.d[3] as u16);
    shock_by(c, b, me, variant, setting, vx, vy);
    rts(c, b);
}

/// Natives of the sparks: A0 = the creator, A1 = the other object, D1.b = variant; A1 to A4 are returned.
fn spark_native(c: &mut Cpu, b: &mut Cps2Bus, between: Option<bool>) {
    let saved = [c.a[1], c.a[2], c.a[3], c.a[4]];
    let (me, other, variant) = (Obj(c.a[0]), c.a[1], c.d[1] as u8);
    match between {
        None => spark_by(c, b, me, other, variant),
        Some(owner_first) => spark_between_by(c, b, me, other, variant, owner_first),
    };
    [c.a[1], c.a[2], c.a[3], c.a[4]] = saved;
    rts(c, b);
}

/// Creates an object of type `kind` in `pool`, variant D0; flags of D0.b.
fn spawn(c: &mut Cpu, b: &mut Cps2Bus, pool: &pools::Pool, kind: u8) -> Option<Obj> {
    let object = pool.alloc_68k(c, b)?;
    object.set_u8(b, Obj::ACTIVE, 1);
    object.set_u8(b, Obj::KIND, kind);
    object.set_u8(b, Obj::VARIANT, c.d[0] as u8);
    c.flags_logic(c.d[0] & 0xff, Size::Byte);
    Some(object)
}

/// 0xddeaa: scenery of type 6.
fn spawn_scenery_6(c: &mut Cpu, b: &mut Cps2Bus) {
    spawn(c, b, &pools::SOBJ, 6);
}

/// 0xdfa4a: scenery of type 11, oriented according to D1.
fn spawn_scenery_11(c: &mut Cpu, b: &mut Cps2Bus) {
    if let Some(object) = spawn(c, b, &pools::SOBJ, 11) {
        object.set_u8(b, 0x2c, c.d[1] as u8);
        c.flags_logic(c.d[1] & 0xff, Size::Byte);
    }
}

/// 0x517ba: player effect of type 44; its setting `+0x2f` is the next byte in the A4 table.
fn spawn_player_effect_44(c: &mut Cpu, b: &mut Cps2Bus) {
    if let Some(object) = spawn(c, b, &pools::PEFF, 44) {
        let setting = rd8(b, c.a[4]);
        c.a[4] = c.a[4].wrapping_add(1);
        object.set_u8(b, 0x2f, setting);
    }
}

fn count_living_players(c: &mut Cpu, b: &mut Cps2Bus) {
    if let Some(alive) = living_players(b, Obj(c.a[0])) {
        c.d[0] = alive as u32;
    }
}

/// 0x25260: a scripted player places itself in front of the object (`A5+0x46`), shifted according to its number (`+2`).
fn place_scripted_player(c: &mut Cpu, b: &mut Cps2Bus) {
    /// X offsets by player number (words, program space: 0x40, 0x10, -0x18, -0x44).
    const OFFSETS: u32 = 0x2528c;
    let me = c.a[0];
    let focus = rd16(b, A5 + 0x46) as i16 as i32 as u32;
    c.a[1] = focus;
    let x = rd16(b, focus + 0x08);
    wr16(b, me + 0x08, x);
    let depth = rd16(b, focus + 0x10);
    wr16(b, me + 0x10, depth);
    wr16(b, me + 0x0c, 0);
    let index = 2 * rd8(b, me + 2) as u32;
    let shift = crate::mem::prog16(b, OFFSETS + index);
    c.d[0] = shift as u32;
    let x = rd16(b, me + 0x08).wrapping_add(shift);
    wr16(b, me + 0x08, x);
    let depth = crate::tools::ops::sub(c, 18, rd16(b, me + 0x10) as u32, Size::Word);
    wr16(b, me + 0x10, depth as u16);
}

const fn aid(entry: u32, name: &'static str, run: fn(&mut Cpu, &mut Cps2Bus), outputs: u32) -> Native {
    Native { entry, name, run: crate::Body::WithCpu(run), touches_sound: false, outputs }
}

pub const NATIVES: &[Native] = &[
    leaf!(0xddeaa, "scenery_type_6", spawn_scenery_6, regs::a(1) | regs::CCR),
    leaf!(0xdfa4a, "scenery_type_11", spawn_scenery_11, regs::a(1) | regs::CCR),
    leaf!(0x517ba, "effect_player_44", spawn_player_effect_44, regs::a(1) | regs::a(4) | regs::CCR),
    leaf!(0xcc18e, "count_living_players", count_living_players, regs::d(0)),
    leaf!(0x25260, "player_scripted_place", place_scripted_player, regs::a(1)),
    aid(0xb10c8, "effect_shock", shock_native, regs::a(1) | regs::d(1) | regs::CCR),
    aid(0xaa96c, "effect_spark", |c, b| spark_native(c, b, None), regs::a(1) | regs::a(2) | regs::a(3) | regs::a(4)),
    aid(0xaaa04, "effect_spark_between", |c, b| spark_native(c, b, Some(true)), regs::ALL),
    aid(0xaaa84, "effect_spark_between_b", |c, b| spark_native(c, b, Some(false)), regs::ALL),
];
