//! Common character code: healing, the jump skeleton and the run-up (action 0x10).

use crate::world::objects::Obj;
use crate::actors::player::Player;
use super::*;
use crate::actors::player::tables::{players, Aligned};

// ---- healing (cleric, fighter's healing potion) -----------------------------------------------

/// Address of the palette modified by the sparkle.
pub(super) const FLICKER_PALETTE: u32 = 0x91_c1f0;

/// 0x41586: looks for an ally to heal within range in front of the caster (`find_ally`, 80 pixels). The first
/// one found receives the healing pose; without an ally the caster heals themselves. `field::AREA` = 1 if it is
/// an ally, and each player to heal carries the flag `0x78` (reset to zero for those examined).
pub fn heal_scan(g: &mut Ctx, p: Player) {
    cov!();
    let ally = find_ally(g, p, 0x50, |g, q| q.set_u8(g, Player::ALLY_MARK, 0));
    if let Some(q) = ally {
        q.set_u8(g, Player::ALLY_MARK, 1);
        if p.u8(g, field::SPELL_CANCELLED) == 0 {
            q.set_state(g, 0x0200_1c00);
            q.set_u8(g, field::SPELL_FROZEN, 0);
            face_toward(g, p, q);
        }
    }
    p.set_u8(g, field::AREA, 1);
    let flags: u8 = Player::all().map(|q| q.u8(g, Player::ALLY_MARK)).fold(0, u8::wrapping_add);
    if flags == 0 {
        p.set_u8(g, field::AREA, 0);
        p.set_u8(g, Player::ALLY_MARK, 1);
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
pub(super) fn face_toward(g: &mut Ctx, p: Player, q: Player) {
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
    let mut n = p.u8(g, Player::SPELL_REPEAT) + 1;
    if n == 3 {
        n = 0;
    }
    p.set_u8(g, Player::SPELL_REPEAT, n);
    for i in 0..4 {
        let v = players("heal_flicker").long_at(g.b, 16 * n as u32 + 4 * i);
        crate::world::mem::wr32(g.b, FLICKER_PALETTE + 4 * i, v);
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
    let mut amount = if kind == 0x2c || kind == 0x2d { level2 } else { players("heal_amounts").word_at(g.b, level2 as u32) };
    let mut heal = |g: &mut Ctx, q: Player| {
        let sum = q.u16(g, Player::HP).wrapping_add(amount);
        let max = q.u16(g, Player::HP_MAX);
        q.set_u16(g, Player::HP, sum.min(max));
        amount = sum;
    };
    if p.u8(g, field::AREA) == 0 {
        p.set_u8(g, Player::ALLY_MARK, 0);
        heal(g, p);
        return;
    }
    for q in Player::all() {
        if q.u8(g, Player::ALLY_MARK) != 0 {
            q.set_u8(g, Player::ALLY_MARK, 0);
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
    match p.u8(g, Player::JUMP_KIND) {
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
pub(super) fn jump_takeoff(g: &mut Ctx, p: Player, kit: &JumpKit) {
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
    g.sound(crate::hw::sound::JUMP_VOICE, p);
    let kind = p.u8(g, Player::JUMP_KIND);
    launch_jump(g, p, kit.speeds, kind);
}

/// Step 2: rise until the vertical speed drops to 0x180.
pub(super) fn jump_rise(g: &mut Ctx, p: Player, kit: &JumpKit) {
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
pub(super) fn jump_apex(g: &mut Ctx, p: Player, kit: &JumpKit) {
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
                let az = if p.u8(g, Player::JUMP_KIND) == 2 { after_run } else { other };
                p.set_u16(g, Obj::AZ, az as u16);
            }
            let anim = by_jump_kind(g, p, 0x17, 0x12, 0xd);
            animation::start(g, p, anim);
        }
    }
}

/// Step 6: descent; on landing the recovery starts.
pub(super) fn jump_fall(g: &mut Ctx, p: Player, kit: &JumpKit) {
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
pub(super) fn jump_recover(g: &mut Ctx, p: Player, kit: &JumpKit) {
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
                p.set_u8(g, Player::ACTION_COUNT, 8);
                animation::start(g, p, 0x1d);
                g.sound(ext::sound::SLIDE, p);
            }
        }
        4 => {
            let left = p.u8(g, Player::ACTION_COUNT).wrapping_sub(1);
            p.set_u8(g, Player::ACTION_COUNT, left);
            if left != 0 && p.u8(g, Player::KEYS_HELD) & 0x20 != 0 {
                dust_jump(g, p);
                move_x(g, p);
                return;
            }
            next_step(g, p);
            p.set_u8(g, Player::ACTION_COUNT, 0xc);
            let brake = if p.u8(g, Player::FACING) != 0 { -0xa0i16 } else { 0xa0 };
            p.set_u16(g, Obj::AX, brake as u16);
            dash_slide(g, p);
        }
        6 => dash_slide(g, p),
        _ => {}
    }
}

/// 0x3fa4e: slide braked to a stop, with dust.
pub(super) fn dash_slide(g: &mut Ctx, p: Player) {
    cov!();
    move_x(g, p);
    animation::step(g, p);
    if momentum_reversed(g, p) {
        to_stand(g, p);
        return;
    }
    let left = p.u8(g, Player::ACTION_COUNT).wrapping_sub(1);
    p.set_u8(g, Player::ACTION_COUNT, left);
    if (left as i8) >= 0 {
        dust_jump(g, p);
    }
}
