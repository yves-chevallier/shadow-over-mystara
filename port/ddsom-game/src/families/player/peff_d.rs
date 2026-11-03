//! Player effects (PEFF), fourth series: types 6 to 10 (beams: magic shot, impact, breath).
//! See `docs/families/joueur.md`.

use crate::families::player::records::Shot;
use crate::player::Player;
use crate::objects::Obj;
use crate::families::cx::Bench;
use crate::families::cx::{handlers, Cx};
use super::pshell::*;
use crate::objects::pools;
use crate::objects::Current;
use crate::player::tables::{player_data, Read};

/// Blinking phase.
const BLINK: u32 = 0x5c;

/// Sign: `neg.w` when the object's facing equals `flip_means_neg` (0: left, 1: right).
fn signed(v: u16, negate: bool) -> u16 {
    if negate { v.wrapping_neg() } else { v }
}

/// The player's move (`+7`) has changed since creation: the lifetime of the effect is over.
fn gesture_changed<R: Bench>(x: &mut Cx<'_, R>, field: u32) -> bool {
    let owner = x.linked(f::OWNER);
    let gesture = owner.u8(x.b, Player::STEP);
    gesture != x.u8(field)
}

/// Makes the object blink: hidden every other frame (`+0x5c` toggles). `hidden_when_zero` chooses the hidden phase.
fn flicker<R: Bench>(x: &mut Cx<'_, R>, hidden_when_zero: bool) {
    x.set_bit(Obj::FLAGS, 1, true);
    let v = x.u8(BLINK) ^ 1;
    x.set8(BLINK, v);
    if (v == 0) != hidden_when_zero {
        x.set_bit(Obj::FLAGS, 1, false);
    }
}

/// Type 6 (0x4b1b6): thrown beam (magic 17). Variant 0: ray placed in front of the caster as long as its move is 4;
/// variants 1 and 2: ends (according to byte `+0x15e` read *beyond* the object, as in the original).
mod beam {
    use super::*;

    /// Original glitch: `+0x15e` read on the effect, past its 0x60 bytes (the special kind is a field of the player record).
    const FOLLOWING_SPECIAL_KIND: u32 = 0x15e;

    const ANIMS: crate::data::RomAddr = crate::data::RomAddr(0x1b5ab8);
    const OFFSETS: u32 = 0x4b2dc;
    const SFX: crate::sound::Sound = crate::sound::CMD_170;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        let child = x.u8(Obj::VARIANT) != 0;
        match x.state(0x4b1c4) {
            0 if !child => init_root(x),
            0 => init_tip(x),
            2 if !child => {
                x.anim_step();
                let owner = x.linked(f::OWNER);
                if owner.u8(x.b, Player::STEP) == 4 {
                    x.add8(Obj::STATE, 2);
                }
                x.reinsert();
            }
            2 => {
                if x.u8(Shot::ANIM_EVENT) != 0 {
                    x.set8(Shot::ANIM_EVENT, 0);
                    crate::families::small::shot_17(&mut crate::families::effects_creators::leftovers::Pure, x.b, x.o);
                }
                if x.anim_step() {
                    x.add8(Obj::STATE, 2);
                }
                x.reinsert();
            }
            _ => x.free_peff(),
        }
    }

    fn init_root<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(Obj::STATE, 2);
        follow_owner(x);
        let dx = signed(-2i16 as u16, x.u8(f::FLIP) != 0);
        x.add16(Obj::X, dx);
        x.add16(Obj::Z, 0x58);
        x.add16(Obj::Y, 0xffff);
        x.anim(ANIMS.0, 0);
    }

    fn init_tip<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(Obj::STATE, 2);
        if x.u8(FOLLOWING_SPECIAL_KIND) == 0x17 {
            let owner = follow_owner(x);
            let _ = owner;
            let dx = signed(-0x30i16 as u16, x.u8(f::FLIP) != 0);
            *x.c.d2() = 0xffff_0000;
            x.add16(Obj::X, dx);
            x.add16(Obj::Z, 0x3a);
            x.add32(Obj::Y, 0xffff_0000);
        } else {
            let owner = follow_owner(x);
            let row = ((owner.u8(x.b, Player::CHARACTER) >> 1) as u32) << 2;
            let (mut dx, dz) = offset(x, OFFSETS, row);
            dx = signed(dx, x.u8(f::FLIP) != 0);
            *x.c.d3() = 0xffff_0000;
            x.add16(Obj::X, dx);
            x.add16(Obj::Z, dz);
            x.add32(Obj::Y, 0xffff_0000);
        }
        x.anim(ANIMS.0, 1);
        crate::sound::play(x.b, SFX, x.o);
    }
}

