//! Collision of an object with the terrain: 0x33fe4 (players, enemies, shadows...) and its variant 0x3403c.
//!
//! The terrain attribute under the object (`video::terrain`, 0x24b0) indexes a table of types in ROM
//! (16-byte entries). A type of kind 0 can be crossed; otherwise it describes an obstacle whose
//! profile depends on the 4 low bits of the position within the cell (D0 = x, D1 = y): the object is
//! pushed out of the obstacle (`Obj::X` or `Obj::Y` corrected) and D5 is 1. For a solid obstacle
//! (kind 1) the routine restarts as long as some remains (unless the `STOP` field of the type is non-zero); for
//! a slope (kind 2) a single pass.
//!
//! Registers: since the handlers that follow reread D0-D2, D5, A1, A2 and the flags, the routine
//! is transcribed instruction by instruction (A1 = address of the original subroutine, which the
//! 68000 reached through a jump table, corrected D1...).

use crate::world::World;

use crate::hw::mem::set_low_word;
use crate::world::objects::Obj;
use crate::world::ops::{Alu, Registers, Size};
use crate::world::ram::{Long, Word};
use crate::{world::ops, Native};
use crate::world::data::data;
use crate::world::tables::engine;
use crate::hw::video::Cell;

/// Switch bit (`ram::DSW`): terrain collision active.
const ENABLED: u16 = 0x800;
/// Last object tested (A0), its x position and its y position (both as a long): consulted by other routines.
const LAST_OBJECT: Word = Word::a5(0x64);
const LAST_X: Word = Word::a5(0x60);
const LAST_Y: Word = Word::a5(0x62);
const LAST_POSITION: Long = Long::a5(0x60);

/// Fields of a terrain type (16-byte entry, words).
mod kind {
    /// 0: can be crossed; 1: solid obstacle; more: slope.
    pub const CATEGORY: u32 = 0;
    /// Obstacle subroutine (0, 4, 8 or 12).
    pub const ROUTINE: u32 = 2;
    /// Slope: direction (0 or other).
    pub const FLIP: u32 = 4;
    /// Slope: height offset.
    pub const SHIFT: u32 = 6;
    /// Depth (slope: base height) of the obstacle's edge.
    pub const EDGE: u32 = 8;
    /// Solid obstacle: non-zero, a single pass.
    pub const STOP: u32 = 10;
}

/// The two copies of the routine (`Variant`): only the type table (`terrain.json`, `kinds`) and the subroutine
/// addresses (visible in A1) change.
#[derive(Clone, Copy)]
pub(crate) struct Variant {
    entry: u32,
    /// The second copy sets D5 before knowing whether the object touches the solid obstacle.
    early_flag: bool,
    /// The table of terrain types (index in `terrain.json`, `kinds`).
    types: usize,
    solid: [u32; 4],
}

pub(crate) const FIRST: Variant = Variant { entry: 0x33fe4, early_flag: false, types: 0, solid: [0x340b6, 0x340d4, 0x340f2, 0x34108] };
pub(crate) const SECOND: Variant = Variant { entry: 0x3403c, early_flag: true, types: 1, solid: [0x34120, 0x3413e, 0x3415c, 0x34172] };

/// Slope subroutines, common to both copies.
const SLOPES: [u32; 4] = [0x341a4, 0x341d0, 0x341fe, 0x3422e];

/// The transcription of 0x33fe4 / 0x3403c on the registers of the original (`Alu`): the game keeps the leftovers
/// D2-D5 and the Z flag that follow it, the tools all the registers and flags.
pub(crate) struct Push<'a, R: Alu> {
    pub(crate) c: &'a mut R,
    pub(crate) b: &'a mut World,
    pub(crate) variant: Variant,
}

