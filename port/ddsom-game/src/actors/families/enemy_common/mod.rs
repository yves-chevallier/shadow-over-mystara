//! Common parts of the ENEMY class handlers (enemies, pool of 22 × 0xC0 bytes): fields, states, sub-object allocation,
//! death, rank and pause helpers. See `docs/families/enemy.md`.
//!
//! All enemies share the same framework: byte `+4` is the main state (0 creation, 2 active, 4 hit, 6 dead), `+5` and
//! `+6` substates; the values are even bytes that index jump tables. The "dead" state frees the slot (`0x21f4`).

pub mod tables;
pub mod vars;

use crate::actors::families::cx::Bench;

use super::cx::Cx;
use super::effects_creators::{self, leftovers::Pure};
use super::eshl_creators;
/// Common library of enemies and bosses (`lib::draw_stance(x)`...).
pub use super::enemy_lib as lib;
/// The common services of the machine (`x.random()`, reads of the program space) for the families that import this
/// module.
pub use crate::world::machine::Machine;
use crate::world::objects::Obj;
use crate::world::objects::pools;
use crate::world::objects::Current;
use crate::world::ram::PLAYERS;
pub use crate::world::objects::Enemy;
/// Work RAM variables read by the enemies and bosses.
pub use vars as var;

/// Fields of ENEMY objects, by offset (the names that the families use; the fields are declared in [`Enemy`]).
pub mod f {
    use super::Enemy;
    /// Main state, substate, sub-substate.
    pub const STATE: u32 = Enemy::STATE;
    pub const SUB: u32 = Enemy::SUB;
    pub const SUB2: u32 = Enemy::STEP;
    pub const SUB3: u32 = Enemy::SUB_STEP;
    /// Collision box set (see `docs/combat.md` §3).
    pub const BOX_SET: u32 = Enemy::BOX_SET;
    /// Orientation (non-zero: faces left).
    pub const FLIP: u32 = Enemy::FLIP;
    /// Key-frame mark of the current animation (low byte of `Obj::ANIM_FLAGS`, bit 7: event mark).
    pub const KEY_FRAME: u32 = Enemy::KEY_FRAME;
    /// Host (word, RAM address) of a part of a composite enemy.
    pub const HOST: u32 = Enemy::HOST;
    /// Parent or owner (word, RAM address); `+0x42`: creator of a sub-object.
    pub const OWNER: u32 = Enemy::TARGET;
    pub const TARGET: u32 = Enemy::TARGET;
    pub const CREATOR: u32 = Enemy::CREATOR;
    /// Freeze counter after a hit (decremented by `0xf1bfc`).
    pub const FREEZE: u32 = Enemy::FREEZE;
    /// Hit points (signed word) and value remembered at the last hit.
    pub const HP: u32 = Enemy::HP;
    /// Immunities (long: one bit per shot behaviour).
    pub const IMMUNITY: u32 = Enemy::IMMUNITY;
}

/// Main states (`+4`), long values for `Cx::set32(0x4, …)`: state in the high byte.
pub mod st {
    pub const INIT: u8 = 0;
    pub const ACTIVE: u8 = 2;
    pub const HIT: u8 = 4;
    pub const DEAD: u8 = 6;
    /// "Dead" state long (`+4..+7`): release on the next pass.
    pub const DEAD_LONG: u32 = 0x0600_0000;
}

/// Allocates an object from the pool `alloc` (address of an allocation routine) and returns its address (`A1`). The
/// original does not examine the carry: it writes through `A1` even if the pool is full.
pub fn alloc<R: Bench>(x: &mut Cx<'_, R>, pool: &pools::Pool) -> Obj {
    x.alloc_unchecked(pool)
}

/// Allocates an active ESHL shot of type `kind`, with variant `variant`, linked to the current object by the word
/// `link` (`None` if the pool is full). The creators below are the small routines 0x9ddee..0xa6824 of the original.
fn spawn_eshl<R: Bench>(x: &mut Cx<'_, R>, kind: u8, variant: u8, link: u32) -> Option<Obj> {
    let o = x.alloc(&pools::ESHL)?;
    o.set_u8(x.b, Obj::ACTIVE, 1);
    o.set_u8(x.b, Obj::KIND, kind);
    o.set_u8(x.b, Obj::VARIANT, variant);
    o.set_u16(x.b, link, x.o.0 as u16);
    Some(o)
}

