//! Player shots (PSHL), third series: types 12 to 17 (objects thrown by the wizards and the cleric: jar, arrows, bursts
//! of flame). See `docs/families/joueur.md`.

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
use crate::player::tables::{player_data, byte, Read};

fn signed(v: u16, negate: bool) -> u16 {
    if negate { v.wrapping_neg() } else { v }
}

/// Counters of live objects (`A5-0x5fba`, `-0x5fb8`, `-0x5fb6`, `-0x5fb4`): absolute addresses.
const COUNTER_14: u32 = 0xff2046;
const COUNTER_15: u32 = 0xff2048;
const COUNTER_16: u32 = 0xff204a;

/// Decrements a RAM counter (`subq.w #1`).
fn decrement<R: Bench>(x: &mut Cx<'_, R>, address: u32) {
    let v = rd16(x.b, address).wrapping_sub(1);
    wr16(x.b, address, v);
}

/// Type 12 (0x530bc): jet of flames. Variant 0: projectile with long speed (like type 3) that falls back and creates
/// three bursts (variant 1); variant 1: burst that waits `+0x5c` frames then burns.
mod jet {
    use super::*;

    /// Delay before the jet starts (word).
    const DELAY: u32 = 0x5c;

    const ANIMS: u32 = 0x15c698;
    const SFX: crate::sound::Sound = crate::sound::CMD_13C_23C;
    const KIND: u8 = 0xc;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        let child = x.u8(Obj::VARIANT) != 0;
        match x.state(0x530ca) {
            0 if !child => {
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
                let dx = player_data().jet_offsets.at16(x.b, row);
                let dz = player_data().jet_offsets.at16(x.b, row + 2);
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
            0 => {
                x.add16(DELAY, 0xffff);
                if x.u16(DELAY) == 0 {
                    x.add8(Obj::STATE, 2);
                    x.anim(ANIMS, 1);
                    x.reinsert();
                    crate::sound::play(x.b, SFX, x.o);
                }
            }
            2 => active(x),
            _ => x.free_pshl(),
        }
    }

    fn active<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(Obj::VARIANT) != 0 {
            if x.anim_step() {
                x.add8(Obj::STATE, 2);
            }
            return;
        }
        x.move_xz();
        if (x.u16(Obj::Z) as i16) >= 0 {
            return;
        }
        x.add8(Obj::STATE, 2);
        let (mut offset, mut delay) = (0u16, 1u16);
        for _ in 0..3 {
            let Some(o) = x.alloc(&pools::PSHL) else { return };
            o.set_u8(x.b, Obj::ACTIVE, 1);
            o.set_u8(x.b, Obj::KIND, KIND);
            o.set_u8(x.b, Obj::VARIANT, 1);
            let (px, py) = (x.u16(Obj::X), x.u16(Obj::Y));
            o.set_u16(x.b, Obj::X, px.wrapping_add(offset));
            o.set_u16(x.b, Obj::Y, py);
            o.set_u8(x.b, f::POWER, 0x10);
            o.set_u8(x.b, f::POWER2, 0x10);
            let owner = x.u16(f::OWNER);
            o.set_u16(x.b, f::OWNER, owner);
            o.set_u16(x.b, DELAY, delay);
            offset = offset.wrapping_add(signed(0x28, x.u8(f::FLIP) == 0));
            delay += 8;
            *x.c.d4() = offset as u32;
            *x.c.d3() = delay as u32;
        }
    }
}

/// Type 13 (0x53290): thrown object consuming an inventory item (set 0x15b0a2); it falls back with the speed its
/// creator gave it.
mod item {
    use super::*;

    const ANIMS: u32 = 0x15b0a2;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.state(0x5329e) {
            0 => {
                x.add8(Obj::STATE, 2);
                let owner = x.linked(f::OWNER);
                let page = (owner.u8(x.b, Player::SPECIAL_COUNT) as u32) << 4 & 0xffff;
                let count = owner.0 + 0x120 + page + 8;
                let left = byte(x.b, count).wrapping_sub(1);
                crate::mem::wr8(x.b, count, left);
                if left == 0 {
                    crate::mem::wr8(x.b, owner.0 + 0x120 + page, 0x26);
                }
                x.copy_pos(owner);
                x.add16(Obj::Y, 0xffff);
                let row = (owner.u8(x.b, Player::CHARACTER) as u32) << 2 & 0xffff;
                let dx = player_data().item_offsets.at16(x.b, row);
                let dz = player_data().item_offsets.at16(x.b, row + 2);
                *x.c.d2() = dz as u32;
                let dx = signed(dx, owner.u8(x.b, f::FLIP) == 0);
                x.add16(Obj::X, dx);
                x.add16(Obj::Z, dz);
                x.anim(ANIMS, 0);
                x.reinsert();
            }
            2 => {
                x.anim_step();
                x.move_xz();
                if (x.u16(Obj::Z) as i16) < 0 {
                    x.add8(Obj::STATE, 2);
                    x.set32(Obj::Z, 0);
                }
                x.reinsert();
            }
            _ => x.free_pshl(),
        }
    }
}

