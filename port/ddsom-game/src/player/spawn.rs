//! Creators of player effects (PEFF pool) and player shots (PSHL pool).
//!
//! The characters' actions draw nothing themselves: at the carried gesture they create an object linked
//! to the player (sword stroke, breath, light, shadow...). The object is born in a pool with its kind
//! (`Obj::KIND`), which selects the ROM handler in charge of animating it, and a few parameters
//! specific to the kind. The word `+0x50` links it to the player who created it; the handler finds there the
//! player's position and orientation.
//!
//! Each creator has a Rust function (called by the actions) and a native at the original address for
//! the callers left in ROM. When the pool is full the allocation fails and nothing is created.

use super::{Ctx, Player};
use crate::mem::{rd16, rd8, wr16, wr8, A5};
use crate::objects::Obj;
use crate::objects::pools;
use crate::player::ram;
use crate::player::tables::{player_data, players, pc8, Read};

/// Bytes of the created object.
mod field {
    /// Link to the creating player (word: address).
    pub const LINK: u32 = 0x50;
    /// Kind parameter.
    pub const PARAM: u32 = 0x2f;
    /// Scene number (byte of A5) copied into the object: the object disappears when the scene changes.
    pub const SCENE: u32 = 0x5b;
}

/// Kinds of player effects.
mod kind {
    pub const BLOOD: u8 = 2;
    pub const SLASH: u8 = 3;
    pub const CURE: u8 = 4;
    pub const HOLY_LIGHT: u8 = 5;
    pub const BEAM_CAST: u8 = 6;
    pub const ICE_GROUND: u8 = 7;
    pub const SHADOW: u8 = 8;
    pub const BEAM: u8 = 9;
    pub const BLESSING: u8 = 0xd;
    pub const BANISH: u8 = 0xe;
    pub const ICE: u8 = 0xc;
    pub const WEAPON_TRAIL: u8 = 0xf;
    pub const LIGHTNING: u8 = 0x13;
    pub const FIREBALL: u8 = 0x16;
    pub const KNIFE: u8 = 0x15;
    pub const RING: u8 = 0x1b;
    pub const SHOWER: u8 = 0x19;
    pub const AURA: u8 = 0x1d;
    pub const STORM: u8 = 0x1f;
    pub const LIGHTNING_TARGET: u8 = 0x20;
    pub const SWARM: u8 = 0x22;
    pub const LEAP: u8 = 0x23;
    pub const SWAP: u8 = 0x24;
    pub const DASH: u8 = 0x25;
    pub const METEORS: u8 = 0x28;
    pub const WALL_TARGET: u8 = 0x29;
    /// Creature summoned by the magic-user (PEFF).
    pub const SUMMON: u8 = 0x12;
    /// Magic-user's portal (PEFF; same number as the fireball, which is a PSHL shot).
    pub const PORTAL: u8 = 0x16;
    /// Witch summoned by the magic-user (PSHL shot).
    pub const WITCH: u8 = 0x12;
    /// Cursed weapon that escapes (PEFF).
    pub const CURSED_WEAPON: u8 = 0x26;
    /// Shards of the dying player (PEFF).
    pub const SHARD: u8 = 0x27;
    /// Effects of the magic-user's spells (PEFF): sparks and portal flash, glow, storm swarm; and rain of
    /// bolts (PSHL).
    pub const PORTAL_SPARK: u8 = 0x13;
    pub const PORTAL_BURST: u8 = 0x14;
    pub const RAIN: u8 = 0x14;
    pub const PORTAL_GLOW: u8 = 0x2d;
    pub const STORM_CLOUD: u8 = 0x2e;
    /// Effects of the magic-user's storm (leftward), rain, wall of fire and wave spells (PEFF).
    pub const STORM_SPELL: u8 = 0x14;
    pub const RAIN_GLOW: u8 = 0x17;
    pub const WALL_SPARK: u8 = 0x15;
    pub const WAVE_BOLT: u8 = 0x1a;
    /// Elf's moons (PEFF).
    pub const MOON: u8 = 0x1c;
    pub const MOON_B: u8 = 0x1e;
}

/// Reserves a slot in `pool` and activates it with kind `kind`.
fn create(g: &mut Ctx, _p: Player, pool: &pools::Pool, kind: u8) -> Option<Obj> {
    cov!();
    let o = pool.alloc(g.b)?;
    o.set_u8(g.b, Obj::ACTIVE, 1);
    o.set_u8(g.b, Obj::KIND, kind);
    Some(o)
}

/// Last gesture of a creator: the link to the player.
fn link(g: &mut Ctx, o: Obj, p: Player) {
    cov!();
    o.set_u16(g.b, field::LINK, p.0 as u16);
}

/// Creates an effect linked to the player: kind, variant (`Obj::VARIANT`), then the link. `fill` writes the other
/// bytes.
fn effect(g: &mut Ctx, p: Player, kind: u8, variant: Option<u8>, fill: impl FnOnce(&mut Ctx, Obj)) -> Option<Obj> {
    cov!();
    let o = create(g, p, &pools::PEFF, kind)?;
    if let Some(v) = variant {
        o.set_u8(g.b, Obj::VARIANT, v);
    }
    fill(g, o);
    link(g, o, p);
    Some(o)
}

