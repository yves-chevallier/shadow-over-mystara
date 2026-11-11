use super::*;
use crate::world::mem::{wr8, A5};
use crate::actors::families::set::tables as set_tables;
use crate::actors::families::set::vars;
use crate::world::objects::Obj;
use crate::actors::families::set::fields::SetObj;
use crate::actors::families::set::fields::WaveLeaderObj;

/// Delays between two waves (bytes, program space), indexed by a 4-bit draw.
const DELAYS: u32 = 0xbc63e;
const CAMERA_LOCK_PARAM: u32 = 0x3c;
/// Free slots of the ENEMY pool (A5+); three are needed per wave.
const ENEMY_FREE: u32 = 0x7e62;
const ENEMIES_PER_WAVE: i16 = 3;
const WAIT_START: u8 = 0xf0;
const WAIT_END: u8 = 0x3c;
const FINAL_WAIT: u16 = 0x12c;
const MAX_WAVES: u8 = 8;
const KIND_WING: u8 = 0x13;
const KIND_LEAD: u8 = 0x14;
const FINAL_VARIANT: u8 = 2;
/// Starting coordinates of the wave leader relative to the camera (`A5+0x506`).
const CAMERA_X: u32 = 0x506;
const SPAWN_OFFSET_X: u16 = 0x200;
const SPAWN_Y: u16 = 0x200;
/// Global variable (A5 - 0x5ac6): the wave's verdict for the rest of the game (h).
const WAVE_RESULT: u32 = A5 - 0x5ac6;
const WAVE_RESULT_FINAL: u8 = 0xff;
/// Leader's fields.
const DEFEATED_A: u32 = 0x20;
const DEFEATED_B: u32 = 0x21;
const DEFEATED_TARGET: u8 = 2;
const WAIT: u32 = 0x22;
const BUSY: u32 = 0x24;
const WAVE_COUNT: u32 = 0x25;
const SIDE: u32 = 0x26;
const LIMIT: u32 = 0x30;
const FLAG: u32 = 0x31;
const DIFF: u32 = 0x32;
/// Chief: address (`+0x46`, also copied into the created enemies), end-of-wave flag
/// (`+0x24`) and counter of living enemies (`+0x2a`).
/// Addresses (words) of the three enemies of the last wave.
const WINGS: u32 = 0x2a;
const BOSS: u32 = 0x46;
const BOSS_STATE: u32 = 0x4;
const BOSS_STATE_READY: u8 = 4;
const BOSS_DONE: u32 = 0x24;
const BOSS_COUNT: u32 = 0x2a;
/// Fields of the created enemies.
const E_LINK_A: u32 = 0xac;
const E_LINK_B: u32 = 0xae;
const E_COUNT: u32 = 0xb0;
const E_SIDE: u32 = 0xb1;
const E_WING_SIDE: u32 = 0xbc;
const E_PARENT: u32 = 0x42;
const E_ANCHOR: u32 = 0x50;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if x.a5_u8(util::IN_GAME) == 0 {
        return;
    }
    match x.u8(Obj::STATE) {
        0 => {
            x.add8(Obj::STATE, 2);
            x.set16(DEFEATED_A, 0);
            x.set16(BUSY, 0);
            x.set8(SetObj::SUBSTATE, 0);
            x.set16(WaveLeaderObj::WAVE_STATE, 0);
            x.set8(LIMIT, 0);
            x.set8(FLAG, 0);
            x.set8(DIFF, 0);
            x.set8(SIDE, 0xff);
            x.set8(WAIT, WAIT_START);
            substate(x);
        }
        2 => substate(x),
        4 => x.add8(Obj::STATE, 2),
        _ => x.free(&pools::SET),
    }
}

fn substate<R: Bench>(x: &mut Cx<'_, R>) {
    let boss = x.linked(BOSS);
    match x.u8(SetObj::SUBSTATE) {
        0 => {
            let w = x.u8(WAIT).wrapping_sub(1);
            x.set8(WAIT, w);
            if w > WAIT_END {
                return;
            }
            x.add8(SetObj::SUBSTATE, 2);
            crate::services::camera_modes::start_shake(x.b, crate::services::camera_modes::shake::B, CAMERA_LOCK_PARAM as u16);
        }
        2 => {
            let (a, b) = (x.u8(DEFEATED_A), x.u8(DEFEATED_B));
            let finished = |x: &mut Cx<'_, R>| {
                x.add8(SetObj::SUBSTATE, 2);
                boss.set_u16(x.b, BOSS_DONE, 1);
                x.set8(DIFF, a.wrapping_sub(b));
            };
            if b == DEFEATED_TARGET {
                x.add8(SetObj::SUBSTATE, 2);
                boss.set_u16(x.b, BOSS_DONE, 1);
            } else if a == DEFEATED_TARGET {
                if boss.u8(x.b, BOSS_STATE) == BOSS_STATE_READY {
                    finished(x);
                }
            } else if x.u8(BUSY) != 0 {
            } else if x.u8(WAVE_COUNT) == MAX_WAVES {
                finished(x);
            } else if x.u8(WAIT) != 0 {
                { let w = x.u8(WAIT); x.set8(WAIT, w - 1); }
            } else if spawn_wave(x).is_some() {
                let rng = x.random();
                let delay = set_tables::pc_byte(x.b, DELAYS + (rng as u32 & 0xf));
                x.set8(WAIT, delay);
            }
        }
        4 => {
            if boss.u16(x.b, BOSS_COUNT) == 1 {
                x.add8(SetObj::SUBSTATE, 2);
            }
        }
        6 => {
            if x.u8(LIMIT) != 0 {
                return;
            }
            let flag = x.u8(FLAG);
            wr8(x.b, WAVE_RESULT, flag);
            if flag != 0 {
                release_one(x, boss);
                x.set_a5_u8(vars::FROZEN, 1);
                x.add8(Obj::STATE, 2);
            } else {
                x.add8(SetObj::SUBSTATE, 2);
                final_wave(x);
            }
        }
        8 => final_wave(x),
        0xa => {
            if x.u8(BUSY) == 0 {
                x.set16(WAIT, FINAL_WAIT);
                x.add8(SetObj::SUBSTATE, 2);
            }
        }
        0xc => {
            if x.u8(FLAG) == 0 {
                let w = x.u16(WAIT).wrapping_sub(1);
                x.set16(WAIT, w);
                if w != 0 {
                    return;
                }
                wr8(x.b, WAVE_RESULT, 1);
            }
            x.set_a5_u8(vars::FROZEN, 1);
            x.add8(SetObj::SUBSTATE, 2);
        }
        0xe => {
            if x.u8(LIMIT) == 0 {
                release_one(x, boss);
                x.add8(Obj::STATE, 2);
            }
        }
        _ => {}
    }
}

