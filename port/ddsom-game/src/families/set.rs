//! Scenery objects (SET class, pool of 32 objects of 0x50 bytes, table 0x39eac): types 0, 1 and
//! 3 to 14. SET objects do not appear in the sorted lists (see `docs/objets.md`).

use crate::families::cx::Bench;
use super::cx::{Cx, Gx};
use crate::objects::Obj;
use crate::objects::pools;
use crate::objects::Current;
use crate::machine::Machine;

/// Type 8 (0xcc25a), shared by the SET (8), SOBJ (4) and EFFCT (18) tables: scene controller
/// generator. On the first frame it waits; on the next it creates a SET object of
/// type 2 (the controller, of variant `+0x2f`) then goes to state 6. In states 4 and 6 it
/// releases itself in its class's pool (EFFCT, SET or SOBJ according to `+0x28`).
///
/// Variant `+3`: the original's jump table has only one entry (variant 1 falls onto the same
/// target), variant 2 jumps into the middle of a `jsr` and beyond into data: outside the contract, the
/// rewrite treats every variant as 0 (the fuzz only draws 0 and 1).
mod scene_spawner {
    use super::*;

    /// Table (program space) of release routines, indexed by `(class - 0xe) * 2` bytes.
    const FREE_BY_CLASS: u32 = 0xcc2b8;
    const WAIT: u8 = 0;
    const SPAWN: u8 = 2;
    const SCENE_CONTROLLER: u8 = 2;
    /// Creator's parameter: variant of the controller to create.
    const VARIANT_PARAM: u32 = 0x2f;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            WAIT => x.add8(0x4, 2),
            SPAWN => {
                let Some(child) = x.alloc(&pools::SET) else { return };
                child.set_u8(x.b, 0, 1);
                child.set_u8(x.b, 2, SCENE_CONTROLLER);
                let variant = x.u8(VARIANT_PARAM);
                child.set_u8(x.b, 3, variant);
                x.set8(0x4, 6);
            }
            _ => {
                let class = x.u8(0x28) as u32;
                let offset = (class.wrapping_sub(0xe) as u16).wrapping_mul(2) as i16;
                let at = FREE_BY_CLASS.wrapping_add(offset as i32 as u32);
                let free = (x.rom16(at) as u32) << 16 | x.rom16(at + 2) as u32;
                let pool = pools::POOLS
                    .into_iter()
                    .find(|p| p.free_routine == free)
                    .unwrap_or_else(|| unreachable!("{free:#x}: not a pool release routine (class {class:#x})"));
                x.free(pool);
            }
        }
    }
}

/// Helpers shared by the types of the SET class.
mod util {
    use super::*;
    use crate::mem::{rd16, wr16, A5};

    /// First player record (A5 + 0x5de), four players spaced 0x200 bytes apart.
    pub const PLAYERS: u32 = A5 + 0x5de;
    pub const PLAYER_STRIDE: u32 = 0x200;
    /// Fields of a player: active (byte), hit points (word).
    pub const P_ACTIVE: u32 = 0x0;
    pub const P_LIFE: u32 = 0x62;
    /// A5 variable: the game is in progress (non-zero).
    pub const IN_GAME: u32 = 0x35;

    pub fn players() -> impl Iterator<Item = Obj> {
        (0..4).map(|i| Obj(PLAYERS + PLAYER_STRIDE * i))
    }

    /// Player present in the game: slot occupied and non-zero hit points.
    pub fn alive<R: Bench>(x: &mut Cx<'_, R>, p: Obj) -> bool {
        p.u8(x.b, P_ACTIVE) != 0 && p.u16(x.b, P_LIFE) != 0
    }

    /// Advances each component (red, green, blue: one nibble each) of a 4+4+4-bit color
    /// by one notch toward its maximum, and forces the brightness nibble to 0xf.
    pub fn brighten<R: Bench>(x: &mut Cx<'_, R>, addr: u32, count: u32) {
        for i in 0..count {
            let a = addr + 2 * i;
            let c = rd16(x.b, a);
            let step = |mask: u16, one: u16| {
                let v = c & mask;
                if v == mask { v } else { v + one }
            };
            wr16(x.b, a, 0xf000 + step(0xf, 1) + step(0xf0, 0x10) + step(0xf00, 0x100));
        }
    }
}

/// Type 9 (0xcc2c8): inert object. Suspended (`+0x40`), it does nothing; otherwise state 0 goes to
/// state 2, which does nothing either (the original tests a collision there with player A6 of which
/// no result is used); states 4 and 6 release it.
mod inert {
    use super::*;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(0x40) != 0 {
            return;
        }
        match x.u8(0x4) {
            0 => x.add8(0x4, 2),
            2 => {}
            _ => x.free(&pools::SET),
        }
    }
}

/// Type 11 (0xcdcaa): end of level of the bonus game (h). When the game is in progress and
/// variable `A5+0x556` exceeds 0x320, it waits for all living players to be in state
/// 0x200 (+4/+5) with no substate, then puts them back in state 0x80000, turns off their weapon (`+0x8a`) and
/// numbers their order (`+0x7d`), sets `A5+0x2c` and goes to state 4 (release on the next frame).
mod level_end {
    use super::*;
    use util::*;

    const PROGRESS: u32 = 0x556;
    const PROGRESS_LIMIT: i16 = 0x320;
    const RESET_STATE: u32 = 0x80000;
    const END_FLAGS: u32 = 0x18000000;
    const P_STATE: u32 = 0x4;
    const P_SUBSTATE: u32 = 0x6;
    const P_ORDER: u32 = 0x7d;
    const P_WEAPON: u32 = 0x8a;
    const P_WEAPON_OBJ: u32 = 0x86;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                wait_players(x);
            }
            2 => wait_players(x),
            _ => x.free(&pools::SET),
        }
    }

    fn wait_players<R: Bench>(x: &mut Cx<'_, R>) {
        if x.a5_u8(IN_GAME) == 0 || (x.a5_u16(PROGRESS) as i16) <= PROGRESS_LIMIT {
            return;
        }
        x.set_a5_u8(0x43, 0);
        x.set_a5_u16(0x51e, 0);
        x.set_a5_u8(0x4ff, 4);
        for p in players() {
            if alive(x, p) && (p.u16(x.b, P_STATE) != 0x200 || p.u8(x.b, P_SUBSTATE) != 0) {
                return;
            }
        }
        let mut order = 0;
        for p in players() {
            if !alive(x, p) {
                continue;
            }
            p.set_u32(x.b, P_STATE, RESET_STATE);
            p.set_u8(x.b, 0x66, 0);
            p.set_u8(x.b, 0x15c, 0);
            if p.u8(x.b, P_WEAPON) != 0 {
                p.set_u8(x.b, P_WEAPON, 0);
                let weapon = Obj(p.u32(x.b, P_WEAPON_OBJ));
                weapon.set_u8(x.b, 5, 8);
                weapon.set_u16(x.b, 6, 0);
            }
            p.set_u8(x.b, P_ORDER, order);
            order += 1;
        }
        x.set_a5_u32(0x2c, END_FLAGS);
        x.add8(0x4, 2);
    }
}

/// Type 12 (0xcdd70): palette fade to white (h). State 0 notes `+0x3c` = 0xf (not read back);
/// state 2, on each frame, brightens by one notch the colors of four blocks of palette RAM
/// (4 palettes of 0x91c100, then 0x91c720, 0x91c800 and 0x91cc00: 32 colors each). Another
/// object stops it by changing its state (4 or 6: release).
mod fade_white {
    use super::*;

    const PALETTE_BLOCKS: [(u32, u32); 4] = [(0x91c100, 128), (0x91c720, 32), (0x91c800, 32), (0x91cc00, 32)];

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set16(0x3c, 0xf);
            }
            2 => {
                for (addr, count) in PALETTE_BLOCKS {
                    util::brighten(x, addr, count);
                }
            }
            _ => x.free(&pools::SET),
        }
    }
}

/// Type 13 (0xcdef6): palette animation (h). The variant picks the destination palette
/// (0x91c000 + 32 bytes per variant); a table of 17 palettes in ROM (0x291f00, or 0x291f00 +
/// 0x360 when `A5+0x70` is non-zero) is copied, from the last to the first, every 8
/// frames. After the last one it releases itself.
mod palette_anim {
    use super::*;
    use crate::mem::{rd32, wr32};

    const PALETTE_RAM: u32 = 0x91c000;
    const PALETTE_BYTES: u32 = 32;
    const TABLES: u32 = 0x291f00;
    const ALT_TABLE_OFFSET: u32 = 0x360;
    const ALT_TABLE_FLAG: u32 = 0x70;
    const STEPS: u8 = 0x10;
    const PERIOD: u8 = 8;
    /// Fields: destination, source table, countdown, current palette.
    const DEST: u32 = 0x20;
    const SOURCE: u32 = 0x24;
    const TICK: u32 = 0x29;
    const STEP: u32 = 0x2a;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                let variant = x.u8(0x3) as u16;
                x.set32(DEST, PALETTE_RAM + ((variant << 5) as i16 as i32 as u32));
                x.set8(TICK, 1);
                let alt = x.a5_u16(ALT_TABLE_FLAG) != 0;
                x.set32(SOURCE, TABLES + if alt { ALT_TABLE_OFFSET } else { 0 });
                x.set8(STEP, STEPS);
            }
            2 => {
                let t = x.u8(TICK).wrapping_sub(1);
                x.set8(TICK, t);
                if t != 0 {
                    return;
                }
                x.set8(TICK, PERIOD);
                let step = ((x.u8(STEP) as u16) << 5) as i16 as i32 as u32;
                let dest = x.u32(DEST);
                let src = x.u32(SOURCE).wrapping_add(step);
                for i in (0..PALETTE_BYTES).step_by(4) {
                    let v = rd32(x.b, src + i);
                    wr32(x.b, dest + i, v);
                }
                let s = x.u8(STEP).wrapping_sub(1);
                x.set8(STEP, s);
                if (s as i8) < 0 {
                    x.add8(0x4, 2);
                    x.free(&pools::SET);
                }
            }
            4 => {
                x.add8(0x4, 2);
                x.free(&pools::SET);
            }
            _ => x.free(&pools::SET),
        }
    }
}

/// Type 14 (0xcdf86): end-of-scene trigger (h). When the game is in progress and
/// camera abscissa `A5+0x506` has reached 0x1c80, it puts all living players in state 2
/// (player's byte `+0`), raises `A5+0x42` and `A5+0x4a`, sets `A5+0x2c` to 0x10000000 and goes to
/// state 4 (release). Variant `+3` has no effect: the original's two branches are
/// identical.
mod scene_trigger {
    use super::*;
    use util::*;

    const CAMERA_X: u32 = 0x506;
    const CAMERA_LIMIT: u16 = 0x1c80;
    const PLAYER_MODE_DONE: u8 = 2;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                trigger(x);
            }
            2 => trigger(x),
            _ => x.free(&pools::SET),
        }
    }

    fn trigger<R: Bench>(x: &mut Cx<'_, R>) {
        if x.a5_u8(IN_GAME) == 0 || x.a5_u16(CAMERA_X) < CAMERA_LIMIT {
            return;
        }
        let mut count = 0;
        for p in players() {
            if alive(x, p) {
                p.set_u8(x.b, P_ACTIVE, PLAYER_MODE_DONE);
                count += 1;
            }
        }
        if count == 0 {
            return;
        }
        x.add8(0x4, 2);
        x.set_a5_u8(0x42, 1);
        x.set_a5_u8(0x4a, 1);
        x.set_a5_u32(0x2c, 0x10000000);
    }
}

/// Types 4 (0xc935a) and 6 (0xcafec): scenery animation by rewriting tile cells (h).
///
/// Variant `+3` picks a 16-byte entry from a ROM table (program space):
/// script address (`+0x30`), address of the tile map to modify in video memory
/// (`+0x34`), non-zero word `+0x38` to loop. The script is a sequence of blocks: duration (word,
/// negative = end), number of columns, number of rows, then one long per cell. Each cell of the
/// map becomes `(attr & 0xff80) + long`: the tile code is replaced, the high bits of
/// the attribute (palette, flips) are kept. Rows are spaced 0x40 bytes apart
/// (0x20 if `+0x2f` is non-zero). At the end of the script, the object restarts from state 0 if `+0x38` is
/// non-zero, otherwise goes to state 4 (release). Type 4 stops during the game pause and when
/// the object is suspended; type 6 ignores the pause, but leaves untouched the cells whose
/// first word is 0x1800.
mod tile_anim {
    use super::*;
    use crate::mem::{rd16, rd32, wr32};

    /// An entry of the script table.
    const ENTRY_SIZE_SHIFT: u16 = 4;
    const ENTRY_SCRIPT: u32 = 0;
    const ENTRY_TILEMAP: u32 = 4;
    const ENTRY_LOOP: u32 = 8;
    /// Champs de l'objet.
    const SCRIPT: u32 = 0x30;
    const TILEMAP: u32 = 0x34;
    const LOOP: u32 = 0x38;
    const TIMER: u32 = 0x3a;
    const SMALL_ROWS: u32 = 0x2f;
    const ROW_BYTES: u32 = 0x40;
    const ROW_BYTES_SMALL: u32 = 0x20;
    const ATTR_KEEP: u32 = 0xff80;
    const TRANSPARENT: u16 = 0x1800;

