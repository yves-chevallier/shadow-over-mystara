use super::*;
use crate::world::objects::Obj;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Obj::HIDDEN) != 0 {
        return;
    }
    match x.u8(Obj::STATE) {
        0 => x.add8(Obj::STATE, 2),
        2 => {}
        _ => x.free(&pools::SET),
    }
}
