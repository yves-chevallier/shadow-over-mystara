use super::*;
use crate::data::RomAddr;
use crate::families::shell::vars;

const ANIMS: RomAddr = RomAddr(0x16ccea);

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => {
            if vars::GAME_PAUSED.get(x.b) != 0 {
                return;
            }
            if x.u16(Shot::OWNER) == 0 {
                return x.free_eshl();
            }
            x.add8(Obj::STATE, 2);
            let owner = x.linked(Shot::OWNER);
            x.set8(Base::BOX_SET, 7);
            let (id, power) = if x.u8(Obj::VARIANT) == 0 { (0xc, 6) } else { (0xd, 7) };
            x.set8(Shot::ATTACK_POWER, power);
            x.set8(Shot::ATTACK_ID, id);
            place_in_front(x, owner, 0x40, 0x1b);
            let variant = x.u8(Obj::VARIANT) as u32;
            x.anim_start(ANIMS.0, variant as u16);
            x.reinsert();
        }
        2 => {
            if suspended(x) {
                return;
            }
            if x.u16(Shot::OWNER) == 0 || x.linked(Shot::OWNER).u8(x.b, Obj::STATE) as i8 >= 4 {
                return x.free_eshl();
            }
            if vars::GAME_PAUSED.get(x.b) != 0 {
                return;
            }
            if x.anim_step() {
                x.set8(Obj::STATE, st::FREE);
            }
        }
        _ => x.free_eshl(),
    }
}
