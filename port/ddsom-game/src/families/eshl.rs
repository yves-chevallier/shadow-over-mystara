//! Enemy shots and attacks (ESHL class, pool of 38 objects of 0x60 bytes, table 0x39b96).
//!
//! An ESHL object is created by an enemy, a boss or an effect: `+0x50` designates its owner (RAM address on 16 bits),
//! `+0x03` is the variant chosen by the creator, `+0x2c` the orientation inherited from the owner. The collision boxes
//! (set `+0x2e`, power `+0x44/+0x45`) are read by `combat`. Each type is a state machine: `+4` main state, `+5` / `+6`
//! substates (even bytes, indices of a jump table of the original).

use crate::families::cx::Bench;
use super::cx::{handlers, Cx};
use super::shell::*;
use crate::objects::Obj;

/// Sounds (wrappers of the 0xe0836-0xee080 area).
mod sfx {
    pub const E7376: u32 = 0xe7376;
    pub const E73E8: u32 = 0xe73e8;
    pub const EBE86: u32 = 0xebe86;
    pub const E46DE: u32 = 0xe46de;
    pub const E47BC: u32 = 0xe47bc;
    pub const EBF6A: u32 = 0xebf6a;
    pub const ECE2C: u32 = 0xece2c;
    pub const EBCBE: u32 = 0xebcbe;
    pub const EBD30: u32 = 0xebd30;
}

use crate::objects::pools;
use crate::objects::Current;

// ---------------------------------------------------------------------------------------------
// Type 0 (0x9dbfc): horizontal projectile, two variants (+3)
// ---------------------------------------------------------------------------------------------

/// Projectile that leaves from the owner's side at 5 pixels per frame. It stops when it hits something (flags `+0x57`)
/// or certain terrains, then falls back blinking before disappearing.
mod shot0 {
    use super::*;

    pub const ANIMS: u32 = 0x160350;
    /// Substates of the fall (`+6`).
    const FALL_START: u8 = 0;
    const FALL_DROP: u8 = 2;
    const FALL_BLINK: u8 = 4;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => {
                if !spawn(x) {
                    return;
                }
                fly(x);
            }
            2 => fly(x),
            4 => fall(x),
            _ => x.free_eshl(),
        }
    }

    /// Creation: places the projectile in front of the owner. False if the owner has disappeared.
    fn spawn<R: Bench>(x: &mut Cx<'_, R>) -> bool {
        x.add8(0x4, 2);
        x.set8(f::BOX_SET, 7);
        if x.u16(f::OWNER) == 0 {
            return false;
        }
        let owner = x.linked(f::OWNER);
        place_in_front(x, owner, 0x40, 0x38);
        x.set32(0x18, 0);
        x.set8(0x2d, 0x18);
        x.set16(0x14, -0x500i16 as u16);
        if x.u8(f::FLIP) != 0 {
            x.set8(0x2d, 8);
            x.set16(0x14, 0x500);
        }
        let (anim, id, power) = if x.u8(0x3) == 0 { (2, 0x18, 3) } else { (0xf, 0x19, 5) };
        x.set8(f::ATTACK_POWER, power);
        x.set8(f::ATTACK_ID, id);
        x.anim_start(ANIMS, anim);
        crate::sound::play(x.b, sfx::EBE86, x.o);
        true
    }

    /// Flight: destroyed on contact, stops on terrains 5 to 8 without a wall (bit 6), otherwise advances.
    fn fly<R: Bench>(x: &mut Cx<'_, R>) {
        let hit = x.u8(f::HIT_FLAGS);
        if hit & 0x08 != 0 {
            crate::sound::play(x.b, sfx::E7376, x.o);
            x.free_eshl();
            return;
        }
        if hit & 0x20 != 0 {
            x.add8(0x4, 2);
            return fall(x);
        }
        let t = crate::video::terrain_of(x.b, x.o);
        let terrain = t.number as u32;
        let ground = (terrain & 0x3f).wrapping_sub(5) as u16 <= 3;
        if ground && terrain & 0x40 == 0 {
            return fall(x);
        }
        x.move_xz();
        x.anim_step();
        x.cull_offscreen();
        x.reinsert();
    }

    /// Fall after the impact: falls, then blinks for 20 frames before disappearing.
    fn fall<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x6) {
            FALL_START => {
                x.add8(0x6, 2);
                crate::sound::play(x.b, sfx::E73E8, x.o);
                x.set16(0x18, -0xc0i16 as u16);
                x.set16(0x1a, -0x10i16 as u16);
                x.set16(0x14, 0);
                let anim = if x.u8(0x3) != 0 { 0x10 } else { 3 };
                x.anim_start(ANIMS, anim);
            }
            FALL_DROP => {
                x.move_xz();
                x.anim_step();
                if (x.u16(0xc) as i16) < 0 {
                    x.set_bit(0x1, 1, true);
                    x.set16(0xc, 0);
                    x.add8(0x6, 2);
                    x.set8(f::TIMER, 0x14);
                }
            }
            FALL_BLINK => {
                let t = x.u8(f::TIMER).wrapping_sub(1);
                x.set8(f::TIMER, t);
                if (t as i8) < 0 {
                    x.set8(0x4, st::FREE);
                }
                blink(x);
                x.anim_step();
            }
            _ => {}
        }
    }
}

/// Shot 1 (0x9de10): ball thrown in a lob. Two modes according to `+0x2f`: 0, the owner throws it in front of itself;
/// otherwise it springs out with a speed drawn from an 8-row table (variant `+3`). It falls back, bounces on the ground
/// (Z < 0: Z reset to 0), waits 80 frames in an animation then blinks for 80 frames before disappearing.
mod shot1 {
    use super::*;

