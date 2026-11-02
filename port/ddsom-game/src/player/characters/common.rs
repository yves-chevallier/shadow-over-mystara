//! Elements common to the code of the six characters (P5).
//!
//! Each character (fighter, cleric, elf, dwarf, magic-user, thief) has its own actions: jump,
//! attack, special attack, action 0x10. They are written on the same model and share a
//! toolbox that the original program stores at the end of the fighter's code (0x3f036-0x3fbb8):
//! continuation of a strike, orientation, freezing of the other players, dust, darkening of
//! the screen during a spell, end of spell. This toolbox is here.
//!
//! Vocabulary of the actions:
//! - the action step (`Player::STEP`, always even) selects the automaton stage;
//! - the animation script signals an event in `field::EVENT` (1: landed hit, 2: second gesture);
//!   the action reads it then clears it;
//! - sounds and effects are created by original routines called through `call`.

use super::super::spawn;
use super::super::actions::to_stand;
use super::super::combos::clear_recognizers;
use super::super::init::init_weapon_stats;
use super::super::walk::{move_x, move_xz, move_z};
use super::super::{animation, ram, Ctx, Player};
use crate::mem::{wr16, wr8, A5};
use crate::objects::Obj;
use crate::machine::Machine;
use crate::player::external as ext;

/// Bytes of the player record specific to the characters' actions.
pub mod field {
    /// Event set by the animation script: 1 landed hit (or throw), 2 second gesture.
    pub const EVENT: u32 = 0x33;
    /// Player effect to create at the end of the frame (variant number; 0: nothing).
    pub const PENDING_EFFECT: u32 = 0x46;
    /// Movement carried by the current attack (h): 1 the attack box stays active.
    pub const MOVE_STATE: u32 = 0x18f;
    /// Kind of the current movement (h): 1 jump or charge, 2 spinning attack.
    pub const MOVE_KIND: u32 = 0x18e;
    /// An attack has already been launched during this jump.
    pub const AIR_ATTACK_DONE: u32 = 0x6b;
    /// The player changed direction in the air (only once per jump).
    pub const AIR_STEERED: u32 = 0x73;
    /// Abscissa at takeoff (word).
    pub const TAKEOFF_X: u32 = 0x80;
    /// Strikes that landed during the attack: bit k = the strike of step k.
    pub const HITS: u32 = 0x70;
    /// Rank in the strike chain (0, 1, 2...).
    pub const COMBO_RANK: u32 = 0x72;
    /// Window (frames) during which the attack button must stay pressed to chain.
    pub const COMBO_WINDOW: u32 = 0x74;
    /// The attack button was released during the window.
    pub const COMBO_RELEASED: u32 = 0x75;
    /// Counter specific to the action (frames, gesture countdowns).
    pub const TIMER: u32 = 0x168;
    /// The other players are frozen by the spell (1); one on the ground or protected is not affected.
    pub const SPELL_FROZEN: u32 = 0x16a;
    /// Row chosen in the special attacks / items table (`checks::special_attack`).
    pub const SPECIAL_KIND: u32 = 0x15e;
    /// Row of the item in the equipment list currently in use (number 0x56: used up).
    pub const ITEM: u32 = 0x140;
    /// Remaining charges of the item currently in use.
    pub const ITEM_CHARGES: u32 = 0x148;
    /// New equipment to put on (swap item).
    pub const ITEM_NEW: u32 = 0x15f;
    /// The spell is cancelled (by a protection): only the failure spark is played.
    pub const SPELL_CANCELLED: u32 = 0x189;
    /// Target object created by the spell (word: address).
    pub const SPELL_TARGET: u32 = 0x1ea;
    /// The spell hits an ally in range (1) or only the caster (0): set by `heal_scan` and the blessing.
    pub const AREA: u32 = 0x1ee;
}

/// Scene in which the shock effects (0xb10c8) are not created.
const NO_SHOCK_SCENE: u8 = 41;

/// Original routines called by these tools.
pub mod rom {
    /// Sound associated with the spark of `item_spark`.
    pub const SPARK_SOUND: u32 = 0xea166;
    /// Voice of the character with the held weapon (D4 = weapon number).
    pub const WEAPON_VOICE: u32 = 0xe7e10;
    /// Creation of a player effect (PEFF pool) of the type given by D0, according to the weapon.
    pub const SPAWN_EFFECT: u32 = 0x4c58e;
    /// Task slot (offset in the task table, D0 of 0x9b6) of the spell's screen tasks.
    pub const SCREEN_TASK_SLOT: u32 = 0x10;
    /// Programs of the spell's screen tasks: darken and restore.
    pub const DIM_TASK: u32 = 0x153e;
    pub const UNDIM_TASK: u32 = 0x1536;
}

/// ROM tables (PC-relative in the original).
mod tables {
    /// Running dust: (vx, vy) per class, four bytes.
    pub const DUST_STAND: u32 = 0x3faac;
    pub const DUST_RUN: u32 = 0x3fb02;
    pub const DUST_JUMP: u32 = 0x3fb48;
    /// Item sparks: rows of 8 bytes (D0, D1, D2, height) indexed by kind.
    pub const SPARKS: u32 = 0x3fbb8;
}

/// Advances the action's automaton by one step (`addq.b #2,(7,A0)`).
pub fn next_step(g: &mut Ctx, p: Player) {
    let step = p.u8(g, Player::STEP).wrapping_add(2);
    p.set_u8(g, Player::STEP, step);
}

/// Waits for the end of the animation then returns to the standing state.
pub fn wait_then_stand(g: &mut Ctx, p: Player) {
    cov!();
    if animation::step(g, p) {
        to_stand(g, p);
    }
}

