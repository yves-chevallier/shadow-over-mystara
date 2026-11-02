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
mod scenery {
    use super::*;

    /// Variante -> routine d'initialisation (13 routines).
    const INIT_KIND: u32 = 0xdd6a4;
    /// Variante -> routine de suivi (23 routines).
    const STEP_KIND: u32 = 0xdd8ea;
    const WAIT: u8 = 0;
    const FOLLOW: u8 = 2;
    const GONE: u8 = 4;
    const FREE: u8 = 6;
    /// End of the animation script (high byte of the flags).
    const ANIM_END: u8 = 0xfe;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        if suspended(x) {
            return;
        }
        match x.u8(0x4) {
            WAIT => init(x),
            FOLLOW => follow(x),
            GONE => x.add8(0x4, 2),
            FREE => x.free(&pools::SOBJ),
            _ => {}
        }
    }

    fn init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        x.set16(0x14, 0);
        x.set8(0x42, 0);
        let variant = x.u8(0x3) as u32;
        x.anim_start(ANIMS, variant as u16);
        match x.rom8(INIT_KIND + variant) {
            0 => {}
            1 => {
                let v = x.u16(0x8);
                x.set16(0xa, v);
                let v = x.u16(0x10);
                x.set16(0x12, v);
            }
            2 => {
                x.set8(0x42, 1);
                let v = x.a5_u32(ram::CAMERA_Y);
                x.set_a5_u32(0x50e, v);
            }
            3 => {
                x.set8(0x2d, 0);
                x.select_speed(&crate::objects::motion::SPEEDS_B, 0xd);
            }
            4 => {
                let v = x.a5_u16(ram::CAMERA2_X);
                x.set16(0xa, v);
                let v = x.a5_u16(ram::CAMERA2_Y);
                x.set16(0xe, v);
                x.add16(0x12, 0xd8);
                x.add16(0xe, 0x190);
            }
            5 => place_at(x, 0, 0x2c0),
            6 => x.set8(0x1d, 0xe),
            7 => x.set8(0x3f, 2),
            8 => place_at(x, 0, 0x2f0),
            9 => place_at(x, 0x100, 0x2f0),
            10 => place_at(x, 0x100, 0x2d0),
            11 => {
                x.set16(0x14, 0);
                x.set16(0x1c, 0x200);
            }
            _ => x.set8(0x42, 1),
        }
        reinsert_by_layer(x);
    }

    /// Positions the scenery (X, Y) and lifts it (Z = -0x100), in the foreground.
    fn place_at<R: Bench>(x: &mut Cx<'_, R>, px: u16, py: u16) {
        x.set16(0x8, px);
        x.set16(0x10, py);
        x.set16(0xc, 0xff00);
        x.set8(0x42, 1);
    }

    fn follow<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(0x2f) == 0 {
            x.anim_step();
        }
        let variant = x.u8(0x3) as u32;
        let (dx, dy) = camera_delta(x);
        match x.rom8(STEP_KIND + variant) {
            0 => cull_beyond(x, 0x60),
            1 => {
                let d = x.a5_u16(ram::CAMERA2_X).wrapping_sub(x.a5_u16(ram::CAMERA_X));
                let v = x.u16(0xa).wrapping_sub(d);
                x.set16(0x8, v);
                let d = x.a5_u16(ram::CAMERA2_Y).wrapping_sub(x.a5_u16(ram::CAMERA_Y));
                let v = x.u16(0x12).wrapping_sub(d);
                x.set16(0x10, v);
                cull_beyond(x, 0x60);
            }
            2 => {
                let v = x.a5_u16(0x5e6);
                x.set16(0x8, v);
                let v = x.a5_u32(0x5ee).wrapping_add(1);
                x.set32(0x10, v);
                let n = x.u8(0x1d).wrapping_sub(1);
                x.set8(0x1d, n);
                if (n as i8) < 0 {
                    x.set8(0x1d, 0xe);
                    let v = x.u8(0xd) ^ 1;
                    x.set8(0xd, v);
                }
                cull_beyond(x, 0x60);
                reinsert_by_layer(x);
            }
            16 => {
                let v = x.a5_u16(0x5e6);
                x.set16(0x8, v);
                let v = x.a5_u32(0x5ee).wrapping_add(1);
                x.set32(0x10, v);
                cull_beyond(x, 0x60);
                reinsert_by_layer(x);
            }
            3 => {
                x.add32(0x8, dx);
                x.add32(0x10, dy);
                reinsert_by_layer(x);
            }
            7 => {
                let d = x.a5_u32(ram::PREV_CAMERA2_X).wrapping_sub(x.a5_u32(ram::CAMERA2_X));
                x.add32(0x8, 0u32.wrapping_sub(d));
                cull_beyond(x, 0x60);
                reinsert_by_layer(x);
            }
            4 => {
                let d2 = dy.wrapping_sub(asr(dy, 1));
                x.add32(0x8, 0u32.wrapping_sub(dx));
                x.add32(0xc, 0u32.wrapping_sub(d2));
                reinsert_by_layer(x);
            }
            5 => {
                let d0 = dx.wrapping_sub(asr(dx, 3));
                let d1 = asr(dy, 3).wrapping_sub(dy);
                x.add32(0x8, d0);
                x.add32(0xc, 0u32.wrapping_sub(d1));
                reinsert_by_layer(x);
            }
            10 => {
                x.add32(0x8, asr(dx, 2));
                x.add32(0xc, asr(dy, 2));
                reinsert_by_layer(x);
            }
            11 => {
                let d = x.a5_u32(ram::CAMERA2_X).wrapping_sub(x.a5_u32(ram::PREV_CAMERA2_X));
                x.add32(0x8, 0u32.wrapping_sub(d));
                reinsert_by_layer(x);
            }
            6 => reinsert_by_layer(x),
            8 => wait_script_end(x, 2),
            9 => {
                x.move_xy();
                if x.u16(0x10) > 0x240 {
                    x.add8(0x4, 2);
                }
            }
            12..=15 => wait_script_end(x, 4),
            17 => {
                let mut d0 = asr(dx, 1);
                let mut d2 = asr(dy, 1);
                if x.u8(0x3) == 0xa {
                    d0 = d0.wrapping_add(asr(d0, 2));
                    d2 = d2.wrapping_add(asr(d2, 2));
                }
                x.add32(0x8, d0);
                x.add32(0xc, d2);
            }
            18 => {
                x.move_y();
                if x.u8(0x1) & 1 == 0 {
                    x.add8(0x4, 2);
                }
            }
            20 => {
                let lim = x.a5_u16(ram::CAMERA_X).wrapping_sub(0x40) as i16;
                if lim > x.u16(0x8) as i16 {
                    x.add8(0x4, 2);
                }
            }
            21 => {
                if x.a5_u8(0x42) == 0 {
                    x.anim_start(ANIMS, variant as u16);
                } else {
                    wait_script_end(x, 2)
                }
            }
            22 => {
                if x.a5_u8(0x42) != 0 {
                    x.move_xy();
                    if x.u16(0x10) > 0x240 {
                        x.add8(0x4, 2);
                    }
                }
            }
            _ => {}
        }
    }

    /// Goes to the next state (+`step`) when the animation is finished.
    fn wait_script_end<R: Bench>(x: &mut Cx<'_, R>, step: u8) {
        if x.u8(0x32) == ANIM_END {
            x.add8(0x4, step);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 1 (0xddc28): fixed scenery with two behaviors
// ---------------------------------------------------------------------------------------------

/// Animated scenery (animation set 0x1c40c6) that stays in place, moves with the camera at half
/// speed or follows the secondary camera depending on the variant.
mod fixture {
    use super::*;

    const ANIMS: u32 = 0x1c40c6;
    /// Variant -> shape of the behavior at creation (0: none, 1: saves X/Y).
    const INIT_KIND: u32 = 0xddc6e;
    /// Variant -> shape of the per-frame behavior (0: screen exit, 1: scrolling, 2: following).
    const STEP_KIND: u32 = 0xddd16;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                let variant = x.u8(0x3) as u32;
                x.anim_start(ANIMS, variant as u16);
                x.reinsert();
                if x.rom8(INIT_KIND + variant) == 1 {
                    let v = x.u16(0x8);
                    x.set16(0xa, v);
                    let v = x.u16(0x10);
                    x.set16(0x12, v);
                }
            }
            2 => {
                if x.u8(0x2f) == 0 {
                    x.anim_step();
                }
                let variant = x.u8(0x3) as u32;
                match x.rom8(STEP_KIND + variant) {
                    0 => cull_beyond(x, 0x60),
                    1 => {
                        let (dx, dy) = camera_delta(x);
                        x.add32(0x8, asr(dx, 1));
                        x.add32(0xc, asr(dy, 1));
                    }
                    _ => {
                        let d = x.a5_u16(ram::CAMERA2_X).wrapping_sub(x.a5_u16(ram::CAMERA_X));
                        let v = x.u16(0xa).wrapping_sub(d);
                        x.set16(0x8, v);
                        let d = x.a5_u16(ram::CAMERA2_Y).wrapping_sub(x.a5_u16(ram::CAMERA_Y));
                        let v = x.u16(0x12).wrapping_sub(d);
                        x.set16(0x10, v);
                        cull_beyond(x, 0x60);
                    }
                }
            }
            _ => x.free(&pools::SOBJ),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 2 (0xdddb2): blinking scenery
// ---------------------------------------------------------------------------------------------

/// Animated scenery that disappears 16 times in a row at each end of script (`+0x14` counts the
/// blinks); its screen margin is the D2 left by the caller.
mod flasher {
    use super::*;

    const BLINKS: u8 = 0x10;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                restart(x);
            }
            2 => {
                if x.anim_step() {
                    x.set_bit(0x1, 1, true);
                    let n = x.u8(0x14).wrapping_sub(1);
                    x.set8(0x14, n);
                    if (n as i8) < 0 {
                        x.set_bit(0x1, 1, false);
                        restart(x);
                    }
                }
            }
            _ => return x.free(&pools::SOBJ),
        }
        let margin = *x.c.d2() as u16;
        cull_beyond(x, margin);
        x.reinsert();
    }

    fn restart<R: Bench>(x: &mut Cx<'_, R>) {
        x.set8(0x14, BLINKS);
        let variant = x.u8(0x3) as u32;
        x.anim_start(ANIMS, variant as u16);
    }
}

