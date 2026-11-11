use super::*;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Obj::STATE) == 0 {
        x.add8(Obj::STATE, 2);
    }
    x.free(&pools::SOBJ);
}
