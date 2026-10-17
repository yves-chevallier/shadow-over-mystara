//! Déplacements des joueurs : vitesses de marche, intégration du mouvement, limites de l'écran.

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::{Cpu, SR_X};

use super::{ram, Ctx, Player};
use crate::chemin::Chemin;
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

/// Ce que la mise à l'écran du joueur va changer : nouvelle profondeur, nouvelle abscisse et bord touché.
struct Plan {
    y: Option<i16>,
    x: Option<(i16, u8)>,
    /// Décisions des branchements de l'original, dans l'ordre.
    path: Chemin,
}

/// Décide, sans rien modifier, comment ramener le joueur dans la zone visible : la profondeur reste
/// entre camera.y + 0x28 et camera.y + 0xa4, l'abscisse entre camera.x + 0x20 et camera.x + 0x160.
fn plan_clamp(b: &mut Cps2Bus, p: Player) -> Plan {
    let cam = camera(b);
    let (cam_x, cam_y) = (rd16(b, cam + 8) as i16, rd16(b, cam + 0xc) as i16);
    let (x, y) = (rd16(b, p.at(0x08)) as i16, rd16(b, p.at(0x10)) as i16);
    let mut path = Chemin::default();
    path.pris(rd8(b, A5 + ram::USE_ALT_CAMERA) == 0);
    let top = cam_y.wrapping_add(0xa4);
    path.pris(top >= y);
    let mut new_y = None;
    if top < y {
        new_y = Some(top);
        path.pris(true);
    } else {
        path.pris(false);
        let bottom = cam_y.wrapping_add(0x28);
        path.pris(bottom <= y);
        if bottom > y {
            new_y = Some(bottom);
        }
    }
    let right = cam_x.wrapping_add(0x160);
    path.pris(right >= x);
    let mut new_x = None;
    if right < x {
        new_x = Some((right, 2));
        path.pris(true);
    } else {
        path.pris(false);
        let left = cam_x.wrapping_add(0x20);
        path.pris(left <= x);
        if left > x {
            new_x = Some((left, 1));
        }
    }
    Plan { y: new_y, x: new_x, path }
}

fn apply_clamp(b: &mut Cps2Bus, p: Player, plan: &Plan) {
    if let Some(y) = plan.y {
        wr16(b, p.at(0x10), y as u16);
    }
    if let Some((x, edge)) = plan.x {
        wr8(b, p.at(EDGE_HIT), edge);
        wr16(b, p.at(0x08), x as u16);
    }
}

/// 0x35c0 : maintient le joueur dans la zone visible. Renvoie vrai si l'abscisse a été corrigée.
pub fn clamp_to_screen(b: &mut Cps2Bus, p: Player) -> bool {
    let plan = plan_clamp(b, p);
    apply_clamp(b, p, &plan);
    plan.x.is_some()
}

fn clamp_native(c: &mut Cpu, b: &mut Cps2Bus) {
    let cam = camera(b);
    let cam_x = rd16(b, cam + 8) as u32;
    let plan = plan_clamp(b, Player(c.a[0]));
    apply_clamp(b, Player(c.a[0]), &plan);
    c.a[1] = cam;
    c.d[0] = plan.x.is_some() as u32;
    // X : retenue de la dernière addition de l'original (bord droit 0x160, sinon bord gauche 0x20)
    let last = if matches!(plan.x, Some((_, 2))) { 0x160 } else { 0x20 };
    let carry = cam_x + last > 0xffff;
    c.flags_logic(c.d[0], Size::Long);
    c.sr = c.sr & !SR_X | if carry { SR_X } else { 0 };
    rts(c, b);
}

pub const CLAMP: Native = Native {
    entry: 0x35c0,
    name: "player_clamp_to_screen",
    cost: |c, b| plan_clamp(b, Player(c.a[0])).path.duree(b, 0x35c0),
    run: clamp_native,
    touches_sound: false,
    outputs: regs::ALL,
    exact_cost: true,
};