// ---------------------------------------------------------------------------------------------
// Type 3 (0xdde18): label stuck to an object
// ---------------------------------------------------------------------------------------------

/// Label that follows an object (long address in `+0x14`) with a constant offset (X + 0x50,
/// Y - 0x58) as long as it is not in state 6.
mod tag {
    use super::*;

    const ANIM: u32 = 0x43;
    const OFFSET_X: u16 = 0x50;
    const OFFSET_Y: u16 = 0x58;
    const HOST: u32 = 0x14;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set16(0x18, OFFSET_X);
                x.set16(0x1a, OFFSET_Y);
                x.anim_start(ANIMS, ANIM as u16);
                x.reinsert();
            }
            2 => {
                let host = Obj(x.u32(HOST) & 0xff_ffff);
                if host.u8(x.b, 0x4) == 6 {
                    x.add8(0x4, 2);
                    return;
                }
                let px = host.u16(x.b, 0x8).wrapping_add(x.u16(0x18));
                let py = host.u16(x.b, 0x10).wrapping_sub(x.u16(0x1a));
                x.set16(0x8, px);
                x.set16(0x10, py);
                x.anim_step();
            }
            _ => x.free(&pools::SOBJ),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 5 (0xdde8c): ghost object
// ---------------------------------------------------------------------------------------------

/// Object with no behavior: it releases itself on its first frame.
mod ghost {
    use super::*;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(0x4) == 0 {
            x.add8(0x4, 2);
        }
        x.free(&pools::SOBJ);
    }
}

// ---------------------------------------------------------------------------------------------
// Type 6 (0xddec4): elements of the scripted scenes
// ---------------------------------------------------------------------------------------------

/// Scene elements (presentation, interludes, ending): 24 variants (`+3`), each a small
/// state machine. The variants are linked to a "master" object (`+0x4c`, often another
/// object of this type) whose byte `+8` serves as a mailbox: 1 "go", 2 "next step",
/// negative "end". Many also load palettes from the ROM (0x28f340...) into the
/// page palette 0x91c080-0x91c3ff and launch sounds (0xea2e8...).
mod scene {
    use super::*;

