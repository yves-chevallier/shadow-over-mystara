//! Special objects (SOBJ class, pool of 16 objects of 0x50 bytes, table 0x39fa8): scrolling scenery
//! and scene elements (title screen, endings, interludes), types 0 to 15 except 4 (shared, see
//! `set.rs`). Voir `docs/families/sobj.md`.
//!
//! SOBJ objects appear in the sorted lists: `+0x42` picks the foreground list
//! (non-zero) or the actors' list. The creator fills in `+3` (variant) and often `+0x4c` /
//! `+0x2f` / `+0x14` (type-specific parameters).

use crate::families::cx::Bench;
use super::cx::{handlers, Cx};
use super::shell::{suspended, GAME_PAUSED};
use crate::mem::{rd16, rd32, rd8, wr16, wr32, wr8};
use crate::objects::Obj;
use crate::objects::pools;
use super::effects_creators::leftovers::Pure;
use crate::objects::Current;
use crate::machine::Machine;

/// Work RAM variables (A5+): game camera and secondary camera.
mod ram {
    pub const CAMERA_X: u32 = 0x506;
    pub const CAMERA_Y: u32 = 0x50a;
    /// Previous camera position (to derive the frame's scroll from it).
    pub const PREV_CAMERA_X: u32 = 0x524;
    pub const PREV_CAMERA_Y: u32 = 0x528;
    pub const CAMERA2_X: u32 = 0x556;
    pub const CAMERA2_Y: u32 = 0x55a;
    pub const PREV_CAMERA2_X: u32 = 0x574;
    /// `A5+0x36` non-zero: the reference camera is the secondary one.
    pub const USE_CAMERA2: u32 = 0x36;
}

/// Animation set shared by SOBJ objects.
const ANIMS: u32 = 0x1c03a6;

/// End of handler for objects with a variable list: foreground list if `+0x42` is non-zero,
/// actors' list otherwise.
fn reinsert_by_layer<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(0x42) != 0 {
        x.reinsert_foreground();
    } else {
        x.reinsert();
    }
}

/// Leaves the screen by more than `margin` pixels: state 4 (disappearance). The margin is in D2; the
/// test is done in unsigned words (original routine 0xddbfc).
fn cull_beyond<R: Bench>(x: &mut Cx<'_, R>, margin: u16) {
    let mut camera = x.a5_u16(ram::CAMERA_X);
    if x.a5_u8(ram::USE_CAMERA2) != 0 {
        camera = x.a5_u16(ram::CAMERA2_X);
    }
    let screen_x = x.u16(0x8).wrapping_sub(camera).wrapping_add(margin);
    let limit = margin.wrapping_mul(2).wrapping_add(0x180);
    // Original glitch: 0xddbfc leaves the limit in D2.w, which the next handler may read (margin of SOBJ 2)
    *x.c.d2() = *x.c.d2() & 0xffff_0000 | limit as u32;
    if screen_x > limit {
        x.set8(0x4, 4);
    }
}

/// Camera offset since the previous frame (X then Y), in 16.16.
fn camera_delta<R: Bench>(x: &mut Cx<'_, R>) -> (u32, u32) {
    (
        x.a5_u32(ram::CAMERA_X).wrapping_sub(x.a5_u32(ram::PREV_CAMERA_X)),
        x.a5_u32(ram::CAMERA_Y).wrapping_sub(x.a5_u32(ram::PREV_CAMERA_Y)),
    )
}

/// Brightness bits forced to maximum on the colors copied into the GFX RAM palettes.
const BRIGHT: u32 = 0xf000_f000;

/// Copies `n` longs from the ROM (data space) to video memory or RAM.
fn copy_longs<R: Bench>(x: &mut Cx<'_, R>, from: u32, to: u32, n: u32) {
    for i in 0..n {
        let v = x.data32(from + 4 * i);
        wr32(x.b, to + 4 * i, v);
    }
}

/// Forces bits in `n` consecutive longs.
fn or_longs<R: Bench>(x: &mut Cx<'_, R>, at: u32, mask: u32, n: u32) {
    for i in 0..n {
        let v = rd32(x.b, at + 4 * i) | mask;
        wr32(x.b, at + 4 * i, v);
    }
}

