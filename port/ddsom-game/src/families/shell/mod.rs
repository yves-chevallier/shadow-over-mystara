//! Common parts of the ESHL class handlers (enemy shots and attacks): fields of an ESHL object, states, placement
//! helpers and terrain collision helpers. Also the tables ([`tables`]) of the ESHL, SOBJ, ITEM and BOX families.

pub mod records;
pub mod tables;
pub mod vars;

pub use records::{Base, Shot};

use crate::families::cx::Bench;
use super::cx::Cx;
use crate::objects::Obj;
use crate::objects::Current;

/// Common main states (`+4`).
pub mod st {
    pub const FREE: u8 = 6;
}

/// Blinking: the object is hidden every other frame (parity of `+0x5c` added to the dispatch loop counter D7).
pub fn blink<R: Bench>(x: &mut Cx<'_, R>) {
    let visible = x.u16(Shot::TIMER).wrapping_add(x.counter()) & 1 != 0;
    x.set_bit(Obj::FLAGS, 1, !visible);
}

/// Copies the owner's position (X, Z, Y: integer parts) into the shot.
pub fn copy_position<R: Bench>(x: &mut Cx<'_, R>, owner: Obj) {
    for off in [0x8, 0xc, 0x10] {
        let v = owner.u16(x.b, off);
        x.set16(off, v);
    }
}

/// Places the shot in front of the owner: its position, one pixel closer (depth - 1), then offset by `dx` toward the
/// side it faces and by `dz` in height.
pub fn place_in_front<R: Bench>(x: &mut Cx<'_, R>, owner: Obj, dx: i16, dz: u16) {
    copy_position(x, owner);
    x.add16(Obj::Y, 0xffff);
    let side = if owner.u8(x.b, Base::FLIP) != 0 { dx } else { -dx };
    x.add16(Obj::X, side as u16);
    x.add16(Obj::Z, dz);
}

/// Game pause: true when the object is suspended (byte `+0x40`).
pub fn suspended<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    x.suspended()
}

/// Terrain under the shot: true when the shot must stop (terrains 5 to 8 without bit 6 of the attribute).
pub fn blocked_by_terrain<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let t = crate::video::terrain_of(x.b, x.o);
    let terrain = t.number as u32;
    (terrain & 0x3f).wrapping_sub(5) as u16 <= 3 && terrain & 0x40 == 0
}

/// Variable `A5+0x3d`: game paused (shots do not move).
pub const GAME_PAUSED: u32 = 0x3d;

/// The camera the players see: the main one (`A5+0x506`), or the alternate one (`A5+0x556`) when `A5+0x36` is set.
/// Its x and y words.
pub fn camera(b: &mut cps2::Cps2Bus) -> (crate::ram::Word, crate::ram::Word) {
    use crate::ram;
    if ram::CAMERA_ALT_ACTIVE.get(b) != 0 {
        (ram::CAMERA_ALT_X, ram::CAMERA_ALT_Y)
    } else {
        (ram::CAMERA_X, ram::CAMERA_Y)
    }
}
