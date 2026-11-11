use super::*;
use crate::world::data::RomAddr;
use crate::actors::families::shell::vars;
use crate::actors::families::shell::records::{Spawner, view};
use crate::actors::families::shell::tables::{tables, Read};

const ANIMS: RomAddr = RomAddr(0x16eec2);
const SOUND_START: crate::hw::sound::Sound = crate::hw::sound::CMD_32D;
const SOUND_END: crate::hw::sound::Sound = crate::hw::sound::CMD_337;
const SOURCE_ARMED: u32 = 0xaf;
const WAIT: u8 = 0;
const HOLD: u8 = 2;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => init(x),
        2 => active(x),
        4 => stop(x),
        _ => x.free_eshl(),
    }
}

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Shot::ATTACK_ID, 6);
    x.set8(Shot::ATTACK_POWER, 5);
    x.set8(Base::BOX_SET, 7);
    match x.u8(Obj::VARIANT) {
        2 => {
            x.add8(Obj::STATE, 2);
            follow(x);
            x.set16(Shot29::TIMER, 0x1e);
            x.anim_start(ANIMS.0, 3);
        }
        0 => {
            x.add8(Obj::STATE, 2);
            follow(x);
            x.anim_start(ANIMS.0, 0);
        }
        _ => {
            x.add16(Shot29::TIMER, 0xffff);
            if x.u16(Shot29::TIMER) != 0 {
                return;
            }
            x.add8(Obj::STATE, 2);
            x.anim_start(ANIMS.0, 1);
        }
    }
    x.reinsert();
}

/// End: the creator is dead or in state 0x20a.
fn stop<R: Bench>(x: &mut Cx<'_, R>) {
    crate::hw::sound::play(x.b, SOUND_END, x.o);
    x.add8(Obj::STATE, 2);
}

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    let source = x.linked(Shot29::SOURCE);
    if (source.u8(x.b, Obj::STATE) as i8) >= 4 || source.u16(x.b, Obj::STATE) == 0x20a {
        return stop(x);
    }
    match x.u8(Obj::VARIANT) {
        2 => {
            follow(x);
            x.add16(Shot29::TIMER, 0xffff);
            if (x.u16(Shot29::TIMER) as i16) < 0 {
                x.add8(Obj::STATE, 2);
            }
            x.anim_step();
        }
        0 => centre(x),
        _ => satellite(x, source),
    }
}

/// Central element: waits for the creator's signal then triggers the row.
fn centre<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Base::SUB_STATE) {
        WAIT => {
            let source = x.linked(Shot29::SOURCE);
            if source.u8(x.b, SOURCE_ARMED) != 0 {
                crate::hw::sound::play(x.b, SOUND_START, x.o);
                x.add8(Base::SUB_STATE, 2);
                x.set16(Shot29::TIMER, 0x50);
                let px = source.u16(x.b, Obj::X);
                let rightwards = source.u8(x.b, Base::FLIP) != 0;
                let start = if rightwards { px.wrapping_add(0x4d) } else { px.wrapping_sub(0x4d) };
                spawn_row(x, 0x1d, start, rightwards);
            } else {
                follow(x);
                x.anim_step();
                x.reinsert();
            }
        }
        HOLD => {
            x.add16(Shot29::TIMER, 0xffff);
            if x.u16(Shot29::TIMER) == 0 {
                x.add8(Obj::STATE, 2);
                x.set8(Base::SUB_STATE, 0);
                x.set8(Obj::ACTIVE, 2);
                let source = x.linked(Shot29::SOURCE);
                source.set_u8(x.b, SOURCE_ARMED, 0);
            }
            follow(x);
            x.anim_step();
            x.reinsert();
        }
        _ => {}
    }
}

/// Row element: blinks according to `A5+0x6f`, changes animation when the epicenter goes out.
fn satellite<R: Bench>(x: &mut Cx<'_, R>, source: Obj) {
    let flicker = |x: &mut Cx<'_, R>| {
        let hidden = vars::FRAME_BYTE.get(x.b) & 1 == 0;
        x.set_bit(Obj::FLAGS, 1, hidden);
    };
    match x.u8(Base::SUB_STATE) {
        WAIT => {
            x.anim_step();
            if source.u16(x.b, Spawner::TIMER) <= 0x30 {
                x.add8(Base::SUB_STATE, 2);
                x.anim_start(ANIMS.0, 2);
            }
            flicker(x);
        }
        HOLD => {
            if x.anim_step() {
                x.add8(Obj::STATE, 2);
            }
            flicker(x);
        }
        _ => {}
    }
}

/// Sticks to the creator; the offset depends on its attack state (`+7`).
fn follow<R: Bench>(x: &mut Cx<'_, R>) {
    let source = x.linked(Shot29::SOURCE);
    copy_position16(x, source);
    let flip = source.u8(x.b, Base::FLIP);
    x.set8(Base::FLIP, flip);
    let attack = source.u8(x.b, Base::SUB_STEP);
    if attack == 0x16 || attack == 0x18 {
        let index = source.u8(x.b, Base::ANIM_EVENT) as u32;
        let mut dx = tables().shot29_attach_shift.at16(x.b, index);
        if flip != 0 {
            dx = dx.wrapping_neg();
        }
        x.add16(Obj::X, dx);
        x.set16(Obj::Z, 0x1e);
    } else {
        let mut dx = -0x2di16 as u16;
        if flip != 0 {
            dx = dx.wrapping_neg();
        }
        x.add16(Obj::X, dx);
        x.set16(Obj::Z, 0x27);
    }
    x.add16(Obj::Y, 0xffff);
}

view! {
    /// Fields of shot 29 (a spawner, see `Spawner`).
    Shot29 {
        /// Number of created shots, row of the offsets (h).
        count, set_count, COUNT: u16 @ 0x56;
        /// Countdown of the current step (h).
        timer, set_timer, TIMER: u16 @ 0x58;
        /// Linked object: shadow or second timer (h).
        link, set_link, LINK: u16 @ 0x5a;
        /// Source object (16-bit pointer).
        source, set_source, SOURCE: u16 @ 0x5c;
        /// Row of the height tables (h).
        row, set_row, ROW: u8 @ 0x60;
    }
}