/// Animation step (0x2d58) with the D0.b that the original leaves, which some routines called afterwards read as a
/// parameter of the created effect (original glitch; `DDSOM_POISON=anim`: long_3p_mortels, frame 22018): the flags of
/// the entry left behind when the duration runs out, otherwise `d0`, the value of D0.b before the call, inherited from
/// the original dispatch (the action step for the table actions, twice the special attack number for the others).
/// Returns (end of the script, D0.b).
pub fn step_with_d0(g: &mut Ctx, p: Player, d0: u8) -> (bool, u8) {
    cov!();
    let o = p.obj();
    let entry_ends = o.u16(g.b, Obj::ANIM_TIMER) == 1;
    let flags = o.u8(g.b, Obj::ANIM_FLAGS);
    let finished = crate::objects::anim::step(g.b, o, true).finished();
    (finished, if entry_ends { flags } else { d0 })
}

/// Plays the voice of the held weapon (D4 = `WEAPON_A`).
pub fn weapon_voice(g: &mut Ctx, p: Player) {
    cov!();
    let weapon = p.u8(g, Player::WEAPON_A) as u32;
    crate::sound::play_with(g.b, rom::WEAPON_VOICE, crate::sound::Source { object: Some(Obj(p.0)), a1: None, d4: Some(weapon) });
}

/// Plays the voice of the weapon held in the left hand (D4 = `WEAPON_B`, magic-user).
pub fn weapon_voice_b(g: &mut Ctx, p: Player) {
    cov!();
    let weapon = p.u8(g, Player::WEAPON_B) as u32;
    crate::sound::play_with(g.b, rom::WEAPON_VOICE, crate::sound::Source { object: Some(Obj(p.0)), a1: None, d4: Some(weapon) });
}

/// Starts animation `n`, then plays the voice of the left-hand weapon (`WEAPON_B`, magic-user).
pub fn start_with_voice_b(g: &mut Ctx, p: Player, n: u16) {
    cov!();
    animation::start_plain(g, p, n);
    weapon_voice_b(g, p);
}

/// Starts animation `n` of the table without weapon offset, then plays the weapon voice.
pub fn start_with_voice(g: &mut Ctx, p: Player, n: u16) {
    cov!();
    animation::start_plain(g, p, n);
    weapon_voice(g, p);
}

/// The held weapon is one of the two weapons with a particular gesture (numbers 8 and 0x18) (h).
pub fn holds_special_weapon(g: &mut Ctx, p: Player) -> bool {
    matches!(p.u8(g, Player::WEAPON_A), 8 | 0x18)
}

/// The second equipment is of a heavy class (number ≥ 0x28).
pub fn holds_heavy_second(g: &mut Ctx, p: Player) -> bool {
    p.u8(g, Player::WEAPON_B) >= 0x28
}

/// End of the attack: if a single effect request is pending (`field::PENDING_EFFECT`), the matching player
/// effect is created (0x4c58e).
pub fn flush_pending_effect(g: &mut Ctx, p: Player) {
    cov!();
    let kind = p.u8(g, field::PENDING_EFFECT) as u32;
    if kind != 0 {
        p.set_u8(g, field::PENDING_EFFECT, 0);
        spawn::weapon_trail(g, p, kind as u8);
    }
}

/// Jump speed of type `kind` (0..3), read from the table `table`
/// of the ROM: rows of four words (vx, ax, vz, az); the direction follows the orientation (0x3fb60).
pub fn launch_jump(g: &mut Ctx, p: Player, table: u32, kind: u8) {
    cov!();
    let row = table + 8 * kind as u32;
    for (i, off) in [0x14, 0x16, 0x18, 0x1a].into_iter().enumerate() {
        let v = g.data16(row + 2 * i as u32);
        p.set_u16(g, off, v);
    }
    if p.u8(g, Player::FACING) == 0 {
        negate_horizontal_speed(g, p);
    }
}

/// `neg.l (0x14,A0)`: the long (speed, acceleration) is negated as a block.
pub fn negate_horizontal_speed(g: &mut Ctx, p: Player) {
    let v = p.u32(g, 0x14).wrapping_neg();
    p.set_u32(g, 0x14, v);
}

/// The horizontal speed has changed direction relative to the facing (it is negative once oriented
/// in the facing direction): the momentum is exhausted.
pub fn momentum_reversed(g: &mut Ctx, p: Player) -> bool {
    let v = p.u16(g, 0x14) as i16;
    let along = if p.u8(g, Player::FACING) == 0 { v.wrapping_neg() } else { v };
    along < 0
}

/// Horizontal drift of a jump (except a jump in place): the character advances in X; if it changed
/// direction in the air (`AIR_STEERED`), it no longer moves until it has returned to its starting point.
pub fn drift(g: &mut Ctx, p: Player) {
    cov!();
    let jump_kind = p.u8(g, 0x67);
    if jump_kind == 0 {
        return;
    }
    if p.u8(g, field::AIR_STEERED) != 0 {
        // the direction changed in the air: the momentum is resumed until back at the start
        let travelled = p.u16(g, 0x08).wrapping_sub(p.u16(g, field::TAKEOFF_X)) as i16;
        let travelled = if (p.u16(g, 0x16) as i16) < 0 { travelled } else { travelled.wrapping_neg() };
        if travelled < 0 {
            return;
        }
    }
    move_x(g, p);
}

/// 0x3e712 / 0x3fe88 / 0x422f8 / 0x45c54: in the air, a direction opposite to the facing turns the
/// character around and gives a horizontal push of `push` (only once per jump).
pub fn air_steer(g: &mut Ctx, p: Player, push: i16) {
    cov!();
    if p.u8(g, field::AIR_STEERED) != 0 || (p.u16(g, 0x18) as i16) < 0 {
        return;
    }
    let keys = p.u8(g, Player::KEYS_HELD) & 3;
    if keys == 0 {
        return;
    }
    let (facing, push): (u8, i16) = if keys & 1 != 0 { (0x20, push) } else { (0, -push) };
    if facing == p.u8(g, Player::FACING) {
        return;
    }
    p.flip(g);
    p.set_u16(g, 0x16, push as u16);
    p.set_u8(g, field::AIR_STEERED, 1);
}

/// Button 1 has just been pressed in the air and no strike has yet been given during this jump.
pub fn air_attack_pressed(g: &mut Ctx, p: Player) -> bool {
    cov!();
    p.u8(g, field::AIR_ATTACK_DONE) == 0 && p.u8(g, Player::KEYS_NEW) & 0x10 != 0
}

