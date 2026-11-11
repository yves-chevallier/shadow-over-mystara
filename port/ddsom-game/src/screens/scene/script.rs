//! Spawn script reader (`0xf2e5c`): brings in enemies, terrain and objects as the screen scrolls.
//!
//! Each scene has a script in ROM (pointer table `0xf2f62`, one long per scene): a sequence
//! of entries sorted by increasing camera x position. The reader runs every game frame (and
//! at scene entry); it consumes the entries whose x position the camera has reached, then
//! waits. Two entry formats, distinguished by word `+2`:
//!
//! | Entry | Size | Content |
//! |---|---|---|
//! | spawn | 0x12 | `+0` trigger x position, `+2` X offset, `+4` Z, `+6` Y, `+8` maximum rank, `+9` minimum players, `+0xb` class (even byte, index into the allocator table), `+0xc` `+0x2c`, `+0xd` type, `+0xe` variant, `+0xf` `+0x2f`, `+0x10` word `+0x42` of the object |
//! | call | 10 | `+0` x position, `+2` = 0xffff, `+4` address of a routine (long), `+8` its D0 argument (word) |
//!
//! A word `+0` equal to 0xffff ends the script. A spawn whose rank exceeds the entry's maximum
//! (`A5+0x7f93`) or that requires more players than `A5+0x35` is skipped; if the class's
//! pool is full, reading stops and resumes at the same entry on the next frame.
//!
//! States (byte `A5-0x55cc`): 0 loads the scene's script, 2 reads, 4 moves to 6, 6 done.

use crate::world::World;

use crate::screens::obj;
use crate::world::screen_tables::{self as tables, Rom};
use crate::screens::vars as sv;
use crate::world::objects::{pools, Obj};
use crate::screens::scene::var;
use crate::Native;
use crate::world::ram::{Byte, Long};

/// Reader state (byte) and cursor in the script (long).
const STATE: Byte = crate::world::ram::SPAWN_SCRIPT_STATE;
const CURSOR: Long = Long(0xff_2a30);
/// Script table (one long per scene, as data) and per-class allocators (words, program space).
const SCRIPTS: Rom = Rom::at(0xf2f62);
const ALLOCATORS: Rom = Rom::at(0xf2f38);
/// Game rank (low byte of word `A5+0x7f92`).
const RANK: Byte = Byte::a5(0x7f93);

/// Word `+0x42` of the spawned objects (their setting).
const FIELD_42: u32 = 0x42;

mod state {
    pub const LOAD: u8 = crate::world::ram::SPAWN_SCRIPT_LOAD;
    pub const READ: u8 = 2;
    pub const ENDING: u8 = 4;
}

/// End of script (word `+0`) and mark of a call entry (word `+2`).
const END: u16 = 0xffff;
const CALL_MARK: u16 = 0xffff;
const CALL_ENTRY_SIZE: u32 = 10;
const SPAWN_ENTRY_SIZE: u32 = 0x12;

/// Fields of a spawn entry.
mod entry {
    pub const TRIGGER_X: u32 = 0;
    pub const OFFSET_X: u32 = 2;
    pub const Z: u32 = 4;
    pub const Y: u32 = 6;
    pub const MAX_RANK: u32 = 8;
    pub const MIN_PLAYERS: u32 = 9;
    pub const CLASS: u32 = 0xb;
    pub const FIELD_2C: u32 = 0xc;
    pub const KIND: u32 = 0xd;
    pub const VARIANT: u32 = 0xe;
    pub const FIELD_2F: u32 = 0xf;
    pub const FIELD_42: u32 = 0x10;
    /// Call entry: routine (long) and its argument (word).
    pub const ROUTINE: u32 = 4;
    pub const ARGUMENT: u32 = 8;
}

/// The routines called by the `call` entries of the scene scripts (all those of the ROM's scripts: table 0xf2f62),
/// with their argument (the D0.w of the entry, of which 0xa8ae8 takes the low byte). The registers they leave are
/// reread by nobody, except the D5 of the trampolines (a leftover of the game, `dispatch::Leftovers`).
fn script_routine(b: &mut World, routine: u32, arg: u16) {
    use crate::actors::families::{effects_creators, small, set_b, set_residue::camera};
    match routine {
        0x2a24 => camera::set_limit_x(b, arg),
        0x2a2a => camera::set_position_y(b, arg),
        0x2a30 => camera::set_mode(b, arg as u8),
        0xa8ae8 => {
            effects_creators::set_list(b, arg as u8);
        }
        0xd40e8 => b.leftovers.d5 = set_b::spawn_bumpers(b),
        0xdd12c => small::item_lock_clear(b),
        r => unreachable!("scene script routine {r:#x}: the ROM's scripts only call 0x2a24, 0x2a2a, 0x2a30, 0xa8ae8, 0xd40e8, 0xdd12c"),
    }
}

