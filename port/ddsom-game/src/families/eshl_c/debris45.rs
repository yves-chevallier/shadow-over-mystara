use super::*;
use crate::families::shell::tables::{tables, Read};

const ANIMS: u32 = 0x17f002;
const SOURCE: u32 = 0x46;
/// Margin of the on-screen restraint.
const MARGIN: u32 = 0x10;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    match x.u8(0x3) {
        0 => variant0(x),
        1 => variant1(x),
        _ => {}
    }
}

/// The object disappears: state 6.
fn vanish<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(0x4, 6);
    x.set8(0x5, 0);
}

/// Source (negative address), possibly alive.
fn source<R: Bench>(x: &mut Cx<'_, R>, check_active: bool) -> Option<Obj> {
    let word = x.u16(SOURCE);
    if s16(word) >= 0 {
        return None;
    }
    let o = obj_of(word);
    if check_active && o.u8(x.b, 0) == 0 {
        return None;
    }
    Some(o)
}

fn variant0<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => {
            x.add8(0x4, 2);
            x.set8(0x5, 0);
            let Some(src) = source(x, true) else { return vanish(x) };
            copy_xyz16(x, src);
            copy_flip(x, src);
            x.add16(0xc, 0x50);
            let r = random(x);
            let row = r & 0x38;
            let (mut vx, mut ax) = (tables().debris45_burst_speeds.at16(x.b, row), tables().debris45_burst_speeds.at16(x.b, row + 2));
            if x.u8(f::FLIP) == 0 {
                (vx, ax) = (neg(vx), neg(ax));
            }
            x.set16(0x14, vx);
            x.set16(0x16, ax);
            let vz = tables().debris45_burst_speeds.at16(x.b, row + 4);
            x.set16(0x18, vz);
            x.set16(0x1a, 0);
            let vy = tables().debris45_burst_speeds.at16(x.b, row + 6);
            x.set16(0x1c, vy);
            x.set16(0x1e, 0);
            let anim = tables().debris45_burst_anims.at8(x.b, r & 3);
            ai_anim(x, ANIMS, anim as u32);
            x.set8(f::BOX_SET, 7);
            x.set8(f::ATTACK_POWER, 0);
        }
        2 => {
            if ai_step(x) {
                return vanish(x);
            }
            if x.u8(0x5) == 0 && x.u8(f::HIT_FLAGS) & 0x8 == 0 && x.u8(0x33) == 0 {
                x.add8(0x5, 2);
                x.set32(0x14, 0);
                x.set32(0x1c, 0);
                let r = random(x) & 6;
                let vz = tables().debris45_hop_vz.at16(x.b, r);
                x.set16(0x18, vz);
                x.set16(0x1a, 0);
                ai_anim(x, ANIMS, 0x11);
            }
            x.move_xyz();
            settle(x);
        }
        4 => x.add8(0x4, 2),
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
    match x.u8(0x4) {
        0 => {
            x.add8(0x4, 2);
            x.set8(0x5, 0);
            let flip = copy_flip(x, src);
            if flip != 0 {
                let k = x.u8(0x2f) as u32;
                let mirrored = tables().debris45_mirror.at8(x.b, k);
                x.set8(0x2f, mirrored);
            }
            let k = x.u8(0x2f);
            x.set8(0x2d, k.wrapping_mul(4));
            x.select_speed(&crate::objects::motion::SPEEDS_A, 0x1f);
            place(x, src);
            x.set8(f::ATTACK_ID, 0x14);
            x.anim_start(ANIMS, 0x1f);
        }
        2 => {
            match x.u8(0x5) {
                0 => {
                    if src.u8(x.b, 0xa4) == 4 {
                        x.anim_step();
                    } else {
                        x.add8(0x5, 2);
                        x.set32(0x14, 0);
                        x.set32(0x1c, 0);
                        x.set16(0x18, neg(0x300));
                        x.set16(0x1a, 0);
                        x.anim_start(ANIMS, 0x21);
                    }
                }
                2 => {
                    x.anim_step();
                    x.move_by_table_xy();
                    x.move_z();
                    if s16(x.u16(0xc)) < 0 {
                        x.set16(0xc, 0);
                        x.add8(0x4, 2);
                    }
                }
                _ => {}
            }
            settle(x);
        }
        4 => x.add8(0x4, 2),
        6 => x.free_eshl(),
        _ => {}
    }
}

/// Places the burst on the source: X and Y of the source, Z + 0x30, then offset according to `+0x2f`.
fn place<R: Bench>(x: &mut Cx<'_, R>, src: Obj) {
    let (sx, sy, sz) = (src.u16(x.b, 0x8), src.u16(x.b, 0x10), src.u16(x.b, 0xc));
    x.set16(0x8, sx);
    x.set16(0x10, sy);
    x.set16(0xc, sz.wrapping_add(0x30));
    let k = 2 * (x.u8(0x2f) & 7) as u32;
    let dx = tables().debris45_place_offsets.at16(x.b, k);
    // the Z row is the same table read with a two-byte offset, indexed by dx
    let dz = tables().debris45_place_offsets.at16(x.b, 2u32.wrapping_add(dx as i16 as i32 as u32));
    x.add16(0x8, dx);
    x.add16(0xc, dz);
}
