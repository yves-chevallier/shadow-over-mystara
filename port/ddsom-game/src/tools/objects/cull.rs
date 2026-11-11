//! Tools: the natives of `crate::world::objects::cull` for the original code that calls them (arguments and results in the registers
//! of the original).

use crate::world::World;
use m68k::decode::Size;
use m68k::Cpu;

use crate::world::objects::cull::*;
use crate::tools::frames::*;
use crate::{regs, Native};
use crate::tools::objects::motion::{cmp, lsl_word};
use crate::world::objects::Obj;
use crate::hw::mem::{rd16, set_low_byte};

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
fn load_screen_x(c: &mut Cpu, b: &mut World, n: usize, obj: Obj, cam: u32) {
    move_w(c, n, obj.u16(b, Obj::X));
    sub_w(c, n, rd16(b, cam + 8));
}

/// D`n`.w = screen height; `move.w Z,Dn ; add.w Y,Dn ; sub.w cam.y,Dn`.
fn load_screen_height(c: &mut Cpu, b: &mut World, n: usize, obj: Obj, cam: u32) {
    move_w(c, n, obj.u16(b, Obj::Z));
    add_w(c, n, obj.u16(b, Obj::Y));
    sub_w(c, n, rd16(b, cam + 0xc));
}

/// 0x1e1a, registers included.
fn cull_offscreen(c: &mut Cpu, b: &mut World, obj: Obj) {
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

pub(crate) const CULL_OFFSCREEN: Native = Native {
    entry: 0x1e1a,
    name: "obj_cull_offscreen",
    run: crate::Body::WithCpu(|c, b| {
        cull_offscreen(c, b, Obj(c.a[0]));
        rts(c, b);
    }),
    touches_sound: false,
    outputs: regs::ALL,
};

/// 0x1e92, registers included: margin in D0.w.
fn in_arena_regs(c: &mut Cpu, b: &mut World, obj: Obj) {
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

pub(crate) const IN_ARENA: Native = Native {
    entry: 0x1e92,
    name: "obj_in_arena",
    run: crate::Body::WithCpu(|c, b| {
        in_arena_regs(c, b, Obj(c.a[0]));
        rts(c, b);
    }),
    touches_sound: false,
    outputs: regs::ALL,
};

/// 0x1e90: `in_arena` with the default margin 0x3e.
pub(crate) const IN_ARENA_DEFAULT: Native = Native {
    entry: 0x1e90,
    name: "obj_in_arena_default",
    run: crate::Body::WithCpu(|c, b| {
        c.d[0] = ARENA_MARGIN as u32;
        c.flags_logic(ARENA_MARGIN as u32, Size::Long);
        in_arena_regs(c, b, Obj(c.a[0]));
        rts(c, b);
    }),
    touches_sound: false,
    outputs: regs::ALL,
};

/// 0x1ed2, registers included; true if the object is visible.
fn on_screen(c: &mut Cpu, b: &mut World, obj: Obj) -> bool {
    let cam = camera(b);
    c.a[2] = cam;
    let class = obj.u8(b, Obj::CLASS);
    c.flags_logic(class as u32, Size::Byte);
    let visible = if class != 0 {
        let index = lsl_word(c, obj.u8(b, Obj::BOUNDS) as u16, 3);
        let t = crate::world::data::data().sprite_tables.bounds_rom + index as u32;
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

pub(crate) const ON_SCREEN: Native = Native {
    entry: 0x1ed2,
    name: "obj_on_screen",
    run: crate::Body::WithCpu(|c, b| {
        on_screen(c, b, Obj(c.a[0]));
        rts(c, b);
    }),
    touches_sound: false,
    outputs: regs::d(0) | regs::CCR,
};
