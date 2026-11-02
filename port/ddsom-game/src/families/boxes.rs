//! BOX family (class 0xA): destructible crates, barrels and chests. Dispatch `0x39e50`, type table
//! `0x39e76` (4 types). 21 slots of 0x80 bytes. See `docs/familles.md`.

use crate::objects::pools;
use crate::objects::Obj;

use super::{add_u8, handler, pointer};
use super::cx::{Bench, Cx};
use crate::objects::Current;
use crate::machine::Machine;

/// Values of the state byte `+4`.
mod state {
    pub const INIT: u8 = 0;
    pub const ACTIVE: u8 = 2;
    pub const FREED: u8 = 4;
    pub const OUTSIDE_SCREEN: u8 = 6;
}

/// Common BOX fields.
mod field {
    pub const SUB_STATE: u32 = 0x05;
    pub const MODE: u32 = 0x2f;
    pub const GAME_SHADOWS: u32 = 0x2e;
    /// Hit received, set by the players' code.
    pub const HIT_B: u32 = 0x6e;
}

mod rom {
    pub const ANIMATIONS: u32 = 0x1bebc2;
}

const SHADOWS: u8 = 0x35;

// ---------------------------------------------------------------------------------------------
// Type 0: crate, barrel or chest (0xd99fa)

/// Type 0: destructible crate, barrel or chest (variants 0 to 3). A ROM sheet gives its
/// characteristics; when struck, it plays its breaking animation, drops up to three ITEM objects then
/// blinks and disappears. A chest (kind >= 3) shakes when struck; it opens (substate 2)
/// when a player opens it, consuming a key if it is locked. A player can also lift the
/// crate (substate 4: it follows their hands), throw it (6: flight, bounce off the screen edge,
/// breaks on landing) or drop it (8: fall).
pub mod chest {
use crate::families::cx::Bench;
use crate::objects::Current;
    use crate::families::cx::{handlers, Cx};
    use crate::mem::A5;
    use crate::objects::{pools, Obj};
    use crate::families::effects_creators;
    use crate::machine::Machine;

    /// Substates `+5` of the active state.
    mod sub {
        pub const INTACT: u8 = 0;
        pub const OPEN: u8 = 2;
        pub const RANGE: u8 = 4;
        pub const LAUNCHED: u8 = 6;
        pub const DROPPED: u8 = 8;
        pub const STRUCK: u8 = 0x0a;
    }

    mod f {
        pub const ACTIVE: u32 = 0x00;
        pub const FLAGS: u32 = 0x01;
        pub const VARIANT: u32 = 0x03;
        pub const STATE: u32 = 0x04;
        pub const SUB_STATE: u32 = 0x05;
        pub const SUB_SUB_STATE: u32 = 0x06;
        pub const X: u32 = 0x08;
        pub const Z: u32 = 0x0c;
        pub const Y: u32 = 0x10;
        pub const VX: u32 = 0x14;
        pub const AX: u32 = 0x16;
        pub const VZ: u32 = 0x18;
        pub const AZ: u32 = 0x1a;
        pub const VY: u32 = 0x1c;
        pub const TURNED: u32 = 0x2c;
        pub const GAME_SHADOWS: u32 = 0x2e;
        /// Hit received, second mark set by the players' code.
        pub const HIT: u32 = 0x33;
        /// Non-zero: in the air (no shadow on the ground).
        pub const IN_FLIGHT: u32 = 0x3a;
        pub const HIDDEN: u32 = 0x40;
        /// Delay before the thrown crate resumes its flight (`+0x53`, modulo 8).
        pub const WAIT_FLIGHT: u32 = 0x53;
        pub const SELF: u32 = 0x5c;
        /// Kind (0 to 2: breaks in one hit; 3 and above: chest that resists).
        pub const KIND: u32 = 0x60;
        /// Non-zero: locked chest (a key `A5+0x45` opens it, the class 5 character does without).
        pub const LOCK: u32 = 0x61;
        /// Loot: in the center, by player class (`+0x63..`), bonus (`+0x69`), on the left (`+0x6a`).
        pub const LOOT_HIGH: u32 = 0x62;
        pub const LOOT_CLASSES: u32 = 0x63;
        pub const LOOT_BONUS: u32 = 0x69;
        pub const LOOT_LEFT: u32 = 0x6a;
        /// Possible special effect (`+0x6b`), "effect played" mark (`+0x7d`).
        pub const SPECIAL: u32 = 0x6b;
        /// Player who struck, opened or carries the crate (16-bit RAM pointer).
        pub const PLAYER: u32 = 0x6c;
        pub const HIT_B: u32 = 0x6e;
        pub const DELAY: u32 = 0x6f;
        /// Chest opened by a class 3 player: a second hit drops the bonus loot.
        pub const SECOND_HIT: u32 = 0x70;
        /// Non-zero: the thrown crate ignores the terrain.
        pub const WITHOUT_SCENERY: u32 = 0x71;
        pub const EFFECT_CREATED: u32 = 0x76;
        /// Non-zero: the opened chest can still be struck.
        pub const HITTABLE: u32 = 0x77;
        pub const ORIGINAL_X: u32 = 0x78;
        pub const ORIGINAL_Y: u32 = 0x7a;
        pub const LOOT_DROPPED: u32 = 0x7c;
        pub const SPECIAL_PLAYED: u32 = 0x7d;
        pub const BONUS_DROPPED: u32 = 0x7e;
        /// The thrown chest bounced off the terrain.
        pub const BOUNCE: u32 = 0x7f;
    }

    /// Fields of the linked player.
    mod j {
        pub const CLASS: u32 = 0x03;
        pub const TURNED: u32 = 0x2c;
        /// Pose of the carrying player (0: no longer carrying).
        pub const POSE: u32 = 0x46;
    }

