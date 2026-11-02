//! EFFCT family (continued): types that no scenario exercises, rewritten so that the `0x39ce0` type table
//! no longer has an entry in original code (validation by `examples/fuzz_effct.rs`).
//!
//! - 43: rain of debris (`0xb4bcc`): at the top of the screen (setting 0, with a dust cloud at the start) or
//!   from the creator (`+0x5c`), speeds and animation drawn at random; it falls, blinks and
//!   disappears, or disappears with the creator;
//! - 46: armor pieces of an enemy (`0xb53f4`): they follow the carrier and replay its received hits,
//!   fall when its life drops below their threshold and break into shards (variant 1) that bounce;
//! - 48: flying machine of an enemy (`0xb59a6`, `0xb59c8`): cannon that turns around its carrier and
//!   fires (1, with its body, variant 0), sweeper placed at the screen edge that fires (2);
//! - 80: fireworks of a character's scene (`0x23606`): the hero (0) fires rockets during a
//!   fade to black, then flies away and ends the scene; spectators and sidekicks (1 to 6) wait for step
//!   0x12 of the scene to react, and leave with the fade;
//! - 63: chunk thrown by an enemy (ENEMY 22 and 23): leaves in an arc from its creator (`+0x42`),
//!   falls back, scatters debris, waits, blinks then disappears;
//! - 81: spark thrown by a character (`0xb96c0`): leaves the player's hand (offset per gesture
//!   `+0x42`), glides in an arc then blinks for the last frames.

use crate::families::cx::Bench;
use super::cx::{handlers, Cx};
use crate::objects::Obj;
use crate::objects::pools;
use crate::objects::Current;
use crate::machine::Machine;

/// Shared fields.
mod f {
    pub const STATE: u32 = 0x04;
    pub const SUB_STATE: u32 = 0x05;
    pub const VARIANT: u32 = 0x03;
    pub const FLAGS: u32 = 0x01;
    pub const FACING: u32 = 0x2c;
}

/// Routines d'origine.
mod rom {
    /// Son d'impact au sol.
    pub const SOUND_LANDING: u32 = 0xece9e;
}

fn free<R: Bench>(x: &mut Cx<'_, R>) {
    x.free(&pools::EFFCT);
}

/// Bit 1 of `FLAGS`: object hidden (blinking).
fn set_hidden<R: Bench>(x: &mut Cx<'_, R>, on: bool) {
    x.set_bit(f::FLAGS, 1, on);
}

// ---------------------------------------------------------------------------------------------
// Type 63: chunk thrown by an enemy
// ---------------------------------------------------------------------------------------------

mod chunk {
    /// Creator (ENEMY 22 or 23).
    pub const CREATOR: u32 = 0x42;
    pub const TIMER: u32 = 0x58;
    /// Animation sets of variants 0 and 1 (variant 2 reuses that of 1).
    pub const ANIMS_0: u32 = 0x16abaa;
    pub const ANIMS_1: u32 = 0x16b762;
    /// Kind of debris (`0x1172e`) at impact.
    pub const DEBRIS_KIND: u32 = 0xc;
}

/// Start (variants 0 and 1): placed on the creator, 0x37 higher, thrown forward.
struct Throw {
    vz: u16,
    az: u16,
    dx: u16,
    vx: u16,
    anims: u32,
}

const THROWS: [Throw; 2] = [
    Throw { vz: 0x4c0, az: 0xffd0, dx: 0xffa0, vx: 0x200, anims: chunk::ANIMS_0 },
    Throw { vz: 0x500, az: 0xffd8, dx: 0xffc8, vx: 0x280, anims: chunk::ANIMS_1 },
];

fn chunk<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        0 => chunk_init(x),
        2 => chunk_active(x),
        _ => free(x),
    }
    if x.u8(Obj::ACTIVE) != 0 {
        x.keep_in_zone();
        x.push_terrain();
        x.reinsert();
    }
}

fn chunk_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    let variant = x.u8(f::VARIANT);
    let Some(throw) = THROWS.get(variant as usize) else {
        // variant 2: no throw, only the animation
        x.anim_start(chunk::ANIMS_1, 0x14);
        return;
    };
    let creator = x.linked(chunk::CREATOR);
    let (px, py, pz) = (creator.u32(x.b, Obj::X), creator.u32(x.b, Obj::Y), creator.u32(x.b, Obj::Z));
    x.set32(Obj::X, px);
    x.set32(Obj::Y, py.wrapping_sub(1));
    x.set32(Obj::Z, pz);
    x.add16(Obj::Z, 0x37);
    let facing = creator.u8(x.b, f::FACING);
    x.set8(f::FACING, facing);
    x.set16(Obj::VZ, throw.vz);
    x.set16(Obj::AZ, throw.az);
    let (mut dx, mut vx) = (throw.dx, throw.vx);
    if facing != 0 {
        dx = dx.wrapping_neg();
        vx = vx.wrapping_neg();
    }
    x.add16(Obj::X, dx);
    x.set16(Obj::VX, vx);
    x.anim_start(throw.anims, 0x14);
}

