use super::*;

const KILL: u32 = 0x0600_0000;
const BADGE_VARIANT: u8 = 1;
const SOUND: crate::sound::Sound = crate::sound::CMD_356_456;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(0x3) == 0 {
        parent(x)
    } else {
        child(x)
    }
}

/// Creates a SOBJ object of type 11 of variant `variant` and parameter `param` (`+0x2c`).
fn spawn<R: Bench>(x: &mut Cx<'_, R>, variant: u8, param: u8) -> Option<Obj> {
    let o = x.alloc(&pools::SOBJ)?;
    o.set_u8(x.b, 0, 1);
    o.set_u8(x.b, 2, 0xb);
    o.set_u8(x.b, 3, variant);
    o.set_u8(x.b, 0x2c, param);
    Some(o)
}

fn parent<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => {
            x.add8(0x4, 2);
            x.set16(0x18, 0x100);
            x.set16(0x14, 0x158);
            if x.u8(0x2c) != 0 {
                x.set16(0x14, 0x28);
            }
            x.set32(0x10, 0xffff_ffff);
            let camera = camera_record(x);
            x.set16(0x42, camera);
            stick_to_camera(x);
            x.anim_start(ANIMS, 0x15c);
            x.reinsert();
        }
        2 => match x.u8(0x5) {
            0 => {
                let z = x.u16(0x18).wrapping_sub(3);
                x.set16(0x18, z);
                if z as i16 > 0x80 {
                    return stick_to_camera(x);
                }
                x.set16(0x18, 0x80);
                let param = x.u8(0x2c);
                match spawn(x, BADGE_VARIANT, param) {
                    Some(child) => {
                        let me = x.o.0 as u16;
                        child.set_u16(x.b, 0x46, me);
                        x.set8(0x48, 0);
                    }
                    None => x.set8(0x48, 0x78),
                }
                x.add8(0x5, 2);
                stick_to_camera(x);
                x.set8(0x44, 0);
                x.anim_start(ANIMS, 0x15d);
            }
            2 => {
                stick_to_camera(x);
                let wait = x.u8(0x48);
                if wait != 0 {
                    let n = wait - 1;
                    x.set8(0x48, n);
                    if n == 0 {
                        return back_to_idle(x);
                    }
                } else if x.u8(0x44) != 0 {
                    return back_to_idle(x);
                }
                x.anim_step();
            }
            4 => {
                let z = x.u16(0x18).wrapping_add(3);
                x.set16(0x18, z);
                if (z as i16) < 0x110 {
                    return stick_to_camera(x);
                }
                x.add8(0x4, 2);
            }
            _ => {}
        },
        4 => {
            x.set32(0x4, KILL);
            x.free(&pools::SOBJ);
        }
        _ => x.free(&pools::SOBJ),
    }
}

fn back_to_idle<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x5, 2);
    x.anim_start(ANIMS, 0x15c);
}

fn child<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => {
            x.add8(0x4, 2);
            let owner = x.linked(0x46);
            let counter = owner.u8(x.b, 0x33);
            x.set8(0x49, counter);
            x.set_bit(0x1, 1, true);
            x.set8(0x48, 3);
            x.set8(0x44, 0x14);
            x.set8(0x45, 1);
            x.set16(0x18, 0x78);
            x.set16(0x14, 0x152);
            if x.u8(0x2c) != 0 {
                x.set8(0x2c, 0);
                x.set16(0x14, 0x2e);
            }
            x.set32(0x10, 0xffff_fffe);
            let camera = camera_record(x);
            x.set16(0x42, camera);
            stick_to_camera(x);
            x.anim_start(ANIMS, 0x15b);
            x.reinsert();
        }
        2 => {
            stick_to_camera(x);
            let owner = x.linked(0x46);
            let counter = owner.u8(x.b, 0x33);
            if counter == x.u8(0x49) {
                return;
            }
            x.set8(0x49, counter);
            if counter != 0 {
                x.set_bit(0x1, 1, false);
                return crate::sound::play(x.b, SOUND, x.o);
            }
            x.set_bit(0x1, 1, true);
            let n = x.u8(0x48).wrapping_sub(1);
            x.set8(0x48, n);
            if n != 0 {
                return;
            }
            owner.set_u8(x.b, 0x44, 1);
            x.set32(0x4, KILL);
            x.free(&pools::SOBJ);
        }
        4 => {
            x.set32(0x4, KILL);
            x.free(&pools::SOBJ);
        }
        _ => x.free(&pools::SOBJ),
    }
}
