//! Boss of type 5 (0x8341c), scene 58: Ezerhorden (h). See `docs/families/boss.md`.
//!
//! Old template, without cutscene: the state `+4` (0 creation, 2 active, 4 hit, 6 release) and the group
//! `+5` (multiples of 4) are set field by field, `+6` is the step. `0x83e3e` (`next_action`) picks the next
//! group from the target's position.
//!
//! | `+5` | Role |
//! |---|---|
//! | 0 | arrival: the entry leap, then the opening cutscene (waits for camera `A5+0x2c` = 0xc) |
//! | 4 | slow walk in place (`+0xa0` frames) |
//! | 8, 0x14 | movement at a speed chosen by the difficulty (X and Y), direction according to `+0x2c` |
//! | 0xc | grabbing a player: drains them then releases them |
//! | 0x10 | attaque simple (animation jusqu'au bout) |
//! | 0x18 | chase: turns toward the target then grabs |
//! | state 4 | hit: reactions, whirlwind fall (`+0x49` = 0xa8), death |
//!
//! Animations : 0x175da6 (corps), 0x175dc4 (attaques), 0x175e0c (chute, mort).

use crate::families::cx::Bench;
use super::boss_common::*;
use super::cx::handlers;
use crate::objects::Obj;

mod g {
    pub const STUN: u32 = 0x56;
    pub const FRAME_KIND: u32 = 0x44;
    pub const REACTION: u32 = 0x49;
    pub const BITING: u32 = 0xa7;
    pub const STEPS: u32 = 0xa3;
    pub const ANIM_N: u32 = 0xa4;
    pub const ANIM_SET: u32 = 0xa5;
    pub const SAVED_STATE: u32 = 0xa8;
    pub const COUNT: u32 = 0xa0;
    pub const BLINK: u32 = 0xa2;
    pub const COOL: u32 = 0xa6;
    pub const IN_ATTACK: u32 = 0xac;
    pub const FALL_TIMER: u32 = 0x90;
    pub const SPIN_DELAY: u32 = 0x87;
    pub const DEATH_FLAG: u32 = 0x91;
}
use g::*;
use crate::objects::pools;
use crate::objects::Current;

/// The handler (0x8341c), which the dispatch loop leaves in A1.
const HANDLER: u32 = 0x8341c;

const SET_BODY: u32 = 0x175da6;
const SET_ATTACK: u32 = 0x175dc4;
const SET_FALL: u32 = 0x175e0c;
/// Table of (X, Y) vectors per angle (32 × 4 bytes), walked through during the whirlwind.
const SPIN_VECTORS: u32 = 0x1c94b6;

/// Entry: the game does not go through the pause hook.
pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        st::INIT => init(x),
        st::ACTIVE => active(x),
        st::HIT => hit(x),
        _ => x.free(&pools::BOSS),
    }
}

// --- aides --------------------------------------------------------------------------------------

/// 0x83dfe, 0x83e12, 0x83e28: starts script `n` of one of the three animation sets.
fn body<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.set8(ANIM_N, n as u8);
    x.set8(ANIM_SET, 0);
    x.anim_start(SET_BODY, n as u16);
}

fn attack_anim<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.set8(ANIM_N, n as u8);
    x.set8(ANIM_SET, 1);
    x.anim_start(SET_ATTACK, n as u16);
}

fn fall_anim<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.set8(ANIM_N, n as u8);
    x.set8(ANIM_SET, 2);
    x.anim_start(SET_FALL, n as u16);
}

/// 0x83dde: freezes it one animation notch requested by `+0x91` (bit 7 = request).
fn take_frame_request<R: Bench>(x: &mut Cx<'_, R>) {
    let v = x.u8(DEATH_FLAG);
    if v & 0x80 == 0 {
        return;
    }
    x.set8(0x2b, v & 0x7f);
    x.set8(0x2a, 1);
    x.set8(DEATH_FLAG, 0);
}

/// Index into the difficulty tables: `(difficulty & 0xfc) / 2` (words).
fn level_index<R: Bench>(x: &mut Cx<'_, R>) -> u32 {
    (var::RANK.get(x.b) as u32 & 0xfc) >> 1
}

fn step<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    x.anim_step()
}

