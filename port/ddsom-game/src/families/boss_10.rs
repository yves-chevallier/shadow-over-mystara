//! Boss of type 10 (0x8ca4a), scenes 65 and 66: Nagpa (h). See `docs/families/boss.md`.
//!
//! Same template as bosses 17 and 6 (states `+4`, groups `+5`, steps `+6` / `+7`, states stored as longs), but
//! shorter: it appears (opening cutscene: it creates an enemy of type 18 and a boss of type 19, variant 1),
//! walks toward a player, jumps on them or fires a shot at them (ESHL 0x21), then dies through a three-stage fall.
//!
//! | `+5` | Role |
//! |---|---|
//! | 0 | opening: waits for the scene (`A5+0x2e` = 6), creates its two companions, introduces itself |
//! | 2 | fight: wait (`+6` = 0), U-turn (2), walk (4), jump (6) |
//! | 4 | ranged shot: recovery (`+6` = 0), shot with cutscene (2) |
//! | 6 | waiting with no player |
//! | state 4 | hit: reactions (table 0x8d176 of which 0x8d294 is specific), get-up, death |
//!
//! Animations: 0x17b14e (moves), 0x17b162 (shots), 0x17b166 (knockback, death).

use crate::families::cx::Bench;
use crate::data::RomAddr;
use crate::objects::Obj;
use crate::record::record;
use super::boss_common::*;
use super::cx::handlers;
use super::eshl_b::shot33::Shot33;
use super::eshl_b::ShotB;

/// Fields of this boss beyond those of `Enemy` and `Boss` (`record!`). A meaning guessed from the code alone is
/// marked (h).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Boss10(pub Obj);

impl Boss10 {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Boss10 {
        /// Hits taken while stunned, then the countdown before getting up (word; `Boss::DOWN_TIME`).
        hits, set_hits, HITS: u16 @ 0x84;
        /// The shot of the cinematic attack (ESHL 0x21, word).
        shot, set_shot, SHOT: u16 @ 0xa8;
        /// Landing point of the jump (words).
        jump_x, set_jump_x, JUMP_X: u16 @ 0xaa;
        jump_y, set_jump_y, JUMP_Y: u16 @ 0xac;
        /// Countdown between two calls while no player is alive.
        wait_blink, set_wait_blink, WAIT_BLINK: u8 @ 0xae;
    }
}
use crate::objects::pools;
use crate::objects::Current;

const SET_BODY: RomAddr = RomAddr(0x17b14e);
const SET_SHOT: RomAddr = RomAddr(0x17b162);
const SET_DIE: RomAddr = RomAddr(0x17b166);
/// Walk sets (0x103e0) depending on the direction (Z of 0x10560).
const WALK_A: RomAddr = RomAddr(0x17b21c);
const WALK_B: RomAddr = RomAddr(0x17b2a0);

const WAIT: u32 = 0x0206_0000;
const FIGHT: u32 = 0x0202_0000;
const INTERRUPTED: u32 = 0x0202_0200;
const SHOOT_FAR: u32 = 0x0204_0200;
const SHOOT_NEAR: u32 = 0x0204_0000;
const REACTIONS: RomAddr = RomAddr(0x8d176);
const STUN_REACTION: RomAddr = RomAddr(0x8d294);

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    x.tick_counters();
    match x.u8(f::STATE) {
        st::INIT => init(x),
        st::ACTIVE => active(x),
        st::HIT => hit(x),
        _ => x.free(&pools::BOSS),
    }
    if x.u8(Enemy::ACTIVE) != 0 {
        x.reinsert();
    }
}

// --- helpers -------------------------------------------------------------------------------------

fn body<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.anim_start(SET_BODY.0, n as u16);
}

fn players_alive<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    var::PLAYERS_ALIVE.get(x.b) != 0
}

fn wait<R: Bench>(x: &mut Cx<'_, R>) {
    x.set32(f::STATE, WAIT);
}

