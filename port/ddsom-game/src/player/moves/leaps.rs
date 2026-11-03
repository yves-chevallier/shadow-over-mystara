//! Leaps of the characters (action 0x12), parry and the special-move actions (combos, backstab).

use crate::player::Player;
use super::*;
use crate::player::tables::{player_data, players, pc8, long, Read};

// ---- action 0x12 of the characters: leaps ---------------------------------------------------------

/// 0x35a78 (fighter, cleric, elf): forward leap, with a possible aerial attack.
pub fn leap_attack(g: &mut Ctx, p: Player) {
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, field::AIR_ATTACK, 0);
            p.set_u8(g, field::AIRBORNE, 0);
            p.set_u16(g, Obj::VX, 0x200);
            p.set_u16(g, Obj::AX, 0);
            p.set_u16(g, Obj::VZ, 0x380);
            p.set_u16(g, Obj::AZ, (-56i16) as u16);
            if p.faces_left(g) {
                p.set_u16(g, Obj::VX, (-0x200i16) as u16);
            }
            animation::start(g, p, 31);
            g.sound(crate::sound::JUMP_VOICE, p);
        }
        2 => {
            animation::step(g, p);
            move_xz(g, p);
            air_attack(g, p);
            if (p.u16(g, Obj::Z) as i16) < 0 {
                next_step(g, p);
                p.set_u32(g, Obj::Z, 0);
                animation::start(g, p, 14);
                g.sound(ext::sound::LANDING, p);
            }
        }
        4 => {
            if animation::step(g, p) {
                p.set_u8(g, Player::SLOW, 0);
                to_stand(g, p);
            }
        }
        v => unreachable!("0x35a78 : value {v:#x} outside the original jump table"),
    }
    let event = p.u8(g, field::ANIM_EVENT);
    if event != 0 {
        p.set_u8(g, field::ANIM_EVENT, 0);
        super::super::spawn::weapon_trail(g, p, event);
    }
}

/// 0x3769c: during the leap, button 1 gives an aerial attack (animation per character, table
/// 0x376d6, or 0x376e2 with down held).
pub(super) fn air_attack(g: &mut Ctx, p: Player) {
    if p.u8(g, field::AIR_ATTACK) != 0 || p.u8(g, Player::KEYS_NEW) & 0x10 == 0 {
        return;
    }
    let character = p.u8(g, Player::CHARACTER) as u32;
    let table = if p.u8(g, Player::KEYS_HELD) & 4 != 0 { 0x376e2 } else { 0x376d6 };
    let anim = pc8(g.b, table + character);
    animation::start_plain(g, p, anim as u16);
    item_curse(g, p);
}

/// 0x35b96 (magic-user): small backward jump (the recoil stops in front of a terrain obstacle).
pub fn hop_back(g: &mut Ctx, p: Player) {
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            animation::start(g, p, 31);
        }
        2 => {
            if !animation::step(g, p) {
                return;
            }
            next_step(g, p);
            p.set_u16(g, Obj::VX, 0x300);
            p.set_u16(g, Obj::AX, (-32i16) as u16);
            let mut shift: u16 = 64;
            if p.faces_left(g) {
                shift = shift.wrapping_neg();
                let pair = p.u32(g, Obj::VX).wrapping_neg();
                p.set_u32(g, Obj::VX, pair);
            }
            let (x, y) = (p.u16(g, Obj::X).wrapping_add(shift), p.u16(g, Obj::Y));
            if crate::video::terrain(g.b, x, y).number & 63 == 0 {
                p.set_u16(g, Obj::X, x);
            }
            super::super::walk::clamp_to_screen(g.b, p);
            animation::start(g, p, 32);
        }
        4 => {
            move_x(g, p);
            let mut vx = p.u16(g, Obj::VX);
            if p.faces_left(g) {
                vx = vx.wrapping_neg();
            }
            if (vx as i16) < 0 {
                to_stand(g, p);
            } else {
                animation::step(g, p);
            }
        }
        v => unreachable!("0x35b96 : value {v:#x} outside the original jump table"),
    }
}

// ---- parry (`+5` = 8, 0x39ff0) ----------------------------------------------------------------------


/// 0x39ff0: after a parry, the player slides backward (speed 0x400 braked by 0x40 per frame); at
/// rest, they resume the game at the action `+0x6f`. During the slide, the opponent's direction
/// held chains a counterattack (except the magic-user). Then terrain and screen limits.
pub fn parry(g: &mut Ctx, p: Player) {
    match p.u8(g, Player::ACTION) {
        0 => {
            p.set_u8(g, Player::ACTION, 2);
            p.set_u8(g, 0x3e, 0);
            set_vx_pair(g, p, 0x400, (-64i16) as u16);
            parry_slide(g, p);
        }
        2 => parry_slide(g, p),
        v => unreachable!("0x39ff0 : value {v:#x} outside the original jump table"),
    }
    crate::terrain::push(g.b, p.obj());
    super::super::walk::clamp_to_screen(g.b, p);
}

