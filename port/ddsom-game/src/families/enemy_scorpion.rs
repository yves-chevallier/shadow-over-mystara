//! Enemy "GiantScorpion" (ENEMY type 10, 0x6681c). See `docs/families/enemy.md`.
//!
//! Variants `+3`: 0 standing, 1 and 4 fall from above, 2 and 3 enter flying through an edge, 5 is the nest that spawns
//! scorpions (list `+0x2f`) and waits to be visible. The arrival `+6` values (2, 4, 6, 8) choose the entrance, then the
//! active state:
//!
//! | `+5` | Role (h) |
//! |---|---|
//! | 0 | arrival (fall, flight, dormant nest) |
//! | 2 | standing: walks back and forth (`+6` = 0 free, 2 child of a parent, 4 free child) |
//! | 4, 10 | randomly drawn hits (`+6` = 2, 4, 6, 8; tables by difficulty) |
//! | 6 | wait of a randomly drawn duration |
//! | 8 | special hit: about-turn then charge with attack `0x1f` |
//!
//! The hit state has its own recoil (no common reaction table). Animations 0x163da8 (movements), 0x163db8 (charge) and
//! 0x163dba (hit).

use crate::families::cx::Bench;
use crate::record::record;
use crate::data::RomAddr;
use super::cx::{handlers, Cx};
use super::enemy_common::*;
use crate::objects::Obj;

const ANIMS: RomAddr = RomAddr(0x163da8);
const ANIMS_STRIKE: RomAddr = RomAddr(0x163db8);
const ANIMS_HIT: RomAddr = RomAddr(0x163dba);

/// Fields specific to this type.
/// Fields of the family beyond the common ones of `Enemy` (`record!`). A meaning guessed from the code alone is
/// marked (h).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Scorpion(pub Obj);

impl Scorpion {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Scorpion {
        timer, set_timer, TIMER: u8 @ 0xa2;
        start_x, set_start_x, START_X: u16 @ 0xa4;
        move_kind, set_move_kind, MOVE: u8 @ 0xa6;
        frozen, set_frozen, FROZEN: u8 @ 0xa7;
        exit_timer, set_exit_timer, EXIT_TIMER: u8 @ 0xa8;
    }
}
use crate::objects::pools;
use crate::objects::Current;

/// Tables of `data/enemies.json`: draws by difficulty (rows of 16 bytes per four ranks, or of 32 bytes per two), gesture
/// durations and speeds.
const ATTACK_PICK_A: &str = "scorpion/attacks_a";
const ATTACK_PICK_B: &str = "scorpion/attacks_b";
const MOVE_PICK: &str = "scorpion/moves";
const IDLE_TIMES: &str = "scorpion/idle_times";
const ATTACK_TABLE: &str = "scorpion/attack_states";
const TIMER_ROW: &str = "scorpion/move_times";
const RETREAT_ROW: &str = "scorpion/retreat_ranks";
const STRIKE_TIMER: &str = "scorpion/strike_times";
const SPEED_ROW: &str = "scorpion/move_speeds";

const STANCE: u32 = 0x0202_0000;
const REACH: u32 = 0x0208_0000;
const HELD: u32 = 0x0404_0000;
const DEAD: u32 = 0x0600_0000;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    match x.u8(f::STATE) {
        st::INIT => init(x),
        st::ACTIVE => active(x),
        st::HIT => hit(x),
        st::DEAD => x.free(&pools::ENEMY),
        _ => {}
    }
}

// --- helpers ------------------------------------------------------------------------------------

fn anim_play<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.anim_start(ANIMS.0, n as u16);
}

/// Starting height of a fall: the object appears above the screen.
fn drop_from_above<R: Bench>(x: &mut Cx<'_, R>) {
    let cam = var::SCROLL_Y.get(x.b).wrapping_add(0x100);
    let depth = x.u16(Obj::Y);
    x.set16(Obj::Z, cam.wrapping_sub(depth));
    x.set16(Obj::VZ, 0);
    x.set16(Obj::AZ, -0x40i16 as u16);
}

