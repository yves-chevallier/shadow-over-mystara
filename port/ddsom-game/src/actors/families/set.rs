//! Scenery objects (SET class, pool of 32 objects of 0x50 bytes, table 0x39eac): types 0, 1 and
//! 3 to 14. SET objects do not appear in the sorted lists (see `docs/objets.md`).

use crate::actors::families::cx::Bench;
use super::cx::{Cx, Gx};
use crate::world::objects::Obj;
use crate::world::objects::pools;
use crate::world::objects::Current;
use crate::world::machine::Machine;

/// The ROM tables that the SET types read (`data/set.json`, `data/dialogs.json`).
pub(crate) mod tables;
/// The work RAM variables that the SET types use.
pub(crate) mod vars;
/// Typed views of the SET objects of this module.
pub mod fields;


/// Type 8 (0xcc25a), shared by the SET (8), SOBJ (4) and EFFCT (18) tables: scene controller
/// generator. On the first frame it waits; on the next it creates a SET object of
/// type 2 (the controller, of variant `+0x2f`) then goes to state 6. In states 4 and 6 it
/// releases itself in its class's pool (EFFCT, SET or SOBJ according to `+0x28`).
///
/// Variant `+3`: the original's jump table has only one entry (variant 1 falls onto the same
/// target), variant 2 jumps into the middle of a `jsr` and beyond into data: outside the contract, the
/// rewrite treats every variant as 0 (the fuzz only draws 0 and 1).
mod scene_spawner;

/// Helpers shared by the types of the SET class.
mod util {
    use super::*;
    use crate::hw::mem::{rd16, wr16};

    /// First player record, four players spaced 0x200 bytes apart.
    pub const PLAYERS: u32 = crate::world::ram::PLAYERS.base;
    pub const PLAYER_STRIDE: u32 = 0x200;
    /// Fields of a player: active (byte), hit points (word).
    pub const P_ACTIVE: u32 = 0x0;
    pub const P_LIFE: u32 = 0x62;
    /// A5 variable: the game is in progress (non-zero).
    pub const IN_GAME: u32 = 0x35;

    pub fn players() -> impl Iterator<Item = Obj> {
        (0..4).map(|i| Obj(PLAYERS + PLAYER_STRIDE * i))
    }

    /// Player present in the game: slot occupied and non-zero hit points.
    pub fn alive<R: Bench>(x: &mut Cx<'_, R>, p: Obj) -> bool {
        p.u8(x.b, P_ACTIVE) != 0 && p.u16(x.b, P_LIFE) != 0
    }

    /// Advances each component (red, green, blue: one nibble each) of a 4+4+4-bit color
    /// by one notch toward its maximum, and forces the brightness nibble to 0xf.
    pub fn brighten<R: Bench>(x: &mut Cx<'_, R>, addr: u32, count: u32) {
        for i in 0..count {
            let a = addr + 2 * i;
            let c = rd16(x.b, a);
            let step = |mask: u16, one: u16| {
                let v = c & mask;
                if v == mask { v } else { v + one }
            };
            wr16(x.b, a, 0xf000 + step(0xf, 1) + step(0xf0, 0x10) + step(0xf00, 0x100));
        }
    }
}

/// Type 9 (0xcc2c8): inert object. Suspended (`+0x40`), it does nothing; otherwise state 0 goes to
/// state 2, which does nothing either (the original tests a collision there with player A6 of which
/// no result is used); states 4 and 6 release it.
mod inert;

/// Type 11 (0xcdcaa): end of level of the bonus game (h). When the game is in progress and
/// variable `A5+0x556` exceeds 0x320, it waits for all living players to be in state
/// 0x200 (+4/+5) with no substate, then puts them back in state 0x80000, turns off their weapon (`+0x8a`) and
/// numbers their order (`+0x7d`), sets `A5+0x2c` and goes to state 4 (release on the next frame).
mod level_end;

/// Type 12 (0xcdd70): palette fade to white (h). State 0 notes `+0x3c` = 0xf (not read back);
/// state 2, on each frame, brightens by one notch the colors of four blocks of palette RAM
/// (4 palettes of 0x91c100, then 0x91c720, 0x91c800 and 0x91cc00: 32 colors each). Another
/// object stops it by changing its state (4 or 6: release).
mod fade_white;

/// Type 13 (0xcdef6): palette animation (h). The variant picks the destination palette
/// (0x91c000 + 32 bytes per variant); a table of 17 palettes in ROM (0x291f00, or 0x291f00 +
/// 0x360 when `A5+0x70` is non-zero) is copied, from the last to the first, every 8
/// frames. After the last one it releases itself.
mod palette_anim;

