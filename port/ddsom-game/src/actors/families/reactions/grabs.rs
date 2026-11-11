//! Grabs by a player and special reactions: seized, grab hits, quiver, roll, zap, petrification, stagger, burn.

use super::*;

// Grabs and special reactions

/// 0x3d00a: grabbed by a player (animation 14), total immunities.
pub(super) fn seized<R: Bench>(x: &mut Cx<'_, R>) {
    x.set32(E::IMMUNITY, u32::MAX);
    apply_pending(x);
    next(x);
    x.set8(E::VULNERABLE, 0);
    x.set8(E::SUB_STEP, 0);
    anim(x, 14);
}
/// 0x3d02a: back to waiting for the grab.
pub(super) fn seized_again<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(E::SUB_STEP, 2);
    anim(x, 14);
}
/// 0x3d036: the grab (`+7`): sticks to the player, then takes their hits.
pub(super) fn seized_by<R: Bench>(x: &mut Cx<'_, R>) {
    let s = x.u8(E::SUB_STEP);
    match s {
        0 => seized_stick(x),
        2 => seized_hits(x),
        4 => {
            let c = x.u8(E::COUNTER).wrapping_sub(1);
            x.set8(E::COUNTER, c);
            if c == 0 {
                return seized_again(x);
            }
            shake(x, 2);
        }
        _ => {}
    }
}
/// 0x3d048: when the player (`+0x50`) signals the hold, the object places itself in front of them.
pub(super) fn seized_stick<R: Bench>(x: &mut Cx<'_, R>) {
    let p = E(x.linked(E::TARGET));
    if p.key_frame(x.b) != 1 {
        return;
    }
    next_sub(x);
    let px = p.x(x.b);
    x.set16(E::X, px);
    let i = 4 * x.u8(E::KIND) as u32;
    let mut d = tables::seized_offsets().at16(x.b, i);
    if !flipped(x) {
        d = d.wrapping_neg();
    }
    x.add16(E::X, d);
}
/// 0x3d0e8: the player (character 5, state 0x02000cxx, action 26) strikes: damage according to their hit (`+0x10c`,
/// table 0x3d0da), points (0x19dce), effect (0xb141a); otherwise end of the grab.
pub(super) fn seized_hits<R: Bench>(x: &mut Cx<'_, R>) {
    let p = x.linked(E::TARGET);
    let character = p.u8(x.b, Player::CHARACTER) >> 1;
    if character != 5 || p.u32(x.b, Player::MAIN) & 0xffff_ff00 != 0x0200_0c00 || p.u8(x.b, Player::SUBACTION) != 26 {
        return next_clear(x);
    }
    if (E(p).key_frame(x.b) as i8) >= 0 {
        return;
    }
    let level = (p.u8(x.b, Player::LEVEL).wrapping_sub(11)) as u32;
    let damage = tables::seized_damage().at8(x.b, level) as u16;
    next_sub(x);
    E(p).set_key_frame(x.b, 0);
    let hp = x.u16(E::HP).wrapping_sub(damage);
    x.set16(E::HP, if (hp as i16) > 0 { hp } else { 0 });
    x.set8(E::COUNTER, 7);
    anim_step(x);
    // 0x19dce: health bar of the grabbed object, in the player's name
    let owner = x.linked(E::TARGET);
    let header = ((owner.u8(x.b, Player::INDEX) as u16) << 8) | x.u8(E::KIND) as u16;
    let popup = crate::world::events::Popup { header, max: x.u16(E::HP_MAX), value: x.u16(E::HP), raw: x.u8(E::CLASS) == 2 };
    crate::world::events::create_popup(x.b, popup);
    let i = 4 * x.u8(E::KIND) as u32;
    let speeds = tables::seized_hit_speeds();
    let mut vx = speeds.at16(x.b, i);
    let ax = speeds.at16(x.b, i + 2);
    if flipped(x) {
        vx = vx.wrapping_neg();
    }
    // leftovers: the spark's speeds in D2.w and D3.w (`dispatch::Leftovers`)
    *x.c.d2() = *x.c.d2() & !0xffff | vx as u32;
    *x.c.d3() = *x.c.d3() & !0xffff | ax as u32;
    crate::actors::families::effects_creators::impact_spark(x.b, x.o, vx, ax);
}
/// 0x3d224: released: predefined throw 1, the type's immunities (0xfe5a).
pub(super) fn released<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(E::ACTIVE, 1);
    anim(x, 3);
    hurt(x);
    launch(x, 1);
    x.set8(E::FREEZE, 1);
    if flipped(x) {
        neg32(x, E::VX);
    }
    lib::set_immunities(x);
}