/// Camera abscissa according to the mode (`A5+0x36`: second screen).
fn camera_x<R: Bench>(x: &mut Cx<'_, R>) -> u16 {
    if var::CAMERA_SELECT.get(x.b) != 0 { var::CAMERA_ALT_X.get(x.b) } else { var::SCROLL_X.get(x.b) }
}

/// 0x67808: advances in X and in depth according to the speeds (8.8 fixed point).
fn advance<R: Bench>(x: &mut Cx<'_, R>) {
    for (pos, speed) in [(Obj::X, Obj::VX), (Obj::Y, Obj::VY)] {
        let d = ((x.u16(speed) as u32) << 16) as i32 >> 8;
        x.add32(pos, d as u32);
    }
}

/// 0x6771a: chooses the gesture (`+0xa6`) in the difficulty row.
fn pick_move<R: Bench>(x: &mut Cx<'_, R>) {
    let row = ((var::RANK.get(x.b) >> 2) << 4) as u32;
    let r = rand_and(x, 0xf);
    let v = tables::table(MOVE_PICK).at8(x.b, row + r);
    x.set8(Scorpion::MOVE, v);
}

/// 0x677e8: duration of the gesture (`+0xa2`) according to the rank `d0` and the gesture.
fn set_timer<R: Bench>(x: &mut Cx<'_, R>, d0: u32) {
    let idx = ((d0 << 2) & !0xff) | (((d0 << 2) as u8).wrapping_add(x.u8(Scorpion::MOVE)) as u32);
    let t = tables::table(TIMER_ROW).at8(x.b, idx);
    x.set8(Scorpion::TIMER, t);
}

/// 0x677de: randomly drawn duration (rank 0 to 3).
fn random_timer<R: Bench>(x: &mut Cx<'_, R>) {
    let r = rand_and(x, 3);
    set_timer(x, r);
}

/// 0x670d2: speed table of the gesture.
fn speed_row<R: Bench>(x: &mut Cx<'_, R>) {
    let idx = x.u8(Scorpion::MOVE) as u32;
    let d0 = tables::table(SPEED_ROW).at8(x.b, idx);
    x.select_speed(&crate::objects::motion::SPEEDS_B, (d0 as u32) as u16);
}

/// Choice of the next hit in the difficulty row: draws `+6` = 2 + byte of the table.
fn pick_attack<R: Bench>(x: &mut Cx<'_, R>, table: &str) {
    x.add8(f::SUB2, 2);
    let row = ((var::RANK.get(x.b) >> 2) << 4) as u32;
    let r = rand_and(x, 0xf);
    let v = tables::table(table).at8(x.b, row + r);
    x.add8(f::SUB2, v);
    pick_move(x);
}

// --- state 0: creation --------------------------------------------------------------------------

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    if paused(x) {
        return;
    }
    x.add8(f::STATE, 2);
    x.set8(f::BOX_SET, 0x16);
    lib::initial_hp(x);
    lib::set_immunities(x);
    for (off, v) in [
        (Enemy::AI_STANCE_ROW, 0),
        (Enemy::EVENT_KIND, 4),
        (Enemy::EVENT_LOW, 0),
        (Enemy::AI_WANDER_ROW, 2),
        (Enemy::AI_WAIT_MASKS, 3),
        (Enemy::AI_BOX_CHANCE, 3),
        (Enemy::AI_BYTE_ROW, 3),
        (Enemy::AI_69, 2),
        (Enemy::AI_SHOT, 3),
        (Enemy::AI_ROW_8E, 0),
        (Scorpion::EXIT_TIMER, 0),
        (Enemy::AI_WAIT_ROW, 2),
        (Enemy::AI_DECISION_A, 4),
        (Enemy::AI_DECISION_B, 3),
        (Enemy::AI_68, 2),
        (f::SUB, 0),
    ] {
        x.set8(off, v);
    }
    x.set16(f::SUB2, 0);
    let d0 = 2 * x.u8(Obj::VARIANT) as u32;
    match x.u8(Obj::VARIANT) {
        0 => {
            x.set8(f::SUB, 2);
            anim_play(x, 0);
        }
        1 => {
            x.set8(f::SUB2, d0 as u8);
            drop_from_above(x);
            anim_play(x, 5);
        }
        2 | 3 => {
            x.set8(f::SUB2, d0 as u8);
            let mut shift = 0x190u16;
            let cam = var::SCROLL_X.get(x.b);
            x.set16(Obj::X, cam);
            x.set16(Obj::VZ, 0x400);
            x.set16(Obj::AZ, -0x40i16 as u16);
            x.set16(Obj::VX, -0x400i16 as u16);
            x.set16(Obj::AX, 0);
            if x.u8(f::FLIP) != 0 {
                shift = -0x10i16 as u16;
                negate(x, Obj::VX);
            }
            x.add16(Obj::X, shift);
            anim_play(x, 6);
        }
        4 => {
            x.set8(f::SUB2, 6);
            drop_from_above(x);
            anim_play(x, 5);
        }
        5 => {
            x.set8(f::SUB2, 8);
            anim_play(x, 0);
            let list = x.u8(Enemy::GROUP) as u32;
            spawn_list(x, "scorpion", list);
        }
        _ => {}
    }
}

