use super::*;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(0x40) != 0 {
        return;
    }
    match x.u8(0x4) {
        0 => x.add8(0x4, 2),
        2 => {}
        _ => x.free(&pools::SET),
    }
}