/// Type 14 (0xcdf86): end-of-scene trigger (h). When the game is in progress and
/// camera abscissa `A5+0x506` has reached 0x1c80, it puts all living players in state 2
/// (player's byte `+0`), raises `A5+0x42` and `A5+0x4a`, sets `A5+0x2c` to 0x10000000 and goes to
/// state 4 (release). Variant `+3` has no effect: the original's two branches are
/// identical.
mod scene_trigger;

/// Types 4 (0xc935a) and 6 (0xcafec): scenery animation by rewriting tile cells (h).
///
/// Variant `+3` picks a 16-byte entry from a ROM table (program space):
/// script address (`+0x30`), address of the tile map to modify in video memory
/// (`+0x34`), non-zero word `+0x38` to loop. The script is a sequence of blocks: duration (word,
/// negative = end), number of columns, number of rows, then one long per cell. Each cell of the
/// map becomes `(attr & 0xff80) + long`: the tile code is replaced, the high bits of
/// the attribute (palette, flips) are kept. Rows are spaced 0x40 bytes apart
/// (0x20 if `+0x2f` is non-zero). At the end of the script, the object restarts from state 0 if `+0x38` is
/// non-zero, otherwise goes to state 4 (release). Type 4 stops during the game pause and when
/// the object is suspended; type 6 ignores the pause, but leaves untouched the cells whose
/// first word is 0x1800.
mod tile_anim;

/// Type 1 (0xbc518): leader of enemy waves (h), active only in a game (`A5+0x35`).
///
/// A chief (the owner `+0x46`, an ENEMY object) calls for waves of three enemies. The leader
/// first waits 180 frames (`+0x22` counts down from 0xf0 to 0x3c, then it locks the camera through
/// 0x2860), then chains the substates `+5`:
///
/// | `+5` | role |
/// |---|---|
/// | 0 | wait of 180 frames, then camera lock |
/// | 2 | launches a wave (`spawn_wave`), then waits for a delay drawn from a table; goes to 4 when `+0x21` or `+0x20` equals 2 (waves defeated, h) or after 8 waves (`+0x25`); notifies the chief through `+0x24` = 1 |
/// | 4 | waits for the chief's counter `+0x2a` to equal 1 |
/// | 6 | waits for `+0x30` = 0; if `+0x31` is non-zero, subtracts one from the chief's counter and finishes, otherwise launches the final wave |
/// | 8 | final wave: first enemy of variant 2, wave result 0xff |
/// | 0xa, 0xc | wait for the end of the wave then 300-frame delay |
/// | 0xe | waits for `+0x30` = 0, subtracts one from the chief's counter and finishes |
///
/// The main state then goes to 4 then 6 (release).
mod wave_leader;

/// Fade to black of the doors (SET 7 [`exit_gate`], SET 29 `set_b::door_b`): subroutine 0x1460 cedes control
/// during its 16 steps. The door calls it like a task subroutine (batch E): the continuation, a [`Step`] that
/// carries the door (A0), is stored in the task's pending calls; at the end of the fade, the task resumes there,
/// then returns to the dispatch loop (its pending call, `dispatch::Step::Dispatch`).
pub mod exit_fading;

/// Type 7 (0xcb29c): zone exit door (h). When a player stays 24 frames inside the door's
/// footprint, they "trigger" the exit (state imposed by the zone); when all living players are
/// ready, the zone changes. It only acts if `+0x2f` is zero, outside pause and with no transition in progress
/// (`A5+0x3d`, `A5+0x50`, `A5+0x42`), and if no other door has taken over (`A5+0x52`).
///
/// | `+4` | role |
/// |---|---|
/// | 0 | initialization from the door table (16 bytes per variant `+3`, program space) |
/// | 2 | watches the four players; `+0x42..+0x45` count the frames spent in the footprint; `+0x16` counts the ready players |
/// | 4 | return from the zone: reloads the scene (maps, palettes, saved camera, players repositioned) |
/// | 6 | release |
///
/// When all living players are ready, the zone changes according to the exit kind `+0x48`:
///
/// | `+0x48` | sortie |
/// |---|---|
/// | 0 | entry into a side zone: fade to black, the door saves the camera, suspends the scenery and the objects (`+0x40`), loads scene `+0x4f`, and waits in state 4 for the return |
/// | 1 | return from a side zone: fade, players released, the suspended objects wake up (those whose counter drops below zero are released) |
/// | 2 | next scene `+0x4f`: fade, then the game goes back to phase 8 (scene entry) |
/// | 3 | fin de partie (`A5+0x42`) |
///
/// The fade to black (0x1460) cedes control for 16 frames: the task resumes at steps
/// [`exit_fading::Step::Zone`] (0xcb896) and [`exit_fading::Step::Scene`] (0xcb9c0).
mod exit_gate;

