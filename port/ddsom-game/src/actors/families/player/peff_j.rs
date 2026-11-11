//! Player effects (PEFF), tenth series: types 35 to 39, 46 and 47 (effects of the leap, the swap, the slide, the
//! dwarf's attack, shards of the monsters hit, rain of the selection screen). See `docs/families/joueur.md`.

use super::vars;
use crate::actors::families::player::records::Shot;
use crate::world::objects::player::Player;
use crate::world::objects::Obj;
use crate::actors::families::cx::Bench;
use crate::actors::families::cx::{handlers, Cx};
use super::pshell::*;
use crate::world::objects::Current;
use crate::actors::player::tables::{player_data, pc16, Aligned};

fn signed(v: u16, negate: bool) -> u16 {
    if negate { v.wrapping_neg() } else { v }
}

/// Type 35 (0x4fc1a): leap effect (kind 0x23, `player/spawn.rs::leap`): placed high in front of the player as long as
/// the latter is in action 0x16.
mod leap {
    use super::*;

    const ANIMS: crate::world::data::RomAddr = crate::world::data::RomAddr(0x1bc60a);

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.state(0x4fc28) {
            0 => {
                x.add8(Obj::STATE, 2);
                let owner = follow_owner(x);
                x.set16(Shot::Y_FRACTION, 0);
                x.set16(Shot::Y_FRACTION, 0x8000);
                let (mut dx, dz) = (-0x62i16 as u16, 0x82u16);
                if owner.u8(x.b, Player::CHARACTER) > 2 {
                    dx = -0x49i16 as u16;
                }
                if x.u8(Obj::VARIANT) != 0 {
                    dx = -0x62i16 as u16;
                }
                let dx = signed(dx, x.u8(f::FLIP) != 0);
                x.add16(Obj::X, dx);
                x.add16(Obj::Z, dz);
                let n = x.u8(Obj::VARIANT) as u32;
                x.anim(ANIMS.0, n);
                x.reinsert();
            }
            2 => {
                let owner = x.linked(f::OWNER);
                if owner.u8(x.b, Player::SUB) != 0 || owner.u16(x.b, Player::HP) == 0 || owner.u8(x.b, Player::SUBACTION) != 0x16 {
                    return x.free_peff();
                }
                if vars::PAUSE.get(x.b) == 0 && owner.u8(x.b, Player::HITSTOP) == 0 {
                    x.copy_pos(owner);
                    x.set16(Shot::Y_FRACTION, 0);
                    x.set16(Shot::Y_FRACTION, 0x8000);
                    let dx = signed(-0x62i16 as u16, x.u8(f::FLIP) != 0);
                    x.add16(Obj::X, dx);
                    x.add16(Obj::Z, 0x82);
                    if x.anim_step() {
                        x.add8(Obj::STATE, 2);
                    }
                }
                x.reinsert();
            }
            _ => x.free_peff(),
        }
    }
}

/// Type 36 (0x4fd4a): equipment swap (kind 0x24): a spark placed in front of the player.
mod swap {
    use super::*;

