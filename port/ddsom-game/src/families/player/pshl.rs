//! Player shots (class PSHL, pool of 18 objects of 0x60 bytes, table 0x39908), first series: types 0 to 5. See
//! `docs/families/joueur.md`.
//!
//! The shots are born from a creator (`player/spawn.rs`) that sets their kind, their owner (`+0x50`), their boxes
//! (`+0x2e`), the power (`+0x45`, `+0x46`) and the speed; the handler makes them live.

use crate::families::enemy_common::records::Enemy;
use super::vars;
use crate::families::player::records::Shot;
use crate::player::Player;
use crate::objects::Obj;
use crate::families::cx::Bench;
use crate::families::cx::{handlers, Cx};
use super::pshell::*;
use crate::mem::{rd16, wr16};
use crate::objects::pools;
use crate::objects::Current;
use crate::player::tables::{player_data, word, Aligned, Read};

const SFX_HIT_WALL: crate::sound::Sound = crate::sound::CMD_122_222;
const SFX_LAND: crate::sound::Sound = crate::sound::CMD_123_223;

fn signed(v: u16, negate: bool) -> u16 {
    if negate { v.wrapping_neg() } else { v }
}

/// The shot is stopped by the terrain: terrains 5 to 8 without bit 6 of the attribute (D2).
fn stopped_by_terrain<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    // Original glitch: the leftovers of 0x24a8 (D3 = cell index) are re-read by a following handler (batch C:
    // `boss11_scene42_mort`, frame 4447)
    let t = crate::video::terrain_of(x.b, x.o);
    leftovers_terrain(x, &t);
    let terrain = t.number as u32;
    (terrain & 0x3f).wrapping_sub(5) as u16 <= 3 && terrain & 0x40 == 0
}

/// Final blinking: the object is hidden every other frame (parity of counter `+0x50`).
fn blink_by_timer<R: Bench>(x: &mut Cx<'_, R>) {
    x.set_bit(Obj::FLAGS, 1, true);
    let t = x.u8(Shot::OWNER);
    if t & 1 != 0 {
        x.set_bit(Obj::FLAGS, 1, false);
    }
}

/// Type 0 (0x51c64): hit marker following its bearer (`+0x5c`).
mod marker {
    use super::*;

    /// Reaction of the carrier when the marker was placed (h).
    const REACTION_SEEN: u32 = 0x42;
    /// Carrier followed by the marker (16-bit pointer).
    const CARRIER: u32 = 0x5c;

    const ANIMS: u32 = 0x15ad38;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.state(0x51c72) {
            0 => {
                x.add8(Obj::STATE, 2);
                let carrier = x.linked(CARRIER);
                x.set8(f::BOX_SET, 6);
                let kind = carrier.u8(x.b, Obj::KIND) as u32;
                x.copy_pos(carrier);
                x.anim(ANIMS, kind);
                x.set32(Obj::FRAME, 0);
                x.reinsert();
            }
            2 => {
                let carrier = x.linked(CARRIER);
                for off in [0x8, 0x10] {
                    let v = carrier.u16(x.b, off);
                    x.set16(off, v);
                }
                let id = carrier.u8(x.b, Enemy::REACTION);
                if id != x.u8(REACTION_SEEN) {
                    return x.free_pshl();
                }
                let z = carrier.u16(x.b, Obj::Z);
                x.set16(Obj::Z, z);
                if x.u8(Shot::SUB_STATE) == 0 {
                    if (z as i16) > 0 {
                        x.add8(Shot::SUB_STATE, 2);
                    }
                    x.reinsert();
                } else if z == 0 {
                    x.free_pshl();
                } else {
                    x.reinsert();
                }
            }
            _ => x.free_pshl(),
        }
    }
}

/// Types 1 and 9 (0x51da6): thrown-object shot (kind 1: set 0x15ae8a, kind 9: set 0x15c552) that flies in a straight
/// line, bounces on impact, falls back then blinks.
mod thrown {
    use super::*;

