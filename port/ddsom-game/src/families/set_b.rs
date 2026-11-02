//! Scenery and animation objects (SET class, pool of 32 objects of 0x50 bytes, table 0x39eac), types 16 to 32. See
//! `docs/families/set_b.md`.
//!
//! SET objects are not in the depth-sorted lists by construction (the `reinsert` calls of the creations nevertheless
//! enter them there when they make them visible).

use crate::families::cx::Bench;
use super::cx::{handlers, Cx};
use super::effects_creators::leftovers::{Pure, OnBench};
use super::eshl_creators;
use crate::mem::{rd32, rd8, wr32, wr8, A5};
use crate::objects::Obj;
use crate::objects::pools;
use crate::objects::Current;
use crate::machine::Machine;

/// The four player records: A5+0x5de, 0x200 bytes each.
const PLAYERS: u32 = A5 + 0x5de;
const PLAYER_SIZE: u32 = 0x200;

/// Fields of a player record used by the scenery.

/// 0xc8312: SET object of the scene, of variant `variant`. Original glitch: the caller does not test the failure and
/// writes into the object that A1 leaves (`Pool::alloc_or_leftover`).
fn scene_set<R: Bench>(x: &mut Cx<'_, R>, variant: u8) -> Obj {
    match super::creators::scene_set_via(&mut OnBench(&mut x.c), x.b, variant) {
        Some(o) => o,
        None => pools::SET.alloc_or_leftover(x.b).expect_err("the SET pool was just found full"),
    }
}

mod pl {
    pub const ACTIVE: u32 = 0x00;
    pub const STATE: u32 = 0x04;
    pub const X: u32 = 0x08;
    pub const Z: u32 = 0x0c;
    pub const Y: u32 = 0x10;
    pub const HEALTH: u32 = 0x62;
}

fn players() -> impl Iterator<Item = Obj> {
    (0..4).map(|i| Obj(PLAYERS + i * PLAYER_SIZE))
}

// ---------------------------------------------------------------------------------------------
// Type 16 (0xd4110): trampoline (h)
// ---------------------------------------------------------------------------------------------

/// 0xd40e8, routine of the appearance scripts (scenes 14 and 109): releases the trampoline lock (`A5+0x46`) and creates
/// the thirteen trampolines of the scene, SET 16 of variants 13 to 1; stops if the pool is full. Returns the D5 that
/// the original leaves (loop counter: `moveq #12,D5`, high word zero; 0xFFFF after the last), a leftover reread by the
/// handlers (`dispatch::Leftovers`); A1 and D6 of the allocation are reread by nobody.
pub(crate) fn spawn_bumpers(b: &mut cps2::Cps2Bus) -> u32 {
    crate::mem::wr16(b, A5 + 0x46, 0);
    for counter in (0..13u32).rev() {
        let Some(o) = pools::SET.alloc(b) else { return counter };
        o.set_u8(b, Obj::ACTIVE, 1);
        o.set_u8(b, Obj::KIND, 0x10);
        o.set_u8(b, Obj::VARIANT, counter as u8 + 1);
    }
    0xffff
}

/// Trampoline / bumper (h): a contact zone (boxes read from the ROM according to `+0x2e` and bytes `+0x38..+0x3a`)
/// that, when a player touches it while pressing the right key, launches the player into the air (hardcoded speeds per
/// trampoline type `+0x42`, player state 0x2002400 or 0x2002402). Only one player at a time: `A5+0x46` remembers the
/// object holding the lock and `+0x14` the delay before releasing it. As long as a living player is out of range of the
/// two presence boxes, the object does nothing.
mod bumper {
    use super::*;

    const ANIMS: u32 = 0x1c4738;
    /// RAM word (A5+0x46): occupied trampoline (address of the object).
    const LOCK: u32 = 0x46;
    /// RAM byte (A5+0x50): game frozen.
    const FROZEN: u32 = 0x50;
    const LOCK_FRAMES: u16 = 0x20;
    /// Variant -> trampoline kind (`+0x42`); variant -> position X, Y (two words).
    const KIND: u32 = 0xd4198;
    const POSITION: u32 = 0xd41a6;
    /// Tables of box pointers (by `+0x2e`), 8 bytes per box (dx, half-width, dz, half-height ...): contact box,
    /// presence box 1 and 2.
    const BOXES_HIT: u32 = 0x11bffa;
    const BOXES_NEAR: u32 = 0x11c11a;
    const BOXES_NEAR2: u32 = 0x11c23a;
    const BOX_SET: u8 = 0x42;
    /// Subfields of the contact box (`+0x38`) and of the presence boxes (`+0x39`, `+0x3a`).
    const HIT_SELECTOR: u32 = 0x38;
    const NEAR_SELECTOR: u32 = 0x39;
    const NEAR2_SELECTOR: u32 = 0x3a;
    /// States of the launched player.
    const AIRBORNE: u32 = 0x0200_2400;
    const AIRBORNE_BACK: u32 = 0x0200_2402;

    /// A launch: player key (bit of `+0x6c`), speeds, initial height, damping (`+0x1e`), arrival state, and whether the
    /// orientation is reset to zero.
    struct Launch {
        button: u8,
        vx: i16,
        vy: i16,
        z: u32,
        drag: i16,
        state: u32,
        reset_flip: bool,
    }

    const LAUNCHES: [Launch; 8] = [
        Launch { button: 0, vx: 0x300, vy: 0x400, z: 0x2_0000, drag: -0x20, state: AIRBORNE, reset_flip: false },
        Launch { button: 1, vx: -0x380, vy: 0x380, z: 0x2_0000, drag: -0x20, state: AIRBORNE, reset_flip: false },
        Launch { button: 2, vx: 0, vy: 0, z: 0x2_0000, drag: -0x30, state: AIRBORNE, reset_flip: false },
        // entry 3 is special (see `kick_up`)
        Launch { button: 3, vx: 0, vy: 0, z: 0, drag: 0, state: 0, reset_flip: false },
        Launch { button: 0, vx: 0x480, vy: 0, z: 0x2_0000, drag: -0x30, state: AIRBORNE_BACK, reset_flip: false },
        Launch { button: 2, vx: -0x200, vy: 0, z: 0x3_0000, drag: -0x30, state: AIRBORNE_BACK, reset_flip: true },
        Launch { button: 0, vx: 0x300, vy: 0x500, z: 0x2_0000, drag: -0x20, state: AIRBORNE, reset_flip: false },
        Launch { button: 2, vx: -0x300, vy: 0, z: 0x2_4000, drag: -0x30, state: AIRBORNE_BACK, reset_flip: true },
    ];

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        let lock = x.a5_u16(LOCK);
        if lock != 0 {
            if lock != x.o.0 as u16 {
                return;
            }
            let n = x.u16(0x14).wrapping_sub(1);
            x.set16(0x14, n);
            if (n as i16) < 0 {
                x.set_a5_u16(LOCK, 0);
            }
        }
        if x.u8(0x40) != 0 || x.a5_u8(FROZEN) != 0 {
            return;
        }
        match x.u8(0x4) {
            0 => init(x),
            2 => watch(x),
            _ => x.free(&pools::SET),
        }
    }

    fn init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        x.set8(0x2e, BOX_SET);
        let variant = x.u8(0x3) as u32;
        let kind = x.rom8(KIND + variant);
        x.set8(0x42, kind);
        let px = x.rom16(POSITION + 4 * variant);
        x.set16(0x8, px);
        let py = x.rom16(POSITION + 4 * variant + 2);
        x.set16(0x10, py);
        x.set16(0xc, 0);
        x.anim_start(ANIMS, variant as u16);
        x.set32(0x34, 0);
        x.reinsert();
    }

    fn watch<R: Bench>(x: &mut Cx<'_, R>) {
        for p in players() {
            if p.u8(x.b, pl::ACTIVE) == 0 || p.u16(x.b, pl::HEALTH) == 0 {
                continue;
            }
            if !within(x, p, BOXES_NEAR, NEAR_SELECTOR) && !within(x, p, BOXES_NEAR2, NEAR2_SELECTOR) {
                return;
            }
        }
        for p in players() {
            if p.u8(x.b, pl::ACTIVE) != 0 && keys(x, p) {
                launch(x, p);
            }
        }
    }

    /// Address of the box of row `selector` of the set `+0x2e` of the table `table`.
    fn box_of<R: Bench>(x: &mut Cx<'_, R>, table: u32, selector: u32) -> u32 {
        let set = x.u8(0x2e) as u32;
        let boxes = x.data32(table + 4 * set);
        boxes.wrapping_add(8 * x.u8(selector) as u32)
    }

    /// Test of an axis: the corrected distance is in the interval (unsigned comparison).
    fn in_range(delta: u16, half: u8, shift: Option<i8>) -> bool {
        let mut d = delta.wrapping_add(half as u16);
        if let Some(s) = shift {
            d = d.wrapping_sub(s as i16 as u16);
        }
        d <= (half as u16).wrapping_mul(2)
    }

    /// The player is in a presence box (box offset by two offset bytes).
    fn within<R: Bench>(x: &mut Cx<'_, R>, p: Obj, table: u32, selector: u32) -> bool {
        let b = box_of(x, table, selector);
        let dx = p.u16(x.b, pl::X).wrapping_sub(x.u16(0x8));
        let shift_x = x.data8(b) as i8;
        let half_x = x.data8(b + 1);
        if !in_range(dx, half_x, Some(shift_x)) {
            return false;
        }
        let dy = p.u16(x.b, pl::Y).wrapping_sub(x.u16(0x10));
        let shift_y = x.data8(b + 2) as i8;
        let half_y = x.data8(b + 3);
        in_range(dy, half_y, Some(shift_y))
    }

    /// The player is in the contact box and in a state that allows it.
    fn keys<R: Bench>(x: &mut Cx<'_, R>, p: Obj) -> bool {
        let b = box_of(x, BOXES_HIT, HIT_SELECTOR);
        if p.u8(x.b, 5) != 0 || p.u8(x.b, 0x8a) != 0 {
            return false;
        }
        if p.u8(x.b, 6) == 0x22 || p.u16(x.b, pl::STATE) == 0x204 || p.u8(x.b, 6) == 0xe {
            return false;
        }
        if p.u16(x.b, pl::HEALTH) == 0 {
            return false;
        }
        if p.u32(x.b, pl::STATE) & 0xffff_ff00 == 0x0200_2000 {
            return false;
        }
        let dx = p.u16(x.b, pl::X).wrapping_sub(x.u16(0x8));
        if !in_range(dx, x.data8(b + 1), None) {
            return false;
        }
        let dy = p.u16(x.b, pl::Y).wrapping_sub(x.u16(0x10));
        in_range(dy, x.data8(b + 3), None)
    }

    /// Launches the player according to the trampoline kind (`+0x42`).
    fn launch<R: Bench>(x: &mut Cx<'_, R>, p: Obj) {
        let kind = x.u8(0x42) as usize;
        if kind >= LAUNCHES.len() {
            return;
        }
        if kind == 3 {
            return kick_up(x, p);
        }
        let l = &LAUNCHES[kind];
        if p.u8(x.b, 0x6e) & 0x20 == 0 || (p.u16(x.b, pl::Z) as i16) > 0 || p.u8(x.b, 0x6c) >> l.button & 1 == 0 {
            return;
        }
        grab(x);
        p.set_u8(x.b, 0x66, 0);
        p.set_u32(x.b, pl::Z, l.z);
        p.set_u16(x.b, 0x16, 0);
        p.set_u16(x.b, 0x18, 0);
        p.set_u16(x.b, 0x14, l.vx as u16);
        p.set_u16(x.b, 0x1c, l.vy as u16);
        p.set_u16(x.b, 0x1a, 0xffff);
        p.set_u16(x.b, 0x1e, l.drag as u16);
        if l.reset_flip {
            p.set_u8(x.b, 0x2c, 0);
        }
        p.set_u32(x.b, pl::STATE, l.state);
    }

    /// Takes the lock and arms the release delay.
    fn grab<R: Bench>(x: &mut Cx<'_, R>) {
        x.set_a5_u16(LOCK, x.o.0 as u16);
        x.set16(0x14, LOCK_FRAMES);
    }

    /// Kind 3: pushes a player who presses key 3 and whose height is at least 0x28: the player is moved by 0x28 in
    /// depth and lowered by as much.
    fn kick_up<R: Bench>(x: &mut Cx<'_, R>, p: Obj) {
        if p.u8(x.b, 0x6c) & 8 == 0 || (p.u16(x.b, pl::Z) as i16) < 0x28 {
            return;
        }
        grab(x);
        let y = p.u16(x.b, pl::Y).wrapping_add(0x28);
        p.set_u16(x.b, pl::Y, y);
        let z = p.u16(x.b, pl::Z).wrapping_sub(0x28);
        p.set_u16(x.b, pl::Z, z);
    }
}

/// Work RAM variables (A5+): camera abscissa and ordinate, secondary camera.
mod ram {
    pub const CAMERA_X: u32 = 0x506;
    pub const CAMERA_Y: u32 = 0x50a;
    pub const CAMERA2_Y: u32 = 0x55a;
}

/// Freezes the game and darkens the screen: `A5+0x490` = 1, `A5+0x48e` = 0xc, `A5+0x48f` = `flag` steps, palettes
/// whitened (0x1686) then fade task 0x14de (`tasks/fading.rs`).
fn show_message<R: Bench>(x: &mut Cx<'_, R>, flag: u8) {
    x.set_a5_u16(0x490, 1);
    x.set_a5_u8(0x48e, 0xc);
    x.set_a5_u8(0x48f, flag);
    crate::palette::full_brightness(x.b);
    start_fade_task(x, FADE_TO_BLACK_TASK);
}

/// Fade-to-black task (steps of state `A5+0x48f`).
const FADE_TO_BLACK_TASK: u32 = 0x14de;
/// Slot of the fade task (offset in the task table).
const FADE_TASK_SLOT: u32 = 0x10;

/// Creates the fade task `entry` (0x9b6, D0 = 0x10, A0 = the entry).
fn start_fade_task<R: Bench>(x: &mut Cx<'_, R>, entry: u32) {
    crate::kernel::create_task(x.b, FADE_TASK_SLOT, entry);
}

// ---------------------------------------------------------------------------------------------
// Type 17 (0xd466a): boss arrival sequence (h)
// ---------------------------------------------------------------------------------------------

/// Sequencer of the boss arrival cutscene (h): a series of 13 steps (`+5`) that wait for the camera to reach an
/// abscissa (0x100, 0x180, 0x200, 0x2b0) or for a countdown (`+0x42`) to run out, and each time launch sounds, messages
/// (0x2824, 0x2860, 0x289c, 0xeda26, 0xeda66...) and random explosions; the last step creates the boss (BOSS object of
/// type 2).
mod boss_intro {
    use super::*;

    const STEP: u32 = 0x42;
    const BLAST: u32 = 0x46;
    /// Sound routines (D0 = number).
    const EDA26: u32 = 0xeda26;
    const EDA66: u32 = 0xeda66;
    const ED94A: u32 = 0xed94a;
    const ED9BC: u32 = 0xed9bc;
    /// Offset tables (16 words each) of the explosion in X and in Y.
    const BLAST_DX: u32 = 0xd4910;
    const BLAST_DY: u32 = 0xd4930;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(BLAST, 0x14);
            }
            2 => step(x),
            4 => x.add8(0x4, 2),
            _ => x.free(&pools::SET),
        }
    }

    fn camera_x<R: Bench>(x: &mut Cx<'_, R>) -> i16 {
        x.a5_u16(ram::CAMERA_X) as i16
    }

    /// Goes to the next step and arms the step's countdown.
    fn next<R: Bench>(x: &mut Cx<'_, R>, frames: u8) {
        x.set8(STEP, frames);
        x.add8(0x5, 2);
    }

    fn countdown<R: Bench>(x: &mut Cx<'_, R>) -> bool {
        let n = x.u8(STEP).wrapping_sub(1);
        x.set8(STEP, n);
        n == 0
    }

    fn step<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x5) {
            0 => {
                x.add8(0x5, 2);
                freeze_and_show(x);
            }
            2 => {
                if camera_x(x) >= 0x100 {
                    crate::sound::play(x.b, EDA26, x.o);
                    x.add8(0x5, 2);
                    crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::A, 0x1e);
                }
            }
            4 => {
                if camera_x(x) >= 0x180 {
                    next(x, 0x28);
                    crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::A, 0x28);
                }
            }
            6 => {
                if countdown(x) {
                    next(x, 0x28);
                    crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::B, 0x28);
                }
            }
            8 => {
                if countdown(x) {
                    next(x, 0x28);
                    crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::A, 0x28);
                }
            }
            10 => {
                if countdown(x) {
                    x.add8(0x5, 2);
                    crate::sound::play(x.b, EDA66, x.o);
                }
            }
            12 => {
                if camera_x(x) >= 0x200 {
                    next(x, 0x32);
                    crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::B, 0x32);
                    crate::sound::play(x.b, EDA26, x.o);
                }
            }
            14 => {
                if countdown(x) {
                    eshl_creators::single_25(&mut Pure, x.b, x.o);
                    next(x, 0x32);
                    crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::C, 0x32);
                }
            }
            16 => explosions(x),
            18 => {
                if countdown(x) {
                    x.add8(0x5, 2);
                    crate::sound::play(x.b, EDA66, x.o);
                }
            }
            20 => {
                if camera_x(x) >= 0x2b0 {
                    next(x, 0x28);
                    crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::C, 0x50);
                    crate::sound::play(x.b, EDA26, x.o);
                }
            }
            22 => spawn_boss(x),
            24 => {
                let n = x.u8(BLAST).wrapping_sub(1);
                x.set8(BLAST, n);
                if n == 0 {
                    crate::sound::play(x.b, EDA66, x.o);
                }
                let n = x.u8(STEP).wrapping_sub(1);
                x.set8(STEP, n);
                if (n as i8) < 0 {
                    x.add8(0x4, 2);
                }
            }
            _ => {}
        }
    }

    /// Step 0: freezes the game (`A5+0x48e` = 0xc, `A5+0x48f` = 7), starts the fade 0x1686 and the fade task 0x14de
    /// (0x9b6).
    fn freeze_and_show<R: Bench>(x: &mut Cx<'_, R>) {
        show_message(x, 7);
    }

    /// Step 16: every 20 frames a hit of ED94A; when `+0x42` is a multiple of 4 a random explosion; at 0, end.
    fn explosions<R: Bench>(x: &mut Cx<'_, R>) {
        let n = x.u8(BLAST).wrapping_sub(1);
        x.set8(BLAST, n);
        if (n as i8) < 0 {
            crate::sound::play(x.b, ED94A, x.o);
            x.set8(BLAST, 0x14);
        }
        if x.u8(STEP) & 3 == 0 {
            eshl_creators::single_25(&mut Pure, x.b, x.o);
            random_blast(x);
        }
        if countdown(x) {
            x.add8(0x5, 2);
            crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::B, 0x32);
        }
    }

    fn random_blast<R: Bench>(x: &mut Cx<'_, R>) {
        let rng = x.random();
        let i = (rng as u32 & 0xf) << 1;
        let dx = x.rom16(BLAST_DX + i);
        let rng = x.random();
        let i = (rng as u32 & 0xf) << 1;
        let dy = x.rom16(BLAST_DY + i);
        let px = dx.wrapping_add(x.a5_u16(ram::CAMERA_X));
        let py = dy.wrapping_add(x.a5_u16(ram::CAMERA_Y));
        crate::families::enemy_common::enemy_blast_at(x, px, py);
    }

    /// Step 22: 30 smokes, then the boss (BOSS type 2) appears.
    fn spawn_boss<R: Bench>(x: &mut Cx<'_, R>) {
        if !countdown(x) {
            return;
        }
        // 0xb4bcc thirty times: the variant is the loop counter (D1, 0x1d down to 0), the setting zero
        for n in (0..=0x1d).rev() {
            crate::families::effects_creators::debris_rain(x.b, x.o, n, 0);
        }
        crate::sound::play(x.b, ED9BC, x.o);
        x.set_a5_u8(0x48e, 0xf);
        crate::palette::full_brightness(x.b);
        let Some(boss) = x.alloc(&pools::BOSS) else { return };
        boss.set_u8(x.b, 0, 1);
        boss.set_u8(x.b, 2, 2);
        x.add8(0x5, 2);
        x.set8(STEP, 0x3c);
        x.set8(BLAST, 0x28);
    }
}