fn chunk_active<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB_STATE) {
        0 => {
            x.move_xz();
            if x.u16(Obj::Z) as i16 > 0 {
                x.anim_step();
                return;
            }
            // on the ground: sound, debris, then the impact frame
            x.add8(f::SUB_STATE, 2);
            x.set16(Obj::Z, 0);
            x.set16(chunk::TIMER, 0x5a);
            crate::sound::play(x.b, rom::SOUND_LANDING, x.o);
            crate::ai_aids::debris_set(x.b, x.o, chunk::DEBRIS_KIND as u16);
            // Original glitch: the exhausted `dbf` counter stays in D5 (`dispatch::Leftovers`)
            *x.c.d5() = 0xffff;
            match x.u8(f::VARIANT) {
                0 => {
                x.anim_start(chunk::ANIMS_0, 0x15);
            }
                1 => {
                x.anim_start(chunk::ANIMS_1, 0x15);
            }
                _ => {}
            }
        }
        2 => {
            let t = x.u16(chunk::TIMER).wrapping_sub(1);
            x.set16(chunk::TIMER, t);
            if t as i16 <= 0 {
                x.add8(f::SUB_STATE, 2);
                x.set16(chunk::TIMER, 0x3c);
            }
            x.anim_step();
        }
        _ => {
            // blinks until the end of the delay
            let v = x.u8(f::FLAGS) ^ 2;
            x.set8(f::FLAGS, v);
            let t = x.u16(chunk::TIMER).wrapping_sub(1);
            x.set16(chunk::TIMER, t);
            if t as i16 > 0 {
                x.anim_step();
            } else {
                free(x);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 81: spark thrown by a character
// ---------------------------------------------------------------------------------------------

mod shard {
    pub const OWNER: u32 = 0x50;
    pub const TIMER: u32 = 0x58;
    /// Offsets (x, depth) of the hand, per player gesture (`+0x42`): 8-byte sheets.
    pub const HAND: u32 = 0x11ae86;
    pub const GESTURE: u32 = 0x42;
    pub const ANIMS: u32 = 0x19844e;
}

fn shard<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        0 => shard_init(x),
        2 => {
            x.move_xz();
            let t = x.u8(shard::TIMER).wrapping_sub(1);
            x.set8(shard::TIMER, t);
            if t == 0 {
                x.add8(f::STATE, 2);
            }
            set_hidden(x, t & 1 != 0);
            x.anim_step();
        }
        _ => free(x),
    }
}

fn shard_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    x.set8(shard::TIMER, 0x20);
    x.set16(Obj::VX, 0xff80);
    x.set16(Obj::VZ, 0x300);
    x.set16(Obj::AZ, 0xffc8);
    let owner = x.linked(shard::OWNER);
    let gesture = (owner.u16(x.b, shard::GESTURE) << 3) as i16 as i32 as u32;
    let mut dx = x.data16(shard::HAND.wrapping_add(gesture).wrapping_add(2));
    let dz = x.data16(shard::HAND.wrapping_add(gesture).wrapping_add(4));
    for off in [Obj::X, Obj::Z, Obj::Y] {
        let v = owner.u16(x.b, off);
        x.set16(off, v);
    }
    x.add16(Obj::Y, 0xffff);
    let facing = owner.u8(x.b, f::FACING) != 0;
    if facing {
        dx = dx.wrapping_neg();
        let vx = x.u16(Obj::VX).wrapping_neg();
        x.set16(Obj::VX, vx);
    }
    x.add16(Obj::X, dx);
    x.add16(Obj::Z, dz);
    let anim = x.u8(f::VARIANT) as u32 + if facing { 0 } else { 1 };
    x.anim_start(shard::ANIMS, anim as u16);
    x.reinsert();
}

// ---------------------------------------------------------------------------------------------
// Type 43: rain of debris
// ---------------------------------------------------------------------------------------------

mod rain {
    /// Followed object: the effect disappears when it is released or when it goes to state 4 or more.
    pub const LINK: u32 = 0x5c;
    pub const SETTING: u32 = 0x2f;
    pub const TIMER: u32 = 0x42;
    /// Reference depth, taken again every other frame.
    pub const SAVED_Y: u32 = 0x1c;
    pub const SPEED_INDEX: u32 = 0x2d;
    pub const ANIMS: u32 = 0x194c74;
    /// Tables (program space): animation speed per variant, then draws (X and Z speeds,
    /// animation) for the rain around the camera and for the creator's sparks.
    pub const SPEED_BY_VARIANT: u32 = 0xb4d7a;
    pub const CAMERA_VX: u32 = 0xb4e04;
    pub const CAMERA_VZ: u32 = 0xb4e24;
    pub const CAMERA_ANIMS: u32 = 0xb4de4;
    pub const SHARD_SPEED: u32 = 0xb4e94;
    pub const SHARD_VX: u32 = 0xb4ea4;
    pub const SHARD_VZ: u32 = 0xb4ec4;
    pub const SHARD_ANIMS: u32 = 0xb4ee4;
}

fn rng<R: Bench>(x: &mut Cx<'_, R>) -> u16 {
    x.random()
}

fn debris_rain<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(rain::LINK) == 0 {
        // plus d'objet suivi : retrait
        return x.set32(f::STATE, 0x0600_0000);
    }
    match x.u8(f::STATE) {
        0 => rain_init(x),
        2 => rain_active(x),
        _ => free(x),
    }
}

fn rain_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    if x.u8(rain::SETTING) == 0 {
        // rain: at the top right of the screen, a dust cloud at the start
        let px = x.u16(Obj::X).wrapping_add(0x1a0).wrapping_add(crate::ram::CAMERA_X.get(x.b));
        let py = x.u16(Obj::Y).wrapping_add(0xa0).wrapping_add(crate::ram::CAMERA_Y.get(x.b));
        x.set16(Obj::X, px);
        x.set16(Obj::Z, py);
        let y = x.u16(Obj::Y);
        x.set16(rain::SAVED_Y, y);
        x.set16(Obj::AX, 0xffe8);
        let o = x.o;
        crate::effects_aids::shock(x.b, o, 5, 0x201, 0, 0);
        x.set16(rain::TIMER, 0x26);
        let variant = x.u8(f::VARIANT) as u32;
        let speed = x.rom8(rain::SPEED_BY_VARIANT + variant);
        x.set8(rain::SPEED_INDEX, speed);
        rain_launch(x, rain::CAMERA_VX, rain::CAMERA_VZ, rain::CAMERA_ANIMS, 0x1f);
    } else {
        // spark leaving the creator
        x.add8(f::SUB_STATE, 4);
        let from = x.linked(rain::LINK);
        for off in [Obj::X, Obj::Z, Obj::Y] {
            let v = from.u16(x.b, off);
            x.set16(off, v);
        }
        x.set16(Obj::AZ, 0xffd8);
        x.set16(rain::TIMER, 0x14);
        let pick = (rng(x) & 0xf) as u32;
        let speed = x.rom8(rain::SHARD_SPEED + pick);
        x.set8(rain::SPEED_INDEX, speed);
        rain_launch(x, rain::SHARD_VX, rain::SHARD_VZ, rain::SHARD_ANIMS, 0xf);
    }
}

/// X and Z speeds and animation drawn from the tables (words, words, bytes; `anim_mask` bounds the last draw).
fn rain_launch<R: Bench>(x: &mut Cx<'_, R>, vx_table: u32, vz_table: u32, anims: u32, anim_mask: u16) {
    let pick = 2 * (rng(x) & 0xf) as u32;
    let vx = x.rom16(vx_table + pick);
    x.set16(Obj::VX, vx);
    let pick = 2 * (rng(x) & 0xf) as u32;
    let vz = x.rom16(vz_table + pick);
    x.set16(Obj::VZ, vz);
    let pick = (rng(x) & anim_mask) as u32;
    let anim = x.rom8(anims + pick);
    x.anim_start(rain::ANIMS, (anim as u32) as u16);
}

/// Countdown of `TIMER`; true when it drops below zero.
fn rain_countdown<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let t = x.u16(rain::TIMER).wrapping_sub(1);
    x.set16(rain::TIMER, t);
    (t as i16) < 0
}