    /// State 4 ("dead"): the object is marked for deletion then released right away.
    const DEAD: u32 = 0x0600_0000;
    /// Palettes of the elements page (GFX RAM): 16 colors.
    const PALETTE_A: u32 = 0x91c080;
    const PALETTE_B: u32 = 0x91c3e0;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x3) {
            0 => lifecycle(x, v0_init, v0_step),
            1 => lifecycle(x, v1_init, v1_step),
            2 => lifecycle(x, v2_init, v2_step),
            3 => match x.u8(0x4) {
                0 => v3_init(x),
                2 => v3_step(x),
                4 => {
                    if signal(x) != 0 {
                        return kill(x);
                    }
                    x.reinsert();
                }
                _ => {}
            },
            4 => lifecycle(x, v4_init, v4_step),
            5 => lifecycle(x, v5_init, v5_step),
            6 => lifecycle(x, v6_init, v6_step),
            7 => lifecycle(x, v7_init, v7_step),
            8 => lifecycle(x, v8_init, v8_step),
            9 => lifecycle(x, v9_init, v9_step),
            10 => lifecycle(x, v10_init, v10_step),
            11 => lifecycle(x, v11_init, |_| {}),
            12 => lifecycle(x, v12_init, v12_step),
            13 => lifecycle(x, v13_init, |_| {}),
            14 => lifecycle(x, v14_init, v14_step),
            15 => lifecycle(x, v15_init, |_| {}),
            16 => lifecycle(x, v16_init, v16_step),
            17 => lifecycle(x, v17_init, v17_step),
            18 => lifecycle(x, v18_init, |_| {}),
            19 => lifecycle(x, v19_init, v19_step),
            20 => lifecycle(x, v20_init, v20_step),
            21 => lifecycle(x, v21_init, |_| {}),
            22 => lifecycle(x, v20_init, v20_fall),
            23 => lifecycle(x, v23_init, v23_follow_and_insert),
            _ => {}
        }
    }

    /// Common life cycle: 0 initialization, 2 alive, 4 dead, 6 release.
    fn lifecycle<R: Bench>(x: &mut Cx<'_, R>, init: fn(&mut Cx<'_, R>), step: fn(&mut Cx<'_, R>)) {
        match x.u8(0x4) {
            0 => init(x),
            2 => step(x),
            4 => kill(x),
            6 => x.free(&pools::SOBJ),
            _ => {}
        }
    }

    fn kill<R: Bench>(x: &mut Cx<'_, R>) {
        x.set32(0x4, DEAD);
        x.free(&pools::SOBJ);
    }

    fn rom32<R: Bench>(x: &mut Cx<'_, R>, addr: u32) -> u32 {
        (x.rom16(addr) as u32) << 16 | x.rom16(addr + 2) as u32
    }

    /// The scene's master (`+0x4c`).
    fn master<R: Bench>(x: &mut Cx<'_, R>) -> Obj {
        Obj(x.u32(0x4c) & 0xff_ffff)
    }

    /// Master's signal byte (`+8`).
    fn signal<R: Bench>(x: &mut Cx<'_, R>) -> u8 {
        let m = master(x);
        m.u8(x.b, 0x8)
    }

    fn send_signal<R: Bench>(x: &mut Cx<'_, R>, v: u8) {
        let m = master(x);
        m.set_u8(x.b, 0x8, v);
    }

    /// Starts script `n` and puts the object back in the actors' list.
    fn start<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
        x.anim_start(ANIMS, n as u16);
        x.reinsert();
    }

    /// Creates a SOBJ object of type 6 (routine 0xddeaa), of variant `variant`; nothing on failure.
    fn spawn<R: Bench>(x: &mut Cx<'_, R>, variant: u8) -> Option<Obj> {
        let Some(child) = x.alloc(&pools::SOBJ) else { return None };
        child.set_u8(x.b, 0, 1);
        child.set_u8(x.b, 2, 6);
        child.set_u8(x.b, 3, variant);
        Some(child)
    }

    /// Scrolls the screen by one pixel every 8 frames (routine 0xdea4a) while keeping the object
    /// still on screen.
    fn scroll_creep<R: Bench>(x: &mut Cx<'_, R>) {
        if x.a5_u8(0x6f) & 7 != 0 {
            return;
        }
        let v = x.a5_u16(ram::CAMERA_X).wrapping_sub(1);
        x.set_a5_u16(ram::CAMERA_X, v);
        x.set_a5_u16(0x50e, v);
        let v = x.a5_u16(ram::CAMERA2_X).wrapping_add(1);
        x.set_a5_u16(ram::CAMERA2_X, v);
        x.set_a5_u16(0x55e, v);
        let (dx, dy) = camera_delta(x);
        x.add32(0x8, dx);
        x.add32(0x10, dy);
    }

    /// Variant 0: sprite tied to the master (animation set, flip indicator and number read from
    /// a table with 8 bytes per value of `+0x2f`), random flip if the indicator is negative.
    const V0_ENTRIES: u32 = 0xddf6c;

    fn random_flip<R: Bench>(x: &mut Cx<'_, R>) {
        x.set8(0x2c, 0);
        let rng = x.random();
        if rng as u32 & 1 != 0 {
            x.set8(0x2c, 0x20);
        }
    }

    fn v0_init<R: Bench>(x: &mut Cx<'_, R>) {
        if (signal(x) as i8) < 0 {
            return x.free(&pools::SOBJ);
        }
        x.add8(0x4, 2);
        let e = V0_ENTRIES + 8 * x.u8(0x2f) as u32;
        let set = rom32(x, e);
        let flip = x.rom8(e + 4);
        x.set8(0x2c, flip);
        let n = x.rom16(e + 6) as u32;
        x.anim_start(set, n as u16);
        if (x.u8(0x2c) as i8) < 0 {
            random_flip(x);
        }
        x.reinsert();
    }

    fn v0_step<R: Bench>(x: &mut Cx<'_, R>) {
        x.anim_step();
        if signal(x) as i8 >= 0 {
            return;
        }
        x.add8(0x4, 2);
    }

    /// Variant 1: light that slowly grows until the master's signal.
    fn v1_init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        x.set32(0x8, 0x30_0000);
        x.set32(0xc, 0xd0_0000);
        x.set32(0x10, 0);
        start(x, 0xee);
        crate::sound::play(x.b, 0xea662, x.o);
    }

    fn v1_step<R: Bench>(x: &mut Cx<'_, R>) {
        if signal(x) != 0 {
            return kill(x);
        }
        x.add32(0x8, 0x8000);
        x.add32(0xc, 0x8000);
    }

    /// Variant 2: scrolling scenery, three phases (`+5`) paced by the master's signal.
    fn v2_init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        x.set16(0x8, 0x100);
        x.set16(0xc, 0xc0);
        x.set32(0x10, 0);
        scroll_creep(x);
        start(x, 0xef);
    }

    fn v2_step<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x5) {
            0 => {
                if signal(x) != 0 {
                    x.add8(0x5, 2);
                }
            }
            2 => {
                scroll_creep(x);
                if signal(x) == 0 {
                    x.add8(0x5, 2);
                }
            }
            4 => {
                if signal(x) != 0 {
                    kill(x);
                }
            }
            _ => {}
        }
    }

    /// Variant 3: launches decorative SET objects from a byte list (0xde1e0, or
    /// 0xde1e8 if `A5+0x38`), one every 30 frames, until the end of its script.
    const V3_LISTS: u32 = 0xde1e0;

    fn v3_init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        x.set16(0x8, 0x60);
        x.set16(0xc, 0xc0);
        x.set32(0x10, 0);
        x.set16(0x42, 0x1e);
        x.set32(0x44, V3_LISTS);
        if x.a5_u16(0x38) != 0 {
            x.add32(0x44, 8);
        }
        x.set16(0x48, 0);
        start(x, 0xf1);
        crate::sound::play(x.b, 0xea6a2, x.o);
    }

    fn v3_step<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u16(0x48) != 0 {
            let child = x.linked(0x48);
            if child.u8(x.b, 0x22) == 0 {
                return;
            }
            x.set16(0x48, 0);
        }
        let n = x.u16(0x42).wrapping_sub(1);
        x.set16(0x42, n);
        if n != 0 {
            return;
        }
        let mut list = x.u32(0x44);
        let kind = x.data8(list);
        list += 1;
        if kind != 0 {
            if let Some(set) = crate::families::creators::scene_set(x.b, kind) {
                x.set16(0x48, set.word());
            }
        }
        x.set32(0x44, list);
        x.anim_step();
        x.set16(0x42, 0x1e);
        if x.u16(0x32) == 0 {
            return;
        }
        x.set16(0x42, 0xa);
        x.add8(0x4, 2);
        send_signal(x, 1);
    }

    /// Variant 4: rises toward the top of the screen (Z - 1.25 per frame) and stops at Z = 0x20.
    fn v4_init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        x.set32(0x10, 0x4000);
        x.set16(0x8, 5);
        x.set16(0xc, 0x98);
        start(x, 0xf5);
    }

    fn v4_step<R: Bench>(x: &mut Cx<'_, R>) {
        x.add32(0xc, 0u32.wrapping_sub(0x14000));
        if x.u16(0xc) as i16 > 0x20 {
            return;
        }
        x.add8(0x4, 2);
        x.set8(0x5, 0);
    }

    /// Variant 5: leader that descends, triggers the scrolling then creates variants 0 (two at
    /// most; X then Z offsets in table 0xde386).
    const V5_OFFSETS: u32 = 0xde386;

    fn v5_init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        x.set32(0x10, 0x8000);
        x.set16(0x8, 0x78);
        x.set16(0xc, 0x90);
        start(x, 0xf6);
    }

    fn v5_step<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x5) {
            0 => {
                x.add32(0xc, 0x10000);
                if x.u16(0xc) < 0xd0 {
                    return;
                }
                x.set16(0xc, 0xd0);
                x.add8(0x5, 2);
            }
            2 => {
                let y = x.a5_u32(ram::CAMERA2_Y).wrapping_add(0x10000);
                x.set_a5_u32(ram::CAMERA2_Y, y);
                x.set_a5_u16(0x562, (y >> 16) as u16);
                if ((y >> 16) as u16) < 0x28 {
                    return;
                }
                x.add8(0x5, 2);
                send_signal(x, 1);
                x.set16(0x42, 1);
            }
            4 => {
                if signal(x) as i8 >= 0 {
                    return v5_spawn(x);
                }
                x.add8(0x4, 2);
                x.set8(0x5, 0);
            }
            _ => {}
        }
    }

    fn v5_spawn<R: Bench>(x: &mut Cx<'_, R>) {
        let n = x.u8(0x43).wrapping_sub(1);
        x.set8(0x43, n);
        if n != 0 || x.u8(0x42) == 2 {
            return;
        }
        x.set8(0x43, 0x30);
        let Some(child) = spawn(x, 0) else { return };
        let which = x.u8(0x42);
        child.set_u8(x.b, 0x2f, which);
        for off in [0x4c, 0x8, 0xc, 0x10] {
            let v = x.u32(off);
            child.set_u32(x.b, off, v);
        }
        let at = V5_OFFSETS + 4 * which as u32;
        let dx = x.rom16(at);
        let dz = x.rom16(at + 2);
        let v = child.u16(x.b, 0x8).wrapping_add(dx);
        child.set_u16(x.b, 0x8, v);
        let v = child.u16(x.b, 0xc).wrapping_add(dz);
        child.set_u16(x.b, 0xc, v);
        let v = child.u32(x.b, 0x10).wrapping_sub(1);
        child.set_u32(x.b, 0x10, v);
        x.add8(0x42, 1);
    }

    /// Variant 6: letter that falls from the top (Z 0x300) down to its stop height (table 0xde3ee:
    /// X, stop), waits for signal 2 then rises by 0x290.
    const V6_TABLE: u32 = 0xde3ee;

    fn v6_init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        x.set32(0x10, 0xf000);
        x.set32(0xc, 0x300_0000);
        let v = x.u8(0x2f) as u32;
        let shift = (v << 8) & 0xffff;
        let y = x.u32(0x10).wrapping_sub(shift);
        x.set32(0x10, y);
        let px = x.rom16(V6_TABLE + 4 * v);
        x.set16(0x8, px);
        let stop = x.rom16(V6_TABLE + 4 * v + 2);
        x.set16(0x14, stop);
        x.anim_start(ANIMS, (v + 0xfe) as u16);
        x.reinsert();
    }

    fn v6_step<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x5) {
            0 => {
                x.add16(0xc, 2);
                let stop = x.u16(0x14);
                if stop <= x.u16(0xc) {
                    x.set16(0xc, stop);
                    x.add8(0x5, 2);
                    let m = master(x);
                    let v = m.u8(x.b, 0xd).wrapping_sub(1);
                    m.set_u8(x.b, 0xd, v);
                }
            }
            2 => {
                if signal(x) == 2 {
                    x.add16(0xc, 0u16.wrapping_sub(0x290));
                    x.add8(0x5, 2);
                }
            }
            4 => {
                if (signal(x) as i8) < 0 {
                    x.add8(0x4, 2);
                }
            }
            _ => {}
        }
        x.reinsert();
    }

    /// Variant 7: palette fade (page B) then a 180-frame wait; also loads a transition
    /// script (`A5+0x5c6`...). Script chosen by `+0x2f` and by `A5+0x72`.
    const V7_SCRIPTS: u32 = 0xde4d2;
    const V7_PALETTES: u32 = 0x292240;

    fn v7_init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        x.set16(0x42, 6);
        let mut off = 4 * x.u8(0x2f) as u32;
        if x.a5_u16(0x72) != 2 {
            off += 8;
        }
        let script = rom32(x, V7_SCRIPTS + off);
        let w = x.data16(script);
        x.set_a5_u16(0x5ca, w);
        let w = x.data16(script + 2);
        x.set_a5_u16(0x5cc, w);
        x.set_a5_u32(0x5c6, script + 4);
        copy_longs(x, V7_PALETTES, PALETTE_B, 8);
        x.set_a5_u8(0x48e, 1);
        crate::palette::no_brightness(x.b);
    }

    fn v7_step<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(0x5) == 0 {
            if rd8(x.b, PALETTE_B) & 0xf0 == 0xf0 {
                x.set16(0x42, 0xb4);
                x.add8(0x5, 2);
                return;
            }
            let n = x.u16(0x42).wrapping_sub(1);
            x.set16(0x42, n);
            if n != 0 {
                return;
            }
            x.set16(0x42, 6);
            for i in 0..8 {
                let v = rd32(x.b, PALETTE_B + 4 * i).wrapping_add(0x1000_1000);
                wr32(x.b, PALETTE_B + 4 * i, v);
            }
        } else {
            let n = x.u16(0x42).wrapping_sub(1);
            x.set16(0x42, n);
            if n != 0 {
                return;
            }
            x.add8(0x4, 2);
            let m = master(x);
            let v = m.u8(x.b, 0xc).wrapping_sub(1);
            m.set_u8(x.b, 0xc, v);
            x.set_a5_u32(0x5c6, 0);
        }
    }

    /// Variant 8: bar that descends (Z + 8 per frame) to the bottom of the screen, then raises the byte
    /// 0xff0eaa (A5-0x7156).
    const SCENE_DONE_FLAG: u32 = 0xff_0eaa;

    fn v8_init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        let v = x.a5_u16(ram::CAMERA_X);
        x.set16(0x8, v);
        let v = x.a5_u16(ram::CAMERA_Y);
        x.set16(0xc, v);
        x.add16(0xc, 0x20);
        x.set32(0x10, 0);
        x.add16(0x10, 0u16.wrapping_sub(0x20));
        start(x, 0x112);
    }

    fn v8_step<R: Bench>(x: &mut Cx<'_, R>) {
        x.add16(0xc, 8);
        let below = x.u16(0xc).wrapping_add(x.u16(0x10)).wrapping_sub(x.a5_u16(ram::CAMERA_Y));
        if below < 0x1c8 {
            return;
        }
        let y = x.a5_u16(ram::CAMERA_Y).wrapping_add(0x1c8);
        x.set16(0xc, y);
        crate::mem::wr8(x.b, SCENE_DONE_FLAG, 0xff);
        x.add8(0x4, 2);
        x.reinsert();
    }

    /// Variant 9: animated text on the secondary camera; signals its end to the master.
    fn v9_init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        let v = x.a5_u16(ram::CAMERA2_X);
        x.set16(0x8, v);
        let v = x.a5_u16(ram::CAMERA2_Y);
        x.set16(0xc, v);
        x.set32(0x10, 0);
        x.add16(0xc, 0xf0);
        start(x, 0x113);
        crate::sound::play(x.b, 0xea2e8, x.o);
    }

    fn v9_step<R: Bench>(x: &mut Cx<'_, R>) {
        if x.anim_step() || (signal(x) as i8) < 0 {
            return x.set8(0x4, 6);
        }
        if x.u16(0x30) != 0x7fff {
            return;
        }
        send_signal(x, 1);
        x.set16(0x30, 6);
    }

    /// Variant 10: plate that descends in two phases (Z 0x88 then 0xa0), waits for signal 2,
    /// rises back to Z 0x80 and waits for the end.
    fn v10_init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        x.set32(0x8, 0x98_0000);
        x.set32(0xc, 0x70_0000);
        x.set32(0x10, 0);
        start(x, 0x114);
        crate::sound::play(x.b, 0xea77e, x.o);
    }

    fn v10_step<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x5) {
            0 => {
                x.add32(0xc, 0x4000);
                if x.u16(0xc) < 0x88 {
                    return;
                }
                x.set16(0xc, 0x88);
                send_signal(x, 1);
                x.add8(0x5, 2);
            }
            2 => {
                x.add32(0xc, 0x4000);
                if x.u16(0xc) < 0xa0 {
                    return;
                }
                x.set16(0xc, 0xa0);
                x.add8(0x5, 2);
            }
            4 => {
                if signal(x) != 2 {
                    return;
                }
                x.add8(0x5, 2);
                x.set8(0x42, 0x1e);
            }
            6 => {
                let n = x.u8(0x42).wrapping_sub(1);
                x.set8(0x42, n);
                if n != 0 {
                    return;
                }
                x.add8(0x5, 2);
                crate::sound::play(x.b, 0xea87e, x.o);
            }
            8 => {
                x.add32(0xc, 0u32.wrapping_sub(0x4000));
                if x.u16(0xc) as i16 > 0x80 {
                    return;
                }
                x.set16(0xc, 0x80);
                send_signal(x, 1);
                x.add8(0x5, 2);
            }
            10 => {
                if (signal(x) as i8) < 0 {
                    x.add8(0x4, 2);
                }
            }
            _ => {}
        }
    }

    /// Variant 11: fixed sprite on the main camera.
    fn v11_init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        x.set32(0x10, 0x8000);
        let y = x.a5_u16(ram::CAMERA_Y);
        x.set16(0xc, y);
        x.add16(0xc, 0xc0);
        x.set16(0x8, 0x30);
        start(x, 0x115);
        crate::sound::play(x.b, 0xea7fe, x.o);
    }

    /// Variant 12: sprite that crosses the screen (1 pixel per frame to the right, or to the
    /// left if `+0x2f`) and moves the camera back.
    fn v12_init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        x.set8(0x2c, 0);
        x.set32(0x8, 0xf0_0000);
        let y = x.a5_u16(ram::CAMERA2_Y);
        x.set16(0xc, y);
        x.set16(0x14, 0x100);
        x.add16(0xc, 0xc0);
        x.set32(0x10, 0);
        if x.u8(0x2f) != 0 {
            x.set8(0x2c, 0x20);
            let v = x.u16(0x14).wrapping_neg();
            x.set16(0x14, v);
            x.add32(0x8, 0u32.wrapping_sub(0x60_0000));
        }
        x.anim_start(ANIMS, 0x116);
        x.reinsert();
    }

    fn v12_step<R: Bench>(x: &mut Cx<'_, R>) {
        let d = (((x.u16(0x14) as u32) << 16) as i32 >> 8) as u32;
        x.add32(0x8, d);
        let screen = x.u16(0x8).wrapping_sub(x.a5_u16(ram::CAMERA_X));
        if screen >= 0x180 {
            x.set8(0x4, 4);
        }
        if x.u8(0x2f) != 0 {
            return;
        }
        let cam = x.a5_u16(ram::CAMERA_X);
        if cam != 0 {
            x.set_a5_u16(ram::CAMERA_X, cam - 1);
        }
    }

    /// Variant 13: frame whose script is a direct entry (16 bytes per `+0x2f` starting from
    /// 0xdea84), on a palette background 0x91c340 (8 identical colors).
    const V13_SCRIPTS: u32 = 0xdea84;

    fn v13_init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        let v = x.a5_u16(ram::CAMERA_X);
        x.set16(0x8, v);
        let v = x.a5_u16(ram::CAMERA_Y);
        x.set16(0xc, v);
        x.set32(0x10, 0);
        for i in 0..8 {
            wr32(x.b, 0x91c340 + 4 * i, 0xf111_f111);
        }
        x.add16(0x8, 0x10);
        x.add16(0xc, 0x50);
        let v = x.u8(0x2f) as u16;
        x.add16(0xc, v << 3);
        x.add16(0xc, v << 4);
        x.anim_start_entry(V13_SCRIPTS + 16 * v as u32);
        x.reinsert();
    }

    /// Variant 14: badge whose sprite and position depend on `+0x46` (number, or object
    /// whose variant is taken if bit 15 is raised); 8-byte table at 0xdef1e.
    const V14_TABLE: u32 = 0xdef1e;

    fn v14_init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        let mut which = x.u16(0x46);
        if (which as i16) < 0 {
            let owner = x.linked(0x46);
            which = owner.u8(x.b, 0x3) as u16;
        }
        x.set32(0xc, 0);
        let row = V14_TABLE.wrapping_add((which << 3) as i16 as i32 as u32);
        let v = x.rom16(row);
        x.set16(0x8, v);
        let v = x.rom16(row + 2);
        x.set16(0x10, v);
        let v = x.rom8(row + 6);
        x.set8(0x2c, v);
        x.anim_start(ANIMS, (0x190 + which as u32) as u16);
        x.reinsert();
    }

    fn v14_step<R: Bench>(x: &mut Cx<'_, R>) {
        if (signal(x) as i8) < 0 {
            x.add8(0x4, 2);
        }
    }

    /// Variant 15: colored pennant (palette 0x28f960, one 32-byte row per number drawn from
    /// table 0xdefa4), position in table 0xdf01a.
    const V15_PALETTE_SELECT: u32 = 0xdefa4;
    const V15_PALETTES: u32 = 0x28f960;
    const V15_POSITIONS: u32 = 0xdf01a;

    fn v15_init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        x.set8(0x5, 0);
        x.set32(0x10, 0);
        let v = x.u8(0x2f) as u32;
        let line = x.rom8(V15_PALETTE_SELECT + v) as u32;
        copy_longs(x, V15_PALETTES + (line << 5), PALETTE_A, 8);
        or_longs(x, PALETTE_A, BRIGHT, 8);
        let px = x.rom16(V15_POSITIONS + 4 * v);
        x.set16(0x8, px);
        let py = x.rom16(V15_POSITIONS + 4 * v + 2);
        x.set16(0xc, py);
        start(x, 0x19c + v);
    }

    /// Variant 16: randomly flipped sprite, which lives as long as the master has not finished and its
    /// animation is not complete.
    fn v16_init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        random_flip(x);
        start(x, 0x1a2);
        crate::sound::play(x.b, 0xea7be, x.o);
    }

    fn v16_step<R: Bench>(x: &mut Cx<'_, R>) {
        if signal(x) as i8 >= 0 && !x.anim_step() {
            return;
        }
        x.add8(0x4, 2);
    }

    /// Variant 17: bubble that comes out of the bottom of the screen (two positions according to `+0x2f`); at the end
    /// of the animation, gives bit (`+0x2f` modulo 8) of `+0x30` back to the owner `+0x46`.
    fn v17_init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        x.set32(0x10, 0x4000);
        x.set16(0xc, 0x1c0);
        x.set32(0x8, 0);
        if x.u8(0x2f) == 0 {
            x.set16(0x8, 0x20);
            x.set32(0x10, 0x2000);
        }
        // `move.w #0x1a3,D0w ; add.b (0x2f,A0),D0b`: the addition does not overflow into the high byte
        let n = 0x100 | (0xa3 + x.u8(0x2f) as u32) & 0xff;
        start(x, n);
        crate::sound::play(x.b, 0xea83e, x.o);
    }

    fn v17_step<R: Bench>(x: &mut Cx<'_, R>) {
        if !x.anim_step() {
            return;
        }
        let owner = x.linked(0x46);
        let bit = x.u8(0x2f) & 7;
        let v = owner.u8(x.b, 0x30) & !(1 << bit);
        owner.set_u8(x.b, 0x30, v);
        x.add8(0x4, 2);
    }

    /// Variant 18: sprite placed on the secondary camera.
    fn v18_init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        let v = x.a5_u16(ram::CAMERA2_X);
        x.set16(0x8, v);
        let v = x.a5_u16(ram::CAMERA2_Y);
        x.set16(0xc, v);
        x.set32(0x10, 0);
        let dz = if x.u8(0x2f) != 0 { 0x50 } else { 0x100 };
        x.add16(0xc, dz);
        x.anim_start(ANIMS, 0xec);
        x.reinsert();
    }

    /// Variant 19: palette dot (0x28f340 + 0x60 + 32 per row, row from table
    /// 0xdf182) placed on the camera; it disappears when the animation of its carrier (`+0x46`)
    /// is finished.
    const V19_LINES: u32 = 0xdf182;
    const V19_PALETTES: u32 = 0x28f340 + 0x60;
    const V19_POSITIONS: u32 = 0xdf234;

    fn v19_init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        let v = x.u8(0x2f) as u32;
        let line = x.rom8(V19_LINES + 2 * v) as u32;
        let flip = x.rom8(V19_LINES + 2 * v + 1);
        x.set8(0x2c, flip);
        copy_longs(x, V19_PALETTES + (line << 5), PALETTE_B, 8);
        or_longs(x, PALETTE_B, BRIGHT, 8);
        let anim = if line == 3 { 0 } else { line };
        start(x, 0x1a8 + anim);
        v19_place(x);
    }

    /// Places the dot from the table, relative to the current camera.
    fn v19_place<R: Bench>(x: &mut Cx<'_, R>) {
        let v = x.u8(0x2f) as u32;
        let px = x.rom16(V19_POSITIONS + 4 * v);
        x.set16(0x8, px);
        let py = x.rom16(V19_POSITIONS + 4 * v + 2);
        x.set16(0xc, py);
        x.set32(0x10, 0);
        let (cx, cy) = if x.a5_u8(ram::USE_CAMERA2) != 0 {
            (x.a5_u16(ram::CAMERA2_X), x.a5_u16(ram::CAMERA2_Y))
        } else {
            (x.a5_u16(ram::CAMERA_X), x.a5_u16(ram::CAMERA_Y))
        };
        x.add16(0x8, cx);
        x.add16(0xc, cy);
    }

    fn v19_step<R: Bench>(x: &mut Cx<'_, R>) {
        v19_place(x);
        let owner = x.linked(0x46);
        if owner.u16(x.b, 0x32) != 0 {
            return;
        }
        x.add8(0x4, 2);
        kill(x);
    }

    /// Variants 20 and 22: debris falling from a randomly drawn point below the camera's bottom edge (8 abscissas
    /// in table 0xdf2a8, 4 acceleration/animation pairs at 0xdf2ea).
    const V20_X: u32 = 0xdf2a8;
    const V20_FALL: u32 = 0xdf2ea;

    fn v20_init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        let v = x.a5_u16(ram::CAMERA_Y);
        x.set16(0xc, v);
        let v = x.a5_u16(ram::CAMERA_X);
        x.set16(0x8, v);
        x.add16(0xc, 0x110);
        x.set16(0x10, 0x10);
        let rng = x.random();
        let k = rng as u32 & 7;
        let dx = x.rom16(V20_X + 2 * k);
        x.add16(0x8, dx);
        x.set32(0x14, 0);
        x.set16(0x18, 0);
        let rng = x.random();
        let k = rng as u32 & 3;
        let az = x.rom16(V20_FALL + 4 * k);
        x.set16(0x1a, az);
        let anim = x.rom16(V20_FALL + 4 * k + 2) as u32;
        x.anim_start(ANIMS, anim as u16);
        x.reinsert();
    }

    fn v20_step<R: Bench>(x: &mut Cx<'_, R>) {
        if (signal(x) as i8) < 0 {
            return kill(x);
        }
        v20_fall(x);
    }

    /// Fall: leaves (dead state) when the bottom of the sprite goes back above the top edge of the screen.
    fn v20_fall<R: Bench>(x: &mut Cx<'_, R>) {
        x.move_xz();
        let below = x.u16(0xc).wrapping_add(x.u16(0x10));
        if (below as i16) <= x.a5_u16(ram::CAMERA_Y) as i16 {
            kill(x);
        }
    }

    /// Variant 21: like 15, with the palette row designated by `+0x2f / 2` (table 0xdf332),
    /// without brightening; positions in table 0xdf396.
    const V21_PALETTE_SELECT: u32 = 0xdf332;
    const V21_POSITIONS: u32 = 0xdf396;

    fn v21_init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        x.set8(0x5, 0);
        x.set32(0x10, 0);
        let v = x.u8(0x2f) as u32;
        let line = x.rom8(V21_PALETTE_SELECT + (v >> 1)) as u32;
        copy_longs(x, V15_PALETTES + (line << 5), PALETTE_A, 8);
        let px = x.rom16(V21_POSITIONS + 4 * v);
        x.set16(0x8, px);
        let py = x.rom16(V21_POSITIONS + 4 * v + 2);
        x.set16(0xc, py);
        start(x, 0x19c + (v >> 1));
    }

    /// Variant 23: spark stuck to the corner of the camera structure (A5+0x4fe, or 0x54e if
    /// `A5+0x36`), whose color of palette 0x91c180 takes the brightness of color 0.
    const V23_PALETTE: u32 = 0x28f4c0;

    fn v23_init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        copy_longs(x, V23_PALETTE, 0x91c180, 8);
        let top = rd32(x.b, 0x91c000) & BRIGHT;
        for i in 0..8 {
            let at = 0x91c180 + 4 * i;
            let v = rd32(x.b, at) & 0x0fff_0fff | top;
            wr32(x.b, at, v);
        }
        x.anim_start(ANIMS, 0x170);
        v23_follow(x);
        x.reinsert();
    }

    fn v23_follow_and_insert<R: Bench>(x: &mut Cx<'_, R>) {
        v23_follow(x);
        x.reinsert();
    }

    fn v23_follow<R: Bench>(x: &mut Cx<'_, R>) {
        let camera = if x.a5_u8(ram::USE_CAMERA2) != 0 { 0x54e } else { 0x4fe };
        x.set16(0x42, (0xff_8000u32 + camera) as u16);
        let cam = x.linked(0x42);
        let v = cam.u16(x.b, 0x8);
        x.set16(0x8, v);
        let v = cam.u16(x.b, 0xc);
        x.set16(0xc, v);
        x.add16(0xc, 0x10);
        x.set16(0x10, 0u16.wrapping_sub(0x10));
        x.set8(0x2c, 0);
        x.add16(0xc, 0x40);
        x.add16(0x8, 0x100);
    }
}

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
mod hint {
    use super::*;