/// The leader counts one enemy less.
fn release_one<R: Bench>(x: &mut Cx<'_, R>, boss: Obj) {
    let n = boss.u16(x.b, BOSS_COUNT).wrapping_sub(1);
    boss.set_u16(x.b, BOSS_COUNT, n);
}

/// Final wave: the leader is of variant 2 and the wave verdict goes to 0xff.
fn final_wave<R: Bench>(x: &mut Cx<'_, R>) {
    if let Some(lead) = spawn_wave(x) {
        lead.set_u8(x.b, Obj::VARIANT, FINAL_VARIANT);
        x.add8(SetObj::SUBSTATE, 2);
        wr8(x.b, WAVE_RESULT, WAVE_RESULT_FINAL);
    }
}

/// Creates three enemies at once: a leader (type 0x14, attached to the wave leader by
/// `+0x50`, placed to the right of the camera) and two wingmen (type 0x13) attached to it.
/// `None` when the ENEMY pool has fewer than three free slots. (The original then sorts the
/// three addresses with a loop that changes nothing.)
fn spawn_wave<R: Bench>(x: &mut Cx<'_, R>) -> Option<Obj> {
    if (x.a5_u16(ENEMY_FREE) as i16) < ENEMIES_PER_WAVE {
        return None;
    }
    let mut slots = [Obj(0); 3];
    for (i, slot) in slots.iter_mut().enumerate() {
        // Original glitch: the allocation failure is not tested (see `Pool::alloc_unchecked`).
        *slot = x.alloc_unchecked(&pools::ENEMY);
        slot.set_u8(x.b, Obj::ACTIVE, 1);
        x.set16(WINGS + 2 * i as u32, slot.0 as u16);
    }
    let low = |o: Obj| o.0 as u16;
    let [lead, wing_a, wing_b] = slots;
    let boss = x.u16(BOSS);
    lead.set_u16(x.b, BOSS, boss);
    for (wing, side, link) in [(wing_a, 0, E_LINK_A), (wing_b, 1, E_LINK_B)] {
        wing.set_u8(x.b, Obj::ACTIVE, 1);
        wing.set_u8(x.b, E_WING_SIDE, side);
        wing.set_u8(x.b, Obj::VARIANT, 0);
        wing.set_u8(x.b, SetObj::PARAM, 1);
        wing.set_u8(x.b, Obj::KIND, KIND_WING);
        wing.set_u16(x.b, E_PARENT, low(lead));
        wing.set_u16(x.b, BOSS, boss);
        lead.set_u16(x.b, link, low(wing));
    }
    lead.set_u8(x.b, E_COUNT, 3);
    let owner = x.linked(BOSS);
    let n = owner.u16(x.b, BOSS_COUNT).wrapping_add(3);
    owner.set_u16(x.b, BOSS_COUNT, n);
    lead.set_u8(x.b, Obj::ACTIVE, 1);
    lead.set_u8(x.b, Obj::KIND, KIND_LEAD);
    lead.set_u8(x.b, Obj::VARIANT, 0);
    lead.set_u16(x.b, E_ANCHOR, low(x.o));
    let camera = x.a5_u16(CAMERA_X);
    lead.set_u16(x.b, Obj::X, camera.wrapping_add(SPAWN_OFFSET_X));
    lead.set_u32(x.b, Obj::Z, 0);
    lead.set_u16(x.b, Obj::Y, SPAWN_Y);
    // arrival side: drawn at random the first time, then alternated
    let mut side = x.u8(SIDE);
    if (side as i8) < 0 {
        let rng = x.random();
        side = ((rng as u32 & 1) << 5) as u8;
        x.set8(SIDE, side ^ 0x20);
    }
    lead.set_u8(x.b, E_SIDE, side);
    let y = lead.u8(x.b, WaveLeaderObj::DEPTH_LOW).wrapping_add(side.wrapping_add(0x10));
    lead.set_u8(x.b, WaveLeaderObj::DEPTH_LOW, y);
    x.add8(DEFEATED_A, 1);
    x.set8(BUSY, 1);
    x.add8(WAVE_COUNT, 1);
    Some(lead)
}