/// End of life: removal once the delay has elapsed, blinking three frames out of four before.
fn rain_fade<R: Bench>(x: &mut Cx<'_, R>) {
    if rain_countdown(x) {
        return x.set8(f::STATE, 6);
    }
    let t = x.u16(rain::TIMER);
    set_hidden(x, t & 3 != 0);
}

fn rain_active<R: Bench>(x: &mut Cx<'_, R>) {
    let link = x.linked(rain::LINK);
    if link.u8(x.b, f::STATE) as i8 >= 4 {
        return free(x);
    }
    match x.u8(f::SUB_STATE) {
        0 => {
            x.move_xz();
            if rain_countdown(x) {
                x.add8(f::SUB_STATE, 2);
                x.set16(rain::TIMER, 0x1c);
            }
            // every other frame, the rain takes its reference depth again (the draw is lost)
            rng(x);
            if x.a5_u16(0x6e) & 1 == 0 {
                let y = x.u16(rain::SAVED_Y);
                x.set16(Obj::Y, y);
            }
            x.anim_step();
        }
        2 => rain_fade(x),
        _ => {
            x.move_xz();
            rain_fade(x);
        }
    }
    x.reinsert();
}

// ---------------------------------------------------------------------------------------------
// Type 46: armor pieces of an enemy (created three at a time by `0xb53f4`)
// ---------------------------------------------------------------------------------------------

mod armour {
    /// Carrier (ENEMY): the piece follows its position and replays its received hit (`+0x33`).
    pub const WEARER: u32 = 0x46;
    pub const PIECE: u32 = 0x2f;
    pub const LAST_HIT: u32 = 0x42;
    /// Carrier's life (`+0x62`) and starting life (`+0x60`): the piece falls when the life drops below
    /// a fraction of the starting life (quarter × (threshold + 1), threshold per piece, table `THRESHOLDS`).
    pub const LIFE: u32 = 0x62;
    pub const FULL_LIFE: u32 = 0x60;
    pub const HIT: u32 = 0x33;
    /// Thresholds (bytes, program space) and animation sets (longs) per piece.
    pub const THRESHOLDS: u32 = 0xb550e;
    pub const ANIM_SETS: u32 = 0xb54dc;
    pub const WORN_ANIMS: u32 = 0x195384;
    /// Shards of the fallen piece (variant 1): per piece, count minus one then kinds (bytes, data).
    pub const SHARDS: u32 = 0xb5720;
    /// Shard (variant 1): per kind, 16-byte sheet (dx, dy, dz, vx, vy, vz and az); bounces.
    pub const SHARD_LAUNCH: u32 = 0xb55dc;
    pub const SHARD_ANIMS: u32 = 0x17efe8;
    pub const BOUNCE_TIMER: u32 = 0x5c;
    pub const FALLING: u32 = 0x05;
    pub const ROLLING: u32 = 0x06;
}

/// Common ending: clamp on screen, shadow, re-listing.
fn on_screen<R: Bench>(x: &mut Cx<'_, R>) {
    x.keep_in_zone();
    x.push_terrain();
    x.reinsert();
}

fn armour<R: Bench>(x: &mut Cx<'_, R>) {
    if x.suspended() {
        return;
    }
    if x.u8(f::VARIANT) != 0 {
        return armour_shard(x);
    }
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            x.set8(armour::LAST_HIT, 0xff);
            x.anim_start(armour::WORN_ANIMS, 4);
        }
        2 => armour_worn(x),
        4 => armour_break(x),
        _ => free(x),
    }
}

/// Has the carrier's life dropped below the piece's threshold?
fn armour_falls<R: Bench>(x: &mut Cx<'_, R>, wearer: Obj) -> bool {
    let quarter = wearer.u16(x.b, armour::FULL_LIFE) >> 2;
    let piece = x.u8(armour::PIECE) as u32;
    let steps = x.rom8(armour::THRESHOLDS + piece) as u16 + 1;
    let threshold = quarter.wrapping_mul(steps);
    threshold as i16 > wearer.u16(x.b, armour::LIFE) as i16
}

fn armour_worn<R: Bench>(x: &mut Cx<'_, R>) {
    let link = x.u16(armour::WEARER);
    let wearer = Obj(link as i16 as i32 as u32 & 0xff_ffff);
    if (link as i16) >= 0 || wearer.u8(x.b, Obj::ACTIVE) == 0 || armour_falls(x, wearer) {
        return x.add8(f::STATE, 2);
    }
    for off in [Obj::X, Obj::Z, Obj::Y] {
        let v = wearer.u16(x.b, off);
        x.set16(off, v);
    }
    let facing = wearer.u8(x.b, f::FACING);
    x.set8(f::FACING, facing);
    let hit = wearer.u8(x.b, armour::HIT) & 0x7f;
    if hit == 0 {
        set_hidden(x, true);
        return on_screen(x);
    }
    if hit == x.u8(armour::LAST_HIT) {
        return on_screen(x);
    }
    // new hit: the piece plays the hit animation
    x.set8(armour::LAST_HIT, hit);
    set_hidden(x, false);
    let piece = x.u8(armour::PIECE) as u32;
    let set = crate::mem::prog32(x.b, armour::ANIM_SETS + 4 * piece);
    x.anim_start(set, (hit as u32) as u16);
}

/// The piece falls: it breaks into shards (variant 1) placed on it.
fn armour_break<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    let mut at = armour::SHARDS + 4 * x.u8(armour::PIECE) as u32;
    let count = x.data8(at);
    at += 1;
    let mut n = count as u16;
    loop {
        let Some(o) = x.alloc(&pools::EFFCT) else { return };
        o.set_u8(x.b, Obj::ACTIVE, 1);
        o.set_u8(x.b, Obj::KIND, 46);
        o.set_u8(x.b, Obj::VARIANT, 1);
        let kind = x.data8(at);
        at += 1;
        o.set_u8(x.b, armour::PIECE, kind);
        for off in [Obj::X, Obj::Y, Obj::Z] {
            let v = x.u16(off);
            o.set_u16(x.b, off, v);
        }
        let facing = x.u8(f::FACING);
        o.set_u8(x.b, f::FACING, facing);
        o.set_u16(x.b, armour::WEARER, x.o.0 as u16);
        n = n.wrapping_sub(1);
        if n == 0xffff {
            return;
        }
    }
}

/// Allocation of an effect: the slot, or `None` if the pool is full.
fn armour_shard<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        0 => armour_shard_init(x),
        2 => armour_shard_active(x),
        4 => x.add8(f::STATE, 2),
        _ => free(x),
    }
}