/// 0x377a6: a cursed weapon that has been used too much (counter `A5+0x56` > 0x7f) falls: the player picks up
/// weapon 0xd and goes to action 0x26. Called at the end of a jump.
pub fn curse_check(g: &mut Ctx, p: Player) {
    cov!();
    if p.u8(g, Player::WEAPON_A) == 0xe && ram::CURSE_COUNT.get(g.b) > 0x7f {
        p.set_u8(g, Player::WEAPON_A, 0xd);
        p.set_state(g, 0x0200_2600);
        wr8(g.b, A5.wrapping_sub(0x5b82), 1);
    }
}

/// Aura that decreases with each aerial strike: at zero the aura goes out (0x3e7c6).
pub fn aura_hit_countdown(g: &mut Ctx, p: Player) {
    cov!();
    if p.u8(g, Player::AURA) == 0 {
        return;
    }
    let left = (p.u8(g, Player::AURA) - 1) & 3;
    p.set_u8(g, Player::AURA, left);
    if left == 0 {
        p.set_u16(g, Player::AURA_TIMER, 0);
        let flags = p.u8(g, Player::FLAGS) & !2;
        p.set_u8(g, Player::FLAGS, flags);
    }
}

/// 0x3f036: continuation of a strike. On the ground, a special attack requested with item 0x2d in hand
/// (key 4) or a turn with button 1 pressed launches the turning attack (`turn_attack`).
pub fn follow_up(g: &mut Ctx, p: Player) {
    cov!();
    if p.u16(g, 0x0c) != 0 {
        return;
    }
    if p.u8(g, Player::KEYS_NEW) & 0x80 != 0 && p.u8(g, 0x15c) == 0 && p.u8(g, field::ITEM) == 0x2d
        && turn_attack(g, p) {
            return;
        }
    let keys = p.u8(g, Player::KEYS_HELD);
    if keys & 3 == 0 {
        return;
    }
    let wanted = if keys & 1 != 0 { 0x20 } else { 0 };
    if wanted == p.u8(g, Player::FACING) || keys & 0x10 == 0 {
        return;
    }
    turn_attack(g, p);
}

/// 0x3f082: turning attack, reserved for equipment of class 0x28..0x30; goes to action
/// 0x16. Returns true if it starts.
pub fn turn_attack(g: &mut Ctx, p: Player) -> bool {
    cov!();
    if holds_special_weapon(g, p) || p.u8(g, Player::WEAPON_B).wrapping_sub(0x28) > 8 {
        return false;
    }
    p.set_state(g, 0x0200_1600);
    true
}

/// 0x3f8d2: the orientation follows the held direction.
pub fn face_held_direction(g: &mut Ctx, p: Player) {
    cov!();
    let dirs = p.u8(g, Player::KEYS_HELD) & 3;
    if dirs == 0 {
        return;
    }
    p.set_u8(g, Player::FACING, 0);
    if dirs & 2 == 0 {
        p.set_u8(g, Player::FACING, 0x20);
    }
}

/// 0x3f8f0: the spell freezes the other living players (`SPELL_FROZEN`) and makes them visible.
pub fn freeze_others(g: &mut Ctx, p: Player) {
    cov!();
    for q in Player::all() {
        if q.u8(g, Player::ACTIVE) == 0 || q.u8(g, Player::POWER_10F) != 0 {
            continue;
        }
        if q.u8(g, Player::MAIN) != 4 {
            let flags = q.u8(g, Player::FLAGS) & !2;
            q.set_u8(g, Player::FLAGS, flags);
        }
        q.set_u8(g, field::SPELL_FROZEN, (q != p) as u8);
    }
}

/// Darkens or restores the screen: starts the screen task `program` (0x153e / 0x1536) after the
/// fade 0x1686. `mode`: 1 darken, 2 restore.
fn screen_task(g: &mut Ctx, mode: u16, program: u32, fade: bool) {
    ram::FADE_DELAY.set(g.b, mode);
    ram::FADE_PAGES.set(g.b, 0xd);
    ram::FADE_STEPS.set(g.b, 5);
    if fade {
        crate::palette::full_brightness(g.b);
    }
    // 0x9b6 (A0 = program, D0 = slot); the new task reads none of the registers it inherits (`DDSOM_POISON=new`)
    crate::kernel::create_task(g.b, rom::SCREEN_TASK_SLOT, program);
}

/// 0x3f926: the spell begins, the screen darkens.
pub fn dim_screen(g: &mut Ctx, _p: Player) {
    cov!();
    screen_task(g, 1, rom::DIM_TASK, true);
}

/// 0x3f954: the spell is over, the screen is restored.
pub fn undim_screen(g: &mut Ctx, p: Player) {
    cov!();
    undim_screen_mode(g, p, 2);
}

/// Restores the screen with mode `mode`: the magic-user's code (0x48772) passes 1 instead of 2.
pub fn undim_screen_mode(g: &mut Ctx, _p: Player, mode: u16) {
    cov!();
    screen_task(g, mode, rom::UNDIM_TASK, false);
}

/// Start of a spell (next action step): game pause (`A5+0x3d`), spell indicator (`A5+0x4a`),
/// action duration (`field::TIMER`) if known from the start.
pub fn begin_spell(g: &mut Ctx, p: Player, timer: Option<u16>) {
    cov!();
    next_step(g, p);
    ram::PAUSE.set(g.b, 1);
    ram::SPELL_FLAG.set(g.b, 1);
    ram::SCROLL1_Y.set(g.b, 0);
    if let Some(timer) = timer {
        p.set_u16(g, field::TIMER, timer);
    }
}

/// End of spell: lifts the pause and the freezes, restores the screen, return to the standing state.
pub fn end_spell(g: &mut Ctx, p: Player) {
    cov!();
    end_spell_mode(g, p, 2);
}

/// `end_spell` with the character's screen restoration mode (see `undim_screen_mode`).
pub fn end_spell_mode(g: &mut Ctx, p: Player, mode: u16) {
    cov!();
    clear_spell_state(g);
    p.set_u8(g, field::SPELL_CANCELLED, 0);
    undim_screen_mode(g, p, mode);
    to_stand(g, p);
}