pub(super) fn parry_slide(g: &mut Ctx, p: Player) {
    move_x(g, p);
    let mut vx = p.u16(g, Obj::VX);
    if p.faces_left(g) {
        vx = vx.wrapping_neg();
    }
    if (vx as i16) < 0 {
        p.set_u32(g, Obj::VX, 0);
        p.set_u8(g, Player::ACTIVE, 1);
        p.set_state(g, 0x0200_0000);
        let resume = p.u8(g, Player::TIMER_6F);
        p.set_u8(g, Player::ACTION, resume);
        return;
    }
    let class = p.class(g);
    if class == 4 {
        return;
    }
    let toward = if p.faces_left(g) { 0 } else { 1 };
    if p.u8(g, Player::KEYS_HELD) >> toward & 1 != 0 {
        p.set_u8(g, Player::ACTIVE, 1);
        p.set_state(g, 0x0200_0c00);
        p.set_u32(g, Obj::VX, 0);
        let kind = player_data().riposte_kinds.at8(g.b, class as u32);
        p.set_u8(g, Player::SUBACTION, kind);
    }
}

// ---- special-move actions (called by `combos`) --------------------------------------------

/// Attack (action 0xc) of sub-action `kind` in the stored direction `facing_from` (or the current direction).
pub(super) fn combo_attack(g: &mut Ctx, p: Player, kind: u8, facing_from: Option<u32>) -> bool {
    p.set_u8(g, Player::HITSTOP, 0);
    p.set_state(g, State::pack(2, 0, 0xc, 0).0);
    p.set_u8(g, Player::SUBACTION, kind);
    if let Some(at) = facing_from {
        let facing = p.u8(g, at);
        p.set_u8(g, Player::FACING, facing);
    }
    item_curse(g, p);
    true
}

/// A special-move action; returns true if it changed the state (Z at 0 in the original).
#[derive(Clone, Copy, Debug)]
pub enum Combo {
    /// 0x37386: run in the direction of the sequence (true only to the left: accidental Z).
    Dash,
    /// 0x373a8: action 0x10, on the ground and outside an attack.
    Action10,
    /// 0x373cc: action 0x12 (or 0x1a with speed class 2).
    Action12,
    /// 0x3733c: attack of sub-action 10.
    Attack10,
    /// 0x37366: attack of sub-action 12.
    Attack12,
    /// 0x373f4: attack of sub-action 22.
    Attack22,
    /// 0x3740e: attack of sub-action 24 (except with 8 HP or less, cleric excepted).
    Attack24,
    /// 0x3743e: attack of sub-action 26 of the dwarf, in the middle of an attack.
    DwarfAttack26,
    /// 0x3746a: attack of sub-action 30 (spell 36 or 44 in row 2).
    Attack30,
    /// 0x374aa: the thief grabs an enemy from behind.
    Backstab,
}

pub fn combo(g: &mut Ctx, p: Player, which: Combo) -> bool {
    let slow = p.u8(g, Player::SLOW) != 0;
    match which {
        Combo::Attack10 => !slow && combo_attack(g, p, 10, Some(0x1b7)),
        Combo::Attack12 => combo_attack(g, p, 12, Some(0x1bd)),
        Combo::Attack22 => combo_attack(g, p, 22, None),
        Combo::Dash => {
            if slow || p.u8(g, Player::ACTION) == 0xc {
                return false;
            }
            p.set_state(g, State::pack(2, 0, 6, 0).0);
            let facing = p.u8(g, 0x1a5);
            p.set_u8(g, Player::FACING, facing);
            facing != 0
        }
        Combo::Action10 => {
            if p.u16(g, Obj::Z) != 0 || p.u8(g, Player::ACTION) == 0xc {
                return false;
            }
            p.set_state(g, State::pack(2, 0, 0x10, 0).0);
            let facing = p.u8(g, 0x1ab);
            p.set_u8(g, Player::FACING, facing);
            true
        }
        Combo::Action12 => {
            if p.u8(g, Player::SPEED_CLASS) == 2 {
                p.set_state(g, State::pack(2, 0, 0x1a, 0).0);
                p.set_u8(g, Player::SCRIPT_TIMER, 0);
            } else {
                p.set_state(g, State::pack(2, 0, 0x12, 0).0);
            }
            true
        }
        Combo::Attack24 => {
            if p.class(g) != 1 && p.u16(g, Player::HP) <= 8 {
                return false;
            }
            p.set_state(g, State::pack(2, 0, 0xc, 0).0);
            p.set_u8(g, Player::SUBACTION, 24);
            item_curse(g, p);
            true
        }
        Combo::DwarfAttack26 => {
            if p.u8(g, Player::ACTION) != 0xc || p.u8(g, Player::SUBACTION) != 0 {
                return false;
            }
            combo_attack(g, p, 26, None)
        }
        Combo::Attack30 => {
            let spell = if p.class(g) == 2 { 36 } else { 44 };
            if p.u8(g, Player::SPECIAL_COUNT) != 2 || p.u8(g, Player::INVENTORY) != spell {
                return false;
            }
            p.set_state(g, State::pack(2, 0, 0xc, 0).0);
            p.set_u8(g, Player::SUBACTION, 30);
            true
        }
        Combo::Backstab => backstab(g, p),
    }
}