    /// Tables and subroutines.
    mod rom {
        pub const STATES: u32 = 0xd9a10;
        pub const SUB_STATES: u32 = 0xd9a6a;
        pub const STRUCK: u32 = 0xd9abc;
        pub const OPEN: u32 = 0xd9b94;
        pub const LAUNCHED: u32 = 0xd9d64;
        pub const DROPPED: u32 = 0xd9f82;
        pub const ANIMATIONS: u32 = 0x1bebc2;
        /// Per kind: address of its 0x2a-byte sheet (characteristics, random loot).
        pub const SHEETS: u32 = 0x1051ae;
        /// Per loot number: (type, variant) of the created ITEM (read PC-relative).
        pub const LOOTS: u32 = 0xda28a;
        /// Special effects (routine addresses), by value of `SPECIAL`.
        pub const SPECIALS: u32 = 0xda06a;
        /// Per carrier class: table of offsets (X, Z) of the carried crate, per pose.
        pub const TAKEN: u32 = 0xda398;
        // subroutines not rewritten, or native
        pub const SOUND_HIT: u32 = 0xeb59e;
        pub const SOUND_BROKEN: u32 = 0xeb610;
        pub const SOUND_LANDS: u32 = 0xeb52c;
        pub const SOUND_SET_DOWN: u32 = 0xeb766;
    }

    /// Crates broken in the scene (byte, `A5-0x5b21`).
    const BROKEN: u32 = A5 - 0x5b21;
    /// Players' keys (byte, `A5+0x45`).
    const KEYS: u32 = A5 + 0x45;
    const SHADOWS: u8 = 0x35;
    const DURATION_BLINK: u8 = 0x1e;

