//! Player effects (PEFF), third series: types 2 to 5 (hold aura, hit, healing, light).
//! See `docs/families/joueur.md`.

use crate::families::cx::Bench;
use crate::families::cx::{handlers, Cx};
use super::pshell::*;
use crate::objects::Obj;
use crate::objects::pools;
use crate::objects::Current;

/// Type 2 (0x4a880): halo of the player who holds pose 0x18 (`+0x6a` of the player) as long as its animation runs (set
/// 0x1b540c).
mod halo {
    use super::*;

    const OFFSETS: u32 = 0x4a93a;
    const ANIMS: u32 = 0x1b540c;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.state(0x4a88e) {
            0 => {
                x.add8(0x4, 2);
                place(x);
                x.anim_start(ANIMS, 0);
                x.reinsert();
            }
            2 => {
                let owner = x.linked(f::OWNER);
                if owner.u8(x.b, 0x5) != 0 || owner.u8(x.b, 0x6a) != 0x18 {
                    return x.free_peff();
                }
                if x.a5_u8(0x3d) == 0 && owner.u8(x.b, 0x53) == 0 {
                    if x.anim_step() {
                        x.add8(0x4, 2);
                    }
                }
                place(x);
                x.reinsert();
            }
            _ => x.free_peff(),
        }
    }

    fn place<R: Bench>(x: &mut Cx<'_, R>) {
        let owner = follow_owner(x);
        let row = (x.u8(0x33) as u32) << 2 & 0xffff;
        let (dz_part, dz) = offset(x, OFFSETS, row);
        x.add16(0xc, dz_part);
        x.set16(0x12, 0);
        owner.set_u16(x.b, 0x12, 0x8000);
        if dz != 0 {
            owner.set_u16(x.b, 0x12, 0);
            x.set16(0x12, 0x8000);
        }
    }
}

/// Type 3 (0x4a9fe): trail of an attack, stuck to the player for the duration of its move (`+0x5c` = the player's move,
/// `+7`). Variant 0 takes animation `+0x2f`, variant 1 animation 3.
mod slash {
    use super::*;

    const ANIMS: u32 = 0x1b5590;
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
        x.add8(0x4, 2);
        let owner = x.linked(f::OWNER);
        if x.u8(0x3) == 0 {
            let n = (x.u8(0x2f) as u32) as u16;
            x.anim_start(ANIMS, n);
            return;
        }
        follow(x, owner, OFFSETS_A);
        x.anim_start(ANIMS, 3);
    }

    /// Places the trail in front of the player (offset according to its variant, inverted according to its facing).
    fn follow<R: Bench>(x: &mut Cx<'_, R>, owner: Obj, table: u32) {
        let flip = owner.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        x.copy_pos(owner);
        x.set16(0x12, 0);
        owner.set_u16(x.b, 0x12, 0x8000);
        let row = ((owner.u8(x.b, 0x3) >> 1) as u32) << 2;
        let (mut dx, dz) = offset(x, table, row);
        if flip == 0 {
            dx = dx.wrapping_neg();
        }
        x.add16(0x8, dx);
        x.add16(0xc, dz);
    }

    fn active<R: Bench>(x: &mut Cx<'_, R>) {
        let owner = x.linked(f::OWNER);
        if owner.u8(x.b, 0x5) != 0 || x.u8(0x5c) != owner.u8(x.b, 0x7) {
            return x.free_peff();
        }
        if x.u8(0x3) == 0 {
            follow(x, owner, OFFSETS_B);
        }
        if x.a5_u8(0x3d) == 0 && owner.u8(x.b, 0x53) == 0 {
            if x.anim_step() {
                x.add8(0x4, 2);
            }
        }
        x.reinsert();
    }
}

/// Type 4 (0x4abaa): healing (variant 0: stars around the healer, 1: rising spark, 2: potion). See `player/spawn.rs`
/// (`cure`, `potion`).
mod cure {
    use super::*;

