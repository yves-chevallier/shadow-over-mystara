//! Loop over the four players and dispatch of the states (P4).
//!
//! `update_players` (0x35292) is called once per frame by the game loop (0x1c86e). For
//! each active player, byte `+4` dispatches to the main state:
//!
//! | `+4` | state |
//! |---|---|
//! | 0 | scripted (cinematics per scene, P3); `+5` selects the sub-state |
//! | 2 | normal game; `+5`: 0 action, 4 hit, 8 parried |
//! | 4 | on the ground / ejected (`+5` must be 0) |
//! | 6 | removed: save of the player, clearing of the record and its sub-objects |

use crate::objects::Obj;
use super::init::{clear_overlays_of, clear_player, UNLOCKED_FLAGS};
use super::states::sub;
use super::{actions, checks, combos, effects, hurt, ram, scripted, Ctx, Player};
use crate::mem::{wr32, wr8};
use crate::objects::lists;
use crate::player::tables::{player_data, Read};

/// One frame of all the active players.
pub fn update_all(g: &mut Ctx) {
    cov!();
    for p in Player::all() {
        if p.is_active(g) {
            update_one(g, p);
        }
    }
}

/// 0x352c4: dispatch according to the main state.
pub fn update_one(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::MAIN) {
        0 => scripted::update(g, p),
        2 => update_playing(g, p),
        4 => {
            if p.u8(g, Player::SUB) == 0 {
                hurt::downed(g, p);
            }
        }
        6 => remove(g, p),
        _ => {}
    }
}

/// 0x352f4: a player in normal game. Counter countdown, status effects, then the sub-state
/// handler, then the end-of-frame checks.
fn update_playing(g: &mut Ctx, p: Player) {
    cov!();
    loop {
        combos::detect(g, p);
        checks::near_death(g, p);
        let freeze = p.u8(g, Player::FREEZE_194);
        if freeze != 0 {
            p.set_u8(g, Player::FREEZE_194, freeze.wrapping_sub(1) & 0x3f);
        }
        let hitstop = p.u8(g, Player::HITSTOP);
        if hitstop != 0 && (hitstop as i8) >= 0 {
            // hit freeze: the handler does not run
            p.set_u8(g, Player::HITSTOP, hitstop - 1);
            return end_of_frame(g, p);
        }
        if p.u8(g, Player::SUB) == 0 {
            let invuln = p.u8(g, Player::INVULN);
            if invuln != 0 {
                p.set_u8(g, Player::INVULN, invuln - 1);
            }
        }
        let timer = p.u8(g, Player::RECOIL_TIMER);
        if timer != 0 {
            p.set_u8(g, Player::RECOIL_TIMER, timer - 1);
        }
        if timer <= 1 {
            p.set_u8(g, Player::RECOIL, 0);
        }
        if p.u8(g, Player::SPELL_FROZEN) != 0 {
            return end_of_frame(g, p);
        }
        if ram::PAUSE.get(g.b) == 0 && p.u8(g, Player::POWER_10F) == 0 {
            status_effects(g, p);
        }
        match p.u8(g, Player::SUB) {
            sub::ACTING => actions::act(g, p),
            sub::HURT => hurt::hurt(g, p),
            sub::PARRY => super::moves::parry(g, p),
            // the table 0x3541c has three entries (0x3553c, 0x3a0a8, 0x39ff0); no code writes another sub-state in the
            // normal game (`docs/code-mort.md`, "Sortie du Cpu, partie 6")
            other => unreachable!("normal-game sub-state {other:#x}: table 0x3541c has only 0, 4 and 8"),
        }
        if p.u16(g, Player::HP) == 0 {
            return finish(g, p);
        }
        // the speed aura (+0x183) makes the frame play twice, except during an attack 0xe
        if p.u8(g, Player::AURA_B) == 0
            || p.state(g) & !0xff == 0x0200_0e00
            || ram::SCENE.get(g.b) == 0x43
        {
            return end_of_frame(g, p);
        }
        let parity = p.u8(g, Player::DOUBLE_PARITY).wrapping_add(1) & 1;
        p.set_u8(g, Player::DOUBLE_PARITY, parity);
        if parity != 0 {
            return end_of_frame(g, p);
        }
    }
}

/// 0x35344..0x353a4: temporary bonuses (power, aura), blinking and aura colors.
fn status_effects(g: &mut Ctx, p: Player) {
    cov!();
    if p.u8(g, Player::POWERUP) != 0 {
        let left = p.u16(g, Player::POWERUP_TIMER).wrapping_sub(1);
        p.set_u16(g, Player::POWERUP_TIMER, left);
        if (left as i16) < 0 {
            p.set_u16(g, Player::POWERUP_TIMER, 0);
            p.set_u8(g, Player::POWERUP, 0);
        }
    }
    let auras = p.u8(g, Player::AURA).wrapping_add(p.u8(g, Player::AURA_B)).wrapping_add(p.u8(g, Player::AURA_C));
    if auras != 0 {
        let left = p.u16(g, Player::AURA_TIMER).wrapping_sub(1);
        p.set_u16(g, Player::AURA_TIMER, left);
        if (left as i16) < 0 {
            p.set_u8(g, Player::AURA, 0);
            p.set_u8(g, Player::AURA_B, 0);
            p.set_u8(g, Player::AURA_C, 0);
            let flags = p.u8(g, Player::FLAGS) & !2;
            p.set_u8(g, Player::FLAGS, flags);
            effects::restore_palette(g, p);
        }
    }
    effects::blink(g, p);
    effects::cycle_aura_b(g, p);
    effects::cycle_aura_c(g, p);
    effects::dying_countdown(g, p);
}

