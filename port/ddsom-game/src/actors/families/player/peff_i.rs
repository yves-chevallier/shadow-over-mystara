//! Player effects (PEFF), ninth series: types 30 (storm), 31 (whirlwind) and 32 (lightning target). See
//! `docs/families/joueur.md`.

use super::vars;
use crate::actors::families::player::records::Shot;
use crate::world::objects::player::Player;
use crate::world::objects::Enemy;
use crate::actors::families::cx::Bench;
use crate::actors::families::cx::{handlers, Cx};
use super::pshell::*;
use crate::world::objects::Obj;
use crate::world::objects::pools;
use crate::world::objects::Current;
use crate::actors::player::tables::{player_data, Aligned, Read};

const SPEEDS: crate::world::data::RomAddr = crate::world::data::RomAddr(0x1c89b6);

fn signed(v: u16, negate: bool) -> u16 {
    if negate { v.wrapping_neg() } else { v }
}

/// Type 30 (0x4e79c): the storm (magic 18, kind 0x1e). Variant 0 is the cloud (state `+5`: 0 rumbles, 4 waits for the
/// end, 8 shakes, 0xc dissipates) which creates sixteen bolts (variant 1, rank `+2f`) then four sparks (variant 2); the
/// bolts reach a random point of the target.
mod storm {
    use super::*;

    /// Target point of the bolt: abscissa, depth.
    const TARGET_X: u32 = 0x54;
    const TARGET_Y: u32 = 0x56;
    /// Countdown of the current phase (word).
    const TIMER: u32 = 0x58;
    /// Bolts alive (counted down by the bolts).
    const BOLTS: u32 = 0x5a;
    /// Parent cloud (16-bit pointer).
    const PARENT: u32 = 0x5c;

