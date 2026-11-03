//! Player effects (PEFF), twelfth series: type 17 (icon of the equipment row of the interface).
//! See `docs/families/joueur.md`.

use super::vars;
use crate::families::player::records::Shot;
use crate::player::Player;
use crate::families::cx::Bench;
use crate::families::cx::{handlers, Cx};
use super::pshell::*;
use crate::objects::Obj;
use crate::objects::Current;
use crate::player::tables::{word};

/// Step of the slide of the icons toward their place (pixels per frame).
const SLIDE_STEP: u16 = 8;

/// Type 17 (0x4c9fe): icon of an equipment slot (rank `+3`) of the owning player, placed on the screen at a spot that
/// depends on the game (`A5+0x20`: one-player mode or more) and on the player. It changes frame when the equipment
/// changes, reaches its place at eight pixels per frame, and follows the scroll of the camera. No processing during the
/// pause.
mod slot_icon {
    use super::*;

    /// Quantity shown.
    const COUNT: u32 = 0x58;
    /// Linked icon (16-bit pointer).
    const ICON: u32 = 0x5c;

    /// Offset tables (X, Y) per player, then per rank: two versions according to `A5+0x20`.
    const PLAYER_A: u32 = 0x4cabe;
    const SLOT_A: u32 = 0x4cae2;
    const PLAYER_B: u32 = 0x4caae;
    const SLOT_B: u32 = 0x4caca;
    const ICONS: crate::data::RomAddr = crate::data::RomAddr(0x1b7726);
    const ICONS_ALT: crate::data::RomAddr = crate::data::RomAddr(0x1b8204);
    /// Resting place of the icon (X, Y) and state: `+0x52`, `+0x54`, `+0x56` (0 moving, 1 taken, negative waiting).
    const HOME: u32 = 0x52;
    const TAKEN: u32 = 0x56;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        if vars::PAUSE.get(x.b) != 0 {
            return;
        }
        match x.u8(Obj::STATE) {
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
        let rank = x.u8(Obj::VARIANT) as u32;
        Obj(owner.0 + rank)
    }

    fn init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(Obj::STATE, 2);
        x.set8(TAKEN, 0);
        let at = slot(x);
        let item = at.u8(x.b, Player::INVENTORY);
        let count = at.u8(x.b, Player::QUANTITIES);
        x.set8(Shot::HIT_FLAGS, item);
        x.set8(COUNT, count);
        let (player_table, slot_table) = if vars::VERSION.get(x.b) == 1 { (PLAYER_A, SLOT_A) } else { (PLAYER_B, SLOT_B) };
        let (mut d1, mut d2) = (vars::CAM_X.get(x.b), vars::CAM_Y.get(x.b));
        let owner = x.linked(f::OWNER);
        let row = (owner.u8(x.b, Player::INDEX) as u32) << 2;
        d1 = d1.wrapping_add(word(x.b, player_table + row));
        d2 = d2.wrapping_add(word(x.b, player_table + row + 2));
        let row = (x.u8(Obj::VARIANT) as u32) << 2;
        d1 = d1.wrapping_add(word(x.b, slot_table + row));
        d2 = d2.wrapping_add(word(x.b, slot_table + row + 2));
        x.set16(HOME, d1);
        x.set16(HOME + 2, d2);
        x.set16(Obj::X, d1);
        x.set16(Obj::Y, d2);
        x.set16(Obj::Z, 0);
        choose_icon(x);
    }

    /// `0x4cb16`: icon frame from the current equipment.
    fn choose_icon<R: Bench>(x: &mut Cx<'_, R>) {
        let at = slot(x);
        let item = at.u8(x.b, Player::INVENTORY) as u32;
        let count = at.u8(x.b, Player::QUANTITIES) as u32;
        let set = if vars::ALT_PALETTE.get(x.b) != 0 { ICONS_ALT.0 } else { ICONS.0 };
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
        let item = at.u8(x.b, Player::INVENTORY);
        let count = at.u8(x.b, Player::QUANTITIES);
        if item == x.u8(Shot::HIT_FLAGS) && count == x.u8(COUNT) {
            return;
        }
        if item == 0x56 {
            let (hx, hy) = (x.u16(HOME), x.u16(HOME + 2));
            x.set16(Obj::X, hx);
            x.set16(Obj::Z, hy);
        }
        x.set8(Shot::HIT_FLAGS, item);
        x.set8(COUNT, count);
        let set = if vars::ALT_PALETTE.get(x.b) != 0 { ICONS_ALT.0 } else { ICONS.0 };
        x.anim_start(set, ((((item as u32) << 4) + count as u32) & 0xffff) as u16);
    }

    /// `0x4cc56`: a picked-up item (linked by `+0x5c`) passes over the icon: it takes it.
    fn pickup<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(TAKEN) != 0 {
            return;
        }
        let item = x.linked(ICON);
        let dx = item.u16(x.b, Obj::X).wrapping_sub(x.u16(Obj::X)).wrapping_add(0x10);
        if dx > 0x20 {
            return;
        }
        let dy = item.u16(x.b, Obj::Z).wrapping_sub(x.u16(Obj::Y)).wrapping_add(8);
        if dy > 0x10 {
            return;
        }
        if item.u8(x.b, COUNT) == 0 {
            return;
        }
        item.set_u8(x.b, COUNT, 0);
        x.set8(TAKEN, 1);
        item.set_u16(x.b, ICON, x.o.0 as u16);
    }

    /// `0x4cbb2`: reaches its place at eight pixels per frame; once taken, it follows the item.
    fn approach<R: Bench>(x: &mut Cx<'_, R>) {
        let taken = x.u8(TAKEN) as i8;
        if taken > 0 {
            let item = x.linked(ICON);
            let (ix, iz) = (item.u16(x.b, Obj::X), item.u16(x.b, Obj::Z));
            x.set16(Obj::X, ix);
            x.set16(Obj::Z, iz);
            x.set16(Obj::Y, 1);
            x.add16(Obj::Z, 0xfff8);
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
        let y = x.u16(Obj::Y) as i16;
        if target as i16 == y {
            return;
        }
        if target as i16 > y {
            x.add16(Obj::Z, 8);
            if target as i16 > x.u16(Obj::Z) as i16 {
                return;
            }
        } else {
            x.add16(Obj::Z, 0xfff8);
            if (target as i16) < (x.u16(Obj::Z) as i16) {
                x.set16(Obj::Z, target);
                return;
            }
        }
        x.set16(Obj::Y, target);
        x.set16(Obj::Z, 0);
    }

    /// Advances by 8 toward `target` without going past it.
    fn step_axis<R: Bench>(x: &mut Cx<'_, R>, at: u32, target: u16) {
        let here = x.u16(at) as i16;
        let t = target as i16;
        if t == here {
            return;
        }
        if t > here {
            x.add16(at, SLIDE_STEP);
            if t <= x.u16(at) as i16 {
                x.set16(at, target);
            }
        } else {
            x.add16(at, SLIDE_STEP.wrapping_neg());
            if t >= x.u16(at) as i16 {
                x.set16(at, target);
            }
        }
    }

    /// `0x4cc9a`: follows the scroll of the camera.
    fn scroll<R: Bench>(x: &mut Cx<'_, R>) {
        let delta = vars::CAM_X_LONG.get(x.b).wrapping_sub(vars::PREV_CAM_X.get(x.b));
        x.add32(Obj::X, delta);
        x.add32(HOME, delta);
    }
}

handlers! {
    PEFF_17 0x4c9fe "peff_17" slot_icon::run,
}
