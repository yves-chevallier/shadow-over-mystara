//! Boss of type 9 (0x8a75c): the rolling war machine of stage 2 (h), with its crew. See
//! `docs/families/boss.md`.
//!
//! The vehicle rolls from one edge of the screen to the other (camera plane `A5+0x556`); its parts (ESHL 0xd and 0xe)
//! and its crew (ENEMY 0x13: a driver `+0xac` and four gunners `+0xae..+0xb4`) are separate objects. It has no
//! combat AI: it passes by, stops, fires (its parts do the work, `+0xa8` / `+0xa9`: requests and
//! damage), and the lost crew is replaced. Hits received only shake it; once destroyed, it leaves the screen
//! and the stage goes on.
//!
//! | State | `+5` | File | Role |
//! |---|---|---|---|
//! | 0 | | `mod` | waits for room in the ESHL and ENEMY pools, then creates parts and driver |
//! | 2 | 0 | `moves` | stage entrance (dialogue, crossing the screen) |
//! | 2 | 2, 4 | `moves` | passes from one edge to the other, stops, shots |
//! | 2 | 6 | `moves` | wait while no player is alive |
//! | 4 | 0 | `hit` | hit received: shake, then resumption |
//! | 4 | 2 | `hit` | destroyed: exit from the screen, dialogue, fall |
//!
//! Animations: 0x17a276.

use crate::families::cx::Bench;
use crate::data::RomAddr;
use crate::record::record;
use crate::objects::Current;
use super::boss_common::*;
use super::enemy_dbeast::DisplacerBeast;
use crate::objects::Obj;

mod hit;
mod moves;

/// Fields of this boss beyond those of `Enemy` and `Boss` (`record!`). A meaning guessed from the code alone is
/// marked (h).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Boss9(pub Obj);

impl Boss9 {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Boss9 {
        /// Counter of the smoke puffs (`+0x42`: logical AND with a mask).
        puffs, set_puffs, PUFFS: u8 @ 0x42;
        /// Blinking after the loss of the crew: duration, in progress.
        blink_time, set_blink_time, BLINK_TIME: u8 @ 0x43;
        blinking, set_blinking, BLINKING: u8 @ 0x5c;
        /// Damage smoke: set once (`+0x5d`), after `+0x78` frames (word).
        smoke_done, set_smoke_done, SMOKE_DONE: u8 @ 0x5d;
        /// Dialogue bubble in progress (word, SET.0 object).
        message, set_message, MESSAGE: u16 @ 0x68;
        /// Passes remaining, wait before setting off again.
        passes, set_passes, PASSES: u8 @ 0x6b;
        wait, set_wait, WAIT: u8 @ 0x6e;
        smoke_delay, set_smoke_delay, SMOKE_DELAY: u16 @ 0x78;
        /// State to resume after a hit (long).
        resume, set_resume, RESUME: u32 @ 0x8a;
        /// Shake after a hit (frames).
        shake, set_shake, SHAKE: u8 @ 0xa0;
        /// Wheel pace (0 stopped, 1 fast, 2 at walking pace).
        gait, set_gait, GAIT: u8 @ 0xa1;
        /// Minuterie (mot).
        timer, set_timer, TIMER: u16 @ 0xa2;
        /// Damage (bits set by HP thresholds: 0x8b9ea), requests to the parts (bit 0 shot, 3 and 4 bombardment).
        damage, set_damage, DAMAGE: u8 @ 0xa8;
        orders, set_orders, ORDERS: u8 @ 0xa9;
        /// Crew to create (3 at the entrance), then 4 when there are no gunners left.
        crew_lost, set_crew_lost, CREW_LOST: u8 @ 0xaa;
        /// Rushes remaining.
        dashes, set_dashes, DASHES: u8 @ 0xab;
        /// Driver and gunners (words).
        driver, set_driver, DRIVER: u16 @ 0xac;
        crew, set_crew, CREW: u16 @ 0xae;
        /// Active gunners: bits, and number × 4.
        crew_bits, set_crew_bits, CREW_BITS: u8 @ 0xb6;
        crew_count, set_crew_count, CREW_COUNT: u8 @ 0xb7;
        /// X position aimed at by the stop (word).
        stop_x, set_stop_x, STOP_X: u16 @ 0xb8;
        /// Counters: bombardments (`+0xb9`), stops (`+0xba`), descent/climb (`+0xbe`), wait for the shot (`+0xbd`),
        /// crew before blinking (`+0xbf`).
        bombs, set_bombs, BOMBS: u8 @ 0xb9;
        stops, set_stops, STOPS: u8 @ 0xba;
        ready, set_ready, READY: u8 @ 0xbd;
        count, set_count, COUNT: u8 @ 0xbe;
        lives, set_lives, LIVES: u8 @ 0xbf;
        /// The two 0xd parts (words, `+0xa4` and `+0xa6`).
        parts, set_parts, PARTS: u16 @ 0xa4;
    }
}
use crate::objects::pools;