fn zone_and_shadow<R: Bench>(x: &mut Cx<'_, R>) {
    lib::keep_in_zone(x);
    lib::collide_terrain(x);
}

/// 0x8d526: picks the next combat step (`+6` = 0, 4 or 6) from a 32-byte table.
fn next_action<R: Bench>(x: &mut Cx<'_, R>) {
    let r = (x.random() as u32) & 0x1f;
    x.set32(f::STATE, FIGHT);
    let step = byte(x, "boss_10/next_steps", r);
    x.set8(f::SUB2, step);
}

/// 0x8d560: interrupts the action (saved state).
fn interrupt<R: Bench>(x: &mut Cx<'_, R>) {
    let s = x.u32(f::STATE);
    x.set32(Boss::SAVED_STATE, s);
    x.set32(f::STATE, INTERRUPTED);
}

fn needs_turn<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    !lib::facing_target(x)
}

fn facing_forward<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    lib::moving_forward(x)
}

/// Is a player inside the box? 0 otherwise (0x8d4ee, 0x8d50a).
fn probe<R: Bench>(x: &mut Cx<'_, R>, d0: i16, d1: u16, d2: u16) -> bool {
    if !lib::target_in_box(x, d0, d1, d2) {
        return false;
    }
    lib::turn_decision(x) != lib::Turn::None
}

// --- state 0: creation ------------------------------------------------------------------------

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    lib::clear_popup(x);
    lib::empty_popups(x);
    lib::clear_all_popups(x);
    x.add8(f::STATE, 2);
    var::BOSS_FIGHT.set(x.b, 1);
    var::STAGE_CLOCK.set(x.b, 0x0459_5900);
    var::SCENE_LEVEL.set(x.b, 0);
    x.set8(Enemy::OWN_PALETTE, 1);
    x.set8(Enemy::PALETTE, 0x18);
    x.set16(Obj::X, 0xf0);
    x.set16(Obj::Y, 0x160);
    x.set16(Obj::Z, 0);
    x.set8(f::BOX_SET, 0x31);
    lib::initial_hp_boss(x);
    pick_target(x);
    x.set8(Boss::DEATH_CREDIT, 1);
    x.set8(Boss::DEATH_VALUE, 0xff);
    for (off, v) in [(Enemy::AI_BYTE_ROW, 0), (Enemy::AI_STANCE_ROW, 0), (Enemy::AI_WANDER_ROW, 0), (Enemy::AI_BOX_ANIM, 0), (Enemy::LOOT_ROW, 0), (Enemy::AI_68, 0), (Enemy::AI_69, 0), (Enemy::VULNERABLE, 0)] {
        x.set8(off, v);
    }
    for (off, v) in [(Enemy::AI_DECISION_A, 1), (Enemy::AI_DECISION_B, 1), (Enemy::AI_WAIT_MASKS, 6), (Enemy::AI_WAIT_ROW, 6), (Enemy::AI_BOX_CHANCE, 7), (Enemy::AI_SHOT, 0xff), (Enemy::AI_ROW_8E, 3), (Enemy::AI_SPARE_ROW, 3)] {
        x.set8(off, v);
    }
    x.set32(f::IMMUNITY, 0xffff_ffff);
    x.set8(Boss::ANIM_KIND, 0);
    x.set8(Boss::ANIM_LAST, 0xff);
    for (off, v) in [(Enemy::EVENT_KIND, 0x21), (Enemy::EVENT_LOW, 0), (Enemy::LOOT, 0)] {
        x.set8(off, v);
    }
    body(x, 0);
    intro(x);
}

// --- state 2: active -----------------------------------------------------------------------------

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    if tst8(x, f::FREEZE).gt() {
        sub8(x, f::FREEZE, 1);
        return zone_and_shadow(x);
    }
    if var::PAUSE.get(x.b) == 0 {
        match x.u8(f::SUB) {
            0 => intro(x),
            2 => fight(x),
            4 => shoot(x),
            6 => wait_state(x),
            _ => {}
        }
        if x.u8(f::SUB) == 0 {
            return;
        }
    }
    zone_and_shadow(x);
}