/// Type 7 (0x4b3b4): end-of-ray spark, two of variant 0 (`+0x2f` = 0, 1) and one of variant 1 that waits for the end of
/// its animation.
mod spark {
    use super::*;

    const ANIMS: crate::data::RomAddr = crate::data::RomAddr(0x1b5bc2);
    const OFFSETS_A: u32 = 0x4b43c;
    const OFFSETS_B: u32 = 0x4b4ae;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        let child = x.u8(Obj::VARIANT) != 0;
        match x.state(0x4b3c2) {
            0 if !child => init_a(x),
            0 => init_b(x),
            2 if !child => {
                x.anim_step();
                flicker(x, false);
                if gesture_changed(x, 0x5d) {
                    x.add8(Obj::STATE, 2);
                }
                x.reinsert();
            }
            2 => {
                if x.anim_step() {
                    x.add8(Obj::STATE, 2);
                }
                x.reinsert();
            }
            _ => x.free_peff(),
        }
    }

    fn init_a<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(Obj::STATE, 2);
        let owner = follow_owner(x);
        let row = ((owner.u8(x.b, Player::CHARACTER) >> 1) as u32) << 2;
        let (dx, dz) = offset(x, OFFSETS_A, row);
        let depth: u32 = if x.u8(Shot::SETTING) == 0 { 0xfffe_8000 } else { 0xffff_0000 };
        *x.c.d3() = depth;
        let dx = signed(dx, x.u8(f::FLIP) != 0);
        x.add16(Obj::X, dx);
        x.add16(Obj::Z, dz);
        x.add32(Obj::Y, depth);
        let n = x.u8(Shot::SETTING) as u32;
        x.anim(ANIMS.0, n);
    }

    fn init_b<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(Obj::STATE, 2);
        let owner = x.linked(f::OWNER);
        let flip = owner.u8(x.b, f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
        x.copy_pos(owner);
        let row = ((owner.u8(x.b, Player::CHARACTER) >> 1) as u32) << 2;
        let (dx, dz) = offset(x, OFFSETS_B, row);
        let dx = signed(dx, owner.u8(x.b, f::FLIP) != 0);
        x.add16(Obj::X, dx);
        x.add16(Obj::Z, dz);
        x.add16(Obj::Y, 1);
        x.anim(ANIMS.0, 2);
    }
}

/// Type 8 (0x4b530): 8-frame glow at the caster's mouth.
mod glow {
    use super::*;

    /// Countdown (h).
    const TIMER: u32 = 0x5c;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.state(0x4b53e) {
            0 => {
                x.add8(Obj::STATE, 2);
                x.set8(TIMER, 8);
                follow_owner(x);
                let dx = signed(-0x16i16 as u16, x.u8(f::FLIP) != 0);
                x.add16(Obj::X, dx);
                x.add16(Obj::Z, 0x66);
                x.add16(Obj::Y, 1);
                x.anim(0x1b5d8e, 0);
            }
            2 => {
                x.add8(TIMER, 0xff);
                if x.u8(TIMER) == 0 {
                    x.add8(Obj::STATE, 2);
                }
                x.reinsert();
            }
            _ => x.free_peff(),
        }
    }
}

/// Type 9 (0x4b61a): ray (variant 0, two copies `+0x2f` = 0, 1: the core of the ray) and its generator (variant 1) that
/// fires shots (PSHL 6) according to the player.
mod ray {
    use super::*;

    /// Parent ray (16-bit pointer).
    const PARENT: u32 = 0x5c;
    /// Shots fired (h).
    const SHOTS: u32 = 0x5d;
    /// Phase of the ray (h).
    const PHASE: u32 = 0x5e;

