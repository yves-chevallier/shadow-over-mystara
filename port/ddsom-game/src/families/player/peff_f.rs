//! Player effects (PEFF), sixth series: types 15 (weapon trail), 16 (item bubble), 18 (rain of stones) and 19
//! (lightning). See `docs/families/joueur.md`.

use crate::families::cx::Bench;
use crate::families::cx::{handlers, Cx};
use super::pshell::*;
use crate::families::effects_creators::leftovers::OnBench;
use crate::objects::pools;
use crate::objects::Current;

fn signed(v: u16, negate: bool) -> u16 {
    if negate { v.wrapping_neg() } else { v }
}

/// Type 15 (0x4c5be): weapon trail (weapons 0x10 to 0x13, rank `+2f`). It follows the bearer at offsets read from a ROM
/// record (`+3`: pose number) and cycles colours for rank 2.
mod trail {
    use super::*;

    const ANIMS: u32 = 0x1b6eda;
    const POSES: u32 = 0x4c6de;
    const COLORS: u32 = 0x4c66e;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.state(0x4c5cc) {
            0 => {
                x.add8(0x4, 2);
                if x.u8(0x2f) == 2 {
                    x.set8(0x5e, 0);
                    x.set8(0x5d, 1);
                    x.set8(0x2a, 1);
                    x.set8(0x2b, 0x14);
                }
                let d3 = place(x);
                let rank = (x.u8(0x2f) as u32) << 3;
                let anim = (d3 as u32).wrapping_add(rank) & 0xffff;
                x.anim(ANIMS, anim);
                cycle(x);
                x.reinsert();
            }
            2 => {
                if x.u8(0x5c) != 0 {
                    place(x);
                }
                if x.anim_step() {
                    x.add8(0x4, 2);
                }
                cycle(x);
            }
            _ => x.free_peff(),
        }
    }

    /// `0x4c674`: places itself on the bearer; returns word D3 of the record (animation number).
    fn place<R: Bench>(x: &mut Cx<'_, R>) -> u16 {
        let owner = x.linked(f::OWNER);
        let flip = owner.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        x.copy_pos(owner);
        let player_row = (owner.u8(x.b, 0x3) as u32) << 2;
        let records = x.data32(POSES + player_row);
        let row = (x.u8(0x3) as u32) << 3;
        let dx = x.data16(records + row);
        let dz = x.data16(records + row + 2);
        let d3 = x.data16(records + row + 4);
        let d4 = x.data8(records + row + 6);
        let d5 = x.data8(records + row + 7);
        *x.c.d2() = dz as u32;
        *x.c.d3() = d3 as u32;
        *x.c.d4() = d4 as u32;
        *x.c.d5() = d5 as u32;
        if d5 != 0 {
            let f = x.u8(f::FLIP) ^ 0x20;
            x.set8(f::FLIP, f);
        }
        let dx = signed(dx, owner.u8(x.b, f::FLIP) != 0);
        x.add16(0x8, dx);
        x.add16(0xc, dz);
        x.add16(0x10, 0xffff);
        x.set8(0x5c, d4);
        d3
    }

    /// `0x4c63c`: the rank 2 trails change colour every 4 frames.
    fn cycle<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(0x2f) != 2 {
            return;
        }
        x.add8(0x5d, 0xff);
        if x.u8(0x5d) != 0 {
            return;
        }
        x.set8(0x5d, 4);
        x.add8(0x5e, 1);
        if x.u8(0x5e) > 5 {
            x.set8(0x5e, 0);
        }
        let k = x.u8(0x5e) as u32;
        let c = x.rom8(COLORS + k);
        x.set8(0x2b, c);
    }
}

/// Type 16 (0x4c948): item bubble above a player (set 0x1b758c), which stays there `+0x58` frames.
mod bubble {
    use super::*;