/// Type 14 (0x53412): jet flame (counter `A5-0x5fba`) that waits `+0x5c` frames then burns.
mod flame {
    use super::*;

    /// Lifetime countdown (h).
    const TIMER: u32 = 0x5c;

    const ANIMS: u32 = 0x15c7a6;
    const SFX: crate::sound::Sound = crate::sound::CMD_14E_24E;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.state(0x53420) {
            0 => {
                x.add8(TIMER, 0xff);
                if x.u8(TIMER) != 0 {
                    return;
                }
                x.add8(Obj::STATE, 2);
                x.set8(f::BOX_SET, 6);
                x.set8(f::POWER, 6);
                x.set8(f::POWER2, 6);
                x.anim(ANIMS, 0);
                x.reinsert();
                crate::sound::play(x.b, SFX, x.o);
            }
            2 => {
                if x.anim_step() {
                    x.add8(Obj::STATE, 2);
                }
            }
            _ => {
                decrement(x, COUNTER_14);
                x.free_pshl();
            }
        }
    }
}

/// Type 15 (0x534d6): thrower projectile (counter `A5-0x5fb8`), which advances 4 pixels per frame for `+0x5c` frames.
mod volley {
    use super::*;

    /// Lifetime of the shot (by class).
    const LIFE: u32 = 0x5c;

    const ANIMS: u32 = 0x15c85a;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.state(0x534e4) {
            0 => {
                let owner = x.linked(f::OWNER);
                if owner.u8(x.b, Player::SUB) != 0 {
                    return die(x);
                }
                x.add8(Obj::STATE, 2);
                x.set8(Shot::OWN_PALETTE, 1);
                x.set8(Shot::PALETTE, 0x14);
                x.set8(f::BOX_SET, 6);
                x.set8(f::POWER, 6);
                x.set8(f::POWER2, 6);
                let flip = owner.u8(x.b, f::FLIP);
                x.set8(f::FLIP, flip);
                x.copy_pos(owner);
                let mut v = owner.u8(x.b, Player::SUBACTION) as u32;
                if owner.u16(x.b, Obj::Z) != 0 {
                    v = 0x10;
                }
                let row = v * 2;
                let dx = player_data().volley_offsets.at16(x.b, row);
                let dz = player_data().volley_offsets.at16(x.b, row + 2);
                *x.c.d2() = dz as u32;
                let dx = signed(dx, owner.u8(x.b, f::FLIP) != 0);
                x.add16(Obj::X, dx);
                x.add16(Obj::Z, dz);
                x.add16(Obj::Y, 0xffff);
                let class = owner.u8(x.b, Player::LEVEL) as u32;
                let life = player_data().volley_lifetimes.at8(x.b, class);
                x.set8(LIFE, life);
                x.anim(ANIMS, 0);
                x.reinsert();
            }
            2 => {
                let dx = signed(4, x.u8(f::FLIP) == 0);
                x.add16(Obj::X, dx);
                x.add8(LIFE, 0xff);
                if x.u8(LIFE) == 0 {
                    x.add8(Obj::STATE, 2);
                }
                x.anim_step();
                x.cull_offscreen();
            }
            _ => die(x),
        }
    }

    fn die<R: Bench>(x: &mut Cx<'_, R>) {
        decrement(x, COUNTER_15);
        x.free_pshl();
    }
}

/// Type 16 (0x5369a): thrower star (counter `A5-0x5fb6`), animation with rotating colours.
mod star {
    use super::*;

    /// Countdown of the current phase.
    const TIMER: u32 = 0x5c;
    /// Phase of the star (h).
    const PHASE: u32 = 0x5d;

