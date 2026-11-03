//! Table-driven draws and parameters: stance rows, counterattack, dodge, parry, countdown (0x788c-0x10112, 0xff54, 0xfffc).

use super::*;

/// Masks (words, by players alive and rank) of 0xff54.
pub(super) const COUNTDOWN_MASKS: u32 = 0xff9a;

/// 0xff54: countdown `+0x98`; at its end, and if the bit drawn in the mask of the players alive and the rank
/// is set, state 0x020e0000; otherwise `o` stays (or becomes again) active in state 0x04040200.
pub fn countdown_then_state(b: &mut Cps2Bus, o: Obj) {
    let left = o.u8(b, g::BEHAVIOUR).wrapping_sub(1);
    o.set_u8(b, g::BEHAVIOUR, left);
    let again = |b: &mut Cps2Bus| {
        o.set_u8(b, Obj::ACTIVE, 1);
        o.set_u32(b, Obj::STATE, 0x0404_0200);
    };
    if left as i8 > 0 {
        return again(b);
    }
    let row = ((crate::ram::PLAYERS_ALIVE.get(b) as u16) << 4).wrapping_add((rd16(b, TICK) >> 2) << 1);
    let mask = crate::mem::prog16(b, COUNTDOWN_MASKS.wrapping_add(offset(row))) as u32;
    let roll = rng::random(b) as u32 & 15;
    if mask >> roll & 1 == 0 {
        return again(b);
    }
    o.set_u32(b, Obj::STATE, 0x020e_0000);
}

/// Behaviour parameter by type of 0xfffc.
pub(super) const BEHAVIOUR_BY_KIND: u32 = 0x1000a;

/// 0xfffc: behaviour parameter `+0x98` of `o`, according to its type.
pub fn behaviour_by_kind(b: &mut Cps2Bus, o: Obj) {
    let kind = o.u8(b, Obj::KIND) as u32;
    let v = crate::mem::prog8(b, BEHAVIOUR_BY_KIND + kind);
    o.set_u8(b, g::BEHAVIOUR, v);
}

/// 32-bit mask of row `number` of the table `pointers`, chosen by the rank; one of its bits is drawn:
/// true if it is set.
pub(super) fn rank_mask_roll(b: &mut Cps2Bus, pointers: u32, number: u8) -> bool {
    let row = rd32(b, pointers + 4 * number as u32);
    let tick = rd16(b, TICK);
    let mask = rd32(b, row.wrapping_add(offset(tick << 2)));
    let roll = rng::random(b) as u32 & 31;
    mask >> roll & 1 != 0
}

/// Byte drawn at random (32 choices) in row `number` of the table `pointers`.
pub(super) fn random_byte(b: &mut Cps2Bus, pointers: u32, number: u8) -> u8 {
    let row = rd32(b, pointers + 4 * number as u32);
    let roll = rng::random(b) as u32 & 31;
    rd8(b, row + roll)
}

/// 0x788c: `+0x6b` drawn in row `+0x6a`.
pub fn draw_6b(b: &mut Cps2Bus, o: Obj) {
    let number = o.u8(b, g::ROW_6A);
    let v = random_byte(b, 0x78ae, number);
    o.set_u8(b, g::DRAW_6B, v);
}

/// 0x79f6: inside the play area, `+0x6e` is drawn in row `+0x6d`; otherwise it is 1.
pub fn draw_6e(b: &mut Cps2Bus, o: Obj) {
    let v = if crate::objects::cull::in_arena(b, o, 0) {
        let number = o.u8(b, g::ROW_6D);
        random_byte(b, 0x7a50, number)
    } else {
        1
    };
    o.set_u8(b, g::DRAW_6E, v);
}

/// 0x79ce: if the bit drawn in the mask of row `+0x6c` is set, `draw_6e` and true; otherwise false.
pub fn maybe_draw_6e(b: &mut Cps2Bus, o: Obj) -> bool {
    let number = o.u8(b, g::ROW_6C);
    if !rank_mask_roll(b, 0x7a34, number) {
        return false;
    }
    draw_6e(b, o);
    true
}

/// The two parry tables: row `+0x68` (0xb048) or `+0x69` (0xb068) of the victim.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Parry {
    A,
    B,
}

/// 0xb048 / 0xb068: does the victim `victim` parry the hit of `attacker`? It must know how to parry (`+0x67`),
/// be on the ground and face the aggressor; a bit is then drawn in the mask of its parry row.
pub fn parries(b: &mut Cps2Bus, attacker: Obj, victim: Obj, table: Parry) -> bool {
    let facing = attacker.u8(b, f::FACING);
    if victim.u8(b, 0x67) == 0 || victim.u16(b, Obj::Z) != 0 || facing == victim.u8(b, f::FACING) {
        return false;
    }
    let field = match table {
        Parry::A => 0x68,
        Parry::B => 0x69,
    };
    let number = victim.u8(b, field);
    rank_mask_roll(b, 0xb0b0, number)
}

/// 0xf118: outside the play area, `+0x84` = 1; otherwise `+0x84` = 0 and `+0x85` is drawn in row `+0x8e`, block
/// chosen by the rank (32 bytes per pair of ranks).
pub fn draw_84(b: &mut Cps2Bus, o: Obj) {
    if !crate::objects::cull::in_arena(b, o, 0) {
        o.set_u16(b, g::OFFSCREEN_84, 1);
        return;
    }
    let number = o.u8(b, g::ROW_8E) as u32;
    let block = (rd16(b, TICK) >> 1) << 5;
    let row = rd32(b, 0xf160 + 4 * number).wrapping_add(offset(block));
    let roll = rng::random(b) as u32 & 31;
    let v = rd8(b, row + roll);
    o.set_u8(b, g::DRAW_85, v);
    o.set_u8(b, g::OFFSCREEN_84, 0);
}

/// Parry levels by scene level and players alive of 0x10112.
pub(super) const PARRY_LEVELS: u32 = 0x10144;

/// 0x10112: parry level `+0x68` = 8 without a scene level (`A5+0x5a`), otherwise read according to it (capped at 4)
/// and the number of players alive.
pub fn set_68(b: &mut Cps2Bus, o: Obj) {
    let level = rd8(b, A5 + 0x5a);
    let v = if level == 0 {
        8
    } else {
        let players = crate::ram::PLAYERS_ALIVE.get(b).wrapping_mul(4);
        let row = ((level.wrapping_sub(1) as i8).min(3) as u8).wrapping_add(players);
        crate::mem::prog8(b, PARRY_LEVELS + row as u32)
    };
    o.set_u8(b, g::PARRY_LEVEL, v);
}
