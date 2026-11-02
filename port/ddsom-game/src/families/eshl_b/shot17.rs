use super::*;

const ANIMS: u32 = 0x16e1f4;
const PALETTE_SRC: u32 = 0x25a660;
const PALETTE_DST: u32 = 0x91c300;
const PALETTE_TINT: u32 = 0x91c000;
/// Spawn offset (Z, X) by `+0x42` (4 bytes per row).
const SPAWN_OFFSETS: u32 = 0xa0670;
/// `+0x2d` according to the angle (Y) for mode 0 and mode > 0.
const AIM_TABLE_B: u32 = 0xa07ac;
const AIM_TABLE_A: u32 = 0xa0892;
/// Vertical and horizontal speed of mode < 0 (4 bytes per row).
const DROP_SPEEDS: u32 = 0xa0962;
const CHARGE: u8 = 0;
const AIM: u8 = 2;
const FALL: u8 = 4;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => init(x),
        2 => active(x),
        4 => {
            if x.anim_step() {
                x.add8(0x4, 2);
            }
        }
        _ => x.free_eshl(),
    }
}

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x4, 2);
    for i in 0..8 {
        let v = x.data32(PALETTE_SRC + 4 * i);
        wr32(x.b, PALETTE_DST + 4 * i, v);
    }
    let tint = rd32(x.b, PALETTE_TINT) & 0xf000_f000;
    for i in 0..8 {
        let v = rd32(x.b, PALETTE_DST + 4 * i) | tint;
        wr32(x.b, PALETTE_DST + 4 * i, v);
    }
    x.set8(0x2a, 1);
    x.set8(0x2b, 0x18);
    x.set8(0x54, 0x3c);
    let row = x.u8(0x42) as u32;
    let dz = x.rom16(SPAWN_OFFSETS + row);
    x.add16(0xc, dz);
    let mut dx = x.rom16(SPAWN_OFFSETS + 2 + row);
    if x.u8(f::FLIP) != 0 {
        dx = dx.wrapping_neg();
    }
    x.add16(0x8, dx);
    x.add32(0x10, 0xffff_ffff);
    x.set8(f::BOX_SET, 7);
    x.set8(f::ATTACK_POWER, 8);
    x.set8(f::ATTACK_ID, 2);
    x.anim_start(ANIMS, 0);
    x.reinsert();
}

/// The parent has left its normal states: the projectile disappears.
fn check_parent<R: Bench>(x: &mut Cx<'_, R>) {
    let parent = ptr(x, 0x56);
    if (parent.u8(x.b, 0x4) as i8) >= 4 {
        kill(x);
    }
}

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    let mode = x.u8(0x43);
    let state = x.u8(0x5);
    match state {
        CHARGE => charge(x),
        AIM => {
            if mode == 0 {
                aim_flat(x)
            } else if (mode as i8) < 0 {
                aim_drop(x)
            } else {
                aim_angle(x)
            }
        }
        FALL => {
            if mode == 0 {
                land(x, |x| { x.move_by_table_xz(); }, true)
            } else if (mode as i8) < 0 {
                land(x, |x| { x.move_xz(); }, true)
            } else {
                land_xyz(x)
            }
        }
        _ => {}
    }
    x.reinsert();
}

/// Charge: animation 0 for 60 frames.
fn charge<R: Bench>(x: &mut Cx<'_, R>) {
    let parent = ptr(x, 0x56);
    if (parent.u8(x.b, 0x4) as i8) >= 4 {
        return kill(x);
    }
    x.anim_step();
    x.add8(0x54, 0xff);
    if (x.u8(0x54) as i8) < 0 {
        x.add8(0x5, 2);
    }
    check_parent(x);
}

/// Start of the firing phase: announcement to the parent (`+0xa5`), 20-frame wait.
fn begin_aim<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if x.u8(0x6) == 0 {
        x.add8(0x6, 2);
        x.set8(0x54, 0x14);
        x.anim_start(ANIMS, 1);
        return false;
    }
    true
}

/// After the wait: notifies the parent, true when the projectile must leave.
fn ready_to_go<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    x.anim_step();
    x.add8(0x54, 0xff);
    if (x.u8(0x54) as i8) >= 0 {
        return false;
    }
    let parent = ptr(x, 0x56);
    parent.set_u8(x.b, 0xa5, 1);
    x.u8(0x55) != 0
}

