//! Player effects (PEFF), second series: types 11, 20, 33. See `docs/families/joueur.md`.

use super::vars;
use crate::actors::families::player::records::Shot;
use crate::world::objects::player::Player;
use crate::world::objects::Obj;
use crate::actors::families::cx::Bench;
use crate::actors::families::cx::{handlers, Cx};
use super::pshell::*;
use crate::world::objects::pools;
use crate::world::objects::Current;
use crate::actors::player::tables::{player_data, word, Aligned};

/// Type 11 (0x4b93c): hailstorm. The parent (variant 0) counts 0x70 frames while creating, every 8 frames, two
/// hailstones (variant 1, directions 0 and 1); each hailstone circles around its starting position following a speed
/// table (`+0x4c`, chosen by `+0x58`) before falling back down.
mod hail {
    use super::*;

    /// Set when the hailstone lands; read by its parent (h).
    const LANDED: u32 = 0x5a;

    const ANIMS: crate::world::data::RomAddr = crate::world::data::RomAddr(0x1b609a);
    const KIND: u8 = 0xb;
    /// Wait duration before the fall: `+0x58` (byte), table turn: `+0x59`.
    const TIMER: u32 = 0x58;
    const PHASE: u32 = 0x59;
    const PARENT: u32 = 0x5c;
    const ANGLE: u32 = 0x2d;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        let child = x.u8(Obj::VARIANT) != 0;
        match x.u8(Obj::STATE) {
            0 if !child => {
                x.add8(Obj::STATE, 2);
                x.set8(TIMER, 0x70);
                x.set8(PHASE, 1);
                parent_tick(x);
            }
            0 => child_init(x),
            2 if !child => parent_tick(x),
            2 => child_active(x),
            4 if !child => x.add8(Obj::STATE, 2),
            4 => child_fall(x),
            _ => x.free_peff(),
        }
    }

    /// One frame of the parent: after 0x70 frames it stops creating hailstones (`+0x5a`).
    fn parent_tick<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(TIMER, 0xff);
        if x.u8(TIMER) == 0 {
            x.add8(Obj::STATE, 2);
            x.set8(LANDED, 1);
        }
        x.add8(PHASE, 0xff);
        let phase = x.u8(PHASE) & 7;
        x.set8(PHASE, phase);
        if phase == 0 {
            spawn_pair(x);
        }
    }

    /// Two hailstones linked to the parent (`+0x5c`) and to the player.
    fn spawn_pair<R: Bench>(x: &mut Cx<'_, R>) {
        for side in 0..2 {
            let Some(o) = x.alloc(&pools::PEFF) else { return };
            o.set_u8(x.b, Obj::ACTIVE, 1);
            o.set_u8(x.b, Obj::KIND, KIND);
            o.set_u8(x.b, Obj::VARIANT, 1);
            o.set_u8(x.b, Shot::SETTING, side);
            o.set_u16(x.b, PARENT, x.o.0 as u16);
            let owner = x.u16(f::OWNER);
            o.set_u16(x.b, f::OWNER, owner);
        }
    }

    fn child_init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(Obj::STATE, 2);
        let owner = x.linked(f::OWNER);
        let flip = owner.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        for off in [0x8, 0x10] {
            let v = owner.u16(x.b, off);
            x.set16(off, v);
        }
        let mut dx = 0x10u16;
        if x.u8(f::FLIP) == 0 {
            dx = dx.wrapping_neg();
        }
        x.add16(Obj::X, dx);
        x.set16(Obj::Z, 0x10);
        x.set8(TIMER, 0);
        x.set8(ANGLE, 0x10);
        select_table(x);
        place(x);
        let r = x.rand_mask(3);
        x.set8(PHASE, r as u8);
        x.anim_start(ANIMS.0, r);
    }

    /// `0x4bae6`: speed table from `+0x58`.
    fn select_table<R: Bench>(x: &mut Cx<'_, R>) {
        let index = x.u8(TIMER) as u32;
        let table = player_data().hail_speed_tables.long_at(x.b, index);
        x.set32(Obj::SPEED_TABLE, table);
    }

    /// `0x4bb1a`: speeds of the current row, then position around the player and movement.
    fn place<R: Bench>(x: &mut Cx<'_, R>) {
        let row = (x.u8(ANGLE) as u32) << 2 & 0xffff;
        let table = x.u32(Obj::SPEED_TABLE);
        let vx = word(x.b, table + row);
        let vy = word(x.b, table + row + 2);
        x.set16(Obj::VX, vx);
        x.set16(Obj::VY, vy);
        x.set16(Obj::AX, 0);
        x.set16(Obj::AY, 0);
        let owner = x.linked(f::OWNER);
        for off in [0x8, 0x10] {
            let v = owner.u16(x.b, off);
            x.set16(off, v);
        }
        let mut dx = 0x10u16;
        if x.u8(f::FLIP) == 0 {
            dx = dx.wrapping_neg();
        }
        x.add16(Obj::X, dx);
        x.add32(Obj::Z, 0x10000);
        x.move_xy();
    }

    fn child_active<R: Bench>(x: &mut Cx<'_, R>) {
        let parent = x.linked(PARENT);
        if parent.u8(x.b, LANDED) == 0 {
            x.anim_step();
            let step: u8 = if x.u8(Shot::SETTING) != 0 { 0xfe } else { 2 };
            let angle = x.u8(ANGLE).wrapping_add(step) & 0x1f;
            x.set8(ANGLE, angle);
            place(x);
            if x.u16(Shot::Z_FRACTION) == 0 && x.u16(Obj::Z) & 0xf == 0 {
                x.add8(TIMER, 4);
                select_table(x);
            }
            if x.u32(Obj::Z) == 0x300000 {
                let r = x.rand_mask(3) + 4;
                x.set8(PHASE, r as u8);
                x.anim_start(ANIMS.0, r);
            }
            if x.u16(Obj::Z) <= 0x58 {
                return x.reinsert();
            }
        }
        // end of the fall: the hailstone bursts
        x.add8(Obj::STATE, 2);
        x.set8(TIMER, 0x20);
        select_table(x);
        x.anim_start(ANIMS.0, 9);
        x.reinsert();
    }

    fn child_fall<R: Bench>(x: &mut Cx<'_, R>) {
        if x.anim_step() {
            x.add8(Obj::STATE, 2);
        }
        let row = (x.u8(ANGLE) as u32) << 2 & 0xffff;
        let table = x.u32(Obj::SPEED_TABLE);
        let vx = word(x.b, table + row);
        let vy = word(x.b, table + row + 2);
        x.set16(Obj::VX, vx);
        x.set16(Obj::VY, vy);
        x.set16(Obj::AX, 0);
        x.set16(Obj::AY, 0);
        x.add32(Obj::Z, 0x8000);
        x.move_xy();
        x.reinsert();
    }
}