impl<R: Alu> Push<'_, R> {
    fn d(&self, n: usize) -> u16 {
        self.c.data(n) as u16
    }
    fn set_d(&mut self, n: usize, v: u16) {
        set_low_word(&mut self.c.d()[n], v);
    }
    /// Word `off` of the terrain type at `a2` (the tables of `terrain.json`, read by address: a terrain number past the
    /// first table reads the second one, as the original does).
    fn field(&mut self, a2: u32, off: u32) -> u16 {
        crate::world::tables::engine().word(self.b, a2 + off)
    }
    /// `sub.w src,Dn`: flags of the original; returns the carry.
    fn sub_d(&mut self, n: usize, src: u16) -> bool {
        let r = self.c.sub(src as u32, self.d(n) as u32, Size::Word);
        self.set_d(n, r as u16);
        self.c.carry()
    }
    /// `add.w src,(off,A0)` / `sub.w src,(off,A0)`.
    fn add_obj(&mut self, sub: bool, src: u16, off: u32) {
        let o = Obj(self.c.a()[0]);
        let v = o.u16(self.b, off) as u32;
        let r = if sub { self.c.sub(src as u32, v, Size::Word) } else { self.c.add(src as u32, v, Size::Word) };
        o.set_u16(self.b, off, r as u16);
    }
    /// `tst.w (off,A2)`: true if zero.
    fn zero(&mut self, a2: u32, off: u32) -> bool {
        let v = self.field(a2, off);
        self.c.logic(v as u32, Size::Word);
        v == 0
    }
    /// `moveq #1,D5`.
    fn collided(&mut self) {
        self.c.d()[5] = 1;
    }

    /// Repeats the passes as long as a solid obstacle restarts the routine (`beq entry`).
    pub(crate) fn run(&mut self) {
        while self.pass() {}
    }

    /// One pass; true to start over.
    fn pass(&mut self) -> bool {
        let a0 = self.c.a()[0];
        let o = Obj(a0);
        if self.variant.entry == FIRST.entry {
            LAST_OBJECT.set(self.b, a0 as u16);
            let (x, y) = (o.u16(self.b, Obj::X), o.u16(self.b, Obj::Y));
            LAST_X.set(self.b, x);
            LAST_Y.set(self.b, y);
        }
        let enabled = crate::world::ram::DSW.get(self.b) & ENABLED;
        self.set_d(0, enabled);
        self.c.logic(enabled as u32, Size::Word);
        if enabled == 0 {
            return false;
        }
        let (x, y) = (o.u16(self.b, Obj::X), o.u16(self.b, Obj::Y));
        self.set_d(0, x);
        self.set_d(1, y);
        let t = crate::world::terrain::terrain(self.b, x, y);
        crate::world::terrain::terrain_regs(self.c, &t);
        let number = self.c.d()[2] as u8;
        o.set_terrain(self.b, number);
        self.c.logic(number as u32, Size::Byte);
        if number == 0 {
            return self.free();
        }
        // lsl.w #4,D2w: entry of the type table
        let d2 = self.d(2);
        self.set_d(2, d2 << 4);
        let carry = d2 >> 12 & 1 != 0;
        self.c.logic((d2 << 4) as u32, Size::Word);
        self.c.flags(ops::C | ops::X, if carry { ops::C | ops::X } else { 0 });
        let types = crate::world::data::data().terrain.kinds[self.variant.types].rom;
        let a2 = types.wrapping_add((d2 << 4) as i16 as u32);
        self.c.a()[2] = a2;
        let category = self.field(a2, kind::CATEGORY);
        self.c.cmp(1, category as u32, Size::Word);
        match category {
            0 => self.free(),
            1 => self.solid(a2),
            _ => {
                self.slope(a2);
                false
            }
        }
    }

    /// Nothing to correct: the object is free (0x3402c).
    fn free(&mut self) -> bool {
        let o = Obj(self.c.a()[0]);
        LAST_OBJECT.set(self.b, 0);
        LAST_POSITION.set(self.b, 0);
        self.c.d()[2] = 0;
        o.set_terrain(self.b, 0);
        self.c.logic(0, Size::Byte);
        false
    }

    /// Solid obstacle: pushes the object out of the cell; true to start over.
    fn solid(&mut self, a2: u32) -> bool {
        let routine = self.field(a2, kind::ROUTINE);
        self.set_d(2, routine);
        self.c.a()[1] = self.variant.solid[routine as usize / 4];
        let edge = self.field(a2, kind::EDGE);
        match routine {
            // below / above the edge: corrects y
            0 | 4 => {
                if self.variant.early_flag {
                    self.collided();
                }
                let y = if routine == 0 { self.d(1) } else { !self.d(1) };
                self.set_d(1, y & 0xf);
                if !self.sub_d(1, edge) {
                    return self.free();
                }
                self.collided();
                if routine == 0 {
                    self.set_d(1, self.d(1).wrapping_neg());
                }
                self.add_obj(false, self.d(1), Obj::Y);
            }
            // right / left of the edge: corrects x
            _ => {
                let x = if routine == 8 { self.d(0) } else { !self.d(0) };
                self.set_d(0, (x & 0xf) + 1);
                self.collided();
                self.add_obj(routine == 8, self.d(0), Obj::X);
            }
        }
        self.zero(a2, kind::STOP)
    }

    /// Slope (kind > 1): a single pass.
    fn slope(&mut self, a2: u32) {
        let routine = self.field(a2, kind::ROUTINE);
        self.set_d(2, routine);
        self.c.a()[1] = SLOPES[routine as usize / 4];
        let flipped = self.field(a2, kind::FLIP) != 0;
        let (edge, shift) = (self.field(a2, kind::EDGE), self.field(a2, kind::SHIFT));
        match routine {
            // the edge is a ramp in x: height above (0) or below (4) y
            0 | 4 => {
                let x = if flipped == (routine == 0) { !self.d(0) } else { self.d(0) };
                let height = ((x & 0xf) as u32 >> (shift & 63).min(31)) as u16;
                self.set_d(0, height.wrapping_add(edge).wrapping_add(1));
                self.set_d(2, shift);
                let y = if routine == 0 { self.d(1) } else { !self.d(1) };
                self.set_d(1, y & 0xf);
                if self.sub_d(0, self.d(1)) {
                    {
                    self.free();
                    return;
                }
                }
                self.collided();
                self.add_obj(routine == 4, self.d(0), Obj::Y);
            }
            // the edge is a ramp in y: width to the left (8) or right (12) of x
            _ => {
                let y = if flipped == (routine == 12) { !self.d(1) } else { self.d(1) };
                self.set_d(1, y & 0xf);
                if self.sub_d(1, edge) {
                    {
                    self.free();
                    return;
                }
                }
                self.set_d(2, shift);
                self.set_d(1, ((self.d(1) as u32) << (shift & 63).min(31)) as u16);
                let x = if routine == 8 { !self.d(0) } else { self.d(0) };
                self.set_d(0, x & 0xf);
                if self.sub_d(1, self.d(0)) {
                    {
                    self.free();
                    return;
                }
                }
                self.collided();
                self.add_obj(routine == 8, self.d(1), Obj::X);
            }
        }
    }
}

