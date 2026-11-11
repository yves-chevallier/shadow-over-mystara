//! Small shared routines (V2): direction between two points (0x1386), grabbed object (0x2810),
//! game over when all players are down (0x28d8, last step of phase 12) and integer
//! division (0x17ea).

use crate::world::World;

use crate::world::objects::{Enemy, Obj};
use crate::actors::player::Player;
use crate::world::ram::{self, PLAYERS_FROZEN};
use crate::Native;

pub(crate) fn set_low_word(r: &mut u32, v: u16) {
    *r = *r & 0xffff_0000 | v as u32;
}

fn set_low_byte(r: &mut u32, v: u8) {
    *r = *r & 0xffff_ff00 | v as u32;
}

// ---------------------------------------------------------------------------------------------
// 0x1386: direction (0..31) from (D3, D4) to (D1, D2)
// ---------------------------------------------------------------------------------------------

/// Computation of 0x1386 on the registers of the original (D1-D4), with the work registers D2-D4 it leaves (read
/// again by the boss 15 and the handlers that follow it: `dispatch::Leftovers`).
pub(crate) struct Compute {
    pub direction: u8,
    pub d2: u32,
    /// D3: the step (0 to 4) of the estimation.
    pub d3: u32,
    /// D4.w: unit of the comparison.
    pub unit: u16,
}

/// Direction from point (D3, D4) to point (D1, D2). The differences are taken as absolute values (16 bits,
/// positions biased by 0x8000 to compare unsigned); the octant depends on the signs and the larger
/// difference, the step (0 to 3) on the ratio between the small and the large difference, estimated by subtractions.
pub(crate) fn compute(b: &mut World, d1: u32, d2: u32, d3: u32, d4: u32) -> Compute {
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
    }
    // direction table (`aids/directions`, program space): indexed by (octant << 3) | step
    let index = (octant << 3 | step) as u32;
    let direction = crate::world::tables::engine_table!("aids/directions").at8(b, index);
    Compute { direction, d2: dy, d3: step as u32, unit }
}

/// 0x1386: direction (0 to 31) from point `de` to point `to` (x, z).
pub fn direction(b: &mut World, de: (u16, u16), to: (u16, u16)) -> u8 {
    compute(b, to.0 as u32, to.1 as u32, de.0 as u32, de.1 as u32).direction
}

/// 0x137e: direction from object `o` (x, z) to point `to`.
pub fn direction_from(b: &mut World, o: Obj, to: (u16, u16)) -> u8 {
    let de = (o.u16(b, Obj::X), o.u16(b, Obj::Y));
    direction(b, de, to)
}

/// 0x1376: direction from object `o` to object `target`.
pub fn direction_to(b: &mut World, o: Obj, target: Obj) -> u8 {
    let to = (target.u16(b, Obj::X), target.u16(b, Obj::Y));
    direction_from(b, o, to)
}

// ---------------------------------------------------------------------------------------------
// 0x2810: is object A0 grabbed by a player?
// ---------------------------------------------------------------------------------------------

/// Result of 0x2810.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NameEntry {
    /// Game frozen (`ram::PLAYERS_FROZEN` non-zero): nothing is read (Z = 0).
    Frozen,
    /// Nobody holds the object (word `+0x46` zero; Z = 1).
    Free,
    /// Player `player` holds it; `normal`: its byte `+0x2c` is zero (Z).
    By { player: Obj, normal: bool },
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
pub fn grabbed(b: &mut World, o: Obj) -> NameEntry {
    if PLAYERS_FROZEN.is_set(b) {
        return NameEntry::Frozen;
    }
    let holder = Enemy(o).host(b);
    if holder == 0 {
        return NameEntry::Free;
    }
    let player = Obj::from_word(holder);
    NameEntry::By { player, normal: player.flip(b) == 0 }
}

// ---------------------------------------------------------------------------------------------
// 0x28d8: all players down
// ---------------------------------------------------------------------------------------------

/// Scenes without game over (shop, cutscene) and particular demo scenes.
const STAGE_NO_END: [u8; 2] = [6, 0xc];
const STAGE_SKIP_CHECK: u8 = 0x43;
const STAGE_SKIP_WAIT: u8 = 0x22;
/// State of a waiting player (`0x2000c00` without the low byte): the end is not triggered.
const STATE_WAITING: u32 = 0x0200_0c00;
/// Game over phase (`ram::PHASE` = 0x10, step 0...) and timer according to the scene.
const PHASE_END: u32 = 0x1000_0000;

/// 0x28d8, end of phase 12: when `ram::PLAYERS_FROZEN` is non-zero (game in progress) and players are down
/// (zero HP), they are put back in the waiting state and the game over phase starts. Nothing happens if a player
/// is still in the air or in the waiting state.
pub fn game_over(b: &mut World) {
    let stage = ram::SCENE.get(b);
    if STAGE_NO_END.contains(&stage) || !PLAYERS_FROZEN.is_set(b) {
        return;
    }
    ram::INPUT_MASK.set(b, 0x3f);
    if ram::CUTSCENE.is_set(b) || ram::PLAYERS_ALIVE.get(b) == 0 {
        return;
    }
    let (mut rank, mut downed) = (0u8, 0u8);
    for p in ram::PLAYERS.iter().map(Player::of) {
        if p.active(b) != 0 && p.hp(b) != 0 {
            if stage != STAGE_SKIP_CHECK
                && (p.obj().u16(b, Obj::Z) != 0 || p.obj().u32(b, Obj::STATE) & 0xffff_ff00 == STATE_WAITING)
            {
                return;
            }
            downed = downed.wrapping_add(1);
            // the bytes reset when the player comes back: palette, effects, grab, magic...
            p.set_palette_group(b, 0);
            p.set_slow(b, 0);
            p.set_special_active(b, 0);
            p.set_powerup(b, 0);
            p.set_powerup_timer(b, 0);
            p.set_aura(b, 0);
            p.set_aura_b(b, 0);
            p.set_aura_c(b, 0);
            let flags = p.flags(b) & !2;
            p.set_flags(b, flags);
            crate::services::palette::character_palette(b, p.obj());
            p.set_active(b, 1);
            if p.slow_8a(b) != 0 {
                p.set_slow_8a(b, 0);
                let object = Enemy(Obj(p.link(b)));
                object.set_substate(b, 8);
                object.set_step_word(b, 0);
            }
            p.set_rank(b, rank);
            rank = rank.wrapping_add(1);
            if stage != STAGE_SKIP_WAIT && stage != STAGE_SKIP_CHECK {
                crate::actors::player::actions::to_stand(&mut crate::actors::player::Ctx::new(b), p);
            }
        }
    }
    if downed != 0 {
        ram::PHASE_LONG.set(b, PHASE_END);
        ram::SCROLL_Y_PLANE1.set(b, 0);
        ram::GAME_OVER_DELAY.set(b, if stage == 1 { 0x60 } else { 0x120 });
    }
}

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

pub const NATIVES: &[Native] = &[];

#[cfg(test)]
mod tests {
    // The functions work on the processor registers: validated by the frame-by-frame
    // comparison (`tools/validate_all.sh`) and the check mode.
}