    fn run<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(f::HIDDEN) != 0 {
            return;
        }
        let state = x.u8(f::STATE);
        let target = jump(x, rom::STATES, state);
        match state {
            0 => init(x),
            2 => active(x),
            4 | 6 => x.free(&pools::BOX),
            _ => crate::families::effects::outside_table(target),
        }
    }

    /// Target of the jump through table `move.w (table,PC,D0.w),D1.w ; jmp` for index `i` (`outside_table`).
    fn jump<R: Bench>(x: &mut Cx<'_, R>, table: u32, i: u8) -> u32 {
        let w = x.rom16(table + i as u32);
        table.wrapping_add(w as i16 as i32 as u32)
    }

    /// Class of the linked player (`+3 >> 1`).
    fn class<R: Bench>(x: &mut Cx<'_, R>) -> u8 {
        let player = x.linked(f::PLAYER);
        player.u8(x.b, j::CLASS) >> 1
    }

    fn count_broken<R: Bench>(x: &mut Cx<'_, R>) {
        let n = crate::mem::rd8(x.b, BROKEN).wrapping_add(1);
        crate::mem::wr8(x.b, BROKEN, n);
    }

    /// `0xda082`: copies the kind's sheet (10 bytes) and draws two random loots.
    fn charge_sheet<R: Bench>(x: &mut Cx<'_, R>) {
        let variant = x.u8(f::VARIANT) as u32;
        let sheet = x.data32(rom::SHEETS + 4 * variant);
        let fields = [f::KIND, f::LOCK, f::LOOT_HIGH, f::LOOT_BONUS, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68];
        for (i, off) in fields.into_iter().enumerate() {
            let v = x.data8(sheet + i as u32);
            x.set8(off, v);
        }
        let draw = (x.random() & 0xf) as u32;
        let v = x.data8(sheet + 0xa + draw);
        x.set8(f::LOOT_LEFT, v);
        let draw = (x.random() & 0xf) as u32;
        let v = x.data8(sheet + 0x1a + draw);
        x.set8(f::SPECIAL, v);
        // sheet read pointers left by the original
    }

    fn init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(f::STATE, 2);
        x.set8(f::SECOND_HIT, 0);
        x.set8(f::HIT_B, 0);
        x.set8(f::GAME_SHADOWS, SHADOWS);
        let px = x.u16(f::X);
        x.set16(f::ORIGINAL_X, px);
        let py = x.u16(f::Y);
        x.set16(f::ORIGINAL_Y, py);
        x.set16(f::SELF, x.o.0 as u16);
        charge_sheet(x);
        if x.u8(f::SPECIAL) != 0 {
            effects_creators::shimmer(x.b, x.o);
        }
        let kind = x.u8(f::KIND) as u32;
        x.anim(rom::ANIMATIONS, kind);
    }

    fn active<R: Bench>(x: &mut Cx<'_, R>) {
        let sub_state = x.u8(f::SUB_STATE);
        let target = jump(x, rom::SUB_STATES, sub_state);
        match sub_state {
            sub::INTACT => intact(x),
            sub::OPEN => {
                let n = x.u8(f::SUB_SUB_STATE);
                let target = jump(x, rom::OPEN, n);
                match n {
                    0 => opening(x),
                    2 => open_wait(x),
                    4 => blinks(x),
                    _ => crate::families::effects::outside_table(target),
                }
            }
            sub::RANGE => {
                x.set8(f::BOUNCE, 0);
                follow_carrier(x);
            }
            sub::LAUNCHED => launched(x),
            sub::DROPPED => {
                let n = x.u8(f::SUB_SUB_STATE);
                let target = jump(x, rom::DROPPED, n);
                match n {
                    0 => {
                        x.add8(f::SUB_SUB_STATE, 2);
                        x.set16(f::VZ, 0xff00);
                        x.set16(f::AZ, 0xffe0);
                    }
                    2 => dropped_fall(x),
                    4 => dropped_broken(x),
                    6 => blinks(x),
                    _ => crate::families::effects::outside_table(target),
                }
            }
            sub::STRUCK => {
                let n = x.u8(f::SUB_SUB_STATE);
                let target = jump(x, rom::STRUCK, n);
                match n {
                    0 => broken(x),
                    2 => blinks(x),
                    4 => tremble(x),
                    _ => crate::families::effects::outside_table(target),
                }
            }
            _ => crate::families::effects::outside_table(target),
        }
        x.reinsert();
    }

    /// Intact: a hit (`HIT_B`) makes it go to the "hit" state; a chest immediately starts
    /// trembler.
    fn intact<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(f::HIT_B) == 0 {
            return;
        }
        x.set32(f::STATE, 0x020a_0000);
        if x.u8(f::KIND) >= 3 {
            x.set8(f::SUB_SUB_STATE, 4);
            x.set8(f::ACTIVE, 2);
            x.set8(f::DELAY, 8);
            crate::sound::play(x.b, rom::SOUND_BROKEN, x.o);
        }
    }

    /// `0xda04e`: plays the crate's special effect, only once (`moment` in D1).
    fn effect_special<R: Bench>(x: &mut Cx<'_, R>, moment: u32) {
        if x.u8(f::SPECIAL_PLAYED) != 0 {
            return;
        }
        x.set8(f::SPECIAL_PLAYED, 1);
        let i = 4 * x.u8(f::SPECIAL) as u32;
        let effect = x.rom32(rom::SPECIALS + i);
        // ESHL creators designated by the original table
        use crate::families::eshl_creators as eshl;
        let r = &mut crate::families::effects_creators::leftovers::Pure;
        match effect {
            0xa1360 => eshl::single_18(r, x.b, x.o),
            0xa1a40 => {
                eshl::single_1b(r, x.b, x.o, moment as u16);
            }
            0xa1670 => {
                eshl::row_19(r, x.b, x.o);
            }
            // Original glitch: the D5 left by what precedes (`Leftovers`) sets the high byte of the spark count.
            0xa17d0 => {
                let d5 = *x.c.d5() as u16;
                eshl::row_1a(r, x.b, x.o, moment as u16, d5);
            }
            0xa1c6a => eshl::four_1c(r, x.b, x.o),
            _ => crate::families::effects::outside_table(effect),
        }
    }

    /// Creates the ITEM `loot` (type and variant read from `LOOTS`) above the crate, thrown with
    /// speeds `(vx, vy)`. `None` if the ITEM pool is full.
    fn created_loot<R: Bench>(x: &mut Cx<'_, R>, loot: u8, vx: u16, vy: u16) -> Option<Obj> {
        let n = x.alloc(&pools::ITEM)?;
        let rank = 2 * loot as u32;
        n.set_u8(x.b, f::ACTIVE, 1);
        let kind = x.data8(rom::LOOTS + rank);
        n.set_u8(x.b, 0x2, kind);
        let variant = x.data8(rom::LOOTS + rank + 1);
        n.set_u8(x.b, f::VARIANT, variant);
        for off in [f::X, f::Z, f::Y] {
            let v = x.u16(off);
            n.set_u16(x.b, off, v);
        }
        let z = n.u16(x.b, f::Z).wrapping_add(0x18);
        n.set_u16(x.b, f::Z, z);
        let y = n.u16(x.b, f::Y).wrapping_sub(1);
        n.set_u16(x.b, f::Y, y);
        n.set_u16(x.b, f::VX, vx);
        n.set_u16(x.b, f::VZ, 0x300);
        n.set_u16(x.b, f::AZ, 0xffd8);
        n.set_u16(x.b, f::VY, vy);
        Some(n)
    }

    /// `0xda152`: the broken crate drops its loot (only once): one object in the center, one depending on the
    /// class of the player who struck (thrown to the right) and one drawn at random (thrown to the left).
    fn dropped_loot<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(f::LOOT_DROPPED) != 0 {
            return;
        }
        x.set8(f::LOOT_DROPPED, 1);
        let high = x.u8(f::LOOT_HIGH);
        if high != 0 && created_loot(x, high, 0, 0).is_none() {
            return;
        }
        let class = class(x) as u32;
        let right = x.u8(f::LOOT_CLASSES + class);
        if right != 0 && created_loot(x, right, 0x140, 0x40).is_none() {
            return;
        }
        let left = x.u8(f::LOOT_LEFT);
        if left != 0 {
            created_loot(x, left, 0xfec0, 0xffc0);
        }
    }

    /// `0xda0e6`: the bonus loot, only once.
    fn dropped_bonus<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(f::BONUS_DROPPED) != 0 {
            return;
        }
        x.set8(f::BONUS_DROPPED, 1);
        let bonus = x.u8(f::LOOT_BONUS);
        if bonus != 0 {
            created_loot(x, bonus, 0, 0);
        }
    }

    /// Final blinking: `DELAY` countdown, the crate blinks; at zero, next state.
    fn blinks<R: Bench>(x: &mut Cx<'_, R>) {
        let remainder = x.u8(f::DELAY).wrapping_sub(1);
        x.set8(f::DELAY, remainder);
        if remainder == 0 {
            x.add8(f::STATE, 2);
        }
        x.set_bit(f::FLAGS, 1, remainder & 1 != 0);
    }

    /// Hit, sub-substate 0: the crate plays its breaking animation and drops its loot; a chest
    /// redevient intact.
    fn broken<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(f::KIND) >= 3 {
            x.set32(f::STATE, 0x0200_0000);
            x.set8(f::HIT_B, 0);
            x.set8(f::ACTIVE, 1);
            return;
        }
        if x.u8(f::HIT) != 0 {
            x.set8(f::HIT, 0);
            dropped_loot(x);
            crate::sound::play(x.b, rom::SOUND_HIT, x.o);
            if x.u8(f::SPECIAL) != 0 {
                effect_special(x, 0);
            }
        }
        if x.anim_step() {
            x.add8(f::SUB_SUB_STATE, 2);
            x.set8(f::ACTIVE, 2);
            x.set8(f::DELAY, DURATION_BLINK);
            count_broken(x);
        }
    }

    /// Hit, sub-substate 4: the chest shakes (±2 in X, ±1 in Y every other frame), then
    /// goes back to its place.
    fn tremble<R: Bench>(x: &mut Cx<'_, R>) {
        let remainder = x.u8(f::DELAY).wrapping_sub(1);
        x.set8(f::DELAY, remainder);
        if remainder == 0 {
            x.set32(f::STATE, 0x0200_0000);
            x.set8(f::HIT_B, 0);
            x.set8(f::ACTIVE, 1);
            let ox = x.u16(f::ORIGINAL_X);
            x.set16(f::X, ox);
            let oy = x.u16(f::ORIGINAL_Y);
            x.set16(f::Y, oy);
            return;
        }
        x.add16(f::X, 2);
        x.add16(f::Y, 1);
        if remainder & 1 == 0 {
            x.add16(f::X, 0xfffc);
            x.add16(f::Y, 0xfffe);
        }
    }

    /// Opened, sub-substate 0 (`0xd9b9a`): a locked chest consumes a key (except for class
    /// 5) or refuses to open; the special effect is created once; then opening animation and
    /// butin au premier coup.
    fn opening<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(f::LOCK) != 0 && class(x) != 5 {
            let keys = crate::mem::rd8(x.b, KEYS);
            if keys == 0 {
                x.set32(f::STATE, 0x0200_0000);
                x.set8(f::ACTIVE, 1);
                spark_placed(x, 6);
                return;
            }
            crate::mem::wr8(x.b, KEYS, keys - 1);
            x.set8(f::LOCK, 0);
        }
        let special = x.u8(f::SPECIAL);
        if special != 0 {
            if x.u8(f::EFFECT_CREATED) == 0 {
                if spark_placed(x, special).is_some() {
                    x.set8(f::EFFECT_CREATED, 1);
                }
            }
            if x.u8(f::HITTABLE) == 0 {
                return;
            }
        }
        if x.u8(f::HIT) != 0 {
            x.set8(f::HIT, 0);
            if x.u8(f::SPECIAL) != 0 {
                effect_special(x, 1);
            }
            dropped_loot(x);
            crate::sound::play(x.b, rom::SOUND_HIT, x.o);
        }
        if !x.anim_step() {
            return;
        }
        x.add8(f::SUB_SUB_STATE, 2);
        x.set8(f::DELAY, 0xa);
        count_broken(x);
        if class(x) == 3 {
            x.set8(f::ACTIVE, 1);
            x.set8(f::SECOND_HIT, 1);
            x.set8(f::DELAY, 0xff);
            x.set8(f::HIT_B, 0);
        }
    }

    /// Opened, sub-substate 2 (`0xd9c5a`): delay, then blinking; opened by class 3, a
    /// new hit drops the bonus then breaks the crate (or makes the chest blink).
    fn open_wait<R: Bench>(x: &mut Cx<'_, R>) {
        let remainder = x.u8(f::DELAY).wrapping_sub(1);
        x.set8(f::DELAY, remainder);
        if remainder == 0 {
            x.add8(f::SUB_SUB_STATE, 2);
            x.set8(f::ACTIVE, 2);
            x.set8(f::DELAY, DURATION_BLINK);
        }
        if x.u8(f::SECOND_HIT) == 0 || x.u8(f::HIT_B) == 0 {
            return;
        }
        if class(x) == 3 {
            dropped_bonus(x);
        }
        if x.u8(f::KIND) < 3 {
            x.add8(f::STATE, 2);
            shards(x);
        } else {
            x.set8(f::SUB_SUB_STATE, 4);
            x.set8(f::ACTIVE, 2);
            x.set8(f::DELAY, DURATION_BLINK);
        }
    }

    /// `moveq #2,D0 ; moveq #0,D1 ; jsr 0xab7d8`: spray of sparks from the crate.
    fn shards<R: Bench>(x: &mut Cx<'_, R>) {
        effects_creators::debris(x.b, x.o, 2);
    }

    /// `0xb1352`: spark placed on the crate; `None` if the pool is full.
    fn spark_placed<R: Bench>(x: &mut Cx<'_, R>, variant: u8) -> Option<Obj> {
        effects_creators::posed_spark(x.b, x.o, variant)
    }

    /// `0xd9cea`: the carried crate follows the player's hands (offset per class and pose, flipped
    /// with them); nothing if the player no longer carries.
    fn follow_carrier<R: Bench>(x: &mut Cx<'_, R>) {
        let player = x.linked(f::PLAYER);
        let class = player.u8(x.b, j::CLASS) as u32;
        let taken = x.data32(rom::TAKEN + 4 * class);
        let pose = player.u8(x.b, j::POSE);
        if pose == 0 {
            return;
        }
        let i = (4 * ((4 * class & 0xff00) | pose as u32)) & 0xffff;
        let mut dx = x.data16(taken + i);
        let dz = x.data16(taken + i + 2);
        let turned = player.u8(x.b, j::TURNED);
        if turned != 0 {
            dx = dx.wrapping_neg();
        }
        x.set8(f::TURNED, turned);
        x.copy_pos(player);
        x.add16(f::X, dx);
        x.add16(f::Z, dz);
    }

    /// Thrown (`0xd9d38`): waits `WAIT_FLIGHT`, then flies (sub-substates) and realigns itself with the terrain.
    fn launched<R: Bench>(x: &mut Cx<'_, R>) {
        let wait_kind = x.u8(f::WAIT_FLIGHT);
        if wait_kind != 0 {
            x.set8(f::WAIT_FLIGHT, wait_kind.wrapping_sub(1) & 7);
            return;
        }
        let n = x.u8(f::SUB_SUB_STATE);
        let target = jump(x, rom::LAUNCHED, n);
        match n {
            0 => launched_start(x),
            2 => launched_flight(x),
            4 => launched_lands(x),
            6 => blinks(x),
            _ => crate::families::effects::outside_table(target),
        }
        x.push_terrain();
    }

    /// Thrown, 0: as long as the player holds it, it follows their hands; once released, it leaves in an arc in front
    /// of
    /// them (and immediately starts its flight).
    fn launched_start<R: Bench>(x: &mut Cx<'_, R>) {
        let player = x.linked(f::PLAYER);
        if player.u8(x.b, j::POSE) != 0 {
            return follow_carrier(x);
        }
        x.add8(f::SUB_SUB_STATE, 2);
        x.set8(f::ACTIVE, 1);
        x.set8(f::IN_FLIGHT, 1);
        x.set16(f::VX, 0x380);
        x.set16(f::AZ, 0xffc8);
        x.set16(f::AX, 0);
        x.set16(f::VZ, 0);
        if player.u8(x.b, j::TURNED) == 0 {
            x.set16(f::VX, 0xfc80);
        }
        launched_flight(x);
    }

    /// Terrain under the crate (`0x24a8`, D2): true if it is an obstacle (attributes 5 to 8).
    fn obstacle<R: Bench>(x: &mut Cx<'_, R>) -> bool {
        let t = crate::video::terrain_of(x.b, x.o);
        // Original glitch: 0x24a8 leaves its work registers as leftovers (D2-D4)
        crate::families::player::pshell::leftovers_terrain(x, &t);
        (t.number & 0x3f).wrapping_sub(5) <= 3
    }

    /// Advances in X and Z (`0x1da6`); returns Z.
    fn advance<R: Bench>(x: &mut Cx<'_, R>) -> i16 {
        x.move_xz();
        x.u16(f::Z) as i16
    }

    /// Thrown, 2: a crate breaks against an obstacle or on touching the ground; a chest bounces once
    /// off the obstacle, then falls back.
    fn launched_flight<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(f::KIND) < 3 {
            if x.u8(f::WITHOUT_SCENERY) == 0 && !obstacle(x) && advance(x) >= 0 {
                return;
            }
            x.add8(f::STATE, 2);
            if x.u8(f::SECOND_HIT) != 0 && class(x) == 3 {
                dropped_bonus(x);
            } else {
                dropped_loot(x);
            }
            shards(x);
            crate::sound::play(x.b, rom::SOUND_LANDS, x.o);
            return count_broken(x);
        }
        let bounces = x.u8(f::BOUNCE) == 0 && (x.u8(f::WITHOUT_SCENERY) != 0 || obstacle(x));
        let pose = if bounces {
            x.set8(f::BOUNCE, 1);
            x.set8(f::WITHOUT_SCENERY, 0);
            let vx = x.u16(f::VX).wrapping_neg();
            x.set16(f::VX, vx);
            let r = x.u8(f::TURNED) ^ 0x20;
            x.set8(f::TURNED, r);
            advance(x) <= 0
        } else {
            advance(x) < 0
        };
        if pose {
            crate::sound::play(x.b, rom::SOUND_SET_DOWN, x.o);
            x.add8(f::SUB_SUB_STATE, 2);
            x.set8(f::WITHOUT_SCENERY, 0);
            x.set16(f::Z, 0);
            x.set8(f::IN_FLIGHT, 0);
            x.set8(f::BOUNCE, 0);
            x.set16(f::VX, 0x200);
            x.set16(f::AX, 0);
            x.set16(f::VZ, 0x300);
            x.set16(f::AZ, 0xffc8);
            if x.u8(f::TURNED) == 0 {
                x.set16(f::VX, 0xfe00);
            }
        }
        stays_on_screen(x);
    }

    /// Thrown, 4: the chest falls back after its bounce; once placed, it becomes intact again (or, opened by
    /// class 3, waits for its blinking while dropping the bonus).
    fn launched_lands<R: Bench>(x: &mut Cx<'_, R>) {
        if advance(x) >= 0 {
            return stays_on_screen(x);
        }
        if x.u8(f::SECOND_HIT) != 0 && class(x) == 3 {
            x.add8(f::SUB_SUB_STATE, 2);
            x.set8(f::ACTIVE, 2);
            x.set8(f::DELAY, DURATION_BLINK);
            return dropped_bonus(x);
        }
        x.set32(f::STATE, 0x0200_0000);
        x.set16(f::Z, 0);
        x.set8(f::HIT_B, 0);
        stays_on_screen(x);
    }

    /// `0xd9f06`: the crate stays between the screen edges (camera X + 0x18 to + 0x168); its
    /// position becomes its new origin.
    fn stays_on_screen<R: Bench>(x: &mut Cx<'_, R>) {
        let cam2 = crate::ram::CAMERA_ALT_ACTIVE.get(x.b) != 0;
        let cam_x = x.a5_u16(if cam2 { 0x556 } else { 0x506 });
        let left = cam_x.wrapping_add(0x18);
        let px = x.u16(f::X);
        if (left as i16) > px as i16 {
            x.set16(f::X, left);
        } else {
            let right = cam_x.wrapping_add(0x168);
            if right <= px {
                x.set16(f::X, right);
            }
        }
        let px = x.u16(f::X);
        x.set16(f::ORIGINAL_X, px);
        let py = x.u16(f::Y);
        x.set16(f::ORIGINAL_Y, py);
    }

    /// Dropped, 2: fall; on the ground, a chest with no special effect becomes intact again, otherwise the crate waits.
    fn dropped_fall<R: Bench>(x: &mut Cx<'_, R>) {
        x.move_z();
        if x.u16(f::Z) as i16 >= 0 {
            return;
        }
        x.add8(f::SUB_SUB_STATE, 2);
        x.set32(f::Z, 0);
        if (x.u8(f::KIND) as i8) < 3 || x.u8(f::SPECIAL) != 0 {
            return;
        }
        x.set32(f::STATE, 0x0200_0000);
        x.set8(f::ACTIVE, 1);
        x.set8(f::HIT_B, 0);
        let px = x.u16(f::X);
        x.set16(f::ORIGINAL_X, px);
        let py = x.u16(f::Y);
        x.set16(f::ORIGINAL_Y, py);
        crate::sound::play(x.b, rom::SOUND_SET_DOWN, x.o);
    }

    /// Dropped, 4: on the first hit, loot and special effect; end of animation: blinking.
    fn dropped_broken<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(f::HIT) != 0 {
            x.set8(f::HIT, 0);
            dropped_loot(x);
            crate::sound::play(x.b, rom::SOUND_HIT, x.o);
            count_broken(x);
            if x.u8(f::SPECIAL) != 0 {
                effect_special(x, 2);
            }
        }
        if x.anim_step() {
            x.add8(f::SUB_SUB_STATE, 2);
            x.set8(f::ACTIVE, 2);
            x.set8(f::DELAY, DURATION_BLINK);
        }
    }

    handlers! {
        0xd99fa "box_crate" #[without] run,
    }
}