/// One reader step (one frame). The registers of the original loop (D0, D1, D6, A1, A4, A6) are reread by nobody.
pub fn step(b: &mut World) {
    if var::PLAYERS.get(b) == 0 {
        return;
    }
    let state = STATE.get(b);
    let mut cursor = match state {
        state::LOAD => {
            STATE.set(b, state::READ);
            let scene = var::SCENE.get(b) as u32;
            let script = SCRIPTS.long(b, scene * 4);
            CURSOR.set(b, script);
            script
        }
        state::READ => CURSOR.get(b),
        state::ENDING => {
            STATE.set(b, state::ENDING + 2);
            return;
        }
        _ => return,
    };
    let camera_x = if var::USE_ALT_CAMERA.get(b) != 0 { sv::CAMERA_2_X } else { sv::CAMERA_X };
    loop {
        let trigger = tables::word(b, cursor + entry::TRIGGER_X);
        if trigger == END {
            STATE.set(b, state::ENDING);
            return;
        }
        if trigger as i16 > camera_x.get(b) as i16 {
            CURSOR.set(b, cursor);
            return;
        }
        if tables::word(b, cursor + entry::OFFSET_X) == CALL_MARK {
            let routine = tables::long(b, cursor + entry::ROUTINE);
            let arg = tables::word(b, cursor + entry::ARGUMENT);
            script_routine(b, routine, arg);
            cursor += CALL_ENTRY_SIZE;
            continue;
        }
        if spawn(b, cursor) == Spawn::PoolFull {
            CURSOR.set(b, cursor);
            return;
        }
        cursor += SPAWN_ENTRY_SIZE;
    }
}

#[derive(PartialEq)]
enum Spawn {
    /// Created, or skipped (rank or number of players insufficient).
    Done,
    /// No room in the pool: retry on the next frame.
    PoolFull,
}

/// A spawn entry.
fn spawn(b: &mut World, e: u32) -> Spawn {
    if tables::byte(b, e + entry::MAX_RANK) > RANK.get(b) {
        return Spawn::Done;
    }
    if tables::byte(b, e + entry::MIN_PLAYERS) > var::PLAYERS.get(b) {
        return Spawn::Done;
    }
    let class = tables::byte(b, e + entry::CLASS) as i8 as i16 as u16;
    let Some(o) = pool_of_class(b, class).alloc(b) else {
        return Spawn::PoolFull;
    };
    let offset = tables::word(b, e + entry::OFFSET_X);
    let bytes = [
        (obj::FACING, tables::byte(b, e + entry::FIELD_2C)),
        (Obj::KIND, tables::byte(b, e + entry::KIND)),
        (Obj::VARIANT, tables::byte(b, e + entry::VARIANT)),
        (obj::INDEX, tables::byte(b, e + entry::FIELD_2F)),
    ];
    let words = [
        (Obj::X, tables::word(b, e + entry::TRIGGER_X).wrapping_add(offset)),
        (Obj::Y, tables::word(b, e + entry::Y)),
        (Obj::Z, tables::word(b, e + entry::Z)),
        (FIELD_42, tables::word(b, e + entry::FIELD_42)),
    ];
    o.set_u8(b, Obj::ACTIVE, 1);
    for (off, v) in bytes {
        o.set_u8(b, off, v);
    }
    for (off, v) in words {
        o.set_u16(b, off, v);
    }
    Spawn::Done
}

/// The pool of class `class` (even byte): the allocator table (0xf2f38, program-space words) gives
/// the address of the allocation routine of one of the pools.
fn pool_of_class(b: &mut World, class: u16) -> &'static pools::Pool {
    let alloc = ALLOCATORS.pc_word(b, class as i16 as i32 as u32) as i16 as i32 as u32 & 0xff_ffff;
    pools::POOLS
        .into_iter()
        .find(|p| p.alloc_routine == alloc)
        // the tenth entry (0x226e) is the allocator of no pool: no script uses it
        .unwrap_or_else(|| unreachable!("spawn script: class {class:#x} without allocator ({alloc:#x})"))
}

/// Resets the reader to the "load the script" state (0xf2e56, scene change).
pub fn reset(b: &mut World) {
    STATE.set(b, state::LOAD);
}



pub const SPAWN_SCRIPT: Native = Native {
    entry: 0xf2e5c,
    name: "scene_script",
    run: crate::Body::Memory(step),
    touches_sound: false,
    outputs: 0,
};

/// Resets the reader to the "load the script" state (scene change).
pub const SCRIPT_RESET: Native = Native {
    entry: 0xf2e56,
    name: "scene_script_reset",
    run: crate::Body::Memory(reset),
    touches_sound: false,
    outputs: 0,
};
