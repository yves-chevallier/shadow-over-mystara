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
//! Animations: 0x175da6 (body), 0x175dc4 (attacks), 0x175e0c (fall, death).

use crate::actors::families::cx::Bench;
use crate::world::objects::player::Player;
use crate::world::data::RomAddr;
use crate::world::record::record;
use super::boss_common::*;
use super::cx::handlers;
use crate::world::objects::Obj;

/// Fields of this boss beyond those of `Enemy` and `Boss` (`record!`). A meaning guessed from the code alone is
/// marked (h).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Boss5(pub Obj);

impl Boss5 {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Boss5 {
        frame_kind, set_frame_kind, FRAME_KIND: u8 @ 0x44;
        spin_delay, set_spin_delay, SPIN_DELAY: u8 @ 0x87;
        fall_timer, set_fall_timer, FALL_TIMER: u8 @ 0x90;
        death_flag, set_death_flag, DEATH_FLAG: u8 @ 0x91;
        count, set_count, COUNT: u16 @ 0xa0;
        blink, set_blink, BLINK: u8 @ 0xa2;
        steps, set_steps, STEPS: u8 @ 0xa3;
        anim_n, set_anim_n, ANIM_N: u8 @ 0xa4;
        anim_set, set_anim_set, ANIM_SET: u8 @ 0xa5;
        cool, set_cool, COOL: u8 @ 0xa6;
        biting, set_biting, BITING: u8 @ 0xa7;
        saved_state, set_saved_state, SAVED_STATE: u32 @ 0xa8;
        in_attack, set_in_attack, IN_ATTACK: u8 @ 0xac;
    }
}
use crate::world::objects::pools;
use crate::world::objects::Current;

/// The handler (0x8341c), which the dispatch loop leaves in A1.
const HANDLER: RomAddr = RomAddr(0x8341c);

const SET_BODY: RomAddr = RomAddr(0x175da6);
const SET_ATTACK: RomAddr = RomAddr(0x175dc4);
const SET_FALL: RomAddr = RomAddr(0x175e0c);

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
    x.set8(Boss5::ANIM_N, n as u8);
    x.set8(Boss5::ANIM_SET, 0);
    x.anim_start(SET_BODY.0, n as u16);
}

fn attack_anim<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.set8(Boss5::ANIM_N, n as u8);
    x.set8(Boss5::ANIM_SET, 1);
    x.anim_start(SET_ATTACK.0, n as u16);
}

fn fall_anim<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.set8(Boss5::ANIM_N, n as u8);
    x.set8(Boss5::ANIM_SET, 2);
    x.anim_start(SET_FALL.0, n as u16);
}

/// 0x83dde: freezes it one animation notch requested by `+0x91` (bit 7 = request).
fn take_frame_request<R: Bench>(x: &mut Cx<'_, R>) {
    let v = x.u8(Boss5::DEATH_FLAG);
    if v & 0x80 == 0 {
        return;
    }
    x.set8(Enemy::PALETTE, v & 0x7f);
    x.set8(Enemy::OWN_PALETTE, 1);
    x.set8(Boss5::DEATH_FLAG, 0);
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
    x.set8(Boss5::BLINK, 0);
    x.set8(Enemy::VULNERABLE, 0);
    x.set8(Enemy::LOOT, 0);
    x.set8(Enemy::EVENT_KIND, 0x27);
    x.set8(Boss::DEATH_CREDIT, 1);
    x.set8(Boss::DEATH_VALUE, 0xff);
    x.set8(Enemy::LOOT_ROW, 0x29);
    var::BOSS_FIGHT.set(x.b, 1);
    lib::clear_popup(x);
    lib::empty_popups(x);
    lib::clear_all_popups(x);
    lib::create_set_15(x);
    x.set8(f::BOX_SET, 0x25);
    lib::initial_hp_boss(x);
    lib::set_immunities(x);
    x.set16(Obj::X, 0x120);
    x.set16(Obj::Z, 0x30);
    x.set16(Obj::Y, 0x178);
    active(x);
}

