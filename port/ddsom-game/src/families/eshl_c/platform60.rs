use super::*;
use crate::families::shell::tables::{tables, Read};

const ANIMS: u32 = 0x1712d4;
const LINK: u32 = 0x5c;
const TIMER: u32 = 0x42;
const RELOAD: u32 = 0x4c;
const HOME_Z: u32 = 0x4e;
/// Last vertical scroll (word), and its new reading.
const LAST_SCROLL: u32 = 0x16;
const SCROLL: u32 = 0x14;
const THUD: crate::sound::Sound = crate::sound::CMD_37D_47D;
/// Variable `A5+0x51e` (variant 3).
const SIGNAL: u32 = 0x51e;
/// Substates (`+5`).
const WAIT: u8 = 0;
const SINK: u8 = 2;
const REST: u8 = 4;
const RISE: u8 = 6;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    match x.u8(0x4) {
        0 => start(x),
        2 => live(x),
        _ => x.free_eshl(),
    }
}

fn start<R: Bench>(x: &mut Cx<'_, R>) {
    let top = screen_top(x);
    x.set16(LAST_SCROLL, top);
    match x.u8(0x3) {
        2 => {
            if x.u16(LINK) == 0 {
                return;
            }
            x.add8(0x4, 2);
            let row = 4 * x.u8(0x2f) as u32;
            let (px, py) = (tables().platform60_start_positions.at16(x.b, row), tables().platform60_start_positions.at16(x.b, row + 2));
            x.set16(0x8, px);
            x.set16(0x10, py);
            x.set16(0xc, 0x110);
            x.set16(HOME_Z, 0x110);
            x.set16(TIMER, 0x1e);
            x.set16(RELOAD, 0x1e);
        }
        0 | 1 | 3 => {
            x.add8(0x4, 2);
            x.set16(0xc, 0x110);
            x.set16(HOME_Z, 0x110);
            let t = x.u16(TIMER);
            x.set16(RELOAD, t);
        }
        _ => return,
    }
    x.set8(f::BOX_SET, 7);
    x.set8(f::ATTACK_ID, 0x17);
    x.anim_start(ANIMS, 0);
    crate::families::small::shot_18(&mut crate::families::effects_creators::leftovers::Pure, x.b, x.o, 2);
    x.reinsert();
}

fn live<R: Bench>(x: &mut Cx<'_, R>) {
    // follows the vertical scroll of the screen
    let top = screen_top(x);
    x.set16(SCROLL, top);
    let delta = top.wrapping_sub(x.u16(LAST_SCROLL));
    x.add16(0xc, delta);
    x.set16(LAST_SCROLL, top);
    match x.u8(0x3) {
        0 => cycle(x, Trigger::Timer),
        1 => cycle(x, Trigger::Player { signal: false }),
        3 => cycle(x, Trigger::Player { signal: true }),
        2 => {
            if x.u16(LINK) != 0 {
                x.set8(0x1c, 0);
                let maker = x.linked(LINK);
                if maker.u16(x.b, 0x4) == 0x204 {
                    x.set8(0x1c, 1);
                }
                cycle(x, Trigger::Maker);
            }
        }
        _ => {}
    }
    let layer = if x.u8(0x5) == 0 {
        0
    } else {
        let z = s16(x.u16(0xc));
        if z >= 0xb0 {
            0x60
        } else if z >= 0x60 {
            0x5f
        } else {
            0xc
        }
    };
    x.set8(0x3b, layer);
}

/// What triggers the descent.
#[derive(Clone, Copy, PartialEq)]
enum Trigger {
    Timer,
    Player { signal: bool },
    Maker,
}

fn cycle<R: Bench>(x: &mut Cx<'_, R>, trigger: Trigger) {
    match x.u8(0x5) {
        WAIT => {
            let go = match trigger {
                Trigger::Timer => {
                    let t = x.u16(TIMER).wrapping_sub(1);
                    x.set16(TIMER, t);
                    t == 0
                }
                Trigger::Player { signal } => {
                    let on = player_on(x);
                    if on && signal {
                        x.set_a5_u16(SIGNAL, 4);
                    }
                    on
                }
                Trigger::Maker => x.u8(0x1c) == 0 && {
                    let t = x.u16(TIMER).wrapping_sub(1);
                    x.set16(TIMER, t);
                    t == 0
                },
            };
            if go {
                x.add8(0x5, 2);
                x.set16(0x18, neg(0x100));
                x.set16(0x1a, neg(0x20));
            }
        }
        SINK => {
            x.move_z();
            let floor = if trigger == Trigger::Maker || x.u8(0x2f) == 0 { 0x20 } else { 0x38 };
            if s16(x.u16(0xc)) <= floor {
                crate::sound::play(x.b, THUD, x.o);
                x.add8(0x5, 2);
                x.set16(TIMER, 0x10);
            }
        }
        REST => {
            let t = x.u16(TIMER).wrapping_sub(1);
            x.set16(TIMER, t);
            if t == 0 {
                x.add8(0x5, 2);
                x.set16(0x18, 0x180);
                x.set16(0x1a, 0);
            }
        }
        RISE => {
            x.move_z();
            if s16(x.u16(HOME_Z)) > s16(x.u16(0xc)) {
                return;
            }
            let n = x.u8(0x5).wrapping_sub(6);
            x.set8(0x5, n);
            match trigger {
                Trigger::Timer | Trigger::Maker => {
                    let reload = x.u16(RELOAD);
                    x.set16(TIMER, reload);
                }
                Trigger::Player { signal: true } => x.set_a5_u16(SIGNAL, 0),
                Trigger::Player { signal: false } => {}
            }
        }
        _ => {}
    }
}

/// Is a player (other than character 5) on the platform? (collision 0x7f19c.)
fn player_on<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    for p in players() {
        let character = p.u8(x.b, 3) >> 1;
        if character == 5 || p.u8(x.b, 0) != 1 {
            continue;
        }
        let hitbox = crate::ai_aids::Hitbox { offset: 0, half_width: 0x28, half_depth: 8 };
        if crate::ai_aids::box_around(x.b, x.o, p, hitbox) != crate::ai_aids::InHitbox::No {
            return true;
        }
    }
    false
}
