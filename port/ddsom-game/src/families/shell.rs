//! Common parts of the ESHL class handlers (enemy shots and attacks): fields of an ESHL object, states, placement
//! helpers and terrain collision helpers.

use crate::families::cx::Bench;
use super::cx::Cx;
use crate::objects::Obj;
use crate::objects::Current;

/// Fields specific to ESHL objects.
pub mod f {
    /// Orientation (non-zero: flipped), copied from the owner at creation.
    pub const FLIP: u32 = 0x2c;
    /// Collision box set.
    pub const BOX_SET: u32 = 0x2e;
    /// Attack power (`+0x44`: attack identifier, `+0x45`: power).
    pub const ATTACK_ID: u32 = 0x44;
    pub const ATTACK_POWER: u32 = 0x45;
    /// Owner (16-bit RAM address).
    pub const OWNER: u32 = 0x50;
    /// Flags set by the collision engine (bit 3: hit, bit 5: parried).
    pub const HIT_FLAGS: u32 = 0x57;
    /// Per-type countdown.
    pub const TIMER: u32 = 0x5c;
}

/// Common main states (`+4`).
pub mod st {
    pub const FREE: u8 = 6;
}

/// Blinking: the object is hidden every other frame (parity of `+0x5c` added to the dispatch loop counter D7).
pub fn blink<R: Bench>(x: &mut Cx<'_, R>) {
    let visible = x.u16(f::TIMER).wrapping_add(x.counter()) & 1 != 0;
    x.set_bit(0x1, 1, !visible);
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
    x.add16(0x10, 0xffff);
    let side = if owner.u8(x.b, f::FLIP) != 0 { dx } else { -dx };
    x.add16(0x8, side as u16);
    x.add16(0xc, dz);
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
