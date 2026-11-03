//! Table-driven draws and parameters: stance rows, counterattack, dodge, parry, countdown (0x788c-0x10112, 0xff54,
//! 0xfffc). A table of addresses is indexed by a byte of the object; the chosen row is read at random (bytes) or by
//! the rank (32-bit masks, a randomly drawn bit).

use super::*;

/// 0xff54: countdown `+0x98`; at its end, and if the bit drawn in the mask of the players alive and the rank
/// (`ai/countdown_masks`, words) is set, state 0x020e0000; otherwise `o` stays (or becomes again) active in state
/// 0x04040200.
pub fn countdown_then_state(b: &mut Cps2Bus, o: Obj) {
    let e = Enemy(o);
    let left = e.ai_behaviour(b).wrapping_sub(1);
    e.set_ai_behaviour(b, left);
    let again = |b: &mut Cps2Bus| {
        e.set_active(b, 1);
        e.set_state_long(b, 0x0404_0200);
    };
    if left as i8 > 0 {
        return again(b);
    }
    let row = ((crate::ram::PLAYERS_ALIVE.get(b) as u16) << 4).wrapping_add((RANK.get(b) >> 2) << 1);
    let mask = table("ai/countdown_masks").at16(b, offset(row)) as u32;
    let roll = rng::random(b) as u32 & 15;
    if mask >> roll & 1 == 0 {
        return again(b);
    }
    e.set_state_long(b, 0x020e_0000);
}

/// 0xfffc: behaviour parameter `+0x98` of `o`, according to its type (`ai/behaviour_by_kind`).
pub fn behaviour_by_kind(b: &mut Cps2Bus, o: Obj) {
    let kind = o.kind(b) as u32;
    let v = table("ai/behaviour_by_kind").at8(b, kind);
    Enemy(o).set_ai_behaviour(b, v);
}

/// 32-bit mask of row `number` of the table of addresses `rows`, chosen by the rank; one of its bits is drawn:
/// true if it is set.
fn rank_mask_roll(b: &mut Cps2Bus, rows: &str, number: u8) -> bool {
    let row = table(rows).nth32(b, number as u32);
    let rank = RANK.get(b);
    let mask = engine().long(b, row.wrapping_add(offset(rank << 2)));
    let roll = rng::random(b) as u32 & 31;
    mask >> roll & 1 != 0
}

/// Byte drawn at random (32 choices) in row `number` of the table of addresses `rows`.
fn random_byte(b: &mut Cps2Bus, rows: &str, number: u8) -> u8 {
    let row = table(rows).nth32(b, number as u32);
    let roll = rng::random(b) as u32 & 31;
    engine().byte(b, row + roll)
}

/// 0x788c: `+0x6b` drawn in row `+0x6a`.
pub fn draw_6b(b: &mut Cps2Bus, o: Obj) {
    let e = Enemy(o);
    let number = e.ai_wander_row(b);
    let v = random_byte(b, "ai/draws_6b", number);
    e.set_ai_wander(b, v);
}

/// 0x79f6: inside the play area, `+0x6e` is drawn in row `+0x6d`; otherwise it is 1.
pub fn draw_6e(b: &mut Cps2Bus, o: Obj) {
    let e = Enemy(o);
    let v = if crate::objects::cull::in_arena(b, o, 0) {
        let number = e.ai_wait_row(b);
        random_byte(b, "ai/draws_6e", number)
    } else {
        1
    };
    e.set_ai_wait(b, v);
}

/// 0x79ce: if the bit drawn in the mask of row `+0x6c` is set, `draw_6e` and true; otherwise false.
pub fn maybe_draw_6e(b: &mut Cps2Bus, o: Obj) -> bool {
    let number = Enemy(o).ai_wait_masks(b);
    if !rank_mask_roll(b, "ai/masks_6c", number) {
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
/// be on the ground and face the aggressor; a bit is then drawn in the mask of its parry row (`ai/parries`).
pub fn parries(b: &mut Cps2Bus, attacker: Obj, victim: Obj, table: Parry) -> bool {
    let v = Enemy(victim);
    let facing = attacker.flip(b);
    if v.vulnerable(b) == 0 || victim.u16(b, Obj::Z) != 0 || facing == v.flip(b) {
        return false;
    }
    let number = match table {
        Parry::A => v.ai_68(b),
        Parry::B => v.ai_69(b),
    };
    rank_mask_roll(b, "ai/parries", number)
}

/// 0xf118: outside the play area, the word `+0x84` = 1; otherwise `+0x84` = 0 and `+0x85` is drawn in row `+0x8e`
/// (`ai/draws_85`), block chosen by the rank (32 bytes per pair of ranks).
pub fn draw_84(b: &mut Cps2Bus, o: Obj) {
    let e = Enemy(o);
    if !crate::objects::cull::in_arena(b, o, 0) {
        // one word: `+0x84` cleared and `+0x85` set to 1
        o.set_u16(b, Enemy::AI_OFFSCREEN, 1);
        return;
    }
    let number = e.ai_row_8e(b) as u32;
    let block = (RANK.get(b) >> 1) << 5;
    let row = table("ai/draws_85").nth32(b, number).wrapping_add(offset(block));
    let roll = rng::random(b) as u32 & 31;
    let v = engine().byte(b, row + roll);
    e.set_ai_draw_85(b, v);
    e.set_ai_offscreen(b, 0);
}

/// 0x10112: parry level `+0x68` = 8 without a scene level (`ram::SCENE_LEVEL`), otherwise read according to it (capped at 4)
/// and the number of players alive (`ai/parry_levels`).
pub fn set_68(b: &mut Cps2Bus, o: Obj) {
    let level = crate::ram::SCENE_LEVEL.get(b);
    let v = if level == 0 {
        8
    } else {
        let players = crate::ram::PLAYERS_ALIVE.get(b).wrapping_mul(4);
        let row = ((level.wrapping_sub(1) as i8).min(3) as u8).wrapping_add(players);
        table("ai/parry_levels").at8(b, row as u32)
    };
    Enemy(o).set_ai_68(b, v);
}
