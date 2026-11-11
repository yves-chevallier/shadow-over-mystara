//! Construction of the sprite list (Object RAM), once per frame.
//!
//! The hardware displays a table of 4-word entries (x, y, tile, attributes) ended by a
//! marker. The game fills it from linked lists of objects: each object points to a
//! frame (`Frame`), made of 16x16 pieces offset from its position. A budget limits
//! the number of pieces per frame; a shadow is added under the characters.

use crate::world::World;

use ddsom_data::sprites::Piece;
use ddsom_data::tables::GroundMark;

use crate::world::data::{data, Missing, RomAddr};
use crate::hw::video::{Vram, OBJECTS};
use crate::world::objects::{Link, Obj};
use crate::world::ram::{Long, Word};

/// Work RAM variables used by rendering.
mod var {
    use crate::world::ram::{Byte, Long, Word};

    /// Piece budget remaining for the current frame (initialised to 0x36a), and its initial value.
    pub const BUDGET: Word = Word::a5(0x5b8);
    pub const BUDGET_INIT: Word = Word::a5(0x5b6);
    /// Player markers to display (one bit per player).
    pub const PLAYER_MARKERS: Byte = Byte::a5(0x5bd);
    /// Heads of the object lists: characters and scenery, foreground (`objects::lists`).
    pub const ACTORS: Word = Word::a5(0x5be);
    pub const FOREGROUND: Word = Word::a5(0x5c2);
    /// The heads and tails of the two lists, as longs (cleared together).
    pub const ACTORS_LONG: Long = Long::a5(0x5be);
    pub const FOREGROUND_LONG: Long = Long::a5(0x5c2);
    /// Texts in sprites: (string pointer, x, y) x 3.
    pub const TEXTS: [u32; 3] = [Long::a5(0x5c6).0, Long::a5(0x5ce).0, Long::a5(0x5d6).0];
    /// List already built for this frame (reset to 0 by VBLANK).
    pub const BUILT: Word = Word::a5(0x7fce);
    /// Lists of visible objects by category: counters and write pointers (five words each).
    pub const VISIBLE_COUNT: u32 = Word::a5(0x7e72).0;
    pub const VISIBLE_PTR: u32 = Word::a5(0x7e7c).0;
}

/// A text displayed in sprites: string pointer, then x and y.
fn text_slot(at: u32) -> (Long, Word, Word) {
    (Long(at), Word(at + 4), Word(at + 6))
}

/// The object category byte (`Obj::CLASS`: 0 scenery; even: character or projectile, entered in a visible list).
fn category(b: &mut World, o: Obj) -> u8 {
    o.class(b)
}

/// Current viewpoint.
#[derive(Clone, Copy)]
struct Camera {
    x: u16,
    y: u16,
}

/// Writes the entries of the object table and keeps the piece budget.
struct Painter<'a> {
    b: &'a mut World,
    at: Vram,
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
    fn sprite(&mut self, x: u16, y: u16, code: u16, attr: u16) {
        for w in [x, y, code, attr] {
            self.at.set(self.b, w);
            self.at = self.at.offset(2);
        }
    }

    /// Removes `n` pieces from the budget; false if not enough remain.
    fn spend(&mut self, n: u16) -> bool {
        let left = var::BUDGET.get(self.b);
        if n > left {
            return false;
        }
        var::BUDGET.set(self.b, left - n);
        true
    }
}

fn camera(b: &mut World) -> Camera {
    let (x, y) = crate::world::ram::camera(b);
    Camera { x, y }
}

fn screen_x(p: &mut Painter, o: Obj) -> u16 {
    let x = o.u16(p.b, Obj::X);
    x.wrapping_sub(p.cam.x)
}

/// Height on screen: y + z relative to the camera.
fn screen_height(p: &mut Painter, o: Obj) -> u16 {
    let h = o.u16(p.b, Obj::Y).wrapping_add(o.u16(p.b, Obj::Z));
    h.wrapping_sub(p.cam.y)
}