/// Loop of the active state (0x8349c); the hit state calls it again to resume the fight.
fn active<R: Bench>(x: &mut Cx<'_, R>) {
    if var::PAUSE.get(x.b) != 0 {
        return;
    }
    if x.u8(Boss5::BLINK) != 0 && sub8(x, Boss5::BLINK, 1).gt() {
        x.set8(Enemy::FRAME_TAG, 0);
        x.set8(Boss5::COOL, 0x20);
    }
    if x.u8(f::FREEZE) != 0 {
        sub8(x, f::FREEZE, 1);
        let v = x.u8(Boss::X_LOW) ^ 2;
        x.set8(Boss::X_LOW, v);
        x.set8(Boss5::BLINK, 0x20);
        return;
    }
    if x.u8(Boss5::COOL) != 0 {
        sub8(x, Boss5::COOL, 1);
    } else {
        x.set32(Enemy::HIT_LOCK, 0);
    }
    x.tick_counters();
    let s = x.u32(f::STATE);
    x.set32(Boss5::SAVED_STATE, s);
    match x.u8(f::SUB) {
        0x0 => intro(x),
        0x4 => stroll(x),
        0x8 => dash(x, "boss_5/dash_speeds"),
        0xc => seize(x),
        0x10 => simple_attack(x),
        0x14 => dash(x, "boss_5/dash_speeds_b"),
        0x18 => chase(x),
        _ => {}
    }
    let s = x.u32(f::STATE);
    x.set32(Boss5::SAVED_STATE, s);
    x.reinsert();
}

// --- +5 = 0: arrival -------------------------------------------------------------------------

fn intro<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 4);
            x.set16(Boss5::COUNT, 0x100);
            super::small::background_1_lit_a(x.b, 0x100);
            crate::services::palette::flash(x.b, crate::services::palette::Flash::F);
            var::FADE_DELAY.set(x.b, 0x10);
            var::FADE_PAGES.set(x.b, 0xf);
            var::FADE_STEPS.set(x.b, 8);
            // 0x9b6 with A0 = 0x14de, D0.w = 0x10: fade task (counting down the number of steps of `A5+0x48f`)
            crate::screens::scene::fading::launch(x.b, crate::screens::scene::fading::DESCENT_STEP);
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
                set.set_u8(x.b, Enemy::ACTIVE, 1);
                set.set_u8(x.b, Enemy::KIND, 2);
                set.set_u8(x.b, Enemy::VARIANT, 0x73);
            }
            crate::services::palette::restore_flash(x.b);
            var::FADE_PAGES.set(x.b, 0xf);
            crate::services::palette::full_brightness(x.b);
            next_action(x);
        }
        _ => {}
    }
}

fn intro_wait_scene<R: Bench>(x: &mut Cx<'_, R>) {
    if var::SUB_STEP.get(x.b) != 4 {
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
            let n = word(x, "boss_5/stroll_times", i);
            x.set16(Boss5::COUNT, n);
            if x.u8(Boss5::ANIM_SET) != 0 || x.u8(Boss5::ANIM_N) != 0 {
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
    let n = x.u16(Boss5::COUNT).wrapping_sub(1);
    x.set16(Boss5::COUNT, n);
    if n & 0x8000 != 0 {
        next_action(x);
    }
}

// --- +5 = 8 and 0x14: movement ---------------------------------------------------------------

/// `table`: speeds (words) per difficulty level.
fn dash<R: Bench>(x: &mut Cx<'_, R>, table: &str) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 4);
            let i = level_index(x);
            let n = word(x, table, i);
            x.select_speed(&crate::world::objects::motion::SPEEDS_B, n);
            if x.u8(f::FLIP) != 0 {
                let v = x.u16(Enemy::VX).wrapping_neg();
                x.set16(Enemy::VX, v);
            }
            if x.u8(Boss5::ANIM_SET) != 0 {
                return body(x, 0);
            }
            if x.u8(Boss5::ANIM_N) != 0 {
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
    if sub8(x, Boss5::STEPS, 1).mi() {
        next_action(x);
    }
}

// --- +5 = 0xc : saisie --------------------------------------------------------------------------

fn seize<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Boss5::IN_ATTACK, 1);
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
            lib::seize_player(x, Obj(HANDLER.0), 0);
            grab_begin(x);
        }
        _ => {}
    }
}