    /// Script table of type 4 and type 6 (program space).
    const TABLE_4: u32 = 0xc93a2;
    const TABLE_6: u32 = 0xcb024;
    const GAME_PAUSED: u32 = 0x3d;

    pub fn run4<R: Bench>(x: &mut Cx<'_, R>) {
        if x.a5_u8(GAME_PAUSED) != 0 || x.u8(0x40) != 0 {
            return;
        }
        run(x, TABLE_4, false);
    }

    pub fn run6<R: Bench>(x: &mut Cx<'_, R>) {
        run(x, TABLE_6, true);
    }

    fn run<R: Bench>(x: &mut Cx<'_, R>, table: u32, skip_transparent: bool) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set16(TIMER, 0);
                let at = table.wrapping_add((((x.u8(0x3) as u16) << ENTRY_SIZE_SHIFT) as i16) as u32);
                let long = |x: &mut Cx<'_, R>, off: u32| (x.rom16(at + off) as u32) << 16 | x.rom16(at + off + 2) as u32;
                let (script, tilemap, looping) = (long(x, ENTRY_SCRIPT), long(x, ENTRY_TILEMAP), x.rom16(at + ENTRY_LOOP));
                x.set32(SCRIPT, script);
                x.set32(TILEMAP, tilemap);
                x.set16(LOOP, looping);
                step(x, skip_transparent);
            }
            2 => step(x, skip_transparent),
            _ => x.free(&pools::SET),
        }
    }

    /// One step: waits for the end of the current block's duration, then applies the next block.
    fn step<R: Bench>(x: &mut Cx<'_, R>, skip_transparent: bool) {
        let t = x.u16(TIMER).wrapping_sub(1);
        x.set16(TIMER, t);
        if (t as i16) >= 0 {
            return;
        }
        let mut script = x.u32(SCRIPT);
        let map = x.u32(TILEMAP);
        let duration = rd16(x.b, script);
        script += 2;
        x.set16(TIMER, duration);
        if (duration as i16) < 0 {
            if x.u16(LOOP) != 0 {
                x.set8(0x4, 0);
            } else {
                x.add8(0x4, 2);
            }
            return;
        }
        let cols = rd16(x.b, script).wrapping_sub(1);
        let rows = rd16(x.b, script + 2).wrapping_sub(1);
        script += 4;
        let stride = if x.u8(SMALL_ROWS) != 0 { ROW_BYTES_SMALL } else { ROW_BYTES };
        let mut row_start = map;
        for _ in 0..=rows as u32 {
            for col in 0..=cols as u32 {
                let cell = row_start + 4 * col;
                let tile = rd32(x.b, script);
                script += 4;
                let keep = skip_transparent && rd16(x.b, script - 4) == TRANSPARENT;
                if !keep {
                    let attr = rd16(x.b, cell + 2) as u32 & ATTR_KEEP;
                    wr32(x.b, cell, attr.wrapping_add(tile));
                } else {
                    let old = rd32(x.b, cell);
                    wr32(x.b, cell, old);
                }
            }
            row_start += stride;
        }
        x.set32(SCRIPT, script);
    }
}

/// Type 1 (0xbc518): leader of enemy waves (h), active only in a game (`A5+0x35`).
///
/// A chief (the owner `+0x46`, an ENEMY object) calls for waves of three enemies. The leader
/// first waits 180 frames (`+0x22` counts down from 0xf0 to 0x3c, then it locks the camera through
/// 0x2860), then chains the substates `+5`:
///
/// | `+5` | role |
/// |---|---|
/// | 0 | wait of 180 frames, then camera lock |
/// | 2 | launches a wave (`spawn_wave`), then waits for a delay drawn from a table; goes to 4 when `+0x21` or `+0x20` equals 2 (waves defeated, h) or after 8 waves (`+0x25`); notifies the chief through `+0x24` = 1 |
/// | 4 | waits for the chief's counter `+0x2a` to equal 1 |
/// | 6 | waits for `+0x30` = 0; if `+0x31` is non-zero, subtracts one from the chief's counter and finishes, otherwise launches the final wave |
/// | 8 | final wave: first enemy of variant 2, wave result 0xff |
/// | 0xa, 0xc | wait for the end of the wave then 300-frame delay |
/// | 0xe | waits for `+0x30` = 0, subtracts one from the chief's counter and finishes |
///
/// The main state then goes to 4 then 6 (release).
mod wave_leader {
    use super::*;
    use crate::mem::{wr8, A5};

    /// Delays between two waves (bytes, program space), indexed by a 4-bit draw.
    const DELAYS: u32 = 0xbc63e;
    const CAMERA_LOCK_PARAM: u32 = 0x3c;
    /// Free slots of the ENEMY pool (A5+); three are needed per wave.
    const ENEMY_FREE: u32 = 0x7e62;
    const ENEMIES_PER_WAVE: i16 = 3;
    const WAIT_START: u8 = 0xf0;
    const WAIT_END: u8 = 0x3c;
    const FINAL_WAIT: u16 = 0x12c;
    const MAX_WAVES: u8 = 8;
    const KIND_WING: u8 = 0x13;
    const KIND_LEAD: u8 = 0x14;
    const FINAL_VARIANT: u8 = 2;
    /// Starting coordinates of the wave leader relative to the camera (`A5+0x506`).
    const CAMERA_X: u32 = 0x506;
    const SPAWN_OFFSET_X: u16 = 0x200;
    const SPAWN_Y: u16 = 0x200;
    /// Global variable (A5 - 0x5ac6): the wave's verdict for the rest of the game (h).
    const WAVE_RESULT: u32 = A5 - 0x5ac6;
    const WAVE_RESULT_FINAL: u8 = 0xff;
    /// Leader's fields.
    const DEFEATED_A: u32 = 0x20;
    const DEFEATED_B: u32 = 0x21;
    const DEFEATED_TARGET: u8 = 2;
    const WAIT: u32 = 0x22;
    const BUSY: u32 = 0x24;
    const WAVE_COUNT: u32 = 0x25;
    const SIDE: u32 = 0x26;
    const LIMIT: u32 = 0x30;
    const FLAG: u32 = 0x31;
    const DIFF: u32 = 0x32;
    /// Chief: address (`+0x46`, also copied into the created enemies), end-of-wave flag
    /// (`+0x24`) and counter of living enemies (`+0x2a`).
    /// Addresses (words) of the three enemies of the last wave.
    const WINGS: u32 = 0x2a;
    const BOSS: u32 = 0x46;
    const BOSS_STATE: u32 = 0x4;
    const BOSS_STATE_READY: u8 = 4;
    const BOSS_DONE: u32 = 0x24;
    const BOSS_COUNT: u32 = 0x2a;
    /// Fields of the created enemies.
    const E_LINK_A: u32 = 0xac;
    const E_LINK_B: u32 = 0xae;
    const E_COUNT: u32 = 0xb0;
    const E_SIDE: u32 = 0xb1;
    const E_WING_SIDE: u32 = 0xbc;
    const E_PARENT: u32 = 0x42;
    const E_ANCHOR: u32 = 0x50;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        if x.a5_u8(util::IN_GAME) == 0 {
            return;
        }
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set16(DEFEATED_A, 0);
                x.set16(BUSY, 0);
                x.set8(0x5, 0);
                x.set16(0x6, 0);
                x.set8(LIMIT, 0);
                x.set8(FLAG, 0);
                x.set8(DIFF, 0);
                x.set8(SIDE, 0xff);
                x.set8(WAIT, WAIT_START);
                substate(x);
            }
            2 => substate(x),
            4 => x.add8(0x4, 2),
            _ => x.free(&pools::SET),
        }
    }

    fn substate<R: Bench>(x: &mut Cx<'_, R>) {
        let boss = x.linked(BOSS);
        match x.u8(0x5) {
            0 => {
                let w = x.u8(WAIT).wrapping_sub(1);
                x.set8(WAIT, w);
                if w > WAIT_END {
                    return;
                }
                x.add8(0x5, 2);
                crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::B, CAMERA_LOCK_PARAM as u16);
            }
            2 => {
                let (a, b) = (x.u8(DEFEATED_A), x.u8(DEFEATED_B));
                let finished = |x: &mut Cx<'_, R>| {
                    x.add8(0x5, 2);
                    boss.set_u16(x.b, BOSS_DONE, 1);
                    x.set8(DIFF, a.wrapping_sub(b));
                };
                if b == DEFEATED_TARGET {
                    x.add8(0x5, 2);
                    boss.set_u16(x.b, BOSS_DONE, 1);
                } else if a == DEFEATED_TARGET {
                    if boss.u8(x.b, BOSS_STATE) == BOSS_STATE_READY {
                        finished(x);
                    }
                } else if x.u8(BUSY) != 0 {
                } else if x.u8(WAVE_COUNT) == MAX_WAVES {
                    finished(x);
                } else if x.u8(WAIT) != 0 {
                    { let w = x.u8(WAIT); x.set8(WAIT, w - 1); }
                } else if spawn_wave(x).is_some() {
                    let rng = x.random();
                    let delay = x.rom8(DELAYS + (rng as u32 & 0xf));
                    x.set8(WAIT, delay);
                }
            }
            4 => {
                if boss.u16(x.b, BOSS_COUNT) == 1 {
                    x.add8(0x5, 2);
                }
            }
            6 => {
                if x.u8(LIMIT) != 0 {
                    return;
                }
                let flag = x.u8(FLAG);
                wr8(x.b, WAVE_RESULT, flag);
                if flag != 0 {
                    release_one(x, boss);
                    x.set_a5_u8(0x42, 1);
                    x.add8(0x4, 2);
                } else {
                    x.add8(0x5, 2);
                    final_wave(x);
                }
            }
            8 => final_wave(x),
            0xa => {
                if x.u8(BUSY) == 0 {
                    x.set16(WAIT, FINAL_WAIT);
                    x.add8(0x5, 2);
                }
            }
            0xc => {
                if x.u8(FLAG) == 0 {
                    let w = x.u16(WAIT).wrapping_sub(1);
                    x.set16(WAIT, w);
                    if w != 0 {
                        return;
                    }
                    wr8(x.b, WAVE_RESULT, 1);
                }
                x.set_a5_u8(0x42, 1);
                x.add8(0x5, 2);
            }
            0xe => {
                if x.u8(LIMIT) == 0 {
                    release_one(x, boss);
                    x.add8(0x4, 2);
                }
            }
            _ => {}
        }
    }

    /// The leader counts one enemy less.
    fn release_one<R: Bench>(x: &mut Cx<'_, R>, boss: Obj) {
        let n = boss.u16(x.b, BOSS_COUNT).wrapping_sub(1);
        boss.set_u16(x.b, BOSS_COUNT, n);
    }

    /// Final wave: the leader is of variant 2 and the wave verdict goes to 0xff.
    fn final_wave<R: Bench>(x: &mut Cx<'_, R>) {
        if let Some(lead) = spawn_wave(x) {
            lead.set_u8(x.b, 3, FINAL_VARIANT);
            x.add8(0x5, 2);
            wr8(x.b, WAVE_RESULT, WAVE_RESULT_FINAL);
        }
    }

    /// Creates three enemies at once: a leader (type 0x14, attached to the wave leader by
    /// `+0x50`, placed to the right of the camera) and two wingmen (type 0x13) attached to it.
    /// `None` when the ENEMY pool has fewer than three free slots. (The original then sorts the
    /// three addresses with a loop that changes nothing.)
    fn spawn_wave<R: Bench>(x: &mut Cx<'_, R>) -> Option<Obj> {
        if (x.a5_u16(ENEMY_FREE) as i16) < ENEMIES_PER_WAVE {
            return None;
        }
        let mut slots = [Obj(0); 3];
        for (i, slot) in slots.iter_mut().enumerate() {
            // Original glitch: the allocation failure is not tested (see `Pool::alloc_unchecked`).
            *slot = x.alloc_unchecked(&pools::ENEMY);
            slot.set_u8(x.b, 0, 1);
            x.set16(WINGS + 2 * i as u32, slot.0 as u16);
        }
        let low = |o: Obj| o.0 as u16;
        let [lead, wing_a, wing_b] = slots;
        let boss = x.u16(BOSS);
        lead.set_u16(x.b, BOSS, boss);
        for (wing, side, link) in [(wing_a, 0, E_LINK_A), (wing_b, 1, E_LINK_B)] {
            wing.set_u8(x.b, 0, 1);
            wing.set_u8(x.b, E_WING_SIDE, side);
            wing.set_u8(x.b, 3, 0);
            wing.set_u8(x.b, 0x2f, 1);
            wing.set_u8(x.b, 2, KIND_WING);
            wing.set_u16(x.b, E_PARENT, low(lead));
            wing.set_u16(x.b, BOSS, boss);
            lead.set_u16(x.b, link, low(wing));
        }
        lead.set_u8(x.b, E_COUNT, 3);
        let owner = x.linked(BOSS);
        let n = owner.u16(x.b, BOSS_COUNT).wrapping_add(3);
        owner.set_u16(x.b, BOSS_COUNT, n);
        lead.set_u8(x.b, 0, 1);
        lead.set_u8(x.b, 2, KIND_LEAD);
        lead.set_u8(x.b, 3, 0);
        lead.set_u16(x.b, E_ANCHOR, low(x.o));
        let camera = x.a5_u16(CAMERA_X);
        lead.set_u16(x.b, 8, camera.wrapping_add(SPAWN_OFFSET_X));
        lead.set_u32(x.b, 0xc, 0);
        lead.set_u16(x.b, 0x10, SPAWN_Y);
        // arrival side: drawn at random the first time, then alternated
        let mut side = x.u8(SIDE);
        if (side as i8) < 0 {
            let rng = x.random();
            side = ((rng as u32 & 1) << 5) as u8;
            x.set8(SIDE, side ^ 0x20);
        }
        lead.set_u8(x.b, E_SIDE, side);
        let y = lead.u8(x.b, 0x11).wrapping_add(side.wrapping_add(0x10));
        lead.set_u8(x.b, 0x11, y);
        x.add8(DEFEATED_A, 1);
        x.set8(BUSY, 1);
        x.add8(WAVE_COUNT, 1);
        Some(lead)
    }
}

