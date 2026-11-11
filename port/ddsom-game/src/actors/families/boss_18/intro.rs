//! Boss 18, group `+5` = 0: arrival (four beats timed by `+0xa6`, with their sounds), then the combat begins.
//!
//! | `+6` | Role |
//! |---|---|
//! | 0 | waits (0x3c frames), sound 0x3e |
//! | 2 | appears (the boss becomes visible), sound 0x7a, animation 0 |
//! | 4 | sound 0x24, animation 1 |
//! | 6 | takes over: immunities, then walks toward a player |

use super::decide;
use crate::actors::families::cx::Bench;
use super::*;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            if sub16(x, Boss::TIMER, 1).gt() {
                return;
            }
            x.add8(f::SUB2, 2);
            x.set16(Boss::TIMER, 0x3c);
            lib::shake_54(x, 0x3e);
        }
        2 => {
            if sub16(x, Boss::TIMER, 1).gt() {
                return;
            }
            x.add8(f::SUB2, 2);
            x.set_bit(Enemy::DISPLAY, 1, false);
            x.set16(Boss::TIMER, 0x78);
            lib::shake_54(x, 0x7a);
            lib::sound(x, crate::services::sound::CMD_331);
            lib::sound(x, crate::services::sound::CMD_23);
            body(x, 0);
        }
        4 => {
            if sub16(x, Boss::TIMER, 1).gt() {
                lib::step(x);
                return;
            }
            x.add8(f::SUB2, 2);
            x.set16(Boss::TIMER, 0x28);
            lib::shake_54(x, 0x24);
            body(x, 1);
        }
        6 => {
            if sub16(x, Boss::TIMER, 1).gt() {
                lib::step(x);
                return;
            }
            x.set8(Enemy::ACTIVE, 1);
            lib::set_immunities(x);
            lib::create_set_14(x);
            decide::chase(x);
        }
        _ => {}
    }
}
