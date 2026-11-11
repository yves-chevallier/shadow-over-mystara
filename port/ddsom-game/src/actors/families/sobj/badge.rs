use super::*;
use crate::actors::families::shell::records::{Base, view};

const KILL: u32 = 0x0600_0000;
const BADGE_VARIANT: u8 = 1;
const SOUND: crate::hw::sound::Sound = crate::hw::sound::CMD_356_456;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Obj::VARIANT) == 0 {
        parent(x)
    } else {
        child(x)
    }
}

/// Creates a SOBJ object of type 11 of variant `variant` and parameter `param` (`+0x2c`).
fn spawn<R: Bench>(x: &mut Cx<'_, R>, variant: u8, param: u8) -> Option<Obj> {
    let o = x.alloc(&pools::SOBJ)?;
    o.set_u8(x.b, Obj::ACTIVE, 1);
    o.set_u8(x.b, Obj::KIND, 0xb);
    o.set_u8(x.b, Obj::VARIANT, variant);
    o.set_u8(x.b, Base::FLIP, param);
    Some(o)
}

fn parent<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => {
            x.add8(Obj::STATE, 2);
            x.set16(Special::CAMERA_DZ, 0x100);
            x.set16(Special::CAMERA_DX, 0x158);
            if x.u8(Base::FLIP) != 0 {
                x.set16(Special::CAMERA_DX, 0x28);
            }
            x.set32(Obj::Y, 0xffff_ffff);
            let camera = camera_record(x);
            x.set16(Badge::CAMERA, camera);
            stick_to_camera(x);
            x.anim_start(ANIMS.0, 0x15c);
            x.reinsert();
        }
        2 => match x.u8(Base::SUB_STATE) {
            0 => {
                let z = x.u16(Special::CAMERA_DZ).wrapping_sub(3);
                x.set16(Special::CAMERA_DZ, z);
                if z as i16 > 0x80 {
                    return stick_to_camera(x);
                }
                x.set16(Special::CAMERA_DZ, 0x80);
                let param = x.u8(Base::FLIP);
                match spawn(x, BADGE_VARIANT, param) {
                    Some(child) => {
                        let me = x.o.0 as u16;
                        child.set_u16(x.b, Badge::OWNER, me);
                        x.set8(Badge::WAIT, 0);
                    }
                    None => x.set8(Badge::WAIT, 0x78),
                }
                x.add8(Base::SUB_STATE, 2);
                stick_to_camera(x);
                x.set8(Badge::FLASH, 0);
                x.anim_start(ANIMS.0, 0x15d);
            }
            2 => {
                stick_to_camera(x);
                let wait = x.u8(Badge::WAIT);
                if wait != 0 {
                    let n = wait - 1;
                    x.set8(Badge::WAIT, n);
                    if n == 0 {
                        return back_to_idle(x);
                    }
                } else if x.u8(Badge::FLASH) != 0 {
                    return back_to_idle(x);
                }
                x.anim_step();
            }
            4 => {
                let z = x.u16(Special::CAMERA_DZ).wrapping_add(3);
                x.set16(Special::CAMERA_DZ, z);
                if (z as i16) < 0x110 {
                    return stick_to_camera(x);
                }
                x.add8(Obj::STATE, 2);
            }
            _ => {}
        },
        4 => {
            x.set32(Obj::STATE, KILL);
            x.free(&pools::SOBJ);
        }
        _ => x.free(&pools::SOBJ),
    }
}

fn back_to_idle<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Base::SUB_STATE, 2);
    x.anim_start(ANIMS.0, 0x15c);
}

fn child<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => {
            x.add8(Obj::STATE, 2);
            let owner = x.linked(Badge::OWNER);
            let counter = owner.u8(x.b, Base::ANIM_EVENT);
            x.set8(Badge::COUNTER, counter);
            x.set_bit(Obj::FLAGS, 1, true);
            x.set8(Badge::WAIT, 3);
            x.set8(Badge::FLASH, 0x14);
            x.set8(Badge::FLASHING, 1);
            x.set16(Special::CAMERA_DZ, 0x78);
            x.set16(Special::CAMERA_DX, 0x152);
            if x.u8(Base::FLIP) != 0 {
                x.set8(Base::FLIP, 0);
                x.set16(Special::CAMERA_DX, 0x2e);
            }
            x.set32(Obj::Y, 0xffff_fffe);
            let camera = camera_record(x);
            x.set16(Badge::CAMERA, camera);
            stick_to_camera(x);
            x.anim_start(ANIMS.0, 0x15b);
            x.reinsert();
        }
        2 => {
            stick_to_camera(x);
            let owner = x.linked(Badge::OWNER);
            let counter = owner.u8(x.b, Base::ANIM_EVENT);
            if counter == x.u8(Badge::COUNTER) {
                return;
            }
            x.set8(Badge::COUNTER, counter);
            if counter != 0 {
                x.set_bit(Obj::FLAGS, 1, false);
                return crate::hw::sound::play(x.b, SOUND, x.o);
            }
            x.set_bit(Obj::FLAGS, 1, true);
            let n = x.u8(Badge::WAIT).wrapping_sub(1);
            x.set8(Badge::WAIT, n);
            if n != 0 {
                return;
            }
            owner.set_u8(x.b, Badge::FLASH, 1);
            x.set32(Obj::STATE, KILL);
            x.free(&pools::SOBJ);
        }
        4 => {
            x.set32(Obj::STATE, KILL);
            x.free(&pools::SOBJ);
        }
        _ => x.free(&pools::SOBJ),
    }
}

view! {
    /// Fields of the badge (type 11).
    Badge {
        /// Camera it sticks to (16-bit pointer).
        camera, set_camera, CAMERA: u16 @ 0x42;
        /// Flash countdown (h).
        flash, set_flash, FLASH: u8 @ 0x44;
        /// Non-zero while flashing (h).
        flashing, set_flashing, FLASHING: u8 @ 0x45;
        /// Owner (16-bit pointer).
        owner, set_owner, OWNER: u16 @ 0x46;
        /// Wait countdown (h).
        wait, set_wait, WAIT: u8 @ 0x48;
        /// Last value of the counter shown (h).
        counter, set_counter, COUNTER: u8 @ 0x49;
    }
}