// --- state 2: active ----------------------------------------------------------------------------

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    let freeze = x.u8(f::FREEZE) as i8;
    if freeze > 0 {
        x.set8(Scorpion::FROZEN, 1);
        x.set8(f::FREEZE, (freeze - 1) as u8);
    } else if !paused(x) {
        match x.u8(f::SUB) {
            0 => arrival(x),
            2 => stance(x),
            4 => melee(x),
            6 => idle(x),
            8 => charge(x),
            10 => melee_b(x),
            _ => {}
        }
    }
    if x.u8(f::SUB) != 0 {
        zone_and_terrain(x);
    }
    x.reinsert();
}

/// Substate 0: the arrival depends on `+6` (set at creation according to the variant).
fn arrival<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        2 => land(x, |x| {
            x.add8(f::SUB, 2);
            x.set16(f::SUB2, 0);
        }),
        4 => fly_in(x),
        6 => land(x, |x| {
            x.add8(f::SUB, 2);
            x.set8(f::SUB2, 2);
        }),
        8 => {
            let dx = x.u16(Obj::X).wrapping_sub(var::SCROLL_X.get(x.b)).wrapping_add(0x20);
            if dx <= 0x1c0 {
                x.set8(f::SUB, 2);
                x.set8(f::SUB2, 4);
            }
        }
        _ => {}
    }
}

/// Fall: as long as the altitude is positive; on landing `done` changes the substate.
fn land<'a, R: Bench>(x: &mut Cx<'a, R>, done: impl FnOnce(&mut Cx<'a, R>)) {
    x.move_xz();
    if (x.u16(Obj::Z) as i16) > 0 {
        return;
    }
    x.set32(Obj::Z, 0);
    done(x);
}

/// Variant 2: flying entrance (animation 6), landing (animation 7), resumption of the walk.
fn fly_in<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 | 4 => {
            x.anim_step();
            if (x.u16(Enemy::ANIM_END) as i16) < 0 {
                if x.u8(f::SUB3) == 0 {
                    x.add8(f::SUB3, 2);
                } else {
                    x.add8(f::SUB, 2);
                    x.set16(f::SUB2, 0);
                }
            }
        }
        2 => {
            x.move_xz();
            if (x.u16(Obj::Z) as i16) >= 0 {
                return;
            }
            x.set32(Obj::Z, 0);
            x.add8(f::SUB3, 2);
            anim_play(x, 7);
        }
        _ => {}
    }
}

/// Substate 2: standing. `+6` = 0 free, 2 child of a parent (`+0x5c`), 4 free child.
fn stance<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => free_stance(x),
        2 => {
            let parent = x.linked(Enemy::SPAWNER);
            if parent.u8(x.b, Enemy::STATE) == 4 {
                return set_state_long(x, HELD);
            }
            walk(x);
        }
        4 => walk(x),
        _ => {}
    }
}

