//! Player shots (PSHL), fourth series: types 19 to 24 (chain lightning, projectiles of the elf and the wizard: arrow,
//! knife, fireball). See `docs/families/joueur.md`.

use super::vars;
use crate::families::player::records::Shot;
use crate::player::Player;
use crate::families::enemy_common::records::Enemy;
use crate::families::cx::Bench;
use crate::families::cx::{handlers, Cx};
use super::pshell::*;
use crate::mem::{rd16, wr16};
use crate::objects::Obj;
use crate::objects::pools;
use crate::objects::Current;
use crate::player::tables::{player_data, players, Read};

fn signed(v: u16, negate: bool) -> u16 {
    if negate { v.wrapping_neg() } else { v }
}

fn stopped_by_terrain<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    // Original glitch: the leftovers of 0x24a8 (D3 = cell index) are re-read by a following handler (batch C:
    // `boss11_scene42_mort`, frame 4447)
    let t = crate::video::terrain_of(x.b, x.o);
    leftovers_terrain(x, &t);
    let terrain = t.number as u32;
    (terrain & 0x3f).wrapping_sub(5) as u16 <= 3 && terrain & 0x40 == 0
}

/// Type 19 (0x53e94): chain lightning. A link (variant 0) creates four screen sparks (EFFCT 12), links them into a list
/// through `+0x5c`, then pursues the nearest living target (ENEMY or BOSS, `+0x5c`) turning by 1/32 of a turn per
/// frame. Counter `A5-0x5fb4` limits the number of links.
mod chain {
    use super::*;