// ---------------------------------------------------------------------------------------------
// Type 18 (0xd4950): vertical scroll toward a target
// ---------------------------------------------------------------------------------------------

/// Makes the secondary camera descend one pixel per frame down to the height `+0x42`, then frees the linked object
/// (`+0x46`) and notifies routine 0x2824.
mod scroll_to {
    use super::*;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => x.add8(0x4, 2),
            2 => {
                let y = x.a5_u32(ram::CAMERA2_Y).wrapping_add(0x10000);
                x.set_a5_u32(ram::CAMERA2_Y, y);
                let target = x.u16(0x42);
                if target as i16 > (y >> 16) as i16 {
                    return;
                }
                x.set_a5_u16(ram::CAMERA2_Y, target);
                if x.u16(0x46) != 0 {
                    let linked = x.linked(0x46);
                    linked.set_u8(x.b, 0x2e, 0);
                    let n = linked.u16(x.b, 0x2a).wrapping_sub(1);
                    linked.set_u16(x.b, 0x2a, n);
                    crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::A, 0x10);
                }
                x.free(&pools::SET);
            }
            _ => x.free(&pools::SET),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 19 (0xd49a2): enemy wave generator (h)
// ---------------------------------------------------------------------------------------------

/// Wave generator (h): it triggers when a living player comes within range (0xc0 in X, 0x30 in depth), then reads a ROM
/// script (entries of 0x14 bytes, one list per variant) and creates the described objects (enemies, bosses or scenery)
/// when the game is in play (`A5+0x2c` = 0xc), keeping the count of the objects still alive (`+0x2a`, which these
/// objects decrement at their death by finding their generator in `+0x46`).
///
/// Script entry: +0 word (> 0: delay before the next; < 0: wait until the number of living objects is at most the low
/// byte, bit 7 of this byte: wait until none lives), +2 minimum rank, +4/+6/+8 position (X, Z, Y), +0xa minimum number
/// of players, +0xb creation routine (byte index in table 0xd4b92), +0xc type, +0xd variant, +0xe/+0xf parameters,
/// +0x10 word `+0x42`, +0x12 absolute position if non-zero (otherwise relative to the camera). A word 0x8000 ends the
/// script.
mod wave_spawner {
    use super::*;

    /// Script pointers, per variant (data space).
    const SCRIPTS: u32 = 0xd4c5e;
    /// Variant -> shape of the start / end (bytes, program space).
    const START_KIND: u32 = 0xd4a0e;
    const END_KIND: u32 = 0xd4bdc;
    /// Creation routines by index (longs, data space).
    const CREATORS: u32 = 0xd4b92;
    const ENTRY_SIZE: u32 = 0x14;
    const SCRIPT_END: u16 = 0x8000;
    /// RAM byte (A5+0x51): active generators. Byte (A5+0x2c): game mode (0xc: in play).
    const ACTIVE_COUNT: u32 = 0x51;
    const GAME_MODE: u32 = 0x2c;
    const PLAYING: u8 = 0xc;
    /// RAM word (A5+0x51e): scroll limit imposed by the wave.
    const SCROLL_LIMIT: u32 = 0x51e;
    /// Fields of the object: global delay, script pointer, wait, threshold, alive.
    const TIMER: u32 = 0x24;
    const CURSOR: u32 = 0x20;
    const DELAY: u32 = 0x26;
    const THRESHOLD: u32 = 0x18;
    const ALIVE: u32 = 0x2a;
    const DONE: u32 = 0x2c;
    const ABORT: u32 = 0x2d;
    const WAIT_ALL: u32 = 0x2e;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(0x40) != 0 {
            return;
        }
        match x.u8(0x4) {
            0 => start(x),
            2 => play(x),
            4 => finish(x),
            _ => x.free(&pools::SET),
        }
    }

    /// A living player is within range of the generator.
    fn player_near<R: Bench>(x: &mut Cx<'_, R>) -> bool {
        for p in players() {
            if p.u8(x.b, pl::ACTIVE) == 0 || p.u16(x.b, pl::HEALTH) == 0 {
                continue;
            }
            let dx = p.u16(x.b, pl::X).wrapping_sub(x.u16(0x8)).wrapping_add(0xc0);
            if dx > 0x180 {
                continue;
            }
            let dy = p.u16(x.b, pl::Y).wrapping_sub(x.u16(0x10)).wrapping_add(0x30);
            if dy <= 0x60 {
                return true;
            }
        }
        false
    }

    fn start<R: Bench>(x: &mut Cx<'_, R>) {
        if !player_near(x) {
            return;
        }
        let n = x.a5_u8(ACTIVE_COUNT).wrapping_add(1);
        x.set_a5_u8(ACTIVE_COUNT, n);
        x.set16(THRESHOLD, 0xffff);
        x.set16(ALIVE, 0);
        x.set8(DONE, 0);
        x.set8(ABORT, 0);
        let variant = x.u8(0x3) as u32;
        let script = x.data32(SCRIPTS + 4 * variant);
        let timer = x.data16(script);
        x.set16(TIMER, timer);
        x.set32(CURSOR, script + 2);
        let me = x.o;
        crate::families::set_residue::begin_wave(x.b, me);
        match x.rom8(START_KIND + variant) {
            0 => x.set_a5_u16(SCROLL_LIMIT, 0x20),
            2 => x.set_a5_u16(SCROLL_LIMIT, 4),
            _ => {}
        }
        x.add8(0x4, 2);
    }

    /// End of the run (stop requested, time elapsed or script finished).
    fn stop<R: Bench>(x: &mut Cx<'_, R>) {
        x.set8(DONE, 1);
        end_script(x);
    }

    fn end_script<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        x.set16(DELAY, 0);
    }

    fn play<R: Bench>(x: &mut Cx<'_, R>) {
        if x.a5_u8(GAME_MODE) != PLAYING {
            return;
        }
        if x.a5_u8(0x42) != 0 || x.u8(ABORT) != 0 {
            return stop(x);
        }
        let n = x.u16(TIMER).wrapping_sub(1);
        x.set16(TIMER, n);
        if (n as i16) < 0 {
            return stop(x);
        }
        if x.u8(WAIT_ALL) != 0 {
            return;
        }
        let threshold = x.u16(THRESHOLD) as i16;
        if threshold < 0 {
            let d = x.u16(DELAY).wrapping_sub(1);
            x.set16(DELAY, d);
            if (d as i16) >= 0 {
                return;
            }
        } else if threshold < x.u16(ALIVE) as i16 {
            return;
        }
        next_entry(x);
    }

    /// Executes the current script entry (skipping those that the number of players or the rank exclude).
    fn next_entry<R: Bench>(x: &mut Cx<'_, R>) {
        let mut e = x.u32(CURSOR);
        loop {
            let head = x.data16(e);
            if head == SCRIPT_END {
                return end_script(x);
            }
            let players_needed = x.data8(e + 0xa) as i8;
            let rank_needed = x.data16(e + 2) as i16;
            if players_needed > x.a5_u8(0x35) as i8 || rank_needed > x.a5_u16(0x7f92) as i16 {
                e += ENTRY_SIZE;
                x.set32(CURSOR, e);
                continue;
            }
            return run_entry(x, e, head);
        }
    }

    fn run_entry<R: Bench>(x: &mut Cx<'_, R>, e: u32, head: u16) {
        x.set8(WAIT_ALL, 0);
        x.set16(THRESHOLD, 0xffff);
        x.set16(DELAY, head);
        if (head as i16) < 0 {
            let low = head & 0xff;
            if low as u8 as i8 <= -1 {
                x.set8(WAIT_ALL, 1);
            }
            x.set16(THRESHOLD, low);
        }
        let index = x.data8(e + 0xb) as u32;
        let pool = match x.data32(CREATORS + index) {
            0x22bc => &pools::ENEMY,
            0x23a8 => &pools::BOSS,
            0x2348 => &pools::SET,
            r => unreachable!("wave creator {r:#x}: table 0xd4b92 only has the ENEMY, BOSS, SET allocations"),
        };
        let Some(o) = pool.alloc(x.b) else {
            x.set16(THRESHOLD, 0xffff);
            x.set16(DELAY, 0);
            return;
        };
        o.set_u8(x.b, 0, 1);
        let v = x.data8(e + 0xc);
        o.set_u8(x.b, 2, v);
        let v = x.data8(e + 0xd);
        o.set_u8(x.b, 3, v);
        let v = x.data8(e + 0xe);
        o.set_u8(x.b, 0x2f, v);
        let v = x.data8(e + 0xf);
        o.set_u8(x.b, 0x2c, v);
        let v = x.data16(e + 4);
        o.set_u16(x.b, 0x8, v);
        let v = x.data16(e + 6);
        o.set_u16(x.b, 0xc, v);
        let v = x.data16(e + 8);
        o.set_u16(x.b, 0x10, v);
        let v = x.data16(e + 0x10);
        o.set_u16(x.b, 0x42, v);
        x.add16(ALIVE, 1);
        let me = x.o.0 as u16;
        o.set_u16(x.b, 0x46, me);
        if x.data8(e + 0x12) == 0 {
            // position relative to the camera
            let camera = if x.a5_u8(0x36) != 0 { 0x54e } else { 0x4fe };
            let cam = Obj(A5 + camera);
            let v = o.u16(x.b, 0x8).wrapping_add(cam.u16(x.b, 0x8));
            o.set_u16(x.b, 0x8, v);
            let v = o.u16(x.b, 0x10).wrapping_add(cam.u16(x.b, 0xc));
            o.set_u16(x.b, 0x10, v);
        }
        x.set32(CURSOR, e + ENTRY_SIZE);
    }

    /// No more living objects: releases the count, lets the variant conclude and releases the scroll limit.
    fn finish<R: Bench>(x: &mut Cx<'_, R>) {
        let n = x.u16(TIMER).wrapping_sub(1);
        x.set16(TIMER, n);
        if (n as i16) < 0 {
            x.set8(DONE, 1);
            x.set16(TIMER, 0x7fff);
        }
        if x.u16(ALIVE) != 0 {
            return;
        }
        let n = x.a5_u8(ACTIVE_COUNT).wrapping_sub(1);
        x.set_a5_u8(ACTIVE_COUNT, if (n as i8) < 0 { 0 } else { n });
        let me = x.o;
        crate::families::set_residue::finish_wave(x.b, me);
        let variant = x.u8(0x3) as u32;
        match x.rom8(END_KIND + variant) {
            0 => {
                // sets the scroll limit at the current camera
                let v = x.a5_u16(ram::CAMERA_X);
                x.set_a5_u16(0x52c, v);
                x.set_a5_u16(SCROLL_LIMIT, 0);
            }
            2 => x.set_a5_u16(SCROLL_LIMIT, 0),
            _ => {}
        }
        x.add8(0x4, 2);
    }
}

// ---------------------------------------------------------------------------------------------
// Type 20 (0xd5f5a): health bar
// ---------------------------------------------------------------------------------------------

/// Health bar of a player (or of two players together if `+3`), drawn in three columns of 44 cells of background layer
/// 1 (GFX RAM 0x91215c, 0x912160, 0x912164; 0x80 bytes per line). The bar fills in pieces of 8 hit points: the
/// displayed life (`+0x2a`) catches up with the real life (`+0x32`, sum of `+0x62` of the player) by one point per
/// frame, advancing the write cursor (`+0x22`) and its sub-step (`+0x26`). Beyond 0x160 points the bar moves to the
/// next column. `+0x30` = maximum life (empty bar on the right), `+0x2c` / `+0x27` = length of the background in whole
/// pieces and remainder.
mod life_bar {
    use super::*;
    use crate::mem::{wr16, wr32};