    const ANIMS: crate::world::data::RomAddr = crate::world::data::RomAddr(0x1bbe14);
    const SFX_RUMBLE: crate::services::sound::Sound = crate::services::sound::CMD_188;
    const SFX_BOLT: crate::services::sound::Sound = crate::services::sound::CMD_189;
    const SFX_BEGIN: crate::services::sound::Sound = crate::services::sound::CMD_18A;
    const SFX_END: crate::services::sound::Sound = crate::services::sound::CMD_194;
    const KIND: u8 = 0x1e;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        let variant = x.u8(Obj::VARIANT);
        match x.state(0x4e7aa) {
            0 => match variant {
                0 => init_cloud(x),
                1 => init_bolt(x),
                _ => init_spark(x),
            },
            2 => match variant {
                0 => cloud(x),
                1 => bolt(x),
                _ => spark(x),
            },
            _ => x.free_peff(),
        }
    }

    fn init_cloud<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(Obj::STATE, 2);
        follow_owner(x);
        x.add16(Obj::Z, 0x20);
        x.add16(Obj::Y, 2);
        x.set16(TIMER, 0x20);
        x.anim(ANIMS.0, 0);
        x.reinsert();
    }

    fn init_bolt<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(Obj::STATE, 2);
        let parent = x.linked(PARENT);
        let (px, py) = (parent.u16(x.b, Obj::X), parent.u16(x.b, Obj::Y));
        x.set16(TARGET_X, px);
        x.set16(TARGET_Y, py.wrapping_add(1));
        x.set16(Obj::X, px);
        x.set16(Obj::Y, py);
        let a = x.rand_and(0x1ff).wrapping_sub(0xff) as u16;
        x.add16(Obj::X, a);
        let mut d2 = a;
        let b = x.rand_and(0x3f).wrapping_sub(0x1f) as u16;
        x.add16(Obj::Y, b);
        let b = b.wrapping_add(0x1f);
        d2 = d2.wrapping_add(0xff).wrapping_add(b);
        let vz = ((d2 as i16) >> 4) as u16;
        *x.c.d2() = d2 as u32;
        x.set16(Obj::AZ, vz);
        x.set16(Obj::VZ, 0);
        let anim = x.rand_and(1) + 1;
        x.anim(ANIMS.0, anim);
    }

    fn init_spark<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(Obj::STATE, 2);
        let owner = x.linked(f::OWNER);
        let px = owner.u16(x.b, Obj::X);
        x.set16(Obj::X, px);
        x.set16(Obj::Z, 0);
        let py = owner.u16(x.b, Obj::Y);
        x.set16(Obj::Y, py);
        let k = (x.u8(Shot::SETTING) as u32) << 3;
        let (dx, dz, dy) = (player_data().storm_offsets_a.word_at(x.b, k), player_data().storm_offsets_a.word_at(x.b, k + 2), player_data().storm_offsets_a.word_at(x.b, k + 4));
        x.add16(Obj::X, dx);
        x.add16(Obj::Z, dz);
        x.add16(Obj::Y, dy);
        x.set32(Obj::SPEED_TABLE, SPEEDS.0);
        x.set16(TIMER, 2);
        let angle = x.u8(Shot::SETTING).wrapping_shl(3) & 0x1f;
        x.set8(Obj::SPEED_INDEX, angle);
        let n = x.u8(Shot::SETTING) as u32;
        x.anim(ANIMS.0, n);
        x.reinsert();
    }

    // --- active states -------------------------------------------------------------------------------

    fn cloud<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(Shot::SUB_STATE) {
            0 => {
                x.add16(TIMER, 0xffff);
                if (x.u16(TIMER) as i16) < 0 {
                    x.add8(Shot::SUB_STATE, 4);
                    return spawn_bolts(x);
                }
                let hidden = x.u8(Obj::FLAGS) & 2 == 0;
                x.set_bit(Obj::FLAGS, 1, hidden);
                let owner = x.linked(f::OWNER);
                let d0 = owner.u16(x.b, Player::ACTION_TIMER);
                crate::actors::camera_modes::start_shake(x.b, crate::actors::camera_modes::shake::A, d0);
                crate::services::sound::play(x.b, SFX_RUMBLE, x.o);
            }
            4 => {
                if vars::PAUSE.get(x.b) == 0 {
                    return end(x);
                }
                let owner = x.linked(f::OWNER);
                let d0 = owner.u16(x.b, Player::ACTION_TIMER);
                crate::actors::camera_modes::start_shake(x.b, crate::actors::camera_modes::shake::B, d0);
                if x.u8(BOLTS) != 0 {
                    return;
                }
                x.set16(TIMER, 0x20);
                x.add8(Shot::SUB_STATE, 4);
                crate::services::sound::play(x.b, SFX_BEGIN, x.o);
            }
            8 => {
                toggle(x, 0x9, 1 << 1);
                toggle(x, 0xd, 1);
                x.add16(TIMER, 0xffff);
                if (x.u16(TIMER) as i16) < 0 {
                    x.add8(Shot::SUB_STATE, 4);
                    let owner = x.linked(f::OWNER);
                    let d0 = owner.u16(x.b, Player::ACTION_TIMER);
                    crate::actors::camera_modes::start_shake(x.b, crate::actors::camera_modes::shake::C, d0);
                    spawn_sparks(x);
                    shake_tail(x);
                }
            }
            _ => shake_tail(x),
        }
    }

    fn toggle<R: Bench>(x: &mut Cx<'_, R>, off: u32, mask: u8) {
        let v = x.u8(off) ^ mask;
        x.set8(off, v);
    }

    /// Substate 0xc: the storm keeps shaking as long as the game is frozen.
    fn shake_tail<R: Bench>(x: &mut Cx<'_, R>) {
        toggle(x, 0x9, 1);
        toggle(x, 0xd, 1);
        if vars::PAUSE.get(x.b) == 0 {
            return end(x);
        }
        x.anim_step();
    }

    fn end<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(Obj::STATE, 2);
        x.anim_step();
        crate::services::sound::play(x.b, SFX_END, x.o);
    }

    /// `0x4ec02`: sixteen bolts.
    fn spawn_bolts<R: Bench>(x: &mut Cx<'_, R>) {
        for rank in (0..16u8).rev() {
            let Some(o) = x.alloc(&pools::PEFF) else { return };
            o.set_u8(x.b, Obj::ACTIVE, 1);
            o.set_u8(x.b, Obj::KIND, KIND);
            o.set_u8(x.b, Obj::VARIANT, 1);
            o.set_u8(x.b, Shot::SETTING, rank);
            o.set_u16(x.b, PARENT, x.o.0 as u16);
            let owner = x.u16(f::OWNER);
            o.set_u16(x.b, f::OWNER, owner);
            x.add8(BOLTS, 1);
        }
    }

    /// `0x4ec36`: four sparks.
    fn spawn_sparks<R: Bench>(x: &mut Cx<'_, R>) {
        for rank in (0..4u8).rev() {
            let Some(o) = x.alloc(&pools::PEFF) else { return };
            o.set_u8(x.b, Obj::ACTIVE, 1);
            o.set_u8(x.b, Obj::KIND, KIND);
            o.set_u8(x.b, Obj::VARIANT, 2);
            o.set_u8(x.b, Shot::SETTING, rank);
            o.set_u16(x.b, PARENT, x.o.0 as u16);
            let owner = x.u16(f::OWNER);
            o.set_u16(x.b, f::OWNER, owner);
        }
    }

    fn move_towards<R: Bench>(x: &mut Cx<'_, R>, at: u32, target: u16, step: u32) {
        let here = x.u16(at);
        if target == here {
            return;
        }
        if (target as i16) > (here as i16) {
            let v = x.u32(at).wrapping_add(step);
            x.set32(at, v);
            if (target as i16) <= (x.u16(at) as i16) {
                x.set16(at, target);
            }
        } else {
            let v = x.u32(at).wrapping_sub(step);
            x.set32(at, v);
            if (target as i16) >= (x.u16(at) as i16) {
                x.set16(at, target);
            }
        }
    }

    fn bolt<R: Bench>(x: &mut Cx<'_, R>) {
        x.reinsert();
        if x.u8(Shot::SUB_STATE) != 0 {
            let parent = x.linked(PARENT);
            let k = (x.u8(Shot::SETTING) as u32) << 2;
            let (dx, dz) = (player_data().storm_offsets_b.word_at(x.b, k), player_data().storm_offsets_b.word_at(x.b, k + 2));
            x.set16(Obj::X, dx);
            x.set16(Obj::Z, dz);
            let (px, pz) = (parent.u16(x.b, Obj::X), parent.u16(x.b, Obj::Z));
            let py = parent.u16(x.b, Obj::Y);
            x.set16(Obj::Y, py);
            x.add16(Obj::Y, 0xffff);
            x.add16(Obj::X, px);
            x.add16(Obj::Z, pz);
            if parent.u8(x.b, Shot::SUB_STATE) == 0xc {
                x.add8(Obj::STATE, 2);
            }
            return;
        }
        x.anim_step();
        let vz = x.u16(Obj::VZ).wrapping_add(x.u16(Obj::AZ));
        x.set16(Obj::VZ, vz);
        let d0 = (((vz as u32) << 16) as i32 >> 8) as u32;
        let z = x.u32(Obj::Z).wrapping_add(d0);
        x.set32(Obj::Z, z);
        let tx = x.u16(TARGET_X);
        let ty = x.u16(TARGET_Y);
        move_towards(x, 0x8, tx, 0x60000);
        move_towards(x, 0x10, ty, 0x8000);
        if x.u16(Obj::X) != tx || x.u16(Obj::Y) != ty {
            return;
        }
        x.add8(Shot::SUB_STATE, 2);
        let parent = x.linked(PARENT);
        let n = parent.u8(x.b, BOLTS).wrapping_sub(1);
        parent.set_u8(x.b, BOLTS, n);
        x.set8(Shot::SETTING, n);
        crate::services::sound::play(x.b, SFX_BOLT, x.o);
        let k = (n as u32) << 2;
        let (dx, dz) = (player_data().storm_offsets_b.word_at(x.b, k), player_data().storm_offsets_b.word_at(x.b, k + 2));
        x.set16(Obj::X, dx);
        x.set16(Obj::Z, dz);
        let parent = x.linked(PARENT);
        let (px, pz) = (parent.u16(x.b, Obj::X), parent.u16(x.b, Obj::Z));
        let py = parent.u16(x.b, Obj::Y);
        x.set16(Obj::Y, py);
        x.add16(Obj::Y, 0xffff);
        x.add16(Obj::X, px);
        x.add16(Obj::Z, pz);
    }

    fn spark<R: Bench>(x: &mut Cx<'_, R>) {
        if vars::PAUSE.get(x.b) == 0 {
            x.add8(Obj::STATE, 2);
            return;
        }
        x.add16(TIMER, 0xffff);
        if (x.u16(TIMER) as i16) < 0 {
            let a = x.u8(Obj::SPEED_INDEX).wrapping_add(1) & 0x1f;
            x.set8(Obj::SPEED_INDEX, a);
            x.set16(TIMER, 2);
        }
        x.reinsert();
        x.move_by_table_xy();
        let k = x.u8(Shot::SETTING);
        if k & 1 == 0 {
            let vx = x.u16(Obj::VX);
            x.set16(Obj::VZ, if k & 2 == 0 { vx } else { ((vx as i16) >> 1) as u16 });
        } else {
            let vx = x.u16(Obj::VX).wrapping_neg();
            x.set16(Obj::VZ, if k & 2 == 0 { vx } else { ((vx as i16) >> 1) as u16 });
        }
        x.move_z();
    }
}