    const LINKS: u32 = 0xff204c;
    const SPEEDS: u32 = 0x1c89b6;
    const ANIMS: u32 = 0x15cdb6;
    const SFX_END: crate::sound::Sound = crate::sound::CMD_162_262;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.state(0x53ea2) {
            0 => init(x),
            2 => active(x),
            4 => dying(x),
            _ => release(x),
        }
    }

    fn release<R: Bench>(x: &mut Cx<'_, R>) {
        let n = rd16(x.b, LINKS).wrapping_add(1);
        wr16(x.b, LINKS, n);
        x.free_pshl();
    }

    fn init<R: Bench>(x: &mut Cx<'_, R>) {
        if pools::EFFCT.free_count(x.b) < 4 {
            return release(x);
        }
        x.add8(Obj::STATE, 2);
        x.set16(0x42, 0x12c);
        x.set8(f::BOX_SET, 6);
        let owner = x.linked(f::OWNER);
        for off in [0x8, 0xc, 0x10] {
            let v = owner.u32(x.b, off);
            x.set32(off, v);
        }
        let dz = x.u16(Obj::VZ);
        x.add16(Obj::Z, dz);
        let mut dx = x.u16(Obj::VX);
        let y = x.u32(Obj::Y).wrapping_sub(1);
        x.set32(Obj::Y, y);
        let flip = owner.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        x.set8(Obj::SPEED_INDEX, 0x18);
        if x.u8(f::FLIP) != 0 {
            x.set8(Obj::SPEED_INDEX, 8);
            dx = dx.wrapping_neg();
        }
        x.add16(Obj::X, dx);
        x.set8(0x47, 2);
        x.set32(Obj::SPEED_TABLE, SPEEDS);
        x.set32(0x58, 0xffff_ffff);
        // four sparks, sorted by address then chained
        let mut sparks = [0u16; 4];
        for s in sparks.iter_mut() {
            // Original glitch: the allocation failure is not tested (see `Pool::alloc_unchecked`).
            let spark = x.alloc_unchecked(&pools::EFFCT);
            *s = spark.0 as u16;
            spark.set_u8(x.b, Obj::ACTIVE, 1);
            spark.set_u8(x.b, Obj::KIND, 0xc);
        }
        sparks.sort();
        for (i, s) in sparks.iter().enumerate() {
            x.set16(Obj::ANIM_TIMER + 2 * i as u32, *s);
        }
        let mut previous = x.o.0 as u16;
        for (rank, s) in sparks.iter().enumerate() {
            let spark = Obj(*s as i16 as i32 as u32 & 0xff_ffff);
            spark.set_u8(x.b, Obj::VARIANT, 0);
            spark.set_u8(x.b, Shot::SETTING, rank as u8);
            spark.set_u16(x.b, 0x5c, previous);
            spark.set_u16(x.b, Shot::ATTACK_POWER, x.o.0 as u16);
            previous = *s;
        }
        find_target(x, 0);
        x.anim(ANIMS, 0);
        x.reinsert();
    }

    fn active<R: Bench>(x: &mut Cx<'_, R>) {
        x.anim_step();
        'follow: {
            if x.u16(0x42) != 0 {
                x.add16(0x42, 0xffff);
                if x.u16(0x42) == 0 {
                    x.set16(0x5c, 0);
                }
                x.add8(0x47, 0xff);
                if x.u8(0x47) != 0 {
                    break 'follow;
                }
                x.set8(0x47, 2);
                if x.u16(0x5c) != 0 {
                    let target = x.linked(0x5c);
                    if target.u16(x.b, Enemy::HP) != 0 {
                        if target.u8(x.b, Enemy::DOWNED) == 0 {
                            steer(x, target);
                        } else {
                            // target in transit: another one is sought
                            x.set16(0x52, target.0 as u16);
                            find_target(x, target.0 as u16);
                            let now = x.linked(0x5c);
                            if now.0 as u16 == x.u16(0x52) {
                                steer(x, now);
                            }
                        }
                        break 'follow;
                    }
                    x.set16(0x5c, 0);
                }
            }
            // no more target (or no more life): removal if the object leaves the screen
            if !off_screen(x) {
                if x.u16(0x42) != 0 {
                    find_target(x, 0);
                }
            } else {
                x.set8(Shot::SUB_STATE, 0);
                x.add8(Obj::STATE, 2);
                if x.u16(0x5c) != 0 {
                    let target = x.linked(0x5c);
                    let n = target.u8(x.b, 0x92).wrapping_sub(1);
                    target.set_u8(x.b, 0x92, n);
                }
                return;
            }
        }
        // history of the angles, elevation toward the target, movement
        let (a, b, c) = (x.u8(0x58), x.u8(0x59), x.u8(0x5a));
        x.set8(0x59, a);
        x.set8(0x5a, b);
        x.set8(0x5b, c);
        rise(x);
        let angle = x.u8(Obj::SPEED_INDEX);
        x.set8(0x58, angle);
        x.set8(Obj::SPEED_INDEX, angle & 0x1f);
        x.move_by_table_xy();
        x.set8(f::FLIP, 0);
        if (x.u8(Obj::SPEED_INDEX) as i8) <= 0x10 {
            x.set8(f::FLIP, 0x20);
        }
        x.reinsert();
    }

    /// Turns one notch toward the target (`0x53fe0`).
    fn steer<R: Bench>(x: &mut Cx<'_, R>, target: Obj) {
        let delta = crate::aids::direction_to(x.b, x.o, target);
        let mut step: u8 = 1;
        let diff = delta.wrapping_sub(x.u8(Obj::SPEED_INDEX)) & 0x1f;
        if diff > 0x10 {
            step = 0xff;
        }
        let a = x.u8(Obj::SPEED_INDEX).wrapping_add(step) & 0x1f;
        x.set8(Obj::SPEED_INDEX, a);
    }

    /// Climbs or descends one notch of 3 pixels toward the height of the target (`0x54090`).
    fn rise<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u16(0x5c) == 0 {
            return;
        }
        let target = x.linked(0x5c);
        let d0 = target.u16(x.b, Enemy::HEIGHT).wrapping_add(0x20).wrapping_sub(x.u16(Obj::Z)).wrapping_add(8);
        if d0 < 0x10 {
            return;
        }
        if (d0 as i16) < 0 {
            let z = x.u32(Obj::Z).wrapping_sub(0x30000);
            x.set32(Obj::Z, z);
            x.set_bit(Obj::SPEED_INDEX, 6, true);
        } else {
            let z = x.u32(Obj::Z).wrapping_add(0x30000);
            x.set32(Obj::Z, z);
            x.set_bit(Obj::SPEED_INDEX, 5, true);
        }
    }

    /// `0x54152`: true when the link is outside the visible zone (camera +/- margins).
    fn off_screen<R: Bench>(x: &mut Cx<'_, R>) -> bool {
        let cam = if vars::USE_CAM2.get(x.b) != 0 { 0x54e } else { 0x4fe };
        let (cx, cy) = (x.a5_u16(cam + 8), x.a5_u16(cam + 0xc));
        let d0 = x.u16(Obj::X).wrapping_sub(cx).wrapping_add(0x40);
        if d0 > 0x200 {
            return true;
        }
        let d0 = x.u16(Obj::Z).wrapping_add(x.u16(Obj::Y)).wrapping_sub(cy).wrapping_add(0x10);
        d0 > 0x120
    }

    /// `0x54190` / `0x54192`: chooses the target. `held` is the priority target already held.
    fn find_target<R: Bench>(x: &mut Cx<'_, R>, held: u16) {
        let mut best_priority = held;
        let mut nearest = 0u16;
        let mut best_distance = u32::MAX;
        for (pool_first, pool_free, total) in [(0x299e, 0x7e62, 0x16u16), (0x3a1e, 0x7e6c, 3u16)] {
            let active = total.wrapping_sub(x.a5_u16(pool_free)) as u8;
            if active == 0 {
                continue;
            }
            let mut left = active;
            let mut a1 = 0xff8000 + pool_first;
            loop {
                let o = Obj(a1);
                if o.u8(x.b, Obj::ACTIVE) != 0 {
                    let candidate = o.u8(x.b, 0x97) & 2 == 0
                        && o.u16(x.b, 0x62) != 0
                        && o.u8(x.b, Obj::FLAGS) & 2 == 0
                        && o.u8(x.b, 0x86) == 0;
                    if candidate {
                        let taken = o.u8(x.b, 0x92);
                        if taken != 0 {
                            let keep = best_priority != 0 && {
                                let b = Obj(0xff0000 | best_priority as u32);
                                b.u8(x.b, 0x92) < taken
                            };
                            if !keep {
                                best_priority = o.0 as u16;
                            }
                        } else {
                            let mut d = 0u32;
                            for off in [0x8, 0xc, 0x10] {
                                let diff = o.u32(x.b, off).wrapping_sub(x.u32(off));
                                d = d.wrapping_add(if o.u32(x.b, off) < x.u32(off) { diff.wrapping_neg() } else { diff });
                            }
                            if d < best_distance {
                                best_distance = d;
                                nearest = o.0 as u16;
                            }
                        }
                    }
                    left -= 1;
                    if left == 0 {
                        break;
                    }
                }
                a1 += 0xc0;
            }
        }
        *x.c.d5() = nearest as u32;
        let chosen = if nearest != 0 { nearest } else { best_priority };
        x.set16(0x5c, chosen);
        if chosen != 0 {
            let target = Obj(chosen as i16 as i32 as u32 & 0xff_ffff);
            let n = target.u8(x.b, 0x92).wrapping_add(1);
            target.set_u8(x.b, 0x92, n);
        }
    }

    /// State 4: the link goes out (with an end shard at the place where it died).
    fn dying<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(Shot::SUB_STATE) == 0 {
            x.add8(Shot::SUB_STATE, 2);
            x.set8(Obj::ACTIVE, 2);
            x.set8(0x42, 4);
            x.unlink();
            if x.u16(0x54) != 0 {
                x.set32(0x58, 0x8080_8080);
                if let Some(spark) = crate::families::effects_creators::attack(x.b, x.o, 6, 0) {
                    let v = x.u16(0x54);
                    spark.set_u16(x.b, 0x54, v);
                    let (px, pz) = (x.u32(Obj::X), x.u32(Obj::Z));
                    spark.set_u32(x.b, Obj::X, px);
                    spark.set_u32(x.b, Obj::Z, pz);
                    let flip = x.u8(f::FLIP);
                    spark.set_u8(x.b, f::FLIP, flip);
                }
                crate::sound::play(x.b, SFX_END, x.o);
            } else {
                x.set32(0x58, 0x8080_8080);
            }
        } else if x.u8(0x42) == 0 {
            x.add8(Obj::STATE, 2);
            release(x);
        }
    }
}

