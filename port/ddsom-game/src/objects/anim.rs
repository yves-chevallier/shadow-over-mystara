//! Object animation scripts.
//!
//! A script is a sequence of *entries* in ROM. The object only keeps the current entry, copied
//! into its fields, and the address of the next one (`Obj::ANIM_NEXT`). Format of an entry:
//!
//! | offset | size | content | object field |
//! |---|---|---|---|
//! | +0x00 | word | duration (frames) | `ANIM_TIMER` |
//! | +0x02 | word | flags (high byte used) | `ANIM_FLAGS` |
//! | +0x04 | long | sprite frame | `FRAME` |
//! | +0x08 | long | frame data A (shadow) | `FRAME_DATA_A` |
//! | +0x0c | long | frame data B (visibility box, priority) | `FRAME_DATA_B` |
//! | +0x10 | 2 longs | only in *long* entries (characters) | `FRAME_DATA_C/D` |
//!
//! If the flags byte of the entry is 0, the next one follows immediately; 0xFE marks the end
//! of the script (the object stays on its last frame, `anim_step` returns C); any other value is
//! followed by a long designating the next entry (loop or jump).
//!
//! An animation *set* is an array of words (signed offsets relative to the start of the array);
//! each offset designates the first element of a script.

use cps2::Cps2Bus;

use ddsom_data::sprites::Entry;

use super::Obj;
use crate::data::{data, Missing, RomAddr};
use crate::regs;

/// Result of an animation step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// The duration of the current entry has not elapsed.
    Running,
    /// The duration was zero: it restarts from 0xFFFF. Original glitch: the subtraction borrows, and the carry
    /// that the callers test (`bcs`) is raised as at the end of the script.
    Wrapped,
    /// A new entry has been loaded.
    Loaded,
    /// End of the script: the object stays on its last frame.
    Ended,
}

impl Step {
    /// The original's carry is raised: end of the script, or zero duration restarting from 0xFFFF (original glitch).
    pub fn finished(self) -> bool {
        matches!(self, Step::Ended | Step::Wrapped)
    }
}

/// An entry as the object receives it: duration, flags, then the frame address and the frame data.
struct Loaded {
    timer: u16,
    flags: u16,
    longs: [u32; 5],
}

/// The entry at `at` (`long`: 24 bytes, with `FRAME_DATA_C/D`), from the animation table, or from the residue when
/// the table does not know it.
fn read_entry(b: &mut Cps2Bus, at: u32, long: bool) -> Loaded {
    let d = data();
    let mut longs = [0; 5];
    if let Some(e) = d.anims.entry_at(RomAddr(at), long) {
        for (l, v) in longs.iter_mut().zip(e.longs(&d.frames)) {
            *l = v;
        }
        return Loaded { timer: e.time, flags: e.flags, longs };
    }
    let missing = Missing { table: "anim entry", key: at };
    let (timer, flags) = (missing.read16(b, at), missing.read16(b, at + 2));
    let n = if long { 5 } else { 3 };
    for (i, l) in longs[..n].iter_mut().enumerate() {
        *l = missing.read32(b, at + 4 + 4 * i as u32);
    }
    Loaded { timer, flags, longs }
}

/// Flags word (`+2`) of the entry at `at`, read without loading the entry (the table searches of `ai_aids` step from
/// entry to entry by 16 bytes, as the original does).
pub fn entry_flags(b: &mut Cps2Bus, at: u32) -> u16 {
    let d = data();
    match [false, true].into_iter().find_map(|long| d.anims.entry_at(RomAddr(at), long)) {
        Some(e) => e.flags,
        None => Missing { table: "anim entry", key: at }.read16(b, at.wrapping_add(2)),
    }
}

/// Copies the entry located at `at` into the object and returns the address following it.
/// `long`: the entry also contains `FRAME_DATA_C/D`.
pub fn load_entry(b: &mut Cps2Bus, obj: Obj, at: u32, long: bool) -> u32 {
    let e = read_entry(b, at, long);
    obj.set_anim_timer(b, e.timer);
    obj.set_anim_flags(b, e.flags);
    let fields: &[u32] = if long {
        &[Obj::FRAME, Obj::FRAME_DATA_A, Obj::FRAME_DATA_B, Obj::FRAME_DATA_C, Obj::FRAME_DATA_D]
    } else {
        &[Obj::FRAME, Obj::FRAME_DATA_A, Obj::FRAME_DATA_B]
    };
    for (&field, &v) in fields.iter().zip(&e.longs) {
        obj.set_u32(b, field, v);
    }
    let next = at + Entry::size(long);
    obj.set_anim_next(b, next);
    next
}

/// Script offset at byte `offset` of the animation set `set` (signed, relative to the set).
fn script_offset(b: &mut Cps2Bus, set: u32, offset: u32) -> i16 {
    match data().anims.script_offset(RomAddr(set), offset as i32) {
        Some(o) => o,
        None => Missing { table: "anim set", key: set }.read16(b, set.wrapping_add(offset)) as i16,
    }
}