    const ANIMS: u32 = 0x1b56da;
    const OFFSETS_PRAYER: u32 = 0x4acb2;
    const OFFSETS_PLAYER: u32 = 0x4ad38;
    const OFFSETS_SPARK: u32 = 0x4ade0;
    const SPARK_VZ: u32 = 0x4adc8;
    const SPARK_TIMES: u32 = 0x4add8;
    const SFX_PRAYER: u32 = 0xe8f5c;
    const SFX_PRAYER_B: u32 = 0xe8fc6;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.state(0x4abb8) {
            0 => match x.u8(0x3) {
                0 => init_prayer(x),
                1 => init_spark(x),
                _ => init_potion(x),
            },
            2 => match x.u8(0x3) {
                1 => spark(x),
                _ => hold(x),
            },
            _ => x.free_peff(),
        }
    }

    fn init_prayer<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        let owner = follow_owner(x);
        let (mut dx, mut dz) = (4i16, 0x65i16);
        if owner.u8(x.b, 0x1ee) != 0 {
            (dx, dz) = (-0x36, 0x4e);
        }
        if x.u8(f::FLIP) != 0 {
            dx = -dx;
        }
        x.add16(0x8, dx as u16);
        x.add16(0xc, dz as u16);
        x.add16(0x10, 0xffff);
        x.set16(0x12, 0x8000);
        if x.u8(0x2f) == 0 {
            crate::sound::play(x.b, SFX_PRAYER, x.o);
        } else {
            crate::sound::play(x.b, SFX_PRAYER_B, x.o);
        }
        let n = (x.u8(0x2f) as u32) as u16;
        x.anim_start(ANIMS, n);
    }

    fn init_spark<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        let owner = x.linked(f::OWNER);
        x.copy_pos(owner);
        x.set16(0x12, 0);
        let row = (owner.u8(x.b, 0x5f) as u32) << 2 & 0xffff;
        let (dx, dz) = offset(x, OFFSETS_SPARK, row);
        x.add16(0x8, dx);
        x.add16(0xc, dz);
        let r = (x.rand_and(7)) * 2;
        let vz = x.rom16(SPARK_VZ + r);
        x.set16(0x18, vz);
        x.set16(0x1a, 0);
        let r = x.rand_and(7);
        let t = x.rom8(SPARK_TIMES + r);
        x.set8(0x5c, t);
        let r = x.rand_and(1);
        x.set8(0x2f, r as u8);
        x.anim_start(ANIMS, (r + 2) as u16);
    }

    /// Potion: the effect is placed on the player who drinks (the healer itself, or the first active player).
    fn init_potion<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        let owner = x.linked(f::OWNER);
        let source = if owner.u8(x.b, 0x1ee) == 0 {
            let row = ((owner.u8(x.b, 0x3) >> 1) as u32) << 2;
            let (dx, dz) = offset(x, OFFSETS_PRAYER, row);
            Some((owner, dx, dz))
        } else {
            let mut found = None;
            let mut p = Obj(0xff85de);
            for _ in 0..4 {
                if p.u8(x.b, 0x78) != 0 {
                    let row = ((p.u8(x.b, 0x3) >> 1) as u32) << 2;
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
            x.add16(0x8, dx);
            x.add16(0xc, dz);
            x.add16(0x10, 0xffff);
            x.set16(0x12, 0x8000);
        }
        x.anim_start(ANIMS, 0);
        crate::sound::play(x.b, SFX_PRAYER, x.o);
    }

    /// Variants 0 and 2: the star spins for one frame then disappears; every 4 ticks of the healing duration (`+0x168`
    /// of the player) it creates a spark.
    fn hold<R: Bench>(x: &mut Cx<'_, R>) {
        x.anim_step();
        if x.a5_u8(0x3d) == 0 {
            x.add8(0x4, 2);
        }
        let owner = x.linked(f::OWNER);
        let timer = owner.u16(x.b, 0x168);
        if timer > 0x1e && timer & 3 == 0 {
            spawn_spark(x);
        }
        x.reinsert();
    }

    /// `0x4ab82`: spark (variant 1); counter `+0x5f` cycles over 8.
    fn spawn_spark<R: Bench>(x: &mut Cx<'_, R>) {
        if let Some(o) = x.alloc(&pools::PEFF) {
            o.set_u8(x.b, 0, 1);
            o.set_u8(x.b, 2, 4);
            o.set_u8(x.b, 3, 1);
            o.set_u16(x.b, f::OWNER, x.o.0 as u16);
            let v = x.u8(0x5f).wrapping_add(1) & 7;
            x.set8(0x5f, v);
        }
    }

    fn spark<R: Bench>(x: &mut Cx<'_, R>) {
        x.move_z();
        let end = x.anim_step();
        if x.u8(0x2f) == 0 {
            if end {
                x.add8(0x4, 2);
            }
        } else {
            x.add8(0x5c, 0xff);
            if x.u8(0x5c) == 0 {
                x.add8(0x4, 2);
            }
        }
        x.reinsert();
    }
}

/// Type 5 (0x4af14): holy light (variant 0: column that blinks then bursts, creating two halos of variant 1, with
/// screen shakes according to a ROM script; variant 1: halo placed on the ground, +2f = 0 in front, 1 behind).
mod holy {
    use super::*;