    const ANIMS: u32 = 0x16c946;
    /// Speed table (16 bytes per variant: VX AX VZ AZ VY AY).
    const SPEEDS: u32 = 0x9dfe2;
    const LOB: u8 = 0;
    const LANDED: u8 = 2;
    const BLINK: u8 = 4;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => {
                x.set8(f::BOX_SET, 7);
                x.set8(f::ATTACK_ID, 8);
                x.set8(f::ATTACK_POWER, 5);
                x.add8(0x4, 2);
                if x.u8(0x2f) != 0 {
                    burst(x);
                } else {
                    lob(x);
                }
                x.anim_start(ANIMS, 0);
                x.reinsert();
                body(x);
            }
            2 => body(x),
            _ => x.free_eshl(),
        }
    }

    /// Thrown by the owner: in front of it, horizontal speed 2.5 pixels per frame.
    fn lob<R: Bench>(x: &mut Cx<'_, R>) {
        let owner = x.linked(f::OWNER);
        place_in_front(x, owner, 0x40, 0x20);
        let vx = if x.u8(f::FLIP) != 0 { 0x280 } else { -0x280i16 as u16 };
        x.set16(0x14, vx);
        x.set16(0x16, 0);
        x.set16(0x18, 0x300);
        x.set16(0x1a, -0x38i16 as u16);
        x.set16(0x1c, 0);
        x.set16(0x1e, 0);
    }

    /// Spring-out: position of the owner, speeds of row `+3` of the table.
    fn burst<R: Bench>(x: &mut Cx<'_, R>) {
        let owner = x.linked(f::OWNER);
        copy_position(x, owner);
        x.add16(0x10, 0xffff);
        x.add16(0xc, 0x32);
        let row = SPEEDS + 16 * x.u8(0x3) as u32;
        for i in 0..6 {
            let v = x.rom16(row + 2 * i);
            x.set16(0x14 + 2 * i, v);
        }
    }

    fn body<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x5) {
            LOB => {
                if !x.move_xyz().gt() {
                    x.add8(0x5, 2);
                    x.set16(f::TIMER, 0x50);
                    x.set32(0xc, 0);
                    crate::sound::play(x.b, sfx::EBCBE, x.o);
                    x.anim_start(ANIMS, 1);
                }
            }
            LANDED => {
                if x.u8(0x33) != 0 {
                    x.set8(0x33, 0);
                    crate::sound::play(x.b, sfx::EBD30, x.o);
                }
                let t = x.u16(f::TIMER).wrapping_sub(1);
                x.set16(f::TIMER, t);
                if t != 0 {
                    x.anim_step();
                } else {
                    x.add8(0x5, 2);
                    x.set16(f::TIMER, 0x50);
                    x.anim_start(ANIMS, 2);
                }
            }
            BLINK => {
                x.anim_step();
                let t = x.u16(f::TIMER).wrapping_sub(1);
                x.set16(f::TIMER, t);
                if t == 0 {
                    x.add8(0x4, 2);
                }
                blink(x);
            }
            _ => {}
        }
        finish(x);
    }

    /// End of the handler when the state is unchanged: sorted list then removal off the screen.
    fn finish<R: Bench>(x: &mut Cx<'_, R>) {
        x.reinsert();
        x.cull_offscreen();
    }
}

/// Shot 2 (0x9e0aa): horizontal projectile without its own animation (the sprite is that of the owner's variant).
/// Destroyed on contact (sound), or on a blocking terrain.
mod shot2 {
    use super::*;

    const ANIMS: u32 = 0x16caa6;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(f::BOX_SET, 7);
                x.set8(f::ATTACK_POWER, 3);
                if x.u16(f::OWNER) == 0 {
                    x.set32(0x4, 0x0600_0000);
                    return;
                }
                let owner = x.linked(f::OWNER);
                place_in_front(x, owner, 0x40, 0x20);
                let (anim, id) = if owner.u8(x.b, 0x2) != 0 { (1, 0xa) } else { (0, 9) };
                x.set8(f::ATTACK_ID, id);
                x.anim_start(ANIMS, anim);
                crate::sound::play(x.b, sfx::EBF6A, x.o);
                fly(x);
            }
            2 => fly(x),
            _ => x.free_eshl(),
        }
    }

    fn fly<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(f::HIT_FLAGS) & 0x28 != 0 {
            crate::sound::play(x.b, sfx::E7376, x.o);
            return x.free_eshl();
        }
        if blocked_by_terrain(x) {
            return x.free_eshl();
        }
        x.move_xz();
        x.cull_offscreen();
        x.reinsert();
    }
}

/// Shot 3 (0x9e1b6): projectile attached to an owner (the creator is alive as long as its state `+4` is below 4). Four
/// variants launched in a fan: offset (dx, dz) and speeds (vx, vy) read from an 8-byte table per variant. Disappears
/// with the end of its animation or when the owner is no longer in normal play.
mod shot3 {
    use super::*;

    const ANIMS: u32 = 0x16cb10;
    const FAN: u32 = 0x9e280;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u16(f::OWNER) == 0 {
            return;
        }
        let owner = x.linked(f::OWNER);
        if owner.u8(x.b, 0x4) as i8 >= 4 {
            return x.free_eshl();
        }
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(f::BOX_SET, 7);
                x.set8(f::ATTACK_POWER, 0);
                x.set8(f::ATTACK_ID, 0x1b);
                place(x, owner);
                x.anim_start(ANIMS, 0);
                x.reinsert();
            }
            2 => {
                x.move_xy();
                if x.anim_step() {
                    x.add8(0x4, 2);
                }
            }
            _ => x.free_eshl(),
        }
    }

    fn place<R: Bench>(x: &mut Cx<'_, R>, owner: Obj) {
        copy_position(x, owner);
        let flip = owner.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        let row = FAN + 8 * x.u8(0x3) as u32;
        let (dx, dz) = (x.rom16(row), x.rom16(row + 2));
        x.add16(0x8, dx);
        x.add16(0xc, dz);
        let (vx, vy) = (x.rom16(row + 4), x.rom16(row + 6));
        x.set16(0x14, vx);
        x.set16(0x1c, vy);
        for off in [0x16, 0x1a, 0x18, 0x1e] {
            x.set16(off, 0);
        }
    }
}

/// Shot 4 (0x9e2dc): bomb thrown by an enemy (no owner: position and direction set by the creator). It falls back
/// according to a speed table, bounces once on the ground with the speeds set aside in `+0x42` / `+0x50`, then explodes
/// at the end of the animation.
mod shot4 {
    use super::*;

