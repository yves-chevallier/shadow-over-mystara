use super::*;
use crate::world::data::RomAddr;

const ANIMS: RomAddr = RomAddr(0x17100e);
const LINK: u32 = 0x5c;
const LIFE: u32 = 0x46;
const RETARGET: u32 = 0x42;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(LINK) == 0 {
        return x.set32(Obj::STATE, 0x0600_0000);
    }
    match x.u8(Obj::STATE) {
        0 => init(x),
        2 => live(x),
        4 => fade(x),
        _ => x.free_eshl(),
    }
}

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Obj::STATE, 2);
    x.set8(Base::BOX_SET, 7);
    x.set8(Shot::ATTACK_ID, 7);
    x.set8(Shot::ATTACK_POWER, 5);
    let maker = x.linked(LINK);
    copy_xyz16(x, maker);
    let flip = copy_flip(x, maker);
    x.add16(Obj::Y, 0xffff);
    x.set8(RETARGET, 0x14);
    x.set16(LIFE, 0x168);
    x.add16(Obj::X, if flip != 0 { 0x4a } else { neg(0x4a) });
    x.add16(Obj::Z, 0x32);
    x.anim_start(ANIMS.0, 0);
    x.select_speed(&crate::world::objects::motion::SPEEDS_B, 7);
}

fn live<R: Bench>(x: &mut Cx<'_, R>) {
    let maker = x.linked(LINK);
    if maker.u16(x.b, Obj::STATE) == 0x406 || maker.u16(x.b, Shot::OWNER) == 0 {
        x.set32(Obj::STATE, 0x0400_0000);
        x.set8(RETARGET, 0x1e);
        return;
    }
    if x.u8(Base::SUB_STATE) == 0 {
        track(x, maker);
    }
    x.reinsert();
}

fn track<R: Bench>(x: &mut Cx<'_, R>, maker: Obj) {
    let left = x.u16(LIFE).wrapping_sub(1);
    x.set16(LIFE, left);
    if s16(left) < 0 {
        return explode(x);
    }
    let n = x.u8(RETARGET).wrapping_sub(1);
    x.set8(RETARGET, n);
    let mut aim = maker;
    if s8(n) < 0 {
        x.set8(RETARGET, 0x14);
        let w = x.linked(LINK).u16(x.b, Shot::OWNER);
        if w == 0 {
            return approach(x);
        }
        aim = obj_of(w);
        let dir = crate::services::aids::direction_to(x.b, x.o, aim);
        x.set8(Obj::SPEED_INDEX, dir);
    }
    let dx = aim.u16(x.b, Obj::X).wrapping_sub(x.u16(Obj::X)).wrapping_add(8);
    if dx > 0x20 {
        return approach(x);
    }
    let dy = aim.u16(x.b, Obj::Y).wrapping_sub(x.u16(Obj::Y)).wrapping_add(8);
    if dy > 0x20 {
        return approach(x);
    }
    explode(x);
}

fn approach<R: Bench>(x: &mut Cx<'_, R>) {
    x.move_by_table_xy();
    x.anim_step();
}

/// Creates 16 bursts (type 58, variants 15 down to 0) then goes to state 6.
fn explode<R: Bench>(x: &mut Cx<'_, R>) {
    for n in (0..16u8).rev() {
        let Some(c) = x.alloc(&pools::ESHL) else { break };
        c.set_u8(x.b, Obj::ACTIVE, 1);
        c.set_u8(x.b, Obj::KIND, 0x3a);
        c.set_u8(x.b, Obj::VARIANT, n);
        c.set_u16(x.b, LINK, x.o.0 as u16);
    }
    x.add8(Obj::STATE, 4);
}

fn fade<R: Bench>(x: &mut Cx<'_, R>) {
    let t = x.u16(RETARGET).wrapping_sub(1);
    x.set16(RETARGET, t);
    if s16(t) < 0 {
        return x.set8(Obj::STATE, 6);
    }
    let hidden = x.u16(RETARGET) & 3 != 0;
    x.set_bit(Obj::FLAGS, 1, hidden);
}
