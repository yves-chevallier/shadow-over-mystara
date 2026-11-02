//! Type SET 15 (0xce024): end-of-level staging and transition screens (h).
//!
//! The type is a *dispatcher*: variant `+3` (0 to 24) picks a small state machine
//! independent of the others (`+4` main state, `+5` substate) whose address is in table 0xce034 of
//! the ROM. These machines animate the screen between two levels (camera shakes, scrolling of
//! scenery loaded by 0x26d4 / 0x2766, palette fades, appearance of objects tied by `+0x4c` to a
//! "conductor" object). They are never called in the scenarios (see
//! `docs/families/set_c.md`). The variants not yet rewritten are run by the original code.

use crate::families::cx::Bench;
use crate::families::cx::Cx;
use crate::mem::wr16;
use crate::objects::Obj;

use crate::objects::pools;
use crate::objects::Current;
use crate::machine::Machine;
use crate::families::set::tables as set_tables;
use crate::families::set::vars;
use crate::families::set::fields::SetObj;
use crate::families::set_c::fields::ScreenObj;
use crate::families::set_c::fields::ScrollScreenObj;
use crate::families::set_c::fields::BossCameraFields;
use crate::families::set_c::fields::ScrollSpeedObj;

/// Table (program space) of the machines' movements, one relative word per variant.
const VARIANTS: u32 = 25;

/// Block grid loaders: 0x26d4 (scroll 2) and 0x2766 (small blocks, scroll 3).
mod rom {
    use crate::video::Grid;
    pub const LOAD_BLOCKS: Grid = Grid::Layer2;
    pub const LOAD_SMALL: Grid = Grid::Layer3;
}

/// Work RAM variables (offsets from A5).
mod a5 {
    /// Background camera: position (X, Y) and its "previous" copy.
    pub const CAM_X: u32 = 0x506;
    pub const CAM_Y: u32 = 0x50a;
    pub const CAM_X_PREV: u32 = 0x50e;
    pub const CAM_Y_PREV: u32 = 0x512;
    /// Second plane (scroll 2): position and copy.
    pub const BG_X: u32 = 0x556;
    pub const BG_Y: u32 = 0x55a;
    pub const BG_X_PREV: u32 = 0x55e;
    pub const BG_Y_PREV: u32 = 0x562;
}

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    let variant = x.u8(Obj::VARIANT) as u32;
    if variant >= VARIANTS {
        return;
    }
    // The state tables of all variants (except 19 and 20, which have no table) end the same way:
    // state 4 = go to 6 (0xce0e8) or, for 0 and 1, release; state 6 = release (0xce0e2); for 24
    // state 4 is specific to the variant.
    if !matches!(variant, 19 | 20) {
        match (variant, x.u8(Obj::STATE)) {
            (0 | 1, 4 | 6) | (_, 6) => return free(x),
            (24, 4) => {}
            (_, 4) => return x.set8(Obj::STATE, 6),
            _ => {}
        }
    }
    match variant {
        0 => title_blink(x),
        1 => quake(x),
        2 => backdrop_slide(x),
        3 => camera_script(x),
        4 => scene_scroll(x),
        5 => block_loader(x),
        6 => scene_scroll_follow(x),
        7 => status::status_window(x),
        8 => later::hp_refill_a(x),
        9 => later::spawn_player_effects(x),
        10 => later::shaking_intro(x),
        11 => later::handover_effects(x),
        12 => later::spawn_player_markers(x),
        13 => later::fireworks(x),
        14 => later::lightning(x),
        15 => later::quake_light(x),
        16 => later::ranking_start(x),
        17 => later::palette_fade(x),
        18 => later::bonus_selection(x),
        19 => later::countdown_digits(x),
        20 => later::credits_scroll(x),
        21 => later::flicker(x),
        22 => later::name_entry(x),
        23 => later::hp_refill_b(x),
        _ => later::results_panel(x),
    }
}

// --- aides ------------------------------------------------------------------------------------

fn next_state<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Obj::STATE, 2);
}

fn next_substate<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(SetObj::SUBSTATE, 2);
}

/// Sets the object to state 6 (release on the next pass).
fn free<R: Bench>(x: &mut Cx<'_, R>) {
    x.free(&pools::SET);
}

