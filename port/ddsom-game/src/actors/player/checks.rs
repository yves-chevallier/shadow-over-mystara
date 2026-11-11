//! Normal-game transitions according to inputs (turn, buttons) and periodic events.
//!
//! Each check is called by the common actions (`actions`) or by the characters' code
//! before their own processing, and returns true when it **changed the state** of the player (the
//! calling handler then stops). This boolean is the inverse of the Z flag that the original
//! leaves on return (`bne` after `bsr`: Z set = nothing done). Some paths of the original leave
//! an accidental Z (last byte written is zero): they are reproduced identically (`z_of`).
//!
//! Inputs (`KEYS_HELD`, `KEYS_NEW`): bits 0..3 directions, 4 button 1 (attack), 5 button 2 (jump),
//! 6 button 3 (throw), 7 button 4 (special).

use crate::world::objects::Obj;
use crate::world::machine::Machine;
use super::actions::to_stand;
use super::{animation, combos};
use super::states::{action, main, state, sub, State};
use super::{ram, Ctx, Player};
use crate::world::mem::{rd16, rd8, wr8, A5};
use crate::world::objects::pools;
use crate::actors::player::tables::{player_data, players, byte, long, Aligned, Read};

/// Buttons (bits of `KEYS_*`).
pub mod key {
    pub const DIRECTIONS: u8 = 0x0f;
    pub const HORIZONTAL: u8 = 0x03;
    pub const DOWN: u8 = 0x04;
    pub const UP: u8 = 0x08;
    pub const ATTACK: u8 = 0x10;
    pub const JUMP: u8 = 0x20;
    pub const THROW: u8 = 0x40;
    pub const SPECIAL: u8 = 0x80;
}


/// Orientation requested by the directions (bit 0: 0x20, otherwise 0).
fn facing_for(keys: u8) -> u8 {
    if keys & 1 != 0 {
        0x20
    } else {
        0
    }
}

/// 0x36f7e: the player asks for the other direction. In global slow walk they turn in place;
/// otherwise the turn is an action (`TURN`) with its own animation.
pub fn turn_check(g: &mut Ctx, p: Player) -> bool {
    cov!();
    let keys = p.u8(g, Player::KEYS_HELD) & key::HORIZONTAL;
    if keys == 0 || facing_for(keys) == p.u8(g, Player::FACING) {
        return false;
    }
    if ram::SLOW_MODE.get(g.b) != 0 {
        p.flip(g);
        return to_stand(g, p);
    }
    p.set_state(g, state::TURN_SLOW.0);
    let mut anim = 6;
    if p.u8(g, Player::SLOW) == 0 {
        p.set_state(g, state::TURN.0);
        anim = 0x30;
        if p.u8(g, Player::SLOW_8A) == 0 {
            anim = 5;
            let weapon_ok = p.u8(g, Player::WEAPON_B).wrapping_sub(0x28) <= 8
                && !matches!(p.u8(g, Player::WEAPON_A), 8 | 0x18);
            if weapon_ok && p.u8(g, Player::KEYS_HELD) & key::ATTACK != 0 {
                p.set_state(g, State::pack(main::PLAYING, sub::ACTING, action::TURN_ATTACK, 0).0);
                return true;
            }
        }
    }
    p.flip(g);
    p.set_u32(g, Player::RECOGNIZERS + 0x12, 0);
    p.set_u16(g, Player::RECOGNIZERS + 0x16, 0);
    animation::start(g, p, anim);
    true
}