    const ANIMS: u32 = 0x15c8a2;
    const SFX: crate::sound::Sound = crate::sound::CMD_14F_24F;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.state(0x536a8) {
            0 => {
                x.add8(TIMER, 0xff);
                if x.u8(TIMER) != 0 {
                    return;
                }
                x.add8(Obj::STATE, 2);
                x.set8(PHASE, 0);
                x.set8(Shot::PALETTE, 0x12);
                x.set8(Shot::OWN_PALETTE, 1);
                x.set8(TIMER, 0x10);
                x.set8(f::BOX_SET, 6);
                x.set8(f::POWER, 6);
                x.set8(f::POWER2, 6);
                x.anim(ANIMS, 0);
                x.reinsert();
                if x.u8(Obj::VARIANT) == 0 {
                    crate::sound::play(x.b, SFX, x.o);
                }
            }
            2 => {
                x.add8(TIMER, 0xff);
                if x.u8(TIMER) == 0 {
                    x.add8(Obj::STATE, 2);
                }
                let frame = vars::FRAME_COUNTER.get(x.b) & 3;
                if frame == 0 {
                    let n = x.u8(PHASE).wrapping_add(1) & 7;
                    x.set8(PHASE, n);
                    let c = player_data().star_colors.at8(x.b, n as u32);
                    x.set8(Shot::PALETTE, c);
                }
                x.anim_step();
            }
            _ => {
                decrement(x, COUNTER_16);
                x.free_pshl();
            }
        }
    }
}

/// Type 17 (0x537fa): stone that leaves in an arc, crashes to the ground with a screen shake and flash.
mod stone {
    use super::*;

    /// Countdown of the current phase (word).
    const TIMER: u32 = 0x5c;

    const ANIMS: u32 = 0x15c90a;
    const SFX: crate::sound::Sound = crate::sound::CMD_169;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.state(0x53808) {
            0 => {
                x.add16(TIMER, 0xffff);
                if x.u16(TIMER) != 0 {
                    return;
                }
                x.add8(Obj::STATE, 2);
                x.set8(f::BOX_SET, 6);
                x.set8(f::POWER, 0xa0);
                x.set8(f::POWER2, 0xa0);
                x.set16(Obj::VX, -0x400i16 as u16);
                x.set16(Obj::VZ, -0x800i16 as u16);
                let mut dx = 0x60u16;
                if x.u8(f::FLIP) != 0 {
                    dx = dx.wrapping_neg();
                    let vx = x.u16(Obj::VX);
                    x.set16(Obj::VX, vx.wrapping_neg());
                }
                x.add16(Obj::X, dx);
                x.anim(ANIMS, 0);
                x.reinsert();
            }
            2 => match x.u8(Shot::SUB_STATE) {
                0 => {
                    x.move_xz();
                    if (x.u16(Obj::Z) as i16) >= 0 {
                        x.anim_step();
                        return;
                    }
                    x.add8(Shot::SUB_STATE, 2);
                    x.set32(Obj::Z, 0);
                    x.set16(TIMER, 4);
                    crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::A, 8);
                    x.anim(ANIMS, 1);
                    crate::sound::play(x.b, SFX, x.o);
                }
                2 => {
                    x.add16(TIMER, 0xffff);
                    if x.u16(TIMER) == 0 {
                        x.add8(Shot::SUB_STATE, 2);
                        x.set16(TIMER, 4);
                        vars::SCROLL_Y_PLANE1.set(x.b, 0x100);
                        crate::palette::set_flash_color(x.b, crate::palette::FLASH_COLOR_RED);
                    }
                    x.anim_step();
                }
                4 => {
                    x.add16(TIMER, 0xffff);
                    if x.u16(TIMER) == 0 {
                        x.add8(Shot::SUB_STATE, 2);
                        x.set16(TIMER, 2);
                        crate::palette::set_flash_color(x.b, crate::palette::FLASH_COLOR_WHITE_VALUE);
                    }
                    x.anim_step();
                }
                _ => {
                    if x.u16(TIMER) != 0 {
                        x.add16(TIMER, 0xffff);
                        if x.u16(TIMER) != 0 {
                            return;
                        }
                        vars::SCROLL_Y_PLANE1.set(x.b, 0);
                    }
                    if x.anim_step() {
                        x.add8(Obj::STATE, 2);
                    }
                }
            },
            _ => x.free_pshl(),
        }
    }
}

handlers! {
    0x530bc "pshl_12" jet::run,
    0x53290 "pshl_13" item::run,
    0x53412 "pshl_14" flame::run,
    0x534d6 "pshl_15" volley::run,
    0x5369a "pshl_16" star::run,
    0x537fa "pshl_17" stone::run,
}
