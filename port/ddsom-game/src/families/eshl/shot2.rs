use super::*;

const ANIMS: u32 = 0x16caa6;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => {
            x.add8(0x4, 2);
            x.set8(f::BOX_SET, 7);
            x.set8(f::ATTACK_POWER, 3);
            if x.u16(f::OWNER) == 0 {
                x.set32(0x4, 0x0600_0000);
                return;
            }
            let owner = x.linked(f::OWNER);
            place_in_front(x, owner, 0x40, 0x20);
            let (anim, id) = if owner.u8(x.b, 0x2) != 0 { (1, 0xa) } else { (0, 9) };
            x.set8(f::ATTACK_ID, id);
            x.anim_start(ANIMS, anim);
            crate::sound::play(x.b, crate::sound::CMD_323_423, x.o);
            fly(x);
        }
        2 => fly(x),
        _ => x.free_eshl(),
    }
}

fn fly<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::HIT_FLAGS) & 0x28 != 0 {
        crate::sound::play(x.b, crate::sound::CMD_122_222, x.o);
        return x.free_eshl();
    }
    if blocked_by_terrain(x) {
        return x.free_eshl();
    }
    x.move_xz();
    x.cull_offscreen();
    x.reinsert();
}