// --- +5 = 0: opening --------------------------------------------------------------------------

fn intro<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            if var::SUB_STEP.get(x.b) != 6 {
                x.anim_step();
                return;
            }
            x.add8(f::SUB2, 2);
            x.set16(Boss::TIMER, 0x20);
            x.anim_start(SET_BODY.0, 6);
            lib::sound(x, crate::sound::CMD_5D);
        }
        2 => {
            if sub16(x, Boss::TIMER, 1).ne() {
                x.anim_step();
                return;
            }
            x.add8(f::SUB2, 2);
            x.set16(Boss::TIMER, 0x60);
            // an enemy (type 18, variant 0xff) and a boss (type 19, variant 1) linked to Nagpa
            let enemy = alloc(x, &pools::ENEMY);
            enemy.set_u8(x.b, Obj::ACTIVE, 1);
            enemy.set_u8(x.b, Obj::KIND, 0x12);
            enemy.set_u8(x.b, Obj::VARIANT, 0xff);
            enemy.set_u16(x.b, f::HOST, x.o.0 as u16);
            let boss = alloc(x, &pools::BOSS);
            boss.set_u8(x.b, Obj::ACTIVE, 1);
            boss.set_u8(x.b, Obj::KIND, 0x13);
            boss.set_u8(x.b, Obj::VARIANT, 1);
            boss.set_u16(x.b, f::HOST, x.o.0 as u16);
            body(x, 0);
        }
        4 => {
            if sub16(x, Boss::TIMER, 1).ne() {
                x.anim_step();
                return;
            }
            x.add8(f::SUB2, 2);
            x.anim_start(SET_SHOT.0, 0);
        }
        6 => {
            if !x.anim_step() {
                return;
            }
            x.add8(f::SUB, 2);
            x.set16(f::SUB2, 0);
            body(x, 0);
            lib::set_immunities(x);
            lib::create_set_14(x);
        }
        _ => {}
    }
}

// --- +5 = 2: combat ------------------------------------------------------------------------------

fn fight<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => idle(x),
        2 => turn(x),
        4 => walk(x),
        6 => jump(x),
        _ => {}
    }
}

fn idle<R: Bench>(x: &mut Cx<'_, R>) {
    if !players_alive(x) {
        return x.set32(f::STATE, WAIT);
    }
    if x.u8(Enemy::SUB_STEP) == 0 {
        x.add8(Enemy::SUB_STEP, 2);
        lib::draw_6e(x);
        x.set8(Boss::ANIM_KIND, 0);
        if x.u8(Boss::ANIM_LAST) != 0 {
            x.set8(Boss::ANIM_LAST, 0);
            body(x, 0);
        }
        return;
    }
    if sub8(x, Boss::IDLE_COUNT, 1).mi() {
        return next_action(x);
    }
    if needs_turn(x) {
        return interrupt(x);
    }
    x.anim_step();
}

fn turn<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Enemy::SUB_STEP) == 0 {
        x.add8(Enemy::SUB_STEP, 2);
        x.set8(Boss::ANIM_KIND, 1);
        body(x, 1);
        return;
    }
    if !x.anim_step() {
        return;
    }
    let side = x.u8(f::FLIP) ^ 0x20;
    x.set8(f::FLIP, side);
    body(x, 0);
    next_action(x);
}

