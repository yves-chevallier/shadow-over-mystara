//! "Troglodyte" enemy (ENEMY type 2, 0x59baa). See `docs/families/enemy.md`.
//!
//! Common framework: `+4` state (0 creation, 2 active, 4 hit, 6 release), `+5` group of behaviours, `+6` and `+7`
//! steps. In the active state:
//!
//! | `+5` | File | Role (h) |
//! |---|---|---|
//! | 0 | `mod` | arrival according to variant `+3` |
//! | 2 | `stance` | standing: walk, turn-around, wait, choice of an attack |
//! | 4 | `attacks` | melee attacks and leaps (`+6` = the attack) |
//! | 6 | `attacks` | dodge and guard leaps |
//! | 8 | `attacks` | fallback attacks |
//! | 10 | `stance` | rest: waits for a player to be alive |
//! | state 4 | `hit` | reactions to hits |
//!
//! Animations: 0x15faa0 (movements) and 0x15fabe (attacks). Series 0 (`+0x2f`) blinks after a hit taken: `flicker`
//! (counters `+0xa5..+0xa8`, `+0xaa`, `+0xae`).

use crate::families::cx::Bench;
use super::cx::{handlers, Cx};
use super::enemy_common::*;
use crate::objects::Obj;

mod attacks;
mod hit;
mod stance;

/// Fields specific to this type (the common fields are in `enemy_common::f`).
pub(self) mod g {
    /// Series: 0 troglodyte (blinks), 1 armed variant.
    pub const SERIES: u32 = 0x2f;
    pub const STRIKE_KIND: u32 = 0x44;
    pub const STRIKE_POWER: u32 = 0x45;
    pub const VULNERABLE: u32 = 0x67;
    pub const POSTURE: u32 = 0x68;
    /// AI counters: delay before a new choice, wait, walk.
    pub const AI_CHOICE: u32 = 0x6b;
    pub const AI_WAIT: u32 = 0x6e;
    pub const AI_WALK: u32 = 0x71;
    pub const COOLDOWN: u32 = 0x77;
    pub const AI_KIND: u32 = 0x78;
    pub const AI_SHOT: u32 = 0x79;
    /// Current and previous look (tint byte read by the display).
    pub const LOOK: u32 = 0xa0;
    pub const LOOK_PREV: u32 = 0xa1;
    /// Repetitions or timer of a step.
    pub const REPEAT: u32 = 0xa2;
    /// Attack to resume after an announcement or a turn-around.
    pub const RESUME: u32 = 0xa3;
    /// Blinking: phase (`+0xa5`, 0xff = at rest), alternation, wait, counters.
    pub const BLINK_PHASE: u32 = 0xa5;
    pub const BLINK_ON: u32 = 0xa6;
    pub const BLINK_WAIT: u32 = 0xa7;
    pub const BLINK_COUNT: u32 = 0xa8;
    pub const BLINK_DELAY: u32 = 0xaa;
    pub const BLINK_REST: u32 = 0xae;
    /// Non-zero byte: the hit state does not draw the shadow.
    pub const NO_SHADOW: u32 = 0xb0;
}

use g::*;
use crate::objects::Current;

/// Animation sets: movements and attacks.
const SET_BODY: u32 = 0x15faa0;
const SET_STRIKE: u32 = 0x15fabe;
/// Long states (`+4..+7`).
const STANCE: u32 = 0x0202_0000;
const IDLE: u32 = 0x020a_0000;
/// ROM tables: attacks to choose per difficulty level (`A5+0x7f92`), 32 bytes per level.
const FALLBACK_MOVES: u32 = 0x5bba6;
const CLOSE_ATTACKS: u32 = 0x5b074;
const CLOSE_ATTACKS_ARMED: u32 = 0x5b274;
const REPEATS_TABLE: u32 = 0x5b4d6;
const JUMP_MOVES: u32 = 0x5b6f8;
const BLINK_DELAYS: u32 = 0x5b974;
const BLINK_TIMES: u32 = 0x5b96c;
/// Walking speeds per series (2 words per series).
const WALK_SPEEDS: u32 = 0x5af64;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    x.tick_counters();
    match x.u8(f::STATE) {
        st::INIT => init(x),
        st::ACTIVE => active(x),
        st::HIT => hit::run(x),
        st::DEAD => dead(x),
        _ => {}
    }
}

