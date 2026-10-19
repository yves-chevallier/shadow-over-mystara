//! Boss 18, groupe `+5` = 0 : arrivée (quatre temps minutés par `+0xa6`, avec leurs sons), puis le combat commence.
//!
//! | `+6` | Rôle |
//! |---|---|
//! | 0 | attend (0x3c images), son 0x3e |
//! | 2 | apparaît (le boss devient visible), son 0x7a, animation 0 |
//! | 4 | son 0x24, animation 1 |
//! | 6 | prend la main : immunités, puis marche vers un joueur |

use super::decide;
use super::*;

pub(super) fn run(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            if sub16(x, TIMER, 1).gt() {
                return;
            }
            x.add8(f::SUB2, 2);
            x.set16(TIMER, 0x3c);
            x.c.d[0] = 0x3e;
            x.call(call::SOUND_EVENT);
        }
        2 => {
            if sub16(x, TIMER, 1).gt() {
                return;
            }
            x.add8(f::SUB2, 2);
            x.set_bit(0x1, 1, false);
            x.set16(TIMER, 0x78);
            x.c.d[0] = 0x7a;
            x.call(call::SOUND_EVENT);
            x.call(0xecf10);
            x.call(0xe13f0);
            body(x, 0);
        }
        4 => {
            if sub16(x, TIMER, 1).gt() {
                step(x);
                return;
            }
            x.add8(f::SUB2, 2);
            x.set16(TIMER, 0x28);
            x.c.d[0] = 0x24;
            x.call(call::SOUND_EVENT);
            body(x, 1);
        }
        6 => {
            if sub16(x, TIMER, 1).gt() {
                step(x);
                return;
            }
            x.set8(0, 1);
            x.call(rom::IMMUNITIES);
            x.call(0xd5f30);
            decide::chase(x);
        }
        _ => {}
    }
}
