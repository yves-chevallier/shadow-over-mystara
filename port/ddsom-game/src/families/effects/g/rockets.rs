use super::*;

// ---------------------------------------------------------------------------------------------
// Type 80: fireworks of a character's scene (created by `0x23606`, variants 2 and 3)
// ---------------------------------------------------------------------------------------------

pub(super) mod fireworks {
    /// Scene step that the actors wait for before leaving (`A5+0x2e`), and return to the game (0xc).
    pub const SCENE_STEP: u32 = 0x2e;
    pub const GO: u8 = 0x12;
    pub const BACK: u8 = 0xc;
    /// Fade in progress (`A5+0x48f`), its settings (`A5+0x48e`, `A5+0x490`) and its palettes (`A5+0x492...`).
    pub const FADING: u32 = 0x48f;
    pub const FADE_SPEED: u32 = 0x48e;
    pub const FADE_MODE: u32 = 0x490;
    pub const FADE_SOURCES: u32 = 0x492;
    /// Fade tasks (programs of `0x9b6`): toward black, then back.
    pub const FADE_OUT: u32 = 0x1ccf8;
    pub const FADE_IN: u32 = 0x1ced4;
    /// Fireworks camera (`A5+0x5a6`, `A5+0x5aa`).
    pub const CAMERA_X: u32 = 0x5a6;
    pub const CAMERA_Y: u32 = 0x5aa;
    pub const TIMER: u32 = 0x58;
    pub const DELAY: u32 = 0x5a;
    pub const HOST: u32 = 0x56;
    pub const SCENERY: u32 = 0x42;
    /// Rockets (type 50): variant, delay and offset (x, y) of each shot (program space).
    pub const ROCKET_VARIANTS: u32 = 0xb9628;
    pub const ROCKET_DELAYS: u32 = 0xb9638;
    pub const ROCKET_PLACES: u32 = 0xb9658;
    pub const ANIMS_HERO: u32 = 0x1836ec;
    pub const ANIMS_FIRE: u32 = 0x1824d4;
    pub const ANIMS_LAST: u32 = 0x18373a;
}

/// Scene sounds.
pub(super) mod sfx {
    pub const LAST_ROCKET: crate::sound::Sound = crate::sound::CMD_5C;
    pub const LIFT_OFF: crate::sound::Sound = crate::sound::CMD_80_80;
    pub const JUMP: crate::sound::Sound = crate::sound::CMD_30C_40C;
    pub const CHEER: crate::sound::Sound = crate::sound::CMD_311_411;
    pub const LANDING: crate::sound::Sound = crate::sound::CMD_35E_45E;
}

pub(super) fn fireworks<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        0 => fireworks_init(x),
        2 => fireworks_active(x),
        _ => free(x),
    }
    // the game task waits (`hero_rockets`): the end of the handler is step `AfterState`
    if crate::tasks::has_ceded() {
        return;
    }
    fireworks_tail(x);
}

/// `sub.w #1,(off,A0)`: returns the exact value (no wrap to 16 bits) that the following `bgt`/`ble` test.
pub(super) fn countdown<R: Bench>(x: &mut Cx<'_, R>, off: u32) -> i32 {
    let v = x.u16(off);
    x.set16(off, v.wrapping_sub(1));
    v as i16 as i32 - 1
}

/// Common ending (`0xb8f04`): the object, if still active, is re-listed.
pub(super) fn fireworks_tail<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Obj::ACTIVE) != 0 {
        x.reinsert();
    }
}

pub(super) fn scene_go<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    x.a5_u8(fireworks::SCENE_STEP) == fireworks::GO
}

pub(super) fn fading<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    x.a5_u8(fireworks::FADING) != 0
}

/// Sets X and Y (and Z) in screen coordinates.
pub(super) fn place<R: Bench>(x: &mut Cx<'_, R>, px: u16, z: Option<u16>, py: u16) {
    x.set16(Obj::X, px);
    if let Some(z) = z {
        x.set16(Obj::Z, z);
    }
    x.set16(Obj::Y, py);
}