    const COLUMN_A: u32 = 0x91215c;
    const COLUMN_B: u32 = 0x912160;
    const COLUMN_C: u32 = 0x912164;
    const ROW: u32 = 0x80;
    /// Column reset to empty when the bar falls back below 0x160.
    const CURSOR_AFTER_RESET: u32 = 0x913760;
    const ROWS: u32 = 44;
    const MAX_LEN: i16 = 0x2c0;
    const SECOND_COLUMN_FROM: i16 = 0x160;
    /// Full cell, empty cell.
    const FULL: u32 = 0x1318_000d;
    const BLANK: u32 = 0x0020_0000;
    /// Attribute of the pieces.
    const PIECE_ATTR: u16 = 0xd;
    /// Partial pieces: word table indexed by the number of points (1 to 8) * 2.
    const PIECES: u32 = 0xd61b2;
    /// Pointers (data space) to the trim (bytes, end 0x40) according to the player's type.
    const FRAMES: u32 = 0xd6232;
    const FRAME_SHARED: u32 = 0xd628c;
    const FRAME_END: u8 = 0x40;
    const FRAME_ATTR: u16 = 2;
    const CAP: u32 = 0x003f_0002;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        let owner = x.linked(0x20);
        let (hp, max) = (owner.u16(x.b, 0x62), owner.u16(x.b, 0x60));
        x.set16(0x32, hp);
        x.set16(0x30, max);
        if x.u8(0x3) != 0 {
            let second = x.linked(0x34);
            let (hp2, max2) = (second.u16(x.b, 0x62), second.u16(x.b, 0x60));
            x.add16(0x32, hp2);
            x.add16(0x30, max2);
        }
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                draw_frame(x);
                bar(x);
            }
            2 => bar(x),
            4 => dismiss(x),
            _ => x.free(&pools::SET),
        }
    }

    fn bar<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x5) {
            0 => draw_full(x),
            2 => animate(x),
            _ => {}
        }
    }

    fn clear_column<R: Bench>(x: &mut Cx<'_, R>, mut at: u32) {
        for _ in 0..ROWS {
            wr32(x.b, at, BLANK);
            at += ROW;
        }
    }

    /// End: columns emptied, object freed.
    fn dismiss<R: Bench>(x: &mut Cx<'_, R>) {
        x.set8(0x4, 6);
        for c in [COLUMN_A, COLUMN_B, COLUMN_C] {
            clear_column(x, c);
        }
        x.free(&pools::SET);
    }

    /// Trim of the left column (cells of the byte list of the player type), and cap of the next column when the life
    /// exceeds the maximum length.
    fn draw_frame<R: Bench>(x: &mut Cx<'_, R>) {
        let player = x.linked(0x20);
        let kind = player.u8(x.b, 2);
        let mut list = x.data32(FRAMES + 4 * kind as u32);
        if x.u8(0x3) != 0 && kind == 0 {
            list = FRAME_SHARED;
        }
        let mut at = COLUMN_A;
        loop {
            let tile = x.data8(list);
            list += 1;
            if tile == FRAME_END {
                break;
            }
            wr16(x.b, at, tile as u16);
            wr16(x.b, at + 2, FRAME_ATTR);
            at += ROW;
        }
        if x.u16(0x32) as i16 <= MAX_LEN {
            return;
        }
        let mut at = COLUMN_B;
        for _ in 0..5 {
            wr32(x.b, at, CAP);
            at += ROW;
        }
    }

    /// Cell of the piece of `d1` points (at most 8), offset by `d0`.
    fn piece<R: Bench>(x: &mut Cx<'_, R>, at: u32, d0: i16, d1: i16) {
        let points = d1.min(8);
        let base = x.rom16(PIECES.wrapping_add((points.wrapping_mul(2)) as i32 as u32));
        wr16(x.b, at, base.wrapping_add(d0 as u16));
        wr16(x.b, at + 2, PIECE_ATTR);
    }

    /// Substate 0: draws the whole bar (the displayed life takes the real life).
    fn draw_full<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u16(0x32) as i16 > MAX_LEN {
            return;
        }
        x.add8(0x5, 2);
        let mut at = COLUMN_B;
        let mut d0 = x.u16(0x32) as i16;
        x.set16(0x2a, d0 as u16);
        let mut d1 = x.u16(0x30) as i16;
        if d1 == 0 {
            return;
        }
        d1 = d1.min(MAX_LEN);
        x.set16(0x2c, d1 as u16 & !7);
        x.set8(0x27, d1 as u8 & 7);
        let mut columns_left = 2;
        loop {
            let mut d2: i16 = (ROWS - 1) as i16;
            loop {
                d1 -= 8;
                d0 -= 8;
                if d0 <= 0 {
                    d1 += 8;
                    d0 += 8;
                    if d0 == 0 {
                        return tail(x, at, d0, d1, d2, true);
                    }
                    return partial(x, at, d0, d1, d2);
                }
                wr32(x.b, at, FULL);
                at += ROW;
                d2 -= 1;
                if d2 == -1 {
                    break;
                }
            }
            if d0 == 0 {
                return tail(x, at - ROW, 0, d1, d2, true);
            }
            columns_left -= 1;
            if columns_left == 0 {
                return tail(x, at, d0, d1, d2, true);
            }
            at = COLUMN_C;
        }
    }

    /// Last piece of the full part, then the empty part.
    fn partial<R: Bench>(x: &mut Cx<'_, R>, at: u32, d0: i16, d1: i16, d2: i16) {
        piece(x, at, d0, d1);
        if d1 <= 8 {
            return tail(x, at, d0, d1, d2, true);
        }
        x.set32(0x22, at);
        tail(x, at + ROW, d0, d1, d2 - 1, false)
    }

    /// Remembers the cursor (`+0x22`) and the sub-step (`+0x26`), then draws the remaining part (empty pieces) until
    /// `d1` is exhausted.
    fn tail<R: Bench>(x: &mut Cx<'_, R>, mut at: u32, d0: i16, mut d1: i16, mut d2: i16, set_cursor: bool) {
        if set_cursor {
            x.set32(0x22, at);
        }
        x.set8(0x26, d0 as u8);
        if x.u8(0x26) == 0 {
            x.set8(0x26, 8);
        }
        loop {
            d1 -= 8;
            if d1 <= 0 {
                return;
            }
            piece(x, at, 0, d1);
            at += ROW;
            d2 -= 1;
            if d2 == -1 {
                d2 = (ROWS - 1) as i16;
                at = COLUMN_C;
            }
        }
    }

    /// Substate 2: the displayed life catches up with the real life by one point per frame.
    fn animate<R: Bench>(x: &mut Cx<'_, R>) {
        let shown = x.u16(0x2a) as i16;
        if shown == 0 {
            return dismiss(x);
        }
        let target = x.u16(0x32) as i16;
        if shown == target {
            return;
        }
        let mut at = x.u32(0x22);
        let full_len = x.u16(0x2c) as i16;
        if shown > target {
            let d0 = shown - 1;
            x.set16(0x2a, d0 as u16);
            if d0 == SECOND_COLUMN_FROM {
                clear_column(x, COLUMN_C);
                x.set8(0x26, 0);
                x.set32(0x22, CURSOR_AFTER_RESET);
                return;
            }
            let n = x.u8(0x26).wrapping_sub(1);
            x.set8(0x26, n);
            if (n as i8) < 0 {
                x.set8(0x26, 7);
                at -= ROW;
            }
            let d1 = if d0 < full_len { 8 } else { x.u8(0x27) as i16 };
            let sub_step = x.u8(0x26) as i16;
            piece(x, at, sub_step, d1);
            x.set32(0x22, at);
        } else {
            let d0 = shown + 1;
            x.set16(0x2a, d0 as u16);
            if d0 == SECOND_COLUMN_FROM + 1 {
                x.set8(0x26, 1);
                let d1 = (x.u16(0x30) as i16).min(MAX_LEN) - SECOND_COLUMN_FROM;
                partial(x, COLUMN_C, d0 - SECOND_COLUMN_FROM, d1, (ROWS - 1) as i16);
                x.set32(0x22, COLUMN_C);
                return;
            }
            let n = x.u8(0x26) + 1;
            x.set8(0x26, n);
            if n == 9 {
                x.set8(0x26, 1);
                at += ROW;
            }
            let d1 = if d0 <= full_len { 8 } else { x.u8(0x27) as i16 };
            let sub_step = x.u8(0x26) as i16;
            piece(x, at, sub_step, d1);
            x.set32(0x22, at);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 21 (0xd6372): continue countdown
// ---------------------------------------------------------------------------------------------

/// Display of the countdown of a player who can continue (h): three digits (`+0x14`, `+0x15`, `+0x16`, from 0:00 to
/// 9:59 (h)) drawn in background layer 1 (0x912b38) and counted down by the second (`+0x17` counts 60 frames); the
/// "ready" inscription blinks beside it (rows of constant cells at 0x912830). At zero, the players still alive are put
/// to death. Variant 1: version without count (invitation only), variant 2: does not depend on `A5+0x50`. States: 0
/// initialization (palette 0x91c420), 2 counting, 4 end of game (final inscription at 0x9127b0), 6 release. The sound
/// script 0xd6922 announces the display, 0xd694e its removal.
mod continue_countdown {
    use super::*;
    use crate::mem::{rd8, wr16, wr32};

    const PALETTE_SRC: u32 = 0x25c0e0;
    const PALETTE_DST: u32 = 0x91c420;
    const BRIGHT: u32 = 0xf000_f000;
    /// Byte (A5-0x5b8e = 0xff2472): sound muted.
    const MUTED: u32 = 0xff_2472;
    const SCRIPT_ON: u32 = 0xd6922;
    const SCRIPT_OFF: u32 = 0xd694e;
    /// Variant -> invitation parameter (`+0x2c`), according to the level `A5+0x34` (bytes).
    const PROMPT_BY_STAGE: u32 = 0xd6464;
    const DIGITS: u32 = 0x912b38;
    const GET_READY: u32 = 0x912830;
    const GAME_OVER: u32 = 0x9127b0;
    const ROW: u32 = 0x80;
    const ATTR: u32 = 3;
    const COLON: u16 = 0x3a;
    /// Rows (left tile, right tile) of the blinking inscription (attribute 3).
    const GET_READY_ROWS: [(u16, u16); 16] = [
        (0x192, 0x1a2), (0x193, 0x1a3), (0x1bc, 0x1cc), (0x1bd, 0x1cd),
        (0x1b6, 0x1c6), (0x1b7, 0x1c7), (0x1b6, 0x1c6), (0x1b7, 0x1c7),
        (0x1d4, 0x1e4), (0x1d5, 0x1e5), (0x2fc, 0x2fc), (0x2fc, 0x2fc),
        (0x1bc, 0x1cc), (0x1bd, 0x1cd), (0x1b2, 0x1c2), (0x1b3, 0x1c3),
    ];
    /// End-of-game inscription.
    const GAME_OVER_ROWS: [(u16, u16); 18] = [
        (0x1ba, 0x1ca), (0x1bb, 0x1cb), (0x194, 0x1a4), (0x195, 0x1a5),
        (0x19c, 0x1ac), (0x19d, 0x1ad), (0x17c, 0x18c), (0x17d, 0x18d),
        (0x2fc, 0x2fc), (0x2fc, 0x2fc), (0x1b0, 0x1c0), (0x1b1, 0x1c1),
        (0x1be, 0x1ce), (0x1bf, 0x1cf), (0x17c, 0x18c), (0x17d, 0x18d),
        (0x1b6, 0x1c6), (0x1b7, 0x1c7),
    ];
    /// States of the condemned player.
    const DOOMED: u32 = 0x0204_0600;
    const SAFE_STATE: u32 = 0x0200_2000;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        let now = x.u8(0x40) as i8;
        if now > x.u8(0x1c) as i8 {
            x.add8(0x14, 1);
            hide(x);
        }
        x.set8(0x1c, now as u8);
        if x.u8(0x3) != 2 && x.a5_u8(0x50) != 0 {
            return hide(x);
        }
        if x.a5_u8(0x42) != 0 || x.a5_u8(GAME_PAUSED) != 0 {
            return hide(x);
        }
        let camera = if x.a5_u8(0x36) != 0 { 0x54e } else { 0x4fe };
        let camera = Obj(A5 + camera);
        match x.u8(0x4) {
            0 => init(x, camera),
            2 => counting(x, camera),
            4 => finish(x),
            _ => x.free(&pools::SET),
        }
    }

    const GAME_PAUSED: u32 = 0x3d;

    /// Removes the announcement (once) and cuts the display.
    fn hide<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(0x18) == 0 {
            return;
        }
        x.set8(0x18, 0);
        if rd8(x.b, MUTED) != 0 {
            return;
        }
        crate::text::clear_box(x.b, SCRIPT_OFF);
    }

    fn rows<R: Bench>(x: &mut Cx<'_, R>, base: u32, rows: &[(u16, u16)]) {
        for (i, (left, right)) in rows.iter().enumerate() {
            let at = base + ROW * i as u32;
            wr32(x.b, at, (*left as u32) << 16 | ATTR);
            wr32(x.b, at + 4, (*right as u32) << 16 | ATTR);
        }
    }

    fn init<R: Bench>(x: &mut Cx<'_, R>, camera: Obj) {
        x.add8(0x4, 2);
        x.set16(0x4c, 0x400);
        let v = camera.u16(x.b, 0x8);
        x.set16(0x4e, v);
        x.set8(0x17, 0);
        let v = x.a5_u8(0x35);
        x.set8(0x1b, v);
        if x.u8(0x5) == 0 {
            let stage = x.a5_u8(0x34) as u32;
            let v = x.rom8(PROMPT_BY_STAGE + stage);
            x.set8(0x2c, v);
        }
        x.set8(0x5, 0);
        // palette: six longs forced to maximum brightness, the last two added
        for i in 0..8 {
            let v = x.data32(PALETTE_SRC + 4 * i);
            let v = if i < 6 { v | BRIGHT } else { v.wrapping_add(BRIGHT) };
            wr32(x.b, PALETTE_DST + 4 * i, v);
        }
        let v = x.u8(0x42);
        x.set8(0x14, v);
        let packed = x.u8(0x43);
        x.set8(0x16, packed);
        x.set8(0x15, packed >> 4);
        x.set8(0x16, packed & 0xf);
    }

    fn counting<R: Bench>(x: &mut Cx<'_, R>, camera: Obj) {
        let players = x.a5_u8(0x35);
        let known = x.u8(0x1b);
        if players != known {
            if (players as i8) < (known as i8) {
                x.set8(0x1b, players);
            } else {
                x.set16(0x4, 0x401);
                x.set16(0x4c, 0);
                return;
            }
        }
        if players == 0 {
            return hide(x);
        }
        if x.a5_u8(0x51) != 0 {
            x.set16(0x4c, 0);
            return hide(x);
        }
        if x.u8(0x3) != 1 {
            if !tick(x) {
                return;
            }
            if camera.u16(x.b, 0x20) == 4 {
                x.set16(0x4c, 0x200);
                return;
            }
        }
        prompt(x, camera)
    }

    /// One-frame countdown; false if the time has elapsed (the players are then condemned).
    fn tick<R: Bench>(x: &mut Cx<'_, R>) -> bool {
        let n = x.u8(0x17).wrapping_sub(1);
        x.set8(0x17, n);
        if (n as i8) < 0 {
            x.set8(0x17, 0x3c);
            if x.u8(0x14) == 0 {
                x.set8(0x18, 1);
            }
            let n = x.u8(0x16).wrapping_sub(1);
            x.set8(0x16, n);
            if (n as i8) < 0 {
                x.set8(0x16, 9);
                let n = x.u8(0x15).wrapping_sub(1);
                x.set8(0x15, n);
                if (n as i8) < 0 {
                    x.set8(0x15, 5);
                    let n = x.u8(0x14).wrapping_sub(1);
                    x.set8(0x14, n);
                    if (n as i8) < 0 {
                        time_out(x);
                        return false;
                    }
                }
            }
        }
        if x.u8(0x18) != 0 {
            draw(x);
        }
        true
    }

    /// The time has elapsed: the living players are condemned.
    fn time_out<R: Bench>(x: &mut Cx<'_, R>) {
        for p in players() {
            if p.u8(x.b, pl::ACTIVE) == 0 || p.u8(x.b, pl::STATE) as i8 >= 4 {
                continue;
            }
            if p.u32(x.b, pl::STATE) & 0xffff_ff00 == SAFE_STATE {
                continue;
            }
            p.set_u16(x.b, pl::HEALTH, 0);
            p.set_u32(x.b, pl::STATE, DOOMED);
            let n = x.a5_u8(0x35).wrapping_sub(1);
            x.set_a5_u8(0x35, n);
            if p.u8(x.b, 0x8a) != 0 {
                p.set_u8(x.b, 0x8a, 0);
                let a2 = Obj(p.u32(x.b, 0x86) & 0xff_ffff);
                a2.set_u8(x.b, 5, 8);
                a2.set_u16(x.b, 6, 0);
            }
        }
        x.set16(0x4, 0x400);
        x.set_a5_u8(0x43, 0xff);
        x.set_a5_u16(0x4ba, 0);
    }

    /// Displays the digits and makes the inscription blink.
    fn draw<R: Bench>(x: &mut Cx<'_, R>) {
        let (d0, d1, d2) = (x.u8(0x14) as u16, x.u8(0x15) as u16, x.u8(0x16) as u16);
        for (i, tile) in [0, d0, COLON, d1, d2].into_iter().enumerate() {
            wr16(x.b, DIGITS + ROW * i as u32, tile);
            wr16(x.b, DIGITS + ROW * i as u32 + 2, ATTR as u16);
        }
        let n = x.u8(0x19).wrapping_sub(1);
        x.set8(0x19, n);
        if n != 0 {
            return;
        }
        x.set8(0x19, 0x80);
        if x.u8(0x15) == 0 {
            x.set8(0x19, 0x10);
        }
        let was = x.bit(0x1a, 0);
        x.set_bit(0x1a, 0, !was);
        if !was {
            rows(x, GET_READY, &GET_READY_ROWS);
        } else if rd8(x.b, MUTED) == 0 {
            crate::text::clear_box(x.b, SCRIPT_OFF);
        }
    }

    /// Periodic invitation (h): as long as the camera does not move, creates an invitation SOBJ object every 256
    /// frames.
    fn prompt<R: Bench>(x: &mut Cx<'_, R>, camera: Obj) {
        if x.u8(0x40) != 0 {
            return;
        }
        if x.a5_u8(0x58) != 0 {
            return x.set16(0x4c, 0);
        }
        let here = camera.u16(x.b, 0x8);
        if here != x.u16(0x4e) {
            x.set16(0x4e, here);
            return x.set16(0x4c, 0x200);
        }
        let n = x.u16(0x4c).wrapping_sub(1);
        x.set16(0x4c, n);
        if (n as i16) >= 0 || x.u8(0x3) == 2 {
            return;
        }
        x.set16(0x4c, 0x100);
        let orientation = x.u8(0x2c);
        let _ = crate::effects_aids::scenery_11(x.b, 0, orientation);
    }

    /// State 4: final inscription then return to state 0 (substate 1) after 0x80 frames.
    fn finish<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(0x5) == 0 {
            x.add8(0x5, 1);
            x.set16(0x4c, 0x80);
            if rd8(x.b, MUTED) != 0 {
                return;
            }
            crate::text::clear_box(x.b, SCRIPT_ON);
            rows(x, GAME_OVER, &GAME_OVER_ROWS);
            return;
        }
        let n = x.u16(0x4c).wrapping_sub(1);
        x.set16(0x4c, n);
        if (n as i16) >= 0 {
            return;
        }
        x.set8(0x4, 0);
        x.set8(0x5, 1);
        x.set8(0x18, 0);
        if rd8(x.b, MUTED) != 0 {
            return;
        }
        crate::text::clear_box(x.b, SCRIPT_ON);
    }
}

// ---------------------------------------------------------------------------------------------
// Type 22 (0xd69c2): rank adjustment at the end of a zone
// ---------------------------------------------------------------------------------------------

/// Adjusts the difficulty rank (A5+0x7f92) on arrival in a zone (h): from the lowest life level (`+0x62`) among the
/// present players, classed into four categories (<= 0x10, <= 0x30, <= 0x50, beyond), it reads a variation from a ROM
/// table (one row of 4 words per number of players, one table per variant) and brings it back into the interval
/// [minimum, maximum]. It acts only once, then frees itself.
mod rank_adjust {
    use super::*;

