use super::*;
use crate::hw::mem::wr16;
use crate::world::objects::Obj;
use crate::actors::families::set_b::fields::TestSelectorObj;

const SCREEN: u32 = 0x912bc0;
const LAST: u8 = 0x32;
const ROW: u32 = 0x80;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => {
            x.add8(Obj::STATE, 2);
            x.set8(TestSelectorObj::CHOICE, 0);
        }
        2 => step(x),
        _ => x.free(&pools::SET),
    }
}

fn step<R: Bench>(x: &mut Cx<'_, R>) {
    let pressed = !x.a5_u8(8) & x.a5_u8(2);
    if pressed >> 4 & 1 != 0 {
        // 0xcc352 with A0 = the first player: its class is that of the SET object created
        let index = x.u8(TestSelectorObj::CHOICE);
        crate::actors::families::creators::set_a(x.b, Obj(PLAYERS), index);
    } else if pressed >> 5 & 1 != 0 {
        let n = x.u8(TestSelectorObj::CHOICE) + 1;
        x.set8(TestSelectorObj::CHOICE, if n > LAST { 0 } else { n });
        show(x);
    } else if pressed >> 6 & 1 != 0 {
        let n = x.u8(TestSelectorObj::CHOICE);
        x.set8(TestSelectorObj::CHOICE, if n == 0 { LAST } else { n - 1 });
        show(x);
    }
}

fn show<R: Bench>(x: &mut Cx<'_, R>) {
    let n = x.u8(TestSelectorObj::CHOICE) as u16;
    for (i, digit) in [n >> 4, n & 0xf].into_iter().enumerate() {
        wr16(x.b, SCREEN + ROW * i as u32, digit & 0xf);
        wr16(x.b, SCREEN + ROW * i as u32 + 2, 0);
    }
}
