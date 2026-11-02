//! Enemy shots and attacks, ESHL types 15 to 39 (continuation of `eshl.rs`). See `docs/families/eshl_b.md`.
//!
//! Same conventions as `eshl.rs`: each type is a `mod` with a `run` function; `+4` is the main
//! state, `+5` and `+6` substates (even bytes, indices into the original's jump tables);
//! `+0x50` designates the owner (16-bit word, or long for a few types).

use crate::families::cx::Bench;
use super::cx::{handlers, Cx};
use super::shell::*;
use crate::mem::{rd32, wr32};
use crate::objects::Obj;
use ddsom_data::effects::Table;
use super::shell::tables::Read;
use crate::objects::pools;
use crate::objects::Current;
use crate::machine::Machine;

/// Long "to release" state (`+4 = 6`, `+5..+7` = 0).
const DEAD: u32 = 0x0600_0000;

// ---------------------------------------------------------------------------------------------
// Aides communes
// ---------------------------------------------------------------------------------------------

/// Owner (16-bit sign-extended word) of another object.
fn link_of<R: Bench>(x: &mut Cx<'_, R>, o: Obj, off: u32) -> Obj {
    Obj(o.u16(x.b, off) as i16 as i32 as u32 & 0xff_ffff)
}

/// Object designated by a long field (`movea.l (off,A0),A1`).
fn ptr<R: Bench>(x: &mut Cx<'_, R>, off: u32) -> Obj {
    Obj(x.u32(off) & 0xff_ffff)
}

/// Reserves an object in the pool whose allocation routine is `alloc`; `None` if the pool is full.
/// The object is marked to be released on the next frame.
fn kill<R: Bench>(x: &mut Cx<'_, R>) {
    x.set32(0x4, DEAD);
}

/// Value of the low word of an address register, to store in a field (`move.w A0w,(off,A1)`).
fn low16(o: Obj) -> u16 {
    o.0 as u16
}

fn reinsert_foreground<R: Bench>(x: &mut Cx<'_, R>) {
    x.reinsert_foreground();
}

/// Copies an object's position (X, Z, Y in 16.16) into the shot.
fn copy_position32<R: Bench>(x: &mut Cx<'_, R>, from: Obj) {
    for off in [0x8, 0xc, 0x10] {
        let v = from.u32(x.b, off);
        x.set32(off, v);
    }
}

/// Calls `1b64` (speed table `n`, indexed by `+0x2d`).
fn speed_table<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.select_speed(&crate::objects::motion::SPEEDS_B, n as u16);
}

// ---------------------------------------------------------------------------------------------
// Type 15 (0x9fe0a): weapon of a composite enemy, sub-object of a sub-object (h)
// ---------------------------------------------------------------------------------------------

/// Piece thrown by a part of a composite enemy (ESHL type 14 then its shot): the owner
/// (`+0x50`) is a type 14 shot whose owner is itself the enemy. Variant 0: projectile
/// that leaves in an arc toward the player (speeds chosen according to the distance between the enemy and its
/// target); variant 1: weapon stuck to the owner, which follows the attack animation.
mod shot15;

// ---------------------------------------------------------------------------------------------
// Type 16 (0xa03e0): picked-up / carried object (h), placed in the foreground list
// ---------------------------------------------------------------------------------------------

/// Object attached to its owner (an enemy: HP in `+0x62`). As long as it is "carried"
/// (`+0x5a` zero) it follows the owner; otherwise it is thrown in an arc (speed table 0x18),
/// spins (`+0x2d` varies by 1 every 25 frames) and goes to state 4 when falling below Z = 0x100.
mod shot16;

// ---------------------------------------------------------------------------------------------
// Type 17 (0xa05ce): projectile with aimed trajectory, three modes according to +0x43 (h)
// ---------------------------------------------------------------------------------------------

/// Projectile that appears while charging (animation 0, 60 frames) then heads toward its target (`+0x50`,
/// long pointer) with a computed vertical speed/angle; `+0x43` picks the mode: 0 aim
/// the angle by table (`1386`), < 0 fire flat toward the target, > 0 aim in depth. The parent
/// (`+0x56`, long pointer) maintains it: if it goes to state >= 4 the projectile dies. At
/// startup it brightens palette 0x91c300 (copy of 0x25a660 tinted by 0x91c000).
mod shot17;

