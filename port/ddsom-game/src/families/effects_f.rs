//! EFFCT family (continued): scene types rewritten after `effects_c.rs` (type table `0x39ce0`).
//!
//! - 0: scenery that slides into place when a scene opens, then empties the scene (variant 3:
//!   waits, scrolls the camera and disappears);
//! - 35: silhouette (variant 0) and reflection (variant 1) of a carrier (`+0x46`): the silhouette animates
//!   and passes its frame to the carrier (`+0xa0`), which the reflection translates into an animation;
//! - 53: object that crosses the screen in an arc, then triggers a scenery, waits for it to finish and creates
//!   type 55;
//! - 54: three pieces of a scene (variant 0 opens the passage: background, players and camera moved,
//!   fade task, then a boss);
//! - 55: machine that, once its delay has elapsed, throws the players, saves the palette and launches the transition;
//! - 68: departure sign, placed on the camera, which frees the players at the end of its animation;
//! - 77: sound fade (two volumes that rise in steps, then stop);
//! - 89: character who enters from the left, waits for a signal from the scene and leaves on the right.

use crate::families::cx::Bench;
use super::cx::{handlers, Cx};
use crate::mem::{rd32, wr32, A5};
use crate::objects::Obj;
use crate::objects::pools;
use crate::objects::Current;

/// Shared fields.
mod f {
    pub const STATE: u32 = 0x04;
    pub const SUB_STATE: u32 = 0x05;
    pub const VARIANT: u32 = 0x03;
    pub const FLAGS: u32 = 0x01;
    pub const FACING: u32 = 0x2c;
}

/// Routines d'origine.
mod rom {
}

/// Camera (x, depth) and secondary camera.
const CAMERA_X: u32 = 0x506;
const CAMERA_Y: u32 = 0x50a;
/// Scene step, advanced by some effects (`A5+0x4ff`).
const SCENE_STEP: u32 = 0x4ff;
/// Object the scene follows (`A5+0x46`, word).
const FOCUS: u32 = 0x46;

/// Players: four records of 0x200 bytes.
const PLAYERS: u32 = A5 + 0x5de;

/// Players present and alive.
fn living_players<R: Bench>(x: &mut Cx<'_, R>) -> Vec<Obj> {
    (0..4)
        .map(|k| Obj(PLAYERS + 0x200 * k))
        .filter(|p| p.u8(x.b, 0) != 0 && p.u16(x.b, 0x62) != 0)
        .collect()
}

fn free<R: Bench>(x: &mut Cx<'_, R>) {
    x.free(&pools::EFFCT);
}

/// Places the object on the camera, offset by (dx, dy), on the ground.
fn at_camera<R: Bench>(x: &mut Cx<'_, R>, dx: u16, dy: u16) {
    let (cx, cy) = (x.a5_u16(CAMERA_X), x.a5_u16(CAMERA_Y));
    x.set16(Obj::X, cx.wrapping_add(dx));
    x.set16(Obj::Y, cy.wrapping_add(dy));
}

/// Copies eight longs of colors from the ROM into the palette, with opacity forced (`| 0xf000f000`).
fn load_palette<R: Bench>(x: &mut Cx<'_, R>, from: u32, to: u32) {
    for k in 0..8 {
        let v = rd32(x.b, from + 4 * k);
        wr32(x.b, to + 4 * k, v | 0xf000_f000);
    }
}

/// Launches task `program` (0x9b6) with parameter D0.w = `param`.
fn new_task<R: Bench>(x: &mut Cx<'_, R>, program: u32, param: u16) {
    crate::kernel::create_task(x.b, param as u32, program);
}

// ---------------------------------------------------------------------------------------------
// Type 0: scene-opening scenery
// ---------------------------------------------------------------------------------------------

mod opening {
    pub const ANIM_SET: u32 = 0x18c04c;
    /// Start position (x, depth) and arrival abscissa (`+0x18`) per variant (words,
    /// program space).
    pub const STARTS: u32 = 0xa8602;
    pub const TARGETS: u32 = 0xa8612;
    pub const TARGET: u32 = 0x18;
    /// Variant 3: delay (`+0x1a`) before scrolling the scene.
    pub const DELAY: u32 = 0x1a;
    /// Colors of variant 3.
    pub const PALETTE: u32 = 0x28dca0;
    pub const PALETTE_RAM: u32 = 0x91c3a0;
    /// Pools emptied on arrival: 32 objects at `A5+0x62de` (except those marked `+0x58`), 18 at `A5+0x10de`.
    pub const POOL_A: u32 = 0x62de;
    pub const POOL_B: u32 = 0x10de;
    pub const SOUND_ARRIVAL: u32 = 0xeaa98;
    pub const SOUND_SCROLL: u32 = 0xedc66;
    /// Variant 3 waits for the camera to be back at this abscissa.
    pub const CAMERA_STOP: i16 = 0x5cc;
}