fn armour_shard_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    x.set8(f::SUB_STATE, 0);
    let sheet = armour::SHARD_LAUNCH + ((x.u8(armour::PIECE) as u16) << 4) as u32;
    let mut dx = x.data16(sheet);
    let dy = x.data16(sheet + 2);
    x.add16(Obj::Y, dy);
    let dz = x.data16(sheet + 4);
    x.add16(Obj::Z, dz);
    let mut vx = x.data16(sheet + 6);
    x.set16(Obj::AX, 0);
    let vy = x.data16(sheet + 8);
    x.set16(Obj::VY, vy);
    x.set16(Obj::AY, 0);
    let vz = x.data32(sheet + 10);
    x.set32(Obj::VZ, vz);
    if x.u8(f::FACING) == 0 {
        dx = dx.wrapping_neg();
        vx = vx.wrapping_neg();
    }
    x.add16(Obj::X, dx);
    x.set16(Obj::VX, vx);
    x.anim_start(armour::SHARD_ANIMS, 9);
}

/// New bounce: vertical speed and gravity, randomly drawn delay before stopping.
fn armour_shard_bounce<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(armour::ROLLING, 0);
    x.set16(Obj::VZ, 0x380);
    x.set16(Obj::AZ, 0xff80);
    let r = rng(x) & 0x1f;
    x.set8(armour::BOUNCE_TIMER, r as u8);
}

fn armour_shard_active<R: Bench>(x: &mut Cx<'_, R>) {
    if x.a5_u8(0x3d) != 0 {
        return on_screen(x);
    }
    if x.u8(armour::FALLING) == 0 {
        x.move_xyz();
        if (x.u16(Obj::Z) as i16) < 0 {
            // first contact with the ground
            x.set16(Obj::Z, 0);
            x.add8(armour::FALLING, 2);
            x.set8(armour::ROLLING, 0);
            armour_shard_bounce(x);
            return on_screen(x);
        }
    }
    if x.u8(armour::ROLLING) == 0 {
        x.anim_step();
        x.move_z();
        if (x.u16(Obj::Z) as i16) >= 0 {
            x.anim_step();
            return on_screen(x);
        }
        x.set16(Obj::Z, 0);
        x.add8(armour::ROLLING, 2);
    }
    let t = x.u8(armour::BOUNCE_TIMER).wrapping_sub(1);
    x.set8(armour::BOUNCE_TIMER, t);
    if (t as i8) < 0 {
        // next bounce (no clamp or re-listing on this frame)
        return armour_shard_bounce(x);
    }
    x.anim_step();
    on_screen(x);
}

// ---------------------------------------------------------------------------------------------
// Type 48: flying machine of an enemy and its cannons (created by `0xb59a6`, `0xb59c8`)
// ---------------------------------------------------------------------------------------------

mod flyer {
    /// Followed object (`+0x46`); variant 0 follows the object followed by its followed object.
    pub const LINK: u32 = 0x46;
    /// Phase of the following (`+0xa4` of the carrier): 1 for variant 1, 3 for variant 2.
    pub const PHASE: u32 = 0xa4;
    pub const ARMED: u32 = 0x42;
    pub const COUNTDOWN: u32 = 0x43;
    /// Variante 1 : trajet circulaire, table de (dx, dy) en `+0x4c`, angle en `+0x5c`, position relative
    /// saved in `+0x18` / `+0x1a`.
    pub const PATH: u32 = 0x4c;
    pub const PATH_TABLE: u32 = 0x1ca4b6;
    pub const ANGLE: u32 = 0x5c;
    pub const REL_X: u32 = 0x18;
    pub const REL_Y: u32 = 0x1a;
    /// Variant 2: a player on the same depth (`+0x2f` = 1).
    pub const PLAYER_IN_LINE: u32 = 0x2f;
    pub const ANIMS: u32 = 0x17f002;
    /// Enemy shot (ESHL 0x2c) released every fourth frame.
    pub const SHOT: u8 = 0x2c;
}

/// The followed object, if it exists and is active (`0xb5bf0`).
fn flyer_link<R: Bench>(x: &mut Cx<'_, R>) -> Option<Obj> {
    let p = x.u16(flyer::LINK);
    let o = Obj(p as i16 as i32 as u32 & 0xff_ffff);
    ((p as i16) < 0 && o.u8(x.b, Obj::ACTIVE) != 0).then_some(o)
}

fn flyer<R: Bench>(x: &mut Cx<'_, R>) {
    if x.suspended() {
        return;
    }
    let state = x.u8(f::STATE);
    match state {
        4 => return x.add8(f::STATE, 2),
        6 => return free(x),
        _ => {}
    }
    match (x.u8(f::VARIANT), state) {
        (0, 0) => {
            x.add8(f::STATE, 2);
            x.anim_start(flyer::ANIMS, 0xe);
        }
        (0, _) => flyer_body(x),
        (1, 0) => flyer_gun_init(x),
        (1, _) => flyer_gun(x),
        (_, 0) => {
            x.add8(f::STATE, 2);
            let Some(link) = flyer_link(x) else { return x.add8(f::STATE, 2) };
            let facing = link.u8(x.b, f::FACING);
            x.set8(f::FACING, facing);
            x.set8(flyer::ARMED, 0);
            x.anim_start(flyer::ANIMS, 8);
        }
        _ => flyer_sweeper(x),
    }
}

/// Variant 0: body placed on the object followed by the followed object, blinking every other frame.
fn flyer_body<R: Bench>(x: &mut Cx<'_, R>) {
    if x.a5_u8(0x3d) != 0 {
        return x.reinsert();
    }
    let Some(link) = flyer_link(x) else { return x.add8(f::STATE, 2) };
    let p = link.u16(x.b, flyer::LINK);
    let host = Obj(p as i16 as i32 as u32 & 0xff_ffff);
    if (p as i16) >= 0 || host.u8(x.b, Obj::ACTIVE) == 0 {
        return x.add8(f::STATE, 2);
    }
    for off in [Obj::X, Obj::Y, Obj::Z] {
        let v = host.u16(x.b, off);
        x.set16(off, v);
    }
    let facing = host.u8(x.b, f::FACING);
    x.set8(f::FACING, facing);
    x.add16(Obj::X, if facing != 0 { 0xfffa } else { 6 });
    x.add16(Obj::Y, 0xffff);
    x.add16(Obj::Z, 0x54);
    if x.a5_u16(0x6e) & 1 != 0 {
        x.set8(f::SUB_STATE, 0);
        set_hidden(x, true);
    } else {
        x.anim_step();
        set_hidden(x, false);
    }
    x.reinsert();
}