/// An actor attached to object `+0x56`: X - `dx`, Z + 0x60, right in front of it.
pub(super) fn follow_host<R: Bench>(x: &mut Cx<'_, R>, dx: u16) {
    let host = x.linked(fireworks::HOST);
    let (hx, hz, hy) = (host.u16(x.b, Obj::X), host.u16(x.b, Obj::Z), host.u32(x.b, Obj::Y));
    x.set16(Obj::X, hx);
    x.set16(Obj::Z, hz);
    x.set32(Obj::Y, hy);
    x.add16(Obj::X, dx.wrapping_neg());
    x.add16(Obj::Z, 0x60);
    x.add16(Obj::Y + 2, 0xffff);
}

pub(super) fn fireworks_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    match x.u8(f::VARIANT) {
        0 => {
            x.set8(f::FACING, 0x20);
            place(x, 0x50, None, 0x160);
            x.anim_start(fireworks::ANIMS_HERO, 0);
        }
        1 => {
            x.set8(0x2a, 1);
            x.set8(0x2b, 0x18);
            place(x, 0x110, None, 0x161);
            x.anim_start(0x17b14e, 0);
        }
        2 => {
            place(x, 0x178, None, 0x154);
            x.anim_start(0x177874, 0);
        }
        3 => {
            place(x, 0x158, Some(0x18), 0x168);
            x.set16(Obj::Y + 2, 0xffff);
            // its two sidekicks (variants 4 and 5)
            for (variant, dx) in [(4u8, 0x20u16), (5, 0x10)] {
                let Some(o) = x.alloc(&pools::EFFCT) else { break };
                o.set_u8(x.b, Obj::ACTIVE, 1);
                o.set_u8(x.b, Obj::KIND, 80);
                o.set_u8(x.b, Obj::VARIANT, variant);
                let (px, pz, py) = (x.u16(Obj::X), x.u16(Obj::Z), x.u32(Obj::Y));
                o.set_u16(x.b, Obj::X, px.wrapping_sub(dx));
                o.set_u16(x.b, Obj::Z, pz.wrapping_add(0x60));
                o.set_u32(x.b, Obj::Y, py);
                let frac = o.u16(x.b, Obj::Y + 2).wrapping_sub(1);
                o.set_u16(x.b, Obj::Y + 2, frac);
                o.set_u16(x.b, fireworks::HOST, x.o.0 as u16);
            }
            x.anim_start(fireworks::ANIMS_FIRE, 0);
        }
        4 => {
                x.anim_start(fireworks::ANIMS_FIRE, 2);
            }
        5 => {
                x.anim_start(fireworks::ANIMS_FIRE, 3);
            }
        _ => {
            load_colours(x, 0x253460, 0x91c2e0, 0x20);
            x.set8(0x2a, 1);
            x.set8(0x2b, 0x17);
            place(x, 0x120, Some(0xe0), 0x160);
            x.anim_start(fireworks::ANIMS_LAST, 3);
        }
    }
}

/// Copies `n` colours from the ROM (data space) into the palette, opaque (`| 0xf000`).
pub(super) fn load_colours<R: Bench>(x: &mut Cx<'_, R>, from: u32, to: u32, n: u32) {
    for k in 0..n {
        let v = x.data16(from + 2 * k) | 0xf000;
        crate::mem::wr16(x.b, to + 2 * k, v);
    }
}

/// `0xb95b8`: copies 0x400 bytes from `from` to `to` in blocks of eight longs (`movem` through D1-D6, A3, A4).
/// Original glitch: the last block's longs 1 to 4 stay in D2-D5 (`dispatch::Leftovers`).
pub(super) fn copy_block<R: Bench>(x: &mut Cx<'_, R>, from: u32, to: u32) {
    let mut last = [0u32; 8];
    for k in 0..0x100 {
        let v = crate::mem::rd32(x.b, from + 4 * k);
        crate::mem::wr32(x.b, to + 4 * k, v);
        last[(k % 8) as usize] = v;
    }
    (*x.c.d2(), *x.c.d3(), *x.c.d4(), *x.c.d5()) = (last[1], last[2], last[3], last[4]);
}