fn opening<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        0 => opening_init(x),
        2 => match x.u8(f::SUB_STATE) {
            0 => opening_slide(x),
            4 => opening_wait(x),
            8 => {
                x.add16(Obj::X, 0xfffe);
                if x.a5_u16(CAMERA_X) as i16 > opening::CAMERA_STOP {
                    return;
                }
                x.add8(f::SUB_STATE, 4);
                crate::sound::play(x.b, opening::SOUND_SCROLL, x.o);
            }
            12 => {
                if !x.anim_step() {
                    return;
                }
                crate::sound::play(x.b, opening::SOUND_ARRIVAL, x.o);
                let step = x.a5_u8(SCENE_STEP).wrapping_add(4);
                x.set_a5_u8(SCENE_STEP, step);
                x.add8(f::STATE, 4);
            }
            _ => {}
        },
        4 => {
            if !x.anim_step() {
                return;
            }
            x.add8(f::STATE, 2);
            let step = x.a5_u8(SCENE_STEP).wrapping_add(2);
            x.set_a5_u8(SCENE_STEP, step);
        }
        _ => free(x),
    }
}

fn opening_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    x.reinsert_foreground();
    let v = x.u8(f::VARIANT) as u32;
    let target = x.rom16(opening::TARGETS + 2 * v);
    x.set16(opening::TARGET, target);
    let (px, py) = (x.rom16(opening::STARTS + 4 * v), x.rom16(opening::STARTS + 4 * v + 2));
    x.set16(Obj::X, px);
    x.set16(Obj::Y, py);
    x.set16(Obj::Z, 0);
    if v == 3 {
        x.set8(0x2b, 0x1d);
        x.set8(0x2a, 1);
        load_palette(x, opening::PALETTE, opening::PALETTE_RAM);
        x.anim_start(opening::ANIM_SET, 0x20);
        return;
    }
    // its accompanying scenery (SOBJ 3), which keeps a long link to the effect
    if let Some(o) = x.alloc(&pools::SOBJ) {
        o.set_u8(x.b, 0, 1);
        o.set_u8(x.b, 2, 3);
        o.set_u32(x.b, 0x14, x.o.0);
    }
    x.anim_start(opening::ANIM_SET, 0x1f);
}

/// Slides left by two pixels per frame until the arrival abscissa.
fn opening_slide<R: Bench>(x: &mut Cx<'_, R>) {
    x.add16(Obj::X, 0xfffe);
    let target = x.u16(opening::TARGET);
    if (target as i16) < (x.u16(Obj::X) as i16) {
        return;
    }
    x.set16(Obj::X, target);
    if x.u8(f::VARIANT) == 3 {
        x.add8(f::SUB_STATE, 4);
        x.set16(opening::DELAY, 0x40);
        return opening_wait(x);
    }
    x.add8(f::STATE, 2);
    clear_scene(x);
    crate::sound::play(x.b, opening::SOUND_ARRIVAL, x.o);
    crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::C, 0x20);
}

/// Ends (state 6) the objects of the two pools. Like the original, an object of the first pool marked
/// `+0x58` makes the walk end with the body of the second loop, on the remaining slots of the
/// first pool: the second one is then not walked.
fn clear_scene<R: Bench>(x: &mut Cx<'_, R>) {
    let (mut slot, mut left) = (A5 + opening::POOL_A, 32);
    let mut spared = false;
    while left > 0 {
        let o = Obj(slot);
        if o.u8(x.b, 0) != 0 {
            if !spared && o.u8(x.b, 0x58) != 0 {
                spared = true;
            } else {
                o.set_u16(x.b, f::STATE, 0x600);
            }
        }
        slot += 0x60;
        left -= 1;
    }
    if spared {
        return;
    }
    for k in 0..18 {
        let o = Obj(A5 + opening::POOL_B + 0x60 * k);
        if o.u8(x.b, 0) != 0 {
            o.set_u16(x.b, f::STATE, 0x600);
        }
    }
}