    const ANIMS_A: u32 = 0x15ae8a;
    const ANIMS_B: u32 = 0x15c552;
    const SFX_START: crate::sound::Sound = crate::sound::CMD_11C_21C;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.state(0x51db4) {
            0 => {
                x.add8(Obj::STATE, 2);
                x.set8(f::BOX_SET, 6);
                let owner = x.linked(f::OWNER);
                for off in [0x8, 0xc] {
                    let v = owner.u16(x.b, off);
                    x.set16(off, v);
                }
                let y = owner.u32(x.b, Obj::Y).wrapping_sub(1);
                x.set32(Obj::Y, y);
                let row = (owner.u8(x.b, Player::CHARACTER) as u32) << 2 & 0xffff;
                let dx = player_data().thrown_offsets.word_at(x.b, row);
                let dz = player_data().thrown_offsets.word_at(x.b, row + 2);
                *x.c.d2() = dz as u32;
                let dx = signed(dx, owner.u8(x.b, f::FLIP) == 0);
                x.add16(Obj::X, dx);
                x.add16(Obj::Z, dz);
                pose(x);
                crate::sound::play(x.b, SFX_START, x.o);
                x.reinsert();
            }
            2 => {
                if x.u8(f::HIT_FLAGS) & 2 != 0 {
                    crate::sound::play(x.b, SFX_HIT_WALL, x.o);
                    return x.free_pshl();
                }
                if !stopped_by_terrain(x) {
                    x.move_xz();
                    return x.cull_offscreen();
                }
                x.add8(Obj::STATE, 2);
                x.set8(Obj::ACTIVE, 2);
                x.set8(Shot::OWNER, 2);
                x.set16(Obj::VZ, 0x300);
                x.set16(Obj::AZ, -0x30i16 as u16);
                x.set16(Obj::VX, 0x100);
                if x.u8(Obj::SPEED_INDEX) == 2 {
                    x.set16(Obj::VX, 0x100u16.wrapping_neg());
                }
                pose(x);
                crate::sound::play(x.b, SFX_LAND, x.o);
            }
            4 => {
                if x.u8(Shot::SUB_STATE) == 0 {
                    x.add8(Shot::OWNER, 0xff);
                    if x.u8(Shot::OWNER) == 0 {
                        x.set8(Shot::OWNER, 2);
                        let step: u8 = if (x.u16(Obj::VX) as i16) < 0 { 0xff } else { 1 };
                        let angle = x.u8(Obj::SPEED_INDEX).wrapping_add(step);
                        x.set8(Obj::SPEED_INDEX, angle);
                        x.anim(ANIMS_A, (angle & 7) as u32);
                    }
                    x.move_xz();
                    if (x.u16(Obj::Z) as i16) >= 0 {
                        return;
                    }
                    x.add8(Shot::SUB_STATE, 2);
                    x.set8(Shot::OWNER, 0x3c);
                    x.set8(Obj::SPEED_INDEX, 2);
                    if (x.u16(Obj::VX) as i16) < 0 {
                        x.set8(Obj::SPEED_INDEX, 6);
                    }
                    pose(x);
                } else {
                    x.add8(Shot::OWNER, 0xff);
                    if x.u8(Shot::OWNER) == 0 {
                        x.add8(Obj::STATE, 2);
                    }
                    blink_by_timer(x);
                }
            }
            _ => x.free_pshl(),
        }
    }

    /// Animation from angle `+0x2d` (`0x51f48`).
    fn pose<R: Bench>(x: &mut Cx<'_, R>) {
        let angle = (x.u8(Obj::SPEED_INDEX) & 7) as u32;
        let set = if x.u8(Obj::KIND) == 1 { ANIMS_A } else { ANIMS_B };
        x.anim_start(set, angle as u16);
    }
}

/// Types 2, 10 and 11 (0x5204e): shot thrown in a straight line (kinds 2, 0xa, 0xb: sets 0x15af1c, 0x15c5e4, 0x15c62a),
/// which crashes to the ground, rises a little then blinks.
mod flung {
    use super::*;

    const SFX_START: crate::sound::Sound = crate::sound::CMD_11B_21B;

