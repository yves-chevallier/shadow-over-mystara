use super::*;
use crate::data::RomAddr;
use crate::families::shell::records::view;
use crate::families::shell::tables::{tables, Read};

const ANIMS: RomAddr = RomAddr(0x16ea90);
const LAND_SOUND: crate::sound::Sound = crate::sound::CMD_8D;
const FLY: u8 = 0;
const GROUND: u8 = 2;
const BLINK: u8 = 4;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => init(x),
        2 => {
            match x.u8(Base::SUB_STATE) {
                FLY => fly(x),
                GROUND => ground(x),
                BLINK => blink_out(x),
                _ => {}
            }
            x.reinsert();
        }
        4 => x.add8(Obj::STATE, 2),
        _ => x.free_eshl(),
    }
}

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add16(Shot26::TIMER, 0xffff);
    if x.u16(Shot26::TIMER) != 0 {
        return;
    }
    x.add8(Obj::STATE, 2);
    x.set8(Base::BOX_SET, 7);
    let source = x.linked(Shot26::SOURCE);
    let (sx, sy) = (source.u16(x.b, Obj::X), source.u16(x.b, Obj::Y));
    x.set16(Obj::X, sx);
    x.set16(Obj::Y, sy);
    let row = x.u8(Shot26::ROW) as u32;
    let height = tables().shot26_heights.at16(x.b, 2 * row);
    x.set16(Obj::Z, height);
    let rng = x.random();
    let mut table = tables().shot26_speed_tables.at16(x.b, 2 * (rng as u32 & 3)) as u32;
    if x.u8(Shot26::COUNT) != 0 && x.u8(Obj::VARIANT) == 0 {
        table = 2;
    }
    speed_table(x, table);
    x.set16(Obj::VZ, 0x300);
    x.set16(Obj::AZ, -0x30i16 as u16);
    x.anim_start(ANIMS.0, 0);
    x.reinsert();
}

fn fly<R: Bench>(x: &mut Cx<'_, R>) {
    x.anim_step();
    x.move_xyz();
    if (x.u16(Obj::Z) as i16) >= 0 {
        return;
    }
    x.add8(Base::SUB_STATE, 2);
    x.set32(Obj::Z, 0);
    x.set16(Shot26::TIMER, 0x80);
    x.anim_start(ANIMS.0, 1);
    crate::sound::play(x.b, LAND_SOUND, x.o);
    if let Some(shadow) = crate::families::small::shot_18(&mut crate::families::effects_creators::leftovers::Pure, x.b, x.o, 1) {
        x.set16(Shot26::LINK, low16(shadow));
    }
}

fn ground<R: Bench>(x: &mut Cx<'_, R>) {
    x.anim_step();
    x.add16(Shot26::TIMER, 0xffff);
    if x.u16(Shot26::TIMER) != 0 {
        return;
    }
    x.add8(Base::SUB_STATE, 2);
    x.set16(Shot26::TIMER, 0x60);
    x.anim_start(ANIMS.0, 2);
    if x.u16(Shot26::LINK) != 0 {
        let shadow = x.linked(Shot26::LINK);
        shadow.set_u8(x.b, Obj::STATE, 6);
    }
}

fn blink_out<R: Bench>(x: &mut Cx<'_, R>) {
    x.anim_step();
    x.add16(Shot26::TIMER, 0xffff);
    if x.u16(Shot26::TIMER) == 0 {
        x.add8(Obj::STATE, 2);
    }
    let hidden = x.u16(Shot26::TIMER) & 1 == 0;
    x.set_bit(Obj::FLAGS, 1, hidden);
}

view! {
    /// Fields of shot 26 (a spawner, see `Spawner`).
    Shot26 {
        /// Number of created shots, row of the offsets (h).
        count, set_count, COUNT: u16 @ 0x56;
        /// Countdown of the current step (h).
        timer, set_timer, TIMER: u16 @ 0x58;
        /// Linked object: shadow or second timer (h).
        link, set_link, LINK: u16 @ 0x5a;
        /// Source object (16-bit pointer).
        source, set_source, SOURCE: u16 @ 0x5c;
        /// Row of the height tables (h).
        row, set_row, ROW: u8 @ 0x60;
    }
}