/// Launches fade task `program` (`0x9b6`, D0.w = 0x10), A0 kept.
pub(super) fn fade_task<R: Bench>(x: &mut Cx<'_, R>, program: u32) {
    crate::kernel::create_task(x.b, 0x10, program);
}

pub(super) fn fireworks_active<R: Bench>(x: &mut Cx<'_, R>) {
    let sub = x.u8(f::SUB_STATE);
    match x.u8(f::VARIANT) {
        0 => hero(x, sub),
        1 => match sub {
            0 if scene_go(x) => {
                x.add8(f::SUB_STATE, 2);
                x.anim_start(0x17b162, 1);
            }
            2 if !fading(x) => {
                x.add8(f::SUB_STATE, 2);
                x.anim_start(0x17b166, 7);
            }
            0 | 2 => {
                x.anim_step();
            }
            _ => {}
        },
        2 => match sub {
            0 if scene_go(x) => {
                x.add8(f::SUB_STATE, 2);
                x.set16(Obj::VX, 0xfe20);
                x.set16(Obj::AX, 0);
                x.set16(Obj::VZ, 0x340);
                x.set16(Obj::AZ, 0xfff0);
                crate::sound::play(x.b, sfx::JUMP, x.o);
                x.anim_start(0x17788c, 7);
            }
            0 => {
                x.anim_step();
            }
            _ => {
                x.anim_step();
                if x.u8(0x33) == 0 && !fading(x) {
                    return free(x);
                }
                x.move_xz();
            }
        },
        3 => match sub {
            0 => {
                if scene_go(x) {
                    x.add8(f::SUB_STATE, 2);
                    crate::sound::play(x.b, sfx::CHEER, x.o);
                }
                x.anim_step();
            }
            _ => {
                x.add16(Obj::X, 0xfffe);
                x.anim_step();
                if !fading(x) {
                    free(x);
                }
            }
        },
        4 => {
            if sub != 0 && !fading(x) {
                return free(x);
            }
            follow_host(x, 0x20);
            if sub != 0 {
                x.anim_step();
            } else if scene_go(x) {
                x.add8(f::SUB_STATE, 2);
                x.anim_step();
            }
        }
        5 => {
            follow_host(x, 0x10);
            if sub == 0 {
                if scene_go(x) {
                    x.add8(f::SUB_STATE, 2);
                }
            } else if !fading(x) {
                free(x);
            }
        }
        _ => last_actor(x, sub),
    }
}

/// Variant 6: falls onto the scene, plays its animation, waits for the return to the game then blinks and leaves.
pub(super) fn last_actor<R: Bench>(x: &mut Cx<'_, R>, sub: u8) {
    match sub {
        0 => {
            let z = x.u16(Obj::Z) as i16 as i32 - 6;
            x.add16(Obj::Z, 0xfffa);
            if z <= 0 {
                x.add8(f::SUB_STATE, 2);
                crate::sound::play(x.b, sfx::LANDING, x.o);
                x.set16(Obj::Z, 0);
                x.anim_start(fireworks::ANIMS_LAST, 5);
                return;
            }
            if x.u8(0x33) == 0 {
                x.anim_step();
            }
        }
        2 => {
            if x.anim_step() {
                x.add8(f::SUB_STATE, 2);
                x.set16(fireworks::TIMER, 0x40);
            }
        }
        4 => {
            if x.a5_u8(fireworks::SCENE_STEP) == fireworks::BACK {
                x.add8(f::SUB_STATE, 2);
                x.set16(fireworks::TIMER, 0x40);
            }
        }
        _ => {
            if countdown(x, fireworks::TIMER) <= 0 {
                return free(x);
            }
            let v = x.u8(f::FLAGS) ^ 2;
            x.set8(f::FLAGS, v);
        }
    }
}