/// Type 31 (0x4ec82): whirlwind (magic 19, kind 0x1f): variant 0 = the sucking eye (`+5`: 0 grows, then it sweeps the
/// zone), variant 1 = a satellite (rank `+0x5a`, angle `+0x2d`) that is pulled in a spiral.
mod whirl {
    use super::*;

    /// Countdown (h).
    const TIMER: u32 = 0x58;
    /// Row of the spawn table (0..0x1f).
    const ROW: u32 = 0x59;
    /// Spawned pieces (h).
    const COUNT: u32 = 0x5a;
    /// Delay of the blinking.
    const BLINK_DELAY: u32 = 0x5b;
    /// Blinks left.
    const BLINKS: u32 = 0x5c;
    /// Blinking phase.
    const BLINK: u32 = 0x5d;

    const ANIMS: crate::world::data::RomAddr = crate::world::data::RomAddr(0x1bbfda);
    const SFX_START: crate::services::sound::Sound = crate::services::sound::CMD_187;
    const SFX_LOOP: crate::services::sound::Sound = crate::services::sound::CMD_173_C;
    const KIND: u8 = 0x1f;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        let child = x.u8(Obj::VARIANT) != 0;
        match x.state(0x4ec90) {
            0 if !child => init_eye(x),
            0 => init_satellite(x),
            2 if !child => eye(x),
            2 => {
                x.move_xz();
                x.add8(TIMER, 0xff);
                if x.u8(TIMER) == 0 {
                    x.add8(Obj::STATE, 2);
                }
            }
            _ => x.free_peff(),
        }
    }

    fn init_eye<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(Obj::STATE, 2);
        x.set8(ROW, 0);
        let owner = follow_owner(x);
        x.add16(Obj::Y, 0xffff);
        let (mut dx, mut dz) = (-0x37i16 as u16, 0x4au16);
        if owner.u8(x.b, Player::SPECIAL_KIND) != 0x25 {
            let row = ((owner.u8(x.b, Player::CHARACTER) >> 1) as u32) << 2;
            dx = player_data().whirl_offsets.word_at(x.b, row);
            dz = player_data().whirl_offsets.word_at(x.b, row + 2);
        }
        *x.c.d2() = dz as u32;
        let dx = signed(dx, owner.u8(x.b, f::FLIP) != 0);
        x.add16(Obj::X, dx);
        x.add16(Obj::Z, dz);
        x.add16(Obj::Y, 0xffff);
        crate::services::sound::play(x.b, SFX_START, x.o);
    }

    fn init_satellite<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(Obj::STATE, 2);
        let owner = x.linked(f::OWNER);
        x.copy_pos(owner);
        x.set8(TIMER, 8);
        let k = x.u8(Obj::SPEED_INDEX) as u32;
        let flip = player_data().whirl_flips.at8(x.b, k);
        x.set8(f::FLIP, flip);
        let n = x.u8(COUNT) as i32;
        let (mut d4, mut d5) = (0u16, 0u16);
        let mut left = n;
        loop {
            x.select_speed(&crate::world::objects::motion::SPEEDS_A, 0x21);
            d4 = d4.wrapping_add(x.u16(Obj::VX));
            d5 = d5.wrapping_add(x.u16(Obj::VY));
            left -= 1;
            if left < 0 {
                break;
            }
        }
        *x.c.d4() = d4 as u32;
        *x.c.d5() = d5 as u32;
        x.add16(Obj::X, ((d4 as i16) >> 8) as u16);
        x.add16(Obj::Z, ((d5 as i16) >> 8) as u16);
        x.select_speed(&crate::world::objects::motion::SPEEDS_A, 0x19);
        let vy = x.u16(Obj::VY);
        x.set16(Obj::VZ, vy);
        let vx = x.u16(Obj::VX).wrapping_neg();
        x.set16(Obj::VX, vx);
        let vz = x.u16(Obj::VZ).wrapping_neg();
        x.set16(Obj::VZ, vz);
        let angle = x.u8(Obj::SPEED_INDEX) as u32;
        x.anim(ANIMS.0, angle);
        x.reinsert();
    }

    fn eye<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(Shot::SUB_STATE) == 0 {
            let owner = x.linked(f::OWNER);
            if owner.u16(x.b, Player::ACTION_TIMER) <= 0x10 {
                x.add8(Shot::SUB_STATE, 2);
                x.set8(BLINK_DELAY, 4);
                x.set8(BLINKS, 3);
                x.set8(BLINK, 1);
                vars::SCROLL_Y_PLANE1.set(x.b, 0x100);
                crate::services::palette::set_flash_color(x.b, crate::services::palette::FLASH_COLOR_WHITE_VALUE);
                crate::services::sound::play(x.b, SFX_LOOP, x.o);
                x.anim(ANIMS.0, 0x20);
                return x.reinsert();
            }
            // the whirlwind forms: one satellite per frame
            let k = x.u8(ROW) as u32;
            x.add8(ROW, 1);
            let n = x.u8(ROW) & 0x1f;
            x.set8(ROW, n);
            let angle = player_data().whirl_spawn_table.at8(x.b, 2 * k);
            let count = player_data().whirl_spawn_table.at8(x.b, 2 * k + 1);
            if let Some(o) = x.alloc(&pools::PEFF) {
                o.set_u8(x.b, Obj::ACTIVE, 1);
                o.set_u8(x.b, Obj::KIND, KIND);
                o.set_u8(x.b, Obj::VARIANT, 1);
                o.set_u8(x.b, COUNT, count);
                o.set_u8(x.b, Obj::SPEED_INDEX, angle);
                o.set_u16(x.b, f::OWNER, x.o.0 as u16);
            }
            return;
        }
        if x.u8(BLINKS) != 0 {
            x.add8(BLINK_DELAY, 0xff);
            if x.u8(BLINK_DELAY) != 0 {
                return;
            }
            x.set8(BLINK_DELAY, 4);
            vars::SCROLL_Y_PLANE1.set(x.b, 0);
            let v = x.u8(BLINK) ^ 1;
            x.set8(BLINK, v);
            if v != 0 {
                vars::SCROLL_Y_PLANE1.set(x.b, 0x100);
            }
            x.add8(BLINKS, 0xff);
            return;
        }
        if !x.anim_step() {
            return;
        }
        x.add8(Obj::STATE, 2);
        vars::SCROLL_Y_PLANE1.set(x.b, 0);
        sweep(x);
    }

    /// `0x4eefe`: attracts (and hurts) all the monsters (bosses, then enemies) in the zone in front of the caster.
    fn sweep<R: Bench>(x: &mut Cx<'_, R>) {
        let owner = x.linked(f::OWNER);
        for (first, count) in [(0x3a1eu32, 3u32), (0x299e, 22)] {
            let mut a2 = 0xff8000 + first;
            for _ in 0..count {
                hit(x, owner, Obj(a2));
                a2 += 0xc0;
            }
        }
    }

    fn hit<R: Bench>(x: &mut Cx<'_, R>, player: Obj, m: Obj) {
        if m.u8(x.b, Enemy::ACTIVE) == 0 || m.u8(x.b, Enemy::DISPLAY) == 0 || m.u16(x.b, Enemy::HP) == 0 || m.u8(x.b, Enemy::BOXES_OFF) != 0 {
            return;
        }
        if m.u32(x.b, Enemy::IMMUNITY) >> 16 & 1 != 0 {
            return;
        }
        if m.u16(x.b, Enemy::STATE_WORD) == 0x400 && matches!(m.u8(x.b, Enemy::REACTION), 0x68 | 0x6c) {
            return;
        }
        let px = player.u16(x.b, Obj::X);
        let mx = m.u16(x.b, Enemy::X);
        if player.u8(x.b, f::FLIP) != 0 {
            let d0 = px.wrapping_sub(0x10);
            if d0 > mx || d0.wrapping_add(0xe0) <= mx {
                return;
            }
        } else {
            let d0 = px.wrapping_add(0x10);
            if d0 <= mx || d0.wrapping_sub(0xe0) > mx {
                return;
            }
        }
        m.set_u32(x.b, Enemy::STATE_LONG, 0x0400_0000);
        m.set_u8(x.b, Enemy::DISPLAY, 1);
        m.set_u8(x.b, Enemy::REACTION, 0x68);
        let (class_id, variant_id) = (player.u8(x.b, Player::INDEX), player.u8(x.b, Player::CHARACTER));
        m.set_u8(x.b, Enemy::HIT_BY_KIND, class_id);
        m.set_u8(x.b, Enemy::HIT_BY_VARIANT, variant_id);
        m.set_u16(x.b, Enemy::HOLDER, player.0 as u16);
        m.set_u8(x.b, Enemy::GRABBED, 0x78);
        if player.u8(x.b, Player::SPECIAL_KIND) == 0x25 {
            let class = player.u8(x.b, Player::LEVEL) as u32;
            let v = player_data().whirl_power.at8(x.b, class);
            m.set_u8(x.b, Enemy::GRABBED, v);
        }
    }
}