    fn set<R: Bench>(x: &mut Cx<'_, R>) -> u32 {
        match x.u8(Obj::KIND) {
            2 => 0x15af1c,
            0xa => 0x15c5e4,
            _ => 0x15c62a,
        }
    }

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.state(0x5205c) {
            0 => {
                x.add8(Obj::STATE, 2);
                x.set8(f::BOX_SET, 6);
                let owner = x.linked(f::OWNER);
                for off in [0x8, 0xc] {
                    let v = owner.u16(x.b, off);
                    x.set16(off, v);
                }
                let y = owner.u32(x.b, Obj::Y).wrapping_sub(1);
                x.set32(Obj::Y, y);
                let mut row = (owner.u8(x.b, Player::CHARACTER) >> 1) as u32;
                if row == 2 && x.u16(Obj::Z) != 0 {
                    row += 4;
                }
                let row = row << 2;
                let dx = player_data().flung_offsets.word_at(x.b, row);
                let dz = player_data().flung_offsets.word_at(x.b, row + 2);
                *x.c.d2() = dz as u32;
                let dx = signed(dx, owner.u8(x.b, f::FLIP) == 0);
                x.add16(Obj::X, dx);
                x.add16(Obj::Z, dz);
                let a = set(x);
                x.anim(a, 0);
                crate::sound::play(x.b, SFX_START, x.o);
                x.reinsert();
            }
            2 => {
                if x.u8(f::HIT_FLAGS) & 2 != 0 {
                    crate::sound::play(x.b, SFX_HIT_WALL, x.o);
                    return x.free_pshl();
                }
                if !stopped_by_terrain(x) {
                    x.anim_step();
                    x.move_x();
                    return x.cull_offscreen();
                }
                x.add8(Obj::STATE, 2);
                x.set8(Obj::ACTIVE, 2);
                x.set8(Shot::OWNER, 4);
                let a = set(x);
                x.anim(a, 1);
                crate::sound::play(x.b, SFX_LAND, x.o);
            }
            4 => {
                match x.u8(Shot::SUB_STATE) {
                    0 => {
                        x.anim_step();
                        x.add8(Shot::OWNER, 0xff);
                        if x.u8(Shot::OWNER) == 0 {
                            x.add8(Shot::SUB_STATE, 2);
                            x.set16(Obj::VZ, 0);
                            x.set16(Obj::AZ, -0x20i16 as u16);
                        }
                    }
                    2 => {
                        x.move_z();
                        if (x.u16(Obj::Z) as i16) < 0 {
                            x.add8(Shot::SUB_STATE, 2);
                            x.set8(Shot::OWNER, 0x3c);
                        }
                    }
                    _ => {
                        x.add8(Shot::OWNER, 0xff);
                        if x.u8(Shot::OWNER) == 0 {
                            x.add8(Obj::STATE, 2);
                        }
                        blink_by_timer(x);
                    }
                }
            }
            _ => x.free_pshl(),
        }
    }
}

/// Type 3 (0x52234): shot with long speed (long `+0x14` = 0x1800000), which falls back, then stops and burns out (30 +
/// 80 frames).
mod arc {
    use super::*;

    /// Countdown of the arc (h).
    const TIMER: u32 = 0x5c;

