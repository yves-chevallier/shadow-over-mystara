//! Type 3, hit state (`+4 = 4`, 0x5caa0). `+5`: 0 and 2 table reaction (`+0x49`), 4 fall and get up, 6 second reaction
//! table, 8 grabbed by a player (the Bow Gnoll runs away on foot). Zone and shadow are tested unless `+0xac` is
//! non-zero.

use crate::actors::families::cx::Bench;
use crate::world::data::RomAddr;
use super::*;
use crate::world::objects::Current;
use crate::world::objects::Obj;

/// Reaction tables shared by the hits, indexed by code `+0x49`.
const REACT_FIRST: RomAddr = RomAddr(0x5cbe8);
const REACT_SECOND: RomAddr = RomAddr(0x5cd14);

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    let freeze = x.u8(f::FREEZE) as i8;
    if freeze > 0 {
        x.set8(f::FREEZE, (freeze - 1) as u8);
    } else {
        match x.u8(f::SUB) {
            0 | 2 => react(x, REACT_FIRST.0),
            4 => fall(x),
            6 => react(x, REACT_SECOND.0),
            8 => grab(x),
            _ => {}
        }
    }
    if x.u8(Bow::NO_ZONE) == 0 {
        zone_and_terrain(x);
    }
    x.reinsert();
}

/// Fall then get up (0x5cc94).
fn fall<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            mv(x, 0xe);
        }
        2 => {
            if paused(x) {
                return;
            }
            let n = x.u16(Enemy::RECOVER).wrapping_sub(1);
            x.set16(Enemy::RECOVER, n);
            if (n as i16) > 0 {
                return;
            }
            x.set8(Enemy::DOWNED, 0);
            x.set8(Enemy::REACTION, 0);
            x.set8(Enemy::ACTIVE, 2);
            x.add8(f::SUB2, 2);
            mv(x, 7);
        }
        4 => {
            if paused(x) {
                return;
            }
            if x.anim_step() {
                x.set8(Enemy::ACTIVE, 1);
                set_state_long(x, STANCE);
                x.set8(Enemy::ACTIVE, 1);
                x.set8(Enemy::DOWNED, 0);
            }
        }
        _ => {}
    }
}

/// 0x5cae0: grabbed by a player: the Bow Gnoll turns around toward the nearest screen edge (according to camera
/// `A5+0x34` and the half of the screen), runs away then disappears.
pub(super) fn grab<R: Bench>(x: &mut Cx<'_, R>) {
    if paused(x) {
        return;
    }
    match x.u8(f::SUB2) {
        0 => {
            x.set8(Bow::NO_ZONE, 1);
            let flipped = x.u8(f::FLIP) != 0;
            // `turn`: turns first (animation 3); otherwise leaves right away
            let turn = if var::SCENE.get(x.b) == 4 {
                flipped
            } else {
                if !lib::in_right_half(x) {
                    flipped
                } else {
                    !flipped
                }
            };
            if turn {
                x.add8(f::SUB2, 2);
                x.set8(Enemy::VULNERABLE, 1);
                return mv(x, 3);
            }
            x.add8(f::SUB2, 4);
            run_away(x);
        }
        2 => {
            if !x.anim_step() {
                return;
            }
            x.add8(f::SUB2, 2);
            let flip = x.u8(f::FLIP) ^ 0x20;
            x.set8(f::FLIP, flip);
            run_away(x);
        }
        4 => {
            x.anim_step();
            x.move_by_table_xy();
            if x.in_arena_default() {
                return;
            }
            if (x.u8(Bow::TIMED) as i8) < 0 && x.u16(f::HOST) != 0 {
                let host = x.linked(f::HOST);
                host.set_u8(x.b, Obj::SPEED_INDEX, 1);
            }
            set_state_long(x, st::DEAD_LONG);
            detach_from_host(x);
        }
        _ => {}
    }
}

/// 0x5cb70: runs off toward the edge (animation 2, speed 0x21).
fn run_away<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Enemy::VULNERABLE, 1);
    walk_angle(x);
    mv(x, 2);
    x.select_speed(&crate::world::objects::motion::SPEEDS_B, 0x21);
}