/// Fade to black of the doors (SET 7 [`exit_gate`], SET 29 `set_b::door_b`): subroutine 0x1460 cedes control
/// during its 16 steps. The door calls it like a task subroutine (batch E): the continuation, a [`Step`] that
/// carries the door (A0), is stored in the task's pending calls; at the end of the fade, the task resumes there,
/// then returns to the dispatch loop (its pending call, `dispatch::Step::Dispatch`).
pub mod exit_fading {
    use super::super::set_b;
    use super::{Bench, Cx};
    use crate::modes::{Flow, Sequence};
    use crate::objects::Obj;

    /// Brightness fade parameters (`tasks::fading`): pages (bit k: page k) and delay between two steps.
    pub(super) const PAGES: u32 = 0x48e;
    const DELAY: u32 = 0x490;

    /// Where the task resumes after the fade (return address of the original's call to 0x1460).
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Step {
        /// Porte de sortie (SET 7), changement de zone (0xcb896) : `exit_gate::after_fade`.
        Zone(Obj),
        /// Exit door (SET 7), next scene (0xcb9c0): `exit_gate::next_scene`.
        Scene(Obj),
        /// Porte de passage (SET 29, 0xd8f06) : `set_b::door_b::after_fade`.
        Passage(Obj),
    }

    impl Step {
        pub const fn address(self) -> u32 {
            match self {
                Step::Zone(_) => 0xcb896,
                Step::Scene(_) => 0xcb9c0,
                Step::Passage(_) => 0xd8f06,
            }
        }

        /// The end of the door's handler, then its return (`rts`) into the dispatch loop.
        pub fn step(self, s: &mut crate::scene::Ctx) -> Flow {
            let (Step::Zone(door) | Step::Scene(door) | Step::Passage(door)) = self;
            crate::dispatch::resume_handler(s.b, Some(door), |b, a| {
                let mut x = Cx { c: a, b, o: door };
                match self {
                    Step::Zone(_) => super::exit_gate::after_fade(&mut x),
                    Step::Scene(_) => super::exit_gate::next_scene(&mut x),
                    Step::Passage(_) => set_b::door_b::after_fade(&mut x),
                }
            });
            Flow::Done
        }
    }

    /// Full fade to black (`jsr 0x1460`, delay 2, four pages), followed by `next`. The fade yields: the handler must
    /// return at once (tail call), and the dispatch loop that called it gives up; the task will resume through the
    /// fade's steps, then `next`, then the loop (`dispatch::Step::Dispatch`).
    pub fn fade_to_black<R: Bench>(x: &mut Cx<'_, R>, next: fn(Obj) -> Step) {
        x.set_a5_u16(DELAY, 2);
        x.set_a5_u8(PAGES, 0xf);
        // The fade runs as a subroutine of the task (`Sequence::subroutine`); the leftovers (`dispatch::Leftovers`)
        // go to the game's state for it, and come back (an empty fade returns at once and runs `next`, which resumes
        // the door through `dispatch::resume_handler`).
        let door = x.o;
        let leftovers = crate::dispatch::Leftovers { d2: *x.c.d2(), d3: *x.c.d3(), d4: *x.c.d4(), d5: *x.c.d5() };
        crate::tasks::table::table(x.b).leftovers = leftovers;
        let mut s = crate::scene::Ctx::new(&mut *x.b);
        s.subroutine(crate::tasks::fading::descent, next(door));
        let l = crate::tasks::table::table(x.b).leftovers;
        (*x.c.d2(), *x.c.d3(), *x.c.d4(), *x.c.d5()) = (l.d2, l.d3, l.d4, l.d5);
    }
}

/// Type 7 (0xcb29c): zone exit door (h). When a player stays 24 frames inside the door's
/// footprint, they "trigger" the exit (state imposed by the zone); when all living players are
/// ready, the zone changes. It only acts if `+0x2f` is zero, outside pause and with no transition in progress
/// (`A5+0x3d`, `A5+0x50`, `A5+0x42`), and if no other door has taken over (`A5+0x52`).
///
/// | `+4` | role |
/// |---|---|
/// | 0 | initialization from the door table (16 bytes per variant `+3`, program space) |
/// | 2 | watches the four players; `+0x42..+0x45` count the frames spent in the footprint; `+0x16` counts the ready players |
/// | 4 | return from the zone: reloads the scene (maps, palettes, saved camera, players repositioned) |
/// | 6 | release |
///
/// When all living players are ready, the zone changes according to the exit kind `+0x48`:
///
/// | `+0x48` | sortie |
/// |---|---|
/// | 0 | entry into a side zone: fade to black, the door saves the camera, suspends the scenery and the objects (`+0x40`), loads scene `+0x4f`, and waits in state 4 for the return |
/// | 1 | return from a side zone: fade, players released, the suspended objects wake up (those whose counter drops below zero are released) |
/// | 2 | next scene `+0x4f`: fade, then the game goes back to phase 8 (scene entry) |
/// | 3 | fin de partie (`A5+0x42`) |
///
/// The fade to black (0x1460) cedes control for 16 frames: the task resumes at steps
/// [`exit_fading::Step::Zone`] (0xcb896) and [`exit_fading::Step::Scene`] (0xcb9c0).
mod exit_gate {
    use super::super::set_b;
    use super::*;
    use crate::mem::{rd16, rd8, wr8, A5};
    use cps2::Cps2Bus;

    // --- ROM table (program space) ---
    const GATES: u32 = 0xcb344;
    const GATE_SHIFT: u16 = 4;
    const GATE_X: u32 = 0;
    const GATE_Z: u32 = 2;
    const GATE_Y: u32 = 4;
    const GATE_EXIT_KIND: u32 = 6;
    const GATE_ZONE: u32 = 7;
    const GATE_SCENE: u32 = 8;
    const GATE_IGNORE_DEPTH: u32 = 9;
    const GATE_BONUS: u32 = 10;
    /// Player states imposed according to the zone index (`+0x46`), longs (4 bytes per index).
    const PLAYER_STATES: u32 = 0xcb998;

    // --- variables de A5 ---
    const IN_GAME: u32 = 0x35;
    const PAUSED: u32 = 0x3d;
    const TRANSITION: u32 = 0x42;
    const EXIT_LOCK: u32 = 0x50;
    const OWNER_GATE: u32 = 0x52;
    /// Variable (A5 - 0x5ac4): non-zero, nobody is judged to be in the footprint anymore.
    const NEAR_BLOCK: u32 = A5 - 0x5ac4;

    // --- door fields ---
    const PLAYER_COUNT_TIMER: u32 = 0x20;
    const ARRIVED: u32 = 0x16;
    const TRIGGERED: u32 = 0x17;
    const COUNTED: u32 = 0x1a;
    const NEAR_COUNTERS: u32 = 0x42;
    const EXIT_KIND: u32 = 0x48;
    const ZONE: u32 = 0x46;
    const NEXT_SCENE: u32 = 0x4f;
    const IGNORE_DEPTH: u32 = 0x1b;
    const BONUS: u32 = 0x24;

    // --- fields of a player ---
    const P_ACTIVE: u32 = 0x0;
    const P_STATE: u32 = 0x4;
    const P_X: u32 = 0x8;
    const P_Z: u32 = 0xc;
    const P_Y: u32 = 0x10;
    const P_LIFE: u32 = 0x62;
    const P_BLOCK_A: u32 = 0x66;
    const P_BLOCK_B: u32 = 0x15c;
    const P_BLOCK_C: u32 = 0x8a;
    const P_ZONES: u32 = 0x6c;
    const P_WEAPON_OBJ: u32 = 0x86;
    const P_RESET: u32 = 0x10f;