/// End of spell that does not touch the cancellation flag (the spell was not cancelled).
pub fn end_spell_keep_cancel(g: &mut Ctx, p: Player) {
    cov!();
    end_spell_keep_cancel_mode(g, p, 2);
}

/// `end_spell_keep_cancel` with the character's screen restoration mode.
pub fn end_spell_keep_cancel_mode(g: &mut Ctx, p: Player, mode: u16) {
    cov!();
    clear_spell_state(g);
    undim_screen_mode(g, p, mode);
    to_stand(g, p);
}

/// Falling back of the spell indicators: indicator and pause, chosen row, freeze of the four players.
pub fn clear_spell_state(g: &mut Ctx) {
    cov!();
    ram::SPELL_FLAG.set(g.b, 0);
    ram::PAUSE.set(g.b, 0);
    ram::SPELL_ROW.set(g.b, 0);
    for k in 0..4 {
        Player::nth(k).set_u8(g, field::SPELL_FROZEN, 0);
    }
}

/// One charge of the item in use is consumed; the last one uses it up (number 0x56).
pub fn consume_charge(g: &mut Ctx, p: Player) {
    cov!();
    let left = p.u8(g, field::ITEM_CHARGES).wrapping_sub(1);
    p.set_u8(g, field::ITEM_CHARGES, left);
    if left == 0 {
        p.set_u8(g, field::ITEM, 0x56);
    }
}

/// Dust at the feet: effect 0xb10c8 with speeds read from a two-word-per-class table.
fn dust(g: &mut Ctx, p: Player, table: u32) {
    let class = (p.u8(g, Player::CHARACTER) >> 1) as u32;
    let vx = g.prog16(table + 4 * class) as u32;
    let vy = g.prog16(table + 2 + 4 * class) as u32;
    // the speeds stay in D2 and D3 (leftovers of the original)
    (g.leftovers.d2, g.leftovers.d3) = (vx, vy);
    g.shock(p, 0, 0x101, vx as u16, vy as u16);
}

/// 0x3fa7c: dust of braking or walking (one frame out of four).
pub fn dust_stand(g: &mut Ctx, p: Player) {
    cov!();
    let phase = p.u8(g, Player::CHARACTER).wrapping_add(ram::FRAME_COUNTER.get(g.b)) & 3;
    if phase == 0 {
        dust(g, p, tables::DUST_STAND);
    }
}

/// 0x3fac4: dust of the charge, as long as the horizontal speed exceeds 2 pixels per frame.
pub fn dust_run(g: &mut Ctx, p: Player) {
    cov!();
    let phase = p.u8(g, Player::CHARACTER).wrapping_add(ram::FRAME_COUNTER.get(g.b)) & 3;
    if phase != 0 {
        return;
    }
    let speed = (p.u16(g, 0x14) as i16).unsigned_abs();
    if speed > 0x200 {
        dust(g, p, tables::DUST_RUN);
    }
}

/// 0x3fb1a: dust of the jump, every other frame (according to the player number).
pub fn dust_jump(g: &mut Ctx, p: Player) {
    cov!();
    let phase = p.u8(g, Player::INDEX).wrapping_add(ram::FRAME_COUNTER.get(g.b)) & 1;
    if phase == 0 {
        dust(g, p, tables::DUST_JUMP);
    }
}

/// 0x3fb8c: spark of the used item: effect 0xb10c8 described by row `kind` of the table
/// 0x3fbb8, with a sound. No effect if the pool is full.
pub fn item_spark(g: &mut Ctx, p: Player, kind: u16) {
    cov!();
    let row = tables::SPARKS + 8 * kind as u32;
    let (d0, d1, d2, height) = (g.prog16(row), g.prog16(row + 2), g.prog16(row + 4), g.prog16(row + 6));
    // the speed and the height stay in D2-D4 (leftovers of the original)
    crate::mem::set_low_word(&mut g.leftovers.d3, 0xffff);
    g.leftovers.d4 = height as u32;
    g.leftovers.d2 = d2 as u32;
    match g.shock(p, d0 as u8, d1, d2, 0xffff) {
        Some(spark) => wr16(g.b, spark.0 + 0x0c, height),
        // Original glitch: only the carry is tested; in the scene without shocks (41: no effect, carry clear) the
        // height is written through the A1 left by the code before the action, and the sound is played. That A1 is
        // not known here (no scenario plays it): the write is left out (`docs/code-mort.md`, "Sortie du Cpu,
        // partie 6").
        None if crate::ram::SCENE.get(g.b) == NO_SHOCK_SCENE => {}
        None => return,
    }
    g.sound(rom::SPARK_SOUND, p);
}

// ---- healing (cleric, fighter's healing potion) -----------------------------------------------

/// Hit points restored by the healing spell according to the level (`WEAPON` 0x13: potion), ROM words.
const HEAL_TABLE: u32 = 0x416cc;
/// Colors of the healing sparkle: three 16-byte blocks copied into the player's palette.
const HEAL_FLICKER: u32 = 0x41792;
/// Address of the palette modified by the sparkle.
const FLICKER_PALETTE: u32 = 0x91_c1f0;

/// 0x41586: looks for an ally to heal within range in front of the caster (`find_ally`, 80 pixels). The first
/// one found receives the healing pose; without an ally the caster heals themselves. `field::AREA` = 1 if it is
/// an ally, and each player to heal carries the flag `0x78` (reset to zero for those examined).
pub fn heal_scan(g: &mut Ctx, p: Player) {
    cov!();
    let ally = find_ally(g, p, 0x50, |g, q| q.set_u8(g, 0x78, 0));
    if let Some(q) = ally {
        q.set_u8(g, 0x78, 1);
        if p.u8(g, field::SPELL_CANCELLED) == 0 {
            q.set_state(g, 0x0200_1c00);
            q.set_u8(g, field::SPELL_FROZEN, 0);
            face_toward(g, p, q);
        }
    }
    p.set_u8(g, field::AREA, 1);
    let flags: u8 = Player::all().map(|q| q.u8(g, 0x78)).fold(0, u8::wrapping_add);
    if flags == 0 {
        p.set_u8(g, field::AREA, 0);
        p.set_u8(g, 0x78, 1);
    }
}

