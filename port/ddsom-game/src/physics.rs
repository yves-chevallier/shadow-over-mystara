//! Physique des objets : intégration accélération -> vitesse -> position.
//!
//! Unités : vitesses et accélérations en 8.8 (pixels/image), positions en 16.16 (pixels).

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::Cpu;

use crate::mem::{rd16, rd32, rts, wr16, wr32};
use crate::timing::path_cycles;
use crate::Native;

/// Enregistrement d'un objet du jeu (joueur, ennemi, projectile...), désigné par son adresse (A0).
#[derive(Clone, Copy)]
pub struct Obj(pub u32);

impl Obj {
    pub const X: u32 = 0x08;
    pub const Y: u32 = 0x10;
    pub const VX: u32 = 0x14;
    pub const AX: u32 = 0x16;
    pub const VY: u32 = 0x1c;
    pub const AY: u32 = 0x1e;

    fn at(self, off: u32) -> u32 {
        self.0.wrapping_add(off)
    }
}

/// Un axe du mouvement : (vitesse, accélération, position).
#[derive(Clone, Copy)]
struct Axis {
    vel: u32,
    acc: u32,
    pos: u32,
}

const AXIS_X: Axis = Axis { vel: Obj::VX, acc: Obj::AX, pos: Obj::X };
const AXIS_Y: Axis = Axis { vel: Obj::VY, acc: Obj::AY, pos: Obj::Y };

/// vitesse += accélération ; position += vitesse.
/// Renvoie le déplacement appliqué (vitesse convertie en 16.16), que l'original laisse dans D0.
fn integrate(c: &mut Cpu, b: &mut Cps2Bus, obj: Obj, axis: Axis) -> u32 {
    let vel = rd16(b, obj.at(axis.vel)).wrapping_add(rd16(b, obj.at(axis.acc)));
    wr16(b, obj.at(axis.vel), vel);
    // 8.8 -> 16.16 : la vitesse signée est décalée de 8 bits
    let delta = ((vel as i16 as i32) << 8) as u32;
    let pos = rd32(b, obj.at(axis.pos));
    // add.l : position et drapeaux exactement comme le 68000
    let new_pos = c.addsub(false, delta, pos, false, Size::Long, false);
    wr32(b, obj.at(axis.pos), new_pos);
    delta
}

/// 0x1d62 : déplace l'objet A0 selon Y puis X.
fn obj_move_xy(c: &mut Cpu, b: &mut Cps2Bus) {
    let obj = Obj(c.a[0]);
    integrate(c, b, obj, AXIS_Y);
    c.d[0] = integrate(c, b, obj, AXIS_X);
    rts(c, b);
}

/// 0x1d78 : déplace l'objet A0 selon X seulement.
fn obj_move_x(c: &mut Cpu, b: &mut Cps2Bus) {
    let obj = Obj(c.a[0]);
    c.d[0] = integrate(c, b, obj, AXIS_X);
    rts(c, b);
}

pub const OBJ_MOVE_XY: Native = Native {
    entry: 0x1d62,
    name: "obj_move_xy",
    cost: |_, b| path_cycles(b, 0x1d62, 0, |_| false),
    run: obj_move_xy,
    touches_sound: false,
};

pub const OBJ_MOVE_X: Native = Native {
    entry: 0x1d78,
    name: "obj_move_x",
    cost: |_, b| path_cycles(b, 0x1d78, 0, |_| false),
    run: obj_move_x,
    touches_sound: false,
};
