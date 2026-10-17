//! Vitesses, accélérations et tables de vitesse des objets.
//!
//! Unités : vitesses et accélérations en 8.8 (pixels/image), positions en 16.16 (pixels). Trois axes :
//! X (abscisse), Y (profondeur sur le sol, clé de tri) et Z (altitude).
//!
//! Deux façons de bouger :
//! - *balistique* : `vitesse += accélération ; position += vitesse` (`advance`) ;
//! - *par table* : la vitesse est lue dans la table `Obj::SPEED_TABLE` à l'index `Obj::SPEED_INDEX`
//!   (deux mots par index : vx puis vy ou vz), on y ajoute l'accélération (`advance_by_table`).

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::{Bus, Cpu, SR_C, SR_N, SR_X, SR_Z};

use super::Obj;
use crate::mem::{rd16, rd8, rts};
use crate::timing::path_cycles;
use crate::{regs, Native};

/// Un axe du mouvement : décalages de la vitesse, de l'accélération et de la position.
#[derive(Clone, Copy)]
pub struct Axis {
    pub vel: u32,
    pub acc: u32,
    pub pos: u32,
}

pub const AXIS_X: Axis = Axis { vel: Obj::VX, acc: Obj::AX, pos: Obj::X };
pub const AXIS_Y: Axis = Axis { vel: Obj::VY, acc: Obj::AY, pos: Obj::Y };
pub const AXIS_Z: Axis = Axis { vel: Obj::VZ, acc: Obj::AZ, pos: Obj::Z };

/// Vitesse 8.8 signée convertie en déplacement 16.16.
fn to_displacement(vel: u16) -> u32 {
    ((vel as i16 as i32) << 8) as u32
}

/// Applique `delta` à la position de l'axe ; renvoie l'ancienne position.
fn displace(b: &mut Cps2Bus, obj: Obj, axis: Axis, delta: u32) -> u32 {
    let old = obj.u32(b, axis.pos);
    obj.set_u32(b, axis.pos, old.wrapping_add(delta));
    old
}

/// Mouvement balistique : vitesse += accélération, position += vitesse.
/// Renvoie (ancienne position, déplacement 16.16).
pub fn advance(b: &mut Cps2Bus, obj: Obj, axis: Axis) -> (u32, u32) {
    let vel = obj.u16(b, axis.vel).wrapping_add(obj.u16(b, axis.acc));
    obj.set_u16(b, axis.vel, vel);
    let delta = to_displacement(vel);
    (displace(b, obj, axis, delta), delta)
}

/// Mouvement par table : vitesse = valeur de la table + accélération, position += vitesse.
/// `entry` : 0 pour la vitesse en X, 2 pour celle en Y ou Z. Renvoie (ancienne position, déplacement).
pub fn advance_by_table(b: &mut Cps2Bus, obj: Obj, axis: Axis, entry: u32) -> (u32, u32) {
    let table = obj.u32(b, Obj::SPEED_TABLE);
    let index = (obj.u8(b, Obj::SPEED_INDEX) as u32) << 2;
    let base = rd16(b, table.wrapping_add(index + entry));
    let vel = base.wrapping_add(obj.u16(b, axis.acc));
    obj.set_u16(b, axis.vel, vel);
    let delta = to_displacement(vel);
    (displace(b, obj, axis, delta), delta)
}

/// Choisit la table de vitesses `table` (adresse) pour l'objet et prend sa première vitesse (index
/// courant) en X et en Y.
pub fn select_speed_table(b: &mut Cps2Bus, obj: Obj, table: u32) {
    obj.set_u32(b, Obj::SPEED_TABLE, table);
    let index = (obj.u8(b, Obj::SPEED_INDEX) as u32) << 2;
    let vx = rd16(b, table.wrapping_add(index));
    let vy = rd16(b, table.wrapping_add(index + 2));
    obj.set_u16(b, Obj::VX, vx);
    obj.set_u16(b, Obj::VY, vy);
}

// ---- drapeaux du 68000 ----

/// `cmp` : drapeaux de dst - src, X inchangé.
pub(super) fn cmp(c: &mut Cpu, src: u32, dst: u32, size: Size) {
    let x = c.sr & SR_X;
    c.addsub(true, src, dst, false, size, false);
    c.sr = (c.sr & !SR_X) | x;
}