    const ANIMS: u32 = 0x16cb74;
    const FALLING: u8 = 0;
    const BOUNCING: u8 = 2;
    const EXPLODING: u8 = 4;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => init(x),
            2 => {
                match x.u8(0x5) {
                    FALLING => fall(x),
                    BOUNCING => bounce(x),
                    EXPLODING => explode(x),
                    _ => {}
                }
                // the original looks here for the terrain under the shot (0x24b0) without reading the result; the
                // registers it leaves are reread by no one (`exit` probe): the search, without effect, is omitted
                x.reinsert();
            }
            _ => x.free_eshl(),
        }
    }

    fn init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        for off in [0x18, 0x1c, 0x1e, 0x16] {
            x.set16(off, 0);
        }
        x.set16(0x1a, -0x38i16 as u16);
        x.set8(f::BOX_SET, 7);
        x.set8(f::ATTACK_POWER, 3);
        x.set8(f::ATTACK_ID, 9);
        x.set8(0x2d, 8);
        let (mut dx, mut bounce_vx, mut bounce_ax) = (0x38i16, -0x250i16, -0x500i16);
        if x.u8(f::FLIP) == 0 {
            x.set8(0x2d, 0x18);
            (dx, bounce_vx, bounce_ax) = (-dx, -bounce_vx, -bounce_ax);
        }
        x.add16(0x8, dx as u16);
        x.set16(0x42, bounce_vx as u16);
        x.set16(f::OWNER, bounce_ax as u16);
        x.select_speed(&crate::objects::motion::SPEEDS_B, 0x36);
        x.anim_start(ANIMS, 0);
    }

    fn fall<R: Bench>(x: &mut Cx<'_, R>) {
        x.move_xz();
        x.anim_step();
        x.cull_offscreen();
        if (x.u16(0xc) as i16) < 0 {
            crate::sound::play(x.b, sfx::ECE2C, x.o);
            x.set16(0xc, 0);
            x.add8(0x5, 2);
            let (vx, ax) = (x.u16(f::OWNER), x.u16(0x42));
            x.set16(0x14, vx);
            x.set16(0x16, ax);
            x.set8(f::ATTACK_POWER, 6);
            x.set8(f::ATTACK_ID, 8);
            x.set8(f::TIMER, 0x20);
            x.select_speed(&crate::objects::motion::SPEEDS_B, 0x17);
        }
    }

    fn bounce<R: Bench>(x: &mut Cx<'_, R>) {
        x.move_by_table_xy();
        x.anim_step();
        x.cull_offscreen();
        let t = x.u8(f::TIMER).wrapping_sub(1);
        x.set8(f::TIMER, t);
        if (t as i8) < 0 {
            x.add8(0x5, 2);
            crate::sound::play(x.b, sfx::EBD30, x.o);
            x.anim_start(ANIMS, 1);
        }
    }

    fn explode<R: Bench>(x: &mut Cx<'_, R>) {
        if x.anim_step() {
            x.set16(0x4, 0x400);
        }
    }
}

/// Shot 6 (0x9ebca): contact effect stuck in front of the owner (2 variants of power 6 and 7). It stays as long as its
/// animation lasts and the owner is in normal play.
mod shot6 {
    use super::*;

    const ANIMS: u32 = 0x16ccea;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => {
                if x.a5_u8(GAME_PAUSED) != 0 {
                    return;
                }
                if x.u16(f::OWNER) == 0 {
                    return x.free_eshl();
                }
                x.add8(0x4, 2);
                let owner = x.linked(f::OWNER);
                x.set8(f::BOX_SET, 7);
                let (id, power) = if x.u8(0x3) == 0 { (0xc, 6) } else { (0xd, 7) };
                x.set8(f::ATTACK_POWER, power);
                x.set8(f::ATTACK_ID, id);
                place_in_front(x, owner, 0x40, 0x1b);
                let variant = x.u8(0x3) as u32;
                x.anim_start(ANIMS, variant as u16);
                x.reinsert();
            }
            2 => {
                if suspended(x) {
                    return;
                }
                if x.u16(f::OWNER) == 0 || x.linked(f::OWNER).u8(x.b, 0x4) as i8 >= 4 {
                    return x.free_eshl();
                }
                if x.a5_u8(GAME_PAUSED) != 0 {
                    return;
                }
                if x.anim_step() {
                    x.set8(0x4, st::FREE);
                }
            }
            _ => x.free_eshl(),
        }
    }
}

/// Shot 7 (0x9ecc0): spark placed on the owner (offset read from a 4-byte table per variant); disappears at the end of
/// the animation.
mod shot7 {
    use super::*;

    const ANIMS: u32 = 0x16d1d6;
    const OFFSETS: u32 = 0x9ed64;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => {
                if x.u16(f::OWNER) == 0 {
                    return;
                }
                x.add8(0x4, 2);
                x.set8(f::BOX_SET, 7);
                x.set8(f::ATTACK_POWER, 6);
                x.set8(f::ATTACK_ID, 0x1c);
                let owner = x.linked(f::OWNER);
                copy_position(x, owner);
                x.add16(0x10, 0xffff);
                let row = OFFSETS + 4 * x.u8(0x3) as u32;
                let (dx, dy) = (x.rom16(row), x.rom16(row + 2));
                x.add16(0x8, dx);
                x.add16(0x10, dy);
                x.anim_start(ANIMS, 0);
                x.reinsert();
                wait_end(x);
            }
            2 => wait_end(x),
            _ => x.free_eshl(),
        }
    }

    fn wait_end<R: Bench>(x: &mut Cx<'_, R>) {
        if x.anim_step() {
            x.add8(0x4, 2);
        }
    }
}

/// Shot 8 (0x9ee00): horizontal thrust in front of the owner. Advances (X, Y, Z) until the animation sets `+0x33`, then
/// finishes the animation.
mod shot8 {
    use super::*;

    const ANIMS: u32 = 0x16ccea;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => {
                if x.u16(f::OWNER) == 0 {
                    // 0x9ee16: without an owner, the effect goes to state 6
                    x.set32(0x4, 0x0600_0000);
                    return;
                }
                x.add8(0x4, 2);
                let owner = x.linked(f::OWNER);
                x.set8(f::BOX_SET, 7);
                x.set8(f::ATTACK_POWER, 6);
                x.set8(f::ATTACK_ID, 0xe);
                place_in_front(x, owner, 0x46, 0x12);
                x.anim_start(ANIMS, 2);
                body(x);
            }
            2 => body(x),
            _ => x.free_eshl(),
        }
    }

    const THRUST: u8 = 0;
    const RECOVER: u8 = 2;

    fn body<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x5) {
            THRUST => {
                x.move_xyz();
                x.cull_offscreen();
                if x.u8(0x33) != 0 {
                    x.add8(0x5, 2);
                }
                x.anim_step();
            }
            RECOVER => {
                if x.anim_step() {
                    x.set8(0x4, st::FREE);
                }
            }
            _ => {}
        }
        x.keep_in_zone();
        x.push_terrain();
        x.reinsert();
    }
}

/// Shot 9 (0x9ef46): attack with three forms according to the variant `+3` (ball that falls back, breath stuck to an
/// enemy, spark placed on an enemy).
mod shot9 {
    use super::*;

