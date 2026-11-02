use super::*;

const ANIMS: u32 = 0x16eec2;
const ATTACH_SHIFT: u32 = 0xa1fac;
const SOUND_START: crate::sound::Sound = crate::sound::CMD_32D;
const SOUND_END: crate::sound::Sound = crate::sound::CMD_337;
const SOURCE_ARMED: u32 = 0xaf;
const WAIT: u8 = 0;
const HOLD: u8 = 2;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => init(x),
        2 => active(x),
        4 => stop(x),
        _ => x.free_eshl(),
    }
}

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(f::ATTACK_ID, 6);
    x.set8(f::ATTACK_POWER, 5);
    x.set8(f::BOX_SET, 7);
    match x.u8(0x3) {
        2 => {
            x.add8(0x4, 2);
            follow(x);
            x.set16(0x58, 0x1e);
            x.anim_start(ANIMS, 3);
        }
        0 => {
            x.add8(0x4, 2);
            follow(x);
            x.anim_start(ANIMS, 0);
        }
        _ => {
            x.add16(0x58, 0xffff);
            if x.u16(0x58) != 0 {
                return;
            }
            x.add8(0x4, 2);
            x.anim_start(ANIMS, 1);
        }
    }
    x.reinsert();
}

/// End: the creator is dead or in state 0x20a.
fn stop<R: Bench>(x: &mut Cx<'_, R>) {
    crate::sound::play(x.b, SOUND_END, x.o);
    x.add8(0x4, 2);
}

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    let source = x.linked(0x5c);
    if (source.u8(x.b, 0x4) as i8) >= 4 || source.u16(x.b, 0x4) == 0x20a {
        return stop(x);
    }
    match x.u8(0x3) {
        2 => {
            follow(x);
            x.add16(0x58, 0xffff);
            if (x.u16(0x58) as i16) < 0 {
                x.add8(0x4, 2);
            }
            x.anim_step();
        }
        0 => centre(x),
        _ => satellite(x, source),
    }
}

/// Central element: waits for the creator's signal then triggers the row.
fn centre<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x5) {
        WAIT => {
            let source = x.linked(0x5c);
            if source.u8(x.b, SOURCE_ARMED) != 0 {
                crate::sound::play(x.b, SOUND_START, x.o);
                x.add8(0x5, 2);
                x.set16(0x58, 0x50);
                let px = source.u16(x.b, 0x8);
                let rightwards = source.u8(x.b, f::FLIP) != 0;
                let start = if rightwards { px.wrapping_add(0x4d) } else { px.wrapping_sub(0x4d) };
                spawn_row(x, 0x1d, start, rightwards);
            } else {
                follow(x);
                x.anim_step();
                x.reinsert();
            }
        }
        HOLD => {
            x.add16(0x58, 0xffff);
            if x.u16(0x58) == 0 {
                x.add8(0x4, 2);
                x.set8(0x5, 0);
                x.set8(0x0, 2);
                let source = x.linked(0x5c);
                source.set_u8(x.b, SOURCE_ARMED, 0);
            }
            follow(x);
            x.anim_step();
            x.reinsert();
        }
        _ => {}
    }
}

/// Row element: blinks according to `A5+0x6f`, changes animation when the epicenter goes out.
fn satellite<R: Bench>(x: &mut Cx<'_, R>, source: Obj) {
    let flicker = |x: &mut Cx<'_, R>| {
        let hidden = x.a5_u8(0x6f) & 1 == 0;
        x.set_bit(0x1, 1, hidden);
    };
    match x.u8(0x5) {
        WAIT => {
            x.anim_step();
            if source.u16(x.b, 0x58) <= 0x30 {
                x.add8(0x5, 2);
                x.anim_start(ANIMS, 2);
            }
            flicker(x);
        }
        HOLD => {
            if x.anim_step() {
                x.add8(0x4, 2);
            }
            flicker(x);
        }
        _ => {}
    }
}

/// Sticks to the creator; the offset depends on its attack state (`+7`).
fn follow<R: Bench>(x: &mut Cx<'_, R>) {
    let source = x.linked(0x5c);
    copy_position16(x, source);
    let flip = source.u8(x.b, f::FLIP);
    x.set8(f::FLIP, flip);
    let attack = source.u8(x.b, 0x7);
    if attack == 0x16 || attack == 0x18 {
        let index = source.u8(x.b, 0x33) as u32;
        let mut dx = x.rom16(ATTACH_SHIFT + index);
        if flip != 0 {
            dx = dx.wrapping_neg();
        }
        x.add16(0x8, dx);
        x.set16(0xc, 0x1e);
    } else {
        let mut dx = -0x2di16 as u16;
        if flip != 0 {
            dx = dx.wrapping_neg();
        }
        x.add16(0x8, dx);
        x.set16(0xc, 0x27);
    }
    x.add16(0x10, 0xffff);
}