/// Jeu d'animations.
const SET: RomAddr = RomAddr(0x17a276);


/// One animation step (0x1a56).
fn anim<R: Bench>(x: &mut Cx<'_, R>) {
    x.anim_step();
}

/// Next step (`addq.b #2,(+6)` or `(+7)`).
fn next6<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB2, 2);
}
fn next7<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Enemy::SUB_STEP, 2);
}
fn steps<R: Bench>(x: &mut Cx<'_, R>, s6: u8, s7: u8) {
    x.set8(f::SUB2, s6);
    x.set8(Enemy::SUB_STEP, s7);
}

/// `bchg.b #1,(0xd,A0)` : l'engin cahote.
fn jolt<R: Bench>(x: &mut Cx<'_, R>) {
    let v = x.u8(Boss::HEIGHT_LOW) ^ 2;
    x.set8(Boss::HEIGHT_LOW, v);
}

/// One frame in `mask` + 1 (`+0x42`), true.
fn every<R: Bench>(x: &mut Cx<'_, R>, mask: u8) -> bool {
    let n = x.u8(Boss9::PUFFS).wrapping_add(1) & mask;
    x.set8(Boss9::PUFFS, n);
    n == 0
}

/// Two smoke puffs (0xb10c8): kind `kind`, offset `dx`.
fn smoke_pair<R: Bench>(x: &mut Cx<'_, R>, kind: u32, dx: u16) {
    for (dy, dz) in [(0x30, 0x28), (0x28, 8)] {
        lib::shock(x, kind, dx, dy, dz);
    }
}

/// 0x8ca22: smoke of the rolling vehicle.
fn smoke<R: Bench>(x: &mut Cx<'_, R>) {
    smoke_pair(x, 1, 0x100)
}

/// A smoke puff every `mask` + 1 frames.
fn smoke_every<R: Bench>(x: &mut Cx<'_, R>, mask: u8) {
    if every(x, mask) {
        smoke(x);
    }
}

/// Tighter smoke when the crew is reduced (more than two gunners: one frame in 4, otherwise in 8).
fn crew_smoke<R: Bench>(x: &mut Cx<'_, R>) {
    let mask = if (x.u8(Boss9::CREW_COUNT) as i8) > 8 { 3 } else { 7 };
    smoke_every(x, mask);
}

/// Has the vehicle left through the edge it is rolling toward (`+0x2c`: 0 toward the left)?
fn past_edge<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let cam = var::CAMERA_ALT_X.get(x.b);
    if x.u8(f::FLIP) == 0 {
        left_of(x, 0x80, cam)
    } else {
        right_of(x, 0x80, cam.wrapping_add(0x180))
    }
}

/// `x + d - edge` negative (`sub.w ; bge`): the vehicle is to the left of the edge.
fn left_of<R: Bench>(x: &mut Cx<'_, R>, d: u16, edge: u16) -> bool {
    let a = x.u16(Obj::X).wrapping_add(d);
    Fl::sub16(a, edge).lt()
}

/// `x - d - edge` positive (`sub.w ; ble`): the vehicle is to the right of the edge.
fn right_of<R: Bench>(x: &mut Cx<'_, R>, d: u16, edge: u16) -> bool {
    let a = x.u16(Obj::X).wrapping_sub(d);
    Fl::sub16(a, edge).gt()
}