    const ANIMS: u32 = 0x16d39a;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x3) {
            0 => bolt(x),
            1 => breath(x),
            _ => sparkle(x),
        }
    }

    fn die<R: Bench>(x: &mut Cx<'_, R>) {
        x.set8(0x4, st::FREE);
    }

    // ----- variant 0: ball that falls back -----

    /// Aligns on the owner (in front of it, slightly deeper). Without an owner, the object is marked for destruction
    /// (and the caller still continues its initialization).
    fn attach<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u16(f::OWNER) == 0 {
            return die(x);
        }
        let owner = x.linked(f::OWNER);
        for off in [0x8, 0x10, 0xc] {
            let v = owner.u32(x.b, off);
            x.set32(off, v);
        }
        let flip = owner.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        x.add16(0x8, if flip != 0 { 0x3e } else { -0x3ei16 as u16 });
        x.add32(0x10, 0x1200);
        x.add16(0xc, -0x1fi16 as u16);
    }

    fn bolt<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                attach(x);
                x.set8(f::BOX_SET, 7);
                x.set8(f::ATTACK_ID, 0x20);
                x.set16(0x18, -0x400i16 as u16);
                x.set16(0x1a, 0);
                // peculiarity of the original: read at a fixed offset of the object (A0 - 0x7f92)
                let phase = x.data16(x.o.0.wrapping_sub(0x7f92)) as u32;
                let mut index = (phase >> 2 & 3) + 6;
                if x.u8(f::FLIP) == 0 {
                    index += 0x10;
                }
                x.set8(0x2d, index as u8);
                x.set32(0x14, 0);
                x.set32(0x1c, 0);
                x.select_speed(&crate::objects::motion::SPEEDS_A, 0x1d);
                x.anim_start(ANIMS, 0);
            }
            2 => {
                if !suspended(x) {
                    match x.u8(0x5) {
                        0 => {
                            x.move_by_table_xy();
                            x.move_z();
                            if (x.u16(0xc) as i16) > 0 {
                                x.anim_step();
                            } else {
                                x.add8(0x5, 2);
                                x.set16(0xc, 0);
                                // the original reads here the low word of the long +0x14 (the acceleration)
                                let (vx, vy) = (x.u16(0x16) as i16 >> 1, x.u16(0x1c) as i16 >> 1);
                                x.set16(0x14, vx as u16);
                                x.set16(0x1c, vy as u16);
                                x.set16(0x18, 0);
                                x.set16(0x1a, 0x10);
                                x.anim_start(ANIMS, 1);
                            }
                        }
                        2 => {
                            x.move_xyz();
                            if x.anim_step() {
                                die(x);
                            }
                        }
                        _ => {}
                    }
                }
                x.reinsert();
            }
            _ => x.free_eshl(),
        }
    }

    // ----- variant 1: breath stuck to an enemy of type 7 -----

    const FIRE_FRAMES: u32 = 0x16d676;
    const END_FRAMES: u32 = 0x16d684;
    /// Limit heights (words) of the rise of the breath, by value of `+0x3e`.
    const RISE_LIMITS: u32 = 0x9f14c;

    /// The creator must be an enemy (class 2) of type 7, variant 0, in state 0x208, itself linked to a player still in
    /// play; otherwise the breath disappears.
    fn breath<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(0x5, 0);
                x.set8(0x2f, 0);
                x.set8(f::BOX_SET, 7);
                x.set8(f::ATTACK_ID, 0x20);
                x.anim_start(ANIMS, 2);
            }
            2 => {
                if !suspended(x) {
                    if (x.u16(0x46) as i16) >= 0 {
                        return die(x);
                    }
                    let enemy = x.linked(0x46);
                    let (class, kind, variant, state) =
                        (enemy.u8(x.b, 0x28), enemy.u8(x.b, 0x2), enemy.u8(x.b, 0x3), enemy.u16(x.b, 0x4));
                    if class != 2 || kind != 7 || variant != 0 || state != 0x208 {
                        return die(x);
                    }
                    let target = enemy.u16(x.b, 0x46);
                    if (target as i16) >= 0 || Obj(target as i16 as i32 as u32 & 0xff_ffff).u8(x.b, 0x4) as i8 >= 4 {
                        return die(x);
                    }
                    follow(x, enemy);
                    breath_body(x, enemy);
                }
                x.reinsert();
            }
            _ => x.free_eshl(),
        }
    }

    fn breath_body<R: Bench>(x: &mut Cx<'_, R>, enemy: Obj) {
        const RISING: u8 = 0;
        const ENDING: u8 = 2;
        match x.u8(0x5) {
            RISING => {
                if enemy.u8(x.b, 0x6) as i8 > 4 {
                    x.add8(0x5, 2);
                    x.set8(0x6, 0);
                    x.anim_start(ANIMS, 4);
                    return end_frames(x);
                }
                match x.u8(0x6) {
                    0 => {
                        x.anim_step();
                        let z = x.u16(0xc) as i16;
                        let k = x.u8(0x3e) as i8;
                        if z >= 0xa0 || k < 0 || x.rom_i16(RISE_LIMITS + 2 * k as u32) < z {
                            return;
                        }
                        x.add8(0x6, 2);
                        x.anim_start(ANIMS, 3);
                        grow(x);
                    }
                    _ => grow(x),
                }
            }
            ENDING => end_frames(x),
            _ => {}
        }
    }

    fn grow<R: Bench>(x: &mut Cx<'_, R>) {
        x.anim_step();
        pick_frame(x, FIRE_FRAMES);
    }

    fn end_frames<R: Bench>(x: &mut Cx<'_, R>) {
        if x.anim_step() {
            return die(x);
        }
        pick_frame(x, END_FRAMES);
    }

    /// Sticks the breath to its creator: position, orientation, one notch deeper.
    fn follow<R: Bench>(x: &mut Cx<'_, R>, enemy: Obj) {
        for off in [0x8, 0xc, 0x10] {
            let v = enemy.u32(x.b, off);
            x.set32(off, v);
        }
        let flip = enemy.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        x.add16(0x12, 1);
        x.add16(0x8, if flip != 0 { 0x3e } else { -0x3ei16 as u16 });
        x.add16(0xc, -0x1fi16 as u16);
    }

    /// Sprite frame of the breath: set chosen according to the height, frame according to `+0x3e`.
    fn pick_frame<R: Bench>(x: &mut Cx<'_, R>, set: u32) {
        let n = ((x.u16(0xc) >> 4).min(0xa) as i16 - 4).max(0);
        x.set8(0x2f, n as u8);
        let frames = set.wrapping_add(x.data16(set + 2 * n as u32) as i16 as i32 as u32);
        let entry = frames + 8 * x.u8(0x3e) as u32;
        let (a, b) = (x.data32(entry), x.data32(entry + 4));
        x.set32(0x34, a);
        x.set32(0x38, b);
    }

    // ----- variant 2: spark placed on an enemy -----

    fn sparkle<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(f::BOX_SET, 7);
                x.set8(f::ATTACK_ID, 0x20);
                x.anim_start(ANIMS, 5);
            }
            2 => {
                if suspended(x) {
                    x.set_bit(0x1, 1, false);
                    return x.reinsert();
                }
                if (x.u16(0x46) as i16) >= 0 {
                    return die(x);
                }
                let host = x.linked(0x46);
                if host.u8(x.b, 0) == 0 {
                    return die(x);
                }
                let hx = host.u16(x.b, 0x8);
                x.set16(0x8, hx);
                let hy = host.u16(x.b, 0x10).wrapping_sub(8);
                x.set16(0x10, hy);
                x.set16(0xc, 0);
                let flip = host.u8(x.b, f::FLIP);
                x.set8(f::FLIP, flip);
                let growth = host.u8(x.b, 0x2f) as u16;
                if growth == 0 {
                    x.set_bit(0x1, 1, true);
                    return x.reinsert();
                }
                let reach = (growth << 4) + 0x74 + (host.u16(x.b, 0xc) & 0xf);
                x.add16(0x8, if flip != 0 { reach } else { reach.wrapping_neg() });
                x.anim_step();
                x.set_bit(0x1, 1, false);
                x.reinsert();
            }
            _ => x.free_eshl(),
        }
    }
}

