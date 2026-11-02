//! EFFCT family (continued): end-of-level and pickup effects, types 72 to 75 and 79 (type table
//! `0x39ce0`, which has 92 entries of 4 bytes).
//!
//! - 72: pickup detector, which creates the sparkle of type 55 and, depending on its variant, the blink 79;
//! - 73: victory screen (sprite that falls, blinks and changes animation depending on the object that created it);
//! - 74: bubble next to a player that disappears when its creator leaves;
//! - 75: menu button, a seven-substate machine;
//! - 79: blink of a picked-up object, detected by nearby players.

use crate::families::cx::Bench;
use crate::machine::Machine;
use super::cx::{handlers, Cx};
use crate::mem::{rd16, rd8, wr16, wr8, A5};
use crate::objects::Obj;
use crate::objects::pools;
use crate::objects::Current;

/// Values of the state byte `+4` (the next one is computed with `+2`).
mod state {
    pub const INIT: u8 = 0;
    pub const ACTIVE: u8 = 2;
    pub const FREE: u8 = 4;
    /// First state of the object tied to a vanished creator: release on the next turn.
    pub const EXIT: u8 = 6;
}

/// Shared fields.
mod f {
    pub const STATE: u32 = 0x04;
    pub const SUB_STATE: u32 = 0x05;
    /// Variante.
    pub const VARIANT: u32 = 0x03;
    pub const FLAGS: u32 = 0x01;
    pub const FACING: u32 = 0x2c;
    /// Creator (16-bit pointer).
    pub const OWNER: u32 = 0x5c;
    pub const TIMER: u32 = 0x58;
    pub const TIMER_B: u32 = 0x42;
}

/// Original routines used.
mod rom {
    pub const SOUND_0ED11C: u32 = 0xed11c;
    pub const SOUND_0ED056: u32 = 0xed056;
    pub const SOUND_0ED90A: u32 = 0xed90a;
    pub const SOUND_0ECF10: u32 = 0xecf10;
}

/// Jeux d'animations.
mod anims {
    pub const INTERFACE: u32 = 0x19431a;
    pub const MENU: u32 = 0x196baa;
    pub const BUBBLE: u32 = 0x1790f6;
    pub const BLINK: u32 = 0x1bc77c;
}

/// Players: four records of 0x200 bytes.
const PLAYERS: u32 = A5 + 0x5de;
const PLAYER_STRIDE: u32 = 0x200;
/// Jeu en pause.
const PAUSED: u32 = 0x3d;

fn players() -> impl Iterator<Item = u32> {
    (0..4).map(|k| PLAYERS + k * PLAYER_STRIDE)
}

/// Creator of the object.
fn owner<R: Bench>(x: &mut Cx<'_, R>) -> Obj {
    x.linked(f::OWNER)
}

fn free<R: Bench>(x: &mut Cx<'_, R>) {
    x.free(&pools::EFFCT);
}

// ---------------------------------------------------------------------------------------------
// Type 72: pickup detector
// ---------------------------------------------------------------------------------------------

/// 0xb8dc2: creates the blink (type 79) at the object's position, one notch higher.
fn spawn_blink<R: Bench>(x: &mut Cx<'_, R>) {
    let Some(blink) = x.alloc(&pools::EFFCT) else { return };
    wr8(x.b, blink.0, 1);
    wr8(x.b, blink.0 + 2, 79);
    for field in [Obj::X, Obj::Z, Obj::Y] {
        let v = x.u16(field);
        wr16(x.b, blink.0 + field, v);
    }
    let z = x.u16(Obj::Z).wrapping_add(16);
    wr16(x.b, blink.0 + Obj::Z, z);
    wr16(x.b, blink.0 + f::OWNER, x.o.0 as u16);
}