/// 0x37018: button 1 (attack). Depending on the context: ground hit, hit in slow walk, or
/// release of a hold.
pub fn attack_button(g: &mut Ctx, p: Player) -> bool {
    cov!();
    if ram::SLOW_MODE.get(g.b) != 0 || p.u8(g, Player::POWER_10F) != 0 {
        return false;
    }
    if p.u8(g, Player::KEYS_NEW) & key::ATTACK == 0 {
        return false;
    }
    if p.u8(g, Player::SLOW_8A) != 0 {
        // lets go of what they hold: the linked object is ordered to leave
        p.set_state(g, State::pack(main::PLAYING, sub::ACTING, action::RELEASE, 0).0);
        p.set_u8(g, Player::CARRY, 2);
        p.set_u8(g, Player::SLOW_8A, 0);
        let held = p.u32(g, Player::LINK);
        wr8(g.b, held + 5, 6);
        animation::start(g, p, 0x31);
        return true;
    }
    if p.u8(g, Player::SLOW) != 0 {
        p.set_u8(g, Player::SUBACTION, 8);
        p.set_state(g, state::ATTACK.0);
        return item_curse(g, p);
    }
    if !target_in_range(g, p) {
        // nobody in range: the type of hit depends on the current action
        let current = p.u8(g, Player::ACTION);
        let kind = if current == action::RUN {
            0xe
        } else if current == action::TURN {
            6
        } else {
            0
        };
        p.set_u8(g, Player::SUBACTION, kind);
    }
    p.set_state(g, state::ATTACK.0);
    item_curse(g, p)
}

/// 0x370b2: button 2 (jump). The direction selects the animation and `+0x67` the jump type:
/// 3 vertical, 0 in place, 1 forward, 2 after a run.
pub fn jump_button(g: &mut Ctx, p: Player) -> bool {
    cov!();
    if ram::FROZEN.get(g.b) != 0 || p.u8(g, Player::SPECIAL_ACTIVE) != 0 || p.u8(g, Player::SLOW_8A) != 0 {
        return false;
    }
    let dir = p.u8(g, Player::KEYS_HELD) & key::DIRECTIONS;
    if p.u8(g, Player::KEYS_NEW) & key::JUMP == 0 {
        return false;
    }
    if p.u8(g, Player::SLOW) != 0 {
        p.set_state(g, State::pack(main::PLAYING, sub::ACTING, action::SLOW_SWITCH, 2).0);
        animation::start(g, p, 9);
        return true;
    }
    if dir == key::DOWN {
        p.set_state(g, State::pack(main::PLAYING, sub::ACTING, action::SLOW_SWITCH, 0).0);
        animation::start(g, p, 8);
        return true;
    }
    p.set_u8(g, Player::JUMP_KIND, 3);
    let mut anim = 0xa;
    if dir != key::UP {
        p.set_u8(g, Player::JUMP_KIND, 0);
        if dir & key::HORIZONTAL != 0 {
            p.set_u8(g, Player::JUMP_KIND, 1);
            anim = 0xf;
        }
        if p.u8(g, Player::ACTION) == action::RUN {
            combos::clear_recognizers(g, p);
            p.set_u8(g, Player::JUMP_KIND, 2);
            anim = 0x14;
        }
    }
    p.set_state(g, state::JUMP.0);
    animation::start(g, p, anim);
    true
}


/// 0x3714c: button 3 (throw). Creates a player effect linked to the player (`0x15a`). Returns true when
/// the original leaves Z at 0: allocation failure, or non-zero effect type. Nothing is done if the button is
/// not a new press or if the player is busy: Z set.
pub fn throw_button(g: &mut Ctx, p: Player) -> bool {
    cov!();
    if p.u8(g, Player::KEYS_NEW) & key::THROW == 0
        || p.u8(g, Player::SPECIAL_ACTIVE) != 0
        || p.u8(g, Player::SLOW_8A) != 0
        || p.u8(g, Player::POWER_10F) != 0
        || matches!(p.u8(g, Player::ACTION), 0xc | 0xe)
    {
        return false;
    }
    let Some(effect) = pools::PEFF.alloc(g.b) else {
        // the allocation leaves C set (`move #1,CCR`), hence Z at 0
        return true;
    };
    wr8(g.b, effect.0, 1);
    wr8(g.b, effect.0 + 3, 0xff);
    crate::world::mem::wr16(g.b, effect.0 + 0x50, p.0 as u16);
    p.set_u16(g, Player::SPECIAL_EFFECT, effect.0 as u16);
    p.set_u8(g, Player::SPECIAL_ACTIVE, 1);
    let character = p.u8(g, Player::CHARACTER) as u32;
    let kind = players("throw_kind").at8(g.b, character);
    wr8(g.b, effect.0 + 2, kind);
    kind != 0
}


