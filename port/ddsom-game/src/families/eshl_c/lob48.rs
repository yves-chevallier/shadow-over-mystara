use super::*;

const ANIMS: u32 = 0x17041e;
const LAUNCHER: u32 = 0x5c;
const WAIT: u32 = 0x4c;
const HOME_Z: u32 = 0x4e;
const TARGET: u32 = 0x58;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => {
            x.add8(0x4, 2);
            x.set16(WAIT, 0x10);
            let z = x.u16(0xc);
            x.set16(HOME_Z, z);
            x.set8(f::BOX_SET, 7);
            x.set8(f::ATTACK_ID, 0x17);
            x.anim_start(ANIMS, 1);
            x.reinsert();
        }
        2 => {
            sub_state(x);
            if x.u8(f::HIT_FLAGS) & 0x8 != 0 {
                x.set8(f::HIT_FLAGS, 0);
                let launcher = x.linked(LAUNCHER);
                let n = launcher.u8(x.b, 0x1d).wrapping_add(1);
                launcher.set_u8(x.b, 0x1d, n);
            }
        }
        4 => {
            x.add8(0x4, 2);
            let launcher = x.linked(LAUNCHER);
            let slot = (8u8.wrapping_add(x.u8(0x3))) as u32;
            launcher.set_u16(x.b, slot, 0);
        }
        _ => x.free_eshl(),
    }
}

fn sub_state<R: Bench>(x: &mut Cx<'_, R>) {
            match x.u8(0x5) {
                0 => {
                    if dec_wait(x) {
                        return;
                    }
                    x.add8(0x5, 2);
                    x.set16(0x18, neg(0x100));
                    x.set16(0x1a, neg(0x20));
                    // Original glitch: 0x54872 returns D0 = 0 even when the shot is created, so the `bne` that was
                    // supposed to hold it in `TARGET` is never taken
                    crate::families::small::shot_18(&mut crate::families::effects_creators::leftovers::Pure, x.b, x.o, 2);
                }
                2 => {
                    x.move_z();
                    if s16(x.u16(0xc)) <= 0x20 {
                        x.add8(0x5, 2);
                        x.set16(WAIT, 0x10);
                    }
                }
                4 => {
                    if dec_wait(x) {
                        return;
                    }
                    x.add8(0x5, 2);
                    x.set16(0x18, 0x180);
                    x.set16(0x1a, 0);
                }
                6 => {
                    x.move_z();
                    if s16(x.u16(HOME_Z)) > s16(x.u16(0xc)) {
                        return;
                    }
                    x.add8(0x4, 2);
                    if x.u16(TARGET) != 0 {
                        let t = x.linked(TARGET);
                        t.set_u8(x.b, 0x4, 6);
                    }
                }
                _ => {}
            }
}

/// Countdown of the delay `+0x4c`: true as long as it has not elapsed.
fn dec_wait<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let t = x.u16(WAIT).wrapping_sub(1);
    x.set16(WAIT, t);
    t != 0
}
