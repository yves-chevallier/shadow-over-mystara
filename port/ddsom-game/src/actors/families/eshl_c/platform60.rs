use super::*;
use crate::world::data::RomAddr;
use crate::actors::families::shell::vars;
use crate::actors::families::shell::records::view;
use crate::actors::families::shell::tables::{tables, Read};

const ANIMS: RomAddr = RomAddr(0x1712d4);
const LINK: u32 = 0x5c;
const TIMER: u32 = 0x42;
const RELOAD: u32 = 0x4c;
const HOME_Z: u32 = 0x4e;
/// Last vertical scroll (word), and its new reading.
const LAST_SCROLL: u32 = 0x16;
const SCROLL: u32 = 0x14;
const THUD: crate::services::sound::Sound = crate::services::sound::CMD_37D_47D;
/// Substates (`+5`).
const WAIT: u8 = 0;
const SINK: u8 = 2;
const REST: u8 = 4;
const RISE: u8 = 6;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    match x.u8(Obj::STATE) {
        0 => start(x),
        2 => live(x),
        _ => x.free_eshl(),
    }
}

fn start<R: Bench>(x: &mut Cx<'_, R>) {
    let top = screen_top(x);
    x.set16(LAST_SCROLL, top);
    match x.u8(Obj::VARIANT) {
        2 => {
            if x.u16(LINK) == 0 {
                return;
            }
            x.add8(Obj::STATE, 2);
            let row = 4 * x.u8(Base::SETTING) as u32;
            let (px, py) = (tables().platform60_start_positions.at16(x.b, row), tables().platform60_start_positions.at16(x.b, row + 2));
            x.set16(Obj::X, px);
            x.set16(Obj::Y, py);
            x.set16(Obj::Z, 0x110);
            x.set16(HOME_Z, 0x110);
            x.set16(TIMER, 0x1e);
            x.set16(RELOAD, 0x1e);
        }
        0 | 1 | 3 => {
            x.add8(Obj::STATE, 2);
            x.set16(Obj::Z, 0x110);
            x.set16(HOME_Z, 0x110);
            let t = x.u16(TIMER);
            x.set16(RELOAD, t);
        }
        _ => return,
    }
    x.set8(Base::BOX_SET, 7);
    x.set8(Shot::ATTACK_ID, 0x17);
    x.anim_start(ANIMS.0, 0);
    crate::actors::families::small::shot_18(&mut crate::actors::families::effects_creators::leftovers::Pure, x.b, x.o, 2);
    x.reinsert();
}

fn live<R: Bench>(x: &mut Cx<'_, R>) {
    // follows the vertical scroll of the screen
    let top = screen_top(x);
    x.set16(SCROLL, top);
    let delta = top.wrapping_sub(x.u16(LAST_SCROLL));
    x.add16(Obj::Z, delta);
    x.set16(LAST_SCROLL, top);
    match x.u8(Obj::VARIANT) {
        0 => cycle(x, Trigger::Timer),
        1 => cycle(x, Trigger::Player { signal: false }),
        3 => cycle(x, Trigger::Player { signal: true }),
        2 => {
            if x.u16(LINK) != 0 {
                x.set8(Platform60::WAITING, 0);
                let maker = x.linked(LINK);
                if maker.u16(x.b, Obj::STATE) == 0x204 {
                    x.set8(Platform60::WAITING, 1);
                }
                cycle(x, Trigger::Maker);
            }
        }
        _ => {}
    }
    let layer = if x.u8(Base::SUB_STATE) == 0 {
        0
    } else {
        let z = s16(x.u16(Obj::Z));
        if z >= 0xb0 {
            0x60
        } else if z >= 0x60 {
            0x5f
        } else {
            0xc
        }
    };
    x.set8(Base::GROUND_MARK, layer);
}

/// What triggers the descent.
#[derive(Clone, Copy, PartialEq)]
enum Trigger {
    Timer,
    Player { signal: bool },
    Maker,
}

fn cycle<R: Bench>(x: &mut Cx<'_, R>, trigger: Trigger) {
    match x.u8(Base::SUB_STATE) {
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
                        vars::CAMERA_MODE.set(x.b, 4);
                    }
                    on
                }
                Trigger::Maker => x.u8(Platform60::WAITING) == 0 && {
                    let t = x.u16(TIMER).wrapping_sub(1);
                    x.set16(TIMER, t);
                    t == 0
                },
            };
            if go {
                x.add8(Base::SUB_STATE, 2);
                x.set16(Obj::VZ, neg(0x100));
                x.set16(Obj::AZ, neg(0x20));
            }
        }
        SINK => {
            x.move_z();
            let floor = if trigger == Trigger::Maker || x.u8(Base::SETTING) == 0 { 0x20 } else { 0x38 };
            if s16(x.u16(Obj::Z)) <= floor {
                crate::services::sound::play(x.b, THUD, x.o);
                x.add8(Base::SUB_STATE, 2);
                x.set16(TIMER, 0x10);
            }
        }
        REST => {
            let t = x.u16(TIMER).wrapping_sub(1);
            x.set16(TIMER, t);
            if t == 0 {
                x.add8(Base::SUB_STATE, 2);
                x.set16(Obj::VZ, 0x180);
                x.set16(Obj::AZ, 0);
            }
        }
        RISE => {
            x.move_z();
            if s16(x.u16(HOME_Z)) > s16(x.u16(Obj::Z)) {
                return;
            }
            let n = x.u8(Base::SUB_STATE).wrapping_sub(6);
            x.set8(Base::SUB_STATE, n);
            match trigger {
                Trigger::Timer | Trigger::Maker => {
                    let reload = x.u16(RELOAD);
                    x.set16(TIMER, reload);
                }
                Trigger::Player { signal: true } => vars::CAMERA_MODE.set(x.b, 0),
                Trigger::Player { signal: false } => {}
            }
        }
        _ => {}
    }
}

/// Is a player (other than character 5) on the platform? (collision 0x7f19c.)
fn player_on<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    for p in players() {
        let character = p.u8(x.b, Obj::VARIANT) >> 1;
        if character == 5 || p.u8(x.b, Obj::ACTIVE) != 1 {
            continue;
        }
        let hitbox = crate::actors::ai_aids::Hitbox { offset: 0, half_width: 0x28, half_depth: 8 };
        if crate::actors::ai_aids::box_around(x.b, x.o, p, hitbox) != crate::actors::ai_aids::InHitbox::No {
            return true;
        }
    }
    false
}

view! {
    /// Fields of the platform (type 60).
    Platform60 {
        /// Non-zero while waiting for its maker (h).
        waiting, set_waiting, WAITING: u8 @ 0x1c;
    }
}