/// Variant 3: delay, then the scene advances (`A5+0x4ff` + 4).
fn opening_wait<R: Bench>(x: &mut Cx<'_, R>) {
    let n = x.u16(opening::DELAY).wrapping_sub(1);
    x.set16(opening::DELAY, n);
    if (n as i16) >= 0 {
        return;
    }
    x.add8(f::SUB_STATE, 4);
    let step = x.a5_u8(SCENE_STEP).wrapping_add(4);
    x.set_a5_u8(SCENE_STEP, step);
}

// ---------------------------------------------------------------------------------------------
// Type 35: silhouette and reflection of a carrier
// ---------------------------------------------------------------------------------------------

mod mirrored {
    pub const ANIM_SET: u32 = 0x174776;
    pub const BEARER: u32 = 0x46;
    /// Current frame, passed by the silhouette to the carrier (`+0xa0` of the carrier; negative: nothing
    /// to show).
    pub const FRAME: u32 = 0x33;
    pub const BEARER_FRAME: u32 = 0xa0;
    /// Reflection animation per silhouette frame (bytes, program space; negative: hidden).
    pub const REFLECTIONS: u32 = 0xb300c;
}

fn mirrored<R: Bench>(x: &mut Cx<'_, R>) {
    if x.suspended() {
        return;
    }
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            x.set_bit(f::FLAGS, 1, true);
            x.anim_start(mirrored::ANIM_SET, 4);
        }
        2 => mirrored_follow(x),
        4 => x.add8(f::STATE, 2),
        _ => free(x),
    }
}

fn mirrored_follow<R: Bench>(x: &mut Cx<'_, R>) {
    let pointer = x.u16(mirrored::BEARER);
    let bearer = x.linked(mirrored::BEARER);
    if (pointer as i16) >= 0 || bearer.u8(x.b, 0) == 0 {
        // porteur disparu
        return x.add8(f::STATE, 2);
    }
    let frame = bearer.u8(x.b, mirrored::BEARER_FRAME);
    if (frame as i8) < 0 {
        return mirrored_hide(x);
    }
    for off in [Obj::X, Obj::Z, Obj::Y] {
        let v = bearer.u16(x.b, off);
        x.set16(off, v);
    }
    let facing = bearer.u8(x.b, f::FACING);
    x.set8(f::FACING, facing);
    if x.u8(f::SUB_STATE) == 0 {
        x.add8(f::SUB_STATE, 2);
        x.set_bit(f::FLAGS, 1, false);
        if x.u8(f::VARIANT) == 0 {
            x.anim_start(mirrored::ANIM_SET, 4);
        }
    }
    if x.u8(f::VARIANT) == 0 {
        // silhouette: one pixel in front of and above the carrier
        x.add16(Obj::Y, 1);
        x.add16(Obj::Z, 0xffff);
        if x.a5_u8(0x3d) == 0 {
            // the bearer (A1 in the original) gets the frame after the animation step
            x.anim_step();
            let frame = x.u8(mirrored::FRAME);
            bearer.set_u8(x.b, mirrored::BEARER_FRAME, frame);
        }
    } else {
        // reflection: one pixel behind and below
        x.add16(Obj::Y, 0xffff);
        x.add16(Obj::Z, 1);
        if frame != x.u8(mirrored::FRAME) {
            let anim = x.rom8(mirrored::REFLECTIONS + frame as u32);
            if (anim as i8) < 0 {
                return mirrored_hide(x);
            }
            x.anim_start(mirrored::ANIM_SET, (anim as u32) as u16);
            x.set_bit(f::FLAGS, 1, false);
        }
    }
    x.reinsert();
}

fn mirrored_hide<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(f::SUB_STATE, 0);
    x.set_bit(f::FLAGS, 1, true);
    x.reinsert();
}

// ---------------------------------------------------------------------------------------------
// Type 53: object that crosses the screen, then triggers the rest of the scene
// ---------------------------------------------------------------------------------------------

mod crossing {
    pub const ANIM_SET: u32 = 0x19685a;
    pub const TIMER: u32 = 0x42;
    /// Scenery created (pointer) and its scene scenery number.
    pub const SCENERY: u32 = 0x5c;
    pub const SCENERY_KIND: u32 = 0x83;
    pub const SOUND_LAND: u32 = 0xe3a2e;
    pub const SOUND_WAIT: u32 = 0xe3a98;
    pub const SOUND_DONE: u32 = 0xe3b02;
}

