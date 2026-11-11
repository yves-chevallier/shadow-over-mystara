use super::*;
use crate::world::data::RomAddr;
use crate::actors::families::shell::tables::{tables, Read};

const ANIMS: RomAddr = RomAddr(0x170468);
const HOST: u32 = 0x42;
const SOUND: crate::services::sound::Sound = crate::services::sound::CMD_35B_45B;
const PHASE: u32 = 0x5a;
const PHASE_TIMER: u32 = 0x5b;
const FALL_TIMER: u32 = 0x58;
/// Saving of `+0x3b` (frame priority) across the animation.
const SAVED_PRIORITY: u32 = 0x5e;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    match x.u8(Obj::STATE) {
        0 => match x.u8(Obj::VARIANT) {
            0 => launch(x, 0x12, 0x70, 0x1e, neg(0x380), true),
            _ => launch(x, 0x17, 0x76, 0x2b, 0x40, false),
        },
        2 => match x.u8(Obj::VARIANT) {
            0 => lob(x),
            _ => slow(x),
        },
        _ => x.free_eshl(),
    }
    if x.u8(Obj::ACTIVE) != 0 {
        x.keep_in_zone();
        x.push_terrain();
        x.reinsert();
    }
}

/// Creation: variant 0 (`lob = true`) or 1.
fn launch<R: Bench>(x: &mut Cx<'_, R>, speed: u32, dx: u16, dz: u16, vz: u16, lob: bool) {
    x.add8(Obj::STATE, 2);
    if !lob {
        x.set16(FALL_TIMER, 0x28);
    }
    x.set8(Base::BOX_SET, 7);
    x.set8(Shot::ATTACK_POWER, 0x14);
    x.select_speed(&crate::world::objects::motion::SPEEDS_B, speed as u16);
    let host = x.linked(HOST);
    copy_xyz16(x, host);
    let flip = x.u8(Base::FLIP);
    x.add16(Obj::X, if flip != 0 { dx } else { neg(dx) });
    x.add16(Obj::Z, dz);
    x.add32(Obj::Y, 2);
    x.set16(Obj::VZ, vz);
    crate::services::sound::play(x.b, SOUND, x.o);
    start_phase(x);
}

/// Advances one phase when its duration has elapsed (and we are not already in phase 2).
fn phase_tick<R: Bench>(x: &mut Cx<'_, R>) {
    if s8(x.u8(PHASE)) < 2 {
        let n = x.u8(PHASE_TIMER).wrapping_sub(1);
        x.set8(PHASE_TIMER, n);
        if s8(n) <= 0 {
            x.add8(PHASE, 1);
            start_phase(x);
            return;
        }
    }
    step_phase(x);
}

/// Starts the animation of the current phase, then one step.
fn start_phase<R: Bench>(x: &mut Cx<'_, R>) {
    let phase = x.u8(PHASE);
    let len = tables().bullet50_phase_lengths.at8(x.b, phase as u32);
    x.set8(PHASE_TIMER, len);
    ai_anim(x, ANIMS.0, phase as u32 + 3);
    let p = x.u8(Base::GROUND_MARK);
    x.set8(SAVED_PRIORITY, p);
    step_phase(x);
}

fn step_phase<R: Bench>(x: &mut Cx<'_, R>) {
    ai_step(x);
    let p = x.u8(Base::GROUND_MARK);
    x.set8(SAVED_PRIORITY, p);
}

fn restore_priority<R: Bench>(x: &mut Cx<'_, R>) {
    let p = x.u8(SAVED_PRIORITY);
    x.set8(Base::GROUND_MARK, p);
}

/// The host is stunned: goes to the end animation (`phase + 6`), substate 4 for variant 0 (state 2, substate 4),
/// next substate for variant 1.
fn host_stunned<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let host = x.linked(HOST);
    host.u8(x.b, Obj::STATE) == 4
}

fn end_animation<R: Bench>(x: &mut Cx<'_, R>) {
    let phase = x.u8(PHASE) as u32;
    ai_anim(x, ANIMS.0, phase + 6);
}

fn lob<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Base::SUB_STATE) {
        0 => {
            if host_stunned(x) {
                x.set16(Obj::STATE, 0x204);
                return end_animation(x);
            }
            x.move_xyz();
            phase_tick(x);
            if s16(x.u16(Obj::Z)) > 0 {
                return restore_priority(x);
            }
            x.add8(Base::SUB_STATE, 2);
            x.select_speed(&crate::world::objects::motion::SPEEDS_B, 0x17);
            x.set16(FALL_TIMER, 0x1a);
            x.set16(Obj::Z, 0);
            restore_priority(x);
        }
        2 => {
            if host_stunned(x) {
                x.set16(Obj::STATE, 0x204);
                return end_animation(x);
            }
            x.move_xy();
            let t = x.u16(FALL_TIMER).wrapping_sub(1);
            x.set16(FALL_TIMER, t);
            if s16(t) <= 0 {
                x.set16(Obj::STATE, 0x204);
                return end_animation(x);
            }
            phase_tick(x);
            restore_priority(x);
        }
        4 => {
            if ai_step(x) {
                x.free_eshl();
            }
        }
        _ => {}
    }
}

fn slow<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Base::SUB_STATE) {
        0 => {
            if host_stunned(x) {
                return end_slow(x);
            }
            x.move_xyz();
            phase_tick(x);
            let t = x.u16(FALL_TIMER).wrapping_sub(1);
            x.set16(FALL_TIMER, t);
            if s16(t) <= 0 {
                return end_slow(x);
            }
            restore_priority(x);
        }
        2 => {
            x.move_xy();
            if ai_step(x) {
                x.free_eshl();
            }
        }
        _ => {}
    }
}

fn end_slow<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Base::SUB_STATE, 2);
    end_animation(x);
}
