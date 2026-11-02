use super::*;
use crate::data::RomAddr;

const ANIMS: RomAddr = RomAddr(0x16f58c);
const OWNER: u32 = 0x46;
const RELEASE: u32 = 0x58;
const HIT: u32 = 0x33;
const SPEED_TABLE: u32 = 33;
const SFX_THROW: crate::sound::Sound = crate::sound::CMD_32F;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    let variant = x.u8(Obj::VARIANT);
    match x.u8(Obj::STATE) {
        0 => init(x, variant),
        2 => match (variant, x.u8(Base::SUB_STATE)) {
            (0, 0) => held(x),
            (0, 2) => {
                x.move_xyz();
                if x.u16(Obj::Z) as i16 > 0 {
                    crate::families::enemy_lib::step(x);
                } else {
                    x.add8(Base::SUB_STATE, 2);
                    x.set16(Obj::Z, 0);
                    ai_anim(x, 3);
                }
            }
            _ => rolling(x),
        },
        _ => x.free_eshl(),
    }
    if x.u8(Obj::ACTIVE) != 0 {
        x.reinsert();
    }
}

fn ai_anim<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    crate::families::enemy_lib::start(x, ANIMS.0, n);
}

fn init<R: Bench>(x: &mut Cx<'_, R>, variant: u8) {
    x.add8(Obj::STATE, 2);
    x.set8(Base::BOX_SET, 7);
    x.set8(Shot::ATTACK_POWER, 0x14);
    if variant != 0 {
        return ai_anim(x, 3);
    }
    crate::placement::load_effect_palette(x.b, crate::placement::effect_palette::LIGHT);
    let dir = if x.u8(Base::FLIP) != 0 { 8 } else { 24 };
    x.set8(Obj::SPEED_INDEX, dir);
    x.select_speed(&crate::objects::motion::SPEEDS_B, SPEED_TABLE as u16);
    x.set16(Obj::VZ, 0xfc00);
    ai_anim(x, 0);
}

/// Held by the launcher: released by `+0x58`, destroyed if the launcher dies (state 4 other than 0x204).
fn held<R: Bench>(x: &mut Cx<'_, R>) {
    let owner = x.linked(OWNER);
    let dying = owner.u8(x.b, Obj::STATE) == 4 && owner.u16(x.b, Obj::STATE) != 0x204;
    if dying {
        return x.free_eshl();
    }
    if x.u8(RELEASE) != 0 {
        x.add8(Base::SUB_STATE, 2);
        crate::sound::play(x.b, SFX_THROW, x.o);
    }
    crate::families::enemy_lib::step(x);
}

fn rolling<R: Bench>(x: &mut Cx<'_, R>) {
    if crate::families::enemy_lib::step(x) {
        return x.free_eshl();
    }
    if x.u8(HIT) == 0 {
        return;
    }
    x.set8(HIT, 0);
    if crate::ai_aids::outside_band(x.b, x.o, 0xfff0, 400) {
        return;
    }
    let Some(ball) = x.alloc(&pools::ESHL) else { return };
    ball.set_u8(x.b, Obj::ACTIVE, 1);
    ball.set_u8(x.b, Obj::KIND, 71);
    ball.set_u8(x.b, Obj::VARIANT, 1);
    for off in [0x8, 0x10] {
        let v = x.u16(off);
        ball.set_u16(x.b, off, v);
    }
    let flip = x.u8(Base::FLIP);
    ball.set_u8(x.b, Base::FLIP, flip);
    let dx: u16 = if flip != 0 { 32 } else { 32u16.wrapping_neg() };
    let bx = ball.u16(x.b, Obj::X).wrapping_add(dx);
    ball.set_u16(x.b, Obj::X, bx);
}