    const ANIMS: crate::data::RomAddr = crate::data::RomAddr(0x1b5db2);

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        let child = x.u8(Obj::VARIANT) != 0;
        match x.state(0x4b628) {
            0 if !child => init_core(x),
            0 => init_generator(x),
            2 if !child => {
                x.anim_step();
                flicker(x, true);
                if gesture_changed(x, 0x5f) {
                    x.add8(Obj::STATE, 2);
                }
                x.reinsert();
            }
            2 => {
                match x.u8(Shot::SUB_STATE) {
                    0 => fire(x),
                    _ => {
                        if x.u8(SHOTS) == 0 {
                            x.add8(Obj::STATE, 2);
                            let owner = x.linked(f::OWNER);
                            owner.set_u16(x.b, Player::ACTION_TIMER, 1);
                        }
                    }
                }
            }
            _ => x.free_peff(),
        }
    }

    fn init_core<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(Obj::STATE, 2);
        let owner = follow_owner(x);
        let (mut dx, mut dz) = (-4i16 as u16, 0x34u16);
        if owner.u8(x.b, Player::SPECIAL_KIND) != 0x27 {
            dx = -0x18i16 as u16;
            dz = 0x45;
        }
        let depth: u32 = if x.u8(Shot::SETTING) == 0 { 0xfffe_8000 } else { 0xffff_0000 };
        *x.c.d2() = depth;
        let dx = signed(dx, x.u8(f::FLIP) != 0);
        x.add16(Obj::X, dx);
        x.add16(Obj::Z, dz);
        x.add32(Obj::Y, depth);
        let n = x.u8(Shot::SETTING) as u32;
        x.anim(ANIMS.0, n);
    }

    fn init_generator<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(Obj::STATE, 2);
        follow_owner(x);
    }

    /// Fires the shots of the ray: one per value of the player's table (`+0x10c`).
    fn fire<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(Shot::SUB_STATE, 2);
        let owner = x.linked(f::OWNER);
        x.set8(SHOTS, 0);
        x.set8(PHASE, 0);
        let player = owner.u8(x.b, Player::LEVEL);
        let mut remaining = player_data().ray_shot_counts.at8(x.b, player as u32);
        *x.c.d4() = remaining as u32;
        let mut created = 0u8;
        while let Some(shot) = x.alloc(&pools::PSHL) {
            shot.set_u8(x.b, Obj::ACTIVE, 1);
            shot.set_u8(x.b, Obj::KIND, 6);
            shot.set_u16(x.b, f::OWNER, owner.0 as u16);
            shot.set_u16(x.b, PARENT, x.o.0 as u16);
            shot.set_u8(x.b, Obj::VARIANT, remaining);
            created += 1;
            if remaining == 0 {
                break;
            }
            remaining -= 1;
        }
        *x.c.d5() = created as u32;
        x.set8(SHOTS, created);
    }
}

/// Type 10 (0x4b7c8): impact of the ray (variant 0, two copies) and final dust (variant 1).
mod impact {
    use super::*;

    const ANIMS: crate::data::RomAddr = crate::data::RomAddr(0x1b5ecc);
    const OFFSETS: u32 = 0x4b83e;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        let child = x.u8(Obj::VARIANT) != 0;
        match x.state(0x4b7d6) {
            0 => init(x),
            2 if !child => {
                x.anim_step();
                flicker(x, true);
                if gesture_changed(x, 0x5d) {
                    x.add8(Obj::STATE, 2);
                }
                x.reinsert();
            }
            2 => {
                x.anim_step();
                let owner = x.linked(f::OWNER);
                if owner.u8(x.b, Player::ANIM_EVENT) != 0 {
                    x.add8(Obj::STATE, 2);
                    owner.set_u8(x.b, Player::ANIM_EVENT, 0);
                    let me = x.o;
                    crate::families::creators::shot_5(x.b, me);
                }
                x.reinsert();
            }
            _ => x.free_peff(),
        }
    }

    fn init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(Obj::STATE, 2);
        let owner = follow_owner(x);
        let mut row = ((owner.u8(x.b, Player::CHARACTER) >> 1) as u32) << 3;
        if x.u8(Obj::VARIANT) != 0 {
            row += 4;
        }
        let (dx, dz) = offset(x, OFFSETS, row);
        let dx = signed(dx, x.u8(f::FLIP) != 0);
        x.add16(Obj::X, dx);
        x.add16(Obj::Z, dz);
        x.add16(Obj::Y, 0xffff);
        let n = x.u8(Obj::VARIANT) as u32;
        x.anim(ANIMS.0, n);
    }
}

handlers! {
    0x4b1b6 "peff_6" beam::run,
    0x4b3b4 "peff_7" spark::run,
    0x4b530 "peff_8" glow::run,
    0x4b61a "peff_9" ray::run,
    0x4b7c8 "peff_10" impact::run,
}
