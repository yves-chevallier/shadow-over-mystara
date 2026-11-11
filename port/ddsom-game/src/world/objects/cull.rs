//! On-screen presence of objects: leaving the screen, play areas, registration for collisions.
//!
//! Screen coordinates are words: abscissa = integer part of `Obj::X` minus that of the
//! camera; height = `Z + Y` minus the camera ordinate. The original's tests use
//! unsigned 16-bit arithmetic with an added margin.
//!
//! The native routines reproduce the registers and flags of the original (the callers
//! chain on `bcs`, `bhi`, `beq`...) by applying the same 16-bit operations.

use crate::world::World;

use super::pools::{QUEUE_BOX, QUEUE_ENEMY_BOSS, QUEUE_ESHL, QUEUE_ITEM, QUEUE_PSHL};
use super::Obj;
use ddsom_data::tables::Bounds;

use crate::world::data::data;
use crate::hw::mem::A5;
use crate::world::ram::{camera as camera_position, Word, CAMERA_ALT_ACTIVE};
use crate::regs;

/// Address of the current camera record (x at +8, y at +0xc).
pub fn camera(b: &mut World) -> u32 {
    A5 + if CAMERA_ALT_ACTIVE.is_set(b) { super::ram::ALT_CAMERA } else { super::ram::CAMERA }
}

/// Visibility box `i` (`sprites/tables.json`, `bounds`: dx, width, dy, height), or the words the original reads past
/// the table (`sprites/bounds_beyond`).
pub(crate) fn visibility_box(b: &mut World, i: u8) -> [u16; 4] {
    let t = &data().sprite_tables;
    match t.bounds.get(i as usize) {
        Some(&Bounds(dx, w, dy, h)) => [dx as u16, w, dy as u16, h],
        None => {
            let at = t.bounds_rom + 8 * i as u32;
            std::array::from_fn(|k| crate::world::tables::engine().word(b, at + 2 * k as u32))
        }
    }
}

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
pub fn push_candidate(b: &mut World, obj: Obj, queue: &super::pools::Queue) -> u16 {
    let (count, cursor) = (Word::a5(queue.count), Word::a5(queue.cursor));
    let n = count.get(b).wrapping_add(1);
    count.set(b, n);
    let at = cursor.get(b);
    Word(at as i16 as i32 as u32 & 0xff_ffff).set(b, obj.0 as u16);
    let next = at.wrapping_add(2);
    cursor.set(b, next);
    next
}

/// Screen abscissa (before margin) and screen height of the object, from the camera position `cam`.
fn screen_x(b: &mut World, obj: Obj, cam: (u16, u16)) -> u16 {
    obj.u16(b, Obj::X).wrapping_sub(cam.0)
}
fn screen_height(b: &mut World, obj: Obj, cam: (u16, u16)) -> u16 {
    obj.u16(b, Obj::Z).wrapping_add(obj.u16(b, Obj::Y)).wrapping_sub(cam.1)
}

// ---- 16-bit operations on D0-D2 with the 68000 flags ----

// ---- 0x1e1a: leaving the screen ----

/// An object off screen (margin 0x40) and active goes to state 6 (disappearance).
pub fn despawn_if_offscreen(b: &mut World, obj: Obj) {
    let cam = camera_position(b);
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
pub fn in_arena(b: &mut World, obj: Obj, margin: u16) -> bool {
    let cam = camera_position(b);
    let x = screen_x(b, obj, cam).wrapping_add(margin);
    if x >= 0x180u16.wrapping_add(margin.wrapping_add(margin)) {
        return false;
    }
    let y = obj.u16(b, Obj::Y).wrapping_sub(cam.1).wrapping_add(ARENA_MARGIN);
    y < 0x100 + 2 * ARENA_MARGIN
}

/// The `bhi` following 0x1e92: true if the last compared value strictly exceeds its bound (the abscissa, when it
/// leaves the area in width; otherwise the depth). False inside the area, and also exactly on the edge in width.
pub fn beyond_arena(b: &mut World, obj: Obj, margin: u16) -> bool {
    let cam = camera_position(b);
    let x = screen_x(b, obj, cam).wrapping_add(margin);
    let edge_x = 0x180u16.wrapping_add(margin.wrapping_add(margin));
    if x >= edge_x {
        return x > edge_x;
    }
    let y = obj.u16(b, Obj::Y).wrapping_sub(cam.1).wrapping_add(ARENA_MARGIN);
    y > 0x100 + 2 * ARENA_MARGIN
}

// ---- 0x1ed2: visibility and registration for collisions ----

/// Visibility of an object: scenery (class 0) within the screen with a margin of 0x60 pixels, the others
/// within their visibility box.
pub fn is_visible(b: &mut World, obj: Obj) -> bool {
    let cam = camera_position(b);
    let class = obj.u8(b, Obj::CLASS);
    if class == 0 {
        screen_x(b, obj, cam).wrapping_add(0x60) <= 0x240 && screen_height(b, obj, cam).wrapping_add(0xc0) <= 0x1c0
    } else {
        let i = obj.bounds(b);
        let [dx, width, dy, height] = visibility_box(b, i);
        let dx = screen_x(b, obj, cam).wrapping_add(dx);
        let dy = screen_height(b, obj, cam).wrapping_add(dy);
        dx <= width && dy <= height
    }
}

/// Updates bit 0 of `Obj::FLAGS` (visible this frame) and registers the visible object in the
/// collision candidate queue of its class. Returns true if the object is visible.
pub fn update_visibility(b: &mut World, obj: Obj) -> bool {
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