// ---------------------------------------------------------------------------------------------
// Type 2: inert scenery (0xda7ac)

/// Type 2: scenery element with no behavior (animation 5 of the BOX set); the object initializes itself,
/// then just sorts itself every frame until its release. Not exercised by the scenarios.
mod scenery {
    pub const ENTRY: u32 = 0xda7ac;
    pub const TABLE_STATES: u32 = 0xda7c2;
    pub const ANIMATION: u16 = 5;
}

fn box_2<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let cache = o.u8(ctx.b, Obj::HIDDEN) != 0;
    if cache {
        return;
    }
    let state = o.u8(ctx.b, Obj::STATE);
    let target = super::effects::target(ctx, scenery::TABLE_STATES, state as u32);
    match state {
        state::INIT => {
            add_u8(ctx, o, Obj::STATE, 2);
            o.set_u8(ctx.b, Obj::ACTIVE, 2);
            o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
            ctx.anim_start(rom::ANIMATIONS, scenery::ANIMATION);
            ctx.reinsert();
        }
        state::ACTIVE => ctx.reinsert(),
        state::FREED | state::OUTSIDE_SCREEN => ctx.free(&pools::BOX),
        _ => crate::families::effects::outside_table(target),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 1: barrier to open (0xda578)

/// Type 1: interactive scenery element struck by the players (`+0x6e`, set by their hit code):
/// variant 0, grid that creates a SET object (`0x8b`, from which a door comes out) then waits for it to end; variant
/// 1, two-phase mechanism whose animation changes the state of the striking object (`+0x6c`);
/// variant 3 like 0 at initialization. The object disappears when the camera has passed it by 0x200.
/// Not exercised.
mod barrier {
    pub const ENTRY: u32 = 0xda578;
    pub const STATES: u32 = 0xda58e;
    pub const INITS: u32 = 0xda5a6;
    pub const ACTIVE_COUNT: u32 = 0xda5f4;
    pub const SUB_STATES_0: u32 = 0xda624;
    pub const SUB_STATES_1: u32 = 0xda65e;
    pub const ANIMATIONS_0: u32 = 0x1bed06;
    pub const ANIMATIONS_1: u32 = 0x1712d4;
    pub const POSITIONS: u32 = 0xda72c;
    pub const SOUND: u32 = 0xeae86;
    pub const HITTER: u32 = 0x6c;
    pub const SET_CREATED: u32 = 0x7c;
}

/// `0xda6f4`: creates the barrier's SET object and attaches it.
fn barrier_created_set<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let Some(new) = super::creators::scene_set(ctx.b, 0x8b) else { return };
    let hitter = pointer(ctx, o, barrier::HITTER);
    let mode = (o.u8(ctx.b, field::MODE) as u32) << 3;
    let class = ((hitter.u8(ctx.b, 3) & 0xf) >> 1) as u32;
    let position = ctx.prog8(barrier::POSITIONS + ((mode + class) & 0xffff));
    new.set_u8(ctx.b, 0x42, position);
    let character = hitter.u8(ctx.b, 2);
    new.set_u8(ctx.b, 0x43, character);
    o.set_u16(ctx.b, barrier::SET_CREATED, new.0 as u16);
}

