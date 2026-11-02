//! Player effects (PEFF), tenth series: types 35 to 39, 46 and 47 (effects of the leap, the swap, the slide, the
//! dwarf's attack, shards of the monsters hit, rain of the selection screen). See `docs/families/joueur.md`.

use crate::families::cx::Bench;
use crate::families::cx::{handlers, Cx};
use super::pshell::*;
use crate::objects::Current;

fn signed(v: u16, negate: bool) -> u16 {
    if negate { v.wrapping_neg() } else { v }
}

/// Type 35 (0x4fc1a): leap effect (kind 0x23, `player/spawn.rs::leap`): placed high in front of the player as long as
/// the latter is in action 0x16.
mod leap {
    use super::*;

    const ANIMS: u32 = 0x1bc60a;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.state(0x4fc28) {
            0 => {
                x.add8(0x4, 2);
                let owner = follow_owner(x);
                x.set16(0x12, 0);
                x.set16(0x12, 0x8000);
                let (mut dx, dz) = (-0x62i16 as u16, 0x82u16);
                if owner.u8(x.b, 0x3) > 2 {
                    dx = -0x49i16 as u16;
                }
                if x.u8(0x3) != 0 {
                    dx = -0x62i16 as u16;
                }
                let dx = signed(dx, x.u8(f::FLIP) != 0);
                x.add16(0x8, dx);
                x.add16(0xc, dz);
                let n = x.u8(0x3) as u32;
                x.anim(ANIMS, n);
                x.reinsert();
            }
            2 => {
                let owner = x.linked(f::OWNER);
                if owner.u8(x.b, 0x5) != 0 || owner.u16(x.b, 0x62) == 0 || owner.u8(x.b, 0x6a) != 0x16 {
                    return x.free_peff();
                }
                if x.a5_u8(0x3d) == 0 && owner.u8(x.b, 0x53) == 0 {
                    x.copy_pos(owner);
                    x.set16(0x12, 0);
                    x.set16(0x12, 0x8000);
                    let dx = signed(-0x62i16 as u16, x.u8(f::FLIP) != 0);
                    x.add16(0x8, dx);
                    x.add16(0xc, 0x82);
                    if x.anim_step() {
                        x.add8(0x4, 2);
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

    const ANIMS: u32 = 0x1bc6b0;
    const OFFSETS_A: u32 = 0x4fdd4;
    const OFFSETS_B: u32 = 0x4fdec;
    const SFX: u32 = 0xe7c20;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.state(0x4fd58) {
            0 => {
                x.add8(0x4, 2);
                let owner = follow_owner(x);
                x.add16(0x10, 0xffff);
                let row = ((owner.u8(x.b, 0x3) >> 1) as u32) << 2;
                let table = if x.u8(0x3) != 0 { OFFSETS_B } else { OFFSETS_A };
                let dx = x.rom16(table + row);
                let dz = x.rom16(table + row + 2);
                *x.c.d2() = dz as u32;
                let dx = signed(dx, x.u8(f::FLIP) != 0);
                x.add16(0x8, dx);
                x.add16(0xc, dz);
                x.anim(ANIMS, 0);
                let voice = owner.u8(x.b, 0x10d) as u32;
                crate::sound::play_with(x.b, SFX, crate::sound::Source { object: Some(x.o), a1: None, d4: Some(voice) });
                x.reinsert();
            }
            2 => {
                if x.anim_step() {
                    x.add8(0x4, 2);
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

    const ANIMS: u32 = 0x1bc6f4;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.state(0x4fe46) {
            0 => {
                x.add8(0x4, 2);
                pose(x);
                x.anim(ANIMS, 0);
                x.reinsert();
            }
            2 => {
                let owner = x.linked(f::OWNER);
                if owner.u8(x.b, 0x5) != 0
                    || owner.u8(x.b, 0x7) != 2
                    || owner.u16(x.b, 0x62) == 0
                    || owner.u8(x.b, 0x6a) != x.u8(0x5c)
                    || owner.u8(x.b, 0x10f) != 0
                {
                    return x.free_peff();
                }
                pose(x);
                if x.a5_u8(0x3d) == 0 && owner.u8(x.b, 0x53) == 0 {
                    x.anim_step();
                }
                x.reinsert();
            }
            _ => x.free_peff(),
        }
    }

    fn pose<R: Bench>(x: &mut Cx<'_, R>) {
        follow_owner(x);
        x.set16(0x12, 0);
        x.set16(0x12, 0x8000);
        let dx = signed(-0x2a_i16 as u16, x.u8(f::FLIP) != 0);
        x.add16(0x8, dx);
        x.add16(0xc, 0xc);
    }
}

/// Type 38 (0x4ff1e): hit shard (kind 0x26): 0x18 frames in front of the player, as long as its action lasts.
mod chip {
    use super::*;

    const ANIMS: u32 = 0x1bc77c;
    const OFFSETS: u32 = 0x4ff9a;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.state(0x4ff2c) {
            0 => {
                x.add8(0x4, 2);
                x.set16(0x14, 0x18);
                let owner = follow_owner(x);
                x.set16(0x12, 0);
                owner.set_u16(x.b, 0x12, 0x8000);
                let row = ((x.u8(0x3) >> 1) as u32) << 2;
                let dx = x.rom16(OFFSETS + row);
                let dz = x.rom16(OFFSETS + row + 2);
                *x.c.d2() = dz as u32;
                let dx = signed(dx, owner.u8(x.b, f::FLIP) == 0);
                x.add16(0x8, dx);
                x.add16(0xc, dz);
                x.anim(ANIMS, 0);
                x.reinsert();
            }
            2 => {
                let owner = x.linked(f::OWNER);
                if owner.u8(x.b, 0x5) == 0 {
                    return x.free_peff();
                }
                x.add16(0x14, 0xffff);
                if x.u16(0x14) == 0 {
                    x.add8(0x4, 2);
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

    const ANIM_SETS: u32 = 0x396e8;
    const FRAMES: u32 = 0x11ae86;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.state(0x4ffea) {
            0 => {
                if x.u8(0x3) == 0 {
                    x.add8(0x4, 2);
                    x.set16(0x30, 0x14);
                } else {
                    let host = x.linked(0x42);
                    x.add8(0x4, 2);
                    let class = host.u8(x.b, 0x10e) as u32;
                    let set = x.data32(ANIM_SETS + 4 * class);
                    let row = (host.u16(x.b, 0x42) as u32) << 3 & 0xffff;
                    let d0 = x.data16(FRAMES + row);
                    let d2 = x.data16(FRAMES + row + 2);
                    let d3 = x.data16(FRAMES + row + 4);
                    *x.c.d2() = d2 as u32;
                    *x.c.d3() = d3 as u32;
                    let dx = signed(d2, host.u8(x.b, f::FLIP) != 0);
                    x.add16(0x8, dx);
                    x.add16(0xc, d3);
                    x.anim_start(set, d0);
                    x.set16(0x30, 0x14);
                }
            }
            2 => {
                if x.u8(0x5) == 0 {
                    x.add16(0x30, 0xffff);
                    if (x.u16(0x30) as i16) <= 0 {
                        x.add8(0x5, 2);
                        x.set16(0x30, 0xe);
                    }
                } else {
                    x.add16(0x30, 0xffff);
                    if (x.u16(0x30) as i16) <= 0 {
                        return x.free_peff();
                    }
                    let h = x.u8(0x1) ^ 2;
                    x.set8(0x1, h);
                }
            }
            _ => x.free_peff(),
        }
        if x.u8(0) != 0 {
            x.reinsert();
        }
    }
}

/// Type 46 (0x51ad6): coin rain of the reward screen (kind 0x2e) placed on the camera, which blinks as it falls.
mod rain {
    use super::*;

    const ANIMS: u32 = 0x1be8b8;
    const OFFSETS: u32 = 0x51b44;
    const DELAYS: u32 = 0x51ba8;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.state(0x51ae4) {
            0 => {
                x.add8(0x4, 2);
                x.set16(0x5a, 0x70);
                let (cx, cy) = (x.a5_u16(a5::CAM_X), x.a5_u16(a5::CAM_Y));
                x.set16(0x8, cx);
                x.set16(0x10, cy);
                let row = (x.u8(0x3) as u32) << 3;
                let dx = x.rom16(OFFSETS + row);
                let dz = x.rom16(OFFSETS + row + 2);
                let delay = x.rom16(OFFSETS + row + 4);
                x.set16(0xc, dz);
                x.set16(0x58, delay);
                x.add16(0x8, dx);
                x.add16(0x10, 0x100);
                x.anim(ANIMS, 1);
            }
            2 => {
                x.add16(0x5a, 0xffff);
                if x.u16(0x58) != 0 {
                    x.add16(0x58, 0xffff);
                    x.set_bit(0x1, 1, true);
                    return;
                }
                if x.anim_step() {
                    let r = (x.rand_and(3) * 2) as u32;
                    let d = x.rom16(DELAYS + r);
                    x.set16(0x58, d);
                    x.anim(ANIMS, 1);
                    if (x.u16(0x5a) as i16) < 0 {
                        x.add8(0x4, 2);
                    }
                }
                let h = x.u8(0x1) ^ 2;
                x.set8(0x1, h);
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
            x.add8(0x4, 2);
            x.set16(0x14, 0x15);
        }
        match state {
            0 | 2 => {
                x.add16(0x14, 0xffff);
                if (x.u16(0x14) as i16) >= 0 {
                    return;
                }
                if x.a5_u16(0x4ba) == 0x100 {
                    x.set_a5_u16(0x4ba, 0);
                }
                x.free_peff();
            }
            _ => x.free_peff(),
        }
    }
}

handlers! {
    0x4fc1a "peff_35" #[without] leap::run,
    0x4fd4a "peff_36" #[without] swap::run,
    0x4fe38 "peff_37" #[without] dash::run,
    0x4ff1e "peff_38" #[without] chip::run,
    0x4ffd0 "peff_39" #[without] chunk::run,
    0x51ad6 "peff_46" #[without] rain::run,
    0x51bcc "peff_47" #[without] timer::run,
}