/// No gunners for long enough (`+0xbf` negative): the vehicle blinks, and sets off on a pass.
fn crew_gone<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if (x.u8(Boss9::LIVES) as i8) >= 0 {
        return false;
    }
    steps(x, 0, 2);
    x.set8(Enemy::ACTIVE, 2);
    x.set8(Boss9::BLINK_TIME, 0x3c);
    x.set8(Boss9::BLINKING, 1);
    reset_lives(x);
    true
}

/// End of a stop: blinking if the crew is lost, otherwise a new pass (`+6` = 2, `+7` = 4).
fn stop_over<R: Bench>(x: &mut Cx<'_, R>) {
    if !crew_gone(x) {
        steps(x, 2, 4);
    }
}

// --- tables by difficulty (ROM data) ----------------------------------------------------

fn difficulty<R: Bench>(x: &mut Cx<'_, R>) -> u32 {
    var::RANK.get(x.b) as u32
}

/// Byte drawn (`mask`) from the difficulty's table, whose address is in the table of longs `lists`.
fn draw<R: Bench>(x: &mut Cx<'_, R>, lists: &str, mask: u32) -> u8 {
    let d = difficulty(x);
    let at = long(x, lists, 4 * d);
    let r = rand_and(x, mask);
    tables::held8(x.b, at + r)
}

/// 0x8c9ee: crew lost before blinking.
fn reset_lives<R: Bench>(x: &mut Cx<'_, R>) {
    let n = per_rank(x, "boss_9/lives").wrapping_sub(1);
    x.set8(Boss9::LIVES, n);
}

/// 0x8c9ba: wait before the shot.
fn reset_count<R: Bench>(x: &mut Cx<'_, R>) {
    let n = per_rank(x, "boss_9/shot_waits").wrapping_add(5);
    x.set8(Boss9::COUNT, n);
}

/// 0x8c420: rushes allowed.
fn reset_dashes<R: Bench>(x: &mut Cx<'_, R>) {
    let n = per_rank(x, "boss_9/dashes");
    x.set8(Boss9::DASHES, n);
}

/// 0x8beca: passes before a stop.
fn draw_passes<R: Bench>(x: &mut Cx<'_, R>) {
    let n = draw(x, "boss_9/passes", 0x1f).wrapping_sub(1);
    x.set8(Boss9::PASSES, n);
}

/// 0x8c450 : bombardements.
fn draw_bombs<R: Bench>(x: &mut Cx<'_, R>) {
    let n = draw(x, "boss_9/bombs", 0xf).wrapping_sub(1);
    x.set8(Boss9::BOMBS, n);
}

/// 0x8c576: stops.
fn draw_stops<R: Bench>(x: &mut Cx<'_, R>) {
    let n = draw(x, "boss_9/stops", 0xf);
    x.set8(Boss9::STOPS, n);
}

/// 0x8bc0c: wait before setting off again, according to the number of players, the difficulty and the crew.
fn reset_wait<R: Bench>(x: &mut Cx<'_, R>) {
    let players = var::PLAYERS_ALIVE.get(x.b) as u32;
    let list = long(x, "boss_9/waits", 4 * players);
    let d = difficulty(x);
    let mut n = tables::held8(x.b, list + d).wrapping_add(0xf);
    if (x.u8(Boss9::CREW_COUNT) as i8) >= 8 {
        n = n.wrapping_add(0xf);
    }
    x.set8(Boss9::WAIT, n);
}

/// 0x8bcd2: X position of the stop, drawn in front of the vehicle.
fn draw_stop_x<R: Bench>(x: &mut Cx<'_, R>) {
    let d = difficulty(x);
    let list = long(x, "boss_9/stop_xs", 4 * d);
    let r = rand_and(x, 0x1f);
    let k = tables::held8(x.b, list + r) as u16;
    let cam = var::CAMERA_ALT_X.get(x.b);
    let goal = if x.u8(f::FLIP) != 0 { cam.wrapping_add(k) } else { cam.wrapping_add(0x180).wrapping_sub(k) };
    x.set16(Boss9::STOP_X, goal);
}

/// 0x8be88: depth of the pass according to that of the target (three lanes).
fn pick_lane<R: Bench>(x: &mut Cx<'_, R>) {
    let lane: u32 = if x.u16(f::TARGET) == 0 {
        1
    } else {
        let t = load_target(x);
        let z = t.u16(x.b, Obj::Y) as i16;
        if z > 0x60 {
            0
        } else if z > 0x48 {
            1
        } else {
            2
        }
    };
    let z = word(x, "boss_9/lanes", 2 * lane);
    x.set16(Obj::Y, z);
}

