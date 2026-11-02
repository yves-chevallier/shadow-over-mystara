use super::*;
use crate::families::shell::tables::{held16, tables, Read};

const ANIMS: u32 = 0x1718d8;
const IMPACT_ANIMS: u32 = 0x16d1d6;
const CREATOR: u32 = 0x5c;
const RANK: u32 = 0x2f;
const SFX_THROW: crate::sound::Sound = crate::sound::CMD_32F;
const SFX_LAND: crate::sound::Sound = crate::sound::CMD_330;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(CREATOR) == 0 {
        x.set32(0x4, 0x0600_0000);
        return;
    }
    match x.u8(0x4) {
        0 => {
            x.add8(0x4, 2);
            crate::placement::load_effect_palette(x.b, crate::placement::effect_palette::LIGHT);
            place(x);
        }
        2 => {
            match x.u8(0x5) {
                0 => {
                    if x.anim_step() {
                        x.add8(0x5, 2);
                        x.set8(f::BOX_SET, 7);
                        x.set8(f::ATTACK_ID, 0x37);
                        crate::sound::play(x.b, SFX_THROW, x.o);
                        x.anim_start(ANIMS, 1);
                    }
                }
                2 => {
                    if x.move_xyz().gt() {
                        x.anim_step();
                    } else {
                        x.set16(0xc, 0);
                        crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::B, 40);
                        x.add8(0x5, 2);
                        x.set8(f::BOX_SET, 7);
                        x.set8(f::ATTACK_ID, 0x1c);
                        x.anim_start(IMPACT_ANIMS, 0);
                        crate::sound::play(x.b, SFX_LAND, x.o);
                    }
                }
                _ => {
                    if x.anim_step() {
                        x.add8(0x4, 2);
                    }
                }
            }
            x.reinsert();
        }
        _ => x.free_eshl(),
    }
}

/// 0xa8478: position (longs) and orientation of the creator, one pixel in front of it, plus the offset and the
/// speeds of the sheet.
fn place<R: Bench>(x: &mut Cx<'_, R>) {
    let creator = x.linked(CREATOR);
    for off in [0x8, 0x10, 0xc] {
        let v = creator.u32(x.b, off);
        x.set32(off, v);
    }
    let flip = creator.u8(x.b, f::FLIP);
    x.set8(f::FLIP, flip);
    x.add16(0x10, 0xffff);
    let rank = x.u8(RANK) as u32;
    let rows = tables().splinter72_rows.at32(x.b, 4 * rank);
    let variant = x.u8(0x3) as u32;
    let row = rows + 18 * variant;
    let [dx, dy, dz] = [0, 2, 4].map(|k| held16(x.b, row + k));
    for (field, k) in [(0x14, 6), (0x18, 8), (0x18, 10), (0x1a, 12), (0x1c, 14), (0x1e, 16)] {
        let v = held16(x.b, row + k);
        x.set16(field, v);
    }
    x.add16(0x8, dx);
    x.add16(0xc, dy);
    x.add16(0x10, dz);
}