/// Type 20 (0x542ee): wall of fire or ice (one column per copy, according to rank `+3`) that advances toward the back
/// of the screen in steps of 16 pixels then goes out.
mod wall {
    use super::*;

    const ANIMS: u32 = 0x15d00c;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.state(0x542fc) {
            0 => {
                x.add8(Obj::STATE, 2);
                x.set8(f::BOX_SET, 6);
                let owner = x.linked(f::OWNER);
                x.copy_pos(owner);
                let mut dx = -0x80i16 as u16;
                if owner.u8(x.b, f::FLIP) != 0 {
                    dx = dx.wrapping_neg();
                }
                x.add16(Obj::X, dx);
                x.add16(Obj::Z, 0x38);
                let rank = x.u8(Obj::VARIANT) as u32;
                x.set16(Shot::Y_FRACTION, rank as u16);
                let delay = (rank << 2) as u8;
                x.set8(0x5a, delay);
                x.set8(0x5c, delay);
                x.set8(0x5b, 0x40u8.wrapping_sub(delay));
                let class = owner.u8(x.b, Player::LEVEL) as u32;
                let p1 = players("magic_damage/1").at8(x.b, class);
                let p2 = players("magic_damage/1").at8(x.b, 30 + class);
                x.set8(f::POWER, p1);
                x.set8(f::POWER2, p2);
                let anim = player_data().wall_pick.at8(x.b, class);
                x.anim(ANIMS, anim as u32);
                x.reinsert();
            }
            2 => {
                match x.u8(Shot::SUB_STATE) {
                    0 => {
                        if x.u8(0x5a) == 0 {
                            x.add8(Shot::SUB_STATE, 2);
                        } else {
                            x.add8(0x5a, 0xff);
                            let dx = signed(0x10, x.u8(f::FLIP) == 0);
                            x.add16(Obj::X, dx);
                        }
                    }
                    2 => {
                        x.add8(0x5b, 0xff);
                        if x.u8(0x5b) == 0 {
                            x.add8(Shot::SUB_STATE, 2);
                            if x.u8(Obj::VARIANT) == 0 {
                                x.add8(Obj::STATE, 2);
                            }
                        }
                    }
                    _ => {
                        if x.u8(0x5c) == 0 {
                            x.add8(Obj::STATE, 2);
                        } else {
                            x.add8(0x5c, 0xff);
                            let dx = signed(0x10, x.u8(f::FLIP) != 0);
                            x.add16(Obj::X, dx);
                        }
                    }
                }
                x.anim_step();
            }
            _ => x.free_pshl(),
        }
    }
}