    const ANIMS: u32 = 0x1b758c;
    const OFFSETS: u32 = 0x4c9e0;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.state(0x4c956) {
            0 => {
                x.add8(0x4, 2);
                x.set16(0x58, 0x12c);
                let owner = x.linked(f::OWNER);
                let kind = owner.u8(x.b, 0x2) as u32;
                x.anim(ANIMS, kind);
            }
            2 => {
                x.add16(0x58, 0xffff);
                if x.u16(0x58) == 0 {
                    x.add8(0x4, 2);
                }
                let owner = x.linked(f::OWNER);
                x.copy_pos(owner);
                x.set16(0x12, 0);
                owner.set_u16(x.b, 0x12, 0x8000);
                let row = ((owner.u8(x.b, 0x3) >> 1) as u32) << 2;
                let mut dx = x.rom16(OFFSETS + row);
                let mut dz = x.rom16(OFFSETS + row + 2);
                if x.a5_u8(0x59) != 0 {
                    dz = dz.wrapping_sub(0x30);
                }
                dx = signed(dx, owner.u8(x.b, f::FLIP) == 0);
                *x.c.d2() = dz as u32;
                x.add16(0x8, dx);
                x.add16(0xc, dz);
                x.anim_step();
                x.reinsert();
            }
            _ => x.free_peff(),
        }
    }
}

/// Type 18 (0x4ccca): cloud of the rain of stones (cleric's magic 18): placed at the top right of the screen with a
/// carpet of stones (PSHL 17), it rises then comes back down at the end of the prayer.
mod storm {
    use super::*;

    const ANIMS: u32 = 0x1ba964;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.state(0x4ccd8) {
            0 => {
                x.add8(0x4, 2);
                x.set8(0x58, 0x60);
                let (cx, cy) = if x.a5_u8(a5::USE_CAM2) != 0 {
                    (x.a5_u16(a5::CAM2_X), x.a5_u16(a5::CAM2_Y))
                } else {
                    (x.a5_u16(a5::CAM_X), x.a5_u16(a5::CAM_Y))
                };
                x.set16(0x8, cx.wrapping_add(0xc0));
                x.set16(0xc, cy.wrapping_add(0x100));
                x.anim(ANIMS, 0);
                let me = x.o;
                crate::families::small::pshl_stones(&mut OnBench(&mut x.c), x.b, me);
                x.reinsert();
            }
            2 => {
                match x.u8(0x5) {
                    0 => {
                        x.add8(0x58, 0xff);
                        if x.u8(0x58) == 0 {
                            x.add8(0x5, 2);
                        }
                        let t = x.u8(0x58) & 1;
                        if t != 0 {
                            x.add16(0xc, 0xffff);
                        }
                    }
                    2 => {
                        let owner = x.linked(f::OWNER);
                        if owner.u8(x.b, 0x7) == 6 {
                            x.add8(0x5, 2);
                            x.set8(0x58, 0x10);
                        }
                    }
                    _ => {
                        x.add16(0xc, 4);
                        x.add8(0x58, 0xff);
                        if x.u8(0x58) == 0 {
                            x.add8(0x4, 2);
                        }
                    }
                }
                x.anim_step();
            }
            _ => x.free_peff(),
        }
    }
}

/// Type 19 (0x4ce0c): lightning (elf's magic 19): the body (variant 0) and four corner sparks (variant 1, rank `+2f`)
/// that blink, light up the screen and repeat `+0x5a` times.
mod flash {
    use super::*;