pub fn push(b: &mut World, o: Obj) -> bool {
    push_collides(b, o).0
}

/// `push`, and true second if the object hit an obstacle (the original's D5 = 1, which callers reread).
pub fn push_collides(b: &mut World, o: Obj) -> (bool, bool) {
    let mut c = Registers::default();
    c.a[0] = o.0;
    Push { c: &mut c, b, variant: FIRST }.run();
    (c.zero(), c.d[5] == 1)
}

/// `push`, with the leftovers of the original that the following handlers reread (`dispatch::Leftovers`): D2 and D3
/// (cell and terrain gap, original glitch: speeds of the spark of `flash_and_shake`, bounce setting of the
/// frost shard) and D5 (1 after a collision). The leftovers go in and out unchanged when the routine does not write
/// them. Returns the original's Z flag.
pub fn push_leftovers(b: &mut World, o: Obj, d2: &mut u32, d3: &mut u32, d5: &mut u32) -> bool {
    let mut c = Registers::default();
    c.a[0] = o.0;
    (c.d[2], c.d[3], c.d[5]) = (*d2, *d3, *d5);
    Push { c: &mut c, b, variant: FIRST }.run();
    (*d2, *d3, *d5) = (c.d[2], c.d[3], c.d[5]);
    c.zero()
}

/// 0x3403c (the second copy of the routine) on object `o`, without registers; `leftovers` are the D2-D5 leftovers
/// (`dispatch::Leftovers`) passed to it and left by it. Returns the original's Z flag.
pub fn push_second_leftovers(b: &mut World, o: Obj, leftovers: [&mut u32; 4]) -> bool {
    let mut c = Registers::default();
    c.a[0] = o.0;
    for (k, r) in leftovers.iter().enumerate() {
        c.d[2 + k] = **r;
    }
    Push { c: &mut c, b, variant: SECOND }.run();
    for (k, r) in leftovers.into_iter().enumerate() {
        *r = c.d[2 + k];
    }
    c.zero()
}

pub const NATIVES: &[Native] = &[];

