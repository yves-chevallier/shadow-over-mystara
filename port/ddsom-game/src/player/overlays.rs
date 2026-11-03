//! Player sub-objects: weapon and shield drawn next to the character (P4).
//!
//! Each player has two 0x60-byte sub-objects (`init::overlay`): the first carries the held weapon
//! (`WEAPON_A`), the second the shield or the second weapon (`WEAPON_B`). Each frame, `update`
//! (0x392e4, called right after the player loop) re-snaps them to the player: same orientation,
//! offset and animation taken from ROM tables according to the character, the player's animation
//! frame (`+0x40`, `+0x42`, `+0x44`) and the weapon. It also makes the weapon palette sparkle
//! when a player is under a power bonus.
//!
//! Leftovers: the game loop chains this routine and the object dispatch loops without
//! resetting anything, and some handlers read the registers D2-D5 it leaves (D3 for example:
//! an effect copies the last horizontal offset). The code below therefore writes them (`Ctx::leftovers`) where
//! the original does, in the same format (byte, word or long) as it.

use super::checks::team_score_reached;
use super::init::overlay;
use super::{Ctx, Player, PLAYER_COUNT};
use crate::mem::{rd16, rd8, set_low_byte, set_low_word, wr16, wr32, wr8, A5};
use crate::objects::anim::load_entry;
use crate::objects::Obj;
use crate::player::ram;
use crate::player::tables::{player_data, byte, word, long, Read};

/// ROM tables (data): one pointer per character to rows of 8 bytes
/// (animation, x offset, z offset, flip, depth).
mod tables {
    /// Held weapon (`+0x40`) and low-numbered shield (`+0x42`).
    pub const BODY: u32 = 0x395d4;
    /// Shield numbered ≥ 0x28 (`+0x44`).
    pub const SHIELD_HIGH: u32 = 0x39604;
    /// Palettes that sparkle under bonus (8 blocks of 32 bytes).
    pub const POWER_PALETTES: u32 = 0x397e0;
}

/// Flag "the team is ready for the ultimate spell" (word at `A5-0x5fc2`).
const TEAM_READY: u32 = A5 - 0x5fc2;
/// Flag stopping the sparkles (byte at `A5-0x65ec`).
const NO_SPARKLE: u32 = A5 - 0x65ec;
/// Sparkling palette (CPS palette RAM).
const SPARKLE_RAM: u32 = 0x91_c1c0;

/// Fields of a player read here.
mod f {
    pub const WEAPON_FRAME: u32 = 0x40;
    pub const SHIELD_LOW_FRAME: u32 = 0x42;
    pub const SHIELD_HIGH_FRAME: u32 = 0x44;
    pub const HAS_WEAPON: u32 = 0x68;
    pub const WEAPON_KIND: u32 = 0x107;
    pub const POWERUP: u32 = 0x79;
    pub const SPARKLE_STEP: u32 = 0x52;
}

/// Table row of a sub-object: 8 bytes.
struct Row {
    anim: u16,
    dx: i16,
    dz: i16,
    /// Bytes 6 and 7 of the row: flip and depth (non-zero: true).
    flip: u8,
    behind: u8,
}

/// Common start of the sub-objects: D2.w = frame.
fn select_table(g: &mut Ctx, frame: u32) {
    cov!();
    set_low_word(&mut g.leftovers.d2, frame as u16);
}

/// Row `index` of the table of `character` (D2.w = index × 8).
fn row(g: &mut Ctx, table: u32, character: u32, index: u32) -> Row {
    cov!();
    let base = long(g.b, table + 4 * character) + 8 * index;
    set_low_word(&mut g.leftovers.d2, (index << 3) as u16);
    Row {
        anim: word(g.b, base),
        dx: word(g.b, base + 2) as i16,
        dz: word(g.b, base + 4) as i16,
        flip: byte(g.b, base + 6),
        behind: byte(g.b, base + 7),
    }
}

/// The sub-object follows the player's activity state and orientation.
fn follow(g: &mut Ctx, p: Player, o: Obj) {
    cov!();
    let (active, facing) = (p.u8(g, Player::ACTIVE), p.u8(g, Player::FACING));
    wr8(g.b, o.0, active);
    wr8(g.b, o.at(0x2c), facing);
}