/// The conductor: the object pointed to by `+0x4c` (long address).
fn conductor<R: Bench>(x: &mut Cx<'_, R>) -> Obj {
    Obj(x.u32(ScreenObj::CONDUCTOR))
}

fn rng<R: Bench>(x: &mut Cx<'_, R>) -> u16 {
    x.random()
}

/// Loads block grid `grid` at `dest` through 0x26d4 or 0x2766; returns the end of the written blocks (the original's
/// A1, where the next call goes on).
pub(super) fn load_blocks<R: Bench>(x: &mut Cx<'_, R>, grid_kind: crate::video::Grid, dest: u32, grid: u16) -> u32 {
    crate::video::load_grid(x.b, grid_kind, grid, dest).destination
}

/// Shake: adds to the camera displacement number `+0x18` & 3 of table `table` (word pairs); returns the X offset (the
/// original's D1, read back by the effect creation that follows).
fn shake<R: Bench>(x: &mut Cx<'_, R>, table: u32) -> u16 {
    let step = (x.u8(ScrollScreenObj::STEP) & 3) as u32;
    x.add8(ScrollScreenObj::STEP, 1);
    let (dx, dy) = (set_tables::word(x.b, table + 4 * step), set_tables::word(x.b, table + 4 * step + 2));
    // Original glitch: the Y offset stays in D2 (`dispatch::Leftovers`)
    *x.c.d2() = *x.c.d2() & 0xffff_0000 | dy as u32;
    let (cx, cy) = (x.a5_u16(a5::CAM_X), x.a5_u16(a5::CAM_Y));
    x.set_a5_u16(a5::CAM_X, cx.wrapping_add(dx));
    x.set_a5_u16(a5::CAM_Y, cy.wrapping_add(dy));
    dx
}

/// Advances a 32-bit field by an 8.8 speed word (signed `speed << 8`).
fn velocity(speed: u16) -> u32 {
    (((speed as u32) << 16) as i32 >> 8) as u32
}

// --- variant 0: title blinking ----------------------------------------------------------------------

/// Variant 0 (0xce066): initializes two routines (0x3525c, 0x1ac86), then, from time to time (rng
/// and frame parity), toggles `+0x43` and launches one of two sound/video routines.
fn title_blink<R: Bench>(x: &mut Cx<'_, R>) {
    const ON: crate::sound::Sound = crate::sound::CMD_1FB;
    const OFF: crate::sound::Sound = crate::sound::CMD_1FD;
    const FLASH: u32 = 0x4ba;
    match x.u8(Obj::STATE) {
        0 => {
            next_state(x);
            crate::families::small::clear_scroll1_start(x);
            crate::palette::set_flash_color(x.b, crate::palette::FLASH_COLOR_WHITE_VALUE);
            x.set8(ScrollScreenObj::DELAY, 0);
            x.set_a5_u16(FLASH, 0);
        }
        2 => {
            if x.u8(ScrollScreenObj::DELAY) != 0 {
                x.add8(ScrollScreenObj::DELAY, 0xff);
                if x.u8(ScrollScreenObj::DELAY) == 0 {
                    x.set_a5_u16(FLASH, 0);
                }
                return;
            }
            if x.a5_u8(vars::FRAMES_LOW) & 3 != 0 || rng(x) & 7 != 0 {
                return;
            }
            x.set8(ScrollScreenObj::DELAY, 3);
            x.set_a5_u16(FLASH, 0x100);
            let was_on = x.bit(ScrollScreenObj::FLASH, 0);
            x.set_bit(ScrollScreenObj::FLASH, 0, !was_on);
            crate::sound::play_global(x.b, if was_on { ON } else { OFF });
        }
        _ => free(x),
    }
}

// --- variant 1: end-of-level shake ----------------------------------------------------------------