// --- state 0: creation, state 2: active ------------------------------------------------------------

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    x.set8(BLINK, 0);
    x.set8(0x67, 0);
    x.set8(0x81, 0);
    x.set8(0x7e, 0x27);
    x.set8(0x93, 1);
    x.set8(0x9a, 0xff);
    x.set8(0x80, 0x29);
    x.set_a5_u8(0x50, 1);
    lib::clear_popup(x);
    lib::empty_popups(x);
    lib::clear_all_popups(x);
    lib::create_set_15(x);
    x.set8(f::BOX_SET, 0x25);
    lib::initial_hp_boss(x);
    lib::set_immunities(x);
    x.set16(0x8, 0x120);
    x.set16(0xc, 0x30);
    x.set16(0x10, 0x178);
    active(x);
}

/// Loop of the active state (0x8349c); the hit state calls it again to resume the fight.
fn active<R: Bench>(x: &mut Cx<'_, R>) {
    if var::PAUSE.get(x.b) != 0 {
        return;
    }
    if x.u8(BLINK) != 0 && sub8(x, BLINK, 1).gt() {
        x.set8(0x38, 0);
        x.set8(COOL, 0x20);
    }
    if x.u8(f::FREEZE) != 0 {
        sub8(x, f::FREEZE, 1);
        let v = x.u8(0x9) ^ 2;
        x.set8(0x9, v);
        x.set8(BLINK, 0x20);
        return;
    }
    if x.u8(COOL) != 0 {
        sub8(x, COOL, 1);
    } else {
        x.set32(0x58, 0);
    }
    x.tick_counters();
    let s = x.u32(f::STATE);
    x.set32(SAVED_STATE, s);
    match x.u8(f::SUB) {
        0x0 => intro(x),
        0x4 => stroll(x),
        0x8 => dash(x, 0x8369c),
        0xc => seize(x),
        0x10 => simple_attack(x),
        0x14 => dash(x, 0x836fa),
        0x18 => chase(x),
        _ => {}
    }
    let s = x.u32(f::STATE);
    x.set32(SAVED_STATE, s);
    x.reinsert();
}

// --- +5 = 0: arrival -------------------------------------------------------------------------

fn intro<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 4);
            x.set16(COUNT, 0x100);
            super::small::background_1_lit_a(x.b, 0x100);
            crate::palette::flash(x.b, crate::palette::Flash::F);
            x.set_a5_u16(0x490, 0x10);
            x.set_a5_u8(0x48e, 0xf);
            x.set_a5_u8(0x48f, 8);
            // 0x9b6 with A0 = 0x14de, D0.w = 0x10: fade task (counting down the number of steps of `A5+0x48f`)
            crate::scene::fading::launch(x.b, crate::scene::fading::DESCENT_STEP);
            intro_wait_scene(x);
        }
        4 => intro_wait_scene(x),
        8 => {
            if step(x) {
                x.add8(f::SUB2, 4);
                body(x, 0);
            }
        }
        0xc => {
            if var::PHASE.get(x.b) != 0xc {
                return;
            }
            lib::create_set_14(x);
            if let Some(set) = x.alloc(&pools::SET) {
                set.set_u8(x.b, 0, 1);
                set.set_u8(x.b, 2, 2);
                set.set_u8(x.b, 3, 0x73);
            }
            crate::palette::restore_flash(x.b);
            x.set_a5_u8(0x48e, 0xf);
            crate::palette::full_brightness(x.b);
            next_action(x);
        }
        _ => {}
    }
}

fn intro_wait_scene<R: Bench>(x: &mut Cx<'_, R>) {
    if x.a5_u8(0x2e) != 4 {
        return;
    }
    x.add8(f::SUB2, 4);
    eshl_pair_3f(x);
    body(x, 1);
}

// --- +5 = 4 : marche lente ------------------------------------------------------------------------

fn stroll<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 4);
            let i = level_index(x);
            let n = x.rom16(0x8362e + i);
            x.set16(COUNT, n);
            if x.u8(ANIM_SET) != 0 || x.u8(ANIM_N) != 0 {
                return body(x, 0);
            }
            stroll_step(x);
        }
        4 => stroll_step(x),
        _ => {}
    }
}

fn stroll_step<R: Bench>(x: &mut Cx<'_, R>) {
    x.anim_step();
    let n = x.u16(COUNT).wrapping_sub(1);
    x.set16(COUNT, n);
    if n & 0x8000 != 0 {
        next_action(x);
    }
}