    const PALETTE: u32 = 0x28ff20;
    const PALETTE_DST: u32 = 0x91c380;
    const FIRST_ADVICE: u32 = 0x118;
    const LAST_ADVICE: u16 = 0x136;
    const ADVICE_FRAMES: u8 = 0x50;
    const LEAVE_SOUND: u32 = 0xea49a;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        let owner = x.linked(0x46);
        if owner.u8(x.b, 0x43) == 0 && rd8(x.b, HINT_INHIBIT) != 0 {
            x.set32(0x4, 0);
            return;
        }
        match x.u8(0x4) {
            0 => start(x),
            2 => step(x),
            4 | 6 => leave(x),
            _ => {}
        }
    }

    fn start<R: Bench>(x: &mut Cx<'_, R>) {
        if x.a5_u8(GAME_PAUSED) != 0 {
            return;
        }
        x.add8(0x4, 2);
        load_palette(x);
        x.set8(0x16, ADVICE_FRAMES);
        x.set16(0x14, FIRST_ADVICE as u16);
        x.anim_start(ANIMS, FIRST_ADVICE as u16);
    }

    /// Bubble palette, maximum brightness; the object becomes visible (`+0x2a`, `+0x2b`).
    fn load_palette<R: Bench>(x: &mut Cx<'_, R>) {
        copy_longs(x, PALETTE, PALETTE_DST, 8);
        or_longs(x, PALETTE_DST, BRIGHT, 8);
        x.set8(0x2a, 1);
        x.set8(0x2b, 0x1c);
    }

    fn step<R: Bench>(x: &mut Cx<'_, R>) {
        if x.a5_u8(GAME_PAUSED) != 0 {
            return;
        }
        if x.u8(0x40) != 0 {
            load_palette(x);
        }
        x.set8(0x40, 0);
        if x.u16(0x46) == 0 {
            return x.set16(0x4, LEAVE_6_2);
        }
        let owner = x.linked(0x46);
        if owner.u8(x.b, 0x42) & 2 != 0 {
            crate::sound::play(x.b, LEAVE_SOUND, x.o);
            // 0xdf896: SOBJ 0xa linked through `+0x46`
            crate::families::small::linked_via(&mut Pure, x.b, x.o, &pools::SOBJ, 0xa, 0x46);
            let owner = x.linked(0x46);
            let v = owner.u8(x.b, 0x42) & !2;
            owner.set_u8(x.b, 0x42, v);
            return x.set16(0x4, LEAVE_6_2);
        }
        match x.u8(0x5) {
            0 => countdown(x),
            2 => next_advice(x),
            _ => {}
        }
        x.reinsert();
    }

    /// Substate 0: the advice stays displayed for `+0x16` frames, then moves on to the next (substate 2)
    /// or, after the last, cedes control to the linked object.
    fn countdown<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u16(0x46) == 0 {
            return x.set16(0x4, LEAVE_6_2);
        }
        let n = x.u8(0x16).wrapping_sub(1);
        x.set8(0x16, n);
        if (n as i8) < 0 {
            if x.u16(0x14) != LAST_ADVICE {
                x.add8(0x5, 2);
            } else {
                // the last piece of advice stays as long as the game is in state 0xc or 6
                let mode = x.a5_u8(0x34);
                if mode == 0xc || mode == 6 {
                    return x.set8(0x16, ADVICE_FRAMES);
                }
                x.set16(0x4, LEAVE_6_2);
                let owner = x.linked(0x46);
                let v = owner.u8(x.b, 0x42) | 1;
                owner.set_u8(x.b, 0x42, v);
                return;
            }
        }
        follow(x);
    }

    fn next_advice<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x5, 0u8.wrapping_sub(2));
        x.set8(0x16, ADVICE_FRAMES);
        let n = x.u16(0x14).wrapping_add(1);
        x.set16(0x14, n);
        x.anim_start(ANIMS, (n as u32) as u16);
        follow(x);
    }

    /// The bubble follows the camera (X + 0xb0, Z + 0xcf) and advances its animation.
    fn follow<R: Bench>(x: &mut Cx<'_, R>) {
        let v = x.a5_u16(ram::CAMERA_X).wrapping_add(0xb0);
        x.set16(0x8, v);
        let v = x.a5_u16(ram::CAMERA_Y).wrapping_add(0xcf);
        x.set16(0xc, v);
        x.set32(0x10, 0);
        x.anim_step();
    }

    fn leave<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(0x5) != 0 {
            return x.free(&pools::SOBJ);
        }
        x.set16(0x4, 0x200);
        load_palette(x);
    }
}