/// Variant 1 (0xce0f0): places the object at the bottom of the screen, waits, then shakes the camera with
/// shake tables (0xd0b10, 0xd0b20) while launching effects; the conductor (`+0x4c`)
/// is notified through its byte `+8` and through `+0xc`.
fn quake<R: Bench>(x: &mut Cx<'_, R>) {
    const SHAKE_A: u32 = 0xd0b10;
    const SHAKE_B: u32 = 0xd0b20;
    const DELAYS: u32 = 0xce138;
    match x.u8(Obj::STATE) {
        0 => {
            next_state(x);
            x.set8(ScrollScreenObj::TIMER, 0x28);
            let v = x.a5_u16(a5::CAM_X).wrapping_add(0xc0);
            x.set16(Obj::X, v);
            let v = x.a5_u16(a5::CAM_Y).wrapping_add(0x80);
            x.set16(Obj::Z, v);
            quake_delay(x, DELAYS);
        }
        2 => match x.u8(SetObj::SUBSTATE) {
            0 => {
                x.add8(ScrollScreenObj::TIMER, 0xff);
                if x.u8(ScrollScreenObj::TIMER) != 0 {
                    return;
                }
                next_substate(x);
                x.set8(ScrollScreenObj::TIMER, 0x3c);
                x.set8(ScrollScreenObj::STEP, 0);
            }
            2 => {
                shake(x, SHAKE_A);
                x.add8(ScrollScreenObj::TIMER, 0xff);
                if x.u8(ScrollScreenObj::TIMER) != 0 {
                    return;
                }
                x.set8(ScrollScreenObj::TIMER, 0x78);
                next_substate(x);
            }
            4 => {
                let dx = shake(x, SHAKE_B);
                quake_effects(x, DELAYS, dx as u8);
                x.add8(ScrollScreenObj::TIMER, 0xff);
                if x.u8(ScrollScreenObj::TIMER) != 0 {
                    return;
                }
                next_substate(x);
                let boss = conductor(x);
                let v = boss.u8(x.b, Obj::Z).wrapping_sub(1);
                boss.set_u8(x.b, Obj::Z, v);
            }
            6 => {
                shake(x, SHAKE_B);
                let boss = conductor(x);
                if boss.u8(x.b, Obj::X) != 2 {
                    return;
                }
                next_substate(x);
                x.add16(Obj::Z, 0u16.wrapping_sub(0x290));
            }
            8 => {
                let dx = shake(x, SHAKE_B);
                quake_effects(x, DELAYS, dx as u8);
                let boss = conductor(x);
                if boss.u8(x.b, Obj::X) != 3 {
                    return;
                }
                next_state(x);
                x.set_a5_u16(a5::CAM_X, 0);
                x.set_a5_u16(a5::CAM_Y, 0x70);
            }
            _ => {}
        },
        _ => free(x),
    }
}

/// Random delay before the next effect: byte of table 0xce138 drawn by the rng.
fn quake_delay<R: Bench>(x: &mut Cx<'_, R>, table: u32) {
    let r = rng(x) & 7;
    let d = set_tables::pc_byte(x.b, table + r as u32);
    x.set8(ScrollScreenObj::DELAY, d);
}

/// Creates an effect (type 0 of 0xb1a7a) tied to the object when the delay `+0x42` has elapsed.
fn quake_effects<R: Bench>(x: &mut Cx<'_, R>, table: u32, setting: u8) {
    x.add8(ScrollScreenObj::DELAY, 0xff);
    if x.u8(ScrollScreenObj::DELAY) != 0 {
        return;
    }
    x.set8(ScrollScreenObj::DELAY, 1);
    // 0xb1a7a, variant 0; original glitch: the setting is the low byte of the D1 left by the shake
    let Some(child) = crate::families::effects_creators::countdown(x.b, 0, setting) else { return };
    child.set_u16(x.b, ScrollScreenObj::PARENT, x.o.0 as u16);
    let owner = x.u32(ScreenObj::CONDUCTOR);
    child.set_u32(x.b, ScreenObj::CONDUCTOR, owner);
    quake_delay(x, table);
}

// --- variant 2: sliding background ---------------------------------------------------------------

