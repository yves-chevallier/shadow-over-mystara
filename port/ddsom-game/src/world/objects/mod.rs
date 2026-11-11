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
//! - `enemy`: fields of the ENEMY and BOSS objects (`Enemy`), shared by the families and the AI helpers.
//! - `current`: the `Current` trait, these routines applied to a handler's object (`x.move_xz()`...).

pub mod anim;
pub mod current;
pub mod cull;
pub mod enemy;
pub mod lists;
pub mod motion;
pub mod player;
pub mod pools;
pub mod rank;

pub use current::Current;
pub use enemy::Enemy;

use crate::world::World;

use crate::hw::mem::{rd16, rd32, rd8, wr16, wr32, wr8};
use crate::world::record::{record, Value};
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

impl Value for Link {
    fn read(b: &mut World, a: u32) -> Self {
        Link(rd16(b, a))
    }
    fn write(self, b: &mut World, a: u32) {
        wr16(b, a, self.0)
    }
}

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

record! {
    impl Obj {
        /// Non-zero byte: the pool slot is occupied.
        active, set_active, ACTIVE: u8 @ 0x00;
        /// Bit 0: visible this frame; bit 1: hidden (not drawn, out of collisions).
        flags, set_flags, FLAGS: u8 @ 0x01;
        /// Type: index of the handler in the table of its class.
        kind, set_kind, KIND: u8 @ 0x02;
        /// Variant (meaning specific to each type; character for the animation tables).
        variant, set_variant, VARIANT: u8 @ 0x03;
        /// State bytes of the handler's automaton: main state, sub-states.
        state, set_state, STATE: u8 @ 0x04;
        /// Abscissa (16.16, pixels).
        x, set_x, X: u32 @ 0x08;
        /// Altitude (16.16): added to `Y` for the screen height.
        z, set_z, Z: u32 @ 0x0c;
        /// Depth on the ground (16.16): sort key of the display lists.
        y, set_y, Y: u32 @ 0x10;
        /// Non-zero byte: hidden or suspended object (tested by `motion::pause_hook`).
        hidden, set_hidden, HIDDEN: u8 @ 0x40;
        /// Speed and acceleration in X, Z, Y (8.8 pixels/frame).
        vx, set_vx, VX: u16 @ 0x14;
        ax, set_ax, AX: u16 @ 0x16;
        vz, set_vz, VZ: u16 @ 0x18;
        az, set_az, AZ: u16 @ 0x1a;
        vy, set_vy, VY: u16 @ 0x1c;
        ay, set_ay, AY: u16 @ 0x1e;
        /// Animation script: address of the next entry (long).
        anim_next, set_anim_next, ANIM_NEXT: u32 @ 0x20;
        /// Links of the sorted list: previous (toward the head), next (toward the tail). Words.
        prev, set_prev, PREV: Link @ 0x24;
        next, set_next, NEXT: Link @ 0x26;
        /// Class code (= original pool: see `pools::Pool::class`).
        class, set_class, CLASS: u8 @ 0x28;
        /// Terrain number under the object, set by the collision with the terrain (`terrain::push`).
        terrain, set_terrain, TERRAIN: u8 @ 0x29;
        /// Orientation: 0 faces right, 0x20 faces left (horizontal mirror of the sprite).
        flip, set_flip, FLIP: u8 @ 0x2c;
        /// Index into the speed table (`SPEED_TABLE`).
        speed_index, set_speed_index, SPEED_INDEX: u8 @ 0x2d;
        /// Bit 0: imposed palette (`PALETTE` replaces the palette of the frame's pieces), and that palette.
        own_palette, set_own_palette, OWN_PALETTE: u8 @ 0x2a;
        palette, set_palette, PALETTE: u8 @ 0x2b;
        /// Collision box set (one set per object kind: `sprites/tables.json`).
        box_set, set_box_set, BOX_SET: u8 @ 0x2e;
        /// Setting given by the creator of the object (group, rank, number in a series...).
        setting, set_setting, SETTING: u8 @ 0x2f;
        /// Current animation entry: remaining duration (frames), flags (high byte: 0 continue,
        /// 0xFE end, other jump), sprite frame, then two longs of data per frame.
        anim_timer, set_anim_timer, ANIM_TIMER: u16 @ 0x30;
        anim_flags, set_anim_flags, ANIM_FLAGS: u16 @ 0x32;
        frame, set_frame, FRAME: u32 @ 0x34;
        frame_data_a, set_frame_data_a, FRAME_DATA_A: u32 @ 0x38;
        /// The bytes of `FRAME_DATA_A`: index of the body box, of the push box and of the attack box in the set
        /// `BOX_SET` (0: none), then the ground mark.
        body_box, set_body_box, BODY_BOX: u8 @ 0x38;
        push_box, set_push_box, PUSH_BOX: u8 @ 0x39;
        attack_box, set_attack_box, ATTACK_BOX: u8 @ 0x3a;
        /// The bytes of `FRAME_DATA_B`: ground mark (`sprites/tables.json`, `marks`), visibility box (`bounds`), then
        /// the display priority.
        ground_mark, set_ground_mark, GROUND_MARK: u8 @ 0x3b;
        bounds, set_bounds, BOUNDS: u8 @ 0x3c;
        priority, set_priority, PRIORITY: u8 @ 0x3f;
        frame_data_b, set_frame_data_b, FRAME_DATA_B: u32 @ 0x3c;
        /// Two additional longs of data for the long animation entries.
        frame_data_c, set_frame_data_c, FRAME_DATA_C: u32 @ 0x40;
        frame_data_d, set_frame_data_d, FRAME_DATA_D: u32 @ 0x44;
        /// Byte reset to zero by `motion::tick_counters` when it is negative.
        counter_53, set_counter_53, COUNTER_53: u8 @ 0x53;
        /// Countdown (frames): at zero, `COUNTDOWN_FLAG` is cleared.
        countdown, set_countdown, COUNTDOWN: u8 @ 0x89;
        countdown_flag, set_countdown_flag, COUNTDOWN_FLAG: u8 @ 0x88;
        /// Word argument given by the creator of the object (object scripts of the SET, creators).
        arg, set_arg, ARG: u16 @ 0x42;
        /// Low word of the address of the object that created or owns this one (scene controller, wave, host).
        owner, set_owner, OWNER: u16 @ 0x46;
        /// Object this one is tied to (word, RAM address): the owner of a shot or an effect, the window that a window
        /// effect closes.
        tied, set_tied, TIED: u16 @ 0x50;
        /// Objects attached to this one (words; a negative word is the RAM address of the object), drawn in front of
        /// it or behind it according to their own depth (`sprites`).
        attached_b, set_attached_b, ATTACHED_B: u16 @ 0x48;
        attached_a, set_attached_a, ATTACHED_A: u16 @ 0x4a;
        /// Current speed table (ROM address of an array of (vx, vy) 8.8 indexed by `SPEED_INDEX`).
        speed_table, set_speed_table, SPEED_TABLE: u32 @ 0x4c;
    }
}

impl Obj {
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

    pub fn u8(self, b: &mut World, off: u32) -> u8 {
        rd8(b, self.at(off))
    }
    pub fn u16(self, b: &mut World, off: u32) -> u16 {
        rd16(b, self.at(off))
    }
    pub fn u32(self, b: &mut World, off: u32) -> u32 {
        rd32(b, self.at(off))
    }
    pub fn set_u8(self, b: &mut World, off: u32, v: u8) {
        wr8(b, self.at(off), v)
    }
    pub fn set_u16(self, b: &mut World, off: u32, v: u16) {
        wr16(b, self.at(off), v)
    }
    pub fn set_u32(self, b: &mut World, off: u32, v: u32) {
        wr32(b, self.at(off), v)
    }

    pub fn is_active(self, b: &mut World) -> bool {
        self.u8(b, Self::ACTIVE) != 0
    }

    /// Depth (sort key), signed.
    pub fn depth(self, b: &mut World) -> i32 {
        self.u32(b, Self::Y) as i32
    }

}
