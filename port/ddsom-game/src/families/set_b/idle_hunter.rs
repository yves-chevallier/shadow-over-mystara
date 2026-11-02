use super::*;

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
    match x.u8(0x4) {
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
    x.add8(0x4, 2);
    for i in 0..4 {
        let v = mark(x, i);
        x.set16(0x10 + 2 * i, v);
        x.set8(0x18 + i, 0);
    }
    let tier = (x.a5_u16(0x7f92) >> 2) << 4;
    let rng = x.random();
    let index = (rng & 0xf).wrapping_add(tier);
    let frames = x.rom8(IDLE_FRAMES + index as u32);
    x.set8(0x1c, frames);
    x.set16(0x1e, WARMUP);
}

fn step<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(0x5) == 0 {
        if x.a5_u8(0x2c) == PLAYING {
            let n = x.u16(0x1e).wrapping_sub(1);
            x.set16(0x1e, n);
            if n == 0 {
                x.add8(0x5, 2);
            }
        }
        return;
    }
    if x.a5_u8(0x50) != 0 {
        return x.set8(0x4, 4);
    }
    if x.a5_u8(0x2c) != PLAYING
        || x.a5_u8(0x35) == 0
        || x.a5_u8(0x3d) != 0
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
    if p.u8(x.b, pl::ACTIVE) & 1 == 0 || p.u16(x.b, pl::HEALTH) == 0 || x.u16(0x8 + 2 * i) != 0 {
        return;
    }
    let here = mark(x, i);
    if here == x.u16(0x10 + 2 * i) {
        let n = x.u8(0x18 + i).wrapping_add(1);
        x.set8(0x18 + i, n);
        if x.u8(0x1c) > n {
            return;
        }
        if let Some(first) = chase(x, p, 2 * i) {
            x.set16(0x8 + 2 * i, first);
        }
    }
    x.set16(0x10 + 2 * i, here);
    x.set8(0x18 + i, 0);
}

/// Creates pursuers for player `p` if the limit is not reached; returns the address of the last one created.
fn chase<R: Bench>(x: &mut Cx<'_, R>, p: Obj, slot: u32) -> Option<u16> {
    let active = [0x8, 0xa, 0xc, 0xe].into_iter().filter(|&o| x.u16(o) != 0).count() as i16;
    let players = x.a5_u8(0x35) as u32;
    let allowed = x.rom16(MAX_ACTIVE + 2 * players) as i16;
    if active >= allowed {
        return None;
    }
    eshl_creators::chasers(&mut Pure, x.b, x.o, slot as u8, p).map(Obj::word)
}
