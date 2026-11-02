//! Boss of type 22 (0x9aeaa): the creature that the type 2 boss releases on its death (h), to which it hands over
//! (`+0x42` = its host). See `docs/families/boss.md`.
//!
//! Same template as type 16 (`boss_16`): even `+5` groups, steps `+6` and `+7`, states set in one go as longs, two
//! animation sets (body 0x18492a, attacks 0x184964). Its second form (`+0xb5` raised) has other animations and
//! other attacks. The reactions to hits are the common routines (`reactions.rs`).
//!
//! | `+5` | File | Role |
//! |---|---|---|
//! | 0 | `moves` | arrival: falls from the top of the screen |
//! | 2 | `moves` | combat: wait (`+6` = 0 and 8), about-face (2), leap toward the target (6), charge (4) |
//! | 4 | `attacks` | attacks: six hits (`+6` = 0 to 22) |
//! | 6 | `moves` | run to a point on the screen (`+6` = 0 and 2), jump (4 and 6) |
//! | 8 | `moves` | same after an attack |
//! | 10 | `moves` | pause |
//! | 12 | `moves` | a grabbed player (`+0x50`): bites |
//! | 14 | `moves` | transformation into the second form |
//! | state 4 | `hit` | reactions to hits, knockback, on the ground, death |

use crate::families::cx::Bench;
use super::boss_common::*;
use super::cx::handlers;

mod attacks;
mod hit;
mod moves;
mod two;

/// Fields specific to this type.
pub(self) mod g {
    pub const VULNERABLE: u32 = 0x67;
    pub const WALK_STEPS: u32 = 0x6b;
    pub const IDLE_COUNT: u32 = 0x6e;
    pub const WALK_DELAY: u32 = 0x71;
    /// Current animation (number) and last one started.
    pub const ANIM_ID: u32 = 0xa2;
    pub const ANIM_LAST: u32 = 0xa3;
    /// Timers (words, or bytes at the same address).
    pub const TIMER: u32 = 0xa0;
    pub const TIMER2: u32 = 0xaa;
    pub const SPOT_TIMER: u32 = 0xbc;
    /// State to resume after an about-face: long at `+0xa6` (states) and byte at `+0xa5` (step `+6`).
    pub const SAVED_STEP: u32 = 0xa5;
    pub const SAVED_STATE: u32 = 0xa6;
    /// Aimed point (words), rotation direction of the trajectory.
    pub const SPOT_X: u32 = 0xb0;
    pub const SPOT_Z: u32 = 0xb2;
    pub const SPIN: u32 = 0xb4;
    /// Seconde forme.
    pub const SECOND: u32 = 0xb5;
    pub const SECOND_FX: u32 = 0xbf;
    pub const EFFECT_COUNT: u32 = 0xb7;
    pub const ARM_TIMER: u32 = 0xbb;
    pub const HIT_TICK: u32 = 0xba;
    pub const BITES: u32 = 0xbe;
    pub const RAGE: u32 = 0xae;
    /// Delay (word) before a player too close triggers the counterattack, and its countdown.
    pub const GUARD: u32 = 0x46;
    pub const GUARD_TIMER: u32 = 0x5c;
    /// Host (boss of type 2).
    pub const HOST: u32 = 0x42;
}
use g::*;
use crate::objects::pools;
use crate::objects::Current;

/// Animation sets of a boss.
#[derive(Clone, Copy)]
pub(self) enum Set {
    A,
    B,
}

/// What distinguishes the type 2 boss from the type 22 one: the code is that of a single family, copied with other
/// animation sets and other ROM tables.
pub(self) struct Kit {
    set_a: u32,
    set_b: u32,
    /// Attack tables (32 bytes per pair of levels): short range, long range, end of attack.
    close: u32,
    far: u32,
    next: u32,
    /// Arrival points of the run (6-byte rows: X, Z, angle, direction) and counterattack chances (rows of
    /// `1 << chances_shift` bytes).
    spot: u32,
    chances: u32,
    chances_shift: u32,
    /// Run-up draw table, hit reaction table.
    dash: u32,
    reactions: u32,
}

