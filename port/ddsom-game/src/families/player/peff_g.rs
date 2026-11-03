//! Player effects (PEFF), seventh series: types 21 to 23 (hit spark, end-of-prayer smoke, prayer glow). See
//! `docs/families/joueur.md`.

use crate::families::player::records::Shot;
use crate::player::Player;
use crate::objects::Obj;
use crate::families::cx::Bench;
use crate::families::cx::{handlers, Cx};
use super::pshell::*;
use crate::objects::pools;
use crate::objects::Current;
use crate::player::tables::{player_data, Aligned, Read};

fn signed(v: u16, negate: bool) -> u16 {
    if negate { v.wrapping_neg() } else { v }
}

/// Type 21 (0x4d2fe): hit spark (variants 0 and 1), placed in front of the player; variant 0 lives as long as its move
/// (`+0x5c`) lasts and blinks.
mod hit_spark {
    use super::*;

    /// Gesture of the player when the spark was created.
    const GESTURE: u32 = 0x5c;
    /// Blinking phase.
    const BLINK: u32 = 0x5d;

    const ANIMS: u32 = 0x1bb146;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        let child = x.u8(Obj::VARIANT) != 0;
        match x.u8(Obj::STATE) {
            0 => {
                x.add8(Obj::STATE, 2);
                let owner = x.linked(f::OWNER);
                let gesture = owner.u8(x.b, Player::STEP);
                x.set8(GESTURE, gesture);
                let flip = owner.u8(x.b, f::FLIP);
                x.set8(f::FLIP, flip);
                x.copy_pos(owner);
                x.add16(Obj::Y, 0xffff);
                let (mut dx, mut dz) = (0x10u16, 0x22u16);
                if child {
                    (dx, dz) = (-0x18i16 as u16, 0x36);
                }
                let dx = signed(dx, x.u8(f::FLIP) != 0);
                x.add16(Obj::X, dx);
                x.add16(Obj::Z, dz);
                let n = x.u8(Obj::VARIANT) as u32;
                x.anim(ANIMS, n);
                x.reinsert();
            }
            2 if !child => {
                x.anim_step();
                x.set_bit(Obj::FLAGS, 1, true);
                let v = x.u8(BLINK) ^ 1;
                x.set8(BLINK, v);
                if v == 0 {
                    x.set_bit(Obj::FLAGS, 1, false);
                }
                let owner = x.linked(f::OWNER);
                let gesture = owner.u8(x.b, Player::STEP);
                if gesture != x.u8(GESTURE) {
                    x.add8(Obj::STATE, 2);
                }
            }
            2 => {
                if x.anim_step() {
                    x.add8(Obj::STATE, 2);
                }
            }
            _ => x.free_peff(),
        }
    }
}

/// Type 22 (0x4d3e2): the cleric's prayer (kind 0x16). Variant 0: nimbus that lasts `+0x168 - 0x1e` frames; variant 1:
/// rays (rank `+2f` 0 = countdown, 1 = rain of sparks); variant 2: final glow.
mod prayer {
    use super::*;

    /// Lifetime (word).
    const TIMER: u32 = 0x5c;

