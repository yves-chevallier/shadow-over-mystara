//! Small scenery (SET) routines called by tasks, scenes or handlers of other
//! families: the 68000 leftover that fit no handler. Each is a native at its original
//! address; the registers it leaves are those of the original.

use crate::world::World;

use crate::hw::mem::{rd16, rd8, wr16, wr32, wr8, A5};
use crate::world::objects::Obj;
use crate::actors::families::set::tables as set_tables;

/// Tile buffer of layer 3: the columns of 44 longwords cleared by 0xd5f12.
const COLUMNS: [u32; 3] = [0x91_215c, 0x91_2160, 0x91_2164];
const COLUMN_ROWS: u32 = 44;
/// Step of one row of the buffer.
const ROW: u32 = 0x80;
/// Empty tile (code 0x20, attribute 0).
pub(crate) const BLANK: u32 = 0x20_0000;

/// Clears the three columns of the tile buffer; returns the end of the last one (A1 of the original).
pub fn clear_columns(b: &mut World) -> u32 {
    let mut a1 = 0;
    for column in COLUMNS {
        a1 = column;
        for _ in 0..COLUMN_ROWS {
            wr32(b, a1, BLANK);
            a1 += ROW;
        }
    }
    a1
}

/// Boss health bar (h): display flag (FF8030) and SET object of the bar (short pointer at FF2476).
const LIFE_BAR_SHOWN: u32 = 0x30;
const LIFE_BAR_OBJECT: u32 = 0xff_2476;

/// 0xd5ef8: when the health bar is displayed, attaches it to boss `boss` (`+0x34`, variant 1, state reset to
/// zero) and clears the tile columns of the old bar (0xd5f12). Returns the end of the cleared columns (the A1
/// of the original), `None` if the bar is not displayed. Core.
pub fn attach_life_bar(b: &mut World, boss: crate::world::objects::Obj) -> Option<u32> {
    if rd8(b, A5 + LIFE_BAR_SHOWN) == 0 {
        return None;
    }
    let bar = rd16(b, LIFE_BAR_OBJECT) as i16 as i32 as u32 & 0xff_ffff;
    wr16(b, bar + 0x34, boss.word());
    wr8(b, bar + 3, 1);
    wr32(b, bar + 4, 0);
    Some(clear_columns(b))
}

/// 0xce668: loads two block grids (0x26d4, destination 0x917000, the second right after) whose numbers are
/// read, as word pairs, from table 0xce686 at index `character`.
pub fn two_grids(b: &mut World, character: u16) {
    const GRIDS: u32 = 0xce686;
    let table = GRIDS.wrapping_add(character.wrapping_mul(4) as u32);
    let mut destination = 0x91_7000;
    for k in 0..2 {
        let n = set_tables::word(b, table + 2 * k); // read through (A4)+: data space
        destination = crate::services::video::load_grid(b, crate::services::video::Grid::Layer2, n, destination).destination;
    }
}

/// Core of `variant_routine` for generator `me`: the variant's routine, nothing (0xd99f8) or a scene
/// music (0xe0c5c, 0xe0d8c), with no registers.
fn variant_sound(b: &mut World, me: Obj, kinds: u32, routines: u32) {
    const NOTHING: u32 = 0xd99f8;
    let variant = me.u8(b, Obj::VARIANT) as u32;
    let index = (set_tables::pc_byte(b, kinds + variant) as u16).wrapping_mul(4);
    match set_tables::pc_long(b, routines + index as u32) {
        NOTHING => {}
        sound @ (0xe0c5c | 0xe0d8c) => crate::services::sound::play(b, crate::services::sound::Sound::at(sound), me),
        r => unreachable!("wave routine {r:#x}: tables 0xd98cc and 0xd996e only have 0xd99f8, 0xe0c5c, 0xe0d8c"),
    }
}

/// 0xd98b8 with no registers (generator `me`).
pub fn begin_wave(b: &mut World, me: Obj) {
    variant_sound(b, me, 0xd98d8, 0xd98cc);
}

/// 0xd995a with no registers (generator `me`).
pub fn finish_wave(b: &mut World, me: Obj) {
    variant_sound(b, me, 0xd9976, 0xd996e);
}

/// Camera settings set by the "call" entries of the scene scripts (D0 = value): scroll limit
/// in X (`A5+0x52c`), vertical position (`A5+0x57c`) and camera mode (`A5+0x4ff`); the
/// other settings (0x2a36, 0x2a42) are in `small`.
pub(crate) mod camera {
    use super::*;

    const LIMIT_X: u32 = 0x52c;
    const POSITION_Y: u32 = 0x57c;
    const MODE: u32 = 0x4ff;

    /// 0x2a24: scroll limit `v` (the flags of the `move.w` are reread by nobody).
    pub fn set_limit_x(b: &mut World, v: u16) {
        wr16(b, A5 + LIMIT_X, v);
    }

    /// 0x2a2a: vertical position `v`.
    pub fn set_position_y(b: &mut World, v: u16) {
        wr16(b, A5 + POSITION_Y, v);
    }

    /// 0x2a30: camera mode `v`.
    pub fn set_mode(b: &mut World, v: u8) {
        wr8(b, A5 + MODE, v);
    }
}
