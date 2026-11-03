use super::*;
use crate::data::RomAddr;
use crate::families::shell::records::{Host, view};
use crate::families::shell::tables::{tables, Read};

const ANIMS: RomAddr = RomAddr(0x16e1f4);
const PALETTE_DST: u32 = 0x91c300;
const PALETTE_TINT: u32 = 0x91c000;
const CHARGE: u8 = 0;
const AIM: u8 = 2;
const FALL: u8 = 4;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => init(x),
        2 => active(x),
        4 => {
            if x.anim_step() {
                x.add8(Obj::STATE, 2);
            }
        }
        _ => x.free_eshl(),
    }
}

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Obj::STATE, 2);
    for i in 0..8 {
        let v = crate::palette::library_excerpt("shot17_palette", 0x25a660, 0x25a680, 4).at32(x.b, 4 * i);
        wr32(x.b, PALETTE_DST + 4 * i, v);
    }
    let tint = rd32(x.b, PALETTE_TINT) & 0xf000_f000;
    for i in 0..8 {
        let v = rd32(x.b, PALETTE_DST + 4 * i) | tint;
        wr32(x.b, PALETTE_DST + 4 * i, v);
    }
    x.set8(Base::OWN_PALETTE, 1);
    x.set8(Base::PALETTE, 0x18);
    x.set8(Shot17::TIMER, 0x3c);
    let row = x.u8(Shot17::ROW) as u32;
    let dz = tables().shot17_spawn_offsets.at16(x.b, row);
    x.add16(Obj::Z, dz);
    let mut dx = tables().shot17_spawn_offsets.at16(x.b, 2 + row);
    if x.u8(Base::FLIP) != 0 {
        dx = dx.wrapping_neg();
    }
    x.add16(Obj::X, dx);
    x.add32(Obj::Y, 0xffff_ffff);
    x.set8(Base::BOX_SET, 7);
    x.set8(Shot::ATTACK_POWER, 8);
    x.set8(Shot::ATTACK_ID, 2);
    x.anim_start(ANIMS.0, 0);
    x.reinsert();
}

/// The parent has left its normal states: the projectile disappears.
fn check_parent<R: Bench>(x: &mut Cx<'_, R>) {
    let parent = ptr(x, 0x56);
    if (parent.u8(x.b, Obj::STATE) as i8) >= 4 {
        kill(x);
    }
}

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    let mode = x.u8(Shot17::MODE);
    let state = x.u8(Base::SUB_STATE);
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
    if (parent.u8(x.b, Obj::STATE) as i8) >= 4 {
        return kill(x);
    }
    x.anim_step();
    x.add8(Shot17::TIMER, 0xff);
    if (x.u8(Shot17::TIMER) as i8) < 0 {
        x.add8(Base::SUB_STATE, 2);
    }
    check_parent(x);
}

/// Start of the firing phase: announcement to the parent (`+0xa5`), 20-frame wait.
fn begin_aim<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if x.u8(Base::STEP) == 0 {
        x.add8(Base::STEP, 2);
        x.set8(Shot17::TIMER, 0x14);
        x.anim_start(ANIMS.0, 1);
        return false;
    }
    true
}

/// After the wait: notifies the parent, true when the projectile must leave.
fn ready_to_go<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    x.anim_step();
    x.add8(Shot17::TIMER, 0xff);
    if (x.u8(Shot17::TIMER) as i8) >= 0 {
        return false;
    }
    let parent = ptr(x, 0x56);
    parent.set_u8(x.b, Host::SHOT_LANDED, 1);
    x.u8(Shot17::LANDED) != 0
}

/// Mode > 0: aims at the target (distance/altitude ratio, angle).
fn aim_angle<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Base::STEP) == 0 {
        x.add8(Base::STEP, 2);
        x.set8(Shot17::TIMER, 0x14);
        x.anim_start(ANIMS.0, 1);
        return;
    }
    let parent = ptr(x, 0x56);
    if (parent.u8(x.b, Obj::STATE) as i8) >= 4 {
        return kill(x);
    }
    if ready_to_go(x) {
        x.add8(Base::SUB_STATE, 2);
        x.set8(Obj::SPEED_INDEX, 0x10);
        if x.u8(Obj::VARIANT) == 8 {
            x.set32(Obj::VX, 0);
            x.set32(Obj::VZ, 0xf800_0000);
            x.set32(Obj::VY, 0xfb80_0000);
        } else {
            let target = ptr(x, Shot::OWNER);
            if target.u8(x.b, Obj::ACTIVE) == 0 {
                return x.set8(Obj::STATE, st::FREE);
            }
            aim_at(x, target);
        }
    }
    check_parent(x);
}