// --- aides ------------------------------------------------------------------------------------

fn body<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.anim_start(SET_BODY, n as u16);
}

fn strike<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.anim_start(SET_STRIKE, n as u16);
}

fn step<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    x.anim_step()
}

/// Walking direction according to the orientation: `flipped` if `+0x2c` is non-zero, `otherwise` if not.
fn set_angle<R: Bench>(x: &mut Cx<'_, R>, flipped: u8, otherwise: u8) {
    let angle = if x.u8(f::FLIP) != 0 { flipped } else { otherwise };
    x.set8(Obj::SPEED_INDEX, angle);
}

fn move_xy<R: Bench>(x: &mut Cx<'_, R>) {
    x.move_by_table_xy();
}

/// Row of 32 bytes of the difficulty in `table` (`A5+0x7f92`).
fn difficulty_row<R: Bench>(x: &mut Cx<'_, R>) -> u32 {
    (((var::RANK.get(x.b) >> 1) << 5) as u32) & 0xffff
}

/// Byte drawn at random from the difficulty row of `table` (A4 = row, D0 = index).
fn pick_byte<R: Bench>(x: &mut Cx<'_, R>, table: u32) -> u8 {
    let row = difficulty_row(x);
    let n = rand_and(x, 0x1f);
    x.data8(table + row + n)
}

/// 0x5af4e: chooses the speed table according to the series (`idx`: 0, 2 or 4).
fn walk_speed<R: Bench>(x: &mut Cx<'_, R>, idx: u32) {
    let series = x.u8(SERIES) as u32;
    let n = x.data16(WALK_SPEEDS + ((series << 2) & 0xff) + idx);
    x.select_speed(&crate::objects::motion::SPEEDS_B, n);
}

/// A player in the box? Both mask decisions (0xa4c2, 0xa4b4) must accept.
fn box_attack<R: Bench>(x: &mut Cx<'_, R>, d0: i16, d1: u16, d2: u16) -> bool {
    let ok = lib::target_in_box(x, d0, d1, d2) && {
        load_target(x);
        lib::decision_b(x)
    } && {
        lib::decision_a(x)
    };
    ok
}

/// 0x5afa0: target in front, at short range.
fn front_box<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    box_attack(x, 0, 0x70, 8)
}

/// 0x5af90: target behind or far in front.
fn side_box<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    box_attack(x, -0xb5, 0x35, 0x15)
}

/// 0x5afda: the target is on the other side of the orientation.
fn needs_turn_around<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if x.u16(f::TARGET) == 0 {
        return false;
    }
    let target = load_target(x);
    let dx = x.u16(Obj::X).wrapping_sub(target.u16(x.b, Obj::X));
    let want = if (dx as i16) < 0 { 0x20 } else { 0 };
    let turn = want != x.u8(f::FLIP) as u32;
    turn
}

/// 0x5af78: back to standing; if decision 0x79ce wants it, to rest.
fn back_to_stance<R: Bench>(x: &mut Cx<'_, R>) {
    set_state_long(x, STANCE);
    if lib::maybe_draw_6e(x) {
        x.set8(f::SUB, 0xa);
    }
}

/// End of the attacks: the next one is drawn from the difficulty row (0x5bb74).
fn end_attack<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(AI_KIND, 0xff);
    let row = difficulty_row(x);
    let n = rand_and(x, 0x1f);
    x.set8(f::SUB, 8);
    let mv = x.data8(FALLBACK_MOVES + row + n);
    x.set8(f::SUB2, mv);
    x.set8(f::SUB3, 0);
}

/// Blinking is started if it is not already (series 0, positive phase).
fn blink_if_running<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(SERIES) == 0 && (x.u8(BLINK_PHASE) as i8) > 0 {
        x.set8(BLINK_ON, 1);
        x.set16(BLINK_COUNT, 0xc);
        x.set8(BLINK_WAIT, 0);
    }
}