// --- +5 = 8 and 0x14: movement ---------------------------------------------------------------

/// `table`: speeds (words) per difficulty level.
fn dash<R: Bench>(x: &mut Cx<'_, R>, table: u32) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 4);
            let i = level_index(x);
            let n = x.rom16(table + i);
            x.select_speed(&crate::objects::motion::SPEEDS_B, n);
            if x.u8(f::FLIP) != 0 {
                let v = x.u16(0x14).wrapping_neg();
                x.set16(0x14, v);
            }
            if x.u8(ANIM_SET) != 0 {
                return body(x, 0);
            }
            if x.u8(ANIM_N) != 0 {
                body(x, 0);
            }
            dash_step(x);
        }
        4 => dash_step(x),
        _ => {}
    }
}

fn dash_step<R: Bench>(x: &mut Cx<'_, R>) {
    x.move_xy();
    x.anim_step();
    if sub8(x, STEPS, 1).mi() {
        next_action(x);
    }
}

// --- +5 = 0xc : saisie --------------------------------------------------------------------------

fn seize<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(IN_ATTACK, 1);
    match x.u8(f::SUB2) {
        0 => grab_begin(x),
        4 => grab_hold(x),
        8 => grab_end(x),
        0xc => {
            // Original glitch: the grab's jump table has only three entries (0, 4, 8); `+6` = 0xc lands on
            // the `moveq #0,D0 ; jsr 0xfd64` of the chase (0x8373a), which "grabs" the object designated by A1. A1 is
            // then the handler's address, set by the dispatch loop (`movea.l (table,D0),A1 ; jsr (A1)`),
            // which nothing changes before: the "grab" writes into the ROM (no effect) and the target `+0x50` becomes
            // 0x341c.
            lib::seize_player(x, Obj(HANDLER), 0);
            grab_begin(x);
        }
        _ => {}
    }
}

fn grab_begin<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB2, 4);
    x.set16(COUNT, 0x80);
    x.set8(BITING, 1);
    x.set8(STEPS, 0);
    attack_anim(x, 1);
    grab_hold(x);
}

/// The grabbed player (`+0x50`) loses HP for as long as the grab lasts.
fn grab_hold<R: Bench>(x: &mut Cx<'_, R>) {
    let target = x.u16(f::TARGET);
    if target == 0 {
        return grab_release(x);
    }
    let p = x.linked(f::TARGET);
    if x.u8(BITING) == 0 {
        p.set_u32(x.b, f::STATE, 0x0204_3200);
        p.set_u8(x.b, 0, 2);
        p.set_u16(x.b, 0x186, x.o.0 as u16);
        p.set_u8(x.b, 0x188, 1);
        x.set8(BITING, 1);
        let hp = p.u16(x.b, f::HP);
        p.set_u16(x.b, f::HP, hp.wrapping_sub(0x10));
        if Fl::sub16(hp, 0x10).le() {
            return grab_kill(x);
        }
    }
    if x.u8(f::KEY_FRAME) != 0 {
        lib::sound(x, 0xe425c);
    }
    if p.u8(x.b, 0) == 0 {
        return grab_release(x);
    }
    if sub8(x, STEPS, 1).mi() {
        x.set8(STEPS, 3);
        let hp = p.u16(x.b, f::HP);
        p.set_u16(x.b, f::HP, hp.wrapping_sub(1));
        if Fl::sub16(hp, 1).le() {
            return grab_kill(x);
        }
    }
    lib::hold_seized(x);
    x.anim_step();
    let n = x.u16(COUNT).wrapping_sub(1);
    x.set16(COUNT, n);
    if n & 0x8000 == 0 {
        return;
    }
    let p = x.linked(f::TARGET);
    x.add8(f::SUB2, 4);
    x.set8(BITING, 0);
    p.set_u32(x.b, f::STATE, 0x0204_3400);
    p.set_u8(x.b, 0x188, 0);
    p.set_u8(x.b, 0x6f, 0);
    attack_anim(x, 2);
}

/// The player has no HP left: released dying (1 HP).
fn grab_kill<R: Bench>(x: &mut Cx<'_, R>) {
    let p = x.linked(f::TARGET);
    p.set_u16(x.b, f::HP, 1);
    p.set_u32(x.b, f::STATE, 0x0204_3400);
    p.set_u8(x.b, 0x188, 0);
    p.set_u8(x.b, 0x6f, 1);
    x.set8(BITING, 0);
    grab_release(x);
}