/// Attached marker (type 8): a small marker (animation 0x13b or 0x13c according to `+3`) stuck to
/// the object `+0x14`, a bit above it (0x20 or 0x10); it disappears when it leaves the screen on the left.
mod marker {
    use super::*;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                let anim = if x.u8(0x3) == 0 { 0x13b } else { 0x13c };
                x.anim_start(ANIMS, anim);
                track(x)
            }
            2 => track(x),
            _ => x.free(&pools::SOBJ),
        }
    }

    fn track<R: Bench>(x: &mut Cx<'_, R>) {
        let target = x.linked(0x14);
        for off in [0x8, 0xc, 0x10] {
            let v = target.u16(x.b, off);
            x.set16(off, v);
        }
        let lift = if x.u8(0x3) == 0 { 0x20 } else { 0x10 };
        x.add16(0xc, 0u16.wrapping_sub(lift));
        x.add16(0x10, 1);
        let limit = x.a5_u16(ram::CAMERA_X).wrapping_sub(0x40) as i16;
        if limit > x.u16(0x8) as i16 {
            return x.free(&pools::SOBJ);
        }
        x.anim_step();
        x.reinsert();
    }
}

/// Blinking frame (type 9): 8 blinks of 20 frames, placed from a table (by `+3`)
/// relative to the camera, or (if `+0x2f`) on the first player at a given depth.
mod frame {
    use super::*;

