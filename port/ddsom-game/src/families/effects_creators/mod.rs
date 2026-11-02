//! Effect (EFFCT) creators: the small routines of the family that allocate an effect, set its type and variant, then
//! link it to their creator. Called by the handlers of players, enemies, bosses, scenery and scenes.
//!
//! Each creator is an ordinary Rust function (creator, variant, setting as arguments; the created effect returned,
//! `None` if the pool is full), written once on a [`leftovers::Regs`]: the core ([`leftovers::Pure`]) touches no
//! register; in the tools, the native (for the original code) passes the processor, which receives what the original
//! leaves: A1 = the created effect, the carry set if the pool is full (callers test `bcs`), the flags of the last
//! write, the work registers (loop counters, draws).

use cps2::Cps2Bus;

use crate::mem::{prog16, prog32, rd16, rd32, rd8, wr16, wr32, A5};
use crate::objects::pools::{self, Pool};
use crate::objects::Obj;
use leftovers::{Pure, Regs};

mod scenery;
pub use scenery::*;

pub(crate) mod leftovers;


/// Work RAM variables read by the routines of the batch (pending `ram.rs`).
pub(crate) mod var {
    use crate::ram::{Word, Byte};
    /// Low word of the frame counter (`A5+0x6c`, long): the phase of the "one frame out of n" effects.
    pub const IMAGES_WORD_LOW: Word = Word::a5(0x6e);
    /// Living players (`A5+0x35`, `ram::DIFFICULTE`, updated by 0xcc18e).
    pub const PLAYERS_ALIVE: Byte = Byte::a5(0x35);
}

/// Players: four records of 0x200 bytes.
const PLAYERS: u32 = A5 + 0x5de;
const PLAYER_STRIDE: u32 = 0x200;
/// Alternate camera (x, depth).
const ALT_CAMERA_X: u32 = A5 + 0x556;
const ALT_CAMERA_Y: u32 = A5 + 0x55a;

/// Effect types created here.
pub(crate) mod kind {
    pub const SPARK: u8 = 4;
    pub const HALO: u8 = 5;
    pub const DEBRIS: u8 = 6;
    pub const FLASH: u8 = 7;
    pub const TRAIL: u8 = 9;
    pub const ATTACK: u8 = 11;
    pub const TWINKLE: u8 = 15;
    pub const INTERFACE: u8 = 17;
    pub const STRIP: u8 = 21;
    pub const VOTE: u8 = 23;
    pub const PICKUP: u8 = 27;
    pub const POSED_SPARK: u8 = 28;
    pub const IMPACT: u8 = 29;
    pub const FISH: u8 = 30;
    pub const COUNTDOWN: u8 = 31;
    pub const MIRRORED: u8 = 35;
    pub const BLOW: u8 = 40;
    pub const TOSS: u8 = 41;
    pub const SCENERY: u8 = 42;
    pub const DEBRIS_RAIN: u8 = 43;
    pub const SPELL: u8 = 44;
    pub const TYPE_46: u8 = 46;
    pub const BEACON: u8 = 47;
    pub const TYPE_48: u8 = 48;
    pub const PROJECTILE: u8 = 51;
    pub const SHIMMER: u8 = 52;
    pub const TYPE_53: u8 = 53;
    pub const TYPE_54: u8 = 54;
    pub const LAUNCHER: u8 = 55;
    pub const SHARDS: u8 = 56;
    pub const TYPE_57: u8 = 57;
    pub const PILLAR: u8 = 67;
    pub const TYPE_68: u8 = 68;
    pub const VICTORY: u8 = 73;
    pub const BUBBLE: u8 = 74;
    pub const TIMER: u8 = 76;
    pub const SOUND_FADE: u8 = 77;
    pub const RAIN: u8 = 83;
    pub const FINALE: u8 = 84;
    pub const WISP: u8 = 86;
    pub const DRIFT: u8 = 90;
    pub const THROWN_SHARD: u8 = 81;
    pub const POSED_FLASH: u8 = 82;
    /// Enemy shots (ESHL) created by two routines of the family.
    pub const ESHL_RISE: u8 = 0x47;
    pub const ESHL_SLASH: u8 = 0x48;
}

/// Links of an effect to its creator (address word, or long for `DRIFT`).
mod link {
    pub const OWNER: u32 = 0x50;
    pub const SECOND: u32 = 0x54;
    pub const CREATOR: u32 = 0x5c;
    pub const SCENERY: u32 = 0x46;
}
/// Setting specific to the type.
const SETTING: u32 = 0x2f;
const TURNED: u32 = 0x2c;

