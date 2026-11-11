use super::*;
use crate::world::data::RomAddr;
use crate::actors::families::shell::records::Host;

const ANIMS: RomAddr = RomAddr(0x17041e);
const LAUNCHER: u32 = 0x5c;
const WAIT: u32 = 0x4c;
const HOME_Z: u32 = 0x4e;
const TARGET: u32 = 0x58;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => {
            x.add8(Obj::STATE, 2);
            x.set16(WAIT, 0x10);
            let z = x.u16(Obj::Z);
            x.set16(HOME_Z, z);
            x.set8(Base::BOX_SET, 7);
            x.set8(Shot::ATTACK_ID, 0x17);
            x.anim_start(ANIMS.0, 1);
            x.reinsert();
        }
        2 => {
            sub_state(x);
            if x.u8(Shot::HIT_FLAGS) & 0x8 != 0 {
                x.set8(Shot::HIT_FLAGS, 0);
                let launcher = x.linked(LAUNCHER);
                let n = launcher.u8(x.b, Host::SHOTS).wrapping_add(1);
                launcher.set_u8(x.b, Host::SHOTS, n);
            }
        }
        4 => {
            x.add8(Obj::STATE, 2);
            let launcher = x.linked(LAUNCHER);
            let slot = (8u8.wrapping_add(x.u8(Obj::VARIANT))) as u32;
            launcher.set_u16(x.b, slot, 0);
        }
        _ => x.free_eshl(),
    }
}

fn sub_state<R: Bench>(x: &mut Cx<'_, R>) {
            match x.u8(Base::SUB_STATE) {
                0 => {
                    if dec_wait(x) {
                        return;
                    }
                    x.add8(Base::SUB_STATE, 2);
                    x.set16(Obj::VZ, neg(0x100));
                    x.set16(Obj::AZ, neg(0x20));
                    // Original glitch: 0x54872 returns D0 = 0 even when the shot is created, so the `bne` that was
                    // supposed to hold it in `TARGET` is never taken
                    crate::actors::families::small::shot_18(&mut crate::actors::families::effects_creators::leftovers::Pure, x.b, x.o, 2);
                }
                2 => {
                    x.move_z();
                    if s16(x.u16(Obj::Z)) <= 0x20 {
                        x.add8(Base::SUB_STATE, 2);
                        x.set16(WAIT, 0x10);
                    }
                }
                4 => {
                    if dec_wait(x) {
                        return;
                    }
                    x.add8(Base::SUB_STATE, 2);
                    x.set16(Obj::VZ, 0x180);
                    x.set16(Obj::AZ, 0);
                }
                6 => {
                    x.move_z();
                    if s16(x.u16(HOME_Z)) > s16(x.u16(Obj::Z)) {
                        return;
                    }
                    x.add8(Obj::STATE, 2);
                    if x.u16(TARGET) != 0 {
                        let t = x.linked(TARGET);
                        t.set_u8(x.b, Obj::STATE, 6);
                    }
                }
                _ => {}
            }
}

/// Countdown of the delay `+0x4c`: true as long as it has not elapsed.
fn dec_wait<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let t = x.u16(WAIT).wrapping_sub(1);
    x.set16(WAIT, t);
    t != 0
}
