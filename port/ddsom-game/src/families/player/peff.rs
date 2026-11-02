//! Player effects (class PEFF, pool of 48 objects of 0x60 bytes, table 0x39992), first series.
//! See `docs/families/joueur.md`.

use crate::families::cx::Bench;
use crate::families::cx::{handlers, Cx};
use super::pshell::*;
use super::peff_shop;
use crate::mem::rd16;
use crate::objects::Obj;
use crate::objects::Current;

/// Type 44 (0x517dc): pellet placed on the secondary camera at a column offset (`+3`), animated by set `0x1bcbbe` (or
/// `0x1bcd5a` in the alternate palette); it leaves when it exits the screen at the bottom.
mod badge {
    use super::*;

    /// Column offsets per variant (words, program ROM).
    const COLUMNS: u32 = 0x5184e;
    /// Animation bytes per `+0x2f` (RAM).
    const ANIM_BYTES: u32 = 0xff2478;
    const ANIMS: u32 = 0x1bcbbe;
    const ANIMS_ALT: u32 = 0x1bcd5a;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => init(x),
            2 => active(x),
            _ => x.free_peff(),
        }
    }

    fn init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        let variant = x.u8(0x3) as u32;
        let column = x.rom16(COLUMNS + 2 * variant);
        let cam_x = x.a5_u16(a5::CAM2_X);
        x.set16(0x8, column.wrapping_add(cam_x));
        let cam_y = x.a5_u16(a5::CAM2_Y);
        x.set16(0xc, cam_y);
        x.set32(0x10, 0);
        let which = x.u8(0x2f) as u32;
        let byte = x.data8(ANIM_BYTES + which);
        x.set8(0x42, byte);
        let n = (which * 2 + byte as u32) & 0xffff;
        let set = if x.a5_u16(a5::ALT_PALETTE) != 0 { ANIMS_ALT } else { ANIMS };
        x.anim_start(set, n as u16);
        x.reinsert();
    }

    fn active<R: Bench>(x: &mut Cx<'_, R>) {
        let d = x.u16(0xc).wrapping_sub(x.a5_u16(a5::CAM2_Y));
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

    /// Per scene (from 0x45): (offset, width) in X and in depth of the two zones (words, in the clear).
    const ZONES_A: u32 = 0x51306;
    const ZONES_B: u32 = 0x51376;
    /// Per scene: dialogue script of each zone (longs, program ROM).
    const SCRIPTS_A: u32 = 0x51106;
    const SCRIPTS_B: u32 = 0x5113e;
    const ANIMS: u32 = 0x1c03a6;
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
        match x.u8(0x4) {
            0 => init(x),
            2 => active(x),
            _ => x.free_peff(),
        }
    }

    fn init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        let scene = x.a5_u8(0x34);
        let row = (scene.wrapping_sub(FIRST_SCENE as u8) as u32) << 3;
        for (table, field) in [(ZONES_A, ZONE_A), (ZONES_B, ZONE_B)] {
            for k in (0..8).step_by(2) {
                let v = x.data16(table + row + k);
                x.set16(field + k, v);
            }
        }
        let index = (scene as u16).wrapping_sub(FIRST_SCENE).wrapping_mul(4) as i16 as u32;
        let a = x.rom32(SCRIPTS_A.wrapping_add(index));
        x.set32(0x1c, a);
        let b = x.rom32(SCRIPTS_B.wrapping_add(index));
        x.set32(0x58, b);
        let variant = x.u8(0x3) as u32;
        x.anim(ANIMS, variant);
        x.reinsert();
    }

    /// Two rounds over the four players (the dialogue interpreter is called with the player ready in the zone), then
    /// scroll tracking and animation step.
    fn active<R: Bench>(x: &mut Cx<'_, R>) {
        let zones: [(u32, fn(&mut Cx<'_, R>, Obj)); 2] =
            [(ZONE_A, peff_shop::talk_a), (ZONE_B, peff_shop::talk_b)];
        for (zone, talk) in zones {
            for k in 0..4 {
                if let Some(player) = in_zone(x, zone, k) {
                    if (player.u8(x.b, PLAYER_READY) as i8) > 0 {
                        talk(x, player);
                    }
                }
            }
        }
        let [dx, dz] = [0, 1].map(|k| {
            (x.a5_u32(SCROLL[k]).wrapping_sub(x.a5_u32(PREVIOUS_SCROLL[k])) as i32 >> 1) as u32
        });
        x.add32(0x8, dx);
        x.add32(0xc, dz);
        x.anim_step();
    }

    /// The player in slot `k` of the list, if it is in the zone.
    fn in_zone<R: Bench>(x: &mut Cx<'_, R>, zone: u32, k: u32) -> Option<Obj> {
        let w = rd16(x.b, PLAYERS + 2 * k);
        if w == 0 {
            return None;
        }
        let p = Obj(w as i16 as i32 as u32 & 0xff_ffff);
        let dx = x.u16(0x8).wrapping_sub(p.u16(x.b, 0x8)).wrapping_add(x.u16(zone));
        if dx > x.u16(zone + 2) {
            return None;
        }
        let dz = x.u16(0xc).wrapping_add(x.u16(0x10)).wrapping_sub(p.u16(x.b, 0xc)).wrapping_sub(p.u16(x.b, 0x10));
        let dz = dz.wrapping_add(x.u16(zone + 4));
        (dz <= x.u16(zone + 6)).then_some(p)
    }
}

handlers! {
    0x51076 "peff_43" shop::run,
    0x517dc "peff_44" badge::run,
}