/// Mode > 0: aims at the target (distance/altitude ratio, angle).
fn aim_angle<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(0x6) == 0 {
        x.add8(0x6, 2);
        x.set8(0x54, 0x14);
        x.anim_start(ANIMS, 1);
        return;
    }
    let parent = ptr(x, 0x56);
    if (parent.u8(x.b, 0x4) as i8) >= 4 {
        return kill(x);
    }
    if ready_to_go(x) {
        x.add8(0x5, 2);
        x.set8(0x2d, 0x10);
        if x.u8(0x3) == 8 {
            x.set32(0x14, 0);
            x.set32(0x18, 0xf800_0000);
            x.set32(0x1c, 0xfb80_0000);
        } else {
            let target = ptr(x, f::OWNER);
            if target.u8(x.b, 0) == 0 {
                return x.set8(0x4, st::FREE);
            }
            aim_at(x, target);
        }
    }
    check_parent(x);
}

/// Computes the vertical speed (slope toward the target) and the angle `+0x2d`.
fn aim_at<R: Bench>(x: &mut Cx<'_, R>, target: Obj) {
    let dy = x.u32(0x10).wrapping_sub(target.u32(x.b, 0x10));
    let mut dx = x.u32(0x8).wrapping_sub(target.u32(x.b, 0x8));
    if (dx as i32) < 0 {
        dx = dx.wrapping_neg();
    }
    let far = if (dx as i32) < (dy as i32) { dy } else { dx } >> 3;
    let quotient = crate::aids::divide(x.u32(0xc), far).0 << 7;
    let slope = (quotient as u16).wrapping_add(((far >> 16) as u16) << 2);
    x.set16(0x18, slope.wrapping_neg());
    let angle = crate::aids::direction_to(x.b, x.o, target) & 0x1f;
    let idx = angle.wrapping_sub(8) as u32;
    let v = x.rom8(AIM_TABLE_B + idx);
    x.set8(0x2d, v);
    speed_table(x, 0x40);
}

/// Mode 0: firing by angle table.
fn aim_flat<R: Bench>(x: &mut Cx<'_, R>) {
    if !begin_aim(x) {
        return;
    }
    if ready_to_go(x) {
        x.add8(0x5, 2);
        let target = ptr(x, f::OWNER);
        if target.u8(x.b, 0) == 0 {
            return x.set8(0x4, st::FREE);
        }
        let to = (target.u16(x.b, 0x8), 0);
        let de = (x.u16(0x8), x.u16(0xc));
        let angle = crate::aids::direction(x.b, de, to) & 0x1f;
        let v = x.rom8(AIM_TABLE_A + angle.wrapping_sub(8) as u32);
        x.set8(0x2d, v);
        speed_table(x, 0x44);
        let v = x.u16(0x1c);
        x.set16(0x18, v);
    }
    check_parent(x);
}

/// Mode < 0: falls in front of the target (speed according to the distance in X).
fn aim_drop<R: Bench>(x: &mut Cx<'_, R>) {
    if !begin_aim(x) {
        return;
    }
    if ready_to_go(x) {
        x.add8(0x5, 2);
        let target = ptr(x, f::OWNER);
        if target.u8(x.b, 0) == 0 {
            return x.set8(0x4, st::FREE);
        }
        let mut d = target.u16(x.b, 0x8).wrapping_sub(x.u16(0x8));
        if (d as i16) < 0 {
            d = d.wrapping_neg();
        }
        let row = if d > 0x50 { 0 } else { 4 };
        let vz = x.rom16(DROP_SPEEDS + row);
        x.set16(0x18, vz);
        x.set16(0x1a, 0);
        x.set16(0x14, 0);
    }
    check_parent(x);
}

/// Fall after the shot (modes 0 and < 0): touches the ground (Z < 0) → impact animation.
fn land<R: Bench>(x: &mut Cx<'_, R>, mover: fn(&mut Cx<'_, R>), clear_state: bool) {
    mover(x);
    x.anim_step();
    if (x.u16(0xc) as i16) >= 0 {
        return;
    }
    x.add8(0x4, 2);
    if clear_state {
        x.set8(0x5, 0);
    }
    x.set32(0xc, 0);
    x.anim_start(ANIMS, 2);
    crate::sound::play(x.b, crate::sound::CMD_36E, x.o);
}

/// Fall of mode > 0: movement in X, Y and Z.
fn land_xyz<R: Bench>(x: &mut Cx<'_, R>) {
    x.move_xyz();
    x.anim_step();
    if (x.u16(0xc) as i16) > 0 {
        return;
    }
    x.add8(0x4, 2);
    x.set8(0x2a, 0);
    x.set32(0xc, 0);
    x.anim_start(ANIMS, 2);
    crate::sound::play(x.b, crate::sound::CMD_36E, x.o);
}