/// 0x353ea: weapon throw (button 3) or special attack (button 4).
fn end_of_frame(g: &mut Ctx, p: Player) {
    cov!();
    if !checks::throw_button(g, p) {
        checks::special_attack(g, p);
    }
    finish(g, p);
}

/// 0x353f4: end of frame.
fn finish(g: &mut Ctx, p: Player) {
    cov!();
    if ram::SCENE.get(g.b) == 0x29
        && p.u16(g, Obj::Z) == 0
        && (ram::FRAME_COUNTER.get(g.b).wrapping_add(p.u8(g, Player::INDEX)) & 0xf) == 0
    {
        crate::families::effects_creators::for_players::wisp(&mut g.leftovers, g.b, p.obj());
    }
    lists::ACTORS.reinsert(g.b, p.obj());
}

/// 0x35436: removes the player from the game: save of their data (score, items, character),
/// clearing of their record and their two sub-objects.
fn remove(g: &mut Ctx, p: Player) {
    cov!();
    lists::unlink(g.b, p.obj());
    let k = p.u8(g, Player::INDEX) as u32 & 3;
    record_high_score(g, p);
    let saved = UNLOCKED_FLAGS + 0x70 + 0x10 * k;
    let (a, b, c) = (p.u32(g, Player::NAME_HEAD), p.u32(g, Player::NAME_TAIL), p.u32(g, Player::SCORE_BCD));
    wr32(g.b, saved, a);
    wr32(g.b, saved + 4, b);
    wr32(g.b, saved + 8, c);
    for (i, field) in [Player::CHARACTER, Player::LEVEL, 0x193].into_iter().enumerate() {
        let v = p.u8(g, field);
        wr8(g.b, saved + 0xc + i as u32, v);
    }
    clear_player(g, k);
    clear_overlays_of(g, k);
}

/// High-score table: ten 14-byte entries at FF0FD0, from best to worst.
mod scores {
    pub const TABLE: u32 = 0xff_0fd0;
    pub const ENTRY: u32 = 0xe;
    pub const COUNT: u32 = 10;
    /// Fields: level, character, name (6 bytes), score (long), magic (`+0xd`).
    pub const LEVEL: u32 = 0x0;
    pub const CHARACTER: u32 = 0x1;
    pub const NAME: u32 = 0x2;
    pub const SCORE: u32 = 0x8;
    pub const MAGIC: u32 = 0xd;
}

/// 0x180c8: the score of the player leaving the game (`+0x110`) takes its place in the table, the worse ones
/// dropping a rank. A name starting with -1 to -7 is replaced by a predefined name.
fn record_high_score(g: &mut Ctx, p: Player) {
    let score = p.u32(g, Player::SCORE_BCD) as i32;
    let entry = |rank: u32| scores::TABLE + scores::ENTRY * rank;
    let score_of = |g: &mut Ctx, rank: u32| crate::mem::rd32(g.b, entry(rank) + scores::SCORE) as i32;
    let last = scores::COUNT - 1;
    if score < score_of(g, last) {
        return;
    }
    let (level, character, magic) = (p.u8(g, Player::LEVEL), p.u8(g, Player::CHARACTER), p.u8(g, Player::MAGIC_USED));
    let (name_a, name_b) = (p.u32(g, Player::NAME_HEAD), p.u16(g, Player::NAME_TAIL));
    let write = |g: &mut Ctx, at: u32| {
        wr8(g.b, at + scores::LEVEL, level);
        wr8(g.b, at + scores::CHARACTER, character);
        wr8(g.b, at + scores::MAGIC, magic);
        wr32(g.b, at + scores::SCORE, score as u32);
    };
    write(g, entry(last));
    wr32(g.b, entry(last) + scores::NAME, name_a);
    crate::mem::wr16(g.b, entry(last) + scores::NAME + 4, name_b);
    for rank in (0..last).rev() {
        if score < score_of(g, rank) {
            return;
        }
        let (from, to) = (entry(rank), entry(rank + 1));
        for off in [0u32, 1, 0xd] {
            let v = crate::mem::rd8(g.b, from + off);
            wr8(g.b, to + off, v);
        }
        for off in [2u32, 8] {
            let v = crate::mem::rd32(g.b, from + off);
            wr32(g.b, to + off, v);
        }
        let v = crate::mem::rd16(g.b, from + 6);
        crate::mem::wr16(g.b, to + 6, v);
        write(g, from);
        let preset = match (name_a >> 24) as u8 {
            0xff if ram::VERSION.get(g.b) == 0 => Some(8 * character as u32),
            0xff => Some(8 * (0xc + character as u32)),
            0xfe => Some(0x30),
            0xfd => Some(0x60),
            0xfc => Some(0x70),
            0xfb => Some(0x90),
            0xfa => Some(0xa8),
            0xf9 => Some(0xb8),
            _ => None,
        };
        let (a, b) = match preset {
            Some(off) => (player_data().score_names.at32(g.b, off), player_data().score_names.at16(g.b, off + 4)),
            None => (name_a, name_b),
        };
        wr32(g.b, from + scores::NAME, a);
        crate::mem::wr16(g.b, from + scores::NAME + 4, b);
    }
}
