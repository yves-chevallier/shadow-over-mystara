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
use crate::record::record;
use crate::data::RomAddr;
use super::cx::{handlers, Cx};
use super::enemy_common::*;
use crate::objects::Obj;

mod attacks;
mod hit;
mod stance;

/// Fields specific to this type (the common fields are in `enemy_common::f`).
/// Fields of the family beyond the common ones of `Enemy` (`record!`). A meaning guessed from the code alone is
/// marked (h).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Trog(pub Obj);

impl Trog {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Trog {
        /// Current and previous look (tint byte read by the display).
        look, set_look, LOOK: u8 @ 0xa0;
        look_prev, set_look_prev, LOOK_PREV: u8 @ 0xa1;
        /// Repetitions or timer of a step.
        repeat, set_repeat, REPEAT: u8 @ 0xa2;
        /// Attack to resume after an announcement or a turn-around.
        resume, set_resume, RESUME: u8 @ 0xa3;
        /// Blinking: phase (`+0xa5`, 0xff = at rest), alternation, wait, counters.
        blink_phase, set_blink_phase, BLINK_PHASE: u8 @ 0xa5;
        blink_on, set_blink_on, BLINK_ON: u8 @ 0xa6;
        blink_wait, set_blink_wait, BLINK_WAIT: u8 @ 0xa7;
        blink_count, set_blink_count, BLINK_COUNT: u16 @ 0xa8;
        blink_delay, set_blink_delay, BLINK_DELAY: u16 @ 0xaa;
        blink_rest, set_blink_rest, BLINK_REST: u16 @ 0xae;
        /// Non-zero byte: the hit state does not draw the shadow.
        no_shadow, set_no_shadow, NO_SHADOW: u8 @ 0xb0;
    }
}

use crate::objects::Current;

/// Animation sets: movements and attacks.
const SET_BODY: RomAddr = RomAddr(0x15faa0);
const SET_STRIKE: RomAddr = RomAddr(0x15fabe);
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
    x.anim_start(SET_BODY.0, n as u16);
}

fn strike<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.anim_start(SET_STRIKE.0, n as u16);
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
    let series = x.u8(Enemy::GROUP) as u32;
    let n = x.data16(WALK_SPEEDS + ((series << 2) & 0xff) + idx);
    x.select_speed(&crate::objects::motion::SPEEDS_B, n);
}

/// A player in the box? Both mask decisions (0xa4c2, 0xa4b4) must accept.
fn box_attack<R: Bench>(x: &mut Cx<'_, R>, d0: i16, d1: u16, d2: u16) -> bool {
    lib::target_in_box(x, d0, d1, d2) && {
        load_target(x);
        lib::decision_b(x)
    } && {
        lib::decision_a(x)
    }
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
    want != x.u8(f::FLIP) as u32
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
    x.set8(Enemy::AI_SHOT, 0xff);
    let row = difficulty_row(x);
    let n = rand_and(x, 0x1f);
    x.set8(f::SUB, 8);
    let mv = x.data8(FALLBACK_MOVES + row + n);
    x.set8(f::SUB2, mv);
    x.set8(f::SUB3, 0);
}

/// Blinking is started if it is not already (series 0, positive phase).
fn blink_if_running<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Enemy::GROUP) == 0 && (x.u8(Trog::BLINK_PHASE) as i8) > 0 {
        x.set8(Trog::BLINK_ON, 1);
        x.set16(Trog::BLINK_COUNT, 0xc);
        x.set8(Trog::BLINK_WAIT, 0);
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
    for (off, v) in [(Enemy::LOOT_ROW, 0xb), (Enemy::AI_68, 4), (Enemy::AI_69, 4), (Enemy::AI_STANCE_ROW, 0), (Enemy::AI_WANDER_ROW, 3), (Enemy::AI_WAIT_MASKS, 3), (Enemy::AI_WAIT_ROW, 5), (Enemy::AI_DECISION_A, 4), (Enemy::AI_DECISION_B, 3), (Enemy::AI_BOX_CHANCE, 3), (Enemy::AI_BOX_ANIM, 0x2c), (Trog::BLINK_PHASE, 0xff)] {
        x.set8(off, v);
    }
    x.set16(Trog::BLINK_REST, 0x258);
    x.set8(Trog::BLINK_ON, 0);
    if x.u8(Enemy::GROUP) != 0 {
        x.add8(Enemy::AI_BOX_ANIM, 4);
        x.set8(Trog::BLINK_PHASE, 0xff);
    }
    for (off, v) in [(Enemy::AI_BYTE_ROW, 4), (Trog::REPEAT, 0x20), (Trog::LOOK_PREV, 2), (Enemy::VULNERABLE, 1), (Enemy::AI_ROW_8E, 0), (Enemy::EVENT_LOW, 0), (Enemy::EVENT_KIND, 0xf)] {
        x.set8(off, v);
    }
    x.set32(Enemy::RESUME_STATE, 0);
    x.set8(Enemy::AI_SHOT, 0xff);
    x.set8(Enemy::LOOT, 0xa);
    x.set8(Enemy::AI_POINT_SET, 0x14);
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
    if x.u8(Enemy::GROUP) == 0 {
        flicker(x);
    }
    x.reinsert();
}