/// Orientation of an object (0 or 0x20).
pub(super) const FACING: u32 = 0x2c;

/// Body boxes (data): one pointer per box set, 8-byte entries.
pub(super) const BODY_BOXES: u32 = 0x11bffa;

/// 0x374aa: an enemy on the ground, facing the same direction as the player (hence from behind), grabbable and
/// close enough is grabbed; the player goes to attack of sub-action 26.
pub(super) fn backstab(g: &mut Ctx, p: Player) -> bool {
    if ram::FROZEN.get(g.b) != 0 {
        return false;
    }
    for enemy in pools::ENEMY.iter() {
        let e = enemy.0;
        let b = &mut *g.b;
        let ok = rd8(b, e) != 0
            && rd8(b, e + 1) & 1 != 0
            && rd8(b, e + 4) == 2
            && rd16(b, e + Obj::Z) == 0
            && rd16(b, e + 0x62) != 0
            && rd8(b, e + 0x86) == 0
            && rd8(b, e + 0x40) == 0
            && rd32(b, e + 0x94) & 1 << 30 == 0
            && rd8(b, p.at(Player::FACING)) == rd8(b, e + FACING);
        if !ok {
            continue;
        }
        let kind = rd8(b, e + 2) as u32;
        if players("ungrabbable").at8(b, kind) != 0 {
            continue;
        }
        let box_set = rd8(b, e + 0x2e) as u32;
        let set = long(b, BODY_BOXES + 4 * box_set);
        let body = set + 8 * rd8(b, e + 0x38) as u32;
        let facing = rd8(b, p.at(0x1cf));
        wr8(b, p.at(Player::FACING), facing);
        if !within_reach(b, p.0, e, body) {
            continue;
        }
        wr8(b, e, 2);
        wr32(b, e + 4, 0x0400_0000);
        wr8(b, e + 0x49, 100);
        let index = rd8(b, p.at(Player::INDEX));
        wr8(b, e + 0x48, index);
        wr16(b, e + 0x50, p.0 as u16);
        wr32(b, p.at(Player::MAIN), 0x0200_0c00);
        wr8(b, p.at(0x47), 88);
        wr8(b, p.at(Player::SUBACTION), 26);
        wr16(b, p.at(0x50), e as u16);
        return true;
    }
    false
}

/// 0x375a8: is enemy `e` (body box `body`) within grabbing range of player `me`? (in
/// depth ±(16 + half-depth), in x in front of the player ±(48 + the enemy's byte `+1`, like the original)).
pub(super) fn within_reach(b: &mut cps2::Cps2Bus, me: u32, e: u32, body: u32) -> bool {
    let sbyte = |b: &mut cps2::Cps2Bus, a: u32| rd8(b, a) as i8 as i16 as u16;
    let near = |gap: u16, half: u16| gap.wrapping_add(half) <= half.wrapping_add(half);
    let gap = 4u16.wrapping_add(rd16(b, me + Obj::Y)).wrapping_sub(sbyte(b, body + 4).wrapping_add(rd16(b, e + Obj::Y)));
    if !near(gap, 16u8.wrapping_add(rd8(b, body + 5)) as u16) {
        return false;
    }
    let mut reach = (-48i16) as u16;
    if rd8(b, me + FACING) != 0 {
        reach = reach.wrapping_neg();
    }
    let mut offset = sbyte(b, body);
    if rd8(b, e + FACING) != 0 {
        offset = offset.wrapping_neg();
    }
    let gap = reach.wrapping_add(rd16(b, me + Obj::X)).wrapping_sub(offset.wrapping_add(rd16(b, e + Obj::X)));
    near(gap, 48u8.wrapping_add(rd8(b, e + 1)) as u16)
}