fn barrier_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let variant = o.u8(ctx.b, Obj::VARIANT);
    let target = super::effects::target(ctx, barrier::INITS, 2 * variant as u32);
    match variant {
        0 | 3 => {
            add_u8(ctx, o, Obj::STATE, 2);
            o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
            o.set_u32(ctx.b, 0x78, 0);
            ctx.anim_start(barrier::ANIMATIONS_0, 0);
        }
        1 => {
            add_u8(ctx, o, Obj::STATE, 2);
            o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
            super::eshl_creators::single_3c(&mut crate::families::effects_creators::leftovers::Pure, ctx.b, o);
            ctx.anim_start(barrier::ANIMATIONS_1, 4);
        }
        _ => crate::families::effects::outside_table(target),
    }
}

/// Variant 0, active: substate 0 waits for the hit, substate 2 waits for the end of the SET; then disappears
/// when it leaves the screen.
fn barrier_active_0<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let sub_state = o.u8(ctx.b, field::SUB_STATE);
    let target = super::effects::target(ctx, barrier::SUB_STATES_0, sub_state as u32);
    match sub_state {
        0 => {
            let hit = o.u8(ctx.b, field::HIT_B);
            if !(hit == 0) {
                add_u8(ctx, o, field::SUB_STATE, 2);
                barrier_created_set(ctx, o);
            }
            ctx.reinsert();
        }
        2 => {
            let set = pointer(ctx, o, barrier::SET_CREATED);
            let finished = set.u8(ctx.b, 0x22) != 0;
            if finished {
                o.set_u8(ctx.b, field::SUB_STATE, 0);
                o.set_u8(ctx.b, field::HIT_B, 0);
            }
        }
        _ => crate::families::effects::outside_table(target),
    }
    let (cam_x, _) = ctx.camera();
    let gap = cam_x.wrapping_sub(o.u16(ctx.b, Obj::X));
    if !((gap as i16) < 0x200) {
        add_u8(ctx, o, Obj::STATE, 2);
    }
}

