//! Construction of the sprite list (Object RAM), once per frame.
//!
//! The hardware displays a table of 4-word entries (x, y, tile, attributes) ended by a
//! marker. The game fills it from linked lists of objects: each object points to a
//! frame (`Frame`), made of 16x16 pieces offset from its position. A budget limits
//! the number of pieces per frame; a shadow is added under the characters.

use cps2::Cps2Bus;

use ddsom_data::sprites::Piece;

use crate::data::{data, Missing, RomAddr};
use crate::mem::{rd16, rd32, rd8, wr16, wr32, wr8};
use crate::Native;

/// A5 variables (RAM FF8000) used by rendering.
mod ram {
    /// Piece budget remaining for the current frame (initialised to 0x36a).
    pub const BUDGET: u32 = 0x5b8;
    pub const BUDGET_INIT: u32 = 0x5b6;
    /// Object bank toggle, inverted at each list built.
    pub const OBJ_BANK: u32 = 0x5bc;
    /// Player markers to display (one bit per player).
    pub const PLAYER_MARKERS: u32 = 0x5bd;
    /// Heads of the object lists: characters and scenery (0x5be), foreground (0x5c2).
    pub const ACTORS: u32 = 0x5be;
    pub const FOREGROUND: u32 = 0x5c2;
    /// Texts in sprites: (string pointer, x, y) x 3.
    pub const TEXTS: [u32; 3] = [0x5c6, 0x5ce, 0x5d6];
    /// Camera (0x4fe, or 0x54e when USE_ALT_CAMERA): x at +8, y at +0xc.
    pub const CAMERA: u32 = 0x4fe;
    pub const ALT_CAMERA: u32 = 0x54e;
    pub const USE_ALT_CAMERA: u32 = 0x36;
    pub const FOUR_PLAYERS: u32 = 0x20;
    pub const STAGE: u32 = 0x34;
    pub const TEXT_WORDS: u32 = 0x72;
    /// Settings (bit 9: no shadows).
    pub const DIP: u32 = 0x7fd0;
    /// List already built for this frame (reset to 0 by VBLANK).
    pub const BUILT: u32 = 0x7fce;
    /// Lists of visible objects by category: counters and write pointers.
    pub const VISIBLE_COUNT: u32 = 0x7e72;
    pub const VISIBLE_PTR: u32 = 0x7e7c;
}

/// Fields of an object.
mod obj {
    pub const ACTIVE: u32 = 0x00;
    /// Bit 0: visible in this frame; bit 1: hidden.
    pub const FLAGS: u32 = 0x01;
    pub const X: u32 = 0x08;
    pub const Z: u32 = 0x0c;
    pub const Y: u32 = 0x10;
    pub const NEXT: u32 = 0x26;
    /// Category (0: scenery; even: character or projectile, entered in a visible list).
    pub const KIND: u32 = 0x28;
    /// Bit 0: imposed palette (PALETTE).
    pub const PALETTE_OVERRIDE: u32 = 0x2a;
    pub const PALETTE: u32 = 0x2b;
    pub const FLIP_X: u32 = 0x2c;
    pub const SHADOW_SET: u32 = 0x2e;
    pub const FRAME: u32 = 0x34;
    pub const SHADOW_A: u32 = 0x38;
    pub const SHADOW_B: u32 = 0x3a;
    pub const GROUND_MARK: u32 = 0x3b;
    pub const BOUNDS: u32 = 0x3c;
    pub const PRIORITY: u32 = 0x3f;
    pub const HIDDEN: u32 = 0x40;
    /// Attached objects (negative index = RAM pointer): drawn in front or behind according to their Y.
    pub const ATTACHED: [u32; 2] = [0x4a, 0x48];
}

/// Program ROM tables.
mod rom {
    /// Visibility boxes by object category: (dx, width, dy, height).
    pub const BOUNDS: u32 = 0x1ca538;
    /// Ground marks: (cost, tile, dx, attributes, mirror dx, dy, priority bits, -).
    pub const GROUND_MARKS: u32 = 0x1e45c;
    /// Positions of the player markers (2 and 4 players).
    pub const MARKERS: [u32; 2] = [0x1eb48, 0x1eb58];
    /// Detailed shadows: table of sets, then 8-byte entries.
    pub const SHADOWS_A: u32 = 0x11bffa;
    pub const SHADOWS_B: u32 = 0x11c23a;
}