fn walk<R: Bench>(x: &mut Cx<'_, R>) {
    if !players_alive(x) {
        return wait(x);
    }
    if x.u8(Enemy::SUB_STEP) != 0 {
        return keep_walking(x);
    }
    walk_steps(x);
    walk_delay(x);
    x.set8(Boss::ANIM_KIND, 2);
    x.add8(Enemy::SUB_STEP, 2);
    x.set16(Enemy::AX, 0);
    x.set16(Enemy::AY, 0);
    if !pick_target(x) {
        return wait(x);
    }
    lib::approach_target(x);
    speed(x, 0xd);
    let last = x.u8(Boss::ANIM_LAST);
    if last == 2 {
        let kind = x.u8(Boss::ANIM_KIND);
        x.set8(Boss::ANIM_LAST, kind);
        return select_walk(x, false);
    }
    let kind = x.u8(Boss::ANIM_KIND);
    x.set8(Boss::ANIM_LAST, kind);
    let n = if facing_forward(x) { 2 } else { 3 };
    body(x, n);
}

/// Picks the walk animation from the key frame (0x103e0); `mirror`: the direction test is inverted.
fn select_walk<R: Bench>(x: &mut Cx<'_, R>, mirror: bool) {
    let z = lib::moving_forward(x);
    let set = if z != mirror { WALK_A.0 } else { WALK_B.0 };
    lib::switch_keeping_time(x, set);
}

fn keep_walking<R: Bench>(x: &mut Cx<'_, R>) {
    if sub8(x, Boss::WALK_DELAY, 1).mi() {
        if !sub8(x, Boss::WALK_STEPS, 1).pl() {
            return next_action(x);
        }
        lib::approach_target(x);
        speed(x, 0xd);
        walk_delay(x);
        return select_walk(x, true);
    }
    if probe(x, -0xc0, 0x40, 0xc) {
        return x.set32(f::STATE, SHOOT_FAR);
    }
    if probe(x, -0x40, 0x30, 6) {
        return x.set32(f::STATE, SHOOT_NEAR);
    }
    if lib::target_in_box(x, -0x10, 0x10, 8) {
        return x.set32(f::STATE, SHOOT_FAR);
    }
    if needs_turn(x) {
        return interrupt(x);
    }
    x.move_by_table_xy();
    x.anim_step();
}

fn walk_delay<R: Bench>(x: &mut Cx<'_, R>) {
    let v = per_rank(x, "boss_10/walk_delays");
    x.set8(Boss::WALK_DELAY, v);
}

fn walk_steps<R: Bench>(x: &mut Cx<'_, R>) {
    let r = (x.random() as u32) & 0x1f;
    let v = byte(x, "boss_10/walk_steps", r);
    x.set8(Boss::WALK_STEPS, v);
}

/// +6 = 6: jump onto the target.
fn jump<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            x.add8(Enemy::SUB_STEP, 2);
            x.set8(Boss::ANIM_KIND, 5);
            let px = var::SCROLL_X.get(x.b);
            let py = var::SCROLL_Y.get(x.b);
            x.set16(Boss10::JUMP_X, px);
            x.set16(Boss10::JUMP_Y, py);
            let down = (((x.random() as u32) & 3) << 4) as u16 + 0x30;
            let y = x.u16(Boss10::JUMP_Y).wrapping_add(down);
            x.set16(Boss10::JUMP_Y, y);
            let across = (((x.random() as u32) & 7) << 4) as u16;
            let xx = x.u16(Boss10::JUMP_X).wrapping_add(across);
            x.set16(Boss10::JUMP_X, xx);
            *x.c.d2() = *x.c.d2() & 0xffff_0000 | 0xc0;
            let found = lib::pick_target_any(x);
            // Original glitch: if nobody is found, the gap is the D2.w that 0x43ae leaves behind: the depth of
            // l'objet
            let mut d2 = x.u16(crate::objects::Obj::Y);
            *x.c.d2() = *x.c.d2() & 0xffff_0000 | d2 as u32;
            if found {
                d2 = 0x30;
                let target = x.linked(f::TARGET);
                let edge = var::SCROLL_X.get(x.b).wrapping_add(0xc0);
                let tx = target.u16(x.b, Obj::X);
                if !(edge <= tx) {
                    d2 = 0xe0;
                }
            }
            let xx = x.u16(Boss10::JUMP_X).wrapping_add(d2);
            x.set16(Boss10::JUMP_X, xx);
            body(x, 7);
            lib::sound(x, crate::sound::CMD_331);
        }
        2 => {
            if x.anim_step() {
                x.add8(Enemy::SUB_STEP, 2);
                let (jx, jy) = (x.u16(Boss10::JUMP_X), x.u16(Boss10::JUMP_Y));
                x.set16(Obj::X, jx);
                x.set16(Obj::Y, jy);
                x.set8(f::FLIP, 0);
                if (x.random() as u32) & 1 != 0 {
                    x.set8(f::FLIP, 0x20);
                }
                body(x, 8);
                lib::sound(x, crate::sound::CMD_331);
            } else if x.u8(f::KEY_FRAME) != 0 {
                x.set8(f::KEY_FRAME, 0);
                x.set8(Enemy::PALETTE, 0x16);
            }
        }
        4 => {
            if x.anim_step() {
                x.set32(f::STATE, FIGHT);
                body(x, 0);
            } else if x.u8(f::KEY_FRAME) != 0 {
                x.set8(f::KEY_FRAME, 0);
                x.set8(Enemy::PALETTE, 0x18);
            }
        }
        _ => {}
    }
}

