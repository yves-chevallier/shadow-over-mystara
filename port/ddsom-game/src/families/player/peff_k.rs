//! Player effects (PEFF), eleventh series: types 40 (meteors) and 45 (indicators of the selection screen).
//! See `docs/families/joueur.md`.

use super::vars;
use crate::families::player::records::Shot;
use crate::player::Player;
use crate::objects::Obj;
use crate::families::cx::Bench;
use crate::families::cx::{handlers, Cx};
use super::pshell::*;
use crate::mem::{rd32, wr32};
use crate::objects::pools;
use crate::objects::Current;
use crate::player::tables::{player_data, Aligned};

/// Palette window of the graphics RAM copied by the effects (8 longs).
const PALETTE_WINDOW: u32 = 0x91c1e0;
/// Mask added or combined with the copied colours (transparency bit of the words).
const PALETTE_MASK: u32 = 0xf000_f000;

/// Copies 8 longs from the ROM to the palette window (`movem.l (A2)+,{D1-D6,A3,A4}` then `movem.l` to `(A1)`): the
/// first six longs are combined with the mask by OR, the last two add it.
fn copy_palette<R: Bench>(x: &mut Cx<'_, R>, source: u32) {
    let mut words = [0u32; 8];
    for (i, w) in words.iter_mut().enumerate() {
        *w = rd32(x.b, source + 4 * i as u32);
    }
    for w in words.iter_mut().take(6) {
        *w |= PALETTE_MASK;
    }
    for w in words.iter_mut().skip(6) {
        *w = w.wrapping_add(PALETTE_MASK);
    }
    for (i, w) in words.iter().enumerate() {
        wr32(x.b, PALETTE_WINDOW + 4 * i as u32, *w);
    }
    // Original glitch: D1-D6 keep the first six longs; D2-D5 remain for the following handlers (`dispatch::Leftovers`)
    (*x.c.d2(), *x.c.d3(), *x.c.d4(), *x.c.d5()) = (words[1], words[2], words[3], words[4]);
}

/// Type 40 (0x50148): meteor rain (magic 20, kind 0x28). Variant 0: the cloud, which changes the palette of the screen
/// then drops meteors (variant 1) while holding for `+0x168` frames; variant 1: a meteor that falls from a random point
/// around the cloud.
mod meteors {
    use super::*;

    /// Countdown (word); low byte: delay between meteors.
    const TIMER: u32 = 0x58;
    /// Delay before the next meteor.
    const DELAY: u32 = 0x59;
    /// Parent shower (16-bit pointer).
    const PARENT: u32 = 0x5c;

    const ANIMS: u32 = 0x1bc790;
    const PALETTE: u32 = 0x25a780;
    const SFX: crate::sound::Sound = crate::sound::CMD_191;
    const KIND: u8 = 0x28;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        let child = x.u8(Obj::VARIANT) != 0;
        match x.state(0x50156) {
            0 if !child => {
                crate::sound::play(x.b, SFX, x.o);
                copy_palette(x, PALETTE);
                x.add8(Obj::STATE, 2);
                follow_owner(x);
                x.add16(Obj::Z, 0);
                x.add16(Obj::Y, 1);
                x.set16(TIMER, 0x510);
                x.anim(ANIMS, 0);
                x.reinsert();
            }
            0 => {
                x.add8(Obj::STATE, 2);
                let parent = x.linked(PARENT);
                let (px, pz) = (parent.u16(x.b, Obj::X), parent.u16(x.b, Obj::Z));
                x.set16(Obj::X, px);
                x.set16(Obj::Z, pz);
                let y = parent.u32(x.b, Obj::Y).wrapping_sub(1);
                x.set32(Obj::Y, y);
                let dx = (x.rand_and(0xff) as u16).wrapping_sub(0x80);
                x.add16(Obj::X, dx);
                *x.c.d2() = dx as u32;
                let dz = (x.rand_and(0x7f) as u16).wrapping_sub(0x3f);
                x.add16(Obj::Z, dz);
                x.anim(ANIMS, 1);
                x.reinsert();
            }
            2 if !child => {
                match x.u8(Shot::SUB_STATE) {
                    0 => cloud(x),
                    _ => {
                        let hidden = x.u8(Obj::FLAGS) & 2 == 0;
                        x.set_bit(Obj::FLAGS, 1, hidden);
                        x.add16(TIMER, 0xffff);
                        if (x.u16(TIMER) as i16) < 0 {
                            x.free_peff();
                        }
                    }
                }
            }
            2 => {
                if x.anim_step() {
                    x.free_peff();
                }
            }
            _ => x.free_peff(),
        }
    }

    fn cloud<R: Bench>(x: &mut Cx<'_, R>) {
        let owner = x.linked(f::OWNER);
        if owner.u16(x.b, Player::ACTION_TIMER) <= 0x20 {
            x.add8(Shot::SUB_STATE, 4);
            x.set16(TIMER, 0x20);
            return;
        }
        x.add8(DELAY, 0xff);
        if (x.u8(DELAY) as i8) < 0 {
            x.set8(DELAY, 0x20);
            x.add8(TIMER, 0xff);
            if (x.u8(TIMER) as i8) >= 0 {
                crate::sound::play(x.b, SFX, x.o);
            }
        }
        x.anim_step();
        let signal = x.u8(Shot::ANIM_EVENT);
        if signal == 0 {
            return;
        }
        if signal & 0x80 != 0 {
            x.add16(Obj::Z, 1);
            return;
        }
        for _ in 0..2 {
            let Some(o) = x.alloc(&pools::PEFF) else { return };
            o.set_u8(x.b, Obj::ACTIVE, 1);
            o.set_u8(x.b, Obj::KIND, KIND);
            o.set_u16(x.b, PARENT, x.o.0 as u16);
            let owner = x.u16(f::OWNER);
            o.set_u16(x.b, f::OWNER, owner);
            o.set_u8(x.b, Obj::VARIANT, 1);
        }
    }
}