    /// Table pointers (data space), per variant.
    const TABLES: u32 = 0xd6aa0;
    const RANK: u32 = 0x7f92;
    const RANK_MAX: u32 = 0x7f94;
    const RANK_MIN: u32 = 0x7f96;
    const RANK_SAVED_FROM: u32 = 0x7f9c;
    const RANK_SAVED: u32 = 0x7fa4;
    /// Life thresholds and corresponding categories (0 = almost dead ... 6 = fit).
    const THRESHOLDS: [(u16, u16); 3] = [(0x30, 2), (0x50, 4), (0xffff, 6)];

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            // state 0 continues into state 2: twice +2
            0 => {
                x.add8(0x4, 4);
                adjust(x);
            }
            2 => {
                x.add8(0x4, 2);
                adjust(x);
            }
            _ => x.free(&pools::SET),
        }
    }

    fn adjust<R: Bench>(x: &mut Cx<'_, R>) {
        if x.a5_u8(0x30) == 0 {
            return;
        }
        let variant = x.u8(0x3) as u32;
        let table = x.data32(TABLES + 4 * variant);
        let count = x.a5_u8(0x35) as u32;
        if count == 0 {
            return;
        }
        let row = count << 3;
        let mut category: u16 = 0xffff;
        for p in players() {
            if p.u8(x.b, pl::ACTIVE) == 0 {
                continue;
            }
            let hp = p.u16(x.b, pl::HEALTH);
            if hp <= 0x10 {
                category = 0;
                break;
            }
            let class = THRESHOLDS.iter().find(|(limit, _)| hp <= *limit).map_or(6, |t| t.1);
            if class <= category {
                category = class;
            }
        }
        let at = table.wrapping_add(row.wrapping_add(category as u32) as u16 as i16 as i32 as u32);
        let delta = x.data16(at) as i16;
        if delta == 0 {
            return;
        }
        let saved = x.a5_u16(RANK_SAVED_FROM);
        x.set_a5_u16(RANK_SAVED, saved);
        let mut rank = (x.a5_u16(RANK) as i16).wrapping_add(delta);
        if delta > 0 {
            if rank >= x.a5_u16(RANK_MAX) as i16 {
                rank = x.a5_u16(RANK_MAX) as i16;
            }
        } else if rank < x.a5_u16(RANK_MIN) as i16 {
            rank = x.a5_u16(RANK_MIN) as i16;
        }
        x.set_a5_u16(RANK, rank as u16 & 0x1f);
    }
}

// ---------------------------------------------------------------------------------------------
// Type 23 (0xd6c5c): zone exit door (h)
// ---------------------------------------------------------------------------------------------

/// Zone exit door (h): each living player who stays motionless on the ground in front of the door (X >= 0x1e0, within
/// 0x20 in depth), with key 0 held down (byte A5+number), for more than 8 frames has "entered": it is put in state
/// 0x2002006 and an effect (EFFCT 0x2f) is created for the first one. When all the living players (or dead ones who
/// remain) have entered, the door goes to state 4: level change (`A5+0x34`) or, at the last zone, end of game. Bits of
/// `+0x17`: players entered; of `+0x1a`: players already counted; `+0x16`: their number; `+0x42..+0x45`: waiting
/// counters of each player; `A5+0x52`: active door.
mod exit_gate {
    use super::*;

    const GATE_LOCK: u32 = 0x52;
    /// Sound script played when leaving the zone.
    const LEAVE_SCRIPT: u32 = 0xd6f0e;
    const EFFECT_TYPE: u8 = 0x2f;
    const ENTERED: u32 = 0x0200_2006;
    const WAITING_AT_DOOR: u32 = 0x0200_2010;
    const WAIT_FRAMES: i8 = 8;
    const DOOR_X: i16 = 0x1e0;
    const LAST_STAGE: u8 = 0x1f;
    const BONUS_STAGE: u8 = 0x1c;
    const END_STAGE: u8 = 0x20;
    const BONUS_STAGES: u8 = 4;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        let me = x.o;
        crate::effects_aids::living_players(x.b, me);
        let lock = x.a5_u16(GATE_LOCK);
        if lock != 0 && lock != x.o.0 as u16 {
            return;
        }
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                for off in [0x42, 0x43, 0x44, 0x45, 0x16, 0x17] {
                    x.set8(off, 0);
                }
                let door_y = if x.u8(0x3) == 0 { 0x160 } else { 0x114 };
                x.set16(0x10, door_y);
            }
            2 => watch(x),
            4 => leave(x),
            _ => x.free(&pools::SET),
        }
    }

    /// Is a player in a position to enter? Counts the consecutive frames (byte `counter`).
    fn at_door<R: Bench>(x: &mut Cx<'_, R>, p: Obj, input: u8, counter: u32) {
        let busy = p.u8(x.b, 0x66).wrapping_add(p.u8(x.b, 0x15c)).wrapping_add(p.u8(x.b, 0x8a));
        let state = p.u32(x.b, pl::STATE) & 0xffff_ff00;
        let standing = matches!(state, 0x0200_0000 | 0x0200_0600 | 0x0200_0400);
        let y = p.u16(x.b, pl::Y).wrapping_sub(x.u16(0x10)).wrapping_add(0x20);
        if busy == 0
            && input & 1 != 0
            && p.u8(x.b, pl::STATE) == 2
            && standing
            && p.u16(x.b, pl::Z) == 0
            && (p.u16(x.b, pl::X) as i16) >= DOOR_X
            && y <= 0x40
        {
            x.add8(counter, 1);
        } else {
            x.set8(counter, 0);
        }
    }

    fn watch<R: Bench>(x: &mut Cx<'_, R>) {
        for (i, p) in players().enumerate() {
            let bit = 1u8 << i;
            let counter = 0x42 + i as u32;
            let entered = x.u8(0x17) & bit != 0;
            let mut counted_check = entered;
            if !entered {
                if p.u8(x.b, pl::ACTIVE) != 1 {
                    continue;
                }
                if (x.u8(counter) as i8) < 0 {
                    counted_check = true;
                } else {
                    let input = x.a5_u8(i as u32);
                    at_door(x, p, input, counter);
                    if (x.u8(counter) as i8) > WAIT_FRAMES {
                        return enter(x, p, bit);
                    }
                    continue;
                }
            }
            if counted_check && x.u8(0x1a) & bit == 0 && p.u32(x.b, pl::STATE) == WAITING_AT_DOOR {
                x.add8(0x16, 1);
                x.set_bit(0x1a, i as u8, true);
            }
        }
        // have all the players concerned entered?
        let mut needed = x.a5_u8(0x35);
        if needed == 0 {
            return;
        }
        for p in players() {
            if p.u8(x.b, pl::ACTIVE) != 0 && p.u16(x.b, pl::HEALTH) == 0 && p.u16(x.b, pl::STATE) != 0x402 {
                needed = needed.wrapping_add(1);
            }
        }
        if needed as i8 > x.u8(0x16) as i8 {
            return;
        }
        scene_end(x);
        x.set_a5_u16(GATE_LOCK, 0);
        x.set16(0x4, 0x400);
    }

    /// End of the zone (0xcc1c2): the count of players entered is added to the byte `A5-0x5b20`, the sound script
    /// 0xcbc02 is played and the dead players who remain on the screen go to state 0x04020000.
    fn scene_end<R: Bench>(x: &mut Cx<'_, R>) {
        const COUNT: u32 = 0xff_24e0;
        const SCRIPT: u32 = 0xcbc02;
        const OUT: u32 = 0x0402_0000;
        let entered = x.u8(0x24);
        let n = rd8(x.b, COUNT).wrapping_add(entered);
        wr8(x.b, COUNT, n);
        crate::text::clear_box(x.b, SCRIPT);
        for p in players() {
            if p.u8(x.b, pl::ACTIVE) != 0 && p.u16(x.b, pl::HEALTH) == 0 {
                let flags = p.u8(x.b, 1) | 2;
                p.set_u8(x.b, 1, flags);
                p.set_u32(x.b, pl::STATE, OUT);
            }
        }
    }

    /// Player `p` (number `bit`) enters: it is put in the "entered" state.
    fn enter<R: Bench>(x: &mut Cx<'_, R>, p: Obj, bit: u8) {
        if p.u8(x.b, pl::ACTIVE) == 0 || p.u16(x.b, pl::HEALTH) == 0 {
            return;
        }
        let v = x.u8(0x17) | bit;
        x.set8(0x17, v);
        if x.a5_u16(GATE_LOCK) == 0 {
            if let Some(effect) = x.alloc(&pools::EFFCT) {
                effect.set_u8(x.b, 0, 1);
                effect.set_u8(x.b, 2, EFFECT_TYPE);
                effect.set_u8(x.b, 0x2f, 0);
                effect.set_u8(x.b, 3, 0xff);
                let v = x.u16(0x10);
                effect.set_u16(x.b, 0x10, v);
            }
        }
        x.set_a5_u16(GATE_LOCK, x.o.0 as u16);
        p.set_u8(x.b, 0, 2);
        p.set_u32(x.b, pl::STATE, ENTERED);
        p.set_u8(x.b, 0x10f, 1);
    }

    /// State 4: cedes control to the players, announces the exit and moves on to the next level.
    fn leave<R: Bench>(x: &mut Cx<'_, R>) {
        x.set_a5_u8(0x50, 0);
        for p in players() {
            if p.u8(x.b, pl::ACTIVE) != 0 && p.u16(x.b, pl::HEALTH) != 0 {
                let v = p.u8(x.b, 1) & !2;
                p.set_u8(x.b, 1, v);
            }
        }
        x.set_a5_u32(0x2c, 0x0800_0000);
        crate::text::clear_box(x.b, LEAVE_SCRIPT);
        let stage = x.a5_u8(0x34);
        if stage != LAST_STAGE {
            x.set_a5_u8(0x34, stage + 1);
            if x.u8(0x2f) != 0 {
                bonus_stage(x);
            }
        } else if x.u8(0x2f) == 0 {
            x.set_a5_u8(0x42, 1);
        } else {
            bonus_stage(x);
        }
        x.free(&pools::SET);
    }

    /// Bonus zone: return to level 0x1c, four times at most, then end of game.
    fn bonus_stage<R: Bench>(x: &mut Cx<'_, R>) {
        x.set_a5_u8(0x34, BONUS_STAGE);
        let n = x.a5_u8(0x4e).wrapping_add(1);
        x.set_a5_u8(0x4e, n);
        if n as i8 >= BONUS_STAGES as i8 {
            x.set_a5_u8(0x34, END_STAGE);
            x.set_a5_u8(0x4e, 0);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 25 (0xd801e): volley of projectiles when the camera moves back (h)
// ---------------------------------------------------------------------------------------------

/// When the game is in play, the camera is in the left half of the level (X <= 0x180) and it has moved back since the
/// previous frame (the player turns around), the object launches three enemy shots (ESHL of type 0x12) at the right of
/// the screen, at the depth of a randomly drawn player (offsets -0x18, +0x18, 0). It waits 0x60 frames (`+8`) between
/// two volleys.
mod rearguard {
    use super::*;

    const ESHL_TYPE: u8 = 0x12;
    const COOLDOWN: u8 = 0x60;
    const FIRST_X: u16 = 0x1a0;
    const SPACING_X: u16 = 0xc;
    const HEIGHT: u16 = 0x20;
    /// Depth offsets of the three shots (words).
    const OFFSETS: u32 = 0xd80fe;
    /// A5+0x53a: cutscene in progress (h).
    const CUTSCENE: u32 = 0x53a;
    const PLAYING: u8 = 0xc;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                let v = x.a5_u16(ram::CAMERA_X);
                x.set16(0xa, v);
                watch(x);
            }
            2 => watch(x),
            _ => x.free(&pools::SET),
        }
    }

    fn remember_camera<R: Bench>(x: &mut Cx<'_, R>) {
        let v = x.a5_u16(ram::CAMERA_X);
        x.set16(0xa, v);
    }

    fn watch<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(0x8) != 0 {
            x.add8(0x8, 0xff);
            return remember_camera(x);
        }
        if x.a5_u8(0x50) != 0 {
            return x.add8(0x4, 2);
        }
        if x.a5_u8(0x2c) != PLAYING || x.a5_u8(0x35) == 0 || x.a5_u8(0x3d) != 0 {
            return remember_camera(x);
        }
        if x.a5_u8(CUTSCENE) != 0 {
            return;
        }
        let camera = x.a5_u16(ram::CAMERA_X) as i16;
        if camera > 0x180 || camera >= x.u16(0xa) as i16 {
            return remember_camera(x);
        }
        // a present player, drawn at random
        let target = loop {
            let rng = x.random();
            let p = Obj(PLAYERS + (rng as u32 & 3) * PLAYER_SIZE);
            if p.u8(x.b, pl::ACTIVE) != 0 {
                break p;
            }
        };
        x.set8(0x8, COOLDOWN);
        let mut shot_x = FIRST_X;
        for i in (0..3u32).rev() {
            let Some(shot) = x.alloc(&pools::ESHL) else { return remember_camera(x) };
            shot.set_u8(x.b, 0, 1);
            shot.set_u8(x.b, 2, ESHL_TYPE);
            shot.set_u8(x.b, 0x2c, 0);
            let v = x.a5_u16(ram::CAMERA_X).wrapping_add(shot_x);
            shot.set_u16(x.b, 0x8, v);
            shot_x += SPACING_X;
            shot.set_u16(x.b, 0xc, HEIGHT);
            let y = target.u16(x.b, pl::Y).wrapping_add(x.rom16(OFFSETS + 2 * i));
            shot.set_u16(x.b, 0x10, y);
        }
        remember_camera(x);
    }
}

// ---------------------------------------------------------------------------------------------
// Type 26 (0xd810a): hunter of motionless players (h)
// ---------------------------------------------------------------------------------------------

/// Penalizes players who stay motionless (h): for each living player whose position (X + its high fractional part) does
/// not change for `+0x1c` frames, it creates pursuers (routine 0xa44bc, three ESHL of type 0x33), within a limit of
/// active ones that depends on the number of players. The delay `+0x1c` is drawn from a ROM table according to the
/// rank. The order in which the four players are examined is drawn at random among four permutations.
mod idle_hunter {
    use super::*;

    /// Motionless delays: 16 bytes per slice of 4 ranks (program space).
    const IDLE_FRAMES: u32 = 0xd8186;
    /// Maximum number of pursuers according to the number of players (words).
    const MAX_ACTIVE: u32 = 0xd8448;
    const CUTSCENE: u32 = 0x53a;
    const PLAYING: u8 = 0xc;
    const WARMUP: u16 = 0x168;
    /// The four examination orders.
    const ORDERS: [[u32; 4]; 4] = [[0, 1, 2, 3], [2, 3, 1, 0], [1, 0, 3, 2], [3, 2, 0, 1]];

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => init(x),
            2 => step(x),
            _ => x.free(&pools::SET),
        }
    }

    /// Reference position of a player: X + following bytes (`+8` and `+0x10` of the player).
    fn mark<R: Bench>(x: &mut Cx<'_, R>, i: u32) -> u16 {
        let base = 0x5de + PLAYER_SIZE * i;
        x.a5_u16(base + 8).wrapping_add(x.a5_u16(base + 0x10))
    }

    fn init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        for i in 0..4 {
            let v = mark(x, i);
            x.set16(0x10 + 2 * i, v);
            x.set8(0x18 + i, 0);
        }
        let tier = (x.a5_u16(0x7f92) >> 2) << 4;
        let rng = x.random();
        let index = (rng & 0xf).wrapping_add(tier);
        let frames = x.rom8(IDLE_FRAMES + index as u32);
        x.set8(0x1c, frames);
        x.set16(0x1e, WARMUP);
    }

    fn step<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(0x5) == 0 {
            if x.a5_u8(0x2c) == PLAYING {
                let n = x.u16(0x1e).wrapping_sub(1);
                x.set16(0x1e, n);
                if n == 0 {
                    x.add8(0x5, 2);
                }
            }
            return;
        }
        if x.a5_u8(0x50) != 0 {
            return x.set8(0x4, 4);
        }
        if x.a5_u8(0x2c) != PLAYING
            || x.a5_u8(0x35) == 0
            || x.a5_u8(0x3d) != 0
            || x.a5_u8(CUTSCENE) != 0
            || (x.a5_u16(ram::CAMERA_X) as i16) > 0x180
        {
            return;
        }
        let rng = x.random();
        for i in ORDERS[(rng as u32 & 3) as usize] {
            check(x, i);
        }
    }

    /// Examination of a player: motionless long enough, it attracts pursuers.
    fn check<R: Bench>(x: &mut Cx<'_, R>, i: u32) {
        let p = Obj(PLAYERS + PLAYER_SIZE * i);
        if p.u8(x.b, pl::ACTIVE) & 1 == 0 || p.u16(x.b, pl::HEALTH) == 0 || x.u16(0x8 + 2 * i) != 0 {
            return;
        }
        let here = mark(x, i);
        if here == x.u16(0x10 + 2 * i) {
            let n = x.u8(0x18 + i).wrapping_add(1);
            x.set8(0x18 + i, n);
            if x.u8(0x1c) > n {
                return;
            }
            if let Some(first) = chase(x, p, 2 * i) {
                x.set16(0x8 + 2 * i, first);
            }
        }
        x.set16(0x10 + 2 * i, here);
        x.set8(0x18 + i, 0);
    }

    /// Creates pursuers for player `p` if the limit is not reached; returns the address of the last one created.
    fn chase<R: Bench>(x: &mut Cx<'_, R>, p: Obj, slot: u32) -> Option<u16> {
        let active = [0x8, 0xa, 0xc, 0xe].into_iter().filter(|&o| x.u16(o) != 0).count() as i16;
        let players = x.a5_u8(0x35) as u32;
        let allowed = x.rom16(MAX_ACTIVE + 2 * players) as i16;
        if active >= allowed {
            return None;
        }
        eshl_creators::chasers(&mut Pure, x.b, x.o, slot as u8, p).map(Obj::word)
    }
}