/// Variant 0: the character who fires the fireworks (nine steps).
pub(super) fn hero<R: Bench>(x: &mut Cx<'_, R>, sub: u8) {
    match sub {
        0 => {
            if !scene_go(x) {
                x.anim_step();
                return;
            }
            x.add8(f::SUB_STATE, 2);
            x.set16(Obj::VX, 0x140);
            x.set16(Obj::AX, 0);
            x.set16(Obj::VZ, 0x340);
            x.set16(Obj::AZ, 0xfff0);
            x.anim_start(fireworks::ANIMS_HERO, 4);
            // the scene goes black: palettes saved, fade
            copy_block(x, 0x91c000, 0x910000);
            copy_block(x, 0x91c800, 0x910400);
            copy_block(x, 0x91cc00, 0x910800);
            x.set_a5_u16(fireworks::FADE_MODE, 3);
            x.set_a5_u8(fireworks::FADE_SPEED, 0xf);
            x.set_a5_u8(fireworks::FADING, 1);
            fade_task(x, fireworks::FADE_OUT);
        }
        2 => {
            x.move_xz();
            x.anim_step();
            if fading(x) {
                return;
            }
            x.add8(f::SUB_STATE, 2);
            x.set16(Obj::X, 0xc0);
            x.set16(Obj::Z, 0);
            load_colours(x, 0x253260, 0x91c260, 0x10);
            x.set16(fireworks::TIMER, 0x10);
            x.set16(fireworks::DELAY, 0);
            x.anim_start(fireworks::ANIMS_HERO, 0);
        }
        4 => hero_rockets(x),
        6 => {
            if !fading(x) {
                x.add8(f::SUB_STATE, 2);
                x.set16(fireworks::TIMER, 0x14);
                crate::families::small::clear_scroll1_start(x);
            }
            x.anim_step();
        }
        8 => {
            if countdown(x, fireworks::DELAY) <= 0 {
                // scenery 0x55, even if it means waiting for a slot
                // Original glitch: a full SET pool makes it restart endlessly (the game freezes).
                let scenery = loop {
                    if let Some(o) = crate::families::creators::scene_set(x.b, 0x55) {
                        break o;
                    }
                };
                x.set16(fireworks::SCENERY, scenery.0 as u16);
                x.add8(f::SUB_STATE, 2);
            }
            x.anim_step();
        }
        10 => {
            let scenery = x.linked(fireworks::SCENERY);
            if scenery.u8(x.b, 0x22) != 0 {
                x.add8(f::SUB_STATE, 2);
                x.set16(fireworks::TIMER, 0xa);
            }
            x.anim_step();
        }
        12 => {
            if countdown(x, fireworks::TIMER) > 0 {
                x.anim_step();
                return;
            }
            x.add8(f::SUB_STATE, 2);
            x.set16(Obj::VZ, 0x700);
            x.set16(Obj::AZ, 0);
            x.set32(Obj::VX, 0);
            crate::sound::play(x.b, sfx::LIFT_OFF, x.o);
            x.anim_start(fireworks::ANIMS_HERO, 6);
        }
        14 => hero_lift_off(x),
        _ => {}
    }
}

/// Step 4: one rocket each time the delay elapses, the last with a sound; then four frames of waiting
/// (the task cedes control) and the fade back.
pub(super) fn hero_rockets<R: Bench>(x: &mut Cx<'_, R>) {
    if countdown(x, fireworks::DELAY) > 0 {
        return;
    }
    let t = countdown(x, fireworks::TIMER);
    if t >= 0 {
        if t == 0 {
            crate::sound::play(x.b, sfx::LAST_ROCKET, x.o);
        }
        return rocket(x);
    }
    x.add8(f::SUB_STATE, 2);
    // The whole game waits four frames (`jsr 0xa40` in the middle of the dispatch loop): the task will resume at step
    // `AfterWait`, then, on return, at `AfterState` (the end of the handler), like the original (the trap leaves D0 and
    // D1, which nobody reads).
    crate::tasks::table::call(x.b, Step::AfterState);
    crate::kernel::wait(x.b, 4, Step::AfterWait);
    // the dispatch loop and the pending Rust callers give up (`tasks::has_ceded`): the task will resume through the
    // steps, then through the loop's pending call (`dispatch::Step::Dispatch`)
}

/// After the wait: fade back from the saved palettes.
pub(super) fn fireworks_fade_back<R: Bench>(x: &mut Cx<'_, R>) {
    x.set_a5_u16(fireworks::FADE_MODE, 1);
    x.set_a5_u8(fireworks::FADE_SPEED, 0xf);
    x.set_a5_u8(fireworks::FADING, 1);
    for (k, v) in [0x910000u32, 0x25c0c0, 0x910400, 0x910800].into_iter().enumerate() {
        x.set_a5_u32(fireworks::FADE_SOURCES + 4 * k as u32, v);
    }
    fade_task(x, fireworks::FADE_IN);
}

