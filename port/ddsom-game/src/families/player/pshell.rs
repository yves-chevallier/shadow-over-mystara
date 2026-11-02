//! Elements common to the handlers of the players' shots (PSHL) and effects (PEFF): object fields, release, creation of
//! sibling objects.
//!
//! A PSHL or PEFF object is born from a creator (`player/spawn.rs`) with its kind (`+2`), its variant (`+3`) and the
//! link `+0x50` to the player; some types keep a link `+0x52` or `+0x5c` to a sibling object. The positions follow the
//! player with an offset read from a ROM table.

use crate::machine::Machine;
use crate::objects::Current;
use crate::families::cx::{Bench, Cx};
use crate::objects::Obj;
use crate::objects::pools;

/// Fields specific to player objects.
pub mod f {
    /// Orientation inherited from the player (non-zero: looks right).
    pub const FLIP: u32 = 0x2c;
    /// Collision box set.
    pub const BOX_SET: u32 = 0x2e;
    /// Identifier (`+0x45`) and power (`+0x46`) of the attack.
    pub const POWER: u32 = 0x45;
    pub const POWER2: u32 = 0x46;
    /// Owning player (16-bit RAM address).
    pub const OWNER: u32 = 0x50;
    /// Flags set by the collision engine.
    pub const HIT_FLAGS: u32 = 0x57;
}

/// Main states (`+4`).
pub mod st {
    pub const FREE: u8 = 4;
}

/// Value of an address register loaded by `movea.w`: the address word, sign-extended.
pub fn ext(o: Obj) -> u32 {
    o.0 as u16 as i16 as i32 as u32
}

/// Offsets of the work RAM (A5).
pub mod a5 {
    /// Secondary camera.
    pub const CAM2_X: u32 = 0x556;
    pub const CAM2_Y: u32 = 0x55a;
    /// Main camera.
    pub const CAM_X: u32 = 0x506;
    pub const CAM_Y: u32 = 0x50a;
    /// Non-zero: the reference camera is the secondary one.
    pub const USE_CAM2: u32 = 0x36;
    /// Non-zero: alternate-palette version (`tst.w (0x70,A5)`).
    pub const ALT_PALETTE: u32 = 0x70;
    /// Frame counter of the game.
    pub const FRAME: u32 = 0x6f;
    /// Free slots of the PEFF pool.
    pub const PEFF_FREE: u32 = 0x7e60;
    /// Game paused (non-zero byte).
    pub const PAUSED: u32 = 0x42;
}

impl<R: Bench> Cx<'_, R> {
    /// Masked draw (`jsr rng ; andi.w #mask,D0`): the low word of the draw, masked.
    pub fn rand_and(&mut self, mask: u16) -> u32 {
        (self.random() & mask) as u32
    }

    /// Masked draw (`jsr rng ; andi.w #mask,D0`), without a register.
    pub fn rand_mask(&mut self, mask: u16) -> u16 {
        self.random() & mask
    }

    /// `jsr rng ; andi.b #mask,D0b`: the low byte of the draw, masked.
    pub fn rand_and_b(&mut self, mask: u8) -> u8 {
        self.random() as u8 & mask
    }

    /// `rand_and_b`, but returns the word D0.w that it leaves: the high byte of the draw and the masked low byte
    /// (re-read by the freeze shard, index of its delays).
    pub fn rand_and_b_word(&mut self, mask: u8) -> u16 {
        let rng = self.random();
        rng & 0xff00 | (rng as u8 & mask) as u16
    }

    /// State `+4` of the handler (`jmp (table,PC,D0.w)` of the original; `_table`: its jump table).
    pub fn state(&mut self, _table: u32) -> u8 {
        self.u8(0x4)
    }

    /// Releases the shot.
    pub fn free_pshl(&mut self) {
        self.free(&pools::PSHL);
    }

    /// Releases the effect.
    pub fn free_peff(&mut self) {
        self.free(&pools::PEFF);
    }

    /// Copies the position (integer X, Z, Y) of another object.
    pub fn copy_pos(&mut self, from: Obj) {
        for off in [0x8, 0xc, 0x10] {
            let v = from.u16(self.b, off);
            self.set16(off, v);
        }
    }

    /// Long from the program ROM (tables read relative to the PC).
    pub fn rom32(&mut self, addr: u32) -> u32 {
        (self.rom16(addr) as u32) << 16 | self.rom16(addr + 2) as u32
    }

    /// Starts script `n` of animation set `set` (`anim_start_indexed`).
    pub fn anim(&mut self, set: u32, n: u32) {
        self.anim_start(set, n as u16);
    }
}

/// Copies the orientation and position of the linked player (`+0x50`); returns the player.
pub fn follow_owner<R: Bench>(x: &mut Cx<'_, R>) -> Obj {
    let owner = x.linked(f::OWNER);
    let flip = owner.u8(x.b, f::FLIP);
    x.set8(f::FLIP, flip);
    x.copy_pos(owner);
    owner
}

/// Offset (X, Z) read from a table of ROM words (PC-relative read), row `row`.
/// Original glitch: D2 keeps the Z offset (leftover re-read by other handlers, `dispatch::Leftovers`).
pub fn offset<R: Bench>(x: &mut Cx<'_, R>, table: u32, row: u32) -> (u16, u16) {
    let dx = x.rom16(table + row);
    let dz = x.rom16(table + row + 2);
    *x.c.d2() = dz as u32;
    (dx, dz)
}

/// Leftovers of the terrain search (0x24a8 / 0x24b0, `video::terrain_regs`): D2 (number), D3 (cell index), D4.w
/// (column). Original glitch: following handlers re-read them (`dispatch::Leftovers`; D3: `boss11_scene42_mort`, D2:
/// bounce of the freeze shard).
pub fn leftovers_terrain<R: Bench>(x: &mut Cx<'_, R>, t: &crate::video::Terrain) {
    if t.fixed_map {
        *x.c.d3() = t.index as u32;
    } else {
        *x.c.d3() = *x.c.d3() & 0xffff_0000 | t.index as u32;
        *x.c.d2() = 0;
    }
    *x.c.d4() = *x.c.d4() & 0xffff_0000 | t.column as u32;
    *x.c.d2() = *x.c.d2() & 0xffff_0000 | t.number as u32;
}