/// Link `off` of `o` to `target` (`move.w An,(off,A1)`), with the flags of the `move`.
fn link(r: &mut impl Regs, b: &mut Cps2Bus, o: Obj, off: u32, target: u32) {
    o.set_u16(b, off, target as u16);
    r.flags(target & 0xffff, Size::Word);
}

/// `move.b v,(off,A1)` with its flags.
fn put8(r: &mut impl Regs, b: &mut Cps2Bus, o: Obj, off: u32, v: u8) {
    o.set_u8(b, off, v);
    r.flags(v as u32, Size::Byte);
}

/// `move.w v,(off,A1)` with its flags.
fn put16(r: &mut impl Regs, b: &mut Cps2Bus, o: Obj, off: u32, v: u16) {
    o.set_u16(b, off, v);
    r.flags(v as u32, Size::Word);
}

/// Allocates an object and sets its activity and type (`move.b #1,(A1)`, `move.b #type,(2,A1)`).
fn spawn_in(r: &mut impl Regs, b: &mut Cps2Bus, pool: &Pool, kind: u8) -> Option<Obj> {
    let o = r.allocated(b, pool)?;
    o.set_u8(b, Obj::ACTIVE, 1);
    put8(r, b, o, Obj::KIND, kind);
    Some(o)
}

pub(crate) fn spawn(r: &mut impl Regs, b: &mut Cps2Bus, kind: u8) -> Option<Obj> {
    spawn_in(r, b, &pools::EFFCT, kind)
}

/// `addq.b #1,(A1)`: the activity is incremented, not set.
fn bump_active(r: &mut impl Regs, b: &mut Cps2Bus, o: Obj) {
    let v = r.addition(1, o.u8(b, Obj::ACTIVE) as u32, Size::Byte);
    o.set_u8(b, Obj::ACTIVE, v as u8);
}

/// Copies the high words of X, Z, Y of the creator.
fn copy_position(b: &mut Cps2Bus, to: Obj, from: Obj) {
    for off in [Obj::X, Obj::Z, Obj::Y] {
        let v = from.u16(b, off);
        to.set_u16(b, off, v);
    }
}

/// `andi.w #mask,D0` on `v`: keeps the bits of `mask` (D0.w and flags of the result).
fn and_d0(r: &mut impl Regs, v: u16, mask: u16) -> u16 {
    let v = v & mask;
    r.d_word(0, v);
    r.flags(v as u32, Size::Word);
    v
}

// ---------------------------------------------------------------------------------------------
// Simple creators: type, variant, setting `+0x2f`, link to the creator

/// 0xae776: interface element (type 17; cursors of the selection screen...).
pub(crate) fn interface(r: &mut impl Regs, b: &mut Cps2Bus) -> Option<Obj> {
    spawn(r, b, kind::INTERFACE)
}

/// 0xaf346: interface banner (type 21) of variant `variant`.
pub(crate) fn strip(r: &mut impl Regs, b: &mut Cps2Bus, variant: u8) -> Option<Obj> {
    let o = spawn(r, b, kind::STRIP)?;
    put8(r, b, o, Obj::VARIANT, variant);
    Some(o)
}

/// 0xb1a7a (end-of-scene screen, type 31), 0xafe02 (path vote, type 23): variant, setting.
pub(crate) fn with_setting_via(r: &mut impl Regs, b: &mut Cps2Bus, kind: u8, variant: u8, setting: u8) -> Option<Obj> {
    let o = spawn(r, b, kind)?;
    o.set_u8(b, Obj::VARIANT, variant);
    put8(r, b, o, SETTING, setting);
    Some(o)
}

/// 0xab7d8: burst of debris (type 6) of variant `variant`, linked to the creator by `+0x50`.
pub fn debris(b: &mut Cps2Bus, creator: Obj, variant: u8) -> Option<Obj> {
    debris_via(&mut Pure, b, creator, variant)
}
pub(crate) fn debris_via(r: &mut impl Regs, b: &mut Cps2Bus, creator: Obj, variant: u8) -> Option<Obj> {
    let o = spawn(r, b, kind::DEBRIS)?;
    o.set_u8(b, Obj::VARIANT, variant);
    o.set_u8(b, 0x42, 0);
    link(r, b, o, link::OWNER, creator.0);
    Some(o)
}