// ---------------------------------------------------------------------------------------------
// Types 27 and 29: passage doors (h)
// ---------------------------------------------------------------------------------------------

/// Pools of objects suspended or freed during a door passage: start, size of an object, number of slots.
pub(super) struct Pool {
    pub(super) base: u32,
    pub(super) size: u32,
    pub(super) count: u32,
}

pub(super) const SET_POOL: Pool = Pool { base: A5 + 0x4a9e, size: 0x50, count: 32 };
pub(super) const SOBJ_POOL: Pool = Pool { base: A5 + 0x795e, size: 0x50, count: 16 };
pub(super) const EFFCT_POOL: Pool = Pool { base: A5 + 0x549e, size: 0x60, count: 38 };
pub(super) const BOX_POOL: Pool = Pool { base: A5 + 0x6ede, size: 0x80, count: 21 };
pub(super) const ITEM_POOL: Pool = Pool { base: A5 + 0x62de, size: 0x60, count: 32 };
pub(super) const ENEMY_POOL: Pool = Pool { base: A5 + 0x299e, size: 0xc0, count: 22 };
pub(super) const BOSS_POOL: Pool = Pool { base: A5 + 0x3a1e, size: 0xc0, count: 3 };
pub(super) const ESHL_POOL: Pool = Pool { base: A5 + 0x3c5e, size: 0x60, count: 38 };

/// Fields of a dispatch loop object common to all the pools.
mod obj {
    pub const SUSPEND: u32 = 0x40;
    pub const SUSPEND_PHASE: u32 = 0x41;
    pub const STATE: u32 = 0x4;
}

/// Runs `run` of the handler with another object as the current object (recursive `bsr` call of the original, which
/// puts A0 on the other object).
pub(super) fn run_on<R: Bench>(x: &mut Cx<'_, R>, other: Obj, run: fn(&mut Cx<'_, R>)) {
    let me = x.o;
    x.o = other;
    run(x);
    x.o = me;
}

/// Suspends the objects of a pool (counter `+0x40` + 1, phase reset to zero).
pub(super) fn suspend<R: Bench>(x: &mut Cx<'_, R>, pool: &Pool) {
    for i in 0..pool.count {
        let o = Obj(pool.base + i * pool.size);
        if o.u8(x.b, 0) != 0 {
            let v = o.u8(x.b, obj::SUSPEND).wrapping_add(1);
            o.set_u8(x.b, obj::SUSPEND, v);
            o.set_u8(x.b, obj::SUSPEND_PHASE, 0);
        }
    }
}

/// Wakes up the objects of a pool; those whose suspension counter becomes negative are deleted (state 6).
pub(super) fn resume<R: Bench>(x: &mut Cx<'_, R>, pool: &Pool) {
    resume_with(x, pool, |_, _| ());
}

pub(super) fn resume_with<'a, R: Bench>(x: &mut Cx<'a, R>, pool: &Pool, mut when_awake: impl FnMut(&mut Cx<'a, R>, Obj)) {
    for i in 0..pool.count {
        let o = Obj(pool.base + i * pool.size);
        if o.u8(x.b, 0) == 0 {
            continue;
        }
        let n = o.u8(x.b, obj::SUSPEND).wrapping_sub(1);
        o.set_u8(x.b, obj::SUSPEND, n);
        if (n as i8) < 0 {
            o.set_u8(x.b, obj::STATE, 6);
            o.set_u8(x.b, obj::SUSPEND, 0);
            o.set_u8(x.b, obj::SUSPEND_PHASE, 0);
        } else {
            when_awake(x, o);
        }
    }
}

/// Variables (A5-0x55d0 and A5-0x55cc) stored with the camera.
const SCROLL_STATE: u32 = 0xff_2a30;
const STAGE_FLAG: u32 = 0xff_2a34;

/// Stores in the door the state of the camera and of the scroll (fields `+0x30..+0x4a`).
pub(super) fn save_camera<R: Bench>(x: &mut Cx<'_, R>) {
    for (src, dst) in [(0x506, 0x30), (0x50a, 0x34), (0x556, 0x38), (0x55a, 0x3c)] {
        let v = x.a5_u32(src);
        x.set32(dst, v);
    }
    let v = rd32(x.b, SCROLL_STATE);
    x.set32(0x4a, v);
    let v = x.a5_u16(0x57c);
    x.set16(0x18, v);
    let v = x.a5_u16(0x51e);
    x.set16(0x14, v);
    let v = rd8(x.b, STAGE_FLAG);
    x.set8(0x49, v);
    let v = x.a5_u16(0x7fc4);
    x.set16(0x1c, v);
}

/// Restores the camera and the scroll stored by `save_camera`.
pub(super) fn restore_camera<R: Bench>(x: &mut Cx<'_, R>) {
    for (dst, src) in [(0x506, 0x30), (0x50a, 0x34), (0x556, 0x38), (0x55a, 0x3c)] {
        let v = x.u32(src);
        x.set_a5_u32(dst, v);
    }
    let v = x.a5_u32(0x506);
    x.set_a5_u32(0x50e, v);
    let v = x.a5_u32(0x50a);
    x.set_a5_u32(0x512, v);
    let v = x.a5_u16(0x556);
    x.set_a5_u16(0x55e, v);
    let v = x.a5_u16(0x55a);
    x.set_a5_u16(0x562, v);
    let v = x.a5_u16(0x50e) & !0x1f;
    x.set_a5_u16(0x50e, v);
    let v = x.a5_u16(0x55e) & !0x1f;
    x.set_a5_u16(0x55e, v);
    let v = x.a5_u16(0x512).wrapping_sub(0x160);
    x.set_a5_u16(0x512, v);
    let v = x.a5_u16(0x562).wrapping_sub(0x160);
    x.set_a5_u16(0x562, v);
    let v = x.u32(0x4a);
    wr32(x.b, SCROLL_STATE, v);
    let v = x.u16(0x18);
    x.set_a5_u16(0x57c, v);
    let v = x.u16(0x14);
    x.set_a5_u16(0x51e, v);
    let v = x.u8(0x49);
    wr8(x.b, STAGE_FLAG, v);
}

/// Puts the players back in play after a passage (state, flags, delays); those that were beyond state 2 are marked dead
/// (0x402).
pub(super) fn reset_players_after_exit<R: Bench>(x: &mut Cx<'_, R>) {
    for p in players() {
        if p.u8(x.b, pl::ACTIVE) == 0 || p.u16(x.b, pl::HEALTH) == 0 {
            continue;
        }
        p.set_u8(x.b, pl::ACTIVE, 1);
        let before = p.u8(x.b, pl::STATE);
        p.set_u32(x.b, pl::STATE, 0);
        let v = p.u8(x.b, 1) & !2;
        p.set_u8(x.b, 1, v);
        for off in [0x66, 0x15c, 0x8a, 0x10f] {
            p.set_u8(x.b, off, 0);
        }
        if (before as i8) > 2 {
            let v = p.u8(x.b, 1) | 2;
            p.set_u8(x.b, 1, v);
            p.set_u16(x.b, pl::STATE, 0x402);
        }
    }
}

/// Sets the players walking again on arrival at an exit door (state 1, delay 0x20).
fn release_players<R: Bench>(x: &mut Cx<'_, R>) {
    for p in players() {
        if p.u8(x.b, pl::ACTIVE) == 0 {
            continue;
        }
        p.set_u8(x.b, pl::ACTIVE, 1);
        let v = p.u8(x.b, 1) & !2;
        p.set_u8(x.b, 1, v);
        for off in [0x66, 0x15c, 0x8a, 0x10f] {
            p.set_u8(x.b, off, 0);
        }
        p.set_u8(x.b, 0x7e, 0x20);
    }
}

/// Is player `p` motionless in front of the door? Counts the frames (byte `counter`). Conditions: nothing in progress
/// (`+0x66`, `+0x15c`, `+0x8a`), key `bit` of `input`, state 2 (walking or stopped), on the ground, within 0x10 in X
/// and 8 in depth (except `+0x1b`).
fn at_gate<R: Bench>(x: &mut Cx<'_, R>, p: Obj, input: u32, counter: u32) {
    let busy = p.u8(x.b, 0x66).wrapping_add(p.u8(x.b, 0x15c)).wrapping_add(p.u8(x.b, 0x8a));
    let bit = x.u8(0x46) as u32 & 31;
    let state = p.u32(x.b, pl::STATE) & 0xffff_ff00;
    let dx = p.u16(x.b, pl::X).wrapping_sub(x.u16(0x8)).wrapping_add(0x10);
    let dy = p.u16(x.b, pl::Y).wrapping_sub(x.u16(0x10)).wrapping_add(8);
    let aligned = dx <= 0x20 && (x.u8(0x1b) != 0 || dy <= 0x10);
    if busy == 0
        && input >> bit & 1 != 0
        && p.u8(x.b, pl::STATE) == 2
        && matches!(state, 0x0200_0000 | 0x0200_0400)
        && p.u16(x.b, pl::Z) == 0
        && aligned
    {
        x.add8(counter, 1);
    } else {
        x.set8(counter, 0);
    }
}

/// Passage door between zones (type 27, h): like `exit_gate`, but the passage leads to another zone or back from it
/// (suspension of the objects of the current zone in `+0x40`, camera saved in `+0x30..+0x4a`), with a door, of variant
/// `+3`: 0 entry into the hidden zone, 1 return. The parameters of each variant (position, key `+0x46`, mode `+0x48`,
/// arrival level `+0x4f`, `+0x1b`) come from 16-byte rows of table 0xd84e8.
mod door_a {
    use super::*;

    const GATE_LOCK: u32 = 0x52;
    const PARAMS: u32 = 0xd84e8;
    const ENTER_KINDS: u32 = 0xd8824;
    const PLACEMENT_SIDE: u32 = 0xd896e;
    const PLACEMENT: u32 = 0xd89be;
    const SCRIPT: u32 = 0xd86e8;
    const CHECK_FRAMES: i8 = 0x18;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        let me = x.o;
        crate::effects_aids::living_players(x.b, me);
        if x.u8(0x40) != 0 || x.a5_u8(0x3d) != 0 {
            return;
        }
        let lock = x.a5_u16(GATE_LOCK);
        if lock != 0 && lock != x.o.0 as u16 {
            return;
        }
        match x.u8(0x4) {
            0 => init(x),
            2 => watch(x),
            4 => arrive(x),
            _ => x.free(&pools::SET),
        }
    }

    fn init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        for off in [0x42, 0x43, 0x44, 0x45, 0x16, 0x17] {
            x.set8(off, 0);
        }
        x.set_a5_u8(0x4e, 0);
        let row = PARAMS + ((x.u8(0x3) as u32) << 4);
        let v = x.rom16(row);
        x.set16(0x8, v);
        let v = x.rom16(row + 2);
        x.set16(0xc, v);
        let v = x.rom16(row + 4);
        x.set16(0x10, v);
        let v = x.rom8(row + 6);
        x.set8(0x48, v);
        let v = x.rom8(row + 7);
        x.set8(0x46, v);
        let v = x.rom8(row + 8);
        x.set8(0x4f, v);
        let v = x.rom8(row + 9);
        x.set8(0x1b, v);
    }

    fn watch<R: Bench>(x: &mut Cx<'_, R>) {
        if x.a5_u8(0x59) == 0 {
            if x.u8(0x48) == 0 {
                return;
            }
            let v = x.a5_u8(0x35);
            x.set8(0x16, v);
            x.set_a5_u16(GATE_LOCK, x.o.0 as u16);
        }
        for (i, p) in players().enumerate() {
            let bit = 1u8 << i;
            let counter = 0x42 + i as u32;
            let entered = x.u8(0x17) & bit != 0;
            let mut counted_check = entered;
            if !entered {
                if p.u8(x.b, pl::ACTIVE) != 1 {
                    continue;
                }
                if (x.u8(counter) as i8) < 0 {
                    counted_check = true;
                } else {
                    let input = p.u8(x.b, 0x6c) as u32;
                    at_gate(x, p, input, counter);
                    if (x.u8(counter) as i8) > CHECK_FRAMES {
                        return enter(x, p, bit);
                    }
                    continue;
                }
            }
            if counted_check && x.u8(0x1a) & bit == 0 && p.u32(x.b, pl::STATE) == 0x0200_2010 {
                x.add8(0x16, 1);
                x.set_bit(0x1a, i as u8, true);
            }
        }
        let mut needed = x.a5_u8(0x35);
        if needed == 0 {
            return;
        }
        for p in players() {
            if p.u8(x.b, pl::ACTIVE) != 0 && p.u16(x.b, pl::HEALTH) == 0 && p.u16(x.b, pl::STATE) != 0x402 {
                needed = needed.wrapping_add(1);
            }
        }
        if needed as i8 > x.u8(0x16) as i8 {
            return;
        }
        leave(x);
    }

    /// A player goes through the door: either an appearance (variant 0 without `A5+0x4e`), or it is put in the entry
    /// state.
    fn enter<R: Bench>(x: &mut Cx<'_, R>, p: Obj, bit: u8) {
        if x.u8(0x3) == 0 && x.a5_u8(0x4e) == 0 {
            let spawned = scene_set(x, 0x8b);
            spawned.set_u8(x.b, 0x42, 0xf);
            let v = p.u8(x.b, 3);
            spawned.set_u8(x.b, 0x43, v);
            let i = bit.trailing_zeros();
            x.set8(0x42 + i, 0);
            return;
        }
        let v = x.u8(0x17) | bit;
        x.set8(0x17, v);
        x.set_a5_u16(GATE_LOCK, x.o.0 as u16);
        let kind = x.u8(0x46) as u32;
        let state = x.rom16(ENTER_KINDS + 4 * kind) as u32;
        let state = state << 16 | x.rom16(ENTER_KINDS + 4 * kind + 2) as u32;
        p.set_u32(x.b, pl::STATE, state);
        p.set_u8(x.b, 0, 2);
        p.set_u8(x.b, 0x10f, 1);
    }

    /// All the players concerned are at the door: the passage begins.
    fn leave<R: Bench>(x: &mut Cx<'_, R>) {
        crate::sound::service(x.b, crate::sound::Service::Ff06 { lock: 0xff, line: 4 });
        x.set_a5_u8(0x51, 1);
        let level = x.u8(0x4f);
        x.set_a5_u8(0x34, level);
        if x.u8(0x3) == 0 {
            x.set_a5_u8(0x42, 1);
            return;
        }
        crate::text::clear_box(x.b, SCRIPT);
        x.set_a5_u32(0x5c6, 0);
        // 0xdd12c: the pickup objects' appearance lock lifted
        crate::mem::wr8(x.b, A5 - 0x5ac4, 0);
        x.set_a5_u16(GATE_LOCK, 0);
        match x.u8(0x48) as i8 {
            1 => exit_zone(x),
            mode if mode > 1 => {}
            _ => {
                save_camera(x);
                suspend(x, &SET_POOL);
                suspend(x, &EFFCT_POOL);
                suspend(x, &SOBJ_POOL);
                x.set16(0x4, 0x400);
                enter_zone(x);
            }
        }
    }

    /// Loading of the hidden zone (state 0x400 of the door).
    fn enter_zone<R: Bench>(x: &mut Cx<'_, R>) {
        crate::families::scenery::resume_scenery(x, crate::families::scenery::Cameras::Start);
        x.set_a5_u8(0x48e, 0xf);
        crate::palette::full_brightness(x.b);
        x.set_a5_u16(GATE_LOCK, 0);
        reset_players_after_exit(x);
        crate::families::scenery::place_players(x);
    }

    /// Return to the original zone: the players are released and all the objects of the hidden zone deleted; each door
    /// of type 0x1b of the zone found again is replayed.
    fn exit_zone<R: Bench>(x: &mut Cx<'_, R>) {
        x.set_a5_u8(0x4e, 1);
        release_players(x);
        x.set8(0x4, 6);
        resume_with(x, &SET_POOL, |x, o| {
            if o.u8(x.b, 2) == 0x1b {
                run_on(x, o, run);
            }
        });
        resume(x, &EFFCT_POOL);
        resume(x, &BOX_POOL);
        resume(x, &ITEM_POOL);
        resume(x, &SOBJ_POOL);
    }

    /// State 4: arrival in the hidden zone (loading of the new zone).
    fn arrive<R: Bench>(x: &mut Cx<'_, R>) {
        crate::families::scenery::resume_scenery(x, crate::families::scenery::Cameras::Return);
        x.set_a5_u8(0x48e, 0xf);
        crate::palette::full_brightness(x.b);
        x.set_a5_u16(GATE_LOCK, 0);
        x.set_a5_u8(0x51, 0);
        restore_camera(x);
        // Original glitch: D4.w = `+0x1c` (the zone's music, read by the sound of 0xe17a4 in door 29) stays for the
        // following handlers (`dispatch::Leftovers`)
        let d4 = x.u16(0x1c);
        *x.c.d4() = (*x.c.d4() & 0xffff_0000) | d4 as u32;
        crate::sound::service(x.b, crate::sound::Service::Ff06 { lock: 0x90, line: 0 });
        for p in players() {
            if p.u8(x.b, pl::ACTIVE) != 0 {
                place(x, p);
            }
        }
        x.set8(0x4, 6);
    }

    /// Places a player on arrival (position relative to the door, table 0xd89be according to the side).
    fn place<R: Bench>(x: &mut Cx<'_, R>, p: Obj) {
        if p.u8(x.b, 4) != 2 {
            let v = p.u8(x.b, 1) | 2;
            p.set_u8(x.b, 1, v);
            crate::placement::place_on_free_ground(x.b, p);
            return;
        }
        p.set_u32(x.b, pl::STATE, 0x0200_0000);
        let v = p.u8(x.b, 1) & !2;
        p.set_u8(x.b, 1, v);
        p.set_u8(x.b, pl::ACTIVE, 1);
        let variant = x.u8(0x3) as u32;
        let side = x.rom8(PLACEMENT_SIDE + variant) as u32;
        let at = (side << 4) + 4 * p.u8(x.b, 2) as u32;
        let dx = x.rom16(PLACEMENT + at);
        let dy = x.rom16(PLACEMENT + 2 + at);
        let v = x.u16(0x8);
        p.set_u16(x.b, pl::X, v);
        let v = x.u16(0x10);
        p.set_u16(x.b, pl::Y, v);
        p.set_u16(x.b, pl::Z, 0);
        let v = p.u16(x.b, pl::X).wrapping_add(dx);
        p.set_u16(x.b, pl::X, v);
        let v = p.u16(x.b, pl::Y).wrapping_add(dy);
        p.set_u16(x.b, pl::Y, v);
    }
}