/// Type 33 (0x4f14c): twig thrown from an edge of the screen (variant `+3`: bit 0 = side), which flies `+0x58` frames
/// then, if the game is not paused, creates a trail (type 33 of variant + 1).
mod spark {
    use super::*;

    /// Countdown (word).
    const TIMER: u32 = 0x58;

    const ANIMS: crate::world::data::RomAddr = crate::world::data::RomAddr(0x1bc298);

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(Obj::STATE) {
            0 => init(x),
            2 => flight(x),
            4 => {
                x.anim_step();
                x.move_xz();
                if x.u8(Obj::FLAGS) & 1 == 0 {
                    x.free_peff();
                }
            }
            _ => x.free_peff(),
        }
    }

    fn init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(Obj::STATE, 2);
        x.set16(TIMER, 1);
        let cam = if vars::USE_CAM2.get(x.b) != 0 { 0x54e } else { 0x4fe };
        let right = x.u8(Obj::VARIANT) & 1 != 0;
        let shift = if right { 0 } else { 0x180 };
        let cam_x = x.a5_u16(cam + 8);
        let cam_y = x.a5_u16(cam + 0xc);
        x.set16(Obj::X, cam_x);
        x.set16(Obj::Y, cam_y);
        x.add16(Obj::X, shift as u16);
        x.set16(Obj::Z, 0xc0);
        let r = x.rand_mask(0x7f);
        x.add16(Obj::Z, r);
        let row = x.rand_mask(0x18) as u32;
        let (vx, vz, az) = (player_data().spark_throws.word_at(x.b, row), player_data().spark_throws.word_at(x.b, row + 2), player_data().spark_throws.word_at(x.b, row + 4));
        x.set16(Obj::VX, vx);
        x.set16(Obj::VZ, vz);
        x.set16(Obj::AZ, az);
        if !right {
            x.set16(Obj::VX, vx.wrapping_neg());
        }
        let n = x.rand_mask(1);
        x.anim_start(ANIMS.0, n);
        x.reinsert();
    }

    fn flight<R: Bench>(x: &mut Cx<'_, R>) {
        x.anim_step();
        x.move_xz();
        let t = (x.u16(TIMER) as i16).wrapping_sub(1);
        x.set16(TIMER, t as u16);
        if t >= 0 {
            return;
        }
        if vars::PAUSE.get(x.b) != 0 {
            if let Some(o) = x.alloc(&pools::PEFF) {
                o.set_u8(x.b, Obj::ACTIVE, 1);
                o.set_u8(x.b, Obj::KIND, 0x21);
                let v = x.u8(Obj::VARIANT).wrapping_add(1);
                o.set_u8(x.b, Obj::VARIANT, v);
            }
        }
        x.add8(Obj::STATE, 2);
    }
}