/// Player `q` is in front of `p` within `reach` pixels (depth: 12 pixels on either side).
pub fn within_reach(g: &mut Ctx, p: Player, q: Player, reach: u16) -> bool {
    cov!();
    let depth = p.u16(g, Obj::Y).wrapping_sub(q.u16(g, Obj::Y)).wrapping_add(0xc);
    if depth > 0x18 {
        return false;
    }
    let mut dx = p.u16(g, Obj::X).wrapping_sub(q.u16(g, Obj::X)) as i16;
    if p.u8(g, Player::FACING) != 0 {
        dx = dx.wrapping_neg();
    }
    dx >= 0 && dx as u16 <= reach
}

/// The target `q` turns toward `p` (it looks toward the side of the smaller abscissa of `p`).
fn face_toward(g: &mut Ctx, p: Player, q: Player) {
    q.set_u8(g, Player::FACING, 0);
    if p.u16(g, Obj::X) > q.u16(g, Obj::X) {
        q.set_u8(g, Player::FACING, 0x20);
    }
}

/// 0x4175c: every 8 frames, the caster's palette takes one of the three healing color blocks.
pub fn heal_flicker(g: &mut Ctx, p: Player) {
    cov!();
    if ram::VBLANK_FRAMES_LOW.get(g.b) & 7 != 0 {
        return;
    }
    let mut n = p.u8(g, 0x1ed) + 1;
    if n == 3 {
        n = 0;
    }
    p.set_u8(g, 0x1ed, n);
    for i in 0..4 {
        let v = g.data32(HEAL_FLICKER + 16 * n as u32 + 4 * i);
        crate::mem::wr32(g.b, FLICKER_PALETTE + 4 * i, v);
    }
}

/// 0x41656: restores hit points to the caster or the allies marked by `heal_scan`, up to
/// the maximum. Spells 0x2c and 0x2d restore twice the level, the others the value from the table.
/// Quirk of the original: after each healing the amount register receives the sum obtained, so that
/// each following ally is healed by as many hit points as the previous one had (before capping).
pub fn apply_heal(g: &mut Ctx, p: Player) {
    cov!();
    let level2 = 2 * p.u8(g, Player::LEVEL) as u16;
    let kind = p.u8(g, field::SPECIAL_KIND);
    let mut amount = if kind == 0x2c || kind == 0x2d { level2 } else { g.data16(HEAL_TABLE + level2 as u32) };
    let mut heal = |g: &mut Ctx, q: Player| {
        let sum = q.u16(g, Player::HP).wrapping_add(amount);
        let max = q.u16(g, Player::HP_MAX);
        q.set_u16(g, Player::HP, sum.min(max));
        amount = sum;
    };
    if p.u8(g, field::AREA) == 0 {
        p.set_u8(g, 0x78, 0);
        heal(g, p);
        return;
    }
    for q in Player::all() {
        if q.u8(g, 0x78) != 0 {
            q.set_u8(g, 0x78, 0);
            heal(g, q);
        }
    }
}

// ---- jump -------------------------------------------------------------------------------------

/// What changes from one character to another in the jump.
pub struct JumpKit {
    /// ROM table (data): four words (vx, ax, vz, az) per jump type `Player::+0x67`.
    pub speeds: u32,
    /// Horizontal push when the player turns the character around in the air.
    pub steer_push: i16,
    /// Strike given in the air with button 1.
    pub air_attack: fn(&mut Ctx, Player),
    /// Called when the recovery is over and the character has returned to the standing state.
    pub after_recover: fn(&mut Ctx, Player),
    /// Called at the end of each jump frame.
    pub finish: fn(&mut Ctx, Player),
    /// Resets specific to the character at takeoff.
    pub takeoff: fn(&mut Ctx, Player),
    /// An aerial gesture is in progress: the rest of the step (rise, peak, descent) is skipped.
    pub busy: fn(&mut Ctx, Player) -> bool,
    /// Gravity (`AZ`) set at the end of the peak: (after a run, otherwise); unchanged if `None`.
    pub apex_gravity: Option<(i16, i16)>,
    /// Steps specific to the character (aerial gestures of steps 0xa, 0xc...).
    pub extra_step: fn(&mut Ctx, Player, u8),
}

impl JumpKit {
    /// Basic jump: no hook.
    pub const PLAIN: JumpKit = JumpKit {
        speeds: 0,
        steer_push: 0x30,
        air_attack: nothing,
        after_recover: nothing,
        finish: nothing,
        takeoff: nothing,
        busy: |_, _| false,
        apex_gravity: None,
        extra_step: |_, _, _| {},
    };
}

/// Does nothing (default variant of the hooks of a `JumpKit`).
pub fn nothing(_: &mut Ctx, _: Player) {}

/// Animation according to the jump type `+0x67`: 2 after a run, 1 forward, otherwise in place.
pub fn by_jump_kind(g: &mut Ctx, p: Player, after_run: u16, forward: u16, standing: u16) -> u16 {
    match p.u8(g, 0x67) {
        2 => after_run,
        1 => forward,
        _ => standing,
    }
}

/// Jump (0x3e578 for the fighter): step 0 waits for the end of the crouch, 2 the rise, 4 the
/// peak, 6 the descent, 8 the recovery. The jump type (`+0x67`, set by `checks::jump_button`)
/// selects the animations: 0 in place, 1 forward, 2 after a run, 3 vertical.
pub fn jump(g: &mut Ctx, p: Player, kit: &JumpKit) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => jump_takeoff(g, p, kit),
        2 => jump_rise(g, p, kit),
        4 => jump_apex(g, p, kit),
        6 => jump_fall(g, p, kit),
        8 => jump_recover(g, p, kit),
        step => (kit.extra_step)(g, p, step),
    }
    (kit.finish)(g, p);
}