// ---------------------------------------------------------------------------------------------
// V2: terrain attribute under a point
// ---------------------------------------------------------------------------------------------

/// Terrain attribute under a point, and the computation leftovers that the original leaves in its registers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Terrain {
    /// Terrain number: bits 7 to 15 of the cell's attribute word (D2.w; 0: free ground).
    pub number: u16,
    /// Corrected coordinates (D0.w, D1.w): y shifted by -/+ 0x10 on the fixed maps, camera 2 otherwise.
    pub x: u16,
    pub y: u16,
    /// Address of the cell (A1): fixed map in ROM, or scroll layer 2 in GFX RAM.
    pub case: u32,
    /// Index of the cell in the map (D3.w, whole D3 on the fixed maps) and column (D4.w).
    pub index: u16,
    pub column: u16,
    /// Fixed map of scenes 2 and 0x14 (D2 is then only written as a low word; elsewhere D2 = 0 first).
    pub fixed_map: bool,
}

/// Terrain number under point (x, y) (0x24b0). Scenes 2 and 0x14: fixed map in ROM (y shifted by -/+ 0x10);
/// elsewhere: cell of scroll layer 2 in GFX RAM, with the camera correction when camera 2 is
/// current.
pub fn terrain(b: &mut World, x: u16, y: u16) -> Terrain {
    let stage = crate::world::ram::SCENE.get(b);
    let (mut x, mut y) = (x, y);
    // the fixed maps of `terrain.json` (scenes 2 and 0x14), with their correction of y
    let fixed_map = data().terrain.maps.iter().find(|m| m.stage == stage).map(|m| (m.rom, m.dy as u16));
    let (word, case, index, column) = if let Some((map, dy)) = fixed_map {
        y = y.wrapping_add(dy);
        let row = 0x1e - ((y as u8 >> 3) & 0x1e) as u16;
        let column = x.wrapping_add(x) & 0xffe0;
        let index = row.wrapping_add(column);
        let case = map.wrapping_add(index as i16 as u32);
        (engine().word(b, case), case, index, column)
    } else {
        use crate::world::ram::{CAMERA_ALT_ACTIVE, CAMERA_ALT_X, CAMERA_ALT_Y, CAMERA_X, CAMERA_Y};
        if CAMERA_ALT_ACTIVE.is_set(b) {
            x = x.wrapping_add(CAMERA_X.get(b).wrapping_sub(CAMERA_ALT_X.get(b)));
            y = y.wrapping_add(CAMERA_Y.get(b).wrapping_sub(CAMERA_ALT_Y.get(b)));
        }
        let page = 0x3000u16.wrapping_sub((y & 0x300) << 4);
        let row = 0x3c - ((y as u8 >> 2) & 0x3c) as u16;
        let column = x.wrapping_shl(2) & 0xfc0;
        let index = page.wrapping_add(row).wrapping_add(column);
        let case = crate::hw::video::SCROLL2.0.wrapping_add(index as i16 as u32);
        (Cell(case).attr(b), case, index, column)
    };
    Terrain { number: (word & 0xff80) >> 7, x, y, case, index, column, fixed_map: fixed_map.is_some() }
}

/// Registers left by 0x24b0: D0/D1 (corrected coordinates), D2 (number), D3/D4 (computation leftovers, read by
/// callers that take them as arguments), A1 (cell) and Z (zero number, the other flags cleared).
pub(crate) fn terrain_regs(r: &mut impl crate::world::ops::Alu, t: &Terrain) {
    if t.fixed_map {
        r.d()[3] = t.index as u32;
    } else {
        set_low_word(&mut r.d()[3], t.index);
        r.d()[2] = 0;
    }
    set_low_word(&mut r.d()[4], t.column);
    r.a()[1] = t.case;
    set_low_word(&mut r.d()[0], t.x);
    set_low_word(&mut r.d()[1], t.y);
    set_low_word(&mut r.d()[2], t.number);
    r.flags(crate::world::ops::ALL, if t.number == 0 { crate::world::ops::Z } else { 0 });
}

/// Terrain number under object `o` (0x24a8: x position at +8, y at +0x10).
pub fn terrain_of(b: &mut World, o: crate::world::objects::Obj) -> Terrain {
    use crate::world::objects::Obj;
    let (x, y) = (o.u16(b, Obj::X), o.u16(b, Obj::Y));
    terrain(b, x, y)
}

