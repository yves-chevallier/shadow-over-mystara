use super::*;

const ANIMS: u32 = 0x1717be;
const SET: u32 = 0x5c;
const TIMER: u32 = 0x42;
const CAMERA_X: u32 = 0x506;
const LIMITS: [u16; 2] = [0x500, 0x650];

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    let variant = x.u8(0x3);
    match x.u8(0x4) {
        0 => init(x, variant),
        2 => {
            let limit = LIMITS[(variant != 0) as usize];
            if (x.a5_u16(CAMERA_X) as i16) < limit as i16 {
                if variant == 0 {
                    x.anim_step();
                } else {
                    wait_and_animate(x);
                }
                return;
            }
            let set = x.linked(SET);
            set.set_u8(x.b, 0x2f, 0);
            x.add8(0x4, 2);
            if variant == 0 {
                x.anim_step();
            }
        }
        _ => x.free_eshl(),
    }
}

fn init<R: Bench>(x: &mut Cx<'_, R>, variant: u8) {
    x.add8(0x4, 2);
    if let Some(set) = x.alloc(&pools::SET) {
        set.set_u8(x.b, 0x0, 1);
        set.set_u8(x.b, 0x2, 7);
        set.set_u8(x.b, 0x3, if variant == 0 { 0x2c } else { 0x2e });
        set.set_u8(x.b, 0x2f, 1);
        x.set16(SET, set.0 as u16);
    }
    if variant != 0 {
        x.set16(TIMER, 4);
    }
    x.set8(f::BOX_SET, 7);
    x.set8(f::ATTACK_ID, if variant == 0 { 0x34 } else { 0x35 });
    x.anim_start(ANIMS, ((variant != 0) as u32) as u16);
    x.reinsert();
}

fn wait_and_animate<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(0x5) == 0 {
        x.add16(TIMER, 0xffff);
        if x.u16(TIMER) as i16 >= 0 {
            return;
        }
        x.add8(0x5, 2);
        x.anim_start(ANIMS, 1);
    } else if x.anim_step() {
        x.add8(0x5, 0xfe);
        x.set16(TIMER, 4);
    }
}