/// Type 21 (0x54450): thrown knife (thief) with long speed 0x8000000.
mod knife {
    use super::*;

    const ANIMS: u32 = 0x15d0b0;
    const SFX_HIT: crate::sound::Sound = crate::sound::CMD_12F_22F;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.state(0x5445e) {
            0 => {
                x.add8(Obj::STATE, 2);
                x.set8(f::BOX_SET, 6);
                let owner = x.linked(f::OWNER);
                let flip = owner.u8(x.b, f::FLIP);
                x.set8(f::FLIP, flip);
                for off in [0x8, 0xc] {
                    let v = owner.u16(x.b, off);
                    x.set16(off, v);
                }
                let y = owner.u32(x.b, Obj::Y);
                x.set32(Obj::Y, y);
                x.add16(Obj::Y, 0xffff);
                x.set32(Obj::VX, 0x800_0000);
                let (mut dx, mut dz) = (0x48u16, 0x2cu16);
                if owner.u16(x.b, Obj::Z) != 0 {
                    (dx, dz) = (0x35, 0x38);
                }
                if x.u8(f::FLIP) == 0 {
                    dx = dx.wrapping_neg();
                    let v = x.u32(Obj::VX).wrapping_neg();
                    x.set32(Obj::VX, v);
                }
                x.add16(Obj::X, dx);
                x.add16(Obj::Z, dz);
                x.anim(ANIMS, 1);
                x.reinsert();
            }
            2 => {
                if x.u8(f::HIT_FLAGS) & 2 != 0 || stopped_by_terrain(x) {
                    x.add8(Obj::STATE, 2);
                    return crate::sound::play(x.b, SFX_HIT, x.o);
                }
                x.anim_step();
                x.move_xy();
                x.cull_offscreen();
            }
            _ => x.free_pshl(),
        }
    }
}