const OBJECT_RAM: u32 = 0x708000;

/// Current viewpoint.
#[derive(Clone, Copy)]
struct Camera {
    x: u16,
    y: u16,
}

/// Writes the entries of the object table and keeps the piece budget.
struct Painter<'a> {
    b: &'a mut Cps2Bus,
    a5: u32,
    at: u32,
    cam: Camera,
    /// D4 as the original leaves it (attributes of the last piece, colour of the last text...):
    /// the handlers that follow read this leftover (e.g. 0xad7b0, which adds D4 to the position).
    d4: u32,
}

impl Painter<'_> {
    fn set_d4_low(&mut self, v: u16) {
        self.d4 = self.d4 & 0xffff_0000 | v as u32;
    }
}

impl Painter<'_> {
    fn ram(&mut self, off: u32) -> u32 {
        self.a5.wrapping_add(off)
    }

    fn sprite(&mut self, x: u16, y: u16, code: u16, attr: u16) {
        for w in [x, y, code, attr] {
            wr16(self.b, self.at, w);
            self.at += 2;
        }
    }

    /// Removes `n` pieces from the budget; false if not enough remain.
    fn spend(&mut self, n: u16) -> bool {
        let a = self.ram(ram::BUDGET);
        let left = rd16(self.b, a);
        if n > left {
            return false;
        }
        wr16(self.b, a, left - n);
        true
    }
}

fn camera(b: &mut Cps2Bus, a5: u32) -> Camera {
    let base = if rd8(b, a5 + ram::USE_ALT_CAMERA) != 0 { ram::ALT_CAMERA } else { ram::CAMERA };
    Camera { x: rd16(b, a5 + base + 8), y: rd16(b, a5 + base + 0xc) }
}

/// Address of an object designated by a word (negative index in RAM FF8000-FFFFFF).
fn obj_at(w: u16) -> u32 {
    w as i16 as i32 as u32 & 0xff_ffff
}

fn screen_x(p: &mut Painter, o: u32) -> u16 {
    let x = rd16(p.b, o + obj::X);
    x.wrapping_sub(p.cam.x)
}

/// Height on screen: y + z relative to the camera.
fn screen_height(p: &mut Painter, o: u32) -> u16 {
    let h = rd16(p.b, o + obj::Y).wrapping_add(rd16(p.b, o + obj::Z));
    h.wrapping_sub(p.cam.y)
}

/// What the construction of the object table leaves in the registers of the original.
#[derive(Clone, Copy, Debug)]
pub struct Leftovers {
    /// D4: attributes of the last piece, colour of the last text... (low word or long, depending on the last one
    /// written).
    pub d4: u32,
    /// A4: write pointer at the end of the table.
    pub end: u32,
    /// A6: block of the current camera.
    pub camera: u32,
}

/// 0x1de96: builds the frame's object table (nothing if already built: `None`). `d4`: the caller's D4,
/// of which the original sometimes replaces only the low word.
/// Original glitch: D4 keeps the attributes of the last piece drawn, and handlers reread it later
/// (0xad7b0 adds it to a position): callers return it to the processor (`Leftovers::d4`).
pub fn build(b: &mut Cps2Bus, d4: u32) -> Option<Leftovers> {
    let a5 = crate::mem::A5;
    if rd16(b, a5 + ram::BUILT) != 0 {
        return None;
    }
    {
        let cam = camera(b, a5);
        let mut p = Painter { b, a5, at: OBJECT_RAM, cam, d4 };
        for a in [ram::BUDGET_INIT, ram::BUDGET] {
            let a = p.ram(a);
            wr16(p.b, a, 0x36a);
        }
        // two empty entries at the head
        p.sprite(0, 0, 0x1800, 0);
        p.sprite(0, 0, 0x1800, 0);
        draw_foreground(&mut p);
        draw_ground_marks(&mut p);
        draw_actors(&mut p);
        draw_player_markers(&mut p);
        for t in ram::TEXTS {
            draw_text(&mut p, t);
        }
        // end-of-table marker
        p.sprite(0x8000, 0x8000, 0, 0);
        let bank = a5 + ram::OBJ_BANK;
        let v = rd8(p.b, bank) ^ 0xff;
        wr8(p.b, bank, v);
        wr16(p.b, a5 + ram::BUILT, 1);
        let camera = a5 + if rd8(p.b, a5 + ram::USE_ALT_CAMERA) != 0 { ram::ALT_CAMERA } else { ram::CAMERA };
        Some(Leftovers { d4: p.d4, end: p.at, camera })
    }
}

