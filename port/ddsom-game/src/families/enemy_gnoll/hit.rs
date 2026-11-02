//! Type 13, hit state (`+4 = 4`, 0x6baba). `+5`: 0 reaction by table (`+0x49`) after dropping the axe, 2 recoil, 4 fall
//! and get up, 6 second reaction table, 8 grabbed by a player. Zone and shadow are always tested; there is neither a
//! hook pause nor a byte `+0` to reset to 1.

use crate::families::cx::Bench;
use super::*;
use crate::objects::Current;
use crate::objects::Obj;

/// Reaction tables shared by the hits, indexed by code `+0x49`.
const REACT_FIRST: u32 = 0x6bb4c;
const REACT_SECOND: u32 = 0x6bd3a;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    let freeze = x.u8(f::FREEZE) as i8;
    if freeze > 0 {
        x.set8(f::FREEZE, (freeze - 1) as u8);
    } else {
        match x.u8(f::SUB) {
            0 => first_reaction(x),
            2 => recoil(x),
            4 => fall(x),
            6 => react(x, REACT_SECOND),
            8 => grab(x),
            _ => {}
        }
    }
    zone_and_terrain(x);
    x.reinsert();
}

/// 0x6bb20: the axe falls (the Gnoll drops it on the first frame), then reaction by the table.
fn first_reaction<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB2) == 0 {
        x.set_bit(1, 1, false);
    }
    if x.u8(WEAPON) != 0 {
        x.set8(WEAPON, 0);
        lib::eshl_16(x, 0);
    }
    react(x, REACT_FIRST);
}

/// Recul (0x6bbf8).
fn recoil<R: Bench>(x: &mut Cx<'_, R>) {
    if paused(x) {
        return;
    }
    if x.u8(f::SUB2) == 0 {
        x.add8(f::SUB2, 2);
        x.set8(f::FREEZE, 4);
        x.set32(Obj::VX, 0x0500_0000);
        x.set16(Obj::AX, -0x38i16 as u16);
        let flip = x.u8(0x4a) ^ 0x20;
        x.set8(0x4a, flip);
        if flip != 0 {
            negate(x, Obj::VX);
            negate(x, Obj::AX);
        }
        return mv(x, 8);
    }
    if x.u8(Obj::VX) != 0 {
        x.move_x();
    }
    if x.anim_step() {
        set_state_long(x, STANCE);
        x.set32(0x58, 0);
    }
}

/// Fall then get up (0x6bc64). The animations depend on `+0xbf` (weapon).
fn fall<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            let anim = 6 + x.u8(ARMED) as u32;
            mv(x, anim);
        }
        2 => {
            if paused(x) {
                return;
            }
            let n = x.u16(0x84).wrapping_sub(1);
            x.set16(0x84, n);
            if (n as i16) > 0 {
                return;
            }
            x.set8(0x86, 0);
            x.set8(0x49, 0);
            x.add8(f::SUB2, 2);
            x.set8(f::SUB3, 0);
            x.set8(0, 2);
            let anim = 6 + x.u8(ARMED) as u32;
            mv(x, anim);
        }
        4 => {
            if paused(x) {
                return;
            }
            if x.u8(f::SUB3) == 0 {
                if !x.anim_step() {
                    return;
                }
                x.set8(0x86, 0);
                x.set8(0, 1);
                x.set8(VULNERABLE, 1);
                if x.u8(ARMED) != 0 && x.u8(Obj::VARIANT) != 0xa {
                    x.add8(f::SUB3, 2);
                    return mv(x, 9);
                }
            } else {
                x.anim_step();
                if x.u8(f::KEY_FRAME) != 0xb {
                    return;
                }
                lib::eshl_16(x, 1);
                x.set8(f::KEY_FRAME, 0);
            }
            set_state_long(x, STANCE);
        }
        _ => {}
    }
}