/// 0x3d26a: hit received by a grabbed object (`+0x87`) or not; depending on the height, recoil (code 0) or low blow
/// (56).
pub(super) fn grab_hit<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(E::GRABBED) == 0 {
        if x.u16(E::HEIGHT) as i16 > 0 {
            x.set8(E::REACTION, 56);
            return run_entry(x, 0x3c2ee);
        }
        x.set8(E::REACTION, 0);
        return run_entry(x, 0x3bb9c);
    }
    if dead(x) {
        mark_dead(x);
        if x.u16(E::HEIGHT) as i16 > 0 {
            x.set8(E::REACTION, 56);
            return run_entry(x, 0x3c2ee);
        }
        x.set8(E::REACTION, 0);
        return run_entry(x, 0x3dd72);
    }
    x.set8(E::VULNERABLE, 0);
    x.set8(E::COUNTER, 16);
    apply_pending(x);
    if x.u8(E::DOWNED) != 0 {
        x.set8(E::REACTION, 108);
        x.set8(E::VULNERABLE, 0);
        x.set8(E::COUNTER, 16);
        return run_entry(x, 0x3d350);
    }
    if x.u16(E::HEIGHT) as i16 <= 0 {
        x.set8(E::VULNERABLE, 0);
        x.set32(E::HEIGHT, 0);
        x.set8(E::STEP, 10);
        anim(x, 15);
        effect(x, 3, 0);
        return;
    }
    anim(x, 2);
    let flip = x.u8(E::FLIP) ^ 0x20;
    x.set8(E::ATTACKER_FLIP, flip);
    launch(x, 0);
    face_attacker(x);
}
/// 0x3d318: on the ground, grabbed: `+0x87` wears off, then get-up through the slide.
pub(super) fn grab_down<R: Bench>(x: &mut Cx<'_, R>) {
    let c = x.u8(E::COUNTER);
    if c != 0 {
        x.set8(E::COUNTER, c - 1);
        if c == 1 {
            x.set32(E::HIT_LOCK, 0);
            x.set8(E::ACTIVE, 1);
        }
    }
    let g = x.u8(E::GRABBED).wrapping_sub(1);
    x.set8(E::GRABBED, g);
    if g == 0 {
        return slide_end(x);
    }
    anim_step(x);
}
/// 0x3d350 (code 108): on the ground and grabbed; death through 0x3c63a.
pub(super) fn grounded_grab<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if dead(x) {
        x.set8(E::SUB, 6);
        x.set8(E::REACTION, 76);
        x.set8(E::ACTIVE, 2);
        return run_entry_flag(x, 0x3c63a);
    }
    let g = x.u8(E::GRABBED).wrapping_sub(1);
    x.set8(E::GRABBED, g);
    if g == 0 {
        get_up(x);
        return true;
    }
    let c = x.u8(E::COUNTER);
    if c != 0 {
        x.set8(E::COUNTER, c - 1);
        if c == 1 {
            x.set32(E::HIT_LOCK, 0);
            x.set8(E::ACTIVE, 1);
        }
    }
    if x.u8(E::STEP) != 0 {
        return true;
    }
    next(x);
    anim(x, 16);
    effect(x, 3, 1);
    true
}