/// The step of the creator's current action (copied into the object so that its handler knows where it comes from).
fn step_of(g: &mut Ctx, p: Player) -> u8 {
    p.u8(g, Player::STEP)
}

// ---- player effect creators --------------------------------------------------------------------

/// 0x4a99a: hit trail. `Some(n)`: trail of hit no. `n` (variant 0), except with weapons 0x10 to 0x13 which
/// leave none; `None`: simple trail (variant 1).
pub fn slash(g: &mut Ctx, p: Player, numbered: Option<u8>) {
    cov!();
    match numbered {
        Some(n) => {
            if p.u8(g, Player::WEAPON_A).wrapping_sub(0x10) <= 3 {
                return;
            }
            let step = step_of(g, p);
            effect(g, p, kind::SLASH, Some(0), |g, o| {
                o.set_u8(g.b, field::PARAM, n);
                o.set_u8(g.b, 0x5c, step);
            });
        }
        None => {
            let step = step_of(g, p);
            effect(g, p, kind::SLASH, Some(1), |g, o| o.set_u8(g.b, 0x5c, step));
        }
    }
}

/// 0x4a860: blood of the fighter's whirlwind.
pub fn blood(g: &mut Ctx, p: Player) {
    cov!();
    let step = step_of(g, p);
    effect(g, p, kind::BLOOD, None, |g, o| o.set_u8(g.b, 0x5c, step));
}

/// 0x4ab48: healing of an ally (cleric's prayer), prayer number `n`.
pub fn cure(g: &mut Ctx, p: Player, n: u8) {
    cov!();
    effect(g, p, kind::CURE, None, |g, o| o.set_u8(g.b, field::PARAM, n));
}

/// 0x4ab64: potion healing (variant 2).
pub fn potion(g: &mut Ctx, p: Player) {
    cov!();
    effect(g, p, kind::CURE, Some(2), |_, _| {});
}

/// 0x4aeb2: cleric's holy light.
pub fn holy_light(g: &mut Ctx, p: Player) {
    cov!();
    let step = step_of(g, p);
    effect(g, p, kind::HOLY_LIGHT, None, |g, o| o.set_u8(g.b, 0x56, step));
}

/// 0x4b5ac: ray. `aimed` false: two rays (parameter 0 and 1); true: a single aimed one (variant 1).
pub fn beam(g: &mut Ctx, p: Player, aimed: bool) {
    cov!();
    if aimed {
        aimed_beam(g, p);
        return;
    }
    let step = step_of(g, p);
    for param in [0, 1] {
        let made = effect(g, p, kind::BEAM, Some(0), |g, o| {
            o.set_u8(g.b, field::PARAM, param);
            o.set_u8(g.b, 0x5f, step);
        });
        if made.is_none() {
            return;
        }
    }
}

/// 0x4b5ac with D0 non-zero: the aimed beam (variant 1). Returns the A1 that the original leaves: the beam, or, when the
/// pool is full, the first slot or the end of the pool (`Pool::alloc_or_leftover`).
pub fn aimed_beam(g: &mut Ctx, p: Player) -> Obj {
    cov!();
    let o = match pools::PEFF.alloc_or_leftover(g.b) {
        Ok(o) => o,
        Err(leftover) => return leftover,
    };
    o.set_u8(g.b, Obj::ACTIVE, 1);
    o.set_u8(g.b, Obj::KIND, kind::BEAM);
    o.set_u8(g.b, Obj::VARIANT, 1);
    link(g, o, p);
    o
}

/// 0x4c19a: blessing, variant `v`.
pub fn blessing(g: &mut Ctx, p: Player, v: u8) {
    cov!();
    effect(g, p, kind::BLESSING, Some(v), |_, _| {});
}

/// 0x4c26c: turning away of monsters. Without a targeted monster (`None`): a single effect; otherwise four
/// (parameters 3 to 0)
/// that follow the monster `foe`.
pub fn banish(g: &mut Ctx, p: Player, foe: Option<u32>) {
    cov!();
    let Some(foe) = foe else {
        effect(g, p, kind::BANISH, None, |_, _| {});
        return;
    };
    for param in (0..4).rev() {
        let made = effect(g, p, kind::BANISH, Some(1), |g, o| {
            o.set_u8(g.b, field::PARAM, param);
            o.set_u16(g.b, 0x5c, foe as u16);
        });
        if made.is_none() {
            return;
        }
    }
}

/// 0x4c58e: weapon trail (weapons 0x10 to 0x13), variant `v`; the parameter is the weapon number.
pub fn weapon_trail(g: &mut Ctx, p: Player, v: u8) {
    cov!();
    let weapon = p.u8(g, Player::WEAPON_A).wrapping_sub(0x10);
    if weapon > 3 {
        return;
    }
    effect(g, p, kind::WEAPON_TRAIL, Some(v), |g, o| o.set_u8(g.b, field::PARAM, weapon));
}

