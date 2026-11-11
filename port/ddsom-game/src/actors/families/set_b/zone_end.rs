use super::*;
use crate::actors::families::set::vars;
use crate::world::objects::Obj;
use crate::actors::families::set::fields::SetObj;
use crate::actors::families::set_b::fields::ZoneEndObj;

const EVENT_TYPE: u32 = 0x3f;
const LOCK_X: u16 = 0x1c0;
const SCROLL_LOCK: u32 = 0x51e;
const EVENT_FLAG: u32 = 0x43;
const DELAY: u16 = 0x78;
/// The SET objects of this type have their orientation (`+0x2c` bit 5) inverted at the end.
const FLIPPED_TYPE: u8 = 0x15;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => {
            x.add8(Obj::STATE, 2);
            sub_state(x);
        }
        2 => sub_state(x),
        _ => x.free(&pools::SET),
    }
}

fn sub_state<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(SetObj::SUBSTATE) {
        0 => {
            if x.a5_u8(vars::PLAYERS_ALIVE) == 0 || x.a5_u16(ram::CAMERA_X) < LOCK_X {
                return;
            }
            x.add8(SetObj::SUBSTATE, 2);
            x.set_a5_u16(SCROLL_LOCK, 4);
            x.set_a5_u8(EVENT_FLAG, 0x1f);
            let event = scene_set(x, EVENT_TYPE as u8).0 as u16;
            x.set16(ZoneEndObj::EVENT, event);
        }
        2 => {
            let event = x.linked(ZoneEndObj::EVENT);
            if event.u8(x.b, ZoneEndObj::STARTED) == 0 {
                return;
            }
            if event.u8(x.b, ZoneEndObj::RUNNING) == 0 {
                x.add8(SetObj::SUBSTATE, 2);
                let v = x.a5_u16(ram::CAMERA_X);
                x.set_a5_u16(vars::CAMERA_MAX_X, v);
                x.set16(ZoneEndObj::DELAY, DELAY);
                let _ = crate::actors::effects_aids::scenery_11(x.b, 0, 0);
                return;
            }
            x.add8(Obj::STATE, 2);
            x.set_a5_u16(SCROLL_LOCK, 0);
            x.set_a5_u8(EVENT_FLAG, 0xff);
            let v = x.a5_u16(ram::CAMERA_X);
            x.set_a5_u16(vars::CAMERA_MIN_X, v);
            // 33 slots: the last is the first EFFCT object (contiguous pool)
            for i in 0..33u32 {
                let o = pools::SET.slot(i);
                if o.u8(x.b, Obj::ACTIVE) != 0 && o.u8(x.b, Obj::KIND) == FLIPPED_TYPE {
                    let v = o.u8(x.b, SetObj::ATTR) ^ 0x20;
                    o.set_u8(x.b, SetObj::ATTR, v);
                }
            }
        }
        4 => {
            let n = x.u16(ZoneEndObj::DELAY).wrapping_sub(1);
            x.set16(ZoneEndObj::DELAY, n);
            if n != 0 {
                return;
            }
            x.add8(Obj::STATE, 2);
            x.set_a5_u16(SCROLL_LOCK, 0);
            eshl_creators::linked_34(&mut Pure, x.b, x.o, 3);
            crate::services::sound::play(x.b, crate::services::sound::CMD_1FA, x.o);
            crate::services::sound::play(x.b, crate::services::sound::CMD_22, x.o);
        }
        _ => {}
    }
}