/// 0x3d3c8: shakes in place (stars 0xabec8(1)), randomly drawn delay (table 0x3d42a).
pub(super) fn quiver<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(E::VULNERABLE, 0);
    x.set8(E::DOWNED, 0);
    apply_pending(x);
    x.set8(E::ACTIVE, 2);
    effect(x, 1, 0);
    anim(x, 3);
    x.set8(E::DOWNED, 0);
    next(x);
    x.set8(E::COUNTER, 0);
    x.set32(E::VX, 0x0180_0000);
    x.set16(E::VZ, 0x400);
    x.set16(E::GRAVITY, -64i16 as u16);
    x.set32(E::VY, 0);
    quiver_delay(x);
    orient_speeds(x);
    stun(x);
    hurt(x);
}
pub(super) fn quiver_delay<R: Bench>(x: &mut Cx<'_, R>) {
    let r = (x.random() as u32) & 3;
    let d = tables::quiver_delays().at8(x.b, r);
    x.set8(E::COUNTER, d);
}
/// 0x3d42e: the shaking; at each delay, stars one time out of four... three out of four.
pub(super) fn quiver_shake<R: Bench>(x: &mut Cx<'_, R>) {
    let c = x.u8(E::COUNTER).wrapping_sub(1);
    x.set8(E::COUNTER, c);
    if (c as i8) <= 0 {
        let r = (x.random() as u32) & 3;
        if r != 0 {
            effect(x, 1, 1);
        }
        quiver_delay(x);
    }
    shake(x, 1);
    if !paused(x) {
        next(x);
    }
}

/// 0x3d4a6 / 0x3d522: low somersault (X 0x100, height 0x800); the second has animation 13 and its stars.
pub(super) fn roll<R: Bench>(x: &mut Cx<'_, R>, stars: bool) {
    x.set8(E::VULNERABLE, 0);
    next(x);
    x.set8(E::GRABBED, 0);
    if stars {
        x.set8(E::DOWNED, 0);
    }
    x.set8(E::FREEZE, 1);
    x.set8(E::COUNTER, 1);
    speeds(x, 0x100, 0, 0x800, -64i16 as u16);
    x.set16(E::AY, 0);
    x.set16(E::VY, 64);
    orient_speeds(x);
    if stars {
        anim(x, 13);
        effect(x, 7, 0);
        return hurt_active(x);
    }
    // 0x3d4e6: death, both sounds; then the somersault animation
    x.set8(E::ACTIVE, 2);
    if dead(x) {
        x.set8(E::SUB, 6);
        death_sound(x);
    }
    hit_sound(x);
    tumble_anim(x);
}
pub(super) fn roll_plain<R: Bench>(x: &mut Cx<'_, R>) {
    roll(x, false);
}
pub(super) fn roll_stars<R: Bench>(x: &mut Cx<'_, R>) {
    roll(x, true);
}

/// 0x3d5a2: electrocuted (80 frames, animation 0), it dies.
pub(super) fn zapped<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(E::VULNERABLE, 0);
    apply_pending(x);
    next(x);
    x.set8(E::ACTIVE, 2);
    x.set8(E::SUB, 6);
    x.set8(E::COUNTER, 80);
    anim(x, 0);
}
/// Byte of the shock effect (EFFCT, 0xb10c8) set by the burn: the spark is drawn 16 pixels higher (h).
const SHOCK_RAISED: u32 = 0x43;

/// 0x3d5c6: sparks every 8 frames (0xb10c8), then disappearance.
pub(super) fn zapped_burn<R: Bench>(x: &mut Cx<'_, R>) {
    let c = x.u8(E::COUNTER).wrapping_sub(1);
    x.set8(E::COUNTER, c);
    if c == 0 {
        crate::actors::families::effects_creators::toss(x.b, x.o);
        x.set32(E::STATE, 0x0600_0000);
        detach_from_host(x);
        return lib::object_event(x);
    }
    if c & 7 != 0 {
        return;
    }
    // Original glitch: in scene 41, 0xb10c8 cedes control with no effect or carry, and the rest writes into the A1
    // left over, the reaction routine itself (ROM): the writes are lost.
    let lib::Impact::Created(o) = lib::shock(x, 7, 0x200, 0, 0xffff) else { return };
    o.set_u8(x.b, SHOCK_RAISED, 1);
    let h = o.u16(x.b, Obj::Z).wrapping_add(16);
    o.set_u16(x.b, Obj::Z, h);
}