/// 0x4dcda: spell downpour, side `side`.
pub fn shower(g: &mut Ctx, p: Player, side: u8) {
    cov!();
    let step = step_of(g, p);
    effect(g, p, kind::SHOWER, Some(1), |g, o| {
        o.set_u8(g.b, field::PARAM, side);
        o.set_u8(g.b, 0x5d, step);
    });
}

/// 0x4e6a8: prayer aura, variant `v`.
pub fn aura(g: &mut Ctx, p: Player, v: u8) {
    cov!();
    let step = step_of(g, p);
    effect(g, p, kind::AURA, Some(v), |g, o| o.set_u8(g.b, 0x59, step));
}

/// 0x4f004: lightning target, variant `v`. Returns the created effect (or nothing if the pool is full).
pub fn lightning_target(g: &mut Ctx, p: Player, v: u8) -> Option<Obj> {
    cov!();
    effect(g, p, kind::LIGHTNING_TARGET, Some(v), |_, _| {})
}

/// 0x4f272: swarm of `count` objects, as long as some remain in the scene's reserve.
pub fn swarm(g: &mut Ctx, p: Player, count: u32) {
    cov!();
    const RESERVE: u32 = 0x5fc0;
    for n in (0..count).rev() {
        let left = rd16(g.b, A5.wrapping_sub(RESERVE));
        if left == 0 {
            return;
        }
        wr16(g.b, A5.wrapping_sub(RESERVE), left - 1);
        let scene = ram::SCENE.get(g.b);
        let made = create(g, p, &pools::PEFF, kind::SWARM);
        let Some(o) = made else { return };
        o.set_u16(g.b, field::LINK, p.0 as u16);
        o.set_u8(g.b, Obj::VARIANT, n as u8);
        o.set_u8(g.b, field::SCENE, scene);
    }
}

/// 0x4fbde: leap effect (variant `v`), except with weapons 0x10 to 0x12.
pub fn leap(g: &mut Ctx, p: Player, v: u8) {
    cov!();
    if matches!(p.u8(g, Player::WEAPON_A), 0x10..=0x12) {
        return;
    }
    let step = step_of(g, p);
    effect(g, p, kind::LEAP, Some(v), |g, o| o.set_u8(g.b, 0x5c, step));
}

/// 0x4fd2c: equipment swap, variant `v`.
pub fn swap(g: &mut Ctx, p: Player, v: u8) {
    cov!();
    effect(g, p, kind::SWAP, Some(v), |_, _| {});
}

/// 0x4fe18: dwarf's long slide; the object keeps the current sub-action.
pub fn dash(g: &mut Ctx, p: Player) {
    cov!();
    let sub = p.u8(g, Player::SUBACTION);
    effect(g, p, kind::DASH, None, |g, o| o.set_u8(g.b, 0x5c, sub));
}

/// 0x50324: wall target; the scene's wall reserve (`A5 - 0x5fbe`) counts one more. Returns what the caller tests:
/// Z clear (the counter's `addq.b` does not reach zero) and A1, the target. Original glitch: when the pool is full, the
/// allocation leaves Z clear and A1 on the first slot or the end of the pool (`Pool::alloc_or_leftover`), which the
/// caller takes as a created target.
pub fn wall_target(g: &mut Ctx, p: Player) -> (bool, Obj) {
    cov!();
    let scene = ram::SCENE.get(g.b);
    let o = match pools::PEFF.alloc_or_leftover(g.b) {
        Ok(o) => o,
        Err(leftover) => return (true, leftover),
    };
    o.set_u8(g.b, Obj::ACTIVE, 1);
    o.set_u8(g.b, Obj::KIND, kind::WALL_TARGET);
    o.set_u8(g.b, field::SCENE, scene);
    link(g, o, p);
    let walls = A5.wrapping_sub(0x5fbe);
    let new = rd8(g.b, walls).wrapping_add(1);
    wr8(g.b, walls, new);
    (new != 0, o)
}

/// 0x4cdd0: bolt on the target `target`, at its position, 0x40 above.
pub fn lightning(g: &mut Ctx, p: Player, target: u32) {
    cov!();
    let step = step_of(g, p);
    let facing = p.u8(g, Player::FACING);
    let created = create(g, p, &pools::PEFF, kind::LIGHTNING);
    let Some(o) = created else { return };
    let (x, z, y) = (rd16(g.b, target + Obj::X), rd16(g.b, target + Obj::Z), rd16(g.b, target + Obj::Y));
    o.set_u16(g.b, field::LINK, p.0 as u16);
    o.set_u8(g.b, Player::FACING, facing);
    o.set_u16(g.b, Obj::X, x);
    o.set_u16(g.b, Obj::Z, z);
    o.set_u16(g.b, Obj::Y, y);
    o.set_u16(g.b, Obj::Z, z.wrapping_add(0x40));
    o.set_u8(g.b, 0x5d, step);
}

