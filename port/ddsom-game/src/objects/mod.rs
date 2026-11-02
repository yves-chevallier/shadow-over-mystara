//! Game object model: pools, depth-sorted lists, animation, speeds, culling.
//!
//! Everything that moves on screen (shots, effects, enemies, bosses, animated scenery, pickable items...)
//! is an *object*: a record of 0x50 to 0xC0 bytes in one of the 10 RAM pools
//! (`pools`). Players have their own record (0x200 bytes) but share the
//! common fields below. See `docs/objets.md` for the details of the formats.
//!
//! - `Obj`: typed view of an object in RAM (address + field constants).
//! - `pools`: allocation, release and initialization of the pools.
//! - `lists`: two doubly-linked lists sorted by depth (drawing order).
//! - `anim`: animation scripts (duration, flags, sprite frame).
//! - `motion`: speeds, accelerations, speed tables by index.
//! - `rank`: rank counter (dynamic difficulty).
//! - `cull`: on-screen presence test and registration in the collision queues.
//! - `current`: the `Current` trait, these routines applied to a handler's object (`x.move_xz()`...).

pub mod anim;
pub mod current;
pub mod cull;
pub mod lists;
pub mod motion;
pub mod pools;
pub mod rank;

pub use current::Current;

use cps2::Cps2Bus;

use crate::mem::{rd16, rd32, rd8, wr16, wr32, wr8};
use crate::Native;

/// All the native routines of the object model.
pub const NATIVES: &[Native] = &[
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
];

/// Work RAM variables (offsets from A5 = 0xFF8000) used by the object model.
pub mod ram {
    /// List of characters and scenery: head, tail (links `Obj::PREV`/`Obj::NEXT`).
    pub const ACTORS_HEAD: u32 = 0x5be;
    pub const ACTORS_TAIL: u32 = 0x5c0;
    /// Foreground list: head, tail.
    pub const FOREGROUND_HEAD: u32 = 0x5c2;
    pub const FOREGROUND_TAIL: u32 = 0x5c4;
    /// Main camera (x at +8, y at +0xc) and alternate camera, chosen if `USE_ALT_CAMERA != 0`.
    pub const CAMERA: u32 = 0x4fe;
    pub const ALT_CAMERA: u32 = 0x54e;
    pub const USE_ALT_CAMERA: u32 = 0x36;
}

/// Link of an object list: 16-bit RAM address (bit 15 set: 0xFF8000-0xFFFFFF), or a
/// positive value marking the end of the list (1: actor list, 2: foreground list).
/// 0: the object is in no list.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Link(pub u16);

impl Link {
    pub const NONE: Link = Link(0);

    pub fn to(o: Obj) -> Link {
        Link(o.0 as u16)
    }

    /// The designated object, except for 0 and the end markers.
    pub fn obj(self) -> Option<Obj> {
        (self.0 & 0x8000 != 0).then_some(Obj(0xff_0000 | self.0 as u32))
    }

    /// Designated object, the address being sign-extended as the 68000 does (`movea.w`).
    pub fn target(self) -> Obj {
        Obj(self.0 as i16 as i32 as u32 & 0xff_ffff)
    }

    pub fn is_none(self) -> bool {
        self.0 == 0
    }
}

/// A game object, designated by its RAM address (register A0 of the original).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Obj(pub u32);

