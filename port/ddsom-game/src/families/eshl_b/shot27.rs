use super::*;

const ANIMS: u32 = 0x16ec24;
const SOUND_END: crate::sound::Sound = crate::sound::CMD_337;
const SOUND_ROW: crate::sound::Sound = crate::sound::CMD_32D;
const ROW_AT: u16 = 0x50;
const RISE: u8 = 0;
const FADE: u8 = 2;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => init(x),
        2 => active(x),
        4 => x.add8(0x4, 2),
        _ => x.free_eshl(),
    }
}

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(0x3) == 0 {
        x.add8(0x4, 2);
        let source = x.linked(0x5c);
        let (sx, sy) = (source.u16(x.b, 0x8), source.u16(x.b, 0x10));
        x.set16(0x8, sx);
        x.set16(0x10, sy);
        x.set16(0xc, 0x20);
        x.add16(0x10, 0xffff);
        x.set16(0x58, 0x60);
    } else {
        x.add16(0x58, 0xffff);
        if x.u16(0x58) != 0 {
            return;
        }
        x.add8(0x4, 2);
    }
    x.set8(f::BOX_SET, 7);
    x.set8(f::ATTACK_ID, 0x16);
    let anim = (x.u8(0x3) != 0) as u32;
    x.anim_start(ANIMS, anim as u16);
    x.reinsert();
}

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(0x3) == 0 {
        x.add16(0x58, 0xffff);
        if x.u16(0x58) == 0 {
            x.add8(0x4, 2);
            x.set8(0x0, 2);
            crate::sound::play(x.b, SOUND_END, x.o);
        }
        x.anim_step();
        if x.u16(0x58) == ROW_AT {
            start_row(x);
            crate::sound::play(x.b, SOUND_ROW, x.o);
        }
        return;
    }
    match x.u8(0x5) {
        RISE => {
            x.anim_step();
            let source = x.linked(0x5c);
            if source.u16(x.b, 0x58) <= 0x30 {
                x.add8(0x5, 2);
                x.anim_start(ANIMS, 2);
            }
            flicker(x);
        }
        FADE => {
            if x.anim_step() {
                x.add8(0x4, 2);
            }
            flicker(x);
        }
        _ => {}
    }
}

/// Hidden every other frame according to the frame counter `A5+0x6f`.
fn flicker<R: Bench>(x: &mut Cx<'_, R>) {
    let hidden = x.a5_u8(0x6f) & 1 == 0;
    x.set_bit(0x1, 1, hidden);
}

/// Creates the row of elements (variant 1) on either side, from the epicenter's creator
/// or the camera edge, up to the screen limits.
fn start_row<R: Bench>(x: &mut Cx<'_, R>) {
    let px = x.u16(0x8);
    let limit = if x.u8(0x56) == 0 {
        let target = x.linked(f::OWNER);
        target.u16(x.b, 0x8)
    } else {
        camera_x(x).wrapping_add(0xc0)
    };
    let rightwards = px <= limit;
    let start = if rightwards { px.wrapping_add(0x20) } else { px.wrapping_sub(0x20) };
    spawn_row(x, 0x1b, start, rightwards);
}