/// 0x4cd8e: storm bolt, at the player's position, 0x48 higher and just in front of them.
pub fn flare(g: &mut Ctx, p: Player) {
    cov!();
    let step = step_of(g, p);
    let Some(o) = create(g, p, &pools::PEFF, kind::LIGHTNING) else { return };
    o.set_u16(g.b, field::LINK, p.0 as u16);
    let facing = p.u8(g, Player::FACING);
    o.set_u8(g.b, Player::FACING, facing);
    for axis in [Obj::X, Obj::Z, Obj::Y] {
        let v = p.u16(g, axis);
        o.set_u16(g.b, axis, v);
    }
    let z = o.u16(g.b, Obj::Z).wrapping_add(0x48);
    o.set_u16(g.b, Obj::Z, z);
    let y = o.u16(g.b, Obj::Y).wrapping_sub(1);
    o.set_u16(g.b, Obj::Y, y);
    o.set_u8(g.b, 0x5d, step);
}

/// 0x4ccac: summoned creature, turned like the player.
pub fn summon(g: &mut Ctx, p: Player) {
    cov!();
    let Some(o) = create(g, p, &pools::PEFF, kind::SUMMON) else { return };
    let facing = p.u8(g, Player::FACING);
    o.set_u8(g.b, Player::FACING, facing);
    link(g, o, p);
}

/// 0x4d3c6: portal, variant `v`.
pub fn portal(g: &mut Ctx, p: Player, v: u8) {
    cov!();
    effect(g, p, kind::PORTAL, Some(v), |_, _| {});
}

// ---- effects of the cast spells ---------------------------------------------------------------------

/// 0x4e2f4: ring (spell 16), variant `v`; the object keeps the action step.
pub fn ring(g: &mut Ctx, p: Player, v: u8) {
    cov!();
    let step = step_of(g, p);
    effect(g, p, kind::RING, Some(v), |g, o| o.set_u8(g.b, 0x5b, step));
}

/// 0x4b196: ray (spell 17), variant `v`.
pub fn beam_cast(g: &mut Ctx, p: Player, v: u8) {
    cov!();
    effect(g, p, kind::BEAM_CAST, Some(v), |g, o| o.set_u8(g.b, field::PARAM, 0));
}

/// 0x4ec66: storm (spell 18), variant `v`.
pub fn storm(g: &mut Ctx, p: Player, v: u8) {
    cov!();
    effect(g, p, kind::STORM, Some(v), |_, _| {});
}

/// 0x5012e: meteors (spell 20).
pub fn meteors(g: &mut Ctx, p: Player) {
    cov!();
    effect(g, p, kind::METEORS, None, |_, _| {});
}

/// 0x4bb68: freeze (spell 15), ice columns: four effects (parameters 0 to 3, offsets 1, 9, 17, 25).
pub fn ice_columns(g: &mut Ctx, p: Player) {
    cov!();
    for n in 0..4u8 {
        let made = effect(g, p, kind::ICE, Some(0), |g, o| {
            o.set_u16(g.b, 0x54, 1 + 8 * n as u16);
            o.set_u8(g.b, field::PARAM, n);
        });
        if made.is_none() {
            return;
        }
    }
}

/// The freeze (spell 15, one part): frozen ground, columns, mist and shards, created one after the other even if one
/// fails.
pub fn ice_spell(g: &mut Ctx, p: Player) {
    cov!();
    ice_ground(g, p);
    ice_columns(g, p);
    ice_mist(g, p);
    ice_shards(g, p);
}

/// 0x4bbbe: freeze, ice mist (variant 2).
pub fn ice_mist(g: &mut Ctx, p: Player) {
    cov!();
    effect(g, p, kind::ICE, Some(2), |_, _| {});
}

/// 0x4bbf6: freeze, six shards (parameters 0, 2... 10) born at the caster's position, one pixel lower.
pub fn ice_shards(g: &mut Ctx, p: Player) {
    cov!();
    let facing = p.u8(g, Player::FACING);
    let (x, z, y) = (p.u16(g, Obj::X), p.u16(g, Obj::Z), p.u16(g, Obj::Y));
    for n in 0..6u8 {
        let created = create(g, p, &pools::PEFF, kind::ICE);
        let Some(o) = created else { return };
        o.set_u8(g.b, Obj::VARIANT, 4);
        o.set_u8(g.b, Player::FACING, facing);
        o.set_u8(g.b, field::PARAM, 2 * n);
        o.set_u16(g.b, 0x54, 2 + 2 * n as u16);
        o.set_u16(g.b, Obj::X, x);
        o.set_u16(g.b, Obj::Z, z);
        o.set_u16(g.b, Obj::Y, y.wrapping_sub(1));
    }
}

/// 0x4bc42: freeze, last effect (variant 5, keeps the action step).
pub fn ice_crown(g: &mut Ctx, p: Player) {
    cov!();
    let step = step_of(g, p);
    effect(g, p, kind::ICE, Some(5), |g, o| o.set_u8(g.b, 0x56, step));
}