/// Step 0: at the end of the crouch the jump starts, with the speed of the jump type.
fn jump_takeoff(g: &mut Ctx, p: Player, kit: &JumpKit) {
    cov!();
    if !animation::step(g, p) {
        return;
    }
    next_step(g, p);
    p.set_u8(g, field::AIR_STEERED, 0);
    p.set_u8(g, field::AIR_ATTACK_DONE, 0);
    (kit.takeoff)(g, p);
    let x = p.u16(g, Obj::X);
    p.set_u16(g, field::TAKEOFF_X, x);
    let anim = by_jump_kind(g, p, 0x15, 0x10, 0xb);
    animation::start(g, p, anim);
    g.sound(ext::sound::JUMP_VOICE, p);
    let kind = p.u8(g, 0x67);
    launch_jump(g, p, kit.speeds, kind);
}

/// Step 2: rise until the vertical speed drops to 0x180.
fn jump_rise(g: &mut Ctx, p: Player, kit: &JumpKit) {
    cov!();
    animation::step(g, p);
    move_z(g, p);
    drift(g, p);
    air_steer(g, p, kit.steer_push);
    (kit.air_attack)(g, p);
    if (kit.busy)(g, p) || p.u16(g, Obj::VZ) > 0x180 {
        return;
    }
    next_step(g, p);
    if p.u8(g, field::AIR_ATTACK_DONE) == 0 {
        let anim = by_jump_kind(g, p, 0x16, 0x11, 0xc);
        animation::start(g, p, anim);
    }
}

/// Step 4: peak of the jump.
fn jump_apex(g: &mut Ctx, p: Player, kit: &JumpKit) {
    cov!();
    move_z(g, p);
    drift(g, p);
    air_steer(g, p, kit.steer_push);
    (kit.air_attack)(g, p);
    if (kit.busy)(g, p) {
        return;
    }
    if animation::step(g, p) {
        next_step(g, p);
        if p.u8(g, field::AIR_ATTACK_DONE) == 0 {
            if let Some((after_run, other)) = kit.apex_gravity {
                let az = if p.u8(g, 0x67) == 2 { after_run } else { other };
                p.set_u16(g, Obj::AZ, az as u16);
            }
            let anim = by_jump_kind(g, p, 0x17, 0x12, 0xd);
            animation::start(g, p, anim);
        }
    }
}

/// Step 6: descent; on landing the recovery starts.
fn jump_fall(g: &mut Ctx, p: Player, kit: &JumpKit) {
    cov!();
    animation::step(g, p);
    move_xz(g, p);
    (kit.air_attack)(g, p);
    if (kit.busy)(g, p) || (p.u16(g, Obj::Z) as i16) > 0 {
        return;
    }
    next_step(g, p);
    land(g, p);
}

/// Landing: on the ground (`Z` = 0), recovery animation and sound.
pub fn land(g: &mut Ctx, p: Player) {
    cov!();
    p.set_u32(g, Obj::Z, 0);
    let anim = by_jump_kind(g, p, 0x18, 0x13, 0xe);
    animation::start(g, p, anim);
    g.sound(ext::sound::LANDING, p);
}

/// Step 8: recovery, then return to the standing state.
fn jump_recover(g: &mut Ctx, p: Player, kit: &JumpKit) {
    cov!();
    if animation::step(g, p) {
        to_stand(g, p);
        (kit.after_recover)(g, p);
    }
}

/// Start of the roll (action 0x12 of the dwarf and the thief): run-up `speed` in the facing direction, without
/// acceleration or vertical speed, animation 0x1f.
pub fn roll_start(g: &mut Ctx, p: Player, speed: u16) {
    cov!();
    let vx = if p.u8(g, Player::FACING) != 0 { speed.wrapping_neg() } else { speed };
    p.set_u16(g, Obj::VX, vx);
    p.set_u32(g, Obj::AX, 0);
    animation::start(g, p, 0x1f);
}

// ---- action 0x10: run-up and slide -------------------------------------------------------------

/// Forward horizontal speed (the long (speed, acceleration) is negated if the character faces
/// left, like `neg.l (0x14,A0)`).
pub fn set_forward_motion(g: &mut Ctx, p: Player, speed: u16, accel: u16) {
    p.set_u16(g, Obj::VX, speed);
    p.set_u16(g, Obj::AX, accel);
    if p.u8(g, Player::FACING) == 0 {
        negate_horizontal_speed(g, p);
    }
}

/// Action 0x10 (0x3f9bc fighter, 0x41f22 cleric): run-up, brief leap, slide. Step 0 starts the run-up, 2
/// waits for the end of the gesture, 4 extends the run-up as long as the jump button is held (eight frames at
/// most), 6 brakes.
pub fn dash(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            set_forward_motion(g, p, 0xb00, 0);
            animation::start(g, p, 0x1c);
            clear_recognizers(g, p);
        }
        2 => {
            if animation::step(g, p) {
                next_step(g, p);
                p.set_u8(g, 0x1e8, 8);
                animation::start(g, p, 0x1d);
                g.sound(ext::sound::SLIDE, p);
            }
        }
        4 => {
            let left = p.u8(g, 0x1e8).wrapping_sub(1);
            p.set_u8(g, 0x1e8, left);
            if left != 0 && p.u8(g, Player::KEYS_HELD) & 0x20 != 0 {
                dust_jump(g, p);
                move_x(g, p);
                return;
            }
            next_step(g, p);
            p.set_u8(g, 0x1e8, 0xc);
            let brake = if p.u8(g, Player::FACING) != 0 { -0xa0i16 } else { 0xa0 };
            p.set_u16(g, Obj::AX, brake as u16);
            dash_slide(g, p);
        }
        6 => dash_slide(g, p),
        _ => {}
    }
}

/// 0x3fa4e: slide braked to a stop, with dust.
fn dash_slide(g: &mut Ctx, p: Player) {
    cov!();
    move_x(g, p);
    animation::step(g, p);
    if momentum_reversed(g, p) {
        to_stand(g, p);
        return;
    }
    let left = p.u8(g, 0x1e8).wrapping_sub(1);
    p.set_u8(g, 0x1e8, left);
    if (left as i8) >= 0 {
        dust_jump(g, p);
    }
}

