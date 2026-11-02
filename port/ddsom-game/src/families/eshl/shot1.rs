use super::*;

const ANIMS: u32 = 0x16c946;
/// Speed table (16 bytes per variant: VX AX VZ AZ VY AY).
const SPEEDS: u32 = 0x9dfe2;
const LOB: u8 = 0;
const LANDED: u8 = 2;
const BLINK: u8 = 4;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => {
            x.set8(f::BOX_SET, 7);
            x.set8(f::ATTACK_ID, 8);
            x.set8(f::ATTACK_POWER, 5);
            x.add8(0x4, 2);
            if x.u8(0x2f) != 0 {
                burst(x);
            } else {
                lob(x);
            }
            x.anim_start(ANIMS, 0);
            x.reinsert();
            body(x);
        }
        2 => body(x),
        _ => x.free_eshl(),
    }
}

/// Thrown by the owner: in front of it, horizontal speed 2.5 pixels per frame.
fn lob<R: Bench>(x: &mut Cx<'_, R>) {
    let owner = x.linked(f::OWNER);
    place_in_front(x, owner, 0x40, 0x20);
    let vx = if x.u8(f::FLIP) != 0 { 0x280 } else { -0x280i16 as u16 };
    x.set16(0x14, vx);
    x.set16(0x16, 0);
    x.set16(0x18, 0x300);
    x.set16(0x1a, -0x38i16 as u16);
    x.set16(0x1c, 0);
    x.set16(0x1e, 0);
}

/// Spring-out: position of the owner, speeds of row `+3` of the table.
fn burst<R: Bench>(x: &mut Cx<'_, R>) {
    let owner = x.linked(f::OWNER);
    copy_position(x, owner);
    x.add16(0x10, 0xffff);
    x.add16(0xc, 0x32);
    let row = SPEEDS + 16 * x.u8(0x3) as u32;
    for i in 0..6 {
        let v = x.rom16(row + 2 * i);
        x.set16(0x14 + 2 * i, v);
    }
}

fn body<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x5) {
        LOB => {
            if !x.move_xyz().gt() {
                x.add8(0x5, 2);
                x.set16(f::TIMER, 0x50);
                x.set32(0xc, 0);
                crate::sound::play(x.b, crate::sound::CMD_320_420, x.o);
                x.anim_start(ANIMS, 1);
            }
        }
        LANDED => {
            if x.u8(0x33) != 0 {
                x.set8(0x33, 0);
                crate::sound::play(x.b, crate::sound::CMD_321_421, x.o);
            }
            let t = x.u16(f::TIMER).wrapping_sub(1);
            x.set16(f::TIMER, t);
            if t != 0 {
                x.anim_step();
            } else {
                x.add8(0x5, 2);
                x.set16(f::TIMER, 0x50);
                x.anim_start(ANIMS, 2);
            }
        }
        BLINK => {
            x.anim_step();
            let t = x.u16(f::TIMER).wrapping_sub(1);
            x.set16(f::TIMER, t);
            if t == 0 {
                x.add8(0x4, 2);
            }
            blink(x);
        }
        _ => {}
    }
    finish(x);
}

/// End of the handler when the state is unchanged: sorted list then removal off the screen.
fn finish<R: Bench>(x: &mut Cx<'_, R>) {
    x.reinsert();
    x.cull_offscreen();
}