/// 0xa532e: creates the pickup sparkle shot (ESHL of type 55) where the detector is touched.
fn spawn_pickup_flash<R: Bench>(x: &mut Cx<'_, R>) {
    let Some(Obj(flash)) = x.alloc(&pools::ESHL) else { return };
    wr8(x.b, flash, 1);
    wr8(x.b, flash + 2, 55);
    let variant = x.u8(f::VARIANT);
    wr8(x.b, flash + 3, variant);
    let (px, py) = (x.u16(Obj::X), x.u16(Obj::Y));
    wr16(x.b, flash + 0x58, px);
    wr16(x.b, flash + 0x5a, py);
    wr16(x.b, flash + f::OWNER, x.o.0 as u16);
}

/// A player steps on the detector: within 6 pixels in depth and 8 in x.
fn player_on_detector<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let (depth, across) = (x.u16(Obj::Y), x.u16(Obj::X));
    players().any(|player| {
        rd8(x.b, player) != 0
            && depth.wrapping_sub(rd16(x.b, player + Obj::Y)).wrapping_add(6) <= 12
            && across.wrapping_sub(rd16(x.b, player + Obj::X)).wrapping_add(8) <= 16
    })
}

fn pickup_detector<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        state::INIT => {
            x.add8(f::STATE, 2);
            x.set8(0, 1);
            if x.u8(0x2f) != 0 {
                return spawn_blink(x);
            }
            pickup_waiting(x);
        }
        state::ACTIVE => pickup_waiting(x),
        state::FREE => {
            let t = x.u8(f::TIMER).wrapping_sub(1);
            x.set8(f::TIMER, t);
            if (t as i8) < 0 {
                x.add8(f::STATE, 2);
            }
        }
        _ => free(x),
    }
}

fn pickup_waiting<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(0x40) != 0 || x.a5_u8(PAUSED) != 0 {
        return;
    }
    if player_on_detector(x) {
        x.add8(f::STATE, 2);
        x.set8(f::TIMER, 2);
        spawn_pickup_flash(x);
    }
}

// ---------------------------------------------------------------------------------------------
// Type 73: victory screen
// ---------------------------------------------------------------------------------------------

/// Blinking: hidden every other frame according to the parity of the game frame counter.
fn blink_by_frame<R: Bench>(x: &mut Cx<'_, R>) {
    let hidden = x.a5_u16(0x6e) & 1 == 0;
    let flags = x.u8(f::FLAGS);
    x.set8(f::FLAGS, if hidden { flags | 2 } else { flags & !2 });
}

fn victory_screen<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        state::INIT => {
            x.add8(f::STATE, 2);
            let (cx, cy) = (x.a5_u16(0x5a6).wrapping_add(0xc0), x.a5_u16(0x5aa).wrapping_add(0x68));
            x.set16(Obj::X, cx);
            x.set16(Obj::Y, cy);
            x.set16(Obj::Z, 0x80);
            x.set16(0x18, 0xf000);
            crate::sound::play(x.b, rom::SOUND_0ED11C, x.o);
            x.anim_start(anims::INTERFACE, 0);
        }
        state::ACTIVE => victory_active(x),
        _ => free(x),
    }
    if x.u8(0) != 0 {
        x.reinsert();
    }
}

