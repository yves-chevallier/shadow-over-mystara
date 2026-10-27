//! Effets de statut des joueurs : clignotement et couleurs d'aura, compte à rebours.
//!
//! Trois compteurs d'aura (`AURA`, `AURA_B`, `AURA_C`, octets `+0x182..+0x184`) sont posés par les
//! objets et sorts ; tant qu'un est non nul, le joueur clignote (A) ou change de palette (B, C). Une
//! palette est un bloc de 16 couleurs (32 octets) copié dans la RAM de palette CPS à l'emplacement du
//! personnage.

use super::{ram, Ctx, Player};
use crate::mem::{rd32, wr32};
use crate::machine::Machine;

/// RAM de palette des personnages (deux octets par couleur, 16 couleurs par bloc).
const PALETTE_RAM: u32 = 0x91_c000;

/// Décalage (mots du code, 0x378aa) du bloc de palette de chaque personnage dans la RAM de palette
/// et dans les blocs sources.
const PALETTE_OFFSETS: u32 = 0x378aa;

/// Palettes d'aura : chaque personnage a 8 blocs de 32 octets, alternés toutes les 8 images.
mod sources {
    pub const AURA_B: u32 = 0x378c2;
    pub const AURA_C: u32 = 0x384c2;
    /// Palette normale, rétablie à la fin d'une aura.
    pub const NORMAL: u32 = 0x390c2;
}

/// Recopie 32 octets de palette de `src` vers `dst`.
fn copy_palette(g: &mut Ctx, src: u32, dst: u32) {
    cov!();
    for i in 0..8 {
        let v = rd32(g.b, src + 4 * i);
        wr32(g.b, dst + 4 * i, v);
    }
}

fn palette_slot(g: &mut Ctx, p: Player) -> u32 {
    cov!();
    let character = p.u8(g, Player::CHARACTER) as u32;
    g.prog16(PALETTE_OFFSETS + 2 * character) as i16 as i32 as u32
}

/// 0x377cc : aura A, le joueur clignote une image sur deux (le bit « caché » suit le compteur).
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

/// Aura B ou C : toutes les 8 images, une nouvelle couleur de palette parmi huit.
fn cycle_palette(g: &mut Ctx, p: Player, aura: u32, base: u32) {
    cov!();
    if p.u8(g, aura) == 0 {
        return;
    }
    let tick = ram::FRAME_COUNTER.get(g.b).wrapping_add(p.u8(g, Player::INDEX)) & 7;
    if tick != 0 {
        return;
    }
    let step = p.u8(g, 0x7f).wrapping_add(1);
    p.set_u8(g, 0x7f, step);
    let character = p.u8(g, Player::CHARACTER) as u32;
    let src = base + ((step as u32 & 7) << 5) + (character << 8);
    let dst = PALETTE_RAM + palette_slot(g, p);
    copy_palette(g, src, dst);
}

/// 0x377f4 : aura B.
pub fn cycle_aura_b(g: &mut Ctx, p: Player) {
    cov!();
    cycle_palette(g, p, Player::AURA_B, sources::AURA_B)
}

/// 0x3782e : aura C.
pub fn cycle_aura_c(g: &mut Ctx, p: Player) {
    cov!();
    cycle_palette(g, p, Player::AURA_C, sources::AURA_C)
}

/// 0x37882 : fin d'une aura, palette normale du personnage.
pub fn restore_palette(g: &mut Ctx, p: Player) {
    cov!();
    let slot = palette_slot(g, p);
    copy_palette(g, sources::NORMAL + slot, PALETTE_RAM + slot);
}

/// 0x39242 : compte à rebours de mort (`DYING_TIMER`) ; un effet toutes les 8 images.
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
