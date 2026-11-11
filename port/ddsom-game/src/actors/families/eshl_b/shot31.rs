use super::*;
use crate::world::data::RomAddr;
use crate::actors::families::shell::vars;
use crate::actors::families::shell::records::{Host, view};

const ANIMS: RomAddr = RomAddr(0x16f504);
const SOUND_SPAWN: crate::services::sound::Sound = crate::services::sound::CMD_334_434;
const SOUND_HIT: crate::services::sound::Sound = crate::services::sound::CMD_138;
const SEARCH: u8 = 0;
const HUNT: u8 = 2;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(Shot31::SOURCE) == 0 {
        return kill(x);
    }
    match x.u8(Obj::STATE) {
        0 => init(x),
        2 => active(x),
        4 => fly_away(x),
        _ => x.free_eshl(),
    }
}

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Obj::STATE, 2);
    x.set8(Base::BOX_SET, 7);
    x.set8(Shot::ATTACK_ID, 7);
    x.set8(Shot::ATTACK_POWER, 5);
    let source = x.linked(Shot31::SOURCE);
    copy_position16(x, source);
    let flip = source.u8(x.b, Base::FLIP);
    x.set8(Base::FLIP, flip);
    x.add16(Obj::Y, 0xffff);
    x.set8(Shot31::TIMER, 0x30);
    let (dx, angle) = if flip != 0 { (0x4a, 8) } else { (-0x4ai16 as u16, 0x18) };
    x.set8(Obj::SPEED_INDEX, angle);
    x.add16(Obj::X, dx);
    x.add16(Obj::Z, 0x32);
    x.anim_start(ANIMS.0, 0);
    speed_table(x, 0x30);
}

fn fly_away<R: Bench>(x: &mut Cx<'_, R>) {
    x.move_by_table_xy();
    x.anim_step();
    x.cull_offscreen();
}

/// Bounce sound when the animation raises its flag (`+0x33`).
fn animation_cue<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Base::ANIM_EVENT) != 0 {
        x.set8(Base::ANIM_EVENT, 0);
        crate::services::sound::play(x.b, SOUND_SPAWN, x.o);
    }
}

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    let source = x.linked(Shot31::SOURCE);
    if (source.u16(x.b, Obj::STATE) as i16) >= 0x406 {
        return fly_away(x);
    }
    match x.u8(Base::SUB_STATE) {
        SEARCH => {
            animation_cue(x);
            x.add8(Shot31::TIMER, 0xff);
            if (x.u8(Shot31::TIMER) as i8) < 0 {
                x.add8(Base::SUB_STATE, 2);
                x.set8(Shot31::TURN, 0xfc);
                if source.u16(x.b, Shot::OWNER) != 0 {
                    let target = link_of(x, source, Shot::OWNER);
                    let d = x.u16(Obj::Y).wrapping_sub(target.u16(x.b, Obj::Y));
                    if (d as i16) < 0 {
                        x.set8(Shot31::TURN, 4);
                    }
                }
            }
            x.anim_step();
            x.move_by_table_xy();
        }
        HUNT => {
            animation_cue(x);
            x.add8(Shot31::TIMER, 0xff);
            if (x.u8(Shot31::TIMER) as i8) < 0 {
                x.set8(Shot31::TIMER, 0xa);
                let to = (source.u16(x.b, Obj::X), source.u16(x.b, Obj::Y));
                let angle = crate::actors::aids::direction_from(x.b, x.o, to).wrapping_add(x.u8(Shot31::TURN)) & 0x1f;
                x.set8(Obj::SPEED_INDEX, angle);
            }
            let touching = |x: &mut Cx<'_, R>, a: u16, b: u16| a.wrapping_sub(b).wrapping_add(8) <= 0x20 && vars::GAME_PAUSED.get(x.b) == 0;
            let (sx, sy) = (source.u16(x.b, Obj::X), source.u16(x.b, Obj::Y));
            let (mx, my) = (x.u16(Obj::X), x.u16(Obj::Y));
            let hit = source.u8(x.b, Obj::STATE) != 4 && touching(x, sx, mx) && touching(x, sy, my);
            if hit {
                crate::services::sound::play(x.b, SOUND_HIT, x.o);
                x.set8(Obj::STATE, st::FREE);
                source.set_u8(x.b, Host::SHOT_ALIVE, 0);
            } else {
                x.move_by_table_xy();
                x.anim_step();
            }
        }
        _ => {}
    }
    x.reinsert();
}

view! {
    /// Fields of shot 31.
    Shot31 {
        /// Countdown (h).
        timer, set_timer, TIMER: u8 @ 0x42;
        /// Angle added to the aim (h).
        turn, set_turn, TURN: u8 @ 0x58;
        /// Source object (16-bit pointer).
        source, set_source, SOURCE: u16 @ 0x5c;
    }
}
