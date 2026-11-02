//! Common character code: strikes, allies in range and the spell skeleton.

use super::*;

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