/// Arithmetic operation `asr.l` on 32 bits.
fn asr(v: u32, n: u32) -> u32 {
    ((v as i32) >> n) as u32
}

// ---------------------------------------------------------------------------------------------
// Type 0 (0xdd61e): scenery element tied to the cameras
// ---------------------------------------------------------------------------------------------

/// Scenery element whose position follows the cameras' scrolling at a fraction of their
/// speed (parallax, clouds, foreground). Variant `+3` picks, through two ROM byte tables,
/// the initialization routine and the per-frame following routine.
mod scenery;

// ---------------------------------------------------------------------------------------------
// Type 1 (0xddc28): fixed scenery with two behaviors
// ---------------------------------------------------------------------------------------------

/// Animated scenery (animation set 0x1c40c6) that stays in place, moves with the camera at half
/// speed or follows the secondary camera depending on the variant.
mod fixture;

// ---------------------------------------------------------------------------------------------
// Type 2 (0xdddb2): blinking scenery
// ---------------------------------------------------------------------------------------------

/// Animated scenery that disappears 16 times in a row at each end of script (`+0x14` counts the
/// blinks); its screen margin is the D2 left by the caller.
mod flasher;

// ---------------------------------------------------------------------------------------------
// Type 3 (0xdde18): label stuck to an object
// ---------------------------------------------------------------------------------------------

/// Label that follows an object (long address in `+0x14`) with a constant offset (X + 0x50,
/// Y - 0x58) as long as it is not in state 6.
mod tag;

// ---------------------------------------------------------------------------------------------
// Type 5 (0xdde8c): ghost object
// ---------------------------------------------------------------------------------------------

/// Object with no behavior: it releases itself on its first frame.
mod ghost;

// ---------------------------------------------------------------------------------------------
// Type 6 (0xddec4): elements of the scripted scenes
// ---------------------------------------------------------------------------------------------

/// Scene elements (presentation, interludes, ending): 24 variants (`+3`), each a small
/// state machine. The variants are linked to a "master" object (`+0x4c`, often another
/// object of this type) whose byte `+8` serves as a mailbox: 1 "go", 2 "next step",
/// negative "end". Many also load palettes from the ROM (0x28f340...) into the
/// page palette 0x91c080-0x91c3ff and launch sounds (0xea2e8...).
mod scene;

// ---------------------------------------------------------------------------------------------
// Types 7, 9, 10: on-screen helps (advice bubble, blinking frame, arrow)
// ---------------------------------------------------------------------------------------------

/// State 6 flag set on exit of the help objects: state 6, substate 0.
const LEAVE_6_0: u16 = 0x0600;
/// State 6, substate 2: the object will release itself on the next frame (see `hint`).
const LEAVE_6_2: u16 = 0x0602;
/// RAM byte (0xff0ec6, A5-0x713a): the help is inhibited as long as it is non-zero.
const HINT_INHIBIT: u32 = 0xff_0ec6;

/// Reads the four players registered at 0xff2aa4 (A5-0x555c): words, 0 = absent.
const PLAYERS: u32 = 0xff_2aa4;

/// Advice bubble (type 7): table at 0x28ff20 for palette 0x91c380, animations 0x118 to
/// 0x136 (one per piece of advice) read in `+0x14`, duration of each piece of advice in `+0x16`. It is tied to
/// an object (`+0x46`) whose `+0x42` bit 1 requests the end and bit 0 signals that the advice has been read.
mod hint;

/// Attached marker (type 8): a small marker (animation 0x13b or 0x13c according to `+3`) stuck to
/// the object `+0x14`, a bit above it (0x20 or 0x10); it disappears when it leaves the screen on the left.
mod marker;

/// Blinking frame (type 9): 8 blinks of 20 frames, placed from a table (by `+3`)
/// relative to the camera, or (if `+0x2f`) on the first player at a given depth.
mod frame;