/// 0xabec8 (attack effect, type 11) and 0xb378a (effects of a hit, type 40): linked to the creator by `+0x5c`, variant
/// and setting.
pub(crate) fn attached_via(r: &mut impl Regs, b: &mut Cps2Bus, kind: u8, creator: Obj, variant: u8, setting: u8) -> Option<Obj> {
    let o = spawn(r, b, kind)?;
    o.set_u16(b, link::CREATOR, creator.word());
    o.set_u8(b, Obj::VARIANT, variant);
    put8(r, b, o, SETTING, setting);
    Some(o)
}

/// 0xabec8: attack effect (type 11) linked to the creator.
pub fn attack(b: &mut Cps2Bus, creator: Obj, variant: u8, setting: u8) -> Option<Obj> {
    attached_via(&mut Pure, b, kind::ATTACK, creator, variant, setting)
}

/// 0xb378a: stars and sparks of a hit (type 40) linked to the creator.
pub fn blow(b: &mut Cps2Bus, creator: Obj, variant: u8, setting: u8) -> Option<Obj> {
    attached_via(&mut Pure, b, kind::BLOW, creator, variant, setting)
}

/// 0xab708: halo (type 5) of the creator's variant, linked by `+0x50`.
pub fn halo(b: &mut Cps2Bus, creator: Obj) -> Option<Obj> {
    halo_via(&mut Pure, b, creator)
}
pub(crate) fn halo_via(r: &mut impl Regs, b: &mut Cps2Bus, creator: Obj) -> Option<Obj> {
    let o = spawn(r, b, kind::HALO)?;
    let v = creator.u8(b, Obj::VARIANT);
    o.set_u8(b, Obj::VARIANT, v);
    link(r, b, o, link::OWNER, creator.0);
    Some(o)
}

/// 0xb1352: placed spark (type 28) of variant `variant`, linked by `+0x5c`.
pub fn posed_spark(b: &mut Cps2Bus, creator: Obj, variant: u8) -> Option<Obj> {
    posed_spark_via(&mut Pure, b, creator, variant)
}
pub(crate) fn posed_spark_via(r: &mut impl Regs, b: &mut Cps2Bus, creator: Obj, variant: u8) -> Option<Obj> {
    let o = spawn(r, b, kind::POSED_SPARK)?;
    o.set_u8(b, Obj::VARIANT, variant);
    link(r, b, o, link::CREATOR, creator.0);
    Some(o)
}

/// 0xb141a: impact spark (type 29, variant 1) with speeds `vx`, `ax`, linked by `+0x5c`. Core.
pub fn impact_spark(b: &mut Cps2Bus, creator: Obj, vx: u16, ax: u16) -> Option<Obj> {
    impact(&mut Pure, b, creator, vx, ax)
}

/// 0xb141a: impact spark (type 29, variant 1) with speeds `vx`, `ax`, linked by `+0x5c`.
pub(crate) fn impact(r: &mut impl Regs, b: &mut Cps2Bus, creator: Obj, vx: u16, ax: u16) -> Option<Obj> {
    let o = spawn(r, b, kind::IMPACT)?;
    o.set_u8(b, Obj::VARIANT, 1);
    o.set_u16(b, Obj::VX, vx);
    o.set_u16(b, Obj::AX, ax);
    link(r, b, o, link::CREATOR, creator.0);
    Some(o)
}

/// 0xb12c0: pickup effect (type 27) of variant `variant`, attached to the same object as the creator (`+0x50`, if it
/// exists: negative pointer).
pub fn pickup(b: &mut Cps2Bus, creator: Obj, variant: u8) -> Option<Obj> {
    pickup_via(&mut Pure, b, creator, variant)
}
pub(crate) fn pickup_via(r: &mut impl Regs, b: &mut Cps2Bus, creator: Obj, variant: u8) -> Option<Obj> {
    let owner = creator.u16(b, link::OWNER);
    r.flags(owner as u32, Size::Word);
    if (owner as i16) >= 0 {
        return None;
    }
    let o = spawn(r, b, kind::PICKUP)?;
    o.set_u8(b, Obj::VARIANT, variant);
    let owner = creator.u16(b, link::OWNER);
    link(r, b, o, link::OWNER, owner as u32);
    Some(o)
}