fn grab_begin<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB2, 4);
    x.set16(Boss5::COUNT, 0x80);
    x.set8(Boss5::BITING, 1);
    x.set8(Boss5::STEPS, 0);
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
    if x.u8(Boss5::BITING) == 0 {
        p.set_u32(x.b, f::STATE, 0x0204_3200);
        p.set_u8(x.b, Enemy::ACTIVE, 2);
        p.set_u16(x.b, Player::HOLDER_ENEMY, x.o.0 as u16);
        p.set_u8(x.b, Player::HELD, 1);
        x.set8(Boss5::BITING, 1);
        let hp = p.u16(x.b, f::HP);
        p.set_u16(x.b, f::HP, hp.wrapping_sub(0x10));
        if Fl::sub16(hp, 0x10).le() {
            return grab_kill(x);
        }
    }
    if x.u8(f::KEY_FRAME) != 0 {
        lib::sound(x, crate::services::sound::CMD_382);
    }
    if p.u8(x.b, Enemy::ACTIVE) == 0 {
        return grab_release(x);
    }
    if sub8(x, Boss5::STEPS, 1).mi() {
        x.set8(Boss5::STEPS, 3);
        let hp = p.u16(x.b, f::HP);
        p.set_u16(x.b, f::HP, hp.wrapping_sub(1));
        if Fl::sub16(hp, 1).le() {
            return grab_kill(x);
        }
    }
    lib::hold_seized(x);
    x.anim_step();
    let n = x.u16(Boss5::COUNT).wrapping_sub(1);
    x.set16(Boss5::COUNT, n);
    if n & 0x8000 == 0 {
        return;
    }
    let p = x.linked(f::TARGET);
    x.add8(f::SUB2, 4);
    x.set8(Boss5::BITING, 0);
    p.set_u32(x.b, f::STATE, 0x0204_3400);
    p.set_u8(x.b, Player::HELD, 0);
    p.set_u8(x.b, Player::TIMER_6F, 0);
    attack_anim(x, 2);
}

/// The player has no HP left: released dying (1 HP).
fn grab_kill<R: Bench>(x: &mut Cx<'_, R>) {
    let p = x.linked(f::TARGET);
    p.set_u16(x.b, f::HP, 1);
    p.set_u32(x.b, f::STATE, 0x0204_3400);
    p.set_u8(x.b, Player::HELD, 0);
    p.set_u8(x.b, Player::TIMER_6F, 1);
    x.set8(Boss5::BITING, 0);
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
    x.set8(Boss5::IN_ATTACK, 1);
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 4);
            x.set8(Boss5::FRAME_KIND, 0x4b);
            lib::sound(x, crate::services::sound::CMD_382);
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
    x.set8(Boss5::IN_ATTACK, 1);
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 4);
            x.set8(Boss5::BLINK, 0x14);
            lib::sound(x, crate::services::sound::CMD_382);
            x.select_speed(&crate::world::objects::motion::SPEEDS_B, 0x13);
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
    let n = x.u8(Boss5::STEPS);
    x.set8(Boss5::COUNT, n);
    let r = (x.random() as u32) as u8 & 2;
    x.set8(Boss5::STEPS, r);
    attack_anim(x, 0x22);
    chase_follow(x);
}

