//! Object lists sorted by depth (drawing order).
//!
//! Two intrusive doubly-linked lists contain the objects to draw: the *actors*
//! (characters, enemies, scenery) and the *foreground*. The links are the words `Obj::PREV` (toward
//! the head) and `Obj::NEXT` (toward the tail); the list is sorted by decreasing `Obj::Y` from the head
//! to the tail. An empty list has null head and tail; the list ends carry the end marker
//! of their list (1: actors, 2: foreground).
//!
//! Each object handler ends with `reinsert`: the object is inserted if it is in no list,
//! otherwise moved only if its depth no longer respects the order.
//!
//! Trap of the original game: handlers often end with `jmp 0x1ee1a`, and the dispatch
//! moves on to the next object without resetting the registers. Some handlers then read a
//! register left by the previous one: 0x4bd32 does `andi.b #$f,D0` then indexes a table with all of
//! D0.w, whose high byte is the list link left by `reinsert`. The natives 0x1ee1a and
//! 0x1ef6c (`Reinsert`) therefore transcribe the original instruction by instruction and return
//! D0-D2, A1-A4 and the flags as it does, depending on the path followed (`SortedList::reinsert_68k`). The
//! callers without this contract use `SortedList::reinsert`, the same transcription without registers.

use cps2::Cps2Bus;
use m68k::Cpu;

use super::{Link, Obj};
use crate::mem::{rd16, rd32, rts, set_low_word, wr16, A5};
use crate::{ops, regs, Native};
use m68k::decode::Size;

#[derive(Clone, Copy, Debug)]
pub struct SortedList {
    head: u32,
    tail: u32,
    end: Link,
}

/// Characters, enemies, scenery.
pub const ACTORS: SortedList = SortedList { head: super::ram::ACTORS_HEAD, tail: super::ram::ACTORS_TAIL, end: Link(1) };
/// Foreground.
pub const FOREGROUND: SortedList =
    SortedList { head: super::ram::FOREGROUND_HEAD, tail: super::ram::FOREGROUND_TAIL, end: Link(2) };

fn is_sentinel_or_none(l: Link) -> bool {
    l.0 as i16 >= 0
}

impl SortedList {
    fn head(self, b: &mut Cps2Bus) -> Link {
        Link(rd16(b, A5 + self.head))
    }
    fn set_head(self, b: &mut Cps2Bus, l: Link) {
        wr16(b, A5 + self.head, l.0)
    }
    fn set_tail(self, b: &mut Cps2Bus, l: Link) {
        wr16(b, A5 + self.tail, l.0)
    }

    /// Inserts `obj` if it is in no list, otherwise puts it back if the order is broken.
    ///
    /// The writes are those of 0x1ee1a, in the same order (like `Reinsert`, without registers): on an
    /// inconsistent list (overlapping links), the original and this function leave the same memory, which the
    /// plan-based version (`plan_reinsert`, `apply`) did not guarantee (discrepancy found by `fuzz_couv` on ESHL 44).
    pub fn reinsert(self, b: &mut Cps2Bus, obj: Obj) {
        let a0 = obj.0;
        let (prev, next) = (rd16(b, a0 + Obj::PREV), rd16(b, a0 + Obj::NEXT));
        if prev.wrapping_add(next) == 0 {
            return self.insert_exact(b, obj);
        }
        if self.relocate_exact(b, obj, Way::toward_head(self)) {
            return;
        }
        self.relocate_exact(b, obj, Way::toward_tail(self));
    }

    /// `Reinsert::relocate` without registers.
    fn relocate_exact(self, b: &mut Cps2Bus, obj: Obj, w: Way) -> bool {
        let a0 = obj.0;
        let link = rd16(b, a0 + w.ahead);
        if (link as i16) >= 0 {
            return false;
        }
        let mut a1 = link as i16 as i32 as u32;
        let y = rd32(b, a0 + Obj::Y);
        if w.settled(y, rd32(b, a1.wrapping_add(Obj::Y))) {
            return false;
        }
        let mut d0;
        loop {
            d0 = rd16(b, a1.wrapping_add(w.ahead));
            if (d0 as i16) >= 0 {
                wr16(b, A5 + w.reach, a0 as u16);
                break;
            }
            let a2 = d0 as i16 as i32 as u32;
            if w.settled(y, rd32(b, a2.wrapping_add(Obj::Y))) {
                break;
            }
            a1 = a2;
        }
        let a3 = rd16(b, a0 + w.ahead) as i16 as i32 as u32;
        let d2 = rd16(b, a0 + w.behind);
        if (d2 as i16) < 0 {
            wr16(b, (d2 as i16 as i32 as u32).wrapping_add(w.ahead), a3 as u16);
        }
        wr16(b, a3.wrapping_add(w.behind), d2);
        if (d2 as i16) >= 0 {
            wr16(b, A5 + w.far, a3 as u16);
        }
        wr16(b, a0 + w.ahead, d0);
        if (d0 as i16) < 0 {
            wr16(b, (d0 as i16 as i32 as u32).wrapping_add(w.behind), a0 as u16);
        }
        wr16(b, a1.wrapping_add(w.ahead), a0 as u16);
        wr16(b, a0 + w.behind, a1 as u16);
        true
    }