/// Enemy shot (ESHL 0x2c) of variant `variant`, every fourth frame (`0xb5d56`, `0xb5d82`).
fn flyer_shoot<R: Bench>(x: &mut Cx<'_, R>, variant: u8) {
    if x.a5_u16(0x6e) & 3 != 0 {
        return;
    }
    let Some(o) = x.alloc(&pools::ESHL) else { return };
    o.set_u8(x.b, Obj::ACTIVE, 1);
    o.set_u8(x.b, Obj::KIND, flyer::SHOT);
    o.set_u8(x.b, Obj::VARIANT, variant);
    o.set_u8(x.b, flyer::PLAYER_IN_LINE, 0);
    o.set_u16(x.b, flyer::LINK, x.o.0 as u16);
}

fn flyer_gun_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    let Some(link) = flyer_link(x) else { return x.add8(f::STATE, 2) };
    let facing = link.u8(x.b, f::FACING);
    x.set8(f::FACING, facing);
    // its body (variant 0)
    if let Some(o) = x.alloc(&pools::EFFCT) {
        o.set_u8(x.b, Obj::ACTIVE, 1);
        o.set_u8(x.b, Obj::KIND, 48);
        o.set_u8(x.b, Obj::VARIANT, 0);
        o.set_u8(x.b, flyer::PLAYER_IN_LINE, 0);
        o.set_u16(x.b, flyer::LINK, x.o.0 as u16);
    }
    x.set8(flyer::ARMED, 0);
    x.set32(flyer::PATH, flyer::PATH_TABLE);
    let rel = if x.u8(f::FACING) != 0 { 0xff40 } else { 0xc0 };
    x.set16(flyer::REL_X, rel);
    x.set16(flyer::ANGLE, 0x1000);
    x.set16(flyer::REL_Y, 0);
}

/// End of the phase: disarmed, the effect leaves; armed, it waits `COUNTDOWN` frames.
fn flyer_wind_down<R: Bench>(x: &mut Cx<'_, R>, keep_while_positive: bool) -> bool {
    if x.u8(flyer::ARMED) as i8 <= 0 {
        x.add8(f::STATE, 2);
        return false;
    }
    let t = x.u8(flyer::COUNTDOWN).wrapping_sub(1);
    x.set8(flyer::COUNTDOWN, t);
    if t as i8 > 0 {
        return keep_while_positive;
    }
    x.add8(f::STATE, 2);
    false
}

/// Variant 1: cannon that turns around the followed object as long as it is in phase 1, and fires.
fn flyer_gun<R: Bench>(x: &mut Cx<'_, R>) {
    if x.a5_u8(0x3d) != 0 {
        return;
    }
    let Some(link) = flyer_link(x) else { return x.add8(f::STATE, 2) };
    if x.u8(f::SUB_STATE) != 0 {
        flyer_wind_down(x, true);
        return;
    }
    if link.u8(x.b, flyer::PHASE) != 1 {
        x.set8(flyer::COUNTDOWN, 0x20);
        return x.add8(f::SUB_STATE, 2);
    }
    // 0xb5c48: one step on the circle (table of 32 positions, angle in 1/256 in the high byte)
    let angle = ((x.u8(flyer::ANGLE) as u16 & 0x1f) << 2) as u32;
    let path = x.u32(flyer::PATH);
    for (pos, rel, step) in [(Obj::X, flyer::REL_X, 0u32), (Obj::Y, flyer::REL_Y, 2)] {
        let r = x.u16(rel);
        x.set16(pos, r);
        let d = x.data16(path.wrapping_add(angle + step)) as i16 as i32 as u32;
        x.add32(pos, d << 8);
        let v = x.u16(pos);
        x.set16(rel, v);
        let base = link.u16(x.b, pos);
        x.add16(pos, base);
    }
    let turn = if x.u8(f::FACING) != 0 { 0xff80 } else { 0x80 };
    x.add16(flyer::ANGLE, turn);
    flyer_shoot(x, 0);
    x.keep_on_screen(0x10);
}

/// Variant 2: sweeper placed at the screen edge, at the depth of the followed object (phase 3), which fires.
fn flyer_sweeper<R: Bench>(x: &mut Cx<'_, R>) {
    if x.a5_u8(0x3d) != 0 {
        return x.add8(f::STATE, 2);
    }
    let Some(link) = flyer_link(x) else { return x.add8(f::STATE, 2) };
    if x.u8(f::SUB_STATE) != 0 {
        if flyer_wind_down(x, true) {
            flyer_sweep(x, link);
        }
        return;
    }
    if link.u8(x.b, flyer::PHASE) != 3 {
        x.set8(flyer::COUNTDOWN, 0x20);
        return x.add8(f::SUB_STATE, 2);
    }
    flyer_sweep(x, link);
    flyer_shoot(x, 1);
}

