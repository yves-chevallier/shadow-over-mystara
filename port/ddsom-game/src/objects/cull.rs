//! On-screen presence of objects: leaving the screen, play areas, registration for collisions.
//!
//! Screen coordinates are words: abscissa = integer part of `Obj::X` minus that of the
//! camera; height = `Z + Y` minus the camera ordinate. The original's tests use
//! unsigned 16-bit arithmetic with an added margin.
//!
//! The native routines reproduce the registers and flags of the original (the callers
//! chain on `bcs`, `bhi`, `beq`...) by applying the same 16-bit operations.

use cps2::Cps2Bus;

use super::pools::{QUEUE_BOX, QUEUE_ENEMY_BOSS, QUEUE_ESHL, QUEUE_ITEM, QUEUE_PSHL};
use super::Obj;
use crate::mem::{rd16, rd8, wr16, A5};
use crate::regs;

/// Address of the current camera (x at +8, y at +0xc).
pub fn camera(b: &mut Cps2Bus) -> u32 {
    let alt = rd8(b, A5 + super::ram::USE_ALT_CAMERA) != 0;
    A5 + if alt { super::ram::ALT_CAMERA } else { super::ram::CAMERA }
}

/// Visibility boxes by index (`BOUNDS_INDEX`): (dx, width, dy, height), 8 bytes each.
pub(crate) const BOUNDS_TABLE: u32 = 0x1ca538;
/// Object byte designating its visibility box (high byte of `FRAME_DATA_B`).
pub(crate) const BOUNDS_INDEX: u32 = 0x3c;

/// Collision candidate queue for the object class: player shots, enemies and bosses, enemy
/// shots, crates, pickable items.
pub(crate) fn queue_of(class: u8) -> Option<&'static super::pools::Queue> {
    match class {
        2 | 4 => Some(&QUEUE_ENEMY_BOSS),
        6 => Some(&QUEUE_PSHL),
        8 => Some(&QUEUE_ESHL),
        0xa => Some(&QUEUE_BOX),
        0xc => Some(&QUEUE_ITEM),
        _ => None,
    }
}

/// Adds the object to the candidate queue; returns the new write pointer.
pub fn push_candidate(b: &mut Cps2Bus, obj: Obj, queue: &super::pools::Queue) -> u16 {
    let n = rd16(b, A5 + queue.count).wrapping_add(1);
    wr16(b, A5 + queue.count, n);
    let at = rd16(b, A5 + queue.cursor);
    wr16(b, at as i16 as i32 as u32, obj.0 as u16);
    let next = at.wrapping_add(2);
    wr16(b, A5 + queue.cursor, next);
    next
}

/// Screen abscissa (before margin) and screen height of the object.
fn screen_x(b: &mut Cps2Bus, obj: Obj, cam: u32) -> u16 {
    obj.u16(b, Obj::X).wrapping_sub(rd16(b, cam + 8))
}
fn screen_height(b: &mut Cps2Bus, obj: Obj, cam: u32) -> u16 {
    obj.u16(b, Obj::Z).wrapping_add(obj.u16(b, Obj::Y)).wrapping_sub(rd16(b, cam + 0xc))
}

// ---- 16-bit operations on D0-D2 with the 68000 flags ----

// ---- 0x1e1a: leaving the screen ----

/// An object off screen (margin 0x40) and active goes to state 6 (disappearance).
pub fn despawn_if_offscreen(b: &mut Cps2Bus, obj: Obj) {
    let cam = camera(b);
    let x_out = screen_x(b, obj, cam).wrapping_add(0x40) >= 0x200;
    let y_out = screen_height(b, obj, cam).wrapping_add(0x40) > 0x180;
    if (x_out || y_out) && obj.is_active(b) {
        obj.set_u8(b, Obj::STATE, 6);
    }
}

