use super::*;
use crate::world::objects::Obj;
use crate::actors::families::set::fields::FadeWhiteObj;

const PALETTE_BLOCKS: [(u32, u32); 4] = [(0x91c100, 128), (0x91c720, 32), (0x91c800, 32), (0x91cc00, 32)];

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => {
            x.add8(Obj::STATE, 2);
            x.set16(FadeWhiteObj::STEPS, 0xf);
        }
        2 => {
            for (addr, count) in PALETTE_BLOCKS {
                util::brighten(x, addr, count);
            }
        }
        _ => x.free(&pools::SET),
    }
}