// ---- common strikes ----------------------------------------------------------------------------

/// Rank in the strike chain: did the previous strike land (bit `hit_bit` of `field::HITS`)?
/// Then the rank advances, and returns to 0 beyond `last`. Otherwise the rank restarts from 0.
pub fn chain_rank(g: &mut Ctx, p: Player, hit_bit: u8, last: u8) -> u8 {
    cov!();
    let mut rank = 0;
    if p.u8(g, field::HITS) & hit_bit != 0 {
        rank = p.u8(g, field::COMBO_RANK) + 1;
        if rank > last {
            rank = 0;
        }
    }
    p.set_u8(g, field::COMBO_RANK, rank);
    rank
}

/// The chain window decreases; if button 1 is released during this time, the chain
/// is abandoned (`COMBO_RELEASED`).
pub fn combo_window(g: &mut Ctx, p: Player) {
    cov!();
    let window = p.u8(g, field::COMBO_WINDOW);
    if window != 0 {
        p.set_u8(g, field::COMBO_WINDOW, window - 1);
        if p.u8(g, Player::KEYS_HELD) & 0x10 == 0 {
            p.set_u8(g, field::COMBO_RELEASED, 1);
        }
    }
}

/// Second step of a basic strike (0x3e8de, 0x3ffea): if button 1 stays pressed during the window and
/// the strike landed, the gesture `follow_anim` extends the strike; otherwise return to the standing state.
/// Returns true if the strike is extended.
pub fn combo_follow(g: &mut Ctx, p: Player, follow_anim: u16) -> bool {
    cov!();
    combo_window(g, p);
    if !animation::step(g, p) {
        return false;
    }
    if p.u8(g, field::COMBO_RELEASED) == 0 && p.u8(g, field::HITS) != 0 {
        next_step(g, p);
        animation::start_plain(g, p, follow_anim);
        true
    } else {
        to_stand(g, p);
        false
    }
}

/// A single-gesture strike: animation `anim` with the weapon voice, then return to the standing state.
pub fn single_strike(g: &mut Ctx, p: Player, anim: u16) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, field::MOVE_STATE, 0);
            start_with_voice(g, p, anim);
        }
        2 => wait_then_stand(g, p),
        _ => {}
    }
}

/// Backward jump (0x3ece2, 0x4033a): small jump that moves away from the opponent. `animate`: the
/// animation script advances on every frame (cleric) or not (fighter).
pub fn back_hop(g: &mut Ctx, p: Player, anim: u16, animate: bool) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, field::MOVE_STATE, 0);
            p.set_u16(g, Obj::VX, 0x100);
            p.set_u16(g, Obj::AX, 0);
            p.set_u16(g, Obj::VZ, 0x300);
            p.set_u16(g, Obj::AZ, (-0x38i16) as u16);
            if p.u8(g, Player::FACING) != 0 {
                let v = (p.u16(g, Obj::VX) as i16).wrapping_neg();
                p.set_u16(g, Obj::VX, v as u16);
            }
            animation::start_plain(g, p, anim);
        }
        2 => {
            if animate {
                animation::step(g, p);
            }
            move_xz(g, p);
            if (p.u16(g, Obj::VZ) as i16) < 0 && (p.u16(g, Obj::Z) as i16) < 0 {
                p.set_u32(g, Obj::Z, 0);
                to_stand(g, p);
            }
        }
        _ => {}
    }
}

/// A forward gesture (0x3e824, 0x3ff56): animation `anim` then return to the standing state.
pub fn pose(g: &mut Ctx, p: Player, anim: u16) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, field::MOVE_STATE, 0);
            animation::start_plain(g, p, anim);
        }
        2 => wait_then_stand(g, p),
        _ => {}
    }
}

/// Step 2 of the strike given while running (0x3ec84, 0x402d8): the momentum runs out (animation `stop_anim`) or the
/// character keeps sliding with dust.
/// Returns true as long as the character slides.
pub fn run_strike_slide(g: &mut Ctx, p: Player, stop_anim: u16) -> bool {
    cov!();
    animation::step(g, p);
    if momentum_reversed(g, p) {
        next_step(g, p);
        p.set_u32(g, Obj::VX, 0);
        animation::start_plain(g, p, stop_anim);
        return false;
    }
    dust_run(g, p);
    move_x(g, p);
    true
}

/// Step 4 of the charge (0x3ebbc, 0x4023a): the landed strikes create effects; the slide slows
/// then stops.
pub fn charge_slide(g: &mut Ctx, p: Player, events: fn(&mut Ctx, Player)) {
    cov!();
    events(g, p);
    move_x(g, p);
    dust_stand(g, p);
    if animation::step(g, p) {
        p.set_u8(g, Player::SLOW, 0);
        to_stand(g, p);
        return;
    }
    if momentum_reversed(g, p) {
        p.set_u32(g, Obj::VX, 0);
    }
}

// ---- allies in range ----------------------------------------------------------------------------

/// First ally that the caster `p` can heal or bless: another active player, on the ground, alive,
/// with no held object or grab, in front of `p` within `reach` pixels. `visit` is called on each
/// player examined before the test (the search stops at the first ally found).
pub fn find_ally(g: &mut Ctx, p: Player, reach: u16, mut visit: impl FnMut(&mut Ctx, Player)) -> Option<Player> {
    cov!();
    for q in Player::all() {
        visit(g, q);
        if q == p || q.u8(g, Player::ACTIVE) & 1 == 0 || q.u16(g, Obj::Z) != 0 {
            continue;
        }
        if q.u8(g, Player::POWER_10F) != 0 || q.u16(g, Player::HP) == 0 || q.u8(g, Player::SLOW_8A) != 0 {
            continue;
        }
        if q.u8(g, Player::ACTION) == 0x22 || !within_reach(g, p, q, reach) {
            continue;
        }
        return Some(q);
    }
    None
}

// ---- spell skeleton ------------------------------------------------------------------------