fn free_stance<R: Bench>(x: &mut Cx<'_, R>) {
    if grabbed(x) {
        return set_state_long(x, HELD);
    }
    if find_player(x, -0x28, 0x18, 8) {
        return set_state_long(x, REACH);
    }
    if find_player(x, -8, 0x48, 0x10) {
        return choose_attack(x);
    }
    walk(x);
}

/// 0x674ca: draws the next attack substate in the difficulty row.
fn choose_attack<R: Bench>(x: &mut Cx<'_, R>) {
    let v = rank_byte(x, ATTACK_TABLE);
    x.set8(f::SUB, v);
    x.add8(f::SUB, 2);
    x.set8(f::STATE, 2);
    x.set16(f::SUB2, 0);
}

/// Walk: forward (animation 1), about-turn (4), back.
fn walk<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            x.set8(Scorpion::TIMER, 0x28);
            let px = x.u16(Obj::X);
            x.set16(Scorpion::START_X, px);
            x.set8(Obj::SPEED_INDEX, 0x18);
            if x.u8(f::FLIP) != 0 {
                x.set8(Obj::SPEED_INDEX, 8);
            }
            x.select_speed(&crate::objects::motion::SPEEDS_B, 0xd);
            anim_play(x, 1);
        }
        2 => {
            let d = ((x.u16(Obj::VX) as u32) << 16) as i32 >> 8;
            x.add32(Obj::X, d as u32);
            let n = x.u8(Scorpion::TIMER).wrapping_sub(1);
            x.set8(Scorpion::TIMER, n);
            if n != 0 {
                x.anim_step();
                return;
            }
            negate(x, Obj::VX);
            x.add8(f::SUB3, 2);
            anim_play(x, 4);
        }
        4 => {
            x.anim_step();
            if (x.u8(Enemy::ANIM_END) as i8) < 0 {
                x.set8(f::SUB3, 2);
                let flip = x.u8(f::FLIP) ^ 0x20;
                x.set8(f::FLIP, flip);
                x.set8(Scorpion::TIMER, 0x28);
                anim_play(x, 1);
            } else {
                x.anim_step();
            }
        }
        _ => {}
    }
}

// --- substates 4 and 10: hits ------------------------------------------------------------------

/// Substate 4: `+6` = 0 draw, 2 and 4 attack movements, 6 about-turn then charge.
fn melee<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB2) == 0 {
        pick_attack(x, ATTACK_PICK_A);
    }
    match x.u8(f::SUB2) {
        2 => {
            if x.u8(f::SUB3) != 0 {
                return attack_tail(x);
            }
            x.add8(f::SUB3, 2);
            face(x, true);
            random_timer(x);
            attack_move(x);
        }
        4 => {
            if x.u8(f::SUB3) != 0 {
                return attack_tail(x);
            }
            x.add8(f::SUB3, 2);
            let r = rand_and(x, 0xf);
            let d0 = tables::table(RETREAT_ROW).at8(x.b, r) as u32;
            set_timer(x, d0);
            face(x, false);
            attack_move(x);
        }
        6 => turn_and_charge(x),
        _ => {}
    }
}

/// Substate 10: same draw with another table; `+6` = 2, 4, 6 movements, 8 pause.
fn melee_b<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB2) == 0 {
        pick_attack(x, ATTACK_PICK_B);
    }
    match x.u8(f::SUB2) {
        2 => {
            if x.u8(f::SUB3) != 0 {
                return attack_tail(x);
            }
            x.add8(f::SUB3, 2);
            set_timer(x, 1);
            let r = rand_and(x, 1);
            x.set8(Obj::SPEED_INDEX, (r << 4) as u8);
            attack_move(x);
        }
        4 | 6 => {
            if x.u8(f::SUB3) != 0 {
                return attack_tail(x);
            }
            x.add8(f::SUB3, 2);
            let rank = if x.u8(f::SUB2) == 4 { 1 } else { 2 };
            set_timer(x, rank);
            face(x, false);
            attack_move(x);
        }
        8 => {
            if x.u8(f::SUB3) != 0 {
                let n = x.u8(Scorpion::TIMER).wrapping_sub(1);
                x.set8(Scorpion::TIMER, n);
                if n == 0 {
                    set_state_long(x, STANCE);
                }
                x.anim_step();
                return;
            }
            x.add8(f::SUB3, 2);
            anim_play(x, 0);
        }
        _ => {}
    }
}