/// `build` in the game. Original glitch: the D4 that the list leaves (attributes of the last piece) is reread later
/// (`DDSOM_POISON=call_1de96`: long_28_scenes, frame 10431): it is the leftover D4 of the game
/// (`dispatch::Leftovers`). A4 and A6 are reread by nobody.
pub fn build_in_game(b: &mut Cps2Bus) {
    let d4 = crate::tasks::table::table(b).leftovers.d4;
    if let Some(r) = build(b, d4) {
        crate::tasks::table::table(b).leftovers.d4 = r.d4;
    }
}

/// 0x1df14: foreground objects.
fn draw_foreground(p: &mut Painter) {
    let mut w = { let a = p.ram(ram::FOREGROUND); rd16(p.b, a) };
    if w == 0 {
        return;
    }
    loop {
        let o = obj_at(w);
        if rd8(p.b, o + obj::HIDDEN) == 0 && check_visible(p, o) {
            draw_object(p, o);
            draw_shadow(p, o);
        }
        w = rd16(p.b, o + obj::NEXT);
        if (w as i16) >= 0 {
            return;
        }
    }
}

/// 0x1ed2: visibility of an object (updates bit 0 of FLAGS) and entry of the visible
/// objects in the list of their category.
fn check_visible(p: &mut Painter, o: u32) -> bool {
    let kind = rd8(p.b, o + obj::KIND);
    let inside = if kind != 0 {
        let t = rom::BOUNDS + 8 * rd8(p.b, o + obj::BOUNDS) as u32;
        let dx = screen_x(p, o).wrapping_add(rd16(p.b, t));
        let dy = screen_height(p, o).wrapping_add(rd16(p.b, t + 4));
        dx <= rd16(p.b, t + 2) && dy <= rd16(p.b, t + 6)
    } else {
        on_screen(p, o)
    };
    let flags = o + obj::FLAGS;
    if !inside || rd8(p.b, flags) & 2 != 0 {
        let f = rd8(p.b, flags) & !1;
        wr8(p.b, flags, f);
        return false;
    }
    let f = rd8(p.b, flags) | 1;
    wr8(p.b, flags, f);
    // lists by category: (2, 4) -> 0, 6 -> 1, 8 -> 2, 10 -> 3, 12 -> 4
    let list = match kind {
        2 | 4 => Some(2),
        6 => Some(0),
        8 => Some(1),
        10 => Some(3),
        12 => Some(4),
        0 | 13.. => None,
        _ => unreachable!("odd object category {kind}: not handled by the game"),
    };
    if let Some(l) = list {
        let count = p.ram(ram::VISIBLE_COUNT + 2 * l);
        let n = rd16(p.b, count).wrapping_add(1);
        wr16(p.b, count, n);
        let ptr = p.ram(ram::VISIBLE_PTR + 2 * l);
        let at = rd16(p.b, ptr);
        wr16(p.b, obj_at(at), o as u16);
        wr16(p.b, ptr, at.wrapping_add(2));
    }
    true
}

/// Screen test of the scenery objects: margin of 0x60 pixels around the screen.
fn on_screen(p: &mut Painter, o: u32) -> bool {
    screen_x(p, o).wrapping_add(0x60) <= 0x240 && screen_height(p, o).wrapping_add(0xc0) <= 0x1c0
}