    const ANIM_BY_VARIANT: u32 = 0xdf80a;
    const OFFSETS: u32 = 0xdf87a;
    const BLINK_FRAMES: u16 = 0x14;
    const BLINKS: u8 = 8;
    /// RAM word (A5+0x46): followed player.
    const TRACKED: u32 = 0x46;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        if x.a5_u8(0x42) != 0 {
            x.set32(0x4, 0x0600_0000);
        }
        if suspended(x) {
            return;
        }
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set16(0x14, BLINK_FRAMES);
                x.set8(0x16, BLINKS);
                place(x);
                let variant = x.u8(0x3) as u32;
                let anim = x.rom16(ANIM_BY_VARIANT + 2 * variant) as u32;
                x.anim_start(ANIMS, anim as u16);
                x.reinsert();
            }
            2 => {
                place(x);
                let n = x.u16(0x14).wrapping_sub(1);
                x.set16(0x14, n);
                if (n as i16) >= 0 {
                    return;
                }
                let v = x.u8(0x1) ^ 2;
                x.set8(0x1, v);
                x.set16(0x14, BLINK_FRAMES);
                let n = x.u8(0x16).wrapping_sub(1);
                x.set8(0x16, n);
                if (n as i8) < 0 {
                    x.set8(0x4, 4);
                }
            }
            _ => x.free(&pools::SOBJ),
        }
    }

    /// Position: with `+0x2f`, it only stays if the followed player is in the depth zone
    /// 0x170..0x260; otherwise, camera + table offset.
    fn place<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(0x2f) != 0 {
            let who = x.a5_u16(TRACKED);
            let ok = who != 0 && {
                let depth = Obj(who as i16 as i32 as u32 & 0xff_ffff).u16(x.b, 0x10) as i16;
                depth < 0x260 && depth >= 0x170
            };
            if !ok {
                return x.set16(0x4, LEAVE_6_0);
            }
        }
        let row = OFFSETS + 4 * x.u8(0x3) as u32;
        let dx = x.rom16(row);
        let dz = x.rom16(row + 2);
        let v = x.a5_u16(ram::CAMERA_X).wrapping_add(dx);
        x.set16(0x8, v);
        let v = x.a5_u16(ram::CAMERA_Y).wrapping_add(dz);
        x.set16(0xc, v);
        x.set32(0x10, 0);
    }
}

/// Advice arrow (type 10): placed on the camera, blinks 10 times (20 frames); its animation
/// and the given events (4 calls to 0x19ae6, with one code per player) come from the advice number
/// of the linked object (`+0x14` of the bubble, 0x118...). Born at the end of a bubble (type 7).
mod arrow {
    use super::*;

    const ANIM_BY_ADVICE: u32 = 0xdf9ae;
    const EVENT_BY_ADVICE: u32 = 0xdfa0e;
    const LAST_ADVICE: u16 = 0x136;
    const BLINK_FRAMES: u16 = 0x14;
    const BLINKS: u8 = 0xa;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        if x.a5_u8(0x42) != 0 {
            x.set32(0x4, 0x0600_0000);
        }
        if suspended(x) {
            // a hidden or suspended arrow disappears
            return x.set32(0x4, 0x0600_0000);
        }
        match x.u8(0x4) {
            0 => start(x),
            2 => step(x),
            _ => x.free(&pools::SOBJ),
        }
    }

    fn advice_of_owner<R: Bench>(x: &mut Cx<'_, R>) -> u16 {
        let owner = x.linked(0x46);
        owner.u16(x.b, 0x14)
    }

    fn start<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u16(0x46) == 0 {
            return;
        }
        let advice = advice_of_owner(x);
        if advice == LAST_ADVICE {
            return x.set8(0x4, 4);
        }
        x.add8(0x4, 2);
        x.set16(0x16, BLINK_FRAMES);
        x.set8(0x18, BLINKS);
        place(x, 0xa7f);
        announce(x);
        pick_anim(x);
        x.reinsert();
    }

    fn place<R: Bench>(x: &mut Cx<'_, R>, dz: u16) {
        let v = x.a5_u16(ram::CAMERA_X).wrapping_add(0x58);
        x.set16(0x8, v);
        let v = x.a5_u16(ram::CAMERA_Y).wrapping_add(dz);
        x.set16(0xc, v);
        x.set32(0x10, 0);
    }

    /// Four consecutive events (codes +0x100 per player).
    fn announce<R: Bench>(x: &mut Cx<'_, R>) {
        let advice = advice_of_owner(x);
        let i = advice.wrapping_sub(0x118).wrapping_mul(2);
        let code = x.rom16(EVENT_BY_ADVICE.wrapping_add(i as i16 as i32 as u32));
        // one event per player (high byte: 0 to 3)
        for player in 0..4u16 {
            crate::events::enqueue_points(x.b, code.wrapping_add(player << 8));
        }
    }

    fn pick_anim<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u16(0x46) == 0 {
            return;
        }
        let advice = advice_of_owner(x);
        let i = advice.wrapping_sub(0x118).wrapping_mul(2);
        let anim = x.rom16(ANIM_BY_ADVICE.wrapping_add(i as i16 as i32 as u32)) as u32;
        x.anim_start(ANIMS, anim as u16);
    }

    fn step<R: Bench>(x: &mut Cx<'_, R>) {
        let n = x.u16(0x16).wrapping_sub(1);
        x.set16(0x16, n);
        if (n as i16) < 0 {
            let v = x.u8(0x1) ^ 2;
            x.set8(0x1, v);
            x.set16(0x16, BLINK_FRAMES);
            let n = x.u8(0x18).wrapping_sub(1);
            x.set8(0x18, n);
            if (n as i8) < 0 {
                return x.set8(0x4, 4);
            }
        }
        place(x, 0xaf);
    }
}

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
mod badge {
    use super::*;