const TWO_KIT: Kit = Kit {
    set_a: 0x1733ac,
    set_b: 0x1733e6,
    close: 0x7ea1e,
    far: 0x7ec1e,
    next: 0x7ee7c,
    spot: 0x7f25a,
    chances: 0x7e7a2,
    chances_shift: 2,
    dash: 0x7cd3e,
    reactions: 0x7e5e2,
};

const PART_KIT: Kit = Kit {
    set_a: 0x18492a,
    set_b: 0x184964,
    close: 0x9d280,
    far: 0x9d480,
    next: 0x9d6e0,
    spot: 0x9dabe,
    chances: 0x9d012,
    chances_shift: 4,
    dash: 0x9b5a0,
    reactions: 0x9ce66,
};

/// Is the boss type 2 (and not type 22)?
pub(self) fn two<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    x.u8(2) == 2
}

pub(self) fn kit<R: Bench>(x: &mut Cx<'_, R>) -> &'static Kit {
    if two(x) { &TWO_KIT } else { &PART_KIT }
}

/// Starts animation `n` of set `set` (0x1a32).
pub(self) fn start<R: Bench>(x: &mut Cx<'_, R>, set: Set, n: u32) {
    let k = kit(x);
    x.anim_start(if matches!(set, Set::A) { k.set_a } else { k.set_b }, n as u16);
}

/// States set in one go (`+4..+7`).
pub(self) mod state {
    pub const STAND: u32 = 0x0202_0000;
    pub const TURN: u32 = 0x0202_0200;
    pub const LEAP: u32 = 0x0202_0600;
    pub const IDLE: u32 = 0x020a_0000;
}

pub(self) mod call {
}

// --- animations --------------------------------------------------------------------------------

/// Animation number `id` (+0xa2); starts animation `n` of set `set` if it is not the last one started (+0xa3).
pub(self) fn pose<R: Bench>(x: &mut Cx<'_, R>, set: Set, id: u8, n: u32) {
    x.set8(ANIM_ID, id);
    if x.u8(ANIM_ID) != x.u8(ANIM_LAST) {
        let id = x.u8(ANIM_ID);
        x.set8(ANIM_LAST, id);
        start(x, set, n);
    }
}

/// Sound settings `+0x44` and `+0x45` set along with the animation.
pub(self) fn sounds<R: Bench>(x: &mut Cx<'_, R>, a: u8, b: u8) {
    x.set8(0x44, a);
    x.set8(0x45, b);
}

/// Animation of the body or of the attacks depending on whether it is in its second form.
pub(self) fn second_form<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    x.u8(SECOND) != 0
}

pub(self) fn next7<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x7, 2);
}
pub(self) fn next6<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB2, 2);
}

/// A dust puff / explosion `D0 .. D3` (0xb10c8) every eight calls (`+0xb7`).
pub(self) fn rubble<R: Bench>(x: &mut Cx<'_, R>, d0: u32, d1: u32, d2: u32, d3: u32) {
    x.add8(EFFECT_COUNT, 1);
    let n = x.u8(EFFECT_COUNT) & 7;
    x.set8(EFFECT_COUNT, n);
    if n != 0 {
        return;
    }
    lib::shock(x, d0, d1 as u16, d2 as u16, d3 as u16);
}

/// 0x9d8e0: is the target on the wrong side (the boss is not facing it)?
pub(self) fn needs_turn<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if x.u16(f::TARGET) == 0 {
        return false;
    }
    let t = x.linked(f::TARGET);
    let ahead = (t.u16(x.b, 0x8).wrapping_sub(x.u16(0x8)) as i16) > 0;
    let wanted = if ahead { 0x20 } else { 0 };
    wanted != x.u8(f::FLIP)
}

