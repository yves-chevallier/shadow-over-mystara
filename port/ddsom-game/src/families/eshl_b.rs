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
use crate::objects::pools;
use crate::objects::Current;
use crate::machine::Machine;

/// Sounds (wrappers of the 0xe0836-0xee080 area) and other effect routines.
mod sfx {
    pub const E4752: u32 = 0xe4752;
    pub const E4E5A: u32 = 0xe4e5a;
    pub const E4FB6: u32 = 0xe4fb6;
    pub const E5020: u32 = 0xe5020;
    pub const E7376: u32 = 0xe7376;
    pub const ED686: u32 = 0xed686;
}


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
// Type 15 (0x9fe0a) : arme d'un ennemi composite, sous-objet d'un sous-objet (h)
// ---------------------------------------------------------------------------------------------

/// Piece thrown by a part of a composite enemy (ESHL type 14 then its shot): the owner
/// (`+0x50`) is a type 14 shot whose owner is itself the enemy. Variant 0: projectile
/// that leaves in an arc toward the player (speeds chosen according to the distance between the enemy and its
/// target); variant 1: weapon stuck to the owner, which follows the attack animation.
mod shot15 {
    use super::*;

    const ANIMS: u32 = 0x17a276;
    /// Speeds (VX VZ AZ AX), one 8-byte row per range (8, 0x10, 0x18).
    const SPEEDS: u32 = 0xa034e;
    /// Substates of state 2 (`+5`).
    const FLY: u8 = 0;
    const HOLD: u8 = 2;
    const SWING: u8 = 4;
    /// Flight phases (`+6`).
    const PHASE_LAST: u8 = 8;
    const PHASE_BLINK: u8 = 10;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => spawn(x),
            2 => active(x),
            _ => x.free_eshl(),
        }
    }

    /// Creation: placed in front of the owner, depending on the variant.
    fn spawn<R: Bench>(x: &mut Cx<'_, R>) {
        x.set8(f::BOX_SET, 7);
        if x.u16(f::OWNER) == 0 {
            return kill(x);
        }
        let owner = x.linked(f::OWNER);
        let variant = x.u8(0x3);
        let (dx, dz, dy) = if variant == 0 { (0x67, 0x4a, 0x10) } else { (0x42, 0x2e, 0) };
        place_in_front(x, owner, dx, dz);
        let flip = owner.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        x.add16(0x10, dy);
        x.set8(0x43, 4);
        match variant {
            0 => {
                x.set32(0x4, 0x0200_0000);
                x.set8(f::ATTACK_POWER, 8);
                x.set8(f::ATTACK_ID, 0x10);
                aim(x);
                x.anim_start(ANIMS, 5);
            }
            1 => {
                x.set8(0x5d, 0);
                x.set8(f::ATTACK_POWER, 0xc);
                x.set8(f::ATTACK_ID, 0x11);
                crate::sound::play(x.b, sfx::E4752, x.o);
                let owner_state = owner.u32(x.b, 0x4);
                if owner_state == 0x0202_0004 {
                    x.set32(0x4, 0x0202_0000);
                    x.anim_start(ANIMS, 0x13);
                } else if owner_state == 0x0202_0204 {
                    x.set32(0x4, 0x0204_0000);
                    x.anim_start(ANIMS, 0x10);
                }
            }
            _ => {}
        }
    }

    /// Initial speeds of variant 0: range between the enemy (owner of the owner) and
    /// its target, row 8, 0x10 or 0x18 of `SPEEDS`.
    fn aim<R: Bench>(x: &mut Cx<'_, R>) {
        let part = x.linked(f::OWNER);
        let enemy = link_of(x, part, f::OWNER);
        let target = link_of(x, enemy, f::OWNER);
        let row = if enemy.u16(x.b, f::OWNER) == 0 {
            0x18
        } else {
            let flipped = enemy.u8(x.b, f::FLIP) != 0;
            let (tx, ex) = (target.u16(x.b, 0x8), enemy.u16(x.b, 0x8));
            let range = if flipped { tx.wrapping_sub(ex) } else { ex.wrapping_sub(tx) } as i16;
            if range <= 0xa8 {
                8
            } else if range <= 0xfc {
                0x10
            } else {
                0x18
            }
        };
        let mut vx = x.rom16(SPEEDS + row);
        if enemy.u8(x.b, f::FLIP) != 0 {
            vx = vx.wrapping_neg();
        }
        x.set16(0x14, vx);
        let vz = x.rom16(SPEEDS + row + 2);
        x.set16(0x18, vz);
        let az = x.rom16(SPEEDS + row + 4);
        x.set16(0x1a, az);
        let ax = x.rom16(SPEEDS + row + 6);
        x.set16(0x16, ax);
    }

    /// Copies the owner's position (offset in front of it).
    fn follow<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u16(f::OWNER) == 0 {
            return;
        }
        let owner = x.linked(f::OWNER);
        let flip = owner.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        place_in_front(x, owner, 0x42, 0x2e);
    }

    fn active<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u16(f::OWNER) == 0 {
            return kill(x);
        }
        if x.u8(0x3) != 0 {
            let part = x.linked(f::OWNER);
            let enemy = link_of(x, part, f::OWNER);
            if enemy.u16(x.b, 0x4) != 0x402 && (enemy.u8(x.b, 0x53) as i8) > 0 {
                return x.reinsert();
            }
        }
        match x.u8(0x5) {
            FLY => fly(x),
            HOLD => hold(x),
            SWING => swing(x),
            _ => {}
        }
        x.reinsert();
    }

    /// The owner has finished its attack (or is in state 0x402): the weapon disappears.
    fn owner_done<R: Bench>(x: &mut Cx<'_, R>) -> bool {
        let part = x.linked(f::OWNER);
        let enemy = link_of(x, part, f::OWNER);
        if enemy.u16(x.b, 0x4) == 0x402 || enemy.u8(x.b, 0xa8) & 1 != 0 {
            x.set8(0x5d, 1);
            x.set8(0x4, st::FREE);
            return true;
        }
        false
    }

    /// Flight of variant 0: the direction reverses every 5 frames, six phases of bounces
    /// (decreasing speeds), then blinking.
    fn fly<R: Bench>(x: &mut Cx<'_, R>) {
        let phase = x.u8(0x6);
        if phase > PHASE_BLINK {
            return;
        }
        x.add8(0x43, 0xff);
        if (x.u8(0x43) as i8) < 0 {
            let flip = x.u8(f::FLIP) ^ 0x20;
            x.set8(f::FLIP, flip);
            x.set8(0x43, 4);
        }
        if phase == PHASE_BLINK {
            x.add8(f::TIMER, 0xff);
            if (x.u8(f::TIMER) as i8) < 0 {
                return x.set8(0x4, st::FREE);
            }
            let visible = x.u8(f::TIMER) & 3 == 0;
            x.set_bit(0x1, 1, !visible);
            return;
        }
        if !x.move_xz().gt() {
            x.add8(0x6, 2);
            if phase == PHASE_LAST {
                x.set8(f::TIMER, 0x32);
            } else {
                // bounce: (VX, VZ) per phase
                let (vx, vz) = [(-0x180i16, 0x3c0), (-0x100, 0x300), (-0xc0, 0x280), (-0x80, 0x200)][phase as usize / 2];
                x.set32(0xc, 0);
                x.set16(0x14, vx as u16);
                x.set16(0x18, vz);
                x.set16(0x1a, -0x38i16 as u16);
                let part = x.linked(f::OWNER);
                let enemy = link_of(x, part, f::OWNER);
                if enemy.u8(x.b, f::FLIP) != 0 {
                    let v = x.u16(0x14).wrapping_neg();
                    x.set16(0x14, v);
                }
            }
        }
        x.cull_offscreen();
    }

    /// Variant 1 (held weapon): follows the owner while the enemy attacks.
    fn hold<R: Bench>(x: &mut Cx<'_, R>) {
        if x.a5_u8(GAME_PAUSED) != 0 {
            return follow(x);
        }
        if owner_done(x) {
            return;
        }
        if x.u8(0x6) == 0 {
            follow(x);
            if x.anim_step() {
                x.set8(0x5d, 1);
                x.set8(0x4, st::FREE);
            }
        }
    }

    /// Variant 1 (hit): three-stage animation (hit, 8-frame wait, return).
    fn swing<R: Bench>(x: &mut Cx<'_, R>) {
        if x.a5_u8(GAME_PAUSED) != 0 {
            return follow(x);
        }
        if owner_done(x) {
            return;
        }
        match x.u8(0x6) {
            0 => {
                follow(x);
                if x.anim_step() {
                    x.add8(0x6, 2);
                    x.set8(0x42, 8);
                    x.anim_start(ANIMS, 0x11);
                }
            }
            2 => {
                follow(x);
                if x.anim_step() {
                    x.add8(0x42, 0xff);
                    if (x.u8(0x42) as i8) >= 0 {
                        x.anim_start(ANIMS, 0x11);
                    } else {
                        x.add8(0x6, 2);
                        x.anim_start(ANIMS, 0x12);
                    }
                }
            }
            4 => {
                follow(x);
                if x.anim_step() {
                    x.set8(0x5d, 1);
                    x.set8(0x4, st::FREE);
                }
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 16 (0xa03e0): picked-up / carried object (h), placed in the foreground list
// ---------------------------------------------------------------------------------------------

/// Object attached to its owner (an enemy: HP in `+0x62`). As long as it is "carried"
/// (`+0x5a` zero) it follows the owner; otherwise it is thrown in an arc (speed table 0x18),
/// spins (`+0x2d` varies by 1 every 25 frames) and goes to state 4 when falling below Z = 0x100.
mod shot16 {
    use super::*;

    const ANIMS: u32 = 0x16e146;
    const HELD: u8 = 0;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(0x58, 4);
                let anim = (x.u8(0x43) != 0) as u32;
                x.anim_start(ANIMS, anim as u16);
                reinsert_foreground(x);
            }
            2 => active(x),
            4 => dropped(x),
            6 => x.free_eshl(),
            _ => {}
        }
    }

    fn active<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x5) {
            HELD => {
                let owner = x.linked(f::OWNER);
                if (owner.u8(x.b, 0x4) as i8) >= 4 {
                    x.set8(0x0, 2);
                }
                if x.u8(0x5a) == 0 {
                    x.anim_step();
                    copy_position32(x, owner);
                    let dx = if x.u8(f::FLIP) != 0 { -0x2ai16 } else { 0x2a };
                    x.add16(0x8, dx as u16);
                    x.add16(0xc, -0x78i16 as u16);
                    x.add32(0x10, 1);
                    x.add32(0xc, 0xffff_ffff);
                    reinsert_foreground(x);
                } else {
                    launch(x);
                }
            }
            _ => flying(x),
        }
    }

    /// Goes to the throw: speed table 0x18, the vertical speed becomes that of Y.
    fn launch<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x5, 2);
        speed_table(x, 0x18);
        let v = x.u32(0x1c);
        x.set32(0x18, v);
        let angle = if x.u8(f::FLIP) != 0 { 0xa } else { 0x16 };
        x.set8(0x2d, angle);
        x.set8(0x55, 2);
        reinsert_foreground(x);
    }

    fn flying<R: Bench>(x: &mut Cx<'_, R>) {
        x.move_by_table_xz();
        x.anim_step();
        if x.u8(0x2d) != 0 {
            x.add8(0x55, 0xff);
            if (x.u8(0x55) as i8) < 0 {
                x.set8(0x55, 0x18);
                let step: u8 = if x.u8(f::FLIP) != 0 { 0xff } else { 1 };
                let v = x.u8(0x2d).wrapping_add(step) & 0x1f;
                x.set8(0x2d, v);
            }
        }
        if (x.u16(0xc) as i16) > 0x100 {
            x.add8(0x4, 4);
        }
        reinsert_foreground(x);
    }

    /// State 4: as long as the owner has HP, nothing; otherwise the object is thrown (or released).
    fn dropped<R: Bench>(x: &mut Cx<'_, R>) {
        let owner = x.linked(f::OWNER);
        if (owner.u16(x.b, 0x62) as i16) <= 0 {
            x.set8(0x0, 2);
            x.set32(0x4, 0x0200_0000);
            return launch(x);
        }
        x.set32(0x4, 0x0200_0000);
        if x.u8(0x5a) != 0 {
            x.set8(0x0, 2);
            x.set32(0x58, 0);
            return;
        }
        x.set32(0x58, 0);
        owner.set_u32(x.b, 0x4, 0x0402_0000);
        if (x.u16(0x62) as i16) > 0 {
            x.add8(0x5, 2);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 17 (0xa05ce): projectile with aimed trajectory, three modes according to +0x43 (h)
// ---------------------------------------------------------------------------------------------

/// Projectile that appears while charging (animation 0, 60 frames) then heads toward its target (`+0x50`,
/// long pointer) with a computed vertical speed/angle; `+0x43` picks the mode: 0 aim
/// the angle by table (`1386`), < 0 fire flat toward the target, > 0 aim in depth. The parent
/// (`+0x56`, long pointer) maintains it: if it goes to state >= 4 the projectile dies. At
/// startup it brightens palette 0x91c300 (copy of 0x25a660 tinted by 0x91c000).
mod shot17 {
    use super::*;

    const ANIMS: u32 = 0x16e1f4;
    const PALETTE_SRC: u32 = 0x25a660;
    const PALETTE_DST: u32 = 0x91c300;
    const PALETTE_TINT: u32 = 0x91c000;
    /// Spawn offset (Z, X) by `+0x42` (4 bytes per row).
    const SPAWN_OFFSETS: u32 = 0xa0670;
    /// `+0x2d` according to the angle (Y) for mode 0 and mode > 0.
    const AIM_TABLE_B: u32 = 0xa07ac;
    const AIM_TABLE_A: u32 = 0xa0892;
    /// Vertical and horizontal speed of mode < 0 (4 bytes per row).
    const DROP_SPEEDS: u32 = 0xa0962;
    const CHARGE: u8 = 0;
    const AIM: u8 = 2;
    const FALL: u8 = 4;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => init(x),
            2 => active(x),
            4 => {
                if x.anim_step() {
                    x.add8(0x4, 2);
                }
            }
            _ => x.free_eshl(),
        }
    }

    fn init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        for i in 0..8 {
            let v = x.data32(PALETTE_SRC + 4 * i);
            wr32(x.b, PALETTE_DST + 4 * i, v);
        }
        let tint = rd32(x.b, PALETTE_TINT) & 0xf000_f000;
        for i in 0..8 {
            let v = rd32(x.b, PALETTE_DST + 4 * i) | tint;
            wr32(x.b, PALETTE_DST + 4 * i, v);
        }
        x.set8(0x2a, 1);
        x.set8(0x2b, 0x18);
        x.set8(0x54, 0x3c);
        let row = x.u8(0x42) as u32;
        let dz = x.rom16(SPAWN_OFFSETS + row);
        x.add16(0xc, dz);
        let mut dx = x.rom16(SPAWN_OFFSETS + 2 + row);
        if x.u8(f::FLIP) != 0 {
            dx = dx.wrapping_neg();
        }
        x.add16(0x8, dx);
        x.add32(0x10, 0xffff_ffff);
        x.set8(f::BOX_SET, 7);
        x.set8(f::ATTACK_POWER, 8);
        x.set8(f::ATTACK_ID, 2);
        x.anim_start(ANIMS, 0);
        x.reinsert();
    }

    /// The parent has left its normal states: the projectile disappears.
    fn check_parent<R: Bench>(x: &mut Cx<'_, R>) {
        let parent = ptr(x, 0x56);
        if (parent.u8(x.b, 0x4) as i8) >= 4 {
            kill(x);
        }
    }

    fn active<R: Bench>(x: &mut Cx<'_, R>) {
        let mode = x.u8(0x43);
        let state = x.u8(0x5);
        match state {
            CHARGE => charge(x),
            AIM => {
                if mode == 0 {
                    aim_flat(x)
                } else if (mode as i8) < 0 {
                    aim_drop(x)
                } else {
                    aim_angle(x)
                }
            }
            FALL => {
                if mode == 0 {
                    land(x, |x| { x.move_by_table_xz(); }, true)
                } else if (mode as i8) < 0 {
                    land(x, |x| { x.move_xz(); }, true)
                } else {
                    land_xyz(x)
                }
            }
            _ => {}
        }
        x.reinsert();
    }

    /// Charge : animation 0 pendant 60 images.
    fn charge<R: Bench>(x: &mut Cx<'_, R>) {
        let parent = ptr(x, 0x56);
        if (parent.u8(x.b, 0x4) as i8) >= 4 {
            return kill(x);
        }
        x.anim_step();
        x.add8(0x54, 0xff);
        if (x.u8(0x54) as i8) < 0 {
            x.add8(0x5, 2);
        }
        check_parent(x);
    }

    /// Start of the firing phase: announcement to the parent (`+0xa5`), 20-frame wait.
    fn begin_aim<R: Bench>(x: &mut Cx<'_, R>) -> bool {
        if x.u8(0x6) == 0 {
            x.add8(0x6, 2);
            x.set8(0x54, 0x14);
            x.anim_start(ANIMS, 1);
            return false;
        }
        true
    }

    /// After the wait: notifies the parent, true when the projectile must leave.
    fn ready_to_go<R: Bench>(x: &mut Cx<'_, R>) -> bool {
        x.anim_step();
        x.add8(0x54, 0xff);
        if (x.u8(0x54) as i8) >= 0 {
            return false;
        }
        let parent = ptr(x, 0x56);
        parent.set_u8(x.b, 0xa5, 1);
        x.u8(0x55) != 0
    }

    /// Mode > 0: aims at the target (distance/altitude ratio, angle).
    fn aim_angle<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(0x6) == 0 {
            x.add8(0x6, 2);
            x.set8(0x54, 0x14);
            x.anim_start(ANIMS, 1);
            return;
        }
        let parent = ptr(x, 0x56);
        if (parent.u8(x.b, 0x4) as i8) >= 4 {
            return kill(x);
        }
        if ready_to_go(x) {
            x.add8(0x5, 2);
            x.set8(0x2d, 0x10);
            if x.u8(0x3) == 8 {
                x.set32(0x14, 0);
                x.set32(0x18, 0xf800_0000);
                x.set32(0x1c, 0xfb80_0000);
            } else {
                let target = ptr(x, f::OWNER);
                if target.u8(x.b, 0) == 0 {
                    return x.set8(0x4, st::FREE);
                }
                aim_at(x, target);
            }
        }
        check_parent(x);
    }

    /// Computes the vertical speed (slope toward the target) and the angle `+0x2d`.
    fn aim_at<R: Bench>(x: &mut Cx<'_, R>, target: Obj) {
        let dy = x.u32(0x10).wrapping_sub(target.u32(x.b, 0x10));
        let mut dx = x.u32(0x8).wrapping_sub(target.u32(x.b, 0x8));
        if (dx as i32) < 0 {
            dx = dx.wrapping_neg();
        }
        let far = if (dx as i32) < (dy as i32) { dy } else { dx } >> 3;
        let quotient = crate::aids::divide(x.u32(0xc), far).0 << 7;
        let slope = (quotient as u16).wrapping_add(((far >> 16) as u16) << 2);
        x.set16(0x18, slope.wrapping_neg());
        let angle = crate::aids::direction_to(x.b, x.o, target) & 0x1f;
        let idx = angle.wrapping_sub(8) as u32;
        let v = x.rom8(AIM_TABLE_B + idx);
        x.set8(0x2d, v);
        speed_table(x, 0x40);
    }

    /// Mode 0: firing by angle table.
    fn aim_flat<R: Bench>(x: &mut Cx<'_, R>) {
        if !begin_aim(x) {
            return;
        }
        if ready_to_go(x) {
            x.add8(0x5, 2);
            let target = ptr(x, f::OWNER);
            if target.u8(x.b, 0) == 0 {
                return x.set8(0x4, st::FREE);
            }
            let to = (target.u16(x.b, 0x8), 0);
            let de = (x.u16(0x8), x.u16(0xc));
            let angle = crate::aids::direction(x.b, de, to) & 0x1f;
            let v = x.rom8(AIM_TABLE_A + angle.wrapping_sub(8) as u32);
            x.set8(0x2d, v);
            speed_table(x, 0x44);
            let v = x.u16(0x1c);
            x.set16(0x18, v);
        }
        check_parent(x);
    }

    /// Mode < 0: falls in front of the target (speed according to the distance in X).
    fn aim_drop<R: Bench>(x: &mut Cx<'_, R>) {
        if !begin_aim(x) {
            return;
        }
        if ready_to_go(x) {
            x.add8(0x5, 2);
            let target = ptr(x, f::OWNER);
            if target.u8(x.b, 0) == 0 {
                return x.set8(0x4, st::FREE);
            }
            let mut d = target.u16(x.b, 0x8).wrapping_sub(x.u16(0x8));
            if (d as i16) < 0 {
                d = d.wrapping_neg();
            }
            let row = if d > 0x50 { 0 } else { 4 };
            let vz = x.rom16(DROP_SPEEDS + row);
            x.set16(0x18, vz);
            x.set16(0x1a, 0);
            x.set16(0x14, 0);
        }
        check_parent(x);
    }

    /// Fall after the shot (modes 0 and < 0): touches the ground (Z < 0) → impact animation.
    fn land<R: Bench>(x: &mut Cx<'_, R>, mover: fn(&mut Cx<'_, R>), clear_state: bool) {
        mover(x);
        x.anim_step();
        if (x.u16(0xc) as i16) >= 0 {
            return;
        }
        x.add8(0x4, 2);
        if clear_state {
            x.set8(0x5, 0);
        }
        x.set32(0xc, 0);
        x.anim_start(ANIMS, 2);
        crate::sound::play(x.b, sfx::ED686, x.o);
    }

    /// Fall of mode > 0: movement in X, Y and Z.
    fn land_xyz<R: Bench>(x: &mut Cx<'_, R>) {
        x.move_xyz();
        x.anim_step();
        if (x.u16(0xc) as i16) > 0 {
            return;
        }
        x.add8(0x4, 2);
        x.set8(0x2a, 0);
        x.set32(0xc, 0);
        x.anim_start(ANIMS, 2);
        crate::sound::play(x.b, sfx::ED686, x.o);
    }
}

