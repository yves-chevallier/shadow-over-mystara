//! Player status effects: blinking and aura colors, countdown.
//!
//! Three aura counters (`AURA`, `AURA_B`, `AURA_C`, bytes `+0x182..+0x184`) are set by
//! items and spells; while one is non-zero, the player blinks (A) or changes palette (B, C). A
//! palette is a block of 16 colors (32 bytes) copied into the CPS palette RAM at the character's
//! slot.

use super::{ram, Ctx, Player};
use crate::hw::mem::wr32;
use crate::actors::player::tables::{player_data, long, Aligned};

/// Character palette RAM (two bytes per color, 16 colors per block).
const PALETTE_RAM: u32 = 0x91_c000;


/// Aura palettes: each character has 8 blocks of 32 bytes, alternated every 8 frames.
mod sources {
    pub const AURA_B: u32 = 0x378c2;
    pub const AURA_C: u32 = 0x384c2;
    /// Normal palette, restored at the end of an aura.
    pub const NORMAL: u32 = 0x390c2;
}

/// Copies 32 palette bytes from `src` to `dst`.
fn copy_palette(g: &mut Ctx, src: u32, dst: u32) {
    cov!();
    for i in 0..8 {
        let v = long(g.b, src + 4 * i);
        wr32(g.b, dst + 4 * i, v);
    }
}

fn palette_slot(g: &mut Ctx, p: Player) -> u32 {
    cov!();
    let character = p.u8(g, Player::CHARACTER) as u32;
    player_data().aura_palette_offsets.word_at(g.b, 2 * character) as i16 as i32 as u32
}

/// 0x377cc: aura A, the player blinks every other frame (the "hidden" bit follows the counter).
pub fn blink(g: &mut Ctx, p: Player) {
    cov!();
    if p.u8(g, Player::AURA) == 0 {
        return;
    }
    let mut flags = p.u8(g, Player::FLAGS) | 2;
    let phase = ram::FRAME_COUNTER.get(g.b).wrapping_add(p.u8(g, Player::INDEX)) & 3;
    if phase & 1 == 0 {
        flags &= !2;
    }
    p.set_u8(g, Player::FLAGS, flags);
}

/// Aura B or C: every 8 frames, a new palette color among eight.
fn cycle_palette(g: &mut Ctx, p: Player, aura: u32, base: u32) {
    cov!();
    if p.u8(g, aura) == 0 {
        return;
    }
    let tick = ram::FRAME_COUNTER.get(g.b).wrapping_add(p.u8(g, Player::INDEX)) & 7;
    if tick != 0 {
        return;
    }
    let step = p.u8(g, Player::AURA_STEP).wrapping_add(1);
    p.set_u8(g, Player::AURA_STEP, step);
    let character = p.u8(g, Player::CHARACTER) as u32;
    let src = base + ((step as u32 & 7) << 5) + (character << 8);
    let dst = PALETTE_RAM + palette_slot(g, p);
    copy_palette(g, src, dst);
}

/// 0x377f4: aura B.
pub fn cycle_aura_b(g: &mut Ctx, p: Player) {
    cov!();
    cycle_palette(g, p, Player::AURA_B, sources::AURA_B)
}

/// 0x3782e: aura C.
pub fn cycle_aura_c(g: &mut Ctx, p: Player) {
    cov!();
    cycle_palette(g, p, Player::AURA_C, sources::AURA_C)
}

/// 0x37882: end of an aura, normal palette of the character.
pub fn restore_palette(g: &mut Ctx, p: Player) {
    cov!();
    let slot = palette_slot(g, p);
    copy_palette(g, sources::NORMAL + slot, PALETTE_RAM + slot);
}

/// 0x39242: death countdown (`DYING_TIMER`); one effect every 8 frames.
pub fn dying_countdown(g: &mut Ctx, p: Player) {
    cov!();
    let left = p.u16(g, Player::DYING_TIMER) as i16;
    if left <= 0 {
        return;
    }
    p.set_u16(g, Player::DYING_TIMER, (left - 1) as u16);
    if ram::FRAME_COUNTER.get(g.b) & 7 == 0 {
        super::spawn::dying_shards(g, p);
    }
}