fn crossing<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            x.set_a5_u16(FOCUS, x.o.0 as u16);
            at_camera(x, 0x180, 0x50);
            x.set16(Obj::Z, 0);
            x.set16(Obj::VX, 0xfc00);
            x.set16(Obj::AX, 0);
            x.set16(Obj::VZ, 0x300);
            x.set16(Obj::AZ, 0xffc0);
            x.anim_start(crossing::ANIM_SET, 0);
            x.reinsert();
        }
        2 => crossing_steps(x),
        4 => x.add8(f::STATE, 2),
        _ => free(x),
    }
}

/// The steps (substate, by 4) chain within the same frame when one of them finishes.
fn crossing_steps<R: Bench>(x: &mut Cx<'_, R>) {
    let mut step = x.u8(f::SUB_STATE);
    loop {
        match step {
            // arched flight down to the ground
            0 => {
                x.anim_step();
                if x.move_xz().pl() {
                    return;
                }
                next_step(x, 1, 0x18);
            }
            // glides on its momentum
            4 => {
                x.anim_step();
                x.move_x();
                if !timer_out(x) {
                    return;
                }
                x.add8(f::SUB_STATE, 4);
                crate::sound::play(x.b, crossing::SOUND_LAND, x.o);
                x.anim_start(crossing::ANIM_SET, 3);
                x.reinsert();
            }
            8 => {
                if !x.anim_step() {
                    return;
                }
                next_step(x, 4, 0xc0);
            }
            12 => {
                x.anim_step();
                if !timer_out(x) {
                    return;
                }
                let Some(scenery_obj) = super::creators::scene_set(x.b, crossing::SCENERY_KIND as u8) else { return };
                x.set16(crossing::SCENERY, scenery_obj.0 as u16);
                x.add8(f::SUB_STATE, 4);
                x.set16(crossing::TIMER, 0);
                x.anim_start(crossing::ANIM_SET, 5);
            }
            // waits for the scenery to finish, playing a sound every 0x30 frames
            16 => {
                x.anim_step();
                let scenery = x.linked(crossing::SCENERY);
                if timer_out(x) {
                    x.set16(crossing::TIMER, 0x30);
                    crate::sound::play(x.b, crossing::SOUND_WAIT, x.o);
                }
                if scenery.u8(x.b, 0x22) == 0 {
                    return;
                }
                x.add8(f::SUB_STATE, 4);
                x.anim_start(crossing::ANIM_SET, 6);
            }
            20 => {
                if !x.anim_step() {
                    return;
                }
                crate::sound::play(x.b, crossing::SOUND_DONE, x.o);
                let o = x.o;
                let launcher = super::effects_creators::launcher(x.b, o);
                if launcher.is_none() {
                    return;
                }
                x.add8(f::SUB_STATE, 4);
                x.anim_start(crossing::ANIM_SET, 7);
                return;
            }
            _ => return,
        }
        step += 4;
    }
}

/// Goes to the next step: delay, animation `anim`, put back in the actors' list.
fn next_step<R: Bench>(x: &mut Cx<'_, R>, anim: u32, timer: u16) {
    x.add8(f::SUB_STATE, 4);
    x.set16(crossing::TIMER, timer);
    x.anim_start(crossing::ANIM_SET, anim as u16);
    x.reinsert();
}

/// `subq.w #1,(+0x42)` followed by `bpl`: true when the delay is exhausted (negative).
fn timer_out<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let n = x.u16(crossing::TIMER).wrapping_sub(1);
    x.set16(crossing::TIMER, n);
    (n as i16) < 0
}

// ---------------------------------------------------------------------------------------------
// Type 54: pieces of a scene that open the passage to a boss
// ---------------------------------------------------------------------------------------------

mod passage {
    pub const ANIM_SET: u32 = 0x196a28;
    /// Positions (x, depth) per variant (words, program space).
    pub const PLACES: u32 = 0xb693c;
    /// Scene fade (`A5+0x48e`: in progress as long as non-zero) and its setting.
    pub const FADE: u32 = 0x48e;
    pub const FADE_PROGRAM: u32 = 0x1ced4;
}