    /// `Reinsert::insert` without registers (0x1eef2).
    fn insert_exact(self, b: &mut Cps2Bus, obj: Obj) {
        let a0 = obj.0;
        let y = rd32(b, a0 + Obj::Y);
        let tail = rd16(b, A5 + self.tail);
        if tail == 0 {
            wr16(b, a0 + Obj::PREV, self.end.0);
            wr16(b, a0 + Obj::NEXT, self.end.0);
            wr16(b, A5 + self.head, a0 as u16);
            wr16(b, A5 + self.tail, a0 as u16);
            return;
        }
        let mut a1 = tail as i16 as i32 as u32;
        if y as i32 <= rd32(b, a1.wrapping_add(Obj::Y)) as i32 {
            wr16(b, a1.wrapping_add(Obj::NEXT), a0 as u16);
            wr16(b, a0 + Obj::PREV, a1 as u16);
            wr16(b, a0 + Obj::NEXT, self.end.0);
            wr16(b, A5 + self.tail, a0 as u16);
            return;
        }
        loop {
            let before = rd16(b, a1.wrapping_add(Obj::PREV));
            if (before as i16) >= 0 {
                wr16(b, a1.wrapping_add(Obj::PREV), a0 as u16);
                wr16(b, a0 + Obj::NEXT, a1 as u16);
                wr16(b, a0 + Obj::PREV, self.end.0);
                wr16(b, A5 + self.head, a0 as u16);
                return;
            }
            a1 = before as i16 as i32 as u32;
            if y as i32 <= rd32(b, a1.wrapping_add(Obj::Y)) as i32 {
                break;
            }
        }
        let a2 = rd16(b, a1.wrapping_add(Obj::NEXT)) as i16 as i32 as u32;
        wr16(b, a1.wrapping_add(Obj::NEXT), a0 as u16);
        wr16(b, a2.wrapping_add(Obj::PREV), a0 as u16);
        wr16(b, a0 + Obj::PREV, a1 as u16);
        wr16(b, a0 + Obj::NEXT, a2 as u16);
    }
}

/// Cases of removing an object from its list.
#[derive(Clone, Copy, Debug)]
pub(super) enum Removal {
    /// Not in a list.
    NotListed,
    /// Between two objects.
    Middle { prev: Obj, next: Obj },
    /// Last of its list (the tail becomes `prev`).
    Last { prev: Obj, list: SortedList },
    /// First of its list (the head becomes `next`).
    First { next: Obj, list: SortedList },
    /// Alone in its list.
    Only { list: SortedList },
}

/// Analyzes the removal of `obj` (without modifying anything), like `0x1f0be`.
fn plan_removal(b: &mut Cps2Bus, obj: Obj) -> Removal {
    let (prev, next) = (obj.prev(b), obj.next(b));
    if prev.is_none() {
        return Removal::NotListed;
    }
    if !is_sentinel_or_none(prev) {
        // an object precedes
        let p = prev.target();
        if !is_sentinel_or_none(next) {
            Removal::Middle { prev: p, next: next.target() }
        } else {
            let list = list_ending_with_mark(next);
            Removal::Last { prev: p, list }
        }
    } else if !is_sentinel_or_none(next) {
        let list = list_ending_with_mark(prev);
        Removal::First { next: next.target(), list }
    } else {
        let list = list_ending_with_mark(next);
        Removal::Only { list }
    }
}

/// The list whose end marker is `end` (`bgt` of the original: > 1, foreground).
fn list_ending_with_mark(end: Link) -> SortedList {
    if end.0 as i16 > 1 { FOREGROUND } else { ACTORS }
}