// --- state 0: creation --------------------------------------------------------------------------

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    if paused(x) {
        return;
    }
    x.add8(f::STATE, 2);
    x.set8(f::BOX_SET, 0xe);
    lib::initial_hp(x);
    lib::set_immunities(x);
    for (off, v) in [(0x80, 0xb), (0x68, 4), (0x69, 4), (0x70, 0), (0x6a, 3), (0x6c, 3), (0x6d, 5), (0x72, 4), (0x73, 3), (0x74, 3), (0x75, 0x2c), (BLINK_PHASE, 0xff)] {
        x.set8(off, v);
    }
    x.set16(BLINK_REST, 0x258);
    x.set8(BLINK_ON, 0);
    if x.u8(SERIES) != 0 {
        x.add8(0x75, 4);
        x.set8(BLINK_PHASE, 0xff);
    }
    for (off, v) in [(0x76, 4), (REPEAT, 0x20), (LOOK_PREV, 2), (VULNERABLE, 1), (0x8e, 0), (0x7f, 0), (0x7e, 0xf)] {
        x.set8(off, v);
    }
    x.set32(0x8a, 0);
    x.set8(AI_KIND, 0xff);
    x.set8(0x81, 0xa);
    x.set8(AI_SHOT, 0x14);
    if pick_target(x) {
        // creation chains directly into the active state
        active(x);
    } else {
        set_state_long(x, IDLE);
    }
}

// --- state 2: active -----------------------------------------------------------------------------

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    let freeze = x.u8(f::FREEZE) as i8;
    let dispatch = if freeze > 0 {
        x.set8(f::FREEZE, (freeze - 1) as u8);
        true
    } else {
        // the paused game prevents the behaviour (but not the shadow)
        !paused(x)
    };
    if !dispatch {
        if x.u8(f::SUB) != 0 {
            zone_and_terrain(x);
        }
        return x.reinsert();
    }
    match x.u8(f::SUB) {
        0 => arrival(x),
        2 => stance::run(x),
        4 => attacks::run(x),
        6 => attacks::run_b(x),
        8 => attacks::run_fallback(x),
        10 => stance::rest(x),
        _ => {}
    }
    if x.u8(f::SUB) != 0 {
        zone_and_terrain(x);
    }
    if x.u8(SERIES) == 0 {
        flicker(x);
    }
    x.reinsert();
}

/// Blinking of series 0 (0x59d02): `+0xa5` is the phase (0xff: at rest, countdown `+0xae`; 0: starts the blinking;
/// positive: alternates visibility `+1` bit 1).
fn flicker<R: Bench>(x: &mut Cx<'_, R>) {
    let phase = x.u8(BLINK_PHASE) as i8;
    if phase == 0 {
        x.add8(BLINK_PHASE, 1);
        x.set8(BLINK_WAIT, 1);
        x.set8(BLINK_ON, 1);
        x.set16(BLINK_REST, 0x258);
        x.set16(BLINK_COUNT, 0xc);
        return pick_blink_delay(x);
    }
    if phase < 0 {
        let n = x.u16(BLINK_REST).wrapping_sub(1);
        x.set16(BLINK_REST, n);
        return;
    }
    if x.u8(BLINK_ON) != 0 {
        let v = x.u8(1) ^ 2;
        x.set8(1, v);
    } else if x.u8(BLINK_WAIT) != 0 {
        x.set_bit(1, 1, true);
        let n = x.u16(BLINK_DELAY).wrapping_sub(1);
        x.set16(BLINK_DELAY, n);
        if (n as i16) < 0 {
            x.set8(BLINK_WAIT, 0);
            x.set8(BLINK_ON, 1);
            x.set16(BLINK_COUNT, 0xc);
            x.set_bit(1, 1, false);
        }
    } else {
        x.set8(BLINK_PHASE, 0xff);
        x.set_bit(1, 1, false);
    }
}

/// 0x5b940: invisibility duration drawn at random.
fn pick_blink_delay<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(BLINK_WAIT, 1);
    let mut d2 = var::RANK.get(x.b);
    d2 = d2 & 0xff00 | (d2 as u8 >> 1) as u16;
    let row = (d2 << 5) as u32;
    *x.c.d2() = *x.c.d2() & 0xffff_0000 | row;
    let n = rand_and(x, 0x1f);
    let idx = x.data8(BLINK_DELAYS + row + n) as u32;
    let w = x.rom16(BLINK_TIMES + idx);
    x.set16(BLINK_DELAY, w);
}