/// Turns one notch toward the target, advances, grabs a player in range.
fn chase_follow<R: Bench>(x: &mut Cx<'_, R>) {
    x.anim_step();
    let criterion = x.u8(Boss5::STEPS);
    let Some(target) = lib::target_by_criterion(x, criterion) else {
        return chase_stop(x);
    };
    let angle = lib::aim_to(x, target);
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
    let n = x.u8(Boss5::COUNT).wrapping_sub(1);
    x.set8(Boss5::COUNT, n);
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

// --- choice of the next action (0x83e3e) ----------------------------------------------------------

/// Group `+5` and angle from the target's position: rows of the table `boss_5/next_actions` (2 bytes per entry:
/// angle and group, duration and half-turn).
fn next_action<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Boss5::BITING, 0);
    x.set8(Boss5::IN_ATTACK, 0);
    let Some(target) = lib::target_by_criterion(x, lib::NEAREST) else {
        x.set8(f::SUB, 4);
        x.set16(f::SUB2, 0);
        x.set8(Boss5::STEPS, 0x10);
        return;
    };
    let mut row: u32 = 0;
    let dy = target.u16(x.b, Obj::Y).wrapping_sub(x.u16(Obj::Y)) as i16;
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
    let mut dx = x.u16(Obj::X).wrapping_sub(target.u16(x.b, Obj::X)) as i16;
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
    row += (x.random() as u32) & 0xe;
    let a = byte(x, "boss_5/next_actions", row);
    x.set8(Obj::SPEED_INDEX, (a << 1) & 0x1e);
    x.set8(f::SUB, (a & 0xf0) >> 2);
    x.set16(f::SUB2, 0);
    let b = byte(x, "boss_5/next_actions", row + 1);
    x.set8(Boss5::STEPS, b);
    if b & 0x80 != 0 {
        let v = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, v);
    }
    x.set8(Boss5::STEPS, b & 0x7f);
}

// --- state 4: hit --------------------------------------------------------------------------------

fn hit<R: Bench>(x: &mut Cx<'_, R>) {
    let freeze = x.u8(f::FREEZE) as i8;
    if freeze > 0 {
        x.set8(f::FREEZE, (freeze - 1) as u8);
    } else {
        x.set8(Boss5::BITING, 0);
        x.set8(Boss5::IN_ATTACK, 0);
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
    if x.u8(Enemy::REACTION) == 0xa8 {
        return tumble(x);
    }
    if x.u16(f::HP) == 0 {
        x.set32(f::STATE, 0x0406_0000);
        return;
    }
    if x.u8(Enemy::REACTION) == 0x38 {
        // hit without effect: back to the saved state
        let s = x.u32(Boss5::SAVED_STATE);
        x.set32(f::STATE, s);
        let stun = x.u8(Enemy::STUN);
        x.set8(f::FREEZE, stun);
        x.set8(Enemy::ACTIVE, 1);
        return active(x);
    }
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 4);
            let stun = x.u8(Enemy::STUN);
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
    x.set8(Enemy::ACTIVE, 1);
    x.set8(Boss5::BLINK, 0x10);
    x.set8(f::STATE, 2);
    if (x.random() as u32) & 1 != 0 {
        return next_action(x);
    }
    x.set8(Boss5::STEPS, 0x40);
    x.set32(f::STATE, 0x0218_0000);
}