/// 0x9d928: is the target far in both depth and X?
pub(self) fn target_far<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if x.u16(f::TARGET) == 0 {
        return false;
    }
    let t = x.linked(f::TARGET);
    let dz = t.u16(x.b, 0x10).wrapping_sub(x.u16(0x10)).wrapping_add(0x20);
    if dz <= 0x40 {
        return false;
    }
    let dx = t.u16(x.b, 0x8).wrapping_sub(x.u16(0x8)).wrapping_add(0xc0);
    dx > 0x180
}

/// 0x9b30a and 0x9b384: a target in the box, with possible hits (0xa4b4 / 0xa4c2); the boss charges or attacks.
fn hunt_box<R: Bench>(x: &mut Cx<'_, R>, d0: i16, d1: u16, d2: u16, table: u32) -> bool {
    if !lib::target_in_box(x, d0, d1, d2) || x.u16(f::TARGET) == 0 {
        return false;
    }
    let t = x.linked(f::TARGET);
    let same = t.u8(x.b, f::FLIP) == x.u8(f::FLIP);
    if !lib::decision(x, same) {
        return false;
    }
    let r = rand_and(x, 0xf);
    if 0xffc0u32 >> r & 1 != 0 {
        x.set32(f::STATE, 0x0202_0800);
        return true;
    }
    x.set32(f::STATE, 0x0204_0000);
    lib::pick_byte(x);
    attack_from_table(x, table);
    true
}

pub(self) fn hunt_close<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let table = kit(x).close;
    hunt_box(x, -0x40, 0x40, 8, table)
}
pub(self) fn hunt_far<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let table = kit(x).far;
    hunt_box(x, -0xd0, 0x50, 0x50, table)
}

/// 0x9d24e and 0x9d254: attack drawn from the difficulty row of `table` (32 bytes per pair of levels).
pub(self) fn attack_from_table<R: Bench>(x: &mut Cx<'_, R>, table: u32) {
    let row = ((var::RANK.get(x.b) >> 1) as u32) << 5;
    let r = rand_and(x, 0x1f);
    let step = x.data8(table + row + r);
    x.set8(f::SUB2, step);
    x.set16(f::STATE, 0x0204);
    x.set8(0x7, 0);
}

/// 0x9d680 (0x7ee1e for type 2): end of an attack: standing, or a new attack if a player is very close.
pub(self) fn next_attack<R: Bench>(x: &mut Cx<'_, R>) {
    for k in 0..4u32 {
        let p = crate::objects::Obj(0xff85de + 0x200 * k);
        if p.u8(x.b, 0) == 0 {
            // type 22 stops at the first absent player, type 2 moves on to the next
            if two(x) {
                continue;
            }
            break;
        }
        if reach(x, p, -0x38, 0x38, 0x18) {
            let row = ((var::RANK.get(x.b) >> 1) as u32) << 5;
            let r = rand_and(x, 0x1f);
            let table = kit(x).next;
            let step = x.data8(table + row + r);
            x.set8(f::SUB2, step);
            x.set16(f::STATE, 0x0208);
            x.set8(0x7, 0);
            return;
        }
    }
    x.set32(f::STATE, state::STAND);
}

/// 0x9d9d0: a player very close (box ±0x28 in X, ±8 in depth) at ground height?
pub(self) fn player_within_reach<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    for k in 0..4u32 {
        let p = crate::objects::Obj(0xff85de + 0x200 * k);
        if p.u8(x.b, 0) == 0 {
            continue;
        }
        if reach(x, p, -0x28, 0x28, 8) {
            return true;
        }
    }
    false
}