/// Walking direction of a gesture: 8 or 0x18 according to the orientation; `forward` to advance toward the side faced.
fn face<R: Bench>(x: &mut Cx<'_, R>, forward: bool) {
    let flipped = x.u8(f::FLIP) != 0;
    x.set8(Obj::SPEED_INDEX, if flipped == forward { 8 } else { 0x18 });
}

/// 0x66f00: speed and animation of the gesture, then common continuation.
fn attack_move<R: Bench>(x: &mut Cx<'_, R>) {
    speed_row(x);
    let n = 1 + x.u8(Scorpion::MOVE) as u32;
    anim_play(x, n);
    attack_tail(x);
}

/// 0x66f1c: advance, countdown and end of the gesture.
fn attack_tail<R: Bench>(x: &mut Cx<'_, R>) {
    advance(x);
    let n = x.u8(Scorpion::TIMER).wrapping_sub(1);
    x.set8(Scorpion::TIMER, n);
    if n == 0 {
        set_state_long(x, STANCE);
    }
    x.anim_step();
}

/// Substate 4, `+6` = 6: turns toward the target, then launches the gesture.
fn turn_and_charge<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => face_target(x),
        2 => wait_turn(x),
        4 => {
            x.add8(f::SUB3, 2);
            random_timer(x);
            attack_move(x);
        }
        6 => attack_tail(x),
        _ => {}
    }
}

/// 0x67052: angle toward the target; if it is on the side faced, animation 4 and wait, otherwise step 4.
fn face_target<R: Bench>(x: &mut Cx<'_, R>) {
    let target = load_target(x);
    let angle = lib::aim_to(x, target);
    x.set8(Obj::SPEED_INDEX, angle);
    let side = ((angle as u16 & 0x10) * 2) as u8;
    if side != x.u8(f::FLIP) {
        x.set8(f::SUB3, 4);
        return;
    }
    x.add8(f::SUB3, 2);
    anim_play(x, 4);
}

/// 0x67088: at the end of the turn animation, the orientation is inverted.
fn wait_turn<R: Bench>(x: &mut Cx<'_, R>) {
    if (x.u16(Enemy::ANIM_END) as i16) < 0 {
        x.add8(f::SUB3, 2);
        let flip = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
    }
    x.anim_step();
}

// --- substate 6: wait --------------------------------------------------------------------------

fn idle<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB2) == 0 {
        x.add8(f::SUB2, 2);
        let r = rand_and(x, 0xf);
        let t = tables::table(IDLE_TIMES).at8(x.b, r);
        x.set8(Enemy::AI_WAIT, t);
        return anim_play(x, 0);
    }
    let n = x.u8(Enemy::AI_WAIT).wrapping_sub(1);
    x.set8(Enemy::AI_WAIT, n);
    if n != 0 {
        x.anim_step();
    } else {
        set_state_long(x, STANCE);
    }
}

// --- substate 8: special hit ------------------------------------------------------------------

fn charge<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => face_target(x),
        2 => wait_turn(x),
        4 => {
            x.add8(f::SUB3, 2);
            x.set8(Enemy::STRIKE_KIND, 0x1f);
            x.set8(Enemy::STRIKE_POWER, 2);
            pick_move(x);
            x.anim_start(ANIMS_STRIKE.0, 0);
            x.set8(Scorpion::FROZEN, 0);
            let m = x.u8(Scorpion::MOVE) as u32;
            let t = tables::table(STRIKE_TIMER).at8(x.b, m);
            x.set8(Scorpion::TIMER, t);
            speed_row(x);
        }
        6 => {
            advance(x);
            x.anim_step();
            let n = x.u8(Scorpion::TIMER).wrapping_sub(1);
            x.set8(Scorpion::TIMER, n);
            if n == 0 {
                set_state_long(x, STANCE);
            }
        }
        _ => {}
    }
}