/// `0xb5cce`: at the screen edge (side depending on the orientation), at the depth of the followed
/// object; notes whether
/// a player present is on the same line (within 8 pixels).
fn flyer_sweep<R: Bench>(x: &mut Cx<'_, R>, link: Obj) {
    x.set8(flyer::PLAYER_IN_LINE, 0);
    let y = link.u16(x.b, Obj::Y);
    x.set16(Obj::Y, y);
    let camera = if crate::ram::CAMERA_ALT_ACTIVE.get(x.b) != 0 { 0x556 } else { 0x506 };
    let mut px = x.a5_u16(camera);
    if x.u8(f::FACING) != 0 {
        px = px.wrapping_add(0x1c0);
    }
    x.set16(Obj::X, px);
    x.set16(Obj::Z, 0x20);
    for k in 0..4 {
        let p = Obj(crate::mem::A5 + 0x5de + 0x200 * k);
        if p.u8(x.b, Obj::ACTIVE) == 0 {
            continue;
        }
        let d = p.u16(x.b, Obj::Y).wrapping_sub(y).wrapping_add(8);
        if d < 0x10 {
            x.set8(flyer::PLAYER_IN_LINE, 1);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 80: fireworks of a character's scene (created by `0x23606`, variants 2 and 3)
// ---------------------------------------------------------------------------------------------

mod fireworks {
    /// Scene step that the actors wait for before leaving (`A5+0x2e`), and return to the game (0xc).
    pub const SCENE_STEP: u32 = 0x2e;
    pub const GO: u8 = 0x12;
    pub const BACK: u8 = 0xc;
    /// Fade in progress (`A5+0x48f`), its settings (`A5+0x48e`, `A5+0x490`) and its palettes (`A5+0x492...`).
    pub const FADING: u32 = 0x48f;
    pub const FADE_SPEED: u32 = 0x48e;
    pub const FADE_MODE: u32 = 0x490;
    pub const FADE_SOURCES: u32 = 0x492;
    /// Fade tasks (programs of `0x9b6`): toward black, then back.
    pub const FADE_OUT: u32 = 0x1ccf8;
    pub const FADE_IN: u32 = 0x1ced4;
    /// Fireworks camera (`A5+0x5a6`, `A5+0x5aa`).
    pub const CAMERA_X: u32 = 0x5a6;
    pub const CAMERA_Y: u32 = 0x5aa;
    pub const TIMER: u32 = 0x58;
    pub const DELAY: u32 = 0x5a;
    pub const HOST: u32 = 0x56;
    pub const SCENERY: u32 = 0x42;
    /// Rockets (type 50): variant, delay and offset (x, y) of each shot (program space).
    pub const ROCKET_VARIANTS: u32 = 0xb9628;
    pub const ROCKET_DELAYS: u32 = 0xb9638;
    pub const ROCKET_PLACES: u32 = 0xb9658;
    pub const ANIMS_HERO: u32 = 0x1836ec;
    pub const ANIMS_FIRE: u32 = 0x1824d4;
    pub const ANIMS_LAST: u32 = 0x18373a;
}

/// Scene sounds.
mod sfx {
    pub const LAST_ROCKET: u32 = 0xe49ae;
    pub const LIFT_OFF: u32 = 0xe580a;
    pub const JUMP: u32 = 0xe3da6;
    pub const CHEER: u32 = 0xe4e5a;
    pub const LANDING: u32 = 0xec7ae;
}

fn fireworks<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        0 => fireworks_init(x),
        2 => fireworks_active(x),
        _ => free(x),
    }
    // the game task waits (`hero_rockets`): the end of the handler is step `AfterState`
    if crate::tasks::has_ceded() {
        return;
    }
    fireworks_tail(x);
}

/// `sub.w #1,(off,A0)`: returns the exact value (no wrap to 16 bits) that the following `bgt`/`ble` test.
fn countdown<R: Bench>(x: &mut Cx<'_, R>, off: u32) -> i32 {
    let v = x.u16(off);
    x.set16(off, v.wrapping_sub(1));
    v as i16 as i32 - 1
}

/// Common ending (`0xb8f04`): the object, if still active, is re-listed.
fn fireworks_tail<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Obj::ACTIVE) != 0 {
        x.reinsert();
    }
}

fn scene_go<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    x.a5_u8(fireworks::SCENE_STEP) == fireworks::GO
}

fn fading<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    x.a5_u8(fireworks::FADING) != 0
}

/// Sets X and Y (and Z) in screen coordinates.
fn place<R: Bench>(x: &mut Cx<'_, R>, px: u16, z: Option<u16>, py: u16) {
    x.set16(Obj::X, px);
    if let Some(z) = z {
        x.set16(Obj::Z, z);
    }
    x.set16(Obj::Y, py);
}

/// An actor attached to object `+0x56`: X - `dx`, Z + 0x60, right in front of it.
fn follow_host<R: Bench>(x: &mut Cx<'_, R>, dx: u16) {
    let host = x.linked(fireworks::HOST);
    let (hx, hz, hy) = (host.u16(x.b, Obj::X), host.u16(x.b, Obj::Z), host.u32(x.b, Obj::Y));
    x.set16(Obj::X, hx);
    x.set16(Obj::Z, hz);
    x.set32(Obj::Y, hy);
    x.add16(Obj::X, dx.wrapping_neg());
    x.add16(Obj::Z, 0x60);
    x.add16(Obj::Y + 2, 0xffff);
}

fn fireworks_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    match x.u8(f::VARIANT) {
        0 => {
            x.set8(f::FACING, 0x20);
            place(x, 0x50, None, 0x160);
            x.anim_start(fireworks::ANIMS_HERO, 0);
        }
        1 => {
            x.set8(0x2a, 1);
            x.set8(0x2b, 0x18);
            place(x, 0x110, None, 0x161);
            x.anim_start(0x17b14e, 0);
        }
        2 => {
            place(x, 0x178, None, 0x154);
            x.anim_start(0x177874, 0);
        }
        3 => {
            place(x, 0x158, Some(0x18), 0x168);
            x.set16(Obj::Y + 2, 0xffff);
            // its two sidekicks (variants 4 and 5)
            for (variant, dx) in [(4u8, 0x20u16), (5, 0x10)] {
                let Some(o) = x.alloc(&pools::EFFCT) else { break };
                o.set_u8(x.b, Obj::ACTIVE, 1);
                o.set_u8(x.b, Obj::KIND, 80);
                o.set_u8(x.b, Obj::VARIANT, variant);
                let (px, pz, py) = (x.u16(Obj::X), x.u16(Obj::Z), x.u32(Obj::Y));
                o.set_u16(x.b, Obj::X, px.wrapping_sub(dx));
                o.set_u16(x.b, Obj::Z, pz.wrapping_add(0x60));
                o.set_u32(x.b, Obj::Y, py);
                let frac = o.u16(x.b, Obj::Y + 2).wrapping_sub(1);
                o.set_u16(x.b, Obj::Y + 2, frac);
                o.set_u16(x.b, fireworks::HOST, x.o.0 as u16);
            }
            x.anim_start(fireworks::ANIMS_FIRE, 0);
        }
        4 => {
                x.anim_start(fireworks::ANIMS_FIRE, 2);
            }
        5 => {
                x.anim_start(fireworks::ANIMS_FIRE, 3);
            }
        _ => {
            load_colours(x, 0x253460, 0x91c2e0, 0x20);
            x.set8(0x2a, 1);
            x.set8(0x2b, 0x17);
            place(x, 0x120, Some(0xe0), 0x160);
            x.anim_start(fireworks::ANIMS_LAST, 3);
        }
    }
}

/// Copies `n` colours from the ROM (data space) into the palette, opaque (`| 0xf000`).
fn load_colours<R: Bench>(x: &mut Cx<'_, R>, from: u32, to: u32, n: u32) {
    for k in 0..n {
        let v = x.data16(from + 2 * k) | 0xf000;
        crate::mem::wr16(x.b, to + 2 * k, v);
    }
}