/// Type 32 (0x4f026): lightning target (kind 0x20): a point thrown from the caster that falls back to the ground and
/// stays on screen before disappearing after `+0x58` frames.
mod target {
    use super::*;

    /// Lifetime (word).
    const TIMER: u32 = 0x58;

    const ANIMS: crate::world::data::RomAddr = crate::world::data::RomAddr(0x1bc23e);

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.state(0x4f034) {
            0 => {
                x.add8(Obj::STATE, 2);
                x.set16(Obj::VX, -0x180i16 as u16);
                x.set16(Obj::VZ, 0x300);
                x.set16(Obj::AZ, -0x48i16 as u16);
                let owner = follow_owner(x);
                x.add16(Obj::Y, 0xffff);
                let row = ((owner.u8(x.b, Player::CHARACTER) >> 1) as u32) << 2;
                let dx = player_data().target_offsets.word_at(x.b, row);
                let dz = player_data().target_offsets.word_at(x.b, row + 2);
                *x.c.d2() = dz as u32;
                let mut dx = dx;
                if owner.u8(x.b, f::FLIP) != 0 {
                    dx = dx.wrapping_neg();
                    let vx = x.u16(Obj::VX);
                    x.set16(Obj::VX, vx.wrapping_neg());
                }
                x.add16(Obj::X, dx);
                x.add16(Obj::Z, dz);
                let n = x.u8(Obj::VARIANT) as u32;
                x.anim(ANIMS.0, n);
                x.reinsert();
            }
            2 => {
                if x.u8(Shot::SUB_STATE) == 0 {
                    x.move_xz();
                    if (x.u16(Obj::Z) as i16) <= 0 {
                        x.add8(Shot::SUB_STATE, 2);
                        x.set16(TIMER, 0x78);
                        x.set32(Obj::Z, 0);
                        let cam_x = if vars::USE_CAM2.get(x.b) != 0 { vars::CAM2_X.get(x.b) } else { vars::CAM_X.get(x.b) };
                        let low = cam_x.wrapping_add(0x20);
                        if (low as i16) > (x.u16(Obj::X) as i16) {
                            x.set16(Obj::X, low);
                        } else {
                            let high = cam_x.wrapping_add(0x160);
                            if high <= x.u16(Obj::X) {
                                x.set16(Obj::X, high);
                            }
                        }
                    }
                } else {
                    x.add16(TIMER, 0xffff);
                    if x.u16(TIMER) == 0 {
                        x.add8(Obj::STATE, 2);
                    }
                }
                x.anim_step();
            }
            _ => x.free_peff(),
        }
    }
}

handlers! {
    PEFF_30 0x4e79c "peff_30" storm::run,
    PEFF_31 0x4ec82 "peff_31" whirl::run,
    PEFF_32 0x4f026 "peff_32" target::run,
}