fn passage<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            let v = x.u8(f::VARIANT) as u32;
            let (px, py) = (x.rom16(passage::PLACES + 4 * v), x.rom16(passage::PLACES + 4 * v + 2));
            x.set16(Obj::X, px);
            x.set16(Obj::Y, py);
            x.set16(Obj::Z, 0);
            x.anim_start(passage::ANIM_SET, v as u16);
            x.reinsert();
        }
        2 => {
            // waits for the object followed by the scene to go to state 4
            let focus = x.a5_u16(FOCUS);
            if focus == 0 {
                return;
            }
            let state = Obj(focus as i16 as i32 as u32 & 0xff_ffff).u8(x.b, f::STATE);
            if (state as i8) < 4 {
                x.anim_step();
                return;
            }
            if x.u8(f::VARIANT) != 0 {
                return free(x);
            }
            x.add8(f::STATE, 2);
            x.anim_start(passage::ANIM_SET, 3);
        }
        4 if x.u8(f::SUB_STATE) == 0 => passage_open(x),
        4 => passage_boss(x),
        _ => free(x),
    }
}

/// Opens the passage: the scene moves back by 0x7c8 pixels (players included), then a fade is launched.
fn passage_open<R: Bench>(x: &mut Cx<'_, R>) {
    if x.a5_u8(passage::FADE) != 0 {
        return;
    }
    x.unlink();
    for p in living_players(x) {
        let (px, pz, py) = (p.u16(x.b, Obj::X), p.u16(x.b, Obj::Z), p.u16(x.b, Obj::Y));
        p.set_u16(x.b, Obj::X, px.wrapping_sub(0x7c8));
        p.set_u16(x.b, Obj::Z, pz.wrapping_sub(0x50));
        p.set_u16(x.b, Obj::Y, py.wrapping_add(0x50));
    }
    x.set_a5_u16(CAMERA_X, 0x4b0);
    x.set_a5_u16(0x52c, 0);
    x.set_a5_u16(0x490, 4);
    x.set_a5_u8(passage::FADE, 13);
    x.set_a5_u8(0x48f, 1);
    x.set_a5_u32(0x492, 0xff1190);
    x.set_a5_u32(0x49a, 0x266fe0);
    x.set_a5_u32(0x49e, 0x27cec0);
    new_task(x, passage::FADE_PROGRAM, 0x10);
    x.add8(f::SUB_STATE, 1);
}

/// Once the fade is done, creates the boss (type 7) in front of the camera and cedes control to the scene.
fn passage_boss<R: Bench>(x: &mut Cx<'_, R>) {
    if x.a5_u8(passage::FADE) != 0 {
        return;
    }
    let Some(boss) = x.alloc(&pools::BOSS) else { return };
    boss.set_u8(x.b, 0, 1);
    boss.set_u8(x.b, 2, 7);
    let (cx, cy) = (x.a5_u16(CAMERA_X), x.a5_u16(CAMERA_Y));
    boss.set_u16(x.b, Obj::X, cx.wrapping_add(0xc0));
    boss.set_u16(x.b, Obj::Y, cy.wrapping_add(0x50));
    x.set_a5_u16(0x51e, 0);
    x.set_a5_u8(0x2e, 8);
    free(x);
}

// ---------------------------------------------------------------------------------------------
// Type 55: machine that throws the players and launches the transition
// ---------------------------------------------------------------------------------------------

mod launcher {
    pub const ANIM_SET: u32 = 0x16ff94;
    pub const TIMER: u32 = 0x42;
    /// Object notified at departure (set to state 4).
    pub const PARTNER: u32 = 0x5c;
    pub const SOUND_START: u32 = 0xed474;
    pub const SOUND_LAUNCH: u32 = 0xedcd0;
    /// Palette save (0x91c000, 33 × 32 bytes) for the fade.
    pub const PALETTE_RAM: u32 = 0x91c000;
    pub const PALETTE_COPY: u32 = 0xff1190;
    pub const FADE_PROGRAM: u32 = 0x1ccf8;
}

fn launcher<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            x.set16(launcher::TIMER, 0x40);
            crate::sound::play(x.b, launcher::SOUND_START, x.o);
            x.set16(Obj::X, 0xa00);
            x.set16(Obj::Y, 0x17c);
            x.anim_start(launcher::ANIM_SET, 0);
            x.reinsert();
        }
        2 => {
            x.anim_step();
            let n = x.u16(launcher::TIMER).wrapping_sub(1);
            x.set16(launcher::TIMER, n);
            if (n as i16) >= 0 {
                return;
            }
            launch(x);
        }
        _ => free(x),
    }
}

