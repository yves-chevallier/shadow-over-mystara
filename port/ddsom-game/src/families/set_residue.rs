//! Small scenery (SET) routines called by tasks, scenes or handlers of other
//! families: the 68000 leftover that fit no handler. Each is a native at its original
//! address; the registers it leaves are those of the original.

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::Cpu;

use crate::mem::{prog32, prog8, rd16, rd8, rts, wr16, wr32, wr8, A5};
use crate::{regs, Native};
use crate::objects::Obj;

/// Tile buffer of layer 3: the columns of 44 longwords cleared by 0xd5f12.
const COLUMNS: [u32; 3] = [0x91_215c, 0x91_2160, 0x91_2164];
const COLUMN_ROWS: u32 = 44;
/// Step of one row of the buffer.
const ROW: u32 = 0x80;
/// Empty tile (code 0x20, attribute 0).
const BLANK: u32 = 0x20_0000;

/// Clears the three columns of the tile buffer; returns the end of the last one (A1 of the original).
pub fn clear_columns(b: &mut Cps2Bus) -> u32 {
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

/// 0xd5f12: clears three columns of the tile buffer (44 rows each) for the scene exit.
fn erase_columns(c: &mut Cpu, b: &mut Cps2Bus) {
    let end = clear_columns(b);
    columns_regs(c, end);
    rts(c, b);
}

/// Registers left by the column clearing: A1 = end of the last one, D0 = exhausted `dbf` counter, D4 =
/// the empty tile.
fn columns_regs(c: &mut Cpu, end: u32) {
    c.a[1] = end;
    c.d[0] = 0xffff;
    c.d[4] = BLANK;
    c.flags_logic(BLANK, Size::Long);
}

/// Boss health bar (h): display flag (FF8030) and SET object of the bar (short pointer at FF2476).
const LIFE_BAR_SHOWN: u32 = 0x30;
const LIFE_BAR_OBJECT: u32 = 0xff_2476;

/// 0xd5ef8: when the health bar is displayed, attaches it to boss `boss` (`+0x34`, variant 1, state reset to
/// zero) and clears the tile columns of the old bar (0xd5f12). Returns the end of the cleared columns (the A1
/// of the original), `None` if the bar is not displayed. Core.
pub fn attach_life_bar(b: &mut Cps2Bus, boss: crate::objects::Obj) -> Option<u32> {
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
pub fn two_grids(b: &mut Cps2Bus, character: u16) {
    const GRIDS: u32 = 0xce686;
    let table = GRIDS.wrapping_add(character.wrapping_mul(4) as u32);
    let mut destination = 0x91_7000;
    for k in 0..2 {
        let n = rd16(b, table + 2 * k); // read through (A4)+: data space
        destination = crate::video::load_grid(b, crate::video::Grid::Layer2, n, destination).destination;
    }
}

/// `two_grids` (character D0) for the tools (the game calls `two_grids`).
fn load_two_grids(c: &mut Cpu, b: &mut Cps2Bus) {
    two_grids(b, c.d[0] as u16);
    rts(c, b);
}

/// Core of `variant_routine` for generator `me`: the variant's routine, nothing (0xd99f8) or a scene
/// music (0xe0c5c, 0xe0d8c), with no registers.
fn variant_sound(b: &mut Cps2Bus, me: Obj, kinds: u32, routines: u32) {
    const NOTHING: u32 = 0xd99f8;
    let variant = me.u8(b, 3) as u32;
    let index = (prog8(b, kinds + variant) as u16).wrapping_mul(4);
    match prog32(b, routines + index as u32) {
        NOTHING => {}
        sound @ (0xe0c5c | 0xe0d8c) => crate::sound::play(b, sound, me),
        r => unreachable!("wave routine {r:#x}: tables 0xd98cc and 0xd996e only have 0xd99f8, 0xe0c5c, 0xe0d8c"),
    }
}

/// 0xd98b8 with no registers (generator `me`).
pub fn begin_wave(b: &mut Cps2Bus, me: Obj) {
    variant_sound(b, me, 0xd98d8, 0xd98cc);
}

/// 0xd995a with no registers (generator `me`).
pub fn finish_wave(b: &mut Cps2Bus, me: Obj) {
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

    /// 0x2a24: scroll limit `v`; the flags of the `move.w` for the caller still on the processor.
    pub fn set_limit_x(c: &mut Cpu, b: &mut Cps2Bus, v: u16) {
        wr16(b, A5 + LIMIT_X, v);
        c.flags_logic(v as u32, Size::Word);
    }

    /// 0x2a2a: vertical position `v`.
    pub fn set_position_y(c: &mut Cpu, b: &mut Cps2Bus, v: u16) {
        wr16(b, A5 + POSITION_Y, v);
        c.flags_logic(v as u32, Size::Word);
    }

    /// 0x2a30: camera mode `v`.
    pub fn set_mode(c: &mut Cpu, b: &mut Cps2Bus, v: u8) {
        wr8(b, A5 + MODE, v);
        c.flags_logic(v as u32, Size::Byte);
    }
}

const fn native(entry: u32, name: &'static str, run: fn(&mut Cpu, &mut Cps2Bus), outputs: u32) -> Native {
    Native { entry, name, run: crate::Body::WithCpu(run), touches_sound: false, outputs }
}

pub const NATIVES: &[Native] = &[
    native(0xd5f12, "set_clears_columns", erase_columns, regs::ALL),
    native(0xce668, "set_two_grids", load_two_grids, regs::ALL),
    native(0xd98b8, "set_wave_start", |c, b| { begin_wave(b, Obj(c.a[0])); rts(c, b) }, regs::ALL),
    native(0xd995a, "set_wave_end", |c, b| { finish_wave(b, Obj(c.a[0])); rts(c, b) }, regs::ALL),
];
