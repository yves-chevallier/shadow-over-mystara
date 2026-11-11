//! Player shots (PSHL), fifth series: types 6 (ray whirlwind) and 18 (cleric's burst).
//! See `docs/families/joueur.md`.

use super::vars;
use crate::actors::families::player::records::Shot;
use crate::actors::player::Player;
use crate::world::objects::Obj;
use crate::actors::families::cx::Bench;
use crate::actors::families::cx::{handlers, Cx};
use super::pshell::*;
use crate::world::objects::pools;
use crate::world::objects::Current;
use crate::actors::player::tables::{player_data, byte, Aligned, Read};

fn signed(v: u16, negate: bool) -> u16 {
    if negate { v.wrapping_neg() } else { v }
}

/// Type 6 (0x527ec): ray shot (PEFF 9) that describes a circle around the bearer (`+0x5c`), the rank of the arc being
/// variant `+3`.
mod orbit {
    use super::*;

    /// Frames between two rotation steps.
    const TURN_DELAY: u32 = 0x58;
    /// Countdown to the next rotation step.
    const TURN_TIMER: u32 = 0x59;
    /// Row of the turn table (even).
    const ARC_ROW: u32 = 0x5a;
    /// Half of the turn table: 0 or 0x10.
    const ARC_SIDE: u32 = 0x5b;
    /// Bearer of the orbit (16-bit pointer).
    const BEARER: u32 = 0x5c;
    /// Orbiting shots of the bearer (h).
    const ORBITS: u32 = 0x5d;

