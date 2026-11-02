use super::*;
use crate::families::shell::tables::{tables, Read};

const ANIMS: u32 = 0x16e5fa;
/// Rows of `shot19_steps` (one per direction).
const STEPS_FACING_RIGHT: u32 = 0x10;
const STEPS_FACING_LEFT: u32 = 0;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => init(x),
        2 => {
            if (x.u16(0xc) as i16) >= 0 {
                x.move_xz();
            } else {
                x.set32(0xc, 0);
                x.add8(0x4, 2);
                x.set8(0x58, 0x30);
                x.anim_start(ANIMS, 0x10);
            }
        }
        4 => {
            x.anim_step();
            x.add8(0x58, 0xff);
            if (x.u8(0x58) as i8) < 0 {
                x.add8(0x4, 2);
            }
        }
        _ => x.free_eshl(),
    }
}

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x4, 2);
    x.set8(f::BOX_SET, 7);
    x.set8(f::ATTACK_POWER, 5);
    x.set8(f::ATTACK_ID, 0x19);
    let target = ptr(x, f::OWNER);
    if target.u8(x.b, 0) == 0 {
        return x.add8(0x6, 4);
    }
    let de = (x.u16(0x8), x.u16(0xc));
    let to = (target.u16(x.b, 0x8), 0);
    let angle = crate::aids::direction(x.b, de, to) & 0x1f;
    let (base, steps) = if x.u8(f::FLIP) != 0 { (9, STEPS_FACING_LEFT) } else { (0x10, STEPS_FACING_RIGHT) };
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
    x.set8(0x2d, sprite);
    let anim = tables().shot19_steps.at8(x.b, entry + 1) as i8 as i16 as u16 as u32;
    let off = 4 * step;
    let dx = tables().shot19_offsets.at16(x.b, off);
    x.add16(0x8, dx);
    let dz = tables().shot19_offsets.at16(x.b, off + 2);
    x.add16(0xc, dz);
    x.set8(f::FLIP, 0);
    x.anim_start(ANIMS, anim as u16);
    speed_table(x, 0x40);
    let v = x.u16(0x1c);
    x.set16(0x18, v);
    x.set16(0x1c, 0);
    x.reinsert();
}