/// Type 22 (0x54560): fireball (magic 14). Variant 0 flies in an arc, variant 1 is the explosion it creates on the
/// ground (with screen shake and flash).
mod fireball {
    use super::*;

    const ANIMS: u32 = 0x15d15e;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        let child = x.u8(Obj::VARIANT) != 0;
        match x.state(0x5456e) {
            0 if !child => {
                x.add8(Obj::STATE, 2);
                let owner = x.linked(f::OWNER);
                x.copy_pos(owner);
                let flip = owner.u8(x.b, f::FLIP);
                x.set8(f::FLIP, flip);
                let row = ((owner.u8(x.b, Player::CHARACTER) >> 1) as u32) << 2;
                let mut dx = player_data().fireball_offsets.at16(x.b, row);
                let dz = player_data().fireball_offsets.at16(x.b, row + 2);
                *x.c.d2() = dz as u32;
                x.set16(Obj::VX, -0x500i16 as u16);
                x.set16(Obj::AX, -0x40i16 as u16);
                x.set16(Obj::VZ, -0x500i16 as u16);
                x.set16(Obj::AZ, 0);
                if x.u8(f::FLIP) != 0 {
                    let v = x.u32(Obj::VX).wrapping_neg();
                    x.set32(Obj::VX, v);
                    dx = dx.wrapping_neg();
                }
                x.add16(Obj::X, dx);
                x.add16(Obj::Z, dz);
                let class = owner.u8(x.b, Player::LEVEL) as u32;
                let p1 = players("magic_damage/4").at8(x.b, class);
                let p2 = players("magic_damage/4").at8(x.b, 30 + class);
                x.set8(f::POWER, p1);
                x.set8(f::POWER2, p2);
                x.anim(ANIMS, 0);
                x.reinsert();
            }
            0 => {
                x.add8(Obj::STATE, 2);
                let parent = x.linked(0x5c);
                x.copy_pos(parent);
                let flip = parent.u8(x.b, f::FLIP);
                x.set8(f::FLIP, flip);
                let (p1, p2) = (parent.u8(x.b, f::POWER), parent.u8(x.b, f::POWER2));
                x.set8(f::POWER, p1);
                x.set8(f::POWER2, p2);
                x.set8(0x56, 4);
                vars::SCROLL_Y_PLANE1.set(x.b, 0x100);
                crate::palette::set_flash_color(x.b, crate::palette::FLASH_COLOR_RED);
                crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::A, 0x20);
                x.anim(ANIMS, 2);
                x.reinsert();
            }
            2 if !child => {
                if x.u8(Shot::SUB_STATE) == 0 {
                    if x.anim_step() {
                        x.add8(Shot::SUB_STATE, 2);
                        x.anim(ANIMS, 1);
                    }
                } else {
                    x.move_xz();
                    x.anim_step();
                    if (x.u16(Obj::Z) as i16) < 0 {
                        x.add8(Obj::STATE, 2);
                        if let Some(o) = x.alloc(&pools::PSHL) {
                            o.set_u8(x.b, Obj::ACTIVE, 1);
                            o.set_u8(x.b, Obj::KIND, 0x16);
                            o.set_u8(x.b, Obj::VARIANT, 1);
                            o.set_u16(x.b, 0x5c, x.o.0 as u16);
                            let owner = x.u16(f::OWNER);
                            o.set_u16(x.b, f::OWNER, owner);
                        }
                    }
                    x.cull_offscreen();
                }
            }
            2 => {
                if x.u8(0x56) != 0 {
                    x.add8(0x56, 0xff);
                    if x.u8(0x56) == 0 {
                        vars::SCROLL_Y_PLANE1.set(x.b, 0);
                    } else if x.u8(0x56) == 2 {
                        crate::palette::set_flash_color(x.b, crate::palette::FLASH_COLOR_WHITE_VALUE);
                    }
                }
                if x.anim_step() {
                    x.add8(Obj::STATE, 2);
                }
            }
            _ => x.free_pshl(),
        }
    }
}