fn barrier_active_1<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let sub_state = o.u8(ctx.b, field::SUB_STATE);
    let target = super::effects::target(ctx, barrier::SUB_STATES_1, sub_state as u32);
    match sub_state {
        0 | 4 => {
            let hit = o.u8(ctx.b, field::HIT_B);
            if hit == 0 {
                return ctx.reinsert();
            }
            add_u8(ctx, o, field::SUB_STATE, 2);
            let hitter = pointer(ctx, o, barrier::HITTER);
            hitter.set_u32(ctx.b, Obj::STATE, 0x0200_3000);
            let animation = if sub_state == 0 { 5 } else { 6 };
            ctx.anim_start(barrier::ANIMATIONS_1, animation);
        }
        2 | 6 => {
            let end = ctx.anim_step();
            if !end {
                return;
            }
            if sub_state == 2 {
                add_u8(ctx, o, field::SUB_STATE, 2);
            } else {
                add_u8(ctx, o, field::SUB_STATE, 0xfa);
            }
            o.set_u8(ctx.b, field::HIT_B, 0);
            crate::sound::play(ctx.b, barrier::SOUND, ctx.obj());
        }
        _ => crate::families::effects::outside_table(target),
    }
}

fn barrier_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let cache = o.u8(ctx.b, Obj::HIDDEN) != 0;
    if cache {
        return;
    }
    let state = o.u8(ctx.b, Obj::STATE);
    let target = super::effects::target(ctx, barrier::STATES, state as u32);
    match state {
        state::INIT => barrier_init(ctx, o),
        state::ACTIVE => {
            let variant = o.u8(ctx.b, Obj::VARIANT);
            let target = super::effects::target(ctx, barrier::ACTIVE_COUNT, 2 * variant as u32);
            match variant {
                0 => barrier_active_0(ctx, o),
                1 => barrier_active_1(ctx, o),
                _ => crate::families::effects::outside_table(target),
            }
        }
        state::FREED | state::OUTSIDE_SCREEN => ctx.free(&pools::BOX),
        _ => crate::families::effects::outside_table(target),
    }
}