/// Creator of a fixed-variant type, linked to the creator by `+0x5c` (types 51, 73, 74, 82, 84, 86).
pub(crate) fn linked_via(r: &mut impl Regs, b: &mut Cps2Bus, creator: Obj, kind: u8, variant: Option<u8>) -> Option<Obj> {
    let o = spawn(r, b, kind)?;
    if let Some(v) = variant {
        o.set_u8(b, Obj::VARIANT, v);
    }
    link(r, b, o, link::CREATOR, creator.0);
    Some(o)
}

/// 0xb97a2: placed burst (type 82, variant left as is) linked to the creator.
pub fn posed_flash(b: &mut Cps2Bus, creator: Obj) -> Option<Obj> {
    linked_posed_flash_via(&mut Pure, b, creator)
}
pub(crate) fn linked_posed_flash_via(r: &mut impl Regs, b: &mut Cps2Bus, creator: Obj) -> Option<Obj> {
    linked_via(r, b, creator, kind::POSED_FLASH, None)
}

/// 0xb8956: scene timer (type 76) of variant `variant`.
pub(crate) fn timer(r: &mut impl Regs, b: &mut Cps2Bus, variant: u8) -> Option<Obj> {
    let o = spawn(r, b, kind::TIMER)?;
    put8(r, b, o, Obj::VARIANT, variant);
    Some(o)
}

/// 0xba18a: drifting dust (type 90), oriented like the creator, linked by the long `+0x46`.
pub(crate) fn drift_via(r: &mut impl Regs, b: &mut Cps2Bus, creator: Obj) -> Option<Obj> {
    let o = spawn(r, b, kind::DRIFT)?;
    let facing = creator.u8(b, TURNED);
    o.set_u8(b, TURNED, facing);
    o.set_u32(b, link::SCENERY, creator.0);
    r.flags(creator.0, Size::Long);
    Some(o)
}

/// 0xb66e4: effect of type 53; the activity is incremented.
pub(crate) fn type_53(r: &mut impl Regs, b: &mut Cps2Bus) -> Option<Obj> {
    let o = r.allocated(b, &pools::EFFCT)?;
    bump_active(r, b, o);
    put8(r, b, o, Obj::KIND, kind::TYPE_53);
    Some(o)
}

/// 0xb6a5c: machine (type 55) linked to the creator by `+0x5c`; activity incremented.
pub fn launcher(b: &mut Cps2Bus, creator: Obj) -> Option<Obj> {
    launcher_via(&mut Pure, b, creator)
}
pub(crate) fn launcher_via(r: &mut impl Regs, b: &mut Cps2Bus, creator: Obj) -> Option<Obj> {
    let o = r.allocated(b, &pools::EFFCT)?;
    bump_active(r, b, o);
    o.set_u8(b, Obj::KIND, kind::LAUNCHER);
    link(r, b, o, link::CREATOR, creator.0);
    Some(o)
}

/// 0xb6c42: burst (type 56) placed at (0xc0, 0x50) of the camera; activity incremented.
pub fn camera_shard(b: &mut Cps2Bus) -> Option<Obj> {
    camera_shard_via(&mut Pure, b)
}
pub(crate) fn camera_shard_via(r: &mut impl Regs, b: &mut Cps2Bus) -> Option<Obj> {
    let o = r.allocated(b, &pools::EFFCT)?;
    bump_active(r, b, o);
    o.set_u8(b, Obj::KIND, kind::SHARDS);
    let (x, y) = (crate::ram::CAMERA_X.get(b), crate::ram::CAMERA_Y.get(b));
    o.set_u16(b, Obj::X, x);
    o.set_u16(b, Obj::Y, y);
    o.set_u16(b, Obj::X, x.wrapping_add(0xc0));
    let y = r.addition(0x50, y as u32, Size::Word);
    o.set_u16(b, Obj::Y, y as u16);
    Some(o)
}

/// 0xb2ef6: silhouette and reflection (type 35, variants 0 and 1) of the creator, whose transmitted frame (`+0xa0`) is
/// reset to zero.
pub(crate) fn mirrored_pair_via(r: &mut impl Regs, b: &mut Cps2Bus, creator: Obj) -> Option<Obj> {
    creator.set_u8(b, 0xa0, 0);
    let mut last = None;
    for variant in 0..2u8 {
        let o = spawn(r, b, kind::MIRRORED)?;
        o.set_u8(b, Obj::VARIANT, variant);
        o.set_u8(b, SETTING, 0);
        link(r, b, o, link::SCENERY, creator.0);
        last = Some(o);
    }
    last
}