/// Variant 2 (0xce294): a background (scroll 3, GFX 0x920000) whose position `+0x34` accelerates;
/// `+5` = 0: it enters; 2: it leaves. Writes its position into the block's 176 words when it changes.
fn backdrop_slide<R: Bench>(x: &mut Cx<'_, R>) {
    const TILEMAP: u32 = 0x920000;
    match x.u8(Obj::STATE) {
        0 => {
            next_state(x);
            x.set32(Obj::X, 0);
            x.set32(ScrollScreenObj::SLIDE, 0);
        }
        2 => {
            let leaving = x.u8(SetObj::SUBSTATE) != 0;
            x.add32(ScrollScreenObj::SLIDE, if leaving { 0x4000 } else { 0x3800 });
            let at = x.a5_u16(a5::CAM_X).wrapping_add(x.u16(ScrollScreenObj::SLIDE));
            if leaving {
                if at >= 0xa0 {
                    x.set8(Obj::STATE, 6);
                    return;
                }
            } else if at >= 0x60 {
                return next_substate(x);
            }
            let pos = x.u16(ScrollScreenObj::SLIDE);
            if pos == x.u16(Obj::X) {
                return;
            }
            x.set16(Obj::X, pos);
            for i in 0..0xb0 {
                wr16(x.b, TILEMAP + 2 * i, pos);
            }
        }
        _ => {}
    }
}

// --- variant 3: the scenery positions itself from the conductor -----------------------------------

/// Variant 3 (0xce324): reads from a table (0xce37a, 16 bytes) the camera position, speed
/// and goal according to the conductor's `+0x3f`, loads two block grids into scroll 3
/// (0x917000), then slides the camera toward the goal; the conductor is then decremented.
fn camera_script<R: Bench>(x: &mut Cx<'_, R>) {
    const TABLE: u32 = 0xce37a;
    match x.u8(Obj::STATE) {
        0 => {
            next_state(x);
            let boss = conductor(x);
            let row = TABLE + ((boss.u8(x.b, BossCameraFields::CAMERA_ROW) as u32) << 4);
            let w = |x: &mut Cx<'_, R>, i: u32| set_tables::word(x.b, row + 2 * i);
            let (cam_x, cam_y) = (w(x, 0), w(x, 1));
            x.set_a5_u16(a5::CAM_X, cam_x);
            x.set_a5_u16(a5::CAM_Y, cam_y);
            for (field, i) in [(ScrollSpeedObj::SPEED_X, 2), (ScrollSpeedObj::SPEED_Y, 3), (Obj::X, 4), (Obj::Z, 5)] {
                let v = w(x, i);
                x.set16(field, v);
            }
            let (g0, g1) = (w(x, 6), w(x, 7));
            let end = load_blocks(x, rom::LOAD_BLOCKS, 0x917000, g0);
            load_blocks(x, rom::LOAD_BLOCKS, end, g1);
        }
        2 => {
            let (vx, vy) = (velocity(x.u16(ScrollSpeedObj::SPEED_X)), velocity(x.u16(ScrollSpeedObj::SPEED_Y)));
            let v = x.a5_u32(a5::CAM_X).wrapping_add(vx);
            x.set_a5_u32(a5::CAM_X, v);
            let v = x.a5_u32(a5::CAM_Y).wrapping_add(vy);
            x.set_a5_u32(a5::CAM_Y, v);
            if x.u16(Obj::X) != x.a5_u16(a5::CAM_X) || x.u16(Obj::Z) != x.a5_u16(a5::CAM_Y) {
                return;
            }
            next_state(x);
            let boss = conductor(x);
            let v = boss.u8(x.b, Obj::X).wrapping_sub(1);
            boss.set_u8(x.b, Obj::X, v);
        }
        _ => {}
    }
}

// --- variants 4 and 6: scrolling of a scene --------------------------------------------------------