/// Places sub-object `o` from row `r` of player `p`.
fn place(g: &mut Ctx, p: Player, o: Obj, r: &Row) {
    cov!();
    if r.flip != 0 {
        let facing = rd8(g.b, o.at(0x2c)) ^ 0x20;
        wr8(g.b, o.at(0x2c), facing);
    }
    wr16(g.b, o.at(0x10), if r.behind != 0 { 0x8000 } else { 0 });
    let dx = if p.u8(g, Player::FACING) != 0 { r.dx.wrapping_neg() } else { r.dx };
    wr16(g.b, o.at(0x08), dx as u16);
    wr16(g.b, o.at(0x0c), r.dz as u16);
    set_low_word(&mut g.leftovers.d3, dx as u16);
    set_low_word(&mut g.leftovers.d4, r.dz as u16);
    set_low_byte(&mut g.leftovers.d5, r.flip);
}

/// Loads animation `n` of the weapon script `weapon` into the sub-object.
fn load_animation(g: &mut Ctx, o: Obj, weapon: u32, n: u16) {
    cov!();
    let set = player_data().overlay_animations.at32(g.b, 4 * weapon);
    let script = crate::objects::anim::script_of(g.b, set, n);
    load_entry(g.b, o, script, false);
    if rd8(g.b, o.at(0x38)) != 0 {
        let facing = rd8(g.b, o.at(0x2c)) ^ 0x20;
        wr8(g.b, o.at(0x2c), facing);
    }
}

/// 0x393c8: held-weapon sub-object.
fn body(g: &mut Ctx, p: Player, o: Obj) {
    cov!();
    let hidden = (p.u8(g, Player::WEAPON_A) as i8) < 0 || p.u8(g, f::HAS_WEAPON) == 0;
    if hidden {
        p.set_u16(g, Player::OVERLAY_BODY, 0);
        return;
    }
    follow(g, p, o);
    wr8(g.b, o.at(0x2a), 1);
    wr8(g.b, o.at(0x2b), 0x12);
    p.set_u8(g, f::WEAPON_KIND, 1);
    let weapon = p.u8(g, Player::WEAPON_A);
    if weapon != 0x10 {
        wr8(g.b, o.at(0x2b), 0x13);
        p.set_u8(g, f::WEAPON_KIND, 2);
        if weapon != 0x11 {
            wr8(g.b, o.at(0x2b), 0x14);
            p.set_u8(g, f::WEAPON_KIND, 3);
            if weapon != 0x12 {
                p.set_u8(g, f::WEAPON_KIND, 5);
                if weapon != 0xb {
                    p.set_u8(g, f::WEAPON_KIND, 0);
                    wr8(g.b, o.at(0x2b), 0xe);
                    if p.u8(g, f::POWERUP) == 0 {
                        wr8(g.b, o.at(0x2a), 0);
                    }
                }
            }
        }
    }
    let frame = p.u16(g, f::WEAPON_FRAME) as u32;
    let character = p.u8(g, Player::CHARACTER) as u32;
    select_table(g, frame);
    if frame == 0 {
        p.set_u16(g, Player::OVERLAY_BODY, 0);
        return;
    }
    let r = row(g, tables::BODY, character, frame);
    place(g, p, o, &r);
    p.set_u16(g, Player::OVERLAY_BODY, o.0 as u16);
    load_animation(g, o, weapon as u32, r.anim);
}

/// 0x394a2: shield or second-weapon sub-object.
fn shield(g: &mut Ctx, p: Player, o: Obj) {
    cov!();
    let shield_kind = p.u8(g, Player::WEAPON_B);
    let weapon = p.u8(g, Player::WEAPON_A);
    if (shield_kind as i8) < 0 || weapon == 8 || weapon == 0x18 {
        p.set_u16(g, Player::OVERLAY_ATTACK, 0);
        return;
    }
    let character = p.u8(g, Player::CHARACTER) as u32;
    if shield_kind >= 0x28 {
        let frame = p.u16(g, f::SHIELD_HIGH_FRAME) as u32;
        set_low_word(&mut g.leftovers.d2, frame as u16);
        if frame == 0 {
            p.set_u16(g, Player::OVERLAY_ATTACK, 0);
            return;
        }
        follow(g, p, o);
        wr8(g.b, o.at(0x2a), 2);
        wr8(g.b, o.at(0x2b), 0x10);
        select_table(g, frame);
        let mut r = row(g, tables::SHIELD_HIGH, character, frame);
        // one palette per pair of characters: the animation index is shifted by 0x20 per class
        r.anim = r.anim.wrapping_add(((character >> 1) << 5) as u16);
        place(g, p, o, &r);
        p.set_u16(g, Player::OVERLAY_ATTACK, o.0 as u16);
        load_animation(g, o, shield_kind as u32, r.anim);
        return;
    }
    if p.u8(g, f::HAS_WEAPON) == 0 {
        p.set_u16(g, Player::OVERLAY_ATTACK, 0);
        return;
    }
    let frame = p.u16(g, f::SHIELD_LOW_FRAME) as u32;
    set_low_word(&mut g.leftovers.d2, frame as u16);
    if frame == 0 {
        p.set_u16(g, Player::OVERLAY_ATTACK, 0);
        return;
    }
    follow(g, p, o);
    wr8(g.b, o.at(0x2a), 0);
    let team_ready = rd16(g.b, TEAM_READY) != 0;
    if p.u8(g, f::POWERUP) != 0 || (team_ready && shield_kind == 0x21) {
        wr8(g.b, o.at(0x2a), 1);
        wr8(g.b, o.at(0x2b), 0xe);
    }
    select_table(g, frame);
    let r = row(g, tables::BODY, character, frame);
    place(g, p, o, &r);
    p.set_u16(g, Player::OVERLAY_ATTACK, o.0 as u16);
    load_animation(g, o, shield_kind as u32, r.anim);
    blink_unless_idle(g, p);
}

