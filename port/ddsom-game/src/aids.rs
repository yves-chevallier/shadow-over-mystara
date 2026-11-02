//! Small shared routines (V2): direction between two points (0x1386), grabbed object (0x2810),
//! game over when all players are down (0x28d8, last step of phase 12) and integer
//! division (0x17ea).

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::Cpu;

use crate::mem::{prog8, rd16, rd32, rd8, rts, wr16, wr32, wr8, A5};
use crate::{ops, regs, Native};

fn set_low_word(r: &mut u32, v: u16) {
    *r = *r & 0xffff_0000 | v as u32;
}

fn set_low_byte(r: &mut u32, v: u8) {
    *r = *r & 0xffff_ff00 | v as u32;
}

// ---------------------------------------------------------------------------------------------
// 0x1386: direction (0..31) from (D3, D4) to (D1, D2)
// ---------------------------------------------------------------------------------------------

/// Direction table, in program ROM: indexed by (octant << 3) | step.
const DIRECTIONS: u32 = 0x140c;

/// Computation of 0x1386 on the registers of the original (D1-D4), with the work registers it leaves.
pub(crate) struct Compute {
    pub direction: u8,
    pub d1: u32,
    pub d2: u32,
    pub d3: u32,
    /// D4.w: unit of the comparison.
    pub unit: u16,
    /// D6: counter of the estimation loop, if it was set.
    pub d6: Option<u32>,
}

/// Direction from point (D3, D4) to point (D1, D2). The differences are taken as absolute values (16 bits,
/// positions biased by 0x8000 to compare unsigned); the octant depends on the signs and the larger
/// difference, the step (0 to 3) on the ratio between the small and the large difference, estimated by subtractions.
pub(crate) fn compute(b: &mut Cps2Bus, d1: u32, d2: u32, d3: u32, d4: u32) -> Compute {
    let mut octant = 0u8;
    let mut dx = d1.wrapping_add(0x8000);
    let (diff, borrow) = (dx as u16).overflowing_sub(d3.wrapping_add(0x8000) as u16);
    let abs = if borrow { diff.wrapping_neg() } else { diff };
    set_low_word(&mut dx, abs);
    if borrow {
        octant = 4;
    }
    let mut dy = d2.wrapping_add(0x8000);
    let (diff, borrow) = (dy as u16).overflowing_sub(d4.wrapping_add(0x8000) as u16);
    let abs = if borrow { diff.wrapping_neg() } else { diff };
    set_low_word(&mut dy, abs);
    if borrow {
        octant += 2;
    }
    // the large difference goes into `dx`
    if (dy as u16) < dx as u16 {
        std::mem::swap(&mut dx, &mut dy);
        if octant == 0 || octant == 4 {
            octant += 1;
        }
    } else if octant == 2 || octant == 6 {
        octant += 1;
    }
    let mut step = 0u8;
    let high = (dy as u16) >> 3;
    set_low_word(&mut dy, high);
    let unit = (dy as u16).wrapping_mul(2);
    let mut d6 = None;
    if (dy as u16) < dx as u16 {
        let mut counter = 2u32;
        loop {
            step += 1;
            let sum = (dy as u8).wrapping_add(unit as u8);
            set_low_byte(&mut dy, sum);
            if dy as u16 >= dx as u16 {
                break;
            }
            // dbf D6
            let left = counter as u16;
            set_low_word(&mut counter, left.wrapping_sub(1));
            if left == 0 {
                step += 1;
                break;
            }
        }
        d6 = Some(counter);
    }
    let index = (octant << 3 | step) as u32;
    let direction = prog8(b, DIRECTIONS + index);
    Compute { direction, d1: dx, d2: dy, d3: step as u32, unit: unit, d6 }
}

/// 0x1386: direction (0 to 31) from point `de` to point `to` (x, z).
pub fn direction(b: &mut Cps2Bus, de: (u16, u16), to: (u16, u16)) -> u8 {
    compute(b, to.0 as u32, to.1 as u32, de.0 as u32, de.1 as u32).direction
}