/// 0x1e3a6: ground marks (circles under the characters), visibility pass of the main list.
fn draw_ground_marks(p: &mut Painter) {
    let mut w = { let a = p.ram(ram::ACTORS); rd16(p.b, a) };
    if w == 0 {
        return;
    }
    loop {
        let o = obj_at(w);
        check_visible(p, o);
        let stage = { let a = p.ram(ram::STAGE); rd8(p.b, a) };
        let skip = (stage == 0x29 && rd16(p.b, o + obj::Z) == 0)
            || (rd8(p.b, o + obj::KIND) != 0 && rd8(p.b, o + obj::HIDDEN) != 0)
            || rd8(p.b, o + obj::FLAGS) & 1 == 0;
        if !skip {
            let e = rom::GROUND_MARKS + 16 * rd8(p.b, o + obj::GROUND_MARK) as u32;
            let field = |p: &mut Painter, i: u32| rd16(p.b, e + 2 * i);
            let cost = field(p, 0);
            if !p.spend(cost) {
                return;
            }
            let (code, mut attr, mut dx) = (field(p, 1), field(p, 3), field(p, 4));
            if rd8(p.b, o + obj::FLIP_X) != 0 {
                dx = dx.wrapping_neg();
                attr ^= 0x20;
            }
            let x = screen_x(p, o).wrapping_add(0x40).wrapping_add(field(p, 2)).wrapping_add(dx);
            let y = rd16(p.b, o + obj::Y).wrapping_sub(p.cam.y).wrapping_add(0xc);
            let y = 0x100u16.wrapping_sub(y).wrapping_sub(field(p, 5));
            let prio = field(p, 6);
            p.set_d4_low(attr);
            p.sprite(x & 0x3ff | prio, y & 0x3ff | 0x2000, code, attr);
        }
        w = rd16(p.b, o + obj::NEXT);
        if (w as i16) >= 0 {
            return;
        }
    }
}

/// 0x1df3e: characters and scenery, with their attached objects in front/behind.
fn draw_actors(p: &mut Painter) {
    let mut w = { let a = p.ram(ram::ACTORS); rd16(p.b, a) };
    if w == 0 {
        return;
    }
    loop {
        let o = obj_at(w);
        let kind = rd8(p.b, o + obj::KIND);
        if kind != 0 && rd8(p.b, o + obj::HIDDEN) != 0 {
            // hidden object
        } else if kind != 0 {
            if rd8(p.b, o + obj::FLAGS) & 1 != 0 {
                draw_object(p, o);
                draw_shadow(p, o);
            }
        } else {
            draw_attached(p, o, true);
            let flags = o + obj::FLAGS;
            let f = rd8(p.b, flags) & !1;
            wr8(p.b, flags, f);
            if f & 2 == 0 && on_screen(p, o) {
                wr8(p.b, flags, f | 1);
                draw_object(p, o);
                draw_shadow(p, o);
            }
            draw_attached(p, o, false);
        }
        w = rd16(p.b, o + obj::NEXT);
        if (w as i16) >= 0 {
            return;
        }
    }
}

/// Attached objects behind (negative Y) or in front of (positive Y) the visible parent.
fn draw_attached(p: &mut Painter, parent: u32, behind: bool) {
    for field in obj::ATTACHED {
        let w = rd16(p.b, parent + field);
        if (w as i16) >= 0 {
            continue;
        }
        let a = obj_at(w);
        let is_behind = (rd16(p.b, a + obj::Y) as i16) < 0;
        if is_behind == behind && rd8(p.b, parent + obj::FLAGS) & 1 != 0 {
            draw_child(p, parent, a);
        }
    }
}

/// Frame of an object: header then pieces.
struct Frame {
    count: u16,
    second: u16,
    bank: u8,
    per_piece: bool,
    pieces: Pieces,
}