    const ANIMS: crate::world::data::RomAddr = crate::world::data::RomAddr(0x1bc6b0);
    const OFFSETS_A: u32 = 0x4fdd4;
    const OFFSETS_B: u32 = 0x4fdec;
    const SFX: crate::services::sound::Sound = crate::services::sound::VOICE_BY_D4;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.state(0x4fd58) {
            0 => {
                x.add8(Obj::STATE, 2);
                let owner = follow_owner(x);
                x.add16(Obj::Y, 0xffff);
                let row = ((owner.u8(x.b, Player::CHARACTER) >> 1) as u32) << 2;
                let table = if x.u8(Obj::VARIANT) != 0 { OFFSETS_B } else { OFFSETS_A };
                let dx = pc16(x.b, table + row);
                let dz = pc16(x.b, table + row + 2);
                *x.c.d2() = dz as u32;
                let dx = signed(dx, x.u8(f::FLIP) != 0);
                x.add16(Obj::X, dx);
                x.add16(Obj::Z, dz);
                x.anim(ANIMS.0, 0);
                let voice = owner.u8(x.b, Player::WEAPON_A) as u32;
                crate::services::sound::play_with(x.b, SFX, crate::services::sound::Source { object: Some(x.o), a1: None, d4: Some(voice) });
                x.reinsert();
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

/// Type 37 (0x4fe38): slide dust (kind 0x25): follows the player as long as it slides (action 2, weapon `+0x6a`
/// unchanged).
mod dash {
    use super::*;

    /// Strike kind of the player when the effect was created.
    const SUBACTION: u32 = 0x5c;

    const ANIMS: crate::world::data::RomAddr = crate::world::data::RomAddr(0x1bc6f4);

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.state(0x4fe46) {
            0 => {
                x.add8(Obj::STATE, 2);
                pose(x);
                x.anim(ANIMS.0, 0);
                x.reinsert();
            }
            2 => {
                let owner = x.linked(f::OWNER);
                if owner.u8(x.b, Player::SUB) != 0
                    || owner.u8(x.b, Player::STEP) != 2
                    || owner.u16(x.b, Player::HP) == 0
                    || owner.u8(x.b, Player::SUBACTION) != x.u8(SUBACTION)
                    || owner.u8(x.b, Player::POWER_10F) != 0
                {
                    return x.free_peff();
                }
                pose(x);
                if vars::PAUSE.get(x.b) == 0 && owner.u8(x.b, Player::HITSTOP) == 0 {
                    x.anim_step();
                }
                x.reinsert();
            }
            _ => x.free_peff(),
        }
    }

    fn pose<R: Bench>(x: &mut Cx<'_, R>) {
        follow_owner(x);
        x.set16(Shot::Y_FRACTION, 0);
        x.set16(Shot::Y_FRACTION, 0x8000);
        let dx = signed(-0x2a_i16 as u16, x.u8(f::FLIP) != 0);
        x.add16(Obj::X, dx);
        x.add16(Obj::Z, 0xc);
    }
}

/// Type 38 (0x4ff1e): hit shard (kind 0x26): 0x18 frames in front of the player, as long as its action lasts.
mod chip {
    use super::*;

    const ANIMS: crate::world::data::RomAddr = crate::world::data::RomAddr(0x1bc77c);

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.state(0x4ff2c) {
            0 => {
                x.add8(Obj::STATE, 2);
                x.set16(Obj::VX, 0x18);
                let owner = follow_owner(x);
                x.set16(Shot::Y_FRACTION, 0);
                owner.set_u16(x.b, Player::Y_FRACTION, 0x8000);
                let row = ((x.u8(Obj::VARIANT) >> 1) as u32) << 2;
                let dx = player_data().chip_offsets.word_at(x.b, row);
                let dz = player_data().chip_offsets.word_at(x.b, row + 2);
                *x.c.d2() = dz as u32;
                let dx = signed(dx, owner.u8(x.b, f::FLIP) == 0);
                x.add16(Obj::X, dx);
                x.add16(Obj::Z, dz);
                x.anim(ANIMS.0, 0);
                x.reinsert();
            }
            2 => {
                let owner = x.linked(f::OWNER);
                if owner.u8(x.b, Player::SUB) == 0 {
                    return x.free_peff();
                }
                x.add16(Obj::VX, 0xffff);
                if x.u16(Obj::VX) == 0 {
                    x.add8(Obj::STATE, 2);
                }
            }
            _ => x.free_peff(),
        }
    }
}

/// Type 39 (0x4ffd0): shard of a monster hit (kind 0x27, created by `0x5009e`): a first copy waits, the second (variant
/// 1) resumes the animation of monster `+0x42`; they blink for 0xe frames.
mod chunk {
    use super::*;