    /// Footprint of the door around its position (width 0x60, depth 0x40) and duration in frames.
    const REACH_X: u16 = 0x30;
    const REACH_Y: u16 = 0x20;
    const NEAR_FRAMES: i8 = 0x18;
    /// Player states recognized in the footprint (low byte ignored).
    const WALKING_STATES: [u32; 3] = [0x2000000, 0x2000600, 0x2000400];
    const PLAYER_STATE_IDLE: u32 = 0x2002010;
    const PLAYER_STATE_GONE: u16 = 0x402;
    const STATE_DONE: u8 = 2;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        refresh_player_count(x);
        if !gate_active(x.b, x.o) {
            return;
        }
        match x.u8(0x4) {
            0 => init(x),
            2 => watch(x),
            4 => arrive(x),
            _ => x.free(&pools::SET),
        }
    }

    // --- common conditions (read only) --------------------------------------------------------------

    fn players() -> impl Iterator<Item = Obj> {
        util::players()
    }

    // --- changement de zone (0xcb844) ---------------------------------------------------------------

    /// Text rectangle cleared on zone change and on return (`text::CLEAR_BOX`, A4 = column, row,
    /// width, height).
    const CLEARED_TEXT_BOX: u32 = 0xcbc02;
    /// Accumulated exit bonus (byte at A5 - 0x5b20).
    const BONUS_TOTAL: u32 = A5 - 0x5b20;
    const P_FLAGS: u32 = 0x1;
    const P_HIDDEN: u8 = 0x2;
    const PLAYER_STATE_DEAD: u32 = 0x402_0000;
    /// Door variants that mark the scene (`A5+0x4a`).
    const MARKING_VARIANTS: [u8; 3] = [0x13, 0x15, 0x16];
    const SCENE: u32 = 0x34;
    const NEXT_SCENE_FLAG: u32 = 0x51;
    const GAME_OVER: u32 = 0x42;
    const PHASE: u32 = 0x2c;
    const PHASE_SCENE_ENTRY: u8 = 8;
    /// Longwords reset to zero before the next scene (the four players' weapons).
    const PLAYER_WEAPONS: [u32; 4] = [0x602, 0x802, 0xa02, 0xc02];
    /// Genres de sortie (`+0x48`).
    const KIND_RETURN: u8 = 1;
    const KIND_NEXT_SCENE: u8 = 2;
    const KIND_GAME_OVER: u8 = 3;

    /// All players are ready (0xcb844): bonus, palette, dead players hidden, then the exit.
    fn change_zone<R: Bench>(x: &mut Cx<'_, R>) {
        let bonus = rd8(x.b, BONUS_TOTAL).wrapping_add(x.u8(BONUS));
        wr8(x.b, BONUS_TOTAL, bonus);
        crate::text::clear_box(x.b, CLEARED_TEXT_BOX);
        for p in players() {
            if p.u8(x.b, P_ACTIVE) != 0 && p.u16(x.b, P_LIFE) == 0 {
                let flags = p.u8(x.b, P_FLAGS) | P_HIDDEN;
                p.set_u8(x.b, P_FLAGS, flags);
                p.set_u32(x.b, P_STATE, PLAYER_STATE_DEAD);
            }
        }
        match x.u8(EXIT_KIND) {
            KIND_NEXT_SCENE => {
                x.set_a5_u8(NEXT_SCENE_FLAG, 1);
                exit_fading::fade_to_black(x, exit_fading::Step::Scene)
            }
            KIND_GAME_OVER => x.set_a5_u8(GAME_OVER, 1),
            _ => {
                x.set_a5_u16(OWNER_GATE, 0);
                if MARKING_VARIANTS.contains(&x.u8(0x3)) {
                    x.set_a5_u8(0x4a, 1);
                }
                exit_fading::fade_to_black(x, exit_fading::Step::Zone)
            }
        }
    }

    /// After the fade of a zone change (0xcb896): return from a side zone, or entry.
    pub fn after_fade<R: Bench>(x: &mut Cx<'_, R>) {
        let scene = x.u8(NEXT_SCENE);
        x.set_a5_u8(SCENE, scene);
        if x.u8(EXIT_KIND) == KIND_RETURN {
            return leave_annex(x);
        }
        save_and_suspend(x);
        x.set16(0x4, 0x400);
        enter_annex(x);
    }

    /// After the end-of-scene fade (0xcb9c0): the game goes back to phase 8 on scene `+0x4f`.
    pub fn next_scene<R: Bench>(x: &mut Cx<'_, R>) {
        for off in PLAYER_WEAPONS {
            x.set_a5_u32(off, 0);
        }
        let scene = x.u8(NEXT_SCENE);
        x.set_a5_u8(SCENE, scene);
        x.set_a5_u8(EXIT_LOCK, 0);
        x.set_a5_u8(PHASE, PHASE_SCENE_ENTRY);
        x.set_a5_u8(PHASE + 1, 0);
        x.set_a5_u16(PHASE + 2, 0);
        x.set_a5_u16(OWNER_GATE, 0);
        set_b::reset_players_after_exit(x);
    }

    /// Pools suspended on entering a side zone, in the original's order (the players' effects,
    /// between the enemy shots and the players' shots, are released instead of suspended).
    const SUSPENDED: [&set_b::Pool; 8] = [
        &set_b::SET_POOL, &set_b::SOBJ_POOL, &set_b::ENEMY_POOL, &set_b::EFFCT_POOL, &set_b::BOX_POOL,
        &set_b::ITEM_POOL, &set_b::BOSS_POOL, &set_b::ESHL_POOL,
    ];
    const PEFF_POOL: set_b::Pool = set_b::Pool { base: A5 + 0x179e, size: 0x60, count: 48 };
    const PSHL_POOL: set_b::Pool = set_b::Pool { base: A5 + 0x10de, size: 0x60, count: 18 };
    /// Scenery of type 0x13 released on entering the doors of variant 8 and 0xa.
    const FREED_KIND: u8 = 0x13;
    /// Cameras imposed by variants 8 and 0xa (`+0x30..+0x3c`).
    const CAMERA_VARIANT_8: [u32; 4] = [0x2a0_0000, 0x108_0000, 0x150_0000, 0];
    const CAMERA_VARIANT_A: [u32; 4] = [0x400_0000, 0x1fe_0000, 0x200_0000, 0x7b_0000];

    /// Saves the scene state in the door and suspends the world (0xcbcba).
    fn save_and_suspend<R: crate::families::cx::Bench>(x: &mut Cx<'_, R>) {
        set_b::save_camera(x);
        let v = x.a5_u16(0x52c);
        x.set16(0x22, v);
        let v = x.a5_u8(NEXT_SCENE_FLAG);
        x.set8(0x25, v);
        x.set_a5_u8(NEXT_SCENE_FLAG, 0);
        let v = x.a5_u8(0x58);
        x.set8(0x26, v);
        let camera = match x.u8(0x3) {
            0xa => Some(CAMERA_VARIANT_A),
            0x8 => Some(CAMERA_VARIANT_8),
            _ => None,
        };
        if let Some(camera) = camera {
            for (i, v) in camera.into_iter().enumerate() {
                x.set32(0x30 + 4 * i as u32, v);
            }
            for i in 0..set_b::SET_POOL.count {
                let o = Obj(set_b::SET_POOL.base + i * set_b::SET_POOL.size);
                if o.u8(x.b, 0) != 0 && o.u8(x.b, 2) == FREED_KIND {
                    pools::SET.release(x.b, o);
                }
            }
        }
        for pool in SUSPENDED {
            set_b::suspend(x, pool);
        }
        free_player_effects(x);
        set_b::suspend(x, &PSHL_POOL);
    }

    fn free_player_effects<R: Bench>(x: &mut Cx<'_, R>) {
        for i in 0..PEFF_POOL.count {
            let o = Obj(PEFF_POOL.base + i * PEFF_POOL.size);
            if o.u8(x.b, 0) != 0 {
                o.set_u8(x.b, 0x4, 6);
            }
        }
    }

    /// Entry into the side zone (0xcc0ca): cameras, scene script, palettes, players repositioned.
    fn enter_annex<R: Bench>(x: &mut Cx<'_, R>) {
        crate::families::scenery::restart_scene(x);
    }

    /// Return from a side zone (0xcbeba): players released, the door releases itself, the world wakes up.
    fn leave_annex<R: Bench>(x: &mut Cx<'_, R>) {
        for p in players() {
            if p.u8(x.b, P_ACTIVE) == 0 {
                continue;
            }
            p.set_u8(x.b, P_ACTIVE, 1);
            let flags = p.u8(x.b, P_FLAGS) & !P_HIDDEN;
            p.set_u8(x.b, P_FLAGS, flags);
            for off in [P_BLOCK_A, P_BLOCK_B, P_BLOCK_C, P_RESET] {
                p.set_u8(x.b, off, 0);
            }
            p.set_u8(x.b, 0x7e, 0x46);
        }
        x.set8(0x4, 6);
        // the other woken doors run immediately (the original's recursive `bsr`)
        set_b::resume_with(x, &set_b::SET_POOL, |x, o| {
            if o.u8(x.b, 2) == 7 {
                set_b::run_on(x, o, run);
            }
        });
        for pool in &SUSPENDED[1..] {
            set_b::resume(x, pool);
        }
        free_player_effects(x);
        set_b::resume(x, &PSHL_POOL);
    }

    /// State 4 (0xcba48): return from the side zone, the previous scene is reloaded.
    fn arrive<R: Bench>(x: &mut Cx<'_, R>) {
        crate::families::scenery::resume_scenery_lit(x, crate::families::scenery::Cameras::Return);
        x.set_a5_u16(OWNER_GATE, 0);
        crate::text::clear_box(x.b, CLEARED_TEXT_BOX);
        set_b::restore_camera(x);
        let v = x.u16(0x22);
        x.set_a5_u16(0x52c, v);
        let v = x.u8(0x25);
        x.set_a5_u8(NEXT_SCENE_FLAG, v);
        let v = x.u8(0x26);
        x.set_a5_u8(0x58, v);
        let music = x.u16(0x1c);
        crate::sound::play_with(x.b, 0xe17a4, crate::sound::Source { d4: Some(music as u32), ..Default::default() });
        for p in players() {
            if p.u8(x.b, P_ACTIVE) != 0 {
                place(x, p);
            }
        }
        x.set8(0x4, 6);
    }

    /// Placement row per variant (byte, program space) then X, Y offsets per row and per
    /// player (0x10 bytes per row, program space).
    const PLACEMENT_ROW: u32 = 0xcbb92;
    const PLACEMENT: u32 = 0xcbbe2;
    const PLAYER_STATE_WALK: u32 = 0x200_0000;

    /// Places a player back in front of the door (0xcbb3c); a player not in state 2 is only hidden
    /// and put on the ground.
    fn place<R: Bench>(x: &mut Cx<'_, R>, p: Obj) {
        if p.u8(x.b, P_STATE) != 2 {
            let flags = p.u8(x.b, P_FLAGS) | P_HIDDEN;
            p.set_u8(x.b, P_FLAGS, flags);
            crate::placement::place_on_free_ground(x.b, p);
            return;
        }
        p.set_u32(x.b, P_STATE, PLAYER_STATE_WALK);
        let flags = p.u8(x.b, P_FLAGS) & !P_HIDDEN;
        p.set_u8(x.b, P_FLAGS, flags);
        p.set_u8(x.b, P_ACTIVE, 1);
        let variant = x.u8(0x3) as u32;
        let row = (x.rom8(PLACEMENT_ROW + variant) as u16) << 4;
        let index = row.wrapping_add(4 * p.u8(x.b, 2) as u16) as u32;
        let (dx, dy) = (x.rom16(PLACEMENT + index), x.rom16(PLACEMENT + 2 + index));
        let (gx, gy) = (x.u16(0x8), x.u16(0x10));
        p.set_u16(x.b, P_X, gx.wrapping_add(dx));
        p.set_u16(x.b, P_Y, gy.wrapping_add(dy));
        p.set_u16(x.b, P_Z, 0);
        // Original glitch: the Y offset stays in D2.w (`dispatch::Leftovers`)
        crate::mem::set_low_word(x.c.d2(), dy);
    }

    fn alive(b: &mut Cps2Bus, p: Obj) -> bool {
        p.u8(b, P_ACTIVE) != 0 && p.u16(b, P_LIFE) != 0
    }

    /// Conditions of 0xcb2a0-0xcb2d8: does the door act on this frame?
    fn gate_active(b: &mut Cps2Bus, me: Obj) -> bool {
        if me.u8(b, 0x2f) != 0 || me.u8(b, 0x40) != 0 {
            return false;
        }
        if rd8(b, A5 + PAUSED) != 0 || rd8(b, A5 + EXIT_LOCK) != 0 || rd8(b, A5 + TRANSITION) != 0 {
            return false;
        }
        let owner = rd16(b, A5 + OWNER_GATE);
        owner == 0 || owner == me.0 as u16
    }

    /// Is player `p` standing in the door's footprint, free to move, and is the door's
    /// zone present in their mask `+0x6c`? (The original tests the zone index bit
    /// in D0, whose high bits are leftovers: the zone indices are below 8.)
    fn in_reach(b: &mut Cps2Bus, me: Obj, p: Obj) -> bool {
        if rd8(b, NEAR_BLOCK) != 0 {
            return false;
        }
        let blocked = p.u8(b, P_BLOCK_A).wrapping_add(p.u8(b, P_BLOCK_B)).wrapping_add(p.u8(b, P_BLOCK_C));
        let zones = p.u8(b, P_ZONES) as u32;
        if blocked != 0 || zones >> (me.u8(b, ZONE) as u32 & 31) & 1 == 0 {
            return false;
        }
        let state = p.u32(b, P_STATE);
        if state >> 24 != 2 || !WALKING_STATES.contains(&(state & !0xff)) || p.u16(b, P_Z) != 0 {
            return false;
        }
        let dx = p.u16(b, P_X).wrapping_sub(me.u16(b, 0x8)).wrapping_add(REACH_X);
        if dx > 2 * REACH_X {
            return false;
        }
        if me.u8(b, IGNORE_DEPTH) != 0 {
            return true;
        }
        let dy = p.u16(b, P_Y).wrapping_sub(me.u16(b, 0x10)).wrapping_add(REACH_Y);
        dy <= 2 * REACH_Y
    }

    /// Are all living players ready (0xcb80c-0xcb840)? `arrived`: players already counted.
    fn everyone_ready(b: &mut Cps2Bus, in_game: u8, arrived: u8) -> bool {
        if in_game == 0 {
            return false;
        }
        let mut missing = in_game;
        for p in players() {
            if p.u8(b, P_ACTIVE) != 0 && p.u16(b, P_LIFE) == 0 && p.u16(b, P_STATE) != PLAYER_STATE_GONE {
                missing = missing.wrapping_add(1);
            }
        }
        (missing as i8) <= (arrived as i8)
    }

    // --- l'automate ---------------------------------------------------------------------------------

    /// Every 32 frames: number of living players in `A5+0x35`.
    fn refresh_player_count<R: Bench>(x: &mut Cx<'_, R>) {
        let t = x.u16(PLAYER_COUNT_TIMER).wrapping_sub(1);
        x.set16(PLAYER_COUNT_TIMER, t);
        if (t as i16) >= 0 {
            return;
        }
        x.set16(PLAYER_COUNT_TIMER, 0x20);
        let n = players().filter(|&p| alive(x.b, p)).count();
        x.set_a5_u8(IN_GAME, n as u8);
    }

    fn init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        for off in [NEAR_COUNTERS, NEAR_COUNTERS + 1, NEAR_COUNTERS + 2, NEAR_COUNTERS + 3, ARRIVED, TRIGGERED] {
            x.set8(off, 0);
        }
        let row = GATES + (((x.u8(0x3) as u16) << GATE_SHIFT) as i16 as i32 as u32);
        let (gx, gz, gy) = (x.rom16(row + GATE_X), x.rom16(row + GATE_Z), x.rom16(row + GATE_Y));
        x.set16(0x8, gx);
        x.set16(0xc, gz);
        x.set16(0x10, gy);
        let fields = [(EXIT_KIND, GATE_EXIT_KIND), (ZONE, GATE_ZONE), (NEXT_SCENE, GATE_SCENE), (IGNORE_DEPTH, GATE_IGNORE_DEPTH), (BONUS, GATE_BONUS)];
        for (field, entry) in fields {
            let v = x.rom8(row + entry);
            x.set8(field, v);
        }
    }

    /// State 2: a player stays in the footprint long enough (trigger), or reaches the "ready"
    /// state (counted once); when all are ready, the zone changes.
    fn watch<R: Bench>(x: &mut Cx<'_, R>) {
        for (i, p) in players().enumerate() {
            let bit = 1u8 << i;
            let counter = NEAR_COUNTERS + i as u32;
            if x.u8(TRIGGERED) & bit == 0 {
                if p.u8(x.b, P_ACTIVE) != 1 {
                    continue;
                }
                if (x.u8(counter) as i8) >= 0 {
                    let inside = in_reach(x.b, x.o, p);
                    let n = if inside { x.u8(counter).wrapping_add(1) } else { 0 };
                    x.set8(counter, n);
                    if (n as i8) > NEAR_FRAMES {
                        return trigger(x, p, bit);
                    }
                    continue;
                }
            }
            if x.u8(COUNTED) & bit == 0 && p.u32(x.b, P_STATE) == PLAYER_STATE_IDLE {
                x.add8(ARRIVED, 1);
                x.set_bit(COUNTED, i as u8, true);
            }
        }
        let (in_game, arrived) = (x.a5_u8(IN_GAME), x.u8(ARRIVED));
        if everyone_ready(x.b, in_game, arrived) {
            change_zone(x);
        }
    }

    /// Player `p` triggers the exit: they go to the state imposed by the zone; the door
    /// becomes the owner of the transition (`A5+0x52`).
    fn trigger<R: Bench>(x: &mut Cx<'_, R>, p: Obj, bit: u8) {
        if p.u8(x.b, P_ACTIVE) == 0 || p.u16(x.b, P_LIFE) == 0 {
            return;
        }
        let triggered = x.u8(TRIGGERED) | bit;
        x.set8(TRIGGERED, triggered);
        if x.a5_u16(OWNER_GATE) == 0 {
            crate::families::effects_creators::beacon(x.b, x.o);
        }
        let me = x.o.0 as u16;
        x.set_a5_u16(OWNER_GATE, me);
        let zone = x.u8(ZONE) as u32;
        let state = (x.rom16(PLAYER_STATES + 4 * zone) as u32) << 16 | x.rom16(PLAYER_STATES + 4 * zone + 2) as u32;
        p.set_u32(x.b, P_STATE, state);
        p.set_u8(x.b, P_ACTIVE, STATE_DONE);
        p.set_u8(x.b, P_RESET, 1);
        match x.u8(0x3) {
            0xa | 0xe => p.set_u32(x.b, P_STATE, 0x2002008),
            0x8 => p.set_u32(x.b, P_STATE, 0x200200a),
            0x28 => {
                p.set_u16(x.b, P_WEAPON_OBJ, x.o.0 as u16);
                p.set_u32(x.b, P_STATE, 0x200200c);
            }
            _ => {}
        }
    }
}