/// Computes the vertical speed (slope toward the target) and the angle `+0x2d`.
fn aim_at<R: Bench>(x: &mut Cx<'_, R>, target: Obj) {
    let dy = x.u32(Obj::Y).wrapping_sub(target.u32(x.b, Obj::Y));
    let mut dx = x.u32(Obj::X).wrapping_sub(target.u32(x.b, Obj::X));
    if (dx as i32) < 0 {
        dx = dx.wrapping_neg();
    }
    let far = if (dx as i32) < (dy as i32) { dy } else { dx } >> 3;
    let quotient = crate::aids::divide(x.u32(Obj::Z), far).0 << 7;
    let slope = (quotient as u16).wrapping_add(((far >> 16) as u16) << 2);
    x.set16(Obj::VZ, slope.wrapping_neg());
    let angle = crate::aids::direction_to(x.b, x.o, target) & 0x1f;
    let idx = angle.wrapping_sub(8) as u32;
    let v = tables().shot17_aim_b.at8(x.b, idx);
    x.set8(Obj::SPEED_INDEX, v);
    speed_table(x, 0x40);
}

/// Mode 0: firing by angle table.
fn aim_flat<R: Bench>(x: &mut Cx<'_, R>) {
    if !begin_aim(x) {
        return;
    }
    if ready_to_go(x) {
        x.add8(Base::SUB_STATE, 2);
        let target = ptr(x, Shot::OWNER);
        if target.u8(x.b, Obj::ACTIVE) == 0 {
            return x.set8(Obj::STATE, st::FREE);
        }
        let to = (target.u16(x.b, Obj::X), 0);
        let de = (x.u16(Obj::X), x.u16(Obj::Z));
        let angle = crate::aids::direction(x.b, de, to) & 0x1f;
        let v = tables().shot17_aim_a.at8(x.b, angle.wrapping_sub(8) as u32);
        x.set8(Obj::SPEED_INDEX, v);
        speed_table(x, 0x44);
        let v = x.u16(Obj::VY);
        x.set16(Obj::VZ, v);
    }
    check_parent(x);
}

/// Mode < 0: falls in front of the target (speed according to the distance in X).
fn aim_drop<R: Bench>(x: &mut Cx<'_, R>) {
    if !begin_aim(x) {
        return;
    }
    if ready_to_go(x) {
        x.add8(Base::SUB_STATE, 2);
        let target = ptr(x, Shot::OWNER);
        if target.u8(x.b, Obj::ACTIVE) == 0 {
            return x.set8(Obj::STATE, st::FREE);
        }
        let mut d = target.u16(x.b, Obj::X).wrapping_sub(x.u16(Obj::X));
        if (d as i16) < 0 {
            d = d.wrapping_neg();
        }
        let row = if d > 0x50 { 0 } else { 4 };
        let vz = tables().shot17_drop_speeds.at16(x.b, row);
        x.set16(Obj::VZ, vz);
        x.set16(Obj::AZ, 0);
        x.set16(Obj::VX, 0);
    }
    check_parent(x);
}

/// Fall after the shot (modes 0 and < 0): touches the ground (Z < 0) → impact animation.
fn land<R: Bench>(x: &mut Cx<'_, R>, mover: fn(&mut Cx<'_, R>), clear_state: bool) {
    mover(x);
    x.anim_step();
    if (x.u16(Obj::Z) as i16) >= 0 {
        return;
    }
    x.add8(Obj::STATE, 2);
    if clear_state {
        x.set8(Base::SUB_STATE, 0);
    }
    x.set32(Obj::Z, 0);
    x.anim_start(ANIMS.0, 2);
    crate::sound::play(x.b, crate::sound::CMD_36E, x.o);
}

/// Fall of mode > 0: movement in X, Y and Z.
fn land_xyz<R: Bench>(x: &mut Cx<'_, R>) {
    x.move_xyz();
    x.anim_step();
    if (x.u16(Obj::Z) as i16) > 0 {
        return;
    }
    x.add8(Obj::STATE, 2);
    x.set8(Base::OWN_PALETTE, 0);
    x.set32(Obj::Z, 0);
    x.anim_start(ANIMS.0, 2);
    crate::sound::play(x.b, crate::sound::CMD_36E, x.o);
}

view! {
    /// Fields of shot 17.
    Shot17 {
        /// Row of the spawn offsets.
        row, set_row, ROW: u8 @ 0x42;
        /// Mode (h).
        mode, set_mode, MODE: u8 @ 0x43;
        /// Countdown (h).
        timer, set_timer, TIMER: u8 @ 0x54;
        /// Non-zero: landed (h).
        landed, set_landed, LANDED: u8 @ 0x55;
    }
}
