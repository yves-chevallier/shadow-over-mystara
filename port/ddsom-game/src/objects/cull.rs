//! Présence à l'écran des objets : sortie d'écran, zones de jeu, inscription aux collisions.
//!
//! Les coordonnées écran sont des mots : abscisse = partie entière de `Obj::X` moins celle de la
//! caméra ; hauteur = `Z + Y` moins l'ordonnée de la caméra. Les tests de l'original se font en
//! arithmétique non signée sur 16 bits avec une marge ajoutée.
//!
//! Les routines natives reproduisent les registres et les drapeaux de l'original (les appelants
//! enchaînent sur `bcs`, `bhi`, `beq`...) en appliquant les mêmes opérations 16 bits.

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::Cpu;

use super::motion::{cmp, lsl_word};
use super::pools::{QUEUE_BOX, QUEUE_ENEMY_BOSS, QUEUE_ESHL, QUEUE_ITEM, QUEUE_PSHL};
use super::Obj;
use crate::mem::{rd16, rd8, rts, set_low_byte, wr16, A5};
use crate::{regs, Native};

/// Adresse de la caméra courante (x en +8, y en +0xc).
pub fn camera(b: &mut Cps2Bus) -> u32 {
    let alt = rd8(b, A5 + super::ram::USE_ALT_CAMERA) != 0;
    A5 + if alt { super::ram::ALT_CAMERA } else { super::ram::CAMERA }
}

/// Boîtes de visibilité par index (`BOUNDS_INDEX`) : (dx, largeur, dy, hauteur), 8 octets chacune.
const BOUNDS_TABLE: u32 = 0x1ca538;
/// Octet de l'objet qui désigne sa boîte de visibilité (octet haut de `FRAME_DATA_B`).
const BOUNDS_INDEX: u32 = 0x3c;

/// File de candidats aux collisions pour la classe d'objet : tirs joueur, ennemis et boss, tirs
/// ennemis, caisses, objets ramassables.
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

/// Ajoute l'objet à la file de candidats ; renvoie le nouveau pointeur d'écriture.
pub fn push_candidate(b: &mut Cps2Bus, obj: Obj, queue: &super::pools::Queue) -> u16 {
    let n = rd16(b, A5 + queue.count).wrapping_add(1);
    wr16(b, A5 + queue.count, n);
    let at = rd16(b, A5 + queue.cursor);
    wr16(b, at as i16 as i32 as u32, obj.0 as u16);
    let next = at.wrapping_add(2);
    wr16(b, A5 + queue.cursor, next);
    next
}

/// Abscisse à l'écran (avant marge) et hauteur à l'écran de l'objet.
fn screen_x(b: &mut Cps2Bus, obj: Obj, cam: u32) -> u16 {
    obj.u16(b, Obj::X).wrapping_sub(rd16(b, cam + 8))
}
fn screen_height(b: &mut Cps2Bus, obj: Obj, cam: u32) -> u16 {
    obj.u16(b, Obj::Z).wrapping_add(obj.u16(b, Obj::Y)).wrapping_sub(rd16(b, cam + 0xc))
}

// ---- opérations 16 bits sur D0-D2 avec les drapeaux du 68000 ----

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
/// D`n`.w = abscisse écran ; `move.w X,Dn ; sub.w cam.x,Dn`.
fn load_screen_x(c: &mut Cpu, b: &mut Cps2Bus, n: usize, obj: Obj, cam: u32) {
    move_w(c, n, obj.u16(b, Obj::X));
    sub_w(c, n, rd16(b, cam + 8));
}
/// D`n`.w = hauteur écran ; `move.w Z,Dn ; add.w Y,Dn ; sub.w cam.y,Dn`.
fn load_screen_height(c: &mut Cpu, b: &mut Cps2Bus, n: usize, obj: Obj, cam: u32) {
    move_w(c, n, obj.u16(b, Obj::Z));
    add_w(c, n, obj.u16(b, Obj::Y));
    sub_w(c, n, rd16(b, cam + 0xc));
}

// ---- 0x1e1a : sortie d'écran ----

/// Un objet hors de l'écran (marge 0x40) et actif passe à l'état 6 (disparition).
pub fn despawn_if_offscreen(b: &mut Cps2Bus, obj: Obj) {
    let cam = camera(b);
    let x_out = screen_x(b, obj, cam).wrapping_add(0x40) >= 0x200;
    let y_out = screen_height(b, obj, cam).wrapping_add(0x40) > 0x180;
    if (x_out || y_out) && obj.is_active(b) {
        obj.set_u8(b, Obj::STATE, 6);
    }
}

/// 0x1e1a : `despawn_if_offscreen` avec les registres de l'original (A1 = caméra, D0 = dernier test).
fn cull_offscreen(c: &mut Cpu, b: &mut Cps2Bus) {
    let obj = Obj(c.a[0]);
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
    rts(c, b);
}

pub const CULL_OFFSCREEN: Native = Native {
    entry: 0x1e1a,
    name: "obj_cull_offscreen",
    run: cull_offscreen,
    touches_sound: false,
    outputs: regs::ALL,
};

// ---- 0x1e92 : zone de jeu élargie ----

/// 0x1e92 : l'objet A0 est-il dans la zone élargie de la marge D0.w ? (C = dedans, comme le
/// `cmp` final de l'original.) Une seconde marge fixe, 0x3e, s'applique à la hauteur.
fn in_arena(c: &mut Cpu, b: &mut Cps2Bus) {
    let obj = Obj(c.a[0]);
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
    rts(c, b);
}

/// 0x1e90 : `in_arena` avec la marge par défaut 0x3e.
fn in_arena_default(c: &mut Cpu, b: &mut Cps2Bus) {
    c.d[0] = 0x3e;
    c.flags_logic(0x3e, Size::Long);
    in_arena(c, b);
}

pub const IN_ARENA: Native = Native {
    entry: 0x1e92,
    name: "obj_in_arena",
    run: in_arena,
    touches_sound: false,
    outputs: regs::ALL,
};

pub const IN_ARENA_DEFAULT: Native = Native {
    entry: 0x1e90,
    name: "obj_in_arena_default",
    run: in_arena_default,
    touches_sound: false,
    outputs: regs::ALL,
};

// ---- 0x1ed2 : visibilité et inscription aux collisions ----

/// Visibilité d'un objet : décors (classe 0) dans l'écran avec une marge de 0x60 pixels, les autres
/// dans leur boîte de visibilité.
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

/// Met à jour le bit 0 de `Obj::FLAGS` (visible à cette image) et inscrit l'objet visible dans la
/// file de candidats aux collisions de sa classe. Renvoie vrai si l'objet est visible.
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

/// 0x1ed2 : D0 = 0 hors écran ou caché, 1 sans file de candidats, sinon classe × 2.
fn on_screen(c: &mut Cpu, b: &mut Cps2Bus) {
    let obj = Obj(c.a[0]);
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
    if visible && flags & 2 == 0 {
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
    rts(c, b);
}

pub const ON_SCREEN: Native = Native {
    entry: 0x1ed2,
    name: "obj_on_screen",
    run: on_screen,
    touches_sound: false,
    outputs: regs::d(0) | regs::CCR,
};