// ---------------------------------------------------------------------------------------------
// Type 18 (0xa0a06): oblique projectile with speed taken from a table (h)
// ---------------------------------------------------------------------------------------------

/// Projectile whose direction depends on the orientation, speed table 1b64 chosen by a global variable
/// (A5+0x7f92); destroyed on contact, on certain terrains, or off screen.
mod shot18;

// ---------------------------------------------------------------------------------------------
// Type 19 (0xa0b20): projectile thrown toward the player, sprite depending on the angle (h)
// ---------------------------------------------------------------------------------------------

/// Projectile that chooses its direction (8 sprites) from the angle toward the target (`+0x50`, long
/// pointer), is launched with speed table 0x40 (VZ = VY), falls until Z < 0 then stays 48
/// images en animation d'impact.
mod shot19;

// ---------------------------------------------------------------------------------------------
// Helper shared by types 20 and 21: sticking to the owner through an offset table
// ---------------------------------------------------------------------------------------------

/// Sticks to the owner with the offset of row `variant` of a table (8 bytes per row:
/// X, integer Y, signed fractional Y, Z). With no owner, the object is released on the next frame.
fn attach<R: Bench>(x: &mut Cx<'_, R>, table: &Table) {
    if x.u16(f::OWNER) == 0 {
        return kill(x);
    }
    let owner = x.linked(f::OWNER);
    copy_position32(x, owner);
    let flip = owner.u8(x.b, f::FLIP);
    x.set8(f::FLIP, flip);
    let row = (x.u8(0x3) << 3) as u32;
    let mut dx = table.at16(x.b, row);
    if flip != 0 {
        dx = dx.wrapping_neg();
    }
    x.add16(0x8, dx);
    let dy = table.at16(x.b, row + 2);
    x.add16(0x10, dy);
    let dy_fraction = table.at16(x.b, row + 4) as i16 as i32 as u32;
    x.add32(0x10, dy_fraction);
    let dz = table.at16(x.b, row + 6);
    x.add16(0xc, dz);
}

// ---------------------------------------------------------------------------------------------
// Type 20 (0xa0c5a): hit of an enemy part (h), two variants
// ---------------------------------------------------------------------------------------------

/// Weapon stuck to its owner (`attach`). Variant 0: randomly drawn idle animation then,
/// when the owner arms its attack (`+0xa5` bit 0), state 2 substate 2: hit animation
/// that creates a type 21 shot; variant 1: delayed weapon (`+0x5c` frames), which strikes
/// too. The states frozen by the pause (`A5+0x3d`, `A5+0x4e`) or the suspended object freeze it.
mod shot20;

// ---------------------------------------------------------------------------------------------
// Type 21 (0xa0f96): wave / breath attached to a weapon (h)
// ---------------------------------------------------------------------------------------------

/// Shot created by type 20: stuck to the owner (`attach`), animation of 0x4f repetitions
/// (`+0x42`) during which it creates an effect (EFFCT 0x18, saved in `+0x46`), then
/// end animation. It disappears if the game is frozen (`A5+0x4e`) or if the holder of its
/// owner is in state `+0xb2`.
mod shot21;

// ---------------------------------------------------------------------------------------------
// Type 22 (0xa1180): spark thrown by an enemy (h)
// ---------------------------------------------------------------------------------------------

/// Spark that leaves the creator's position with a speed drawn from a per-variant table
/// (speeds in X and Z), falls until Z <= 0 then blinks for 30 frames.
mod shot22;

// ---------------------------------------------------------------------------------------------
// Type 23 (0xa1292): scenery object that slides in front of the camera (h)
// ---------------------------------------------------------------------------------------------

/// Object placed at the camera's border (`A5+0x556`), which moves according to speed table 0xd
/// for 48 frames, waits for the signal `A5+0x42` then leaves again with table 2.
mod shot23;

// ---------------------------------------------------------------------------------------------
// Type 24 (0xa137e): wave of enemies / shots launched by a creator (h)
// ---------------------------------------------------------------------------------------------

