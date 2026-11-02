//! Player effects (PEFF), fifteenth series: type 24 (the interface's pile of gold that takes back the player's items in
//! exchange for points). See `docs/families/joueur.md`.

use crate::families::cx::Bench;
use crate::families::cx::{handlers, Cx};
use crate::mem::{rd32, wr32, wr8};
use super::pshell::*;
use crate::objects::Obj;
use crate::objects::pools;
use crate::objects::Current;

const ANIMS: u32 = 0x1c40c6;
/// Position of the icon per player (X, Y): two versions according to `A5+0x20` (1: one-player screen).
const PLAYER_SPOTS_A: u32 = 0x4d792;
const PLAYER_SPOTS_B: u32 = 0x4d782;
/// Per-character tables (pointers) giving, for each piece of equipment, 0 (nothing), 1 (sold `+0x148` times), ≥ 2 (sold
/// once) or negative (exchanged for nothing).
const SELL_TABLES: u32 = 0x4d884;
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

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(0x16, 0);
                let table = if x.a5_u8(0x20) == 1 { PLAYER_SPOTS_A } else { PLAYER_SPOTS_B };
                let (cx, cy) = (x.a5_u16(a5::CAM_X), x.a5_u16(a5::CAM_Y));
                let owner = x.linked(f::OWNER);
                let row = (owner.u8(x.b, 0x2) as u32) << 2;
                let dx = x.data16(table + row);
                let dy = x.data16(table + row + 2);
                x.set16(0x8, cx.wrapping_add(dx));
                x.set16(0x10, cy.wrapping_add(dy));
                x.anim(ANIMS, 0x2b);
                x.reinsert();
            }
            2 => {
                check(x);
                let delta = x.a5_u32(a5::CAM_X).wrapping_sub(x.a5_u32(0x524));
                x.add32(0x8, delta);
            }
            _ => x.free(&pools::EFFCT),
        }
    }

    /// `0x4d7ac`
    fn check<R: Bench>(x: &mut Cx<'_, R>) {
        let linked = x.linked(0x5c);
        let flags = x.u8(0x17);
        let had = flags & 1 != 0;
        x.set8(0x17, flags & !1);
        if had {
            x.set16(0x1a, 0);
        }
        let d0 = linked.u16(x.b, 0x5c);
        if d0 == 0 && x.u16(0x1a) != 0 {
            x.set8(0x17, 1);
        } else {
            x.set16(0x1a, d0);
        }
        let dx = linked.u16(x.b, 0x8).wrapping_sub(x.u16(0x8));
        if dx > 0x18 {
            return away(x);
        }
        let dy = x.u16(0x10).wrapping_sub(linked.u16(x.b, 0x10)).wrapping_sub(linked.u16(x.b, 0xc));
        if dy > 0x20 {
            return away(x);
        }
        if x.u8(0x16) == 0 {
            x.set8(0x16, 1);
            return x.anim(ANIMS, 0x2c);
        }
        if x.u8(0x17) == 0 {
            return;
        }
        sell(x);
    }

    fn away<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(0x16) == 0 {
            return;
        }
        x.set8(0x16, 0);
        x.anim(ANIMS, 0x2b);
    }

    /// `0x4d81c`: the linked player (`+0x1a`) hands back the equipment of its slot `+3`.
    fn sell<R: Bench>(x: &mut Cx<'_, R>) {
        let who = Obj(x.u16(0x1a) as i16 as i32 as u32 & 0xff_ffff);
        let player_word = who.u16(x.b, f::OWNER);
        let player = Obj(player_word as i16 as i32 as u32 & 0xff_ffff);
        let rank = who.u8(x.b, 0x3) as u32;
        let slot = player.0 + rank;
        let character = (player.u8(x.b, 0x3) & 0xe) as u32;
        let table = x.rom32(SELL_TABLES + character * 2);
        let id = crate::mem::rd8(x.b, slot + 0x140) as u32;
        let code = x.data8(table + id);
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
                let add = x.data8(price + k);
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
    0x4d718 "peff_24" coins::run,
}
