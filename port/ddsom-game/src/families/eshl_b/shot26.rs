use super::*;
use crate::families::shell::tables::{tables, Read};

const ANIMS: u32 = 0x16ea90;
const LAND_SOUND: crate::sound::Sound = crate::sound::CMD_8D;
const FLY: u8 = 0;
const GROUND: u8 = 2;
const BLINK: u8 = 4;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => init(x),
        2 => {
            match x.u8(0x5) {
                FLY => fly(x),
                GROUND => ground(x),
                BLINK => blink_out(x),
                _ => {}
            }
            x.reinsert();
        }
        4 => x.add8(0x4, 2),
        _ => x.free_eshl(),
    }
}

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add16(0x58, 0xffff);
    if x.u16(0x58) != 0 {
        return;
    }
    x.add8(0x4, 2);
    x.set8(f::BOX_SET, 7);
    let source = x.linked(0x5c);
    let (sx, sy) = (source.u16(x.b, 0x8), source.u16(x.b, 0x10));
    x.set16(0x8, sx);
    x.set16(0x10, sy);
    let row = x.u8(0x60) as u32;
    let height = tables().shot26_heights.at16(x.b, 2 * row);
    x.set16(0xc, height);
    let rng = x.random();
    let mut table = tables().shot26_speed_tables.at16(x.b, 2 * (rng as u32 & 3)) as u32;
    if x.u8(0x56) != 0 && x.u8(0x3) == 0 {
        table = 2;
    }
    speed_table(x, table);
    x.set16(0x18, 0x300);
    x.set16(0x1a, -0x30i16 as u16);
    x.anim_start(ANIMS, 0);
    x.reinsert();
}

fn fly<R: Bench>(x: &mut Cx<'_, R>) {
    x.anim_step();
    x.move_xyz();
    if (x.u16(0xc) as i16) >= 0 {
        return;
    }
    x.add8(0x5, 2);
    x.set32(0xc, 0);
    x.set16(0x58, 0x80);
    x.anim_start(ANIMS, 1);
    crate::sound::play(x.b, LAND_SOUND, x.o);
    if let Some(shadow) = crate::families::small::shot_18(&mut crate::families::effects_creators::leftovers::Pure, x.b, x.o, 1) {
        x.set16(0x5a, low16(shadow));
    }
}

fn ground<R: Bench>(x: &mut Cx<'_, R>) {
    x.anim_step();
    x.add16(0x58, 0xffff);
    if x.u16(0x58) != 0 {
        return;
    }
    x.add8(0x5, 2);
    x.set16(0x58, 0x60);
    x.anim_start(ANIMS, 2);
    if x.u16(0x5a) != 0 {
        let shadow = x.linked(0x5a);
        shadow.set_u8(x.b, 0x4, 6);
    }
}

fn blink_out<R: Bench>(x: &mut Cx<'_, R>) {
    x.anim_step();
    x.add16(0x58, 0xffff);
    if x.u16(0x58) == 0 {
        x.add8(0x4, 2);
    }
    let hidden = x.u16(0x58) & 1 == 0;
    x.set_bit(0x1, 1, hidden);
}