/// Ground mark `i` (`sprites/tables.json`), or the words the original reads past the table (`sprites/marks_beyond`).
fn ground_mark(b: &mut World, i: u8) -> [u16; 8] {
    let t = &data().sprite_tables;
    match t.marks.get(i as usize) {
        Some(m) => GroundMark::words(m),
        None => {
            let at = t.marks_rom + 16 * i as u32;
            std::array::from_fn(|k| crate::world::tables::engine().word(b, at + 2 * k as u32))
        }
    }
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
pub fn build(b: &mut World, d4: u32) -> Option<Leftovers> {
    if var::BUILT.get(b) != 0 {
        return None;
    }
    let cam = camera(b);
    let mut p = Painter { b, at: OBJECTS, cam, d4 };
    for budget in [var::BUDGET_INIT, var::BUDGET] {
        budget.set(p.b, 0x36a);
    }
    // two empty entries at the head
    p.sprite(0, 0, 0x1800, 0);
    p.sprite(0, 0, 0x1800, 0);
    draw_foreground(&mut p);
    draw_ground_marks(&mut p);
    draw_actors(&mut p);
    draw_player_markers(&mut p);
    for t in var::TEXTS {
        draw_text(&mut p, t);
    }
    // end-of-table marker
    p.sprite(0x8000, 0x8000, 0, 0);
    let bank = crate::services::video::OBJECT_BANK.get(p.b) ^ 0xff;
    crate::services::video::OBJECT_BANK.set(p.b, bank);
    var::BUILT.set(p.b, 1);
    let camera = if crate::world::ram::CAMERA_ALT_ACTIVE.is_set(p.b) { crate::services::scroll::CAMERA_2 } else { crate::services::scroll::CAMERA_1 };
    Some(Leftovers { d4: p.d4, end: p.at.0, camera: camera.0 .0 })
}

/// `build` in the game. Original glitch: the D4 that the list leaves (attributes of the last piece) is reread later
/// (`DDSOM_POISON=call_1de96`: long_28_scenes, frame 10431): it is the leftover D4 of the game
/// (`dispatch::Leftovers`). A4 and A6 are reread by nobody.
pub fn build_in_game(b: &mut World) {
    let d4 = b.leftovers.d4;
    if let Some(r) = build(b, d4) {
        b.leftovers.d4 = r.d4;
    }
}

/// 0x1df14: foreground objects.
fn draw_foreground(p: &mut Painter) {
    let mut w = var::FOREGROUND.get(p.b);
    if w == 0 {
        return;
    }
    loop {
        let o = Link(w).target();
        if o.hidden(p.b) == 0 && check_visible(p, o) {
            draw_object(p, o);
            draw_shadow(p, o);
        }
        w = o.next(p.b).0;
        if (w as i16) >= 0 {
            return;
        }
    }
}

/// 0x1ed2: visibility of an object (updates bit 0 of FLAGS) and entry of the visible
/// objects in the list of their category.
fn check_visible(p: &mut Painter, o: Obj) -> bool {
    let kind = category(p.b, o);
    let inside = if kind != 0 {
        let i = o.bounds(p.b);
        let [dx, width, dy, height] = crate::world::objects::cull::visibility_box(p.b, i);
        let dx = screen_x(p, o).wrapping_add(dx);
        let dy = screen_height(p, o).wrapping_add(dy);
        dx <= width && dy <= height
    } else {
        on_screen(p, o)
    };
    if !inside || o.flags(p.b) & 2 != 0 {
        let f = o.flags(p.b) & !1;
        o.set_flags(p.b, f);
        return false;
    }
    let f = o.flags(p.b) | 1;
    o.set_flags(p.b, f);
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
        let count = Word(var::VISIBLE_COUNT + 2 * l);
        let n = count.get(p.b).wrapping_add(1);
        count.set(p.b, n);
        let ptr = Word(var::VISIBLE_PTR + 2 * l);
        let at = ptr.get(p.b);
        Word(Link(at).target().0).set(p.b, o.word());
        ptr.set(p.b, at.wrapping_add(2));
    }
    true
}

/// Screen test of the scenery objects: margin of 0x60 pixels around the screen.
fn on_screen(p: &mut Painter, o: Obj) -> bool {
    screen_x(p, o).wrapping_add(0x60) <= 0x240 && screen_height(p, o).wrapping_add(0xc0) <= 0x1c0
}

/// 0x1e3a6: ground marks (circles under the characters), visibility pass of the main list.
fn draw_ground_marks(p: &mut Painter) {
    let mut w = var::ACTORS.get(p.b);
    if w == 0 {
        return;
    }
    loop {
        let o = Link(w).target();
        check_visible(p, o);
        let stage = crate::world::ram::SCENE.get(p.b);
        let skip = (stage == 0x29 && o.u16(p.b, Obj::Z) == 0)
            || (category(p.b, o) != 0 && o.hidden(p.b) != 0)
            || o.flags(p.b) & 1 == 0;
        if !skip {
            let i = o.ground_mark(p.b);
            let [cost, code, fixed_x, mut attr, mut dx, dy, prio, _] = ground_mark(p.b, i);
            if !p.spend(cost) {
                return;
            }
            if o.flip(p.b) != 0 {
                dx = dx.wrapping_neg();
                attr ^= 0x20;
            }
            let x = screen_x(p, o).wrapping_add(0x40).wrapping_add(fixed_x).wrapping_add(dx);
            let y = o.u16(p.b, Obj::Y).wrapping_sub(p.cam.y).wrapping_add(0xc);
            let y = 0x100u16.wrapping_sub(y).wrapping_sub(dy);
            p.set_d4_low(attr);
            p.sprite(x & 0x3ff | prio, y & 0x3ff | 0x2000, code, attr);
        }
        w = o.next(p.b).0;
        if (w as i16) >= 0 {
            return;
        }
    }
}