/// 0x9eca4: a burst (ESHL 7) of variant `variant`, owner `+0x50`.
pub fn eshl_debris<R: Bench>(x: &mut Cx<'_, R>, variant: u8) {
    spawn_eshl(x, 7, variant, Enemy::TARGET);
}

/// 0xa1d54: an ESHL 0x1d of variant `variant`, linked by `+0x5c`.
pub fn eshl_sweep_1d<R: Bench>(x: &mut Cx<'_, R>, variant: u8) {
    spawn_eshl(x, 0x1d, variant, Enemy::SPAWNER);
}

/// 0xa2048: an ESHL 0x1e, owner `+0x50`.
pub fn eshl_wave_1e<R: Bench>(x: &mut Cx<'_, R>) {
    spawn_eshl(x, 0x1e, 0, Enemy::TARGET);
}

/// 0xa219e: an ESHL 0x1f, linked by `+0x5c`.
pub fn eshl_wave_1f<R: Bench>(x: &mut Cx<'_, R>) {
    spawn_eshl(x, 0x1f, 0, Enemy::SPAWNER);
}

/// 0xb64b0: an EFFCT 0x33 linked by `+0x5c`.
pub fn effct_33<R: Bench>(x: &mut Cx<'_, R>) {
    effects_creators::linked_via(&mut Pure, x.b, x.o, effects_creators::kind::PROJECTILE, None);
}

/// 0xb53f4: three EFFCT 0x2e (`+0x2f` = 2, 1, 0) linked by `+0x46`; the first pool refusal stops the series.
pub fn effct_trio_2e<R: Bench>(x: &mut Cx<'_, R>) {
    effects_creators::type_46_trio(&mut Pure, x.b, x.o);
}

/// 0x667f4: an enemy of type 10, variant 4, linked by `+0x5c`, at (`px`, `py`) (explosions of the arrival of boss 2,
/// SET 17).
pub fn enemy_blast_at<R: Bench>(x: &mut Cx<'_, R>, px: u16, py: u16) {
    let Some(o) = x.alloc(&pools::ENEMY) else { return };
    let e = Enemy(o);
    e.set_active(x.b, 1);
    e.set_kind(x.b, 0xa);
    e.set_spawner(x.b, x.o.0 as u16);
    e.set_variant(x.b, 4);
    e.set_x(x.b, px);
    e.set_y(x.b, py);
}

/// 0x9ddee: an ESHL 1 shot of variant `variant`, index `+0x2f` = 1, owner `+0x50`.
pub fn eshl_thrown_by_beast<R: Bench>(x: &mut Cx<'_, R>, variant: u8) {
    eshl_creators::single_1(&mut Pure, x.b, x.o, variant);
}

/// 0xa2f96: sixteen ESHL 0x27 shots (variants 15 down to 0) linked by `+0x5c`.
pub fn eshl_burst_27<R: Bench>(x: &mut Cx<'_, R>) {
    eshl_creators::ring_27(&mut Pure, x.b, x.o);
}

/// 0xa6824: two ESHL 0x3f shots (orientation 0 then 0x20) linked by `+0x5c`.
pub fn eshl_pair_3f<R: Bench>(x: &mut Cx<'_, R>) {
    eshl_creators::pair_3f(&mut Pure, x.b, x.o);
}

/// 0xa460a, 0xa464e, 0xa4676: an ESHL 0x34 shot of variant 2, 1 or 0 linked by `+0x46` (variant 1 copies the creator's
/// orientation).
pub fn eshl_shot_34<R: Bench>(x: &mut Cx<'_, R>, variant: u8) {
    let flip = x.u8(f::FLIP);
    if let Some(o) = spawn_eshl(x, 0x34, variant, f::HOST) {
        let e = Enemy(o);
        e.set_group(x.b, 0);
        if variant == 1 {
            e.set_flip(x.b, flip);
        }
    }
}