/// Shot 10 (0x9f376): edge projectile (power 0x10), placed according to a table row chosen by the register D5 of the
/// dispatch (peculiarity of the original). Waits for a state of the owner, then leaves 240 frames before disappearing.
mod shot10 {
    use super::*;

    const ANIMS: u32 = 0x16da84;
    const PLACEMENT: u32 = 0x9f3c8;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(f::ATTACK_POWER, 0x10);
                x.set8(f::ATTACK_ID, 1);
                x.set8(f::BOX_SET, 0x40);
                x.add32(0x10, u32::MAX);
                // Original glitch: the placement row is the low word of D5, left by the previous handler
                let row = PLACEMENT.wrapping_add(*x.c.d5() as u16 as i16 as i32 as u32);
                let (z, dx) = (x.rom16(row), x.rom16(row + 2));
                x.set16(0xc, z);
                let flip = x.u8(f::FLIP) != 0;
                x.add16(0x8, if flip { dx } else { dx.wrapping_neg() });
                x.anim_start(ANIMS, 0);
            }
            2 => {
                let owner = x.linked(f::OWNER);
                if owner.u8(x.b, 0x4) as i8 >= 4 {
                    return x.free_eshl();
                }
                match x.u8(0x5) {
                    2 => {
                        x.add8(0x5, 2);
                        x.set16(0x52, 0xf0);
                        x.anim_start(ANIMS, 1);
                    }
                    0 | 1 => {
                        x.anim_step();
                        x.reinsert();
                    }
                    _ => {
                        let t = x.u16(0x52).wrapping_sub(1);
                        x.set16(0x52, t);
                        if (t as i16) < 0 {
                            x.add8(0x4, 2);
                        }
                        x.anim_step();
                        x.reinsert();
                    }
                }
            }
            _ => x.free_eshl(),
        }
    }
}

/// Shot 11 (0x9f426): halo stuck to an enemy (displacement read from a 4-byte table per variant). Its animation
/// advances according to bits 0 and 1 of `+0xb0` of the owner; it can also put the owner in state 4 (stunned) after
/// having been frozen (`+0x53`).
mod shot11 {
    use super::*;

    const ANIMS: u32 = 0x169fb0;
    const OFFSETS: u32 = 0x9f524;
    /// Variable `A5+0x6f`: bit 0 = forced animation rate.
    const FORCE_ANIM: u32 = 0x6f;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                attach(x);
                x.set32(0xc, 0);
                x.set8(f::BOX_SET, 0x3b);
                x.anim_start(ANIMS, 1);
            }
            2 => {
                if frozen(x) {
                    return x.reinsert();
                }
                if x.a5_u8(GAME_PAUSED) == 0 {
                    let owner = attach(x);
                    let flags = owner.u8(x.b, 0xb0);
                    if flags != 0 {
                        let differ = (flags & 1 != 0) != (flags & 2 != 0);
                        if !differ || x.a5_u8(FORCE_ANIM) & 1 != 0 {
                            x.anim_step();
                        }
                    }
                }
                x.reinsert();
            }
            4 => {
                let owner = x.linked(f::OWNER);
                if owner.u8(x.b, 0x4) == 4 {
                    if !frozen(x) {
                        attach(x);
                    }
                } else {
                    let v = x.u16(0x54);
                    owner.set_u16(x.b, 0x54, v);
                    owner.set_u32(x.b, 0x4, 0x0400_0000);
                }
                x.reinsert();
            }
            _ => x.free_eshl(),
        }
    }

    /// Countdown `+0x53` positive: decremented, and the halo stays frozen.
    fn frozen<R: Bench>(x: &mut Cx<'_, R>) -> bool {
        let n = x.u8(0x53);
        if (n as i8) > 0 {
            x.set8(0x53, n - 1);
            return true;
        }
        false
    }

    fn attach<R: Bench>(x: &mut Cx<'_, R>) -> Obj {
        let owner = x.linked(f::OWNER);
        for off in [0x8, 0x10] {
            let v = owner.u32(x.b, off);
            x.set32(off, v);
        }
        let row = OFFSETS + (4 * x.u8(0x3) as u32 & 0xff);
        let (dx, dy) = (x.rom16(row), x.rom16(row + 2));
        x.add16(0x8, dx);
        x.add16(0x10, dy);
        owner
    }
}

/// Shot 12 (0x9f5a8): mark placed on the attachment point `+0x5a` (offset in X and Z read from a table, according to
/// the variant); the animation comes from a byte table in ROM.
mod shot12 {
    use super::*;

    const ANIMS: u32 = 0x16dda2;
    const OFFSETS: u32 = 0x9f654;
    const ANIM_OF_VARIANT: u32 = 0x9f5e0;
    /// Attachment point (RAM address on 16 bits).
    const ANCHOR: u32 = 0x5a;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                attach(x);
                let variant = x.u8(0x3) as u32;
                let anim = x.rom8(ANIM_OF_VARIANT + variant);
                x.anim_start(ANIMS, (anim as u32) as u16);
                animate(x);
            }
            2 => animate(x),
            _ => x.free_eshl(),
        }
    }

    fn animate<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(0x5) == 0 {
            x.anim_step();
        }
        x.reinsert();
    }

    fn attach<R: Bench>(x: &mut Cx<'_, R>) {
        let anchor = x.linked(ANCHOR);
        for off in [0x8, 0xc, 0x10] {
            let v = anchor.u16(x.b, off);
            x.set16(off, v);
        }
        x.add32(0x10, u32::MAX);
        let row = OFFSETS + (4 * x.u8(0x3) as u32 & 0xff);
        let (dx, dz) = (x.rom16(row), x.rom16(row + 2));
        let flip = anchor.u8(x.b, f::FLIP) != 0;
        x.add16(0x8, if flip { dx } else { dx.wrapping_neg() });
        x.add16(0xc, dz);
    }
}

/// Shot 13 (0x9f67c): effect stuck to the owner (position, orientation and offset per variant read from an 8-byte
/// table), whose animation (0xc or 0xe) follows byte `+0xa1` of the owner. When paused, it only follows the position.
mod shot13 {
    use super::*;

