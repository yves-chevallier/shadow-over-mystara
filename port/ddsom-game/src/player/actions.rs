//! Common actions of the normal game (`+4` = 2, `+5` = 0): standing, walking, turning, and dispatch
//! to the characters' code.
//!
//! Byte `+6` selects the action in the handler table 0x35586 (26 actions, all
//! even); byte `+7` is the step of the action (`0`: first call, the action prepares its
//! animation; the following values are the steps of a small automaton). An action ends by
//! switching back to `to_stand` or by changing `+4..+7`.
//!
//! Actions 0x0a..0x12 (jump, attacks) are character-specific code: `characters::run`
//! reads their address in a table indexed by `Player::CHARACTER`.

use super::characters::{self, Slot};
use super::moves;
use super::checks::{attack_button, jump_button, turn_check};
use super::states::{action, state};
use super::walk::{clamp_to_screen, move_xy, walk_speed};
use super::{animation, ram, Ctx, Player};
use crate::mem::wr8;



/// 0x35620: standing animation depending on the modes (slow walk, mount, weapon).
pub fn start_stand(g: &mut Ctx, p: Player) {
    cov!();
    let anim = stand_animation(g, p);
    animation::start(g, p, anim);
}

/// Address entries of `to_stand` (0x35614, called by `aids.rs`) and `start_stand` (0x35620). The
/// registers left by the animation (D5, D6, A4) are read by nobody (`DDSOM_POISON=anim`).
fn stand_native(c: &mut m68k::Cpu, b: &mut cps2::Cps2Bus, reset: bool) {
    let p = Player(c.a[0]);
    let mut g = Ctx::new(c, b);
    if reset {
        to_stand(&mut g, p);
    } else {
        start_stand(&mut g, p);
    }
    crate::mem::rts(c, b);
}

pub const TO_STAND: crate::Native = crate::Native {
    entry: 0x35614,
    name: "player_to_stand",
    run: crate::Body::Processeur(|c, b| stand_native(c, b, true)),
    touches_sound: false,
    outputs: crate::regs::ALL,
};

pub const START_STAND: crate::Native = crate::Native {
    entry: 0x35620,
    name: "player_start_stand",
    run: crate::Body::Processeur(|c, b| stand_native(c, b, false)),
    touches_sound: false,
    outputs: crate::regs::ALL,
};

fn stand_animation(g: &mut Ctx, p: Player) -> u16 {
    let mut anim = 1;
    if p.u8(g, Player::SLOW) == 0 {
        anim = 0x2e;
        if p.u8(g, Player::SLOW_8A) == 0 {
            anim = if matches!(p.u8(g, Player::WEAPON_A), 8 | 0x18) { 3 } else { 0 };
        }
    }
    if ram::SLOW_MODE.get(g.b) != 0 {
        anim = 0x35;
    }
    anim
}

/// 0x35614: return to the standing state. Returns true (Z = 0 after the animation).
pub fn to_stand(g: &mut Ctx, p: Player) -> bool {
    cov!();
    p.set_state(g, state::STAND.0);
    p.set_u8(g, Player::SUBACTION, 0);
    start_stand(g, p);
    true
}

/// 0x3553c: runs the current action then re-snaps the player (scenery, screen limits).
pub fn act(g: &mut Ctx, p: Player) {
    cov!();
    run_action(g, p);
    let current = p.u8(g, Player::ACTION);
    if current == action::NO_TERRAIN_B || current == action::NO_TERRAIN_A {
        return;
    }
    let scripted = ram::INPUT_RAW.get(g.b) != 0;
    let skip_terrain = scripted
        || (ram::SCENE.get(g.b) == 4 && current == action::GRAB)
        || current == action::NO_TERRAIN_C;
    if !skip_terrain {
        crate::terrain::push(g.b, p.obj());
    }
    clamp_to_screen(g.b, p);
}

/// Dispatches to the handler of the action (`+6`).
fn run_action(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::ACTION) {
        action::STAND => stand(g, p),
        action::SLOW_SWITCH => slow_switch(g, p),
        action::WALK => walk(g, p),
        action::TURN => turn(g, p),
        action::GRAB => grabbed(g, p),
        action::JUMP => characters::run(g, p, Slot::Jump),
        action::ATTACK => {
            aura_countdown(g, p);
            characters::run(g, p, Slot::Attack)
        }
        action::SPECIAL => {
            aura_countdown(g, p);
            characters::run(g, p, Slot::Special)
        }
        0x10 => characters::run(g, p, Slot::Action10),
        0x12 => characters::run(g, p, Slot::Action12),
        action::RUN => moves::run(g, p),
        action::RELEASE => moves::carry(g, p),
        action::SPECIAL_ALT => moves::guard(g, p, 0x80),
        action::TURN_ATTACK => moves::guard(g, p, 0x10),
        action::NO_TERRAIN_B => moves::hop(g, p),
        0x1c => moves::pause(g, p, 34),
        0x1e => moves::pause(g, p, 33),
        0x28 => moves::slow_end(g, p),
        action::PUSH => moves::push(g, p),
        action::CLIMB => moves::climb(g, p),
        action::NO_TERRAIN_A => moves::exit_scene(g, p),
        action::NO_TERRAIN_C => moves::fall_in(g, p),
        action::SPELL_FIZZLE => moves::spell_fizzle(g, p),
        // rare poses: the original dispatch leaves them D0 = action number (high byte zero), which the routines
        // they call by address may read
        0x26 => {
            g.c.d[0] = 0x26;
            moves::curse_drop(g, p)
        }
        0x2c => {
            g.c.d[0] = 0x2c;
            moves::dust_puffs(g, p)
        }
        0x30 => {
            g.c.d[0] = 0x30;
            moves::box_recoil(g, p)
        }
        other => unreachable!("action {other:#x} outside the original table 0x35586 (26 even actions)"),
    }
}