fn grab_release<R: Bench>(x: &mut Cx<'_, R>) {
    attack_anim(x, 2);
    x.add8(f::SUB2, 4);
    grab_end(x);
}

fn grab_end<R: Bench>(x: &mut Cx<'_, R>) {
    if step(x) {
        next_action(x);
    }
}

// --- +5 = 0x10 : attaque simple ---------------------------------------------------------------

fn simple_attack<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(IN_ATTACK, 1);
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 4);
            x.set8(FRAME_KIND, 0x4b);
            lib::sound(x, 0xe425c);
            attack_anim(x, 0);
            end_of_script(x);
        }
        4 => end_of_script(x),
        _ => {}
    }
}

fn end_of_script<R: Bench>(x: &mut Cx<'_, R>) {
    if step(x) {
        next_action(x);
    }
}

// --- +5 = 0x18 : poursuite --------------------------------------------------------------------

fn chase<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(IN_ATTACK, 1);
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 4);
            x.set8(BLINK, 0x14);
            lib::sound(x, 0xe425c);
            x.select_speed(&crate::objects::motion::SPEEDS_B, 0x13);
            x.set8(Obj::SPEED_INDEX, 0x18);
            if x.u8(f::FLIP) != 0 {
                x.set8(Obj::SPEED_INDEX, 8);
            }
            attack_anim(x, 0x21);
            chase_turn(x);
        }
        4 => chase_turn(x),
        8 => chase_follow(x),
        0xc => end_of_script(x),
        _ => {}
    }
}

fn chase_turn<R: Bench>(x: &mut Cx<'_, R>) {
    if !step(x) {
        return;
    }
    x.add8(f::SUB2, 4);
    let n = x.u8(STEPS);
    x.set8(COUNT, n);
    let r = random(x) as u8 & 2;
    x.set8(STEPS, r);
    attack_anim(x, 0x22);
    chase_follow(x);
}

/// Turns one notch toward the target, advances, grabs a player in range.
fn chase_follow<R: Bench>(x: &mut Cx<'_, R>) {
    x.anim_step();
    let criterion = x.u8(STEPS);
    let Some(target) = lib::target_by_criterion(x, criterion) else {
        return chase_stop(x);
    };
    let angle = lib::aim_to(x, target) as u8;
    let mine = x.u8(Obj::SPEED_INDEX);
    let diff = angle.wrapping_sub(mine) & 0x1f;
    if diff != 0 {
        let mut a = mine;
        if diff >= 0x10 {
            a = a.wrapping_sub(2);
        }
        a = a.wrapping_add(1) & 0x1f;
        x.set8(Obj::SPEED_INDEX, a);
    }
    let a = x.u8(Obj::SPEED_INDEX);
    if a > 0x10 {
        x.set8(f::FLIP, 0);
    } else if a != 0x10 && a != 0 {
        x.set8(f::FLIP, 0x20);
    }
    x.move_by_table_xy();
    if let Some(player) = lib::attack_hits_a_player(x, 8) {
        lib::seize_player(x, player, 0);
        return grab_begin(x);
    }
    let n = x.u8(COUNT).wrapping_sub(1);
    x.set8(COUNT, n);
    if n & 0x80 == 0 {
        return;
    }
    chase_stop(x);
}

fn chase_stop<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB2, 4);
    attack_anim(x, 2);
    end_of_script(x);
}

// --- choix de l'action suivante (0x83e3e) ----------------------------------------------------------