/// Type 20 (0x4d08c): the witch. The parent (variant 0) is born in a SET, follows the whirlwind (state 2) and creates a
/// shard (variant 1) every frame; the shard flies in an arc from the edge of the screen.
mod witch {
    use super::*;

    /// Frame counter (word).
    const TIMER: u32 = 0x58;
    /// Delay of the blinking.
    const BLINK_DELAY: u32 = 0x5b;
    /// Blinking phase.
    const BLINK: u32 = 0x5c;
    /// Set when the witch lands (h).
    const LANDED: u32 = 0x5d;
    /// Rank of the shard (from its parent).
    const RANK: u32 = 0x5e;

    const ANIMS: crate::world::data::RomAddr = crate::world::data::RomAddr(0x1bb058);
    const KIND: u8 = 0x14;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        let child = x.u8(Obj::VARIANT) != 0;
        match x.u8(Obj::STATE) {
            0 if !child => parent_init(x),
            0 => child_init(x),
            2 if !child => parent_active(x),
            2 => child_active(x),
            _ => x.free_peff(),
        }
    }

    fn parent_init<R: Bench>(x: &mut Cx<'_, R>) {
        let Some(set) = x.alloc(&pools::SET) else { return };
        set.set_u8(x.b, Obj::ACTIVE, 1);
        set.set_u8(x.b, Obj::KIND, 0);
        set.set_u8(x.b, Obj::VARIANT, 0x4e);
        set.set_u8(x.b, Shot::SETTING, 2);
        x.add8(Obj::STATE, 2);
        x.set8(BLINK, 0);
        x.set8(RANK, 0);
        x.set8(BLINK_DELAY, 0x1e);
        let owner = x.linked(f::OWNER);
        let flip = owner.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        x.copy_pos(owner);
        x.add16(Obj::Z, 0x60);
        x.add16(Obj::Y, 1);
        x.anim_start(ANIMS.0, 0);
        crate::services::palette::set_flash_color(x.b, crate::actors::families::small::LIGHTNING_BLUE);
        x.reinsert();
    }

    fn child_init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(Obj::STATE, 2);
        let owner = x.linked(f::OWNER);
        let flip = owner.u8(x.b, f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
        x.set16(Obj::VX, -0x400i16 as u16);
        x.set16(Obj::VZ, -0x700i16 as u16);
        let (cam_x, cam_y) = if vars::USE_CAM2.get(x.b) != 0 { (vars::CAM2_X, vars::CAM2_Y) } else { (vars::CAM_X, vars::CAM_Y) };
        let (cx, cy) = (cam_x.get(x.b), cam_y.get(x.b));
        x.set16(Obj::X, cx);
        x.set16(Obj::Y, cy);
        x.add16(Obj::Y, 0x18);
        x.set16(Obj::Z, 0xe8);
        let r = x.rand_mask(0x1f);
        x.add16(Obj::Z, r);
        let mut d2 = (x.u8(RANK) as u32 & 0xf) * 2;
        if owner.u8(x.b, f::FLIP) != 0 {
            let vx = x.u16(Obj::VX);
            x.set16(Obj::VX, vx.wrapping_neg());
            d2 += 0x20;
        }
        let base = player_data().witch_spread.word_at(x.b, d2);
        let jitter = x.rand_mask(3);
        x.add16(Obj::X, jitter + base);
        let anim = x.rand_mask(1) + 1;
        x.anim_start(ANIMS.0, anim);
        x.reinsert();
    }

    fn child_active<R: Bench>(x: &mut Cx<'_, R>) {
        x.move_xz();
        if (x.u16(Obj::Z) as i16) >= 0 {
            return;
        }
        x.add8(Obj::STATE, 2);
    }

    /// Creates a shard (variant 1) that keeps the rank `+0x5e` of this parent; returns true if the pool is full.
    fn spawn_spark<R: Bench>(x: &mut Cx<'_, R>) -> bool {
        match x.alloc(&pools::PEFF) {
            None => {
                true
            }
            Some(o) => {
                o.set_u8(x.b, Obj::ACTIVE, 1);
                o.set_u8(x.b, Obj::KIND, KIND);
                o.set_u8(x.b, Obj::VARIANT, 1);
                let rank = x.u8(RANK);
                o.set_u8(x.b, RANK, rank);
                let owner = x.u16(f::OWNER);
                o.set_u16(x.b, f::OWNER, owner);
                false
            }
        }
    }

    fn parent_active<R: Bench>(x: &mut Cx<'_, R>) {
        let owner = x.linked(f::OWNER);
        if owner.u8(x.b, Player::STEP) == 0xe {
            x.add8(Obj::STATE, 2);
            vars::SCROLL_Y_PLANE1.set(x.b, 0);
        } else {
            if x.u8(Shot::ANIM_EVENT) != 0 {
                x.set8(Shot::ANIM_EVENT, 0);
                x.set8(LANDED, 1);
            }
            if x.u8(LANDED) != 0 {
                x.add16(Obj::X, 2);
                x.add16(Obj::Z, 1);
                if x.u16(TIMER) & 1 == 0 {
                    x.add16(Obj::X, 0xfffc);
                    x.add16(Obj::Z, 0xfffe);
                }
            }
            x.add16(TIMER, 1);
            let t = x.u16(TIMER);
            if t > 0x140 {
                x.set_bit(Obj::FLAGS, 1, true);
                if t & 1 == 0 {
                    x.set_bit(Obj::FLAGS, 1, false);
                }
            }
            x.add8(BLINK_DELAY, 0xff);
            if x.u8(BLINK_DELAY) == 0 {
                x.set8(BLINK_DELAY, 2);
                vars::SCROLL_Y_PLANE1.set(x.b, 0x100);
                let v = x.u8(BLINK) ^ 1;
                x.set8(BLINK, v);
                if v == 0 {
                    x.set8(BLINK_DELAY, 0x60);
                    vars::SCROLL_Y_PLANE1.set(x.b, 0);
                }
            }
        }
        if x.u8(LANDED) != 0 && !spawn_spark(x) {
            x.add8(RANK, 1);
        }
        x.anim_step();
    }
}

handlers! {
    PEFF_11 0x4b93c "peff_11" hail::run,
    PEFF_20 0x4d08c "peff_20" witch::run,
    PEFF_33 0x4f14c "peff_33" spark::run,
}
