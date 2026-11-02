use super::*;

const PALETTE: u32 = 0x28ff20;
const PALETTE_DST: u32 = 0x91c380;
const FIRST_ADVICE: u32 = 0x118;
const LAST_ADVICE: u16 = 0x136;
const ADVICE_FRAMES: u8 = 0x50;
const LEAVE_SOUND: crate::sound::Sound = crate::sound::CMD_137_237;

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