/// 0x1df3e: characters and scenery, with their attached objects in front/behind.
fn draw_actors(p: &mut Painter) {
    let mut w = var::ACTORS.get(p.b);
    if w == 0 {
        return;
    }
    loop {
        let o = Link(w).target();
        let kind = category(p.b, o);
        if kind != 0 && o.hidden(p.b) != 0 {
            // hidden object
        } else if kind != 0 {
            if o.flags(p.b) & 1 != 0 {
                draw_object(p, o);
                draw_shadow(p, o);
            }
        } else {
            draw_attached(p, o, true);
            let f = o.flags(p.b) & !1;
            o.set_flags(p.b, f);
            if f & 2 == 0 && on_screen(p, o) {
                o.set_flags(p.b, f | 1);
                draw_object(p, o);
                draw_shadow(p, o);
            }
            draw_attached(p, o, false);
        }
        w = o.next(p.b).0;
        if (w as i16) >= 0 {
            return;
        }
    }
}

/// Attached objects behind (negative Y) or in front of (positive Y) the visible parent.
fn draw_attached(p: &mut Painter, parent: Obj, behind: bool) {
    for field in [Obj::ATTACHED_A, Obj::ATTACHED_B] {
        let w = parent.u16(p.b, field);
        if (w as i16) >= 0 {
            continue;
        }
        let a = Link(w).target();
        let is_behind = (a.u16(p.b, Obj::Y) as i16) < 0;
        if is_behind == behind && parent.flags(p.b) & 1 != 0 {
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
    fn piece(&self, b: &mut World, i: u32) -> PieceWords {
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

fn frame_at(b: &mut World, f: u32) -> Frame {
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

fn style(b: &mut World, o: Obj) -> Style {
    Style { flip: o.flip(b) != 0, palette: (o.own_palette(b) & 1 != 0).then(|| o.palette(b)) }
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
fn draw_object(p: &mut Painter, o: Obj) {
    if o.active(p.b) == 0 {
        return;
    }
    let f = o.frame(p.b);
    if f == 0 {
        return;
    }
    let fr = frame_at(p.b, f);
    if !p.spend(fr.count) {
        return;
    }
    let x = screen_x(p, o);
    let y = (!screen_height(p, o)).wrapping_add(0x101);
    let bits = position_bits(fr.bank, o.priority(p.b));
    let st = style(p.b, o);
    emit_frame(p, &fr, x, y, bits, st, true);
}

/// 0x1e0b4: attached object, positioned relative to its parent (priority 4).
fn draw_child(p: &mut Painter, parent: Obj, a: Obj) {
    if a.active(p.b) == 0 {
        return;
    }
    let f = a.frame(p.b);
    let fr = frame_at(p.b, f);
    if !p.spend(fr.count) {
        return;
    }
    let x = a.u16(p.b, Obj::X).wrapping_add(parent.u16(p.b, Obj::X)).wrapping_sub(p.cam.x);
    let h = a
        .u16(p.b, Obj::Z)
        .wrapping_add(parent.u16(p.b, Obj::Z))
        .wrapping_add(parent.u16(p.b, Obj::Y))
        .wrapping_sub(p.cam.y);
    let y = (!h).wrapping_add(0x101);
    let bits = position_bits(fr.bank, 4);
    let st = style(p.b, a);
    emit_frame(p, &fr, x, y, bits, st, true);
}

/// 0x1ec4e: ground shadow, plus any detailed shadows.
fn draw_shadow(p: &mut Painter, o: Obj) {
    if crate::world::ram::DSW.get(p.b) & 0x200 != 0 || !p.spend(0xb) {
        return;
    }
    let x = screen_x(p, o).wrapping_add(0x38);
    let y = 0x100u16.wrapping_sub(screen_height(p, o)).wrapping_sub(8);
    p.sprite(x & 0x3ff | 0x8000, y & 0x3ff | 0x2000, 0x1890, 0xf);
    // the detailed shadows are the body box (`+0x38`) and the attack box (`+0x3a`) of the object's set
    for (box_index, table) in [(o.body_box(p.b), 0), (o.attack_box(p.b), 2)] {
        if box_index != 0 {
            let set = o.box_set(p.b) as usize;
            let shadow = shadow_box(p.b, table, set, box_index as usize);
            draw_shadow_quad(p, o, shadow);
        }
    }
}

/// Collision box `n` of set `set` of the box table `table` (`sprites/tables.json`: 0 body, 2 attack), or the bytes the
/// original reads at that place when the set is not a set of the table.
fn shadow_box(b: &mut World, table: usize, set: usize, n: usize) -> [u8; 8] {
    let t = &data().sprite_tables;
    if set < t.box_tables[table].sets.len() {
        if let Some(h) = t.hit_box(table, set, n) {
            return h.bytes();
        }
    }
    let at = t.box_tables[table].rom + 4 * set as u32;
    let missing = Missing { table: "collision box", key: at };
    let boxes = missing.read32(b, at);
    std::array::from_fn(|k| missing.read8(b, boxes + 8 * n as u32 + k as u32))
}

/// 0x1ed22: detailed shadow in four quarters around a point (a collision box: x offset, half-width, z offset,
/// half-height, y offset).
fn draw_shadow_quad(p: &mut Painter, o: Obj, e: [u8; 8]) {
    let mut dx = e[0] as i8 as u16;
    if o.flip(p.b) != 0 {
        dx = dx.wrapping_neg();
    }
    let cx = screen_x(p, o).wrapping_add(dx);
    let dy = e[2].wrapping_add(e[4]) as i8 as u16;
    let cy = screen_height(p, o).wrapping_add(dy);
    let (w, h) = (e[1] as u16, e[3] as u16);
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
    let four = crate::world::ram::CABINET.get(p.b) & 2 != 0;
    // positions of the markers (`data/scenes.json`: two and four players)
    let table = crate::world::tables::engine().get(if four { "markers_alt" } else { "markers" });
    let shown = var::PLAYER_MARKERS.get(p.b);
    // (high tile, low tile) and palette of each player
    let looks = [(0x1fba, 0x1fbc, 0x0c), (0x1fc0, 0x1fc2, 0x0c), (0x1fba, 0x1fbc, 0x0d), (0x1fc0, 0x1fc2, 0x0d)];
    for (k, (top, bottom, pal)) in looks.into_iter().enumerate() {
        if shown & (1 << k) != 0 {
            let x = table.at16(p.b, 4 * k as u32);
            let y = table.at16(p.b, 4 * k as u32 + 2);
            p.sprite(x, y, top, 0x100 | pal);
            p.sprite(x.wrapping_sub(0x20), y.wrapping_add(0x10), bottom, 0x300 | pal);
        }
    }
}

/// 0x1eb90 / 0x1ec0c / 0x1ec2c: character string displayed in 8-pixel sprites.
fn draw_text(p: &mut Painter, slot: u32) {
    let (string, slot_x, slot_y) = text_slot(slot);
    let s = string.get(p.b);
    if s == 0 {
        return;
    }
    let mut x = slot_x.get(p.b).wrapping_sub(0x4000);
    let mut y = slot_y.get(p.b).wrapping_add(0x2000);
    let mut color = 0xf;
    p.d4 = 0xf;
    let words = crate::world::ram::CABINET_PLAYERS.get(p.b) == 2;
    // the strings (`sprites/texts@…` of `data/engine.json`, or in RAM)
    let byte = |b: &mut World, a: u32| crate::world::tables::engine().byte(b, a);
    let word = |b: &mut World, a: u32| crate::world::tables::engine().word(b, a);
    let mut at = s;
    loop {
        if words {
            let w = word(p.b, at);
            at += 2;
            match w {
                0 => return,
                0xffff => {
                    x = x.wrapping_add(word(p.b, at));
                    y = y.wrapping_add(word(p.b, at + 2));
                    at += 4;
                }
                0xfffe => {
                    color = word(p.b, at);
                    p.set_d4_low(color);
                    at += 2;
                }
                code => {
                    p.sprite(x, y, code, color);
                    x = x.wrapping_add(8);
                }
            }
        } else {
            let c = byte(p.b, at);
            at += 1;
            match c {
                0xff => {
                    x = x.wrapping_sub(8 * byte(p.b, at) as u16);
                    y = y.wrapping_add(byte(p.b, at + 1) as i8 as u16);
                    at += 2;
                }
                0xfe => {
                    color = byte(p.b, at) as u16;
                    p.d4 = color as u32;
                    at += 1;
                }
                0xfd => return,
                _ => {
                    let code = if c & 0xf0 == 0 {
                        let lo = byte(p.b, at);
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
pub fn clear_lists(b: &mut World) {
    var::ACTORS_LONG.set(b, 0);
    var::FOREGROUND_LONG.set(b, 0);
    var::PLAYER_MARKERS.set(b, 0);
    for text in var::TEXTS {
        Long(text).set(b, 0);
    }
}

