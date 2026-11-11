use crate::actors::families::cx::Bench;
use crate::world::objects::Current;
use crate::actors::families::cx::{handlers, Cx};
use crate::world::objects::{pools, Obj};
use crate::actors::families::shell::tables::{held16, tables, Read};
use crate::actors::families::shell::vars;
use crate::actors::families::effects_creators;
use crate::world::machine::Machine;

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
    pub const ANIMATIONS: crate::world::data::RomAddr = crate::world::data::RomAddr(0x1bebc2);
    // subroutines not rewritten, or native
    pub const SOUND_HIT: crate::services::sound::Sound = crate::services::sound::CMD_139_239;
    pub const SOUND_BROKEN: crate::services::sound::Sound = crate::services::sound::CMD_146_246;
    pub const SOUND_LANDS: crate::services::sound::Sound = crate::services::sound::CMD_13F_23F;
    pub const SOUND_SET_DOWN: crate::services::sound::Sound = crate::services::sound::CMD_149_249;
}

/// Crates broken in the scene (byte, `A5-0x5b21`).
/// Players' keys (byte, `A5+0x45`).
const SHADOWS: u8 = 0x35;
const DURATION_BLINK: u8 = 0x1e;

fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::HIDDEN) != 0 {
        return;
    }
    let state = x.u8(f::STATE);
    match state {
        0 => init(x),
        2 => active(x),
        4 | 6 => x.free(&pools::BOX),
        _ => crate::actors::families::effects::bad_index(state),
    }
}

/// Class of the linked player (`+3 >> 1`).
fn class<R: Bench>(x: &mut Cx<'_, R>) -> u8 {
    let player = x.linked(f::PLAYER);
    player.u8(x.b, j::CLASS) >> 1
}

fn count_broken<R: Bench>(x: &mut Cx<'_, R>) {
    let n = vars::BROKEN.get(x.b).wrapping_add(1);
    vars::BROKEN.set(x.b, n);
}

/// `0xda082`: copies the kind's sheet (10 bytes) and draws two random loots.
fn charge_sheet<R: Bench>(x: &mut Cx<'_, R>) {
    let variant = x.u8(f::VARIANT) as u32;
    let sheet = crate::world::tables::boxes().get("fiches").at32(x.b, 4 * variant);
    let fields = [f::KIND, f::LOCK, f::LOOT_HIGH, f::LOOT_BONUS, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68];
    for (i, off) in fields.into_iter().enumerate() {
        let v = crate::world::tables::boxes().byte(x.b, sheet + i as u32);
        x.set8(off, v);
    }
    let draw = (x.random() & 0xf) as u32;
    let v = crate::world::tables::boxes().byte(x.b, sheet + 0xa + draw);
    x.set8(f::LOOT_LEFT, v);
    let draw = (x.random() & 0xf) as u32;
    let v = crate::world::tables::boxes().byte(x.b, sheet + 0x1a + draw);
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
    x.anim(rom::ANIMATIONS.0, kind);
}

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    let sub_state = x.u8(f::SUB_STATE);
    match sub_state {
        sub::INTACT => intact(x),
        sub::OPEN => {
            let n = x.u8(f::SUB_SUB_STATE);
            match n {
                0 => opening(x),
                2 => open_wait(x),
                4 => blinks(x),
                _ => crate::actors::families::effects::bad_index(n),
            }
        }
        sub::RANGE => {
            x.set8(f::BOUNCE, 0);
            follow_carrier(x);
        }
        sub::LAUNCHED => launched(x),
        sub::DROPPED => {
            let n = x.u8(f::SUB_SUB_STATE);
            match n {
                0 => {
                    x.add8(f::SUB_SUB_STATE, 2);
                    x.set16(f::VZ, 0xff00);
                    x.set16(f::AZ, 0xffe0);
                }
                2 => dropped_fall(x),
                4 => dropped_broken(x),
                6 => blinks(x),
                _ => crate::actors::families::effects::bad_index(n),
            }
        }
        sub::STRUCK => {
            let n = x.u8(f::SUB_SUB_STATE);
            match n {
                0 => broken(x),
                2 => blinks(x),
                4 => tremble(x),
                _ => crate::actors::families::effects::bad_index(n),
            }
        }
        _ => crate::actors::families::effects::bad_index(sub_state),
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
        crate::services::sound::play(x.b, rom::SOUND_BROKEN, x.o);
    }
}