/// Three-stage zone door (type 28, h): when the camera reaches X = 0x1c0 it blocks the scroll (`A5+0x51e` = 4), creates
/// a SET event (type 0x3f) and waits for it to finish (`+0x22` of that event); it then launches the invitation (SOBJ
/// 11) and a delay of 0x78 frames, then frees the scroll and launches the end-of-zone routines (0xa462c, 0xe55a2,
/// 0xe13b0).
mod zone_end {
    use super::*;

    const EVENT_TYPE: u32 = 0x3f;
    const LOCK_X: u16 = 0x1c0;
    const SCROLL_LOCK: u32 = 0x51e;
    const EVENT_FLAG: u32 = 0x43;
    const DELAY: u16 = 0x78;
    /// The SET objects of this type have their orientation (`+0x2c` bit 5) inverted at the end.
    const FLIPPED_TYPE: u8 = 0x15;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                sub_state(x);
            }
            2 => sub_state(x),
            _ => x.free(&pools::SET),
        }
    }

    fn sub_state<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x5) {
            0 => {
                if x.a5_u8(0x35) == 0 || x.a5_u16(ram::CAMERA_X) < LOCK_X {
                    return;
                }
                x.add8(0x5, 2);
                x.set_a5_u16(SCROLL_LOCK, 4);
                x.set_a5_u8(EVENT_FLAG, 0x1f);
                let event = scene_set(x, EVENT_TYPE as u8).0 as u16;
                x.set16(0x10, event);
            }
            2 => {
                let event = x.linked(0x10);
                if event.u8(x.b, 0x22) == 0 {
                    return;
                }
                if event.u8(x.b, 0x23) == 0 {
                    x.add8(0x5, 2);
                    let v = x.a5_u16(ram::CAMERA_X);
                    x.set_a5_u16(0x52c, v);
                    x.set16(0x14, DELAY);
                    let _ = crate::effects_aids::scenery_11(x.b, 0, 0);
                    return;
                }
                x.add8(0x4, 2);
                x.set_a5_u16(SCROLL_LOCK, 0);
                x.set_a5_u8(EVENT_FLAG, 0xff);
                let v = x.a5_u16(ram::CAMERA_X);
                x.set_a5_u16(0x532, v);
                // 33 slots: the last is the first EFFCT object (contiguous pool)
                for i in 0..33u32 {
                    let o = Obj(SET_POOL.base + i * SET_POOL.size);
                    if o.u8(x.b, 0) != 0 && o.u8(x.b, 2) == FLIPPED_TYPE {
                        let v = o.u8(x.b, 0x2c) ^ 0x20;
                        o.set_u8(x.b, 0x2c, v);
                    }
                }
            }
            4 => {
                let n = x.u16(0x14).wrapping_sub(1);
                x.set16(0x14, n);
                if n != 0 {
                    return;
                }
                x.add8(0x4, 2);
                x.set_a5_u16(SCROLL_LOCK, 0);
                eshl_creators::linked_34(&mut Pure, x.b, x.o, 3);
                crate::sound::play(x.b, 0xe55a2, x.o);
                crate::sound::play(x.b, 0xe13b0, x.o);
            }
            _ => {}
        }
    }
}

/// Passage door (type 29, h): variant of door 27 that shifts all the scenery (`SET`, `SOBJ`, `EFFCT`, `BOX`, `ITEM`,
/// `BOSS`, `ESHL`) instead of three pools, and puts the players back on a fixed marker (table 0xd9052). Same fields
/// (`+0x42..+0x45` counters, `+0x16` number entered, `+0x17` bits entered, `+0x30..` saved camera).
pub(super) mod door_b {
    use super::*;

    const GATE_LOCK: u32 = 0x52;
    const PARAMS: u32 = 0xd8e5e;
    const PLACEMENT: u32 = 0xd9052;
    const SOUND_FLAG: u32 = 0xff_24e0;
    const NEAR_X: u16 = 0x30;
    const NEAR_Y: u16 = 0x20;

    /// Stays on the processor: the exit fade (`watch`) cedes control (`set::exit_fading`).
    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        let me = x.o;
        crate::effects_aids::living_players(x.b, me);
        if x.u8(0x40) != 0 || x.a5_u8(0x3d) != 0 {
            return;
        }
        let lock = x.a5_u16(GATE_LOCK);
        if lock != 0 && lock != x.o.0 as u16 {
            return;
        }
        match x.u8(0x4) {
            0 => init(x),
            2 => watch(x),
            4 => arrive(x),
            _ => x.free(&pools::SET),
        }
    }

    fn init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        for off in [0x42, 0x43, 0x44, 0x45, 0x16, 0x17] {
            x.set8(off, 0);
        }
        x.set_a5_u8(0x50, 0);
        let row = PARAMS + ((x.u8(0x3) as u32) << 4);
        let v = x.rom16(row);
        x.set16(0x8, v);
        let v = x.rom16(row + 2);
        x.set16(0xc, v);
        let v = x.rom16(row + 4);
        x.set16(0x10, v);
        for (off, at) in [(0x48, 6), (0x46, 7), (0x4f, 8), (0x1b, 9)] {
            let v = x.rom8(row + at);
            x.set8(off, v);
        }
    }

    /// The player is within range of the door.
    fn near<R: Bench>(x: &mut Cx<'_, R>, p: Obj) -> bool {
        let dx = p.u16(x.b, pl::X).wrapping_sub(x.u16(0x8)).wrapping_add(NEAR_X);
        if dx > 2 * NEAR_X {
            return false;
        }
        if x.u8(0x1b) != 0 {
            return true;
        }
        let dy = p.u16(x.b, pl::Y).wrapping_sub(x.u16(0x10)).wrapping_add(NEAR_Y);
        dy <= 2 * NEAR_Y
    }

    fn watch<R: Bench>(x: &mut Cx<'_, R>) {
        if !players().any(|p| p.u8(x.b, pl::ACTIVE) == 1 && near(x, p)) {
            return;
        }
        let v = rd8(x.b, SOUND_FLAG).wrapping_add(1);
        wr8(x.b, SOUND_FLAG, v);
        x.set_a5_u16(GATE_LOCK, 0);
        x.set_a5_u8(0x43, 0xff);
        // exit fade: cedes control, the task resumes at `after_fade`
        use crate::families::set::exit_fading::{fade_to_black, Step};
        fade_to_black(x, Step::Passage)
    }

    /// After the exit fade (0xd8f06).
    pub fn after_fade<R: Bench>(x: &mut Cx<'_, R>) {
        let level = x.u8(0x4f);
        x.set_a5_u8(0x34, level);
        match x.u8(0x48) as i8 {
            1 => exit_zone(x),
            mode if mode > 1 => x.set_a5_u8(0x42, 1),
            _ => {
                x.set_a5_u8(0x4a, 0);
                save_camera(x);
                for pool in [&SET_POOL, &SOBJ_POOL, &EFFCT_POOL, &BOX_POOL, &ITEM_POOL, &BOSS_POOL, &ESHL_POOL] {
                    suspend(x, pool);
                }
                x.set16(0x4, 0x400);
                enter_zone(x);
            }
        }
    }

    fn enter_zone<R: Bench>(x: &mut Cx<'_, R>) {
        crate::families::scenery::resume_scenery(x, crate::families::scenery::Cameras::Start);
        x.set_a5_u8(0x48e, 0xf);
        crate::palette::full_brightness(x.b);
        crate::sound::play(x.b, 0xe0c5c, x.o);
        x.set_a5_u16(GATE_LOCK, 0);
        reset_players_after_exit(x);
        crate::families::scenery::place_players(x);
    }

    fn exit_zone<R: Bench>(x: &mut Cx<'_, R>) {
        release_players(x);
        x.set8(0x4, 6);
        resume_with(x, &SET_POOL, |x, o| {
            if o.u8(x.b, 2) == 0x1d {
                run_on(x, o, run);
            }
        });
        for pool in [&ENEMY_POOL, &EFFCT_POOL, &BOX_POOL, &ITEM_POOL, &BOSS_POOL, &ESHL_POOL, &SOBJ_POOL] {
            resume(x, pool);
        }
    }

    fn arrive<R: Bench>(x: &mut Cx<'_, R>) {
        crate::families::scenery::resume_scenery(x, crate::families::scenery::Cameras::Return);
        x.set_a5_u8(0x48e, 0xf);
        crate::palette::full_brightness(x.b);
        x.set_a5_u16(GATE_LOCK, 0);
        x.set_a5_u8(0x4a, 1);
        x.set_a5_u8(0x43, 0xf);
        restore_camera(x);
        let music = x.u16(0x1c);
        crate::sound::play_with(x.b, 0xe17a4, crate::sound::Source { d4: Some(music as u32), ..Default::default() });
        for p in players() {
            if p.u8(x.b, pl::ACTIVE) != 0 {
                place(x, p);
            }
        }
        x.set8(0x4, 6);
    }

    fn place<R: Bench>(x: &mut Cx<'_, R>, p: Obj) {
        if p.u8(x.b, 4) != 2 {
            let v = p.u8(x.b, 1) | 2;
            p.set_u8(x.b, 1, v);
            crate::placement::place_on_free_ground(x.b, p);
            return;
        }
        p.set_u32(x.b, pl::STATE, 0x0200_2a00);
        let v = p.u8(x.b, 1) & !2;
        p.set_u8(x.b, 1, v);
        p.set_u8(x.b, pl::ACTIVE, 1);
        let slot = 2 * p.u8(x.b, 2) as u32;
        let px = x.rom16(PLACEMENT + slot);
        p.set_u16(x.b, pl::X, px);
        p.set_u16(x.b, pl::Z, 0xa0);
        let y = x.a5_u16(ram::CAMERA_Y).wrapping_add(0xf0);
        p.set_u16(x.b, pl::Y, y);
    }
}

// ---------------------------------------------------------------------------------------------
// Type 30 (0xd9424): end of game (continue screen)
// ---------------------------------------------------------------------------------------------

/// When `A5+0x42` (end of game) is raised, plays the end music (routine 0xa8ae8, D0 = 7), waits 0x80 frames then
/// launches the per-component fade task 0x1ccf8 and frees itself.
mod game_over {
    use super::*;

    const WAIT_FRAMES: u16 = 0x80;
    /// Per-component fade-to-white task.
    const FADE_TO_WHITE_TASK: u32 = 0x1ccf8;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 | 2 => {
                if x.a5_u8(0x42) == 0 {
                    return;
                }
                // 0xa8ae8 (D0 = 7): scenery list 7
                crate::families::effects_creators::set_list_via(&mut OnBench(&mut x.c), x.b, 7);
                x.add8(0x4, 2);
                x.set16(0x4c, WAIT_FRAMES);
                crate::families::scenery::save_palette_page_1(x);
                // the original chains onto the countdown (0xd9458) from this very frame: 0x80 becomes 0x7f
                countdown(x);
            }
            4 => countdown(x),
            _ => x.free(&pools::SET),
        }
    }

    fn countdown<R: Bench>(x: &mut Cx<'_, R>) {
        let n = x.u16(0x4c).wrapping_sub(1);
        x.set16(0x4c, n);
        if (n as i16) >= 0 {
            return;
        }
        x.set_a5_u16(0x490, 1);
        x.set_a5_u8(0x48e, 0xd);
        x.set_a5_u8(0x48f, 1);
        start_fade_task(x, FADE_TO_WHITE_TASK);
        x.free(&pools::SET);
    }
}

// ---------------------------------------------------------------------------------------------
// Type 31 (0xd9492): waiting message in a long level (h)
// ---------------------------------------------------------------------------------------------

/// Level timer (h): counts frames as long as a player is in play (`+0x42`) and triggers at 0x258, 0x4b0 and 0x708
/// frames three messages (speech + effects: routines 0xa66c0, 0x2824, 0x2860, 0x289c, 0xeda26, 0xeda66); if the camera
/// advances by more than 0xa00 since the start (`+0x1e`) it cuts short and goes directly to the last step.
mod level_clock {
    use super::*;