    const ANIMS: u32 = 0x1b598c;
    const SHAKE_TABLE: u32 = 0x4b172;
    const SCRIPT_BY_PLAYER: u32 = 0x4b0b2;
    const SCRIPTS: u32 = 0x4b0da;
    const SFX_START: u32 = 0xe9f5e;
    const SFX_BURST_A: u32 = 0xe9fc8;
    const SFX_BURST_B: u32 = 0xe996c;
    const SFX_END: u32 = 0xea032;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        let child = x.u8(0x3) != 0;
        match x.state(0x4af22) {
            0 if !child => init(x),
            0 => init_halo(x),
            2 if !child => active(x),
            2 => {
                if x.anim_step() {
                    x.add8(0x4, 2);
                }
                x.reinsert();
            }
            _ => x.free_peff(),
        }
    }

    fn init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        x.set16(0x58, 0);
        let owner = x.linked(f::OWNER);
        let flip = owner.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        x.copy_pos(owner);
        let mut dx = -2i16;
        if x.u8(f::FLIP) != 0 {
            dx = 2;
        }
        x.add16(0x8, dx as u16);
        x.add16(0xc, 0x7a);
        x.add16(0x10, 0xffff);
        x.anim_start(ANIMS, 0);
        crate::sound::play(x.b, SFX_START, x.o);
    }

    fn init_halo<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        let owner = x.linked(f::OWNER);
        x.copy_pos(owner);
        let dy: u16 = if x.u8(0x2f) != 0 { 1 } else { 0xffff };
        *x.c.d2() = dy as u32;
        x.add16(0x10, dy);
        let anim = x.u8(0x2f) as u32 + 1;
        x.anim_start(ANIMS, anim as u16);
    }

    fn active<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(0x5) {
            0 => rise(x),
            2 => fall(x),
            _ => script(x),
        }
        x.reinsert();
    }

    /// Column that blinks more and more slowly, then rises when the player changes move.
    fn rise<R: Bench>(x: &mut Cx<'_, R>) {
        x.add16(0x58, 1);
        x.set_bit(0x1, 1, false);
        let t = x.u16(0x58);
        if t > 0x40 {
            x.anim_step();
            let owner = x.linked(f::OWNER);
            let action = owner.u8(x.b, 0x7);
            if action != x.u8(0x56) {
                x.add8(0x5, 2);
                x.set16(0x18, -0x400i16 as u16);
                x.set16(0x1a, -0x80i16 as u16);
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
            x.set_bit(0x1, 1, true);
        }
    }

    /// Fall of the column to the ground, then bursting.
    fn fall<R: Bench>(x: &mut Cx<'_, R>) {
        x.anim_step();
        x.move_z();
        if x.u16(0xc) > 8 {
            return;
        }
        x.add8(0x5, 2);
        x.set16(0x58, 0x50);
        x.set16(0xc, 8);
        x.anim_start(ANIMS, 0);
        crate::families::small::peff_5_pair(&mut crate::families::effects_creators::leftovers::Pure, x.b, x.o);
        crate::sound::play(x.b, SFX_BURST_A, x.o);
        crate::sound::play(x.b, SFX_BURST_B, x.o);
        let owner = x.linked(f::OWNER);
        let player = owner.u8(x.b, 0x10c) as u32;
        let row = x.rom8(SCRIPT_BY_PLAYER + player) as u32;
        let script = x.data32(SCRIPTS + row);
        x.set32(0x5c, script);
        x.set16(0x58, 8);
        x.set8(0x5b, 0);
    }

    /// Screen shake script: (command, duration) pairs read in sequence in the ROM.
    fn script<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(0x5b) == 0 {
            let mut at = x.u32(0x5c);
            let command = x.data8(at);
            let duration = x.data8(at + 1);
            at += 2;
            x.set8(0x5a, command);
            x.set8(0x5b, duration);
            x.set32(0x5c, at);
            if command == 0xff {
                x.add8(0x4, 2);
                let owner = x.linked(f::OWNER);
                owner.set_u16(x.b, 0x168, 1);
                crate::sound::play(x.b, SFX_END, x.o);
            } else {
                // the table designates the shake by its routine (0x2824, 0x2860, 0x289c)
                let mode = crate::camera_modes::shake::of_routine(x.rom32(SHAKE_TABLE + command as u32));
                crate::camera_modes::start_shake(x.b, mode, duration as u16);
            }
        }
        x.add8(0x5b, 0xff);
        x.add16(0x58, 0xffff);
        x.set_bit(0x1, 1, true);
        let t = x.u16(0x58);
        if (t as i16) >= 0 && t & 1 != 0 {
            x.set_bit(0x1, 1, false);
        }
    }
}

handlers! {
    0x4a880 "peff_2" halo::run,
    0x4a9fe "peff_3" slash::run,
    0x4abaa "peff_4" cure::run,
    0x4af14 "peff_5" holy::run,
}