/// Variant 0: generator. It places itself on its creator (`+0x5c`), then for `+0x58` frames it
/// launches a type 24 shot (variant 1) every 2 frames and, at `+0x58 == 0x20`, spawns
/// a group of enemies (ENEMY type 1, number and positions according to `A5+0x7f92` and `A5+0x35`).
/// Variant 1: effect that it launches, at one of the 16 positions of table 0xa1438 (`+0x56`).
mod shot24;

// ---------------------------------------------------------------------------------------------
// Type 25 (0xa16bc): projectile that falls from the sky onto a target (h)
// ---------------------------------------------------------------------------------------------

/// After a delay (`+0x58` frames), falls from the camera height + 0x100 above the
/// creator's position (`+0x5c`), with a random offset for non-zero variants.
/// On impact (Z < 0): sounds and effects, then it sets its shadow (`+0x5a`) to state 6.
mod shot25;

// ---------------------------------------------------------------------------------------------
// Type 26 (0xa18dc): projectile thrown in an arc by a creator, with shadow (h)
// ---------------------------------------------------------------------------------------------

/// After a delay, leaves the creator's position (`+0x5c`) at a height from table 0xa1968 (according to
/// the creator's `+0x60`), with speed table 3 (or 2). Falls back (Z < 0), stays 128 frames
/// on the ground then 96 blinking, before disappearing.
mod shot26;

// ---------------------------------------------------------------------------------------------
// Type 27 (0xa1a64): curtain of sparks from a creator, two variants (h)
// ---------------------------------------------------------------------------------------------

/// Variant 0: epicenter (animation 0, 96 frames) placed on the creator (`+0x5c`); at 80 frames
/// it creates a row of type 27 shots variant 1 on either side (`spawn_row`) and the
/// sound. Variant 1: an element of the row, blinking, which disappears with the epicenter.
mod shot27;

// ---------------------------------------------------------------------------------------------
// Type 28 (0xa1c96): fragment spinning around a creator (h)
// ---------------------------------------------------------------------------------------------

/// Fragment (4 variants, `+3`) thrown from the creator (`+0x5c`), at a height from table
/// 0xa1d18 according to the creator's `+0x60`, with speed table 0x3f; it advances and disappears
/// off screen (test one frame out of 4).
mod shot28;

// ---------------------------------------------------------------------------------------------
// Helpers for types 29 to 39
// ---------------------------------------------------------------------------------------------

/// AI animation start (`0x103d6`: `anim_start_indexed` then `+0x44 = +0x3e`).
fn ai_start<R: Bench>(x: &mut Cx<'_, R>, set: u32, n: u32) {
    crate::families::enemy_lib::start(x, set, n);
}

/// Pas d'animation de l'IA (`0x1040c`) ; vrai en fin de script.
fn ai_step<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    crate::families::enemy_lib::step(x)
}

/// Left abscissa of the current camera (`A5+0x506`, or `0x556` if the alternate camera is active).
fn camera_x<R: Bench>(x: &mut Cx<'_, R>) -> u16 {
    if x.a5_u8(0x36) != 0 { x.a5_u16(0x556) } else { x.a5_u16(0x506) }
}

/// Copies the position (X, Z, Y: integer parts) of an object.
fn copy_position16<R: Bench>(x: &mut Cx<'_, R>, from: Obj) {
    for off in [0x8, 0xc, 0x10] {
        let v = from.u16(x.b, off);
        x.set16(off, v);
    }
}

/// Creates a row of shots of type `kind` (variant 1) starting from `px`, every 0x38 toward the
/// right or the left up to the screen limits (`+0x58` numbers the elements).
fn spawn_row<R: Bench>(x: &mut Cx<'_, R>, kind: u8, mut px: u16, rightwards: bool) {
    let mut label: u16 = 1;
    loop {
        let Some(e) = x.alloc(&pools::ESHL) else { return };
        e.set_u8(x.b, 0, 1);
        e.set_u8(x.b, 2, kind);
        e.set_u8(x.b, 3, 1);
        e.set_u16(x.b, 0x8, px);
        let (z, y) = (x.u16(0xc), x.u16(0x10));
        e.set_u16(x.b, 0xc, z.wrapping_sub(1));
        e.set_u16(x.b, 0x10, y.wrapping_add(1));
        e.set_u16(x.b, 0x58, label);
        label = label.wrapping_add(1);
        e.set_u16(x.b, 0x5c, low16(x.o));
        let camera = camera_x(x);
        if rightwards {
            px = px.wrapping_add(0x38);
            if px > camera.wrapping_add(0x1c0) {
                return;
            }
        } else {
            px = px.wrapping_sub(0x38);
            if camera > px {
                return;
            }
        }
    }
}