/// 0x8c988: new target (0x424e or 0x44ee, at random).
fn retarget<R: Bench>(x: &mut Cx<'_, R>) {
    let r = rand_and(x, 0xf);
    let k = byte(x, "boss_9/retargets", r);
    if k == 0 {
        lib::pick_target(x);
    } else {
        lib::random_player(x);
    }
}

/// 0x8bff0: counts the active gunners (state 0x0200): bits in `+0xb6`, number × 4 in `+0xb7`; none: `+0xaa` = 4.
fn count_crew<R: Bench>(x: &mut Cx<'_, R>) {
    let (mut bits, mut count) = (0u8, 0u8);
    for k in 0..4u32 {
        let link = x.u16(Boss9::CREW + 2 * k);
        let m = Obj(link as i16 as i32 as u32 & 0xff_ffff);
        if m.u16(x.b, Enemy::STATE_WORD) == 0x0200 {
            count = count.wrapping_add(4);
            bits |= 1 << k;
        }
    }
    *x.c.d3() = bits as u32; // re-read: D3 re-read afterwards (DDSOM_POISON : covb_long3p_02)
    *x.c.d4() = 4;
    *x.c.d5() = 0xffff;
    x.set8(Boss9::CREW_BITS, bits);
    x.set8(Boss9::CREW_COUNT, count);
    if count == 0 {
        x.set8(Boss9::CREW_LOST, 4);
    }
}

/// 0x8c026: rolling speed according to the crew (fast table in hard mode), wheel pace.
fn set_speed<R: Bench>(x: &mut Cx<'_, R>) {
    let hard = var::DAMAGE_SETTING.get(x.b) != 0;
    let mut top = if hard { 0x800u16 } else { 0x680 };
    if x.u8(f::FLIP) == 0 {
        top = top.wrapping_neg();
    }
    if top == x.u16(Enemy::VX) {
        return;
    }
    let i = x.u8(Boss9::CREW_COUNT) as u32 * 2;
    let table = if hard { "boss_9/speeds_hard" } else { "boss_9/speeds" };
    let mut vx = word(x, table, i);
    let mut vz = word(x, table, 2 + i);
    let gait = byte(x, table, 5 + i);
    *x.c.d2() = gait as u32;
    if x.u8(f::FLIP) == 0 {
        vx = vx.wrapping_neg();
        vz = vz.wrapping_neg();
    }
    x.set8(Boss9::GAIT, gait);
    x.set16(Enemy::VX, vx);
    x.set16(Enemy::AX, vz);
}

/// 0x8c1a0: rush (full crew, draw by players and difficulty): full speed; true if launched.
fn try_dash<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if x.u8(Boss9::CREW_COUNT) != 0x10 || x.u8(Boss9::DASHES) == 0 {
        return false;
    }
    let players = var::PLAYERS_ALIVE.get(x.b) as u32;
    let list = long(x, "boss_9/dash_chances", 4 * players);
    let d = difficulty(x);
    let chances = tables::held32(x.b, list + 4 * d);
    *x.c.d2() = chances;
    let r = rand_and(x, 0x1f);
    if chances >> r & 1 == 0 {
        return false;
    }
    x.set8(Boss9::GAIT, 1);
    let mut v = if var::DAMAGE_SETTING.get(x.b) != 0 { 0x800u16 } else { 0x680 };
    if x.u8(f::FLIP) == 0 {
        v = v.wrapping_neg();
    }
    x.set16(Enemy::VX, v);
    x.add8(Boss9::DASHES, 0xff);
    true
}

