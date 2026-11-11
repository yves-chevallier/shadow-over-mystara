use super::*;
use crate::world::data::RomAddr;
use crate::world::ram;

const ANIMS: RomAddr = RomAddr(0x1717be);
const SET: u32 = 0x5c;
const TIMER: u32 = 0x42;
const LIMITS: [u16; 2] = [0x500, 0x650];

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    let variant = x.u8(Obj::VARIANT);
    match x.u8(Obj::STATE) {
        0 => init(x, variant),
        2 => {
            let limit = LIMITS[(variant != 0) as usize];
            if (ram::CAMERA_X.get(x.b) as i16) < limit as i16 {
                if variant == 0 {
                    x.anim_step();
                } else {
                    wait_and_animate(x);
                }
                return;
            }
            let set = x.linked(SET);
            set.set_u8(x.b, Base::SETTING, 0);
            x.add8(Obj::STATE, 2);
            if variant == 0 {
                x.anim_step();
            }
        }
        _ => x.free_eshl(),
    }
}

fn init<R: Bench>(x: &mut Cx<'_, R>, variant: u8) {
    x.add8(Obj::STATE, 2);
    if let Some(set) = x.alloc(&pools::SET) {
        set.set_u8(x.b, Obj::ACTIVE, 1);
        set.set_u8(x.b, Obj::KIND, 7);
        set.set_u8(x.b, Obj::VARIANT, if variant == 0 { 0x2c } else { 0x2e });
        set.set_u8(x.b, Base::SETTING, 1);
        x.set16(SET, set.0 as u16);
    }
    if variant != 0 {
        x.set16(TIMER, 4);
    }
    x.set8(Base::BOX_SET, 7);
    x.set8(Shot::ATTACK_ID, if variant == 0 { 0x34 } else { 0x35 });
    x.anim_start(ANIMS.0, ((variant != 0) as u32) as u16);
    x.reinsert();
}

fn wait_and_animate<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Base::SUB_STATE) == 0 {
        x.add16(TIMER, 0xffff);
        if x.u16(TIMER) as i16 >= 0 {
            return;
        }
        x.add8(Base::SUB_STATE, 2);
        x.anim_start(ANIMS.0, 1);
    } else if x.anim_step() {
        x.add8(Base::SUB_STATE, 0xfe);
        x.set16(TIMER, 4);
    }
}