    const ON: u32 = 0xeda26;
    const OFF: u32 = 0xeda66;
    const TIME: u32 = 0x42;
    const CUE: u32 = 0x1a;
    const TICK: u32 = 0x1c;
    const FAR: i16 = 0xa00;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set16(TIME, 0);
                let v = x.a5_u16(ram::CAMERA_X);
                x.set16(0x1e, v);
            }
            2 => step(x),
            _ => x.free(&pools::SET),
        }
    }

    /// 0xa66c0, which reads its table row in D2 (left to the following handlers, `dispatch::Leftovers`).
    fn speech<R: Bench>(x: &mut Cx<'_, R>, kind: u32) {
        // the row stays in D2 (`dispatch::Leftovers`)
        *x.c.d2() = kind;
        crate::families::scenery::eshl_volley(x, kind);
    }

    fn countdown<R: Bench>(x: &mut Cx<'_, R>, reset: u16, kind: u32) {
        let n = x.u16(TICK).wrapping_sub(1);
        x.set16(TICK, n);
        if (n as i16) < 0 {
            x.set16(TICK, reset);
            speech(x, kind);
        }
        let n = x.u16(CUE).wrapping_sub(1);
        x.set16(CUE, n);
        if n == 0 {
            crate::sound::play(x.b, OFF, x.o);
        }
    }

    fn step<R: Bench>(x: &mut Cx<'_, R>) {
        if x.a5_u8(0x35) != 0 {
            x.add16(TIME, 1);
        }
        if x.u8(0x5) != 6 {
            let far = x.a5_u16(ram::CAMERA_X).wrapping_sub(x.u16(0x1e)) as i16;
            if far >= FAR {
                x.set16(CUE, 0x78);
                x.set8(0x5, 6);
            }
        }
        let time = x.u16(TIME) as i16;
        match x.u8(0x5) {
            0 => {
                if time < 0x258 {
                    return;
                }
                x.add8(0x5, 2);
                show_message(x, 2);
                x.set16(TICK, 0x50);
                speech(x, 0);
                x.set16(CUE, 0x28);
                crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::A, 0x28);
                crate::sound::play(x.b, ON, x.o);
            }
            2 => {
                countdown(x, 0x50, 0);
                if time < 0x4b0 {
                    return;
                }
                x.add8(0x5, 2);
                show_message(x, 2);
                x.set16(TICK, 0x3c);
                speech(x, 1);
                x.set16(CUE, 0x28);
                crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::B, 0x28);
            }
            4 => {
                countdown(x, 0x3c, 1);
                if time < 0x708 {
                    return;
                }
                x.add8(0x5, 2);
                speech(x, 2);
                x.set16(CUE, 0x78);
                crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::C, 0x78);
                crate::sound::play(x.b, ON, x.o);
            }
            6 => {
                let n = x.u16(CUE).wrapping_sub(1);
                x.set16(CUE, n);
                if n != 0 {
                    return;
                }
                x.add8(0x4, 2);
                x.set_a5_u8(0x42, 1);
                crate::sound::play(x.b, OFF, x.o);
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 32 (0xd96d0): test selector (h)
// ---------------------------------------------------------------------------------------------

/// Test selector (h): with player 1's keys (bytes A5+2 pressed, A5+8 previous), button 5 increases a number (0 to 0x32,
/// `+0x2f`), button 6 decreases it, button 4 confirms it (routine 0xcc352 on the first player). The number is displayed
/// as two hexadecimal digits in the background (0x912bc0).
mod test_selector {
    use super::*;
    use crate::mem::wr16;

    const SCREEN: u32 = 0x912bc0;
    const LAST: u8 = 0x32;
    const ROW: u32 = 0x80;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(0x2f, 0);
            }
            2 => step(x),
            _ => x.free(&pools::SET),
        }
    }

    fn step<R: Bench>(x: &mut Cx<'_, R>) {
        let pressed = !x.a5_u8(8) & x.a5_u8(2);
        if pressed >> 4 & 1 != 0 {
            // 0xcc352 with A0 = the first player: its class is that of the SET object created
            let index = x.u8(0x2f);
            crate::families::creators::set_a(x.b, Obj(PLAYERS), index);
        } else if pressed >> 5 & 1 != 0 {
            let n = x.u8(0x2f) + 1;
            x.set8(0x2f, if n > LAST { 0 } else { n });
            show(x);
        } else if pressed >> 6 & 1 != 0 {
            let n = x.u8(0x2f);
            x.set8(0x2f, if n == 0 { LAST } else { n - 1 });
            show(x);
        }
    }

    fn show<R: Bench>(x: &mut Cx<'_, R>) {
        let n = x.u8(0x2f) as u16;
        for (i, digit) in [n >> 4, n & 0xf].into_iter().enumerate() {
            wr16(x.b, SCREEN + ROW * i as u32, digit & 0xf);
            wr16(x.b, SCREEN + ROW * i as u32 + 2, 0);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 24 (0xd7056): dialogue window
// ---------------------------------------------------------------------------------------------

/// Dialogue window (h): frame drawn in background layer 1 (GFX RAM 0x912000, 0x80 bytes per line) that opens, receives
/// a text, waits for a key then closes. The variant `+3` designates, through table 0x1d78d0, a window definition in the
/// ROM (position, size, text `+0x24`, kind `+0x2f`...). The kind (24 values) sets the initialization (palettes, choice
/// of another text according to the number of players...) and the construction of the text's token list (words in the
/// buffer 0xff2072, reread by the text engine through the cell `A5+0x5c6+ (+0x38)`). Special tokens: 0 end, negatives =
/// commands, 0x7000 to 0x7009 = substitutions (number of players, player names...).
///
/// States: 0 opening, 2 life (substates `+5`: 0 frame opening, 2 unfolding, 4 waiting for a key, 6 closing), 4 and 6
/// release.
mod dialog {
    use super::*;
    use crate::mem::{rd16, rd32, wr16, wr32};

    /// Window definitions per variant (pointers, data space).
    const DEFINITIONS: u32 = 0x1d78d0;
    const SCREEN: u32 = 0x912000;
    /// Token buffer (A5-0x5f8e) and text engine cells (A5+0x5c6, 8 bytes each).
    const BUFFER: u32 = 0xff_2072;
    const SLOTS: u32 = A5 + 0x5c6;
    /// Counter of open windows (A5-0x5b8e) and flag (A5-0x5fbe).
    const OPEN_COUNT: u32 = 0xff_2472;
    const FLAG_SKIP: u32 = 0xff_2042;
    const PALETTE_SRC: u32 = 0x292260;
    const PALETTE_ALT: u32 = 0x292200;
    const CHARS: u32 = 0xd7e8a;
    const SEPARATOR: u32 = 0xd7e26;
    const NAME_TABLE: u32 = 0xd7604;
    const PLAYER_TITLES: u32 = 0xd7db6;
    const PLAYERS_ONE_TWO: u32 = 0xd7d96;
    const PLAYERS_ONE_TWO_B: u32 = 0xd7da6;
    const TOK_LINK_A: u16 = 0xfffe;
    /// Opening and closing words of a player name.
    const NAME_OPEN: u32 = 0xfffe_0018;
    const NAME_CLOSE: u32 = 0xfffe_000f;
    /// Creation / search of linked objects.

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => open(x),
            2 => step(x),
            _ => close(x),
        }
    }

    // --- token output ----------------------------------------------------------------------

    /// Write pointer (A4) and number of characters (D6) of the token list.
    struct Out {
        at: u32,
        n: u16,
    }

    impl Out {
        fn put<R: Bench>(&mut self, x: &mut Cx<'_, R>, w: u16) {
            wr16(x.b, self.at, w);
            self.at += 2;
        }
        fn put32<R: Bench>(&mut self, x: &mut Cx<'_, R>, v: u32) {
            self.put(x, (v >> 16) as u16);
            self.put(x, v as u16);
        }
        /// Terminator, not advanced.
        fn end<R: Bench>(&mut self, x: &mut Cx<'_, R>) {
            wr16(x.b, self.at, 0);
        }
    }

    /// Command (negative token): copies the token, `-8 * n`, then the word that follows the next word (0xfffe: the next
    /// word alone).
    fn command<R: Bench>(x: &mut Cx<'_, R>, o: &mut Out, a6: &mut u32, tok: u16) {
        o.put(x, tok);
        if tok == TOK_LINK_A {
            let w = x.data16(*a6);
            *a6 += 2;
            return o.put(x, w);
        }
        o.put(x, (o.n << 3).wrapping_neg());
        o.n = 0;
        *a6 += 2;
        let w = x.data16(*a6);
        *a6 += 2;
        o.put(x, w);
    }

    /// Reads the next word of the text.
    fn next<R: Bench>(x: &mut Cx<'_, R>, a6: &mut u32) -> u16 {
        let w = x.data16(*a6);
        *a6 += 2;
        w
    }

    /// Raw text: tokens copied, 0x7000 (number of players) replaced according to the number of players.
    fn plain<R: Bench>(x: &mut Cx<'_, R>, o: &mut Out, a6: &mut u32) {
        loop {
            let tok = next(x, a6);
            if tok == 0 {
                return o.end(x);
            }
            if (tok as i16) < 0 {
                command(x, o, a6, tok);
            } else if tok == 0x7000 {
                if x.a5_u8(0x35) != 1 {
                    o.put(x, 0x18f3);
                    o.n += 1;
                }
            } else {
                o.put(x, tok);
                o.n += 1;
            }
        }
    }

    /// Character string of a name (up to 6 bytes, index into the tile table 0xd7e8a; 0xff terminates). `special`: bytes
    /// >= 0xf9 designate a predefined name of the table 0xd7604.
    fn name<R: Bench>(x: &mut Cx<'_, R>, o: &mut Out, mut a1: u32) {
        let mut remaining: u16 = 5;
        let first = x.data8(a1);
        if first.wrapping_add(1) >= 0xfa {
            let index = (0u8.wrapping_sub(first).wrapping_sub(3)).wrapping_mul(4) as u32;
            a1 = x.data32(NAME_TABLE + index);
            remaining = x.data16(a1);
            a1 += 2;
        }
        loop {
            let c = x.data8(a1);
            a1 += 1;
            if c == 0xff {
                return;
            }
            let tile = x.data16(CHARS + 2 * c as u32);
            o.put(x, tile);
            o.n += 1;
            remaining = remaining.wrapping_sub(1);
            if remaining == 0xffff {
                return;
            }
        }
    }

    /// Variant for names with a category (starting byte >= 0x82: 0x88, 0x90, 0x82, 0xa0 or other, each with its own
    /// text in the table 0xd7604).
    fn name_category<R: Bench>(x: &mut Cx<'_, R>, o: &mut Out, mut a1: u32) {
        let mut remaining: u16 = 5;
        let first = x.data8(a1);
        if first >= 0x82 {
            let mut d0 = 0u32;
            for probe in [0x88, 0x90, 0x82, 0xa0] {
                if first == probe {
                    break;
                }
                d0 += 4;
            }
            a1 = x.data32(NAME_TABLE + d0);
            remaining = x.data16(a1);
            a1 += 2;
        }
        loop {
            let c = x.data8(a1);
            a1 += 1;
            if c == 0xff {
                return;
            }
            let tile = x.data16(CHARS + 2 * c as u32);
            o.put(x, tile);
            o.n += 1;
            remaining = remaining.wrapping_sub(1);
            if remaining == 0xffff {
                return;
            }
        }
    }

    /// End of a name block: word -1, `-8 * n`, 0x10 (routine 0xd75a6).
    fn name_tail<R: Bench>(x: &mut Cx<'_, R>, o: &mut Out) {
        o.put(x, 0xffff);
        o.put(x, (o.n << 3).wrapping_neg());
        o.put(x, 0x10);
    }

    /// Name of player `p` between the openings 0xfffe0018 and 0xfffe000f (routine 0xd75b6 without the end).
    fn player_name<R: Bench>(x: &mut Cx<'_, R>, o: &mut Out, p: Obj) {
        o.put32(x, NAME_OPEN);
        name(x, o, p.0 + 0x118);
    }

    /// Complete block of a kind 1 player (routine 0xd7536): title, name, end.
    fn player_block<R: Bench>(x: &mut Cx<'_, R>, o: &mut Out, p: Obj) {
        let index = ((p.u8(x.b, 3) >> 1).wrapping_mul(4)) as u32;
        let mut a1 = x.data32(PLAYER_TITLES + index);
        loop {
            let w = x.data16(a1);
            if w == 0 {
                break;
            }
            o.put(x, w);
            a1 += 2;
            o.n += 1;
        }
        player_name(x, o, p);
        o.put32(x, NAME_CLOSE);
        name_tail(x, o);
    }

    /// `movem.l (A2)+,D0-D6/A3 ; movem.l D0-D6/A3,(A1)`: the eight longs remain in D0-D6 and A3. Returns the seventh
    /// (D6), which the closing of a window rereads (`shrink`: the number of lines). Original glitch: D2-D5 remain for
    /// the following handlers (`dispatch::Leftovers`).
    fn copy_palette<R: Bench>(x: &mut Cx<'_, R>, src: u32, dst: u32) -> u32 {
        copy_longs(x, src, dst, 8);
        let [d2, d3, d4, d5, d6] = [2, 3, 4, 5, 6].map(|i| x.data32(src + 4 * i));
        (*x.c.d2(), *x.c.d3(), *x.c.d4(), *x.c.d5()) = (d2, d3, d4, d5);
        d6
    }

    fn copy_longs<R: Bench>(x: &mut Cx<'_, R>, from: u32, to: u32, n: u32) {
        for i in 0..n {
            let v = x.data32(from + 4 * i);
            wr32(x.b, to + 4 * i, v);
        }
    }

    // --- text construction, according to the kind ----------------------------------------------

    /// Builds the token list of the kind `kind` (table 0xd7464: the index is shifted by one notch, kind `k` jumps to
    /// the `k`-th entry of the table 0xd7466; kind 0 is handled before).
    fn build<R: Bench>(x: &mut Cx<'_, R>, kind: u8) {
        let mut o = Out { at: BUFFER, n: 0 };
        let mut a6 = x.u32(0x24);
        match kind {
            2 => {
                copy_palette(x, PALETTE_ALT, 0x91c300);
                for p in players() {
                    if p.u8(x.b, pl::ACTIVE) != 0 && p.u8(x.b, 4) != 4 {
                        o.n = 0;
                        player_block(x, &mut o, p);
                    }
                }
                o.end(x);
            }
            3 => {
                let setting = x.u8(0x34);
                let Some(other) = crate::families::effects_creators::vote(x.b, 0, setting) else {
                    x.add8(0x5, 0xfe);
                    return;
                };
                other.set_u16(x.b, 0x50, x.o.0 as u16);
                x.set16(0x32, 0xffff);
                build(x, 1);
            }
            4 => {
                copy_palette(x, PALETTE_ALT, 0x91c300);
                let mut left = x.a5_u8(0x35);
                for p in players() {
                    if p.u8(x.b, pl::ACTIVE) == 0 || (p.u8(x.b, 4) as i8) >= 4 {
                        continue;
                    }
                    o.n = 0;
                    player_name(x, &mut o, p);
                    left = left.wrapping_sub(1);
                    if left == 0 {
                        continue;
                    }
                    let sep = x.data16(SEPARATOR);
                    o.put(x, sep);
                    o.n += 1;
                    name_tail(x, &mut o);
                }
                let sep = x.data16(SEPARATOR);
                o.put(x, sep);
                o.end(x);
            }
            6 | 7 => {
                loop {
                    let tok = next(x, &mut a6);
                    if tok == 0 {
                        break;
                    }
                    if (tok as i16) < 0 {
                        command(x, &mut o, &mut a6, tok);
                        continue;
                    }
                    let table = match tok {
                        0x7001 => PLAYERS_ONE_TWO,
                        0x7002 => PLAYERS_ONE_TWO_B,
                        _ => {
                            o.put(x, tok);
                            o.n += 1;
                            continue;
                        }
                    };
                    let index = ((x.a5_u8(0x35).wrapping_sub(1)) as u16).wrapping_mul(4) as u32;
                    let mut a1 = x.data32(table + index);
                    loop {
                        let w = x.data16(a1);
                        if w == 0 {
                            break;
                        }
                        o.put(x, w);
                        a1 += 2;
                        o.n += 1;
                    }
                }
                o.end(x);
            }
            8 => {
                loop {
                    let tok = next(x, &mut a6);
                    if tok == 0 {
                        break;
                    }
                    if (tok as i16) < 0 {
                        command(x, &mut o, &mut a6, tok);
                    } else if tok == 0x7003 {
                        let w = if x.a5_u8(0x35) == 1 { 0x2013 } else { 0x2018 };
                        o.put(x, w);
                        o.n += 1;
                    } else {
                        o.put(x, tok);
                        o.n += 1;
                    }
                }
                o.end(x);
            }
            10 => build_9(x, &mut o, a6),
            12 => x.set16(0x32, 0xffff),
            13 => text_with_name(x, &mut o, a6, 0x7005, false),
            15 => {
                let a1 = x.u32(0x24);
                let first = x.data32(a1);
                x.set32(0x24, first);
                if x.a5_u8(0x35) != 1 {
                    let second = x.data32(a1 + 4);
                    x.set32(0x24, second);
                }
                build(x, 3);
            }
            16 => {
                x.set8(0x23, 1);
                let slot = x.linked(0x3a);
                let v = x.u32(0x24);
                slot.set_u32(x.b, 0, v);
            }
            17 => build_16(x, &mut o, a6),
            18 => {
                build(x, 1);
                if let Ok(child) = crate::effects_aids::scenery_6(x.b, 0x13) {
                    child.set_u16(x.b, 0x46, x.o.0 as u16);
                    let v = x.u8(0x34);
                    child.set_u8(x.b, 0x2f, v);
                }
            }
            21 => build_20(x, &mut o, a6),
            1 | 5 | 9 | 11 | 14 | 19 | 20 | 22 | 23 => {
                plain(x, &mut o, &mut a6);
                let v = rd16(x.b, BUFFER);
                x.set16(0x30, v);
            }
            _ => {}
        }
    }

    /// Text whose token `special` calls the name of a player located at 0xff0fd2 (kinds 12, 20).
    fn text_with_name<R: Bench>(x: &mut Cx<'_, R>, o: &mut Out, mut a6: u32, special: u16, _alt: bool) {
        loop {
            let tok = next(x, &mut a6);
            if tok == 0 {
                return o.end(x);
            }
            if (tok as i16) < 0 {
                command(x, o, &mut a6, tok);
            } else if tok == special {
                o.put32(x, NAME_OPEN);
                name_category(x, o, 0xff_0fd2);
                o.put32(x, NAME_CLOSE);
            } else {
                o.put(x, tok);
                o.n += 1;
            }
        }
    }

    /// Kind 9: the class 5 player (byte `+3` >> 1) gives its name in place of the token 0x7004; without this player,
    /// the text is only a link to the other text.
    fn build_9<R: Bench>(x: &mut Cx<'_, R>, o: &mut Out, a6: u32) {
        let mut found = None;
        for p in players() {
            if p.u8(x.b, pl::ACTIVE) == 0 || (p.u8(x.b, 4) as i8) >= 4 {
                continue;
            }
            if p.u8(x.b, 3) >> 1 == 5 {
                found = Some(p);
                break;
            }
        }
        let Some(player) = found else {
            let slot = x.linked(0x3a);
            let v = x.data32(a6);
            slot.set_u32(x.b, 0, v);
            return;
        };
        let mut a6 = x.data32(a6 + 4);
        loop {
            let tok = next(x, &mut a6);
            if tok == 0 {
                return o.end(x);
            }
            if (tok as i16) < 0 {
                command(x, o, &mut a6, tok);
            } else if tok == 0x7004 {
                name(x, o, player.0 + 0x118);
            } else {
                o.put(x, tok);
                o.n += 1;
            }
        }
    }

    /// Kind 16: tokens 0x7006 to 0x7009 = names of the owner `+0x46`, equipped objects...
    fn build_16<R: Bench>(x: &mut Cx<'_, R>, o: &mut Out, mut a6: u32) {
        loop {
            let tok = next(x, &mut a6);
            if tok == 0 {
                // end: looks for a test piece (0xce00a, type 17) to link to this text
                wr16(x.b, o.at, tok);
                if let Ok(created) = crate::families::creators::set_15_pure(x.b, 0x11) {
                    created.set_u16(x.b, 0x46, x.o.0 as u16);
                    let v = x.u16(0x34);
                    created.set_u16(x.b, 0x42, v);
                } else {
                    x.add8(0x5, 0xfe);
                }
                return;
            }
            if (tok as i16) < 0 {
                command(x, o, &mut a6, tok);
                continue;
            }
            if (tok as i16) < 0x7006 {
                o.put(x, tok);
                o.n += 1;
                continue;
            }
            let entry = x.rom16(0xd7988 + 2 * (tok - 0x7006) as u32) as u32;
            match 0xd7988 + entry {
                0xd7990 => owner_name(x, o),
                0xd799c => equipped_item(x, o),
                0xd7a72 => {
                    if x.u16(0x46) != 0 {
                        owner_name(x, o)
                    }
                }
                _ => {}
            }
        }
    }

    /// Name of the window's owner (`+0x46`) with its openings (routine 0xd75ba).
    fn owner_name<R: Bench>(x: &mut Cx<'_, R>, o: &mut Out) {
        let owner = x.linked(0x46);
        o.put32(x, NAME_OPEN);
        name(x, o, owner.0 + 0x118);
    }

    /// First equipped object of the owner that appears in one of the lists of kind 16 (routines 0xd799c, 0xd7a38 and
    /// 0xd7a5c): copies the object's text.
    fn equipped_item<R: Bench>(x: &mut Cx<'_, R>, o: &mut Out) {
        let owner = x.linked(0x46);
        let (a, b, c) = if owner.u8(x.b, 3) >> 1 == 0 {
            (0xd7a20, 0xd7a20, 0xd7a26)
        } else {
            (0xd7a2c, 0xd7a2c, 0xd7a32)
        };
        let mut candidates = vec![(a, owner.u8(x.b, 0x10d)), (b, owner.u8(x.b, 0x10e))];
        for i in 0..6 {
            candidates.push((c, owner.u8(x.b, 0x140 + i)));
        }
        for (list, value) in candidates {
            let mut index = None;
            let mut at = list;
            let mut i = 0;
            loop {
                let e = x.data8(at);
                at += 1;
                if (e as i8) < 0 {
                    break;
                }
                if e == value {
                    index = Some(i);
                    break;
                }
                i += 1;
            }
            if let Some(i) = index {
                let text = x.rom16(0xd7a48 + 4 * i);
                let mut text = (text as u32) << 16 | x.rom16(0xd7a48 + 4 * i + 2) as u32;
                loop {
                    let w = x.data16(text);
                    if w == 0 {
                        return;
                    }
                    o.put(x, w);
                    o.n += 1;
                    text += 2;
                }
            }
        }
    }

    /// Kind 20: like kind 12 (token 0x7005), but preceded by a palette change in a one-player game; token 0x7000
    /// skipped in solo or replaced by 0x2003.
    fn build_20<R: Bench>(x: &mut Cx<'_, R>, o: &mut Out, mut a6: u32) {
        o.put32(x, 0xfffe_000c);
        if x.a5_u8(0x50) != 0 && x.a5_u8(0x42) != 0 {
            o.at -= 4;
            copy_palette(x, 0x292260, 0x91c260);
        }
        loop {
            let tok = next(x, &mut a6);
            if tok == 0 {
                return o.end(x);
            }
            if (tok as i16) < 0 {
                command(x, o, &mut a6, tok);
            } else if tok == 0x7000 {
                if x.a5_u8(0x35) != 1 {
                    o.put(x, 0x2003);
                    o.n += 1;
                }
            } else if tok == 0x7005 {
                name_category(x, o, 0xff_0fd2);
                // the rest of the text is read as for kind 12
                return text_with_name(x, o, a6, 0x7005, true);
            } else {
                o.put(x, tok);
                o.n += 1;
            }
        }
    }

    // --- loading a window definition -------------------------------------------------------------

    /// Routine 0xd7124: reads the definition at `def`, computes the screen address and applies the kind's
    /// initialization. Returns the screen address (A1 at the routine's exit) and, if a palette was copied, the D6 that
    /// it leaves (`copy_palette`).
    fn load<R: Bench>(x: &mut Cx<'_, R>, mut def: u32) -> (u32, Option<u32>) {
        x.set16(0x30, 0);
        wr16(x.b, BUFFER, 0);
        let col = x.data8(def) as u16;
        let row = x.data8(def + 1) as u16;
        let v = x.data8(def + 2);
        x.set8(0x18, v);
        let v = x.data8(def + 3);
        x.set8(0x19, v);
        let v = x.data32(def + 4);
        x.set32(0x24, v);
        let v = x.data16(def + 8);
        x.set16(0x8, v);
        let v = x.data16(def + 10);
        x.set16(0xc, v);
        let v = x.data8(def + 12);
        x.set8(0x2f, v);
        let v = x.data8(def + 13);
        x.set8(0x1a, v);
        let v = x.data16(def + 14);
        x.set16(0x32, v);
        let v = x.data16(def + 16);
        x.set16(0x34, v);
        def += 18;
        x.set32(0x1c, def);
        let offset = (col << 7).wrapping_add(row << 2) as i16 as i32 as u32;
        let a1 = SCREEN.wrapping_add(offset);
        x.set32(0x14, a1);
        let height = if x.u8(0x18) != 0 { 0xb } else { 0xf };
        x.set8(0x20, height);
        let v = x.u8(0x19);
        x.set8(0x21, v);
        kind_init(x, a1)
    }

    /// Initialization specific to the kind (jump table 0xd7196).
    fn kind_init<R: Bench>(x: &mut Cx<'_, R>, a1: u32) -> (u32, Option<u32>) {
        match x.u8(0x2f) {
            1 => {
                let script = x.u32(0x24);
                let pick = if x.a5_u8(0x35) == 1 { 0 } else { 4 };
                let next = x.data32(script + pick);
                load(x, next)
            }
            2 | 4 => {
                let more = x.a5_u8(0x35).wrapping_sub(1).wrapping_mul(2);
                x.add8(0x19, more);
                let v = x.u8(0x19);
                x.set8(0x21, v);
                (a1, None)
            }
            5 => {
                let mut d0 = (x.u8(0x42) as u32 * 2) as u8;
                d0 = d0.wrapping_add(x.a5_u8(0x6f) & 1);
                let step = x.rom8(0xd7218 + d0 as u32) as u32;
                let script = x.u32(0x24).wrapping_add(step);
                load(x, script)
            }
            9 | 17 | 21 => {
                x.set8(0x5, 2);
                x.set8(0x19, 0);
                x.set8(0x21, 0);
                x.set8(0x20, 0xff);
                match x.u8(0x2f) {
                    9 => {
                        x.set8(0x3, 1);
                        x.set8(0x2f, 0);
                        (a1, None)
                    }
                    17 => {
                        x.set8(0x1b, 1);
                        copy_palette(x, PALETTE_ALT, 0x91c300);
                        x.set16(0x32, 0xffff);
                        let d6 = copy_palette(x, PALETTE_SRC, 0x91c1e0);
                        for base in [0x91c1e0, 0x91c300] {
                            for i in 0..8 {
                                let v = rd32(x.b, base + 4 * i) & 0x0fff_0fff;
                                wr32(x.b, base + 4 * i, v);
                            }
                        }
                        (0x91c200, Some(d6))
                    }
                    _ => {
                        if rd8(x.b, FLAG_SKIP) != 0 {
                            x.set8(0x5, 6);
                            x.set8(0x1b, 1);
                            return (a1, None);
                        }
                        x.set8(0x1b, 1);
                        let d6 = copy_palette(x, PALETTE_SRC, 0x91c180);
                        (palette_tail(x, 0x91c180), Some(d6))
                    }
                }
            }
            11 => {
                let script = x.u32(0x24);
                let kind = x.u8(0x23).wrapping_mul(4) as u32;
                let next = x.data32(script + kind);
                load(x, next)
            }
            13 => {
                let d6 = copy_palette(x, PALETTE_ALT, 0x91c300);
                (0x91c320, Some(d6))
            }
            14 => {
                x.set8(0x36, 1);
                x.set8(0x2f, 0);
                (a1, None)
            }
            19 => {
                x.set8(0x38, 0x10);
                let mut script = x.u32(0x24);
                for _ in 0..x.u8(0x42) {
                    script += 0x12;
                }
                load(x, script)
            }
            20 => {
                let mut script = x.u32(0x24);
                for p in players() {
                    if p.u8(x.b, pl::ACTIVE) == 0 || (p.u8(x.b, 4) as i8) >= 4 {
                        continue;
                    }
                    if p.u8(x.b, 3) >> 1 == 3 {
                        script += 0x24;
                        break;
                    }
                }
                load(x, script)
            }
            22 => {
                x.set8(0x36, 1);
                x.set8(0x2f, 1);
                (a1, None)
            }
            23 => {
                x.set32(0x4, 0x0400_0000);
                (a1, None)
            }
            _ => (a1, None),
        }
    }

    /// Window light: if a game is in progress and its bit 0 is set (`A5+0x48e`), the brightness of the palette 0x91c2a0
    /// is carried over to the eight longs starting at `a1` (0x91c1e0 or 0x91c380). Returns the address after the eight
    /// longs.
    fn palette_tail<R: Bench>(x: &mut Cx<'_, R>, a1: u32) -> u32 {
        if x.a5_u8(0x48f) == 0 || x.a5_u8(0x48e) & 1 == 0 {
            return a1;
        }
        let bright = rd32(x.b, 0x91c2a0) & 0xf000_f000;
        for i in 0..8 {
            let v = rd32(x.b, a1 + 4 * i) & 0x0fff_0fff | bright;
            wr32(x.b, a1 + 4 * i, v);
        }
        a1 + 0x20
    }

    // --- states -----------------------------------------------------------------------------

    fn open<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        x.set8(0x36, 3);
        let variant = x.u8(0x3) as u32;
        let def = x.data32(DEFINITIONS + 4 * variant);
        load(x, def);
        let stage = x.a5_u8(0x34);
        let a1 = if (0x45..0x5a).contains(&stage) { 0x91c380 } else { 0x91c1e0 };
        match x.u8(0x2f) {
            0x11 => return,
            0x15 => {
                if rd8(x.b, FLAG_SKIP) == 0 {
                    return;
                }
                return x.set8(0x2f, 0);
            }
            _ => {}
        }
        copy_palette(x, PALETTE_SRC, a1);
        palette_tail(x, a1);
    }

    fn step<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x5) {
            0 => {
                x.add8(0x5, 2);
                let v = x.u8(0x36);
                x.set8(0x1b, v);
                let a1 = x.u32(0x14);
                frame_top(x, a1);
                x.add16(0x16, 8);
                let a1 = x.u32(0x14);
                frame_middle(x, a1);
            }
            2 => grow(x),
            4 => wait_key(x),
            6 => shrink(x),
            _ => {}
        }
    }

    fn rd8(b: &mut cps2::Cps2Bus, a: u32) -> u8 {
        crate::mem::rd8(b, a)
    }

    /// Substate 2: the frame grows by one line every `+0x36` frames.
    fn grow<R: Bench>(x: &mut Cx<'_, R>) {
        let n = x.u8(0x1b).wrapping_sub(1);
        x.set8(0x1b, n);
        if (n as i8) > 0 {
            return;
        }
        let v = x.u8(0x36);
        x.set8(0x1b, v);
        if x.u8(0x21) != 0 {
            x.add8(0x21, 0xff);
            let a1 = x.u32(0x14);
            frame_side(x, a1);
            x.add16(0x16, 4);
            let a1 = x.u32(0x14);
            frame_middle(x, a1);
            return;
        }
        x.add8(0x5, 2);
        let v = x.u8(0x19);
        x.set8(0x21, v);
        let slot = SLOTS + x.u8(0x38) as u32;
        x.set16(0x3a, slot as u16);
        let v = x.u16(0x8);
        wr16(x.b, slot + 4, v);
        let v = x.u16(0xc);
        wr16(x.b, slot + 6, v);
        let kind = x.u8(0x2f);
        if kind == 0 {
            let v = x.u32(0x24);
            wr32(x.b, slot, v);
            return;
        }
        if kind != 0xc {
            wr32(x.b, slot, BUFFER);
        }
        build(x, kind);
    }

    /// Substate 4: waits for a player to press (countdown `+0x32`; -1 = endless).
    fn wait_key<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u16(0x32) == 0xffff {
            return;
        }
        if poll_keys(x) != 0 {
            x.set16(0x32, 0);
        }
        if x.u16(0x32) == 0 {
            x.add8(0x5, 2);
            if x.u8(0x2f) == 0xc {
                return;
            }
            let slot = x.linked(0x3a);
            slot.set_u32(x.b, 0, 0);
            return;
        }
        x.add16(0x32, 0xffff);
    }

    /// Keys pressed by the players concerned (byte `A5+0xc+n`, bits 4 to 7).
    fn poll_keys<R: Bench>(x: &mut Cx<'_, R>) -> u8 {
        let mut who: i8 = -1;
        if x.u8(0x3) == 0x8b {
            who = x.u8(0x43) as i8;
        } else if (x.u8(0x37) as i8) >= 0 {
            who = x.u8(0x37) as i8;
        }
        if x.a5_u8(0x30) == 0 {
            return 0;
        }
        let mut keys = 0u8;
        for n in (0..4u32).rev() {
            let p = Obj(PLAYERS + n * PLAYER_SIZE);
            if p.u8(x.b, pl::ACTIVE) == 0 || (p.u8(x.b, 4) as i8) >= 4 {
                continue;
            }
            if who >= 0 && p.u8(x.b, 2) as i8 != who {
                continue;
            }
            keys |= x.a5_u8(0xc + n);
        }
        keys & 0xf0
    }

    /// Substate 6: the frame closes by one line per step; then the object is deleted.
    fn shrink<R: Bench>(x: &mut Cx<'_, R>) {
        let n = x.u8(0x1b).wrapping_sub(1);
        x.set8(0x1b, n);
        if (n as i8) > 0 {
            return;
        }
        let v = x.u8(0x36);
        x.set8(0x1b, v);
        let mut a6;
        // D6: number of lines of the window (byte), which the loading of a definition may overwrite
        let d6;
        if x.u8(0x21) != 0 {
            x.add8(0x21, 0xff);
            let a1 = x.u32(0x14);
            erase_column(x, a1);
            x.add16(0x16, 0xfffc);
            let a1 = x.u32(0x14);
            return frame_middle(x, a1);
        }
        if (x.u8(0x1a) as i8) >= 0 {
            x.set8(0x5, 0);
            a6 = x.u32(0x14).wrapping_sub(8);
            let lines = x.u8(0x20) as u32;
            let def = x.u32(0x1c);
            let (a1, palette_d6) = load(x, def);
            d6 = palette_d6.unwrap_or(lines);
            if x.u8(0x20) == d6 as u8 && a1 == a6 {
                return;
            }
        } else {
            x.add8(0x5, 2);
            x.set8(0x22, 1);
            x.set32(0x4, 0x0600_0000);
            a6 = x.u32(0x14).wrapping_sub(8);
            d6 = x.u8(0x20) as u32;
        }
        if (d6 as u8 as i8) < 0 {
            return;
        }
        let count = (d6 as u16).wrapping_mul(2).wrapping_add(1);
        for _ in 0..=count {
            for k in 0..3 {
                wr32(x.b, a6 + 4 * k, 0x20_0000);
            }
            a6 += 0xc + 0x74;
        }
    }

    fn close<R: Bench>(x: &mut Cx<'_, R>) {
        let n = x.a5_u8(0x51).wrapping_sub(1);
        x.set_a5_u8(0x51, n);
        x.set8(0x22, 0);
        let v = crate::mem::rd8(x.b, OPEN_COUNT).wrapping_sub(1);
        crate::mem::wr8(x.b, OPEN_COUNT, v);
        x.free(&pools::SET);
    }

    // --- drawing the frame --------------------------------------------------------------------

    fn frame_top<R: Bench>(x: &mut Cx<'_, R>, mut a1: u32) {
        let tiles: [u32; 2] = if x.u8(0x18) != 0 { [0x0434_000f, 0x0444_000f] } else { [0x0400_000f, 0x0410_000f] };
        let (mut d0, mut d1) = (tiles[0], tiles[1]);
        let rows = x.u8(0x20) as i16;
        for _ in 0..=rows {
            wr32(x.b, a1, d0);
            wr32(x.b, a1 + 4, d1);
            d0 = d0.wrapping_add(0x1_0000);
            d1 = d1.wrapping_add(0x1_0000);
            a1 += 0x80;
        }
        d0 = (d0 & !0xff) | (d0 as u8 | 0x20) as u32;
        d1 = (d1 & !0xff) | (d1 as u8 | 0x20) as u32;
        d0 = d0.wrapping_add(0xffff_0000);
        d1 = d1.wrapping_add(0xffff_0000);
        let rows = x.u8(0x20) as i16;
        for _ in 0..=rows {
            wr32(x.b, a1, d0);
            wr32(x.b, a1 + 4, d1);
            d0 = d0.wrapping_add(0xffff_0000);
            d1 = d1.wrapping_add(0xffff_0000);
            a1 += 0x80;
        }
    }

    fn frame_side<R: Bench>(x: &mut Cx<'_, R>, mut a1: u32) {
        let mut d0 = if x.u8(0x18) != 0 { 0x0432_000f } else { 0x0430_000f };
        wr32(x.b, a1, d0);
        a1 += 0x80;
        d0 = (d0 & !0xff) | (d0 as u8 | 0x20) as u32;
        let count = (x.u8(0x20) as u16).wrapping_mul(2).wrapping_sub(1) as u32 + 1;
        for _ in 0..count {
            wr32(x.b, a1, 0x0431_000f);
            a1 += 0x80;
        }
        wr32(x.b, a1, d0);
    }

    fn frame_middle<R: Bench>(x: &mut Cx<'_, R>, mut a1: u32) {
        let mut d0 = if x.u8(0x18) != 0 { 0x0454_000f } else { 0x0420_000f };
        let rows = x.u8(0x20) as i16;
        for _ in 0..=rows {
            wr32(x.b, a1, d0);
            d0 = d0.wrapping_add(0x1_0000);
            a1 += 0x80;
        }
        d0 = (d0 & !0xff) | (d0 as u8 | 0x20) as u32;
        d0 = d0.wrapping_add(0xffff_0000);
        let rows = x.u8(0x20) as i16;
        for _ in 0..=rows {
            wr32(x.b, a1, d0);
            d0 = d0.wrapping_add(0xffff_0000);
            a1 += 0x80;
        }
    }

    fn erase_column<R: Bench>(x: &mut Cx<'_, R>, mut a1: u32) {
        let rows = (x.u8(0x20) as u16).wrapping_mul(2).wrapping_add(1);
        for _ in 0..=rows {
            wr32(x.b, a1, 0x20_0000);
            a1 += 0x80;
        }
    }
}

handlers! {
    0xd4110 "set_16" bumper::run,
    0xd7056 "set_24" dialog::run,
    0xd9424 "set_30" game_over::run,
    0xd9492 "set_31" level_clock::run,
    0xd96d0 "set_32" test_selector::run,
    0xd8458 "set_27" door_a::run,
    0xd8ce4 "set_28" zone_end::run,
    0xd8dce "set_29" door_b::run,
    0xd801e "set_25" rearguard::run,
    0xd810a "set_26" idle_hunter::run,
    0xd69c2 "set_22" rank_adjust::run,
    0xd6c5c "set_23" exit_gate::run,
    0xd5f5a "set_20" life_bar::run,
    0xd6372 "set_21" continue_countdown::run,
    0xd466a "set_17" boss_intro::run,
    0xd49a2 "set_19" wave_spawner::run,
    0xd4950 "set_18" scroll_to::run,
}
