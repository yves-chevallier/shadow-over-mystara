//! Placement of a player on free ground and the players' effect palettes (subsystem X2).

use cps2::Cps2Bus;

use crate::mem::{prog16, rd16, rd32, rd8, wr16, wr32, A5};
use crate::objects::Obj;

/// Free terrains: numbers below 4.
const FREE_TERRAIN: u16 = 4;
/// Position of the players' camera (x, depth); `A5+0x36` selects the second camera.
const CAMERA: (u32, u32) = (0x506, 0x50a);
const ALT_CAMERA: (u32, u32) = (0x556, 0x55a);
/// Offsets (words, code) of the starting point of the search, per player (`+2`).
const START_X: u32 = 0x3658;
const START_Y: u32 = 0x3660;
/// Last starting point (RAM words).
const LAST_X: u32 = A5 - 0x719a;
const LAST_Y: u32 = A5 - 0x7198;

fn camera(b: &mut Cps2Bus) -> (u16, u16) {
    let (x, y) = if rd8(b, A5 + 0x36) != 0 { ALT_CAMERA } else { CAMERA };
    (rd16(b, A5 + x), rd16(b, A5 + y))
}

/// End of the 0x3668 search, enough to rebuild the registers of the original.
#[cfg_attr(not(feature = "tools"), allow(dead_code))] // fields read by the tools (registers of the original)
pub(crate) struct Search {
    /// Loop counters (D5, D6) at the time of exit.
    pub(crate) d5: u32,
    pub(crate) d6: u32,
    /// Last terrain examined (D0-D4, A1).
    pub(crate) last: crate::video::Terrain,
    /// Height set (D0).
    pub(crate) height: u16,
    /// Depth set (D1).
    pub(crate) y: u16,
}

/// 0x3668: places player `o` at resurrection. From a player-specific starting point, looks for a free terrain
/// by trying, like the original, the starting point and then the same point shifted by 16 pixels (in depth,
/// then in x); failing that, the screen centre (+192, +80). The height makes the player fall from the top of the
/// screen (camera + 256 - depth).
pub fn place_on_free_ground(b: &mut Cps2Bus, o: Obj) {
    search(b, o);
}

pub(crate) fn search(b: &mut Cps2Bus, o: Obj) -> Search {
    let me = o.0;
    let player = 2 * rd8(b, me + 2) as u32;
    let (cam_x, _) = camera(b);
    let x0 = cam_x.wrapping_add(prog16(b, START_X + player));
    wr16(b, LAST_X, x0);
    let mut found = None;
    let (mut d5, mut d6) = (0, 0);
    let mut last = None;
    'search: for i in 0..5u16 {
        d5 = (4 - i) as u32;
        // x of the attempt: the search resumes with the coordinate corrected by the previous terrain (D0 of 0x24b0)
        let mut x = if i == 0 { x0 } else { x0.wrapping_add(16) };
        let (_, cam_y) = camera(b);
        let y0 = cam_y.wrapping_add(prog16(b, START_Y + player));
        wr16(b, LAST_Y, y0);
        for j in 0..10u16 {
            d6 = (9 - j) as u32;
            let t = crate::video::terrain(b, x, if j == 0 { y0 } else { y0.wrapping_add(16) });
            last = Some(t);
            if t.number < FREE_TERRAIN {
                found = Some((t.x, t.y));
                break 'search;
            }
            x = t.x;
        }
        d6 = d6 & 0xffff_0000 | 0xffff;
    }
    let (x, y) = found.unwrap_or_else(|| {
        d5 = d5 & 0xffff_0000 | 0xffff;
        let (cam_x, cam_y) = camera(b);
        (cam_x.wrapping_add(192), cam_y.wrapping_add(80))
    });
    wr16(b, me + 8, x);
    wr16(b, me + 0x10, y);
    let (_, cam_y) = camera(b);
    let height = cam_y.wrapping_add(256).wrapping_sub(y);
    wr16(b, me + 0x0c, height);
    Search { d5, d6, last: last.expect("at least one attempt"), height, y }
}

/// 16-colour palettes (data) copied into the palette 0x91c1e0 by 0x3af8..0x3b14.
const EFFECT_PALETTES: u32 = 0x25a640;
pub(crate) const EFFECT_PALETTE_RAM: u32 = 0x91c1e0;

/// Players' effect palettes: offset in `EFFECT_PALETTES` (entries 0x3af8, 0x3afe, 0x3b04, 0x3b0c, 0x3b14).
pub mod effect_palette {
    /// Brightening (0x3af8).
    pub const LIGHT: u16 = 0x20;
    /// Ordinary colours (0x3afe).
    pub const NORMAL: u16 = 0;
    pub const P360: u16 = 0x360;
    pub const P380: u16 = 0x380;
    /// Red (0x3b14).
    pub const RED: u16 = 0x3a0;
}

/// 0x3b1c: copies the effect palette at offset `offset` (32 bytes); returns the eight longs copied and the next
/// source.
pub fn load_effect_palette(b: &mut Cps2Bus, offset: u16) -> ([u32; 8], u32) {
    let from = EFFECT_PALETTES.wrapping_add(offset as i16 as i32 as u32);
    let longs: [u32; 8] = std::array::from_fn(|i| rd32(b, from + 4 * i as u32));
    for (i, &v) in longs.iter().enumerate() {
        wr32(b, EFFECT_PALETTE_RAM + 4 * i as u32, v);
    }
    (longs, from + 32)
}