/// 0x39634: certain spells (magic-users 8 and 9, or wand charge 0..5) make the weapon blink:
/// the object is hidden every other frame.
fn blink_unless_idle(g: &mut Ctx, p: Player) {
    cov!();
    let character = p.u8(g, Player::CHARACTER);
    if !matches!(character, 8 | 9) && p.u8(g, Player::AURA) != 0 {
        return;
    }
    // D.b receives each counter examined: the last one remains, whether or not it matches
    let kind = p.u8(g, Player::WEAPON_B);
    let charge = match kind {
        0x24 => p.u8(g, Player::GLOW_SHIELD),
        0x25 => p.u8(g, Player::GLOW_WEAPON),
        0x26 => p.u8(g, Player::GLOW_RING),
        _ => p.u8(g, Player::GLOW_OTHER),
    };
    if !(0x24..=0x27).contains(&kind) || (charge as i8) > 5 {
        return;
    }
    let phase = ram::FRAME_COUNTER.get(g.b) & 7;
    if phase & 4 != 0 {
        p.set_u16(g, Player::OVERLAY_ATTACK, 0);
    }
}

/// 0x397a8: scrolls the sparkling weapon palette (eight blocks, one every 8 frames).
/// `target` is the sub-object whose `SPARKLE_STEP` byte counts the steps (the original uses the
/// last sub-object handled).
fn sparkle(g: &mut Ctx, target: u32) {
    cov!();
    let phase = ram::FRAME_COUNTER.get(g.b) & 7;
    if phase != 0 {
        return;
    }
    let step = rd8(g.b, target + f::SPARKLE_STEP).wrapping_add(1) & 7;
    wr8(g.b, target + f::SPARKLE_STEP, step);
    let src = tables::POWER_PALETTES + ((step as u32) << 5);
    // the original loads the eight longs into D0-D6 and A3, then stores them all at once: D2-D5 stay
    let mut words = [0u32; 8];
    for (i, w) in words.iter_mut().enumerate() {
        *w = long(g.b, src + 4 * i as u32);
        wr32(g.b, SPARKLE_RAM + 4 * i as u32, *w);
    }
    (g.leftovers.d2, g.leftovers.d3, g.leftovers.d4, g.leftovers.d5) = (words[2], words[3], words[4], words[5]);
}

/// 0x392e4: updates all the sub-objects. A0 on entry stays in place if no player is active.
pub fn update(g: &mut Ctx) {
    cov!();
    wr16(g.b, TEAM_READY, 0);
    if ram::PLAYERS_ALIVE.get(g.b) as i8 >= 3 {
        let reached = team_score_reached(g);
        // 0x37658 leaves D5 = its exhausted loop counter
        g.leftovers.d5 = 0xffff;
        if reached {
            wr16(g.b, TEAM_READY, 1);
        }
    }
    // Original glitch: the sparkle counts its steps in the last sub-object handled (A0); without one, in the record of
    // player 4, the A0 that the player loop (0x35292) leaves
    let mut last = Player::nth(PLAYER_COUNT - 1).0;
    for k in 0..PLAYER_COUNT {
        let p = Player::nth(k);
        if p.u8(g, Player::ACTIVE) == 0 || p.u16(g, Player::MAIN) == 0x402 {
            continue;
        }
        body(g, p, Obj(overlay(k, false)));
        last = overlay(k, true);
        shield(g, p, Obj(last));
    }
    if rd8(g.b, NO_SPARKLE) != 0 {
        return;
    }
    let powered: u8 = Player::all().map(|q| q.u8(g, f::POWERUP)).fold(0, |a, b| a.wrapping_add(b));
    if powered != 0 || Player::all().any(|q| q.u8(g, Player::WEAPON_B) == 0x21) {
        sparkle(g, last);
    }
}