/// 0x8c124 / 0x8c132: replaces the lost gunners (ENEMY 0x13, variant 2 with no gunner at all, 1 otherwise).
fn refill_crew<R: Bench>(x: &mut Cx<'_, R>, all: bool) {
    if all {
        if x.u8(Boss9::CREW_COUNT) == 0x10 {
            return;
        }
    } else if (pools::ENEMY.free_count(x.b) as i16) < 4 || (x.u8(Boss9::CREW_COUNT) as i8) > 4 {
        return;
    }
    let mut slot = 5u8;
    for k in 0..4u32 {
        *x.c.d4() = k;
        if x.u8(Boss9::CREW_BITS) >> k & 1 == 0 {
            x.set16(Boss9::CREW + 2 * k, 0);
            if let Some(m) = x.alloc(&pools::ENEMY) {
                let variant = if x.u8(Boss9::CREW_COUNT) == 0 { 2 } else { 1 };
                m.set_u8(x.b, Enemy::VARIANT, variant);
                m.set_u8(x.b, Enemy::ACTIVE, 1);
                DisplacerBeast(m).set_part(x.b, slot);
                m.set_u8(x.b, Enemy::KIND, 0x13);
                m.set_u16(x.b, Enemy::CREATOR, x.o.0 as u16);
                x.set16(Boss9::CREW + 2 * k, m.0 as u16);
            }
        }
        slot = slot.wrapping_sub(1);
    }
    *x.c.d2() = 0xffff;
    *x.c.d4() = 4;
    *x.c.d5() = slot as u32;
}

/// 0x8c8d2: a target on the ground in front of the vehicle? 1 very close, 0 within firing range, 2 otherwise.
fn target_range<R: Bench>(x: &mut Cx<'_, R>) -> u8 {
    if x.u16(f::TARGET) == 0 {
        2
    } else {
        let t = load_target(x);
        if t.u16(x.b, Obj::Z) != 0 {
            2
        } else if lib::target_in_box(x, -0x40, 0x40, 0x100) {
            1
        } else if lib::target_in_box(x, -0x100, 0x80, 0x100) {
            0
        } else {
            2
        }
    }
}

/// 0x8c698 / 0x8c6a0: firing stop drawn by difficulty (`base`: 2 close target, 0xe target in range).
fn draw_halt<R: Bench>(x: &mut Cx<'_, R>, base: u8) {
    *x.c.d2() = base as u32;
    let k = rank_byte(x, "boss_9/halts").wrapping_add(base);
    x.set8(f::SUB2, k);
    x.set16(f::STATE, 0x0204);
    x.set8(Enemy::SUB_STEP, 0);
}

/// 0x8b9ea: damage by eighths of HP (bits of `+0xa8`, cumulative).
fn damage_bits<R: Bench>(x: &mut Cx<'_, R>) {
    let q = x.u16(Enemy::HP_MAX) >> 3;
    let hp = x.u16(f::HP) as i16;
    let mut stage = 7u8;
    let mut d1 = q.wrapping_mul(6);
    for k in 0..7u8 {
        if (d1 as i16) < hp {
            stage = k;
            break;
        }
        if k < 6 {
            d1 = d1.wrapping_sub(q);
        }
    }
    const MASKS: [u8; 8] = [0, 0x40, 0xc0, 0xc4, 0xcc, 0xdc, 0xdd, 0xdd];
    let v = x.u8(Boss9::DAMAGE) | MASKS[stage as usize];
    x.set8(Boss9::DAMAGE, v);
}

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        st::INIT => init(x),
        st::ACTIVE => active(x),
        st::HIT => hit::run(x),
        _ => x.free(&pools::BOSS),
    }
}

// --- state 0: creation --------------------------------------------------------------------------