// --- +5 = 4: shot ----------------------------------------------------------------------------------

fn shoot<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => match x.u8(Enemy::SUB_STEP) {
            0 => {
                x.add8(Enemy::SUB_STEP, 2);
                x.anim_start(SET_SHOT.0, 0);
                lib::sound(x, crate::sound::CMD_5A);
            }
            2 => {
                if x.anim_step() {
                    next_action(x);
                }
            }
            _ => {}
        },
        2 => cinematic_shot(x),
        _ => {}
    }
}

/// 0x8d0a0: end of the shot.
fn shot_end<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Enemy::SUB_STEP, 2);
    var::CAMERA_MODE.set(x.b, 0);
    x.anim_start(SET_SHOT.0, 0);
}

fn cinematic_shot<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            x.add8(Enemy::SUB_STEP, 2);
            var::CAMERA_MODE.set(x.b, 4);
            var::INPUT_MASK.set(x.b, 0x7f);
            x.set8(Enemy::ACTIVE, 2);
            x.anim_start(SET_SHOT.0, 1);
            lib::sound(x, crate::sound::CMD_5A);
        }
        2 => {
            x.anim_step();
            if x.u8(f::KEY_FRAME) == 0 {
                return;
            }
            x.add8(Enemy::SUB_STEP, 2);
            x.set16(Boss::TIMER, 0xa);
            x.set8(f::KEY_FRAME, 0);
            let Some(shot) = x.alloc(&pools::ESHL) else { return shot_end(x) };
            shot.set_u8(x.b, Obj::ACTIVE, 1);
            shot.set_u8(x.b, Obj::KIND, 0x21);
            shot.set_u16(x.b, ShotB::HOST, x.o.0 as u16);
            let side = x.u8(f::FLIP);
            shot.set_u8(x.b, f::FLIP, side ^ 0x20);
            for off in [Obj::X, Obj::Z, Obj::Y] {
                let v = x.u16(off);
                shot.set_u16(x.b, off, v);
            }
            let dx: i16 = if side == 0 { -0x4c } else { 0x4c };
            let px = shot.u16(x.b, Obj::X).wrapping_add(dx as u16);
            let pz = shot.u16(x.b, Obj::Z).wrapping_add(0x20);
            shot.set_u16(x.b, Obj::X, px);
            shot.set_u16(x.b, Obj::Z, pz);
            x.set16(Boss10::SHOT, shot.0 as u16);
        }
        4 => {
            if sub16(x, Boss::TIMER, 1).gt() {
                x.anim_step();
                return;
            }
            x.add8(Enemy::SUB_STEP, 2);
            x.set16(Boss::TIMER, 0x20);
            let shot = x.linked(Boss10::SHOT);
            shot.set_u8(x.b, Shot33::FIRST, 1);
            lib::sound(x, crate::sound::CMD_5E);
            x.anim_step();
        }
        6 => {
            if sub16(x, Boss::TIMER, 1).gt() {
                x.anim_step();
                return;
            }
            x.add8(Enemy::SUB_STEP, 2);
            let shot = x.linked(Boss10::SHOT);
            shot.set_u8(x.b, Shot33::SECOND, 1);
            x.anim_step();
        }
        8 => {
            let shot = x.linked(Boss10::SHOT);
            if shot.u8(x.b, Shot33::DONE) != 0 {
                return shot_end(x);
            }
            x.anim_step();
        }
        10 => {
            if x.anim_step() {
                var::INPUT_MASK.set(x.b, 0xff);
                x.set8(Enemy::ACTIVE, 1);
                next_action(x);
            }
        }
        _ => {}
    }
}