    const ANIMS: u32 = 0x17a276;
    const OFFSETS: u32 = 0x9f776;
    /// The handler's address (A1 left by the dispatch loop).
    const HANDLER: u32 = 0x9f67c;
    const CURRENT_ANIM: u32 = 0x42;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => {
                if x.a5_u8(GAME_PAUSED) != 0 {
                    attach(x);
                    return;
                }
                x.add8(0x4, 2);
                attach(x);
                x.set32(0xc, 0);
                x.set8(f::BOX_SET, 0x29);
                x.anim_start(ANIMS, 0xc);
            }
            2 => {
                let owner = attach(x);
                if x.a5_u8(GAME_PAUSED) != 0 {
                    return;
                }
                let mode = owner.u8(x.b, 0xa1);
                let anim = if mode != 0 { 0xe } else { 0xc };
                if anim != x.u8(CURRENT_ANIM) {
                    x.set8(CURRENT_ANIM, anim);
                    x.anim_start(ANIMS, (anim as u32) as u16);
                }
                if mode != 2 {
                    x.anim_step();
                }
                x.reinsert();
            }
            _ => x.free_eshl(),
        }
    }

    /// Sticks the effect to its owner (0x9f712). Without an owner, the effect goes to state 6 (`move.l #0x6000000,(4,A0)`
    /// at 0x9f692, whose `rts` returns to the caller, which goes on) and A1 keeps the value left by the dispatch loop.
    /// Original glitch: that A1 is the handler's address (0x9f67c); the "owner's `+0xa1`" byte read next is a constant
    /// byte of its code.
    fn attach<R: Bench>(x: &mut Cx<'_, R>) -> Obj {
        if x.u16(f::OWNER) == 0 {
            x.set32(0x4, 0x0600_0000);
            return Obj(HANDLER);
        }
        let owner = x.linked(f::OWNER);
        for off in [0x8, 0x10, 0xc] {
            let v = owner.u32(x.b, off);
            x.set32(off, v);
        }
        let flip = owner.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        let row = OFFSETS + (8 * x.u8(0x3) as u32 & 0xff);
        let (dx, dy, dfrac, dz) = (x.rom16(row), x.rom16(row + 2), x.rom16(row + 4), x.rom16(row + 6));
        x.add16(0x8, if flip != 0 { dx.wrapping_neg() } else { dx });
        x.add16(0x10, dy);
        x.add32(0x10, dfrac as i16 as i32 as u32);
        x.add16(0xc, dz);
        owner
    }
}

/// Shot 14 (0x9f786): attack part of a composite enemy (variant `+3` from 0 to 7, 9 types of forms) attached to its
/// owner (`+0x50`). It reads the owner's attack flags (`+0xa8`: attack launched, `+0xa9`: attack in preparation, one
/// bit per variant) and pushes it into the corresponding state, plays the phase's animation and creates shots of type
/// 15.
mod shot14 {
    use super::*;

    const ANIMS: u32 = 0x17a276;
    const ANIMS_CHARGE: u32 = 0x17a29e;
    const ANIMS_END: u32 = 0x17a2b4;
    const ANIM_OF_VARIANT: u32 = 0x9f7dc;
    const OFFSETS: u32 = 0x9fd50;
    /// Byte table (data space) of the edge shot animations.
    const BREATH_ANIMS: u32 = 0x9fe00;
    /// Attack flags of the owner.
    const LAUNCHED: u32 = 0xa8;
    const PREPARING: u32 = 0xa9;