/// Type 23 (0x5471a): the elf's lightning, placed in front of the caster (move 0x17 places it higher).
mod bolt {
    use super::*;

    const ANIMS: u32 = 0x15d3fa;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.state(0x54728) {
            0 => {
                x.add8(Obj::STATE, 2);
                x.set8(f::POWER, 0);
                let owner = x.linked(f::OWNER);
                let flip = owner.u8(x.b, f::FLIP);
                x.set8(f::FLIP, flip);
                x.copy_pos(owner);
                if owner.u8(x.b, Player::SPECIAL_KIND) == 0x17 {
                    let dx = signed(-0x3c_i16 as u16, x.u8(f::FLIP) != 0);
                    *x.c.d2() = 0xfffe_8000;
                    x.add16(Obj::X, dx);
                    x.add16(Obj::Z, 0x39);
                    x.add32(Obj::Y, 0xfffe_8000);
                } else {
                    let row = ((owner.u8(x.b, Player::CHARACTER) >> 1) as u32) << 2;
                    let dx = player_data().bolt_offsets.at16(x.b, row);
                    let dz = player_data().bolt_offsets.at16(x.b, row + 2);
                    *x.c.d2() = dz as u32;
                    let dx = signed(dx, x.u8(f::FLIP) != 0);
                    *x.c.d3() = 0xfffe_8000;
                    x.add16(Obj::X, dx);
                    x.add16(Obj::Z, dz);
                    x.add32(Obj::Y, 0xfffe_8000);
                }
                x.anim(ANIMS, 0);
                x.reinsert();
            }
            2 => {
                if x.anim_step() {
                    x.add8(Obj::STATE, 2);
                }
            }
            _ => x.free_pshl(),
        }
    }
}

/// Type 24 (0x548a0): lightning spark stuck to its creator (`+0x5c`).
mod spark {
    use super::*;

    const ANIMS: u32 = 0x15ad38;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.state(0x548ae) {
            0 => {
                x.add8(Obj::STATE, 2);
                x.set8(f::BOX_SET, 6);
                let parent = x.linked(0x5c);
                x.copy_pos(parent);
                let n = 0x1du32.wrapping_add(x.u8(Obj::VARIANT) as u32) & 0xff;
                x.anim(ANIMS, n);
                x.set32(Obj::FRAME, 0);
                x.reinsert();
            }
            2 => {
                let parent = x.linked(0x5c);
                x.copy_pos(parent);
                x.reinsert();
            }
            _ => x.free_pshl(),
        }
    }
}

handlers! {
    0x53e94 "pshl_19" chain::run,
    0x542ee "pshl_20" wall::run,
    0x54450 "pshl_21" knife::run,
    0x54560 "pshl_22" fireball::run,
    0x5471a "pshl_23" bolt::run,
    0x548a0 "pshl_24" spark::run,
}