/// `0xb95e8`: a rocket (type 50): variant, delay until the next one and place read for shot `TIMER`.
pub(super) fn rocket<R: Bench>(x: &mut Cx<'_, R>) {
    let Some(o) = x.alloc(&pools::EFFCT) else { return };
    o.set_u8(x.b, Obj::ACTIVE, 1);
    o.set_u8(x.b, Obj::KIND, 0x32);
    let n = x.u16(fireworks::TIMER) as i16 as i32 as u32;
    let variant = x.rom8(fireworks::ROCKET_VARIANTS.wrapping_add(n));
    o.set_u8(x.b, Obj::VARIANT, variant);
    let delay = x.rom16(fireworks::ROCKET_DELAYS.wrapping_add(2 * n));
    x.set16(fireworks::DELAY, delay);
    let px = x.a5_u16(fireworks::CAMERA_X).wrapping_add(x.rom16(fireworks::ROCKET_PLACES.wrapping_add(4 * n)));
    let pz = x.a5_u16(fireworks::CAMERA_Y).wrapping_add(x.rom16(fireworks::ROCKET_PLACES.wrapping_add(4 * n + 2)));
    o.set_u16(x.b, Obj::X, px);
    o.set_u16(x.b, Obj::Z, pz);
}

/// Step 14: flies away leaving a trail (type 50, variant 7) every five frames, up to the top
/// of the screen: the scene ends (`A5+0x2c` = 0x10000000).
pub(super) fn hero_lift_off<R: Bench>(x: &mut Cx<'_, R>) {
    if countdown(x, fireworks::TIMER) <= 0 {
        x.set16(fireworks::TIMER, 5);
        if let Some(o) = x.alloc(&pools::EFFCT) {
            o.set_u8(x.b, Obj::ACTIVE, 1);
            o.set_u8(x.b, Obj::KIND, 0x32);
            o.set_u8(x.b, Obj::VARIANT, 7);
            o.set_u16(x.b, fireworks::SCENERY, x.o.0 as u16);
            let facing = x.u8(f::FACING);
            o.set_u8(x.b, f::FACING, facing);
            for off in [Obj::X, Obj::Z, Obj::Y] {
                let v = x.u16(off);
                o.set_u16(x.b, off, v);
            }
            let y = o.u16(x.b, Obj::Y).wrapping_add(1);
            o.set_u16(x.b, Obj::Y, y);
            let frame = x.u32(0x34);
            o.set_u32(x.b, 0x34, frame);
            for off in [0x3cu32, 0x3f] {
                let v = x.u8(off);
                o.set_u8(x.b, off, v);
            }
            o.set_u16(x.b, 0x30, 0xe);
        }
    }
    x.move_xz();
    let top = x.a5_u16(fireworks::CAMERA_Y).wrapping_add(0x100);
    let height = x.u16(Obj::Y).wrapping_add(x.u16(Obj::Z));
    if height as i16 >= top as i16 {
        x.add8(f::SUB_STATE, 2);
        x.set_a5_u32(0x2c, 0x1000_0000);
        set_hidden(x, true);
    }
}

crate::steps! {
    /// Resumptions of the game task when the fireworks (type 80) made it wait (`hero_rockets`); the
    /// context returned by the core has the object in A0.
    pub enum Step {
        /// After the four-frame wait: fade back.
        AfterWait = 0xb91d0 => |x| {
            crate::dispatch::resume_handler(x.b, None, |b, a| fireworks_fade_back(&mut Gx::dispatch(b, a)));
            crate::modes::Flow::Done
        },
        /// Return from the jump through the state table (`jsr (table,PC)`): end of the handler.
        AfterState = 0xb8f04 => |x| {
            crate::dispatch::resume_handler(x.b, None, |b, a| fireworks_tail(&mut Gx::dispatch(b, a)));
            crate::modes::Flow::Done
        },
    }
}