/// Type 0 (0xba310): palette animation by script (h), the most frequent scenery (over 13,000
/// calls per scenario). Variant `+3` picks an 8-byte entry of table `TABLE`: script address
/// (`+0x30`) and source palette address (`+0x34`). Parameter `+0x2f` picks the
/// page of palette RAM to write to (0: 0x91c800, 1: 0x91cc00, 2: 0x91c000, other: 0x91c400).
///
/// The script starts with a header (starting palette row number, duration of the first block)
/// then is made of blocks: duration (word: 0 → state 4, negative → end), number of rows, and for
/// each row the index (in 32-byte rows) of the source palette. Each copied row keeps
/// the brightness nibble (`& 0xf000f000`) of the destination row. A duration of 0xffff restarts
/// the script, any other negative jumps to the long address that follows.
///
/// | `+4` | role |
/// |---|---|
/// | 0 | initialization and first block |
/// | 2 | one block at each deadline of `+0x3c`; if `A5+0x42` (scene in transition), a per-variant table decides whether to continue (0), wait (1) or finish (2, 3) |
/// | 4 | end: a per-variant table picks release (0) or reconversion into variant 0x48, 0x4c or 0x4d |
/// | 6 | release |
///
/// Variant 0x4e ignores the pause and the suspension.
mod palette_script {
    use super::*;
    use crate::mem::{rd16, rd32, wr32};

    /// Script table (data space): script then source palettes, 8 bytes per variant.
    const TABLE: u32 = 0xba57a;
    /// During a scene transition: action per variant (byte, program space).
    const TRANSITION_ACTION: u32 = 0xba44c;
    /// End of script: continuation per variant (byte, program space).
    const END_ACTION: u32 = 0xba4ca;
    const PALETTE_PAGES: [u32; 4] = [0x91c800, 0x91cc00, 0x91c000, 0x91c400];
    /// Fields: script, source palettes, destination, countdown.
    const SCRIPT: u32 = 0x30;
    const SOURCE: u32 = 0x34;
    const DEST: u32 = 0x38;
    const TIMER: u32 = 0x3c;
    const PAGE: u32 = 0x2f;
    const KEEP_BRIGHTNESS: u32 = 0xf000f000;
    const LINE_BYTES: u32 = 32;
    const STATE_FINISH: u16 = 0x400;
    const GAME_PAUSED: u32 = 0x3d;
    const TRANSITION: u32 = 0x42;
    /// Variant that ignores pause and suspension.
    const ALWAYS_RUNS: u8 = 0x4e;
    /// Variantes de remplacement en fin de script.
    const NEXT_VARIANTS: [(u8, u8); 3] = [(0x48, 0), (0x4c, 0), (0x4d, 1)];

    pub fn run(x: &mut Gx) {
        let variant = x.u8(0x3);
        if variant != ALWAYS_RUNS && (x.a5_u8(GAME_PAUSED) != 0 || x.u8(0x40) != 0) {
            return;
        }
        match x.u8(0x4) {
            0 => start(x),
            2 => play(x),
            4 => finish(x),
            _ => x.free(&pools::SET),
        }
    }

    fn start(x: &mut Gx) {
        x.add8(0x4, 2);
        let entry = TABLE.wrapping_add((((x.u8(0x3) as u16) << 3) as i16) as u32);
        let (script, source) = (x.data32(entry), x.data32(entry + 4));
        x.set32(SOURCE, source);
        x.set32(SCRIPT, script);
        let page = PALETTE_PAGES[x.u8(PAGE).min(3) as usize];
        let first_line = x.data16(script) as i16;
        x.set32(DEST, page.wrapping_add(((first_line as u16) << 5) as i16 as i32 as u32));
        let timer = x.data16(script + 2);
        x.set16(TIMER, timer);
        let dest = x.u32(DEST);
        copy_lines(x, script + 4, source, dest);
    }

    fn play(x: &mut Gx) {
        if x.a5_u8(TRANSITION) != 0 {
            let variant = x.u8(0x3) as u32;
            let action = x.rom8(TRANSITION_ACTION + variant);
            match action {
                0 => {}
                1 => return,
                2 => {
                    x.set16(0x4, STATE_FINISH);
                    return finish(x);
                }
                _ => return finish(x),
            }
        }
        let t = x.u16(TIMER).wrapping_sub(1);
        x.set16(TIMER, t);
        if (t as i16) > 0 {
            return;
        }
        let mut script = x.u32(SCRIPT);
        let source = x.u32(SOURCE);
        let duration = x.data16(script);
        script += 2;
        x.set16(TIMER, duration);
        if (duration as i16) < 0 {
            if duration == 0xffff {
                x.set16(0x4, 0);
                start(x);
            } else {
                let target = x.data32(script);
                x.set32(SCRIPT, target);
            }
        } else if duration == 0 {
            x.set16(0x4, STATE_FINISH);
            finish(x);
        } else {
            let dest = x.u32(DEST);
            copy_lines(x, script, source, dest);
        }
    }

    /// Copies the rows of a block (number, then one index per row) to the destination palette
    /// and saves the continuation of the script.
    /// Original glitch: the last long copied (with the brightness) stays in D4, which the unsteered raft
    /// (0xad7b0) adds to the players' position: it is a leftover from the dispatch loop (`Leftovers::d4`).
    fn copy_lines(x: &mut Gx, mut script: u32, source: u32, mut dest: u32) {
        let count = rd16(x.b, script).wrapping_sub(1);
        script += 2;
        let mut last = 0;
        for _ in 0..=count as u32 {
            let line = (rd16(x.b, script) << 5) as u32;
            script += 2;
            let keep = rd32(x.b, dest) & KEEP_BRIGHTNESS;
            for i in (0..LINE_BYTES).step_by(4) {
                last = rd32(x.b, source + line + i) | keep;
                wr32(x.b, dest + i, last);
            }
            dest += LINE_BYTES;
        }
        x.set32(SCRIPT, script);
        x.leftovers().d4 = last;
    }

    fn finish(x: &mut Gx) {
        let variant = x.u8(0x3) as u32;
        match x.rom8(END_ACTION + variant) {
            0 => x.set8(0x4, 6),
            n @ 1..=3 => {
                let (next, page) = NEXT_VARIANTS[n as usize - 1];
                x.set8(0x3, next);
                x.set8(PAGE, page);
                x.set32(0x4, 0);
            }
            _ => {}
        }
    }
}

/// Type 10 (0xcc3c2): timed dialogue bubble (h). On creation (state 0) it picks a text
/// from the scene's tables (one table per game mode `A5+0x72`, one entry per variant `+3`),
/// creates a SOBJ object of type 0xc (the frame, via 0xdfc78) whose address it keeps in `+0x22`,
/// publishes the text at `A5+0x5ce..0x5d4` and loads palette 0x91c180. It lives 120 frames
/// (`+0x20`), or until variable `A5-0x5fbe` becomes non-zero, then clears the text,
/// requests the release of its frame (state 6) and releases itself.
mod dialogue {
    use super::*;
    use crate::mem::{wr32, wr8, A5};

    /// Text tables per game mode (longs, program space), then per variant (longs ×4).
    const TEXT_TABLES: u32 = 0xcc438;
    const GAME_MODE: u32 = 0x72;
    /// Creation of a SOBJ object of type 0xc: D0 = variant, carry = failure; A1 = the object.
    const PALETTE_SRC: u32 = 0x292260;
    const PALETTE_DST: u32 = 0x91c180;
    /// Stop variable (A5 - 0x5fbe) and "bubble displayed" variable (A5 - 0x5b8c).
    const STOP: u32 = A5 - 0x5fbe;
    const SHOWN: u32 = A5 - 0x5b8c;
    /// Text variables published for display.
    const TEXT_ADDR: u32 = 0x5ce;
    const TEXT_SIZE: u32 = 0x5d2;
    const TEXT_WIDTH: u32 = 0x5d4;
    const TEXT_WIDTH_VALUE: u16 = 0xc4;
    const LIFETIME: u8 = 0x78;
    const TIMER: u32 = 0x20;
    const FRAME: u32 = 0x22;
    const FRAME_WIDTH: u32 = 0x14;
    const FRAME_FREE_STATE: u32 = 0x6000000;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        if crate::mem::rd8(x.b, STOP) == 0 {
            if x.u8(0x4) == 0 {
                return create(x);
            }
            let t = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, t);
            if t != 0 {
                return;
            }
        }
        wr8(x.b, SHOWN, 0);
        x.set_a5_u32(TEXT_ADDR, 0);
        let frame = x.u16(FRAME);
        if frame != 0 {
            wr32(x.b, (frame as i16 as i32 as u32 & 0xff_ffff) + 4, FRAME_FREE_STATE);
        }
        x.free(&pools::SET);
    }

    fn create<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        x.set8(TIMER, LIFETIME);
        let variant = (x.u8(0x3) << 2) as u32;
        let mode = x.a5_u16(GAME_MODE).wrapping_sub(2).wrapping_mul(2) as i16 as i32 as u32;
        let table = (x.rom16(TEXT_TABLES.wrapping_add(mode)) as u32) << 16 | x.rom16(TEXT_TABLES.wrapping_add(mode) + 2) as u32;
        let text = x.data32(table + variant);
        let width = x.data16(text);
        x.set16(FRAME, 0);
        let variant = x.u8(0x2f);
        if let Ok(frame) = crate::families::creators::sobj_12_pure(x.b, variant) {
            frame.set_u16(x.b, FRAME_WIDTH, width);
            x.set16(FRAME, frame.0 as u16);
        }
        let size = x.data16(text + 2);
        x.set_a5_u16(TEXT_SIZE, size);
        x.set_a5_u16(TEXT_WIDTH, TEXT_WIDTH_VALUE);
        x.set_a5_u32(TEXT_ADDR, text + 4);
        for i in (0..32).step_by(4) {
            let v = x.data32(PALETTE_SRC + i);
            wr32(x.b, PALETTE_DST + i, v);
        }
    }
}

/// Type 5 (0xc9780): name entry on the score board (h). Two kinds of objects according to `+3`:
///
/// * variant 0: the **board** (a single object). It keeps in `+0x18..+0x1f` the addresses of the four
///   cursors, detects the cells where two cursors overlap (two 8-byte records
///   at `+0x30` and `+0x40` for the previous frame) and draws those cells;
/// * non-zero variant: a player's **cursor** (`+0x2f` = player number, `+0x22` = its
///   record). It moves the cursor (`+0x8` column, `+0xc` row) over a grid of
///   characters (11 columns × 6 rows, or a wide grid if `A5+0x70`), writes the letters into the
///   player's buffer (`+0x118..+0x11f`, 0xff = empty, `+0x18` = length), handles erasing, the
///   voiced vowels (0x9d, 0x9f, conversion table), two secret codes (key sequences
///   recognized in the buffer) and computes a BCD check digit.
///
/// States (`+4`): 0 initialization, 2 entry, 4 finished (the cursor waits), 6 release. The board
/// releases the cursors (state 6 in the object) then itself in states 4 and 6.
mod name_entry {
    use super::*;
    use crate::mem::{rd16, rd32, rd8, wr16, wr32, wr8};

    // --- video RAM and tables ---
    const TILEMAP: u32 = 0x912000;
    const PALETTE_BANK: u32 = 0x91c400;
    const PALETTE_SOURCE: u32 = 0x25cd40;
    const PALETTE_ENTRY: u32 = 0x20;
    const KEEP_BRIGHTNESS: u32 = 0xf000f000;
    /// Starting cell of the cursors (program space): two words per player and per mode.
    const START_CELLS: u32 = 0xc9abe;
    /// Width of each row of the grid (words, data space).
    const ROW_WIDTH_NARROW: u32 = 0xcad74;
    const ROW_WIDTH_WIDE: u32 = 0xcad80;
    /// Default names: 8 bytes per player variant (+0x60 in the wide grid), data space.
    const DEFAULT_NAMES: u32 = 0xc9daa;
    /// Character of each cell (bytes, program space), index = column + 16 × row.
    const GRID_NARROW: u32 = 0xcaacc;
    const GRID_WIDE: u32 = 0xcab36;
    /// Voiced vowel conversions (bytes, data space) for 0x9f and 0x9d.
    const VOICED: u32 = 0xcabdc;
    const SEMI_VOICED: u32 = 0xcac64;
    /// Secret codes: character sequences (length byte, characters), end = negative byte.
    const RESET_CODES: u32 = 0xcade6;
    const SPECIAL_CODES: u32 = 0xcad8a;
    const SPECIAL_REPLACEMENTS: u32 = 0xca5a6;
    /// BCD value of each character (bytes, data space) and default check digit.
    const CHAR_DIGITS: u32 = 0xcacec;
    const DEFAULT_CHECK: u32 = 0xca6aa;
    /// Tints of the special cell (bytes, program space).
    const SPECIAL_TILES: u32 = 0xca83e;
    /// Tile placement tables (words, program space), narrow grid then wide.
    const NARROW_ROW_OFFSETS: u32 = 0xca894;
    const NARROW_CELLS: u32 = 0xca8c8;
    const WIDE_ROW_OFFSETS: u32 = 0xca9c8;
    const WIDE_CELLS: u32 = 0xca9f2;
    /// Positions of the end indicator (words, program space) and end records (longs).
    const FINISH_POSITIONS: u32 = 0xca0e4;
    const FINISH_RECORDS: u32 = 0xca1a4;
    const FLAGS_ARRAY: u32 = 0xff2478;