/// 0xb6ba8: effect of type 57 linked to the creator by `+0x50`: variant, setting.
pub(crate) fn type_57(r: &mut impl Regs, b: &mut Cps2Bus, creator: Obj, variant: u8, setting: u8) -> Option<Obj> {
    let o = r.allocated(b, &pools::EFFCT)?;
    o.set_u8(b, Obj::ACTIVE, 1);
    o.set_u16(b, link::OWNER, creator.word());
    o.set_u8(b, Obj::KIND, kind::TYPE_57);
    o.set_u8(b, Obj::VARIANT, variant);
    put8(r, b, o, SETTING, setting);
    Some(o)
}

/// 0xb68b0: three effects of type 54, variants 0 (left as is), 1 and 2.
pub(crate) fn type_54_trio(r: &mut impl Regs, b: &mut Cps2Bus) -> Option<Obj> {
    let mut last = None;
    for variant in 0..3u8 {
        let o = r.allocated(b, &pools::EFFCT)?;
        bump_active(r, b, o);
        put8(r, b, o, Obj::KIND, kind::TYPE_54);
        if variant != 0 {
            put8(r, b, o, Obj::VARIANT, variant);
        }
        last = Some(o);
    }
    last
}

// ---------------------------------------------------------------------------------------------
// Creators that place the effect on their creator

/// 0xae3ec: sparkle (type 15) of the creator's variant `+0x54`, placed 16 pixels higher, linked by `+0x5c`; nothing if
/// `+0x54` is zero.
pub fn twinkle(b: &mut Cps2Bus, creator: Obj) -> Option<Obj> {
    twinkle_via(&mut Pure, b, creator)
}
pub(crate) fn twinkle_via(r: &mut impl Regs, b: &mut Cps2Bus, creator: Obj) -> Option<Obj> {
    let variant = creator.u8(b, link::SECOND);
    r.flags(variant as u32, Size::Byte);
    if variant == 0 {
        return None;
    }
    let o = spawn(r, b, kind::TWINKLE)?;
    o.set_u8(b, Obj::VARIANT, variant);
    copy_position(b, o, creator);
    let z = o.u16(b, Obj::Z).wrapping_add(0x10);
    o.set_u16(b, Obj::Z, z);
    link(r, b, o, link::CREATOR, creator.0);
    Some(o)
}

/// 0xb6596: character sparkle (type 52) placed right in front of the creator, raised according to its byte `+0x60`
/// (word table 0xb65d6), linked by `+0x5c`. D0 = the rise.
pub fn shimmer(b: &mut Cps2Bus, creator: Obj) -> Option<Obj> {
    shimmer_via(&mut Pure, b, creator)
}
pub(crate) fn shimmer_via(r: &mut impl Regs, b: &mut Cps2Bus, creator: Obj) -> Option<Obj> {
    let o = spawn(r, b, kind::SHIMMER)?;
    copy_position(b, o, creator);
    let y = o.u16(b, Obj::Y).wrapping_sub(1);
    o.set_u16(b, Obj::Y, y);
    o.set_u16(b, link::CREATOR, creator.word());
    let k = 2 * creator.u8(b, 0x60) as u32;
    let lift = prog16(b, 0xb65d6 + k);
    r.d(0, lift as u32);
    let z = r.addition(lift as u32, o.u16(b, Obj::Z) as u32, Size::Word);
    o.set_u16(b, Obj::Z, z as u16);
    Some(o)
}

/// 0xb5730: marker (type 47) in place of the creator: setting `+0x46`, same variant.
pub(crate) fn beacon_via(r: &mut impl Regs, b: &mut Cps2Bus, creator: Obj) -> Option<Obj> {
    let o = spawn(r, b, kind::BEACON)?;
    let setting = creator.u8(b, 0x46);
    o.set_u8(b, SETTING, setting);
    let v = creator.u8(b, Obj::VARIANT);
    o.set_u8(b, Obj::VARIANT, v);
    copy_position(b, o, creator);
    r.flags(creator.u16(b, Obj::Y) as u32, Size::Word);
    Some(o)
}

/// 0xb3c14: thrown object (type 41) from the creator, 24 pixels higher, same orientation.
pub(crate) fn toss_via(r: &mut impl Regs, b: &mut Cps2Bus, creator: Obj) -> Option<Obj> {
    let o = spawn(r, b, kind::TOSS)?;
    let facing = creator.u8(b, TURNED);
    o.set_u8(b, TURNED, facing);
    copy_position(b, o, creator);
    let z = r.addition(0x18, o.u16(b, Obj::Z) as u32, Size::Word);
    o.set_u16(b, Obj::Z, z as u16);
    Some(o)
}

