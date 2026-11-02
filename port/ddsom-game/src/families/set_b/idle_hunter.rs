use super::*;
use crate::families::set::tables as set_tables;
use crate::families::set::vars;
use crate::objects::Obj;
use crate::families::set::fields::SetObj;
use crate::families::set_b::fields::IdleHunterObj;

/// Motionless delays: 16 bytes per slice of 4 ranks (program space).
const IDLE_FRAMES: u32 = 0xd8186;
/// Maximum number of pursuers according to the number of players (words).
const MAX_ACTIVE: u32 = 0xd8448;
const CUTSCENE: u32 = 0x53a;
const PLAYING: u8 = 0xc;
const WARMUP: u16 = 0x168;
/// The four examination orders.
const ORDERS: [[u32; 4]; 4] = [[0, 1, 2, 3], [2, 3, 1, 0], [1, 0, 3, 2], [3, 2, 0, 1]];

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => init(x),
        2 => step(x),
        _ => x.free(&pools::SET),
    }
}

/// Reference position of a player: X + following bytes (`+8` and `+0x10` of the player).
fn mark<R: Bench>(x: &mut Cx<'_, R>, i: u32) -> u16 {
    let base = 0x5de + PLAYER_SIZE * i;
    x.a5_u16(base + 8).wrapping_add(x.a5_u16(base + 0x10))
}

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Obj::STATE, 2);
    for i in 0..4 {
        let v = mark(x, i);
        x.set16(IdleHunterObj::MARKS + 2 * i, v);
        x.set8(IdleHunterObj::IDLE + i, 0);
    }
    let tier = (x.a5_u16(vars::RANK) >> 2) << 4;
    let rng = x.random();
    let index = (rng & 0xf).wrapping_add(tier);
    let frames = set_tables::pc_byte(x.b, IDLE_FRAMES + index as u32);
    x.set8(IdleHunterObj::IDLE_LIMIT, frames);
    x.set16(IdleHunterObj::WARMUP, WARMUP);
}

fn step<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(SetObj::SUBSTATE) == 0 {
        if x.a5_u8(vars::PHASE) == PLAYING {
            let n = x.u16(IdleHunterObj::WARMUP).wrapping_sub(1);
            x.set16(IdleHunterObj::WARMUP, n);
            if n == 0 {
                x.add8(SetObj::SUBSTATE, 2);
            }
        }
        return;
    }
    if x.a5_u8(vars::EXIT_LOCK) != 0 {
        return x.set8(Obj::STATE, 4);
    }
    if x.a5_u8(vars::PHASE) != PLAYING
        || x.a5_u8(vars::PLAYERS_ALIVE) == 0
        || x.a5_u8(vars::PAUSED) != 0
        || x.a5_u8(CUTSCENE) != 0
        || (x.a5_u16(ram::CAMERA_X) as i16) > 0x180
    {
        return;
    }
    let rng = x.random();
    for i in ORDERS[(rng as u32 & 3) as usize] {
        check(x, i);
    }
}

/// Examination of a player: motionless long enough, it attracts pursuers.
fn check<R: Bench>(x: &mut Cx<'_, R>, i: u32) {
    let p = Obj(PLAYERS + PLAYER_SIZE * i);
    if p.u8(x.b, pl::ACTIVE) & 1 == 0 || p.u16(x.b, pl::HEALTH) == 0 || x.u16(IdleHunterObj::HUNTERS + 2 * i) != 0 {
        return;
    }
    let here = mark(x, i);
    if here == x.u16(IdleHunterObj::MARKS + 2 * i) {
        let n = x.u8(IdleHunterObj::IDLE + i).wrapping_add(1);
        x.set8(IdleHunterObj::IDLE + i, n);
        if x.u8(IdleHunterObj::IDLE_LIMIT) > n {
            return;
        }
        if let Some(first) = chase(x, p, 2 * i) {
            x.set16(IdleHunterObj::HUNTERS + 2 * i, first);
        }
    }
    x.set16(IdleHunterObj::MARKS + 2 * i, here);
    x.set8(IdleHunterObj::IDLE + i, 0);
}

/// Creates pursuers for player `p` if the limit is not reached; returns the address of the last one created.
fn chase<R: Bench>(x: &mut Cx<'_, R>, p: Obj, slot: u32) -> Option<u16> {
    let active = (0..4).filter(|&i| x.u16(IdleHunterObj::HUNTERS + 2 * i) != 0).count() as i16;
    let players = x.a5_u8(vars::PLAYERS_ALIVE) as u32;
    let allowed = set_tables::pc_word(x.b, MAX_ACTIVE + 2 * players) as i16;
    if active >= allowed {
        return None;
    }
    eshl_creators::chasers(&mut Pure, x.b, x.o, slot as u8, p).map(Obj::word)
}
