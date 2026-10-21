//! Déplacements des joueurs : vitesses de marche, intégration du mouvement, limites de l'écran.

use cps2::Cps2Bus;
use m68k::Cpu;

use super::{ram, Ctx, Player};
use crate::mem::{rd16, rd8, rts, wr16, wr8, A5};
use crate::objects::motion::{advance, AXIS_X, AXIS_Y, AXIS_Z};
use crate::objects::Obj;
use crate::{regs, Native};

/// Tables de vitesses (ROM) : un pointeur par personnage vers 16 couples (vx, vy) 8.8, un par
/// combinaison de directions (bits 0..3 de `KEYS_HELD`).
pub mod speeds {
    /// Marche lente forcée (1 pixel par image).
    pub const SLOW: u32 = 0x10_493e;
    /// Marche lente (monture, boue...) (h).
    pub const SLOW_B: u32 = 0x10_4ffe;
    /// Marche normale (2 pixels par image).
    pub const NORMAL: u32 = 0x10_45de;
    /// Marche rapide (3 pixels par image).
    pub const FAST: u32 = 0x10_4c9e;
    /// Course (4,5 pixels par image).
    pub const RUN: u32 = 0x10_4aee;
    /// Marche accroupie (0,2 pixel par image).
    pub const CRAWL: u32 = 0x10_4e4e;
}

/// 0x392c4 : vitesse (vx, vy) lue dans la table du personnage selon les directions maintenues ;
/// accélérations remises à zéro.
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

/// 0x39260 : vitesse de marche, selon les drapeaux de ralentissement et la classe de vitesse.
pub fn walk_speed(g: &mut Ctx, p: Player) {
    cov!();
    let tables = if p.u8(g, Player::SLOW) != 0 || g.a5u8(ram::SLOW_MODE) != 0 {
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

/// 0x3929e : vitesse de course.
pub fn run_speed(g: &mut Ctx, p: Player) {
    cov!();
    set_speed(g, p, speeds::RUN);
}

/// 0x392b2 : vitesse accroupie.
pub fn crawl_speed(g: &mut Ctx, p: Player) {
    cov!();
    set_speed(g, p, speeds::CRAWL);
}

/// Mouvement balistique de l'objet sur Y puis X (0x1d62).
pub fn move_xy(g: &mut Ctx, p: Player) {
    cov!();
    advance(g.b, p.obj(), AXIS_Y);
    advance(g.b, p.obj(), AXIS_X);
}

/// Sur X seulement (0x1d78).
pub fn move_x(g: &mut Ctx, p: Player) {
    cov!();
    advance(g.b, p.obj(), AXIS_X);
}

/// Sur Y, X puis Z (0x1d90).
pub fn move_xyz(g: &mut Ctx, p: Player) {
    cov!();
    advance(g.b, p.obj(), AXIS_Y);
    advance(g.b, p.obj(), AXIS_X);
    advance(g.b, p.obj(), AXIS_Z);
}

/// Sur X puis Z (0x1da6).
pub fn move_xz(g: &mut Ctx, p: Player) {
    cov!();
    advance(g.b, p.obj(), AXIS_X);
    advance(g.b, p.obj(), AXIS_Z);
}

/// Sur Z seulement (0x1dbc).
pub fn move_z(g: &mut Ctx, p: Player) {
    cov!();
    advance(g.b, p.obj(), AXIS_Z);
}

/// Caméra qui cadre les joueurs : adresse de sa structure (position x en +8, y en +0xc).
fn camera(b: &mut Cps2Bus) -> u32 {
    A5 + if rd8(b, A5 + ram::USE_ALT_CAMERA) != 0 { 0x54e } else { 0x4fe }
}

/// Dernier bord touché : octet `+0x5a` (1 gauche, 2 droite).
const EDGE_HIT: u32 = 0x5a;

/// Maintient le joueur dans la zone visible : la profondeur reste entre camera.y + 0x28 et
/// camera.y + 0xa4, l'abscisse entre camera.x + 0x20 et camera.x + 0x160 (le bord touché est
/// mémorisé). Renvoie vrai si l'abscisse a été corrigée.
pub fn clamp_to_screen(b: &mut Cps2Bus, p: Player) -> bool {
    cov!();
    let cam = camera(b);
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

/// 0x35dc : seulement l'abscisse (entre camera.x + 0x20 et camera.x + 0x160). Vrai si corrigée.
pub fn clamp_x(b: &mut Cps2Bus, p: Player) -> bool {
    cov!();
    let cam = camera(b);
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

/// Entrée 68000 (0x35c0) : D0 = 1 si l'abscisse a été corrigée, A1 = caméra.
fn clamp_native(c: &mut Cpu, b: &mut Cps2Bus) {
    let cam = camera(b);
    c.d[0] = clamp_to_screen(b, Player(c.a[0])) as u32;
    c.a[1] = cam;
    rts(c, b);
}

pub const CLAMP: Native = Native {
    entry: 0x35c0,
    name: "player_clamp_to_screen",
    cost: |_, _| 200,
    run: clamp_native,
    touches_sound: false,
    outputs: regs::d(0) | regs::a(1),
    exact_cost: false,
};
