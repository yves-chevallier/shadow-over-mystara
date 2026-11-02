//! Player movement: walking speeds, motion integration, screen limits.

use cps2::Cps2Bus;

use super::{ram, Ctx, Player};
use crate::mem::{rd16, wr16, wr8};
use crate::objects::motion::{advance, AXIS_X, AXIS_Y, AXIS_Z};
use crate::objects::Obj;
use crate::machine::Machine;

/// Speed tables (ROM): one pointer per character to 16 pairs (vx, vy) 8.8, one per
/// combination of directions (bits 0..3 of `KEYS_HELD`).
pub mod speeds {
    /// Forced slow walk (1 pixel per frame).
    pub const SLOW: u32 = 0x10_493e;
    /// Slow walk (mount, mud...) (h).
    pub const SLOW_B: u32 = 0x10_4ffe;
    /// Normal walk (2 pixels per frame).
    pub const NORMAL: u32 = 0x10_45de;
    /// Fast walk (3 pixels per frame).
    pub const FAST: u32 = 0x10_4c9e;
    /// Run (4.5 pixels per frame).
    pub const RUN: u32 = 0x10_4aee;
    /// Crouched walk (0.2 pixel per frame).
    pub const CRAWL: u32 = 0x10_4e4e;
}

/// 0x392c4: speed (vx, vy) read from the character's table according to the held directions;
/// accelerations reset to zero.
pub fn set_speed(g: &mut Ctx, p: Player, tables: u32) {
    cov!();
    let character = p.u8(g, Player::CHARACTER) as u32;
    let table = g.data32(tables + 4 * character);
    let dir = (p.u8(g, Player::KEYS_HELD) & 0xf) as u32;
    let (vx, vy) = (g.data16(table + 4 * dir), g.data16(table + 4 * dir + 2));
    p.set_u16(g, Obj::VX, vx);
    p.set_u16(g, Obj::VY, vy);
    p.set_u16(g, Obj::AX, 0);
    p.set_u16(g, Obj::AY, 0);
}

/// 0x39260: walking speed, according to the slowdown flags and the speed class.
pub fn walk_speed(g: &mut Ctx, p: Player) {
    cov!();
    let tables = if p.u8(g, Player::SLOW) != 0 || ram::SLOW_MODE.get(g.b) != 0 {
        speeds::SLOW
    } else if p.u8(g, Player::SLOW_8A) != 0 {
        speeds::SLOW_B
    } else if p.u8(g, Player::SPEED_CLASS) != 0 {
        speeds::NORMAL
    } else {
        speeds::FAST
    };
    set_speed(g, p, tables);
}

/// 0x3929e: running speed.
pub fn run_speed(g: &mut Ctx, p: Player) {
    cov!();
    set_speed(g, p, speeds::RUN);
}

/// 0x392b2: crouched speed.
pub fn crawl_speed(g: &mut Ctx, p: Player) {
    cov!();
    set_speed(g, p, speeds::CRAWL);
}

/// Ballistic motion of the object on Y then X (0x1d62).
pub fn move_xy(g: &mut Ctx, p: Player) {
    cov!();
    advance(g.b, p.obj(), AXIS_Y);
    advance(g.b, p.obj(), AXIS_X);
}

/// On X only (0x1d78).
pub fn move_x(g: &mut Ctx, p: Player) {
    cov!();
    advance(g.b, p.obj(), AXIS_X);
}

/// On Y, X then Z (0x1d90).
pub fn move_xyz(g: &mut Ctx, p: Player) {
    cov!();
    advance(g.b, p.obj(), AXIS_Y);
    advance(g.b, p.obj(), AXIS_X);
    advance(g.b, p.obj(), AXIS_Z);
}

/// On X then Z (0x1da6).
pub fn move_xz(g: &mut Ctx, p: Player) {
    cov!();
    advance(g.b, p.obj(), AXIS_X);
    advance(g.b, p.obj(), AXIS_Z);
}

/// On Z only (0x1dbc).
pub fn move_z(g: &mut Ctx, p: Player) {
    cov!();
    advance(g.b, p.obj(), AXIS_Z);
}

/// Last edge touched: byte `+0x5a` (1 left, 2 right).
const EDGE_HIT: u32 = 0x5a;

/// Keeps the player within the visible area: depth stays between camera.y + 0x28 and
/// camera.y + 0xa4, abscissa between camera.x + 0x20 and camera.x + 0x160 (the touched edge is
/// remembered). Returns true if the abscissa was corrected.
pub fn clamp_to_screen(b: &mut Cps2Bus, p: Player) -> bool {
    cov!();
    let cam = crate::objects::cull::camera(b);
    let cam_y = rd16(b, cam + 0xc) as i16;
    let y = rd16(b, p.at(0x10)) as i16;
    let (top, bottom) = (cam_y.wrapping_add(0xa4), cam_y.wrapping_add(0x28));
    if y > top {
        wr16(b, p.at(0x10), top as u16);
    } else if y < bottom {
        wr16(b, p.at(0x10), bottom as u16);
    }
    clamp_x(b, p)
}

/// 0x35dc: only the abscissa (between camera.x + 0x20 and camera.x + 0x160). True if corrected.
pub fn clamp_x(b: &mut Cps2Bus, p: Player) -> bool {
    cov!();
    let cam = crate::objects::cull::camera(b);
    let cam_x = rd16(b, cam + 8) as i16;
    let x = rd16(b, p.at(0x08)) as i16;
    let (right, left) = (cam_x.wrapping_add(0x160), cam_x.wrapping_add(0x20));
    let edge = if x > right {
        Some((right, 2))
    } else if x < left {
        Some((left, 1))
    } else {
        None
    };
    if let Some((nx, side)) = edge {
        wr8(b, p.at(EDGE_HIT), side);
        wr16(b, p.at(0x08), nx as u16);
    }
    edge.is_some()
}