// ---------------------------------------------------------------------------------------------
// Type 18 (0xa0a06): oblique projectile with speed taken from a table (h)
// ---------------------------------------------------------------------------------------------

/// Projectile whose direction depends on the orientation, speed table 1b64 chosen by a global variable
/// (A5+0x7f92); destroyed on contact, on certain terrains, or off screen.
mod shot18 {
    use super::*;

    const ANIMS: u32 = 0x16e534;
    /// Speed table number (byte) according to A5+0x7f92.
    const SPEED_TABLES: u32 = 0xa0a60;
    const LEVEL_VARIABLE: u32 = 0x7f92;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(f::BOX_SET, 7);
                x.set8(f::ATTACK_POWER, 6);
                x.set8(f::ATTACK_ID, 0x1a);
                let angle = if x.u8(f::FLIP) != 0 { 8 } else { 0x18 };
                x.set8(0x2d, angle);
                let level = x.a5_u16(LEVEL_VARIABLE);
                let table = x.rom8(SPEED_TABLES.wrapping_add(level as i16 as i32 as u32));
                speed_table(x, level as u32 & 0xff00 | table as u32);
                x.anim_start(ANIMS, 7);
            }
            2 => fly(x),
            _ => x.free_eshl(),
        }
    }

    fn fly<R: Bench>(x: &mut Cx<'_, R>) {
        let hit = x.u8(f::HIT_FLAGS);
        if hit & 0x08 != 0 {
            crate::sound::play(x.b, sfx::E7376, x.o);
            return x.free_eshl();
        }
        if hit & 0x20 != 0 {
            x.add8(0x4, 2);
            return x.free_eshl();
        }
        if blocked_by_terrain(x) {
            return x.free_eshl();
        }
        x.move_by_table_xy();
        x.anim_step();
        x.cull_offscreen();
        x.reinsert();
    }
}

// ---------------------------------------------------------------------------------------------
// Type 19 (0xa0b20): projectile thrown toward the player, sprite depending on the angle (h)
// ---------------------------------------------------------------------------------------------

/// Projectile that chooses its direction (8 sprites) from the angle toward the target (`+0x50`, long
/// pointer), is launched with speed table 0x40 (VZ = VY), falls until Z < 0 then stays 48
/// images en animation d'impact.
mod shot19 {
    use super::*;