fn launch<R: Bench>(x: &mut Cx<'_, R>) {
    let partner = x.linked(launcher::PARTNER);
    partner.set_u16(x.b, f::STATE, 0x400);
    for k in 0..4 {
        let p = Obj(PLAYERS + 0x200 * k);
        if p.u8(x.b, 0) == 0 || p.u16(x.b, 0x62) == 0 {
            continue;
        }
        let px = p.u16(x.b, Obj::X).wrapping_add(0x310);
        p.set_u16(x.b, Obj::X, px);
        crate::sound::service(x.b, crate::sound::Service::Ff05);
        crate::sound::play(x.b, launcher::SOUND_LAUNCH, x.o);
        // player animation 2, then the player is thrown into the air
        crate::objects::anim::start_by_character(x.b, p, &crate::objects::anim::SETS_BY_VARIANT, 2);
        p.set_u16(x.b, Obj::VX, 0);
        p.set_u16(x.b, Obj::AX, 0);
        p.set_u16(x.b, Obj::VZ, 0x400);
        p.set_u16(x.b, Obj::AZ, 0xfff0);
        p.set_u16(x.b, Obj::Z, 0x50);
        let py = p.u16(x.b, Obj::Y).wrapping_sub(0x50);
        p.set_u16(x.b, Obj::Y, py);
        p.set_u32(x.b, f::STATE, 0x0008_040e);
    }
    for k in 0..33 * 8 {
        let v = rd32(x.b, launcher::PALETTE_RAM + 4 * k);
        wr32(x.b, launcher::PALETTE_COPY + 4 * k, v);
    }
    x.set_a5_u8(0x48f, 1);
    x.set_a5_u16(0x490, 1);
    x.set_a5_u8(0x48e, 13);
    new_task(x, launcher::FADE_PROGRAM, 0x10);
    x.set_a5_u16(CAMERA_X, 0xc78);
    // sparks (type 56): seventeen in principle (`dbf D6`), but the allocation uses D6 to walk the pool.
    // Original glitch: the loop counts in D6 (`dbf`), which the allocation rewrites (rank of the slot found
    // from the end of the pool): it stops when the pool is full, or when the last slot has just been taken.
    let last = pools::EFFCT.slot(pools::EFFCT.slots - 1);
    while let Some(shard) = super::effects_creators::camera_shard(x.b) {
        if shard == last {
            break;
        }
    }
    free(x);
}

// ---------------------------------------------------------------------------------------------
// Type 68: departure sign
// ---------------------------------------------------------------------------------------------

mod start_sign {
    pub const ANIM_SET: u32 = 0x197ef0;
    pub const PALETTE: u32 = 0x28ff20;
    pub const PALETTE_RAM: u32 = 0x91c380;
    /// Request to reload the colors.
    pub const RELOAD: u32 = 0x40;
}

fn start_sign_palette<R: Bench>(x: &mut Cx<'_, R>) {
    load_palette(x, start_sign::PALETTE, start_sign::PALETTE_RAM);
    x.set8(0x2a, 1);
    x.set8(0x2b, 0x1c);
}

fn start_sign<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            start_sign_palette(x);
            at_camera_z(x);
            x.anim_start(start_sign::ANIM_SET, 0);
            x.reinsert();
        }
        2 => {
            at_camera_z(x);
            if x.u8(start_sign::RELOAD) != 0 {
                start_sign_palette(x);
                x.set8(start_sign::RELOAD, 0);
            }
            if x.anim_step() && x.a5_u16(0x52) == 0 {
                // end of the animation: the living, standing players (step ≤ 10) set off again
                x.set16(f::STATE, 0x602);
                x.set_a5_u8(0x59, 0);
                for p in living_players(x) {
                    if p.u8(x.b, 6) <= 10 {
                        p.set_u32(x.b, f::STATE, 0x0200_2c00);
                    }
                }
            }
            x.reinsert();
        }
        _ => {
            if x.u8(f::SUB_STATE) != 0 {
                return free(x);
            }
            x.set16(f::STATE, 0x200);
            start_sign_palette(x);
        }
    }
}

/// Sign placed at (0xb0, 0xd0) from the camera, in height (`+0xc`), zero depth.
fn at_camera_z<R: Bench>(x: &mut Cx<'_, R>) {
    let (cx, cy) = (x.a5_u16(CAMERA_X), x.a5_u16(CAMERA_Y));
    x.set16(Obj::X, cx);
    x.set16(Obj::Z, cy);
    x.set32(Obj::Y, 0);
    x.add16(Obj::X, 0xb0);
    x.add16(Obj::Z, 0xd0);
}

