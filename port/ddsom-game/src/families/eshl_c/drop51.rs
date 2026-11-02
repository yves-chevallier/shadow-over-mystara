use super::*;

const ANIMS: u32 = 0x16ea18;
const LAUNCHER: u32 = 0x5c;
const WAIT: u32 = 0x42;
/// Top of the screen: `A5+0x50a` (player 1) or `A5+0x55a` (player 2, if `A5+0x36`).
const SCREEN_TOP: u32 = 0x50a;
const SCREEN_TOP_P2: u32 = 0x55a;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => {
            if wait(x) {
                return;
            }
            x.add8(0x4, 2);
            x.set16(WAIT, 0x1e);
            x.set8(f::BOX_SET, 7);
            x.set8(f::ATTACK_ID, 0x17);
            let top = if x.a5_u8(0x36) != 0 { SCREEN_TOP_P2 } else { SCREEN_TOP };
            let top = x.a5_u16(top);
            let depth = x.u16(0x10);
            x.set16(0xc, top.wrapping_add(0x100).wrapping_sub(depth));
            x.anim_start(ANIMS, 0);
            x.reinsert();
        }
        2 => {
            if x.u8(0x5) == 0 {
                if wait(x) {
                    return;
                }
                x.add8(0x5, 2);
                x.set16(0x18, neg(0x100));
                x.set16(0x1a, neg(0x40));
                return;
            }
            x.anim_step();
            x.move_z();
            if s16(x.u16(0xc)) >= 0 {
                if x.u8(f::HIT_FLAGS) & 0x8 != 0 {
                    x.set8(f::HIT_FLAGS, 0);
                    let launcher = x.linked(LAUNCHER);
                    let n = launcher.u8(x.b, 0x1d).wrapping_add(1);
                    launcher.set_u8(x.b, 0x1d, n);
                }
                return;
            }
            x.add8(0x4, 2);
            crate::families::effects_creators::debris(x.b, x.o, 3);
            crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::B, 8);
        }
        4 => {
            x.add8(0x4, 2);
            if x.u8(0x2f) == 0 {
                let launcher = x.linked(LAUNCHER);
                let slot = 8u8.wrapping_add(x.u8(0x3)) as u32;
                launcher.set_u16(x.b, slot, 0);
            }
        }
        _ => x.free_eshl(),
    }
}

/// Countdown of the delay `+0x42`: true as long as it has not elapsed.
fn wait<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let t = x.u16(WAIT).wrapping_sub(1);
    x.set16(WAIT, t);
    t != 0
}
