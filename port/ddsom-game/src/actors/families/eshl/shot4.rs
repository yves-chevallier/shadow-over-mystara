use super::*;
use crate::world::data::RomAddr;
use crate::actors::families::shell::records::view;

const ANIMS: RomAddr = RomAddr(0x16cb74);
const FALLING: u8 = 0;
const BOUNCING: u8 = 2;
const EXPLODING: u8 = 4;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => init(x),
        2 => {
            match x.u8(Base::SUB_STATE) {
                FALLING => fall(x),
                BOUNCING => bounce(x),
                EXPLODING => explode(x),
                _ => {}
            }
            // the original looks here for the terrain under the shot (0x24b0) without reading the result; the
            // registers it leaves are reread by no one (`exit` probe): the search, without effect, is omitted
            x.reinsert();
        }
        _ => x.free_eshl(),
    }
}

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Obj::STATE, 2);
    for off in [0x18, 0x1c, 0x1e, 0x16] {
        x.set16(off, 0u16);
    }
    x.set16(Obj::AZ, -0x38i16 as u16);
    x.set8(Base::BOX_SET, 7);
    x.set8(Shot::ATTACK_POWER, 3);
    x.set8(Shot::ATTACK_ID, 9);
    x.set8(Obj::SPEED_INDEX, 8);
    let (mut dx, mut bounce_vx, mut bounce_ax) = (0x38i16, -0x250i16, -0x500i16);
    if x.u8(Base::FLIP) == 0 {
        x.set8(Obj::SPEED_INDEX, 0x18);
        (dx, bounce_vx, bounce_ax) = (-dx, -bounce_vx, -bounce_ax);
    }
    x.add16(Obj::X, dx as u16);
    x.set16(Shot4::BOUNCE_VX, bounce_vx as u16);
    x.set16(Shot::OWNER, bounce_ax as u16);
    x.select_speed(&crate::world::objects::motion::SPEEDS_B, 0x36);
    x.anim_start(ANIMS.0, 0);
}

fn fall<R: Bench>(x: &mut Cx<'_, R>) {
    x.move_xz();
    x.anim_step();
    x.cull_offscreen();
    if (x.u16(Obj::Z) as i16) < 0 {
        crate::hw::sound::play(x.b, crate::hw::sound::CMD_352_452, x.o);
        x.set16(Obj::Z, 0);
        x.add8(Base::SUB_STATE, 2);
        let (vx, ax) = (x.u16(Shot::OWNER), x.u16(Shot4::BOUNCE_VX));
        x.set16(Obj::VX, vx);
        x.set16(Obj::AX, ax);
        x.set8(Shot::ATTACK_POWER, 6);
        x.set8(Shot::ATTACK_ID, 8);
        x.set8(Shot::TIMER, 0x20);
        x.select_speed(&crate::world::objects::motion::SPEEDS_B, 0x17);
    }
}

fn bounce<R: Bench>(x: &mut Cx<'_, R>) {
    x.move_by_table_xy();
    x.anim_step();
    x.cull_offscreen();
    let t = x.u8(Shot::TIMER).wrapping_sub(1);
    x.set8(Shot::TIMER, t);
    if (t as i8) < 0 {
        x.add8(Base::SUB_STATE, 2);
        crate::hw::sound::play(x.b, crate::hw::sound::CMD_321_421, x.o);
        x.anim_start(ANIMS.0, 1);
    }
}

fn explode<R: Bench>(x: &mut Cx<'_, R>) {
    if x.anim_step() {
        x.set16(Obj::STATE, 0x400);
    }
}

view! {
    /// Fields of shot 4.
    Shot4 {
        /// Speed X of the bounce (h).
        bounce_vx, set_bounce_vx, BOUNCE_VX: u16 @ 0x42;
    }
}
