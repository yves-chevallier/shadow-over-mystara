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