    const ANIMS: u32 = 0x16e5fa;
    /// Rows of 16 bytes (one per direction): (value of `+0x2d`, animation) per angle step.
    const STEPS_FACING_RIGHT: u32 = 0xa0bea;
    const STEPS_FACING_LEFT: u32 = 0xa0bda;
    /// Offsets (X, Z) of the sprite per angle step.
    const OFFSETS: u32 = 0xa0bfa;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => init(x),
            2 => {
                if (x.u16(0xc) as i16) >= 0 {
                    x.move_xz();
                } else {
                    x.set32(0xc, 0);
                    x.add8(0x4, 2);
                    x.set8(0x58, 0x30);
                    x.anim_start(ANIMS, 0x10);
                }
            }
            4 => {
                x.anim_step();
                x.add8(0x58, 0xff);
                if (x.u8(0x58) as i8) < 0 {
                    x.add8(0x4, 2);
                }
            }
            _ => x.free_eshl(),
        }
    }

    fn init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        x.set8(f::BOX_SET, 7);
        x.set8(f::ATTACK_POWER, 5);
        x.set8(f::ATTACK_ID, 0x19);
        let target = ptr(x, f::OWNER);
        if target.u8(x.b, 0) == 0 {
            return x.add8(0x6, 4);
        }
        let de = (x.u16(0x8), x.u16(0xc));
        let to = (target.u16(x.b, 0x8), 0);
        let angle = crate::aids::direction(x.b, de, to) & 0x1f;
        let (base, steps) = if x.u8(f::FLIP) != 0 { (9, STEPS_FACING_LEFT) } else { (0x10, STEPS_FACING_RIGHT) };
        let r = angle.wrapping_sub(base);
        let step = if r == 0 {
            0
        } else if (r as i8) < 0 || r > 8 {
            7
        } else {
            r as u32
        };
        let entry = steps + 2 * step;
        let sprite = x.data8(entry);
        x.set8(0x2d, sprite);
        let anim = x.data8(entry + 1) as i8 as i16 as u16 as u32;
        let off = OFFSETS + 4 * step;
        let dx = x.data16(off);
        x.add16(0x8, dx);
        let dz = x.data16(off + 2);
        x.add16(0xc, dz);
        x.set8(f::FLIP, 0);
        x.anim_start(ANIMS, anim as u16);
        speed_table(x, 0x40);
        let v = x.u16(0x1c);
        x.set16(0x18, v);
        x.set16(0x1c, 0);
        x.reinsert();
    }
}

// ---------------------------------------------------------------------------------------------
// Helper shared by types 20 and 21: sticking to the owner through an offset table
// ---------------------------------------------------------------------------------------------

/// Long read in program space (table addressed PC-relative).
fn rom32<R: Bench>(x: &mut Cx<'_, R>, addr: u32) -> u32 {
    (x.rom16(addr) as u32) << 16 | x.rom16(addr + 2) as u32
}

/// Sticks to the owner with the offset of row `variant` of a table (8 bytes per row:
/// X, integer Y, signed fractional Y, Z). With no owner, the object is released on the next frame.
fn attach<R: Bench>(x: &mut Cx<'_, R>, table: u32) {
    if x.u16(f::OWNER) == 0 {
        return kill(x);
    }
    let owner = x.linked(f::OWNER);
    copy_position32(x, owner);
    let flip = owner.u8(x.b, f::FLIP);
    x.set8(f::FLIP, flip);
    let row = table + (x.u8(0x3) << 3) as u32;
    let mut dx = x.rom16(row);
    if flip != 0 {
        dx = dx.wrapping_neg();
    }
    x.add16(0x8, dx);
    let dy = x.rom16(row + 2);
    x.add16(0x10, dy);
    let dy_fraction = x.rom16(row + 4) as i16 as i32 as u32;
    x.add32(0x10, dy_fraction);
    let dz = x.rom16(row + 6);
    x.add16(0xc, dz);
}

// ---------------------------------------------------------------------------------------------
// Type 20 (0xa0c5a): hit of an enemy part (h), two variants
// ---------------------------------------------------------------------------------------------

/// Weapon stuck to its owner (`attach`). Variant 0: randomly drawn idle animation then,
/// when the owner arms its attack (`+0xa5` bit 0), state 2 substate 2: hit animation
/// that creates a type 21 shot; variant 1: delayed weapon (`+0x5c` frames), which strikes
/// too. The states frozen by the pause (`A5+0x3d`, `A5+0x4e`) or the suspended object freeze it.
mod shot20 {
    use super::*;

    const ANIMS: u32 = 0x17d7d2;
    const ANIMS_STRIKE: u32 = 0x17d7e0;
    const ATTACH: u32 = 0xa0f86;
    /// Animation d'attente selon un tirage 0..7.
    const IDLE_ANIMS: u32 = 0xa0d60;
    /// Wait (frames) of variant 1 from a 0..7 draw.
    const DELAYS: u32 = 0xa0e2c;
    const FROZEN: u32 = 0x4e;
    const ARMED: u32 = 0xa5;
    /// Substates (`+5`).
    const IDLE: u8 = 0;
    const STRIKE: u8 = 2;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => init(x),
            2 => active(x),
            _ => x.free_eshl(),
        }
    }

    fn init<R: Bench>(x: &mut Cx<'_, R>) {
        if x.a5_u8(GAME_PAUSED) != 0 {
            return;
        }
        x.add8(0x4, 2);
        x.set8(0x1d, 0);
        attach(x, ATTACH);
        let mut anim = 3;
        if x.u8(0x3) != 0 {
            x.set16(f::TIMER, 0x1e);
            anim = 2;
        }
        x.anim_start(ANIMS, anim);
    }

    fn active<R: Bench>(x: &mut Cx<'_, R>) {
        if x.a5_u8(GAME_PAUSED) != 0 {
            return;
        }
        if x.a5_u8(FROZEN) != 0 || suspended(x) {
            return freeze(x);
        }
        match x.u8(0x5) {
            IDLE => idle(x),
            STRIKE => strike(x),
            _ => {}
        }
        x.reinsert();
    }

    /// Frozen object: variant 0 goes back once to state 2 and gives the weapon back to the owner.
    fn freeze<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(0x3) != 0 || x.u8(0x1d) != 0 {
            return;
        }
        x.set8(0x1d, 1);
        x.set32(0x4, 0x0200_0000);
        let owner = x.linked(f::OWNER);
        let armed = owner.u8(x.b, ARMED) & !1;
        owner.set_u8(x.b, ARMED, armed);
        crate::sound::play(x.b, sfx::E5020, x.o);
        x.anim_start(ANIMS, 3);
    }

    fn idle<R: Bench>(x: &mut Cx<'_, R>) {
        match (x.u8(0x6), x.u8(0x3)) {
            (0, 0) => {
                x.set8(0x1d, 0);
                x.add8(0x6, 2);
                let rng = x.random();
                let anim = x.rom8(IDLE_ANIMS + (rng as u32 & 7)) as u32;
                x.anim_start(ANIMS, anim as u16);
                attach(x, ATTACH);
                x.anim_step();
            }
            (0, _) => {
                x.add16(f::TIMER, 0xffff);
                if (x.u16(f::TIMER) as i16) < 0 {
                    x.add8(0x6, 2);
                    x.set8(f::BOX_SET, 7);
                    x.set8(f::ATTACK_ID, 0x28);
                    x.anim_start(ANIMS_STRIKE, 0);
                }
                attach(x, ATTACH);
                x.anim_step();
            }
            (2, 0) => {
                let owner = x.linked(f::OWNER);
                if (owner.u32(x.b, 0x4) as i32) >= 0x0200_0600 && x.u8(0x33) != 0 {
                    x.set8(0x33, 0);
                    crate::sound::play(x.b, sfx::E4E5A, x.o);
                }
                if x.anim_step() {
                    if x.u16(f::OWNER) == 0 {
                        return kill(x);
                    }
                    let owner = x.linked(f::OWNER);
                    if owner.u8(x.b, ARMED) & 1 != 0 {
                        return x.set32(0x4, 0x0202_0000);
                    }
                    x.add8(0x6, 0xfe);
                }
                attach(x, ATTACH);
                x.anim_step();
            }
            (2, _) => {
                if x.anim_step() {
                    x.add8(0x6, 0xfe);
                    let rng = x.random();
                    let delay = x.rom16(DELAYS + 2 * (rng as u32 & 7));
                    x.set16(f::TIMER, delay);
                    x.anim_start(ANIMS, 2);
                } else {
                    attach(x, ATTACH);
                }
            }
            _ => {}
        }
    }

    fn strike<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x6) {
            0 => {
                x.add8(0x6, 2);
                x.anim_start(ANIMS_STRIKE, 1);
                attach(x, ATTACH);
                x.anim_step();
            }
            2 => {
                if !x.anim_step() {
                    return attach(x, ATTACH);
                }
                crate::sound::play(x.b, sfx::E4E5A, x.o);
                crate::sound::play(x.b, sfx::E4FB6, x.o);
                x.add8(0x6, 2);
                // Original glitch: the allocation failure is not tested (see `Pool::alloc_unchecked`).
                let blow = x.alloc_unchecked(&pools::ESHL);
                let active = x.u8(0x0);
                blow.set_u8(x.b, 0, active);
                blow.set_u8(x.b, 3, 0);
                blow.set_u8(x.b, 2, 0x15);
                blow.set_u16(x.b, f::OWNER, low16(x.o));
                x.set16(0x42, low16(blow));
                x.anim_start(ANIMS_STRIKE, 2);
            }
            4 => {
                let blow_gone = x.u16(0x42) == 0 || {
                    let blow = x.linked(0x42);
                    blow.u8(x.b, 0x1e) == 1
                };
                if blow_gone {
                    x.add8(0x6, 2);
                    x.anim_start(ANIMS_STRIKE, 3);
                } else {
                    attach(x, ATTACH);
                    x.anim_step();
                }
            }
            6 => {
                if x.anim_step() {
                    x.set32(0x4, 0x0200_0000);
                    let owner = x.linked(f::OWNER);
                    let armed = owner.u8(x.b, ARMED) & !1;
                    owner.set_u8(x.b, ARMED, armed);
                    crate::sound::play(x.b, sfx::E5020, x.o);
                    x.anim_start(ANIMS, 3);
                } else {
                    attach(x, ATTACH);
                }
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 21 (0xa0f96): wave / breath attached to a weapon (h)
// ---------------------------------------------------------------------------------------------

/// Shot created by type 20: stuck to the owner (`attach`), animation of 0x4f repetitions
/// (`+0x42`) during which it creates an effect (EFFCT 0x18, saved in `+0x46`), then
/// end animation. It disappears if the game is frozen (`A5+0x4e`) or if the holder of its
/// owner is in state `+0xb2`.
mod shot21 {
    use super::*;

    const ANIMS: u32 = 0x16e752;
    const ATTACH: u32 = 0xa113a;
    const LOOP: u8 = 0;
    const END: u8 = 2;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => init(x),
            2 => active(x),
            _ => {
                crate::sound::play(x.b, sfx::E5020, x.o);
                x.free_eshl()
            }
        }
    }

    fn init<R: Bench>(x: &mut Cx<'_, R>) {
        if x.a5_u8(GAME_PAUSED) != 0 {
            return;
        }
        x.add8(0x4, 2);
        attach(x, ATTACH);
        x.set8(f::BOX_SET, 7);
        x.set8(f::ATTACK_ID, 0xb);
        x.set8(0x42, 0x4f);
        x.set8(0x1e, 0);
        x.anim_start(ANIMS, 2);
        crate::sound::play(x.b, sfx::E4FB6, x.o);
    }

    fn burn<R: Bench>(x: &mut Cx<'_, R>) {
        x.set8(0x1e, 1);
        kill(x);
    }

    fn active<R: Bench>(x: &mut Cx<'_, R>) {
        if x.a5_u8(GAME_PAUSED) != 0 {
            return;
        }
        if x.a5_u8(0x4e) != 0 {
            return burn(x);
        }
        let weapon = x.linked(f::OWNER);
        let host = link_of(x, weapon, f::OWNER);
        if host.u8(x.b, 0xb2) != 0 || suspended(x) {
            return burn(x);
        }
        match x.u8(0x5) {
            LOOP => {
                attach(x, ATTACH);
                if !x.anim_step() {
                    return x.reinsert();
                }
                x.add8(0x42, 0xff);
                if (x.u8(0x42) as i8) < 0 {
                    let effect = x.linked(0x46);
                    effect.set_u8(x.b, 0x43, 1);
                    x.add8(0x5, 2);
                    x.anim_start(ANIMS, 1);
                } else {
                    if x.u8(0x46) == 0 {
                        if let Some(e) = x.alloc(&pools::EFFCT) {
                            e.set_u8(x.b, 0, 1);
                            e.set_u8(x.b, 3, 0);
                            e.set_u8(x.b, 2, 0x18);
                            e.set_u16(x.b, f::OWNER, low16(x.o));
                            x.set16(0x46, low16(e));
                        }
                    }
                    x.anim_start(ANIMS, 0);
                }
            }
            END => {
                attach(x, ATTACH);
                if x.anim_step() {
                    burn(x);
                }
            }
            _ => {}
        }
        x.reinsert();
    }
}

