use super::*;

const ANIMS: u32 = 0x16ea18;
/// Delay before the fall (bytes) per variant (32 entries).
const DELAYS: u32 = 0xa679e;
const IMPACT_SOUND: crate::sound::Sound = crate::sound::CMD_34D_44D;
const TIMER: u32 = 0x42;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => start(x),
        2 => live(x),
        4 => x.add8(0x4, 2),
        _ => x.free_eshl(),
    }
}

fn start<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x4, 2);
    x.set8(0x5, 0);
    x.set8(f::BOX_SET, 7);
    x.set8(f::ATTACK_ID, 0x25);
    // camera: block of A5+0x4fe (player 1) or A5+0x54e (player 2)
    let camera = if x.a5_u8(0x36) != 0 { 0x54e } else { 0x4fe };
    x.set16(0xc, 0x100);
    let mut dx = random(x) as u16 & 0x1ff;
    if s16(dx) > 0x180 {
        dx = (dx >> 1) + 0x40;
    }
    let px = dx.wrapping_add(x.a5_u16(camera + 8));
    x.set16(0x8, px);
    let mut dy = random(x) as u16 & 0x7f;
    if s16(dy) > 0x50 {
        dy = (dy >> 1) + 0x10;
    }
    let py = dy.wrapping_add(0x10).wrapping_add(x.a5_u16(camera + 0xc));
    x.set16(0x10, py);
    x.set16(0x18, neg(0x200));
    x.set16(0x1a, neg(0x20));
    x.set32(0x14, 0);
    x.set32(0x1c, 0);
    let variant = x.u8(0x3) & 0x1f;
    let delay = x.rom8(DELAYS + variant as u32);
    x.set8(TIMER, delay);
    x.anim_start(ANIMS, 0);
}

fn live<R: Bench>(x: &mut Cx<'_, R>) {
    if s8(x.u8(TIMER)) > 0 {
        let t = x.u8(TIMER) - 1;
        return x.set8(TIMER, t);
    }
    x.anim_step();
    x.move_z();
    if s16(x.u16(0xc)) < 0 {
        crate::families::effects_creators::linked(x.b, x.o, crate::families::effects_creators::kind::WISP, Some(1));
        x.set16(0xc, 0);
        x.add8(0x4, 2);
        crate::families::effects_creators::debris(x.b, x.o, 3);
        crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::B, 8);
        crate::sound::play(x.b, IMPACT_SOUND, x.o);
    }
    x.keep_on_screen(0x10);
    x.reinsert();
}