    /// Phases (`+4`).
    const START: u8 = 0;
    const ACTIVE: u8 = 2;
    const ENDING: u8 = 4;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            START => {
                if x.a5_u8(GAME_PAUSED) != 0 {
                    return attach(x);
                }
                if (x.a5_u16(0x7e64) as i16) < 1 {
                    return;
                }
                x.add8(0x4, 2);
                attach(x);
                x.set32(0xc, 0);
                x.set8(f::BOX_SET, 7);
                let variant = x.u8(0x3) as u32;
                let anim = x.rom8(ANIM_OF_VARIANT + variant);
                x.anim_start(ANIMS, (anim as u32) as u16);
            }
            ACTIVE => {
                if x.a5_u8(GAME_PAUSED) != 0 {
                    return attach(x);
                }
                let owner = x.linked(f::OWNER);
                let stalled = owner.u16(x.b, 0x4) != 0x402 && (owner.u8(x.b, 0x53) as i8) > 0;
                if !stalled {
                    match x.u8(0x5) {
                        0 => enter(x, owner),
                        2 => attack(x),
                        _ => {}
                    }
                }
                x.reinsert();
            }
            ENDING => {
                match x.u8(0x5) {
                    0 => finish(x),
                    2 => attach(x),
                    _ => {}
                }
                x.reinsert();
            }
            _ => x.free_eshl(),
        }
    }

    /// Sticks the part to the owner: position, orientation and offset according to the variant.
    fn attach<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u16(f::OWNER) == 0 {
            // the original calls itself indefinitely; this case does not exist in play
            return;
        }
        let owner = x.linked(f::OWNER);
        for off in [0x8, 0x10, 0xc] {
            let v = owner.u32(x.b, off);
            x.set32(off, v);
        }
        let flip = owner.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        let row = OFFSETS + (8 * x.u8(0x3) as u32 & 0xff);
        let (mut dx, mut dy, mut dfrac, dz) = (x.rom16(row), x.rom16(row + 2), x.rom16(row + 4), x.rom16(row + 6));
        if flip != 0 {
            dx = dx.wrapping_neg();
            if x.u8(0x3) == 2 {
                dx = dx.wrapping_sub(0x24);
                dy = dy.wrapping_sub(0x20);
                dfrac = 0x9000;
            }
        }
        x.add16(0x8, dx);
        x.add16(0x10, dy);
        x.add32(0x10, dfrac as i16 as i32 as u32);
        x.add16(0xc, dz);
    }

    /// First substate of the active phase: according to the variant and the owner's attack flags, sets the part's
    /// states (`+4..+7`), then animates.
    fn enter<R: Bench>(x: &mut Cx<'_, R>, owner: Obj) {
        attach(x);
        let v = x.u8(0x3);
        let flags = |x: &mut Cx<'_, R>, off: u32| owner.u8(x.b, off) >> (v & 7) & 1 != 0;
        let (launched, preparing) = (flags(x, LAUNCHED), flags(x, PREPARING));
        let owner_state = owner.u32(x.b, 0x4);
        let remap = |x: &mut Cx<'_, R>, rules: &[(u32, u32)]| {
            for &(from, to) in rules {
                if owner_state == from {
                    x.set32(0x4, to);
                }
            }
        };
        match v {
            0 => {
                if launched {
                    x.set32(0x4, 0x0400_0a00);
                } else if preparing {
                    remap(
                        x,
                        &[
                            (0x0204_0402, 0x0202_0000),
                            (0x0204_0602, 0x0202_0200),
                            (0x0204_0a06, 0x0202_0000),
                            (0x0204_1002, 0x0202_0600),
                            (0x0204_1204, 0x0202_0600),
                            (0x0204_1606, 0x0202_0600),
                        ],
                    );
                }
            }
            2 => {
                if launched {
                    x.set32(0x4, 0x0400_0400);
                }
            }
            3 | 4 => {
                if launched {
                    x.set32(0x4, if v == 3 { 0x0400_0800 } else { 0x0400_0600 });
                } else if preparing {
                    remap(
                        x,
                        &[(0x0204_0804, 0x0202_0400), (0x0204_0a02, 0x0202_0400), (0x0204_1404, 0x0202_0400), (0x0204_1602, 0x0202_0400)],
                    );
                }
            }
            6 => {
                if launched {
                    x.set32(0x4, 0x0400_0000);
                }
            }
            7 => {
                if launched {
                    x.set32(0x4, 0x0400_0200);
                }
            }
            _ => {}
        }
        if v == 2 {
            let anim = if owner.u8(x.b, 0xa1) == 2 { 4 } else { 3 };
            if anim != x.u8(0x5c) {
                x.set8(0x5c, anim);
                x.anim_start(ANIMS, (anim as u32) as u16);
            }
        }
        x.anim_step();
        x.reinsert();
    }

    /// Creates a type 15 shot linked to this part; the original does not examine the allocation failure.
    fn spawn_child<R: Bench>(x: &mut Cx<'_, R>, variant: u8) -> Obj {
        // Original glitch: the allocation failure is not tested (see `Pool::alloc_unchecked`).
        let child = x.alloc_unchecked(&pools::ESHL);
        let active = x.u8(0);
        child.set_u8(x.b, 0, active);
        child.set_u8(x.b, 2, 0xf);
        child.set_u8(x.b, 3, variant);
        child.set_u16(x.b, f::OWNER, x.o.0 as u16);
        child
    }

    /// Substate 2 of the active phase: five animation sequences according to `+6` / `+7`.
    fn attack<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x6) {
            0 => match x.u8(0x7) {
                0 => begin_charge(x, 3),
                2 => {
                    attach(x);
                    if x.anim_step() {
                        x.add8(0x7, 2);
                        let child = spawn_child(x, 1);
                        x.set16(0x44, child.0 as u16);
                    }
                }
                4 => wait_child(x),
                _ => close(x, 7),
            },
            2 => match x.u8(0x7) {
                0 => begin_charge(x, 3),
                2 => {
                    attach(x);
                    if x.anim_step() {
                        x.add8(0x7, 2);
                        let owner = x.linked(f::OWNER);
                        owner.set_u8(x.b, 0xbd, 0);
                        let child = spawn_child(x, 1);
                        x.set16(0x44, child.0 as u16);
                    }
                }
                4 => {
                    let child = x.u16(0x44);
                    if child == 0 || Obj(child as i16 as i32 as u32 & 0xff_ffff).u8(x.b, 0x5d) == 1 {
                        x.add8(0x7, 2);
                        let owner = x.linked(f::OWNER);
                        owner.set_u8(x.b, 0xbd, 0);
                        x.anim_start(ANIMS_CHARGE, 4);
                    } else {
                        attach(x);
                        x.anim_step();
                    }
                }
                _ => close(x, 7),
            },
            4 => match x.u8(0x7) {
                0 => {
                    x.add8(0x7, 2);
                    x.set8(f::ATTACK_POWER, 8);
                    x.set8(f::ATTACK_ID, 0xf);
                    crate::sound::play(x.b, sfx::E46DE, x.o);
                    breath_anim(x);
                }
                _ => {
                    if x.u8(0x33) != 0 {
                        x.set8(0x33, 0);
                        let owner = x.linked(f::OWNER);
                        owner.set_u8(x.b, 0xbd, 0);
                    }
                    attach(x);
                    if x.anim_step() {
                        x.set32(0x4, 0x0200_0000);
                        let owner = x.linked(f::OWNER);
                        let v = owner.u8(x.b, PREPARING);
                        owner.set_u8(x.b, PREPARING, v & !0x18);
                        let anim = if x.u8(0x3) == 3 { 8 } else { 9 };
                        x.anim_start(ANIMS, anim);
                    }
                }
            },
            _ => match x.u8(0x7) {
                0 => {
                    x.add8(0x7, 2);
                    x.anim_start(ANIMS_CHARGE, 2);
                }
                2 => {
                    if x.u8(0x33) != 0 {
                        crate::sound::play(x.b, sfx::E47BC, x.o);
                        x.add8(0x7, 2);
                        let child = spawn_child(x, 0);
                        x.set16(0x42, child.0 as u16);
                    }
                    attach(x);
                    x.anim_step();
                }
                // 0x9fbd4: the third word of the jump table (0x9fbe2) leads to 0x9fb1e, the end of the other series
                _ => close(x, 7),
            },
        }
    }

    fn begin_charge<R: Bench>(x: &mut Cx<'_, R>, anim: u32) {
        x.add8(0x7, 2);
        x.anim_start(ANIMS_CHARGE, anim as u16);
    }

    fn wait_child<R: Bench>(x: &mut Cx<'_, R>) {
        let child = x.u16(0x44);
        if child == 0 || Obj(child as i16 as i32 as u32 & 0xff_ffff).u8(x.b, 0x5d) == 1 {
            x.add8(0x7, 2);
            x.anim_start(ANIMS_CHARGE, 4);
        } else {
            attach(x);
            x.anim_step();
        }
    }

    /// End of a charge: cedes control to the owner (state 2) and starts the closing animation.
    fn close<R: Bench>(x: &mut Cx<'_, R>, anim: u32) {
        attach(x);
        if x.anim_step() {
            x.set32(0x4, 0x0200_0000);
            let owner = x.linked(f::OWNER);
            let v = owner.u8(x.b, PREPARING);
            owner.set_u8(x.b, PREPARING, v & !1);
            owner.set_u8(x.b, 0xbd, 0);
            x.anim_start(ANIMS, anim as u16);
        }
    }

    /// Breath animation according to the distance between the owner and its target.
    fn breath_anim<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u16(f::OWNER) == 0 {
            return;
        }
        let owner = x.linked(f::OWNER);
        let target = owner.u16(x.b, f::OWNER);
        let mut index = 3u32;
        if target != 0 {
            let target = Obj(target as i16 as i32 as u32 & 0xff_ffff);
            let (a, b) = if owner.u8(x.b, f::FLIP) == 0 {
                (target.u16(x.b, 0x8), owner.u16(x.b, 0x8))
            } else {
                (owner.u16(x.b, 0x8), target.u16(x.b, 0x8))
            };
            index = if (b.wrapping_sub(a) as i16) > 0xfc { 3 } else { 2 };
        }
        index *= 2;
        let first = x.data8(BREATH_ANIMS + index);
        let second = x.data8(BREATH_ANIMS + index + 1);
        let anim = if x.u8(0x3) == 3 { first } else { second };
        x.anim_start(ANIMS_CHARGE, (anim as u32) as u16);
    }

    /// Phase 4, substate 0: releases the owner's preparation flag and starts the end animation chosen by `+6`.
    fn finish<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x5, 2);
        let v = x.u8(0x3);
        let owner = x.linked(f::OWNER);
        let flags = owner.u8(x.b, PREPARING);
        owner.set_u8(x.b, PREPARING, flags & !(1 << (v & 7)));
        let anim = match x.u8(0x6) {
            0 => 5,
            2 => 6,
            4 => 7,
            6 => 4,
            8 => 3,
            _ => 2,
        };
        x.anim_start(ANIMS_END, anim);
        attach(x);
    }
}