/// Type 45 (0x518b4): indicator of the selection screen (kind 0x2d) placed on the player; variant `+0x38` chooses the
/// palette effect launched when the script signals it.
mod lamp {
    use super::*;

    /// Step of the player's move when the lamp was created.
    const MOVE_STEP: u32 = 0x5d;

    const ANIMS: u32 = 0x1be8b8;
    const PALETTE_A: u32 = 0x25b200;
    const PALETTE_B: u32 = 0x25a780;
    const SFX: crate::sound::Sound = crate::sound::CMD_195;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.state(0x518c2) {
            0 => {
                x.add8(Obj::STATE, 2);
                let owner = x.linked(f::OWNER);
                x.copy_pos(owner);
                x.add16(Obj::Z, 0x40);
                x.add16(Obj::Y, 0xffff);
                x.anim(ANIMS, 0);
                crate::sound::play(x.b, SFX, x.o);
                active(x);
            }
            2 => active(x),
            _ => x.free_peff(),
        }
    }

    fn active<R: Bench>(x: &mut Cx<'_, R>) {
        let owner = x.linked(f::OWNER);
        if x.u8(MOVE_STEP) != owner.u8(x.b, Player::STEP) {
            x.add8(Obj::STATE, 2);
            return crate::palette::restore_flash(x.b);
        }
        x.copy_pos(owner);
        x.add16(Obj::Z, 0x40);
        x.add16(Obj::Y, 0xffff);
        let row = (x.u8(Obj::FRAME_DATA_A) as u32) * 2;
        let dx = player_data().lamp_offsets_x.word_at(x.b, row);
        let dz = player_data().lamp_offsets_z.word_at(x.b, row);
        *x.c.d2() = dz as u32;
        x.add16(Obj::X, dx);
        x.add16(Obj::Z, dz);
        if x.u8(Shot::ANIM_EVENT) != 0 {
            x.set8(Shot::ANIM_EVENT, 0);
            palette_effect(x);
        }
        x.anim_step();
        x.set_bit(Obj::FLAGS, 1, false);
        let blink = vars::FRAME_COUNTER.get(x.b) & 1;
        if blink != 0 {
            x.set_bit(Obj::FLAGS, 1, false);
        }
        x.reinsert();
    }

    /// 0x3afe: ordinary effect palette. Original glitch: the copied longs remain in D2-D5 (`dispatch::Leftovers`).
    fn normal_palette<R: Bench>(x: &mut Cx<'_, R>) {
        let (longs, _) = crate::placement::load_effect_palette(x.b, crate::placement::effect_palette::NORMAL);
        (*x.c.d2(), *x.c.d3(), *x.c.d4(), *x.c.d5()) = (longs[2], longs[3], longs[4], longs[5]);
    }

    /// `0x519b0`: palette effect of the variant.
    fn palette_effect<R: Bench>(x: &mut Cx<'_, R>) {
        let v = x.u8(Obj::FRAME_DATA_A);
        match v {
            0 => { crate::palette::flash(x.b, crate::palette::Flash::Gray); },
            1 => {
                copy_palette(x, PALETTE_A);
                { crate::palette::flash(x.b, crate::palette::Flash::B); };
            }
            2 => {
                normal_palette(x);
                { crate::palette::flash(x.b, crate::palette::Flash::D); };
            }
            3 => {
                normal_palette(x);
                { crate::palette::flash(x.b, crate::palette::Flash::C); };
            }
            4 => {
                copy_palette(x, PALETTE_B);
                { crate::palette::flash(x.b, crate::palette::Flash::G); };
            }
            _ => crate::palette::restore_flash(x.b),
        }
    }
}

handlers! {
    0x50148 "peff_40" meteors::run,
    0x518b4 "peff_45" lamp::run,
}