/// 0x137e: direction from object `o` (x, z) to point `to`.
pub fn direction_from(b: &mut Cps2Bus, o: crate::objects::Obj, to: (u16, u16)) -> u8 {
    let de = (rd16(b, o.0 + 8), rd16(b, o.0 + 0x10));
    direction(b, de, to)
}

/// 0x1376: direction from object `o` to object `target`.
pub fn direction_to(b: &mut Cps2Bus, o: crate::objects::Obj, target: crate::objects::Obj) -> u8 {
    let to = (rd16(b, target.0 + 8), rd16(b, target.0 + 0x10));
    direction_from(b, o, to)
}

/// Native of 0x1386: work registers as the original leaves them (D1 to D4 and D6), D0 = the direction.
fn direction_native(c: &mut Cpu, b: &mut Cps2Bus) {
    let r = compute(b, c.d[1], c.d[2], c.d[3], c.d[4]);
    if let Some(d6) = r.d6 {
        c.d[6] = d6;
    }
    c.d[1] = r.d1;
    c.d[2] = r.d2;
    c.d[3] = r.d3;
    // `add.l #0x8000,D4` (carry in the high word), then the unit in D4.w
    c.d[4] = c.d[4].wrapping_add(0x8000);
    set_low_word(&mut c.d[4], r.unit);
    c.d[0] = r.direction as u32;
    ops::logic(c, r.direction as u32, Size::Byte);
    rts(c, b);
}

/// 0x137e: direction from object A0 (D3, D4 = its x and z) to (D1, D2).
fn direction_from_object(c: &mut Cpu, b: &mut Cps2Bus) {
    let (x, z) = (rd16(b, c.a[0] + 8), rd16(b, c.a[0] + 0x10));
    set_low_word(&mut c.d[3], x);
    set_low_word(&mut c.d[4], z);
    direction_native(c, b);
}

/// 0x1376: direction from object A0 to object A1.
fn direction_to_object(c: &mut Cpu, b: &mut Cps2Bus) {
    let (x, z) = (rd16(b, c.a[1] + 8), rd16(b, c.a[1] + 0x10));
    set_low_word(&mut c.d[1], x);
    set_low_word(&mut c.d[2], z);
    direction_from_object(c, b);
}

pub const DIRECTION_FROM_OBJECT: Native = Native {
    entry: 0x137e,
    name: "direction_object",
    run: crate::Body::WithCpu(direction_from_object),
    touches_sound: false,
    outputs: regs::d(0) | regs::CCR,
};

pub const DIRECTION_TO_OBJECT: Native = Native {
    entry: 0x1376,
    name: "direction_to_object",
    run: crate::Body::WithCpu(direction_to_object),
    touches_sound: false,
    outputs: regs::d(0) | regs::CCR,
};

pub const DIRECTION: Native = Native {
    entry: 0x1386,
    name: "direction",
    run: crate::Body::WithCpu(direction_native),
    touches_sound: false,
    outputs: regs::d(0) | regs::CCR,
};

// ---------------------------------------------------------------------------------------------
// 0x2810: is object A0 grabbed by a player?
// ---------------------------------------------------------------------------------------------

/// Result of 0x2810.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NameEntry {
    /// Game frozen (`A5+0x42` non-zero): nothing is read (Z = 0).
    Frozen,
    /// Nobody holds the object (word `+0x46` zero; Z = 1).
    Free,
    /// Player `player` holds it; `normal`: its byte `+0x2c` is zero (Z).
    By { player: crate::objects::Obj, normal: bool },
}

impl NameEntry {
    /// The Z flag that the original leaves (the callers' test).
    pub fn z(self) -> bool {
        match self {
            NameEntry::Frozen => false,
            NameEntry::Free => true,
            NameEntry::By { normal, .. } => normal,
        }
    }
}