/// 0xaba16: single-animation burst (type 7) of the creator: orientation, full position (longs) right in front of it,
/// setting `+0x2f`.
pub fn flash(b: &mut Cps2Bus, creator: Obj) -> Option<Obj> {
    flash_via(&mut Pure, b, creator)
}
pub(crate) fn flash_via(r: &mut impl Regs, b: &mut Cps2Bus, creator: Obj) -> Option<Obj> {
    let o = spawn(r, b, kind::FLASH)?;
    let facing = creator.u8(b, TURNED);
    o.set_u8(b, TURNED, facing);
    for off in [Obj::X, Obj::Z, Obj::Y] {
        let v = rd32(b, creator.0 + off);
        wr32(b, o.0 + off, v);
    }
    let y = r.subtract(1, o.u32(b, Obj::Y), Size::Long);
    o.set_u32(b, Obj::Y, y);
    let setting = creator.u8(b, SETTING);
    put8(r, b, o, SETTING, setting);
    Some(o)
}

/// 0xaaa40: spark (type 4, variant `variant`) between the creator and `other`, placed at (x, z, y).
pub(crate) fn spark_at_via(r: &mut impl Regs, b: &mut Cps2Bus, creator: Obj, other: u32, variant: u8, (x, z, y): (u16, u16, u16)) -> Option<Obj> {
    let o = spawn(r, b, kind::SPARK)?;
    o.set_u8(b, Obj::VARIANT, variant);
    let facing = creator.u8(b, TURNED) ^ 0x20;
    o.set_u8(b, TURNED, facing);
    o.set_u16(b, link::OWNER, other as u16);
    o.set_u16(b, link::SECOND, creator.word());
    o.set_u16(b, Obj::X, x);
    o.set_u16(b, Obj::Z, z);
    o.set_u16(b, Obj::Y, y);
    r.flags(y as u32, Size::Word);
    Some(o)
}

// ---------------------------------------------------------------------------------------------
// Natives: the original's registers (A0 = the creator, D0/D1... = parameters)

// ---------------------------------------------------------------------------------------------
// Cores of the creators called by the handlers (Rust functions: no registers nor carry; the created effect, `None` if
// the pool is full)

