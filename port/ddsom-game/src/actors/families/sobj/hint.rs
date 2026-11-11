use super::*;
use crate::actors::families::shell::vars;
use crate::world::ram;
use crate::actors::families::shell::records::{Base, view};

const PALETTE: RomAddr = RomAddr(0x28ff20);
const PALETTE_DST: u32 = 0x91c380;
const FIRST_ADVICE: u32 = 0x118;
const LAST_ADVICE: u16 = 0x136;
const ADVICE_FRAMES: u8 = 0x50;
const LEAVE_SOUND: crate::services::sound::Sound = crate::services::sound::CMD_137_237;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    let owner = x.linked(Hint::OWNER);
    if owner.u8(x.b, Hint::OWNER_QUIET) == 0 && rd8(x.b, HINT_INHIBIT) != 0 {
        x.set32(Obj::STATE, 0);
        return;
    }
    match x.u8(Obj::STATE) {
        0 => start(x),
        2 => step(x),
        4 | 6 => leave(x),
        _ => {}
    }
}

fn start<R: Bench>(x: &mut Cx<'_, R>) {
    if vars::GAME_PAUSED.get(x.b) != 0 {
        return;
    }
    x.add8(Obj::STATE, 2);
    load_palette(x);
    x.set8(Hint::ADVICE_TIMER, ADVICE_FRAMES);
    x.set16(Hint::ADVICE, FIRST_ADVICE as u16);
    x.anim_start(ANIMS.0, FIRST_ADVICE as u16);
}

/// Bubble palette, maximum brightness; the object becomes visible (`+0x2a`, `+0x2b`).
fn load_palette<R: Bench>(x: &mut Cx<'_, R>) {
    copy_longs(x, PALETTE, PALETTE_DST, 8);
    or_longs(x, PALETTE_DST, BRIGHT, 8);
    x.set8(Base::OWN_PALETTE, 1);
    x.set8(Base::PALETTE, 0x1c);
}

fn step<R: Bench>(x: &mut Cx<'_, R>) {
    if vars::GAME_PAUSED.get(x.b) != 0 {
        return;
    }
    if x.u8(Hint::WAITING) != 0 {
        load_palette(x);
    }
    x.set8(Hint::WAITING, 0);
    if x.u16(Hint::OWNER) == 0 {
        return x.set16(Obj::STATE, LEAVE_6_2);
    }
    let owner = x.linked(Hint::OWNER);
    if owner.u8(x.b, Hint::OWNER_FLAGS) & 2 != 0 {
        crate::services::sound::play(x.b, LEAVE_SOUND, x.o);
        // 0xdf896: SOBJ 0xa linked through `+0x46`
        crate::actors::families::small::linked_via(&mut Pure, x.b, x.o, &pools::SOBJ, 0xa, 0x46);
        let owner = x.linked(Hint::OWNER);
        let v = owner.u8(x.b, Hint::OWNER_FLAGS) & !2;
        owner.set_u8(x.b, Hint::OWNER_FLAGS, v);
        return x.set16(Obj::STATE, LEAVE_6_2);
    }
    match x.u8(Base::SUB_STATE) {
        0 => countdown(x),
        2 => next_advice(x),
        _ => {}
    }
    x.reinsert();
}

/// Substate 0: the advice stays displayed for `+0x16` frames, then moves on to the next (substate 2)
/// or, after the last, cedes control to the linked object.
fn countdown<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(Hint::OWNER) == 0 {
        return x.set16(Obj::STATE, LEAVE_6_2);
    }
    let n = x.u8(Hint::ADVICE_TIMER).wrapping_sub(1);
    x.set8(Hint::ADVICE_TIMER, n);
    if (n as i8) < 0 {
        if x.u16(Hint::ADVICE) != LAST_ADVICE {
            x.add8(Base::SUB_STATE, 2);
        } else {
            // the last piece of advice stays as long as the game is in state 0xc or 6
            let mode = ram::SCENE.get(x.b);
            if mode == 0xc || mode == 6 {
                return x.set8(Hint::ADVICE_TIMER, ADVICE_FRAMES);
            }
            x.set16(Obj::STATE, LEAVE_6_2);
            let owner = x.linked(Hint::OWNER);
            let v = owner.u8(x.b, Hint::OWNER_FLAGS) | 1;
            owner.set_u8(x.b, Hint::OWNER_FLAGS, v);
            return;
        }
    }
    follow(x);
}

fn next_advice<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Base::SUB_STATE, 0u8.wrapping_sub(2));
    x.set8(Hint::ADVICE_TIMER, ADVICE_FRAMES);
    let n = x.u16(Hint::ADVICE).wrapping_add(1);
    x.set16(Hint::ADVICE, n);
    x.anim_start(ANIMS.0, (n as u32) as u16);
    follow(x);
}

/// The bubble follows the camera (X + 0xb0, Z + 0xcf) and advances its animation.
fn follow<R: Bench>(x: &mut Cx<'_, R>) {
    let v = ram::CAMERA_X.get(x.b).wrapping_add(0xb0);
    x.set16(Obj::X, v);
    let v = ram::CAMERA_Y.get(x.b).wrapping_add(0xcf);
    x.set16(Obj::Z, v);
    x.set32(Obj::Y, 0);
    x.anim_step();
}

fn leave<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Base::SUB_STATE) != 0 {
        return x.free(&pools::SOBJ);
    }
    x.set16(Obj::STATE, 0x200);
    load_palette(x);
}

view! {
    /// Fields of the advice bubble.
    Hint {
        /// Number of the advice shown.
        advice, set_advice, ADVICE: u16 @ 0x14;
        /// Frames left for the current advice.
        advice_timer, set_advice_timer, ADVICE_TIMER: u8 @ 0x16;
        /// Non-zero: waits (h).
        waiting, set_waiting, WAITING: u8 @ 0x40;
        /// Advice flags of the owner (bits 0 and 1) (h).
        owner_flags, set_owner_flags, OWNER_FLAGS: u8 @ 0x42;
        /// Zero: the owner allows the inhibition of the hints (h).
        owner_quiet, set_owner_quiet, OWNER_QUIET: u8 @ 0x43;
        /// Owner (16-bit pointer).
        owner, set_owner, OWNER: u16 @ 0x46;
    }
}