/// `0xb95b8`: copies 0x400 bytes from `from` to `to` in blocks of eight longs (`movem` through D1-D6, A3, A4).
/// Original glitch: the last block's longs 1 to 4 stay in D2-D5 (`dispatch::Leftovers`).
fn copy_block<R: Bench>(x: &mut Cx<'_, R>, from: u32, to: u32) {
    let mut last = [0u32; 8];
    for k in 0..0x100 {
        let v = crate::mem::rd32(x.b, from + 4 * k);
        crate::mem::wr32(x.b, to + 4 * k, v);
        last[(k % 8) as usize] = v;
    }
    (*x.c.d2(), *x.c.d3(), *x.c.d4(), *x.c.d5()) = (last[1], last[2], last[3], last[4]);
}

/// Launches fade task `program` (`0x9b6`, D0.w = 0x10), A0 kept.
fn fade_task<R: Bench>(x: &mut Cx<'_, R>, program: u32) {
    crate::kernel::create_task(x.b, 0x10, program);
}

fn fireworks_active<R: Bench>(x: &mut Cx<'_, R>) {
    let sub = x.u8(f::SUB_STATE);
    match x.u8(f::VARIANT) {
        0 => hero(x, sub),
        1 => match sub {
            0 if scene_go(x) => {
                x.add8(f::SUB_STATE, 2);
                x.anim_start(0x17b162, 1);
            }
            2 if !fading(x) => {
                x.add8(f::SUB_STATE, 2);
                x.anim_start(0x17b166, 7);
            }
            0 | 2 => {
                x.anim_step();
            }
            _ => {}
        },
        2 => match sub {
            0 if scene_go(x) => {
                x.add8(f::SUB_STATE, 2);
                x.set16(Obj::VX, 0xfe20);
                x.set16(Obj::AX, 0);
                x.set16(Obj::VZ, 0x340);
                x.set16(Obj::AZ, 0xfff0);
                crate::sound::play(x.b, sfx::JUMP, x.o);
                x.anim_start(0x17788c, 7);
            }
            0 => {
                x.anim_step();
            }
            _ => {
                x.anim_step();
                if x.u8(0x33) == 0 && !fading(x) {
                    return free(x);
                }
                x.move_xz();
            }
        },
        3 => match sub {
            0 => {
                if scene_go(x) {
                    x.add8(f::SUB_STATE, 2);
                    crate::sound::play(x.b, sfx::CHEER, x.o);
                }
                x.anim_step();
            }
            _ => {
                x.add16(Obj::X, 0xfffe);
                x.anim_step();
                if !fading(x) {
                    free(x);
                }
            }
        },
        4 => {
            if sub != 0 && !fading(x) {
                return free(x);
            }
            follow_host(x, 0x20);
            if sub != 0 {
                x.anim_step();
            } else if scene_go(x) {
                x.add8(f::SUB_STATE, 2);
                x.anim_step();
            }
        }
        5 => {
            follow_host(x, 0x10);
            if sub == 0 {
                if scene_go(x) {
                    x.add8(f::SUB_STATE, 2);
                }
            } else if !fading(x) {
                free(x);
            }
        }
        _ => last_actor(x, sub),
    }
}

/// Variant 6: falls onto the scene, plays its animation, waits for the return to the game then blinks and leaves.
fn last_actor<R: Bench>(x: &mut Cx<'_, R>, sub: u8) {
    match sub {
        0 => {
            let z = x.u16(Obj::Z) as i16 as i32 - 6;
            x.add16(Obj::Z, 0xfffa);
            if z <= 0 {
                x.add8(f::SUB_STATE, 2);
                crate::sound::play(x.b, sfx::LANDING, x.o);
                x.set16(Obj::Z, 0);
                x.anim_start(fireworks::ANIMS_LAST, 5);
                return;
            }
            if x.u8(0x33) == 0 {
                x.anim_step();
            }
        }
        2 => {
            if x.anim_step() {
                x.add8(f::SUB_STATE, 2);
                x.set16(fireworks::TIMER, 0x40);
            }
        }
        4 => {
            if x.a5_u8(fireworks::SCENE_STEP) == fireworks::BACK {
                x.add8(f::SUB_STATE, 2);
                x.set16(fireworks::TIMER, 0x40);
            }
        }
        _ => {
            if countdown(x, fireworks::TIMER) <= 0 {
                return free(x);
            }
            let v = x.u8(f::FLAGS) ^ 2;
            x.set8(f::FLAGS, v);
        }
    }
}

/// Variant 0: the character who fires the fireworks (nine steps).
fn hero<R: Bench>(x: &mut Cx<'_, R>, sub: u8) {
    match sub {
        0 => {
            if !scene_go(x) {
                x.anim_step();
                return;
            }
            x.add8(f::SUB_STATE, 2);
            x.set16(Obj::VX, 0x140);
            x.set16(Obj::AX, 0);
            x.set16(Obj::VZ, 0x340);
            x.set16(Obj::AZ, 0xfff0);
            x.anim_start(fireworks::ANIMS_HERO, 4);
            // the scene goes black: palettes saved, fade
            copy_block(x, 0x91c000, 0x910000);
            copy_block(x, 0x91c800, 0x910400);
            copy_block(x, 0x91cc00, 0x910800);
            x.set_a5_u16(fireworks::FADE_MODE, 3);
            x.set_a5_u8(fireworks::FADE_SPEED, 0xf);
            x.set_a5_u8(fireworks::FADING, 1);
            fade_task(x, fireworks::FADE_OUT);
        }
        2 => {
            x.move_xz();
            x.anim_step();
            if fading(x) {
                return;
            }
            x.add8(f::SUB_STATE, 2);
            x.set16(Obj::X, 0xc0);
            x.set16(Obj::Z, 0);
            load_colours(x, 0x253260, 0x91c260, 0x10);
            x.set16(fireworks::TIMER, 0x10);
            x.set16(fireworks::DELAY, 0);
            x.anim_start(fireworks::ANIMS_HERO, 0);
        }
        4 => hero_rockets(x),
        6 => {
            if !fading(x) {
                x.add8(f::SUB_STATE, 2);
                x.set16(fireworks::TIMER, 0x14);
                crate::families::small::clear_scroll1_start(x);
            }
            x.anim_step();
        }
        8 => {
            if countdown(x, fireworks::DELAY) <= 0 {
                // scenery 0x55, even if it means waiting for a slot
                // Original glitch: a full SET pool makes it restart endlessly (the game freezes).
                let scenery = loop {
                    if let Some(o) = super::creators::scene_set(x.b, 0x55) {
                        break o;
                    }
                };
                x.set16(fireworks::SCENERY, scenery.0 as u16);
                x.add8(f::SUB_STATE, 2);
            }
            x.anim_step();
        }
        10 => {
            let scenery = x.linked(fireworks::SCENERY);
            if scenery.u8(x.b, 0x22) != 0 {
                x.add8(f::SUB_STATE, 2);
                x.set16(fireworks::TIMER, 0xa);
            }
            x.anim_step();
        }
        12 => {
            if countdown(x, fireworks::TIMER) > 0 {
                x.anim_step();
                return;
            }
            x.add8(f::SUB_STATE, 2);
            x.set16(Obj::VZ, 0x700);
            x.set16(Obj::AZ, 0);
            x.set32(Obj::VX, 0);
            crate::sound::play(x.b, sfx::LIFT_OFF, x.o);
            x.anim_start(fireworks::ANIMS_HERO, 6);
        }
        14 => hero_lift_off(x),
        _ => {}
    }
}