// --- +5 = 6: waiting --------------------------------------------------------------------------

fn wait_state<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB2) == 0 {
        x.add8(f::SUB2, 2);
        x.set8(Boss10::WAIT_BLINK, 0xfa);
        x.anim_start(SET_BODY.0, 6);
        return lib::sound(x, crate::sound::CMD_5D);
    }
    if var::PLAYERS_ALIVE.get(x.b) != 0 {
        lib::pick_target_any(x);
        return next_action(x);
    }
    if !sub8(x, Boss10::WAIT_BLINK, 1).eq() {
        x.anim_step();
        return;
    }
    x.set8(Boss10::WAIT_BLINK, 0xfa);
    lib::sound(x, crate::sound::CMD_5D);
    x.anim_step();
}

// --- state 4: hit --------------------------------------------------------------------------------

fn hit<R: Bench>(x: &mut Cx<'_, R>) {
    if tst8(x, f::FREEZE).gt() {
        sub8(x, f::FREEZE, 1);
    } else {
        match x.u8(f::SUB) {
            0 | 2 => reaction(x),
            4 => get_up(x),
            6 | 8 => down(x),
            _ => {}
        }
    }
    x.set8(Enemy::VULNERABLE, 0);
    lib::collide_terrain(x);
}

/// 0x8d57a: no HP left? Then goes to group 6 (death).
fn out_of_hp<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if tst16(x, f::HP).gt() {
        return false;
    }
    x.set8(f::SUB, 6);
    x.set8(Enemy::ACTIVE, 2);
    true
}

fn reaction<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Enemy::PALETTE, 0x18);
    if x.u8(f::SUB2) == 0 {
        let flags = var::INPUT_MASK.get(x.b) | 0x80;
        var::INPUT_MASK.set(x.b, flags);
        if out_of_hp(x) {
            return down(x);
        }
    }
    if reaction_routine(x, REACTIONS.0) == STUN_REACTION.0 {
        return stun(x);
    }
    react(x, REACTIONS.0);
}