    // --- variables de A5 ---
    const INPUT_HELD: u32 = 0x0;
    const INPUT_PRESSED: u32 = 0xc;
    const BUTTONS_MASK: u32 = 0x4;
    const BUTTONS_OLD: u32 = 0xa;
    const GAME_MODE: u32 = 0x34;
    const WIDE_FLAG: u32 = 0x70;
    const BLINK_FLAGS: u32 = 0x344;

    // --- common fields of the board and the cursors ---
    const WIDE: u32 = 0x13;
    const COUNT_TYPED: u32 = 0x14;
    const PAIRS: u32 = 0x15;
    const COUNT_DONE: u32 = 0x16;
    const COUNT_ACTIVE: u32 = 0x17;
    const OWNER: u32 = 0x22;
    const ACTIVE: u32 = 0x24;
    const PLAYER: u32 = 0x2f;
    // --- cursor fields ---
    const LENGTH: u32 = 0x18;
    const VOICED_PENDING: u32 = 0x19;
    const DONE: u32 = 0x1a;
    const NO_NAME: u32 = 0x1b;
    const PREV_X: u32 = 0x1c;
    const PREV_Y: u32 = 0x1e;
    const REPEAT: u32 = 0x20;
    const TYPED: u32 = 0x21;
    const HELD_FRAMES: u32 = 0x2e;
    const COL: u32 = 0x8;
    const ROW: u32 = 0xc;
    /// Player's character buffer: relative address, size.
    const BUFFER: u32 = 0x118;
    const EMPTY: u8 = 0xff;
    const ENTER: u8 = 0x81;
    const NAME_END: u8 = 0xf0;
    // --- fields of a board overlap record ---
    const REC_DIR: u32 = 0;
    const REC_TIMER: u32 = 1;
    const REC_COL: u32 = 2;
    const REC_ROW: u32 = 4;
    const REC_MASK: u32 = 6;
    const RECORDS: u32 = 0x30;
    const PREV_RECORDS: u32 = 0x40;
    const SLAVES: u32 = 0x18;
    const FREE_ON_NEXT: u32 = 0x6000000;
    const GRID_COLS: u16 = 11;
    const GRID_ROWS: u16 = 6;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(0x3) != 0 {
            return cursor::run(x);
        }
        match x.u8(0x4) {
            0 => {
                board::init(x);
                board::update(x);
            }
            2 => board::update(x),
            _ => board::release(x),
        }
    }

    // --- aides communes -------------------------------------------------------------------------

    /// High byte of another object's record designated by a word.
    fn at<R: Bench>(x: &mut Cx<'_, R>, off: u32) -> Obj {
        x.linked(off)
    }

    fn wide<R: Bench>(x: &mut Cx<'_, R>) -> bool {
        x.u8(WIDE) != 0
    }

    /// 0xca85c: held keys and pressed keys of the frame for player `player`.
    fn inputs<R: Bench>(x: &mut Cx<'_, R>, player: u32) -> (u8, u8) {
        (x.a5_u8(INPUT_HELD + player), x.a5_u8(INPUT_PRESSED + player))
    }

    /// 0xca8a0: address in the tile map and size (columns, rows - 1) of the cell (`col`,
    /// `row`). Rend (adresse, D0 = taille).
    fn cell_region<R: Bench>(x: &mut Cx<'_, R>, col: u16, row: u16) -> (u32, u16) {
        let (row_offsets, cells) = if wide(x) { (WIDE_ROW_OFFSETS, WIDE_CELLS) } else { (NARROW_ROW_OFFSETS, NARROW_CELLS) };
        let row_off = x.rom16(row_offsets.wrapping_add((row.wrapping_mul(2)) as i16 as i32 as u32));
        let index = col.wrapping_mul(4).wrapping_add(row_off);
        let at = |o: u32| cells.wrapping_add(o).wrapping_add(index as i16 as i32 as u32);
        let (offset, size) = (x.rom16(at(0)), x.rom16(at(2)));
        (TILEMAP + 2 + offset as i16 as i32 as u32, size)
    }

    /// 0xca86e: fills region `size` (high byte: rows - 1, low byte: columns - 1) with
    /// tile `tile`, one word every 4 bytes.
    fn fill_tiles<R: Bench>(x: &mut Cx<'_, R>, mut a1: u32, size: u16, tile: u16) {
        let cols = size & 0xff;
        for _ in 0..=(size >> 8) as u32 {
            let mut advance = 0;
            for _ in 0..=cols as u32 {
                wr16(x.b, a1, tile);
                a1 += 4;
                advance += 4;
            }
            a1 = a1 + 0x80 - advance;
        }
    }

    /// 0xca81a: redraws cell (`col`, `row`): tile 0x1c, or a tint from the table when it
    /// is in the special column.
    fn redraw_cell<R: Bench>(x: &mut Cx<'_, R>, col: u16, row: u16) {
        let (a1, size) = cell_region(x, col, row);
        let mut tile: u16 = 0x1c;
        if !wide(x) {
            if col == 0xa {
                tile = x.rom8(SPECIAL_TILES.wrapping_add(row.wrapping_sub(1) as i16 as i32 as u32)) as u16;
            }
        } else if row == 4 && (col as i16) >= 3 {
            tile = x.rom8(SPECIAL_TILES.wrapping_add(row.wrapping_sub(3) as i16 as i32 as u32)) as u16;
        }
        fill_tiles(x, a1, size, tile);
    }

    // --- the board (variant 0) -------------------------------------------------------------

    mod board {
        use super::*;

        /// 0xc979e: state 0. Resets the records to zero and loads four palettes (with the
        /// brightness of the page's first color).
        pub fn init<R: Bench>(x: &mut Cx<'_, R>) {
            x.add8(0x4, 2);
            for off in [0x30, 0x34, 0x38, 0x3c] {
                x.set32(off, 0);
            }
            x.set8(COUNT_DONE, 0);
            x.set8(PAIRS, 0);
            let wide = x.a5_u16(WIDE_FLAG) != 0;
            x.set8(WIDE, wide as u8);
            let keep = rd32(x.b, 0x91c000) & KEEP_BRIGHTNESS;
            for i in 0..4u32 {
                for j in (0..PALETTE_ENTRY).step_by(4) {
                    let v = x.data32(PALETTE_SOURCE + i * 0x100 + j);
                    wr32(x.b, PALETTE_BANK + i * PALETTE_ENTRY + j, v | keep);
                }
            }
        }

        /// States 4 and 6: releases the cursors then the board.
        pub fn release<R: Bench>(x: &mut Cx<'_, R>) {
            for i in 0..4 {
                let slave = at(x, SLAVES + 2 * i);
                slave.set_u32(x.b, 4, FREE_ON_NEXT);
            }
            x.free(&pools::SET);
        }

        /// 0xc9810: one frame. Counts the cursors, spots the shared cells, animates and draws.
        pub fn update<R: Bench>(x: &mut Cx<'_, R>) {
            count_cursors(x);
            count_finished(x);
            // records of the previous frame (shared cells to ignore according to the count)
            for (dst, src) in [(0x40, 0x30), (0x48, 0x38), (0x44, 0x34), (0x4c, 0x3c)] {
                let v = x.u32(src);
                x.set32(dst, v);
            }
            if x.u8(PAIRS) != 2 {
                if x.u8(PAIRS) == 0 {
                    x.set32(0x40, u32::MAX);
                    x.set32(0x44, u32::MAX);
                }
                x.set32(0x48, u32::MAX);
                x.set32(0x4c, u32::MAX);
            }
            for off in [0x30, 0x34, 0x38, 0x3c] {
                x.set32(off, 0);
            }
            let pairs = find_shared_cells(x);
            x.set8(PAIRS, pairs);
            animate_records(x);
            draw_cursors(x);
            for i in 0..pairs as u32 {
                let rec = x.o.0 + RECORDS + 8 * i;
                let (col, row) = (rd16(x.b, rec + REC_COL), rd16(x.b, rec + REC_ROW));
                let (a1, size) = cell_region(x, col, row);
                let mut tile: u16 = 0x18;
                if shared_in_special(x, col, row) {
                    tile = 0;
                }
                tile = (tile & 0xff00) | (tile as u8).wrapping_add(rd8(x.b, rec + REC_DIR)) as u16;
                fill_tiles(x, a1, size, tile);
            }
        }

        /// Cells where at least two cursors overlap: records (column, row, player
        /// mask) for the first two, row-by-row scan. Returns their number.
        fn find_shared_cells<R: Bench>(x: &mut Cx<'_, R>) -> u8 {
            let mut found = 0u8;
            let (mut col, mut row) = (0u16, 0u16);
            loop {
                let rec = x.o.0 + RECORDS + 8 * found as u32;
                let mut hits = 0u16;
                for i in 0..4 {
                    let slave = at(x, SLAVES + 2 * i);
                    if slave.u8(x.b, ACTIVE) != 0 && slave.u16(x.b, COL) == col && slave.u16(x.b, ROW) == row {
                        wr16(x.b, rec + REC_COL, col);
                        wr16(x.b, rec + REC_ROW, row);
                        let player = slave.u8(x.b, PLAYER) as u32;
                        let mask = rd8(x.b, rec + REC_MASK) | 1u8.wrapping_shl(player & 7);
                        wr8(x.b, rec + REC_MASK, mask);
                        hits += 1;
                    }
                }
                let last = row == GRID_ROWS - 1 && col == GRID_COLS - 1;
                if !last {
                    let mask = rd8(x.b, rec + REC_MASK);
                    wr8(x.b, rec + REC_MASK, 0);
                    if hits >= 2 {
                        found += 1;
                        wr8(x.b, rec + REC_MASK, mask);
                    }
                    col += 1;
                    if col >= GRID_COLS {
                        row += 1;
                        col = 0;
                    }
                    if found == 2 {
                        break;
                    }
                    continue;
                }
                break;
            }
            found
        }

        /// 0xc9912 and following: for each shared cell, maintains the "featured" player
        /// (`REC_DIR`) and their display time (`REC_TIMER`) taking the previous frame into account.
        fn animate_records<R: Bench>(x: &mut Cx<'_, R>) {
            for k in 0..2u32 {
                let rec = x.o.0 + RECORDS + 8 * k;
                if rd8(x.b, rec + REC_MASK) == 0 {
                    return;
                }
                let (col, row) = (rd16(x.b, rec + REC_COL), rd16(x.b, rec + REC_ROW));
                let mask = rd8(x.b, rec + REC_MASK);
                let previous = (0..2u32).map(|p| x.o.0 + PREV_RECORDS + 8 * p).find(|&p| rd16(x.b, p + REC_COL) == col && rd16(x.b, p + REC_ROW) == row);
                if let Some(old) = previous {
                    let dir = rd8(x.b, old + REC_DIR);
                    wr8(x.b, rec + REC_DIR, dir);
                    let timer = rd8(x.b, old + REC_TIMER);
                    wr8(x.b, rec + REC_TIMER, timer);
                    let old_mask = rd8(x.b, old + REC_MASK);
                    if old_mask != mask {
                        let new_bits = !old_mask & mask;
                        if new_bits != 0 {
                            // a new player arrives: they become featured
                            let first = new_bits.trailing_zeros() as u8;
                            wr8(x.b, rec + REC_DIR, first);
                            wr8(x.b, rec + REC_TIMER, 0xd);
                        } else if mask >> (dir & 7) & 1 != 0 {
                            // the featured player is still there
                        } else {
                            next_player(x, rec, mask);
                            continue;
                        }
                    }
                    let timer = rd8(x.b, rec + REC_TIMER).wrapping_sub(1);
                    wr8(x.b, rec + REC_TIMER, timer);
                    if timer == 0 {
                        next_player(x, rec, mask);
                    }
                } else {
                    // new cell: players who already have a name entered are preferred
                    let mut with_name = 0u8;
                    for bit in (0..4u32).rev() {
                        let slave = at(x, SLAVES + 2 * bit);
                        if mask >> bit & 1 != 0 && slave.u8(x.b, COUNT_ACTIVE) != 0 {
                            with_name |= 1 << bit;
                        }
                    }
                    let choose = if with_name != 0 { with_name } else { mask };
                    wr8(x.b, rec + REC_TIMER, 0xc);
                    let mut d = 0u8;
                    while choose >> d & 1 == 0 {
                        d = (d + 1) & 3;
                    }
                    wr8(x.b, rec + REC_DIR, d);
                }
            }
        }

        /// 0xc998a: next player in the mask, in turn (display time 12 frames).
        fn next_player<R: Bench>(x: &mut Cx<'_, R>, rec: u32, mask: u8) {
            let mut d = rd8(x.b, rec + REC_DIR);
            loop {
                d = d.wrapping_add(1) & 3;
                if mask >> d & 1 != 0 {
                    break;
                }
            }
            wr8(x.b, rec + REC_DIR, d);
            wr8(x.b, rec + REC_TIMER, 0xc);
        }

        /// 0xca7ea: is the shared cell in the special column? (base tile 0 otherwise 0x18).
        fn shared_in_special<R: Bench>(x: &mut Cx<'_, R>, col: u16, row: u16) -> bool {
            if !wide(x) {
                col == 0xa
            } else {
                row == 4 && (col as i16) >= 3
            }
        }

        /// 0xca6b0: number of active cursors (`+0x17`) and of those that typed a letter (`+0x14`).
        fn count_cursors<R: Bench>(x: &mut Cx<'_, R>) {
            let (mut active, mut typed) = (0u8, 0u8);
            for i in 0..4 {
                let slave = at(x, SLAVES + 2 * i);
                let player = Obj(slave.u16(x.b, OWNER) as i16 as i32 as u32 & 0xff_ffff);
                if player.u8(x.b, 0) != 0 && (player.u8(x.b, 4) as i8) < 4 {
                    active += 1;
                    if slave.u8(x.b, TYPED) != 0 {
                        typed += 1;
                    }
                }
            }
            x.set8(COUNT_ACTIVE, active);
            x.set8(COUNT_TYPED, typed);
        }

        /// 0xca6ea: number of cursors that have finished their entry (`+0x16`).
        fn count_finished<R: Bench>(x: &mut Cx<'_, R>) {
            let n = (0..4).filter(|&i| at(x, SLAVES + 2 * i).u8(x.b, DONE) != 0).count();
            x.set8(COUNT_DONE, n as u8);
        }

        /// 0xca78c: draws the cursors (their cell, special tint in the commands column).
        fn draw_cursors<R: Bench>(x: &mut Cx<'_, R>) {
            for i in 0..4 {
                let slave = at(x, SLAVES + 2 * i);
                if slave.u8(x.b, ACTIVE) == 0 {
                    continue;
                }
                let (col, row) = (slave.u16(x.b, COL), slave.u16(x.b, ROW));
                let (a1, size) = cell_region(x, col, row);
                let base: u8 = if !wide(x) {
                    if col == 0xa { 0 } else { 0x18 }
                } else if row == 4 && (col as i16) >= 3 {
                    0
                } else {
                    0x18
                };
                let tile = base.wrapping_add(slave.u8(x.b, PLAYER)) as u16;
                fill_tiles(x, a1, size, tile);
            }
        }
    }

    // --- a player's cursor (non-zero variants) --------------------------------------------

    mod cursor {
        use super::*;

        pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
            match x.u8(0x4) {
                0 => init(x),
                2 => step(x),
                4 => {}
                _ => finish_and_free(x),
            }
        }

        fn owner<R: Bench>(x: &mut Cx<'_, R>) -> Obj {
            at(x, OWNER)
        }

        /// 0xc9a74: state 0. A player who already has a name (non-empty buffer) is marked finished; otherwise
        /// the cursor goes to its player's starting cell.
        fn init<R: Bench>(x: &mut Cx<'_, R>) {
            let player = owner(x);
            x.set8(ACTIVE, 0);
            x.set8(DONE, 0);
            x.set8(HELD_FRAMES, 0);
            if player.u8(x.b, 0) == 0 || (player.u8(x.b, 4) as i8) >= 4 {
                return;
            }
            if x.a5_u8(GAME_MODE) != 1 && !(player.u32(x.b, BUFFER) == u32::MAX && player.u32(x.b, BUFFER + 4) == u32::MAX) {
                x.set8(DONE, 1);
                x.set8(0x4, 4);
                return;
            }
            let wide = x.a5_u16(WIDE_FLAG) != 0;
            x.set8(WIDE, wide as u8);
            let index = ((x.u8(PLAYER) << 2) as u16).wrapping_add((wide as u16) << 4) as u32;
            let (col, row) = (x.rom16(START_CELLS + index), x.rom16(START_CELLS + 2 + index));
            x.set16(COL, col);
            x.set16(ROW, row);
            x.add8(0x4, 2);
            x.set8(ACTIVE, 1);
            x.set8(LENGTH, 0);
            x.set8(VOICED_PENDING, 0);
            x.set8(NO_NAME, 0);
            x.set8(REPEAT, 0x1e);
            step(x);
        }

        /// 0xc9b9e: state 2, one frame of entry.
        fn step<R: Bench>(x: &mut Cx<'_, R>) {
            x.set8(TYPED, 0);
            let (c, r) = (x.u16(COL), x.u16(ROW));
            x.set16(PREV_X, c);
            x.set16(PREV_Y, r);
            handle_input(x);
            x.set8(COUNT_ACTIVE, 1);
            if x.u16(COL) == x.u16(PREV_X) && x.u16(ROW) == x.u16(PREV_Y) {
                x.set8(COUNT_ACTIVE, 0);
            }
            let player = x.u8(PLAYER) as u32;
            let (held, _) = inputs(x, player);
            if held & 0x80 != 0 {
                x.add8(HELD_FRAMES, 1);
            }
        }

        /// A move: the grid repeats the direction every 6 frames, 30 the first time.
        fn handle_input<R: Bench>(x: &mut Cx<'_, R>) {
            let player = x.u8(PLAYER) as u32;
            let (held, pressed) = inputs(x, player);
            let mut repeat = 0x1e;
            for dir in 0..4u32 {
                if held >> dir & 1 != 0 {
                    if pressed >> dir & 1 != 0 {
                        return step_cursor(x, dir, repeat);
                    }
                    repeat = 6;
                    let t = x.u8(REPEAT).wrapping_sub(1);
                    x.set8(REPEAT, t);
                    if t == 0 {
                        return step_cursor(x, dir, repeat);
                    }
                    break;
                }
            }
            if pressed & 0x10 != 0 {
                return confirm_button(x);
            }
            if pressed & 0x20 != 0 {
                return erase_button(x);
            }
            // start button: validated when all the other buttons are released
            let released = !x.a5_u8(BUTTONS_OLD) & x.a5_u8(BUTTONS_MASK);
            if released >> (x.u8(PLAYER) as u32 & 31) & 1 == 0 {
                return;
            }
            if x.u8(LENGTH) == 0 {
                load_default_name(x);
            }
            finish(x);
        }

        /// 0xc9ef8: moves the cursor one cell in direction `dir`.
        fn step_cursor<R: Bench>(x: &mut Cx<'_, R>, dir: u32, repeat: u8) {
            x.set8(REPEAT, repeat);
            let (col, row) = (x.u16(COL), x.u16(ROW));
            let slot = (dir * 2 + if wide(x) { 8 } else { 0 }) as u16;
            move_cursor(x, slot, col, row);
        }

        /// Movement routines (table 0xc9f26): `slot` = direction × 2 (+ 8 in the wide grid).
        fn move_cursor<R: Bench>(x: &mut Cx<'_, R>, slot: u16, col: u16, row: u16) {
            let width = |x: &mut Cx<'_, R>, table: u32, row: u16| x.data16(table.wrapping_add(row.wrapping_mul(2) as i16 as i32 as u32));
            match slot {
                // to the right, narrow grid
                0 => {
                    x.add16(COL, 1);
                    let w = width(x, ROW_WIDTH_NARROW, row);
                    if w < x.u16(COL) {
                        x.set16(COL, 0);
                        x.add16(ROW, 1);
                        if x.u16(ROW) > 5 {
                            x.set16(ROW, 0);
                        }
                    }
                }
                // to the left, narrow grid
                2 => {
                    x.add16(COL, 0xffff);
                    if (x.u16(COL) as i16) < 0 {
                        x.add16(ROW, 0xffff);
                        if (x.u16(ROW) as i16) < 0 {
                            x.set16(ROW, 5);
                        }
                        let r = x.u16(ROW);
                        let w = width(x, ROW_WIDTH_NARROW, r);
                        x.set16(COL, w);
                    }
                }
                // down, narrow grid
                4 => {
                    if col as u8 == 0xa && row as u8 == 4 {
                        x.set16(COL, 9);
                    }
                    x.add16(ROW, 1);
                    if x.u16(ROW) > 5 {
                        x.set16(ROW, 0);
                    }
                }
                // up, narrow grid
                6 => {
                    if col as u8 == 0xa && row as u8 == 1 {
                        x.set16(COL, 9);
                    }
                    x.add16(ROW, 0xffff);
                    if (x.u16(ROW) as i16) < 0 {
                        x.set16(ROW, 5);
                    }
                }
                // to the right, wide grid
                8 => {
                    x.add16(COL, 1);
                    let w = width(x, ROW_WIDTH_WIDE, row);
                    if w < x.u16(COL) {
                        x.set16(COL, 0);
                        x.add16(ROW, 1);
                        if x.u16(ROW) > 4 {
                            x.set16(ROW, 0);
                        }
                    }
                }
                // to the left, wide grid
                0xa => {
                    x.add16(COL, 0xffff);
                    if (x.u16(COL) as i16) < 0 {
                        x.add16(ROW, 0xffff);
                        if (x.u16(ROW) as i16) < 0 {
                            x.set16(ROW, 4);
                        }
                        let r = x.u16(ROW);
                        let w = width(x, ROW_WIDTH_WIDE, r);
                        x.set16(COL, w);
                    }
                }
                // down, wide grid
                0xc => {
                    x.add16(ROW, 1);
                    if (x.u16(ROW) as i16) > 4 {
                        x.set16(ROW, 0);
                    }
                    clamp_wide(x);
                }
                // up, wide grid
                _ => {
                    x.add16(ROW, 0xffff);
                    if (x.u16(ROW) as i16) < 0 {
                        x.set16(ROW, 4);
                    }
                    clamp_wide(x);
                }
            }
            redraw_cell(x, col, row);
        }

        /// Brings the column back within the width of the current row (wide grid).
        fn clamp_wide<R: Bench>(x: &mut Cx<'_, R>) {
            let r = x.u16(ROW);
            let w = x.data16(ROW_WIDTH_WIDE.wrapping_add(r.wrapping_mul(2) as i16 as i32 as u32));
            if w < x.u16(COL) {
                x.set16(COL, w);
            }
        }

        /// 0xc9c1a: button 1 — chooses the current cell: command (special column) or character.
        fn confirm_button<R: Bench>(x: &mut Cx<'_, R>) {
            let special = if !wide(x) {
                x.u16(COL) == 0xa
            } else {
                x.u16(ROW) == 4 && (x.u16(COL) as i16) >= 3
            };
            if special {
                return special_command(x);
            }
            let ch = char_under_cursor(x);
            let buffer = owner(x).0 + BUFFER;
            let len = x.u8(LENGTH);
            enter_char(x, ch, buffer, len);
        }

        /// 0xcaaae: character of the current cell.
        fn char_under_cursor<R: Bench>(x: &mut Cx<'_, R>) -> u8 {
            let index = x.u16(COL).wrapping_add(x.u16(ROW) << 4);
            let table = if wide(x) { GRID_WIDE } else { GRID_NARROW };
            x.rom8(table.wrapping_add(index as i16 as i32 as u32))
        }

        /// 0xc9cd8: command of the special column according to the row: 1 confirm (0x81), 2 default
        /// name, 3 erase, 4 finish.
        fn special_command<R: Bench>(x: &mut Cx<'_, R>) {
            let n = if !wide(x) { x.u16(ROW) } else { x.u16(COL).wrapping_sub(2) };
            match n {
                1 => {
                    let buffer = owner(x).0 + BUFFER;
                    let len = x.u8(LENGTH);
                    enter_char(x, ENTER, buffer, len);
                }
                2 => load_default_name(x),
                3 => erase_button(x),
                4 => finish(x),
                _ => {}
            }
        }

        /// 0xcab86: adds character `ch`; 0x9f and 0x9d transform the previous letter.
        fn enter_char<R: Bench>(x: &mut Cx<'_, R>, mut ch: u8, buffer: u32, len: u8) {
            if x.u8(NO_NAME) != 0 {
                return;
            }
            if ch == 0x9f || ch == 0x9d {
                let table = if ch == 0x9f { VOICED } else { SEMI_VOICED };
                if len == 0 {
                    return;
                }
                let previous = rd8(x.b, buffer + len as u32 - 1);
                if previous == 0x6f {
                    return;
                }
                let converted = x.data8(table + previous as u32);
                if converted == 0 {
                    return;
                }
                ch = converted;
                x.add8(LENGTH, 0xff);
                x.set8(VOICED_PENDING, 0);
            }
            if x.u8(VOICED_PENDING) != 0 {
                return;
            }
            store_char(x, ch, buffer);
        }

        /// 0xc9c90: writes the character into the buffer; buffer full → cursor on "confirm".
        fn store_char<R: Bench>(x: &mut Cx<'_, R>, ch: u8, buffer: u32) {
            let len = x.u8(LENGTH) as u32;
            wr8(x.b, buffer + len, ch);
            x.set8(TYPED, 1);
            x.add8(LENGTH, 1);
            if x.u8(LENGTH) != 6 {
                return;
            }
            let (col, row) = (x.u16(COL), x.u16(ROW));
            x.set8(VOICED_PENDING, 1);
            x.set16(ROW, 4);
            let target = if wide(x) { 6 } else { 0xa };
            x.set16(COL, target);
            redraw_cell(x, col, row);
        }

        /// 0xc9e6a: erases the last character (or the whole buffer after a default name).
        fn erase_button<R: Bench>(x: &mut Cx<'_, R>) {
            if x.u8(LENGTH) == 0 {
                return;
            }
            let player = owner(x);
            if x.u8(NO_NAME) == 0 {
                x.set8(VOICED_PENDING, 0);
                x.add8(LENGTH, 0xff);
                let len = x.u8(LENGTH) as u32;
                wr8(x.b, player.0 + BUFFER + len, EMPTY);
            } else {
                x.set8(VOICED_PENDING, 0);
                x.set8(NO_NAME, 0);
                player.set_u32(x.b, BUFFER, u32::MAX);
                player.set_u32(x.b, BUFFER + 4, u32::MAX);
                x.set8(LENGTH, 0);
            }
        }

        /// 0xc9d16: loads the player's default name into the buffer.
        fn load_default_name<R: Bench>(x: &mut Cx<'_, R>) {
            x.set8(NO_NAME, 0);
            let player = owner(x);
            let mut offset = (player.u8(x.b, 3) as u16) << 3;
            if wide(x) {
                offset += 0x60;
            }
            let mut length = 0u8;
            for i in 0..8u32 {
                let mut c = x.data8(DEFAULT_NAMES + offset as u32 + i);
                wr8(x.b, player.0 + BUFFER + i, c);
                length = length.wrapping_add(1);
                if c == NAME_END {
                    length = length.wrapping_sub(1);
                    c = EMPTY;
                    wr8(x.b, player.0 + BUFFER + i, c);
                }
            }
            x.set8(LENGTH, length);
            if length == 6 {
                x.set8(VOICED_PENDING, 1);
            }
            if player.u8(x.b, BUFFER) >= NAME_END {
                x.set8(NO_NAME, 1);
                x.set8(LENGTH, 1);
            }
            let (col, row) = (x.u16(COL), x.u16(ROW));
            redraw_cell(x, col, row);
            x.set16(ROW, 4);
            let target = if wide(x) { 6 } else { 0xa };
            x.set16(COL, target);
        }

        /// 0xc9eb0: finishes the entry (empty name → default name), checks the secret codes, computes
        /// the check digit and writes the result into the player.
        fn finish<R: Bench>(x: &mut Cx<'_, R>) {
            let n;
            if x.u8(LENGTH) == 0 {
                load_default_name(x);
                x.add16(ROW, 0xfffe);
                n = special_codes(x);
                scan_reset_codes(x, n);
            } else {
                let (col, row) = (x.u16(COL), x.u16(ROW));
                redraw_cell(x, col, row);
                n = special_codes(x);
                scan_reset_codes(x, n);
                check_repeated_name(x);
            }
            check_digit(x);
            commit_name(x);
        }

        /// 0xca33c: a buffer of identical letters or entirely "confirm" becomes the default
        /// name again.
        fn check_repeated_name<R: Bench>(x: &mut Cx<'_, R>) {
            let player = owner(x);
            let b = |x: &mut Cx<'_, R>, i: u32| player.u8(x.b, BUFFER + i);
            let first = b(x, 0);
            let mut same = true;
            for i in 0..5 {
                if b(x, i) != first {
                    same = false;
                    break;
                }
            }
            if same {
                return load_default_name(x);
            }
            for i in 0..6 {
                let c = b(x, i);
                if c == EMPTY {
                    return load_default_name(x);
                }
                if c != ENTER {
                    return;
                }
            }
            load_default_name(x);
        }

        /// 0xca374: looks in the buffer for a sequence from the reset-code table.
        /// `n`: N flag left by 0xca524 (the scan only takes place if it is raised).
        fn scan_reset_codes<R: Bench>(x: &mut Cx<'_, R>, n: bool) {
            if !n {
                return;
            }
            let buffer = owner(x).0 + BUFFER;
            if find_code(x, buffer, RESET_CODES, 0).is_some() {
                load_default_name(x);
            }
        }

        /// Looks in the buffer's 6 bytes for a sequence from the table (length byte, characters,
        /// then `trailer` action bytes), like routines 0xca384 and 0xca54c: the first letter
        /// is searched further along in the buffer; a mismatch re-reads the faulty letter as the start of the
        /// sequence without rewinding the counter. Returns the address following the characters of the sequence found.
        fn find_code<R: Bench>(x: &mut Cx<'_, R>, buffer: u32, table: u32, trailer: u32) -> Option<u32> {
            enum Phase {
                Start,
                ReadLength,
                Search,
                Continue,
                NextPattern,
            }
            let (mut step, mut a4, mut a3, mut a1) = (Phase::Start, table, table, buffer);
            let (mut left, mut remaining, mut first) = (0u16, 5i32, 0u8);
            loop {
                match step {
                    Phase::Start => {
                        a1 = buffer;
                        remaining = 5;
                        a3 = a4;
                        step = Phase::ReadLength;
                    }
                    Phase::ReadLength => {
                        let length = x.data8(a4);
                        a4 += 1;
                        if (length as i8) < 0 {
                            return None;
                        }
                        left = length as u16;
                        first = x.data8(a4);
                        a4 += 1;
                        step = Phase::Search;
                    }
                    Phase::Search => {
                        let c = rd8(x.b, a1);
                        a1 += 1;
                        if c == first {
                            step = Phase::Continue;
                        } else {
                            remaining -= 1;
                            if remaining < 0 {
                                step = Phase::NextPattern;
                            }
                        }
                    }
                    Phase::Continue => {
                        left = left.wrapping_sub(1);
                        if left == 0 {
                            return Some(a4);
                        }
                        remaining -= 1;
                        if remaining < 0 {
                            step = Phase::NextPattern;
                            continue;
                        }
                        let want = x.data8(a4);
                        a4 += 1;
                        let c = rd8(x.b, a1);
                        a1 += 1;
                        if want != c {
                            a1 -= 1;
                            a4 = a3;
                            step = Phase::ReadLength;
                        }
                    }
                    Phase::NextPattern => {
                        a4 = a3;
                        let length = x.data8(a4) as u32;
                        a4 += 1 + length + trailer;
                        step = Phase::Start;
                    }
                }
            }
        }

        /// 0xca524: if the player holds the three buttons (0x70) for more than a second, looks for a
        /// special code and replaces the buffer. Returns the N flag left by the routine.
        fn special_codes<R: Bench>(x: &mut Cx<'_, R>) -> bool {
            let player = x.u8(PLAYER) as u32;
            let (_, pressed) = inputs(x, player);
            let n_of = |a: u8, b: u8| (a.wrapping_sub(b) as i8) < 0;
            if pressed != 0x70 {
                return n_of(pressed, 0x70);
            }
            let held_frames = x.u8(HELD_FRAMES);
            if held_frames <= 0x3c {
                return n_of(held_frames, 0x3c);
            }
            let buffer = owner(x).0 + BUFFER;
            match find_code(x, buffer, SPECIAL_CODES, 1) {
                None => true,
                Some(after) => {
                    let action = x.data8(after) as u32;
                    let src = SPECIAL_REPLACEMENTS + (action << 3);
                    let (a, b) = (x.data32(src), x.data32(src + 4));
                    wr32(x.b, buffer, a);
                    wr32(x.b, buffer + 4, b);
                    (b as i32) < 0
                }
            }
        }

        /// 0xca5fe: BCD check digit of the name (`+0x2a..+0x2d`, result in `+0x2b`).
        fn check_digit<R: Bench>(x: &mut Cx<'_, R>) {
            let buffer = owner(x).0 + BUFFER;
            let first = rd8(x.b, buffer) as i8 as i16;
            let d0 = (-first).wrapping_sub(2) as u16;
            if d0 < 0x10 {
                let v = x.rom8(DEFAULT_CHECK + (d0 & 0xff) as u32);
                x.set8(0x2b, v);
                return;
            }
            for i in 0..6 {
                let c = rd8(x.b, buffer + i);
                if c == 0xff {
                    continue;
                }
                x.set8(0x2c, 0);
                let digit = x.data8(CHAR_DIGITS + c as u32);
                x.set8(0x2d, digit);
                add_bcd(x);
            }
            while x.u8(0x2a) != 0 {
                let carry_in = x.u8(0x2a);
                x.set8(0x2d, carry_in);
                x.set8(0x2c, 0);
                x.set8(0x2a, 0);
                add_bcd(x);
            }
            while x.u8(0x2b) >= 0x10 {
                let high = x.u8(0x2b) >> 4;
                let low = x.u8(0x2b) & 0xf;
                x.set8(0x2b, low);
                x.set8(0x2d, high);
                let (v, _) = abcd(x.u8(0x2d), x.u8(0x2b), false);
                x.set8(0x2b, v);
            }
        }

        /// Adds the BCD value (`+0x2c:+0x2d`) to (`+0x2a:+0x2b`) with carry (two ABCD).
        fn add_bcd<R: Bench>(x: &mut Cx<'_, R>) {
            let (low, carry) = abcd(x.u8(0x2d), x.u8(0x2b), false);
            x.set8(0x2b, low);
            let (high, _) = abcd(x.u8(0x2c), x.u8(0x2a), carry);
            x.set8(0x2a, high);
        }

        /// ABCD: `dst + src + carry` in BCD; returns (result, carry).
        fn abcd(src: u8, dst: u8, carry: bool) -> (u8, bool) {
            let mut r = (src & 0xf) as u16 + (dst & 0xf) as u16 + carry as u16;
            if r > 9 {
                r += 6;
            }
            r += (src & 0xf0) as u16 + (dst & 0xf0) as u16;
            let carry = r > 0x99;
            if carry {
                r -= 0xa0;
            }
            (r as u8, carry)
        }

        /// 0xca0f4: writes the result: end indicator on screen, player record
        /// (validated name, check digit), the cursor goes to state 4.
        fn commit_name<R: Bench>(x: &mut Cx<'_, R>) {
            let player = owner(x);
            let kind = player.u8(x.b, 2) as u16;
            let mut index = kind.wrapping_mul(2);
            if x.a5_u8(BLINK_FLAGS) & 1 != 0 {
                index = index.wrapping_add(8);
            }
            let offset = x.rom16(FINISH_POSITIONS.wrapping_add(index as i16 as i32 as u32));
            let mut a1 = TILEMAP.wrapping_add(offset as i16 as i32 as u32);
            let mut tile: u32 = 0x6700013;
            tile = tile & !0xff | (tile as u8).wrapping_add(kind as u8) as u32;
            for _ in 0..4 {
                wr32(x.b, a1, tile);
                a1 += 0x80;
                tile = tile.wrapping_add(0x10000);
            }
            let swapped = tile.rotate_left(16);
            let swapped = swapped & !0xff | (swapped as u8).wrapping_add(x.u8(0x2b)) as u32;
            wr32(x.b, a1, swapped.rotate_left(16));
            x.set32(0x4, 0x4000000);
            x.set8(DONE, 1);
            x.set8(ACTIVE, 0);
            let check = (x.u8(0x2b) << 2) as u32;
            let record = {
                let at = FINISH_RECORDS + check;
                (x.rom16(at) as u32) << 16 | x.rom16(at + 2) as u32
            };
            let variant = (player.u8(x.b, 3) >> 1) as u32;
            let a1 = record + variant * 5;
            let (i0, v0, i1, v1, flag) = (x.data8(a1) as u32, x.data8(a1 + 1), x.data8(a1 + 2) as u32, x.data8(a1 + 3), x.data8(a1 + 4) as u32);
            wr8(x.b, player.0 + 0x160 + i0, v0);
            if (i1 as u8 as i8) >= 0 {
                wr8(x.b, player.0 + 0x16b + i1, v1);
            }
            wr8(x.b, FLAGS_ARRAY + flag, 1);
        }

        /// 0xca2f8: state 6. If the player is still there and the entry is not finished, finishes it automatically;
        /// then release.
        fn finish_and_free<R: Bench>(x: &mut Cx<'_, R>) {
            let player = owner(x);
            if player.u8(x.b, 0) != 0 && (player.u8(x.b, 4) as i8) < 4 && x.u8(DONE) == 0 {
                x.set8(DONE, 1);
                finish(x);
            }
            x.free(&pools::SET);
        }
    }
}

/// Declares a handler native: address, name, function.
macro_rules! entry {
    ($entry:literal $name:literal, $(#[$without:ident])? $f:path) => {
        super::cx::handlers!(@one $entry $name $(#[$without])? $f)
    };
}

pub const NATIVES: &[crate::Native] = &[
    entry!(0xcc25a "set_common_cc25a", #[without] scene_spawner::run),
    entry!(0xba310 "set_0", #[without] palette_script::run),
    entry!(0xbc518 "set_1", #[without] wave_leader::run),
    entry!(0xc935a "set_4", #[without] tile_anim::run4),
    entry!(0xc9780 "set_5", #[without] name_entry::run),
    entry!(0xcafec "set_6", #[without] tile_anim::run6),
    entry!(0xcb29c "set_7", #[without] exit_gate::run),
    entry!(0xcc2c8 "set_9", #[without] inert::run),
    entry!(0xcc3c2 "set_10", #[without] dialogue::run),
    entry!(0xcdcaa "set_11", #[without] level_end::run),
    entry!(0xcdd70 "set_12", #[without] fade_white::run),
    entry!(0xcdef6 "set_13", #[without] palette_anim::run),
    entry!(0xcdf86 "set_14", #[without] scene_trigger::run),
];
