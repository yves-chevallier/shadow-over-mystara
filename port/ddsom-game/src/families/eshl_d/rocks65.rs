use super::*;

/// Per variant: animation set, attack identifier, waits per rank (bytes, program ROM).
const ANIMS: [u32; 2] = [0x16ea18, 0x171574];
const ATTACK: [u8; 2] = [0x25, 0x38];
const DELAYS: [u32; 2] = [0xa739a, 0xa74b6];
const TIMER: u32 = 0x42;
const RANK: u32 = 0x2f;
/// Corner of the screen (player 1, or player 2 if `A5+0x36`).
const SCREEN: u32 = 0x4fe;
const SCREEN_P2: u32 = 0x54e;
const SFX: [crate::sound::Sound; 2] = [crate::sound::CMD_34D_44D, crate::sound::CMD_330];

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    let v = (x.u8(0x3) != 0) as usize;
    // variant 1 has one more state (end of animation) before the release
    let state = x.u8(0x4);
    match (state, v) {
        (0, _) => init(x, v),
        (2, _) => fall(x, v),
        (4, 1) => {
            if x.anim_step() {
                x.add8(0x4, 2);
            }
        }
        (4, 0) | (6, 1) => x.add8(0x4, 2),
        _ => x.free_eshl(),
    }
}

fn init<R: Bench>(x: &mut Cx<'_, R>, v: usize) {
    x.add8(0x4, 2);
    x.set8(0x5, 0);
    x.set8(f::BOX_SET, 7);
    x.set8(f::ATTACK_ID, ATTACK[v]);
    let screen = if x.a5_u8(0x36) != 0 { SCREEN_P2 } else { SCREEN };
    x.set16(0xc, 0x100);
    let r = random(x) & 0x1ff;
    let dx = if r > 0x180 { (r >> 1) + 0x40 } else { r };
    let left = x.a5_u16(screen + 8);
    x.set16(0x8, dx.wrapping_add(left));
    let r = random(x) & 0x7f;
    let dz = if r > 0x50 { (r >> 1) + 0x10 } else { r } + 0x10;
    let top = x.a5_u16(screen + 0xc);
    x.set16(0x10, dz.wrapping_add(top));
    x.set16(0x18, 0xfe00);
    x.set16(0x1a, 0xffe0);
    x.set32(0x14, 0);
    x.set32(0x1c, 0);
    let rank = (x.u8(RANK) & 0x1f) as u32;
    let delay = x.rom8(DELAYS[v] + rank);
    x.set16(TIMER, delay as u16);
    x.anim_start(ANIMS[v], 0);
}

/// Pseudo-random draw (low word of D0).
fn random<R: Bench>(x: &mut Cx<'_, R>) -> u16 {
    x.random()
}

fn fall<R: Bench>(x: &mut Cx<'_, R>, v: usize) {
    if x.u16(TIMER) as i16 > 0 {
        x.add16(TIMER, 0xffff);
        return;
    }
    x.anim_step();
    x.move_z();
    if x.u16(0xc) as i16 > 0 {
        x.keep_on_screen(16);
        x.reinsert();
        return;
    }
    x.set16(0xc, 0);
    x.add8(0x4, 2);
    if v == 0 {
        crate::families::effects_creators::debris(x.b, x.o, 3);
    } else {
        x.anim_start(ANIMS[1], 1);
    }
    crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::B, 8);
    crate::sound::play(x.b, SFX[v], x.o);
}