/// Step 4: one rocket each time the delay elapses, the last with a sound; then four frames of waiting
/// (the task cedes control) and the fade back.
fn hero_rockets<R: Bench>(x: &mut Cx<'_, R>) {
    if countdown(x, fireworks::DELAY) > 0 {
        return;
    }
    let t = countdown(x, fireworks::TIMER);
    if t >= 0 {
        if t == 0 {
            crate::sound::play(x.b, sfx::LAST_ROCKET, x.o);
        }
        return rocket(x);
    }
    x.add8(f::SUB_STATE, 2);
    // The whole game waits four frames (`jsr 0xa40` in the middle of the dispatch loop): the task will resume at step
    // `AfterWait`, then, on return, at `AfterState` (the end of the handler), like the original. The trap runs on a
    // work bench (D0 = 4; it only leaves D0 and D1, which nobody reads).
    crate::tasks::table::call(x.b, Step::AfterState);
    let mut bench = m68k::Cpu::default();
    bench.d[0] = 4;
    crate::kernel::wait(&mut bench, x.b, Step::AfterWait);
    // the dispatch loop and the pending Rust callers give up (`tasks::has_ceded`): the task will resume through the
    // steps, then through the loop's pending call (`dispatch::Step::Dispatch`)
}

/// After the wait: fade back from the saved palettes.
fn fireworks_fade_back<R: Bench>(x: &mut Cx<'_, R>) {
    x.set_a5_u16(fireworks::FADE_MODE, 1);
    x.set_a5_u8(fireworks::FADE_SPEED, 0xf);
    x.set_a5_u8(fireworks::FADING, 1);
    for (k, v) in [0x910000u32, 0x25c0c0, 0x910400, 0x910800].into_iter().enumerate() {
        x.set_a5_u32(fireworks::FADE_SOURCES + 4 * k as u32, v);
    }
    fade_task(x, fireworks::FADE_IN);
}

/// `0xb95e8`: a rocket (type 50): variant, delay until the next one and place read for shot `TIMER`.
fn rocket<R: Bench>(x: &mut Cx<'_, R>) {
    let Some(o) = x.alloc(&pools::EFFCT) else { return };
    o.set_u8(x.b, Obj::ACTIVE, 1);
    o.set_u8(x.b, Obj::KIND, 0x32);
    let n = x.u16(fireworks::TIMER) as i16 as i32 as u32;
    let variant = x.rom8(fireworks::ROCKET_VARIANTS.wrapping_add(n));
    o.set_u8(x.b, Obj::VARIANT, variant);
    let delay = x.rom16(fireworks::ROCKET_DELAYS.wrapping_add(2 * n));
    x.set16(fireworks::DELAY, delay);
    let px = x.a5_u16(fireworks::CAMERA_X).wrapping_add(x.rom16(fireworks::ROCKET_PLACES.wrapping_add(4 * n)));
    let pz = x.a5_u16(fireworks::CAMERA_Y).wrapping_add(x.rom16(fireworks::ROCKET_PLACES.wrapping_add(4 * n + 2)));
    o.set_u16(x.b, Obj::X, px);
    o.set_u16(x.b, Obj::Z, pz);
}

/// Step 14: flies away leaving a trail (type 50, variant 7) every five frames, up to the top
/// of the screen: the scene ends (`A5+0x2c` = 0x10000000).
fn hero_lift_off<R: Bench>(x: &mut Cx<'_, R>) {
    if countdown(x, fireworks::TIMER) <= 0 {
        x.set16(fireworks::TIMER, 5);
        if let Some(o) = x.alloc(&pools::EFFCT) {
            o.set_u8(x.b, Obj::ACTIVE, 1);
            o.set_u8(x.b, Obj::KIND, 0x32);
            o.set_u8(x.b, Obj::VARIANT, 7);
            o.set_u16(x.b, fireworks::SCENERY, x.o.0 as u16);
            let facing = x.u8(f::FACING);
            o.set_u8(x.b, f::FACING, facing);
            for off in [Obj::X, Obj::Z, Obj::Y] {
                let v = x.u16(off);
                o.set_u16(x.b, off, v);
            }
            let y = o.u16(x.b, Obj::Y).wrapping_add(1);
            o.set_u16(x.b, Obj::Y, y);
            let frame = x.u32(0x34);
            o.set_u32(x.b, 0x34, frame);
            for off in [0x3cu32, 0x3f] {
                let v = x.u8(off);
                o.set_u8(x.b, off, v);
            }
            o.set_u16(x.b, 0x30, 0xe);
        }
    }
    x.move_xz();
    let top = x.a5_u16(fireworks::CAMERA_Y).wrapping_add(0x100);
    let height = x.u16(Obj::Y).wrapping_add(x.u16(Obj::Z));
    if height as i16 >= top as i16 {
        x.add8(f::SUB_STATE, 2);
        x.set_a5_u32(0x2c, 0x1000_0000);
        set_hidden(x, true);
    }
}

crate::steps! {
    /// Resumptions of the game task when the fireworks (type 80) made it wait (`hero_rockets`); the
    /// context returned by the core has the object in A0.
    pub enum Step {
        /// After the four-frame wait: fade back.
        AfterWait = 0xb91d0 => |x| {
            fireworks_fade_back(&mut Cx::new(x.c, x.b));
            crate::modes::Flow::Done
        },
        /// Return from the jump through the state table (`jsr (table,PC)`): end of the handler.
        AfterState = 0xb8f04 => |x| {
            fireworks_tail(&mut Cx::new(x.c, x.b));
            crate::modes::Flow::Done
        },
    }
}

handlers! {
    0xb8ef6 "effct_firework" #[without] fireworks,
    0xb59ea "effct_machine_volante" #[without] flyer,
    0xb5454 "effct_armure" #[without] armour,
    0xb4bec "effct_rain_debris" #[without] debris_rain,
    0xb7866 "effct_enemy_piece" #[without] chunk,
    0xb96dc "effct_shard_thrown" #[without] shard,
}