handler!(BOX_1, barrier::ENTRY, "box_barrier", #[without] barrier_handler);
handler!(BOX_2, scenery::ENTRY, "box_scenery", #[without] box_2);

// ---------------------------------------------------------------------------------------------
// Type 3 : bloc poussable (0xda7f2)

/// Type 3: block that a player pushes (action 0x2e, registered by the player in `+0x6c`). Variant 0: slides
/// in X between two bounds, accompanied by a SET (type 7, variant 0xc); out of the bounds, it stops (substate
/// 2) and notifies its SET. Variant 1: slides in height (`+0x10`) and realigns itself with the terrain at each step.
/// The pushing player is placed back against the block (offset per character and direction).
pub mod pushes {
use crate::families::cx::Bench;
use crate::objects::Current;
    use crate::families::cx::{handlers, Cx};
    use crate::objects::{pools, Obj};

    const ANIMATIONS: u32 = 0x1bebc2;
    /// Realignment of the object after the collision with the terrain (`terrain::push_68k`).
    /// Offset of the player against the block, per character (words, plain): according to the push direction.
    const AGAINST_HIGH: u32 = 0xdab52;
    const AGAINST_LOW: u32 = 0xdab62;
    const AGAINST_LEFT_1: u32 = 0xdab72;
    const AGAINST_RIGHT_1: u32 = 0xdab82;
    const AGAINST_LEFT_0: u32 = 0xdab92;
    const AGAINST_RIGHT_0: u32 = 0xdaba2;

    mod f {
        /// Non nul : objet suspendu.
        pub const HIDDEN: u32 = 0x40;
        pub const SET: u32 = 0x5c;
        /// Bounds of variant 0 (minimum X excluded, maximum X).
        pub const X_MIN: u32 = 0x60;
        pub const X_MAX: u32 = 0x62;
        pub const STOPS: u32 = 0x64;
        pub const MOBILE: u32 = 0x65;
        pub const PUSHES: u32 = 0x66;
        /// Character of the pushing player × 2.
        pub const CHARACTER: u32 = 0x68;
        /// Pushing player (word, 0 = none).
        pub const PLAYER: u32 = 0x6c;
    }
    /// Fields of the pushing player.
    mod j {
        /// States `+4..+6` (the `+4` long masked) of the player who is pushing.
        pub const STATES: u32 = 0x4;
        pub const PUSHING: u32 = 0x0200_2e00;
        pub const CHARACTER: u32 = 0x3;
        pub const DIRECTION: u32 = 0x7;
        /// Bit 4 of `+0x6e`: step of two pixels; `+0x33` non-zero: one more pixel.
        pub const MOMENTUM: u32 = 0x6e;
        pub const FORCE: u32 = 0x33;
    }
    /// Push directions (`+7` of the player).
    mod dir {
        pub const HIGH: u8 = 2;
        pub const LOW: u8 = 4;
        pub const LEFT: u8 = 6;
        pub const RIGHT: u8 = 8;
    }

    fn run<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(f::HIDDEN) != 0 {
            return;
        }
        let state = x.u8(0x4);
        match state {
            0 => {
                let v = 2 * x.u8(0x3) as u32;
                if v == 0 { init_0(x) } else { init_1(x) }
                x.reinsert();
            }
            2 => {
                if x.a5_u8(0x3d) == 0 {
                    let v = 2 * x.u8(0x3) as u32;
                    if x.u8(0x5) == 0 {
                        if v == 0 { active_0(x) } else { active_1(x) }
                    }
                }
                x.reinsert();
            }
            _ => x.free(&pools::BOX),
        }
    }

    fn init_common<R: Bench>(x: &mut Cx<'_, R>, mobile: u8) {
        x.add8(0x4, 2);
        x.set8(0x0, 1);
        x.set8(0x2e, 0x35);
        x.set8(f::MOBILE, mobile);
    }

    fn init_0<R: Bench>(x: &mut Cx<'_, R>) {
        init_common(x, 1);
        x.set16(0x8, 0x234);
        x.set16(0x10, 0x290);
        x.set16(f::X_MAX, 0x290);
        x.set16(f::X_MIN, 0x1d8);
        x.anim_start(ANIMATIONS, 7);
        let Some(set) = x.alloc(&pools::SET) else { return };
        set.set_u8(x.b, 0x0, 1);
        set.set_u8(x.b, 0x2, 7);
        set.set_u8(x.b, 0x3, 0xc);
        set.set_u8(x.b, 0x2f, 1);
        x.set16(f::SET, set.0 as u16);
    }

    fn init_1<R: Bench>(x: &mut Cx<'_, R>) {
        init_common(x, 0);
        x.anim_start(ANIMATIONS, 6);
    }

    /// The pushing player (A1), if still pushing; otherwise the block forgets them.
    fn pusher<R: Bench>(x: &mut Cx<'_, R>) -> Option<Obj> {
        if x.u16(f::PLAYER) == 0 {
            return None;
        }
        let player = x.linked(f::PLAYER);
        let states = player.u32(x.b, j::STATES) & 0xffff_ff00;
        if states != j::PUSHING {
            x.set8(f::PUSHES, 0);
            x.set16(f::PLAYER, 0);
            return None;
        }
        let character = (player.u8(x.b, j::CHARACTER) & 0xfe) as u32;
        x.set16(f::CHARACTER, character as u16);
        Some(player)
    }

    /// Places the player back against the block on axis `axe` (+8 or +0x10).
    fn realign_player<R: Bench>(x: &mut Cx<'_, R>, axe: u32, table: u32) {
        let player = x.linked(f::PLAYER);
        let v = x.u16(axe);
        player.set_u16(x.b, axe, v);
        let i = x.u16(f::CHARACTER) as i16 as i32 as u32;
        let d = x.data16(table.wrapping_add(i));
        player.set_u16(x.b, axe, v.wrapping_add(d));
    }

    /// Variant 0: sliding in X between the bounds.
    fn active_0<R: Bench>(x: &mut Cx<'_, R>) {
        let px = x.u16(0x8);
        if px > x.u16(f::X_MAX) || px <= x.u16(f::X_MIN) {
            x.add8(0x5, 2);
            x.set8(f::STOPS, 1);
            let set = x.linked(f::SET);
            set.set_u8(x.b, 0x2f, 0);
            return;
        }
        let Some(player) = pusher(x) else { return };
        let (sign, table) = match player.u8(x.b, j::DIRECTION) {
            dir::LEFT => (-1i16, AGAINST_LEFT_0),
            dir::RIGHT => (1, AGAINST_RIGHT_0),
            _ => return,
        };
        if player.u8(x.b, j::MOMENTUM) & 0x10 != 0 {
            x.add16(0x8, (2 * sign) as u16);
        }
        if player.u8(x.b, j::FORCE) != 0 {
            x.add16(0x8, sign as u16);
        }
        realign_player(x, 0x8, table);
    }

    /// One step of variant 1: the block advances then realigns itself with the terrain.
    fn step<R: Bench>(x: &mut Cx<'_, R>, axe: u32, d: i16) {
        x.add16(axe, d as u16);
        realign_scenery(x);
    }

    fn realign_scenery<R: Bench>(x: &mut Cx<'_, R>) {
        x.push_terrain();
        // 0x35c0: keeps the crate on screen like a player
        crate::player::walk::clamp_to_screen(x.b, crate::player::Player(x.o.0));
    }

    /// Variant 1: sliding in height (directions 2 and 4) or in X (6 and 8).
    fn active_1<R: Bench>(x: &mut Cx<'_, R>) {
        let Some(player) = pusher(x) else { return };
        let direction = player.u8(x.b, j::DIRECTION);
        let (axe, sign, table) = match direction {
            dir::HIGH => (0x10, 1i16, AGAINST_HIGH),
            dir::LOW => (0x10, -1, AGAINST_LOW),
            dir::LEFT => (0x8, -1, AGAINST_LEFT_1),
            dir::RIGHT => (0x8, 1, AGAINST_RIGHT_1),
            _ => return,
        };
        let horizontal = axe == 0x8;
        let step_long = if horizontal { 2 * sign } else { sign };
        if player.u8(x.b, j::MOMENTUM) & 0x10 != 0 {
            step(x, axe, step_long);
        }
        // the player is read again after each call
        let player = x.linked(f::PLAYER);
        if player.u8(x.b, j::FORCE) != 0 {
            if horizontal {
                x.add16(axe, sign as u16);
            } else {
                step(x, axe, sign);
            }
        }
        if horizontal {
            realign_scenery(x);
        }
        realign_player(x, axe, table);
    }

    handlers! {
        0xda7f2 "box_pushes" #[without] run,
    }
}