/// 0x9da00: is `p` in the boss's box (offset `d0`, half-width `d1`, half-depth `d2`), on the ground?
pub(self) fn reach<R: Bench>(x: &mut Cx<'_, R>, p: crate::objects::Obj, d0: i16, d1: u16, d2: u16) -> bool {
    if p.u16(x.b, 0xc) != 0 {
        return false;
    }
    let mut off = d0 as u16;
    if x.u8(f::FLIP) != 0 {
        off = off.wrapping_neg();
    }
    let dx = off.wrapping_add(x.u16(0x8)).wrapping_sub(p.u16(x.b, 0x8)).wrapping_add(d1);
    if dx > d1.wrapping_mul(2) {
        return false;
    }
    let dz = x.u16(0x10).wrapping_sub(p.u16(x.b, 0x10)).wrapping_add(d2);
    dz <= d2.wrapping_mul(2)
}

/// 0x9da5c: chooses the arrival point of the run (`+0xb0`, `+0xb2`), the angle and the rotation direction, from the
/// boss's position on the screen (table 0x9dabe: X, Z, angle, direction).
pub(self) fn choose_spot<R: Bench>(x: &mut Cx<'_, R>) {
    let dx = x.u16(0x8).wrapping_sub(var::SCROLL_X.get(x.b));
    let dz = x.u16(0x10).wrapping_sub(var::SCROLL_Y.get(x.b));
    let row = if (dx as i16) > 0xc0 {
        if (dz as i16) > 0x80 { 0x12 } else { 0xc }
    } else if (dz as i16) > 0x80 {
        6
    } else {
        0
    };
    let base = kit(x).spot;
    let sx = x.rom16(base + row).wrapping_add(var::SCROLL_X.get(x.b));
    x.set16(SPOT_X, sx);
    let sz = x.rom16(base + 2 + row).wrapping_add(var::SCROLL_Y.get(x.b));
    x.set16(SPOT_Z, sz);
    let angle = x.rom8(base + 4 + row);
    x.set8(0x2d, angle);
    let spin = x.rom8(base + 5 + row);
    x.set8(SPIN, spin);
}

/// 0x9db72: five dust puffs in a line under the boss as it lands.
pub(self) fn dust_line<R: Bench>(x: &mut Cx<'_, R>) {
    for dz in [-0x30i16, -0x18, 0, 0x18, 0x30] {
        lib::shock(x, 1, 0, dz as u16, 4);
    }
}

/// 0x7f328 and 0x7f3a0: dust puffs in a line, toward the right (the boss faces left) or toward the left.
pub(self) fn dust_trail<R: Bench>(x: &mut Cx<'_, R>, right: bool) {
    const RIGHT: [i16; 6] = [-0x10, 8, 0x20, 0x38, 0x50, 0x68];
    const LEFT: [i16; 6] = [-0x68, -0x58, -0x38, -0x20, -0x8, 0x10];
    for dz in if right { RIGHT } else { LEFT } {
        lib::shock(x, 1, 0, dz as u16, 4);
    }
}

/// Five calls of 0x9eca4 (bursts), D5 = 4 to 0 like the original's `dbf` loop.
pub(self) fn scatter<R: Bench>(x: &mut Cx<'_, R>) {
    for i in (0..5u32).rev() {
        *x.c.d5() = i;
        eshl_debris(x, i as u8);
    }
    *x.c.d5() |= 0xffff;
}

/// 0x9db28: fifteen palette colors written at 0x91c340; the boss takes palette offset 0x1a.
fn set_palette<R: Bench>(x: &mut Cx<'_, R>) {
    const COLORS: [u16; 15] = [
        0xf333, 0xfdef, 0xfaed, 0xf9b9, 0xf575, 0xf244, 0xffff, 0xfddd, 0xfa9a, 0xf656, 0xf5cb, 0xf397, 0xf075, 0xf053, 0xf031,
    ];
    for (i, c) in COLORS.iter().enumerate() {
        crate::mem::wr16(x.b, 0x91c340 + 2 * i as u32, *c);
    }
    x.set8(0x2b, 0x1a);
}

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    x.tick_counters();
    if two(x) {
        two::spawn_scene_controller(x);
    }
    match x.u8(f::STATE) {
        st::INIT if two(x) => two::init(x),
        st::INIT => init(x),
        st::ACTIVE => active(x),
        st::HIT => hit::run(x),
        _ => {
            if !two(x) {
                x.set_a5_u8(0x50, 0);
            }
            x.free(&pools::BOSS);
        }
    }
}