/// Scene parameters (subroutine 0xce4a2, pointer table 0xce4ea indexed by `index`
/// bytes): background position and speeds, then a list of 4x4 blocks loaded into 0x918000.
fn load_scene<R: Bench>(x: &mut Cx<'_, R>, index: u32) {
    const SCENES: u32 = 0xce4ea;
    let at = SCENES.wrapping_add(index);
    let mut p = set_tables::pc_long(x.b, at);
    let mut next = |x: &mut Cx<'_, R>| {
        let w = set_tables::word(x.b, p);
        p += 2;
        w
    };
    for field in [Obj::X, Obj::Z, ScrollSpeedObj::SPEED_X, ScrollSpeedObj::SPEED_Y, ScrollScreenObj::MASK_X, ScrollScreenObj::MASK_Y] {
        let w = next(x);
        x.set16(field, w);
    }
    let (px, py) = (x.u16(Obj::X), x.u16(Obj::Z));
    x.set_a5_u16(a5::BG_X, px);
    x.set_a5_u16(a5::BG_Y, py);
    let count = next(x);
    let w = next(x);
    x.set16(ScrollScreenObj::OFFSET_X, w);
    for _ in 0..=count {
        let offset = next(x) as i16 as i32 as u32;
        let grid = next(x);
        load_blocks(x, rom::LOAD_SMALL, 0x918000u32.wrapping_add(offset), grid);
    }
}

/// One step of background scrolling: position += speed (8.8), masked by `+0x1c` / `+0x1e`, then
/// published to the second plane with the offset `+0x16`.
fn scroll_step<R: Bench>(x: &mut Cx<'_, R>) {
    let v = x.u32(Obj::X).wrapping_add(velocity(x.u16(ScrollSpeedObj::SPEED_X)));
    x.set32(Obj::X, v);
    let v = x.u32(Obj::Z).wrapping_add(velocity(x.u16(ScrollSpeedObj::SPEED_Y)));
    x.set32(Obj::Z, v);
    let mask = x.u16(ScrollScreenObj::MASK_X);
    let v = x.u16(Obj::X) & mask;
    x.set16(Obj::X, v);
    let bg_x = x.u16(Obj::X).wrapping_add(x.u16(ScrollScreenObj::OFFSET_X));
    x.set_a5_u16(a5::BG_X, bg_x);
    let mask = x.u16(ScrollScreenObj::MASK_Y);
    let v = x.u16(Obj::Z) & mask;
    x.set16(Obj::Z, v);
    let bg_y = x.u16(Obj::Z);
    x.set_a5_u16(a5::BG_Y, bg_y);
}

/// Variant 4 (0xce47c): loads scene `+0x2f` (bit 0 ignored), then scrolls it endlessly.
fn scene_scroll<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => {
            next_state(x);
            let index = (x.u8(SetObj::PARAM) as u32 & !1) * 4;
            load_scene(x, index & 0xffff);
        }
        2 => scroll_step(x),
        _ => {}
    }
}

fn publish_background<R: Bench>(x: &mut Cx<'_, R>) {
    let (px, py) = (x.a5_u16(a5::BG_X), x.a5_u16(a5::BG_Y));
    x.set_a5_u16(a5::BG_X_PREV, px);
    x.set_a5_u16(a5::BG_Y_PREV, py);
}

/// Variant 6 (0xce6bc): like 4 (scene `+0x2f`, bit 0 included), copying the background into its "previous"
/// copy, until the conductor switches to variant 2.
fn scene_scroll_follow<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => {
            next_state(x);
            let index = (x.u8(SetObj::PARAM) as u32) * 4;
            load_scene(x, index & 0xffff);
            publish_background(x);
        }
        2 => {
            scroll_step(x);
            publish_background(x);
            if conductor(x).u8(x.b, Obj::VARIANT) == 2 {
                next_state(x);
            }
        }
        _ => {}
    }
}

// --- variant 5: block loader ---------------------------------------------------------------------------

/// Variant 5 (0xce64a): loads two block grids (0xce686 + 4 × `+0x2f`) into scroll 3.
fn block_loader<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => {
            next_state(x);
            let index = x.u8(SetObj::PARAM) as u32;
            block_loader_for(x, index);
        }
        2 => next_state(x),
        _ => {}
    }
}

/// Loads the two block grids no. `index` (word pairs at 0xce686) into scroll 3.
fn block_loader_for<R: Bench>(x: &mut Cx<'_, R>, index: u32) {
    const GRIDS: u32 = 0xce686;
    let at = GRIDS + 4 * index;
    let (g0, g1) = (set_tables::word(x.b, at), set_tables::word(x.b, at + 2));
    let end = load_blocks(x, rom::LOAD_BLOCKS, 0x917000, g0);
    load_blocks(x, rom::LOAD_BLOCKS, end, g1);
}

mod later;
mod status;