// ---- player shot creators ---------------------------------------------------------------------

/// The eight thrown-item shots (special attack, numbers 0 to 8 except 3), in the order of their original addresses:
/// 0x51d02, 0x51d54, 0x51f6a, 0x51fb6, 0x52002, 0x52204, 0x5308c, 0x52394.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Thrown {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
}

/// Description of a thrown shot: kind, powers (against ordinary enemies and against bosses), initial horizontal speed
/// in the facing direction (shots F, G and H have none: their handler places them), the shot copies
/// the caster's orientation or (A and B) chooses an animation direction.
struct Shot {
    kind: u8,
    power: (u8, u8),
    speed: Option<u16>,
    copies_facing: bool,
}

impl Thrown {
    fn shot(self) -> Shot {
        let (kind, power, speed, copies_facing) = match self {
            Thrown::A => (1, (0xc, 0xc), Some(0x700), false),
            Thrown::B => (9, (0x12, 0x12), Some(0x700), false),
            Thrown::C => (2, (0xc, 6), Some(0xa00), true),
            Thrown::D => (0xa, (0x12, 0x12), Some(0xa00), true),
            Thrown::E => (0xb, (0x10, 0x10), Some(0xa00), true),
            Thrown::F => (3, (0x10, 0x10), None, false),
            Thrown::G => (0xc, (0x14, 0x14), None, false),
            Thrown::H => (4, (0xe, 0xe), None, false),
        };
        Shot { kind, power, speed, copies_facing }
    }
}

/// 0x51d02 to 0x5308c: shot of a thrown item. Returns false if the pool is full (D0 = 0 and Z set as in the original).
pub fn thrown(g: &mut Ctx, p: Player, which: Thrown) -> bool {
    cov!();
    let shot = which.shot();
    let Some(o) = create(g, p, &pools::PSHL, shot.kind) else {
        return false;
    };
    o.set_u8(g.b, 0x2a, 2);
    o.set_u8(g.b, 0x45, shot.power.0);
    o.set_u8(g.b, 0x46, shot.power.1);
    let facing = p.u8(g, Player::FACING);
    if shot.copies_facing {
        o.set_u8(g.b, Player::FACING, facing);
    }
    o.set_u16(g.b, field::LINK, p.0 as u16);
    let Some(speed) = shot.speed else {
        return true;
    };
    o.set_u16(g.b, Obj::VX, speed);
    o.set_u16(g.b, Obj::AX, 0);
    o.set_u32(g.b, Obj::VZ, 0);
    // shots A and B choose their animation direction (2 to the right, 6 to the left)
    let directional = !shot.copies_facing;
    if directional {
        o.set_u8(g.b, Obj::SPEED_INDEX, 2);
    }
    if facing == 0 {
        if directional {
            o.set_u8(g.b, Obj::SPEED_INDEX, 6);
        }
        o.set_u16(g.b, Obj::VX, speed.wrapping_neg());
    }
    true
}

/// Magic-user wands: wand reserve (`+0x18b..+0x18d`), its counter of living objects (RAM word) and the
/// SET object number the interface displays when the reserve drops to 5.
struct Rod {
    counter: u32,
    kind: u8,
    reserve: u32,
    interface: u32,
}

const ROD_FLAME: Rod = Rod { counter: 0xff_2046, kind: 0xe, reserve: 0x18c, interface: 0xe };
const ROD_STAR: Rod = Rod { counter: 0xff_204a, kind: 0x10, reserve: 0x18d, interface: 0xd };
const ROD_BALL: Rod = Rod { counter: 0xff_2048, kind: 0xf, reserve: 0x18b, interface: 0xc };

/// Common end of the wands (0x533de): one charge less, and at 5 the interface is notified (0xcc352, D0).
fn rod_used(g: &mut Ctx, p: Player, rod: &Rod) {
    cov!();
    let left = p.u8(g, rod.reserve).wrapping_sub(1);
    p.set_u8(g, rod.reserve, left);
    if left == 5 {
        crate::families::creators::for_players::set_a(&mut g.leftovers, g.b, p.obj(), rod.interface as u8);
    }
}

