use super::*;

const ANIMS: u32 = 0x16ccea;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => {
            if x.u16(f::OWNER) == 0 {
                // 0x9ee16: without an owner, the effect goes to state 6
                x.set32(0x4, 0x0600_0000);
                return;
            }
            x.add8(0x4, 2);
            let owner = x.linked(f::OWNER);
            x.set8(f::BOX_SET, 7);
            x.set8(f::ATTACK_POWER, 6);
            x.set8(f::ATTACK_ID, 0xe);
            place_in_front(x, owner, 0x46, 0x12);
            x.anim_start(ANIMS, 2);
            body(x);
        }
        2 => body(x),
        _ => x.free_eshl(),
    }
}

const THRUST: u8 = 0;
const RECOVER: u8 = 2;

fn body<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x5) {
        THRUST => {
            x.move_xyz();
            x.cull_offscreen();
            if x.u8(0x33) != 0 {
                x.add8(0x5, 2);
            }
            x.anim_step();
        }
        RECOVER => {
            if x.anim_step() {
                x.set8(0x4, st::FREE);
            }
        }
        _ => {}
    }
    x.keep_in_zone();
    x.push_terrain();
    x.reinsert();
}
