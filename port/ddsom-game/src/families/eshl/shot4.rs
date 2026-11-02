use super::*;

const ANIMS: u32 = 0x16cb74;
const FALLING: u8 = 0;
const BOUNCING: u8 = 2;
const EXPLODING: u8 = 4;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => init(x),
        2 => {
            match x.u8(0x5) {
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
    x.add8(0x4, 2);
    for off in [0x18, 0x1c, 0x1e, 0x16] {
        x.set16(off, 0);
    }
    x.set16(0x1a, -0x38i16 as u16);
    x.set8(f::BOX_SET, 7);
    x.set8(f::ATTACK_POWER, 3);
    x.set8(f::ATTACK_ID, 9);
    x.set8(0x2d, 8);
    let (mut dx, mut bounce_vx, mut bounce_ax) = (0x38i16, -0x250i16, -0x500i16);
    if x.u8(f::FLIP) == 0 {
        x.set8(0x2d, 0x18);
        (dx, bounce_vx, bounce_ax) = (-dx, -bounce_vx, -bounce_ax);
    }
    x.add16(0x8, dx as u16);
    x.set16(0x42, bounce_vx as u16);
    x.set16(f::OWNER, bounce_ax as u16);
    x.select_speed(&crate::objects::motion::SPEEDS_B, 0x36);
    x.anim_start(ANIMS, 0);
}

fn fall<R: Bench>(x: &mut Cx<'_, R>) {
    x.move_xz();
    x.anim_step();
    x.cull_offscreen();
    if (x.u16(0xc) as i16) < 0 {
        crate::sound::play(x.b, crate::sound::CMD_352_452, x.o);
        x.set16(0xc, 0);
        x.add8(0x5, 2);
        let (vx, ax) = (x.u16(f::OWNER), x.u16(0x42));
        x.set16(0x14, vx);
        x.set16(0x16, ax);
        x.set8(f::ATTACK_POWER, 6);
        x.set8(f::ATTACK_ID, 8);
        x.set8(f::TIMER, 0x20);
        x.select_speed(&crate::objects::motion::SPEEDS_B, 0x17);
    }
}

fn bounce<R: Bench>(x: &mut Cx<'_, R>) {
    x.move_by_table_xy();
    x.anim_step();
    x.cull_offscreen();
    let t = x.u8(f::TIMER).wrapping_sub(1);
    x.set8(f::TIMER, t);
    if (t as i8) < 0 {
        x.add8(0x5, 2);
        crate::sound::play(x.b, crate::sound::CMD_321_421, x.o);
        x.anim_start(ANIMS, 1);
    }
}

fn explode<R: Bench>(x: &mut Cx<'_, R>) {
    if x.anim_step() {
        x.set16(0x4, 0x400);
    }
}