/// 0x2810: is object `o` grabbed by a player (word `+0x46`)?
pub fn grabbed(b: &mut Cps2Bus, o: crate::objects::Obj) -> NameEntry {
    if rd8(b, A5 + 0x42) != 0 {
        return NameEntry::Frozen;
    }
    let holder = rd16(b, o.0 + 0x46);
    if holder == 0 {
        return NameEntry::Free;
    }
    let player = crate::objects::Obj::from_word(holder);
    NameEntry::By { player, normal: rd8(b, player.0 + 0x2c) == 0 }
}

/// Native: D0.w = the word `+0x46`, A1 = the player (word sign-extended), flags of the last test.
fn grabbed_native(c: &mut Cpu, b: &mut Cps2Bus) {
    if grabbed(b, crate::objects::Obj(c.a[0])) == NameEntry::Frozen {
        ops::logic(c, 1, Size::Byte);
    } else {
        let holder = rd16(b, c.a[0] + 0x46);
        set_low_word(&mut c.d[0], holder);
        ops::logic(c, holder as u32, Size::Word);
        if holder != 0 {
            c.a[1] = holder as i16 as i32 as u32;
            let state = rd8(b, c.a[1] + 0x2c);
            ops::logic(c, state as u32, Size::Byte);
        }
    }
    rts(c, b);
}

pub const GRABBED: Native = Native {
    entry: 0x2810,
    name: "object_seized",
    run: crate::Body::WithCpu(grabbed_native),
    touches_sound: false,
    outputs: regs::d(0) | regs::a(1) | regs::CCR,
};

// ---------------------------------------------------------------------------------------------
// 0x28d8: all players down
// ---------------------------------------------------------------------------------------------

/// A5 variables.
mod var {
    pub const STAGE: u32 = 0x34;
    pub const GAME_ON: u32 = 0x42;
    pub const LOCK: u32 = 0x3d;
    pub const STAGE_FLAG: u32 = 0x43;
    pub const PHASE: u32 = 0x2c;
    pub const HOLD: u32 = 0xffff_aa3a; // A5 - 0x55c6
}

mod player {
    pub const ACTIVE: u32 = 0x00;
    pub const FLAGS: u32 = 0x01;
    pub const STATE: u32 = 0x04;
    pub const ALTITUDE: u32 = 0x0c;
    pub const HP: u32 = 0x62;
    /// Bytes reset to zero when the player comes back (effects, grab, magic...).
    pub const CLEARED: [u32; 8] = [0x2a, 0x66, 0x15c, 0x79, 0x7a, 0x182, 0x183, 0x184];
    pub const RANK: u32 = 0x7d;
    pub const GRAB: u32 = 0x8a;
    pub const GRAB_OBJECT: u32 = 0x86;
}

/// Scenes without game over (shop, cutscene) and particular demo scenes.
const STAGE_NO_END: [u8; 2] = [6, 0xc];
const STAGE_SKIP_CHECK: u8 = 0x43;
const STAGE_SKIP_WAIT: u8 = 0x22;
/// State of a waiting player (`0x2000c00` without the low byte): the end is not triggered.
const STATE_WAITING: u32 = 0x0200_0c00;
/// Game over phase (`A5+0x2c` = 0x10, step 0...) and timer according to the scene.
const PHASE_END: u32 = 0x1000_0000;