/// `0xda04e`: plays the crate's special effect, only once (`moment` in D1).
fn effect_special<R: Bench>(x: &mut Cx<'_, R>, moment: u32) {
    if x.u8(f::SPECIAL_PLAYED) != 0 {
        return;
    }
    x.set8(f::SPECIAL_PLAYED, 1);
    let special = x.u8(f::SPECIAL);
    // ESHL creators of the original table of routines 0xda06a (0xa1360, 0xa1a40, 0xa1670, 0xa17d0, 0xa1c6a, 0xa1360)
    use crate::actors::families::eshl_creators as eshl;
    let r = &mut crate::actors::families::effects_creators::leftovers::Pure;
    match special {
        0 | 5 => eshl::single_18(r, x.b, x.o),
        1 => {
            eshl::single_1b(r, x.b, x.o, moment as u16);
        }
        2 => {
            eshl::row_19(r, x.b, x.o);
        }
        // Original glitch: the D5 left by what precedes (`Leftovers`) sets the high byte of the spark count.
        3 => {
            let d5 = *x.c.d5() as u16;
            eshl::row_1a(r, x.b, x.o, moment as u16, d5);
        }
        4 => eshl::four_1c(r, x.b, x.o),
        _ => crate::actors::families::effects::bad_index(special),
    }
}

/// Creates the ITEM `loot` (type and variant read from `LOOTS`) above the crate, thrown with
/// speeds `(vx, vy)`. `None` if the ITEM pool is full.
fn created_loot<R: Bench>(x: &mut Cx<'_, R>, loot: u8, vx: u16, vy: u16) -> Option<Obj> {
    let n = x.alloc(&pools::ITEM)?;
    let rank = 2 * loot as u32;
    n.set_u8(x.b, f::ACTIVE, 1);
    let kind = crate::world::tables::boxes().get("loot").at8(x.b, rank);
    n.set_u8(x.b, Obj::KIND, kind);
    let variant = crate::world::tables::boxes().get("loot").at8(x.b, rank + 1);
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
        crate::services::sound::play(x.b, rom::SOUND_HIT, x.o);
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
/// loot on the first hit.
fn opening<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::LOCK) != 0 && class(x) != 5 {
        let keys = vars::KEYS.get(x.b);
        if keys == 0 {
            x.set32(f::STATE, 0x0200_0000);
            x.set8(f::ACTIVE, 1);
            spark_placed(x, 6);
            return;
        }
        vars::KEYS.set(x.b, keys - 1);
        x.set8(f::LOCK, 0);
    }
    let special = x.u8(f::SPECIAL);
    if special != 0 {
        if x.u8(f::EFFECT_CREATED) == 0 && spark_placed(x, special).is_some() {
                x.set8(f::EFFECT_CREATED, 1);
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
        crate::services::sound::play(x.b, rom::SOUND_HIT, x.o);
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
    let taken = tables().chest_taken.at32(x.b, 4 * class);
    let pose = player.u8(x.b, j::POSE);
    if pose == 0 {
        return;
    }
    let i = (4 * (((4 * class) & 0xff00) | pose as u32)) & 0xffff;
    let mut dx = held16(x.b, taken + i);
    let dz = held16(x.b, taken + i + 2);
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
    match n {
        0 => launched_start(x),
        2 => launched_flight(x),
        4 => launched_lands(x),
        6 => blinks(x),
        _ => crate::actors::families::effects::bad_index(n),
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
    let t = crate::world::terrain::terrain_of(x.b, x.o);
    // Original glitch: 0x24a8 leaves its work registers as leftovers (D2-D4)
    crate::actors::families::player::pshell::leftovers_terrain(x, &t);
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
        crate::services::sound::play(x.b, rom::SOUND_LANDS, x.o);
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
        crate::services::sound::play(x.b, rom::SOUND_SET_DOWN, x.o);
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
    let cam_x = crate::actors::families::shell::camera(x.b).0.get(x.b);
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
    crate::services::sound::play(x.b, rom::SOUND_SET_DOWN, x.o);
}

/// Dropped, 4: on the first hit, loot and special effect; end of animation: blinking.
fn dropped_broken<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::HIT) != 0 {
        x.set8(f::HIT, 0);
        dropped_loot(x);
        crate::services::sound::play(x.b, rom::SOUND_HIT, x.o);
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
    BOX_CRATE 0xd99fa "box_crate" run,
}