    /// Player whose hand holds the chunk (16-bit pointer) (h).
    const HOST: u32 = 0x42;


    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.state(0x4ffea) {
            0 => {
                if x.u8(Obj::VARIANT) == 0 {
                    x.add8(Obj::STATE, 2);
                    x.set16(Obj::ANIM_TIMER, 0x14);
                } else {
                    let host = x.linked(HOST);
                    x.add8(Obj::STATE, 2);
                    let class = host.u8(x.b, Player::WEAPON_B) as u32;
                    let set = player_data().overlay_animations.long_at(x.b, 4 * class);
                    let row = (host.u16(x.b, HOST) as u32) << 3 & 0xffff;
                    let d0 = crate::actors::families::effects::tables::tables().hand_offsets.word_at(x.b, row);
                    let d2 = crate::actors::families::effects::tables::tables().hand_offsets.word_at(x.b, row + 2);
                    let d3 = crate::actors::families::effects::tables::tables().hand_offsets.word_at(x.b, row + 4);
                    *x.c.d2() = d2 as u32;
                    *x.c.d3() = d3 as u32;
                    let dx = signed(d2, host.u8(x.b, f::FLIP) != 0);
                    x.add16(Obj::X, dx);
                    x.add16(Obj::Z, d3);
                    x.anim_start(set, d0);
                    x.set16(Obj::ANIM_TIMER, 0x14);
                }
            }
            2 => {
                if x.u8(Shot::SUB_STATE) == 0 {
                    x.add16(Obj::ANIM_TIMER, 0xffff);
                    if (x.u16(Obj::ANIM_TIMER) as i16) <= 0 {
                        x.add8(Shot::SUB_STATE, 2);
                        x.set16(Obj::ANIM_TIMER, 0xe);
                    }
                } else {
                    x.add16(Obj::ANIM_TIMER, 0xffff);
                    if (x.u16(Obj::ANIM_TIMER) as i16) <= 0 {
                        return x.free_peff();
                    }
                    let h = x.u8(Obj::FLAGS) ^ 2;
                    x.set8(Obj::FLAGS, h);
                }
            }
            _ => x.free_peff(),
        }
        if x.u8(Obj::ACTIVE) != 0 {
            x.reinsert();
        }
    }
}

/// Type 46 (0x51ad6): coin rain of the reward screen (kind 0x2e) placed on the camera, which blinks as it falls.
mod rain {
    use super::*;

    /// Delay before the drop (word).
    const DELAY: u32 = 0x58;
    /// Lifetime (word).
    const TIMER: u32 = 0x5a;

    const ANIMS: crate::world::data::RomAddr = crate::world::data::RomAddr(0x1be8b8);

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.state(0x51ae4) {
            0 => {
                x.add8(Obj::STATE, 2);
                x.set16(TIMER, 0x70);
                let (cx, cy) = (vars::CAM_X.get(x.b), vars::CAM_Y.get(x.b));
                x.set16(Obj::X, cx);
                x.set16(Obj::Y, cy);
                let row = (x.u8(Obj::VARIANT) as u32) << 3;
                let dx = player_data().rain_offsets.word_at(x.b, row);
                let dz = player_data().rain_offsets.word_at(x.b, row + 2);
                let delay = player_data().rain_offsets.word_at(x.b, row + 4);
                x.set16(Obj::Z, dz);
                x.set16(DELAY, delay);
                x.add16(Obj::X, dx);
                x.add16(Obj::Y, 0x100);
                x.anim(ANIMS.0, 1);
            }
            2 => {
                x.add16(TIMER, 0xffff);
                if x.u16(DELAY) != 0 {
                    x.add16(DELAY, 0xffff);
                    x.set_bit(Obj::FLAGS, 1, true);
                    return;
                }
                if x.anim_step() {
                    let r = x.rand_and(3) * 2;
                    let d = player_data().rain_delays.word_at(x.b, r);
                    x.set16(DELAY, d);
                    x.anim(ANIMS.0, 1);
                    if (x.u16(TIMER) as i16) < 0 {
                        x.add8(Obj::STATE, 2);
                    }
                }
                let h = x.u8(Obj::FLAGS) ^ 2;
                x.set8(Obj::FLAGS, h);
                x.reinsert();
            }
            _ => x.free_peff(),
        }
    }
}

/// Type 47 (0x51bcc): wait of 0x16 frames before cutting the lightning of the victory screen.
mod timer {
    use super::*;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        let state = x.state(0x51bda);
        if state == 0 {
            // initialization chains into state 2
            x.add8(Obj::STATE, 2);
            x.set16(Obj::VX, 0x15);
        }
        match state {
            0 | 2 => {
                x.add16(Obj::VX, 0xffff);
                if (x.u16(Obj::VX) as i16) >= 0 {
                    return;
                }
                if vars::SCROLL_Y_PLANE1.get(x.b) == 0x100 {
                    vars::SCROLL_Y_PLANE1.set(x.b, 0);
                }
                x.free_peff();
            }
            _ => x.free_peff(),
        }
    }
}

handlers! {
    PEFF_35 0x4fc1a "peff_35" leap::run,
    PEFF_36 0x4fd4a "peff_36" swap::run,
    PEFF_37 0x4fe38 "peff_37" dash::run,
    PEFF_38 0x4ff1e "peff_38" chip::run,
    PEFF_39 0x4ffd0 "peff_39" chunk::run,
    PEFF_46 0x51ad6 "peff_46" rain::run,
    PEFF_47 0x51bcc "peff_47" timer::run,
}