/// 0xaaa40: spark (type 4) between the creator and `other`, placed at (x, z, y).
pub fn spark_at(b: &mut Cps2Bus, creator: Obj, other: u32, variant: u8, pos: (u16, u16, u16)) -> Option<Obj> {
    spark_at_via(&mut Pure, b, creator, other, variant, pos)
}
/// 0xb3df6: two scenery effects (variant 0).
pub fn scenery_0(b: &mut Cps2Bus, creator: Obj) -> Option<Obj> {
    scenery_0_pair_via(&mut Pure, b, creator)
}
/// 0xb3f40: scenery effects of variant 3.
pub fn scenery_3(b: &mut Cps2Bus, creator: Obj) -> Option<Obj> {
    scenery_burst_via(&mut Pure, b, creator, 3, 6, true)
}
/// 0xb3f6a: one frame out of four, puff of variant 4.
pub fn scenery_4(b: &mut Cps2Bus, creator: Obj) -> Option<Obj> {
    scenery_4_via(&mut Pure, b, creator)
}
/// 0xb3fb0: scenery effect of variant 5, setting `setting`.
pub fn scenery_5(b: &mut Cps2Bus, creator: Obj, setting: u8) -> Option<Obj> {
    scenery_via(&mut Pure, b, creator, 5, Some(setting))
}
/// 0xb3fd4: one time out of sixteen, scenery effect of variant 6.
pub fn scenery_6(b: &mut Cps2Bus, creator: Obj) -> Option<Obj> {
    scenery_by_chance_via(&mut Pure, b, creator, 0xf, 6, None)
}
/// 0xb4002: scenery effect of variant 7, zero setting.
pub fn scenery_7(b: &mut Cps2Bus, creator: Obj) -> Option<Obj> {
    scenery_via(&mut Pure, b, creator, 7, Some(0))
}
/// 0xb4026: scenery effects of variant 8.
pub fn scenery_8(b: &mut Cps2Bus, creator: Obj) -> Option<Obj> {
    scenery_burst_via(&mut Pure, b, creator, 8, 9, false)
}
/// 0xb4088: one time out of four, scenery effect of variant 10, zero setting.
pub fn scenery_10(b: &mut Cps2Bus, creator: Obj) -> Option<Obj> {
    scenery_by_chance_via(&mut Pure, b, creator, 3, 10, Some(0))
}
/// 0xb3e4e (`second` false) and 0xb3ec6: cries of a boss.
pub fn shouts(b: &mut Cps2Bus, creator: Obj, second: bool) -> Option<Obj> {
    if second { shouts_via(&mut Pure, b, creator, 2, 0xb3f00) } else { shouts_via(&mut Pure, b, creator, 1, 0xb3e86) }
}
/// 0xb2ef6: silhouette and its reflection.
pub fn mirrored_pair(b: &mut Cps2Bus, creator: Obj) -> Option<Obj> {
    mirrored_pair_via(&mut Pure, b, creator)
}
/// 0xba18a: drift.
pub fn drift(b: &mut Cps2Bus, creator: Obj) -> Option<Obj> {
    drift_via(&mut Pure, b, creator)
}
/// 0xb5730: marker.
pub fn beacon(b: &mut Cps2Bus, creator: Obj) -> Option<Obj> {
    beacon_via(&mut Pure, b, creator)
}
/// 0xb3c14 : lance.
pub fn toss(b: &mut Cps2Bus, creator: Obj) -> Option<Obj> {
    toss_via(&mut Pure, b, creator)
}
/// 0xb4f44: a spell effect, setting `setting`.
pub fn spell(b: &mut Cps2Bus, creator: Obj, setting: u8) -> Option<Obj> {
    spell_via(&mut Pure, b, creator, setting)
}
/// 0xb4ef4 (setting 0), 0xb4f60 (2), 0xb4f1c (4): six spell effects.
pub fn spell_ring(b: &mut Cps2Bus, creator: Obj, setting: u8) -> Option<Obj> {
    spell_ring_via(&mut Pure, b, creator, setting)
}
/// 0xa817c: shot that rises as a column.
pub fn eshl_rise(b: &mut Cps2Bus, creator: Obj) -> Option<Obj> {
    eshl_rise_via(&mut Pure, b, creator)
}
/// 0xa836c: shot of a hit, setting `setting`.
pub fn eshl_slash(b: &mut Cps2Bus, creator: Obj, setting: u8) -> Option<Obj> {
    eshl_slash_via(&mut Pure, b, creator, setting)
}
/// 0xb4bcc: rain of debris.
pub fn debris_rain(b: &mut Cps2Bus, creator: Obj, variant: u8, setting: u8) -> Option<Obj> {
    debris_rain_via(&mut Pure, b, creator, variant, setting)
}
/// 0xb1a7a: end-of-scene screen (type 31).
pub fn countdown(b: &mut Cps2Bus, variant: u8, setting: u8) -> Option<Obj> {
    with_setting_via(&mut Pure, b, kind::COUNTDOWN, variant, setting)
}
/// 0xafe02: path vote (type 23).
pub fn vote(b: &mut Cps2Bus, variant: u8, setting: u8) -> Option<Obj> {
    with_setting_via(&mut Pure, b, kind::VOTE, variant, setting)
}
/// Effect of kind `kind` linked to the creator (0xb84ae victory, 0xb85e2 bubble, 0xb9c6a/0xb9c88 embers...).
pub fn linked(b: &mut Cps2Bus, creator: Obj, kind: u8, variant: Option<u8>) -> Option<Obj> {
    linked_via(&mut Pure, b, creator, kind, variant)
}
/// Effect of kind `kind` without link (0xb8a0a sound fade, 0xb9942 rain...).
pub fn spawn_kind(b: &mut Cps2Bus, kind: u8) -> Option<Obj> {
    spawn(&mut Pure, b, kind)
}

/// Creators called by the player module (`player/`): cores on `r` (the player's leftovers D2-D5).
pub(crate) mod for_players {
    use super::*;