/// Group `+5` and angle from the target's position: rows of table 0x83f4e (2 bytes per entry:
/// angle and group, duration and half-turn).
fn next_action<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(BITING, 0);
    x.set8(IN_ATTACK, 0);
    let Some(target) = lib::target_by_criterion(x, lib::NEAREST) else {
        x.set8(f::SUB, 4);
        x.set16(f::SUB2, 0);
        x.set8(STEPS, 0x10);
        return;
    };
    let mut row: u32 = 0;
    let dy = target.u16(x.b, 0x10).wrapping_sub(x.u16(0x10)) as i16;
    row += if dy >= 0x14 {
        0
    } else if dy < -0x14 {
        0x40
    } else if dy >= 4 {
        0x10
    } else if dy < -4 {
        0x30
    } else {
        0x20
    };
    let mut dx = x.u16(0x8).wrapping_sub(target.u16(x.b, 0x8)) as i16;
    if x.u8(f::FLIP) != 0 {
        dx = dx.wrapping_neg();
    }
    row += if dx >= 0x80 {
        0xa0
    } else if dx < -0x80 {
        0x190
    } else if dx >= 0x40 {
        0x50
    } else if dx < -0x40 {
        0x140
    } else if dx >= 0 {
        0
    } else {
        0xf0
    };
    row += random(x) & 0xe;
    let a = x.rom8(0x83f4e + row);
    x.set8(Obj::SPEED_INDEX, (a << 1) & 0x1e);
    x.set8(f::SUB, (a & 0xf0) >> 2);
    x.set16(f::SUB2, 0);
    let b = x.rom8(0x83f4f + row);
    x.set8(STEPS, b);
    if b & 0x80 != 0 {
        let v = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, v);
    }
    x.set8(STEPS, b & 0x7f);
}

// --- state 4: hit --------------------------------------------------------------------------------

fn hit<R: Bench>(x: &mut Cx<'_, R>) {
    let freeze = x.u8(f::FREEZE) as i8;
    if freeze > 0 {
        x.set8(f::FREEZE, (freeze - 1) as u8);
    } else {
        x.set8(BITING, 0);
        x.set8(IN_ATTACK, 0);
        match x.u8(f::SUB) {
            0 | 2 | 4 => reaction(x),
            6 => dying(x),
            _ => {}
        }
    }
    lib::keep_in_zone(x);
    lib::collide_terrain(x);
}

fn reaction<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(REACTION) == 0xa8 {
        return tumble(x);
    }
    if x.u16(f::HP) == 0 {
        x.set32(f::STATE, 0x0406_0000);
        return;
    }
    if x.u8(REACTION) == 0x38 {
        // hit without effect: back to the saved state
        let s = x.u32(SAVED_STATE);
        x.set32(f::STATE, s);
        let stun = x.u8(STUN);
        x.set8(f::FREEZE, stun);
        x.set8(0, 1);
        return active(x);
    }
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 4);
            let stun = x.u8(STUN);
            x.set8(f::FREEZE, stun);
            fall_anim(x, 0);
            recover(x);
        }
        4 => recover(x),
        _ => {}
    }
}

/// Waits for the end of the recoil, then resumes the fight (or jumps to the chase).
fn recover<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::FREEZE) != 0 {
        sub8(x, f::FREEZE, 1);
        return;
    }
    if !step(x) {
        return;
    }
    x.set8(0, 1);
    x.set8(BLINK, 0x10);
    x.set8(f::STATE, 2);
    if random(x) & 1 != 0 {
        return next_action(x);
    }
    x.set8(STEPS, 0x40);
    x.set32(f::STATE, 0x0218_0000);
}

/// `+0x49` = 0xa8: the hit throws the boss, who falls back in a whirlwind (`+6` = 0 to 0x10).
fn tumble<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(FALL_TIMER, 0x3c);
            take_frame_request(x);
            x.set32(0x14, 0);
            x.set32(0x18, 0);
            x.set32(0x1c, 0);
            x.set8(SPIN_DELAY, if x.counter() & 1 != 0 { 1 } else { 2 });
            let a = (random(x) as u16 + 8) & 0x1f;
            x.set8(Obj::SPEED_INDEX, a as u8);
            fall_anim(x, 0);
        }
        2 => {
            if sub8(x, FALL_TIMER, 1).ne() {
                spin(x);
            } else {
                x.add8(f::SUB2, 2);
            }
        }
        4 => {
            x.set8(0x67, 0);
            x.add8(f::SUB2, 2);
            x.set8(FALL_TIMER, 0x1e);
            x.set8(f::STATE + 3, 0);
            x.set32(0x14, 0);
            x.set16(0x18, 0x600);
            x.set16(0x1a, (-0x80i16) as u16);
            fall_anim(x, 0);
            lib::sound(x, 0xebbda);
            lib::attack_effect(x, 8, 0);
        }
        6 | 0xa => {
            x.anim_step();
            if x.u8(f::STATE + 3) == 0 {
                x.move_xz();
            }
            if sub8(x, FALL_TIMER, 1).ne() {
                if (x.u16(0xc) as i16) > 0 {
                    return;
                }
                x.set32(0xc, 0);
                x.add8(f::STATE + 3, 2);
            } else {
                x.add8(f::SUB2, 2);
                x.set8(f::STATE + 3, 0);
            }
        }
        8 => {
            x.add8(f::SUB2, 2);
            x.set8(FALL_TIMER, 0x3c);
            x.set32(0x14, 0);
            x.set32(0x1c, 0);
            x.set16(0x18, 0x800);
            x.set16(0x1a, (-0x40i16) as u16);
            fall_anim(x, 0);
            lib::attack_effect(x, 7, 0);
        }
        0xc => {
            x.add8(f::SUB2, 2);
            take_frame_request(x);
            x.set8(FALL_TIMER, 0x12);
            x.set16(0x14, 0x1000);
            x.set16(0x16, 0);
            x.set32(0x18, 0x0100_0000);
            x.set16(0x1a, 0);
            fall_anim(x, 0);
            if x.u8(f::FLIP) != 0 {
                let v = x.u16(0x14).wrapping_neg();
                x.set16(0x14, v);
            }
            lib::attack_effect(x, 2, 0);
        }
        0xe => {}
        0x10 => {
            x.set16(f::HP, 0);
            x.set8(f::SUB, 6);
            x.set16(f::SUB2, 0);
        }
        _ => {}
    }
}