/// 0x3d62c: petrified (animation 20); in the air, it falls first.
pub(super) fn petrified<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(E::VULNERABLE, 0);
    apply_pending(x);
    next(x);
    x.set8(E::SUB, 6);
    x.set8(E::ACTIVE, 2);
    defer_link(x);
    anim(x, 20);
    if x.u16(E::HEIGHT) as i16 <= 0 {
        next(x);
        x.set8(E::COUNTER, 40);
        return lib::sound(x, crate::services::sound::CMD_31F_41F);
    }
    x.set16(E::VZ, 0);
    x.set16(E::GRAVITY, -64i16 as u16);
    x.set32(E::VX, 0);
}
pub(super) fn petrified_fall<R: Bench>(x: &mut Cx<'_, R>) {
    move_xz(x);
    if x.u16(E::HEIGHT) as i16 > 0 {
        return;
    }
    x.set32(E::HEIGHT, 0);
    next(x);
    x.set8(E::COUNTER, 1);
    landed(x);
}
pub(super) fn petrified_end<R: Bench>(x: &mut Cx<'_, R>) {
    if paused(x) {
        return;
    }
    lib::debris_spray(x, 5);
    x.set32(E::STATE, 0x0600_0000);
    detach_from_host(x);
    lib::object_event(x);
}

/// 0x3d762 (code 0x5c...): in the air, throw (code 4); death on the ground; otherwise 60 frames of shaking then
/// glissade.
pub(super) fn stagger<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(E::VULNERABLE, 0);
    if x.u16(E::HEIGHT) as i16 > 0 {
        x.set8(E::REACTION, 4);
        return run_entry(x, 0x3bc24);
    }
    if dead(x) {
        mark_dead(x);
        return run_entry(x, 0x3dd72);
    }
    next(x);
    x.set8(E::COUNTER, 60);
    stun(x);
    x.set16(E::VX, 0x180);
    x.set16(E::AX, -56i16 as u16);
    face_attacker(x);
    anim(x, 0);
}
pub(super) fn stagger_shake<R: Bench>(x: &mut Cx<'_, R>) {
    let c = x.u8(E::COUNTER).wrapping_sub(1);
    x.set8(E::COUNTER, c);
    if c == 40 {
        x.set32(E::HIT_LOCK, 0);
    }
    shake(x, 2);
    if x.u8(E::COUNTER) == 0 {
        next(x);
    }
}

/// 0x3d828: small throw (animation 3), oriented speeds.
pub(super) fn hop_back<R: Bench>(x: &mut Cx<'_, R>) {
    apply_pending(x);
    next(x);
    if dead(x) {
        mark_dead(x);
    }
    anim(x, 3);
    x.set8(E::COUNTER, 1);
    x.set32(E::VX, 0x0200_0000);
    x.set16(E::VZ, 0x400);
    x.set16(E::GRAVITY, -64i16 as u16);
    x.set32(E::VY, 0);
    orient_speeds(x);
    stun(x);
}

/// 0x3d882: burned (0xe909a), animation D0 left as is, six stars (0xabec8(4)).
pub(super) fn burnt<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(E::VULNERABLE, 0);
    lib::sound(x, crate::services::sound::CMD_176_276);
    apply_pending(x);
    next(x);
    x.set8(E::ACTIVE, 1);
    if x.u16(E::HEIGHT) as i16 > 0 {
        x.set16(E::VZ, 0);
        x.set16(E::GRAVITY, -64i16 as u16);
        x.set32(E::VX, 0);
        x.add8(E::STEP, (-2i8) as u8);
    }
    next(x);
    x.set8(E::DOWNED, 0);
    x.set8(E::GRABBED, 0);
    x.set8(E::COUNTER, 0xf0);
    // Original glitch: the animation is the D0 left by the dispatch on `+6`, the current step (0)
    anim(x, 0);
    if !lib::attack_effects(x, 4, 5) {
        x.set8(E::STEP, 6);
    }
}
pub(super) fn burnt_fall<R: Bench>(x: &mut Cx<'_, R>) {
    move_xz(x);
    if x.u16(E::HEIGHT) as i16 > 0 {
        return;
    }
    x.set32(E::HEIGHT, 0);
    next(x);
}
pub(super) fn burnt_wait<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(E::HEIGHT) != 0 {
        move_xz(x);
        if x.u16(E::HEIGHT) as i16 > 0 {
            return;
        }
        x.set32(E::HEIGHT, 0);
    }
    anim_step(x);
    let c = x.u8(E::COUNTER).wrapping_sub(1);
    x.set8(E::COUNTER, c);
    if c == 0 {
        slide_end(x);
    }
}
