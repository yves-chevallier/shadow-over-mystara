use super::*;
use crate::data::RomAddr;
use crate::families::shell::records::Host;
use crate::families::shell::tables::{tables, Read};

/// Offset of the rows in `debris45_place_offsets`.
const PLACE_ROWS: u32 = 0x16;

const ANIMS: RomAddr = RomAddr(0x17f002);
const SOURCE: u32 = 0x46;
/// Margin of the on-screen restraint.
const MARGIN: u32 = 0x10;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    match x.u8(Obj::VARIANT) {
        0 => variant0(x),
        1 => variant1(x),
        _ => {}
    }
}

/// The object disappears: state 6.
fn vanish<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Obj::STATE, 6);
    x.set8(Base::SUB_STATE, 0);
}

/// Source (negative address), possibly alive.
fn source<R: Bench>(x: &mut Cx<'_, R>, check_active: bool) -> Option<Obj> {
    let word = x.u16(SOURCE);
    if s16(word) >= 0 {
        return None;
    }
    let o = obj_of(word);
    if check_active && o.u8(x.b, Obj::ACTIVE) == 0 {
        return None;
    }
    Some(o)
}

fn variant0<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => {
            x.add8(Obj::STATE, 2);
            x.set8(Base::SUB_STATE, 0);
            let Some(src) = source(x, true) else { return vanish(x) };
            copy_xyz16(x, src);
            copy_flip(x, src);
            x.add16(Obj::Z, 0x50);
            let r = random(x);
            let row = r & 0x38;
            let (mut vx, mut ax) = (tables().debris45_burst_speeds.at16(x.b, row), tables().debris45_burst_speeds.at16(x.b, row + 2));
            if x.u8(Base::FLIP) == 0 {
                (vx, ax) = (neg(vx), neg(ax));
            }
            x.set16(Obj::VX, vx);
            x.set16(Obj::AX, ax);
            let vz = tables().debris45_burst_speeds.at16(x.b, row + 4);
            x.set16(Obj::VZ, vz);
            x.set16(Obj::AZ, 0);
            let vy = tables().debris45_burst_speeds.at16(x.b, row + 6);
            x.set16(Obj::VY, vy);
            x.set16(Obj::AY, 0);
            let anim = tables().debris45_burst_anims.at8(x.b, r & 3);
            ai_anim(x, ANIMS.0, anim as u32);
            x.set8(Base::BOX_SET, 7);
            x.set8(Shot::ATTACK_POWER, 0);
        }
        2 => {
            if ai_step(x) {
                return vanish(x);
            }
            if x.u8(Base::SUB_STATE) == 0 && x.u8(Shot::HIT_FLAGS) & 0x8 == 0 && x.u8(Base::ANIM_EVENT) == 0 {
                x.add8(Base::SUB_STATE, 2);
                x.set32(Obj::VX, 0);
                x.set32(Obj::VY, 0);
                let r = random(x) & 6;
                let vz = tables().debris45_hop_vz.at16(x.b, r);
                x.set16(Obj::VZ, vz);
                x.set16(Obj::AZ, 0);
                ai_anim(x, ANIMS.0, 0x11);
            }
            x.move_xyz();
            settle(x);
        }
        4 => x.add8(Obj::STATE, 2),
        6 => x.free_eshl(),
        _ => {}
    }
}

/// Common end: kept on the screen, then put back in the sorted list.
fn settle<R: Bench>(x: &mut Cx<'_, R>) {
    x.keep_on_screen(MARGIN as u16);
    x.reinsert();
}

fn variant1<R: Bench>(x: &mut Cx<'_, R>) {
    let Some(src) = source(x, false) else { return vanish(x) };
    match x.u8(Obj::STATE) {
        0 => {
            x.add8(Obj::STATE, 2);
            x.set8(Base::SUB_STATE, 0);
            let flip = copy_flip(x, src);
            if flip != 0 {
                let k = x.u8(Base::SETTING) as u32;
                let mirrored = tables().debris45_mirror.at8(x.b, k);
                x.set8(Base::SETTING, mirrored);
            }
            let k = x.u8(Base::SETTING);
            x.set8(Obj::SPEED_INDEX, k.wrapping_mul(4));
            x.select_speed(&crate::objects::motion::SPEEDS_A, 0x1f);
            place(x, src);
            x.set8(Shot::ATTACK_ID, 0x14);
            x.anim_start(ANIMS.0, 0x1f);
        }
        2 => {
            match x.u8(Base::SUB_STATE) {
                0 => {
                    if src.u8(x.b, Host::PHASE_B) == 4 {
                        x.anim_step();
                    } else {
                        x.add8(Base::SUB_STATE, 2);
                        x.set32(Obj::VX, 0);
                        x.set32(Obj::VY, 0);
                        x.set16(Obj::VZ, neg(0x300));
                        x.set16(Obj::AZ, 0);
                        x.anim_start(ANIMS.0, 0x21);
                    }
                }
                2 => {
                    x.anim_step();
                    x.move_by_table_xy();
                    x.move_z();
                    if s16(x.u16(Obj::Z)) < 0 {
                        x.set16(Obj::Z, 0);
                        x.add8(Obj::STATE, 2);
                    }
                }
                _ => {}
            }
            settle(x);
        }
        4 => x.add8(Obj::STATE, 2),
        6 => x.free_eshl(),
        _ => {}
    }
}

/// Places the burst on the source: X and Y of the source, Z + 0x30, then offset according to `+0x2f`.
fn place<R: Bench>(x: &mut Cx<'_, R>, src: Obj) {
    let (sx, sy, sz) = (src.u16(x.b, Obj::X), src.u16(x.b, Obj::Y), src.u16(x.b, Obj::Z));
    x.set16(Obj::X, sx);
    x.set16(Obj::Y, sy);
    x.set16(Obj::Z, sz.wrapping_add(0x30));
    let k = 2 * (x.u8(Base::SETTING) & 7) as u32;
    // the rows start at offset 0x16 of the table (see `debris45_place_offsets`)
    let dx = tables().debris45_place_offsets.at16(x.b, PLACE_ROWS + k);
    // the Z row is the same table read with a two-byte offset, indexed by dx
    let dz = tables().debris45_place_offsets.at16(x.b, (PLACE_ROWS + 2).wrapping_add(dx as i16 as i32 as u32));
    x.add16(Obj::X, dx);
    x.add16(Obj::Z, dz);
}
