//! Player effects (PEFF), third series: types 2 to 5 (hold aura, hit, healing, light).
//! See `docs/families/joueur.md`.

use super::vars;
use crate::families::player::records::Shot;
use crate::player::Player;
use crate::families::cx::Bench;
use crate::families::cx::{handlers, Cx};
use super::pshell::*;
use crate::objects::Obj;
use crate::objects::pools;
use crate::objects::Current;
use crate::player::tables::{player_data, byte, Aligned, Read};

/// Type 2 (0x4a880): halo of the player who holds pose 0x18 (`+0x6a` of the player) as long as its animation runs (set
/// 0x1b540c).
mod halo {
    use super::*;

    const OFFSETS: u32 = 0x4a93a;
    const ANIMS: crate::data::RomAddr = crate::data::RomAddr(0x1b540c);

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.state(0x4a88e) {
            0 => {
                x.add8(Obj::STATE, 2);
                place(x);
                x.anim_start(ANIMS.0, 0);
                x.reinsert();
            }
            2 => {
                let owner = x.linked(f::OWNER);
                if owner.u8(x.b, Player::SUB) != 0 || owner.u8(x.b, Player::SUBACTION) != 0x18 {
                    return x.free_peff();
                }
                if vars::PAUSE.get(x.b) == 0 && owner.u8(x.b, Player::HITSTOP) == 0 && x.anim_step() {
                        x.add8(Obj::STATE, 2);
                    }
                place(x);
                x.reinsert();
            }
            _ => x.free_peff(),
        }
    }

    fn place<R: Bench>(x: &mut Cx<'_, R>) {
        let owner = follow_owner(x);
        let row = (x.u8(Shot::ANIM_EVENT) as u32) << 2 & 0xffff;
        let (dz_part, dz) = offset(x, OFFSETS, row);
        x.add16(Obj::Z, dz_part);
        x.set16(Shot::Y_FRACTION, 0);
        owner.set_u16(x.b, Player::Y_FRACTION, 0x8000);
        if dz != 0 {
            owner.set_u16(x.b, Player::Y_FRACTION, 0);
            x.set16(Shot::Y_FRACTION, 0x8000);
        }
    }
}

/// Type 3 (0x4a9fe): trail of an attack, stuck to the player for the duration of its move (`+0x5c` = the player's move,
/// `+7`). Variant 0 takes animation `+0x2f`, variant 1 animation 3.
mod slash {
    use super::*;

    /// Step of the player's move when the trail was created.
    const MOVE_STEP: u32 = 0x5c;

    const ANIMS: crate::data::RomAddr = crate::data::RomAddr(0x1b5590);
    const OFFSETS_A: u32 = 0x4aa90;
    const OFFSETS_B: u32 = 0x4ab2a;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.state(0x4aa0c) {
            0 => init(x),
            2 => active(x),
            _ => x.free_peff(),
        }
    }

    fn init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(Obj::STATE, 2);
        let owner = x.linked(f::OWNER);
        if x.u8(Obj::VARIANT) == 0 {
            let n = (x.u8(Shot::SETTING) as u32) as u16;
            x.anim_start(ANIMS.0, n);
            return;
        }
        follow(x, owner, OFFSETS_A);
        x.anim_start(ANIMS.0, 3);
    }

    /// Places the trail in front of the player (offset according to its variant, inverted according to its facing).
    fn follow<R: Bench>(x: &mut Cx<'_, R>, owner: Obj, table: u32) {
        let flip = owner.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        x.copy_pos(owner);
        x.set16(Shot::Y_FRACTION, 0);
        owner.set_u16(x.b, Player::Y_FRACTION, 0x8000);
        let row = ((owner.u8(x.b, Player::CHARACTER) >> 1) as u32) << 2;
        let (mut dx, dz) = offset(x, table, row);
        if flip == 0 {
            dx = dx.wrapping_neg();
        }
        x.add16(Obj::X, dx);
        x.add16(Obj::Z, dz);
    }

    fn active<R: Bench>(x: &mut Cx<'_, R>) {
        let owner = x.linked(f::OWNER);
        if owner.u8(x.b, Player::SUB) != 0 || x.u8(MOVE_STEP) != owner.u8(x.b, Player::STEP) {
            return x.free_peff();
        }
        if x.u8(Obj::VARIANT) == 0 {
            follow(x, owner, OFFSETS_B);
        }
        if vars::PAUSE.get(x.b) == 0 && owner.u8(x.b, Player::HITSTOP) == 0 && x.anim_step() {
                x.add8(Obj::STATE, 2);
            }
        x.reinsert();
    }
}

