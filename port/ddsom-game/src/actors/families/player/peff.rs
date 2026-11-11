//! Player effects (class PEFF, pool of 48 objects of 0x60 bytes, table 0x39992), first series.
//! See `docs/families/joueur.md`.

use super::vars;
use crate::actors::families::player::records::Shot;
use crate::actors::families::cx::Bench;
use crate::actors::families::cx::{handlers, Cx};
use super::peff_shop;
use crate::world::mem::rd16;
use crate::world::objects::Obj;
use crate::world::objects::Current;
use crate::actors::player::tables::{player_data, byte, word, Aligned};

/// Type 44 (0x517dc): pellet placed on the secondary camera at a column offset (`+3`), animated by set `0x1bcbbe` (or
/// `0x1bcd5a` in the alternate palette); it leaves when it exits the screen at the bottom.
mod badge {
    use super::*;

    /// Animation byte chosen by the badge.
    const ANIM_BYTE: u32 = 0x42;

    /// Animation bytes per `+0x2f` (RAM).
    const ANIM_BYTES: u32 = 0xff2478;
    const ANIMS: crate::world::data::RomAddr = crate::world::data::RomAddr(0x1bcbbe);
    const ANIMS_ALT: crate::world::data::RomAddr = crate::world::data::RomAddr(0x1bcd5a);

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(Obj::STATE) {
            0 => init(x),
            2 => active(x),
            _ => x.free_peff(),
        }
    }

    fn init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(Obj::STATE, 2);
        let variant = x.u8(Obj::VARIANT) as u32;
        let column = player_data().badge_columns.word_at(x.b, 2 * variant);
        let cam_x = vars::CAM2_X.get(x.b);
        x.set16(Obj::X, column.wrapping_add(cam_x));
        let cam_y = vars::CAM2_Y.get(x.b);
        x.set16(Obj::Z, cam_y);
        x.set32(Obj::Y, 0);
        let which = x.u8(Shot::SETTING) as u32;
        let byte = byte(x.b, ANIM_BYTES + which);
        x.set8(ANIM_BYTE, byte);
        let n = (which * 2 + byte as u32) & 0xffff;
        let set = if vars::ALT_PALETTE.get(x.b) != 0 { ANIMS_ALT.0 } else { ANIMS.0 };
        x.anim_start(set, n as u16);
        x.reinsert();
    }

    fn active<R: Bench>(x: &mut Cx<'_, R>) {
        let d = x.u16(Obj::Z).wrapping_sub(vars::CAM2_Y.get(x.b));
        if d >= 0x110 {
            x.free_peff();
        }
    }
}

/// Type 43 (0x51076): shop counter (scenes 0x45 and following). Two zones, set from the scene, watch the four players;
/// a player in a zone with positive `+0x58` starts the zone's dialogue script (pointers `+0x1c` and `+0x58`,
/// interpreter 0x514ac / 0x514cc: `peff_shop.rs`). The object follows the screen scroll at half speed.
mod shop {
    use super::*;

    /// Second dialogue script of the scene shop (long).
    const SCRIPT_B: u32 = 0x58;

    /// Per scene (from 0x45): (offset, width) in X and in depth of the two zones (words, in the clear).
    const ZONES_A: u32 = 0x51306;
    const ZONES_B: u32 = 0x51376;
    const ANIMS: crate::world::data::RomAddr = crate::world::data::RomAddr(0x1c03a6);
    const FIRST_SCENE: u16 = 0x45;
    /// Four registered players (words, 0 = absent).
    const PLAYERS: u32 = 0xff2aa4;
    /// Scroll position and its value on the previous frame (longs, A5).
    const SCROLL: [u32; 2] = [0x506, 0x50a];
    const PREVIOUS_SCROLL: [u32; 2] = [0x524, 0x528];
    /// Zone A: `+0x14` X offset, `+0x16` width, `+0x18` depth offset, `+0x1a` extent.
    const ZONE_A: u32 = 0x14;
    const ZONE_B: u32 = 0x50;
    /// Player byte: positive when it can talk to the merchant.
    const PLAYER_READY: u32 = 0x58;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(Obj::STATE) {
            0 => init(x),
            2 => active(x),
            _ => x.free_peff(),
        }
    }

    fn init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(Obj::STATE, 2);
        let scene = vars::SCENE.get(x.b);
        let row = (scene.wrapping_sub(FIRST_SCENE as u8) as u32) << 3;
        for (table, field) in [(ZONES_A, ZONE_A), (ZONES_B, ZONE_B)] {
            for k in (0..8).step_by(2) {
                let v = word(x.b, table + row + k);
                x.set16(field + k, v);
            }
        }
        let index = (scene as u16).wrapping_sub(FIRST_SCENE).wrapping_mul(4) as i16 as u32;
        let a = player_data().shop_scripts_a.long_past(x.b, index);
        x.set32(Obj::VY, a);
        let b = player_data().shop_scripts_b.long_past(x.b, index);
        x.set32(SCRIPT_B, b);
        let variant = x.u8(Obj::VARIANT) as u32;
        x.anim(ANIMS.0, variant);
        x.reinsert();
    }

    /// One round over the four players: `talk` for each player in `zone` who is ready.
    fn talk_in<R: Bench>(x: &mut Cx<'_, R>, zone: u32, talk: fn(&mut Cx<'_, R>, Obj)) {
        for k in 0..4 {
            if let Some(player) = in_zone(x, zone, k) {
                if (player.u8(x.b, PLAYER_READY) as i8) > 0 {
                    talk(x, player);
                }
            }
        }
    }

    /// Two rounds over the four players (the dialogue interpreter is called with the player ready in the zone), then
    /// scroll tracking and animation step.
    fn active<R: Bench>(x: &mut Cx<'_, R>) {
        talk_in(x, ZONE_A, peff_shop::talk_a);
        talk_in(x, ZONE_B, peff_shop::talk_b);
        let [dx, dz] = [0, 1].map(|k| {
            (x.a5_u32(SCROLL[k]).wrapping_sub(x.a5_u32(PREVIOUS_SCROLL[k])) as i32 >> 1) as u32
        });
        x.add32(Obj::X, dx);
        x.add32(Obj::Z, dz);
        x.anim_step();
    }

    /// The player in slot `k` of the list, if it is in the zone.
    fn in_zone<R: Bench>(x: &mut Cx<'_, R>, zone: u32, k: u32) -> Option<Obj> {
        let w = rd16(x.b, PLAYERS + 2 * k);
        if w == 0 {
            return None;
        }
        let p = Obj(w as i16 as i32 as u32 & 0xff_ffff);
        let dx = x.u16(Obj::X).wrapping_sub(p.u16(x.b, Obj::X)).wrapping_add(x.u16(zone));
        if dx > x.u16(zone + 2) {
            return None;
        }
        let dz = x.u16(Obj::Z).wrapping_add(x.u16(Obj::Y)).wrapping_sub(p.u16(x.b, Obj::Z)).wrapping_sub(p.u16(x.b, Obj::Y));
        let dz = dz.wrapping_add(x.u16(zone + 4));
        (dz <= x.u16(zone + 6)).then_some(p)
    }
}

handlers! {
    PEFF_43 0x51076 "peff_43" shop::run,
    PEFF_44 0x517dc "peff_44" badge::run,
}