/// 0x5336a (flames, kind 0xe) and 0x535f2 (stars, kind 0x10): a row of shots in front of the magic-user, as long as no
/// previous row is alive. The number of shots comes from the level (bytes at `counts`), each 0x28 pixels further
/// than the previous one, with its variant and a delay (`+0x5c`) increasing. A full pool stops the row.
fn rod_row(g: &mut Ctx, p: Player, rod: &Rod, first_offset: u16, counts: u32) {
    cov!();
    if rd16(g.b, rod.counter) != 0 {
        return;
    }
    let forward = p.u8(g, Player::FACING) != 0;
    let (mut variant, mut delay) = (0u8, 1u8);
    let mut offset = if forward { first_offset } else { first_offset.wrapping_neg() };
    let level = p.u8(g, Player::LEVEL) as u32;
    // `subq.w #1` on a zero count would give 65,536 iterations: the full pool stops them
    let mut left = (pc8(g.b, counts + level) as u16).wrapping_sub(1);
    while let Some(o) = create(g, p, &pools::PSHL, rod.kind) {
        let alive = rd16(g.b, rod.counter);
        wr16(g.b, rod.counter, alive.wrapping_add(1));
        let (facing, x, y) = (p.u8(g, Player::FACING), p.u16(g, Obj::X), p.u16(g, Obj::Y));
        o.set_u8(g.b, Obj::VARIANT, variant);
        o.set_u8(g.b, 0x5c, delay);
        o.set_u8(g.b, 0x2c, facing);
        o.set_u16(g.b, Obj::X, x.wrapping_add(offset));
        o.set_u16(g.b, Obj::Y, y);
        o.set_u16(g.b, field::LINK, p.0 as u16);
        delay = delay.wrapping_add(6);
        variant = variant.wrapping_add(1);
        offset = offset.wrapping_add(if forward { 0x28 } else { 0x28u16.wrapping_neg() });
        left = left.wrapping_sub(1);
        if left == 0xffff {
            break;
        }
    }
    rod_used(g, p, rod);
}

/// The three wand shots (`magic_user::rod_shot`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RodShot {
    /// 0x5336a: a row of flames.
    Flame,
    /// 0x53478: a ball.
    Ball,
    /// 0x535f2: a row of stars.
    Star,
}

/// Shot of the held wand.
pub fn rod_shot(g: &mut Ctx, p: Player, shot: RodShot) {
    match shot {
        RodShot::Flame => rod_row(g, p, &ROD_FLAME, 0x38, 0x533f4),
        RodShot::Ball => rod_ball(g, p),
        RodShot::Star => rod_row(g, p, &ROD_STAR, 0x48, 0x5367c),
    }
}

/// 0x53478: wand ball (kind 0xf): at most as many alive as the level indicates (bytes at 0x534b8). An allocation
/// failure does not consume a charge.
fn rod_ball(g: &mut Ctx, p: Player) {
    cov!();
    let level = p.u8(g, Player::LEVEL) as u32;
    let most = pc8(g.b, 0x534b8 + level) as i16;
    let alive = rd16(g.b, ROD_BALL.counter) as i16;
    if most <= alive {
        return;
    }
    let Some(o) = create(g, p, &pools::PSHL, ROD_BALL.kind) else { return };
    let alive = rd16(g.b, ROD_BALL.counter);
    wr16(g.b, ROD_BALL.counter, alive.wrapping_add(1));
    o.set_u16(g.b, field::LINK, p.0 as u16);
    rod_used(g, p, &ROD_BALL);
}

/// 0x54522: fireball (spell 14), shot of kind 0x16.
pub fn fireball(g: &mut Ctx, p: Player) {
    cov!();
    let Some(o) = create(g, p, &pools::PSHL, kind::FIREBALL) else { return };
    link(g, o, p);
}

/// 0x52b54: freeze, frozen ground (spell 15): two shots of kind 7 (variants 0 and 1). Returns false if the pool is
/// full.
pub fn ice_ground(g: &mut Ctx, p: Player) -> bool {
    cov!();
    for v in 0..2 {
        let Some(o) = create(g, p, &pools::PSHL, kind::ICE_GROUND) else {
            return false;
        };
        o.set_u8(g.b, Obj::VARIANT, v);
        link(g, o, p);
    }
    true
}

/// 0x54426: thrown knife (shot of kind 0x15, powers 10 and 5). Returns false if the pool is full.
pub fn knife(g: &mut Ctx, p: Player) -> bool {
    cov!();
    let Some(o) = create(g, p, &pools::PSHL, kind::KNIFE) else {
        return false;
    };
    o.set_u8(g.b, 0x45, 0xa);
    o.set_u8(g.b, 0x46, 5);
    link(g, o, p);
    true
}

/// 0x52e20: six shadows of the backward kick (shots of kind 8, variants 5 to 0).
pub fn shadows(g: &mut Ctx, p: Player) {
    cov!();
    for v in (0..6).rev() {
        let Some(o) = create(g, p, &pools::PSHL, kind::SHADOW) else { return };
        o.set_u8(g.b, Obj::VARIANT, v);
        link(g, o, p);
    }
}

/// Shadow of the jumping kick (shot of kind 8, parameter 2), as created by the thief's 0x49212.
pub fn shadow(g: &mut Ctx, p: Player) {
    cov!();
    let Some(o) = create(g, p, &pools::PSHL, kind::SHADOW) else { return };
    o.set_u8(g.b, field::PARAM, 2);
    link(g, o, p);
}

/// 0x53918: the witch, five shots (parameters 0 to 4); the byte `+0x2d` of each comes from the table 0x5394e.
pub fn witch(g: &mut Ctx, p: Player) {
    cov!();
    for n in 0..5u8 {
        let Some(o) = create(g, p, &pools::PSHL, kind::WITCH) else { return };
        let facing = p.u8(g, Player::FACING);
        o.set_u8(g.b, Player::FACING, facing);
        o.set_u16(g.b, field::LINK, p.0 as u16);
        let slot = player_data().spawn_slots.at8(g.b, n as u32);
        o.set_u8(g.b, Obj::SPEED_INDEX, slot);
        o.set_u8(g.b, 0x5d, n);
    }
}

