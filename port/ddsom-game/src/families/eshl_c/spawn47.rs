use super::*;

const ANIMS: u32 = 0x16fbf4;
/// Tables of 16 words of offsets: in X (inside the band / outside the band), in Y (same), delays.
const X_INSIDE: u32 = 0xa3fd4;
const X_OUTSIDE: u32 = 0xa3ff4;
const Y_INSIDE: u32 = 0xa4014;
const Y_OUTSIDE: u32 = 0xa4034;
const DELAYS: u32 = 0xa4068;
const CAMERA_X: u32 = 0x5a6;
const CAMERA_Y: u32 = 0x5aa;
const DELAY: u32 = 0x58;
const WAIT: u8 = 0;
const PLAY: u8 = 2;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    match x.u8(0x4) {
        0 => {
            x.add8(0x4, 2);
            x.set8(f::BOX_SET, 7);
            x.set8(f::ATTACK_POWER, 0x1e);
            x.set_bit(0x1, 1, true);
            place(x);
            let r = random(x);
            let delay = x.rom16(DELAYS + 2 * (r & 0xf));
            x.set16(DELAY, delay);
            ai_anim(x, ANIMS, 1);
        }
        2 => match x.u8(0x5) {
            WAIT => {
                let t = x.u16(DELAY).wrapping_sub(1);
                x.set16(DELAY, t);
                if s16(t) <= 0 {
                    x.add8(0x5, 2);
                    x.set_bit(0x1, 1, false);
                }
            }
            PLAY => {
                if ai_step(x) {
                    x.add8(0x4, 2);
                }
            }
            _ => {}
        },
        _ => x.free_eshl(),
    }
    if x.u8(0) != 0 {
        x.reinsert();
    }
}

/// Offsets the position at random: brings it back toward the screen if it is too far from it.
fn place<R: Bench>(x: &mut Cx<'_, R>) {
    let idx = 2 * (random(x) & 0xf);
    let cam = x.a5_u16(CAMERA_X);
    let (lo, hi) = (cam.wrapping_add(0x20), cam.wrapping_add(0x160));
    let px = x.u16(0x8);
    let dx = if s16(lo) >= s16(px) {
        x.rom16(X_OUTSIDE + idx)
    } else if s16(hi) <= s16(px) {
        neg(x.rom16(X_OUTSIDE + idx))
    } else {
        x.rom16(X_INSIDE + idx)
    };
    x.add16(0x8, dx);
    let idx = 2 * (random(x) & 0xf);
    let cam = x.a5_u16(CAMERA_Y);
    let (lo, hi) = (cam.wrapping_add(0x38), cam.wrapping_add(0x68));
    let py = x.u16(0x10);
    let dy = if s16(lo) >= s16(py) {
        x.rom16(Y_OUTSIDE + idx)
    } else if s16(hi) <= s16(py) {
        neg(x.rom16(Y_OUTSIDE + idx))
    } else {
        x.rom16(Y_INSIDE + idx)
    };
    x.add16(0x10, dy);
}