/// Substate 0: arrival according to variant `+3` (0 walk, 1 fall, 2 leap, 3 and 4 jump, 5 wake-up).
fn arrival<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::VARIANT) {
        0 => {
            if x.u8(f::SUB2) != 0 {
                let t = x.u8(REPEAT).wrapping_sub(1);
                x.set8(REPEAT, t);
                if t == 0 {
                    x.set8(LOOK_PREV, 0);
                    set_state_long(x, STANCE);
                    x.set8(0, 1);
                }
                move_xy(x);
                step(x);
                return;
            }
            x.add8(f::SUB2, 2);
            x.set8(VULNERABLE, 1);
            x.set16(LOOK, 0);
            x.set8(REPEAT, 0x18);
            set_angle(x, 8, 0x18);
            x.set16(LOOK, 0);
            walk_speed(x, 0);
            body(x, 2);
        }
        1 => match x.u8(f::SUB2) {
            0 => {
                x.add8(f::SUB2, 2);
                x.set8(VULNERABLE, 1);
                x.set8(LOOK_PREV, 0xe);
                x.set16(Obj::VZ, -0x300i16 as u16);
                x.set16(Obj::AZ, -0x80i16 as u16);
                let cam = var::SCROLL_Y.get(x.b).wrapping_add(0x100).wrapping_sub(x.u16(Obj::Y));
                x.set16(Obj::Z, cam);
                body(x, 0xb);
            }
            2 => {
                x.move_z();
                if (x.u16(Obj::Z) as i16) < 0 {
                    x.add8(f::SUB2, 2);
                    x.set32(Obj::Z, 0);
                    body(x, 0xc);
                }
            }
            4 => end_arrival(x),
            _ => {}
        },
        2 => match x.u8(f::SUB2) {
            0 => {
                x.add8(f::SUB2, 2);
                x.set16(Obj::VX, 0);
                let t = load_target(x);
                if t.u8(x.b, 0) == 0 {
                    set_state_long(x, IDLE);
                    return rest_begin(x);
                }
                x.set16(Obj::VZ, 0x680);
                x.set16(Obj::AZ, -0x38i16 as u16);
                x.set16(Obj::AX, 0);
                let d = t.u16(x.b, Obj::X).wrapping_sub(x.u16(Obj::X)) << 2;
                x.set16(Obj::VX, d);
                body(x, 4);
            }
            2 => {
                x.move_xz();
                if step(x) && (x.u16(Obj::Z) as i16) < 0 {
                    x.set16(Obj::Z, 0);
                    x.add8(f::SUB2, 2);
                    body(x, 0xc);
                }
            }
            4 => end_arrival(x),
            _ => {}
        },
        3 | 4 => {
            if x.u8(f::SUB2) == 0 {
                let scene = var::SCENE.get(x.b);
                if scene == 6 || scene == 0xc {
                    lib::sound(x, 0xeb92e);
                } else {
                    lib::sound(x, 0xeb9a0);
                }
                x.add8(f::SUB2, 2);
                x.set16(Obj::VZ, 0x600);
                x.set16(Obj::AZ, -0x38i16 as u16);
                x.set16(Obj::VY, 0x80);
                x.set16(Obj::AY, 0);
                if x.u8(Obj::VARIANT) != 3 {
                    negate(x, Obj::VY);
                }
                body(x, 0xa);
            } else {
                step(x);
                if x.move_yz().pl() {
                    return;
                }
                x.set8(LOOK_PREV, 0xe0);
                x.set16(Obj::Z, 0);
                set_state_long(x, STANCE);
            }
        }
        5 => {
            if x.u8(f::SUB2) != 0 {
                end_arrival(x)
            } else {
                x.add8(f::SUB2, 2);
                body(x, 9);
            }
        }
        _ => {}
    }
}

/// 0x59f38: once the arrival animation is over, the troglodyte stands up.
fn end_arrival<R: Bench>(x: &mut Cx<'_, R>) {
    if step(x) {
        x.set8(LOOK_PREV, 0xe0);
        set_state_long(x, STANCE);
        x.set8(0, 1);
    }
}

/// 0x5a010: rest look and animation 0xe.
fn rest_begin<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB2, 2);
    x.set8(LOOK_PREV, 1);
    body(x, 0xe);
}

// --- state 6: release ------------------------------------------------------------------------

fn dead<R: Bench>(x: &mut Cx<'_, R>) {
    free(x);
}

handlers! {
    0x59baa "enemy_2" #[without] run,
}