/// At the first step of an attack, the aura counter (`+0x182`) decreases; at zero the aura goes out.
fn aura_countdown(g: &mut Ctx, p: Player) {
    cov!();
    if p.u8(g, Player::STEP) == 0 && p.u8(g, Player::AURA) != 0 {
        let left = p.u8(g, Player::AURA) - 1;
        p.set_u8(g, Player::AURA, left);
        if left == 0 {
            p.set_u16(g, Player::AURA_TIMER, 0);
            let flags = p.u8(g, Player::FLAGS) & !2;
            p.set_u8(g, Player::FLAGS, flags);
        }
    }
}

/// 0x355ba: standing. Inputs trigger turn, attack, jump or walk; after a
/// state change the animation still advances by one step.
fn stand(g: &mut Ctx, p: Player) {
    cov!();
    if p.u8(g, Player::STEP) == 0 {
        to_stand(g, p);
        return;
    }
    let acted = turn_check(g, p) || attack_button(g, p) || jump_button(g, p);
    if !acted && p.u8(g, Player::KEYS_HELD) & 0xf != 0 {
        p.set_state(g, state::WALK.0);
        if ram::SLOW_MODE.get(g.b) == 0 {
            let anim = if p.u8(g, Player::SLOW_8A) != 0 {
                0x2f
            } else if p.u8(g, Player::SLOW) != 0 {
                4
            } else {
                2
            };
            animation::start(g, p, anim);
        }
        return;
    }
    if ram::SLOW_MODE.get(g.b) == 0 {
        animation::step(g, p);
    }
}

/// 0x35652: toggles slow / normal walk (transition animation) (h).
fn slow_switch(g: &mut Ctx, p: Player) {
    cov!();
    let entering = p.u8(g, Player::STEP) == 0;
    if animation::step(g, p) {
        p.set_u8(g, Player::SLOW, entering as u8);
        to_stand(g, p);
    }
}

/// 0x3567a: walk.
fn walk(g: &mut Ctx, p: Player) {
    cov!();
    if ram::SLOW_MODE.get(g.b) == 0 && p.class(g) == 4 {
        return walk_magic_user(g, p);
    }
    if p.u8(g, Player::KEYS_HELD) & 0xf == 0 {
        to_stand(g, p);
        return;
    }
    if turn_check(g, p) || attack_button(g, p) || jump_button(g, p) {
        return;
    }
    walk_speed(g, p);
    move_xy(g, p);
    animation::step(g, p);
}

/// 0x356c2: magic-user walk (stopping and resuming go through a transition animation).
fn walk_magic_user(g: &mut Ctx, p: Player) {
    cov!();
    if p.u8(g, Player::STEP) != 0 {
        if turn_check(g, p) || attack_button(g, p) || jump_button(g, p) {
            return;
        }
        if animation::step(g, p) {
            to_stand(g, p);
        }
        return;
    }
    if p.u8(g, Player::KEYS_HELD) & 0xf == 0 {
        p.set_state(g, state::STAND.0);
        let mut anim = 1;
        if p.u8(g, Player::SLOW) == 0 {
            anim = 0x2e;
            if p.u8(g, Player::SLOW_8A) == 0 {
                p.set_state(g, 0x0200_0402);
                anim = 3;
            }
        }
        animation::start(g, p, anim);
        return;
    }
    if turn_check(g, p) || attack_button(g, p) || jump_button(g, p) {
        return;
    }
    walk_speed(g, p);
    move_xy(g, p);
    animation::step(g, p);
}

/// 0x358dc: turn.
fn turn(g: &mut Ctx, p: Player) {
    cov!();
    if p.u8(g, Player::STEP) == 0 {
        if attack_button(g, p) || jump_button(g, p) {
            return;
        }
    }
    if animation::step(g, p) {
        to_stand(g, p);
    }
}

/// 0x36abe: the player is grabbed (action `GRAB`), three steps: grab animation (0x27, 0x28 in
/// slow walk), wait for the aggressor's gesture then release of the linked object (`+0x8c`).
fn grabbed(g: &mut Ctx, p: Player) {
    cov!();
    match p.u8(g, Player::STEP) {
        0 => {
            let step = p.u8(g, Player::STEP) + 2;
            p.set_u8(g, Player::STEP, step);
            let anim = if p.u8(g, Player::SLOW) != 0 { 0x28 } else { 0x27 };
            animation::start(g, p, anim);
        }
        2 => {
            animation::step(g, p);
            if p.u8(g, 0x33) == 0 {
                return;
            }
            let step = p.u8(g, Player::STEP) + 2;
            p.set_u8(g, Player::STEP, step);
            let holder = p.u16(g, 0x8c) as i16 as i32 as u32 & 0xff_ffff;
            let kind = crate::mem::rd8(g.b, holder + 2);
            if kind != 6 && kind != 8 {
                wr8(g.b, holder + 5, 4);
            }
        }
        4 => {
            if animation::step(g, p) {
                p.set_u8(g, Player::ACTIVE, 1);
                p.set_u16(g, 0x8c, 0);
                to_stand(g, p);
            }
        }
        v => unreachable!("0x36abe : value {v:#x} outside the original jump table"),
    }
}