/// 0x53954 (variant 1) and 0x539e4 (variant 4): witch's hits; their powers (against ordinary enemies
/// and against bosses) depend on the player's level (`+0x10c`).
pub fn witch_strike(g: &mut Ctx, p: Player, v: u8) {
    cov!();
    let Some(o) = create(g, p, &pools::PSHL, kind::WITCH) else { return };
    o.set_u8(g.b, Obj::VARIANT, v);
    o.set_u16(g.b, field::LINK, p.0 as u16);
    let level = p.u8(g, Player::LEVEL) as u32;
    let (power, boss) = (players("magic_damage/2").at8(g.b, level), players("magic_damage/2").at8(g.b, 30 + level));
    o.set_u8(g.b, 0x45, power);
    o.set_u8(g.b, 0x46, boss);
}

// ---- creators without a native until now (reachable, never exercised by the scenarios) --------------------

/// 0x4fefe: the cursed weapon that escapes from the hands (`checks::item_curse`), with the current sub-action
/// (`+0x5c`).
pub fn cursed_weapon(g: &mut Ctx, p: Player) {
    cov!();
    let sub = p.u8(g, Player::SUBACTION);
    effect(g, p, kind::CURSED_WEAPON, None, |g, o| o.set_u8(g.b, 0x5c, sub));
}

/// Copies the player's orientation and position into the shard, a bit in front of them (depth + 1).
fn shard_place(g: &mut Ctx, p: Player, o: Obj) {
    let facing = p.u8(g, Player::FACING);
    o.set_u8(g.b, Player::FACING, facing);
    let (x, z) = (p.u16(g, Obj::X), p.u16(g, Obj::Z));
    o.set_u16(g.b, Obj::X, x);
    o.set_u16(g.b, Obj::Z, z);
    let y = p.u32(g, Obj::Y).wrapping_add(1);
    o.set_u32(g.b, Obj::Y, y);
}

/// 0x5009e: shards of the dying player (`effects::dying_countdown`): the first takes the frame and palette of the
/// player (`+0x34`, `+0x3c`, `+0x3f`); a second (variant 1), which holds the player (`+0x42`), if `+0x42` is
/// non-zero. Nothing without `+0x68`, nor during a scenario.
pub fn dying_shards(g: &mut Ctx, p: Player) {
    cov!();
    if p.u8(g, Player::ARMED) == 0 || ram::INPUT_RAW.get(g.b) != 0 {
        return;
    }
    let Some(o) = create(g, p, &pools::PEFF, kind::SHARD) else { return };
    shard_place(g, p, o);
    let frame = p.u32(g, Obj::FRAME);
    o.set_u32(g.b, Obj::FRAME, frame);
    for off in [0x3c, 0x3f] {
        let v = p.u8(g, off);
        o.set_u8(g.b, off, v);
    }
    if p.u16(g, Player::SHIELD_LOW_FRAME) == 0 {
        return;
    }
    let Some(o) = create(g, p, &pools::PEFF, kind::SHARD) else { return };
    o.set_u8(g.b, Obj::VARIANT, 1);
    shard_place(g, p, o);
    o.set_u16(g.b, 0x42, p.0 as u16);
}

/// 0x51aa4: storm swarm (kind 0x2e), in two halves: the second (variant 1) turned to the left.
pub fn storm_cloud(g: &mut Ctx, p: Player) {
    cov!();
    if create(g, p, &pools::PEFF, kind::STORM_CLOUD).is_none() {
        return;
    }
    let Some(o) = create(g, p, &pools::PEFF, kind::STORM_CLOUD) else { return };
    o.set_u8(g.b, Obj::VARIANT, 1);
    o.set_u8(g.b, Player::FACING, 0x20);
}

/// 0x51872: portal glow (kind 0x2d) above the player (0x48 higher, a bit in front), with the action
/// step (`+0x5d`).
pub fn portal_glow(g: &mut Ctx, p: Player) {
    cov!();
    let Some(o) = create(g, p, &pools::PEFF, kind::PORTAL_GLOW) else { return };
    o.set_u16(g.b, field::LINK, p.0 as u16);
    let facing = p.u8(g, Player::FACING);
    o.set_u8(g.b, Player::FACING, facing);
    for off in [Obj::X, Obj::Z, Obj::Y] {
        let v = p.u16(g, off);
        o.set_u16(g.b, off, v);
    }
    let z = o.u16(g.b, Obj::Z).wrapping_add(0x48);
    o.set_u16(g.b, Obj::Z, z);
    let y = o.u16(g.b, Obj::Y).wrapping_sub(1);
    o.set_u16(g.b, Obj::Y, y);
    let step = step_of(g, p);
    o.set_u8(g.b, 0x5d, step);
}