    const ANIMS: crate::world::data::RomAddr = crate::world::data::RomAddr(0x15b346);
    const SPEEDS: crate::world::data::RomAddr = crate::world::data::RomAddr(0x1c89b6);
    const SFX: crate::hw::sound::Sound = crate::hw::sound::CMD_33A_43A;
    const TURN_TABLE: u32 = 0x52b32;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.state(0x527fa) {
            0 => init(x),
            2 => {
                match x.u8(Shot::SUB_STATE) {
                    0 => wait(x),
                    2 => {
                        x.add8(Shot::SUB_STATE, 2);
                        x.anim_step();
                    }
                    _ => fly(x),
                }
                x.reinsert();
            }
            _ => {
                let parent = x.linked(BEARER);
                let n = parent.u8(x.b, ORBITS).wrapping_sub(1);
                parent.set_u8(x.b, ORBITS, n);
                x.free_pshl();
            }
        }
    }

    fn init<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(Obj::STATE, 2);
        let parent = x.linked(BEARER);
        x.copy_pos(parent);
        let flip = parent.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        let v = x.u8(Obj::VARIANT) as u32;
        let delay = player_data().orbit_delays.at8(x.b, v);
        x.set8(Obj::COUNTER_53, delay);
        x.set8(f::BOX_SET, 8);
        let row = v << 3;
        let dx = player_data().orbit_offsets.word_at(x.b, row);
        let dz = player_data().orbit_offsets.word_at(x.b, row + 2);
        let dy = player_data().orbit_offsets.word_at(x.b, row + 4);
        x.add16(Obj::Z, dz);
        x.add16(Obj::Y, dy);
        let dx = signed(dx, x.u8(f::FLIP) != 0);
        x.add16(Obj::X, dx);
        let owner = x.linked(f::OWNER);
        let class = owner.u8(x.b, Player::LEVEL) as u32;
        let p1 = player_data().orbit_power_a.at8(x.b, class);
        let p2 = player_data().orbit_power_b.at8(x.b, class);
        x.set8(f::POWER, p1);
        x.set8(f::POWER2, p2);
        x.set_bit(Obj::FLAGS, 1, true);
        x.anim(ANIMS.0, 0);
        crate::hw::sound::play(x.b, SFX, x.o);
    }

    /// Substate 0: waits for delay `+0x53`, then moves on to the opening animation.
    fn wait<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(Shot::STEP) == 0 {
            let delay = x.u8(Obj::COUNTER_53);
            if delay != 0 {
                x.add8(Obj::COUNTER_53, 0xff);
                if x.u8(Obj::COUNTER_53) != 0 {
                    return;
                }
            }
            x.set_bit(Obj::FLAGS, 1, false);
            x.add8(Shot::STEP, 2);
        }
        if x.anim_step() {
            x.add8(Shot::SUB_STATE, 2);
            x.set8(Shot::STEP, 0);
            x.set16(Shot::OWN_PALETTE, 0x100);
            x.anim(ANIMS.0, 1);
        }
    }

    /// Substate 4: circular flight.
    fn fly<R: Bench>(x: &mut Cx<'_, R>) {
        match x.u8(Shot::STEP) {
            0 => {
                x.add8(Shot::STEP, 2);
                x.set8(Shot::SUB_STEP, 0);
                x.set32(Obj::SPEED_TABLE, SPEEDS.0);
                x.set8(Obj::SPEED_INDEX, 0x18);
                x.set8(ARC_ROW, 0);
                x.set8(Shot::SETTING, 0);
                if x.u8(f::FLIP) != 0 {
                    x.set8(Obj::SPEED_INDEX, 8);
                    x.set8(ARC_ROW, 8);
                    x.add8(Shot::SETTING, 2);
                }
                x.set8(ARC_SIDE, 0);
                let owner = x.linked(f::OWNER);
                let cam_y = if vars::USE_CAM2.get(x.b) != 0 { vars::CAM2_Y.get(x.b) } else { vars::CAM_Y.get(x.b) };
                let d0 = owner.u16(x.b, Obj::Y).wrapping_sub(cam_y);
                if d0 < 0x60 {
                    x.set8(ARC_SIDE, 0x10);
                    x.add8(Shot::SETTING, 1);
                }
                x.set16(TURN_DELAY, 0x202);
                x.anim_step();
            }
            _ => match x.u8(Shot::SUB_STEP) {
                0 => {
                    let cam_x = if vars::USE_CAM2.get(x.b) != 0 { vars::CAM2_X.get(x.b) } else { vars::CAM_X.get(x.b) };
                    let d1 = x.u16(Obj::X).wrapping_sub(cam_x) as i16;
                    if x.u8(Obj::SPEED_INDEX) == 8 {
                        if d1 > 0x130 {
                            x.add8(Shot::SUB_STEP, 2);
                        }
                    } else if d1 < 0x50 {
                        x.add8(Shot::SUB_STEP, 2);
                    }
                    x.move_by_table_xy();
                    x.anim_step();
                }
                2 => {
                    turn(x);
                    let k = (x.u8(Shot::SETTING) as u32) << 1;
                    let want_a = player_data().orbit_target_angles.at8(x.b, k);
                    let want_b = player_data().orbit_target_angles.at8(x.b, k + 1);
                    if want_a == x.u8(ARC_SIDE) && want_b == x.u8(Obj::SPEED_INDEX) {
                        x.add8(Shot::SUB_STEP, 2);
                    }
                    x.anim_step();
                }
                _ => {
                    x.move_by_table_xy();
                    x.anim_step();
                    x.cull_offscreen();
                }
            },
        }
    }

    /// `0x52a9e`: advances one rotation step every `+0x58` frames.
    fn turn<R: Bench>(x: &mut Cx<'_, R>) {
        x.move_by_table_xy();
        x.add8(TURN_TIMER, 0xff);
        if x.u8(TURN_TIMER) != 0 {
            return;
        }
        let mut d3 = ((x.u8(ARC_ROW) & 8) >> 3) as u32;
        let d4 = (x.u8(ARC_SIDE) >> 3) as u32;
        d3 += d4;
        let step = player_data().orbit_turn_steps.at8(x.b, d3);
        let timer = x.u8(TURN_DELAY);
        x.set8(TURN_TIMER, timer);
        let angle = x.u8(Obj::SPEED_INDEX).wrapping_add(step) & 0x1f;
        x.set8(Obj::SPEED_INDEX, angle);
        let low = angle & 0xf;
        let row = x.u8(ARC_ROW).wrapping_add(x.u8(ARC_SIDE)) as u32;
        let at = TURN_TABLE + row;
        let expected = byte(x.b, at);
        if low != expected {
            return;
        }
        let d5 = x.u8(ARC_ROW) & 6;
        *x.c.d5() = d5 as u32;
        if d5 == 6 && low == byte(x.b, at) {
            let flip = x.u8(f::FLIP) ^ 0x20;
            x.set8(f::FLIP, flip);
            let v = x.u8(ARC_SIDE) ^ 0x10;
            x.set8(ARC_SIDE, v);
        }
        let next = x.u8(ARC_ROW).wrapping_add(2) & 0xf;
        x.set8(ARC_ROW, next);
        let anim = byte(x.b, at + 1);
        x.anim_start(ANIMS.0, (anim as u32) as u16);
    }
}

/// Type 18 (0x53a08): cleric's burst of flames (five variants). 0: starting circle; 1: ball in flight with random
/// creations; 2: spark; 3: ember; 4: ring that turns around the caster.
mod fountain {
    use super::*;

    /// Countdown of the current phase (word).
    const TIMER: u32 = 0x5a;
    /// Creations left (h).
    const SPAWNS: u32 = 0x5c;
    /// Rotation step of the ring.
    const TURN_STEP: u32 = 0x5d;
    /// Countdown of the screen flash (word).
    const FLASH_TIMER: u32 = 0x5e;