    const KILL: u32 = 0x0600_0000;
    const BADGE_VARIANT: u8 = 1;
    const SOUND: u32 = 0xea428;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(0x3) == 0 {
            parent(x)
        } else {
            child(x)
        }
    }

    /// Creates a SOBJ object of type 11 of variant `variant` and parameter `param` (`+0x2c`).
    fn spawn<R: Bench>(x: &mut Cx<'_, R>, variant: u8, param: u8) -> Option<Obj> {
        let Some(o) = x.alloc(&pools::SOBJ) else { return None };
        o.set_u8(x.b, 0, 1);
        o.set_u8(x.b, 2, 0xb);
        o.set_u8(x.b, 3, variant);
        o.set_u8(x.b, 0x2c, param);
        Some(o)
    }

    fn parent<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set16(0x18, 0x100);
                x.set16(0x14, 0x158);
                if x.u8(0x2c) != 0 {
                    x.set16(0x14, 0x28);
                }
                x.set32(0x10, 0xffff_ffff);
                let camera = camera_record(x);
                x.set16(0x42, camera);
                stick_to_camera(x);
                x.anim_start(ANIMS, 0x15c);
                x.reinsert();
            }
            2 => match x.u8(0x5) {
                0 => {
                    let z = x.u16(0x18).wrapping_sub(3);
                    x.set16(0x18, z);
                    if z as i16 > 0x80 {
                        return stick_to_camera(x);
                    }
                    x.set16(0x18, 0x80);
                    let param = x.u8(0x2c);
                    match spawn(x, BADGE_VARIANT, param) {
                        Some(child) => {
                            let me = x.o.0 as u16;
                            child.set_u16(x.b, 0x46, me);
                            x.set8(0x48, 0);
                        }
                        None => x.set8(0x48, 0x78),
                    }
                    x.add8(0x5, 2);
                    stick_to_camera(x);
                    x.set8(0x44, 0);
                    x.anim_start(ANIMS, 0x15d);
                }
                2 => {
                    stick_to_camera(x);
                    let wait = x.u8(0x48);
                    if wait != 0 {
                        let n = wait - 1;
                        x.set8(0x48, n);
                        if n == 0 {
                            return back_to_idle(x);
                        }
                    } else if x.u8(0x44) != 0 {
                        return back_to_idle(x);
                    }
                    x.anim_step();
                }
                4 => {
                    let z = x.u16(0x18).wrapping_add(3);
                    x.set16(0x18, z);
                    if (z as i16) < 0x110 {
                        return stick_to_camera(x);
                    }
                    x.add8(0x4, 2);
                }
                _ => {}
            },
            4 => {
                x.set32(0x4, KILL);
                x.free(&pools::SOBJ);
            }
            _ => x.free(&pools::SOBJ),
        }
    }

    fn back_to_idle<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x5, 2);
        x.anim_start(ANIMS, 0x15c);
    }

    fn child<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                let owner = x.linked(0x46);
                let counter = owner.u8(x.b, 0x33);
                x.set8(0x49, counter);
                x.set_bit(0x1, 1, true);
                x.set8(0x48, 3);
                x.set8(0x44, 0x14);
                x.set8(0x45, 1);
                x.set16(0x18, 0x78);
                x.set16(0x14, 0x152);
                if x.u8(0x2c) != 0 {
                    x.set8(0x2c, 0);
                    x.set16(0x14, 0x2e);
                }
                x.set32(0x10, 0xffff_fffe);
                let camera = camera_record(x);
                x.set16(0x42, camera);
                stick_to_camera(x);
                x.anim_start(ANIMS, 0x15b);
                x.reinsert();
            }
            2 => {
                stick_to_camera(x);
                let owner = x.linked(0x46);
                let counter = owner.u8(x.b, 0x33);
                if counter == x.u8(0x49) {
                    return;
                }
                x.set8(0x49, counter);
                if counter != 0 {
                    x.set_bit(0x1, 1, false);
                    return crate::sound::play(x.b, SOUND, x.o);
                }
                x.set_bit(0x1, 1, true);
                let n = x.u8(0x48).wrapping_sub(1);
                x.set8(0x48, n);
                if n != 0 {
                    return;
                }
                owner.set_u8(x.b, 0x44, 1);
                x.set32(0x4, KILL);
                x.free(&pools::SOBJ);
            }
            4 => {
                x.set32(0x4, KILL);
                x.free(&pools::SOBJ);
            }
            _ => x.free(&pools::SOBJ),
        }
    }
}

/// Interface dot (type 12): stuck to the camera (Z + 0x30), animation 0x1ab + variant.
mod hud_dot {
    use super::*;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set16(0x18, 0x30);
                x.set32(0x10, 0);
                let camera = camera_record(x);
                x.set16(0x42, camera);
                stick_to_camera(x);
                // `move.w #0x1ab,D0w ; add.b (3,A0),D0b`: the addition does not overflow into the high byte
                let n = 0x100 | (0xab + x.u8(0x3) as u32) & 0xff;
                x.anim_start(ANIMS, n as u16);
                x.reinsert();
            }
            2 => stick_to_camera(x),
            4 => {
                x.set32(0x4, 0x0600_0000);
                x.free(&pools::SOBJ);
            }
            _ => x.free(&pools::SOBJ),
        }
    }
}

/// Scrolling panel (type 13): born to the right of the camera (X + 0x20, Z 0x1c8, depth -0x100),
/// advances with it until the end of its animation (script 0x3b) then orders the four registered
/// players (0xff2aa4...) to go to state 4 and releases itself.
mod panel {
    use super::*;

    const PALETTE: u32 = 0x28ff20;
    const PALETTE_DST: u32 = 0x91c3c0;
    const SLIDE_ANIMS: u32 = 0x1c40c6;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(0x4) == 0 {
            x.add8(0x4, 2);
            copy_longs(x, PALETTE, PALETTE_DST, 8);
            or_longs(x, PALETTE_DST, BRIGHT, 8);
            x.set8(0x2b, 0x1e);
            x.set8(0x2a, 1);
            let v = x.a5_u16(ram::CAMERA_X).wrapping_add(0x20);
            x.set16(0x8, v);
            x.set16(0xc, 0x1c8);
            x.set16(0x10, 0xff00);
            x.anim_start(SLIDE_ANIMS, 0x3b);
            x.reinsert();
        }
        let d = x.a5_u16(ram::CAMERA_X).wrapping_sub(x.a5_u16(ram::PREV_CAMERA_X));
        x.add16(0x8, d);
        if !x.anim_step() {
            return;
        }
        for i in 0..4 {
            let at = PLAYERS + 2 * i;
            let who = rd16(x.b, at);
            if who != 0 {
                wr16(x.b, who as i16 as i32 as u32 + 4, 0x400);
                wr16(x.b, at, 0);
            }
        }
        x.free(&pools::SOBJ);
    }
}

// ---------------------------------------------------------------------------------------------
// Type 14 : titre de niveau
// ---------------------------------------------------------------------------------------------

/// Level title (type 14): animated banner (set 0x1c03a6, number drawn from table 0xdff0c according to
/// level `A5+0x34`) stuck to the camera, whose palette (0x91c1e0) brightens in steps of 4
/// frames in 16 stages read from 0x291f00; it lasts 0xb4 frames. Levels 7 and 0x2b have their
/// own sequence.
mod stage_title {
    use super::*;