/// 0x371be: button 4 (special attack), only in game phase 0xc.
pub fn special_attack(g: &mut Ctx, p: Player) {
    cov!();
    if ram::PHASE.get(g.b) != 0xc || ram::PAUSE.get(g.b) != 0 {
        return;
    }
    if p.u8(g, Player::KEYS_NEW) & key::SPECIAL == 0 || p.u8(g, Player::SPECIAL_ACTIVE) != 0 {
        return;
    }
    if p.u16(g, Player::MAIN) != 0x200 || p.u8(g, Player::POWER_10F) != 0 || p.u16(g, Obj::Z) != 0 {
        return;
    }
    if matches!(p.u8(g, Player::ACTION), 0xc | 0xe | 0x14 | 0x22) || p.u8(g, Player::SLOW_8A) != 0 {
        return;
    }
    // the chosen spell slot (`0x15d`) gives the spell identifier at the head of its 16-byte row
    let slot = p.at(Player::SLOT_TABLE) + ((p.u8(g, Player::SPECIAL_COUNT) as u32) << 4);
    let id = rd8(g.b, slot);
    p.set_u8(g, Player::SPECIAL_ID, id);
    if id == 0x2d {
        let weapon_ok = p.u8(g, Player::WEAPON_B).wrapping_sub(0x28) <= 8
            && !matches!(p.u8(g, Player::WEAPON_A), 8 | 0x18);
        if weapon_ok {
            p.set_state(g, State::pack(main::PLAYING, sub::ACTING, action::SPECIAL_ALT, 0).0);
        }
        return;
    }
    if id == 0x56 {
        if p.u8(g, Player::SUBACTION) != 0x1c {
            p.set_u8(g, Player::SLOW, 0);
            p.set_u8(g, Player::SUBACTION, 0x1c);
            p.set_state(g, state::ATTACK.0);
        }
        return;
    }
    let ready = p.u8(g, Player::SPECIAL_COUNT) == 2 || {
        let charge = rd8(g.b, slot + 8) as u32 + 0x150;
        p.u8(g, charge) != 0
    };
    if ready {
        p.set_state(g, state::SPECIAL.0);
        let anim = players("special_anim").at8(g.b, id as u32);
        p.set_u8(g, Player::SPECIAL_KIND, anim);
    } else if p.state(g) & !0xff != 0x0200_3200 {
        p.set_state(g, State::pack(main::PLAYING, sub::ACTING, action::SPELL_FIZZLE, 0).0);
    }
}

/// 0x3760e: the magic-user (character 8/9) with weapon 0x21 casts their ultimate spell when at least
/// three players are there, their scores exceed 2,000,000 and all are ready.
pub fn near_death(g: &mut Ctx, p: Player) {
    cov!();
    if ram::INPUT_MASK.get(g.b) & 0x80 == 0 || ram::PLAYERS_ALIVE.get(g.b) < 3 {
        return;
    }
    if !team_score_reached(g) || !combos::all_ready(g) {
        return;
    }
    if p.class(g) == 4 && p.u8(g, Player::WEAPON_B) == 0x21 {
        p.set_state(g, state::SPECIAL.0);
        p.set_u8(g, Player::SPECIAL_KIND, 0x24);
    }
}

/// Total of the scores (BCD) of the living players, kept in `A5-0x5fc6`.
pub const TEAM_SCORE: u32 = 0xff_203a;

/// 0x37658: BCD sum of the scores of the living players; true if it reaches 2,000,000.
pub fn team_score_reached(g: &mut Ctx) -> bool {
    cov!();
    crate::world::mem::wr32(g.b, TEAM_SCORE, 0);
    for q in Player::all() {
        if q.u8(g, Player::ACTIVE) == 0 || q.u16(g, Player::HP) == 0 {
            continue;
        }
        let mut carry = false;
        for i in (0..4).rev() {
            let sum = rd8(g.b, TEAM_SCORE + i);
            let add = q.u8(g, Player::SCORE_BCD + i);
            let (r, c) = bcd_add(sum, add, carry);
            wr8(g.b, TEAM_SCORE + i, r);
            carry = c;
        }
    }
    (crate::world::mem::rd32(g.b, TEAM_SCORE) as i32) >= 0x0200_0000
}

