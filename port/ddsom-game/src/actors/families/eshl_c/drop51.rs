use super::*;
use crate::world::data::RomAddr;
use crate::actors::families::shell::records::Host;

const ANIMS: RomAddr = RomAddr(0x16ea18);
const LAUNCHER: u32 = 0x5c;
const WAIT: u32 = 0x42;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => {
            if wait(x) {
                return;
            }
            x.add8(Obj::STATE, 2);
            x.set16(WAIT, 0x1e);
            x.set8(Base::BOX_SET, 7);
            x.set8(Shot::ATTACK_ID, 0x17);
            let top = camera(x.b).1.get(x.b);
            let depth = x.u16(Obj::Y);
            x.set16(Obj::Z, top.wrapping_add(0x100).wrapping_sub(depth));
            x.anim_start(ANIMS.0, 0);
            x.reinsert();
        }
        2 => {
            if x.u8(Base::SUB_STATE) == 0 {
                if wait(x) {
                    return;
                }
                x.add8(Base::SUB_STATE, 2);
                x.set16(Obj::VZ, neg(0x100));
                x.set16(Obj::AZ, neg(0x40));
                return;
            }
            x.anim_step();
            x.move_z();
            if s16(x.u16(Obj::Z)) >= 0 {
                if x.u8(Shot::HIT_FLAGS) & 0x8 != 0 {
                    x.set8(Shot::HIT_FLAGS, 0);
                    let launcher = x.linked(LAUNCHER);
                    let n = launcher.u8(x.b, Host::SHOTS).wrapping_add(1);
                    launcher.set_u8(x.b, Host::SHOTS, n);
                }
                return;
            }
            x.add8(Obj::STATE, 2);
            crate::actors::families::effects_creators::debris(x.b, x.o, 3);
            crate::actors::camera_modes::start_shake(x.b, crate::actors::camera_modes::shake::B, 8);
        }
        4 => {
            x.add8(Obj::STATE, 2);
            if x.u8(Base::SETTING) == 0 {
                let launcher = x.linked(LAUNCHER);
                let slot = 8u8.wrapping_add(x.u8(Obj::VARIANT)) as u32;
                launcher.set_u16(x.b, slot, 0);
            }
        }
        _ => x.free_eshl(),
    }
}

/// Countdown of the delay `+0x42`: true as long as it has not elapsed.
fn wait<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let t = x.u16(WAIT).wrapping_sub(1);
    x.set16(WAIT, t);
    t != 0
}