    const ANIMS: u32 = 0x15af62;
    const SFX_BURST: crate::sound::Sound = crate::sound::CMD_125_225;
    const SFX_MID: crate::sound::Sound = crate::sound::CMD_12E_22E;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.state(0x52242) {
            0 => {
                x.add8(Obj::STATE, 2);
                let owner = x.linked(f::OWNER);
                let flip = owner.u8(x.b, f::FLIP);
                x.set8(f::FLIP, flip);
                x.copy_pos(owner);
                x.add16(Obj::Y, 0xffff);
                x.set32(Obj::VX, 0x180_0000);
                x.set16(Obj::VZ, 0x300);
                x.set16(Obj::AZ, -0x38i16 as u16);
                let row = (owner.u8(x.b, Player::CHARACTER) as u32) << 2 & 0xffff;
                let dx = player_data().arc_offsets.word_at(x.b, row);
                let dz = player_data().arc_offsets.word_at(x.b, row + 2);
                *x.c.d2() = dz as u32;
                let mut dx = dx;
                if x.u8(f::FLIP) == 0 {
                    dx = dx.wrapping_neg();
                    let vx = x.u16(Obj::VX);
                    x.set16(Obj::VX, vx.wrapping_neg());
                }
                x.add16(Obj::X, dx);
                x.add16(Obj::Z, dz);
                x.anim(ANIMS, 0);
                x.reinsert();
                active(x);
            }
            2 => active(x),
            _ => x.free_pshl(),
        }
    }

    fn active<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(Shot::SUB_STATE) {
            0 => {
                x.move_xz();
                if (x.u16(Obj::Z) as i16) >= 0 {
                    return;
                }
                x.add8(Shot::SUB_STATE, 2);
                x.set32(Obj::Z, 0);
                x.set16(TIMER, 0x50);
                x.anim(ANIMS, 1);
                crate::sound::play(x.b, SFX_BURST, x.o);
            }
            2 => {
                if x.u16(TIMER) == 0x48 {
                    crate::sound::play(x.b, SFX_MID, x.o);
                }
                x.add16(TIMER, 0xffff);
                if x.u16(TIMER) == 0 {
                    x.add8(Shot::SUB_STATE, 2);
                    x.set16(TIMER, 0x50);
                    x.anim(ANIMS, 2);
                }
                x.anim_step();
            }
            _ => {
                x.anim_step();
                x.add16(TIMER, 0xffff);
                if x.u16(TIMER) == 0 {
                    x.add8(Obj::STATE, 2);
                }
                x.set_bit(Obj::FLAGS, 1, true);
                let t = x.u16(TIMER);
                if t & 1 != 0 {
                    x.set_bit(Obj::FLAGS, 1, false);
                }
            }
        }
    }
}

/// Type 4 (0x523c4): projectile that bounces on the terrain (axe, `+0x5c` bounces) then burns out blinking (`+0x5d`:
/// 0x20 frames).
mod bouncer {
    use super::*;

    /// Bounces left.
    const BOUNCES: u32 = 0x5c;
    /// Countdown of the burn-out blinking.
    const BURN_TIMER: u32 = 0x5d;
    /// Frames drawn in front (`Obj::ACTIVE` = 2) after a bounce (h).
    const FRONT_TIMER: u32 = 0x5e;