impl Obj {
    /// Non-zero byte: the pool slot is occupied.
    pub const ACTIVE: u32 = 0x00;
    /// Bit 0: visible this frame; bit 1: hidden (not drawn, out of collisions).
    pub const FLAGS: u32 = 0x01;
    /// Type: index of the handler in the table of its class.
    pub const KIND: u32 = 0x02;
    /// Variant (meaning specific to each type; character for the animation tables).
    pub const VARIANT: u32 = 0x03;
    /// State bytes of the handler's automaton: main state, sub-states.
    pub const STATE: u32 = 0x04;
    /// Abscissa (16.16, pixels).
    pub const X: u32 = 0x08;
    /// Altitude (16.16): added to `Y` for the screen height.
    pub const Z: u32 = 0x0c;
    /// Depth on the ground (16.16): sort key of the display lists.
    pub const Y: u32 = 0x10;
    /// Non-zero byte: hidden or suspended object (tested by `motion::pause_hook`).
    pub const HIDDEN: u32 = 0x40;
    /// Speed and acceleration in X, Z, Y (8.8 pixels/frame).
    pub const VX: u32 = 0x14;
    pub const AX: u32 = 0x16;
    pub const VZ: u32 = 0x18;
    pub const AZ: u32 = 0x1a;
    pub const VY: u32 = 0x1c;
    pub const AY: u32 = 0x1e;
    /// Animation script: address of the next entry (long).
    pub const ANIM_NEXT: u32 = 0x20;
    /// Links of the sorted list: previous (toward the head), next (toward the tail). Words.
    pub const PREV: u32 = 0x24;
    pub const NEXT: u32 = 0x26;
    /// Class code (= original pool: see `pools::Pool::class`).
    pub const CLASS: u32 = 0x28;
    /// Index into the speed table (`SPEED_TABLE`).
    pub const SPEED_INDEX: u32 = 0x2d;
    /// Current animation entry: remaining duration (frames), flags (high byte: 0 continue,
    /// 0xFE end, other jump), sprite frame, then two longs of data per frame.
    pub const ANIM_TIMER: u32 = 0x30;
    pub const ANIM_FLAGS: u32 = 0x32;
    pub const FRAME: u32 = 0x34;
    pub const FRAME_DATA_A: u32 = 0x38;
    pub const FRAME_DATA_B: u32 = 0x3c;
    /// Two additional longs of data for the long animation entries.
    pub const FRAME_DATA_C: u32 = 0x40;
    pub const FRAME_DATA_D: u32 = 0x44;
    /// Byte reset to zero by `motion::tick_counters` when it is negative.
    pub const COUNTER_53: u32 = 0x53;
    /// Countdown (frames): at zero, `COUNTDOWN_FLAG` is cleared.
    pub const COUNTDOWN: u32 = 0x89;
    pub const COUNTDOWN_FLAG: u32 = 0x88;
    /// Current speed table (ROM address of an array of (vx, vy) 8.8 indexed by `SPEED_INDEX`).
    pub const SPEED_TABLE: u32 = 0x4c;

    pub fn at(self, off: u32) -> u32 {
        self.0.wrapping_add(off)
    }

    /// Object designated by a word (link, 16-bit pointer of a field): address sign-extended as
    /// `movea.w` does, i.e. RAM FF8000-FFFFFF for a negative word.
    pub fn from_word(w: u16) -> Obj {
        Obj(w as i16 as i32 as u32 & 0xff_ffff)
    }

    /// The low word of the address, as objects reference each other.
    pub fn word(self) -> u16 {
        self.0 as u16
    }

    /// The address as register An holds it after `movea.w` (sign-extended to 32 bits): this
    /// is the value the game stores when it remembers an object on 32 bits (`move.l A1,(0x86,A0)`).
    pub fn long_ref(self) -> u32 {
        self.0 as u16 as i16 as i32 as u32
    }

    pub fn u8(self, b: &mut Cps2Bus, off: u32) -> u8 {
        rd8(b, self.at(off))
    }
    pub fn u16(self, b: &mut Cps2Bus, off: u32) -> u16 {
        rd16(b, self.at(off))
    }
    pub fn u32(self, b: &mut Cps2Bus, off: u32) -> u32 {
        rd32(b, self.at(off))
    }
    pub fn set_u8(self, b: &mut Cps2Bus, off: u32, v: u8) {
        wr8(b, self.at(off), v)
    }
    pub fn set_u16(self, b: &mut Cps2Bus, off: u32, v: u16) {
        wr16(b, self.at(off), v)
    }
    pub fn set_u32(self, b: &mut Cps2Bus, off: u32, v: u32) {
        wr32(b, self.at(off), v)
    }

    pub fn is_active(self, b: &mut Cps2Bus) -> bool {
        self.u8(b, Self::ACTIVE) != 0
    }

    /// Depth (sort key), signed.
    pub fn depth(self, b: &mut Cps2Bus) -> i32 {
        self.u32(b, Self::Y) as i32
    }

    pub fn prev(self, b: &mut Cps2Bus) -> Link {
        Link(self.u16(b, Self::PREV))
    }
    pub fn next(self, b: &mut Cps2Bus) -> Link {
        Link(self.u16(b, Self::NEXT))
    }
    pub fn set_prev(self, b: &mut Cps2Bus, l: Link) {
        self.set_u16(b, Self::PREV, l.0)
    }
    pub fn set_next(self, b: &mut Cps2Bus, l: Link) {
        self.set_u16(b, Self::NEXT, l.0)
    }
}