    const ANIMS: u32 = 0x1baa5c;
    const CORNERS: u32 = 0x4ceb2;
    const FLIPS: u32 = 0x4ceae;
    const DELAYS: u32 = 0x4cec2;
    const SFX_BODY: u32 = 0xe981e;
    const SFX_CORNER: u32 = 0xe9888;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        let child = x.u8(0x3) != 0;
        match x.state(0x4ce1a) {
            0 if !child => {
                x.add8(0x4, 2);
                x.anim(ANIMS, 0);
                x.reinsert();
                crate::sound::play(x.b, SFX_BODY, x.o);
            }
            0 => init_corner(x),
            2 if !child => body(x),
            2 => corner(x),
            _ => x.free_peff(),
        }
    }

    fn init_corner<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        x.set_bit(0x1, 1, true);
        let (cx, cy) = if x.a5_u8(a5::USE_CAM2) != 0 {
            (x.a5_u16(a5::CAM2_X), x.a5_u16(a5::CAM2_Y))
        } else {
            (x.a5_u16(a5::CAM_X), x.a5_u16(a5::CAM_Y))
        };
        x.set16(0x8, cx);
        x.set16(0xc, cy);
        let k = x.u8(0x2f) as u32;
        *x.c.d3() = k;
        let row = k << 2;
        let dx = x.rom16(CORNERS + row);
        let dz = x.rom16(CORNERS + row + 2);
        x.add16(0x8, dx);
        x.add16(0xc, dz);
        let flip = x.rom8(FLIPS + k);
        x.set8(f::FLIP, flip);
        let delay = x.rom16(DELAYS + 2 * k);
        x.set16(0x58, delay);
        let anim = if k > 1 { 2 } else { 1 };
        x.anim(ANIMS, anim);
    }

    fn body<R: Bench>(x: &mut Cx<'_, R>) {
        let owner = x.linked(f::OWNER);
        if x.u8(0x5d) != owner.u8(x.b, 0x7) {
            x.add8(0x4, 2);
        }
        if x.u8(0x33) != 0 {
            x.set8(0x33, 0);
            x.set8(0x5c, 1);
            x.add16(0x10, 2);
            spawn_corners(x);
        }
        if x.u8(0x5c) != 0 {
            x.add16(0x8, 2);
            x.add16(0xc, 1);
            if x.u16(0x58) & 1 == 0 {
                x.add16(0x8, 0xfffc);
                x.add16(0xc, 0xfffe);
            }
        }
        x.add16(0x58, 1);
        let t = x.u16(0x58);
        if t > 0x140 {
            x.set_bit(0x1, 1, true);
            if t & 1 == 0 {
                x.set_bit(0x1, 1, false);
            }
        }
        x.anim_step();
    }

    /// `0x4cfbc`: four corner sparks, then the screen flash.
    fn spawn_corners<R: Bench>(x: &mut Cx<'_, R>) {
        for rank in (0..4u8).rev() {
            let Some(o) = x.alloc(&pools::PEFF) else { break };
            o.set_u8(x.b, 0, 1);
            o.set_u8(x.b, 2, 0x13);
            o.set_u8(x.b, 3, 1);
            o.set_u8(x.b, 0x2f, rank);
            o.set_u8(x.b, 0x5a, 2);
            o.set_u16(x.b, f::OWNER, x.o.0 as u16);
        }
        crate::palette::set_flash_color(x.b, crate::palette::FLASH_COLOR_RED);
    }

    fn corner<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u16(0x58) != 0 {
            x.add16(0x58, 0xffff);
            if x.u16(0x58) != 0 {
                return;
            }
            x.set_bit(0x1, 1, false);
            x.set8(0x5b, 2);
            x.set_a5_u16(0x4ba, 0x100);
            crate::sound::play(x.b, SFX_CORNER, x.o);
        }
        if x.anim_step() {
            x.add8(0x5a, 0xff);
            if x.u8(0x5a) == 0 {
                x.add8(0x4, 2);
                return;
            }
            x.set16(0x58, 0x18);
            x.set_bit(0x1, 1, true);
            let anim = if x.u8(0x2f) > 1 { 2 } else { 1 };
            x.anim(ANIMS, anim);
        } else {
            if x.u8(0x5b) != 0 {
                x.add8(0x5b, 0xff);
                if x.u8(0x5b) == 0 {
                    x.set_a5_u16(0x4ba, 0);
                }
            }
            x.reinsert();
        }
    }
}

handlers! {
    0x4c5be "peff_15" trail::run,
    0x4c948 "peff_16" bubble::run,
    0x4ccca "peff_18" storm::run,
    0x4ce0c "peff_19" flash::run,
}