// ---------------------------------------------------------------------------------------------
// Type 22 (0xa1180): spark thrown by an enemy (h)
// ---------------------------------------------------------------------------------------------

/// Spark that leaves the creator's position with a speed drawn from a per-variant table
/// (speeds in X and Z), falls until Z <= 0 then blinks for 30 frames.
mod shot22 {
    use super::*;

    const ANIMS: u32 = 0x1649d0;
    /// Offsets (X, Z) according to `+0x2f` (4 bytes per row).
    const OFFSETS: u32 = 0xa1194;
    /// Speeds (VX+AX, VZ+AZ) according to the variant (8 bytes per row).
    const SPEEDS: u32 = 0xa123a;
    const FLY: u8 = 0;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => init(x),
            2 => {
                if x.u8(0x5) == FLY {
                    x.move_xz();
                    if (x.u16(0xc) as i16) > 0 {
                        return;
                    }
                    x.set32(0xc, 0);
                    x.add8(0x5, 2);
                } else {
                    let hidden = !x.bit(0x1, 1);
                    x.set_bit(0x1, 1, hidden);
                    x.add8(0x54, 0xff);
                    if x.u8(0x54) == 0 {
                        x.free_eshl();
                    }
                }
            }
            _ => x.free_eshl(),
        }
    }

    fn init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        x.set8(0x54, 0x1e);
        x.set8(f::ATTACK_ID, 0x1a);
        x.set8(f::ATTACK_POWER, 5);
        let row = OFFSETS + ((x.u8(0x2f) << 2) as u32);
        let mut dx = x.rom16(row);
        let dz = x.rom16(row + 2);
        x.add16(0xc, dz);
        x.add16(0x10, 0xffff);
        x.add16(0xc, 1);
        let variant = x.u8(0x3);
        let row = SPEEDS + ((variant as u32) << 3);
        let vx = rom32(x, row);
        x.set32(0x14, vx);
        let vz = rom32(x, row + 4);
        x.set32(0x18, vz);
        if x.u8(f::FLIP) != 0 {
            dx = dx.wrapping_neg();
            x.set32(0x14, vx.wrapping_neg());
        }
        x.add16(0x8, dx);
        x.anim_start(ANIMS, (variant.wrapping_add(10) as u32) as u16);
        x.reinsert();
    }
}

// ---------------------------------------------------------------------------------------------
// Type 23 (0xa1292): scenery object that slides in front of the camera (h)
// ---------------------------------------------------------------------------------------------

/// Object placed at the camera's border (`A5+0x556`), which moves according to speed table 0xd
/// for 48 frames, waits for the signal `A5+0x42` then leaves again with table 2.
mod shot23 {
    use super::*;

    const ANIMS: u32 = 0x1c03a6;
    const WAIT: u8 = 0;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => init(x),
            2 => match x.u8(0x5) {
                WAIT => {
                    x.add8(0x54, 0xff);
                    if (x.u8(0x54) as i8) < 0 {
                        x.add8(0x5, 2);
                    }
                    slide(x);
                }
                _ => {
                    if x.a5_u8(0x42) == 0 {
                        x.anim_step();
                        return;
                    }
                    x.set8(0x54, 0x30);
                    x.add8(0x4, 2);
                    x.set8(0x2d, 0x10);
                    speed_table(x, 2);
                    x.anim_step();
                }
            },
            4 => {
                x.add8(0x54, 0xff);
                if (x.u8(0x54) as i8) < 0 {
                    x.add8(0x4, 2);
                }
                slide(x);
            }
            _ => x.free_eshl(),
        }
    }

    fn slide<R: Bench>(x: &mut Cx<'_, R>) {
        x.move_by_table_xy();
        x.anim_step();
        reinsert_foreground(x);
    }

    fn init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        x.set8(0x54, 0x30);
        x.set8(0x2d, 0);
        speed_table(x, 0xd);
        let camera = x.a5_u16(0x556);
        x.set16(0x8, camera);
        x.set32(0xc, 0);
        x.set32(0x10, 0);
        x.set16(0xc, 0x70);
        x.add16(0x10, -0x20i16 as u16);
        let mut anim = 0x4a;
        if x.u8(0x2f) != 0 {
            anim += 0x9f;
            x.add16(0x10, 0x80);
            x.add16(0xc, -0x10i16 as u16);
        }
        x.anim_start(ANIMS, anim);
    }
}

// ---------------------------------------------------------------------------------------------
// Type 24 (0xa137e): wave of enemies / shots launched by a creator (h)
// ---------------------------------------------------------------------------------------------

/// Variant 0: generator. It places itself on its creator (`+0x5c`), then for `+0x58` frames it
/// launches a type 24 shot (variant 1) every 2 frames and, at `+0x58 == 0x20`, spawns
/// a group of enemies (ENEMY type 1, number and positions according to `A5+0x7f92` and `A5+0x35`).
/// Variant 1: effect that it launches, at one of the 16 positions of table 0xa1438 (`+0x56`).
mod shot24 {
    use super::*;

    const ANIMS: u32 = 0x16e920;
    /// Offsets (X, Z, Y) of the 16 effects (8 bytes per row).
    const OFFSETS: u32 = 0xa1438;
    /// Nombre d'ennemis selon (A5+0x7f92 / 2) * 4 + A5+0x35.
    const GROUP_SIZES: u32 = 0xa15ae;
    /// Relative positions (X, Y) of the enemies: 2 pairs per row, drawn at random.
    const GROUP_OFFSETS: u32 = 0xa15ee;
    const GROUP_SIZE_SELECTOR: u32 = 0x7f92;
    const SPAWN_ENEMIES_AT: u16 = 0x20;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => init(x),
            2 => active(x),
            4 => x.add8(0x4, 2),
            _ => x.free_eshl(),
        }
    }

    fn init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        let source = x.linked(0x5c);
        if x.u8(0x3) == 0 {
            x.set8(f::BOX_SET, 7);
            x.set8(f::ATTACK_ID, 0x2f);
            x.set16(0x58, 0x50);
            x.set16(0x5a, 1);
            x.set16(0x56, 0);
            copy_position16(x, source);
            x.anim_start(ANIMS, 2);
            x.reinsert();
            x.set32(0x34, 0);
            active(x);
        } else {
            copy_position16(x, source);
            let row = OFFSETS + ((x.u16(0x56) << 3) as u32);
            for (off, column) in [(0x8, 0), (0xc, 2), (0x10, 4)] {
                let d = x.rom16(row + column);
                x.add16(off, d);
            }
            x.anim_start(ANIMS, 1);
            x.reinsert();
        }
    }

    fn active<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(0x3) != 0 {
            if x.anim_step() {
                x.add8(0x4, 2);
            }
            return;
        }
        x.add16(0x58, 0xffff);
        if x.u16(0x58) == 0 {
            x.add8(0x4, 2);
        }
        if x.u16(0x58) == SPAWN_ENEMIES_AT {
            spawn_enemies(x);
        }
        x.add16(0x5a, 0xffff);
        if x.u16(0x5a) == 0 {
            x.set16(0x5a, 2);
            spawn_effect(x);
        }
    }

    /// Launches the next effect (variant 1) and advances `+0x56` modulo 16.
    fn spawn_effect<R: Bench>(x: &mut Cx<'_, R>) {
        if let Some(e) = x.alloc(&pools::ESHL) {
            e.set_u8(x.b, 0, 1);
            e.set_u8(x.b, 2, 0x18);
            e.set_u8(x.b, 3, 1);
            let (n, source) = (x.u16(0x56), x.u16(0x5c));
            e.set_u16(x.b, 0x56, n);
            e.set_u16(x.b, 0x5c, source);
            x.set16(0x56, n.wrapping_add(1) & 0xf);
        }
    }

    /// Spawns the group of enemies around the creator.
    fn spawn_enemies<R: Bench>(x: &mut Cx<'_, R>) {
        let mut d0 = x.a5_u8(0x35) as u16;
        if d0 != 0 {
            d0 = d0.wrapping_sub(1) & 3;
        }
        d0 = d0.wrapping_add((x.a5_u16(GROUP_SIZE_SELECTOR) >> 1) << 2);
        let mut count = x.rom8(GROUP_SIZES.wrapping_add(d0 as i16 as i32 as u32)) as u16;
        count = count.wrapping_sub(1);
        let mut label: u16 = 1;
        let rng = x.random();
        let mut table = GROUP_OFFSETS + ((rng as u32 & 7) << 3);
        loop {
            let Some(e) = x.alloc(&pools::ENEMY) else { return };
            e.set_u8(x.b, 0, 1);
            e.set_u8(x.b, 2, 1);
            e.set_u8(x.b, 3, 4);
            let source = x.linked(0x5c);
            let (sx, sy) = (source.u16(x.b, 0x8), source.u16(x.b, 0x10));
            let (dx, dy) = (x.data16(table), x.data16(table + 2));
            table += 4;
            e.set_u16(x.b, 0x8, sx.wrapping_add(dx));
            e.set_u16(x.b, 0x10, sy.wrapping_add(dy));
            e.set_u16(x.b, 0x42, label);
            label = label.wrapping_add(0x40);
            count = count.wrapping_sub(1);
            if count == 0xffff {
                return;
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 25 (0xa16bc): projectile that falls from the sky onto a target (h)
// ---------------------------------------------------------------------------------------------

/// After a delay (`+0x58` frames), falls from the camera height + 0x100 above the
/// creator's position (`+0x5c`), with a random offset for non-zero variants.
/// On impact (Z < 0): sounds and effects, then it sets its shadow (`+0x5a`) to state 6.
mod shot25 {
    use super::*;

    const ANIMS: u32 = 0x16ea18;
    const IMPACT_EFFECT: u32 = 0xeafdc;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => wait_and_drop(x),
            2 => fall(x),
            4 => x.add8(0x4, 2),
            _ => x.free_eshl(),
        }
    }

    fn wait_and_drop<R: Bench>(x: &mut Cx<'_, R>) {
        x.add16(0x58, 0xffff);
        if x.u16(0x58) != 0 {
            return;
        }
        x.add8(0x4, 2);
        x.set8(f::BOX_SET, 7);
        x.set8(f::ATTACK_ID, 0x17);
        let source = x.linked(0x5c);
        let (sx, sy) = (source.u16(x.b, 0x8), source.u16(x.b, 0x10));
        x.set16(0x8, sx);
        x.set16(0x10, sy);
        let camera = if x.a5_u8(0x36) != 0 { x.a5_u16(0x55a) } else { x.a5_u16(0x50a) };
        x.set16(0xc, camera.wrapping_add(0x100).wrapping_sub(sy));
        x.set16(0x18, -0x100i16 as u16);
        x.set16(0x1a, -0x40i16 as u16);
        let variant = x.u8(0x3);
        if variant != 0 {
            let rng = x.random();
            let mut dx = rng & 0x7f;
            if variant & 1 != 0 {
                dx = dx.wrapping_neg();
            }
            x.add16(0x8, dx);
            let rng = x.random();
            let r = rng & 0x3f;
            let dy = if r & 1 != 0 { 0xffff } else { 0 };
            x.add16(0x10, dy);
        }
        x.anim_start(ANIMS, 0);
        if let Some(ombre) = crate::families::small::shot_18(&mut crate::families::effects_creators::leftovers::Pure, x.b, x.o, 0) {
            x.set16(0x5a, low16(ombre));
        }
        x.reinsert();
    }

    fn fall<R: Bench>(x: &mut Cx<'_, R>) {
        x.anim_step();
        x.move_z();
        if (x.u16(0xc) as i16) >= 0 {
            return;
        }
        x.add8(0x4, 2);
        x.set32(0xc, 0);
        crate::families::effects_creators::debris(x.b, x.o, 3);
        crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::B, 8);
        crate::sound::play(x.b, IMPACT_EFFECT, x.o);
        if x.u16(0x5a) != 0 {
            let shadow = x.linked(0x5a);
            shadow.set_u8(x.b, 0x4, 6);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 26 (0xa18dc): projectile thrown in an arc by a creator, with shadow (h)
// ---------------------------------------------------------------------------------------------

/// After a delay, leaves the creator's position (`+0x5c`) at a height from table 0xa1968 (according to
/// the creator's `+0x60`), with speed table 3 (or 2). Falls back (Z < 0), stays 128 frames
/// on the ground then 96 blinking, before disappearing.
mod shot26 {
    use super::*;

    const ANIMS: u32 = 0x16ea90;
    const HEIGHTS: u32 = 0xa1968;
    const SPEED_TABLES: u32 = 0xa1972;
    const LAND_SOUND: u32 = 0xeb1a4;
    const FLY: u8 = 0;
    const GROUND: u8 = 2;
    const BLINK: u8 = 4;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => init(x),
            2 => {
                match x.u8(0x5) {
                    FLY => fly(x),
                    GROUND => ground(x),
                    BLINK => blink_out(x),
                    _ => {}
                }
                x.reinsert();
            }
            4 => x.add8(0x4, 2),
            _ => x.free_eshl(),
        }
    }

    fn init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add16(0x58, 0xffff);
        if x.u16(0x58) != 0 {
            return;
        }
        x.add8(0x4, 2);
        x.set8(f::BOX_SET, 7);
        let source = x.linked(0x5c);
        let (sx, sy) = (source.u16(x.b, 0x8), source.u16(x.b, 0x10));
        x.set16(0x8, sx);
        x.set16(0x10, sy);
        let row = x.u8(0x60) as u32;
        let height = x.rom16(HEIGHTS + 2 * row);
        x.set16(0xc, height);
        let rng = x.random();
        let mut table = x.rom16(SPEED_TABLES + 2 * (rng as u32 & 3)) as u32;
        if x.u8(0x56) != 0 && x.u8(0x3) == 0 {
            table = 2;
        }
        speed_table(x, table);
        x.set16(0x18, 0x300);
        x.set16(0x1a, -0x30i16 as u16);
        x.anim_start(ANIMS, 0);
        x.reinsert();
    }

    fn fly<R: Bench>(x: &mut Cx<'_, R>) {
        x.anim_step();
        x.move_xyz();
        if (x.u16(0xc) as i16) >= 0 {
            return;
        }
        x.add8(0x5, 2);
        x.set32(0xc, 0);
        x.set16(0x58, 0x80);
        x.anim_start(ANIMS, 1);
        crate::sound::play(x.b, LAND_SOUND, x.o);
        if let Some(ombre) = crate::families::small::shot_18(&mut crate::families::effects_creators::leftovers::Pure, x.b, x.o, 1) {
            x.set16(0x5a, low16(ombre));
        }
    }

    fn ground<R: Bench>(x: &mut Cx<'_, R>) {
        x.anim_step();
        x.add16(0x58, 0xffff);
        if x.u16(0x58) != 0 {
            return;
        }
        x.add8(0x5, 2);
        x.set16(0x58, 0x60);
        x.anim_start(ANIMS, 2);
        if x.u16(0x5a) != 0 {
            let shadow = x.linked(0x5a);
            shadow.set_u8(x.b, 0x4, 6);
        }
    }

    fn blink_out<R: Bench>(x: &mut Cx<'_, R>) {
        x.anim_step();
        x.add16(0x58, 0xffff);
        if x.u16(0x58) == 0 {
            x.add8(0x4, 2);
        }
        let hidden = x.u16(0x58) & 1 == 0;
        x.set_bit(0x1, 1, hidden);
    }
}