/// Address of script `index` in an animation set (offset read at `set + 2 × index`, without 16-bit wrap-around:
/// see `start` for the exact indexing of the original).
pub fn script_of(b: &mut Cps2Bus, set: u32, index: u16) -> u32 {
    let off = script_offset(b, set, 2 * index as u32);
    set.wrapping_add(off as i32 as u32)
}

/// One animation step: counts down the duration and, at zero, loads the next entry.
pub fn step(b: &mut Cps2Bus, obj: Obj, long: bool) -> Step {
    let before = obj.anim_timer(b);
    let timer = before.wrapping_sub(1);
    obj.set_anim_timer(b, timer);
    if timer != 0 {
        return if before == 0 { Step::Wrapped } else { Step::Running };
    }
    let next = obj.anim_next(b);
    match obj.u8(b, Obj::ANIM_FLAGS) {
        0 => {
            load_entry(b, obj, next, long);
        }
        0xfe => {
            obj.set_anim_timer(b, 1);
            return Step::Ended;
        }
        _ => {
            let target = match data().anims.jump_at(RomAddr(next)) {
                Some(t) => t.0,
                None => Missing { table: "anim jump", key: next }.read32(b, next),
            };
            load_entry(b, obj, target, long);
        }
    }
    Step::Loaded
}

/// 0x1a32: starts script `n` of the animation set `set` (`lea set,A4 ; moveq #n,D0 ; jsr 0x1a32`). The index
/// is a word (`add.w D0,D0 ; move.w (A4,D0.w),D0`): the double is sign-extended. Returns the address following
/// the loaded entry.
pub fn start(b: &mut Cps2Bus, obj: Obj, set: u32, n: u16) -> u32 {
    let script = script_at(b, set, n);
    load_entry(b, obj, script, false)
}

fn script_at(b: &mut Cps2Bus, set: u32, n: u16) -> u32 {
    let off = script_offset(b, set, n.wrapping_add(n) as i16 as i32 as u32);
    set.wrapping_add(off as i32 as u32)
}

/// 0x1a3c: starts the animation at entry `at` (A4 of the original).
pub fn start_entry(b: &mut Cps2Bus, obj: Obj, at: u32) -> u32 {
    load_entry(b, obj, at, false)
}

/// Animation sets specific to each character (long entries): a pointer table read in the code,
/// one set per `Obj::VARIANT` variant.
pub struct CharacterSets {
    /// Original routine that starts an animation from these sets.
    pub routine: u32,
    /// Table of set pointers (program space).
    pub pointers: u32,
    /// Without a weapon (byte `0x68` zero), the animation numbers are shifted by 0x40.
    pub weapon_offset: bool,
    /// The variant is sign-extended before indexing.
    pub sign_extend: bool,
}

/// 0x2c16: player sets, numbers shifted by 0x40 with no weapon in hand.
pub const SETS_ARMED: CharacterSets =
    CharacterSets { routine: 0x2c16, pointers: 0x2c58, weapon_offset: true, sign_extend: true };
/// 0x2c88: second table, direct number.
pub const SETS_PLAIN: CharacterSets =
    CharacterSets { routine: 0x2c88, pointers: 0x2cc0, weapon_offset: false, sign_extend: true };
/// 0x2cf0: third table, variant taken without sign extension.
pub const SETS_BY_VARIANT: CharacterSets =
    CharacterSets { routine: 0x2cf0, pointers: 0x2d28, weapon_offset: false, sign_extend: false };

/// Object byte: the character holds their weapon.
pub(crate) const ARMED: u32 = 0x68;

/// Starts animation `n` (D6 of the original) of the set specific to the object's character (long entries).
pub fn start_by_character(b: &mut Cps2Bus, obj: Obj, sets: &CharacterSets, n: u16) -> u32 {
    let n = if sets.weapon_offset && obj.u8(b, ARMED) == 0 { n.wrapping_add(0x40) } else { n };
    let variant = obj.variant(b);
    let v = if sets.sign_extend { variant as i8 as i16 as u16 } else { variant as u16 };
    let index = (v << 2) as i16 as i32;
    let set = match data().anims.character_sets(RomAddr(sets.pointers)).and_then(|p| p.get(index / 4)) {
        Some(set) => set.0,
        None => Missing { table: "character sets", key: sets.pointers }.pc_long(b, sets.pointers.wrapping_add(index as u32)),
    };
    let script = script_at(b, set, n);
    load_entry(b, obj, script, true)
}

// ---- adaptation to the 68000 registers ----

/// Registers left behind by the original routines (see `diag::dead_regs`, label `anim`).
pub const REGS_START: u32 = regs::d(0) | regs::a(4) | regs::CCR;
pub const REGS_STEP: u32 = regs::d(0) | regs::a(4) | regs::CCR;
pub const REGS_BY_CHARACTER: u32 = regs::d(5) | regs::d(6) | regs::a(4) | regs::CCR;

// ---- natives (remaining calls by address) ----