// ---------------------------------------------------------------------------------------------
// Type 77 : fondu sonore
// ---------------------------------------------------------------------------------------------

mod sound_fade {
    pub const STEPS: u32 = 0x30;
    pub const DELAY: u32 = 0x32;
    pub const LEVEL_A: u32 = 0x34;
    pub const LEVEL_B: u32 = 0x36;
}

/// Sound levels: service commands 0xff08 and 0xff09 (0xe0ad8, 0xe0af8), with the level as parameter.
fn sound_fade_levels<R: Bench>(x: &mut Cx<'_, R>) {
    let a = x.u16(sound_fade::LEVEL_A);
    crate::sound::service(x.b, crate::sound::Service::Ff08(a));
    let b = x.u16(sound_fade::LEVEL_B);
    crate::sound::service(x.b, crate::sound::Service::Ff09(b));
}

fn sound_fade<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            x.set16(sound_fade::STEPS, 0x18);
            x.set16(sound_fade::DELAY, 5);
            x.set16(sound_fade::LEVEL_A, 0xff00);
            x.set16(sound_fade::LEVEL_B, 0xff00);
            sound_fade_levels(x);
        }
        2 if x.u8(f::SUB_STATE) == 0 => {
            let n = x.u16(sound_fade::DELAY).wrapping_sub(1);
            x.set16(sound_fade::DELAY, n);
            if (n as i16) > 0 {
                return;
            }
            x.add16(sound_fade::LEVEL_A, 2);
            x.add16(sound_fade::LEVEL_B, 4);
            sound_fade_levels(x);
            x.set16(sound_fade::DELAY, 5);
            let n = x.u16(sound_fade::STEPS).wrapping_sub(1);
            x.set16(sound_fade::STEPS, n);
            if (n as i16) > 0 {
                return;
            }
            crate::sound::service(x.b, crate::sound::Service::Ff07 { lock: 0, line: 0xc });
            free(x);
        }
        2 => {}
        _ => free(x),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 89: character who crosses the scene
// ---------------------------------------------------------------------------------------------

mod walker {
    pub const ANIM_SET: u32 = 0x19920e;
    pub const DELAY: u32 = 0x54;
    /// Scene signal (`A5-0x55c8`) that makes it leave again.
    pub const SIGNAL: u32 = 0xff2a38;
}

fn walker<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            x.set8(f::FACING, 0x20);
            x.set8(walker::DELAY, 0x38);
            at_camera(x, 0xffd0, 0x60);
            x.push_terrain();
            x.anim_start(walker::ANIM_SET, 0);
            x.reinsert();
        }
        2 => {
            match x.u8(f::SUB_STATE) {
                // enters walking, four pixels per frame
                0 => {
                    let n = x.u8(walker::DELAY).wrapping_sub(1);
                    x.set8(walker::DELAY, n);
                    if n == 0 {
                        x.add8(f::SUB_STATE, 2);
                        x.anim_start(walker::ANIM_SET, 1);
                    } else {
                        x.add16(Obj::X, 4);
                        x.anim_step();
                    }
                }
                2 => {
                    x.anim_step();
                    if crate::mem::rd8(x.b, walker::SIGNAL) == 0x14 {
                        x.add8(f::SUB_STATE, 2);
                        x.anim_start(walker::ANIM_SET, 0);
                    }
                }
                // leaves on the right, up to 0x1e0 pixels from the camera
                _ => {
                    let limit = x.a5_u16(CAMERA_X).wrapping_add(0x1e0);
                    if limit > x.u16(Obj::X) {
                        x.add16(Obj::X, 4);
                    }
                    x.anim_step();
                }
            }
            x.push_terrain();
            x.reinsert();
        }
        _ => free(x),
    }
}

handlers! {
    0xa8594 "effct_opening" #[without] opening,
    0xb2f40 "effct_silhouette_reflection" #[without] mirrored,
    0xb66f8 "effct_traversee" #[without] crossing,
    0xb68f4 "effct_passage" #[without] passage,
    0xb6a74 "effct_launcher" #[without] launcher,
    0xb7f28 "effct_sign_start" #[without] start_sign,
    0xb896e "effct_sound_fade" #[without] sound_fade,
    0xba0b0 "effct_passant" #[without] walker,
}