/// Advice arrow (type 10): placed on the camera, blinks 10 times (20 frames); its animation
/// and the given events (4 calls to 0x19ae6, with one code per player) come from the advice number
/// of the linked object (`+0x14` of the bubble, 0x118...). Born at the end of a bubble (type 7).
mod arrow;

// ---------------------------------------------------------------------------------------------
// Types 11, 12: interface elements stuck to the camera
// ---------------------------------------------------------------------------------------------

/// Address (word) of the camera structure in RAM: A5+0x4fe, or A5+0x54e if `A5+0x36`.
fn camera_record<R: Bench>(x: &mut Cx<'_, R>) -> u16 {
    let record = if x.a5_u8(ram::USE_CAMERA2) != 0 { 0x54e } else { 0x4fe };
    (0xff_8000u32 + record) as u16
}

/// Places the object at the position of camera structure `+0x42`, offset by (`+0x14`, `+0x18`).
fn stick_to_camera<R: Bench>(x: &mut Cx<'_, R>) {
    let camera = x.linked(0x42);
    let v = camera.u16(x.b, 0x8).wrapping_add(x.u16(0x14));
    x.set16(0x8, v);
    let v = camera.u16(x.b, 0xc).wrapping_add(x.u16(0x18));
    x.set16(0xc, v);
}

/// Interface mark (type 11): two variants. Variant 0 (the bubble) enters from the bottom
/// (Z 0x100 -> 0x80, 3 per frame), creates a variant 1 (the finger, with its parameter `+0x2c`),
/// waits for it to finish then leaves again (Z -> 0x110). Variant 1 sticks to the bubble and follows the
/// counter `+0x33` of the owner `+0x46` (blinks, sounds, notifies `+0x44` when it has used up
/// its attempts).
mod badge;

/// Interface dot (type 12): stuck to the camera (Z + 0x30), animation 0x1ab + variant.
mod hud_dot;

/// Scrolling panel (type 13): born to the right of the camera (X + 0x20, Z 0x1c8, depth -0x100),
/// advances with it until the end of its animation (script 0x3b) then orders the four registered
/// players (0xff2aa4...) to go to state 4 and releases itself.
mod panel;

// ---------------------------------------------------------------------------------------------
// Type 14 : titre de niveau
// ---------------------------------------------------------------------------------------------

/// Level title (type 14): animated banner (set 0x1c03a6, number drawn from table 0xdff0c according to
/// level `A5+0x34`) stuck to the camera, whose palette (0x91c1e0) brightens in steps of 4
/// frames in 16 stages read from 0x291f00; it lasts 0xb4 frames. Levels 7 and 0x2b have their
/// own sequence.
mod stage_title;

// ---------------------------------------------------------------------------------------------
// Type 15: target to hit (h)
// ---------------------------------------------------------------------------------------------

/// Target (type 15, variants 0x25 and others, rectangle `+0x16..+0x1c` around the object): when
/// a player touches it with an attack (player's `+0x58` positive), it takes the hit; on the third
/// hit (`+0x14`) it raises a RAM flag (0xff2ac0 for variant 0x25, 0xff2ac1
/// otherwise), plays a sound and goes to state 4. `+0x15` records that a player is inside, `+0x1e`
/// the code (word) saved at 0xff2ab2 on first contact.
mod target;

handlers! {
    0xdd61e "sobj_00" scenery::run,
    0xddc28 "sobj_01" fixture::run,
    0xdddb2 "sobj_02" flasher::run,
    0xdde18 "sobj_03" tag::run,
    0xdde8c "sobj_05" ghost::run,
    0xddec4 "sobj_06" scene::run,
    0xdf4d4 "sobj_07" hint::run,
    0xdf6b0 "sobj_08" marker::run,
    0xdf774 "sobj_09" frame::run,
    0xdf8ae "sobj_10" arrow::run,
    0xdfa68 "sobj_11" badge::run,
    0xdfca0 "sobj_12" hud_dot::run,
    0xdfd44 "sobj_13" panel::run,
    0xdfe3c "sobj_14" stage_title::run,
    0xe0258 "sobj_15" target::run,
}