/// Frees the current enemy ("dead" state handled).
pub fn free<R: Bench>(x: &mut Cx<'_, R>) {
    x.free(&pools::ENEMY);
}

/// Game paused (`A5+0x3d` non-zero).
pub fn paused<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    var::PAUSE.get(x.b) != 0
}

/// The handler is suspended (pause hook `0x29ea`, Z = not suspended).
pub fn suspended<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    x.suspended()
}

/// Object designated by a 16-bit word of the current object (`movea.w (off,A0),A1`).
pub fn link<R: Bench>(x: &mut Cx<'_, R>, off: u32) -> Obj {
    x.linked(off)
}

/// Writes the state long (`+4..+7`) of another object.
pub fn set_state<R: Bench>(x: &mut Cx<'_, R>, o: Obj, state: u32) {
    o.set_u32(x.b, f::STATE, state);
}

/// Common loop of the active states of the AI enemies: as long as the freeze `+0x53` is positive it decreases and
/// nothing else moves; otherwise, outside pause, `sub` runs the substate (it returns true to abandon the frame without
/// the rest, like the original which pops its return address). The rest tests the zone and the collision with the
/// terrain as soon as the substate is no longer 0, then puts the object back in the list.
pub fn active_loop<'a, R: Bench>(x: &mut Cx<'a, R>, sub: impl FnOnce(&mut Cx<'a, R>) -> bool) {
    let freeze = x.u8(f::FREEZE) as i8;
    if freeze > 0 {
        x.set8(f::FREEZE, (freeze - 1) as u8);
    } else if !paused(x) && sub(x) {
        return;
    }
    if x.u8(f::SUB) != 0 {
        lib::keep_in_zone(x);
        lib::collide_terrain(x);
    }
    x.reinsert();
}

/// Byte of the "hit" state that skips the zone test when non-zero (`hit_loop`; type 4 has it at `+0xa5`).
const NO_ZONE_TEST: u32 = 0xae;

/// Common loop of the "hit" state: same freeze; the zone is tested only if `+0xae` is zero.
pub fn hit_loop<'a, R: Bench>(x: &mut Cx<'a, R>, sub: impl FnOnce(&mut Cx<'a, R>)) {
    hit_loop_at(x, NO_ZONE_TEST, sub)
}

/// `hit_loop` with the "no zone test" byte at another offset (`+0xa5` for type 4).
pub fn hit_loop_at<'a, R: Bench>(x: &mut Cx<'a, R>, no_zone: u32, sub: impl FnOnce(&mut Cx<'a, R>)) {
    let freeze = x.u8(f::FREEZE) as i8;
    if freeze > 0 {
        x.set8(f::FREEZE, (freeze - 1) as u8);
    } else {
        sub(x);
    }
    if x.u8(no_zone) == 0 {
        lib::keep_in_zone(x);
        lib::collide_terrain(x);
    }
    x.reinsert();
}

/// The reaction routine designated by the hit code `+0x49` in the reaction table `table` (its address: the key of the
/// table in `data/enemies.json`; longs, indexed by the code).
pub fn reaction_routine<R: Bench>(x: &mut Cx<'_, R>, table: u32) -> u32 {
    let code = x.u8(Enemy::REACTION) as u32;
    tables::reactions(table).at32(x.b, code)
}

/// Reaction to a hit: the common routine, chosen in `table` (longs, indexed by the reaction code `+0x49`) by the
/// collision, plays the recoil, fall or death animation. The routines are called by `jsr (A1)` with D0 = the code: the
/// routine read remains the index of the Rust tables (`reactions`, `small::own_reaction`).
pub fn react<R: Bench>(x: &mut Cx<'_, R>, table: u32) {
    let routine = reaction_routine(x, table);
    run_reaction(x, routine);
}

/// Runs the reaction routine `routine` (common, or specific to the type).
pub fn run_reaction<R: Bench>(x: &mut Cx<'_, R>, routine: u32) {
    if !super::reactions::run_common(x, routine) {
        super::small::own_reaction(x, routine)
            .unwrap_or_else(|| unreachable!("reaction {routine:#x}: neither common nor specific to the type"));
    }
}

