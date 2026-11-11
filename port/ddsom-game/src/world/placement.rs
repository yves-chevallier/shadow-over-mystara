//! Placement of a player on free ground and the players' effect palettes (subsystem X2).

use crate::world::World;

use crate::hw::video;
use crate::world::objects::Obj;
use crate::actors::player::Player;
use crate::world::ram::{camera, Word};
use crate::world::tables::engine;

/// Free terrains: numbers below 4.
const FREE_TERRAIN: u16 = 4;
/// Last starting point (`A5-0x719a`, `A5-0x7198`).
const LAST_X: Word = Word(0xff0e66);
const LAST_Y: Word = Word(0xff0e68);

/// Offset of the starting point of the search for player `o` in `table` (`placement/start_x`, `start_y`: words of
/// the code, per player).
fn start_offset(b: &mut World, o: Obj, table: &crate::world::tables::Table) -> u16 {
    let player = Player::of(o).index(b) as u32;
    table.nth16(b, player)
}

/// Starting point of the search for player `o` (relative to the camera): x.
pub(crate) fn start_x(b: &mut World, o: Obj) -> u16 {
    camera(b).0.wrapping_add(start_offset(b, o, crate::world::tables::engine_table!("placement/start_x")))
}

/// Starting point of the search for player `o`: depth.
pub(crate) fn start_y(b: &mut World, o: Obj) -> u16 {
    camera(b).1.wrapping_add(start_offset(b, o, crate::world::tables::engine_table!("placement/start_y")))
}

/// Can a player be placed on terrain `t`?
pub(crate) fn is_free(t: &crate::services::video::Terrain) -> bool {
    t.number < FREE_TERRAIN
}

/// 0x3668: places player `o` at resurrection. From a player-specific starting point, looks for a free terrain
/// by trying, like the original, the starting point and then the same point shifted by 16 pixels (in depth,
/// then in x); failing that, the screen centre (+192, +80). The height makes the player fall from the top of the
/// screen (camera + 256 - depth).
pub fn place_on_free_ground(b: &mut World, o: Obj) {
    let x0 = start_x(b, o);
    LAST_X.set(b, x0);
    let mut found = None;
    'search: for i in 0..5 {
        // x of the attempt: the search resumes with the coordinate corrected by the previous terrain (D0 of 0x24b0)
        let mut x = if i == 0 { x0 } else { x0.wrapping_add(16) };
        let y0 = start_y(b, o);
        LAST_Y.set(b, y0);
        for j in 0..10 {
            let t = crate::services::video::terrain(b, x, if j == 0 { y0 } else { y0.wrapping_add(16) });
            if is_free(&t) {
                found = Some((t.x, t.y));
                break 'search;
            }
            x = t.x;
        }
    }
    let (x, y) = found.unwrap_or_else(|| {
        let (cam_x, cam_y) = camera(b);
        (cam_x.wrapping_add(192), cam_y.wrapping_add(80))
    });
    o.set_u16(b, Obj::X, x);
    o.set_u16(b, Obj::Y, y);
    let (_, cam_y) = camera(b);
    o.set_u16(b, Obj::Z, cam_y.wrapping_add(256).wrapping_sub(y));
}

/// 16-colour palettes of the library (from palette 0x3b2, at 0x25a640) copied into the palette of colour 0xf0 of page
/// 0 (0x91c1e0) by 0x3af8..0x3b14.
const EFFECT_PALETTES: u32 = ddsom_data::Palettes::ROM + 32 * 0x3b2;
pub(crate) const EFFECT_PALETTE_RAM: video::Vram = video::palette(0, 0xf0);

/// Players' effect palettes: offset from `EFFECT_PALETTES` (entries 0x3af8, 0x3afe, 0x3b04, 0x3b0c, 0x3b14).
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
pub fn load_effect_palette(b: &mut World, offset: u16) -> ([u32; 8], u32) {
    let from = EFFECT_PALETTES.wrapping_add(offset as i16 as i32 as u32);
    let longs: [u32; 8] = std::array::from_fn(|i| engine().long(b, from + 4 * i as u32));
    for (i, &v) in longs.iter().enumerate() {
        EFFECT_PALETTE_RAM.offset(4 * i as u32).set_long(b, v);
    }
    (longs, from + 32)
}
