use super::*;
use crate::data::RomAddr;
use crate::families::shell::records::view;
use crate::families::shell::tables::{tables, Read};

const ANIMS: RomAddr = RomAddr(0x16e5fa);
/// Rows of `shot19_steps` (one per direction).
const STEPS_FACING_RIGHT: u32 = 0x10;
const STEPS_FACING_LEFT: u32 = 0;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => init(x),
        2 => {
            if (x.u16(Obj::Z) as i16) >= 0 {
                x.move_xz();
            } else {
                x.set32(Obj::Z, 0);
                x.add8(Obj::STATE, 2);
                x.set8(Shot19::TIMER, 0x30);
                x.anim_start(ANIMS.0, 0x10);
            }
        }
        4 => {
            x.anim_step();
            x.add8(Shot19::TIMER, 0xff);
            if (x.u8(Shot19::TIMER) as i8) < 0 {
                x.add8(Obj::STATE, 2);
            }
        }
        _ => x.free_eshl(),
    }
}

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Obj::STATE, 2);
    x.set8(Base::BOX_SET, 7);
    x.set8(Shot::ATTACK_POWER, 5);
    x.set8(Shot::ATTACK_ID, 0x19);
    let target = ptr(x, Shot::OWNER);
    if target.u8(x.b, Obj::ACTIVE) == 0 {
        return x.add8(Base::STEP, 4);
    }
    let de = (x.u16(Obj::X), x.u16(Obj::Z));
    let to = (target.u16(x.b, Obj::X), 0);
    let angle = crate::aids::direction(x.b, de, to) & 0x1f;
    let (base, steps) = if x.u8(Base::FLIP) != 0 { (9, STEPS_FACING_LEFT) } else { (0x10, STEPS_FACING_RIGHT) };
    let r = angle.wrapping_sub(base);
    let step = if r == 0 {
        0
    } else if (r as i8) < 0 || r > 8 {
        7
    } else {
        r as u32
    };
    let entry = steps + 2 * step;
    let sprite = tables().shot19_steps.at8(x.b, entry);
    x.set8(Obj::SPEED_INDEX, sprite);
    let anim = tables().shot19_steps.at8(x.b, entry + 1) as i8 as i16 as u16 as u32;
    let off = 4 * step;
    let dx = tables().shot19_offsets.at16(x.b, off);
    x.add16(Obj::X, dx);
    let dz = tables().shot19_offsets.at16(x.b, off + 2);
    x.add16(Obj::Z, dz);
    x.set8(Base::FLIP, 0);
    x.anim_start(ANIMS.0, anim as u16);
    speed_table(x, 0x40);
    let v = x.u16(Obj::VY);
    x.set16(Obj::VZ, v);
    x.set16(Obj::VY, 0);
    x.reinsert();
}

view! {
    /// Fields of shot 19.
    Shot19 {
        /// Countdown (h).
        timer, set_timer, TIMER: u8 @ 0x58;
    }
}