/// `lsl.w #n` : résultat et drapeaux (N, Z, C = X = dernier bit sorti, V = 0).
pub(super) fn lsl_word(c: &mut Cpu, v: u16, n: u32) -> u16 {
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

/// Flags de la dernière addition `add.l D0,(position)`.
fn add_flags(c: &mut Cpu, old: u32, delta: u32) {
    c.addsub(false, delta, old, false, Size::Long, false);
}

// ---- adaptation aux registres du 68000 ----

/// Enchaîne les mouvements balistiques ; D0 = déplacement du dernier, flags de sa dernière addition.
fn ballistic(c: &mut Cpu, b: &mut Cps2Bus, axes: &[Axis]) {
    let obj = Obj(c.a[0]);
    for &axis in axes {
        let (old, delta) = advance(b, obj, axis);
        add_flags(c, old, delta);
        c.d[0] = delta;
    }
    rts(c, b);
}

/// 0x1d62 : Y puis X.
fn move_xy(c: &mut Cpu, b: &mut Cps2Bus) {
    ballistic(c, b, &[AXIS_Y, AXIS_X])
}
/// 0x1d78 : X seul.
fn move_x(c: &mut Cpu, b: &mut Cps2Bus) {
    ballistic(c, b, &[AXIS_X])
}
/// 0x1dbc : Z seul.
fn move_z(c: &mut Cpu, b: &mut Cps2Bus) {
    ballistic(c, b, &[AXIS_Z])
}
/// 0x1d90 : Y, X puis Z.
fn move_xyz(c: &mut Cpu, b: &mut Cps2Bus) {
    ballistic(c, b, &[AXIS_Y, AXIS_X, AXIS_Z])
}
/// 0x1da6 : X puis Z.
fn move_xz(c: &mut Cpu, b: &mut Cps2Bus) {
    ballistic(c, b, &[AXIS_X, AXIS_Z])
}
/// 0x1dd4 : Y puis Z.
fn move_yz(c: &mut Cpu, b: &mut Cps2Bus) {
    ballistic(c, b, &[AXIS_Y, AXIS_Z])
}
/// 0x1e02 : Y seul.
fn move_y(c: &mut Cpu, b: &mut Cps2Bus) {
    ballistic(c, b, &[AXIS_Y])
}

macro_rules! straight_native {
    ($const:ident, $entry:expr, $name:expr, $run:expr, $outputs:expr) => {
        pub const $const: Native = Native {
            entry: $entry,
            name: $name,
            cost: |_, b| path_cycles(b, $entry, 0, |_| false),
            run: $run,
            touches_sound: false,
            outputs: $outputs,
            exact_cost: true,
        };
    };
}

straight_native!(MOVE_XY, 0x1d62, "obj_move_xy", move_xy, regs::ALL);
straight_native!(MOVE_X, 0x1d78, "obj_move_x", move_x, regs::ALL);
straight_native!(MOVE_XYZ, 0x1d90, "obj_move_xyz", move_xyz, regs::ALL);
straight_native!(MOVE_XZ, 0x1da6, "obj_move_xz", move_xz, regs::ALL);
straight_native!(MOVE_Z, 0x1dbc, "obj_move_z", move_z, regs::ALL);
straight_native!(MOVE_YZ, 0x1dd4, "obj_move_yz", move_yz, regs::ALL);
straight_native!(MOVE_Y, 0x1e02, "obj_move_y", move_y, regs::ALL);

/// Mouvement par table sur X puis sur Y ou Z. L'original laisse D0 = index << 2, D1 = déplacement
/// du second axe, A4 = table.
fn move_by_table(c: &mut Cpu, b: &mut Cps2Bus, second: Axis) {
    let obj = Obj(c.a[0]);
    c.d[0] = (obj.u8(b, Obj::SPEED_INDEX) as u32) << 2;
    c.a[4] = obj.u32(b, Obj::SPEED_TABLE);
    advance_by_table(b, obj, AXIS_X, 0);
    let (old, delta) = advance_by_table(b, obj, second, 2);
    add_flags(c, old, delta);
    c.d[1] = delta;
    rts(c, b);
}

/// 0x1cb4 : X et Y par table.
fn move_by_table_xy(c: &mut Cpu, b: &mut Cps2Bus) {
    move_by_table(c, b, AXIS_Y)
}

/// 0x1cee : X et Z par table.
fn move_by_table_xz(c: &mut Cpu, b: &mut Cps2Bus) {
    move_by_table(c, b, AXIS_Z)
}

straight_native!(MOVE_BY_TABLE_XY, 0x1cb4, "obj_move_table_xy", move_by_table_xy, regs::ALL);
straight_native!(MOVE_BY_TABLE_XZ, 0x1cee, "obj_move_table_xz", move_by_table_xz, regs::ALL);

/// Lecture d'un long en espace programme (tables incrustées dans le code, lues en relatif au PC).
pub(super) fn prog32(b: &mut Cps2Bus, a: u32) -> u32 {
    (b.read_prog16(a & 0xff_ffff) as u32) << 16 | b.read_prog16(a.wrapping_add(2) & 0xff_ffff) as u32
}

/// Sélection de table de vitesses : D0 = numéro de table, `tables` = tableau de pointeurs dans le
/// code. Laisse D0 = index << 2 et A4 = table.
fn speed_table_native(c: &mut Cpu, b: &mut Cps2Bus, tables: u32) {
    let obj = Obj(c.a[0]);
    let n = lsl_word(c, c.d[0] as u16, 2);
    let table = prog32(b, tables.wrapping_add(n as i16 as i32 as u32));
    select_speed_table(b, obj, table);
    let index = lsl_word(c, obj.u8(b, Obj::SPEED_INDEX) as u16, 2);
    c.d[0] = index as u32;
    c.a[4] = table;
    c.flags_logic(obj.u16(b, Obj::VY) as u32, Size::Word);
    rts(c, b);
}

fn speed_table_a(c: &mut Cpu, b: &mut Cps2Bus) {
    speed_table_native(c, b, 0x1ab4)
}

fn speed_table_b(c: &mut Cpu, b: &mut Cps2Bus) {
    speed_table_native(c, b, 0x1b84)
}

straight_native!(SPEED_TABLE_A, 0x1a94, "obj_select_speed_table_a", speed_table_a, regs::ALL);
straight_native!(SPEED_TABLE_B, 0x1b64, "obj_select_speed_table_b", speed_table_b, regs::ALL);

/// 0x29ea : point d'extension laissé vide par le jeu ; ne fait que tester l'octet `Obj::HIDDEN`
/// (les appelants enchaînent sur `bne`).
fn pause_hook(c: &mut Cpu, b: &mut Cps2Bus) {
    let v = rd8(b, c.a[0] + Obj::HIDDEN);
    c.flags_logic(v as u32, Size::Byte);
    rts(c, b);
}

pub const PAUSE_HOOK: Native = Native {
    entry: 0x29ea,
    name: "obj_pause_hook",
    cost: |_, b| path_cycles(b, 0x29ea, 0, |_| false),
    run: pause_hook,
    touches_sound: false,
    outputs: regs::ALL,
    exact_cost: true,
};

/// 0xf1bfc : compteurs de fin de handler. Un octet négatif en `COUNTER_53` est remis à zéro ; le
/// compte à rebours `COUNTDOWN` baisse et, quand il atteint zéro (ou moins), efface `COUNTDOWN_FLAG`.
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

fn countdowns(c: &mut Cpu, b: &mut Cps2Bus) {
    let obj = Obj(c.a[0]);
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
    tick_counters(b, obj);
    rts(c, b);
}

pub const COUNTDOWNS: Native = Native {
    entry: 0xf1bfc,
    name: "obj_tick_counters",
    cost: |c, b| {
        let obj = Obj(c.a[0]);
        let non_negative = (obj.u8(b, Obj::COUNTER_53) as i8) >= 0;
        let left = obj.u8(b, Obj::COUNTDOWN);
        let path = non_negative as u64 | (left == 0) as u64 * 2 | (left as i8 > 1) as u64 * 4;
        path_cycles(b, 0xf1bfc, path, |pc| match pc {
            0xf1c00 => non_negative,
            0xf1c0c => left == 0,
            _ => left as i8 > 1,
        })
    },
    run: countdowns,
    touches_sound: false,
    outputs: regs::ALL,
    exact_cost: true,
};
