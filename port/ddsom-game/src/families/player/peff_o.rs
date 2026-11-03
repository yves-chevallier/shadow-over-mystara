//! Player effects (PEFF), fifteenth series: type 24 (the interface's pile of gold that takes back the player's items in
//! exchange for points). See `docs/families/joueur.md`.

use super::vars;
use crate::player::Player;
use crate::families::cx::Bench;
use crate::families::cx::{handlers, Cx};
use crate::mem::{rd32, wr32, wr8};
use super::pshell::*;
use crate::objects::Obj;
use crate::objects::pools;
use crate::objects::Current;
use crate::player::tables::{player_data, byte, word, Aligned};

const ANIMS: crate::data::RomAddr = crate::data::RomAddr(0x1c40c6);
/// Position of the icon per player (X, Y): two versions according to `A5+0x20` (1: one-player screen).
const PLAYER_SPOTS_A: u32 = 0x4d792;
const PLAYER_SPOTS_B: u32 = 0x4d782;
/// Prices in points (BCD on 3 bytes, 4 bytes per piece of equipment).
const PRICES: u32 = 0x4db30;

/// Decimal addition of a byte (ABCD) with incoming carry: result and outgoing carry.
fn bcd_add(a: u8, b: u8, carry: bool) -> (u8, bool) {
    let mut r = a as u16 + b as u16 + carry as u16;
    if (a & 0xf) + (b & 0xf) + carry as u8 > 9 {
        r += 6;
    }
    let out = r > 0x99;
    if out {
        r += 0x60;
    }
    (r as u8, out)
}

/// The pile: placed in the corner of the player's screen, it animates when a player carrying a piece of equipment (rank
/// `+3` of the row) comes near and presses the button (`+0x17`); the equipment is taken back and its price added to the
/// score (capped at 999999).
mod coins {
    use super::*;

    /// Low byte of `Obj::AX`, used as flags (h).
    const AX_LOW: u32 = 0x17;
    /// Linked coin (16-bit pointer).
    const LINKED: u32 = 0x5c;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(Obj::STATE) {
            0 => {
                x.add8(Obj::STATE, 2);
                x.set8(Obj::AX, 0);
                let table = if vars::VERSION.get(x.b) == 1 { PLAYER_SPOTS_A } else { PLAYER_SPOTS_B };
                let (cx, cy) = (vars::CAM_X.get(x.b), vars::CAM_Y.get(x.b));
                let owner = x.linked(f::OWNER);
                let row = (owner.u8(x.b, Player::INDEX) as u32) << 2;
                let dx = word(x.b, table + row);
                let dy = word(x.b, table + row + 2);
                x.set16(Obj::X, cx.wrapping_add(dx));
                x.set16(Obj::Y, cy.wrapping_add(dy));
                x.anim(ANIMS.0, 0x2b);
                x.reinsert();
            }
            2 => {
                check(x);
                let delta = vars::CAM_X_LONG.get(x.b).wrapping_sub(vars::PREV_CAM_X.get(x.b));
                x.add32(Obj::X, delta);
            }
            _ => x.free(&pools::EFFCT),
        }
    }

    /// `0x4d7ac`
    fn check<R: Bench>(x: &mut Cx<'_, R>) {
        let linked = x.linked(LINKED);
        let flags = x.u8(AX_LOW);
        let had = flags & 1 != 0;
        x.set8(AX_LOW, flags & !1);
        if had {
            x.set16(Obj::AZ, 0);
        }
        let d0 = linked.u16(x.b, LINKED);
        if d0 == 0 && x.u16(Obj::AZ) != 0 {
            x.set8(AX_LOW, 1);
        } else {
            x.set16(Obj::AZ, d0);
        }
        let dx = linked.u16(x.b, Obj::X).wrapping_sub(x.u16(Obj::X));
        if dx > 0x18 {
            return away(x);
        }
        let dy = x.u16(Obj::Y).wrapping_sub(linked.u16(x.b, Obj::Y)).wrapping_sub(linked.u16(x.b, Obj::Z));
        if dy > 0x20 {
            return away(x);
        }
        if x.u8(Obj::AX) == 0 {
            x.set8(Obj::AX, 1);
            return x.anim(ANIMS.0, 0x2c);
        }
        if x.u8(AX_LOW) == 0 {
            return;
        }
        sell(x);
    }

    fn away<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(Obj::AX) == 0 {
            return;
        }
        x.set8(Obj::AX, 0);
        x.anim(ANIMS.0, 0x2b);
    }

    /// `0x4d81c`: the linked player (`+0x1a`) hands back the equipment of its slot `+3`.
    fn sell<R: Bench>(x: &mut Cx<'_, R>) {
        let who = Obj(x.u16(Obj::AZ) as i16 as i32 as u32 & 0xff_ffff);
        let player_word = who.u16(x.b, f::OWNER);
        let player = Obj(player_word as i16 as i32 as u32 & 0xff_ffff);
        let rank = who.u8(x.b, Player::CHARACTER) as u32;
        let slot = player.0 + rank;
        let character = (player.u8(x.b, Player::CHARACTER) & 0xe) as u32;
        let table = player_data().sell_tables.long_at(x.b, character * 2);
        let id = crate::mem::rd8(x.b, slot + 0x140) as u32;
        let code = byte(x.b, table + id);
        if code == 0 {
            return;
        }
        let repeats: u16 = if code == 1 {
            (crate::mem::rd8(x.b, slot + 0x148) as u16).wrapping_sub(1)
        } else if (code as i8) > 1 {
            0
        } else {
            wr8(x.b, slot + 0x140, 0x56);
            wr8(x.b, slot + 0x148, 0);
            return;
        };
        let price = PRICES + (id << 2);
        let mut left = repeats;
        loop {
            // 3 decimal bytes of `price+1..price+3` added to `(0x115..0x117, player)`
            let mut carry = false;
            for k in (1..=3u32).rev() {
                let add = byte(x.b, price + k);
                let sum = crate::mem::rd8(x.b, player.0 + 0x114 + k);
                let (r, c) = bcd_add(sum, add, carry);
                wr8(x.b, player.0 + 0x114 + k, r);
                carry = c;
            }
            if (rd32(x.b, player.0 + 0x114) as i32) > 0x99_9999 {
                wr32(x.b, player.0 + 0x114, 0x99_9999);
            }
            let again = left != 0;
            left = left.wrapping_sub(1);
            if !again {
                break;
            }
        }
        wr8(x.b, slot + 0x140, 0x56);
        wr8(x.b, slot + 0x148, 0);
    }
}

handlers! {
    PEFF_24 0x4d718 "peff_24" coins::run,
}