    const ANIMS: u32 = 0x15b0a2;
    const SFX_START: crate::sound::Sound = crate::sound::CMD_11D_21D;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.state(0x523d2) {
            0 => {
                x.add8(Obj::STATE, 2);
                x.set8(Shot::HIT_FLAGS, 0);
                x.set8(FRONT_TIMER, 0);
                x.set8(BOUNCES, 2);
                x.set8(f::BOX_SET, 6);
                let owner = x.linked(f::OWNER);
                let flip = owner.u8(x.b, f::FLIP);
                x.set8(f::FLIP, flip);
                x.copy_pos(owner);
                x.add16(Obj::Y, 0xffff);
                x.set32(Obj::VX, 0x200_0000);
                x.set16(Obj::VZ, 0x800);
                x.set16(Obj::AZ, -0x60i16 as u16);
                let row = (owner.u8(x.b, Player::CHARACTER) as u32) << 2 & 0xffff;
                let dx = player_data().bouncer_offsets.word_at(x.b, row);
                let dz = player_data().bouncer_offsets.word_at(x.b, row + 2);
                *x.c.d2() = dz as u32;
                let mut dx = dx;
                if x.u8(f::FLIP) == 0 {
                    dx = dx.wrapping_neg();
                    let vx = x.u16(Obj::VX);
                    x.set16(Obj::VX, vx.wrapping_neg());
                }
                x.add16(Obj::X, dx);
                x.add16(Obj::Z, dz);
                x.anim(ANIMS, 0);
                x.reinsert();
                active(x);
            }
            2 => active(x),
            _ => x.free_pshl(),
        }
    }

    fn active<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(Shot::SUB_STATE) != 0 {
            return burn(x);
        }
        x.set8(Obj::ACTIVE, 1);
        if x.u8(FRONT_TIMER) != 0 {
            x.add8(FRONT_TIMER, 0xff);
            x.set8(Obj::ACTIVE, 2);
        }
        x.anim_step();
        x.move_xz();
        let hit = x.u8(Shot::HIT_FLAGS) & 2 != 0;
        let mut bounce = hit;
        if !hit {
            let terrain = crate::video::terrain_of(x.b, x.o).number as u32;
            let ground = (terrain & 0x3f).wrapping_sub(5) as u16 <= 3;
            if !ground || terrain & 0x40 != 0 {
                if (x.u16(Obj::Z) as i16) > 0 {
                    return finish(x);
                }
                x.set16(Obj::Z, 0);
                bounce = true;
            } else {
                // against a wall: U-turn
                let flip = x.u8(f::FLIP) ^ 0x20;
                x.set8(f::FLIP, flip);
                let vx = x.u16(Obj::VX);
                x.set16(Obj::VX, vx.wrapping_neg());
            }
        }
        let _ = bounce;
        x.set16(Obj::VZ, 0x600);
        x.set16(Obj::AZ, -0x40i16 as u16);
        x.set8(Obj::ACTIVE, 2);
        x.set8(FRONT_TIMER, 8);
        x.set8(Shot::HIT_FLAGS, 0);
        x.add8(BOUNCES, 0xff);
        if x.u8(BOUNCES) == 0 {
            x.set8(Shot::SUB_STATE, 2);
            x.set8(Obj::ACTIVE, 2);
            x.set8(BURN_TIMER, 0x20);
        }
        finish(x);
    }

    fn finish<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(Shot::ANIM_EVENT) != 0 {
            x.set8(Shot::ANIM_EVENT, 0);
            crate::sound::play(x.b, SFX_START, x.o);
        }
    }

    /// End of life: blinks 0x20 frames, falls back then disappears.
    fn burn<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(BURN_TIMER, 0xff);
        if x.u8(BURN_TIMER) == 0 {
            return x.free_pshl();
        }
        x.set_bit(Obj::FLAGS, 1, true);
        let t = x.u8(BURN_TIMER);
        if t & 1 != 0 {
            x.set_bit(Obj::FLAGS, 1, false);
        }
        if (x.u16(Obj::Z) as i16) > 0 {
            x.move_xz();
            return;
        }
        x.anim(ANIMS, 1);
    }
}

/// Type 5 (0x5265a): burst explosion (variant 0: projectile that leaves in an arc and bursts; variant 1: flame placed
/// by the bursting, offset according to its rank `+0x2f`).
mod burst {
    use super::*;

    /// Delay before the shot starts (set by the creator).
    const DELAY: u32 = 0x5c;
    /// Countdown of the screen flash.
    const FLASH_TIMER: u32 = 0x5e;