/// 0x8d294: stun.
fn stun<R: Bench>(x: &mut Cx<'_, R>) {
    sub16(x, Boss10::HITS, 1);
    match x.u8(f::SUB2) {
        0 => {
            let px = x.u16(Obj::X);
            x.set16(Enemy::X_SAVED, px);
            sub16(x, Boss10::HITS, 0xa);
            x.set8(Enemy::ACTIVE, 2);
            x.add8(f::SUB2, 2);
            let stun = x.u8(Enemy::STUN);
            x.set8(f::FREEZE, stun);
            x.set8(Boss::SHAKE, 0x10);
            x.anim_start(SET_DIE.0, 8);
            lib::sound(x, crate::sound::CMD_5B);
        }
        2 => {
            if x.u8(Boss::SHAKE) == 0 {
                x.add8(f::SUB2, 2);
                let px = x.u16(Enemy::X_SAVED);
                x.set16(Obj::X, px);
                return;
            }
            sub8(x, Boss::SHAKE, 1);
            let mut d = 2i16;
            if x.u8(Boss::SHAKE) & 1 != 0 {
                d = -d;
            }
            if x.u8(f::FLIP) != 0 {
                d = -d;
            }
            x.add16(Obj::X, d as u16);
        }
        4 => {
            x.set32(Enemy::HIT_LOCK, 0);
            if !tst16(x, Boss10::HITS).gt() {
                x.set8(Enemy::ACTIVE, 2);
            }
            x.set32(Enemy::AX_LONG, 0);
            x.set32(f::STATE, 0x0404_0200);
            x.set16(Boss10::HITS, 1);
            x.anim_step();
        }
        _ => {}
    }
}

fn get_up<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.anim_start(SET_BODY.0, 9);
        }
        2 => {
            if var::PAUSE.get(x.b) != 0 || sub16(x, Boss10::HITS, 1).gt() {
                return;
            }
            x.set8(Enemy::ACTIVE, 2);
            x.add8(f::SUB2, 2);
            x.set16(Boss10::HITS, 0);
            x.set8(Enemy::DOWNED, 0);
            x.set8(Enemy::REACTION, 0);
        }
        4 => {
            if var::PAUSE.get(x.b) != 0 {
                return;
            }
            if x.anim_step() {
                x.set32(f::STATE, 0x0202_0600);
                x.set8(Enemy::ACTIVE, 1);
                x.anim_start(SET_BODY.0, 7);
            }
        }
        _ => {}
    }
}

fn down<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(Enemy::ACTIVE, 2);
            var::PLAYERS_FROZEN.set(x.b, 1);
            var::BOSS_FIGHT.set(x.b, 0);
            let kind = x.u8(Enemy::KIND);
            record_defeat(x, kind);
            x.set32(f::IMMUNITY, 0x0fff_ffff);
            lib::flash_and_shake(x);
            crate::sound::service(x.b, crate::sound::Service::Ff05);
            lib::sound(x, crate::sound::CMD_FF);
            x.set16(Enemy::AX, 0);
            x.set16(Enemy::VZ, 0x500);
            x.set16(Enemy::GRAVITY, (-0x38i16) as u16);
            let mut vx: i16 = 0x200;
            let mut side = 0;
            let edge = var::CAMERA1_X.get(x.b).wrapping_add(0xc0);
            if !Fl::sub16(edge, x.u16(Obj::X)).ge() {
                vx = -vx;
                side = 0x20;
            }
            x.set16(Enemy::VX, vx as u16);
            x.set8(f::FLIP, side);
            x.anim_start(SET_DIE.0, 0x15);
            lib::sound(x, crate::sound::CMD_5C);
        }
        2 => {
            x.move_xz();
            if cmp16(x, Enemy::VZ, 0x100).le() {
                x.add8(f::SUB2, 2);
                x.anim_start(SET_DIE.0, 0x16);
            } else {
                x.anim_step();
            }
        }
        4 => {
            x.move_xz();
            if cmp16(x, Enemy::VZ, 0xff00).le() {
                x.add8(f::SUB2, 2);
                x.anim_start(SET_DIE.0, 0x17);
            }
        }
        6 => {
            x.move_xz();
            if tst16(x, Obj::Z).gt() {
                return;
            }
            x.add8(f::SUB2, 2);
            x.set16(Obj::Z, 0);
            lib::sound(x, crate::sound::CMD_AA);
            x.anim_start(SET_DIE.0, 0x18);
        }
        8 => {
            if x.anim_step() {
                x.add8(f::SUB2, 2);
            }
        }
        _ => {}
    }
}

handlers! {
    0x8ca4a "boss_10" run,
}