/// Blinking according to the parity of `A5+0x6e` and of the variant: hidden when they differ.
fn blink_by_parity<R: Bench>(x: &mut Cx<'_, R>) {
    let hidden = (x.u8(0x3) ^ x.a5_u16(0x6e) as u8) & 1 != 0;
    x.set_bit(0x1, 1, hidden);
}

/// Template of types 32, 33, 35, 36 and 38: pause (suspended object), state, then, if the object still
/// exists, put back in the list.
fn guarded<R: Bench>(x: &mut Cx<'_, R>, state: fn(&mut Cx<'_, R>)) {
    if suspended(x) {
        return;
    }
    state(x);
    if x.u8(0x0) != 0 {
        x.reinsert();
    }
}

// ---------------------------------------------------------------------------------------------
// Type 29 (0xa1d70): flame / wave of a row created by a creator (h)
// ---------------------------------------------------------------------------------------------

/// Three variants: 0 = central element placed on the creator (`+0x5c`), which waits for the creator
/// to arm its attack (`+0xaf`) then creates a row of variant 1 elements (on either side,
/// `spawn_row`); 1 = element of the row, blinking, which goes out with the epicenter; 2 = the
/// delayed version (30 frames). All disappear with the creator (state >= 4 or 0x20a).
mod shot29;

// ---------------------------------------------------------------------------------------------
// Type 30 (0xa2060): hit delivered by an enemy, with sound (h)
// ---------------------------------------------------------------------------------------------

/// Sticks a hit to the owner (inverted orientation): waits for the owner to arm its attack
/// (`+0xaf`), plays the hit (animation 1, 48 frames), the return (animation 2), then cedes control.
mod shot30;

// ---------------------------------------------------------------------------------------------
// Type 31 (0xa21b6): homing ball from a creator (h)
// ---------------------------------------------------------------------------------------------

/// Ball that leaves the creator (`+0x5c`, side it faces) with speed table 0x30. It waits
/// 48 frames (`+0x42`), then every 10 frames orients its direction (`+0x2d`) toward it (angle +
/// `+0x58`, sign depending on the height of its target) and explodes on contact (0x20 box around the
/// creator). When the creator goes to state >= 0x406, it flies straight and leaves the screen.
mod shot31;

// ---------------------------------------------------------------------------------------------
// Types 32 and 33: shots of a host (+0x42), AI animations (0x103d6 / 0x1040c)
// ---------------------------------------------------------------------------------------------

/// True when the object's host (`+0x42`) is in state 4: the shot must disappear (0xa2510).
fn host_dead<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let host = x.linked(0x42);
    host.u8(x.b, 0x4) == 4
}

/// Type 32 (0xa239a): homing projectile fired at the host's position (h)
///
/// At the start (state 0) it aims at the owner (`+0x50`, variant 0) or at the position (`+0xbc/+0xbe`)
/// of the host (`+0x42`, other variants): direction `+0x2d` from the angle, vertical speed
/// computed to reach the target in a number of steps proportional to the distance. Then it flies
/// (state 2 substate 0: waits for flag `+0x58`), falls (substate 2) playing a sound if
/// its fall crosses the visible area, and ends with its animation (substate 4).
mod shot32;

/// Type 33 (0xa2524): three-stage shot tied to its host (h)
///
/// Same template as type 32 (suspension, dead host = host state 4). Animation 0 then, when
/// the animation raises `+0x58`, sound and animation 1; when it raises `+0x59`, animation 2 until
/// `+0x32` empties (`+0x5a` = 1), then end at the end of the script.
mod shot33;

// ---------------------------------------------------------------------------------------------
// Type 34 (0xa2630): staging of a boss attack, three variants (h)
// ---------------------------------------------------------------------------------------------