/// Blinking of series 0 (0x59d02): `+0xa5` is the phase (0xff: at rest, countdown `+0xae`; 0: starts the blinking;
/// positive: alternates visibility `+1` bit 1).
fn flicker<R: Bench>(x: &mut Cx<'_, R>) {
    let phase = x.u8(Trog::BLINK_PHASE) as i8;
    if phase == 0 {
        x.add8(Trog::BLINK_PHASE, 1);
        x.set8(Trog::BLINK_WAIT, 1);
        x.set8(Trog::BLINK_ON, 1);
        x.set16(Trog::BLINK_REST, 0x258);
        x.set16(Trog::BLINK_COUNT, 0xc);
        return pick_blink_delay(x);
    }
    if phase < 0 {
        let n = x.u16(Trog::BLINK_REST).wrapping_sub(1);
        x.set16(Trog::BLINK_REST, n);
        return;
    }
    if x.u8(Trog::BLINK_ON) != 0 {
        let v = x.u8(Enemy::DISPLAY) ^ 2;
        x.set8(Enemy::DISPLAY, v);
    } else if x.u8(Trog::BLINK_WAIT) != 0 {
        x.set_bit(Enemy::DISPLAY, 1, true);
        let n = x.u16(Trog::BLINK_DELAY).wrapping_sub(1);
        x.set16(Trog::BLINK_DELAY, n);
        if (n as i16) < 0 {
            x.set8(Trog::BLINK_WAIT, 0);
            x.set8(Trog::BLINK_ON, 1);
            x.set16(Trog::BLINK_COUNT, 0xc);
            x.set_bit(Enemy::DISPLAY, 1, false);
        }
    } else {
        x.set8(Trog::BLINK_PHASE, 0xff);
        x.set_bit(Enemy::DISPLAY, 1, false);
    }
}

/// 0x5b940: invisibility duration drawn at random.
fn pick_blink_delay<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Trog::BLINK_WAIT, 1);
    let mut d2 = var::RANK.get(x.b);
    d2 = d2 & 0xff00 | (d2 as u8 >> 1) as u16;
    let row = (d2 << 5) as u32;
    *x.c.d2() = *x.c.d2() & 0xffff_0000 | row;
    let n = rand_and(x, 0x1f);
    let idx = x.data8(BLINK_DELAYS + row + n) as u32;
    let w = x.rom16(BLINK_TIMES + idx);
    x.set16(Trog::BLINK_DELAY, w);
}

/// Substate 0: arrival according to variant `+3` (0 walk, 1 fall, 2 leap, 3 and 4 jump, 5 wake-up).
fn arrival<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::VARIANT) {
        0 => {
            if x.u8(f::SUB2) != 0 {
                let t = x.u8(Trog::REPEAT).wrapping_sub(1);
                x.set8(Trog::REPEAT, t);
                if t == 0 {
                    x.set8(Trog::LOOK_PREV, 0);
                    set_state_long(x, STANCE);
                    x.set8(Enemy::ACTIVE, 1);
                }
                move_xy(x);
                step(x);
                return;
            }
            x.add8(f::SUB2, 2);
            x.set8(Enemy::VULNERABLE, 1);
            x.set16(Trog::LOOK, 0);
            x.set8(Trog::REPEAT, 0x18);
            set_angle(x, 8, 0x18);
            x.set16(Trog::LOOK, 0);
            walk_speed(x, 0);
            body(x, 2);
        }
        1 => match x.u8(f::SUB2) {
            0 => {
                x.add8(f::SUB2, 2);
                x.set8(Enemy::VULNERABLE, 1);
                x.set8(Trog::LOOK_PREV, 0xe);
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
                if t.u8(x.b, Enemy::ACTIVE) == 0 {
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
                    lib::sound(x, crate::sound::CMD_305_405);
                } else {
                    lib::sound(x, crate::sound::CMD_36F_46F);
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
                x.set8(Trog::LOOK_PREV, 0xe0);
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
        x.set8(Trog::LOOK_PREV, 0xe0);
        set_state_long(x, STANCE);
        x.set8(Enemy::ACTIVE, 1);
    }
}

/// 0x5a010: rest look and animation 0xe.
fn rest_begin<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB2, 2);
    x.set8(Trog::LOOK_PREV, 1);
    body(x, 0xe);
}

// --- state 6: release ------------------------------------------------------------------------

fn dead<R: Bench>(x: &mut Cx<'_, R>) {
    free(x);
}

handlers! {
    0x59baa "enemy_2" run,
}