/// The carried gesture (`field::EVENT` non-zero) has just been signaled: it is consumed.
pub fn take_event(g: &mut Ctx, p: Player) -> bool {
    cov!();
    if p.u8(g, field::EVENT) == 0 {
        return false;
    }
    p.set_u8(g, field::EVENT, 0);
    true
}

/// The spell is cancelled by a protection (`field::SPELL_CANCELLED`).
pub fn is_cancelled(g: &mut Ctx, p: Player) -> bool {
    p.u8(g, field::SPELL_CANCELLED) != 0
}

/// One frame of countdown: `field::TIMER` decreases; true when it reaches zero.
pub fn countdown(g: &mut Ctx, p: Player) -> bool {
    cov!();
    let left = p.u16(g, field::TIMER).wrapping_sub(1);
    p.set_u16(g, field::TIMER, left);
    left == 0
}

/// Start of a spell: next step, pause, spell indicator, duration, one charge of the reserve
/// (`reserve`: byte of `+0x151..+0x157`), orientation, freezing of the other players.
pub fn begin_cast(g: &mut Ctx, p: Player, timer: Option<u16>, reserve: Option<u32>) {
    cov!();
    begin_spell(g, p, timer);
    if let Some(reserve) = reserve {
        let left = p.u8(g, reserve).wrapping_sub(1);
        p.set_u8(g, reserve, left);
    }
    face_held_direction(g, p);
    freeze_others(g, p);
}

/// Complete start of a spell: `begin_cast`, darkened screen, gesture `anim`.
pub fn start_cast(g: &mut Ctx, p: Player, timer: u16, reserve: Option<u32>, anim: u16) {
    cov!();
    begin_cast(g, p, Some(timer), reserve);
    dim_screen(g, p);
    animation::start_plain(g, p, anim);
}

/// Cancellation of the spell (a protection): the spark `spark` is played and the countdown goes to 16 frames.
pub fn cancel_cast(g: &mut Ctx, p: Player, spark: u16) {
    cov!();
    p.set_u16(g, field::TIMER, 0x10);
    item_spark(g, p, spark);
}

/// 0x43864 / 0x46f00: the screen changes tint (D0 = 0xc000 or 0x2000, one phase out of two) as long as the
/// previous gradient is finished.
pub fn flash_screen(g: &mut Ctx, p: Player) {
    cov!();
    if ram::FADE_STEPS.get(g.b) != 0 {
        return;
    }
    ram::FADE_PAGES.set(g.b, 0xc);
    let phase = (p.u8(g, 0x1eb) + 1) & 3;
    p.set_u8(g, 0x1eb, phase);
    let color: u16 = if phase & 2 != 0 { 0x2000 } else { 0xc000 };
    crate::palette::set_brightness(g.b, color);
}

/// Intensity setting of the freeze spells (up and down during the final phase): after a pause of
/// `+0x1e8` frames, the up key (bit 3) decreases `+0x1e9`, the down key (bit 2) increases it (from
/// 0 to 4), and the gesture `base + intensity` is played; otherwise the animation advances.
pub fn intensity_keys(g: &mut Ctx, p: Player, base: u16) {
    cov!();
    let wait = p.u8(g, 0x1e8);
    if wait != 0 {
        p.set_u8(g, 0x1e8, wait - 1);
        animation::step(g, p);
        return;
    }
    let keys = p.u8(g, Player::KEYS_HELD);
    let forward = if p.u8(g, Player::FACING) != 0 { 1 } else { 0 };
    let level = p.u8(g, 0x1e9);
    if keys >> forward & 1 == 0 {
        let next = if keys & 8 != 0 {
            level.checked_sub(1)
        } else if keys & 4 != 0 {
            (level != 4).then_some(level + 1)
        } else {
            None
        };
        if let Some(next) = next {
            p.set_u8(g, 0x1e8, 0x10);
            p.set_u8(g, 0x1e9, next);
            animation::start_plain(g, p, base + next as u16);
            return;
        }
    }
    animation::step(g, p);
}

/// The four screen flashes of the freeze spell: `+0x4ba` is 0x100 three steps out of four.
pub fn blink_screen(g: &mut Ctx, p: Player) {
    cov!();
    ram::SCROLL1_Y.set(g.b, 0);
    let blinks = p.u8(g, 0x1ea);
    if blinks != 0 {
        ram::SCROLL1_Y.set(g.b, 0x100);
        p.set_u8(g, 0x1ea, blinks - 1);
    }
}

/// Final step of the spells that let the animation finish: the countdown decreases; at zero the spell ends
/// (screen restored with `undim_mode`), otherwise the animation advances.
pub fn wind_down(g: &mut Ctx, p: Player, undim_mode: u16) {
    cov!();
    if countdown(g, p) {
        end_spell_mode(g, p, undim_mode);
    } else {
        animation::step(g, p);
    }
}

/// Simple equipment swap (elf, magic-user): the picked-up item (`field::ITEM_NEW`) takes the place
/// of weapon `slot` (`WEAPON_A` or `WEAPON_B`) and the old weapon that of the list item. With
/// `with_effect`, gesture 1 does the swap and gesture 2 creates the swap effect (0x4fd2c); otherwise any
/// gesture does the swap.
pub fn swap_weapon(g: &mut Ctx, p: Player, slot: u32, with_effect: bool) {
    cov!();
    if p.u8(g, Player::STEP) == 0 {
        next_step(g, p);
        animation::start(g, p, 0x32);
        return;
    }
    if animation::step(g, p) {
        to_stand(g, p);
        return;
    }
    let event = p.u8(g, field::EVENT);
    if event == 0 || (with_effect && event > 2) {
        return;
    }
    p.set_u8(g, field::EVENT, 0);
    if with_effect && event == 2 {
        spawn::swap(g, p, 0);
        return;
    }
    let new = p.u8(g, field::ITEM_NEW).wrapping_sub(0x2e);
    let held = p.u8(g, slot);
    p.set_u8(g, field::ITEM, held.wrapping_add(0x2e));
    p.set_u8(g, slot, new);
    init_weapon_stats(g, p);
}
