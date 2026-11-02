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

use cps2::Cps2Bus;
use m68k::decode::Fetch;

use crate::mem::{rd16, rd32, rd8, wr32, wr8, A5};
use crate::objects::{pools, Obj};
use crate::scene::var;
use crate::Native;

/// Reader state (byte) and cursor in the script (long).
const STATE: u32 = A5 - 0x55cc;
const CURSOR: u32 = A5 - 0x55d0;
/// Script table (one long per scene, as data) and per-class allocators (words, program space).
const SCRIPTS: u32 = 0xf2f62;
const ALLOCATORS: u32 = 0xf2f38;
/// Game rank (low byte of word `A5+0x7f92`).
const RANK: u32 = 0x7f93;

mod state {
    pub const LOAD: u8 = 0;
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
}

/// The routines called by the `call` entries of the scene scripts (all those of the ROM's scripts: table 0xf2f62),
/// with their argument (the D0.w of the entry). They are still on the processor (`families/`): the registers they
/// leave are reread by nobody but the leftovers D2-D5.
fn script_routine(b: &mut Cps2Bus, routine: u32, arg: u16) {
    use crate::families::{effects_creators, small, set_b, set_residue::camera};
    crate::bench::on_bench(b, |c, b| match routine {
        0x2a24 => camera::set_limit_x(c, b, arg),
        0x2a2a => camera::set_position_y(c, b, arg),
        0x2a30 => camera::set_mode(c, b, arg as u8),
        0xa8ae8 => {
            effects_creators::set_list_d0_68k(c, b, arg);
        }
        0xd40e8 => set_b::spawn_bumpers(c, b),
        0xdd12c => small::item_lock_clear(c, b),
        r => unreachable!("scene script routine {r:#x}: the ROM's scripts only call 0x2a24, 0x2a2a, 0x2a30, 0xa8ae8, 0xd40e8, 0xdd12c"),
    })
}

/// One reader step (one frame). The registers of the original loop (D0, D1, D6, A1, A4, A6) are reread by nobody.
pub fn step(b: &mut Cps2Bus) {
    if rd8(b, A5 + var::PLAYERS) == 0 {
        return;
    }
    let state = rd8(b, STATE);
    let mut cursor = match state {
        state::LOAD => {
            wr8(b, STATE, state::READ);
            let scene = rd8(b, A5 + var::SCENE) as u32;
            let script = rd32(b, SCRIPTS + scene * 4);
            wr32(b, CURSOR, script);
            script
        }
        state::READ => rd32(b, CURSOR),
        state::ENDING => {
            wr8(b, STATE, state::ENDING + 2);
            return;
        }
        _ => return,
    };
    let camera = A5 + if rd8(b, A5 + var::USE_ALT_CAMERA) != 0 { var::ALT_CAMERA } else { var::CAMERA };
    loop {
        let trigger = rd16(b, cursor + entry::TRIGGER_X);
        if trigger == END {
            wr8(b, STATE, state::ENDING);
            return;
        }
        if trigger as i16 > rd16(b, camera + 8) as i16 {
            wr32(b, CURSOR, cursor);
            return;
        }
        if rd16(b, cursor + entry::OFFSET_X) == CALL_MARK {
            let routine = rd32(b, cursor + 4);
            let arg = rd16(b, cursor + 8);
            script_routine(b, routine, arg);
            cursor += CALL_ENTRY_SIZE;
            continue;
        }
        if spawn(b, cursor) == Spawn::PoolFull {
            wr32(b, CURSOR, cursor);
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
fn spawn(b: &mut Cps2Bus, e: u32) -> Spawn {
    if rd8(b, e + entry::MAX_RANK) > rd8(b, A5 + RANK) {
        return Spawn::Done;
    }
    if rd8(b, e + entry::MIN_PLAYERS) > rd8(b, A5 + var::PLAYERS) {
        return Spawn::Done;
    }
    let class = rd8(b, e + entry::CLASS) as i8 as i16 as u16;
    let Some(o) = pool_of_class(b, class).alloc(b) else {
        return Spawn::PoolFull;
    };
    let offset = rd16(b, e + entry::OFFSET_X);
    let bytes = [
        (0x2c, rd8(b, e + entry::FIELD_2C)),
        (Obj::KIND, rd8(b, e + entry::KIND)),
        (Obj::VARIANT, rd8(b, e + entry::VARIANT)),
        (0x2f, rd8(b, e + entry::FIELD_2F)),
    ];
    let words = [
        (Obj::X, rd16(b, e + entry::TRIGGER_X).wrapping_add(offset)),
        (Obj::Y, rd16(b, e + entry::Y)),
        (Obj::Z, rd16(b, e + entry::Z)),
        (0x42, rd16(b, e + entry::FIELD_42)),
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
fn pool_of_class(b: &mut Cps2Bus, class: u16) -> &'static pools::Pool {
    let alloc = b.fetch16(ALLOCATORS + class as i16 as i32 as u32) as i16 as i32 as u32 & 0xff_ffff;
    pools::POOLS
        .into_iter()
        .find(|p| p.alloc_routine == alloc)
        // the tenth entry (0x226e) is the allocator of no pool: no script uses it
        .unwrap_or_else(|| unreachable!("spawn script: class {class:#x} without allocator ({alloc:#x})"))
}

/// Resets the reader to the "load the script" state (0xf2e56, scene change).
pub fn reset(b: &mut Cps2Bus) {
    wr8(b, STATE, state::LOAD);
}

/// Entries of the two routines.
pub const SPAWN_ENTRY: u32 = 0xf2e5c;
pub const RESET_ENTRY: u32 = 0xf2e56;

pub const SPAWN_SCRIPT: Native = Native {
    entry: SPAWN_ENTRY,
    name: "scene_script",
    run: crate::Body::Memory(step),
    touches_sound: false,
    outputs: 0,
};

/// Resets the reader to the "load the script" state (scene change).
pub const SCRIPT_RESET: Native = Native {
    entry: RESET_ENTRY,
    name: "scene_script_reset",
    run: crate::Body::Memory(reset),
    touches_sound: false,
    outputs: 0,
};