// ---------------------------------------------------------------------------------------------
// Type 27 (0xa1a64): curtain of sparks from a creator, two variants (h)
// ---------------------------------------------------------------------------------------------

/// Variant 0: epicenter (animation 0, 96 frames) placed on the creator (`+0x5c`); at 80 frames
/// it creates a row of type 27 shots variant 1 on either side (`spawn_row`) and the
/// sound. Variant 1: an element of the row, blinking, which disappears with the epicenter.
mod shot27 {
    use super::*;

    const ANIMS: u32 = 0x16ec24;
    const SOUND_END: u32 = 0xed90a;
    const SOUND_ROW: u32 = 0xed40a;
    const ROW_AT: u16 = 0x50;
    const RISE: u8 = 0;
    const FADE: u8 = 2;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => init(x),
            2 => active(x),
            4 => x.add8(0x4, 2),
            _ => x.free_eshl(),
        }
    }

    fn init<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(0x3) == 0 {
            x.add8(0x4, 2);
            let source = x.linked(0x5c);
            let (sx, sy) = (source.u16(x.b, 0x8), source.u16(x.b, 0x10));
            x.set16(0x8, sx);
            x.set16(0x10, sy);
            x.set16(0xc, 0x20);
            x.add16(0x10, 0xffff);
            x.set16(0x58, 0x60);
        } else {
            x.add16(0x58, 0xffff);
            if x.u16(0x58) != 0 {
                return;
            }
            x.add8(0x4, 2);
        }
        x.set8(f::BOX_SET, 7);
        x.set8(f::ATTACK_ID, 0x16);
        let anim = (x.u8(0x3) != 0) as u32;
        x.anim_start(ANIMS, anim as u16);
        x.reinsert();
    }

    fn active<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(0x3) == 0 {
            x.add16(0x58, 0xffff);
            if x.u16(0x58) == 0 {
                x.add8(0x4, 2);
                x.set8(0x0, 2);
                crate::sound::play(x.b, SOUND_END, x.o);
            }
            x.anim_step();
            if x.u16(0x58) == ROW_AT {
                start_row(x);
                crate::sound::play(x.b, SOUND_ROW, x.o);
            }
            return;
        }
        match x.u8(0x5) {
            RISE => {
                x.anim_step();
                let source = x.linked(0x5c);
                if source.u16(x.b, 0x58) <= 0x30 {
                    x.add8(0x5, 2);
                    x.anim_start(ANIMS, 2);
                }
                flicker(x);
            }
            FADE => {
                if x.anim_step() {
                    x.add8(0x4, 2);
                }
                flicker(x);
            }
            _ => {}
        }
    }

    /// Hidden every other frame according to the frame counter `A5+0x6f`.
    fn flicker<R: Bench>(x: &mut Cx<'_, R>) {
        let hidden = x.a5_u8(0x6f) & 1 == 0;
        x.set_bit(0x1, 1, hidden);
    }

    /// Creates the row of elements (variant 1) on either side, from the epicenter's creator
    /// or the camera edge, up to the screen limits.
    fn start_row<R: Bench>(x: &mut Cx<'_, R>) {
        let px = x.u16(0x8);
        let limit = if x.u8(0x56) == 0 {
            let target = x.linked(f::OWNER);
            target.u16(x.b, 0x8)
        } else {
            camera_x(x).wrapping_add(0xc0)
        };
        let rightwards = px <= limit;
        let start = if rightwards { px.wrapping_add(0x20) } else { px.wrapping_sub(0x20) };
        spawn_row(x, 0x1b, start, rightwards);
    }
}

// ---------------------------------------------------------------------------------------------
// Type 28 (0xa1c96): fragment spinning around a creator (h)
// ---------------------------------------------------------------------------------------------

/// Fragment (4 variants, `+3`) thrown from the creator (`+0x5c`), at a height from table
/// 0xa1d18 according to the creator's `+0x60`, with speed table 0x3f; it advances and disappears
/// off screen (test one frame out of 4).
mod shot28 {
    use super::*;

    const ANIMS: u32 = 0x16ee30;
    const HEIGHTS: u32 = 0xa1d18;
    const ANGLES: u32 = 0xa1d22;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(f::BOX_SET, 7);
                x.set8(f::ATTACK_ID, 0x2e);
                let source = x.linked(0x5c);
                for off in [0x8, 0xc, 0x10] {
                    let v = source.u16(x.b, off);
                    x.set16(off, v);
                }
                let row = (source.u8(x.b, 0x60) & 0xf) as u32;
                let dz = x.rom16(HEIGHTS + 2 * row);
                x.add16(0xc, dz);
                x.add16(0x10, 0xffff);
                let variant = x.u8(0x3);
                let angle = x.rom8(ANGLES + (variant & 3) as u32);
                x.set8(0x2d, angle);
                speed_table(x, 0x3f);
                x.anim_start(ANIMS, (variant as u32) as u16);
                x.reinsert();
            }
            2 => {
                x.move_by_table_xy();
                if x.a5_u8(0x6f).wrapping_add(x.u8(0x3)) & 3 == 0 {
                    x.cull_offscreen();
                }
                x.reinsert();
            }
            4 => x.add8(0x4, 2),
            _ => x.free_eshl(),
        }
    }
}

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
mod shot29 {
    use super::*;

    const ANIMS: u32 = 0x16eec2;
    const ATTACH_SHIFT: u32 = 0xa1fac;
    const SOUND_START: u32 = 0xed40a;
    const SOUND_END: u32 = 0xed90a;
    const SOURCE_ARMED: u32 = 0xaf;
    const WAIT: u8 = 0;
    const HOLD: u8 = 2;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => init(x),
            2 => active(x),
            4 => stop(x),
            _ => x.free_eshl(),
        }
    }

    fn init<R: Bench>(x: &mut Cx<'_, R>) {
        x.set8(f::ATTACK_ID, 6);
        x.set8(f::ATTACK_POWER, 5);
        x.set8(f::BOX_SET, 7);
        match x.u8(0x3) {
            2 => {
                x.add8(0x4, 2);
                follow(x);
                x.set16(0x58, 0x1e);
                x.anim_start(ANIMS, 3);
            }
            0 => {
                x.add8(0x4, 2);
                follow(x);
                x.anim_start(ANIMS, 0);
            }
            _ => {
                x.add16(0x58, 0xffff);
                if x.u16(0x58) != 0 {
                    return;
                }
                x.add8(0x4, 2);
                x.anim_start(ANIMS, 1);
            }
        }
        x.reinsert();
    }

    /// End: the creator is dead or in state 0x20a.
    fn stop<R: Bench>(x: &mut Cx<'_, R>) {
        crate::sound::play(x.b, SOUND_END, x.o);
        x.add8(0x4, 2);
    }

    fn active<R: Bench>(x: &mut Cx<'_, R>) {
        let source = x.linked(0x5c);
        if (source.u8(x.b, 0x4) as i8) >= 4 || source.u16(x.b, 0x4) == 0x20a {
            return stop(x);
        }
        match x.u8(0x3) {
            2 => {
                follow(x);
                x.add16(0x58, 0xffff);
                if (x.u16(0x58) as i16) < 0 {
                    x.add8(0x4, 2);
                }
                x.anim_step();
            }
            0 => centre(x),
            _ => satellite(x, source),
        }
    }

    /// Central element: waits for the creator's signal then triggers the row.
    fn centre<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x5) {
            WAIT => {
                let source = x.linked(0x5c);
                if source.u8(x.b, SOURCE_ARMED) != 0 {
                    crate::sound::play(x.b, SOUND_START, x.o);
                    x.add8(0x5, 2);
                    x.set16(0x58, 0x50);
                    let px = source.u16(x.b, 0x8);
                    let rightwards = source.u8(x.b, f::FLIP) != 0;
                    let start = if rightwards { px.wrapping_add(0x4d) } else { px.wrapping_sub(0x4d) };
                    spawn_row(x, 0x1d, start, rightwards);
                } else {
                    follow(x);
                    x.anim_step();
                    x.reinsert();
                }
            }
            HOLD => {
                x.add16(0x58, 0xffff);
                if x.u16(0x58) == 0 {
                    x.add8(0x4, 2);
                    x.set8(0x5, 0);
                    x.set8(0x0, 2);
                    let source = x.linked(0x5c);
                    source.set_u8(x.b, SOURCE_ARMED, 0);
                }
                follow(x);
                x.anim_step();
                x.reinsert();
            }
            _ => {}
        }
    }

    /// Row element: blinks according to `A5+0x6f`, changes animation when the epicenter goes out.
    fn satellite<R: Bench>(x: &mut Cx<'_, R>, source: Obj) {
        let flicker = |x: &mut Cx<'_, R>| {
            let hidden = x.a5_u8(0x6f) & 1 == 0;
            x.set_bit(0x1, 1, hidden);
        };
        match x.u8(0x5) {
            WAIT => {
                x.anim_step();
                if source.u16(x.b, 0x58) <= 0x30 {
                    x.add8(0x5, 2);
                    x.anim_start(ANIMS, 2);
                }
                flicker(x);
            }
            HOLD => {
                if x.anim_step() {
                    x.add8(0x4, 2);
                }
                flicker(x);
            }
            _ => {}
        }
    }

    /// Sticks to the creator; the offset depends on its attack state (`+7`).
    fn follow<R: Bench>(x: &mut Cx<'_, R>) {
        let source = x.linked(0x5c);
        copy_position16(x, source);
        let flip = source.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        let attack = source.u8(x.b, 0x7);
        if attack == 0x16 || attack == 0x18 {
            let index = source.u8(x.b, 0x33) as u32;
            let mut dx = x.rom16(ATTACH_SHIFT + index);
            if flip != 0 {
                dx = dx.wrapping_neg();
            }
            x.add16(0x8, dx);
            x.set16(0xc, 0x1e);
        } else {
            let mut dx = -0x2di16 as u16;
            if flip != 0 {
                dx = dx.wrapping_neg();
            }
            x.add16(0x8, dx);
            x.set16(0xc, 0x27);
        }
        x.add16(0x10, 0xffff);
    }
}