    const ANIMS: crate::world::data::RomAddr = crate::world::data::RomAddr(0x15cb54);

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.state(0x53a16) {
            0 => {
                match x.u8(Obj::VARIANT) {
                    0 => init_ring(x),
                    1 => init_ball(x),
                    2 => init_spark(x),
                    3 => init_ember(x),
                    _ => init_halo(x),
                }
            }
            2 => {
                match x.u8(Obj::VARIANT) {
                    0 => ring(x),
                    1 => ball(x),
                    2 => spark(x),
                    3 => ember(x),
                    _ => halo(x),
                }
            }
            _ => x.free_pshl(),
        }
    }

    fn init_ring<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(Obj::STATE, 2);
        x.set16(TIMER, 0x10);
        let owner = x.linked(f::OWNER);
        x.copy_pos(owner);
        let n = 0x45;
        x.select_speed(&crate::world::objects::motion::SPEEDS_B, n);
        x.move_xy();
        x.anim(ANIMS.0, 0);
        x.reinsert();
    }

    fn init_ball<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(Obj::STATE, 2);
        x.set16(TIMER, 1);
        x.set8(SPAWNS, 2);
        let owner = x.linked(f::OWNER);
        let flip = owner.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        x.copy_pos(owner);
        x.set16(Obj::VX, 0xe80);
        x.set16(Obj::AX, -0x40i16 as u16);
        let mut dx = 0x46u16;
        if x.u8(f::FLIP) == 0 {
            let v = x.u32(Obj::VX).wrapping_neg();
            x.set32(Obj::VX, v);
            dx = dx.wrapping_neg();
        }
        x.add16(Obj::X, dx);
        x.add16(Obj::Y, 0xffff);
        x.anim(ANIMS.0, 3);
        x.set16(FLASH_TIMER, 6);
        vars::SCROLL_Y_PLANE1.set(x.b, 0x100);
        crate::services::palette::set_flash_color(x.b, crate::services::palette::FLASH_COLOR_RED);
        x.reinsert();
    }

    fn init_spark<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(Obj::STATE, 2);
        let owner = x.linked(f::OWNER);
        let flip = owner.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        x.copy_pos(owner);
        x.add16(Obj::Y, 1);
        x.anim(ANIMS.0, 4);
        x.reinsert();
    }

    fn init_ember<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(Obj::STATE, 2);
        x.set16(TIMER, 0x90);
        let r = x.rand_and(7);
        x.add16(TIMER, r as u16);
        let owner = x.linked(f::OWNER);
        let flip = owner.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        x.copy_pos(owner);
        x.add16(Obj::Y, 1);
        x.anim(ANIMS.0, 5);
        x.reinsert();
    }

    fn init_halo<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(Obj::STATE, 2);
        x.set16(TIMER, 1);
        x.set8(SPAWNS, 4);
        let owner = x.linked(f::OWNER);
        let flip = owner.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        x.copy_pos(owner);
        let mut dx = 0x30u16;
        x.set8(Obj::SPEED_INDEX, 8);
        if x.u8(f::FLIP) == 0 {
            dx = dx.wrapping_neg();
            x.set8(Obj::SPEED_INDEX, 0x18);
        }
        x.add16(Obj::X, dx);
        let n = 0x4a;
        x.select_speed(&crate::world::objects::motion::SPEEDS_B, n);
        x.move_xy();
        x.anim(ANIMS.0, 3);
        x.set16(FLASH_TIMER, 6);
        vars::SCROLL_Y_PLANE1.set(x.b, 0x100);
        crate::services::palette::set_flash_color(x.b, crate::services::palette::FLASH_COLOR_RED);
        x.reinsert();
    }

    // --- active states ----------------------------------------------------------------------------

    fn ring<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(Shot::SUB_STATE) == 0 {
            x.add16(TIMER, 0xffff);
            if x.u16(TIMER) == 0 {
                x.set16(TIMER, 0x10);
                x.add8(Shot::SETTING, 1);
                let step = x.u8(Shot::SETTING);
                let mut play = step == 1;
                if !play {
                    x.set16(TIMER, 0x20);
                    play = step == 2;
                }
                if play {
                    x.anim(ANIMS.0, step as u32);
                    return;
                }
                x.add8(Shot::SUB_STATE, 2);
                x.set16(TIMER, 0x14);
                let mut i = x.u8(TURN_STEP) as u32;
                if x.u8(f::FLIP) != 0 {
                    i += 6;
                }
                let v = player_data().fountain_turn.at8(x.b, i);
                x.set8(TURN_STEP, v);
            }
            x.anim_step();
        } else {
            x.add16(TIMER, 0xffff);
            if x.u16(TIMER) == 0 {
                x.add8(Obj::STATE, 2);
            }
            let want: u8 = if x.u8(f::FLIP) != 0 { 8 } else { 0x18 };
            if want != x.u8(Obj::SPEED_INDEX) {
                let step = x.u8(TURN_STEP);
                let angle = x.u8(Obj::SPEED_INDEX).wrapping_add(step) & 0x1f;
                x.set8(Obj::SPEED_INDEX, angle);
                let owner = x.linked(f::OWNER);
                x.copy_pos(owner);
                let n = 0x45;
                x.select_speed(&crate::world::objects::motion::SPEEDS_B, n);
                x.move_xy();
            }
            x.anim_step();
        }
    }

    fn ball<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u16(FLASH_TIMER) != 0 {
            x.add16(FLASH_TIMER, 0xffff);
            if x.u16(FLASH_TIMER) == 0 {
                vars::SCROLL_Y_PLANE1.set(x.b, 0);
            } else if x.u16(FLASH_TIMER) == 2 {
                crate::services::palette::set_flash_color(x.b, crate::actors::families::small::LIGHTNING_END);
            }
        }
        x.move_x();
        x.add16(TIMER, 0xffff);
        let t = x.u16(TIMER) & 3;
        x.set16(TIMER, t);
        if t == 0 {
            spawn(x, 2, None);
        }
        x.add8(SPAWNS, 0xff);
        let t = x.u8(SPAWNS) & 3;
        x.set8(SPAWNS, t);
        if t == 0 {
            let r = x.rand_and(0xf);
            if 0x3f >> r & 1 != 0 {
                spawn(x, 3, None);
            }
        }
        x.cull_offscreen();
        if x.u8(Obj::STATE) == 6 {
            vars::SCROLL_Y_PLANE1.set(x.b, 0);
        }
        x.anim_step();
    }

    /// Creates an ember or a spark linked to the ball (`0x53992`, `0x539b2`).
    fn spawn<R: Bench>(x: &mut Cx<'_, R>, variant: u8, _power: Option<(u8, u8)>) {
        if let Some(o) = x.alloc(&pools::PSHL) {
            o.set_u8(x.b, Obj::ACTIVE, 1);
            o.set_u8(x.b, Obj::KIND, 0x12);
            o.set_u8(x.b, Obj::VARIANT, variant);
            o.set_u16(x.b, f::OWNER, x.o.0 as u16);
        }
    }

    fn spark<R: Bench>(x: &mut Cx<'_, R>) {
        if x.anim_step() {
            x.add8(Obj::STATE, 2);
        }
    }

    fn ember<R: Bench>(x: &mut Cx<'_, R>) {
        x.add16(TIMER, 0xffff);
        if x.u16(TIMER) == 0 {
            x.add8(Obj::STATE, 2);
        }
        x.anim_step();
    }

    fn halo<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(Shot::SUB_STATE) == 0 {
            if x.u16(FLASH_TIMER) != 0 {
                x.add16(FLASH_TIMER, 0xffff);
                if x.u16(FLASH_TIMER) == 0 {
                    x.add8(Shot::SUB_STATE, 2);
                    x.set16(TIMER, 1);
                    x.set8(SPAWNS, 4);
                    vars::SCROLL_Y_PLANE1.set(x.b, 0);
                } else if x.u16(FLASH_TIMER) == 2 {
                    crate::services::palette::set_flash_color(x.b, crate::actors::families::small::LIGHTNING_END);
                }
            }
            x.anim_step();
        } else {
            x.add16(TIMER, 0xffff);
            if x.u16(TIMER) == 0 {
                x.set16(TIMER, 1);
                let step: u8 = if x.u8(f::FLIP) != 0 { 0xff } else { 1 };
                let angle = x.u8(Obj::SPEED_INDEX).wrapping_add(step) & 0x1f;
                x.set8(Obj::SPEED_INDEX, angle);
                let owner = x.linked(f::OWNER);
                let flip = owner.u8(x.b, f::FLIP);
                x.set8(f::FLIP, flip);
                x.copy_pos(owner);
                let dx = signed(0x30, x.u8(f::FLIP) == 0);
                x.add16(Obj::X, dx);
                let n = 0x4a;
                x.select_speed(&crate::world::objects::motion::SPEEDS_B, n);
                x.move_xy();
                let want: u8 = if x.u8(f::FLIP) != 0 { 8 } else { 0x18 };
                if want == x.u8(Obj::SPEED_INDEX) {
                    x.add8(Obj::STATE, 2);
                }
            }
            x.add8(SPAWNS, 0xff);
            let t = x.u8(SPAWNS) & 3;
            x.set8(SPAWNS, t);
            if t == 0 {
                spawn(x, 2, None);
            }
            x.anim_step();
        }
    }
}

handlers! {
    PSHL_6 0x527ec "pshl_6" orbit::run,
    PSHL_18 0x53a08 "pshl_18" fountain::run,
}
