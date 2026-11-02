//! On-screen presence of objects: leaving the screen, play areas, registration for collisions.
//!
//! Screen coordinates are words: abscissa = integer part of `Obj::X` minus that of the
//! camera; height = `Z + Y` minus the camera ordinate. The original's tests use
//! unsigned 16-bit arithmetic with an added margin.
//!
//! The native routines reproduce the registers and flags of the original (the callers
//! chain on `bcs`, `bhi`, `beq`...) by applying the same 16-bit operations.

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::Cpu;

use super::motion::{cmp, lsl_word};
use super::pools::{QUEUE_BOX, QUEUE_ENEMY_BOSS, QUEUE_ESHL, QUEUE_ITEM, QUEUE_PSHL};
use super::Obj;
use crate::mem::{rd16, rd8, rts, set_low_byte, wr16, A5};
use crate::{regs, Native};

/// Address of the current camera (x at +8, y at +0xc).
pub fn camera(b: &mut Cps2Bus) -> u32 {
    let alt = rd8(b, A5 + super::ram::USE_ALT_CAMERA) != 0;
    A5 + if alt { super::ram::ALT_CAMERA } else { super::ram::CAMERA }
}

/// Visibility boxes by index (`BOUNDS_INDEX`): (dx, width, dy, height), 8 bytes each.
const BOUNDS_TABLE: u32 = 0x1ca538;
/// Object byte designating its visibility box (high byte of `FRAME_DATA_B`).
const BOUNDS_INDEX: u32 = 0x3c;

/// Collision candidate queue for the object class: player shots, enemies and bosses, enemy
/// shots, crates, pickable items.
fn queue_of(class: u8) -> Option<&'static super::pools::Queue> {
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

fn dw(c: &Cpu, n: usize) -> u16 {
    c.d[n] as u16
}
fn set_dw(c: &mut Cpu, n: usize, v: u16) {
    c.d[n] = (c.d[n] & 0xffff_0000) | v as u32;
}
/// `move.w v,Dn`
fn move_w(c: &mut Cpu, n: usize, v: u16) {
    set_dw(c, n, v);
    c.flags_logic(v as u32, Size::Word);
}
/// `add.w src,Dn`
fn add_w(c: &mut Cpu, n: usize, src: u16) {
    let r = c.addsub(false, src as u32, dw(c, n) as u32, false, Size::Word, false) as u16;
    set_dw(c, n, r);
}
/// `sub.w src,Dn`
fn sub_w(c: &mut Cpu, n: usize, src: u16) {
    let r = c.addsub(true, src as u32, dw(c, n) as u32, false, Size::Word, false) as u16;
    set_dw(c, n, r);
}
/// `cmp.w src,Dn`
fn cmp_w(c: &mut Cpu, n: usize, src: u16) {
    cmp(c, src as u32, dw(c, n) as u32, Size::Word);
}
/// D`n`.w = screen abscissa; `move.w X,Dn ; sub.w cam.x,Dn`.
fn load_screen_x(c: &mut Cpu, b: &mut Cps2Bus, n: usize, obj: Obj, cam: u32) {
    move_w(c, n, obj.u16(b, Obj::X));
    sub_w(c, n, rd16(b, cam + 8));
}
/// D`n`.w = screen height; `move.w Z,Dn ; add.w Y,Dn ; sub.w cam.y,Dn`.
fn load_screen_height(c: &mut Cpu, b: &mut Cps2Bus, n: usize, obj: Obj, cam: u32) {
    move_w(c, n, obj.u16(b, Obj::Z));
    add_w(c, n, obj.u16(b, Obj::Y));
    sub_w(c, n, rd16(b, cam + 0xc));
}

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

/// `despawn_if_offscreen` with the original's registers (A1 = camera, D0 = last test).
#[track_caller]
pub fn despawn_if_offscreen_68k(c: &mut Cpu, b: &mut Cps2Bus, obj: Obj) {
    cull_offscreen(c, b, obj);
    crate::diag::dead_regs(c, "cull", REGS_DESPAWN);
}

/// 0x1e1a, registers included.
fn cull_offscreen(c: &mut Cpu, b: &mut Cps2Bus, obj: Obj) {
    let cam = camera(b);
    c.a[1] = cam;
    load_screen_x(c, b, 0, obj, cam);
    add_w(c, 0, 0x40);
    cmp_w(c, 0, 0x200);
    let mut off = c.cond(4 /* cc */);
    if !off {
        load_screen_height(c, b, 0, obj, cam);
        add_w(c, 0, 0x40);
        cmp_w(c, 0, 0x180);
        off = c.cond(2 /* hi */);
    }
    if off {
        let active = obj.u8(b, Obj::ACTIVE);
        c.flags_logic(active as u32, Size::Byte);
        if active != 0 {
            obj.set_u8(b, Obj::STATE, 6);
            c.flags_logic(6, Size::Byte);
        }
    }
}

pub const CULL_OFFSCREEN: Native = Native {
    entry: 0x1e1a,
    name: "obj_cull_offscreen",
    run: crate::Body::Processeur(|c, b| {
        cull_offscreen(c, b, Obj(c.a[0]));
        rts(c, b);
    }),
    touches_sound: false,
    outputs: regs::ALL,
};

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

/// `in_arena` with the original's registers: D0 = margin (long), then D0-D2 and A1 (camera) of the computations,
/// carry = inside.
#[track_caller]
pub fn in_arena_68k(c: &mut Cpu, b: &mut Cps2Bus, obj: Obj, margin: u16) -> bool {
    c.d[0] = margin as u32;
    c.flags_logic(margin as u32, Size::Long);
    in_arena_regs(c, b, obj);
    let inside = c.cond(5 /* cs */);
    crate::diag::dead_regs(c, "cull", REGS_IN_ARENA);
    inside
}