    const ANIMS: u32 = 0x15b13c;
    const SFX_RIGHT: crate::sound::Sound = crate::sound::CMD_168_B;
    const SFX_LEFT: crate::sound::Sound = crate::sound::CMD_168_C;
    const SFX_FLAME: crate::sound::Sound = crate::sound::CMD_169;
    /// Counter of the flames alive (`A5-0x5fbc`, absolute address).
    const FLAMES: u32 = 0xff2044;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        let child = x.u8(Obj::VARIANT) != 0;
        match x.state(0x52668) {
            0 if !child => {
                x.add8(Obj::STATE, 2);
                x.set16(Obj::VX, 0x500);
                x.set16(Obj::AX, 0x40);
                x.set16(Obj::VZ, -0x500i16 as u16);
                x.set16(Obj::AZ, 0);
                if x.u8(f::FLIP) == 0 {
                    let v = x.u32(Obj::VX).wrapping_neg();
                    x.set32(Obj::VX, v);
                }
                x.anim(ANIMS, 0);
                if (x.u16(Obj::VX) as i16) < 0 {
                    crate::sound::play(x.b, SFX_LEFT, x.o);
                } else {
                    crate::sound::play(x.b, SFX_RIGHT, x.o);
                }
            }
            0 => flame_init(x),
            2 if !child => {
                x.move_xz();
                x.anim_step();
                if (x.u16(Obj::Z) as i16) < 0 {
                    x.add8(Obj::STATE, 2);
                    spawn_flames(x);
                }
                x.cull_offscreen();
                x.reinsert();
            }
            2 => {
                if x.u8(FLASH_TIMER) != 0 {
                    x.add8(FLASH_TIMER, 0xff);
                    if x.u8(FLASH_TIMER) == 0 {
                        vars::SCROLL_Y_PLANE1.set(x.b, 0);
                    } else if x.u8(FLASH_TIMER) == 2 {
                        crate::palette::set_flash_color(x.b, crate::palette::FLASH_COLOR_WHITE_VALUE);
                    }
                }
                if x.anim_step() {
                    x.add8(Obj::STATE, 2);
                }
            }
            _ => x.free_pshl(),
        }
    }

    fn flame_init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(DELAY, 0xff);
        if x.u8(DELAY) != 0 {
            return;
        }
        x.add8(Obj::STATE, 2);
        x.set8(f::BOX_SET, 6);
        let count = rd16(x.b, FLAMES) as u32;
        let table = player_data().burst_offset_tables.long_at(x.b, count << 2 & 0xffff);
        let k = (x.u8(Shot::SETTING) as u32) << 2 & 0xffff;
        let dx = word(x.b, table + k);
        let dy = word(x.b, table + k + 2);
        let dx = signed(dx, x.u8(f::FLIP) != 0);
        *x.c.d2() = dy as u32;
        x.add16(Obj::X, dx);
        x.add16(Obj::Y, dy);
        x.set8(FLASH_TIMER, 4);
        vars::SCROLL_Y_PLANE1.set(x.b, 0x100);
        crate::palette::set_flash_color(x.b, crate::palette::FLASH_COLOR_RED);
        crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::A, 0x20);
        x.anim(ANIMS, 1);
        crate::sound::play(x.b, SFX_FLAME, x.o);
        x.reinsert();
    }

    /// `0x525ce`: creates the flames (variant 1), one more every 4 steps of delay.
    fn spawn_flames<R: Bench>(x: &mut Cx<'_, R>) {
        wr16(x.b, FLAMES, 0);
        let owner = x.linked(f::OWNER);
        let player = owner.u8(x.b, Player::LEVEL) as u32;
        let n = player_data().burst_shot_counts.at8(x.b, player).wrapping_sub(1);
        let (mut rank, mut delay) = (0u8, 1u8);
        let mut remaining = n as u16;
        *x.c.d5() = n as u32;
        while let Some(o) = x.alloc(&pools::PSHL) {
            o.set_u8(x.b, Obj::ACTIVE, 1);
            o.set_u8(x.b, Obj::KIND, 5);
            o.set_u8(x.b, Obj::VARIANT, 1);
            let flip = x.u8(f::FLIP);
            o.set_u8(x.b, f::FLIP, flip);
            let owner_word = x.u16(f::OWNER);
            o.set_u16(x.b, f::OWNER, owner_word);
            let (p1, p2) = (x.u8(Shot::ATTACK_ID), x.u8(Shot::ATTACK_POWER));
            o.set_u8(x.b, Shot::ATTACK_ID, p1);
            o.set_u8(x.b, Shot::ATTACK_POWER, p2);
            let (px, py) = (x.u16(Obj::X), x.u16(Obj::Y));
            o.set_u16(x.b, Obj::X, px);
            o.set_u16(x.b, Obj::Y, py);
            o.set_u32(x.b, Obj::Z, 0);
            o.set_u8(x.b, Shot::SETTING, rank);
            o.set_u8(x.b, DELAY, delay);
            rank = rank.wrapping_add(1);
            delay = delay.wrapping_add(4);
            let count = rd16(x.b, FLAMES).wrapping_add(1);
            wr16(x.b, FLAMES, count);
            let done = remaining == 0;
            remaining = remaining.wrapping_sub(1);
            if done {
                break;
            }
        }
        *x.c.d3() = rank as u32;
        *x.c.d4() = delay as u32;
    }
}

handlers! {
    0x51c64 "pshl_0" marker::run,
    0x51da6 "pshl_1" thrown::run,
    0x5204e "pshl_2" flung::run,
    0x52234 "pshl_3" arc::run,
    0x523c4 "pshl_4" bouncer::run,
    0x5265a "pshl_5" burst::run,
}