/// Staging object tied to its creator (`+0x5c`). Variant 0: full-screen veil (camera position,
/// 600 frames) with a SET scenery (type 0 variant 0x47) attached in both directions;
/// variant 1: same idea next to the creator (400 frames, a single scenery, exit animation);
/// variant 2: placed on the creator. On the first pass variant 0 loads four palettes
/// (0x91c2e0...) from the ROM, tinted by 0xf000f000.
mod shot34;

// ---------------------------------------------------------------------------------------------
// Type 35 (0xa28d2): falling spark with shadow, 16 variants (h)
// ---------------------------------------------------------------------------------------------

/// Decorative spark: speed table 0xb with direction `+0x2d` according to the variant (0, 2, ... 30),
/// flies 90 frames (`+0x58`), then animates its end. Its shadow is drawn by 0x33fe4; it blinks
/// according to the parity of `A5+0x6e` and of the variant.
mod shot35;

// ---------------------------------------------------------------------------------------------
// Type 36 (0xa29cc): debris thrown into the air, falls back onto the scene (h)
// ---------------------------------------------------------------------------------------------

/// Appears at a random position on the screen (tables 0xa2b0a / 0xa2b2a) at height 0x100 - Y,
/// with a randomly drawn vertical speed; falls back (Z <= 0), plays an impact sound then animates
/// its end.
mod shot36;

// ---------------------------------------------------------------------------------------------
// Type 37 (0xa2b62): meteorite thrown by a creator, falls back and explodes (h)
// ---------------------------------------------------------------------------------------------

/// Falls from the sky at a random position on the screen (tables 0xa2ce0 / 0xa2d00), random vertical speed
/// (0xa2c7e) and random start animation (0xa2c9e). On the ground (Z <= 0) it triggers
/// effect 0xb10c8 / 0xb4bcc, blinks 25 frames and disappears; it disappears with the creator.
mod shot37;

// ---------------------------------------------------------------------------------------------
// Type 38 (0xa2d48): projectile oriented toward its target (16 sprites), thrown by an effect (h)
// ---------------------------------------------------------------------------------------------

/// Created with its creator's pointer in `+0x46` (negative word = RAM address). Takes its position
/// (+0x30 in altitude), speeds from row `+0x2f` of table 0xa2ed4 (or 0xa2f32 for non-zero
/// variants), and its sprite from the angle toward the creator's target, bounded to 0x14..0x16
/// (mirrored if flipped). Then it flies (speed table + gravity) down to the ground.
mod shot38;

// ---------------------------------------------------------------------------------------------
// Type 39 (0xa2fb8): fireball thrown into the air by a creator, 8 variants (h)
// ---------------------------------------------------------------------------------------------

/// Leaves the creator (`+0x5c`) upward (VZ = -5, speed table 0x13), falls back spinning
/// (`+0x2d` varies), ignites on the ground (end animation) then signals the creator (`+0xae`).
/// Disappears with the creator (state 0x406).
mod shot39;
handlers! {
    0x9fe0a "eshl_15" shot15::run,
    0xa03e0 "eshl_16" shot16::run,
    0xa05ce "eshl_17" shot17::run,
    0xa0a06 "eshl_18" shot18::run,
    0xa0b20 "eshl_19" shot19::run,
    0xa0c5a "eshl_20" shot20::run,
    0xa0f96 "eshl_21" shot21::run,
    0xa1180 "eshl_22" shot22::run,
    0xa1292 "eshl_23" shot23::run,
    0xa137e "eshl_24" shot24::run,
    0xa16bc "eshl_25" shot25::run,
    0xa18dc "eshl_26" shot26::run,
    0xa1a64 "eshl_27" shot27::run,
    0xa1c96 "eshl_28" shot28::run,
    0xa1d70 "eshl_29" shot29::run,
    0xa2060 "eshl_30" shot30::run,
    0xa21b6 "eshl_31" shot31::run,
    0xa239a "eshl_32" shot32::run,
    0xa2524 "eshl_33" shot33::run,
    0xa2630 "eshl_34" shot34::run,
    0xa28d2 "eshl_35" shot35::run,
    0xa29cc "eshl_36" shot36::run,
    0xa2b62 "eshl_37" shot37::run,
    0xa2d48 "eshl_38" shot38::run,
    0xa2fb8 "eshl_39" shot39::run,
}
