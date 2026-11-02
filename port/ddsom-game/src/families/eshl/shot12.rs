use super::*;
use crate::families::shell::tables::{tables, Read};

const ANIMS: u32 = 0x16dda2;
/// Attachment point (RAM address on 16 bits).
const ANCHOR: u32 = 0x5a;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => {
            x.add8(0x4, 2);
            attach(x);
            let variant = x.u8(0x3) as u32;
            let anim = tables().shot12_anims_of_variant.at8(x.b, variant);
            x.anim_start(ANIMS, (anim as u32) as u16);
            animate(x);
        }
        2 => animate(x),
        _ => x.free_eshl(),
    }
}

fn animate<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(0x5) == 0 {
        x.anim_step();
    }
    x.reinsert();
}

fn attach<R: Bench>(x: &mut Cx<'_, R>) {
    let anchor = x.linked(ANCHOR);
    for off in [0x8, 0xc, 0x10] {
        let v = anchor.u16(x.b, off);
        x.set16(off, v);
    }
    x.add32(0x10, u32::MAX);
    let row = (4 * x.u8(0x3) as u32) & 0xff;
    let (dx, dz) = (tables().shot12_offsets.at16(x.b, row), tables().shot12_offsets.at16(x.b, row + 2));
    let flip = anchor.u8(x.b, f::FLIP) != 0;
    x.add16(0x8, if flip { dx } else { dx.wrapping_neg() });
    x.add16(0xc, dz);
}