// ---------------------------------------------------------------------------------------------
// Type 30 (0xa2060): hit delivered by an enemy, with sound (h)
// ---------------------------------------------------------------------------------------------

/// Sticks a hit to the owner (inverted orientation): waits for the owner to arm its attack
/// (`+0xaf`), plays the hit (animation 1, 48 frames), the return (animation 2), then cedes control.
mod shot30 {
    use super::*;

    const ANIMS: u32 = 0x16f174;
    const SOUND_START: u32 = 0xed474;
    const SOUND_END: u32 = 0xed90a;
    const OWNER_ARMED: u32 = 0xaf;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => init(x),
            2 => active(x),
            4 => {
                crate::sound::play(x.b, SOUND_END, x.o);
                x.free_eshl()
            }
            _ => x.free_eshl(),
        }
    }

    fn init<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u16(f::OWNER) == 0 {
            return;
        }
        x.add8(0x4, 2);
        let owner = x.linked(f::OWNER);
        copy_position16(x, owner);
        let flip = owner.u8(x.b, f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
        x.set8(f::ATTACK_ID, 5);
        x.set8(f::ATTACK_POWER, 0xc);
        x.set8(f::BOX_SET, 7);
        x.add16(0xc, 0x27);
        let dx = if flip != 0 { -0x2di16 } else { 0x2d };
        x.add16(0x8, dx as u16);
        x.anim_start(ANIMS, 0);
    }

    fn active<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u16(f::OWNER) == 0 {
            return;
        }
        let owner = x.linked(f::OWNER);
        if (owner.u8(x.b, 0x4) as i8) >= 4 || owner.u16(x.b, 0x4) == 0x20a {
            crate::sound::play(x.b, SOUND_END, x.o);
            return x.free_eshl();
        }
        match x.u8(0x5) {
            0 => {
                if owner.u8(x.b, OWNER_ARMED) != 0 {
                    x.add8(0x5, 2);
                }
                x.anim_step();
                x.reinsert();
            }
            2 => {
                x.add8(0x5, 2);
                crate::sound::play(x.b, SOUND_START, x.o);
                x.set16(0x58, 0x30);
                x.anim_start(ANIMS, 1);
            }
            4 => {
                x.add16(0x58, 0xffff);
                if (x.u16(0x58) as i16) < 0 {
                    x.add8(0x5, 2);
                    x.anim_start(ANIMS, 2);
                } else {
                    x.anim_step();
                    x.reinsert();
                }
            }
            6 => {
                if x.anim_step() {
                    crate::sound::play(x.b, SOUND_END, x.o);
                    x.add8(0x4, 4);
                    x.set8(0x5, 0);
                    let owner = x.linked(f::OWNER);
                    owner.set_u8(x.b, OWNER_ARMED, 0);
                }
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 31 (0xa21b6): homing ball from a creator (h)
// ---------------------------------------------------------------------------------------------

/// Ball that leaves the creator (`+0x5c`, side it faces) with speed table 0x30. It waits
/// 48 frames (`+0x42`), then every 10 frames orients its direction (`+0x2d`) toward it (angle +
/// `+0x58`, sign depending on the height of its target) and explodes on contact (0x20 box around the
/// creator). When the creator goes to state >= 0x406, it flies straight and leaves the screen.
mod shot31 {
    use super::*;

    const ANIMS: u32 = 0x16f504;
    const SOUND_SPAWN: u32 = 0xedaa6;
    const SOUND_HIT: u32 = 0xedb18;
    const SEARCH: u8 = 0;
    const HUNT: u8 = 2;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u16(0x5c) == 0 {
            return kill(x);
        }
        match x.u8(0x4) {
            0 => init(x),
            2 => active(x),
            4 => fly_away(x),
            _ => x.free_eshl(),
        }
    }

    fn init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        x.set8(f::BOX_SET, 7);
        x.set8(f::ATTACK_ID, 7);
        x.set8(f::ATTACK_POWER, 5);
        let source = x.linked(0x5c);
        copy_position16(x, source);
        let flip = source.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        x.add16(0x10, 0xffff);
        x.set8(0x42, 0x30);
        let (dx, angle) = if flip != 0 { (0x4a, 8) } else { (-0x4ai16 as u16, 0x18) };
        x.set8(0x2d, angle);
        x.add16(0x8, dx);
        x.add16(0xc, 0x32);
        x.anim_start(ANIMS, 0);
        speed_table(x, 0x30);
    }

    fn fly_away<R: Bench>(x: &mut Cx<'_, R>) {
        x.move_by_table_xy();
        x.anim_step();
        x.cull_offscreen();
    }

    /// Bounce sound when the animation raises its flag (`+0x33`).
    fn animation_cue<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(0x33) != 0 {
            x.set8(0x33, 0);
            crate::sound::play(x.b, SOUND_SPAWN, x.o);
        }
    }

    fn active<R: Bench>(x: &mut Cx<'_, R>) {
        let source = x.linked(0x5c);
        if (source.u16(x.b, 0x4) as i16) >= 0x406 {
            return fly_away(x);
        }
        match x.u8(0x5) {
            SEARCH => {
                animation_cue(x);
                x.add8(0x42, 0xff);
                if (x.u8(0x42) as i8) < 0 {
                    x.add8(0x5, 2);
                    x.set8(0x58, 0xfc);
                    if source.u16(x.b, f::OWNER) != 0 {
                        let target = link_of(x, source, f::OWNER);
                        let d = x.u16(0x10).wrapping_sub(target.u16(x.b, 0x10));
                        if (d as i16) < 0 {
                            x.set8(0x58, 4);
                        }
                    }
                }
                x.anim_step();
                x.move_by_table_xy();
            }
            HUNT => {
                animation_cue(x);
                x.add8(0x42, 0xff);
                if (x.u8(0x42) as i8) < 0 {
                    x.set8(0x42, 0xa);
                    let to = (source.u16(x.b, 0x8), source.u16(x.b, 0x10));
                    let angle = crate::aids::direction_from(x.b, x.o, to).wrapping_add(x.u8(0x58)) & 0x1f;
                    x.set8(0x2d, angle);
                }
                let touching = |x: &mut Cx<'_, R>, a: u16, b: u16| a.wrapping_sub(b).wrapping_add(8) <= 0x20 && x.a5_u8(GAME_PAUSED) == 0;
                let (sx, sy) = (source.u16(x.b, 0x8), source.u16(x.b, 0x10));
                let (mx, my) = (x.u16(0x8), x.u16(0x10));
                let hit = source.u8(x.b, 0x4) != 4 && touching(x, sx, mx) && touching(x, sy, my);
                if hit {
                    crate::sound::play(x.b, SOUND_HIT, x.o);
                    x.set8(0x4, st::FREE);
                    source.set_u8(x.b, 0xb5, 0);
                } else {
                    x.move_by_table_xy();
                    x.anim_step();
                }
            }
            _ => {}
        }
        x.reinsert();
    }
}

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
mod shot32 {
    use super::*;

    const ANIMS: u32 = 0x16f58c;
    const SOUND_FLYING: u32 = 0xed4de;
    const SOUND_LANDING: u32 = 0xed548;
    const SCREEN_LEFT: u32 = 0x5a6;
    const FLY: u8 = 0;
    const FALL: u8 = 2;
    const END: u8 = 4;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        guarded(x, state)
    }

    fn state<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => init(x),
            2 => match x.u8(0x5) {
                FLY => {
                    if host_dead(x) {
                        return x.free_eshl();
                    }
                    if x.u8(0x58) != 0 {
                        x.add8(0x5, 2);
                        crate::sound::play(x.b, SOUND_FLYING, x.o);
                    }
                    ai_step(x);
                }
                FALL => {
                    if host_dead(x) {
                        return x.free_eshl();
                    }
                    x.move_xyz();
                    if (x.u16(0xc) as i16) > 0 {
                        ai_step(x);
                        return;
                    }
                    x.add8(0x5, 2);
                    x.set16(0xc, 0);
                    x.set8(0x5a, 1);
                    let left = x.a5_u16(SCREEN_LEFT);
                    let x_pos = x.u16(0x8);
                    let inside = (left.wrapping_sub(0x30) as i16) < x_pos as i16 && (left.wrapping_add(0x1b0) as i16) > x_pos as i16;
                    if inside {
                        crate::sound::play(x.b, SOUND_LANDING, x.o);
                    }
                    ai_start(x, ANIMS, 2);
                }
                END => {
                    if ai_step(x) {
                        x.add8(0x4, 2);
                    }
                }
                _ => {}
            },
            _ => x.free_eshl(),
        }
    }

    fn init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        x.set8(f::BOX_SET, 7);
        x.set8(f::ATTACK_POWER, 0x14);
        crate::placement::load_effect_palette(x.b, crate::placement::effect_palette::LIGHT);
        let variant = x.u8(0x3);
        let (target, angle);
        if variant == 0 {
            target = x.linked(f::OWNER);
            angle = crate::aids::direction_to(x.b, x.o, target);
        } else {
            target = x.linked(0x42);
            let to = (target.u16(x.b, 0xbc), target.u16(x.b, 0xbe));
            angle = crate::aids::direction_from(x.b, x.o, to);
        }
        x.set8(0x2d, angle);
        speed_table(x, 0x41);
        // horizontal distance to the target
        let reference = if variant == 0 { target.u32(x.b, 0x8) } else { target.u32(x.b, 0xbc) };
        let mut distance = x.u32(0x8).wrapping_sub(reference);
        if (distance as i32) < 0 {
            distance = distance.wrapping_neg();
        }
        distance &= 0x7fff_0000;
        // number of 9-pixel steps to cover it
        let step = 0x0009_0000u32;
        let mut steps: u32 = 0;
        let mut rest = distance as i32;
        loop {
            rest = rest.wrapping_sub(step as i32);
            if rest <= 0 {
                break;
            }
            steps = (steps + 1) & 0xffff;
        }
        let quotient = crate::aids::divide(x.u16(0xc) as u32, steps).0;
        let slope = (quotient as i32).wrapping_neg() as u32;
        let slope = (slope.rotate_left(16) as i32 >> 8) as u16;
        x.set16(0x18, slope);
        ai_start(x, ANIMS, 0);
    }
}

/// Type 33 (0xa2524): three-stage shot tied to its host (h)
///
/// Same template as type 32 (suspension, dead host = host state 4). Animation 0 then, when
/// the animation raises `+0x58`, sound and animation 1; when it raises `+0x59`, animation 2 until
/// `+0x32` empties (`+0x5a` = 1), then end at the end of the script.
mod shot33 {
    use super::*;