/// Whirlwind step (0x83da8): movement through the vector table of angle `+0x2d`.
fn spin<R: Bench>(x: &mut Cx<'_, R>) {
    x.add32(0xc, 0x20000);
    if (x.u16(0xc) as i16) >= 0x40 {
        x.set32(0xc, 0x0040_0000);
    }
    if sub8(x, SPIN_DELAY, 1).eq() {
        x.set8(SPIN_DELAY, 1);
        let a = x.u8(Obj::SPEED_INDEX).wrapping_add(2) & 0x1f;
        x.set8(Obj::SPEED_INDEX, a);
        x.set8(f::FLIP, 0);
        if a.wrapping_sub(8) >= 0x10 {
            x.set8(f::FLIP, 0x20);
        }
    }
    let i = (x.u8(Obj::SPEED_INDEX) as u32) << 2;
    let vx = x.data16(SPIN_VECTORS + i).wrapping_add(x.u16(0x16));
    x.set16(0x14, vx);
    let dx = ((vx as u32) << 16) as i32 >> 8;
    x.add32(0x8, dx as u32);
    let vy = x.data16(SPIN_VECTORS + i + 2).wrapping_add(x.u16(0x1e));
    x.set16(0x1c, vy);
    let dy = ((vy as u32) << 16) as i32 >> 8;
    x.add32(0x10, dy as u32);
}

/// State 6 of `+5`: death (two-stage sequence then release).
fn dying<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(IN_ATTACK, 2);
    match x.u8(f::SUB2) {
        0 => {
            addq8(x, f::SUB2, 2);
            x.set16(COUNT, 0);
            x.set_a5_u8(0x50, 0);
            x.set_a5_u8(0x42, 1);
            crate::palette::set_flash_color(x.b, crate::palette::FLASH_COLOR_WHITE_VALUE);
            lib::drop_rewards(x);
            crate::sound::service(x.b, crate::sound::Service::Ff05);
            lib::sound(x, 0xe3b6c);
            let kind = x.u8(0x2);
            set_abs8(x, 0xff2a84, kind);
            set_abs8(x, 0xff2a85, 0);
            let bar = x.u8(0x7e);
            set_abs8(x, 0xff2a86, bar);
            lib::flash_and_shake(x);
            lib::object_event(x);
            fall_anim(x, 4);
            dying_burst(x);
        }
        2 => dying_burst(x),
        _ => {}
    }
}

fn dying_burst<R: Bench>(x: &mut Cx<'_, R>) {
    let n = x.u16(COUNT).wrapping_sub(1);
    x.set16(COUNT, n);
    if n & 0x8000 != 0 {
        x.set_a5_u16(0x4be, 5);
        let r = (random(x) & 0x1f) as u16 + 0x10;
        x.set16(COUNT, r);
    }
    if step(x) {
        x.free(&pools::BOSS);
    }
}

handlers! {
    0x8341c "boss_5" #[without] run,
}
