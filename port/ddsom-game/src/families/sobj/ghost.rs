use super::*;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(0x4) == 0 {
        x.add8(0x4, 2);
    }
    x.free(&pools::SOBJ);
}