/// Type 0 (0xba310): palette animation by script (h), the most frequent scenery (over 13,000
/// calls per scenario). Variant `+3` picks an 8-byte entry of table `TABLE`: script address
/// (`+0x30`) and source palette address (`+0x34`). Parameter `+0x2f` picks the
/// page of palette RAM to write to (0: 0x91c800, 1: 0x91cc00, 2: 0x91c000, other: 0x91c400).
///
/// The script starts with a header (starting palette row number, duration of the first block)
/// then is made of blocks: duration (word: 0 → state 4, negative → end), number of rows, and for
/// each row the index (in 32-byte rows) of the source palette. Each copied row keeps
/// the brightness nibble (`& 0xf000f000`) of the destination row. A duration of 0xffff restarts
/// the script, any other negative jumps to the long address that follows.
///
/// | `+4` | role |
/// |---|---|
/// | 0 | initialization and first block |
/// | 2 | one block at each deadline of `+0x3c`; if `A5+0x42` (scene in transition), a per-variant table decides whether to continue (0), wait (1) or finish (2, 3) |
/// | 4 | end: a per-variant table picks release (0) or reconversion into variant 0x48, 0x4c or 0x4d |
/// | 6 | release |
///
/// Variant 0x4e ignores the pause and the suspension.
mod palette_script;

/// Type 10 (0xcc3c2): timed dialogue bubble (h). On creation (state 0) it picks a text
/// from the scene's tables (one table per game mode `A5+0x72`, one entry per variant `+3`),
/// creates a SOBJ object of type 0xc (the frame, via 0xdfc78) whose address it keeps in `+0x22`,
/// publishes the text at `A5+0x5ce..0x5d4` and loads palette 0x91c180. It lives 120 frames
/// (`+0x20`), or until variable `A5-0x5fbe` becomes non-zero, then clears the text,
/// requests the release of its frame (state 6) and releases itself.
mod dialogue;

/// Type 5 (0xc9780): name entry on the score board (h). Two kinds of objects according to `+3`:
///
/// * variant 0: the **board** (a single object). It keeps in `+0x18..+0x1f` the addresses of the four
///   cursors, detects the cells where two cursors overlap (two 8-byte records
///   at `+0x30` and `+0x40` for the previous frame) and draws those cells;
/// * non-zero variant: a player's **cursor** (`+0x2f` = player number, `+0x22` = its
///   record). It moves the cursor (`+0x8` column, `+0xc` row) over a grid of
///   characters (11 columns × 6 rows, or a wide grid if `A5+0x70`), writes the letters into the
///   player's buffer (`+0x118..+0x11f`, 0xff = empty, `+0x18` = length), handles erasing, the
///   voiced vowels (0x9d, 0x9f, conversion table), two secret codes (key sequences
///   recognized in the buffer) and computes a BCD check digit.
///
/// States (`+4`): 0 initialization, 2 entry, 4 finished (the cursor waits), 6 release. The board
/// releases the cursors (state 6 in the object) then itself in states 4 and 6.
mod name_entry;

crate::actors::families::cx::handlers! {
    SET_COMMON_CC25A 0xcc25a "set_common_cc25a" scene_spawner::run,
    SET_0 0xba310 "set_0" palette_script::run,
    SET_1 0xbc518 "set_1" wave_leader::run,
    SET_4 0xc935a "set_4" tile_anim::run4,
    SET_5 0xc9780 "set_5" name_entry::run,
    SET_6 0xcafec "set_6" tile_anim::run6,
    SET_7 0xcb29c "set_7" exit_gate::run,
    SET_9 0xcc2c8 "set_9" inert::run,
    SET_10 0xcc3c2 "set_10" dialogue::run,
    SET_11 0xcdcaa "set_11" level_end::run,
    SET_12 0xcdd70 "set_12" fade_white::run,
    SET_13 0xcdef6 "set_13" palette_anim::run,
    SET_14 0xcdf86 "set_14" scene_trigger::run,
}