/// Packed decimal addition of one byte (ABCD) with incoming carry; returns the result and the carry.
fn bcd_add(a: u8, b: u8, carry: bool) -> (u8, bool) {
    let mut r = a as u16 + b as u16 + carry as u16;
    if (a & 0xf) + (b & 0xf) + carry as u8 > 9 {
        r += 6;
    }
    let out = r > 0x99;
    if out {
        r += 0x60;
    }
    (r as u8, out)
}


/// 0x376ee: cursed items (weapons 0xe and 0xf): on each hit, a draw decides an effect (the
/// player drops their weapon, or a monster appears). Returns the inverse of Z like the original.
pub fn item_curse(g: &mut Ctx, p: Player) -> bool {
    cov!();
    let weapon = p.u8(g, Player::WEAPON_A);
    if weapon != 0xe && weapon != 0xf {
        return true;
    }
    let pick = (g.random() & 0xf) as u32;
    let mask = player_data().curse_masks.word_at(g.b, 2 * pick);
    let bit = g.random() & 0xf;
    if mask >> bit & 1 != 0 {
        if weapon != 0xe {
            p.set_state(g, 0x0204_2600);
            super::spawn::cursed_weapon(g, p);
            // Z of the creator's last `move.w` (the link: a non-zero address word), or of the allocation that fails
            // (Z clear): always true
            return true;
        }
        let count = ram::CURSE_COUNT.get(g.b).wrapping_add(1);
        ram::CURSE_COUNT.set(g.b, count);
        if count == 0x50 {
            return curse_fall(g, p);
        }
        let facing = p.u8(g, Player::FACING);
        p.set_u8(g, Player::OVERLAY_ATTACK, facing);
        p.set_state(g, 0x0204_3600);
        super::spawn::cursed_weapon(g, p);
    }
    if p.u8(g, Player::WEAPON_A) != 0xe {
        return true;
    }
    let count = ram::CURSE_COUNT.get(g.b).wrapping_add(1);
    ram::CURSE_COUNT.set(g.b, count);
    if count <= 0x4f {
        // `bls`: the Z flag remains that of the comparison
        return count != 0x4f;
    }
    curse_fall(g, p)
}

/// 0x37768: the cursed player loses their weapon (becomes a weapon 0xd) if on the ground.
fn curse_fall(g: &mut Ctx, p: Player) -> bool {
    cov!();
    if p.u16(g, Obj::Z) == 0 {
        p.set_u8(g, Player::WEAPON_A, 0xd);
        p.set_state(g, 0x0200_2600);
        wr8(g.b, CURSE_DROPPED, 1);
    }
    true
}

/// Flag at `A5-0x5b82`: a cursed weapon was dropped (h).
const CURSE_DROPPED: u32 = A5 - 0x5b82;

/// Box tables in ROM (data): pointer per box set, then 8 bytes per box.
mod boxes {
    pub const BODY: u32 = 0x11bffa;
    /// Attack box sets (`sprites/tables.json`, box table `attack`).
    pub const ATTACK: u32 = 0x11c23a;
}

/// 0x371a: is there a boss or an enemy within range of the hit? Tests attack box 1 (sub-
/// action 2) then box 2 (sub-action 4) of the player's box set; if either hits, `SUBACTION`
/// receives the corresponding number.
pub fn target_in_range(g: &mut Ctx, p: Player) -> bool {
    cov!();
    let box_set = p.u8(g, Player::BOX_SET) as u32;
    let set = long(g.b, boxes::ATTACK + 4 * box_set);
    for (slot, sub) in [(1u32, 2u8), (2, 4)] {
        let attack_box = set + 8 * slot;
        if probe(g, p, attack_box, slot == 2) {
            p.set_u8(g, Player::SUBACTION, sub);
            return true;
        }
    }
    false
}