/// Removes `obj` from its list (if it is in one) and resets its links to zero: `0x1f0be`.
pub fn unlink(b: &mut Cps2Bus, obj: Obj) {
    let removal = plan_removal(b, obj);
    apply_removal(b, obj, removal);
}

fn apply_removal(b: &mut Cps2Bus, obj: Obj, removal: Removal) {
    match removal {
        Removal::NotListed => return,
        Removal::Middle { prev, next } => {
            let (p, n) = (obj.prev(b), obj.next(b));
            next.set_prev(b, p);
            prev.set_next(b, n);
        }
        Removal::Last { prev, list } => {
            list.set_tail(b, Link::to(prev));
            prev.set_next(b, list.end);
        }
        Removal::First { next, list } => {
            list.set_head(b, Link::to(next));
            next.set_prev(b, list.end);
        }
        Removal::Only { list } => {
            list.set_head(b, Link::NONE);
            list.set_tail(b, Link::NONE);
        }
    }
    obj.set_prev(b, Link::NONE);
    obj.set_next(b, Link::NONE);
}

/// Removes from their lists all the actor objects then, if the actors were not empty,
/// those of the foreground: `0x1f184`. Returns the last object removed.
pub fn unlink_all(b: &mut Cps2Bus) -> Option<Obj> {
    let mut last = None;
    for list in [ACTORS, FOREGROUND] {
        let head = list.head(b);
        if head.is_none() {
            break;
        }
        let mut cur = head.target();
        loop {
            let next = cur.next(b);
            unlink(b, cur);
            last = Some(cur);
            if is_sentinel_or_none(next) {
                break;
            }
            cur = next.target();
        }
    }
    last
}

// ---- adaptation to the 68000 registers ----

/// Direction in which a misplaced object looks for its new place.
#[derive(Clone, Copy)]
struct Way {
    /// Link followed to search (`Obj::PREV` toward the head, `Obj::NEXT` toward the tail) and opposite link.
    ahead: u32,
    behind: u32,
    /// List end reached by following `ahead`, and opposite end (addresses relative to A5).
    reach: u32,
    far: u32,
    /// Toward the head (decreasing depths at the head): stop as soon as `y <= neighbor`, otherwise `y >= neighbor`.
    toward_head: bool,
}

impl Way {
    fn toward_head(list: SortedList) -> Way {
        Way { ahead: Obj::PREV, behind: Obj::NEXT, reach: list.head, far: list.tail, toward_head: true }
    }
    fn toward_tail(list: SortedList) -> Way {
        Way { ahead: Obj::NEXT, behind: Obj::PREV, reach: list.tail, far: list.head, toward_head: false }
    }
    /// Is the object of depth `y` already on the right side of `neighbour`?
    fn settled(self, y: u32, neighbour: u32) -> bool {
        if self.toward_head { y as i32 <= neighbour as i32 } else { y as i32 >= neighbour as i32 }
    }
}

/// Reinsertion of A0 in `list` (0x1ee1a, 0x1ef6c), instruction by instruction: the following object
/// handlers re-read D0, D1, D2, A1-A4 and the flags as the original leaves them
/// (see the module header), depending on the path followed. The `move.w` to memory set N and Z
/// (`store`), the `movea.w` sign-extend and touch no flag.
struct Reinsert<'a> {
    c: &'a mut Cpu,
    b: &'a mut Cps2Bus,
    /// The reinserted object (A0 of the original).
    obj: Obj,
    list: SortedList,
}