/// Type 4 (0x4abaa): healing (variant 0: stars around the healer, 1: rising spark, 2: potion). See `player/spawn.rs`
/// (`cure`, `potion`).
mod cure {
    use super::*;

    /// Countdown of the current phase.
    const TIMER: u32 = 0x5c;
    /// Step of the palette cycle (0..7).
    const PALETTE_STEP: u32 = 0x5f;

    const ANIMS: crate::data::RomAddr = crate::data::RomAddr(0x1b56da);
    const OFFSETS_PRAYER: u32 = 0x4acb2;
    const OFFSETS_PLAYER: u32 = 0x4ad38;
    const OFFSETS_SPARK: u32 = 0x4ade0;
    const SFX_PRAYER: crate::sound::Sound = crate::sound::CMD_174;
    const SFX_PRAYER_B: crate::sound::Sound = crate::sound::CMD_175;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.state(0x4abb8) {
            0 => match x.u8(Obj::VARIANT) {
                0 => init_prayer(x),
                1 => init_spark(x),
                _ => init_potion(x),
            },
            2 => match x.u8(Obj::VARIANT) {
                1 => spark(x),
                _ => hold(x),
            },
            _ => x.free_peff(),
        }
    }

    fn init_prayer<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(Obj::STATE, 2);
        let owner = follow_owner(x);
        let (mut dx, mut dz) = (4i16, 0x65i16);
        if owner.u8(x.b, Player::SPELL_AREA) != 0 {
            (dx, dz) = (-0x36, 0x4e);
        }
        if x.u8(f::FLIP) != 0 {
            dx = -dx;
        }
        x.add16(Obj::X, dx as u16);
        x.add16(Obj::Z, dz as u16);
        x.add16(Obj::Y, 0xffff);
        x.set16(Shot::Y_FRACTION, 0x8000);
        if x.u8(Shot::SETTING) == 0 {
            crate::sound::play(x.b, SFX_PRAYER, x.o);
        } else {
            crate::sound::play(x.b, SFX_PRAYER_B, x.o);
        }
        let n = (x.u8(Shot::SETTING) as u32) as u16;
        x.anim_start(ANIMS.0, n);
    }

    fn init_spark<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(Obj::STATE, 2);
        let owner = x.linked(f::OWNER);
        x.copy_pos(owner);
        x.set16(Shot::Y_FRACTION, 0);
        let row = (owner.u8(x.b, Player::PRAYER_STEP) as u32) << 2 & 0xffff;
        let (dx, dz) = offset(x, OFFSETS_SPARK, row);
        x.add16(Obj::X, dx);
        x.add16(Obj::Z, dz);
        let r = (x.rand_and(7)) * 2;
        let vz = player_data().cure_spark_vz.word_at(x.b, r);
        x.set16(Obj::VZ, vz);
        x.set16(Obj::AZ, 0);
        let r = x.rand_and(7);
        let t = player_data().cure_spark_times.at8(x.b, r);
        x.set8(TIMER, t);
        let r = x.rand_and(1);
        x.set8(Shot::SETTING, r as u8);
        x.anim_start(ANIMS.0, (r + 2) as u16);
    }

    /// Potion: the effect is placed on the player who drinks (the healer itself, or the first active player).
    fn init_potion<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(Obj::STATE, 2);
        let owner = x.linked(f::OWNER);
        let source = if owner.u8(x.b, Player::SPELL_AREA) == 0 {
            let row = ((owner.u8(x.b, Player::CHARACTER) >> 1) as u32) << 2;
            let (dx, dz) = offset(x, OFFSETS_PRAYER, row);
            Some((owner, dx, dz))
        } else {
            let mut found = None;
            let mut p = Obj(0xff85de);
            for _ in 0..4 {
                if p.u8(x.b, Player::ALLY_MARK) != 0 {
                    let row = ((p.u8(x.b, Player::CHARACTER) >> 1) as u32) << 2;
                    let (dx, dz) = offset(x, OFFSETS_PLAYER, row);
                    found = Some((p, dx, dz));
                    break;
                }
                p = Obj(p.0 + 0x200);
            }
            found
        };
        if let Some((p, mut dx, dz)) = source {
            if p.u8(x.b, f::FLIP) != 0 {
                dx = dx.wrapping_neg();
            }
            let flip = p.u8(x.b, f::FLIP);
            x.set8(f::FLIP, flip);
            x.copy_pos(p);
            x.add16(Obj::X, dx);
            x.add16(Obj::Z, dz);
            x.add16(Obj::Y, 0xffff);
            x.set16(Shot::Y_FRACTION, 0x8000);
        }
        x.anim_start(ANIMS.0, 0);
        crate::sound::play(x.b, SFX_PRAYER, x.o);
    }

    /// Variants 0 and 2: the star spins for one frame then disappears; every 4 ticks of the healing duration (`+0x168`
    /// of the player) it creates a spark.
    fn hold<R: Bench>(x: &mut Cx<'_, R>) {
        x.anim_step();
        if vars::PAUSE.get(x.b) == 0 {
            x.add8(Obj::STATE, 2);
        }
        let owner = x.linked(f::OWNER);
        let timer = owner.u16(x.b, Player::ACTION_TIMER);
        if timer > 0x1e && timer & 3 == 0 {
            spawn_spark(x);
        }
        x.reinsert();
    }

    /// `0x4ab82`: spark (variant 1); counter `+0x5f` cycles over 8.
    fn spawn_spark<R: Bench>(x: &mut Cx<'_, R>) {
        if let Some(o) = x.alloc(&pools::PEFF) {
            o.set_u8(x.b, Obj::ACTIVE, 1);
            o.set_u8(x.b, Obj::KIND, 4);
            o.set_u8(x.b, Obj::VARIANT, 1);
            o.set_u16(x.b, f::OWNER, x.o.0 as u16);
            let v = x.u8(PALETTE_STEP).wrapping_add(1) & 7;
            x.set8(PALETTE_STEP, v);
        }
    }

    fn spark<R: Bench>(x: &mut Cx<'_, R>) {
        x.move_z();
        let end = x.anim_step();
        if x.u8(Shot::SETTING) == 0 {
            if end {
                x.add8(Obj::STATE, 2);
            }
        } else {
            x.add8(TIMER, 0xff);
            if x.u8(TIMER) == 0 {
                x.add8(Obj::STATE, 2);
            }
        }
        x.reinsert();
    }
}