    const ANIMS: u32 = 0x16f8ea;
    const SOUND_FIRE: u32 = 0xed474;
    const SOUND_END: u32 = 0xed90a;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        if suspended(x) {
            return;
        }
        if host_dead(x) {
            crate::sound::play(x.b, SOUND_END, x.o);
            return x.free_eshl();
        }
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(f::BOX_SET, 7);
                x.set8(f::ATTACK_POWER, 0xf);
                ai_start(x, ANIMS, 0);
            }
            2 => match x.u8(0x5) {
                0 => {
                    if x.u8(0x58) == 0 {
                        ai_step(x);
                    } else {
                        x.add8(0x5, 2);
                        crate::sound::play(x.b, SOUND_FIRE, x.o);
                        ai_start(x, ANIMS, 1);
                    }
                }
                2 => {
                    if x.u8(0x59) == 0 {
                        ai_step(x);
                    } else {
                        x.add8(0x5, 2);
                        x.set8(0x6, 0);
                        ai_start(x, ANIMS, 2);
                    }
                }
                4 => match x.u8(0x6) {
                    0 => {
                        ai_step(x);
                        if x.u16(0x32) != 0 {
                            x.add8(0x6, 2);
                            x.set8(0x5a, 1);
                        }
                    }
                    2 => {
                        if ai_step(x) {
                            x.add8(0x4, 2);
                        }
                    }
                    _ => {}
                },
                _ => {}
            },
            _ => {
                crate::sound::play(x.b, SOUND_END, x.o);
                return x.free_eshl();
            }
        }
        if x.u8(0x0) != 0 {
            x.reinsert();
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 34 (0xa2630): staging of a boss attack, three variants (h)
// ---------------------------------------------------------------------------------------------

/// Staging object tied to its creator (`+0x5c`). Variant 0: full-screen veil (camera position,
/// 600 frames) with a SET scenery (type 0 variant 0x47) attached in both directions;
/// variant 1: same idea next to the creator (400 frames, a single scenery, exit animation);
/// variant 2: placed on the creator. On the first pass variant 0 loads four palettes
/// (0x91c2e0...) from the ROM, tinted by 0xf000f000.
mod shot34 {
    use super::*;

    const ANIMS_SCREEN: u32 = 0x1c03a6;
    const ANIMS: u32 = 0x17e03a;
    /// (destination in the palette, source in the ROM), 8 longs each.
    const PALETTES: [(u32, u32); 4] = [(0x91c2e0, 0x2538c0), (0x91c300, 0x2538e0), (0x91c360, 0x253900), (0x91c3e0, 0x253920)];
    const TINT: u32 = 0xf000_f000;
    const SOURCE_BUSY: u32 = 0xa0;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u16(0x5c) == 0 {
            return kill(x);
        }
        match x.u8(0x4) {
            0 => init(x),
            2 => active(x),
            _ => x.free_eshl(),
        }
    }

    fn load_palettes<R: Bench>(x: &mut Cx<'_, R>) {
        for (dst, src) in PALETTES {
            for i in 0..8 {
                let v = x.data32(src + 4 * i);
                wr32(x.b, dst + 4 * i, v);
            }
            for i in 0..8 {
                let v = rd32(x.b, dst + 4 * i) | TINT;
                wr32(x.b, dst + 4 * i, v);
            }
        }
    }

    /// SET scenery (type 0, variant 0x47); `None` if the pool is full.
    fn spawn_scenery<R: Bench>(x: &mut Cx<'_, R>) -> Option<Obj> {
        let s = x.alloc(&pools::SET)?;
        s.set_u8(x.b, 0, 1);
        s.set_u8(x.b, 2, 0);
        s.set_u8(x.b, 3, 0x47);
        Some(s)
    }

    /// Places itself on the creator (position, orientation, one pixel closer).
    fn place_on_source<R: Bench>(x: &mut Cx<'_, R>) -> Obj {
        let source = x.linked(0x5c);
        copy_position16(x, source);
        let flip = source.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        x.add16(0x10, 0xffff);
        source
    }

    fn init<R: Bench>(x: &mut Cx<'_, R>) {
        let variant = x.u8(0x3);
        if variant == 0 && x.u8(0x5) == 0 {
            x.add8(0x5, 2);
            return load_palettes(x);
        }
        if x.a5_u8(GAME_PAUSED) != 0 {
            return;
        }
        x.add8(0x4, 2);
        x.set8(0x5, 0);
        if variant == 2 {
            place_on_source(x);
            x.add16(0x8, 0x9c);
            x.add16(0x10, -0x62i16 as u16);
            x.anim_start(ANIMS, 8);
            return;
        }
        if let Some(s) = spawn_scenery(x) {
            s.set_u8(x.b, 0x2f, 0);
            x.set16(f::OWNER, low16(s));
        }
        let source = place_on_source(x);
        if variant == 0 {
            if let Some(s) = spawn_scenery(x) {
                x.set8(0x2f, 0);
                s.set_u16(x.b, f::OWNER, low16(x.o));
            }
            let camera_x = x.a5_u16(0x506);
            x.set16(0x8, camera_x);
            let camera_y = x.a5_u16(0x50a);
            x.set16(0xc, camera_y.wrapping_add(0xda));
            x.set16(0x10, 0);
            x.set16(0x42, 0x258);
            x.anim_start(ANIMS_SCREEN, 0xf0);
        } else {
            let dx = if source.u8(x.b, f::FLIP) != 0 { 8 } else { -8i16 as u16 };
            x.add16(0x8, dx);
            x.add16(0xc, 0x20);
            x.set16(0x42, 0x190);
            x.anim_start(ANIMS, 2);
        }
    }

    fn active<R: Bench>(x: &mut Cx<'_, R>) {
        if x.a5_u8(GAME_PAUSED) != 0 {
            return;
        }
        let source = x.linked(0x5c);
        if (source.u8(x.b, 0x4) as i8) >= 4 {
            return x.free_eshl();
        }
        match x.u8(0x3) {
            0 => {
                x.add16(0x42, 0xffff);
                if (x.u16(0x42) as i16) < 0 {
                    let scenery = x.linked(f::OWNER);
                    scenery.set_u8(x.b, 0x4, 4);
                    kill(x);
                    source.set_u8(x.b, SOURCE_BUSY, 0);
                }
                x.anim_step();
            }
            1 => match x.u8(0x5) {
                0 => {
                    x.add16(0x42, 0xffff);
                    if (x.u16(0x42) as i16) < 0 {
                        x.add8(0x5, 2);
                        source.set_u8(x.b, SOURCE_BUSY, 0);
                        x.anim_start(ANIMS, 7);
                    } else {
                        x.anim_step();
                    }
                }
                2 => {
                    if x.anim_step() {
                        let scenery = x.linked(f::OWNER);
                        scenery.set_u8(x.b, 0x4, 4);
                        kill(x);
                    }
                }
                _ => {}
            },
            _ => {
                x.anim_step();
            }
        }
        x.reinsert();
    }
}

// ---------------------------------------------------------------------------------------------
// Type 35 (0xa28d2): falling spark with shadow, 16 variants (h)
// ---------------------------------------------------------------------------------------------

/// Decorative spark: speed table 0xb with direction `+0x2d` according to the variant (0, 2, ... 30),
/// flies 90 frames (`+0x58`), then animates its end. Its shadow is drawn by 0x33fe4; it blinks
/// according to the parity of `A5+0x6e` and of the variant.
mod shot35 {
    use super::*;

    const ANIMS: u32 = 0x16fafa;
    const DIRECTIONS: u32 = 0xa2934;
    const FLY: u8 = 0;
    const FADE: u8 = 2;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        guarded(x, state)
    }

    fn state<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(f::BOX_SET, 7);
                x.set8(f::ATTACK_POWER, 0xc);
                x.set16(0x58, 0x5a);
                let variant = x.u8(0x3) as u32;
                let direction = x.rom8(DIRECTIONS + variant);
                x.set8(0x2d, direction);
                speed_table(x, 0xb);
                ai_start(x, ANIMS, 0);
            }
            2 => {
                match x.u8(0x5) {
                    FLY => {
                        x.add16(0x58, 0xffff);
                        if (x.u16(0x58) as i16) <= 0 {
                            x.add8(0x5, 2);
                            ai_start(x, ANIMS, 1);
                        } else {
                            ai_step(x);
                            if (x.u16(0x58) as i16) > 0x38 {
                                x.move_by_table_xy();
                            }
                        }
                    }
                    FADE => {
                        if ai_step(x) {
                            x.add8(0x4, 2);
                        } else {
                            x.move_by_table_xy();
                        }
                    }
                    _ => {}
                }
                blink_by_parity(x);
                x.push_terrain();
            }
            _ => x.free_eshl(),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 36 (0xa29cc): debris thrown into the air, falls back onto the scene (h)
// ---------------------------------------------------------------------------------------------

/// Appears at a random position on the screen (tables 0xa2b0a / 0xa2b2a) at height 0x100 - Y,
/// with a randomly drawn vertical speed; falls back (Z <= 0), plays an impact sound then animates
/// its end.
mod shot36 {
    use super::*;

    const ANIMS: u32 = 0x16fbf4;
    const SPEEDS: u32 = 0xa2aa4;
    const X_OFFSETS: u32 = 0xa2b0a;
    const Y_OFFSETS: u32 = 0xa2b2a;
    const SOUND_LAND: u32 = 0xed548;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        guarded(x, state)
    }

    fn state<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(f::BOX_SET, 7);
                x.set8(f::ATTACK_POWER, 0x1e);
                launch(x);
                place(x);
                ai_start(x, ANIMS, 0);
            }
            2 => match x.u8(0x5) {
                0 => {
                    x.move_xyz();
                    if (x.u16(0xc) as i16) > 0 {
                        ai_step(x);
                        return;
                    }
                    x.add8(0x5, 2);
                    x.set16(0xc, 0);
                    crate::sound::play(x.b, SOUND_LAND, x.o);
                    crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::B, 0x20);
                    ai_start(x, ANIMS, 1);
                }
                2 => {
                    if ai_step(x) {
                        x.add8(0x4, 2);
                    }
                }
                _ => {}
            },
            _ => x.free_eshl(),
        }
    }

    /// Direction according to the orientation, speed table 0x17 and randomly drawn vertical speed.
    fn launch<R: Bench>(x: &mut Cx<'_, R>) {
        let direction = if x.u8(f::FLIP) != 0 { 0x18 } else { 8 };
        x.set8(0x2d, direction);
        speed_table(x, 0x17);
        let rng = x.random();
        let r = rng as u32 & 0xf;
        let vz = x.rom16(SPEEDS + 2 * r);
        x.set16(0x18, vz);
    }

    /// Random position around the camera (left screen: offset of -0xc0).
    fn place<R: Bench>(x: &mut Cx<'_, R>) {
        let rng = x.random();
        let r = rng as u32 & 0xf;
        let mut dx = x.rom16(X_OFFSETS + 2 * r);
        let rng = x.random();
        let r = rng as u32 & 0xf;
        let dy = x.rom16(Y_OFFSETS + 2 * r);
        if x.u8(f::FLIP) == 0 {
            dx = dx.wrapping_sub(0xc0);
        }
        let z = 0x100u16.wrapping_sub(dy);
        let px = dx.wrapping_add(x.a5_u16(0x5a6));
        let py = dy.wrapping_add(x.a5_u16(0x5aa));
        x.set16(0x8, px);
        x.set16(0x10, py);
        x.set16(0xc, z);
    }
}

// ---------------------------------------------------------------------------------------------
// Type 37 (0xa2b62): meteorite thrown by a creator, falls back and explodes (h)
// ---------------------------------------------------------------------------------------------

/// Falls from the sky at a random position on the screen (tables 0xa2ce0 / 0xa2d00), random vertical speed
/// (0xa2c7e) and random start animation (0xa2c9e). On the ground (Z <= 0) it triggers
/// effect 0xb10c8 / 0xb4bcc, blinks 25 frames and disappears; it disappears with the creator.
mod shot37 {
    use super::*;