impl Reinsert<'_> {
    fn a0(&self) -> u32 {
        self.obj.0
    }
    fn a0w(&self) -> u16 {
        self.obj.0 as u16
    }
    /// `movea.w src,An`.
    fn movea(&mut self, n: usize, w: u16) -> u32 {
        self.c.a[n] = w as i16 as i32 as u32;
        self.c.a[n]
    }
    /// `movea.w (off,Am),An`.
    fn movea_mem(&mut self, n: usize, base: u32, off: u32) -> u32 {
        let w = rd16(self.b, base.wrapping_add(off));
        self.movea(n, w)
    }
    /// `move.w (off,An),Dn`: low word of Dn and flags.
    fn load(&mut self, d: usize, base: u32, off: u32) -> u16 {
        let v = rd16(self.b, base.wrapping_add(off));
        set_low_word(&mut self.c.d[d], v);
        ops::logic(self.c, v as u32, Size::Word);
        v
    }
    /// `move.w src,(off,An)`: writes and sets the flags.
    fn store(&mut self, base: u32, off: u32, v: u16) {
        wr16(self.b, base.wrapping_add(off), v);
        ops::logic(self.c, v as u32, Size::Word);
    }
    /// `cmp.l (Y,An),Dn`: flags; returns the depth of the neighbor.
    fn compare_depth(&mut self, d: usize, an: u32) -> u32 {
        let y = rd32(self.b, an.wrapping_add(Obj::Y));
        let dn = self.c.d[d];
        ops::cmp(self.c, y, dn, Size::Long);
        y
    }

    fn run(&mut self) {
        let a0 = self.a0();
        let prev = self.load(0, a0, Obj::PREV);
        let sum = ops::add(self.c, rd16(self.b, a0 + Obj::NEXT) as u32, prev as u32, Size::Word);
        set_low_word(&mut self.c.d[0], sum as u16);
        if sum as u16 == 0 {
            return self.insert();
        }
        if self.relocate(Way::toward_head(self.list)) {
            return;
        }
        self.relocate(Way::toward_tail(self.list));
    }

    /// The object is already in the list: moves it if it is on the wrong side of a neighbor in the direction
    /// `w`. Returns true when the routine is done (moved, or nothing to do at the end of the list).
    fn relocate(&mut self, w: Way) -> bool {
        let a0 = self.a0();
        let link = self.load(0, a0, w.ahead);
        if (link as i16) >= 0 {
            return false;
        }
        let mut a1 = self.movea(1, link);
        self.c.d[1] = rd32(self.b, a0 + Obj::Y);
        let y1 = self.compare_depth(1, a1);
        if w.settled(self.c.d[1], y1) {
            return false;
        }
        // searches for the last neighbor that the object still has to pass
        loop {
            let link = self.load(0, a1, w.ahead);
            if (link as i16) >= 0 {
                self.store(A5, w.reach, self.a0w());
                break;
            }
            let a2 = self.movea(2, link);
            let y2 = self.compare_depth(1, a2);
            if w.settled(self.c.d[1], y2) {
                break;
            }
            a1 = self.movea(1, link);
        }
        // removal: the neighbor `behind` and the one `ahead` are linked together
        let a3 = self.movea_mem(3, a0, w.ahead);
        let d2 = self.load(2, a0, w.behind);
        if (d2 as i16) < 0 {
            let a4 = self.movea(4, d2);
            self.store(a4, w.ahead, a3 as u16);
        }
        self.store(a3, w.behind, d2);
        if (d2 as i16) >= 0 {
            self.store(A5, w.far, a3 as u16);
        }
        // insertion in front of A1 (in the `ahead` direction)
        let d0 = self.c.d[0] as u16;
        self.store(a0, w.ahead, d0);
        if (d0 as i16) < 0 {
            let a2 = self.movea(2, d0);
            self.store(a2, w.behind, self.a0w());
        }
        self.store(a1, w.ahead, self.a0w());
        self.store(a0, w.behind, a1 as u16);
        true
    }

    /// The object is in no list: insertion starting from the tail (0x1eef2).
    fn insert(&mut self) {
        let (a0, list) = (self.a0(), self.list);
        let y = rd32(self.b, a0 + Obj::Y);
        self.c.d[0] = y;
        let tail = self.load(1, A5, list.tail);
        if tail == 0 {
            self.store(a0, Obj::PREV, list.end.0);
            self.store(a0, Obj::NEXT, list.end.0);
            self.store(A5, list.head, self.a0w());
            self.store(A5, list.tail, self.a0w());
            return;
        }
        let mut a1 = self.movea(1, tail);
        let y1 = self.compare_depth(0, a1);
        if y as i32 <= y1 as i32 {
            // deeper than the whole list: new tail
            self.store(a1, Obj::NEXT, self.a0w());
            self.store(a0, Obj::PREV, a1 as u16);
            self.store(a0, Obj::NEXT, list.end.0);
            self.store(A5, list.tail, self.a0w());
            return;
        }
        loop {
            let before = self.load(1, a1, Obj::PREV);
            if (before as i16) >= 0 {
                // becomes the new head
                self.store(a1, Obj::PREV, self.a0w());
                self.store(a0, Obj::NEXT, a1 as u16);
                self.store(a0, Obj::PREV, list.end.0);
                self.store(A5, list.head, self.a0w());
                return;
            }
            a1 = self.movea(1, before);
            let y1 = self.compare_depth(0, a1);
            if y as i32 <= y1 as i32 {
                break;
            }
        }
        // between A1 and its successor
        let a2 = self.movea_mem(2, a1, Obj::NEXT);
        self.store(a1, Obj::NEXT, self.a0w());
        self.store(a2, Obj::PREV, self.a0w());
        self.store(a0, Obj::PREV, a1 as u16);
        self.store(a0, Obj::NEXT, a2 as u16);
    }
}