// --- state 0: creation --------------------------------------------------------------------------

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    set_palette(x);
    let r = random(x);
    x.set8(EFFECT_COUNT, r as u8);
    x.set8(f::BOX_SET, 0x22);
    lib::create_life_bar(x);
    lib::initial_hp_boss(x);
    for (off, v) in [(0x70, 0), (0x7e, 0x23), (0x7f, 0), (0x8e, 3), (0x78, 0xff), (0x68, 8), (0x69, 8), (0x6a, 3), (0x6c, 1), (0x6d, 8)] {
        x.set8(off, v);
    }
    let speed = if x.u16(f::TARGET) != 0 && var::PLAYERS_ALIVE.get(x.b) != 1 { 7 } else { 6 };
    x.set8(0x72, speed);
    x.set8(0x73, speed);
    for (off, v) in [(0x74, 5), (0x75, 0x3c), (0x76, 6), (0x81, 0x12), (0x80, 0x17)] {
        x.set8(off, v);
    }
    x.set16(GUARD, 0xb4);
    x.set16(GUARD_TIMER, 0xf0);
    var::SCENE_LEVEL.set(x.b, 0);
    x.set32(f::IMMUNITY, 0xffff_ffff);
    pick_target(x);
    lib::draw_stance(x);
    lib::draw_6b(x);
    x.set8(RAGE, 0xff);
    x.set8(SECOND_FX, 0);
    x.set16(SPOT_TIMER, 0x10);
    x.set8(0x93, 0);
    // `jmp 0x1a32`: the registers it leaves to the following handler are re-read by no one (`exit` probe)
    let set = kit(x).set_a;
    x.anim_start(set, 0);
}

// --- state 2: active -----------------------------------------------------------------------------

/// 0x9afbc: one group per frame (outside freeze and pause); a countdown (`+0x5c`, restart at 0xf0) triggers, when
/// the boss is on the ground and `+0x46` designates an object, state 0x0204_1600 (anger).
fn active<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(f::TARGET) == 0 {
        pick_target(x);
    }
    if tst8(x, f::FREEZE).gt() {
        sub8(x, f::FREEZE, 1);
    } else if var::PAUSE.get(x.b) == 0 {
        group(x);
        if tst8(x, ARM_TIMER).gt() {
            sub8(x, ARM_TIMER, 1);
            x.set8(0x38, 0);
        }
    }
    let expired = sub16(x, GUARD_TIMER, 1).le();
    let fire = !expired && (x.u16(GUARD) as i16) < 0 && x.u16(0xc) == 0;
    if fire {
        x.set8(0x86, 0);
        x.set16(0x84, 0);
        x.set8(0x49, 0);
        x.set8(0, 1);
        let fuse = if two(x) { 0x46 } else { 0x50 };
        x.set8(ARM_TIMER, fuse);
        x.set32(f::STATE, 0x0204_1600);
    }
    if expired || fire {
        x.set16(GUARD, 0xb4);
        x.set16(GUARD_TIMER, 0xf0);
    }
    lib::keep_on_screen(x, 0x30);
    lib::collide_terrain(x);
    x.reinsert();
}

fn group<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB) {
        0 if two(x) => two::arrive(x),
        0 => moves::arrive(x),
        2 => moves::fight(x),
        4 => attacks::run(x),
        6 => moves::run_to_spot(x),
        8 => moves::run_to_spot_after_attack(x),
        10 => moves::paused(x),
        12 => moves::holding(x),
        14 => moves::transform(x),
        _ => {}
    }
}

handlers! {
    0x9aeaa "boss_22" #[without] run,
    0x7c5a8 "boss_2" #[without] run,
}