/// Pieces of a frame: from the frame table, or read by address in the residue for a frame address the table does not
/// know.
enum Pieces {
    Typed(&'static [Piece]),
    Residue { offsets: u32, pieces: u32 },
}

/// Piece `i` of a frame: offset (dx, dy) from the anchor, sprite code and attributes (own attributes only when the
/// frame has per-piece attributes).
struct PieceWords {
    dx: u16,
    dy: u16,
    code: u16,
    attr: u16,
}

impl Frame {
    fn piece(&self, b: &mut Cps2Bus, i: u32) -> PieceWords {
        match self.pieces {
            Pieces::Typed(pieces) => {
                let p = pieces[i as usize];
                PieceWords { dx: p.dx() as u16, dy: p.dy() as u16, code: p.code(), attr: p.attr() }
            }
            Pieces::Residue { offsets, pieces } => {
                let missing = Missing { table: "frame", key: pieces - 10 };
                let at = pieces + if self.per_piece { 4 } else { 2 } * i;
                PieceWords {
                    dx: missing.read16(b, offsets + 4 * i),
                    dy: missing.read16(b, offsets + 4 * i + 2),
                    code: missing.read16(b, at),
                    attr: if self.per_piece { missing.read16(b, at + 2) } else { 0 },
                }
            }
        }
    }
}

fn frame_at(b: &mut Cps2Bus, f: u32) -> Frame {
    if let Some(fr) = data().frames.at(RomAddr(f)) {
        return Frame {
            count: fr.cost,
            second: fr.attr,
            bank: fr.bank,
            per_piece: fr.per_piece(),
            pieces: Pieces::Typed(&fr.pieces),
        };
    }
    let missing = Missing { table: "frame", key: f };
    let (count, second, bank_mode) = (missing.read16(b, f), missing.read16(b, f + 2), missing.read16(b, f + 4));
    Frame {
        count,
        second,
        bank: (bank_mode >> 8) as u8,
        per_piece: bank_mode as u8 != 0,
        pieces: Pieces::Residue { offsets: missing.read32(b, f + 6), pieces: f + 10 },
    }
}

/// Priority/bank word combined with the positions (ror.l #3 of (bank << 16 | priority)).
fn position_bits(bank: u8, prio: u8) -> u32 {
    ((bank as u32) << 16 | prio as u32).rotate_right(3)
}

/// Style of an object: mirror and imposed palette.
#[derive(Clone, Copy)]
struct Style {
    flip: bool,
    palette: Option<u8>,
}

fn style(b: &mut Cps2Bus, o: u32) -> Style {
    Style {
        flip: rd8(b, o + obj::FLIP_X) != 0,
        palette: (rd8(b, o + obj::PALETTE_OVERRIDE) & 1 != 0).then(|| rd8(b, o + obj::PALETTE)),
    }
}

fn with_palette(attr: u16, pal: Option<u8>) -> u16 {
    match pal {
        Some(p) => (attr & 0xff00) | ((attr as u8 & 0xe0) | p) as u16,
        None => attr,
    }
}

/// Emits the pieces of a frame at position (x, y) (y already flipped for the screen).
fn emit_frame(p: &mut Painter, fr: &Frame, x: u16, y: u16, bits: u32, st: Style, palette_after_flip: bool) {
    let n = if fr.per_piece { fr.second } else { fr.count };
    let mut common = fr.second;
    if !fr.per_piece {
        common = with_palette(common, st.palette);
        if st.flip {
            common ^= 0x20;
        }
    }
    if !fr.per_piece {
        p.set_d4_low(common);
    } else {
        p.set_d4_low(fr.second);
    }
    for i in 0..n as u32 {
        let PieceWords { dx, dy, code, attr } = fr.piece(p.b, i);
        let mut attr = if fr.per_piece { attr } else { common };
        let width = (attr & 0xf00) >> 4;
        if fr.per_piece && st.flip {
            p.set_d4_low(width);
        }
        let px = if st.flip {
            x.wrapping_sub(dx).wrapping_sub(0x10).wrapping_add(0x40).wrapping_sub(width)
        } else {
            x.wrapping_add(dx).wrapping_add(0x40)
        };
        if fr.per_piece {
            if st.flip && palette_after_flip {
                attr = with_palette(attr ^ 0x20, st.palette);
            } else if st.flip {
                attr ^= 0x20;
                attr = with_palette(attr, st.palette);
            } else {
                attr = with_palette(attr, st.palette);
            }
        }
        let pos = ((px as u32) << 16 | y.wrapping_add(dy) as u32) & 0x3ff_03ff | bits;
        p.sprite((pos >> 16) as u16, pos as u16, code, attr);
    }
}

/// 0x1e062: draws the current frame of an object.
fn draw_object(p: &mut Painter, o: u32) {
    if rd8(p.b, o + obj::ACTIVE) == 0 {
        return;
    }
    let f = rd32(p.b, o + obj::FRAME);
    if f == 0 {
        return;
    }
    let fr = frame_at(p.b, f);
    if !p.spend(fr.count) {
        return;
    }
    let x = screen_x(p, o);
    let y = (!screen_height(p, o)).wrapping_add(0x101);
    let bits = position_bits(fr.bank, rd8(p.b, o + obj::PRIORITY));
    let st = style(p.b, o);
    emit_frame(p, &fr, x, y, bits, st, true);
}

/// 0x1e0b4: attached object, positioned relative to its parent (priority 4).
fn draw_child(p: &mut Painter, parent: u32, a: u32) {
    if rd8(p.b, a + obj::ACTIVE) == 0 {
        return;
    }
    let f = rd32(p.b, a + obj::FRAME);
    let fr = frame_at(p.b, f);
    if !p.spend(fr.count) {
        return;
    }
    let x = rd16(p.b, a + obj::X).wrapping_add(rd16(p.b, parent + obj::X)).wrapping_sub(p.cam.x);
    let h = rd16(p.b, a + obj::Z)
        .wrapping_add(rd16(p.b, parent + obj::Z))
        .wrapping_add(rd16(p.b, parent + obj::Y))
        .wrapping_sub(p.cam.y);
    let y = (!h).wrapping_add(0x101);
    let bits = position_bits(fr.bank, 4);
    let st = style(p.b, a);
    emit_frame(p, &fr, x, y, bits, st, true);
}

/// 0x1ec4e: ground shadow, plus any detailed shadows.
fn draw_shadow(p: &mut Painter, o: u32) {
    let dip = { let a = p.ram(ram::DIP); rd16(p.b, a) };
    if dip & 0x200 != 0 || !p.spend(0xb) {
        return;
    }
    let x = screen_x(p, o).wrapping_add(0x38);
    let y = 0x100u16.wrapping_sub(screen_height(p, o)).wrapping_sub(8);
    p.sprite(x & 0x3ff | 0x8000, y & 0x3ff | 0x2000, 0x1890, 0xf);
    for (field, table) in [(obj::SHADOW_A, rom::SHADOWS_A), (obj::SHADOW_B, rom::SHADOWS_B)] {
        let n = rd8(p.b, o + field) as u32;
        if n != 0 {
            let set = rd8(p.b, o + obj::SHADOW_SET) as u32;
            let base = rd32(p.b, table + 4 * set);
            draw_shadow_quad(p, o, base + 8 * n);
        }
    }
}

/// 0x1ed22: detailed shadow in four quarters around a point.
fn draw_shadow_quad(p: &mut Painter, o: u32, e: u32) {
    let mut dx = rd8(p.b, e) as i8 as u16;
    if rd8(p.b, o + obj::FLIP_X) != 0 {
        dx = dx.wrapping_neg();
    }
    let cx = screen_x(p, o).wrapping_add(dx);
    let dy = rd8(p.b, e + 2).wrapping_add(rd8(p.b, e + 4)) as i8 as u16;
    let cy = screen_height(p, o).wrapping_add(dy);
    let (w, h) = (rd8(p.b, e + 1) as u16, rd8(p.b, e + 3) as u16);
    p.d4 = w as u32;
    let quads = [
        (cx.wrapping_sub(w), cy.wrapping_add(h), 0x1893),
        (cx.wrapping_sub(w), cy.wrapping_sub(h).wrapping_add(0x10), 0x1895),
        (cx.wrapping_add(w).wrapping_sub(0x10), cy.wrapping_sub(h).wrapping_add(0x10), 0x1896),
        (cx.wrapping_add(w).wrapping_sub(0x10), cy.wrapping_add(h), 0x1894),
    ];
    for (x, y, code) in quads {
        let sx = x.wrapping_add(0x40) & 0x3ff | 0x8000;
        let sy = 0x100u16.wrapping_sub(y) & 0x3ff | 0x2000;
        p.sprite(sx, sy, code, 0xf);
    }
}

/// 0x1ea6c: player markers ("1P" arrows...).
fn draw_player_markers(p: &mut Painter) {
    let four = { let a = p.ram(ram::FOUR_PLAYERS); rd8(p.b, a) } & 2 != 0;
    let table = rom::MARKERS[four as usize];
    let shown = { let a = p.ram(ram::PLAYER_MARKERS); rd8(p.b, a) };
    // (high tile, low tile) and palette of each player
    let looks = [(0x1fba, 0x1fbc, 0x0c), (0x1fc0, 0x1fc2, 0x0c), (0x1fba, 0x1fbc, 0x0d), (0x1fc0, 0x1fc2, 0x0d)];
    for (k, (top, bottom, pal)) in looks.into_iter().enumerate() {
        if shown & (1 << k) != 0 {
            let x = rd16(p.b, table + 4 * k as u32);
            let y = rd16(p.b, table + 4 * k as u32 + 2);
            p.sprite(x, y, top, 0x100 | pal);
            p.sprite(x.wrapping_sub(0x20), y.wrapping_add(0x10), bottom, 0x300 | pal);
        }
    }
}

/// 0x1eb90 / 0x1ec0c / 0x1ec2c: character string displayed in 8-pixel sprites.
fn draw_text(p: &mut Painter, slot: u32) {
    let s = { let a = p.ram(slot); rd32(p.b, a) };
    if s == 0 {
        return;
    }
    let mut x = { let a = p.ram(slot + 4); rd16(p.b, a) }.wrapping_sub(0x4000);
    let mut y = { let a = p.ram(slot + 6); rd16(p.b, a) }.wrapping_add(0x2000);
    let mut color = 0xf;
    p.d4 = 0xf;
    let words = { let a = p.ram(ram::TEXT_WORDS); rd16(p.b, a) } == 2;
    let mut at = s;
    loop {
        if words {
            let w = rd16(p.b, at);
            at += 2;
            match w {
                0 => return,
                0xffff => {
                    x = x.wrapping_add(rd16(p.b, at));
                    y = y.wrapping_add(rd16(p.b, at + 2));
                    at += 4;
                }
                0xfffe => {
                    color = rd16(p.b, at);
                    p.set_d4_low(color);
                    at += 2;
                }
                code => {
                    p.sprite(x, y, code, color);
                    x = x.wrapping_add(8);
                }
            }
        } else {
            let c = rd8(p.b, at);
            at += 1;
            match c {
                0xff => {
                    x = x.wrapping_sub(8 * rd8(p.b, at) as u16);
                    y = y.wrapping_add(rd8(p.b, at + 1) as i8 as u16);
                    at += 2;
                }
                0xfe => {
                    color = rd8(p.b, at) as u16;
                    p.d4 = color as u32;
                    at += 1;
                }
                0xfd => return,
                _ => {
                    let code = if c & 0xf0 == 0 {
                        let lo = rd8(p.b, at);
                        at += 1;
                        ((c as u16 & 0xf) << 8) | lo as u16
                    } else {
                        c as u16
                    };
                    p.sprite(x, y, code + 0x1800, color);
                    x = x.wrapping_add(8);
                }
            }
        }
    }
}

/// 0x1de7c: clears the heads of the object lists, the player markers and the three texts.
pub fn clear_lists(b: &mut Cps2Bus) {
    let a5 = crate::mem::A5;
    wr32(b, a5 + ram::ACTORS, 0);
    wr32(b, a5 + ram::FOREGROUND, 0);
    wr8(b, a5 + ram::PLAYER_MARKERS, 0);
    for text in ram::TEXTS {
        wr32(b, a5 + text, 0);
    }
}

pub const RESET_LISTS: Native = Native {
    entry: 0x1de7c,
    name: "sprites_clear_lists",
    run: crate::Body::Memory(clear_lists),
    touches_sound: false,
    outputs: 0,
};
