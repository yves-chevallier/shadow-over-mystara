use super::*;

const ANIMS: u32 = 0x16ccea;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => {
            if x.a5_u8(GAME_PAUSED) != 0 {
                return;
            }
            if x.u16(f::OWNER) == 0 {
                return x.free_eshl();
            }
            x.add8(0x4, 2);
            let owner = x.linked(f::OWNER);
            x.set8(f::BOX_SET, 7);
            let (id, power) = if x.u8(0x3) == 0 { (0xc, 6) } else { (0xd, 7) };
            x.set8(f::ATTACK_POWER, power);
            x.set8(f::ATTACK_ID, id);
            place_in_front(x, owner, 0x40, 0x1b);
            let variant = x.u8(0x3) as u32;
            x.anim_start(ANIMS, variant as u16);
            x.reinsert();
        }
        2 => {
            if suspended(x) {
                return;
            }
            if x.u16(f::OWNER) == 0 || x.linked(f::OWNER).u8(x.b, 0x4) as i8 >= 4 {
                return x.free_eshl();
            }
            if x.a5_u8(GAME_PAUSED) != 0 {
                return;
            }
            if x.anim_step() {
                x.set8(0x4, st::FREE);
            }
        }
        _ => x.free_eshl(),
    }
}
