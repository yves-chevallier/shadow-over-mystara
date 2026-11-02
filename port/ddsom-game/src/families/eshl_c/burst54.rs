use super::*;

const ANIMS: u32 = 0x1706f6;
const LINK: u32 = 0x5c;
const TIMER: u32 = 0x42;
const SOUND: crate::sound::Sound = crate::sound::CMD_366;
/// Rows of 9 words: DX, DZ, DY, VX, AX, VZ, AZ, VY, AY.
const ROWS: u32 = 0xa4fe6;
/// Rows of 10 words: the same, then the animation.
const ROWS_ANIM: u32 = 0xa51b4;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(LINK) == 0 {
        return x.free_eshl();
    }
    match x.u8(0x4) {
        0 => {
            crate::sound::play(x.b, SOUND, x.o);
            x.add8(0x4, 2);
            x.set8(f::BOX_SET, 7);
            x.set8(f::ATTACK_ID, 0x23);
            let with_anim = x.u8(0x2f) != 0;
            launch(x, with_anim);
        }
        2 => {
            match x.u8(0x5) {
                0 => {
                    let t = x.u16(TIMER).wrapping_sub(1);
                    x.set16(TIMER, t);
                    if s16(t) < 0 {
                        x.add8(0x5, 2);
                        x.set16(TIMER, 0xa);
                    }
                    x.move_xyz();
                    x.anim_step();
                }
                2 => {
                    let t = x.u16(TIMER).wrapping_sub(1);
                    x.set16(TIMER, t);
                    if s16(t) < 0 {
                        x.add8(0x4, 2);
                    }
                    x.anim_step();
                }
                _ => {}
            }
            blink_by_variant(x);
            x.reinsert();
        }
        _ => x.free_eshl(),
    }
}

/// Places itself on the launcher and takes the speeds of the variant's row.
fn launch<R: Bench>(x: &mut Cx<'_, R>, with_anim: bool) {
    let src = x.linked(LINK);
    for off in [0x8, 0x10, 0xc] {
        let v = src.u32(x.b, off);
        x.set32(off, v);
    }
    copy_flip(x, src);
    x.add16(0x10, 0xffff);
    x.set16(TIMER, if with_anim { 0x1e } else { 0x3c });
    let flip = x.u8(f::FLIP) ^ 0x20;
    x.set8(f::FLIP, flip);
    let v = x.u8(0x3) as u32;
    let row = if with_anim { ROWS_ANIM + 20 * v } else { ROWS + 18 * v };
    let word = |x: &mut Cx<'_, R>, i: u32| x.rom16(row + 2 * i);
    let (mut dx, dz, dy, mut vx, mut ax) = (word(x, 0), word(x, 1), word(x, 2), word(x, 3), word(x, 4));
    for (i, off) in [0x18u32, 0x1a, 0x1c, 0x1e].into_iter().enumerate() {
        let w = word(x, 5 + i as u32);
        x.set16(off, w);
    }
    if flip != 0 {
        dx = neg(dx);
        vx = neg(vx);
        ax = neg(ax);
    }
    x.add16(0x8, dx);
    x.add16(0xc, dz);
    x.add16(0x10, dy);
    x.set16(0x14, vx);
    x.set16(0x16, ax);
    let anim = if with_anim { word(x, 9) as u32 } else { 0 };
    x.anim_start(ANIMS, anim as u16);
}