/// `add.b #v,(off,An)` on another object (modulo 2^8).
pub fn add8<R: Bench>(x: &mut Cx<'_, R>, o: Obj, off: u32, v: u8) {
    let n = o.u8(x.b, off).wrapping_add(v);
    o.set_u8(x.b, off, n);
}

/// `add.w #v,(off,An)` on another object (modulo 2^16).
pub fn add16<R: Bench>(x: &mut Cx<'_, R>, o: Obj, off: u32, v: u16) {
    let n = o.u16(x.b, off).wrapping_add(v);
    o.set_u16(x.b, off, n);
}

/// Starts script `n` of the animation set `set` (`movea.l set,A4 ; moveq n,D0 ; jsr 0x1a32`).
pub fn anim<R: Bench>(x: &mut Cx<'_, R>, set: u32, n: u32) {
    x.anim_start(set, n as u16);
}

// --- common helpers of the types of the third batch ---------------------------------------------

/// Common end of the active substates: zone test (0xfbae) then collision with the terrain (0x33fe4). True if the object
/// was brought back into the screen or pushed back by the terrain (the original's D5, which the skeleton rereads).
pub fn zone_and_terrain<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    lib::keep_in_zone(x);
    lib::collide_terrain(x);
    // the original's D5 (`dispatch::Leftovers::d5`): 1 if the zone moved the object (`moveq`) or if the terrain pushed
    // it back
    *x.c.d5() != 0
}

/// `neg.w (off,A0)`.
pub fn negate<R: Bench>(x: &mut Cx<'_, R>, off: u32) {
    let v = x.u16(off).wrapping_neg();
    x.set16(off, v);
}

/// `movea.w (0x50,A0),A1`: the target (sign-extended address). The original keeps it in A1, which the beetle and the
/// scorpion then reread (`aim_to`): they take the returned value.
pub fn load_target<R: Bench>(x: &mut Cx<'_, R>) -> Obj {
    x.linked(f::TARGET)
}

/// Draw `rand & mask` (`andi.w #mask,D0w`).
pub fn rand_and<R: Bench>(x: &mut Cx<'_, R>, mask: u32) -> u32 {
    x.random() as u32 & mask
}

/// Row of 32 bytes of the difficulty level (`RANK / 2`) in a table of `data/enemies.json` by rank.
pub fn rank_row<R: Bench>(x: &mut Cx<'_, R>) -> u32 {
    ((var::RANK.get(x.b) >> 1) << 5) as u32
}

/// Byte drawn at random (5 bits) in the row of the difficulty level of the table `name` (rows of 32 bytes).
pub fn rank_byte<R: Bench>(x: &mut Cx<'_, R>, name: &str) -> u8 {
    let row = rank_row(x);
    let r = rand_and(x, 0x1f);
    tables::table(name).at8(x.b, row + r)
}

/// Writes the state long of the current object (`move.l #v,(4,A0)`).
pub fn set_state_long<R: Bench>(x: &mut Cx<'_, R>, v: u32) {
    x.set32(f::STATE, v);
}

// --- common AI helpers --------------------------------------------------------------------------

/// Chooses a player as target (0x424e); false if none is alive.
pub fn pick_target<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    lib::pick_target(x).is_some()
}

/// The enemy is grabbed by a player (0x2810).
pub fn grabbed<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    !crate::actors::aids::grabbed(x.b, x.o).z()
}

/// The enemy's target (player designated by `+0x50`), or `None` if it has none.
pub fn target<R: Bench>(x: &mut Cx<'_, R>) -> Option<Obj> {
    if x.u16(f::TARGET) == 0 {
        return None;
    }
    Some(x.linked(f::TARGET))
}

/// Byte of the families that hold the running animation (`+0xa2`, zero: none), read by `needs_turn`.
const RUNNING_ANIM: u32 = 0xa2;

