//! Player effects (PEFF), twelfth series: type 17 (icon of the equipment row of the interface).
//! See `docs/families/joueur.md`.

use crate::families::cx::Bench;
use crate::families::cx::{handlers, Cx};
use super::pshell::*;
use crate::objects::Obj;
use crate::objects::Current;

/// Type 17 (0x4c9fe): icon of an equipment slot (rank `+3`) of the owning player, placed on the screen at a spot that
/// depends on the game (`A5+0x20`: one-player mode or more) and on the player. It changes frame when the equipment
/// changes, reaches its place at eight pixels per frame, and follows the scroll of the camera. No processing during the
/// pause.
mod slot_icon {
    use super::*;

    /// Offset tables (X, Y) per player, then per rank: two versions according to `A5+0x20`.
    const PLAYER_A: u32 = 0x4cabe;
    const SLOT_A: u32 = 0x4cae2;
    const PLAYER_B: u32 = 0x4caae;
    const SLOT_B: u32 = 0x4caca;
    const ICONS: u32 = 0x1b7726;
    const ICONS_ALT: u32 = 0x1b8204;
    /// Resting place of the icon (X, Y) and state: `+0x52`, `+0x54`, `+0x56` (0 moving, 1 taken, negative waiting).
    const HOME: u32 = 0x52;
    const TAKEN: u32 = 0x56;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        if x.a5_u8(0x3d) != 0 {
            return;
        }
        match x.u8(0x4) {
            0 => {
                init(x);
                active(x);
            }
            2 => active(x),
            _ => x.free_peff(),
        }
    }

    /// Owning player + rank of the slot (`lea (0,A1,D0.w),A1`).
    fn slot<R: Bench>(x: &mut Cx<'_, R>) -> Obj {
        let owner = x.linked(f::OWNER);
        let rank = x.u8(0x3) as u32;
        Obj(owner.0 + rank)
    }

    fn init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        x.set8(TAKEN, 0);
        let at = slot(x);
        let item = at.u8(x.b, 0x140);
        let count = at.u8(x.b, 0x148);
        x.set8(0x57, item);
        x.set8(0x58, count);
        let (player_table, slot_table) = if x.a5_u8(0x20) == 1 { (PLAYER_A, SLOT_A) } else { (PLAYER_B, SLOT_B) };
        let (mut d1, mut d2) = (x.a5_u16(a5::CAM_X), x.a5_u16(a5::CAM_Y));
        let owner = x.linked(f::OWNER);
        let row = (owner.u8(x.b, 0x2) as u32) << 2;
        d1 = d1.wrapping_add(x.data16(player_table + row));
        d2 = d2.wrapping_add(x.data16(player_table + row + 2));
        let row = (x.u8(0x3) as u32) << 2;
        d1 = d1.wrapping_add(x.data16(slot_table + row));
        d2 = d2.wrapping_add(x.data16(slot_table + row + 2));
        x.set16(HOME, d1);
        x.set16(HOME + 2, d2);
        x.set16(0x8, d1);
        x.set16(0x10, d2);
        x.set16(0xc, 0);
        choose_icon(x);
    }

    /// `0x4cb16`: icon frame from the current equipment.
    fn choose_icon<R: Bench>(x: &mut Cx<'_, R>) {
        let at = slot(x);
        let item = at.u8(x.b, 0x140) as u32;
        let count = at.u8(x.b, 0x148) as u32;
        let set = if x.a5_u16(a5::ALT_PALETTE) != 0 { ICONS_ALT } else { ICONS };
        x.anim_start(set, (((item << 4) + count) & 0xffff) as u16);
    }

    fn active<R: Bench>(x: &mut Cx<'_, R>) {
        refresh(x);
        pickup(x);
        approach(x);
        scroll(x);
        x.reinsert();
    }

    /// `0x4cb50`: the equipment has changed: new frame (the icon of a removed item goes back to its place).
    fn refresh<R: Bench>(x: &mut Cx<'_, R>) {
        let at = slot(x);
        let item = at.u8(x.b, 0x140);
        let count = at.u8(x.b, 0x148);
        if item == x.u8(0x57) && count == x.u8(0x58) {
            return;
        }
        if item == 0x56 {
            let (hx, hy) = (x.u16(HOME), x.u16(HOME + 2));
            x.set16(0x8, hx);
            x.set16(0xc, hy);
        }
        x.set8(0x57, item);
        x.set8(0x58, count);
        let set = if x.a5_u16(a5::ALT_PALETTE) != 0 { ICONS_ALT } else { ICONS };
        x.anim_start(set, ((((item as u32) << 4) + count as u32) & 0xffff) as u16);
    }

    /// `0x4cc56`: a picked-up item (linked by `+0x5c`) passes over the icon: it takes it.
    fn pickup<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(TAKEN) != 0 {
            return;
        }
        let item = x.linked(0x5c);
        let dx = item.u16(x.b, 0x8).wrapping_sub(x.u16(0x8)).wrapping_add(0x10);
        if dx > 0x20 {
            return;
        }
        let dy = item.u16(x.b, 0xc).wrapping_sub(x.u16(0x10)).wrapping_add(8);
        if dy > 0x10 {
            return;
        }
        if item.u8(x.b, 0x58) == 0 {
            return;
        }
        item.set_u8(x.b, 0x58, 0);
        x.set8(TAKEN, 1);
        item.set_u16(x.b, 0x5c, x.o.0 as u16);
    }

    /// `0x4cbb2`: reaches its place at eight pixels per frame; once taken, it follows the item.
    fn approach<R: Bench>(x: &mut Cx<'_, R>) {
        let taken = x.u8(TAKEN) as i8;
        if taken > 0 {
            let item = x.linked(0x5c);
            let (ix, iz) = (item.u16(x.b, 0x8), item.u16(x.b, 0xc));
            x.set16(0x8, ix);
            x.set16(0xc, iz);
            x.set16(0x10, 1);
            x.add16(0xc, 0xfff8);
            return;
        }
        if taken < 0 {
            let v = x.u8(TAKEN).wrapping_sub(1);
            x.set8(TAKEN, v);
            if (v as i8) < 0 {
                x.set8(TAKEN, 0);
            }
            return;
        }
        let target = x.u16(HOME);
        step_axis(x, 0x8, target);
        let target = x.u16(HOME + 2);
        let y = x.u16(0x10) as i16;
        if target as i16 == y {
            return;
        }
        if target as i16 > y {
            x.add16(0xc, 8);
            if target as i16 > x.u16(0xc) as i16 {
                return;
            }
        } else {
            x.add16(0xc, 0xfff8);
            if (target as i16) < (x.u16(0xc) as i16) {
                x.set16(0xc, target);
                return;
            }
        }
        x.set16(0x10, target);
        x.set16(0xc, 0);
    }

    /// Advances by 8 toward `target` without going past it.
    fn step_axis<R: Bench>(x: &mut Cx<'_, R>, at: u32, target: u16) {
        let here = x.u16(at) as i16;
        let t = target as i16;
        if t == here {
            return;
        }
        if t > here {
            x.add16(at, 8);
            if t <= x.u16(at) as i16 {
                x.set16(at, target);
            }
        } else {
            x.add16(at, 0xfff8);
            if t >= x.u16(at) as i16 {
                x.set16(at, target);
            }
        }
    }

    /// `0x4cc9a`: follows the scroll of the camera.
    fn scroll<R: Bench>(x: &mut Cx<'_, R>) {
        let delta = x.a5_u32(a5::CAM_X).wrapping_sub(x.a5_u32(0x524));
        x.add32(0x8, delta);
        x.add32(HOME, delta);
    }
}

handlers! {
    0x4c9fe "peff_17" slot_icon::run,
}