/// Shot 5 (0x9e458): contact zone of a large enemy (link `+0x46` to that enemy). It copies the enemy's visibility
/// flags, chooses its sprite frame at each frame (two tables, one for even frames, the other, with flipping, for odd
/// frames when `+0xb2` of the enemy is zero) and its collision boxes (body, push, attack) according to the enemy's
/// attack state `+0xa0/+0xa1`. It disappears with it.
mod shot5 {
    use super::*;

    const ANIMS: u32 = 0x17b9c2;
    /// Offset tables (words, program space) to frame arrays (longs).
    const FRAMES_EVEN: u32 = 0x9e638;
    const FRAMES_ODD: u32 = 0x9e6a4;
    /// Choice of the odd frame: bytes (frame, flip) indexed by `+0x42`, `+0x43`.
    const MIRRORED: u32 = 0x9e6bc;
    /// Boxes per value of `+0x2f`: 4 bytes (index of the body, of the push, of the attack, then an index stored in
    /// `+0x3b`).
    const BOXES: u32 = 0x9e55c;
    const HOST: u32 = 0x46;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        if suspended(x) {
            return;
        }
        if (x.u16(HOST) as i16) >= 0 {
            return vanish(x);
        }
        let host = x.linked(HOST);
        if host.u8(x.b, 0) == 0 {
            return vanish(x);
        }
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(0x5, 0);
                x.set8(0x42, 0);
                x.set8(0x43, 0);
                x.set8(0x2d, 0);
                x.anim_start(ANIMS, 0);
                let kind = host.u8(x.b, 0x2);
                x.set8(f::ATTACK_ID, if kind == 0xc { 0x29 } else { 0x2a });
                x.set8(f::BOX_SET, 7);
                boxes(x, host);
            }
            2 => {
                if x.a5_u8(GAME_PAUSED) == 0 {
                    x.set8(0x56, 0);
                    x.set8(0x53, 0);
                    select_frame(x);
                    boxes(x, host);
                }
                let flags = host.u8(x.b, 0x1);
                x.set8(0x1, flags);
                x.reinsert();
            }
            4 => x.add8(0x4, 2),
            _ => x.free_eshl(),
        }
    }

    fn vanish<R: Bench>(x: &mut Cx<'_, R>) {
        x.set8(0x4, st::FREE);
        x.set8(0x5, 0);
    }

    /// Collision boxes according to the enemy's attack state: disarmed (0) for some states.
    fn boxes<R: Bench>(x: &mut Cx<'_, R>, host: Obj) {
        let row = 4 * x.u8(0x2f) as u32;
        let index = x.rom8(BOXES + 3 + row);
        x.set8(0x3b, index);
        let (kind, detail) = (host.u16(x.b, 0xa0), host.u8(x.b, 0xa1));
        let special = matches!(kind, 0x200 | 0x224 | 0x225 | 0x226) || kind >> 8 == 4;
        let armed = if kind >> 8 == 0 {
            match detail {
                5 | 6 => true,
                7 => !special,
                _ => false,
            }
        } else {
            !special
        };
        for i in 0..3 {
            let v = if armed { x.rom8(BOXES + i + row) } else { 0 };
            x.set8(0x38 + i, v);
        }
    }

    /// Frame array number `set` of an offset table (words, relative to the table).
    fn table<R: Bench>(x: &mut Cx<'_, R>, base: u32, set: u32) -> u32 {
        base.wrapping_add(x.rom16(base + 2 * set) as i16 as i32 as u32)
    }

    /// Chooses the sprite frame (`+0x34`); `+0x2d` is a counter 1..=0x11.
    fn select_frame<R: Bench>(x: &mut Cx<'_, R>) {
        let mut n = x.u8(0x2d) as u16;
        if n >= 0x11 {
            n = 0;
        }
        n += 1;
        x.set8(0x2d, n as u8);
        let host = x.linked(HOST);
        let set = x.u8(0x2f) as u32;
        if host.u8(x.b, 0xb2) != 0 || n & 1 == 0 {
            let frames = table(x, FRAMES_EVEN, set);
            let frame = x.data32(frames + 2 * (n & !1) as u32);
            x.set32(0x34, frame);
        } else if (x.u16(HOST) as i16) < 0 {
            let row = (x.u8(0x42) as u32 >> 1 << 5) + 2 * (x.u8(0x43) as u32 >> 1);
            let flip = x.data8(MIRRORED + row + 1);
            x.set8(f::FLIP, flip);
            // the original clears D0 before this read: the frame always comes from the first row
            let slot = 4 * x.data8(MIRRORED) as u32;
            let frames = table(x, FRAMES_ODD, set);
            let frame = x.data32(frames + slot);
            x.set32(0x34, frame);
        }
    }
}

handlers! {
    0x9dbfc "eshl_00" shot0::run,
    0x9de10 "eshl_01" shot1::run,
    0x9e0aa "eshl_02" shot2::run,
    0x9e1b6 "eshl_03" shot3::run,
    0x9e2dc "eshl_04" shot4::run,
    0x9ebca "eshl_06" shot6::run,
    0x9ecc0 "eshl_07" shot7::run,
    0x9ee00 "eshl_08" shot8::run,
    0x9ef46 "eshl_09" shot9::run,
    0x9f376 "eshl_10" shot10::run,
    0x9f426 "eshl_11" shot11::run,
    0x9f5a8 "eshl_12" shot12::run,
    0x9f67c "eshl_13" shot13::run,
    0x9f786 "eshl_14" shot14::run,
    0x9e458 "eshl_05" shot5::run,
}