/// 0x8a772: outside cutscene, and as soon as there is room (10 ESHL, 5 ENEMY): parts, driver, position.
fn init<R: Bench>(x: &mut Cx<'_, R>) {
    if var::PAUSE.get(x.b) != 0 {
        return;
    }
    let r = x.random() as u32;
    x.set8(Boss9::PUFFS, r as u8);
    if (pools::ESHL.free_count(x.b) as i16) < 0xa || (pools::ENEMY.free_count(x.b) as i16) < 5 {
        return;
    }
    x.add8(f::STATE, 2);
    var::BOSS_FIGHT.set(x.b, 1);
    var::SCENE_ACTOR.set(x.b, x.o.0 as u16);
    lib::clear_popup(x);
    lib::empty_popups(x);
    lib::clear_all_popups(x);
    lib::create_set_15(x);
    x.set8(Boss::DEATH_CREDIT, 1);
    x.set8(Boss::DEATH_VALUE, 1);
    x.set8(Enemy::AI_SPARE_ROW, 2);
    x.set8(f::BOX_SET, 0x29);
    lib::initial_hp_boss(x);
    lib::set_immunities(x);
    // two 0xd parts (linked by `+0xa4`, `+0xa6`), eight 0xe parts
    for (kind, last, linked) in [(0xdu8, 1u8, true), (0xe, 7, false)] {
        for k in (0..=last).rev() {
            *x.c.d5() = k as u32;
            let Some(p) = x.alloc(&pools::ESHL) else { break };
            let active = x.u8(Enemy::ACTIVE);
            p.set_u8(x.b, Enemy::ACTIVE, active);
            p.set_u8(x.b, Enemy::VARIANT, k);
            p.set_u8(x.b, Enemy::KIND, kind);
            p.set_u16(x.b, f::OWNER, x.o.0 as u16);
            if linked {
                x.set16(Boss9::PARTS + 2 * (last - k) as u32, p.0 as u16);
            }
        }
    }
    if let Some(d) = x.alloc(&pools::ENEMY) {
        let active = x.u8(Enemy::ACTIVE);
        d.set_u8(x.b, Enemy::ACTIVE, active);
        DisplacerBeast(d).set_part(x.b, 6);
        d.set_u8(x.b, Enemy::VARIANT, 1);
        d.set_u8(x.b, Enemy::KIND, 0x13);
        d.set_u16(x.b, Enemy::CREATOR, x.o.0 as u16);
        x.set16(Boss9::DRIVER, d.0 as u16);
    }
    x.set8(Enemy::EVENT_KIND, 0x21);
    x.set16(Boss9::SMOKE_DELAY, 0x300);
    for off in [Boss9::SMOKE_DONE, Boss9::DAMAGE, Boss9::ORDERS, Boss9::CREW_LOST, Enemy::FRAME_TAG, Enemy::VULNERABLE] {
        x.set8(off, 0u8);
    }
    x.set8(Boss9::GAIT, 2);
    x.set8(Enemy::ACTIVE, 2);
    x.set8(f::FLIP, 0x20);
    x.set8(Enemy::HEADING, 8);
    x.set16(Obj::Y, 0x48);
    // `move.w #-0x8000,D3 ; swap ; asr.l #16`: minus half a unit
    *x.c.d3() = 0xffff_8000;
    let z = x.u32(Obj::Y).wrapping_add(0xffff_8000);
    x.set32(Obj::Y, z);
    x.set16(Obj::X, 0x80);
    lib::pick_target_any(x);
    x.set32(Obj::Z, 0);
    x.anim_start(SET.0, 6);
}

// --- state 2: active -----------------------------------------------------------------------------

/// 0x8a8c0: freeze, blinking, one group per frame, shadow; with no target or player: wait.
fn active<R: Bench>(x: &mut Cx<'_, R>) {
    let freeze = x.u8(f::FREEZE);
    if freeze != 0 && (freeze as i8) > 0 {
        x.set8(f::FREEZE, freeze - 1);
    } else {
        if freeze != 0 {
            x.set8(f::FREEZE, 0);
        }
        if var::PAUSE.get(x.b) != 0 {
            return;
        }
        if x.u8(Boss9::BLINKING) != 0 && sub8(x, Boss9::BLINK_TIME, 1).mi() {
            x.set8(Enemy::ACTIVE, 1);
            x.set8(Boss9::BLINKING, 0);
            x.set8(Boss9::BLINK_TIME, 0);
        }
        match x.u8(f::SUB) {
            0 => moves::intro(x),
            2 => moves::start(x),
            4 => moves::fight(x),
            _ => moves::wait_for_players(x),
        }
        lib::collide_terrain(x);
    }
    let rolling = x.u8(Boss9::GAIT) != 2;
    x.set8(Boss::SHADOW_B, rolling as u8);
    if x.u16(f::TARGET) == 0 || var::PLAYERS_ALIVE.get(x.b) == 0 {
        retarget(x);
        x.set32(f::STATE, 0x0206_0000);
    }
    x.reinsert();
}

handlers! {
    0x8a75c "boss_9" run,
}

use super::cx::handlers;