/// Registers left behind by the original routines (see `diag::dead_regs`, label `cull`).
pub const REGS_DESPAWN: u32 = regs::d(0) | regs::a(1) | regs::CCR;
pub const REGS_IN_ARENA: u32 = regs::d(0) | regs::d(1) | regs::d(2) | regs::a(1) | regs::CCR;
pub const REGS_VISIBILITY: u32 = regs::d(0) | regs::d(1) | regs::d(2) | regs::a(1) | regs::a(2) | regs::CCR;

// ---- 0x1e92: widened play area ----

/// Margin of the default play area (0x1e90); it is also the fixed margin of the height.
pub const ARENA_MARGIN: u16 = 0x3e;

/// 0x1e92: is the object within the play area widened by `margin` pixels? In width, the screen abscissa
/// plus the margin must stay below 0x180 + 2 × margin; in depth (`Obj::Y` alone, without altitude), with the
/// fixed margin `ARENA_MARGIN`, below 0x100 + 2 × 0x3e. Unsigned 16-bit comparisons, like the original.
pub fn in_arena(b: &mut Cps2Bus, obj: Obj, margin: u16) -> bool {
    let cam = camera(b);
    let x = screen_x(b, obj, cam).wrapping_add(margin);
    if x >= 0x180u16.wrapping_add(margin.wrapping_add(margin)) {
        return false;
    }
    let y = obj.u16(b, Obj::Y).wrapping_sub(rd16(b, cam + 0xc)).wrapping_add(ARENA_MARGIN);
    y < 0x100 + 2 * ARENA_MARGIN
}

/// The `bhi` following 0x1e92: true if the last compared value strictly exceeds its bound (the abscissa, when it
/// leaves the area in width; otherwise the depth). False inside the area, and also exactly on the edge in width.
pub fn beyond_arena(b: &mut Cps2Bus, obj: Obj, margin: u16) -> bool {
    let cam = camera(b);
    let x = screen_x(b, obj, cam).wrapping_add(margin);
    let edge_x = 0x180u16.wrapping_add(margin.wrapping_add(margin));
    if x >= edge_x {
        return x > edge_x;
    }
    let y = obj.u16(b, Obj::Y).wrapping_sub(rd16(b, cam + 0xc)).wrapping_add(ARENA_MARGIN);
    y > 0x100 + 2 * ARENA_MARGIN
}

// ---- 0x1ed2: visibility and registration for collisions ----

/// Visibility of an object: scenery (class 0) within the screen with a margin of 0x60 pixels, the others
/// within their visibility box.
pub fn is_visible(b: &mut Cps2Bus, obj: Obj) -> bool {
    let cam = camera(b);
    let class = obj.u8(b, Obj::CLASS);
    if class == 0 {
        screen_x(b, obj, cam).wrapping_add(0x60) <= 0x240 && screen_height(b, obj, cam).wrapping_add(0xc0) <= 0x1c0
    } else {
        let t = BOUNDS_TABLE + 8 * obj.u8(b, BOUNDS_INDEX) as u32;
        let dx = screen_x(b, obj, cam).wrapping_add(rd16(b, t));
        let dy = screen_height(b, obj, cam).wrapping_add(rd16(b, t + 4));
        dx <= rd16(b, t + 2) && dy <= rd16(b, t + 6)
    }
}

/// Updates bit 0 of `Obj::FLAGS` (visible this frame) and registers the visible object in the
/// collision candidate queue of its class. Returns true if the object is visible.
pub fn update_visibility(b: &mut Cps2Bus, obj: Obj) -> bool {
    let flags = obj.u8(b, Obj::FLAGS);
    if !is_visible(b, obj) || flags & 2 != 0 {
        obj.set_u8(b, Obj::FLAGS, flags & !1);
        return false;
    }
    obj.set_u8(b, Obj::FLAGS, flags | 1);
    if let Some(q) = queue_of(obj.u8(b, Obj::CLASS)) {
        push_candidate(b, obj, q);
    }
    true
}