/// Registers left by the original reinsertion (see `diag::dead_regs`, label `lists`).
pub const REGS_REINSERT: u32 =
    regs::d(0) | regs::d(1) | regs::d(2) | regs::a(1) | regs::a(2) | regs::a(3) | regs::a(4) | regs::CCR;

impl SortedList {
    /// `reinsert`, with the leftover D2 of the original (`dispatch::Leftovers`): when the object is moved, the low
    /// word of D2
    /// receives the link it leaves (`move.w (behind,A0),D2`). Original glitch: a following handler re-reads it (the
    /// bounce of the freeze shard, 0x4bda8). D2 goes in and out unchanged otherwise.
    pub fn reinsert_leftovers(self, b: &mut Cps2Bus, obj: Obj, d2: &mut u32) {
        let mut c = Cpu::default();
        c.d[2] = *d2;
        Reinsert { c: &mut c, b, obj, list: self }.run();
        *d2 = c.d[2];
    }

    /// `reinsert` with the original's registers, depending on the path followed (see `Reinsert`).
    #[track_caller]
    pub fn reinsert_68k(self, c: &mut Cpu, b: &mut Cps2Bus, obj: Obj) {
        Reinsert { c, b, obj, list: self }.run();
        crate::diag::dead_regs(c, "lists", REGS_REINSERT);
    }
}

/// `unlink_all` with the original's register: A0 = last object removed (unchanged if the lists are empty).
#[track_caller]
pub fn unlink_all_68k(c: &mut Cpu, b: &mut Cps2Bus) -> Option<Obj> {
    let last = unlink_all_regs(c, b);
    crate::diag::dead_regs(c, "lists", regs::a(0));
    last
}

fn unlink_all_regs(c: &mut Cpu, b: &mut Cps2Bus) -> Option<Obj> {
    let last = unlink_all(b);
    if let Some(o) = last {
        c.a[0] = o.0 | 0xff00_0000; // movea.w: sign-extended
    }
    last
}

fn reinsert_native(list: SortedList, c: &mut Cpu, b: &mut Cps2Bus) {
    let obj = Obj(c.a[0]);
    Reinsert { c, b, obj, list }.run();
    rts(c, b);
}

/// 0x1f0be: removes object A0 from its list. The original leaves no useful register: the Rust callers
/// call `unlink` directly.
fn unlink_native(c: &mut Cpu, b: &mut Cps2Bus) {
    unlink(b, Obj(c.a[0]));
    rts(c, b);
}

/// 0x1f184: empties both lists.
fn unlink_all_native(c: &mut Cpu, b: &mut Cps2Bus) {
    unlink_all_regs(c, b);
    rts(c, b);
}

pub const REINSERT_ACTORS: Native = Native {
    entry: 0x1ee1a,
    name: "list_reinsert_actors",
    run: crate::Body::WithCpu(|c, b| reinsert_native(ACTORS, c, b)),
    touches_sound: false,
    outputs: regs::ALL,
};

pub const REINSERT_FOREGROUND: Native = Native {
    entry: 0x1ef6c,
    name: "list_reinsert_foreground",
    run: crate::Body::WithCpu(|c, b| reinsert_native(FOREGROUND, c, b)),
    touches_sound: false,
    outputs: regs::ALL,
};

pub const UNLINK: Native = Native {
    entry: 0x1f0be,
    name: "list_unlink",
    run: crate::Body::WithCpu(unlink_native),
    touches_sound: false,
    outputs: regs::a(0),
};

pub const UNLINK_ALL: Native = Native {
    entry: 0x1f184,
    name: "list_unlink_all",
    run: crate::Body::WithCpu(unlink_all_native),
    touches_sound: false,
    outputs: regs::a(0),
};