/// The enemy is not looking toward its target (0x74eac, 0x5f710): false without a running animation (`+0xa2` zero) or
/// without a target.
pub fn needs_turn<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if x.u8(RUNNING_ANIM) == 0 {
        return false;
    }
    let Some(t) = target(x) else { return false };
    let dx = Enemy(t).x(x.b).wrapping_sub(x.u16(Enemy::X)) as i16;
    let facing = if dx > 0 { 0x20 } else { 0 };
    facing != x.u8(f::FLIP)
}

// --- helpers of the types of the fourth batch ---------------------------------------------------

/// 0x67452: looks for the closest player in a box in front of the enemy (offset `d0`, half-width `d1`, half-depth `d2`)
/// and stores it in `+0x50`; false if none. The box follows the enemy's orientation. D5 keeps, like the original, the
/// last computed gap.
pub fn find_player<R: Bench>(x: &mut Cx<'_, R>, d0: i16, d1: u16, d2: u16) -> bool {
    x.set16(f::TARGET, 0);
    let d0 = if x.u8(f::FLIP) != 0 { d0.wrapping_neg() } else { d0 } as u16;
    let mut best = 0xffffu16;
    let mut d5 = *x.c.d5() as u16;
    for p in PLAYERS.iter() {
        let player = Enemy(p);
        if player.active(x.b) == 0 {
            continue;
        }
        let dy = x.u16(Enemy::Y).wrapping_sub(player.y(x.b)).wrapping_add(d2);
        d5 = dy;
        if dy > d2.wrapping_mul(2) {
            continue;
        }
        let dx = x.u16(Enemy::X).wrapping_add(d0).wrapping_sub(player.x(x.b)).wrapping_add(d1);
        d5 = dx;
        if dx > d1.wrapping_mul(2) {
            continue;
        }
        let distance = dx.wrapping_add(dy);
        if distance <= best {
            best = distance;
            x.set16(f::TARGET, p.0 as u16);
        }
    }
    // Original glitch: D5.w keeps the last gap (`dispatch::Leftovers::d5`)
    *x.c.d5() = *x.c.d5() & !0xffff | d5 as u32;
    x.u16(f::TARGET) != 0
}

/// Creates the objects of a piece list: the tables `<family>/lists` (addresses of the lists) and `<family>/allocs`
/// (allocation routines) of `data/enemies.json` give the list `index`, a count then entries of `allocation routine
/// (byte), type, variant, +0x2f, X, height, depth`; the creator is stored in `+0x5c` of each object. Allocations that
/// fail skip their entry.
pub fn spawn_list<R: Bench>(x: &mut Cx<'_, R>, family: &str, index: u32) {
    let (lists, allocs) = tables::piece_lists(family);
    let mut at = lists.nth32(x.b, index);
    let count = tables::held16(x.b, at);
    at += 2;
    for _ in 0..=count {
        let routine_index = tables::held8(x.b, at) as u32;
        at += 1;
        let routine = allocs.nth32(x.b, routine_index);
        if let Some(o) = x.alloc(pools::pool_of_alloc_routine(routine)) {
            let (kind, variant, group) = (tables::held8(x.b, at), tables::held8(x.b, at + 1), tables::held8(x.b, at + 2));
            let (px, ph, pz) = (tables::held16(x.b, at + 3), tables::held16(x.b, at + 5), tables::held16(x.b, at + 7));
            let e = Enemy(o);
            e.set_active(x.b, 1);
            e.set_kind(x.b, kind);
            e.set_variant(x.b, variant);
            e.set_group(x.b, group);
            e.set_x(x.b, px);
            e.set_height(x.b, ph);
            e.set_y(x.b, pz);
            e.set_spawner(x.b, x.o.0 as u16);
        }
        at += 9;
    }
}

/// 0x27ee: the enemy detaches from its host (`HOST`): if the host is active and still has parts (`+0x2a`), the count
/// decreases and the link is cleared.
pub fn detach_from_host<R: Bench>(x: &mut Cx<'_, R>) {
    let host = x.u16(f::HOST);
    if host != 0 {
        let h = Enemy(x.linked(f::HOST));
        let parts = h.parts(x.b);
        if h.active(x.b) != 0 && parts != 0 {
            h.set_parts(x.b, parts - 1);
            x.set16(f::HOST, 0);
        }
    }
}