/// 0x1e92, registers included: margin in D0.w.
fn in_arena_regs(c: &mut Cpu, b: &mut Cps2Bus, obj: Obj) {
    let cam = camera(b);
    c.a[1] = cam;
    load_screen_x(c, b, 1, obj, cam);
    add_w(c, 1, dw(c, 0));
    move_w(c, 2, 0x180);
    add_w(c, 0, dw(c, 0));
    add_w(c, 2, dw(c, 0));
    cmp_w(c, 1, dw(c, 2));
    if !c.cond(4 /* cc */) {
        c.d[0] = 0x3e;
        c.flags_logic(0x3e, Size::Long);
        move_w(c, 1, obj.u16(b, Obj::Y));
        sub_w(c, 1, rd16(b, cam + 0xc));
        add_w(c, 1, dw(c, 0));
        move_w(c, 2, 0x100);
        add_w(c, 0, dw(c, 0));
        add_w(c, 2, dw(c, 0));
        cmp_w(c, 1, dw(c, 2));
    }
}

pub const IN_ARENA: Native = Native {
    entry: 0x1e92,
    name: "obj_in_arena",
    run: crate::Body::Processeur(|c, b| {
        in_arena_regs(c, b, Obj(c.a[0]));
        rts(c, b);
    }),
    touches_sound: false,
    outputs: regs::ALL,
};

/// 0x1e90: `in_arena` with the default margin 0x3e.
pub const IN_ARENA_DEFAULT: Native = Native {
    entry: 0x1e90,
    name: "obj_in_arena_default",
    run: crate::Body::Processeur(|c, b| {
        c.d[0] = ARENA_MARGIN as u32;
        c.flags_logic(ARENA_MARGIN as u32, Size::Long);
        in_arena_regs(c, b, Obj(c.a[0]));
        rts(c, b);
    }),
    touches_sound: false,
    outputs: regs::ALL,
};

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

/// `update_visibility` with the original's registers: D0 = 0 off screen or hidden, 1 without candidate queue,
/// otherwise class × 2; D1, D2, A1, A2 of the computations.
#[track_caller]
pub fn update_visibility_68k(c: &mut Cpu, b: &mut Cps2Bus, obj: Obj) -> bool {
    let visible = on_screen(c, b, obj);
    crate::diag::dead_regs(c, "cull", REGS_VISIBILITY);
    visible
}

/// 0x1ed2, registers included; true if the object is visible.
fn on_screen(c: &mut Cpu, b: &mut Cps2Bus, obj: Obj) -> bool {
    let cam = camera(b);
    c.a[2] = cam;
    let class = obj.u8(b, Obj::CLASS);
    c.flags_logic(class as u32, Size::Byte);
    let visible = if class != 0 {
        let index = lsl_word(c, obj.u8(b, BOUNDS_INDEX) as u16, 3);
        let t = BOUNDS_TABLE + index as u32;
        c.d[0] = index as u32;
        load_screen_x(c, b, 0, obj, cam);
        add_w(c, 0, rd16(b, t));
        cmp_w(c, 0, rd16(b, t + 2));
        let x_ok = c.cond(3 /* ls */);
        set_low_byte(&mut c.d[1], if x_ok { 0xff } else { 0 });
        load_screen_height(c, b, 0, obj, cam);
        add_w(c, 0, rd16(b, t + 4));
        cmp_w(c, 0, rd16(b, t + 6));
        let y_ok = c.cond(3);
        set_low_byte(&mut c.d[2], if y_ok { 0xff } else { 0 });
        c.a[1] = t + 6;
        set_low_byte(&mut c.d[1], if x_ok && y_ok { 0xff } else { 0 });
        c.flags_logic(c.d[1] & 0xff, Size::Byte);
        x_ok && y_ok
    } else {
        c.d[0] = 0x60;
        c.flags_logic(0x60, Size::Long);
        load_screen_x(c, b, 1, obj, cam);
        add_w(c, 1, dw(c, 0));
        move_w(c, 2, 0x180);
        add_w(c, 0, dw(c, 0));
        add_w(c, 2, dw(c, 0));
        cmp_w(c, 1, dw(c, 2));
        !c.cond(2 /* hi */) && {
            load_screen_height(c, b, 1, obj, cam);
            add_w(c, 1, dw(c, 0));
            move_w(c, 2, 0x100);
            add_w(c, 2, dw(c, 0));
            cmp_w(c, 1, dw(c, 2));
            !c.cond(2)
        }
    };
    let flags = obj.u8(b, Obj::FLAGS);
    let shown = visible && flags & 2 == 0;
    if shown {
        obj.set_u8(b, Obj::FLAGS, flags | 1);
        c.d[0] = class as u32;
        c.flags_logic(class as u32, Size::Byte);
        match queue_of(class) {
            Some(q) => {
                add_w(c, 0, dw(c, 0));
                let next = push_candidate(b, obj, q);
                c.a[2] = next as i16 as i32 as u32 & 0xff_ffff;
                c.flags_logic(next as u32, Size::Word);
            }
            None => {
                c.d[0] = 1;
                c.flags_logic(1, Size::Long);
            }
        }
    } else {
        obj.set_u8(b, Obj::FLAGS, flags & !1);
        c.d[0] = 0;
        c.flags_logic(0, Size::Long);
    }
    shown
}

pub const ON_SCREEN: Native = Native {
    entry: 0x1ed2,
    name: "obj_on_screen",
    run: crate::Body::Processeur(|c, b| {
        on_screen(c, b, Obj(c.a[0]));
        rts(c, b);
    }),
    touches_sound: false,
    outputs: regs::d(0) | regs::CCR,
};