    const ANIMS: u32 = 0x16fe3e;
    const SPEEDS: u32 = 0xa2c7e;
    const START_ANIMS: u32 = 0xa2c9e;
    const X_OFFSETS: u32 = 0xa2ce0;
    const Y_OFFSETS: u32 = 0xa2d00;
    const FALL: u8 = 0;
    const BLINK: u8 = 2;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u16(0x5c) == 0 {
            return kill(x);
        }
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(f::BOX_SET, 7);
                x.set8(f::ATTACK_POWER, 5);
                x.set8(f::ATTACK_ID, 0x17);
                x.set8(0x50, 0x14);
                launch(x);
                place(x);
            }
            2 => active(x),
            _ => x.free_eshl(),
        }
    }

    fn launch<R: Bench>(x: &mut Cx<'_, R>) {
        let rng = x.random();
        let r = rng as u32 & 0xf;
        let vz = x.rom16(SPEEDS + 2 * r);
        x.set16(0x18, vz);
        let rng = x.random();
        let r = rng as u32 & 7;
        let anim = x.rom8(START_ANIMS + r) as u32;
        x.anim_start(ANIMS, anim as u16);
    }

    fn place<R: Bench>(x: &mut Cx<'_, R>) {
        let rng = x.random();
        let r = rng as u32 & 0xf;
        let dx = x.rom16(X_OFFSETS + 2 * r);
        let rng = x.random();
        let r = rng as u32 & 0xf;
        let dy = x.rom16(Y_OFFSETS + 2 * r);
        let px = dx.wrapping_add(x.a5_u16(0x506));
        let py = dy.wrapping_add(x.a5_u16(0x50a));
        x.set16(0x8, px);
        x.set16(0x10, py);
        x.set16(0xc, 0x100);
    }

    fn active<R: Bench>(x: &mut Cx<'_, R>) {
        let source = x.linked(0x5c);
        if (source.u8(x.b, 0x4) as i8) >= 4 {
            return x.free_eshl();
        }
        match x.u8(0x5) {
            FALL => {
                x.move_z();
                if (x.u16(0xc) as i16) > 0 {
                    x.anim_step();
                } else {
                    x.add8(0x5, 2);
                    x.set16(0xc, 0);
                    x.set8(0x42, 0x19);
                    *x.c.d2() &= 0xffff_0000;
                    *x.c.d3() &= 0xffff_0000;
                    let choc = crate::effects_aids::shock(x.b, x.o, 0, 0x201, 0, 0);
                    // Original glitch: the debris variant is the D1.b that the shock (0xb10c8) leaves: the high byte
                    // of its setting, or the low byte when the shock is not created (scene 41, pool full).
                    let variant = if choc.is_some() { 2 } else { 1 };
                    for _ in 0..2 {
                        *x.c.d2() = 1;
                        crate::families::effects_creators::debris_rain(x.b, x.o, variant, 1);
                    }
                }
            }
            BLINK => {
                x.add8(0x42, 0xff);
                if (x.u8(0x42) as i8) < 0 {
                    x.add8(0x4, 2);
                    x.set_bit(0x1, 1, true);
                    x.set8(0x0, 2);
                } else {
                    let hidden = x.u8(0x42) & 3 != 0;
                    x.set_bit(0x1, 1, hidden);
                }
            }
            _ => {}
        }
        x.reinsert();
    }
}

// ---------------------------------------------------------------------------------------------
// Type 38 (0xa2d48): projectile oriented toward its target (16 sprites), thrown by an effect (h)
// ---------------------------------------------------------------------------------------------

/// Created with its creator's pointer in `+0x46` (negative word = RAM address). Takes its position
/// (+0x30 in altitude), speeds from row `+0x2f` of table 0xa2ed4 (or 0xa2f32 for non-zero
/// variants), and its sprite from the angle toward the creator's target, bounded to 0x14..0x16
/// (mirrored if flipped). Then it flies (speed table + gravity) down to the ground.
mod shot38 {
    use super::*;

    const ANIMS: u32 = 0x174776;
    const SPEED_ROWS: u32 = 0xa2ed4;
    const SPEED_ROWS_VARIANT: u32 = 0xa2f34;
    const SPRITES: u32 = 0xa2e10;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        if suspended(x) {
            return;
        }
        state(x)
    }

    /// Mirror of an angle (0x20 - angle) for flipped objects.
    fn mirror(a: u16) -> u16 {
        a.wrapping_sub(0x10).wrapping_neg().wrapping_add(0x10)
    }

    fn state<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => init(x),
            2 => {
                fall(x);
                x.keep_on_screen(0x10);
                x.push_terrain();
                x.reinsert();
            }
            4 => x.add8(0x4, 2),
            _ => x.free_eshl(),
        }
    }

    /// Movement while Z > 0; on the ground, goes to the next state.
    fn fall<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(0x29) == 0 {
            x.move_by_table_xz();
            x.move_y();
            if (x.u16(0xc) as i16) >= 0 {
                return;
            }
            x.set16(0xc, 0);
        }
        x.add8(0x4, 2);
        x.set8(0x5, 0);
    }

    fn init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        let creator = x.u16(0x46);
        if (creator as i16) >= 0 {
            return x.set8(0x4, st::FREE);
        }
        let host = Obj(creator as i16 as i32 as u32 & 0xff_ffff);
        copy_position16(x, host);
        let flip = host.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        x.add16(0xc, 0x30);
        speeds(x);
        let mut angle = host.u16(x.b, 0x50);
        if (angle as i16) < 0 {
            let target = Obj(angle as i16 as i32 as u32 & 0xff_ffff);
            let to = (target.u16(x.b, 0x8), target.u16(x.b, 0xc).wrapping_add(0x30));
            let de = (x.u16(0x8), x.u16(0xc));
            angle = crate::aids::direction(x.b, de, to) as u16;
        }
        let flipped = flip != 0;
        let mapped = |a: u16| if flipped { mirror(a) } else { a };
        let seen = mapped(angle) as i16;
        let chosen = if seen < 0x14 {
            mapped(0x14)
        } else if seen > 0x16 {
            mapped(0x16)
        } else {
            angle
        };
        x.set8(0x2d, chosen as u8);
        let sprite = x.rom8(SPRITES + ((mapped(chosen) >> 1) & 0xf) as u32) as u32;
        ai_start(x, ANIMS, sprite);
        x.set8(f::BOX_SET, 7);
        x.set8(f::ATTACK_POWER, 8);
    }

    /// Speed table and initial speeds of row `+0x2f`.
    fn speeds<R: Bench>(x: &mut Cx<'_, R>) {
        let base = if x.u8(0x3) != 0 { SPEED_ROWS_VARIANT } else { SPEED_ROWS };
        let row = base + ((x.u8(0x2f) as u32) << 4);
        let table = x.data32(row);
        x.set32(0x4c, table);
        let mut vx = x.data16(row + 4);
        let mut ax = x.data16(row + 6);
        if x.u8(f::FLIP) == 0 {
            vx = vx.wrapping_neg();
            ax = ax.wrapping_neg();
        }
        x.set16(0x14, vx);
        x.set16(0x16, ax);
        for i in 0..4 {
            let v = x.data16(row + 8 + 2 * i);
            x.set16(0x18 + 2 * i, v);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 39 (0xa2fb8): fireball thrown into the air by a creator, 8 variants (h)
// ---------------------------------------------------------------------------------------------

/// Leaves the creator (`+0x5c`) upward (VZ = -5, speed table 0x13), falls back spinning
/// (`+0x2d` varies), ignites on the ground (end animation) then signals the creator (`+0xae`).
/// Disappears with the creator (state 0x406).
mod shot39 {
    use super::*;

    const ANIMS: u32 = 0x16ff22;
    const DIRECTIONS: u32 = 0xa3148;
    const START_ANIMS: u32 = 0xa3158;
    const FRAMES: u32 = 0xa31ba;
    const SOUND_IMPACT: u32 = 0xecb98;
    const FLY: u8 = 0;
    const SETTLE: u8 = 2;
    const BURN: u8 = 4;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u16(0x5c) == 0 {
            return kill(x);
        }
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(f::BOX_SET, 7);
                x.set8(f::ATTACK_POWER, 5);
                x.set8(f::ATTACK_ID, 0x1d);
                x.set8(0x46, 4);
                launch(x);
                speed_table(x, 0x13);
            }
            2 => active(x),
            _ => x.free_eshl(),
        }
    }

    fn launch<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u16(0x5c) == 0 {
            return;
        }
        let source = x.linked(0x5c);
        copy_position32(x, source);
        let flip = source.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        x.add16(0x10, 0xffff);
        x.add16(0xc, 0x100);
        x.set16(0x18, -0x500i16 as u16);
        x.set16(0x1a, -0x38i16 as u16);
        let variant = x.u8(0x3) as u32;
        let direction = x.rom8(DIRECTIONS + variant);
        x.set8(0x2d, direction);
        let anim = x.rom8(START_ANIMS + variant) as u32;
        x.anim_start(ANIMS, anim as u16);
    }

    /// Cycles the 8 flame frames every 4 frames.
    fn spin<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x46, 0xff);
        if (x.u8(0x46) as i8) >= 0 {
            return;
        }
        x.set8(0x46, 4);
        x.set8(0x2a, 1);
        let n = x.u8(0x47).wrapping_add(1) & 7;
        x.set8(0x47, n);
        let frame = x.rom8(FRAMES + n as u32);
        x.set8(0x2b, frame);
    }

    fn active<R: Bench>(x: &mut Cx<'_, R>) {
        spin(x);
        let source = x.linked(0x5c);
        if source.u16(x.b, 0x4) == 0x406 {
            return x.free_eshl();
        }
        match x.u8(0x5) {
            FLY => {
                x.move_by_table_xy();
                x.move_z();
                if (x.u16(0xc) as i16) <= 0 {
                    crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::B, 0x30);
                    crate::sound::play(x.b, SOUND_IMPACT, x.o);
                    x.add8(0x5, 2);
                    x.set16(0xc, 0);
                    x.set8(0x42, 0x30);
                }
                x.anim_step();
            }
            SETTLE => {
                x.add8(0x42, 0xff);
                if (x.u8(0x42) as i8) < 0 {
                    x.add8(0x5, 2);
                    x.set8(0x42, 0x1e);
                    let d = x.u8(0x2d).wrapping_sub(6) & 0x1f;
                    x.set8(0x2d, d);
                    x.set8(0x43, 3);
                }
                x.anim_step();
            }
            BURN => {
                x.anim_step();
                if (x.u8(0x42) as i8) < 0x14 {
                    x.set8(0x3a, 0);
                }
                x.add8(0x42, 0xff);
                if (x.u8(0x42) as i8) < 0 {
                    x.add8(0x4, 2);
                    source.set_u8(x.b, 0xae, 1);
                } else {
                    x.add8(0x43, 0xff);
                    if (x.u8(0x43) as i8) < 0 {
                        x.set8(0x43, 3);
                        let d = x.u8(0x2d).wrapping_add(1) & 0x1f;
                        x.set8(0x2d, d);
                    }
                    x.move_by_table_xy();
                }
            }
            _ => {}
        }
        blink_by_parity(x);
        x.reinsert();
    }
}
handlers! {
    0x9fe0a "eshl_15" #[without] shot15::run,
    0xa03e0 "eshl_16" #[without] shot16::run,
    0xa05ce "eshl_17" #[without] shot17::run,
    0xa0a06 "eshl_18" #[without] shot18::run,
    0xa0b20 "eshl_19" #[without] shot19::run,
    0xa0c5a "eshl_20" #[without] shot20::run,
    0xa0f96 "eshl_21" #[without] shot21::run,
    0xa1180 "eshl_22" #[without] shot22::run,
    0xa1292 "eshl_23" #[without] shot23::run,
    0xa137e "eshl_24" #[without] shot24::run,
    0xa16bc "eshl_25" #[without] shot25::run,
    0xa18dc "eshl_26" #[without] shot26::run,
    0xa1a64 "eshl_27" #[without] shot27::run,
    0xa1c96 "eshl_28" #[without] shot28::run,
    0xa1d70 "eshl_29" #[without] shot29::run,
    0xa2060 "eshl_30" #[without] shot30::run,
    0xa21b6 "eshl_31" #[without] shot31::run,
    0xa239a "eshl_32" #[without] shot32::run,
    0xa2524 "eshl_33" #[without] shot33::run,
    0xa2630 "eshl_34" #[without] shot34::run,
    0xa28d2 "eshl_35" #[without] shot35::run,
    0xa29cc "eshl_36" #[without] shot36::run,
    0xa2b62 "eshl_37" #[without] shot37::run,
    0xa2d48 "eshl_38" #[without] shot38::run,
    0xa2fb8 "eshl_39" #[without] shot39::run,
}