    const ANIM_BY_STAGE: u32 = 0xdff0c;
    const FADE: u32 = 0x291f00;
    const FADE_OUT_FINAL: u32 = 0x292120;
    const PALETTE_DST: u32 = 0x91c1e0;
    const DURATION: u16 = 0xb4;
    const STEP_FRAMES: u16 = 4;
    const STEPS: u16 = 0x10;
    const STAGE_7: u8 = 7;
    const STAGE_2B: u8 = 0x2b;
    /// Direct animation scripts of level 0x2b (normal, with `A5+0x70`).
    const SCRIPT_2B: u32 = 0xe0152;
    const SCRIPT_2B_ALT: u32 = 0xe01cc;
    /// `A5+0x70`: two-player game (h), shifts the title.
    const ALT: u32 = 0x70;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        if x.a5_u8(0x48f) != 0 {
            return;
        }
        match x.u8(0x4) {
            0 => start(x),
            2 => step(x),
            4 => {
                if x.a5_u8(0x34) == STAGE_2B {
                    x.set_a5_u32(0x5c6, 0);
                }
                x.free(&pools::SOBJ)
            }
            _ => x.free(&pools::SOBJ),
        }
    }

    fn fade_to<R: Bench>(x: &mut Cx<'_, R>, step: u16) {
        let from = FADE + ((step as u32) << 5 & 0xffff);
        copy_longs(x, from, PALETTE_DST, 8);
    }

    fn start<R: Bench>(x: &mut Cx<'_, R>) {
        if x.a5_u8(0x30) == 0 {
            return x.free(&pools::SOBJ);
        }
        let stage = x.a5_u8(0x34);
        if stage == STAGE_2B {
            return start_2b(x);
        }
        if x.a5_u8(0x4e) != 0 && stage == 0x1c {
            return x.free(&pools::SOBJ);
        }
        x.add8(0x4, 2);
        x.set16(0x14, DURATION);
        x.set16(0x18, STEP_FRAMES);
        x.set16(0x1c, STEPS);
        fade_to(x, STEPS);
        let mut anim = x.rom16(ANIM_BY_STAGE + 2 * stage as u32);
        if x.a5_u16(ALT) != 0 {
            anim = anim.wrapping_add(0x10d);
        }
        x.anim_start(ANIMS, (anim as u32) as u16);
        place(x);
    }

    fn start_2b<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        x.set16(0x14, DURATION);
        x.set16(0x18, STEP_FRAMES);
        x.set16(0x1c, STEPS);
        fade_to(x, STEPS);
        let at = if x.a5_u16(ALT) != 0 { SCRIPT_2B_ALT } else { SCRIPT_2B };
        x.anim_start_entry(at);
        place(x);
    }

    /// Title at (camera X + 0x44, camera Z + 0xa8); a bit further left in two-player mode.
    fn place<R: Bench>(x: &mut Cx<'_, R>) {
        let camera = Obj(0xff_0000 | camera_record(x) as u32);
        let v = camera.u16(x.b, 0x8);
        x.set16(0x8, v);
        let v = camera.u16(x.b, 0xc);
        x.set16(0xc, v);
        x.set16(0x10, 0);
        x.add16(0x8, 0x44);
        if x.a5_u16(ALT) != 0 {
            x.add16(0x8, 0u16.wrapping_sub(0xc));
        }
        x.add16(0xc, 0xa8);
    }

    fn step<R: Bench>(x: &mut Cx<'_, R>) {
        let stage = x.a5_u8(0x34);
        if stage == STAGE_7 && x.u8(0x5) == 0 {
            return step_stage_7(x);
        }
        if stage == STAGE_2B {
            return step_stage_2b(x);
        }
        place(x);
        x.anim_step();
        if tick_fade(x) {
            return finish_tick(x);
        }
        finish_tick(x)
    }

    /// One fade step: every 4 frames, next palette; at stage 0 the counter stays
    /// stuck at 0xffff.
    fn tick_fade<R: Bench>(x: &mut Cx<'_, R>) -> bool {
        let n = x.u16(0x18).wrapping_sub(1);
        x.set16(0x18, n);
        if n != 0 {
            return false;
        }
        let s = x.u16(0x1c).wrapping_sub(1);
        x.set16(0x1c, s);
        if s != 0 {
            x.set16(0x18, STEP_FRAMES);
        } else {
            x.set16(0x18, 0xffff);
        }
        let s = x.u16(0x1c);
        fade_to(x, s);
        true
    }

    /// Countdown of the title's duration; at zero it goes to state 4. Put back in the list.
    fn finish_tick<R: Bench>(x: &mut Cx<'_, R>) {
        let n = x.u16(0x14).wrapping_sub(1);
        x.set16(0x14, n);
        if n == 0 {
            x.add8(0x4, 2);
        }
        x.reinsert();
    }

    /// Level 0x2b: like the normal fade but without advancing the animation.
    fn step_stage_2b<R: Bench>(x: &mut Cx<'_, R>) {
        tick_fade(x);
        place(x);
        finish_tick(x);
    }

    /// Level 7, substate 0: the fade lasts until the negative stage, where the final palette
    /// (0x292120) is set and the substate goes to 2.
    fn step_stage_7<R: Bench>(x: &mut Cx<'_, R>) {
        place(x);
        let n = x.u16(0x18).wrapping_sub(1);
        x.set16(0x18, n);
        if n == 0 {
            let s = x.u16(0x1c).wrapping_sub(1);
            x.set16(0x1c, s);
            if (s as i16) < 0 {
                x.set16(0x18, 0xffff);
                copy_longs(x, FADE_OUT_FINAL, PALETTE_DST, 8);
                x.add8(0x5, 2);
                return x.reinsert();
            }
            x.set16(0x18, STEP_FRAMES);
            fade_to(x, s);
        }
        finish_tick(x);
    }
}

// ---------------------------------------------------------------------------------------------
// Type 15: target to hit (h)
// ---------------------------------------------------------------------------------------------

/// Target (type 15, variants 0x25 and others, rectangle `+0x16..+0x1c` around the object): when
/// a player touches it with an attack (player's `+0x58` positive), it takes the hit; on the third
/// hit (`+0x14`) it raises a RAM flag (0xff2ac0 for variant 0x25, 0xff2ac1
/// otherwise), plays a sound and goes to state 4. `+0x15` records that a player is inside, `+0x1e`
/// the code (word) saved at 0xff2ab2 on first contact.
mod target {
    use super::*;

    const RECTS: u32 = 0xe02c6;
    const HITS_TO_BREAK: u8 = 3;
    const SOUND: u32 = 0xea57e;
    const SHOOTER_CODE: u32 = 0xff_2ab2;
    const FLAG_A: u32 = 0xff_2ac0;
    const FLAG_B: u32 = 0xff_2ac1;
    const VARIANT_A: u8 = 0x25;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(0x14, 0);
                let row = if x.u8(0x3) == VARIANT_A { 0 } else { 1 };
                let rect = RECTS + 8 * row;
                for i in 0..4 {
                    let v = x.data16(rect + 2 * i);
                    x.set16(0x16 + 2 * i, v);
                }
                let v = x.u16(0x8);
                x.set16(0xa, v);
                let v = x.u16(0x10);
                x.set16(0x12, v);
                let variant = x.u8(0x3) as u32;
                x.anim_start(0x1c40c6, variant as u16);
                x.reinsert();
            }
            2 => {
                follow_camera2(x);
                let y = x.u16(0x12).wrapping_sub(cam2_dy(x));
                x.set16(0x10, y);
                x.set8(0x15, 0);
                for i in 0..4 {
                    let who = rd16(x.b, PLAYERS + 2 * i);
                    if who != 0 {
                        probe(x, Obj(who as i16 as i32 as u32 & 0xff_ffff));
                    }
                }
                follow_camera2(x);
            }
            4 => follow_camera2(x),
            _ => x.free(&pools::SOBJ),
        }
    }

    fn cam2_dx<R: Bench>(x: &mut Cx<'_, R>) -> u16 {
        x.a5_u16(ram::CAMERA2_X).wrapping_sub(x.a5_u16(ram::CAMERA_X))
    }

    fn cam2_dy<R: Bench>(x: &mut Cx<'_, R>) -> u16 {
        x.a5_u16(ram::CAMERA2_Y).wrapping_sub(x.a5_u16(ram::CAMERA_Y))
    }

    fn follow_camera2<R: Bench>(x: &mut Cx<'_, R>) {
        let v = x.u16(0xa).wrapping_sub(cam2_dx(x));
        x.set16(0x8, v);
    }

    /// Is player `p` inside the rectangle? If so, `+0x15` = 1; an attack in progress
    /// (`+0x58` > 0) compte un coup.
    fn probe<R: Bench>(x: &mut Cx<'_, R>, p: Obj) {
        let dx = x.u16(0x8).wrapping_sub(p.u16(x.b, 0x8)).wrapping_add(x.u16(0x16));
        if dx > x.u16(0x18) {
            return;
        }
        let dy = x
            .u16(0xc)
            .wrapping_add(x.u16(0x10))
            .wrapping_sub(p.u16(x.b, 0xc))
            .wrapping_sub(p.u16(x.b, 0x10))
            .wrapping_add(x.u16(0x1a));
        if dy > x.u16(0x1c) {
            return;
        }
        x.set8(0x15, 1);
        if x.u8(0x7) == 0 {
            let code = x.u16(0x1e);
            if code != 0 {
                wr16(x.b, SHOOTER_CODE, code);
                x.set8(0x7, 1);
            }
        }
        if (p.u8(x.b, 0x58) as i8) > 0 {
            hit(x, p);
        }
    }

    fn hit<R: Bench>(x: &mut Cx<'_, R>, p: Obj) {
        p.set_u8(x.b, 0x58, 0xff);
        let n = x.u8(0x14) + 1;
        x.set8(0x14, n);
        if n < HITS_TO_BREAK {
            return;
        }
        crate::sound::play(x.b, SOUND, x.o);
        let flag = if x.u8(0x3) == VARIANT_A { FLAG_A } else { FLAG_B };
        wr8(x.b, flag, 1);
        x.add8(0x4, 2);
    }
}

handlers! {
    0xdd61e "sobj_00" #[without] scenery::run,
    0xddc28 "sobj_01" #[without] fixture::run,
    0xdddb2 "sobj_02" #[without] flasher::run,
    0xdde18 "sobj_03" #[without] tag::run,
    0xdde8c "sobj_05" #[without] ghost::run,
    0xddec4 "sobj_06" #[without] scene::run,
    0xdf4d4 "sobj_07" #[without] hint::run,
    0xdf6b0 "sobj_08" #[without] marker::run,
    0xdf774 "sobj_09" #[without] frame::run,
    0xdf8ae "sobj_10" #[without] arrow::run,
    0xdfa68 "sobj_11" #[without] badge::run,
    0xdfca0 "sobj_12" #[without] hud_dot::run,
    0xdfd44 "sobj_13" #[without] panel::run,
    0xdfe3c "sobj_14" #[without] stage_title::run,
    0xe0258 "sobj_15" #[without] target::run,
}