/// Type 5 (0x4af14): holy light (variant 0: column that blinks then bursts, creating two halos of variant 1, with
/// screen shakes according to a ROM script; variant 1: halo placed on the ground, +2f = 0 in front, 1 behind).
mod holy {
    use super::*;

    /// Player action when the effect started.
    const ACTION: u32 = 0x56;
    /// Frame counter (word).
    const TIMER: u32 = 0x58;
    /// Current script command.
    const COMMAND: u32 = 0x5a;
    /// Countdown of the current command.
    const DURATION: u32 = 0x5b;
    /// Script of the shakes (long: address).
    const SCRIPT: u32 = 0x5c;

    const ANIMS: crate::data::RomAddr = crate::data::RomAddr(0x1b598c);
    const SFX_START: crate::sound::Sound = crate::sound::CMD_192;
    const SFX_BURST_A: crate::sound::Sound = crate::sound::CMD_193;
    const SFX_BURST_B: crate::sound::Sound = crate::sound::CMD_188;
    const SFX_END: crate::sound::Sound = crate::sound::CMD_194;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        let child = x.u8(Obj::VARIANT) != 0;
        match x.state(0x4af22) {
            0 if !child => init(x),
            0 => init_halo(x),
            2 if !child => active(x),
            2 => {
                if x.anim_step() {
                    x.add8(Obj::STATE, 2);
                }
                x.reinsert();
            }
            _ => x.free_peff(),
        }
    }

    fn init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(Obj::STATE, 2);
        x.set16(TIMER, 0);
        let owner = x.linked(f::OWNER);
        let flip = owner.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        x.copy_pos(owner);
        let mut dx = -2i16;
        if x.u8(f::FLIP) != 0 {
            dx = 2;
        }
        x.add16(Obj::X, dx as u16);
        x.add16(Obj::Z, 0x7a);
        x.add16(Obj::Y, 0xffff);
        x.anim_start(ANIMS.0, 0);
        crate::sound::play(x.b, SFX_START, x.o);
    }

    fn init_halo<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(Obj::STATE, 2);
        let owner = x.linked(f::OWNER);
        x.copy_pos(owner);
        let dy: u16 = if x.u8(Shot::SETTING) != 0 { 1 } else { 0xffff };
        *x.c.d2() = dy as u32;
        x.add16(Obj::Y, dy);
        let anim = x.u8(Shot::SETTING) as u32 + 1;
        x.anim_start(ANIMS.0, anim as u16);
    }

    fn active<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(Shot::SUB_STATE) {
            0 => rise(x),
            2 => fall(x),
            _ => script(x),
        }
        x.reinsert();
    }

    /// Column that blinks more and more slowly, then rises when the player changes move.
    fn rise<R: Bench>(x: &mut Cx<'_, R>) {
        x.add16(TIMER, 1);
        x.set_bit(Obj::FLAGS, 1, false);
        let t = x.u16(TIMER);
        if t > 0x40 {
            x.anim_step();
            let owner = x.linked(f::OWNER);
            let action = owner.u8(x.b, Player::STEP);
            if action != x.u8(ACTION) {
                x.add8(Shot::SUB_STATE, 2);
                x.set16(Obj::VZ, -0x400i16 as u16);
                x.set16(Obj::AZ, -0x80i16 as u16);
            }
            return;
        }
        let mut mask = 1u16;
        if t <= 0x30 {
            mask = 3;
            if t <= 0x20 {
                mask = 7;
            }
        }
        if t & mask != 0 {
            x.set_bit(Obj::FLAGS, 1, true);
        }
    }

    /// Fall of the column to the ground, then bursting.
    fn fall<R: Bench>(x: &mut Cx<'_, R>) {
        x.anim_step();
        x.move_z();
        if x.u16(Obj::Z) > 8 {
            return;
        }
        x.add8(Shot::SUB_STATE, 2);
        x.set16(TIMER, 0x50);
        x.set16(Obj::Z, 8);
        x.anim_start(ANIMS.0, 0);
        crate::families::small::peff_5_pair(&mut crate::families::effects_creators::leftovers::Pure, x.b, x.o);
        crate::sound::play(x.b, SFX_BURST_A, x.o);
        crate::sound::play(x.b, SFX_BURST_B, x.o);
        let owner = x.linked(f::OWNER);
        let player = owner.u8(x.b, Player::LEVEL) as u32;
        let row = player_data().holy_script_by_player.at8(x.b, player) as u32;
        let script = player_data().holy_scripts.long_at(x.b, row);
        x.set32(SCRIPT, script);
        x.set16(TIMER, 8);
        x.set8(DURATION, 0);
    }

    /// Screen shake script: (command, duration) pairs read in sequence in the ROM.
    fn script<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(DURATION) == 0 {
            let mut at = x.u32(SCRIPT);
            let command = byte(x.b, at);
            let duration = byte(x.b, at + 1);
            at += 2;
            x.set8(COMMAND, command);
            x.set8(DURATION, duration);
            x.set32(SCRIPT, at);
            if command == 0xff {
                x.add8(Obj::STATE, 2);
                let owner = x.linked(f::OWNER);
                owner.set_u16(x.b, Player::ACTION_TIMER, 1);
                crate::sound::play(x.b, SFX_END, x.o);
            } else {
                // the table designates the shake by its routine (0x2824, 0x2860, 0x289c)
                let mode = crate::camera_modes::shake::of_routine(player_data().holy_shake_table.long_at(x.b, command as u32));
                crate::camera_modes::start_shake(x.b, mode, duration as u16);
            }
        }
        x.add8(DURATION, 0xff);
        x.add16(TIMER, 0xffff);
        x.set_bit(Obj::FLAGS, 1, true);
        let t = x.u16(TIMER);
        if (t as i16) >= 0 && t & 1 != 0 {
            x.set_bit(Obj::FLAGS, 1, false);
        }
    }
}

handlers! {
    0x4a880 "peff_2" halo::run,
    0x4a9fe "peff_3" slash::run,
    0x4abaa "peff_4" cure::run,
    0x4af14 "peff_5" holy::run,
}