// --- state 4: hit -------------------------------------------------------------------------------

fn hit<R: Bench>(x: &mut Cx<'_, R>) {
    let freeze = x.u8(f::FREEZE) as i8;
    if freeze > 0 {
        x.set8(f::FREEZE, (freeze - 1) as u8);
    } else {
        match x.u8(f::SUB) {
            0 => knock_back(x),
            2 => recoil(x),
            4 => grabbed_flight(x),
            _ => {}
        }
    }
    x.reinsert();
}

fn knock_back<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB, 2);
    let freeze = x.u8(Enemy::STUN);
    x.set8(f::FREEZE, freeze);
    x.set16(Obj::VZ, 0x400);
    x.set16(Obj::AZ, -0x40i16 as u16);
    x.set16(Obj::VX, 0x200);
    x.set16(Obj::AX, 0);
    let flip = x.u8(Enemy::ATTACKER_FLIP) ^ 0x20;
    x.set8(f::FLIP, flip);
    if flip != 0 {
        negate(x, Obj::VX);
        negate(x, Obj::AX);
    }
    x.set8(Enemy::ACTIVE, 2);
    x.anim_start(ANIMS_HIT.0, 0);
    zone_and_terrain(x);
}

fn recoil<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.move_xz();
            if x.u8(Enemy::BLOCKED) != 0 {
                x.set16(Obj::VX, 0);
                x.set16(Obj::AX, 0);
            }
            if (x.u16(Obj::Z) as i16) <= 0 {
                x.add8(f::SUB2, 2);
                x.set32(Obj::Z, 0);
                x.set8(Scorpion::TIMER, 0x28);
                x.set16(Obj::ANIM_TIMER, 1);
                detach_from_host(x);
            }
            x.anim_step();
        }
        2 => {
            let n = x.u8(Scorpion::TIMER).wrapping_sub(1);
            x.set8(Scorpion::TIMER, n);
            if n == 0 {
                set_state_long(x, DEAD);
                lib::object_event(x);
            } else {
                x.set_bit(Enemy::DISPLAY, 1, n & 3 != 0);
                x.anim_step();
            }
        }
        _ => {}
    }
    zone_and_terrain(x);
}

/// Substate 4: grabbed by a player, the scorpion runs toward the closest edge of the screen.
fn grabbed_flight<R: Bench>(x: &mut Cx<'_, R>) {
    if var::PAUSE.get(x.b) != 0 {
        return;
    }
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            let cam = camera_x(x);
            x.set16(Obj::VX, 0x300);
            let mut want = 0x20;
            let dx = x.u16(Obj::X).wrapping_sub(cam);
            if (dx as i16) < 0xc0 {
                negate(x, Obj::VX);
                want = 0;
            }
            if want != x.u8(f::FLIP) {
                return anim_play(x, 4);
            }
            run_off(x);
        }
        2 => {
            if x.anim_step() {
                let flip = x.u8(f::FLIP) ^ 0x20;
                x.set8(f::FLIP, flip);
                run_off(x);
            }
        }
        4 => {
            x.move_x();
            if !x.in_arena_default() {
                detach_from_host(x);
                set_state_long(x, DEAD);
            }
            if x.u8(Scorpion::EXIT_TIMER) != 0 {
                let n = x.u8(Scorpion::EXIT_TIMER).wrapping_sub(1);
                x.set8(Scorpion::EXIT_TIMER, n);
                zone_and_terrain(x);
            }
            x.anim_step();
        }
        _ => {}
    }
}

/// 0x673e4: the scorpion runs (animation 2) for at most 0xff frames.
fn run_off<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB2, 2);
    x.set8(Scorpion::EXIT_TIMER, 0xff);
    anim_play(x, 2);
}

handlers! {
    0x6681c "enemy_10" run,
}
