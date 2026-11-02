use super::*;

const ANIMS: u32 = 0x16d1d6;
const OFFSETS: u32 = 0x9ed64;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => {
            if x.u16(f::OWNER) == 0 {
                return;
            }
            x.add8(0x4, 2);
            x.set8(f::BOX_SET, 7);
            x.set8(f::ATTACK_POWER, 6);
            x.set8(f::ATTACK_ID, 0x1c);
            let owner = x.linked(f::OWNER);
            copy_position(x, owner);
            x.add16(0x10, 0xffff);
            let row = OFFSETS + 4 * x.u8(0x3) as u32;
            let (dx, dy) = (x.rom16(row), x.rom16(row + 2));
            x.add16(0x8, dx);
            x.add16(0x10, dy);
            x.anim_start(ANIMS, 0);
            x.reinsert();
            wait_end(x);
        }
        2 => wait_end(x),
        _ => x.free_eshl(),
    }
}

fn wait_end<R: Bench>(x: &mut Cx<'_, R>) {
    if x.anim_step() {
        x.add8(0x4, 2);
    }
}