/// `+0x49` = 0xa8: the hit throws the boss, who falls back in a whirlwind (`+6` = 0 to 0x10).
fn tumble<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(Boss5::FALL_TIMER, 0x3c);
            take_frame_request(x);
            x.set32(Enemy::VX_LONG, 0);
            x.set32(Enemy::VZ_LONG, 0);
            x.set32(Enemy::VY_LONG, 0);
            x.set8(Boss5::SPIN_DELAY, if x.counter() & 1 != 0 { 1 } else { 2 });
            let a = ((x.random() as u32) as u16 + 8) & 0x1f;
            x.set8(Obj::SPEED_INDEX, a as u8);
            fall_anim(x, 0);
        }
        2 => {
            if sub8(x, Boss5::FALL_TIMER, 1).ne() {
                spin(x);
            } else {
                x.add8(f::SUB2, 2);
            }
        }
        4 => {
            x.set8(Enemy::VULNERABLE, 0);
            x.add8(f::SUB2, 2);
            x.set8(Boss5::FALL_TIMER, 0x1e);
            x.set8(f::STATE + 3, 0);
            x.set32(Enemy::VX_LONG, 0);
            x.set16(Enemy::VZ, 0x600);
            x.set16(Enemy::GRAVITY, (-0x80i16) as u16);
            fall_anim(x, 0);
            lib::sound(x, crate::services::sound::CMD_31E_41E_B);
            lib::attack_effect(x, 8, 0);
        }
        6 | 0xa => {
            x.anim_step();
            if x.u8(f::STATE + 3) == 0 {
                x.move_xz();
            }
            if sub8(x, Boss5::FALL_TIMER, 1).ne() {
                if (x.u16(Obj::Z) as i16) > 0 {
                    return;
                }
                x.set32(Obj::Z, 0);
                x.add8(f::STATE + 3, 2);
            } else {
                x.add8(f::SUB2, 2);
                x.set8(f::STATE + 3, 0);
            }
        }
        8 => {
            x.add8(f::SUB2, 2);
            x.set8(Boss5::FALL_TIMER, 0x3c);
            x.set32(Enemy::VX_LONG, 0);
            x.set32(Enemy::VY_LONG, 0);
            x.set16(Enemy::VZ, 0x800);
            x.set16(Enemy::GRAVITY, (-0x40i16) as u16);
            fall_anim(x, 0);
            lib::attack_effect(x, 7, 0);
        }
        0xc => {
            x.add8(f::SUB2, 2);
            take_frame_request(x);
            x.set8(Boss5::FALL_TIMER, 0x12);
            x.set16(Enemy::VX, 0x1000);
            x.set16(Enemy::AX, 0);
            x.set32(Enemy::VZ_LONG, 0x0100_0000);
            x.set16(Enemy::GRAVITY, 0);
            fall_anim(x, 0);
            if x.u8(f::FLIP) != 0 {
                let v = x.u16(Enemy::VX).wrapping_neg();
                x.set16(Enemy::VX, v);
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

/// Whirlwind step (0x83da8): movement through the circle of the whirls (`tables::circle`: X and depth offsets per
/// angle `+0x2d`).
fn spin<R: Bench>(x: &mut Cx<'_, R>) {
    x.add32(Obj::Z, 0x20000);
    if (x.u16(Obj::Z) as i16) >= 0x40 {
        x.set32(Obj::Z, 0x0040_0000);
    }
    if sub8(x, Boss5::SPIN_DELAY, 1).eq() {
        x.set8(Boss5::SPIN_DELAY, 1);
        let a = x.u8(Obj::SPEED_INDEX).wrapping_add(2) & 0x1f;
        x.set8(Obj::SPEED_INDEX, a);
        x.set8(f::FLIP, 0);
        if a.wrapping_sub(8) >= 0x10 {
            x.set8(f::FLIP, 0x20);
        }
    }
    let i = (x.u8(Obj::SPEED_INDEX) as u32) << 2;
    let vx = tables::circle(x.b, i).wrapping_add(x.u16(Enemy::AX));
    x.set16(Enemy::VX, vx);
    let dx = ((vx as u32) << 16) as i32 >> 8;
    x.add32(Obj::X, dx as u32);
    let vy = tables::circle(x.b, i + 2).wrapping_add(x.u16(Enemy::AY));
    x.set16(Enemy::VY, vy);
    let dy = ((vy as u32) << 16) as i32 >> 8;
    x.add32(Obj::Y, dy as u32);
}

/// State 6 of `+5`: death (two-stage sequence then release).
fn dying<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Boss5::IN_ATTACK, 2);
    match x.u8(f::SUB2) {
        0 => {
            addq8(x, f::SUB2, 2);
            x.set16(Boss5::COUNT, 0);
            var::BOSS_FIGHT.set(x.b, 0);
            var::PLAYERS_FROZEN.set(x.b, 1);
            crate::services::palette::set_flash_color(x.b, crate::services::palette::FLASH_COLOR_WHITE_VALUE);
            lib::drop_rewards(x);
            crate::services::sound::service(x.b, crate::services::sound::Service::Ff05);
            lib::sound(x, crate::services::sound::CMD_FF);
            let kind = x.u8(Enemy::KIND);
            record_defeat(x, kind);
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
    let n = x.u16(Boss5::COUNT).wrapping_sub(1);
    x.set16(Boss5::COUNT, n);
    if n & 0x8000 != 0 {
        var::SHAKE.set(x.b, 5);
        let r = ((x.random() as u32) & 0x1f) as u16 + 0x10;
        x.set16(Boss5::COUNT, r);
    }
    if step(x) {
        x.free(&pools::BOSS);
    }
}

handlers! {
    BOSS_5 0x8341c "boss_5" run,
}