/// 0x4cfee: four portal sparks (kind 0x13, variant 1, numbers 3 to 0 at `+0x2f`), then the screen takes the
/// bolt color (0x1ac68), even if the pool is full.
pub fn portal_sparks(g: &mut Ctx, p: Player) {
    cov!();
    for n in (0..=3u8).rev() {
        let Some(o) = create(g, p, &pools::PEFF, kind::PORTAL_SPARK) else { break };
        o.set_u8(g.b, Obj::VARIANT, 1);
        o.set_u8(g.b, field::PARAM, n);
        o.set_u8(g.b, 0x5a, 4);
        link(g, o, p);
    }
    crate::palette::set_flash_color(g.b, crate::palette::FLASH_COLOR_RED);
}

/// 0x4d020: effect of the storm spell cast to the left (kind 0x14, handler 0x4d08c), linked to the player.
pub fn storm_spell(g: &mut Ctx, p: Player) {
    cov!();
    effect(g, p, kind::STORM_SPELL, None, |_, _| {});
}

/// 0x4d660: glow of the rain (kind 0x17), variant `v` (0: preparation, 1: cast), with the action step
/// of the player (`+0x5d`).
pub fn rain_glow(g: &mut Ctx, p: Player, v: u8) {
    cov!();
    let step = step_of(g, p);
    effect(g, p, kind::RAIN_GLOW, Some(v), |g, o| o.set_u8(g.b, 0x5d, step));
}

/// 0x4d2c0: spark of the wall of fire (kind 0x15), variant `v` (0: preparation, 1: cast).
pub fn wall_spark(g: &mut Ctx, p: Player, v: u8) {
    cov!();
    effect(g, p, kind::WALL_SPARK, Some(v), |_, _| {});
}

/// 0x4dfe2: wave lightning (kind 0x1a) with the player's action step (`+0x59`). `columns` false: the
/// column (variant left by the allocation), then the player's word `+0x12` becomes 0x8000 if it could be
/// created; true: two effects of variant 1 (parameters 1 then 0).
pub fn wave_bolt(g: &mut Ctx, p: Player, columns: bool) {
    cov!();
    let step = step_of(g, p);
    if !columns {
        if effect(g, p, kind::WAVE_BOLT, None, |g, o| o.set_u8(g.b, 0x59, step)).is_some() {
            p.set_u16(g, Player::Y_FRACTION, 0x8000);
        }
        return;
    }
    for param in [1, 0] {
        let made = effect(g, p, kind::WAVE_BOLT, Some(1), |g, o| {
            o.set_u8(g.b, field::PARAM, param);
            o.set_u8(g.b, 0x59, step);
        });
        if made.is_none() {
            return;
        }
    }
}

/// 0x4e4de: elf's moon (kind 0x1c), variant `v` (0, 2 and 3: the three placed moons; 1: the strike).
pub fn moon(g: &mut Ctx, p: Player, v: u8) {
    cov!();
    effect(g, p, kind::MOON, Some(v), |_, _| {});
}

/// 0x4e782: second effect of the elf's moon (kind 0x1e).
pub fn moon_b(g: &mut Ctx, p: Player) {
    cov!();
    effect(g, p, kind::MOON_B, None, |_, _| {});
}

/// 0x4d064: portal shard (kind 0x14, variant 1) numbered `n` (`+0x5e`). True if it could be created (D0 = 0).
pub fn portal_burst(g: &mut Ctx, p: Player, n: u8) -> bool {
    cov!();
    effect(g, p, kind::PORTAL_BURST, Some(1), |g, o| o.set_u8(g.b, 0x5e, n)).is_some()
}

/// 0x54274: rain of bolts (PSHL shots of kind 0x14, variants 0, 1, 2...): one bolt every 0x40 pixels, from
/// 0x80 pixels behind the player, up to the screen edge in front of them (left camera edge - 0x40,
/// or + 0x1c0 when facing left). The positions are not written: only the number of bolts depends on them.
pub fn rain(g: &mut Ctx, p: Player) {
    cov!();
    let left = p.faces_left(g);
    let toward = |d: u16| if left { d.wrapping_neg() } else { d };
    let mut x = p.u16(g, Obj::X).wrapping_add(toward(0xff80));
    let camera = if ram::USE_ALT_CAMERA.get(g.b) != 0 { ram::CAMERA_ALT_X.get(g.b) } else { ram::CAMERA_X.get(g.b) };
    let edge = camera.wrapping_add(if left { 0x1c0 } else { 0xffc0 });
    let mut n = 0u8;
    loop {
        let Some(o) = create(g, p, &pools::PSHL, kind::RAIN) else { return };
        let facing = p.u8(g, Player::FACING);
        o.set_u8(g.b, Player::FACING, facing);
        o.set_u8(g.b, Obj::VARIANT, n);
        o.set_u16(g.b, field::LINK, p.0 as u16);
        x = x.wrapping_add(toward(0xffc0));
        if (toward(x.wrapping_sub(edge)) as i16) < 0 {
            return;
        }
        n = n.wrapping_add(1);
    }
}