    /// 0xb378a: blow stars (type 40) attached to `me`, variant `variant`, setting `setting`.
    pub(crate) fn blow(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj, variant: u8, setting: u8) -> Option<Obj> {
        attached_via(r, b, kind::BLOW, me, variant, setting)
    }
    /// 0xab708: halo of `me`.
    pub(crate) fn halo(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj) -> Option<Obj> {
        halo_via(r, b, me)
    }
    /// 0xab7d8: debris spray of `me`, variant `variant`.
    pub(crate) fn debris(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj, variant: u8) -> Option<Obj> {
        debris_via(r, b, me, variant)
    }
    /// 0xb6ba8: effect of type 57 of `me`.
    pub(crate) fn type_57(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj, variant: u8, setting: u8) -> Option<Obj> {
        super::type_57(r, b, me, variant, setting)
    }
    /// 0xb97ba: posed flash (variant 1) linked to `me`.
    pub(crate) fn posed_flash_b(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj) -> Option<Obj> {
        linked_via(r, b, me, kind::POSED_FLASH, Some(1))
    }
    /// 0xb96c0: thrown shard of variant `variant` linked to `me`.
    pub(crate) fn thrown_shard(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj, variant: u8) -> Option<Obj> {
        super::thrown_shard(r, b, me, variant)
    }
    /// 0xb9c6a: wisp (type 86, variant 0) linked to `me`.
    pub(crate) fn wisp(r: &mut impl Regs, b: &mut Cps2Bus, me: Obj) -> Option<Obj> {
        linked_via(r, b, me, kind::WISP, Some(0))
    }
}

// ---------------------------------------------------------------------------------------------
// Creators called by the screens (mode machine, scenes): cores without registers

/// The created effect, or the A1 that the original leaves when the EFFCT pool is full (`Pool::alloc_or_leftover`, which
/// writes nothing after a failure): for callers that do not test the carry.
pub fn or_rest(b: &mut Cps2Bus, created: Option<Obj>) -> Obj {
    created.unwrap_or_else(|| pools::EFFCT.alloc_or_leftover(b).expect_err("the EFFCT pool was just found full"))
}

/// 0xae776: interface element (type 17).
pub fn element_d_interface(b: &mut Cps2Bus) -> Option<Obj> {
    interface(&mut Pure, b)
}

/// 0xaf346: interface banner (type 21) of variant `variant`.
pub fn banner(b: &mut Cps2Bus, variant: u8) -> Option<Obj> {
    strip(&mut Pure, b, variant)
}

/// 0xb1a7a: end screen (type 31) of variant `variant`, setting `setting`.
pub fn end_screen(b: &mut Cps2Bus, variant: u8, setting: u8) -> Option<Obj> {
    with_setting_via(&mut Pure, b, kind::COUNTDOWN, variant, setting)
}

/// 0xb9aa0: finale (type 74) linked to the creator.
pub fn finale(b: &mut Cps2Bus, creator: Obj) -> Option<Obj> {
    linked(b, creator, kind::FINALE, None)
}

/// 0xb7f14: effect of type 68.
pub fn type_68_pure(b: &mut Cps2Bus) -> Option<Obj> {
    spawn(&mut Pure, b, kind::TYPE_68)
}

/// 0xb8956: scene timer (type 76) of variant `variant`.
pub fn countdown_timer(b: &mut Cps2Bus, variant: u8) -> Option<Obj> {
    timer(&mut Pure, b, variant)
}

/// 0xb66e4: effect of type 53 (activity incremented).
pub fn type_53_pure(b: &mut Cps2Bus) -> Option<Obj> {
    type_53(&mut Pure, b)
}

/// 0xb9942: rain (type 83).
pub fn rain(b: &mut Cps2Bus) -> Option<Obj> {
    spawn(&mut Pure, b, kind::RAIN)
}

/// 0xb1764: school of fish.
pub fn bench(b: &mut Cps2Bus) -> Option<Obj> {
    fish_school(&mut Pure, b)
}

/// 0xb17ba: swimming fish.
pub fn swimmers(b: &mut Cps2Bus) -> Option<Obj> {
    fish_swimmers(&mut Pure, b)
}

/// 0xabb70: trails of the players.
pub fn trails_pure(b: &mut Cps2Bus) -> Option<Obj> {
    trails(&mut Pure, b)
}

/// 0xb68b0: three effects of type 54.
pub fn types_54(b: &mut Cps2Bus) -> Option<Obj> {
    type_54_trio(&mut Pure, b)
}

/// 0xa8a42: camera 1 in mode 0x60.
pub fn camera_in_mode_60(b: &mut Cps2Bus) {
    crate::mem::wr8(b, A5 + 0x4ff, 0);
    wr16(b, A5 + 0x51e, 0x60);
}
use crate::ops::Size;