    const ANIMS: u32 = 0x1bb240;
    const KIND: u8 = 0x16;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        let variant = x.u8(Obj::VARIANT);
        match x.u8(Obj::STATE) {
            0 => match variant {
                0 => init_halo(x),
                1 => init_rays(x),
                _ => init_glow(x),
            },
            2 => match variant {
                0 => {
                    x.anim_step();
                    x.add16(TIMER, 0xffff);
                    if x.u16(TIMER) == 0 {
                        x.add8(Obj::STATE, 2);
                    }
                }
                1 => rays(x),
                _ => {
                    if x.anim_step() {
                        x.add8(Obj::STATE, 2);
                    }
                }
            },
            _ => x.free_peff(),
        }
    }

    fn init_halo<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(Obj::STATE, 2);
        let owner = x.linked(f::OWNER);
        let flip = owner.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        x.copy_pos(owner);
        x.add16(Obj::X, 0);
        x.add16(Obj::Z, 0x37);
        x.add16(Obj::Y, 0xffff);
        if owner.u8(x.b, Player::SPECIAL_KIND) == 0x24 {
            x.add16(Obj::Y, 2);
        }
        let class = owner.u8(x.b, Player::LEVEL) as u32;
        let life = player_data().prayer_durations.word_at(x.b, class * 2);
        x.set16(TIMER, life);
        x.anim(ANIMS, 0);
        x.reinsert();
    }

    fn init_rays<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(Shot::SETTING) == 0 {
            x.add8(Obj::STATE, 2);
            let owner = x.linked(f::OWNER);
            let life = owner.u16(x.b, Player::ACTION_TIMER).wrapping_sub(0x1e);
            x.set16(TIMER, life);
            return;
        }
        x.add8(Obj::STATE, 2);
        x.set16(TIMER, 0x28);
        let owner = x.linked(f::OWNER);
        let flip = owner.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        x.copy_pos(owner);
        x.add16(Obj::Z, 0x20);
        x.select_speed(&crate::objects::motion::SPEEDS_B, 0x43);
        x.move_by_table_xy();
        x.select_speed(&crate::objects::motion::SPEEDS_B, 0x21);
        x.anim(ANIMS, 1);
        x.reinsert();
    }

    fn init_glow<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(Obj::STATE, 2);
        let owner = x.linked(f::OWNER);
        let flip = owner.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        x.copy_pos(owner);
        x.add16(Obj::X, 0);
        x.add16(Obj::Z, 0x37);
        x.add16(Obj::Y, 0xffff);
        x.anim(ANIMS, 3);
        x.reinsert();
    }

    fn rays<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(Shot::SETTING) == 0 {
            x.add16(TIMER, 0xffff);
            if x.u16(TIMER) == 0 {
                x.add8(Obj::STATE, 2);
                return;
            }
            if x.u16(TIMER) & 7 == 0 {
                for rank in (0..5u32).rev() {
                    let Some(o) = x.alloc(&pools::PEFF) else { break };
                    o.set_u8(x.b, Obj::ACTIVE, 1);
                    o.set_u8(x.b, Obj::KIND, KIND);
                    o.set_u8(x.b, Obj::VARIANT, 1);
                    o.set_u8(x.b, Shot::SETTING, 1);
                    let owner = x.u16(f::OWNER);
                    o.set_u16(x.b, f::OWNER, owner);
                    let angle = player_data().prayer_angles.at8(x.b, rank);
                    o.set_u8(x.b, Obj::SPEED_INDEX, angle);
                }
            }
            return;
        }
        x.set_bit(Obj::FLAGS, 1, true);
        let d = x.u16(TIMER).wrapping_add(x.counter());
        if d & 1 != 0 {
            x.set_bit(Obj::FLAGS, 1, false);
        }
        if x.u8(Shot::SUB_STATE) == 0 {
            x.add16(TIMER, 0xffff);
            if x.u16(TIMER) == 0 {
                x.add8(Shot::SUB_STATE, 2);
                x.anim(ANIMS, 2);
            }
            x.anim_step();
            x.move_by_table_xy();
            x.reinsert();
        } else {
            if x.anim_step() {
                x.add8(Obj::STATE, 2);
            }
            x.move_by_table_xy();
            x.reinsert();
        }
    }
}

/// Type 23 (0x4d682): prayer glow (kind 0x17), stuck to the caster; variant 1 is higher.
mod glow {
    use super::*;

    /// Gesture of the player when the glow was created.
    const GESTURE: u32 = 0x5d;

    const ANIMS: u32 = 0x1bb402;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(Obj::STATE) {
            0 => {
                x.add8(Obj::STATE, 2);
                follow_owner(x);
                let (mut dx, mut dz) = (0x13u16, 0x2du16);
                if x.u8(Obj::VARIANT) != 0 {
                    (dx, dz) = (-0x69i16 as u16, 0x3a);
                }
                let dx = signed(dx, x.u8(f::FLIP) != 0);
                x.add16(Obj::X, dx);
                x.add16(Obj::Z, dz);
                x.add16(Obj::Y, 0xffff);
                let n = x.u8(Obj::VARIANT) as u32;
                x.anim(ANIMS, n);
                x.reinsert();
            }
            2 => {
                let owner = x.linked(f::OWNER);
                let gesture = owner.u8(x.b, Player::STEP);
                if gesture != x.u8(GESTURE) {
                    x.add8(Obj::STATE, 2);
                }
                x.anim_step();
            }
            _ => x.free_peff(),
        }
    }
}

handlers! {
    0x4d2fe "peff_21" hit_spark::run,
    0x4d3e2 "peff_22" prayer::run,
    0x4d682 "peff_23" glow::run,
}
