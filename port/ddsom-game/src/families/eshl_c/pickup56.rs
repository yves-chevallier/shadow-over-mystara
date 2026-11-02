use super::*;
use crate::data::RomAddr;
use crate::families::shell::vars;
use crate::ram;
use crate::families::shell::records::Host;
use crate::families::shell::tables::{tables, Read};

const ANIMS: RomAddr = RomAddr(0x170c3e);
/// Creation animation per object type (variant / 2).
const INIT_ANIMS: [u32; 6] = [2, 4, 5, 5, 4, 7];
const PICKUP_SOUND: crate::sound::Sound = crate::sound::CMD_36E_46E;
const COUNTDOWN: u32 = 0x56;
const STAGE_NO_CULL: u8 = 0x19;
const LINK: u32 = 0x5c;
const HIDDEN: u32 = 0x40;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => {
            let v = x.u8(Obj::VARIANT);
            if v < 12 {
                init(x, v);
            }
            x.reinsert();
        }
        2 => live(x),
        _ => x.free_eshl(),
    }
}

fn init<R: Bench>(x: &mut Cx<'_, R>, v: u8) {
    let k = v as usize / 2;
    x.add8(Obj::STATE, 2);
    x.set8(Base::BOX_SET, 7);
    x.set8(Base::OWN_PALETTE, 1);
    x.set8(Base::PALETTE, 0x14);
    x.set8(Shot::ATTACK_ID, 0x21);
    if v != 2 * k as u8 {
        x.set8(Base::PALETTE, 0x13);
        x.set8(Shot::ATTACK_ID, 0x22);
    }
    x.anim_start(ANIMS.0, (INIT_ANIMS[k]) as u16);
    if k == 3 {
        random_delay(x);
    }
}

/// Delay drawn from table 0xa55b2.
fn random_delay<R: Bench>(x: &mut Cx<'_, R>) {
    let r = x.random() as u32 & 0xf;
    let d = tables().pickup56_delays.at16(x.b, 2 * r);
    x.set16(COUNTDOWN, d);
}

fn live<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(HIDDEN) != 0 {
        return x.reinsert();
    }
    let v = x.u8(Obj::VARIANT);
    match v / 2 {
        0 => touch_then_pick(x, true),
        1 => flash_animate(x),
        2 => touch_then_pick(x, false),
        3 => wait_and_replay(x),
        4 => hp_dependent(x),
        5 => flash_animate(x),
        _ => {}
    }
    if v < 12 && ram::SCENE.get(x.b) != STAGE_NO_CULL {
        let edge = camera(x.b).0.get(x.b).wrapping_sub(0x200);
        if !(s16(edge) < s16(x.u16(Obj::X))) {
            x.set8(Obj::STATE, 6);
        }
    }
    x.reinsert();
}

/// Makes the object visible then hides it every other frame (except if `A5+0x33`).
fn flash<R: Bench>(x: &mut Cx<'_, R>) {
    x.set_bit(Obj::FLAGS, 1, false);
    if vars::BLINK_SUSPENDED.get(x.b) == 0 && vars::FRAME_BYTE.get(x.b).wrapping_add(x.counter() as u8) & 1 != 0 {
        x.set_bit(Obj::FLAGS, 1, true);
    }
}

fn flash_animate<R: Bench>(x: &mut Cx<'_, R>) {
    flash(x);
    x.anim_step();
}

/// A player on the ground, close enough, picks up the object: next `+5`, 30-frame countdown.
fn pickup_touch<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(HIDDEN) != 0 || vars::GAME_PAUSED.get(x.b) != 0 {
        return;
    }
    for p in players() {
        if p.u8(x.b, Obj::ACTIVE) == 0 || p.u16(x.b, Obj::Z) != 0 {
            continue;
        }
        let dy = x.u16(Obj::Y).wrapping_sub(p.u16(x.b, Obj::Y)).wrapping_add(6);
        if dy > 0xc {
            continue;
        }
        let dx = x.u16(Obj::X).wrapping_sub(p.u16(x.b, Obj::X)).wrapping_add(8);
        if dx > 0x10 {
            continue;
        }
        x.add8(Base::SUB_STATE, 2);
        x.set16(COUNTDOWN, 0x1e);
        return;
    }
}

/// Pickup countdown: blinks every 8 steps; `true` when it has elapsed.
fn pick_countdown<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let t = x.u16(COUNTDOWN).wrapping_sub(1);
    x.set16(COUNTDOWN, t);
    if t == 0 {
        x.add8(Base::SUB_STATE, 2);
        x.set8(Obj::FLAGS, 1);
        return true;
    }
    if t & 7 == 0 {
        let flags = x.u8(Obj::FLAGS);
        x.set8(Obj::FLAGS, flags ^ 2);
    }
    false
}

/// Variants 0, 1 (`with_effects`) and 4, 5: wait for the pickup, blink, play the end animation.
fn touch_then_pick<R: Bench>(x: &mut Cx<'_, R>, with_effects: bool) {
    match x.u8(Base::SUB_STATE) {
        0 => pickup_touch(x),
        2 => {
            if pick_countdown(x) && with_effects {
                x.anim_start(ANIMS.0, 3);
                crate::sound::play(x.b, PICKUP_SOUND, x.o);
            }
        }
        4 => {
            if x.anim_step() {
                x.add8(Obj::STATE, 2);
            }
        }
        _ => {}
    }
}

/// Variants 6, 7: wait for a delay (outside pause), then replay animation 5 with a new delay.
fn wait_and_replay<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Base::SUB_STATE) == 0 {
        if x.u8(HIDDEN) != 0 || vars::GAME_PAUSED.get(x.b) != 0 {
            return;
        }
        let t = x.u16(COUNTDOWN).wrapping_sub(1);
        x.set16(COUNTDOWN, t);
        if t == 0 {
            x.add8(Base::SUB_STATE, 2);
        }
    } else if x.anim_step() {
        x.set8(Base::SUB_STATE, 0);
        random_delay(x);
        x.anim_start(ANIMS.0, 5);
    }
}

/// Variants 8, 9: alive as long as the creator's HP are non-zero; at its death, animation 6, then blinks for 32
/// frames before disappearing.
fn hp_dependent<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Base::SUB_STATE) {
        0 => {
            let maker = x.linked(LINK);
            if maker.u16(x.b, Host::LIFE) != 0 {
                flash_animate(x);
            } else {
                x.add8(Base::SUB_STATE, 2);
                x.anim_start(ANIMS.0, 6);
            }
        }
        2 => {
            if x.anim_step() {
                x.add8(Base::SUB_STATE, 2);
                x.set16(COUNTDOWN, 0x20);
            }
            flash(x);
        }
        4 => {
            x.set_bit(Obj::FLAGS, 1, false);
            let t = x.u16(COUNTDOWN).wrapping_sub(1);
            x.set16(COUNTDOWN, t);
            if t == 0 {
                x.add8(Obj::STATE, 2);
            }
            if x.u16(COUNTDOWN) & 1 != 0 {
                x.set_bit(Obj::FLAGS, 1, true);
            }
        }
        _ => {}
    }
}