/// 0x28d8, end of phase 12: when `A5+0x42` is non-zero (game in progress) and players are down
/// (zero HP), they are put back in the waiting state and the game over phase starts. Nothing happens if a player
/// is still in the air or in the waiting state. (The processor is only used for the players' context, for
/// `player::actions::to_stand`.)
pub fn game_over(_c: &mut Cpu, b: &mut Cps2Bus) {
    let stage = rd8(b, A5 + var::STAGE);
    if STAGE_NO_END.contains(&stage) || rd8(b, A5 + var::GAME_ON) == 0 {
        return;
    }
    wr8(b, A5 + var::STAGE_FLAG, 0x3f);
    if rd8(b, A5 + var::LOCK) != 0 || crate::ram::PLAYERS_ALIVE.get(b) == 0 {
        return;
    }
    let (mut rank, mut downed) = (0u8, 0u8);
    for p in crate::ram::PLAYERS.iter() {
        let p = p.0;
        if rd8(b, p + player::ACTIVE) != 0 && rd16(b, p + player::HP) != 0 {
            if stage != STAGE_SKIP_CHECK {
                if rd16(b, p + player::ALTITUDE) != 0 || rd32(b, p + player::STATE) & 0xffff_ff00 == STATE_WAITING {
                    return;
                }
            }
            downed = downed.wrapping_add(1);
            for off in player::CLEARED {
                wr8(b, p + off, 0);
            }
            let flags = rd8(b, p + player::FLAGS) & !2;
            wr8(b, p + player::FLAGS, flags);
            crate::palette::character_palette(b, crate::objects::Obj(p));
            wr8(b, p + player::ACTIVE, 1);
            if rd8(b, p + player::GRAB) != 0 {
                wr8(b, p + player::GRAB, 0);
                let object = rd32(b, p + player::GRAB_OBJECT);
                wr8(b, object + 5, 8);
                wr16(b, object + 6, 0);
            }
            wr8(b, p + player::RANK, rank);
            rank = rank.wrapping_add(1);
            if stage != STAGE_SKIP_WAIT && stage != STAGE_SKIP_CHECK {
                crate::player::actions::to_stand(&mut crate::player::Ctx::new(b), crate::player::Player(p));
            }
        }
    }
    if downed != 0 {
        wr32(b, A5 + var::PHASE, PHASE_END);
        crate::ram::SCROLL_Y_PLANE1.set(b, 0);
        wr16(b, A5.wrapping_add(var::HOLD), if stage == 1 { 0x60 } else { 0x120 });
    }
}

fn game_over_native(c: &mut Cpu, b: &mut Cps2Bus) {
    game_over(c, b);
    rts(c, b);
}

pub const GAME_OVER: Native = Native {
    entry: 0x28d8,
    name: "game_over_check",
    run: crate::Body::WithCpu(game_over_native),
    touches_sound: false,
    outputs: 0,
};

/// `0x17ea`: 32-bit unsigned division by shifts and subtractions (32 rounds). Returns the quotient and
/// the remainder; with a divisor above 2^31 the remainder overflows as in the original (zero divisor:
/// quotient 0xffffffff, remainder the dividend).
pub fn divide(mut n: u32, d: u32) -> (u32, u32) {
    let mut r = 0u32;
    for _ in 0..32 {
        let carry = n >> 31;
        n <<= 1;
        r = r.wrapping_shl(1) | carry;
        if r >= d {
            r = r.wrapping_sub(d);
            n += 1;
        }
    }
    (n, r)
}

/// D0 / D1: quotient in D0, remainder in D2; D3 = 0xffff (loop counter).
fn divide_native(c: &mut Cpu, b: &mut Cps2Bus) {
    let (q, r) = divide(c.d[0], c.d[1]);
    c.d[0] = q;
    c.d[2] = r;
    c.d[3] = 0xffff;
    rts(c, b);
}

pub const DIVIDE: Native = Native {
    entry: 0x17ea,
    name: "division",
    run: crate::Body::WithCpu(divide_native),
    touches_sound: false,
    outputs: regs::d(0) | regs::d(2) | regs::d(3),
};

pub const NATIVES: &[Native] = &[DIRECTION, DIRECTION_FROM_OBJECT, DIRECTION_TO_OBJECT, GRABBED, GAME_OVER, DIVIDE];

#[cfg(test)]
mod tests {
    // The functions work on the processor registers: validated by the frame-by-frame
    // comparison (`tools/validate_all.sh`) and the check mode.
}