fn victory_active<R: Bench>(x: &mut Cx<'_, R>) {
    let creator = owner(x);
    match x.u8(f::SUB_STATE) {
        0 => {
            x.move_xz();
            if (x.u16(Obj::Z) as i16) <= 0 {
                x.add8(f::SUB_STATE, 2);
                x.set16(Obj::Z, 0);
            }
            x.anim_step();
        }
        2 => {
            if creator.u32(x.b, f::STATE) != 0x0200_0200 {
                x.anim_step();
                return;
            }
            x.add8(f::SUB_STATE, 2);
            blink_by_frame(x);
            crate::sound::play(x.b, rom::SOUND_0ED056, x.o);
            x.anim_start(anims::INTERFACE, 1);
        }
        4 => {
            blink_by_frame(x);
            if creator.u32(x.b, f::STATE) != 0x0200_0800 {
                x.anim_step();
                return;
            }
            x.add8(f::SUB_STATE, 2);
            crate::sound::play(x.b, rom::SOUND_0ED90A, x.o);
            x.anim_start(anims::INTERFACE, 2);
        }
        6 => {
            blink_by_frame(x);
            if x.anim_step() {
                free(x);
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------------------------
// Type 74: bubble next to the creator
// ---------------------------------------------------------------------------------------------

fn bubble<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(f::OWNER) == 0 {
        return x.set32(f::STATE, 0x0600_0000);
    }
    match x.u8(f::STATE) {
        state::INIT => {
            x.add8(f::STATE, 2);
            let creator = owner(x);
            for field in [Obj::X, Obj::Y, Obj::Z] {
                let v = creator.u32(x.b, field);
                x.set32(field, v);
            }
            let facing = creator.u8(x.b, f::FACING);
            x.set8(f::FACING, facing);
            x.add16(Obj::Y, 1);
            let (mut dx, dz) = (36u16, 106u16);
            x.set16(0x14, 128);
            x.set16(0x18, 1472);
            x.set16(0x1a, 0xffc8);
            if facing != 0 {
                dx = dx.wrapping_neg();
                x.set16(0x14, 128u16.wrapping_neg());
            }
            x.add16(Obj::X, dx);
            x.add16(Obj::Z, dz);
            x.anim_start(anims::BUBBLE, 14);
            x.reinsert();
        }
        state::ACTIVE => match x.u8(f::SUB_STATE) {
            0 => {
                x.move_xz();
                if (x.u16(Obj::Z) as i16) > 64 {
                    x.anim_step();
                    return;
                }
                x.add8(f::SUB_STATE, 2);
                x.set16(Obj::Z, 0);
                x.set16(f::TIMER_B, 150);
                x.anim_start(anims::BUBBLE, 15);
            }
            2 => {
                let t = x.u16(f::TIMER_B).wrapping_sub(1);
                x.set16(f::TIMER_B, t);
                if (t as i16) < 0 {
                    x.add8(f::SUB_STATE, 2);
                    x.set16(f::TIMER_B, 60);
                }
            }
            4 => {
                let t = x.u16(f::TIMER_B).wrapping_sub(1);
                x.set16(f::TIMER_B, t);
                if (t as i16) < 0 {
                    x.set8(f::STATE, state::EXIT);
                } else {
                    let flags = x.u8(f::FLAGS) & !2;
                    x.set8(f::FLAGS, if t & 1 != 0 { flags | 2 } else { flags });
                }
            }
            _ => {}
        },
        _ => free(x),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 75 : bouton de menu
// ---------------------------------------------------------------------------------------------

/// Variable `A5-0x55c8` (0xff2a38): step of the menu sequence that drives the button's substates.
const MENU_STEP: u32 = 0xff2a38;
/// Variable `A5+0x2e`: selection step.
const SELECT_STEP: u32 = 0x2e;

fn menu_button<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        state::INIT => {
            x.add8(f::STATE, 2);
            if x.u8(f::VARIANT) == 0 {
                x.set16(Obj::X, 328);
                x.set16(Obj::Y, 368);
                x.anim_start(anims::MENU, 0);
            } else {
                let (cx, cy) = (crate::ram::CAMERA_X.get(x.b), crate::ram::CAMERA_Y.get(x.b));
                x.set16(Obj::X, cx.wrapping_add(0xc0));
                x.set16(Obj::Y, cy.wrapping_add(0x40));
                x.set16(Obj::Z, 40);
                x.anim_start(anims::INTERFACE, 3);
            }
            x.reinsert();
        }
        state::ACTIVE => menu_button_active(x),
        _ => free(x),
    }
}

fn menu_step<R: Bench>(x: &mut Cx<'_, R>) -> u8 {
    crate::mem::rd8(x.b, MENU_STEP)
}

fn menu_button_active<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::VARIANT) != 0 {
        x.anim_step();
        return;
    }
    let advance = |x: &mut Cx<'_, R>, n: u32| {
        x.add8(f::SUB_STATE, 2);
        x.anim_start(anims::MENU, n as u16);
    };
    match x.u8(f::SUB_STATE) {
        0 => {
            if x.a5_u8(SELECT_STEP) == 6 {
                advance(x, 4);
            } else {
                x.anim_step();
            }
        }
        2 => {
            if x.anim_step() {
                advance(x, 5);
                crate::sound::play(x.b, rom::SOUND_0ECF10, x.o);
            } else if x.u8(0x33) != 0 {
                x.set8(0x33, 0);
                // Original glitch: the allocation failure is not tested (see `Pool::alloc_unchecked`).
                let Obj(spawned) = x.alloc_unchecked(&pools::BOSS);
                wr8(x.b, spawned, 1);
                wr8(x.b, spawned + 2, 8);
            }
        }
        4 => {
            if x.anim_step() {
                x.add8(f::SUB_STATE, 2);
                let flags = x.u8(f::FLAGS) | 2;
                x.set8(f::FLAGS, flags);
            }
        }
        6 => {
            if menu_step(x) == 20 {
                x.add8(f::SUB_STATE, 2);
                let flags = x.u8(f::FLAGS) & !2;
                x.set8(f::FLAGS, flags);
                x.anim_start(anims::MENU, 6);
                crate::sound::play(x.b, rom::SOUND_0ECF10, x.o);
            }
        }
        8 => {
            if x.anim_step() {
                advance(x, 7);
            }
        }
        10 => {
            if menu_step(x) == 28 {
                advance(x, 5);
                crate::sound::play(x.b, rom::SOUND_0ECF10, x.o);
            } else {
                x.anim_step();
            }
        }
        12 => {
            if x.anim_step() {
                x.add8(f::STATE, 2);
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------------------------
// Type 79: blink of a picked-up object
// ---------------------------------------------------------------------------------------------

/// A living player of class 5 is in range (32 in depth, 96 in x).
fn class_5_player_near<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let (depth, across) = (x.u16(Obj::Y), x.u16(Obj::X));
    players().any(|player| {
        rd8(x.b, player) != 0
            && rd16(x.b, player + 0x62) != 0
            && rd8(x.b, player + 3) >> 1 == 5
            && depth.wrapping_sub(rd16(x.b, player + Obj::Y)).wrapping_add(32) <= 64
            && across.wrapping_sub(rd16(x.b, player + Obj::X)).wrapping_add(96) <= 192
    })
}

fn blinking<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        state::INIT => {
            x.add8(f::STATE, 2);
            let flags = x.u8(f::FLAGS) | 2;
            x.set8(f::FLAGS, flags);
            x.set8(0x57, 8);
            let v = x.random();
            x.set16(f::TIMER, v & 31);
            x.anim_start(anims::BLINK, 0);
            x.reinsert();
        }
        state::ACTIVE => {
            let creator = owner(x);
            if creator.u8(x.b, f::STATE) != 2 {
                x.add8(f::STATE, 2);
            }
            if x.u8(0x5b) == 0 {
                if class_5_player_near(x) {
                    x.set8(0x5b, 1);
                }
                return;
            }
            if creator.u8(x.b, 0x40) != 0 {
                let flags = x.u8(f::FLAGS) | 2;
                return x.set8(f::FLAGS, flags);
            }
            let t = x.u16(f::TIMER).wrapping_sub(1);
            x.set16(f::TIMER, t);
            if t != 0 {
                return;
            }
            x.set16(f::TIMER, 48);
            let on = x.u8(0x5a) ^ 1;
            x.set8(0x5a, on);
            let flags = x.u8(f::FLAGS) & !2;
            x.set8(f::FLAGS, flags);
            if x.u8(0x5a) != 0 {
                return;
            }
            x.set8(f::FLAGS, flags | 2);
            let left = x.u8(0x57).wrapping_sub(1);
            x.set8(0x57, left);
            if left == 0 {
                x.set32(f::STATE, 0x0400_0000);
            }
        }
        _ => free(x),
    }
}

handlers! {
    0xb841c "effct_detecteur" #[without] pickup_detector,
    0xb84c6 "effct_victory" #[without] victory_screen,
    0xb85fa "effct_bubble_creator" #[without] bubble,
    0xb871a "effct_button" #[without] menu_button,
    0xb8df2 "effct_blink" #[without] blinking,
}