/// 0x3776: the bosses then the enemies.
fn probe(g: &mut Ctx, p: Player, attack_box: u32, second: bool) -> bool {
    cov!();
    pools::BOSS.iter().chain(pools::ENEMY.iter()).any(|o| keys(g, p, attack_box, o.0, second))
}

/// 0x3796: does the player's attack box overlap the body box of object `o`?
/// For the first box, the object must not be airborne; for the second, it must be
/// carried (`+0x86`) or be an enemy of type 0xa.
fn keys(g: &mut Ctx, p: Player, attack_box: u32, o: u32, second: bool) -> bool {
    cov!();
    let b = &mut *g.b;
    if rd8(b, o) & 1 == 0 || rd8(b, o + 0x38) == 0 {
        return false;
    }
    if !second {
        if (rd16(b, o + 0xc) as i16) > 0 {
            return false;
        }
    } else if rd8(b, o + 0x86) == 0 && (rd8(b, o + 0x28) != 2 || rd8(b, o + 2) != 0xa) {
        return false;
    }
    let body_index = rd8(b, o + 0x2e) as u32;
    let body_set = long(b, boxes::BODY + 4 * body_index);
    let body_box = body_set + 8 * rd8(b, o + 0x38) as u32;
    // depth (Y)
    let (ya, yb) = (sbyte(b, attack_box + 4), sbyte(b, body_box + 4));
    let (pya, pyb) = (rd16(b, p.0 + 0x10), rd16(b, o + 0x10));
    if !overlap(ya, yb, pya, pyb, byte(b, attack_box + 5), byte(b, body_box + 5)) {
        return false;
    }
    // abscissa (X), offsets inverted according to the orientation of each
    let (mut xa, mut xb) = (sbyte(b, attack_box), sbyte(b, body_box));
    if rd8(b, p.0 + 0x2c) != 0 {
        xa = xa.wrapping_neg();
    }
    if rd8(b, o + 0x2c) != 0 {
        xb = xb.wrapping_neg();
    }
    let (pxa, pxb) = (rd16(b, p.0 + 8), rd16(b, o + 8));
    if !overlap(xa, xb, pxa, pxb, byte(b, attack_box + 1), byte(b, body_box + 1)) {
        return false;
    }
    // altitude (Z)
    let (za, zb) = (sbyte(b, attack_box + 2), sbyte(b, body_box + 2));
    let (pza, pzb) = (rd16(b, p.0 + 0xc), rd16(b, o + 0xc));
    overlap(za, zb, pza, pzb, byte(b, attack_box + 3), byte(b, body_box + 3))
}

/// Signed byte extended to 16 bits.
fn sbyte(b: &mut crate::world::World, a: u32) -> i16 {
    byte(b, a) as i8 as i16
}

/// Overlap on one axis, in 16-bit arithmetic like the original: `h` is the sum (on 8 bits)
/// of the half-sizes, and `(offset + position)` of the two boxes must differ by at most `h`.
fn overlap(ofs_a: i16, ofs_b: i16, pos_a: u16, pos_b: u16, half_a: u8, half_b: u8) -> bool {
    let d = ofs_a.wrapping_add(pos_a as i16).wrapping_sub(ofs_b.wrapping_add(pos_b as i16));
    let h = half_a.wrapping_add(half_b) as u16;
    (d as u16).wrapping_add(h) <= h + h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bcd_addition() {
        assert_eq!(bcd_add(0x19, 0x01, false), (0x20, false));
        assert_eq!(bcd_add(0x99, 0x01, false), (0x00, true));
        assert_eq!(bcd_add(0x45, 0x55, true), (0x01, true));
    }

    #[test]
    fn box_overlap_on_an_axis() {
        // neighboring positions, half-sizes 2 + 2: overlap
        assert!(overlap(0, 0, 10, 12, 2, 2));
        // too far apart
        assert!(!overlap(0, 0, 10, 30, 2, 2));
        // the offsets count
        assert!(overlap(-8, 0, 20, 12, 2, 2));
    }
}
